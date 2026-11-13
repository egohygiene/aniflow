//! Synthetic local bytes only; these cases establish no native/media acceptance.
use super::*;
fn request(root: &Path) -> EvidenceRequest {
    let file = root.join("original 音.txt");
    let bytes = b"synthetic retained original; not media";
    std::fs::write(&file, bytes).unwrap();
    EvidenceRequest {
        schema: "aniflow.release-evidence-request/v1".into(),
        release_id: "fixture".into(),
        version: "v1".into(),
        subjects: vec![SubjectSelection {
            id: "original".into(),
            role: SubjectRole::Original,
            visibility: MemberVisibility::Private,
            parents: vec![],
            input: SubjectInput::DeclaredFile {
                file,
                expected_sha256: local::hash(bytes),
                maximum_bytes: 4096,
                declared_origin: "Synthetic operator history; no executed provider".into(),
            },
        }],
        maximum_bundle_bytes: 16_777_216,
        minimum_free_bytes: 0,
    }
}
#[test]
fn exact_copies_verify_without_the_original_and_manifest_is_not_self_referential() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let request = request(&root);
    let output = root.join("retained");
    let token = CancellationToken::default();
    let planned = plan(&request, &token).unwrap();
    let receipt = seal(&request, &output, &token).unwrap();
    assert_eq!(receipt.subject_manifest_sha256, planned.manifest_sha256);
    let manifest = std::fs::read(output.join("subjects.json")).unwrap();
    assert_eq!(local::hash(&manifest), receipt.subject_manifest_sha256);
    assert!(
        !String::from_utf8(manifest)
            .unwrap()
            .contains("subject_manifest_sha256")
    );
    let SubjectInput::DeclaredFile { file, .. } = &request.subjects[0].input else {
        panic!("fixture");
    };
    assert_eq!(
        std::fs::read(file).unwrap(),
        std::fs::read(output.join("subjects/0000-original.bin")).unwrap()
    );
    std::fs::remove_file(file).unwrap();
    let checked = verify(&output, &receipt.subject_manifest_sha256, &token).unwrap();
    assert_eq!(
        checked.producer_acceptance,
        "not_reobserved_by_archival_byte_check"
    );
    assert_eq!(checked.timestamp_state, "not_verified");
}
#[test]
fn changed_retained_bytes_and_changed_manifest_with_recomputed_receipt_refuse_old_pin() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let request = request(&root);
    let token = CancellationToken::default();
    let output = root.join("retained");
    let sealed = seal(&request, &output, &token).unwrap();
    let copy = output.join("subjects/0000-original.bin");
    let saved = std::fs::read(&copy).unwrap();
    std::fs::write(&copy, b"tampered").unwrap();
    assert!(verify(&output, &sealed.subject_manifest_sha256, &token).is_err());
    std::fs::write(&copy, saved).unwrap();
    let mut manifest: SubjectManifest = local::read(&output.join("subjects.json")).unwrap();
    manifest.subjects[0].origin = SubjectOrigin::DeclaredFile {
        locator: root.join("invented"),
        declared_origin: "changed history".into(),
        technical_acceptance: "not_established".into(),
    };
    std::fs::write(
        output.join("subjects.json"),
        local::bytes(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        output.join("seal.json"),
        local::bytes(&receipt(&manifest).unwrap()).unwrap(),
    )
    .unwrap();
    assert!(verify(&output, &sealed.subject_manifest_sha256, &token).is_err());
}
#[test]
fn incomplete_unknown_and_symlink_members_are_never_adopted_or_cleaned() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let request = request(&root);
    let token = CancellationToken::default();
    let output = root.join("retained");
    let receipt = seal(&request, &output, &token).unwrap();
    let extra = output.join("subjects/extra");
    std::fs::write(&extra, b"keep unknown evidence").unwrap();
    assert!(verify(&output, &receipt.subject_manifest_sha256, &token).is_err());
    assert!(extra.exists());
    std::fs::remove_file(&extra).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(output.join("subjects/0000-original.bin"), &extra).unwrap();
        assert!(verify(&output, &receipt.subject_manifest_sha256, &token).is_err());
        std::fs::remove_file(&extra).unwrap();
    }
    std::fs::remove_file(output.join("seal.json")).unwrap();
    assert!(verify(&output, &receipt.subject_manifest_sha256, &token).is_err());
    assert!(seal(&request, &output, &token).is_err());
    assert!(output.join("subjects/0000-original.bin").exists());
}
#[test]
fn changed_input_cancelled_request_and_storage_limit_refuse_before_output() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut request = request(&root);
    let output = root.join("not created");
    let token = CancellationToken::default();
    let SubjectInput::DeclaredFile {
        expected_sha256, ..
    } = &mut request.subjects[0].input
    else {
        panic!("fixture");
    };
    *expected_sha256 = "0".repeat(64);
    assert!(seal(&request, &output, &token).is_err());
    assert!(!output.exists());
    let mut request = self::request(&root);
    request.maximum_bundle_bytes = 1;
    assert!(seal(&request, &output, &token).is_err());
    assert!(!output.exists());
    let token = CancellationToken::default();
    token.cancel();
    assert!(seal(&self::request(&root), &output, &token).is_err());
    assert!(!output.exists());
}
#[test]
fn lineage_is_ordered_and_public_projection_cannot_reveal_private_parents_or_history() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut request = request(&root);
    let mut public = request.subjects[0].clone();
    public.id = "delivery".into();
    public.role = SubjectRole::FinalDelivery;
    public.visibility = MemberVisibility::Public;
    public.parents = vec!["original".into()];
    request.subjects.push(public);
    let planned = plan(&request, &CancellationToken::default()).unwrap();
    let summary = public_summary(&planned.manifest).to_string();
    assert!(summary.contains("delivery"));
    for private in [
        "original",
        root.to_str().unwrap(),
        "operator history",
        "locator",
        "request_sha256",
    ] {
        assert!(!summary.contains(private));
    }
    request.subjects[0].parents = vec!["delivery".into()];
    assert!(request.validate().is_err());
}
#[test]
fn serialized_native_or_posting_claims_cannot_bypass_current_producer_selection() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let mut request = request(&root);
    let token = CancellationToken::default();
    request.subjects[0].input = SubjectInput::AcceptedMedia {
        input: destination::MediaInput::AcceptedVideo {
            run_directory: root.clone(),
            artifact_id: "delivery".into(),
        },
    };
    assert!(plan(&request, &token).is_err());
    request.subjects[0].input = SubjectInput::PostingMember {
        directory: root,
        member: PostingMember::Media,
    };
    assert!(plan(&request, &token).is_err());
}
#[test]
fn observations_append_without_changing_subjects_and_corruption_refuses_the_next_write() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let request = request(&root);
    let output = root.join("retained");
    let token = CancellationToken::default();
    let receipt = seal(&request, &output, &token).unwrap();
    let immutable: Vec<_> = [
        "request.json",
        "subjects.json",
        "seal.json",
        "subjects/0000-original.bin",
    ]
    .iter()
    .map(|p| std::fs::read(output.join(p)).unwrap())
    .collect();
    let first = record_verification(&output, &receipt.subject_manifest_sha256, &token).unwrap();
    let second = record_verification(&output, &receipt.subject_manifest_sha256, &token).unwrap();
    assert_eq!(first.sequence, 1);
    assert_eq!(second.sequence, 2);
    assert_eq!(
        second.previous_observation_sha256,
        Some(local::hash(&local::bytes(&first).unwrap()))
    );
    assert_eq!(
        immutable,
        [
            "request.json",
            "subjects.json",
            "seal.json",
            "subjects/0000-original.bin"
        ]
        .iter()
        .map(|p| std::fs::read(output.join(p)).unwrap())
        .collect::<Vec<_>>()
    );
    std::fs::write(output.join("observations/00000001.json"), b"partial write").unwrap();
    assert!(record_verification(&output, &receipt.subject_manifest_sha256, &token).is_err());
    assert!(!output.join("observations/00000003.json").exists());
}
