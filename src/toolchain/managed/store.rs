use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::de::DeserializeOwned;

use super::catalog::{identifier, package_sha256};
use super::extract::{extract, verify_file};
use super::*;

const OWNER: &[u8] = b"aniflow.managed-store/v1\n";
const MAXIMUM_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;

/// Only this namespace is writable/deletable by lifecycle operations. A store
/// constructor performs no filesystem mutation or acquisition.
#[derive(Debug, Clone)]
pub struct ManagedStore {
    root: PathBuf,
}

pub(crate) fn safe_path(path: &Path) -> crate::Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(dependency(
            "managed store paths must be absolute and contain no parent traversal",
        ));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(dependency(format!(
                    "managed store refuses symlink: {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io(error)),
        }
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> crate::Result<T> {
    safe_path(path)?;
    let metadata = fs::symlink_metadata(path).map_err(io)?;
    if !metadata.is_file() || metadata.len() > MAXIMUM_DOCUMENT_BYTES {
        return Err(dependency(
            "managed document has unsafe type or exceeds byte limit",
        ));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(io)?
        .take(MAXIMUM_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > MAXIMUM_DOCUMENT_BYTES {
        return Err(dependency("managed document exceeds byte limit"));
    }
    serde_json::from_slice(&bytes).map_err(io)
}

fn write_new(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    safe_path(path)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.sync_all().map_err(io)
}

fn sync_directory(path: &Path) -> crate::Result<()> {
    #[cfg(unix)]
    {
        File::open(path).map_err(io)?.sync_all().map_err(io)?;
    }
    Ok(())
}

#[cfg(unix)]
struct Writer {
    _lock: nix::fcntl::Flock<File>,
}
#[cfg(not(unix))]
struct Writer {
    path: PathBuf,
}
#[cfg(not(unix))]
impl Drop for Writer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl ManagedStore {
    pub fn new(root: impl Into<PathBuf>) -> crate::Result<Self> {
        let supplied = root.into();
        let root = if supplied.is_absolute() {
            supplied
        } else {
            std::env::current_dir().map_err(io)?.join(supplied)
        };
        safe_path(&root)?;
        if root.parent().is_none() || root.components().count() < 3 {
            return Err(dependency(
                "managed root must be a dedicated nested directory",
            ));
        }
        Ok(Self { root })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Exact future install root; validates the id without creating anything.
    pub fn package_root(&self, id: &str) -> crate::Result<PathBuf> {
        self.version(id)
    }

    fn owned(&self) -> crate::Result<()> {
        safe_path(&self.root)?;
        let marker = self.root.join("owner");
        safe_path(&marker)?;
        let metadata = fs::symlink_metadata(&marker).map_err(|_| dependency("directory is not an aniflow managed store; existing user directories are never adopted"))?;
        if !metadata.is_file()
            || metadata.len() != OWNER.len() as u64
            || fs::read(marker).map_err(io)? != OWNER
        {
            return Err(dependency("managed store ownership marker mismatch"));
        }
        Ok(())
    }

    fn writer(&self) -> crate::Result<Writer> {
        safe_path(&self.root)?;
        if !self.root.exists() {
            fs::create_dir_all(&self.root).map_err(io)?;
        }
        safe_path(&self.root)?;
        if !self.root.join("owner").exists() {
            if fs::read_dir(&self.root).map_err(io)?.next().is_some() {
                return Err(dependency("refusing to adopt a nonempty unowned directory"));
            }
            match write_new(&self.root.join("owner"), OWNER) {
                Ok(()) => {}
                Err(_) if self.root.join("owner").exists() => self.owned()?,
                Err(error) => return Err(error),
            }
        }
        self.owned()?;
        let path = self.root.join("writer.lock");
        safe_path(&path)?;
        #[cfg(unix)]
        {
            use nix::fcntl::{Flock, FlockArg};
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .map_err(io)?;
            let lock = Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|_| dependency("managed store is busy; another acquisition or lifecycle operation holds its writer lock"))?;
            let writer = Writer { _lock: lock };
            self.initialize_directories()?;
            Ok(writer)
        }
        #[cfg(not(unix))]
        {
            OpenOptions::new().create_new(true).write(true).open(&path).map_err(|_| dependency("managed store is busy; reconcile any interrupted exclusive writer before retrying"))?;
            let writer = Writer { path };
            self.initialize_directories()?;
            Ok(writer)
        }
    }

    fn initialize_directories(&self) -> crate::Result<()> {
        for name in ["versions", "archives", "staging", "quarantine"] {
            let path = self.root.join(name);
            safe_path(&path)?;
            fs::create_dir_all(&path).map_err(io)?;
        }
        Ok(())
    }

    fn version(&self, id: &str) -> crate::Result<PathBuf> {
        identifier(id)?;
        let path = self.root.join("versions").join(id);
        safe_path(&path)?;
        Ok(path)
    }
    fn archive(&self, artifact: &ManagedArtifact) -> PathBuf {
        self.root.join("archives").join(&artifact.sha256)
    }

    fn observed(&self, id: &str) -> crate::Result<InstalledManagedPackage> {
        self.owned()?;
        let root = self.version(id)?;
        safe_path(&root)?;
        let receipt: ManagedReceipt = read_json(&root.join("receipt.json"))?;
        receipt.package.validate()?;
        if receipt.schema != MANAGED_RECEIPT_SCHEMA
            || receipt.catalog_revision.is_empty()
            || receipt.package.id != id
            || package_sha256(&receipt.package)? != receipt.package_sha256
        {
            return Err(dependency("managed receipt identity mismatch"));
        }
        validate_membership(&root, &receipt.package)?;
        for file in receipt.package.files() {
            verify_file(&root.join(&file.path), &file.sha256, file.length)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if file.executable
                    && fs::metadata(root.join(&file.path))
                        .map_err(io)?
                        .permissions()
                        .mode()
                        & 0o111
                        == 0
                {
                    return Err(dependency(
                        "managed executable permission is absent; repair the package",
                    ));
                }
            }
        }
        Ok(InstalledManagedPackage { root, receipt })
    }

    /// Inspect verifies every retained tool/model/library byte against both the
    /// local receipt and the currently selected compiled catalog specification.
    pub fn inspect(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
    ) -> crate::Result<InstalledManagedPackage> {
        catalog.validate()?;
        let expected = catalog.package(id)?;
        let installed = self.observed(id)?;
        if &installed.receipt.package != expected {
            return Err(dependency(
                "managed package id has different bytes in this catalog; immutable package ids cannot be repointed",
            ));
        }
        Ok(installed)
    }

    /// This read-only operation neither creates the root nor starts transport.
    pub fn plan(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
        offline: bool,
    ) -> crate::Result<ManagedPlan> {
        catalog.validate()?;
        let package = catalog.package(id)?;
        safe_path(&self.root)?;
        if self.root.exists() {
            if self.owned().is_err() {
                if !self.root.is_dir() || fs::read_dir(&self.root).map_err(io)?.next().is_some() {
                    return Err(dependency(
                        "directory is not an aniflow managed store; existing user directories are never adopted",
                    ));
                }
            }
        }
        let installed_bytes = package.files().map(|file| file.length).sum();
        let ready = self.inspect(catalog, id).is_ok();
        let corrupt = !ready && self.version(id)?.exists();
        let missing: Vec<_> = if ready {
            Vec::new()
        } else {
            package
                .artifacts
                .iter()
                .filter(|artifact| {
                    verify_file(&self.archive(artifact), &artifact.sha256, artifact.length).is_err()
                })
                .collect()
        };
        let state = if ready {
            ManagedPlanState::Ready
        } else if corrupt {
            ManagedPlanState::Corrupt
        } else if missing.is_empty() {
            ManagedPlanState::Cached
        } else {
            ManagedPlanState::Missing
        };
        Ok(ManagedPlan {
            package_id: id.to_owned(), package_sha256: package_sha256(package)?, state,
            offline, network_bytes: missing.iter().map(|artifact| artifact.length).sum(), installed_bytes,
            missing_artifacts: missing.iter().map(|artifact| artifact.id.clone()).collect(),
            message: if ready { "verified local package is ready" } else if corrupt { "installed package is corrupt; selected repair is required" } else if missing.is_empty() { "verified cached archives can be installed without network" } else if offline { "exact artifacts are missing; offline mode refuses network; install this package online or supply explicit paths" } else { "selected run will acquire only this package's missing exact artifacts" }.to_owned(),
        })
    }

    pub fn acquire(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
        downloader: &dyn ManagedDownloader,
        control: &ManagedControl,
    ) -> crate::Result<InstalledManagedPackage> {
        catalog.validate()?;
        control.check()?;
        if let Ok(installed) = self.inspect(catalog, id) {
            return Ok(installed);
        }
        // Offline refusal occurs before creating an ownership marker or lock.
        let plan = self.plan(catalog, id, control.offline)?;
        if control.offline && plan.network_bytes != 0 {
            return Err(dependency(plan.message));
        }
        let _writer = self.writer()?;
        if let Ok(installed) = self.inspect(catalog, id) {
            return Ok(installed);
        }
        if self.version(id)?.exists() {
            return Err(dependency(
                "managed package exists but fails verification; run selected repair, retaining its old bytes in quarantine",
            ));
        }
        self.install_locked(catalog, id, downloader, control)
    }

    fn install_locked(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
        downloader: &dyn ManagedDownloader,
        control: &ManagedControl,
    ) -> crate::Result<InstalledManagedPackage> {
        let package = catalog.package(id)?;
        let stage = tempfile::Builder::new()
            .prefix("install-")
            .tempdir_in(self.root.join("staging"))
            .map_err(io)?;
        for artifact in &package.artifacts {
            control.check()?;
            let archive = self.archive(artifact);
            safe_path(&archive)?;
            if verify_file(&archive, &artifact.sha256, artifact.length).is_err() {
                if control.offline {
                    return Err(dependency(
                        "exact artifact cache is missing/corrupt; offline acquisition refuses network",
                    ));
                }
                if archive.exists() {
                    self.quarantine(&archive, "archive")?;
                }
                let download_stage = tempfile::Builder::new()
                    .prefix("download-")
                    .tempdir_in(self.root.join("staging"))
                    .map_err(io)?;
                let partial = download_stage.path().join("artifact.partial");
                control.emit(ManagedProgress::DownloadStarted {
                    artifact_id: artifact.id.clone(),
                    bytes: artifact.length,
                });
                downloader.download(artifact, &partial, control)?;
                control.check()?;
                // Transport success is not integrity evidence, including injected transports.
                verify_file(&partial, &artifact.sha256, artifact.length)?;
                fs::rename(&partial, &archive).map_err(io)?;
                sync_directory(&self.root.join("archives"))?;
            }
            control.emit(ManagedProgress::ArchiveVerified {
                artifact_id: artifact.id.clone(),
            });
            extract(artifact, &archive, stage.path(), control)?;
        }
        control.check()?;
        let receipt = ManagedReceipt {
            schema: MANAGED_RECEIPT_SCHEMA.to_owned(),
            catalog_revision: catalog.revision.clone(),
            package_sha256: package_sha256(package)?,
            package: package.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        write_new(
            &stage.path().join("receipt.json"),
            &serde_json::to_vec_pretty(&receipt).map_err(io)?,
        )?;
        validate_membership(stage.path(), package)?;
        sync_tree(stage.path())?;
        control.check()?;
        let destination = self.version(id)?;
        if destination.exists() {
            return Err(dependency(
                "managed immutable install destination already exists",
            ));
        }
        fs::rename(stage.path(), &destination).map_err(io)?;
        sync_directory(&self.root.join("versions"))?;
        let installed = self.inspect(catalog, id)?;
        control.emit(ManagedProgress::Installed {
            package_id: id.to_owned(),
        });
        Ok(installed)
    }

    fn quarantine(&self, path: &Path, prefix: &str) -> crate::Result<()> {
        safe_path(path)?;
        let slot = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(self.root.join("quarantine"))
            .map_err(io)?;
        fs::rename(path, slot.path().join("retained")).map_err(io)?;
        let _retained = slot.keep();
        sync_directory(&self.root.join("quarantine"))
    }

    pub fn repair(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
        downloader: &dyn ManagedDownloader,
        control: &ManagedControl,
    ) -> crate::Result<InstalledManagedPackage> {
        catalog.validate()?;
        catalog.package(id)?;
        control.check()?;
        if let Ok(installed) = self.inspect(catalog, id) {
            return Ok(installed);
        }
        let plan = self.plan(catalog, id, control.offline)?;
        if control.offline && plan.network_bytes != 0 {
            return Err(dependency(plan.message));
        }
        let _writer = self.writer()?;
        let path = self.version(id)?;
        if path.exists() {
            self.require_unreferenced(id)?;
            // A changed catalog cannot overwrite an old package under the same id.
            if let Ok(receipt) = read_json::<ManagedReceipt>(&path.join("receipt.json")) {
                if receipt.package != *catalog.package(id)? {
                    return Err(dependency(
                        "repair refuses to repoint an existing immutable package id",
                    ));
                }
            }
            self.quarantine(&path, "package")?;
        }
        self.install_locked(catalog, id, downloader, control)
    }

    pub fn list(&self) -> crate::Result<Vec<ManagedReceipt>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        self.owned()?;
        let versions = self.root.join("versions");
        safe_path(&versions)?;
        if !versions.exists() {
            return Ok(Vec::new());
        }
        let mut receipts = Vec::new();
        for entry in fs::read_dir(versions).map_err(io)? {
            let entry = entry.map_err(io)?;
            let id = entry
                .file_name()
                .into_string()
                .map_err(|_| dependency("managed version id is not UTF-8"))?;
            receipts.push(self.observed(&id)?.receipt);
        }
        receipts.sort_by(|left, right| left.package.id.cmp(&right.package.id));
        Ok(receipts)
    }

    pub fn references(&self) -> crate::Result<ManagedReferences> {
        if !self.root.exists() {
            return Ok(ManagedReferences::default());
        }
        self.owned()?;
        let path = self.root.join("references.json");
        safe_path(&path)?;
        if !path.exists() {
            return Ok(ManagedReferences::default());
        }
        let references: ManagedReferences = read_json(&path)?;
        if references.active.len() > 200 || references.retained.len() > 50_000 {
            return Err(dependency("managed references exceed bounded inventory"));
        }
        for id in &references.active {
            identifier(id)?;
        }
        for reference in &references.retained {
            identifier(&reference.package_id)?;
            identifier(&reference.run_id)?;
        }
        Ok(references)
    }

    fn write_references(&self, references: &ManagedReferences) -> crate::Result<()> {
        if references.active.len() > 200 || references.retained.len() > 50_000 {
            return Err(dependency("managed references exceed bounded inventory"));
        }
        let bytes = serde_json::to_vec_pretty(references).map_err(io)?;
        if bytes.len() as u64 > MAXIMUM_DOCUMENT_BYTES {
            return Err(dependency("managed references exceed document byte budget"));
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.root).map_err(io)?;
        temporary.write_all(&bytes).map_err(io)?;
        temporary.as_file().sync_all().map_err(io)?;
        safe_path(&self.root.join("references.json"))?;
        temporary
            .persist(self.root.join("references.json"))
            .map_err(io)?;
        sync_directory(&self.root)
    }

    fn require_unreferenced(&self, id: &str) -> crate::Result<()> {
        let references = self.references()?;
        if references.active.iter().any(|active| active == id)
            || references
                .retained
                .iter()
                .any(|retained| retained.package_id == id)
        {
            return Err(dependency(
                "managed version is active or referenced by a retained run; release its references/select another version before mutation",
            ));
        }
        Ok(())
    }

    /// A retained run must take this reference before using managed paths. The
    /// same lock serializes retention, uninstall, repair and activation.
    pub fn retain(&self, id: &str, run_id: &str) -> crate::Result<()> {
        identifier(run_id)?;
        let _writer = self.writer()?;
        self.observed(id)?;
        let mut references = self.references()?;
        let reference = ManagedRetention {
            package_id: id.to_owned(),
            run_id: run_id.to_owned(),
        };
        if !references.retained.contains(&reference) {
            references.retained.push(reference);
        }
        self.write_references(&references)
    }

    /// Explicitly release only a selected run reference; no run directory is deleted.
    pub fn release_retention(&self, id: &str, run_id: &str) -> crate::Result<()> {
        identifier(id)?;
        identifier(run_id)?;
        let _writer = self.writer()?;
        let mut references = self.references()?;
        references
            .retained
            .retain(|reference| reference.package_id != id || reference.run_id != run_id);
        self.write_references(&references)
    }

    pub fn activate(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
    ) -> crate::Result<InstalledManagedPackage> {
        let _writer = self.writer()?;
        let installed = self.inspect(catalog, id)?;
        let provider = installed.receipt.package.provider;
        let mut references = self.references()?;
        let mut retained_active = Vec::new();
        for active in &references.active {
            let previous = self.observed(active)?;
            if previous.receipt.package.provider != provider {
                retained_active.push(active.clone());
            }
        }
        retained_active.push(id.to_owned());
        references.active = retained_active;
        self.write_references(&references)?;
        Ok(installed)
    }

    /// Rollback only activates an already installed verified package; it never downloads.
    pub fn rollback(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
    ) -> crate::Result<InstalledManagedPackage> {
        self.activate(catalog, id)
    }

    /// Explicitly deselect an active version before repairing/removing it.
    /// Retained-run references remain independently authoritative.
    pub fn deactivate(&self, id: &str) -> crate::Result<()> {
        identifier(id)?;
        let _writer = self.writer()?;
        let mut references = self.references()?;
        references.active.retain(|active| active != id);
        self.write_references(&references)
    }

    /// Upgrade is a selected exact catalog install followed by activation. It
    /// preserves the old version and all retained-run references.
    pub fn upgrade(
        &self,
        catalog: &ManagedCatalog,
        id: &str,
        downloader: &dyn ManagedDownloader,
        control: &ManagedControl,
    ) -> crate::Result<InstalledManagedPackage> {
        self.acquire(catalog, id, downloader, control)?;
        self.activate(catalog, id)
    }

    /// Uninstall accepts only a package id. User-supplied paths and arbitrary
    /// recursive deletion are deliberately absent from this API.
    pub fn uninstall(&self, id: &str) -> crate::Result<()> {
        identifier(id)?;
        let _writer = self.writer()?;
        self.require_unreferenced(id)?;
        let installed = self.observed(id)?;
        validate_membership(&installed.root, &installed.receipt.package)?;
        let expected: std::collections::BTreeSet<_> = installed
            .receipt
            .package
            .files()
            .map(|file| PathBuf::from(&file.path))
            .chain(std::iter::once(PathBuf::from("receipt.json")))
            .collect();
        let mut directories = Vec::new();
        for entry in walkdir::WalkDir::new(&installed.root).follow_links(false) {
            let entry = entry.map_err(io)?;
            safe_path(entry.path())?;
            let relative = entry.path().strip_prefix(&installed.root).map_err(io)?;
            if entry.file_type().is_dir() {
                directories.push(entry.path().to_owned());
            } else if !entry.file_type().is_file() || !expected.contains(relative) {
                return Err(dependency(
                    "uninstall refuses unknown or linked files in managed version; reconcile/quarantine them explicitly",
                ));
            }
        }
        // All members are validated before any byte is removed.
        for path in &expected {
            fs::remove_file(installed.root.join(path)).map_err(io)?;
        }
        directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for directory in directories {
            fs::remove_dir(directory).map_err(io)?;
        }
        sync_directory(&self.root.join("versions"))
    }
}

fn validate_membership(root: &Path, package: &ManagedPackage) -> crate::Result<()> {
    let files: std::collections::BTreeSet<_> = package
        .files()
        .map(|file| PathBuf::from(&file.path))
        .chain(std::iter::once(PathBuf::from("receipt.json")))
        .collect();
    let mut directories = std::collections::BTreeSet::from([PathBuf::new()]);
    for file in &files {
        let mut parent = file.parent();
        while let Some(value) = parent {
            directories.insert(value.to_owned());
            parent = value.parent();
        }
    }
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(io)?;
        safe_path(entry.path())?;
        let relative = entry.path().strip_prefix(root).map_err(io)?;
        if entry.file_type().is_dir() {
            if !directories.contains(relative) {
                return Err(dependency(
                    "managed version contains an undeclared directory",
                ));
            }
        } else if !entry.file_type().is_file() || !files.contains(relative) {
            return Err(dependency(
                "managed version contains an undeclared, linked or special file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if entry.file_type().is_file()
                && fs::symlink_metadata(entry.path()).map_err(io)?.nlink() != 1
            {
                return Err(dependency("managed version refuses hard-linked files"));
            }
        }
    }
    Ok(())
}

fn sync_tree(root: &Path) -> crate::Result<()> {
    let mut directories = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(io)?;
        safe_path(entry.path())?;
        if entry.file_type().is_dir() {
            directories.push(entry.path().to_owned());
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        sync_directory(&directory)?;
    }
    Ok(())
}
