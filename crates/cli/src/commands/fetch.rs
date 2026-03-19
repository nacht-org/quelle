//! Fetch command handlers for retrieving novel and chapter data.

use eyre::Result;

use quelle_engine::ExtensionEngine;
use quelle_storage::{
    ChapterContent,
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{AssetId, NovelId},
};
use quelle_store::StoreManager;
use std::io::Cursor;
use tracing::{error, warn};
use url::Url;

use crate::cli::FetchCommands;
use crate::engine::create_extension_engine;

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
            let engine = create_extension_engine()?;
            handle_fetch_chapters(novel_id, None, store_manager, storage, &engine, dry_run).await
        }
        FetchCommands::All { url } => handle_fetch_all(url, store_manager, storage, dry_run).await,
    }
}

/// Handle the `add` command — fetch novel metadata then optionally fetch chapters.
pub async fn handle_add_command(
    url: Url,
    no_chapters: bool,
    max_chapters: Option<usize>,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would add novel from: {}", url);
        if !no_chapters {
            match max_chapters {
                Some(limit) => println!("Would fetch first {} chapters.", limit),
                None => println!("Would fetch all chapters."),
            }
        }
        return Ok(());
    }

    println!("Fetching novel...");
    handle_fetch_novel(url.clone(), store_manager, storage, false).await?;

    if !no_chapters {
        let engine = create_extension_engine()?;
        handle_fetch_chapters(
            url.to_string(),
            max_chapters,
            store_manager,
            storage,
            &engine,
            false,
        )
        .await?;
    }

    println!("Added.");
    Ok(())
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

    let engine = create_extension_engine()?;

    println!("Fetching novel...");
    match store_manager.fetch_novel(&engine, url.as_ref()).await {
        Ok(novel) => {
            println!("title: {}", novel.title);
            println!("authors: {}", novel.authors.join(", "));
            let total_chapters: u32 = novel.volumes.iter().map(|v| v.chapters.len() as u32).sum();
            println!("chapters: {}", total_chapters);

            match storage.store_novel(&novel).await {
                Ok(novel_id) => {
                    println!("Saved to library: {}", novel_id.as_str());

                    if let Some(cover_url) = &novel.cover {
                        match fetch_and_store_asset(&novel_id, cover_url, storage).await {
                            Ok(_) => {}
                            Err(e) => warn!("Failed to fetch cover: {}", e),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: Failed to save novel: {}", e);
                    return Err(e.into());
                }
            }
        }
        Err(e) => {
            eprintln!("Error: Failed to fetch novel: {}", e);
            return Err(e.into());
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

    let engine = create_extension_engine()?;
    let chapter = match store_manager.fetch_chapter(&engine, url.as_ref()).await {
        Ok(ch) => ch,
        Err(e) => {
            eprintln!("Error: Failed to fetch chapter: {}", e);
            return Err(e.into());
        }
    };

    println!("chapters: {} chars fetched", chapter.data.len());

    let novel = match storage.find_novel_by_url(url.as_ref()).await {
        Ok(Some(novel)) => novel,
        _ => {
            println!("Chapter not saved: novel not in library.");
            return Ok(());
        }
    };

    let mut saved = false;
    for volume in &novel.volumes {
        if volume.chapters.iter().any(|ch| ch.url == url.to_string()) {
            let filter = quelle_storage::types::NovelFilter { source_ids: vec![] };
            let novels = match storage.list_novels(&filter).await {
                Ok(novels) => novels,
                Err(e) => {
                    eprintln!("Error: Failed to list novels: {}", e);
                    break;
                }
            };
            if let Some(novel_summary) = novels.iter().find(|n| n.title == novel.title) {
                let content = ChapterContent {
                    data: chapter.data.clone(),
                };
                match storage
                    .store_chapter_content(&novel_summary.id, volume.index, url.as_ref(), &content)
                    .await
                {
                    Ok(_) => {
                        println!("Saved chapter content to library.");
                        saved = true;
                    }
                    Err(e) => {
                        eprintln!("Error: Failed to save chapter: {}", e);
                    }
                }
            }
            break;
        }
    }

    if !saved {
        println!("Chapter not saved: could not locate in novel structure.");
    }

    Ok(())
}

/// Fetch all (or up to `max_chapters`) chapters for a novel identified by ID or URL.
pub async fn handle_fetch_chapters(
    novel_id: String,
    max_chapters: Option<usize>,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    engine: &ExtensionEngine,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        match max_chapters {
            Some(n) => println!("Would fetch up to {} chapters for: {}", n, novel_id),
            None => println!("Would fetch all chapters for: {}", novel_id),
        }
        return Ok(());
    }

    // Resolve novel + its storage ID from either a URL or a direct novel ID.
    let (_novel, novel_storage_id) = if novel_id.starts_with("http") {
        let novel = match storage.find_novel_by_url(&novel_id).await? {
            Some(novel) => novel,
            None => {
                eprintln!("Not found: {}", novel_id);
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
                eprintln!("Not found: {}", novel_id);
                return Ok(());
            }
        };
        (novel, id)
    };

    let mut chapters = storage.list_chapters(&novel_storage_id).await?;
    let original_count = chapters.len();

    if let Some(limit) = max_chapters {
        if chapters.len() > limit {
            chapters.truncate(limit);
            println!("Fetching {} of {} chapters...", limit, original_count);
        } else {
            println!("Fetching {} chapters...", chapters.len());
        }
    } else {
        println!("Fetching {} chapters...", chapters.len());
    }

    let mut success_count = 0usize;
    let mut failed_count = 0usize;
    let mut skipped_count = 0usize;

    for chapter_info in chapters {
        if chapter_info.has_content() {
            skipped_count += 1;
            continue;
        }

        let chapter_content = match store_manager
            .fetch_chapter(engine, &chapter_info.chapter_url)
            .await
        {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to fetch '{}': {}", chapter_info.chapter_title, e);
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
            Ok(_) => {
                success_count += 1;
            }
            Err(e) => {
                error!("Failed to store '{}': {}", chapter_info.chapter_title, e);
                failed_count += 1;
            }
        }
    }

    println!(
        "fetched: {}, skipped: {}, failed: {}",
        success_count, skipped_count, failed_count
    );

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

    println!("Fetching novel and all chapters...");

    handle_fetch_novel(url.clone(), store_manager, storage, false).await?;

    let engine = create_extension_engine()?;
    handle_fetch_chapters(
        url.to_string(),
        None,
        store_manager,
        storage,
        &engine,
        false,
    )
    .await?;

    println!("Done.");
    Ok(())
}

async fn fetch_and_store_asset(
    novel_id: &NovelId,
    asset_url: &str,
    storage: &FilesystemStorage,
) -> Result<AssetId> {
    if let Some(existing_asset_id) = storage.find_asset_by_url(asset_url).await? {
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

    let asset_id = storage
        .store_asset(asset, reader)
        .await
        .map_err(|e| eyre::eyre!("Failed to store asset: {}", e))?;

    println!("Cover saved.");
    Ok(asset_id)
}
