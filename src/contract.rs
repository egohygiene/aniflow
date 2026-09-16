use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{Error, ErrorCategory, Result};

pub const MACHINE_SCHEMA_VERSION: u32 = 1;

/// Public CLI operations represented by the machine contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CommandName {
    Doctor,
    Inspect,
    Plan,
    PlanV3,
    Run,
    RunV3,
    Resume,
    ResumeV3,
    Status,
    StatusV3,
    SegmentPlan,
    SegmentRun,
    SegmentResume,
    SegmentReconstruct,
}

impl fmt::Display for CommandName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Doctor => "doctor",
            Self::Inspect => "inspect",
            Self::Plan => "plan",
            Self::PlanV3 => "plan_v3",
            Self::Run => "run",
            Self::RunV3 => "run_v3",
            Self::Resume => "resume",
            Self::ResumeV3 => "resume_v3",
            Self::Status => "status",
            Self::StatusV3 => "status_v3",
            Self::SegmentPlan => "segment_plan",
            Self::SegmentRun => "segment_run",
            Self::SegmentResume => "segment_resume",
            Self::SegmentReconstruct => "segment_reconstruct",
        };
        formatter.write_str(name)
    }
}

/// Serializable representation of a typed public error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ErrorReport {
    pub category: ErrorCategory,
    pub message: String,
}

impl From<&Error> for ErrorReport {
    fn from(error: &Error) -> Self {
        Self {
            category: error.category(),
            message: error.message().to_owned(),
        }
    }
}

/// Success or failure carried by a machine-readable command envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum MachineOutcome<T> {
    Success {
        result: T,
    },
    Error {
        error: ErrorReport,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<T>,
    },
}

/// Versioned machine-readable result shared by the CLI and external consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MachineEnvelope<T> {
    pub schema_version: u32,
    pub command: CommandName,
    #[serde(flatten)]
    pub outcome: MachineOutcome<T>,
}

impl<T> MachineEnvelope<T> {
    #[must_use]
    pub const fn success(command: CommandName, result: T) -> Self {
        Self {
            schema_version: MACHINE_SCHEMA_VERSION,
            command,
            outcome: MachineOutcome::Success { result },
        }
    }

    #[must_use]
    pub fn failure(command: CommandName, error: &Error, result: Option<T>) -> Self {
        Self {
            schema_version: MACHINE_SCHEMA_VERSION,
            command,
            outcome: MachineOutcome::Error {
                error: ErrorReport::from(error),
                result,
            },
        }
    }

    pub fn ensure_supported(&self) -> Result<()> {
        if self.schema_version == MACHINE_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCategory::Configuration,
                format!(
                    "unsupported machine contract version {}; expected {}",
                    self.schema_version, MACHINE_SCHEMA_VERSION
                ),
            ))
        }
    }
}

impl<T> MachineEnvelope<T>
where
    T: DeserializeOwned,
{
    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_slice(input).map_err(|error| {
            Error::new(
                ErrorCategory::Configuration,
                format!("invalid machine contract JSON: {error}"),
            )
        })?;
        let object = value.as_object().ok_or_else(|| {
            Error::new(
                ErrorCategory::Configuration,
                "machine contract must be a JSON object",
            )
        })?;
        if let Some(field) = object.keys().find(|field| {
            !matches!(
                field.as_str(),
                "schema_version" | "command" | "status" | "result" | "error"
            )
        }) {
            return Err(Error::new(
                ErrorCategory::Configuration,
                format!("machine contract contains unknown field {field}"),
            ));
        }
        if object.get("status").and_then(serde_json::Value::as_str) == Some("success")
            && object.contains_key("error")
        {
            return Err(Error::new(
                ErrorCategory::Configuration,
                "successful machine contracts must not contain an error field",
            ));
        }
        let envelope: Self = serde_json::from_value(value).map_err(|error| {
            Error::new(
                ErrorCategory::Configuration,
                format!("invalid machine contract JSON: {error}"),
            )
        })?;
        envelope.ensure_supported()?;
        Ok(envelope)
    }
}
