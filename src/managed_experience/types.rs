use crate::temporal::StreamSelection;
use crate::video_interpolation::{RifeHost, RifePreset};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MANAGED_OPTIONS_SCHEMA: &str = "aniflow.managed-options/v1";
pub const MANAGED_MUSIC_VIDEO_SCHEMA: &str = "aniflow.managed-music-video/v1";
pub const MANAGED_RIFE_SCHEMA: &str = "aniflow.managed-rife/v1";
pub const MANAGED_EXPERIENCE_PLAN_SCHEMA: &str = "aniflow.managed-experience-plan/v1";

/// Every role resolves independently: command line, project, user, managed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedToolOptions {
    pub executable: Option<PathBuf>,
    pub model_directory: Option<PathBuf>,
    pub model_param: Option<PathBuf>,
    pub model_weights: Option<PathBuf>,
    pub version: Option<String>,
    pub model: Option<String>,
}
impl ManagedToolOptions {
    #[must_use]
    pub fn over(&self, lower: &Self) -> Self {
        Self {
            executable: self.executable.clone().or_else(|| lower.executable.clone()),
            model_directory: self
                .model_directory
                .clone()
                .or_else(|| lower.model_directory.clone()),
            model_param: self.model_param.clone().or_else(|| {
                if self.model_directory.is_some() {
                    None
                } else {
                    lower.model_param.clone()
                }
            }),
            model_weights: self.model_weights.clone().or_else(|| {
                if self.model_directory.is_some() {
                    None
                } else {
                    lower.model_weights.clone()
                }
            }),
            version: self.version.clone().or_else(|| lower.version.clone()),
            model: self.model.clone().or_else(|| lower.model.clone()),
        }
    }
}

