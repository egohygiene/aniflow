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
    ToolchainPreparePublishing,
    ToolchainPrepareCore,
    ToolchainPrepareWorkflow,
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
    VideoAdvisoryNotes,
    VideoAdvisoryPrepare,
    VideoAdvisoryRunLocal,
    VideoStagesPlan,
    VideoStagesStatus,
    VideoStagesManual,
    VideoFrameReviewStatus,
    VideoFrameReviewRecord,
    VideoRifeRecoveredPlan,
    VideoRifeRecoveredRun,
    VideoRifeRecoveredResume,
    VideoRifeRecoveredStatus,
    VideoRifeRecoveredBindAssembly,
    VideoRifeChunksBindAssembly,
    VideoRifeChunksStatus,
    VideoRifeChunksResume,
    VideoRifeChunksRun,
    VideoRifeChunksPlan,
    VideoDeflickerPlan,
    VideoDeflickerRun,
    VideoDeflickerResume,
    VideoDeflickerStatus,
    VideoDeflickerRecordReview,
    VideoRifePlan,
    VideoRifeRun,
    VideoRifeResume,
    VideoRifeStatus,
    VideoRifePreviewPlan,
    VideoRifePreviewRun,
    VideoRifePreviewResume,
    VideoRifePreviewStatus,

    VideoCaptionPreviewPlan,
    VideoCaptionPreviewRun,
    VideoCaptionPreviewResume,
    VideoCaptionPreviewStatus,
    VideoCaptionDeliveryPlan,
    VideoCaptionDeliveryRun,
    VideoCaptionDeliveryResume,
    VideoCaptionDeliveryStatus,
    VideoFinishingPlan,
    VideoFinishingRun,
    VideoFinishingResume,
    VideoFinishingStatus,
    VideoFinishingNextAction,
    VideoFinishingRecordReview,
    VideoHistoricalImportReviewPlan,
    VideoSpatialCarrierInspect,
    VideoRecoveredV2Plan,
    VideoRecoveredV2Run,
    VideoRecoveredV2Resume,
    VideoRecoveredV2Status,
    VideoRecoveredPlan,
    VideoRecoveredRun,
    VideoRecoveredResume,
    VideoRecoveredStatus,
    VideoSpatialCarrierPlan,
    VideoSpatialCarrierCreate,
    VideoSpatialCarrierStatus,
    VideoHistoricalImportPlan,
    VideoHistoricalImportRun,
    VideoHistoricalImportResume,
    VideoHistoricalImportStatus,

    DeliveryPreflight,
    DeliveryAudioPublishingPlan,
    DeliveryAudioPublishingRun,
    DeliveryAudioPublishingResume,
    DeliveryAudioPublishingStatus,
    DeliveryAudioPublishingNextAction,
    DeliveryAudioPublishingRecordReview,
    DeliveryPublishingPlan,
    DeliveryPublishingRun,
    DeliveryPublishingResume,
    DeliveryPublishingStatus,
    DeliveryPublishingNextAction,
    DeliveryPublishingRecordReview,
    DeliveryLosslessQcResumablePlan,
    DeliveryLosslessQcResumableRun,
    DeliveryLosslessQcResumableResume,
    DeliveryLosslessQcResumableStatus,
    DeliveryLosslessQcPlan,
    DeliveryLosslessQcRun,
    DeliveryLosslessQcStatus,
    DeliveryAudioQcResumablePlan,
    DeliveryAudioQcResumableRun,
    DeliveryAudioQcResumableResume,
    DeliveryAudioQcResumableStatus,
    DeliveryAudioQcPlan,
    DeliveryAudioQcRun,
    DeliveryAudioQcStatus,
    DeliveryVideoExportIntent,
    DeliveryVideoExportPlan,
    DeliveryVideoExportRun,
    DeliveryVideoExportResume,
    DeliveryVideoExportStatus,
    DeliveryPostingPlan,
    DeliveryPackage,
    DeliveryPackageStatus,
    DeliveryPostUploadRecord,
    DeliveryPostUploadInspect,
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
            Self::ToolchainPreparePublishing=>"toolchain_prepare_publishing",
            Self::ToolchainPrepareCore=>"toolchain_prepare_core",
            Self::ToolchainPrepareWorkflow=>"toolchain_prepare_workflow",
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
            Self::VideoAdvisoryNotes=>"video_advisory_notes",
            Self::VideoAdvisoryPrepare=>"video_advisory_prepare",
            Self::VideoAdvisoryRunLocal=>"video_advisory_run_local",
            Self::VideoStagesPlan=>"video_stages_plan",
            Self::VideoStagesStatus=>"video_stages_status",
            Self::VideoStagesManual=>"video_stages_manual",
            Self::VideoFrameReviewStatus => "video_frame_review_status",
            Self::VideoFrameReviewRecord => "video_frame_review_record",
            Self::VideoRifeRecoveredPlan=>"video_rife_recovered_plan",
            Self::VideoRifeRecoveredRun=>"video_rife_recovered_run",
            Self::VideoRifeRecoveredResume=>"video_rife_recovered_resume",
            Self::VideoRifeRecoveredStatus=>"video_rife_recovered_status",
            Self::VideoRifeRecoveredBindAssembly=>"video_rife_recovered_bind_assembly",
            Self::VideoRifeChunksBindAssembly=>"video_rife_chunks_bind_assembly",
            Self::VideoRifeChunksStatus=>"video_rife_chunks_status",
            Self::VideoRifeChunksResume=>"video_rife_chunks_resume",
            Self::VideoRifeChunksRun=>"video_rife_chunks_run",
            Self::VideoRifeChunksPlan=>"video_rife_chunks_plan",
            Self::VideoDeflickerPlan=>"video_deflicker_plan",
            Self::VideoDeflickerRun=>"video_deflicker_run",
            Self::VideoDeflickerResume=>"video_deflicker_resume",
            Self::VideoDeflickerStatus=>"video_deflicker_status",
            Self::VideoDeflickerRecordReview=>"video_deflicker_record_review",
            Self::VideoRifePlan => "video_rife_plan",
            Self::VideoRifeRun => "video_rife_run",
            Self::VideoRifeResume => "video_rife_resume",
            Self::VideoRifeStatus => "video_rife_status",
            Self::VideoRifePreviewPlan => "video_rife_preview_plan",
            Self::VideoRifePreviewRun => "video_rife_preview_run",
            Self::VideoRifePreviewResume => "video_rife_preview_resume",
            Self::VideoRifePreviewStatus => "video_rife_preview_status",

            Self::VideoCaptionPreviewPlan => "video_caption_preview_plan",
            Self::VideoCaptionPreviewRun => "video_caption_preview_run",
            Self::VideoCaptionPreviewResume => "video_caption_preview_resume",
            Self::VideoCaptionPreviewStatus => "video_caption_preview_status",
            Self::VideoCaptionDeliveryPlan => "video_caption_delivery_plan",
            Self::VideoCaptionDeliveryRun => "video_caption_delivery_run",
            Self::VideoCaptionDeliveryResume => "video_caption_delivery_resume",
            Self::VideoCaptionDeliveryStatus => "video_caption_delivery_status",
            Self::VideoFinishingPlan => "video_finishing_plan",
            Self::VideoFinishingRun => "video_finishing_run",
            Self::VideoFinishingResume => "video_finishing_resume",
            Self::VideoFinishingStatus => "video_finishing_status",
            Self::VideoFinishingNextAction => "video_finishing_next_action",
            Self::VideoFinishingRecordReview => "video_finishing_record_review",
            Self::VideoHistoricalImportReviewPlan => "video_historical_import_review_plan",
            Self::VideoSpatialCarrierInspect => "video_spatial_carrier_inspect",
            Self::VideoRecoveredV2Plan=>"video_recovered_v2_plan",
            Self::VideoRecoveredV2Run=>"video_recovered_v2_run",
            Self::VideoRecoveredV2Resume=>"video_recovered_v2_resume",
            Self::VideoRecoveredV2Status=>"video_recovered_v2_status",
            Self::VideoRecoveredPlan => "video_recovered_plan",
            Self::VideoRecoveredRun => "video_recovered_run",
            Self::VideoRecoveredResume => "video_recovered_resume",
            Self::VideoRecoveredStatus => "video_recovered_status",
            Self::VideoSpatialCarrierPlan => "video_spatial_carrier_plan",
            Self::VideoSpatialCarrierCreate => "video_spatial_carrier_create",
            Self::VideoSpatialCarrierStatus => "video_spatial_carrier_status",
            Self::VideoHistoricalImportPlan => "video_historical_import_plan",
            Self::VideoHistoricalImportRun => "video_historical_import_run",
            Self::VideoHistoricalImportResume => "video_historical_import_resume",
            Self::VideoHistoricalImportStatus => "video_historical_import_status",

            Self::DeliveryPreflight => "delivery_preflight",
            Self::DeliveryAudioPublishingPlan=>"delivery_audio_publishing_plan",
            Self::DeliveryAudioPublishingRun=>"delivery_audio_publishing_run",
            Self::DeliveryAudioPublishingResume=>"delivery_audio_publishing_resume",
            Self::DeliveryAudioPublishingStatus=>"delivery_audio_publishing_status",
            Self::DeliveryAudioPublishingNextAction=>"delivery_audio_publishing_next_action",
            Self::DeliveryAudioPublishingRecordReview=>"delivery_audio_publishing_record_review",
            Self::DeliveryPublishingPlan => "delivery_publishing_plan",
            Self::DeliveryPublishingRun => "delivery_publishing_run",
            Self::DeliveryPublishingResume => "delivery_publishing_resume",
            Self::DeliveryPublishingStatus => "delivery_publishing_status",
            Self::DeliveryPublishingNextAction => "delivery_publishing_next_action",
            Self::DeliveryPublishingRecordReview => "delivery_publishing_record_review",
            Self::DeliveryLosslessQcResumablePlan=>"delivery_lossless_qc_resumable_plan",
            Self::DeliveryLosslessQcResumableRun=>"delivery_lossless_qc_resumable_run",
            Self::DeliveryLosslessQcResumableResume=>"delivery_lossless_qc_resumable_resume",
            Self::DeliveryLosslessQcResumableStatus=>"delivery_lossless_qc_resumable_status",
            Self::DeliveryLosslessQcPlan=>"delivery_lossless_qc_plan",
            Self::DeliveryLosslessQcRun=>"delivery_lossless_qc_run",
            Self::DeliveryLosslessQcStatus=>"delivery_lossless_qc_status",
            Self::DeliveryAudioQcResumablePlan=>"delivery_audio_qc_resumable_plan",
            Self::DeliveryAudioQcResumableRun=>"delivery_audio_qc_resumable_run",
            Self::DeliveryAudioQcResumableResume=>"delivery_audio_qc_resumable_resume",
            Self::DeliveryAudioQcResumableStatus=>"delivery_audio_qc_resumable_status",
            Self::DeliveryAudioQcPlan => "delivery_audio_qc_plan",
            Self::DeliveryAudioQcRun => "delivery_audio_qc_run",
            Self::DeliveryAudioQcStatus => "delivery_audio_qc_status",
            Self::DeliveryVideoExportIntent => "delivery_video_export_intent",
            Self::DeliveryVideoExportPlan => "delivery_video_export_plan",
            Self::DeliveryVideoExportRun => "delivery_video_export_run",
            Self::DeliveryVideoExportResume => "delivery_video_export_resume",
            Self::DeliveryVideoExportStatus => "delivery_video_export_status",
            Self::DeliveryPostingPlan => "delivery_posting_plan",
            Self::DeliveryPackage => "delivery_package",
            Self::DeliveryPackageStatus => "delivery_package_status",
            Self::DeliveryPostUploadRecord => "delivery_post_upload_record",
            Self::DeliveryPostUploadInspect => "delivery_post_upload_inspect",
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
            (CommandName::VideoAdvisoryNotes,"video_advisory_notes"),
            (CommandName::VideoAdvisoryPrepare,"video_advisory_prepare"),
            (CommandName::VideoAdvisoryRunLocal,"video_advisory_run_local"),
            (CommandName::VideoStagesPlan,"video_stages_plan"),
            (CommandName::VideoStagesStatus,"video_stages_status"),
            (CommandName::VideoStagesManual,"video_stages_manual"),
            (CommandName::VideoFrameReviewStatus, "video_frame_review_status"),
            (CommandName::VideoFrameReviewRecord, "video_frame_review_record"),
            (CommandName::VideoRifeRecoveredPlan,"video_rife_recovered_plan"),
            (CommandName::VideoRifeRecoveredRun,"video_rife_recovered_run"),
            (CommandName::VideoRifeRecoveredResume,"video_rife_recovered_resume"),
            (CommandName::VideoRifeRecoveredStatus,"video_rife_recovered_status"),
            (CommandName::VideoRifeRecoveredBindAssembly,"video_rife_recovered_bind_assembly"),
            (CommandName::VideoRifeChunksBindAssembly,"video_rife_chunks_bind_assembly"),
            (CommandName::VideoRifeChunksStatus,"video_rife_chunks_status"),
            (CommandName::VideoRifeChunksResume,"video_rife_chunks_resume"),
            (CommandName::VideoRifeChunksRun,"video_rife_chunks_run"),
            (CommandName::VideoRifeChunksPlan,"video_rife_chunks_plan"),
            (CommandName::VideoDeflickerPlan,"video_deflicker_plan"),
            (CommandName::VideoDeflickerRun,"video_deflicker_run"),
            (CommandName::VideoDeflickerResume,"video_deflicker_resume"),
            (CommandName::VideoDeflickerStatus,"video_deflicker_status"),
            (CommandName::VideoDeflickerRecordReview,"video_deflicker_record_review"),
            (CommandName::VideoRifePlan, "video_rife_plan"),
            (CommandName::VideoRifeRun, "video_rife_run"),
            (CommandName::VideoRifeResume, "video_rife_resume"),
            (CommandName::VideoRifeStatus, "video_rife_status"),
            (CommandName::VideoRifePreviewPlan, "video_rife_preview_plan"),
            (CommandName::VideoRifePreviewRun, "video_rife_preview_run"),
            (CommandName::VideoRifePreviewResume, "video_rife_preview_resume"),
            (CommandName::VideoRifePreviewStatus, "video_rife_preview_status"),

            (CommandName::VideoCaptionPreviewPlan, "video_caption_preview_plan"),
            (CommandName::VideoCaptionPreviewRun, "video_caption_preview_run"),
            (CommandName::VideoCaptionPreviewResume, "video_caption_preview_resume"),
            (CommandName::VideoCaptionPreviewStatus, "video_caption_preview_status"),
            (CommandName::VideoCaptionDeliveryPlan, "video_caption_delivery_plan"),
            (CommandName::VideoCaptionDeliveryRun, "video_caption_delivery_run"),
            (CommandName::VideoCaptionDeliveryResume, "video_caption_delivery_resume"),
            (CommandName::VideoCaptionDeliveryStatus, "video_caption_delivery_status"),
            (CommandName::VideoFinishingPlan, "video_finishing_plan"),
            (CommandName::VideoFinishingRun, "video_finishing_run"),
            (CommandName::VideoFinishingResume, "video_finishing_resume"),
            (CommandName::VideoFinishingStatus, "video_finishing_status"),
            (CommandName::VideoFinishingNextAction, "video_finishing_next_action"),
            (CommandName::VideoFinishingRecordReview, "video_finishing_record_review"),
            (CommandName::VideoHistoricalImportReviewPlan, "video_historical_import_review_plan"),
            (CommandName::VideoSpatialCarrierInspect, "video_spatial_carrier_inspect"),
            (CommandName::VideoRecoveredV2Plan,"video_recovered_v2_plan"),
            (CommandName::VideoRecoveredV2Run,"video_recovered_v2_run"),
            (CommandName::VideoRecoveredV2Resume,"video_recovered_v2_resume"),
            (CommandName::VideoRecoveredV2Status,"video_recovered_v2_status"),
            (CommandName::VideoRecoveredPlan, "video_recovered_plan"),
            (CommandName::VideoRecoveredRun, "video_recovered_run"),
            (CommandName::VideoRecoveredResume, "video_recovered_resume"),
            (CommandName::VideoRecoveredStatus, "video_recovered_status"),
            (CommandName::VideoSpatialCarrierPlan, "video_spatial_carrier_plan"),
            (CommandName::VideoSpatialCarrierCreate, "video_spatial_carrier_create"),
            (CommandName::VideoSpatialCarrierStatus, "video_spatial_carrier_status"),
            (CommandName::VideoHistoricalImportPlan, "video_historical_import_plan"),
            (CommandName::VideoHistoricalImportRun, "video_historical_import_run"),
            (CommandName::VideoHistoricalImportResume, "video_historical_import_resume"),
            (CommandName::VideoHistoricalImportStatus, "video_historical_import_status"),

            (CommandName::DeliveryPreflight, "delivery_preflight"),
            (CommandName::DeliveryVideoExportIntent, "delivery_video_export_intent"),
            (CommandName::DeliveryAudioPublishingPlan,"delivery_audio_publishing_plan"),
            (CommandName::DeliveryAudioPublishingRun,"delivery_audio_publishing_run"),
            (CommandName::DeliveryAudioPublishingResume,"delivery_audio_publishing_resume"),
            (CommandName::DeliveryAudioPublishingStatus,"delivery_audio_publishing_status"),
            (CommandName::DeliveryAudioPublishingNextAction,"delivery_audio_publishing_next_action"),
            (CommandName::DeliveryAudioPublishingRecordReview,"delivery_audio_publishing_record_review"),
            (CommandName::DeliveryPublishingPlan, "delivery_publishing_plan"),
            (CommandName::DeliveryPublishingRun, "delivery_publishing_run"),
            (CommandName::DeliveryPublishingResume, "delivery_publishing_resume"),
            (CommandName::DeliveryPublishingStatus, "delivery_publishing_status"),
            (CommandName::DeliveryPublishingNextAction, "delivery_publishing_next_action"),
            (CommandName::DeliveryPublishingRecordReview, "delivery_publishing_record_review"),
            (CommandName::DeliveryLosslessQcResumablePlan,"delivery_lossless_qc_resumable_plan"),
            (CommandName::DeliveryLosslessQcResumableRun,"delivery_lossless_qc_resumable_run"),
            (CommandName::DeliveryLosslessQcResumableResume,"delivery_lossless_qc_resumable_resume"),
            (CommandName::DeliveryLosslessQcResumableStatus,"delivery_lossless_qc_resumable_status"),
            (CommandName::DeliveryLosslessQcPlan,"delivery_lossless_qc_plan"),
            (CommandName::DeliveryLosslessQcRun,"delivery_lossless_qc_run"),
            (CommandName::DeliveryLosslessQcStatus,"delivery_lossless_qc_status"),
            (CommandName::DeliveryAudioQcResumablePlan,"delivery_audio_qc_resumable_plan"),
            (CommandName::DeliveryAudioQcResumableRun,"delivery_audio_qc_resumable_run"),
            (CommandName::DeliveryAudioQcResumableResume,"delivery_audio_qc_resumable_resume"),
            (CommandName::DeliveryAudioQcResumableStatus,"delivery_audio_qc_resumable_status"),
            (CommandName::DeliveryAudioQcPlan, "delivery_audio_qc_plan"),
            (CommandName::DeliveryAudioQcRun, "delivery_audio_qc_run"),
            (CommandName::DeliveryAudioQcStatus, "delivery_audio_qc_status"),
            (CommandName::DeliveryVideoExportPlan, "delivery_video_export_plan"),
            (CommandName::DeliveryVideoExportRun, "delivery_video_export_run"),
            (CommandName::DeliveryVideoExportResume, "delivery_video_export_resume"),
            (CommandName::DeliveryVideoExportStatus, "delivery_video_export_status"),
            (CommandName::DeliveryPostingPlan, "delivery_posting_plan"),
            (CommandName::DeliveryPackage, "delivery_package"),
            (CommandName::DeliveryPackageStatus, "delivery_package_status"),
            (CommandName::DeliveryPostUploadRecord, "delivery_post_upload_record"),
            (CommandName::DeliveryPostUploadInspect, "delivery_post_upload_inspect"),
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
            (CommandName::ToolchainPreparePublishing,"toolchain_prepare_publishing"),
            (CommandName::ToolchainPrepareCore,"toolchain_prepare_core"),
            (CommandName::ToolchainPrepareWorkflow,"toolchain_prepare_workflow"),
        ] {
            assert_eq!(command.to_string(), expected);
            assert_eq!(serde_json::to_value(command).unwrap(), expected);
            let decoded: CommandName = serde_json::from_value(serde_json::json!(expected)).unwrap();
            assert_eq!(decoded, command);
        }
    }
}
