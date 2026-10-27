//! Bounded per-frame Upscayl recovery using ordinary Pipeline v3 checkpoints.
mod provider;
mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use crate::audio_inspection::process::{open_regular, verify_pin};
use crate::frame_import::{self, FrameImportRequest};
use crate::frame_io::identity;
use crate::provider::{canonical_json_bytes, canonical_sha256, decode_json, require_sha256};
use crate::{CancellationToken, Error, ErrorCategory, Result};
pub use provider::{execute_collect_invocation, execute_frame_invocation};
pub use types::*;

pub(crate) const MAXIMUM_TOOL_BYTES: u64 = 134_217_728;
pub(crate) const MAXIMUM_MODEL_BYTES: u64 = 1_073_741_824;
const MAXIMUM_PLAN_BYTES: u64 = 67_108_864;
fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input, message) }

impl UpscaylRecoveryConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("Upscayl recovery configuration exceeds 64 KiB")); }
        let value: Self = decode_json(bytes, "Upscayl recovery configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        self.import.validate()?;
        let mut limits = self.import.tool_limits();
        limits.ffprobe = self.upscayl.clone();
        limits.validate()?;
        if self.schema != UPSCAYL_RECOVERY_CONFIGURATION_SCHEMA_V1 || self.model.native_scale != 4
            || !(2..=4).contains(&self.output_scale) || !(32..=4096).contains(&self.tile_size)
            || self.gpu_id > 255 || self.threads != "1:1:1"
            || !(1024..=268_435_456).contains(&self.maximum_output_frame_bytes)
            || !(1..=134_217_728).contains(&self.maximum_output_pixels)
            || !(1_048_576..=17_592_186_044_416).contains(&self.maximum_working_bytes)
            || self.minimum_free_bytes > 1_099_511_627_776
        { return Err(invalid("unsupported bounded per-frame Upscayl settings")); }
        let name = self.model.name.as_bytes();
        if name.is_empty() || name.len() > 64 || !name[0].is_ascii_alphanumeric()
            || name.iter().any(|byte| !byte.is_ascii_alphanumeric() && !b"._-".contains(byte))
        { return Err(invalid("Upscayl model name must be a short portable ASCII token")); }
        // The pinned Unix interface overrides -z from these substrings, in this
        // order. Refuse a conflicting name rather than trusting our own flag.
        for scale in 1..=4 {
            if self.model.name.contains(&format!("x{scale}")) || self.model.name.contains(&format!("{scale}x")) {
                if scale != self.model.native_scale { return Err(invalid("model name overrides the declared native scale in the pinned Upscayl interface")); }
                break;
            }
        }
        for file in [&self.model.param, &self.model.weights] {
            require_sha256(&file.sha256, "Upscayl model digest")?;
            if !file.path.is_absolute() || file.path.components().any(|part| matches!(part, Component::CurDir | Component::ParentDir))
                || file.path.to_str().is_none_or(|path| path.chars().any(char::is_control)
                    || path.split('/').any(|part| matches!(part, "." | ".."))) {
                return Err(invalid("Upscayl model files require normalized absolute UTF-8 paths without controls or traversal"));
            }
        }
        if self.model.param.path == self.model.weights.path { return Err(invalid("model param and weights must be separate files")); }
        Ok(())
    }

    pub fn output_dimensions(&self, width: u32, height: u32) -> Result<(u32, u32)> {
        self.validate()?;
        let output_width = width.checked_mul(u32::from(self.output_scale)).filter(|value| *value > 0).ok_or_else(|| invalid("Upscayl output width exceeds its bound"))?;
        let output_height = height.checked_mul(u32::from(self.output_scale)).filter(|value| *value > 0).ok_or_else(|| invalid("Upscayl output height exceeds its bound"))?;
        let native_width = width.checked_mul(u32::from(self.model.native_scale)).ok_or_else(|| invalid("Upscayl native intermediate width exceeds its bound"))?;
        let native_height = height.checked_mul(u32::from(self.model.native_scale)).ok_or_else(|| invalid("Upscayl native intermediate height exceeds its bound"))?;
        if u64::from(output_width) * u64::from(output_height) > self.maximum_output_pixels
            || u64::from(native_width) * u64::from(native_height) > self.maximum_output_pixels {
            return Err(invalid("Upscayl output or native intermediate exceeds maximum_output_pixels"));
        }
        Ok((output_width, output_height))
    }

    pub fn model_filenames(&self) -> Result<(String, String)> {
        self.validate()?;
        let stem = if self.model.name == "realesr-animevideov3" {
            format!("{}-x{}", self.model.name, self.model.native_scale)
        } else { self.model.name.clone() };
        Ok((format!("{stem}.param"), format!("{stem}.bin")))
    }
}

