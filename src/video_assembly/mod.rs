//! Fixed processed-frame assembly and delivery through Pipeline v3.
//! Imported frame history remains declared; new pixel, clock and packet
//! observations never replace the runtime's validation/acceptance authority.
mod media;
mod provider;
mod types;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use crate::audio_inspection::{AudioInspectionConfiguration, AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1};
use crate::audio_inspection::process::{GroupPolicy, open_regular, verify_pin};
use crate::frame_io::{copy_pinned, ensure_canonical_directory, identity, inventory};
use crate::provider::{canonical_json_bytes, decode_json, require_sha256};
use crate::temporal::{StreamSelection, TemporalInspection};
use crate::upscayl_recovery::{UpscaylOutputIndex, UPSCAYL_OUTPUT_INDEX_SCHEMA_V1};
use crate::video_interpolation::{RifeFrame, RifeFrameIndex, RIFE_FRAME_INDEX_SCHEMA_V1};
use crate::{CancellationToken, Error, ErrorCategory, Result};

pub use media::AssemblyMediaEvidence;
pub use provider::execute_invocation;
pub use types::*;

const MAXIMUM_PLAN_BYTES: u64 = 67_108_864;
pub(crate) const MAXIMUM_TOOL_BYTES: u64 = 134_217_728;
fn invalid(message: impl Into<String>) -> Error { Error::new(ErrorCategory::Input, message) }
fn cancelled(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() { return Err(Error::new(ErrorCategory::Execution, "assembly preflight cancelled")); }
    Ok(())
}

impl AssemblyConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("assembly configuration exceeds 64 KiB")); }
        let value: Self = decode_json(bytes, "assembly configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != ASSEMBLY_CONFIGURATION_SCHEMA_V1
            || self.ffmpeg.version != ASSEMBLY_FFMPEG_VERSION || self.ffprobe.version != ASSEMBLY_FFMPEG_VERSION
            || self.ffmpeg.executable == self.ffprobe.executable
            || !(1..=21_600_000).contains(&self.timeout_milliseconds)
            || !(1024..=67_108_864).contains(&self.maximum_capture_bytes)
            || !(1..=1_099_511_627_776).contains(&self.maximum_source_bytes)
            || !(1024..=1_099_511_627_776).contains(&self.maximum_output_bytes)
            || !(1024..=268_435_456).contains(&self.maximum_frame_bytes)
            || !(1024..=1_099_511_627_776).contains(&self.maximum_total_frame_bytes)
            || !(1..=134_217_728).contains(&self.maximum_pixels)
            || !(1_048_576..=17_592_186_044_416).contains(&self.maximum_working_bytes)
            || self.minimum_free_bytes > 1_099_511_627_776 || self.delivery_crf > 51
            || !matches!(self.preset.as_str(), "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" | "medium" | "slow" | "slower" | "veryslow") {
            return Err(invalid("unsupported assembly profile: exact FFmpeg/FFprobe 8.0.1 and finite media, capture, pixel and delivery bounds are required"));
        }
        self.tool_limits().validate()
    }

    /// Base decoder limits remain within their existing contract. Long native
    /// operations use run_tool_isolated_with_limits with this profile's bounds.
    pub(crate) fn tool_limits(&self) -> AudioInspectionConfiguration {
        AudioInspectionConfiguration {
            schema:AUDIO_INSPECTION_CONFIGURATION_SCHEMA_V1.to_owned(),
            ffmpeg:self.ffmpeg.clone(), ffprobe:self.ffprobe.clone(),
            tool_timeout_milliseconds:self.timeout_milliseconds.min(120_000),
            maximum_tool_output_bytes:(self.maximum_capture_bytes as u64).min(1_048_576),
        }
    }
}

pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:ASSEMBLY_CONFIGURATION_SCHEMA_V1.to_owned(), version:"1.0.0".to_owned(),
        sha256:format!("{:x}", Sha256::digest(include_bytes!("../../providers/video-assembly/configuration.schema.json"))) }
}
pub fn operation_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference { id:ASSEMBLY_OPERATION_SCHEMA_V1.to_owned(), version:"1.0.0".to_owned(),
        sha256:format!("{:x}", Sha256::digest(include_bytes!("../../providers/video-assembly/operation.schema.json"))) }
}

