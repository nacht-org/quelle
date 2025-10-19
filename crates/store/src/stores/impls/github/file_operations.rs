//! GitHub file operations using raw.githubusercontent.com URLs

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

use super::api;
use crate::error::{Result, StoreError};
use crate::stores::file_operations::FileOperations;
use crate::stores::providers::git::GitReference;

/// Cache entry for GitHub file operations
#[derive(Debug, Clone)]
struct CacheEntry {
    content: Vec<u8>,
    cached_at: Instant,
}

/// File operations implementation for GitHub repositories using raw URLs
pub struct GitHubFileOperations {
    owner: String,
    repo: String,
    reference: String,
    base_url: String,
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    cache_ttl: Duration,
}

/// Builder for GitHubFileOperations
pub struct GitHubFileOperationsBuilder {
    owner: String,
    repo: String,
    reference: GitReference,
    client: Option<reqwest::Client>,
    cache_ttl: Duration,
}

impl GitHubFileOperations {
    /// Create a new builder
    pub fn builder(
        owner: String,
        repo: String,
        reference: GitReference,
    ) -> GitHubFileOperationsBuilder {
        GitHubFileOperationsBuilder {
            owner,
            repo,
            reference,
            client: None,
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Internal constructor with resolved reference (used by builder)
    pub(crate) fn new_with_resolved_reference(
        owner: String,
        repo: String,
        resolved_reference: String,
        client: reqwest::Client,
        cache_ttl: Duration,
    ) -> Self {
        let base_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            owner, repo, resolved_reference
        );

        Self {
            owner,
            repo,
            reference: resolved_reference,
            base_url,
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl,
        }
    }

    /// Get repository information
    pub fn repo_info(&self) -> (&str, &str, &str) {
        (&self.owner, &self.repo, &self.reference)
    }

    /// Get the base URL for raw file access
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get cached file content if still valid
    async fn get_cached_file(&self, path: &str) -> Option<Vec<u8>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(path) {
            if entry.cached_at.elapsed() < self.cache_ttl {
                debug!("Cache hit for GitHub file: {}", path);
                return Some(entry.content.clone());
            }
        }
        None
    }

    /// Cache file content
    async fn cache_file(&self, path: &str, content: &[u8]) {
        let mut cache = self.cache.write().await;
        cache.insert(
            path.to_string(),
            CacheEntry {
                content: content.to_vec(),
                cached_at: Instant::now(),
            },
        );
        debug!("Cached GitHub file: {}", path);
    }

    /// Clear the entire cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Cleared GitHub file cache for {}/{}", self.owner, self.repo);
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, u64) {
        let cache = self.cache.read().await;
        let entries = cache.len();
        let size_bytes = cache.values().map(|entry| entry.content.len() as u64).sum();
        (entries, size_bytes)
    }

    /// Construct the full raw URL for a path
    fn construct_raw_url(&self, path: &str) -> String {
        let clean_path = path.trim_start_matches('/');
        format!("{}/{}", self.base_url, clean_path)
    }
}

impl GitHubFileOperationsBuilder {
    /// Set custom HTTP client
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = Some(client);
        self
    }

    /// Set cache TTL
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Build naively (assumes "main" for default branch)
    pub fn build_naive(self) -> GitHubFileOperations {
        let client = self.client.unwrap_or_else(|| api::create_default_client());
        let resolved_reference = self.reference.to_string();

        GitHubFileOperations::new_with_resolved_reference(
            self.owner,
            self.repo,
            resolved_reference,
            client,
            self.cache_ttl,
        )
    }

    /// Build with accurate default branch resolution via GitHub API
    pub async fn build(self) -> Result<GitHubFileOperations> {
        let client = self.client.unwrap_or_else(|| api::create_default_client());
        let resolved_reference =
            Self::resolve_reference_static(&self.reference, &self.owner, &self.repo, &client)
                .await?;

        Ok(GitHubFileOperations::new_with_resolved_reference(
            self.owner,
            self.repo,
            resolved_reference,
            client,
            self.cache_ttl,
        ))
    }

    /// Resolve the reference to an actual branch name (static version)
    ///
    /// For GitReference::Default, this method:
    /// 1. First tries the GitHub API to get the repository's actual default branch
    /// 2. Falls back to testing common branch names ("main", "master")
    /// 3. Finally defaults to "main" if nothing else works
    ///
    /// This approach is more reliable than assuming specific files exist.
    async fn resolve_reference_static(
        reference: &GitReference,
        owner: &str,
        repo: &str,
        client: &reqwest::Client,
    ) -> Result<String> {
        api::resolve_git_reference(client, owner, repo, reference).await
    }
}

impl FileOperations for GitHubFileOperations {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        // Check cache first
        if let Some(cached_content) = self.get_cached_file(path).await {
            return Ok(cached_content);
        }

