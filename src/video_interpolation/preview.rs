//! Bounded native preview over the original index, never a full-run carrier.
pub mod movie;
pub(super) mod runtime;
use super::*;
use crate::temporal::{RationalTime, TimeRange};
pub use runtime::{observe, plan, resume, run, status};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub const PROVIDER_ID: &str = "org.egohygiene.aniflow.rife-preview";
pub const CAPABILITY_ID: &str = "aniflow/rife-preview";
const CONFIGURATION: &str = "aniflow.rife-preview-configuration/v1";
const SCOPE: &str = "selected_original_clock_window_only; no_full_run_or_assembly_acceptance; no_perceptual_approval";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRange {
    pub first_source_ordinal: u64,
    pub end_source_ordinal: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewConfiguration {
    pub schema: String,
    pub configuration: RifeConfiguration,
    pub range: PreviewRange,
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
}
#[derive(Debug, Clone)]
pub struct PreviewRequest {
    pub run: super::runtime::RifeRunRequest,
    pub range: PreviewRange,
}
impl PreviewRequest {
    #[must_use]
    pub fn new(mut run: super::runtime::RifeRunRequest, range: PreviewRange) -> Self {
        run.execution_limits.timeout = std::time::Duration::from_secs(3600);
        run.execution_limits.maximum_artifact_files = 256;
        Self { run, range }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewFrame {
    pub frame: RifeOutputFrame,
    pub presentation: TimeRange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewReport {
    pub schema: String,
    pub operation: PreviewConfiguration,
    pub provider_lock_sha256: String,
    pub source_index_sha256: String,
    pub source_index_byte_count: u64,
    pub source_frames_sha256: String,
    pub full_source_plan: RifePlan,
    pub window: TimeRange,
    pub frames: Vec<PreviewFrame>,
    pub review_scope: String,
    pub native_qualified: bool,
    pub creative_approval: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewCompletion {
    pub schema: String,
    pub run_directory: PathBuf,
    pub plan_sha256: String,
    pub run_manifest_sha256: String,
    pub report_sha256: String,
    pub frames_directory: PathBuf,
    pub report: PreviewReport,
}
impl PreviewConfiguration {
    pub(super) fn validate(&self) -> Result<()> {
        self.configuration.validate()?;
        if self.schema != CONFIGURATION
            || !self.configuration.enabled
            || !normalized_absolute(&self.frames_directory)
            || !normalized_absolute(&self.frame_index)
            || self.range.first_source_ordinal >= self.range.end_source_ordinal
            || self.range.end_source_ordinal - self.range.first_source_ordinal > 120
        {
            return Err(invalid(
                "RIFE preview requires explicit original locators and a nonempty window of at most 120 source frames",
            ));
        }
        Ok(())
    }
    pub(super) fn mapping(
        &self,
        index: &RifeFrameIndex,
    ) -> Result<(RifePlan, TimeRange, Vec<PreviewFrame>)> {
        self.validate()?;
        let plan = super::plan(&self.configuration, index)?;
        super::validate_mapping_plan(&plan)?;
        if self.range.end_source_ordinal > plan.source_frame_count {
            return Err(invalid("RIFE preview range is outside the original source"));
        }
        let period = plan
            .source_frame_rate
            .reciprocal()
            .map_err(temporal_error)?;
        let count = self.range.end_source_ordinal - self.range.first_source_ordinal;
        if period.checked_mul(count).map_err(temporal_error)?
            > RationalTime::new(10, 1).map_err(temporal_error)?
        {
            return Err(invalid(
                "RIFE preview window exceeds ten seconds on the original exact clock",
            ));
        }
        let origin = index
            .temporal_inspection
            .video_timeline()
            .map_err(temporal_error)?
            .source_time
            .ok_or_else(|| invalid("RIFE original source interval unavailable"))?
            .start;
        let start = origin
            .checked_add(
                period
                    .checked_mul(self.range.first_source_ordinal)
                    .map_err(temporal_error)?,
            )
            .map_err(temporal_error)?;
        let end = origin
            .checked_add(
                period
                    .checked_mul(self.range.end_source_ordinal)
                    .map_err(temporal_error)?,
            )
            .map_err(temporal_error)?;
        let window = TimeRange::new(start, end).map_err(temporal_error)?;
        let output_period = plan
            .output_frame_rate
            .reciprocal()
            .map_err(temporal_error)?;
        let mut frames = Vec::with_capacity(count as usize * 2);
        for ordinal in self.range.first_source_ordinal * 2..self.range.end_source_ordinal * 2 {
            let (left, right, kind) = super::mapped_output_frame(&plan, ordinal)?;
            let start = origin
                .checked_add(output_period.checked_mul(ordinal).map_err(temporal_error)?)
                .map_err(temporal_error)?;
            frames.push(PreviewFrame {
                frame: RifeOutputFrame {
                    ordinal,
                    filename: format!("frame_{ordinal:08}.png"),
                    sha256: "0".repeat(64),
                    source_left_ordinal: left,
                    source_right_ordinal: right,
                    origin: kind,
                },
                presentation: TimeRange::new(
                    start,
                    start.checked_add(output_period).map_err(temporal_error)?,
                )
                .map_err(temporal_error)?,
            });
        }
        Ok((plan, window, frames))
    }
    fn schema_reference() -> crate::ConfigurationSchemaReference {
        crate::ConfigurationSchemaReference {
            id: CONFIGURATION.into(),
            version: "1.0.0".into(),
            sha256: format!(
                "{:x}",
                Sha256::digest(include_bytes!(
                    "../../docs/contracts/rife-preview-configuration-v1.schema.json"
                ))
            ),
        }
    }
    pub(super) fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self)
            .map_err(|_| invalid("cannot serialize RIFE preview configuration"))?;
        crate::ProviderConfiguration::new(
            crate::ProviderReference {
                id: PROVIDER_ID.into(),
                version: "1.0.0".into(),
            },
            crate::CapabilityReference {
                id: CAPABILITY_ID.into(),
                version: "1.0.0".into(),
            },
            Self::schema_reference(),
            values
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
    fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let original = RifeProviderConfiguration {
            schema: RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),
            configuration: self.configuration.clone(),
        }
        .manifest()?;
        let mut value = serde_json::to_value(original)
            .map_err(|_| invalid("cannot project original RIFE capability"))?;
        value["provider"]["id"] = serde_json::json!(PROVIDER_ID);
        value["provider"]["display_name"] = serde_json::json!("aniflow bounded RIFE preview");
        value["configuration_schemas"] = serde_json::json!([Self::schema_reference()]);
        let cap = &mut value["capabilities"][0];
        cap["id"] = serde_json::json!(CAPABILITY_ID);
        cap["configuration_schema"] = serde_json::json!(Self::schema_reference());
        cap["outputs"][0]["artifact_type"] =
            serde_json::json!("application/vnd.aniflow.rife-preview-frames");
        cap["outputs"][1]["artifact_type"] =
            serde_json::json!("application/vnd.aniflow.rife-preview-report+json");
        crate::ProviderManifest::from_json_slice(&crate::provider::canonical_json_bytes(&value)?)
    }
}
impl PreviewReport {
    pub(super) fn validate_against(&self, index: &RifeFrameIndex) -> Result<()> {
        let (plan, window, mut expected) = self.operation.mapping(index)?;
        if self.schema != "aniflow.rife-preview-report/v1"
            || self.full_source_plan != plan
            || self.window != window
            || self.frames.len() != expected.len()
            || self.review_scope != SCOPE
            || self.native_qualified
            || self.creative_approval != "not_performed"
        {
            return Err(invalid(
                "RIFE preview differs from its bounded original-source mapping",
            ));
        }
        require_sha256(&self.provider_lock_sha256, "RIFE preview lock")?;
        require_sha256(&self.source_index_sha256, "RIFE preview original index")?;
        require_sha256(
            &self.source_frames_sha256,
            "RIFE preview original directory",
        )?;
        for (actual, expected) in self.frames.iter().zip(&mut expected) {
            require_sha256(&actual.frame.sha256, "RIFE preview PNG")?;
            expected.frame.sha256 = actual.frame.sha256.clone();
            if *actual != *expected
                || (actual.frame.origin != RifeFrameOrigin::InterpolatedMidpoint
                    && actual.frame.sha256
                        != index.frames[actual.frame.source_left_ordinal as usize].sha256)
            {
                return Err(invalid(
                    "RIFE preview frame time/origin/source/held digest differs from original plan",
                ));
            }
        }
        Ok(())
    }
}
pub(super) fn report_template(
    operation: &PreviewConfiguration,
    index: &RifeFrameIndex,
    index_identity: &(String, u64),
    directory_sha256: &str,
    provider_lock: &str,
) -> Result<PreviewReport> {
    let (full_source_plan, window, frames) = operation.mapping(index)?;
    Ok(PreviewReport {
        schema: "aniflow.rife-preview-report/v1".into(),
        operation: operation.clone(),
        provider_lock_sha256: provider_lock.into(),
        source_index_sha256: index_identity.0.clone(),
        source_index_byte_count: index_identity.1,
        source_frames_sha256: directory_sha256.into(),
        full_source_plan,
        window,
        frames,
        review_scope: SCOPE.into(),
        native_qualified: false,
        creative_approval: "not_performed".into(),
    })
}
#[cfg(all(test, unix))]
mod tests;