impl UpscaylFrameOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        let frame = &self.frame;
        if self.schema != UPSCAYL_FRAME_OPERATION_SCHEMA_V1 || frame.ordinal >= UPSCAYL_MAXIMUM_FRAMES as u64
            || frame.source_byte_count == 0 || frame.source_byte_count > self.settings.import.maximum_source_bytes
            || frame.input_byte_count == 0 || frame.input_byte_count > self.settings.import.maximum_frame_bytes
            || frame.filename.is_empty() || frame.filename.len() > 128 || !frame.filename.ends_with(".png")
            || frame.filename.starts_with('.')
            || !frame.filename.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            || u64::from(frame.input_width) * u64::from(frame.input_height) > self.settings.import.maximum_pixels
        { return Err(invalid("invalid bounded source-frame operation")); }
        for value in [&frame.source_sha256, &frame.frame_index_sha256, &frame.input_sha256] { require_sha256(value, "source frame identity")?; }
        frame.source_time.duration().map_err(|error| Error::from_anyhow(ErrorCategory::Media, error))?;
        self.settings.output_dimensions(frame.input_width, frame.input_height)?;
        Ok(())
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        configuration(self, UPSCAYL_FRAME_PROVIDER_ID, UPSCAYL_FRAME_CAPABILITY_ID, frame_schema_reference())
    }
    pub fn manifest(&self) -> Result<crate::ProviderManifest> { self.validate()?; manifest(&self.settings, false) }
}

impl UpscaylCollectOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        require_sha256(&self.frame_index_sha256, "original frame index")?;
        if self.schema != UPSCAYL_COLLECT_OPERATION_SCHEMA_V1 || !(1..=UPSCAYL_MAXIMUM_FRAMES as u64).contains(&self.frame_count) {
            return Err(invalid("invalid bounded Upscayl collector operation"));
        }
        Ok(())
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        configuration(self, UPSCAYL_COLLECT_PROVIDER_ID, UPSCAYL_COLLECT_CAPABILITY_ID, collect_schema_reference())
    }
    pub fn manifest(&self) -> Result<crate::ProviderManifest> { self.validate()?; manifest(&self.settings, true) }
}

impl UpscaylOutputIndex {
    pub fn validate(&self) -> Result<()> {
        self.source_index.validate()?;
        if self.schema != UPSCAYL_OUTPUT_INDEX_SCHEMA_V1 || self.frames.len() != self.source_index.frames.len()
            || self.frames.len() > UPSCAYL_MAXIMUM_FRAMES || self.output_profile != self.source_index.frame_profile
            || !(2..=4).any(|scale| self.source_index.width.checked_mul(scale) == Some(self.output_width)
                && self.source_index.height.checked_mul(scale) == Some(self.output_height)) {
            return Err(invalid("upscaled index must preserve the complete source mapping with explicit derived geometry"));
        }
        for (source, output) in self.source_index.frames.iter().zip(&self.frames) {
            if source.ordinal != output.ordinal || source.filename != output.filename { return Err(invalid("upscaled frame mapping differs from original index")); }
            require_sha256(&output.sha256, "upscaled frame digest")?;
        }
        Ok(())
    }
}

fn configuration<T: Serialize>(operation: &T, provider: &str, capability: &str, schema: crate::ConfigurationSchemaReference) -> Result<crate::ProviderConfiguration> {
    let value = serde_json::to_value(operation).map_err(|error| invalid(error.to_string()))?;
    let values = value.as_object().ok_or_else(|| invalid("Upscayl operation must be an object"))?.iter().map(|(key,value)| (key.clone(),value.clone())).collect();
    crate::ProviderConfiguration::new(crate::ProviderReference { id:provider.to_owned(), version:"1.0.0".to_owned() }, crate::CapabilityReference { id:capability.to_owned(), version:"1.0.0".to_owned() }, schema, values)
}

