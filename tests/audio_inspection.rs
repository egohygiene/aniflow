#![cfg(unix)]

//! Synthetic process fixtures exercise the actual native ABI and v3 checkpoints.
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aniflow::audio_analysis::AudioAnalysis;
use aniflow::audio_inspection::{
    self, AudioInspectionConfiguration, AudioInspectionRequest, AudioTechnicalInspection,
    AudioToolPin,
};
use aniflow::{
    CancellationToken, PipelineV3RunOutcome, PipelineV3RunProgress, PipelineV3RunState, status_v3,
};
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;

const TOOL: &str = r#"#!/usr/bin/python3
import hashlib, json, pathlib, struct, sys, time
root = pathlib.Path(__file__).parent
tool = pathlib.Path(__file__).name
mode = (root / 'mode').read_text()
args = sys.argv[1:]
if args == ['-version']:
    if mode == 'version_flood':
        print('x' * 100000)
    else:
        print(tool + ' version ' + ('9.9.9' if mode == 'wrong_version' else '6.1.1') + ' synthetic')
    raise SystemExit(0)
with (root / 'launches').open('a') as log:
    log.write(tool + '\n')
source = pathlib.Path(args[args.index('-i') + 1])
assert source.name.startswith('.audio-snapshot-'), source
if mode == 'nonzero':
    raise SystemExit(23)
if mode == 'sleep':
    (root / 'started').write_text('yes')
    time.sleep(120)
if mode == 'flood':
    print('x' * 100000)
    raise SystemExit(0)
data = source.read_bytes()
offset = 12
while offset < len(data):
    name, size = data[offset:offset + 4], struct.unpack_from('<I', data, offset + 4)[0]
    chunk = data[offset + 8:offset + 8 + size]
    if name == b'fmt ':
        tag, channels, rate, byte_rate, alignment, bits = struct.unpack('<HHIIHH', chunk[:16])
    elif name == b'data':
        pcm = chunk
    offset += 8 + size + size % 2
frames = len(pcm) // alignment
codec, sample_fmt = {(1, 16): ('pcm_s16le', 's16'), (1, 24): ('pcm_s24le', 's32'), (3, 32): ('pcm_f32le', 'flt')}[(tag, bits)]
if tool == 'ffprobe':
    print(json.dumps({'streams': [{'index': 0, 'codec_type': 'audio', 'codec_name': codec,
        'sample_fmt': sample_fmt, 'sample_rate': str(rate), 'channels': channels, 'bits_per_sample': bits,
        'bits_per_raw_sample': str(bits),
        'time_base': '1/' + str(rate), 'duration_ts': frames + (1 if mode == 'wrong_frames' else 0),
        'bit_rate': str(rate * channels * bits)}], 'format': {'format_name': 'wav', 'nb_streams': 1}}))
else:
    assert args[args.index('-codec:a') + 1] == codec, args
    assert args[-5:] == ['-f', 'hash', '-hash', 'sha256', 'pipe:1'], args
    print('SHA256=' + ('0' * 64 if mode == 'wrong_hash' else hashlib.sha256(pcm).hexdigest()))
"#;

struct Fixture {
    root: TempDir,
    input: PathBuf,
    configuration: AudioInspectionConfiguration,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn wave_bytes(rate: u32, channels: u16, frames: u32, tone: bool) -> Vec<u8> {
    let size = frames * u32::from(channels) * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());
    for frame in 0..frames {
        for channel in 0..channels {
            let sample: i16 = if tone {
                if (frame / 8 + u32::from(channel)) % 2 == 0 {
                    1200
                } else {
                    -1200
                }
            } else {
                0
            };
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
    }
    bytes
}

fn native_wave_bytes(rate: u32, channels: u16, frames: u32, float: bool) -> (Vec<u8>, Vec<u8>) {
    let bits: u16 = if float { 32 } else { 24 };
    let alignment = channels * (bits / 8);
    let mut pcm = Vec::new();
    for frame in 0..frames {
        for channel in 0..channels {
            let index = (frame + u32::from(channel)) as usize % 6;
            if float {
                // Preserve negative zero, a subnormal and over-unity samples.
                let sample = [-0.0_f32, f32::from_bits(1), 1.25, -1.5, 0.25, -0.5][index];
                pcm.extend_from_slice(&sample.to_le_bytes());
            } else {
                let sample = [-8_388_608_i32, 8_388_607, 1, -1, 257, -257][index];
                pcm.extend_from_slice(&sample.to_le_bytes()[..3]);
            }
        }
    }
    let size = u32::try_from(pcm.len()).unwrap();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + size + size % 2).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&(if float { 3_u16 } else { 1_u16 }).to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(alignment)).to_le_bytes());
    bytes.extend_from_slice(&alignment.to_le_bytes());
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&pcm);
    if size % 2 == 1 { bytes.push(0); }
    (bytes, pcm)
}

