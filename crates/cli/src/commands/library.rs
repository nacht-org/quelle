//! Library management command handlers for browsing and maintaining novel collections.

use eyre::Result;
use quelle_engine::{ExtensionEngine, registry::ExtensionSession};
use quelle_storage::{
    ChapterContent,
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};
use quelle_store::StoreManager;
use tracing::{error, warn};

use crate::engine::{ExtensionRegistry, create_extension_engine};
use crate::resolve::{resolve_novel_id, show_novel_not_found_help};
use crate::{cli::LibraryCommands, engine::create_extension_session};

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

/// Handle the `update` command — sync and download new chapters.
pub async fn handle_update_command(
    novel: String,
    check_only: bool,
    storage: &FilesystemStorage,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would update novel(s): {}", novel);
        return Ok(());
    }

    if novel == "all" {
        if check_only {
            println!("Checking for updates...");
            return handle_sync_novels("all".to_string(), storage, store_manager, false).await;
        } else {
            println!("Updating all novels...");
            let engine = create_extension_engine()?;
            return handle_update_novels("all".to_string(), storage, store_manager, &engine, false)
                .await;
        }
    }

    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            let novel_id_str = novel_id.as_str().to_string();
            if check_only {
                handle_sync_novels(novel_id_str, storage, store_manager, false).await
            } else {
                let engine = create_extension_engine()?;
                handle_update_novels(novel_id_str, storage, store_manager, &engine, false).await
            }
        }
        None => {
            show_novel_not_found_help(&novel, storage).await;
            Ok(())
        }
    }
}

/// Handle the `read` command — display chapter content or chapter list.
pub async fn handle_read_command(
    novel: String,
    chapter: Option<String>,
    list: bool,
    storage: &FilesystemStorage,
    _store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would read from novel: {}", novel);
        return Ok(());
    }

    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            let novel_id_str = novel_id.as_str().to_string();
            if list {
                handle_list_chapters(novel_id_str, true, storage).await
            } else {
                match chapter {
                    Some(chapter_id) => {
                        handle_read_chapter(novel_id_str, chapter_id, storage).await
                    }
                    None => {
                        println!("Specify a chapter number or use --list.");
                        handle_list_chapters(novel_id_str, true, storage).await
                    }
                }
            }
        }
        None => {
            show_novel_not_found_help(&novel, storage).await;
            Ok(())
        }
    }
}

/// Handle the `remove` command — delete a novel from the library.
pub async fn handle_remove_command(
    novel: String,
    force: bool,
    storage: &FilesystemStorage,
    _store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would remove novel: {}", novel);
        return Ok(());
    }

    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            handle_remove_novel(novel_id.as_str().to_string(), force, storage, false).await
        }
        None => {
            show_novel_not_found_help(&novel, storage).await;
            Ok(())
        }
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
        println!("No novels in library.");
    } else {
        println!("novels: {}", novels.len());
        for novel in novels {
            println!("  {} - {} chapters", novel.title, novel.total_chapters);
        }
    }
    Ok(())
}

