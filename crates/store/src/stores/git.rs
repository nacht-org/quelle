//! Git store type alias and convenience functions
//!
//! This module provides type aliases and convenience functions for creating
//! git-based stores using LocallyCachedStore with GitProvider.

#[cfg(feature = "git")]
use std::path::PathBuf;
#[cfg(feature = "git")]
use std::time::Duration;

#[cfg(feature = "git")]
use crate::error::Result;
#[cfg(feature = "git")]
use crate::stores::{
    locally_cached::LocallyCachedStore,
    providers::{GitAuth, GitProvider, GitReference},
    traits::WritableStore,
};

/// Type alias for a git-based store
///
/// This is a LocallyCachedStore that uses a GitProvider to sync data
/// from a git repository to local storage.
#[cfg(feature = "git")]
pub type GitStore = LocallyCachedStore<GitProvider>;

/// Convenience functions for creating git stores
#[cfg(feature = "git")]
impl GitStore {
    /// Create a new git store with default settings
    ///
    /// # Arguments
    /// * `name` - Human-readable name for the store
    /// * `url` - Git repository URL
    /// * `cache_dir` - Local directory where the repository will be cached
    ///
    /// # Examples
    /// ```rust
    /// use quelle_store::GitStore;
    /// use tempfile::TempDir;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let temp_dir = TempDir::new()?;
    /// let store = GitStore::from_url(
    ///     "example-store".to_string(),
    ///     "https://github.com/user/extensions-repo.git".to_string(),
    ///     temp_dir.path().to_path_buf(),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_url(name: String, url: String, cache_dir: PathBuf) -> Result<Self> {
        let provider =
            GitProvider::new(url, cache_dir.clone(), GitReference::Default, GitAuth::None);
        LocallyCachedStore::new(provider, cache_dir, name)
    }

    /// Create a new git store with authentication
    ///
    /// # Arguments
    /// * `name` - Human-readable name for the store
    /// * `url` - Git repository URL
    /// * `cache_dir` - Local directory where the repository will be cached
    /// * `auth` - Authentication configuration
    ///
    /// # Examples
    /// ```rust
    /// use quelle_store::{GitStore, GitAuth};
    /// use tempfile::TempDir;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let temp_dir = TempDir::new()?;
    /// let auth = GitAuth::Token { token: "ghp_xxxx".to_string() };
    /// let store = GitStore::with_auth(
    ///     "private-store".to_string(),
    ///     "https://github.com/user/private-repo.git".to_string(),
    ///     temp_dir.path().to_path_buf(),
    ///     auth,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_auth(name: String, url: String, cache_dir: PathBuf, auth: GitAuth) -> Result<Self> {
        let provider = GitProvider::new(url, cache_dir.clone(), GitReference::Default, auth);
        LocallyCachedStore::new(provider, cache_dir, name)
    }

    /// Create a new git store with a specific branch
    ///
    /// # Arguments
    /// * `name` - Human-readable name for the store
    /// * `url` - Git repository URL
    /// * `cache_dir` - Local directory where the repository will be cached
    /// * `branch` - Branch name to checkout
    ///
    /// # Examples
    /// ```rust
    /// use quelle_store::GitStore;
    /// use tempfile::TempDir;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let temp_dir = TempDir::new()?;
    /// let store = GitStore::with_branch(
    ///     "dev-store".to_string(),
    ///     "https://github.com/user/repo.git".to_string(),
    ///     temp_dir.path().to_path_buf(),
    ///     "develop".to_string(),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_branch(
        name: String,
        url: String,
        cache_dir: PathBuf,
        branch: String,
    ) -> Result<Self> {
        let provider = GitProvider::new(
            url,
            cache_dir.clone(),
            GitReference::Branch(branch),
            GitAuth::None,
        );
        LocallyCachedStore::new(provider, cache_dir, name)
    }

    /// Create a new git store with a specific tag
    ///
    /// # Arguments
    /// * `name` - Human-readable name for the store
    /// * `url` - Git repository URL
    /// * `cache_dir` - Local directory where the repository will be cached
    /// * `tag` - Tag name to checkout
    ///
    /// # Examples
    /// ```rust
    /// use quelle_store::GitStore;
    /// use tempfile::TempDir;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let temp_dir = TempDir::new()?;
    /// let store = GitStore::with_tag(
    ///     "stable-store".to_string(),
    ///     "https://github.com/user/repo.git".to_string(),
    ///     temp_dir.path().to_path_buf(),
    ///     "v1.0.0".to_string(),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_tag(name: String, url: String, cache_dir: PathBuf, tag: String) -> Result<Self> {
        let provider = GitProvider::new(
            url,
            cache_dir.clone(),
            GitReference::Tag(tag),
            GitAuth::None,
        );
        LocallyCachedStore::new(provider, cache_dir, name)
    }

    /// Create a new git store with a specific commit
    ///
    /// # Arguments
    /// * `name` - Human-readable name for the store
    /// * `url` - Git repository URL
    /// * `cache_dir` - Local directory where the repository will be cached
    /// * `commit` - Commit hash to checkout
    ///
    /// # Examples
    /// ```rust
    /// use quelle_store::GitStore;
    /// use tempfile::TempDir;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let temp_dir = TempDir::new()?;
    /// let store = GitStore::with_commit(
    ///     "pinned-store".to_string(),
    ///     "https://github.com/user/repo.git".to_string(),
    ///     temp_dir.path().to_path_buf(),
    ///     "abc123def456".to_string(),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_commit(
        name: String,
        url: String,
        cache_dir: PathBuf,
        commit: String,
    ) -> Result<Self> {
        let provider = GitProvider::new(
            url,
            cache_dir.clone(),
            GitReference::Commit(commit),
            GitAuth::None,
        );
        LocallyCachedStore::new(provider, cache_dir, name)
    }

    /// Create a fully customized git store
    ///
    /// # Arguments
    /// * `name` - Human-readable name for the store
    /// * `url` - Git repository URL
    /// * `cache_dir` - Local directory where the repository will be cached
    /// * `reference` - Git reference (branch/tag/commit) to checkout
    /// * `auth` - Authentication configuration
    /// * `fetch_interval` - How often to check for updates
    /// * `shallow` - Whether to use shallow clones for faster cloning
    ///
    /// # Examples
    /// ```rust
    /// use quelle_store::{GitStore, GitAuth, GitReference};
    /// use tempfile::TempDir;
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let temp_dir = TempDir::new()?;
    /// let auth = GitAuth::Token { token: "ghp_xxxx".to_string() };
    /// let reference = GitReference::Branch("main".to_string());
    ///
    /// let store = GitStore::with_config(
    ///     "custom-store".to_string(),
    ///     "https://github.com/user/repo.git".to_string(),
    ///     temp_dir.path().to_path_buf(),
    ///     reference,
    ///     auth,
    ///     Duration::from_secs(1800), // Check for updates every 30 minutes
    ///     false, // Don't use shallow clone
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_config(
        name: String,
        url: String,
        cache_dir: PathBuf,
        reference: GitReference,
        auth: GitAuth,
        fetch_interval: Duration,
        shallow: bool,
    ) -> Result<Self> {
        let provider = GitProvider::new(url, cache_dir.clone(), reference, auth)
            .with_fetch_interval(fetch_interval)
            .with_shallow(shallow);
        LocallyCachedStore::new(provider, cache_dir, name)
    }

    /// Get the git repository URL
    pub fn url(&self) -> &str {
        self.provider().url()
    }

    /// Get the cache directory where the repository is stored
    pub fn cache_dir(&self) -> &std::path::Path {
        self.provider().cache_dir()
    }

    /// Check if this git store supports writing operations
    pub fn is_writable(&self) -> bool {
        self.provider().is_writable()
    }

    /// Check the git repository status
    pub async fn check_git_status(&self) -> Result<crate::publish_git::GitStatus> {
        self.provider().check_repository_status().await
    }

    /// Initialize the git repository with a basic store structure
    pub async fn initialize_repository(
        &self,
        config: crate::publish_git::GitInitConfig,
    ) -> Result<crate::publish_git::GitInitResult> {
        self.provider().initialize_repository(config).await
    }

    /// Publish an extension to the git repository
    pub async fn publish_extension(
        &self,
        package: &crate::ExtensionPackage,
    ) -> Result<crate::publish_git::GitPublishResult> {
        use crate::publish_git::GitPublishResult;
        use std::path::PathBuf;

        // Ensure we're synced first
        self.ensure_synced().await?;

        // Check repository status - must be clean before publishing
        let status = self.check_git_status().await?;
        if !status.is_publishable() {
            if let Some(reason) = status.publish_blocking_reason() {
                return Ok(GitPublishResult::failure(
                    package.manifest.id.clone(),
                    package.manifest.version.to_string(),
                    format!("Cannot publish: {}", reason),
                ));
            }
        }

        // Publish to local store first
        let _publish_result = self
            .local_store()
            .publish(package.clone(), crate::publish::PublishOptions::default())
            .await?;

        // Determine which files were affected
        let extension_dir = self.sync_dir().join(&package.manifest.id);
        let affected_files = vec![
            PathBuf::from("store.json"), // Store manifest always updated
            extension_dir
                .strip_prefix(self.sync_dir())
                .unwrap()
                .to_path_buf(),
        ];

        // Note: PublishResult doesn't have modified_files field in current implementation
        // This would need to be added if we want to track specific files

        // Git operations
        match self.git_publish_workflow(package, &affected_files).await {
            Ok((commit_hash, branch)) => Ok(GitPublishResult::success(
                commit_hash,
                branch,
                affected_files,
                package.manifest.id.clone(),
                package.manifest.version.to_string(),
            )),
            Err(e) => Ok(GitPublishResult::failure(
                package.manifest.id.clone(),
                package.manifest.version.to_string(),
                format!("Git operation failed: {}", e),
            )),
        }
    }

    /// Unpublish an extension from the git repository
    pub async fn unpublish_extension(
        &self,
        extension_id: &str,
        version: &str,
    ) -> Result<crate::publish_git::GitPublishResult> {
        use crate::publish_git::GitPublishResult;
        use std::path::PathBuf;

        // Ensure we're synced first
        self.ensure_synced().await?;

        // Check repository status - must be clean before unpublishing
        let status = self.check_git_status().await?;
        if !status.is_publishable() {
            if let Some(reason) = status.publish_blocking_reason() {
                return Ok(GitPublishResult::failure(
                    extension_id.to_string(),
                    version.to_string(),
                    format!("Cannot unpublish: {}", reason),
                ));
            }
        }

        // Unpublish from local store first
        let _unpublish_result = self
            .local_store()
            .unpublish(extension_id, crate::publish::UnpublishOptions::new())
            .await?;

        // Determine affected files
        let affected_files = vec![
            PathBuf::from("store.json"), // Store manifest always updated
        ];

        // Create a dummy manifest for commit message
        let dummy_manifest = crate::manifest::ExtensionManifest {
            id: extension_id.to_string(),
            name: extension_id.to_string(),
            version: version.to_string(),
            author: String::new(),
            langs: Vec::new(),
            base_urls: Vec::new(),
            rds: Vec::new(),
            attrs: Vec::new(),
            checksum: crate::manifest::Checksum {
                algorithm: crate::manifest::ChecksumAlgorithm::Sha256,
                value: String::new(),
            },
            signature: None,
        };

        let dummy_package =
            crate::ExtensionPackage::new(dummy_manifest, Vec::new(), "git".to_string());

        // Git operations
        match self
            .git_unpublish_workflow(&dummy_package, &affected_files)
            .await
        {
            Ok((commit_hash, branch)) => Ok(GitPublishResult::success(
                commit_hash,
                branch,
                affected_files,
                extension_id.to_string(),
                version.to_string(),
            )),
            Err(e) => Ok(GitPublishResult::failure(
                extension_id.to_string(),
                version.to_string(),
                format!("Git operation failed: {}", e),
            )),
        }
    }

    /// Internal workflow for git publishing operations
    async fn git_publish_workflow(
        &self,
        package: &crate::ExtensionPackage,
        affected_files: &[PathBuf],
    ) -> Result<(String, String)> {
        let provider = self.provider();
        let write_config = provider.write_config.as_ref().ok_or_else(|| {
            crate::error::GitStoreError::NoWritePermission {
                url: provider.url().to_string(),
            }
        })?;

        // Convert relative paths to absolute paths for git operations
        let absolute_files: Vec<PathBuf> = affected_files
            .iter()
            .map(|path| provider.cache_dir().join(path))
            .collect();

        // Stage changes
        provider.git_add(&absolute_files).await?;

        // Create commit message
        let commit_message = write_config
            .commit_message_template
            .replace("{action}", "Add")
            .replace("{extension_id}", &package.manifest.id)
            .replace("{version}", &package.manifest.version.to_string());

        // Commit changes
        let commit_hash = provider.git_commit(&commit_message).await?;

        // Get current branch
        let current_branch = self
            .check_git_status()
            .await?
            .current_branch
            .unwrap_or_else(|| "main".to_string());

        // Push if auto-push is enabled
        if write_config.auto_push {
            provider.git_push().await?;
        }

        Ok((commit_hash, current_branch))
    }

    /// Internal workflow for git unpublishing operations
    async fn git_unpublish_workflow(
        &self,
        package: &crate::ExtensionPackage,
        affected_files: &[PathBuf],
    ) -> Result<(String, String)> {
        let provider = self.provider();
        let write_config = provider.write_config.as_ref().ok_or_else(|| {
            crate::error::GitStoreError::NoWritePermission {
                url: provider.url().to_string(),
            }
        })?;

        // Convert relative paths to absolute paths for git operations
        let absolute_files: Vec<PathBuf> = affected_files
            .iter()
            .map(|path| provider.cache_dir().join(path))
            .collect();

        // Stage changes
        provider.git_add(&absolute_files).await?;

        // Create commit message
        let commit_message = write_config
            .commit_message_template
            .replace("{action}", "Remove")
            .replace("{extension_id}", &package.manifest.id)
            .replace("{version}", &package.manifest.version.to_string());

        // Commit changes
        let commit_hash = provider.git_commit(&commit_message).await?;

        // Get current branch
        let current_branch = self
            .check_git_status()
            .await?
            .current_branch
            .unwrap_or_else(|| "main".to_string());

        // Push if auto-push is enabled
        if write_config.auto_push {
            provider.git_push().await?;
        }

        Ok((commit_hash, current_branch))
    }
}

