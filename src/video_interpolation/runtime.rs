//! One fixed v3 facade over the existing serial RIFE provider. Ordinary native
//! checkpoints own execution/resume; there is no per-pair journal or adoption.
use super::*;
use crate::frame_io::{ensure_canonical_directory, identity, inspect_png, inventory};
use crate::{
    CancellationToken, ErrorCategory, PipelineV3Plan, PipelineV3RunOutcome, PipelineV3RunProgress,
};
use std::{io::Read, path::PathBuf, time::Duration};
const PLAN_BYTES: u64 = 67_108_864;
#[derive(Debug, Clone)]
pub struct RifeRunRequest {
    pub frames_directory: PathBuf,
    pub frame_index: PathBuf,
    pub configuration: RifeConfiguration,
    pub provider_executable: PathBuf,
    pub execution_limits: crate::ProviderExecutionLimits,
}
impl RifeRunRequest {
    #[must_use]
    pub fn new(
        frames: impl Into<PathBuf>,
        index: impl Into<PathBuf>,
        configuration: RifeConfiguration,
        provider: impl Into<PathBuf>,
    ) -> Self {
        let maximum_artifact_bytes = configuration.settings.maximum_artifact_bytes;
        Self {
            frames_directory: frames.into(),
            frame_index: index.into(),
            configuration,
            provider_executable: provider.into(),
            execution_limits: crate::ProviderExecutionLimits {
                timeout: Duration::from_secs(21_600),
                termination_grace_period: Duration::from_secs(2),
                maximum_stdout_bytes: 1_048_576,
                maximum_stderr_bytes: 1_048_576,
                maximum_artifact_files: RIFE_MAXIMUM_FRAMES + 2,
                maximum_artifact_bytes,
            },
        }
    }
}
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() {
        return Err(Error::new(
            ErrorCategory::Execution,
            "RIFE facade cancelled",
        ));
    }
    Ok(())
}
fn state(message: &str) -> Error {
    Error::new(ErrorCategory::State, message)
}
fn mapped(failure: crate::PipelinePlanningFailure) -> Error {
    Error::new(failure.category(), failure.message)
}
struct Prepared {
    plan: PipelineV3Plan,
    registry: crate::ProviderRegistry,
    bindings: Vec<crate::PipelineInputBinding>,
}
pub(crate) struct RifeComponent {
    pub id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub byte_count: u64,
}
pub(crate) fn observe_components(
    configuration: &RifeConfiguration,
    token: &CancellationToken,
) -> Result<Vec<RifeComponent>> {
    cancelled(token)?;
    configuration.validate()?;
    crate::audio_inspection::process::verify_pin(&configuration.settings.tool, "rife-ncnn-vulkan")
        .map_err(|_| invalid("RIFE tool is missing or differs from its explicit pin"))?;
    let tool = identity(
        &configuration.settings.tool.executable,
        super::provider::MAXIMUM_TOOL_BYTES,
    )?;
    if tool.0 != configuration.settings.tool.sha256 {
        return Err(invalid("RIFE tool changed during component observation"));
    }
    let models = super::provider::model_identities(
        &configuration.settings,
        &configuration.settings.model.directory,
    )?;
    let mut values = vec![RifeComponent {
        id: "rife-tool".into(),
        path: configuration.settings.tool.executable.clone(),
        sha256: tool.0,
        byte_count: tool.1,
    }];
    for (id, (path, observed, _)) in ["rife-param", "rife-weights"].into_iter().zip(models) {
        cancelled(token)?;
        values.push(RifeComponent {
            id: id.into(),
            path,
            sha256: observed.0,
            byte_count: observed.1,
        });
    }
    if let Some(runtime) = &configuration.settings.managed_runtime {
        runtime.observed_bytes()?;
        for (i, file) in runtime.runtime_files.iter().enumerate() {
            cancelled(token)?;
            values.push(RifeComponent {
                id: format!("rife-runtime-{i}"),
                path: file.path.clone(),
                sha256: file.sha256.clone(),
                byte_count: file.byte_count,
            });
        }
    }
    Ok(values)
}
pub(super) fn component_inventory(configuration: &RifeConfiguration) -> crate::ComponentInventory {
    crate::ComponentInventory {
        tools: {
            let mut tools = vec![crate::ComponentIdentity {
                id: "rife-ncnn-vulkan".into(),
                version: configuration.settings.tool.version.clone(),
                sha256: Some(configuration.settings.tool.sha256.clone()),
            }];
            if let Some(runtime) = &configuration.settings.managed_runtime {
                tools.push(crate::ComponentIdentity {
                    id: "rife-managed-package".into(),
                    version: "1.0.0".into(),
                    sha256: Some(runtime.package_sha256.clone()),
                });
            }
            tools
        },
        codecs: Vec::new(),
        models: vec![
            crate::ComponentIdentity {
                id: "rife-v4.6-param".into(),
                version: "4.6.0".into(),
                sha256: Some(configuration.settings.model.param_sha256.clone()),
            },
            crate::ComponentIdentity {
                id: "rife-v4.6-bin".into(),
                version: "4.6.0".into(),
                sha256: Some(configuration.settings.model.bin_sha256.clone()),
            },
        ],
    }
}
fn prepare(request: &RifeRunRequest, token: &CancellationToken) -> Result<Prepared> {
    cancelled(token)?;
    let configuration = &request.configuration;
    let wrapper = RifeProviderConfiguration {
        schema: RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),
        configuration: configuration.clone(),
    };
    wrapper.validate()?;
    let limits = &request.execution_limits;
    if limits.timeout.is_zero()
        || limits.timeout > Duration::from_secs(21_600)
        || limits.termination_grace_period > Duration::from_secs(10)
        || !(1..=1_048_576).contains(&limits.maximum_stdout_bytes)
        || !(1..=1_048_576).contains(&limits.maximum_stderr_bytes)
        || !(1..=RIFE_MAXIMUM_FRAMES + 2).contains(&limits.maximum_artifact_files)
        || limits.maximum_artifact_bytes == 0
        || limits.maximum_artifact_bytes > configuration.settings.maximum_artifact_bytes
    {
        return Err(invalid(
            "RIFE facade execution limits exceed its finite serial profile",
        ));
    }
    ensure_canonical_directory(&request.frames_directory)?;
    let (index, index_identity) = super::provider::read_index(&request.frame_index)?;
    let interpolation = super::plan(configuration, &index)?;
    if interpolation.output_frame_count > RIFE_MAXIMUM_FRAMES {
        return Err(invalid(
            "RIFE facade supports at most 50000 original / 100000 derived frames",
        ));
    }
    let output_frames = interpolation.output_frame_count;
    let template = RifeOutputIndex {
        schema: RIFE_OUTPUT_INDEX_SCHEMA_V1.into(),
        frames: super::provider::output_frame_template(&interpolation)?,
        plan: interpolation,
    };
    let output_index_bytes = crate::provider::canonical_json_bytes(&template)?.len() as u64;
    if output_index_bytes > RIFE_MAXIMUM_INDEX_BYTES {
        return Err(invalid(
            "RIFE output index exceeds the native evidence bound",
        ));
    }
    drop(template);
    let names = index
        .frames
        .iter()
        .map(|f| std::ffi::OsString::from(&f.filename))
        .collect::<Vec<_>>();
    inventory(&request.frames_directory, &names)?;
    let declaration = crate::AuthoredPipelineInput {
        id: "source_frames".into(),
        artifact_type: "application/vnd.aniflow.rife-frames".into(),
        artifact_role: crate::ArtifactRole::Intermediate,
        stream_role: Some(crate::StreamRole::Video),
    };
    let directory =
        crate::pipeline_v3::hash_pipeline_input(&declaration, &request.frames_directory)
            .map_err(mapped)?;
    let maximum = super::provider::frame_byte_bound(&index, &configuration.settings)?;
    let mut total = 0u64;
    for frame in &index.frames {
        cancelled(token)?;
        let observed = inspect_png(
            &request.frames_directory.join(&frame.filename),
            index.width,
            index.height,
            maximum,
        )?;
        if observed.0 != frame.sha256 {
            return Err(invalid(
                "RIFE frame bytes differ from the original immutable index",
            ));
        }
        total = total
            .checked_add(observed.1)
            .filter(|n| *n <= configuration.settings.maximum_artifact_bytes)
            .ok_or_else(|| invalid("RIFE complete input frame bytes exceed admission"))?;
    }
    let components = observe_components(configuration, token)?;
    super::provider::admit_storage(
        &configuration.settings,
        &index,
        total,
        components[0].byte_count,
        index_identity.1,
        output_index_bytes,
        output_frames,
    )?;
    let inventory = component_inventory(configuration);
    let mut registry = crate::ProviderRegistry::new();
    registry.register(crate::ProviderRegistration::new(
        "rife-native",
        wrapper.manifest()?,
        wrapper.provider_configuration()?,
        request.provider_executable.clone(),
        "aniflow-rife-v1",
        inventory,
    )?)?;
    let bindings = vec![
        crate::PipelineInputBinding::new("source_frames", &request.frames_directory),
        crate::PipelineInputBinding::new("source_index", &request.frame_index),
    ];
    let pipeline = crate::PipelineV3Configuration::from_yaml_slice(include_bytes!(
        "../../providers/rife/pipeline.yml"
    ))
    .map_err(mapped)?;
    let mut context = crate::audio_inspection::context();
    context.host.memory_mib = configuration.host.available_memory_mib;
    context.host.gpu_available = configuration.settings.device_id >= 0;
    context.allowed_side_effects.push(crate::SideEffect::Ai);
    if context.host.gpu_available {
        context.allowed_side_effects.push(crate::SideEffect::Gpu);
    }
    let plan =
        crate::resolve_pipeline_v3(&pipeline, &bindings, &registry, &context).map_err(mapped)?;
    if !plan.payload.inputs.iter().any(|i| {
        i.id == "source_index"
            && i.content_sha256 == index_identity.0
            && i.byte_count == index_identity.1
    }) || !plan.payload.inputs.iter().any(|i| {
        i.id == "source_frames"
            && i.content_sha256 == directory.content_sha256
            && i.byte_count == directory.byte_count
            && i.file_count == directory.file_count
    }) || identity(&request.frame_index, RIFE_MAXIMUM_INDEX_BYTES)? != index_identity
    {
        return Err(state("RIFE inputs changed during facade admission"));
    }
    cancelled(token)?;
    Ok(Prepared {
        plan,
        registry,
        bindings,
    })
}
/// Hash/inspect declared local inputs without launching RIFE or creating output.
/// RAM/device values are explicit caller declarations, not measured host truth.
pub fn plan(request: &RifeRunRequest, token: &CancellationToken) -> Result<PipelineV3Plan> {
    Ok(prepare(request, token)?.plan)
}
fn parent(request: &RifeRunRequest, selected: Option<PathBuf>) -> Result<PathBuf> {
    let value = selected.unwrap_or_else(|| PathBuf::from(".aniflow/runs"));
    let parent = if value.is_absolute() {
        value
    } else {
        std::env::current_dir()
            .map_err(|_| invalid("RIFE output cwd unavailable"))?
            .join(value)
    };
    if parent
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid("RIFE output parent must be normalized"));
    }
    if let Some(runtime) = &request.configuration.settings.managed_runtime {
        runtime.require_output_disjoint(&parent)?;
    }
    for input in [
        &request.frames_directory,
        &request.frame_index,
        &request.provider_executable,
        &request.configuration.settings.tool.executable,
        &request.configuration.settings.model.directory,
    ] {
        if parent.starts_with(input) {
            return Err(invalid(
                "RIFE output parent overlaps original inputs or components",
            ));
        }
    }
    let mut existing = parent.as_path();
    while !existing
        .try_exists()
        .map_err(|_| invalid("RIFE output ancestor unavailable"))?
    {
        existing = existing
            .parent()
            .ok_or_else(|| invalid("RIFE output parent missing"))?;
    }
    ensure_canonical_directory(existing)?;
    Ok(parent)
}
pub fn run<F: FnMut(&PipelineV3RunProgress)>(
    request: RifeRunRequest,
    output: Option<PathBuf>,
    token: &CancellationToken,
    progress: F,
) -> Result<PipelineV3RunOutcome> {
    run_new(request, output, None, None, token, progress)
}
pub(crate) fn run_at<F: FnMut(&PipelineV3RunProgress)>(
    request: RifeRunRequest,
    directory: &Path,
    expected: &PipelineV3Plan,
    token: &CancellationToken,
    progress: F,
) -> Result<PipelineV3RunOutcome> {
    run_new(
        request,
        Some(crate::run_v3::exact_run_parent(directory)?),
        Some(directory),
        Some(expected),
        token,
        progress,
    )
}
fn run_new<F: FnMut(&PipelineV3RunProgress)>(
    request: RifeRunRequest,
    output: Option<PathBuf>,
    exact: Option<&Path>,
    expected: Option<&PipelineV3Plan>,
    token: &CancellationToken,
    mut progress: F,
) -> Result<PipelineV3RunOutcome> {
    let parent = parent(&request, output)?;
    let prepared = prepare(&request, token)?;
    if expected.is_some_and(|p| *p != prepared.plan) {
        return Err(state(
            "RIFE plan changed after the exact recipe handoff; no child initialized",
        ));
    }
    let mut execution =
        crate::PipelineV3RunRequest::new(prepared.plan, prepared.bindings, prepared.registry)
            .with_execution_limits(request.execution_limits);
    execution.output_directory = Some(parent);
    let mut retention_error = None;
    let result = {
        let mut observer = |event: &PipelineV3RunProgress| {
            if request.configuration.settings.managed_runtime.is_some() {
                if let PipelineV3RunProgress::Started { run_directory, .. } = event {
                    if let Err(error) = retain_json(
                        run_directory,
                        "rife-configuration.json",
                        &request.configuration,
                    ) {
                        token.cancel();
                        retention_error = Some(error);
                    }
                }
            }
            progress(event);
        };
        match exact {
            Some(directory) => crate::run_v3::run_v3_at_with_progress_and_cancellation(
                execution,
                directory,
                token,
                &mut observer,
            ),
            None => crate::run_v3_with_progress_and_cancellation(execution, token, &mut observer),
        }
    };
    if let Some(error) = retention_error {
        return Err(error);
    }
    result
}

