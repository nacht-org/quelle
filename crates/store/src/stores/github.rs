//! GitHub store implementation
//!
//! This module provides GitHubStore which reads individual files from GitHub
//! repositories via the GitHub API without cloning the entire repository. For publishing,
//! it uses git operations by lazy-initializing a GitProvider.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::error::{Result, StoreError};
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstalledExtension, SearchQuery,
    StoreHealth, UpdateInfo,
};
use crate::publish::ExtensionVisibility;
use crate::publish::{
    PublishOptions, PublishRequirements, PublishResult, UnpublishOptions, UnpublishResult,
    ValidationReport,
};
use crate::store_manifest::StoreManifest;

use crate::stores::providers::git::{GitAuth, GitReference, GitWriteConfig};
use crate::stores::traits::{BaseStore, CacheStats, CacheableStore, ReadableStore, WritableStore};

/// File cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    content: Vec<u8>,
    cached_at: Instant,
}

/// GitHub store that uses raw GitHub URLs for efficient file reading
pub struct GitHubStore {
    /// Store name
    name: String,
    /// Repository owner (user or organization)
    owner: String,
    /// Repository name
    repo: String,
    /// Git reference to use (branch, tag, or commit)
    reference: GitReference,
    /// Authentication for GitHub API
    auth: Option<GitAuth>,
    /// HTTP client for GitHub requests
    client: reqwest::Client,
    /// Local cache directory for storing files and git operations
    cache_dir: PathBuf,
    /// In-memory file cache
    file_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Cache TTL for files
    cache_ttl: Duration,
    /// Write configuration for git operations
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

        if let Some(path) = url.strip_prefix("git@github.com:") {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() >= 2 {
                return Ok((parts[0].to_string(), parts[1].to_string()));
            }
        }

        Err(StoreError::InvalidConfiguration(format!(
            "Invalid GitHub URL: {}",
            url
        )))
    }

    /// Set the name for this store
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the cache directory where the repository data will be stored
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(path.into());
        self
    }

    /// Set authentication for the GitHub API and git operations
    pub fn auth(mut self, auth: GitAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set authentication using a GitHub token
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

    /// Set the cache TTL for files (how long to keep files in memory)
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

    /// Build the GitHubStore
    pub fn build(self) -> Result<GitHubStore> {
        let cache_dir = self.cache_dir.ok_or_else(|| {
            StoreError::InvalidConfiguration(
                "cache_dir must be set before building GitHubStore".to_string(),
            )
        })?;

        let name = self.name.ok_or_else(|| {
            StoreError::InvalidConfiguration(
                "name must be set before building GitHubStore".to_string(),
            )
        })?;

        let client = reqwest::Client::builder()
            .user_agent("quelle-store/0.1.0")
            .build()
            .map_err(|e| {
                StoreError::InvalidConfiguration(format!("Failed to create reqwest client: {}", e))
            })?;

        Ok(GitHubStore {
            name,
            owner: self.owner,
            repo: self.repo,
            reference: self.reference,
            auth: self.auth.clone(),
            client,
            cache_dir,
            file_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: self.cache_ttl,
            write_config: self.write_config,
        })
    }
}

impl GitHubStore {
    /// Create a new GitHub store builder
    pub fn builder(owner: impl Into<String>, repo: impl Into<String>) -> GitHubStoreBuilder {
        GitHubStoreBuilder::new(owner, repo)
    }

    /// Create a GitHub store builder from a GitHub URL
    pub fn from_url(url: impl AsRef<str>) -> Result<GitHubStoreBuilder> {
        GitHubStoreBuilder::from_url(url)
    }

