//! Bounded offline audio inspection through the existing Pipeline v3 runtime.
//!
//! Applications explicitly identify the native provider executable. Planning,
//! execution, and resume reobserve pinned tools before selecting that provider;
//! no prepared bundle, alternative checkpoint engine, or media rewriting occurs.

pub(crate) mod process;
mod provider;
mod types;
pub(crate) mod wav;

use std::fmt;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest as _, Sha256};

pub use types::*;

use crate::{
    CancellationToken, ComponentIdentity, ComponentInventory, Error, ErrorCategory, HostResources,
    PipelineInputBinding, PipelinePlanningContext, PipelinePlanningFailure,
    PipelineV3Configuration, PipelineV3Plan, PipelineV3ResumeRequest, PipelineV3RunOutcome,
    PipelineV3RunProgress, PipelineV3RunRequest, PipelineV3Workspace, ProviderExecutionLimits,
    ProviderInvocationRequest, ProviderManifest, ProviderRegistration, ProviderRegistry,
    SideEffect,
};

const PROVIDER_MANIFEST: &[u8] = include_bytes!("../../providers/audio-inspection/manifest.json");
const PIPELINE: &[u8] = include_bytes!("../../providers/audio-inspection/pipeline.yml");

/// Explicit source, configured tools, and native provider authority.
#[derive(Debug, Clone)]
pub struct AudioInspectionRequest {
    pub input: PathBuf,
    pub configuration: AudioInspectionConfiguration,
    pub provider_executable: PathBuf,
    pub execution_limits: ProviderExecutionLimits,
    /// Optional verified whole-stem selection; input then identifies the original mix.
    pub stem_selection: Option<crate::audio_stem::StemSelection>,
}

impl AudioInspectionRequest {
    #[must_use]
    pub fn new(
        input: impl Into<PathBuf>,
        configuration: AudioInspectionConfiguration,
        provider_executable: impl Into<PathBuf>,
    ) -> Self {
        Self {
            input: input.into(),
            configuration,
            provider_executable: provider_executable.into(),
            execution_limits: default_execution_limits(),
            stem_selection: None,
        }
    }

    #[must_use]
    pub fn with_stem_selection(mut self, selection: crate::audio_stem::StemSelection) -> Self {
        self.stem_selection = Some(selection);
        self
    }

    #[must_use]
    pub const fn with_execution_limits(mut self, limits: ProviderExecutionLimits) -> Self {
        self.execution_limits = limits;
        self
    }
}

/// Conservative outer bounds; configured child tool bounds remain independent.
#[must_use]
pub const fn default_execution_limits() -> ProviderExecutionLimits {
    ProviderExecutionLimits {
        timeout: Duration::from_secs(600),
        termination_grace_period: Duration::from_millis(2_000),
        maximum_stdout_bytes: 1_048_576,
        maximum_stderr_bytes: 1_048_576,
        maximum_artifact_files: 8,
        maximum_artifact_bytes: 288_358_400,
    }
}

/// Typed failure retaining dependency or planning evidence for machine clients.
#[derive(Debug)]
pub struct AudioInspectionFailure {
    pub error: Error,
    pub preflight: Option<Box<AudioInspectionPreflight>>,
    pub planning: Option<Box<PipelinePlanningFailure>>,
}

impl fmt::Display for AudioInspectionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for AudioInspectionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<Error> for AudioInspectionFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            preflight: None,
            planning: None,
        }
    }
}

impl From<PipelinePlanningFailure> for AudioInspectionFailure {
    fn from(planning: PipelinePlanningFailure) -> Self {
        Self {
            error: Error::new(planning.category(), planning.message.clone()),
            preflight: None,
            planning: Some(Box::new(planning)),
        }
    }
}

/// Bounded, cancellable tool inspection without source or workspace mutation.
pub fn preflight(
    configuration: &AudioInspectionConfiguration,
    cancellation: &CancellationToken,
) -> crate::Result<AudioInspectionPreflight> {
    provider::preflight(configuration, cancellation)
}

