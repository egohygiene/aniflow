//! Local ownership and atomic entry storage. All mutation holds one namespace lock.
use super::*;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use crate::provider::canonical_sha256;
use crate::CancellationToken;

const MARKER_SCHEMA: &str = "aniflow.cache-owner/v1";
const MAXIMUM_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
const MAXIMUM_SCAN_NODES: usize = 250_000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker { schema: String, owner: String }

/// Exclusive create has no wait and no unsafe stale-PID reclamation. A crashed
/// holder leaves its lock as evidence; operators must reconcile it explicitly.
pub(crate) struct ExclusiveWriter { path: PathBuf, token: Vec<u8> }
impl ExclusiveWriter {
    pub(crate) fn acquire(root: &Path, name: &str) -> Result<Self> {
        require_safe_path(root)?;
        let path = root.join(name);
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|error| if error.kind() == std::io::ErrorKind::AlreadyExists {
                cache_error(CacheFailureCode::Busy, "writer lock exists; finish or reconcile the owning operation before retrying")
            } else { io_error(error) })?;
        let token = format!("aniflow-exclusive-writer/v1 {} {}\n", std::process::id(), Utc::now().to_rfc3339()).into_bytes();
        let guard = Self { path, token };
        file.write_all(&guard.token).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        sync_directory(root)?;
        Ok(guard)
    }
}
impl Drop for ExclusiveWriter {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
            && fs::read(&self.path).is_ok_and(|bytes| bytes == self.token) {
            let _ = fs::remove_file(&self.path);
            if let Some(parent) = self.path.parent() { let _ = sync_directory(parent); }
        }
    }
}

pub(crate) struct CacheSession { pub(crate) policy: CachePolicy, _writer: ExclusiveWriter }
impl CacheSession {
    pub(crate) fn open(policy: &CachePolicy, initialize: bool) -> Result<Self> {
        policy.validate()?;
        let mut policy = policy.clone();
        policy.root = absolute_path(&policy.root)?;
        require_safe_path(&policy.root)?;
        if !policy.root.exists() {
            if !initialize { return Err(cache_error(CacheFailureCode::OwnershipMismatch, "cache namespace does not exist")); }
            // Only an absent leaf is initialized; no populated directory is adopted.
            let parent = policy.root.parent().ok_or_else(|| cache_error(CacheFailureCode::UnsafePath, "cache root has no parent"))?;
            require_safe_path(parent)?;
            fs::create_dir(&policy.root).map_err(io_error)?;
            write_new(&policy.root.join("owner.json"), &Marker { schema: MARKER_SCHEMA.to_owned(), owner: policy.owner.clone() })?;
            fs::create_dir(policy.root.join("entries")).map_err(io_error)?;
            fs::create_dir(policy.root.join("temporary")).map_err(io_error)?;
            sync_directory(&policy.root)?;
            sync_directory(parent)?;
        }
        validate_root(&policy)?;
        let writer = ExclusiveWriter::acquire(&policy.root, "writer.lock")?;
        Ok(Self { policy, _writer: writer })
    }

    pub(crate) fn path(&self, key: &str) -> Result<PathBuf> {
        require_digest(key)?;
        Ok(self.policy.root.join("entries").join(key))
    }

    pub(crate) fn lookup(&self, key: &str, cancellation: &CancellationToken) -> Result<Option<(CacheEntry, bool)>> {
        let path = self.path(key)?;
        if !path.try_exists().map_err(io_error)? { return Ok(None); }
        let entry = read_entry(&self.policy, &path, true, cancellation)?;
        let expired = is_expired(&self.policy, &entry, Utc::now());
        Ok(Some((entry, expired)))
    }

    pub(crate) fn preflight(&self, additional_bytes: u64, additional_entries: u64) -> Result<()> {
        let snapshot = inspect_cache(&self.policy)?;
        if snapshot.byte_count.checked_add(additional_bytes).is_none_or(|size| size > self.policy.maximum_bytes)
            || (snapshot.entries.len() as u64).checked_add(additional_entries).is_none_or(|count| count > self.policy.maximum_entries) {
            return Err(cache_error(CacheFailureCode::StorageLimit, "cache budget exhausted; inspect and explicitly prune or adjust the policy"));
        }
        ensure_storage(&self.policy.root, additional_bytes, self.policy.minimum_free_bytes)
    }