        let url = self.construct_raw_url(path);
        debug!("Fetching file from GitHub raw URL: {}", url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            StoreError::NetworkError(format!(
                "Failed to fetch file {} from GitHub {}/{}: {}",
                path, self.owner, self.repo, e
            ))
        })?;

        if response.status() == 404 {
            return Err(StoreError::ExtensionNotFound(format!(
                "File not found in GitHub repository {}/{}: {}",
                self.owner, self.repo, path
            )));
        }

        if !response.status().is_success() {
            return Err(StoreError::NetworkError(format!(
                "GitHub raw file request failed with status {}: {} (repo: {}/{})",
                response.status(),
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error"),
                self.owner,
                self.repo
            )));
        }

        let content_bytes = response
            .bytes()
            .await
            .map_err(|e| {
                StoreError::NetworkError(format!(
                    "Failed to read response content for {} from {}/{}: {}",
                    path, self.owner, self.repo, e
                ))
            })?
            .to_vec();

        // Cache the file
        self.cache_file(path, &content_bytes).await;

        Ok(content_bytes)
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        // Check cache first
        if self.get_cached_file(path).await.is_some() {
            return Ok(true);
        }

        let url = self.construct_raw_url(path);
        debug!("Checking if GitHub file exists: {}", url);

        let response = self.client.head(&url).send().await.map_err(|e| {
            StoreError::NetworkError(format!(
                "Failed to check file {} in GitHub {}/{}: {}",
                path, self.owner, self.repo, e
            ))
        })?;

        match response.status().as_u16() {
            200 => Ok(true),
            404 => Ok(false),
            _ => Err(StoreError::NetworkError(format!(
                "GitHub file existence check failed with status {}: {} (repo: {}/{}, file: {})",
                response.status(),
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error"),
                self.owner,
                self.repo,
                path
            ))),
        }
    }

    async fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        // Use centralized GitHub API function
        api::list_directory_names(&self.client, &self.owner, &self.repo, path, &self.reference)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_reference_to_string() {
        assert_eq!(GitReference::Branch("main".to_string()).to_string(), "main");
        assert_eq!(
            GitReference::Tag("v1.0.0".to_string()).to_string(),
            "v1.0.0"
        );
        assert_eq!(
            GitReference::Commit("abc123".to_string()).to_string(),
            "abc123"
        );
        assert_eq!(GitReference::Default.to_string(), "main");
    }

    #[test]
    fn test_github_file_operations_creation() {
        let ops = GitHubFileOperations::builder(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        )
        .build_naive();

        let (owner, repo, reference) = ops.repo_info();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
        assert_eq!(reference, "main");

        assert_eq!(
            ops.base_url(),
            "https://raw.githubusercontent.com/owner/repo/main"
        );
    }

    #[test]
    fn test_raw_url_construction() {
        let ops = GitHubFileOperations::builder(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        )
        .build_naive();

        assert_eq!(
            ops.construct_raw_url("file.txt"),
            "https://raw.githubusercontent.com/owner/repo/main/file.txt"
        );
        assert_eq!(
            ops.construct_raw_url("/file.txt"),
            "https://raw.githubusercontent.com/owner/repo/main/file.txt"
        );
        assert_eq!(
            ops.construct_raw_url("dir/file.txt"),
            "https://raw.githubusercontent.com/owner/repo/main/dir/file.txt"
        );
    }

    #[test]
    fn test_builder_pattern() {
        let ops = GitHubFileOperations::builder(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("develop".to_string()),
        )
        .with_cache_ttl(Duration::from_secs(600))
        .build_naive();

        let (owner, repo, reference) = ops.repo_info();
        assert_eq!(owner, "owner");
        assert_eq!(repo, "repo");
        assert_eq!(reference, "develop");

        assert_eq!(
            ops.base_url(),
            "https://raw.githubusercontent.com/owner/repo/develop"
        );
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let ops = GitHubFileOperations::builder(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        )
        .build_naive();

        // Initially no cache
        assert!(ops.get_cached_file("test.txt").await.is_none());

        // Cache a file
        let content = b"test content";
        ops.cache_file("test.txt", content).await;

        // Should now be cached
        let cached = ops.get_cached_file("test.txt").await;
        assert_eq!(cached, Some(content.to_vec()));

        // Clear cache
        ops.clear_cache().await;
        assert!(ops.get_cached_file("test.txt").await.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let ops = GitHubFileOperations::builder(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        )
        .build_naive();

        let (entries, size) = ops.cache_stats().await;
        assert_eq!(entries, 0);
        assert_eq!(size, 0);

        ops.cache_file("test1.txt", b"content1").await;
        ops.cache_file("test2.txt", b"longer content").await;

        let (entries, size) = ops.cache_stats().await;
        assert_eq!(entries, 2);
        assert_eq!(size, 8 + 14); // "content1" + "longer content"
    }
}
