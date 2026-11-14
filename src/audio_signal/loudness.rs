//! Source-format-preserving meter composition over existing #13 parsers.
//! This separately versioned receipt does not change native-v3's unavailable
//! meter contract or declare an unqualified algorithm qualified.
use super::{
    AudioSignalMeasurement, AudioSignalUnavailableReason, AudioSignalUnit, AudioSignalValue,
};
use crate::audio_analysis::AudioScope;
use crate::audio_inspection::process::{GroupPolicy, hash_regular, run_tool_isolated};
use crate::audio_inspection::{AudioInspectionConfiguration, NativeSampleFormat};
use crate::{CancellationToken, Error, ErrorCategory, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeLoudnessReceipt {
    pub schema: String,
    pub pcm_sha256: String,
    pub frames: u64,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub integrated_loudness: AudioSignalMeasurement,
    pub true_peak: AudioSignalMeasurement,
    pub method: String,
    pub commands: Vec<crate::audio_inspection::AudioTechnicalCommandEvidence>,
    pub runtime_qualified: bool,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCategory::Media, message)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MeterProfile {
    Float48,
    Native48,
    Native441,
}
impl MeterProfile {
    fn rate(self) -> u32 {
        if self == Self::Native441 {
            44100
        } else {
            48000
        }
    }
    fn schema(self) -> &'static str {
        match self {
            Self::Float48 => "aniflow.native-pcm-loudness/v1",
            Self::Native48 => "aniflow.native-pcm-loudness/v2",
            Self::Native441 => "aniflow.native-pcm-loudness/v3",
        }
    }
    fn method(self) -> &'static str {
        if self == Self::Native441 {
            "Existing #13 FFmpeg 8.0.1 ebur128 two-pass meter/parser, 44.1 kHz native PCM16/PCM24/float32; original-rate windows and independent native sample peak; no quantization, resampling or downmix"
        } else {
            method(self == Self::Native48)
        }
    }
    fn parse(schema: &str) -> Result<Self> {
        match schema {
            "aniflow.native-pcm-loudness/v1" => Ok(Self::Float48),
            "aniflow.native-pcm-loudness/v2" => Ok(Self::Native48),
            "aniflow.native-pcm-loudness/v3" => Ok(Self::Native441),
            _ => Err(invalid("unsupported native meter receipt version")),
        }
    }
}

/// Structural readback of a hash-bound observation; does not rerun or qualify
/// the meter, or turn a supplied numeric observation into musical approval.
pub(crate) fn validate_receipt(receipt: &NativeLoudnessReceipt) -> Result<()> {
    let profile = MeterProfile::parse(&receipt.schema)?;
    let scope = AudioScope {
        channels: (0..receipt.channels).collect(),
        stem_id: None,
    };
    if receipt.sample_rate_hz != profile.rate()
        || !matches!(receipt.channels, 1 | 2)
        || receipt.frames == 0
        || receipt.runtime_qualified
        || receipt.commands != super::provider::command_evidence(profile.rate())
        || receipt.method != profile.method()
        || receipt.integrated_loudness.scope != scope
        || receipt.true_peak.scope != scope
        || receipt.integrated_loudness.unit != AudioSignalUnit::LoudnessUnitsFullScale
        || receipt.true_peak.unit != AudioSignalUnit::DecibelsTruePeak
    {
        return Err(invalid(
            "native loudness receipt differs from the fixed unqualified complete-PCM meter profile",
        ));
    }
    crate::provider::require_sha256(&receipt.pcm_sha256, "native meter PCM")?;
    for measurement in [&receipt.integrated_loudness, &receipt.true_peak] {
        if matches!(&measurement.value,AudioSignalValue::Measured {value} if !value.is_finite()) {
            return Err(invalid("native loudness observation is not finite"));
        }
    }
    Ok(())
}

