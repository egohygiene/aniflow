//! Closed provider-invocation contract for Pipeline v3 execution.
//!
//! The request is operational data: local paths are intentionally present so
//! an explicitly registered provider can read immutable inputs and write only
//! its assigned outputs. Durable compatibility evidence must retain semantic
//! identities rather than hashing this path-bearing document.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorCategory, Result};
use crate::provider::{ArtifactRole, ProviderConfiguration, StreamRole, invalid, require_sha256};
use crate::provider_runtime::ArtifactKind;

/// Closed JSON request understood by Pipeline v3 provider executables.
pub const PROVIDER_INVOCATION_SCHEMA_V1: &str = "aniflow.provider-invocation/v1";
/// Exact execution semantics used by the v1 provider-invocation ABI.
pub const PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1: &str =
    "aniflow.provider-invocation/direct-argv/v1";
/// Built-in validation contract proving immutable artifact integrity.
pub const ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1: &str =
    "aniflow.validation/artifact-integrity/v1";
/// Fixed direct-argv option followed by the absolute invocation request path.
pub const PROVIDER_INVOCATION_ARGUMENT: &str = "--aniflow-invocation";

/// One typed local artifact bound to a declared provider port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderInvocationArtifactBinding {
    pub port: String,
    pub artifact_id: String,
    pub artifact_type: String,
    pub artifact_role: ArtifactRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_role: Option<StreamRole>,
    pub kind: ArtifactKind,
    pub path: PathBuf,
}

impl ProviderInvocationArtifactBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        port: impl Into<String>,
        artifact_id: impl Into<String>,
        artifact_type: impl Into<String>,
        artifact_role: ArtifactRole,
        stream_role: Option<StreamRole>,
        kind: ArtifactKind,
        path: impl Into<PathBuf>,
    ) -> Result<Self> {
        let binding = Self {
            port: port.into(),
            artifact_id: artifact_id.into(),
            artifact_type: artifact_type.into(),
            artifact_role,
            stream_role,
            kind,
            path: path.into(),
        };
        binding.validate("artifact binding")?;
        Ok(binding)
    }

    fn validate(&self, label: &str) -> Result<()> {
        validate_local_id(&self.port, &format!("{label} port"))?;
        validate_local_id(&self.artifact_id, &format!("{label} artifact_id"))?;
        validate_artifact_type(&self.artifact_type, label)?;
        validate_absolute_path(&self.path, &format!("{label} path"))
    }
}

/// Complete, closed request supplied to one exact provider implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderInvocationRequest {
    pub schema: String,
    pub execution_semantics: String,
    pub stage_id: String,
    pub provider_lock_sha256: String,
    pub configuration: ProviderConfiguration,
    pub inputs: Vec<ProviderInvocationArtifactBinding>,
    pub outputs: Vec<ProviderInvocationArtifactBinding>,
}

impl ProviderInvocationRequest {
    pub fn new(
        stage_id: impl Into<String>,
        provider_lock_sha256: impl Into<String>,
        configuration: ProviderConfiguration,
        inputs: Vec<ProviderInvocationArtifactBinding>,
        outputs: Vec<ProviderInvocationArtifactBinding>,
    ) -> Result<Self> {
        let request = Self {
            schema: PROVIDER_INVOCATION_SCHEMA_V1.to_owned(),
            execution_semantics: PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1.to_owned(),
            stage_id: stage_id.into(),
            provider_lock_sha256: provider_lock_sha256.into(),
            configuration,
            inputs,
            outputs,
        };
        request.validate()?;
        Ok(request)
    }

