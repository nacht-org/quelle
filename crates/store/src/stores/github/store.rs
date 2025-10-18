//! GitHub store implementation using FileBasedProcessor

use async_trait::async_trait;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tracing::{debug, info};

use super::file_operations::GitHubFileOperations;
use crate::error::{Result, StoreError};
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstalledExtension, SearchQuery,
    StoreHealth, UpdateInfo,
};
use crate::publish::{
    PublishOptions, PublishRequirements, PublishResult, UnpublishOptions, UnpublishResult,
    ValidationReport,
};
use crate::store_manifest::StoreManifest;
use crate::stores::file_operations::FileBasedProcessor;
use crate::stores::providers::git::GitReference;
use crate::stores::providers::git::{GitAuth, GitWriteConfig};
use crate::stores::traits::{BaseStore, CacheableStore, ReadableStore, WritableStore};

/// GitHub store that uses FileBasedProcessor with GitHub-specific file operations
pub struct GitHubStore {
    processor: FileBasedProcessor<GitHubFileOperations>,
    owner: String,
    repo: String,
    reference: GitReference,
    name: String,
    cache_dir: Option<PathBuf>,
    write_config: Option<GitWriteConfig>,
}

/// Builder for creating GitHub stores with a fluent API
pub struct GitHubStoreBuilder {
    name: Option<String>,
    owner: String,
    repo: String,
    cache_dir: Option<PathBuf>,
    reference: GitReference,
    auth: Option<GitAuth>,
    cache_ttl: Duration,
    write_config: Option<GitWriteConfig>,
}

impl GitHubStoreBuilder {
    /// Create a new builder for the given GitHub repository
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            name: None,
            owner: owner.into(),
            repo: repo.into(),
            cache_dir: None,
            reference: GitReference::Default,
            auth: None,
            cache_ttl: Duration::from_secs(300), // 5 minutes
            write_config: None,
        }
    }

    /// Create a builder from a GitHub URL
    pub fn from_url(url: impl AsRef<str>) -> Result<Self> {
        let url = url.as_ref();
        let (owner, repo) = Self::parse_github_url(url)?;
        Ok(Self::new(owner, repo))
    }

    /// Parse a GitHub URL to extract owner and repo
    fn parse_github_url(url: &str) -> Result<(String, String)> {
        let url = url.trim_end_matches('/').trim_end_matches(".git");

        if let Some(path) = url.strip_prefix("https://github.com/") {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 2 {
                return Ok((parts[0].to_string(), parts[1].to_string()));
            }
        }

        Err(StoreError::InvalidConfiguration(format!(
            "Invalid GitHub URL: {}. Expected format: https://github.com/owner/repo",
            url
        )))
    }

    /// Set the store name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the cache directory for git operations
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(path.into());
        self
    }

    /// Set authentication for private repositories
    pub fn auth(mut self, auth: GitAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set authentication token
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.auth = Some(GitAuth::Token {
            token: token.into(),
        });
        self
    }

    /// Set to use a specific branch
    pub fn branch(mut self, branch: impl Into<String>) -> Self {
        self.reference = GitReference::Branch(branch.into());
        self
    }

    /// Set to use a specific tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.reference = GitReference::Tag(tag.into());
        self
    }

    /// Set to use a specific commit
    pub fn commit(mut self, commit: impl Into<String>) -> Self {
        self.reference = GitReference::Commit(commit.into());
        self
    }

    /// Set the git reference directly
    pub fn reference(mut self, reference: GitReference) -> Self {
        self.reference = reference;
        self
    }

    /// Set the cache TTL for HTTP requests
    pub fn cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Enable writing with default configuration
    pub fn writable(mut self) -> Self {
        self.write_config = Some(GitWriteConfig::default());
        self
    }

    /// Set the author for commits
    pub fn author(mut self, name: impl Into<String>, email: impl Into<String>) -> Self {
        let mut config = self.write_config.unwrap_or_default();
        config.author = Some(crate::stores::providers::git::GitAuthor::new(name, email));
        self.write_config = Some(config);
        self
    }

    /// Set the commit message style
    pub fn commit_style(mut self, style: crate::stores::providers::git::CommitStyle) -> Self {
        let mut config = self.write_config.unwrap_or_default();
        config.commit_style = style;
        self.write_config = Some(config);
        self
    }

    /// Disable automatic pushing (commits will be local only)
    pub fn no_auto_push(mut self) -> Self {
        let mut config = self.write_config.unwrap_or_default();
        config.auto_push = false;
        self.write_config = Some(config);
        self
    }

    /// Set a custom write configuration
    pub fn write_config(mut self, config: GitWriteConfig) -> Self {
        self.write_config = Some(config);
        self
    }

    /// Build the GitHubStore naively (assumes "main" as default branch)
    pub fn build(self) -> Result<GitHubStore> {
        let name = self
            .name
            .unwrap_or_else(|| format!("{}/{}", self.owner, self.repo));

        // Create GitHub file operations with cache TTL
        let file_ops = GitHubFileOperations::builder(
            self.owner.clone(),
            self.repo.clone(),
            self.reference.clone(),
        )
        .with_cache_ttl(self.cache_ttl)
        .build_naive();

        let processor = FileBasedProcessor::new(file_ops, name.clone());

        Ok(GitHubStore {
            processor,
            owner: self.owner,
            repo: self.repo,
            reference: self.reference,
            name,
            cache_dir: self.cache_dir,
            write_config: self.write_config,
        })
    }

    /// Build the GitHubStore asynchronously with accurate default branch resolution
    ///
    /// This method will probe the repository to determine the actual default branch
    /// when GitReference::Default is used, ensuring accurate branch resolution.
    /// Use this instead of `build()` when you need precise default branch detection.
    ///
    /// # Example
    /// ```ignore
    /// // Naive - assumes "main" (fast)
    /// let store = GitHubStore::builder("owner", "repo")
    ///     .reference(GitReference::Default)
    ///     .build()?;
    ///
    /// // Accurate - probes repository (slower but correct)
    /// let store = GitHubStore::builder("owner", "repo")
    ///     .reference(GitReference::Default)
    ///     .build_async()
    ///     .await?;
    /// ```
    pub async fn build_async(self) -> Result<GitHubStore> {
        let name = self
            .name
            .unwrap_or_else(|| format!("{}/{}", self.owner, self.repo));

        // Create GitHub file operations with accurate branch resolution
        let file_ops = GitHubFileOperations::builder(
            self.owner.clone(),
            self.repo.clone(),
            self.reference.clone(),
        )
        .with_cache_ttl(self.cache_ttl)
        .build()
        .await?;

        let processor = FileBasedProcessor::new(file_ops, name.clone());

        Ok(GitHubStore {
            processor,
            owner: self.owner,
            repo: self.repo,
            reference: self.reference,
            name,
            cache_dir: self.cache_dir,
            write_config: self.write_config,
        })
    }
}

