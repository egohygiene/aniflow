use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::{Builder, NamedTempFile, TempDir};

use crate::pipeline::{
    CommandProcessor, FrameProcessor, Pipeline, ProcessorLimits, sanitize_identifier,
};
use crate::provider::{
    ArtifactCardinality, ArtifactPort, ArtifactRole, BatchingMode, CancellationMode,
    CapabilityBehavior, CapabilityDeclaration, CapabilityKind, CapabilityReference,
    ComponentIdentity, ComponentRequirement, ComputeRequirements, ConfigurationSchemaReference,
    DeterminismClass, FidelityClass, LifecycleSupport, PROVIDER_MANIFEST_SCHEMA_V1, ProgressMode,
    ProvenanceContract, ProvenanceField, ProviderConfiguration, ProviderIdentity, ProviderManifest,
    ProviderReference, ProviderRequirements, RequirementLevel, SideEffect, StreamRole,
    canonical_sha256,
};
use crate::provider_runtime::{
    ArtifactKind, ComponentInventory, ExpectedProviderOutput, HostResources, ProviderCandidate,
    ProviderExecutionLimits, ProviderExecutionOutcome, ProviderExecutionReport,
    ProviderExecutionRequest, ProviderLock, ProviderRegistration, ProviderRegistry,
    ProviderResolutionFailure, ProviderResolutionRequest, ResolvedProvider,
};
use crate::segmentation::CancellationToken;
use crate::workspace::RunWorkspace;

const ADAPTER_VERSION: &str = "1.0.0";
const CONFIGURATION_SCHEMA_ID: &str = "aniflow.pipeline-v2-processor.configuration/v1";
const CONFIGURATION_SCHEMA_BYTES: &[u8] =
    include_bytes!("../docs/contracts/pipeline-v2-processor-configuration-v1.schema.json");

/// Provider-native registrations resolved from the enabled pipeline v2 processors.
#[derive(Debug)]
pub(crate) struct PipelineProviderRegistry {
    providers: BTreeMap<String, PipelineProvider>,
}

impl PipelineProviderRegistry {
    pub(crate) fn resolve(pipeline: &Pipeline) -> Result<Self> {
        let working_directory = std::env::current_dir()
            .context("failed to inspect the processor working directory")?
            .canonicalize()
            .context("failed to resolve the processor working directory")?;
        let mut providers = BTreeMap::new();

        for (index, processor) in pipeline.enabled_frame_processors().enumerate() {
            let stage = format!(
                "frame_{:02}_{}",
                index + 1,
                sanitize_identifier(processor.id())
            );
            insert_provider(
                &mut providers,
                stage,
                AdapterDefinition::from_frame(processor)?,
                &working_directory,
            )?;
        }
        for (index, processor) in pipeline.enabled_audio_processors().enumerate() {
            let stage = format!(
                "audio_{:02}_{}",
                index + 1,
                sanitize_identifier(&processor.id)
            );
            insert_provider(
                &mut providers,
                stage,
                AdapterDefinition::from_command(processor, AdapterKind::ExternalAudio)?,
                &working_directory,
            )?;
        }
        for (index, processor) in pipeline.enabled_video_processors().enumerate() {
            let stage = format!(
                "video_{:02}_{}",
                index + 1,
                sanitize_identifier(&processor.id)
            );
            insert_provider(
                &mut providers,
                stage,
                AdapterDefinition::from_command(processor, AdapterKind::ExternalWholeVideo)?,
                &working_directory,
            )?;
        }

        Ok(Self { providers })
    }

    pub(crate) fn get(&self, stage: &str) -> Result<&PipelineProvider> {
        self.providers
            .get(stage)
            .with_context(|| format!("provider registration is missing for stage `{stage}`"))
    }
}