    pub(crate) fn staging(&self) -> Result<tempfile::TempDir> {
        tempfile::Builder::new().prefix("entry-").tempdir_in(self.policy.root.join("temporary")).map_err(io_error)
    }

    pub(crate) fn publish(&self, staging: tempfile::TempDir, mut payload: CacheEntryPayload, cancellation: &CancellationToken) -> Result<(CacheEntry, bool)> {
        check_cancelled(cancellation)?;
        payload.contents = inventory(&staging.path().join("snapshot"), &self.policy, true, cancellation)?;
        let mut entry = CacheEntry { schema: CACHE_ENTRY_SCHEMA_V1.to_owned(), entry_sha256: String::new(), payload };
        entry.entry_sha256 = canonical_sha256(&entry.payload)?;
        let destination = self.path(&entry.payload.key_sha256)?;
        if destination.try_exists().map_err(io_error)? {
            let existing = read_entry(&self.policy, &destination, true, cancellation)?;
            if existing.payload.result_sha256 != entry.payload.result_sha256 {
                return Err(cache_error(CacheFailureCode::ConflictingEntry, "the same cache key produced different accepted results; invalidate explicitly before replacing it"));
            }
            return Ok((existing, false));
        }
        write_new(&staging.path().join("entry.json"), &entry)?;
        let bytes = tree_size(staging.path())?;
        if bytes > self.policy.maximum_entry_bytes { return Err(cache_error(CacheFailureCode::StorageLimit, "sealed entry exceeds the maximum entry bytes")); }
        // The staging bytes are already included in inventory; publication adds a
        // namespace entry, not another copy of those bytes.
        self.preflight(0, 1)?;
        sync_tree(staging.path(), cancellation)?;
        check_cancelled(cancellation)?;
        fs::rename(staging.path(), &destination).map_err(io_error)?;
        sync_directory(&self.policy.root.join("entries"))?;
        Ok((entry, true))
    }
}

pub fn inspect_cache(policy: &CachePolicy) -> Result<CacheInspection> {
    policy.validate()?;
    let mut policy = policy.clone(); policy.root = absolute_path(&policy.root)?;
    require_safe_path(&policy.root)?;
    let mut report = CacheInspection { schema: CACHE_INSPECTION_SCHEMA_V1.to_owned(), root: policy.root.clone(), owner: policy.owner.clone(), exists: false,
        writer_present: false, entries: Vec::new(), temporary_entries: 0, byte_count: 0 };
    if !policy.root.try_exists().map_err(io_error)? { return Ok(report); }
    validate_root(&policy)?;
    report.exists = true;
    report.writer_present = policy.root.join("writer.lock").try_exists().map_err(io_error)?;
    report.byte_count = tree_size(&policy.root)?;
    let now = Utc::now();
    for path in children(&policy.root.join("entries"))? {
        let key = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_owned();
        require_digest(&key)?;
        let bytes = tree_size(&path)?;
        let (state, created_at, diagnostic) = match read_entry(&policy, &path, false, &CancellationToken::default()) {
            Ok(entry) => (if is_expired(&policy, &entry, now) { CacheEntryState::Expired } else { CacheEntryState::Available }, Some(entry.payload.created_at), None),
            Err(error) => (CacheEntryState::Invalid, None, Some(error.message().to_owned())),
        };
        report.entries.push(CacheEntrySummary { key_sha256: key, state, byte_count: bytes, created_at, diagnostic });
    }
    report.temporary_entries = children(&policy.root.join("temporary"))?.len() as u64;
    Ok(report)
}

/// Explicitly evict a sealed, owned entry; unknown/corrupt entries are never deleted.
pub fn invalidate_cache(policy: &CachePolicy, key: &str) -> Result<CacheOperation> {
    require_digest(key)?;
    let session = CacheSession::open(policy, false)?;
    let path = session.path(key)?;
    let mut report = operation(policy, "invalidate", false);
    if !path.try_exists().map_err(io_error)? { return Ok(report); }
    read_entry(&session.policy, &path, false, &CancellationToken::default())?;
    report.selected_keys.push(key.to_owned());
    report.reclaimed_bytes = tree_size(&path)?;
    fs::remove_dir_all(path).map_err(io_error)?;
    sync_directory(&session.policy.root.join("entries"))?;
    report.removed_keys.push(key.to_owned());
    Ok(report)
}

