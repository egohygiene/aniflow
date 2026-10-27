use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::audio_inspection::AudioToolPin;
use crate::frame_import::FrameImportConfiguration;
use crate::temporal::TimeRange;
use crate::video_interpolation::{RifeFrame, RifeFrameIndex};

pub const UPSCAYL_RECOVERY_CONFIGURATION_SCHEMA_V1: &str = "aniflow.upscayl-recovery-configuration/v1";
pub const UPSCAYL_FRAME_OPERATION_SCHEMA_V1: &str = "aniflow.upscayl-frame-operation/v1";
pub const UPSCAYL_COLLECT_OPERATION_SCHEMA_V1: &str = "aniflow.upscayl-collect-operation/v1";
pub const UPSCAYL_FRAME_REPORT_SCHEMA_V1: &str = "aniflow.upscayl-frame-report/v1";
pub const UPSCAYL_OUTPUT_INDEX_SCHEMA_V1: &str = "aniflow.upscayl-output-index/v1";
pub const UPSCAYL_RECOVERY_REPORT_SCHEMA_V1: &str = "aniflow.upscayl-recovery-report/v1";
pub const UPSCAYL_FRAME_PROVIDER_ID: &str = "org.egohygiene.aniflow.upscayl-frame";
pub const UPSCAYL_COLLECT_PROVIDER_ID: &str = "org.egohygiene.aniflow.upscayl-collect";
pub const UPSCAYL_FRAME_CAPABILITY_ID: &str = "aniflow/upscayl-frame";
pub const UPSCAYL_COLLECT_CAPABILITY_ID: &str = "aniflow/upscayl-collect";
pub const UPSCAYL_INTERFACE_REVISION: &str = "0beb39028a0ddd83250e845b4c3333c0675e3b97";
pub const UPSCAYL_MAXIMUM_FRAMES: usize = 1024;
pub const UPSCAYL_MAXIMUM_FRAME_REPORT_BYTES: u64 = 65_536;
pub const UPSCAYL_MAXIMUM_REPORT_BYTES: u64 = 67_108_864;

/// Explicit regular model file identity. No model discovery or download occurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylModelFile { pub path: PathBuf, pub sha256: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylModelPin {
    pub name: String,
    /// Declared model-native scale, separately passed as -z. This profile admits 4.
    pub native_scale: u8,
    pub param: UpscaylModelFile,
    pub weights: UpscaylModelFile,
}

/// Explicit per-frame policy; native batch remains the separate legacy adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylRecoveryConfiguration {
    pub schema: String,
    pub import: FrameImportConfiguration,
    pub upscayl: AudioToolPin,
    pub model: UpscaylModelPin,
    pub output_scale: u8,
    #[serde(default = "tile_size")]
    pub tile_size: u32,
    #[serde(default)]
    pub gpu_id: u32,
    #[serde(default = "threads")]
    pub threads: String,
    #[serde(default)]
    pub tta: bool,
    #[serde(default = "frame_bytes")]
    pub maximum_output_frame_bytes: u64,
    #[serde(default = "output_pixels")]
    pub maximum_output_pixels: u64,
    #[serde(default = "working_bytes")]
    pub maximum_working_bytes: u64,
    #[serde(default = "free_bytes")]
    pub minimum_free_bytes: u64,
}
fn tile_size() -> u32 { 32 }
fn threads() -> String { "1:1:1".to_owned() }
fn frame_bytes() -> u64 { 67_108_864 }
fn output_pixels() -> u64 { 134_217_728 }
fn working_bytes() -> u64 { 274_877_906_944 }
fn free_bytes() -> u64 { 67_108_864 }

/// One declared source-frame mapping, bound into its exact provider operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylFrameSpec {
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub frame_index_sha256: String,
    pub ordinal: u64,
    pub filename: String,
    pub source_time: TimeRange,
    pub input_sha256: String,
    pub input_byte_count: u64,
    pub input_width: u32,
    pub input_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylFrameOperation {
    pub schema: String,
    pub settings: UpscaylRecoveryConfiguration,
    pub frame: UpscaylFrameSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylCollectOperation {
    pub schema: String,
    pub settings: UpscaylRecoveryConfiguration,
    pub frame_index_sha256: String,
    pub frame_count: u64,
}

/// Observed output of one successful invocation, subordinate to runtime acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylFrameReport {
    pub schema: String,
    pub stage_id: String,
    pub provider_lock_sha256: String,
    pub configuration_sha256: String,
    pub frame: UpscaylFrameSpec,
    pub upscayl_sha256: String,
    pub decoder_sha256: String,
    pub model_param_sha256: String,
    pub model_weights_sha256: String,
    pub native_scale: u8,
    pub output_scale: u8,
    pub output_width: u32,
    pub output_height: u32,
    pub output_sha256: String,
    pub output_byte_count: u64,
    pub input_decoded_rgb24_sha256: String,
    pub output_decoded_rgb24_sha256: String,
    pub processor_elapsed_milliseconds: u64,
    pub backend_qualification: String,
}

/// Derived spatial view. The source inspection and exact clock remain unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylOutputIndex {
    pub schema: String,
    pub source_index: RifeFrameIndex,
    pub output_width: u32,
    pub output_height: u32,
    pub output_profile: String,
    pub frames: Vec<RifeFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylRecoveryReport {
    pub schema: String,
    pub stage_id: String,
    pub provider_lock_sha256: String,
    pub configuration_sha256: String,
    pub source_relationship: String,
    pub historical_processing: String,
    pub backend_qualification: String,
    pub output_index: UpscaylOutputIndex,
    pub frames: Vec<UpscaylFrameReport>,
}

#[derive(Debug, Clone)]
pub struct UpscaylRecoveryRequest {
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: UpscaylRecoveryConfiguration,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}
impl UpscaylRecoveryRequest {
    #[must_use]
    pub fn new(source_video: impl Into<PathBuf>, frames_directory: impl Into<PathBuf>, frame_index: impl Into<PathBuf>, configuration: UpscaylRecoveryConfiguration, provider_executable: impl Into<PathBuf>) -> Self {
        Self { source_video:source_video.into(), frames_directory:frames_directory.into(), frame_index:frame_index.into(), configuration, provider_executable:provider_executable.into(), execution_limits:crate::ProviderExecutionLimits {
            timeout:std::time::Duration::from_secs(21_600), termination_grace_period:std::time::Duration::from_millis(2_000), maximum_stdout_bytes:1_048_576, maximum_stderr_bytes:1_048_576, maximum_artifact_files:1_027, maximum_artifact_bytes:68_853_694_464,
        } }
    }
}

/// Presentation only; immutable stage checkpoints remain the authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylFrameProgress {
    pub total: u64,
    pub completed: u64,
    pub reused: u64,
    pub pending: u64,
    pub failed: u64,
    pub estimated_remaining_milliseconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylRecoveryProgress {
    pub runtime: crate::PipelineV3RunProgress,
    pub frames: UpscaylFrameProgress,
}