fn quantities(
    rate: u32,
    frames: u64,
    peaks: &[f64],
    integrated: f64,
    true_peak: Option<f64>,
) -> Result<(AudioSignalMeasurement, AudioSignalMeasurement)> {
    if rate != 48_000 {
        return Err(invalid("legacy native meter supports 48 kHz only"));
    }
    quantities_for_rate(rate, frames, peaks, integrated, true_peak)
}
fn quantities_for_rate(
    rate: u32,
    frames: u64,
    peaks: &[f64],
    integrated: f64,
    true_peak: Option<f64>,
) -> Result<(AudioSignalMeasurement, AudioSignalMeasurement)> {
    if !matches!(rate, 44100 | 48000)
        || frames == 0
        || !matches!(peaks.len(), 1 | 2)
        || peaks.iter().any(|p| !p.is_finite() || *p < 0.0)
        || !integrated.is_finite()
    {
        return Err(invalid("unsupported native loudness observation"));
    }
    let scope = AudioScope {
        channels: (0..peaks.len() as u16).collect(),
        stem_id: None,
    };
    let peak = peaks.iter().copied().fold(0.0_f64, f64::max);
    let unavailable = |reason| AudioSignalValue::Unavailable { reason };
    let method = super::AudioSignalMethod::for_sample_rate_v2(rate);
    let integrated = if peak == 0.0 {
        unavailable(AudioSignalUnavailableReason::SilentInput)
    } else if frames < method.integrated_minimum_frames {
        unavailable(AudioSignalUnavailableReason::InsufficientDuration)
    } else if integrated <= -70.0 {
        unavailable(AudioSignalUnavailableReason::BelowAbsoluteGate)
    } else {
        AudioSignalValue::Measured { value: integrated }
    };
    let true_peak = if peak == 0.0 {
        if true_peak.is_some() {
            return Err(invalid("silent PCM returned finite true peak"));
        }
        unavailable(AudioSignalUnavailableReason::SilentInput)
    } else {
        let value = true_peak
            .filter(|p| p.is_finite())
            .ok_or_else(|| invalid("nonzero PCM returned no finite true peak"))?;
        if value + f64::from(method.true_peak_sample_peak_tolerance_millidecibels) / 1000.0
            < 20.0 * peak.log10()
        {
            return Err(invalid(
                "true peak is below independently observed sample peak",
            ));
        }
        AudioSignalValue::Measured { value }
    };
    Ok((
        AudioSignalMeasurement {
            unit: AudioSignalUnit::LoudnessUnitsFullScale,
            scope: scope.clone(),
            value: integrated,
        },
        AudioSignalMeasurement {
            unit: AudioSignalUnit::DecibelsTruePeak,
            scope,
            value: true_peak,
        },
    ))
}

