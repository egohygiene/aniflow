//! Explicit, serial RIFE frame interpolation. Resource estimates are admission
//! policy, never measured memory or an enforced GPU quota.

mod provider;
mod types;

use std::collections::BTreeSet;
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use crate::provider::{canonical_sha256, decode_json, require_sha256};
use crate::temporal::TEMPORAL_INSPECTION_SCHEMA_V1;
use crate::{Error, ErrorCategory, Result};

pub use provider::execute_invocation;
pub use types::*;

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Input, message)
}

fn temporal_error(error: anyhow::Error) -> Error {
    Error::from_anyhow(ErrorCategory::Media, error)
}

fn normalized_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.to_str().is_some_and(|value| {
            !value.chars().any(char::is_control)
                && !value.split('/').any(|part| matches!(part, "." | ".."))
        })
        && !path.components().any(|part| matches!(part, Component::CurDir | Component::ParentDir))
}

impl RifeConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 {
            return Err(invalid("RIFE configuration exceeds 64 KiB"));
        }
        let value: Self = decode_json(bytes, "RIFE configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != RIFE_CONFIGURATION_SCHEMA_V1 {
            return Err(invalid("unsupported RIFE configuration schema"));
        }
        // Reuse the existing exact executable/version/digest path contract.
        crate::audio_inspection::AudioInspectionConfiguration {
            schema: crate::audio_inspection::AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.to_owned(),
            ffmpeg: self.settings.tool.clone(),
            ffprobe: self.settings.tool.clone(),
            tool_timeout_milliseconds: self.settings.tool_timeout_milliseconds,
            maximum_tool_output_bytes: self.settings.maximum_tool_output_bytes,
        }.validate()?;
        let model = &self.settings.model;
        if !normalized_absolute(&model.directory)
            || model.directory.file_name().and_then(|value| value.to_str()) != Some("rife-v4.6")
            || !(1..=16_777_216).contains(&model.param_bytes)
            || !(1..=268_435_456).contains(&model.bin_bytes)
            || !(-1..=31).contains(&self.settings.device_id)
            || !(1024..=67_108_864).contains(&self.settings.maximum_frame_bytes)
            || !(1_048_576..=1_099_511_627_776).contains(&self.settings.maximum_artifact_bytes)
        {
            return Err(invalid("unsupported RIFE model/device/artifact bounds"));
        }
        require_sha256(&model.param_sha256, "RIFE param SHA-256")?;
        require_sha256(&model.bin_sha256, "RIFE bin SHA-256")?;
        if self.enabled
            && (!matches!(self.host.operating_system.as_str(), "macos" | "linux")
                || !matches!(self.host.architecture.as_str(), "aarch64" | "x86_64")
                || self.host.total_memory_mib < 4096
                || self.host.available_memory_mib == 0
                || self.host.available_memory_mib > self.host.total_memory_mib)
        {
            return Err(invalid("enabled RIFE requires explicit supported host and memory observations"));
        }
        Ok(())
    }
}

impl RifeFrameIndex {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() as u64 > RIFE_MAXIMUM_INDEX_BYTES {
            return Err(invalid("RIFE frame index exceeds 16 MiB"));
        }
        let value: Self = decode_json(bytes, "RIFE frame index")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != RIFE_FRAME_INDEX_SCHEMA_V1
            || self.temporal_inspection.schema != TEMPORAL_INSPECTION_SCHEMA_V1
            || self.frame_profile != "rgb8_sdr_bt709_square_unrotated"
            || self.width == 0 || self.height == 0
            || self.frames.is_empty() || self.frames.len() as u64 > RIFE_MAXIMUM_FRAMES
        {
            return Err(invalid("unsupported RIFE extracted frame profile"));
        }
        require_sha256(&self.temporal_inspection.source_sha256, "source SHA-256")?;
        self.temporal_inspection.require_processing().map_err(temporal_error)?;
        let timeline = self.temporal_inspection.video_timeline().map_err(temporal_error)?;
        let stream = self.temporal_inspection.stream(timeline.stream_index)
            .ok_or_else(|| invalid("missing RIFE selected stream"))?;
        if stream.width != Some(u64::from(self.width))
            || stream.height != Some(u64::from(self.height))
            || timeline.frames.len() != self.frames.len()
        {
            return Err(invalid("frame geometry/cardinality does not match temporal inspection"));
        }
        let mut names = BTreeSet::new();
        for (ordinal, frame) in self.frames.iter().enumerate() {
            if frame.ordinal != ordinal as u64
                || frame.filename.is_empty() || frame.filename.len() > 128
                || !frame.filename.ends_with(".png")
                || !frame.filename.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
                || frame.filename.starts_with('.')
                || !names.insert(&frame.filename)
            {
                return Err(invalid("frame index requires ordered unique portable PNG basenames"));
            }
            require_sha256(&frame.sha256, "frame SHA-256")?;
        }
        if self.cut_before_ordinals.iter().any(|ordinal| *ordinal == 0 || *ordinal >= self.frames.len() as u64)
            || self.cut_before_ordinals.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid("cut boundaries must be sorted unique right-frame ordinals within the source"));
        }
        Ok(())
    }
}

