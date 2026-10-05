//! Inert preparation of the existing native audio-inspection registration.
//!
//! Preparation observes local bytes without launching a tool or writing files.
//! The returned adapter digest is preparation-time evidence, not an extension
//! of the registration locator contract. Compare it with the first plan's
//! implementation digest; reprepare if the adapter changes before planning.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{ToolchainPreflightConfiguration, ToolchainReport, inspect_profile};
use crate::audio_analysis::AudioArtifactReference;
use crate::audio_inspection::{
    AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1,
    AUDIO_INSPECTION_MAXIMUM_BYTES, AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V2, AudioInspectionConfiguration,
    AudioInspectionProviderConfiguration, AudioToolPin,
};
use crate::{
    ComponentIdentity, ComponentInventory, Error, ErrorCategory, ProviderImplementationIdentity,
    ProviderManifest, ProviderRegistrationDocument, Result, PROVIDER_REGISTRATION_SCHEMA_V1,
};

pub const AUDIO_INSPECTION_REGISTRATION_SCHEMA: &str =
    "aniflow.toolchain.audio-inspection-registration/v1";
pub const TOOLCHAIN_REGISTRATION_PREPARATION_SCHEMA: &str =
    "aniflow.toolchain.registration-preparation/v1";
const MAXIMUM_REQUEST_BYTES: usize = 1_048_576;
const MAXIMUM_PREPARATION_BYTES: usize = 8 * 1_048_576;
const FILE_NAMES: [&str; 4] = [
    "configuration.json", "manifest.json", "preflight.json", "registration.json",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInspectionRegistrationRequest {
    pub schema: String,
    pub registration_directory: PathBuf,
    pub adapter: ToolchainRegistrationAdapter,
    pub preflight: ToolchainPreflightConfiguration,
    /// Caller-declared identity only. Preparation never opens source media.
    pub source: AudioArtifactReference,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainRegistrationAdapter {
    pub relative_path: String,
    pub implementation_id: String,
    pub expected_sha256: String,
    pub maximum_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainRegistrationFile {
    pub relative_path: String,
    /// SHA-256 of the canonical JSON encoding of `content`.
    pub sha256: String,
    pub content: Value,
}

/// Fresh preparation evidence; deserialization does not authorize execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainRegistrationPreparation {
    pub schema: String,
    pub request_sha256: String,
    pub ready: bool,
    pub native_qualification: bool,
    /// The ID is caller-supplied; the digest is observed from the adapter file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<ProviderImplementationIdentity>,
    pub inspection: ToolchainReport,
    pub files: Vec<ToolchainRegistrationFile>,
    pub diagnostics: Vec<String>,
}

fn invalid(detail: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, detail)
}

fn registration_document(
    request: &AudioInspectionRegistrationRequest,
    components: ComponentInventory,
) -> Result<ProviderRegistrationDocument> {
    ProviderRegistrationDocument::from_json_slice(&crate::provider::canonical_json_bytes(
        &serde_json::json!({
            "schema": PROVIDER_REGISTRATION_SCHEMA_V1,
            "registration_id": request.preflight.bindings[0].registration_id,
            "manifest": "manifest.json",
            "configuration": "configuration.json",
            "executable": request.adapter.relative_path,
            "implementation_id": request.adapter.implementation_id,
            "components": components,
        }),
    )?)
}

impl AudioInspectionRegistrationRequest {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_json_slice(&super::types::read_json(path.as_ref())?)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        let request: Self = super::types::decode_unique_json(bytes)?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_INSPECTION_REGISTRATION_SCHEMA {
            return Err(invalid("unsupported audio-inspection registration request schema"));
        }
        self.preflight.validate()?;
        if self.preflight.bindings.len() != 1 {
            return Err(invalid("registration preparation requires exactly one explicit stage binding"));
        }
        let directory = self.registration_directory.to_str()
            .ok_or_else(|| invalid("registration directory must be UTF-8"))?;
        if !self.registration_directory.is_absolute() || directory.len() > 4096
            || directory.chars().any(char::is_control)
            || directory.split('/').any(|part| matches!(part, "." | ".."))
        {
            return Err(invalid("registration directory must be an absolute printable path without traversal, at most 4096 bytes"));
        }
        if self.adapter.relative_path.len() > 4096 || self.adapter.implementation_id.len() > 128
            || !(1..=1_073_741_824).contains(&self.adapter.maximum_bytes)
        {
            return Err(invalid("adapter locator, implementation ID or byte bound is unsupported"));
        }
        crate::provider::require_sha256(&self.adapter.expected_sha256, "adapter expected SHA-256")?;
        registration_document(self, ComponentInventory::default())?;
        let first = self.adapter.relative_path.split('/').next().unwrap_or("");
        if FILE_NAMES.iter().any(|name| first.eq_ignore_ascii_case(name)) {
            return Err(invalid("adapter locator overlaps a reserved registration document filename"));
        }
        // These source/limit bounds are the closed native-v2 wrapper's bounds;
        // its full validator also runs on the actual derived tool pins below.
        if self.source.id != "source_audio"
            || !(44..=AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&self.source.byte_size)
            || !(1..=120_000).contains(&self.tool_timeout_milliseconds)
            || !(1024..=1_048_576).contains(&self.maximum_tool_output_bytes)
        {
            return Err(invalid("source identity or native audio-inspection limits are unsupported"));
        }
        crate::provider::require_sha256(&self.source.sha256, "source SHA-256")?;
        let selected = &self.preflight.bindings[0].capability_ids;
        let effects: BTreeSet<_> = self.preflight.profile.capabilities.iter()
            .filter(|capability| selected.contains(&capability.id))
            .flat_map(|capability| capability.side_effects.iter().copied()).collect();
        let manifest = provider_manifest()?;
        let expected: BTreeSet<_> = manifest.capabilities[0].behavior.side_effects.iter().copied().collect();
        if effects != expected {
            return Err(invalid("selected profile side effects must exactly match native audio inspection"));
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAXIMUM_REQUEST_BYTES {
            return Err(invalid("registration preparation request exceeds one MiB"));
        }
        Ok(bytes)
    }

    pub fn sha256(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(self.canonical_json_bytes()?)))
    }
}

