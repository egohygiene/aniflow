//! Closed, bounded contracts for probabilistic audio-to-MIDI candidates.
//! Deserialization alone is unvalidated; use the parsers or `validate` methods.
use crate::audio_analysis::{
    AudioArtifactReference, AudioConfidence, AudioProvenanceClass, AudioRationalTime, AudioScope,
    AudioSource,
};
use crate::audio_inspection::{
    AudioInspectionConfiguration, AudioTechnicalCommandEvidence, AudioToolPin,
};
use crate::{Error, ErrorCategory, ProviderReference, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

pub const AUDIO_MIDI_CONFIGURATION_SCHEMA_V1: &str = "aniflow.audio-midi.configuration/v1";
pub const AUDIO_MIDI_PROVIDER_CONFIGURATION_SCHEMA_V1: &str =
    "aniflow.audio-midi.provider-configuration/v1";
pub const AUDIO_MIDI_REPORT_SCHEMA_V1: &str = "aniflow.audio-midi/v1";
pub const AUDIO_MIDI_OBSERVATION_SCHEMA_V1: &str = "aniflow.audio-midi-observation/v1";
pub const AUDIO_MIDI_PREFLIGHT_SCHEMA_V1: &str = "aniflow.audio-midi-preflight/v1";
pub const AUDIO_MIDI_PROBE_SCHEMA_V1: &str = "aniflow.audio-midi-probe/v1";
pub const AUDIO_MIDI_PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-midi";
pub const AUDIO_MIDI_PROVIDER_VERSION: &str = "1.0.0";
pub const AUDIO_MIDI_CAPABILITY_ID: &str = "aniflow/audio-midi-extraction";
pub const AUDIO_MIDI_BASIC_PITCH_VERSION: &str = "0.4.0";
pub const AUDIO_MIDI_BASIC_PITCH_REVISION: &str = "9991303bba609a3b93089d13ec80d1d495083596";
pub const AUDIO_MIDI_CONFIDENCE_REASON: &str = "Note activations and MIDI velocities are not calibrated confidence; inferred notes are unreviewed candidates.";
pub const AUDIO_MIDI_MAXIMUM_NOTES: usize = 8192;
pub const AUDIO_MIDI_MAXIMUM_RAW_BYTES: usize = 2 * 1024 * 1024;
pub const AUDIO_MIDI_MAXIMUM_REPORT_BYTES: usize = 8 * 1024 * 1024;
pub const AUDIO_MIDI_MAXIMUM_MODEL_BYTES: u64 = 230444;
pub const AUDIO_MIDI_MODEL_GIT_BLOB_SHA1: &str = "c30e5f9438e798604b7177aa26be1fe64482f767";
pub const AUDIO_MIDI_SAMPLE_RATE: u32 = 22050;
pub const AUDIO_MIDI_MAXIMUM_FRAMES: u64 = 2646000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiAdapterPin {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiRuntimePin {
    pub sha256: String,
    pub file_count: u64,
    pub byte_count: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiModelPin {
    pub path: PathBuf,
    pub sha256: String,
    pub byte_size: u64,
    pub model_id: String,
    pub revision: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiConfiguration {
    pub schema: String,
    pub python: AudioToolPin,
    pub adapter: MidiAdapterPin,
    pub runtime: MidiRuntimePin,
    pub model: MidiModelPin,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMidiProviderConfiguration {
    pub schema: String,
    pub settings: MidiConfiguration,
    pub tools: AudioInspectionConfiguration,
    pub source: AudioArtifactReference,
    pub upstream_analysis_artifact_id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMidiDiagnosticCode {
    MissingTool,
    InvalidTool,
    ToolDigestMismatch,
    ToolVersionMismatch,
    MissingModel,
    InvalidModel,
    ModelDigestMismatch,
    ModelSizeMismatch,
    InvalidAdapter,
    AdapterDigestMismatch,
    RuntimeMismatch,
    InvalidProbe,
    ToolTimeout,
    ToolOutputLimit,
    ToolFailed,
    Cancelled,
    UnsupportedPlatform,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMidiDiagnostic {
    pub code: AudioMidiDiagnosticCode,
    pub component: String,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMidiPreflight {
    pub schema: String,
    pub ready: bool,
    pub diagnostics: Vec<AudioMidiDiagnostic>,
    pub probe: Option<AudioMidiProbe>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMidiLicense {
    pub basic_pitch_expression: String,
    pub basic_pitch_metadata_sha256: String,
    pub onnxruntime_expression: String,
    pub onnxruntime_metadata_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMidiProbe {
    pub schema: String,
    pub python_version: String,
    pub basic_pitch_version: String,
    pub basic_pitch_revision: String,
    pub onnxruntime_version: String,
    pub numpy_version: String,
    pub librosa_version: String,
    pub scipy_version: String,
    pub pretty_midi_version: String,
    pub runtime_sha256: String,
    pub runtime_file_count: u64,
    pub runtime_byte_count: u64,
    pub license: AudioMidiLicense,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiModelEvidence {
    pub sha256: String,
    pub byte_size: u64,
    pub model_id: String,
    pub revision: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiSettingsEvidence {
    pub python_version: String,
    pub python_sha256: String,
    pub adapter_sha256: String,
    pub runtime: MidiRuntimePin,
    pub model: MidiModelEvidence,
    pub tool_timeout_milliseconds: u64,
    pub maximum_tool_output_bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiMethod {
    pub sample_rate_hz: u32,
    pub downmix: String,
    pub backend: String,
    pub gpu: bool,
    pub pitch_bends: bool,
    pub instrument: String,
    pub native_time_quantization: String,
    pub maximum_native_time_error_nanoseconds: u16,
    pub velocity_mapping: String,
    pub minimum_pitch: u8,
    pub maximum_pitch: u8,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiNativeNote {
    pub start_microseconds: u64,
    pub end_microseconds: u64,
    pub pitch: u8,
    pub velocity: u8,
    pub activation: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMidiNativeObservation {
    pub schema: String,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub sample_frames: u64,
    pub duration_microseconds: u64,
    pub notes: Vec<MidiNativeNote>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiNote {
    pub id: String,
    pub start: AudioRationalTime,
    pub end: AudioRationalTime,
    pub start_tick: u32,
    pub end_tick: u32,
    pub pitch: u8,
    pub velocity: u8,
    pub activation: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiObservation {
    pub native: AudioMidiNativeObservation,
    pub notes: Vec<MidiNote>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiUnavailableReason {
    UnsupportedSampleRate,
    UnsupportedChannels,
    DurationLimit,
    SilentInput,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum MidiResult {
    Candidate { observation: MidiObservation },
    Unavailable { reason: MidiUnavailableReason },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMidiReport {
    pub schema: String,
    pub source: AudioSource,
    pub scope: AudioScope,
    pub technical_artifact: AudioArtifactReference,
    pub upstream_analysis_artifact: AudioArtifactReference,
    pub raw_observation: Option<AudioArtifactReference>,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
    pub settings: MidiSettingsEvidence,
    pub probe: AudioMidiProbe,
    pub commands: Vec<AudioTechnicalCommandEvidence>,
    pub method: MidiMethod,
    pub provenance: AudioProvenanceClass,
    pub confidence: AudioConfidence,
    pub result: MidiResult,
}

impl MidiConfiguration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "MIDI configuration")?;
        let value: Self = crate::provider::decode_json(bytes, "MIDI configuration")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_MIDI_CONFIGURATION_SCHEMA_V1 {
            return Err(invalid("unsupported MIDI configuration schema"));
        }
        normalized_path(&self.python.executable)?;
        normalized_path(&self.adapter.path)?;
        normalized_path(&self.model.path)?;
        MidiSettingsEvidence::from_configuration(self).validate()
    }
}
impl AudioMidiProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        self.tools.validate()?;
        reference(&self.source, 256 * 1024 * 1024)?;
        if self.schema != AUDIO_MIDI_PROVIDER_CONFIGURATION_SCHEMA_V1
            || self.source.id != "source_audio"
            || self.source.byte_size < 44
            || !matches!(
                self.upstream_analysis_artifact_id.as_str(),
                "analysis" | "stem_analysis"
            )
        {
            return Err(invalid(
                "unsupported MIDI provider configuration or source binding",
            ));
        }
        Ok(())
    }
    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let value =
            serde_json::to_value(self).map_err(|_| invalid("cannot encode MIDI settings"))?;
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_MIDI_PROVIDER_ID.into(),
                version: AUDIO_MIDI_PROVIDER_VERSION.into(),
            },
            crate::CapabilityReference {
                id: AUDIO_MIDI_CAPABILITY_ID.into(),
                version: "1.0.0".into(),
            },
            configuration_schema_reference(),
            value
                .as_object()
                .expect("configuration struct")
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
}
#[must_use]
pub fn configuration_schema_reference() -> crate::ConfigurationSchemaReference {
    crate::ConfigurationSchemaReference {
        id: AUDIO_MIDI_PROVIDER_CONFIGURATION_SCHEMA_V1.into(),
        version: "1.0.0".into(),
        sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../providers/audio-midi/provider-configuration.schema.json"
            ))
        ),
    }
}
impl From<&MidiModelPin> for MidiModelEvidence {
    fn from(value: &MidiModelPin) -> Self {
        Self {
            sha256: value.sha256.clone(),
            byte_size: value.byte_size,
            model_id: value.model_id.clone(),
            revision: value.revision.clone(),
        }
    }
}
impl MidiSettingsEvidence {
    #[must_use]
    pub fn from_configuration(value: &MidiConfiguration) -> Self {
        Self {
            python_version: value.python.version.clone(),
            python_sha256: value.python.sha256.clone(),
            adapter_sha256: value.adapter.sha256.clone(),
            runtime: value.runtime.clone(),
            model: MidiModelEvidence::from(&value.model),
            tool_timeout_milliseconds: value.tool_timeout_milliseconds,
            maximum_tool_output_bytes: value.maximum_tool_output_bytes,
        }
    }
    pub fn validate(&self) -> Result<()> {
        python_version(&self.python_version)?;
        for hash in [
            &self.python_sha256,
            &self.adapter_sha256,
            &self.runtime.sha256,
            &self.model.sha256,
        ] {
            crate::provider::require_sha256(hash, "MIDI configured digest")?;
        }
        runtime_bounds(self.runtime.file_count, self.runtime.byte_count)?;
        if self.model.model_id != "basic-pitch-onnx-icassp-2022"
            || self.model.revision != AUDIO_MIDI_BASIC_PITCH_REVISION
            || self.model.byte_size != AUDIO_MIDI_MAXIMUM_MODEL_BYTES
        {
            return Err(invalid(
                "MIDI model must name the bounded pinned ICASSP 2022 ONNX profile",
            ));
        }
        if !(1..=120000).contains(&self.tool_timeout_milliseconds)
            || !(1024..=AUDIO_MIDI_MAXIMUM_RAW_BYTES as u64)
                .contains(&self.maximum_tool_output_bytes)
        {
            return Err(invalid(
                "MIDI tool execution bounds exceed the supported profile",
            ));
        }
        Ok(())
    }
}
impl Default for MidiMethod {
    fn default() -> Self {
        Self {
            sample_rate_hz: 22050,
            downmix: "none".into(),
            backend: "onnxruntime_cpu".into(),
            gpu: false,
            pitch_bends: false,
            instrument: "unavailable".into(),
            native_time_quantization: "nearest_microsecond_ties_up".into(),
            maximum_native_time_error_nanoseconds: 500,
            velocity_mapping: "round_ties_even_127_times_activation".into(),
            minimum_pitch: 21,
            maximum_pitch: 108,
        }
    }
}
impl AudioMidiProbe {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "MIDI probe")?;
        let value: Self = crate::provider::decode_json(bytes, "MIDI probe")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        python_version(&self.python_version)?;
        if self.schema != AUDIO_MIDI_PROBE_SCHEMA_V1
            || self.basic_pitch_version != AUDIO_MIDI_BASIC_PITCH_VERSION
            || self.basic_pitch_revision != AUDIO_MIDI_BASIC_PITCH_REVISION
            || self.onnxruntime_version != "1.20.1"
            || self.numpy_version != "1.26.4"
            || self.librosa_version != "0.10.2.post1"
            || self.scipy_version != "1.13.1"
            || self.pretty_midi_version != "0.2.10"
            || self.license.basic_pitch_expression != "Apache-2.0"
            || self.license.onnxruntime_expression != "MIT"
        {
            return Err(invalid(
                "MIDI probe differs from the pinned CPU dependency profile",
            ));
        }
        for hash in [
            &self.runtime_sha256,
            &self.license.basic_pitch_metadata_sha256,
            &self.license.onnxruntime_metadata_sha256,
        ] {
            crate::provider::require_sha256(hash, "MIDI probe digest")?;
        }
        runtime_bounds(self.runtime_file_count, self.runtime_byte_count)
    }
}
impl AudioMidiPreflight {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
            && self.diagnostics.is_empty()
            && self.probe.is_some()
            && self.schema == AUDIO_MIDI_PREFLIGHT_SCHEMA_V1
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_MIDI_PREFLIGHT_SCHEMA_V1
            || self.ready != self.diagnostics.is_empty()
            || self.ready != self.probe.is_some()
            || self.diagnostics.len() > 32
        {
            return Err(invalid("inconsistent MIDI preflight outcome"));
        }
        if let Some(probe) = &self.probe {
            probe.validate()?;
        }
        for diagnostic in &self.diagnostics {
            text(&diagnostic.component, 256)?;
            text(&diagnostic.message, 4096)?;
        }
        Ok(())
    }
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, 65536, "MIDI preflight")?;
        let value: Self = crate::provider::decode_json(bytes, "MIDI preflight")?;
        value.validate()?;
        Ok(value)
    }
}
impl MidiObservation {
    pub fn validate(&self, source: &AudioSource) -> Result<()> {
        if self != &super::normalize::from_native(self.native.clone(), source)? {
            return Err(invalid(
                "MIDI candidates contradict raw notes or deterministic timing and velocity mapping",
            ));
        }
        Ok(())
    }
}
impl AudioMidiReport {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        bounded(bytes, AUDIO_MIDI_MAXIMUM_REPORT_BYTES, "MIDI report")?;
        let value: Self = crate::provider::decode_json(bytes, "MIDI report")?;
        value.validate()?;
        Ok(value)
    }
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        bounded(&bytes, AUDIO_MIDI_MAXIMUM_REPORT_BYTES, "MIDI report")?;
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<()> {
        self.source.validate()?;
        self.settings.validate()?;
        self.probe.validate()?;
        let scope = AudioScope {
            channels: (0..self.source.channels).collect(),
            stem_id: self.source.stem.as_ref().map(|s| s.id.clone()),
        };
        if self.schema != AUDIO_MIDI_REPORT_SCHEMA_V1
            || self.source.artifact.id != "source_audio"
            || self.scope != scope
            || self.provider
                != (ProviderReference {
                    id: AUDIO_MIDI_PROVIDER_ID.into(),
                    version: AUDIO_MIDI_PROVIDER_VERSION.into(),
                })
            || self.method != MidiMethod::default()
            || self.provenance != AudioProvenanceClass::Probabilistic
            || self.confidence
                != (AudioConfidence::Unavailable {
                    reason: AUDIO_MIDI_CONFIDENCE_REASON.into(),
                })
        {
            return Err(invalid(
                "unsupported MIDI report identity, method, scope or uncertainty",
            ));
        }
        if !(8000..=192000).contains(&self.source.sample_rate_hz)
            || !(1..=2).contains(&self.source.channels)
            || self.source.frame_count > u64::from(self.source.sample_rate_hz) * 600
            || self.source.artifact.byte_size > 256 * 1024 * 1024
            || self.source.artifact.byte_size
                < 44 + self.source.frame_count * u64::from(self.source.channels) * 2
        {
            return Err(invalid(
                "MIDI source is outside the inspected PCM16 profile",
            ));
        }
        for hash in [
            &self.implementation_sha256,
            &self.configuration_sha256,
            &self.provider_lock_sha256,
        ] {
            crate::provider::require_sha256(hash, "MIDI report digest")?;
        }
        reference(&self.technical_artifact, 8 * 1024 * 1024)?;
        reference(&self.upstream_analysis_artifact, 8 * 1024 * 1024)?;
        if self.technical_artifact.id != "technical"
            || !matches!(
                self.upstream_analysis_artifact.id.as_str(),
                "analysis" | "stem_analysis"
            )
        {
            return Err(invalid("MIDI upstream identities differ from the pipeline"));
        }
        if self.settings.python_version != self.probe.python_version
            || self.settings.runtime.sha256 != self.probe.runtime_sha256
            || self.settings.runtime.file_count != self.probe.runtime_file_count
            || self.settings.runtime.byte_count != self.probe.runtime_byte_count
        {
            return Err(invalid(
                "MIDI probe differs from the configured runtime identity",
            ));
        }
        let observed = match &self.result {
            MidiResult::Candidate { observation } => {
                observation.validate(&self.source)?;
                let raw = self
                    .raw_observation
                    .as_ref()
                    .ok_or_else(|| invalid("MIDI candidates require raw observation identity"))?;
                reference(raw, AUDIO_MIDI_MAXIMUM_RAW_BYTES as u64)?;
                if raw.id != "midi_observation" {
                    return Err(invalid("unexpected MIDI raw observation identity"));
                }
                true
            }
            MidiResult::Unavailable { reason } => {
                let valid = match reason {
                    MidiUnavailableReason::UnsupportedSampleRate => {
                        self.source.sample_rate_hz != 22050
                    }
                    MidiUnavailableReason::UnsupportedChannels => {
                        self.source.sample_rate_hz == 22050 && self.source.channels != 1
                    }
                    MidiUnavailableReason::DurationLimit => {
                        self.source.sample_rate_hz == 22050
                            && self.source.channels == 1
                            && self.source.frame_count > AUDIO_MIDI_MAXIMUM_FRAMES
                    }
                    MidiUnavailableReason::SilentInput => {
                        self.source.sample_rate_hz == 22050
                            && self.source.channels == 1
                            && self.source.frame_count <= AUDIO_MIDI_MAXIMUM_FRAMES
                    }
                };
                if !valid || self.raw_observation.is_some() {
                    return Err(invalid(
                        "unavailable MIDI result has inconsistent reason or fabricated observation",
                    ));
                }
                false
            }
        };
        if self.commands != super::provider::command_evidence(observed) {
            return Err(invalid(
                "MIDI command evidence differs from its pinned invocation",
            ));
        }
        Ok(())
    }
}
fn python_version(value: &str) -> Result<()> {
    crate::provider::validate_semantic_version(value, "MIDI Python version")?;
    if value.len() > 256 || !(value.starts_with("3.10.") || value.starts_with("3.11.")) {
        return Err(invalid("MIDI native profile requires Python 3.10 or 3.11"));
    }
    Ok(())
}
fn runtime_bounds(files: u64, bytes: u64) -> Result<()> {
    if files == 0 || files > 50000 || bytes == 0 || bytes > 2 * 1024 * 1024 * 1024 {
        return Err(invalid("MIDI runtime inventory exceeds bounds"));
    }
    Ok(())
}
fn normalized_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|p| matches!(p, Component::CurDir | Component::ParentDir))
        || path.to_str().is_none_or(|s| {
            s.len() > 4096
                || s.chars().any(char::is_control)
                || s.split('/').any(|p| matches!(p, "." | ".."))
        })
    {
        return Err(invalid(
            "MIDI paths must be absolute normalized UTF-8 paths",
        ));
    }
    Ok(())
}
pub(super) fn reference(value: &AudioArtifactReference, maximum: u64) -> Result<()> {
    crate::provider::require_sha256(&value.sha256, "MIDI artifact digest")?;
    if value.id.is_empty()
        || value.id.len() > 256
        || value.byte_size == 0
        || value.byte_size > maximum
    {
        return Err(invalid("MIDI artifact bounds invalid"));
    }
    Ok(())
}
fn text(value: &str, maximum: usize) -> Result<()> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(invalid("MIDI evidence text exceeds bounds"));
    }
    Ok(())
}
pub(super) fn bounded(bytes: &[u8], maximum: usize, label: &str) -> Result<()> {
    if bytes.len() > maximum {
        return Err(invalid(format!("{label} exceeds its byte bound")));
    }
    Ok(())
}
pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> AudioMidiReport {
        AudioMidiReport::from_json_slice(include_bytes!(
            "../../docs/contracts/examples/audio-midi-v1.example.json"
        ))
        .expect("published synthetic MIDI report validates")
    }
    #[test]
    fn published_report_round_trips_and_preserves_probabilistic_authority() {
        let report = fixture();
        assert_eq!(
            AudioMidiReport::from_json_slice(&report.canonical_json_bytes().unwrap()).unwrap(),
            report
        );
    }
    #[test]
    fn report_refuses_confidence_instrument_and_provider_promotions() {
        let mut value = fixture();
        value.confidence = AudioConfidence::Calibrated { score: 0.99 };
        assert!(value.validate().is_err());
        let mut value = fixture();
        value.method.instrument = "piano".into();
        assert!(value.validate().is_err());
        let mut value = fixture();
        value.provenance = AudioProvenanceClass::Deterministic;
        assert!(value.validate().is_err());
        let mut value = fixture();
        value.settings.runtime.sha256 = "f".repeat(64);
        assert!(value.validate().is_err());
    }
    #[test]
    fn report_preserves_full_scope_and_raw_observation_identity() {
        let mut value = fixture();
        value.scope.channels.clear();
        assert!(value.validate().is_err());
        let mut value = fixture();
        value.raw_observation = None;
        assert!(value.validate().is_err());
        let mut value = fixture();
        value.settings.model.byte_size += 1;
        assert!(value.validate().is_err());
        let mut value = fixture();
        value.commands.clear();
        assert!(value.validate().is_err());
    }
    #[test]
    fn preflight_requires_consistent_readiness_and_probe() {
        let report = fixture();
        let mut value = AudioMidiPreflight {
            schema: AUDIO_MIDI_PREFLIGHT_SCHEMA_V1.into(),
            ready: true,
            diagnostics: vec![],
            probe: Some(report.probe),
        };
        value.validate().unwrap();
        value.probe = None;
        assert!(value.validate().is_err());
        value.ready = false;
        assert!(value.validate().is_err());
        value.diagnostics.push(AudioMidiDiagnostic {
            code: AudioMidiDiagnosticCode::InvalidProbe,
            component: "probe".into(),
            message: "Synthetic refusal".into(),
        });
        value.validate().unwrap();
    }
}
