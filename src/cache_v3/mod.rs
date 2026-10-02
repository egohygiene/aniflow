//! Opt-in local stage cache. Cache storage never grants acceptance authority.
mod store;

use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::{Error, ErrorCategory, Result};

pub use store::{inspect_cache, invalidate_cache, prune_cache};
pub(crate) use store::{CacheSession, ExclusiveWriter, available_storage, require_safe_path};

pub const CACHE_POLICY_SCHEMA_V1: &str = "aniflow.cache-policy/v1";
pub const CACHE_ENTRY_SCHEMA_V1: &str = "aniflow.cache-entry/v1";
pub const CACHE_INSPECTION_SCHEMA_V1: &str = "aniflow.cache-inspection/v1";
pub const CACHE_OPERATION_SCHEMA_V1: &str = "aniflow.cache-operation/v1";
pub const CACHE_DECISION_SCHEMA_V1: &str = "aniflow.cache-decision/v1";
pub const CACHE_KEY_SEMANTICS_V1: &str = "aniflow.stage-cache-key/v1";

/// Explicit ownership and storage limits. The root itself is an exclusively
/// managed namespace, not a general user directory or provider model cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachePolicy {
    pub schema: String,
    pub root: PathBuf,
    pub owner: String,
    pub maximum_bytes: u64,
    pub maximum_entries: u64,
    pub maximum_entry_bytes: u64,
    pub maximum_entry_files: u64,
    pub maximum_age_seconds: u64,
    pub minimum_free_bytes: u64,
    pub maximum_prune_entries: u64,
}

impl CachePolicy {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, owner: impl Into<String>) -> Self {
        Self {
            schema: CACHE_POLICY_SCHEMA_V1.to_owned(), root: root.into(), owner: owner.into(),
            maximum_bytes: 4 * 1024 * 1024 * 1024, maximum_entries: 1024,
            maximum_entry_bytes: 256 * 1024 * 1024, maximum_entry_files: 100_000,
            maximum_age_seconds: 30 * 24 * 60 * 60, minimum_free_bytes: 64 * 1024 * 1024,
            maximum_prune_entries: 128,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != CACHE_POLICY_SCHEMA_V1 || self.root.as_os_str().is_empty()
            || self.owner.len() > 128 || !self.owner.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
            || !self.owner.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
            || self.maximum_entries == 0 || self.maximum_entries > 4096
            || self.maximum_entry_files == 0 || self.maximum_entry_files > 100_000
            || self.maximum_entry_bytes < 1024 * 1024 || self.maximum_bytes < self.maximum_entry_bytes
            || self.maximum_age_seconds == 0 || self.maximum_age_seconds > 10 * 365 * 24 * 60 * 60
            || self.maximum_prune_entries == 0 || self.maximum_prune_entries > 4096 {
            return Err(cache_error(CacheFailureCode::InvalidPolicy, "invalid cache schema, owner or bounded storage policy"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheFailureCode {
    InvalidPolicy, UnsafePath, OwnershipMismatch, Busy, CorruptEntry, IncompatibleEntry,
    StorageLimit, ConflictingEntry, UnsupportedPlatform, Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheDiagnostic { pub code: CacheFailureCode, pub message: String }

pub(crate) fn cache_error(code: CacheFailureCode, message: impl Into<String>) -> Error {
    Error::from_cache(CacheDiagnostic { code, message: message.into() })
}

pub(crate) fn io_error(error: impl std::fmt::Display) -> Error {
    Error::new(ErrorCategory::Io, format!("cache I/O failed: {error}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheContent {
    pub relative_path: String,
    pub kind: crate::ArtifactKind,
    pub byte_count: u64,
    /// Directories have no content digest; files bind their exact bytes.
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheEntryPayload {
    pub owner: String,
    pub key_sha256: String,
    pub stage_id: String,
    pub origin_plan_sha256: String,
    pub checkpoint: crate::StageCheckpointReference,
    pub result_sha256: String,
    pub created_at: DateTime<Utc>,
    pub contents: Vec<CacheContent>,
}

/// The entry digest seals origin proof and the exact payload inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheEntry { pub schema: String, pub entry_sha256: String, pub payload: CacheEntryPayload }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheEntryState { Available, Expired, Invalid }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheEntrySummary {
    pub key_sha256: String,
    pub state: CacheEntryState,
    pub byte_count: u64,
    pub created_at: Option<DateTime<Utc>>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheInspection {
    pub schema: String, pub root: PathBuf, pub owner: String, pub exists: bool,
    pub writer_present: bool, pub entries: Vec<CacheEntrySummary>,
    pub temporary_entries: u64, pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheOperation {
    pub schema: String, pub operation: String, pub owner: String,
    pub selected_keys: Vec<String>, pub removed_keys: Vec<String>,
    pub reclaimed_bytes: u64, pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheDecisionKind { Miss, Expired, Hit, Published, Existing, Rerun, RunCheckpoint, NotCacheable }

/// A run keeps these receipts independently of ordinary provider progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheDecision {
    pub schema: String, pub stage_id: String, pub key_sha256: Option<String>,
    pub decision: CacheDecisionKind, pub entry_sha256: Option<String>,
}

pub(crate) fn require_digest(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(cache_error(CacheFailureCode::CorruptEntry, "expected a lowercase SHA-256 identity"));
    }
    Ok(())
}
