//! Test to simulate CLI add behavior and ensure it doesn't reset content index

use quelle_engine::bindings::quelle::extension::novel::{Chapter, NovelStatus, Volume};
use quelle_storage::{
    backends::filesystem::FilesystemStorage, traits::BookStorage, ChapterContent, Novel,
};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_cli_add_preserves_existing_content_on_novel_update() {
    // Simulate the full CLI add workflow:
    // 1. User adds a novel for the first time
    // 2. User downloads some chapter content
    // 3. User runs `quelle add` again (maybe novel has new chapters)
    // 4. Existing content should be preserved

    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await.unwrap();

    let initial_novel = create_initial_novel();
    let novel_id = storage.store_novel(&initial_novel).await.unwrap();

    let chapter1_content = ChapterContent {
        data: "This is Chapter 1 content that the user downloaded.".to_string(),
    };
    let chapter2_content = ChapterContent {
        data: "This is Chapter 2 content that the user downloaded.".to_string(),
    };

    // Store content for chapters 1 and 2
    storage
        .store_chapter_content(
            &novel_id,
            1,
            "https://example.com/novel/chapter-1",
            &chapter1_content,
        )
        .await
        .unwrap();

    storage
        .store_chapter_content(
            &novel_id,
            1,
            "https://example.com/novel/chapter-2",
            &chapter2_content,
        )
        .await
        .unwrap();

    // Verify content is stored
    let chapters_after_download = storage.list_chapters(&novel_id).await.unwrap();
    assert_eq!(chapters_after_download.len(), 3, "Should have 3 chapters");
    assert!(
        chapters_after_download[0].has_content(),
        "Chapter 1 should have content"
    );
    assert!(
        chapters_after_download[1].has_content(),
        "Chapter 2 should have content"
    );
    assert!(
        !chapters_after_download[2].has_content(),
        "Chapter 3 should not have content"
    );

    let updated_novel = create_updated_novel_with_new_chapters();

    // This simulates what happens when user runs `quelle add <url>` again
    let updated_novel_id = storage.store_novel(&updated_novel).await.unwrap();

    // Should be the same ID
    assert_eq!(
        novel_id, updated_novel_id,
        "Novel ID should remain the same"
    );

    let chapters_after_update = storage.list_chapters(&novel_id).await.unwrap();
    assert_eq!(
        chapters_after_update.len(),
        5,
        "Should now have 5 chapters (2 new ones added)"
    );

    // Original chapters should still have content
    assert!(
        chapters_after_update[0].has_content(),
        "Chapter 1 should still have content after update"
    );
    assert!(
        chapters_after_update[1].has_content(),
        "Chapter 2 should still have content after update"
    );

    // Original chapter 3 and new chapters should not have content
    assert!(
        !chapters_after_update[2].has_content(),
        "Chapter 3 should still not have content after update"
    );
    assert!(
        !chapters_after_update[3].has_content(),
        "Chapter 4 (new) should not have content"
    );
    assert!(
        !chapters_after_update[4].has_content(),
        "Chapter 5 (new) should not have content"
    );

    // Verify we can still retrieve the original content
    let retrieved_ch1 = storage
        .get_chapter_content(&novel_id, 1, "https://example.com/novel/chapter-1")
        .await
        .unwrap();
    let retrieved_ch2 = storage
        .get_chapter_content(&novel_id, 1, "https://example.com/novel/chapter-2")
        .await
        .unwrap();

    assert!(
        retrieved_ch1.is_some(),
        "Chapter 1 content should still exist"
    );
    assert!(
        retrieved_ch2.is_some(),
        "Chapter 2 content should still exist"
    );
    assert_eq!(
        retrieved_ch1.unwrap().data,
        chapter1_content.data,
        "Chapter 1 content should be unchanged"
    );
    assert_eq!(
        retrieved_ch2.unwrap().data,
        chapter2_content.data,
        "Chapter 2 content should be unchanged"
    );

    let novel_file_path = get_novel_file_path(&temp_dir, &novel_id);
    let file_content = fs::read_to_string(&novel_file_path).unwrap();
    let json: Value = serde_json::from_str(&file_content).unwrap();
    let content_index = &json["metadata"]["content_index"]["chapters"];

    // Should have entries for chapters 1 and 2
    assert!(
        content_index["https://example.com/novel/chapter-1"].is_object(),
        "Chapter 1 should be in content index"
    );
    assert!(
        content_index["https://example.com/novel/chapter-2"].is_object(),
        "Chapter 2 should be in content index"
    );

    // Should not have entries for chapters 3, 4, 5
    assert!(
        content_index["https://example.com/novel/chapter-3"].is_null(),
        "Chapter 3 should not be in content index"
    );
    assert!(
        content_index["https://example.com/novel/chapter-4"].is_null(),
        "Chapter 4 should not be in content index"
    );
    assert!(
        content_index["https://example.com/novel/chapter-5"].is_null(),
        "Chapter 5 should not be in content index"
    );

    // Verify metadata has correct content sizes
    let ch1_metadata = &content_index["https://example.com/novel/chapter-1"];
    let ch2_metadata = &content_index["https://example.com/novel/chapter-2"];

    assert_eq!(
        ch1_metadata["content_size"].as_u64().unwrap() as usize,
        chapter1_content.data.len(),
        "Chapter 1 content size should be correct"
    );
    assert_eq!(
        ch2_metadata["content_size"].as_u64().unwrap() as usize,
        chapter2_content.data.len(),
        "Chapter 2 content size should be correct"
    );
}

