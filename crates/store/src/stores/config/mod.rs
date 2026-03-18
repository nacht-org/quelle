//! Store configuration management
//!
//! This module provides configuration structures for managing extension sources
//! and registry configurations.

pub mod registry;
pub mod source;

// Re-export commonly used configuration types
pub use registry::{SourceConfig, SourceConfigs, SourceCounts};
pub use source::{create_readable_store_from_source, ExtensionSource, RegistryConfig, StoreType};
