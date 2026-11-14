//! Explicit short, silent before/after comparison; native v3 owns acceptance.
use crate::destination::{cancelled, invalid, local};
use crate::final_video::{FinalVideoArtifact, FinalVideoHandoff};
use crate::frame_io::{ensure_canonical_directory, identity};
use crate::music_video_recipe::manual::ManualReview;
use crate::provider::{canonical_json_bytes, canonical_sha256};
use crate::video_assembly::AssemblyConfiguration;
use crate::{CancellationToken, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
mod native;
mod runtime;
pub(crate) use native::execute_invocation;
pub use runtime::{plan, record_review, resume, run, status};
#[cfg(test)]
mod tests;
const PROVIDER: &str = "org.egohygiene.aniflow.deflicker-comparison";
const CAPABILITY: &str = "aniflow/deflicker-comparison";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Mean,
    Median,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub tools: AssemblyConfiguration,
    pub window: u8,
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_polish: Option<crate::spatial_polish::Profile>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub video_run_directory: PathBuf,
    pub artifact_id: String,
    pub first_frame: u64,
    pub frame_count: u64,
    pub configuration: Option<Configuration>,
    pub provider_executable: Option<PathBuf>,
}
impl Request {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 {
            return Err(invalid("deflicker request exceeds 64 KiB"));
        }
        let r: Self = crate::provider::decode_json(bytes, "deflicker comparison request")?;
        r.validate()?;
        Ok(r)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != "aniflow.deflicker-comparison-request/v1"
            || !crate::destination::identifier(&self.artifact_id)
            || !(1..=240).contains(&self.frame_count)
        {
            return Err(invalid(
                "deflicker comparison requires an explicit artifact and 1..240 original frames",
            ));
        }
        if let Some(c) = &self.configuration {
            c.tools.validate()?;
            if c.tools.delivery_metadata.is_some()
                || !(3..=31).contains(&c.window)
                || c.window % 2 != 1
                || self.frame_count < u64::from(c.window)
                || self.provider_executable.is_none()
            {
                return Err(invalid(
                    "selected deflicker needs exact tools/provider, odd 3..31 window fitting the excerpt and no metadata policy",
                ));
            }
        }
        if let Some(profile) = self
            .configuration
            .as_ref()
            .and_then(|c| c.spatial_polish.as_ref())
        {
            profile.validate()?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub schema: String,
    pub request: Request,
    pub source: FinalVideoHandoff,
    pub artifact: FinalVideoArtifact,
    pub renderer_sha256: Option<String>,
    pub working_bytes: u64,
}
impl Operation {
    fn validate(&self) -> Result<()> {
        self.request.validate()?;
        if self.schema != "aniflow.deflicker-comparison-operation/v1"
            || self
                .source
                .artifacts
                .iter()
                .find(|a| a.artifact_id == self.request.artifact_id)
                != Some(&self.artifact)
        {
            return Err(invalid(
                "deflicker operation does not bind its selected accepted source",
            ));
        }
        if self
            .request
            .first_frame
            .checked_add(self.request.frame_count)
            .is_none_or(|n| n > self.artifact.frame_count)
            || self.artifact.frame_rate.numerator() <= 0
        {
            return Err(invalid(
                "deflicker excerpt lies outside exact original frame clock",
            ));
        }
        if self.request.configuration.is_some() {
            let a = &self.artifact;
            let duration = a
                .frame_rate
                .reciprocal()
                .and_then(|p| p.checked_mul(self.request.frame_count))
                .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))?;
            if duration
                > crate::temporal::RationalTime::new(10, 1)
                    .map_err(|e| crate::Error::from_anyhow(crate::ErrorCategory::Media, e))?
            {
                return Err(invalid("deflicker excerpt exceeds ten seconds"));
            }
            if a.video_codec != "h264"
                || a.pixel_format != "yuv420p"
                || a.width == 0
                || a.height == 0
                || a.width.checked_mul(a.height).is_none_or(|n| n > 8_294_400)
                || a.color_primaries != "bt709"
                || a.color_transfer != "bt709"
                || a.color_matrix != "bt709"
                || a.color_range != "tv"
                || !matches!(a.sample_aspect_ratio.as_str(), "1:1" | "1/1")
                || a.orientation_degrees != 0
            {
                return Err(invalid(
                    "initial comparison supports accepted unrotated 1:1 H.264 yuv420p BT.709 SDR at up to 4K pixels",
                ));
            }
            crate::provider::require_sha256(
                self.renderer_sha256
                    .as_deref()
                    .ok_or_else(|| invalid("selected comparison renderer missing"))?,
                "deflicker renderer",
            )?;
        } else if self.renderer_sha256.is_some() || self.working_bytes != 0 {
            return Err(invalid(
                "bypassed comparison cannot bind tools or native working data",
            ));
        }
        if admission(&self.request, &self.artifact)?
            != (self.renderer_sha256.clone(), self.working_bytes)
        {
            return Err(invalid(
                "comparison exact component/working admission differs",
            ));
        }
        Ok(())
    }
    fn context(&self) -> Result<String> {
        canonical_sha256(self)
    }
    fn schema_ref() -> crate::ConfigurationSchemaReference {
        crate::ConfigurationSchemaReference {
            id: "aniflow.deflicker-comparison-operation/v1".into(),
            version: "1.0.0".into(),
            sha256: local::hash(include_bytes!(
                "../../docs/contracts/deflicker-comparison-operation-v1.schema.json"
            )),
        }
    }
    fn configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let v =
            serde_json::to_value(self).map_err(|_| invalid("deflicker operation serialization"))?;
        crate::ProviderConfiguration::new(
            crate::ProviderReference {
                id: PROVIDER.into(),
                version: "1.0.0".into(),
            },
            crate::CapabilityReference {
                id: CAPABILITY.into(),
                version: "1.0.0".into(),
            },
            Self::schema_ref(),
            v.as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
    fn manifest(&self) -> Result<crate::ProviderManifest> {
        let c = self
            .request
            .configuration
            .as_ref()
            .ok_or_else(|| invalid("bypass has no native manifest"))?;
        let port = |name: &str, kind: &str, role: &str, stream: &str| serde_json::json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let value = serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER,"version":"1.0.0","display_name":"aniflow explicit deflicker comparison"},"configuration_schemas":[Self::schema_ref()],"capabilities":[{"id":CAPABILITY,"version":"1.0.0","kind":"frame_processor","configuration_schema":Self::schema_ref(),"inputs":[port("source","video/mp4","temporal_source","video")],"outputs":[port("comparison","application/vnd.aniflow.deflicker-comparison","intermediate","video"),port("evidence","application/vnd.aniflow.deflicker-report+json","validation_evidence","timed_metadata")],"batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":crate::video_assembly::component_inventory(&c.tools).tools.iter().map(|t|serde_json::json!({"id":t.id,"version_requirement":format!("={}",t.version),"sha256":t.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},"behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],"provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub original_ordinal: u64,
    pub source_time: crate::temporal::RationalTime,
    pub before_sha256: String,
    pub before_bytes: u64,
    pub after_sha256: String,
    pub after_bytes: u64,
    pub before_rgb_sha256: String,
    pub after_rgb_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub operation: Operation,
    pub provider_lock_sha256: String,
    pub frames: Vec<Frame>,
    pub comparison_sha256: String,
    pub visual_approval: String,
    pub runtime_qualified: bool,
    pub audio_scope: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub operation: Operation,
    pub context_sha256: String,
    pub native_plan: Option<crate::PipelineV3Plan>,
    pub bypassed: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub schema: String,
    pub run_directory: PathBuf,
    pub context_sha256: String,
    pub bypassed: bool,
    pub complete: bool,
    pub native_manifest_sha256: Option<String>,
    pub comparison_directory: Option<PathBuf>,
    pub report: Option<Report>,
    pub review: Option<ManualReview>,
    pub publication_authority: String,
}

fn admission(r: &Request, a: &FinalVideoArtifact) -> Result<(Option<String>, u64)> {
    let Some(c) = &r.configuration else {
        return Ok((None, 0));
    };
    let tools = crate::video_assembly::observe_components(&c.tools)?;
    let pin = identity(
        r.provider_executable
            .as_ref()
            .ok_or_else(|| invalid("comparison provider missing"))?,
        crate::video_assembly::MAXIMUM_TOOL_BYTES,
    )?;
    let pixels = u64::from(a.width)
        .checked_mul(u64::from(a.height))
        .ok_or_else(|| invalid("comparison pixel overflow"))?;
    let frame = pixels
        .checked_mul(4)
        .and_then(|v| v.checked_add(4096))
        .ok_or_else(|| invalid("comparison frame overflow"))?;
    let all = frame
        .checked_mul(r.frame_count)
        .and_then(|v| v.checked_mul(2))
        .ok_or_else(|| invalid("comparison total output overflow"))?;
    if pixels > c.tools.maximum_pixels
        || frame > c.tools.maximum_frame_bytes
        || all > c.tools.maximum_total_frame_bytes
        || a.byte_count > c.tools.maximum_source_bytes
    {
        return Err(invalid(
            "selected full comparison exceeds frame/pixel/source admission",
        ));
    }
    let work = a
        .byte_count
        .checked_mul(2)
        .and_then(|v| v.checked_add(all.checked_mul(3)?))
        .and_then(|v| v.checked_add((tools[0].1 + tools[1].1 + pin.1).checked_mul(3)?))
        .and_then(|v| v.checked_add(16_777_216))
        .ok_or_else(|| invalid("comparison working overflow"))?;
    if work > c.tools.maximum_working_bytes {
        return Err(invalid(
            "comparison complete working admission exceeds selected bound",
        ));
    }
    Ok((Some(pin.0), work))
}
