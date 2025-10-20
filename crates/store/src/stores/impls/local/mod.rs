//! Local filesystem store implementation

pub mod file_operations;
pub mod index;
pub mod store;

// Re-export the main types for convenience
pub use store::{LocalStore, LocalStoreBuilder, LocalStoreManifest};
