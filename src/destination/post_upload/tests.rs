//! Synthetic declarations/archives only, with no real post or native acceptance.
use super::*;
fn request() -> ObservationRequest {
    ObservationRequest {
        schema: "aniflow.post-upload-observation-request/v1".into(),
        package_directory: PathBuf::from("/missing-original-package"),
        package_receipt_sha256: "a".repeat(64),
        destination: Destination::Soundcloud,
        declaration: PublicationDeclaration {
            url: "https://soundcloud.com/synthetic/fixture".into(),
            posted_at: None,
            observed_at: DateTime::parse_from_rfc3339("2026-11-09T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            observer: "SYNTHETIC_PRIVATE_OBSERVER".into(),
            acquisition: Acquisition::Unknown,
            quality: QualityDeclaration {
                picture: SpotCheck::NotReviewed,
                audio_stereo: SpotCheck::NotReviewed,
                timing: SpotCheck::NotReviewed,
                metadata: SpotCheck::NotReviewed,
                watermark: SpotCheck::Unavailable,
                notes: "SYNTHETIC_PRIVATE_NOTES".into(),
            },
        },
        observed_file: PathBuf::from("/missing-copy.wav"),
        expected_observed_sha256: local::hash(b"synthetic platform-copy bytes"),
        maximum_copy_bytes: 4096,
        minimum_free_bytes: 0,
    }
}
fn package() -> PostingReceipt {
    PostingReceipt {
        schema: "aniflow.posting-receipt/v1".into(),
        package_id: "1".repeat(64),
        preflight_sha256: "2".repeat(64),
        review_context_sha256: "3".repeat(64),
        review_subject_sha256: "4".repeat(64),
        review_sha256: "5".repeat(64),
        members: [
            ("public/audio.wav", MemberVisibility::Public),
            ("public/release.json", MemberVisibility::Public),
            ("private/request.json", MemberVisibility::Private),
            ("private/review.json", MemberVisibility::Private),
        ]
        .into_iter()
        .map(|(path, visibility)| posting::PackageMember {
            path: path.into(),
            sha256: "6".repeat(64),
            byte_count: 123,
            visibility,
        })
        .collect(),
        publication_authority: "none".into(),
        proof_state: "not_asserted".into(),
    }
}
fn archive() -> (tempfile::TempDir, PathBuf, String, ObservationReceipt) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap().join("retained");
    local::directory(&root).unwrap();
    let package = package();
    let mut request = request();
    request.package_receipt_sha256 = local::hash(&local::bytes(&package).unwrap());
    let observed = b"synthetic platform-copy bytes";
    let receipt = build(
        &request,
        &package,
        (local::hash(observed), observed.len() as u64),
    )
    .unwrap();
    for (name, bytes) in [
        ("request.json", local::bytes(&request).unwrap()),
        ("package-snapshot.json", local::bytes(&package).unwrap()),
        ("observation.json", local::bytes(&receipt).unwrap()),
        ("observed.bin", observed.to_vec()),
    ] {
        local::write(&root.join(name), &bytes).unwrap();
    }
    let pin = local::hash(&local::bytes(&receipt).unwrap());
    (temporary, root, pin, receipt)
}
#[test]
fn locator_is_only_a_bounded_declaration_and_cannot_contain_credentials_or_query_secrets() {
    assert!(locator("https://soundcloud.com/artist/song"));
    for url in [
        "http://soundcloud.com/a",
        "https://user:secret@soundcloud.com/a",
        "https://soundcloud.com/a?token=secret",
        "https://soundcloud.com/a#secret",
        "https://soundcloud.com\\evil/a",
        "https://soundcloud..com/a",
        "https://soundcloud.com/a\n",
    ] {
        assert!(!locator(url));
    }
}
#[test]
fn supplied_chronology_limits_and_concern_reasons_are_validated() {
    let mut r = request();
    assert!(r.validate().is_ok());
    r.declaration.posted_at = Some(r.declaration.observed_at + chrono::Duration::seconds(1));
    assert!(r.validate().is_err());
    r.declaration.posted_at = None;
    r.maximum_copy_bytes = 0;
    assert!(r.validate().is_err());
    r.maximum_copy_bytes = 4096;
    r.declaration.quality.metadata = SpotCheck::Concerns;
    r.declaration.quality.notes.clear();
    assert!(r.validate().is_err());
}
#[test]
fn byte_change_does_not_prove_quality_or_metadata_failure() {
    let r = request();
    let receipt = build(&r, &package(), ("7".repeat(64), 222)).unwrap();
    assert_eq!(receipt.byte_relationship, ByteRelationship::Different);
    assert!(receipt.technical_validation.starts_with("not_performed"));
    assert_eq!(receipt.declaration.quality.timing, SpotCheck::NotReviewed);
    assert_eq!(
        relation(&("6".repeat(64), 123), &("6".repeat(64), 123)).unwrap(),
        ByteRelationship::Identical
    );
    assert!(relation(&("6".repeat(64), 123), &("6".repeat(64), 124)).is_err());
}
#[test]
fn archive_readback_does_not_require_or_authenticate_original_platform_paths() {
    let (_temporary, root, pin, receipt) = archive();
    assert_eq!(
        inspect(&root, &pin, &CancellationToken::default()).unwrap(),
        receipt
    );
    assert!(
        receipt
            .platform_authentication
            .starts_with("not_established")
    );
    assert_eq!(receipt.publication_authority, "none");
}
#[test]
fn tampered_copy_wrong_receipt_pin_and_unknown_members_refuse_without_cleanup() {
    let (_temporary, root, pin, _receipt) = archive();
    let token = CancellationToken::default();
    assert!(inspect(&root, &"0".repeat(64), &token).is_err());
    std::fs::write(root.join("unknown"), b"preserve").unwrap();
    assert!(inspect(&root, &pin, &token).is_err());
    assert!(root.join("unknown").exists());
    std::fs::remove_file(root.join("unknown")).unwrap();
    std::fs::write(root.join("observed.bin"), b"changed").unwrap();
    assert!(inspect(&root, &pin, &token).is_err());
}
#[test]
fn public_summary_omits_url_observer_history_paths_and_review_notes() {
    let (_temporary, _root, _pin, receipt) = archive();
    let summary = public_summary(&receipt).unwrap().to_string();
    for private in [
        "soundcloud.com",
        "SYNTHETIC_PRIVATE",
        "missing-original",
        "missing-copy",
        "posted_at",
        "observed_at",
    ] {
        assert!(!summary.contains(private));
    }
}
#[test]
fn cancelled_or_unbound_request_does_not_create_an_output() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap().join("not-created");
    let r = request();
    let token = CancellationToken::default();
    token.cancel();
    assert!(retain(&r, &root, &token).is_err());
    assert!(!root.exists());
    assert!(retain(&r, &root, &CancellationToken::default()).is_err());
    assert!(!root.exists());
}
