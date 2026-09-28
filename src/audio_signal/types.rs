//! Closed, source-bound contracts for deterministic PCM signal measurements.
use serde::{Deserialize, Serialize};

use crate::audio_analysis::{AudioArtifactReference, AudioFrameRange, AudioScope, AudioSource};
use crate::audio_inspection::{
    AudioInspectionConfiguration, AudioTechnicalCommandEvidence, AudioToolObservation,
};
use crate::provider::{decode_json, require_sha256};
use crate::{Error, ErrorCategory, ProviderReference, Result};

pub const AUDIO_SIGNAL_CONFIGURATION_SCHEMA_V1: &str = "aniflow.audio-signal.configuration/v1";
pub const AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-signal.provider-configuration/v1";
pub const AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V1: &str = "aniflow.audio-signal-measurements/v1";
pub const AUDIO_SIGNAL_PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-signal";
pub const AUDIO_SIGNAL_PROVIDER_VERSION: &str = "1.0.0";
pub const AUDIO_SIGNAL_CAPABILITY_ID: &str = "aniflow/audio-signal-measurements";

const fn default_silence_threshold() -> u16 {
    32
}
const fn default_minimum_silence() -> u32 {
    100
}
const fn default_clipping_threshold() -> u16 {
    32767
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalAnalysisConfiguration {
    pub schema: String,
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold_pcm: u16,
    #[serde(default = "default_minimum_silence")]
    pub minimum_silence_milliseconds: u32,
    #[serde(default = "default_clipping_threshold")]
    pub clipping_threshold_pcm: u16,
}

impl Default for SignalAnalysisConfiguration {
    fn default() -> Self {
        Self {
            schema: AUDIO_SIGNAL_CONFIGURATION_SCHEMA_V1.to_owned(),
            silence_threshold_pcm: 32,
            minimum_silence_milliseconds: 100,
            clipping_threshold_pcm: 32767,
        }
    }
}

impl SignalAnalysisConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 {
            return Err(invalid("signal configuration exceeds 64 KiB"));
        }
        let value: Self = decode_json(bytes, "audio signal configuration")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_SIGNAL_CONFIGURATION_SCHEMA_V1
            || self.silence_threshold_pcm > 32768
            || !(1..=600_000).contains(&self.minimum_silence_milliseconds)
            || !(1..=32768).contains(&self.clipping_threshold_pcm)
        {
            return Err(invalid(
                "unsupported audio signal configuration or thresholds",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSignalProviderConfiguration {
    pub schema: String,
    pub settings: SignalAnalysisConfiguration,
    pub tools: AudioInspectionConfiguration,
    pub source: AudioArtifactReference,
}

impl AudioSignalProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        self.tools.validate()?;
        if self.schema != AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V1
            || self.source.id != "source_audio"
            || !(44..=268_435_456).contains(&self.source.byte_size)
        {
            return Err(invalid(
                "invalid source-bound signal provider configuration",
            ));
        }
        require_sha256(&self.source.sha256, "signal source sha256")
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self)
            .map_err(|_| invalid("cannot encode signal configuration"))?;
        let values = values
            .as_object()
            .expect("configuration struct is object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_SIGNAL_PROVIDER_ID.to_owned(),
                version: AUDIO_SIGNAL_PROVIDER_VERSION.to_owned(),
            },
            crate::CapabilityReference {
                id: AUDIO_SIGNAL_CAPABILITY_ID.to_owned(),
                version: "1.0.0".to_owned(),
            },
            configuration_schema_reference(),
            values,
        )
    }
}

