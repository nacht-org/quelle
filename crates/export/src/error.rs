//! Error types for export operations.

use thiserror::Error;

/// Result type for export operations.
pub type Result<T> = std::result::Result<T, ExportError>;

/// Error types for export operations.
#[derive(Error, Debug)]
pub enum ExportError {
    /// Format is not supported.
    #[error("Unsupported export format: '{format}'")]
    UnsupportedFormat { format: String },

    /// Novel was not found in storage.
    #[error("Novel not found: {novel_id}")]
    NovelNotFound { novel_id: String },

    /// Storage operation failed.
    #[error("Storage error: {0}")]
    Storage(#[from] quelle_storage::BookStorageError),

    /// I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid configuration.
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },

    /// Format-specific error.
    #[error("Format error: {message}")]
    FormatError { message: String },

    /// Other error.
    #[error("Export error: {0}")]
    Other(#[from] eyre::Report),
}
