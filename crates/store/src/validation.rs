//! Extension validation framework
//!
//! This module provides a validation system for extension packages before publishing.
//! It includes a pluggable validation system where different validation rules can be
//! applied to ensure extensions meet quality, security, and compatibility requirements.

use std::collections::HashMap;

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
    fn rule_name(&self) -> &str;

    /// Description of what this rule validates
    fn description(&self) -> &str;

    /// Validate an extension package
    async fn validate(&self, package: &ExtensionPackage) -> Result<Vec<ValidationIssue>>;

    /// Priority of this rule (higher priority runs first)
    fn priority(&self) -> u8 {
        100
    }

    /// Whether this rule should block publishing if it fails
    fn is_blocking(&self) -> bool {
        true
    }
}

/// Validation engine that orchestrates multiple validation rules
pub struct ValidationEngine {
    rules: Vec<Box<dyn ValidationRule>>,
    config: ValidationConfig,
}

/// Configuration for the validation engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Maximum time to spend on validation
    pub max_validation_time: Duration,

    /// Whether to continue validation after finding critical issues
    pub fail_fast: bool,

    /// Whether to run non-blocking rules even if blocking rules fail
    pub run_all_rules: bool,

    /// Custom configuration per rule
    pub rule_configs: HashMap<String, serde_json::Value>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_validation_time: Duration::from_secs(30),
            fail_fast: true,
            run_all_rules: false,
            rule_configs: HashMap::new(),
        }
    }
}

/// Comprehensive validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionValidationReport {
    /// Whether the package passed all validation
    pub passed: bool,

    /// All validation issues found
    pub issues: Vec<ValidationIssue>,

    /// Time taken for validation
    pub validation_duration: Duration,

    /// Results per rule
    pub rule_results: HashMap<String, RuleResult>,

    /// Overall validation summary
    pub summary: ValidationSummary,

    /// Validation metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of a single validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleResult {
    /// Name of the rule
    pub rule_name: String,

    /// Whether the rule passed
    pub passed: bool,

    /// Issues found by this rule
    pub issues: Vec<ValidationIssue>,

    /// Time taken by this rule
    pub duration: Duration,

    /// Whether the rule was skipped
    pub skipped: bool,

    /// Reason for skipping (if applicable)
    pub skip_reason: Option<String>,
}

/// Summary of validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    /// Total number of rules run
    pub rules_run: usize,

    /// Number of rules that passed
    pub rules_passed: usize,

    /// Number of rules that failed
    pub rules_failed: usize,

    /// Number of rules skipped
    pub rules_skipped: usize,

    /// Issues by severity
    pub issues_by_severity: HashMap<String, usize>,

    /// Whether any blocking rules failed
    pub has_blocking_failures: bool,
}