    /// Get the GitHub repository URL
    pub fn github_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo)
    }

    /// Get the git clone URL
    pub fn git_url(&self) -> String {
        match &self.auth {
            Some(GitAuth::SshKey { .. }) => {
                format!("git@github.com:{}/{}.git", self.owner, self.repo)
            }
            _ => {
                format!("https://github.com/{}/{}.git", self.owner, self.repo)
            }
        }
    }

    /// Get the cache directory where the repository data is stored
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Check if this GitHub store supports writing operations
    pub fn is_writable(&self) -> bool {
        self.write_config.is_some()
    }

    /// Get the effective reference (resolve default branch)
    /// Get the effective reference to use for file reads
    async fn get_effective_reference(&self) -> Result<String> {
        match &self.reference {
            GitReference::Default => Ok("main".to_string()), // Default to main for raw URLs
            GitReference::Branch(branch) => Ok(branch.clone()),
            GitReference::Tag(tag) => Ok(tag.clone()),
            GitReference::Commit(commit) => Ok(commit.clone()),
        }
    }

    /// Read a file from GitHub using raw URLs
    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        // Check cache first
        if let Some(cached_content) = self.get_cached_file(path) {
            return Ok(cached_content);
        }

        let reference = self.get_effective_reference().await?;

        // Construct raw GitHub URL
        let raw_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            self.owner, self.repo, reference, path
        );

        debug!("Fetching file from raw GitHub URL: {}", raw_url);

        let response = self.client.get(&raw_url).send().await.map_err(|e| {
            StoreError::NetworkError(format!("Failed to fetch file {}: {}", path, e))
        })?;

        if response.status() == 404 {
            return Err(StoreError::ExtensionNotFound(path.to_string()));
        }

        if !response.status().is_success() {
            return Err(StoreError::NetworkError(format!(
                "HTTP error {} when fetching file {}: {}",
                response.status(),
                path,
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error")
            )));
        }

        let content_bytes = response
            .bytes()
            .await
            .map_err(|e| {
                StoreError::NetworkError(format!(
                    "Failed to read response content for {}: {}",
                    path, e
                ))
            })?
            .to_vec();

        // Cache the file
        self.cache_file(path, &content_bytes);

        Ok(content_bytes)
    }

    /// Get cached file content
    fn get_cached_file(&self, path: &str) -> Option<Vec<u8>> {
        let cache = self.file_cache.read().unwrap();
        if let Some(entry) = cache.get(path) {
            if entry.cached_at.elapsed() < self.cache_ttl {
                debug!("Cache hit for file: {}", path);
                return Some(entry.content.clone());
            }
        }
        None
    }

    /// Cache file in memory
    fn cache_file(&self, path: &str, content: &[u8]) {
        let mut cache = self.file_cache.write().unwrap();
        cache.insert(
            path.to_string(),
            CacheEntry {
                content: content.to_vec(),
                cached_at: Instant::now(),
            },
        );
    }

    /// Clear the file cache
    pub fn clear_cache(&self) {
        let mut cache = self.file_cache.write().unwrap();
        cache.clear();
        info!("Cleared GitHub store file cache");
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.file_cache.read().unwrap();
        let total_entries = cache.len();
        let valid_entries = cache
            .values()
            .filter(|entry| entry.cached_at.elapsed() < self.cache_ttl)
            .count();
        (valid_entries, total_entries)
    }
}

#[async_trait]
impl BaseStore for GitHubStore {
    async fn get_store_manifest(&self) -> Result<StoreManifest> {
        // Return the GitHub store's own manifest, not the local store's
        Ok(
            StoreManifest::new(self.name.clone(), "github".to_string(), "1.0.0".to_string())
                .with_url(self.github_url())
                .with_description("GitHub-based extension store".to_string()),
        )
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        // Check GitHub raw URL connectivity by trying to fetch a manifest file
        let test_result = if let Ok(content) = self.read_file("store.json").await {
            Ok(content)
        } else if let Ok(content) = self.read_file("manifest.json").await {
            Ok(content)
        } else if let Ok(content) = self.read_file("README.md").await {
            Ok(content)
        } else {
            self.read_file("README").await
        };

        match test_result {
            Ok(_) => Ok(StoreHealth {
                healthy: true,
                last_check: chrono::Utc::now(),
                response_time: None,
                error: None,
                extension_count: None,
                store_version: None,
            }),
            Err(e) => Ok(StoreHealth {
                healthy: false,
                last_check: chrono::Utc::now(),
                response_time: None,
                error: Some(format!("GitHub raw URL error: {}", e)),
                extension_count: None,
                store_version: None,
            }),
        }
    }
}

