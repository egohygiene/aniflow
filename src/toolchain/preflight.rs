//! Explicit, read-only toolchain binding for selected native audio-inspection
//! stages. Supplied observations remain declarations; no probe is launched.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ToolchainDependencyKind, ToolchainInventory, ToolchainProfile, ToolchainReport, ToolchainStatus, inspect_profile};
use crate::audio_inspection::{
    AUDIO_INSPECTION_CAPABILITY_ID, AUDIO_INSPECTION_CAPABILITY_VERSION,
    AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V2, AUDIO_INSPECTION_PROVIDER_ID,
    AUDIO_INSPECTION_PROVIDER_VERSION, AudioInspectionProviderConfiguration, AudioToolPin,
};
use crate::{Error, ErrorCategory, PipelineV3Plan, ProviderRegistry, Result, SideEffect};

pub const TOOLCHAIN_PREFLIGHT_CONFIGURATION_SCHEMA: &str = "aniflow.toolchain.preflight/v1";
pub const TOOLCHAIN_PREFLIGHT_REPORT_SCHEMA: &str = "aniflow.toolchain.preflight-report/v1";
const MAXIMUM_CONFIGURATION_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainPreflightConfiguration {
    pub schema: String,
    pub profile: ToolchainProfile,
    pub inventory: ToolchainInventory,
    pub bindings: Vec<AudioInspectionToolchainBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInspectionToolchainBinding {
    pub stage_id: String,
    pub registration_id: String,
    pub capability_ids: Vec<String>,
    pub ffmpeg_dependency_id: String,
    pub ffprobe_dependency_id: String,
}

/// Fresh consistency evidence for the explicitly selected bindings only.
/// Deserializing this report never authorizes execution or skips fresh checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainPreflightReport {
    pub schema: String,
    pub configuration_sha256: String,
    pub plan_sha256: String,
    pub ready: bool,
    pub native_qualification: bool,
    /// One inspection over the union of selected capabilities shares the
    /// existing aggregate file-hashing bound across all stage bindings.
    pub inspections: Vec<ToolchainReport>,
    pub diagnostics: Vec<String>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

fn identifier(value: &str, field: &str) -> Result<()> {
    if value.len() > 128 {
        return Err(invalid(format!("{field} exceeds 128 bytes")));
    }
    crate::provider::require_token(value, field)
}

impl ToolchainPreflightConfiguration {
    /// Read a bounded nonsymlink JSON declaration without launching a process.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_json_slice(&super::types::read_json(path.as_ref())?)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        let value: Self = super::types::decode_unique_json(bytes)?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != TOOLCHAIN_PREFLIGHT_CONFIGURATION_SCHEMA {
            return Err(invalid("unsupported toolchain preflight configuration schema"));
        }
        self.profile.validate()?;
        self.inventory.validate()?;
        if self.bindings.is_empty() || self.bindings.len() > 128 {
            return Err(invalid("toolchain preflight requires 1..=128 explicit stage bindings"));
        }
        if self.inventory.artifacts.iter().any(|artifact| {
            !self.profile.dependencies.iter().any(|dependency| dependency.id == artifact.dependency_id)
        }) {
            return Err(invalid("preflight inventory references an unknown profile dependency"));
        }
        let mut stages = BTreeSet::new();
        for binding in &self.bindings {
            identifier(&binding.stage_id, "preflight stage ID")?;
            identifier(&binding.registration_id, "preflight registration ID")?;
            identifier(&binding.ffmpeg_dependency_id, "preflight FFmpeg dependency ID")?;
            identifier(&binding.ffprobe_dependency_id, "preflight ffprobe dependency ID")?;
            if !stages.insert(&binding.stage_id) {
                return Err(invalid("toolchain preflight stage bindings must be unique"));
            }
            if binding.ffmpeg_dependency_id == binding.ffprobe_dependency_id {
                return Err(invalid("FFmpeg and ffprobe require distinct dependency mappings"));
            }
            if binding.capability_ids.is_empty() || binding.capability_ids.len() > 64 {
                return Err(invalid("preflight bindings require 1..=64 explicit profile capabilities"));
            }
            let mut capabilities = BTreeSet::new();
            let mut dependencies = BTreeSet::new();
            for id in &binding.capability_ids {
                identifier(id, "preflight profile capability ID")?;
                if !capabilities.insert(id) {
                    return Err(invalid("duplicate capability in toolchain preflight binding"));
                }
                let capability = self.profile.capabilities.iter().find(|value| &value.id == id)
                    .ok_or_else(|| invalid(format!("unknown preflight profile capability {id}")))?;
                if capability.backend.is_some() || capability.scale.is_some()
                    || !capability.effective_settings.is_empty()
                {
                    return Err(invalid("audio-inspection preflight does not support profile backend, scale or effective settings"));
                }
                if capability.side_effects.iter().any(|effect| !matches!(effect,
                    SideEffect::FilesystemRead | SideEffect::FilesystemWrite
                        | SideEffect::EnvironmentRead | SideEffect::Subprocess))
                {
                    return Err(invalid("audio-inspection preflight supports only file read/write, environment read and subprocess effects"));
                }
                dependencies.extend(capability.dependency_ids.iter().map(String::as_str));
            }
            let expected = BTreeSet::from([
                binding.ffmpeg_dependency_id.as_str(), binding.ffprobe_dependency_id.as_str(),
            ]);
            if dependencies != expected {
                return Err(invalid("selected profile dependencies must exactly equal the two mapped audio tools"));
            }
            for id in expected {
                if !self.profile.dependencies.iter().any(|dependency| dependency.id == id
                    && dependency.kind == ToolchainDependencyKind::Tool)
                {
                    return Err(invalid("audio-inspection preflight mappings require declared tool dependencies"));
                }
            }
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAXIMUM_CONFIGURATION_BYTES {
            return Err(invalid("toolchain preflight configuration exceeds one MiB"));
        }
        Ok(bytes)
    }

    pub fn configuration_sha256(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(self.canonical_json_bytes()?)))
    }
}

