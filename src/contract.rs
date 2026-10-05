use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{Error, ErrorCategory, Result};

pub const MACHINE_SCHEMA_VERSION: u32 = 1;

/// Public CLI operations represented by the machine contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CommandName {
    Doctor,
    ToolchainDoctor,
    ToolchainPlan,
    ToolchainProbe,
    ToolchainPrepareRegistration,
    Inspect,
    TemporalInspect,
    AudioPlan,
    AudioInspect,
    AudioAnalyze,
    AudioResume,
    AudioTranscribe,
    AudioTranscriptExport,
    AudioLyricsAlign,
    AudioLyricsExport,
    AudioMidiExtract,
    AudioMidiExport,
    TimedTextFormats,
    TimedTextConvert,
    Plan,
    PlanV3,
    Run,
    RunV3,
    Resume,
    ResumeV3,
    Status,
    StatusV3,
    CacheInspect,
    CacheInvalidate,
    CachePrune,
    SegmentPlan,
    SegmentRun,
    SegmentResume,
    SegmentReconstruct,
}

impl fmt::Display for CommandName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Doctor => "doctor",
            Self::ToolchainDoctor => "toolchain_doctor",
            Self::ToolchainPlan => "toolchain_plan",
            Self::ToolchainProbe => "toolchain_probe",
            Self::ToolchainPrepareRegistration => "toolchain_prepare_registration",
            Self::Inspect => "inspect",
            Self::TemporalInspect => "temporal_inspect",
            Self::AudioPlan => "audio_plan",
            Self::AudioInspect => "audio_inspect",
            Self::AudioAnalyze => "audio_analyze",
            Self::AudioResume => "audio_resume",
            Self::AudioTranscribe => "audio_transcribe",
            Self::AudioTranscriptExport => "audio_transcript_export",
            Self::AudioLyricsAlign => "audio_lyrics_align",
            Self::AudioLyricsExport => "audio_lyrics_export",
            Self::AudioMidiExtract => "audio_midi_extract",
            Self::AudioMidiExport => "audio_midi_export",
            Self::TimedTextFormats => "timed_text_formats",
            Self::TimedTextConvert => "timed_text_convert",
            Self::Plan => "plan",
            Self::PlanV3 => "plan_v3",
            Self::Run => "run",
            Self::RunV3 => "run_v3",
            Self::Resume => "resume",
            Self::ResumeV3 => "resume_v3",
            Self::Status => "status",
            Self::StatusV3 => "status_v3",
            Self::CacheInspect => "cache_inspect",
            Self::CacheInvalidate => "cache_invalidate",
            Self::CachePrune => "cache_prune",
            Self::SegmentPlan => "segment_plan",
            Self::SegmentRun => "segment_run",
            Self::SegmentResume => "segment_resume",
            Self::SegmentReconstruct => "segment_reconstruct",
        };
        formatter.write_str(name)
    }
}

/// Serializable representation of a typed public error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ErrorReport {
    pub category: ErrorCategory,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<crate::temporal::TemporalDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<crate::validation::ValidationDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<crate::cache_v3::CacheDiagnostic>,
}

impl From<&Error> for ErrorReport {
    fn from(error: &Error) -> Self {
        Self {
            category: error.category(),
            message: error.message().to_owned(),
            temporal: error.temporal_diagnostic().cloned(),
            validation: error.validation_diagnostic().cloned(),
            cache: error.cache_diagnostic().cloned(),
        }
    }
}

/// Success or failure carried by a machine-readable command envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum MachineOutcome<T> {
    Success {
        result: T,
    },
    Error {
        error: ErrorReport,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<T>,
    },
}

/// Versioned machine-readable result shared by the CLI and external consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MachineEnvelope<T> {
    pub schema_version: u32,
    pub command: CommandName,
    #[serde(flatten)]
    pub outcome: MachineOutcome<T>,
}

impl<T> MachineEnvelope<T> {
    #[must_use]
    pub const fn success(command: CommandName, result: T) -> Self {
        Self {
            schema_version: MACHINE_SCHEMA_VERSION,
            command,
            outcome: MachineOutcome::Success { result },
        }
    }

    #[must_use]
    pub fn failure(command: CommandName, error: &Error, result: Option<T>) -> Self {
        Self {
            schema_version: MACHINE_SCHEMA_VERSION,
            command,
            outcome: MachineOutcome::Error {
                error: ErrorReport::from(error),
                result,
            },
        }
    }

    pub fn ensure_supported(&self) -> Result<()> {
        if self.schema_version == MACHINE_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCategory::Configuration,
                format!(
                    "unsupported machine contract version {}; expected {}",
                    self.schema_version, MACHINE_SCHEMA_VERSION
                ),
            ))
        }
    }
}

impl<T> MachineEnvelope<T>
where
    T: DeserializeOwned,
{
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_slice(input).map_err(|error| {
            Error::new(
                ErrorCategory::Configuration,
                format!("invalid machine contract JSON: {error}"),
            )
        })?;
        let object = value.as_object().ok_or_else(|| {
            Error::new(
                ErrorCategory::Configuration,
                "machine contract must be a JSON object",
            )
        })?;
        if let Some(field) = object.keys().find(|field| {
            !matches!(
                field.as_str(),
                "schema_version" | "command" | "status" | "result" | "error"
            )
        }) {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!("machine contract contains unknown field {field}"),
            ));
        }
        if object.get("status").and_then(serde_json::Value::as_str) == Some("success")
            && object.contains_key("error")
        {
            return Err(Error::new(
                ErrorCategory::Configuration,
                "successful machine contracts must not contain an error field",
            ));
        }
        let envelope: Self = serde_json::from_value(value).map_err(|error| {
            Error::new(
                ErrorCategory::Configuration,
                format!("invalid machine contract JSON: {error}"),
            )
        })?;
        envelope.ensure_supported()?;
        Ok(envelope)
    }
}

#[cfg(test)]
mod toolchain_command_tests {
    use super::*;

    #[test]
    fn toolchain_commands_have_distinct_stable_machine_names() {
        for (command, expected) in [
            (CommandName::ToolchainDoctor, "toolchain_doctor"),
            (CommandName::ToolchainPlan, "toolchain_plan"),
            (CommandName::ToolchainProbe, "toolchain_probe"),
            (CommandName::ToolchainPrepareRegistration, "toolchain_prepare_registration"),
        ] {
            assert_eq!(command.to_string(), expected);
            assert_eq!(serde_json::to_value(command).unwrap(), expected);
            let decoded: CommandName = serde_json::from_value(serde_json::json!(expected)).unwrap();
            assert_eq!(decoded, command);
        }
    }
}
