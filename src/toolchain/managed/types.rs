use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub const MANAGED_CATALOG_SCHEMA: &str = "aniflow.managed-catalog/v1";
pub const MANAGED_RECEIPT_SCHEMA: &str = "aniflow.managed-receipt/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedProvider {
    Upscayl,
    Rife,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedPlatform {
    pub os: String,
    pub arch: String,
}
impl ManagedPlatform {
    #[must_use]
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedArtifactFormat {
    Zip,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedFileRole {
    Executable,
    ModelParameter,
    ModelWeights,
    Runtime,
    License,
    Notice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedFile {
    pub member: String,
    pub path: String,
    pub sha256: String,
    pub length: u64,
    pub role: ManagedFileRole,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedArtifact {
    pub id: String,
    pub url: String,
    pub sha256: String,
    pub length: u64,
    pub format: ManagedArtifactFormat,
    pub files: Vec<ManagedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedLicense {
    pub spdx: String,
    pub path: String,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedPackage {
    pub id: String,
    pub provider: ManagedProvider,
    pub version: String,
    pub platform: ManagedPlatform,
    pub backend: String,
    pub source_revision: String,
    pub model_id: String,
    pub model_scale: u8,
    pub executable: String,
    pub artifacts: Vec<ManagedArtifact>,
    pub licenses: Vec<ManagedLicense>,
    pub notes: Vec<String>,
}
impl ManagedPackage {
    pub fn files(&self) -> impl Iterator<Item = &ManagedFile> {
        self.artifacts
            .iter()
            .flat_map(|artifact| artifact.files.iter())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedCatalog {
    pub schema: String,
    pub revision: String,
    pub packages: Vec<ManagedPackage>,
}
impl ManagedCatalog {
    pub fn package(&self, id: &str) -> crate::Result<&ManagedPackage> {
        self.packages
            .iter()
            .find(|package| package.id == id)
            .ok_or_else(|| {
                super::dependency(format!(
                    "managed package {id:?} is absent from catalog {}",
                    self.revision
                ))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedIdentity {
    pub package_id: String,
    pub package_sha256: String,
    pub catalog_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedReceipt {
    pub schema: String,
    pub catalog_revision: String,
    pub package_sha256: String,
    pub package: ManagedPackage,
    pub installed_at: String,
}
impl ManagedReceipt {
    #[must_use]
    pub fn identity(&self) -> ManagedIdentity {
        ManagedIdentity {
            package_id: self.package.id.clone(),
            package_sha256: self.package_sha256.clone(),
            catalog_revision: self.catalog_revision.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledManagedPackage {
    pub root: PathBuf,
    pub receipt: ManagedReceipt,
}
impl InstalledManagedPackage {
    #[must_use]
    pub fn executable(&self) -> PathBuf {
        self.root.join(&self.receipt.package.executable)
    }
    pub fn files(&self, role: ManagedFileRole) -> impl Iterator<Item = PathBuf> + '_ {
        self.receipt
            .package
            .files()
            .filter(move |file| file.role == role)
            .map(|file| self.root.join(&file.path))
    }
    #[must_use]
    pub fn path(&self, relative: &Path) -> Option<PathBuf> {
        self.receipt
            .package
            .files()
            .any(|file| Path::new(&file.path) == relative)
            .then(|| self.root.join(relative))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedPlanState {
    Ready,
    Cached,
    Missing,
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedPlan {
    pub package_id: String,
    pub package_sha256: String,
    pub state: ManagedPlanState,
    pub offline: bool,
    pub network_bytes: u64,
    pub installed_bytes: u64,
    pub missing_artifacts: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ManagedProgress {
    DownloadStarted {
        artifact_id: String,
        bytes: u64,
    },
    DownloadProgress {
        artifact_id: String,
        bytes: u64,
        total: u64,
    },
    ArchiveVerified {
        artifact_id: String,
    },
    FileInstalled {
        path: String,
        bytes: u64,
    },
    Installed {
        package_id: String,
    },
}

#[derive(Clone)]
pub struct ManagedControl {
    pub offline: bool,
    pub cancellation: crate::CancellationToken,
    pub timeout_seconds: u64,
    pub observer: Option<Arc<dyn Fn(ManagedProgress) + Send + Sync>>,
}
impl Default for ManagedControl {
    fn default() -> Self {
        Self {
            offline: false,
            cancellation: crate::CancellationToken::default(),
            timeout_seconds: 300,
            observer: None,
        }
    }
}
impl ManagedControl {
    pub(crate) fn check(&self) -> crate::Result<()> {
        if self.cancellation.is_cancelled() {
            return Err(super::dependency(
                "managed acquisition cancelled; no partial package is ready",
            ));
        }
        if !(1..=3600).contains(&self.timeout_seconds) {
            return Err(super::dependency(
                "managed download timeout must be 1..=3600 seconds",
            ));
        }
        Ok(())
    }
    pub(crate) fn emit(&self, progress: ManagedProgress) {
        if let Some(observer) = &self.observer {
            observer(progress);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRetention {
    pub package_id: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedReferences {
    pub active: Vec<String>,
    pub retained: Vec<ManagedRetention>,
}