#[tokio::test]
async fn test_cli_add_handles_novel_metadata_updates() {
    // Test that novel metadata (title, authors, description, etc.) gets updated
    // while preserving content index

    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await.unwrap();

    // Store initial novel
    let initial_novel = create_initial_novel();
    let novel_id = storage.store_novel(&initial_novel).await.unwrap();

    // Store some content
    let content = ChapterContent {
        data: "Test content".to_string(),
    };
    storage
        .store_chapter_content(
            &novel_id,
            1,
            "https://example.com/novel/chapter-1",
            &content,
        )
        .await
        .unwrap();

    // Update novel with different metadata
    let mut updated_novel = initial_novel.clone();
    updated_novel.title = "Updated Novel Title".to_string();
    updated_novel.authors.push("New Co-Author".to_string());
    updated_novel
        .description
        .push("Additional description line.".to_string());
    updated_novel.cover = Some("https://example.com/new-cover.jpg".to_string());

    // Store updated novel
    storage.store_novel(&updated_novel).await.unwrap();

    // Verify content is still there
    let chapters = storage.list_chapters(&novel_id).await.unwrap();
    assert!(chapters[0].has_content(), "Content should be preserved");

    // Verify updated metadata in the stored novel
    let retrieved_novel = storage.get_novel(&novel_id).await.unwrap().unwrap();
    assert_eq!(retrieved_novel.title, "Updated Novel Title");
    assert_eq!(retrieved_novel.authors.len(), 2);
    assert_eq!(retrieved_novel.description.len(), 2);
    assert!(retrieved_novel.cover.is_some());
}

fn create_initial_novel() -> Novel {
    Novel {
        url: "https://example.com/novel".to_string(),
        title: "Test Novel".to_string(),
        authors: vec!["Author Name".to_string()],
        cover: None,
        description: vec!["A test novel for CLI behavior testing.".to_string()],
        volumes: vec![Volume {
            name: "Volume 1".to_string(),
            index: 1,
            chapters: vec![
                Chapter {
                    title: "Chapter 1: The Start".to_string(),
                    index: 1,
                    url: "https://example.com/novel/chapter-1".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 2: The Middle".to_string(),
                    index: 2,
                    url: "https://example.com/novel/chapter-2".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 3: The Conflict".to_string(),
                    index: 3,
                    url: "https://example.com/novel/chapter-3".to_string(),
                    updated_at: None,
                },
            ],
        }],
        metadata: vec![],
        status: NovelStatus::Ongoing,
        langs: vec!["en".to_string()],
    }
}

fn create_updated_novel_with_new_chapters() -> Novel {
    Novel {
        url: "https://example.com/novel".to_string(), // Same URL - this is key!
        title: "Test Novel - Updated".to_string(),
        authors: vec!["Author Name".to_string()],
        cover: Some("https://example.com/cover.jpg".to_string()),
        description: vec![
            "A test novel for CLI behavior testing.".to_string(),
            "This novel has been updated with new chapters.".to_string(),
        ],
        volumes: vec![Volume {
            name: "Volume 1".to_string(),
            index: 1,
            chapters: vec![
                Chapter {
                    title: "Chapter 1: The Start".to_string(),
                    index: 1,
                    url: "https://example.com/novel/chapter-1".to_string(),
                    updated_at: Some("2023-12-01T00:00:00Z".to_string()),
                },
                Chapter {
                    title: "Chapter 2: The Middle".to_string(),
                    index: 2,
                    url: "https://example.com/novel/chapter-2".to_string(),
                    updated_at: Some("2023-12-01T00:00:00Z".to_string()),
                },
                Chapter {
                    title: "Chapter 3: The Conflict".to_string(),
                    index: 3,
                    url: "https://example.com/novel/chapter-3".to_string(),
                    updated_at: None,
                },
                // New chapters added!
                Chapter {
                    title: "Chapter 4: The Resolution".to_string(),
                    index: 4,
                    url: "https://example.com/novel/chapter-4".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 5: The End".to_string(),
                    index: 5,
                    url: "https://example.com/novel/chapter-5".to_string(),
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
