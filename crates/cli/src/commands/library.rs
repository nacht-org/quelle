use eyre::Result;
use quelle_storage::{
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};

use crate::cli::LibraryCommands;

pub async fn handle_library_command(
    cmd: LibraryCommands,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        LibraryCommands::List { source } => handle_list_novels(source, storage).await,
        LibraryCommands::Show { novel_id } => handle_show_novel(novel_id, storage).await,
        LibraryCommands::Chapters {
            novel_id,
            downloaded_only,
        } => handle_list_chapters(novel_id, downloaded_only, storage).await,
        LibraryCommands::Read { novel_id, chapter } => {
            handle_read_chapter(novel_id, chapter, storage).await
        }
        LibraryCommands::Sync { novel_id } => handle_sync_novels(novel_id, dry_run).await,
        LibraryCommands::Update { novel_id } => handle_update_novels(novel_id, dry_run).await,
        LibraryCommands::Remove { novel_id, force } => {
            handle_remove_novel(novel_id, force, storage, dry_run).await
        }
        LibraryCommands::Cleanup => handle_cleanup_library(storage, dry_run).await,
        LibraryCommands::Stats => handle_library_stats(storage).await,
    }
}

async fn handle_list_novels(source: Option<String>, storage: &FilesystemStorage) -> Result<()> {
    let filter = if let Some(source) = source {
        NovelFilter {
            source_ids: vec![source],
        }
    } else {
        NovelFilter { source_ids: vec![] }
    };

    let novels = storage.list_novels(&filter).await?;
    if novels.is_empty() {
        println!("📚 No novels found in library.");
        println!("💡 Use 'quelle fetch novel <url>' to add novels to your library.");
    } else {
        println!("📚 Library ({} novels):", novels.len());
        for novel in novels {
            println!("  📖 {} by {}", novel.title, novel.authors.join(", "));
            println!("     ID: {}", novel.id.as_str());
            println!("     Status: {:?}", novel.status);
            if novel.total_chapters > 0 {
                println!(
                    "     Chapters: {} total, {} downloaded",
                    novel.total_chapters, novel.stored_chapters
                );
            }
            println!();
        }
    }
    Ok(())
}

async fn handle_show_novel(novel_id: String, storage: &FilesystemStorage) -> Result<()> {
    let id = NovelId::new(novel_id);
    match storage.get_novel(&id).await? {
        Some(novel) => {
            println!("📖 {}", novel.title);
            println!("Authors: {}", novel.authors.join(", "));
            println!("URL: {}", novel.url);
            println!("Status: {:?}", novel.status);
            if !novel.langs.is_empty() {
                println!("Languages: {}", novel.langs.join(", "));
            }
            if !novel.description.is_empty() {
                println!("Description: {}", novel.description.join(" "));
            }
            if let Some(cover) = &novel.cover {
                println!("Cover: {}", cover);
            }
        }
        None => {
            println!("❌ Novel not found: {}", id.as_str());
        }
    }
    Ok(())
}

async fn handle_list_chapters(
    novel_id: String,
    downloaded_only: bool,
    storage: &FilesystemStorage,
) -> Result<()> {
    let id = NovelId::new(novel_id);
    let chapters = storage.list_chapters(&id).await?;

    if chapters.is_empty() {
        println!("📄 No chapters found for novel: {}", id.as_str());
        return Ok(());
    }

    println!("📄 Chapters for {}:", id.as_str());
    for chapter in chapters {
        if !downloaded_only || chapter.has_content() {
            let status = if chapter.has_content() { "✅" } else { "⬜" };
            println!(
                "  {} {} - {}",
                status, chapter.chapter_index, chapter.chapter_title
            );
        }
    }
    Ok(())
}

async fn handle_read_chapter(
    novel_id: String,
    chapter: String,
    storage: &FilesystemStorage,
) -> Result<()> {
    let novel_id = NovelId::new(novel_id);
    let chapters = storage.list_chapters(&novel_id).await?;

    if let Some(chapter_info) = chapters
        .iter()
        .find(|c| c.chapter_index.to_string() == chapter || c.chapter_url == chapter)
    {
        match storage
            .get_chapter_content(
                &novel_id,
                chapter_info.volume_index,
                &chapter_info.chapter_url,
            )
            .await?
        {
            Some(content) => {
                println!(
                    "📖 {} - {}",
                    chapter_info.chapter_index, chapter_info.chapter_title
                );
                println!("{}", "=".repeat(50));
                println!("{}", content.data);
            }
            None => {
                println!(
                    "❌ Chapter content not downloaded: {}",
                    chapter_info.chapter_title
                );
                println!(
                    "💡 Use 'quelle fetch chapter {}' to download it",
                    chapter_info.chapter_url
                );
            }
        }
    } else {
        println!("❌ Chapter not found: {}", chapter);
    }
    Ok(())
}

