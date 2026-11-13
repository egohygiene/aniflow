//! Synthetic archive/proof/public projections; authored uncompiled/unrun.
use super::*;
fn proof_selection(
    bundle: &Path,
    manifest_sha256: &str,
    subject: detached::ProofSubject,
    label: &str,
    version: u8,
) -> ProofSelection {
    let manifest: SubjectManifest =
        read_exact(&bundle.join("subjects.json"), local::DOCUMENT_BYTES).unwrap();
    let hash = match &subject {
        detached::ProofSubject::Manifest {} => manifest_sha256.to_owned(),
        detached::ProofSubject::Subject { id } => manifest
            .subjects
            .iter()
            .find(|s| &s.id == id)
            .unwrap()
            .sha256
            .clone(),
    };
    let root = bundle.parent().unwrap();
    let file = root.join(format!("{label}.ots"));
    let mut bytes = b"\x00OpenTimestamps\x00\x00Proof\x00\xbf\x89\xe2\xe8\x84\xe8\x92\x94".to_vec();
    bytes.extend([version, 8]);
    for i in (0..64).step_by(2) {
        bytes.push(u8::from_str_radix(&hash[i..i + 2], 16).unwrap());
    }
    let uri = b"https://private-calendar.example";
    bytes.extend([
        0xff,
        0,
        0x83,
        0xdf,
        0xe3,
        0x0d,
        0x2e,
        0xf9,
        0x0c,
        0x8e,
        (uri.len() + 1) as u8,
        uri.len() as u8,
    ]);
    bytes.extend(uri);
    bytes.extend([8, 0, 0x05, 0x88, 0x96, 0x0d, 0x73, 0xd7, 0x19, 0x01, 1, 100]);
    std::fs::write(&file, &bytes).unwrap();
    let request = detached::ProofRetentionRequest {
        schema: "aniflow.detached-proof-retention-request/v1".into(),
        bundle_directory: bundle.to_owned(),
        manifest_sha256: manifest_sha256.into(),
        subject,
        proof_file: file,
        expected_proof_sha256: local::hash(&bytes),
        previous: None,
        maximum_subject_bytes: 2_097_152,
        minimum_free_bytes: 0,
    };
    let directory = root.join(label);
    let receipt = detached::retain(&request, &directory, &CancellationToken::new()).unwrap();
    ProofSelection {
        directory,
        receipt_sha256: local::hash(&local::bytes(&receipt).unwrap()),
    }
}
fn fixture(proof: bool) -> (tempfile::TempDir, HandoffRequest, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut subjects = vec![];
    for (i, id, content, visibility, role, parents) in [
        (
            0,
            "private-parent",
            b"private-creation-history".as_slice(),
            MemberVisibility::Private,
            SubjectRole::Original,
            vec![],
        ),
        (
            1,
            "selected-cover",
            b"public cover".as_slice(),
            MemberVisibility::Public,
            SubjectRole::Milestone,
            vec!["private-parent".into()],
        ),
        (
            2,
            "selected-final",
            b"public final".as_slice(),
            MemberVisibility::Public,
            SubjectRole::FinalDelivery,
            vec!["private-parent".into(), "selected-cover".into()],
        ),
        (
            3,
            "unselected-public",
            b"unselected sidecar".as_slice(),
            MemberVisibility::Public,
            SubjectRole::Sidecar,
            vec![],
        ),
    ] {
        let file = root.join(format!("source-{i}.txt"));
        std::fs::write(&file, content).unwrap();
        subjects.push(SubjectSelection {
            id: id.into(),
            role,
            visibility,
            parents,
            input: SubjectInput::DeclaredFile {
                file,
                expected_sha256: local::hash(content),
                maximum_bytes: 4096,
                declared_origin: "PRIVATE operator history".into(),
            },
        });
    }
    let evidence = EvidenceRequest {
        schema: "aniflow.release-evidence-request/v1".into(),
        release_id: "public-pilot".into(),
        version: "v1".into(),
        subjects,
        maximum_bundle_bytes: 33_554_432,
        minimum_free_bytes: 0,
    };
    let bundle_directory = root.join("archive");
    let seal = super::super::seal(&evidence, &bundle_directory, &CancellationToken::new()).unwrap();
    let selected_proof = proof.then(|| {
        proof_selection(
            &bundle_directory,
            &seal.subject_manifest_sha256,
            detached::ProofSubject::Subject {
                id: "selected-final".into(),
            },
            "file-proof",
            1,
        )
    });
    let request = HandoffRequest {
        schema: REQUEST.into(),
        bundle_directory,
        manifest_sha256: seal.subject_manifest_sha256,
        subjects: vec![
            Disclosure {
                id: "selected-final".into(),
                proof: selected_proof,
            },
            Disclosure {
                id: "selected-cover".into(),
                proof: None,
            },
        ],
        maximum_archive_subject_bytes: 1_048_576,
        maximum_proof_bytes: 1_048_576,
        minimum_free_bytes: 0,
    };
    (temp, request, root.join("handoff"))
}
#[test]
fn only_selected_public_fields_and_public_parent_edges_leave_the_archive() {
    let (_temp, r, _) = fixture(true);
    let p = plan(&r, &CancellationToken::new()).unwrap();
    let public = &p.public_handoff;
    assert_eq!(
        public
            .subjects
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec!["selected-cover", "selected-final"]
    );
    assert!(public.subjects[0].parents.is_empty());
    assert_eq!(public.subjects[1].parents, vec!["selected-cover"]);
    let proof = public.subjects[1].proof.as_ref().unwrap();
    assert_eq!(proof.pending_calendar_count, 1);
    assert_eq!(proof.bitcoin_claim_count, 1);
    let json = String::from_utf8(local::bytes(public).unwrap()).unwrap();
    for private in [
        r.bundle_directory.parent().unwrap().to_str().unwrap(),
        r.manifest_sha256.as_str(),
        "private-parent",
        "unselected-public",
        "PRIVATE operator history",
        "private-calendar.example",
        "claimed_block_height",
        "claimed_merkle_root_hex",
        "receipt_sha256",
        "declared_origin",
        "locator",
    ] {
        assert!(!json.contains(private));
    }
    assert_eq!(public.timestamp_state, "not_verified");
    assert_eq!(public.signature_state, "not_signed");
    assert_eq!(proof.availability, AVAILABILITY);
}
#[test]
fn portable_readback_survives_source_loss_without_reasserting_current_owner_facts() {
    let (_temp, r, root) = fixture(true);
    let created = create(&r, &root, &CancellationToken::new()).unwrap();
    let bytes = std::fs::read(root.join(PUBLIC_PATH)).unwrap();
    std::fs::remove_dir_all(&r.bundle_directory).unwrap();
    std::fs::remove_dir_all(&r.subjects[0].proof.as_ref().unwrap().directory).unwrap();
    let portable =
        PublicHandoff::from_pinned_json(&bytes, &created.receipt.public_handoff_sha256).unwrap();
    assert_eq!(portable, created.public_handoff);
    assert_eq!(portable.evidence_scope, SCOPE);
    assert!(observe(&root, &created.receipt_sha256, &CancellationToken::new()).is_err());
}
#[test]
fn private_missing_and_duplicate_subject_requests_cannot_create_a_packet() {
    for id in ["private-parent", "missing"] {
        let (_temp, mut r, root) = fixture(false);
        r.subjects[0].id = id.into();
        assert!(create(&r, &root, &CancellationToken::new()).is_err());
        assert!(!root.exists());
    }
    let (_temp, mut r, root) = fixture(false);
    r.subjects[1].id = r.subjects[0].id.clone();
    assert!(create(&r, &root, &CancellationToken::new()).is_err());
    assert!(!root.exists());
}
#[test]
fn aggregate_or_unsupported_proofs_never_become_public_file_evidence() {
    let (_temp, mut r, root) = fixture(false);
    r.subjects[0].proof = Some(proof_selection(
        &r.bundle_directory,
        &r.manifest_sha256,
        detached::ProofSubject::Manifest {},
        "aggregate-proof",
        1,
    ));
    assert!(create(&r, &root, &CancellationToken::new()).is_err());
    assert!(!root.exists());
    r.subjects[0].proof = Some(proof_selection(
        &r.bundle_directory,
        &r.manifest_sha256,
        detached::ProofSubject::Subject {
            id: "selected-final".into(),
        },
        "unsupported-proof",
        2,
    ));
    assert!(create(&r, &root, &CancellationToken::new()).is_err());
    assert!(!root.exists());
}
#[test]
fn proof_from_another_archive_is_not_bound_by_equal_file_bytes_alone() {
    let (_temp, mut r, root) = fixture(false);
    let mut other: EvidenceRequest = read_exact(
        &r.bundle_directory.join("request.json"),
        local::DOCUMENT_BYTES,
    )
    .unwrap();
    other.version = "v2".into();
    let directory = r.bundle_directory.with_file_name("other-archive");
    let sealed = super::super::seal(&other, &directory, &CancellationToken::new()).unwrap();
    r.subjects[0].proof = Some(proof_selection(
        &directory,
        &sealed.subject_manifest_sha256,
        detached::ProofSubject::Subject {
            id: "selected-final".into(),
        },
        "other-proof",
        1,
    ));
    assert!(create(&r, &root, &CancellationToken::new()).is_err());
    assert!(!root.exists());
}
#[test]
fn explicit_byte_budgets_and_external_source_pins_are_required() {
    let (_temp, mut r, root) = fixture(true);
    r.maximum_archive_subject_bytes = 1;
    assert!(create(&r, &root, &CancellationToken::new()).is_err());
    assert!(!root.exists());
    r.maximum_archive_subject_bytes = 1_048_576;
    r.maximum_proof_bytes = 0;
    assert!(plan(&r, &CancellationToken::new()).is_err());
    r.maximum_proof_bytes = 1_048_576;
    r.manifest_sha256 = "0".repeat(64);
    assert!(plan(&r, &CancellationToken::new()).is_err());
    let (_temp, mut r, _) = fixture(true);
    r.subjects[0].proof.as_mut().unwrap().receipt_sha256 = "0".repeat(64);
    assert!(plan(&r, &CancellationToken::new()).is_err());
}
#[test]
fn changed_or_extra_packet_members_and_stale_proofs_are_refused_without_cleanup() {
    let (_temp, r, root) = fixture(true);
    let status = create(&r, &root, &CancellationToken::new()).unwrap();
    std::fs::write(root.join("public/extra"), b"preserve").unwrap();
    assert!(observe(&root, &status.receipt_sha256, &CancellationToken::new()).is_err());
    assert!(root.join("public/extra").exists());
    std::fs::remove_file(root.join("public/extra")).unwrap();
    std::fs::write(root.join(PUBLIC_PATH), b"{}").unwrap();
    assert!(observe(&root, &status.receipt_sha256, &CancellationToken::new()).is_err());
    let (_temp, r, root) = fixture(true);
    let status = create(&r, &root, &CancellationToken::new()).unwrap();
    std::fs::write(
        r.subjects[0]
            .proof
            .as_ref()
            .unwrap()
            .directory
            .join("proof.ots"),
        b"changed",
    )
    .unwrap();
    assert!(observe(&root, &status.receipt_sha256, &CancellationToken::new()).is_err());
}
#[test]
fn cancelled_existing_overlapping_and_unknown_request_fields_never_adopt_work() {
    let (_temp, r, root) = fixture(false);
    let token = CancellationToken::new();
    token.cancel();
    assert!(create(&r, &root, &token).is_err());
    assert!(!root.exists());
    assert!(
        create(
            &r,
            &r.bundle_directory.join("overlap"),
            &CancellationToken::new()
        )
        .is_err()
    );
    assert!(!r.bundle_directory.join("overlap").exists());
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("keep"), b"preserve").unwrap();
    assert!(create(&r, &root, &CancellationToken::new()).is_err());
    assert_eq!(std::fs::read(root.join("keep")).unwrap(), b"preserve");
    let mut value = serde_json::to_value(&r).unwrap();
    value["include_private"] = serde_json::json!(true);
    assert!(HandoffRequest::from_json_slice(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn proof_free_projection_and_pinned_reader_cannot_claim_signing_or_trusted_time() {
    let (_temp, mut r, _) = fixture(false);
    r.maximum_proof_bytes = 0;
    let p = plan(&r, &CancellationToken::new()).unwrap();
    let original = local::bytes(&p.public_handoff).unwrap();
    assert!(PublicHandoff::from_pinned_json(&original, &"0".repeat(64)).is_err());
    for field in [
        "signature_state",
        "timestamp_state",
        "ownership",
        "publication_authority",
    ] {
        let mut value = serde_json::to_value(&p.public_handoff).unwrap();
        value[field] = serde_json::json!("verified");
        let bytes = crate::provider::canonical_json_bytes(&value).unwrap();
        assert!(PublicHandoff::from_pinned_json(&bytes, &local::hash(&bytes)).is_err());
    }
    let mut value = serde_json::to_value(&p.public_handoff).unwrap();
    value["private_path"] = serde_json::json!("/secret");
    let bytes = crate::provider::canonical_json_bytes(&value).unwrap();
    assert!(PublicHandoff::from_pinned_json(&bytes, &local::hash(&bytes)).is_err());
    assert!(p.public_handoff.subjects.iter().all(|s| s.proof.is_none()));
}