/// Read-only admission and exact clock plan. Does not inspect tools, reserve
/// memory, infer cuts, create directories, launch RIFE or authorize execution.
pub fn plan(configuration: &RifeConfiguration, index: &RifeFrameIndex) -> Result<RifePlan> {
    configuration.validate()?;
    index.validate()?;
    let timeline = index.temporal_inspection.video_timeline().map_err(temporal_error)?;
    let source_frame_rate = timeline.frame_period.ok_or_else(|| invalid("missing CFR frame period"))?
        .reciprocal().map_err(temporal_error)?;
    let source_frame_count = index.frames.len() as u64;
    let duration = source_frame_rate.reciprocal()
        .and_then(|period| period.checked_mul(source_frame_count))
        .map_err(temporal_error)?;
    let (output_frame_count, output_frame_rate, resources) = if configuration.enabled {
        (source_frame_count.checked_mul(2).ok_or_else(|| invalid("output count overflow"))?,
         source_frame_rate.checked_mul(2).map_err(temporal_error)?,
         Some(resolve_resources(configuration, index.width, index.height)?))
    } else {
        (source_frame_count, source_frame_rate, None)
    };
    Ok(RifePlan {
        schema: RIFE_PLAN_SCHEMA_V1.to_owned(),
        enabled: configuration.enabled,
        configuration_sha256: canonical_sha256(configuration)?,
        frame_index_sha256: canonical_sha256(index)?,
        source_sha256: index.temporal_inspection.source_sha256.clone(),
        source_frame_count,
        output_frame_count,
        source_frame_rate,
        output_frame_rate,
        duration,
        cut_before_ordinals: index.cut_before_ordinals.clone(),
        resources,
        native_qualified: false,
        audio_policy: "not_processed; preserve_selected_audio_at_assembly".to_owned(),
    })
}

fn resolve_resources(configuration: &RifeConfiguration, width: u32, height: u32) -> Result<RifeResources> {
    let host = &configuration.host;
    let preset = match configuration.settings.preset {
        RifePreset::Auto if host.operating_system == "linux"
            && host.architecture == "x86_64"
            && host.total_memory_mib >= 32_768
            && host.available_memory_mib >= 16_384 => RifePreset::Balanced,
        RifePreset::Auto => RifePreset::Conservative,
        value => value,
    };
    if preset == RifePreset::Balanced && host.total_memory_mib < 32_768 {
        return Err(invalid("balanced RIFE requires at least 32 GiB observed RAM; use conservative before upscale"));
    }
    let (maximum_padded_pixels, preset_budget_mib) = match preset {
        RifePreset::Conservative => (2_100_000_u64, 2048_u64),
        RifePreset::Balanced => (8_400_000_u64, 8192_u64),
        RifePreset::Auto => unreachable!("auto normalized above"),
    };
    let padded_width = u64::from(width).div_ceil(32) * 32;
    let padded_height = u64::from(height).div_ceil(32) * 32;
    let pixels = padded_width.checked_mul(padded_height).ok_or_else(|| invalid("padded pixel overflow"))?;
    if pixels > maximum_padded_pixels {
        return Err(invalid("RIFE padded frame exceeds preset cap; interpolate before 4K upscale or select a qualified larger profile"));
    }
    let estimated_working_memory_mib = pixels.checked_mul(512)
        .and_then(|bytes| bytes.checked_add(512 * 1_048_576))
        .ok_or_else(|| invalid("memory estimate overflow"))?.div_ceil(1_048_576);
    let working_memory_budget_mib = (host.total_memory_mib / 4)
        .min(host.available_memory_mib / 3).min(preset_budget_mib);
    if estimated_working_memory_mib > working_memory_budget_mib {
        return Err(invalid("RIFE heuristic working set exceeds observed memory headroom; free memory or reduce explicit input geometry"));
    }
    Ok(RifeResources {
        preset,
        maximum_active_pairs: 1,
        threads: "1:1:1".to_owned(),
        padded_width,
        padded_height,
        maximum_padded_pixels,
        estimated_working_memory_mib,
        working_memory_budget_mib,
        memory_estimate_method: "uncalibrated:512MiB+512B_per_padded_pixel/v1".to_owned(),
        memory_limit_enforced: false,
    })
}

