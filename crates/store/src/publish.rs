//! Publishing API for extension stores
//!
//! This module defines the traits and types needed for publishing extensions
//! to various store backends. It supports different publishing models including
//! direct uploads, pull request workflows, and validation pipelines.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::models::ExtensionPackage;
use crate::registry::ValidationIssue;
use crate::store::Store;

/// Trait for stores that support publishing extensions
#[async_trait]
pub trait PublishableStore: Store {
    /// Publish a new extension version to this store
    async fn publish_extension(
        &mut self,
        package: ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<PublishResult>;

    /// Update an existing extension (may require special permissions)
    async fn update_extension(
        &mut self,
        name: &str,
        package: ExtensionPackage,
        options: &PublishUpdateOptions,
    ) -> Result<PublishResult>;

    /// Remove an extension version from the store
    async fn unpublish_extension(
        &mut self,
        id: &str,
        version: &str,
        options: &UnpublishOptions,
    ) -> Result<UnpublishResult>;

    /// Validate a package before publishing (dry-run)
    async fn validate_publish(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport>;

    /// Get publishing requirements and constraints for this store
    fn publish_requirements(&self) -> PublishRequirements;

    /// Check if the current credentials allow publishing
    async fn can_publish(&self, extension_id: &str) -> Result<PublishPermissions>;

    /// Get publishing statistics and quotas
    async fn get_publish_stats(&self) -> Result<PublishStats>;
}

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

    /// Key for signing the extension package
    pub signing_key: Option<String>,

    /// Additional metadata to include with the publication
    pub metadata: HashMap<String, serde_json::Value>,

    /// Whether to run validation before publishing
    pub skip_validation: bool,

    /// Timeout for the publishing operation
    pub timeout: Option<Duration>,

    /// Whether to create a backup before publishing
    pub create_backup: bool,

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
            signing_key: None,
            metadata: HashMap::new(),
            skip_validation: false,
            timeout: Some(Duration::from_secs(300)), // 5 minutes
            create_backup: true,
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
            create_backup: false,
            timeout: Some(Duration::from_secs(30)),
            ..Default::default()
        }
    }

    /// Create options for production releases
    pub fn production_defaults() -> Self {
        Self {
            overwrite_existing: false,
            skip_validation: false,
            create_backup: true,
            visibility: ExtensionVisibility::Public,
            ..Default::default()
        }
    }
}

/// Options for updating an existing extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishUpdateOptions {
    /// Base publishing options
    #[serde(flatten)]
    pub publish_options: PublishOptions,

    /// Whether to preserve existing metadata
    pub preserve_metadata: bool,

    /// Whether to merge or replace tags
    pub merge_tags: bool,

    /// Reason for the update
    pub update_reason: Option<String>,
}

/// Options for unpublishing an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpublishOptions {
    /// Authentication token
    pub access_token: Option<String>,

    /// Reason for unpublishing
    pub reason: Option<String>,

    /// Whether to keep a tombstone record
    pub keep_record: bool,

    /// Whether to notify users who have installed this version
    pub notify_users: bool,
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
    /// Only available to specific organizations/teams
    Organization,
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

    /// Store-specific information
    pub store_info: HashMap<String, serde_json::Value>,
}

impl PublishResult {
    /// Create a successful result
    pub fn success(
        version: String,
        download_url: String,
        publication_id: String,
        package_size: u64,
        content_hash: String,
    ) -> Self {
        Self {
            version,
            download_url,
            published_at: Utc::now(),
            publication_id,
            package_size,
            content_hash,
            warnings: Vec::new(),
            store_info: HashMap::new(),
        }
    }

    /// Add a warning message
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add store-specific information
    pub fn with_store_info(mut self, key: String, value: serde_json::Value) -> Self {
        self.store_info.insert(key, value);
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

    /// Additional validation metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ValidationReport {
    /// Create a passed validation report
    pub fn passed() -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
            validation_duration: Duration::from_secs(0),
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
            metadata: HashMap::new(),
        }
    }

    /// Create a failed validation report
    pub fn failed(issues: Vec<ValidationIssue>) -> Self {
        Self {
            passed: false,
            issues,
            validation_duration: Duration::from_secs(0),
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
            metadata: HashMap::new(),
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

    /// Store-specific requirements
    pub store_specific: HashMap<String, serde_json::Value>,
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
            store_specific: HashMap::new(),
        }
    }
}

/// Publishing permissions for a user/token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishPermissions {
    /// Whether the user can publish new extensions
    pub can_publish: bool,

    /// Whether the user can update existing extensions
    pub can_update: bool,

    /// Whether the user can unpublish extensions
    pub can_unpublish: bool,

    /// Specific extensions the user has permission for (None = all)
    pub allowed_extensions: Option<Vec<String>>,

    /// Maximum package size this user can publish
    pub max_package_size: Option<u64>,

    /// Rate limits for this user
    pub rate_limits: RateLimits,
}

/// Rate limiting information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
    /// Publications per hour
    pub publications_per_hour: Option<u32>,

    /// Publications per day
    pub publications_per_day: Option<u32>,

    /// Total bandwidth per day (in bytes)
    pub bandwidth_per_day: Option<u64>,
}

/// Publishing statistics and quotas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishStats {
    /// Total number of extensions published
    pub total_extensions: u64,

    /// Total storage used (in bytes)
    pub total_storage_used: u64,

    /// Publications in the last 24 hours
    pub recent_publications: u32,

    /// Available storage quota (in bytes)
    pub storage_quota: Option<u64>,

    /// Current rate limit status
    pub rate_limit_status: RateLimitStatus,

    /// Store-specific statistics
    pub store_specific: HashMap<String, serde_json::Value>,
}

/// Current rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    /// Publications remaining in current window
    pub publications_remaining: Option<u32>,

    /// When the rate limit window resets
    pub reset_time: Option<DateTime<Utc>>,

    /// Whether currently rate limited
    pub is_limited: bool,
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
        assert!(options.create_backup);
    }

    #[test]
    fn test_dev_options() {
        let options = PublishOptions::dev_defaults();
        assert!(options.overwrite_existing);
        assert!(options.skip_validation);
        assert!(!options.create_backup);
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
            "1.0.0".to_string(),
            "https://example.com/download".to_string(),
            "pub-123".to_string(),
            1024,
            "abc123".to_string(),
        )
        .with_warning("Minor issue detected".to_string())
        .with_store_info(
            "cdn_url".to_string(),
            serde_json::Value::String("https://cdn.example.com".to_string()),
        );

        assert_eq!(result.version, "1.0.0");
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.store_info.len(), 1);
    }
}
