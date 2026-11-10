//! Inert typed setup for the selected frame-repair and Upscayl facades.
//!
//! Local file identity and supplied observations are inspected without reading
//! media, launching tools, writing configuration or changing provider authority.
//! Profile settings are the sole source of processing options. A ready result
//! is preparation evidence only; normal planning and fresh runtime checks remain
//! authoritative, and host/runtime and native qualification stay unestablished.

use std::collections::BTreeSet;
mod workflow;
pub use workflow::*;
pub use workflow::bundle as workflow_bundle;
mod export;
mod core;
pub use core::*;
mod publishing;
pub use publishing::*;
pub use export::DestinationVideoSetupOptions;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    ToolchainArtifactInventory, ToolchainCapability, ToolchainDependencyKind,
    ToolchainInventory, ToolchainObservation, ToolchainProfile, ToolchainRegistrationFile,
    ToolchainReport, ToolchainScaleMode, inspect_profile,
};
use crate::audio_inspection::AudioToolPin;
use crate::frame_import::{FRAME_IMPORT_CONFIGURATION_SCHEMA_V1, FrameImportConfiguration};
use crate::frame_repair::allenk::{
    ALLENK_INTERFACE_REVISION, ALLENK_INTERFACE_VERSION,
    ALLENK_REPAIR_CONFIGURATION_SCHEMA_V1, AllenkRepairConfiguration, AllenkWatermarkProfile,
};
use crate::upscayl_recovery::{
    UPSCAYL_INTERFACE_REVISION, UPSCAYL_RECOVERY_CONFIGURATION_SCHEMA_V1,
    UpscaylExecutionPolicy, UpscaylModelFile, UpscaylModelPin, UpscaylRecoveryConfiguration,
};
use crate::{Error, ErrorCategory, Result, SideEffect};
use crate::video_interpolation::{RifeConfiguration,RifeHost,RifePreset,RifeSettings,RifeModelPin,RIFE_CONFIGURATION_SCHEMA_V1};

pub const MEDIA_SETUP_REQUEST_SCHEMA: &str = "aniflow.toolchain.media-setup/v1";
pub const MEDIA_SETUP_PREPARATION_SCHEMA: &str = "aniflow.toolchain.media-setup-preparation/v1";
const MAXIMUM_REQUEST_BYTES: usize = 1_048_576;
const MAXIMUM_PREPARATION_BYTES: usize = 8 * 1_048_576;
const MAXIMUM_TOOL_BYTES: u64 = 134_217_728;
const DECODER_FLAGS: &[&str] = &[
    "-version", "-hide_banner", "-nostdin", "-v", "-xerror", "-err_detect",
    "-threads", "-protocol_whitelist", "-f", "-pattern_type", "-c:v", "-i",
    "-map", "-an", "-sn", "-dn", "-pix_fmt", "-format_version", "-hash",
];
const DECODER_FEATURES: &[&str] = &[
    "decoder:png", "encoder:rawvideo", "demuxer:image2", "muxer:framehash",
    "protocol:file", "protocol:pipe", "pixel_format:rgb24", "hash:sha256",
];
const ALLENK_FLAGS: &[&str] = &[
    "--version", "--input", "--output", "--remove", "--no-banner", "--threshold",
];
const UPSCAYL_FLAGS: &[&str] = &["-i", "-o", "-m", "-n", "-z", "-s", "-t", "-g", "-j", "-f"];
const RIFE_FLAGS:&[&str]=&["-0","-1","-o","-m","-s","-g","-j"];

/// Exactly one explicitly chosen typed mapping. IDs reference the supplied
/// profile and inventory; they never search PATH or select ambient providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MediaSetupSelection {
    FrameExtraction {ffmpeg_dependency_id:String,ffprobe_dependency_id:String},
    CleanVideoAssembly {ffmpeg_dependency_id:String,ffprobe_dependency_id:String},
    FinalLosslessAudioQc {ffmpeg_dependency_id:String,ffprobe_dependency_id:String},
    FinalVideoAudioQc {ffmpeg_dependency_id:String,ffprobe_dependency_id:String},
    MetadataRemux {ffmpeg_dependency_id:String,ffprobe_dependency_id:String},
    LosslessAudioExport {ffmpeg_dependency_id:String,ffprobe_dependency_id:String},
    DestinationVideo { ffmpeg_dependency_id:String,ffprobe_dependency_id:String },
    CaptionRendering { ffmpeg_dependency_id:String,ffprobe_dependency_id:String },
    RifeInterpolation { processor_dependency_id:String,param_dependency_id:String,weights_dependency_id:String },
    AllenkImageRepair {
        ffmpeg_dependency_id: String,
        processor_dependency_id: String,
    },
    UpscaylRecovery {
        ffmpeg_dependency_id: String,
        processor_dependency_id: String,
        param_dependency_id: String,
        weights_dependency_id: String,
    },
}

/// Pure import policy, without a duplicate decoder pin or source-media path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaImportOptions {
    pub tool_timeout_milliseconds: u64,
    pub maximum_source_bytes: u64,
    pub maximum_frame_bytes: u64,
    pub maximum_total_frame_bytes: u64,
    pub maximum_pixels: u64,
}

