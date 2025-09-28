//! Supporting types for the book storage system.

use serde::{Deserialize, Serialize};

/// Unique identifier for a novel within the storage system.
///
/// This is a simple string wrapper that allows different backends to use
/// whatever identification scheme works best for them (URLs, UUIDs, integers, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NovelId(pub String);

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
    pub statuses: Vec<NovelStatus>,
    pub title_contains: Option<String>,
    pub has_content: Option<bool>,
}

/// Information about a chapter with volume context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterInfo {
    pub volume_index: i32,
    pub chapter_url: String,
    pub chapter_title: String,
    pub chapter_index: i32,
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

/// Storage system statistics.
#[derive(Debug, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_novels: u64,
    pub total_chapters: u64,
    pub novels_by_source: Vec<(String, u64)>,
}

impl StorageStats {
    /// Create a new empty storage stats
    pub fn new() -> Self {
        Self {
            total_novels: 0,
            total_chapters: 0,
            novels_by_source: Vec::new(),
        }
    }
}

impl Default for StorageStats {
    fn default() -> Self {
        Self::new()
    }
}