fn insert_provider(
    providers: &mut BTreeMap<String, PipelineProvider>,
    stage: String,
    definition: AdapterDefinition,
    working_directory: &Path,
) -> Result<()> {
    let provider = PipelineProvider::resolve(&stage, definition, working_directory)?;
    if providers.insert(stage.clone(), provider).is_some() {
        bail!("provider stage `{stage}` was registered more than once");
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct PipelineProvider {
    stage: String,
    resolved: ResolvedProvider,
    output_port: String,
    limits: ProviderExecutionLimits,
    working_directory: PathBuf,
    sensitive_paths: Vec<PathBuf>,
}

impl PipelineProvider {
    fn resolve(
        stage: &str,
        definition: AdapterDefinition,
        working_directory: &Path,
    ) -> Result<Self> {
        let executable = resolve_configured_executable(&definition.command, working_directory);
        let sensitive_paths = definition
            .upscayl_model
            .as_ref()
            .map(|model| vec![model.directory.clone()])
            .unwrap_or_default();
        let configuration_schema = configuration_schema();
        let provider_reference = ProviderReference {
            id: definition.kind.provider_id().to_owned(),
            version: ADAPTER_VERSION.to_owned(),
        };
        let capability_reference = CapabilityReference {
            id: definition.kind.capability_id().to_owned(),
            version: ADAPTER_VERSION.to_owned(),
        };
        let (model_requirements, model_inventory) = definition.model_evidence()?;
        let side_effects = definition.kind.side_effects();
        let manifest = ProviderManifest {
            schema: PROVIDER_MANIFEST_SCHEMA_V1.to_owned(),
            provider: ProviderIdentity {
                id: provider_reference.id.clone(),
                version: provider_reference.version.clone(),
                display_name: definition.kind.display_name().to_owned(),
            },
            configuration_schemas: vec![configuration_schema.clone()],
            capabilities: vec![CapabilityDeclaration {
                id: capability_reference.id.clone(),
                version: capability_reference.version.clone(),
                kind: definition.kind.capability_kind(),
                configuration_schema: configuration_schema.clone(),
                inputs: vec![definition.kind.input_port()],
                outputs: vec![definition.kind.output_port()],
                batching: definition.kind.batching(),
                max_concurrency: definition.maximum_concurrency,
                requirements: ProviderRequirements {
                    tools: Vec::new(),
                    codecs: Vec::new(),
                    models: model_requirements,
                    compute: ComputeRequirements {
                        minimum_cpu_threads: 1,
                        minimum_memory_mib: 0,
                        minimum_storage_mib: 0,
                        gpu: definition.kind.gpu_requirement(),
                        network: definition.kind.network_requirement(),
                    },
                },
                behavior: CapabilityBehavior {
                    determinism: definition.kind.determinism(),
                    fidelity: FidelityClass::Lossy,
                    cacheable: false,
                    content_changes: true,
                    side_effects: side_effects.clone(),
                },
                lifecycle: LifecycleSupport {
                    progress: ProgressMode::LifecycleEvents,
                    cancellation: CancellationMode::ProcessSignal,
                },
            }],
            provenance: complete_provenance_contract(),
        };
        let configuration = ProviderConfiguration::new(
            provider_reference,
            capability_reference,
            configuration_schema,
            definition.values,
        )?;
        let registration_id = format!("pipeline-v2-{stage}");
        let registration = ProviderRegistration::new(
            &registration_id,
            manifest,
            configuration,
            executable,
            definition.kind.implementation_id(),
            ComponentInventory {
                tools: Vec::new(),
                codecs: Vec::new(),
                models: model_inventory,
            },
        )?;
        let mut registry = ProviderRegistry::new();
        registry.register(registration)?;
        let request = ProviderResolutionRequest {
            capability_id: definition.kind.capability_id().to_owned(),
            capability_version_requirement: format!("={ADAPTER_VERSION}"),
            replacement: None,
            primary: ProviderCandidate { registration_id },
            fallbacks: Vec::new(),
            allowed_side_effects: side_effects,
            offline: definition.kind.offline(),
            host: observed_host(definition.kind),
        };
        let resolved = registry
            .resolve(&request)
            .map_err(|failure| resolution_error(stage, &failure))?;

        Ok(Self {
            stage: stage.to_owned(),
            resolved,
            output_port: definition.kind.output_port_name().to_owned(),
            limits: definition.limits.into(),
            working_directory: working_directory.to_path_buf(),
            sensitive_paths,
        })
    }

    pub(crate) fn prepare(&self, workspace: &RunWorkspace) -> Result<()> {
        let lock_path = workspace.provider_lock(&self.stage);
        if lock_path.is_file() {
            let persisted = fs::read(&lock_path)
                .with_context(|| format!("failed to read {}", lock_path.display()))?;
            let persisted = ProviderLock::from_json_slice(&persisted)?;
            if persisted != *self.resolved.provider_lock() {
                bail!(
                    "provider lock for stage `{}` is incompatible with the resolved provider",
                    self.stage
                );
            }
            return Ok(());
        }
        self.resolved.provider_lock().write_new(lock_path)?;
        Ok(())
    }

    pub(crate) fn execute_file<F>(
        &self,
        workspace: &RunWorkspace,
        invocation: &str,
        output_name: &str,
        private_paths: &[&Path],
        cancellation: &CancellationToken,
        arguments: F,
    ) -> Result<ProviderOutput>
    where
        F: FnOnce(&Path) -> Vec<OsString>,
    {
        self.execute(
            workspace,
            invocation,
            PathBuf::from(output_name),
            ArtifactKind::File,
            private_paths,
            cancellation,
            arguments,
        )
    }

    pub(crate) fn execute_directory<F>(
        &self,
        workspace: &RunWorkspace,
        invocation: &str,
        output_name: &str,
        private_paths: &[&Path],
        cancellation: &CancellationToken,
        arguments: F,
    ) -> Result<ProviderOutput>
    where
        F: FnOnce(&Path) -> Vec<OsString>,
    {
        self.execute(
            workspace,
            invocation,
            PathBuf::from(output_name),
            ArtifactKind::Directory,
            private_paths,
            cancellation,
            arguments,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute<F>(
        &self,
        workspace: &RunWorkspace,
        invocation: &str,
        relative_output: PathBuf,
        output_kind: ArtifactKind,
        private_paths: &[&Path],
        cancellation: &CancellationToken,
        arguments: F,
    ) -> Result<ProviderOutput>
    where
        F: FnOnce(&Path) -> Vec<OsString>,
    {
        let work_directory = workspace.provider_work(&self.stage);
        fs::create_dir_all(&work_directory)
            .with_context(|| format!("failed to create {}", work_directory.display()))?;
        let temporary = Builder::new()
            .prefix("invocation-")
            .tempdir_in(&work_directory)
            .with_context(|| {
                format!(
                    "failed to create provider output for stage `{}`",
                    self.stage
                )
            })?;
        let output_directory = temporary.path().canonicalize().with_context(|| {
            format!(
                "failed to resolve provider output for stage `{}`",
                self.stage
            )
        })?;
        let output = output_directory.join(&relative_output);
        let arguments = arguments(&output);
        let sensitive_values = sensitive_values(
            private_paths,
            &self.sensitive_paths,
            &output,
            &workspace.root,
        );
        let request = ProviderExecutionRequest {
            arguments,
            working_directory: self.working_directory.clone(),
            output_directory,
            expected_outputs: vec![ExpectedProviderOutput {
                port: self.output_port.clone(),
                relative_path: relative_output,
                kind: output_kind,
            }],
            limits: self.limits,
            sensitive_values,
        };
        let report = self.resolved.execute(&request, cancellation, |_| {})?;
        let report_path = workspace.provider_report(&self.stage, invocation);
        write_json_atomically(&report_path, &report)?;
        if report.payload.outcome != ProviderExecutionOutcome::Succeeded {
            let detail = report
                .payload
                .failure
                .as_ref()
                .map_or("provider did not return failure detail", |failure| {
                    failure.detail.as_str()
                });
            let code = report
                .payload
                .failure
                .as_ref()
                .map(|failure| format!("{:?}", failure.code))
                .unwrap_or_else(|| "unknown".to_owned());
            bail!(
                "provider invocation `{invocation}` for stage `{}` failed with {code}: {detail}; see {}",
                self.stage,
                report_path.display()
            );
        }

        Ok(ProviderOutput {
            path: output,
            report,
            _temporary: temporary,
        })
    }
}

#[derive(Debug)]
pub(crate) struct ProviderOutput {
    pub(crate) path: PathBuf,
    pub(crate) report: ProviderExecutionReport,
    _temporary: TempDir,
}

#[derive(Debug, Clone, Copy)]
enum AdapterKind {
    ExternalFrame,
    UpscaylNcnn,
    GeminiWatermarkRemover,
    ExternalAudio,
    ExternalWholeVideo,
}

impl AdapterKind {
    const fn provider_id(self) -> &'static str {
        match self {
            Self::ExternalFrame => "org.egohygiene.aniflow.pipeline-v2-external-frame",
            Self::UpscaylNcnn => "org.egohygiene.aniflow.pipeline-v2-upscayl-ncnn",
            Self::GeminiWatermarkRemover => {
                "org.egohygiene.aniflow.pipeline-v2-gemini-watermark-remover"
            }
            Self::ExternalAudio => "org.egohygiene.aniflow.pipeline-v2-external-audio",
            Self::ExternalWholeVideo => "org.egohygiene.aniflow.pipeline-v2-external-whole-video",
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::ExternalFrame => "pipeline v2 external frame adapter",
            Self::UpscaylNcnn => "pipeline v2 upscayl ncnn adapter",
            Self::GeminiWatermarkRemover => "pipeline v2 Gemini Watermark Remover adapter",
            Self::ExternalAudio => "pipeline v2 external audio adapter",
            Self::ExternalWholeVideo => "pipeline v2 external whole-video adapter",
        }
    }

    const fn implementation_id(self) -> &'static str {
        match self {
            Self::ExternalFrame => "pipeline-v2-external-frame-process",
            Self::UpscaylNcnn => "pipeline-v2-upscayl-ncnn-process",
            Self::GeminiWatermarkRemover => "pipeline-v2-gemini-watermark-remover-process",
            Self::ExternalAudio => "pipeline-v2-external-audio-process",
            Self::ExternalWholeVideo => "pipeline-v2-external-whole-video-process",
        }
    }

    const fn capability_id(self) -> &'static str {
        match self {
            Self::ExternalFrame | Self::UpscaylNcnn | Self::GeminiWatermarkRemover => {
                "aniflow/frame.process"
            }
            Self::ExternalAudio => "aniflow/audio.process",
            Self::ExternalWholeVideo => "aniflow/whole-video.process",
        }
    }

    const fn capability_kind(self) -> CapabilityKind {
        match self {
            Self::ExternalFrame | Self::UpscaylNcnn | Self::GeminiWatermarkRemover => {
                CapabilityKind::FrameProcessor
            }
            Self::ExternalAudio => CapabilityKind::AudioProcessor,
            Self::ExternalWholeVideo => CapabilityKind::WholeVideoProcessor,
        }
    }

    const fn batching(self) -> BatchingMode {
        match self {
            Self::ExternalFrame => BatchingMode::PerArtifact,
            Self::UpscaylNcnn | Self::GeminiWatermarkRemover => BatchingMode::DirectoryBatch,
            Self::ExternalAudio | Self::ExternalWholeVideo => BatchingMode::WholeArtifact,
        }
    }

    const fn input_artifact_type(self) -> &'static str {
        match self {
            Self::ExternalFrame => "image/png",
            Self::UpscaylNcnn | Self::GeminiWatermarkRemover => {
                "application/vnd.aniflow.frame-set+directory"
            }
            Self::ExternalAudio => "application/vnd.aniflow.audio-artifact",
            Self::ExternalWholeVideo => "application/vnd.aniflow.video-artifact",
        }
    }

    const fn output_artifact_type(self) -> &'static str {
        self.input_artifact_type()
    }

    const fn input_port_name(self) -> &'static str {
        match self {
            Self::ExternalFrame => "frame",
            Self::UpscaylNcnn | Self::GeminiWatermarkRemover => "frames",
            Self::ExternalAudio => "audio",
            Self::ExternalWholeVideo => "video",
        }
    }

    const fn output_port_name(self) -> &'static str {
        match self {
            Self::ExternalFrame => "processed_frame",
            Self::UpscaylNcnn | Self::GeminiWatermarkRemover => "processed_frames",
            Self::ExternalAudio => "processed_audio",
            Self::ExternalWholeVideo => "processed_video",
        }
    }

    const fn stream_role(self) -> StreamRole {
        match self {
            Self::ExternalAudio => StreamRole::Audio,
            Self::ExternalFrame
            | Self::UpscaylNcnn
            | Self::GeminiWatermarkRemover
            | Self::ExternalWholeVideo => StreamRole::Video,
        }
    }

    fn input_port(self) -> ArtifactPort {
        ArtifactPort {
            name: self.input_port_name().to_owned(),
            artifact_type: self.input_artifact_type().to_owned(),
            artifact_role: ArtifactRole::TemporalComponent,
            stream_role: Some(self.stream_role()),
            cardinality: ArtifactCardinality::One,
            immutable: true,
        }
    }

    fn output_port(self) -> ArtifactPort {
        ArtifactPort {
            name: self.output_port_name().to_owned(),
            artifact_type: self.output_artifact_type().to_owned(),
            artifact_role: ArtifactRole::Intermediate,
            stream_role: Some(self.stream_role()),
            cardinality: ArtifactCardinality::One,
            immutable: true,
        }
    }

    fn side_effects(self) -> Vec<SideEffect> {
        let mut effects = vec![
            SideEffect::FilesystemRead,
            SideEffect::FilesystemWrite,
            SideEffect::EnvironmentRead,
            SideEffect::Subprocess,
        ];
        match self {
            Self::ExternalFrame | Self::ExternalAudio | Self::ExternalWholeVideo => {
                effects.push(SideEffect::Network);
            }
            Self::UpscaylNcnn => effects.push(SideEffect::Gpu),
            Self::GeminiWatermarkRemover => {}
        }
        effects
    }

    const fn gpu_requirement(self) -> RequirementLevel {
        match self {
            Self::UpscaylNcnn => RequirementLevel::Optional,
            _ => RequirementLevel::Forbidden,
        }
    }

    const fn network_requirement(self) -> RequirementLevel {
        match self {
            Self::ExternalFrame | Self::ExternalAudio | Self::ExternalWholeVideo => {
                RequirementLevel::Optional
            }
            Self::UpscaylNcnn | Self::GeminiWatermarkRemover => RequirementLevel::Forbidden,
        }
    }

    const fn offline(self) -> bool {
        matches!(self, Self::UpscaylNcnn | Self::GeminiWatermarkRemover)
    }

    const fn determinism(self) -> DeterminismClass {
        match self {
            Self::UpscaylNcnn | Self::GeminiWatermarkRemover => {
                DeterminismClass::ConfigurationDependent
            }
            Self::ExternalFrame | Self::ExternalAudio | Self::ExternalWholeVideo => {
                DeterminismClass::EnvironmentDependent
            }
        }
    }
}

