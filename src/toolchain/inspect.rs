use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
#[cfg(unix)]
use std::fs::OpenOptions;
use std::io::Read;

use semver::{Version, VersionReq};
use sha2::{Digest, Sha256};

use super::types::*;
use crate::provider::canonical_sha256;
use crate::{Error, ErrorCategory, Result};

const MAXIMUM_TOTAL_HASH_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn invalid(detail: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, detail)
}

struct FileObservation {
    status: ToolchainStatus,
    digest: Option<String>,
    detail: String,
}

#[cfg(unix)]
fn open_regular_candidate(path: &std::path::Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new().read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_regular_candidate(_path: &std::path::Path) -> std::io::Result<File> {
    Err(std::io::Error::other("bounded toolchain observation requires a supported Unix host"))
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool { false }

fn observe_file(
    artifact: &ToolchainArtifactInventory,
    remaining_hash_bytes: &mut u64,
) -> FileObservation {
    let path = artifact.path.display();
    let metadata = match fs::symlink_metadata(&artifact.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return FileObservation {
            status: ToolchainStatus::Missing, digest: None,
            detail: format!("declared local file {path} is missing"),
        },
        Err(error) => return FileObservation {
            status: ToolchainStatus::Unverified, digest: None,
            detail: format!("cannot inspect declared local file {path}: {error}"),
        },
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() == 0 {
        return FileObservation { status: ToolchainStatus::Incompatible, digest: None,
            detail: format!("declared local file {path} must be a nonempty regular file, not a symlink") };
    }
    if metadata.len() > artifact.maximum_bytes {
        return FileObservation { status: ToolchainStatus::Incompatible, digest: None,
            detail: format!("declared local file {path} exceeds its {} byte observation bound", artifact.maximum_bytes) };
    }
    if metadata.len() > *remaining_hash_bytes {
        return FileObservation { status: ToolchainStatus::Unverified, digest: None,
            detail: format!("declared local file {path} exceeds the remaining bounded hash budget") };
    }
    // Charge before reading so failures cannot turn the total bound into an
    // unbounded repeated-read allowance. Each selected dependency is read once.
    *remaining_hash_bytes -= metadata.len();
    let observed = (|| -> std::io::Result<(String, u64)> {
        let input = open_regular_candidate(&artifact.path)?;
        let opened = input.metadata()?;
        if !opened.is_file() || !same_file(&opened, &metadata)
            || opened.len() != metadata.len() || opened.modified()? != metadata.modified()?
        {
            return Err(std::io::Error::other("file changed before hashing"));
        }
        let mut input = input.take(metadata.len() + 1);
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 65536];
        let mut count = 0_u64;
        loop {
            let amount = input.read(&mut buffer)?;
            if amount == 0 { break; }
            count += amount as u64;
            if count > metadata.len() {
                return Err(std::io::Error::other("file grew during hashing"));
            }
            digest.update(&buffer[..amount]);
        }
        let finished = input.get_ref().metadata()?;
        let after = fs::symlink_metadata(&artifact.path)?;
        if finished.len() != metadata.len() || finished.modified()? != metadata.modified()?
            || count != metadata.len() || !after.is_file() || after.file_type().is_symlink()
            || !same_file(&opened, &after)
            || after.len() != metadata.len() || after.modified()? != metadata.modified()?
        {
            return Err(std::io::Error::other("file changed during hashing"));
        }
        Ok((format!("{:x}", digest.finalize()), count))
    })();
    match observed {
        Ok((digest, bytes)) if digest == artifact.expected_sha256 => FileObservation {
            status: ToolchainStatus::Installed, digest: Some(digest.clone()),
            detail: format!("local file {path}: {bytes} bytes, SHA-256 {digest}; matches the caller-supplied inventory pin"),
        },
        Ok((digest, bytes)) => FileObservation {
            status: ToolchainStatus::Incompatible, digest: Some(digest.clone()),
            detail: format!("local file {path}: {bytes} bytes, SHA-256 {digest}; differs from expected inventory SHA-256 {}", artifact.expected_sha256),
        },
        Err(error) => FileObservation { status: ToolchainStatus::Unverified, digest: None,
            detail: format!("could not establish a stable bounded identity for {path}: {error}") },
    }
}

fn fact(
    capability: &str,
    dependency: Option<&str>,
    check: &str,
    status: ToolchainStatus,
    detail: impl Into<String>,
    provenance: Option<String>,
) -> ToolchainFact {
    ToolchainFact {
        capability_id: capability.to_owned(), dependency_id: dependency.map(str::to_owned),
        check: check.to_owned(), status, detail: detail.into(), provenance,
    }
}

fn supplied_provenance(observation: &ToolchainObservation) -> Option<String> {
    // Preserve bounded caller provenance exactly. The binding fact separately
    // states the observed digest and whether this supplied record is usable.
    Some(observation.provenance.clone())
}

fn inspect_dependency(
    capability: &ToolchainCapability,
    dependency: &ToolchainDependency,
    inventory: Option<&ToolchainArtifactInventory>,
    file: Option<&FileObservation>,
    facts: &mut Vec<ToolchainFact>,
) {
    let id = Some(dependency.id.as_str());
    let identity_check = if dependency.kind == ToolchainDependencyKind::Model { "model_identity" } else { "tool_identity" };
    let (identity_status, identity_detail) = match (inventory, file) {
        (Some(_), Some(file)) => (file.status, file.detail.clone()),
        _ => (ToolchainStatus::Missing, "no explicit local inventory path or byte pin was supplied".to_owned()),
    };
    facts.push(fact(&capability.id, id, identity_check, identity_status, identity_detail,
        Some("local regular-file observation; no process was launched".to_owned())));
    if let Some(expected) = &dependency.sha256 {
        let (status, detail) = match file.and_then(|file| file.digest.as_deref()) {
            Some(actual) if actual == expected => (ToolchainStatus::Installed, format!("observed SHA-256 matches explicit profile pin {expected}")),
            Some(actual) => (ToolchainStatus::Incompatible, format!("observed SHA-256 {actual} differs from profile pin {expected}")),
            None => (ToolchainStatus::Unverified, format!("profile SHA-256 {expected} could not be compared with local bytes")),
        };
        facts.push(fact(&capability.id, id, "profile_pin", status, detail, None));
    }
    let supplied = inventory.and_then(|artifact| artifact.observation.as_ref());
    let bound = supplied.filter(|observation| {
        file.is_some_and(|file| file.status == ToolchainStatus::Installed
            && file.digest.as_deref() == Some(observation.executable_sha256.as_str()))
    });
    let locator = inventory.map_or_else(|| "no declared path".to_owned(), |value| value.path.display().to_string());
    let (binding_status, binding_detail) = match (supplied, file.and_then(|value| value.digest.as_deref())) {
        (Some(observation), Some(actual)) if observation.executable_sha256 != actual => (
            ToolchainStatus::Incompatible,
            format!("{locator}: supplied observation is bound to SHA-256 {}, but current local bytes are {actual}", observation.executable_sha256)),
        (Some(observation), _) if bound.is_some() => (
            ToolchainStatus::Installed,
            format!("{locator}: supplied observation is bound to verified local SHA-256 {}", observation.executable_sha256)),
        (Some(observation), _) => (ToolchainStatus::Unverified,
            format!("{locator}: supplied observation SHA-256 {} cannot be bound to verified local bytes", observation.executable_sha256)),
        (None, _) => (ToolchainStatus::Unverified,
            format!("{locator}: no digest-bound package/version/build observation was supplied")),
    };
    facts.push(fact(&capability.id, id, "observation_binding", binding_status, binding_detail,
        supplied.and_then(supplied_provenance)));
    let missing_observation = || format!("{locator}: no supplied observation bound to the verified local digest; no version, flag, feature or model probe was run");
    if let Some(requirement) = &dependency.version_requirement {
        let (status, detail) = match bound.and_then(|observation| observation.version.as_deref()) {
            Some(raw) => match Version::parse(raw) {
                Ok(version) if VersionReq::parse(requirement).expect("validated version requirement").matches(&version) => (
                    ToolchainStatus::Installed, format!("{locator}: supplied semantic version {raw} satisfies {requirement}")),
                Ok(_) => (ToolchainStatus::Incompatible, format!("{locator}: supplied semantic version {raw} does not satisfy {requirement}")),
                Err(_) => (ToolchainStatus::Unverified, format!("{locator}: supplied raw version {raw:?} is not semantic version evidence for {requirement}; no version was invented")),
            },
            None => (ToolchainStatus::Unverified, missing_observation()),
        };
        facts.push(fact(&capability.id, id, "version", status, detail, bound.and_then(supplied_provenance)));
    }
    // A byte digest never establishes which package/model revision the bytes
    // came from. Revision evidence is required even without a prescribed pin.
    let (status, detail) = match bound.and_then(|observation| observation.package_revision.as_deref()) {
        Some(revision) if dependency.package_revision.as_deref().is_none_or(|expected| revision == expected) => (
            ToolchainStatus::Installed, format!("{locator}: caller supplied package/model revision {revision}; origin authenticity was not independently established")),
        Some(revision) => (ToolchainStatus::Incompatible, format!("{locator}: supplied package/model revision {revision} differs from required {}", dependency.package_revision.as_deref().unwrap_or_default())),
        None => (ToolchainStatus::Unverified, format!("{locator}: digest-bound package/model revision evidence is missing")),
    };
    facts.push(fact(&capability.id, id, "package_revision", status, detail, bound.and_then(supplied_provenance)));
    for (check, required, observed) in [
        ("flags", &dependency.required_flags, bound.map(|value| &value.flags)),
        ("features", &dependency.required_features, bound.map(|value| &value.features)),
    ] {
        if required.is_empty() { continue; }
        let (status, detail) = if let Some(observed) = observed {
            let missing: Vec<_> = required.iter().filter(|value| !observed.contains(value)).collect();
            if missing.is_empty() {
                (ToolchainStatus::Installed, format!("{locator}: supplied digest-bound {check} include exact required values {required:?}"))
            } else {
                (ToolchainStatus::Incompatible, format!("{locator}: supplied digest-bound {check} omit required values {missing:?}"))
            }
        } else { (ToolchainStatus::Unverified, missing_observation()) };
        facts.push(fact(&capability.id, id, check, status, detail, bound.and_then(supplied_provenance)));
    }
    if let Some(expected_scale) = dependency.native_scale {
        let (status, detail) = match bound.and_then(|value| value.native_scale) {
            Some(scale) if scale == expected_scale => (ToolchainStatus::Installed, format!("{locator}: supplied model native scale {scale} matches the declared requirement")),
            Some(scale) => (ToolchainStatus::Incompatible, format!("{locator}: supplied model native scale {scale} differs from required {expected_scale}")),
            None => (ToolchainStatus::Unverified, format!("{locator}: model native scale {expected_scale} is only a requirement; no digest-bound scale observation was supplied")),
        };
        facts.push(fact(&capability.id, id, "model_native_scale", status, detail, bound.and_then(supplied_provenance)));
    }
}

/// Inspect only explicitly selected dependencies and return an inert setup plan.
/// An empty capability selection uses the profile's nonoptional defaults.
pub fn inspect_profile(
    profile: &ToolchainProfile,
    inventory: &ToolchainInventory,
    capability_ids: &[String],
) -> Result<ToolchainReport> {
    profile.validate()?;
    inventory.validate()?;
    if !cfg!(unix) {
        return Err(invalid("bounded local toolchain observation requires a supported Unix host"));
    }
    let dependencies: BTreeMap<_, _> = profile.dependencies.iter().map(|value| (value.id.as_str(), value)).collect();
    let capabilities: BTreeMap<_, _> = profile.capabilities.iter().map(|value| (value.id.as_str(), value)).collect();
    let artifacts: BTreeMap<_, _> = inventory.artifacts.iter().map(|value| (value.dependency_id.as_str(), value)).collect();
    if artifacts.keys().any(|id| !dependencies.contains_key(id)) {
        return Err(invalid("toolchain inventory references an unknown dependency"));
    }
    let selected = if capability_ids.is_empty() { &profile.default_capabilities } else { capability_ids };
    let mut selected_set = BTreeSet::new();
    for id in selected {
        if !capabilities.contains_key(id.as_str()) { return Err(invalid(format!("unknown toolchain capability {id}"))); }
        if !selected_set.insert(id.clone()) { return Err(invalid("duplicate toolchain capability selection")); }
    }
    let selected_dependencies: BTreeSet<_> = selected_set.iter()
        .flat_map(|id| capabilities[id.as_str()].dependency_ids.iter().map(String::as_str)).collect();
    let mut files = BTreeMap::new();
    let mut remaining = MAXIMUM_TOTAL_HASH_BYTES;
    for id in selected_dependencies {
        if let Some(artifact) = artifacts.get(id) { files.insert(id, observe_file(artifact, &mut remaining)); }
    }
    let mut facts = Vec::new();
    for id in &selected_set {
        let capability = capabilities[id.as_str()];
        let platform_matches = capability.platforms.contains(&inventory.platform);
        facts.push(fact(id, None, "platform", if platform_matches { ToolchainStatus::Installed } else { ToolchainStatus::Incompatible },
            format!("caller-supplied platform {}/{} {}; supported declaration {:?}", inventory.platform.os, inventory.platform.arch,
                if platform_matches { "matches" } else { "does not match" }, capability.platforms),
            Some("caller-supplied inventory.platform; host execution was not probed".to_owned())));
        if let Some(requirement) = &capability.backend {
            let observed = inventory.hardware.iter().find(|value| value.name == requirement.name);
            let (status, detail, provenance) = match observed {
                Some(value) if !value.available => (ToolchainStatus::Missing, format!("backend {} is explicitly reported unavailable", requirement.name), Some(value.provenance.clone())),
                Some(value) => {
                    let missing: Vec<_> = requirement.required_features.iter().filter(|feature| !value.features.contains(feature)).collect();
                    (if missing.is_empty() { ToolchainStatus::Installed } else { ToolchainStatus::Incompatible },
                        format!("caller-supplied backend {} availability; missing required features {missing:?}", requirement.name), Some(value.provenance.clone()))
                },
                None => (ToolchainStatus::Unverified, format!("backend {} has no explicit hardware observation", requirement.name), None),
            };
            facts.push(fact(id, None, "backend", status, detail,
                provenance));
        }
        if let Some(scale) = &capability.scale {
            facts.push(fact(id, None, "scale", ToolchainStatus::Installed,
                format!("requested output scale {}; native model scale {}; mode {:?}; these are declared settings, and model identity/scale observations are separate", scale.requested, scale.native, scale.mode),
                Some("explicit profile scale declaration; no image or model was executed".to_owned())));
        }
        facts.push(fact(id, None, "configuration", ToolchainStatus::Installed,
            format!("declared effective settings {}; declared side effects {}",
                serde_json::to_string(&capability.effective_settings).map_err(|_| invalid("cannot encode effective settings"))?,
                serde_json::to_string(&capability.side_effects).map_err(|_| invalid("cannot encode side effects"))?),
            Some("profile declaration only; no settings were applied".to_owned())));
        for dependency_id in &capability.dependency_ids {
            inspect_dependency(capability, dependencies[dependency_id.as_str()], artifacts.get(dependency_id.as_str()).copied(), files.get(dependency_id.as_str()), &mut facts);
        }
    }
    let ready = facts.iter().all(|fact| fact.status == ToolchainStatus::Installed);
    let mut actions = Vec::new();
    for fact in &facts {
        if fact.status == ToolchainStatus::Installed { continue; }
        let suggested_locators = fact.dependency_id.as_deref().and_then(|id| dependencies.get(id))
            .map_or_else(Vec::new, |value| value.suggested_locators.clone());
        actions.push(ToolchainAction {
            capability_id: fact.capability_id.clone(), dependency_id: fact.dependency_id.clone(),
            kind: match fact.status { ToolchainStatus::Missing => "supply_local_dependency", ToolchainStatus::Incompatible => "resolve_requirement_mismatch", _ => "supply_bound_observation" }.to_owned(),
            detail: format!("Resolve {}: {}. Suggested locators are unverified references, not installation commands or approval to download.", fact.check, fact.detail),
            suggested_locators,
        });
    }
    for id in &selected_set {
        actions.push(ToolchainAction {
            capability_id: id.clone(), dependency_id: None, kind: "explicit_adapter_handoff".to_owned(),
            detail: "Native qualification remains outstanding. Any later Pipeline v3 integration must explicitly register a protocol adapter and bind package revision, backend, scale and effective settings in its closed configuration. Provider-lock/v1 gains no implicit fields. Ordinary native CLI tools are not protocol providers. This plan performs no registration or execution.".to_owned(),
            suggested_locators: Vec::new(),
        });
    }
    Ok(ToolchainReport {
        schema: TOOLCHAIN_REPORT_SCHEMA.to_owned(), profile_id: profile.id.clone(),
        profile_sha256: canonical_sha256(profile)?, inventory_sha256: canonical_sha256(inventory)?,
        selected_capabilities: selected_set.into_iter().collect(), ready,
        native_qualification: false, facts, actions,
    })
}
