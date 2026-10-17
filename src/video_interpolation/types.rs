use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::audio_inspection::AudioToolPin;
use crate::temporal::{RationalTime, TemporalInspection};

pub const RIFE_CONFIGURATION_SCHEMA_V1: &str = "aniflow.rife-configuration/v1";
pub const RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1: &str = "aniflow.rife-provider-configuration/v1";
pub const RIFE_FRAME_INDEX_SCHEMA_V1: &str = "aniflow.rife-frame-index/v1";
pub const RIFE_PLAN_SCHEMA_V1: &str = "aniflow.rife-plan/v1";
pub const RIFE_OUTPUT_INDEX_SCHEMA_V1: &str = "aniflow.rife-output-index/v1";
pub const RIFE_PROVIDER_ID: &str = "org.egohygiene.aniflow.rife";
pub const RIFE_CAPABILITY_ID: &str = "aniflow/rife-interpolation";
pub const RIFE_MAXIMUM_INDEX_BYTES: u64 = 16_777_216;
pub const RIFE_MAXIMUM_FRAMES: u64 = 100_000;

/// Explicit observations supplied by the operator, not a memory reservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeHost {
    pub operating_system: String,
    pub architecture: String,
    pub total_memory_mib: u64,
    pub available_memory_mib: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RifePreset {
    #[default]
    Auto,
    Conservative,
    Balanced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeModelPin {
    pub directory: PathBuf,
    pub param_sha256: String,
    pub param_bytes: u64,
    pub bin_sha256: String,
    pub bin_bytes: u64,
}

/// First bounded profile: midpoint-only 2x, serial pairs, no TTA or UHD claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeSettings {
    pub tool: AudioToolPin,
    pub model: RifeModelPin,
    #[serde(default)]
    pub preset: RifePreset,
    /// -1 explicitly selects CPU; nonnegative selects one Vulkan device.
    #[serde(default = "cpu_device")]
    pub device_id: i32,
    #[serde(default = "pair_timeout")]
    pub tool_timeout_milliseconds: u64,
    #[serde(default = "capture_bytes")]
    pub maximum_tool_output_bytes: u64,
    #[serde(default = "frame_bytes")]
    pub maximum_frame_bytes: u64,
    #[serde(default = "artifact_bytes")]
    pub maximum_artifact_bytes: u64,
}

fn cpu_device() -> i32 { -1 }
fn pair_timeout() -> u64 { 120_000 }
fn capture_bytes() -> u64 { 1_048_576 }
fn frame_bytes() -> u64 { 67_108_864 }
fn artifact_bytes() -> u64 { 68_719_476_736 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeConfiguration {
    pub schema: String,
    #[serde(default)]
    pub enabled: bool,
    pub host: RifeHost,
    pub settings: RifeSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeProviderConfiguration {
    pub schema: String,
    pub configuration: RifeConfiguration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeFrame {
    pub ordinal: u64,
    pub filename: String,
    pub sha256: String,
}

/// Import evidence for already-extracted, RGB8 SDR frames; not native inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeFrameIndex {
    pub schema: String,
    pub temporal_inspection: TemporalInspection,
    pub width: u32,
    pub height: u32,
    /// Only "rgb8_sdr_bt709_square_unrotated" is admitted by this profile.
    pub frame_profile: String,
    pub frames: Vec<RifeFrame>,
    /// Zero-based right-frame ordinals at which a new shot begins.
    pub cut_before_ordinals: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeResources {
    pub preset: RifePreset,
    pub maximum_active_pairs: u32,
    pub threads: String,
    pub padded_width: u64,
    pub padded_height: u64,
    pub maximum_padded_pixels: u64,
    /// Uncalibrated admission heuristic; not measured RSS or a GPU quota.
    pub estimated_working_memory_mib: u64,
    pub working_memory_budget_mib: u64,
    pub memory_estimate_method: String,
    pub memory_limit_enforced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifePlan {
    pub schema: String,
    pub enabled: bool,
    pub configuration_sha256: String,
    pub frame_index_sha256: String,
    pub source_sha256: String,
    pub source_frame_count: u64,
    pub output_frame_count: u64,
    pub source_frame_rate: RationalTime,
    pub output_frame_rate: RationalTime,
    pub duration: RationalTime,
    pub cut_before_ordinals: Vec<u64>,
    pub resources: Option<RifeResources>,
    pub native_qualified: bool,
    pub audio_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RifeFrameOrigin {
    Original,
    InterpolatedMidpoint,
    CutHold,
    EndpointHold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeOutputFrame {
    pub ordinal: u64,
    pub filename: String,
    pub sha256: String,
    pub source_left_ordinal: u64,
    pub source_right_ordinal: u64,
    pub origin: RifeFrameOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeOutputIndex {
    pub schema: String,
    pub plan: RifePlan,
    pub frames: Vec<RifeOutputFrame>,
}