impl GitHubStore {
    /// Create a new builder
    pub fn builder(owner: impl Into<String>, repo: impl Into<String>) -> GitHubStoreBuilder {
        GitHubStoreBuilder::new(owner, repo)
    }

    /// Create from a GitHub URL
    pub fn from_url(url: &str) -> Result<GitHubStoreBuilder> {
        GitHubStoreBuilder::from_url(url)
    }

    /// Get the GitHub repository URL
    pub fn github_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo)
    }

    /// Get the git URL for cloning
    pub fn git_url(&self) -> String {
        match &self.write_config {
            Some(_) => format!("git@github.com:{}/{}.git", self.owner, self.repo),
            None => format!("https://github.com/{}/{}.git", self.owner, self.repo),
        }
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> Option<&PathBuf> {
        self.cache_dir.as_ref()
    }

    /// Check if this GitHub store supports writing operations
    pub fn is_writable(&self) -> bool {
        self.write_config.is_some()
    }

    /// Clear the HTTP cache
    pub async fn clear_cache(&self) {
        self.processor.file_ops().clear_cache().await;
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, u64) {
        self.processor.file_ops().cache_stats().await
    }
}

#[async_trait]
impl BaseStore for GitHubStore {
    async fn get_store_manifest(&self) -> Result<StoreManifest> {
        self.processor.get_store_manifest().await
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        let start_time = SystemTime::now();

        // Try to read the store manifest to check if repository is accessible
        let manifest_result = self.get_store_manifest().await;
        let is_healthy = manifest_result.is_ok();
        let error_message = if let Err(ref e) = manifest_result {
            Some(format!("Failed to access GitHub repository: {}", e))
        } else {
            None
        };

        // Count extensions if healthy
        let extension_count = if is_healthy {
            match self.list_extensions().await {
                Ok(extensions) => Some(extensions.len()),
                Err(_) => Some(0),
            }
        } else {
            Some(0)
        };

        Ok(StoreHealth {
            healthy: is_healthy,
            last_check: chrono::Utc::now(),
            response_time: Some(start_time.elapsed().unwrap_or_default()),
            error: error_message,
            extension_count,
            store_version: None,
        })
    }
}

