//! Extension validation framework
//!
//! Minimal validation system for extension packages before installation.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::ExtensionPackage;
use crate::registry::{IssueSeverity, ValidationIssue, ValidationIssueType};
use crate::Result;

/// Core trait for validation rules
#[async_trait]
pub trait ValidationRule: Send + Sync {
    /// Name of the validation rule
    fn rule_name(&self) -> &'static str;

    /// Description of what this rule validates
    fn description(&self) -> &'static str;

    /// Validate an extension package
    async fn validate(&self, package: &ExtensionPackage) -> Result<Vec<ValidationIssue>>;

    /// Whether this rule is blocking (prevents installation if failed)
    fn is_blocking(&self) -> bool {
        true
    }
}

/// Simple validation engine
pub struct ValidationEngine {
    rules: Vec<Box<dyn ValidationRule>>,
}

/// Validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passed: bool,
    pub issues: Vec<ValidationIssue>,
    pub validation_duration: Duration,
}

impl ValidationEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(mut self, rule: Box<dyn ValidationRule>) -> Self {
        self.rules.push(rule);
        self
    }

    pub async fn validate(&self, package: &ExtensionPackage) -> Result<ValidationReport> {
        let start = Instant::now();
        let mut all_issues = Vec::new();
        let mut has_blocking_failure = false;

        for rule in &self.rules {
            let issues = rule.validate(package).await?;

            // Check if any critical/error issues from blocking rules
            if rule.is_blocking() {
                for issue in &issues {
                    if matches!(
                        issue.severity,
                        IssueSeverity::Critical | IssueSeverity::Error
                    ) {
                        has_blocking_failure = true;
                    }
                }
            }

            all_issues.extend(issues);
        }

        Ok(ValidationReport {
            passed: !has_blocking_failure,
            issues: all_issues,
            validation_duration: start.elapsed(),
        })
    }
}

impl Default for ValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Basic security validation rule
pub struct SecurityValidationRule;

impl SecurityValidationRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRule for SecurityValidationRule {
    fn rule_name(&self) -> &'static str {
        "security_validation"
    }

    fn description(&self) -> &'static str {
        "Basic security checks for extension packages"
    }

    async fn validate(&self, package: &ExtensionPackage) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check WASM size (max 50MB)
        const MAX_WASM_SIZE: u64 = 50 * 1024 * 1024;
        if package.wasm_component.len() as u64 > MAX_WASM_SIZE {
            issues.push(ValidationIssue {
                extension_name: package.manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: format!(
                    "WASM component too large ({} bytes, max: {} bytes)",
                    package.wasm_component.len(),
                    MAX_WASM_SIZE
                ),
                severity: IssueSeverity::Critical,
            });
        }

        // Check WASM magic number
        if package.wasm_component.len() >= 4 {
            let magic = &package.wasm_component[0..4];
            if magic != b"\0asm" {
                issues.push(ValidationIssue {
                    extension_name: package.manifest.name.clone(),
                    issue_type: ValidationIssueType::CorruptedFiles,
                    description: "Invalid WASM file format".to_string(),
                    severity: IssueSeverity::Critical,
                });
            }
        } else {
            issues.push(ValidationIssue {
                extension_name: package.manifest.name.clone(),
                issue_type: ValidationIssueType::CorruptedFiles,
                description: "WASM file too small to be valid".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        // Check extension name for path traversal
        let name = &package.manifest.name;
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            issues.push(ValidationIssue {
                extension_name: package.manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: "Extension name contains invalid characters".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        Ok(issues)
    }
}

/// Basic manifest validation rule
pub struct ManifestValidationRule;

impl ManifestValidationRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRule for ManifestValidationRule {
    fn rule_name(&self) -> &'static str {
        "manifest_validation"
    }

    fn description(&self) -> &'static str {
        "Basic manifest field validation"
    }

    async fn validate(&self, package: &ExtensionPackage) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        let manifest = &package.manifest;

        // Check required fields are not empty
        if manifest.id.trim().is_empty() {
            issues.push(ValidationIssue {
                extension_name: manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: "Extension ID cannot be empty".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        if manifest.name.trim().is_empty() {
            issues.push(ValidationIssue {
                extension_name: manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: "Extension name cannot be empty".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        if manifest.version.trim().is_empty() {
            issues.push(ValidationIssue {
                extension_name: manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: "Extension version cannot be empty".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        if manifest.author.trim().is_empty() {
            issues.push(ValidationIssue {
                extension_name: manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: "Extension author cannot be empty".to_string(),
                severity: IssueSeverity::Error,
            });
        }

        Ok(issues)
    }
}

/// Create default validator with essential rules
pub fn create_default_validator() -> ValidationEngine {
    ValidationEngine::new()
        .add_rule(Box::new(ManifestValidationRule::new()))
        .add_rule(Box::new(SecurityValidationRule::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Attribute, ExtensionManifest, FileReference, ReadingDirection};
    use crate::models::ExtensionPackage;

    fn create_test_package(name: &str, wasm_content: &[u8]) -> ExtensionPackage {
        let manifest = ExtensionManifest {
            id: format!("test-{}", name),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![ReadingDirection::Ltr],
            attrs: vec![Attribute::Fanfiction],
            signature: None,
            wasm_file: FileReference {
                path: "extension.wasm".to_string(),
                checksum: "dummy-checksum".to_string(),
                size: wasm_content.len() as u64,
            },
            assets: Vec::new(),
        };

        ExtensionPackage {
            manifest,
            wasm_component: wasm_content.to_vec(),
            metadata: None,
            assets: std::collections::HashMap::new(),
            source_store: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_valid_package() {
        let engine = create_default_validator();
        let valid_wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let package = create_test_package("valid-extension", &valid_wasm);

        let report = engine.validate(&package).await.unwrap();
        assert!(report.passed, "Valid package should pass validation");
        assert!(report.issues.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_wasm() {
        let engine = create_default_validator();
        let invalid_wasm = [0x12, 0x34, 0x56, 0x78];
        let package = create_test_package("invalid-extension", &invalid_wasm);

        let report = engine.validate(&package).await.unwrap();
        assert!(!report.passed, "Invalid WASM should fail validation");
        assert!(!report.issues.is_empty());
    }

    #[tokio::test]
    async fn test_empty_fields() {
        let engine = create_default_validator();
        let valid_wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let mut package = create_test_package("test-extension", &valid_wasm);

        // Make name empty
        package.manifest.name = "".to_string();

        let report = engine.validate(&package).await.unwrap();
        assert!(!report.passed, "Empty name should fail validation");
        assert!(report
            .issues
            .iter()
            .any(|i| matches!(i.severity, IssueSeverity::Critical)));
    }
}
