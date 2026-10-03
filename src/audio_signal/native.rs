//! Versioned source-format-aware signal evidence. This profile measures original
//! samples without quantization and does not claim qualified loudness or true peak.
use serde::{Deserialize, Serialize};

use super::types::*;
use crate::audio_analysis::{AudioArtifactReference, AudioScope, AudioSource};
use crate::audio_inspection::{
    AudioInspectionConfiguration, AudioTechnicalCommandEvidence, AudioToolObservation,
    NativeSampleFormat,
};
use crate::provider::{decode_json, require_sha256};
use crate::{ProviderReference, Result};

pub const AUDIO_SIGNAL_CONFIGURATION_SCHEMA_V2: &str = "aniflow.audio-signal.configuration/v2";
pub const AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V2: &str =
    "aniflow.audio-signal.provider-configuration/v2";
pub const AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3: &str = "aniflow.audio-signal-measurements/v3";
pub const AUDIO_SIGNAL_NATIVE_PROVIDER_VERSION: &str = "3.0.0";
pub const AUDIO_SIGNAL_NATIVE_CAPABILITY_VERSION: &str = "3.0.0";

fn default_silence_ratio() -> f64 { 32.0 / 32768.0 }
fn default_clipping_ratio() -> f64 { 1.0 }
fn default_minimum_silence() -> u32 { 100 }

/// Explicit native amplitude ratios. Integer samples use the signed full-scale
/// denominator; float32 samples use 1.0 while retaining finite overs unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSignalAnalysisConfiguration {
    pub schema: String,
    #[serde(default = "default_silence_ratio")]
    pub silence_threshold_ratio: f64,
    #[serde(default = "default_minimum_silence")]
    pub minimum_silence_milliseconds: u32,
    #[serde(default = "default_clipping_ratio")]
    pub clipping_threshold_ratio: f64,
}

impl Default for NativeSignalAnalysisConfiguration {
    fn default() -> Self {
        Self {
            schema: AUDIO_SIGNAL_CONFIGURATION_SCHEMA_V2.to_owned(),
            silence_threshold_ratio: default_silence_ratio(),
            minimum_silence_milliseconds: default_minimum_silence(),
            clipping_threshold_ratio: default_clipping_ratio(),
        }
    }
}

impl NativeSignalAnalysisConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65_536 { return Err(invalid("signal configuration exceeds 64 KiB")); }
        let settings: Self = decode_json(bytes, "native audio signal configuration")?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_SIGNAL_CONFIGURATION_SCHEMA_V2
            || !self.silence_threshold_ratio.is_finite()
            || !(0.0..=1.0).contains(&self.silence_threshold_ratio)
            || !self.clipping_threshold_ratio.is_finite()
            || !(0.0..=1.0).contains(&self.clipping_threshold_ratio)
            || self.clipping_threshold_ratio == 0.0
            || !(1..=600_000).contains(&self.minimum_silence_milliseconds)
        {
            return Err(invalid("unsupported native signal configuration or full-scale ratio thresholds"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAudioSignalProviderConfiguration {
    pub schema: String,
    pub settings: NativeSignalAnalysisConfiguration,
    pub tools: AudioInspectionConfiguration,
    pub source: AudioArtifactReference,
}

impl NativeAudioSignalProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        self.tools.validate()?;
        if self.schema != AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V2
            || self.source.id != "source_audio"
            || !(44..=268_435_456).contains(&self.source.byte_size)
        { return Err(invalid("invalid native source-bound signal configuration")); }
        require_sha256(&self.source.sha256, "native signal source sha256")
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self)
            .map_err(|_| invalid("cannot encode native signal configuration"))?;
        let values = values.as_object().expect("configuration is an object")
            .iter().map(|(key, value)| (key.clone(), value.clone())).collect();
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_SIGNAL_PROVIDER_ID.to_owned(),
                version: AUDIO_SIGNAL_NATIVE_PROVIDER_VERSION.to_owned(),
            },
            crate::CapabilityReference {
                id: AUDIO_SIGNAL_CAPABILITY_ID.to_owned(),
                version: AUDIO_SIGNAL_NATIVE_CAPABILITY_VERSION.to_owned(),
            },
            native_configuration_schema_reference(), values,
        )
    }
}

