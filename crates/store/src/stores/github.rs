//! GitHub store implementation
//!
//! This module provides GitHubStore which reads individual files from GitHub
//! repositories via the GitHub API without cloning the entire repository. For publishing,
//! it uses git operations by lazy-initializing a GitProvider.

use async_trait::async_trait;
use base64::Engine;
use octocrab::Octocrab;
use serde::Deserialize;
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
use crate::stores::local::LocalStore;
use crate::stores::providers::git::{GitAuth, GitProvider, GitReference, GitWriteConfig};
use crate::stores::traits::{BaseStore, CacheStats, CacheableStore, ReadableStore, WritableStore};

/// GitHub API response for repository information
#[derive(Debug, Clone, Deserialize)]
struct GitHubRepository {
    name: String,
    full_name: String,
    default_branch: String,
    clone_url: String,
}

/// File cache entry
#[derive(Debug, Clone)]
struct CacheEntry {
    content: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
    cached_at: Instant,
}

/// GitHub store that uses GitHub API for reads and git operations for writes
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
    auth: GitAuth,
    /// Octocrab client for GitHub API requests
    client: Octocrab,
    /// Local cache directory for storing files and git operations
    cache_dir: PathBuf,
    /// In-memory file cache
    file_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Cache TTL for files
    cache_ttl: Duration,
    /// Last time we checked for updates
    last_check: RwLock<Option<Instant>>,
    /// Check interval for updates
    check_interval: Duration,
    /// Git provider for write operations (created when needed)
    _git_provider_placeholder: (),
    /// Write configuration for git operations
    write_config: Option<GitWriteConfig>,
    /// Repository information cache
    repo_info: RwLock<Option<GitHubRepository>>,
    /// Local store for extension management (lazy-initialized)
    local_store: RwLock<Option<Arc<LocalStore>>>,
}

