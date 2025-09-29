use eyre::Result;
use quelle_storage::{
    ChapterContent,
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};
use tracing::{error, info, warn};

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
        LibraryCommands::Sync { novel_id } => handle_sync_novels(novel_id, storage, dry_run).await,
        LibraryCommands::Update { novel_id } => {
            handle_update_novels(novel_id, storage, dry_run).await
        }
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

async fn handle_sync_novels(
    novel_id: String,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        if novel_id == "all" {
            println!("Would sync all novels for new chapters");
        } else {
            println!("Would sync novel {} for new chapters", novel_id);
        }
        return Ok(());
    }

    // Initialize extension infrastructure
    let mut store_manager = crate::utils::create_store_manager().await?;
    let engine = crate::utils::create_extension_engine()?;

    if novel_id == "all" {
        println!("🔄 Syncing all novels for new chapters...");

        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("📚 No novels in library to sync");
            return Ok(());
        }

        let mut total_new_chapters = 0;
        let mut synced_count = 0;
        let mut failed_count = 0;

        for novel_summary in novels {
            match sync_single_novel(&novel_summary.id, storage, &mut store_manager, &engine).await {
                Ok(new_chapters) => {
                    if new_chapters > 0 {
                        println!(
                            "  📖 {} - {} new chapters found",
                            novel_summary.title, new_chapters
                        );
                        total_new_chapters += new_chapters;
                    } else {
                        println!("  📖 {} - up to date", novel_summary.title);
                    }
                    synced_count += 1;
                }
                Err(e) => {
                    warn!("❌ Failed to sync {}: {}", novel_summary.title, e);
                    failed_count += 1;
                }
            }
        }

        println!("\n📊 Sync Summary:");
        println!("  🔄 Novels synced: {}", synced_count);
        println!("  📄 New chapters found: {}", total_new_chapters);
        if failed_count > 0 {
            println!("  ❌ Failed syncs: {}", failed_count);
        }

        if total_new_chapters > 0 {
            println!("\n💡 Use 'quelle library update all' to download new chapters");
        }
    } else {
        let id = NovelId::new(novel_id.clone());
        println!("🔄 Syncing novel {} for new chapters...", id.as_str());

        match sync_single_novel(&id, storage, &mut store_manager, &engine).await {
            Ok(new_chapters) => {
                if new_chapters > 0 {
                    println!("✅ Found {} new chapters", new_chapters);
                    println!(
                        "💡 Use 'quelle library update {}' to download them",
                        novel_id
                    );
                } else {
                    println!("✅ Novel is up to date - no new chapters found");
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to sync novel: {}", e);
                return Err(e);
            }
        }
    }
    Ok(())
}

async fn handle_update_novels(
    novel_id: String,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        if novel_id == "all" {
            println!("Would fetch new chapters for all novels");
        } else {
            println!("Would fetch new chapters for novel: {}", novel_id);
        }
        return Ok(());
    }

    // Initialize extension infrastructure
    let mut store_manager = crate::utils::create_store_manager().await?;
    let engine = crate::utils::create_extension_engine()?;

    if novel_id == "all" {
        println!("📥 Updating all novels with new chapters...");

        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("📚 No novels in library to update");
            return Ok(());
        }

        let mut total_downloaded = 0;
        let mut updated_count = 0;
        let mut failed_count = 0;

        for novel_summary in novels {
            match update_single_novel(&novel_summary.id, storage, &mut store_manager, &engine).await
            {
                Ok(downloaded) => {
                    if downloaded > 0 {
                        println!(
                            "  📖 {} - downloaded {} chapters",
                            novel_summary.title, downloaded
                        );
                        total_downloaded += downloaded;
                    } else {
                        println!("  📖 {} - no new chapters to download", novel_summary.title);
                    }
                    updated_count += 1;
                }
                Err(e) => {
                    warn!("❌ Failed to update {}: {}", novel_summary.title, e);
                    failed_count += 1;
                }
            }
        }

        println!("\n📊 Update Summary:");
        println!("  📖 Novels processed: {}", updated_count);
        println!("  📄 Chapters downloaded: {}", total_downloaded);
        if failed_count > 0 {
            println!("  ❌ Failed updates: {}", failed_count);
        }
    } else {
        let id = NovelId::new(novel_id.clone());
        println!("📥 Updating novel {} with new chapters...", id.as_str());

        match update_single_novel(&id, storage, &mut store_manager, &engine).await {
            Ok(downloaded) => {
                if downloaded > 0 {
                    println!("✅ Downloaded {} new chapters", downloaded);
                } else {
                    println!("✅ No new chapters to download - novel is up to date");
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to update novel: {}", e);
                return Err(e);
            }
        }
    }
    Ok(())
}