fn source_identity(
    input: &Path,
    cancellation: &CancellationToken,
) -> crate::Result<crate::audio_analysis::AudioArtifactReference> {
    let metadata = fs::symlink_metadata(input).map_err(|_| {
        Error::new(
            ErrorCategory::Input,
            "audio inspection source is unavailable",
        )
    })?;
    if !metadata.is_file() || !(44..=AUDIO_INSPECTION_MAXIMUM_BYTES).contains(&metadata.len()) {
        return Err(Error::new(
            ErrorCategory::Input,
            "audio inspection requires a regular source file between 44 bytes and 256 MiB",
        ));
    }
    let mut file = fs::File::open(input).map_err(|_| {
        Error::new(
            ErrorCategory::Input,
            "audio inspection source cannot be opened",
        )
    })?;
    let mut digest = Sha256::new();
    let mut byte_size = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::new(
                ErrorCategory::Execution,
                "audio inspection cancelled while identifying its source",
            ));
        }
        let count = file.read(&mut buffer).map_err(|_| {
            Error::new(
                ErrorCategory::Input,
                "audio inspection source cannot be read",
            )
        })?;
        if count == 0 {
            break;
        }
        byte_size += count as u64;
        if byte_size > AUDIO_INSPECTION_MAXIMUM_BYTES {
            return Err(Error::new(
                ErrorCategory::Input,
                "audio inspection source exceeded its 256 MiB bound",
            ));
        }
        digest.update(&buffer[..count]);
    }
    if byte_size != metadata.len() {
        return Err(Error::new(
            ErrorCategory::Input,
            "audio inspection source changed during identification",
        ));
    }
    Ok(crate::audio_analysis::AudioArtifactReference {
        id: "source_audio".to_owned(),
        sha256: format!("{:x}", digest.finalize()),
        byte_size,
    })
}

pub(crate) struct PreparedInspection {
    pub registry: ProviderRegistry,
    pub source: crate::audio_analysis::AudioArtifactReference,
    pub bindings: Vec<PipelineInputBinding>,
    pub stem: Option<crate::audio_stem::VerifiedStemImport>,
}

pub(crate) fn prepare_registry(
    request: &AudioInspectionRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<PreparedInspection, AudioInspectionFailure> {
    let stem = request
        .stem_selection
        .as_ref()
        .map(|selection| {
            crate::audio_stem::import_selection(&request.input, selection, cancellation)
        })
        .transpose()?;
    let selected_path = stem.as_ref().map_or(request.input.as_path(), |value| {
        value.selected_path.as_path()
    });
    let observed = preflight(&request.configuration, cancellation)?;
    if !observed.is_ready() {
        return Err(AudioInspectionFailure {
            error: Error::new(
                ErrorCategory::Dependency,
                "audio inspection dependency preflight refused execution",
            ),
            preflight: Some(Box::new(observed)),
            planning: None,
        });
    }
    let source = source_identity(selected_path, cancellation)?;
    let configuration = AudioInspectionProviderConfiguration {
        schema: "aniflow.audio-inspection.provider-configuration/v1".to_owned(),
        settings: request.configuration.clone(),
        source: source.clone(),
    }
    .provider_configuration()?;
    let mut manifest = ProviderManifest::from_json_slice(PROVIDER_MANIFEST)?;
    let mut tools = Vec::new();
    for (id, pin) in [
        ("ffmpeg", &request.configuration.ffmpeg),
        ("ffprobe", &request.configuration.ffprobe),
    ] {
        let version = pin.version.clone();
        let requirement = manifest.capabilities[0]
            .requirements
            .tools
            .iter_mut()
            .find(|requirement| requirement.id == id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCategory::Internal,
                    "audio provider manifest is missing a tool requirement",
                )
            })?;
        requirement.version_requirement = format!("={version}");
        requirement.sha256 = Some(pin.sha256.clone());
        tools.push(ComponentIdentity {
            id: id.to_owned(),
            version,
            sha256: Some(pin.sha256.clone()),
        });
    }
    let registration = ProviderRegistration::new(
        "audio-inspection-native",
        manifest,
        configuration,
        request.provider_executable.clone(),
        "aniflow-audio-inspection-v1",
        ComponentInventory {
            tools,
            codecs: Vec::new(),
            models: Vec::new(),
        },
    )?;
    let mut registry = ProviderRegistry::new();
    registry.register(registration)?;
    let bindings = vec![PipelineInputBinding::new("source_audio", selected_path)];
    Ok(PreparedInspection {
        registry,
        source,
        bindings,
        stem,
    })
}