pub(super) fn retain_json<T: serde::Serialize>(root: &Path, name: &str, value: &T) -> Result<()> {
    let workspace = crate::PipelineV3Workspace::open_read_only(root)?;
    workspace.publish_json_new(&root.join(name), value)
}

pub(super) fn retained_json<T: serde::de::DeserializeOwned>(root: &Path, name: &str) -> Result<T> {
    ensure_canonical_directory(root)?;
    let path = root.join(name);
    let before = identity(&path, 65_536)?;
    let mut bytes = Vec::new();
    crate::audio_inspection::process::open_regular(&path)
        .map_err(|_| state("exact retained RIFE configuration unavailable; supply the original explicit configuration"))?
        .take(65_537).read_to_end(&mut bytes)
        .map_err(|_| state("cannot read retained RIFE configuration"))?;
    if (format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64) != before
        || identity(&path, 65_536)? != before
    {
        return Err(state(
            "retained RIFE configuration changed during inspection",
        ));
    }
    crate::provider::decode_json(&bytes, "retained exact RIFE configuration")
}

/// Recover exact managed pins without selecting today's defaults or acquiring files.
/// The sidecar is a convenience copy: the immutable plan remains authoritative.
pub fn retained_configuration(root: impl AsRef<Path>) -> Result<RifeConfiguration> {
    let root = root.as_ref();
    let plan = retained(root)?;
    let configuration: RifeConfiguration = retained_json(root, "rife-configuration.json")?;
    let wrapper = RifeProviderConfiguration {
        schema: RIFE_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),
        configuration: configuration.clone(),
    };
    let effective = wrapper.provider_configuration()?;
    let lock = &plan.payload.stages[0].provider_lock.payload;
    if effective.effective_configuration_sha256 != lock.effective_configuration_sha256
        || effective.configuration_schema != lock.configuration_schema
    {
        return Err(state(
            "retained RIFE configuration differs from the immutable provider lock",
        ));
    }
    Ok(configuration)
}
fn retained(directory: &Path) -> Result<PipelineV3Plan> {
    ensure_canonical_directory(directory)?;
    let workspace = crate::PipelineV3Workspace::open_read_only(directory)?;
    let path = workspace.plan();
    let before = identity(&path, PLAN_BYTES)?;
    let mut bytes = Vec::new();
    crate::audio_inspection::process::open_regular(&path)
        .map_err(|_| state("RIFE retained plan unavailable"))?
        .take(PLAN_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| state("cannot read RIFE retained plan"))?;
    if (format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64) != before
        || identity(&path, PLAN_BYTES)? != before
    {
        return Err(state("RIFE retained plan changed"));
    }
    let plan = PipelineV3Plan::from_json_slice(&bytes).map_err(mapped)?;
    if plan.payload.pipeline_name != "serial-rife-interpolation"
        || plan.payload.stages.len() != 1
        || plan.payload.outputs.len() != 2
        || plan.payload.stages[0].id != "interpolate"
        || plan.payload.stages[0].capability.id != RIFE_CAPABILITY_ID
        || plan.payload.stages[0].provider_lock.payload.provider.id != RIFE_PROVIDER_ID
    {
        return Err(state("not a fixed RIFE facade run"));
    }
    Ok(plan)
}
pub fn resume<F: FnMut(&PipelineV3RunProgress)>(
    request: RifeRunRequest,
    directory: impl AsRef<Path>,
    token: &CancellationToken,
    progress: F,
) -> Result<PipelineV3RunOutcome> {
    let root = directory.as_ref();
    if let Some(runtime) = &request.configuration.settings.managed_runtime {
        runtime.require_output_disjoint(root)?;
    }
    let prepared = prepare(&request, token)?;
    if retained(root)? != prepared.plan {
        return Err(state(
            "RIFE original frames/index, tool/model/provider/configuration or bounds changed; no stale checkpoint reuse",
        ));
    }
    let execution = crate::PipelineV3ResumeRequest::new(root, prepared.bindings, prepared.registry)
        .with_execution_limits(request.execution_limits);
    crate::resume_v3_with_progress_and_cancellation(execution, token, progress)
}
pub fn status(directory: impl AsRef<Path>) -> Result<crate::PipelineV3RunManifest> {
    let root = directory.as_ref();
    let plan = retained(root)?;
    let manifest = crate::status_v3(root)?;
    if manifest.payload.plan_sha256 != plan.plan_sha256 || retained(root)? != plan {
        return Err(state("RIFE status plan binding changed"));
    }
    Ok(manifest)
}

#[cfg(all(test, unix))]
#[path = "runtime_tests.rs"]
mod tests;
