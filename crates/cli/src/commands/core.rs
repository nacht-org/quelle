//! Core CLI command handlers for novel management operations.

use eyre::Result;
use quelle_storage::backends::filesystem::FilesystemStorage;
use quelle_store::StoreManager;
use url::Url;

use crate::{
    commands::export::handle_export,
    utils::{resolve_novel_id, show_novel_not_found_help},
};

/// Handle the add command - add a novel to library
pub async fn handle_add_command(
    url: Url,
    no_chapters: bool,
    max_chapters: Option<usize>,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    use crate::cli::FetchCommands;
    use crate::commands::fetch::handle_fetch_command;

    if dry_run {
        println!("Would add novel from: {}", url);
        if !no_chapters {
            match max_chapters {
                Some(limit) => println!("Would fetch first {} chapters", limit),
                None => println!("Would fetch all chapters"),
            }
        }
        return Ok(());
    }

    println!("Adding novel: {}", url);

    let fetch_novel_cmd = FetchCommands::Novel { url: url.clone() };
    handle_fetch_command(fetch_novel_cmd, store_manager, storage, false).await?;

    if !no_chapters {
        println!("Fetching chapters...");

        handle_fetch_chapters_with_limit(url.to_string(), max_chapters, store_manager, storage)
            .await?;
    }

    println!("Novel added");
    Ok(())
}

/// Handle the update command - update novels with new chapters
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

    use crate::commands::library::{handle_sync_novels, handle_update_novels};

    if novel == "all" {
        if check_only {
            println!("Checking for updates...");
            return handle_sync_novels("all".to_string(), storage, store_manager, false).await;
        } else {
            println!("Updating all novels...");
            return handle_update_novels("all".to_string(), storage, false).await;
        }
    }

    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            let novel_id_str = novel_id.as_str().to_string();
            if check_only {
                handle_sync_novels(novel_id_str, storage, store_manager, false).await
            } else {
                handle_update_novels(novel_id_str, storage, false).await
            }
        }
        None => {
            show_novel_not_found_help(&novel, storage).await;
            Ok(())
        }
    }
}

/// Handle the read command - read a chapter from library
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

            use crate::commands::library::{handle_list_chapters, handle_read_chapter};

            if list {
                handle_list_chapters(novel_id_str, true, storage).await
            } else {
                match chapter {
                    Some(chapter_id) => {
                        handle_read_chapter(novel_id_str, chapter_id, storage).await
                    }
                    None => {
                        println!("Specify chapter number or use --list");
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

/// Handle the remove command - remove a novel from library
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

    use crate::commands::library::handle_remove_novel;

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

/// Handle fetching chapters with optional limit
async fn handle_fetch_chapters_with_limit(
    novel_id: String,
    max_chapters: Option<usize>,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
) -> Result<()> {
    use crate::commands::fetch::{
        fetch_chapter_with_extension, find_and_install_extension_for_url,
    };
    use quelle_storage::{ChapterContent, traits::BookStorage, types::NovelId};

    println!("Fetching chapters...");

    let (novel, novel_storage_id) = if novel_id.starts_with("http") {
        let novel = match storage.find_novel_by_url(&novel_id).await? {
            Some(novel) => novel,
            None => {
                println!("Novel not found: {}", novel_id);
                return Ok(());
            }
        };
        let filter = quelle_storage::types::NovelFilter { source_ids: vec![] };
        let novels = storage.list_novels(&filter).await?;
        let storage_id = novels
            .iter()
            .find(|n| n.title == novel.title)
            .map(|n| n.id.clone())
            .unwrap_or_else(|| NovelId::new(novel_id.clone()));
        (novel, storage_id)
    } else {
        let id = NovelId::new(novel_id.clone());
        let novel = match storage.get_novel(&id).await? {
            Some(novel) => novel,
            None => {
                println!("Novel not found: {}", novel_id);
                return Ok(());
            }
        };
        (novel, id)
    };

    let extension = match find_and_install_extension_for_url(&novel.url, store_manager).await {
        Ok(ext) => ext,
        Err(e) => {
            tracing::error!("Failed to find/install extension: {}", e);
            return Err(e);
        }
    };

    let mut chapters = storage.list_chapters(&novel_storage_id).await?;
    let original_count = chapters.len();

    if let Some(limit) = max_chapters
        && chapters.len() > limit
    {
        chapters.truncate(limit);
        println!("Limited to {} of {} chapters", limit, original_count);
    }

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;

    println!("Processing {} chapters", chapters.len());

    for chapter_info in chapters {
        if chapter_info.has_content() {
            skipped_count += 1;
            continue;
        }

        print!(".");

        let chapter_content = match fetch_chapter_with_extension(
            &extension,
            store_manager.registry_store(),
            &chapter_info.chapter_url,
        )
        .await
        {
            Ok(content) => content,
            Err(e) => {
                tracing::error!("  Failed to fetch {}: {}", chapter_info.chapter_title, e);
                failed_count += 1;
                continue;
            }
        };

        let content = ChapterContent {
            data: chapter_content.data,
        };

        match storage
            .store_chapter_content(
                &novel_storage_id,
                chapter_info.volume_index,
                &chapter_info.chapter_url,
                &content,
            )
            .await
        {
            Ok(_updated_chapter) => {
                success_count += 1;
            }
            Err(e) => {
                tracing::error!("  Failed to store {}: {}", chapter_info.chapter_title, e);
                failed_count += 1;
            }
        }
    }

    println!(
        "\nFetched: {}, skipped: {}, failed: {}",
        success_count, skipped_count, failed_count
    );

    Ok(())
}

/// Handle the export command - export novels to various formats
pub async fn handle_export_command(
    novel_input: String,
    format: String,
    output: Option<String>,
    include_images: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match format.as_str() {
        "epub" => {}
        #[cfg(feature = "pdf")]
        "pdf" => {}
        _ => {
            eprintln!("Unsupported format: {}", format);
            #[cfg(feature = "pdf")]
            eprintln!("Supported: epub, pdf");
            #[cfg(not(feature = "pdf"))]
            eprintln!("Supported: epub");
            return Ok(());
        }
    }

    if dry_run {
        println!("Would export novel '{}' in {} format", novel_input, format);
        if let Some(ref output_dir) = output {
            println!("  Output dir: {}", output_dir);
        }
        println!("  Include images: {}", include_images);
        return Ok(());
    }

    handle_export(
        novel_input,
        format,
        output,
        include_images,
        storage,
        dry_run,
    )
    .await
}