struct AdapterDefinition {
    kind: AdapterKind,
    command: String,
    limits: ProcessorLimits,
    maximum_concurrency: u32,
    values: BTreeMap<String, Value>,
    upscayl_model: Option<UpscaylModel>,
}

impl AdapterDefinition {
    fn from_frame(processor: &FrameProcessor) -> Result<Self> {
        let mut values = common_configuration(processor.id(), processor.limits())?;
        match processor {
            FrameProcessor::External {
                command,
                arguments,
                concurrency,
                limits,
                ..
            } => {
                values.insert("processor_kind".to_owned(), json!("external_frame"));
                values.insert("arguments".to_owned(), json!(arguments));
                values.insert("concurrency".to_owned(), json!(concurrency));
                Ok(Self {
                    kind: AdapterKind::ExternalFrame,
                    command: command.clone(),
                    limits: *limits,
                    maximum_concurrency: u32::try_from(*concurrency)
                        .context("frame processor concurrency exceeds the provider contract")?,
                    values,
                    upscayl_model: None,
                })
            }
            FrameProcessor::UpscaylNcnn {
                command,
                model,
                model_path,
                scale,
                tile_size,
                gpu_id,
                tta,
                additional_arguments,
                limits,
                ..
            } => {
                values.insert("processor_kind".to_owned(), json!("upscayl_ncnn"));
                values.insert("model".to_owned(), json!(model));
                values.insert("model_path".to_owned(), json!(model_path));
                values.insert("scale".to_owned(), json!(scale));
                values.insert("tile_size".to_owned(), json!(tile_size));
                values.insert("gpu_id".to_owned(), json!(gpu_id));
                values.insert("tta".to_owned(), json!(tta));
                values.insert(
                    "additional_arguments".to_owned(),
                    json!(additional_arguments),
                );
                Ok(Self {
                    kind: AdapterKind::UpscaylNcnn,
                    command: command.clone(),
                    limits: *limits,
                    maximum_concurrency: 1,
                    values,
                    upscayl_model: model_path.as_ref().map(|path| UpscaylModel {
                        name: model.clone(),
                        directory: path.clone(),
                    }),
                })
            }
            FrameProcessor::GeminiWatermarkRemover {
                command,
                json,
                additional_arguments,
                limits,
                ..
            } => {
                values.insert(
                    "processor_kind".to_owned(),
                    json!("gemini_watermark_remover"),
                );
                values.insert("json".to_owned(), json!(json));
                values.insert(
                    "additional_arguments".to_owned(),
                    json!(additional_arguments),
                );
                Ok(Self {
                    kind: AdapterKind::GeminiWatermarkRemover,
                    command: command.clone(),
                    limits: *limits,
                    maximum_concurrency: 1,
                    values,
                    upscayl_model: None,
                })
            }
        }
    }

