//! Integration test to verify that the novel structure in the manifest file is properly updated
//! when chapter content is stored and deleted.

use quelle_engine::bindings::quelle::extension::novel::{Chapter, NovelStatus, Volume};
use quelle_storage::{
    backends::filesystem::FilesystemStorage, traits::BookStorage, ChapterContent, Novel,
};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_novel_structure_updates_on_content_changes() {
    // Create a temporary directory for this test
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await.unwrap();

    // Create a test novel
    let novel = create_test_novel();
    let novel_id = storage.store_novel(&novel).await.unwrap();

    // Get the actual novel file path from the storage backend
    // We need to replicate the logic from get_novel_file to find the correct path
    let id_str = novel_id.as_str();
    let parts: Vec<&str> = id_str.splitn(2, "::").collect();
    let source_id = parts.first().unwrap_or(&"unknown");
    let novel_url = parts.get(1).unwrap_or(&id_str);

    // Create a SHA256 hash like the storage backend does
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(novel_url.as_bytes());
    let novel_hash = format!("{:x}", hasher.finalize());

    let novel_file_path = temp_dir
        .path()
        .join("novels")
        .join(source_id)
        .join(novel_hash)
        .join("novel.json");

    // Read the initial novel manifest
    let initial_content = fs::read_to_string(&novel_file_path).unwrap();
    let initial_json: Value = serde_json::from_str(&initial_content).unwrap();

    // Check that content_index initially has no chapters
    let content_index = &initial_json["metadata"]["content_index"]["chapters"];
    assert!(
        content_index.as_object().unwrap().is_empty(),
        "Content index should be empty initially"
    );

    // Store content for the first chapter
    let chapter_content = ChapterContent {
        data: "This is the content of Chapter 1: The Beginning.".to_string(),
    };

    let chapter_info = storage
        .store_chapter_content(
            &novel_id,
            1,
            "https://example.com/chapter-1",
            &chapter_content,
        )
        .await
        .unwrap();

    assert!(chapter_info.has_content());
    assert!(chapter_info.content_size().is_some());

    // Read the updated novel manifest
    let updated_content = fs::read_to_string(&novel_file_path).unwrap();
    let updated_json: Value = serde_json::from_str(&updated_content).unwrap();

    // Check that the first chapter now shows in content_index
    let updated_content_index = &updated_json["metadata"]["content_index"]["chapters"];
    let chapter1_url = "https://example.com/chapter-1";
    let chapter2_url = "https://example.com/chapter-2";

    // Verify Chapter 1 is in content index
    assert!(
        updated_content_index[chapter1_url].is_object(),
        "Chapter 1 should be in content index"
    );
    assert!(
        updated_content_index[chapter1_url]["content_size"]
            .as_u64()
            .is_some(),
        "Chapter 1 should have content_size in index"
    );
    assert!(
        updated_content_index[chapter1_url]["stored_at"]
            .as_str()
            .is_some(),
        "Chapter 1 should have stored_at in index"
    );

    // Verify Chapter 2 is not in content index
    assert!(
        updated_content_index[chapter2_url].is_null(),
        "Chapter 2 should not be in content index"
    );

    // Store content for the second chapter
    let chapter2_content = ChapterContent {
        data: "This is the content of Chapter 2: The Journey. It's much longer than the first chapter to test different content sizes.".to_string(),
    };

    storage
        .store_chapter_content(
            &novel_id,
            1,
            "https://example.com/chapter-2",
            &chapter2_content,
        )
        .await
        .unwrap();

    // Read the manifest again
    let final_content = fs::read_to_string(&novel_file_path).unwrap();
    let final_json: Value = serde_json::from_str(&final_content).unwrap();
    let final_content_index = &final_json["metadata"]["content_index"]["chapters"];

    // Both chapters should now be in content index
    assert!(
        final_content_index[chapter1_url].is_object(),
        "Chapter 1 should be in content index"
    );
    assert!(
        final_content_index[chapter2_url].is_object(),
        "Chapter 2 should be in content index"
    );

    // Test deletion
    let deleted_chapter = storage
        .delete_chapter_content(&novel_id, 1, "https://example.com/chapter-1")
        .await
        .unwrap();

    assert!(
        deleted_chapter.is_some(),
        "Should return ChapterInfo for deleted chapter"
    );
    let deleted_info = deleted_chapter.unwrap();
    assert!(
        !deleted_info.has_content(),
        "Deleted chapter should not have content"
    );

    // Read the manifest after deletion
    let after_delete_content = fs::read_to_string(&novel_file_path).unwrap();
    let after_delete_json: Value = serde_json::from_str(&after_delete_content).unwrap();
    let after_delete_content_index = &after_delete_json["metadata"]["content_index"]["chapters"];

    // Verify Chapter 1 is no longer in content index
    assert!(
        after_delete_content_index[chapter1_url].is_null(),
        "Chapter 1 should not be in content index after deletion"
    );

    // Verify Chapter 2 is still in content index
    assert!(
        after_delete_content_index[chapter2_url].is_object(),
        "Chapter 2 should still be in content index"
    );

    // Test list_chapters method reads from novel structure
    let chapter_list = storage.list_chapters(&novel_id).await.unwrap();

    assert!(
        !chapter_list[0].has_content(),
        "Chapter 1 should show no content in list_chapters"
    );
    assert!(
        chapter_list[1].has_content(),
        "Chapter 2 should show content in list_chapters"
    );
}

fn create_test_novel() -> Novel {
    Novel {
        url: "https://example.com/novel".to_string(),
        title: "Test Novel for Structure Updates".to_string(),
        authors: vec!["Test Author".to_string()],
        cover: None,
        description: vec!["A test novel to verify structure updates.".to_string()],
        volumes: vec![Volume {
            name: "Volume 1".to_string(),
            index: 1,
            chapters: vec![
                Chapter {
                    title: "Chapter 1: The Beginning".to_string(),
                    index: 1,
                    url: "https://example.com/chapter-1".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 2: The Journey".to_string(),
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