#[cfg(test)]
#[cfg(feature = "git")]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::from_url(
            "test-store".to_string(),
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
        assert_eq!(store.cache_dir(), temp_dir.path());
    }

    #[test]
    fn test_git_store_with_auth() {
        let temp_dir = TempDir::new().unwrap();
        let auth = GitAuth::Token {
            token: "test-token".to_string(),
        };

        let store = GitStore::with_auth(
            "auth-store".to_string(),
            "https://github.com/test/private-repo.git".to_string(),
            temp_dir.path().to_path_buf(),
            auth,
        )
        .unwrap();

        assert_eq!(store.url(), "https://github.com/test/private-repo.git");
    }

    #[test]
    fn test_git_store_with_branch() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::with_branch(
            "branch-store".to_string(),
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path().to_path_buf(),
            "develop".to_string(),
        )
        .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_git_store_with_tag() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::with_tag(
            "tag-store".to_string(),
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path().to_path_buf(),
            "v1.0.0".to_string(),
        )
        .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_git_store_with_commit() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::with_commit(
            "commit-store".to_string(),
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path().to_path_buf(),
            "abc123".to_string(),
        )
        .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_git_store_with_config() {
        let temp_dir = TempDir::new().unwrap();
        let auth = GitAuth::Token {
            token: "test-token".to_string(),
        };
        let reference = GitReference::Branch("main".to_string());

        let store = GitStore::with_config(
            "config-store".to_string(),
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path().to_path_buf(),
            reference,
            auth,
            Duration::from_secs(1800),
            false,
        )
        .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_git_store_writability() {
        let temp_dir = TempDir::new().unwrap();

        // Read-only store
        let readonly_store = GitStore::from_url(
            "readonly-store".to_string(),
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path().to_path_buf(),
        )
        .unwrap();

        assert!(!readonly_store.is_writable());

        // Writable store
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        )
        .enable_writing();

        let writable_store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "writable-store".to_string(),
        )
        .unwrap();

        assert!(writable_store.is_writable());
    }
}
