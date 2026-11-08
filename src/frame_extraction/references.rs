//! Bounded periodic reference images. No generation, model calls or uploads.
use std::{ffi::OsString,fs::OpenOptions,io::{Read,Write},path::{Path,PathBuf}};
use serde::{Deserialize,Serialize};
use serde_json::json;
use sha2::{Digest,Sha256};
use crate::{CancellationToken,Result};
use crate::frame_io::{identity,copy_pinned,ensure_canonical_directory,ensure_parent,require_absent,inspect_png,inventory};
use crate::provider::canonical_json_bytes;
use crate::temporal::{RationalTime,TimeRange};
use super::{FrameExtractionConfiguration,invalid};
pub const PROVIDER_ID:&str="org.egohygiene.aniflow.reference-frames";
pub const CAPABILITY_ID:&str="aniflow/reference-frames";
const OPERATION_SCHEMA:&str="aniflow.reference-frames-operation/v1";
const DOCUMENT_BYTES:u64=67_108_864;
const TOOL_BYTES:u64=134_217_728;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingPolicy {
    pub schema:String,pub every_n_frames:u64,pub maximum_samples:u64,
    pub include_first:bool,pub include_last:bool,pub overflow:String,
}
impl SamplingPolicy {
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.reference-sampling/v1"||!(1..=100_000).contains(&self.every_n_frames)
            ||!(1..=256).contains(&self.maximum_samples)||!self.include_first||!self.include_last||self.overflow!="reject" {
            return Err(invalid("reference sampling requires bounded periodic cadence, first/last, 1..256 samples and explicit reject overflow"));
        }Ok(())
    }
    /// Select positions, not distinct pixels. Static/cut/duplicate images keep
    /// their source timestamps; no scene/semantic importance is inferred.
    pub fn select(&self,source_frames:u64)->Result<Vec<SelectedFrame>> {
        self.validate()?;
        if !(1..=100_000).contains(&source_frames){return Err(invalid("reference source needs 1..100000 observed frames"));}
        let periodic=(source_frames-1)/self.every_n_frames+1;
        let extra=u64::from((source_frames-1)%self.every_n_frames!=0);
        if periodic+extra>self.maximum_samples{return Err(invalid("reference cadence plus first/last exceeds maximum_samples; increase interval or explicit sample cap"));}
        let mut positions=(0..source_frames).step_by(self.every_n_frames as usize).collect::<Vec<_>>();
        if positions.last().copied()!=Some(source_frames-1){positions.push(source_frames-1);}
        Ok(positions.into_iter().map(|source_ordinal|{
            let mut reasons=Vec::new();if source_ordinal==0{reasons.push("first".into());}
            if source_ordinal%self.every_n_frames==0{reasons.push("periodic".into());}
            if source_ordinal==source_frames-1{reasons.push("last".into());}
            SelectedFrame {source_ordinal,reasons}
        }).collect())
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedFrame {pub source_ordinal:u64,pub reasons:Vec<String>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceImage {
    pub sample_ordinal:u64,pub source_ordinal:u64,pub source_time:TimeRange,pub reasons:Vec<String>,
    pub filename:String,pub sha256:String,pub byte_count:u64,pub decoded_rgb24_sha256:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceManifest {
    pub schema:String,pub source_sha256:String,pub source_byte_count:u64,pub source_frames:u64,
    pub source_period:RationalTime,pub width:u32,pub height:u32,pub policy:SamplingPolicy,
    pub ffmpeg:crate::audio_inspection::AudioToolObservation,pub ffprobe:crate::audio_inspection::AudioToolObservation,
    pub images:Vec<ReferenceImage>,pub conversion:String,pub native_qualified:bool,
}
impl ReferenceManifest {
    pub fn validate(&self)->Result<()> {
        crate::provider::require_sha256(&self.source_sha256,"reference source")?;
        let selected=self.policy.select(self.source_frames)?;
        if self.schema!="aniflow.reference-manifest/v1"||self.source_byte_count==0||self.source_period<=RationalTime::ZERO
            ||self.width==0||self.height==0||self.images.len()!=selected.len()||self.native_qualified
            ||self.conversion!="explicit_bt709_source_to_full_range_rgb24" {return Err(invalid("reference manifest differs from its bounded selected source"));}
        for (id,pin) in [("ffmpeg",&self.ffmpeg),("ffprobe",&self.ffprobe)] {
            crate::provider::require_sha256(&pin.sha256,"reference tool")?;
            if pin.id!=id||pin.version!="8.0.1"{return Err(invalid("reference tool provenance is outside the pinned interface"));}
        }
        for (ordinal,(image,pick)) in self.images.iter().zip(&selected).enumerate() {
            crate::provider::require_sha256(&image.sha256,"reference image")?;crate::provider::require_sha256(&image.decoded_rgb24_sha256,"reference pixels")?;
            let interval=TimeRange::new(self.source_period.checked_mul(pick.source_ordinal).map_err(|e|invalid(e.to_string()))?,self.source_period.checked_mul(pick.source_ordinal+1).map_err(|e|invalid(e.to_string()))?).map_err(|e|invalid(e.to_string()))?;
            if image.sample_ordinal!=ordinal as u64||image.source_ordinal!=pick.source_ordinal||image.reasons!=pick.reasons
                ||image.filename!=format!("frame-{ordinal:08}.png")||image.byte_count==0||image.source_time!=interval{return Err(invalid("reference image mapping/name/reason/time differs from selected source"));}
        }Ok(())
    }
    /// Portable review view, containing only fixed generated names/identities.
    pub fn markdown(&self)->Result<String> {
        self.validate()?;
        let mut text=format!("# Reference frames\n\nSource SHA-256: `{}` ({} bytes). Original clock: `{}` seconds/frame.\n\nReview these images locally before handing selected files to a separate creative app. No visual or generation approval is implied.\n\n| Image | Source frame | Exact source interval | Reason |\n| --- | ---: | --- | --- |\n",self.source_sha256,self.source_byte_count,self.source_period);
        for image in &self.images {text.push_str(&format!("| [sample {}](frames/{}) | {} | {}–{} | {} |\n",image.sample_ordinal,image.filename,image.source_ordinal,image.source_time.start,image.source_time.end,image.reasons.join(", ")));}
        text.push_str("\nTool provenance and image/pixel hashes are recorded in manifest.json. Original audio/video are unchanged; this sparse reference set cannot reconstruct the complete clip.\n");Ok(text)
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceOperation {
    pub schema:String,pub settings:FrameExtractionConfiguration,pub settings_schema_sha256:String,
    pub source_sha256:String,pub source_byte_count:u64,pub policy:SamplingPolicy,
}
impl ReferenceOperation {
    pub fn validate(&self)->Result<()> {
        self.settings.validate()?;self.policy.validate()?;crate::provider::require_sha256(&self.source_sha256,"reference operation source")?;
        if self.schema!=OPERATION_SCHEMA||self.settings_schema_sha256!=super::configuration_schema_reference().sha256
            ||self.source_byte_count==0||self.source_byte_count>self.settings.maximum_source_bytes{return Err(invalid("reference operation has incompatible source/settings authority"));}Ok(())
    }
    fn schema()->crate::ConfigurationSchemaReference {crate::ConfigurationSchemaReference {id:OPERATION_SCHEMA.into(),version:"1.0.0".into(),sha256:format!("{:x}",Sha256::digest(include_bytes!("../../docs/contracts/reference-frames-operation-v1.schema.json")))}}
    pub fn provider_configuration(&self)->Result<crate::ProviderConfiguration> {
        self.validate()?;let value=serde_json::to_value(self).map_err(|_|invalid("cannot serialize reference operation"))?;
        crate::ProviderConfiguration::new(crate::ProviderReference {id:PROVIDER_ID.into(),version:"1.0.0".into()},crate::CapabilityReference {id:CAPABILITY_ID.into(),version:"1.0.0".into()},Self::schema(),value.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.clone())).collect())
    }
    pub fn manifest(&self)->Result<crate::ProviderManifest> {
        self.validate()?;let port=|name:&str,kind:&str,role:&str,stream:&str|json!({"name":name,"artifact_type":kind,"artifact_role":role,"stream_role":stream,"cardinality":"one","immutable":true});
        let tools=[("ffmpeg",&self.settings.ffmpeg),("ffprobe",&self.settings.ffprobe)].into_iter().map(|(id,pin)|json!({"id":id,"version_requirement":format!("={}",pin.version),"sha256":pin.sha256})).collect::<Vec<_>>();
        let value=json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER_ID,"version":"1.0.0","display_name":"aniflow bounded reference images"},"configuration_schemas":[Self::schema()],"capabilities":[{
            "id":CAPABILITY_ID,"version":"1.0.0","kind":"frame_processor","configuration_schema":Self::schema(),"inputs":[port("source","video/*","temporal_source","video")],
            "outputs":[port("frames","application/vnd.aniflow.reference-frames","intermediate","video"),port("manifest","application/vnd.aniflow.reference-manifest+json","validation_evidence","timed_metadata"),port("review","text/markdown","validation_evidence","timed_metadata")],
            "batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":tools,"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"environment_dependent","fidelity":"lossy","cacheable":false,"content_changes":true,"side_effects":["filesystem_read","filesystem_write","environment_read","subprocess"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],
            "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}});
        crate::ProviderManifest::from_json_slice(&canonical_json_bytes(&value)?)
    }
    fn working(&self)->Result<u64> {
        let tools=super::observe_components(&self.settings)?;
        let base=super::checked_working_bytes(&self.settings,self.source_byte_count,tools[0].1+tools[1].1)?;
        base.checked_add(self.settings.maximum_total_frame_bytes.checked_mul(2).ok_or_else(||invalid("reference byte limit overflow"))?)
            .filter(|v|*v<=self.settings.maximum_working_bytes).ok_or_else(||invalid("reference snapshots/selected frames exceed maximum_working_bytes"))
    }
}
#[derive(Debug,Clone)]
pub struct ReferenceRequest {pub extraction:super::FrameExtractionRequest,pub policy:SamplingPolicy}
impl ReferenceRequest {
    pub fn new(mut extraction:super::FrameExtractionRequest,policy:SamplingPolicy)->Self {
        extraction.execution_limits.maximum_artifact_files=policy.maximum_samples.min(256).saturating_mul(2).saturating_add(32);
        Self {extraction,policy}
    }
}
struct Prepared {plan:crate::PipelineV3Plan,bindings:Vec<crate::PipelineInputBinding>,registry:crate::ProviderRegistry,operation:ReferenceOperation}
fn prepare(request:&ReferenceRequest,token:&CancellationToken)->Result<Prepared> {
    if token.is_cancelled(){return Err(invalid("reference planning cancelled"));}
    let extraction=&request.extraction;extraction.configuration.validate()?;extraction.execution_limits.validate()?;
    let source=identity(&extraction.source_video,extraction.configuration.maximum_source_bytes)?;
    let operation=ReferenceOperation {schema:OPERATION_SCHEMA.into(),settings:extraction.configuration.clone(),settings_schema_sha256:super::configuration_schema_reference().sha256,source_sha256:source.0.clone(),source_byte_count:source.1,policy:request.policy.clone()};
    operation.validate()?;super::observe_components(&extraction.configuration)?;
    let outputs=[("frames","reference_frames","artifacts/references/frames","directory"),("manifest","reference_manifest","artifacts/references/manifest.json","file"),("review","reference_review","artifacts/references/README.md","file")];
    let value=json!({"schema":"aniflow.pipeline/v3","name":"bounded-reference-frames","inputs":[{"id":"source_video","artifact_type":"video/*","artifact_role":"temporal_source","stream_role":"video"}],
        "stages":[{"id":"reference_frames","depends_on":[],"capability":{"id":CAPABILITY_ID,"version_requirement":"=1.0.0"},"provider":{"primary":{"registration_id":"reference_frames"},"fallbacks":[]},"inputs":[{"port":"source","artifacts":["source_video"]}],
            "outputs":outputs.iter().map(|(port,id,path,kind)|json!({"port":port,"artifacts":[{"id":id,"relative_path":path,"kind":kind}]})).collect::<Vec<_>>(),
            "validations":outputs.iter().map(|(_,id,_,_)|json!({"id":format!("{id}_integrity"),"artifact":id,"contract":crate::ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1})).collect::<Vec<_>>()}],
        "outputs":outputs.iter().map(|(_,id,_,_)|json!({"id":id,"artifact":id,"required_validations":[format!("{id}_integrity")]})).collect::<Vec<_>>()});
    let pipeline=crate::PipelineV3Configuration::from_yaml_slice(&canonical_json_bytes(&value)?).map_err(|e|invalid(e.message))?;
    let mut registry=crate::ProviderRegistry::new();registry.register(crate::ProviderRegistration::new("reference_frames",operation.manifest()?,operation.provider_configuration()?,extraction.provider_executable.clone(),"aniflow-reference-frames-v1",crate::video_assembly::component_inventory(&extraction.configuration.assembly_settings()))?)?;
    let bindings=vec![crate::PipelineInputBinding::new("source_video",&extraction.source_video)];
    let plan=crate::resolve_pipeline_v3(&pipeline,&bindings,&registry,&crate::audio_inspection::context()).map_err(|e|invalid(e.message))?;
    if !plan.payload.inputs.iter().any(|i|i.id=="source_video"&&i.content_sha256==source.0&&i.byte_count==source.1){return Err(invalid("reference source changed while planning"));}
    Ok(Prepared {plan,bindings,registry,operation})
}
pub fn plan(request:&ReferenceRequest,token:&CancellationToken)->Result<crate::PipelineV3Plan> {Ok(prepare(request,token)?.plan)}
pub fn run<F:FnMut(&crate::PipelineV3RunProgress)>(request:ReferenceRequest,parent:PathBuf,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(&parent)?;
    let extraction=&request.extraction;
    for input in [&extraction.source_video,&extraction.configuration.ffmpeg.executable,&extraction.configuration.ffprobe.executable,&extraction.provider_executable] {
        if parent.starts_with(input)||input.starts_with(&parent){return Err(invalid("reference run parent overlaps source/tools"));}
    }
    let ready=prepare(&request,token)?;super::require_storage(&parent,ready.operation.working()?,extraction.configuration.minimum_free_bytes)?;
    crate::run_v3_with_progress_and_cancellation(crate::PipelineV3RunRequest::new(ready.plan,ready.bindings,ready.registry).with_output_directory(parent).with_execution_limits(extraction.execution_limits.clone()),token,progress)
}
pub fn resume<F:FnMut(&crate::PipelineV3RunProgress)>(request:ReferenceRequest,root:&Path,token:&CancellationToken,progress:F)->Result<crate::PipelineV3RunOutcome> {
    ensure_canonical_directory(root)?;let ready=prepare(&request,token)?;let workspace=crate::PipelineV3Workspace::open_read_only(root)?;
    let mut bytes=Vec::new();crate::audio_inspection::process::open_regular(&workspace.plan()).map_err(|_|invalid("retained reference plan unavailable"))?.take(DOCUMENT_BYTES+1).read_to_end(&mut bytes).map_err(|_|invalid("cannot read reference plan"))?;
    if bytes.len() as u64>DOCUMENT_BYTES||crate::PipelineV3Plan::from_json_slice(&bytes).map_err(|e|invalid(e.message))?!=ready.plan{return Err(invalid("reference source/policy/tools changed; start a new run"));}
    crate::resume_v3_with_progress_and_cancellation(crate::PipelineV3ResumeRequest::new(root,ready.bindings,ready.registry).with_execution_limits(request.extraction.execution_limits),token,progress)
}

fn select_arguments(mut args:Vec<OsString>,ordinals:&[u64],maximum:u64)->Result<Vec<OsString>> {
    if ordinals.is_empty()||ordinals.len() as u64>maximum{return Err(invalid("reference filter needs a bounded nonempty selection"));}
    let filter=args.iter().position(|a|a=="-vf").ok_or_else(||invalid("source RGB conversion missing"))?+1;
    let current=args[filter].to_str().ok_or_else(||invalid("RGB conversion is not text"))?;
    let select=ordinals.iter().map(|n|format!("eq(n\\,{n})")).collect::<Vec<_>>().join("+");
    args[filter]=format!("select={select},{current}").into();
    let limit=args.iter().position(|a|a=="-frames:v").ok_or_else(||invalid("source frame bound missing"))?+1;
    args[limit]=(maximum+1).to_string().into();Ok(args)
}
fn extract(operation:&ReferenceOperation,settings:&FrameExtractionConfiguration,source:&Path,directory:&Path,scratch:&Path,token:&CancellationToken)->Result<ReferenceManifest> {
    let observation=crate::video_assembly::media::observe_extraction_source(&settings.assembly_settings(),source,&settings.source_selection,scratch,token)?;
    let timeline=observation.temporal.video_timeline().map_err(|e|invalid(e.to_string()))?;
    if timeline.frames.len() as u64>settings.maximum_frames||u64::from(observation.width)*u64::from(observation.height)>settings.maximum_pixels{return Err(invalid("reference source exceeds its configured count/pixel bounds"));}
    let selected=operation.policy.select(timeline.frames.len() as u64)?;
    let ordinals=selected.iter().map(|v|v.source_ordinal).collect::<Vec<_>>();
    let period=timeline.frame_period.ok_or_else(||invalid("reference initial profile requires observed CFR"))?;
    let frames=crate::video_assembly::media::FrameSet {directory:None,width:observation.width,height:observation.height,period,frame_count:selected.len(),source_inspection:observation.temporal.clone(),derived_video_clock:None};
    let pixels=u64::from(observation.width)*u64::from(observation.height);
    let ceiling=pixels.checked_mul(3).and_then(|v|v.checked_add(u64::from(observation.height))).and_then(|v|v.checked_mul(2)).and_then(|v|v.checked_add(65_536)).ok_or_else(||invalid("reference PNG envelope overflow"))?;
    if ceiling>settings.maximum_frame_bytes||ceiling.checked_mul(selected.len() as u64).is_none_or(|v|v>settings.maximum_total_frame_bytes){return Err(invalid("selected reference PNG envelope exceeds frame/aggregate byte limits"));}
    let hash_args=select_arguments(super::media::hash_arguments(source,&observation,settings)?,&ordinals,operation.policy.maximum_samples)?;
    let expected=crate::video_assembly::media::parse_selected_framehash(&super::media::run(&hash_args,settings,scratch,token)?,&frames,Some(&ordinals))?;
    std::fs::create_dir(directory).map_err(|_|invalid("cannot exclusively create selected reference directory"))?;
    let args=select_arguments(super::media::extraction_arguments(source,&observation,directory,settings)?,&ordinals,operation.policy.maximum_samples)?;
    super::media::run(&args,settings,scratch,token)?;
    let names=(0..selected.len()).map(|n|OsString::from(format!("frame-{n:08}.png"))).collect::<Vec<_>>();inventory(directory,&names)?;
    let mut images=Vec::new();let mut total=0u64;
    for (ordinal,(selected,rgb)) in selected.iter().zip(&expected).enumerate() {
        if token.is_cancelled(){return Err(invalid("reference extraction cancelled"));}
        let filename=format!("frame-{ordinal:08}.png");let path=directory.join(&filename);
        let observed=inspect_png(&path,observation.width,observation.height,settings.maximum_frame_bytes)?;
        total=total.checked_add(observed.1).filter(|v|*v<=settings.maximum_total_frame_bytes).ok_or_else(||invalid("reference PNG aggregate overflow"))?;
        let decoded=crate::frame_decode::decode(&settings.ffmpeg,&settings.tool_limits(),token,scratch,&path,observation.width,observation.height)?;
        if decoded!=*rgb||identity(&path,settings.maximum_frame_bytes)?!=observed{return Err(invalid("reference PNG changed or differs from exact selected source RGB"));}
        images.push(ReferenceImage {sample_ordinal:ordinal as u64,source_ordinal:selected.source_ordinal,source_time:timeline.frames[selected.source_ordinal as usize].source_time,reasons:selected.reasons.clone(),filename,sha256:observed.0,byte_count:observed.1,decoded_rgb24_sha256:decoded});
    }
    let fresh=crate::video_assembly::media::observe_extraction_source(&settings.assembly_settings(),source,&settings.source_selection,scratch,token)?;
    if fresh.temporal!=observation.temporal||fresh.width!=observation.width||fresh.height!=observation.height||fresh.pixel_format!=observation.pixel_format||fresh.color_range!=observation.color_range{return Err(invalid("reference source observations changed during sampling"));}
    let manifest=ReferenceManifest {schema:"aniflow.reference-manifest/v1".into(),source_sha256:operation.source_sha256.clone(),source_byte_count:operation.source_byte_count,source_frames:timeline.frames.len() as u64,source_period:period,width:observation.width,height:observation.height,policy:operation.policy.clone(),
        ffmpeg:crate::audio_inspection::AudioToolObservation{id:"ffmpeg".into(),version:settings.ffmpeg.version.clone(),sha256:settings.ffmpeg.sha256.clone()},ffprobe:crate::audio_inspection::AudioToolObservation{id:"ffprobe".into(),version:settings.ffprobe.version.clone(),sha256:settings.ffprobe.sha256.clone()},images,conversion:"explicit_bt709_source_to_full_range_rgb24".into(),native_qualified:false};
    manifest.validate()?;Ok(manifest)
}
fn write(path:&Path,bytes:&[u8])->Result<()> {
    let mut options=OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut file=options.open(path).map_err(|_|invalid("cannot exclusively create reference manifest"))?;file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|_|invalid("cannot sync reference manifest"))
}
pub fn execute_invocation(request:&crate::ProviderInvocationRequest)->Result<()> {
    request.validate()?;let operation:ReferenceOperation=serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|_|invalid("invalid reference values"))?).map_err(|_|invalid("invalid closed reference operation"))?;operation.validate()?;
    if !cfg!(unix)||request.configuration!=operation.provider_configuration()?||request.stage_id!="reference_frames"||request.inputs.len()!=1||request.outputs.len()!=3{return Err(invalid("reference invocation is outside its fixed Unix stage"));}
    let source=&request.inputs[0];
    if source.port!="source"||source.artifact_id!="source_video"||source.kind!=crate::ArtifactKind::File||source.artifact_type!="video/*"||source.artifact_role!=crate::ArtifactRole::TemporalSource||source.stream_role!=Some(crate::StreamRole::Video){return Err(invalid("reference source port differs from the fixed profile"));}
    let output=|port:&str,id:&str,kind:crate::ArtifactKind,mime:&str,role:crate::ArtifactRole,stream:crate::StreamRole|->Result<&Path>{let found=request.outputs.iter().filter(|v|v.port==port).collect::<Vec<_>>();if found.len()!=1{return Err(invalid("reference output missing/ambiguous"));}let v=found[0];if v.artifact_id!=id||v.kind!=kind||v.artifact_type!=mime||v.artifact_role!=role||v.stream_role!=Some(stream){return Err(invalid("reference output port differs"));}Ok(&v.path)};
    let frames=output("frames","reference_frames",crate::ArtifactKind::Directory,"application/vnd.aniflow.reference-frames",crate::ArtifactRole::Intermediate,crate::StreamRole::Video)?;
    let manifest_path=output("manifest","reference_manifest",crate::ArtifactKind::File,"application/vnd.aniflow.reference-manifest+json",crate::ArtifactRole::ValidationEvidence,crate::StreamRole::TimedMetadata)?;
    let review=output("review","reference_review",crate::ArtifactKind::File,"text/markdown",crate::ArtifactRole::ValidationEvidence,crate::StreamRole::TimedMetadata)?;
    let parent=frames.parent().ok_or_else(||invalid("reference parent missing"))?;
    let outputs=[frames,manifest_path,review];
    for (i,path) in outputs.iter().enumerate(){if path.parent()!=Some(parent)||outputs[..i].contains(path){return Err(invalid("reference outputs must be distinct siblings"));}require_absent(path)?;for input in [&source.path,&operation.settings.ffmpeg.executable,&operation.settings.ffprobe.executable]{if path.starts_with(input)||input.starts_with(path){return Err(invalid("reference output overlaps source/tools"));}}}
    ensure_parent(parent)?;super::require_storage(parent,operation.working()?,operation.settings.minimum_free_bytes)?;
    let source_identity=(operation.source_sha256.clone(),operation.source_byte_count);if identity(&source.path,operation.settings.maximum_source_bytes)?!=source_identity{return Err(invalid("reference source differs from locked identity"));}
    let tools=super::observe_components(&operation.settings)?;
    let private=tempfile::Builder::new().prefix(".reference-frames-").tempdir_in(parent).map_err(|_|invalid("cannot reserve private reference work"))?;let root=private.path().canonicalize().map_err(|_|invalid("cannot resolve private references"))?;
    let token=CancellationToken::default();let result:Result<()>=(||{
        let mut settings=operation.settings.clone();settings.ffmpeg.executable=root.join("ffmpeg");settings.ffprobe.executable=root.join("ffprobe");
        for (pin,copy,expected) in [(&operation.settings.ffmpeg,&settings.ffmpeg.executable,&tools[0]),(&operation.settings.ffprobe,&settings.ffprobe.executable,&tools[1])] {
            copy_pinned(&pin.executable,copy,expected,TOOL_BYTES,&token)?;
            #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(copy,std::fs::Permissions::from_mode(0o700)).map_err(|_|invalid("cannot set private reference tool mode"))?;}
        }
        let snapshot=root.join("source.media");copy_pinned(&source.path,&snapshot,&source_identity,settings.maximum_source_bytes,&token)?;
        let private_frames=root.join("frames");let manifest=extract(&operation,&settings,&snapshot,&private_frames,&root,&token)?;
        if identity(&source.path,settings.maximum_source_bytes)?!=source_identity||identity(&snapshot,settings.maximum_source_bytes)?!=source_identity||super::observe_components(&operation.settings)?!=tools||super::observe_components(&settings)?!=tools{return Err(invalid("reference source/tools changed"));}
        let bytes=canonical_json_bytes(&manifest)?;if bytes.len() as u64>DOCUMENT_BYTES{return Err(invalid("reference manifest exceeds its bound"));}
        std::fs::create_dir(frames).map_err(|_|invalid("cannot exclusively create reference frame output"))?;
        for image in &manifest.images {copy_pinned(&private_frames.join(&image.filename),&frames.join(&image.filename),&(image.sha256.clone(),image.byte_count),settings.maximum_frame_bytes,&token)?;}
        inventory(frames,&manifest.images.iter().map(|i|OsString::from(&i.filename)).collect::<Vec<_>>())?;
        write(manifest_path,&bytes)?;write(review,manifest.markdown()?.as_bytes())?;
        if identity(&source.path,settings.maximum_source_bytes)?!=source_identity||super::observe_components(&operation.settings)?!=tools{return Err(invalid("reference original source/tools changed during publication"));}Ok(())
    })();
    if let Err(error)=&result {let retained=private.keep();let _=write(&retained.join("failure.json"),&canonical_json_bytes(&json!({"schema":"aniflow.reference-frames-failure/v1","category":error.category(),"cause":"unproven","accepted":false}))?);}
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_filter_keeps_original_clock_and_bounded_sentinel() {
        let args=vec!["-vf".into(),"format=pix_fmts=rgb24".into(),"-fps_mode:v".into(),"passthrough".into(),"-enc_time_base:v".into(),"1/30".into(),"-frames:v".into(),"100001".into()];
        let args=select_arguments(args,&[0,30,59],3).unwrap();
        assert_eq!(args[1],"select=eq(n\\,0)+eq(n\\,30)+eq(n\\,59),format=pix_fmts=rgb24");assert_eq!(args[7],"4");
        assert!(args.windows(2).any(|a|a==[OsString::from("-fps_mode:v"),OsString::from("passthrough")]));
        assert!(!args.iter().any(|a|a=="-r"||a=="-shortest"||a=="-ss"));
    }
}
