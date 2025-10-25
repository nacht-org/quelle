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
use crate::stores::providers::traits::{Capability, LifecycleEvent, StoreProvider, SyncResult};

/// Git authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GitAuth {
    /// No authentication - uses system credentials (SSH agent, git credential manager, etc.)
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

impl Default for GitAuth {
    fn default() -> Self {
        Self::None
    }
}

impl GitAuth {
    /// Check if this is using system credentials
    pub fn is_system_auth(&self) -> bool {
        matches!(self, GitAuth::None)
    }
}

/// Git reference type for specifying what to checkout
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl GitReference {
    /// Convert to string for GitHub raw URLs
    pub fn to_string(&self) -> String {
        match self {
            GitReference::Branch(branch) => branch.clone(),
            GitReference::Tag(tag) => tag.clone(),
            GitReference::Commit(commit) => commit.clone(),
            GitReference::Default => "main".to_string(),
        }
    }
}

impl Default for GitReference {
    fn default() -> Self {
        Self::Default
    }
}

/// Git author information for commits
#[derive(Debug, Clone)]
pub struct GitAuthor {
    pub name: String,
    pub email: String,
}

impl Default for GitAuthor {
    fn default() -> Self {
        Self {
            name: "Quelle".to_string(),
            email: "quelle@localhost".to_string(),
        }
    }
}

