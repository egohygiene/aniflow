//! Deterministic local-provider resolution and bounded process execution.
//!
//! Provider manifests remain inert declaration data. A provider can run only
//! after an embedding application explicitly registers its executable and
//! grants every declared side effect.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

use crate::error::{Error, ErrorCategory, Result};
use crate::provider::{
    ArtifactCardinality, CapabilityDeclaration, CapabilityReference, ComponentIdentity,
    ComponentRequirement, ConfigurationSchemaReference, ProviderConfiguration, ProviderManifest,
    ProviderReference, RequirementLevel, SideEffect, canonical_sha256, decode_json, invalid,
    require_nonempty, require_schema, require_sha256, require_token, validate_capability_id,
    validate_capability_reference, validate_component_identities, validate_configuration_schema,
    validate_provider_reference, validate_semantic_version,
};
use crate::segmentation::CancellationToken;

pub const PROVIDER_LOCK_SCHEMA_V1: &str = "aniflow.provider-lock/v1";
pub const PROVIDER_EVENT_SCHEMA_V1: &str = "aniflow.provider-event/v1";
pub const PROVIDER_EXECUTION_REPORT_SCHEMA_V1: &str = "aniflow.provider-execution-report/v1";

/// Exact component identities observed for a registered implementation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentInventory {
    pub tools: Vec<ComponentIdentity>,
    pub codecs: Vec<ComponentIdentity>,
    pub models: Vec<ComponentIdentity>,
}

impl ComponentInventory {
    fn validate(&self) -> Result<()> {
        validate_runtime_components(&self.tools, "tool")?;
        validate_runtime_components(&self.codecs, "codec")?;
        validate_runtime_components(&self.models, "model")
    }
}

/// Caller-observed local capacity used for deterministic availability checks.
///
/// CPU, memory, storage, GPU, and network observations are supplied by the
/// embedding application so platform-specific measurement remains outside the
/// provider declaration and can itself be retained as run evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostResources {
    pub cpu_threads: u16,
    pub memory_mib: u64,
    pub storage_mib: u64,
    pub gpu_available: bool,
    pub network_available: bool,
}

impl HostResources {
    pub fn validate(&self) -> Result<()> {
        if self.cpu_threads == 0 {
            return Err(invalid("host cpu_threads must be at least 1"));
        }
        Ok(())
    }
}

/// One explicit local implementation. Registering never launches it.
#[derive(Debug, Clone)]
pub struct ProviderRegistration {
    registration_id: String,
    manifest: ProviderManifest,
    configuration: ProviderConfiguration,
    executable: PathBuf,
    implementation_id: String,
    components: ComponentInventory,
    capability: CapabilityDeclaration,
}

impl ProviderRegistration {
    pub fn new(
        registration_id: impl Into<String>,
        manifest: ProviderManifest,
        configuration: ProviderConfiguration,
        executable: impl Into<PathBuf>,
        implementation_id: impl Into<String>,
        components: ComponentInventory,
    ) -> Result<Self> {
        let registration_id = registration_id.into();
        let executable = executable.into();
        let implementation_id = implementation_id.into();

        require_token(&registration_id, "registration_id")?;
        require_token(&implementation_id, "implementation_id")?;
        manifest.validate()?;
        configuration.validate()?;
        components.validate()?;

        if !executable.is_absolute() {
            return Err(invalid(
                "registered provider executable must be an absolute path",
            ));
        }
        if configuration.provider.id != manifest.provider.id
            || configuration.provider.version != manifest.provider.version
        {
            return Err(invalid(
                "provider configuration identity does not match its manifest",
            ));
        }

        let capability = manifest
            .capabilities
            .iter()
            .find(|candidate| {
                candidate.id == configuration.capability.id
                    && candidate.version == configuration.capability.version
            })
            .cloned()
            .ok_or_else(|| {
                invalid("provider configuration capability is not declared by its manifest")
            })?;
        if capability.configuration_schema != configuration.configuration_schema {
            return Err(invalid(
                "provider configuration schema does not match its capability declaration",
            ));
        }
        if !manifest
            .configuration_schemas
            .contains(&configuration.configuration_schema)
        {
            return Err(invalid(
                "provider configuration schema is not declared by its manifest",
            ));
        }

        Ok(Self {
            registration_id,
            manifest,
            configuration,
            executable,
            implementation_id,
            components,
            capability,
        })
    }

    #[must_use]
    pub fn registration_id(&self) -> &str {
        &self.registration_id
    }

    #[must_use]
    pub fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    #[must_use]
    pub fn configuration(&self) -> &ProviderConfiguration {
        &self.configuration
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn implementation_id(&self) -> &str {
        &self.implementation_id
    }

    #[must_use]
    pub fn components(&self) -> &ComponentInventory {
        &self.components
    }
}

/// Registry lookup key used by a primary, replacement, or fallback slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCandidate {
    pub registration_id: String,
}

/// Deterministic origin of the selected provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSelectionSource {
    Replacement,
    Primary,
    Fallback,
}

/// Exact precedence slot considered during resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSelection {
    pub source: ProviderSelectionSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_index: Option<u32>,
}

impl ProviderSelection {
    fn replacement() -> Self {
        Self {
            source: ProviderSelectionSource::Replacement,
            fallback_index: None,
        }
    }

    fn primary() -> Self {
        Self {
            source: ProviderSelectionSource::Primary,
            fallback_index: None,
        }
    }

    fn fallback(index: usize) -> Result<Self> {
        Ok(Self {
            source: ProviderSelectionSource::Fallback,
            fallback_index: Some(
                u32::try_from(index).map_err(|_| {
                    invalid("fallback candidate count exceeds the v1 contract limit")
                })?,
            ),
        })
    }

    fn validate(&self) -> Result<()> {
        match (self.source, self.fallback_index) {
            (ProviderSelectionSource::Fallback, Some(_))
            | (ProviderSelectionSource::Replacement | ProviderSelectionSource::Primary, None) => {
                Ok(())
            }
            _ => Err(invalid(
                "fallback_index must be present only for a fallback selection",
            )),
        }
    }
}

/// Complete standalone resolution policy for one capability invocation.
#[derive(Debug, Clone)]
pub struct ProviderResolutionRequest {
    pub capability_id: String,
    pub capability_version_requirement: String,
    pub replacement: Option<ProviderCandidate>,
    pub primary: ProviderCandidate,
    pub fallbacks: Vec<ProviderCandidate>,
    pub allowed_side_effects: Vec<SideEffect>,
    pub offline: bool,
    pub host: HostResources,
}

impl ProviderResolutionRequest {
    fn validate(&self) -> Result<VersionReq> {
        validate_capability_id(&self.capability_id)?;
        let requirement =
            VersionReq::parse(&self.capability_version_requirement).map_err(|error| {
                invalid(format!(
                    "invalid capability version requirement {}: {error}",
                    self.capability_version_requirement
                ))
            })?;
        self.host.validate()?;

        let mut registrations = BTreeSet::new();
        for candidate in self
            .replacement
            .iter()
            .chain(std::iter::once(&self.primary))
            .chain(self.fallbacks.iter())
        {
            require_token(&candidate.registration_id, "candidate registration_id")?;
            if !registrations.insert(candidate.registration_id.as_str()) {
                return Err(invalid(format!(
                    "candidate registration {} appears more than once",
                    candidate.registration_id
                )));
            }
        }

        let effects = self
            .allowed_side_effects
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if effects.len() != self.allowed_side_effects.len() {
            return Err(invalid("allowed_side_effects contains a duplicate"));
        }
        Ok(requirement)
    }
}

/// Stable machine reason for an unavailable candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityCode {
    NotRegistered,
    CapabilityMismatch,
    VersionMismatch,
    ExecutableUnavailable,
    ExecutableNotRegularFile,
    ExecutableNotRunnable,
    ComponentMissing,
    ComponentVersionMismatch,
    ComponentDigestMismatch,
    InsufficientCpu,
    InsufficientMemory,
    InsufficientStorage,
    GpuUnavailable,
    NetworkUnavailable,
    OfflineIncompatible,
    SideEffectDenied,
}

/// Actionable, non-secret detail for one availability rejection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvailabilityReason {
    pub code: AvailabilityCode,
    pub detail: String,
}

/// Evidence for one precedence slot, including successful selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResolutionAttempt {
    pub candidate: ProviderCandidate,
    pub selection: ProviderSelection,
    pub available: bool,
    pub reasons: Vec<AvailabilityReason>,
}

/// Typed resolution failure with every attempted candidate retained in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResolutionFailure {
    pub message: String,
    pub attempts: Vec<ProviderResolutionAttempt>,
}

impl fmt::Display for ProviderResolutionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ProviderResolutionFailure {}

/// Exact executable implementation identity retained in a provider lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderImplementationIdentity {
    pub id: String,
    pub executable_sha256: String,
}

/// Canonical material covered by `lock_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderLockPayload {
    pub registration_id: String,
    pub provider: ProviderReference,
    pub capability: CapabilityReference,
    pub configuration_schema: ConfigurationSchemaReference,
    pub effective_configuration_sha256: String,
    pub manifest_sha256: String,
    pub implementation: ProviderImplementationIdentity,
    pub tools: Vec<ComponentIdentity>,
    pub codecs: Vec<ComponentIdentity>,
    pub models: Vec<ComponentIdentity>,
    pub authorized_side_effects: Vec<SideEffect>,
    pub selection: ProviderSelection,
    pub offline: bool,
}