/// Complete selected capability settings; every field is explicit in setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllenkSetupOptions {
    pub import: MediaImportOptions,
    pub watermark_profile: AllenkWatermarkProfile,
    pub detection_threshold_percent: u8,
    pub maximum_working_bytes: u64,
}

/// Native model scale and requested output scale are distinct declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaylSetupOptions {
    pub import: MediaImportOptions,
    pub model_name: String,
    pub native_scale: u8,
    pub output_scale: u8,
    pub tile_size: u32,
    pub gpu_id: u32,
    pub threads: String,
    pub tta: bool,
    pub maximum_output_frame_bytes: u64,
    pub maximum_output_pixels: u64,
    pub maximum_working_bytes: u64,
    pub minimum_free_bytes: u64,
}

/// Explicit supplied host observations and fixed serial midpoint policy.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RifeSetupOptions {
    pub host:RifeHost,pub preset:RifePreset,pub device_id:i32,
    pub tool_timeout_milliseconds:u64,pub maximum_tool_output_bytes:u64,
    pub maximum_frame_bytes:u64,pub maximum_artifact_bytes:u64,
}
/// Existing assembly settings with executable pins supplied only by the two
/// selected inventory roles. No source media or metadata edit is selected here.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinishingMediaOptions {
    pub timeout_milliseconds:u64,pub maximum_capture_bytes:usize,
    pub maximum_source_bytes:u64,pub maximum_output_bytes:u64,pub maximum_frame_bytes:u64,
    pub maximum_total_frame_bytes:u64,pub maximum_pixels:u64,pub maximum_working_bytes:u64,
    pub minimum_free_bytes:u64,pub delivery_crf:u8,pub preset:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionSetupOptions {
    pub media:FinishingMediaOptions,pub mode:crate::subtitle_delivery::CaptionMode,
    /// Font is a typed local asset, not an executable or neural model dependency.
    pub font:crate::subtitle_delivery::CaptionFont,pub style:crate::subtitle_delivery::CaptionStyle,
    pub preview_cue_indices:Vec<usize>,
}
fn placeholder_tool(name:&str)->AudioToolPin {AudioToolPin {executable:format!("/declared/{name}").into(),version:"8.0.1".into(),sha256:"0".repeat(64)}}
impl CaptionSetupOptions {
    fn configuration(&self,ffmpeg:AudioToolPin,ffprobe:AudioToolPin)->crate::subtitle_delivery::CaptionConfiguration {
        let m=&self.media;
        crate::subtitle_delivery::CaptionConfiguration {schema:"aniflow.caption-configuration/v1".into(),mode:self.mode,font:self.font.clone(),style:self.style.clone(),preview_cue_indices:self.preview_cue_indices.clone(),
            settings:crate::video_assembly::AssemblyConfiguration {schema:crate::video_assembly::ASSEMBLY_CONFIGURATION_SCHEMA_V1.into(),ffmpeg,ffprobe,
                timeout_milliseconds:m.timeout_milliseconds,maximum_capture_bytes:m.maximum_capture_bytes,maximum_source_bytes:m.maximum_source_bytes,maximum_output_bytes:m.maximum_output_bytes,maximum_frame_bytes:m.maximum_frame_bytes,maximum_total_frame_bytes:m.maximum_total_frame_bytes,maximum_pixels:m.maximum_pixels,maximum_working_bytes:m.maximum_working_bytes,minimum_free_bytes:m.minimum_free_bytes,delivery_crf:m.delivery_crf,preset:m.preset.clone(),delivery_metadata:None}}
    }
}
/// Interface needs are attributed supplied build observations, not live probes.
fn caption_requirements(mode:crate::subtitle_delivery::CaptionMode)->(Vec<&'static str>,Vec<&'static str>) {
    let mut flags=vec!["-version","-hide_banner","-nostdin","-v","-xerror","-err_detect","-threads","-protocol_whitelist","-noautorotate","-copyts","-i","-map","-an","-sn","-dn","-filter_script:v","-frames:v","-fps_mode","-map_metadata","-map_chapters","-codec:v","-pix_fmt","-n","-f","-avoid_negative_ts","-use_editlist","-movflags","-codec:a"];
    let mut features=vec!["protocol:file","protocol:pipe","demuxer:mov","decoder:h264","decoder:aac","encoder:png","muxer:image2","muxer:mp4","muxer:null","filter:drawtext","filter:select","pixel_format:rgb24","pixel_format:yuv420p","hash:sha256"];
    match mode {
        crate::subtitle_delivery::CaptionMode::SoftMux=>{flags.push("-codec:s");features.extend(["demuxer:srt","encoder:mov_text","decoder:mov_text","encoder:srt","muxer:srt"]);},
        crate::subtitle_delivery::CaptionMode::BurnIn=>{flags.extend(["-preset","-crf","-color_primaries","-color_trc","-colorspace","-color_range"]);features.push("encoder:libx264");},
    }(flags,features)
}
impl RifeSetupOptions {
    fn configuration(&self,tool:AudioToolPin,model:RifeModelPin)->RifeConfiguration {
        RifeConfiguration {schema:RIFE_CONFIGURATION_SCHEMA_V1.into(),enabled:true,host:self.host.clone(),settings:RifeSettings {
            tool,model,preset:self.preset,device_id:self.device_id,tool_timeout_milliseconds:self.tool_timeout_milliseconds,
            maximum_tool_output_bytes:self.maximum_tool_output_bytes,maximum_frame_bytes:self.maximum_frame_bytes,maximum_artifact_bytes:self.maximum_artifact_bytes}}
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaSetupRequest {
    pub schema: String,
    pub profile: ToolchainProfile,
    pub inventory: ToolchainInventory,
    pub capability_id: String,
    pub selection: MediaSetupSelection,
}

/// One reviewable existing-format configuration is returned only when selected
/// inventory and typed mapping agree. Deserialization confers no authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaSetupPreparation {
    pub schema: String,
    pub request_sha256: String,
    pub ready: bool,
    pub native_qualification: bool,
    pub host_runtime_evidence: String,
    pub inspection: ToolchainReport,
    pub files: Vec<ToolchainRegistrationFile>,
    pub diagnostics: Vec<String>,
}

enum Options {
    Extract(FrameExtractionSetupOptions),
    Assembly(FinishingMediaOptions),
    FinalLosslessQc(FinalLosslessQcSetupOptions),
    FinalQc(FinalVideoAudioQcSetupOptions),
    Metadata(MetadataRemuxSetupOptions),
    Lossless(LosslessAudioSetupOptions),
    Destination(DestinationVideoSetupOptions),
    Captions(CaptionSetupOptions),
    Rife(RifeSetupOptions),
    Allenk(AllenkSetupOptions),
    Upscayl(UpscaylSetupOptions),
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

impl MediaSetupSelection {
    fn dependencies(&self) -> Vec<(&str, ToolchainDependencyKind)> {
        match self {
            Self::FrameExtraction {ffmpeg_dependency_id,ffprobe_dependency_id}|Self::CleanVideoAssembly {ffmpeg_dependency_id,ffprobe_dependency_id}|Self::FinalLosslessAudioQc {ffmpeg_dependency_id,ffprobe_dependency_id}|Self::FinalVideoAudioQc {ffmpeg_dependency_id,ffprobe_dependency_id}|Self::MetadataRemux {ffmpeg_dependency_id,ffprobe_dependency_id}|Self::LosslessAudioExport {ffmpeg_dependency_id,ffprobe_dependency_id}=>vec![(ffmpeg_dependency_id,ToolchainDependencyKind::Tool),(ffprobe_dependency_id,ToolchainDependencyKind::Tool)],
            Self::DestinationVideo {ffmpeg_dependency_id,ffprobe_dependency_id}=>vec![(ffmpeg_dependency_id,ToolchainDependencyKind::Tool),(ffprobe_dependency_id,ToolchainDependencyKind::Tool)],
            Self::CaptionRendering {ffmpeg_dependency_id,ffprobe_dependency_id}=>vec![(ffmpeg_dependency_id,ToolchainDependencyKind::Tool),(ffprobe_dependency_id,ToolchainDependencyKind::Tool)],
            Self::RifeInterpolation {processor_dependency_id,param_dependency_id,weights_dependency_id}=>vec![
                (processor_dependency_id,ToolchainDependencyKind::Tool),(param_dependency_id,ToolchainDependencyKind::Model),(weights_dependency_id,ToolchainDependencyKind::Model)],
            Self::AllenkImageRepair { ffmpeg_dependency_id, processor_dependency_id } => vec![
                (ffmpeg_dependency_id, ToolchainDependencyKind::Tool),
                (processor_dependency_id, ToolchainDependencyKind::Tool),
            ],
            Self::UpscaylRecovery { ffmpeg_dependency_id, processor_dependency_id, param_dependency_id, weights_dependency_id } => vec![
                (ffmpeg_dependency_id, ToolchainDependencyKind::Tool),
                (processor_dependency_id, ToolchainDependencyKind::Tool),
                (param_dependency_id, ToolchainDependencyKind::Model),
                (weights_dependency_id, ToolchainDependencyKind::Model),
            ],
        }
    }
}

impl MediaSetupRequest {
    /// Read one bounded nonsymlink declaration, without examining source media.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_json_slice(&super::types::read_json(path.as_ref())?)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAXIMUM_REQUEST_BYTES {
            return Err(invalid("media setup request exceeds one MiB"));
        }
        let request: Self = super::types::decode_unique_json(bytes)?;
        request.validate()?;
        Ok(request)
    }