async fn handle_show_novel(novel_input: String, storage: &FilesystemStorage) -> Result<()> {
    match resolve_novel_id(&novel_input, storage).await? {
        Some(novel_id) => match storage.get_novel(&novel_id).await? {
            Some(novel) => {
                println!("title: {}", novel.title);
                println!("authors: {}", novel.authors.join(", "));
                println!("status: {:?}", novel.status);
            }
            None => {
                eprintln!("Not found: {}", novel_id.as_str());
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
                println!("No chapters found.");
                return Ok(());
            }

            println!("chapters: {}", chapters.len());
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
                            "{} - {}",
                            chapter_info.chapter_index, chapter_info.chapter_title
                        );
                        println!("{}", "=".repeat(50));
                        println!("{}", content.data);
                    }
                    None => {
                        eprintln!(
                            "Chapter content not downloaded: {}",
                            chapter_info.chapter_title
                        );
                        eprintln!(
                            "Use 'quelle fetch chapter {}' to download it.",
                            chapter_info.chapter_url
                        );
                    }
                }
            } else {
                eprintln!("Not found: chapter '{}'", chapter);
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
            println!("Would sync all novels for new chapters.");
        } else {
            println!("Would sync novel '{}' for new chapters.", novel_input);
        }
        return Ok(());
    }

    if novel_input == "all" {
        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("No novels to sync.");
            return Ok(());
        }

        let mut total_new_chapters = 0u32;
        let mut synced_count = 0u32;
        let mut failed_count = 0u32;

        for novel_summary in novels {
            match sync_single_novel(&novel_summary.id, storage, store_manager).await {
                Ok(new_chapters) => {
                    if new_chapters > 0 {
                        println!("  {}: {} new chapter(s)", novel_summary.title, new_chapters);
                        total_new_chapters += new_chapters;
                    }
                    synced_count += 1;
                }
                Err(e) => {
                    warn!("Failed to sync '{}': {}", novel_summary.title, e);
                    failed_count += 1;
                }
            }
        }
        println!(
            "synced: {}, new chapters: {}, failed: {}",
            synced_count, total_new_chapters, failed_count
        );
    } else {
        match resolve_novel_id(&novel_input, storage).await? {
            Some(novel_id) => match sync_single_novel(&novel_id, storage, store_manager).await {
                Ok(new_chapters) => {
                    if new_chapters > 0 {
                        println!("{} new chapter(s) found.", new_chapters);
                    } else {
                        println!("Up to date.");
                    }
                }
                Err(e) => {
                    eprintln!("Error: Failed to sync: {}", e);
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
    store_manager: &mut StoreManager,
    engine: &ExtensionEngine,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        if novel_input == "all" {
            println!("Would fetch new chapters for all novels.");
        } else {
            println!("Would fetch new chapters for novel: {}", novel_input);
        }
        return Ok(());
    }

    if novel_input == "all" {
        println!("Updating all novels...");

        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("No novels to update.");
            return Ok(());
        }

        let mut extension_registry = ExtensionRegistry::new(engine, store_manager);

        let mut total_downloaded = 0u32;
        let mut updated_count = 0u32;
        let mut failed_count = 0u32;

        for novel_summary in novels {
            let novel = storage
                .get_novel(&novel_summary.id)
                .await?
                .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_summary.id.as_str()))?;

            let extension = extension_registry.get_extension(&novel.url).await?;
            match update_single_novel(&novel_summary.id, storage, &extension).await {
                Ok(downloaded) => {
                    if downloaded > 0 {
                        println!(
                            "  {}: {} chapter(s) downloaded",
                            novel_summary.title, downloaded
                        );
                        total_downloaded += downloaded;
                    }
                    updated_count += 1;
                }
                Err(e) => {
                    warn!("Failed to update '{}': {}", novel_summary.title, e);
                    failed_count += 1;
                }
            }
        }
        println!(
            "updated: {}, downloaded: {}, failed: {}",
            updated_count, total_downloaded, failed_count
        );
    } else {
        match resolve_novel_id(&novel_input, storage).await? {
            Some(novel_id) => {
                println!("Updating '{}'...", novel_input);
                let novel = storage
                    .get_novel(&novel_id)
                    .await?
                    .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

                let extension = create_extension_session(engine, store_manager, &novel.url).await?;
                match update_single_novel(&novel_id, storage, &extension).await {
                    Ok(downloaded) => {
                        if downloaded > 0 {
                            println!("{} chapter(s) downloaded.", downloaded);
                        } else {
                            println!("Up to date.");
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: Failed to update: {}", e);
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
    store_manager: &mut StoreManager,
) -> Result<u32> {
    let stored_novel = storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

    let engine = create_extension_engine()?;
    let extension = create_extension_session(&engine, store_manager, &stored_novel.url).await?;
    let fresh_novel = crate::engine::fetch_novel(&extension, &stored_novel.url).await?;

    let stored_chapters = storage.list_chapters(novel_id).await?;
    let stored_chapter_urls: std::collections::HashSet<_> =
        stored_chapters.iter().map(|ch| &ch.chapter_url).collect();

    let mut new_chapters = 0u32;
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
    extension: &ExtensionSession<'_>,
) -> Result<u32> {
    let chapters = storage.list_chapters(novel_id).await?;
    let mut downloaded_count = 0u32;
    let mut failed_count = 0u32;

    for chapter_info in chapters {
        if !chapter_info.has_content() {
            tracing::info!("Downloading chapter: {}", chapter_info.chapter_title);
            match crate::engine::fetch_chapter(&extension, &chapter_info.chapter_url).await {
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
                            error!("Failed to store '{}': {}", chapter_info.chapter_title, e);
                            failed_count += 1;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch '{}': {}", chapter_info.chapter_title, e);
                    failed_count += 1;
                }
            }
        }
    }

    if failed_count > 0 {
        warn!("{} chapter(s) failed to download.", failed_count);
    }

    Ok(downloaded_count)
}

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
                    print!("Remove '{}'? [y/N]: ", novel.title);
                    use std::io::{self, Write};
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    if !input.trim().to_lowercase().starts_with('y') {
                        println!("Cancelled.");
                        return Ok(());
                    }
                }

                storage.delete_novel(&novel_id).await?;
                println!("Removed.");
            }
            None => {
                eprintln!("Not found: {}", novel_id.as_str());
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
        println!("Would perform library cleanup.");
        return Ok(());
    }

    let report = storage.cleanup_dangling_data().await?;
    println!(
        "orphaned chapters removed: {}",
        report.orphaned_chapters_removed
    );
    println!("novels fixed: {}", report.novels_fixed);
    if !report.errors_encountered.is_empty() {
        eprintln!(
            "Warning: {} error(s) during cleanup:",
            report.errors_encountered.len()
        );
        for err in &report.errors_encountered {
            eprintln!("  {}", err);
        }
    }
    Ok(())
}

async fn handle_library_stats(storage: &FilesystemStorage) -> Result<()> {
    let novels = storage.list_novels(&NovelFilter::default()).await?;
    let total_novels = novels.len();
    let total_chapters: u32 = novels.iter().map(|n| n.total_chapters).sum();
    let downloaded_chapters: u32 = novels.iter().map(|n| n.stored_chapters).sum();

    println!("novels: {}", total_novels);
    println!(
        "chapters: {} total, {} downloaded",
        total_chapters, downloaded_chapters
    );

    if total_chapters > 0 {
        let percentage = (downloaded_chapters as f64 / total_chapters as f64) * 100.0;
        println!("download progress: {:.1}%", percentage);
    }
    Ok(())
}
