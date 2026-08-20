//! Reusable application facade for aniflow video workflows.
//!
//! The crate-root API is intentionally small and provisional while aniflow is
//! pre-1.0. Internal pipeline, process, state, and workspace representations
//! are not public contracts.

mod command;
mod contract;
mod error;
mod facade;
mod media;
mod pipeline;
mod run;
mod state;
mod workspace;

pub use contract::{
    CommandName, ErrorReport, MACHINE_SCHEMA_VERSION, MachineEnvelope, MachineOutcome,
};
pub use error::{Error, ErrorCategory, Result};
pub use facade::{
    ArtifactStatus, DependencyStatus, DoctorReport, FrameProcessorPlan, PipelinePlan,
    ProgressState, RunOperation, RunOutcome, RunProgress, RunRequest, RunStatus, StageStatus,
    doctor, inspect, plan, resume, resume_with_progress, run, run_with_progress, status,
};
pub use media::MediaInspection;
