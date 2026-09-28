//! Closed parser for the pinned four-times-resampled peak measurement export.
use crate::{Error, ErrorCategory, Result};

fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}

/// Accept exactly one five-line Overall block from the requested astats instance.
/// The peak is absent only for the explicit negative-infinity silence sentinel;
/// the caller must corroborate silence against independently observed PCM.
pub(super) fn true_peak(
    stderr: &[u8],
    source_frames: u64,
    sample_rate_hz: u32,
) -> Result<Option<f64>> {
    let expected_samples = source_frames
        .checked_add(u64::from(sample_rate_hz / 10))
        .and_then(|frames| frames.checked_mul(4))
        .ok_or_else(|| invalid("signal_tool_output: resampled frame count overflow"))?;
    let text = std::str::from_utf8(stderr)
        .map_err(|_| invalid("signal_tool_output: astats output is not UTF-8"))?;
    let mut prefix = None;
    let mut fields = Vec::new();
    for line in text.lines() {
        if !line.starts_with("[Parsed_astats_") {
            continue;
        }
        let (identity, field) = line
            .split_once("] ")
            .ok_or_else(|| invalid("signal_tool_output: malformed astats log prefix"))?;
        if !identity.starts_with("[Parsed_astats_2 @ ")
            || prefix.is_some_and(|prior| prior != identity)
        {
            return Err(invalid(
                "signal_tool_output: unexpected or multiple astats instances",
            ));
        }
        prefix = Some(identity);
        fields.push(field.trim());
        if fields.len() > 5 {
            return Err(invalid(
                "signal_tool_output: duplicate or unexpected astats fields",
            ));
        }
    }
    if fields.len() != 5
        || fields[0] != "Overall"
        || fields[3] != "Number of NaNs: 0.000000"
        || fields[4] != "Number of Infs: 0.000000"
    {
        return Err(invalid(
            "signal_tool_output: incomplete astats Overall block or nonfinite sample evidence",
        ));
    }
    let count = fields[2]
        .strip_prefix("Number of samples: ")
        .ok_or_else(|| invalid("signal_tool_output: missing astats sample count"))?;
    if count.is_empty()
        || !count.bytes().all(|byte| byte.is_ascii_digit())
        || count.parse::<u64>().ok() != Some(expected_samples)
    {
        return Err(invalid(
            "signal_tool_output: resampled sample count differs from the exact padded four-times source grid",
        ));
    }
    let peak = fields[1]
        .strip_prefix("Peak level dB: ")
        .ok_or_else(|| invalid("signal_tool_output: missing astats peak"))?;
    if peak == "-inf" {
        return Ok(None);
    }
    let unsigned = peak.strip_prefix('-').unwrap_or(peak);
    let valid_decimal = unsigned.split_once('.').is_some_and(|(integer, fraction)| {
        !integer.is_empty()
            && integer.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.len() == 6
            && fraction.bytes().all(|byte| byte.is_ascii_digit())
    });
    let peak = peak
        .parse::<f64>()
        .map_err(|_| invalid("signal_tool_output: malformed astats peak value"))?;
    if !valid_decimal || !peak.is_finite() || !(-400.0..=24.0).contains(&peak) {
        return Err(invalid(
            "signal_tool_output: astats peak must be a finite bounded six-decimal value",
        ));
    }
    Ok(Some(peak))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(peak: &str, count: &str) -> String {
        format!(
            "unrelated FFmpeg banner\n[Parsed_astats_2 @ 0x123] Overall\n[Parsed_astats_2 @ 0x123] Peak level dB: {peak}\n[Parsed_astats_2 @ 0x123] Number of samples: {count}\n[Parsed_astats_2 @ 0x123] Number of NaNs: 0.000000\n[Parsed_astats_2 @ 0x123] Number of Infs: 0.000000\n"
        )
    }

    #[test]
    fn peak_and_silence_require_exact_padded_sample_count() {
        assert_eq!(
            true_peak(block("-6.020600", "422400").as_bytes(), 96000, 96000).unwrap(),
            Some(-6.020600)
        );
        assert_eq!(
            true_peak(block("-inf", "38404").as_bytes(), 1, 96000).unwrap(),
            None
        );
        assert!(true_peak(block("-6.020600", "422399").as_bytes(), 96000, 96000).is_err());
        assert!(true_peak(block("-6.020600", "422400.0").as_bytes(), 96000, 96000).is_err());
        assert!(true_peak(block("-6.020600", "422400").as_bytes(), u64::MAX, 96000).is_err());
    }

    #[test]
    fn missing_duplicate_and_multiple_instance_blocks_are_refused() {
        let valid = block("-6.020600", "422400");
        for value in [
            valid.replace("[Parsed_astats_2 @ 0x123] Overall\n", ""),
            format!("{valid}{valid}"),
            valid.replace("Peak level dB", "RMS level dB"),
            valid.replace(
                "[Parsed_astats_2 @ 0x123] Number of samples",
                "[Parsed_astats_2 @ 0x456] Number of samples",
            ),
            valid.replace("Parsed_astats_2", "Parsed_astats_3"),
        ] {
            assert!(true_peak(value.as_bytes(), 96000, 96000).is_err());
        }
        assert!(true_peak(b"", 96000, 96000).is_err());
        assert!(true_peak(&[255], 96000, 96000).is_err());
    }

    #[test]
    fn malformed_nonfinite_and_unpinned_precision_are_refused() {
        for peak in [
            "NaN",
            "inf",
            "-NaN",
            "1e1",
            "-6.0",
            "-6.0206000",
            "25.000000",
            "--1.000000",
        ] {
            assert!(
                true_peak(block(peak, "422400").as_bytes(), 96000, 96000).is_err(),
                "accepted {peak}"
            );
        }
        for name in ["NaNs", "Infs"] {
            let corrupted = block("-6.020600", "422400").replace(
                &format!("Number of {name}: 0.000000"),
                &format!("Number of {name}: 1.000000"),
            );
            assert!(true_peak(corrupted.as_bytes(), 96000, 96000).is_err());
        }
    }
}
