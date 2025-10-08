//! Fetch command handlers for retrieving novel and chapter data.

use eyre::Result;

use quelle_storage::{
    ChapterContent,
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{AssetId, NovelId},
};
use quelle_store::StoreManager;
use std::io::Cursor;
use tracing::{error, info, warn};
use url::Url;

use crate::cli::FetchCommands;

pub async fn handle_fetch_command(
    cmd: FetchCommands,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        FetchCommands::Novel { url } => {
            handle_fetch_novel(url, store_manager, storage, dry_run).await
        }
        FetchCommands::Chapter { url } => {
            handle_fetch_chapter(url, store_manager, storage, dry_run).await
        }
        FetchCommands::Chapters { novel_id } => {
            handle_fetch_chapters(novel_id, store_manager, storage, dry_run).await
        }
        FetchCommands::All { url } => handle_fetch_all(url, store_manager, storage, dry_run).await,
    }
}

async fn handle_fetch_novel(
    url: Url,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch novel from: {}", url);
        return Ok(());
    }

    // Find extension that can handle this URL
    match find_extension_for_url(url.as_ref(), store_manager).await? {
        Some((extension_id, _store_name)) => {
            if store_manager.get_installed(&extension_id).await?.is_none() {
                match store_manager.install(&extension_id, None, None).await {
                    Ok(installed) => {
                        println!("Installed {} v{}", installed.name, installed.version);
                    }
                    Err(e) => {
                        eprintln!("Failed to install {}: {}", extension_id, e);
                        return Err(e.into());
                    }
                }
            }

            println!("Fetching novel...");

            if let Some(installed) = store_manager.get_installed(&extension_id).await? {
                match fetch_novel_with_extension(
                    &installed,
                    store_manager.registry_store(),
                    url.as_ref(),
                )
                .await
                {
                    Ok(novel) => {
                        println!("{}", novel.title);
                        println!("Authors: {}", novel.authors.join(", "));

                        let total_chapters: u32 =
                            novel.volumes.iter().map(|v| v.chapters.len() as u32).sum();
                        println!("Chapters: {}", total_chapters);

                        // Save to local storage
                        match storage.store_novel(&novel).await {
                            Ok(novel_id) => {
                                println!(
                                    "💾 Saved to local library with ID: {}",
                                    novel_id.as_str()
                                );

                                // Fetch cover image if available
                                if let Some(cover_url) = &novel.cover {
                                    match fetch_and_store_asset(&novel_id, cover_url, storage).await
                                    {
                                        Ok(_) => {}
                                        Err(e) => warn!("Failed to fetch cover: {}", e),
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to save: {}", e);
                                return Err(e.into());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch novel: {}", e);
                        return Err(e);
                    }
                }
            } else {
                eprintln!("Extension {} not found", extension_id);
            }
        }
        None => {
            eprintln!("No compatible extension found for: {}", url);
        }
    }
    Ok(())
}

async fn handle_fetch_chapter(
    url: Url,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch chapter from: {}", url);
        return Ok(());
    }

    let (extension_id, _store_name) =
        match find_extension_for_url(url.as_ref(), store_manager).await? {
            Some(ext) => ext,
            None => {
                eprintln!("No compatible extension found for: {}", url);
                return Ok(());
            }
        };
    if store_manager.get_installed(&extension_id).await?.is_none() {
        let installed = match store_manager.install(&extension_id, None, None).await {
            Ok(installed) => installed,
            Err(e) => {
                eprintln!("Failed to install {}: {}", extension_id, e);
                return Err(e.into());
            }
        };
        println!("Installed {} v{}", installed.name, installed.version);
    }

    let installed = match store_manager.get_installed(&extension_id).await? {
        Some(installed) => installed,
        None => {
            eprintln!("Extension {} not found", extension_id);
            return Ok(());
        }
    };

    let chapter = match fetch_chapter_with_extension(
        &installed,
        store_manager.registry_store(),
        url.as_ref(),
    )
    .await
    {
        Ok(chapter) => chapter,
        Err(e) => {
            eprintln!("Failed to fetch chapter: {}", e);
            return Err(e);
        }
    };

    println!("Chapter fetched ({} chars)", chapter.data.len());
    let _preview = if chapter.data.len() > 200 {
        format!("{}...", &chapter.data[..200])
    } else {
        chapter.data.clone()
    };
    let novel = match storage.find_novel_by_url(url.as_ref()).await {
        Ok(Some(novel)) => novel,
        _ => {
            println!("Chapter not saved - novel not in library");
            return Ok(());
        }
    };

    // Find the chapter in the novel structure
    let mut saved = false;
    for volume in &novel.volumes {
        if volume.chapters.iter().any(|ch| ch.url == url.to_string()) {
            // Find the novel ID from the library listing
            let filter = quelle_storage::types::NovelFilter { source_ids: vec![] };
            let novels = match storage.list_novels(&filter).await {
                Ok(novels) => novels,
                Err(e) => {
                    eprintln!("Failed to list novels: {}", e);
                    break;
                }
            };
            if let Some(novel_summary) = novels.iter().find(|n| n.title == novel.title) {
                match storage
                    .store_chapter_content(&novel_summary.id, volume.index, url.as_ref(), &chapter)
                    .await
                {
                    Ok(_updated_chapter) => {
                        println!("💾 Saved chapter content to local library");
                        saved = true;
                    }
                    Err(e) => {
                        eprintln!("Failed to save chapter: {}", e);
                    }
                }
            }
            break;
        }
    }
    if !saved {
        println!("Chapter not saved - could not locate in novel structure");
    }

    Ok(())
}

async fn handle_fetch_chapters(
    novel_id: String,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch all chapters for: {}", novel_id);
        return Ok(());
    }

    println!("Fetching all chapters for novel: {}", novel_id);

    // Try to find novel by ID or URL
    let (novel, novel_storage_id) = if novel_id.starts_with("http") {
        let novel = match storage.find_novel_by_url(&novel_id).await? {
            Some(novel) => novel,
            None => {
                println!("Novel not found with URL: {}", novel_id);
                return Ok(());
            }
        };
        // Find the storage ID from the library listing
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
                println!("Novel not found with ID: {}", novel_id);
                return Ok(());
            }
        };
        (novel, id)
    };

    let extension = match find_and_install_extension_for_url(&novel.url, store_manager).await {
        Ok(ext) => ext,
        Err(e) => {
            error!("Failed to find/install extension: {}", e);
            return Err(e);
        }
    };

    let chapters = storage.list_chapters(&novel_storage_id).await?;
    let mut success_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;

    println!("📄 Found {} chapters to process", chapters.len());

    for chapter_info in chapters {
        if chapter_info.has_content() {
            println!("  {} (already downloaded)", chapter_info.chapter_title);
            skipped_count += 1;
            continue;
        }

        println!("Fetching: {}", chapter_info.chapter_title);

        let chapter_content = match fetch_chapter_with_extension(
            &extension,
            store_manager.registry_store(),
            &chapter_info.chapter_url,
        )
        .await
        {
            Ok(content) => content,
            Err(e) => {
                error!("  Failed to fetch {}: {}", chapter_info.chapter_title, e);
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
                println!("  {}", chapter_info.chapter_title);
                success_count += 1;
            }
            Err(e) => {
                error!("  Failed to store {}: {}", chapter_info.chapter_title, e);
                failed_count += 1;
            }
        }
    }

    println!("Fetch complete:");
    println!("  Successfully fetched: {}", success_count);
    println!("  Already downloaded: {}", skipped_count);
    if failed_count > 0 {
        println!("  Failed: {}", failed_count);
    }
    Ok(())
}