impl ValidationEngine {
    /// Create a new validation engine
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            config: ValidationConfig::default(),
        }
    }

    /// Create validation engine with configuration
    pub fn with_config(config: ValidationConfig) -> Self {
        Self {
            rules: Vec::new(),
            config,
        }
    }

    /// Add a validation rule
    pub fn add_rule(mut self, rule: Box<dyn ValidationRule>) -> Self {
        self.rules.push(rule);
        // Sort by priority (higher first)
        self.rules.sort_by(|a, b| b.priority().cmp(&a.priority()));
        self
    }

    /// Add multiple validation rules
    pub fn add_rules(mut self, rules: Vec<Box<dyn ValidationRule>>) -> Self {
        for rule in rules {
            self.rules.push(rule);
        }
        // Sort by priority (higher first)
        self.rules.sort_by(|a, b| b.priority().cmp(&a.priority()));
        self
    }

    /// Validate an extension package
    pub async fn validate(&self, package: &ExtensionPackage) -> Result<ExtensionValidationReport> {
        let start_time = Instant::now();
        let mut all_issues = Vec::new();
        let mut rule_results = HashMap::new();
        let mut has_blocking_failures = false;

        for rule in &self.rules {
            let rule_start = Instant::now();
            let rule_name = rule.rule_name().to_string();

            // Check if we should skip this rule due to fail_fast
            if self.config.fail_fast && has_blocking_failures {
                rule_results.insert(
                    rule_name.clone(),
                    RuleResult {
                        rule_name: rule_name.clone(),
                        passed: false,
                        issues: Vec::new(),
                        duration: Duration::from_nanos(0),
                        skipped: true,
                        skip_reason: Some(
                            "Skipped due to fail_fast and previous blocking failures".to_string(),
                        ),
                    },
                );
                continue;
            }

            // Check timeout
            if start_time.elapsed() > self.config.max_validation_time {
                rule_results.insert(
                    rule_name.clone(),
                    RuleResult {
                        rule_name: rule_name.clone(),
                        passed: false,
                        issues: Vec::new(),
                        duration: Duration::from_nanos(0),
                        skipped: true,
                        skip_reason: Some("Skipped due to validation timeout".to_string()),
                    },
                );
                break;
            }

            // Run the validation rule
            match rule.validate(package).await {
                Ok(issues) => {
                    let rule_duration = rule_start.elapsed();
                    let rule_passed = issues.is_empty()
                        || !issues.iter().any(|i| {
                            matches!(i.severity, IssueSeverity::Critical | IssueSeverity::Error)
                        });

                    // Check if this rule failure should block further processing
                    if !rule_passed && rule.is_blocking() {
                        has_blocking_failures = true;
                    }

                    all_issues.extend(issues.clone());

                    rule_results.insert(
                        rule_name.clone(),
                        RuleResult {
                            rule_name: rule_name.clone(),
                            passed: rule_passed,
                            issues,
                            duration: rule_duration,
                            skipped: false,
                            skip_reason: None,
                        },
                    );
                }
                Err(e) => {
                    let rule_duration = rule_start.elapsed();
                    let error_issue = ValidationIssue {
                        extension_name: package.manifest.name.clone(),
                        issue_type: ValidationIssueType::InvalidManifest,
                        description: format!("Validation rule '{}' failed: {}", rule_name, e),
                        severity: IssueSeverity::Error,
                    };

                    all_issues.push(error_issue.clone());

                    rule_results.insert(
                        rule_name.clone(),
                        RuleResult {
                            rule_name: rule_name.clone(),
                            passed: false,
                            issues: vec![error_issue],
                            duration: rule_duration,
                            skipped: false,
                            skip_reason: None,
                        },
                    );

                    if rule.is_blocking() {
                        has_blocking_failures = true;
                    }
                }
            }
        }

        let validation_duration = start_time.elapsed();

        // Generate summary
        let summary = self.generate_summary(&rule_results, &all_issues, has_blocking_failures);

        // Determine if validation passed
        let passed = !has_blocking_failures
            && !all_issues
                .iter()
                .any(|i| matches!(i.severity, IssueSeverity::Critical));

        Ok(ExtensionValidationReport {
            passed,
            issues: all_issues,
            validation_duration,
            rule_results,
            summary,
            metadata: HashMap::new(),
        })
    }

    /// Generate validation summary
    fn generate_summary(
        &self,
        rule_results: &HashMap<String, RuleResult>,
        all_issues: &[ValidationIssue],
        has_blocking_failures: bool,
    ) -> ValidationSummary {
        let rules_run = rule_results.values().filter(|r| !r.skipped).count();
        let rules_passed = rule_results
            .values()
            .filter(|r| !r.skipped && r.passed)
            .count();
        let rules_failed = rule_results
            .values()
            .filter(|r| !r.skipped && !r.passed)
            .count();
        let rules_skipped = rule_results.values().filter(|r| r.skipped).count();

        let mut issues_by_severity = HashMap::new();
        for issue in all_issues {
            let severity_key = format!("{:?}", issue.severity);
            *issues_by_severity.entry(severity_key).or_insert(0) += 1;
        }

        ValidationSummary {
            rules_run,
            rules_passed,
            rules_failed,
            rules_skipped,
            issues_by_severity,
            has_blocking_failures,
        }
    }
}