pub(crate) fn bind_source_plan(
    plan: &PipelineV3Plan,
    source: &crate::audio_analysis::AudioArtifactReference,
) -> crate::Result<()> {
    if !plan.payload.inputs.iter().any(|input| {
        input.id == source.id
            && input.content_sha256 == source.sha256
            && input.byte_count == source.byte_size
    }) {
        return Err(Error::new(
            ErrorCategory::Input,
            "source changed between audio configuration and pipeline planning",
        ));
    }
    Ok(())
}

pub(crate) fn context() -> PipelinePlanningContext {
    PipelinePlanningContext {
        // The provider requires one CPU and no measured memory/storage minimum;
        // zero records an unclaimed capacity, rather than inventing host facts.
        host: HostResources {
            cpu_threads: 1,
            memory_mib: 0,
            storage_mib: 0,
            gpu_available: false,
            network_available: false,
        },
        allowed_side_effects: vec![
            SideEffect::FilesystemRead,
            SideEffect::FilesystemWrite,
            SideEffect::EnvironmentRead,
            SideEffect::Subprocess,
        ],
        offline: true,
    }
}

pub(crate) fn attach_selected_stem(
    request: &AudioInspectionRequest,
    pipeline: &mut PipelineV3Configuration,
    prepared: &mut PreparedInspection,
) -> crate::Result<()> {
    if let Some(stem) = &prepared.stem {
        crate::audio_stem::augment_pipeline(
            pipeline,
            &mut prepared.registry,
            &mut prepared.bindings,
            &request.input,
            &request.provider_executable,
            stem,
        )?;
    }
    Ok(())
}

pub(crate) fn resolve_prepared(
    pipeline: PipelineV3Configuration,
    prepared: PreparedInspection,
) -> std::result::Result<
    (PipelineV3Plan, ProviderRegistry, Vec<PipelineInputBinding>),
    AudioInspectionFailure,
> {
    let plan = crate::resolve_pipeline_v3(
        &pipeline,
        &prepared.bindings,
        &prepared.registry,
        &context(),
    )?;
    bind_source_plan(&plan, &prepared.source)?;
    crate::audio_stem::bind_authorities_plan(&plan, prepared.stem.as_ref())?;
    Ok((plan, prepared.registry, prepared.bindings))
}
pub(crate) fn finish_plan(
    request: &AudioInspectionRequest,
    mut pipeline: PipelineV3Configuration,
    mut prepared: PreparedInspection,
) -> std::result::Result<
    (PipelineV3Plan, ProviderRegistry, Vec<PipelineInputBinding>),
    AudioInspectionFailure,
> {
    attach_selected_stem(request, &mut pipeline, &mut prepared)?;
    resolve_prepared(pipeline, prepared)
}

