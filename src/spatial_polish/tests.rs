use super::*;

fn profile() -> Profile {
    Profile {
        schema: SCHEMA.into(),
        denoise: Some(Denoise { strength_hundredths: 120, patch_odd: 3, search_odd: 7 }),
        deband: Some(Deband { threshold_thousandths: 20, radius: 16 }),
        sharpen: Some(Sharpen { amount_hundredths: 50, kernel_odd: 5 }),
    }
}

#[test]
fn selected_order_and_decimal_arguments_are_closed() {
    assert_eq!(profile().filters().unwrap(), vec![
        "nlmeans=s=1.20:p=3:pc=3:r=7:rc=7",
        "deband=1thr=0.020:2thr=0.020:3thr=0.020:4thr=0.020:range=-16:direction=-0.7853981633974483:blur=1:coupling=0",
        "unsharp=luma_msize_x=5:luma_msize_y=5:luma_amount=0.50:chroma_msize_x=3:chroma_msize_y=3:chroma_amount=0.00",
    ]);
}

#[test]
fn rejects_empty_unknown_arbitrary_and_out_of_bounds_settings() {
    let valid = serde_json::to_value(profile()).unwrap();
    for (key, value) in [
        ("filter", serde_json::json!("movie=/private/source")),
        ("temporal", serde_json::json!(true)),
        ("auto_review", serde_json::json!(true)),
    ] {
        let mut bad = valid.clone(); bad[key] = value;
        assert!(Profile::from_json_slice(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    for (stage, key, value) in [
        ("denoise", "strength_hundredths", serde_json::json!(0)),
        ("denoise", "strength_hundredths", serde_json::json!(1001)),
        ("denoise", "strength_hundredths", serde_json::json!(1.2)),
        ("denoise", "patch_odd", serde_json::json!(4)),
        ("denoise", "search_odd", serde_json::json!(16)),
        ("denoise", "search_odd", serde_json::json!(8)),
        ("deband", "threshold_thousandths", serde_json::json!(251)),
        ("deband", "radius", serde_json::json!(0)),
        ("deband", "radius", serde_json::json!(33)),
        ("sharpen", "amount_hundredths", serde_json::json!(201)),
        ("sharpen", "kernel_odd", serde_json::json!(9)),
        ("denoise", "filter", serde_json::json!("null;movie=bad")),
    ] {
        let mut bad = valid.clone(); bad[stage][key] = value;
        assert!(Profile::from_json_slice(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    assert!(Profile::from_json_slice(br#"{"schema":"aniflow.spatial-polish/v1"}"#).is_err());
    assert!(Profile::from_json_slice(&vec![b' '; 4097]).is_err());
}

#[test]
fn individual_choices_and_strength_changes_have_distinct_identities() {
    let mut p = profile(); let all = p.identity_sha256().unwrap();
    p.denoise = None; p.deband = None;
    assert_eq!(p.filters().unwrap().len(), 1);
    assert_ne!(all, p.identity_sha256().unwrap());
    let before = p.identity_sha256().unwrap();
    p.sharpen.as_mut().unwrap().amount_hundredths = 1;
    assert!(p.filters().unwrap()[0].contains("luma_amount=0.01:"));
    assert_ne!(before, p.identity_sha256().unwrap());
}
