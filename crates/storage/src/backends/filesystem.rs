//! Filesystem-based storage backend implementation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::{BookStorageError, Result};
use crate::models::{
    chapter_content_from_json, chapter_content_to_json, novel_from_json, novel_to_json,
};
use crate::traits::BookStorage;
use crate::types::{
    Asset, AssetId, AssetSummary, ChapterInfo, CleanupReport, NovelFilter, NovelId, NovelSummary,
};
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
    stored_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChapterStorageMetadata {
    volume_index: i32,
    chapter_url: String,
    stored_at: DateTime<Utc>,
    content_size: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StorageIndex {
    novels: Vec<IndexedNovel>,
    assets: Vec<IndexedAsset>,
    last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexedNovel {
    id: NovelId,
    title: String,
    authors: Vec<String>,
    status: crate::types::NovelStatus,
    total_chapters: u32,
    stored_chapters: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexedAsset {
    id: AssetId,
    novel_id: NovelId,
    original_url: String,
    mime_type: String,
    size: u64,
    filename: String,
    stored_at: DateTime<Utc>,
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
        let source_id = parts.first().unwrap_or(&"unknown");
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

    fn get_asset_metadata_file(&self, novel_id: &NovelId, asset_id: &AssetId) -> PathBuf {
        self.get_novel_dir(novel_id)
            .join("assets")
            .join("metadata")
            .join(format!("{}.json", asset_id.as_str()))
    }

    fn get_asset_data_file(&self, novel_id: &NovelId, filename: &str) -> PathBuf {
        self.get_novel_dir(novel_id)
            .join("assets")
            .join("data")
            .join(filename)
    }

    fn hash_string(&self, input: &str) -> String {
        // Simple hash for filesystem safety - in production you might want something more sophisticated
        format!("{:x}", md5::compute(input.as_bytes()))
    }

    /// Generate a filesystem-safe asset ID based on a UUID
    fn generate_asset_id(&self) -> String {
        // Generate a UUID and use only the first part for brevity
        let uuid = uuid::Uuid::new_v4().to_string();
        // Take first 8 characters for shorter filenames
        uuid[..8].to_string()
    }

    /// Extract file extension from URL
    fn extract_extension_from_url(&self, url: &str) -> Option<String> {
        if let Ok(parsed_url) = url::Url::parse(url) {
            if let Some(path_segments) = parsed_url.path_segments() {
                if let Some(last_segment) = path_segments.last() {
                    // Remove query parameters if present
                    let filename = last_segment.split('?').next().unwrap_or(last_segment);
                    if !filename.is_empty() && filename.contains('.') {
                        if let Some(extension) = filename.split('.').last() {
                            return Some(extension.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    /// Determine file extension from MIME type as fallback
    fn extension_from_mime_type(&self, mime_type: &str) -> &str {
        match mime_type {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/svg+xml" => "svg",
            "text/html" => "html",
            "text/css" => "css",
            "application/javascript" => "js",
            "application/json" => "json",
            "application/pdf" => "pdf",
            _ => "bin",
        }
    }

    /// Generate a filename with proper extension, ensuring it's filesystem safe
    fn generate_safe_filename(&self, url: &str, mime_type: &str, asset_id: &str) -> String {
        // Try to extract extension from URL first, then fallback to MIME type
        let extension = self
            .extract_extension_from_url(url)
            .unwrap_or_else(|| self.extension_from_mime_type(mime_type).to_string());

        // Always use asset_id as base filename
        let filename = format!("{}.{}", asset_id, extension);

        // Ensure filename is filesystem safe
        filename
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Normalize URL by removing trailing slashes and trimming whitespace
    fn normalize_url(&self, url: &str) -> String {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        // Remove trailing slash unless it's just the protocol (e.g., "https://")
        if trimmed.ends_with('/') && trimmed.len() > 1 && !trimmed.ends_with("://") {
            trimmed.trim_end_matches('/').to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn extract_source_id(&self, url: &str) -> String {
        let normalized_url = self.normalize_url(url);
        // Extract domain from URL to use as source ID
        if let Ok(parsed_url) = url::Url::parse(&normalized_url) {
            if let Some(host) = parsed_url.host_str() {
                // Remove www. prefix if present and convert to lowercase
                let clean_host = host.strip_prefix("www.").unwrap_or(host).to_lowercase();
                return clean_host;
            }
        }

        // Fallback: try to extract domain manually for malformed URLs
        if let Some(start) = normalized_url.find("://") {
            let after_protocol = &normalized_url[start + 3..];
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
            return Ok(StorageIndex {
                novels: Vec::new(),
                assets: Vec::new(),
                last_updated: Utc::now(),
            });
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
        let now = Utc::now();

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
            created_at: now,
            updated_at: now,
        };

        index.novels.push(indexed_novel);
        index.last_updated = now;

        self.save_index(&index).await
    }

    async fn remove_from_index(&self, novel_id: &NovelId) -> Result<()> {
        let mut index = self.load_index().await?;
        index.novels.retain(|n| n.id != *novel_id);
        index.last_updated = Utc::now();
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
            if entry.file_type().await.is_ok_and(|ft| ft.is_dir()) {
                let volume_dir = entry.path();
                let mut volume_entries = fs::read_dir(&volume_dir).await.map_err(|e| {
                    BookStorageError::BackendError {
                        source: Some(eyre::eyre!("Failed to read volume directory: {}", e)),
                    }
                })?;

                while let Ok(Some(chapter_entry)) = volume_entries.next_entry().await {
                    if chapter_entry.file_type().await.is_ok_and(|ft| ft.is_file())
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

    async fn add_asset_to_index(&self, asset: &Asset, data_size: u64) -> Result<()> {
        let mut index = self.load_index().await?;

        let indexed_asset = IndexedAsset {
            id: asset.id.clone(),
            novel_id: asset.novel_id.clone(),
            original_url: self.normalize_url(&asset.original_url),
            mime_type: asset.mime_type.clone(),
            size: data_size,
            filename: asset.filename.clone(),
            stored_at: Utc::now(),
        };

        // Remove existing entry if it exists
        index.assets.retain(|a| a.id != asset.id);
        index.assets.push(indexed_asset);

        self.save_index(&index).await?;
        Ok(())
    }

    async fn remove_asset_from_index(&self, asset_id: &AssetId) -> Result<()> {
        let mut index = self.load_index().await?;
        index.assets.retain(|a| a.id != *asset_id);
        self.save_index(&index).await?;
        Ok(())
    }

    fn find_asset_in_index<'a>(
        &self,
        index: &'a StorageIndex,
        asset_id: &AssetId,
    ) -> Option<&'a IndexedAsset> {
        index.assets.iter().find(|a| a.id == *asset_id)
    }
}

#[async_trait]
impl BookStorage for FilesystemStorage {
    async fn store_novel(&self, novel: &Novel) -> Result<NovelId> {
        // Normalize and validate input data
        let normalized_url = self.normalize_url(&novel.url);
        if normalized_url.is_empty() {
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

        // Generate an ID based on the normalized novel URL for this backend
        let source_id = self.extract_source_id(&normalized_url);
        let id_string = format!("{}::{}", source_id, normalized_url);
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
        let source_id = self.extract_source_id(&normalized_url);
        let metadata = NovelStorageMetadata {
            source_id,
            stored_at: Utc::now(),
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
            stored_at: Utc::now(),
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
        // Normalize and validate input data
        let normalized_chapter_url = self.normalize_url(chapter_url);
        if normalized_chapter_url.is_empty() {
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

        let chapter_file = self.get_chapter_file(novel_id, volume_index, &normalized_chapter_url);

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
        let content_size = content.data.len() as u64;
        let metadata = ChapterStorageMetadata {
            volume_index,
            chapter_url: normalized_chapter_url.clone(),
            stored_at: Utc::now(),
            content_size,
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
        let normalized_chapter_url = self.normalize_url(chapter_url);
        let chapter_file = self.get_chapter_file(novel_id, volume_index, &normalized_chapter_url);

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
        let normalized_chapter_url = self.normalize_url(chapter_url);
        let chapter_file = self.get_chapter_file(novel_id, volume_index, &normalized_chapter_url);

        if !chapter_file.exists() {
            return Ok(false);
        }

        // Remove the chapter file
        fs::remove_file(&chapter_file)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to delete chapter file: {}", e)),
            })?;

        // Update stored chapter count for the novel
        self.update_index_stored_chapters(novel_id).await?;

        Ok(true)
    }

    async fn exists_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<bool> {
        let normalized_chapter_url = self.normalize_url(chapter_url);
        let chapter_file = self.get_chapter_file(novel_id, volume_index, &normalized_chapter_url);
        Ok(chapter_file.exists())
    }

    async fn list_novels(&self, filter: &NovelFilter) -> Result<Vec<NovelSummary>> {
        let index = self.load_index().await?;

        let mut summaries: Vec<NovelSummary> = index
            .novels
            .into_iter()
            .filter(|novel| {
                // Filter by source IDs if provided
                if !filter.source_ids.is_empty() {
                    let parts: Vec<&str> = novel.id.as_str().splitn(2, "::").collect();
                    let source_id = parts.first().unwrap_or(&"unknown");
                    if !filter.source_ids.contains(&source_id.to_string()) {
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

    async fn find_novel_by_url(&self, url: &str) -> Result<Option<Novel>> {
        let normalized_url = self.normalize_url(url);
        let index = self.load_index().await?;

        // Find the novel in our index that matches the normalized URL
        for indexed_novel in &index.novels {
            let parts: Vec<&str> = indexed_novel.id.as_str().splitn(2, "::").collect();
            if let Some(novel_url) = parts.get(1) {
                if *novel_url == normalized_url {
                    return self.get_novel(&indexed_novel.id).await;
                }
            }
        }

        Ok(None)
    }

    async fn list_chapters(&self, novel_id: &NovelId) -> Result<Vec<ChapterInfo>> {
        // Get the novel first to access chapter metadata
        let novel = self.get_novel(novel_id).await?;
        let novel = match novel {
            Some(n) => n,
            None => return Ok(Vec::new()),
        };

        let mut chapter_infos = Vec::new();

        // Iterate through volumes and chapters
        for volume in &novel.volumes {
            for chapter in &volume.chapters {
                let chapter_file = self.get_chapter_file(novel_id, volume.index, &chapter.url);

                let chapter_info = if chapter_file.exists() {
                    if let Ok(content) = fs::read_to_string(&chapter_file).await {
                        if let Ok(combined) = serde_json::from_str::<serde_json::Value>(&content) {
                            // Extract metadata
                            if let Some(metadata) = combined.get("metadata") {
                                if let Ok(chapter_metadata) =
                                    serde_json::from_value::<ChapterStorageMetadata>(
                                        metadata.clone(),
                                    )
                                {
                                    ChapterInfo::with_content(
                                        volume.index,
                                        chapter.url.clone(),
                                        chapter.title.clone(),
                                        chapter.index,
                                        chapter_metadata.stored_at,
                                        chapter_metadata.content_size,
                                    )
                                } else {
                                    ChapterInfo::new(
                                        volume.index,
                                        chapter.url.clone(),
                                        chapter.title.clone(),
                                        chapter.index,
                                    )
                                }
                            } else {
                                ChapterInfo::new(
                                    volume.index,
                                    chapter.url.clone(),
                                    chapter.title.clone(),
                                    chapter.index,
                                )
                            }
                        } else {
                            ChapterInfo::new(
                                volume.index,
                                chapter.url.clone(),
                                chapter.title.clone(),
                                chapter.index,
                            )
                        }
                    } else {
                        ChapterInfo::new(
                            volume.index,
                            chapter.url.clone(),
                            chapter.title.clone(),
                            chapter.index,
                        )
                    }
                } else {
                    ChapterInfo::new(
                        volume.index,
                        chapter.url.clone(),
                        chapter.title.clone(),
                        chapter.index,
                    )
                };

                chapter_infos.push(chapter_info);
            }
        }

        // Sort by volume index, then chapter index
        chapter_infos.sort_by(|a, b| {
            a.volume_index
                .cmp(&b.volume_index)
                .then(a.chapter_index.cmp(&b.chapter_index))
        });

        Ok(chapter_infos)
    }

    async fn cleanup_dangling_data(&self) -> Result<CleanupReport> {
        let mut report = CleanupReport::new();
        let index = self.load_index().await?;

        // Clean up orphaned assets (assets in index but novel doesn't exist)
        let mut orphaned_assets = Vec::new();
        for indexed_asset in &index.assets {
            if !self.exists_novel(&indexed_asset.novel_id).await? {
                orphaned_assets.push(indexed_asset.id.clone());
            }
        }

        // Remove orphaned assets
        for asset_id in orphaned_assets {
            if self.delete_asset(&asset_id).await? {
                report.orphaned_chapters_removed += 1; // Reusing field for assets too
            }
        }

        // TODO: Implement additional cleanup logic for chapters and novels
        Ok(report)
    }

    // === Asset Operations ===

    /// Create an Asset with properly generated ID and filename
    fn create_asset(&self, novel_id: NovelId, original_url: String, mime_type: String) -> Asset {
        let asset_id = self.generate_asset_id();
        let filename = self.generate_safe_filename(&original_url, &mime_type, &asset_id);

        Asset {
            id: AssetId::from(asset_id),
            novel_id,
            original_url,
            mime_type,
            size: 0, // Will be updated by storage
            filename,
        }
    }

    async fn store_asset(
        &self,
        asset: Asset,
        mut reader: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
    ) -> Result<AssetId> {
        let metadata_file = self.get_asset_metadata_file(&asset.novel_id, &asset.id);
        let data_file = self.get_asset_data_file(&asset.novel_id, &asset.filename);

        // Create directory structure for both metadata and data
        if let Some(parent) = metadata_file.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!(
                        "Failed to create asset metadata directory: {}",
                        e
                    )),
                })?;
        }
        if let Some(parent) = data_file.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to create asset data directory: {}", e)),
                })?;
        }

        // Stream binary data directly to file
        let mut data_writer =
            fs::File::create(&data_file)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to create asset data file: {}", e)),
                })?;

        tokio::io::copy(&mut reader, &mut data_writer)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to write asset data: {}", e)),
            })?;

        // Get the actual file size and update asset
        let file_metadata =
            fs::metadata(&data_file)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to get asset file metadata: {}", e)),
                })?;

        let asset_id = asset.id.clone();
        let mut updated_asset = asset;
        updated_asset.size = file_metadata.len();

        // Store metadata as JSON with correct size
        let metadata_json = serde_json::to_string_pretty(&updated_asset).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to serialize asset metadata".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        fs::write(&metadata_file, metadata_json)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to write asset metadata file: {}", e)),
            })?;

        self.add_asset_to_index(&updated_asset, file_metadata.len())
            .await?;

        Ok(asset_id)
    }

    async fn get_asset(&self, asset_id: &AssetId) -> Result<Option<Asset>> {
        let index = self.load_index().await?;

        if let Some(indexed_asset) = self.find_asset_in_index(&index, asset_id) {
            let metadata_file = self.get_asset_metadata_file(&indexed_asset.novel_id, asset_id);

            if metadata_file.exists() {
                let content = fs::read_to_string(&metadata_file).await.map_err(|e| {
                    BookStorageError::BackendError {
                        source: Some(eyre::eyre!("Failed to read asset metadata file: {}", e)),
                    }
                })?;

                let asset: Asset = serde_json::from_str(&content).map_err(|e| {
                    BookStorageError::DataConversionError {
                        message: "Failed to deserialize asset metadata".to_string(),
                        source: Some(eyre::eyre!("JSON error: {}", e)),
                    }
                })?;

                return Ok(Some(asset));
            }
        }

        Ok(None)
    }

    async fn get_asset_data(&self, asset_id: &AssetId) -> Result<Option<Vec<u8>>> {
        let index = self.load_index().await?;

        if let Some(indexed_asset) = self.find_asset_in_index(&index, asset_id) {
            let data_file =
                self.get_asset_data_file(&indexed_asset.novel_id, &indexed_asset.filename);

            if data_file.exists() {
                let data =
                    fs::read(&data_file)
                        .await
                        .map_err(|e| BookStorageError::BackendError {
                            source: Some(eyre::eyre!("Failed to read asset data file: {}", e)),
                        })?;

                return Ok(Some(data));
            }
        }

        Ok(None)
    }

    async fn delete_asset(&self, asset_id: &AssetId) -> Result<bool> {
        let index = self.load_index().await?;

        if let Some(indexed_asset) = self.find_asset_in_index(&index, asset_id) {
            let metadata_file = self.get_asset_metadata_file(&indexed_asset.novel_id, asset_id);
            let data_file =
                self.get_asset_data_file(&indexed_asset.novel_id, &indexed_asset.filename);

            // Remove both metadata and data files
            if metadata_file.exists() {
                fs::remove_file(&metadata_file).await.map_err(|e| {
                    BookStorageError::BackendError {
                        source: Some(eyre::eyre!("Failed to delete asset metadata file: {}", e)),
                    }
                })?;
            }

            if data_file.exists() {
                fs::remove_file(&data_file)
                    .await
                    .map_err(|e| BookStorageError::BackendError {
                        source: Some(eyre::eyre!("Failed to delete asset data file: {}", e)),
                    })?;
            }

            // Remove from index
            self.remove_asset_from_index(asset_id).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn find_asset_by_url(&self, url: &str) -> Result<Option<AssetId>> {
        let index = self.load_index().await?;
        let normalized_url = self.normalize_url(url);

        for indexed_asset in &index.assets {
            if indexed_asset.original_url == normalized_url {
                return Ok(Some(indexed_asset.id.clone()));
            }
        }

        Ok(None)
    }

    async fn get_novel_assets(&self, novel_id: &NovelId) -> Result<Vec<AssetSummary>> {
        let index = self.load_index().await?;

        let summaries = index
            .assets
            .iter()
            .filter(|asset| asset.novel_id == *novel_id)
            .map(|indexed_asset| AssetSummary {
                id: indexed_asset.id.clone(),
                novel_id: indexed_asset.novel_id.clone(),
                original_url: indexed_asset.original_url.clone(),
                mime_type: indexed_asset.mime_type.clone(),
                size: indexed_asset.size,
                filename: indexed_asset.filename.clone(),
            })
            .collect();

        Ok(summaries)
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
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        let stored_count = chapters.iter().filter(|c| c.has_content()).count();
        assert_eq!(stored_count, 0);

        // Store a chapter
        let content = ChapterContent {
            data: "Test chapter content".to_string(),
        };
        storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        // Check that chapter is now marked as stored
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        let stored_count = chapters.iter().filter(|c| c.has_content()).count();
        assert_eq!(stored_count, 1);

        // Delete a chapter
        let deleted = storage
            .delete_chapter_content(&novel_id, 1, "https://test.com/chapter-1")
            .await
            .unwrap();
        assert!(deleted);

        // Check that chapter is no longer marked as stored
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        let stored_count = chapters.iter().filter(|c| c.has_content()).count();
        assert_eq!(stored_count, 0);
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

        // Verify novels can be filtered by source
        let filter = crate::types::NovelFilter {
            source_ids: vec!["example.com".to_string()],
        };
        let novels = storage.list_novels(&filter).await.unwrap();
        assert_eq!(novels.len(), 2); // Both should be found under example.com
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

    #[tokio::test]
    async fn test_chapter_storage_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        // Initially no chapters should have content
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        assert_eq!(chapters.len(), 1); // Novel has 1 chapter
        assert!(!chapters[0].has_content());
        assert!(chapters[0].stored_at().is_none());
        assert!(chapters[0].content_size().is_none());

        // Store chapter content
        let content = ChapterContent {
            data: "This is test chapter content with some text.".to_string(),
        };
        let content_size = content.data.len() as u64;

        storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        // Check that metadata is now populated
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        assert_eq!(chapters.len(), 1);
        assert!(chapters[0].has_content());
        assert!(chapters[0].stored_at().is_some());
        assert_eq!(chapters[0].content_size(), Some(content_size));
        assert!(chapters[0].updated_at().is_some());

        // Verify timestamps are recent (within last minute)
        let now = Utc::now();
        let stored_at = chapters[0].stored_at().unwrap();
        let diff = now.signed_duration_since(stored_at);
        assert!(diff.num_seconds() < 60);

        // Delete chapter content
        let deleted = storage
            .delete_chapter_content(&novel_id, 1, "https://test.com/chapter-1")
            .await
            .unwrap();
        assert!(deleted);

        // Check that has_content is now false and no storage metadata
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        assert_eq!(chapters.len(), 1);
        assert!(!chapters[0].has_content());
        assert!(chapters[0].stored_at().is_none()); // No historical data when file deleted
        assert!(chapters[0].content_size().is_none()); // Size cleared
    }

    #[tokio::test]
    async fn test_chapter_content_status_pattern_matching() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        assert_eq!(chapters.len(), 1);

        // Test pattern matching on NotStored
        match &chapters[0].content_status {
            crate::types::ChapterContentStatus::NotStored => {
                println!("Chapter content not stored - as expected");
            }
            crate::types::ChapterContentStatus::Stored { .. } => {
                panic!("Expected NotStored, got Stored");
            }
        }

        // Store content
        let content = ChapterContent {
            data: "Pattern matching test content".to_string(),
        };
        storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        let chapters = storage.list_chapters(&novel_id).await.unwrap();

        // Test pattern matching on Stored with destructuring
        match &chapters[0].content_status {
            crate::types::ChapterContentStatus::NotStored => {
                panic!("Expected Stored, got NotStored");
            }
            crate::types::ChapterContentStatus::Stored {
                stored_at,
                content_size,
                updated_at,
            } => {
                assert_eq!(*content_size, 29); // Length of test content
                assert!(stored_at <= updated_at);
                let now = Utc::now();
                assert!(now.signed_duration_since(*stored_at).num_seconds() < 60);
                println!(
                    "Chapter stored at {} with {} bytes",
                    stored_at.format("%Y-%m-%d %H:%M:%S"),
                    content_size
                );
            }
        }

        // Test helper methods work consistently
        assert!(chapters[0].has_content());
        assert_eq!(chapters[0].content_size(), Some(29));
        assert!(chapters[0].stored_at().is_some());
        assert!(chapters[0].updated_at().is_some());
    }

    #[tokio::test]
    async fn test_url_normalization() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Test URL normalization
        assert_eq!(
            storage.normalize_url("https://example.com"),
            "https://example.com"
        );
        assert_eq!(
            storage.normalize_url("https://example.com/"),
            "https://example.com"
        );
        assert_eq!(
            storage.normalize_url("https://example.com/path/"),
            "https://example.com/path"
        );
        assert_eq!(
            storage.normalize_url("  https://example.com/  "),
            "https://example.com"
        );
        assert_eq!(storage.normalize_url("https://"), "https://"); // Don't remove protocol slash
        assert_eq!(storage.normalize_url(""), "");
        assert_eq!(storage.normalize_url("   "), "");

        // Test that novels with trailing slashes are treated as the same
        let mut novel1 = create_test_novel();
        novel1.url = "https://example.com/novel".to_string();

        let mut novel2 = create_test_novel();
        novel2.url = "https://example.com/novel/".to_string();

        // Store first novel
        let _id1 = storage.store_novel(&novel1).await.unwrap();

        // Try to store second novel with trailing slash - should fail as it's the same novel
        let result = storage.store_novel(&novel2).await;
        assert!(result.is_err()); // Should fail because it's already stored

        // But we should be able to find it with either URL
        let found1 = storage
            .find_novel_by_url("https://example.com/novel")
            .await
            .unwrap();
        let found2 = storage
            .find_novel_by_url("https://example.com/novel/")
            .await
            .unwrap();

        assert!(found1.is_some());
        assert!(found2.is_some());
        assert_eq!(found1.unwrap().url, found2.unwrap().url);
    }

    #[tokio::test]
    async fn test_asset_id_filename_consistency() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel_id = NovelId::from("test-novel");

        // Test different URL patterns and their filename extraction
        let test_cases = vec![
            ("https://example.com/cover.jpg", "image/jpeg"),
            ("https://cdn.site.com/assets/image.png?v=123", "image/png"),
            ("https://example.com/files/document.pdf", "application/pdf"),
            ("https://example.com/no-extension", "image/webp"),
        ];

        for (url, mime_type) in test_cases {
            // Create asset using the new method
            let asset =
                storage.create_asset(novel_id.clone(), url.to_string(), mime_type.to_string());

            // Verify asset ID is filesystem-safe and short
            assert_eq!(asset.id.as_str().len(), 8);
            assert!(asset
                .id
                .as_str()
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-'));

            // Verify filename contains the asset ID and proper extension
            assert!(asset.filename.starts_with(asset.id.as_str()));

            // Check extension handling
            if url.contains(".jpg") {
                assert!(asset.filename.ends_with(".jpg"));
            } else if url.contains(".png") {
                assert!(asset.filename.ends_with(".png"));
            } else if url.contains(".pdf") {
                assert!(asset.filename.ends_with(".pdf"));
            } else if mime_type == "image/webp" {
                assert!(asset.filename.ends_with(".webp"));
            }

            // Verify the filename is filesystem-safe
            assert!(asset
                .filename
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_'));
        }
    }

    #[tokio::test]
    async fn test_comprehensive_asset_storage() {
        use std::io::Cursor;

        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel_id = NovelId::from("test-novel");

        // Test cases with realistic asset scenarios
        let test_assets = vec![
            ("https://cdn.example.com/covers/novel123.jpg", "image/jpeg"),
            ("https://assets.site.com/images/banner.png?v=2", "image/png"),
            ("https://example.com/files/document.pdf", "application/pdf"),
            ("https://cdn.site.com/avatar", "image/webp"), // No extension in URL
            ("https://example.com/image.GIF", "image/gif"), // Mixed case extension
        ];

        for (url, mime_type) in test_assets {
            // Create asset with proper ID and filename
            let asset =
                storage.create_asset(novel_id.clone(), url.to_string(), mime_type.to_string());

            // Test data
            let test_data = format!("Test data for asset from {}", url).into_bytes();
            let reader = Box::new(Cursor::new(test_data.clone()));

            // Store the asset
            let stored_id = storage.store_asset(asset.clone(), reader).await.unwrap();
            assert_eq!(stored_id, asset.id);

            // Verify the asset is indexed correctly
            let index = storage.load_index().await.unwrap();
            let indexed_asset = storage.find_asset_in_index(&index, &asset.id).unwrap();
            assert_eq!(indexed_asset.filename, asset.filename);
            assert_eq!(indexed_asset.original_url, url);
            assert_eq!(indexed_asset.mime_type, mime_type);

            // Verify files exist with correct names
            let metadata_file = storage.get_asset_metadata_file(&novel_id, &asset.id);
            let data_file = storage.get_asset_data_file(&novel_id, &asset.filename);

            assert!(
                metadata_file.exists(),
                "Metadata file should exist: {:?}",
                metadata_file
            );
            assert!(
                data_file.exists(),
                "Data file should exist: {:?}",
                data_file
            );

            // Verify metadata file is named with asset ID
            assert!(metadata_file
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&asset.id.as_str()));

            // Verify data file has the proper filename with extension
            assert_eq!(
                data_file.file_name().unwrap().to_string_lossy(),
                asset.filename
            );

            // Verify we can retrieve the asset data
            let retrieved_data = storage.get_asset_data(&asset.id).await.unwrap();
            assert!(retrieved_data.is_some());
            assert_eq!(retrieved_data.unwrap(), test_data);

            println!(
                "✅ Asset from {} stored as {} with ID {}",
                url,
                asset.filename,
                asset.id.as_str()
            );
        }
    }

    #[tokio::test]
    async fn test_end_to_end_asset_workflow() {
        use quelle_engine::bindings::quelle::extension::novel::{
            Chapter, Novel, NovelStatus, Volume,
        };
        use std::io::Cursor;

        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Create a test novel first
        let novel = Novel {
            title: "Test Novel".to_string(),
            authors: vec!["Test Author".to_string()],
            url: "https://example.com/novel".to_string(),
            cover: Some("https://cdn.example.com/cover.jpg".to_string()),
            description: vec!["A test novel".to_string()],
            status: NovelStatus::Ongoing,
            langs: vec!["en".to_string()],
            volumes: vec![Volume {
                name: "Volume 1".to_string(),
                index: 1,
                chapters: vec![Chapter {
                    title: "Chapter 1".to_string(),
                    index: 1,
                    url: "https://example.com/chapter-1".to_string(),
                    updated_at: None,
                }],
            }],
            metadata: vec![],
        };

        // Store the novel
        let novel_id = storage.store_novel(&novel).await.unwrap();

        // Test complete asset workflow: create -> store -> retrieve -> verify
        let test_cases = vec![
            (
                "https://cdn.example.com/cover.jpg",
                "image/jpeg",
                b"fake_jpeg_data",
            ),
            (
                "https://assets.site.com/image.png",
                "image/png",
                b"fake_png_data_",
            ),
        ];

        for (url, mime_type, data) in test_cases {
            // 1. Create asset using the storage method
            let asset =
                storage.create_asset(novel_id.clone(), url.to_string(), mime_type.to_string());

            // 2. Verify asset structure is correct
            assert!(asset.id.as_str().len() == 8);
            assert!(asset.filename.starts_with(asset.id.as_str()));
            assert_eq!(asset.novel_id, novel_id);
            assert_eq!(asset.original_url, url);
            assert_eq!(asset.mime_type, mime_type);

            // 3. Store the asset
            let reader = Box::new(Cursor::new(data.to_vec()));
            let stored_id = storage.store_asset(asset.clone(), reader).await.unwrap();
            assert_eq!(stored_id, asset.id);

            // 4. Verify files exist with expected names
            let metadata_file = storage.get_asset_metadata_file(&novel_id, &asset.id);
            let data_file = storage.get_asset_data_file(&novel_id, &asset.filename);

            assert!(metadata_file.exists());
            assert!(data_file.exists());
            assert_eq!(
                metadata_file.file_name().unwrap().to_string_lossy(),
                format!("{}.json", asset.id.as_str())
            );
            assert_eq!(
                data_file.file_name().unwrap().to_string_lossy(),
                asset.filename
            );

            // 5. Retrieve and verify asset metadata
            let retrieved_asset = storage.get_asset(&asset.id).await.unwrap();
            assert!(retrieved_asset.is_some());
            let retrieved_asset = retrieved_asset.unwrap();
            assert_eq!(retrieved_asset.filename, asset.filename);

            // 6. Retrieve and verify asset data
            let retrieved_data = storage.get_asset_data(&asset.id).await.unwrap();
            assert!(retrieved_data.is_some());
            assert_eq!(retrieved_data.unwrap(), data);

            // 7. Verify asset appears in novel's asset list
            let novel_assets = storage.get_novel_assets(&novel_id).await.unwrap();
            assert!(novel_assets.iter().any(|a| a.id == asset.id));
        }
    }

    #[tokio::test]
    async fn test_asset_storage_separation() {
        use crate::types::{Asset, AssetId};
        use std::io::Cursor;

        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Create test novel first
        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        // Create test asset
        let asset = Asset {
            id: AssetId::from("test-asset".to_string()),
            novel_id: novel_id.clone(),
            original_url: "https://example.com/image.jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
            size: 0,
            filename: "test-asset.jpg".to_string(),
        };

        // Test binary data
        let test_data = b"fake image data";
        let reader = Box::new(Cursor::new(test_data.to_vec()));

        // Store asset
        let stored_id = storage.store_asset(asset.clone(), reader).await.unwrap();
        assert_eq!(stored_id, asset.id);

        // Verify metadata and data are stored separately
        let metadata_file = storage.get_asset_metadata_file(&novel_id, &asset.id);
        let data_file = storage.get_asset_data_file(&novel_id, &asset.filename);

        // Both files should exist
        assert!(metadata_file.exists());
        assert!(data_file.exists());

        // Metadata file should contain JSON (not binary data)
        let metadata_content = std::fs::read_to_string(&metadata_file).unwrap();
        assert!(metadata_content.contains("test-asset"));
        assert!(metadata_content.contains("image/jpeg"));
        // Should NOT contain binary data
        assert!(!metadata_content.contains("fake image data"));

        // Data file should contain raw binary data
        let data_content = std::fs::read(&data_file).unwrap();
        assert_eq!(data_content, test_data);

        // Verify we can retrieve metadata
        let retrieved_asset = storage.get_asset(&asset.id).await.unwrap().unwrap();
        assert_eq!(retrieved_asset.id, asset.id);
        assert_eq!(retrieved_asset.mime_type, "image/jpeg");
        assert_eq!(retrieved_asset.size, test_data.len() as u64);

        // Verify we can retrieve data
        let retrieved_data = storage.get_asset_data(&asset.id).await.unwrap().unwrap();
        assert_eq!(retrieved_data, test_data);
    }
}