    fn capability(&self) -> Result<&ToolchainCapability> {
        self.profile.capabilities.iter().find(|value| value.id == self.capability_id)
            .ok_or_else(|| invalid("media setup requires one explicit existing capability"))
    }

    fn options(&self) -> Result<Options> {
        let value = serde_json::to_value(&self.capability()?.effective_settings)
            .map_err(|_| invalid("cannot encode selected media settings"))?;
        match self.selection {
            MediaSetupSelection::FrameExtraction {..}=>serde_json::from_value(value).map(Options::Extract),
            MediaSetupSelection::CleanVideoAssembly {..}=>serde_json::from_value(value).map(Options::Assembly),
            MediaSetupSelection::FinalLosslessAudioQc {..}=>serde_json::from_value(value).map(Options::FinalLosslessQc),
            MediaSetupSelection::FinalVideoAudioQc {..}=>serde_json::from_value(value).map(Options::FinalQc),
            MediaSetupSelection::MetadataRemux {..}=>serde_json::from_value(value).map(Options::Metadata),
            MediaSetupSelection::LosslessAudioExport {..}=>serde_json::from_value(value).map(Options::Lossless),
            MediaSetupSelection::DestinationVideo {..}=>serde_json::from_value(value).map(Options::Destination),
            MediaSetupSelection::CaptionRendering {..}=>serde_json::from_value(value).map(Options::Captions),
            MediaSetupSelection::RifeInterpolation {..}=>serde_json::from_value(value).map(Options::Rife),
            MediaSetupSelection::AllenkImageRepair { .. } => serde_json::from_value(value).map(Options::Allenk),
            MediaSetupSelection::UpscaylRecovery { .. } => serde_json::from_value(value).map(Options::Upscayl),
        }.map_err(|error| invalid(format!("selected capability requires exact closed media options: {error}")))
    }

