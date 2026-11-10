//! Prepare only explicitly selected current files; no discovery or adoption.
use super::*;
const REQUEST_SCHEMA:&str="aniflow.historical-import-preparation-request/v1";
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedPath {pub ordinal:u64,pub file:PathBuf}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationRequest {
    pub schema:String,pub source_video:PathBuf,pub source_index:PathBuf,
    pub frames:Vec<SelectedPath>,pub settings:ImportSettings,pub provider_executable:PathBuf,
}
fn absolute(path:&Path)->bool {path.is_absolute()&&path.as_os_str().len()<=4096&&path.components().all(|c|!matches!(c,std::path::Component::ParentDir|std::path::Component::CurDir))}
fn selection(frames:&[SelectedPath])->Result<()> {
    if frames.is_empty()||frames.len()>MAX_FRAMES{return Err(invalid("prepare 1..128 explicit historical frame paths"));}
    let mut previous=None;let mut paths=std::collections::BTreeSet::new();
    for frame in frames {
        if frame.ordinal>=100_000||previous.is_some_and(|p|p>=frame.ordinal)||!absolute(&frame.file)||!paths.insert(&frame.file){return Err(invalid("selected historical paths require distinct normalized locators and strictly increasing original ordinals"));}
        previous=Some(frame.ordinal);
    }Ok(())
}
impl PreparationRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>262_144{return Err(invalid("historical import preparation exceeds 256 KiB"));}
        let value:Self=crate::provider::decode_json(bytes,"historical import preparation request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        if self.schema!=REQUEST_SCHEMA||![&self.source_video,&self.source_index,&self.provider_executable].into_iter().all(|p|absolute(p)){return Err(invalid("explicit original source/index/provider locators required"));}
        self.settings.decoder.validate()?;selection(&self.frames)
    }
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedImport {
    pub schema:String,pub request_sha256:String,pub request:ImportRequest,pub review:ImportReviewPlan,
    pub provider_evidence:String,pub import_approval:String,pub execution_authority:String,
}
/// Observe current selected PNG structure/bytes and the original clock. Full
/// native decode and any supplied exact import decision remain separate owners.
pub fn prepare(request:&PreparationRequest,token:&CancellationToken)->Result<PreparedImport> {
    check_cancelled(token)?;request.validate()?;
    let (index,index_pin)=frame_import::read_index(&request.source_index)?;
    request.settings.validate(index.width,index.height)?;
    let mut frames=Vec::new();let mut total=0u64;
    for selected in &request.frames {
        check_cancelled(token)?;
        if selected.ordinal>=index.frames.len() as u64{return Err(invalid("selected original ordinal is outside the exact index"));}
        let before=identity(&selected.file,request.settings.decoder.maximum_frame_bytes)?;
        total=total.checked_add(before.1).filter(|n|*n<=request.settings.decoder.maximum_total_frame_bytes).ok_or_else(||invalid("selected historical bytes exceed declared total"))?;
        let observed=inspect_png(&selected.file,request.settings.output_width,request.settings.output_height,request.settings.decoder.maximum_frame_bytes)?;
        if observed!=before{return Err(invalid("selected historical PNG changed during preparation"));}
        frames.push(SelectedFile {ordinal:selected.ordinal,file:selected.file.clone(),sha256:observed.0,byte_count:observed.1});
    }
    let prepared=ImportRequest {schema:"aniflow.historical-spatial-import-request/v1".into(),source_video:request.source_video.clone(),source_index:request.source_index.clone(),
        frames,settings:request.settings.clone(),provider_executable:request.provider_executable.clone()};
    let review=review_plan(&prepared,token)?;
    if review.intent.source_index_sha256!=index_pin.0||review.intent.source_index_bytes!=index_pin.1||review_plan(&prepared,token)?!=review{return Err(invalid("original source/index or selected frames changed during preparation"));}
    check_cancelled(token)?;let request_sha256=canonical_sha256(&prepared)?;
    let result=PreparedImport {schema:"aniflow.historical-import-preparation/v1".into(),request_sha256,request:prepared,review,
        provider_evidence:"unobserved; existing native plan owns provider/tool identity".into(),import_approval:"none; supply a separate exact decision after review".into(),execution_authority:"none".into()};
    crate::destination::local::bytes(&result)?;Ok(result)
}
/// Save an inert request after reobserving its exact subject. Never create an
/// approval or replace an existing document, input, decoder or provider.
pub fn write_request(prepared:&PreparedImport,path:&Path,token:&CancellationToken)->Result<()> {
    check_cancelled(token)?;
    if !absolute(path)||prepared.schema!="aniflow.historical-import-preparation/v1"||prepared.request_sha256!=canonical_sha256(&prepared.request)?
        ||prepared.import_approval!="none; supply a separate exact decision after review"||prepared.execution_authority!="none"
        ||review_plan(&prepared.request,token)?!=prepared.review {return Err(invalid("prepared import subject changed before saving request"));}
    for source in [&prepared.request.source_video,&prepared.request.source_index,&prepared.request.provider_executable,&prepared.request.settings.decoder.ffmpeg.executable].into_iter().chain(prepared.request.frames.iter().map(|f|&f.file)) {
        if path.starts_with(source)||source.starts_with(path){return Err(invalid("prepared request must be separate from selected source/index/frames/tools"));}
    }
    let bytes=canonical_json_bytes(&prepared.request)?;if bytes.len()>262_144{return Err(invalid("prepared import request exceeds 256 KiB"));}
    check_cancelled(token)?;crate::destination::local::write(path,&bytes)
}

