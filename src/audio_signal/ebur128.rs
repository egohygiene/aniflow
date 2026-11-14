//! Strict parsing of bounded, pinned-tool EBU meter exports.
use crate::audio_analysis::AudioFrameRange;
use crate::{Error, ErrorCategory, Result};

pub(super) struct EburSummary {
    pub integrated: f64,
    pub loudness_range: f64,
    pub lra_threshold: f64,
    pub true_peak: Option<f64>,
}

pub(super) struct ShortTermValue {
    pub range: AudioFrameRange,
    pub loudness: f64,
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}

fn scalar(line: &str, prefix: &str, unit: &str) -> Result<f64> {
    let value = line
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(unit))
        .ok_or_else(|| invalid("signal_tool_output: unexpected meter summary field"))?
        .trim();
    let value: f64 = value
        .parse()
        .map_err(|_| invalid("signal_tool_output: malformed numeric meter value"))?;
    if !value.is_finite() || !(-200.0..=100.0).contains(&value) {
        return Err(invalid(
            "signal_tool_output: non-finite or implausible meter value",
        ));
    }
    Ok(value)
}

fn decimal(value: &str) -> Option<u64> {
    (!value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn progress_clock(value: &str, minimum_hour_digits: usize) -> bool {
    let parts: Vec<_> = value.split(':').collect();
    parts.len() == 3
        && parts[0].len() >= minimum_hour_digits
        && decimal(parts[0]).is_some()
        && parts[1].len() == 2
        && decimal(parts[1]).is_some_and(|minutes| minutes < 60)
        && parts[2].len() == 5
        && parts[2].as_bytes()[2] == b'.'
        && decimal(&parts[2][..2]).is_some_and(|seconds| seconds < 60)
        && decimal(&parts[2][3..]).is_some()
}

fn null_output_trailer(lines: &[&str]) -> bool {
    // FFmpeg n8.0.1 fftools/ffmpeg_mux.c emits the null mux accounting line;
    // fftools/ffmpeg.c emits final progress even when -nostats is selected.
    // Accept only this audio-only pair, never arbitrary trailing diagnostics.
    let Some((address, accounting)) = lines[0]
        .strip_prefix("[out#0/null @ 0x")
        .and_then(|value| value.split_once("] video:0KiB audio:"))
    else {
        return false;
    };
    if address.is_empty()
        || address.len() > 16
        || !address.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !accounting
            .strip_suffix(
                "KiB subtitle:0KiB other streams:0KiB global headers:0KiB muxing overhead: unknown",
            )
            .is_some_and(|audio| decimal(audio).is_some())
    {
        return false;
    }
    let Some((time, progress)) = lines[1]
        .strip_prefix("size=N/A time=")
        .and_then(|value| value.split_once(" bitrate=N/A speed="))
    else {
        return false;
    };
    let Some((speed, elapsed)) = progress.split_once(" elapsed=") else {
        return false;
    };
    let speed = speed.trim_start_matches(' ');
    let Some(speed) = speed.strip_suffix('x') else {
        return false;
    };
    progress_clock(time, 2)
        && progress_clock(elapsed, 1)
        && !speed.is_empty()
        && speed.len() <= 24
        && speed.starts_with(|character: char| character.is_ascii_digit())
        && speed
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-'))
        && speed
            .parse::<f64>()
            .is_ok_and(|speed| speed.is_finite() && speed >= 0.0)
}

pub(super) fn summary(stderr: &[u8]) -> Result<EburSummary> {
    let text = std::str::from_utf8(stderr)
        .map_err(|_| invalid("signal_tool_output: meter output is not UTF-8"))?;
    let summary = text
        .rsplit_once("Summary:")
        .ok_or_else(|| invalid("signal_tool_output: complete meter summary is missing"))?
        .1;
    let lines: Vec<_> = summary
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if !(lines.len() == 10 || (lines.len() == 12 && null_output_trailer(&lines[10..])))
        || lines[0] != "Integrated loudness:"
        || lines[3] != "Loudness range:"
        || lines[8] != "True peak:"
    {
        return Err(invalid(
            "signal_tool_output: unsupported meter summary layout",
        ));
    }
    let integrated = scalar(lines[1], "I:", "LUFS")?;
    scalar(lines[2], "Threshold:", "LUFS")?;
    let loudness_range = scalar(lines[4], "LRA:", "LU")?;
    let lra_threshold = scalar(lines[5], "Threshold:", "LUFS")?;
    let low = scalar(lines[6], "LRA low:", "LUFS")?;
    let high = scalar(lines[7], "LRA high:", "LUFS")?;
    if loudness_range < 0.0 || high < low || (high - low - loudness_range).abs() > 0.21 {
        return Err(invalid(
            "signal_tool_output: inconsistent loudness range evidence",
        ));
    }
    let true_peak = if lines[9]
        .strip_prefix("Peak:")
        .is_some_and(|value| value.trim() == "-inf dBFS")
    {
        None
    } else {
        Some(scalar(lines[9], "Peak:", "dBFS")?)
    };
    Ok(EburSummary {
        integrated,
        loudness_range,
        lra_threshold,
        true_peak,
    })
}

pub(super) fn short_term(stdout: &[u8], rate: u32, frames: u64) -> Result<Vec<ShortTermValue>> {
    let text = std::str::from_utf8(stdout)
        .map_err(|_| invalid("signal_tool_output: short-term output is not UTF-8"))?;
    let lines: Vec<_> = text.lines().collect();
    let hop = u64::from(rate / 10);
    if hop == 0 || rate % 10 != 0 || lines.len() as u64 != (frames / hop) * 2 {
        return Err(invalid(
            "signal_tool_output: short-term record count does not match complete 100 ms input windows",
        ));
    }
    let window = u64::from(rate) * 3;
    let mut values = Vec::new();
    for (index, pair) in lines.chunks_exact(2).enumerate() {
        let fields: Vec<_> = pair[0].split_whitespace().collect();
        if fields.len() != 3 {
            return Err(invalid("signal_tool_output: malformed meter frame header"));
        }
        let frame_index = fields[0]
            .strip_prefix("frame:")
            .and_then(|value| value.parse::<u64>().ok());
        let pts = fields[1]
            .strip_prefix("pts:")
            .and_then(|value| value.parse::<u64>().ok());
        let pts_time = fields[2]
            .strip_prefix("pts_time:")
            .and_then(|value| value.parse::<f64>().ok());
        if frame_index != Some(index as u64)
            || pts != Some(index as u64 * hop)
            || pts_time.is_none_or(|time| {
                !time.is_finite()
                    || time < 0.0
                    || (time - (index as u64 * hop) as f64 / f64::from(rate)).abs() > 0.001
            })
        {
            return Err(invalid(
                "signal_tool_output: unexpected short-term frame ordering or timestamp",
            ));
        }
        let loudness = scalar(pair[1], "lavfi.r128.S=", "")?;
        let end = (index as u64 + 1) * hop;
        if end >= window {
            values.push(ShortTermValue {
                range: AudioFrameRange {
                    start: end - window,
                    end,
                },
                loudness,
            });
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMMARY: &str = "tool logs\nSummary:\n\nIntegrated loudness:\nI: -20.1 LUFS\nThreshold: -30.1 LUFS\nLoudness range:\nLRA: 2.0 LU\nThreshold: -40.1 LUFS\nLRA low: -21.0 LUFS\nLRA high: -19.0 LUFS\nTrue peak:\nPeak: -6.0 dBFS\n";
    const NULL_TRAILER: &str = "[out#0/null @ 0x557ac1cb50c0] video:0KiB audio:500KiB subtitle:0KiB other streams:0KiB global headers:0KiB muxing overhead: unknown\nsize=N/A time=00:00:02.66 bitrate=N/A speed= 178x elapsed=0:00:00.01    \n";

    #[test]
    fn exact_ffmpeg_801_audio_null_trailer_retains_the_strict_meter_contract() {
        let native = format!("{SUMMARY}{NULL_TRAILER}");
        let parsed = summary(native.as_bytes()).unwrap();
        assert_eq!(parsed.integrated, -20.1);
        assert_eq!(parsed.true_peak, Some(-6.0));
        let scientific_speed = native.replace("speed= 178x", "speed=1.23e+03x");
        assert!(summary(scientific_speed.as_bytes()).is_ok());
        for corrupt in [
            native.replace("0x557ac1cb50c0", "0xnot-an-address"),
            native.replace("video:0KiB", "video:1KiB"),
            native.replace("audio:500KiB", "audio:NaNKiB"),
            native.replace("subtitle:0KiB", "subtitle:1KiB"),
            native.replace("other streams:0KiB", "other streams:1KiB"),
            native.replace("global headers:0KiB", "global headers:1KiB"),
            native.replace("muxing overhead: unknown", "muxing overhead: 0%"),
            native.replace("time=00:00:02.66", "time=00:00:62.66"),
            native.replace("speed= 178x", "speed= NaNx"),
            native.replace("speed= 178x", "speed= -1x"),
            native.replace("elapsed=0:00:00.01", "elapsed=invalid"),
            native.replace("Peak: -6.0 dBFS", "Peak: -6.0 LUFS"),
            native.replace("LRA high: -19.0 LUFS\n", ""),
            native.replace("True peak:\n", "True peak:\nUnexpected field:\n"),
            native.replace(NULL_TRAILER, "warning: unexpected diagnostic\n"),
            format!("{native}warning: unexpected diagnostic\n"),
        ] {
            assert!(summary(corrupt.as_bytes()).is_err(), "accepted: {corrupt}");
        }
    }

    #[test]
    fn pinned_summary_layout_retains_distinct_units_and_gate_threshold() {
        let parsed = summary(SUMMARY.as_bytes()).unwrap();
        assert_eq!(parsed.integrated, -20.1);
        assert_eq!(parsed.loudness_range, 2.0);
        assert_eq!(parsed.lra_threshold, -40.1);
        assert_eq!(parsed.true_peak, Some(-6.0));
    }

    #[test]
    fn nonfinite_or_incomplete_evidence_is_not_a_zero_measurement() {
        for corrupt in [
            SUMMARY.replace("-20.1", "NaN"),
            SUMMARY.replace("-20.1", "-inf"),
            SUMMARY.replace("Peak: -6.0 dBFS", ""),
            SUMMARY.replace("LRA: 2.0", "LRA: 30.0"),
        ] {
            assert!(summary(corrupt.as_bytes()).is_err());
        }
        assert_eq!(
            summary(SUMMARY.replace("Peak: -6.0", "Peak: -inf").as_bytes())
                .unwrap()
                .true_peak,
            None
        );
    }

    #[test]
    fn complete_short_term_window_ends_after_its_export_frame_start() {
        let mut text = String::new();
        for index in 0..30 {
            text += &format!(
                "frame:{index} pts:{} pts_time:{}\nlavfi.r128.S=-20.000\n",
                index * 800,
                index as f64 / 10.0
            );
        }
        let values = short_term(text.as_bytes(), 8000, 24001).unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(
            values[0].range,
            AudioFrameRange {
                start: 0,
                end: 24000
            }
        );
        assert_eq!(values[0].loudness, -20.0);
        assert!(short_term(text.replace("pts:800 ", "pts:801 ").as_bytes(), 8000, 24001).is_err());
        assert!(short_term(text.replace("S=-20.000", "S=NaN").as_bytes(), 8000, 24001).is_err());
        assert!(short_term(text.as_bytes(), 8000, 24800).is_err());
        assert!(short_term(text.as_bytes(), 11025, 24001).is_err());
    }

    #[test]
    fn sub_100ms_audio_has_no_exported_records() {
        assert!(short_term(b"", 8000, 799).unwrap().is_empty());
        assert!(short_term(b"", 8000, 800).is_err());
    }
}
