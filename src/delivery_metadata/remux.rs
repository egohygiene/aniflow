//! One fixed metadata-only native stage using existing Pipeline v3 authority.
use std::{fs::OpenOptions,io::{Read,Write},path::{Path,PathBuf}};
use serde::{Deserialize,Serialize};
use serde_json::json;
use sha2::{Digest,Sha256};
use crate::{CancellationToken,Result};
use crate::frame_io::{identity,copy_pinned,ensure_canonical_directory,ensure_parent,require_absent};
use crate::provider::canonical_json_bytes;
use crate::video_assembly::AssemblyConfiguration;
pub use crate::video_assembly::media::{MetadataRemuxMedia,MetadataRemuxPlayback};
use crate::temporal::StreamSelection;
use super::invalid;
mod subtitle;
pub use subtitle::{SubtitlePreservation,PreservedSubtitleEvidence};
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.metadata-remux";
pub const CAPABILITY_ID:&str="aniflow/metadata-remux";
const SCHEMA:&str="aniflow.metadata-remux-operation/v1";
const TOOL_BYTES:u64=134_217_728;
const DOCUMENT_BYTES:u64=67_108_864;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemuxOperation {
    pub schema:String,pub settings:AssemblyConfiguration,pub settings_schema_sha256:String,
    pub source_sha256:String,pub source_byte_count:u64,pub selection:StreamSelection,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub subtitle_preservation:Option<SubtitlePreservation>,
}
impl RemuxOperation {
    pub fn validate(&self)->Result<()> {
        self.settings.validate()?;crate::provider::require_sha256(&self.source_sha256,"remux source")?;
        if self.schema!=(if self.subtitle_preservation.is_some(){"aniflow.metadata-remux-operation/v2"}else{SCHEMA})
            ||self.settings.delivery_metadata.is_none()
            ||self.settings_schema_sha256!=crate::video_assembly::configuration_schema_reference().sha256
            ||self.source_byte_count==0||self.source_byte_count>self.settings.maximum_source_bytes
            ||self.selection.video_stream.is_none()||self.selection.discard_streams!=self.subtitle_preservation.as_ref().map(|s|vec![s.stream_index]).unwrap_or_default()
            ||(self.selection.no_audio&&self.selection.audio_stream.is_some()) {return Err(invalid("remux requires its exact closed policy, source and explicit lossless stream selection"));}
        if let Some(binding)=&self.subtitle_preservation {
            binding.validate()?;let artifact=binding.artifact()?;
            if artifact.sha256!=self.source_sha256||artifact.byte_count!=self.source_byte_count
                ||self.selection.video_stream!=Some(0)||self.selection.audio_stream!=artifact.audio.as_ref().map(|_|1)
                ||self.selection.no_audio!=artifact.audio.is_none()||self.selection.audio_extent!=Some(crate::temporal::AudioExtentPolicy::PreserveExact) {
                return Err(invalid("subtitle remux must bind exactly the accepted native caption video/audio/subtitle membership"));
            }
        }Ok(())
    }
    fn schema(&self)->crate::ConfigurationSchemaReference {
        let (id,version,bytes):(&str,&str,&[u8])=if self.subtitle_preservation.is_some(){("aniflow.metadata-remux-operation/v2","2.0.0",include_bytes!("../../docs/contracts/metadata-remux-operation-v2.schema.json"))}
            else{(SCHEMA,"1.0.0",include_bytes!("../../docs/contracts/metadata-remux-operation-v1.schema.json"))};
        crate::ConfigurationSchemaReference {id:id.into(),version:version.into(),sha256:format!("{:x}",Sha256::digest(bytes))}
    }
    pub fn provider_configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let value=serde_json::to_value(self).map_err(|_|invalid("cannot encode remux operation"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference{id:PROVIDER_ID.into(),version:"1.0.0".into()},
            crate::CapabilityReference{id:CAPABILITY_ID.into(),version:"1.0.0".into()},self.schema(),
            value.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    pub fn manifest(&self)->Result<crate::ProviderManifest> {
        self.validate()?;let port=|name:&str,kind:&str,role:&str,stream:&str|json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let value=json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow metadata-only MP4 remux"},"configuration_schemas":[self.schema()],"capabilities":[{
            "id":CAPABILITY_ID,"version":"1.0.0","kind":"assembler_encoder","configuration_schema":self.schema(),
            "inputs":[port("source","video/*","temporal_source","video")],
            "outputs":[port("media","video/mp4","candidate_master","video"),port("evidence","application/vnd.aniflow.metadata-remux-report+json","validation_evidence","timed_metadata")],
            "batching":"whole_artifact","max_concurrency":1,
            "requirements":{"tools":crate::video_assembly::component_inventory(&self.settings).tools.iter().map(|p|json!({"id":p.id,"version_requirement":format!("={}",p.version),"sha256":p.sha256})).collect::<Vec<_>>(),"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"environment_dependent","fidelity":"lossless","cacheable":false,"content_changes":false,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},
            "lifecycle":{"progress":"none","cancellation":"process_signal"}}],"provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
    fn working_bytes(&self)->Result<u64> {
        let tools=crate::video_assembly::observe_components(&self.settings)?;
        let bytes=self.source_byte_count.checked_mul(3).and_then(|v|v.checked_add(self.settings.maximum_output_bytes.checked_mul(3)?))
            .and_then(|v|v.checked_add((tools[0].1+tools[1].1).checked_mul(2)?))
            .and_then(|v|v.checked_add((self.settings.maximum_capture_bytes as u64).checked_mul(16)?))
            .and_then(|v|v.checked_add(DOCUMENT_BYTES*4)).ok_or_else(||invalid("remux working bound overflow"))?;
        if bytes>self.settings.maximum_working_bytes{return Err(invalid("remux snapshots/candidates exceed maximum_working_bytes"));}Ok(bytes)
    }
}
#[derive(Debug,Clone)]
pub struct RemuxRequest {
    pub source_video:PathBuf,pub selection:StreamSelection,pub settings:AssemblyConfiguration,
    pub provider_executable:PathBuf,pub execution_limits:crate::ProviderExecutionLimits,
    pub subtitle_preservation:Option<SubtitlePreservation>,
}
impl RemuxRequest {
    pub fn new(source_video:PathBuf,selection:StreamSelection,settings:AssemblyConfiguration,provider_executable:PathBuf)->Self {
        let execution_limits=crate::ProviderExecutionLimits {timeout:std::time::Duration::from_secs(86_400),termination_grace_period:std::time::Duration::from_secs(2),
            maximum_stdout_bytes:1_048_576,maximum_stderr_bytes:1_048_576,maximum_artifact_files:128,maximum_artifact_bytes:settings.maximum_working_bytes};
        Self {source_video,selection,settings,provider_executable,execution_limits,subtitle_preservation:None}
    }
}
struct Prepared {plan:crate::PipelineV3Plan,bindings:Vec<crate::PipelineInputBinding>,registry:crate::ProviderRegistry,operation:RemuxOperation}
fn prepare(request:&RemuxRequest,token:&CancellationToken)->Result<Prepared> {
    if token.is_cancelled(){return Err(invalid("remux planning cancelled"));}
    let source=identity(&request.source_video,request.settings.maximum_source_bytes)?;
    if let Some(binding)=&request.subtitle_preservation {binding.recheck(&request.source_video,token)?;}
    let operation=RemuxOperation {schema:(if request.subtitle_preservation.is_some(){"aniflow.metadata-remux-operation/v2"}else{SCHEMA}).into(),settings:request.settings.clone(),settings_schema_sha256:crate::video_assembly::configuration_schema_reference().sha256,
        source_sha256:source.0.clone(),source_byte_count:source.1,selection:request.selection.clone(),subtitle_preservation:request.subtitle_preservation.clone()};operation.validate()?;
    crate::video_assembly::observe_components(&request.settings)?;
    let value=json!({"schema":"aniflow.pipeline/v3","name":"metadata-only-remux","inputs":[{"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}],
        "stages":[{"id":"metadata_remux","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"metadata_remux"},"fallbacks":[]},
            "inputs":[{"port":"source","artifacts":["source_video"]}],"outputs":[{"port":"media","artifacts":[{"id":"metadata_delivery","relative_path":"artifacts/metadata-remux/delivery.mp4","kind":"file"}]},{"port":"evidence","artifacts":[{"id":"metadata_evidence","relative_path":"artifacts/metadata-remux/private.json","kind":"file"}]}],
            "validations":[{"id":"remux_temporal","artifact":"metadata_delivery","contract":crate::validation::TEMPORAL_MEDIA_VALIDATION_CONTRACT_V1,
                "validator":{"capability":{"id":crate::validation::native::NATIVE_VALIDATION_CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"remux_validator"},"fallbacks":[]}},
                "temporal":{"source_artifact":"source_video","source_selection":request.selection,"artifact_selection":if request.subtitle_preservation.is_some(){request.selection.clone()}else{StreamSelection {video_stream:Some(0),audio_stream:None,no_audio:false,discard_streams:Vec::new(),audio_extent:request.selection.audio_extent}},"allow_authored_subtitles":request.subtitle_preservation.is_some()}},
                {"id":"remux_evidence_integrity","artifact":"metadata_evidence","contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1}]}],
        "outputs":[{"id":"metadata_delivery","artifact":"metadata_delivery","required_validations":["remux_temporal"]},{"id":"metadata_evidence","artifact":"metadata_evidence","required_validations":["remux_evidence_integrity"]}]});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(e.message))?;
    let mut registry=crate::ProviderRegistry::new();
    registry.register(crate::ProviderRegistration::new("metadata_remux",operation.manifest()?,operation.provider_configuration()?,request.provider_executable.clone(),"aniflow-metadata-remux-v1",crate::video_assembly::component_inventory(&request.settings))?)?;
    registry.register(crate::validation::native::registration("remux_validator",request.provider_executable.clone(),crate::video_assembly::native_validation_settings(&request.settings),"video/mp4","video/*")?)?;
    let bindings=vec![crate::PipelineInputBinding::new("source_video",&request.source_video)];
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&crate::audio_inspection::context()).map_err(|e|invalid(e.message))?;
    if !plan.payload.inputs.iter().any(|i|i.id=="source_video"&&i.content_sha256==source.0&&i.byte_count==source.1){return Err(invalid("remux input changed during planning"));}
    Ok(Prepared {plan,bindings,registry,operation})
}
pub fn plan(request:&RemuxRequest,token:&CancellationToken)->Result<crate::PipelineV3Plan> {Ok(prepare(request,token)?.plan)}
pub fn run<F:FnMut(&crate::PipelineV3RunProgress)>(request:RemuxRequest,parent:PathBuf,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(&parent)?;
    for input in [&request.source_video,&request.settings.ffmpeg.executable,&request.settings.ffprobe.executable,&request.provider_executable] {
        if parent.starts_with(input)||input.starts_with(&parent){return Err(invalid("remux run parent overlaps a source or executable"));}
    }
    let ready=prepare(&request,token)?;
    crate::video_assembly::require_storage(&parent,ready.operation.working_bytes()?,request.settings.minimum_free_bytes)?;
    crate::video_assembly::require_validator_storage(&request.settings)?;
    crate::run_v3_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(ready.plan,ready.bindings,ready.registry).with_output_directory(parent).with_execution_limits(request.execution_limits),token,progress)
}
pub fn resume<F:FnMut(&crate::PipelineV3RunProgress)>(request:RemuxRequest,root:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(root)?;let ready=prepare(&request,token)?;
    let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let mut bytes=Vec::new();crate::audio_inspection::process::open_regular(&workspace.plan()).map_err(|_|invalid("retained remux plan unavailable"))?.take(DOCUMENT_BYTES+1).read_to_end(&mut bytes).map_err(|_|invalid("cannot read retained remux plan"))?;
    if bytes.len() as u64>DOCUMENT_BYTES||crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|e|invalid(e.message))?!=ready.plan{return Err(invalid("remux source/policy/tools changed; start a new run"));}
    crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,ready.bindings,ready.registry).with_execution_limits(request.execution_limits),token,progress)
}
pub(crate) fn run_at<F:FnMut(&crate::PipelineV3RunProgress)>(request:RemuxRequest,root:&Path,expected:&crate::PipelineV3Plan,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    let parent=crate::run_v3::exact_run_parent(root)?;ensure_canonical_directory(&parent)?;
    for input in [&request.source_video,&request.settings.ffmpeg.executable,&request.settings.ffprobe.executable,&request.provider_executable] {
        if root.starts_with(input)||input.starts_with(root){return Err(invalid("remux child overlaps a source or executable"));}
    }
    let ready=prepare(&request,token)?;if ready.plan!=*expected{return Err(invalid("remux changed before fixed child initialization"));}
    crate::video_assembly::require_storage(&parent,ready.operation.working_bytes()?,request.settings.minimum_free_bytes)?;
    crate::video_assembly::require_validator_storage(&request.settings)?;
    crate::run_v3::run_v3_at_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(ready.plan,ready.bindings,ready.registry).with_output_directory(parent).with_execution_limits(request.execution_limits),root,token,progress)
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemuxReport {pub schema:String,pub operation:RemuxOperation,pub provider_lock_sha256:String,pub media:MetadataRemuxMedia}
fn write(path:&Path,bytes:&[u8])->Result<()> {
    let mut options=OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut file=options.open(path).map_err(|_|invalid("cannot create private remux evidence"))?;
    file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|_|invalid("cannot sync private remux evidence"))
}
pub fn execute_invocation(request:&crate::ProviderInvocationRequest)->Result<()> {
    request.validate()?;
    let operation:RemuxOperation=serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|_|invalid("invalid remux values"))?).map_err(|_|invalid("invalid closed remux operation"))?;
    operation.validate()?;
    if !cfg!(unix)||request.configuration!=operation.provider_configuration()?||request.stage_id!="metadata_remux"||request.inputs.len()!=1||request.outputs.len()!=2{return Err(invalid("remux invocation is outside its fixed Unix stage profile"));}
    let source=&request.inputs[0];
    if source.port!="source"||source.artifact_id!="source_video"||source.kind!=crate::ArtifactKind::File||source.artifact_type!="video/*"||source.artifact_role!=crate::ArtifactRole::TemporalSource||source.stream_role!=Some(crate::StreamRole::Video){return Err(invalid("remux input port differs from the fixed source profile"));}
    let output=|port:&str,id:&str,kind:&str,role:crate::ArtifactRole,stream:crate::StreamRole|->Result<&Path>{
        let values=request.outputs.iter().filter(|v|v.port==port).collect::<Vec<_>>();
        if values.len()!=1{return Err(invalid("remux output is missing or ambiguous"));}let v=values[0];
        if v.artifact_id!=id||v.kind!=crate::ArtifactKind::File||v.artifact_type!=kind||v.artifact_role!=role||v.stream_role!=Some(stream){return Err(invalid("remux output contract differs"));}Ok(&v.path)
    };
    let candidate=output("media","metadata_delivery","video/mp4",crate::ArtifactRole::CandidateMaster,crate::StreamRole::Video)?;
    let report=output("evidence","metadata_evidence","application/vnd.aniflow.metadata-remux-report+json",crate::ArtifactRole::ValidationEvidence,crate::StreamRole::TimedMetadata)?;
    let parent=candidate.parent().ok_or_else(||invalid("remux output parent missing"))?;
    if report.parent()!=Some(parent)||candidate==report{return Err(invalid("remux outputs must be distinct siblings"));}
    for path in [candidate,report] {require_absent(path)?;for input in [&source.path,&operation.settings.ffmpeg.executable,&operation.settings.ffprobe.executable]{if path.starts_with(input)||input.starts_with(path){return Err(invalid("remux outputs overlap inputs/tools"));}}}
    ensure_parent(parent)?;crate::video_assembly::require_storage(parent,operation.working_bytes()?,operation.settings.minimum_free_bytes)?;
    let expected=(operation.source_sha256.clone(),operation.source_byte_count);
    if identity(&source.path,operation.settings.maximum_source_bytes)?!=expected{return Err(invalid("remux source differs from its locked operation"));}
    if let Some(binding)=&operation.subtitle_preservation {binding.recheck(&binding.artifact()?.locator,&CancellationToken::default())?;}
    let tools=crate::video_assembly::observe_components(&operation.settings)?;
    let private=tempfile::Builder::new().prefix(".metadata-remux-").tempdir_in(parent).map_err(|_|invalid("cannot reserve private remux work"))?;
    let root=private.path().canonicalize().map_err(|_|invalid("cannot resolve remux scratch"))?;
    let token=CancellationToken::default();
    let result:Result<()>=(||{
        let mut settings=operation.settings.clone();settings.ffmpeg.executable=root.join("ffmpeg");settings.ffprobe.executable=root.join("ffprobe");
        for (pin,destination,expected) in [(&operation.settings.ffmpeg,&settings.ffmpeg.executable,&tools[0]),(&operation.settings.ffprobe,&settings.ffprobe.executable,&tools[1])] {
            copy_pinned(&pin.executable,destination,expected,TOOL_BYTES,&token)?;
            #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(destination,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot set private remux tool mode"))?;}
        }
        let snapshot=root.join("source.media");copy_pinned(&source.path,&snapshot,&expected,settings.maximum_source_bytes,&token)?;
        let new=root.join("delivery.mp4");
        let media=crate::video_assembly::media::metadata_remux(&settings,&snapshot,&operation.selection,operation.subtitle_preservation.as_ref(),&new,&root,&token)?;
        if identity(&source.path,settings.maximum_source_bytes)?!=expected||crate::video_assembly::observe_components(&operation.settings)?!=tools||crate::video_assembly::observe_components(&settings)?!=tools{return Err(invalid("remux original source or tools changed"));}
        if let Some(binding)=&operation.subtitle_preservation {binding.recheck(&binding.artifact()?.locator,&token)?;}
        let output_identity=identity(&new,settings.maximum_output_bytes)?;
        let value=RemuxReport {schema:"aniflow.metadata-remux-report/v1".into(),operation:operation.clone(),provider_lock_sha256:request.provider_lock_sha256.clone(),media};
        let bytes=canonical_json_bytes(&value)?;if bytes.len() as u64>DOCUMENT_BYTES{return Err(invalid("remux private receipt exceeds its evidence bound"));}
        write(report,&bytes)?;copy_pinned(&new,candidate,&output_identity,settings.maximum_output_bytes,&token)?;Ok(())
    })();
    if let Err(error)=&result {let root=private.keep();let _=write(&root.join("failure.json"),&canonical_json_bytes(&json!({"schema":"aniflow.metadata-remux-failure/v1","category":error.category(),"accepted":false,"cause":"unproven"}))?);}
    result
}

