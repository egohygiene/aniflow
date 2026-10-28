use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use super::{MusicVideoComponent, MusicVideoConfiguration, MusicVideoPhaseState, MusicVideoTargetResolution};

pub const SOURCE_MUSIC_VIDEO_CONFIGURATION_SCHEMA_V1: &str = "aniflow.source-music-video-recipe/v1";
pub const SOURCE_MUSIC_VIDEO_INTENT_SCHEMA_V1: &str = "aniflow.source-music-video-intent/v1";
pub const SOURCE_MUSIC_VIDEO_PLAN_SCHEMA_V1: &str = "aniflow.source-music-video-plan/v1";
pub const SOURCE_MUSIC_VIDEO_STATUS_SCHEMA_V1: &str = "aniflow.source-music-video-status/v1";
pub const SOURCE_MUSIC_VIDEO_OUTCOME_SCHEMA_V1: &str = "aniflow.source-music-video-outcome/v1";

/// Source entry composes the existing extraction and indexed recipe contracts.
/// No arbitrary stage list or speculative frame identity is introduced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoConfiguration {
    pub schema: String,
    pub extraction: crate::frame_extraction::FrameExtractionConfiguration,
    pub recipe: MusicVideoConfiguration,
}

#[derive(Debug, Clone)]
pub struct SourceMusicVideoRequest {
    pub source_video: PathBuf,
    pub configuration: SourceMusicVideoConfiguration,
    pub provider_executable: PathBuf,
}
impl SourceMusicVideoRequest {
    #[must_use]
    pub fn new(source_video: impl Into<PathBuf>, configuration: SourceMusicVideoConfiguration, provider_executable: impl Into<PathBuf>) -> Self {
        Self { source_video:source_video.into(), configuration, provider_executable:provider_executable.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoIntentPayload {
    pub configuration: SourceMusicVideoConfiguration,
    pub source_video: PathBuf,
    pub provider_executable: PathBuf,
    pub components: Vec<MusicVideoComponent>,
    pub initial_inputs: Vec<crate::PipelineInputIdentity>,
    pub initial_plan_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoIntent {
    pub schema: String,
    pub payload: SourceMusicVideoIntentPayload,
    pub intent_sha256: String,
}

/// Source entry has its own phase contract, leaving indexed v1 unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMusicVideoPhase { ExtractFrames, PrepareFrames, Upscale, Assembly }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoPhasePlan {
    pub phase: SourceMusicVideoPhase,
    pub state: MusicVideoPhaseState,
    pub reason: Option<String>,
    pub plan: Option<crate::PipelineV3Plan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoPlan {
    pub schema: String,
    pub intent: SourceMusicVideoIntent,
    pub output_width: Option<u32>,
    pub output_height: Option<u32>,
    pub target_resolution: Option<MusicVideoTargetResolution>,
    pub phases: Vec<SourceMusicVideoPhasePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoPhaseStatus {
    pub phase: SourceMusicVideoPhase,
    pub state: MusicVideoPhaseState,
    pub reason: Option<String>,
    pub run_directory: Option<PathBuf>,
    pub plan_sha256: Option<String>,
    pub manifest: Option<crate::PipelineV3RunManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoStatus {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    pub complete: bool,
    pub output_width: Option<u32>,
    pub output_height: Option<u32>,
    pub target_resolution: Option<MusicVideoTargetResolution>,
    pub phases: Vec<SourceMusicVideoPhaseStatus>,
    pub outputs: Vec<crate::PipelineV3Output>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMusicVideoOutcome {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    pub output_width: u32,
    pub output_height: u32,
    pub target_resolution: Option<MusicVideoTargetResolution>,
    pub phases: Vec<SourceMusicVideoPhaseStatus>,
    pub outputs: Vec<crate::PipelineV3Output>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicVideoEntryKind { Indexed, Source }
