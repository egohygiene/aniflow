//! Bounded musical estimates through the existing audio Pipeline v3 lifecycle.
//!
//! Technical inspection and optional stem lineage precede the local estimator.
//! Musical candidates preserve their upstream source clock, channel and stem scope.
mod provider;
mod types;

use std::path::PathBuf;

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
const MUSICAL_PIPELINE: &[u8] = include_bytes!("../../providers/audio-musical/pipeline.yml");
const MANIFEST: &[u8] = include_bytes!("../../providers/audio-musical/manifest.json");

#[derive(Debug, Clone)]
pub struct MusicalAnalysisRequest {
    pub inspection: AudioInspectionRequest,
    pub configuration: MusicalAnalysisConfiguration,
}

impl MusicalAnalysisRequest {
    #[must_use]
    pub const fn new(
        inspection: AudioInspectionRequest,
        configuration: MusicalAnalysisConfiguration,
    ) -> Self {
        Self {
            inspection,
            configuration,
        }
    }
}

/// Reobserve configured Python, adapter and package identities without processing media.
pub fn preflight(
    configuration: &MusicalAnalysisConfiguration,
    tools: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> crate::Result<AudioMusicalProbe> {
    provider::preflight(configuration, tools, cancellation)
}

fn prepare_plan(
    request: &MusicalAnalysisRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<
    (PipelineV3Plan, ProviderRegistry, Vec<PipelineInputBinding>),
    AudioInspectionFailure,
> {
    request.configuration.validate()?;
    let mut prepared = audio_inspection::prepare_registry(&request.inspection, cancellation)?;
    preflight(
        &request.configuration,
        &request.inspection.configuration,
        cancellation,
    )?;
    let mut pipeline = PipelineV3Configuration::from_yaml_slice(INSPECTION_PIPELINE)?;
    audio_inspection::attach_selected_stem(&request.inspection, &mut pipeline, &mut prepared)?;
    let upstream_analysis_artifact_id = pipeline
        .outputs
        .iter()
        .find(|output| output.id == "analysis")
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "musical pipeline requires upstream normalized analysis",
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
                "musical upstream analysis producer is missing",
            )
        })?
        .id
        .clone();
    let configuration = AudioMusicalProviderConfiguration {
        schema: AUDIO_MUSICAL_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
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
                "musical manifest has an unsupported native tool requirement",
            )
            .into());
        }
        requirement.version_requirement = format!("={}", request.configuration.python.version);
        requirement.sha256 = Some(request.configuration.python.sha256.clone());
    }
    prepared.registry.register(ProviderRegistration::new(
        "audio-musical-native",
        manifest,
        configuration,
        &request.inspection.provider_executable,
        "aniflow-audio-musical-v1",
        ComponentInventory {
            tools: vec![ComponentIdentity {
                id: "python".to_owned(),
                version: request.configuration.python.version.clone(),
                sha256: Some(request.configuration.python.sha256.clone()),
            }],
            codecs: Vec::new(),
            models: Vec::new(),
        },
    )?)?;
    let mut extension = PipelineV3Configuration::from_yaml_slice(MUSICAL_PIPELINE)?;
    if extension.stages.len() != 1 {
        return Err(Error::new(
            ErrorCategory::Internal,
            "musical pipeline extension requires exactly one stage",
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
    audio_inspection::resolve_prepared(pipeline, prepared)
}

/// Preflight pinned dependencies and resolve inspection, lineage and musical stages.
pub fn plan(
    request: &MusicalAnalysisRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<PipelineV3Plan, AudioInspectionFailure> {
    Ok(prepare_plan(request, cancellation)?.0)
}

pub fn run<F>(
    request: MusicalAnalysisRequest,
    output_directory: Option<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioInspectionFailure>
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
    request: MusicalAnalysisRequest,
    run_directory: impl Into<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioInspectionFailure>
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
