//! Signal measurements composed with technical inspection through Pipeline v3.
//!
//! The first stage independently inspects and decodes a source snapshot. The
//! signal stage consumes that evidence and its normalized analysis, retaining
//! the existing provider, checkpoint, cancellation, and immutable-source rules.

mod astats;
mod ebur128;
mod pcm;
mod provider;
mod types;

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

pub use types::*;

use crate::audio_inspection::{self, AudioInspectionFailure, AudioInspectionRequest};
use crate::{
    CancellationToken, ComponentIdentity, ComponentInventory, Error, ErrorCategory,
    PipelineInputBinding, PipelineV3Configuration, PipelineV3Plan, PipelineV3ResumeRequest,
    PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunRequest, PipelineV3Workspace,
    ProviderInvocationRequest, ProviderManifest, ProviderRegistration, ProviderRegistry,
};

const PROVIDER_MANIFEST: &[u8] = include_bytes!("../../providers/audio-signal/manifest.json");
const PIPELINE: &[u8] = include_bytes!("../../providers/audio-signal/pipeline.yml");

/// Explicit technical-inspection authority and versioned signal settings.
#[derive(Debug, Clone)]
pub struct SignalAnalysisRequest {
    pub inspection: AudioInspectionRequest,
    pub settings: SignalAnalysisConfiguration,
}

impl SignalAnalysisRequest {
    #[must_use]
    pub const fn new(
        inspection: AudioInspectionRequest,
        settings: SignalAnalysisConfiguration,
    ) -> Self {
        Self {
            inspection,
            settings,
        }
    }
}

fn bindings(request: &SignalAnalysisRequest) -> Vec<PipelineInputBinding> {
    vec![PipelineInputBinding::new(
        "source_audio",
        request.inspection.input.clone(),
    )]
}

fn prepare_plan(
    request: &SignalAnalysisRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<(PipelineV3Plan, ProviderRegistry), AudioInspectionFailure> {
    // Settings fail cheaply before tools launch. The existing inspection helper
    // reobserves both exact tool pins and binds the immutable source identity.
    request.settings.validate()?;
    let (mut registry, source) =
        audio_inspection::prepare_registry(&request.inspection, cancellation)?;
    let configuration = AudioSignalProviderConfiguration {
        schema: AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
        settings: request.settings.clone(),
        tools: request.inspection.configuration.clone(),
        source: source.clone(),
    }
    .provider_configuration()?;
    let mut manifest = ProviderManifest::from_json_slice(PROVIDER_MANIFEST)?;
    for requirement in &mut manifest.capabilities[0].requirements.tools {
        let pin = match requirement.id.as_str() {
            "ffmpeg" => &request.inspection.configuration.ffmpeg,
            "ffprobe" => &request.inspection.configuration.ffprobe,
            _ => {
                return Err(Error::new(
                    ErrorCategory::Internal,
                    "signal provider manifest declares an unsupported tool requirement",
                )
                .into());
            }
        };
        requirement.version_requirement = format!("={}", pin.version);
        requirement.sha256 = Some(pin.sha256.clone());
    }
    let tools = [
        ("ffmpeg", &request.inspection.configuration.ffmpeg),
        ("ffprobe", &request.inspection.configuration.ffprobe),
    ]
    .into_iter()
    .map(|(id, pin)| ComponentIdentity {
        id: id.to_owned(),
        version: pin.version.clone(),
        sha256: Some(pin.sha256.clone()),
    })
    .collect();
    registry.register(ProviderRegistration::new(
        "audio-signal-native",
        manifest,
        configuration,
        request.inspection.provider_executable.clone(),
        "aniflow-audio-signal-v2",
        ComponentInventory {
            tools,
            codecs: Vec::new(),
            models: Vec::new(),
        },
    )?)?;
    let pipeline = PipelineV3Configuration::from_yaml_slice(PIPELINE)?;
    let plan = crate::resolve_pipeline_v3(
        &pipeline,
        &bindings(request),
        &registry,
        &audio_inspection::context(),
    )?;
    audio_inspection::bind_source_plan(&plan, &source)?;
    Ok((plan, registry))
}

/// Preflight pinned tools and resolve both stages without creating a workspace.
pub fn plan(
    request: &SignalAnalysisRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<PipelineV3Plan, AudioInspectionFailure> {
    Ok(prepare_plan(request, cancellation)?.0)
}

/// Execute technical inspection and measurements with ordinary v3 checkpoints.
pub fn run<F>(
    request: SignalAnalysisRequest,
    output_directory: Option<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioInspectionFailure>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let (plan, registry) = prepare_plan(&request, cancellation)?;
    let mut execution = PipelineV3RunRequest::new(plan, bindings(&request), registry)
        .with_execution_limits(request.inspection.execution_limits);
    execution.output_directory = output_directory;
    Ok(crate::run_v3_with_progress_and_cancellation(
        execution,
        cancellation,
        on_progress,
    )?)
}

fn require_same_plan(run_directory: &Path, requested: &PipelineV3Plan) -> crate::Result<()> {
    let workspace = PipelineV3Workspace::open_read_only(run_directory)?;
    let path = workspace.plan();
    let metadata = fs::symlink_metadata(&path).map_err(|_| {
        Error::new(
            ErrorCategory::State,
            "saved audio analysis plan is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.len() > 1_048_576 {
        return Err(Error::new(
            ErrorCategory::State,
            "saved audio analysis plan must be a regular file no larger than 1 MiB",
        ));
    }
    let file = fs::File::open(path).map_err(|_| {
        Error::new(
            ErrorCategory::State,
            "saved audio analysis plan cannot be opened",
        )
    })?;
    let mut bytes = Vec::new();
    file.take(1_048_577).read_to_end(&mut bytes).map_err(|_| {
        Error::new(
            ErrorCategory::State,
            "saved audio analysis plan cannot be read",
        )
    })?;
    if bytes.len() > 1_048_576 {
        return Err(Error::new(
            ErrorCategory::State,
            "saved audio analysis plan exceeds 1 MiB",
        ));
    }
    let saved = PipelineV3Plan::from_json_slice(&bytes)
        .map_err(|failure| Error::new(ErrorCategory::State, failure.message))?;
    if saved.plan_sha256 != requested.plan_sha256 {
        return Err(Error::new(
            ErrorCategory::State,
            "requested signal analysis profile, source, tools, or settings differ from the saved run; start a new run",
        ));
    }
    Ok(())
}

/// Reobserve tools and require the exact two-stage plan before compatible reuse.
pub fn resume<F>(
    request: SignalAnalysisRequest,
    run_directory: impl Into<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioInspectionFailure>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let run_directory = run_directory.into();
    let (plan, registry) = prepare_plan(&request, cancellation)?;
    // A registry containing both providers could otherwise resume a technical-
    // only run successfully, without producing the selected signal analysis.
    require_same_plan(&run_directory, &plan)?;
    let execution = PipelineV3ResumeRequest::new(run_directory, bindings(&request), registry)
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
