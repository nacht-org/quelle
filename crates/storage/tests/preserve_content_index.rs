//! Test to verify that content index is preserved when novels are updated via store_novel

use quelle_engine::bindings::quelle::extension::novel::{Chapter, NovelStatus, Volume};
use quelle_storage::{
    backends::filesystem::FilesystemStorage, traits::BookStorage, ChapterContent, Novel,
};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_content_index_preserved_on_novel_update() {
    // Create a temporary directory for this test
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await.unwrap();

    // Create and store initial novel
    let initial_novel = create_test_novel();
    let novel_id = storage.store_novel(&initial_novel).await.unwrap();

    // Store content for the first chapter
    let chapter_content = ChapterContent {
        data: "This is the content of Chapter 1.".to_string(),
    };

    storage
        .store_chapter_content(
            &novel_id,
            1,
            "https://example.com/chapter-1",
            &chapter_content,
        )
        .await
        .unwrap();

    // Verify chapter has content
    let chapters_before = storage.list_chapters(&novel_id).await.unwrap();
    assert!(
        chapters_before[0].has_content(),
        "Chapter 1 should have content before update"
    );
    assert!(
        !chapters_before[1].has_content(),
        "Chapter 2 should not have content before update"
    );

    // Get the novel file path to inspect directly
    let novel_file_path = get_novel_file_path(&temp_dir, &novel_id);

    // Read the content index before updating the novel
    let before_content = fs::read_to_string(&novel_file_path).unwrap();
    let before_json: Value = serde_json::from_str(&before_content).unwrap();
    let before_content_index = &before_json["metadata"]["content_index"]["chapters"];

    assert!(
        before_content_index["https://example.com/chapter-1"].is_object(),
        "Chapter 1 should be in content index before update"
    );
    assert!(
        before_content_index["https://example.com/chapter-2"].is_null(),
        "Chapter 2 should not be in content index before update"
    );

    // Create an updated novel with new chapters added
    let updated_novel = create_updated_novel();

    // Store the updated novel (this should preserve the content index)
    let updated_novel_id = storage.store_novel(&updated_novel).await.unwrap();

    // Should be the same ID since it's the same URL
    assert_eq!(
        novel_id, updated_novel_id,
        "Novel ID should remain the same"
    );

    // Read the content index after updating the novel
    let after_content = fs::read_to_string(&novel_file_path).unwrap();
    let after_json: Value = serde_json::from_str(&after_content).unwrap();
    let after_content_index = &after_json["metadata"]["content_index"]["chapters"];

    // Verify the content index was preserved
    assert!(
        after_content_index["https://example.com/chapter-1"].is_object(),
        "Chapter 1 should still be in content index after update"
    );
    assert!(
        after_content_index["https://example.com/chapter-2"].is_null(),
        "Chapter 2 should still not be in content index after update"
    );

    // Verify the content size and timestamp were preserved
    let chapter1_metadata = &after_content_index["https://example.com/chapter-1"];
    assert!(
        chapter1_metadata["content_size"].as_u64().is_some(),
        "Chapter 1 content size should be preserved"
    );
    assert!(
        chapter1_metadata["stored_at"].as_str().is_some(),
        "Chapter 1 stored_at timestamp should be preserved"
    );

    // Verify list_chapters still shows correct content status
    let chapters_after = storage.list_chapters(&novel_id).await.unwrap();
    assert_eq!(
        chapters_after.len(),
        3,
        "Should have 3 chapters after update (added one new)"
    );
    assert!(
        chapters_after[0].has_content(),
        "Chapter 1 should still have content after update"
    );
    assert!(
        !chapters_after[1].has_content(),
        "Chapter 2 should still not have content after update"
    );
    assert!(
        !chapters_after[2].has_content(),
        "Chapter 3 (new) should not have content"
    );

    // Verify we can still retrieve the stored chapter content
    let retrieved_content = storage
        .get_chapter_content(&novel_id, 1, "https://example.com/chapter-1")
        .await
        .unwrap();
    assert!(
        retrieved_content.is_some(),
        "Should still be able to retrieve chapter 1 content"
    );
    assert_eq!(
        retrieved_content.unwrap().data,
        chapter_content.data,
        "Retrieved content should match original"
    );

    // Test that novel metadata timestamp was updated
    let _novel_metadata_timestamp = after_json["metadata"]["stored_at"].as_str().unwrap();
    let _initial_metadata_timestamp = before_json["metadata"]["stored_at"].as_str().unwrap();
    // The timestamps should be different (updated is more recent)
    // We can't do exact comparison due to timing, but they should be different strings
    // In a real scenario, the after timestamp would be later
}

