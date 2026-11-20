//! Exact managed package and runtime-file identity at the provider boundary.
//!
//! Downloads and package selection belong to the managed store. These pins bind
//! the selected receipt to ordinary provider configuration, then copy only its
//! declared runtime files beside a private executable. System driver libraries
//! remain host observations; an empty closure never claims native qualification.
use crate::frame_io::{copy_pinned, identity};
use crate::provider::require_sha256;
use crate::{CancellationToken, Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

const MAX_FILE: u64 = 134_217_728;
const MAX_TOTAL: u64 = 1_073_741_824;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRuntimeFile {
    pub path: PathBuf,
    /// Confined path relative to the staged executable's parent directory.
    pub relative_path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRuntimePin {
    pub package_id: String,
    pub package_sha256: String,
    pub catalog_revision: String,
    pub runtime_files: Vec<ManagedRuntimeFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub installation_roots: Vec<PathBuf>,
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Dependency, message)
}
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        return Err(Error::new(
            ErrorCategory::Execution,
            "managed runtime staging cancelled",
        ));
    }
    Ok(())
}
fn portable(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && !matches!(value, "." | "..")
}

impl ManagedRuntimePin {
    pub fn validate(&self) -> Result<()> {
        if !portable(&self.package_id)
            || !portable(&self.catalog_revision)
            || self.runtime_files.len() > 64
        {
            return Err(invalid(
                "managed runtime identity or file count exceeds its bound",
            ));
        }
        require_sha256(&self.package_sha256, "managed package identity")?;
        if self.installation_roots.len() > 8 {
            return Err(invalid(
                "managed runtime protects at most eight installation roots",
            ));
        }
        let mut roots = BTreeSet::new();
        for root in &self.installation_roots {
            if !root.is_absolute()
                || root.to_str().is_none_or(|s| {
                    s.chars().any(char::is_control) || s.split('/').any(|p| matches!(p, "." | ".."))
                })
                || root
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
                || !roots.insert(root)
            {
                return Err(invalid(
                    "managed installation roots require distinct normalized absolute paths",
                ));
            }
        }
        let mut paths = BTreeSet::new();
        let mut folded_paths = BTreeSet::new();
        let mut folded_directories = std::collections::BTreeMap::new();
        let mut total = 0_u64;
        for file in &self.runtime_files {
            require_sha256(&file.sha256, "managed runtime file identity")?;
            if !file.path.is_absolute()
                || file
                    .path
                    .to_str()
                    .is_none_or(|s| s.chars().any(char::is_control))
                || file
                    .path
                    .components()
                    .any(|p| matches!(p, Component::ParentDir | Component::CurDir))
                || file.relative_path.is_absolute()
                || file.relative_path.as_os_str().is_empty()
                || file.relative_path.components().count() > 8
                || file
                    .relative_path
                    .components()
                    .any(|p| !matches!(p, Component::Normal(n) if n.to_str().is_some_and(portable)))
                || !(1..=MAX_FILE).contains(&file.byte_count)
                || !paths.insert(file.relative_path.clone())
            {
                return Err(invalid(
                    "managed runtime file requires distinct confined portable paths and finite bytes",
                ));
            }
            // Product scratch and model namespaces cannot be replaced by a package.
            let first = file
                .relative_path
                .components()
                .next()
                .and_then(|p| p.as_os_str().to_str())
                .unwrap_or("");
            let folded = file
                .relative_path
                .to_str()
                .ok_or_else(|| invalid("runtime path must be UTF-8"))?
                .to_ascii_lowercase();
            if !folded_paths.insert(folded) {
                return Err(invalid(
                    "managed runtime paths collide on a case-insensitive filesystem",
                ));
            }
            let mut ancestor = file.relative_path.parent();
            while let Some(path) = ancestor.filter(|p| !p.as_os_str().is_empty()) {
                let key = path
                    .to_str()
                    .ok_or_else(|| invalid("runtime directory must be UTF-8"))?
                    .to_ascii_lowercase();
                if folded_directories
                    .insert(key, path.to_path_buf())
                    .is_some_and(|prior| prior != path)
                {
                    return Err(invalid(
                        "managed runtime directories differ only by ASCII case",
                    ));
                }
                ancestor = path.parent();
            }
            if [
                "processor",
                "ffmpeg",
                "rife-ncnn-vulkan",
                "models",
                "rife-v4.6",
                "input",
                "output",
                "input.png",
                "output.png",
                "failure.json",
            ]
            .contains(&first.to_ascii_lowercase().as_str())
            {
                return Err(invalid(
                    "managed runtime file overlaps a reserved provider artifact",
                ));
            }
            total = total
                .checked_add(file.byte_count)
                .filter(|n| *n <= MAX_TOTAL)
                .ok_or_else(|| invalid("managed runtime files exceed 1 GiB"))?;
        }
        if folded_paths
            .iter()
            .any(|path| folded_directories.contains_key(path))
        {
            return Err(invalid(
                "managed runtime file collides with a directory on a case-insensitive filesystem",
            ));
        }
        for path in &paths {
            if paths
                .iter()
                .any(|other| path != other && path.starts_with(other))
            {
                return Err(invalid(
                    "managed runtime file collides with a runtime directory",
                ));
            }
        }
        Ok(())
    }