    fn from_command(processor: &CommandProcessor, kind: AdapterKind) -> Result<Self> {
        let mut values = common_configuration(&processor.id, processor.limits)?;
        values.insert(
            "processor_kind".to_owned(),
            json!(match kind {
                AdapterKind::ExternalAudio => "external_audio",
                AdapterKind::ExternalWholeVideo => "external_whole_video",
                _ => unreachable!("command processors use an audio or whole-video adapter"),
            }),
        );
        values.insert("arguments".to_owned(), json!(processor.arguments));
        values.insert(
            "output_extension".to_owned(),
            json!(processor.output_extension),
        );
        Ok(Self {
            kind,
            command: processor.command.clone(),
            limits: processor.limits,
            maximum_concurrency: 1,
            values,
            upscayl_model: None,
        })
    }

    fn model_evidence(&self) -> Result<(Vec<ComponentRequirement>, Vec<ComponentIdentity>)> {
        let Some(model) = &self.upscayl_model else {
            return Ok((Vec::new(), Vec::new()));
        };
        let identity = model.identity()?;
        let requirement = ComponentRequirement {
            id: "upscayl-model".to_owned(),
            version_requirement: format!("={ADAPTER_VERSION}"),
            sha256: identity.as_ref().and_then(|value| value.sha256.clone()),
        };
        Ok((vec![requirement], identity.into_iter().collect()))
    }
}