impl Fixture {
    fn new(rate: u32, channels: u16, tone: bool) -> Self {
        let root = tempfile::Builder::new()
            .prefix("audio inspection ü ")
            .tempdir()
            .unwrap();
        fs::write(root.path().join("mode"), "ok").unwrap();
        let pin = |name: &str| {
            let executable = root.path().join(name);
            fs::write(&executable, TOOL).unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
            AudioToolPin {
                executable,
                version: "6.1.1".to_owned(),
                sha256: digest(TOOL.as_bytes()),
            }
        };
        let configuration = AudioInspectionConfiguration {
            schema: "aniflow.audio-inspection.configuration/v1".to_owned(),
            ffmpeg: pin("ffmpeg"),
            ffprobe: pin("ffprobe"),
            tool_timeout_milliseconds: 2000,
            maximum_tool_output_bytes: 4096,
        };
        let input = root.path().join("synthetic tone ü.wav");
        fs::write(&input, wave_bytes(rate, channels, 257, tone)).unwrap();
        Self {
            root,
            input,
            configuration,
        }
    }
    fn request(&self) -> AudioInspectionRequest {
        AudioInspectionRequest::new(
            &self.input,
            self.configuration.clone(),
            env!("CARGO_BIN_EXE_aniflow"),
        )
    }
    fn mode(&self, mode: &str) {
        fs::write(self.root.path().join("mode"), mode).unwrap();
    }
    fn run(&self) -> Result<PipelineV3RunOutcome, audio_inspection::AudioInspectionFailure> {
        audio_inspection::run(
            self.request(),
            Some(self.root.path().join("runs")),
            &CancellationToken::default(),
            |_| {},
        )
    }
    fn assert_failed(&self) {
        let mut run = None;
        let result = audio_inspection::run(
            self.request(),
            Some(self.root.path().join("runs")),
            &CancellationToken::default(),
            |progress| {
                if let PipelineV3RunProgress::Started { run_directory, .. } = progress {
                    run = Some(run_directory.clone());
                }
            },
        );
        assert!(result.is_err(), "unexpected success");
        let run = run.unwrap_or_else(|| {
            panic!(
                "durable run should have started; fixture mode {:?}; result {result:?}",
                fs::read_to_string(self.root.path().join("mode")).unwrap()
            )
        });
        let manifest = status_v3(run).unwrap();
        assert_ne!(manifest.payload.state, PipelineV3RunState::Complete);
    }
}

fn artifact<'a>(outcome: &'a PipelineV3RunOutcome, id: &str) -> &'a Path {
    &outcome
        .outputs
        .iter()
        .find(|output| output.id == id)
        .unwrap()
        .path
}

