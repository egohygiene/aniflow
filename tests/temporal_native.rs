//! Synthetic native regression recipes for the later validation pass; not run for #32.
use std::process::Command;

use aniflow::segmentation::{self, CancellationToken, SegmentMode, SegmentRequest};
use aniflow::temporal::{self, RationalTime, StreamSelection, TemporalCode};

fn ffmpeg(arguments: &[&std::ffi::OsStr]) {
    let result = Command::new("ffmpeg")
        .args(arguments)
        .output()
        .expect("install FFmpeg before native temporal qualification");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn fractional_native_segments_and_reconstruction_retain_exact_source_timing() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("fractional café.mp4");
    let mut arguments: Vec<&std::ffi::OsStr> = [
        "-hide_banner",
        "-nostdin",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=32x32:r=24000/1001",
        "-frames:v",
        "4",
        "-c:v",
        "libx264",
        "-bf",
        "0",
        "-pix_fmt",
        "yuv420p",
    ]
    .into_iter()
    .map(std::ffi::OsStr::new)
    .collect();
    arguments.push(source.as_os_str());
    ffmpeg(&arguments);
    let original_bytes = std::fs::read(&source).unwrap();
    let original = temporal::inspect(&source, &StreamSelection::default()).unwrap();
    original.require_processing().unwrap();
    assert_eq!(
        original.video_timeline().unwrap().frame_period,
        Some(RationalTime::new(1001, 24000).unwrap())
    );
    assert_eq!(original.video_timeline().unwrap().frames.len(), 4);
    let directory = root.path().join("segments");
    let request = SegmentRequest::new(&source, &directory, 90, SegmentMode::TranscodeH264Aac);
    let token = CancellationToken::default();
    let outcome = segmentation::execute(&request, &token, &mut |_| {}).unwrap();
    assert_eq!(outcome.segment_count, 2);
    let resumed = segmentation::resume(&directory, &token, &mut |_| {}).unwrap();
    assert_eq!(resumed.generated_segments, 0);
    assert_eq!(resumed.resumed_segments, 2);
    let output = root.path().join("reconstructed.mp4");
    let report = segmentation::reconstruct(&directory, &output, &token, &mut |_| {}).unwrap();
    assert_eq!(
        report.source_streams[0].source_time,
        original.video_timeline().unwrap().source_time.unwrap()
    );
    let derived = temporal::inspect(&output, &StreamSelection::default()).unwrap();
    temporal::validate_reconstruction(&original, &derived, false).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), original_bytes);
}

#[test]
fn native_multi_video_requires_selection_and_explicit_discard() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("multi.mp4");
    let mut arguments: Vec<&std::ffi::OsStr> = [
        "-hide_banner",
        "-nostdin",
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=32x32:r=30",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=32x32:r=30",
        "-map",
        "0:v:0",
        "-map",
        "1:v:0",
        "-frames:v:0",
        "4",
        "-frames:v:1",
        "4",
        "-c:v",
        "libx264",
        "-bf",
        "0",
    ]
    .into_iter()
    .map(std::ffi::OsStr::new)
    .collect();
    arguments.push(source.as_os_str());
    ffmpeg(&arguments);
    let bytes = std::fs::read(&source).unwrap();
    let ambiguous = temporal::inspect(&source, &StreamSelection::default()).unwrap();
    assert!(!ambiguous.processing.supported);
    assert_eq!(ambiguous.diagnostics[0].code, TemporalCode::AmbiguousVideo);
    let intent = StreamSelection {
        video_stream: Some(1),
        discard_streams: vec![0],
        ..StreamSelection::default()
    };
    let selected = temporal::inspect(&source, &intent).unwrap();
    selected.require_processing().unwrap();
    assert_eq!(selected.selected.unwrap().video, Some(1));
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
}