    /// Validate declarations before any dependency file is inspected. Missing
    /// installed assets remain preparation diagnostics, not malformed requests.
    pub fn validate(&self) -> Result<()> {
        if self.schema != MEDIA_SETUP_REQUEST_SCHEMA {
            return Err(invalid("unsupported media setup request schema"));
        }
        self.profile.validate()?;
        self.inventory.validate()?;
        let capability = self.capability()?;
        let mapped = self.selection.dependencies();
        let ids: BTreeSet<_> = mapped.iter().map(|(id, _)| *id).collect();
        if ids.len() != mapped.len() || ids != capability.dependency_ids.iter().map(String::as_str).collect() {
            return Err(invalid("selected media capability dependencies must exactly equal distinct mapped roles"));
        }
        for (id, kind) in mapped {
            if !self.profile.dependencies.iter().any(|dependency| dependency.id == id && dependency.kind == kind) {
                return Err(invalid("media dependency mappings require the exact declared tool/model kinds"));
            }
        }
        let mut effects = BTreeSet::from([
            SideEffect::FilesystemRead, SideEffect::FilesystemWrite,
            SideEffect::EnvironmentRead, SideEffect::Subprocess,
        ]);
        match self.options()? {
            Options::Extract(options)=>{options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe")).validate()?;if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("core extraction has no neural backend/model scale"));}},
            Options::Assembly(options)=>{options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe"),None).validate()?;if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("core assembly has no neural backend/model scale"));}},
            Options::FinalLosslessQc(options)=>{options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe"))?;if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("final lossless QC has no neural backend/scale"));}},
            Options::FinalQc(options)=>{
                options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe"))?;
                if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("final audio QC has no neural backend/scale"));}
            },
            Options::Metadata(options)=>{
                options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe"))?;
                if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("metadata remux has no neural backend/scale"));}
            },
            Options::Lossless(options)=>{
                options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe")).validate()?;
                if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("lossless audio export has no neural backend/scale"));}
            },
            Options::Destination(options)=>{
                options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe")).validate()?;
                if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("destination video uses a fixed CPU codec profile without a neural model/backend scale"));}
            },
            Options::Captions(options)=>{
                options.configuration(placeholder_tool("ffmpeg"),placeholder_tool("ffprobe")).validate()?;
                if capability.backend.is_some()||capability.scale.is_some(){return Err(invalid("caption rendering uses the fixed CPU native profile without a neural model/spatial scale"));}
            },
            Options::Rife(options)=>{
                let placeholder=AudioToolPin {executable:"/declared/rife".into(),version:"1.0.0".into(),sha256:"0".repeat(64)};
                options.configuration(placeholder,RifeModelPin {directory:"/declared/rife-v4.6".into(),param_sha256:"0".repeat(64),param_bytes:1,bin_sha256:"0".repeat(64),bin_bytes:1}).validate()?;
                if options.host.operating_system!=self.inventory.platform.os||options.host.architecture!=self.inventory.platform.arch||capability.scale.is_some(){return Err(invalid("RIFE host must match supplied inventory; temporal 2x is not a spatial model scale"));}
                effects.insert(SideEffect::Ai);
                if options.device_id>=0 {
                    effects.insert(SideEffect::Gpu);
                    if capability.backend.as_ref().is_none_or(|b|b.name!="vulkan"||!b.required_features.iter().any(|s|s=="ncnn-vulkan")){return Err(invalid("RIFE GPU selection requires an explicit ncnn-vulkan backend"));}
                } else if capability.backend.is_some(){return Err(invalid("RIFE CPU device -1 cannot carry a GPU backend declaration"));}
            },
            Options::Allenk(options) => {
                options.import.validate()?;
                crate::frame_repair::allenk::validate_threshold(options.detection_threshold_percent)
                    .map_err(|error| invalid(error.to_string()))?;
                crate::frame_repair::validate_working_bytes(options.maximum_working_bytes)
                    .map_err(|error| invalid(error.to_string()))?;
                if capability.backend.is_some() || capability.scale.is_some() {
                    return Err(invalid("basic allenk image repair has no model scale or GPU backend mapping"));
                }
            }
            Options::Upscayl(options) => {
                options.import.validate()?;
                crate::upscayl_recovery::validate_processing_policy(&options.model_name, options.native_scale,
                    options.output_scale, options.tile_size, options.gpu_id, &options.threads)
                    .map_err(|error| invalid(error.to_string()))?;
                crate::upscayl_recovery::validate_resource_policy(options.maximum_output_frame_bytes,
                    options.maximum_output_pixels, options.maximum_working_bytes, options.minimum_free_bytes)
                    .map_err(|error| invalid(error.to_string()))?;
                effects.insert(SideEffect::Gpu);
                let backend = capability.backend.as_ref()
                    .ok_or_else(|| invalid("Upscayl setup requires an explicit Vulkan backend"))?;
                if backend.name != "vulkan" || !backend.required_features.iter().any(|value| value == "ncnn-vulkan") {
                    return Err(invalid("Upscayl backend must declare vulkan with ncnn-vulkan support"));
                }
                let mode = if options.output_scale == options.native_scale { ToolchainScaleMode::Native } else { ToolchainScaleMode::PostResize };
                if capability.scale.as_ref().is_none_or(|scale| scale.native != u32::from(options.native_scale)
                    || scale.requested != u32::from(options.output_scale) || scale.mode != mode) {
                    return Err(invalid("Upscayl profile scale must match separate native/output settings and resize mode"));
                }
                for (id, kind) in self.selection.dependencies() {
                    if kind == ToolchainDependencyKind::Model
                        && !self.profile.dependencies.iter().any(|dependency| dependency.id == id && dependency.native_scale == Some(4)) {
                        return Err(invalid("both Upscayl model dependencies require declared native scale 4"));
                    }
                }
            }
        }
        if effects != capability.side_effects.iter().copied().collect() {
            return Err(invalid("selected media side effects must exactly match the typed adapter"));
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAXIMUM_REQUEST_BYTES { return Err(invalid("media setup request exceeds one MiB")); }
        Ok(bytes)
    }

    pub fn sha256(&self) -> Result<String> {
        self.canonical_json_bytes()?;
        crate::provider::canonical_sha256(self)
    }
}

