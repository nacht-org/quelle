use eyre::{Context, Result};
use quelle_engine::ExtensionEngine;
use quelle_storage::{
    ChapterContent, Novel,
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{Asset, AssetId, NovelId},
};
use quelle_store::{StoreManager, models::SearchQuery};
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
    // Initialize the extension engine with HTTP executor
    let http_executor = std::sync::Arc::new(quelle_engine::http::ReqwestExecutor::new());
    let engine = ExtensionEngine::new(http_executor)?;

    match cmd {
        FetchCommands::Novel { url } => {
            handle_fetch_novel(url, store_manager, storage, &engine, dry_run).await
        }
        FetchCommands::Chapter { url } => {
            handle_fetch_chapter(url, store_manager, storage, &engine, dry_run).await
        }
        FetchCommands::Chapters { novel_id } => {
            handle_fetch_chapters(novel_id, store_manager, storage, &engine, dry_run).await
        }
        FetchCommands::All { url } => {
            handle_fetch_all(url, store_manager, storage, &engine, dry_run).await
        }
    }
}

async fn handle_fetch_novel(
    url: Url,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    engine: &ExtensionEngine,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch novel from: {}", url);
        return Ok(());
    }

    info!("📖 Fetching novel from: {}", url);

    // Find and install extension for this URL
    let extension = find_and_install_extension_for_url(&url.to_string(), store_manager).await?;

    // Fetch novel using extension
    let novel = fetch_novel_with_extension(&extension, &url.to_string(), engine).await?;

    // Generate novel ID from URL
    let novel_id = NovelId::new(url.to_string());

    // Fetch cover if available
    if let Some(cover_url) = &novel.cover {
        info!("📷 Fetching cover image from: {}", cover_url);
        match fetch_and_store_asset(&novel_id, cover_url, storage).await {
            Ok(_) => info!("✅ Cover image fetched successfully"),
            Err(e) => warn!("⚠️ Failed to fetch cover image: {}", e),
        }
    }

    // Store novel
    let stored_novel_id = storage.store_novel(&novel).await?;
    println!("✅ Novel stored with ID: {}", stored_novel_id.as_str());
    println!("  Title: {}", novel.title);
    println!("  Authors: {}", novel.authors.join(", "));
    if !novel.description.is_empty() {
        println!("  Description: {}", novel.description.join(" "));
    }
    Ok(())
}

async fn handle_fetch_chapter(
    url: Url,
    store_manager: &mut StoreManager,
    _storage: &FilesystemStorage,
    engine: &ExtensionEngine,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch chapter from: {}", url);
        return Ok(());
    }

    info!("📄 Fetching chapter from: {}", url);

    // Find extension for this URL
    let extension = find_and_install_extension_for_url(&url.to_string(), store_manager).await?;

    // Fetch chapter using extension
    let chapter_content =
        fetch_chapter_with_extension(&extension, &url.to_string(), engine).await?;

    // Store chapter content
    let _content = ChapterContent {
        data: chapter_content.data,
    };

    // TODO: Parse content for embedded images and fetch them
    // This would scan HTML/markdown for <img> tags and download assets

    // We need to find which novel this chapter belongs to
    // For now, we'll assume the user needs to fetch the novel first
    println!("🚧 Chapter fetching requires novel to be fetched first");
    println!(
        "💡 Use 'quelle fetch novel {}' first, then fetch chapters",
        url
    );

    println!("✅ Chapter content fetched (ready for storage once novel is available)");
    Ok(())
}