/// Deterministic expiry-first, then oldest-first retention. Preview is read-only.
pub fn prune_cache(policy: &CachePolicy, dry_run: bool) -> Result<CacheOperation> {
    let session = if dry_run { None } else { Some(CacheSession::open(policy, false)?) };
    let snapshot = inspect_cache(policy)?;
    if dry_run && snapshot.writer_present { return Err(cache_error(CacheFailureCode::Busy, "cache writer is active; retry retention preview later")); }
    let mut candidates = snapshot.entries.iter().filter(|entry| entry.state != CacheEntryState::Invalid).collect::<Vec<_>>();
    candidates.sort_by(|a,b| (a.state != CacheEntryState::Expired, a.created_at, &a.key_sha256).cmp(&(b.state != CacheEntryState::Expired, b.created_at, &b.key_sha256)));
    let mut bytes = snapshot.byte_count;
    let mut count = snapshot.entries.len() as u64;
    let mut report = operation(policy, "prune", dry_run);
    for entry in candidates {
        if report.selected_keys.len() as u64 >= policy.maximum_prune_entries { break; }
        if entry.state != CacheEntryState::Expired && bytes <= policy.maximum_bytes && count <= policy.maximum_entries { continue; }
        report.selected_keys.push(entry.key_sha256.clone());
        if let Some(session) = &session {
            let path = session.path(&entry.key_sha256)?;
            read_entry(&session.policy, &path, false, &CancellationToken::default())?;
            fs::remove_dir_all(path).map_err(io_error)?;
            sync_directory(&session.policy.root.join("entries"))?;
            report.removed_keys.push(entry.key_sha256.clone());
            report.reclaimed_bytes = report.reclaimed_bytes.saturating_add(entry.byte_count);
        }
        bytes = bytes.saturating_sub(entry.byte_count); count = count.saturating_sub(1);
    }
    Ok(report)
}

fn operation(policy: &CachePolicy, name: &str, dry_run: bool) -> CacheOperation {
    CacheOperation { schema: CACHE_OPERATION_SCHEMA_V1.to_owned(), operation: name.to_owned(), owner: policy.owner.clone(), selected_keys: Vec::new(), removed_keys: Vec::new(), reclaimed_bytes: 0, dry_run }
}

fn read_entry(policy: &CachePolicy, path: &Path, verify_bytes: bool, cancellation: &CancellationToken) -> Result<CacheEntry> {
    require_safe_path(path)?;
    let entry: CacheEntry = read_document(&path.join("entry.json"))?;
    if entry.schema != CACHE_ENTRY_SCHEMA_V1 || entry.payload.owner != policy.owner {
        return Err(cache_error(CacheFailureCode::IncompatibleEntry, "cache entry schema or owner differs"));
    }
    require_digest(&entry.payload.key_sha256)?;
    require_digest(&entry.payload.result_sha256)?;
    require_digest(&entry.payload.origin_plan_sha256)?;
    entry.payload.checkpoint.validate()?;
    if path.file_name().and_then(|n| n.to_str()) != Some(entry.payload.key_sha256.as_str())
        || canonical_sha256(&entry.payload)? != entry.entry_sha256 || entry.payload.created_at > Utc::now()
        || entry.payload.stage_id != entry.payload.checkpoint.stage_id {
        return Err(cache_error(CacheFailureCode::CorruptEntry, "entry identity, timestamp or seal is invalid"));
    }
    let observed = inventory(&path.join("snapshot"), policy, verify_bytes, cancellation)?;
    if observed.len() != entry.payload.contents.len() || observed.iter().zip(&entry.payload.contents).any(|(a,b)| {
        a.relative_path != b.relative_path || a.kind != b.kind || a.byte_count != b.byte_count || (verify_bytes && a.sha256 != b.sha256)
    }) { return Err(cache_error(CacheFailureCode::CorruptEntry, "snapshot inventory differs from its sealed evidence")); }
    if children(path)?.len() != 2 || tree_size(path)? > policy.maximum_entry_bytes {
        return Err(cache_error(CacheFailureCode::CorruptEntry, "entry contains unexpected content or exceeds bounds"));
    }
    Ok(entry)
}

fn is_expired(policy: &CachePolicy, entry: &CacheEntry, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(entry.payload.created_at).num_seconds() as u64 >= policy.maximum_age_seconds
}