/// Read only the public-safe summary of an exact currently accepted fixed run.
/// No private names/values or source locator are projected, and no tool is run.
pub fn public_summary(root:&Path)->Result<super::MetadataPublicSummary> {
    Ok(observe_accepted_report(root)?.media.metadata.public_summary)
}

/// Private current native report, rechecked through the same accepted owner.
/// This performs no probe/remux and grants no public disclosure authority.
pub fn observe_accepted_report(root:&Path)->Result<RemuxReport> {
    ensure_canonical_directory(root)?;let before=crate::status_v3(root)?;
    if before.payload.state!=crate::PipelineV3RunState::Complete||before.payload.delivery.is_none(){return Err(invalid("remux summary requires current complete acceptance"));}
    let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let mut bytes=Vec::new();crate::audio_inspection::process::open_regular(&workspace.plan()).map_err(|_|invalid("remux plan missing"))?.take(DOCUMENT_BYTES+1).read_to_end(&mut bytes).map_err(|_|invalid("cannot read remux plan"))?;
    if bytes.len() as u64>DOCUMENT_BYTES{return Err(invalid("remux plan exceeds its bound"));}
    let plan=crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|e|invalid(e.message))?;
    let stage=plan.payload.stages.iter().find(|s|s.id=="metadata_remux").ok_or_else(||invalid("accepted run has no remux stage"))?;
    let select=|id:&str|->Result<&crate::ArtifactEvidence>{let values=before.payload.outputs.iter().filter(|a|a.id==id).collect::<Vec<_>>();if values.len()!=1{return Err(invalid("remux accepted output missing/ambiguous"));}Ok(values[0])};
    let report=select("metadata_evidence")?;let media=select("metadata_delivery")?;
    let path=root.join(report.relative_path.as_deref().ok_or_else(||invalid("remux report locator unavailable"))?);
    if !path.starts_with(root)||identity(&path,DOCUMENT_BYTES)?!=(report.sha256.clone(),report.byte_count){return Err(invalid("accepted remux report changed"));}
    let mut bytes=Vec::new();crate::audio_inspection::process::open_regular(&path).map_err(|_|invalid("private remux report unavailable"))?.take(DOCUMENT_BYTES+1).read_to_end(&mut bytes).map_err(|_|invalid("cannot read private remux report"))?;
    if format!("{:x}",Sha256::digest(&bytes))!=report.sha256{return Err(invalid("remux report changed during read"));}
    let value:RemuxReport=serde_json::from_slice(&bytes).map_err(|_|invalid("invalid private remux report"))?;
    value.operation.validate()?;
    if value.schema!="aniflow.metadata-remux-report/v1"||value.provider_lock_sha256!=stage.provider_lock.lock_sha256
        ||value.operation.provider_configuration()?.effective_configuration_sha256!=stage.provider_lock.payload.effective_configuration_sha256
        ||stage.provider_lock.payload.provider.id!=PROVIDER_ID||stage.capability.id!=CAPABILITY_ID
        ||value.media.source.source_sha256!=value.operation.source_sha256
        ||!plan.payload.inputs.iter().any(|i|i.id=="source_video"&&i.content_sha256==value.operation.source_sha256&&i.byte_count==value.operation.source_byte_count)
        ||value.media.candidate.source_sha256!=media.sha256||value.media.candidate.source_size_bytes!=media.byte_count
        ||value.media.container.source_sha256!=media.sha256||value.media.metadata.candidate_sha256!=media.sha256
        ||value.media.metadata.source_sha256!=value.operation.source_sha256
        ||value.media.metadata.effective_policy!=value.operation.settings.delivery_metadata.clone().unwrap()
        ||value.media.metadata.public_summary.status!=super::MetadataInspectionStatus::CleanWithinScope
        ||!value.media.full_decode_observed||value.media.native_qualified {return Err(invalid("remux report differs from current fixed acceptance"));}
    super::mp4::agrees(&value.media.container,&value.media.metadata.after)?;
    match (&value.operation.subtitle_preservation,&value.media.subtitle) {
        (None,None)=>{},(Some(binding),Some(evidence))=>{
            binding.recheck(&binding.artifact()?.locator,&CancellationToken::default())?;
            if evidence.schema!="aniflow.preserved-subtitle/v1"||evidence.source_stream!=binding.stream_index
                ||evidence.output_stream!=binding.stream_index||evidence.packet_count==0||!evidence.complete_text_roundtrip
                ||evidence.text_timing_sha256!=crate::video_assembly::media::subtitle_text_identity(&binding.document)? {
                return Err(invalid("accepted remux subtitle evidence differs from its current authored source"));
            }
            crate::provider::require_sha256(&evidence.packet_configuration_timing_sha256,"preserved subtitle packets")?;
        },_=>return Err(invalid("remux cannot omit or invent accepted subtitle evidence")),
    }
    if crate::status_v3(root)?!=before{return Err(invalid("remux acceptance changed during summary"));}
    Ok(value)
}
