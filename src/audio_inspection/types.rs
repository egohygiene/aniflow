//! Closed contracts for the bounded PCM16 inspection adapter.
use std::path::{Component, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audio_analysis::{AudioArtifactReference, AudioRationalTime};
use crate::provider::{decode_json, require_sha256};
use crate::{Error, ErrorCategory, ProviderReference, Result};

pub const AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-inspection.configuration/v1";
pub const AUDIO_TECHNICAL_INSPECTION_SCHEMA_V1: &str = "aniflow.audio-technical-inspection/v1";
pub const AUDIO_INSPECTION_PREFLIGHT_SCHEMA_V1: &str = "aniflow.audio-inspection-preflight/v1";
pub const AUDIO_INSPECTION_PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-inspection";
pub const AUDIO_INSPECTION_PROVIDER_VERSION: &str = "1.0.0";
pub const AUDIO_INSPECTION_CAPABILITY_ID: &str = "aniflow/audio-technical-inspection";
pub const AUDIO_INSPECTION_MAXIMUM_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioToolPin {
    pub executable: PathBuf,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInspectionConfiguration {
    pub schema: String,
    pub ffmpeg: AudioToolPin,
    pub ffprobe: AudioToolPin,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}

impl AudioInspectionConfiguration {
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        if input.len() > 65_536 {
            return Err(invalid("inspection configuration exceeds 64 KiB"));
        }
        let value: Self = decode_json(input, "audio inspection configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1 {
            return Err(invalid("unsupported audio inspection configuration schema"));
        }
        if !(1..=120_000).contains(&self.tool_timeout_milliseconds)
            || !(1024..=1_048_576).contains(&self.maximum_tool_output_bytes)
        {
            return Err(invalid(
                "inspection tool timeout/output bounds are unsupported",
            ));
        }
        for tool in [&self.ffmpeg, &self.ffprobe] {
            if !tool.executable.is_absolute()
                || tool
                    .executable
                    .components()
                    .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
                || tool.executable.to_str().is_none_or(|path| {
                    path.chars().any(char::is_control)
                        || path.split('/').any(|part| part == "." || part == "..")
                })
            {
                return Err(invalid(
                    "tool executable must be an absolute normalized UTF-8 path",
                ));
            }
            if tool.version.is_empty()
                || tool.version.len() > 256
                || tool
                    .version
                    .chars()
                    .any(|value| value.is_control() || value.is_whitespace())
            {
                return Err(invalid(
                    "tool version must be one exact bounded version token",
                ));
            }
            crate::provider::validate_semantic_version(&tool.version, "tool version")?;
            require_sha256(&tool.sha256, "tool sha256")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioInspectionDiagnosticCode {
    MissingTool,
    InvalidTool,
    ToolDigestMismatch,
    ToolVersionMismatch,
    ToolTimeout,
    ToolOutputLimit,
    ToolFailed,
    Cancelled,
    UnsupportedPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInspectionDiagnostic {
    pub code: AudioInspectionDiagnosticCode,
    pub tool: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioToolObservation {
    pub id: String,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInspectionPreflight {
    pub schema: String,
    pub ready: bool,
    pub tools: Vec<AudioToolObservation>,
    pub diagnostics: Vec<AudioInspectionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTechnicalCommandEvidence {
    pub tool: String,
    /// Exact argument order, with the private source path replaced by {snapshot}.
    pub arguments: Vec<String>,
}

/// Successful deterministic inspection of the closed PCM16 RIFF/WAV profile.
/// Presence never substitutes for reading this artifact through its validation API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTechnicalInspection {
    pub schema: String,
    pub source: AudioArtifactReference,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
    pub tools: Vec<AudioToolObservation>,
    pub container: String,
    pub codec: String,
    pub sample_format: String,
    pub stream_index: u32,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frame_count: u64,
    pub duration: AudioRationalTime,
    pub pcm_bitrate_bits_per_second: u64,
    pub pcm_sha256: String,
    pub decoded_pcm_sha256: String,
    pub decode_complete: bool,
    pub source_unchanged: bool,
    pub commands: Vec<AudioTechnicalCommandEvidence>,
}

impl AudioTechnicalInspection {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 1_048_576 {
            return Err(invalid("technical report exceeds 1 MiB"));
        }
        let report: Self = decode_json(bytes, "audio technical inspection")?;
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_TECHNICAL_INSPECTION_SCHEMA_V1
            || self.provider.id != AUDIO_INSPECTION_PROVIDER_ID
            || self.provider.version != AUDIO_INSPECTION_PROVIDER_VERSION
            || self.container != "wav"
            || self.codec != "pcm_s16le"
            || self.sample_format != "s16"
            || self.stream_index != 0
            || !(8000..=192000).contains(&self.sample_rate_hz)
            || !matches!(self.channels, 1 | 2)
            || self.frame_count == 0
            || self.frame_count > u64::from(self.sample_rate_hz) * 600
            || self.source.byte_size < 44
            || self.source.byte_size > AUDIO_INSPECTION_MAXIMUM_BYTES
            || !self.decode_complete
            || !self.source_unchanged
            || self.pcm_sha256 != self.decoded_pcm_sha256
        {
            return Err(invalid(
                "technical evidence does not satisfy the complete PCM16 inspection profile",
            ));
        }
        let minimum_bytes = self
            .frame_count
            .checked_mul(u64::from(self.channels))
            .and_then(|value| value.checked_mul(2))
            .and_then(|value| value.checked_add(44))
            .ok_or_else(|| invalid("technical PCM byte count overflows"))?;
        if self.source.byte_size < minimum_bytes {
            return Err(invalid(
                "technical source is too short for its declared PCM frame count",
            ));
        }
        let source = crate::audio_analysis::AudioSource {
            artifact: self.source.clone(),
            stream_index: 0,
            sample_rate_hz: self.sample_rate_hz,
            channels: self.channels,
            frame_count: self.frame_count,
            origin: AudioRationalTime {
                numerator: 0,
                denominator: 1,
            },
            stem: None,
        };
        source.validate()?;
        if self.duration != source.time_for_frame(self.frame_count)?
            || self.pcm_bitrate_bits_per_second
                != u64::from(self.sample_rate_hz) * u64::from(self.channels) * 16
        {
            return Err(invalid("technical duration or PCM bitrate is inconsistent"));
        }
        for digest in [
            &self.implementation_sha256,
            &self.configuration_sha256,
            &self.provider_lock_sha256,
            &self.pcm_sha256,
            &self.decoded_pcm_sha256,
        ] {
            require_sha256(digest, "technical identity sha256")?;
        }
        if self.tools.len() != 2 || self.tools[0].id != "ffmpeg" || self.tools[1].id != "ffprobe" {
            return Err(invalid(
                "technical evidence requires exact ffmpeg and ffprobe identities",
            ));
        }
        for tool in &self.tools {
            require_sha256(&tool.sha256, "observed tool sha256")?;
            crate::provider::validate_semantic_version(&tool.version, "observed tool version")?;
            if tool.version.is_empty()
                || tool.version.len() > 256
                || tool
                    .version
                    .chars()
                    .any(|value| value.is_control() || value.is_whitespace())
            {
                return Err(invalid("observed tool version is invalid"));
            }
        }
        if self.commands != super::provider::command_evidence() {
            return Err(invalid(
                "technical command evidence does not match the supported profile",
            ));
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        crate::provider::canonical_json_bytes(self)
    }
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

pub const AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-inspection.provider-configuration/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioInspectionProviderConfiguration {
    pub schema: String,
    pub settings: AudioInspectionConfiguration,
    pub source: AudioArtifactReference,
}

impl AudioInspectionProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        if self.schema != AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V1
            || self.source.id != "source_audio"
            || !(44..=AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&self.source.byte_size)
        {
            return Err(invalid(
                "invalid source-bound inspection provider configuration",
            ));
        }
        require_sha256(&self.source.sha256, "source sha256")
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self)
            .map_err(|_| invalid("cannot encode inspection configuration"))?;
        let values = values
            .as_object()
            .expect("configuration struct is object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_INSPECTION_PROVIDER_ID.to_owned(),
                version: AUDIO_INSPECTION_PROVIDER_VERSION.to_owned(),
            },
            crate::CapabilityReference {
                id: AUDIO_INSPECTION_CAPABILITY_ID.to_owned(),
                version: "1.0.0".to_owned(),
            },
            configuration_schema_reference(),
            values,
        )
    }
}

#[must_use]
pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    use sha2::{Digest, Sha256};
    crate::ConfigurationSchemaReference {
        id: AUDIO_INSPECTION_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
        version: "1.0.0".to_owned(),
        sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../providers/audio-inspection/provider-configuration.schema.json"
            ))
        ),
    }
}

impl AudioInspectionPreflight {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready && self.diagnostics.is_empty() && self.tools.len() == 2
    }
}
