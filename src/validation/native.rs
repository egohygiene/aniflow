//! Pinned, offline FFprobe/FFmpeg observations for the admitted CFR profile.
//! Invoked through the bounded provider runtime, never by a completion parser.
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use super::*;
use crate::{ArtifactKind, ProviderInvocationRequest};
use crate::command::{ProcessLimits, run_bounded};

pub const NATIVE_VALIDATION_PROVIDER_ID: &str = "org.egohygiene.aniflow.validation";
pub const NATIVE_VALIDATION_CAPABILITY_ID: &str = "aniflow/temporal.validate";
pub const NATIVE_VALIDATION_CONFIGURATION_SCHEMA: &str = "aniflow.native-validation.configuration/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationToolPin {
    pub path: PathBuf,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeValidationConfiguration {
    pub ffmpeg: ValidationToolPin,
    pub ffprobe: ValidationToolPin,
    pub maximum_media_bytes: u64,
    pub decode_timeout_seconds: u64,
}

impl NativeValidationConfiguration {
    pub fn validate(&self) -> Result<()> {
        for pin in [&self.ffmpeg, &self.ffprobe] {
            require_sha256(&pin.sha256, "validator tool digest")?;
            if !pin.path.is_absolute() || pin.path.to_str().is_none() || pin.version.is_empty() || pin.version.chars().any(char::is_whitespace) {
                return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "validator tools need absolute UTF-8 paths, exact version tokens and SHA-256 pins"));
            }
        }
        if self.maximum_media_bytes == 0 || self.maximum_media_bytes > 1_099_511_627_776 || !(1..=3600).contains(&self.decode_timeout_seconds) {
            return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "media and timeout bounds must be explicit (at most 1 TiB and 3600 seconds)"));
        }
        Ok(())
    }
}

