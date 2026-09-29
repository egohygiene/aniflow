//! Bounded candidate alignment of reviewed lyrics through the audio Pipeline v3 lifecycle.
//!
//! Technical inspection and optional stem lineage precede the local aligner.
//! Original reviewed text and authority remain immutable beside candidate timing.
mod export;
mod normalize;
mod provider;
mod types;

use std::path::PathBuf;

pub use export::{AlignmentExportOutcome, AlignmentTimingAuthority, export_alignment_file};
pub use normalize::{alignment_phrase, normalize, normalized_analysis, timed_text, tokenize};
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
const ALIGNMENT_PIPELINE: &[u8] = include_bytes!("../../providers/audio-alignment/pipeline.yml");
const MANIFEST: &[u8] = include_bytes!("../../providers/audio-alignment/manifest.json");

#[derive(Debug, Clone)]
pub struct AlignmentRequest {
    pub inspection: AudioInspectionRequest,
    pub configuration: AlignmentConfiguration,
    pub lyrics: PathBuf,
}

impl AlignmentRequest {
    #[must_use]
    pub fn new(
        inspection: AudioInspectionRequest,
        configuration: AlignmentConfiguration,
        lyrics: impl Into<PathBuf>,
    ) -> Self {
        Self {
            inspection,
            configuration,
            lyrics: lyrics.into(),
        }
    }
}

/// Suggested outer artifact bound for simultaneous private audio, model and tool staging.
/// `AlignmentRequest::new` retains all explicitly supplied inspection limits.
#[must_use]
pub const fn default_execution_limits() -> crate::ProviderExecutionLimits {
    let mut limits = audio_inspection::default_execution_limits();
    limits.maximum_artifact_bytes = 512 * 1024 * 1024;
    limits.maximum_artifact_files = 24;
    limits
}

