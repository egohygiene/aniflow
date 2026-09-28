//! Closed contracts for bounded heuristic musical estimates.
use crate::audio_analysis::{
    AudioArtifactReference, AudioConfidence, AudioProvenanceClass, AudioScope, AudioSource,
};
use crate::audio_inspection::{
    AudioInspectionConfiguration, AudioTechnicalCommandEvidence, AudioToolPin,
};
use crate::provider::{decode_json, require_sha256};
use crate::{Error, ErrorCategory, ProviderReference, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

pub const AUDIO_MUSICAL_CONFIGURATION_SCHEMA_V1: &str = "aniflow.audio-musical.configuration/v1";
pub const AUDIO_MUSICAL_PROVIDER_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-musical.provider-configuration/v1";
pub const AUDIO_MUSICAL_ANALYSIS_SCHEMA_V1: &str = "aniflow.audio-musical-analysis/v1";
pub const AUDIO_MUSICAL_OBSERVATION_SCHEMA_V1: &str = "aniflow.audio-musical-observation/v1";
pub const AUDIO_MUSICAL_PROBE_SCHEMA_V1: &str = "aniflow.audio-musical-probe/v1";
pub const AUDIO_MUSICAL_PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-musical";
pub const AUDIO_MUSICAL_PROVIDER_VERSION: &str = "1.0.0";
pub const AUDIO_MUSICAL_CAPABILITY_ID: &str = "aniflow/audio-musical-structure";
pub const AUDIO_MUSICAL_CONFIDENCE_REASON: &str =
    "Native estimator scores are not calibrated probabilities.";
pub const AUDIO_MUSICAL_ESSENTIA_VERSION: &str = "2.1b6.dev1389";
pub const AUDIO_MUSICAL_NUMPY_VERSION: &str = "2.3.5";
pub const AUDIO_MUSICAL_PYYAML_VERSION: &str = "6.0.3";
pub const AUDIO_MUSICAL_SIX_VERSION: &str = "1.17.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalAdapterPin {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalRuntimePin {
    pub sha256: String,
    pub file_count: u64,
    pub byte_count: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusicalAnalysisConfiguration {
    pub schema: String,
    pub python: AudioToolPin,
    pub adapter: AudioMusicalAdapterPin,
    pub runtime: AudioMusicalRuntimePin,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalProviderConfiguration {
    pub schema: String,
    pub settings: MusicalAnalysisConfiguration,
    pub tools: AudioInspectionConfiguration,
    pub source: AudioArtifactReference,
    pub upstream_analysis_artifact_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalLicense {
    pub essentia_expression: String,
    pub essentia_metadata_sha256: String,
    pub numpy_metadata_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalProbe {
    pub schema: String,
    pub python_version: String,
    pub essentia_version: String,
    pub essentia_runtime_version: String,
    pub essentia_git_sha: String,
    pub numpy_version: String,
    pub pyyaml_version: String,
    pub six_version: String,
    pub runtime_sha256: String,
    pub runtime_file_count: u64,
    pub runtime_byte_count: u64,
    pub license: AudioMusicalLicense,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalBpm {
    pub value: f64,
    pub ticks_seconds: Vec<f64>,
    pub raw_confidence: f64,
    pub estimates: Vec<f64>,
    pub bpm_intervals: Vec<f64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMusicalKeyProfile {
    Krumhansl,
    Temperley,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalKey {
    pub profile: AudioMusicalKeyProfile,
    pub key: String,
    pub scale: String,
    pub raw_strength: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalObservation {
    pub schema: String,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub sample_frames: u64,
    pub duration_seconds: f64,
    pub downmix: String,
    pub bpm: AudioMusicalBpm,
    pub key_profiles: Vec<AudioMusicalKey>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalBeat {
    pub id: String,
    pub seconds: f64,
    pub source_frame: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMusicalUnavailableReason {
    SilentDownmix,
    InsufficientDuration,
    UnsupportedSampleRate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMusicalFamilyUnavailableReason {
    NoEstimate,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioMusicalFamilyStatus {
    Estimated {},
    Unavailable {
        reason: AudioMusicalFamilyUnavailableReason,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalTempoCandidate {
    pub id: String,
    pub method: String,
    pub value: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalKeyCandidate {
    pub id: String,
    pub profile: AudioMusicalKeyProfile,
    pub pitch_class: u8,
    pub mode: String,
    pub raw_strength: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioMusicalResult {
    Estimated {
        observation: AudioMusicalObservation,
        beats: Vec<AudioMusicalBeat>,
        key_disagreement: bool,
        tempo_status: AudioMusicalFamilyStatus,
        beats_status: AudioMusicalFamilyStatus,
        key_status: AudioMusicalFamilyStatus,
        tempo_candidates: Vec<AudioMusicalTempoCandidate>,
        key_candidates: Vec<AudioMusicalKeyCandidate>,
    },
    Unavailable {
        reason: AudioMusicalUnavailableReason,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMusicalUnsupportedFamily {
    Downbeats,
    Meter,
    TempoChanges,
    Chords,
    Onsets,
    RhythmDensity,
    Sections,
    Spectral,
    Timbre,
}
impl AudioMusicalUnsupportedFamily {
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![
            Self::Downbeats,
            Self::Meter,
            Self::TempoChanges,
            Self::Chords,
            Self::Onsets,
            Self::RhythmDensity,
            Self::Sections,
            Self::Spectral,
            Self::Timbre,
        ]
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalMethod {
    pub downmix: String,
    pub beat_quantization: String,
    pub sample_rate_hz: u32,
    pub minimum_frames: u64,
    pub rhythm_method: String,
    pub minimum_tempo_bpm: u16,
    pub maximum_tempo_bpm: u16,
    pub key_method: String,
}
impl Default for AudioMusicalMethod {
    fn default() -> Self {
        Self {
            downmix: "arithmetic_average".into(),
            beat_quantization: "nearest_source_frame_ties_up".into(),
            sample_rate_hz: 44100,
            minimum_frames: 352800,
            rhythm_method: "rhythm_extractor2013_multifeature".into(),
            minimum_tempo_bpm: 40,
            maximum_tempo_bpm: 208,
            key_method: "key_extractor_default_parameters".into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalAnalysis {
    pub schema: String,
    pub source: AudioSource,
    pub scope: AudioScope,
    pub technical_artifact: AudioArtifactReference,
    pub upstream_analysis_artifact: AudioArtifactReference,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
    pub probe: AudioMusicalProbe,
    pub settings: AudioMusicalSettingsEvidence,
    pub commands: Vec<AudioTechnicalCommandEvidence>,
    pub method: AudioMusicalMethod,
    pub provenance: AudioProvenanceClass,
    pub confidence: AudioConfidence,
    pub result: AudioMusicalResult,
    pub unsupported_families: Vec<AudioMusicalUnsupportedFamily>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMusicalSettingsEvidence {
    pub python_version: String,
    pub python_sha256: String,
    pub adapter_sha256: String,
    pub runtime: AudioMusicalRuntimePin,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
impl AudioMusicalSettingsEvidence {
    #[must_use]
    pub fn from_configuration(value: &MusicalAnalysisConfiguration) -> Self {
        Self {
            python_version: value.python.version.clone(),
            python_sha256: value.python.sha256.clone(),
            adapter_sha256: value.adapter.sha256.clone(),
            runtime: value.runtime.clone(),
            tool_timeout_milliseconds: value.tool_timeout_milliseconds,
            maximum_tool_output_bytes: value.maximum_tool_output_bytes,
        }
    }
}

impl MusicalAnalysisConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "musical configuration")?;
        let value: Self = decode_json(bytes, "musical configuration")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_MUSICAL_CONFIGURATION_SCHEMA_V1
            || !(1..=120000).contains(&self.tool_timeout_milliseconds)
            || !(1024..=1048576).contains(&self.maximum_tool_output_bytes)
        {
            return Err(invalid(
                "unsupported musical configuration schema or bounds",
            ));
        }
        normalized_path(&self.python.executable)?;
        normalized_path(&self.adapter.path)?;
        crate::provider::validate_semantic_version(&self.python.version, "Python version")?;
        for digest in [
            &self.python.sha256,
            &self.adapter.sha256,
            &self.runtime.sha256,
        ] {
            require_sha256(digest, "musical runtime sha256")?;
        }
        runtime_bounds(self.runtime.file_count, self.runtime.byte_count)
    }
}
impl AudioMusicalProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        self.tools.validate()?;
        artifact(&self.source, 268435456)?;
        if self.schema != AUDIO_MUSICAL_PROVIDER_CONFIGURATION_SCHEMA_V1
            || self.source.id != "source_audio"
            || self.source.byte_size < 44
            || !matches!(
                self.upstream_analysis_artifact_id.as_str(),
                "analysis" | "stem_analysis"
            )
        {
            return Err(invalid("invalid musical provider source or schema"));
        }
        Ok(())
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|_| invalid("cannot encode musical configuration"))?;
        let values = value
            .as_object()
            .expect("configuration struct")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_MUSICAL_PROVIDER_ID.into(),
                version: AUDIO_MUSICAL_PROVIDER_VERSION.into(),
            },
            crate::CapabilityReference {
                id: AUDIO_MUSICAL_CAPABILITY_ID.into(),
                version: "1.0.0".into(),
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
        id: AUDIO_MUSICAL_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),
        version: "1.0.0".into(),
        sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../providers/audio-musical/provider-configuration.schema.json"
            ))
        ),
    }
}
fn normalized_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
        || path.to_str().is_none_or(|value| {
            value.len() > 4096
                || value.chars().any(char::is_control)
                || value.split('/').any(|part| matches!(part, "." | ".."))
        })
    {
        return Err(invalid(
            "musical executable/adapter must be absolute normalized UTF-8 paths",
        ));
    }
    Ok(())
}
fn artifact(value: &AudioArtifactReference, maximum: u64) -> Result<()> {
    require_sha256(&value.sha256, "musical artifact sha256")?;
    if value.id.is_empty() || value.byte_size == 0 || value.byte_size > maximum {
        return Err(invalid("musical artifact bounds invalid"));
    }
    Ok(())
}
fn bounded(bytes: &[u8], maximum: usize, label: &str) -> Result<()> {
    if bytes.len() > maximum {
        return Err(invalid(format!("{label} exceeds byte bound")));
    }
    Ok(())
}
pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

impl AudioMusicalProbe {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "musical probe")?;
        let value: Self = decode_json(bytes, "musical probe")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_MUSICAL_PROBE_SCHEMA_V1
            || self.essentia_version != AUDIO_MUSICAL_ESSENTIA_VERSION
            || self.essentia_runtime_version != "2.1-beta6-dev"
            || self.essentia_git_sha != "v2.1_beta5-1389-g36ec3d92"
            || self.numpy_version != AUDIO_MUSICAL_NUMPY_VERSION
            || self.pyyaml_version != AUDIO_MUSICAL_PYYAML_VERSION
            || self.six_version != AUDIO_MUSICAL_SIX_VERSION
            || self.license.essentia_expression != "AGPL-3.0-only"
        {
            return Err(invalid(
                "unsupported musical runtime package or license identity",
            ));
        }
        crate::provider::validate_semantic_version(&self.python_version, "musical Python version")?;
        for text in [&self.essentia_runtime_version, &self.essentia_git_sha] {
            if text.is_empty()
                || text.len() > 256
                || text.chars().any(|c| c.is_control() || c.is_whitespace())
            {
                return Err(invalid("invalid Essentia build identity"));
            }
        }
        for digest in [
            &self.runtime_sha256,
            &self.license.essentia_metadata_sha256,
            &self.license.numpy_metadata_sha256,
        ] {
            require_sha256(digest, "musical probe sha256")?;
        }
        runtime_bounds(self.runtime_file_count, self.runtime_byte_count)
    }
}
impl AudioMusicalObservation {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 1048576, "musical observation")?;
        let value: Self = decode_json(bytes, "musical observation")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_MUSICAL_OBSERVATION_SCHEMA_V1
            || self.sample_rate_hz != 44100
            || !matches!(self.channels, 1 | 2)
            || !(352800..=26460000).contains(&self.sample_frames)
            || !self.duration_seconds.is_finite()
            || self.duration_seconds != self.sample_frames as f64 / 44100.0
            || self.downmix != "arithmetic_average"
        {
            return Err(invalid(
                "musical observation clock is outside the fixed 44.1 kHz profile",
            ));
        }
        if !self.bpm.value.is_finite()
            || !(0.0..=1000.0).contains(&self.bpm.value)
            || !self.bpm.raw_confidence.is_finite()
            || self.bpm.raw_confidence < 0.0
        {
            return Err(invalid("invalid native tempo or confidence metric"));
        }
        for values in [
            &self.bpm.ticks_seconds,
            &self.bpm.estimates,
            &self.bpm.bpm_intervals,
        ] {
            if values.len() > 8192 || values.iter().any(|v| !v.is_finite()) {
                return Err(invalid(
                    "musical candidate collection exceeds finite bounded profile",
                ));
            }
        }
        if self.bpm.estimates.iter().any(|v| *v <= 0.0 || *v > 1000.0)
            || self
                .bpm
                .bpm_intervals
                .iter()
                .any(|v| *v <= 0.0 || *v > 600.0)
        {
            return Err(invalid("invalid native tempo alternatives or intervals"));
        }
        let mut previous = None;
        for seconds in &self.bpm.ticks_seconds {
            let frame = quantize_beat(*seconds, self.sample_frames)?;
            if previous.is_some_and(|(time, oldframe)| time >= *seconds || oldframe >= frame) {
                return Err(invalid(
                    "beat seconds and quantized frames must be strictly increasing",
                ));
            }
            previous = Some((*seconds, frame));
        }
        if self.key_profiles.len() != 2
            || self.key_profiles[0].profile != AudioMusicalKeyProfile::Krumhansl
            || self.key_profiles[1].profile != AudioMusicalKeyProfile::Temperley
        {
            return Err(invalid(
                "both ordered independent key profiles must be retained",
            ));
        }
        for key in &self.key_profiles {
            if !key.raw_strength.is_finite() || !(-1.0..=1.0).contains(&key.raw_strength) {
                return Err(invalid(
                    "native key strength must be finite within correlation range",
                ));
            }
            let absent = key.key.is_empty() && key.scale.is_empty() && key.raw_strength == 0.0;
            if !absent
                && (pitch_class(&key.key).is_none()
                    || !matches!(key.scale.as_str(), "major" | "minor"))
            {
                return Err(invalid("native key must be a known pitch class and major/minor mode or explicit absence"));
            }
        }
        Ok(())
    }
}
impl AudioMusicalResult {
    pub fn from_observation(
        observation: AudioMusicalObservation,
        source: &AudioSource,
    ) -> Result<Self> {
        observation.validate()?;
        if source.sample_rate_hz != observation.sample_rate_hz
            || source.channels != observation.channels
            || source.frame_count != observation.sample_frames
        {
            return Err(invalid("musical observation differs from its source clock"));
        }
        let beats = observation
            .bpm
            .ticks_seconds
            .iter()
            .enumerate()
            .map(|(index, seconds)| {
                Ok(AudioMusicalBeat {
                    id: format!("beat_{index:06}"),
                    seconds: *seconds,
                    source_frame: quantize_beat(*seconds, source.frame_count)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut tempo_candidates = Vec::new();
        if observation.bpm.value > 0.0 {
            tempo_candidates.push(AudioMusicalTempoCandidate {
                id: "tempo_primary".into(),
                method: "rhythm_extractor2013_multifeature".into(),
                value: observation.bpm.value,
            });
        }
        for (index, value) in observation.bpm.estimates.iter().enumerate() {
            tempo_candidates.push(AudioMusicalTempoCandidate {
                id: format!("tempo_estimate_{index:06}"),
                method: "rhythm_extractor2013_estimate".into(),
                value: *value,
            });
        }
        let key_candidates = observation
            .key_profiles
            .iter()
            .filter(|key| key.raw_strength > 0.0)
            .map(|key| AudioMusicalKeyCandidate {
                id: match key.profile {
                    AudioMusicalKeyProfile::Krumhansl => "key_krumhansl",
                    AudioMusicalKeyProfile::Temperley => "key_temperley",
                }
                .into(),
                profile: key.profile,
                pitch_class: pitch_class(&key.key).expect("validated key"),
                mode: key.scale.clone(),
                raw_strength: key.raw_strength,
            })
            .collect::<Vec<_>>();
        let key_disagreement = key_candidates.len() == 2
            && (key_candidates[0].pitch_class != key_candidates[1].pitch_class
                || key_candidates[0].mode != key_candidates[1].mode);
        Ok(Self::Estimated {
            tempo_status: family_status(!tempo_candidates.is_empty()),
            beats_status: family_status(!beats.is_empty()),
            key_status: family_status(!key_candidates.is_empty()),
            observation,
            beats,
            key_disagreement,
            tempo_candidates,
            key_candidates,
        })
    }
}
fn family_status(available: bool) -> AudioMusicalFamilyStatus {
    if available {
        AudioMusicalFamilyStatus::Estimated {}
    } else {
        AudioMusicalFamilyStatus::Unavailable {
            reason: AudioMusicalFamilyUnavailableReason::NoEstimate,
        }
    }
}
fn quantize_beat(seconds: f64, frames: u64) -> Result<u64> {
    if !seconds.is_finite() || seconds < 0.0 || seconds >= frames as f64 / 44100.0 {
        return Err(invalid("beat time must be finite and inside the source"));
    }
    let frame = (seconds * 44100.0).round() as u64;
    if frame >= frames {
        return Err(invalid("quantized beat lies beyond the source"));
    }
    Ok(frame)
}
#[must_use]
pub fn pitch_class(key: &str) -> Option<u8> {
    match key {
        "C" => Some(0),
        "C#" | "Db" => Some(1),
        "D" => Some(2),
        "D#" | "Eb" => Some(3),
        "E" => Some(4),
        "F" => Some(5),
        "F#" | "Gb" => Some(6),
        "G" => Some(7),
        "G#" | "Ab" => Some(8),
        "A" => Some(9),
        "A#" | "Bb" => Some(10),
        "B" => Some(11),
        _ => None,
    }
}
fn runtime_bounds(files: u64, bytes: u64) -> Result<()> {
    if files == 0 || files > 25000 || bytes == 0 || bytes > 1073741824 {
        return Err(invalid("musical runtime inventory exceeds bounds"));
    }
    Ok(())
}
impl AudioMusicalAnalysis {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 8 * 1024 * 1024, "musical analysis")?;
        let value: Self = decode_json(bytes, "musical analysis")?;
        value.validate()?;
        Ok(value)
    }
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        crate::provider::canonical_json_bytes(self)
    }
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.probe.validate()?;
        if self.schema != AUDIO_MUSICAL_ANALYSIS_SCHEMA_V1
            || self.provider.id != AUDIO_MUSICAL_PROVIDER_ID
            || self.provider.version != AUDIO_MUSICAL_PROVIDER_VERSION
            || self.source.artifact.id != "source_audio"
            || !(8000..=192000).contains(&self.source.sample_rate_hz)
            || !matches!(self.source.channels, 1 | 2)
            || self.source.frame_count > u64::from(self.source.sample_rate_hz) * 600
            || self.source.artifact.byte_size > 268435456
            || self.source.artifact.byte_size
                < 44 + self.source.frame_count * u64::from(self.source.channels) * 2
        {
            return Err(invalid(
                "musical report does not match the native bounded source profile",
            ));
        }
        if self.scope.channels != (0..self.source.channels).collect::<Vec<_>>()
            || self.scope.stem_id != self.source.stem.as_ref().map(|stem| stem.id.clone())
            || self.technical_artifact.id != "technical"
            || !matches!(
                self.upstream_analysis_artifact.id.as_str(),
                "analysis" | "stem_analysis"
            )
        {
            return Err(invalid(
                "musical evidence must preserve the complete source and stem scope",
            ));
        }
        artifact(&self.technical_artifact, 1048576)?;
        artifact(&self.upstream_analysis_artifact, 8 * 1024 * 1024)?;
        for digest in [
            &self.implementation_sha256,
            &self.configuration_sha256,
            &self.provider_lock_sha256,
            &self.settings.python_sha256,
            &self.settings.adapter_sha256,
            &self.settings.runtime.sha256,
        ] {
            require_sha256(digest, "musical report identity sha256")?;
        }
        if self.settings.python_version != self.probe.python_version
            || self.settings.runtime.sha256 != self.probe.runtime_sha256
            || self.settings.runtime.file_count != self.probe.runtime_file_count
            || self.settings.runtime.byte_count != self.probe.runtime_byte_count
            || !(1..=120000).contains(&self.settings.tool_timeout_milliseconds)
            || !(1024..=1048576).contains(&self.settings.maximum_tool_output_bytes)
        {
            return Err(invalid(
                "musical report runtime probe must match configured identities and bounds",
            ));
        }
        if self.method != AudioMusicalMethod::default()
            || self.provenance != AudioProvenanceClass::Heuristic
            || self.confidence
                != (AudioConfidence::Unavailable {
                    reason: AUDIO_MUSICAL_CONFIDENCE_REASON.into(),
                })
            || self.unsupported_families != AudioMusicalUnsupportedFamily::all()
        {
            return Err(invalid("musical method, uncertainty or explicit unsupported families differ from this profile"));
        }
        let analyzed = matches!(self.result, AudioMusicalResult::Estimated { .. });
        let mut expected = vec![musical_command(false)];
        if analyzed {
            expected.push(musical_command(true));
        }
        expected.push(musical_command(false));
        if self.commands != expected {
            return Err(invalid(
                "musical command evidence differs from the exact executed profile",
            ));
        }
        match &self.result {
            AudioMusicalResult::Estimated { observation, .. } => {
                if self.result
                    != AudioMusicalResult::from_observation(observation.clone(), &self.source)?
                {
                    return Err(invalid("musical candidates, quantization, family outcomes or disagreement contradict raw observations"));
                }
            }
            AudioMusicalResult::Unavailable { reason } => match reason {
                AudioMusicalUnavailableReason::UnsupportedSampleRate
                    if self.source.sample_rate_hz == 44100 =>
                {
                    return Err(invalid(
                        "unsupported-rate reason contradicts supported source rate",
                    ))
                }
                AudioMusicalUnavailableReason::InsufficientDuration
                    if self.source.sample_rate_hz != 44100 || self.source.frame_count >= 352800 =>
                {
                    return Err(invalid(
                        "insufficient-duration reason contradicts source clock",
                    ))
                }
                AudioMusicalUnavailableReason::SilentDownmix
                    if self.source.sample_rate_hz != 44100 || self.source.frame_count < 352800 =>
                {
                    return Err(invalid(
                        "silent-downmix result requires supported duration and rate",
                    ))
                }
                _ => {}
            },
        }
        Ok(())
    }
}
#[must_use]
pub fn musical_command(analyze: bool) -> AudioTechnicalCommandEvidence {
    let mut arguments = vec!["-I".into(), "-B".into(), "{adapter}".into()];
    if analyze {
        arguments.extend(["--input".into(), "{snapshot}".into()]);
    } else {
        arguments.push("--probe".into());
    }
    AudioTechnicalCommandEvidence {
        tool: "python".into(),
        arguments,
    }
}