/// Reinspect selected local dependencies and compare them with the exact
/// planned native audio-inspection configurations and tool components. This
/// neither selects a provider nor executes any tool, adapter or media input.
pub fn preflight_toolchain(
    plan: &PipelineV3Plan,
    registry: &ProviderRegistry,
    configuration: &ToolchainPreflightConfiguration,
) -> Result<ToolchainPreflightReport> {
    let configuration_sha256 = configuration.configuration_sha256()?;
    plan.validate().map_err(|error| invalid(format!("invalid preflight plan: {error}")))?;
    // Check every mapping before inspecting files. A nested validator or an
    // unsupported adapter cannot be mistaken for a supported main stage.
    for binding in &configuration.bindings {
        let stage = plan.payload.stages.iter().find(|stage| stage.id == binding.stage_id)
            .ok_or_else(|| invalid(format!("unknown main preflight stage {}", binding.stage_id)))?;
        let lock = &stage.provider_lock.payload;
        if lock.registration_id != binding.registration_id {
            return Err(invalid(format!("stage {} does not select preflight registration {}", binding.stage_id, binding.registration_id)));
        }
        if lock.provider.id != AUDIO_INSPECTION_PROVIDER_ID
            || lock.provider.version != AUDIO_INSPECTION_PROVIDER_VERSION
            || lock.capability.id != AUDIO_INSPECTION_CAPABILITY_ID
            || lock.capability.version != AUDIO_INSPECTION_CAPABILITY_VERSION
        {
            return Err(invalid(format!("stage {} is not the supported native audio-inspection provider", binding.stage_id)));
        }
    }
    let mut diagnostics = Vec::new();
    if plan.payload.toolchain_preflight_sha256.as_ref()
        .is_some_and(|expected| expected != &configuration_sha256)
    {
        diagnostics.push("supplied toolchain declaration differs from the existing plan preflight guard".to_owned());
    }
    if let Err(error) = crate::run_v3::resolve_exact_providers(plan, registry) {
        diagnostics.push(format!("current explicit provider locks are unavailable or changed: {error}"));
    }
    let platform = &configuration.inventory.platform;
    if platform.os != std::env::consts::OS || platform.arch != std::env::consts::ARCH {
        diagnostics.push(format!(
            "inventory platform {}/{} differs from current host {}/{}",
            platform.os, platform.arch, std::env::consts::OS, std::env::consts::ARCH,
        ));
    }
    let selected: BTreeSet<_> = configuration.bindings.iter()
        .flat_map(|binding| binding.capability_ids.iter().cloned()).collect();
    let inspection = inspect_profile(&configuration.profile, &configuration.inventory,
        &selected.into_iter().collect::<Vec<_>>())?;
    if !inspection.ready {
        let failures: Vec<_> = inspection.facts.iter()
            .filter(|fact| fact.status != ToolchainStatus::Installed).collect();
        for fact in failures.iter().take(8) {
            diagnostics.push(format!("selected capability {} {}: {}", fact.capability_id,
                fact.check, diagnostic_excerpt(&fact.detail)));
        }
        if failures.len() > 8 {
            diagnostics.push(format!("{} additional inventory failures are retained in the inspection facts", failures.len() - 8));
        }
    }
    for binding in &configuration.bindings {
        let stage = plan.payload.stages.iter().find(|stage| stage.id == binding.stage_id)
            .expect("main stage checked above");
        let Some(registration) = registry.registration(&binding.registration_id) else {
            diagnostics.push(format!("stage {} registration {} is not explicitly supplied", binding.stage_id, binding.registration_id));
            continue;
        };
        let values = serde_json::to_value(&registration.configuration().values)
            .map_err(|_| invalid("cannot encode audio-inspection provider values"))?;
        let typed: AudioInspectionProviderConfiguration = serde_json::from_value(values)
            .map_err(|error| invalid(format!("stage {} has invalid closed audio-inspection settings: {error}", binding.stage_id)))?;
        typed.validate()?;
        if typed.schema != AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V2
            || &typed.provider_configuration()? != registration.configuration()
        {
            return Err(invalid(format!("stage {} does not contain the exact native audio-inspection provider configuration", binding.stage_id)));
        }
        let effects: BTreeSet<_> = binding.capability_ids.iter().flat_map(|id| {
            configuration.profile.capabilities.iter().find(|capability| &capability.id == id)
                .expect("profile capability validated").side_effects.iter().copied()
        }).collect();
        let planned_effects: BTreeSet<_> = stage.capability.behavior.side_effects.iter().copied().collect();
        let locked_effects: BTreeSet<_> = stage.provider_lock.payload.authorized_side_effects.iter().copied().collect();
        if effects != planned_effects || effects != locked_effects {
            diagnostics.push(format!("stage {} profile side effects do not exactly match the planned adapter and lock", binding.stage_id));
        }
        for (dependency_id, component_id, pin) in [
            (&binding.ffmpeg_dependency_id, "ffmpeg", &typed.settings.ffmpeg),
            (&binding.ffprobe_dependency_id, "ffprobe", &typed.settings.ffprobe),
        ] {
            check_tool_binding(configuration, &binding.stage_id, dependency_id, component_id, pin,
                &stage.provider_lock.payload.tools, &mut diagnostics);
        }
    }
    Ok(ToolchainPreflightReport {
        schema: TOOLCHAIN_PREFLIGHT_REPORT_SCHEMA.to_owned(),
        configuration_sha256, plan_sha256: plan.plan_sha256.clone(),
        ready: diagnostics.is_empty() && inspection.ready,
        native_qualification: false,
        inspections: vec![inspection], diagnostics,
    })
}

