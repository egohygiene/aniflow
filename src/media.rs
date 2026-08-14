use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInspection {
    pub source: String,
    pub duration_seconds: f64,
    pub width: u64,
    pub height: u64,
    pub average_frame_rate: String,
    pub frames_per_second: f64,
    pub estimated_frame_count: u64,
    pub video_codec: String,
    pub pixel_format: Option<String>,
    pub has_audio: bool,
    pub audio_codec: Option<String>,
    pub has_subtitles: bool,
}

pub fn inspect(input: &Path) -> Result<MediaInspection> {
    if !input.is_file() {
        bail!("input video does not exist: {}", input.display());
    }

    let canonical_input = input
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", input.display()))?;
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(&canonical_input)
        .output()
        .context("failed to execute ffprobe")?;

    if !output.status.success() {
        bail!(
            "ffprobe could not inspect {}: {}",
            canonical_input.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let root: Value =
        serde_json::from_slice(&output.stdout).context("ffprobe returned invalid JSON")?;
    let streams = root["streams"]
        .as_array()
        .context("ffprobe response did not contain streams")?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .context("source does not contain a video stream")?;
    let audio = streams
        .iter()
        .find(|stream| stream["codec_type"] == "audio");
    let has_subtitles = streams
        .iter()
        .any(|stream| stream["codec_type"] == "subtitle");

    let duration_seconds = root["format"]["duration"]
        .as_str()
        .or_else(|| video["duration"].as_str())
        .unwrap_or("0")
        .parse::<f64>()
        .context("source duration is not numeric")?;
    let average_frame_rate = video["avg_frame_rate"].as_str().unwrap_or("0/1").to_owned();
    let frames_per_second = parse_frame_rate(&average_frame_rate)?;
    let estimated_frame_count = (duration_seconds * frames_per_second).round() as u64;

    Ok(MediaInspection {
        source: canonical_input.display().to_string(),
        duration_seconds,
        width: video["width"].as_u64().unwrap_or_default(),
        height: video["height"].as_u64().unwrap_or_default(),
        average_frame_rate,
        frames_per_second,
        estimated_frame_count,
        video_codec: string_field(video, "codec_name").unwrap_or_else(|| "unknown".to_owned()),
        pixel_format: string_field(video, "pix_fmt"),
        has_audio: audio.is_some(),
        audio_codec: audio.and_then(|stream| string_field(stream, "codec_name")),
        has_subtitles,
    })
}

pub fn parse_frame_rate(value: &str) -> Result<f64> {
    let (numerator, denominator) = value
        .split_once('/')
        .context("frame rate must use numerator/denominator form")?;
    let numerator = numerator
        .parse::<f64>()
        .context("invalid frame-rate numerator")?;
    let denominator = denominator
        .parse::<f64>()
        .context("invalid frame-rate denominator")?;

    if numerator <= 0.0 || denominator <= 0.0 {
        bail!("frame rate must be greater than zero");
    }

    Ok(numerator / denominator)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value[key].as_str().map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::parse_frame_rate;

    #[test]
    fn parses_ntsc_frame_rate() {
        let actual = parse_frame_rate("30000/1001").expect("frame rate should parse");
        assert!((actual - 29.970_029_97).abs() < 0.000_001);
    }
}