impl MediaSetupPreparation {
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAXIMUM_PREPARATION_BYTES {
            return Err(invalid("media setup preparation exceeds eight MiB"));
        }
        Ok(bytes)
    }
}

impl MediaImportOptions {
    fn validate(&self) -> Result<()> {
        crate::frame_import::validate_import_policy(self.tool_timeout_milliseconds, self.maximum_source_bytes,
            self.maximum_frame_bytes, self.maximum_total_frame_bytes, self.maximum_pixels)
            .map_err(|error| invalid(error.to_string()))
    }

    fn configuration(&self, ffmpeg: AudioToolPin) -> FrameImportConfiguration {
        FrameImportConfiguration {
            schema: FRAME_IMPORT_CONFIGURATION_SCHEMA_V1.to_owned(), ffmpeg,
            tool_timeout_milliseconds: self.tool_timeout_milliseconds,
            maximum_source_bytes: self.maximum_source_bytes,
            maximum_frame_bytes: self.maximum_frame_bytes,
            maximum_total_frame_bytes: self.maximum_total_frame_bytes,
            maximum_pixels: self.maximum_pixels,
        }
    }
}

fn artifact<'a>(request: &'a MediaSetupRequest, id: &str) -> Result<&'a ToolchainArtifactInventory> {
    request.inventory.artifacts.iter().find(|value| value.dependency_id == id)
        .ok_or_else(|| invalid(format!("{id}: supply an explicit local inventory artifact")))
}

fn observation<'a>(artifact: &'a ToolchainArtifactInventory, id: &str) -> Result<&'a ToolchainObservation> {
    let observation = artifact.observation.as_ref()
        .ok_or_else(|| invalid(format!("{id}: supply a digest-bound observation")))?;
    if observation.executable_sha256 != artifact.expected_sha256 {
        return Err(invalid(format!("{id}: supplied observation differs from its inventory digest")));
    }
    if observation.package_revision.is_none() {
        return Err(invalid(format!("{id}: supply an attributed package/model revision")));
    }
    Ok(observation)
}

fn includes(actual: &[String], required: &[&str], id: &str, kind: &str) -> Result<()> {
    let missing: Vec<_> = required.iter().filter(|value| !actual.iter().any(|item| item == **value)).collect();
    if !missing.is_empty() {
        return Err(invalid(format!("{id}: typed adapter requires supplied {kind} {missing:?}; no probe was run")));
    }
    Ok(())
}

