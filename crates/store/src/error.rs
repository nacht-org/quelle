use thiserror::Error;

use crate::manager::publish::PublishError;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Extension '{0}' not found")]
    ExtensionNotFound(String),

    #[error("Version '{version}' not found for extension '{extension}'")]
    VersionNotFound { extension: String, version: String },

    #[error("Checksum verification failed for '{0}'")]
    ChecksumMismatch(String),

    #[error("Invalid manifest for extension '{0}': {1}")]
    InvalidManifest(String, String),

    #[error("Invalid manifest at '{path}': {source}")]
    InvalidManifestFile {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },

    #[error("Dependency resolution failed: {0}")]
    DependencyError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("IO operation '{operation}' failed on path '{path}': {source}")]
    IoOperation {
        operation: String,
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[error("Store '{0}' is unhealthy: {1}")]
    StoreUnhealthy(String, String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid version format: {0}")]
    InvalidVersion(#[from] semver::Error),

    #[error("Concurrent access error: {0}")]
    ConcurrencyError(String),

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Unsupported store type: {0}")]
    UnsupportedStoreType(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Serialization operation '{operation}' failed: {source}")]
    SerializationErrorWithContext {
        operation: String,
        source: serde_json::Error,
    },

    #[error("Failed to create store of type '{store_type}': {source}")]
    StoreCreationError {
        store_type: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Store not available: {0}")]
    StoreUnavailable(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid package: {reason}")]
    InvalidPackage { reason: String },

    #[error("Timeout error: operation timed out")]
    Timeout,

    #[error("Git operation '{operation}' failed for repository '{url}': {source}")]
    GitError {
        operation: String,
        url: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Extension '{0}' is already installed")]
    ExtensionAlreadyInstalled(String),

    #[error("Invalid extension name: {0}")]
    InvalidExtensionName(String),

    #[error("Corrupted registry: {0}")]
    CorruptedRegistry(String),

    #[error("Publishing error: {0}")]
    PublishError(#[from] PublishError),
}

pub type Result<T> = std::result::Result<T, StoreError>;

impl StoreError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            StoreError::NetworkError(_) => true,
            StoreError::StoreUnhealthy(_, _) => true,
            StoreError::StoreUnavailable(_) => true,
            StoreError::Timeout => true,
            StoreError::ConcurrencyError(_) => true,
            _ => false,
        }
    }

    pub fn is_user_error(&self) -> bool {
        match self {
            StoreError::ExtensionNotFound(_) => true,
            StoreError::VersionNotFound { .. } => true,
            StoreError::InvalidVersion(_) => true,
            StoreError::PermissionDenied(_) => true,
            StoreError::ConfigError(_) => true,
            _ => false,
        }
    }
}

#[derive(Error, Debug)]
pub enum LocalStoreError {
    #[error("Extension directory not found: {0}")]
    DirectoryNotFound(String),

    #[error("Invalid directory structure: {0}")]
    InvalidStructure(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Checksum mismatch for file: {0}")]
    ChecksumMismatch(String),
}

impl From<LocalStoreError> for StoreError {
    fn from(err: LocalStoreError) -> Self {
        match err {
            LocalStoreError::DirectoryNotFound(name) => StoreError::ExtensionNotFound(name),
            LocalStoreError::FileNotFound(file) => StoreError::ExtensionNotFound(file),
            LocalStoreError::InvalidStructure(msg) => StoreError::ConfigError(msg),
            LocalStoreError::Io(err) => StoreError::IoError(err),
            LocalStoreError::Json(err) => StoreError::SerializationError(err),
            LocalStoreError::InvalidManifest(msg) => StoreError::ValidationError(msg),
            LocalStoreError::ChecksumMismatch(file) => StoreError::ChecksumMismatch(file),
        }
    }
}

#[cfg(feature = "git")]
#[derive(Error, Debug)]
pub enum GitStoreError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Clone failed: {0}")]
    CloneFailed(String),

    #[error("Fetch failed: {0}")]
    FetchFailed(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Network timeout")]
    NetworkTimeout,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // Publishing-specific errors
    #[error(
        "Repository has uncommitted changes. Please commit or stash changes before publishing."
    )]
    DirtyRepository,

    #[error("Push rejected by remote: {reason}")]
    PushRejected { reason: String },

    #[error("Merge conflict detected in files: {files:?}")]
    MergeConflict { files: Vec<String> },

    #[error("Repository is not initialized as a git repository")]
    NotARepository,

    #[error("No write permissions for repository: {url}")]
    NoWritePermission { url: String },

    #[error("Extension {id} version {version} already exists in repository")]
    ExtensionVersionExists { id: String, version: String },

    #[error("Commit failed: {reason}")]
    CommitFailed { reason: String },

    #[error("Failed to create branch {branch}: {reason}")]
    BranchCreationFailed { branch: String, reason: String },

    #[error("Repository is in an inconsistent state: {details}")]
    InconsistentState { details: String },
}

#[cfg(feature = "git")]
impl From<GitStoreError> for StoreError {
    fn from(err: GitStoreError) -> Self {
        match err {
            GitStoreError::Git(err) => StoreError::NetworkError(err.to_string()),
            GitStoreError::RepositoryNotFound(repo) => StoreError::StoreUnavailable(repo),
            GitStoreError::BranchNotFound(branch) => {
                StoreError::ConfigError(format!("Branch not found: {}", branch))
            }
            GitStoreError::CloneFailed(msg) => StoreError::NetworkError(msg),
            GitStoreError::FetchFailed(msg) => StoreError::NetworkError(msg),
            GitStoreError::AuthenticationFailed => {
                StoreError::PermissionDenied("Git authentication failed".to_string())
            }
            GitStoreError::NetworkTimeout => StoreError::Timeout,
            GitStoreError::Io(err) => StoreError::IoError(err),
            GitStoreError::DirtyRepository => {
                StoreError::ValidationError("Repository has uncommitted changes".to_string())
            }
            GitStoreError::PushRejected { reason } => {
                StoreError::NetworkError(format!("Push rejected: {}", reason))
            }
            GitStoreError::MergeConflict { files } => {
                StoreError::ValidationError(format!("Merge conflict in files: {:?}", files))
            }
            GitStoreError::NotARepository => {
                StoreError::ConfigError("Directory is not a git repository".to_string())
            }
            GitStoreError::NoWritePermission { url } => {
                StoreError::PermissionDenied(format!("No write permission for repository: {}", url))
            }
            GitStoreError::ExtensionVersionExists { id, version } => StoreError::ValidationError(
                format!("Extension {} version {} already exists", id, version),
            ),
            GitStoreError::CommitFailed { reason } => {
                StoreError::RuntimeError(format!("Commit failed: {}", reason))
            }
            GitStoreError::BranchCreationFailed { branch, reason } => {
                StoreError::RuntimeError(format!("Failed to create branch {}: {}", branch, reason))
            }
            GitStoreError::InconsistentState { details } => {
                StoreError::RuntimeError(format!("Repository in inconsistent state: {}", details))
            }
        }
    }
}

#[cfg(feature = "http")]
#[derive(Error, Debug)]
pub enum HttpStoreError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),

    #[error("HTTP response error: {status}")]
    ResponseError { status: u16 },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Authentication required")]
    AuthenticationRequired,

    #[error("Server unavailable")]
    ServerUnavailable,
}

#[cfg(feature = "http")]
impl From<HttpStoreError> for StoreError {
    fn from(err: HttpStoreError) -> Self {
        match err {
            HttpStoreError::RequestFailed(msg) => StoreError::NetworkError(msg),
            HttpStoreError::ResponseError { status } => match status {
                404 => StoreError::ExtensionNotFound("HTTP 404".to_string()),
                401 | 403 => StoreError::PermissionDenied(format!("HTTP {}", status)),
                429 => StoreError::NetworkError("Rate limit exceeded".to_string()),
                500..=599 => StoreError::StoreUnavailable(format!("Server error: {}", status)),
                _ => StoreError::NetworkError(format!("HTTP error: {}", status)),
            },
            HttpStoreError::Network(err) => StoreError::NetworkError(err.to_string()),
            HttpStoreError::InvalidUrl(url) => {
                StoreError::ConfigError(format!("Invalid URL: {}", url))
            }
            HttpStoreError::DownloadFailed(msg) => StoreError::NetworkError(msg),
            HttpStoreError::RateLimitExceeded => {
                StoreError::NetworkError("Rate limit exceeded".to_string())
            }
            HttpStoreError::AuthenticationRequired => {
                StoreError::PermissionDenied("Authentication required".to_string())
            }
            HttpStoreError::ServerUnavailable => {
                StoreError::StoreUnavailable("Server unavailable".to_string())
            }
        }
    }
}
