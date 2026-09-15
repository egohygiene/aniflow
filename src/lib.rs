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
mod processor_runtime;
mod provider;
mod provider_runtime;
mod run;
mod segmentation;
mod state;
mod workspace;

pub use contract::{
    CommandName, ErrorReport, MACHINE_SCHEMA_VERSION, MachineEnvelope, MachineOutcome,
};
pub use error::{Error, ErrorCategory, Result};
pub use facade::{
    ArtifactStatus, DependencyStatus, DoctorReport, FrameProcessorPlan, PipelinePlan,
    ProgressState, RunOperation, RunOutcome, RunProgress, RunRequest, RunStatus, StageStatus,
    doctor, inspect, plan, plan_segments, reconstruct_segments, reconstruct_segments_with_progress,
    resume, resume_segments, resume_segments_with_progress, resume_with_progress,
    resume_with_progress_and_cancellation, run, run_with_progress,
    run_with_progress_and_cancellation, segment, segment_with_progress, status,
};
pub use media::MediaInspection;
pub use provider::{
    ArtifactCardinality, ArtifactPort, ArtifactRole, BatchingMode,
    COMPATIBILITY_FINGERPRINT_SCHEMA_V1, CancellationMode, CapabilityBehavior,
    CapabilityDeclaration, CapabilityKind, CapabilityReference, CompatibilityFingerprint,
    CompatibilityFingerprintPayload, ComponentIdentity, ComponentRequirement, ComputeRequirements,
    ConfigurationSchemaReference, DeterminismClass, FidelityClass, FingerprintArtifact,
    LifecycleSupport, PROVIDER_CONFIGURATION_SCHEMA_V1, PROVIDER_MANIFEST_SCHEMA_V1, ProgressMode,
    ProvenanceContract, ProvenanceField, ProviderConfiguration, ProviderIdentity, ProviderManifest,
    ProviderReference, ProviderRequirements, RequirementLevel, SideEffect, StreamRole,
    ValidatedOutputFingerprint,
};
pub use provider_runtime::{
    ArtifactKind, ArtifactObservation, AvailabilityCode, AvailabilityReason, CapturedDiagnostic,
    ComponentInventory, ExpectedProviderOutput, HostResources, PROVIDER_EVENT_SCHEMA_V1,
    PROVIDER_EXECUTION_REPORT_SCHEMA_V1, PROVIDER_LOCK_SCHEMA_V1, ProviderCandidate, ProviderEvent,
    ProviderEventKind, ProviderExecutionBounds, ProviderExecutionFailure,
    ProviderExecutionFailureCode, ProviderExecutionLimits, ProviderExecutionOutcome,
    ProviderExecutionReport, ProviderExecutionReportPayload, ProviderExecutionRequest,
    ProviderImplementationIdentity, ProviderLock, ProviderLockPayload, ProviderRegistration,
    ProviderRegistry, ProviderResolutionAttempt, ProviderResolutionFailure,
    ProviderResolutionRequest, ProviderSelection, ProviderSelectionSource, ProviderTermination,
    ResolvedProvider, StreamKind, TerminationReason,
};
pub use segmentation::{
    BoundaryAccuracy, CancellationToken, PlannedSegment, RECONSTRUCT_CAPABILITY_ID_V1,
    RECONSTRUCTION_REPORT_SCHEMA_V1, ReconstructionReport, SEGMENT_CAPABILITY_ID_V1,
    SEGMENT_MANIFEST_SCHEMA_V1, SEGMENT_PLAN_SCHEMA_V1, SegmentManifest, SegmentMode,
    SegmentOutcome, SegmentPlan, SegmentProgress, SegmentRecord, SegmentRequest,
};