/// Self-validating local authority and compatibility record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderLock {
    pub schema: String,
    pub algorithm: String,
    pub lock_sha256: String,
    pub payload: ProviderLockPayload,
}

impl ProviderLock {
    fn new(payload: ProviderLockPayload) -> Result<Self> {
        validate_lock_payload(&payload)?;
        let lock_sha256 = canonical_sha256(&payload)?;
        Ok(Self {
            schema: PROVIDER_LOCK_SCHEMA_V1.to_owned(),
            algorithm: "sha256".to_owned(),
            lock_sha256,
            payload,
        })
    }

    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let provider_lock: Self = decode_json(input, "provider lock")?;
        provider_lock.validate()?;
        Ok(provider_lock)
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(&self.schema, PROVIDER_LOCK_SCHEMA_V1)?;
        if self.algorithm != "sha256" {
            return Err(invalid(format!(
                "unsupported provider lock algorithm {}; expected sha256",
                self.algorithm
            )));
        }
        require_sha256(&self.lock_sha256, "lock_sha256")?;
        validate_lock_payload(&self.payload)?;
        let expected = canonical_sha256(&self.payload)?;
        if expected != self.lock_sha256 {
            return Err(invalid(format!(
                "provider lock does not match its canonical payload; expected {expected}"
            )));
        }
        Ok(())
    }

    /// Atomically publish a new lock without replacing an existing record.
    pub fn write_new(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        let path = path.as_ref();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| {
            io_failure(format!(
                "failed to create provider lock directory {}: {error}",
                parent.display()
            ))
        })?;
        let mut temporary = NamedTempFile::new_in(parent).map_err(|error| {
            io_failure(format!(
                "failed to create temporary provider lock in {}: {error}",
                parent.display()
            ))
        })?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), self).map_err(|error| {
            Error::new(
                ErrorCategory::Internal,
                format!("failed to encode provider lock: {error}"),
            )
        })?;
        temporary
            .as_file_mut()
            .write_all(b"\n")
            .map_err(|error| io_failure(format!("failed to finish provider lock: {error}")))?;
        temporary
            .as_file_mut()
            .sync_all()
            .map_err(|error| io_failure(format!("failed to sync provider lock: {error}")))?;
        temporary.persist_noclobber(path).map_err(|error| {
            let message = if error.error.kind() == io::ErrorKind::AlreadyExists {
                format!("provider lock already exists: {}", path.display())
            } else {
                format!(
                    "failed to publish provider lock {}: {}",
                    path.display(),
                    error.error
                )
            };
            io_failure(message)
        })?;
        Ok(())
    }
}

/// Resolution result that can execute only its selected registration.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    registration: ProviderRegistration,
    provider_lock: ProviderLock,
    attempts: Vec<ProviderResolutionAttempt>,
}

impl ResolvedProvider {
    #[must_use]
    pub fn registration(&self) -> &ProviderRegistration {
        &self.registration
    }

    #[must_use]
    pub fn provider_lock(&self) -> &ProviderLock {
        &self.provider_lock
    }

    #[must_use]
    pub fn attempts(&self) -> &[ProviderResolutionAttempt] {
        &self.attempts
    }

    /// Execute the selected provider exactly once. Resolution fallback is no
    /// longer consulted after this method starts.
    pub fn execute<F>(
        &self,
        request: &ProviderExecutionRequest,
        cancellation: &CancellationToken,
        mut on_event: F,
    ) -> Result<ProviderExecutionReport>
    where
        F: FnMut(&ProviderEvent),
    {
        self.provider_lock.validate()?;
        validate_execution_request(request, &self.registration.capability)?;

        let started_at = Utc::now().to_rfc3339();
        let started = Instant::now();
        let mut events = Vec::new();
        let executable_matches_lock =
            executable_identity(&self.registration.executable).is_ok_and(|digest| {
                digest == self.provider_lock.payload.implementation.executable_sha256
            });
        if !executable_matches_lock {
            emit_event(
                &mut events,
                &self.provider_lock.lock_sha256,
                ProviderEventKind::ExecutionFailed,
                &mut on_event,
            )?;
            return build_report(ReportParts {
                provider_lock: self.provider_lock.clone(),
                outcome: ProviderExecutionOutcome::Failed,
                started_at,
                started,
                bounds: request.limits.into(),
                termination: ProviderTermination {
                    reason: TerminationReason::PreflightRejected,
                    exit_code: None,
                    signal: None,
                },
                stdout: empty_diagnostic(StreamKind::Stdout),
                stderr: empty_diagnostic(StreamKind::Stderr),
                outputs: Vec::new(),
                events,
                failure: Some(execution_failure(
                    ProviderExecutionFailureCode::ImplementationChanged,
                    "registered provider executable no longer matches its lock",
                )),
            });
        }
        if cancellation.is_cancelled() {
            emit_event(
                &mut events,
                &self.provider_lock.lock_sha256,
                ProviderEventKind::CancellationObserved,
                &mut on_event,
            )?;
            emit_event(
                &mut events,
                &self.provider_lock.lock_sha256,
                ProviderEventKind::ExecutionFailed,
                &mut on_event,
            )?;
            return build_report(ReportParts {
                provider_lock: self.provider_lock.clone(),
                outcome: ProviderExecutionOutcome::Cancelled,
                started_at,
                started,
                bounds: request.limits.into(),
                termination: ProviderTermination {
                    reason: TerminationReason::Cancelled,
                    exit_code: None,
                    signal: None,
                },
                stdout: empty_diagnostic(StreamKind::Stdout),
                stderr: empty_diagnostic(StreamKind::Stderr),
                outputs: Vec::new(),
                events,
                failure: Some(execution_failure(
                    ProviderExecutionFailureCode::Cancelled,
                    "cancellation was already requested before provider start",
                )),
            });
        }

        emit_event(
            &mut events,
            &self.provider_lock.lock_sha256,
            ProviderEventKind::ExecutionStarted,
            &mut on_event,
        )?;
        let mut command = Command::new(&self.registration.executable);
        command
            .args(&request.arguments)
            .current_dir(&request.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                emit_event(
                    &mut events,
                    &self.provider_lock.lock_sha256,
                    ProviderEventKind::ExecutionFailed,
                    &mut on_event,
                )?;
                return build_report(ReportParts {
                    provider_lock: self.provider_lock.clone(),
                    outcome: ProviderExecutionOutcome::Failed,
                    started_at,
                    started,
                    bounds: request.limits.into(),
                    termination: ProviderTermination {
                        reason: TerminationReason::SpawnFailed,
                        exit_code: None,
                        signal: None,
                    },
                    stdout: empty_diagnostic(StreamKind::Stdout),
                    stderr: empty_diagnostic(StreamKind::Stderr),
                    outputs: Vec::new(),
                    events,
                    failure: Some(execution_failure(
                        ProviderExecutionFailureCode::SpawnFailed,
                        "registered provider could not be started",
                    )),
                });
            }
        };
        let stdout = child.stdout.take().ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "provider stdout pipe was not created",
            )
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            Error::new(
                ErrorCategory::Internal,
                "provider stderr pipe was not created",
            )
        })?;
        let stdout_exceeded = Arc::new(AtomicBool::new(false));
        let stderr_exceeded = Arc::new(AtomicBool::new(false));
        let stdout_reader = spawn_capture_reader(
            stdout,
            request.limits.maximum_stdout_bytes,
            Arc::clone(&stdout_exceeded),
        );
        let stderr_reader = spawn_capture_reader(
            stderr,
            request.limits.maximum_stderr_bytes,
            Arc::clone(&stderr_exceeded),
        );

        let mut stop = None;
        let status = loop {
            if cancellation.is_cancelled() {
                stop = Some(StopTrigger::Cancelled);
            } else if started.elapsed() >= request.limits.timeout {
                stop = Some(StopTrigger::TimedOut);
            } else if stdout_exceeded.load(Ordering::SeqCst) {
                stop = Some(StopTrigger::StdoutLimit);
            } else if stderr_exceeded.load(Ordering::SeqCst) {
                stop = Some(StopTrigger::StderrLimit);
            } else if let Some(limit) = observe_artifact_limit(
                &request.output_directory,
                request.limits.maximum_artifact_files,
                request.limits.maximum_artifact_bytes,
            ) {
                stop = Some(limit);
            }

            if stop.is_some() {
                break terminate_process_tree(&mut child, request.limits.termination_grace_period)
                    .map_err(|error| {
                        execution_error(format!(
                            "failed to terminate provider process tree: {error}"
                        ))
                    })?;
            }
            if let Some(status) = child.try_wait().map_err(|error| {
                execution_error(format!("failed to observe provider process: {error}"))
            })? {
                cleanup_process_group(child.id(), request.limits.termination_grace_period);
                break Some(status);
            }
            thread::sleep(Duration::from_millis(10));
        };

        let stdout_capture = join_capture(stdout_reader)?;
        let stderr_capture = join_capture(stderr_reader)?;
        let stdout = redact_capture(
            StreamKind::Stdout,
            &stdout_capture,
            &request.sensitive_values,
        );
        let stderr = redact_capture(
            StreamKind::Stderr,
            &stderr_capture,
            &request.sensitive_values,
        );

        if let Some(trigger) = stop.or({
            if stdout.truncated {
                Some(StopTrigger::StdoutLimit)
            } else if stderr.truncated {
                Some(StopTrigger::StderrLimit)
            } else {
                None
            }
        }) {
            let (event, outcome, reason, code, detail) = trigger.evidence();
            emit_event(
                &mut events,
                &self.provider_lock.lock_sha256,
                event,
                &mut on_event,
            )?;
            cleanup_output_directory(&request.output_directory)?;
            emit_event(
                &mut events,
                &self.provider_lock.lock_sha256,
                ProviderEventKind::ExecutionFailed,
                &mut on_event,
            )?;
            return build_report(ReportParts {
                provider_lock: self.provider_lock.clone(),
                outcome,
                started_at,
                started,
                bounds: request.limits.into(),
                termination: termination(reason, status.as_ref()),
                stdout,
                stderr,
                outputs: Vec::new(),
                events,
                failure: Some(execution_failure(code, detail)),
            });
        }

        let Some(status) = status else {
            cleanup_output_directory(&request.output_directory)?;
            return Err(Error::new(
                ErrorCategory::Internal,
                "provider process ended without termination status",
            ));
        };
        emit_event(
            &mut events,
            &self.provider_lock.lock_sha256,
            ProviderEventKind::ProcessExited,
            &mut on_event,
        )?;

        if stdout_capture.read_failed || stderr_capture.read_failed {
            cleanup_output_directory(&request.output_directory)?;
            emit_event(
                &mut events,
                &self.provider_lock.lock_sha256,
                ProviderEventKind::ExecutionFailed,
                &mut on_event,
            )?;
            return build_report(ReportParts {
                provider_lock: self.provider_lock.clone(),
                outcome: ProviderExecutionOutcome::Failed,
                started_at,
                started,
                bounds: request.limits.into(),
                termination: termination(TerminationReason::NaturalExit, Some(&status)),
                stdout,
                stderr,
                outputs: Vec::new(),
                events,
                failure: Some(execution_failure(
                    ProviderExecutionFailureCode::CaptureReadFailed,
                    "provider diagnostics could not be read completely",
                )),
            });
        }
        if !status.success() {
            cleanup_output_directory(&request.output_directory)?;
            emit_event(
                &mut events,
                &self.provider_lock.lock_sha256,
                ProviderEventKind::ExecutionFailed,
                &mut on_event,
            )?;
            return build_report(ReportParts {
                provider_lock: self.provider_lock.clone(),
                outcome: ProviderExecutionOutcome::Failed,
                started_at,
                started,
                bounds: request.limits.into(),
                termination: termination(TerminationReason::NaturalExit, Some(&status)),
                stdout,
                stderr,
                outputs: Vec::new(),
                events,
                failure: Some(execution_failure(
                    ProviderExecutionFailureCode::ExitFailure,
                    "provider returned a non-zero exit status",
                )),
            });
        }

        emit_event(
            &mut events,
            &self.provider_lock.lock_sha256,
            ProviderEventKind::OutputValidationStarted,
            &mut on_event,
        )?;
        let outputs = match validate_outputs(request, &self.registration.capability) {
            Ok(outputs) => outputs,
            Err(failure) => {
                let failure = redact_execution_failure(failure, &request.sensitive_values);
                cleanup_output_directory(&request.output_directory)?;
                let (outcome, event) = match failure.code {
                    ProviderExecutionFailureCode::ArtifactFileLimit
                    | ProviderExecutionFailureCode::ArtifactByteLimit => (
                        ProviderExecutionOutcome::ArtifactLimitExceeded,
                        ProviderEventKind::ArtifactLimitExceeded,
                    ),
                    _ => (
                        ProviderExecutionOutcome::InvalidOutput,
                        ProviderEventKind::ExecutionFailed,
                    ),
                };
                emit_event(
                    &mut events,
                    &self.provider_lock.lock_sha256,
                    event,
                    &mut on_event,
                )?;
                if event != ProviderEventKind::ExecutionFailed {
                    emit_event(
                        &mut events,
                        &self.provider_lock.lock_sha256,
                        ProviderEventKind::ExecutionFailed,
                        &mut on_event,
                    )?;
                }
                return build_report(ReportParts {
                    provider_lock: self.provider_lock.clone(),
                    outcome,
                    started_at,
                    started,
                    bounds: request.limits.into(),
                    termination: termination(TerminationReason::NaturalExit, Some(&status)),
                    stdout,
                    stderr,
                    outputs: Vec::new(),
                    events,
                    failure: Some(failure),
                });
            }
        };
        emit_event(
            &mut events,
            &self.provider_lock.lock_sha256,
            ProviderEventKind::OutputValidated,
            &mut on_event,
        )?;
        emit_event(
            &mut events,
            &self.provider_lock.lock_sha256,
            ProviderEventKind::ExecutionSucceeded,
            &mut on_event,
        )?;
        build_report(ReportParts {
            provider_lock: self.provider_lock.clone(),
            outcome: ProviderExecutionOutcome::Succeeded,
            started_at,
            started,
            bounds: request.limits.into(),
            termination: termination(TerminationReason::NaturalExit, Some(&status)),
            stdout,
            stderr,
            outputs,
            events,
            failure: None,
        })
    }
}

