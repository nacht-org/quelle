//! Publishing API for extension stores
//!
//! This module defines the traits and types needed for publishing extensions
//! to various store backends. It supports different publishing models including
//! direct uploads, pull request workflows, and validation pipelines.

use std::time::Duration;

use quelle_types::version::Version;
use serde::{Deserialize, Serialize};

use crate::registry::ValidationIssue;

/// Options for publishing a new extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishOptions {
    /// Whether to overwrite an existing version
    pub overwrite_existing: bool,

    /// Visibility level for the extension
    pub visibility: ExtensionVisibility,

    /// Whether to run validation before publishing
    pub skip_validation: bool,

    /// Timeout for the publishing operation
    pub timeout: Option<Duration>,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            overwrite_existing: false,
            visibility: ExtensionVisibility::Public,
            skip_validation: false,
            timeout: Some(Duration::from_secs(300)), // 5 minutes
        }
    }
}

impl PublishOptions {
    /// Create options for development/testing
    pub fn dev_defaults() -> Self {
        Self {
            overwrite_existing: true,
            skip_validation: true,
            timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        }
    }

    /// Create options for production releases
    pub fn production_defaults() -> Self {
        Self {
            overwrite_existing: false,
            skip_validation: false,
            visibility: ExtensionVisibility::Public,
            ..Default::default()
        }
    }
}

/// Options for unpublishing an extension
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnpublishOptions {
    /// Version to unpublish (None means all versions)
    pub version: Option<String>,
}

/// Visibility levels for published extensions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionVisibility {
    /// Publicly discoverable and installable
    Public,
    /// Only accessible to authenticated users
    Private,
    /// Not discoverable but installable via direct link
    Unlisted,
}

/// Result of a successful publish operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    /// The ID of the extension that was published
    pub extension_id: String,
    /// The version that was published
    pub version: Version,
    /// Content hash of the published package
    pub content_hash: String,
    /// Any warnings generated during publishing
    pub warnings: Vec<String>,
}

impl PublishResult {
    /// Create a successful result
    pub fn success(extension_id: String, version: Version, content_hash: String) -> Self {
        Self {
            extension_id,
            version,
            content_hash,
            warnings: Vec::new(),
        }
    }

    /// Add a warning message
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// Result of an unpublish operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpublishResult {
    /// The ID of the extension that was unpublished
    pub extension_id: String,
    /// The version that was unpublished
    pub version: String,
}

/// Validation report for a publish operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Whether the package passed validation
    pub passed: bool,

    /// List of validation issues found
    pub issues: Vec<ValidationIssue>,

    /// Time taken to perform validation
    pub validation_duration: Duration,
}

impl ValidationReport {
    /// Create a passed validation report
    pub fn passed() -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
            validation_duration: Duration::from_secs(0),
        }
    }

    /// Create a failed validation report
    pub fn failed(issues: Vec<ValidationIssue>) -> Self {
        Self {
            passed: false,
            issues,
            validation_duration: Duration::from_secs(0),
        }
    }

    /// Check if there are any critical issues
    pub fn has_critical_issues(&self) -> bool {
        self.issues.iter().any(|issue| {
            matches!(
                issue.severity,
                crate::registry::core::IssueSeverity::Critical
            )
        })
    }
}

/// Publishing requirements for a store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequirements {
    /// Whether authentication is required
    pub requires_authentication: bool,

    /// Whether packages must be signed
    pub requires_signing: bool,

    /// Maximum package size allowed (in bytes)
    pub max_package_size: Option<u64>,

    /// Supported visibility levels
    pub supported_visibility: Vec<ExtensionVisibility>,
}

impl Default for PublishRequirements {
    fn default() -> Self {
        Self {
            requires_authentication: false,
            requires_signing: false,
            max_package_size: None,
            supported_visibility: vec![ExtensionVisibility::Public, ExtensionVisibility::Unlisted],
        }
    }
}

/// Errors specific to publishing operations
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Version {0} already exists")]
    VersionAlreadyExists(String),

    #[error("Authentication required")]
    AuthenticationRequired,

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Package too large: {size} bytes (max: {max} bytes)")]
    PackageTooLarge { size: u64, max: u64 },

    #[error("Validation failed: {0} critical issues")]
    ValidationFailed(usize),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    #[error("Invalid visibility level: {0}")]
    InvalidVisibility(String),

    #[error("Store does not support publishing")]
    PublishingNotSupported,
}
