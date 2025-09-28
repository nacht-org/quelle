//! Basic usage example for the storage crate.
//!
//! This example demonstrates how to use the FilesystemStorage backend
//! to store and retrieve novels and chapters.
//!
//! Run with: cargo run --example basic_usage

use quelle_engine::bindings::quelle::extension::novel::{
    Chapter, ChapterContent, Metadata, Namespace, Novel, NovelStatus, Volume,
};
use storage::{BookStorage, FilesystemStorage, NovelFilter};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Storage Basic Usage Example");

    // Create a temporary directory for this example
    let temp_dir = TempDir::new()?;
    println!("📁 Using temporary storage at: {:?}", temp_dir.path());

    // Create and initialize the filesystem storage
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await?;
    println!("✅ Storage initialized");

    // Create a sample novel
    let novel = create_sample_novel();
    println!("📚 Created sample novel: '{}'", novel.title);

    // Store the novel
    let novel_id = storage.store_novel(&novel).await?;
    println!("💾 Stored novel with ID: {}", novel_id);

    // Retrieve the novel by ID
    let retrieved_novel = storage.get_novel(&novel_id).await?;
    match retrieved_novel {
        Some(n) => println!("📖 Retrieved novel by ID: '{}'", n.title),
        None => println!("❌ Novel not found!"),
    }

    // Retrieve the novel by URL (more common lookup pattern)
    let found_by_url = storage.find_novel_by_url(&novel.url).await?;
    match found_by_url {
        Some(n) => println!("🔍 Found novel by URL: '{}'", n.title),
        None => println!("❌ Novel not found by URL!"),
    }

    // Store some chapter content
    let chapter_content = ChapterContent {
        data: "This is the content of the first chapter. It's a great story about...".to_string(),
    };

    storage
        .store_chapter_content(
            &novel_id,
            1, // volume_index
            "https://example.com/novel/chapter-1",
            &chapter_content,
        )
        .await?;
    println!("📄 Stored chapter content");

    // Retrieve the chapter content
    let retrieved_content = storage
        .get_chapter_content(&novel_id, 1, "https://example.com/novel/chapter-1")
        .await?;

    match retrieved_content {
        Some(content) => println!(
            "📝 Retrieved chapter content: {} characters",
            content.data.len()
        ),
        None => println!("❌ Chapter content not found!"),
    }

    // List all novels
    let filter = NovelFilter::default();
    let novels = storage.list_novels(&filter).await?;
    println!("📋 Found {} novels in storage", novels.len());

    for summary in &novels {
        println!(
            "  - {}: {} by {}",
            summary.id,
            summary.title,
            summary.authors.join(", ")
        );
    }

    // List chapters for the novel
    let chapters = storage.list_chapters(&novel_id).await?;
    println!("📄 Found {} chapters in novel", chapters.len());

    for chapter in &chapters {
        println!(
            "  - Volume {}, Chapter {}: '{}'",
            chapter.volume_index, chapter.chapter_index, chapter.chapter_title
        );
    }

    // Get storage statistics
    let stats = storage.get_storage_stats().await?;
    println!("📊 Storage Stats:");
    println!("  - Total novels: {}", stats.total_novels);
    println!("  - Total chapters: {}", stats.total_chapters);
    println!("  - Novels by source:");
    for (source, count) in &stats.novels_by_source {
        println!("    - {}: {}", source, count);
    }

    // Search novels
    let search_results = storage.search_novels("Sample").await?;
    println!(
        "🔍 Search results for 'Sample': {} novels found",
        search_results.len()
    );

    // Cleanup demo
    let cleanup_report = storage.cleanup_dangling_data().await?;
    println!("🧹 Cleanup report:");
    println!(
        "  - Orphaned chapters removed: {}",
        cleanup_report.orphaned_chapters_removed
    );
    println!("  - Novels fixed: {}", cleanup_report.novels_fixed);
    println!("  - Errors: {}", cleanup_report.errors_encountered.len());

    println!("✨ Example completed successfully!");

    Ok(())
}

/// Creates a sample novel for demonstration purposes
fn create_sample_novel() -> Novel {
    Novel {
        url: "https://example.com/novel/sample-novel".to_string(),
        authors: vec!["Jane Author".to_string(), "John Writer".to_string()],
        title: "Sample Novel: A Great Story".to_string(),
        cover: Some("https://example.com/covers/sample-novel.jpg".to_string()),
        description: vec![
            "This is a fantastic novel about adventures.".to_string(),
            "Join our heroes on their epic journey!".to_string(),
        ],
        volumes: vec![
            Volume {
                name: "Volume 1: The Beginning".to_string(),
                index: 1,
                chapters: vec![
                    Chapter {
                        title: "Chapter 1: The Start".to_string(),
                        index: 1,
                        url: "https://example.com/novel/chapter-1".to_string(),
                        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
                    },
                    Chapter {
                        title: "Chapter 2: The Journey".to_string(),
                        index: 2,
                        url: "https://example.com/novel/chapter-2".to_string(),
                        updated_at: Some("2024-01-02T00:00:00Z".to_string()),
                    },
                ],
            },
            Volume {
                name: "Volume 2: The Adventure Continues".to_string(),
                index: 2,
                chapters: vec![Chapter {
                    title: "Chapter 3: New Challenges".to_string(),
                    index: 3,
                    url: "https://example.com/novel/chapter-3".to_string(),
                    updated_at: Some("2024-01-03T00:00:00Z".to_string()),
                }],
            },
        ],
        metadata: vec![
            Metadata {
                name: "genre".to_string(),
                value: "Fantasy".to_string(),
                ns: Namespace::Dc,
                others: vec![("subgenre".to_string(), "Epic Fantasy".to_string())],
            },
            Metadata {
                name: "language".to_string(),
                value: "English".to_string(),
                ns: Namespace::Dc,
                others: vec![],
            },
        ],
        status: NovelStatus::Ongoing,
        langs: vec!["en".to_string()],
    }
}
