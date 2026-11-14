//! Explicit offline toolchain inventories and read-only setup plans.
//!
//! Files are hashed at caller-supplied paths. Version, flags, build features and
//! hardware remain supplied observations, never invented process-probe results.
//! Readiness means inventory consistency; it is not native qualification or
//! permission to register, execute, install, download or publish anything.
//! Explicit `probe_tools` is a separate operation that runs pinned diagnostic
//! queries; it does not change the offline inspection or setup-plan boundary.

mod inspect;
mod media_setup;
mod preflight;
mod probe_parse;
mod probe_runtime;
mod probe_types;
mod registration;
mod types;

pub use inspect::inspect_profile;
pub use media_setup::{
    AllenkSetupOptions, CaptionSetupOptions, CoreMediaSetupPreparation, CoreMediaSetupRequest,
    DestinationVideoSetupOptions, FinalLosslessQcSetupOptions, FinalVideoAudioQcSetupOptions,
    FinishingMediaOptions, FrameExtractionSetupOptions, LosslessAudioSetupOptions,
    MEDIA_SETUP_PREPARATION_SCHEMA, MEDIA_SETUP_REQUEST_SCHEMA, MediaImportOptions,
    MediaSetupPreparation, MediaSetupRequest, MediaSetupSelection, MetadataRemuxSetupOptions,
    MetadataSubtitleSetup, PublishingSetupChoice, PublishingSetupPreparation,
    PublishingSetupRequest, PublishingSetupStep, RifeSetupOptions, UpscaylSetupOptions,
    WorkflowSetupPreparation, WorkflowSetupRequest, WorkflowSetupStep, prepare_core_media_setup,
    prepare_media_setup, prepare_publishing_setup, prepare_workflow_setup, workflow_bundle,
};
pub(crate) use preflight::require_toolchain_preflight;
pub use preflight::{
    AudioInspectionToolchainBinding, TOOLCHAIN_PREFLIGHT_CONFIGURATION_SCHEMA,
    TOOLCHAIN_PREFLIGHT_REPORT_SCHEMA, ToolchainPreflightConfiguration, ToolchainPreflightReport,
    preflight_toolchain,
};
pub use probe_runtime::probe_tools;
pub use probe_types::*;
pub use registration::{
    AUDIO_INSPECTION_REGISTRATION_SCHEMA, AudioInspectionRegistrationRequest,
    TOOLCHAIN_REGISTRATION_PREPARATION_SCHEMA, ToolchainRegistrationAdapter,
    ToolchainRegistrationFile, ToolchainRegistrationPreparation,
    prepare_audio_inspection_registration,
};
pub use types::*;