    /// Decode a normalized closed request and recheck every invariant.
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let request: Self = serde_json::from_slice(input).map_err(|error| {
            invalid(format!(
                "invalid Pipeline v3 provider invocation JSON: {error}"
            ))
        })?;
        let value: serde_json::Value = serde_json::from_slice(input).map_err(|error| {
            invalid(format!(
                "invalid Pipeline v3 provider invocation JSON: {error}"
            ))
        })?;
        let normalized = serde_json::to_value(&request).map_err(|error| {
            Error::new(
                ErrorCategory::Internal,
                format!("failed to normalize Pipeline v3 provider invocation: {error}"),
            )
        })?;
        if value != normalized {
            return Err(invalid(
                "Pipeline v3 provider invocation JSON must use the normalized contract representation",
            ));
        }
        request.validate()?;
        Ok(request)
    }

    /// Validate the schema, execution ABI, configuration, and path bindings.
    pub fn validate(&self) -> Result<()> {
        if self.schema != PROVIDER_INVOCATION_SCHEMA_V1 {
            return Err(invalid(format!(
                "unsupported Pipeline v3 provider invocation schema; expected {PROVIDER_INVOCATION_SCHEMA_V1}"
            )));
        }
        if self.execution_semantics != PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1 {
            return Err(invalid(format!(
                "unsupported Pipeline v3 provider invocation execution semantics; expected {PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1}"
            )));
        }
        validate_local_id(&self.stage_id, "provider invocation stage_id")?;
        require_sha256(&self.provider_lock_sha256, "provider_lock_sha256")?;
        self.configuration.validate()?;
        if self.inputs.is_empty() {
            return Err(invalid(
                "Pipeline v3 provider invocation requires at least one input binding",
            ));
        }

        let mut input_keys = BTreeSet::new();
        let mut input_paths = Vec::new();
        for (index, binding) in self.inputs.iter().enumerate() {
            binding.validate(&format!("inputs[{index}]"))?;
            if !input_keys.insert((binding.port.as_str(), binding.artifact_id.as_str())) {
                return Err(invalid(format!(
                    "input artifact {} is bound more than once to port {}",
                    binding.artifact_id, binding.port
                )));
            }
            input_paths.push(binding.path.as_path());
        }

        let mut output_keys = BTreeSet::new();
        let mut output_ids = BTreeSet::new();
        let mut output_paths: Vec<&Path> = Vec::new();
        for (index, binding) in self.outputs.iter().enumerate() {
            binding.validate(&format!("outputs[{index}]"))?;
            if !output_keys.insert((binding.port.as_str(), binding.artifact_id.as_str())) {
                return Err(invalid(format!(
                    "output artifact {} is bound more than once to port {}",
                    binding.artifact_id, binding.port
                )));
            }
            if !output_ids.insert(binding.artifact_id.as_str()) {
                return Err(invalid(format!(
                    "output artifact {} is bound more than once",
                    binding.artifact_id
                )));
            }
            if output_paths
                .iter()
                .any(|other| paths_overlap(other, &binding.path))
            {
                return Err(invalid("provider invocation output paths must not overlap"));
            }
            if input_paths
                .iter()
                .any(|input| paths_overlap(input, &binding.path))
            {
                return Err(invalid(
                    "provider invocation output paths must not overlap immutable inputs",
                ));
            }
            output_paths.push(binding.path.as_path());
        }
        Ok(())
    }

    /// Encode the normalized request as compact UTF-8 JSON.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| {
            Error::new(
                ErrorCategory::Internal,
                format!("failed to encode Pipeline v3 provider invocation: {error}"),
            )
        })
    }
}

/// Build the only argv shape allowed by provider-invocation v1.
pub fn provider_invocation_arguments(request_path: impl AsRef<Path>) -> Result<Vec<OsString>> {
    let request_path = request_path.as_ref();
    validate_absolute_path(request_path, "provider invocation request path")?;
    Ok(vec![
        OsString::from(PROVIDER_INVOCATION_ARGUMENT),
        request_path.as_os_str().to_owned(),
    ])
}

fn validate_local_id(value: &str, field: &str) -> Result<()> {
    let valid = value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{field} must start with a lowercase letter and contain only lowercase letters, numbers, hyphens, or underscores"
        )))
    }
}

fn validate_artifact_type(value: &str, field: &str) -> Result<()> {
    if !value.is_empty()
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{field} artifact_type must be a non-empty printable token"
        )))
    }
}

