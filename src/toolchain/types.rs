//! Closed, bounded declarations and supplied observations for offline toolchains.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
#[cfg(unix)]
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{Error, ErrorCategory, Result, SideEffect};

pub const TOOLCHAIN_PROFILE_SCHEMA: &str = "aniflow.toolchain-profile/v1";
pub const TOOLCHAIN_INVENTORY_SCHEMA: &str = "aniflow.toolchain-inventory/v1";
pub const TOOLCHAIN_REPORT_SCHEMA: &str = "aniflow.toolchain-report/v1";
const MAXIMUM_JSON_BYTES: usize = 1_048_576;
const MAXIMUM_ARTIFACT_BYTES: u64 = 1_073_741_824;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProfile {
    pub schema: String,
    pub id: String,
    pub default_capabilities: Vec<String>,
    pub dependencies: Vec<ToolchainDependency>,
    pub capabilities: Vec<ToolchainCapability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainDependencyKind {
    Tool,
    Model,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainDependency {
    pub id: String,
    pub kind: ToolchainDependencyKind,
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_requirement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub required_flags: Vec<String>,
    pub required_features: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_scale: Option<u32>,
    pub suggested_locators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainCapability {
    pub id: String,
    pub optional: bool,
    pub dependency_ids: Vec<String>,
    pub platforms: Vec<ToolchainPlatform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<ToolchainBackendRequirement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<ToolchainScaleRequirement>,
    pub effective_settings: BTreeMap<String, Value>,
    pub side_effects: Vec<SideEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainPlatform {
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainBackendRequirement {
    pub name: String,
    pub required_features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainScaleRequirement {
    pub native: u32,
    pub requested: u32,
    pub mode: ToolchainScaleMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainScaleMode {
    Native,
    PostResize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainInventory {
    pub schema: String,
    pub platform: ToolchainPlatform,
    pub artifacts: Vec<ToolchainArtifactInventory>,
    pub hardware: Vec<ToolchainHardwareObservation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainArtifactInventory {
    pub dependency_id: String,
    pub path: PathBuf,
    pub expected_sha256: String,
    #[serde(default = "default_maximum_bytes")]
    pub maximum_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<ToolchainObservation>,
}

const fn default_maximum_bytes() -> u64 {
    536_870_912
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainObservation {
    pub executable_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_scale: Option<u32>,
    pub flags: Vec<String>,
    pub features: Vec<String>,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainHardwareObservation {
    pub name: String,
    pub available: bool,
    pub features: Vec<String>,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainReport {
    pub schema: String,
    pub profile_id: String,
    pub profile_sha256: String,
    pub inventory_sha256: String,
    pub selected_capabilities: Vec<String>,
    pub ready: bool,
    pub native_qualification: bool,
    pub facts: Vec<ToolchainFact>,
    pub actions: Vec<ToolchainAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainFact {
    pub capability_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_id: Option<String>,
    pub check: String,
    pub status: ToolchainStatus,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainStatus {
    Installed,
    Missing,
    Incompatible,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainAction {
    pub capability_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_id: Option<String>,
    pub kind: String,
    pub detail: String,
    pub suggested_locators: Vec<String>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

fn token(value: &str, field: &str) -> Result<()> {
    if value.len() > 128 {
        return Err(invalid(format!("{field} exceeds 128 bytes")));
    }
    crate::provider::require_token(value, field)
}

fn text(value: &str, maximum: usize, field: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > maximum
        || value.chars().any(char::is_control)
        || value.trim().is_empty()
    {
        return Err(invalid(format!("{field} must contain 1..={maximum} printable bytes")));
    }
    Ok(())
}

fn unique_strings(values: &[String], maximum: usize, item_maximum: usize, field: &str) -> Result<()> {
    if values.len() > maximum {
        return Err(invalid(format!("{field} exceeds {maximum} entries")));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        text(value, item_maximum, field)?;
        if !seen.insert(value) {
            return Err(invalid(format!("{field} contains a duplicate entry")));
        }
    }
    Ok(())
}

fn identifiers(values: &[String], maximum: usize, field: &str) -> Result<()> {
    unique_strings(values, maximum, 128, field)?;
    for value in values {
        token(value, field)?;
    }
    Ok(())
}

fn features(values: &[String], field: &str) -> Result<()> {
    unique_strings(values, 128, 256, field)
}

fn locators(values: &[String]) -> Result<()> {
    unique_strings(values, 16, 2048, "suggested locators")?;
    for value in values {
        let Some(rest) = value.strip_prefix("https://") else {
            return Err(invalid("suggested locators must use https"));
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if authority.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(invalid("suggested locators require an https authority without whitespace"));
        }
    }
    Ok(())
}

fn platform(value: &ToolchainPlatform) -> Result<()> {
    token(&value.os, "platform OS")?;
    token(&value.arch, "platform architecture")
}

fn scale(value: u32, field: &str) -> Result<()> {
    if !(1..=64).contains(&value) {
        return Err(invalid(format!("{field} must be in 1..=64")));
    }
    Ok(())
}

fn settings_depth(value: &Value, depth: usize) -> Result<()> {
    if depth > 16 {
        return Err(invalid("effective settings exceed depth 16"));
    }
    match value {
        Value::Array(values) => {
            for value in values {
                settings_depth(value, depth + 1)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                settings_depth(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

impl ToolchainProfile {
    /// Read only a regular nonsymlink JSON file, bounded to one MiB.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_json_slice(&read_json(path.as_ref())?)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        let value: Self = decode_unique_json(bytes)?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != TOOLCHAIN_PROFILE_SCHEMA {
            return Err(invalid("unsupported toolchain profile schema"));
        }
        token(&self.id, "profile ID")?;
        if self.dependencies.len() > 128 || self.capabilities.len() > 64 {
            return Err(invalid("profile exceeds 128 dependencies or 64 capabilities"));
        }
        if self.default_capabilities.is_empty() {
            return Err(invalid("profile requires at least one default capability"));
        }
        identifiers(&self.default_capabilities, 64, "default capabilities")?;
        let mut dependencies = BTreeMap::new();
        for dependency in &self.dependencies {
            dependency.validate()?;
            if dependencies.insert(dependency.id.as_str(), dependency).is_some() {
                return Err(invalid("duplicate toolchain dependency ID"));
            }
        }
        let mut capabilities = BTreeMap::new();
        for capability in &self.capabilities {
            capability.validate()?;
            if capabilities.insert(capability.id.as_str(), capability).is_some() {
                return Err(invalid("duplicate toolchain capability ID"));
            }
            for dependency_id in &capability.dependency_ids {
                let dependency = dependencies.get(dependency_id.as_str()).ok_or_else(|| {
                    invalid(format!("capability {} references unknown dependency", capability.id))
                })?;
                if !capability.optional && dependency.optional {
                    return Err(invalid("core capabilities cannot require optional dependencies"));
                }
                if let (Some(native), Some(scale)) = (dependency.native_scale, &capability.scale) {
                    if native != scale.native {
                        return Err(invalid("capability native scale differs from its model declaration"));
                    }
                }
            }
            if let Some(scale) = &capability.scale {
                let backed_by_model = capability.dependency_ids.iter().any(|id| {
                    dependencies.get(id.as_str()).is_some_and(|dependency| {
                        dependency.kind == ToolchainDependencyKind::Model
                            && dependency.native_scale == Some(scale.native)
                    })
                });
                if !backed_by_model {
                    return Err(invalid("scale capabilities require a referenced model with matching native scale"));
                }
            }
        }
        for id in &self.default_capabilities {
            let capability = capabilities.get(id.as_str()).ok_or_else(|| {
                invalid("default capability references an unknown capability")
            })?;
            if capability.optional {
                return Err(invalid("optional capabilities cannot be selected by default"));
            }
        }
        Ok(())
    }
}

impl ToolchainDependency {
    fn validate(&self) -> Result<()> {
        token(&self.id, "dependency ID")?;
        if let Some(value) = &self.version_requirement {
            text(value, 256, "version requirement")?;
            semver::VersionReq::parse(value)
                .map_err(|_| invalid("tool version requirement must be a semantic version requirement"))?;
        }
        if let Some(value) = &self.package_revision {
            text(value, 256, "package revision")?;
        }
        if let Some(value) = &self.sha256 {
            crate::provider::require_sha256(value, "dependency SHA-256")?;
        }
        features(&self.required_flags, "required flags")?;
        features(&self.required_features, "required features")?;
        locators(&self.suggested_locators)?;
        match self.kind {
            ToolchainDependencyKind::Tool => {
                if self.native_scale.is_some() {
                    return Err(invalid("tool dependencies cannot declare a model native scale"));
                }
            }
            ToolchainDependencyKind::Model => {
                if self.version_requirement.is_some()
                    || !self.required_flags.is_empty()
                    || !self.required_features.is_empty()
                {
                    return Err(invalid("models cannot declare tool versions, flags or features"));
                }
                if let Some(value) = self.native_scale {
                    scale(value, "model native scale")?;
                }
            }
        }
        Ok(())
    }
}

impl ToolchainCapability {
    fn validate(&self) -> Result<()> {
        token(&self.id, "capability ID")?;
        if self.dependency_ids.is_empty() || self.platforms.is_empty() || self.platforms.len() > 32 {
            return Err(invalid("capabilities require dependencies and 1..=32 platforms"));
        }
        identifiers(&self.dependency_ids, 128, "capability dependencies")?;
        let mut platforms = BTreeSet::new();
        for value in &self.platforms {
            platform(value)?;
            if !platforms.insert(value) {
                return Err(invalid("duplicate capability platform"));
            }
        }
        if let Some(backend) = &self.backend {
            token(&backend.name, "backend name")?;
            features(&backend.required_features, "backend required features")?;
        }
        if let Some(value) = &self.scale {
            scale(value.native, "native scale")?;
            scale(value.requested, "requested scale")?;
            if value.mode == ToolchainScaleMode::Native && value.native != value.requested {
                return Err(invalid("native scale mode requires equal native and requested scale"));
            }
        }
        let mut side_effects = BTreeSet::new();
        for value in &self.side_effects {
            if !side_effects.insert(value) {
                return Err(invalid("duplicate capability side effect"));
            }
        }
        for value in self.effective_settings.values() {
            settings_depth(value, 1)?;
        }
        let settings = serde_json::to_vec(&self.effective_settings)
            .map_err(|_| invalid("effective settings cannot be serialized"))?;
        if settings.len() > 65_536 {
            return Err(invalid("effective settings exceed 64 KiB"));
        }
        Ok(())
    }
}

impl ToolchainInventory {
    /// Read supplied metadata only; never execute, locate or install a tool.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_json_slice(&read_json(path.as_ref())?)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        let value: Self = decode_unique_json(bytes)?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != TOOLCHAIN_INVENTORY_SCHEMA {
            return Err(invalid("unsupported toolchain inventory schema"));
        }
        platform(&self.platform)?;
        if self.artifacts.len() > 128 || self.hardware.len() > 32 {
            return Err(invalid("inventory exceeds 128 artifacts or 32 hardware observations"));
        }
        let mut dependencies = BTreeSet::new();
        for artifact in &self.artifacts {
            token(&artifact.dependency_id, "inventory dependency ID")?;
            if !dependencies.insert(&artifact.dependency_id) {
                return Err(invalid("duplicate inventory dependency ID"));
            }
            literal_absolute_path(&artifact.path)?;
            crate::provider::require_sha256(&artifact.expected_sha256, "inventory SHA-256")?;
            if artifact.maximum_bytes == 0 || artifact.maximum_bytes > MAXIMUM_ARTIFACT_BYTES {
                return Err(invalid("artifact maximum bytes must be in 1..=1073741824"));
            }
            if let Some(observation) = &artifact.observation {
                observation.validate()?;
            }
        }
        let mut hardware = BTreeSet::new();
        for observation in &self.hardware {
            token(&observation.name, "hardware observation name")?;
            if !hardware.insert(&observation.name) {
                return Err(invalid("duplicate hardware observation name"));
            }
            features(&observation.features, "hardware features")?;
            text(&observation.provenance, 4096, "hardware provenance")?;
        }
        Ok(())
    }
}

impl ToolchainObservation {
    fn validate(&self) -> Result<()> {
        crate::provider::require_sha256(&self.executable_sha256, "observation executable SHA-256")?;
        if let Some(value) = &self.version {
            // Preserve raw non-semver output so inspection can report it unverified.
            text(value, 256, "observed version")?;
        }
        if let Some(value) = &self.package_revision {
            text(value, 256, "observed package revision")?;
        }
        if let Some(value) = self.native_scale {
            scale(value, "observed native scale")?;
        }
        unique_strings(&self.flags, 4096, 256, "observed flags")?;
        unique_strings(&self.features, 4096, 256, "observed features")?;
        text(&self.provenance, 4096, "observation provenance")
    }
}

fn literal_absolute_path(path: &Path) -> Result<()> {
    let value = path.to_str().ok_or_else(|| invalid("inventory path must be UTF-8"))?;
    let lexical_traversal = value
        .split(|character| character == '/' || (cfg!(windows) && character == '\\'))
        .any(|part| matches!(part, "." | ".."));
    if !path.is_absolute()
        || value.contains('\0')
        || lexical_traversal
        || path.components().any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid("inventory paths must be literal absolute paths without traversal or NUL"));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn read_json(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| invalid("toolchain JSON file is unavailable"))?;
    if !metadata.file_type().is_file() || metadata.len() > MAXIMUM_JSON_BYTES as u64 {
        return Err(invalid("toolchain JSON must be a nonsymlink regular file no larger than one MiB"));
    }
    let mut input = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| invalid("toolchain JSON file cannot be safely opened"))?;
    let opened = input.metadata().map_err(|_| invalid("toolchain JSON file is unavailable"))?;
    let current = fs::symlink_metadata(path)
        .map_err(|_| invalid("toolchain JSON file changed during opening"))?;
    if !same_json_file(&metadata, &opened) || !same_json_file(&metadata, &current) {
        return Err(invalid("toolchain JSON must remain the same nonsymlink regular file"));
    }
    let mut bytes = Vec::new();
    (&mut input).take(MAXIMUM_JSON_BYTES as u64 + 1).read_to_end(&mut bytes)
        .map_err(|_| invalid("toolchain JSON file cannot be read"))?;
    let after = input.metadata().map_err(|_| invalid("toolchain JSON file is unavailable"))?;
    let current = fs::symlink_metadata(path)
        .map_err(|_| invalid("toolchain JSON file changed during reading"))?;
    if bytes.len() > MAXIMUM_JSON_BYTES
        || bytes.len() as u64 != metadata.len()
        || !same_json_file(&metadata, &after)
        || !same_json_file(&metadata, &current)
    {
        return Err(invalid("toolchain JSON file changed or exceeded its one MiB bound"));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn same_json_file(expected: &fs::Metadata, actual: &fs::Metadata) -> bool {
    actual.file_type().is_file()
        && expected.dev() == actual.dev()
        && expected.ino() == actual.ino()
        && expected.len() == actual.len()
        && matches!((expected.modified(), actual.modified()), (Ok(left), Ok(right)) if left == right)
}

#[cfg(not(unix))]
pub(super) fn read_json(_path: &Path) -> Result<Vec<u8>> {
    Err(invalid("safe toolchain JSON file loading is unsupported on this host; use bounded JSON bytes"))
}

pub(super) fn decode_unique_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    decode_unique_json_with_limit(bytes, MAXIMUM_JSON_BYTES)
}

pub(super) fn decode_unique_json_with_limit<T: DeserializeOwned>(bytes: &[u8], maximum_bytes: usize) -> Result<T> {
    if bytes.len() > maximum_bytes {
        return Err(invalid(format!("toolchain JSON exceeds its {maximum_bytes} byte bound")));
    }
    let value: UniqueJson = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid toolchain JSON: {error}")))?;
    reject_structural_nulls(&value.0)?;
    serde_json::from_value(value.0)
        .map_err(|error| invalid(format!("invalid toolchain declaration: {error}")))
}

fn reject_structural_nulls(value: &Value) -> Result<()> {
    match value {
        Value::Null => return Err(invalid("toolchain optional fields must be omitted, not null")),
        Value::Array(values) => {
            for value in values { reject_structural_nulls(value)?; }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key != "effective_settings" { reject_structural_nulls(value)?; }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Retain duplicate-key rejection even within untyped effective settings.
struct UniqueJson(Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueJson;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("JSON with unique object keys")
            }

            fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(Value::Bool(value)))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(Value::Number(value.into())))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(Value::Number(value.into())))
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueJson(Value::Number(number)))
                    .ok_or_else(|| E::custom("JSON numbers must be finite"))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(Value::String(value.to_owned())))
            }

            fn visit_string<E: de::Error>(self, value: String) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(Value::String(value)))
            }

            fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(Value::Null))
            }

            fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(Value::Null))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueJson>()? {
                    values.push(value.0);
                }
                Ok(UniqueJson(Value::Array(values)))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate JSON object key"));
                    }
                    values.insert(key, map.next_value::<UniqueJson>()?.0);
                }
                Ok(UniqueJson(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile_json() -> Value {
        json!({
            "schema": TOOLCHAIN_PROFILE_SCHEMA, "id": "synthetic",
            "default_capabilities": ["audio/inspect"],
            "dependencies": [{"id": "ffprobe", "kind": "tool", "optional": false,
                "version_requirement": ">=6.0.0", "required_flags": [],
                "required_features": [], "suggested_locators": ["https://example.invalid/tool"]}],
            "capabilities": [{"id": "audio/inspect", "optional": false,
                "dependency_ids": ["ffprobe"], "platforms": [{"os": "linux", "arch": "x86_64"}],
                "effective_settings": {}, "side_effects": ["filesystem_read"]}]
        })
    }

    #[test]
    fn closed_json_rejects_duplicates_unknown_fields_and_missing_arrays() {
        let original = profile_json().to_string();
        assert!(ToolchainProfile::from_json_slice(original.as_bytes()).is_ok());
        let duplicate = original.replace("\"effective_settings\":{}", "\"effective_settings\":{\"nested\":{\"a\":1,\"a\":2}}");
        assert!(ToolchainProfile::from_json_slice(duplicate.as_bytes()).is_err());
        let mut value = profile_json();
        value["dependencies"][0].as_object_mut().unwrap().remove("required_flags");
        assert!(ToolchainProfile::from_json_slice(value.to_string().as_bytes()).is_err());
        let mut value = profile_json();
        value["capabilities"][0]["execute"] = json!("unrecognized command");
        assert!(ToolchainProfile::from_json_slice(value.to_string().as_bytes()).is_err());
        let mut value = profile_json();
        value["dependencies"][0]["sha256"] = Value::Null;
        assert!(ToolchainProfile::from_json_slice(value.to_string().as_bytes()).is_err());
        value["dependencies"][0].as_object_mut().unwrap().remove("sha256");
        value["capabilities"][0]["effective_settings"]["nullable_setting"] = Value::Null;
        assert!(ToolchainProfile::from_json_slice(value.to_string().as_bytes()).is_ok());
    }

    #[test]
    fn profile_refuses_optional_core_dependencies_unknown_refs_and_oversized_settings() {
        let mut profile = ToolchainProfile::from_json_slice(profile_json().to_string().as_bytes()).unwrap();
        profile.dependencies[0].optional = true;
        assert!(profile.validate().is_err());
        profile.dependencies[0].optional = false;
        profile.capabilities[0].dependency_ids.push("absent".to_owned());
        assert!(profile.validate().is_err());
        profile.capabilities[0].dependency_ids.pop();
        profile.capabilities[0].effective_settings.insert("large".to_owned(), json!("x".repeat(65_536)));
        assert!(profile.validate().is_err());
        let mut nested = Value::Null;
        for _ in 0..17 { nested = json!([nested]); }
        profile.capabilities[0].effective_settings.insert("large".to_owned(), nested);
        assert!(profile.validate().is_err());
    }

    #[test]
    fn inventory_preserves_literal_paths_and_raw_versions_but_refuses_traversal() {
        let path = std::env::temp_dir().join("a tool;$(literal)");
        let bytes = json!({"schema": TOOLCHAIN_INVENTORY_SCHEMA,
            "platform": {"os": "linux", "arch": "x86_64"}, "hardware": [],
            "artifacts": [{"dependency_id": "ffprobe", "path": path,
                "expected_sha256": "a".repeat(64), "observation": {
                    "executable_sha256": "a".repeat(64), "version": "development snapshot",
                    "native_scale": 4, "flags": [], "features": [], "provenance": "synthetic fixture"}}]
        }).to_string();
        let mut inventory = ToolchainInventory::from_json_slice(bytes.as_bytes()).unwrap();
        assert_eq!(inventory.artifacts[0].path, path);
        assert_eq!(inventory.artifacts[0].maximum_bytes, 536_870_912);
        inventory.artifacts[0].path = std::env::temp_dir().join("..").join("tool");
        assert!(inventory.validate().is_err());
        inventory.artifacts[0].path = path;
        inventory.artifacts[0].observation.as_mut().unwrap().native_scale = Some(65);
        assert!(inventory.validate().is_err());
    }

    #[test]
    fn scaling_requires_model_evidence_and_native_mode_cannot_resize() {
        let mut profile = ToolchainProfile::from_json_slice(profile_json().to_string().as_bytes()).unwrap();
        profile.capabilities[0].scale = Some(ToolchainScaleRequirement {
            native: 4, requested: 4, mode: ToolchainScaleMode::Native,
        });
        assert!(profile.validate().is_err());
        profile.dependencies[0].kind = ToolchainDependencyKind::Model;
        profile.dependencies[0].version_requirement = None;
        profile.dependencies[0].native_scale = Some(4);
        assert!(profile.validate().is_ok());
        profile.capabilities[0].scale.as_mut().unwrap().requested = 2;
        assert!(profile.validate().is_err());
        profile.capabilities[0].scale.as_mut().unwrap().mode = ToolchainScaleMode::PostResize;
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn loader_refuses_oversized_documents_without_reading_artifacts() {
        let source = tempfile::NamedTempFile::new().unwrap();
        source.as_file().set_len(MAXIMUM_JSON_BYTES as u64 + 1).unwrap();
        assert!(ToolchainProfile::load(source.path()).is_err());
        assert!(ToolchainInventory::from_json_slice(&vec![b' '; MAXIMUM_JSON_BYTES + 1]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn loader_refuses_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("profile.json");
        fs::write(&source, profile_json().to_string()).unwrap();
        let link = directory.path().join("link.json");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(ToolchainProfile::load(&link).is_err());
        assert!(ToolchainProfile::load(&source).is_ok());
    }
}
