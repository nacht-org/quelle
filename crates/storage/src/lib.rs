//! Book storage interface and implementations for the Quelle project.
//!
//! This crate provides a trait-based storage system for managing e-book content,
//! including novels, chapters, and related metadata.

pub mod backends;
pub mod error;
pub mod models;
pub mod traits;
pub mod types;

// Re-export the main interface and types for easy access
pub use backends::FilesystemStorage;
pub use error::{BookStorageError, Result};
pub use traits::BookStorage;
pub use types::{
    ChapterContentStatus, ChapterInfo, CleanupReport, NovelFilter, NovelId, NovelSummary,
    StorageStats,
};

// Import the WIT types that we'll be working with
pub use quelle_engine::bindings::quelle::extension::novel::{
    ChapterContent, Novel, NovelStatus as WitNovelStatus,
};

// Convert between our types and WIT types
impl From<WitNovelStatus> for types::NovelStatus {
    fn from(status: WitNovelStatus) -> Self {
        match status {
            WitNovelStatus::Ongoing => Self::Ongoing,
            WitNovelStatus::Hiatus => Self::Hiatus,
            WitNovelStatus::Completed => Self::Completed,
            WitNovelStatus::Stub => Self::Stub,
            WitNovelStatus::Dropped => Self::Dropped,
            WitNovelStatus::Unknown => Self::Unknown,
        }
    }
}

impl From<types::NovelStatus> for WitNovelStatus {
    fn from(status: types::NovelStatus) -> Self {
        match status {
            types::NovelStatus::Ongoing => Self::Ongoing,
            types::NovelStatus::Hiatus => Self::Hiatus,
            types::NovelStatus::Completed => Self::Completed,
            types::NovelStatus::Stub => Self::Stub,
            types::NovelStatus::Dropped => Self::Dropped,
            types::NovelStatus::Unknown => Self::Unknown,
        }
    }
}
