//! Explicit one-shot local notes through the existing registered provider runtime.
//! This adapter declares loopback networking; it does not establish a network sandbox.
use super::{
    Advisory, Candidate, CandidateIdentity, Mode, Packet, Request, prepare, validate_candidate,
};
use crate::destination::{cancelled, invalid, local};
use crate::provider::{
    ProviderConfiguration, ProviderManifest, canonical_sha256, decode_json, require_sha256,
};
use crate::provider_runtime::{
    ArtifactKind, ComponentInventory, ExpectedProviderOutput, HostResources, ProviderCandidate,
    ProviderExecutionLimits, ProviderExecutionOutcome, ProviderExecutionRequest,
    ProviderRegistration, ProviderRegistry, ProviderResolutionRequest,
};
use crate::{CancellationToken, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

const PROVIDER: &str = "org.egohygiene.aniflow.amv-ollama";
const CAPABILITY: &str = "aniflow/amv.evidence-notes";
const ADAPTER: &[u8] = include_bytes!("../../providers/amv-ollama/provider.py");
const TEMPLATE: &str = "Review only the normalized record facts. Return observed_fact_ids, suggestions and questions as JSON matching the supplied schema. Cite known fact identifiers and exact final-video ordinal/PTS together if used. Never infer visual, audio, safety, decoding, watermark, technical acceptance or human approval from these records. No tools, commands, paths, metadata or execution instructions are available. Suggestions are unverified review questions, not decisions.";

/// Private deployment selection. No endpoint, executable path or diagnostic text
/// is included in the portable telemetry below. Each invocation is explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub schema: String,
    pub enabled: bool,
    pub python_executable: PathBuf,
    pub endpoint: SocketAddr,
    pub runtime_version: String,
    pub model_name: String,
    pub model_sha256: String,
    pub service_egress_disabled_declared: bool,
    pub context_tokens: u32,
    pub output_tokens: u32,
    pub timeout_milliseconds: u64,
}
impl Configuration {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        super::bounded(bytes, 16_384)?;
        let value: Self = decode_json(bytes, "AMV local runtime configuration")?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        require_sha256(&self.model_sha256, "AMV selected model digest")?;
        if self.schema != "aniflow.amv-local-runtime/v1"
            || !self.endpoint.ip().is_loopback()
            || self.endpoint.port() == 0
            || !self.service_egress_disabled_declared
            || !self.python_executable.is_absolute()
            || self.python_executable.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || self.model_name.is_empty()
            || self.model_name.len() > 48
            || !self
                .model_name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-:/".contains(&c))
            || self.runtime_version.is_empty()
            || self.runtime_version.len() > 48
            || !self
                .runtime_version
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-+".contains(&c))
            || !(512..=32_768).contains(&self.context_tokens)
            || self.output_tokens == 0
            || self.output_tokens > 2048
            || self.output_tokens >= self.context_tokens
            || !(1..=300_000).contains(&self.timeout_milliseconds)
        {
            return Err(invalid(
                "explicit bounded local AMV model selection required",
            ));
        }
        Ok(())
    }
    pub fn identity(&self) -> Result<CandidateIdentity> {
        self.validate()?;
        // Deployment location is private. Semantic pins include model, adapter,
        // exact templates/contracts and generation bounds, never host paths.
        let semantic = serde_json::json!({"schema":"aniflow.amv-local-profile/v1","provider":PROVIDER,"model_name":self.model_name,"model_sha256":self.model_sha256,"runtime_version":self.runtime_version,"adapter_sha256":local::hash(ADAPTER),"template_sha256":local::hash(TEMPLATE.as_bytes()),"output_schema_sha256":local::hash(include_bytes!("../../docs/contracts/amv-advisory-candidate-v1.schema.json")),"context_tokens":self.context_tokens,"output_tokens":self.output_tokens,"timeout_milliseconds":self.timeout_milliseconds,"attempts":1,"modality":"normalized_record_evidence","tools":false});
        Ok(CandidateIdentity {
            provider_id: PROVIDER.into(),
            model_id: format!("{}/sha256:{}", self.model_name, self.model_sha256),
            runtime_revision: format!("ollama/{}", self.runtime_version),
            profile_sha256: canonical_sha256(&semantic)?,
            template_sha256: local::hash(TEMPLATE.as_bytes()),
            output_schema_sha256: local::hash(include_bytes!(
                "../../docs/contracts/amv-advisory-candidate-v1.schema.json"
            )),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Disabled,
    Unavailable,
    Succeeded,
    Refused,
    Cancelled,
    TimedOut,
}
/// Portable correlation only. Raw model text, locators, prompts, configuration
/// values, diagnostics and operational provider events are deliberately excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Telemetry {
    pub schema: String,
    pub state: State,
    pub packet_sha256: Option<String>,
    pub profile_sha256: Option<String>,
    pub provider_lock_sha256: Option<String>,
    pub execution_report_sha256: Option<String>,
    pub process_attempts: u8,
    pub model_calls_reported: Option<u8>,
    pub duration_milliseconds: Option<u64>,
    pub input_tokens_reported: Option<u64>,
    pub output_tokens_reported: Option<u64>,
    pub cost_state: String,
    pub cache_hit: bool,
    pub offline_qualified: bool,
    pub model_weights_verified: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunResult {
    pub schema: String,
    pub telemetry: Telemetry,
    pub advisory: Option<Advisory>,
    pub private_run_directory: Option<PathBuf>,
}
fn result(state: State) -> RunResult {
    RunResult {
        schema: "aniflow.amv-local-result/v1".into(),
        telemetry: Telemetry {
            schema: "aniflow.amv-local-telemetry/v1".into(),
            state,
            packet_sha256: None,
            profile_sha256: None,
            provider_lock_sha256: None,
            execution_report_sha256: None,
            process_attempts: 0,
            model_calls_reported: None,
            duration_milliseconds: None,
            input_tokens_reported: None,
            output_tokens_reported: None,
            cost_state: "unknown".into(),
            cache_hit: false,
            offline_qualified: false,
            model_weights_verified: false,
        },
        advisory: None,
        private_run_directory: None,
    }
}

fn registration(config: &Configuration) -> Result<ProviderRegistration> {
    config.validate()?;
    let schema = serde_json::json!({"id":"aniflow.amv-local-provider/v1","version":"1.0.0","sha256":local::hash(include_bytes!("../../docs/contracts/amv-local-provider-v1.schema.json"))});
    let manifest:ProviderManifest=serde_json::from_value(serde_json::json!({"schema":"aniflow.provider-manifest/v1","provider":{"id":PROVIDER,"version":"1.0.0","display_name":"aniflow local record-evidence notes"},"configuration_schemas":[schema.clone()],"capabilities":[{"id":CAPABILITY,"version":"1.0.0","kind":"artifact_validator","configuration_schema":schema.clone(),"inputs":[{"name":"packet","artifact_type":"application/json","artifact_role":"validation_evidence","cardinality":"one","immutable":true}],"outputs":[{"name":"notes","artifact_type":"application/json","artifact_role":"run_evidence","cardinality":"one","immutable":true}],"batching":"whole_artifact","max_concurrency":1,"requirements":{"tools":[],"codecs":[],"models":[],"compute":{"minimum_cpu_threads":1,"minimum_memory_mib":1,"minimum_storage_mib":1,"gpu":"forbidden","network":"required"}},"behavior":{"determinism":"environment_dependent","fidelity":"observational","cacheable":false,"content_changes":false,"side_effects":["filesystem_read","filesystem_write","subprocess","network","ai"]},"lifecycle":{"progress":"none","cancellation":"process_signal"}}],"provenance":{"required":["input_digests","output_digests","effective_configuration","provider_identity","capability_identity","validation_evidence"]}})).map_err(|_|invalid("embedded AMV provider manifest differs"))?;
    let mut values = BTreeMap::new();
    values.insert(
        "deployment".into(),
        serde_json::to_value(config).map_err(|_| invalid("AMV deployment cannot be encoded"))?,
    );
    values.insert(
        "profile".into(),
        serde_json::to_value(config.identity()?)
            .map_err(|_| invalid("AMV profile cannot be encoded"))?,
    );
    values.insert(
        "adapter_sha256".into(),
        serde_json::json!(local::hash(ADAPTER)),
    );
    let configuration = ProviderConfiguration::new(
        crate::ProviderReference {
            id: PROVIDER.into(),
            version: "1.0.0".into(),
        },
        crate::CapabilityReference {
            id: CAPABILITY.into(),
            version: "1.0.0".into(),
        },
        serde_json::from_value(schema).map_err(|_| invalid("AMV configuration schema differs"))?,
        values,
    )?;
    ProviderRegistration::new(
        "amv_local",
        manifest,
        configuration,
        &config.python_executable,
        "amv_ollama_v1",
        ComponentInventory::default(),
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeResult {
    schema: String,
    candidate: Candidate,
    runtime_version_reported: String,
    model_name_reported: String,
    model_sha256_reported: String,
    model_calls: u8,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}
fn accept(
    packet: &Packet,
    config: &Configuration,
    value: NativeResult,
) -> Result<(Candidate, Option<u64>, Option<u64>)> {
    super::bounded(&local::bytes(&value.candidate)?, 65_536)?;
    if value.schema != "aniflow.amv-ollama-result/v1"
        || value.model_calls != 1
        || value.runtime_version_reported != config.runtime_version
        || value.model_name_reported != config.model_name
        || value.model_sha256_reported != config.model_sha256
        || value.candidate.declared_identity != config.identity()?
        || value
            .input_tokens
            .is_some_and(|n| n > u64::from(config.context_tokens))
        || value
            .output_tokens
            .is_some_and(|n| n > u64::from(config.output_tokens))
    {
        return Err(invalid("local AMV provider identity or usage differs"));
    }
    super::validate_supplied(packet, &value.candidate)?;
    Ok((value.candidate, value.input_tokens, value.output_tokens))
}

/// Disabled paths read no source/configuration/executable and create no workspace.
/// Enabled paths retain a fresh private root, even on failure, and never retry or
/// reuse loose provider files. Only the existing runtime owns child execution.
pub fn run(
    request: &Request,
    config: &Configuration,
    root: &Path,
    host: HostResources,
    token: &CancellationToken,
) -> Result<RunResult> {
    request.validate()?;
    if request.mode == Mode::Disabled || !config.enabled {
        return Ok(result(State::Disabled));
    }
    cancelled(token)?;
    config.validate()?;
    let mut outcome = result(State::Unavailable);
    let identity = config.identity()?;
    outcome.telemetry.profile_sha256 = Some(identity.profile_sha256.clone());
    let mut registry = ProviderRegistry::new();
    registry.register(registration(config)?)?;
    let resolution = ProviderResolutionRequest {
        capability_id: CAPABILITY.into(),
        capability_version_requirement: "=1.0.0".into(),
        replacement: None,
        primary: ProviderCandidate {
            registration_id: "amv_local".into(),
        },
        fallbacks: vec![],
        allowed_side_effects: vec![
            crate::SideEffect::FilesystemRead,
            crate::SideEffect::FilesystemWrite,
            crate::SideEffect::Subprocess,
            crate::SideEffect::Network,
            crate::SideEffect::Ai,
        ],
        offline: false,
        host,
    };
    let Ok(provider) = registry.resolve(&resolution) else {
        return Ok(outcome);
    };
    let packet = prepare(request, token)?
        .packet
        .ok_or_else(|| invalid("AMV packet unavailable"))?;
    let source = request
        .video_run_directory
        .canonicalize()
        .map_err(|_| invalid("AMV source run unavailable"))?;
    local::parent(root)?;
    if root.starts_with(&source) || source.starts_with(root) {
        return Err(invalid(
            "AMV advisory output must be disjoint from accepted source",
        ));
    }
    cancelled(token)?;
    local::directory(root)?;
    local::directory(&root.join("outputs"))?;
    outcome.private_run_directory = Some(root.to_owned());
    outcome.telemetry.packet_sha256 = Some(packet.packet_sha256.clone());
    outcome.telemetry.provider_lock_sha256 = Some(provider.provider_lock().lock_sha256.clone());
    local::write(&root.join("adapter.py"), ADAPTER)?;
    let invocation = serde_json::json!({"schema":"aniflow.amv-ollama-invocation/v1","packet":packet,"identity":identity,"endpoint":config.endpoint.to_string(),"runtime_version":config.runtime_version,"model_name":config.model_name,"model_sha256":config.model_sha256,"service_egress_disabled_declared":config.service_egress_disabled_declared,"context_tokens":config.context_tokens,"output_tokens":config.output_tokens,"timeout_milliseconds":config.timeout_milliseconds,"template":TEMPLATE,"candidate_schema":serde_json::from_slice::<serde_json::Value>(include_bytes!("../../docs/contracts/amv-advisory-candidate-v1.schema.json")).map_err(|_|invalid("AMV candidate schema unavailable"))?});
    let input = local::bytes(&invocation)?;
    super::bounded(&input, 131_072)?;
    local::write(&root.join("invocation.json"), &input)?;
    local::write(
        &root.join("provider-lock.json"),
        &local::bytes(provider.provider_lock())?,
    )?;
    cancelled(token)?;
    if prepare(request, token)?.packet.as_ref() != Some(&packet) {
        return Err(invalid("AMV evidence changed before local invocation"));
    }
    let report = provider.execute_v3_strict(
        &ProviderExecutionRequest {
            arguments: vec![
                OsString::from("-I"),
                OsString::from("-S"),
                root.join("adapter.py").into_os_string(),
                OsString::from("--input"),
                root.join("invocation.json").into_os_string(),
                OsString::from("--output"),
                root.join("outputs/notes.json").into_os_string(),
            ],
            working_directory: root.to_owned(),
            output_directory: root.join("outputs"),
            expected_outputs: vec![ExpectedProviderOutput {
                port: "notes".into(),
                relative_path: "notes.json".into(),
                kind: ArtifactKind::File,
            }],
            limits: ProviderExecutionLimits {
                timeout: Duration::from_millis(config.timeout_milliseconds),
                termination_grace_period: Duration::from_millis(250),
                maximum_stdout_bytes: 1024,
                maximum_stderr_bytes: 1024,
                maximum_artifact_files: 1,
                maximum_artifact_bytes: 131_072,
            },
            sensitive_values: vec![
                root.to_string_lossy().into_owned(),
                config.endpoint.to_string(),
            ],
        },
        token,
        |_| {},
    )?;
    // This complete execution record is private. Portable telemetry below is an
    // explicit projection, not a passthrough of child diagnostics or events.
    local::write(&root.join("execution.json"), &local::bytes(&report)?)?;
    outcome.telemetry.execution_report_sha256 = Some(report.report_sha256.clone());
    outcome.telemetry.duration_milliseconds = Some(report.payload.duration_milliseconds);
    outcome.telemetry.process_attempts = u8::from(
        report
            .payload
            .events
            .iter()
            .any(|event| event.kind == crate::ProviderEventKind::ExecutionStarted),
    );
    outcome.telemetry.state = match report.payload.outcome {
        ProviderExecutionOutcome::Succeeded => State::Succeeded,
        ProviderExecutionOutcome::TimedOut => State::TimedOut,
        ProviderExecutionOutcome::Cancelled => State::Cancelled,
        _ => State::Refused,
    };
    if outcome.telemetry.state == State::Succeeded {
        let accepted = (|| -> Result<Advisory> {
            let native: NativeResult =
                local::read_bounded(&root.join("outputs/notes.json"), 131_072)?;
            let output = report
                .payload
                .outputs
                .first()
                .ok_or_else(|| invalid("AMV notes output missing"))?;
            let observed = crate::frame_io::identity(&root.join("outputs/notes.json"), 131_072)?;
            if observed != (output.sha256.clone(), output.byte_count)
                || crate::frame_io::identity(&root.join("adapter.py"), ADAPTER.len() as u64)?
                    != (local::hash(ADAPTER), ADAPTER.len() as u64)
                || crate::frame_io::identity(&root.join("invocation.json"), 131_072)?
                    != (local::hash(&input), input.len() as u64)
            {
                return Err(invalid("AMV local input/adapter/output changed"));
            }
            let (candidate, input_tokens, output_tokens) = accept(&packet, config, native)?;
            let advisory = validate_candidate(request, &packet, &candidate, token)?;
            outcome.telemetry.model_calls_reported = Some(1);
            outcome.telemetry.input_tokens_reported = input_tokens;
            outcome.telemetry.output_tokens_reported = output_tokens;
            Ok(advisory)
        })();
        match accepted {
            Ok(advisory) => outcome.advisory = Some(advisory),
            Err(_) => {
                outcome.telemetry.state = if token.is_cancelled() {
                    State::Cancelled
                } else {
                    State::Refused
                }
            }
        }
    }
    // Advisory v1 deliberately retains its supplied-candidate semantics. The
    // separate execution projection supplies observed process facts without
    // rewriting an existing schema or promoting model-reported weights to proof.
    local::write(&root.join("result.json"), &local::bytes(&outcome)?)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests;