pub fn frame_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:UPSCAYL_FRAME_OPERATION_SCHEMA_V1.to_owned(), version:"1.0.0".to_owned(), sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/upscayl-recovery/frame-operation.schema.json"))) }
}
pub fn collect_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:UPSCAYL_COLLECT_OPERATION_SCHEMA_V1.to_owned(), version:"1.0.0".to_owned(), sha256:format!("{:x}",Sha256::digest(include_bytes!("../../providers/upscayl-recovery/collect-operation.schema.json"))) }
}

fn port(name: &str, kind: &str, role: &str, stream: &str, cardinality: &str) -> Value {
    json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":cardinality,"immutable":true})
}

fn manifest(settings: &UpscaylRecoveryConfiguration, collect: bool) -> Result<crate::ProviderManifest> {
    let (provider, capability, schema) = if collect { (UPSCAYL_COLLECT_PROVIDER_ID,UPSCAYL_COLLECT_CAPABILITY_ID,collect_schema_reference()) }
        else { (UPSCAYL_FRAME_PROVIDER_ID,UPSCAYL_FRAME_CAPABILITY_ID,frame_schema_reference()) };
    let frame_type = "application/vnd.aniflow.upscayl-frame-report+json";
    let (inputs, outputs) = if collect {
        (vec![port("index","application/vnd.aniflow.rife-frame-index+json","validation_evidence","timed_metadata","one"), port("frames","image/png","intermediate","video","one_or_more"),port("evidence",frame_type,"validation_evidence","timed_metadata","one_or_more")],
         vec![port("frames","application/vnd.aniflow.upscayl-frames","intermediate","video","one"),port("index","application/vnd.aniflow.upscayl-output-index+json","validation_evidence","timed_metadata","one"),port("evidence","application/vnd.aniflow.upscayl-recovery-report+json","validation_evidence","timed_metadata","one")])
    } else {
        (vec![port("frame","image/png","intermediate","video","one")],vec![port("frame","image/png","intermediate","video","one"),port("evidence",frame_type,"validation_evidence","timed_metadata","one")])
    };
    let tools: Vec<_> = [("upscayl-bin",&settings.upscayl),("ffmpeg",&settings.import.ffmpeg)].into_iter()
        .map(|(id,pin)| json!({"id":id,"version_requirement":format!("={}",pin.version),"sha256":pin.sha256})).collect();
    let models = vec![json!({"id":"upscayl-param","version_requirement":"=1.0.0","sha256":settings.model.param.sha256}),json!({"id":"upscayl-weights","version_requirement":"=1.0.0","sha256":settings.model.weights.sha256})];
    let value = json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":provider,"version":"1.0.0","display_name":"aniflow bounded Upscayl recovery"},"configuration_schemas":[schema.clone()],"capabilities":[{
        "id":capability,"version":"1.0.0","kind":"frame_processor","configuration_schema":schema,"inputs":inputs,"outputs":outputs,"batching":"whole_artifact","max_concurrency":1,
        "requirements":{"tools":tools,"codecs":[],"models":models,"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":if collect {"forbidden"} else {"optional"},"network":"forbidden"}},
        "behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":if collect {vec!["filesystem_read","filesystem_write"]} else {vec!["filesystem_read","filesystem_write","environment_read","subprocess","gpu"]}},
        "lifecycle":{"progress":"none","cancellation":"process_signal"}}],
        "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
    crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
}

fn inventory(settings: &UpscaylRecoveryConfiguration) -> crate::ComponentInventory {
    crate::ComponentInventory { tools:[("upscayl-bin",&settings.upscayl),("ffmpeg",&settings.import.ffmpeg)].into_iter().map(|(id,pin)| crate::ComponentIdentity { id:id.to_owned(), version:pin.version.clone(), sha256:Some(pin.sha256.clone()) }).collect(), codecs:Vec::new(),
        models:[("upscayl-param",&settings.model.param),("upscayl-weights",&settings.model.weights)].into_iter().map(|(id,pin)| crate::ComponentIdentity { id:id.to_owned(), version:"1.0.0".to_owned(), sha256:Some(pin.sha256.clone()) }).collect() }
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution,"Upscayl recovery preflight cancelled")); }
    Ok(())
}

