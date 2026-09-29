//! Bounded MIDI candidates through the existing audio Pipeline v3 lifecycle.
//!
//! Technical inspection and optional stem lineage precede the local estimator.
//! Note candidates preserve their upstream source clock, channel and stem scope.
mod export;
mod midi;
mod normalize;
mod provider;
mod types;

use std::path::PathBuf;

pub use export::{
    AUDIO_MIDI_EXPORT_SCHEMA_V1, MidiCandidateAuthority, MidiExportMapping, MidiExportOutcome,
    MidiExportProfile, MidiExportReport, export_midi_file,
};
pub use midi::{
    MIDI_MAXIMUM_BYTES, MIDI_MICROSECONDS_PER_QUARTER, MIDI_TICKS_PER_QUARTER, MidiReadBack,
    MidiReadBackNote, encode_midi, read_midi,
};
pub use normalize::{normalize, normalized_analysis};
pub use types::*;

use crate::audio_inspection::{
    self, AudioInspectionConfiguration, AudioInspectionFailure, AudioInspectionRequest,
};
use crate::{
    CancellationToken, ComponentIdentity, ComponentInventory, Error, ErrorCategory,
    PipelineInputBinding, PipelineV3Configuration, PipelineV3Plan, PipelineV3ResumeRequest,
    PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunRequest, ProviderInvocationRequest,
    ProviderManifest, ProviderRegistration, ProviderRegistry,
};

const INSPECTION_PIPELINE: &[u8] = include_bytes!("../../providers/audio-inspection/pipeline.yml");
const MIDI_PIPELINE: &[u8] = include_bytes!("../../providers/audio-midi/pipeline.yml");
const MANIFEST: &[u8] = include_bytes!("../../providers/audio-midi/manifest.json");

#[derive(Debug, Clone)]
pub struct MidiRequest {
    pub inspection: AudioInspectionRequest,
    pub configuration: MidiConfiguration,
}

impl MidiRequest {
    #[must_use]
    pub const fn new(inspection: AudioInspectionRequest, configuration: MidiConfiguration) -> Self {
        Self {
            inspection,
            configuration,
        }
    }
}

/// Suggested outer artifact bound for simultaneous private audio, model and adapter staging.
/// `MidiRequest::new` retains all explicitly supplied inspection limits.
#[must_use]
pub const fn default_execution_limits() -> crate::ProviderExecutionLimits {
    let mut limits = audio_inspection::default_execution_limits();
    limits.maximum_artifact_bytes = 384 * 1024 * 1024;
    limits
}

/// Reobserve pinned local interpreter, adapter, runtime inventory and model without inference.
pub fn preflight(
    configuration: &MidiConfiguration,
    tools: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> crate::Result<AudioMidiPreflight> {
    provider::preflight(configuration, tools, cancellation)
}

/// A failure retaining the typed dependency observations or pipeline planning refusal.
#[derive(Debug)]
pub struct AudioMidiFailure {
    pub error: Error,
    pub preflight: Option<Box<AudioMidiPreflight>>,
    pub inspection_preflight: Option<Box<crate::audio_inspection::AudioInspectionPreflight>>,
    pub planning: Option<Box<crate::PipelinePlanningFailure>>,
}
impl std::fmt::Display for AudioMidiFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}
impl std::error::Error for AudioMidiFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
impl From<Error> for AudioMidiFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            preflight: None,
            inspection_preflight: None,
            planning: None,
        }
    }
}
impl From<AudioInspectionFailure> for AudioMidiFailure {
    fn from(failure: AudioInspectionFailure) -> Self {
        Self {
            error: failure.error,
            preflight: None,
            inspection_preflight: failure.preflight,
            planning: failure.planning,
        }
    }
}
impl From<crate::PipelinePlanningFailure> for AudioMidiFailure {
    fn from(planning: crate::PipelinePlanningFailure) -> Self {
        Self {
            error: Error::new(planning.category(), planning.message.clone()),
            preflight: None,
            inspection_preflight: None,
            planning: Some(Box::new(planning)),
        }
    }
}

