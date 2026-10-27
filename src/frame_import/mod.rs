//! Import observed PNG workspaces through the existing Pipeline v3 runtime.
//! Current decoder evidence never invents historical processing provenance.
mod provider;
mod types;

use std::ffi::OsString;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use sha2::{Digest, Sha256};
use crate::audio_inspection::{AudioInspectionConfiguration, AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1};
use crate::audio_inspection::process::{open_regular, verify_pin};
use crate::frame_io::{ensure_canonical_directory, identity, inventory};
use crate::provider::decode_json;
use crate::video_interpolation::{RifeFrameIndex, RIFE_MAXIMUM_INDEX_BYTES};
use crate::{CancellationToken, Error, ErrorCategory, Result};
pub use provider::execute_invocation;
pub use types::*;

fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input, message) }

impl FrameImportConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("frame import configuration exceeds 64 KiB")); }
        let value: Self = decode_json(bytes, "frame import configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != FRAME_IMPORT_CONFIGURATION_SCHEMA_V1
            || !(1..=1_099_511_627_776).contains(&self.maximum_source_bytes)
            || !(1024..=67_108_864).contains(&self.maximum_frame_bytes)
            || !(1_048_576..=1_099_511_627_776).contains(&self.maximum_total_frame_bytes)
            || !(1..=8_400_000).contains(&self.maximum_pixels)
        { return Err(invalid("unsupported frame import profile or resource bounds")); }
        self.tool_limits().validate()
    }

    pub(crate) fn tool_limits(&self) -> AudioInspectionConfiguration {
        AudioInspectionConfiguration {
            schema: AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.to_owned(),
            ffmpeg: self.ffmpeg.clone(), ffprobe: self.ffmpeg.clone(),
            tool_timeout_milliseconds: self.tool_timeout_milliseconds,
            maximum_tool_output_bytes: 65_536,
        }
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self).map_err(|error| invalid(error.to_string()))?
            .as_object().ok_or_else(|| invalid("frame import configuration must be an object"))?
            .iter().map(|(key, value)| (key.clone(), value.clone())).collect();
        crate::ProviderConfiguration::new(
            crate::ProviderReference { id: FRAME_IMPORT_PROVIDER_ID.to_owned(), version: "1.0.0".to_owned() },
            crate::CapabilityReference { id: FRAME_IMPORT_CAPABILITY_ID.to_owned(), version: "1.0.0".to_owned() },
            configuration_schema_reference(), values,
        )
    }

    /// Inert exact manifest for the contemporary importer, not a prior processor.
    pub fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let schema = configuration_schema_reference();
        let value = serde_json::json!({
            "schema":"aniflow.provider-manifest/v1",
            "provider":{"id":FRAME_IMPORT_PROVIDER_ID,"version":"1.0.0","display_name":"aniflow observed frame import"},
            "configuration_schemas":[schema.clone()],
            "capabilities":[{
                "id":FRAME_IMPORT_CAPABILITY_ID,"version":"1.0.0","kind":"frame_processor","configuration_schema":schema,
                "inputs":[
                    {"name":"source","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video","cardinality":"one","immutable":true},
                    {"name":"frames","artifact_type":"application/vnd.aniflow.rife-frames","artifact_role":"intermediate","stream_role":"video","cardinality":"one","immutable":true},
                    {"name":"index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata","cardinality":"one","immutable":true}
                ],
                "outputs":[
                    {"name":"frames","artifact_type":"application/vnd.aniflow.rife-frames","artifact_role":"intermediate","stream_role":"video","cardinality":"one","immutable":true},
                    {"name":"evidence","artifact_type":"application/vnd.aniflow.frame-import-report+json","artifact_role":"validation_evidence","stream_role":"timed_metadata","cardinality":"one","immutable":true}
                ],
                "batching":"whole_artifact","max_concurrency":1,
                "requirements":{"tools":[{"id":"ffmpeg","version_requirement":format!("={}",self.ffmpeg.version),"sha256":self.ffmpeg.sha256}],"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
                "behavior":{"determinism":"environment_dependent","fidelity":"observational","cacheable":false,"content_changes":false,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},
                "lifecycle":{"progress":"none","cancellation":"process_signal"}
            }],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}
        });
        crate::ProviderManifest::from_json_slice(&serde_json::to_vec(&value).map_err(|error| invalid(error.to_string()))?)
    }
}

pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference {
        id: FRAME_IMPORT_CONFIGURATION_SCHEMA_V1.to_owned(), version: "1.0.0".to_owned(),
        sha256: format!("{:x}", Sha256::digest(include_bytes!("../../providers/frame-import/configuration.schema.json"))),
    }
}

pub(crate) fn read_index(path: &Path) -> Result<(RifeFrameIndex, (String, u64))> {
    let observed = identity(path, RIFE_MAXIMUM_INDEX_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(path).map_err(|_| invalid("cannot open frame index"))?
        .take(RIFE_MAXIMUM_INDEX_BYTES + 1).read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot read frame index"))?;
    if (format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64) != observed {
        return Err(invalid("frame index changed during observation"));
    }
    Ok((RifeFrameIndex::from_json_slice(&bytes)?, observed))
}

pub(crate) fn admit_geometry(config: &FrameImportConfiguration, index: &RifeFrameIndex) -> Result<u64> {
    index.validate()?;
    let pixels = u64::from(index.width).checked_mul(u64::from(index.height))
        .filter(|value| *value <= config.maximum_pixels)
        .ok_or_else(|| invalid("frame import exceeds the admitted pixel count"))?;
    pixels.checked_mul(3).ok_or_else(|| invalid("decoded RGB24 byte count overflow"))
}

// The source video can be much larger than one frame. Keep its admission hash
// cancellable without introducing another source snapshot or processing step.
fn identify_source(path: &Path, maximum: u64, cancellation: &CancellationToken) -> Result<(String, u64)> {
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(invalid("frame import source must be canonical and symlink-free"));
    }
    let file = open_regular(path).map_err(|_| invalid("cannot safely open frame import source"))?;
    let metadata = file.metadata().map_err(|_| invalid("cannot inspect frame import source"))?;
    if metadata.len() == 0 || metadata.len() > maximum { return Err(invalid("frame import source exceeds its nonempty byte bound")); }
    let mut input = file.take(maximum + 1);
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "frame import source identification cancelled")); }
        let count = input.read(&mut buffer).map_err(|_| invalid("cannot read frame import source"))?;
        if count == 0 { break; }
        total += count as u64;
        if total > maximum { return Err(invalid("frame import source grew beyond its byte bound")); }
        digest.update(&buffer[..count]);
    }
    if total != metadata.len() || path.canonicalize().ok().as_deref() != Some(path) {
        return Err(invalid("frame import source changed during identification"));
    }
    Ok((format!("{:x}", digest.finalize()), total))
}

pub(crate) struct PreparedFrameInputs {
    pub registry: crate::ProviderRegistry,
    pub bindings: Vec<crate::PipelineInputBinding>,
    pub source: (String, u64),
    pub raw_index: (String, u64),
    pub index: RifeFrameIndex,
    pub frame_bytes: u64,
}

pub(crate) fn prepare_inputs(request: &FrameImportRequest, cancellation: &CancellationToken) -> Result<PreparedFrameInputs> {
    if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "frame import planning cancelled")); }
    request.configuration.validate()?;
    let (index, raw_index) = read_index(&request.frame_index)?;
    admit_geometry(&request.configuration, &index)?;
    ensure_canonical_directory(&request.frames_directory)?;
    let names: Vec<OsString> = index.frames.iter().map(|frame| frame.filename.clone().into()).collect();
    inventory(&request.frames_directory, &names)?;
    let mut total = 0_u64;
    for frame in &index.frames {
        if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "frame import planning cancelled")); }
        let observed = identity(&request.frames_directory.join(&frame.filename), request.configuration.maximum_frame_bytes)?;
        if observed.0 != frame.sha256 { return Err(invalid("PNG bytes differ from the declared frame digest")); }
        total = total.checked_add(observed.1).filter(|value| *value <= request.configuration.maximum_total_frame_bytes)
            .ok_or_else(|| invalid("frame workspace exceeds the aggregate byte bound"))?;
    }
    let source = identify_source(&request.source_video, request.configuration.maximum_source_bytes, cancellation)?;
    if source.0 != index.temporal_inspection.source_sha256 || source.1 == 0
        || source.1 != index.temporal_inspection.source_size_bytes
    { return Err(invalid("source video identity differs from the declared temporal inspection")); }
    verify_pin(&request.configuration.ffmpeg, "ffmpeg").map_err(|_| Error::new(ErrorCategory::Dependency, "frame import decoder identity is unavailable or changed"))?;
    let mut registry = crate::ProviderRegistry::new();
    registry.register(crate::ProviderRegistration::new(
        "frame-import-native", request.configuration.manifest()?, request.configuration.provider_configuration()?,
        request.provider_executable.clone(), "aniflow-frame-import-v1",
        crate::ComponentInventory { tools:vec![crate::ComponentIdentity { id:"ffmpeg".to_owned(), version:request.configuration.ffmpeg.version.clone(), sha256:Some(request.configuration.ffmpeg.sha256.clone()) }], codecs:Vec::new(), models:Vec::new() },
    )?)?;
    let bindings = vec![
        crate::PipelineInputBinding::new("source_video", &request.source_video),
        crate::PipelineInputBinding::new("source_frames", &request.frames_directory),
        crate::PipelineInputBinding::new("source_index", &request.frame_index),
    ];
    Ok(PreparedFrameInputs { registry, bindings, source, raw_index, index, frame_bytes: total })
}