impl Default for ValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Security validation rule - checks for potentially dangerous content
pub struct SecurityValidationRule {
    config: SecurityRuleConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRuleConfig {
    /// Maximum allowed WASM size (in bytes)
    pub max_wasm_size: u64,

    /// Forbidden file patterns in assets
    pub forbidden_patterns: Vec<String>,

    /// Required security headers in manifest
    pub required_security_headers: Vec<String>,

    /// Whether to perform deep WASM analysis
    pub analyze_wasm_content: bool,
}

impl Default for SecurityRuleConfig {
    fn default() -> Self {
        Self {
            max_wasm_size: 50 * 1024 * 1024, // 50MB
            forbidden_patterns: vec![
                "*.exe".to_string(),
                "*.dll".to_string(),
                "*.so".to_string(),
                "*.dylib".to_string(),
                "*.bat".to_string(),
                "*.sh".to_string(),
                "*.ps1".to_string(),
            ],
            required_security_headers: vec![],
            analyze_wasm_content: false, // Basic implementation
        }
    }
}

impl Default for SecurityValidationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityValidationRule {
    pub fn new() -> Self {
        Self {
            config: SecurityRuleConfig::default(),
        }
    }

    pub fn with_config(config: SecurityRuleConfig) -> Self {
        Self { config }
    }

    /// Check if a file path matches forbidden patterns
    fn is_forbidden_file(&self, path: &str) -> bool {
        for pattern in &self.config.forbidden_patterns {
            if pattern.starts_with('*') && pattern.len() > 1 {
                let extension = &pattern[1..];
                if path.ends_with(extension) {
                    return true;
                }
            } else if path == pattern {
                return true;
            }
        }
        false
    }

    /// Basic WASM content validation
    fn validate_wasm_content(&self, wasm_content: &[u8]) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Check WASM magic number
        if wasm_content.len() < 8 {
            issues.push(ValidationIssue {
                extension_name: "unknown".to_string(),
                issue_type: ValidationIssueType::CorruptedFiles,
                description: "WASM file too small to be valid".to_string(),
                severity: IssueSeverity::Critical,
            });
            return issues;
        }

        // Check WASM magic number (0x00 0x61 0x73 0x6d)
        let magic = &wasm_content[0..4];
        if magic != [0x00, 0x61, 0x73, 0x6d] {
            issues.push(ValidationIssue {
                extension_name: "unknown".to_string(),
                issue_type: ValidationIssueType::CorruptedFiles,
                description: "Invalid WASM magic number".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        // Check WASM version (should be 1)
        let version = &wasm_content[4..8];
        if version != [0x01, 0x00, 0x00, 0x00] {
            issues.push(ValidationIssue {
                extension_name: "unknown".to_string(),
                issue_type: ValidationIssueType::CorruptedFiles,
                description: format!("Unsupported WASM version: {:?}", version),
                severity: IssueSeverity::Warning,
            });
        }

        issues
    }
}

#[async_trait]
impl ValidationRule for SecurityValidationRule {
    fn rule_name(&self) -> &str {
        "security"
    }

    fn description(&self) -> &str {
        "Validates extension packages for security issues including file size limits, forbidden file types, and WASM content integrity"
    }

    fn priority(&self) -> u8 {
        200 // High priority for security
    }

    fn is_blocking(&self) -> bool {
        true
    }

    async fn validate(&self, package: &ExtensionPackage) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        // Check WASM size
        if package.wasm_component.len() as u64 > self.config.max_wasm_size {
            issues.push(ValidationIssue {
                extension_name: package.manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: format!(
                    "WASM component size ({} bytes) exceeds maximum allowed size ({} bytes)",
                    package.wasm_component.len(),
                    self.config.max_wasm_size
                ),
                severity: IssueSeverity::Critical,
            });
        }

        // Check for forbidden files in assets
        for asset_path in package.assets.keys() {
            if self.is_forbidden_file(asset_path) {
                issues.push(ValidationIssue {
                    extension_name: package.manifest.name.clone(),
                    issue_type: ValidationIssueType::InvalidManifest,
                    description: format!("Forbidden file type in assets: {}", asset_path),
                    severity: IssueSeverity::Critical,
                });
            }
        }

        // Validate WASM content
        let mut wasm_issues = self.validate_wasm_content(&package.wasm_component);
        for issue in &mut wasm_issues {
            issue.extension_name = package.manifest.name.clone();
        }
        issues.extend(wasm_issues);

        // Check extension name for suspicious patterns
        let name = &package.manifest.name;
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            issues.push(ValidationIssue {
                extension_name: package.manifest.name.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: "Extension name contains path traversal characters".to_string(),
                severity: IssueSeverity::Critical,
            });
        }

        // Check for suspicious base URLs
        for base_url in &package.manifest.base_urls {
            if base_url.starts_with("file://") || base_url.starts_with("ftp://") {
                issues.push(ValidationIssue {
                    extension_name: package.manifest.name.clone(),
                    issue_type: ValidationIssueType::InvalidManifest,
                    description: format!("Potentially unsafe base URL protocol: {}", base_url),
                    severity: IssueSeverity::Warning,
                });
            }
        }

        Ok(issues)
    }
}

/// Create a default validation engine with essential rules
pub fn create_default_validator() -> ValidationEngine {
    ValidationEngine::new().add_rule(Box::new(SecurityValidationRule::new()))
}

/// Create a strict validation engine with enhanced security rules
pub fn create_strict_validator() -> ValidationEngine {
    let strict_security_config = SecurityRuleConfig {
        max_wasm_size: 10 * 1024 * 1024, // 10MB limit
        forbidden_patterns: vec![
            "*.exe".to_string(),
            "*.dll".to_string(),
            "*.so".to_string(),
            "*.dylib".to_string(),
            "*.bat".to_string(),
            "*.sh".to_string(),
            "*.ps1".to_string(),
            "*.scr".to_string(),
            "*.com".to_string(),
        ],
        required_security_headers: vec![],
        analyze_wasm_content: true,
    };

    ValidationEngine::new().add_rule(Box::new(SecurityValidationRule::with_config(
        strict_security_config,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ExtensionManifest, ReadingDirection};
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
            attrs: vec![],

            signature: None,
            wasm_file: crate::manifest::FileReference::new(
                "./extension.wasm".to_string(),
                wasm_content,
            ),
            assets: vec![],
        };

        ExtensionPackage::new(manifest, wasm_content.to_vec(), "test-store".to_string())
    }

    #[tokio::test]
    async fn test_security_rule_valid_package() {
        let rule = SecurityValidationRule::new();

        // Valid WASM magic number + version
        let valid_wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x00];
        let package = create_test_package("valid-extension", &valid_wasm);

        let issues = rule.validate(&package).await.unwrap();
        assert!(issues.is_empty(), "Valid package should have no issues");
    }