fn prepare_plan(
    request: &AudioInspectionRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<
    (PipelineV3Plan, ProviderRegistry, Vec<PipelineInputBinding>),
    AudioInspectionFailure,
> {
    let prepared = prepare_registry(request, cancellation)?;
    let pipeline = PipelineV3Configuration::from_yaml_slice(PIPELINE)?;
    finish_plan(request, pipeline, prepared)
}

/// Recheck tool identity and resolve the exact pipeline without creating outputs.
pub fn plan(
    request: &AudioInspectionRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<PipelineV3Plan, AudioInspectionFailure> {
    Ok(prepare_plan(request, cancellation)?.0)
}

/// Execute the native provider through Pipeline v3's existing bounded runtime.
pub fn run<F>(
    request: AudioInspectionRequest,
    output_directory: Option<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioInspectionFailure>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let (plan, registry, input_bindings) = prepare_plan(&request, cancellation)?;
    let mut execution = PipelineV3RunRequest::new(plan, input_bindings, registry)
        .with_execution_limits(request.execution_limits);
    execution.output_directory = output_directory;
    Ok(crate::run_v3_with_progress_and_cancellation(
        execution,
        cancellation,
        on_progress,
    )?)
}

pub(crate) fn require_same_plan(
    run_directory: &Path,
    requested: &PipelineV3Plan,
) -> crate::Result<()> {
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
            "requested audio analysis profile, source, tools, settings, or stem relationship differ from the saved run; start a new run",
        ));
    }
    Ok(())
}

/// Reobserve tools before delegating compatible-checkpoint reuse to Pipeline v3.
pub fn resume<F>(
    request: AudioInspectionRequest,
    run_directory: impl Into<PathBuf>,
    cancellation: &CancellationToken,
    on_progress: F,
) -> std::result::Result<PipelineV3RunOutcome, AudioInspectionFailure>
where
    F: FnMut(&PipelineV3RunProgress),
{
    let run_directory = run_directory.into();
    let (plan, registry, input_bindings) = prepare_plan(&request, cancellation)?;
    require_same_plan(&run_directory, &plan)?;
    let execution = PipelineV3ResumeRequest::new(run_directory, input_bindings, registry)
        .with_execution_limits(request.execution_limits);
    Ok(crate::resume_v3_with_progress_and_cancellation(
        execution,
        cancellation,
        on_progress,
    )?)
}

/// Native implementation of the fixed provider ABI; no ordinary CLI parsing.
pub fn execute_provider_invocation(path: impl AsRef<Path>) -> crate::Result<()> {
    let path = path.as_ref();
    if !path.is_absolute() {
        return Err(Error::new(
            ErrorCategory::Configuration,
            "provider invocation path must be absolute",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| Error::new(ErrorCategory::Input, "provider invocation is unavailable"))?;
    if !metadata.is_file() || metadata.len() > 1_048_576 {
        return Err(Error::new(
            ErrorCategory::Configuration,
            "provider invocation must be a regular file no larger than 1 MiB",
        ));
    }
    let file = fs::File::open(path)
        .map_err(|_| Error::new(ErrorCategory::Input, "provider invocation cannot be opened"))?;
    let mut bytes = Vec::new();
    file.take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new(ErrorCategory::Input, "provider invocation cannot be read"))?;
    if bytes.len() > 1_048_576 {
        return Err(Error::new(
            ErrorCategory::Configuration,
            "provider invocation exceeds 1 MiB",
        ));
    }
    let request = ProviderInvocationRequest::from_json_slice(&bytes)?;
    match request.configuration.provider.id.as_str() {
        AUDIO_INSPECTION_PROVIDER_ID => provider::execute_invocation(&request),
        crate::audio_signal::AUDIO_SIGNAL_PROVIDER_ID => {
            crate::audio_signal::execute_provider_invocation(&request)
        }
        crate::audio_stem::AUDIO_STEM_PROVIDER_ID => {
            crate::audio_stem::execute_provider_invocation(&request)
        }
        crate::audio_musical::AUDIO_MUSICAL_PROVIDER_ID => {
            crate::audio_musical::execute_provider_invocation(&request)
        }
        _ => Err(Error::new(
            ErrorCategory::Configuration,
            "native provider invocation selects an unsupported provider identity",
        )),
    }
}
