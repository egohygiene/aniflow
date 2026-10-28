use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::audio_inspection::{AudioToolObservation, AudioToolPin};
use crate::temporal::StreamSelection;
use super::FrameExtractionMediaEvidence;

pub const FRAME_EXTRACTION_CONFIGURATION_SCHEMA_V1: &str = "aniflow.frame-extraction-configuration/v1";
pub const FRAME_EXTRACTION_OPERATION_SCHEMA_V1: &str = "aniflow.frame-extraction-operation/v1";
pub const FRAME_EXTRACTION_REPORT_SCHEMA_V1: &str = "aniflow.frame-extraction-report/v1";
pub const FRAME_EXTRACTION_PROVIDER_ID: &str = "org.egohygiene.aniflow.frame-extraction";
pub const FRAME_EXTRACTION_CAPABILITY_ID: &str = "aniflow/extract-frames";
pub const FRAME_EXTRACTION_MAXIMUM_INDEX_BYTES: u64 = crate::video_interpolation::RIFE_MAXIMUM_INDEX_BYTES;
pub const FRAME_EXTRACTION_MAXIMUM_REPORT_BYTES: u64 = 67_108_864;
pub const FRAME_EXTRACTION_MAXIMUM_TOOL_BYTES: u64 = 134_217_728;

/// Closed source-to-RGB-PNG extraction. Planning binds files and policy; native
/// admission and actual geometry/count remain pending until stage execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameExtractionConfiguration {
    pub schema: String,
    pub ffmpeg: AudioToolPin,
    pub ffprobe: AudioToolPin,
    pub source_selection: StreamSelection,
    #[serde(default = "timeout")]
    pub timeout_milliseconds: u64,
    #[serde(default = "capture_bytes")]
    pub maximum_capture_bytes: usize,
    #[serde(default = "source_bytes")]
    pub maximum_source_bytes: u64,
    #[serde(default = "frame_bytes")]
    pub maximum_frame_bytes: u64,
    #[serde(default = "total_frame_bytes")]
    pub maximum_total_frame_bytes: u64,
    #[serde(default = "pixels")]
    pub maximum_pixels: u64,
    #[serde(default = "frames")]
    pub maximum_frames: u64,
    #[serde(default = "working_bytes")]
    pub maximum_working_bytes: u64,
    #[serde(default = "free_bytes")]
    pub minimum_free_bytes: u64,
}
fn timeout() -> u64 { 21_600_000 }
fn capture_bytes() -> usize { 67_108_864 }
fn source_bytes() -> u64 { 68_719_476_736 }
fn frame_bytes() -> u64 { 67_108_864 }
fn total_frame_bytes() -> u64 { 274_877_906_944 }
fn pixels() -> u64 { 8_400_000 }
fn frames() -> u64 { 100_000 }
fn working_bytes() -> u64 { 2_199_023_255_552 }
fn free_bytes() -> u64 { 67_108_864 }

/// Frozen source identity and settings, carried by the ordinary provider lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameExtractionOperation {
    pub schema: String,
    pub settings: FrameExtractionConfiguration,
    pub source_sha256: String,
    pub source_byte_count: u64,
}

/// Contemporary extraction observations, subordinate to v3 stage acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameExtractionReport {
    pub schema: String,
    pub stage_id: String,
    pub provider_lock_sha256: String,
    pub configuration_sha256: String,
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub frame_index_sha256: String,
    pub frames_sha256: String,
    pub frames_byte_count: u64,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    pub ffmpeg: AudioToolObservation,
    pub ffprobe: AudioToolObservation,
    pub source_relationship: String,
    pub historical_processing: String,
    pub backend_qualification: String,
    pub media: FrameExtractionMediaEvidence,
}

pub type FrameExtractionPlan = crate::PipelineV3Plan;
pub type FrameExtractionOutcome = crate::PipelineV3RunOutcome;

#[derive(Debug, Clone)]
pub struct FrameExtractionRequest {
    pub source_video: PathBuf,
    pub configuration: FrameExtractionConfiguration,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}
impl FrameExtractionRequest {
    #[must_use]
    pub fn new(source_video: impl Into<PathBuf>, configuration: FrameExtractionConfiguration, provider_executable: impl Into<PathBuf>) -> Self {
        // The runtime monitors the whole candidate root during execution,
        // including private source/tools and both private/final frame copies.
        let maximum_artifact_bytes = configuration.maximum_working_bytes;
        let maximum_artifact_files = configuration.maximum_frames.saturating_mul(2).saturating_add(32);
        Self { source_video:source_video.into(), configuration, provider_executable:provider_executable.into(),
            execution_limits:crate::ProviderExecutionLimits {
                timeout:std::time::Duration::from_secs(86_400), termination_grace_period:std::time::Duration::from_millis(2_000),
                maximum_stdout_bytes:1_048_576, maximum_stderr_bytes:1_048_576, maximum_artifact_files, maximum_artifact_bytes,
            } }
    }
}