#[async_trait]
impl ReadableStore for GitHubStore {
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        self.processor.find_extensions_for_url(url).await
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        self.processor.list_extensions().await
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        self.processor.search_extensions(query).await
    }

    async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>> {
        self.processor.get_extension_info(name).await
    }

    async fn get_extension_version_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        self.processor
            .get_extension_version_info(name, version)
            .await
    }

    async fn get_extension_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
        self.processor.get_extension_manifest(name, version).await
    }

    async fn get_extension_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        self.processor.get_extension_metadata(name, version).await
    }

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        let manifest = self.processor.get_extension_manifest(id, version).await?;
        let wasm_bytes = self
            .processor
            .get_extension_wasm(id, Some(&manifest.version))
            .await?;
        let metadata = self
            .processor
            .get_extension_metadata(id, Some(&manifest.version))
            .await?;

        let mut package = ExtensionPackage::new(manifest.clone(), wasm_bytes, self.name.clone());

        if let Some(meta) = metadata {
            package = package.with_metadata(meta);
        }

        // Load all assets
        for asset_ref in &manifest.assets {
            match self
                .processor
                .get_extension_asset(id, Some(&manifest.version), &asset_ref.path)
                .await
            {
                Ok(content) => {
                    package.add_asset(asset_ref.path.clone(), content);
                }
                Err(e) => {
                    debug!("Failed to load asset {}: {}", asset_ref.path, e);
                    // Continue loading other assets
                }
            }
        }

        Ok(package)
    }

    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<String>> {
        self.processor.get_extension_latest_version(id).await
    }

    async fn list_extension_versions(&self, id: &str) -> Result<Vec<String>> {
        self.processor.list_extension_versions(id).await
    }

    async fn check_extension_version_exists(&self, id: &str, version: &str) -> Result<bool> {
        self.processor
            .check_extension_version_exists(id, version)
            .await
    }

    async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>> {
        let mut updates = Vec::new();

        for installed_ext in installed {
            if let Ok(Some(latest_version)) =
                self.get_extension_latest_version(&installed_ext.id).await
            {
                if latest_version != installed_ext.version {
                    // Simple version comparison - in practice you'd want semver
                    if latest_version > installed_ext.version {
                        updates.push(UpdateInfo {
                            extension_name: installed_ext.id.clone(),
                            current_version: installed_ext.version.clone(),
                            latest_version,
                            update_available: true,
                            changelog_url: Some(format!(
                                "https://github.com/{}/{}/releases",
                                self.owner, self.repo
                            )),
                            breaking_changes: false, // Would need to analyze changes
                            security_update: false,
                            update_size: None,
                            store_source: self.name.clone(),
                        });
                    }
                }
            }
        }

        Ok(updates)
    }
}

#[async_trait]
impl WritableStore for GitHubStore {
    fn publish_requirements(&self) -> PublishRequirements {
        PublishRequirements {
            requires_authentication: true,
            requires_signing: false,
            max_package_size: Some(25 * 1024 * 1024), // 25MB - GitHub has file size limits
            allowed_file_extensions: vec![
                "wasm".to_string(),
                "json".to_string(),
                "md".to_string(),
                "txt".to_string(),
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "svg".to_string(),
            ],
            forbidden_patterns: vec![
                "*.exe".to_string(),
                "*.dll".to_string(),
                "*.so".to_string(),
                "*.dylib".to_string(),
            ],
            required_metadata: vec!["name".to_string(), "version".to_string()],
            supported_visibility: vec![crate::publish::ExtensionVisibility::Public],
            enforces_versioning: true,
            validation_rules: Vec::new(),
        }
    }

    async fn publish(
        &self,
        _package: ExtensionPackage,
        _options: PublishOptions,
    ) -> Result<PublishResult> {
        // GitHub store publishing would require git operations
        // This would typically be implemented by using a GitProvider internally
        // for now, return an error indicating this needs implementation
        Err(StoreError::UnsupportedOperation(
            "Publishing to GitHub stores requires git integration (use GitStore for publishing to GitHub)".to_string(),
        ))
    }

