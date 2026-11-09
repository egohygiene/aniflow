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
    ToolchainPrepareMedia,
    Inspect,
    TemporalInspect,
    VideoInterpolationPlan,
    VideoFrameSelectionPlan,
    VideoExtractFrames,
    VideoImportFrames,
    VideoRepairFrames,
    VideoUpscaleFrames,
    VideoUpscalePreview,
    VideoAssembleFrames,
    /// Source-entry and indexed-entry recipes share the command; result schemas distinguish them.
    VideoMusicVideo,
    /// Inspect either recipe entry form using its immutable intent schema.
    VideoMusicVideoStatus,
    VideoCompletion,
    VideoFrameReviewStatus,
    VideoFrameReviewRecord,
    VideoRifePlan,
    VideoRifeRun,
    VideoRifeResume,
    VideoRifeStatus,
    DeliveryPreflight,
    DeliveryPostingPlan,
    DeliveryPackage,
    DeliveryPackageStatus,
    DeliveryAudioExportPlan,
    DeliveryAudioExportRun,
    DeliveryAudioExportResume,
    DeliveryAudioExportStatus,
    DeliveryEvidencePlan,
    DeliveryEvidenceSeal,
    DeliveryEvidenceVerify,
    DeliveryEvidenceRecordVerification,
    VideoLyricsPrepareRequest,
    VideoLyricsInitialize,
    VideoLyricsImport,
    VideoLyricsStatus,
    VideoLyricsReview,
    VideoLyricsExport,
    VideoLyricsAttach,
    VideoMetadataRemuxPlan,
    VideoMetadataRemuxRun,
    VideoMetadataRemuxResume,
    VideoMetadataRemuxStatus,
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
            Self::ToolchainPrepareMedia => "toolchain_prepare_media",
            Self::Inspect => "inspect",
            Self::TemporalInspect => "temporal_inspect",
            Self::VideoInterpolationPlan => "video_interpolation_plan",
            Self::VideoFrameSelectionPlan => "video_frame_selection_plan",
            Self::VideoExtractFrames => "video_extract_frames",
            Self::VideoImportFrames => "video_import_frames",
            Self::VideoRepairFrames => "video_repair_frames",
            Self::VideoUpscaleFrames => "video_upscale_frames",
            Self::VideoUpscalePreview => "video_upscale_preview",
            Self::VideoAssembleFrames => "video_assemble_frames",
            Self::VideoMusicVideo => "video_music_video",
            Self::VideoMusicVideoStatus => "video_music_video_status",
            Self::VideoCompletion => "video_completion",
            Self::VideoFrameReviewStatus => "video_frame_review_status",
            Self::VideoFrameReviewRecord => "video_frame_review_record",
            Self::VideoRifePlan => "video_rife_plan",
            Self::VideoRifeRun => "video_rife_run",
            Self::VideoRifeResume => "video_rife_resume",
            Self::VideoRifeStatus => "video_rife_status",
            Self::DeliveryPreflight => "delivery_preflight",
            Self::DeliveryPostingPlan => "delivery_posting_plan",
            Self::DeliveryPackage => "delivery_package",
            Self::DeliveryPackageStatus => "delivery_package_status",
            Self::DeliveryAudioExportPlan => "delivery_audio_export_plan",
            Self::DeliveryAudioExportRun => "delivery_audio_export_run",
            Self::DeliveryAudioExportResume => "delivery_audio_export_resume",
            Self::DeliveryAudioExportStatus => "delivery_audio_export_status",
            Self::DeliveryEvidencePlan => "delivery_evidence_plan",
            Self::DeliveryEvidenceSeal => "delivery_evidence_seal",
            Self::DeliveryEvidenceVerify => "delivery_evidence_verify",
            Self::DeliveryEvidenceRecordVerification => "delivery_evidence_record_verification",
            Self::VideoLyricsPrepareRequest => "video_lyrics_prepare_request",
            Self::VideoLyricsInitialize => "video_lyrics_initialize",
            Self::VideoLyricsImport => "video_lyrics_import",
            Self::VideoLyricsStatus => "video_lyrics_status",
            Self::VideoLyricsReview => "video_lyrics_review",
            Self::VideoLyricsExport => "video_lyrics_export",
            Self::VideoLyricsAttach => "video_lyrics_attach",
            Self::VideoMetadataRemuxPlan => "video_metadata_remux_plan",
            Self::VideoMetadataRemuxRun => "video_metadata_remux_run",
            Self::VideoMetadataRemuxResume => "video_metadata_remux_resume",
            Self::VideoMetadataRemuxStatus => "video_metadata_remux_status",
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
            (CommandName::VideoInterpolationPlan, "video_interpolation_plan"),
            (CommandName::VideoFrameSelectionPlan, "video_frame_selection_plan"),
            (CommandName::VideoExtractFrames, "video_extract_frames"),
            (CommandName::VideoAssembleFrames, "video_assemble_frames"),
            (CommandName::VideoUpscalePreview, "video_upscale_preview"),
            (CommandName::VideoMusicVideo, "video_music_video"),
            (CommandName::VideoMusicVideoStatus, "video_music_video_status"),
            (CommandName::VideoCompletion, "video_completion"),
            (CommandName::VideoFrameReviewStatus, "video_frame_review_status"),
            (CommandName::VideoFrameReviewRecord, "video_frame_review_record"),
            (CommandName::VideoRifePlan, "video_rife_plan"),
            (CommandName::VideoRifeRun, "video_rife_run"),
            (CommandName::VideoRifeResume, "video_rife_resume"),
            (CommandName::VideoRifeStatus, "video_rife_status"),
            (CommandName::DeliveryPreflight, "delivery_preflight"),
            (CommandName::DeliveryPostingPlan, "delivery_posting_plan"),
            (CommandName::DeliveryPackage, "delivery_package"),
            (CommandName::DeliveryPackageStatus, "delivery_package_status"),
            (CommandName::DeliveryAudioExportPlan, "delivery_audio_export_plan"),
            (CommandName::DeliveryAudioExportRun, "delivery_audio_export_run"),
            (CommandName::DeliveryAudioExportResume, "delivery_audio_export_resume"),
            (CommandName::DeliveryAudioExportStatus, "delivery_audio_export_status"),
            (CommandName::DeliveryEvidencePlan, "delivery_evidence_plan"),
            (CommandName::DeliveryEvidenceSeal, "delivery_evidence_seal"),
            (CommandName::DeliveryEvidenceVerify, "delivery_evidence_verify"),
            (CommandName::DeliveryEvidenceRecordVerification, "delivery_evidence_record_verification"),
            (CommandName::VideoLyricsPrepareRequest, "video_lyrics_prepare_request"),
            (CommandName::VideoLyricsInitialize, "video_lyrics_initialize"),
            (CommandName::VideoLyricsImport, "video_lyrics_import"),
            (CommandName::VideoLyricsStatus, "video_lyrics_status"),
            (CommandName::VideoLyricsReview, "video_lyrics_review"),
            (CommandName::VideoLyricsExport, "video_lyrics_export"),
            (CommandName::VideoLyricsAttach, "video_lyrics_attach"),
            (CommandName::VideoMetadataRemuxPlan, "video_metadata_remux_plan"),
            (CommandName::VideoMetadataRemuxRun, "video_metadata_remux_run"),
            (CommandName::VideoMetadataRemuxResume, "video_metadata_remux_resume"),
            (CommandName::VideoMetadataRemuxStatus, "video_metadata_remux_status"),
            (CommandName::ToolchainPrepareRegistration, "toolchain_prepare_registration"),
            (CommandName::ToolchainPrepareMedia, "toolchain_prepare_media"),
        ] {
            assert_eq!(command.to_string(), expected);
            assert_eq!(serde_json::to_value(command).unwrap(), expected);
            let decoded: CommandName = serde_json::from_value(serde_json::json!(expected)).unwrap();
            assert_eq!(decoded, command);
        }
    }
}