fn validate_root(policy: &CachePolicy) -> Result<()> {
    require_safe_path(&policy.root)?;
    let marker: Marker = read_document(&policy.root.join("owner.json"))?;
    if marker.schema != MARKER_SCHEMA || marker.owner != policy.owner { return Err(cache_error(CacheFailureCode::OwnershipMismatch, "cache root is not owned by the selected policy")); }
    for path in children(&policy.root)? {
        match path.file_name().and_then(|n| n.to_str()) {
            Some("owner.json" | "writer.lock" | "entries" | "temporary") => {},
            _ => return Err(cache_error(CacheFailureCode::OwnershipMismatch, "cache root contains unknown content; refusing to adopt it")),
        }
    }
    for name in ["entries", "temporary"] {
        let path = policy.root.join(name); require_safe_path(&path)?;
        if !fs::symlink_metadata(path).map_err(io_error)?.is_dir() { return Err(cache_error(CacheFailureCode::UnsafePath, "cache layout is not a real directory")); }
    }
    Ok(())
}

pub(crate) fn absolute_path(path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().map_err(io_error)?.join(path) };
    if path.components().any(|part| matches!(part, Component::ParentDir | Component::CurDir)) { return Err(cache_error(CacheFailureCode::UnsafePath, "cache paths cannot contain dot components")); }
    Ok(path)
}

/// Reject links throughout an existing path, including ancestors. Missing suffixes
/// are permitted only for callers that explicitly create their own leaf.
pub(crate) fn require_safe_path(path: &Path) -> Result<()> {
    let path = absolute_path(path)?;
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Err(cache_error(CacheFailureCode::UnsafePath, "cache path contains a symbolic link")),
            Ok(_) => {}, Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}, Err(error) => return Err(io_error(error)),
        }
    }
    Ok(())
}

fn children(path: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for child in fs::read_dir(path).map_err(io_error)? {
        if result.len() >= 4096 { return Err(cache_error(CacheFailureCode::StorageLimit, "cache directory scan exceeds 4096 entries")); }
        result.push(child.map_err(io_error)?.path());
    }
    result.sort(); Ok(result)
}

fn tree_size(path: &Path) -> Result<u64> {
    require_safe_path(path)?;
    let mut bytes = 0_u64;
    for (index, item) in walkdir::WalkDir::new(path).follow_links(false).into_iter().enumerate() {
        if index >= MAXIMUM_SCAN_NODES { return Err(cache_error(CacheFailureCode::StorageLimit, "cache inventory exceeds its scan bound")); }
        let item = item.map_err(io_error)?;
        let metadata = fs::symlink_metadata(item.path()).map_err(io_error)?;
        if !metadata.is_file() && !metadata.is_dir() { return Err(cache_error(CacheFailureCode::UnsafePath, "cache contains linked or special content")); }
        bytes = bytes.checked_add(if metadata.is_file() { metadata.len() } else { 0 }).ok_or_else(|| cache_error(CacheFailureCode::StorageLimit, "cache byte count overflow"))?;
    }
    Ok(bytes)
}

fn inventory(root: &Path, policy: &CachePolicy, hashes: bool, cancellation: &CancellationToken) -> Result<Vec<CacheContent>> {
    require_safe_path(root)?;
    if !fs::metadata(root).map_err(io_error)?.is_dir() { return Err(cache_error(CacheFailureCode::CorruptEntry, "snapshot directory is missing")); }
    let mut result = Vec::new(); let mut total = 0_u64; let mut seen = BTreeSet::new();
    for item in walkdir::WalkDir::new(root).min_depth(1).follow_links(false) {
        check_cancelled(cancellation)?;
        if result.len() as u64 >= policy.maximum_entry_files { return Err(cache_error(CacheFailureCode::StorageLimit, "snapshot file/directory count exceeds policy")); }
        let item = item.map_err(io_error)?; let metadata = fs::symlink_metadata(item.path()).map_err(io_error)?;
        let relative = item.path().strip_prefix(root).map_err(io_error)?.to_str().ok_or_else(|| cache_error(CacheFailureCode::UnsafePath, "snapshot path is not UTF-8"))?.replace('\\', "/");
        if relative.split('/').any(|p| p.is_empty() || p == "." || p == ".." || p.contains(':')) || relative.chars().any(char::is_control) || !seen.insert(relative.clone()) { return Err(cache_error(CacheFailureCode::UnsafePath, "invalid portable snapshot path")); }
        let kind = if metadata.is_dir() { crate::ArtifactKind::Directory } else if metadata.is_file() { crate::ArtifactKind::File } else { return Err(cache_error(CacheFailureCode::UnsafePath, "snapshot has linked or special entries")); };
        let size = if metadata.is_file() { metadata.len() } else { 0 };
        total = total.checked_add(size).ok_or_else(|| cache_error(CacheFailureCode::StorageLimit, "snapshot size overflow"))?;
        if total > policy.maximum_entry_bytes { return Err(cache_error(CacheFailureCode::StorageLimit, "snapshot exceeds entry budget")); }
        let sha256 = if metadata.is_file() && hashes { Some(hash_file(item.path(), size, cancellation)?) } else { None };
        result.push(CacheContent { relative_path: relative, kind, byte_count: size, sha256 });
    }
    result.sort_by(|a,b| a.relative_path.cmp(&b.relative_path)); Ok(result)
}