    pub fn observed_bytes(&self) -> Result<u64> {
        self.validate()?;
        for root in &self.installation_roots {
            crate::frame_io::ensure_canonical_directory(root)?;
        }
        let mut total = 0_u64;
        for file in &self.runtime_files {
            if identity(&file.path, MAX_FILE)? != (file.sha256.clone(), file.byte_count) {
                return Err(invalid(
                    "managed runtime original differs from its package pin",
                ));
            }
            total = total
                .checked_add(file.byte_count)
                .ok_or_else(|| invalid("runtime byte sum overflow"))?;
        }
        Ok(total)
    }

    /// Generated artifacts must stay outside immutable managed namespaces. An
    /// ancestor used only as the parent of a fresh child workspace is allowed.
    pub fn require_output_disjoint(&self, output: &Path) -> Result<()> {
        self.validate()?;
        if !output.is_absolute()
            || output.to_str().is_none_or(|s| {
                s.chars().any(char::is_control) || s.split('/').any(|p| matches!(p, "." | ".."))
            })
        {
            return Err(invalid(
                "managed output guard requires a normalized absolute path",
            ));
        }
        if self
            .installation_roots
            .iter()
            .any(|root| output.starts_with(root))
            || self
                .runtime_files
                .iter()
                .any(|file| output == file.path || output.starts_with(&file.path))
        {
            return Err(invalid(
                "generated output overlaps an immutable managed installation",
            ));
        }
        Ok(())
    }