/// Builder for creating GitHub stores with a fluent API
pub struct GitHubStoreBuilder {
    name: Option<String>,
    owner: String,
    repo: String,
    cache_dir: Option<PathBuf>,
    reference: GitReference,
    auth: GitAuth,
    cache_ttl: Duration,
    check_interval: Duration,
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
            auth: GitAuth::None,
            cache_ttl: Duration::from_secs(300),      // 5 minutes
            check_interval: Duration::from_secs(300), // 5 minutes
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
        self.auth = auth;
        self
    }

    /// Set authentication using a GitHub token
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.auth = GitAuth::Token {
            token: token.into(),
        };
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

    /// Set the check interval for updates
    pub fn check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = interval;
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

        let client = match &self.auth {
            GitAuth::Token { token } => Octocrab::builder()
                .personal_token(token.clone())
                .build()
                .map_err(|e| {
                StoreError::InvalidConfiguration(format!("Failed to create octocrab client: {}", e))
            })?,
            GitAuth::None => Octocrab::builder().build().map_err(|e| {
                StoreError::InvalidConfiguration(format!("Failed to create octocrab client: {}", e))
            })?,
            _ => Octocrab::builder().build().map_err(|e| {
                StoreError::InvalidConfiguration(format!("Failed to create octocrab client: {}", e))
            })?,
        };

        Ok(GitHubStore {
            name,
            owner: self.owner,
            repo: self.repo,
            reference: self.reference,
            auth: self.auth,
            client,
            cache_dir,
            file_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: self.cache_ttl,
            last_check: RwLock::new(None),
            check_interval: self.check_interval,
            _git_provider_placeholder: (),
            write_config: self.write_config,
            repo_info: RwLock::new(None),
            local_store: RwLock::new(None),
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
            GitAuth::None | GitAuth::Token { .. } => {
                format!("https://github.com/{}/{}.git", self.owner, self.repo)
            }
            GitAuth::SshKey { .. } => {
                format!("git@github.com:{}/{}.git", self.owner, self.repo)
            }
            GitAuth::UserPassword { .. } => {
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

    /// Create a git provider for write operations when needed
    fn create_git_provider(&self) -> Result<GitProvider> {
        let mut git_provider = GitProvider::new(
            self.git_url(),
            self.cache_dir.join("git"),
            self.reference.clone(),
            self.auth.clone(),
        );

        if let Some(ref write_config) = self.write_config {
            git_provider = git_provider.with_write_config(write_config.clone());
        }

        Ok(git_provider)
    }

    /// Get or create the local store for extension management
    async fn get_local_store(&self) -> Result<Arc<LocalStore>> {
        {
            let local_store_guard = self.local_store.read().unwrap();
            if let Some(ref local_store) = *local_store_guard {
                return Ok(local_store.clone());
            }
        }

        // Create local store in a subdirectory
        let local_path = self.cache_dir.join("extensions");
        std::fs::create_dir_all(&local_path).map_err(|e| StoreError::IoOperation {
            operation: "create local store directory".to_string(),
            path: local_path.clone(),
            source: e,
        })?;

        let local_store = Arc::new(LocalStore::new(&local_path)?);

        // Initialize if needed
        if local_store.get_store_manifest().await.is_err() {
            local_store
                .initialize_store(
                    self.name.clone(),
                    Some("GitHub-based extension store".to_string()),
                )
                .await?;
        }

        let local_store_clone = Arc::clone(&local_store);
        {
            let mut local_store_guard = self.local_store.write().unwrap();
            *local_store_guard = Some(local_store);
        }

        Ok(local_store_clone)
    }

    /// Get repository information from GitHub API
    async fn get_repository_info(&self) -> Result<GitHubRepository> {
        {
            let repo_info_guard = self.repo_info.read().unwrap();
            if let Some(ref repo_info) = *repo_info_guard {
                return Ok(repo_info.clone());
            }
        }

        let repo = self
            .client
            .repos(&self.owner, &self.repo)
            .get()
            .await
            .map_err(|e| {
                StoreError::NetworkError(format!("Failed to fetch repository info: {}", e))
            })?;

        let repo_info = GitHubRepository {
            name: repo.name,
            full_name: repo
                .full_name
                .unwrap_or_else(|| format!("{}/{}", self.owner, self.repo)),
            default_branch: repo.default_branch.unwrap_or_else(|| "main".to_string()),
            clone_url: repo
                .clone_url
                .map(|url| url.to_string())
                .unwrap_or_else(|| format!("https://github.com/{}/{}.git", self.owner, self.repo)),
        };

        {
            let mut repo_info_guard = self.repo_info.write().unwrap();
            *repo_info_guard = Some(repo_info.clone());
        }

        Ok(repo_info)
    }

    /// Get the effective reference (resolve default branch)
    async fn get_effective_reference(&self) -> Result<String> {
        match &self.reference {
            GitReference::Default => {
                let repo_info = self.get_repository_info().await?;
                Ok(repo_info.default_branch)
            }
            GitReference::Branch(branch) => Ok(branch.clone()),
            GitReference::Tag(tag) => Ok(tag.clone()),
            GitReference::Commit(commit) => Ok(commit.clone()),
        }
    }

    /// Read a file from GitHub API
    /// Read a file from the GitHub repository
    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        // Check cache first
        if let Some(cached_content) = self.get_cached_file(path) {
            return Ok(cached_content);
        }

        let reference = self.get_effective_reference().await?;

        debug!(
            "Fetching file from GitHub API: {}/{} at {}",
            self.owner, self.repo, path
        );

        let content = self
            .client
            .repos(&self.owner, &self.repo)
            .get_content()
            .path(path)
            .r#ref(&reference)
            .send()
            .await
            .map_err(|e| {
                // Check if it's a 404 error by examining the error message
                let error_str = e.to_string();
                if error_str.contains("404") || error_str.contains("Not Found") {
                    StoreError::ExtensionNotFound(path.to_string())
                } else {
                    StoreError::NetworkError(format!("Failed to fetch file {}: {}", path, e))
                }
            })?;

        let content_bytes = match content.items.first() {
            Some(item) => {
                if let Some(content_str) = &item.content {
                    if item.encoding.as_deref() == Some("base64") {
                        base64::engine::general_purpose::STANDARD
                            .decode(content_str.replace('\n', ""))
                            .map_err(|e| {
                                StoreError::NetworkError(format!(
                                    "Failed to decode base64 content for {}: {}",
                                    path, e
                                ))
                            })?
                    } else {
                        content_str.clone().into_bytes()
                    }
                } else if let Some(download_url) = &item.download_url {
                    // For large files, GitHub provides a download URL
                    let download_response = reqwest::get(download_url).await.map_err(|e| {
                        StoreError::NetworkError(format!(
                            "Failed to download large file {}: {}",
                            path, e
                        ))
                    })?;

                    download_response
                        .bytes()
                        .await
                        .map_err(|e| {
                            StoreError::NetworkError(format!(
                                "Failed to read download content for {}: {}",
                                path, e
                            ))
                        })?
                        .to_vec()
                } else {
                    return Err(StoreError::NetworkError(format!(
                        "No content available for file: {}",
                        path
                    )));
                }
            }
            None => {
                return Err(StoreError::ExtensionNotFound(path.to_string()));
            }
        };

        // Cache the file (we'll use None for etag and last_modified since octocrab doesn't expose headers easily)
        self.cache_file(path, &content_bytes, None, None);

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
    fn cache_file(
        &self,
        path: &str,
        content: &[u8],
        etag: Option<String>,
        last_modified: Option<String>,
    ) {
        let mut cache = self.file_cache.write().unwrap();
        cache.insert(
            path.to_string(),
            CacheEntry {
                content: content.to_vec(),
                etag,
                last_modified,
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
        let local_store = self.get_local_store().await?;
        local_store.get_store_manifest().await
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        // Check GitHub API connectivity
        match self.get_repository_info().await {
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
                error: Some(format!("GitHub API error: {}", e)),
                extension_count: None,
                store_version: None,
            }),
        }
    }
}

#[async_trait]
impl ReadableStore for GitHubStore {
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        let local_store = self.get_local_store().await?;
        local_store.find_extensions_for_url(url).await
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        let local_store = self.get_local_store().await?;
        local_store.list_extensions().await
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        let local_store = self.get_local_store().await?;
        local_store.search_extensions(query).await
    }

    async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>> {
        let local_store = self.get_local_store().await?;
        local_store.get_extension_info(name).await
    }

    async fn get_extension_version_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        let local_store = self.get_local_store().await?;
        local_store.get_extension_version_info(name, version).await
    }

    async fn get_extension_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
        let local_store = self.get_local_store().await?;
        local_store.get_extension_manifest(name, version).await
    }

    async fn get_extension_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        let local_store = self.get_local_store().await?;
        local_store.get_extension_metadata(name, version).await
    }

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        let local_store = self.get_local_store().await?;
        local_store.get_extension_package(id, version).await
    }

    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<String>> {
        let local_store = self.get_local_store().await?;
        local_store.get_extension_latest_version(id).await
    }

    async fn list_extension_versions(&self, id: &str) -> Result<Vec<String>> {
        let local_store = self.get_local_store().await?;
        local_store.list_extension_versions(id).await
    }

    async fn check_extension_version_exists(&self, id: &str, version: &str) -> Result<bool> {
        let local_store = self.get_local_store().await?;
        local_store
            .check_extension_version_exists(id, version)
            .await
    }

    async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>> {
        let local_store = self.get_local_store().await?;
        local_store.check_extension_updates(installed).await
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
        package: ExtensionPackage,
        options: PublishOptions,
    ) -> Result<PublishResult> {
        if !self.is_writable() {
            return Err(StoreError::PermissionDenied(
                "Store is not configured for write operations".to_string(),
            ));
        }

        // First publish to local store
        let local_store = self.get_local_store().await?;
        let local_result = local_store.publish(package, options.clone()).await?;

        // Note: For now, we don't have direct git provider event handling
        // This would be implemented when we add proper git write support

        Ok(local_result)
    }

    async fn unpublish(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        if !self.is_writable() {
            return Err(StoreError::PermissionDenied(
                "Store is not configured for write operations".to_string(),
            ));
        }

        // First unpublish from local store
        let local_store = self.get_local_store().await?;
        let local_result = local_store.unpublish(extension_id, options.clone()).await?;

        // Note: For now, we don't have direct git provider event handling
        // This would be implemented when we add proper git write support

        Ok(local_result)
    }

    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport> {
        let local_store = self.get_local_store().await?;
        local_store.validate_package(package, options).await
    }
}

