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
    Asset, AssetId, ChapterContentStatus, ChapterInfo, CleanupReport, NovelFilter,
    NovelId, NovelSummary,
};

// Re-export domain types from quelle_types
pub use quelle_types::{ChapterContent, Novel, NovelStatus};