/// Build an explicit native registration without launching a process. Tool
/// bytes are pinned here and rechecked by the provider before and after use.
pub fn registration(registration_id: &str, executable: impl Into<PathBuf>, settings: NativeValidationConfiguration, artifact_type: &str, source_type: &str) -> Result<crate::ProviderRegistration> {
    use crate::{ProviderConfiguration, ProviderManifest, ProviderReference, CapabilityReference, ConfigurationSchemaReference, ComponentInventory, ComponentIdentity};
    settings.validate()?;
    for pin in [&settings.ffmpeg, &settings.ffprobe] { tool(pin)?; }
    let schema = ConfigurationSchemaReference {
        id: NATIVE_VALIDATION_CONFIGURATION_SCHEMA.to_owned(), version: "1.0.0".to_owned(),
        sha256: format!("{:x}", Sha256::digest(include_bytes!("../../docs/contracts/native-validation-configuration-v1.schema.json"))),
    };
    let port = |name: &str, artifact_type: &str, role: &str| {
        let mut port = serde_json::json!({"name":name,"artifact_type":artifact_type,"artifact_role":role,"cardinality":"one","immutable":true});
        if matches!(name,"artifact"|"source") { port["stream_role"] = serde_json::json!("video"); }
        port
    };
    // Banner tokens remain exact in configuration; component locks normalize
    // numeric major/minor releases to semantic versions without changing pins.
    let tools = [(&settings.ffmpeg, "ffmpeg"), (&settings.ffprobe, "ffprobe")].into_iter().map(|(pin,id)| {
        Ok(serde_json::json!({"id":id,"version_requirement":format!("={}", component_version(&pin.version)?),"sha256":pin.sha256}))
    }).collect::<Result<Vec<_>>>()?;
    let manifest: ProviderManifest = serde_json::from_value(serde_json::json!({
        "schema":"aniflow.provider-manifest/v1",
        "provider":{"id":NATIVE_VALIDATION_PROVIDER_ID,"version":"1.0.0","display_name":"aniflow exact temporal validator"},
        "configuration_schemas":[schema.clone()],
        "capabilities":[{
            "id":NATIVE_VALIDATION_CAPABILITY_ID,"version":"1.0.0","kind":"temporal_validator","configuration_schema":schema.clone(),
            "inputs":[port("artifact",artifact_type,"candidate_master"),port("context",VALIDATION_CONTEXT_SCHEMA_V1,"run_evidence"),port("source",source_type,"temporal_source")],
            "outputs":[port("report",VALIDATOR_OBSERVATION_SCHEMA_V1,"validation_evidence")],
            "batching":"whole_artifact","max_concurrency":1,
            "requirements":{"tools":tools,"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":0,"minimum_storage_mib":0,"gpu":"forbidden","network":"forbidden"}},
            "behavior":{"determinism":"deterministic","fidelity":"observational","cacheable":true,"content_changes":false,"side_effects":["filesystem_read","filesystem_write","subprocess"]},
            "lifecycle":{"progress":"none","cancellation":"process_signal"}
        }],
        "provenance":{"required":["input_digests","output_digests","pipeline_configuration","effective_configuration","provider_identity","capability_identity","tool_versions","codec_versions","model_identities","validation_evidence"]}
    })).map_err(|e| Error::new(ErrorCategory::Configuration,e.to_string()))?;
    let values = serde_json::from_value(serde_json::to_value(&settings).map_err(|e| Error::new(ErrorCategory::Configuration,e.to_string()))?).map_err(|e| Error::new(ErrorCategory::Configuration,e.to_string()))?;
    let configuration = ProviderConfiguration::new(ProviderReference { id:NATIVE_VALIDATION_PROVIDER_ID.to_owned(),version:"1.0.0".to_owned() }, CapabilityReference { id:NATIVE_VALIDATION_CAPABILITY_ID.to_owned(),version:"1.0.0".to_owned() }, schema, values)?;
    let tools = [(&settings.ffmpeg,"ffmpeg"),(&settings.ffprobe,"ffprobe")].into_iter().map(|(pin,id)| Ok(ComponentIdentity { id:id.to_owned(),version:component_version(&pin.version)?,sha256:Some(pin.sha256.clone()) })).collect::<Result<Vec<_>>>()?;
    crate::ProviderRegistration::new(registration_id,manifest,configuration,executable,"aniflow.native-temporal-validator/v1",ComponentInventory { tools,codecs:Vec::new(),models:Vec::new() })
}

fn component_version(token: &str) -> Result<String> {
    let mut version = token.to_owned();
    if token.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') {
        match token.split('.').count() { 1 => version.push_str(".0.0"), 2 => version.push_str(".0"), _ => {} }
    }
    semver::Version::parse(&version).map(|version| version.to_string()).map_err(|_| refusal(ValidationFailureCode::UnsupportedProfile,"native","tool banner is not a supported numeric or semantic version token"))
}

fn tool(pin: &ValidationToolPin) -> Result<&str> {
    let metadata = fs::symlink_metadata(&pin.path).map_err(|e| Error::new(ErrorCategory::Dependency, e.to_string()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || crate::temporal::sha256_file(&pin.path).map_err(|e| Error::from_anyhow(ErrorCategory::Dependency, e))? != pin.sha256 {
        return Err(refusal(ValidationFailureCode::StaleEvidence, "native", "validator tool bytes no longer match their exact pin"));
    }
    pin.path.to_str().ok_or_else(|| refusal(ValidationFailureCode::UnsupportedProfile, "native", "tool path is not UTF-8"))
}

fn verify_version(pin: &ValidationToolPin, name: &str) -> Result<()> {
    let result = run_bounded(tool(pin)?, ["-version"], ProcessLimits { timeout: Duration::from_secs(10), maximum_output_bytes: 65536 }, None)
        .map_err(|e| Error::from_anyhow(ErrorCategory::Dependency, e))?;
    let text = String::from_utf8_lossy(&result.stdout);
    let mut tokens = text.lines().next().unwrap_or_default().split_whitespace();
    if !result.status.success() || result.stdout_truncated || result.stderr_truncated || tokens.next() != Some(name) || tokens.next() != Some("version") || tokens.next() != Some(pin.version.as_str()) {
        return Err(refusal(ValidationFailureCode::IncompatibleEvidence, "native", "validator tool version differs from its declared pin"));
    }
    Ok(())
}

fn bounded_file(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|e| Error::new(ErrorCategory::Input, e.to_string()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > maximum { return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "expected a bounded regular nonsymlink file")); }
    let mut bytes = Vec::new();
    fs::File::open(path).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?.take(maximum + 1).read_to_end(&mut bytes).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    if bytes.len() as u64 > maximum { return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "file exceeds observation limit")); }
    Ok(bytes)
}