struct UpscaylModel {
    name: String,
    directory: PathBuf,
}

impl UpscaylModel {
    fn identity(&self) -> Result<Option<ComponentIdentity>> {
        let mut files = Vec::new();
        for extension in ["bin", "param"] {
            let path = self.directory.join(format!("{}.{extension}", self.name));
            if !path.is_file() {
                return Ok(None);
            }
            files.push(ModelFileIdentity {
                name: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .context("upscayl model filename must be valid UTF-8")?
                    .to_owned(),
                sha256: sha256_file(&path)?,
            });
        }
        files.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Some(ComponentIdentity {
            id: "upscayl-model".to_owned(),
            version: ADAPTER_VERSION.to_owned(),
            sha256: Some(canonical_sha256(&files)?),
        }))
    }
}

#[derive(Serialize)]
struct ModelFileIdentity {
    name: String,
    sha256: String,
}

fn common_configuration(
    processor_id: &str,
    limits: ProcessorLimits,
) -> Result<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    values.insert("processor_id".to_owned(), json!(processor_id));
    values.insert("limits".to_owned(), serde_json::to_value(limits)?);
    Ok(values)
}

fn configuration_schema() -> ConfigurationSchemaReference {
    ConfigurationSchemaReference {
        id: CONFIGURATION_SCHEMA_ID.to_owned(),
        version: ADAPTER_VERSION.to_owned(),
        sha256: format!("{:x}", Sha256::digest(CONFIGURATION_SCHEMA_BYTES)),
    }
}

fn complete_provenance_contract() -> ProvenanceContract {
    ProvenanceContract {
        required: vec![
            ProvenanceField::InputDigests,
            ProvenanceField::OutputDigests,
            ProvenanceField::PipelineConfiguration,
            ProvenanceField::EffectiveConfiguration,
            ProvenanceField::ProviderIdentity,
            ProvenanceField::CapabilityIdentity,
            ProvenanceField::ToolVersions,
            ProvenanceField::CodecVersions,
            ProvenanceField::ModelIdentities,
            ProvenanceField::ValidationEvidence,
        ],
    }
}

fn observed_host(_kind: AdapterKind) -> HostResources {
    let cpu_threads = std::thread::available_parallelism()
        .map(|count| u16::try_from(count.get()).unwrap_or(u16::MAX))
        .unwrap_or(1);
    HostResources {
        cpu_threads,
        memory_mib: 0,
        storage_mib: 0,
        // Pipeline v2 does not probe these resources. Optional requirements do not
        // block selection, so report only evidence that was actually observed.
        gpu_available: false,
        network_available: false,
    }
}

fn resolution_error(stage: &str, failure: &ProviderResolutionFailure) -> anyhow::Error {
    let attempts = failure
        .attempts
        .iter()
        .flat_map(|attempt| {
            attempt.reasons.iter().map(move |reason| {
                let code = serde_json::to_string(&reason.code)
                    .unwrap_or_else(|_| "\"unknown\"".to_owned());
                format!(
                    "{}: {} ({})",
                    attempt.candidate.registration_id,
                    code.trim_matches('"'),
                    reason.detail
                )
            })
        })
        .collect::<Vec<_>>()
        .join("; ");
    anyhow::anyhow!(
        "provider resolution failed for stage `{stage}`: {}; {attempts}",
        failure.message
    )
}

