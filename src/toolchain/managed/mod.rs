//! Verified, user-owned external tool/model acquisition. This module prepares
//! exact paths for existing media owners; it never processes media or approves it.
mod catalog;
mod extract;
mod store;
mod transport;
mod types;

pub use catalog::{bundled_catalog, package_sha256};
pub use store::ManagedStore;
pub use transport::{HttpsDownloader, ManagedDownloader};
pub use types::*;

fn dependency(message: impl Into<String>) -> crate::Error {
    crate::Error::new(crate::ErrorCategory::Dependency, message)
}
fn io(error: impl std::fmt::Display) -> crate::Error {
    crate::Error::new(
        crate::ErrorCategory::Io,
        format!("managed toolchain: {error}"),
    )
}