fn hash_file(path: &Path, size: u64, cancellation: &CancellationToken) -> Result<String> {
    let mut file = File::open(path).map_err(io_error)?.take(size.saturating_add(1));
    let mut digest = Sha256::new(); let mut bytes = 0_u64; let mut buffer = [0_u8; 64 * 1024];
    loop { check_cancelled(cancellation)?; let n = file.read(&mut buffer).map_err(io_error)?; if n == 0 { break; } bytes += n as u64; digest.update(&buffer[..n]); }
    if bytes != size { return Err(cache_error(CacheFailureCode::CorruptEntry, "snapshot file changed while hashing")); }
    Ok(format!("{:x}", digest.finalize()))
}

fn read_document<T: DeserializeOwned>(path: &Path) -> Result<T> {
    require_safe_path(path)?;
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_DOCUMENT_BYTES { return Err(cache_error(CacheFailureCode::CorruptEntry, "cache document is not a bounded regular file")); }
    let mut bytes = Vec::new(); File::open(path).map_err(io_error)?.take(MAXIMUM_DOCUMENT_BYTES + 1).read_to_end(&mut bytes).map_err(io_error)?;
    if bytes.len() as u64 > MAXIMUM_DOCUMENT_BYTES { return Err(cache_error(CacheFailureCode::CorruptEntry, "cache document exceeds bound")); }
    serde_json::from_slice(&bytes).map_err(|error| cache_error(CacheFailureCode::CorruptEntry, format!("invalid cache document: {error}")))
}

fn write_new<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io_error)?;
    if bytes.len() as u64 > MAXIMUM_DOCUMENT_BYTES { return Err(cache_error(CacheFailureCode::StorageLimit, "cache document exceeds bound")); }
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(io_error)?;
    file.write_all(&bytes).map_err(io_error)?; file.sync_all().map_err(io_error)
}

pub(crate) fn ensure_storage(path: &Path, required: u64, reserve: u64) -> Result<()> {
    let available = available_storage(path)?;
    if required.checked_add(reserve).is_none_or(|needed| needed > available) {
        return Err(cache_error(CacheFailureCode::StorageLimit, "insufficient available filesystem storage for bounded stage work"));
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn available_storage(path: &Path) -> Result<u64> {
    let stats = nix::sys::statvfs::statvfs(path).map_err(io_error)?;
    (stats.blocks_available() as u64).checked_mul(stats.fragment_size() as u64).ok_or_else(|| cache_error(CacheFailureCode::StorageLimit, "available storage overflow"))
}
#[cfg(not(unix))]
pub(crate) fn available_storage(_path: &Path) -> Result<u64> {
    Err(cache_error(CacheFailureCode::UnsupportedPlatform, "cache writes require a qualified native storage preflight; this platform is currently unsupported"))
}

fn sync_tree(root: &Path, cancellation: &CancellationToken) -> Result<()> {
    for item in walkdir::WalkDir::new(root).contents_first(true).follow_links(false) {
        check_cancelled(cancellation)?; let item = item.map_err(io_error)?;
        if item.file_type().is_dir() { sync_directory(item.path())?; } else { File::open(item.path()).map_err(io_error)?.sync_all().map_err(io_error)?; }
    }
    Ok(())
}
#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> { File::open(path).map_err(io_error)?.sync_all().map_err(io_error) }
#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> { Ok(()) }
fn check_cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() { Err(cache_error(CacheFailureCode::Cancelled, "cache operation cancelled")) } else { Ok(()) }
}
