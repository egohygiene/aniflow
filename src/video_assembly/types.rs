use super::AssemblyMediaEvidence;
use crate::audio_inspection::{AudioToolObservation, AudioToolPin};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const ASSEMBLY_CONFIGURATION_SCHEMA_V1: &str = "aniflow.video-assembly-configuration/v1";
pub const ASSEMBLY_OPERATION_SCHEMA_V1: &str = "aniflow.video-assembly-operation/v1";
pub const ASSEMBLY_REPORT_SCHEMA_V1: &str = "aniflow.video-assembly-report/v1";
pub const ASSEMBLY_PROVIDER_ID: &str = "org.egohygiene.aniflow.video-assembly";
pub const ASSEMBLY_MASTER_CAPABILITY_ID: &str = "aniflow/assemble-master";
pub const ASSEMBLY_DELIVERY_CAPABILITY_ID: &str = "aniflow/encode-delivery";
pub const ASSEMBLY_FFMPEG_VERSION: &str = "8.0.1";
pub const ASSEMBLY_MAXIMUM_INDEX_BYTES: u64 = 67_108_864;
pub const ASSEMBLY_MAXIMUM_REPORT_BYTES: u64 = 67_108_864;
pub const ASSEMBLY_MAXIMUM_FRAMES: u64 = 100_000;

/// Two fixed media operations, both owned by ordinary Pipeline v3 stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssemblyStageKind {
    Master,
    Delivery,
}

impl AssemblyStageKind {
    pub(crate) fn stage_id(self) -> &'static str {
        match self {
            Self::Master => "assemble_master",
            Self::Delivery => "encode_delivery",
        }
    }
    pub(crate) fn capability_id(self) -> &'static str {
        match self {
            Self::Master => ASSEMBLY_MASTER_CAPABILITY_ID,
            Self::Delivery => ASSEMBLY_DELIVERY_CAPABILITY_ID,
        }
    }
    pub(crate) fn artifact_id(self) -> &'static str {
        match self {
            Self::Master => "lossless_master",
            Self::Delivery => "h264_delivery",
        }
    }
}

/// Exact local FFmpeg interface and finite admission bounds. No download,
/// arbitrary FFmpeg arguments or audio reencoding are part of this profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyConfiguration {
    pub schema: String,
    pub ffmpeg: AudioToolPin,
    pub ffprobe: AudioToolPin,
    #[serde(default = "timeout")]
    pub timeout_milliseconds: u64,
    #[serde(default = "capture_bytes")]
    pub maximum_capture_bytes: usize,
    #[serde(default = "source_bytes")]
    pub maximum_source_bytes: u64,
    #[serde(default = "output_bytes")]
    pub maximum_output_bytes: u64,
    #[serde(default = "frame_bytes")]
    pub maximum_frame_bytes: u64,
    #[serde(default = "total_frame_bytes")]
    pub maximum_total_frame_bytes: u64,
    #[serde(default = "pixels")]
    pub maximum_pixels: u64,
    #[serde(default = "working_bytes")]
    pub maximum_working_bytes: u64,
    #[serde(default = "free_bytes")]
    pub minimum_free_bytes: u64,
    #[serde(default = "crf")]
    pub delivery_crf: u8,
    #[serde(default = "preset")]
    pub preset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_metadata: Option<crate::delivery_metadata::DeliveryMetadataPolicy>,
}
fn timeout() -> u64 {
    21_600_000
}
fn capture_bytes() -> usize {
    67_108_864
}
fn source_bytes() -> u64 {
    68_719_476_736
}
fn output_bytes() -> u64 {
    137_438_953_472
}
fn frame_bytes() -> u64 {
    268_435_456
}
fn total_frame_bytes() -> u64 {
    274_877_906_944
}
fn pixels() -> u64 {
    134_217_728
}
fn working_bytes() -> u64 {
    2_199_023_255_552
}
fn free_bytes() -> u64 {
    67_108_864
}
fn crf() -> u8 {
    18
}
fn preset() -> String {
    "slow".to_owned()
}