fn resolve_configured_executable(command: &str, working_directory: &Path) -> PathBuf {
    let configured = Path::new(command);
    if configured.is_absolute() {
        return canonical_or_owned(configured);
    }
    if configured.components().count() > 1 {
        return canonical_or_owned(&working_directory.join(configured));
    }

    let mut first_candidate = None;
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            let directory = if directory.is_absolute() {
                directory
            } else {
                working_directory.join(directory)
            };
            for candidate in executable_candidates(&directory, command) {
                first_candidate.get_or_insert_with(|| candidate.clone());
                if is_runnable_file(&candidate) {
                    return canonical_or_owned(&candidate);
                }
            }
        }
    }
    first_candidate.unwrap_or_else(|| working_directory.join(configured))
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn executable_candidates(directory: &Path, command: &str) -> Vec<PathBuf> {
    let base = directory.join(command);
    #[cfg(windows)]
    {
        if Path::new(command).extension().is_some() {
            return vec![base];
        }
        let extensions = std::env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .filter(|extension| !extension.is_empty())
                    .map(|extension| extension.to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![".COM".to_owned(), ".EXE".to_owned()]);
        return std::iter::once(base.clone())
            .chain(extensions.into_iter().map(|extension| {
                let mut value = base.as_os_str().to_owned();
                value.push(extension);
                PathBuf::from(value)
            }))
            .collect();
    }
    #[cfg(not(windows))]
    {
        vec![base]
    }
}

fn is_runnable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn sensitive_values(
    private_paths: &[&Path],
    configured_paths: &[PathBuf],
    output: &Path,
    run_directory: &Path,
) -> Vec<String> {
    let mut values = private_paths
        .iter()
        .copied()
        .chain(configured_paths.iter().map(PathBuf::as_path))
        .chain([output, run_directory])
        .filter_map(|path| path.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    values
}

fn write_json_atomically(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .context("provider evidence path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create provider evidence in {}", parent.display()))?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), value)
        .context("failed to encode provider evidence")?;
    temporary
        .as_file_mut()
        .write_all(b"\n")
        .context("failed to finish provider evidence")?;
    temporary
        .as_file_mut()
        .sync_all()
        .context("failed to sync provider evidence")?;
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to replace {}", path.display()))?;
    }
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to publish {}", path.display()))?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open model component {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read model component {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

