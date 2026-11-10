//! A silent, locally zero-based viewing copy of an accepted PNG preview.
//! Ordinary v3 owns execution and acceptance; the original mapping stays explicit.
mod runtime;
mod native;
pub use runtime::{plan, run, resume, status, observe};
pub(crate) use native::execute_invocation;

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::destination::local;
use crate::frame_io::identity;
use crate::provider::{canonical_json_bytes, decode_json, require_sha256};
use crate::temporal::{RationalTime, TimeRange};
use crate::video_assembly::AssemblyConfiguration;
use crate::{CancellationToken, Error, ErrorCategory, Result};
use super::{PreviewFrame, PreviewReport};

pub const PROVIDER_ID: &str = "org.egohygiene.aniflow.rife-preview-movie";
pub const CAPABILITY_ID: &str = "aniflow/rife-preview-movie";
const STAGE: &str = "encode_rife_preview_movie";
const REQUEST: &str = "aniflow.rife-preview-movie-request/v1";
const OPERATION: &str = "aniflow.rife-preview-movie-operation/v1";
const REPORT: &str = "aniflow.rife-preview-movie-report/v1";
const MAXIMUM_REQUEST_BYTES: usize = 65_536;
const MAXIMUM_PREVIEW_REPORT_BYTES: u64 = 524_288;
const SCOPE: &str = "selected_preview_only; no_full_run_or_assembly_acceptance; no_perceptual_approval";
const COLOR: &str = "explicit_RGB8_full_range_interpretation_to_BT709_limited_yuv420p; source_color_not_authenticated";
const CLOCK: &str = "movie_local_zero; original_global_clock_preserved_in_mapping_only";
const AUDIO: &str = "silent_preview; original_audio_unchanged_and_unassessed";

fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input, message) }
fn temporal(error: anyhow::Error) -> Error { Error::from_anyhow(ErrorCategory::Media, error) }
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "RIFE preview movie cancelled")); }
    Ok(())
}
fn absolute(path: &Path) -> bool {
    super::super::normalized_absolute(path) && path.as_os_str().len() <= 4096
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieRequest {
    pub schema: String,
    pub preview_run_directory: PathBuf,
    pub expected_preview_report_sha256: String,
    pub settings: AssemblyConfiguration,
    pub provider_executable: PathBuf,
}
impl MovieRequest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAXIMUM_REQUEST_BYTES { return Err(invalid("RIFE preview movie request exceeds 64 KiB")); }
        let request: Self = decode_json(bytes, "RIFE preview movie request")?;
        request.validate()?;
        Ok(request)
    }
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        let s = &self.settings;
        require_sha256(&self.expected_preview_report_sha256, "selected accepted preview report")?;
        if !cfg!(unix) || self.schema != REQUEST || !absolute(&self.preview_run_directory) || !absolute(&self.provider_executable)
            || s.timeout_milliseconds > 120_000 || s.maximum_capture_bytes > 16_777_216
            || s.maximum_source_bytes > 134_217_728 || s.maximum_output_bytes > 268_435_456
            || s.maximum_frame_bytes > 67_108_864 || s.maximum_total_frame_bytes > 134_217_728
            || s.maximum_pixels > 8_294_400 || s.maximum_working_bytes > 4_294_967_296
            || s.minimum_free_bytes > 1_073_741_824 || s.delivery_metadata.is_some() {
            return Err(invalid("preview movie requires normalized locators and explicit short silent RGB8/H.264 resource bounds; metadata delivery policy is unsupported"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieSource {
    pub preview_plan_sha256: String,
    pub preview_run_manifest_sha256: String,
    pub report_path: PathBuf,
    pub report_sha256: String,
    pub report_byte_count: u64,
    pub frames_directory: PathBuf,
    pub frames_sha256: String,
    pub frames_byte_count: u64,
    pub source_index_path: PathBuf,
    pub source_index_sha256: String,
    pub source_index_byte_count: u64,
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    pub frame_rate: RationalTime,
    pub first_original_output_ordinal: u64,
    pub original_window: TimeRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieOperation {
    pub schema: String,
    pub request: MovieRequest,
    pub source: MovieSource,
    pub renderer_sha256: String,
    pub renderer_byte_count: u64,
    pub tool_byte_counts: [u64; 2],
    pub working_bytes: u64,
}
impl MovieOperation {
    fn validate(&self) -> Result<()> {
        self.request.validate()?;
        let a = &self.source;
        let s = &self.request.settings;
        for sha in [&a.preview_plan_sha256, &a.preview_run_manifest_sha256, &a.report_sha256,
            &a.frames_sha256, &a.source_index_sha256, &self.renderer_sha256] {
            require_sha256(sha, "RIFE preview movie binding")?;
        }
        if self.schema != OPERATION || a.report_sha256 != self.request.expected_preview_report_sha256
            || ![&a.report_path, &a.frames_directory, &a.source_index_path].iter().all(|p| absolute(p))
            || !(1..=MAXIMUM_PREVIEW_REPORT_BYTES).contains(&a.report_byte_count)
            || !(1..=super::super::RIFE_MAXIMUM_INDEX_BYTES).contains(&a.source_index_byte_count)
            || a.frames_byte_count == 0 || a.frames_byte_count > s.maximum_source_bytes
            || a.frames_byte_count > s.maximum_total_frame_bytes || !(2..=240).contains(&a.frame_count)
            || a.frame_count % 2 != 0 || a.first_original_output_ordinal % 2 != 0
            || a.first_original_output_ordinal.checked_add(a.frame_count).is_none()
            || a.width == 0 || a.height == 0 || a.width % 2 != 0 || a.height % 2 != 0
            || u64::from(a.width) * u64::from(a.height) > s.maximum_pixels
            || a.frame_rate.numerator() <= 0 || a.frame_rate.numerator() > i32::MAX as i64
            || a.frame_rate.denominator() > i32::MAX as u64
            || self.renderer_byte_count == 0 || self.renderer_byte_count > crate::video_assembly::MAXIMUM_TOOL_BYTES
            || self.tool_byte_counts.iter().any(|n| *n == 0 || *n > crate::video_assembly::MAXIMUM_TOOL_BYTES) {
            return Err(invalid("preview movie source geometry, exact clock, cardinality, byte pins or finite admission differs"));
        }
        let period = a.frame_rate.reciprocal().map_err(temporal)?;
        let duration = period.checked_mul(a.frame_count).map_err(temporal)?;
        if duration > RationalTime::new(10, 1).map_err(temporal)?
            || a.original_window.end != a.original_window.start.checked_add(duration).map_err(temporal)?
            || self.working_bytes != self.required_working_bytes()? || self.working_bytes > s.maximum_working_bytes {
            return Err(invalid("preview movie duration or complete working admission differs"));
        }
        Ok(())
    }
    fn required_working_bytes(&self) -> Result<u64> {
        let s = &self.request.settings;
        let tools = self.tool_byte_counts[0].checked_add(self.tool_byte_counts[1])
            .and_then(|n| n.checked_add(self.renderer_byte_count)).ok_or_else(|| invalid("preview movie tool byte overflow"))?;
        [(self.source.frames_byte_count, 3), (self.source.source_index_byte_count, 3),
            (self.source.report_byte_count, 3), (s.maximum_output_bytes, 4), (tools, 2),
            (s.maximum_capture_bytes as u64, 12), (local::DOCUMENT_BYTES, 8), (33_554_432, 1)]
            .into_iter().try_fold(0u64, |sum, (bytes, copies)| bytes.checked_mul(copies)
                .and_then(|n| sum.checked_add(n)).ok_or_else(|| invalid("preview movie working byte overflow")))
    }
    fn schema_reference() -> crate::ConfigurationSchemaReference {
        crate::ConfigurationSchemaReference { id: OPERATION.into(), version: "1.0.0".into(),
            sha256: local::hash(include_bytes!("../../../docs/contracts/rife-preview-movie-operation-v1.schema.json")) }
    }
    fn configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let v = serde_json::to_value(self).map_err(|_| invalid("cannot serialize preview movie operation"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference { id: PROVIDER_ID.into(), version: "1.0.0".into() },
            crate::CapabilityReference { id: CAPABILITY_ID.into(), version: "1.0.0".into() }, Self::schema_reference(),
            v.as_object().ok_or_else(|| invalid("preview movie operation must be an object"))?.iter().map(|(k,v)| (k.clone(),v.clone())).collect())
    }
    fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let port = |name: &str, kind: &str, role: &str, stream: &str| serde_json::json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let v = serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow accepted RIFE preview movie"},"configuration_schemas":[Self::schema_reference()],"capabilities":[{
            "id":CAPABILITY_ID,"version":"1.0.0","kind":"assembler_encoder","configuration_schema":Self::schema_reference(),
            "inputs":[port("frames","application/vnd.aniflow.rife-preview-frames","intermediate","video"),port("preview","application/vnd.aniflow.rife-preview-report+json","validation_evidence","timed_metadata"),port("index","application/vnd.aniflow.rife-frame-index+json","validation_evidence","timed_metadata")],
            "outputs":[port("media","video/mp4","intermediate","video"),port("evidence","application/vnd.aniflow.rife-preview-movie-report+json","validation_evidence","timed_metadata")],
            "batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":crate::video_assembly::component_inventory(&self.request.settings).tools.iter().map(|t|serde_json::json!({"id":t.id,"version_requirement":format!("={}",t.version),"sha256":t.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&v)?)
    }
}

/// Each local picture refers to one unchanged global preview mapping entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieFrame {
    pub local_ordinal: u64,
    pub local_presentation: TimeRange,
    pub original: PreviewFrame,
}
fn mapping(report: &PreviewReport) -> Result<Vec<MovieFrame>> {
    let period = report.full_source_plan.output_frame_rate.reciprocal().map_err(temporal)?;
    report.frames.iter().enumerate().map(|(i, frame)| {
        Ok(MovieFrame { local_ordinal: i as u64,
            local_presentation: TimeRange::new(period.checked_mul(i as u64).map_err(temporal)?,
                period.checked_mul(i as u64 + 1).map_err(temporal)?).map_err(temporal)?, original: frame.clone() })
    }).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieMedia {
    pub sha256: String,
    pub byte_count: u64,
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    pub frame_rate: RationalTime,
    pub local_interval: TimeRange,
    pub video_codec: String,
    pub pixel_format: String,
    pub color_primaries: String,
    pub color_transfer: String,
    pub color_matrix: String,
    pub color_range: String,
    pub sample_aspect_ratio: String,
    pub faststart: bool,
    pub full_decode_observed: bool,
    pub audio_stream_count: u32,
}
impl MovieMedia {
    fn validate(&self, operation: &MovieOperation) -> Result<()> {
        let s = &operation.source;
        require_sha256(&self.sha256, "preview movie bytes")?;
        let zero = RationalTime::new(0, 1).map_err(temporal)?;
        let end = s.frame_rate.reciprocal().and_then(|p| p.checked_mul(s.frame_count)).map_err(temporal)?;
        if self.byte_count == 0 || self.byte_count > operation.request.settings.maximum_output_bytes
            || self.width != s.width || self.height != s.height || self.frame_count != s.frame_count || self.frame_rate != s.frame_rate
            || self.local_interval != TimeRange::new(zero, end).map_err(temporal)? || self.video_codec != "h264"
            || self.pixel_format != "yuv420p" || self.color_primaries != "bt709" || self.color_transfer != "bt709"
            || self.color_matrix != "bt709" || self.color_range != "tv" || self.sample_aspect_ratio != "1:1"
            || !self.faststart || !self.full_decode_observed || self.audio_stream_count != 0 {
            return Err(invalid("preview movie media differs from exact local frame clock and closed silent H.264 viewing profile"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieReport {
    pub schema: String,
    pub operation: MovieOperation,
    pub provider_lock_sha256: String,
    pub frames: Vec<MovieFrame>,
    pub media: MovieMedia,
    pub clock_policy: String,
    pub color_policy: String,
    pub audio_scope: String,
    pub review_scope: String,
    pub native_qualified: bool,
    pub creative_approval: String,
    pub publication_authority: String,
}
impl MovieReport {
    fn validate(&self, preview: &PreviewReport) -> Result<()> {
        self.operation.validate()?;
        self.media.validate(&self.operation)?;
        require_sha256(&self.provider_lock_sha256, "preview movie native provider lock")?;
        let source = &self.operation.source;
        if self.schema != REPORT || self.frames != mapping(preview)? || source.frame_count != preview.frames.len() as u64
            || source.frame_rate != preview.full_source_plan.output_frame_rate || source.original_window != preview.window
            || preview.frames.first().map(|f| f.frame.ordinal) != Some(source.first_original_output_ordinal)
            || self.clock_policy != CLOCK || self.color_policy != COLOR
            || self.audio_scope != AUDIO || self.review_scope != SCOPE || self.native_qualified
            || self.creative_approval != "not_performed" || self.publication_authority != "none" {
            return Err(invalid("preview movie report changed its source mapping or declared technical/creative scope"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovieCompletion {
    pub schema: String,
    pub run_directory: PathBuf,
    pub plan_sha256: String,
    pub run_manifest_sha256: String,
    pub report_sha256: String,
    pub movie_path: PathBuf,
    pub report: MovieReport,
}

#[cfg(all(test, unix))]
mod tests;
