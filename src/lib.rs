//! Reusable application facade for aniflow video workflows.
//!
//! The crate-root API is intentionally small and provisional while aniflow is
//! pre-1.0. Internal pipeline, process, state, and workspace representations
//! are not public contracts.

mod command;
mod contract;
mod error;
mod facade;
mod invocation_v3;
mod media;
mod pipeline;
mod pipeline_v3;
mod processor_runtime;
mod provider;
mod provider_runtime;
mod run;
mod run_v3;
mod segmentation;
mod state;
mod state_v3;
mod workspace;
mod workspace_v3;

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
pub use invocation_v3::{
    ARTIFACT_INTEGRITY_VALIDATION_CONTRACT_V1, PROVIDER_INVOCATION_ARGUMENT,
    PROVIDER_INVOCATION_EXECUTION_SEMANTICS_V1, PROVIDER_INVOCATION_SCHEMA_V1,
    ProviderInvocationArtifactBinding, ProviderInvocationRequest, provider_invocation_arguments,
};
pub use media::MediaInspection;
pub use pipeline_v3::{
    ArtifactValidation, AuthoredInputBinding, AuthoredOutputBinding, AuthoredPipelineInput,
    AuthoredPipelineStage, CANONICAL_JSON_SCHEMA_V1, CapabilityRequirement, ExpectedArtifact,
    FinalOutputRequirement, NormalizedPlanningPolicy, PIPELINE_PLAN_SCHEMA_V1,
    PIPELINE_PLANNING_FAILURE_SCHEMA_V1, PIPELINE_V3_CONFIGURATION_SCHEMA,
    PIPELINE_V3_PLAN_SCHEMA_V1, PIPELINE_V3_SCHEMA, PROVIDER_REGISTRATION_SCHEMA_V1,
    PipelineInputBinding, PipelineInputIdentity, PipelineInputKind, PipelinePlanningContext,
    PipelinePlanningDiagnostic, PipelinePlanningDiagnosticCode, PipelinePlanningFailure,
    PipelineV3, PipelineV3Configuration, PipelineV3Plan, PipelineV3PlanPayload,
    PlannedExpectedArtifact, PlannedOutputBinding, PlannedProviderResolutionAttempt,
    ProviderRegistrationDocument, ProviderSelectionIntent, ResolvedPipelinePlan,
    ResolvedPipelineStage, plan_v3, resolve_pipeline_v3,
};
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
    ArtifactContentObservation, ArtifactKind, ArtifactObservation, AvailabilityCode,
    AvailabilityReason, CapturedDiagnostic, ComponentInventory, ExpectedProviderOutput,
    HostResources, PROVIDER_EVENT_SCHEMA_V1, PROVIDER_EXECUTION_REPORT_SCHEMA_V1,
    PROVIDER_LOCK_SCHEMA_V1, ProviderCandidate, ProviderEvent, ProviderEventKind,
    ProviderExecutionBounds, ProviderExecutionFailure, ProviderExecutionFailureCode,
    ProviderExecutionLimits, ProviderExecutionOutcome, ProviderExecutionReport,
    ProviderExecutionReportPayload, ProviderExecutionRequest, ProviderImplementationIdentity,
    ProviderLock, ProviderLockPayload, ProviderRegistration, ProviderRegistry,
    ProviderResolutionAttempt, ProviderResolutionFailure, ProviderResolutionRequest,
    ProviderSelection, ProviderSelectionSource, ProviderTermination, ResolvedProvider, StreamKind,
    TerminationReason, observe_existing_artifact,
};
pub use run_v3::{
    PIPELINE_V3_RUN_OUTCOME_SCHEMA_V1, PIPELINE_V3_RUN_RECOVERY_SCHEMA_V1, PipelineV3Output,
    PipelineV3ProgressState, PipelineV3ResumeRequest, PipelineV3RunOperation, PipelineV3RunOutcome,
    PipelineV3RunProgress, PipelineV3RunRecovery, PipelineV3RunRequest, resume_v3,
    resume_v3_with_progress_and_cancellation, run_v3, run_v3_with_progress_and_cancellation,
    status_v3,
};
pub use segmentation::{
    BoundaryAccuracy, CancellationToken, PlannedSegment, RECONSTRUCT_CAPABILITY_ID_V1,
    RECONSTRUCTION_REPORT_SCHEMA_V1, ReconstructionReport, SEGMENT_CAPABILITY_ID_V1,
    SEGMENT_MANIFEST_SCHEMA_V1, SEGMENT_PLAN_SCHEMA_V1, SegmentManifest, SegmentMode,
    SegmentOutcome, SegmentPlan, SegmentProgress, SegmentRecord, SegmentRequest,
};
pub use state_v3::{
    ArtifactEvidence, CompatibilityDecision, CompatibilityReason, CompatibilityReasonCode,
    EvidenceReference, PIPELINE_V3_RUN_MANIFEST_SCHEMA_V1, PIPELINE_V3_STAGE_CHECKPOINT_SCHEMA_V1,
    PipelineV3RunManifest, PipelineV3RunManifestPayload, PipelineV3RunState, PipelineV3StageState,
    StageCheckpoint, StageCheckpointPayload, StageCheckpointReference, StageRunRecord,
    ValidationEvidence, append_run_manifest, load_latest_run_manifest, load_run_manifest_chain,
    load_stage_checkpoint, publish_stage_checkpoint,
};
pub use workspace_v3::PipelineV3Workspace;
