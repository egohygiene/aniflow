//! Closed target-resolution policy over exact current frame identities.
//! Decisions are pure. Structural PNG observation and native acceptance remain
//! separate: this module does not qualify a model or assert decoded fidelity.
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::provider::{canonical_sha256, decode_json, require_sha256};
use crate::upscayl_recovery::UpscaylRecoveryConfiguration;
use crate::{CancellationToken, Error, ErrorCategory, Result};

pub const UPSCALE_TARGET_POLICY_SCHEMA_V1: &str = "aniflow.upscale-target-policy/v1";
pub const UPSCALE_TARGET_DECISION_SCHEMA_V1: &str = "aniflow.upscale-target-decision/v1";
pub const UPSCALE_TARGET_FRAME_PROFILE: &str = "rgb8_sdr_bt709_square_unrotated";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpscaleTargetMode { Exact, Minimum }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaleTargetPolicy {
    pub schema: String,
    pub width: u32,
    pub height: u32,
    pub mode: UpscaleTargetMode,
    #[serde(default)]
    pub force_reprocess: bool,
}

/// Exact current input identities and index-declared frame profile. Dimensions
/// supplied by observe_input have also been checked against every PNG IHDR.
/// The profile is not a native probe result or a claim about prior processing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaleTargetInput {
    pub source_sha256: String,
    pub source_byte_count: u64,
    pub frame_index_sha256: String,
    /// Existing Pipeline v3 directory content identity over the observed
    /// frame set (only the selected subset for a bounded preview).
    pub frames_sha256: String,
    pub frames_byte_count: u64,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    pub frame_profile: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpscaleTargetStatus { Disabled, AlreadyTarget, AboveTarget, Execute }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaleTargetDecision {
    pub schema: String,
    pub input: UpscaleTargetInput,
    pub policy: Option<UpscaleTargetPolicy>,
    pub input_sha256: String,
    pub policy_sha256: Option<String>,
    pub status: UpscaleTargetStatus,
    pub reason: String,
    pub effective_width: u32,
    pub effective_height: u32,
    pub output_scale: Option<u8>,
    /// Present only on bypass: the exact input remains the output authority.
    pub unchanged_input_sha256: Option<String>,
}

/// Geometric arithmetic and configured ceilings, not measured memory, disk
/// usage or a native allocation limit. Full runtime storage admission remains
/// owned by the existing Upscayl plan and provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpscaleTargetResources {
    pub frame_count: u64,
    pub input_pixels_per_frame: u64,
    pub effective_pixels_per_frame: u64,
    pub effective_rgb24_bytes_per_frame: u64,
    pub effective_rgb24_bytes_total: u64,
    pub native_intermediate_pixels_per_frame: Option<u64>,
    pub maximum_output_frame_bytes: Option<u64>,
    pub maximum_output_png_bytes: Option<u64>,
    pub maximum_working_bytes: Option<u64>,
    pub minimum_free_bytes: Option<u64>,
    pub memory_limit_enforced: bool,
}

fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input,message) }
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() { return Err(Error::new(ErrorCategory::Execution,"target-resolution observation cancelled")); }
    Ok(())
}

impl UpscaleTargetPolicy {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("target-resolution policy exceeds 64 KiB")); }
        let value: Self = decode_json(bytes,"target-resolution policy")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != UPSCALE_TARGET_POLICY_SCHEMA_V1 || self.width == 0 || self.height == 0 {
            return Err(invalid("target-resolution policy requires its supported schema and two positive oriented dimensions"));
        }
        Ok(())
    }
}

impl UpscaleTargetInput {
    pub fn validate(&self) -> Result<()> {
        for (value,label) in [(&self.source_sha256,"target source digest"),(&self.frame_index_sha256,"target frame index digest"),(&self.frames_sha256,"target frames digest")] {
            require_sha256(value,label)?;
        }
        if self.source_byte_count == 0 || self.frames_byte_count == 0 || !(1..=100_000).contains(&self.frame_count)
            || self.width == 0 || self.height == 0 || self.frame_profile != UPSCALE_TARGET_FRAME_PROFILE {
            return Err(invalid("target-resolution input requires 1..100000 frames with positive geometry and the declared RGB8 SDR BT709 square unrotated profile"));
        }
        Ok(())
    }
}