/// Explicit registry. It performs no filesystem search or manifest execution.
#[derive(Debug, Clone, Default)]
pub struct ProviderRegistry {
    registrations: BTreeMap<String, ProviderRegistration>,
}

impl ProviderRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, registration: ProviderRegistration) -> Result<()> {
        let id = registration.registration_id.clone();
        match self.registrations.entry(id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(registration);
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(invalid(format!(
                    "provider registration {id} already exists"
                )));
            }
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        request: &ProviderResolutionRequest,
    ) -> std::result::Result<ResolvedProvider, ProviderResolutionFailure> {
        let version_requirement =
            request
                .validate()
                .map_err(|error| ProviderResolutionFailure {
                    message: error.to_string(),
                    attempts: Vec::new(),
                })?;
        let mut ordered = Vec::new();
        if let Some(replacement) = &request.replacement {
            ordered.push((replacement, ProviderSelection::replacement()));
        }
        ordered.push((&request.primary, ProviderSelection::primary()));
        for (index, fallback) in request.fallbacks.iter().enumerate() {
            let selection =
                ProviderSelection::fallback(index).map_err(|error| ProviderResolutionFailure {
                    message: error.to_string(),
                    attempts: Vec::new(),
                })?;
            ordered.push((fallback, selection));
        }

        let mut attempts = Vec::with_capacity(ordered.len());
        for (candidate, selection) in ordered {
            let Some(registration) = self.registrations.get(&candidate.registration_id) else {
                attempts.push(ProviderResolutionAttempt {
                    candidate: candidate.clone(),
                    selection,
                    available: false,
                    reasons: vec![availability(
                        AvailabilityCode::NotRegistered,
                        "candidate is not explicitly registered",
                    )],
                });
                continue;
            };

            let assessment = assess_availability(registration, request, &version_requirement);
            let (reasons, executable_sha256, components) = match assessment {
                Ok(evidence) => (Vec::new(), evidence.0, evidence.1),
                Err(reasons) => (reasons, String::new(), ComponentInventory::default()),
            };
            let available = reasons.is_empty();
            attempts.push(ProviderResolutionAttempt {
                candidate: candidate.clone(),
                selection: selection.clone(),
                available,
                reasons,
            });
            if !available {
                continue;
            }

            let mut effects = registration.capability.behavior.side_effects.clone();
            effects.sort_unstable();
            let payload = ProviderLockPayload {
                registration_id: registration.registration_id.clone(),
                provider: registration.configuration.provider.clone(),
                capability: registration.configuration.capability.clone(),
                configuration_schema: registration.configuration.configuration_schema.clone(),
                effective_configuration_sha256: registration
                    .configuration
                    .effective_configuration_sha256
                    .clone(),
                manifest_sha256: canonical_sha256(&registration.manifest).map_err(|error| {
                    ProviderResolutionFailure {
                        message: error.to_string(),
                        attempts: attempts.clone(),
                    }
                })?,
                implementation: ProviderImplementationIdentity {
                    id: registration.implementation_id.clone(),
                    executable_sha256,
                },
                tools: components.tools,
                codecs: components.codecs,
                models: components.models,
                authorized_side_effects: effects,
                selection,
                offline: request.offline,
            };
            let provider_lock =
                ProviderLock::new(payload).map_err(|error| ProviderResolutionFailure {
                    message: error.to_string(),
                    attempts: attempts.clone(),
                })?;
            return Ok(ResolvedProvider {
                registration: registration.clone(),
                provider_lock,
                attempts,
            });
        }

        Err(ProviderResolutionFailure {
            message: format!(
                "no registered provider is available for {} {}",
                request.capability_id, request.capability_version_requirement
            ),
            attempts,
        })
    }
}