#[must_use]
pub fn native_configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    use sha2::{Digest, Sha256};
    crate::ConfigurationSchemaReference {
        id: AUDIO_SIGNAL_PROVIDER_CONFIGURATION_SCHEMA_V2.to_owned(),
        version: "2.0.0".to_owned(),
        sha256: format!("{:x}", Sha256::digest(include_bytes!(
            "../../providers/audio-signal/provider-configuration-native.schema.json"
        ))),
    }
}

/// Fixed numeric method identity; no decoder, level correction, or integer
/// intermediate is used to compute these source-sample quantities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAudioSignalMethod {
    pub algorithm: String,
    pub full_scale_reference: f64,
    pub accumulation: String,
}

impl NativeAudioSignalMethod {
    #[must_use]
    pub fn for_sample_format(format: NativeSampleFormat) -> Self {
        Self {
            algorithm: "aniflow.native-sample-statistics/v1".to_owned(),
            full_scale_reference: match format {
                NativeSampleFormat::Pcm16 => 32768.0,
                NativeSampleFormat::Pcm24 => 8388608.0,
                NativeSampleFormat::Float32 => 1.0,
            },
            accumulation: "compensated_f64_sum_of_squares".to_owned(),
        }
    }
}

/// Native companion v3. Unsupported meter quantities remain explicit, while
/// finite float32 peaks above full scale remain visible as positive dBFS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAudioSignalMeasurements {
    pub schema: String,
    pub source: AudioSource,
    pub source_format: NativeSampleFormat,
    pub technical_artifact: AudioArtifactReference,
    pub inspection_analysis_artifact: AudioArtifactReference,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
    pub tools: Vec<AudioToolObservation>,
    pub settings: NativeSignalAnalysisConfiguration,
    pub method: NativeAudioSignalMethod,
    pub commands: Vec<AudioTechnicalCommandEvidence>,
    pub channels: Vec<AudioChannelSignal>,
    pub integrated_loudness: AudioSignalMeasurement,
    pub loudness_range: AudioSignalMeasurement,
    pub true_peak: AudioSignalMeasurement,
    pub short_term_status: AudioSignalSeriesStatus,
    pub silence_regions: Vec<AudioSignalRegion>,
    pub clipping_regions: Vec<AudioSignalRegion>,
}

