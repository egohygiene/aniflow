//! A private inert package, with completion last and caller-pinned readback.
use super::*;
use std::{collections::BTreeMap,path::{Path,PathBuf}};
use crate::{CancellationToken,frame_io::identity,destination::local};
const MANIFEST:&str="workflow-bundle.json";
const REQUEST:&str="setup-request.json";
const PREPARATION:&str="setup-preparation.json";
const MAX_TOTAL:u64=16_777_216;
const AUTHORITY:&str="none; local configuration package only";
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleFile {pub path:String,pub sha256:String,pub byte_count:u64}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    pub schema:String,pub request_sha256:String,pub preparation_sha256:String,pub files:Vec<BundleFile>,
    pub selected_roles:Vec<String>,pub omitted_roles:Vec<String>,pub tools_or_models_copied:bool,
    pub provider_registration_performed:bool,pub native_qualified:bool,pub execution_authority:String,pub publication_authority:String,
}
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleReadback {
    pub schema:String,pub manifest_sha256:String,pub request_sha256:String,pub preparation_sha256:String,
    pub file_count:usize,pub selected_roles:Vec<String>,pub retained_integrity:String,pub current_dependencies:String,
    pub native_qualified:bool,pub execution_authority:String,pub publication_authority:String,
}
struct Documents {manifest:BundleManifest,files:BTreeMap<String,Vec<u8>>}
fn encode<T:Serialize>(value:&T,maximum:usize)->Result<Vec<u8>> {let b=crate::provider::canonical_json_bytes(value)?;if b.len()>maximum{return Err(invalid("workflow package document exceeds its fixed byte bound"));}Ok(b)}
fn documents(request:&WorkflowSetupRequest,prepared:&WorkflowSetupPreparation)->Result<Documents> {
    request.validate()?;
    if prepared.schema!="aniflow.toolchain.workflow-setup-preparation/v1"||prepared.request_sha256!=crate::provider::canonical_sha256(request)?
        ||!prepared.ready||prepared.native_qualification||prepared.host_runtime_evidence!="unobserved"||prepared.source_media_evidence!="unobserved"
        ||prepared.provider_evidence!="unobserved; actual domain planning/execution supplies native identity"
        ||prepared.execution_authority!="none; selection order is not a runtime plan"||prepared.publication_authority!="none"
        ||!prepared.diagnostics.is_empty()||prepared.selections.len()!=request.selections.len() {return Err(invalid("only a complete ready inert workflow preparation can be packaged"));}
    let mut files=BTreeMap::new();files.insert(REQUEST.into(),encode(request,1_048_576)?);files.insert(PREPARATION.into(),encode(prepared,2_097_152)?);
    let mut selected_roles=Vec::new();
    for (choice,step) in request.selections.iter().zip(&prepared.selections) {
        let role=super::role(&choice.selection);
        if step.role!=role||step.capability_id!=choice.capability_id||step.selection!=choice.selection||!step.preparation.ready||step.preparation.native_qualification
            ||!step.preparation.inspection.ready||step.preparation.host_runtime_evidence!="unobserved"
            ||step.preparation.request_sha256!=crate::provider::canonical_sha256(&request.selected(choice))?
            ||step.preparation.files.len()!=1||step.preparation.files[0].relative_path!="configuration.json"||!step.preparation.diagnostics.is_empty(){return Err(invalid("workflow package role/capability/configuration differs from exact preparation"));}
        let file=&step.preparation.files[0];let bytes=encode(&file.content,1_048_576)?;
        if local::hash(&bytes)!=file.sha256{return Err(invalid("prepared configuration bytes differ from declared digest"));}
        if files.insert(format!("{role}/configuration.json"),bytes).is_some(){return Err(invalid("workflow package repeats a domain role"));}
        selected_roles.push(role.to_owned());
    }
    let omitted=ROLES.iter().filter(|r|!selected_roles.iter().any(|s|s.as_str()==**r)).map(|r|r.to_string()).collect::<Vec<_>>();
    if omitted!=prepared.omitted_roles{return Err(invalid("workflow package omitted roles differ"));}
    let mut total=0u64;let mut subjects=Vec::new();
    for (path,bytes) in &files {total=total.checked_add(bytes.len() as u64).filter(|n|*n<=MAX_TOTAL).ok_or_else(||invalid("workflow package exceeds 16 MiB"))?;subjects.push(BundleFile {path:path.clone(),sha256:local::hash(bytes),byte_count:bytes.len() as u64});}
    let manifest=BundleManifest {schema:"aniflow.toolchain-workflow-bundle/v1".into(),request_sha256:local::hash(&files[REQUEST]),preparation_sha256:local::hash(&files[PREPARATION]),files:subjects,
        selected_roles,omitted_roles:omitted,tools_or_models_copied:false,provider_registration_performed:false,native_qualified:false,execution_authority:AUTHORITY.into(),publication_authority:"none".into()};
    encode(&manifest,262_144)?;Ok(Documents {manifest,files})
}
fn disjoint(request:&WorkflowSetupRequest,root:&Path)->Result<()> {
    for file in &request.inventory.artifacts {if root.starts_with(&file.path)||file.path.starts_with(root){return Err(invalid("workflow package must be separate from dependency locators"));}}Ok(())
}
fn inventory(root:&Path,manifest:&BundleManifest)->Result<()> {
    let roles=manifest.selected_roles.iter().map(String::as_str).collect::<Vec<_>>();
    local::entries(root,&[MANIFEST,REQUEST,PREPARATION],&roles,false)?;
    for role in &roles {if !ROLES.contains(role){return Err(invalid("workflow package contains an unknown role"));}local::entries(&root.join(role),&["configuration.json"],&[],false)?;}Ok(())
}
/// Capture only selected ready configuration documents. No tool/model copy,
/// installation, discovery, registration, provider process or source media.
pub fn create(request:&WorkflowSetupRequest,root:&Path,token:&CancellationToken)->Result<BundleReadback> {
    crate::destination::cancelled(token)?;local::parent(root)?;request.validate()?;disjoint(request,root)?;
    let prepared=prepare_workflow_setup(request)?;let docs=documents(request,&prepared)?;
    crate::destination::cancelled(token)?;
    if prepare_workflow_setup(request)?!=prepared{return Err(invalid("workflow dependencies changed before package creation"));}
    local::directory(root)?;
    for role in &docs.manifest.selected_roles {local::directory(&root.join(role))?;}
    for (name,bytes) in &docs.files {crate::destination::cancelled(token)?;local::write(&root.join(name),bytes)?;}
    crate::destination::cancelled(token)?;
    if prepare_workflow_setup(request)?!=prepared{return Err(invalid("workflow dependencies changed while writing; preserve incomplete package"));}
    crate::destination::cancelled(token)?;let bytes=encode(&docs.manifest,262_144)?;local::write(&root.join(MANIFEST),&bytes)?;
    observe(root,&local::hash(&bytes),true,token)
}
/// The caller's manifest pin binds local retained bytes, not authorship or
/// native suitability. Optional dependency reobservation runs no tools.
pub fn observe(root:&Path,manifest_sha256:&str,reobserve_dependencies:bool,token:&CancellationToken)->Result<BundleReadback> {
    crate::destination::cancelled(token)?;crate::provider::require_sha256(manifest_sha256,"workflow bundle manifest")?;
    let manifest_pin=identity(&root.join(MANIFEST),262_144)?;
    if manifest_pin.0!=manifest_sha256{return Err(invalid("workflow bundle differs from external manifest pin"));}
    let manifest:BundleManifest=local::read_bounded(&root.join(MANIFEST),262_144)?;
    if manifest.schema!="aniflow.toolchain-workflow-bundle/v1"||!(1..=11).contains(&manifest.selected_roles.len())||manifest.files.len()!=manifest.selected_roles.len()+2
        ||manifest.tools_or_models_copied||manifest.provider_registration_performed||manifest.native_qualified||manifest.execution_authority!=AUTHORITY||manifest.publication_authority!="none" {return Err(invalid("workflow bundle contract or authority differs"));}
    // Bound and confine the fixed role inventory before reading retained files.
    let role_set=manifest.selected_roles.iter().collect::<BTreeSet<_>>();
    if role_set.len()!=manifest.selected_roles.len()||manifest.selected_roles.iter().any(|r|!ROLES.contains(&r.as_str())){return Err(invalid("workflow bundle has duplicate or unknown roles"));}
    inventory(root,&manifest)?;
    let request:WorkflowSetupRequest=local::read_bounded(&root.join(REQUEST),1_048_576)?;
    let prepared:WorkflowSetupPreparation=local::read_bounded(&root.join(PREPARATION),2_097_152)?;
    let expected=documents(&request,&prepared)?;
    if manifest!=expected.manifest{return Err(invalid("workflow package manifest differs from retained request/preparation"));}
    let mut pins=Vec::<(PathBuf,(String,u64))>::new();
    for subject in &manifest.files {
        crate::destination::cancelled(token)?;let path=root.join(&subject.path);let pin=identity(&path,2_097_152)?;
        if pin!=(subject.sha256.clone(),subject.byte_count)||expected.files.get(&subject.path).is_none_or(|b|local::hash(b)!=pin.0||b.len() as u64!=pin.1){return Err(invalid("workflow package document changed or differs from exact retained configuration"));}pins.push((path,pin));
    }
    let current_dependencies=if reobserve_dependencies {match prepare_workflow_setup(&request){Ok(current) if current==prepared=>"current_preparation_matches",_=>"changed_or_unavailable"}}else{"not_reobserved"};
    crate::destination::cancelled(token)?;
    for (path,pin) in &pins {if identity(path,2_097_152)?!=*pin{return Err(invalid("workflow package changed during observation"));}}
    if identity(&root.join(MANIFEST),262_144)?!=manifest_pin{return Err(invalid("workflow manifest changed during observation"));}inventory(root,&manifest)?;
    Ok(BundleReadback {schema:"aniflow.toolchain-workflow-bundle-readback/v1".into(),manifest_sha256:manifest_sha256.into(),request_sha256:manifest.request_sha256,preparation_sha256:manifest.preparation_sha256,
        file_count:manifest.files.len(),selected_roles:manifest.selected_roles,retained_integrity:"intact".into(),current_dependencies:current_dependencies.into(),native_qualified:false,execution_authority:AUTHORITY.into(),publication_authority:"none".into()})
}

#[cfg(test)] mod tests;