#[test]
fn synthetic_profiles_produce_exact_evidence_and_preserve_source() {
    for (rate, channels, tone) in [
        (8000, 1, false),
        (44100, 2, true),
        (48000, 1, true),
        (192000, 2, false),
    ] {
        let fixture = Fixture::new(rate, channels, tone);
        let source = fs::read(&fixture.input).unwrap();
        let outcome = fixture.run().unwrap();
        let technical_bytes = fs::read(artifact(&outcome, "technical")).unwrap();
        let technical = AudioTechnicalInspection::from_json_slice(&technical_bytes).unwrap();
        assert_eq!(
            (
                technical.sample_rate_hz,
                technical.channels,
                technical.frame_count
            ),
            (rate, channels, 257)
        );
        assert_eq!(technical.source.sha256, digest(&source));
        assert_eq!(technical.pcm_sha256, digest(&source[44..]));
        assert_eq!(technical.pcm_sha256, technical.decoded_pcm_sha256);
        if rate == 8000 {
            let document: serde_json::Value = serde_json::from_slice(&technical_bytes).unwrap();
            for (pointer, value) in [
                ("/decode_complete", serde_json::json!(false)),
                ("/source_unchanged", serde_json::json!(false)),
                ("/duration/numerator", serde_json::json!(999)),
                ("/pcm_bitrate_bits_per_second", serde_json::json!(1)),
                ("/source/byte_size", serde_json::json!(44)),
                ("/decoded_pcm_sha256", serde_json::json!("0".repeat(64))),
                ("/tools/0/version", serde_json::json!("nightly")),
                ("/commands/0/arguments/0", serde_json::json!("-changed")),
            ] {
                let mut invalid = document.clone();
                *invalid.pointer_mut(pointer).unwrap() = value;
                assert!(
                    AudioTechnicalInspection::from_json_slice(
                        &serde_json::to_vec(&invalid).unwrap()
                    )
                    .is_err(),
                    "accepted {pointer}"
                );
            }
        }
        assert_eq!(
            technical.duration.numerator * i64::from(rate),
            257 * i64::from(technical.duration.denominator)
        );
        AudioAnalysis::from_json_slice(&fs::read(artifact(&outcome, "analysis")).unwrap()).unwrap();
        for output in &outcome.outputs {
            assert_eq!(digest(&fs::read(&output.path).unwrap()), output.sha256);
        }
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
        assert_eq!(
            status_v3(&outcome.run_directory).unwrap().payload.state,
            PipelineV3RunState::Complete
        );
    }
}