impl AssemblyOperation {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        require_sha256(&self.source_sha256, "assembly source digest")?;
        require_sha256(&self.frame_index_sha256, "assembly frame index digest")?;
        if self.schema != ASSEMBLY_OPERATION_SCHEMA_V1 || self.source_byte_count == 0
            || self.source_byte_count > self.settings.maximum_source_bytes
            || !(1..=100_000).contains(&self.frame_count)
            || !matches!(self.index_schema.as_str(), RIFE_FRAME_INDEX_SCHEMA_V1 | UPSCAYL_OUTPUT_INDEX_SCHEMA_V1)
            || self.width < 2 || self.height < 2 || self.width % 2 != 0 || self.height % 2 != 0
            || u64::from(self.width) * u64::from(self.height) > self.settings.maximum_pixels
            || (self.source_selection.no_audio && self.source_selection.audio_stream.is_some()) {
            return Err(invalid("assembly operation exceeds its frozen source/index/geometry profile"));
        }
        Ok(())
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| invalid(error.to_string()))?;
        let values = value.as_object().ok_or_else(|| invalid("assembly operation must be an object"))?
            .iter().map(|(key,value)| (key.clone(),value.clone())).collect();
        crate::ProviderConfiguration::new(
            crate::ProviderReference { id:ASSEMBLY_PROVIDER_ID.to_owned(), version:"1.0.0".to_owned() },
            crate::CapabilityReference { id:self.stage.capability_id().to_owned(), version:"1.0.0".to_owned() },
            operation_schema_reference(), values,
        )
    }

    pub fn manifest(&self) -> Result<crate::ProviderManifest> {
        self.validate()?;
        let port = |name: &str, artifact_type: &str, role: &str, stream: &str| json!({"name":name,"artifact_type":artifact_type,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let mut inputs = vec![port("source","video/*","temporal_source","video"), port("index","application/vnd.aniflow.assembly-frame-index+json","validation_evidence","timed_metadata")];
        inputs.push(match self.stage {
            AssemblyStageKind::Master => port("frames","application/vnd.aniflow.assembly-frames","intermediate","video"),
            AssemblyStageKind::Delivery => port("master","video/mp4","candidate_master","video"),
        });
        let tools: Vec<_> = [("ffmpeg",&self.settings.ffmpeg),("ffprobe",&self.settings.ffprobe)].into_iter()
            .map(|(id,pin)| json!({"id":id,"version_requirement":format!("={}",pin.version),"sha256":pin.sha256})).collect();
        let schema = operation_schema_reference();
        let manifest = json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":ASSEMBLY_PROVIDER_ID,"version":"1.0.0","display_name":"aniflow verified processed-frame assembly"},"configuration_schemas":[schema.clone()],"capabilities":[{
            "id":self.stage.capability_id(),"version":"1.0.0","kind":"assembler_encoder","configuration_schema":schema,
            "inputs":inputs,"outputs":[port("media","video/mp4","candidate_master","video"),port("evidence","application/vnd.aniflow.video-assembly-report+json","validation_evidence","timed_metadata")],
            "batching":"whole_artifact","max_concurrency":1,
            "requirements":{"tools":tools,"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"environment_dependent","fidelity":if self.stage == AssemblyStageKind::Master {"lossless"} else {"lossy"},"cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},
            "lifecycle":{"progress":"none","cancellation":"process_signal"}}],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&manifest)?)
    }
}

/// A borrowed spatial view over existing wire contracts; never a rewritten
/// source inspection or a new invented processing history.
pub(crate) enum AssemblyFrameIndex { Original(RifeFrameIndex), Upscaled(UpscaylOutputIndex) }
impl AssemblyFrameIndex {
    pub(crate) fn frames(&self) -> &[RifeFrame] { match self { Self::Original(value) => &value.frames, Self::Upscaled(value) => &value.frames } }
    pub(crate) fn temporal(&self) -> &TemporalInspection { match self { Self::Original(value) => &value.temporal_inspection, Self::Upscaled(value) => &value.source_index.temporal_inspection } }
    pub(crate) fn dimensions(&self) -> (u32,u32) { match self { Self::Original(value) => (value.width,value.height), Self::Upscaled(value) => (value.output_width,value.output_height) } }
    pub(crate) fn schema(&self) -> &'static str { match self { Self::Original(_) => RIFE_FRAME_INDEX_SCHEMA_V1, Self::Upscaled(_) => UPSCAYL_OUTPUT_INDEX_SCHEMA_V1 } }
}