/// Compact mapping avoids allocating a second whole-video frame list in plan.
pub fn output_frame(plan: &RifePlan, ordinal: u64) -> Result<(u64, u64, RifeFrameOrigin)> {
    validate_mapping_plan(plan)?;
    mapped_output_frame(plan, ordinal)
}

pub(super) fn validate_mapping_plan(plan: &RifePlan) -> Result<()> {
    if plan.schema != RIFE_PLAN_SCHEMA_V1 || !plan.enabled
        || plan.source_frame_count == 0 || plan.source_frame_count > RIFE_MAXIMUM_FRAMES
        || plan.output_frame_count != plan.source_frame_count * 2
        || plan.cut_before_ordinals.iter().any(|cut| *cut == 0 || *cut >= plan.source_frame_count)
        || plan.cut_before_ordinals.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("output mapping requires an enabled plan and valid ordinal"));
    }
    Ok(())
}

/// Internal iteration after one complete plan validation, with logarithmic cut lookup.
pub(super) fn mapped_output_frame(plan: &RifePlan, ordinal: u64) -> Result<(u64, u64, RifeFrameOrigin)> {
    if ordinal >= plan.output_frame_count {
        return Err(invalid("output mapping ordinal is outside the admitted plan"));
    }
    let left = ordinal / 2;
    if ordinal % 2 == 0 {
        return Ok((left, left, RifeFrameOrigin::Original));
    }
    let right = left + 1;
    if right == plan.source_frame_count {
        Ok((left, left, RifeFrameOrigin::EndpointHold))
    } else if plan.cut_before_ordinals.binary_search(&right).is_ok() {
        Ok((left, left, RifeFrameOrigin::CutHold))
    } else {
        Ok((left, right, RifeFrameOrigin::InterpolatedMidpoint))
    }
}

impl RifeOutputIndex {
    /// Parse a bounded candidate. Use validate_against before trusting its claims.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() as u64 > RIFE_MAXIMUM_INDEX_BYTES {
            return Err(invalid("RIFE output index exceeds 16 MiB"));
        }
        decode_json(bytes, "RIFE output index")
    }

    /// Recompute semantic evidence from the exact configuration/source index.
    /// This does not decode output PNGs or establish native/perceptual quality.
    pub fn validate_against(&self, configuration: &RifeConfiguration, index: &RifeFrameIndex) -> Result<()> {
        let expected = plan(configuration, index)?;
        if self.schema != RIFE_OUTPUT_INDEX_SCHEMA_V1 || !expected.enabled
            || self.plan != expected || self.frames.len() as u64 != expected.output_frame_count
        {
            return Err(invalid("RIFE output index is incompatible with the exact admitted plan"));
        }
        for (ordinal, frame) in self.frames.iter().enumerate() {
            let ordinal = ordinal as u64;
            let (left, right, origin) = mapped_output_frame(&expected, ordinal)?;
            if frame.ordinal != ordinal || frame.filename != format!("frame_{ordinal:08}.png")
                || frame.source_left_ordinal != left || frame.source_right_ordinal != right
                || frame.origin != origin
            {
                return Err(invalid("RIFE output ordinal/source-pair mapping differs from the plan"));
            }
            require_sha256(&frame.sha256, "RIFE output frame SHA-256")?;
            if origin != RifeFrameOrigin::InterpolatedMidpoint
                && frame.sha256 != index.frames[left as usize].sha256
            {
                return Err(invalid("RIFE copied/held frame differs from its original digest"));
            }
        }
        Ok(())
    }
}

pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference {
        id: RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
        version: "1.0.0".to_owned(),
        sha256: format!("{:x}", Sha256::digest(include_bytes!("../../providers/rife/provider-configuration.schema.json"))),
    }
}

impl RifeProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        if self.schema != RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1 || !self.configuration.enabled {
            return Err(invalid("RIFE invocation requires the enabled v1 provider profile"));
        }
        self.configuration.validate()
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let encoded = serde_json::to_value(self).map_err(|error| invalid(error.to_string()))?;
        let values = encoded.as_object().ok_or_else(|| invalid("RIFE provider values must be an object"))?
            .iter().map(|(key, value)| (key.clone(), value.clone())).collect();
        crate::ProviderConfiguration::new(
            crate::ProviderReference { id: RIFE_PROVIDER_ID.to_owned(), version: "1.0.0".to_owned() },
            crate::CapabilityReference { id: RIFE_CAPABILITY_ID.to_owned(), version: "1.0.0".to_owned() },
            configuration_schema_reference(), values,
        )
    }

    /// Generate an exact inert manifest; registering/launching remains separate.
    pub fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let gpu = self.configuration.settings.device_id >= 0;
        let schema = configuration_schema_reference();
        let mut effects = vec!["filesystem_read", "filesystem_write", "environment_read", "subprocess", "ai"];
        if gpu { effects.push("gpu"); }
        let value = serde_json::json!({
            "schema": "aniflow.provider-manifest/v1",
            "provider": {"id": RIFE_PROVIDER_ID, "version": "1.0.0", "display_name": "aniflow serial RIFE frame interpolation"},
            "configuration_schemas": [schema.clone()],
            "capabilities": [{
                "id": RIFE_CAPABILITY_ID, "version": "1.0.0", "kind": "frame_processor",
                "configuration_schema": schema,
                "inputs": [
                    {"name":"frames", "artifact_type":"application/vnd.aniflow.rife-frames", "artifact_role":"intermediate", "stream_role":"video", "cardinality":"one", "immutable":true},
                    {"name":"index", "artifact_type":"application/vnd.aniflow.rife-frame-index+json", "artifact_role":"validation_evidence", "stream_role":"timed_metadata", "cardinality":"one", "immutable":true}
                ],
                "outputs": [
                    {"name":"frames", "artifact_type":"application/vnd.aniflow.rife-frames", "artifact_role":"intermediate", "stream_role":"video", "cardinality":"one", "immutable":true},
                    {"name":"index", "artifact_type":"application/vnd.aniflow.rife-output-index+json", "artifact_role":"validation_evidence", "stream_role":"timed_metadata", "cardinality":"one", "immutable":true}
                ],
                "batching":"whole_artifact", "max_concurrency":1,
                "requirements": {
                    "tools":[{"id":"rife-ncnn-vulkan", "version_requirement":format!("={}", self.configuration.settings.tool.version), "sha256":self.configuration.settings.tool.sha256}],
                    "codecs":[], "models":[
                        {"id":"rife-v4.6-param", "version_requirement":"=4.6.0", "sha256":self.configuration.settings.model.param_sha256},
                        {"id":"rife-v4.6-bin", "version_requirement":"=4.6.0", "sha256":self.configuration.settings.model.bin_sha256}
                    ],
                    "compute":{"minimum_cpu_threads":1, "minimum_memory_mib":4096, "minimum_storage_mib":0, "gpu":if gpu {"required"} else {"forbidden"}, "network":"forbidden"}
                },
                "behavior":{"determinism":"environment_dependent", "fidelity":"lossy", "cacheable":false, "content_changes":true, "side_effects":effects},
                "lifecycle":{"progress":"none", "cancellation":"process_signal"}
            }],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}
        });
        crate::ProviderManifest::from_json_slice(&serde_json::to_vec(&value).map_err(|error| invalid(error.to_string()))?)
    }
}