#[test]
fn plan_is_read_only_and_resume_reuses_exact_checkpoint() {
    let fixture = Fixture::new(48000, 2, true);
    audio_inspection::plan(&fixture.request(), &CancellationToken::default()).unwrap();
    assert!(!fixture.root.path().join("runs").exists());
    assert!(!fixture.root.path().join("launches").exists());
    let outcome = fixture.run().unwrap();
    let launches = fs::read(fixture.root.path().join("launches")).unwrap();
    let resumed = audio_inspection::resume(
        fixture.request(),
        &outcome.run_directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap();
    assert!(resumed.executed_stages.is_empty());
    assert_eq!(resumed.reused_stages, ["inspect_audio"]);
    assert_eq!(
        fs::read(fixture.root.path().join("launches")).unwrap(),
        launches
    );
}

#[test]
fn native_pcm24_and_float32_inspection_preserves_payload_precision_and_exact_timing() {
    for (rate, channels, float, codec, sample_format, bits) in [
        (8000, 1, false, "pcm_s24le", "s32", 24_u64),
        (48000, 2, false, "pcm_s24le", "s32", 24),
        (44100, 1, true, "pcm_f32le", "flt", 32),
        (192000, 2, true, "pcm_f32le", "flt", 32),
    ] {
        let fixture = Fixture::new(rate, channels, false);
        let (source, pcm) = native_wave_bytes(rate, channels, 257, float);
        fs::write(&fixture.input, &source).unwrap();
        let outcome = fixture.run().unwrap();
        let technical = AudioTechnicalInspection::from_json_slice(
            &fs::read(artifact(&outcome, "technical")).unwrap()
        ).unwrap();
        assert_eq!(technical.schema, "aniflow.audio-technical-inspection/v2");
        assert_eq!(technical.provider.version, "2.0.0");
        assert_eq!(technical.codec, codec);
        assert_eq!(technical.sample_format, sample_format);
        assert_eq!(technical.source.sha256, digest(&source));
        assert_eq!(technical.source.byte_size, source.len() as u64);
        assert_eq!(technical.pcm_sha256, digest(&pcm));
        assert_eq!(technical.decoded_pcm_sha256, digest(&pcm));
        assert_eq!(technical.sample_rate_hz, rate);
        assert_eq!(technical.channels, channels);
        assert_eq!(technical.frame_count, 257);
        assert_eq!(technical.pcm_bitrate_bits_per_second, u64::from(rate) * u64::from(channels) * bits);
        assert_eq!(technical.duration.numerator * i64::from(rate), 257 * i64::from(technical.duration.denominator));
        assert!(technical.decode_complete && technical.source_unchanged);
        let arguments = &technical.commands[1].arguments;
        let codec_index = arguments.iter().position(|argument| argument == "-codec:a").unwrap();
        assert_eq!(arguments[codec_index + 1], codec);
        assert!(!arguments.iter().any(|argument| argument == "-ar" || argument == "-ac" || argument == "-af"));
        let analysis = AudioAnalysis::from_json_slice(&fs::read(artifact(&outcome, "analysis")).unwrap()).unwrap();
        assert_eq!(analysis.source.sample_rate_hz, rate);
        assert_eq!(analysis.source.channels, channels);
        assert_eq!(analysis.source.frame_count, 257);
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn native_inspection_resume_reuses_exact_bytes_and_refuses_source_mutation() {
    for float in [false, true] {
        let fixture = Fixture::new(48000, 2, false);
        let (mut source, _) = native_wave_bytes(48000, 2, 257, float);
        fs::write(&fixture.input, &source).unwrap();
        let outcome = fixture.run().unwrap();
        let launches = fs::read(fixture.root.path().join("launches")).unwrap();
        let resumed = audio_inspection::resume(
            fixture.request(), &outcome.run_directory, &CancellationToken::default(), |_| {}
        ).unwrap();
        assert_eq!(resumed.reused_stages, ["inspect_audio"]);
        assert!(resumed.executed_stages.is_empty());
        assert_eq!(resumed.outputs, outcome.outputs);
        assert_eq!(fs::read(fixture.root.path().join("launches")).unwrap(), launches);
        // Alter a native sample's low bit while preserving width, channel count,
        // duration and valid float representation. The source digest must win.
        source[44] ^= 1;
        fs::write(&fixture.input, &source).unwrap();
        assert!(audio_inspection::resume(
            fixture.request(), &outcome.run_directory, &CancellationToken::default(), |_| {}
        ).is_err());
        assert_eq!(fs::read(fixture.root.path().join("launches")).unwrap(), launches);
        assert_eq!(fs::read(&fixture.input).unwrap(), source);
    }
}

#[test]
fn native_nonfinite_samples_and_malformed_payloads_never_complete() {
    for variant in ["nan", "positive_infinity", "negative_infinity", "wrong_alignment", "missing_padding", "incomplete_frame"] {
        let fixture = Fixture::new(8000, 1, false);
        let float = matches!(variant, "nan" | "positive_infinity" | "negative_infinity");
        let (mut source, _) = native_wave_bytes(8000, 1, 257, float);
        match variant {
            "nan" => source[44..48].copy_from_slice(&f32::NAN.to_le_bytes()),
            "positive_infinity" => source[44..48].copy_from_slice(&f32::INFINITY.to_le_bytes()),
            "negative_infinity" => source[44..48].copy_from_slice(&f32::NEG_INFINITY.to_le_bytes()),
            "wrong_alignment" => source[32..34].copy_from_slice(&2_u16.to_le_bytes()),
            "missing_padding" => { source.pop(); },
            "incomplete_frame" => {
                source[40..44].copy_from_slice(&(257_u32 * 3 - 1).to_le_bytes());
                source.truncate(source.len() - 2);
                let riff_size = u32::try_from(source.len() - 8).unwrap();
                source[4..8].copy_from_slice(&riff_size.to_le_bytes());
            },
            _ => unreachable!(),
        }
        fs::write(&fixture.input, &source).unwrap();
        fixture.assert_failed();
        assert_eq!(fs::read(&fixture.input).unwrap(), source, "{variant}");
    }
}

#[test]
fn missing_version_mismatch_and_overflow_have_typed_preflight_diagnostics() {
    for (mode, expected) in [
        ("wrong_version", "tool_version_mismatch"),
        ("version_flood", "tool_output_limit"),
        ("missing", "missing_tool"),
    ] {
        let fixture = Fixture::new(8000, 1, false);
        fixture.mode(mode);
        if mode == "missing" {
            fs::remove_file(&fixture.configuration.ffmpeg.executable).unwrap();
        }
        let failure =
            audio_inspection::plan(&fixture.request(), &CancellationToken::default()).unwrap_err();
        let value = serde_json::to_value(failure.preflight.unwrap()).unwrap();
        assert_eq!(value["ready"], false);
        assert_eq!(value["diagnostics"][0]["code"], expected);
    }
}

#[test]
fn stale_tool_source_and_repin_cannot_reuse_a_completed_run() {
    let mut fixture = Fixture::new(44100, 1, true);
    let outcome = fixture.run().unwrap();
    fs::write(
        &fixture.configuration.ffmpeg.executable,
        format!("{TOOL}\n# changed\n"),
    )
    .unwrap();
    let error = audio_inspection::resume(
        fixture.request(),
        &outcome.run_directory,
        &CancellationToken::default(),
        |_| {},
    )
    .unwrap_err();
    assert_eq!(
        serde_json::to_value(error.preflight.unwrap()).unwrap()["diagnostics"][0]["code"],
        "tool_digest_mismatch"
    );
    fixture.configuration.ffmpeg.sha256 =
        digest(&fs::read(&fixture.configuration.ffmpeg.executable).unwrap());
    assert!(
        audio_inspection::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .is_err()
    );
    fs::write(&fixture.configuration.ffmpeg.executable, TOOL).unwrap();
    fixture.configuration.ffmpeg.sha256 = digest(TOOL.as_bytes());
    fs::write(&fixture.input, wave_bytes(44100, 1, 258, true)).unwrap();
    assert!(
        audio_inspection::resume(
            fixture.request(),
            &outcome.run_directory,
            &CancellationToken::default(),
            |_| {}
        )
        .is_err()
    );
}

#[test]
fn malformed_and_unsupported_sources_never_complete() {
    for variant in [
        "truncated",
        "float_with_pcm16_width",
        "three_channels",
        "bad_rate",
        "empty",
        "extra_bytes",
    ] {
        let fixture = Fixture::new(8000, 1, false);
        let mut bytes = fs::read(&fixture.input).unwrap();
        match variant {
            "truncated" => {
                bytes.pop();
            }
            "float_with_pcm16_width" => bytes[20..22].copy_from_slice(&3_u16.to_le_bytes()),
            "three_channels" => bytes = wave_bytes(8000, 3, 257, false),
            "bad_rate" => bytes = wave_bytes(4000, 1, 257, false),
            "empty" => bytes = wave_bytes(8000, 1, 0, false),
            _ => bytes.push(0),
        }
        fs::write(&fixture.input, bytes).unwrap();
        fixture.assert_failed();
    }
}

#[test]
fn provider_failure_mismatch_timeout_and_capture_overflow_never_complete() {
    for mode in ["nonzero", "wrong_frames", "wrong_hash", "flood", "sleep"] {
        let mut fixture = Fixture::new(8000, 1, true);
        fixture.mode(mode);
        fixture.configuration.tool_timeout_milliseconds = if mode == "sleep" { 200 } else { 2000 };
        let start = Instant::now();
        fixture.assert_failed();
        // Include debug-build identity hashing while still catching an unbounded 120s child.
        assert!(start.elapsed() < Duration::from_secs(60));
    }
}

#[test]
fn cancellation_during_tool_execution_preserves_recovery_state() {
    let fixture = Fixture::new(8000, 1, true);
    fixture.mode("sleep");
    let cancellation = CancellationToken::default();
    let token = cancellation.clone();
    let started = fixture.root.path().join("started");
    let cancel = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !started.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        token.cancel();
    });
    let mut run = None;
    let result = audio_inspection::run(
        fixture.request(),
        Some(fixture.root.path().join("runs")),
        &cancellation,
        |progress| {
            if let PipelineV3RunProgress::Started { run_directory, .. } = progress {
                run = Some(run_directory.clone());
            }
        },
    );
    cancel.join().unwrap();
    assert!(result.is_err());
    assert_eq!(
        status_v3(run.unwrap()).unwrap().payload.state,
        PipelineV3RunState::Cancelled
    );
}

#[test]
fn canonical_cli_reports_dependency_diagnostics() {
    let fixture = Fixture::new(8000, 1, false);
    fixture.mode("wrong_version");
    let configuration = fixture.root.path().join("tools.json");
    fs::write(
        &configuration,
        serde_json::to_vec(&fixture.configuration).unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aniflow"))
        .args(["--output", "json", "audio", "plan", "--input"])
        .arg(&fixture.input)
        .arg("--configuration")
        .arg(configuration)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(report.to_string().contains("tool_version_mismatch"));
    assert!(!fixture.root.path().join("runs").exists());
}