impl NativeAudioSignalMeasurements {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 8 * 1024 * 1024 { return Err(invalid("native signal report exceeds 8 MiB")); }
        let report: Self = decode_json(bytes, "native audio signal measurements")?;
        report.validate()?;
        Ok(report)
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        crate::provider::canonical_json_bytes(self)
    }

    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.settings.validate()?;
        let source = &self.source;
        let payload_bytes = source.frame_count.checked_mul(u64::from(source.channels))
            .and_then(|count| count.checked_mul(u64::from(self.source_format.bytes_per_sample())))
            .ok_or_else(|| invalid("native signal source payload size overflow"))?;
        let minimum_source_bytes = payload_bytes.checked_add(payload_bytes % 2)
            .and_then(|count| count.checked_add(44))
            .ok_or_else(|| invalid("native signal source envelope size overflow"))?;
        if self.schema != AUDIO_SIGNAL_MEASUREMENTS_SCHEMA_V3
            || self.provider.id != AUDIO_SIGNAL_PROVIDER_ID
            || self.provider.version != AUDIO_SIGNAL_NATIVE_PROVIDER_VERSION
            || source.artifact.id != "source_audio"
            || !(44..=268_435_456).contains(&source.artifact.byte_size)
            || !(8000..=192000).contains(&source.sample_rate_hz)
            || !matches!(source.channels, 1 | 2)
            || source.stem.is_some()
            || source.frame_count > u64::from(source.sample_rate_hz) * 600
            || source.artifact.byte_size < minimum_source_bytes
        { return Err(invalid("native signal evidence does not satisfy its source-format profile")); }
        for (reference, id, maximum) in [
            (&self.technical_artifact, "technical", 1_048_576),
            (&self.inspection_analysis_artifact, "analysis", 8 * 1024 * 1024),
        ] {
            if reference.id != id || reference.byte_size == 0 || reference.byte_size > maximum {
                return Err(invalid("native signal report requires bounded upstream evidence"));
            }
            require_sha256(&reference.sha256, "native inspection artifact sha256")?;
        }
        for digest in [&self.implementation_sha256, &self.configuration_sha256, &self.provider_lock_sha256] {
            require_sha256(digest, "native signal identity sha256")?;
        }
        if self.tools.len() != 2 || self.tools[0].id != "ffmpeg" || self.tools[1].id != "ffprobe" {
            return Err(invalid("native signal evidence requires upstream tool identities"));
        }
        for tool in &self.tools {
            if tool.version.len() > 256 { return Err(invalid("tool version exceeds 256 bytes")); }
            crate::provider::validate_semantic_version(&tool.version, "native signal tool version")?;
            require_sha256(&tool.sha256, "native signal tool sha256")?;
        }
        if self.method != NativeAudioSignalMethod::for_sample_format(self.source_format)
            || !self.commands.is_empty()
        { return Err(invalid("native signal method cannot claim an external meter invocation")); }
        if self.channels.len() != usize::from(source.channels) {
            return Err(invalid("native signal report requires one record per source channel"));
        }
        let maximum = if self.source_format == NativeSampleFormat::Float32 { f64::from(f32::MAX) } else { 1.0 };
        for (index, channel) in self.channels.iter().enumerate() {
            if usize::from(channel.channel) != index
                || !channel.sample_peak_ratio.is_finite() || !channel.rms_ratio.is_finite()
                || !(0.0..=maximum).contains(&channel.sample_peak_ratio)
                || !(0.0..=maximum).contains(&channel.rms_ratio)
                || channel.rms_ratio > channel.sample_peak_ratio * (1.0 + 1e-12)
                || (channel.sample_peak_ratio == 0.0) != (channel.rms_ratio == 0.0)
            { return Err(invalid("native channel ratios are nonfinite or inconsistent with their source format")); }
            let scope = AudioScope { channels: vec![channel.channel], stem_id: None };
            validate_native_scalar(&channel.sample_peak, AudioSignalUnit::DecibelsFullScale, &scope, channel.sample_peak_ratio)?;
            validate_native_scalar(&channel.rms, AudioSignalUnit::DecibelsFullScale, &scope, channel.rms_ratio)?;
            let crest_ratio = if channel.rms_ratio == 0.0 { 0.0 } else { channel.sample_peak_ratio / channel.rms_ratio };
            validate_native_scalar(&channel.crest_factor, AudioSignalUnit::Decibels, &scope, crest_ratio)?;
        }
        let scope = AudioScope { channels: (0..source.channels).collect(), stem_id: None };
        for (value, unit) in [
            (&self.integrated_loudness, AudioSignalUnit::LoudnessUnitsFullScale),
            (&self.loudness_range, AudioSignalUnit::LoudnessUnits),
            (&self.true_peak, AudioSignalUnit::DecibelsTruePeak),
        ] {
            if value.scope != scope || value.unit != unit || value.value != (AudioSignalValue::Unavailable {
                reason: AudioSignalUnavailableReason::UnsupportedNativeSignalProfile,
            }) { return Err(invalid("native signal meters require explicit unavailable evidence")); }
        }
        if self.short_term_status != (AudioSignalSeriesStatus::Unavailable {
            reason: AudioSignalUnavailableReason::UnsupportedNativeSignalProfile,
        }) { return Err(invalid("native short-term loudness has no qualified meter")); }
        self.validate_regions(&self.silence_regions, true)?;
        self.validate_regions(&self.clipping_regions, false)
    }

    fn validate_regions(&self, regions: &[AudioSignalRegion], silence: bool) -> Result<()> {
        if regions.len() > 10_000 { return Err(invalid("native signal regions exceed 10000 entries")); }
        let minimum = (u64::from(self.settings.minimum_silence_milliseconds)
            * u64::from(self.source.sample_rate_hz)).div_ceil(1000);
        let mut previous: Option<&AudioSignalRegion> = None;
        for region in regions {
            region.range.validate(&self.source)?;
            if region.channel >= self.source.channels || (silence && region.range.end - region.range.start < minimum) {
                return Err(invalid("native signal region has invalid scope or duration"));
            }
            if previous.is_some_and(|prior| (region.channel, region.range.start, region.range.end)
                <= (prior.channel, prior.range.start, prior.range.end)
                || (region.channel == prior.channel && region.range.start <= prior.range.end)) {
                return Err(invalid("native signal regions must be sorted and coalesced without overlaps"));
            }
            previous = Some(region);
        }
        Ok(())
    }
}

