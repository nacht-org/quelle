//! Library management command handlers for browsing and maintaining novel collections.

use eyre::Result;
use quelle_storage::{
    ChapterContent,
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};
use quelle_store::StoreManager;
use tracing::{error, info, warn};

use crate::cli::LibraryCommands;
use crate::utils::{resolve_novel_id, show_novel_not_found_help};

pub async fn handle_library_command(
    cmd: LibraryCommands,
    storage: &FilesystemStorage,
    _store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        LibraryCommands::List { source } => handle_list_novels(source, storage).await,
        LibraryCommands::Show { novel } => handle_show_novel(novel, storage).await,
        LibraryCommands::Chapters {
            novel,
            downloaded_only,
        } => handle_list_chapters(novel, downloaded_only, storage).await,
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
        println!("No novels in library");
    } else {
        println!("Library ({} novels):", novels.len());
        for novel in novels {
            println!("{} - {} chapters", novel.title, novel.total_chapters);
        }
    }
    Ok(())
}

async fn handle_show_novel(novel_input: String, storage: &FilesystemStorage) -> Result<()> {
    match resolve_novel_id(&novel_input, storage).await? {
        Some(novel_id) => match storage.get_novel(&novel_id).await? {
            Some(novel) => {
                println!("{}", novel.title);
                println!("Authors: {}", novel.authors.join(", "));
                println!("Status: {:?}", novel.status);
            }
            None => {
                println!("Novel not found: {}", novel_id.as_str());
            }
        },
        None => {
            show_novel_not_found_help(&novel_input, storage).await;
        }
    }
    Ok(())
}

pub async fn handle_list_chapters(
    novel_input: String,
    downloaded_only: bool,
    storage: &FilesystemStorage,
) -> Result<()> {
    match resolve_novel_id(&novel_input, storage).await? {
        Some(novel_id) => {
            let chapters = storage.list_chapters(&novel_id).await?;

            if chapters.is_empty() {
                println!("No chapters found");
                return Ok(());
            }

            println!("Chapters ({}):", chapters.len());
            for chapter in chapters {
                if !downloaded_only || chapter.has_content() {
                    let status = if chapter.has_content() { "Y" } else { "N" };
                    println!(
                        "  [{}] Ch.{} {}",
                        status, chapter.chapter_index, chapter.chapter_title
                    );
                }
            }
        }
        None => {
            show_novel_not_found_help(&novel_input, storage).await;
        }
    }
    Ok(())
}

pub async fn handle_read_chapter(
    novel_input: String,
    chapter: String,
    storage: &FilesystemStorage,
) -> Result<()> {
    match resolve_novel_id(&novel_input, storage).await? {
        Some(novel_id) => {
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
                println!("Chapter not found: {}", chapter);
            }
        }
        None => {
            show_novel_not_found_help(&novel_input, storage).await;
        }
    }
    Ok(())
}

pub async fn handle_sync_novels(
    novel_input: String,
    storage: &dyn BookStorage,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        if novel_input == "all" {
            println!("Would sync all novels for new chapters");
        } else {
            println!("Would sync novel {} for new chapters", novel_input);
        }
        return Ok(());
    }

    if novel_input == "all" {
        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("No novels to sync");
            return Ok(());
        }

        let mut total_new_chapters = 0;
        let mut synced_count = 0;
        let mut failed_count = 0;

        for novel_summary in novels {
            match sync_single_novel(&novel_summary.id, storage, store_manager).await {
                Ok(new_chapters) => {
                    if new_chapters > 0 {
                        println!(
                            "  📖 {} - {} new chapters found",
                            novel_summary.title, new_chapters
                        );
                        total_new_chapters += new_chapters;
                    }
                    synced_count += 1;
                }
                Err(e) => {
                    warn!("❌ Failed to sync {}: {}", novel_summary.title, e);
                    failed_count += 1;
                }
            }
        }
        println!(
            "Synced: {}, new chapters: {}, failed: {}",
            synced_count, total_new_chapters, failed_count
        );
    } else {
        match resolve_novel_id(&novel_input, storage).await? {
            Some(novel_id) => match sync_single_novel(&novel_id, storage, store_manager).await {
                Ok(new_chapters) => {
                    if new_chapters > 0 {
                        println!("Found {} new chapters", new_chapters);
                    } else {
                        println!("Up to date");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to sync: {}", e);
                    return Err(e);
                }
            },
            None => {
                show_novel_not_found_help(&novel_input, storage).await;
            }
        }
    }
    Ok(())
}

