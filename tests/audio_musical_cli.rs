use std::process::Command;

#[test]
fn musical_selection_requires_its_configuration_before_opening_files() {
    for command in ["plan", "analyze", "resume"] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_aniflow"));
        process.args(["audio", command]);
        if command == "resume" {
            process.arg("absent-run");
        }
        let output = process
            .args([
                "--analysis",
                "musical",
                "--input",
                "absent.wav",
                "--configuration",
                "absent-inspection.json",
            ])
            .output()
            .expect("CLI starts");
        assert_eq!(output.status.code(), Some(2));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("--musical-configuration"), "{error}");
        assert!(error.contains("required"), "{error}");
        assert!(!error.contains("cannot be read"), "{error}");
    }
}

#[test]
fn musical_and_signal_settings_are_exclusive() {
    let output = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args([
            "audio",
            "analyze",
            "--analysis",
            "musical",
            "--input",
            "absent.wav",
            "--configuration",
            "absent-inspection.json",
            "--musical-configuration",
            "absent-musical.json",
            "--signal-configuration",
            "absent-signal.json",
        ])
        .output()
        .expect("CLI starts");
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("cannot be used with"), "{error}");
    assert!(!error.contains("cannot be read"), "{error}");
}