async fn sync_single_novel(
    novel_id: &NovelId,
    storage: &FilesystemStorage,
    store_manager: &mut quelle_store::StoreManager,
    engine: &quelle_engine::ExtensionEngine,
) -> Result<u32> {
    // Get the stored novel
    let stored_novel = storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

    // Find extension for this novel
    let extension = find_and_install_extension_for_url(&stored_novel.url, store_manager).await?;

    // Fetch fresh novel metadata
    let fresh_novel = fetch_novel_with_extension(
        &extension,
        store_manager.registry_store(),
        &stored_novel.url,
    )
    .await?;

    // Get current chapters from storage
    let stored_chapters = storage.list_chapters(novel_id).await?;
    let stored_chapter_urls: std::collections::HashSet<_> =
        stored_chapters.iter().map(|ch| &ch.chapter_url).collect();

    // Count new chapters
    let mut new_chapters = 0;
    for volume in &fresh_novel.volumes {
        for chapter in &volume.chapters {
            if !stored_chapter_urls.contains(&chapter.url) {
                new_chapters += 1;
            }
        }
    }

    // Update stored novel with fresh metadata (this will add new chapters)
    if new_chapters > 0 {
        storage.store_novel(&fresh_novel).await?;
    }

    Ok(new_chapters)
}

async fn update_single_novel(
    novel_id: &NovelId,
    storage: &FilesystemStorage,
    store_manager: &mut quelle_store::StoreManager,
    engine: &quelle_engine::ExtensionEngine,
) -> Result<u32> {
    // Get the stored novel
    let stored_novel = storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

    // Find extension for this novel
    let extension = find_and_install_extension_for_url(&stored_novel.url, store_manager).await?;

    // Get chapters that need downloading
    let chapters = storage.list_chapters(novel_id).await?;
    let mut downloaded_count = 0;
    let mut failed_count = 0;

    for chapter_info in chapters {
        if !chapter_info.has_content() {
            info!("📄 Downloading chapter: {}", chapter_info.chapter_title);
            match fetch_chapter_with_extension(
                &extension,
                store_manager.registry_store(),
                &chapter_info.chapter_url,
            )
            .await
            {
                Ok(chapter_content) => {
                    let content = ChapterContent {
                        data: chapter_content.data,
                    };
                    match storage
                        .store_chapter_content(
                            novel_id,
                            chapter_info.volume_index,
                            &chapter_info.chapter_url,
                            &content,
                        )
                        .await
                    {
                        Ok(_) => {
                            downloaded_count += 1;
                        }
                        Err(e) => {
                            error!("  ❌ Failed to store {}: {}", chapter_info.chapter_title, e);
                            failed_count += 1;
                        }
                    }
                }
                Err(e) => {
                    error!("  ❌ Failed to fetch {}: {}", chapter_info.chapter_title, e);
                    failed_count += 1;
                }
            }
        }
    }

    if failed_count > 0 {
        warn!("⚠️ {} chapters failed to download", failed_count);
    }

    Ok(downloaded_count)
}

// Helper functions from fetch.rs - we need to add these to avoid duplication
use crate::commands::fetch::{
    fetch_chapter_with_extension, fetch_novel_with_extension, find_and_install_extension_for_url,
};

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