/// Select an existing integer output factor. No model, file or tool is needed.
/// Disabled and at/above-target cases retain the exact current input identity.
/// Exact specifies the result when processing is needed; an existing larger
/// same-aspect input is retained unless force_reprocess explicitly forbids skip.
pub fn decide(input: &UpscaleTargetInput, policy: Option<&UpscaleTargetPolicy>) -> Result<UpscaleTargetDecision> {
    input.validate()?;
    let input_sha256 = canonical_sha256(input)?;
    let mut decision = UpscaleTargetDecision {
        schema:UPSCALE_TARGET_DECISION_SCHEMA_V1.to_owned(),input:input.clone(),policy:policy.cloned(),input_sha256:input_sha256.clone(),
        policy_sha256:policy.map(canonical_sha256).transpose()?,status:UpscaleTargetStatus::Disabled,
        reason:"target-resolution processing is disabled".to_owned(),effective_width:input.width,effective_height:input.height,
        output_scale:None,unchanged_input_sha256:Some(input_sha256),
    };
    let Some(policy) = policy else { return Ok(decision); };
    policy.validate()?;
    if u64::from(input.width) * u64::from(policy.height) != u64::from(input.height) * u64::from(policy.width) {
        return Err(invalid("target aspect ratio differs from both oriented input dimensions; implicit crop, stretch or resize is unsupported"));
    }
    if !policy.force_reprocess && input.width >= policy.width && input.height >= policy.height {
        if input.width == policy.width && input.height == policy.height {
            decision.status = UpscaleTargetStatus::AlreadyTarget;
            decision.reason = "current frames already match both target dimensions; preserving the exact input".to_owned();
        } else {
            decision.status = UpscaleTargetStatus::AboveTarget;
            decision.reason = "current frames exceed both target dimensions; preserving the exact input".to_owned();
        }
        return Ok(decision);
    }
    for scale in 2_u8..=4 {
        let Some(width) = input.width.checked_mul(u32::from(scale)) else { continue; };
        let Some(height) = input.height.checked_mul(u32::from(scale)) else { continue; };
        let meets_target = match policy.mode {
            UpscaleTargetMode::Exact => width == policy.width && height == policy.height,
            UpscaleTargetMode::Minimum => width >= policy.width && height >= policy.height,
        };
        if meets_target {
            decision.status = UpscaleTargetStatus::Execute;
            decision.reason = match policy.mode {
                UpscaleTargetMode::Exact => format!("integer {scale}x output reaches both exact target dimensions"),
                UpscaleTargetMode::Minimum => format!("integer {scale}x output is the smallest supported factor meeting both minimum target dimensions"),
            };
            decision.effective_width = width;
            decision.effective_height = height;
            decision.output_scale = Some(scale);
            decision.unchanged_input_sha256 = None;
            return Ok(decision);
        }
    }
    Err(invalid("target cannot be reached with an integer output scale of 2, 3 or 4; 1x reprocessing, downscaling and implicit resize are unsupported"))
}

impl UpscaleTargetDecision {
    pub fn validate(&self) -> Result<()> {
        if decide(&self.input,self.policy.as_ref())? != *self { return Err(invalid("target-resolution decision differs from its exact input and policy")); }
        Ok(())
    }

    /// Baseline output_scale is a template value. Only that field is replaced;
    /// the native model scale, model pins and every other setting are retained.
    /// A bypass does not validate or inspect the neural configuration.
    pub fn resolved_configuration(&self, settings: &UpscaylRecoveryConfiguration) -> Result<Option<UpscaylRecoveryConfiguration>> {
        self.validate()?;
        let Some(scale) = self.output_scale else { return Ok(None); };
        let mut resolved = settings.clone();
        resolved.output_scale = scale;
        resolved.validate()?;
        if resolved.output_dimensions(self.input.width,self.input.height)? != (self.effective_width,self.effective_height) {
            return Err(invalid("resolved target geometry differs from the existing Upscayl output profile"));
        }
        Ok(Some(resolved))
    }