    pub fn stage(&self, executable_parent: &Path, token: &CancellationToken) -> Result<()> {
        cancelled(token)?;
        self.observed_bytes()?;
        crate::frame_io::ensure_canonical_directory(executable_parent)?;
        for file in &self.runtime_files {
            cancelled(token)?;
            let destination = executable_parent.join(&file.relative_path);
            if let Some(parent) = file.relative_path.parent() {
                let mut current = executable_parent.to_path_buf();
                for component in parent.components() {
                    current.push(component);
                    match std::fs::create_dir(&current) {
                        Ok(()) => (),
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            crate::frame_io::ensure_canonical_directory(&current)?;
                        }
                        Err(_) => {
                            return Err(invalid(
                                "cannot create confined runtime library directory",
                            ));
                        }
                    }
                }
            }
            copy_pinned(
                &file.path,
                &destination,
                &(file.sha256.clone(), file.byte_count),
                MAX_FILE,
                token,
            )?;
        }
        self.verify_staged(executable_parent)
    }

    pub fn verify_staged(&self, executable_parent: &Path) -> Result<()> {
        self.observed_bytes()?;
        for file in &self.runtime_files {
            if identity(&executable_parent.join(&file.relative_path), MAX_FILE)?
                != (file.sha256.clone(), file.byte_count)
            {
                return Err(invalid(
                    "private managed runtime file changed during execution",
                ));
            }
        }
        Ok(())
    }

    /// Verify exact provider-root membership while the caller separately checks
    /// the contents of its own model/input/output directories.
    pub fn verify_membership(
        &self,
        parent: &Path,
        base_files: &[&str],
        base_directories: &[&str],
    ) -> Result<()> {
        self.validate()?;
        let mut files: BTreeSet<PathBuf> = base_files.iter().map(PathBuf::from).collect();
        let base_dirs: BTreeSet<PathBuf> = base_directories.iter().map(PathBuf::from).collect();
        let mut directories = base_dirs.clone();
        for file in &self.runtime_files {
            files.insert(file.relative_path.clone());
            let mut ancestor = file.relative_path.parent();
            while let Some(path) = ancestor.filter(|p| !p.as_os_str().is_empty()) {
                directories.insert(path.to_path_buf());
                ancestor = path.parent();
            }
        }
        let mut observed_files = BTreeSet::new();
        let mut observed_directories = BTreeSet::new();
        let walk = walkdir::WalkDir::new(parent)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                entry
                    .path()
                    .strip_prefix(parent)
                    .map_or(true, |p| !base_dirs.contains(p))
            });
        // filter_entry intentionally excludes base directories; inspect them
        // directly below so their exact child inventories stay with the owner.
        for entry in walk {
            let entry =
                entry.map_err(|_| invalid("cannot inspect managed private runtime membership"))?;
            let relative = entry
                .path()
                .strip_prefix(parent)
                .map_err(|_| invalid("private runtime escaped its parent"))?
                .to_path_buf();
            if entry.file_type().is_file() && files.contains(&relative) {
                observed_files.insert(relative);
            } else if entry.file_type().is_dir() && directories.contains(&relative) {
                observed_directories.insert(relative);
            } else {
                return Err(invalid(
                    "native processor created an undeclared private runtime artifact",
                ));
            }
        }
        for path in &base_dirs {
            crate::frame_io::ensure_canonical_directory(&parent.join(path))?;
            observed_directories.insert(path.clone());
        }
        if observed_files != files || observed_directories != directories {
            return Err(invalid("private managed runtime membership is incomplete"));
        }
        self.verify_staged(parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn pin(directory: &Path) -> ManagedRuntimePin {
        let source = directory.join("original.dylib");
        std::fs::write(&source, b"fixture runtime library, never executed").unwrap();
        ManagedRuntimePin {
            package_id: "fixture-runtime".into(),
            package_sha256: "a".repeat(64),
            catalog_revision: "fixture-v1".into(),
            installation_roots: Vec::new(),
            runtime_files: vec![ManagedRuntimeFile {
                path: source,
                relative_path: "lib/libfixture.dylib".into(),
                sha256: format!(
                    "{:x}",
                    Sha256::digest(b"fixture runtime library, never executed")
                ),
                byte_count: 39,
            }],
        }
    }
    #[test]
    fn exact_runtime_staging_membership_and_both_original_and_private_drift() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut pin = pin(&root);
        // Keep the fixture's byte declaration independent of its digest.
        pin.runtime_files[0].byte_count =
            std::fs::metadata(&pin.runtime_files[0].path).unwrap().len();
        let staged = root.join("staged");
        std::fs::create_dir(&staged).unwrap();
        pin.stage(&staged, &CancellationToken::default()).unwrap();
        pin.verify_membership(&staged, &[], &[]).unwrap();
        std::fs::write(staged.join("undeclared.txt"), b"extra").unwrap();
        assert!(pin.verify_membership(&staged, &[], &[]).is_err());
        std::fs::remove_file(staged.join("undeclared.txt")).unwrap();
        std::fs::write(
            staged.join("lib/libfixture.dylib"),
            b"changed private bytes",
        )
        .unwrap();
        assert!(pin.verify_staged(&staged).is_err());
        std::fs::write(&pin.runtime_files[0].path, b"changed original bytes").unwrap();
        assert!(pin.observed_bytes().is_err());
    }
    #[test]
    fn runtime_paths_lengths_collisions_and_cancel_refuse() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let pin = pin(&root);
        for path in [
            "../lib.dylib",
            "/absolute.dylib",
            "lib/../../escape",
            "models/tool.dylib",
            "MODELS/tool.dylib",
            "FFMPEG",
            "processor",
            "lib/bad name.so",
        ] {
            let mut changed = pin.clone();
            changed.runtime_files[0].relative_path = path.into();
            assert!(changed.validate().is_err(), "{path}");
        }
        let mut duplicate = pin.clone();
        duplicate
            .runtime_files
            .push(duplicate.runtime_files[0].clone());
        assert!(duplicate.validate().is_err());
        let mut protected = pin.clone();
        protected.installation_roots = vec![root.join("installed")];
        assert!(
            protected
                .require_output_disjoint(&root.join("installed/run"))
                .is_err()
        );
        assert!(
            protected
                .require_output_disjoint(&root.join("installed"))
                .is_err()
        );
        protected.require_output_disjoint(&root).unwrap();
        assert!(
            protected.observed_bytes().is_err(),
            "missing installation root cannot be observed"
        );
        let mut case_collision = pin.clone();
        let mut alternate = case_collision.runtime_files[0].clone();
        alternate.relative_path = "LIB/libfixture.dylib".into();
        case_collision.runtime_files.push(alternate);
        assert!(case_collision.validate().is_err());
        let mut directory_collision = pin.clone();
        let mut alternate = directory_collision.runtime_files[0].clone();
        alternate.relative_path = "LIB/other.dylib".into();
        directory_collision.runtime_files.push(alternate);
        assert!(directory_collision.validate().is_err());
        let mut parent = pin.clone();
        let mut child = parent.runtime_files[0].clone();
        child.relative_path = "lib/libfixture.dylib/child".into();
        parent.runtime_files.push(child);
        assert!(parent.validate().is_err());
        let token = CancellationToken::default();
        token.cancel();
        assert!(pin.stage(&root, &token).is_err());
        let mut length = pin.clone();
        length.runtime_files[0].byte_count = 0;
        assert!(length.validate().is_err());
    }
}