#[cfg(test)] mod tests {
    use super::*;
    fn selected(n:u64)->SelectedPath {SelectedPath {ordinal:n,file:format!("/synthetic/frame-{n}.png").into()}}
    #[test] fn explicit_order_and_unique_files_are_required(){assert!(selection(&[selected(5),selected(9)]).is_ok());assert!(selection(&[selected(5),selected(5)]).is_err());assert!(selection(&[selected(9),selected(5)]).is_err());let mut other=selected(9);other.file=selected(5).file;assert!(selection(&[selected(5),other]).is_err());}
    #[test] fn no_folder_discovery_or_unbounded_selection(){assert!(selection(&[]).is_err());assert!(selection(&(0..129).map(selected).collect::<Vec<_>>()).is_err());assert!(selection(&[selected(100_000)]).is_err());let mut s=selected(1);s.file="../loose.png".into();assert!(selection(&[s]).is_err());}
    #[test] fn supplied_hash_or_processing_history_cannot_replace_observation(){for field in ["sha256","byte_count","model","approved"]{let mut value=json!({"ordinal":5,"file":"/synthetic/frame.png"});value[field]=json!("invented");assert!(crate::provider::decode_json::<SelectedPath>(&canonical_json_bytes(&value).unwrap(),"selected path").is_err());}}
    #[test] fn preparation_refuses_cancelled_selection_before_access(){let token=CancellationToken::new();token.cancel();let decoder=FrameImportConfiguration {schema:frame_import::FRAME_IMPORT_CONFIGURATION_SCHEMA_V1.into(),ffmpeg:crate::audio_inspection::AudioToolPin {executable:"/synthetic/ffmpeg".into(),version:"8.0.1".into(),sha256:"a".repeat(64)},tool_timeout_milliseconds:1000,maximum_source_bytes:1_048_576,maximum_frame_bytes:1_048_576,maximum_total_frame_bytes:16_777_216,maximum_pixels:1_048_576};
        let request=PreparationRequest {schema:REQUEST_SCHEMA.into(),source_video:"/synthetic/missing-source".into(),source_index:"/synthetic/missing-index".into(),frames:vec![selected(5)],settings:ImportSettings {decoder,output_width:4,output_height:4,minimum_free_bytes:0},provider_executable:"/synthetic/provider".into()};assert!(prepare(&request,&token).unwrap_err().message.contains("cancel"));}
}