/// User configuration is loaded only from an explicitly supplied file. A
/// missing explicit file is an error, never a reason to choose managed bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedOptions {
    pub cache_directory: Option<PathBuf>,
    #[serde(default)]
    pub offline: bool,
    #[serde(default)]
    pub no_download: bool,
    #[serde(default)]
    pub upscayl: ManagedToolOptions,
    #[serde(default)]
    pub rife: ManagedToolOptions,
}
impl ManagedOptions {
    #[must_use]
    pub fn over(&self, lower: &Self) -> Self {
        Self {
            cache_directory: self
                .cache_directory
                .clone()
                .or_else(|| lower.cache_directory.clone()),
            offline: self.offline || lower.offline,
            no_download: self.no_download || lower.no_download,
            upscayl: self.upscayl.over(&lower.upscayl),
            rife: self.rife.over(&lower.rife),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedUserConfiguration {
    pub schema: String,
    pub options: ManagedOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMusicVideoConfiguration {
    pub schema: String,
    #[serde(default)]
    pub tools: ManagedOptions,
    #[serde(default = "source_selection")]
    pub source_selection: StreamSelection,
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    #[serde(default = "output_scale")]
    pub output_scale: u8,
    #[serde(default)]
    pub interpolation: Option<ManagedRifeConfiguration>,
    #[serde(default = "delivery_crf")]
    pub delivery_crf: u8,
    #[serde(default = "preset")]
    pub preset: String,
    #[serde(default = "source_bytes")]
    pub maximum_source_bytes: u64,
    #[serde(default = "video_output_bytes")]
    pub maximum_video_output_bytes: u64,
    #[serde(default = "capture_bytes")]
    pub maximum_capture_bytes: usize,
    #[serde(default = "output_frame_bytes")]
    pub maximum_output_frame_bytes: u64,
    #[serde(default = "output_pixels")]
    pub maximum_output_pixels: u64,
    #[serde(default = "input_frame_bytes")]
    pub maximum_input_frame_bytes: u64,
    #[serde(default = "maximum_frames")]
    pub maximum_frames: u64,
    #[serde(default = "working_bytes")]
    pub maximum_total_frame_bytes: u64,
    #[serde(default = "frames_per_chunk")]
    pub frames_per_chunk: u32,
    #[serde(default = "chunk_timeout")]
    pub chunk_timeout_milliseconds: u64,
    #[serde(default = "working_bytes")]
    pub maximum_working_bytes: u64,
    #[serde(default = "free_bytes")]
    pub minimum_free_bytes: u64,
}
impl Default for ManagedMusicVideoConfiguration {
    fn default() -> Self {
        Self {
            schema: MANAGED_MUSIC_VIDEO_SCHEMA.to_owned(),
            tools: ManagedOptions::default(),
            source_selection: source_selection(),
            ffmpeg: None,
            ffprobe: None,
            output_scale: output_scale(),
            interpolation: None,
            delivery_crf: delivery_crf(),
            preset: preset(),
            maximum_source_bytes: source_bytes(),
            maximum_video_output_bytes: video_output_bytes(),
            maximum_capture_bytes: capture_bytes(),
            maximum_output_frame_bytes: output_frame_bytes(),
            maximum_output_pixels: output_pixels(),
            maximum_input_frame_bytes: input_frame_bytes(),
            maximum_frames: maximum_frames(),
            maximum_total_frame_bytes: working_bytes(),
            frames_per_chunk: frames_per_chunk(),
            chunk_timeout_milliseconds: chunk_timeout(),
            maximum_working_bytes: working_bytes(),
            minimum_free_bytes: free_bytes(),
        }
    }
}
fn source_selection() -> StreamSelection {
    StreamSelection {
        video_stream: Some(0),
        audio_stream: Some(1),
        no_audio: false,
        discard_streams: Vec::new(),
        audio_extent: None,
    }
}
fn output_scale() -> u8 {
    4
}
fn delivery_crf() -> u8 {
    18
}
fn preset() -> String {
    "slow".to_owned()
}
fn working_bytes() -> u64 {
    274_877_906_944
}
fn source_bytes() -> u64 {
    1_073_741_824
}
fn video_output_bytes() -> u64 {
    536_870_912
}
fn capture_bytes() -> usize {
    16_777_216
}
fn output_frame_bytes() -> u64 {
    67_108_864
}
fn output_pixels() -> u64 {
    134_217_728
}
fn input_frame_bytes() -> u64 {
    67_108_864
}
fn maximum_frames() -> u64 {
    100_000
}
fn frames_per_chunk() -> u32 {
    16
}
fn chunk_timeout() -> u64 {
    1_800_000
}
fn free_bytes() -> u64 {
    67_108_864
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedRifeConfiguration {
    pub schema: String,
    #[serde(default)]
    pub tools: ManagedOptions,
    /// Required observed memory values; this is not a reservation or quota.
    pub host: RifeHost,
    #[serde(default = "cpu")]
    pub device_id: i32,
    #[serde(default)]
    pub preset: RifePreset,
}
fn cpu() -> i32 {
    -1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedManagedRuntimeFile {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedManagedTool {
    pub executable: PathBuf,
    pub version: String,
    pub model_id: String,
    pub model_scale: u8,
    pub model_directory: PathBuf,
    pub param_path: PathBuf,
    pub weights_path: PathBuf,
    pub executable_sha256: String,
    pub param_sha256: String,
    pub weights_sha256: String,
    pub param_bytes: u64,
    pub weights_bytes: u64,
    pub runtime_files: Vec<ResolvedManagedRuntimeFile>,
    pub installation_roots: Vec<PathBuf>,
    pub package_id: Option<String>,
    pub package_sha256: Option<String>,
    pub catalog_revision: Option<String>,
}

impl ManagedMusicVideoConfiguration {
    pub fn validate(&self) -> crate::Result<()> {
        use super::resolve::configuration;
        if self.schema != MANAGED_MUSIC_VIDEO_SCHEMA
            || !(2..=4).contains(&self.output_scale)
            || !(2..=256).contains(&self.frames_per_chunk)
            || !(1..=21_600_000).contains(&self.chunk_timeout_milliseconds)
            || !(1..=100_000).contains(&self.maximum_frames)
            || !(1024..=67_108_864).contains(&self.maximum_input_frame_bytes)
            || !(1024..=268_435_456).contains(&self.maximum_output_frame_bytes)
            || !(1..=134_217_728).contains(&self.maximum_output_pixels)
            || !(1_048_576..=1_099_511_627_776).contains(&self.maximum_total_frame_bytes)
            || !(1_048_576..=17_592_186_044_416).contains(&self.maximum_working_bytes)
            || self.minimum_free_bytes > 1_099_511_627_776
            || !(1..=1_099_511_627_776).contains(&self.maximum_source_bytes)
            || !(1024..=1_099_511_627_776).contains(&self.maximum_video_output_bytes)
            || !(65_536..=67_108_864).contains(&self.maximum_capture_bytes)
            || self.delivery_crf > 51
            || !matches!(self.preset.as_str(), "slow" | "medium" | "fast")
        {
            return Err(configuration(
                "managed source settings exceed the supported finite native scale/chunk/resource/delivery profile",
            ));
        }
        let selection = &self.source_selection;
        let mut discarded = selection.discard_streams.clone();
        discarded.sort_unstable();
        discarded.dedup();
        if selection.video_stream.is_none()
            || (selection.no_audio == selection.audio_stream.is_some())
            || selection.video_stream == selection.audio_stream
            || discarded.len() != selection.discard_streams.len()
            || selection
                .video_stream
                .is_some_and(|value| discarded.contains(&value))
            || selection
                .audio_stream
                .is_some_and(|value| discarded.contains(&value))
        {
            return Err(configuration(
                "managed source requires disjoint explicit video/audio-or-no-audio selections and unique discarded streams",
            ));
        }
        if let Some(rife) = &self.interpolation {
            rife.validate()?;
        }
        Ok(())
    }
}
impl ManagedRifeConfiguration {
    pub fn validate(&self) -> crate::Result<()> {
        if self.schema != MANAGED_RIFE_SCHEMA
            || self.host.operating_system != std::env::consts::OS
            || self.host.architecture != std::env::consts::ARCH
            || !matches!(self.host.operating_system.as_str(), "macos" | "linux")
            || !matches!(self.host.architecture.as_str(), "x86_64" | "aarch64")
            || self.host.total_memory_mib < 4096
            || self.host.available_memory_mib == 0
            || self.host.available_memory_mib > self.host.total_memory_mib
            || !(-1..=31).contains(&self.device_id)
        {
            return Err(super::resolve::configuration(
                "managed RIFE requires this supported host, valid observed total/available RAM, and device -1..31",
            ));
        }
        Ok(())
    }
}
