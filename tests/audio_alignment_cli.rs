use std::process::Command;

fn cli(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(arguments)
        .output()
        .expect("run aniflow")
}

#[test]
fn alignment_requires_explicit_reviewed_lyrics_and_matching_configuration() {
    let missing_lyrics = cli(&[
        "audio",
        "plan",
        "--analysis",
        "lyrics-alignment",
        "--input",
        "absent.wav",
        "--configuration",
        "tools.json",
        "--alignment-configuration",
        "alignment.json",
    ]);
    assert_eq!(missing_lyrics.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing_lyrics.stderr).contains("--lyrics"));
    let conflicting = cli(&[
        "audio",
        "plan",
        "--analysis",
        "lyrics-alignment",
        "--input",
        "absent.wav",
        "--configuration",
        "tools.json",
        "--alignment-configuration",
        "alignment.json",
        "--lyrics",
        "reviewed.json",
        "--transcription-configuration",
        "transcription.json",
    ]);
    assert_eq!(conflicting.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&conflicting.stderr).contains("cannot be used with"));
}

#[test]
fn lyrics_subcommands_are_available_and_analyze_remains_bounded() {
    for arguments in [
        vec!["audio", "lyrics", "align", "--help"],
        vec!["audio", "lyrics", "export", "--help"],
        vec!["audio", "resume", "--help"],
    ] {
        let result = cli(&arguments);
        assert!(result.status.success());
        let help = String::from_utf8_lossy(&result.stdout);
        if arguments[2] == "export" {
            assert!(help.contains("--alignment"));
        } else {
            assert!(help.contains("--lyrics"));
            assert!(help.contains("--alignment-configuration"));
        }
    }
    let result = cli(&["audio", "analyze", "--analysis", "lyrics-alignment"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("invalid value"));
}
