use super::*;
fn disjoint(root: &Path, record: &SubjectRecord) -> Result<()> {
    let file = source(&record.origin)?;
    if root.starts_with(&file) || file.starts_with(root) {
        return Err(invalid(
            "release root must be separate from selected sources",
        ));
    }
    match &record.origin {
        SubjectOrigin::AcceptedMedia { observation } => {
            local::disjoint(root, &observation.input, &file)
        }
        SubjectOrigin::PostingMember { directory, .. }
            if root.starts_with(directory) || directory.starts_with(root) =>
        {
            Err(invalid(
                "release root must be separate from its posting package",
            ))
        }
        _ => Ok(()),
    }
}
fn inventory(root: &Path, manifest: &SubjectManifest, sealed: bool, writer: bool) -> Result<()> {
    let files = if sealed {
        vec!["request.json", "subjects.json", "seal.json"]
    } else {
        vec!["request.json", "subjects.json"]
    };
    local::entries(root, &files, &["subjects", "observations"], writer)?;
    let names: Vec<_> = manifest
        .subjects
        .iter()
        .map(|s| {
            s.path
                .strip_prefix("subjects/")
                .ok_or_else(|| invalid("invalid fixed subject path"))
        })
        .collect::<Result<_>>()?;
    local::entries(&root.join("subjects"), &names, &[], false)
}
fn check_copies(
    root: &Path,
    manifest: &SubjectManifest,
    maximum: u64,
    token: &CancellationToken,
) -> Result<()> {
    for record in &manifest.subjects {
        cancelled(token)?;
        if identity(&root.join(&record.path), maximum)?
            != (record.sha256.clone(), record.byte_count)
        {
            return Err(invalid(
                "retained subject bytes differ from the pinned manifest",
            ));
        }
    }
    Ok(())
}
/// Retain selected exact bytes in a fresh private root. Completion marker last;
/// failure leaves evidence in place without adopting or cleaning a partial root.
pub fn seal(
    request: &EvidenceRequest,
    root: &Path,
    token: &CancellationToken,
) -> Result<SealReceipt> {
    let planned = plan(request, token)?;
    let parent = local::parent(root)?;
    for record in &planned.manifest.subjects {
        disjoint(root, record)?;
    }
    crate::video_assembly::require_storage(
        parent,
        planned.projected_bundle_bytes,
        request.minimum_free_bytes,
    )?;
    cancelled(token)?;
    local::directory(root)?;
    local::sync(parent)?;
    let _writer = crate::cache_v3::ExclusiveWriter::acquire(root, "writer.lock")?;
    local::directory(&root.join("subjects"))?;
    local::directory(&root.join("observations"))?;
    local::write(&root.join("request.json"), &local::bytes(request)?)?;
    for record in &planned.manifest.subjects {
        let copy = root.join(&record.path);
        crate::frame_io::copy_pinned(
            &source(&record.origin)?,
            &copy,
            &(record.sha256.clone(), record.byte_count),
            request.maximum_bundle_bytes,
            token,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| invalid("cannot retain private subject permissions"))?;
            std::fs::File::open(&copy)
                .and_then(|f| f.sync_all())
                .map_err(|_| invalid("cannot sync retained subject permissions"))?;
        }
    }
    // Reobserve actual producer authority and source bytes before acceptance.
    if plan(request, token)? != planned {
        return Err(invalid(
            "selected sources or their acceptance changed during retention",
        ));
    }
    check_copies(root, &planned.manifest, request.maximum_bundle_bytes, token)?;
    local::write(
        &root.join("subjects.json"),
        &local::bytes(&planned.manifest)?,
    )?;
    let saved: EvidenceRequest = read_exact(&root.join("request.json"), local::DOCUMENT_BYTES)?;
    if saved != *request {
        return Err(invalid("private selection changed during retention"));
    }
    inventory(root, &planned.manifest, false, true)?;
    local::entries(&root.join("observations"), &[], &[], false)?;
    local::sync(&root.join("subjects"))?;
    local::sync(&root.join("observations"))?;
    cancelled(token)?;
    let receipt = receipt(&planned.manifest)?;
    local::write(&root.join("seal.json"), &local::bytes(&receipt)?)?;
    Ok(receipt)
}
fn bindings(request: &EvidenceRequest, manifest: &SubjectManifest) -> Result<()> {
    if manifest.schema != "aniflow.release-subjects/v1"
        || manifest.release_id != request.release_id
        || manifest.version != request.version
        || manifest.request_sha256 != local::hash(&local::bytes(request)?)
        || manifest.subjects.len() != request.subjects.len()
        || manifest.creative_approval != "not_performed"
        || manifest.ownership != "not_established"
        || manifest.publication_authority != "none"
    {
        return Err(invalid(
            "subject manifest does not match its fixed private selection contract",
        ));
    }
    for (index, (selection, record)) in request.subjects.iter().zip(&manifest.subjects).enumerate()
    {
        if record.id != selection.id
            || record.role != selection.role
            || record.visibility != selection.visibility
            || record.parents != selection.parents
            || record.path != format!("subjects/{index:04}-{}.bin", selection.id)
            || !digest(&record.sha256)
            || record.byte_count > MAXIMUM_BYTES
        {
            return Err(invalid(
                "subject manifest has a changed selection, lineage, path or identity",
            ));
        }
        let bound = match (&selection.input, &record.origin) {
            (
                SubjectInput::AcceptedMedia { input },
                SubjectOrigin::AcceptedMedia { observation },
            ) => {
                observation.input == *input
                    && observation.file.sha256 == record.sha256
                    && observation.file.byte_count == record.byte_count
            }
            (
                SubjectInput::PostingMember { directory, member },
                SubjectOrigin::PostingMember {
                    directory: saved,
                    member: chosen,
                    receipt,
                    receipt_sha256,
                },
            ) => {
                let selected = posting_member(receipt, *chosen)?;
                directory == saved
                    && member == chosen
                    && digest(receipt_sha256)
                    && selected.sha256 == record.sha256
                    && selected.byte_count == record.byte_count
            }
            (
                SubjectInput::DeclaredFile {
                    file,
                    expected_sha256,
                    maximum_bytes,
                    declared_origin,
                },
                SubjectOrigin::DeclaredFile {
                    locator,
                    declared_origin: saved,
                    technical_acceptance,
                },
            ) => {
                file == locator
                    && expected_sha256 == &record.sha256
                    && record.byte_count <= *maximum_bytes
                    && declared_origin == saved
                    && technical_acceptance == "not_established; history is operator_declared"
            }
            _ => false,
        };
        if !bound {
            return Err(invalid(
                "subject origin differs from its explicit retained selection",
            ));
        }
    }
    projected(request, manifest)?;
    Ok(())
}
pub(super) fn verify_inner(
    root: &Path,
    pin: &str,
    token: &CancellationToken,
    writer: bool,
) -> Result<ByteVerification> {
    cancelled(token)?;
    if !digest(pin) {
        return Err(invalid(
            "archival verification needs a supplied known lowercase manifest SHA-256",
        ));
    }
    ensure_canonical_directory(root)?;
    let path = root.join("subjects.json");
    let manifest: SubjectManifest = read_exact(&path, local::DOCUMENT_BYTES)?;
    if local::hash(&local::bytes(&manifest)?) != pin
        || identity(&path, local::DOCUMENT_BYTES)?.0 != pin
    {
        return Err(invalid(
            "subject manifest differs from the supplied external digest",
        ));
    }
    let request: EvidenceRequest = read_exact(&root.join("request.json"), local::DOCUMENT_BYTES)?;
    request.validate()?;
    bindings(&request, &manifest)?;
    let saved: SealReceipt = read_exact(&root.join("seal.json"), local::DOCUMENT_BYTES)?;
    if saved != receipt(&manifest)? {
        return Err(invalid(
            "completion marker differs from the exact retained subject manifest",
        ));
    }
    inventory(root, &manifest, true, writer)?;
    check_copies(root, &manifest, request.maximum_bundle_bytes, token)?;
    let result = ByteVerification {
        schema: "aniflow.release-byte-verification/v1".into(),
        subject_manifest_sha256: pin.into(),
        subject_count: saved.subject_count,
        subject_bytes: saved.subject_bytes,
        state: "exact_bytes_match_supplied_subject_digest".into(),
        producer_acceptance: "not_reobserved_by_archival_byte_check".into(),
        timestamp_state: "not_verified".into(),
        ownership: "not_established".into(),
    };
    observations::chain(root, &result, token)?;
    if identity(&path, local::DOCUMENT_BYTES)?.0 != pin {
        return Err(invalid(
            "subject manifest changed during archival verification",
        ));
    }
    cancelled(token)?;
    Ok(result)
}
/// Read-only byte check against a caller-supplied known manifest digest. Does
/// not require surviving originals or reassert historical producer acceptance.
pub fn verify(
    root: &Path,
    manifest_sha256: &str,
    token: &CancellationToken,
) -> Result<ByteVerification> {
    verify_inner(root, manifest_sha256, token, false)
}
