//! Git publishing types and results

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of a git publishing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPublishResult {
    /// Whether the operation was successful
    pub success: bool,
    /// Git commit hash if successful
    pub commit_hash: Option<String>,
    /// Branch that was committed to
    pub branch: String,
    /// Files that were modified
    pub modified_files: Vec<PathBuf>,
    /// Extension ID that was published
    pub extension_id: String,
    /// Extension version that was published
    pub version: String,
    /// Timestamp of the operation
    pub timestamp: DateTime<Utc>,
    /// Any warnings during the operation
    pub warnings: Vec<String>,
    /// Error message if failed
    pub error: Option<String>,
}

impl GitPublishResult {
    /// Create a successful publish result
    pub fn success(
        commit_hash: String,
        branch: String,
        modified_files: Vec<PathBuf>,
        extension_id: String,
        version: String,
    ) -> Self {
        Self {
            success: true,
            commit_hash: Some(commit_hash),
            branch,
            modified_files,
            extension_id,
            version,
            timestamp: Utc::now(),
            warnings: Vec::new(),
            error: None,
        }
    }

    /// Create a failed publish result
    pub fn failure(extension_id: String, version: String, error: String) -> Self {
        Self {
            success: false,
            commit_hash: None,
            branch: String::new(),
            modified_files: Vec::new(),
            extension_id,
            version,
            timestamp: Utc::now(),
            warnings: Vec::new(),
            error: Some(error),
        }
    }

    /// Add a warning to the result
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Add multiple warnings to the result
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }
}

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

/// Result of initializing a git store
#[derive(Debug, Clone)]
pub struct GitInitResult {
    /// Whether initialization was successful
    pub success: bool,
    /// Path to the initialized repository
    pub repository_path: PathBuf,
    /// Initial commit hash if created
    pub initial_commit: Option<String>,
    /// Whether a new repository was created or existing one was used
    pub created_new: bool,
    /// Any warnings during initialization
    pub warnings: Vec<String>,
    /// Error message if failed
    pub error: Option<String>,
}

impl GitInitResult {
    /// Create a successful init result
    pub fn success(
        repository_path: PathBuf,
        initial_commit: Option<String>,
        created_new: bool,
    ) -> Self {
        Self {
            success: true,
            repository_path,
            initial_commit,
            created_new,
            warnings: Vec::new(),
            error: None,
        }
    }

    /// Create a failed init result
    pub fn failure(repository_path: PathBuf, error: String) -> Self {
        Self {
            success: false,
            repository_path,
            initial_commit: None,
            created_new: false,
            warnings: Vec::new(),
            error: Some(error),
        }
    }

    /// Add a warning to the result
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// Configuration for initializing a git store
#[derive(Debug, Clone)]
pub struct GitInitConfig {
    /// Store name
    pub store_name: String,
    /// Store description
    pub store_description: Option<String>,
    /// Initial store version
    pub store_version: String,
    /// Author for initial commit
    pub author: crate::stores::providers::git::GitAuthor,
    /// Whether to create an initial commit
    pub create_initial_commit: bool,
    /// Initial commit message
    pub initial_commit_message: String,
}

impl GitInitConfig {
    /// Create a new init config with required fields
    pub fn new(store_name: String) -> Self {
        Self {
            store_name: store_name.clone(),
            store_description: None,
            store_version: "1.0.0".to_string(),
            author: crate::stores::providers::git::GitAuthor::default(),
            create_initial_commit: true,
            initial_commit_message: format!("Initialize {} extension store", store_name),
        }
    }

    /// Set store description
    pub fn with_description(mut self, description: String) -> Self {
        self.store_description = Some(description);
        self
    }

    /// Set store version
    pub fn with_version(mut self, version: String) -> Self {
        self.store_version = version;
        self
    }

    /// Set author information
    pub fn with_author(mut self, author: crate::stores::providers::git::GitAuthor) -> Self {
        self.author = author;
        self
    }

    /// Set whether to create initial commit
    pub fn with_initial_commit(mut self, create: bool) -> Self {
        self.create_initial_commit = create;
        self
    }

    /// Set initial commit message
    pub fn with_commit_message(mut self, message: String) -> Self {
        self.initial_commit_message = message;
        self
    }
}
