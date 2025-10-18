//! Manager module - Orchestration layer for store operations
//!
//! This module provides the core orchestration logic that coordinates between
//! different store backends and the local registry. It handles high-level operations
//! like search, installation workflows, publishing, and update management.

pub mod core;
pub mod publish;
pub mod store_manifest;

// Re-export commonly used types from this module
pub use core::{ManagedStore, StoreManager};
pub use publish::{
    ExtensionVisibility, PublishError, PublishOptions, PublishRequirements, PublishResult,
    UnpublishOptions, UnpublishResult, ValidationReport,
};
pub use store_manifest::{ExtensionSummary, StoreManifest, UrlPattern};