async fn handle_fetch_chapters(
    novel_id: String,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    engine: &ExtensionEngine,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch all chapters for: {}", novel_id);
        return Ok(());
    }

    info!("📚 Fetching all chapters for novel: {}", novel_id);

    let novel_id = NovelId::new(novel_id);
    let novel = storage
        .get_novel(&novel_id)
        .await?
        .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

    let extension = find_and_install_extension_for_url(&novel.url, store_manager).await?;

    let chapters = storage.list_chapters(&novel_id).await?;
    let mut success_count = 0;
    let mut failed_count = 0;

    for chapter_info in chapters {
        if !chapter_info.has_content() {
            info!("📄 Fetching chapter: {}", chapter_info.chapter_title);
            match fetch_chapter_with_extension(&extension, &chapter_info.chapter_url, engine).await
            {
                Ok(chapter_content) => {
                    let content = ChapterContent {
                        data: chapter_content.data,
                    };
                    match storage
                        .store_chapter_content(
                            &novel_id,
                            chapter_info.volume_index,
                            &chapter_info.chapter_url,
                            &content,
                        )
                        .await
                    {
                        Ok(_) => {
                            println!("  ✅ {}", chapter_info.chapter_title);
                            success_count += 1;
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
        } else {
            println!("  ⏭️ {} (already downloaded)", chapter_info.chapter_title);
        }
    }

    println!(
        "📊 Fetch complete: {} successful, {} failed",
        success_count, failed_count
    );
    Ok(())
}

async fn handle_fetch_all(
    url: Url,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    engine: &ExtensionEngine,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would fetch everything from: {}", url);
        return Ok(());
    }

    info!("🚀 Fetching everything from: {}", url);

    // First fetch the novel
    let extension = find_and_install_extension_for_url(&url.to_string(), store_manager).await?;
    let novel = fetch_novel_with_extension(&extension, &url.to_string(), engine).await?;

    // Generate novel ID from URL
    let novel_id = NovelId::new(url.to_string());

    // Fetch cover
    if let Some(cover_url) = &novel.cover {
        match fetch_and_store_asset(&novel_id, cover_url, storage).await {
            Ok(_) => info!("✅ Cover image fetched"),
            Err(e) => warn!("⚠️ Failed to fetch cover: {}", e),
        }
    }

    let stored_novel_id = storage.store_novel(&novel).await?;
    println!("✅ Novel stored: {}", novel.title);

    // Then fetch all chapters
    let chapters = storage.list_chapters(&stored_novel_id).await?;
    let mut success_count = 0;
    let mut failed_count = 0;

    for chapter_info in chapters {
        info!("📄 Fetching chapter: {}", chapter_info.chapter_title);
        match fetch_chapter_with_extension(&extension, &chapter_info.chapter_url, engine).await {
            Ok(chapter_content) => {
                let content = ChapterContent {
                    data: chapter_content.data,
                };
                match storage
                    .store_chapter_content(
                        &stored_novel_id,
                        chapter_info.volume_index,
                        &chapter_info.chapter_url,
                        &content,
                    )
                    .await
                {
                    Ok(_) => {
                        println!("  ✅ {}", chapter_info.chapter_title);
                        success_count += 1;
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

    println!(
        "🎉 Complete! Novel + {} chapters fetched ({} failed)",
        success_count, failed_count
    );
    Ok(())
}

pub async fn find_and_install_extension_for_url(
    url: &str,
    store_manager: &mut StoreManager,
) -> Result<quelle_store::models::InstalledExtension> {
    // First, try to find an installed extension that can handle this URL
    let installed = store_manager.list_installed().await?;

    for ext in &installed {
        // Check if this extension supports the URL's domain
        let url_parts: Vec<&str> = url.split('/').collect();
        if url_parts.len() >= 3 {
            let domain = url_parts[2];

            // Check supported sites in manifest by looking at base_urls
            for base_url in &ext.manifest.base_urls {
                if domain.contains(base_url) || base_url.contains(domain) {
                    info!("✅ Found installed extension: {} for {}", ext.name, domain);
                    return Ok(ext.clone());
                }
            }
        }
    }

    // If no installed extension found, we need to search for one
    warn!("❌ No installed extension found for URL: {}", url);

    // Extract domain for search
    let url_parts: Vec<&str> = url.split('/').collect();
    let domain = if url_parts.len() >= 3 {
        url_parts[2]
    } else {
        return Err(eyre::eyre!("Invalid URL format: {}", url));
    };

    println!("🔍 Searching for extension that supports: {}", domain);

    // Search for extensions that might support this domain
    let search_query = SearchQuery::new().with_text(domain.to_string()).limit(10);

    match store_manager.search_all_stores(&search_query).await {
        Ok(results) => {
            if results.is_empty() {
                return Err(eyre::eyre!(
                    "No extensions found for {}.\n\
                     💡 Try:\n\
                     1. Adding more extension stores\n\
                     2. Installing a compatible extension manually\n\
                     3. Checking if the URL is supported",
                    domain
                ));
            }

            // Try to find a good match
            for result in &results {
                if result.name.to_lowercase().contains(&domain.to_lowercase())
                    || result
                        .description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&domain.to_lowercase()))
                {
                    println!(
                        "📦 Found potential extension: {} - {}",
                        result.name, result.id
                    );
                    print!("Install this extension? (y/N): ");

                    use std::io::{self, Write};
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    if input.trim().to_lowercase().starts_with('y') {
                        match store_manager.install(&result.id, None, None).await {
                            Ok(installed) => {
                                println!("✅ Installed {}", installed.name);
                                return Ok(installed);
                            }
                            Err(e) => {
                                error!("❌ Failed to install {}: {}", result.name, e);
                            }
                        }
                    }
                }
            }

            let result_names: Vec<String> = results.iter().map(|r| r.id.clone()).collect();
            Err(eyre::eyre!(
                "No suitable extension found or installed for {}.\n\
                 Available extensions: {}\n\
                 💡 Try installing one manually with: quelle extension install <id>",
                domain,
                result_names.join(", ")
            ))
        }
        Err(e) => Err(eyre::eyre!(
            "Failed to search for extensions: {}\n\
             💡 Try installing a compatible extension manually",
            e
        )),
    }
}

pub async fn fetch_novel_with_extension(
    extension: &quelle_store::models::InstalledExtension,
    url: &str,
    engine: &ExtensionEngine,
) -> Result<Novel> {
    // Get WASM component bytes
    let wasm_bytes = extension.get_wasm_bytes();

    // Create runner and fetch novel info
    let runner = engine.new_runner_from_bytes(wasm_bytes).await?;
    let (_, result) = runner.fetch_novel_info(url).await?;

    match result {
        Ok(novel) => Ok(novel),
        Err(wit_error) => Err(eyre::eyre!("Extension error: {:?}", wit_error)),
    }
}

pub async fn fetch_chapter_with_extension(
    extension: &quelle_store::models::InstalledExtension,
    url: &str,
    engine: &ExtensionEngine,
) -> Result<ChapterContent> {
    // Get WASM component bytes
    let wasm_bytes = extension.get_wasm_bytes();

    // Create runner and fetch chapter content
    let runner = engine.new_runner_from_bytes(wasm_bytes).await?;
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
    info!("📷 Fetching asset from: {}", asset_url);

    // Make HTTP request to fetch the asset
    let response = reqwest::get(asset_url).await?;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "Failed to fetch asset: HTTP {}",
            response.status()
        ));
    }

    // Get content type
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    // Get the asset data
    let data = response.bytes().await?;

    // Create asset metadata
    let asset = Asset {
        id: AssetId::from(format!("asset_{}", uuid::Uuid::new_v4())),
        novel_id: novel_id.clone(),
        original_url: asset_url.to_string(),
        mime_type: content_type,
        size: 0, // Will be updated by storage
    };

    // Create reader from data
    let reader = Box::new(Cursor::new(data.to_vec()));

    // Store asset
    storage
        .store_asset(asset, reader)
        .await
        .context("Failed to store asset")
}