async fn handle_fetch_all(
    url: Url,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch everything from: {}", url);
        return Ok(());
    }

    println!("Fetching novel and chapters...");

    handle_fetch_novel(url.clone(), store_manager, storage, false).await?;
    handle_fetch_chapters(url.to_string(), store_manager, storage, false).await?;

    println!("Complete");
    Ok(())
}

/// Find an extension that can handle the given URL
pub async fn find_extension_for_url(
    url: &str,
    store_manager: &StoreManager,
) -> Result<Option<(String, String)>> {
    store_manager
        .find_extension_for_url(url)
        .await
        .map_err(|e| eyre::eyre!("Failed to find extension for URL: {}", e))
}

/// Find and install an extension that can handle the given URL
pub async fn find_and_install_extension_for_url(
    url: &str,
    store_manager: &mut StoreManager,
) -> Result<quelle_store::models::InstalledExtension> {
    match find_extension_for_url(url, store_manager).await? {
        Some((extension_id, _store_name)) => {
            // Install extension if not already installed
            if let Some(installed) = store_manager.get_installed(&extension_id).await? {
                return Ok(installed);
            }

            println!("Installing extension {}...", extension_id);
            match store_manager.install(&extension_id, None, None).await {
                Ok(installed) => {
                    info!("Installed {} v{}", installed.name, installed.version);
                    Ok(installed)
                }
                Err(e) => {
                    error!("Failed to install {}: {}", extension_id, e);
                    Err(e.into())
                }
            }
        }
        None => Err(eyre::eyre!(
            "No extension found for URL: {}\n\
             Try adding more extension stores with: quelle store add",
            url
        )),
    }
}

