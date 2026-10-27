use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::audio_inspection::{AudioToolObservation, AudioToolPin};
use crate::video_interpolation::RifeFrameIndex;

pub const FRAME_IMPORT_CONFIGURATION_SCHEMA_V1: &str = "aniflow.frame-import-configuration/v1";
pub const FRAME_IMPORT_REPORT_SCHEMA_V1: &str = "aniflow.frame-import-report/v1";
pub const FRAME_IMPORT_PROVIDER_ID: &str = "org.egohygiene.aniflow.frame-import";
pub const FRAME_IMPORT_CAPABILITY_ID: &str = "aniflow/frame-import";
pub const FRAME_IMPORT_MAXIMUM_REPORT_BYTES: u64 = 67_108_864;

/// Exact local decoder authority and finite admission bounds. These are not
/// operating-system memory reservations or a security sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameImportConfiguration {
    pub schema: String,
    pub ffmpeg: AudioToolPin,
    #[serde(default = "timeout")]
    pub tool_timeout_milliseconds: u64,
    #[serde(default = "total_bytes")]
    pub maximum_source_bytes: u64,
    #[serde(default = "frame_bytes")]
    pub maximum_frame_bytes: u64,
    #[serde(default = "total_bytes")]
    pub maximum_total_frame_bytes: u64,
    #[serde(default = "pixels")]
    pub maximum_pixels: u64,
}
fn timeout() -> u64 { 120_000 }
fn total_bytes() -> u64 { 68_719_476_736 }
fn frame_bytes() -> u64 { 67_108_864 }
fn pixels() -> u64 { 8_400_000 }

/// Current observed bytes and decoded RGB samples for one immutable copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedFrameObservation {
    pub ordinal: u64,
    pub filename: String,
    pub sha256: String,
    pub byte_count: u64,
    pub decoded_rgb24_sha256: String,
    pub decoded_byte_count: u64,
}

/// Contemporary import evidence. The caller-declared relationship to source
/// video is bound by its digest, not proven by PNG decode or this report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameImportReport {
    pub schema: String,
    pub stage_id: String,
    pub provider_lock_sha256: String,
    pub configuration_sha256: String,
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub frame_index_sha256: String,
    pub index: RifeFrameIndex,
    pub historical_processing: String,
    pub source_relationship: String,
    pub decode_method: String,
    pub decoder: AudioToolObservation,
    pub frames: Vec<ImportedFrameObservation>,
}

/// Paths are supplied explicitly and are never durable historical authority.
#[derive(Debug, Clone)]
pub struct FrameImportRequest {
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: FrameImportConfiguration,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}

impl FrameImportRequest {
    #[must_use]
    pub fn new(
        source_video: impl Into<PathBuf>,
        frames_directory: impl Into<PathBuf>,
        frame_index: impl Into<PathBuf>,
        configuration: FrameImportConfiguration,
        provider_executable: impl Into<PathBuf>,
    ) -> Self {
        Self {
            source_video: source_video.into(),
            frames_directory: frames_directory.into(),
            frame_index: frame_index.into(),
            configuration,
            provider_executable: provider_executable.into(),
            execution_limits: crate::ProviderExecutionLimits {
                timeout: std::time::Duration::from_secs(21_600),
                termination_grace_period: std::time::Duration::from_millis(2_000),
                maximum_stdout_bytes: 1_048_576,
                maximum_stderr_bytes: 1_048_576,
                maximum_artifact_files: 100_002,
                maximum_artifact_bytes: 68_786_585_600,
            },
        }
    }
}