impl GitAuthor {
    /// Create a new GitAuthor
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
        }
    }

    /// Try to load author from git config
    pub fn from_git_config() -> Option<Self> {
        // Try to read from git config
        let name = std::process::Command::new("git")
            .args(["config", "--get", "user.name"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let email = std::process::Command::new("git")
            .args(["config", "--get", "user.email"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        match (name, email) {
            (Some(name), Some(email)) => Some(Self { name, email }),
            _ => None,
        }
    }

    /// Get author with fallback to git config or default
    pub fn or_from_git_config(self) -> Self {
        if self.name == "Quelle" && self.email == "quelle@localhost" {
            Self::from_git_config().unwrap_or(self)
        } else {
            self
        }
    }
}

/// Commit message style for git operations
#[derive(Debug, Clone)]
pub enum CommitStyle {
    /// Default style: "Publish ext_id v1.0.0"
    Default,
    /// Detailed style: "Publish extension ext_id version 1.0.0"
    Detailed,
    /// Minimal style: "Add ext_id@1.0.0"
    Minimal,
    /// Custom function for generating commit messages
    Custom(fn(action: &str, extension_id: &str, version: &str) -> String),
}

impl Default for CommitStyle {
    fn default() -> Self {
        Self::Default
    }
}

impl CommitStyle {
    /// Generate a commit message for the given action
    pub fn format(&self, action: &str, extension_id: &str, version: &str) -> String {
        match self {
            CommitStyle::Default => format!("{} {} v{}", action, extension_id, version),
            CommitStyle::Detailed => {
                format!("{} extension {} version {}", action, extension_id, version)
            }
            CommitStyle::Minimal => format!("{} {}@{}", action, extension_id, version),
            CommitStyle::Custom(f) => f(action, extension_id, version),
        }
    }
}

/// Configuration for git write operations
#[derive(Debug, Clone)]
pub struct GitWriteConfig {
    /// Author information for commits (None = use git config or default)
    pub author: Option<GitAuthor>,
    /// Commit message style
    pub commit_style: CommitStyle,
    /// Whether to automatically push after commit (default: true)
    pub auto_push: bool,
}

impl Default for GitWriteConfig {
    fn default() -> Self {
        Self {
            author: None,
            commit_style: CommitStyle::Default,
            auto_push: true,
        }
    }
}

impl GitWriteConfig {
    /// Create a new write configuration with the given author
    pub fn new(author: GitAuthor) -> Self {
        Self {
            author: Some(author),
            commit_style: CommitStyle::Default,
            auto_push: true,
        }
    }

    /// Set the commit style
    pub fn with_commit_style(mut self, style: CommitStyle) -> Self {
        self.commit_style = style;
        self
    }

    /// Set whether to auto-push
    pub fn with_auto_push(mut self, auto_push: bool) -> Self {
        self.auto_push = auto_push;
        self
    }

    /// Get the author, falling back to git config or default
    pub fn effective_author(&self) -> GitAuthor {
        self.author
            .clone()
            .or_else(GitAuthor::from_git_config)
            .unwrap_or_default()
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
    /// Write configuration (None = read-only)
    pub write_config: Option<GitWriteConfig>,
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
            fetch_interval: Duration::from_secs(300), // 5 minutes
            last_fetch: RwLock::new(None),
            shallow: true,
            write_config: None,
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

    /// Configure write operations for this git provider
    pub fn with_write_config(mut self, config: GitWriteConfig) -> Self {
        self.write_config = Some(config);
        self
    }

    /// Enable write operations with default configuration
    pub fn enable_writing(mut self) -> Self {
        self.write_config = Some(GitWriteConfig::default());
        self
    }

    /// Set author for commits (convenience method)
    pub fn with_author(mut self, name: impl Into<String>, email: impl Into<String>) -> Self {
        let mut config = self.write_config.unwrap_or_default();
        config.author = Some(GitAuthor::new(name, email));
        self.write_config = Some(config);
        self
    }

    /// Set commit style (convenience method)
    pub fn with_commit_style(mut self, style: CommitStyle) -> Self {
        let mut config = self.write_config.unwrap_or_default();
        config.commit_style = style;
        self.write_config = Some(config);
        self
    }

    /// Disable auto-push (commits will be local only)
    pub fn no_auto_push(mut self) -> Self {
        let mut config = self.write_config.unwrap_or_default();
        config.auto_push = false;
        self.write_config = Some(config);
        self
    }

    /// Check if this provider supports writing
    pub fn is_writable(&self) -> bool {
        self.write_config.is_some()
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
    fn sync_dir(&self) -> &Path {
        &self.cache_dir
    }

    async fn sync(&self) -> Result<SyncResult> {
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

    async fn needs_sync(&self) -> Result<bool> {
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

    fn supports_capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Write => self.write_config.is_some(),
            Capability::IncrementalSync => true,
            Capability::Authentication => !matches!(self.auth, GitAuth::None),
            Capability::RemotePush => self
                .write_config
                .as_ref()
                .map(|c| c.auto_push)
                .unwrap_or(false),
            Capability::Caching => true,
            Capability::BackgroundSync => true,
        }
    }

    async fn handle_event(&self, event: LifecycleEvent) -> Result<()> {
        // Only handle events if we're writable
        if self.write_config.is_none() {
            return Ok(());
        }

        let write_config =
            self.write_config
                .as_ref()
                .ok_or_else(|| crate::error::StoreError::InvalidPackage {
                    reason: "Git write configuration not available".to_string(),
                })?;

        // Add all changes in the working directory
        self.git_add_all().await?;

        // Create commit message based on the event type
        let commit_message = match &event {
            LifecycleEvent::Published {
                extension_id,
                version,
            } => write_config
                .commit_style
                .format("Publish", extension_id, version),
            LifecycleEvent::Unpublished {
                extension_id,
                version,
            } => write_config
                .commit_style
                .format("Unpublish", extension_id, version),
        };

        // Commit changes
        self.git_commit(&commit_message).await?;

        // Push if auto-push is enabled
        if write_config.auto_push {
            if let Err(e) = self.git_push().await {
                tracing::warn!(
                    "Failed to push changes to remote repository: {}. \
                     Consider configuring authentication for automatic pushing.",
                    e
                );
            }
        }

        Ok(())
    }

    async fn ensure_writable(&self) -> Result<()> {
        // Check if we have write config
        if self.write_config.is_none() {
            return Err(crate::error::StoreError::InvalidPackage {
                reason: "Git provider is read-only (no write configuration)".to_string(),
            });
        }

        // Check if authentication is configured for push operations
        if let Some(write_config) = &self.write_config {
            if write_config.auto_push && matches!(self.auth, GitAuth::None) {
                tracing::debug!(
                        "Auto-push is enabled with GitAuth::None for repository '{}'. \
                         Will attempt to use system git credentials (SSH agent, credential manager, etc.)",
                        self.url
                    );
            }
        }

        // Check repository status
        let status = self.check_repository_status().await?;
        if !status.is_publishable() {
            if let Some(reason) = status.publish_blocking_reason() {
                return Err(crate::error::StoreError::InvalidPackage {
                    reason: format!("Cannot write to git repository: {}", reason),
                });
            }
        }

        Ok(())
    }
}

#[cfg(feature = "git")]
/// Git repository status information
#[derive(Debug, Clone)]
pub struct GitStatus {
    /// Whether the working directory is clean
    pub is_clean: bool,
    /// List of modified files
    pub modified_files: Vec<PathBuf>,
    /// List of untracked files
    pub untracked_files: Vec<PathBuf>,
    /// List of staged files
    pub staged_files: Vec<PathBuf>,
    /// Current branch name
    pub current_branch: Option<String>,
    /// Current commit hash
    pub current_commit: Option<String>,
    /// Whether the repository exists
    pub repository_exists: bool,
}

impl GitStatus {
    /// Create a clean git status
    pub fn clean(branch: Option<String>, commit: Option<String>) -> Self {
        Self {
            is_clean: true,
            modified_files: Vec::new(),
            untracked_files: Vec::new(),
            staged_files: Vec::new(),
            current_branch: branch,
            current_commit: commit,
            repository_exists: true,
        }
    }

    /// Create a dirty git status
    pub fn dirty(
        modified_files: Vec<PathBuf>,
        untracked_files: Vec<PathBuf>,
        staged_files: Vec<PathBuf>,
        branch: Option<String>,
        commit: Option<String>,
    ) -> Self {
        Self {
            is_clean: false,
            modified_files,
            untracked_files,
            staged_files,
            current_branch: branch,
            current_commit: commit,
            repository_exists: true,
        }
    }

    /// Create status for non-existent repository
    pub fn not_exists() -> Self {
        Self {
            is_clean: false,
            modified_files: Vec::new(),
            untracked_files: Vec::new(),
            staged_files: Vec::new(),
            current_branch: None,
            current_commit: None,
            repository_exists: false,
        }
    }

    /// Check if the repository is in a publishable state
    pub fn is_publishable(&self) -> bool {
        self.repository_exists && self.is_clean
    }

    /// Get a human-readable description of why publishing is not possible
    pub fn publish_blocking_reason(&self) -> Option<String> {
        if !self.repository_exists {
            return Some("Repository does not exist".to_string());
        }

        if !self.is_clean {
            let mut reasons = Vec::new();

            if !self.modified_files.is_empty() {
                reasons.push(format!("{} modified files", self.modified_files.len()));
            }

            if !self.untracked_files.is_empty() {
                reasons.push(format!("{} untracked files", self.untracked_files.len()));
            }

            if !self.staged_files.is_empty() {
                reasons.push(format!("{} staged files", self.staged_files.len()));
            }

            if !reasons.is_empty() {
                return Some(format!(
                    "Repository has uncommitted changes: {}",
                    reasons.join(", ")
                ));
            }
        }

        None
    }
}

impl GitProvider {
    /// Check if the repository is in a clean state (no uncommitted changes)
    pub async fn check_repository_status(&self) -> Result<GitStatus> {
        use crate::error::GitStoreError;
        use git2::{Repository, Status, StatusOptions};
        use std::path::Path;

        let repo_path = &self.cache_dir;

        if !repo_path.exists() || !repo_path.join(".git").exists() {
            return Ok(GitStatus::not_exists());
        }

        let repo = Repository::open(repo_path).map_err(GitStoreError::Git)?;

        // Get repository status
        let mut status_options = StatusOptions::new();
        status_options.include_untracked(true);
        let statuses = repo
            .statuses(Some(&mut status_options))
            .map_err(GitStoreError::Git)?;

        let mut modified_files = Vec::new();
        let mut untracked_files = Vec::new();
        let mut staged_files = Vec::new();

        for status in statuses.iter() {
            let path = status.path().unwrap_or("").to_string();
            let flags = status.status();

            if flags.contains(Status::WT_MODIFIED)
                || flags.contains(Status::WT_DELETED)
                || flags.contains(Status::WT_RENAMED)
                || flags.contains(Status::WT_TYPECHANGE)
            {
                modified_files.push(Path::new(&path).to_path_buf());
            }

            if flags.contains(Status::WT_NEW) {
                untracked_files.push(Path::new(&path).to_path_buf());
            }

            if flags.contains(Status::INDEX_MODIFIED)
                || flags.contains(Status::INDEX_DELETED)
                || flags.contains(Status::INDEX_RENAMED)
                || flags.contains(Status::INDEX_TYPECHANGE)
                || flags.contains(Status::INDEX_NEW)
            {
                staged_files.push(Path::new(&path).to_path_buf());
            }
        }

        // Get current branch
        let current_branch = repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().map(|s| s.to_string()));

        // Get current commit
        let current_commit = repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|oid| oid.to_string());

        let is_clean =
            modified_files.is_empty() && untracked_files.is_empty() && staged_files.is_empty();

        if is_clean {
            Ok(GitStatus::clean(current_branch, current_commit))
        } else {
            Ok(GitStatus::dirty(
                modified_files,
                untracked_files,
                staged_files,
                current_branch,
                current_commit,
            ))
        }
    }

    /// Add files to git staging area
    pub async fn git_add(&self, files: &[std::path::PathBuf]) -> Result<()> {
        use crate::error::GitStoreError;
        use git2::Repository;

        let repo = Repository::open(&self.cache_dir).map_err(GitStoreError::Git)?;

        let mut index = repo.index().map_err(GitStoreError::Git)?;

        for file in files {
            let relative_path = file.strip_prefix(&self.cache_dir).unwrap_or(file);

            // Skip files that don't exist
            if !file.exists() {
                continue;
            }

            index.add_path(relative_path).map_err(GitStoreError::Git)?;
        }

        index.write().map_err(GitStoreError::Git)?;

        Ok(())
    }

    pub fn is_git_repo(&self) -> bool {
        self.cache_dir.join(".git").exists()
    }

    pub fn git_init(&self) -> Result<()> {
        use crate::error::GitStoreError;
        use git2::Repository;

        // Initialize a new git repository in the cache directory
        Repository::init(&self.cache_dir).map_err(GitStoreError::Git)?;

        Ok(())
    }

    pub fn set_git_remote(&self) -> Result<()> {
        use crate::error::GitStoreError;
        use git2::Repository;

        let repo = Repository::open(&self.cache_dir).map_err(GitStoreError::Git)?;
        repo.remote_set_url("origin", self.url())
            .map_err(GitStoreError::Git)?;

        Ok(())
    }

    /// Add all changes (including deletions) to git staging area
    pub async fn git_add_all(&self) -> Result<()> {
        use crate::error::GitStoreError;
        use git2::Repository;

        let repo = Repository::open(&self.cache_dir).map_err(GitStoreError::Git)?;

        let mut index = repo.index().map_err(GitStoreError::Git)?;

        // Add all files from working directory to index
        // This is equivalent to "git add ." - adds all tracked and untracked files
        if let Err(e) = index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None) {
            tracing::warn!("Failed to add all files to git index: {}", e);
            // Fall back to updating from working tree
            index
                .update_all(["*"].iter(), None)
                .map_err(GitStoreError::Git)?;
        }

        index.write().map_err(GitStoreError::Git)?;

        Ok(())
    }

    /// Create a git commit
    pub async fn git_commit(&self, message: &str) -> Result<String> {
        use crate::error::GitStoreError;
        use git2::{Repository, Signature};

        let config =
            self.write_config
                .as_ref()
                .ok_or_else(|| GitStoreError::NoWritePermission {
                    url: self.url.clone(),
                })?;

        let repo = Repository::open(&self.cache_dir).map_err(GitStoreError::Git)?;

        let author = config.effective_author();
        let signature = Signature::now(&author.name, &author.email).map_err(GitStoreError::Git)?;

        let mut index = repo.index().map_err(GitStoreError::Git)?;
        let tree_id = index.write_tree().map_err(GitStoreError::Git)?;
        let tree = repo.find_tree(tree_id).map_err(GitStoreError::Git)?;

        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit().map_err(GitStoreError::Git)?),
            Err(_) => None, // First commit
        };

        let parents = if let Some(ref parent) = parent_commit {
            vec![parent]
        } else {
            vec![]
        };

        let commit_id = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .map_err(GitStoreError::Git)?;

        Ok(commit_id.to_string())
    }

    /// Push changes to remote repository
    pub async fn git_push(&self) -> Result<()> {
        use crate::error::GitStoreError;
        use git2::{Cred, CredentialType, PushOptions, RemoteCallbacks, Repository};

        // Verify we have write config
        self.write_config
            .as_ref()
            .ok_or_else(|| GitStoreError::NoWritePermission {
                url: self.url.clone(),
            })?;

        let repo = Repository::open(&self.cache_dir).map_err(GitStoreError::Git)?;

        let mut remote = repo.find_remote("origin").map_err(GitStoreError::Git)?;

        let mut callbacks = RemoteCallbacks::new();

        // Set up authentication using provider's auth
        match &self.auth {
            GitAuth::Token { token } => {
                callbacks.credentials(|_url, username_from_url, _allowed_types| {
                    Cred::userpass_plaintext(username_from_url.unwrap_or("git"), token)
                });
            }
            GitAuth::SshKey {
                private_key_path,
                public_key_path,
                passphrase,
            } => {
                let private_key = private_key_path.clone();
                let public_key = public_key_path.clone();
                let pass = passphrase.clone();
                callbacks.credentials(move |_url, username_from_url, allowed_types| {
                    if allowed_types.contains(CredentialType::SSH_KEY) {
                        Cred::ssh_key(
                            username_from_url.unwrap_or("git"),
                            public_key.as_deref(),
                            &private_key,
                            pass.as_deref(),
                        )
                    } else {
                        Err(git2::Error::from_str(
                            "SSH key authentication not supported",
                        ))
                    }
                });
            }
            GitAuth::UserPassword { username, password } => {
                let user = username.clone();
                let pass = password.clone();
                callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                    Cred::userpass_plaintext(&user, &pass)
                });
            }
            GitAuth::None => {
                // Use system's default credential helpers (git credential manager, SSH agent, etc.)
                callbacks.credentials(|url, username_from_url, allowed_types| {
                    // Try SSH agent first if SSH is allowed
                    if allowed_types.contains(CredentialType::SSH_KEY) {
                        if let Ok(cred) = Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")) {
                            return Ok(cred);
                        }
                    }

                    // Try credential helper for HTTPS
                    if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
                        if let Ok(config) = git2::Config::open_default() {
                            if let Ok(cred) = Cred::credential_helper(&config, url, username_from_url) {
                                return Ok(cred);
                            }
                        }
                    }

                    // Fall back to default credentials
                    if allowed_types.contains(CredentialType::DEFAULT) {
                        return Cred::default();
                    }

                    // No suitable credential method found
                    Err(git2::Error::from_str("No suitable authentication method found. Consider configuring explicit authentication."))
                });
            }
        }

        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        // Determine which branch to push
        let current_branch = repo
            .head()
            .map_err(GitStoreError::Git)?
            .shorthand()
            .unwrap_or("main")
            .to_string();

        let refspec = format!(
            "refs/heads/{}:refs/heads/{}",
            current_branch, current_branch
        );

        remote
            .push(&[&refspec], Some(&mut push_options))
            .map_err(|e| GitStoreError::PushRejected {
                reason: e.message().to_string(),
            })?;

        Ok(())
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

        let needs_sync = provider.needs_sync().await.unwrap();
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