/// Fresh file observations, never a native availability or qualification probe.
fn verify_components(settings: &UpscaylRecoveryConfiguration, cancellation: &CancellationToken) -> Result<u64> {
    settings.validate()?;
    let mut total = 0_u64;
    for (pin,id) in [(&settings.upscayl,"upscayl-bin"),(&settings.import.ffmpeg,"ffmpeg")] {
        check_cancelled(cancellation)?;
        verify_pin(pin,id).map_err(|_| Error::new(ErrorCategory::Dependency,"Upscayl executable or decoder pin changed"))?;
        total += identity(&pin.executable,MAXIMUM_TOOL_BYTES)?.1;
    }
    for pin in [&settings.model.param,&settings.model.weights] {
        check_cancelled(cancellation)?;
        let observed = identity(&pin.path,MAXIMUM_MODEL_BYTES)?;
        if observed.0 != pin.sha256 || observed.1 == 0 { return Err(Error::new(ErrorCategory::Dependency,"Upscayl model file changed or is empty")); }
        total += observed.1;
    }
    check_cancelled(cancellation)?;
    Ok(total)
}

/// Generic v3 entry points also recheck component files before accepting reuse.
pub(crate) fn preflight_plan(plan: &crate::PipelineV3Plan, registry: &crate::ProviderRegistry, cancellation: &CancellationToken) -> Result<()> {
    let mut checked = BTreeSet::new();
    let mut frames = BTreeMap::new();
    let mut collectors = Vec::new();
    for stage in &plan.payload.stages {
        let id = stage.capability.id.as_str();
        if !matches!(id, UPSCAYL_FRAME_CAPABILITY_ID | UPSCAYL_COLLECT_CAPABILITY_ID) { continue; }
        check_cancelled(cancellation)?;
        if stage.capability.version != "1.0.0" { return Err(invalid("unsupported Upscayl recovery capability version")); }
        let registration = registry.registration(&stage.provider_lock.payload.registration_id).ok_or_else(|| invalid("Upscayl registration is missing"))?;
        let config = registration.configuration();
        if config.effective_configuration_sha256 != stage.provider_lock.payload.effective_configuration_sha256 || config.capability.id != id {
            return Err(invalid("Upscayl effective configuration differs from the locked plan"));
        }
        let value = serde_json::to_value(&config.values).map_err(|error| invalid(error.to_string()))?;
        let settings = if id == UPSCAYL_FRAME_CAPABILITY_ID {
            let operation: UpscaylFrameOperation = serde_json::from_value(value).map_err(|_| invalid("invalid Upscayl frame operation"))?;
            if operation.provider_configuration()? != *config { return Err(invalid("Upscayl frame operation differs from its lock")); }
            let source_id = format!("source_frame_{:06}",operation.frame.ordinal);
            if stage.id != format!("upscale_frame_{:06}",operation.frame.ordinal)
                || !stage.depends_on.is_empty() || stage.inputs.len() != 1
                || stage.inputs[0].port != "frame" || stage.inputs[0].artifacts != [source_id.clone()]
                || !plan.payload.inputs.iter().any(|input| input.id == source_id
                    && input.content_sha256 == operation.frame.input_sha256 && input.byte_count == operation.frame.input_byte_count)
                || !plan.payload.inputs.iter().any(|input| input.id == "source_video"
                    && input.content_sha256 == operation.frame.source_sha256 && input.byte_count == operation.frame.source_byte_count)
            { return Err(invalid("Upscayl frame requires its fixed source-bound independent stage")); }
            frames.insert(stage.id.clone(),operation.clone());
            operation.settings
        } else {
            let operation: UpscaylCollectOperation = serde_json::from_value(value).map_err(|_| invalid("invalid Upscayl collector operation"))?;
            if operation.provider_configuration()? != *config { return Err(invalid("Upscayl collector operation differs from its lock")); }
            collectors.push((stage,operation.clone()));
            operation.settings
        };
        if checked.insert(canonical_sha256(&settings)?) { verify_components(&settings,cancellation)?; }
    }
    if frames.len() > UPSCAYL_MAXIMUM_FRAMES || collectors.len() > 1 { return Err(invalid("Upscayl recovery topology exceeds its fixed bound")); }
    for (collector,operation) in collectors {
        let count = operation.frame_count;
        let stage_ids: Vec<_> = (0..count).map(|ordinal| format!("upscale_frame_{ordinal:06}")).collect();
        let frame_ids: Vec<_> = (0..count).map(|ordinal| format!("frame_{ordinal:06}")).collect();
        let report_ids: Vec<_> = (0..count).map(|ordinal| format!("evidence_{ordinal:06}")).collect();
        let has_binding = |port: &str, ids: &[String]| collector.inputs.iter().filter(|binding| binding.port == port && binding.artifacts == ids).count() == 1;
        if collector.id != "collect_upscaled_frames" || collector.depends_on != stage_ids
            || frames.len() != count as usize || collector.inputs.len() != 3
            || !has_binding("index", &["source_index".to_owned()]) || !has_binding("frames", &frame_ids) || !has_binding("evidence", &report_ids)
            || !plan.payload.inputs.iter().any(|input| input.id == "source_index")
        { return Err(invalid("Upscayl collector requires every exact accepted frame-stage dependency; loose reports or images cannot be adopted")); }
        for ordinal in 0..count {
            let id = &stage_ids[ordinal as usize];
            let frame = frames.get(id).ok_or_else(|| invalid("Upscayl collector frame producer is missing"))?;
            let stage = plan.payload.stages.iter().find(|stage| &stage.id == id).ok_or_else(|| invalid("Upscayl frame stage is missing"))?;
            if frame.settings != operation.settings || frame.frame.frame_index_sha256 != operation.frame_index_sha256
                || stage.outputs.len() != 2
                || !stage.outputs.iter().any(|binding| binding.port == "frame" && binding.artifacts.len() == 1 && binding.artifacts[0].id == frame_ids[ordinal as usize])
                || !stage.outputs.iter().any(|binding| binding.port == "evidence" && binding.artifacts.len() == 1 && binding.artifacts[0].id == report_ids[ordinal as usize])
            { return Err(invalid("Upscayl collector input must be produced by the matching locked frame operation")); }
        }
    }
    Ok(())
}

