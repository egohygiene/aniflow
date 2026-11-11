use super::*;
/// Create one new private packet. Only PUBLIC_PATH is the approved field
/// projection; the enclosing request and receipt remain private evidence.
pub fn create(request: &HandoffRequest, root: &Path, token: &CancellationToken) -> Result<HandoffStatus> {
    cancelled(token)?;request.validate()?;let parent=local::parent(root)?;
    for source in std::iter::once(&request.bundle_directory).chain(request.subjects.iter().filter_map(|s|s.proof.as_ref().map(|p|&p.directory))) {
        if root.starts_with(source)||source.starts_with(root) {return Err(invalid("handoff root overlaps its private archive or proof evidence"));}
    }
    let expected=plan(request,token)?;
    crate::video_assembly::require_storage(parent,8_388_608,request.minimum_free_bytes)?;
    cancelled(token)?;local::directory(root)?;local::sync(parent)?;
    local::write(&root.join("request.json"),&local::bytes(request)?)?;
    local::directory(&root.join("public"))?;
    local::write(&root.join(PUBLIC_PATH),&local::bytes(&expected.public_handoff)?)?;
    if plan(request,token)?!=expected {return Err(invalid("private archive or proof interpretation changed while preparing handoff; preserve partial packet"));}
    if read_exact::<HandoffRequest>(&root.join("request.json"),131_072)?!=*request
        || identity(&root.join(PUBLIC_PATH),local::DOCUMENT_BYTES)?!=(expected.public_handoff_sha256.clone(),expected.public_handoff_byte_count) {
        return Err(invalid("handoff selections or public bytes changed before completion receipt"));
    }
    local::entries(root,&["request.json"],&["public"],false)?;
    local::entries(&root.join("public"),&["release-evidence.json"],&[],false)?;
    let receipt=receipt(&expected);let bytes=local::bytes(&receipt)?;
    cancelled(token)?;local::write(&root.join("receipt.json"),&bytes)?;
    observe(root,&local::hash(&bytes),token)
}
/// Current private-owner readback reobserves upstream bytes. A portable consumer
/// with no private sources uses PublicHandoff::from_pinned_json instead.
pub fn observe(root: &Path, receipt_sha256: &str, token: &CancellationToken) -> Result<HandoffStatus> {
    cancelled(token)?;require_sha256(receipt_sha256,"handoff receipt external digest")?;
    local::entries(root,&["request.json","receipt.json"],&["public"],false)?;
    local::entries(&root.join("public"),&["release-evidence.json"],&[],false)?;
    let receipt_path=root.join("receipt.json");
    if identity(&receipt_path,local::DOCUMENT_BYTES)?.0!=receipt_sha256 {return Err(invalid("handoff private receipt differs from external pin"));}
    let request:HandoffRequest=read_exact(&root.join("request.json"),131_072)?;
    let saved:HandoffReceipt=read_exact(&receipt_path,local::DOCUMENT_BYTES)?;
    let expected=plan(&request,token)?;
    if saved!=receipt(&expected) {return Err(invalid("handoff receipt differs from current exact selected archive/proof context"));}
    let public_path=root.join(PUBLIC_PATH);
    let public:PublicHandoff=read_exact(&public_path,local::DOCUMENT_BYTES)?;
    public.validate()?;
    if public!=expected.public_handoff || identity(&public_path,local::DOCUMENT_BYTES)?!=(saved.public_handoff_sha256.clone(),saved.public_handoff_byte_count)
        || read_exact::<HandoffRequest>(&root.join("request.json"),131_072)?!=request || read_exact::<HandoffReceipt>(&receipt_path,local::DOCUMENT_BYTES)?!=saved
        || identity(&receipt_path,local::DOCUMENT_BYTES)?.0!=receipt_sha256 || plan(&request,token)?!=expected {
        return Err(invalid("public handoff packet or private source changed during current observation"));
    }
    local::entries(root,&["request.json","receipt.json"],&["public"],false)?;
    local::entries(&root.join("public"),&["release-evidence.json"],&[],false)?;
    cancelled(token)?;
    Ok(HandoffStatus {schema:"aniflow.release-evidence-handoff-status/v1".into(),receipt_sha256:receipt_sha256.into(),receipt:saved,public_handoff:public})
}