pub(crate) fn bind_prepared_plan(plan: &crate::PipelineV3Plan, prepared: &PreparedFrameInputs) -> Result<()> {
    for (id, expected) in [("source_video", &prepared.source), ("source_index", &prepared.raw_index)] {
        if !plan.payload.inputs.iter().any(|input| input.id == id && input.content_sha256 == expected.0 && input.byte_count == expected.1) {
            return Err(invalid("source or frame index changed between import admission and planning"));
        }
    }
    Ok(())
}

fn prepare(request: &FrameImportRequest, cancellation: &CancellationToken) -> Result<(crate::PipelineV3Plan, crate::ProviderRegistry, Vec<crate::PipelineInputBinding>)> {
    let prepared = prepare_inputs(request, cancellation)?;
    let pipeline = crate::PipelineV3Configuration::from_yaml_slice(include_bytes!("../../providers/frame-import/pipeline.yml"))
        .map_err(|failure| Error::new(failure.category(), failure.message))?;
    let plan = crate::resolve_pipeline_v3(&pipeline, &prepared.bindings, &prepared.registry, &crate::audio_inspection::context())
        .map_err(|failure| Error::new(failure.category(), failure.message))?;
    bind_prepared_plan(&plan, &prepared)?;
    if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "frame import planning cancelled")); }
    Ok((plan, prepared.registry, prepared.bindings))
}

/// Resolve real identities without creating a workspace or launching the decoder.
/// The plan is not decoded-image acceptance or a historical-provenance claim.
pub fn plan(request: &FrameImportRequest, cancellation: &CancellationToken) -> Result<crate::PipelineV3Plan> {
    Ok(prepare(request, cancellation)?.0)
}

pub(crate) fn output_parent(request: &FrameImportRequest, requested: Option<PathBuf>) -> Result<PathBuf> {
    let parent = requested.unwrap_or_else(|| PathBuf::from(".aniflow/runs"));
    let parent = if parent.is_absolute() { parent } else {
        std::env::current_dir().map_err(|_| invalid("cannot resolve frame import output parent"))?.join(parent)
    };
    if parent.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err(invalid("frame import output parent must not contain parent traversal"));
    }
    for input in [&request.source_video, &request.frames_directory, &request.frame_index, &request.configuration.ffmpeg.executable] {
        if parent.starts_with(input) { return Err(invalid("frame import output parent must be outside every input artifact")); }
    }
    // Inspect existing ancestors without creating directories. In particular,
    // do not create a new run inside the original PNG directory and then refuse.
    let mut existing = parent.as_path();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(metadata) if metadata.is_dir() && existing.canonicalize().ok().as_deref() == Some(existing) => break,
            Ok(_) => return Err(invalid("frame import output ancestors must be nonsymlink directories")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing.parent().ok_or_else(|| invalid("frame import output has no existing ancestor"))?;
            }
            Err(_) => return Err(invalid("cannot inspect frame import output parent")),
        }
    }
    Ok(parent)
}

/// Execute a fresh import using existing immutable stage acceptance and recovery.
pub fn run<F>(request: FrameImportRequest, output_directory: Option<std::path::PathBuf>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let parent = output_parent(&request, output_directory)?;
    let (plan, registry, bindings) = prepare(&request, cancellation)?;
    let mut execution = crate::PipelineV3RunRequest::new(plan, bindings, registry).with_execution_limits(request.execution_limits);
    execution.output_directory = Some(parent);
    crate::run_v3_with_progress_and_cancellation(execution, cancellation, on_progress)
}