#[must_use]
pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    use sha2::{Digest, Sha256};
    crate::ConfigurationSchemaReference {
        id: AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V1.to_owned(),
        version: "1.0.0".to_owned(),
        sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../providers/audio-signal/provider-configuration.schema.json"
            ))
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSignalUnit {
    DecibelsFullScale,
    Decibels,
    LoudnessUnitsFullScale,
    LoudnessUnits,
    DecibelsTruePeak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSignalUnavailableReason {
    SilentInput,
    InsufficientDuration,
    BelowAbsoluteGate,
    BelowMeasurementFloor,
    InsufficientGatedWindows,
    UnsupportedTruePeakRate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioSignalValue {
    Measured {
        value: f64,
    },
    Unavailable {
        reason: AudioSignalUnavailableReason,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSignalMeasurement {
    pub unit: AudioSignalUnit,
    pub scope: AudioScope,
    pub value: AudioSignalValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioChannelSignal {
    pub channel: u16,
    /// Absolute signed PCM16 amplitude divided by 32768; distinct from true peak.
    pub sample_peak_ratio: f64,
    pub rms_ratio: f64,
    pub sample_peak: AudioSignalMeasurement,
    pub rms: AudioSignalMeasurement,
    pub crest_factor: AudioSignalMeasurement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSignalMethod {
    pub short_term_window_frames: u64,
    pub short_term_hop_frames: u64,
    pub integrated_minimum_frames: u64,
    pub loudness_range_minimum_frames: u64,
    pub true_peak_padding_frames: u64,
    pub loudness_range_minimum_gated_windows: u32,
    pub true_peak_max_sample_rate_hz: u32,
    pub true_peak_target_sample_rate_hz: u32,
    pub tool_summary_decimal_places: u8,
    pub short_term_decimal_places: u8,
    pub short_term_floor_lufs: i16,
    pub true_peak_sample_peak_tolerance_millidecibels: u16,
}

impl AudioSignalMethod {
    #[must_use]
    pub const fn for_sample_rate(rate: u32) -> Self {
        let rate = rate as u64;
        Self {
            short_term_window_frames: rate * 3,
            short_term_hop_frames: rate / 10,
            integrated_minimum_frames: rate * 2 / 5,
            loudness_range_minimum_frames: rate * 60,
            true_peak_padding_frames: if rate <= 48000 { rate / 10 } else { 0 },
            loudness_range_minimum_gated_windows: 10,
            true_peak_max_sample_rate_hz: 48000,
            true_peak_target_sample_rate_hz: 192000,
            tool_summary_decimal_places: 1,
            short_term_decimal_places: 3,
            short_term_floor_lufs: -70,
            true_peak_sample_peak_tolerance_millidecibels: 200,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioSignalSeriesStatus {
    Measured {},
    Unavailable {
        reason: AudioSignalUnavailableReason,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioShortTermLoudness {
    pub range: AudioFrameRange,
    pub measurement: AudioSignalMeasurement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSignalRegion {
    pub channel: u16,
    pub range: AudioFrameRange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSignalMeasurements {
    pub schema: String,
    pub source: AudioSource,
    pub technical_artifact: AudioArtifactReference,
    pub inspection_analysis_artifact: AudioArtifactReference,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
    pub tools: Vec<AudioToolObservation>,
    pub settings: SignalAnalysisConfiguration,
    pub method: AudioSignalMethod,
    pub commands: Vec<AudioTechnicalCommandEvidence>,
    pub channels: Vec<AudioChannelSignal>,
    pub integrated_loudness: AudioSignalMeasurement,
    pub loudness_range: AudioSignalMeasurement,
    pub lra_threshold_lufs: f64,
    pub lra_qualifying_windows: u32,
    pub true_peak: AudioSignalMeasurement,
    pub short_term_status: AudioSignalSeriesStatus,
    pub short_term: Vec<AudioShortTermLoudness>,
    pub silence_regions: Vec<AudioSignalRegion>,
    pub clipping_regions: Vec<AudioSignalRegion>,
}

impl AudioSignalMeasurements {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(invalid("signal report exceeds 8 MiB"));
        }
        let report: Self = decode_json(bytes, "audio signal measurements")?;
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.settings.validate()?;
        let source = &self.source;
        if self.schema != AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V1
            || self.provider.id != AUDIO_SIGNAL_PROVIDER_ID
            || self.provider.version != AUDIO_SIGNAL_PROVIDER_VERSION
            || source.artifact.id != "source_audio"
            || !(44..=268_435_456).contains(&source.artifact.byte_size)
            || !(8000..=192000).contains(&source.sample_rate_hz)
            || source.sample_rate_hz % 10 != 0
            || !matches!(source.channels, 1 | 2)
            || source.stem.is_some()
            || source.frame_count > u64::from(source.sample_rate_hz) * 600
            || source.artifact.byte_size < 44 + source.frame_count * u64::from(source.channels) * 2
        {
            return Err(invalid(
                "signal evidence does not satisfy the supported PCM16 source profile",
            ));
        }
        for (reference, expected_id, maximum) in [
            (&self.technical_artifact, "technical", 1_048_576),
            (
                &self.inspection_analysis_artifact,
                "analysis",
                8 * 1024 * 1024,
            ),
        ] {
            if reference.id != expected_id
                || reference.byte_size == 0
                || reference.byte_size > maximum
            {
                return Err(invalid(
                    "signal report requires bounded upstream inspection artifact identities",
                ));
            }
            require_sha256(&reference.sha256, "inspection artifact sha256")?;
        }
        for digest in [
            &self.implementation_sha256,
            &self.configuration_sha256,
            &self.provider_lock_sha256,
        ] {
            require_sha256(digest, "signal identity sha256")?;
        }
        if self.tools.len() != 2 || self.tools[0].id != "ffmpeg" || self.tools[1].id != "ffprobe" {
            return Err(invalid(
                "signal report requires ordered ffmpeg and ffprobe identities",
            ));
        }
        for tool in &self.tools {
            if tool.version.len() > 256 {
                return Err(invalid("tool version exceeds 256 bytes"));
            }
            crate::provider::validate_semantic_version(&tool.version, "signal tool version")?;
            require_sha256(&tool.sha256, "signal tool sha256")?;
        }
        if self.method != AudioSignalMethod::for_sample_rate(source.sample_rate_hz)
            || self.commands != super::provider::command_evidence(source.sample_rate_hz)
        {
            return Err(invalid(
                "signal method or command evidence differs from the fixed profile",
            ));
        }
        self.validate_measurements()?;
        self.validate_regions(&self.silence_regions, true)?;
        self.validate_regions(&self.clipping_regions, false)
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        crate::provider::canonical_json_bytes(self)
    }

    fn validate_measurements(&self) -> Result<()> {
        if self.channels.len() != usize::from(self.source.channels) {
            return Err(invalid(
                "signal report requires exactly one record per source channel",
            ));
        }
        for (index, channel) in self.channels.iter().enumerate() {
            if usize::from(channel.channel) != index
                || !channel.sample_peak_ratio.is_finite()
                || !channel.rms_ratio.is_finite()
                || !(0.0..=1.0).contains(&channel.sample_peak_ratio)
                || !(0.0..=1.0).contains(&channel.rms_ratio)
                || channel.rms_ratio > channel.sample_peak_ratio + 1e-12
                || (channel.sample_peak_ratio == 0.0) != (channel.rms_ratio == 0.0)
            {
                return Err(invalid(
                    "channel amplitude ratios are nonfinite, unordered, or inconsistent",
                ));
            }
            let scope = AudioScope {
                channels: vec![channel.channel],
                stem_id: None,
            };
            validate_measurement(
                &channel.sample_peak,
                AudioSignalUnit::DecibelsFullScale,
                &scope,
            )?;
            validate_measurement(&channel.rms, AudioSignalUnit::DecibelsFullScale, &scope)?;
            validate_measurement(&channel.crest_factor, AudioSignalUnit::Decibels, &scope)?;
            validate_logarithm(&channel.sample_peak.value, channel.sample_peak_ratio)?;
            validate_logarithm(&channel.rms.value, channel.rms_ratio)?;
            if channel.rms_ratio == 0.0 {
                require_unavailable(
                    &channel.crest_factor.value,
                    AudioSignalUnavailableReason::SilentInput,
                )?;
            } else {
                require_near(
                    &channel.crest_factor.value,
                    20.0 * (channel.sample_peak_ratio / channel.rms_ratio).log10(),
                )?;
            }
        }
        let scope = AudioScope {
            channels: (0..self.source.channels).collect(),
            stem_id: None,
        };
        validate_measurement(
            &self.integrated_loudness,
            AudioSignalUnit::LoudnessUnitsFullScale,
            &scope,
        )?;
        validate_measurement(&self.loudness_range, AudioSignalUnit::LoudnessUnits, &scope)?;
        validate_measurement(&self.true_peak, AudioSignalUnit::DecibelsTruePeak, &scope)?;
        let silent = self
            .channels
            .iter()
            .all(|channel| channel.sample_peak_ratio == 0.0);
        validate_gated(
            &self.integrated_loudness.value,
            silent,
            self.source.frame_count < self.method.integrated_minimum_frames,
        )?;
        if !self.lra_threshold_lufs.is_finite()
            || !(-400.0..=24.0).contains(&self.lra_threshold_lufs)
        {
            return Err(invalid("LRA threshold must be a finite bounded LUFS value"));
        }
        let qualifying = self.short_term.iter().filter(|entry| matches!(entry.measurement.value, AudioSignalValue::Measured { value } if value >= (-69.999_f64).max(self.lra_threshold_lufs + 0.1))).count();
        if self.lra_qualifying_windows as usize != qualifying {
            return Err(invalid(
                "LRA qualifying window count disagrees with the declared threshold and series",
            ));
        }
        if silent {
            require_unavailable(
                &self.loudness_range.value,
                AudioSignalUnavailableReason::SilentInput,
            )?;
        } else if self.source.frame_count < self.method.loudness_range_minimum_frames {
            require_unavailable(
                &self.loudness_range.value,
                AudioSignalUnavailableReason::InsufficientDuration,
            )?;
        } else if self.lra_qualifying_windows < self.method.loudness_range_minimum_gated_windows {
            require_unavailable(
                &self.loudness_range.value,
                AudioSignalUnavailableReason::InsufficientGatedWindows,
            )?;
        } else if !matches!(self.loudness_range.value, AudioSignalValue::Measured { .. }) {
            return Err(invalid(
                "sufficient gated LRA windows require a finite measurement",
            ));
        }
        if self.source.sample_rate_hz > self.method.true_peak_max_sample_rate_hz {
            require_unavailable(
                &self.true_peak.value,
                AudioSignalUnavailableReason::UnsupportedTruePeakRate,
            )?;
        } else if silent {
            require_unavailable(
                &self.true_peak.value,
                AudioSignalUnavailableReason::SilentInput,
            )?;
        } else if !matches!(self.true_peak.value, AudioSignalValue::Measured { .. }) {
            return Err(invalid(
                "non-silent true peak must contain a finite measurement",
            ));
        }
        if let AudioSignalValue::Measured { value } = self.true_peak.value {
            let maximum_sample_peak = self
                .channels
                .iter()
                .map(|channel| channel.sample_peak_ratio)
                .fold(0.0_f64, f64::max);
            let tolerance =
                f64::from(self.method.true_peak_sample_peak_tolerance_millidecibels) / 1000.0;
            if maximum_sample_peak > 0.0 && value < 20.0 * maximum_sample_peak.log10() - tolerance {
                return Err(invalid(
                    "true peak materially underreads the observed native sample peak",
                ));
            }
        }
        if self.short_term.len() > 10_000 {
            return Err(invalid("short-term series exceeds 10000 windows"));
        }
        let window = self.method.short_term_window_frames;
        let hop = self.method.short_term_hop_frames;
        if self.source.frame_count < window {
            if !self.short_term.is_empty()
                || self.short_term_status
                    != (AudioSignalSeriesStatus::Unavailable {
                        reason: AudioSignalUnavailableReason::InsufficientDuration,
                    })
            {
                return Err(invalid(
                    "short input requires an explicit unavailable short-term series",
                ));
            }
        } else {
            let expected = (self.source.frame_count - window) / hop + 1;
            if self.short_term_status != (AudioSignalSeriesStatus::Measured {})
                || self.short_term.len() as u64 != expected
            {
                return Err(invalid(
                    "short-term series must cover every full 3-second window on its exact 100ms grid",
                ));
            }
            for (index, measurement) in self.short_term.iter().enumerate() {
                let start = index as u64 * hop;
                if measurement.range
                    != (AudioFrameRange {
                        start,
                        end: start + window,
                    })
                {
                    return Err(invalid(
                        "short-term window is not on the declared source sample grid",
                    ));
                }
                measurement.range.validate(&self.source)?;
                validate_measurement(
                    &measurement.measurement,
                    AudioSignalUnit::LoudnessUnitsFullScale,
                    &scope,
                )?;
                if silent {
                    require_unavailable(
                        &measurement.measurement.value,
                        AudioSignalUnavailableReason::SilentInput,
                    )?;
                } else if matches!(measurement.measurement.value, AudioSignalValue::Unavailable { reason } if !matches!(reason, AudioSignalUnavailableReason::SilentInput | AudioSignalUnavailableReason::BelowMeasurementFloor))
                {
                    return Err(invalid(
                        "complete short-term windows may be unavailable only for zero energy or the explicit measurement floor",
                    ));
                }
                if matches!(measurement.measurement.value, AudioSignalValue::Measured { value } if value < f64::from(self.method.short_term_floor_lufs))
                {
                    return Err(invalid(
                        "short-term values below the declared -70 LUFS profile floor must be unavailable",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_regions(&self, regions: &[AudioSignalRegion], silence: bool) -> Result<()> {
        if regions.len() > 10_000 {
            return Err(invalid("signal region collection exceeds 10000 entries"));
        }
        let minimum = (u64::from(self.settings.minimum_silence_milliseconds)
            * u64::from(self.source.sample_rate_hz))
        .div_ceil(1000);
        let mut previous: Option<&AudioSignalRegion> = None;
        for region in regions {
            region.range.validate(&self.source)?;
            if region.channel >= self.source.channels
                || (silence && region.range.end - region.range.start < minimum)
            {
                return Err(invalid(
                    "signal region has invalid channel scope or insufficient silence duration",
                ));
            }
            if let Some(prior) = previous {
                if (region.channel, region.range.start, region.range.end)
                    <= (prior.channel, prior.range.start, prior.range.end)
                    || (region.channel == prior.channel && region.range.start <= prior.range.end)
                {
                    return Err(invalid(
                        "signal regions must be sorted, nonoverlapping, and coalesced per channel",
                    ));
                }
            }
            previous = Some(region);
        }
        Ok(())
    }
}

fn validate_measurement(
    measurement: &AudioSignalMeasurement,
    unit: AudioSignalUnit,
    scope: &AudioScope,
) -> Result<()> {
    if measurement.unit != unit || &measurement.scope != scope {
        return Err(invalid(
            "measurement unit or source channel scope is inconsistent",
        ));
    }
    if let AudioSignalValue::Measured { value } = measurement.value {
        let valid_range = match unit {
            AudioSignalUnit::DecibelsFullScale => (-400.0..=0.0).contains(&value),
            AudioSignalUnit::Decibels | AudioSignalUnit::LoudnessUnits => {
                (0.0..=400.0).contains(&value)
            }
            AudioSignalUnit::LoudnessUnitsFullScale | AudioSignalUnit::DecibelsTruePeak => {
                (-400.0..=24.0).contains(&value)
            }
        };
        if !value.is_finite() || !valid_range {
            return Err(invalid(
                "logarithmic measurement is nonfinite or outside the bounded PCM16 profile",
            ));
        }
    }
    Ok(())
}

fn validate_logarithm(value: &AudioSignalValue, ratio: f64) -> Result<()> {
    if ratio == 0.0 {
        require_unavailable(value, AudioSignalUnavailableReason::SilentInput)
    } else {
        require_near(value, 20.0 * ratio.log10())
    }
}

fn require_near(value: &AudioSignalValue, expected: f64) -> Result<()> {
    if let AudioSignalValue::Measured { value } = value {
        if value.is_finite() && (*value - expected).abs() <= 1e-8 {
            return Ok(());
        }
    }
    Err(invalid(
        "logarithmic measurement disagrees with its linear amplitude",
    ))
}

fn require_unavailable(
    value: &AudioSignalValue,
    expected: AudioSignalUnavailableReason,
) -> Result<()> {
    if matches!(value, AudioSignalValue::Unavailable { reason } if *reason == expected) {
        Ok(())
    } else {
        Err(invalid(
            "measurement must carry its explicit unavailable reason",
        ))
    }
}

fn validate_gated(value: &AudioSignalValue, silent: bool, too_short: bool) -> Result<()> {
    if silent {
        return require_unavailable(value, AudioSignalUnavailableReason::SilentInput);
    }
    if too_short {
        return require_unavailable(value, AudioSignalUnavailableReason::InsufficientDuration);
    }
    if matches!(value, AudioSignalValue::Measured { value } if *value <= -70.0) {
        return Err(invalid(
            "integrated values at or below the absolute gate must be unavailable",
        ));
    }
    if matches!(value, AudioSignalValue::Unavailable { reason } if *reason != AudioSignalUnavailableReason::BelowAbsoluteGate)
    {
        return Err(invalid(
            "supported gated measurement has an inconsistent unavailable reason",
        ));
    }
    Ok(())
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}