    pub fn resources(&self, settings: &UpscaylRecoveryConfiguration) -> Result<UpscaleTargetResources> {
        let resolved = self.resolved_configuration(settings)?;
        let pixels = u64::from(self.effective_width) * u64::from(self.effective_height);
        let rgb = pixels.checked_mul(3).ok_or_else(|| invalid("target RGB byte arithmetic overflow"))?;
        let total = rgb.checked_mul(self.input.frame_count).ok_or_else(|| invalid("target total RGB byte arithmetic overflow"))?;
        let mut resources = UpscaleTargetResources {
            frame_count:self.input.frame_count,input_pixels_per_frame:u64::from(self.input.width)*u64::from(self.input.height),
            effective_pixels_per_frame:pixels,effective_rgb24_bytes_per_frame:rgb,effective_rgb24_bytes_total:total,
            native_intermediate_pixels_per_frame:None,maximum_output_frame_bytes:None,maximum_output_png_bytes:None,
            maximum_working_bytes:None,minimum_free_bytes:None,memory_limit_enforced:false,
        };
        if let Some(settings) = resolved {
            let native = u64::from(settings.model.native_scale);
            resources.native_intermediate_pixels_per_frame = Some(resources.input_pixels_per_frame.checked_mul(native).and_then(|value| value.checked_mul(native))
                .ok_or_else(|| invalid("target native intermediate pixel arithmetic overflow"))?);
            resources.maximum_output_frame_bytes = Some(settings.maximum_output_frame_bytes);
            resources.maximum_output_png_bytes = Some(self.input.frame_count.checked_mul(settings.maximum_output_frame_bytes)
                .ok_or_else(|| invalid("target output PNG ceiling arithmetic overflow"))?);
            resources.maximum_working_bytes = Some(settings.maximum_working_bytes);
            resources.minimum_free_bytes = Some(settings.minimum_free_bytes);
        }
        Ok(resources)
    }
}

/// Read-only bounded structural observation of every current indexed frame.
/// Accepts original or existing Upscayl unchanged-clock indices. Neither file
/// names nor historical processing claims influence the geometry decision.
/// Native source profile and decoded fidelity are still separately admitted.
pub fn observe_input(frames_directory: &Path, frame_index: &Path, maximum_frame_bytes: u64,
    maximum_total_frame_bytes: u64, token: &CancellationToken) -> Result<UpscaleTargetInput> {
    observe_frames(frames_directory,frame_index,None,maximum_frame_bytes,maximum_total_frame_bytes,token)
}

/// Observe only a one-based inclusive selection of at most 256 frames. The
/// complete index is validated, but unselected PNGs are not read or attested.
/// The returned frames identity covers exactly the selected files; a preview
/// must retain its explicit range separately and cannot adopt this as a full
/// imported or upscaled set. Source native admission remains separate.
pub fn observe_selected_input(frames_directory: &Path, frame_index: &Path, range: &crate::frame_selection::InclusiveFrameRange,
    maximum_frame_bytes: u64, maximum_total_frame_bytes: u64, token: &CancellationToken) -> Result<UpscaleTargetInput> {
    observe_frames(frames_directory,frame_index,Some(*range),maximum_frame_bytes,maximum_total_frame_bytes,token)
}

