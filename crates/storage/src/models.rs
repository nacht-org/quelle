//! Storage model types and conversion utilities.
//!
//! Since the domain types now live in `quelle_types`, this module provides
//! storage-specific models and serialization helpers. The `quelle_types`
//! structs are serialized directly — no intermediate `Storage*` wrappers needed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{BookStorageError, Result};
use crate::{ChapterContent, Novel};

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

/// Serialize a [`Novel`] to a pretty-printed JSON string.
pub fn novel_to_json(novel: &Novel) -> Result<String> {
    serde_json::to_string_pretty(novel).map_err(|e| BookStorageError::DataConversionError {
        message: "Failed to serialize novel to JSON".to_string(),
        source: Some(eyre::eyre!("JSON error: {}", e)),
    })
}

/// Deserialize a [`Novel`] from a JSON string.
pub fn novel_from_json(json: &str) -> Result<Novel> {
    serde_json::from_str::<Novel>(json).map_err(|e| BookStorageError::DataConversionError {
        message: "Failed to deserialize novel from JSON".to_string(),
        source: Some(eyre::eyre!("JSON error: {}", e)),
    })
}

/// Serialize a [`ChapterContent`] to a pretty-printed JSON string.
pub fn chapter_content_to_json(content: &ChapterContent) -> Result<String> {
    serde_json::to_string_pretty(content).map_err(|e| BookStorageError::DataConversionError {
        message: "Failed to serialize chapter content to JSON".to_string(),
        source: Some(eyre::eyre!("JSON error: {}", e)),
    })
}

/// Deserialize a [`ChapterContent`] from a JSON string.
pub fn chapter_content_from_json(json: &str) -> Result<ChapterContent> {
    serde_json::from_str::<ChapterContent>(json).map_err(|e| {
        BookStorageError::DataConversionError {
            message: "Failed to deserialize chapter content from JSON".to_string(),
            source: Some(eyre::eyre!("JSON error: {}", e)),
        }
    })
}
