//! Signal measurements composed with technical inspection through Pipeline v3.
//!
//! The first stage independently inspects and decodes a source snapshot. The
//! signal stage consumes that evidence and its normalized analysis, retaining
//! the existing provider, checkpoint, cancellation, and immutable-source rules.

mod astats;
mod ebur128;
pub(crate) mod loudness;
mod native;
mod pcm;
pub use loudness::NativeLoudnessReceipt;
mod provider;
mod types;

use std::path::PathBuf;

pub use native::*;
pub use types::*;

use crate::audio_inspection::{self, AudioInspectionFailure, AudioInspectionRequest};
use crate::{
    CancellationToken, ComponentIdentity, ComponentInventory, Error, ErrorCategory,
    PipelineInputBinding, PipelineV3Configuration, PipelineV3Plan, PipelineV3ResumeRequest,
    PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunRequest, ProviderInvocationRequest,
    ProviderManifest, ProviderRegistration, ProviderRegistry,
};

const PROVIDER_MANIFEST: &[u8] = include_bytes!("../../providers/audio-signal/manifest.json");
const PIPELINE: &[u8] = include_bytes!("../../providers/audio-signal/pipeline.yml");
const NATIVE_PROVIDER_MANIFEST: &[u8] =
    include_bytes!("../../providers/audio-signal/manifest-native.json");
const NATIVE_PIPELINE: &[u8] = include_bytes!("../../providers/audio-signal/pipeline-native.yml");

/// Explicit technical-inspection authority and versioned signal settings.
#[derive(Debug, Clone)]
pub struct SignalAnalysisRequest {
    pub inspection: AudioInspectionRequest,
    pub settings: SignalAnalysisConfiguration,
    pub native_settings: Option<NativeSignalAnalysisConfiguration>,
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
            native_settings: None,
        }
    }

    /// Select source-format-aware ratios without reinterpreting PCM16 settings.
    #[must_use]
    pub fn new_native(
        inspection: AudioInspectionRequest,
        settings: NativeSignalAnalysisConfiguration,
    ) -> Self {
        Self {
            inspection,
            settings: SignalAnalysisConfiguration::default(),
            native_settings: Some(settings),
        }
    }
}

fn prepare_plan(
    request: &SignalAnalysisRequest,
    cancellation: &CancellationToken,
    preflight_tools: bool,
) -> std::result::Result<
    (PipelineV3Plan, ProviderRegistry, Vec<PipelineInputBinding>),
    AudioInspectionFailure,
> {
    // Settings fail cheaply before tools launch. The existing inspection helper
    // reobserves both exact tool pins and binds the immutable source identity.
    request.settings.validate()?;
    if let Some(settings) = &request.native_settings {
        settings.validate()?;
        if request.settings != SignalAnalysisConfiguration::default() {
            return Err(Error::new(
                ErrorCategory::Configuration,
                "native signal requests must not override legacy PCM16 thresholds",
            )
            .into());
        }
    }
    let mut prepared = if preflight_tools {
        audio_inspection::prepare_registry(&request.inspection, cancellation)?
    } else {
        audio_inspection::prepare_registry_read_only(&request.inspection, cancellation)?
    };
    let native = request.native_settings.is_some();
    let configuration = if let Some(settings) = &request.native_settings {
        NativeAudioSignalProviderConfiguration {
            schema: AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V2.to_owned(),
            settings: settings.clone(),
            tools: request.inspection.configuration.clone(),
            source: prepared.source.clone(),
        }
        .provider_configuration()?
    } else {
        AudioSignalProviderConfiguration {
            schema: AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
            settings: request.settings.clone(),
            tools: request.inspection.configuration.clone(),
            source: prepared.source.clone(),
        }
        .provider_configuration()?
    };
    let mut manifest = ProviderManifest::from_json_slice(if native {
        NATIVE_PROVIDER_MANIFEST
    } else {
        PROVIDER_MANIFEST
    })?;
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
    prepared.registry.register(ProviderRegistration::new(
        "audio-signal-native",
        manifest,
        configuration,
        request.inspection.provider_executable.clone(),
        if native {
            "aniflow-audio-signal-v3"
        } else {
            "aniflow-audio-signal-v2"
        },
        ComponentInventory {
            tools,
            codecs: Vec::new(),
            models: Vec::new(),
        },
    )?)?;
    let pipeline =
        PipelineV3Configuration::from_yaml_slice(if native { NATIVE_PIPELINE } else { PIPELINE })?;
    audio_inspection::finish_plan(&request.inspection, pipeline, prepared)
}

/// Preflight pinned tools and resolve both stages without creating a workspace.
pub fn plan(
    request: &SignalAnalysisRequest,
    cancellation: &CancellationToken,
) -> std::result::Result<PipelineV3Plan, AudioInspectionFailure> {
    Ok(prepare_plan(request, cancellation, true)?.0)
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
    let (plan, registry, input_bindings) = prepare_plan(&request, cancellation, true)?;
    let mut execution = PipelineV3RunRequest::new(plan, input_bindings, registry)
        .with_execution_limits(request.inspection.execution_limits);
    execution.output_directory = output_directory;
    Ok(crate::run_v3_with_progress_and_cancellation(
        execution,
        cancellation,
        on_progress,
    )?)
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
    let (plan, registry, input_bindings) = prepare_plan(&request, cancellation, true)?;
    // A registry containing both providers could otherwise resume a technical-
    // only run successfully, without producing the selected signal analysis.
    audio_inspection::require_same_plan(&run_directory, &plan)?;
    let execution = PipelineV3ResumeRequest::new(run_directory, input_bindings, registry)
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

/// Identity-only exact reconstruction for final-QC status; never probes tools.
pub(crate) fn plan_read_only(
    request: &SignalAnalysisRequest,
    token: &CancellationToken,
) -> crate::Result<PipelineV3Plan> {
    if request.inspection.stem_selection.is_some() || request.native_settings.is_none() {
        return Err(Error::new(
            ErrorCategory::Configuration,
            "final-QC readback requires whole native PCM",
        ));
    }
    Ok(prepare_plan(request, token, false).map_err(|e| e.error)?.0)
}

/// Fixed child slot; the ordinary v3 runtime owns all native acceptance.
pub(crate) fn run_at<F: FnMut(&PipelineV3RunProgress)>(
    request: SignalAnalysisRequest,
    root: &std::path::Path,
    expected: &PipelineV3Plan,
    token: &CancellationToken,
    progress: F,
) -> crate::Result<PipelineV3RunOutcome> {
    let parent = crate::run_v3::exact_run_parent(root)?;
    let (plan, registry, bindings) = prepare_plan(&request, token, true).map_err(|e| e.error)?;
    if plan != *expected {
        return Err(Error::new(
            ErrorCategory::State,
            "final-QC native signal plan changed before execution",
        ));
    }
    crate::run_v3::run_v3_at_with_progress_and_cancellation(
        PipelineV3RunRequest::new(plan, bindings, registry)
            .with_output_directory(parent)
            .with_execution_limits(request.inspection.execution_limits),
        root,
        token,
        progress,
    )
}
