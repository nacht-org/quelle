use eyre::Result;
use quelle_storage::backends::filesystem::FilesystemStorage;
use quelle_store::StoreManager;
use url::Url;

use crate::utils::{resolve_novel_id, show_novel_not_found_help};

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
            println!("Would also fetch all chapters");
        }
        return Ok(());
    }

    println!("📚 Adding novel from: {}", url);

    // First, fetch the novel metadata
    let fetch_novel_cmd = FetchCommands::Novel { url: url.clone() };
    handle_fetch_command(fetch_novel_cmd, store_manager, storage, false).await?;

    // Then fetch chapters unless explicitly disabled
    if !no_chapters {
        println!("📄 Fetching chapters...");
        let fetch_chapters_cmd = FetchCommands::Chapters {
            novel_id: url.to_string(),
        };

        // TODO: Handle max_chapters limit in the future
        if max_chapters.is_some() {
            println!(
                "💡 Note: max_chapters limit not yet implemented, fetching all available chapters"
            );
        }

        handle_fetch_command(fetch_chapters_cmd, store_manager, storage, false).await?;
    }

    println!("✅ Novel added successfully!");
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

    // Handle "all" case
    if novel == "all" {
        if check_only {
            println!("🔍 Checking all novels for new chapters");
            return handle_sync_novels("all".to_string(), storage, store_manager, false).await;
        } else {
            println!("🔄 Updating all novels with new chapters");
            return handle_update_novels("all".to_string(), storage, false).await;
        }
    }

    // Resolve the novel identifier
    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            let novel_id_str = novel_id.as_str().to_string();
            if check_only {
                println!("🔍 Checking for new chapters in: {}", novel);
                handle_sync_novels(novel_id_str, storage, store_manager, false).await
            } else {
                println!("🔄 Updating novel: {}", novel);
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

    // Resolve the novel identifier
    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            let novel_id_str = novel_id.as_str().to_string();

            use crate::commands::library::{handle_list_chapters, handle_read_chapter};

            if list {
                println!("📚 Listing chapters for: {}", novel);
                handle_list_chapters(novel_id_str, true, storage).await
            } else {
                match chapter {
                    Some(chapter_id) => {
                        println!("📖 Reading chapter: {}", chapter_id);
                        handle_read_chapter(novel_id_str, chapter_id, storage).await
                    }
                    None => {
                        println!(
                            "📚 Please specify a chapter to read, or use --list to see available chapters"
                        );
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

    // Resolve the novel identifier
    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            println!("🗑️  Removing novel: {}", novel);
            handle_remove_novel(novel_id.as_str().to_string(), force, storage, false).await
        }
        None => {
            show_novel_not_found_help(&novel, storage).await;
            Ok(())
        }
    }
}

/// Handle the export command - export novels to various formats
pub async fn handle_export_command(
    novel: String,
    format: String,
    output: Option<String>,
    include_images: bool,
    _storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would export novel '{}' in {} format", novel, format);
        if let Some(output_dir) = &output {
            println!("Output directory: {}", output_dir);
        }
        return Ok(());
    }

    match format.as_str() {
        "epub" => {
            // Call the export functionality directly
            println!("📖 Exporting novel '{}' to EPUB format", novel);
            if let Some(output_dir) = &output {
                println!("Output directory: {}", output_dir);
            }
            if include_images {
                println!("Including images in export");
            }

            // TODO: Implement direct EPUB export functionality
            println!("💡 EPUB export functionality will be implemented here");
            Ok(())
        }
        _ => {
            println!("❌ Unsupported format: {}", format);
            println!("💡 Supported formats: epub");
            Ok(())
        }
    }
}
