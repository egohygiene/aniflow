//! Explicit offline toolchain inventories and read-only setup plans.
//!
//! Files are hashed at caller-supplied paths. Version, flags, build features and
//! hardware remain supplied observations, never invented process-probe results.
//! Readiness means inventory consistency; it is not native qualification or
//! permission to register, execute, install, download or publish anything.
//! Explicit `probe_tools` is a separate operation that runs pinned diagnostic
//! queries; it does not change the offline inspection or setup-plan boundary.

mod inspect;
mod preflight;
mod probe_parse;
mod probe_runtime;
mod probe_types;
mod registration;
mod types;

pub use inspect::inspect_profile;
pub use preflight::{
    AudioInspectionToolchainBinding, ToolchainPreflightConfiguration,
    ToolchainPreflightReport, TOOLCHAIN_PREFLIGHT_CONFIGURATION_SCHEMA,
    TOOLCHAIN_PREFLIGHT_REPORT_SCHEMA, preflight_toolchain,
};
pub(crate) use preflight::require_toolchain_preflight;
pub use probe_runtime::probe_tools;
pub use probe_types::*;
pub use registration::{
    AUDIO_INSPECTION_REGISTRATION_SCHEMA, TOOLCHAIN_REGISTRATION_PREPARATION_SCHEMA,
    AudioInspectionRegistrationRequest, ToolchainRegistrationAdapter,
    ToolchainRegistrationFile, ToolchainRegistrationPreparation,
    prepare_audio_inspection_registration,
};
pub use types::*;
