//! Separate versioned fixed recovered continuation with optional interpolation.
use super::*;
use crate::video_interpolation::recovered::{RecoveredBinding, RecoveredRifeRequest};
use crate::video_interpolation::{self as rife, RifeConfiguration};
mod runtime;
pub use runtime::{completion, resume, run, status};
#[cfg(test)]
mod tests;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithRifeRequest {
    pub schema: String,
    pub base: RecoveredRequest,
    pub interpolation: Option<RifeConfiguration>,
}
impl WithRifeRequest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4_194_304 {
            return Err(state("recovered v2 request exceeds 4 MiB"));
        }
        let value: Self = crate::provider::decode_json(bytes, "recovered recipe v2 request")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.base.validate()?;
        if self.schema != "aniflow.recovered-music-video-request/v2" {
            return Err(state("unsupported recovered v2 request"));
        }
        if let Some(configuration) = &self.interpolation {
            configuration.validate()?;
            if !configuration.enabled {
                return Err(state(
                    "omit interpolation for bypass; a selected RIFE profile must be explicitly enabled",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithRifeIntent {
    pub schema: String,
    pub request: WithRifeRequest,
    pub spatial: RecoveredIntent,
    pub interpolation_components: Vec<RecoveredComponent>,
    pub projected_working_bytes: u64,
    pub intent_sha256: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithRifePhase {
    MissingFrames,
    CompleteCarrier,
    Interpolation,
    AssemblyCarrier,
    Assembly,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithRifeStep {
    pub id: String,
    pub phase: WithRifePhase,
    pub state: RecoveredStepState,
    pub ordinals: Vec<u64>,
    pub run_directory: Option<PathBuf>,
    pub native_plan_sha256: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithRifePlan {
    pub schema: String,
    pub intent: WithRifeIntent,
    pub steps: Vec<WithRifeStep>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithRifeStatus {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    pub reused_frames: u64,
    pub missing_frames: u64,
    pub interpolation_selected: bool,
    pub steps: Vec<WithRifeStep>,
    pub carrier_sha256: Option<String>,
    pub complete: bool,
    pub completion: Option<crate::final_video::FinalVideoHandoff>,
    pub creative_approval: String,
    pub publication_authority: String,
}
fn digest(intent: &WithRifeIntent) -> Result<String> {
    canonical_sha256(&(
        &intent.schema,
        &intent.request,
        &intent.spatial,
        &intent.interpolation_components,
        intent.projected_working_bytes,
    ))
}
fn extra_components(
    configuration: Option<&RifeConfiguration>,
    token: &CancellationToken,
) -> Result<Vec<RecoveredComponent>> {
    configuration
        .map(|c| {
            rife::runtime::observe_components(c, token).map(|parts| {
                parts
                    .into_iter()
                    .map(|p| RecoveredComponent {
                        path: p.path,
                        sha256: p.sha256,
                        byte_count: p.byte_count,
                    })
                    .collect()
            })
        })
        .transpose()
        .map(|v| v.unwrap_or_default())
}
fn phases(interpolation: bool) -> Vec<WithRifePhase> {
    let mut phases = vec![WithRifePhase::CompleteCarrier];
    if interpolation {
        phases.extend([WithRifePhase::Interpolation, WithRifePhase::AssemblyCarrier]);
    }
    phases.push(WithRifePhase::Assembly);
    phases
}
fn pending(id: impl Into<String>, phase: WithRifePhase) -> WithRifeStep {
    WithRifeStep {
        id: id.into(),
        phase,
        state: RecoveredStepState::Pending,
        ordinals: vec![],
        run_directory: None,
        native_plan_sha256: None,
    }
}
fn phase_id(phase: WithRifePhase) -> &'static str {
    match phase {
        WithRifePhase::MissingFrames => "missing",
        WithRifePhase::CompleteCarrier => "carrier",
        WithRifePhase::Interpolation => "interpolate",
        WithRifePhase::AssemblyCarrier => "rife-assembly",
        WithRifePhase::Assembly => "assembly",
    }
}
pub fn plan(request: &WithRifeRequest, token: &CancellationToken) -> Result<WithRifePlan> {
    request.validate()?;
    let spatial = super::plan(&request.base, token)?.intent;
    if let Some(configuration) = &request.interpolation {
        let shape = rife::plan(configuration, &spatial.initial_selection.source_index)?;
        if shape.output_frame_count > rife::RIFE_MAXIMUM_FRAMES {
            return Err(state(
                "recovered interpolation exceeds existing 50000-source/100000-output RIFE profile before missing work",
            ));
        }
        rife::spatial::admit_geometry(
            configuration,
            request.base.spatial.output_width,
            request.base.spatial.output_height,
        )?;
    }
    let interpolation_components = extra_components(request.interpolation.as_ref(), token)?;
    let additional = request
        .interpolation
        .as_ref()
        .map(|c| {
            c.settings
                .maximum_artifact_bytes
                .checked_mul(3)
                .and_then(|v| v.checked_add(MAXIMUM_DOCUMENT_BYTES * 3))
                .ok_or_else(|| state("recovered RIFE whole-work admission overflow"))
        })
        .transpose()?
        .unwrap_or(0);
    let projected_working_bytes = spatial
        .projected_working_bytes
        .checked_add(additional)
        .ok_or_else(|| state("recovered v2 working sum overflow"))?;
    if projected_working_bytes > request.base.maximum_working_bytes {
        return Err(state(
            "recovered optional interpolation exceeds whole-recipe working admission",
        ));
    }
    let mut intent = WithRifeIntent {
        schema: "aniflow.recovered-music-video-intent/v2".into(),
        request: request.clone(),
        spatial,
        interpolation_components,
        projected_working_bytes,
        intent_sha256: String::new(),
    };
    intent.intent_sha256 = digest(&intent)?;
    bytes(&intent)?;
    let mut steps = intent
        .spatial
        .batches
        .iter()
        .enumerate()
        .map(|(i, part)| WithRifeStep {
            id: format!("missing_{i:04}"),
            phase: WithRifePhase::MissingFrames,
            state: RecoveredStepState::Pending,
            ordinals: part.clone(),
            run_directory: None,
            native_plan_sha256: if i == 0 {
                intent
                    .spatial
                    .first_native_plan
                    .as_ref()
                    .map(|p| p.plan_sha256.clone())
            } else {
                None
            },
        })
        .collect::<Vec<_>>();
    steps.extend(
        phases(request.interpolation.is_some())
            .into_iter()
            .map(|p| pending(phase_id(p), p)),
    );
    Ok(WithRifePlan {
        schema: "aniflow.recovered-music-video-plan/v2".into(),
        intent,
        steps,
    })
}
fn rife_request(root: &Path, intent: &WithRifeIntent, pin: &str) -> Result<RecoveredRifeRequest> {
    Ok(RecoveredRifeRequest {
        schema: "aniflow.recovered-rife-request/v1".into(),
        source: RecoveredBinding {
            carrier_directory: root.join("carrier"),
            carrier_sha256: pin.into(),
        },
        configuration: intent
            .request
            .interpolation
            .clone()
            .ok_or_else(|| state("interpolation was not selected"))?,
        provider_executable: intent.request.base.provider_executable.clone(),
    })
}
fn assembly_request(root: &Path, intent: &WithRifeIntent) -> Result<AssemblyRequest> {
    let base = &intent.request.base;
    if intent.request.interpolation.is_none() {
        return Ok(super::assembly_request(root, base));
    }
    let path = root.join("rife-assembly.json");
    let carrier: video_assembly::rife::RifeAssemblyIndex = workspace::read_document(&path)?;
    let frames = carrier.validate_binding()?;
    Ok(AssemblyRequest::new(
        &base.spatial.source_video,
        frames,
        path,
        base.assembly.clone(),
        &base.provider_executable,
    ))
}
