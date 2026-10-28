//! Extract one bounded original-clock PNG sequence through Pipeline v3.
//! Planning is read-only. Native media admission and extraction evidence are
//! produced by one ordinary stage, never by a second checkpoint authority.
mod media;
mod provider;
mod types;

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use serde_json::json;
use sha2::{Digest, Sha256};
use crate::audio_inspection::process::open_regular;
use crate::frame_io::identity;
use crate::provider::{canonical_json_bytes, decode_json, require_sha256};
use crate::{CancellationToken, Error, ErrorCategory, Result};
pub use media::FrameExtractionMediaEvidence;
pub use provider::execute_invocation;
pub use types::*;
pub(crate) use crate::video_assembly::require_storage;

const MAXIMUM_PLAN_BYTES: u64 = 67_108_864;
const REGISTRATION_ID: &str = "frame-extraction-native";
const IMPLEMENTATION_ID: &str = "aniflow-frame-extraction-v1";
fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input,message) }
fn cancelled(token: &CancellationToken) -> Result<()> {
    if token.is_cancelled() { return Err(Error::new(ErrorCategory::Execution,"frame extraction admission cancelled")); }
    Ok(())
}

impl FrameExtractionConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("frame extraction configuration exceeds 64 KiB")); }
        let value: Self = decode_json(bytes,"frame extraction configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != FRAME_EXTRACTION_CONFIGURATION_SCHEMA_V1
            || !(1..=21_600_000).contains(&self.timeout_milliseconds)
            || !(65_536..=67_108_864).contains(&self.maximum_capture_bytes)
            || !(1..=1_099_511_627_776).contains(&self.maximum_source_bytes)
            || !(1024..=67_108_864).contains(&self.maximum_frame_bytes)
            || !(1_048_576..=1_099_511_627_776).contains(&self.maximum_total_frame_bytes)
            || !(1..=33_554_432).contains(&self.maximum_pixels)
            || !(1..=100_000).contains(&self.maximum_frames)
            || !(1_048_576..=17_592_186_044_416).contains(&self.maximum_working_bytes)
            || self.minimum_free_bytes > 1_099_511_627_776 {
            return Err(invalid("frame extraction requires finite source, frame, count, pixel, capture and storage bounds"));
        }
        let selection = &self.source_selection;
        let discarded: BTreeSet<_> = selection.discard_streams.iter().copied().collect();
        if selection.video_stream.is_none() || (!selection.no_audio && selection.audio_stream.is_none())
            || (selection.no_audio && selection.audio_stream.is_some())
            || selection.video_stream.is_some_and(|video| Some(video) == selection.audio_stream || discarded.contains(&video))
            || selection.audio_stream.is_some_and(|audio| discarded.contains(&audio))
            || discarded.len() != selection.discard_streams.len() || discarded.len() > 4096 {
            return Err(invalid("frame extraction needs explicit video and audio-or-no-audio selection without conflicts or repeated discards"));
        }
        self.assembly_settings().validate()
    }

    /// Internal reuse of the same bounded native source-admission profile.
    pub(crate) fn assembly_settings(&self) -> crate::video_assembly::AssemblyConfiguration {
        crate::video_assembly::AssemblyConfiguration {
            schema:crate::video_assembly::ASSEMBLY_CONFIGURATION_SCHEMA_V1.to_owned(),
            ffmpeg:self.ffmpeg.clone(), ffprobe:self.ffprobe.clone(), timeout_milliseconds:self.timeout_milliseconds,
            maximum_capture_bytes:self.maximum_capture_bytes, maximum_source_bytes:self.maximum_source_bytes,
            maximum_output_bytes:self.maximum_total_frame_bytes, maximum_frame_bytes:self.maximum_frame_bytes,
            maximum_total_frame_bytes:self.maximum_total_frame_bytes, maximum_pixels:self.maximum_pixels,
            maximum_working_bytes:self.maximum_working_bytes, minimum_free_bytes:self.minimum_free_bytes,
            delivery_crf:18, preset:"slow".to_owned(),
        }
    }

    pub(crate) fn tool_limits(&self) -> crate::audio_inspection::AudioInspectionConfiguration {
        self.assembly_settings().tool_limits()
    }
}

pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:FRAME_EXTRACTION_CONFIGURATION_SCHEMA_V1.to_owned(),version:"1.0.0".to_owned(),
        sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/frame-extraction/configuration.schema.json"))) }
}
pub fn operation_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:FRAME_EXTRACTION_OPERATION_SCHEMA_V1.to_owned(),version:"1.0.0".to_owned(),
        sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/frame-extraction/operation.schema.json"))) }
}

impl FrameExtractionOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        require_sha256(&self.source_sha256,"frame extraction source digest")?;
        if self.schema != FRAME_EXTRACTION_OPERATION_SCHEMA_V1 || self.source_byte_count == 0
            || self.source_byte_count > self.settings.maximum_source_bytes {
            return Err(invalid("frame extraction operation differs from its bounded source profile"));
        }
        Ok(())
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| invalid(error.to_string()))?;
        let values = value.as_object().ok_or_else(|| invalid("frame extraction operation must be an object"))?
            .iter().map(|(key,value)| (key.clone(),value.clone())).collect();
        crate::ProviderConfiguration::new(
            crate::ProviderReference { id:FRAME_EXTRACTION_PROVIDER_ID.to_owned(),version:"1.0.0".to_owned() },
            crate::CapabilityReference { id:FRAME_EXTRACTION_CAPABILITY_ID.to_owned(),version:"1.0.0".to_owned() },
            operation_schema_reference(),values,
        )
    }

    pub fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let port = |name: &str, artifact_type: &str, role: &str, stream: &str| json!({"name":name,"artifact_type":artifact_type,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let schema = operation_schema_reference();
        let tools: Vec<_> = [("ffmpeg",&self.settings.ffmpeg),("ffprobe",&self.settings.ffprobe)].into_iter()
            .map(|(id,pin)| json!({"id":id,"version_requirement":format!("={}",pin.version),"sha256":pin.sha256})).collect();
        let manifest = json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":FRAME_EXTRACTION_PROVIDER_ID,"version":"1.0.0","display_name":"aniflow original-clock frame extraction"},
            "configuration_schemas":[schema.clone()],"capabilities":[{
                "id":FRAME_EXTRACTION_CAPABILITY_ID,"version":"1.0.0","kind":"frame_processor","configuration_schema":schema,
                "inputs":[port("source","video/*","temporal_source","video")],
                "outputs":[port("frames","application/vnd.aniflow.rife-frames","intermediate","video"),
                    port("index","application/vnd.aniflow.rife-frame-index+json","validation_evidence","timed_metadata"),
                    port("evidence","application/vnd.aniflow.frame-extraction-report+json","validation_evidence","timed_metadata")],
                "batching":"whole_artifact","max_concurrency":1,
                "requirements":{"tools":tools,"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
                "behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},
                "lifecycle":{"progress":"none","cancellation":"process_signal"}}],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&manifest)?)
    }
}

pub(crate) fn observe_components(settings: &FrameExtractionConfiguration) -> Result<[(String,u64);2]> {
    settings.validate()?;
    crate::video_assembly::observe_components(&settings.assembly_settings())
}

/// Conservative source/tool/observation admission before geometry is known.
/// Native extraction separately admits the observed PNG envelope before writing
/// frames. Configured frame ceilings are not an invented allocation estimate.
pub(crate) fn checked_working_bytes(settings: &FrameExtractionConfiguration, source_bytes: u64, tool_bytes: u64) -> Result<u64> {
    let terms = [(source_bytes,3),(tool_bytes,2),
        (FRAME_EXTRACTION_MAXIMUM_INDEX_BYTES,4),(FRAME_EXTRACTION_MAXIMUM_REPORT_BYTES,4),(settings.maximum_capture_bytes as u64,12)];
    let bytes = terms.into_iter().try_fold(0_u64,|sum,(size,count)| size.checked_mul(count).and_then(|value| sum.checked_add(value))
        .ok_or_else(|| invalid("frame extraction working-data bound overflow")))?;
    if bytes > settings.maximum_working_bytes { return Err(invalid("frame extraction source/tool snapshots and observations exceed maximum_working_bytes")); }
    Ok(bytes)
}

