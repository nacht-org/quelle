use std::path::PathBuf;

use clap::Parser;
use eyre::{Context, Result};
use quelle_engine::ExtensionEngine;
use quelle_export::epub::EpubExporter;
use quelle_storage::{
    ChapterContent, Novel,
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{Asset, AssetId, NovelFilter, NovelId},
};
use quelle_store::{SearchQuery, StoreManager, registry::LocalRegistryStore};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tracing::{error, info, warn};
use uuid;

mod cli;

use cli::{
    Cli, Commands, ConfigCommands, ExportCommands, ExtensionCommands, FetchCommands,
    LibraryCommands,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else if cli.quiet {
            tracing::Level::ERROR
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Initialize storage
    let storage_path = cli
        .storage_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".quelle")
        });

    let storage = FilesystemStorage::new(&storage_path);
    storage.initialize().await?;

    // Initialize store manager
    let registry_dir = storage_path.join("extensions");
    let registry_store = Box::new(LocalRegistryStore::new(&registry_dir).await?);
    let mut store_manager = StoreManager::new(registry_store)
        .await
        .context("Failed to initialize store manager")?;

    // Initialize engine
    use quelle_engine::http::HeadlessChromeExecutor;
    use std::sync::Arc;
    let executor = Arc::new(HeadlessChromeExecutor::new());
    let engine = ExtensionEngine::new(executor)?;

    // Handle commands
    match cli.command {
        Commands::Fetch { command } => {
            handle_fetch_command(command, &mut store_manager, &storage, &engine, cli.dry_run).await
        }
        Commands::Library { command } => {
            handle_library_command(command, &storage, cli.dry_run).await
        }
        Commands::Export { command } => handle_export_command(command, &storage, cli.dry_run).await,
        Commands::Search {
            query,
            author,
            tags,
            source,
            limit,
        } => handle_search_command(query, author, tags, source, limit, cli.dry_run).await,
        Commands::Extension { command } => {
            handle_extension_command(command, &mut store_manager, cli.dry_run).await
        }
        Commands::Config { command } => handle_config_command(command, cli.dry_run).await,
    }
}