pub async fn handle_update_novels(
    novel_input: String,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        if novel_input == "all" {
            println!("Would fetch new chapters for all novels");
        } else {
            println!("Would fetch new chapters for novel: {}", novel_input);
        }
        return Ok(());
    }

    let mut store_manager = crate::utils::create_store_manager().await?;
    let engine = crate::utils::create_extension_engine()?;

    if novel_input == "all" {
        println!("📥 Updating all novels with new chapters...");

        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("No novels to update");
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
                    }
                    updated_count += 1;
                }
                Err(e) => {
                    warn!("❌ Failed to update {}: {}", novel_summary.title, e);
                    failed_count += 1;
                }
            }
        }
        println!(
            "Updated: {}, downloaded: {}, failed: {}",
            updated_count, total_downloaded, failed_count
        );
    } else {
        match resolve_novel_id(&novel_input, storage).await? {
            Some(novel_id) => {
                println!("📥 Updating novel {} with new chapters...", novel_input);

                match update_single_novel(&novel_id, storage, &mut store_manager, &engine).await {
                    Ok(downloaded) => {
                        if downloaded > 0 {
                            println!("Downloaded {} chapters", downloaded);
                        } else {
                            println!("Up to date");
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to update: {}", e);
                        return Err(e);
                    }
                }
            }
            None => {
                show_novel_not_found_help(&novel_input, storage).await;
            }
        }
    }
    Ok(())
}

async fn sync_single_novel(
    novel_id: &NovelId,
    storage: &dyn BookStorage,
    store_manager: &mut quelle_store::StoreManager,
) -> Result<u32> {
    let stored_novel = storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

    let extension = find_and_install_extension_for_url(&stored_novel.url, store_manager).await?;

    let fresh_novel = fetch_novel_with_extension(
        &extension,
        store_manager.registry_store(),
        &stored_novel.url,
    )
    .await?;

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

    if new_chapters > 0 {
        storage.store_novel(&fresh_novel).await?;
    }

    Ok(new_chapters)
}

async fn update_single_novel(
    novel_id: &NovelId,
    storage: &FilesystemStorage,
    store_manager: &mut quelle_store::StoreManager,
    _engine: &quelle_engine::ExtensionEngine,
) -> Result<u32> {
    let stored_novel = storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

    let extension = find_and_install_extension_for_url(&stored_novel.url, store_manager).await?;

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
                        Ok(_updated_chapter) => {
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

pub async fn handle_remove_novel(
    novel_input: String,
    force: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would remove novel: {}", novel_input);
        return Ok(());
    }

    match resolve_novel_id(&novel_input, storage).await? {
        Some(novel_id) => match storage.get_novel(&novel_id).await? {
            Some(novel) => {
                if !force {
                    print!("Are you sure you want to remove '{}'? (y/N): ", novel.title);
                    use std::io::{self, Write};
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    if !input.trim().to_lowercase().starts_with('y') {
                        println!("Cancelled");
                        return Ok(());
                    }
                }

                storage.delete_novel(&novel_id).await?;
                println!("Removed: {}", novel.title);
            }
            None => {
                println!("Novel not found: {}", novel_id.as_str());
            }
        },
        None => {
            show_novel_not_found_help(&novel_input, storage).await;
        }
    }
    Ok(())
}

async fn handle_cleanup_library(storage: &FilesystemStorage, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would perform library cleanup");
        return Ok(());
    }

    let report = storage.cleanup_dangling_data().await?;
    println!(
        "Cleanup: {} orphaned chapters removed",
        report.orphaned_chapters_removed
    );
    println!("Updated {} novel metadata", report.novels_fixed);
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

    println!("Novels: {}", total_novels);
    println!(
        "Chapters: {} total, {} downloaded",
        total_chapters, downloaded_chapters
    );

    if total_chapters > 0 {
        let percentage = (downloaded_chapters as f64 / total_chapters as f64) * 100.0;
        println!("  📊 Download progress: {:.1}%", percentage);
    }
    Ok(())
}