fn component_inventory(settings: &FrameExtractionConfiguration) -> crate::ComponentInventory {
    crate::ComponentInventory { tools:[("ffmpeg",&settings.ffmpeg),("ffprobe",&settings.ffprobe)].into_iter()
        .map(|(id,pin)| crate::ComponentIdentity { id:id.to_owned(),version:pin.version.clone(),sha256:Some(pin.sha256.clone()) }).collect(),
        codecs:Vec::new(),models:Vec::new() }
}

fn authored_pipeline() -> Result<crate::PipelineV3Configuration> {
    let outputs = [("frames","extracted_frames","artifacts/frame-extraction/frames","directory"),
        ("index","extracted_index","artifacts/frame-extraction/index.json","file"),
        ("evidence","extraction_evidence","artifacts/frame-extraction/extraction.json","file")];
    let value = json!({"schema":"aniflow.pipeline/v3","name":"original-clock-frame-extraction",
        "description":"Read-only planning binds source/tools; native source admission, geometry, frame count and RGB conversion evidence remain pending until extraction.",
        "inputs":[{"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}],
        "stages":[{"id":"extract_frames","depends_on":[],"capability":{"id":FRAME_EXTRACTION_CAPABILITY_ID,"version_requirement":"=1.0.0"},
            "provider":{"primary":{"registration_id":REGISTRATION_ID},"fallbacks":[]},"inputs":[{"port":"source","artifacts":["source_video"]}],
            "outputs":outputs.iter().map(|(port,id,path,kind)| json!({"port":port,"artifacts":[{"id":id,"relative_path":path,"kind":kind}]})).collect::<Vec<_>>(),
            "validations":outputs.iter().map(|(_,id,_,_)| json!({"id":format!("{id}_integrity"),"artifact":id,"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1})).collect::<Vec<_>>() }],
        "outputs":outputs.iter().map(|(_,id,_,_)| json!({"id":id,"artifact":id,"required_validations":[format!("{id}_integrity")]})).collect::<Vec<_>>() });
    crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|failure| Error::new(failure.category(),failure.message))
}

struct Prepared {
    plan: FrameExtractionPlan,
    bindings: Vec<crate::PipelineInputBinding>,
    registry: crate::ProviderRegistry,
    required_free_bytes: u64,
}

fn prepare(request: &FrameExtractionRequest, token: &CancellationToken) -> Result<Prepared> {
    cancelled(token)?;
    request.configuration.validate()?;
    request.execution_limits.validate()?;
    let paths = [&request.source_video,&request.configuration.ffmpeg.executable,&request.configuration.ffprobe.executable,&request.provider_executable];
    for (position,path) in paths.iter().enumerate() {
        if !path.is_absolute() || path.components().any(|part| matches!(part,Component::CurDir|Component::ParentDir))
            || path.canonicalize().ok().as_deref() != Some(path.as_path())
            || std::fs::symlink_metadata(path).map_or(true,|metadata| !metadata.is_file())
            || paths[..position].iter().any(|other| path.starts_with(other) || other.starts_with(path)) {
            return Err(invalid("extraction source and executables must be distinct canonical nonsymlink files without traversal"));
        }
    }
    let source = identity(&request.source_video,request.configuration.maximum_source_bytes)?;
    if source.1 == 0 { return Err(invalid("frame extraction source must be nonempty")); }
    let components = observe_components(&request.configuration)?;
    let tool_bytes = components[0].1.checked_add(components[1].1).ok_or_else(|| invalid("frame extraction tool-byte bound overflow"))?;
    let required_free_bytes = checked_working_bytes(&request.configuration,source.1,tool_bytes)?;
    let operation = FrameExtractionOperation { schema:FRAME_EXTRACTION_OPERATION_SCHEMA_V1.to_owned(),settings:request.configuration.clone(),
        source_sha256:source.0.clone(),source_byte_count:source.1 };
    let mut registry = crate::ProviderRegistry::new();
    registry.register(crate::ProviderRegistration::new(REGISTRATION_ID,operation.manifest()?,operation.provider_configuration()?,
        request.provider_executable.clone(),IMPLEMENTATION_ID,component_inventory(&request.configuration))?)?;
    let bindings = vec![crate::PipelineInputBinding::new("source_video",&request.source_video)];
    let plan = crate::resolve_pipeline_v3(&authored_pipeline()?,&bindings,&registry,&crate::audio_inspection::context())
        .map_err(|failure| Error::new(failure.category(),failure.message))?;
    if !plan.payload.inputs.iter().any(|input| input.id == "source_video" && input.content_sha256 == source.0 && input.byte_count == source.1) {
        return Err(invalid("frame extraction source changed during planning"));
    }
    preflight_plan(&plan,&registry,token)?;
    if canonical_json_bytes(&plan)?.len() as u64 > MAXIMUM_PLAN_BYTES { return Err(invalid("frame extraction plan exceeds 64 MiB")); }
    cancelled(token)?;
    Ok(Prepared { plan,bindings,registry,required_free_bytes })
}

/// The same fixed topology and exact provider admission apply to generic v3
/// run/resume entry points. No tool execution or workspace write occurs here.
pub(crate) fn preflight_plan(plan: &crate::PipelineV3Plan, registry: &crate::ProviderRegistry, token: &CancellationToken) -> Result<()> {
    let stages: Vec<_> = plan.payload.stages.iter().filter(|stage| stage.capability.id == FRAME_EXTRACTION_CAPABILITY_ID
        || stage.provider_lock.payload.provider.id == FRAME_EXTRACTION_PROVIDER_ID).collect();
    if stages.is_empty() { return Ok(()); }
    if stages.len() != 1 || plan.payload.stages.len() != 1 { return Err(invalid("frame extraction requires its single fixed stage")); }
    cancelled(token)?;
    let stage = stages[0];
    let registration = registry.registration(REGISTRATION_ID).ok_or_else(|| invalid("frame extraction registration is unavailable"))?;
    let operation: FrameExtractionOperation = serde_json::from_value(serde_json::to_value(&registration.configuration().values)
        .map_err(|_| invalid("invalid extraction operation"))?).map_err(|_| invalid("invalid closed extraction operation"))?;
    operation.validate()?;
    let manifest = operation.manifest()?;
    if stage.id != "extract_frames" || stage.capability.id != FRAME_EXTRACTION_CAPABILITY_ID || stage.capability.version != "1.0.0"
        || stage.provider_lock.payload.registration_id != REGISTRATION_ID || stage.provider_lock.payload.provider.id != FRAME_EXTRACTION_PROVIDER_ID
        || operation.provider_configuration()? != *registration.configuration() || manifest != *registration.manifest()
        || component_inventory(&operation.settings) != *registration.components() || registration.implementation_id() != IMPLEMENTATION_ID
        || registration.configuration().effective_configuration_sha256 != stage.provider_lock.payload.effective_configuration_sha256
        || manifest.capabilities.first() != Some(&stage.capability) {
        return Err(invalid("frame extraction stage differs from its exact frozen operation"));
    }
    let components = observe_components(&operation.settings)?;
    let tool_bytes = components[0].1.checked_add(components[1].1).ok_or_else(|| invalid("frame extraction tool-byte bound overflow"))?;
    checked_working_bytes(&operation.settings,operation.source_byte_count,tool_bytes)?;
    let expected = authored_pipeline()?;
    if plan.payload.pipeline_name != expected.name || plan.payload.outputs != expected.outputs || plan.payload.inputs.len() != 1
        || plan.payload.configuration_sha256 != expected.configuration_sha256().map_err(|failure| Error::new(failure.category(),failure.message))? {
        return Err(invalid("frame extraction plan differs from its fixed source/output profile"));
    }
    let input = &plan.payload.inputs[0];
    let declaration = &expected.inputs[0];
    if input.id != declaration.id || input.artifact_type != declaration.artifact_type || input.artifact_role != declaration.artifact_role
        || input.stream_role != declaration.stream_role || input.kind != crate::PipelineInputKind::File || input.file_count != 1
        || input.content_sha256 != operation.source_sha256 || input.byte_count != operation.source_byte_count {
        return Err(invalid("frame extraction operation does not match its planned immutable source"));
    }
    let authored = &expected.stages[0];
    if stage.depends_on != authored.depends_on || stage.inputs != authored.inputs || stage.provider_selection != authored.provider
        || stage.capability_requirement != authored.capability || stage.validations != authored.validations
        || !stage.validation_providers.is_empty() || stage.outputs.len() != authored.outputs.len() {
        return Err(invalid("frame extraction graph or validation obligations differ from the fixed stage"));
    }
    for (actual,expected) in stage.outputs.iter().zip(&authored.outputs) {
        if actual.port != expected.port || actual.artifacts.len() != expected.artifacts.len()
            || actual.artifacts.iter().zip(&expected.artifacts).any(|(actual,expected)| actual.id != expected.id
                || actual.relative_path != expected.relative_path || actual.kind != expected.kind) {
            return Err(invalid("frame extraction output identity or destination differs from its fixed profile"));
        }
    }
    cancelled(token)
}

fn output_parent(request: &FrameExtractionRequest, requested: Option<PathBuf>) -> Result<PathBuf> {
    let parent = requested.unwrap_or_else(|| PathBuf::from(".aniflow/runs"));
    let parent = if parent.is_absolute() { parent } else { std::env::current_dir().map_err(|_| invalid("cannot resolve extraction output parent"))?.join(parent) };
    if parent.components().any(|part| matches!(part,Component::CurDir|Component::ParentDir)) { return Err(invalid("extraction output parent must be normalized without traversal")); }
    for input in [&request.source_video,&request.configuration.ffmpeg.executable,&request.configuration.ffprobe.executable,&request.provider_executable] {
        if parent.starts_with(input) { return Err(invalid("extraction output parent overlaps a source or executable")); }
    }
    let mut existing = parent.as_path();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(metadata) if metadata.is_dir() && existing.canonicalize().ok().as_deref() == Some(existing) => break,
            Ok(_) => return Err(invalid("extraction output ancestors must be canonical nonsymlink directories")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => existing = existing.parent().ok_or_else(|| invalid("extraction output has no existing ancestor"))?,
            Err(_) => return Err(invalid("cannot inspect extraction output parent")),
        }
    }
    Ok(parent)
}

/// Hash exact source/tool identities without probing, decoding or workspace
/// writes. Source support, geometry and frame count are pending native facts.
pub fn plan(request: &FrameExtractionRequest, token: &CancellationToken) -> Result<FrameExtractionPlan> { Ok(prepare(request,token)?.plan) }

pub fn run<F>(request: FrameExtractionRequest, output_directory: Option<PathBuf>, token: &CancellationToken, progress: F) -> Result<FrameExtractionOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    run_new(request,output_directory,None,None,token,progress)
}

/// Fixed-recipe child creation through the same extraction and v3 authority.
/// The exact child leaf must be absent; retained work is never overwritten.
pub(crate) fn run_at<F>(request: FrameExtractionRequest, run_directory: impl AsRef<Path>, expected_plan: &crate::PipelineV3Plan,
    token: &CancellationToken, progress: F) -> Result<FrameExtractionOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let directory = run_directory.as_ref();
    let parent = crate::run_v3::exact_run_parent(directory)?;
    run_new(request,Some(parent),Some(directory),Some(expected_plan),token,progress)
}

fn run_new<F>(request: FrameExtractionRequest, output_directory: Option<PathBuf>, exact_directory: Option<&Path>,
    expected_plan: Option<&crate::PipelineV3Plan>, token: &CancellationToken, progress: F) -> Result<FrameExtractionOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let parent = output_parent(&request,output_directory)?;
    let prepared = prepare(&request,token)?;
    if expected_plan.is_some_and(|expected| expected != &prepared.plan) {
        return Err(Error::new(ErrorCategory::State,"extraction plan changed after the recipe handoff; no child run was created"));
    }
    require_storage(&parent,prepared.required_free_bytes,request.configuration.minimum_free_bytes)?;
    let execution = crate::PipelineV3RunRequest::new(prepared.plan,prepared.bindings,prepared.registry)
        .with_output_directory(parent).with_execution_limits(request.execution_limits);
    match exact_directory {
        Some(directory) => crate::run_v3::run_v3_at_with_progress_and_cancellation(execution,directory,token,progress),
        None => crate::run_v3_with_progress_and_cancellation(execution,token,progress),
    }
}

/// Reuse only the unchanged source, configuration, tools and accepted v3 stage.
/// Interrupted extraction retries the whole stage; loose PNGs are not adopted.
pub fn resume<F>(request: FrameExtractionRequest, run_directory: impl AsRef<Path>, token: &CancellationToken, progress: F) -> Result<FrameExtractionOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let prepared = prepare(&request,token)?;
    let root = run_directory.as_ref().canonicalize().map_err(|_| invalid("cannot resolve extraction run directory"))?;
    let workspace = crate::PipelineV3Workspace::open_read_only(&root)?;
    let path = workspace.plan();
    let observed = identity(&path,MAXIMUM_PLAN_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(&path).map_err(|_| invalid("cannot safely open extraction plan"))?.take(MAXIMUM_PLAN_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot read bounded extraction plan"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed { return Err(invalid("extraction plan changed while reading")); }
    let retained = crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|failure| Error::new(failure.category(),failure.message))?;
    if retained != prepared.plan { return Err(invalid("extraction source, tools or settings changed; start a new run")); }
    // A retrying provider admits new allocations; complete-stage reuse does not
    // reserve a second full extraction footprint.
    require_storage(&root,1_048_576,request.configuration.minimum_free_bytes)?;
    let execution = crate::PipelineV3ResumeRequest::new(root,prepared.bindings,prepared.registry).with_execution_limits(request.execution_limits);
    crate::resume_v3_with_progress_and_cancellation(execution,token,progress)
}

#[cfg(all(test, unix))]
mod exact_child_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture() -> (tempfile::TempDir, PathBuf, FrameExtractionRequest) {
        let directory = tempfile::tempdir().expect("synthetic directory");
        let root = directory.path().canonicalize().expect("canonical directory");
        let source = root.join("source.media");
        std::fs::write(&source,b"synthetic source; no native media claim").expect("source fixture");
        let pin = |name: &str| {
            let path = root.join(name);
            let bytes = format!("#!/bin/sh\nexit 99\n# {name}\n");
            std::fs::write(&path,&bytes).expect("tool fixture");
            std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o700)).expect("tool mode");
            crate::audio_inspection::AudioToolPin { executable:path,version:"8.0.1".to_owned(),
                sha256:format!("{:x}",Sha256::digest(bytes.as_bytes())) }
        };
        let configuration = FrameExtractionConfiguration::from_json_slice(&serde_json::to_vec(&json!({
            "schema":FRAME_EXTRACTION_CONFIGURATION_SCHEMA_V1,"ffmpeg":pin("ffmpeg"),"ffprobe":pin("ffprobe"),
            "source_selection":{"video_stream":0,"audio_stream":null,"no_audio":true,"discard_streams":[]}
        })).expect("fixture configuration bytes")).expect("fixture configuration");
        let provider = std::env::current_exe().expect("test executable").canonicalize().expect("canonical executable");
        let request = FrameExtractionRequest::new(source,configuration,provider);
        (directory,root,request)
    }

    #[test]
    fn exact_child_refuses_changed_plan_before_workspace_creation() {
        let (_directory,root,mut request) = fixture();
        let token = CancellationToken::default();
        let expected = plan(&request,&token).expect("source-only plan");
        request.configuration.maximum_frames = 2;
        let child = root.join("extract_frames");
        let mut progressed = false;
        let failure = run_at(request,&child,&expected,&token,|_| progressed = true).expect_err("changed source intent");
        assert_eq!(failure.category(),ErrorCategory::State);
        assert!(failure.message().contains("plan changed"));
        assert!(!progressed);
        assert!(!child.exists());
    }

    #[test]
    fn exact_child_preserves_existing_incomplete_initialization() {
        let (_directory,root,request) = fixture();
        let token = CancellationToken::default();
        let expected = plan(&request,&token).expect("source-only plan");
        let child = root.join("extract_frames");
        std::fs::create_dir(&child).expect("retained child");
        let retained = child.join("partial-source-observation");
        std::fs::write(&retained,b"retain failure evidence").expect("retained evidence");
        let mut progressed = false;
        let failure = run_at(request,&child,&expected,&token,|_| progressed = true).expect_err("existing child refused");
        assert_eq!(failure.category(),ErrorCategory::State);
        assert!(!progressed);
        assert_eq!(std::fs::read(&retained).expect("preserved evidence"),b"retain failure evidence");
        assert!(!child.join("plan.json").exists());
    }
}