impl ToolchainRegistrationPreparation {
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAXIMUM_PREPARATION_BYTES {
            return Err(invalid("registration preparation exceeds eight MiB"));
        }
        Ok(bytes)
    }
}

fn provider_manifest() -> Result<ProviderManifest> {
    ProviderManifest::from_json_slice(include_bytes!("../../providers/audio-inspection/manifest.json"))
}

fn artifact<T: Serialize>(relative_path: &str, content: &T) -> Result<ToolchainRegistrationFile> {
    let content = serde_json::to_value(content)
        .map_err(|error| invalid(format!("cannot encode prepared registration document: {error}")))?;
    Ok(ToolchainRegistrationFile {
        relative_path: relative_path.to_owned(),
        sha256: crate::provider::canonical_sha256(&content)?,
        content,
    })
}

fn tool_pin(request: &AudioInspectionRegistrationRequest, dependency_id: &str) -> Result<AudioToolPin> {
    let inventory = request.preflight.inventory.artifacts.iter()
        .find(|artifact| artifact.dependency_id == dependency_id)
        .ok_or_else(|| invalid(format!("{dependency_id}: missing explicit tool inventory")))?;
    let observation = inventory.observation.as_ref()
        .ok_or_else(|| invalid(format!("{dependency_id}: missing digest-bound version observation")))?;
    if observation.executable_sha256 != inventory.expected_sha256 {
        return Err(invalid(format!("{dependency_id}: observation digest differs from inventory pin")));
    }
    let version = observation.version.as_ref()
        .ok_or_else(|| invalid(format!("{dependency_id}: missing exact tool version")))?;
    // Never normalize a vendor token here: the native provider checks the exact banner token.
    crate::provider::validate_semantic_version(version, "audio tool version")?;
    let pin = AudioToolPin {
        executable: inventory.path.clone(), version: version.clone(),
        sha256: inventory.expected_sha256.clone(),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::symlink_metadata(&pin.executable)
            .map_err(|_| invalid(format!("{dependency_id}: tool file is unavailable")))?;
        if !metadata.is_file() || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(invalid(format!("{dependency_id}: tool must be a nonsymlink executable regular file")));
        }
    }
    Ok(pin)
}