fn validate_lock_payload(payload: &ProviderLockPayload) -> Result<()> {
    require_token(&payload.registration_id, "registration_id")?;
    validate_provider_reference(&payload.provider)?;
    validate_capability_reference(&payload.capability)?;
    validate_configuration_schema(&payload.configuration_schema)?;
    require_sha256(
        &payload.effective_configuration_sha256,
        "effective_configuration_sha256",
    )?;
    require_sha256(&payload.manifest_sha256, "manifest_sha256")?;
    require_token(&payload.implementation.id, "implementation id")?;
    require_sha256(
        &payload.implementation.executable_sha256,
        "implementation executable_sha256",
    )?;
    validate_runtime_components(&payload.tools, "tool")?;
    validate_runtime_components(&payload.codecs, "codec")?;
    validate_runtime_components(&payload.models, "model")?;
    let effects = payload
        .authorized_side_effects
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if effects.len() != payload.authorized_side_effects.len()
        || !payload
            .authorized_side_effects
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err(invalid("authorized_side_effects must be unique and sorted"));
    }
    if payload.offline && effects.contains(&SideEffect::Network) {
        return Err(invalid(
            "an offline provider lock cannot authorize the network side effect",
        ));
    }
    payload.selection.validate()
}

fn validate_runtime_components(components: &[ComponentIdentity], kind: &str) -> Result<()> {
    validate_component_identities(components, kind)?;
    let mut ids = BTreeSet::new();
    for component in components {
        validate_semantic_version(&component.version, &format!("{kind} version"))?;
        if !ids.insert(component.id.as_str()) {
            return Err(invalid(format!(
                "runtime {kind} inventory must contain one exact identity per id"
            )));
        }
    }
    Ok(())
}

fn assess_availability(
    registration: &ProviderRegistration,
    request: &ProviderResolutionRequest,
    version_requirement: &VersionReq,
) -> std::result::Result<(String, ComponentInventory), Vec<AvailabilityReason>> {
    let mut reasons = Vec::new();
    if registration.capability.id != request.capability_id {
        reasons.push(availability(
            AvailabilityCode::CapabilityMismatch,
            format!(
                "registered capability {} does not match requested capability {}",
                registration.capability.id, request.capability_id
            ),
        ));
    }
    match Version::parse(&registration.capability.version) {
        Ok(version) if !version_requirement.matches(&version) => reasons.push(availability(
            AvailabilityCode::VersionMismatch,
            format!(
                "capability version {} does not satisfy {}",
                registration.capability.version, request.capability_version_requirement
            ),
        )),
        Ok(_) => {}
        Err(_) => reasons.push(availability(
            AvailabilityCode::VersionMismatch,
            "registered capability version is invalid",
        )),
    }

    let executable_sha256 = match executable_identity(&registration.executable) {
        Ok(digest) => Some(digest),
        Err(reason) => {
            reasons.push(reason);
            None
        }
    };
    let tools = match_components(
        &registration.capability.requirements.tools,
        &registration.components.tools,
        "tool",
        &mut reasons,
    );
    let codecs = match_components(
        &registration.capability.requirements.codecs,
        &registration.components.codecs,
        "codec",
        &mut reasons,
    );
    let models = match_components(
        &registration.capability.requirements.models,
        &registration.components.models,
        "model",
        &mut reasons,
    );

    let compute = &registration.capability.requirements.compute;
    if request.host.cpu_threads < compute.minimum_cpu_threads {
        reasons.push(availability(
            AvailabilityCode::InsufficientCpu,
            format!(
                "provider requires {} CPU threads; host observation has {}",
                compute.minimum_cpu_threads, request.host.cpu_threads
            ),
        ));
    }
    if request.host.memory_mib < compute.minimum_memory_mib {
        reasons.push(availability(
            AvailabilityCode::InsufficientMemory,
            format!(
                "provider requires {} MiB memory; host observation has {} MiB",
                compute.minimum_memory_mib, request.host.memory_mib
            ),
        ));
    }
    if request.host.storage_mib < compute.minimum_storage_mib {
        reasons.push(availability(
            AvailabilityCode::InsufficientStorage,
            format!(
                "provider requires {} MiB storage; host observation has {} MiB",
                compute.minimum_storage_mib, request.host.storage_mib
            ),
        ));
    }
    if compute.gpu == RequirementLevel::Required && !request.host.gpu_available {
        reasons.push(availability(
            AvailabilityCode::GpuUnavailable,
            "provider requires a GPU but the host observation has none",
        ));
    }
    if compute.network == RequirementLevel::Required && !request.host.network_available {
        reasons.push(availability(
            AvailabilityCode::NetworkUnavailable,
            "provider requires network access but the host observation has none",
        ));
    }
    if request.offline
        && (compute.network != RequirementLevel::Forbidden
            || registration
                .capability
                .behavior
                .side_effects
                .contains(&SideEffect::Network))
    {
        reasons.push(availability(
            AvailabilityCode::OfflineIncompatible,
            "provider declares network use and cannot receive an offline lock",
        ));
    }

    let allowed = request
        .allowed_side_effects
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for effect in &registration.capability.behavior.side_effects {
        if !allowed.contains(effect) {
            reasons.push(availability(
                AvailabilityCode::SideEffectDenied,
                format!("provider side effect {effect:?} is not authorized"),
            ));
        }
    }

    match (reasons.is_empty(), executable_sha256) {
        (true, Some(executable_sha256)) => Ok((
            executable_sha256,
            ComponentInventory {
                tools,
                codecs,
                models,
            },
        )),
        _ => Err(reasons),
    }
}

fn executable_identity(path: &Path) -> std::result::Result<String, AvailabilityReason> {
    let metadata = fs::metadata(path).map_err(|_| {
        availability(
            AvailabilityCode::ExecutableUnavailable,
            "registered executable is unavailable",
        )
    })?;
    if !metadata.is_file() {
        return Err(availability(
            AvailabilityCode::ExecutableNotRegularFile,
            "registered executable is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(availability(
                AvailabilityCode::ExecutableNotRunnable,
                "registered executable has no execute permission",
            ));
        }
    }
    hash_file(path).map_err(|_| {
        availability(
            AvailabilityCode::ExecutableUnavailable,
            "registered executable could not be read for identity",
        )
    })
}

fn match_components(
    requirements: &[ComponentRequirement],
    inventory: &[ComponentIdentity],
    kind: &str,
    reasons: &mut Vec<AvailabilityReason>,
) -> Vec<ComponentIdentity> {
    let mut matched = Vec::new();
    for requirement in requirements {
        let Some(component) = inventory.iter().find(|value| value.id == requirement.id) else {
            reasons.push(availability(
                AvailabilityCode::ComponentMissing,
                format!("required {kind} {} is unavailable", requirement.id),
            ));
            continue;
        };
        let version_matches = VersionReq::parse(&requirement.version_requirement)
            .ok()
            .zip(Version::parse(&component.version).ok())
            .is_some_and(|(requirement, version)| requirement.matches(&version));
        if !version_matches {
            reasons.push(availability(
                AvailabilityCode::ComponentVersionMismatch,
                format!(
                    "{kind} {} version {} does not satisfy {}",
                    component.id, component.version, requirement.version_requirement
                ),
            ));
            continue;
        }
        if requirement
            .sha256
            .as_ref()
            .is_some_and(|required| component.sha256.as_ref() != Some(required))
        {
            reasons.push(availability(
                AvailabilityCode::ComponentDigestMismatch,
                format!("{kind} {} digest does not match", component.id),
            ));
            continue;
        }
        matched.push(component.clone());
    }
    matched.sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
    matched
}

fn availability(code: AvailabilityCode, detail: impl Into<String>) -> AvailabilityReason {
    AvailabilityReason {
        code,
        detail: detail.into(),
    }
}

/// Expected filesystem shape for one declared output port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    File,
    Directory,
}

/// Caller-owned path binding for one provider output port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedProviderOutput {
    pub port: String,
    pub relative_path: PathBuf,
    pub kind: ArtifactKind,
}

/// Hard runtime and artifact bounds for one local invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderExecutionLimits {
    pub timeout: Duration,
    pub termination_grace_period: Duration,
    pub maximum_stdout_bytes: u64,
    pub maximum_stderr_bytes: u64,
    pub maximum_artifact_files: u64,
    pub maximum_artifact_bytes: u64,
}

/// Serializable bounds retained in execution evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionBounds {
    pub timeout_milliseconds: u64,
    pub termination_grace_milliseconds: u64,
    pub maximum_stdout_bytes: u64,
    pub maximum_stderr_bytes: u64,
    pub maximum_artifact_files: u64,
    pub maximum_artifact_bytes: u64,
}

impl From<ProviderExecutionLimits> for ProviderExecutionBounds {
    fn from(limits: ProviderExecutionLimits) -> Self {
        Self {
            timeout_milliseconds: duration_milliseconds(limits.timeout),
            termination_grace_milliseconds: duration_milliseconds(limits.termination_grace_period),
            maximum_stdout_bytes: limits.maximum_stdout_bytes,
            maximum_stderr_bytes: limits.maximum_stderr_bytes,
            maximum_artifact_files: limits.maximum_artifact_files,
            maximum_artifact_bytes: limits.maximum_artifact_bytes,
        }
    }
}

