//! One explicit selection over existing inert per-domain setup owners.
use super::*;
const ROLES:[&str;11]=["frame_extraction","image_repair","upscale","interpolation","clean_video_assembly","caption_rendering","metadata_remux","destination_video","lossless_audio_export","final_video_audio_qc","final_lossless_audio_qc"];
fn role(selection:&MediaSetupSelection)->&'static str {match selection {
    MediaSetupSelection::FrameExtraction {..}=>ROLES[0],MediaSetupSelection::AllenkImageRepair {..}=>ROLES[1],MediaSetupSelection::UpscaylRecovery {..}=>ROLES[2],
    MediaSetupSelection::RifeInterpolation {..}=>ROLES[3],MediaSetupSelection::CleanVideoAssembly {..}=>ROLES[4],MediaSetupSelection::CaptionRendering {..}=>ROLES[5],
    MediaSetupSelection::MetadataRemux {..}=>ROLES[6],MediaSetupSelection::DestinationVideo {..}=>ROLES[7],MediaSetupSelection::LosslessAudioExport {..}=>ROLES[8],
    MediaSetupSelection::FinalVideoAudioQc {..}=>ROLES[9],MediaSetupSelection::FinalLosslessAudioQc {..}=>ROLES[10],
}}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSetupRequest {pub schema:String,pub profile:ToolchainProfile,pub inventory:ToolchainInventory,pub selections:Vec<PublishingSetupChoice>}
impl WorkflowSetupRequest {
    pub fn load(path:impl AsRef<Path>)->Result<Self>{Self::from_json_slice(&super::super::types::read_json(path.as_ref())?)}
    pub fn from_json_slice(bytes:&[u8])->Result<Self> {
        if bytes.len()>MAXIMUM_REQUEST_BYTES{return Err(invalid("workflow setup exceeds one MiB"));}
        let value:Self=super::super::types::decode_unique_json(bytes)?;value.validate()?;Ok(value)
    }
    fn selected(&self,choice:&PublishingSetupChoice)->MediaSetupRequest {MediaSetupRequest {schema:MEDIA_SETUP_REQUEST_SCHEMA.into(),profile:self.profile.clone(),inventory:self.inventory.clone(),capability_id:choice.capability_id.clone(),selection:choice.selection.clone()}}
    pub fn validate(&self)->Result<()> {
        if self.schema!="aniflow.toolchain.workflow-setup/v1"||!(1..=11).contains(&self.selections.len()){return Err(invalid("workflow preparation requires 1..11 explicit domain roles"));}
        let mut ids=BTreeSet::new();let mut roles=BTreeSet::new();
        for choice in &self.selections {
            if !ids.insert(&choice.capability_id)||!roles.insert(role(&choice.selection)){return Err(invalid("workflow selection repeats a capability ID or domain role"));}
            self.selected(choice).validate()?;
        }Ok(())
    }
}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSetupStep {pub role:String,pub capability_id:String,pub selection:MediaSetupSelection,pub preparation:MediaSetupPreparation}
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSetupPreparation {
    pub schema:String,pub request_sha256:String,pub ready:bool,pub selections:Vec<WorkflowSetupStep>,pub omitted_roles:Vec<String>,pub diagnostics:Vec<String>,
    pub native_qualification:bool,pub host_runtime_evidence:String,pub source_media_evidence:String,pub provider_evidence:String,pub execution_authority:String,pub publication_authority:String,
}
/// Read only selected dependency identities and declarations. No media, process,
/// model discovery, registration or configuration write is performed.
pub fn prepare_workflow_setup(request:&WorkflowSetupRequest)->Result<WorkflowSetupPreparation> {
    request.validate()?;if crate::provider::canonical_json_bytes(request)?.len()>MAXIMUM_REQUEST_BYTES{return Err(invalid("workflow setup exceeds one MiB"));}
    let mut selections=Vec::new();let mut diagnostics=Vec::new();let mut selected=BTreeSet::new();
    for choice in &request.selections {
        let name=role(&choice.selection);selected.insert(name);
        let preparation=prepare_media_setup(&request.selected(choice))?;
        diagnostics.extend(preparation.diagnostics.iter().map(|d|format!("{name} / {}: {d}",choice.capability_id)));
        selections.push(WorkflowSetupStep {role:name.into(),capability_id:choice.capability_id.clone(),selection:choice.selection.clone(),preparation});
    }
    for (choice,prepared) in request.selections.iter().zip(&selections) {
        if prepare_media_setup(&request.selected(choice))?!=prepared.preparation{return Err(invalid("selected workflow dependencies changed during preparation; no stable whole-selection report"));}
    }
    let value=WorkflowSetupPreparation {schema:"aniflow.toolchain.workflow-setup-preparation/v1".into(),request_sha256:crate::provider::canonical_sha256(request)?,ready:selections.iter().all(|s|s.preparation.ready),selections,
        omitted_roles:ROLES.iter().filter(|name|!selected.contains(**name)).map(|s|s.to_string()).collect(),diagnostics,native_qualification:false,
        host_runtime_evidence:"unobserved".into(),source_media_evidence:"unobserved".into(),provider_evidence:"unobserved; actual domain planning/execution supplies native identity".into(),execution_authority:"none; selection order is not a runtime plan".into(),publication_authority:"none".into()};
    if crate::provider::canonical_json_bytes(&value)?.len()>MAXIMUM_PREPARATION_BYTES{return Err(invalid("workflow setup preparation exceeds eight MiB"));}Ok(value)
}
#[cfg(test)]
mod tests;