    async fn unpublish(
        &self,
        _extension_id: &str,
        _options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        // GitHub store unpublishing would require git operations
        Err(StoreError::UnsupportedOperation(
            "Unpublishing from GitHub stores requires git integration (use GitStore for publishing to GitHub)".to_string(),
        ))
    }

    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        _options: &PublishOptions,
    ) -> Result<ValidationReport> {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Basic validation
        if package.manifest.id.is_empty() {
            issues.push("Extension ID cannot be empty".to_string());
        }

        if package.manifest.name.is_empty() {
            issues.push("Extension name cannot be empty".to_string());
        }

        if package.manifest.version.is_empty() {
            issues.push("Extension version cannot be empty".to_string());
        }

        // WASM validation
        if package.wasm_component.is_empty() {
            issues.push("WASM component cannot be empty".to_string());
        }

        // File size check (GitHub has lower limits)
        let total_size = package.calculate_total_size();
        const MAX_SIZE: u64 = 25 * 1024 * 1024; // 25MB
        if total_size > MAX_SIZE {
            issues.push(format!(
                "Package size ({} bytes) exceeds GitHub maximum allowed size ({} bytes)",
                total_size, MAX_SIZE
            ));
        }

        if total_size > 5 * 1024 * 1024 {
            // 5MB warning for GitHub
            warnings.push(format!(
                "Package size is large ({} MB) for GitHub. Consider optimizing assets.",
                total_size / (1024 * 1024)
            ));
        }

        use crate::registry::{IssueSeverity, ValidationIssue, ValidationIssueType};

        let validation_issues: Vec<ValidationIssue> = issues
            .into_iter()
            .map(|msg| ValidationIssue {
                extension_name: package.manifest.id.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: msg,
                severity: IssueSeverity::Critical,
            })
            .chain(warnings.into_iter().map(|msg| ValidationIssue {
                extension_name: package.manifest.id.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: msg,
                severity: IssueSeverity::Warning,
            }))
            .collect();

        Ok(ValidationReport {
            passed: validation_issues
                .iter()
                .all(|issue| !matches!(issue.severity, IssueSeverity::Critical)),
            issues: validation_issues,
            validation_duration: std::time::Duration::from_millis(10),
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}

#[async_trait]
impl CacheableStore for GitHubStore {
    async fn refresh_cache(&self) -> Result<()> {
        // Clear the cache to force fresh fetches
        self.clear_cache().await;
        info!(
            "Refreshed GitHub store cache for {}/{}",
            self.owner, self.repo
        );
        Ok(())
    }

    async fn clear_cache(&self) -> Result<()> {
        self.processor.file_ops().clear_cache().await;
        info!(
            "Cleared GitHub store cache for {}/{}",
            self.owner, self.repo
        );
        Ok(())
    }

    async fn cache_stats(&self) -> Result<crate::stores::traits::CacheStats> {
        let (entries, size_bytes) = self.cache_stats().await;
        Ok(crate::stores::traits::CacheStats {
            entries,
            size_bytes,
            hit_rate: 0.0,      // Would need to track hits/misses to calculate this
            last_refresh: None, // Would need to track refresh times
        })
    }
}

impl Clone for GitHubStore {
    fn clone(&self) -> Self {
        // Note: This creates a new GitHubFileOperations instance with its own cache
        let file_ops = GitHubFileOperations::builder(
            self.owner.clone(),
            self.repo.clone(),
            self.reference.clone(),
        )
        .build_naive();

        let processor = FileBasedProcessor::new(file_ops, self.name.clone());

        Self {
            processor,
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            reference: self.reference.clone(),
            name: self.name.clone(),
            cache_dir: self.cache_dir.clone(),
            write_config: self.write_config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_store_builder_basic() {
        let store = GitHubStore::builder("owner", "repo")
            .name("test-store")
            .build()
            .unwrap();

        assert_eq!(store.github_url(), "https://github.com/owner/repo");
        assert!(!store.is_writable());
    }

    #[test]
    fn test_github_store_builder_from_url() {
        let store = GitHubStore::from_url("https://github.com/owner/repo")
            .unwrap()
            .name("url-store")
            .build()
            .unwrap();

        assert_eq!(store.github_url(), "https://github.com/owner/repo");
    }

    #[test]
    fn test_github_store_builder_writable() {
        let store = GitHubStore::builder("owner", "repo")
            .writable()
            .author("Test Author", "test@example.com")
            .build()
            .unwrap();

        assert!(store.is_writable());
    }

    #[test]
    fn test_parse_github_url_https() {
        let (owner, repo) =
            GitHubStoreBuilder::parse_github_url("https://github.com/owner/repo").unwrap();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn test_parse_github_url_invalid() {
        let result = GitHubStoreBuilder::parse_github_url("https://example.com/owner/repo");
        assert!(result.is_err());
    }
}
