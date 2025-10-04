//! Supporting types for the book storage system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for a novel within the storage system.
///
/// This is a simple string wrapper that allows different backends to use
/// whatever identification scheme works best for them (URLs, UUIDs, integers, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NovelId(String);

impl NovelId {
    /// Create a new NovelId
    pub fn new(id: String) -> Self {
        Self(id)
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NovelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for NovelId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for NovelId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

/// Lightweight summary of a novel for listing/searching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovelSummary {
    pub id: NovelId,
    pub title: String,
    pub authors: Vec<String>,
    pub status: NovelStatus,
    pub total_chapters: u32,
    pub stored_chapters: u32,
}

/// Status of a novel (from WIT definitions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NovelStatus {
    Ongoing,
    Hiatus,
    Completed,
    Stub,
    Dropped,
    Unknown,
}

/// Filter criteria for querying novels.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NovelFilter {
    pub source_ids: Vec<String>,
}

/// Status of chapter content storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChapterContentStatus {
    /// Chapter content is not stored
    NotStored,
    /// Chapter content is stored with metadata
    Stored {
        stored_at: DateTime<Utc>,
        content_size: u64,
        updated_at: DateTime<Utc>,
    },
}

impl ChapterContentStatus {
    /// Check if content is stored
    pub fn is_stored(&self) -> bool {
        matches!(self, ChapterContentStatus::Stored { .. })
    }

    /// Get content size if stored
    pub fn content_size(&self) -> Option<u64> {
        match self {
            ChapterContentStatus::NotStored => None,
            ChapterContentStatus::Stored { content_size, .. } => Some(*content_size),
        }
    }

    /// Get stored timestamp if stored
    pub fn stored_at(&self) -> Option<DateTime<Utc>> {
        match self {
            ChapterContentStatus::NotStored => None,
            ChapterContentStatus::Stored { stored_at, .. } => Some(*stored_at),
        }
    }

    /// Get last updated timestamp if stored
    pub fn updated_at(&self) -> Option<DateTime<Utc>> {
        match self {
            ChapterContentStatus::NotStored => None,
            ChapterContentStatus::Stored { updated_at, .. } => Some(*updated_at),
        }
    }
}

/// Information about a chapter with volume context and storage metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterInfo {
    pub volume_index: i32,
    pub chapter_url: String,
    pub chapter_title: String,
    pub chapter_index: i32,

    // Storage status
    pub content_status: ChapterContentStatus,
}

impl ChapterInfo {
    /// Create a new ChapterInfo without storage metadata (content not stored)
    pub fn new(
        volume_index: i32,
        chapter_url: String,
        chapter_title: String,
        chapter_index: i32,
    ) -> Self {
        Self {
            volume_index,
            chapter_url,
            chapter_title,
            chapter_index,
            content_status: ChapterContentStatus::NotStored,
        }
    }

    /// Create a new ChapterInfo with storage metadata (content is stored)
    pub fn with_content(
        volume_index: i32,
        chapter_url: String,
        chapter_title: String,
        chapter_index: i32,
        stored_at: DateTime<Utc>,
        content_size: u64,
    ) -> Self {
        Self {
            volume_index,
            chapter_url,
            chapter_title,
            chapter_index,
            content_status: ChapterContentStatus::Stored {
                stored_at,
                content_size,
                updated_at: stored_at,
            },
        }
    }

    /// Update storage metadata when content is stored
    pub fn mark_stored(&mut self, content_size: u64) {
        let now = Utc::now();
        match &self.content_status {
            ChapterContentStatus::NotStored => {
                self.content_status = ChapterContentStatus::Stored {
                    stored_at: now,
                    content_size,
                    updated_at: now,
                };
            }
            ChapterContentStatus::Stored { stored_at, .. } => {
                self.content_status = ChapterContentStatus::Stored {
                    stored_at: *stored_at, // Keep original stored time
                    content_size,
                    updated_at: now,
                };
            }
        }
    }

    /// Mark content as removed
    pub fn mark_removed(&mut self) {
        self.content_status = ChapterContentStatus::NotStored;
    }

    /// Check if content is stored
    pub fn has_content(&self) -> bool {
        self.content_status.is_stored()
    }

    /// Get content size if available
    pub fn content_size(&self) -> Option<u64> {
        self.content_status.content_size()
    }

    /// Get stored timestamp if available
    pub fn stored_at(&self) -> Option<DateTime<Utc>> {
        self.content_status.stored_at()
    }

    /// Get updated timestamp if available
    pub fn updated_at(&self) -> Option<DateTime<Utc>> {
        self.content_status.updated_at()
    }
}

/// Report from cleanup operations.
#[derive(Debug, Serialize, Deserialize)]
pub struct CleanupReport {
    pub orphaned_chapters_removed: u32,
    pub novels_fixed: u32,
    pub errors_encountered: Vec<String>,
}

impl CleanupReport {
    /// Create a new empty cleanup report
    pub fn new() -> Self {
        Self {
            orphaned_chapters_removed: 0,
            novels_fixed: 0,
            errors_encountered: Vec::new(),
        }
    }

    /// Add an error to the report
    pub fn add_error(&mut self, error: String) {
        self.errors_encountered.push(error);
    }

    /// Check if the cleanup was successful (no errors)
    pub fn is_successful(&self) -> bool {
        self.errors_encountered.is_empty()
    }
}

impl Default for CleanupReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for an asset within the storage system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(pub String);

impl AssetId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AssetId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for AssetId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

/// Asset metadata without binary data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub novel_id: NovelId,
    pub original_url: String,
    pub mime_type: String,
    pub size: u64,
    pub filename: String,
}

/// Summary information about an asset (without binary data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSummary {
    pub id: AssetId,
    pub novel_id: NovelId,
    pub original_url: String,
    pub mime_type: String,
    pub size: u64,
    pub filename: String,
}