async fn handle_fetch_command(
    cmd: FetchCommands,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
    engine: &ExtensionEngine,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        FetchCommands::Novel { url } => {
            if dry_run {
                println!("Would fetch novel from: {}", url);
                return Ok(());
            }

            info!("📖 Fetching novel from: {}", url);

            // Find and install extension for this URL
            let extension =
                find_and_install_extension_for_url(&url.to_string(), store_manager).await?;

            // Fetch novel using extension
            let novel = fetch_novel_with_extension(&extension, &url.to_string(), engine).await?;

            // Fetch cover if available
            if let Some(cover_url) = &novel.cover {
                info!("📷 Fetching cover image from: {}", cover_url);
                match fetch_and_store_asset(&novel.id, cover_url, storage).await {
                    Ok(_) => info!("✅ Cover image fetched successfully"),
                    Err(e) => warn!("⚠️ Failed to fetch cover image: {}", e),
                }
            }

            // Store novel
            let novel_id = storage.store_novel(&novel).await?;
            println!("✅ Novel stored with ID: {}", novel_id.as_str());
            println!("  Title: {}", novel.title);
            println!("  Authors: {}", novel.authors.join(", "));
            if !novel.description.is_empty() {
                println!("  Description: {}", novel.description.join(" "));
            }
        }

        FetchCommands::Chapter { url } => {
            if dry_run {
                println!("Would fetch chapter from: {}", url);
                return Ok(());
            }

            info!("📄 Fetching chapter from: {}", url);

            // Find extension for this URL
            let extension =
                find_and_install_extension_for_url(&url.to_string(), store_manager).await?;

            // Fetch chapter using extension
            let chapter =
                fetch_chapter_with_extension(&extension, &url.to_string(), engine).await?;

            // Store chapter content
            let content = ChapterContent {
                data: chapter.content,
            };

            // TODO: Parse content for embedded images and fetch them
            // This would scan HTML/markdown for <img> tags and download assets

            // Find the chapter in storage and store content
            // For now, we'll need to manually specify volume_index
            storage
                .store_chapter_content(&chapter.novel_id, 0, &url.to_string(), &content)
                .await?;

            println!("✅ Chapter stored: {}", chapter.title);
        }

        FetchCommands::Chapters { novel_id } => {
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
                    match fetch_chapter_with_extension(
                        &extension,
                        &chapter_info.chapter_url,
                        engine,
                    )
                    .await
                    {
                        Ok(chapter) => {
                            let content = ChapterContent {
                                data: chapter.content,
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
                                    error!(
                                        "  ❌ Failed to store {}: {}",
                                        chapter_info.chapter_title, e
                                    );
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
        }

        FetchCommands::All { url } => {
            if dry_run {
                println!("Would fetch everything from: {}", url);
                return Ok(());
            }

            info!("🚀 Fetching everything from: {}", url);

            // First fetch the novel
            let extension =
                find_and_install_extension_for_url(&url.to_string(), store_manager).await?;
            let novel = fetch_novel_with_extension(&extension, &url.to_string(), engine).await?;

            // Fetch cover
            if let Some(cover_url) = &novel.cover {
                match fetch_and_store_asset(&novel.id, cover_url, storage).await {
                    Ok(_) => info!("✅ Cover image fetched"),
                    Err(e) => warn!("⚠️ Failed to fetch cover: {}", e),
                }
            }

            let novel_id = storage.store_novel(&novel).await?;
            println!("✅ Novel stored: {}", novel.title);

            // Then fetch all chapters
            let chapters = storage.list_chapters(&novel_id).await?;
            let mut success_count = 0;
            let mut failed_count = 0;

            for chapter_info in chapters {
                info!("📄 Fetching chapter: {}", chapter_info.chapter_title);
                match fetch_chapter_with_extension(&extension, &chapter_info.chapter_url, engine)
                    .await
                {
                    Ok(chapter) => {
                        let content = ChapterContent {
                            data: chapter.content,
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
                                error!(
                                    "  ❌ Failed to store {}: {}",
                                    chapter_info.chapter_title, e
                                );
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
        }
    }
    Ok(())
}

async fn handle_library_command(
    cmd: LibraryCommands,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        LibraryCommands::List { source } => {
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
                    println!("     ID: {}", novel.id);
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
        }

        LibraryCommands::Show { novel_id } => {
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
        }

        LibraryCommands::Chapters {
            novel_id,
            downloaded_only,
        } => {
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
        }

        LibraryCommands::Read { novel_id, chapter } => {
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
        }

        LibraryCommands::Sync { novel_id } => {
            if dry_run {
                println!("Would sync: {}", novel_id);
                return Ok(());
            }

            if novel_id == "all" {
                println!("🚧 Sync all novels is not yet implemented");
            } else {
                println!("🚧 Sync novel is not yet implemented");
                println!("📚 Novel ID: {}", novel_id);
            }
        }

        LibraryCommands::Update { novel_id } => {
            if dry_run {
                println!("Would update: {}", novel_id);
                return Ok(());
            }

            if novel_id == "all" {
                println!("🚧 Update all novels is not yet implemented");
            } else {
                println!("🚧 Update novel is not yet implemented");
                println!("📚 Novel ID: {}", novel_id);
            }
        }

        LibraryCommands::Remove { novel_id, force } => {
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
        }

        LibraryCommands::Cleanup => {
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
        }

        LibraryCommands::Stats => {
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
        }
    }

    Ok(())
}

async fn handle_export_command(
    cmd: ExportCommands,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        ExportCommands::Epub {
            novel_id,
            chapters,
            output,
            template: _,
            combine_volumes: _,
            updated,
        } => {
            if dry_run {
                println!("Would export to EPUB: {}", novel_id);
                return Ok(());
            }

            let output_dir = output
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

            if novel_id == "all" {
                println!("📚 Exporting all novels to EPUB...");
                let novels = storage.list_novels(&NovelFilter::default()).await?;

                if novels.is_empty() {
                    println!("📚 No novels found in library");
                    return Ok(());
                }

                for novel in novels {
                    if updated {
                        // TODO: Check if novel was updated since last export
                        // For now, skip this check
                    }

                    println!("📖 Exporting {}...", novel.title);
                    match export_novel_to_epub(&novel.id, storage, &output_dir, chapters.as_deref())
                        .await
                    {
                        Ok(path) => println!("  ✅ Exported to: {}", path.display()),
                        Err(e) => {
                            error!("  ❌ Failed to export {}: {}", novel.title, e);
                        }
                    }
                }
            } else {
                let id = NovelId::new(novel_id);
                match storage.get_novel(&id).await? {
                    Some(novel) => {
                        println!("📖 Exporting {} to EPUB...", novel.title);
                        let path =
                            export_novel_to_epub(&id, storage, &output_dir, chapters.as_deref())
                                .await?;
                        println!("✅ Exported to: {}", path.display());
                    }
                    None => {
                        println!("❌ Novel not found: {}", id.as_str());
                    }
                }
            }
        }
        ExportCommands::Pdf { novel_id, .. } => {
            if dry_run {
                println!("Would export to PDF: {}", novel_id);
            } else {
                println!("🚧 PDF export is not yet implemented");
                println!("📄 Novel ID: {}", novel_id);
            }
        }
        ExportCommands::Html { novel_id, .. } => {
            if dry_run {
                println!("Would export to HTML: {}", novel_id);
            } else {
                println!("🚧 HTML export is not yet implemented");
                println!("🌐 Novel ID: {}", novel_id);
            }
        }
        ExportCommands::Txt { novel_id, .. } => {
            if dry_run {
                println!("Would export to TXT: {}", novel_id);
            } else {
                println!("🚧 TXT export is not yet implemented");
                println!("📝 Novel ID: {}", novel_id);
            }
        }
    }
    Ok(())
}

async fn handle_search_command(
    query: String,
    author: Option<String>,
    tags: Option<String>,
    source: Option<String>,
    limit: usize,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!(
            "Would search for: {} (author: {:?}, tags: {:?}, source: {:?})",
            query, author, tags, source
        );
        return Ok(());
    }

    println!("🔍 Searching for novels: {}", query);

    // Build search query
    let mut search_query = SearchQuery::new().with_text(query.clone());

    if let Some(author) = &author {
        search_query = search_query.with_author(author.clone());
    }

    if let Some(tags) = &tags {
        let tag_list: Vec<String> = tags.split(',').map(|s| s.trim().to_string()).collect();
        search_query = search_query.with_tags(tag_list);
    }

    search_query = search_query.limit(limit);

    // For now, we need a store manager to search
    // In a real implementation, this would be passed in or created here
    println!("⚠️ Novel search across extensions is not yet fully implemented");
    println!("💡 This would search across all installed extensions that support novel search");

    // Show what we would search for
    println!("🔍 Search parameters:");
    println!("  Query: {}", query);
    if let Some(author) = author {
        println!("  Author: {}", author);
    }
    if let Some(tags) = tags {
        println!("  Tags: {}", tags);
    }
    if let Some(source) = source {
        println!("  Source filter: {}", source);
    }
    println!("  Limit: {}", limit);

    println!("\n💡 To search for extensions instead, use:");
    println!("  quelle extension search {}", query);

    Ok(())
}

async fn export_novel_to_epub(
    novel_id: &NovelId,
    storage: &FilesystemStorage,
    output_dir: &PathBuf,
    chapters_filter: Option<&str>,
) -> Result<PathBuf> {
    let novel = storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| eyre::eyre!("Novel not found: {}", novel_id.as_str()))?;

    let exporter = EpubExporter::new();
    let output_path = output_dir.join(format!("{}.epub", sanitize_filename(&novel.title)));

    // For now, export all chapters. In the future, we can parse chapters_filter
    // to support ranges like "1-10" or "1,3,5-10"
    if let Some(_filter) = chapters_filter {
        info!("Chapter filtering not yet implemented, exporting all chapters");
    }

    exporter
        .export_novel(novel_id, storage, &output_path)
        .await
        .context("Failed to export novel to EPUB")?;

    Ok(output_path)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '_',
            c => c,
        })
        .collect()
}

