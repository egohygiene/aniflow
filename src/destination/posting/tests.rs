//! Pure package bindings and local refusal/inventory cases, not native proof.
use super::*;
fn request() -> PostingRequest {
    let r = super::super::tests::request();
    let media = super::super::tests::facts(&r);
    PostingRequest {
        schema: "aniflow.posting-request/v1".into(),
        preflight: super::super::evaluate(&r, media).unwrap(),
        public_metadata: PublicReleaseMetadata {
            title: "Synthetic song".into(),
            creator: "Synthetic creator".into(),
            release_id: "fixture-release".into(),
            version: "v1".into(),
            credits: "Fixture only".into(),
            description: None,
            artwork_references: vec![],
            provenance_reference: None,
        },
        metadata_scope_policy: MetadataScopePolicy::AcknowledgeInheritedOrUninspectedScope,
        maximum_package_bytes: 10_000_000,
        minimum_free_bytes: 0,
    }
}
fn declaration(plan: &PostingPlan) -> ManualReview {
    ManualReview {
        schema: "aniflow.manual-review/v1".into(),
        context_sha256: plan.review_context_sha256.clone(),
        subject_sha256: plan.review_subject_sha256.clone(),
        decision: ReviewDecision::Approve,
        reviewer: "synthetic reviewer".into(),
        notes: "Not an actual content review".into(),
    }
}
#[test]
fn public_projection_never_copies_private_locations_or_requirement_checker() {
    let r = request();
    let p = build_plan(&r).unwrap();
    let public = String::from_utf8(local::bytes(&public_manifest(&r, &p)).unwrap()).unwrap();
    assert!(!public.contains("/synthetic"));
    assert!(!public.contains("checked_by"));
    assert!(!public.contains("run_manifest"));
    assert!(!public.contains("private_report"));
    assert!(public.contains("Synthetic song"));
    assert!(public.contains("not_asserted"));
}
#[test]
fn review_context_binds_public_metadata_profile_and_scope() {
    let r = request();
    let p = build_plan(&r).unwrap();
    let review_record = declaration(&p);
    review(&p, &review_record).unwrap();
    let mut changed = r.clone();
    changed.public_metadata.title = "Different exact title".into();
    let later = build_plan(&changed).unwrap();
    assert_ne!(later.review_context_sha256, p.review_context_sha256);
    assert!(review(&later, &review_record).is_err());
    let mut rejected = review_record;
    rejected.decision = ReviewDecision::Reject;
    assert!(review(&p, &rejected).is_err());
}
#[test]
fn inherited_or_unknown_metadata_needs_explicit_scope_and_failed_scope_never_bypasses() {
    let mut r = request();
    r.metadata_scope_policy = MetadataScopePolicy::RequireCleanWithinDeclaredScope;
    assert!(build_plan(&r).is_err());
    r.metadata_scope_policy = MetadataScopePolicy::AcknowledgeInheritedOrUninspectedScope;
    assert!(build_plan(&r).is_ok());
    let MediaFacts::Video { completion, .. } = &mut r.preflight.payload.media.facts else {
        panic!("fixture");
    };
    completion.artifacts[0].metadata = Some(crate::delivery_metadata::MetadataPublicSummary {
        schema: "aniflow.delivery-metadata-summary/v1".into(),
        mode: crate::delivery_metadata::MetadataMode::Minimal,
        status: crate::delivery_metadata::MetadataInspectionStatus::ResidualFields,
        before_fields: 1,
        after_fields: 1,
        residual_fields: 1,
        regenerated_fields: 0,
        inspected_scope: "synthetic".into(),
        unsupported_scope: "not measured".into(),
        source_credentials: "unchanged".into(),
        signal_marks: "unchanged".into(),
    });
    assert!(build_plan(&r).is_err());
}
#[test]
fn an_invented_preflight_cannot_create_a_posting_package() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let mut r = request();
    r.preflight.payload.request.input = MediaInput::AcceptedVideo {
        run_directory: root.clone(),
        artifact_id: "delivery".into(),
    };
    r.preflight.preflight_sha256 = canonical_sha256(&r.preflight.payload).unwrap();
    let p = build_plan(&r).unwrap();
    let output = root.join("must not create");
    assert!(create(&r, &declaration(&p), &output, &CancellationToken::default()).is_err());
    assert!(!output.exists());
}
#[test]
fn partial_and_unknown_members_never_gain_acceptance_or_automatic_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let copy = root.join("kept media.mp4");
    std::fs::write(&copy, b"retained partial bytes").unwrap();
    assert!(verify(&root, &CancellationToken::default()).is_err());
    assert_eq!(std::fs::read(copy).unwrap(), b"retained partial bytes");
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
}
#[test]
fn member_mutation_or_extra_public_sidecar_refuses_fixed_membership() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    local::directory(&root.join("public")).unwrap();
    local::directory(&root.join("private")).unwrap();
    let mut r = request();
    let payload = b"synthetic bytes, not media";
    r.preflight.payload.media.file.sha256 = local::hash(payload);
    r.preflight.payload.media.file.byte_count = payload.len() as u64;
    let p = build_plan(&r).unwrap();
    let d = declaration(&p);
    let rec = receipt(&r, &p, &d).unwrap();
    local::write(
        &root.join("private/request.json"),
        &local::bytes(&r).unwrap(),
    )
    .unwrap();
    local::write(
        &root.join("private/review.json"),
        &local::bytes(&d).unwrap(),
    )
    .unwrap();
    local::write(
        &root.join("public/release.json"),
        &local::bytes(&public_manifest(&r, &p)).unwrap(),
    )
    .unwrap();
    local::write(&root.join("public/delivery.mp4"), payload).unwrap();
    check_members(&root, &r, &p, &rec).unwrap();
    let extra = root.join("public/undeclared.srt");
    std::fs::write(&extra, b"unselected").unwrap();
    assert!(check_members(&root, &r, &p, &rec).is_err());
    std::fs::remove_file(extra).unwrap();
    std::fs::write(root.join("public/delivery.mp4"), b"changed").unwrap();
    assert!(check_members(&root, &r, &p, &rec).is_err());
}
#[test]
fn output_overlap_and_symlink_members_refuse_without_touching_sources() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source = root.join("source.mp4");
    std::fs::write(&source, b"original").unwrap();
    let input = MediaInput::AcceptedVideo {
        run_directory: root.clone(),
        artifact_id: "delivery".into(),
    };
    assert!(local::disjoint(&root.join("nested"), &input, &source).is_err());
    #[cfg(unix)]
    {
        let path = root.join("alias");
        std::os::unix::fs::symlink(&source, &path).unwrap();
        assert!(local::entries(&root, &["source.mp4", "alias"], &[], false).is_err());
    }
    assert_eq!(std::fs::read(source).unwrap(), b"original");
}
