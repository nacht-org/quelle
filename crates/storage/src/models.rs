//! Storage model types and conversion utilities.
//!
//! Since WIT-generated types don't implement Serde traits, we define storage-specific
//! models that can be serialized and provide conversion utilities between WIT and storage types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{BookStorageError, Result};
use crate::{ChapterContent, Novel};
use quelle_engine::bindings::quelle::extension::novel::{
    Chapter, ChapterContent as WitChapterContent, Metadata, Namespace, Novel as WitNovel,
    NovelStatus, Volume,
};

/// Content metadata for a single chapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterContentMetadata {
    pub content_size: u64,
    pub stored_at: DateTime<Utc>,
}

/// Content index that tracks which chapters have content
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentIndex {
    /// Map from chapter URL to content metadata
    pub chapters: HashMap<String, ChapterContentMetadata>,
}

impl ContentIndex {
    pub fn mark_chapter_stored(&mut self, chapter_url: String, content_size: u64) {
        self.chapters.insert(
            chapter_url,
            ChapterContentMetadata {
                content_size,
                stored_at: Utc::now(),
            },
        );
    }

    pub fn mark_chapter_removed(&mut self, chapter_url: &str) {
        self.chapters.remove(chapter_url);
    }

    pub fn has_content(&self, chapter_url: &str) -> bool {
        self.chapters.contains_key(chapter_url)
    }

    pub fn get_content_metadata(&self, chapter_url: &str) -> Option<&ChapterContentMetadata> {
        self.chapters.get(chapter_url)
    }
}

/// Storage representation of a Novel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNovel {
    pub url: String,
    pub authors: Vec<String>,
    pub title: String,
    pub cover: Option<String>,
    pub description: Vec<String>,
    pub volumes: Vec<StorageVolume>,
    pub metadata: Vec<StorageMetadata>,
    pub status: String, // We'll store as string to avoid enum conversion issues
    pub langs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageVolume {
    pub name: String,
    pub index: i32,
    pub chapters: Vec<StorageChapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageChapter {
    pub title: String,
    pub index: i32,
    pub url: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    pub name: String,
    pub value: String,
    pub ns: String, // Store as string to avoid enum conversion
    pub others: Vec<(String, String)>,
}

/// Storage representation of ChapterContent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageChapterContent {
    pub data: String,
}

impl StorageNovel {
    /// Convert from WIT Novel type to storage type
    pub fn from_wit_novel(novel: &Novel) -> Self {
        Self {
            url: novel.url.clone(),
            authors: novel.authors.clone(),
            title: novel.title.clone(),
            cover: novel.cover.clone(),
            description: novel.description.clone(),
            volumes: novel
                .volumes
                .iter()
                .map(StorageVolume::from_wit_volume)
                .collect(),
            metadata: novel
                .metadata
                .iter()
                .map(StorageMetadata::from_wit_metadata)
                .collect(),
            status: match novel.status {
                NovelStatus::Ongoing => "Ongoing".to_string(),
                NovelStatus::Hiatus => "Hiatus".to_string(),
                NovelStatus::Completed => "Completed".to_string(),
                NovelStatus::Stub => "Stub".to_string(),
                NovelStatus::Dropped => "Dropped".to_string(),
                NovelStatus::Unknown => "Unknown".to_string(),
            },
            langs: novel.langs.clone(),
        }
    }

    /// Convert to WIT Novel type from storage type
    pub fn to_wit_novel(&self) -> Result<Novel> {
        // Convert status string back to enum
        let status = match self.status.as_str() {
            "Ongoing" => NovelStatus::Ongoing,
            "Hiatus" => NovelStatus::Hiatus,
            "Completed" => NovelStatus::Completed,
            "Stub" => NovelStatus::Stub,
            "Dropped" => NovelStatus::Dropped,
            _ => NovelStatus::Unknown,
        };

        let volumes: Result<Vec<Volume>> = self.volumes.iter().map(|v| v.to_wit_volume()).collect();

        let metadata: Result<Vec<Metadata>> =
            self.metadata.iter().map(|m| m.to_wit_metadata()).collect();

        Ok(WitNovel {
            url: self.url.clone(),
            authors: self.authors.clone(),
            title: self.title.clone(),
            cover: self.cover.clone(),
            description: self.description.clone(),
            volumes: volumes?,
            metadata: metadata?,
            status,
            langs: self.langs.clone(),
        })
    }
}

