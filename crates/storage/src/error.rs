//! Error types for the book storage system.

use thiserror::Error;

/// Errors that can occur during book storage operations.
#[derive(Debug, Error)]
pub enum BookStorageError {
    #[error("Novel not found: {id}")]
    NovelNotFound {
        id: String,
        #[source]
        source: Option<eyre::Report>,
    },

    #[error("Chapter not found: novel={novel_id}, volume={volume_index}, chapter={chapter_url}")]
    ChapterNotFound {
        novel_id: String,
        volume_index: i32,
        chapter_url: String,
        #[source]
        source: Option<eyre::Report>,
    },

    #[error("Novel already exists: {id}")]
    NovelAlreadyExists {
        id: String,
        #[source]
        source: Option<eyre::Report>,
    },

    #[error("Invalid novel data: {message}")]
    InvalidNovelData {
        message: String,
        #[source]
        source: Option<eyre::Report>,
    },

    #[error("Invalid chapter data: {message}")]
    InvalidChapterData {
        message: String,
        #[source]
        source: Option<eyre::Report>,
    },

    #[error("Data conversion failed: {message}")]
    DataConversionError {
        message: String,
        #[source]
        source: Option<eyre::Report>,
    },

    #[error("Storage operation failed: {operation}")]
    StorageOperationFailed {
        operation: String,
        #[source]
        source: Option<eyre::Report>,
    },

    #[error("Storage backend error")]
    BackendError {
        #[source]
        source: Option<eyre::Report>,
    },
}

/// Result type alias for book storage operations.
pub type Result<T> = std::result::Result<T, BookStorageError>;
