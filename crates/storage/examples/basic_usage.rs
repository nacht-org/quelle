//! Basic usage example for the storage crate.
//!
//! This example demonstrates how to use the FilesystemStorage backend
//! to store and retrieve novels and chapters, including working with
//! chapter storage metadata.
//!
//! Run with: cargo run --example basic_usage

use quelle_engine::bindings::quelle::extension::novel::{
    Chapter, ChapterContent, Novel, NovelStatus, Volume,
};
use quelle_storage::{BookStorage, ChapterContentStatus, FilesystemStorage, NovelFilter};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary directory for this example
    let temp_dir = TempDir::new()?;
    let storage = FilesystemStorage::new(temp_dir.path());
    storage.initialize().await?;

    // Create a sample novel
    let novel = create_sample_novel();
    println!("Created novel: '{}'", novel.title);

    // Store the novel
    let novel_id = storage.store_novel(&novel).await?;
    println!("Stored novel with ID: {}", novel_id);

    // Retrieve the novel by URL
    let found_novel = storage.find_novel_by_url(&novel.url).await?;
    assert!(found_novel.is_some());
    println!("Found novel by URL: '{}'", found_novel.unwrap().title);

    // List all chapters (initially none have content)
    let chapters = storage.list_chapters(&novel_id).await?;
    println!("\nChapters ({} total):", chapters.len());
    for chapter in &chapters {
        match &chapter.content_status {
            ChapterContentStatus::NotStored => {
                println!(
                    "  ❌ {}: {} (no content)",
                    chapter.chapter_index, chapter.chapter_title
                );
            }
            ChapterContentStatus::Stored {
                content_size,
                stored_at,
                ..
            } => {
                println!(
                    "  ✅ {}: {} ({} bytes, stored {})",
                    chapter.chapter_index,
                    chapter.chapter_title,
                    content_size,
                    stored_at.format("%Y-%m-%d %H:%M")
                );
            }
        }
    }

    // Store content for one chapter
    let chapter_content = ChapterContent {
        data: "This is the content of chapter 1. The story begins here...".to_string(),
    };

    storage
        .store_chapter_content(
            &novel_id,
            1, // volume_index
            "https://example.com/novel/chapter-1",
            &chapter_content,
        )
        .await?;
    println!("\nStored content for chapter 1");

    // List chapters again to see the updated status
    let chapters = storage.list_chapters(&novel_id).await?;
    println!("\nUpdated chapter status:");
    for chapter in &chapters {
        match &chapter.content_status {
            ChapterContentStatus::NotStored => {
                println!(
                    "  ❌ {}: {} (missing content)",
                    chapter.chapter_index, chapter.chapter_title
                );
            }
            ChapterContentStatus::Stored {
                content_size,
                stored_at,
                ..
            } => {
                println!(
                    "  ✅ {}: {} ({} bytes, stored {})",
                    chapter.chapter_index,
                    chapter.chapter_title,
                    content_size,
                    stored_at.format("%Y-%m-%d %H:%M")
                );
            }
        }
    }

    // Show which chapters still need content
    let missing_chapters: Vec<_> = chapters
        .iter()
        .filter(|ch| matches!(ch.content_status, ChapterContentStatus::NotStored))
        .collect();

    if !missing_chapters.is_empty() {
        println!("\nChapters still needing content:");
        for chapter in missing_chapters {
            println!("  - {}: {}", chapter.chapter_index, chapter.chapter_title);
        }
    }

    // Show summary
    let filter = NovelFilter { source_ids: vec![] };
    let novels = storage.list_novels(&filter).await?;
    let stored_chapters_count = chapters.iter().filter(|c| c.has_content()).count();

    println!("\nSummary:");
    println!("  Novels: {}", novels.len());
    println!("  Chapters with content: {}", stored_chapters_count);

    Ok(())
}

fn create_sample_novel() -> Novel {
    Novel {
        url: "https://example.com/novel/sample-novel".to_string(),
        authors: vec!["Jane Author".to_string()],
        title: "Sample Novel".to_string(),
        cover: None,
        description: vec!["A sample novel for testing.".to_string()],
        volumes: vec![Volume {
            name: "Volume 1".to_string(),
            index: 1,
            chapters: vec![
                Chapter {
                    title: "Chapter 1: The Beginning".to_string(),
                    index: 1,
                    url: "https://example.com/novel/chapter-1".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 2: The Journey".to_string(),
                    index: 2,
                    url: "https://example.com/novel/chapter-2".to_string(),
                    updated_at: None,
                },
                Chapter {
                    title: "Chapter 3: The End".to_string(),
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