impl ProviderExecutionLimits {
    fn validate(self) -> Result<()> {
        if self.timeout.is_zero() {
            return Err(invalid("provider timeout must be greater than zero"));
        }
        if self.maximum_stdout_bytes == 0
            || self.maximum_stderr_bytes == 0
            || self.maximum_artifact_files == 0
            || self.maximum_artifact_bytes == 0
        {
            return Err(invalid(
                "provider capture and artifact limits must be greater than zero",
            ));
        }
        if self.timeout.subsec_nanos() % 1_000_000 != 0
            || self.termination_grace_period.subsec_nanos() % 1_000_000 != 0
        {
            return Err(invalid(
                "provider timeout and termination grace must use millisecond precision",
            ));
        }
        if usize::try_from(self.maximum_stdout_bytes).is_err()
            || usize::try_from(self.maximum_stderr_bytes).is_err()
        {
            return Err(invalid(
                "provider capture limits exceed this platform's addressable size",
            ));
        }
        Ok(())
    }
}

/// One direct-argv invocation. Arguments and redaction values are never copied
/// into the execution report.
#[derive(Debug, Clone)]
pub struct ProviderExecutionRequest {
    pub arguments: Vec<OsString>,
    pub working_directory: PathBuf,
    pub output_directory: PathBuf,
    pub expected_outputs: Vec<ExpectedProviderOutput>,
    pub limits: ProviderExecutionLimits,
    pub sensitive_values: Vec<String>,
}

/// Captured stream identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    Stdout,
    Stderr,
}

/// Bounded, redacted process diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedDiagnostic {
    pub stream: StreamKind,
    pub retained_text: String,
    pub total_bytes: u64,
    pub truncated: bool,
    pub redacted: bool,
}

/// Validated immutable output identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactObservation {
    pub port: String,
    pub relative_path: String,
    pub kind: ArtifactKind,
    pub file_count: u64,
    pub byte_count: u64,
    pub sha256: String,
}

/// High-level lifecycle observation emitted during an invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderEventKind {
    ExecutionStarted,
    CancellationObserved,
    TimeoutExceeded,
    CaptureLimitExceeded,
    ArtifactLimitExceeded,
    ProcessExited,
    OutputValidationStarted,
    OutputValidated,
    ExecutionSucceeded,
    ExecutionFailed,
}

/// Ordered provider event, scoped to one exact lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvent {
    pub schema: String,
    pub sequence: u64,
    pub observed_at: String,
    pub provider_lock_sha256: String,
    pub kind: ProviderEventKind,
}

impl ProviderEvent {
    fn new(sequence: u64, provider_lock_sha256: &str, kind: ProviderEventKind) -> Self {
        Self {
            schema: PROVIDER_EVENT_SCHEMA_V1.to_owned(),
            sequence,
            observed_at: Utc::now().to_rfc3339(),
            provider_lock_sha256: provider_lock_sha256.to_owned(),
            kind,
        }
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(&self.schema, PROVIDER_EVENT_SCHEMA_V1)?;
        if self.sequence == 0 {
            return Err(invalid("provider event sequence must be at least 1"));
        }
        DateTime::parse_from_rfc3339(&self.observed_at)
            .map_err(|error| invalid(format!("invalid provider event observed_at: {error}")))?;
        require_sha256(&self.provider_lock_sha256, "provider_lock_sha256")
    }
}

/// Process-level termination mechanism, independent from output acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    NaturalExit,
    PreflightRejected,
    SpawnFailed,
    Cancelled,
    TimedOut,
    CaptureLimitExceeded,
    ArtifactLimitExceeded,
}

/// Terminal execution outcome after output validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderExecutionOutcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    CaptureLimitExceeded,
    ArtifactLimitExceeded,
    InvalidOutput,
}

/// Stable failure categories suitable for callers and state machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderExecutionFailureCode {
    ImplementationChanged,
    SpawnFailed,
    ExitFailure,
    Cancelled,
    Timeout,
    StdoutLimit,
    StderrLimit,
    ArtifactFileLimit,
    ArtifactByteLimit,
    CaptureReadFailed,
    MissingOutput,
    UnexpectedOutput,
    InvalidOutputType,
    EmptyOutput,
    SymlinkOutput,
    OutputReadFailed,
}

/// Redacted, actionable terminal failure evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionFailure {
    pub code: ProviderExecutionFailureCode,
    pub detail: String,
}

/// Process exit data. A zero exit remains insufficient for success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderTermination {
    pub reason: TerminationReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
}

/// Canonical material covered by `report_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionReportPayload {
    pub provider_lock: ProviderLock,
    pub outcome: ProviderExecutionOutcome,
    pub started_at: String,
    pub finished_at: String,
    pub duration_milliseconds: u64,
    pub bounds: ProviderExecutionBounds,
    pub termination: ProviderTermination,
    pub stdout: CapturedDiagnostic,
    pub stderr: CapturedDiagnostic,
    pub outputs: Vec<ArtifactObservation>,
    pub events: Vec<ProviderEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<ProviderExecutionFailure>,
}

/// Self-validating execution evidence with no argv or environment material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionReport {
    pub schema: String,
    pub algorithm: String,
    pub report_sha256: String,
    pub payload: ProviderExecutionReportPayload,
}

impl ProviderExecutionReport {
    fn new(payload: ProviderExecutionReportPayload) -> Result<Self> {
        validate_report_payload(&payload)?;
        let report_sha256 = canonical_sha256(&payload)?;
        Ok(Self {
            schema: PROVIDER_EXECUTION_REPORT_SCHEMA_V1.to_owned(),
            algorithm: "sha256".to_owned(),
            report_sha256,
            payload,
        })
    }

    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let report: Self = decode_json(input, "provider execution report")?;
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<()> {
        require_schema(&self.schema, PROVIDER_EXECUTION_REPORT_SCHEMA_V1)?;
        if self.algorithm != "sha256" {
            return Err(invalid(format!(
                "unsupported provider execution report algorithm {}; expected sha256",
                self.algorithm
            )));
        }
        require_sha256(&self.report_sha256, "report_sha256")?;
        validate_report_payload(&self.payload)?;
        let expected = canonical_sha256(&self.payload)?;
        if expected != self.report_sha256 {
            return Err(invalid(format!(
                "provider execution report does not match its canonical payload; expected {expected}"
            )));
        }
        Ok(())
    }
}