/// Fetch novel information using an installed extension
pub async fn fetch_novel_with_extension(
    installed: &quelle_store::models::InstalledExtension,
    registry: &dyn quelle_store::registry::RegistryStore,
    url: &str,
) -> Result<quelle_storage::Novel> {
    let engine = crate::utils::create_extension_engine()?;
    let wasm_bytes = registry.get_extension_wasm_bytes(&installed.id).await?;
    let runner = engine.new_runner_from_bytes(&wasm_bytes).await?;
    let (_, result) = runner.fetch_novel_info(url).await?;

    match result {
        Ok(novel) => Ok(novel),
        Err(wit_error) => Err(eyre::eyre!("Extension error: {:?}", wit_error)),
    }
}

/// Fetch chapter content using an installed extension
pub async fn fetch_chapter_with_extension(
    installed: &quelle_store::models::InstalledExtension,
    registry: &dyn quelle_store::registry::RegistryStore,
    url: &str,
) -> Result<ChapterContent> {
    let engine = crate::utils::create_extension_engine()?;
    let wasm_bytes = registry.get_extension_wasm_bytes(&installed.id).await?;
    let runner = engine.new_runner_from_bytes(&wasm_bytes).await?;
    let (_, result) = runner.fetch_chapter(url).await?;

    match result {
        Ok(chapter) => Ok(chapter),
        Err(wit_error) => Err(eyre::eyre!("Extension error: {:?}", wit_error)),
    }
}

async fn fetch_and_store_asset(
    novel_id: &NovelId,
    asset_url: &str,
    storage: &FilesystemStorage,
) -> Result<AssetId> {
    if let Some(existing_asset_id) = storage.find_asset_by_url(asset_url).await? {
        println!("Cover already downloaded");
        return Ok(existing_asset_id);
    }

    println!("Downloading cover...");
    let response = reqwest::get(asset_url).await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "Failed to fetch asset: HTTP {}",
            response.status()
        ));
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let data = response.bytes().await?;
    let asset = storage.create_asset(novel_id.clone(), asset_url.to_string(), content_type);
    let reader = Box::new(Cursor::new(data.to_vec()));

    // Store asset
    let asset_id = storage
        .store_asset(asset, reader)
        .await
        .map_err(|e| eyre::eyre!("Failed to store asset: {}", e))?;

    println!("Cover image saved");
    Ok(asset_id)
}
