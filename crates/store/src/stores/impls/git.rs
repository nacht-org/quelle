//! Git store type alias and builder
//!
//! This module provides a type alias and builder pattern for creating
//! git-based stores using LocallyCachedStore with GitProvider.

#[cfg(feature = "git")]
use std::path::PathBuf;
#[cfg(feature = "git")]
use std::time::Duration;

#[cfg(feature = "git")]
use crate::error::Result;
#[cfg(feature = "git")]
use crate::stores::impls::locally_cached::LocallyCachedStore;
#[cfg(feature = "git")]
use crate::stores::providers::git::{
    CommitStyle, GitAuth, GitAuthor, GitProvider, GitReference, GitWriteConfig,
};

/// Type alias for a git-based store
///
/// This is a LocallyCachedStore that uses a GitProvider to sync data
/// from a git repository to local storage.
#[cfg(feature = "git")]
pub type GitStore = LocallyCachedStore<GitProvider>;

/// Builder for creating git stores with a fluent API
#[cfg(feature = "git")]
pub struct GitStoreBuilder {
    url: String,
    cache_dir: Option<PathBuf>,
    name: Option<String>,
    reference: GitReference,
    auth: GitAuth,
    fetch_interval: Duration,
    shallow: bool,
    write_config: Option<GitWriteConfig>,
}

#[cfg(feature = "git")]
impl GitStoreBuilder {
    /// Create a new builder for the given git repository URL
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            cache_dir: None,
            name: None,
            reference: GitReference::Default,
            auth: GitAuth::None,
            fetch_interval: Duration::from_secs(300), // 5 minutes
            shallow: true,
            write_config: None,
        }
    }

    /// Set the cache directory where the git repository will be stored
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(path.into());
        self
    }

    /// Set the name for this store
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set authentication for the git repository
    pub fn auth(mut self, auth: GitAuth) -> Self {
        self.auth = auth;
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

    /// Set the fetch interval for checking updates
    pub fn fetch_interval(mut self, interval: Duration) -> Self {
        self.fetch_interval = interval;
        self
    }

    /// Enable or disable shallow cloning
    pub fn shallow(mut self, shallow: bool) -> Self {
        self.shallow = shallow;
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
        config.author = Some(GitAuthor::new(name, email));
        self.write_config = Some(config);
        self
    }

    /// Set the commit message style
    pub fn commit_style(mut self, style: CommitStyle) -> Self {
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

    /// Build the GitStore
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `cache_dir` was not set via the builder
    /// - `name` was not set via the builder
    /// - The store cannot be initialized
    pub fn build(self) -> Result<GitStore> {
        let cache_dir = self.cache_dir.ok_or_else(|| {
            crate::error::StoreError::InvalidConfiguration(
                "cache_dir must be set before building GitStore".to_string(),
            )
        })?;

        let name = self.name.ok_or_else(|| {
            crate::error::StoreError::InvalidConfiguration(
                "name must be set before building GitStore".to_string(),
            )
        })?;

        let mut provider = GitProvider::new(self.url, cache_dir, self.reference, self.auth)
            .with_fetch_interval(self.fetch_interval)
            .with_shallow(self.shallow);

        if let Some(write_config) = self.write_config {
            provider = provider.with_write_config(write_config);
        }

        LocallyCachedStore::new(provider, name)
    }

    /// Build the GitStore with explicit cache_dir and name parameters
    ///
    /// This is a convenience method that's equivalent to calling
    /// `builder.cache_dir(cache_dir).name(name).build()`
    ///
    /// # Deprecated
    ///
    /// Use `.cache_dir().name().build()` instead for a more fluent API
    #[deprecated(since = "0.1.0", note = "Use .cache_dir().name().build() instead")]
    pub fn build_with(self, cache_dir: PathBuf, name: impl Into<String>) -> Result<GitStore> {
        self.cache_dir(cache_dir).name(name).build()
    }
}

/// Convenience methods for creating git stores
#[cfg(feature = "git")]
impl GitStore {
    /// Create a new git store builder
    ///
    /// # Examples
    /// ```rust
    /// use quelle_store::GitStore;
    /// use tempfile::TempDir;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let temp_dir = TempDir::new()?;
    /// let store = GitStore::builder("https://github.com/user/repo.git")
    ///     .cache_dir(temp_dir.path())
    ///     .name("my-store")
    ///     .branch("main")
    ///     .writable()
    ///     .author("Bot", "bot@example.com")
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder(url: impl Into<String>) -> GitStoreBuilder {
        GitStoreBuilder::new(url)
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
    pub async fn check_git_status(&self) -> Result<crate::stores::providers::git::GitStatus> {
        self.provider().check_repository_status().await
    }
}

#[cfg(test)]
#[cfg(feature = "git")]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_store_builder_basic() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .name("test-store")
            .build()
            .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
        assert_eq!(store.cache_dir(), temp_dir.path());
        assert!(!store.is_writable());
    }

    #[test]
    fn test_git_store_builder_with_auth() {
        let temp_dir = TempDir::new().unwrap();
        let auth = GitAuth::Token {
            token: "test-token".to_string(),
        };

        let store = GitStore::builder("https://github.com/test/private-repo.git")
            .cache_dir(temp_dir.path())
            .name("auth-store")
            .auth(auth)
            .build()
            .unwrap();

        assert_eq!(store.url(), "https://github.com/test/private-repo.git");
    }

    #[test]
    fn test_git_store_builder_with_branch() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .name("branch-store")
            .branch("develop")
            .build()
            .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_git_store_builder_with_tag() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .name("tag-store")
            .tag("v1.0.0")
            .build()
            .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_git_store_builder_with_commit() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .name("commit-store")
            .commit("abc123")
            .build()
            .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
    }

    #[test]
    fn test_git_store_builder_writable() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .name("writable-store")
            .writable()
            .author("Test Author", "test@example.com")
            .build()
            .unwrap();

        assert!(store.is_writable());
    }

    #[test]
    fn test_git_store_builder_custom_config() {
        let temp_dir = TempDir::new().unwrap();
        let auth = GitAuth::Token {
            token: "test-token".to_string(),
        };

        let store = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .name("custom-store")
            .auth(auth)
            .branch("main")
            .fetch_interval(Duration::from_secs(1800))
            .shallow(false)
            .writable()
            .author("Bot", "bot@example.com")
            .commit_style(CommitStyle::Detailed)
            .build()
            .unwrap();

        assert_eq!(store.url(), "https://github.com/test/repo.git");
        assert!(store.is_writable());
    }

    #[test]
    fn test_git_store_builder_no_auto_push() {
        let temp_dir = TempDir::new().unwrap();
        let store = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .name("no-push-store")
            .writable()
            .no_auto_push()
            .build()
            .unwrap();

        assert!(store.is_writable());
        // Check that auto_push is false (we'd need to access the provider for this)
    }

    #[test]
    fn test_git_store_builder_missing_cache_dir() {
        let result = GitStore::builder("https://github.com/test/repo.git")
            .name("test-store")
            .build();

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("cache_dir must be set"));
        }
    }

    #[test]
    fn test_git_store_builder_missing_name() {
        let temp_dir = TempDir::new().unwrap();
        let result = GitStore::builder("https://github.com/test/repo.git")
            .cache_dir(temp_dir.path())
            .build();

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("name must be set"));
        }
    }
}