fn validate_report_payload(payload: &ProviderExecutionReportPayload) -> Result<()> {
    payload.provider_lock.validate()?;
    if payload.bounds.timeout_milliseconds == 0
        || payload.bounds.maximum_stdout_bytes == 0
        || payload.bounds.maximum_stderr_bytes == 0
        || payload.bounds.maximum_artifact_files == 0
        || payload.bounds.maximum_artifact_bytes == 0
    {
        return Err(invalid(
            "execution report bounds must retain non-zero timeout, capture, and artifact limits",
        ));
    }
    let started = DateTime::parse_from_rfc3339(&payload.started_at)
        .map_err(|error| invalid(format!("invalid execution started_at: {error}")))?;
    let finished = DateTime::parse_from_rfc3339(&payload.finished_at)
        .map_err(|error| invalid(format!("invalid execution finished_at: {error}")))?;
    if finished < started {
        return Err(invalid("execution finished_at precedes started_at"));
    }
    if payload.stdout.stream != StreamKind::Stdout || payload.stderr.stream != StreamKind::Stderr {
        return Err(invalid("execution report stream identities are reversed"));
    }
    validate_diagnostic(
        &payload.stdout,
        payload.bounds.maximum_stdout_bytes,
        "stdout",
    )?;
    validate_diagnostic(
        &payload.stderr,
        payload.bounds.maximum_stderr_bytes,
        "stderr",
    )?;
    if payload.events.is_empty() {
        return Err(invalid("execution report requires lifecycle events"));
    }
    let mut previous_event_time = started;
    for (index, event) in payload.events.iter().enumerate() {
        event.validate()?;
        let event_time = DateTime::parse_from_rfc3339(&event.observed_at)
            .expect("provider event timestamp was validated");
        if event_time < previous_event_time || event_time > finished {
            return Err(invalid(
                "provider event timestamps must be ordered within the execution interval",
            ));
        }
        previous_event_time = event_time;
        let expected_sequence = u64::try_from(index + 1)
            .map_err(|_| invalid("provider event count exceeds the v1 contract limit"))?;
        if event.sequence != expected_sequence {
            return Err(invalid("provider event sequence is not contiguous"));
        }
        if event.provider_lock_sha256 != payload.provider_lock.lock_sha256 {
            return Err(invalid(
                "provider event references a different provider lock",
            ));
        }
    }
    let mut ports = BTreeSet::new();
    for output in &payload.outputs {
        require_token(&output.port, "output port")?;
        require_nonempty(&output.relative_path, "output relative_path")?;
        let relative_path = Path::new(&output.relative_path);
        validate_relative_path(relative_path)?;
        let normalized =
            normalized_relative_path(relative_path).map_err(|failure| invalid(failure.detail))?;
        if normalized != output.relative_path {
            return Err(invalid(
                "execution report output paths must use normalized forward slashes",
            ));
        }
        require_sha256(&output.sha256, "output sha256")?;
        if output.file_count == 0 || output.byte_count == 0 {
            return Err(invalid("accepted provider outputs must be non-empty"));
        }
        if !ports.insert(output.port.as_str()) {
            return Err(invalid("execution report contains a duplicate output port"));
        }
    }
    if !payload
        .outputs
        .windows(2)
        .all(|pair| pair[0].port < pair[1].port)
    {
        return Err(invalid("execution report outputs must be sorted by port"));
    }
    let terminal_event = payload
        .events
        .last()
        .map(|event| event.kind)
        .expect("execution report event emptiness was checked");
    if let Some(failure) = &payload.failure {
        require_nonempty(&failure.detail, "execution failure detail")?;
    }
    if payload.termination.exit_code.is_some() && payload.termination.signal.is_some() {
        return Err(invalid(
            "provider termination cannot contain both an exit code and a signal",
        ));
    }
    match payload.outcome {
        ProviderExecutionOutcome::Succeeded => {
            if payload.failure.is_some()
                || payload.termination.reason != TerminationReason::NaturalExit
                || payload.termination.exit_code != Some(0)
                || terminal_event != ProviderEventKind::ExecutionSucceeded
            {
                return Err(invalid(
                    "successful execution report has inconsistent terminal evidence",
                ));
            }
        }
        ProviderExecutionOutcome::Failed => {
            let Some(failure) = &payload.failure else {
                return Err(invalid("unsuccessful execution report requires a failure"));
            };
            let consistent = match payload.termination.reason {
                TerminationReason::PreflightRejected => {
                    failure.code == ProviderExecutionFailureCode::ImplementationChanged
                }
                TerminationReason::SpawnFailed => {
                    failure.code == ProviderExecutionFailureCode::SpawnFailed
                }
                TerminationReason::NaturalExit => matches!(
                    failure.code,
                    ProviderExecutionFailureCode::ExitFailure
                        | ProviderExecutionFailureCode::CaptureReadFailed
                ),
                _ => false,
            };
            if !consistent || terminal_event != ProviderEventKind::ExecutionFailed {
                return Err(invalid(
                    "failed execution report has inconsistent terminal evidence",
                ));
            }
        }
        ProviderExecutionOutcome::Cancelled => validate_bounded_failure(
            payload,
            TerminationReason::Cancelled,
            &[ProviderExecutionFailureCode::Cancelled],
            terminal_event,
        )?,
        ProviderExecutionOutcome::TimedOut => validate_bounded_failure(
            payload,
            TerminationReason::TimedOut,
            &[ProviderExecutionFailureCode::Timeout],
            terminal_event,
        )?,
        ProviderExecutionOutcome::CaptureLimitExceeded => validate_bounded_failure(
            payload,
            TerminationReason::CaptureLimitExceeded,
            &[
                ProviderExecutionFailureCode::StdoutLimit,
                ProviderExecutionFailureCode::StderrLimit,
            ],
            terminal_event,
        )?,
        ProviderExecutionOutcome::ArtifactLimitExceeded => {
            let Some(failure) = &payload.failure else {
                return Err(invalid("unsuccessful execution report requires a failure"));
            };
            if !matches!(
                payload.termination.reason,
                TerminationReason::ArtifactLimitExceeded | TerminationReason::NaturalExit
            ) || !matches!(
                failure.code,
                ProviderExecutionFailureCode::ArtifactFileLimit
                    | ProviderExecutionFailureCode::ArtifactByteLimit
            ) || terminal_event != ProviderEventKind::ExecutionFailed
            {
                return Err(invalid(
                    "artifact-limit report has inconsistent terminal evidence",
                ));
            }
        }
        ProviderExecutionOutcome::InvalidOutput => {
            let Some(failure) = &payload.failure else {
                return Err(invalid("unsuccessful execution report requires a failure"));
            };
            if payload.termination.reason != TerminationReason::NaturalExit
                || payload.termination.exit_code != Some(0)
                || !matches!(
                    failure.code,
                    ProviderExecutionFailureCode::MissingOutput
                        | ProviderExecutionFailureCode::UnexpectedOutput
                        | ProviderExecutionFailureCode::InvalidOutputType
                        | ProviderExecutionFailureCode::EmptyOutput
                        | ProviderExecutionFailureCode::SymlinkOutput
                        | ProviderExecutionFailureCode::OutputReadFailed
                )
                || terminal_event != ProviderEventKind::ExecutionFailed
            {
                return Err(invalid(
                    "invalid-output report has inconsistent terminal evidence",
                ));
            }
        }
    }
    Ok(())
}

fn validate_diagnostic(
    diagnostic: &CapturedDiagnostic,
    maximum_bytes: u64,
    name: &str,
) -> Result<()> {
    if diagnostic.truncated != (diagnostic.total_bytes > maximum_bytes) {
        return Err(invalid(format!(
            "{name} truncation does not match its applied capture bound"
        )));
    }
    let maximum_rendered_bytes = maximum_bytes.saturating_mul(10);
    if u64::try_from(diagnostic.retained_text.len()).unwrap_or(u64::MAX) > maximum_rendered_bytes {
        return Err(invalid(format!(
            "{name} retained diagnostic text exceeds the redaction-safe bound"
        )));
    }
    Ok(())
}

fn validate_bounded_failure(
    payload: &ProviderExecutionReportPayload,
    reason: TerminationReason,
    codes: &[ProviderExecutionFailureCode],
    terminal_event: ProviderEventKind,
) -> Result<()> {
    let Some(failure) = &payload.failure else {
        return Err(invalid("unsuccessful execution report requires a failure"));
    };
    if payload.termination.reason != reason
        || !codes.contains(&failure.code)
        || terminal_event != ProviderEventKind::ExecutionFailed
    {
        return Err(invalid(
            "bounded execution report has inconsistent terminal evidence",
        ));
    }
    Ok(())
}

struct ReportParts {
    provider_lock: ProviderLock,
    outcome: ProviderExecutionOutcome,
    started_at: String,
    started: Instant,
    bounds: ProviderExecutionBounds,
    termination: ProviderTermination,
    stdout: CapturedDiagnostic,
    stderr: CapturedDiagnostic,
    outputs: Vec<ArtifactObservation>,
    events: Vec<ProviderEvent>,
    failure: Option<ProviderExecutionFailure>,
}

fn build_report(parts: ReportParts) -> Result<ProviderExecutionReport> {
    let duration_milliseconds = duration_milliseconds(parts.started.elapsed());
    ProviderExecutionReport::new(ProviderExecutionReportPayload {
        provider_lock: parts.provider_lock,
        outcome: parts.outcome,
        started_at: parts.started_at,
        finished_at: Utc::now().to_rfc3339(),
        duration_milliseconds,
        bounds: parts.bounds,
        termination: parts.termination,
        stdout: parts.stdout,
        stderr: parts.stderr,
        outputs: parts.outputs,
        events: parts.events,
        failure: parts.failure,
    })
}

fn duration_milliseconds(duration: Duration) -> u64 {
    let milliseconds = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    if duration.is_zero() {
        0
    } else {
        milliseconds.max(1)
    }
}

fn emit_event<F>(
    events: &mut Vec<ProviderEvent>,
    provider_lock_sha256: &str,
    kind: ProviderEventKind,
    on_event: &mut F,
) -> Result<()>
where
    F: FnMut(&ProviderEvent),
{
    let sequence = u64::try_from(events.len() + 1)
        .map_err(|_| invalid("provider event count exceeds the v1 contract limit"))?;
    let event = ProviderEvent::new(sequence, provider_lock_sha256, kind);
    on_event(&event);
    events.push(event);
    Ok(())
}

