//! Closed requests and retained evidence for explicit, bounded tool probes.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::probe_parse::{parse_probe_observation, probe_commands};
use super::types::{ToolchainObservation, ToolchainStatus};
use crate::{Error, ErrorCategory, Result};

pub const TOOLCHAIN_PROBE_CONFIGURATION_SCHEMA: &str = "aniflow.toolchain-probe-configuration/v1";
pub const TOOLCHAIN_PROBE_REPORT_SCHEMA: &str = "aniflow.toolchain-probe-report/v1";
const MAXIMUM_REPORT_BYTES: usize = 67_108_864;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeConfiguration {
    pub schema: String,
    pub tools: Vec<ToolchainProbeTool>,
    pub limits: ToolchainProbeLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeTool {
    pub dependency_id: String,
    pub profile: ToolchainProbeProfile,
    pub path: PathBuf,
    pub expected_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller_package_revision: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolchainProbeProfile {
    Ffmpeg,
    Ffprobe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeLimits {
    pub timeout_milliseconds: u64,
    pub total_timeout_milliseconds: u64,
    pub termination_grace_milliseconds: u64,
    pub maximum_stdout_bytes: u64,
    pub maximum_stderr_bytes: u64,
    pub maximum_total_capture_bytes: u64,
    pub maximum_executable_bytes: u64,
    pub maximum_total_hash_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeReport {
    pub schema: String,
    pub request_sha256: String,
    pub configuration: ToolchainProbeConfiguration,
    pub complete: bool,
    pub native_qualification: bool,
    pub tools: Vec<ToolchainProbeToolResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeToolResult {
    pub dependency_id: String,
    pub profile: ToolchainProbeProfile,
    pub status: ToolchainStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_sha256: Option<String>,
    pub commands: Vec<ToolchainProbeCommandResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<ToolchainProbeVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<ToolchainObservation>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeCommandResult {
    pub command_id: String,
    pub arguments: Vec<String>,
    pub outcome: ToolchainProbeCommandOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
    pub stdout: ToolchainProbeCapture,
    pub stderr: ToolchainProbeCapture,
    pub duration_milliseconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainProbeCommandOutcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    OutputLimit,
    SpawnFailed,
    CaptureFailed,
    CleanupFailed,
}

/// The digest covers exactly the retained bytes. `total_bytes` counts observed
/// bytes, and does not claim to count bytes left unread after termination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeCapture {
    pub retained_hex: String,
    pub total_bytes: u64,
    pub truncated: bool,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolchainProbeVersion {
    pub raw_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_semver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalization: Option<ToolchainVersionNormalization>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainVersionNormalization {
    Exact,
    AppendPatchZero,
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCategory::Configuration, message)
}

fn printable(value: &str, maximum: usize, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(invalid(format!("{field} must contain 1..={maximum} printable bytes")));
    }
    Ok(())
}

impl ToolchainProbeConfiguration {
    /// Load only a bounded request document; no executable is opened or run.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_json_slice(&super::types::read_json(path.as_ref())?)
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        let value: Self = super::types::decode_unique_json(bytes)?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != TOOLCHAIN_PROBE_CONFIGURATION_SCHEMA {
            return Err(invalid("unsupported toolchain probe configuration schema"));
        }
        if self.tools.is_empty() || self.tools.len() > 8 {
            return Err(invalid("toolchain probes require 1..=8 explicit tools"));
        }
        let mut ids = BTreeSet::new();
        for tool in &self.tools {
            if tool.dependency_id.len() > 128 {
                return Err(invalid("probe dependency ID exceeds 128 bytes"));
            }
            crate::provider::require_token(&tool.dependency_id, "probe dependency ID")?;
            if !ids.insert(&tool.dependency_id) {
                return Err(invalid("probe dependency IDs must be unique"));
            }
            let path = tool.path.to_str().ok_or_else(|| invalid("probe paths must be UTF-8"))?;
            printable(path, 4096, "probe path")?;
            if !tool.path.is_absolute()
                || path.split(|character| character == '/' || (cfg!(windows) && character == '\\'))
                    .any(|component| matches!(component, "." | ".."))
                || tool.path.components().any(|part| matches!(part, Component::CurDir | Component::ParentDir))
            {
                return Err(invalid("probe paths must be literal absolute paths without traversal"));
            }
            crate::provider::require_sha256(&tool.expected_sha256, "probe executable SHA-256")?;
            if let Some(revision) = &tool.caller_package_revision {
                printable(revision, 256, "caller package revision")?;
            }
        }
        self.limits.validate()
    }

    pub fn sha256(&self) -> Result<String> {
        self.validate()?;
        crate::provider::canonical_sha256(self)
    }
}

impl ToolchainProbeLimits {
    pub fn validate(&self) -> Result<()> {
        for (value, maximum, name) in [
            (self.timeout_milliseconds, 60_000, "command timeout"),
            (self.total_timeout_milliseconds, 600_000, "total timeout"),
            (self.termination_grace_milliseconds, 5_000, "termination grace"),
            (self.maximum_stdout_bytes, 4_194_304, "stdout capture"),
            (self.maximum_stderr_bytes, 4_194_304, "stderr capture"),
            (self.maximum_total_capture_bytes, 16_777_216, "total capture"),
            (self.maximum_executable_bytes, 1_073_741_824, "executable bytes"),
            (self.maximum_total_hash_bytes, 17_179_869_184, "total hash bytes"),
        ] {
            if value == 0 || value > maximum {
                return Err(invalid(format!("probe {name} must be in 1..={maximum}")));
            }
        }
        if self.total_timeout_milliseconds < self.timeout_milliseconds {
            return Err(invalid("total probe timeout cannot be shorter than a command timeout"));
        }
        Ok(())
    }
}

impl ToolchainProbeCapture {
    pub fn from_bytes(raw: &[u8], total_bytes: u64, truncated: bool) -> Self {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut retained_hex = String::with_capacity(raw.len().saturating_mul(2));
        for byte in raw {
            retained_hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
            retained_hex.push(char::from(DIGITS[usize::from(byte & 15)]));
        }
        Self { retained_hex, total_bytes, truncated, sha256: format!("{:x}", Sha256::digest(raw)) }
    }

    pub fn bytes(&self) -> Result<Vec<u8>> {
        if self.retained_hex.len() > 8_388_608 || self.retained_hex.len() % 2 != 0 {
            return Err(invalid("probe capture exceeds its bound or has an odd hex length"));
        }
        let digit = |value: u8| -> Result<u8> {
            match value {
                b'0'..=b'9' => Ok(value - b'0'),
                b'a'..=b'f' => Ok(value - b'a' + 10),
                _ => Err(invalid("probe capture must use lowercase hexadecimal")),
            }
        };
        let mut bytes = Vec::with_capacity(self.retained_hex.len() / 2);
        for pair in self.retained_hex.as_bytes().chunks_exact(2) {
            bytes.push(digit(pair[0])? * 16 + digit(pair[1])?);
        }
        if self.total_bytes < bytes.len() as u64
            || (!self.truncated && self.total_bytes != bytes.len() as u64)
            || self.sha256 != format!("{:x}", Sha256::digest(&bytes))
        {
            return Err(invalid("probe capture length or retained-byte digest does not match"));
        }
        Ok(bytes)
    }

    pub fn text(&self) -> Result<String> {
        String::from_utf8(self.bytes()?).map_err(|_| invalid("probe capture is not UTF-8"))
    }
}

impl ToolchainProbeReport {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        let value: Self = super::types::decode_unique_json_with_limit(bytes, MAXIMUM_REPORT_BYTES)?;
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != TOOLCHAIN_PROBE_REPORT_SCHEMA || self.native_qualification {
            return Err(invalid("unsupported probe report schema or native qualification claim"));
        }
        if self.configuration.sha256()? != self.request_sha256
            || self.tools.len() != self.configuration.tools.len()
        {
            return Err(invalid("probe report does not match its complete request identity"));
        }
        let mut retained_total = 0_u64;
        for (tool, result) in self.configuration.tools.iter().zip(&self.tools) {
            if result.dependency_id != tool.dependency_id || result.profile != tool.profile {
                return Err(invalid("probe results must preserve requested tool order and identity"));
            }
            for digest in [&result.before_sha256, &result.after_sha256].into_iter().flatten() {
                crate::provider::require_sha256(digest, "probe observed executable SHA-256")?;
            }
            if result.diagnostics.len() > 32 {
                return Err(invalid("probe tool exceeds 32 diagnostics"));
            }
            for diagnostic in &result.diagnostics {
                printable(diagnostic, 8192, "probe diagnostic")?;
            }
            let expected = probe_commands(tool.profile);
            if result.commands.len() > expected.len() {
                return Err(invalid("probe result has commands outside its fixed profile"));
            }
            for (command, (id, arguments)) in result.commands.iter().zip(&expected) {
                if command.command_id != *id || command.arguments != *arguments {
                    return Err(invalid("probe result commands must be an exact fixed-profile prefix"));
                }
                if command.exit_code.is_some() && command.signal.is_some() {
                    return Err(invalid("probe command cannot both exit and terminate by signal"));
                }
                if command.signal.is_some_and(|signal| signal <= 0)
                    || (command.outcome == ToolchainProbeCommandOutcome::Succeeded
                        && (command.exit_code != Some(0) || command.signal.is_some()
                            || command.stdout.truncated || command.stderr.truncated))
                {
                    return Err(invalid("probe command success or signal contradicts process facts"));
                }
                if let Some(detail) = &command.detail {
                    printable(detail, 8192, "probe command detail")?;
                }
                for (capture, maximum) in [
                    (&command.stdout, self.configuration.limits.maximum_stdout_bytes),
                    (&command.stderr, self.configuration.limits.maximum_stderr_bytes),
                ] {
                    let length = capture.bytes()?.len() as u64;
                    if length > maximum {
                        return Err(invalid("probe capture exceeds its requested stream bound"));
                    }
                    retained_total = retained_total.checked_add(length)
                        .ok_or_else(|| invalid("probe capture byte total overflow"))?;
                }
            }
            match (&result.observation, &result.version) {
                (Some(observation), Some(version)) => {
                    if result.before_sha256.as_deref() != Some(tool.expected_sha256.as_str())
                        || result.after_sha256.as_deref() != Some(tool.expected_sha256.as_str())
                    {
                        return Err(invalid("parsed probe observations require unchanged pinned executable bytes"));
                    }
                    let parsed = parse_probe_observation(tool, &result.commands, &tool.expected_sha256)?;
                    if *observation != parsed.observation || *version != parsed.version {
                        return Err(invalid("parsed probe facts do not match retained command evidence"));
                    }
                    let expected_status = if version.normalized_semver.is_some() {
                        ToolchainStatus::Installed
                    } else { ToolchainStatus::Unverified };
                    if result.status != expected_status {
                        return Err(invalid("probe status disagrees with its parsed release evidence"));
                    }
                }
                (None, None) if result.status != ToolchainStatus::Installed => {}
                _ => return Err(invalid("probe observations, versions and installed status must be consistent")),
            }
        }
        if retained_total > self.configuration.limits.maximum_total_capture_bytes {
            return Err(invalid("probe captures exceed the requested total retained-byte bound"));
        }
        if self.complete != self.tools.iter().all(|tool| tool.status == ToolchainStatus::Installed) {
            return Err(invalid("probe completion disagrees with tool results"));
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes = crate::provider::canonical_json_bytes(self)?;
        if bytes.len() > MAXIMUM_REPORT_BYTES {
            return Err(invalid("toolchain probe report exceeds 64 MiB"));
        }
        Ok(bytes)
    }
}
