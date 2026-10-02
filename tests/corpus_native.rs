//! Native codec qualification is explicit and uses generated media only.
use std::fs;
use std::path::Path;
use std::process::Command;

use aniflow::temporal::{self, RationalTime, StreamSelection};
use serde_json::Value;

#[test]
fn generated_y4m_is_inspected_through_the_public_temporal_api() {
    let directory = tempfile::tempdir().unwrap();
    let bundle = directory.path().join("corpus");
    let python = std::env::var_os("ANIFLOW_CORPUS_PYTHON").unwrap_or_else(|| "python3".into());
    let result = Command::new(python).arg("-B")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/temporal-corpus.py"))
        .args(["generate", "--output"]).arg(&bundle).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let catalog: Value = serde_json::from_slice(&fs::read(bundle.join("catalog.json")).unwrap()).unwrap();
    let mut inspected = 0;
    for case in catalog["cases"].as_array().unwrap().iter().filter(|case|
        case["family"] == "container" && case["oracle"]["outcome"] == "decode"
    ) {
        let input = bundle.join(case["id"].as_str().unwrap()).join("input.y4m");
        let before = fs::read(&input).unwrap();
        let report = temporal::inspect(&input, &StreamSelection::default()).unwrap();
        report.require_processing().unwrap();
        let timeline = report.video_timeline().unwrap();
        let rate = case["parameters"]["rate"].as_str().unwrap().replace(':', "/");
        assert_eq!(timeline.frame_period, Some(RationalTime::parse(&rate).unwrap().reciprocal().unwrap()));
        assert_eq!(timeline.frames.len() as u64, case["parameters"]["frames"].as_u64().unwrap());
        temporal::validate_reconstruction(&report, &report, false).unwrap();
        assert_eq!(fs::read(input).unwrap(), before);
        inspected += 1;
    }
    assert_eq!(inspected, 4);
}
