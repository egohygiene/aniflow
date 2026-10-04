//! Explicit offline toolchain inventories and read-only setup plans.
//!
//! Files are hashed at caller-supplied paths. Version, flags, build features and
//! hardware remain supplied observations, never invented process-probe results.
//! Readiness means inventory consistency; it is not native qualification or
//! permission to register, execute, install, download or publish anything.

mod inspect;
mod types;

pub use inspect::inspect_profile;
pub use types::*;
