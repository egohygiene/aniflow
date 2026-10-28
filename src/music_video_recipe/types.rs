use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::frame_import::FrameImportConfiguration;
use crate::frame_repair::allenk::AllenkRepairConfiguration;
use crate::frame_selection::InclusiveFrameRange;
use crate::upscayl_recovery::UpscaylRecoveryConfiguration;
use crate::video_assembly::AssemblyConfiguration;
use crate::upscale_target::{UpscaleTargetDecision, UpscaleTargetPolicy, UpscaleTargetResources};

pub const MUSIC_VIDEO_CONFIGURATION_SCHEMA_V1: &str = "aniflow.music-video-recipe/v1";
pub const MUSIC_VIDEO_PLAN_SCHEMA_V1: &str = "aniflow.music-video-plan/v1";
pub const MUSIC_VIDEO_INTENT_SCHEMA_V1: &str = "aniflow.music-video-intent/v1";
pub const MUSIC_VIDEO_STATUS_SCHEMA_V1: &str = "aniflow.music-video-status/v1";
pub const MUSIC_VIDEO_OUTCOME_SCHEMA_V1: &str = "aniflow.music-video-outcome/v1";
pub const MUSIC_VIDEO_RECOVERY_SCHEMA_V1: &str = "aniflow.music-video-recovery/v1";

/// Exactly one importer authority: named repair already includes import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum MusicVideoRepair {
    Disabled { reason: String, import: FrameImportConfiguration },
    Allenk { configuration: AllenkRepairConfiguration, ranges: Vec<InclusiveFrameRange> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum MusicVideoUpscale {
    Disabled { reason: String },
    Upscayl { configuration: UpscaylRecoveryConfiguration },
    TargetResolution { policy: UpscaleTargetPolicy, configuration: UpscaylRecoveryConfiguration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicVideoTargetInputPhase { OriginalInput, PreparedFrames }

/// The initial decision observes original PNG structure. Once preparation is
/// accepted, status and execution bind the decision to that exact frame set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoTargetResolution {
    pub input_phase: MusicVideoTargetInputPhase,
    pub decision: UpscaleTargetDecision,
    pub resources: UpscaleTargetResources,
    /// Only the neural phase's dependencies; import and assembly keep theirs.
    pub required_components: Vec<String>,
}

/// A fixed indexed-input recipe, not a list of arbitrary executable steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoConfiguration {
    pub schema: String,
    pub repair: MusicVideoRepair,
    pub upscale: MusicVideoUpscale,
    pub assembly: AssemblyConfiguration,
}

#[derive(Debug, Clone)]
pub struct MusicVideoRequest {
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: MusicVideoConfiguration,
    pub provider_executable: PathBuf,
}
impl MusicVideoRequest {
    #[must_use]
    pub fn new(source_video: impl Into<PathBuf>, frames_directory: impl Into<PathBuf>, frame_index: impl Into<PathBuf>, configuration: MusicVideoConfiguration, provider_executable: impl Into<PathBuf>) -> Self {
        Self { source_video:source_video.into(),frames_directory:frames_directory.into(),frame_index:frame_index.into(),configuration,provider_executable:provider_executable.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoComponent {
    pub id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
}

/// Immutable caller intent and exact initial identities. Paths locate bytes;
/// only child v3 plans, checkpoints and acceptance records authorize reuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoIntentPayload {
    pub configuration: MusicVideoConfiguration,
    pub source_video: PathBuf,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub provider_executable: PathBuf,
    pub components: Vec<MusicVideoComponent>,
    pub initial_inputs: Vec<crate::PipelineInputIdentity>,
    pub initial_plan_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_resolution: Option<MusicVideoTargetResolution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoIntent {
    pub schema: String,
    pub payload: MusicVideoIntentPayload,
    pub intent_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicVideoPhase { PrepareFrames, Upscale, Assembly }
impl MusicVideoPhase {
    pub(crate) fn id(self) -> &'static str { match self { Self::PrepareFrames => "prepare_frames", Self::Upscale => "upscale", Self::Assembly => "assembly" } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicVideoPhaseState { Disabled, AlreadyTarget, AboveTarget, Pending, Ready, Running, Complete, Failed, Cancelled, IncompleteInitialization }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoPhasePlan {
    pub phase: MusicVideoPhase,
    pub state: MusicVideoPhaseState,
    pub reason: Option<String>,
    /// None means predecessor outputs do not exist yet; no digest is guessed.
    pub plan: Option<crate::PipelineV3Plan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoPlan {
    pub schema: String,
    pub intent: MusicVideoIntent,
    pub output_width: u32,
    pub output_height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_resolution: Option<MusicVideoTargetResolution>,
    pub phases: Vec<MusicVideoPhasePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoPhaseStatus {
    pub phase: MusicVideoPhase,
    pub state: MusicVideoPhaseState,
    pub reason: Option<String>,
    pub run_directory: Option<PathBuf>,
    pub plan_sha256: Option<String>,
    /// Verified by status_v3; never inferred from loose artifacts.
    pub manifest: Option<crate::PipelineV3RunManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoStatus {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_resolution: Option<MusicVideoTargetResolution>,
    pub phases: Vec<MusicVideoPhaseStatus>,
    pub outputs: Vec<crate::PipelineV3Output>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoOutcome {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_resolution: Option<MusicVideoTargetResolution>,
    pub phases: Vec<MusicVideoPhaseStatus>,
    pub outputs: Vec<crate::PipelineV3Output>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MusicVideoProgress {
    Started { run_directory: PathBuf },
    Child { phase: String, run_directory: PathBuf, event: crate::PipelineV3RunProgress },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicVideoRecovery {
    pub schema: String,
    pub run_directory: PathBuf,
}
