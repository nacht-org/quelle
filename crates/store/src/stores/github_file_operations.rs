//! GitHub-specific file operations implementation using raw.githubusercontent.com
//!
//! This implementation uses GitHub's raw file URLs to access repository contents
//! without requiring the GitHub API, making it simpler and not subject to API rate limits.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

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
pub(crate) struct GitHubFileOperations {
    owner: String,
    repo: String,
    reference: String,
    original_reference: GitReference,
    base_url: String,
    client: reqwest::Client,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    cache_ttl: Duration,
}

impl GitHubFileOperations {
    /// Create a new GitHub file operations instance
    pub fn new(owner: String, repo: String, reference: GitReference) -> Self {
        let reference_str = reference.to_string();
        let base_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            owner, repo, reference_str
        );

        Self {
            owner,
            repo,
            reference: reference_str,
            original_reference: reference.clone(),
            base_url,
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(300), // 5 minutes default
        }
    }

    /// Create with custom client
    pub fn with_client(
        owner: String,
        repo: String,
        reference: GitReference,
        client: reqwest::Client,
    ) -> Self {
        let reference_str = reference.to_string();
        let base_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            owner, repo, reference_str
        );

        Self {
            owner,
            repo,
            reference: reference_str,
            original_reference: reference.clone(),
            base_url,
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Set the cache TTL
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
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

    /// Get effective reference by resolving default branch if needed.
    ///
    /// When GitReference::Default is used, this method will probe the repository
    /// to determine the actual default branch (main vs master) by testing common
    /// branch names. This is useful for accurate branch resolution since GitHub
    /// repositories may use either "main" or "master" as their default.
    ///
    /// # Returns
    /// - The actual branch name for Default references (e.g., "main" or "master")
    /// - The original reference string for explicit branches, tags, or commits
    ///
    /// # Example
    /// ```ignore
    /// let ops = GitHubFileOperations::new("owner".to_string(), "repo".to_string(), GitReference::Default);
    /// let actual_branch = ops.get_effective_reference().await?;
    /// // actual_branch will be "main" or "master" depending on the repo's default
    /// ```
    pub async fn get_effective_reference(&self) -> Result<String> {
        if matches!(self.original_reference, GitReference::Default) {
            // For Default reference, we need to find the actual default branch
            // We can try common default branches in order
            let default_branches = ["main", "master"];

            for branch in &default_branches {
                let test_url = format!(
                    "https://raw.githubusercontent.com/{}/{}/{}/README.md",
                    self.owner, self.repo, branch
                );

                if let Ok(response) = self.client.head(&test_url).send().await {
                    if response.status().is_success() {
                        debug!("Resolved default branch to: {}", branch);
                        return Ok(branch.to_string());
                    }
                }
            }

            // Fallback to main if we can't determine
            warn!(
                "Could not resolve default branch for {}/{}, using 'main'",
                self.owner, self.repo
            );
            Ok("main".to_string())
        } else {
            Ok(self.reference.clone())
        }
    }

    /// Resolve the default branch and update the base URL if needed.
    ///
    /// This method calls `get_effective_reference()` and updates the internal
    /// base URL to use the resolved branch. This is useful for optimizing
    /// subsequent file operations when using GitReference::Default.
    ///
    /// # Returns
    /// The resolved reference string that is now being used.
    pub async fn resolve_and_update_reference(&mut self) -> Result<String> {
        let effective_ref = self.get_effective_reference().await?;

        // Only update if the reference actually changed
        if effective_ref != self.reference {
            self.reference = effective_ref.clone();
            self.base_url = format!(
                "https://raw.githubusercontent.com/{}/{}/{}",
                self.owner, self.repo, effective_ref
            );
            debug!("Updated base URL to: {}", self.base_url);
        }

        Ok(effective_ref)
    }
}

#[async_trait]
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
        // For GitHub raw URLs, we can't directly list directories like we can with filesystem
        // We need to use the GitHub API for this. For now, we'll implement a simple approach
        // that tries to read common file patterns or uses the GitHub API.

        let api_url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
            self.owner, self.repo, path, self.reference
        );

        debug!("Listing GitHub directory via API: {}", api_url);

        let response = self
            .client
            .get(&api_url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "quelle-store")
            .send()
            .await
            .map_err(|e| {
                StoreError::NetworkError(format!(
                    "Failed to list directory {} in GitHub {}/{}: {}",
                    path, self.owner, self.repo, e
                ))
            })?;

        if response.status() == 404 {
            return Ok(Vec::new()); // Directory doesn't exist
        }

        if !response.status().is_success() {
            return Err(StoreError::NetworkError(format!(
                "GitHub API directory listing failed with status {}: {} (repo: {}/{}, path: {})",
                response.status(),
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or("Unknown error"),
                self.owner,
                self.repo,
                path
            )));
        }

        let content: serde_json::Value = response.json().await.map_err(|e| {
            StoreError::NetworkError(format!(
                "Failed to parse GitHub API response for directory {} in {}/{}: {}",
                path, self.owner, self.repo, e
            ))
        })?;

        let mut entries = Vec::new();

        if let Some(array) = content.as_array() {
            for item in array {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    entries.push(name.to_string());
                }
            }
        }

        entries.sort();
        Ok(entries)
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
        let ops = GitHubFileOperations::new(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        );

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
        let ops = GitHubFileOperations::new(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        );

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

    #[tokio::test]
    async fn test_cache_operations() {
        let ops = GitHubFileOperations::new(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        );

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
        let ops = GitHubFileOperations::new(
            "owner".to_string(),
            "repo".to_string(),
            GitReference::Branch("main".to_string()),
        );

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