fn tool_pin(request: &MediaSetupRequest, id: &str, flags: &[&str], features: &[&str], revision: Option<&str>) -> Result<AudioToolPin> {
    let artifact = artifact(request, id)?;
    let observed = observation(artifact, id)?;
    let version = observed.version.as_ref()
        .ok_or_else(|| invalid(format!("{id}: supply the exact semantic version declaration; no tool was launched")))?;
    crate::provider::validate_semantic_version(version, "media tool version")?;
    if revision.is_some_and(|required| observed.package_revision.as_deref() != Some(required)) {
        return Err(invalid(format!("{id}: supplied package revision differs from the selected adapter interface")));
    }
    includes(&observed.flags, flags, id, "flags")?;
    includes(&observed.features, features, id, "features")?;
    let metadata = std::fs::symlink_metadata(&artifact.path)
        .map_err(|_| invalid(format!("{id}: mapped executable is unavailable")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink()
        || metadata.len() == 0 || metadata.len() > MAXIMUM_TOOL_BYTES {
        return Err(invalid(format!("{id}: mapped tool must be a nonempty nonsymlink regular file within 128 MiB")));
    }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(invalid(format!("{id}: mapped tool file has no executable permission")));
        }
    }
    Ok(AudioToolPin { executable: artifact.path.clone(), version: version.clone(), sha256: artifact.expected_sha256.clone() })
}

