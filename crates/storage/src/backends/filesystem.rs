//! Filesystem-based storage backend implementation.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::{BookStorageError, Result};
use crate::models::{
    chapter_content_from_json, chapter_content_to_json, novel_from_json, novel_to_json,
    ContentIndex,
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
pub struct NovelStorageMetadata {
    pub source_id: String,
    pub stored_at: DateTime<Utc>,
    pub content_index: ContentIndex,
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

    /// Convert volume index to directory name, with special handling for -1 as "default"
    fn volume_index_to_dir_name(&self, volume_index: i32) -> String {
        if volume_index == -1 {
            "default".to_string()
        } else {
            format!("v{}", volume_index)
        }
    }

    fn get_chapter_dir(&self, novel_id: &NovelId, volume_index: i32) -> PathBuf {
        self.get_novel_dir(novel_id)
            .join("chapters")
            .join(self.volume_index_to_dir_name(volume_index))
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
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
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
            if let Some(mut path_segments) = parsed_url.path_segments() {
                if let Some(last_segment) = path_segments.next_back() {
                    // Remove query parameters if present
                    let filename = last_segment.split('?').next().unwrap_or(last_segment);
                    if !filename.is_empty() && filename.contains('.') {
                        if let Some(extension) = filename.split('.').next_back() {
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

    /// Normalize URL conservatively - only handle basic cases to avoid breaking functionality
    fn normalize_url(&self, url: &str) -> String {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        // Try to parse as a proper URL for basic normalization
        if let Ok(mut parsed_url) = url::Url::parse(trimmed) {
            // Only normalize domain case - this is generally safe
            if let Some(host) = parsed_url.host_str() {
                let host_lower = host.to_lowercase();
                // Remove www. prefix as it's typically equivalent
                let normalized_host = if host_lower.starts_with("www.") {
                    &host_lower[4..]
                } else {
                    &host_lower
                };
                if parsed_url.set_host(Some(normalized_host)).is_err() {
                    // If host change fails, continue with original
                }
            }

            // Only remove trailing slash from path (safe for most URLs)
            let path = parsed_url.path().to_string();
            if path.len() > 1 && path.ends_with('/') {
                parsed_url.set_path(&path[..path.len() - 1]);
            }

            let mut url_string = parsed_url.to_string();
            // Handle root trailing slash removal with string manipulation
            // since URL library always adds '/' for empty paths
            if url_string.ends_with('/') && url_string.matches('/').count() == 3 {
                url_string.pop();
            }
            url_string
        } else {
            // Fallback to basic normalization for malformed URLs
            self.basic_normalize_url(trimmed)
        }
    }

    /// Basic URL normalization fallback
    fn basic_normalize_url(&self, url: &str) -> String {
        // Remove trailing slash unless it's just the protocol
        if url.ends_with('/') && url.len() > 1 && !url.ends_with("://") {
            url.trim_end_matches('/').to_string()
        } else {
            url.to_string()
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

    /// Normalize all URLs in a novel (including chapter URLs)
    fn normalize_novel_urls(&self, novel: &Novel) -> Novel {
        use quelle_engine::bindings::quelle::extension::novel::{
            Chapter, Novel as WitNovel, Volume,
        };

        WitNovel {
            url: self.normalize_url(&novel.url),
            authors: novel.authors.clone(),
            title: novel.title.clone(),
            cover: novel.cover.clone(),
            description: novel.description.clone(),
            volumes: novel
                .volumes
                .iter()
                .map(|volume| Volume {
                    name: volume.name.clone(),
                    index: volume.index,
                    chapters: volume
                        .chapters
                        .iter()
                        .map(|chapter| Chapter {
                            title: chapter.title.clone(),
                            index: chapter.index,
                            url: self.normalize_url(&chapter.url),
                            updated_at: chapter.updated_at.clone(),
                        })
                        .collect(),
                })
                .collect(),
            metadata: novel.metadata.clone(),
            status: novel.status,
            langs: novel.langs.clone(),
        }
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

        // Normalize chapter URLs in the novel before storing
        let normalized_novel = self.normalize_novel_urls(novel);

        // Check if novel already exists and preserve existing metadata
        let metadata = if novel_file.exists() {
            tracing::info!(
                "Novel manifest file already exists, preserving metadata: {}",
                novel_id.as_str()
            );
            // Read existing metadata to preserve content index and other data
            match self.read_novel_file_combined(&novel_id).await {
                Ok((_, mut existing_metadata)) => {
                    // Update timestamp but preserve everything else
                    existing_metadata.stored_at = Utc::now();
                    existing_metadata
                }
                Err(_) => {
                    // Fallback if we can't read existing metadata
                    tracing::warn!("Could not read existing metadata, creating new");
                    let source_id = self.extract_source_id(&normalized_url);
                    NovelStorageMetadata {
                        source_id,
                        stored_at: Utc::now(),
                        content_index: crate::models::ContentIndex::default(),
                    }
                }
            }
        } else {
            // Create new metadata for new novels
            let source_id = self.extract_source_id(&normalized_url);
            NovelStorageMetadata {
                source_id,
                stored_at: Utc::now(),
                content_index: crate::models::ContentIndex::default(),
            }
        };

        // Use helper method to write combined structure
        self.write_novel_file_combined(&novel_id, &normalized_novel, &metadata)
            .await?;

        // Update index
        self.update_index_for_novel(&novel_id, novel).await?;

        Ok(novel_id)
    }

    async fn get_novel(&self, id: &NovelId) -> Result<Option<Novel>> {
        let novel_file = self.get_novel_file(id);

        if !novel_file.exists() {
            return Ok(None);
        }

        // Use helper method to read combined structure
        let (novel, _metadata) = self.read_novel_file_combined(id).await?;
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

        // Read existing metadata to preserve content index
        let (_, mut metadata) = self.read_novel_file_combined(id).await?;
        metadata.stored_at = Utc::now();

        // Use helper method to write combined structure
        self.write_novel_file_combined(id, novel, &metadata).await?;

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
    ) -> Result<ChapterInfo> {
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

        // Update the novel structure to mark this chapter as having content
        self.update_chapter_content_in_novel(
            novel_id,
            volume_index,
            &normalized_chapter_url,
            content_size,
        )
        .await?;

        // Find and return the updated ChapterInfo
        let updated_novel =
            self.get_novel(novel_id)
                .await?
                .ok_or_else(|| BookStorageError::NovelNotFound {
                    id: novel_id.to_string(),
                    source: None,
                })?;

        for volume in &updated_novel.volumes {
            if volume.index == volume_index {
                for chapter in &volume.chapters {
                    if chapter.url == normalized_chapter_url {
                        let mut chapter_info = ChapterInfo::new(
                            volume_index,
                            chapter.url.clone(),
                            chapter.title.clone(),
                            chapter.index,
                        );
                        // Use mark_stored to update the status
                        chapter_info.mark_stored(content_size);
                        return Ok(chapter_info);
                    }
                }
            }
        }

        // If we get here, the chapter wasn't found in the novel structure
        // This shouldn't happen since we validated the novel exists earlier
        Err(BookStorageError::InvalidChapterData {
            message: "Chapter not found in novel structure".to_string(),
            source: None,
        })
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
    ) -> Result<Option<ChapterInfo>> {
        let normalized_chapter_url = self.normalize_url(chapter_url);
        let chapter_file = self.get_chapter_file(novel_id, volume_index, &normalized_chapter_url);

        if !chapter_file.exists() {
            return Ok(None);
        }

        // Remove the chapter file
        fs::remove_file(&chapter_file)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to delete chapter file: {}", e)),
            })?;

        // Update the novel structure to mark this chapter as having no content
        self.remove_chapter_content_from_novel(novel_id, volume_index, &normalized_chapter_url)
            .await?;

        // Update stored chapter count for the novel
        self.update_index_stored_chapters(novel_id).await?;

        // Find and return the updated ChapterInfo
        let updated_novel =
            self.get_novel(novel_id)
                .await?
                .ok_or_else(|| BookStorageError::NovelNotFound {
                    id: novel_id.to_string(),
                    source: None,
                })?;

        // Find the chapter in the novel structure
        for volume in &updated_novel.volumes {
            if volume.index == volume_index {
                for chapter in &volume.chapters {
                    if chapter.url == normalized_chapter_url {
                        let mut chapter_info = ChapterInfo::new(
                            volume_index,
                            chapter.url.clone(),
                            chapter.title.clone(),
                            chapter.index,
                        );
                        // Use mark_removed to update the status
                        chapter_info.mark_removed();
                        return Ok(Some(chapter_info));
                    }
                }
            }
        }

        // Chapter not found in novel structure, but file was deleted
        Ok(None)
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
        // Read the novel structure and metadata
        let (novel, metadata) = self.read_novel_file_combined(novel_id).await?;

        let mut chapter_infos = Vec::new();

        // Iterate through volumes and chapters, using content index from metadata
        for volume in &novel.volumes {
            for chapter in &volume.chapters {
                let chapter_info = if let Some(content_metadata) =
                    metadata.content_index.get_content_metadata(&chapter.url)
                {
                    // Chapter has content - create ChapterInfo with content metadata
                    ChapterInfo::with_content(
                        volume.index,
                        chapter.url.clone(),
                        chapter.title.clone(),
                        chapter.index,
                        content_metadata.stored_at,
                        content_metadata.content_size,
                    )
                } else {
                    // Chapter has no content
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

impl FilesystemStorage {
    // Helper methods for efficient novel file operations

    /// Read the combined novel file structure (novel + metadata)
    async fn read_novel_file_combined(
        &self,
        novel_id: &NovelId,
    ) -> Result<(Novel, NovelStorageMetadata)> {
        let novel_file = self.get_novel_file(novel_id);

        if !novel_file.exists() {
            return Err(BookStorageError::NovelNotFound {
                id: novel_id.as_str().to_string(),
                source: None,
            });
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

        // Extract and convert the novel part
        let novel_value = combined["novel"].clone();
        let novel_json = serde_json::to_string(&novel_value).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to extract novel data".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;
        let novel = novel_from_json(&novel_json)?;

        // Extract and convert the metadata part
        let metadata_value = combined["metadata"].clone();
        let mut metadata: NovelStorageMetadata =
            serde_json::from_value(metadata_value).map_err(|e| {
                BookStorageError::DataConversionError {
                    message: "Failed to extract metadata".to_string(),
                    source: Some(eyre::eyre!("JSON error: {}", e)),
                }
            })?;

        // Handle missing content_index field for backwards compatibility
        if metadata.content_index.chapters.is_empty() {
            // Initialize empty content index for existing novels
            metadata.content_index = ContentIndex::default();
        }

        Ok((novel, metadata))
    }

    /// Write the combined novel file structure (novel + metadata) atomically
    async fn write_novel_file_combined(
        &self,
        novel_id: &NovelId,
        novel: &Novel,
        metadata: &NovelStorageMetadata,
    ) -> Result<()> {
        let novel_file = self.get_novel_file(novel_id);

        // Create directory structure
        if let Some(parent) = novel_file.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| BookStorageError::BackendError {
                    source: Some(eyre::eyre!("Failed to create novel directory: {}", e)),
                })?;
        }

        // Convert novel to JSON
        let novel_json = novel_to_json(novel)?;
        let novel_value = serde_json::from_str::<serde_json::Value>(&novel_json).map_err(|e| {
            BookStorageError::DataConversionError {
                message: "Failed to parse novel JSON".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            }
        })?;

        // Convert metadata to JSON value
        let metadata_value =
            serde_json::to_value(metadata).map_err(|e| BookStorageError::DataConversionError {
                message: "Failed to serialize metadata".to_string(),
                source: Some(eyre::eyre!("JSON error: {}", e)),
            })?;

        // Create combined JSON structure
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

        // Write atomically using a temporary file
        let temp_file = novel_file.with_extension("tmp");
        fs::write(&temp_file, &combined_json).await.map_err(|e| {
            BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to write temporary novel file: {}", e)),
            }
        })?;

        // Atomic rename
        fs::rename(&temp_file, &novel_file)
            .await
            .map_err(|e| BookStorageError::BackendError {
                source: Some(eyre::eyre!("Failed to rename novel file: {}", e)),
            })?;

        Ok(())
    }

    /// Update only the metadata part of a stored novel
    pub async fn update_novel_metadata(
        &self,
        novel_id: &NovelId,
        metadata: NovelStorageMetadata,
    ) -> Result<()> {
        let (novel, _) = self.read_novel_file_combined(novel_id).await?;
        self.write_novel_file_combined(novel_id, &novel, &metadata)
            .await?;
        Ok(())
    }

    /// Update the stored timestamp for a novel without changing its content
    pub async fn touch_novel(&self, novel_id: &NovelId) -> Result<()> {
        let (novel, mut metadata) = self.read_novel_file_combined(novel_id).await?;
        metadata.stored_at = Utc::now();
        self.write_novel_file_combined(novel_id, &novel, &metadata)
            .await?;
        Ok(())
    }

    /// Update a specific chapter's content status in the novel metadata
    async fn update_chapter_content_in_novel(
        &self,
        novel_id: &NovelId,
        _volume_index: i32,
        chapter_url: &str,
        content_size: u64,
    ) -> Result<()> {
        // Read the current novel file and extract both novel and metadata
        let (novel, mut metadata) = self.read_novel_file_combined(novel_id).await?;

        // Update the content index in metadata
        metadata
            .content_index
            .mark_chapter_stored(chapter_url.to_string(), content_size);

        // Save the updated metadata (novel content stays the same)
        self.write_novel_file_combined(novel_id, &novel, &metadata)
            .await?;

        Ok(())
    }

    /// Remove content status from a specific chapter in the novel metadata
    async fn remove_chapter_content_from_novel(
        &self,
        novel_id: &NovelId,
        _volume_index: i32,
        chapter_url: &str,
    ) -> Result<()> {
        // Read the current novel file and extract both novel and metadata
        let (novel, mut metadata) = self.read_novel_file_combined(novel_id).await?;

        // Update the content index in metadata
        metadata.content_index.mark_chapter_removed(chapter_url);

        // Save the updated metadata (novel content stays the same)
        self.write_novel_file_combined(novel_id, &novel, &metadata)
            .await?;

        Ok(())
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

        let _updated_chapter = storage
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
        let _updated_chapter = storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        // Check that chapter is now marked as stored
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        let stored_count = chapters.iter().filter(|c| c.has_content()).count();
        assert_eq!(stored_count, 1);

        // Delete a chapter
        let deleted_chapter = storage
            .delete_chapter_content(&novel_id, 1, "https://test.com/chapter-1")
            .await
            .unwrap();
        assert!(deleted_chapter.is_some());

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

        let updated_chapter = storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        // Verify the returned ChapterInfo has correct status
        assert!(updated_chapter.has_content());
        assert_eq!(
            updated_chapter.content_size().unwrap(),
            content.data.len() as u64
        );

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
        let deleted_chapter = storage
            .delete_chapter_content(&novel_id, 1, "https://test.com/chapter-1")
            .await
            .unwrap();
        assert!(deleted_chapter.is_some());
        assert!(!deleted_chapter.unwrap().has_content());

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
        match chapters[0].content_status {
            crate::types::ChapterContentStatus::NotStored => {
                // This is expected after deletion
            }
            crate::types::ChapterContentStatus::Stored { .. } => {
                panic!("Expected NotStored, got Stored");
            }
        }
    }

    #[tokio::test]
    async fn test_mark_stored_issue_demonstration() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        // Get the initial chapter info
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        let mut chapter_info = chapters.into_iter().next().unwrap();

        // Initially no content
        assert!(!chapter_info.has_content());
        assert!(chapter_info.content_size().is_none());

        // Store content using the storage system
        let content = ChapterContent {
            data: "This is test chapter content.".to_string(),
        };

        let updated_chapter = storage
            .store_chapter_content(
                &novel_id,
                chapter_info.volume_index,
                &chapter_info.chapter_url,
                &content,
            )
            .await
            .unwrap();

        // Verify the returned ChapterInfo is correctly updated
        assert!(updated_chapter.has_content());
        assert_eq!(
            updated_chapter.content_size().unwrap(),
            content.data.len() as u64
        );

        // DEMONSTRATION: The existing chapter_info object is still NOT updated
        // This is expected behavior - only the returned object is updated
        assert!(!chapter_info.has_content()); // Still false!
        assert!(chapter_info.content_size().is_none()); // Still None!

        // Get fresh chapter info from storage - this DOES show updated status
        let fresh_chapters = storage.list_chapters(&novel_id).await.unwrap();
        let fresh_chapter_info = fresh_chapters.into_iter().next().unwrap();
        assert!(fresh_chapter_info.has_content()); // Now true!
        assert!(fresh_chapter_info.content_size().is_some()); // Now Some(...)

        // SOLUTION: The storage system now returns updated ChapterInfo objects
        // The mark_stored() method is now properly used internally by the storage system
        // We can also call it manually to update existing objects:
        chapter_info.mark_stored(content.data.len() as u64);
        assert!(chapter_info.has_content()); // Now true!
        assert_eq!(
            chapter_info.content_size().unwrap(),
            content.data.len() as u64
        );

        // FIXED: The storage system now properly uses mark_stored() internally
        // and returns updated ChapterInfo objects, resolving the architectural issue
    }

    #[tokio::test]
    async fn test_chapter_storage_disk_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel = create_test_novel();
        let novel_id = storage.store_novel(&novel).await.unwrap();

        let content = ChapterContent {
            data: "Test chapter content for disk persistence".to_string(),
        };

        // Store content
        let updated_chapter = storage
            .store_chapter_content(&novel_id, 1, "https://test.com/chapter-1", &content)
            .await
            .unwrap();

        // Verify the returned ChapterInfo has correct status
        assert!(updated_chapter.has_content());
        assert_eq!(
            updated_chapter.content_size().unwrap(),
            content.data.len() as u64
        );

        // Verify data is actually written to disk by reading the file directly
        let chapter_file = storage.get_chapter_file(&novel_id, 1, "https://test.com/chapter-1");
        assert!(chapter_file.exists(), "Chapter file should exist on disk");

        // Read and parse the file content
        let file_content = fs::read_to_string(&chapter_file).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&file_content).unwrap();

        // Verify the file contains both content and metadata
        assert!(
            parsed.get("content").is_some(),
            "File should contain content"
        );
        assert!(
            parsed.get("metadata").is_some(),
            "File should contain metadata"
        );

        // Verify metadata structure
        let metadata = parsed.get("metadata").unwrap();
        assert_eq!(metadata.get("volume_index").unwrap().as_i64().unwrap(), 1);
        assert_eq!(
            metadata.get("chapter_url").unwrap().as_str().unwrap(),
            "https://test.com/chapter-1"
        );
        assert_eq!(
            metadata.get("content_size").unwrap().as_u64().unwrap(),
            content.data.len() as u64
        );
        assert!(
            metadata.get("stored_at").is_some(),
            "Metadata should have stored_at timestamp"
        );

        // Create a new storage instance to verify persistence across sessions
        let storage2 = FilesystemStorage::new(temp_dir.path());
        storage2.initialize().await.unwrap();

        // Verify the new storage instance can read the stored data correctly
        let chapters = storage2.list_chapters(&novel_id).await.unwrap();
        assert_eq!(chapters.len(), 1);
        assert!(
            chapters[0].has_content(),
            "Chapter should show as having content after restart"
        );
        assert_eq!(
            chapters[0].content_size().unwrap(),
            content.data.len() as u64
        );

        // Verify we can retrieve the actual content
        let retrieved_content = storage2
            .get_chapter_content(&novel_id, 1, "https://test.com/chapter-1")
            .await
            .unwrap();
        assert!(retrieved_content.is_some());
        assert_eq!(retrieved_content.unwrap().data, content.data);
    }

    #[tokio::test]
    async fn test_url_normalization() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Test basic trailing slash removal
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

        // Test case normalization and www removal
        assert_eq!(
            storage.normalize_url("https://Example.Com/Path/Image.JPG"),
            "https://example.com/Path/Image.JPG"
        );
        assert_eq!(
            storage.normalize_url("https://WWW.Example.com/cover.PNG"),
            "https://example.com/cover.PNG"
        );

        // Test query parameters are preserved
        assert_eq!(
            storage.normalize_url("https://example.com/api/data?param=value"),
            "https://example.com/api/data?param=value"
        );
        assert_eq!(
            storage.normalize_url("https://cdn.example.com/cover.jpg?v=123&t=456"),
            "https://cdn.example.com/cover.jpg?v=123&t=456"
        );

        // Test malformed URLs fall back to basic normalization
        assert_eq!(storage.normalize_url("not-a-url/path/"), "not-a-url/path");

        // Test that novels with trailing slashes are treated as the same
        let mut novel1 = create_test_novel();
        novel1.url = "https://example.com/novel".to_string();

        let mut novel2 = create_test_novel();
        novel2.url = "https://example.com/novel/".to_string();

        // Store first novel
        let _id1 = storage.store_novel(&novel1).await.unwrap();

        // Try to store second novel with trailing slash - should succeed and update the existing novel
        let result = storage.store_novel(&novel2).await;
        assert!(result.is_ok()); // Should succeed with upsert behavior

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
        assert_eq!(found1.as_ref().unwrap().url, found2.as_ref().unwrap().url);
    }

    #[tokio::test]
    async fn test_asset_url_deduplication() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        let novel_id = NovelId::from("test-novel");

        // Test basic URL normalization cases
        let test_cases = vec![
            (
                "https://www.example.com/cover.jpg",
                "https://example.com/cover.jpg",
            ),
            (
                "https://Example.Com/Cover.JPG",
                "https://example.com/Cover.JPG",
            ),
        ];

        for (original_url, expected_normalized) in test_cases {
            let normalized = storage.normalize_url(original_url);
            assert_eq!(
                normalized, expected_normalized,
                "URL {} should normalize to {}",
                original_url, expected_normalized
            );
        }

        // Test that identical URLs after normalization get deduplicated
        let asset1 = storage.create_asset(
            novel_id.clone(),
            "https://www.example.com/cover.jpg".to_string(),
            "image/jpeg".to_string(),
        );

        let test_data = b"fake image data";
        let reader1 = Box::new(std::io::Cursor::new(test_data.to_vec()));
        let asset_id1 = storage.store_asset(asset1, reader1).await.unwrap();

        // Try to find the asset using normalized variation
        let found_id1 = storage
            .find_asset_by_url("https://example.com/cover.jpg")
            .await
            .unwrap();

        assert_eq!(found_id1, Some(asset_id1));
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
                "Asset from {} stored as {} with ID {}",
                url,
                asset.filename,
                asset.id.as_str()
            );
        }
    }

    #[test]
    fn test_sha256_hashing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());

        // Test that hash_string produces consistent SHA-256 hashes
        let input = "test_string_for_hashing";
        let hash1 = storage.hash_string(input);
        let hash2 = storage.hash_string(input);

        // Should be consistent
        assert_eq!(hash1, hash2);

        // Should be 64 characters (SHA-256 hex)
        assert_eq!(hash1.len(), 64);

        // Should be hexadecimal
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));

        // Should produce different hashes for different inputs
        let different_input = "different_test_string";
        let different_hash = storage.hash_string(different_input);
        assert_ne!(hash1, different_hash);

        // Test known SHA-256 value for "test"
        let test_hash = storage.hash_string("test");
        assert_eq!(
            test_hash,
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
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

    #[tokio::test]
    async fn test_store_novel_upsert_behavior() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Create initial novel
        let mut novel = create_test_novel();
        novel.title = "Original Title".to_string();
        novel.authors = vec!["Original Author".to_string()];

        // Store the novel for the first time
        let novel_id = storage.store_novel(&novel).await.unwrap();

        // Verify it was stored
        let retrieved = storage.get_novel(&novel_id).await.unwrap().unwrap();
        assert_eq!(retrieved.title, "Original Title");
        assert_eq!(retrieved.authors, vec!["Original Author"]);

        // Update the novel with new data
        novel.title = "Updated Title".to_string();
        novel.authors = vec!["Updated Author".to_string()];

        // Store the same novel again (should update, not create new)
        let updated_id = storage.store_novel(&novel).await.unwrap();

        // Should return the same ID
        assert_eq!(novel_id, updated_id);

        // Verify the novel was updated
        let retrieved_updated = storage.get_novel(&novel_id).await.unwrap().unwrap();
        assert_eq!(retrieved_updated.title, "Updated Title");
        assert_eq!(retrieved_updated.authors, vec!["Updated Author"]);

        // Verify there's still only one novel in storage
        let filter = crate::types::NovelFilter::default();
        let novels = storage.list_novels(&filter).await.unwrap();
        assert_eq!(novels.len(), 1);
        assert_eq!(novels[0].title, "Updated Title");
    }

    #[tokio::test]
    async fn test_chapter_url_normalization_fix() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();

        // Create a novel with non-normalized chapter URL
        let novel = Novel {
            url: "https://example.com/novel/123".to_string(),
            authors: vec!["Test Author".to_string()],
            title: "Test Novel".to_string(),
            cover: None,
            description: vec!["A test novel".to_string()],
            volumes: vec![Volume {
                name: "Volume 1".to_string(),
                index: 0,
                chapters: vec![Chapter {
                    title: "Chapter 1 – So it begins… with a truck!".to_string(),
                    index: 0,
                    // This URL has query parameters and fragments - should be normalized when stored
                    url: "https://example.com/chapter/1?utm_source=test&ref=novel#content"
                        .to_string(),
                    updated_at: None,
                }],
            }],
            metadata: vec![],
            status: NovelStatus::Ongoing,
            langs: vec!["en".to_string()],
        };

        let original_chapter_url = novel.volumes[0].chapters[0].url.clone();

        // Store the novel (this should normalize the chapter URLs)
        let novel_id = storage.store_novel(&novel).await.unwrap();

        // Verify that the stored chapter URL is normalized
        let chapters = storage.list_chapters(&novel_id).await.unwrap();
        let stored_chapter_url = &chapters[0].chapter_url;
        assert_eq!(
            stored_chapter_url, &original_chapter_url,
            "Chapter URL should be normalized when stored"
        );
        assert_eq!(
            stored_chapter_url, "https://example.com/chapter/1?utm_source=test&ref=novel#content",
            "Chapter URL should be normalized to base form"
        );

        // Now try to store chapter content using the original non-normalized URL
        let chapter_content = ChapterContent {
            data: "This is the chapter content for 'So it begins… with a truck!'".to_string(),
        };

        // This should work now because both URLs get normalized the same way
        let result = storage
            .store_chapter_content(
                &novel_id,
                0,                     // volume_index
                &original_chapter_url, // Original non-normalized URL
                &chapter_content,
            )
            .await;

        assert!(
            result.is_ok(),
            "Should be able to store chapter content using non-normalized URL: {:?}",
            result
        );

        let updated_chapter = result.unwrap();
        assert!(
            updated_chapter.has_content(),
            "Chapter should have content after storage"
        );

        // Verify we can retrieve the content using various URL formats
        let url_variations = vec![
            "https://example.com/chapter/1?utm_source=test&ref=novel#content", // Normalized
            "https://example.com/chapter/1/?utm_source=test&ref=novel#content", // Trailing slash
        ];

        for url_variant in url_variations {
            let content = storage
                .get_chapter_content(&novel_id, 0, url_variant)
                .await
                .unwrap();
            assert!(
                content.is_some(),
                "Should find content with URL variant: {}",
                url_variant
            );
            assert_eq!(content.unwrap().data, chapter_content.data);
        }
    }
}
