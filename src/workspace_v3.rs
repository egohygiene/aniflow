use std::fs;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use tempfile::NamedTempFile;

use crate::error::{Error, ErrorCategory, Result};

/// Isolated filesystem layout for one Pipeline v3 run.
///
/// Opening an existing workspace is deliberately read-only. Callers must invoke
/// an explicit publishing method before this type mutates an existing run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineV3Workspace {
    root: PathBuf,
}

impl PipelineV3Workspace {
    /// Create a uniquely named workspace beneath `parent`.
    pub fn create(parent: Option<&Path>, pipeline_name: &str) -> Result<Self> {
        let parent = parent
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".aniflow/runs"));
        fs::create_dir_all(&parent).map_err(|error| {
            io_error(format!(
                "failed to create Pipeline v3 run parent {}: {error}",
                parent.display()
            ))
        })?;

        let safe_name = sanitize_name(pipeline_name);
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%6fZ");
        for suffix in 0_u16..=999 {
            let leaf = if suffix == 0 {
                format!("{timestamp}-{safe_name}")
            } else {
                format!("{timestamp}-{safe_name}-{suffix:03}")
            };
            let root = parent.join(leaf);
            match Self::create_at(&root) {
                Ok(workspace) => return Ok(workspace),
                Err(error) if error.category() == ErrorCategory::State && root.exists() => {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }

        Err(Error::new(
            ErrorCategory::State,
            format!(
                "could not allocate a unique Pipeline v3 run beneath {}",
                parent.display()
            ),
        ))
    }

    /// Exclusively create a workspace at an exact path.
    pub fn create_at(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let parent = root
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| {
            io_error(format!(
                "failed to create Pipeline v3 workspace parent {}: {error}",
                parent.display()
            ))
        })?;
        fs::create_dir(root).map_err(|error| {
            let category = if error.kind() == std::io::ErrorKind::AlreadyExists {
                ErrorCategory::State
            } else {
                ErrorCategory::Io
            };
            Error::new(
                category,
                format!(
                    "failed to exclusively create Pipeline v3 workspace {}: {error}",
                    root.display()
                ),
            )
        })?;

        let workspace = Self {
            root: root.to_path_buf(),
        };
        if let Err(error) = workspace
            .create_layout()
            .and_then(|()| workspace.sync_layout())
            .and_then(|()| sync_directory(parent))
        {
            // The root was created exclusively by this call, so cleanup cannot
            // remove a pre-existing user workspace.
            let _ = fs::remove_dir_all(&workspace.root);
            let _ = sync_directory(parent);
            return Err(error);
        }
        Ok(workspace)
    }

    /// Open and validate an existing workspace without creating or repairing it.
    pub fn open_read_only(root: impl AsRef<Path>) -> Result<Self> {
        let workspace = Self {
            root: root.as_ref().to_path_buf(),
        };
        require_real_directory(&workspace.root, "Pipeline v3 workspace")?;
        for directory in workspace.required_directories() {
            require_real_directory(&directory, "Pipeline v3 workspace directory")?;
        }
        Ok(workspace)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn run_id(&self) -> String {
        self.root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("aniflow-v3-run")
            .to_owned()
    }

    #[must_use]
    pub fn plan_directory(&self) -> PathBuf {
        self.root.join("plan")
    }

    #[must_use]
    pub fn plan(&self) -> PathBuf {
        self.plan_directory().join("plan.json")
    }

    #[must_use]
    pub fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    #[must_use]
    pub fn manifests(&self) -> PathBuf {
        self.state().join("manifests")
    }

    #[must_use]
    pub fn manifest_revision(&self, revision: u64) -> PathBuf {
        self.manifests().join(format!("{revision:020}.json"))
    }

    #[must_use]
    pub fn checkpoints(&self) -> PathBuf {
        self.state().join("checkpoints")
    }

    #[must_use]
    pub fn stage_checkpoint(&self, stage_id: &str, checkpoint_sha256: &str) -> PathBuf {
        self.checkpoints().join(format!(
            "{}-{}.json",
            path_component(stage_id),
            path_component(checkpoint_sha256)
        ))
    }

    #[must_use]
    pub fn providers(&self) -> PathBuf {
        self.root.join("providers")
    }

    #[must_use]
    pub fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    #[must_use]
    pub fn stage_work(&self, stage_id: &str) -> PathBuf {
        self.work()
            .join(format!("stage-{}", path_component(stage_id)))
    }

    #[must_use]
    pub fn artifacts(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    /// Atomically publish exact plan bytes without replacing prior authority.
    pub fn publish_plan_bytes(&self, bytes: &[u8]) -> Result<PathBuf> {
        let path = self.plan();
        write_bytes_new(&path, bytes)?;
        Ok(path)
    }

    /// Atomically publish a JSON value without replacing an existing file.
    pub fn publish_json_new<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        require_target_beneath(&self.root, path)?;
        let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| {
            Error::new(
                ErrorCategory::Internal,
                format!("failed to encode {}: {error}", path.display()),
            )
        })?;
        bytes.push(b'\n');
        write_bytes_new(path, &bytes)
    }

    fn create_layout(&self) -> Result<()> {
        for directory in self.required_directories() {
            fs::create_dir(&directory).map_err(|error| {
                io_error(format!(
                    "failed to create Pipeline v3 workspace directory {}: {error}",
                    directory.display()
                ))
            })?;
        }
        Ok(())
    }

    fn sync_layout(&self) -> Result<()> {
        for directory in self.required_directories().into_iter().rev() {
            sync_directory(&directory)?;
        }
        sync_directory(&self.root)
    }

    fn required_directories(&self) -> [PathBuf; 7] {
        [
            self.plan_directory(),
            self.state(),
            self.manifests(),
            self.checkpoints(),
            self.providers(),
            self.work(),
            self.artifacts(),
        ]
    }
}