fn verify_media(path: &Path, evidence: &ArtifactEvidence, maximum: u64) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|e| Error::new(ErrorCategory::Input, e.to_string()))?;
    if evidence.kind != ArtifactKind::File || !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > maximum || metadata.len() != evidence.byte_count
        || crate::temporal::sha256_file(path).map_err(|e| Error::from_anyhow(ErrorCategory::Input, e))? != evidence.sha256 {
        return Err(refusal(ValidationFailureCode::ArtifactChanged, &evidence.id, "media bytes do not match the validation context"));
    }
    Ok(())
}

/// Native provider entry point. The outer runtime owns cancellation and process
/// descendants; this implementation only writes its new assigned report.
pub fn execute_invocation(request: &ProviderInvocationRequest) -> Result<()> {
    request.validate()?;
    if !cfg!(unix) { return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "native validation currently requires Unix provider process groups")); }
    if request.configuration.provider.id != NATIVE_VALIDATION_PROVIDER_ID || request.configuration.provider.version != "1.0.0"
        || request.configuration.capability.id != NATIVE_VALIDATION_CAPABILITY_ID || request.configuration.capability.version != "1.0.0"
        || request.configuration.configuration_schema.id != NATIVE_VALIDATION_CONFIGURATION_SCHEMA || request.configuration.configuration_schema.version != "1.0.0" {
        return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "native validator requires its exact versioned provider/capability/configuration"));
    }
    let settings: NativeValidationConfiguration = serde_json::from_value(serde_json::to_value(&request.configuration.values).map_err(|e| Error::new(ErrorCategory::Configuration, e.to_string()))?)
        .map_err(|e| Error::new(ErrorCategory::Configuration, e.to_string()))?;
    settings.validate()?;
    let schema_digest = format!("{:x}", Sha256::digest(include_bytes!("../../docs/contracts/native-validation-configuration-v1.schema.json")));
    if request.configuration.configuration_schema.sha256 != schema_digest {
        return Err(refusal(ValidationFailureCode::IncompatibleEvidence, "native", "native validator configuration schema digest differs from the shipped schema"));
    }
    if request.inputs.len() != 3 || request.outputs.len() != 1 { return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "temporal validator needs artifact, context and source inputs and one report")); }
    let binding = |name: &str| request.inputs.iter().find(|b| b.port == name).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, "native", format!("missing {name} binding")));
    let context_binding = binding("context")?;
    let artifact = binding("artifact")?;
    let source = binding("source")?;
    let context: ValidationContext = serde_json::from_slice(&bounded_file(&context_binding.path, 1_048_576)?).map_err(|e| Error::new(ErrorCategory::Configuration, e.to_string()))?;
    context.validate()?;
    let policy = context.temporal.as_ref().ok_or_else(|| refusal(ValidationFailureCode::UnsupportedProfile, "native", "native validator requires the temporal-media profile"))?;
    let source_evidence = context.inputs.iter().find(|a| a.id == policy.source_artifact).ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, "native", "missing source evidence"))?;
    if context_binding.artifact_type != VALIDATION_CONTEXT_SCHEMA_V1 || context_binding.kind != ArtifactKind::File || artifact.kind != ArtifactKind::File || source.kind != ArtifactKind::File
        || context.validator_lock_sha256 != request.provider_lock_sha256 || artifact.artifact_id != context.artifact.id || source.artifact_id != source_evidence.id {
        return Err(refusal(ValidationFailureCode::StaleEvidence, "native", "invocation does not match context authority"));
    }
    let output = &request.outputs[0];
    if output.port != "report" || output.artifact_type != VALIDATOR_OBSERVATION_SCHEMA_V1 || output.kind != ArtifactKind::File {
        return Err(refusal(ValidationFailureCode::UnsupportedProfile, "native", "native validator output must be its exact report port"));
    }
    verify_version(&settings.ffprobe, "ffprobe")?;
    verify_version(&settings.ffmpeg, "ffmpeg")?;
    verify_media(&source.path, source_evidence, settings.maximum_media_bytes)?;
    verify_media(&artifact.path, &context.artifact, settings.maximum_media_bytes)?;
    let source_inspection = crate::temporal::inspect_with_program(&source.path, &policy.source_selection, tool(&settings.ffprobe)?)
        .map_err(|e| Error::from_anyhow(ErrorCategory::Media, e))?;
    let artifact_inspection = crate::temporal::inspect_with_program(&artifact.path, &policy.artifact_selection, tool(&settings.ffprobe)?)
        .map_err(|e| Error::from_anyhow(ErrorCategory::Media, e))?;
    let snapshot = tempfile::Builder::new().prefix("aniflow-validation-decode-").tempfile().map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    fs::copy(&artifact.path, snapshot.path()).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    verify_media(snapshot.path(), &context.artifact, settings.maximum_media_bytes)?;
    let mut arguments: Vec<OsString> = ["-v", "error", "-nostdin", "-xerror", "-err_detect", "explode", "-threads", "1", "-protocol_whitelist", "file,pipe", "-i"].into_iter().map(OsString::from).collect();
    arguments.push(snapshot.path().as_os_str().to_owned());
    let selected = artifact_inspection.selected.as_ref().ok_or_else(|| refusal(ValidationFailureCode::MissingEvidence, "native", "selected streams are unavailable"))?;
    for index in selected.video.into_iter().chain(selected.audio) { arguments.extend([OsString::from("-map"), OsString::from(format!("0:{index}"))]); }
    arguments.extend(["-f", "null", "-"].into_iter().map(OsString::from));
    let decoded = run_bounded(tool(&settings.ffmpeg)?, arguments, ProcessLimits { timeout: Duration::from_secs(settings.decode_timeout_seconds), maximum_output_bytes: 65536 }, None).map_err(|e| Error::from_anyhow(ErrorCategory::Media, e))?;
    let disposition = if decoded.status.success() && !decoded.stdout_truncated && !decoded.stderr_truncated && decoded.stderr.is_empty() { ValidationDisposition::Passed } else { ValidationDisposition::Failed };
    let observation = ValidatorObservation {
        schema: VALIDATOR_OBSERVATION_SCHEMA_V1.to_owned(), context_sha256: context.sha256()?, artifact_sha256: context.artifact.sha256.clone(), disposition,
        checks: [ValidationCriterion::ArtifactIdentity, ValidationCriterion::SourceLineage, ValidationCriterion::Provenance, ValidationCriterion::Decodability].into_iter().map(|criterion| ValidationCheck { criterion, disposition }).collect(),
        temporal: Some(TemporalValidationObservation { source: source_inspection, artifact: artifact_inspection }),
    };
    verify_media(&source.path, source_evidence, settings.maximum_media_bytes)?;
    verify_media(&artifact.path, &context.artifact, settings.maximum_media_bytes)?;
    tool(&settings.ffprobe)?; tool(&settings.ffmpeg)?;
    let parent = output.path.parent().ok_or_else(|| Error::new(ErrorCategory::Configuration, "report has no parent"))?;
    fs::create_dir_all(parent).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(&output.path).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    let bytes = crate::provider::canonical_json_bytes(&observation)?;
    file.write_all(&bytes).and_then(|()| file.sync_all()).map_err(|e| Error::new(ErrorCategory::Io, e.to_string()))?;
    Ok(())
}