pub(crate) fn measure(
    path: &Path,
    expected: &(String, u64),
    configuration: &AudioInspectionConfiguration,
    directory: &Path,
    token: &CancellationToken,
) -> Result<NativeLoudnessReceipt> {
    measure_profile(
        path,
        expected,
        configuration,
        directory,
        token,
        MeterProfile::Float48,
    )
}
/// Same existing two-pass owner over exact native integer/float samples. V1
/// float-only wire meanings remain unchanged; this v2 view stays unqualified.
pub(crate) fn measure_native(
    path: &Path,
    expected: &(String, u64),
    configuration: &AudioInspectionConfiguration,
    directory: &Path,
    token: &CancellationToken,
) -> Result<NativeLoudnessReceipt> {
    measure_profile(
        path,
        expected,
        configuration,
        directory,
        token,
        MeterProfile::Native48,
    )
}
/// Explicit new original-rate native profile; old meter/QC meanings stay fixed.
pub(crate) fn measure_native_44100(
    path: &Path,
    expected: &(String, u64),
    configuration: &AudioInspectionConfiguration,
    directory: &Path,
    token: &CancellationToken,
) -> Result<NativeLoudnessReceipt> {
    measure_profile(
        path,
        expected,
        configuration,
        directory,
        token,
        MeterProfile::Native441,
    )
}
fn method(native: bool) -> &'static str {
    if native {
        "Existing #13 FFmpeg 8.0.1 ebur128 two-pass meter/parser, 48 kHz native PCM16/PCM24/float32; independent native sample peak; no quantization, resampling or downmix"
    } else {
        "Existing #13 FFmpeg 8.0.1 ebur128 two-pass meter/parser, 48 kHz float32; independent native sample peak; no PCM16 quantization"
    }
}
fn measure_profile(
    path: &Path,
    expected: &(String, u64),
    configuration: &AudioInspectionConfiguration,
    directory: &Path,
    token: &CancellationToken,
    profile: MeterProfile,
) -> Result<NativeLoudnessReceipt> {
    configuration.validate()?;
    if configuration.ffmpeg.version != "8.0.1" || configuration.ffprobe.version != "8.0.1" {
        return Err(invalid("native meter requires exact 8.0.1 tools"));
    }
    if hash_regular(
        path,
        crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES,
    )
    .map_err(|_| invalid("PCM meter source unavailable"))?
        != *expected
    {
        return Err(invalid("PCM meter source changed"));
    }
    let wave = crate::audio_inspection::wav::inspect_native(path)?;
    if (profile == MeterProfile::Float48 && wave.sample_format != NativeSampleFormat::Float32)
        || wave.sample_rate_hz != profile.rate()
        || !matches!(wave.channels, 1 | 2)
    {
        return Err(invalid(
            "native loudness requires its exact selected original-rate mono/stereo sample profile",
        ));
    }
    let samples = super::pcm::measure_native(path, &wave, 0.0, 100, 1.0)?;
    let commands = super::provider::command_evidence(wave.sample_rate_hz);
    let mut captures = Vec::new();
    for command in &commands {
        let arguments: Vec<OsString> = command
            .arguments
            .iter()
            .map(|v| {
                if v == "{snapshot}" {
                    path.as_os_str().to_owned()
                } else {
                    v.into()
                }
            })
            .collect();
        captures.push(
            run_tool_isolated(
                &configuration.ffmpeg,
                "ffmpeg",
                &arguments,
                configuration,
                token,
                GroupPolicy::Own,
                Some(directory),
            )
            .map_err(|_| invalid("native loudness tool capture failed or exceeded its bound"))?,
        );
    }
    let main = super::ebur128::summary(&captures[0].stderr)?;
    super::ebur128::short_term(&captures[0].stdout, wave.sample_rate_hz, wave.frame_count)?;
    if captures.len() != 2 || !captures[1].stdout.is_empty() {
        return Err(invalid(
            "native true-peak pass differs from the fixed meter profile",
        ));
    }
    let peak = super::ebur128::summary(&captures[1].stderr)?.true_peak;
    let peaks = samples
        .channels
        .iter()
        .map(|v| v.sample_peak_ratio)
        .collect::<Vec<_>>();
    let (integrated_loudness, true_peak) = if profile == MeterProfile::Native441 {
        quantities_for_rate(
            wave.sample_rate_hz,
            wave.frame_count,
            &peaks,
            main.integrated,
            peak,
        )?
    } else {
        quantities(
            wave.sample_rate_hz,
            wave.frame_count,
            &peaks,
            main.integrated,
            peak,
        )?
    };
    if hash_regular(
        path,
        crate::audio_inspection::AUDIO_INSPECTION_MAXIMUM_BYTES,
    )
    .map_err(|_| invalid("PCM meter source unavailable"))?
        != *expected
    {
        return Err(invalid("PCM meter source changed during measurement"));
    }
    Ok(NativeLoudnessReceipt {
        schema: profile.schema().into(),
        pcm_sha256: wave.pcm_sha256,
        frames: wave.frame_count,
        sample_rate_hz: wave.sample_rate_hz,
        channels: wave.channels,
        integrated_loudness,
        true_peak,
        method: profile.method().into(),
        commands,
        runtime_qualified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_44100_uses_original_rate_windows_and_keeps_legacy_rate_refusal() {
        let (l, p) = quantities_for_rate(44100, 44100, &[0.5, 0.25], -12.0, Some(-5.5)).unwrap();
        assert_eq!(l.value, AudioSignalValue::Measured { value: -12.0 });
        assert_eq!(p.scope.channels, vec![0, 1]);
        let (short, _) = quantities_for_rate(44100, 100, &[0.5], -12.0, Some(-5.5)).unwrap();
        assert_eq!(
            short.value,
            AudioSignalValue::Unavailable {
                reason: AudioSignalUnavailableReason::InsufficientDuration
            }
        );
        assert!(quantities(44100, 44100, &[0.5], -12.0, Some(-5.5)).is_err());
        assert!(quantities_for_rate(96000, 96000, &[0.5], -12.0, Some(-5.5)).is_err());
    }
    #[test]
    fn native_44100_receipt_version_rate_commands_and_qualification_are_exact() {
        let (integrated_loudness, true_peak) =
            quantities_for_rate(44100, 44100, &[0.5], -12.0, Some(-5.5)).unwrap();
        let profile = MeterProfile::Native441;
        let mut r = NativeLoudnessReceipt {
            schema: profile.schema().into(),
            pcm_sha256: "a".repeat(64),
            frames: 44100,
            sample_rate_hz: 44100,
            channels: 1,
            integrated_loudness,
            true_peak,
            method: profile.method().into(),
            commands: super::super::provider::command_evidence(44100),
            runtime_qualified: false,
        };
        validate_receipt(&r).unwrap();
        let old = r.clone();
        r.sample_rate_hz = 48000;
        assert!(validate_receipt(&r).is_err());
        r = old.clone();
        r.schema = "aniflow.native-pcm-loudness/v2".into();
        assert!(validate_receipt(&r).is_err());
        r = old;
        r.runtime_qualified = true;
        assert!(validate_receipt(&r).is_err());
    }
    #[test]
    fn native_44100_silence_gate_and_finite_overs_keep_explicit_quantities() {
        let (l, p) = quantities_for_rate(44100, 44100, &[0.0], -70.0, None).unwrap();
        assert_eq!(
            l.value,
            AudioSignalValue::Unavailable {
                reason: AudioSignalUnavailableReason::SilentInput
            }
        );
        assert_eq!(p.value, l.value);
        let (l, _) = quantities_for_rate(44100, 44100, &[0.5], -71.0, Some(-5.5)).unwrap();
        assert_eq!(
            l.value,
            AudioSignalValue::Unavailable {
                reason: AudioSignalUnavailableReason::BelowAbsoluteGate
            }
        );
        let (_, p) = quantities_for_rate(44100, 44100, &[2.0], -3.0, Some(6.2)).unwrap();
        assert_eq!(p.value, AudioSignalValue::Measured { value: 6.2 });
        assert!(quantities_for_rate(44100, 44100, &[2.0], -3.0, Some(-3.0)).is_err());
    }
    #[test]
    fn native_integer_meter_view_is_versioned_and_never_qualifies_itself() {
        let (integrated_loudness, true_peak) =
            quantities(48000, 48000, &[0.5, 0.25], -12.0, Some(-5.5)).unwrap();
        let mut receipt = NativeLoudnessReceipt {
            schema: "aniflow.native-pcm-loudness/v2".into(),
            pcm_sha256: "a".repeat(64),
            frames: 48000,
            sample_rate_hz: 48000,
            channels: 2,
            integrated_loudness,
            true_peak,
            method: method(true).into(),
            commands: super::super::provider::command_evidence(48000),
            runtime_qualified: false,
        };
        validate_receipt(&receipt).unwrap();
        receipt.runtime_qualified = true;
        assert!(validate_receipt(&receipt).is_err());
        receipt.runtime_qualified = false;
        receipt.schema = "aniflow.native-pcm-loudness/v1".into();
        assert!(validate_receipt(&receipt).is_err());
        receipt.method = method(false).into();
        validate_receipt(&receipt).unwrap();
    }
    #[test]
    fn silence_and_short_inputs_keep_typed_unavailability() {
        let (l, p) = quantities(48_000, 100, &[0.0], -70.0, None).unwrap();
        assert_eq!(
            l.value,
            AudioSignalValue::Unavailable {
                reason: AudioSignalUnavailableReason::SilentInput
            }
        );
        assert_eq!(p.value, l.value);
        let (l, _) = quantities(48_000, 100, &[0.5], -12.0, Some(-6.0)).unwrap();
        assert_eq!(
            l.value,
            AudioSignalValue::Unavailable {
                reason: AudioSignalUnavailableReason::InsufficientDuration
            }
        );
    }
    #[test]
    fn finite_overs_are_retained_and_inconsistent_true_peak_is_refused() {
        let (_, p) = quantities(48_000, 48_000, &[2.0, 0.5], -3.0, Some(6.2)).unwrap();
        assert_eq!(p.value, AudioSignalValue::Measured { value: 6.2 });
        assert!(quantities(48_000, 48_000, &[2.0], -3.0, Some(-3.0)).is_err());
        assert!(quantities(44_100, 44_100, &[0.5], -3.0, Some(-3.0)).is_err());
    }
}