impl From<ProcessorLimits> for ProviderExecutionLimits {
    fn from(limits: ProcessorLimits) -> Self {
        Self {
            timeout: Duration::from_secs(limits.timeout_seconds),
            termination_grace_period: Duration::from_millis(limits.termination_grace_milliseconds),
            maximum_stdout_bytes: limits.maximum_stdout_bytes,
            maximum_stderr_bytes: limits.maximum_stderr_bytes,
            maximum_artifact_files: limits.maximum_artifact_files,
            maximum_artifact_bytes: limits.maximum_artifact_bytes,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::thread;

    use tempfile::tempdir;

    use super::*;
    use crate::pipeline::{FrameValidation, Output};

    fn fixture_script(directory: &Path) -> PathBuf {
        let path = directory.join("fixture provider");
        fs::write(
            &path,
            r#"#!/bin/sh
mode="$1"
input="$2"
output="$3"
case "$mode" in
  copy-file)
    cp "$input" "$output"
    ;;
  copy-directory)
    mkdir -p "$output"
    cp "$input"/* "$output"/
    ;;
  missing)
    printf "completed without output\n"
    ;;
  invalid)
    printf "not a png" > "$output"
    ;;
  fail)
    printf "fixture failed\n" >&2
    exit 23
    ;;
  sleep)
    sleep 5
    ;;
  *)
    exit 64
    ;;
esac
"#,
        )
        .expect("fixture provider should be written");
        let mut permissions = fs::metadata(&path)
            .expect("fixture provider metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("fixture provider should be executable");
        path
    }

    fn external_pipeline(executable: &Path, limits: ProcessorLimits) -> Pipeline {
        Pipeline {
            version: 2,
            name: "provider-fixture".to_owned(),
            description: String::new(),
            frame_processors: vec![FrameProcessor::External {
                id: "fixture".to_owned(),
                enabled: true,
                command: executable.to_string_lossy().into_owned(),
                arguments: vec!["{input}".to_owned(), "{output}".to_owned()],
                concurrency: 1,
                limits,
            }],
            validation: FrameValidation::default(),
            audio_processors: Vec::new(),
            subtitles: None,
            video_processors: Vec::new(),
            renderflow: None,
            output: Output::default(),
        }
    }

    fn workspace(root: &Path) -> RunWorkspace {
        let workspace = RunWorkspace {
            root: root.join("run"),
        };
        workspace
            .create_directories()
            .expect("fixture workspace should be created");
        workspace
    }

    fn execute_mode(
        provider: &PipelineProvider,
        workspace: &RunWorkspace,
        input: &Path,
        mode: &str,
        cancellation: &CancellationToken,
    ) -> Result<ProviderOutput> {
        provider.prepare(workspace)?;
        provider.execute_file(
            workspace,
            mode,
            "frame.png",
            &[input],
            cancellation,
            |output| {
                vec![
                    OsString::from(mode),
                    input.as_os_str().to_owned(),
                    output.as_os_str().to_owned(),
                ]
            },
        )
    }

    #[test]
    fn provider_adapter_accepts_only_a_valid_declared_output() {
        let root = tempdir().expect("temporary directory should be created");
        let executable = fixture_script(root.path());
        let pipeline = external_pipeline(&executable, ProcessorLimits::default());
        let providers =
            PipelineProviderRegistry::resolve(&pipeline).expect("fixture provider should resolve");
        let provider = providers
            .get("frame_01_fixture")
            .expect("fixture provider should exist");
        let workspace = workspace(root.path());
        let input = root.path().join("input.png");
        fs::write(&input, b"fixture bytes").expect("fixture input should be written");

        let output = execute_mode(
            provider,
            &workspace,
            &input,
            "copy-file",
            &CancellationToken::default(),
        )
        .expect("valid provider output should be accepted");

        assert_eq!(
            output.report.payload.outcome,
            ProviderExecutionOutcome::Succeeded
        );
        assert_eq!(
            fs::read(&output.path).expect("accepted output should remain readable"),
            b"fixture bytes"
        );
        assert!(workspace.provider_lock("frame_01_fixture").is_file());
        assert!(
            workspace
                .provider_report("frame_01_fixture", "copy-file")
                .is_file()
        );
    }

    #[test]
    fn provider_adapter_retains_nonzero_and_missing_output_reports() {
        let root = tempdir().expect("temporary directory should be created");
        let executable = fixture_script(root.path());
        let providers = PipelineProviderRegistry::resolve(&external_pipeline(
            &executable,
            ProcessorLimits::default(),
        ))
        .expect("fixture provider should resolve");
        let provider = providers
            .get("frame_01_fixture")
            .expect("fixture provider should exist");
        let workspace = workspace(root.path());
        let input = root.path().join("input.png");
        fs::write(&input, b"fixture bytes").expect("fixture input should be written");

        for (mode, outcome) in [
            ("fail", ProviderExecutionOutcome::Failed),
            ("missing", ProviderExecutionOutcome::InvalidOutput),
        ] {
            let error = execute_mode(
                provider,
                &workspace,
                &input,
                mode,
                &CancellationToken::default(),
            )
            .expect_err("invalid provider execution should fail");
            assert!(error.to_string().contains("provider invocation"));
            let report = fs::read(workspace.provider_report("frame_01_fixture", mode))
                .expect("failure report should be retained");
            let report = ProviderExecutionReport::from_json_slice(&report)
                .expect("failure report should validate");
            assert_eq!(report.payload.outcome, outcome);
        }
    }

    #[test]
    fn provider_adapter_enforces_timeout_and_cancellation() {
        let root = tempdir().expect("temporary directory should be created");
        let executable = fixture_script(root.path());
        let limits = ProcessorLimits {
            timeout_seconds: 1,
            ..ProcessorLimits::default()
        };
        let providers = PipelineProviderRegistry::resolve(&external_pipeline(&executable, limits))
            .expect("fixture provider should resolve");
        let provider = providers
            .get("frame_01_fixture")
            .expect("fixture provider should exist");
        let workspace = workspace(root.path());
        let input = root.path().join("input.png");
        fs::write(&input, b"fixture bytes").expect("fixture input should be written");

        execute_mode(
            provider,
            &workspace,
            &input,
            "sleep",
            &CancellationToken::default(),
        )
        .expect_err("sleeping provider should time out");
        let timeout_report = fs::read(workspace.provider_report("frame_01_fixture", "sleep"))
            .expect("timeout report should be retained");
        let timeout_report = ProviderExecutionReport::from_json_slice(&timeout_report)
            .expect("timeout report should validate");
        assert_eq!(
            timeout_report.payload.outcome,
            ProviderExecutionOutcome::TimedOut
        );

        let cancellation = CancellationToken::default();
        let signal = cancellation.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            signal.cancel();
        });
        execute_mode(provider, &workspace, &input, "sleep", &cancellation)
            .expect_err("cancelled provider should fail");
        canceller.join().expect("canceller should finish");
        let cancellation_report = fs::read(workspace.provider_report("frame_01_fixture", "sleep"))
            .expect("cancellation report should be retained");
        let cancellation_report = ProviderExecutionReport::from_json_slice(&cancellation_report)
            .expect("cancellation report should validate");
        assert_eq!(
            cancellation_report.payload.outcome,
            ProviderExecutionOutcome::Cancelled
        );
    }

    #[test]
    fn every_pipeline_v2_processor_family_resolves_to_a_typed_provider() {
        let root = tempdir().expect("temporary directory should be created");
        let executable = fixture_script(root.path());
        let command = executable.to_string_lossy().into_owned();
        let limits = ProcessorLimits::default();
        let pipeline = Pipeline {
            version: 2,
            name: "all-adapters".to_owned(),
            description: String::new(),
            frame_processors: vec![
                FrameProcessor::External {
                    id: "external".to_owned(),
                    enabled: true,
                    command: command.clone(),
                    arguments: vec!["{input}".to_owned(), "{output}".to_owned()],
                    concurrency: 2,
                    limits,
                },
                FrameProcessor::GeminiWatermarkRemover {
                    id: "gemini".to_owned(),
                    enabled: true,
                    command: command.clone(),
                    json: true,
                    additional_arguments: Vec::new(),
                    limits,
                },
                FrameProcessor::UpscaylNcnn {
                    id: "upscayl".to_owned(),
                    enabled: true,
                    command: command.clone(),
                    model: "fixture".to_owned(),
                    model_path: None,
                    scale: 2,
                    tile_size: None,
                    gpu_id: None,
                    tta: false,
                    additional_arguments: Vec::new(),
                    limits,
                },
            ],
            validation: FrameValidation::default(),
            audio_processors: vec![CommandProcessor {
                id: "audio".to_owned(),
                enabled: true,
                command: command.clone(),
                arguments: vec!["{input}".to_owned(), "{output}".to_owned()],
                output_extension: Some("wav".to_owned()),
                limits,
            }],
            subtitles: None,
            video_processors: vec![CommandProcessor {
                id: "video".to_owned(),
                enabled: true,
                command,
                arguments: vec!["{input}".to_owned(), "{output}".to_owned()],
                output_extension: Some("mp4".to_owned()),
                limits,
            }],
            renderflow: None,
            output: Output::default(),
        };

        let providers = PipelineProviderRegistry::resolve(&pipeline)
            .expect("every adapter family should resolve");
        assert_eq!(providers.providers.len(), 5);
        assert_eq!(
            providers
                .get("frame_01_external")
                .unwrap()
                .resolved
                .provider_lock()
                .payload
                .capability
                .id,
            "aniflow/frame.process"
        );
        assert_eq!(
            providers
                .get("audio_01_audio")
                .unwrap()
                .resolved
                .provider_lock()
                .payload
                .capability
                .id,
            "aniflow/audio.process"
        );
        assert_eq!(
            providers
                .get("video_01_video")
                .unwrap()
                .resolved
                .provider_lock()
                .payload
                .capability
                .id,
            "aniflow/whole-video.process"
        );
    }

    #[test]
    fn missing_configured_executable_returns_typed_resolution_evidence() {
        let root = tempdir().expect("temporary directory should be created");
        let missing = root.path().join("missing-provider");
        let error = PipelineProviderRegistry::resolve(&external_pipeline(
            &missing,
            ProcessorLimits::default(),
        ))
        .expect_err("missing executable should prevent provider resolution");

        assert!(error.to_string().contains("executable_unavailable"));
        assert!(error.to_string().contains("frame_01_fixture"));
    }

    #[test]
    fn compatibility_command_name_resolves_once_to_an_absolute_candidate() {
        let working_directory = std::env::current_dir().expect("current directory should exist");
        let executable = resolve_configured_executable("sh", &working_directory);

        assert!(executable.is_absolute());
        assert!(is_runnable_file(&executable));
    }

    #[test]
    fn missing_explicit_upscayl_model_files_return_typed_evidence() {
        let root = tempdir().expect("temporary directory should be created");
        let executable = fixture_script(root.path());
        let processor = FrameProcessor::UpscaylNcnn {
            id: "upscayl".to_owned(),
            enabled: true,
            command: executable.to_string_lossy().into_owned(),
            model: "fixture".to_owned(),
            model_path: Some(root.path().to_path_buf()),
            scale: 2,
            tile_size: None,
            gpu_id: None,
            tta: false,
            additional_arguments: Vec::new(),
            limits: ProcessorLimits::default(),
        };
        let definition =
            AdapterDefinition::from_frame(&processor).expect("upscayl definition should be valid");
        let error = PipelineProvider::resolve(
            "frame_01_upscayl",
            definition,
            &std::env::current_dir().expect("current directory should exist"),
        )
        .expect_err("missing explicit model files should prevent resolution");

        assert!(error.to_string().contains("component_missing"));
        assert!(error.to_string().contains("upscayl-model"));
    }

    #[test]
    fn explicit_upscayl_model_files_are_bound_into_the_provider_lock() {
        let root = tempdir().expect("temporary directory should be created");
        let executable = fixture_script(root.path());
        fs::write(root.path().join("fixture.bin"), b"model bin")
            .expect("model binary should be written");
        fs::write(root.path().join("fixture.param"), b"model parameters")
            .expect("model parameters should be written");
        let processor = FrameProcessor::UpscaylNcnn {
            id: "upscayl".to_owned(),
            enabled: true,
            command: executable.to_string_lossy().into_owned(),
            model: "fixture".to_owned(),
            model_path: Some(root.path().to_path_buf()),
            scale: 2,
            tile_size: None,
            gpu_id: None,
            tta: false,
            additional_arguments: Vec::new(),
            limits: ProcessorLimits::default(),
        };
        let definition =
            AdapterDefinition::from_frame(&processor).expect("upscayl definition should be valid");
        let provider = PipelineProvider::resolve(
            "frame_01_upscayl",
            definition,
            &std::env::current_dir().expect("current directory should exist"),
        )
        .expect("upscayl provider should resolve");

        let models = &provider.resolved.provider_lock().payload.models;
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "upscayl-model");
        assert!(models[0].sha256.is_some());
    }

    #[test]
    fn provider_output_can_outlive_the_execution_scope_until_promotion() {
        let root = tempdir().expect("temporary directory should be created");
        let executable = fixture_script(root.path());
        let providers = PipelineProviderRegistry::resolve(&external_pipeline(
            &executable,
            ProcessorLimits::default(),
        ))
        .expect("fixture provider should resolve");
        let workspace = workspace(root.path());
        let input = root.path().join("input.png");
        fs::write(&input, b"fixture bytes").expect("fixture input should be written");
        let output = execute_mode(
            providers.get("frame_01_fixture").unwrap(),
            &workspace,
            &input,
            "copy-file",
            &CancellationToken::default(),
        )
        .expect("provider should succeed");

        assert!(output.path.is_file());
        drop(output);
    }
}
