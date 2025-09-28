//! Filesystem-based storage backend implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::{BookStorageError, Result};
use crate::models::{
    chapter_content_from_json, chapter_content_to_json, novel_from_json, novel_to_json,
};
use crate::traits::BookStorage;
use crate::types::{ChapterInfo, CleanupReport, NovelFilter, NovelId, NovelSummary, StorageStats};
use crate::{ChapterContent, Novel};

/// Filesystem-based storage backend.
///
/// This implementation stores novels and chapters as JSON files in a structured
/// directory hierarchy on the local filesystem.
///
/// Directory structure:
/// ```text
/// storage_root/
/// +-- novels/
/// |   +-- {source_id}/
/// |       +-- {novel_url_hash}/
/// |           +-- novel.json
/// |           +-- chapters/
/// |               +-- {volume_index}/
/// |                   +-- {chapter_url_hash}.json
/// +-- metadata/
///     +-- stats.json
///     +-- index.json
/// ```
#[derive(Debug, Clone)]
pub struct FilesystemStorage {
    root_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct NovelStorageMetadata {
    source_id: String,
    stored_at: String, // ISO 8601 timestamp
}

#[derive(Debug, Serialize, Deserialize)]
struct ChapterStorageMetadata {
    volume_index: i32,
    chapter_url: String,
    stored_at: String, // ISO 8601 timestamp
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StorageIndex {
    novels: Vec<IndexedNovel>,
    last_updated: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexedNovel {
    id: NovelId,
    title: String,
    authors: Vec<String>,
    status: crate::types::NovelStatus,
    total_chapters: u32,
    stored_chapters: u32,
    created_at: String,
    updated_at: String,
}

impl FilesystemStorage {
    /// Create a new filesystem storage backend.
    ///
    /// # Arguments
    /// * `root_path` - Path to the root storage directory
    pub fn new<P: AsRef<Path>>(root_path: P) -> Self {
        Self {
            root_path: root_path.as_ref().to_path_buf(),
        }
    }

    /// Initialize the storage directory structure.
    pub async fn initialize(&self) -> Result<()> {
        let novels_dir = self.root_path.join("novels");
        let metadata_dir = self.root_path.join("metadata");

        fs::create_dir_all(&novels_dir)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to create novels directory: {}", e)),
            })?;

        fs::create_dir_all(&metadata_dir)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to create metadata directory: {}", e)),
            })?;

        // Initialize index if it doesn't exist
        let index_path = self.get_index_path();
        if !index_path.exists() {
            self.save_index(&StorageIndex::default()).await?;
        }