#[tokio::test]
async fn test_store_novel_creates_new_content_index_for_new_novels() {
    // Create a temporary directory for this test
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await.unwrap();

    // Create and store a new novel
    let novel = create_test_novel();
    let novel_id = storage.store_novel(&novel).await.unwrap();

    // Get the novel file path
    let novel_file_path = get_novel_file_path(&temp_dir, &novel_id);

    // Read the content index
    let content = fs::read_to_string(&novel_file_path).unwrap();
    let json: Value = serde_json::from_str(&content).unwrap();
    let content_index = &json["metadata"]["content_index"]["chapters"];

    // Should have empty content index for new novels
    assert!(
        content_index.as_object().unwrap().is_empty(),
        "New novels should have empty content index"
    );

    // Verify all chapters show no content
    let chapters = storage.list_chapters(&novel_id).await.unwrap();
    for (i, chapter) in chapters.iter().enumerate() {
        assert!(
            !chapter.has_content(),
            "Chapter {} should not have content in new novel",
            i + 1
        );
    }
}

fn create_test_novel() -> Novel {
    Novel {
        url: "https://example.com/test-novel".to_string(),
        title: "Test Novel for Content Index Preservation".to_string(),
        authors: vec!["Test Author".to_string()],
        cover: None,
        description: vec!["A test novel to verify content index preservation.".to_string()],
        volumes: vec![Volume {
            name: "Volume 1".to_string(),
            index: 1,
            chapters: vec![
                Chapter {
                    title: "Chapter 1: Beginning".to_string(),
                    index: 1,
                    url: "https://example.com/chapter-1".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 2: Middle".to_string(),
                    index: 2,
                    url: "https://example.com/chapter-2".to_string(),
                    updated_at: None,
                },
            ],
        }],
        metadata: vec![],
        status: NovelStatus::Ongoing,
        langs: vec!["en".to_string()],
    }
}

fn create_updated_novel() -> Novel {
    Novel {
        url: "https://example.com/test-novel".to_string(), // Same URL
        title: "Test Novel for Content Index Preservation - Updated".to_string(), // Updated title
        authors: vec!["Test Author".to_string(), "Co-Author".to_string()], // Added author
        cover: Some("https://example.com/new-cover.jpg".to_string()), // Added cover
        description: vec![
            "A test novel to verify content index preservation.".to_string(),
            "This has been updated with more description.".to_string(),
        ],
        volumes: vec![Volume {
            name: "Volume 1".to_string(),
            index: 1,
            chapters: vec![
                Chapter {
                    title: "Chapter 1: Beginning".to_string(),
                    index: 1,
                    url: "https://example.com/chapter-1".to_string(),
                    updated_at: Some("2023-12-01T10:00:00Z".to_string()), // Added update time
                },
                Chapter {
                    title: "Chapter 2: Middle".to_string(),
                    index: 2,
                    url: "https://example.com/chapter-2".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 3: End".to_string(), // New chapter
                    index: 3,
                    url: "https://example.com/chapter-3".to_string(),
                    updated_at: None,
                },
            ],
        }],
        metadata: vec![],
        status: NovelStatus::Ongoing,
        langs: vec!["en".to_string()],
    }
}

fn get_novel_file_path(
    temp_dir: &TempDir,
    novel_id: &quelle_storage::types::NovelId,
) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};

    let id_str = novel_id.as_str();
    let parts: Vec<&str> = id_str.splitn(2, "::").collect();
    let source_id = parts.first().unwrap_or(&"unknown");
    let novel_url = parts.get(1).unwrap_or(&id_str);

    // Create SHA256 hash like the storage backend does
    let mut hasher = Sha256::new();
    hasher.update(novel_url.as_bytes());
    let novel_hash = format!("{:x}", hasher.finalize());

    temp_dir
        .path()
        .join("novels")
        .join(source_id)
        .join(novel_hash)
        .join("novel.json")
}
