use std::collections::BTreeSet;
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use super::*;

pub const MAXIMUM_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAXIMUM_INSTALLED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAXIMUM_ARCHIVE_MEMBERS: usize = 30_000;

/// The compiled catalog is part of the source package, never fetched as policy.
pub fn bundled_catalog() -> crate::Result<ManagedCatalog> {
    let catalog: ManagedCatalog =
        serde_json::from_str(include_str!("../../../providers/managed/catalog.v1.json"))
            .map_err(io)?;
    catalog.validate()?;
    Ok(catalog)
}

pub fn package_sha256(package: &ManagedPackage) -> crate::Result<String> {
    let bytes = serde_json::to_vec(package).map_err(io)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(crate) fn identifier(value: &str) -> crate::Result<()> {
    if value.is_empty()
        || value.len() > 180
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err(dependency(
            "managed identifier must be 1..=180 safe ASCII characters",
        ));
    }
    Ok(())
}

pub(crate) fn relative(value: &str) -> crate::Result<()> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains(['\\', ':', '\0'])
        || !value.is_ascii()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.split('/').any(|part| {
            part.is_empty() || part == "." || part == ".." || part.ends_with([' ', '.'])
        })
        || !Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(dependency(format!(
            "unsafe managed relative path: {value:?}"
        )));
    }
    for part in value.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
            || part
                .bytes()
                .any(|byte| byte < 32 || b"<>\"|?*".contains(&byte))
        {
            return Err(dependency(
                "managed paths may not use portable reserved names or control characters",
            ));
        }
    }
    Ok(())
}

pub(crate) fn digest(value: &str) -> crate::Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(dependency(
            "managed SHA-256 must be an actual lowercase 64-digit digest",
        ));
    }
    Ok(())
}

/// Acquisition intentionally supports only public upstream release infrastructure.
/// There is no auth header, ambient credential forwarding or arbitrary URL fetch.
pub(crate) fn https_url(value: &str) -> crate::Result<()> {
    let rest = value
        .strip_prefix("https://")
        .ok_or_else(|| dependency("managed artifacts require HTTPS"))?;
    let host = rest.split('/').next().unwrap_or("");
    if ![
        "github.com",
        "raw.githubusercontent.com",
        "objects.githubusercontent.com",
        "release-assets.githubusercontent.com",
    ]
    .contains(&host)
        || value.contains(['\r', '\n', '\0', '#'])
        || rest.len() == host.len()
    {
        return Err(dependency(
            "managed download URL must use an allowed public GitHub upstream host",
        ));
    }
    Ok(())
}

impl ManagedCatalog {
    pub fn validate(&self) -> crate::Result<()> {
        if self.schema != MANAGED_CATALOG_SCHEMA
            || self.revision.is_empty()
            || self.revision.len() > 200
            || self.packages.len() > 200
        {
            return Err(dependency("unsupported or unbounded managed catalog"));
        }
        let mut ids = BTreeSet::new();
        for package in &self.packages {
            package.validate()?;
            if !ids.insert(package.id.to_ascii_lowercase()) {
                return Err(dependency("duplicate managed package id"));
            }
        }
        Ok(())
    }
}

impl ManagedPackage {
    pub fn validate(&self) -> crate::Result<()> {
        identifier(&self.id)?;
        relative(&self.executable)?;
        if self.version.is_empty()
            || self.source_revision.is_empty()
            || self.model_id.is_empty()
            || !["macos", "linux", "windows"].contains(&self.platform.os.as_str())
            || !["x86_64", "aarch64"].contains(&self.platform.arch.as_str())
            || self.backend != "vulkan"
            || !(1..=8).contains(&self.model_scale)
            || self.artifacts.is_empty()
            || self.artifacts.len() > 32
            || self.licenses.is_empty()
        {
            return Err(dependency(
                "managed package lacks a bounded provider/platform/model/license declaration",
            ));
        }
        let mut paths = BTreeSet::new();
        let mut artifact_ids = BTreeSet::new();
        let mut installed_bytes = 0_u64;
        let mut file_count = 0_usize;
        for artifact in &self.artifacts {
            identifier(&artifact.id)?;
            if !artifact_ids.insert(&artifact.id) {
                return Err(dependency("duplicate managed artifact id"));
            }
            https_url(&artifact.url)?;
            digest(&artifact.sha256)?;
            if artifact.length == 0
                || artifact.length > MAXIMUM_ARTIFACT_BYTES
                || artifact.files.is_empty()
                || artifact.files.len() > MAXIMUM_ARCHIVE_MEMBERS
                || (artifact.format == ManagedArtifactFormat::Raw && artifact.files.len() != 1)
            {
                return Err(dependency("managed artifact exceeds bounded inventory"));
            }
            let mut members = BTreeSet::new();
            for file in &artifact.files {
                file_count += 1;
                if file_count > MAXIMUM_ARCHIVE_MEMBERS {
                    return Err(dependency(
                        "managed package exceeds total selected member budget",
                    ));
                }
                relative(&file.path)?;
                if file.path.eq_ignore_ascii_case("receipt.json") {
                    return Err(dependency("managed file uses reserved receipt path"));
                }
                if artifact.format == ManagedArtifactFormat::Zip {
                    relative(&file.member)?;
                } else if !file.member.is_empty() {
                    return Err(dependency("raw artifact must have an empty archive member"));
                }
                digest(&file.sha256)?;
                if file.length == 0
                    || file.length > MAXIMUM_ARTIFACT_BYTES
                    || !paths.insert(file.path.to_ascii_lowercase())
                    || !members.insert(file.member.to_ascii_lowercase())
                {
                    return Err(dependency("duplicate, empty or oversized managed file"));
                }
                installed_bytes = installed_bytes
                    .checked_add(file.length)
                    .ok_or_else(|| dependency("managed package byte budget overflow"))?;
            }
        }
        if installed_bytes > MAXIMUM_INSTALLED_BYTES
            || !self.files().any(|file| {
                file.path == self.executable
                    && file.role == ManagedFileRole::Executable
                    && file.executable
            })
            || !self
                .files()
                .any(|file| file.role == ManagedFileRole::ModelParameter)
            || !self
                .files()
                .any(|file| file.role == ManagedFileRole::ModelWeights)
        {
            return Err(dependency(
                "managed package lacks complete executable/model closure or exceeds byte budget",
            ));
        }
        // A file may not also be another file's parent, including case variants.
        for path in &paths {
            let mut parent = Path::new(path).parent();
            while let Some(value) = parent {
                if paths.contains(&value.to_string_lossy().into_owned()) {
                    return Err(dependency("managed file/directory path collision"));
                }
                parent = value.parent();
            }
        }
        for license in &self.licenses {
            relative(&license.path)?;
            https_url(&license.source_url)?;
            if license.spdx.is_empty()
                || !self.files().any(|file| {
                    file.path == license.path
                        && matches!(
                            file.role,
                            ManagedFileRole::License | ManagedFileRole::Notice
                        )
                })
            {
                return Err(dependency(
                    "managed license has no retained license/notice file",
                ));
            }
        }
        Ok(())
    }
}