#[async_trait]
impl CacheableStore for GitHubStore {
    async fn refresh_cache(&self) -> Result<()> {
        // Clear memory cache
        self.clear_cache();

        // Refresh repository info
        {
            let mut repo_info_guard = self.repo_info.write().unwrap();
            *repo_info_guard = None;
        }

        // Get fresh repository info
        let _ = self.get_repository_info().await?;

        // Update last check time
        {
            let mut last_check = self.last_check.write().unwrap();
            *last_check = Some(Instant::now());
        }

        Ok(())
    }

    async fn clear_cache(&self) -> Result<()> {
        self.clear_cache();
        Ok(())
    }

    async fn cache_stats(&self) -> Result<CacheStats> {
        let (valid, total) = self.cache_stats();
        let last_refresh = *self.last_check.read().unwrap();

        Ok(CacheStats {
            entries: valid,
            size_bytes: 0, // We don't track size in memory cache
            hit_rate: if total > 0 {
                valid as f64 / total as f64
            } else {
                0.0
            },
            last_refresh: last_refresh.map(|instant| {
                chrono::Utc::now()
                    - chrono::Duration::from_std(instant.elapsed()).unwrap_or_default()
            }),
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
            last_check: RwLock::new(*self.last_check.read().unwrap()),
            check_interval: self.check_interval,
            _git_provider_placeholder: (), // Placeholder for git provider
            write_config: self.write_config.clone(),
            repo_info: RwLock::new(self.repo_info.read().unwrap().clone()),
            local_store: RwLock::new(None), // Don't clone the local store, let it be lazy-initialized
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