#[async_trait]
impl ReadableStore for GitHubStore {
    async fn find_extensions_for_url(&self, _url: &str) -> Result<Vec<(String, String)>> {
        // GitHub store doesn't support URL-based extension discovery yet
        Ok(vec![])
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        // Try to read extensions from a directory structure in the GitHub repo
        // For now, return empty - this would need to be implemented based on
        // the expected directory structure in the GitHub repository
        Ok(vec![])
    }

    async fn search_extensions(&self, _query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        // GitHub store doesn't support searching yet
        Ok(vec![])
    }

    async fn get_extension_info(&self, _name: &str) -> Result<Vec<ExtensionInfo>> {
        // Would need to read extension info from GitHub repo structure
        Ok(vec![])
    }

    async fn get_extension_version_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        // Try to read extension manifest from GitHub
        let version = version.unwrap_or("latest");
        let manifest_path = format!("extensions/{}/{}/manifest.json", name, version);
        let manifest_content = self.read_file(&manifest_path).await?;
        let manifest: ExtensionManifest = serde_json::from_slice(&manifest_content)?;

        Ok(ExtensionInfo {
            id: manifest.id,
            name: manifest.name,
            version: manifest.version,
            description: None, // ExtensionManifest doesn't have description
            author: manifest.author,
            tags: vec![],       // ExtensionManifest doesn't have tags
            last_updated: None, // Could be derived from Git info
            download_count: None,
            size: None,
            homepage: None,   // ExtensionManifest doesn't have homepage
            repository: None, // ExtensionManifest doesn't have repository
            license: None,    // ExtensionManifest doesn't have license
            store_source: self.name.clone(),
        })
    }

    async fn get_extension_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
        let version = version.unwrap_or("latest");
        let manifest_path = format!("extensions/{}/{}/manifest.json", name, version);
        let manifest_content = self.read_file(&manifest_path).await?;
        let manifest: ExtensionManifest = serde_json::from_slice(&manifest_content)?;
        Ok(manifest)
    }

    async fn get_extension_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        let version = version.unwrap_or("latest");
        let metadata_path = format!("extensions/{}/{}/metadata.json", name, version);
        match self.read_file(&metadata_path).await {
            Ok(content) => {
                let metadata: ExtensionMetadata = serde_json::from_slice(&content)?;
                Ok(Some(metadata))
            }
            Err(StoreError::ExtensionNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        let version = version.unwrap_or("latest");

        // Read manifest
        let manifest = self.get_extension_manifest(id, Some(version)).await?;

        // Read WASM file
        let wasm_path = format!("extensions/{}/{}/extension.wasm", id, version);
        let wasm_content = self.read_file(&wasm_path).await?;

        // Read metadata if available
        let metadata = self.get_extension_metadata(id, Some(version)).await?;

        Ok(ExtensionPackage {
            manifest,
            wasm_component: wasm_content,
            metadata,
            assets: std::collections::HashMap::new(), // TODO: implement asset reading
            source_store: self.name.clone(),
        })
    }

    async fn get_extension_latest_version(&self, _id: &str) -> Result<Option<String>> {
        // Would need to scan directory structure or use Git tags
        Ok(None)
    }

    async fn list_extension_versions(&self, _id: &str) -> Result<Vec<String>> {
        // Would need to scan directory structure
        Ok(vec![])
    }

    async fn check_extension_version_exists(&self, id: &str, version: &str) -> Result<bool> {
        let manifest_path = format!("extensions/{}/{}/manifest.json", id, version);
        match self.read_file(&manifest_path).await {
            Ok(_) => Ok(true),
            Err(StoreError::ExtensionNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn check_extension_updates(
        &self,
        _installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>> {
        // Would need to implement version comparison
        Ok(vec![])
    }
}

#[async_trait]
impl WritableStore for GitHubStore {
    fn publish_requirements(&self) -> PublishRequirements {
        PublishRequirements {
            requires_authentication: true,
            requires_signing: false,
            max_package_size: Some(50 * 1024 * 1024), // 50MB
            allowed_file_extensions: vec!["wasm".to_string(), "json".to_string()],
            forbidden_patterns: vec![],
            required_metadata: vec!["name".to_string(), "version".to_string()],
            supported_visibility: vec![ExtensionVisibility::Public, ExtensionVisibility::Private],
            enforces_versioning: true,
            validation_rules: vec!["manifest".to_string(), "wasm".to_string()],
        }
    }

    async fn publish(
        &self,
        _package: ExtensionPackage,
        _options: PublishOptions,
    ) -> Result<PublishResult> {
        if !self.is_writable() {
            return Err(StoreError::PermissionDenied(
                "Store is not configured for write operations".to_string(),
            ));
        }

        // TODO: Implement direct GitHub publishing via Git operations
        Err(StoreError::InvalidConfiguration(
            "Publishing to GitHub store not yet implemented".to_string(),
        ))
    }

    async fn unpublish(
        &self,
        _extension_id: &str,
        _options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        if !self.is_writable() {
            return Err(StoreError::PermissionDenied(
                "Store is not configured for write operations".to_string(),
            ));
        }

        // TODO: Implement direct GitHub unpublishing via Git operations
        Err(StoreError::InvalidConfiguration(
            "Unpublishing from GitHub store not yet implemented".to_string(),
        ))
    }

    async fn validate_package(
        &self,
        _package: &ExtensionPackage,
        _options: &PublishOptions,
    ) -> Result<ValidationReport> {
        // TODO: Implement GitHub-specific validation
        Err(StoreError::InvalidConfiguration(
            "Package validation for GitHub store not yet implemented".to_string(),
        ))
    }
}

#[async_trait]
impl CacheableStore for GitHubStore {
    async fn refresh_cache(&self) -> Result<()> {
        // Clear memory cache
        self.clear_cache();

        // Refresh repository info
        // Clear the file cache to force refresh
        self.clear_cache();

        Ok(())
    }

    async fn clear_cache(&self) -> Result<()> {
        self.clear_cache();
        Ok(())
    }

    async fn cache_stats(&self) -> Result<CacheStats> {
        let (valid, total) = self.cache_stats();

        Ok(CacheStats {
            entries: valid,
            size_bytes: 0, // We don't track size in memory cache
            hit_rate: if total > 0 {
                valid as f64 / total as f64
            } else {
                0.0
            },
            last_refresh: None, // Simplified - no tracking of refresh time
        })
    }
}

impl Clone for GitHubStore {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            reference: self.reference.clone(),
            auth: self.auth.clone(),
            client: self.client.clone(),
            cache_dir: self.cache_dir.clone(),
            file_cache: Arc::clone(&self.file_cache),
            cache_ttl: self.cache_ttl,
            write_config: self.write_config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_github_store_builder_basic() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitHubStore::builder("octocat", "Hello-World")
            .cache_dir(temp_dir.path())
            .name("test-store")
            .build()
            .unwrap();

        assert_eq!(store.github_url(), "https://github.com/octocat/Hello-World");
        assert!(!store.is_writable());
    }

    #[test]
    fn test_github_store_builder_from_url() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitHubStore::from_url("https://github.com/octocat/Hello-World")
            .unwrap()
            .cache_dir(temp_dir.path())
            .name("test-store")
            .build()
            .unwrap();

        assert_eq!(store.github_url(), "https://github.com/octocat/Hello-World");
    }

    #[test]
    fn test_github_store_builder_writable() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitHubStore::builder("octocat", "Hello-World")
            .cache_dir(temp_dir.path())
            .name("writable-store")
            .writable()
            .author("Test Author", "test@example.com")
            .token("ghp_test_token")
            .build()
            .unwrap();

        assert!(store.is_writable());
    }

    #[test]
    fn test_parse_github_url_https() {
        let (owner, repo) =
            GitHubStoreBuilder::parse_github_url("https://github.com/octocat/Hello-World").unwrap();
        assert_eq!(owner, "octocat");
        assert_eq!(repo, "Hello-World");
    }

    #[test]
    fn test_parse_github_url_invalid() {
        let result = GitHubStoreBuilder::parse_github_url("https://gitlab.com/octocat/Hello-World");
        assert!(result.is_err());
    }
}
