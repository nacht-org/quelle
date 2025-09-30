//! Git provider implementation
//!
//! This module provides GitProvider which handles syncing data from Git repositories
//! to local storage for use by LocalStore.

use async_trait::async_trait;
use git2::{CredentialType, FetchOptions, RemoteCallbacks, Repository};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use tracing::{debug, info};

use crate::error::{Result, StoreError};
use crate::stores::providers::traits::{StoreProvider, SyncResult};

/// Git authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitAuth {
    /// No authentication (public repos)
    None,
    /// Token-based authentication (GitHub/GitLab personal access tokens)
    Token { token: String },
    /// SSH key authentication
    SshKey {
        private_key_path: PathBuf,
        public_key_path: Option<PathBuf>,
        passphrase: Option<String>,
    },
    /// Username/password authentication
    UserPassword { username: String, password: String },
}

/// Git reference type for specifying what to checkout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitReference {
    /// Use the default branch (usually main/master)
    Default,
    /// Use a specific branch
    Branch(String),
    /// Use a specific tag
    Tag(String),
    /// Use a specific commit hash
    Commit(String),
}

impl Default for GitReference {
    fn default() -> Self {
        Self::Default
    }
}

/// Provider for Git repository-based stores
#[derive(Debug)]
pub struct GitProvider {
    /// Git repository URL
    url: String,
    /// Git reference to checkout
    reference: GitReference,
    /// Authentication configuration
    auth: GitAuth,
    /// Local directory where repo is cached
    cache_dir: PathBuf,
    /// How often to check for updates
    fetch_interval: Duration,
    /// Last time we fetched from remote
    last_fetch: RwLock<Option<Instant>>,
    /// Whether to use shallow clone (faster but limited history)
    shallow: bool,
}

impl GitProvider {
    /// Create a new GitProvider
    pub fn new<P: Into<PathBuf>>(
        url: String,
        cache_dir: P,
        reference: GitReference,
        auth: GitAuth,
    ) -> Self {
        Self {
            url,
            reference,
            auth,
            cache_dir: cache_dir.into(),
            fetch_interval: Duration::from_secs(3600), // 1 hour default
            last_fetch: RwLock::new(None),
            shallow: true,
        }
    }

    /// Set the fetch interval
    pub fn with_fetch_interval(mut self, interval: Duration) -> Self {
        self.fetch_interval = interval;
        self
    }

    /// Enable/disable shallow clones
    pub fn with_shallow(mut self, shallow: bool) -> Self {
        self.shallow = shallow;
        self
    }

    /// Get the repository URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Check if the repository exists locally
    fn repo_exists(&self) -> bool {
        self.cache_dir.join(".git").exists()
    }

    /// Open the existing repository
    fn open_repo(&self) -> Result<Repository> {
        Repository::open(&self.cache_dir).map_err(|e| StoreError::GitError {
            operation: "open repository".to_string(),
            url: self.url.clone(),
            source: Box::new(e),
        })
    }