impl StorageVolume {
    fn from_wit_volume(volume: &Volume) -> Self {
        Self {
            name: volume.name.clone(),
            index: volume.index,
            chapters: volume
                .chapters
                .iter()
                .map(StorageChapter::from_wit_chapter)
                .collect(),
        }
    }

    fn to_wit_volume(&self) -> Result<Volume> {
        let chapters: Result<Vec<_>> = self.chapters.iter().map(|c| c.to_wit_chapter()).collect();

        Ok(Volume {
            name: self.name.clone(),
            index: self.index,
            chapters: chapters?,
        })
    }
}

impl StorageChapter {
    fn from_wit_chapter(chapter: &Chapter) -> Self {
        Self {
            title: chapter.title.clone(),
            index: chapter.index,
            url: chapter.url.clone(),
            updated_at: chapter.updated_at.clone(),
        }
    }

    fn to_wit_chapter(&self) -> Result<Chapter> {
        Ok(Chapter {
            title: self.title.clone(),
            index: self.index,
            url: self.url.clone(),
            updated_at: self.updated_at.clone(),
        })
    }
}

impl StorageMetadata {
    fn from_wit_metadata(metadata: &Metadata) -> Self {
        Self {
            name: metadata.name.clone(),
            value: metadata.value.clone(),
            ns: match metadata.ns {
                Namespace::Dc => "Dc".to_string(),
                Namespace::Opf => "Opf".to_string(),
            },
            others: metadata.others.clone(),
        }
    }

    fn to_wit_metadata(&self) -> Result<Metadata> {
        // Convert namespace string back to enum
        let ns = match self.ns.as_str() {
            "Dc" => Namespace::Dc,
            "Opf" => Namespace::Opf,
            _ => {
                return Err(BookStorageError::InvalidNovelData {
                    message: format!("Invalid namespace: {}", self.ns),
                    source: None,
                });
            }
        };

        Ok(Metadata {
            name: self.name.clone(),
            value: self.value.clone(),
            ns,
            others: self.others.clone(),
        })
    }
}

impl StorageChapterContent {
    /// Convert from WIT ChapterContent type to storage type
    pub fn from_wit_chapter_content(content: &ChapterContent) -> Self {
        Self {
            data: content.data.clone(),
        }
    }

    /// Convert to WIT ChapterContent type from storage type
    pub fn to_wit_chapter_content(&self) -> ChapterContent {
        WitChapterContent {
            data: self.data.clone(),
        }
    }
}

/// Helper functions for easy conversion
pub fn novel_to_json(novel: &Novel) -> Result<String> {
    let storage_novel = StorageNovel::from_wit_novel(novel);
    serde_json::to_string_pretty(&storage_novel).map_err(|e| {
        BookStorageError::DataConversionError {
            message: "Failed to serialize novel to JSON".to_string(),
            source: Some(eyre::eyre!("JSON error: {}", e)),
        }
    })
}

pub fn novel_from_json(json: &str) -> Result<Novel> {
    let storage_novel: StorageNovel =
        serde_json::from_str(json).map_err(|e| BookStorageError::DataConversionError {
            message: "Failed to deserialize novel from JSON".to_string(),
            source: Some(eyre::eyre!("JSON error: {}", e)),
        })?;

    storage_novel.to_wit_novel()
}

pub fn chapter_content_to_json(content: &ChapterContent) -> Result<String> {
    let storage_content = StorageChapterContent::from_wit_chapter_content(content);
    serde_json::to_string_pretty(&storage_content).map_err(|e| {
        BookStorageError::DataConversionError {
            message: "Failed to serialize chapter content to JSON".to_string(),
            source: Some(eyre::eyre!("JSON error: {}", e)),
        }
    })
}

pub fn chapter_content_from_json(json: &str) -> Result<ChapterContent> {
    let storage_content: StorageChapterContent =
        serde_json::from_str(json).map_err(|e| BookStorageError::DataConversionError {
            message: "Failed to deserialize chapter content from JSON".to_string(),
            source: Some(eyre::eyre!("JSON error: {}", e)),
        })?;

    Ok(storage_content.to_wit_chapter_content())
}
