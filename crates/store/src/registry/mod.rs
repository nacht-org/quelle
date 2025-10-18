//! Registry module - Local state management for installed extensions
//!
//! This module manages the local registry of installed extensions, including
//! installation/uninstallation operations, manifest handling, and validation
//! of extension packages before they are added to the local registry.

pub mod core;
pub mod manifest;
pub mod validation;

// Re-export commonly used types from this module
pub use core::{LocalRegistryStore, RegistryStore, ValidationIssue};
pub use manifest::ExtensionManifest;
pub use validation::{
    create_default_validator, ManifestValidationRule, SecurityValidationRule, ValidationEngine,
    ValidationRule,
};