fn diagnostic_excerpt(detail: &str) -> String {
    // Full facts remain in the inspection report; flattened refusal messages
    // only need a bounded actionable excerpt and never terminal controls.
    let mut excerpt = String::new();
    for character in detail.chars() {
        let character = if character.is_control() { ' ' } else { character };
        if excerpt.len() + character.len_utf8() > 2045 {
            excerpt.push_str("...");
            break;
        }
        excerpt.push(character);
    }
    excerpt
}

fn check_tool_binding(
    configuration: &ToolchainPreflightConfiguration,
    stage_id: &str,
    dependency_id: &str,
    component_id: &str,
    pin: &AudioToolPin,
    locked_tools: &[crate::ComponentIdentity],
    diagnostics: &mut Vec<String>,
) {
    let Some(artifact) = configuration.inventory.artifacts.iter()
        .find(|artifact| artifact.dependency_id == dependency_id) else {
        diagnostics.push(format!("stage {stage_id} requires explicit inventory dependency {dependency_id}"));
        return;
    };
    if artifact.path != pin.executable || artifact.expected_sha256 != pin.sha256 {
        diagnostics.push(format!("stage {stage_id} {component_id} path/digest differs between inventory and adapter settings"));
    }
    if !artifact.observation.as_ref().is_some_and(|observation|
        observation.executable_sha256 == pin.sha256 && observation.version.as_deref() == Some(pin.version.as_str()))
    {
        diagnostics.push(format!("stage {stage_id} {component_id} requires matching digest-bound inventory version evidence"));
    }
    if !locked_tools.iter().any(|component| component.id == component_id
        && component.version == pin.version && component.sha256.as_deref() == Some(pin.sha256.as_str()))
    {
        diagnostics.push(format!("stage {stage_id} {component_id} is absent from the actual planned lock or has a different version/digest"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if !std::fs::symlink_metadata(&artifact.path).is_ok_and(|metadata|
            metadata.file_type().is_file() && metadata.permissions().mode() & 0o111 != 0)
        {
            diagnostics.push(format!("stage {stage_id} {component_id} is not a nonsymlink executable regular file"));
        }
    }
}

/// Enforce the optional plan guard on every run/resume. A retained report is
/// never accepted in place of the exact declaration and fresh local checks.
pub(crate) fn require_toolchain_preflight(
    plan: &PipelineV3Plan,
    registry: &ProviderRegistry,
    configuration: Option<&ToolchainPreflightConfiguration>,
) -> Result<()> {
    match (&plan.payload.toolchain_preflight_sha256, configuration) {
        (None, None) => Ok(()),
        (Some(_), None) => Err(invalid("guarded plan requires its exact toolchain preflight configuration")),
        (None, Some(_)) => Err(invalid("toolchain preflight configuration cannot be supplied to an unguarded plan; bind it explicitly first")),
        (Some(expected), Some(configuration)) => {
            if &configuration.configuration_sha256()? != expected {
                return Err(invalid("toolchain preflight configuration differs from the plan guard"));
            }
            let report = preflight_toolchain(plan, registry, configuration)?;
            if !report.ready {
                return Err(Error::new(ErrorCategory::Dependency,
                    format!("toolchain preflight refused execution: {}", report.diagnostics.join("; "))));
            }
            Ok(())
        }
    }
}