/// Frozen stage-specific admission, bound into the existing provider lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyOperation {
    pub schema: String,
    pub stage: AssemblyStageKind,
    pub settings: AssemblyConfiguration,
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub source_selection: crate::temporal::StreamSelection,
    pub frame_index_sha256: String,
    pub index_schema: String,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_video_clock: Option<crate::temporal::DerivedVideoClock>,
}

/// Contemporary observations, subordinate to the runtime's immutable
/// validation and acceptance records. Imported processing history is unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyReport {
    pub schema: String,
    pub stage: AssemblyStageKind,
    pub stage_id: String,
    pub provider_lock_sha256: String,
    pub configuration_sha256: String,
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub frame_index_sha256: String,
    pub index_schema: String,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    pub output_sha256: String,
    pub output_byte_count: u64,
    pub ffmpeg: AudioToolObservation,
    pub ffprobe: AudioToolObservation,
    pub source_relationship: String,
    pub historical_processing: String,
    pub backend_qualification: String,
    pub media: AssemblyMediaEvidence,
}

#[derive(Debug, Clone)]
pub struct AssemblyRequest {
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: AssemblyConfiguration,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}

impl AssemblyRequest {
    #[must_use]
    pub fn new(
        source_video: impl Into<PathBuf>,
        frames_directory: impl Into<PathBuf>,
        frame_index: impl Into<PathBuf>,
        configuration: AssemblyConfiguration,
        provider_executable: impl Into<PathBuf>,
    ) -> Self {
        // Runtime observation covers the whole candidate root, including
        // private scratch. Its byte envelope is already admitted by assembly.
        let maximum_artifact_bytes = configuration.maximum_working_bytes;
        // At most N private PNGs plus tools (2), source/index (2), delivery
        // master (1), candidate (1), metadata receipt (1), assigned outputs (2)
        // and failure receipt (1). Some slots are mutually exclusive.
        let maximum_artifact_files = ASSEMBLY_MAXIMUM_FRAMES.saturating_add(10);
        Self {
            source_video: source_video.into(),
            frames_directory: frames_directory.into(),
            frame_index: frame_index.into(),
            configuration,
            provider_executable: provider_executable.into(),
            execution_limits: crate::ProviderExecutionLimits {
                timeout: std::time::Duration::from_secs(86_400),
                termination_grace_period: std::time::Duration::from_millis(2_000),
                maximum_stdout_bytes: 1_048_576,
                maximum_stderr_bytes: 1_048_576,
                maximum_artifact_files,
                maximum_artifact_bytes,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_cover_admitted_private_work_without_widening_the_working_envelope() {
        let configuration: AssemblyConfiguration = serde_json::from_value(serde_json::json!({
            "schema":ASSEMBLY_CONFIGURATION_SCHEMA_V1,
            "ffmpeg":{"executable":"/synthetic/ffmpeg","version":"8.0.1","sha256":"a".repeat(64)},
            "ffprobe":{"executable":"/synthetic/ffprobe","version":"8.0.1","sha256":"b".repeat(64)},
            "maximum_working_bytes":2147483648_u64,
            "maximum_source_bytes":16777216,
            "maximum_output_bytes":33554432,
            "maximum_total_frame_bytes":33554432,
            "maximum_capture_bytes":4194304
        }))
        .unwrap();
        let admitted = crate::video_assembly::checked_working_bytes(
            &configuration,
            1_048_576,
            16_777_216,
            33_554_432,
        )
        .unwrap();
        let request = AssemblyRequest::new(
            "source",
            "frames",
            "index",
            configuration.clone(),
            "provider",
        );
        let private_fixed_files = 2 + 2 + 1 + 1 + 1 + 2 + 1;
        assert!(
            request.execution_limits.maximum_artifact_files
                >= ASSEMBLY_MAXIMUM_FRAMES + private_fixed_files
        );
        assert!(request.execution_limits.maximum_artifact_bytes >= admitted);
        assert_eq!(
            request.execution_limits.maximum_artifact_bytes,
            configuration.maximum_working_bytes
        );
        assert!(admitted > configuration.maximum_output_bytes + ASSEMBLY_MAXIMUM_REPORT_BYTES);
    }
}