pub(crate) fn read_index(path: &Path) -> Result<(AssemblyFrameIndex,(String,u64))> {
    let observed = identity(path,ASSEMBLY_MAXIMUM_INDEX_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(path).map_err(|_| invalid("cannot safely open assembly index"))?.take(ASSEMBLY_MAXIMUM_INDEX_BYTES+1)
        .read_to_end(&mut bytes).map_err(|_| invalid("cannot read assembly index"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed { return Err(invalid("assembly index changed while reading")); }
    let header: Value = decode_json(&bytes,"assembly index")?;
    let index = match header.get("schema").and_then(Value::as_str) {
        Some(RIFE_FRAME_INDEX_SCHEMA_V1) => AssemblyFrameIndex::Original(RifeFrameIndex::from_json_slice(&bytes)?),
        Some(UPSCAYL_OUTPUT_INDEX_SCHEMA_V1) => {
            let value: UpscaylOutputIndex = decode_json(&bytes,"Upscayl output index")?;
            value.validate()?;
            AssemblyFrameIndex::Upscaled(value)
        },
        _ => return Err(invalid("assembly accepts only original or Upscayl unchanged-clock frame indices; derived interpolation clocks are unsupported")),
    };
    Ok((index,observed))
}

pub(crate) fn admit_index(settings: &AssemblyConfiguration, index: &AssemblyFrameIndex) -> Result<()> {
    settings.validate()?;
    index.temporal().require_processing().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    let (width,height) = index.dimensions();
    if index.frames().is_empty() || index.frames().len() > 100_000 || width < 2 || height < 2 || width % 2 != 0 || height % 2 != 0
        || u64::from(width) * u64::from(height) > settings.maximum_pixels {
        return Err(invalid("assembly needs 1..100000 indexed frames with even supported dimensions for H264 delivery"));
    }
    Ok(())
}

pub(crate) fn observe_components(settings: &AssemblyConfiguration) -> Result<[(String,u64);2]> {
    settings.validate()?;
    let mut observed = [(String::new(),0),(String::new(),0)];
    for ((id,pin),value) in [("ffmpeg",&settings.ffmpeg),("ffprobe",&settings.ffprobe)].into_iter().zip(&mut observed) {
        verify_pin(pin,id).map_err(|_| Error::new(ErrorCategory::Dependency,"assembly executable is unavailable or differs from its exact pin"))?;
        *value = identity(&pin.executable,MAXIMUM_TOOL_BYTES)?;
        if value.0 != pin.sha256 { return Err(Error::new(ErrorCategory::Dependency,"assembly tool changed during observation")); }
    }
    Ok(observed)
}

/// Observe the assembly source profile before a recipe spends work on frames.
/// This is an admission observation, never a checkpoint or backend qualification.
/// The caller holds ownership of the canonical recipe workspace. Each native
/// process owns its group because this call runs outside a provider invocation.
pub(crate) fn admit_source(settings: &AssemblyConfiguration, source_video: &Path,
    expected: &TemporalInspection, workspace: &Path, cancellation: &CancellationToken) -> Result<()> {
    cancelled(cancellation)?;
    settings.validate()?;
    if !cfg!(unix) { return Err(Error::new(ErrorCategory::Dependency,"assembly source admission requires Unix process-group cancellation")); }
    expected.require_processing().map_err(|error| Error::from_anyhow(ErrorCategory::Media,error))?;
    require_sha256(&expected.source_sha256,"assembly admission source digest")?;
    ensure_canonical_directory(workspace)?;
    for input in [source_video,&settings.ffmpeg.executable,&settings.ffprobe.executable] {
        if input.canonicalize().ok().as_deref() != Some(input)
            || workspace.starts_with(input) || input.starts_with(workspace) {
            return Err(invalid("assembly source admission needs canonical inputs outside its owned workspace"));
        }
    }
    let source = identity(source_video,settings.maximum_source_bytes)?;
    if source.1 == 0 || source != (expected.source_sha256.clone(),expected.source_size_bytes) {
        return Err(invalid("assembly source admission differs from the original indexed source"));
    }
    let components = observe_components(settings)?;
    let tool_bytes = components[0].1.checked_add(components[1].1).ok_or_else(|| invalid("assembly source tool-byte bound overflow"))?;
    let required = source.1.checked_add(tool_bytes)
        .and_then(|bytes| bytes.checked_add((settings.maximum_capture_bytes as u64).checked_mul(12)?))
        .and_then(|bytes| bytes.checked_add(65_536))
        .ok_or_else(|| invalid("assembly source admission storage bound overflow"))?;
    let working = required.checked_add(source.1).and_then(|bytes| bytes.checked_add(tool_bytes));
    if working.is_none_or(|bytes| bytes > settings.maximum_working_bytes) {
        return Err(invalid("assembly source snapshots and observations exceed maximum_working_bytes"));
    }
    require_storage(workspace,required,settings.minimum_free_bytes)?;
    cancelled(cancellation)?;
    let private = tempfile::Builder::new().prefix(".assembly-source-").tempdir_in(workspace)
        .map_err(|_| Error::new(ErrorCategory::Io,"cannot reserve private assembly source admission work"))?;
    let root = private.path().to_path_buf();
    let observed: Result<()> = (|| {
        ensure_canonical_directory(&root)?;
        let source_copy = root.join("source.media");
        let mut private_settings = settings.clone();
        private_settings.ffmpeg.executable = root.join("ffmpeg");
        private_settings.ffprobe.executable = root.join("ffprobe");
        for (original,copy,expected) in [
            (&settings.ffmpeg.executable,&private_settings.ffmpeg.executable,&components[0]),
            (&settings.ffprobe.executable,&private_settings.ffprobe.executable,&components[1]),
        ] {
            copy_pinned(original,copy,expected,MAXIMUM_TOOL_BYTES,cancellation)?;
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(copy,std::fs::Permissions::from_mode(0o700))
                    .map_err(|_| Error::new(ErrorCategory::Io,"cannot make a private assembly admission tool executable"))?;
            }
        }
        copy_pinned(source_video,&source_copy,&source,settings.maximum_source_bytes,cancellation)?;
        if observe_components(&private_settings)? != components {
            return Err(invalid("private assembly admission components differ from their pins"));
        }
        let admission = media::admit_source(&private_settings,&source_copy,expected,&root,cancellation,GroupPolicy::Own);
        // Recheck even when native admission refuses the source. No observation
        // can authorize changing the original or its exact private snapshots.
        if identity(source_video,settings.maximum_source_bytes)? != source
            || identity(&source_copy,settings.maximum_source_bytes)? != source
            || observe_components(settings)? != components
            || observe_components(&private_settings)? != components {
            return Err(invalid("assembly source or tool identity changed during source admission"));
        }
        admission?;
        cancelled(cancellation)
    })();
    match observed {
        Ok(()) => private.close().map_err(|_| Error::new(ErrorCategory::Io,"cannot clean successful private assembly source admission work")),
        Err(error) => {
            let retained = private.keep();
            let message: String = error.message().chars().take(4096).collect();
            let failure = json!({"schema":"aniflow.video-assembly-source-admission-failure/v1",
                "category":error.category(),"message":message,"message_truncated":error.message().chars().nth(4096).is_some(),
                "cause":"unproven","accepted":false,
                "source_sha256":source.0,"source_byte_count":source.1});
            // Retaining snapshots remains useful if storage failure prevents
            // writing this additional, explicitly non-authoritative note.
            if let Ok(bytes) = canonical_json_bytes(&failure) {
                if let Ok(mut file) = std::fs::OpenOptions::new().create_new(true).write(true).open(retained.join("failure.json")) {
                    let _ = file.write_all(&bytes).and_then(|()| file.sync_all());
                }
            }
            Err(error)
        }
    }
}

pub(crate) fn checked_working_bytes(settings: &AssemblyConfiguration, source_bytes: u64, frame_bytes: u64, tool_bytes: u64) -> Result<u64> {
    // Includes stage candidates, accepted copies, snapshots used by native
    // validation, retained reports and bounded capture buffers. These are
    // admission bounds, not operating-system reservations.
    let terms = [(source_bytes,3),(frame_bytes,2),(settings.maximum_output_bytes,6),(tool_bytes,2),
        (ASSEMBLY_MAXIMUM_REPORT_BYTES,8),(settings.maximum_capture_bytes as u64,12)];
    let bytes = terms.into_iter().try_fold(0_u64,|total,(value,count)| {
        value.checked_mul(count).and_then(|value| total.checked_add(value)).ok_or_else(|| invalid("assembly working-data bound overflow"))
    })?;
    if bytes > settings.maximum_working_bytes { return Err(invalid("assembly candidates, accepted copies, snapshots and observations exceed maximum_working_bytes")); }
    Ok(bytes)
}

pub(crate) fn require_storage(path: &Path, required: u64, reserve: u64) -> Result<()> {
    let mut existing = path;
    while !existing.exists() { existing = existing.parent().ok_or_else(|| invalid("assembly storage ancestor is unavailable"))?; }
    let available = crate::cache_v3::available_storage(existing)?;
    if required.checked_add(reserve).is_none_or(|value| value > available) { return Err(Error::new(ErrorCategory::Io,"insufficient available storage for bounded assembly; free owned workspace space or reduce the explicit bounds before resuming")); }
    Ok(())
}

fn component_inventory(settings: &AssemblyConfiguration) -> crate::ComponentInventory {
    crate::ComponentInventory { tools:[("ffmpeg",&settings.ffmpeg),("ffprobe",&settings.ffprobe)].into_iter()
        .map(|(id,pin)| crate::ComponentIdentity { id:id.to_owned(),version:pin.version.clone(),sha256:Some(pin.sha256.clone()) }).collect(),
        codecs:Vec::new(),models:Vec::new() }
}

fn native_validation_settings(settings: &AssemblyConfiguration) -> crate::validation::native::NativeValidationConfiguration {
    let pin = |pin: &crate::audio_inspection::AudioToolPin| crate::validation::native::ValidationToolPin { path:pin.executable.clone(),version:pin.version.clone(),sha256:pin.sha256.clone() };
    crate::validation::native::NativeValidationConfiguration {
        ffmpeg:pin(&settings.ffmpeg),ffprobe:pin(&settings.ffprobe),
        maximum_media_bytes:settings.maximum_source_bytes.max(settings.maximum_output_bytes),
        decode_timeout_seconds:settings.timeout_milliseconds.div_ceil(1000).min(3600),
    }
}

fn require_validator_storage(settings: &AssemblyConfiguration) -> Result<()> {
    // The existing validator owns temporary snapshots outside this run root.
    // Observe that volume independently; neither check reserves filesystem space.
    let temporary = std::env::temp_dir().canonicalize().map_err(|_| invalid("native validator temporary directory is unavailable"))?;
    ensure_canonical_directory(&temporary)?;
    let required = settings.maximum_source_bytes.max(settings.maximum_output_bytes).checked_mul(2)
        .and_then(|bytes| bytes.checked_add(3 * ASSEMBLY_MAXIMUM_REPORT_BYTES))
        .ok_or_else(|| invalid("native validator temporary storage bound overflow"))?;
    require_storage(&temporary,required,settings.minimum_free_bytes)
}

fn stage_document(operation: &AssemblyOperation) -> Value {
    let kind = operation.stage;
    let media_id = kind.artifact_id();
    let evidence_id = if kind == AssemblyStageKind::Master { "master_evidence" } else { "delivery_evidence" };
    let directory = if kind == AssemblyStageKind::Master { "master" } else { "delivery" };
    let mut inputs = vec![json!({"port":"source","artifacts":["source_video"]}),json!({"port":"index","artifacts":["source_index"]})];
    inputs.push(if kind == AssemblyStageKind::Master { json!({"port":"frames","artifacts":["source_frames"]}) }
        else { json!({"port":"master","artifacts":["lossless_master"]}) });
    let depends_on = if kind == AssemblyStageKind::Master { Vec::<&str>::new() } else { vec![AssemblyStageKind::Master.stage_id()] };
    json!({"id":kind.stage_id(),"depends_on":depends_on,
        "capability":{"id":kind.capability_id(),"version_requirement":"=1.0.0"},
        "provider":{"primary":{"registration_id":kind.stage_id()},"fallbacks":[]},"inputs":inputs,
        "outputs":[{"port":"media","artifacts":[{"id":media_id,"relative_path":format!("artifacts/assembly/{directory}/{directory}.mp4"),"kind":"file"}]},
            {"port":"evidence","artifacts":[{"id":evidence_id,"relative_path":format!("artifacts/assembly/{directory}/{directory}.json"),"kind":"file"}]}],
        "validations":[{"id":format!("{media_id}_temporal"),"artifact":media_id,"contract":crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1,
            "validator":{"capability":{"id":crate::validation::native::NATIVE_VALIDATION_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"assembly_temporal_validator"},"fallbacks":[]}},
            "temporal":{"source_artifact":"source_video","source_selection":operation.source_selection,
                "artifact_selection":StreamSelection { video_stream:Some(0),audio_stream:None,no_audio:false,discard_streams:Vec::new() },"allow_authored_subtitles":false}},
            {"id":format!("{evidence_id}_integrity"),"artifact":evidence_id,"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]})
}

fn authored_pipeline(master: &AssemblyOperation) -> Result<crate::PipelineV3Configuration> {
    let mut delivery = master.clone(); delivery.stage = AssemblyStageKind::Delivery;
    let value = json!({"schema":"aniflow.pipeline/v3","name":"verified-processed-frame-assembly",
        "inputs":[{"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"},
            {"id":"source_index","artifact_type":"application/vnd.aniflow.assembly-frame-index+json","artifact_role":"validation_evidence","stream_role":"timed_metadata"},
            {"id":"source_frames","artifact_type":"application/vnd.aniflow.assembly-frames","artifact_role":"intermediate","stream_role":"video"}],
        "stages":[stage_document(master),stage_document(&delivery)],
        "outputs":[{"id":"lossless_master","artifact":"lossless_master","required_validations":["lossless_master_temporal"]},
            {"id":"h264_delivery","artifact":"h264_delivery","required_validations":["h264_delivery_temporal"]},
            {"id":"master_evidence","artifact":"master_evidence","required_validations":["master_evidence_integrity"]},
            {"id":"delivery_evidence","artifact":"delivery_evidence","required_validations":["delivery_evidence_integrity"]}]});
    crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|failure| Error::new(failure.category(),failure.message))
}

struct Prepared {
    plan: crate::PipelineV3Plan,
    bindings: Vec<crate::PipelineInputBinding>,
    registry: crate::ProviderRegistry,
    required_free_bytes: u64,
}

fn prepare(request: &AssemblyRequest, cancellation: &CancellationToken) -> Result<Prepared> {
    cancelled(cancellation)?;
    request.configuration.validate()?;
    let (index,index_identity) = read_index(&request.frame_index)?;
    admit_index(&request.configuration,&index)?;
    ensure_canonical_directory(&request.frames_directory)?;
    let names: Vec<OsString> = index.frames().iter().map(|frame| frame.filename.clone().into()).collect();
    inventory(&request.frames_directory,&names)?;
    let mut frame_bytes = 0_u64;
    for frame in index.frames() {
        cancelled(cancellation)?;
        let observed = identity(&request.frames_directory.join(&frame.filename),request.configuration.maximum_frame_bytes)?;
        if observed.0 != frame.sha256 { return Err(invalid("assembly PNG bytes differ from their indexed digest")); }
        frame_bytes = frame_bytes.checked_add(observed.1).filter(|value| *value <= request.configuration.maximum_total_frame_bytes)
            .ok_or_else(|| invalid("assembly frame directory exceeds maximum_total_frame_bytes"))?;
    }
    let source = identity(&request.source_video,request.configuration.maximum_source_bytes)?;
    if source.0 != index.temporal().source_sha256 || source.1 != index.temporal().source_size_bytes || source.1 == 0 {
        return Err(invalid("assembly source media differs from the index's immutable temporal source"));
    }
    let components = observe_components(&request.configuration)?;
    let required_free_bytes = checked_working_bytes(&request.configuration,source.1,frame_bytes,components[0].1+components[1].1)?;
    let (width,height) = index.dimensions();
    let master = AssemblyOperation {
        schema:ASSEMBLY_OPERATION_SCHEMA_V1.to_owned(),stage:AssemblyStageKind::Master,settings:request.configuration.clone(),
        source_sha256:source.0.clone(),source_byte_count:source.1,source_selection:index.temporal().selection_intent.clone(),
        frame_index_sha256:index_identity.0.clone(),index_schema:index.schema().to_owned(),frame_count:index.frames().len() as u64,width,height,
    };
    let pipeline = authored_pipeline(&master)?;
    let mut registry = crate::ProviderRegistry::new();
    for kind in [AssemblyStageKind::Master,AssemblyStageKind::Delivery] {
        let mut operation = master.clone(); operation.stage = kind;
        registry.register(crate::ProviderRegistration::new(kind.stage_id(),operation.manifest()?,operation.provider_configuration()?,
            request.provider_executable.clone(),"aniflow-video-assembly-v1",component_inventory(&request.configuration))?)?;
    }
    registry.register(crate::validation::native::registration("assembly_temporal_validator",request.provider_executable.clone(),
        native_validation_settings(&request.configuration),"video/mp4","video/*")?)?;
    let bindings = vec![crate::PipelineInputBinding::new("source_video",&request.source_video),crate::PipelineInputBinding::new("source_index",&request.frame_index),
        crate::PipelineInputBinding::new("source_frames",&request.frames_directory)];
    let plan = crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&crate::audio_inspection::context())
        .map_err(|failure| Error::new(failure.category(),failure.message))?;
    for (id,expected) in [("source_video",&source),("source_index",&index_identity)] {
        if !plan.payload.inputs.iter().any(|input| input.id == id && input.content_sha256 == expected.0 && input.byte_count == expected.1) {
            return Err(invalid("assembly source or index changed between admission and planning"));
        }
    }
    if canonical_json_bytes(&plan)?.len() as u64 > MAXIMUM_PLAN_BYTES { return Err(invalid("assembly plan exceeds 64 MiB")); }
    cancelled(cancellation)?;
    Ok(Prepared { plan,bindings,registry,required_free_bytes })
}

/// Exact component and topology admission also applies to generic v3 entry
/// points, so a hand-authored plan cannot turn this facade into arbitrary argv.
pub(crate) fn preflight_plan(plan: &crate::PipelineV3Plan, registry: &crate::ProviderRegistry, cancellation: &CancellationToken) -> Result<()> {
    let stages: Vec<_> = plan.payload.stages.iter().filter(|stage| matches!(stage.capability.id.as_str(),ASSEMBLY_MASTER_CAPABILITY_ID|ASSEMBLY_DELIVERY_CAPABILITY_ID)).collect();
    if stages.is_empty() { return Ok(()); }
    if stages.len() != 2 || plan.payload.stages.len() != 2 { return Err(invalid("assembly requires its fixed master followed by delivery topology")); }
    let mut operations = Vec::new();
    for stage in &stages {
        cancelled(cancellation)?;
        let registration = registry.registration(&stage.provider_lock.payload.registration_id).ok_or_else(|| invalid("assembly registration is unavailable"))?;
        let operation: AssemblyOperation = serde_json::from_value(serde_json::to_value(&registration.configuration().values).map_err(|_| invalid("invalid assembly operation"))?)
            .map_err(|_| invalid("invalid closed assembly operation"))?;
        operation.validate()?;
        if stage.id != operation.stage.stage_id() || stage.capability.id != operation.stage.capability_id() || stage.capability.version != "1.0.0"
            || stage.provider_lock.payload.registration_id != stage.id || operation.provider_configuration()? != *registration.configuration()
            || operation.manifest()? != *registration.manifest() || component_inventory(&operation.settings) != *registration.components()
            || registration.implementation_id() != "aniflow-video-assembly-v1"
            || registration.configuration().effective_configuration_sha256 != stage.provider_lock.payload.effective_configuration_sha256 {
            return Err(invalid("assembly stage differs from its exact frozen provider operation"));
        }
        observe_components(&operation.settings)?;
        operations.push(operation);
    }
    let master = operations.iter().find(|operation| operation.stage == AssemblyStageKind::Master).ok_or_else(|| invalid("assembly master stage is missing"))?;
    let master_registration = registry.registration(AssemblyStageKind::Master.stage_id()).ok_or_else(|| invalid("assembly master registration is missing"))?;
    let delivery_registration = registry.registration(AssemblyStageKind::Delivery.stage_id()).ok_or_else(|| invalid("assembly delivery registration is missing"))?;
    if master_registration.executable() != delivery_registration.executable() { return Err(invalid("assembly stages must use the same exact provider executable")); }
    let mut expected_delivery = master.clone(); expected_delivery.stage = AssemblyStageKind::Delivery;
    if operations.iter().filter(|operation| **operation == expected_delivery).count() != 1 { return Err(invalid("assembly delivery must use the same exact source, index, geometry and settings as its master")); }
    let expected = authored_pipeline(master)?;
    if plan.payload.pipeline_name != expected.name || plan.payload.outputs != expected.outputs || plan.payload.inputs.len() != expected.inputs.len() {
        return Err(invalid("assembly plan output/input set differs from its fixed profile"));
    }
    for authored in &expected.inputs {
        if !plan.payload.inputs.iter().any(|input| input.id == authored.id && input.artifact_type == authored.artifact_type
            && input.artifact_role == authored.artifact_role && input.stream_role == authored.stream_role) {
            return Err(invalid("assembly input contracts differ from their fixed profile"));
        }
    }
    for (id,digest,bytes) in [("source_video",master.source_sha256.as_str(),Some(master.source_byte_count)),("source_index",master.frame_index_sha256.as_str(),None)] {
        if !plan.payload.inputs.iter().any(|input| input.id == id && input.content_sha256 == digest && bytes.is_none_or(|bytes| input.byte_count == bytes)) {
            return Err(invalid("assembly source/index operation does not match planned immutable inputs"));
        }
    }
    for authored in &expected.stages {
        let stage = stages.iter().find(|stage| stage.id == authored.id).ok_or_else(|| invalid("assembly fixed stage is missing"))?;
        if stage.depends_on != authored.depends_on || stage.inputs != authored.inputs || stage.provider_selection != authored.provider
            || stage.capability_requirement != authored.capability || stage.validations != authored.validations || stage.outputs.len() != authored.outputs.len()
            || stage.validation_providers.len() != 1 {
            return Err(invalid("assembly stage inputs, dependencies or validation obligations differ from the fixed topology"));
        }
        for (actual,expected) in stage.outputs.iter().zip(&authored.outputs) {
            if actual.port != expected.port || actual.artifacts.len() != expected.artifacts.len()
                || actual.artifacts.iter().zip(&expected.artifacts).any(|(actual,expected)| actual.id != expected.id || actual.relative_path != expected.relative_path || actual.kind != expected.kind) {
                return Err(invalid("assembly output identity or destination differs from its fixed profile"));
            }
        }
        let gate = &stage.validation_providers[0].stage;
        let gate_registration = registry.registration(&gate.provider_lock.payload.registration_id).ok_or_else(|| invalid("assembly validator registration is unavailable"))?;
        let expected_gate = crate::validation::native::registration("assembly_temporal_validator",master_registration.executable(),
            native_validation_settings(&master.settings),"video/mp4","video/*")?;
        if gate_registration.configuration() != expected_gate.configuration() || gate_registration.manifest() != expected_gate.manifest()
            || gate_registration.components() != expected_gate.components() || gate_registration.implementation_id() != expected_gate.implementation_id()
            || gate_registration.executable() != expected_gate.executable()
            || gate_registration.configuration().effective_configuration_sha256 != gate.provider_lock.payload.effective_configuration_sha256 {
            return Err(invalid("assembly requires its exact native temporal validator identity, executable, tool pins and media bounds"));
        }
    }
    Ok(())
}

fn output_parent(request: &AssemblyRequest, requested: Option<PathBuf>) -> Result<PathBuf> {
    let parent = requested.unwrap_or_else(|| PathBuf::from(".aniflow/runs"));
    let parent = if parent.is_absolute() { parent } else { std::env::current_dir().map_err(|_| invalid("cannot resolve assembly output parent"))?.join(parent) };
    if parent.components().any(|component| matches!(component,Component::ParentDir|Component::CurDir)) { return Err(invalid("assembly output parent must be normalized without traversal")); }
    for input in [&request.source_video,&request.frame_index,&request.frames_directory,&request.configuration.ffmpeg.executable,&request.configuration.ffprobe.executable,&request.provider_executable] {
        if parent.starts_with(input) { return Err(invalid("assembly output parent overlaps an input or executable")); }
    }
    let mut existing = parent.as_path();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(metadata) if metadata.is_dir() && existing.canonicalize().ok().as_deref() == Some(existing) => break,
            Ok(_) => return Err(invalid("assembly output ancestors must be nonsymlink directories")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => existing = existing.parent().ok_or_else(|| invalid("assembly output has no existing ancestor"))?,
            Err(_) => return Err(invalid("cannot inspect assembly output parent")),
        }
    }
    Ok(parent)
}

/// Resolve exact identities and two stages without tools or workspace writes.
pub fn plan(request: &AssemblyRequest, cancellation: &CancellationToken) -> Result<crate::PipelineV3Plan> { Ok(prepare(request,cancellation)?.plan) }

pub fn run<F>(request: AssemblyRequest, output_directory: Option<PathBuf>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    run_new(request,output_directory,None,None,cancellation,on_progress)
}

/// Fixed-recipe child creation through the same native and temporal authority.
pub(crate) fn run_at<F>(request: AssemblyRequest, run_directory: impl AsRef<Path>, expected_plan: &crate::PipelineV3Plan, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let directory = run_directory.as_ref();
    let parent = crate::run_v3::exact_run_parent(directory)?;
    run_new(request,Some(parent),Some(directory),Some(expected_plan),cancellation,on_progress)
}

fn run_new<F>(request: AssemblyRequest, output_directory: Option<PathBuf>, exact_directory: Option<&Path>, expected_plan: Option<&crate::PipelineV3Plan>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let parent = output_parent(&request,output_directory)?;
    let prepared = prepare(&request,cancellation)?;
    if expected_plan.is_some_and(|expected| expected != &prepared.plan) {
        return Err(Error::new(ErrorCategory::State,"assembly plan changed after the recipe handoff; no child run was created"));
    }
    require_storage(&parent,prepared.required_free_bytes,request.configuration.minimum_free_bytes)?;
    require_validator_storage(&request.configuration)?;
    let execution = crate::PipelineV3RunRequest::new(prepared.plan,prepared.bindings,prepared.registry).with_output_directory(parent).with_execution_limits(request.execution_limits);
    match exact_directory {
        Some(directory) => crate::run_v3::run_v3_at_with_progress_and_cancellation(execution,directory,cancellation,on_progress),
        None => crate::run_v3_with_progress_and_cancellation(execution,cancellation,on_progress),
    }
}

pub fn resume<F>(request: AssemblyRequest, run_directory: impl AsRef<Path>, cancellation: &CancellationToken, on_progress: F) -> Result<crate::PipelineV3RunOutcome>
where F: FnMut(&crate::PipelineV3RunProgress) {
    let prepared = prepare(&request,cancellation)?;
    let root = run_directory.as_ref().canonicalize().map_err(|_| invalid("cannot resolve assembly run directory"))?;
    let workspace = crate::PipelineV3Workspace::open_read_only(&root)?;
    let path = workspace.plan();
    let observed = identity(&path,MAXIMUM_PLAN_BYTES)?;
    let mut bytes = Vec::new();
    open_regular(&path).map_err(|_| invalid("cannot safely open retained assembly plan"))?.take(MAXIMUM_PLAN_BYTES+1).read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot read retained assembly plan"))?;
    if (format!("{:x}",Sha256::digest(&bytes)),bytes.len() as u64) != observed { return Err(invalid("retained assembly plan changed while reading")); }
    let retained = crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|failure| Error::new(failure.category(),failure.message))?;
    if retained != prepared.plan { return Err(invalid("assembly source, index, frames, tool pins or configuration changed; start a new run")); }
    // Each attempted stage separately checks its immediate allocation; accepted
    // stages are reused without charging another full run's output allocation.
    require_storage(&root,1_048_576,request.configuration.minimum_free_bytes)?;
    let execution = crate::PipelineV3ResumeRequest::new(root,prepared.bindings,prepared.registry).with_execution_limits(request.execution_limits);
    crate::resume_v3_with_progress_and_cancellation(execution,cancellation,on_progress)
}
