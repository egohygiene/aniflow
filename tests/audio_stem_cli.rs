use std::process::Command;

#[test]
fn incomplete_stem_selection_is_rejected_before_source_or_tool_access() {
    for extra in [
        vec!["--stem-id", "vocals"],
        vec!["--stem-stage", "separate_vocals"],
        vec!["--stem-run", "absent-run"],
        vec!["--stem-channels", "0"],
        vec!["--stem-start-frame", "0"],
        vec!["--stem-end-frame", "44100"],
        vec!["--stem-duration-tolerance-milliseconds", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_aniflow"))
            .args([
                "audio",
                "plan",
                "--input",
                "absent-source.wav",
                "--configuration",
                "absent-configuration.json",
            ])
            .args(&extra)
            .output()
            .expect("CLI starts");
        assert_eq!(output.status.code(), Some(2), "{extra:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(message.contains("required"), "{extra:?}: {message}");
        assert!(!message.contains("cannot be read"), "{message}");
    }
}

#[test]
fn stem_scope_never_silently_accepts_missing_boundaries_or_excess_tolerance() {
    for extra in [
        vec!["--stem-start-frame", "0"],
        vec!["--stem-end-frame", "44100"],
        vec!["--stem-duration-tolerance-milliseconds", "21"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_aniflow"))
            .args([
                "audio",
                "plan",
                "--input",
                "absent-source.wav",
                "--configuration",
                "absent-configuration.json",
                "--stem-run",
                "absent-run",
                "--stem-stage",
                "separate_vocals",
                "--stem-id",
                "vocals",
            ])
            .args(&extra)
            .output()
            .expect("CLI starts");
        assert_eq!(output.status.code(), Some(2), "{extra:?}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("cannot be read"));
    }
}