/// Reobserve the pinned local executable and model without processing media.
pub fn preflight(
    configuration: &AlignmentConfiguration,
    tools: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> crate::Result<AudioAlignmentPreflight> {
    provider::preflight(configuration, tools, cancellation)
}

/// A failure retaining the typed dependency observations or pipeline planning refusal.
#[derive(Debug)]
pub struct AudioAlignmentFailure {
    pub error: Error,
    pub preflight: Option<Box<AudioAlignmentPreflight>>,
    pub inspection_preflight: Option<Box<crate::audio_inspection::AudioInspectionPreflight>>,
    pub planning: Option<Box<crate::PipelinePlanningFailure>>,
}
impl std::fmt::Display for AudioAlignmentFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}
impl std::error::Error for AudioAlignmentFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
impl From<Error> for AudioAlignmentFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            preflight: None,
            inspection_preflight: None,
            planning: None,
        }
    }
}
impl From<AudioInspectionFailure> for AudioAlignmentFailure {
    fn from(failure: AudioInspectionFailure) -> Self {
        Self {
            error: failure.error,
            preflight: None,
            inspection_preflight: failure.preflight,
            planning: failure.planning,
        }
    }
}
impl From<crate::PipelinePlanningFailure> for AudioAlignmentFailure {
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
    request: &AlignmentRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<
    (PipelineV3Plan, ProviderRegistry, Vec<PipelineInputBinding>),
    AudioAlignmentFailure,
> {
    request.configuration.validate()?;
    let reviewed = provider::read_reviewed_lyrics(&request.lyrics)?;
    let mut prepared = audio_inspection::prepare_registry(&request.inspection, cancellation)?;
    let preflight = preflight(
        &request.configuration,
        &request.inspection.configuration,
        cancellation,
    )?;
    if !preflight.is_ready() {
        return Err(AudioAlignmentFailure {
            error: Error::new(
                ErrorCategory::Dependency,
                "alignment dependencies are unavailable; inspect typed preflight diagnostics",
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
                "alignment pipeline requires upstream normalized analysis",
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
                "alignment upstream analysis producer is missing",
            )
        })?
        .id
        .clone();
    let configuration = AudioAlignmentProviderConfiguration {
        schema: AUDIO_ALIGNMENT_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
        settings: request.configuration.clone(),
        tools: request.inspection.configuration.clone(),
        source: prepared.source.clone(),
        reviewed_lyrics: reviewed.artifact.clone(),
        upstream_analysis_artifact_id: upstream_analysis_artifact_id.clone(),
    }
    .provider_configuration()?;
    let mut manifest = ProviderManifest::from_json_slice(MANIFEST)?;
    for requirement in &mut manifest.capabilities[0].requirements.tools {
        if requirement.id != "pocketsphinx" {
            return Err(Error::new(
                ErrorCategory::Internal,
                "alignment manifest has an unsupported native tool requirement",
            )
            .into());
        }
        requirement.version_requirement =
            format!("={}", request.configuration.pocketsphinx.version);
        requirement.sha256 = Some(request.configuration.pocketsphinx.sha256.clone());
    }
    let model_sha256 = crate::provider::canonical_sha256(&request.configuration.model.files)?;
    for requirement in &mut manifest.capabilities[0].requirements.models {
        requirement.sha256 = Some(match requirement.id.as_str() {
            "en-us" => model_sha256.clone(),
            "cmudict-en-us" => request.configuration.dictionary.sha256.clone(),
            _ => {
                return Err(Error::new(
                    ErrorCategory::Internal,
                    "alignment manifest has an unsupported model requirement",
                )
                .into());
            }
        });
    }
    prepared.registry.register(ProviderRegistration::new(
        "audio-alignment-native",
        manifest,
        configuration,
        &request.inspection.provider_executable,
        "aniflow-audio-alignment-v1",
        ComponentInventory {
            tools: vec![ComponentIdentity {
                id: "pocketsphinx".to_owned(),
                version: request.configuration.pocketsphinx.version.clone(),
                sha256: Some(request.configuration.pocketsphinx.sha256.clone()),
            }],
            codecs: Vec::new(),
            models: vec![
                ComponentIdentity {
                    id: "cmudict-en-us".to_owned(),
                    version: "1.0.0".to_owned(),
                    sha256: Some(request.configuration.dictionary.sha256.clone()),
                },
                ComponentIdentity {
                    id: "en-us".to_owned(),
                    version: "1.0.0".to_owned(),
                    sha256: Some(model_sha256),
                },
            ],
        },
    )?)?;
    let mut extension = PipelineV3Configuration::from_yaml_slice(ALIGNMENT_PIPELINE)?;
    if extension.stages.len() != 1 {
        return Err(Error::new(
            ErrorCategory::Internal,
            "alignment pipeline extension requires exactly one stage",
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
    let lyrics_input = extension
        .inputs
        .into_iter()
        .find(|input| input.id == "reviewed_lyrics")
        .ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "alignment pipeline must declare reviewed lyrics",
            )
        })?;
    pipeline.inputs.push(lyrics_input);
    prepared.bindings.push(PipelineInputBinding::new(
        "reviewed_lyrics",
        &request.lyrics,
    ));
    pipeline.stages.push(stage);
    pipeline.outputs.retain(|output| output.id != "analysis");
    pipeline.outputs.extend(extension.outputs);
    let mut context = audio_inspection::context();
    context.allowed_side_effects.push(crate::SideEffect::Ai);
    let plan =
        crate::resolve_pipeline_v3(&pipeline, &prepared.bindings, &prepared.registry, &context)?;
    audio_inspection::bind_source_plan(&plan, &prepared.source)?;
    audio_inspection::bind_source_plan(&plan, &reviewed.artifact)?;
    crate::audio_stem::bind_authorities_plan(&plan, prepared.stem.as_ref())?;
    Ok((plan, prepared.registry, prepared.bindings))
}

/// Resolve inspection, optional stem lineage, and proposed lyrics-alignment stages.
pub fn plan(
    request: &AlignmentRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<PipelineV3Plan, AudioAlignmentFailure> {
    Ok(prepare_plan(request, cancellation)?.0)
}

pub fn run<F>(
    request: AlignmentRequest,
    output_directory: Option<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioAlignmentFailure>
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
    request: AlignmentRequest,
    run_directory: impl Into<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioAlignmentFailure>
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