    #[tokio::test]
    async fn test_security_rule_invalid_wasm() {
        let rule = SecurityValidationRule::new();

        let invalid_wasm = [0x12, 0x34, 0x56, 0x78]; // Invalid magic number
        let package = create_test_package("invalid-extension", &invalid_wasm);

        let issues = rule.validate(&package).await.unwrap();
        assert!(!issues.is_empty(), "Invalid WASM should have issues");
        assert!(issues
            .iter()
            .any(|i| matches!(i.severity, IssueSeverity::Critical)));
    }

    #[tokio::test]
    async fn test_security_rule_forbidden_assets() {
        let rule = SecurityValidationRule::new();

        let valid_wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x00];
        let mut package = create_test_package("test-extension", &valid_wasm);

        // Add forbidden file
        package
            .assets
            .insert("malware.exe".to_string(), vec![0x4d, 0x5a]); // PE header

        let issues = rule.validate(&package).await.unwrap();
        assert!(
            !issues.is_empty(),
            "Package with forbidden files should have issues"
        );
        assert!(issues
            .iter()
            .any(|i| i.description.contains("Forbidden file type")));
    }

    #[tokio::test]
    async fn test_validation_engine() {
        let engine = create_default_validator();

        let valid_wasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x00];
        let package = create_test_package("test-extension", &valid_wasm);

        let report = engine.validate(&package).await.unwrap();
        assert!(report.passed, "Valid package should pass validation");
        assert_eq!(report.summary.rules_run, 1);
        assert_eq!(report.summary.rules_passed, 1);
    }

    #[tokio::test]
    async fn test_validation_engine_with_issues() {
        let engine = create_default_validator();

        let invalid_wasm = [0x12, 0x34, 0x56, 0x78]; // Invalid magic number
        let package = create_test_package("invalid-extension", &invalid_wasm);

        let report = engine.validate(&package).await.unwrap();
        assert!(!report.passed, "Invalid package should fail validation");
        assert!(!report.issues.is_empty());
        assert!(report.summary.has_blocking_failures);
    }
}