fn import_request(request: &UpscaylRecoveryRequest) -> FrameImportRequest {
    FrameImportRequest::new(&request.source_video,&request.frames_directory,&request.frame_index,request.configuration.import.clone(),&request.provider_executable)
}

struct Prepared {
    plan: crate::PipelineV3Plan,
    registry: crate::ProviderRegistry,
    bindings: Vec<crate::PipelineInputBinding>,
    frames: usize,
    required_free_bytes: u64,
}

fn prepare(request: &UpscaylRecoveryRequest, cancellation: &CancellationToken) -> Result<Prepared> {
    check_cancelled(cancellation)?;
    request.configuration.validate()?;
    request.execution_limits.validate()?;
    let (index,_) = frame_import::read_index(&request.frame_index)?;
    if index.frames.len() > UPSCAYL_MAXIMUM_FRAMES { return Err(invalid("per-frame recovery admits at most 1024 frames per run")); }
    let admitted = frame_import::prepare_inputs(&import_request(request),cancellation)?;
    let settings = &request.configuration;
    settings.output_dimensions(admitted.index.width,admitted.index.height)?;
    let component_bytes = verify_components(settings,cancellation)?;
    let frame_count = admitted.index.frames.len();
    let metadata_bound = (frame_count as u64 + 1).checked_pow(2).and_then(|value| value.checked_mul(16_384))
        .ok_or_else(|| invalid("Upscayl retained metadata budget overflow"))?;
    let required_free_bytes = (frame_count as u64).checked_mul(settings.maximum_output_frame_bytes)
        .and_then(|value| value.checked_mul(3)).and_then(|value| value.checked_add(metadata_bound))
        .and_then(|value| value.checked_add(component_bytes * 2))
        .and_then(|value| value.checked_add(settings.import.maximum_frame_bytes + settings.maximum_output_frame_bytes))
        .and_then(|value| value.checked_add(4 * UPSCAYL_MAXIMUM_REPORT_BYTES))
        .ok_or_else(|| invalid("Upscayl working-data bound overflow"))?;
    let working = required_free_bytes.checked_add(admitted.source.1).and_then(|value| value.checked_add(admitted.frame_bytes));
    if working.is_none_or(|value| value > settings.maximum_working_bytes) { return Err(invalid("Upscayl candidates, accepted copies, retained manifests and scratch exceed maximum_working_bytes")); }
    let index_digest = canonical_sha256(&admitted.index)?;
    let timeline = admitted.index.temporal_inspection.video_timeline().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    let mut registry = crate::ProviderRegistry::new();
    let mut bindings = vec![crate::PipelineInputBinding::new("source_video",&request.source_video),crate::PipelineInputBinding::new("source_index",&request.frame_index)];
    let mut inputs = vec![json!({"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}),json!({"id":"source_index","artifact_type":"application/vnd.aniflow.rife-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"})];
    let mut stages = Vec::with_capacity(frame_count + 1);
    let mut frame_ids = Vec::with_capacity(frame_count);
    let mut report_ids = Vec::with_capacity(frame_count);
    let mut stage_ids = Vec::with_capacity(frame_count);
    for frame in &admitted.index.frames {
        check_cancelled(cancellation)?;
        let stage_id = format!("upscale_frame_{:06}",frame.ordinal);
        let frame_id = format!("frame_{:06}",frame.ordinal);
        let evidence_id = format!("evidence_{:06}",frame.ordinal);
        let input_id = format!("source_frame_{:06}",frame.ordinal);
        let path = request.frames_directory.join(&frame.filename);
        let observed = identity(&path,settings.import.maximum_frame_bytes)?;
        if observed.0 != frame.sha256 { return Err(invalid("source frame changed during Upscayl planning")); }
        let operation = UpscaylFrameOperation { schema:UPSCAYL_FRAME_OPERATION_SCHEMA_V1.to_owned(),settings:settings.clone(),frame:UpscaylFrameSpec {
            source_sha256:admitted.source.0.clone(),source_byte_count:admitted.source.1,frame_index_sha256:index_digest.clone(),ordinal:frame.ordinal,filename:frame.filename.clone(),source_time:timeline.frames[frame.ordinal as usize].source_time,input_sha256:observed.0,input_byte_count:observed.1,input_width:admitted.index.width,input_height:admitted.index.height,
        } };
        registry.register(crate::ProviderRegistration::new(&stage_id,operation.manifest()?,operation.provider_configuration()?,request.provider_executable.clone(),"aniflow-upscayl-frame-v1",inventory(settings))?)?;
        inputs.push(json!({"id":input_id,"artifact_type":"image/png","artifact_role":"intermediate","stream_role":"video"}));
        bindings.push(crate::PipelineInputBinding::new(&input_id,path));
        stages.push(json!({"id":stage_id,"depends_on":[],"capability":{"id":UPSCAYL_FRAME_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":stage_id},"fallbacks":[]},"inputs":[{"port":"frame","artifacts":[input_id]}],
            "outputs":[{"port":"frame","artifacts":[{"id":frame_id,"relative_path":format!("artifacts/upscayl/{:06}/frame.png",frame.ordinal),"kind":"file"}]},{"port":"evidence","artifacts":[{"id":evidence_id,"relative_path":format!("artifacts/upscayl/{:06}/frame.json",frame.ordinal),"kind":"file"}]}],
            "validations":[{"id":format!("{frame_id}_integrity"),"artifact":frame_id,"contract":"aniflow.validation/artifact-integrity/v1"},{"id":format!("{evidence_id}_integrity"),"artifact":evidence_id,"contract":"aniflow.validation/artifact-integrity/v1"}]}));
        frame_ids.push(frame_id); report_ids.push(evidence_id); stage_ids.push(stage_id);
    }
    let collect = UpscaylCollectOperation { schema:UPSCAYL_COLLECT_OPERATION_SCHEMA_V1.to_owned(),settings:settings.clone(),frame_index_sha256:index_digest,frame_count:frame_count as u64 };
    registry.register(crate::ProviderRegistration::new("collect_upscaled_frames",collect.manifest()?,collect.provider_configuration()?,request.provider_executable.clone(),"aniflow-upscayl-collect-v1",inventory(settings))?)?;
    stages.push(json!({"id":"collect_upscaled_frames","depends_on":stage_ids,"capability":{"id":UPSCAYL_COLLECT_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"collect_upscaled_frames"},"fallbacks":[]},"inputs":[{"port":"index","artifacts":["source_index"]},{"port":"frames","artifacts":frame_ids},{"port":"evidence","artifacts":report_ids}],
        "outputs":[{"port":"frames","artifacts":[{"id":"upscaled_frames","relative_path":"artifacts/upscaled/frames","kind":"directory"}]},{"port":"index","artifacts":[{"id":"upscaled_index","relative_path":"artifacts/upscaled/index.json","kind":"file"}]},{"port":"evidence","artifacts":[{"id":"upscale_evidence","relative_path":"artifacts/upscaled/upscale.json","kind":"file"}]}],
        "validations":[{"id":"upscaled_frames_integrity","artifact":"upscaled_frames","contract":"aniflow.validation/artifact-integrity/v1"},{"id":"upscaled_index_integrity","artifact":"upscaled_index","contract":"aniflow.validation/artifact-integrity/v1"},{"id":"upscale_evidence_integrity","artifact":"upscale_evidence","contract":"aniflow.validation/artifact-integrity/v1"}]}));
    let authored = json!({"schema":"aniflow.pipeline/v3","name":"upscayl-per-frame-recovery","inputs":inputs,"stages":stages,"outputs":[{"id":"upscaled_frames","artifact":"upscaled_frames","required_validations":["upscaled_frames_integrity"]},{"id":"upscaled_index","artifact":"upscaled_index","required_validations":["upscaled_index_integrity"]},{"id":"upscale_evidence","artifact":"upscale_evidence","required_validations":["upscale_evidence_integrity"]}]});
    let pipeline = crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&authored)?).map_err(|failure| Error::new(failure.category(),failure.message))?;
    let mut context = crate::audio_inspection::context();
    // Availability remains unobserved. Optional GPU admission permits an
    // explicitly selected attempt; the actual upstream has no CPU fallback.
    context.allowed_side_effects.push(crate::SideEffect::Gpu);
    let plan = crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&context).map_err(|failure| Error::new(failure.category(),failure.message))?;
    frame_import::bind_prepared_plan(&plan,&admitted)?;
    if canonical_json_bytes(&plan)?.len() as u64 > MAXIMUM_PLAN_BYTES { return Err(invalid("bounded Upscayl plan exceeds 64 MiB")); }
    check_cancelled(cancellation)?;
    Ok(Prepared { plan,registry,bindings,frames:frame_count,required_free_bytes })
}