        Ok(())
    }

    fn get_novel_dir(&self, novel_id: &NovelId) -> PathBuf {
        // Extract source_id and novel_url from the composite ID
        let id_str = novel_id.as_str();
        let parts: Vec<&str> = id_str.splitn(2, "::").collect();
        let source_id = parts.get(0).unwrap_or(&"unknown");
        let novel_url = parts.get(1).unwrap_or(&id_str);

        let novel_hash = self.hash_string(novel_url);
        self.root_path
            .join("novels")
            .join(source_id)
            .join(novel_hash)
    }

    fn get_novel_file(&self, novel_id: &NovelId) -> PathBuf {
        self.get_novel_dir(novel_id).join("novel.json")
    }

    fn get_chapter_dir(&self, novel_id: &NovelId, volume_index: i32) -> PathBuf {
        self.get_novel_dir(novel_id)
            .join("chapters")
            .join(volume_index.to_string())
    }

    fn get_chapter_file(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> PathBuf {
        let chapter_hash = self.hash_string(chapter_url);
        self.get_chapter_dir(novel_id, volume_index)
            .join(format!("{}.json", chapter_hash))
    }

    fn get_index_path(&self) -> PathBuf {
        self.root_path.join("metadata").join("index.json")
    }

    fn hash_string(&self, input: &str) -> String {
        // Simple hash for filesystem safety - in production you might want something more sophisticated
        format!("{:x}", md5::compute(input.as_bytes()))
    }

    fn extract_source_id(&self, url: &str) -> String {
        // Extract domain from URL to use as source ID
        if let Ok(parsed_url) = url::Url::parse(url) {
            if let Some(host) = parsed_url.host_str() {
                // Remove www. prefix if present and convert to lowercase
                let clean_host = host.strip_prefix("www.").unwrap_or(host).to_lowercase();
                return clean_host;
            }
        }

        // Fallback: try to extract domain manually for malformed URLs
        if let Some(start) = url.find("://") {
            let after_protocol = &url[start + 3..];
            if let Some(end) = after_protocol.find('/') {
                let host = &after_protocol[..end];
                let clean_host = host.strip_prefix("www.").unwrap_or(host).to_lowercase();
                return clean_host;
            } else {
                let clean_host = after_protocol
                    .strip_prefix("www.")
                    .unwrap_or(after_protocol)
                    .to_lowercase();
                return clean_host;
            }
        }

        // Final fallback
        "unknown".to_string()
    }

    async fn load_index(&self) -> Result<StorageIndex> {
        let index_path = self.get_index_path();
        if !index_path.exists() {
            return Ok(StorageIndex::default());
        }

        let content =
            fs::read_to_string(&index_path)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to read index file: {}", e)),
                })?;

        serde_json::from_str(&content).map_err(|e| BookStorageError::DataConversionError {
            message: "Failed to deserialize index".to_string(),
            source: Some(eyre::eyre!("JSON error: {}", e)),
        })
    }

    async fn save_index(&self, index: &StorageIndex) -> Result<()> {
        let index_path = self.get_index_path();

        let content = serde_json::to_string_pretty(index).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to serialize index".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        fs::write(&index_path, content)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to write index file: {}", e)),
            })?;

        Ok(())
    }

    async fn update_index_for_novel(&self, novel_id: &NovelId, novel: &Novel) -> Result<()> {
        let mut index = self.load_index().await?;
        let now = chrono::Utc::now().to_rfc3339();

        // Remove existing entry if it exists
        index.novels.retain(|n| n.id != *novel_id);

        // Count stored chapters
        let stored_chapters = self.count_stored_chapters_for_novel(novel_id).await?;

        // Add updated entry
        let indexed_novel = IndexedNovel {
            id: novel_id.clone(),
            title: novel.title.clone(),
            authors: novel.authors.clone(),
            status: novel.status.into(),
            total_chapters: self.count_total_chapters(novel),
            stored_chapters,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        index.novels.push(indexed_novel);
        index.last_updated = now;

        self.save_index(&index).await
    }

    async fn remove_from_index(&self, novel_id: &NovelId) -> Result<()> {
        let mut index = self.load_index().await?;
        index.novels.retain(|n| n.id != *novel_id);
        index.last_updated = chrono::Utc::now().to_rfc3339();
        self.save_index(&index).await
    }

    fn count_total_chapters(&self, novel: &Novel) -> u32 {
        novel
            .volumes
            .iter()
            .map(|volume| volume.chapters.len() as u32)
            .sum()
    }

    async fn count_stored_chapters_for_novel(&self, novel_id: &NovelId) -> Result<u32> {
        let novel_dir = self.get_novel_dir(novel_id);
        let chapters_dir = novel_dir.join("chapters");

        if !chapters_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        let mut entries =
            fs::read_dir(&chapters_dir)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to read chapters directory: {}", e)),
                })?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.file_type().await.map_or(false, |ft| ft.is_dir()) {
                let volume_dir = entry.path();
                let mut volume_entries = fs::read_dir(&volume_dir).await.map_err(|e| {
                    BookStorageError::BackendError {
                        source: Some(eyre::eyre!("Failed to read volume directory: {}", e)),
                    }
                })?;

                while let Ok(Some(chapter_entry)) = volume_entries.next_entry().await {
                    if chapter_entry
                        .file_type()
                        .await
                        .map_or(false, |ft| ft.is_file())
                        && chapter_entry
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".json")
                    {
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    async fn update_index_stored_chapters(&self, novel_id: &NovelId) -> Result<()> {
        let mut index = self.load_index().await?;

        // Find the novel in the index and update its stored chapter count
        if let Some(indexed_novel) = index.novels.iter_mut().find(|n| n.id == *novel_id) {
            indexed_novel.stored_chapters = self.count_stored_chapters_for_novel(novel_id).await?;
            self.save_index(&index).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl BookStorage for FilesystemStorage {
    async fn store_novel(&self, novel: &Novel) -> Result<NovelId> {
        // Validate input data
        if novel.url.trim().is_empty() {
            return Err(BookStorageError::InvalidNovelData {
                message: "Novel URL cannot be empty".to_string(),
                source: None,
            });
        }

        if novel.title.trim().is_empty() {
            return Err(BookStorageError::InvalidNovelData {
                message: "Novel title cannot be empty".to_string(),
                source: None,
            });
        }

        // Generate an ID based on the novel URL for this backend
        let source_id = self.extract_source_id(&novel.url);
        let id_string = format!("{}::{}", source_id, novel.url);
        let novel_id = NovelId::new(id_string);
        let novel_file = self.get_novel_file(&novel_id);

        // Create directory structure
        if let Some(parent) = novel_file.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to create novel directory: {}", e)),
                })?;
        }

        // Check if novel already exists
        if novel_file.exists() {
            return Err(BookStorageError::NovelAlreadyExists {
                id: novel_id.as_str().to_string(),
                source: None,
            });
        }

        // Convert novel to JSON string using conversion utilities
        let novel_json = novel_to_json(novel)?;

        // Store metadata separately
        let source_id = self.extract_source_id(&novel.url);
        let metadata = NovelStorageMetadata {
            source_id,
            stored_at: chrono::Utc::now().to_rfc3339(),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to serialize novel metadata".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        // Create a combined JSON object
        let novel_value = serde_json::from_str::<serde_json::Value>(&novel_json).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to parse novel JSON".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        let metadata_value =
            serde_json::from_str::<serde_json::Value>(&metadata_json).map_err(|e| {
                BookStorageError::DataConversionError {
                    message: "Failed to parse metadata JSON".to_string(),
                    source: Some(eyre::eyre!("JSON error: {}", e)),
                }
            })?;

        let combined = serde_json::json!({
            "novel": novel_value,
            "metadata": metadata_value
        });

        let combined_json = serde_json::to_string_pretty(&combined).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to create combined JSON".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        fs::write(&novel_file, combined_json).await.map_err(|e| {
            BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to write novel file: {}", e)),
            }
        })?;

        // Update index
        self.update_index_for_novel(&novel_id, novel).await?;

        Ok(novel_id)
    }

    async fn get_novel(&self, id: &NovelId) -> Result<Option<Novel>> {
        let novel_file = self.get_novel_file(id);

        if !novel_file.exists() {
            return Ok(None);
        }

        let content =
            fs::read_to_string(&novel_file)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to read novel file: {}", e)),
                })?;

        // Parse the combined JSON
        let combined: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| BookStorageError::DataConversionError {
                message: "Failed to parse novel file".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            })?;

        // Extract the novel part and convert it back
        let novel_value = combined["novel"].clone();
        let novel_json = serde_json::to_string(&novel_value).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to extract novel data".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        let novel = novel_from_json(&novel_json)?;

        Ok(Some(novel))
    }

    async fn update_novel(&self, id: &NovelId, novel: &Novel) -> Result<()> {
        let novel_file = self.get_novel_file(id);

        if !novel_file.exists() {
            return Err(BookStorageError::NovelNotFound {
                id: id.as_str().to_string(),
                source: None,
            });
        }

        // Convert novel to JSON string using conversion utilities
        let novel_json = novel_to_json(novel)?;

        // Store metadata separately
        let metadata = NovelStorageMetadata {
            source_id: id
                .as_str()
                .split("::")
                .next()
                .unwrap_or("unknown")
                .to_string(),
            stored_at: chrono::Utc::now().to_rfc3339(),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to serialize novel metadata".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        // Create a combined JSON object
        let novel_value = serde_json::from_str::<serde_json::Value>(&novel_json).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to parse novel JSON".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        let metadata_value =
            serde_json::from_str::<serde_json::Value>(&metadata_json).map_err(|e| {
                BookStorageError::DataConversionError {
                    message: "Failed to parse metadata JSON".to_string(),
                    source: Some(eyre::eyre!("JSON error: {}", e)),
                }
            })?;

        let combined = serde_json::json!({
            "novel": novel_value,
            "metadata": metadata_value
        });

        let combined_json = serde_json::to_string_pretty(&combined).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to create combined JSON".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        fs::write(&novel_file, combined_json).await.map_err(|e| {
            BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to write novel file: {}", e)),
            }
        })?;

        // Update index
        self.update_index_for_novel(id, novel).await?;

        Ok(())
    }

    async fn delete_novel(&self, id: &NovelId) -> Result<bool> {
        let novel_dir = self.get_novel_dir(id);

        if !novel_dir.exists() {
            return Ok(false);
        }

        fs::remove_dir_all(&novel_dir)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to delete novel directory: {}", e)),
            })?;

        // Remove from index
        self.remove_from_index(id).await?;

        Ok(true)
    }

    async fn exists_novel(&self, id: &NovelId) -> Result<bool> {
        Ok(self.get_novel_file(id).exists())
    }

    async fn store_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
        content: &ChapterContent,
    ) -> Result<()> {
        // Validate input data
        if chapter_url.trim().is_empty() {
            return Err(BookStorageError::InvalidChapterData {
                message: "Chapter URL cannot be empty".to_string(),
                source: None,
            });
        }

        if content.data.trim().is_empty() {
            return Err(BookStorageError::InvalidChapterData {
                message: "Chapter content cannot be empty".to_string(),
                source: None,
            });
        }

        // Check if novel exists
        if !self.exists_novel(novel_id).await? {
            return Err(BookStorageError::NovelNotFound {
                id: novel_id.to_string(),
                source: None,
            });
        }

        let chapter_file = self.get_chapter_file(novel_id, volume_index, chapter_url);

        // Create directory structure
        if let Some(parent) = chapter_file.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to create chapter directory: {}", e)),
                })?;
        }

        // Convert chapter content to JSON string using conversion utilities
        let content_json = chapter_content_to_json(content)?;

        // Store metadata separately
        let metadata = ChapterStorageMetadata {
            volume_index,
            chapter_url: chapter_url.to_string(),
            stored_at: chrono::Utc::now().to_rfc3339(),
        };

        let metadata_json = serde_json::to_string_pretty(&metadata).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to serialize chapter metadata".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        // Create a combined JSON object
        let content_value =
            serde_json::from_str::<serde_json::Value>(&content_json).map_err(|e| {
                BookStorageError::DataConversionError {
                    message: "Failed to parse content JSON".to_string(),
                    source: Some(eyre::eyre!("JSON error: {}", e)),
                }
            })?;

        let metadata_value =
            serde_json::from_str::<serde_json::Value>(&metadata_json).map_err(|e| {
                BookStorageError::DataConversionError {
                    message: "Failed to parse metadata JSON".to_string(),
                    source: Some(eyre::eyre!("JSON error: {}", e)),
                }
            })?;

        let combined = serde_json::json!({
            "content": content_value,
            "metadata": metadata_value
        });

        let combined_json = serde_json::to_string_pretty(&combined).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to create combined JSON".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        fs::write(&chapter_file, combined_json).await.map_err(|e| {
            BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to write chapter file: {}", e)),
            }
        })?;

        // Update index to reflect the new stored chapter count
        self.update_index_stored_chapters(novel_id).await?;

        Ok(())
    }

    async fn get_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<Option<ChapterContent>> {
        let chapter_file = self.get_chapter_file(novel_id, volume_index, chapter_url);

        if !chapter_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&chapter_file).await.map_err(|e| {
            BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to read chapter file: {}", e)),
            }
        })?;

        // Parse the combined JSON
        let combined: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| BookStorageError::DataConversionError {
                message: "Failed to parse chapter file".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            })?;

        // Extract the content part and convert it back
        let content_value = combined["content"].clone();
        let content_json = serde_json::to_string(&content_value).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to extract chapter content data".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        let chapter_content = chapter_content_from_json(&content_json)?;

        Ok(Some(chapter_content))
    }

    async fn delete_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<bool> {
        let chapter_file = self.get_chapter_file(novel_id, volume_index, chapter_url);

        if !chapter_file.exists() {
            return Ok(false);
        }

        fs::remove_file(&chapter_file)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to delete chapter file: {}", e)),
            })?;

        // Update index to reflect the reduced stored chapter count
        self.update_index_stored_chapters(novel_id).await?;

        Ok(true)
    }

    async fn exists_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<bool> {
        Ok(self
            .get_chapter_file(novel_id, volume_index, chapter_url)
            .exists())
    }

    async fn list_novels(&self, filter: &NovelFilter) -> Result<Vec<NovelSummary>> {
        let index = self.load_index().await?;

        let mut summaries: Vec<NovelSummary> = index
            .novels
            .into_iter()
            .filter(|novel| {
                // Apply filters
                if !filter.source_ids.is_empty() {
                    let parts: Vec<&str> = novel.id.as_str().splitn(2, "::").collect();
                    let source_id = parts.get(0).unwrap_or(&"unknown");
                    if !filter.source_ids.contains(&source_id.to_string()) {
                        return false;
                    }
                }

                if !filter.statuses.is_empty() && !filter.statuses.contains(&novel.status) {
                    return false;
                }

                if let Some(ref title_filter) = filter.title_contains {
                    if !novel
                        .title
                        .to_lowercase()
                        .contains(&title_filter.to_lowercase())
                    {
                        return false;
                    }
                }

                if let Some(has_content) = filter.has_content {
                    if has_content && novel.stored_chapters == 0 {
                        return false;
                    }
                    if !has_content && novel.stored_chapters > 0 {
                        return false;
                    }
                }

                true
            })
            .map(|novel| NovelSummary {
                id: novel.id,
                title: novel.title,
                authors: novel.authors,
                status: novel.status,
                total_chapters: novel.total_chapters,
                stored_chapters: novel.stored_chapters,
            })
            .collect();

        // Sort by title for consistent ordering
        summaries.sort_by(|a, b| a.title.cmp(&b.title));

        Ok(summaries)
    }

    async fn find_novels_by_source(&self, source_id: &str) -> Result<Vec<NovelSummary>> {
        let filter = NovelFilter {
            source_ids: vec![source_id.to_string()],
            ..Default::default()
        };
        self.list_novels(&filter).await
    }

    async fn find_novel_by_url(&self, url: &str) -> Result<Option<Novel>> {
        let index = self.load_index().await?;

        // Find the novel in our index that matches the URL
        for indexed_novel in &index.novels {
            let parts: Vec<&str> = indexed_novel.id.as_str().splitn(2, "::").collect();
            if let Some(novel_url) = parts.get(1) {
                if *novel_url == url {
                    return self.get_novel(&indexed_novel.id).await;
                }
            }
        }

        Ok(None)
    }

    async fn search_novels(&self, query: &str) -> Result<Vec<NovelSummary>> {
        let filter = NovelFilter {
            title_contains: Some(query.to_string()),
            ..Default::default()
        };
        self.list_novels(&filter).await
    }

    async fn count_novels(&self, filter: &NovelFilter) -> Result<u64> {
        let novels = self.list_novels(filter).await?;
        Ok(novels.len() as u64)
    }

    async fn list_chapters(&self, novel_id: &NovelId) -> Result<Vec<ChapterInfo>> {
        // Get the novel first to access chapter metadata
        let novel = self.get_novel(novel_id).await?;
        let novel = match novel {
            Some(n) => n,
            None => return Ok(Vec::new()),
        };

        let mut stored_chapters = Vec::new();

        // Iterate through volumes and chapters
        for volume in &novel.volumes {
            for chapter in &volume.chapters {
                stored_chapters.push(ChapterInfo {
                    volume_index: volume.index,
                    chapter_url: chapter.url.clone(),
                    chapter_title: chapter.title.clone(),
                    chapter_index: chapter.index,
                });
            }
        }

        // Sort by volume index, then chapter index
        stored_chapters.sort_by(|a, b| {
            a.volume_index
                .cmp(&b.volume_index)
                .then(a.chapter_index.cmp(&b.chapter_index))
        });

        Ok(stored_chapters)
    }

    async fn cleanup_dangling_data(&self) -> Result<CleanupReport> {
        let report = CleanupReport::new();

        // TODO: Implement cleanup logic
        // This would involve:
        // 1. Finding chapter files without corresponding novels
        // 2. Finding novels without proper index entries
        // 3. Cleaning up empty directories
        // 4. Validating JSON files

        // For now, just return empty report
        Ok(report)
    }

    async fn get_storage_stats(&self) -> Result<StorageStats> {
        let index = self.load_index().await?;

        let total_novels = index.novels.len() as u64;
        let total_chapters: u64 = index.novels.iter().map(|n| n.stored_chapters as u64).sum();

        // Group by source
        let mut novels_by_source = std::collections::HashMap::new();
        for novel in &index.novels {
            let parts: Vec<&str> = novel.id.as_str().splitn(2, "::").collect();
            let source_id = parts.get(0).unwrap_or(&"unknown").to_string();
            *novels_by_source.entry(source_id).or_insert(0) += 1;
        }

        let novels_by_source: Vec<(String, u64)> = novels_by_source.into_iter().collect();

        Ok(StorageStats {
            total_novels,
            total_chapters,
            novels_by_source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quelle_engine::bindings::quelle::extension::novel::{
        Chapter, ChapterContent, Novel, NovelStatus, Volume,
    };
    use tempfile::TempDir;

    fn create_test_novel() -> Novel {
        Novel {
            url: "https://test.com/novel".to_string(),
            authors: vec!["Test Author".to_string()],
            title: "Test Novel".to_string(),
            cover: None,
            description: vec!["A test novel".to_string()],
            volumes: vec![Volume {
                name: "Volume 1".to_string(),
                index: 1,
                chapters: vec![Chapter {
                    title: "Chapter 1".to_string(),
                    index: 1,
                    url: "https://test.com/chapter-1".to_string(),
                    updated_at: None,
                }],
            }],
            metadata: vec![],
            status: NovelStatus::Ongoing,
            langs: vec!["en".to_string()],
        }
    }

    #[tokio::test]
    async fn test_filesystem_storage_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());

        storage.initialize().await.unwrap();

        assert!(temp_dir.path().join("novels").exists());
        assert!(temp_dir.path().join("metadata").exists());
        assert!(temp_dir.path().join("metadata/index.json").exists());
    }

    #[tokio::test]
    async fn test_store_and_retrieve_novel() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        let retrieved = storage.get_novel(&novel_id).await.unwrap();
        assert!(retrieved.is_some());

        let retrieved_novel = retrieved.unwrap();
        assert_eq!(retrieved_novel.title, novel.title);
        assert_eq!(retrieved_novel.authors, novel.authors);

        // Test find by URL
        let found_by_url = storage.find_novel_by_url(&novel.url).await.unwrap();
        assert!(found_by_url.is_some());
        assert_eq!(found_by_url.unwrap().title, novel.title);
    }

    #[tokio::test]
    async fn test_store_and_retrieve_chapter_content() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        let content = ChapterContent {
            data: "Test chapter content".to_string(),
        };

        storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        let retrieved = storage
            .get_chapter_content(&novel_id, 1, "https://test.com/chapter-1")
            .await
            .unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, content.data);
    }

    #[tokio::test]
    async fn test_list_novels() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        storage.store_novel(&novel).await.unwrap();

        let filter = NovelFilter::default();
        let novels = storage.list_novels(&filter).await.unwrap();

        assert_eq!(novels.len(), 1);
        assert_eq!(novels[0].title, novel.title);
    }

    #[tokio::test]
    async fn test_find_novel_by_url() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        storage.store_novel(&novel).await.unwrap();

        // Test finding by URL
        let found = storage.find_novel_by_url(&novel.url).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, novel.title);

        // Test not found
        let not_found = storage
            .find_novel_by_url("https://nonexistent.com")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_validation_errors() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Test empty URL validation
        let mut invalid_novel = create_test_novel();
        invalid_novel.url = "".to_string();
        let result = storage.store_novel(&invalid_novel).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::BookStorageError::InvalidNovelData { .. }
        ));

        // Test empty title validation
        let mut invalid_novel = create_test_novel();
        invalid_novel.title = "".to_string();
        let result = storage.store_novel(&invalid_novel).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::BookStorageError::InvalidNovelData { .. }
        ));

        // Test chapter validation - empty URL
        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();
        let content = ChapterContent {
            data: "Test content".to_string(),
        };
        let result = storage
            .store_chapter_content(&novel_id, 1, "", &content)
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::BookStorageError::InvalidChapterData { .. }
        ));

        // Test chapter validation - empty content
        let content = ChapterContent {
            data: "".to_string(),
        };
        let result = storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter", &content)
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::BookStorageError::InvalidChapterData { .. }
        ));
    }

    #[tokio::test]
    async fn test_chapter_counting_updates() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        // Initially no stored chapters
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.total_chapters, 0);

        // Store a chapter
        let content = ChapterContent {
            data: "Test chapter content".to_string(),
        };
        storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        // Check that stats are updated
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.total_chapters, 1);

        // Store another chapter
        storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-2", &content)
            .await
            .unwrap();

        // Check that stats are updated again
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.total_chapters, 2);

        // Delete a chapter
        let deleted = storage
            .delete_chapter_content(&novel_id, 1, "https://test.com/chapter-1")
            .await
            .unwrap();
        assert!(deleted);

        // Check that stats are updated after deletion
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.total_chapters, 1);
    }

    #[tokio::test]
    async fn test_source_id_extraction() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Test URL with domain
        let mut novel = create_test_novel();
        novel.url = "https://example.com/novel/test".to_string();
        let novel_id = storage.store_novel(&novel).await.unwrap();
        assert!(novel_id.as_str().starts_with("example.com::"));

        // Test URL with www prefix
        let mut novel2 = create_test_novel();
        novel2.url = "https://www.example.com/novel/test2".to_string();
        let novel_id2 = storage.store_novel(&novel2).await.unwrap();
        assert!(novel_id2.as_str().starts_with("example.com::")); // www should be stripped

        // Verify storage stats show correct source grouping
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.novels_by_source.len(), 1); // Both should be grouped under example.com
        assert_eq!(stats.novels_by_source[0].0, "example.com");
        assert_eq!(stats.novels_by_source[0].1, 2);
    }

    #[tokio::test]
    async fn test_chapter_storage_nonexistent_novel() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let fake_novel_id = NovelId::new("fake::https://fake.com/novel".to_string());
        let content = ChapterContent {
            data: "Test content".to_string(),
        };

        // Try to store chapter for non-existent novel
        let result = storage
            .store_chapter_content(&fake_novel_id, 1, "https://fake.com/chapter", &content)
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::BookStorageError::NovelNotFound { .. }
        ));
    }
}
