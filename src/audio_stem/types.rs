//! Closed provider-neutral stem lineage and explicit selection contracts.
use std::collections::BTreeSet;
use std::path::{Component, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audio_analysis::{AudioArtifactReference, AudioFrameRange, AudioScope, AudioSource};
use crate::provider::{decode_json, require_sha256, validate_provider_reference};
use crate::{Error, ErrorCategory, ProviderReference, Result};

pub const AUDIO_STEM_CONFIGURATION_SCHEMA_V1: &str = "aniflow.audio-stem.configuration/v1";
pub const AUDIO_STEM_LINEAGE_SCHEMA_V1: &str = "aniflow.audio-stem-lineage/v1";
pub const AUDIO_STEM_PROVIDER_ID: &str = "org.egohygiene.aniflow.audio-stem";
pub const AUDIO_STEM_PROVIDER_VERSION: &str = "1.0.0";
pub const AUDIO_STEM_CAPABILITY_ID: &str = "aniflow/audio-stem-lineage";

/// Explicit retained separation run and artifact identity; no inferred stem role.
/// Omitted channel/range selections mean the entire independently inspected stem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStemSelection {
    pub run_directory: PathBuf,
    pub stage_id: String,
    pub stem_id: String,
    pub channels: Option<Vec<u16>>,
    pub range: Option<AudioFrameRange>,
    pub duration_tolerance_milliseconds: u16,
}

/// Short name retained for the importer and callers constructing selection requests.
pub type StemSelection = AudioStemSelection;

impl AudioStemSelection {
    #[must_use]
    pub fn new(
        run_directory: impl Into<PathBuf>,
        stage_id: impl Into<String>,
        stem_id: impl Into<String>,
    ) -> Self {
        Self {
            run_directory: run_directory.into(),
            stage_id: stage_id.into(),
            stem_id: stem_id.into(),
            channels: None,
            range: None,
            duration_tolerance_milliseconds: 20,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !self.run_directory.is_absolute()
            || self
                .run_directory
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
            || self.run_directory.to_str().is_none_or(|path| {
                path.len() > 4096
                    || path.chars().any(char::is_control)
                    || path.split('/').any(|part| matches!(part, "." | ".."))
            })
        {
            return Err(invalid(
                "stem run directory must be an absolute normalized UTF-8 path",
            ));
        }
        local_id(&self.stage_id)?;
        local_id(&self.stem_id)?;
        if self.duration_tolerance_milliseconds > 20 {
            return Err(invalid(
                "stem duration tolerance cannot exceed 20 milliseconds",
            ));
        }
        if let Some(channels) = &self.channels {
            if channels.is_empty()
                || channels.len() > 2
                || channels.iter().any(|channel| *channel > 1)
                || channels.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(invalid(
                    "stem channels must be an ordered nonempty bounded selection",
                ));
            }
        }
        if let Some(range) = self.range {
            if range.start >= range.end || range.end > 115_200_000 {
                return Err(invalid("stem selection range must be nonempty and bounded"));
            }
        }
        Ok(())
    }
}

/// The source and stem each use their own zero-origin clock. Only total duration
/// is compared: this does not claim sample alignment, phase or separation quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioStemTimingBasis {
    ZeroOriginDurationOnly,
}

/// Verified provider-neutral relationship facts, including raw retained authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioStemLineage {
    pub original_mix: AudioSource,
    pub selected_stem: AudioSource,
    pub stem_id: String,
    pub source_stage_id: String,
    pub source_mix_artifact_id: String,
    pub source_stem_artifact_id: String,
    pub source_stem_port: String,
    pub scope: AudioScope,
    pub range: AudioFrameRange,
    pub relationship_evidence: AudioArtifactReference,
    /// Raw byte identities, distinct from the canonical plan/checkpoint digests.
    pub authority_artifacts: Vec<AudioArtifactReference>,
    pub source_plan_sha256: String,
    pub source_checkpoint_sha256: String,
    pub source_provider: ProviderReference,
    pub source_provider_lock_sha256: String,
    pub source_implementation_sha256: String,
    pub source_configuration_sha256: String,
    pub duration_tolerance_milliseconds: u16,
    pub timing_basis: AudioStemTimingBasis,
}