fn validate_execution_request(
    request: &ProviderExecutionRequest,
    capability: &CapabilityDeclaration,
) -> Result<()> {
    request.limits.validate()?;
    if !request.working_directory.is_absolute() || !request.output_directory.is_absolute() {
        return Err(invalid(
            "provider working and output directories must be absolute paths",
        ));
    }
    let working_metadata = fs::metadata(&request.working_directory).map_err(|error| {
        invalid(format!(
            "provider working directory is unavailable: {error}"
        ))
    })?;
    if !working_metadata.is_dir() {
        return Err(invalid("provider working directory is not a directory"));
    }
    let output_metadata = fs::symlink_metadata(&request.output_directory)
        .map_err(|error| invalid(format!("provider output directory is unavailable: {error}")))?;
    if output_metadata.file_type().is_symlink() || !output_metadata.is_dir() {
        return Err(invalid(
            "provider output directory must be a real directory, not a symlink",
        ));
    }
    if fs::read_dir(&request.output_directory)
        .map_err(|error| invalid(format!("provider output directory is unreadable: {error}")))?
        .next()
        .is_some()
    {
        return Err(invalid(
            "provider output directory must be empty before execution",
        ));
    }
    let canonical_working = request.working_directory.canonicalize().map_err(|error| {
        invalid(format!(
            "provider working directory cannot be resolved: {error}"
        ))
    })?;
    let canonical_output = request.output_directory.canonicalize().map_err(|error| {
        invalid(format!(
            "provider output directory cannot be resolved: {error}"
        ))
    })?;
    if canonical_working == canonical_output || canonical_working.starts_with(&canonical_output) {
        return Err(invalid(
            "provider working directory must not be the output directory or its descendant",
        ));
    }

    let mut redactions = BTreeSet::new();
    for value in &request.sensitive_values {
        require_nonempty(value, "sensitive redaction value")?;
        if !redactions.insert(value.as_str()) {
            return Err(invalid("sensitive_values contains a duplicate"));
        }
    }

    let declared = capability
        .outputs
        .iter()
        .map(|port| port.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut bound = BTreeSet::new();
    let mut paths = Vec::new();
    for expected in &request.expected_outputs {
        require_token(&expected.port, "expected output port")?;
        if !declared.contains(expected.port.as_str()) {
            return Err(invalid(format!(
                "expected output port {} is not declared by the capability",
                expected.port
            )));
        }
        if !bound.insert(expected.port.as_str()) {
            return Err(invalid(format!(
                "expected output port {} is bound more than once",
                expected.port
            )));
        }
        validate_relative_path(&expected.relative_path)?;
        if expected.relative_path.to_str().is_none() {
            return Err(invalid("expected output path must be valid UTF-8"));
        }
        if paths.iter().any(|path: &PathBuf| {
            path.starts_with(&expected.relative_path) || expected.relative_path.starts_with(path)
        }) {
            return Err(invalid("expected output paths must not overlap"));
        }
        paths.push(expected.relative_path.clone());
    }
    if bound != declared {
        return Err(invalid(
            "every declared output port must have exactly one expected path binding",
        ));
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(
            "expected output path must be a non-empty confined relative path",
        ));
    }
    if path.components().any(|component| match component {
        Component::Normal(value) => value.to_str().is_none_or(|value| value.contains('\\')),
        _ => true,
    }) {
        return Err(invalid(
            "expected output path must use portable UTF-8 components",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct CaptureBytes {
    retained: Vec<u8>,
    total_bytes: u64,
    truncated: bool,
    read_failed: bool,
}

fn spawn_capture_reader<R>(
    mut reader: R,
    limit: u64,
    exceeded: Arc<AtomicBool>,
) -> thread::JoinHandle<CaptureBytes>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let capacity = usize::try_from(limit.min(64 * 1024)).unwrap_or(64 * 1024);
        let mut retained = Vec::with_capacity(capacity);
        let mut total_bytes = 0_u64;
        let mut read_failed = false;
        let mut buffer = [0_u8; 8192];
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(_) => {
                    read_failed = true;
                    break;
                }
            };
            total_bytes = total_bytes.saturating_add(read as u64);
            let remaining = limit.saturating_sub(retained.len() as u64);
            let keep = usize::try_from(remaining.min(read as u64)).unwrap_or(0);
            retained.extend_from_slice(&buffer[..keep]);
            if total_bytes > limit {
                exceeded.store(true, Ordering::SeqCst);
            }
        }
        CaptureBytes {
            retained,
            total_bytes,
            truncated: total_bytes > limit,
            read_failed,
        }
    })
}

fn join_capture(reader: thread::JoinHandle<CaptureBytes>) -> Result<CaptureBytes> {
    reader.join().map_err(|_| {
        Error::new(
            ErrorCategory::Internal,
            "provider diagnostic reader panicked",
        )
    })
}

fn empty_diagnostic(stream: StreamKind) -> CapturedDiagnostic {
    CapturedDiagnostic {
        stream,
        retained_text: String::new(),
        total_bytes: 0,
        truncated: false,
        redacted: false,
    }
}

fn redact_capture(
    stream: StreamKind,
    capture: &CaptureBytes,
    sensitive_values: &[String],
) -> CapturedDiagnostic {
    let mut retained_text = String::from_utf8_lossy(&capture.retained).into_owned();
    let mut redacted = false;
    for sensitive in sensitive_values {
        if retained_text.contains(sensitive) {
            retained_text = retained_text.replace(sensitive, "[REDACTED]");
            redacted = true;
        }
    }
    CapturedDiagnostic {
        stream,
        retained_text,
        total_bytes: capture.total_bytes,
        truncated: capture.truncated,
        redacted,
    }
}

#[derive(Debug, Clone, Copy)]
enum StopTrigger {
    Cancelled,
    TimedOut,
    StdoutLimit,
    StderrLimit,
    ArtifactFileLimit,
    ArtifactByteLimit,
}

impl StopTrigger {
    fn evidence(
        self,
    ) -> (
        ProviderEventKind,
        ProviderExecutionOutcome,
        TerminationReason,
        ProviderExecutionFailureCode,
        &'static str,
    ) {
        match self {
            Self::Cancelled => (
                ProviderEventKind::CancellationObserved,
                ProviderExecutionOutcome::Cancelled,
                TerminationReason::Cancelled,
                ProviderExecutionFailureCode::Cancelled,
                "provider execution was cancelled",
            ),
            Self::TimedOut => (
                ProviderEventKind::TimeoutExceeded,
                ProviderExecutionOutcome::TimedOut,
                TerminationReason::TimedOut,
                ProviderExecutionFailureCode::Timeout,
                "provider exceeded its wall-clock timeout",
            ),
            Self::StdoutLimit => (
                ProviderEventKind::CaptureLimitExceeded,
                ProviderExecutionOutcome::CaptureLimitExceeded,
                TerminationReason::CaptureLimitExceeded,
                ProviderExecutionFailureCode::StdoutLimit,
                "provider stdout exceeded its capture limit",
            ),
            Self::StderrLimit => (
                ProviderEventKind::CaptureLimitExceeded,
                ProviderExecutionOutcome::CaptureLimitExceeded,
                TerminationReason::CaptureLimitExceeded,
                ProviderExecutionFailureCode::StderrLimit,
                "provider stderr exceeded its capture limit",
            ),
            Self::ArtifactFileLimit => (
                ProviderEventKind::ArtifactLimitExceeded,
                ProviderExecutionOutcome::ArtifactLimitExceeded,
                TerminationReason::ArtifactLimitExceeded,
                ProviderExecutionFailureCode::ArtifactFileLimit,
                "provider outputs exceeded the artifact file-count limit",
            ),
            Self::ArtifactByteLimit => (
                ProviderEventKind::ArtifactLimitExceeded,
                ProviderExecutionOutcome::ArtifactLimitExceeded,
                TerminationReason::ArtifactLimitExceeded,
                ProviderExecutionFailureCode::ArtifactByteLimit,
                "provider outputs exceeded the artifact byte limit",
            ),
        }
    }
}

fn observe_artifact_limit(
    output_directory: &Path,
    maximum_files: u64,
    maximum_bytes: u64,
) -> Option<StopTrigger> {
    let mut files = 0_u64;
    let mut bytes = 0_u64;
    for entry in WalkDir::new(output_directory).min_depth(1) {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() {
            files = files.saturating_add(1);
            bytes = bytes.saturating_add(metadata.len());
            if files > maximum_files {
                return Some(StopTrigger::ArtifactFileLimit);
            }
            if bytes > maximum_bytes {
                return Some(StopTrigger::ArtifactByteLimit);
            }
        }
    }
    None
}

fn termination(reason: TerminationReason, status: Option<&ExitStatus>) -> ProviderTermination {
    ProviderTermination {
        reason,
        exit_code: status.and_then(ExitStatus::code),
        signal: exit_signal(status),
    }
}

#[cfg(unix)]
fn exit_signal(status: Option<&ExitStatus>) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    status.and_then(|status| status.signal())
}

#[cfg(not(unix))]
fn exit_signal(_status: Option<&ExitStatus>) -> Option<i32> {
    None
}

#[cfg(unix)]
fn terminate_process_tree(child: &mut Child, grace: Duration) -> io::Result<Option<ExitStatus>> {
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let group = Pid::from_raw(child.id() as i32);
    match killpg(group, Signal::SIGTERM) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => return Err(io::Error::other(error)),
    }
    let deadline = Instant::now().checked_add(grace).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "provider termination grace is too large",
        )
    })?;
    let mut status = None;
    while Instant::now() < deadline {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if matches!(killpg(group, None), Err(Errno::ESRCH)) {
            if status.is_none() {
                child.kill()?;
                status = Some(child.wait()?);
            }
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(10));
    }
    match killpg(group, Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => return Err(io::Error::other(error)),
    }
    if status.is_none() {
        if child.try_wait()?.is_none() {
            child.kill()?;
        }
        status = Some(child.wait()?);
    }
    Ok(status)
}

#[cfg(not(unix))]
fn terminate_process_tree(child: &mut Child, _grace: Duration) -> io::Result<Option<ExitStatus>> {
    child.kill()?;
    child.wait().map(Some)
}

