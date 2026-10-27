use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::audio_inspection::AudioToolPin;
use crate::frame_import::FrameImportConfiguration;
use crate::frame_selection::{FrameSelectionPlan, InclusiveFrameRange};
use crate::temporal::TimeRange;
use crate::video_interpolation::RifeFrameIndex;

pub const FRAME_REPAIR_CONFIGURATION_SCHEMA_V1: &str = "aniflow.frame-repair-configuration/v1";
pub const FRAME_REPAIR_PROVIDER_CONFIGURATION_SCHEMA_V1: &str = "aniflow.frame-repair-provider-configuration/v1";
pub const FRAME_REPAIR_REPORT_SCHEMA_V1: &str = "aniflow.frame-repair-report/v1";
pub const FRAME_REPAIR_PROVIDER_ID: &str = "org.egohygiene.aniflow.frame-repair";
pub const FRAME_REPAIR_CAPABILITY_ID: &str = "aniflow/frame-repair";
pub const FRAME_REPAIR_MAXIMUM_REPORT_BYTES: u64 = 67_108_864;

/// Explicit caller-selected direct-image interface. The version/profile are
/// declarations; the executable bytes are observed. No model identity is inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectImageProcessor {
    pub profile: String,
    pub tool: AudioToolPin,
    pub arguments: Vec<String>,
}

/// Import/decode policy and the exact processor selected for manual ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRepairConfiguration {
    pub schema: String,
    pub import: FrameImportConfiguration,
    pub processor: DirectImageProcessor,
    #[serde(default = "working_bytes")]
    pub maximum_working_bytes: u64,
}
fn working_bytes() -> u64 { 274_877_906_944 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRepairOperation {
    pub schema: String,
    pub settings: FrameRepairConfiguration,
    pub ranges: Vec<InclusiveFrameRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRepairFrameObservation {
    pub ordinal: u64,
    pub filename: String,
    pub source_time: TimeRange,
    pub selected: bool,
    pub bytes_changed: bool,
    pub input_sha256: String,
    pub output_sha256: String,
    pub output_byte_count: u64,
    pub decoded_rgb24_sha256: String,
}

/// Observed new operation, never a visual-quality or historical-origin claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameRepairReport {
    pub schema: String,
    pub stage_id: String,
    pub provider_lock_sha256: String,
    pub configuration_sha256: String,
    pub import_report_sha256: String,
    pub source_relationship: String,
    pub historical_processing: String,
    pub processor_profile: String,
    pub processor_sha256: String,
    pub processor_declared_version: String,
    pub model_environment_evidence: String,
    pub decoder_sha256: String,
    pub selection: FrameSelectionPlan,
    pub changed_frame_count: u64,
    pub output_index: RifeFrameIndex,
    pub frames: Vec<FrameRepairFrameObservation>,
}

/// A fresh import followed by selective repair in the same ordered v3 run.
#[derive(Debug, Clone)]
pub struct FrameRepairRequest {
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: FrameRepairConfiguration,
    pub ranges: Vec<InclusiveFrameRange>,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}

impl FrameRepairRequest {
    #[must_use]
    pub fn new(
        source_video: impl Into<PathBuf>, frames_directory: impl Into<PathBuf>,
        frame_index: impl Into<PathBuf>, configuration: FrameRepairConfiguration,
        ranges: Vec<InclusiveFrameRange>, provider_executable: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source_video: source_video.into(), frames_directory: frames_directory.into(),
            frame_index: frame_index.into(), configuration, ranges,
            provider_executable: provider_executable.into(),
            execution_limits: crate::ProviderExecutionLimits {
                timeout: std::time::Duration::from_secs(21_600),
                termination_grace_period: std::time::Duration::from_millis(2_000),
                maximum_stdout_bytes: 1_048_576, maximum_stderr_bytes: 1_048_576,
                maximum_artifact_files: 100_003, maximum_artifact_bytes: 68_853_694_464,
            },
        }
    }
}