// Helper functions

async fn find_and_install_extension_for_url(
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

            // Check supported sites in manifest
            for site in &ext.manifest.supported_sites {
                if domain.contains(site) || site.contains(domain) {
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

            return Err(eyre::eyre!(
                "No suitable extension found or installed for {}.\n\
                 Available extensions: {}\n\
                 💡 Try installing one manually with: quelle extension install <id>",
                domain,
                results.iter().map(|r| &r.id).collect::<Vec<_>>().join(", ")
            ));
        }
        Err(e) => {
            return Err(eyre::eyre!(
                "Failed to search for extensions: {}\n\
                 💡 Try installing a compatible extension manually",
                e
            ));
        }
    }
}

async fn fetch_novel_with_extension(
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

async fn fetch_chapter_with_extension(
    extension: &quelle_store::models::InstalledExtension,
    url: &str,
    engine: &ExtensionEngine,
) -> Result<quelle_engine::bindings::quelle::extension::novel::Chapter> {
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

async fn handle_extension_command(
    cmd: ExtensionCommands,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        ExtensionCommands::Install { id, version, force } => {
            if dry_run {
                println!("Would install extension: {} (version: {:?})", id, version);
                return Ok(());
            }

            println!("📦 Installing extension: {}", id);

            // Check if already installed
            if !force {
                if let Some(installed) = store_manager.get_installed(&id).await? {
                    println!(
                        "⚠️ Extension {} v{} is already installed",
                        installed.name, installed.version
                    );
                    println!("💡 Use --force to reinstall");
                    return Ok(());
                }
            }

            // Install the extension
            match store_manager.install(&id, version.as_deref(), None).await {
                Ok(installed) => {
                    println!("✅ Installed {} v{}", installed.name, installed.version);
                }
                Err(e) => {
                    eprintln!("❌ Failed to install {}: {}", id, e);
                    return Err(e.into());
                }
            }
        }

        ExtensionCommands::List { detailed } => {
            let installed = store_manager.list_installed().await?;

            if installed.is_empty() {
                println!("📦 No extensions installed");
                println!("💡 Use 'quelle extension search <query>' to find extensions");
                return Ok(());
            }

            println!("📦 Installed extensions ({}):", installed.len());
            for ext in installed {
                if detailed {
                    println!("  📦 {} v{}", ext.name, ext.version);
                    println!("     ID: {}", ext.id);
                    println!(
                        "     Installed: {}",
                        ext.installed_at.format("%Y-%m-%d %H:%M")
                    );
                    if let Some(source) = &ext.source_store {
                        println!("     Source: {}", source);
                    }
                    println!();
                } else {
                    println!("  📦 {} v{} - {}", ext.name, ext.version, ext.id);
                }
            }
        }

        ExtensionCommands::Update {
            id,
            prerelease,
            force,
        } => {
            if dry_run {
                println!("Would update extension: {}", id);
                return Ok(());
            }

            if id == "all" {
                println!("🔄 Updating all extensions...");
                let installed = store_manager.list_installed().await?;

                if installed.is_empty() {
                    println!("📦 No extensions installed");
                    return Ok(());
                }

                for ext in installed {
                    println!("🔄 Checking for updates: {}", ext.name);
                    // TODO: Implement update checking logic
                    // This would need to check available versions and compare
                    println!("  ℹ️ Update checking not yet implemented for {}", ext.name);
                }
            } else {
                match store_manager.get_installed(&id).await? {
                    Some(installed) => {
                        println!("🔄 Checking for updates: {}", installed.name);
                        // TODO: Implement update logic
                        println!("  ℹ️ Update checking not yet implemented");
                        println!("  Current version: {}", installed.version);
                        if prerelease {
                            println!("  Would include pre-release versions");
                        }
                        if force {
                            println!("  Would force update even if no new version");
                        }
                    }
                    None => {
                        println!("❌ Extension not installed: {}", id);
                    }
                }
            }
        }

        ExtensionCommands::Remove { id, force } => {
            if dry_run {
                println!("Would remove extension: {}", id);
                return Ok(());
            }

            match store_manager.get_installed(&id).await? {
                Some(installed) => {
                    if !force {
                        print!(
                            "Are you sure you want to remove '{}'? (y/N): ",
                            installed.name
                        );
                        use std::io::{self, Write};
                        io::stdout().flush()?;
                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;
                        if !input.trim().to_lowercase().starts_with('y') {
                            println!("❌ Cancelled");
                            return Ok(());
                        }
                    }

                    match store_manager.uninstall(&id).await {
                        Ok(_) => {
                            println!("✅ Removed extension: {}", installed.name);
                        }
                        Err(e) => {
                            eprintln!("❌ Failed to remove {}: {}", installed.name, e);
                            return Err(e.into());
                        }
                    }
                }
                None => {
                    println!("❌ Extension not installed: {}", id);
                }
            }
        }

        ExtensionCommands::Search {
            query,
            author,
            limit,
        } => {
            if dry_run {
                println!("Would search for extensions: {}", query);
                return Ok(());
            }

            println!("🔍 Searching for extensions: {}", query);

            // Build search query
            let mut search_query = SearchQuery::new().with_text(query);

            if let Some(author) = author {
                search_query = search_query.with_author(author);
            }

            search_query = search_query.limit(limit);

            // Search across all extension stores
            match store_manager.search_all_stores(&search_query).await {
                Ok(results) => {
                    if results.is_empty() {
                        println!(
                            "❌ No extensions found matching '{}'",
                            search_query.text().unwrap_or("")
                        );
                        println!(
                            "💡 Try adding more extension stores or using different search terms"
                        );
                    } else {
                        println!("📦 Found {} extension(s):", results.len());
                        for (i, result) in results.iter().enumerate() {
                            println!(
                                "{}. {} v{} by {}",
                                i + 1,
                                result.name,
                                result.version,
                                result.author
                            );
                            if let Some(desc) = &result.description {
                                let short_desc = if desc.len() > 100 {
                                    format!("{}...", &desc[..97])
                                } else {
                                    desc.clone()
                                };
                                println!("   {}", short_desc);
                            }
                            println!("   ID: {} (from {})", result.id, result.store_source);
                            println!();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Search failed: {}", e);
                    return Err(e.into());
                }
            }
        }

        ExtensionCommands::Info { id } => {
            match store_manager.get_installed(&id).await? {
                Some(ext) => {
                    println!("📦 {}", ext.name);
                    println!("ID: {}", ext.id);
                    println!("Version: {}", ext.version);
                    println!(
                        "Installed: {}",
                        ext.installed_at.format("%Y-%m-%d %H:%M:%S")
                    );

                    if let Some(source) = &ext.source_store {
                        println!("Source: {}", source);
                    }

                    // Show manifest information if available
                    println!("\nManifest Information:");
                    println!("  Name: {}", ext.manifest.name);
                    println!("  Version: {}", ext.manifest.version);

                    if !ext.manifest.authors.is_empty() {
                        println!("  Authors: {}", ext.manifest.authors.join(", "));
                    }

                    if !ext.manifest.description.is_empty() {
                        println!("  Description: {}", ext.manifest.description);
                    }

                    if !ext.manifest.homepage.is_empty() {
                        println!("  Homepage: {}", ext.manifest.homepage);
                    }

                    if !ext.manifest.repository.is_empty() {
                        println!("  Repository: {}", ext.manifest.repository);
                    }

                    if !ext.manifest.keywords.is_empty() {
                        println!("  Keywords: {}", ext.manifest.keywords.join(", "));
                    }

                    // Show capabilities
                    if !ext.manifest.capabilities.is_empty() {
                        println!("  Capabilities: {}", ext.manifest.capabilities.join(", "));
                    }

                    // Show supported sites if available
                    if !ext.manifest.supported_sites.is_empty() {
                        println!("  Supported Sites:");
                        for site in &ext.manifest.supported_sites {
                            println!("    - {}", site);
                        }
                    }
                }
                None => {
                    println!("❌ Extension not installed: {}", id);
                    println!("💡 Use 'quelle extension search {}' to find it", id);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    pub storage: StorageConfig,
    pub export: ExportConfig,
    pub fetch: FetchConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StorageConfig {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExportConfig {
    pub format: String,
    pub include_covers: bool,
    pub output_dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FetchConfig {
    pub auto_fetch_covers: bool,
    pub auto_fetch_assets: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage: StorageConfig {
                path: dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".quelle")
                    .to_string_lossy()
                    .to_string(),
            },
            export: ExportConfig {
                format: "epub".to_string(),
                include_covers: true,
                output_dir: None,
            },
            fetch: FetchConfig {
                auto_fetch_covers: true,
                auto_fetch_assets: true,
            },
        }
    }
}

fn get_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("quelle")
        .join("config.json")
}

async fn load_config() -> Result<Config> {
    let config_path = get_config_path();

    if config_path.exists() {
        let content = tokio::fs::read_to_string(&config_path).await?;
        let config: Config =
            serde_json::from_str(&content).context("Failed to parse configuration file")?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

async fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path();

    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let content = serde_json::to_string_pretty(config)?;
    tokio::fs::write(&config_path, content).await?;

    Ok(())
}

fn set_config_value(config: &mut Config, key: &str, value: &str) -> Result<()> {
    match key {
        "storage.path" => config.storage.path = value.to_string(),
        "export.format" => config.export.format = value.to_string(),
        "export.include-covers" | "export.include_covers" => {
            config.export.include_covers = value.parse().context("Invalid boolean value")?;
        }
        "export.output-dir" | "export.output_dir" => {
            config.export.output_dir = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        "fetch.auto-fetch-covers" | "fetch.auto_fetch_covers" => {
            config.fetch.auto_fetch_covers = value.parse().context("Invalid boolean value")?;
        }
        "fetch.auto-fetch-assets" | "fetch.auto_fetch_assets" => {
            config.fetch.auto_fetch_assets = value.parse().context("Invalid boolean value")?;
        }
        _ => return Err(eyre::eyre!("Unknown configuration key: {}", key)),
    }
    Ok(())
}

fn get_config_value(config: &Config, key: &str) -> Option<String> {
    match key {
        "storage.path" => Some(config.storage.path.clone()),
        "export.format" => Some(config.export.format.clone()),
        "export.include-covers" | "export.include_covers" => {
            Some(config.export.include_covers.to_string())
        }
        "export.output-dir" | "export.output_dir" => config.export.output_dir.clone(),
        "fetch.auto-fetch-covers" | "fetch.auto_fetch_covers" => {
            Some(config.fetch.auto_fetch_covers.to_string())
        }
        "fetch.auto-fetch-assets" | "fetch.auto_fetch_assets" => {
            Some(config.fetch.auto_fetch_assets.to_string())
        }
        _ => None,
    }
}

async fn handle_config_command(cmd: ConfigCommands, dry_run: bool) -> Result<()> {
    match cmd {
        ConfigCommands::Set { key, value } => {
            if dry_run {
                println!("Would set config: {} = {}", key, value);
                return Ok(());
            }

            let mut config = load_config().await?;

            match set_config_value(&mut config, &key, &value) {
                Ok(_) => {
                    save_config(&config).await?;
                    println!("✅ Set {} = {}", key, value);
                }
                Err(e) => {
                    eprintln!("❌ Failed to set {}: {}", key, e);
                    return Err(e);
                }
            }
        }

        ConfigCommands::Get { key } => {
            let config = load_config().await?;

            match get_config_value(&config, &key) {
                Some(value) => println!("{} = {}", key, value),
                None => {
                    println!("❌ Unknown configuration key: {}", key);
                    println!("💡 Use 'quelle config show' to see all available keys");
                }
            }
        }

        ConfigCommands::Show => {
            let config = load_config().await?;

            println!("📋 Current Configuration:");
            println!();

            println!("Storage:");
            println!("  storage.path = {}", config.storage.path);
            println!();

            println!("Export:");
            println!("  export.format = {}", config.export.format);
            println!("  export.include-covers = {}", config.export.include_covers);
            if let Some(ref output_dir) = config.export.output_dir {
                println!("  export.output-dir = {}", output_dir);
            } else {
                println!("  export.output-dir = (not set)");
            }
            println!();

            println!("Fetch:");
            println!(
                "  fetch.auto-fetch-covers = {}",
                config.fetch.auto_fetch_covers
            );
            println!(
                "  fetch.auto-fetch-assets = {}",
                config.fetch.auto_fetch_assets
            );
            println!();

            println!("Configuration file: {}", get_config_path().display());
        }

        ConfigCommands::Reset { force } => {
            if dry_run {
                println!("Would reset configuration to defaults");
                return Ok(());
            }

            if !force {
                print!("Are you sure you want to reset all configuration? (y/N): ");
                use std::io::{self, Write};
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("❌ Cancelled");
                    return Ok(());
                }
            }

            let default_config = Config::default();
            save_config(&default_config).await?;
            println!("✅ Configuration reset to defaults");
        }
    }

    Ok(())
}