impl AudioStemLineage {
    pub fn validate(&self) -> Result<()> {
        pcm_source(&self.original_mix)?;
        pcm_source(&self.selected_stem)?;
        for id in [
            &self.stem_id,
            &self.source_stage_id,
            &self.source_mix_artifact_id,
            &self.source_stem_artifact_id,
            &self.source_stem_port,
        ] {
            local_id(id)?;
        }
        if self.original_mix.artifact.id != "original_mix"
            || self.original_mix.stem.is_some()
            || self.selected_stem.artifact.id != "source_audio"
            || self.stem_id != self.source_stem_artifact_id
            || self.relationship_evidence.id != "separation_evidence"
            || self.duration_tolerance_milliseconds > 20
        {
            return Err(invalid(
                "stem lineage source identities or duration tolerance are inconsistent",
            ));
        }
        let stem_identity = self
            .selected_stem
            .stem
            .as_ref()
            .ok_or_else(|| invalid("selected source must declare its stem lineage"))?;
        if stem_identity.id != self.stem_id
            || stem_identity.original_mix != self.original_mix.artifact
            || stem_identity.relationship_evidence_id != "stem_lineage"
        {
            return Err(invalid(
                "selected stem must reference the neutral lineage companion and exact original mix",
            ));
        }
        if self.scope.channels != (0..self.selected_stem.channels).collect::<Vec<_>>()
            || self.scope.stem_id.as_deref() != Some(self.stem_id.as_str())
            || self.range
                != (AudioFrameRange {
                    start: 0,
                    end: self.selected_stem.frame_count,
                })
        {
            return Err(invalid(
                "v1 stem lineage supports only the full stem and every channel",
            ));
        }
        self.range.validate(&self.selected_stem)?;
        let left = u128::from(self.original_mix.frame_count)
            * u128::from(self.selected_stem.sample_rate_hz);
        let right = u128::from(self.selected_stem.frame_count)
            * u128::from(self.original_mix.sample_rate_hz);
        let difference = left.abs_diff(right);
        let rate_product = u128::from(self.original_mix.sample_rate_hz)
            * u128::from(self.selected_stem.sample_rate_hz);
        if difference * 1000 > u128::from(self.duration_tolerance_milliseconds) * rate_product {
            return Err(invalid(
                "stem and original mix durations differ beyond the selected exact tolerance",
            ));
        }
        artifact(&self.relationship_evidence, 8 * 1024 * 1024)?;
        if self.authority_artifacts.len() < 6 || self.authority_artifacts.len() > 16 {
            return Err(invalid(
                "stem lineage requires six to sixteen retained authority artifacts",
            ));
        }
        let mut ids = BTreeSet::new();
        for reference in &self.authority_artifacts {
            artifact(reference, 16 * 1024 * 1024)?;
            if reference
                .id
                .strip_prefix("stem_authority_")
                .is_none_or(str::is_empty)
                || !ids.insert(reference.id.as_str())
            {
                return Err(invalid(
                    "stem authority references must use unique stem_authority_ identifiers",
                ));
            }
        }
        for required in [
            "stem_authority_plan",
            "stem_authority_manifest",
            "stem_authority_checkpoint",
            "stem_authority_lock",
            "stem_authority_report",
        ] {
            if !ids.remove(required) {
                return Err(invalid(
                    "stem lineage omits a required retained authority artifact",
                ));
            }
        }
        if ids.is_empty() {
            return Err(invalid(
                "stem lineage requires retained artifact validation evidence",
            ));
        }
        for index in 0..ids.len() {
            if !ids.contains(format!("stem_authority_validation_{index}").as_str()) {
                return Err(invalid(
                    "retained validation authority identities must form a complete zero-based sequence",
                ));
            }
        }
        for digest in [
            &self.source_plan_sha256,
            &self.source_checkpoint_sha256,
            &self.source_provider_lock_sha256,
            &self.source_implementation_sha256,
            &self.source_configuration_sha256,
        ] {
            require_sha256(digest, "stem source authority sha256")?;
        }
        validate_provider_reference(&self.source_provider)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioStemProviderConfiguration {
    pub schema: String,
    pub lineage: AudioStemLineage,
    pub upstream_analysis_artifact_id: String,
}

impl AudioStemProviderConfiguration {
    pub fn validate(&self) -> Result<()> {
        if self.schema != AUDIO_STEM_CONFIGURATION_SCHEMA_V1
            || !matches!(
                self.upstream_analysis_artifact_id.as_str(),
                "analysis" | "signal_analysis"
            )
        {
            return Err(invalid(
                "unsupported stem provider configuration or upstream analysis identity",
            ));
        }
        self.lineage.validate()
    }

    pub fn provider_configuration(&self) -> Result<crate::ProviderConfiguration> {
        self.validate()?;
        let values = serde_json::to_value(self)
            .map_err(|_| invalid("cannot encode stem provider configuration"))?;
        let values = values
            .as_object()
            .expect("configuration struct is object")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        crate::ProviderConfiguration::new(
            ProviderReference {
                id: AUDIO_STEM_PROVIDER_ID.to_owned(),
                version: AUDIO_STEM_PROVIDER_VERSION.to_owned(),
            },
            crate::CapabilityReference {
                id: AUDIO_STEM_CAPABILITY_ID.to_owned(),
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
        id: AUDIO_STEM_CONFIGURATION_SCHEMA_V1.to_owned(),
        version: "1.0.0".to_owned(),
        sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../providers/audio-stem/configuration.schema.json"
            ))
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioStemLineageReport {
    pub schema: String,
    pub lineage: AudioStemLineage,
    pub upstream_analysis_artifact: AudioArtifactReference,
    pub provider: ProviderReference,
    pub implementation_sha256: String,
    pub configuration_sha256: String,
    pub provider_lock_sha256: String,
}

impl AudioStemLineageReport {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(invalid("stem lineage report exceeds 8 MiB"));
        }
        let value: Self = decode_json(bytes, "audio stem lineage")?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        self.lineage.validate()?;
        if self.schema != AUDIO_STEM_LINEAGE_SCHEMA_V1
            || self.provider.id != AUDIO_STEM_PROVIDER_ID
            || self.provider.version != AUDIO_STEM_PROVIDER_VERSION
            || !matches!(
                self.upstream_analysis_artifact.id.as_str(),
                "analysis" | "signal_analysis"
            )
        {
            return Err(invalid(
                "stem lineage report has inconsistent native provider or upstream identity",
            ));
        }
        artifact(&self.upstream_analysis_artifact, 8 * 1024 * 1024)?;
        let configuration = AudioStemProviderConfiguration {
            schema: AUDIO_STEM_CONFIGURATION_SCHEMA_V1.to_owned(),
            lineage: self.lineage.clone(),
            upstream_analysis_artifact_id: self.upstream_analysis_artifact.id.clone(),
        }
        .provider_configuration()?;
        if self.configuration_sha256 != configuration.effective_configuration_sha256 {
            return Err(invalid(
                "lineage report configuration digest does not bind its relationship facts",
            ));
        }
        for digest in [
            &self.implementation_sha256,
            &self.configuration_sha256,
            &self.provider_lock_sha256,
        ] {
            require_sha256(digest, "stem lineage provider sha256")?;
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        crate::provider::canonical_json_bytes(self)
    }
}

fn pcm_source(source: &AudioSource) -> Result<()> {
    source.validate()?;
    if !(8000..=192000).contains(&source.sample_rate_hz)
        || !matches!(source.channels, 1 | 2)
        || source.frame_count > u64::from(source.sample_rate_hz) * 600
        || !(44..=268_435_456).contains(&source.artifact.byte_size)
        || source.artifact.byte_size < 44 + source.frame_count * u64::from(source.channels) * 2
    {
        return Err(invalid(
            "stem lineage supports only bounded PCM16 audio clocks",
        ));
    }
    Ok(())
}

fn artifact(reference: &AudioArtifactReference, maximum: u64) -> Result<()> {
    local_id(&reference.id)?;
    require_sha256(&reference.sha256, "stem artifact sha256")?;
    if reference.byte_size == 0 || reference.byte_size > maximum {
        return Err(invalid("stem evidence artifact size is outside its bound"));
    }
    Ok(())
}

fn local_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(invalid(
            "stem identities must be bounded lowercase ASCII identifiers",
        ));
    }
    Ok(())
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}