fn require_storage(path: &Path, required: u64, reserve: u64) -> Result<()> {
    let mut existing = path;
    while !existing.exists() { existing = existing.parent().ok_or_else(|| invalid("storage ancestor is unavailable"))?; }
    let available = crate::cache_v3::available_storage(existing)?;
    if required.checked_add(reserve).is_none_or(|value| value > available) { return Err(Error::new(ErrorCategory::Io,"insufficient available storage for bounded Upscayl recovery; free storage or choose a new smaller run")); }
    Ok(())
}

/// Resolve the bounded fixed pipeline without launching tools or writing a run.
pub fn plan(request: &UpscaylRecoveryRequest, cancellation: &CancellationToken) -> Result<crate::PipelineV3Plan> { Ok(prepare(request,cancellation)?.plan) }

/// Start a fresh run. Every frame has one ordinary accepted stage checkpoint.
pub fn run<F>(request: UpscaylRecoveryRequest, output_directory: Option<PathBuf>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&UpscaylRecoveryProgress) {
    let parent = frame_import::output_parent(&import_request(&request),output_directory)?;
    for input in [&request.configuration.upscayl.executable,&request.configuration.model.param.path,&request.configuration.model.weights.path] {
        if parent.starts_with(input) { return Err(invalid("Upscayl output parent overlaps an input component")); }
    }
    let prepared = prepare(&request,cancellation)?;
    require_storage(&parent,prepared.required_free_bytes,request.configuration.minimum_free_bytes)?;
    let execution = crate::PipelineV3RunRequest::new(prepared.plan,prepared.bindings,prepared.registry).with_output_directory(parent).with_execution_limits(request.execution_limits);
    crate::run_v3_with_progress_and_cancellation(execution,cancellation,progress_observer(prepared.frames,on_progress))
}