/// Prepare four existing-format documents without writing, registering,
/// probing, installing, downloading or reading media. `ready` means current
/// local-byte and supplied-inventory consistency, never native qualification.
pub fn prepare_audio_inspection_registration(
    request: &AudioInspectionRegistrationRequest,
) -> Result<ToolchainRegistrationPreparation> {
    let request_sha256 = request.sha256()?;
    let binding = &request.preflight.bindings[0];
    let inspection = inspect_profile(
        &request.preflight.profile, &request.preflight.inventory, &binding.capability_ids,
    )?;
    let mut diagnostics = Vec::new();
    if !inspection.ready {
        diagnostics.push("selected tool inventory is not ready; inspect its facts and actions".to_owned());
    }
    let platform = &request.preflight.inventory.platform;
    if platform.os != std::env::consts::OS || platform.arch != std::env::consts::ARCH {
        diagnostics.push("supplied inventory platform differs from the current preparation host".to_owned());
    }
    let adapter = match observe_adapter(request) {
        Ok(identity) => {
            if identity.executable_sha256 != request.adapter.expected_sha256 {
                diagnostics.push(format!("adapter digest {} differs from caller pin {}",
                    identity.executable_sha256, request.adapter.expected_sha256));
            }
            Some(identity)
        }
        Err(error) => { diagnostics.push(error.to_string()); None }
    };
    let files = match prepare_files(request) {
        Ok(files) if diagnostics.is_empty() => files,
        Ok(_) => Vec::new(),
        Err(error) => { diagnostics.push(error.to_string()); Vec::new() }
    };
    let result = ToolchainRegistrationPreparation {
        schema: TOOLCHAIN_REGISTRATION_PREPARATION_SCHEMA.to_owned(), request_sha256,
        ready: diagnostics.is_empty(), native_qualification: false,
        adapter, inspection, files, diagnostics,
    };
    result.canonical_json_bytes()?;
    Ok(result)
}

fn prepare_files(request: &AudioInspectionRegistrationRequest) -> Result<Vec<ToolchainRegistrationFile>> {
    let binding = &request.preflight.bindings[0];
    let settings = AudioInspectionConfiguration {
        schema: AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.to_owned(),
        ffmpeg: tool_pin(request, &binding.ffmpeg_dependency_id)?,
        ffprobe: tool_pin(request, &binding.ffprobe_dependency_id)?,
        tool_timeout_milliseconds: request.tool_timeout_milliseconds,
        maximum_tool_output_bytes: request.maximum_tool_output_bytes,
    };
    let configuration = AudioInspectionProviderConfiguration {
        schema: AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V2.to_owned(),
        settings: settings.clone(), source: request.source.clone(),
    }.provider_configuration()?;
    let mut manifest = provider_manifest()?;
    let mut tools = Vec::new();
    for (id, pin) in [("ffmpeg", &settings.ffmpeg), ("ffprobe", &settings.ffprobe)] {
        let requirement = manifest.capabilities[0].requirements.tools.iter_mut()
            .find(|requirement| requirement.id == id)
            .ok_or_else(|| invalid("native audio manifest lacks its required tool"))?;
        requirement.version_requirement = format!("={}", pin.version);
        requirement.sha256 = Some(pin.sha256.clone());
        tools.push(ComponentIdentity { id: id.to_owned(), version: pin.version.clone(), sha256: Some(pin.sha256.clone()) });
    }
    manifest.validate()?;
    let document = registration_document(request, ComponentInventory {
        tools, codecs: Vec::new(), models: Vec::new(),
    })?;
    Ok(vec![
        artifact(FILE_NAMES[0], &configuration)?,
        artifact(FILE_NAMES[1], &manifest)?,
        artifact(FILE_NAMES[2], &request.preflight)?,
        artifact(FILE_NAMES[3], &document)?,
    ])
}