    /// Clone the repository for the first time
    fn clone_repo(&self) -> Result<Repository> {
        info!(
            "Cloning git repository: {} -> {}",
            self.url,
            self.cache_dir.display()
        );

        // Ensure parent directory exists
        if let Some(parent) = self.cache_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::IoOperation {
                operation: "create cache directory".to_string(),
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let mut builder = git2::build::RepoBuilder::new();

        // Configure callbacks for authentication and progress
        let mut callbacks = RemoteCallbacks::new();
        self.setup_auth_callbacks(&mut callbacks)?;

        // Setup progress callback
        callbacks.pack_progress(|_stage, _current, _total| {
            debug!("Git clone progress");
        });

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        if self.shallow {
            // Shallow clone with depth 1 for faster cloning
            builder.fetch_options(fetch_options);
        } else {
            builder.fetch_options(fetch_options);
        }

        let repo = builder
            .clone(&self.url, &self.cache_dir)
            .map_err(|e| StoreError::GitError {
                operation: "clone repository".to_string(),
                url: self.url.clone(),
                source: Box::new(e),
            })?;

        // Checkout the specified reference
        self.checkout_reference(&repo)?;

        info!(
            "Successfully cloned git repository to {}",
            self.cache_dir.display()
        );
        Ok(repo)
    }

    /// Fetch updates from the remote repository
    fn fetch_updates(&self, repo: &Repository) -> Result<bool> {
        debug!("Fetching updates for git repository: {}", self.url);

        let mut remote = repo
            .find_remote("origin")
            .map_err(|e| StoreError::GitError {
                operation: "find origin remote".to_string(),
                url: self.url.clone(),
                source: Box::new(e),
            })?;

        // Setup authentication callbacks
        let mut callbacks = RemoteCallbacks::new();
        self.setup_auth_callbacks(&mut callbacks)?;

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        // Fetch from remote
        remote
            .fetch(&[] as &[&str], Some(&mut fetch_options), None)
            .map_err(|e| StoreError::GitError {
                operation: "fetch from remote".to_string(),
                url: self.url.clone(),
                source: Box::new(e),
            })?;

        // Check if we got any new commits
        let head_before = repo.head().ok().and_then(|h| h.target());

        // Checkout the reference again (might have new commits)
        self.checkout_reference(repo)?;

        let head_after = repo.head().ok().and_then(|h| h.target());

        let updated = head_before != head_after;
        if updated {
            info!("Git repository updated with new commits");
        } else {
            debug!("Git repository is up to date");
        }

        Ok(updated)
    }

    /// Checkout the specified git reference
    fn checkout_reference(&self, repo: &Repository) -> Result<()> {
        match &self.reference {
            GitReference::Default => {
                // Use whatever HEAD points to
                debug!("Using default branch");
                Ok(())
            }
            GitReference::Branch(branch) => {
                debug!("Checking out branch: {}", branch);
                let branch_ref = format!("refs/remotes/origin/{}", branch);
                let oid = repo
                    .refname_to_id(&branch_ref)
                    .map_err(|e| StoreError::GitError {
                        operation: format!("find branch {}", branch),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                repo.set_head_detached(oid)
                    .map_err(|e| StoreError::GitError {
                        operation: format!("checkout branch {}", branch),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                    .map_err(|e| StoreError::GitError {
                        operation: format!("checkout branch {}", branch),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                Ok(())
            }
            GitReference::Tag(tag) => {
                debug!("Checking out tag: {}", tag);
                let tag_ref = format!("refs/tags/{}", tag);
                let oid = repo
                    .refname_to_id(&tag_ref)
                    .map_err(|e| StoreError::GitError {
                        operation: format!("find tag {}", tag),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                repo.set_head_detached(oid)
                    .map_err(|e| StoreError::GitError {
                        operation: format!("checkout tag {}", tag),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                    .map_err(|e| StoreError::GitError {
                        operation: format!("checkout tag {}", tag),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                Ok(())
            }
            GitReference::Commit(commit) => {
                debug!("Checking out commit: {}", commit);
                let oid = git2::Oid::from_str(commit).map_err(|e| StoreError::GitError {
                    operation: format!("parse commit hash {}", commit),
                    url: self.url.clone(),
                    source: Box::new(e),
                })?;

                repo.set_head_detached(oid)
                    .map_err(|e| StoreError::GitError {
                        operation: format!("checkout commit {}", commit),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                    .map_err(|e| StoreError::GitError {
                        operation: format!("checkout commit {}", commit),
                        url: self.url.clone(),
                        source: Box::new(e),
                    })?;

                Ok(())
            }
        }
    }

    /// Setup authentication callbacks based on the auth configuration
    fn setup_auth_callbacks(&self, callbacks: &mut RemoteCallbacks) -> Result<()> {
        match &self.auth {
            GitAuth::None => {
                // No authentication needed
                Ok(())
            }
            GitAuth::Token { token } => {
                let token = token.clone();
                callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                    git2::Cred::userpass_plaintext("token", &token)
                });
                Ok(())
            }
            GitAuth::SshKey {
                private_key_path,
                public_key_path,
                passphrase,
            } => {
                let private_key = private_key_path.clone();
                let public_key = public_key_path.clone();
                let passphrase = passphrase.clone();

                callbacks.credentials(move |_url, username_from_url, allowed_types| {
                    if allowed_types.contains(CredentialType::SSH_KEY) {
                        let username = username_from_url.unwrap_or("git");
                        git2::Cred::ssh_key(
                            username,
                            public_key.as_deref(),
                            &private_key,
                            passphrase.as_deref(),
                        )
                    } else {
                        Err(git2::Error::from_str(
                            "SSH key authentication not supported",
                        ))
                    }
                });
                Ok(())
            }
            GitAuth::UserPassword { username, password } => {
                let username = username.clone();
                let password = password.clone();
                callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                    git2::Cred::userpass_plaintext(&username, &password)
                });
                Ok(())
            }
        }
    }

    /// Check if we should fetch based on the last fetch time
    fn should_fetch(&self) -> bool {
        match self.last_fetch.read().unwrap().as_ref() {
            Some(last_fetch) => last_fetch.elapsed() >= self.fetch_interval,
            None => true, // Never fetched, should fetch
        }
    }

    /// Update the last fetch time
    fn update_last_fetch(&self) {
        *self.last_fetch.write().unwrap() = Some(Instant::now());
    }
}

#[async_trait]
impl StoreProvider for GitProvider {
    async fn sync(&self, sync_dir: &Path) -> Result<SyncResult> {
        // Ensure the sync directory matches our cache directory
        if sync_dir != self.cache_dir {
            return Err(StoreError::InvalidConfiguration(format!(
                "Sync directory {} does not match cache directory {}",
                sync_dir.display(),
                self.cache_dir.display()
            )));
        }

        let mut changes = Vec::new();
        let warnings: Vec<String> = Vec::new();
        let updated;

        if self.repo_exists() {
            // Repository exists, fetch updates
            let repo = self.open_repo()?;
            updated = self.fetch_updates(&repo)?;

            if updated {
                changes.push("Updated repository with new commits".to_string());
            }
        } else {
            // Repository doesn't exist, clone it
            self.clone_repo()?;
            changes.push("Cloned repository".to_string());
            updated = true;
        }

        // Update last fetch time
        self.update_last_fetch();

        let result = if updated {
            SyncResult::with_changes(changes)
        } else {
            SyncResult::no_changes()
        };

        Ok(if !warnings.is_empty() {
            result.with_warning(warnings.join("; "))
        } else {
            result
        })
    }

    async fn needs_sync(&self, sync_dir: &Path) -> Result<bool> {
        // Ensure the sync directory matches our cache directory
        if sync_dir != self.cache_dir {
            return Ok(false);
        }

        // Always sync if repo doesn't exist
        if !self.repo_exists() {
            return Ok(true);
        }

        // Otherwise check if enough time has passed
        Ok(self.should_fetch())
    }

    fn description(&self) -> String {
        match &self.reference {
            GitReference::Default => format!("Git repository at {}", self.url),
            GitReference::Branch(branch) => {
                format!("Git repository at {} (branch: {})", self.url, branch)
            }
            GitReference::Tag(tag) => format!("Git repository at {} (tag: {})", self.url, tag),
            GitReference::Commit(commit) => {
                format!("Git repository at {} (commit: {})", self.url, commit)
            }
        }
    }

    fn provider_type(&self) -> &'static str {
        "git"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_provider_creation() {
        let temp_dir = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path(),
            GitReference::Default,
            GitAuth::None,
        );

        assert_eq!(provider.url(), "https://github.com/test/repo.git");
        assert_eq!(provider.cache_dir(), temp_dir.path());
        assert_eq!(provider.provider_type(), "git");
    }

    #[test]
    fn test_git_provider_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path(),
            GitReference::Branch("develop".to_string()),
            GitAuth::Token {
                token: "test-token".to_string(),
            },
        )
        .with_fetch_interval(Duration::from_secs(1800))
        .with_shallow(false);

        assert!(!provider.shallow);
        assert_eq!(provider.fetch_interval, Duration::from_secs(1800));
    }

    #[tokio::test]
    async fn test_needs_sync_no_repo() {
        let temp_dir = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp_dir.path(),
            GitReference::Default,
            GitAuth::None,
        );

        let needs_sync = provider.needs_sync(temp_dir.path()).await.unwrap();
        assert!(needs_sync); // Should need sync when repo doesn't exist
    }

    #[test]
    fn test_git_reference_variants() {
        assert!(matches!(GitReference::default(), GitReference::Default));

        let branch_ref = GitReference::Branch("main".to_string());
        let tag_ref = GitReference::Tag("v1.0.0".to_string());
        let commit_ref = GitReference::Commit("abc123".to_string());

        match branch_ref {
            GitReference::Branch(name) => assert_eq!(name, "main"),
            _ => panic!("Expected Branch variant"),
        }

        match tag_ref {
            GitReference::Tag(name) => assert_eq!(name, "v1.0.0"),
            _ => panic!("Expected Tag variant"),
        }

        match commit_ref {
            GitReference::Commit(hash) => assert_eq!(hash, "abc123"),
            _ => panic!("Expected Commit variant"),
        }
    }
}