fn prepare_configuration(request: &MediaSetupRequest) -> Result<Value> {
    // The processing facades refuse noncanonical paths, including symlinked
    // ancestors. Reuse selected-role inventory identities; the existing caption
    // owner separately admits the explicitly pinned, bounded local font.
    let mut paths = BTreeSet::new();
    for (id, _) in request.selection.dependencies() {
        let artifact = artifact(request, id)?;
        if artifact.path.canonicalize().ok().as_deref() != Some(artifact.path.as_path()) {
            return Err(invalid(format!("{id}: use the canonical dependency path without symlink ancestors")));
        }
        if !paths.insert(&artifact.path) {
            return Err(invalid("distinct media dependency roles require distinct local artifact paths"));
        }
    }
    let value = match (&request.selection, request.options()?) {
        (MediaSetupSelection::FrameExtraction {ffmpeg_dependency_id,ffprobe_dependency_id},Options::Extract(options))=>{
            let (flags,features)=core::extraction_requirements();let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let ffprobe=tool_pin(request,ffprobe_dependency_id,export::PROBE_FLAGS,&["demuxer:mov","protocol:file","protocol:pipe","decoder:h264","decoder:aac"],None)?;
            let configuration=options.configuration(ffmpeg,ffprobe);configuration.validate()?;serde_json::to_value(configuration)
        },
        (MediaSetupSelection::CleanVideoAssembly {ffmpeg_dependency_id,ffprobe_dependency_id},Options::Assembly(options))=>{
            let (flags,features)=core::assembly_requirements();let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let ffprobe=tool_pin(request,ffprobe_dependency_id,export::PROBE_FLAGS,export::PROBE_FEATURES,None)?;
            let configuration=options.configuration(ffmpeg,ffprobe,None);configuration.validate()?;serde_json::to_value(configuration)
        },
        (MediaSetupSelection::FinalLosslessAudioQc {ffmpeg_dependency_id,ffprobe_dependency_id},Options::FinalLosslessQc(options))=>{
            let (flags,features)=publishing::final_lossless_qc_requirements(options.format);let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let mut probe=vec!["protocol:file","protocol:pipe","demuxer:wav","decoder:pcm_s16le","decoder:pcm_s24le","decoder:pcm_f32le"];if options.format==crate::lossless_audio::LosslessFormat::Flac{probe.extend(["demuxer:flac","decoder:flac"]);}
            let ffprobe=tool_pin(request,ffprobe_dependency_id,&["-version","-v","-protocol_whitelist","-show_streams","-show_format","-show_entries","-of","-i"],&probe,None)?;
            Ok(options.configuration(ffmpeg,ffprobe)?)
        },
        (MediaSetupSelection::FinalVideoAudioQc {ffmpeg_dependency_id,ffprobe_dependency_id},Options::FinalQc(options))=>{
            let (flags,features)=publishing::qc_requirements();
            let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let ffprobe=tool_pin(request,ffprobe_dependency_id,export::PROBE_FLAGS,export::PROBE_FEATURES,None)?;
            Ok(options.configuration(ffmpeg,ffprobe)?)
        },
        (MediaSetupSelection::MetadataRemux {ffmpeg_dependency_id,ffprobe_dependency_id},Options::Metadata(options))=>{
            let (flags,features)=publishing::metadata_requirements(options.subtitles);
            let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let mut probe_features=export::PROBE_FEATURES.to_vec();if options.subtitles==MetadataSubtitleSetup::PreserveAcceptedSoftSubtitle {probe_features.push("decoder:mov_text");}
            let ffprobe=tool_pin(request,ffprobe_dependency_id,export::PROBE_FLAGS,&probe_features,None)?;
            Ok(options.configuration(ffmpeg,ffprobe)?)
        },
        (MediaSetupSelection::LosslessAudioExport {ffmpeg_dependency_id,ffprobe_dependency_id},Options::Lossless(options))=>{
            let (flags,features)=publishing::lossless_requirements(options.format);
            let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let probe_features=if options.format==crate::lossless_audio::LosslessFormat::Flac {vec!["protocol:file","protocol:pipe","demuxer:flac","decoder:flac"]}else{vec!["protocol:file","protocol:pipe","demuxer:wav","decoder:pcm_s16le","decoder:pcm_s24le","decoder:pcm_f32le"]};
            let ffprobe=tool_pin(request,ffprobe_dependency_id,&["-version","-v","-protocol_whitelist","-show_entries","-of","-i"],&probe_features,None)?;
            let configuration=options.configuration(ffmpeg,ffprobe);configuration.validate()?;serde_json::to_value(configuration)
        },
        (MediaSetupSelection::DestinationVideo {ffmpeg_dependency_id,ffprobe_dependency_id},Options::Destination(options))=>{
            let (flags,features)=export::requirements(&options.geometry);
            let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let ffprobe=tool_pin(request,ffprobe_dependency_id,export::PROBE_FLAGS,export::PROBE_FEATURES,None)?;
            let configuration=options.configuration(ffmpeg,ffprobe);configuration.validate()?;
            serde_json::to_value(configuration)
        },
        (MediaSetupSelection::CaptionRendering {ffmpeg_dependency_id,ffprobe_dependency_id},Options::Captions(options))=>{
            let (flags,features)=caption_requirements(options.mode);
            let ffmpeg=tool_pin(request,ffmpeg_dependency_id,&flags,&features,None)?;
            let ffprobe=tool_pin(request,ffprobe_dependency_id,&["-version","-v","-protocol_whitelist","-select_streams","-show_streams","-show_format","-show_chapters","-show_programs","-show_frames","-show_packets","-show_data_hash","-show_entries","-of","-i"],&["demuxer:mov","protocol:file","protocol:pipe","decoder:h264","decoder:aac"],None)?;
            let configuration=options.configuration(ffmpeg,ffprobe);configuration.validate()?;
            crate::subtitle_delivery::observe_font(&configuration.font)?;
            serde_json::to_value(configuration)
        },
        (MediaSetupSelection::RifeInterpolation {processor_dependency_id,param_dependency_id,weights_dependency_id},Options::Rife(options))=>{
            let features=if options.device_id<0 {vec!["model-pair-loading","cpu"]}else{vec!["model-pair-loading","vulkan"]};
            let processor=tool_pin(request,processor_dependency_id,RIFE_FLAGS,&features,None)?;
            let param=artifact(request,param_dependency_id)?;let weights=artifact(request,weights_dependency_id)?;
            let parent=param.path.parent().ok_or_else(||invalid("RIFE model parent is missing"))?;
            if parent!=weights.path.parent().ok_or_else(||invalid("RIFE weights parent is missing"))?
                ||parent.file_name().and_then(|s|s.to_str())!=Some("rife-v4.6")
                ||param.path.file_name().and_then(|s|s.to_str())!=Some("flownet.param")
                ||weights.path.file_name().and_then(|s|s.to_str())!=Some("flownet.bin")
                ||observation(param,param_dependency_id)?.package_revision!=observation(weights,weights_dependency_id)?.package_revision {
                return Err(invalid("RIFE requires exact attributed model companions in one rife-v4.6 directory"));
            }
            let mut entries=BTreeSet::new();
            for entry in std::fs::read_dir(parent).map_err(|_|invalid("cannot inspect RIFE model directory"))? {
                if entries.len()>=2{return Err(invalid("RIFE model directory has undeclared members"));}
                entries.insert(entry.map_err(|_|invalid("cannot inspect RIFE model member"))?.file_name());
            }
            if entries!=BTreeSet::from([std::ffi::OsString::from("flownet.param"),std::ffi::OsString::from("flownet.bin")]){return Err(invalid("RIFE model directory is incomplete"));}
            let configuration=options.configuration(processor,RifeModelPin {directory:parent.into(),param_sha256:param.expected_sha256.clone(),param_bytes:std::fs::metadata(&param.path).map_err(|_|invalid("RIFE param unavailable"))?.len(),
                bin_sha256:weights.expected_sha256.clone(),bin_bytes:std::fs::metadata(&weights.path).map_err(|_|invalid("RIFE weights unavailable"))?.len()});
            configuration.validate()?;serde_json::to_value(configuration)
        },
        (MediaSetupSelection::AllenkImageRepair { ffmpeg_dependency_id, processor_dependency_id }, Options::Allenk(options)) => {
            let decoder = tool_pin(request, ffmpeg_dependency_id, DECODER_FLAGS, DECODER_FEATURES, None)?;
            let mut flags = ALLENK_FLAGS.to_vec();
            flags.push(match options.watermark_profile { AllenkWatermarkProfile::CurrentV2 => "--no-legacy", AllenkWatermarkProfile::LegacyV1 => "--legacy" });
            let processor = tool_pin(request, processor_dependency_id, &flags, &["visible-image-repair"], Some(ALLENK_INTERFACE_REVISION))?;
            if processor.version != ALLENK_INTERFACE_VERSION {
                return Err(invalid("allenk setup requires the exact 0.3.2 basic-image interface"));
            }
            let configuration = AllenkRepairConfiguration {
                schema: ALLENK_REPAIR_CONFIGURATION_SCHEMA_V1.to_owned(),
                import: options.import.configuration(decoder), tool: processor,
                watermark_profile: options.watermark_profile,
                detection_threshold_percent: options.detection_threshold_percent,
                maximum_working_bytes: options.maximum_working_bytes,
            };
            configuration.validate()?;
            serde_json::to_value(configuration)
        }
        (MediaSetupSelection::UpscaylRecovery { ffmpeg_dependency_id, processor_dependency_id, param_dependency_id, weights_dependency_id }, Options::Upscayl(options)) => {
            let decoder = tool_pin(request, ffmpeg_dependency_id, DECODER_FLAGS, DECODER_FEATURES, None)?;
            let mut flags = UPSCAYL_FLAGS.to_vec();
            if options.tta { flags.push("-x"); }
            let processor = tool_pin(request, processor_dependency_id, &flags, &["model-pair-loading", "vulkan"], Some(UPSCAYL_INTERFACE_REVISION))?;
            let param = artifact(request, param_dependency_id)?;
            let weights = artifact(request, weights_dependency_id)?;
            let param_observation = observation(param, param_dependency_id)?;
            let weights_observation = observation(weights, weights_dependency_id)?;
            if param_observation.native_scale != Some(4) || weights_observation.native_scale != Some(4)
                || param_observation.package_revision != weights_observation.package_revision {
                return Err(invalid("Upscayl model companions require matching supplied revisions and native scale 4"));
            }
            let configuration = UpscaylRecoveryConfiguration {
                schema: UPSCAYL_RECOVERY_CONFIGURATION_SCHEMA_V1.to_owned(),
                import: options.import.configuration(decoder), upscayl: processor,
                model: UpscaylModelPin {
                    name: options.model_name, native_scale: options.native_scale,
                    param: UpscaylModelFile { path: param.path.clone(), sha256: param.expected_sha256.clone() },
                    weights: UpscaylModelFile { path: weights.path.clone(), sha256: weights.expected_sha256.clone() },
                },
                output_scale: options.output_scale, execution: UpscaylExecutionPolicy::PerFrame,
                tile_size: options.tile_size,
                gpu_id: options.gpu_id, threads: options.threads, tta: options.tta,
                maximum_output_frame_bytes: options.maximum_output_frame_bytes,
                maximum_output_pixels: options.maximum_output_pixels,
                maximum_working_bytes: options.maximum_working_bytes,
                minimum_free_bytes: options.minimum_free_bytes,
            };
            configuration.validate()?;
            serde_json::to_value(configuration)
        }
        _ => return Err(invalid("media setup mapping differs from its selected options")),
    };
    value.map_err(|_| invalid("cannot encode prepared media configuration"))
}