#[cfg(all(unix, not(target_os = "redox")))]
fn observe_adapter(request: &AudioInspectionRegistrationRequest) -> Result<ProviderImplementationIdentity> {
    use std::fs::{self, File, OpenOptions};
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use nix::fcntl::{OFlag, openat};
    use nix::sys::stat::Mode;

    let unavailable = |error: std::io::Error| invalid(format!("cannot observe prepared adapter: {error}"));
    let root_metadata = fs::symlink_metadata(&request.registration_directory).map_err(unavailable)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(invalid("registration directory must be an existing nonsymlink directory"));
    }
    let root = fs::canonicalize(&request.registration_directory).map_err(unavailable)?;
    if root != request.registration_directory {
        return Err(invalid("registration directory must have a canonical path without symlink ancestors"));
    }
    let mut directory = OpenOptions::new().read(true)
        .custom_flags(nix::libc::O_DIRECTORY | nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(&root).map_err(unavailable)?;
    let opened_root = directory.metadata().map_err(unavailable)?;
    if opened_root.dev() != root_metadata.dev() || opened_root.ino() != root_metadata.ino() {
        return Err(invalid("registration directory changed while opening"));
    }
    let parts: Vec<_> = request.adapter.relative_path.split('/').collect();
    for part in &parts[..parts.len() - 1] {
        directory = File::from(openat(&directory, *part,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty()).map_err(|error| invalid(format!("adapter parent cannot be safely opened: {error}")))?);
    }
    let mut file = File::from(openat(&directory, parts[parts.len() - 1],
        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::empty()).map_err(|error| invalid(format!("adapter cannot be safely opened: {error}")))?);
    let before = file.metadata().map_err(unavailable)?;
    if !before.is_file() || before.len() == 0 || before.len() > request.adapter.maximum_bytes
        || before.permissions().mode() & 0o111 == 0
    {
        return Err(invalid("adapter must be an executable nonempty regular file within its byte bound"));
    }
    let mut digest = Sha256::new();
    let mut observed_bytes = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let remaining = (before.len() + 1 - observed_bytes).min(buffer.len() as u64) as usize;
        let count = file.read(&mut buffer[..remaining]).map_err(unavailable)?;
        if count == 0 { break; }
        observed_bytes += count as u64;
        if observed_bytes > before.len() {
            return Err(invalid("adapter grew during bounded observation"));
        }
        digest.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(unavailable)?;
    let locator = root.join(&request.adapter.relative_path);
    let named = fs::symlink_metadata(&locator).map_err(unavailable)?;
    let current_root = fs::symlink_metadata(&root).map_err(unavailable)?;
    let same = |actual: &fs::Metadata| {
        actual.is_file() && !actual.file_type().is_symlink()
            && actual.dev() == before.dev() && actual.ino() == before.ino()
            && actual.len() == before.len() && actual.mode() == before.mode()
            && actual.mtime() == before.mtime() && actual.mtime_nsec() == before.mtime_nsec()
            && actual.ctime() == before.ctime() && actual.ctime_nsec() == before.ctime_nsec()
    };
    if observed_bytes != before.len() || !same(&after) || !same(&named)
        || !current_root.is_dir() || current_root.file_type().is_symlink()
        || current_root.dev() != opened_root.dev() || current_root.ino() != opened_root.ino()
        || fs::canonicalize(&locator).map_err(unavailable)? != locator
    {
        return Err(invalid("adapter or its confined locator changed during observation"));
    }
    Ok(ProviderImplementationIdentity {
        id: request.adapter.implementation_id.clone(),
        executable_sha256: format!("{:x}", digest.finalize()),
    })
}

#[cfg(not(all(unix, not(target_os = "redox"))))]
fn observe_adapter(_request: &AudioInspectionRegistrationRequest) -> Result<ProviderImplementationIdentity> {
    Err(invalid("safe adapter observation is unsupported on this host"))
}
