//! Complete copy-only composition over current accepted spatial units. Native
//! v3 remains the acceptance owner; historical processing is never inferred.
use std::path::{Path,PathBuf};
use std::collections::BTreeSet;
use serde::{Serialize,Deserialize};
use crate::{CancellationToken,Result};
use crate::destination::local;
use crate::frame_io::{identity,inspect_png,inventory,copy_pinned,ensure_canonical_directory};
use crate::video_interpolation::{RifeFrameIndex,RifeFrame};
use crate::provider::{canonical_json_bytes,canonical_sha256,require_sha256};
use super::{invalid,check_cancelled,UpscaylRecoveryConfiguration};
mod origins;
pub const SCHEMA:&str="aniflow.complete-spatial-carrier/v1";
pub(crate) const DOCUMENT_BYTES:u64=67_108_864;
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeUnitSelection {pub stage_id:String,pub ordinals:Vec<u64>}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum SpatialSelection {
    Historical {run_directory:PathBuf,ordinals:Vec<u64>},
    Native {run_directory:PathBuf,configuration:UpscaylRecoveryConfiguration,units:Vec<NativeUnitSelection>},
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarrierRequest {
    pub schema:String,pub source_video:PathBuf,pub source_index:PathBuf,pub sources:Vec<SpatialSelection>,
    pub output_width:u32,pub output_height:u32,pub maximum_source_bytes:u64,pub maximum_frame_bytes:u64,
    pub maximum_total_frame_bytes:u64,pub maximum_working_bytes:u64,pub minimum_free_bytes:u64,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum SpatialOriginKind {ReviewedHistoricalImport,NativeFrame,NativeChunk}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialOrigin {
    pub kind:SpatialOriginKind,pub run_directory:PathBuf,pub stage_id:String,pub plan_sha256:String,
    pub checkpoint_sha256:String,pub provider_lock_sha256:String,pub report_sha256:String,
    pub import_decision_sha256:Option<String>,pub processing_history:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedSpatialFrame {
    pub ordinal:u64,pub filename:String,pub sha256:String,pub byte_count:u64,
    pub decoded_rgb24_sha256:String,pub source_time:crate::temporal::TimeRange,
    pub input_file:PathBuf,pub origin_index:u32,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialSelectionPlan {
    pub schema:String,pub request:CarrierRequest,pub source_index:RifeFrameIndex,pub source_index_sha256:String,
    pub source_index_bytes:u64,pub origins:Vec<SpatialOrigin>,pub frames:Vec<SelectedSpatialFrame>,
    pub missing:Vec<u64>,pub complete:bool,pub total_bytes:u64,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteSpatialCarrier {
    pub schema:String,pub selection:SpatialSelectionPlan,pub frames_directory:PathBuf,
    pub creative_approval:String,pub native_qualified:bool,
}
impl CarrierRequest {
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>4_194_304{return Err(invalid("spatial carrier request exceeds 4 MiB"));}
        let value:Self=crate::provider::decode_json(bytes,"spatial carrier request")?;value.validate()?;Ok(value)
    }
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.spatial-carrier-request/v1"||self.sources.len()>1024
            ||!(1..=1_099_511_627_776).contains(&self.maximum_source_bytes)
            ||!(1024..=268_435_456).contains(&self.maximum_frame_bytes)
            ||!(1024..=1_099_511_627_776).contains(&self.maximum_total_frame_bytes)
            ||!(1_048_576..=17_592_186_044_416).contains(&self.maximum_working_bytes)||self.minimum_free_bytes>1_099_511_627_776
            ||self.output_width<2||self.output_height<2||self.output_width>16384||self.output_height>16384||self.output_width%2!=0||self.output_height%2!=0
            ||u64::from(self.output_width)*u64::from(self.output_height)>134_217_728 {
            return Err(invalid("complete spatial selection requires explicit finite source/frame/pixel/storage bounds"));
        }
        let mut count=0usize;
        for source in &self.sources {
            match source {
                SpatialSelection::Historical {ordinals:values,..}=>{ordinals(values,128)?;count=count.checked_add(values.len()).ok_or_else(||invalid("spatial selection count overflow"))?;},
                SpatialSelection::Native {configuration,units,..}=>{
                    configuration.validate()?;if units.is_empty()||units.len()>1024{return Err(invalid("select 1..1024 native spatial units"));}
                    for unit in units {ordinals(&unit.ordinals,256)?;count=count.checked_add(unit.ordinals.len()).ok_or_else(||invalid("spatial selection count overflow"))?;}
                },
            }
            if count>100_000{return Err(invalid("spatial selection exceeds 100000 total frames"));}
        }
        Ok(())
    }
}
pub(super) fn ordinals(values:&[u64],maximum:usize)->Result<()> {
    if values.is_empty()||values.len()>maximum||values.windows(2).any(|w|w[0]>=w[1]) {
        return Err(invalid("select a bounded strictly increasing nonempty ordinal list"));
    }Ok(())
}
fn coverage(index:&RifeFrameIndex,frames:&[SelectedSpatialFrame])->Result<Vec<u64>> {
    let mut seen=BTreeSet::new();
    for frame in frames {
        let original=index.frames.get(frame.ordinal as usize).ok_or_else(||invalid("selected spatial ordinal is outside the exact original index"))?;
        let time=index.temporal_inspection.video_timeline().map_err(|e|crate::Error::from_anyhow(crate::ErrorCategory::Media,e))?.frames[frame.ordinal as usize].source_time;
        if !seen.insert(frame.ordinal)||original.ordinal!=frame.ordinal||original.filename!=frame.filename||time!=frame.source_time||frame.byte_count==0 {
            return Err(invalid("spatial selection has duplicate, renamed, retimed or empty frames"));
        }
        require_sha256(&frame.sha256,"selected spatial PNG")?;require_sha256(&frame.decoded_rgb24_sha256,"selected spatial RGB")?;
    }
    Ok(index.frames.iter().filter(|f|!seen.contains(&f.ordinal)).map(|f|f.ordinal).collect())
}
/// Current accepted selected units only. Missing ordinals are exposed rather
/// than substituted, synthesized or treated as a complete index.
pub fn inspect(request:&CarrierRequest,token:&CancellationToken)->Result<SpatialSelectionPlan> {
    check_cancelled(token)?;request.validate()?;
    let (index,pin)=crate::frame_import::read_index(&request.source_index)?;
    if index.frames.len()>100_000||!(2..=4).any(|s|index.width.checked_mul(s)==Some(request.output_width)&&index.height.checked_mul(s)==Some(request.output_height)) {
        return Err(invalid("spatial carrier preserves the original complete clock at uniform explicit 2x/3x/4x geometry"));
    }
    let source=(index.temporal_inspection.source_sha256.clone(),index.temporal_inspection.source_size_bytes);
    if identity(&request.source_video,request.maximum_source_bytes)?!=source{return Err(invalid("spatial carrier original source differs from exact index"));}
    let mut origins=Vec::new();let mut frames=Vec::new();
    for selection in &request.sources {origins::append(selection,request,&index,&pin,&mut origins,&mut frames,token)?;}
    frames.sort_by_key(|f|f.ordinal);let missing=coverage(&index,&frames)?;
    let mut total=0u64;
    for frame in &frames {
        check_cancelled(token)?;
        if inspect_png(&frame.input_file,request.output_width,request.output_height,request.maximum_frame_bytes)?!=(frame.sha256.clone(),frame.byte_count) {
            return Err(invalid("accepted selected spatial PNG changed"));
        }
        total=total.checked_add(frame.byte_count).ok_or_else(||invalid("spatial frame byte sum overflow"))?;
    }
    if total>request.maximum_total_frame_bytes||identity(&request.source_index,16_777_216)?!=pin||identity(&request.source_video,request.maximum_source_bytes)?!=source {
        return Err(invalid("selected spatial bounds/source/index changed"));
    }
    check_cancelled(token)?;
    Ok(SpatialSelectionPlan {schema:"aniflow.spatial-selection-plan/v1".into(),request:request.clone(),source_index:index,
        source_index_sha256:pin.0,source_index_bytes:pin.1,origins,frames,complete:missing.is_empty(),missing,total_bytes:total})
}
pub fn plan(request:&CarrierRequest,token:&CancellationToken)->Result<SpatialSelectionPlan> {
    let value=inspect(request,token)?;
    if !value.complete{return Err(invalid("complete spatial carrier requires every original ordinal exactly once; inspect missing work first"));}Ok(value)
}
fn document<T:Serialize>(value:&T)->Result<Vec<u8>> {
    let bytes=canonical_json_bytes(value)?;if bytes.len() as u64>DOCUMENT_BYTES{return Err(invalid("complete spatial carrier exceeds 64 MiB"));}Ok(bytes)
}
fn write(path:&Path,bytes:&[u8])->Result<()> {
    use std::io::Write;
    let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut file=options.open(path).map_err(|_|invalid("cannot reserve fresh spatial carrier"))?;
    file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|_|invalid("cannot sync spatial carrier"))?;
    local::sync(path.parent().ok_or_else(||invalid("spatial carrier parent missing"))?)
}
/// Fresh copy-only publication. Native sources retain acceptance and history;
/// this composition marker is written last and is not a new media checkpoint.
pub fn create(request:&CarrierRequest,root:&Path,token:&CancellationToken)->Result<CompleteSpatialCarrier> {
    let selection=plan(request,token)?;let parent=local::parent(root)?;
    for input in [&request.source_video,&request.source_index].into_iter().chain(selection.frames.iter().map(|f|&f.input_file)).chain(selection.origins.iter().map(|o|&o.run_directory)) {
        if root.starts_with(input)||input.starts_with(root){return Err(invalid("spatial carrier must be disjoint from original source/index and accepted origins"));}
    }
    let required=selection.total_bytes.checked_add(DOCUMENT_BYTES).ok_or_else(||invalid("carrier storage overflow"))?;
    if required>request.maximum_working_bytes{return Err(invalid("complete spatial carrier exceeds declared working bound"));}
    crate::video_assembly::require_storage(parent,required,request.minimum_free_bytes)?;
    local::directory(root)?;let frames_directory=root.join("frames");local::directory(&frames_directory)?;
    for frame in &selection.frames {copy_pinned(&frame.input_file,&frames_directory.join(&frame.filename),&(frame.sha256.clone(),frame.byte_count),request.maximum_frame_bytes,token)?;}
    if plan(request,token)?!=selection{return Err(invalid("spatial origins changed during complete carrier copy; retain partial root"));}
    let value=CompleteSpatialCarrier {schema:SCHEMA.into(),selection,frames_directory,creative_approval:"not_performed; individual import declarations do not approve complete movie".into(),native_qualified:false};
    inventory(&value.frames_directory,&value.selection.frames.iter().map(|f|f.filename.clone().into()).collect::<Vec<_>>())?;
    check_cancelled(token)?;write(&root.join("carrier.json"),&document(&value)?)?;Ok(value)
}
impl CompleteSpatialCarrier {
    pub fn validate_shape(&self)->Result<()> {
        let selection=&self.selection;selection.request.validate()?;selection.source_index.validate()?;
        let total=selection.frames.iter().try_fold(0u64,|sum,f|sum.checked_add(f.byte_count).ok_or_else(||invalid("spatial carrier byte sum overflow")))?;
        if self.schema!=SCHEMA||selection.schema!="aniflow.spatial-selection-plan/v1"||!selection.complete||!selection.missing.is_empty()
            ||!coverage(&selection.source_index,&selection.frames)?.is_empty()||selection.frames.windows(2).any(|w|w[0].ordinal>=w[1].ordinal)
            ||selection.frames.iter().any(|f|f.origin_index as usize>=selection.origins.len())||total!=selection.total_bytes||total>selection.request.maximum_total_frame_bytes
            ||selection.origins.is_empty()||selection.origins.len()>100_000||self.native_qualified
            ||self.creative_approval!="not_performed; individual import declarations do not approve complete movie" {
            return Err(invalid("not a complete source-bound spatial carrier"));
        }Ok(())
    }
    pub fn frames(&self)->Vec<RifeFrame> {self.selection.frames.iter().map(|f|RifeFrame {ordinal:f.ordinal,filename:f.filename.clone(),sha256:f.sha256.clone()}).collect()}
    pub fn validate_binding(&self,token:&CancellationToken)->Result<()> {
        self.validate_shape()?;ensure_canonical_directory(&self.frames_directory)?;
        if self.frames_directory.file_name().and_then(|v|v.to_str())!=Some("frames")||plan(&self.selection.request,token)?!=self.selection {
            return Err(invalid("complete spatial carrier differs from current accepted origins/source"));
        }
        let root=self.frames_directory.parent().ok_or_else(||invalid("spatial carrier root missing"))?;
        local::entries(root,&["carrier.json"],&["frames"],true)?;
        let bytes=document(self)?;
        if identity(&root.join("carrier.json"),DOCUMENT_BYTES)?!=(local::hash(&bytes),bytes.len() as u64){return Err(invalid("complete carrier marker differs from selected exact bytes"));}
        inventory(&self.frames_directory,&self.selection.frames.iter().map(|f|f.filename.clone().into()).collect::<Vec<_>>())?;
        for frame in &self.selection.frames {check_cancelled(token)?;if inspect_png(&self.frames_directory.join(&frame.filename),self.selection.request.output_width,self.selection.request.output_height,self.selection.request.maximum_frame_bytes)?!=(frame.sha256.clone(),frame.byte_count){return Err(invalid("complete carrier copied PNG changed"));}}
        Ok(())
    }
}
pub fn observe(root:&Path,carrier_sha256:&str,token:&CancellationToken)->Result<CompleteSpatialCarrier> {
    require_sha256(carrier_sha256,"complete carrier external pin")?;ensure_canonical_directory(root)?;
    let path=root.join("carrier.json");let pin=identity(&path,DOCUMENT_BYTES)?;
    if pin.0!=carrier_sha256{return Err(invalid("complete carrier differs from external digest"));}
    let value:CompleteSpatialCarrier=local::read_bounded(&path,DOCUMENT_BYTES)?;
    if value.frames_directory!=root.join("frames"){return Err(invalid("complete carrier points outside selected root"));}
    value.validate_binding(token)?;
    if identity(&path,DOCUMENT_BYTES)?!=pin{return Err(invalid("complete carrier changed during observation"));}Ok(value)
}
#[cfg(test)]
mod tests;
