use std::process::Command;

fn cli(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(arguments)
        .output()
        .expect("run aniflow")
}

#[test]
fn midi_selection_requires_exact_settings_and_rejects_other_families() {
    let base = [
        "audio",
        "plan",
        "--analysis",
        "midi-candidates",
        "--input",
        "absent.wav",
        "--configuration",
        "tools.json",
    ];
    let missing = cli(&base);
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--midi-configuration"));
    for extra in [
        "--signal-configuration",
        "--musical-configuration",
        "--transcription-configuration",
        "--alignment-configuration",
        "--lyrics",
    ] {
        let mut arguments = base.to_vec();
        arguments.extend(["--midi-configuration", "midi.json", extra, "other.json"]);
        let conflicting = cli(&arguments);
        assert_eq!(conflicting.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&conflicting.stderr).contains("cannot be used with"));
    }
}

#[test]
fn midi_commands_are_explicit_and_export_refusal_is_machine_readable() {
    for arguments in [
        vec!["audio", "midi", "extract", "--help"],
        vec!["audio", "midi", "export", "--help"],
    ] {
        let result = cli(&arguments);
        assert!(result.status.success());
        let help = String::from_utf8_lossy(&result.stdout);
        assert!(help.contains("--output-directory"));
        assert!(help.contains(if arguments[2] == "extract" {
            "--midi-configuration"
        } else {
            "--candidate"
        }));
    }
    let temporary = tempfile::tempdir().unwrap();
    let output = temporary.path().join("must-not-be-created");
    let missing = temporary.path().join("missing.json");
    let result = cli(&[
        "--output",
        "json",
        "audio",
        "midi",
        "export",
        "--candidate",
        missing.to_str().unwrap(),
        "--output-directory",
        output.to_str().unwrap(),
    ]);
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let envelope: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(envelope["command"], "audio_midi_export");
    assert!(!output.exists());
    assert_eq!(
        cli(&["audio", "analyze", "--analysis", "midi-candidates"])
            .status
            .code(),
        Some(2)
    );
}
