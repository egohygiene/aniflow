//! A fixed recovered-frame entry: sparse missing native work, complete carrier,
//! then existing verified master/delivery assembly. Child v3 owns acceptance.
use super::{MAXIMUM_DOCUMENT_BYTES, state, workspace};
use crate::destination::local;
use crate::frame_io::{ensure_canonical_directory, identity};
use crate::provider::{canonical_json_bytes, canonical_sha256};
use crate::upscayl_recovery::{UpscaylRecoveryConfiguration, carrier, selected};
use crate::video_assembly::{self, AssemblyConfiguration, AssemblyRequest};
use crate::{CancellationToken, PipelineV3Plan, PipelineV3RunProgress, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
mod runtime;
pub mod with_rife;
pub use runtime::{completion, resume, run, status};
#[cfg(test)]
mod tests;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredRequest {
    pub schema: String,
    pub spatial: carrier::CarrierRequest,
    pub frames_directory: Option<PathBuf>,
    pub upscale: Option<UpscaylRecoveryConfiguration>,
    pub frames_per_run: u16,
    pub assembly: AssemblyConfiguration,
    pub provider_executable: PathBuf,
    pub maximum_working_bytes: u64,
    pub minimum_free_bytes: u64,
}
impl RecoveredRequest {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4_194_304 {
            return Err(state("recovered recipe request exceeds 4 MiB"));
        }
        let value: Self = crate::provider::decode_json(bytes, "recovered music-video request")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        self.spatial.validate()?;
        self.assembly.validate()?;
        if self.schema != "aniflow.recovered-music-video-request/v1"
            || !(1..=128).contains(&self.frames_per_run)
            || !(1_048_576..=17_592_186_044_416).contains(&self.maximum_working_bytes)
            || self.minimum_free_bytes > 1_099_511_627_776
            || self.frames_directory.is_some() != self.upscale.is_some()
        {
            return Err(state(
                "recovered entry needs finite serial work bounds and paired original frames/native settings when missing work is selected",
            ));
        }
        if let Some(settings) = &self.upscale {
            settings.validate()?;
            if !settings.execution.is_per_frame() {
                return Err(state(
                    "sparse missing work requires the explicit per-frame policy; accepted existing chunks can still be reused",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissingFramePin {
    pub ordinal: u64,
    pub byte_count: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredComponent {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredIntent {
    pub schema: String,
    pub request: RecoveredRequest,
    pub initial_selection: carrier::SpatialSelectionPlan,
    pub missing_inputs: Vec<MissingFramePin>,
    pub batches: Vec<Vec<u64>>,
    pub components: Vec<RecoveredComponent>,
    pub first_native_plan: Option<PipelineV3Plan>,
    pub projected_working_bytes: u64,
    pub intent_sha256: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveredPhase {
    MissingFrames,
    CompleteCarrier,
    Assembly,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveredStepState {
    Pending,
    Incomplete,
    Complete,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredStep {
    pub id: String,
    pub phase: RecoveredPhase,
    pub state: RecoveredStepState,
    pub ordinals: Vec<u64>,
    pub run_directory: Option<PathBuf>,
    pub native_plan_sha256: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredPlan {
    pub schema: String,
    pub intent: RecoveredIntent,
    pub steps: Vec<RecoveredStep>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveredStatus {
    pub schema: String,
    pub run_directory: PathBuf,
    pub intent_sha256: String,
    pub reused_frames: u64,
    pub missing_frames: u64,
    pub steps: Vec<RecoveredStep>,
    pub carrier_sha256: Option<String>,
    pub complete: bool,
    pub completion: Option<crate::final_video::FinalVideoHandoff>,
    pub creative_approval: String,
    pub publication_authority: String,
}
fn batches(missing: &[u64], size: u16) -> Result<Vec<Vec<u64>>> {
    if !(1..=128).contains(&size)
        || missing.windows(2).any(|w| w[0] >= w[1])
        || missing.iter().any(|v| *v >= 100_000)
    {
        return Err(state(
            "missing work must preserve strictly increasing original ordinals in 1..128-frame runs",
        ));
    }
    Ok(missing.chunks(size as usize).map(|s| s.to_vec()).collect())
}
fn selected_request(
    request: &RecoveredRequest,
    ordinals: Vec<u64>,
) -> Result<selected::SelectedRecoveryRequest> {
    Ok(selected::SelectedRecoveryRequest {
        schema: "aniflow.selected-spatial-recovery-request/v1".into(),
        source_video: request.spatial.source_video.clone(),
        source_index: request.spatial.source_index.clone(),
        frames_directory: request
            .frames_directory
            .clone()
            .ok_or_else(|| state("missing original frames directory was not selected"))?,
        ordinals,
        configuration: request
            .upscale
            .clone()
            .ok_or_else(|| state("missing-work native settings were not selected"))?,
        provider_executable: request.provider_executable.clone(),
    })
}
fn digest(intent: &RecoveredIntent) -> Result<String> {
    canonical_sha256(&(
        &intent.schema,
        &intent.request,
        &intent.initial_selection,
        &intent.missing_inputs,
        &intent.batches,
        &intent.components,
        &intent.first_native_plan,
        intent.projected_working_bytes,
    ))
}
fn bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = canonical_json_bytes(value)?;
    if bytes.len() as u64 > MAXIMUM_DOCUMENT_BYTES {
        return Err(state("recovered recipe evidence exceeds 64 MiB"));
    }
    Ok(bytes)
}
fn components(request: &RecoveredRequest, needs_native: bool) -> Result<Vec<RecoveredComponent>> {
    let mut values = std::collections::BTreeMap::new();
    values.insert(
        request.provider_executable.clone(),
        identity(&request.provider_executable, 134_217_728)?,
    );
    for (pin, observed) in [&request.assembly.ffmpeg, &request.assembly.ffprobe]
        .into_iter()
        .zip(video_assembly::observe_components(&request.assembly)?)
    {
        values.insert(pin.executable.clone(), observed);
    }
    if needs_native {
        let settings = request
            .upscale
            .as_ref()
            .ok_or_else(|| state("missing native settings"))?;
        for pin in [&settings.upscayl, &settings.import.ffmpeg] {
            let observed = identity(&pin.executable, 134_217_728)?;
            if observed.0 != pin.sha256 {
                return Err(state("recovered native tool pin changed"));
            }
            values.insert(pin.executable.clone(), observed);
        }
        for pin in [&settings.model.param, &settings.model.weights] {
            let observed = identity(&pin.path, 1_073_741_824)?;
            if observed.0 != pin.sha256 {
                return Err(state("recovered model pin changed"));
            }
            values.insert(pin.path.clone(), observed);
        }
    }
    Ok(values
        .into_iter()
        .map(|(path, (sha256, byte_count))| RecoveredComponent {
            path,
            sha256,
            byte_count,
        })
        .collect())
}
pub fn plan(request: &RecoveredRequest, token: &CancellationToken) -> Result<RecoveredPlan> {
    request.validate()?;
    crate::destination::cancelled(token)?;
    let initial_selection = carrier::inspect(&request.spatial, token)?;
    let batches = batches(&initial_selection.missing, request.frames_per_run)?;
    if request.spatial.sources.len() + batches.len() > 1024 {
        return Err(state(
            "recovered selection plus new native sources exceeds 1024 source groups; choose a larger bounded frames_per_run or a smaller explicit job",
        ));
    }
    let mut missing_inputs = Vec::new();
    if !batches.is_empty() {
        let native = request.upscale.as_ref().ok_or_else(|| {
            state("missing frames require explicit native configuration and original PNG directory")
        })?;
        if native.output_dimensions(
            initial_selection.source_index.width,
            initial_selection.source_index.height,
        )? != (request.spatial.output_width, request.spatial.output_height)
        {
            return Err(state(
                "missing native work and recovered selections require identical geometry",
            ));
        }
        let directory = request
            .frames_directory
            .as_ref()
            .ok_or_else(|| state("missing original PNG directory"))?;
        ensure_canonical_directory(directory)?;
        let mut total = 0u64;
        for ordinal in &initial_selection.missing {
            let frame = &initial_selection.source_index.frames[*ordinal as usize];
            let observed = crate::frame_io::inspect_png(
                &directory.join(&frame.filename),
                initial_selection.source_index.width,
                initial_selection.source_index.height,
                native.import.maximum_frame_bytes,
            )?;
            if observed.0 != frame.sha256 {
                return Err(state(
                    "missing original frame differs from immutable index before expensive work",
                ));
            }
            total = total
                .checked_add(observed.1)
                .ok_or_else(|| state("missing original frame byte overflow"))?;
            missing_inputs.push(MissingFramePin {
                ordinal: *ordinal,
                byte_count: observed.1,
            });
        }
        if total > native.import.maximum_total_frame_bytes {
            return Err(state("missing original frames exceed selected input bound"));
        }
    }
    let first_native_plan = batches
        .first()
        .map(|part| selected::plan(&selected_request(request, part.clone())?, token))
        .transpose()?;
    let components = components(request, !batches.is_empty())?;
    let missing_output = (initial_selection.missing.len() as u64)
        .checked_mul(
            request
                .upscale
                .as_ref()
                .map_or(0, |c| c.maximum_output_frame_bytes),
        )
        .ok_or_else(|| state("recovered output bound overflow"))?;
    let metadata_bytes = batches.iter().try_fold(0u64, |total, batch| {
        ((batch.len() as u64 + 1).checked_pow(2))
            .and_then(|v| v.checked_mul(16_384))
            .and_then(|v| total.checked_add(v))
            .ok_or_else(|| state("recovered retained metadata budget overflow"))
    })?;
    let input_bytes = missing_inputs.iter().try_fold(0u64, |total, f| {
        total
            .checked_add(f.byte_count)
            .ok_or_else(|| state("recovered input byte sum overflow"))
    })?;
    let component_bytes = components.iter().try_fold(0u64, |total, f| {
        total
            .checked_add(f.byte_count)
            .ok_or_else(|| state("recovered component byte sum overflow"))
    })?;
    let projected_working_bytes = missing_output
        .checked_mul(5)
        .and_then(|v| v.checked_add(initial_selection.total_bytes.checked_mul(3)?))
        .and_then(|v| v.checked_add(metadata_bytes))
        .and_then(|v| v.checked_add(input_bytes))
        .and_then(|v| v.checked_add(component_bytes.checked_mul(3)?))
        .and_then(|v| v.checked_add(request.assembly.maximum_output_bytes.checked_mul(4)?))
        .and_then(|v| v.checked_add(MAXIMUM_DOCUMENT_BYTES * 4))
        .ok_or_else(|| state("recovered aggregate working budget overflow"))?;
    if projected_working_bytes > request.maximum_working_bytes {
        return Err(state(
            "recovered selected native/carrier/assembly bounds exceed whole-recipe working admission",
        ));
    }
    let mut intent = RecoveredIntent {
        schema: "aniflow.recovered-music-video-intent/v1".into(),
        request: request.clone(),
        initial_selection,
        missing_inputs,
        batches,
        components,
        first_native_plan,
        projected_working_bytes,
        intent_sha256: String::new(),
    };
    intent.intent_sha256 = digest(&intent)?;
    bytes(&intent)?;
    let mut steps = intent
        .batches
        .iter()
        .enumerate()
        .map(|(i, ordinals)| RecoveredStep {
            id: format!("missing_{i:04}"),
            phase: RecoveredPhase::MissingFrames,
            state: RecoveredStepState::Pending,
            ordinals: ordinals.clone(),
            run_directory: None,
            native_plan_sha256: if i == 0 {
                intent
                    .first_native_plan
                    .as_ref()
                    .map(|p| p.plan_sha256.clone())
            } else {
                None
            },
        })
        .collect::<Vec<_>>();
    steps.extend([
        RecoveredStep {
            id: "carrier".into(),
            phase: RecoveredPhase::CompleteCarrier,
            state: RecoveredStepState::Pending,
            ordinals: vec![],
            run_directory: None,
            native_plan_sha256: None,
        },
        RecoveredStep {
            id: "assembly".into(),
            phase: RecoveredPhase::Assembly,
            state: RecoveredStepState::Pending,
            ordinals: vec![],
            run_directory: None,
            native_plan_sha256: None,
        },
    ]);
    crate::destination::cancelled(token)?;
    Ok(RecoveredPlan {
        schema: "aniflow.recovered-music-video-plan/v1".into(),
        intent,
        steps,
    })
}
fn child(root: &Path, id: &str) -> PathBuf {
    root.join("children").join(id)
}
fn completed_request(root: &Path, intent: &RecoveredIntent) -> carrier::CarrierRequest {
    let mut request = intent.request.spatial.clone();
    for (i, ordinals) in intent.batches.iter().enumerate() {
        request.sources.push(carrier::SpatialSelection::Native {
            run_directory: child(root, &format!("missing_{i:04}")),
            configuration: intent.request.upscale.clone().unwrap(),
            units: ordinals
                .iter()
                .map(|ordinal| carrier::NativeUnitSelection {
                    stage_id: format!("upscale_frame_{ordinal:06}"),
                    ordinals: vec![*ordinal],
                })
                .collect(),
        });
    }
    request
}
fn assembly_request(root: &Path, request: &RecoveredRequest) -> AssemblyRequest {
    AssemblyRequest::new(
        &request.spatial.source_video,
        root.join("carrier/frames"),
        root.join("carrier/carrier.json"),
        request.assembly.clone(),
        &request.provider_executable,
    )
}
fn present(path: &Path, directory: bool) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            workspace::require_existing_canonical(path)?;
            if metadata.is_dir() != directory || !directory && !metadata.is_file() {
                return Err(state("recovered slot has the wrong filesystem kind"));
            }
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(state("cannot inspect recovered slot")),
    }
}
