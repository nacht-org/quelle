//! Example demonstrating efficient novel metadata updates
//!
//! This example shows how to use the new helper methods to update
//! novel metadata without rewriting the entire novel structure.

use chrono::Utc;
use quelle_storage::{
    backends::filesystem::{FilesystemStorage, NovelStorageMetadata},
    traits::BookStorage,
};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for this example
    let temp_dir = TempDir::new()?;
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await?;

    // Create a test novel (you would get this from your extension)
    let novel = create_test_novel();

    println!("📚 Storing novel: {}", novel.title);
    let novel_id = storage.store_novel(&novel).await?;

    // Scenario 1: Update just the timestamp (e.g., when chapter content is saved)
    println!("\n⏰ Updating novel timestamp...");
    storage.touch_novel(&novel_id).await?;
    println!("✅ Novel timestamp updated");

    // Scenario 2: Update specific metadata (e.g., change source_id or add custom metadata)
    println!("\n📝 Updating novel metadata...");
    let custom_metadata = NovelStorageMetadata {
        source_id: "custom_source".to_string(),
        stored_at: Utc::now(),
        content_index: quelle_storage::models::ContentIndex::default(),
    };
    storage
        .update_novel_metadata(&novel_id, custom_metadata)
        .await?;
    println!("✅ Novel metadata updated with custom source_id");

    // Scenario 3: Read the current metadata to check values
    println!("\n🔍 Reading current novel metadata...");
    if let Some(novel) = storage.get_novel(&novel_id).await? {
        println!("Novel title: {}", novel.title);
        println!(
            "Total chapters: {}",
            novel
                .volumes
                .iter()
                .map(|v| v.chapters.len())
                .sum::<usize>()
        );
    }

    println!("\n💡 Benefits of these helper methods:");
    println!("  • Atomic updates - no risk of corrupting the file");
    println!("  • Efficient - only read/write what's necessary");
    println!("  • Clean API - easy to understand and use");
    println!("  • Safe - proper error handling and validation");

    Ok(())
}

fn create_test_novel() -> quelle_storage::Novel {
    use quelle_engine::bindings::quelle::extension::novel::{Chapter, NovelStatus, Volume};
    use quelle_storage::Novel;

    Novel {
        url: "https://example.com/novel".to_string(),
        title: "Example Novel".to_string(),
        authors: vec!["Test Author".to_string()],
        cover: None,
        description: vec!["A test novel for demonstration.".to_string()],
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