/// Recheck all current pins and the exact plan before attempting run-local reuse.
pub fn resume<F>(request: UpscaylRecoveryRequest, run_directory: impl AsRef<Path>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&UpscaylRecoveryProgress) {
    let prepared = prepare(&request,cancellation)?;
    let root = run_directory.as_ref().canonicalize().map_err(|_| invalid("cannot resolve the Upscayl run directory"))?;
    let workspace = crate::PipelineV3Workspace::open_read_only(&root)?;
    let plan_path = workspace.plan();
    let observed = identity(&plan_path,MAXIMUM_PLAN_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(&plan_path).map_err(|_| invalid("cannot open retained Upscayl plan"))?.take(MAXIMUM_PLAN_BYTES+1).read_to_end(&mut bytes).map_err(|_| invalid("cannot read retained Upscayl plan"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed { return Err(invalid("retained Upscayl plan changed while reading")); }
    let retained = crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|failure| Error::new(failure.category(),failure.message))?;
    if retained != prepared.plan { return Err(invalid("Upscayl source, settings or exact provider/model identity changed; start a new run")); }
    // Retained frames are not charged a second complete-run allocation. This
    // checks room for bounded new metadata; each attempted frame and collector
    // independently rechecks its immediate work without trusting loose outputs.
    let next_metadata_bytes = (prepared.frames as u64 + 1) * 65_536;
    require_storage(&root,next_metadata_bytes,request.configuration.minimum_free_bytes)?;
    let execution = crate::PipelineV3ResumeRequest::new(root,prepared.bindings,prepared.registry).with_execution_limits(request.execution_limits);
    crate::resume_v3_with_progress_and_cancellation(execution,cancellation,progress_observer(prepared.frames,on_progress))
}

fn progress_observer<F>(total: usize, mut on_progress: F) -> impl FnMut(&crate::PipelineV3RunProgress)
where F: FnMut(&UpscaylRecoveryProgress) {
    let mut completed = BTreeSet::new();
    let mut reused = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut started = BTreeMap::new();
    let mut measured_milliseconds = 0_u64;
    let mut measured_frames = 0_u64;
    move |event| {
        if let crate::PipelineV3RunProgress::Stage { stage_id,state } = event {
            if stage_id.starts_with("upscale_frame_") {
                use crate::PipelineV3ProgressState as State;
                match state {
                    State::Running => { started.insert(stage_id.clone(),Instant::now()); failed.remove(stage_id); }
                    State::Complete => { completed.insert(stage_id.clone()); if let Some(start) = started.remove(stage_id) { measured_milliseconds = measured_milliseconds.saturating_add(u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)); measured_frames += 1; } }
                    State::Reused => { completed.insert(stage_id.clone()); reused.insert(stage_id.clone()); }
                    State::Invalidated => { completed.remove(stage_id); reused.remove(stage_id); }
                    State::Failed | State::Cancelled => { failed.insert(stage_id.clone()); }
                    State::Validating => {}
                }
            }
        }
        let pending = (total as u64).saturating_sub(completed.len() as u64);
        let estimate = (measured_frames > 0 && failed.is_empty()).then(|| measured_milliseconds.saturating_mul(pending) / measured_frames.max(1));
        on_progress(&UpscaylRecoveryProgress { runtime:event.clone(),frames:UpscaylFrameProgress { total:total as u64,completed:completed.len() as u64,reused:reused.len() as u64,pending,failed:failed.len() as u64,estimated_remaining_milliseconds:estimate } });
    }
}