fn require_target_beneath(root: &Path, target: &Path) -> Result<()> {
    let parent = target.parent().ok_or_else(|| {
        Error::new(
            ErrorCategory::State,
            format!(
                "Pipeline v3 state target has no parent: {}",
                target.display()
            ),
        )
    })?;
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        io_error(format!(
            "failed to resolve Pipeline v3 workspace {}: {error}",
            root.display()
        ))
    })?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        io_error(format!(
            "failed to resolve Pipeline v3 state parent {}: {error}",
            parent.display()
        ))
    })?;
    if target == root || !canonical_parent.starts_with(&canonical_root) {
        return Err(Error::new(
            ErrorCategory::State,
            format!(
                "Pipeline v3 state target must remain beneath {}: {}",
                root.display(),
                target.display()
            ),
        ));
    }
    Ok(())
}

fn write_bytes_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        Error::new(
            ErrorCategory::State,
            format!("Pipeline v3 state path has no parent: {}", path.display()),
        )
    })?;
    require_real_directory(parent, "Pipeline v3 state parent")?;
    reject_existing_target(path)?;

    let mut temporary = NamedTempFile::new_in(parent).map_err(|error| {
        io_error(format!(
            "failed to create temporary Pipeline v3 state in {}: {error}",
            parent.display()
        ))
    })?;
    temporary.as_file_mut().write_all(bytes).map_err(|error| {
        io_error(format!(
            "failed to write temporary Pipeline v3 state for {}: {error}",
            path.display()
        ))
    })?;
    temporary.as_file_mut().sync_all().map_err(|error| {
        io_error(format!(
            "failed to sync temporary Pipeline v3 state for {}: {error}",
            path.display()
        ))
    })?;
    temporary.persist_noclobber(path).map_err(|error| {
        let category = if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            ErrorCategory::State
        } else {
            ErrorCategory::Io
        };
        Error::new(
            category,
            format!(
                "failed to publish immutable Pipeline v3 state {}: {}",
                path.display(),
                error.error
            ),
        )
    })?;
    sync_directory(parent)
}

fn reject_existing_target(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(Error::new(
            ErrorCategory::State,
            format!(
                "refusing to replace immutable Pipeline v3 state {}",
                path.display()
            ),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(format!(
            "failed to inspect Pipeline v3 state target {}: {error}",
            path.display()
        ))),
    }
}

fn require_real_directory(path: &Path, kind: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        Error::new(
            ErrorCategory::State,
            format!("{kind} is unavailable at {}: {error}", path.display()),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::new(
            ErrorCategory::State,
            format!("{kind} must be a real directory: {}", path.display()),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            io_error(format!(
                "failed to sync Pipeline v3 state directory {}: {error}",
                path.display()
            ))
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

fn sanitize_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-').to_lowercase();
    if sanitized.is_empty() {
        "pipeline-v3".to_owned()
    } else {
        sanitized
    }
}

fn path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() || matches!(sanitized.as_str(), "." | "..") {
        "invalid".to_owned()
    } else {
        sanitized
    }
}

fn io_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Io, message)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::PipelineV3Workspace;

    #[test]
    fn creates_the_complete_layout_exclusively() {
        let temporary = tempdir().expect("temporary directory should be created");
        let root = temporary.path().join("run");
        let workspace =
            PipelineV3Workspace::create_at(&root).expect("Pipeline v3 workspace should be created");

        assert_eq!(workspace.root(), root);
        for path in [
            workspace.plan_directory(),
            workspace.state(),
            workspace.manifests(),
            workspace.checkpoints(),
            workspace.providers(),
            workspace.work(),
            workspace.artifacts(),
        ] {
            assert!(
                path.is_dir(),
                "missing workspace directory {}",
                path.display()
            );
        }

        let error = PipelineV3Workspace::create_at(&root)
            .expect_err("an existing workspace must never be reused by create");
        assert_eq!(error.category(), crate::ErrorCategory::State);
    }

    #[test]
    fn read_only_open_never_repairs_a_missing_directory() {
        let temporary = tempdir().expect("temporary directory should be created");
        let root = temporary.path().join("run");
        let workspace =
            PipelineV3Workspace::create_at(&root).expect("Pipeline v3 workspace should be created");
        fs::remove_dir(workspace.providers()).expect("fixture directory should be removed");

        PipelineV3Workspace::open_read_only(&root)
            .expect_err("incomplete workspace must be rejected");
        assert!(!workspace.providers().exists());
    }

    #[test]
    fn immutable_publication_refuses_replacement() {
        let temporary = tempdir().expect("temporary directory should be created");
        let workspace = PipelineV3Workspace::create_at(temporary.path().join("run"))
            .expect("Pipeline v3 workspace should be created");
        workspace
            .publish_plan_bytes(b"first")
            .expect("first plan should publish");

        workspace
            .publish_plan_bytes(b"second")
            .expect_err("published plan must be immutable");
        assert_eq!(
            fs::read(workspace.plan()).expect("plan should remain readable"),
            b"first"
        );
    }

    #[test]
    fn json_publication_is_confined_to_the_workspace() {
        let temporary = tempdir().expect("temporary directory should be created");
        let workspace = PipelineV3Workspace::create_at(temporary.path().join("run"))
            .expect("Pipeline v3 workspace should be created");
        let escaped = temporary.path().join("escaped.json");

        workspace
            .publish_json_new(&escaped, &serde_json::json!({ "escaped": true }))
            .expect_err("publication must not escape the workspace");
        assert!(!escaped.exists());
    }
}