fn validate_absolute_path(path: &Path, field: &str) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid(format!(
            "{field} must be an absolute normalized path"
        )));
    }
    let Some(value) = path.to_str() else {
        return Err(invalid(format!("{field} must be valid UTF-8")));
    };
    if value.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{field} must not contain control characters"
        )));
    }
    Ok(())
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    let folded_components = |path: &Path| {
        path.components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .map(|value| value.to_lowercase())
            })
            .collect::<Option<Vec<_>>>()
    };
    let (Some(left), Some(right)) = (folded_components(left), folded_components(right)) else {
        return left == right || left.starts_with(right) || right.starts_with(left);
    };
    left == right || left.starts_with(&right) || right.starts_with(&left)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::*;
    use crate::provider::{CapabilityReference, ConfigurationSchemaReference, ProviderReference};

    #[test]
    fn invocation_round_trips_and_builds_the_fixed_argv() {
        let root = std::env::temp_dir().join("aniflow-invocation-contract");
        let request = ProviderInvocationRequest::new(
            "enhance",
            "a".repeat(64),
            configuration(),
            vec![binding(
                "frames",
                "source",
                ArtifactKind::Directory,
                root.join("source"),
            )],
            vec![binding(
                "processed_frames",
                "enhanced",
                ArtifactKind::Directory,
                root.join("output"),
            )],
        )
        .expect("invocation should validate");

        let encoded = request.to_json_bytes().expect("request should encode");
        assert_eq!(
            ProviderInvocationRequest::from_json_slice(&encoded).expect("request should decode"),
            request
        );
        let request_path = root.join("request.json");
        assert_eq!(
            provider_invocation_arguments(&request_path).expect("argv should validate"),
            vec![
                OsString::from(PROVIDER_INVOCATION_ARGUMENT),
                request_path.into_os_string(),
            ]
        );
    }

    #[test]
    fn invocation_rejects_nulls_relative_paths_and_overlapping_outputs() {
        let root = std::env::temp_dir().join("aniflow-invocation-invalid");
        let request = ProviderInvocationRequest::new(
            "enhance",
            "b".repeat(64),
            configuration(),
            vec![binding(
                "frames",
                "source",
                ArtifactKind::Directory,
                root.join("source"),
            )],
            vec![binding(
                "processed_frames",
                "enhanced",
                ArtifactKind::Directory,
                root.join("output"),
            )],
        )
        .expect("baseline should validate");

        let mut value = serde_json::to_value(&request).expect("request should serialize");
        value["inputs"][0]["stream_role"] = Value::Null;
        let encoded = serde_json::to_vec(&value).expect("request should encode");
        assert!(ProviderInvocationRequest::from_json_slice(&encoded).is_err());

        let mut relative = request.clone();
        relative.inputs[0].path = PathBuf::from("relative/source");
        assert!(relative.validate().is_err());

        let mut case_folded_overlap = request.clone();
        #[cfg(windows)]
        {
            case_folded_overlap.inputs[0].path = PathBuf::from(r"C:\Data\source");
            case_folded_overlap.outputs[0].path = PathBuf::from(r"c:\data\source\output");
        }
        #[cfg(not(windows))]
        {
            case_folded_overlap.inputs[0].path = PathBuf::from("/tmp/Data/source");
            case_folded_overlap.outputs[0].path = PathBuf::from("/tmp/data/source/output");
        }
        assert!(case_folded_overlap.validate().is_err());

        let mut overlapping = request;
        overlapping.outputs.push(binding(
            "preview",
            "preview",
            ArtifactKind::File,
            root.join("output/preview.png"),
        ));
        assert!(overlapping.validate().is_err());
    }

    fn binding(
        port: &str,
        artifact_id: &str,
        kind: ArtifactKind,
        path: PathBuf,
    ) -> ProviderInvocationArtifactBinding {
        ProviderInvocationArtifactBinding::new(
            port,
            artifact_id,
            "application/octet-stream",
            ArtifactRole::Intermediate,
            Some(StreamRole::Video),
            kind,
            path,
        )
        .expect("binding should validate")
    }

    fn configuration() -> ProviderConfiguration {
        ProviderConfiguration::new(
            ProviderReference {
                id: "org.egohygiene.fixture".to_owned(),
                version: "1.0.0".to_owned(),
            },
            CapabilityReference {
                id: "aniflow/frame.process".to_owned(),
                version: "1.0.0".to_owned(),
            },
            ConfigurationSchemaReference {
                id: "aniflow.fixture/v1".to_owned(),
                version: "1.0.0".to_owned(),
                sha256: "c".repeat(64),
            },
            BTreeMap::new(),
        )
        .expect("configuration should validate")
    }
}
