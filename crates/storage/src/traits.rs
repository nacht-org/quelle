//! Trait definitions for the book storage system.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{ChapterInfo, CleanupReport, NovelFilter, NovelId, NovelSummary, StorageStats};

use crate::{ChapterContent, Novel};

/// Main trait for book storage operations.
///
/// This trait defines the interface for storing and retrieving e-book content,
/// including novels, chapters, and metadata. Implementations can use different
/// backends such as file systems, databases, or cloud storage.
#[async_trait]
pub trait BookStorage: Send + Sync {
    // === Novel Operations ===

    /// Store a complete novel with its metadata and chapter structure.
    ///
    /// # Arguments
    /// * `novel` - The novel data to store
    ///
    /// # Returns
    /// The generated `NovelId` for the stored novel
    async fn store_novel(&self, novel: &Novel) -> Result<NovelId>;

    /// Get a novel by its ID.
    ///
    /// # Returns
    /// `Some(novel)` if found, `None` if not found
    async fn get_novel(&self, id: &NovelId) -> Result<Option<Novel>>;

    /// Update an existing novel's metadata and structure.
    ///
    /// # Arguments
    /// * `id` - The novel ID to update
    /// * `novel` - The updated novel data
    async fn update_novel(&self, id: &NovelId, novel: &Novel) -> Result<()>;

    /// Delete a novel and all its associated data.
    ///
    /// # Returns
    /// `true` if the novel was deleted, `false` if it didn't exist
    async fn delete_novel(&self, id: &NovelId) -> Result<bool>;

    /// Check if a novel exists.
    async fn exists_novel(&self, id: &NovelId) -> Result<bool>;

    // === Chapter Content Operations ===

    /// Store content for a specific chapter.
    ///
    /// # Arguments
    /// * `novel_id` - The novel this chapter belongs to
    /// * `volume_index` - The volume index (from the novel's volume structure)
    /// * `chapter_url` - The chapter's URL (unique identifier within the novel)
    /// * `content` - The chapter content to store
    async fn store_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
        content: &ChapterContent,
    ) -> Result<()>;

    /// Get content for a specific chapter.
    ///
    /// # Returns
    /// `Some(content)` if found, `None` if not found
    async fn get_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<Option<ChapterContent>>;

    /// Delete content for a specific chapter.
    ///
    /// # Returns
    /// `true` if the chapter content was deleted, `false` if it didn't exist
    async fn delete_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<bool>;

    /// Check if chapter content exists.
    async fn exists_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<bool>;

    // === Query Operations ===

    /// List novels with optional filtering.
    async fn list_novels(&self, filter: &NovelFilter) -> Result<Vec<NovelSummary>>;

    /// Find novels from a specific source.
    async fn find_novels_by_source(&self, source_id: &str) -> Result<Vec<NovelSummary>>;

    /// Find a novel by its URL.
    ///
    /// This is the most common lookup pattern since users typically know the novel URL.
    async fn find_novel_by_url(&self, url: &str) -> Result<Option<Novel>>;

    /// Search novels by text query.
    ///
    /// Performs a text-based search across novel titles, authors, and descriptions.
    async fn search_novels(&self, query: &str) -> Result<Vec<NovelSummary>>;

    /// Count total novels matching filter.
    async fn count_novels(&self, filter: &NovelFilter) -> Result<u64>;

    /// List chapters for a novel.
    ///
    /// Returns information about all chapters for the novel,
    /// including volume and chapter metadata.
    async fn list_chapters(&self, novel_id: &NovelId) -> Result<Vec<ChapterInfo>>;

    // === Maintenance Operations ===

    /// Remove orphaned chapter content and fix inconsistencies.
    ///
    /// This operation scans the storage for:
    /// - Chapter content without corresponding novels
    /// - Broken references and inconsistent data
    /// - Other integrity issues
    ///
    /// # Returns
    /// A report detailing what was cleaned up and any errors encountered
    async fn cleanup_dangling_data(&self) -> Result<CleanupReport>;

    /// Get storage statistics.
    ///
    /// Returns information about the current state of the storage,
    /// including counts and size information.
    async fn get_storage_stats(&self) -> Result<StorageStats>;
}