fn validate_native_scalar(value: &AudioSignalMeasurement, unit: AudioSignalUnit, scope: &AudioScope, ratio: f64) -> Result<()> {
    if value.unit != unit || &value.scope != scope { return Err(invalid("native scalar scope or unit mismatch")); }
    if ratio == 0.0 {
        if value.value == (AudioSignalValue::Unavailable { reason: AudioSignalUnavailableReason::SilentInput }) { return Ok(()); }
    } else if let AudioSignalValue::Measured { value } = value.value {
        let minimum = if unit == AudioSignalUnit::Decibels { 0.0 } else { -1200.0 };
        if value.is_finite() && (minimum..=1200.0).contains(&value)
            && (value - 20.0 * ratio.log10()).abs() <= 1e-8 { return Ok(()); }
    }
    Err(invalid("native scalar measurement disagrees with the original sample amplitude"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_report() -> NativeAudioSignalMeasurements {
        NativeAudioSignalMeasurements::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/audio-signal-measurements-v3-float32.example.json"
        )).expect("authored native float example should parse")
    }

    #[test]
    fn native_settings_require_explicit_ratio_units_and_closed_finite_thresholds() {
        let settings = NativeSignalAnalysisConfiguration::default();
        settings.validate().unwrap();
        assert_eq!(settings.silence_threshold_ratio, 32.0 / 32768.0);
        assert_eq!(settings.clipping_threshold_ratio, 1.0);
        let legacy = serde_json::to_vec(&SignalAnalysisConfiguration::default()).unwrap();
        assert!(NativeSignalAnalysisConfiguration::from_json_slice(&legacy).is_err());
        let native = serde_json::to_vec(&settings).unwrap();
        assert!(SignalAnalysisConfiguration::from_json_slice(&native).is_err());
        for bad in [f64::NAN, f64::INFINITY, -0.1, 1.01] {
            let mut altered = settings.clone();
            altered.silence_threshold_ratio = bad;
            assert!(altered.validate().is_err());
        }
        let mut zero_clip = settings;
        zero_clip.clipping_threshold_ratio = 0.0;
        assert!(zero_clip.validate().is_err());
    }

    #[test]
    fn native_report_preserves_float_overs_and_refuses_pcm_relabeling_or_fake_meter_evidence() {
        let report = float_report();
        assert_eq!(report.channels[0].sample_peak_ratio, 1.5);
        assert_eq!(report.channels[0].rms_ratio, 1.5);
        assert!(matches!(report.channels[0].sample_peak.value,
            AudioSignalValue::Measured { value } if value > 0.0));
        let mut relabeled = report.clone();
        relabeled.source_format = NativeSampleFormat::Pcm24;
        relabeled.method = NativeAudioSignalMethod::for_sample_format(NativeSampleFormat::Pcm24);
        assert!(relabeled.validate().is_err());
        let mut fabricated = report.clone();
        fabricated.true_peak.value = AudioSignalValue::Measured { value: 3.6 };
        assert!(fabricated.validate().is_err());
        let mut wrong_method = report.clone();
        wrong_method.method.full_scale_reference = 32768.0;
        assert!(wrong_method.validate().is_err());
        let bytes = report.canonical_json_bytes().unwrap();
        assert!(AudioSignalMeasurements::from_json_slice(&bytes).is_err());
    }

    #[test]
    fn native_report_rejects_nonfinite_ratios_and_checks_odd_pcm24_riff_padding() {
        let mut report = float_report();
        report.channels[0].sample_peak_ratio = f64::INFINITY;
        assert!(report.validate().is_err());
        let mut pcm = NativeAudioSignalMeasurements::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/audio-signal-measurements-v3-pcm24.example.json"
        )).unwrap();
        pcm.source.frame_count = 1;
        pcm.source.artifact.byte_size = 47;
        pcm.silence_regions.clear();
        assert!(pcm.validate().is_err());
        pcm.source.artifact.byte_size = 48;
        pcm.validate().expect("one packed PCM24 frame needs its RIFF pad byte");
    }
}