#[cfg(unix)]
fn cleanup_process_group(process_id: u32, grace: Duration) {
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let group = Pid::from_raw(process_id as i32);
    if killpg(group, Signal::SIGTERM).is_err() {
        return;
    }
    let Some(deadline) = Instant::now().checked_add(grace) else {
        let _ = killpg(group, Signal::SIGKILL);
        return;
    };
    while Instant::now() < deadline {
        if matches!(killpg(group, None), Err(Errno::ESRCH)) {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = killpg(group, Signal::SIGKILL);
}

#[cfg(not(unix))]
fn cleanup_process_group(_process_id: u32, _grace: Duration) {}

#[derive(Debug, Serialize)]
struct DirectoryDigestEntry {
    relative_path: String,
    kind: ArtifactKind,
    byte_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

fn validate_outputs(
    request: &ProviderExecutionRequest,
    capability: &CapabilityDeclaration,
) -> std::result::Result<Vec<ArtifactObservation>, ProviderExecutionFailure> {
    let expected_paths = request
        .expected_outputs
        .iter()
        .map(|expected| request.output_directory.join(&expected.relative_path))
        .collect::<Vec<_>>();

    for entry in WalkDir::new(&request.output_directory).min_depth(1) {
        let entry = entry.map_err(|_| {
            execution_failure(
                ProviderExecutionFailureCode::OutputReadFailed,
                "provider output directory could not be traversed",
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(path).map_err(|_| {
            execution_failure(
                ProviderExecutionFailureCode::OutputReadFailed,
                "provider output metadata could not be read",
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(execution_failure(
                ProviderExecutionFailureCode::SymlinkOutput,
                "provider output contains a symbolic link",
            ));
        }
        let allowed = expected_paths
            .iter()
            .any(|expected| path.starts_with(expected) || expected.starts_with(path));
        if !allowed {
            let relative = path
                .strip_prefix(&request.output_directory)
                .ok()
                .and_then(Path::to_str)
                .unwrap_or("<unrepresentable>");
            return Err(execution_failure(
                ProviderExecutionFailureCode::UnexpectedOutput,
                format!("provider created unexpected output {relative}"),
            ));
        }
    }

    let declarations = capability
        .outputs
        .iter()
        .map(|port| (port.name.as_str(), port))
        .collect::<BTreeMap<_, _>>();
    let mut observations = Vec::new();
    let mut total_files = 0_u64;
    let mut total_bytes = 0_u64;
    for expected in &request.expected_outputs {
        let declaration = declarations
            .get(expected.port.as_str())
            .expect("execution request validation binds declared output ports");
        let path = request.output_directory.join(&expected.relative_path);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && declaration.cardinality == ArtifactCardinality::Optional =>
            {
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(execution_failure(
                    ProviderExecutionFailureCode::MissingOutput,
                    format!("provider did not create output port {}", expected.port),
                ));
            }
            Err(_) => {
                return Err(execution_failure(
                    ProviderExecutionFailureCode::OutputReadFailed,
                    format!("output port {} metadata could not be read", expected.port),
                ));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(execution_failure(
                ProviderExecutionFailureCode::SymlinkOutput,
                format!("output port {} is a symbolic link", expected.port),
            ));
        }
        let kind_matches = match expected.kind {
            ArtifactKind::File => metadata.is_file(),
            ArtifactKind::Directory => metadata.is_dir(),
        };
        if !kind_matches {
            return Err(execution_failure(
                ProviderExecutionFailureCode::InvalidOutputType,
                format!(
                    "output port {} has the wrong filesystem type",
                    expected.port
                ),
            ));
        }

        let (file_count, byte_count, sha256) = match expected.kind {
            ArtifactKind::File => {
                let bytes = metadata.len();
                if bytes == 0 {
                    return Err(execution_failure(
                        ProviderExecutionFailureCode::EmptyOutput,
                        format!("output port {} is empty", expected.port),
                    ));
                }
                let digest = hash_file(&path).map_err(|_| {
                    execution_failure(
                        ProviderExecutionFailureCode::OutputReadFailed,
                        format!("output port {} could not be hashed", expected.port),
                    )
                })?;
                (1, bytes, digest)
            }
            ArtifactKind::Directory => hash_directory(&path, &expected.port)?,
        };
        total_files = total_files.saturating_add(file_count);
        total_bytes = total_bytes.saturating_add(byte_count);
        if total_files > request.limits.maximum_artifact_files {
            return Err(execution_failure(
                ProviderExecutionFailureCode::ArtifactFileLimit,
                "validated outputs exceed the artifact file-count limit",
            ));
        }
        if total_bytes > request.limits.maximum_artifact_bytes {
            return Err(execution_failure(
                ProviderExecutionFailureCode::ArtifactByteLimit,
                "validated outputs exceed the artifact byte limit",
            ));
        }
        observations.push(ArtifactObservation {
            port: expected.port.clone(),
            relative_path: normalized_relative_path(&expected.relative_path)?,
            kind: expected.kind,
            file_count,
            byte_count,
            sha256,
        });
    }
    observations.sort_by(|left, right| left.port.cmp(&right.port));
    Ok(observations)
}

fn hash_directory(
    root: &Path,
    port: &str,
) -> std::result::Result<(u64, u64, String), ProviderExecutionFailure> {
    let mut entries = Vec::new();
    let mut file_count = 0_u64;
    let mut byte_count = 0_u64;
    for entry in WalkDir::new(root).min_depth(1).sort_by_file_name() {
        let entry = entry.map_err(|_| {
            execution_failure(
                ProviderExecutionFailureCode::OutputReadFailed,
                format!("output port {port} could not be traversed"),
            )
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| {
            execution_failure(
                ProviderExecutionFailureCode::OutputReadFailed,
                format!("output port {port} metadata could not be read"),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(execution_failure(
                ProviderExecutionFailureCode::SymlinkOutput,
                format!("output port {port} contains a symbolic link"),
            ));
        }
        let relative = entry.path().strip_prefix(root).map_err(|_| {
            execution_failure(
                ProviderExecutionFailureCode::OutputReadFailed,
                format!("output port {port} path escaped its root"),
            )
        })?;
        let relative_path = normalized_relative_path(relative)?;
        if metadata.is_file() {
            let size = metadata.len();
            let digest = hash_file(entry.path()).map_err(|_| {
                execution_failure(
                    ProviderExecutionFailureCode::OutputReadFailed,
                    format!("output port {port} contains an unreadable file"),
                )
            })?;
            file_count = file_count.saturating_add(1);
            byte_count = byte_count.saturating_add(size);
            entries.push(DirectoryDigestEntry {
                relative_path,
                kind: ArtifactKind::File,
                byte_count: size,
                sha256: Some(digest),
            });
        } else if metadata.is_dir() {
            entries.push(DirectoryDigestEntry {
                relative_path,
                kind: ArtifactKind::Directory,
                byte_count: 0,
                sha256: None,
            });
        } else {
            return Err(execution_failure(
                ProviderExecutionFailureCode::InvalidOutputType,
                format!("output port {port} contains an unsupported filesystem entry"),
            ));
        }
    }
    if file_count == 0 || byte_count == 0 {
        return Err(execution_failure(
            ProviderExecutionFailureCode::EmptyOutput,
            format!("output port {port} is empty"),
        ));
    }
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let digest = canonical_sha256(&entries).map_err(|error| {
        execution_failure(
            ProviderExecutionFailureCode::OutputReadFailed,
            format!("output port {port} identity failed: {error}"),
        )
    })?;
    Ok((file_count, byte_count, digest))
}

fn normalized_relative_path(path: &Path) -> std::result::Result<String, ProviderExecutionFailure> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(execution_failure(
                ProviderExecutionFailureCode::OutputReadFailed,
                "provider output contains a non-portable relative path",
            ));
        };
        let value = value
            .to_str()
            .filter(|value| !value.contains('\\'))
            .ok_or_else(|| {
                execution_failure(
                    ProviderExecutionFailureCode::OutputReadFailed,
                    "provider output contains a non-portable relative path",
                )
            })?;
        components.push(value);
    }
    Ok(components.join("/"))
}

fn cleanup_output_directory(output_directory: &Path) -> Result<()> {
    let root_metadata = match fs::symlink_metadata(output_directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(io_failure(format!(
                "failed to inspect provider output cleanup root: {error}"
            )));
        }
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(io_failure(
            "refusing to clean a provider output root that is not a real directory",
        ));
    }
    for entry in fs::read_dir(output_directory).map_err(|error| {
        io_failure(format!(
            "failed to inspect provider output cleanup directory: {error}"
        ))
    })? {
        let entry = entry.map_err(|error| {
            io_failure(format!("failed to inspect provider output entry: {error}"))
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            io_failure(format!(
                "failed to inspect provider output for cleanup: {error}"
            ))
        })?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(&path).map_err(|error| {
                io_failure(format!(
                    "failed to remove partial provider directory: {error}"
                ))
            })?;
        } else {
            fs::remove_file(&path).map_err(|error| {
                io_failure(format!("failed to remove partial provider output: {error}"))
            })?;
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut source = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn execution_failure(
    code: ProviderExecutionFailureCode,
    detail: impl Into<String>,
) -> ProviderExecutionFailure {
    ProviderExecutionFailure {
        code,
        detail: detail.into(),
    }
}

fn redact_execution_failure(
    mut failure: ProviderExecutionFailure,
    sensitive_values: &[String],
) -> ProviderExecutionFailure {
    for sensitive in sensitive_values {
        failure.detail = failure.detail.replace(sensitive, "[REDACTED]");
    }
    failure
}

fn execution_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Execution, message)
}

fn io_failure(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Io, message)
}