fn prepare_plan(
    request: &MidiRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<
    (PipelineV3Plan, ProviderRegistry, Vec<PipelineInputBinding>),
    AudioMidiFailure,
> {
    request.configuration.validate()?;
    let mut prepared = audio_inspection::prepare_registry(&request.inspection, cancellation)?;
    let preflight = preflight(
        &request.configuration,
        &request.inspection.configuration,
        cancellation,
    )?;
    if !preflight.is_ready() {
        return Err(AudioMidiFailure {
            error: Error::new(
                ErrorCategory::Dependency,
                "midi dependencies are unavailable; inspect typed preflight diagnostics",
            ),
            preflight: Some(Box::new(preflight)),
            inspection_preflight: None,
            planning: None,
        });
    }
    let mut pipeline = PipelineV3Configuration::from_yaml_slice(INSPECTION_PIPELINE)?;
    audio_inspection::attach_selected_stem(&request.inspection, &mut pipeline, &mut prepared)?;
    let upstream_analysis_artifact_id = pipeline
        .outputs
        .iter()
        .find(|output| output.id == "analysis")
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "midi pipeline requires upstream normalized analysis",
            )
        })?
        .artifact
        .clone();
    let dependency = pipeline
        .stages
        .iter()
        .find(|stage| {
            stage.outputs.iter().any(|port| {
                port.artifacts
                    .iter()
                    .any(|artifact| artifact.id == upstream_analysis_artifact_id)
            })
        })
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "midi upstream analysis producer is missing",
            )
        })?
        .id
        .clone();
    let configuration = AudioMidiProviderConfiguration {
        schema: AUDIO_MIDI_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
        settings: request.configuration.clone(),
        tools: request.inspection.configuration.clone(),
        source: prepared.source.clone(),
        upstream_analysis_artifact_id: upstream_analysis_artifact_id.clone(),
    }
    .provider_configuration()?;
    let mut manifest = ProviderManifest::from_json_slice(MANIFEST)?;
    for requirement in &mut manifest.capabilities[0].requirements.tools {
        if requirement.id != "python" {
            return Err(Error::new(
                ErrorCategory::Internal,
                "midi manifest has an unsupported native tool requirement",
            )
            .into());
        }
        requirement.version_requirement = format!("={}", request.configuration.python.version);
        requirement.sha256 = Some(request.configuration.python.sha256.clone());
    }
    manifest.capabilities[0].requirements.models[0].sha256 =
        Some(request.configuration.model.sha256.clone());
    prepared.registry.register(ProviderRegistration::new(
        "audio-midi-native",
        manifest,
        configuration,
        &request.inspection.provider_executable,
        "aniflow-audio-midi-v1",
        ComponentInventory {
            tools: vec![ComponentIdentity {
                id: "python".to_owned(),
                version: request.configuration.python.version.clone(),
                sha256: Some(request.configuration.python.sha256.clone()),
            }],
            codecs: Vec::new(),
            models: vec![ComponentIdentity {
                id: "basic-pitch-onnx-icassp-2022".to_owned(),
                version: "1.0.0".to_owned(),
                sha256: Some(request.configuration.model.sha256.clone()),
            }],
        },
    )?)?;
    let mut extension = PipelineV3Configuration::from_yaml_slice(MIDI_PIPELINE)?;
    if extension.stages.len() != 1 {
        return Err(Error::new(
            ErrorCategory::Internal,
            "midi pipeline extension requires exactly one stage",
        )
        .into());
    }
    let mut stage = extension.stages.remove(0);
    stage.depends_on = vec![dependency];
    for input in &mut stage.inputs {
        if input.port == "upstream_analysis" {
            input.artifacts = vec![upstream_analysis_artifact_id.clone()];
        }
    }
    pipeline.stages.push(stage);
    pipeline.outputs.retain(|output| output.id != "analysis");
    pipeline.outputs.extend(extension.outputs);
    let mut context = audio_inspection::context();
    context.allowed_side_effects.push(crate::SideEffect::Ai);
    let plan =
        crate::resolve_pipeline_v3(&pipeline, &prepared.bindings, &prepared.registry, &context)?;
    audio_inspection::bind_source_plan(&plan, &prepared.source)?;
    crate::audio_stem::bind_authorities_plan(&plan, prepared.stem.as_ref())?;
    Ok((plan, prepared.registry, prepared.bindings))
}

/// Resolve inspection, optional stem lineage, and candidate MIDI stages.
pub fn plan(
    request: &MidiRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<PipelineV3Plan, AudioMidiFailure> {
    Ok(prepare_plan(request, cancellation)?.0)
}

pub fn run<F>(
    request: MidiRequest,
    output_directory: Option<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioMidiFailure>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let (plan, registry, bindings) = prepare_plan(&request, cancellation)?;
    let mut execution = PipelineV3RunRequest::new(plan, bindings, registry)
        .with_execution_limits(request.inspection.execution_limits);
    execution.output_directory = output_directory;
    Ok(crate::run_v3_with_progress_and_cancellation(
        execution,
        cancellation,
        on_progress,
    )?)
}

/// Reobserve dependencies and all retained lineage before exact-plan checkpoint reuse.
pub fn resume<F>(
    request: MidiRequest,
    run_directory: impl Into<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioMidiFailure>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let run_directory = run_directory.into();
    let (plan, registry, bindings) = prepare_plan(&request, cancellation)?;
    audio_inspection::require_same_plan(&run_directory, &plan)?;
    let execution = PipelineV3ResumeRequest::new(run_directory, bindings, registry)
        .with_execution_limits(request.inspection.execution_limits);
    Ok(crate::resume_v3_with_progress_and_cancellation(
        execution,
        cancellation,
        on_progress,
    )?)
}

pub(crate) fn execute_provider_invocation(
    request: &ProviderInvocationRequest,
) -> crate::Result<()> {
    provider::execute_invocation(request)
}
