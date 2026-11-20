//! User-facing dependency resolution before the existing exact native contracts.
//! Planning observes paths and store receipts without downloads or native probes.
//! Execution acquires only selected missing managed roles, then freezes their
//! bytes in the ordinary source recipe and native stage configurations.
mod builders;
mod resolve;
mod types;
pub use builders::{
    build_rife, build_source, local_ffmpeg_pin, source_assembly_working_bytes,
    source_validator_storage_admission,
};
pub use resolve::*;
pub use types::*;
