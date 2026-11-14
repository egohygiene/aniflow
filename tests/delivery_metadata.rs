//! Synthetic policy cases authored; execution remains deferred under #64.
use aniflow::delivery_metadata::*;
use serde_json::{Value, json};
use std::path::Path;
fn policy(mode: MetadataMode) -> DeliveryMetadataPolicy {
    DeliveryMetadataPolicy {
        schema: DELIVERY_METADATA_POLICY_V1.to_owned(),
        mode,
        approved: Default::default(),
    }
}
fn document() -> Value {
    json!({"format":{"tags":{"title":"inherited private title","location":"private-location"}},
    "streams":[{"index":4,"tags":{"language":"jpn","private_device":"private-camera"}},{"index":7,"tags":{"language":"eng"}}],
    "chapters":[{"tags":{"title":"private chapter"}}],"programs":[]})
}
fn candidate() -> Value {
    json!({"format":{"tags":{"encoder":"Lavf"}},
    "streams":[{"index":0,"tags":{"language":"jpn"}},{"index":1,"tags":{"language":"eng"}}],"chapters":[],"programs":[]})
}
fn receipt(policy: &DeliveryMetadataPolicy, after: &Value) -> MetadataPrivateReceipt {
    verify(
        policy,
        inventory(&document()).unwrap(),
        inventory(after).unwrap(),
        &"a".repeat(64),
        &"b".repeat(64),
        4,
        Some(7),
    )
    .unwrap()
}
#[test]
fn selected_clear_maps_each_scope_and_preserves_language_without_shell_interpolation() {
    let mut settings = policy(MetadataMode::ClearInheritedThenApplyAllowlist);
    settings.approved.insert(
        ReleaseField::Title,
        "Unicode 雪; $(do not execute)".to_owned(),
    );
    let args = remux_arguments(
        Path::new("/source original.mp4"),
        Path::new("/new candidate.mp4"),
        &settings,
        &inventory(&document()).unwrap(),
        4,
        Some(7),
    )
    .unwrap();
    let args: Vec<_> = args.iter().map(|v| v.to_str().unwrap()).collect();
    for pair in [
        ["-c", "copy"],
        ["-map_metadata", "-1"],
        ["-map_metadata:s:v:0", "-1"],
        ["-map_metadata:s:a:0", "-1"],
        ["-map_chapters", "-1"],
        ["-metadata:s:v:0", "language=jpn"],
        ["-metadata", "title=Unicode 雪; $(do not execute)"],
    ] {
        assert!(args.windows(2).any(|v| v == pair));
    }
    for forbidden in ["-shortest", "-t", "-ss", "-af", "-vf", "-y"] {
        assert!(!args.contains(&forbidden));
    }
}
#[test]
fn inspected_fields_do_not_imply_universal_sanitation_and_public_summary_omits_private_values() {
    let value = receipt(&policy(MetadataMode::Minimal), &candidate());
    assert_eq!(
        value.public_summary.status,
        MetadataInspectionStatus::Unsupported
    );
    let public = serde_json::to_string(&value.public_summary).unwrap();
    for private in [
        "private-location",
        "private-camera",
        "inherited private title",
        "private chapter",
    ] {
        assert!(!public.contains(private));
        assert!(serde_json::to_string(&value).unwrap().contains(private));
    }
}
#[test]
fn residual_scopes_missing_credits_and_changed_language_cannot_pass_selected_policy() {
    let mut output = candidate();
    output["format"]["tags"]["location"] = json!("residual");
    assert_eq!(
        receipt(&policy(MetadataMode::Minimal), &output)
            .public_summary
            .status,
        MetadataInspectionStatus::ResidualFields
    );
    let mut settings = policy(MetadataMode::ClearInheritedThenApplyAllowlist);
    settings.approved.insert(
        ReleaseField::Creator,
        "Operator selected creator".to_owned(),
    );
    assert_eq!(
        receipt(&settings, &candidate()).public_summary.status,
        MetadataInspectionStatus::Failed
    );
    let mut output = candidate();
    output["streams"][0]["tags"]["language"] = json!("eng");
    assert_eq!(
        receipt(&policy(MetadataMode::Minimal), &output)
            .public_summary
            .status,
        MetadataInspectionStatus::Failed
    );
    let mut output = candidate();
    output["chapters"] = document()["chapters"].clone();
    assert_eq!(
        receipt(&policy(MetadataMode::Minimal), &output)
            .public_summary
            .status,
        MetadataInspectionStatus::Failed
    );
}
#[test]
fn preserve_uses_explicit_original_input_scopes_and_never_invents_creator_fields() {
    let mut args = Vec::new();
    append_arguments(
        &mut args,
        &policy(MetadataMode::Preserve),
        &inventory(&document()).unwrap(),
        1,
        4,
        Some(7),
    )
    .unwrap();
    let args: Vec<_> = args.iter().map(|v| v.to_str().unwrap()).collect();
    for pair in [
        ["-map_metadata", "1"],
        ["-map_metadata:s:v:0", "1:s:4"],
        ["-map_metadata:s:a:0", "1:s:7"],
        ["-map_chapters", "1"],
    ] {
        assert!(args.windows(2).any(|v| v == pair));
    }
    assert!(!args.contains(&"-metadata"));
}
#[test]
fn malformed_or_uninspected_scopes_and_unapproved_fields_are_refused() {
    let mut value = document();
    value.as_object_mut().unwrap().remove("programs");
    assert!(inventory(&value).is_err());
    let mut value = document();
    value["format"]["tags"]["large"] = json!("x".repeat(8193));
    assert!(inventory(&value).is_err());
    let mut settings = policy(MetadataMode::Minimal);
    settings
        .approved
        .insert(ReleaseField::Title, "no implicit override".to_owned());
    assert!(settings.validate().is_err());
    settings.mode = MetadataMode::ClearInheritedThenApplyAllowlist;
    for value in [
        "http://example.invalid",
        "https://user@example.invalid",
        "https://",
        "https://example.invalid/\nprivate",
    ] {
        settings
            .approved
            .insert(ReleaseField::PublicUrl, value.to_owned());
        assert!(settings.validate().is_err());
    }
    assert!(
        remux_arguments(
            Path::new("/same.mp4"),
            Path::new("/same.mp4"),
            &policy(MetadataMode::Minimal),
            &inventory(&document()).unwrap(),
            4,
            Some(7)
        )
        .is_err()
    );
    assert!(serde_json::from_value::<DeliveryMetadataPolicy>(json!({"schema":DELIVERY_METADATA_POLICY_V1,"mode":"minimal","approved":{"device":"private"}})).is_err());
}