fn observe_frames(frames_directory: &Path, frame_index: &Path, range: Option<crate::frame_selection::InclusiveFrameRange>,
    maximum_frame_bytes: u64, maximum_total_frame_bytes: u64, token: &CancellationToken) -> Result<UpscaleTargetInput> {
    cancelled(token)?;
    if !(1..=268_435_456).contains(&maximum_frame_bytes) || !(1..=1_099_511_627_776).contains(&maximum_total_frame_bytes) {
        return Err(invalid("target frame observation requires a frame bound of 1..256 MiB and a total bound of 1 byte..1 TiB"));
    }
    let (index,index_identity) = crate::video_assembly::read_index(frame_index)?;
    let (width,height) = index.dimensions();
    crate::frame_io::ensure_canonical_directory(frames_directory)?;
    let frames = if let Some(range) = range {
        if range.first == 0 || range.last < range.first || range.last > index.frames().len() as u64 || range.last - range.first >= 256 {
            return Err(invalid("target preview requires a valid one-based inclusive selection of at most 256 indexed frames"));
        }
        &index.frames()[(range.first - 1) as usize..range.last as usize]
    } else { index.frames() };
    let names: Vec<std::ffi::OsString> = frames.iter().map(|frame| frame.filename.clone().into()).collect();
    if range.is_none() { crate::frame_io::inventory(frames_directory,&names)?; }
    let declaration = crate::AuthoredPipelineInput { id:"source_frames".to_owned(),artifact_type:"application/vnd.aniflow.assembly-frames".to_owned(),
        artifact_role:crate::ArtifactRole::Intermediate,stream_role:Some(crate::StreamRole::Video) };
    let mut bytes = 0_u64;
    let mut entries = Vec::with_capacity(frames.len());
    for frame in frames {
        cancelled(token)?;
        let observed = crate::frame_io::inspect_png(&frames_directory.join(&frame.filename),width,height,maximum_frame_bytes)?;
        if observed.0 != frame.sha256 { return Err(invalid("current PNG bytes differ from their indexed identity during target observation")); }
        bytes = bytes.checked_add(observed.1).filter(|value| *value <= maximum_total_frame_bytes)
            .ok_or_else(|| invalid("target frame observation exceeds its total byte bound"))?;
        entries.push(crate::pipeline_v3::DirectoryContentEntry {
            relative_path:frame.filename.clone(),kind:crate::PipelineInputKind::File,byte_count:observed.1,content_sha256:Some(observed.0),
        });
    }
    entries.sort_by(|left,right| left.relative_path.cmp(&right.relative_path));
    let expected_frames_sha256 = canonical_sha256(&entries)?;
    cancelled(token)?;
    if range.is_none() {
        let directory = crate::pipeline_v3::hash_pipeline_input(&declaration,frames_directory).map_err(|failure| Error::new(failure.category(),failure.message))?;
        if directory.file_count != frames.len() as u64 || directory.byte_count != bytes || directory.content_sha256 != expected_frames_sha256 {
            return Err(invalid("current frame directory changed during target observation"));
        }
    }
    // Match every digest again after directory hashing so an unrelated changed
    // frame cannot establish a new content identity through the directory hash.
    for frame in frames {
        cancelled(token)?;
        let observed = crate::frame_io::identity(&frames_directory.join(&frame.filename),maximum_frame_bytes)?;
        let position = entries.binary_search_by(|entry| entry.relative_path.as_str().cmp(frame.filename.as_str()))
            .map_err(|_| invalid("target observation lost an indexed frame identity"))?;
        if entries[position].content_sha256.as_ref() != Some(&observed.0) || entries[position].byte_count != observed.1 {
            return Err(invalid("current frame identity changed during target observation"));
        }
    }
    if range.is_none() { crate::frame_io::inventory(frames_directory,&names)?; }
    if crate::frame_io::identity(frame_index,crate::video_assembly::ASSEMBLY_MAXIMUM_INDEX_BYTES)? != index_identity {
        return Err(invalid("current frame index changed during target observation"));
    }
    let frame_profile = match &index {
        crate::video_assembly::AssemblyFrameIndex::Original(value) => value.frame_profile.clone(),
        crate::video_assembly::AssemblyFrameIndex::Upscaled(value) => value.output_profile.clone(),
    };
    let input = UpscaleTargetInput {
        source_sha256:index.temporal().source_sha256.clone(),source_byte_count:index.temporal().source_size_bytes,
        frame_index_sha256:index_identity.0,frames_sha256:expected_frames_sha256,frames_byte_count:bytes,
        frame_count:frames.len() as u64,width,height,frame_profile,
    };
    input.validate()?;
    Ok(input)
}