/// Inspect one selected profile and prepare an existing facade configuration.
/// This function writes nothing, launches no process and reads no source media.
pub fn prepare_media_setup(request: &MediaSetupRequest) -> Result<MediaSetupPreparation> {
    let request_sha256 = request.sha256()?;
    let inspection = inspect_profile(&request.profile, &request.inventory, &[request.capability_id.clone()])?;
    let mut diagnostics = Vec::new();
    if !inspection.ready {
        diagnostics.push("selected dependency inventory is not ready; resolve its facts and remediation actions".to_owned());
    }
    let platform = &request.inventory.platform;
    if !matches!((platform.os.as_str(), platform.arch.as_str()), ("linux", "x86_64") | ("macos", "aarch64")) {
        diagnostics.push("selected media setup currently admits candidate Linux x86_64 or macOS Apple Silicon hosts only; native qualification remains unestablished".to_owned());
    }
    if platform.os != std::env::consts::OS || platform.arch != std::env::consts::ARCH {
        diagnostics.push("supplied inventory platform differs from the current preparation host".to_owned());
    }
    let configuration = if inspection.ready {
        match prepare_configuration(request) {
            Ok(value) => Some(value),
            Err(error) => { diagnostics.push(error.to_string()); None }
        }
    } else { None };
    let files = match configuration {
        Some(content) if diagnostics.is_empty() => vec![ToolchainRegistrationFile {
            relative_path: "configuration.json".to_owned(),
            sha256: crate::provider::canonical_sha256(&content)?, content,
        }],
        _ => Vec::new(),
    };
    let result = MediaSetupPreparation {
        schema: MEDIA_SETUP_PREPARATION_SCHEMA.to_owned(), request_sha256,
        ready: diagnostics.is_empty(), native_qualification: false,
        host_runtime_evidence: "unobserved".to_owned(), inspection, files, diagnostics,
    };
    result.canonical_json_bytes()?;
    Ok(result)
}