async fn handle_sync_novels(novel_id: String, dry_run: bool) -> Result<()> {
    if dry_run {
        if novel_id == "all" {
            println!("Would sync all novels for new chapters");
        } else {
            println!("Would sync novel {} for new chapters", novel_id);
        }
        return Ok(());
    }

    if novel_id == "all" {
        println!("🔄 Syncing all novels for new chapters...");
        println!("🚧 This feature requires:");
        println!("  1. Extension engine to re-fetch novel metadata");
        println!("  2. Comparison with stored chapter lists");
        println!("  3. Detection of new chapters since last sync");
        println!("💡 This would check all novels in your library for new chapters");
        println!("💡 Use 'quelle library update all' to download new content");
    } else {
        let id = NovelId::new(novel_id.clone());
        println!("🔄 Syncing novel {} for new chapters...", id.as_str());
        println!("🚧 This feature requires:");
        println!(
            "  1. Extension engine to re-fetch novel metadata for: {}",
            novel_id
        );
        println!("  2. Comparison with stored chapter list");
        println!("  3. Detection of new chapters since last sync");
        println!(
            "💡 Use 'quelle library update {}' to download new content",
            novel_id
        );
    }
    Ok(())
}

async fn handle_update_novels(novel_id: String, dry_run: bool) -> Result<()> {
    if dry_run {
        if novel_id == "all" {
            println!("Would fetch new chapters for all novels");
        } else {
            println!("Would fetch new chapters for novel: {}", novel_id);
        }
        return Ok(());
    }

    if novel_id == "all" {
        println!("📥 Updating all novels with new chapters...");
        println!("🚧 This feature requires:");
        println!("  1. Integration with fetch command logic");
        println!("  2. Automatic detection of novels that need updates");
        println!("  3. Batch processing of chapter downloads");
        println!("💡 For now, use 'quelle fetch chapters <novel_id>' for individual novels");
    } else {
        let id = NovelId::new(novel_id.clone());
        println!("📥 Updating novel {} with new chapters...", id.as_str());
        println!("🚧 This feature requires:");
        println!("  1. Integration with fetch command logic");
        println!("  2. Detection of new chapters for: {}", novel_id);
        println!("  3. Automatic chapter content download");
        println!("💡 For now, use 'quelle fetch chapters {}'", novel_id);
    }
    Ok(())
}

async fn handle_remove_novel(
    novel_id: String,
    force: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would remove novel: {}", novel_id);
        return Ok(());
    }

    let id = NovelId::new(novel_id);
    match storage.get_novel(&id).await? {
        Some(novel) => {
            if !force {
                print!("Are you sure you want to remove '{}'? (y/N): ", novel.title);
                use std::io::{self, Write};
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("❌ Cancelled");
                    return Ok(());
                }
            }

            storage.delete_novel(&id).await?;
            println!("✅ Removed novel: {}", novel.title);
        }
        None => {
            println!("❌ Novel not found: {}", id.as_str());
        }
    }
    Ok(())
}

async fn handle_cleanup_library(storage: &FilesystemStorage, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would perform library cleanup");
        return Ok(());
    }

    println!("🧹 Cleaning up library...");
    let report = storage.cleanup_dangling_data().await?;
    println!("✅ Cleanup completed:");
    println!(
        "  Removed {} orphaned chapters",
        report.orphaned_chapters_removed
    );
    println!("  Updated {} novel metadata", report.novels_fixed);
    if !report.errors_encountered.is_empty() {
        println!("  {} errors encountered:", report.errors_encountered.len());
        for error in &report.errors_encountered {
            println!("    - {}", error);
        }
    }
    Ok(())
}

async fn handle_library_stats(storage: &FilesystemStorage) -> Result<()> {
    let novels = storage.list_novels(&NovelFilter::default()).await?;
    let total_novels = novels.len();
    let total_chapters: u32 = novels.iter().map(|n| n.total_chapters).sum();
    let downloaded_chapters: u32 = novels.iter().map(|n| n.stored_chapters).sum();

    println!("📊 Library Statistics:");
    println!("  📖 Novels: {}", total_novels);
    println!(
        "  📄 Chapters: {} total, {} downloaded",
        total_chapters, downloaded_chapters
    );

    if total_chapters > 0 {
        let percentage = (downloaded_chapters as f64 / total_chapters as f64) * 100.0;
        println!("  📊 Download progress: {:.1}%", percentage);
    }
    Ok(())
}
