//! Publishing API for extension stores
//!
//! This module defines the traits and types needed for publishing extensions
//! to various store backends. It supports different publishing models including
//! direct uploads, pull request workflows, and validation pipelines.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::registry::ValidationIssue;

/// Options for publishing a new extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishOptions {
    /// Whether to overwrite an existing version
    pub overwrite_existing: bool,

    /// Mark this as a pre-release version
    pub pre_release: bool,

    /// Visibility level for the extension
    pub visibility: ExtensionVisibility,

    /// Authentication token for the store
    pub access_token: Option<String>,

    /// Whether to run validation before publishing
    pub skip_validation: bool,

    /// Timeout for the publishing operation
    pub timeout: Option<Duration>,

    /// Tags to associate with this publication
    pub tags: Vec<String>,

    /// Release notes or changelog
    pub release_notes: Option<String>,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            overwrite_existing: false,
            pre_release: false,
            visibility: ExtensionVisibility::Public,
            access_token: None,
            skip_validation: false,
            timeout: Some(Duration::from_secs(300)), // 5 minutes
            tags: Vec::new(),
            release_notes: None,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpublishOptions {
    /// Authentication token
    pub access_token: Option<String>,

    /// Version to unpublish (None means all versions)
    pub version: Option<String>,

    /// Reason for unpublishing
    pub reason: Option<String>,

    /// Whether to keep a tombstone record
    pub keep_record: bool,

    /// Whether to notify users who have installed this version
    pub notify_users: bool,
}

impl Default for UnpublishOptions {
    fn default() -> Self {
        Self {
            access_token: None,
            version: None,
            reason: None,
            keep_record: true,
            notify_users: false,
        }
    }
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
    pub version: String,

    /// URL where the extension can be downloaded
    pub download_url: String,

    /// When the extension was published
    pub published_at: DateTime<Utc>,

    /// Unique identifier for this publication
    pub publication_id: String,

    /// Size of the published package in bytes
    pub package_size: u64,

    /// Content hash of the published package
    pub content_hash: String,

    /// Any warnings generated during publishing
    pub warnings: Vec<String>,
}

impl PublishResult {
    /// Create a successful result
    pub fn success(
        extension_id: String,
        version: String,
        download_url: String,
        publication_id: String,
        package_size: u64,
        content_hash: String,
    ) -> Self {
        Self {
            extension_id,
            version,
            download_url,
            published_at: Utc::now(),
            publication_id,
            package_size,
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

    /// When the unpublishing occurred
    pub unpublished_at: DateTime<Utc>,

    /// Whether a tombstone record was kept
    pub tombstone_created: bool,

    /// Number of users notified (if applicable)
    pub users_notified: Option<u32>,
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

    /// Validator version used
    pub validator_version: String,
}

impl ValidationReport {
    /// Create a passed validation report
    pub fn passed() -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
            validation_duration: Duration::from_secs(0),
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create a failed validation report
    pub fn failed(issues: Vec<ValidationIssue>) -> Self {
        Self {
            passed: false,
            issues,
            validation_duration: Duration::from_secs(0),
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Check if there are any critical issues
    pub fn has_critical_issues(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| matches!(issue.severity, crate::registry::IssueSeverity::Critical))
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

    /// Allowed file extensions in packages
    pub allowed_file_extensions: Vec<String>,

    /// Forbidden file patterns
    pub forbidden_patterns: Vec<String>,

    /// Required metadata fields
    pub required_metadata: Vec<String>,

    /// Supported visibility levels
    pub supported_visibility: Vec<ExtensionVisibility>,

    /// Whether versioning is enforced
    pub enforces_versioning: bool,

    /// Validation rules that will be applied
    pub validation_rules: Vec<String>,
}

impl Default for PublishRequirements {
    fn default() -> Self {
        Self {
            requires_authentication: false,
            requires_signing: false,
            max_package_size: None,
            allowed_file_extensions: vec![
                "wasm".to_string(),
                "json".to_string(),
                "md".to_string(),
                "txt".to_string(),
            ],
            forbidden_patterns: vec![
                "*.exe".to_string(),
                "*.dll".to_string(),
                "*.so".to_string(),
                "*.dylib".to_string(),
            ],
            required_metadata: vec!["name".to_string(), "version".to_string()],
            supported_visibility: vec![ExtensionVisibility::Public, ExtensionVisibility::Unlisted],
            enforces_versioning: true,
            validation_rules: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_options_defaults() {
        let options = PublishOptions::default();
        assert!(!options.overwrite_existing);
        assert!(!options.pre_release);
        assert_eq!(options.visibility, ExtensionVisibility::Public);
    }

    #[test]
    fn test_dev_options() {
        let options = PublishOptions::dev_defaults();
        assert!(options.overwrite_existing);
        assert!(options.skip_validation);
        assert_eq!(options.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_validation_report() {
        let report = ValidationReport::passed();
        assert!(report.passed);
        assert!(report.issues.is_empty());
        assert!(!report.has_critical_issues());
    }

    #[test]
    fn test_publish_result_builder() {
        let result = PublishResult::success(
            "ext-123".to_string(),
            "1.0.0".to_string(),
            "https://example.com/download".to_string(),
            "pub-123".to_string(),
            1024,
            "abc123".to_string(),
        )
        .with_warning("Minor issue detected".to_string());

        assert_eq!(result.version, "1.0.0");
        assert_eq!(result.warnings.len(), 1);
    }
}
