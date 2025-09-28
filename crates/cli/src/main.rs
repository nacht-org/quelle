mod cli;
mod store_commands;

use std::path::PathBuf;

use clap::Parser;

use quelle_engine::ExtensionEngine;
use quelle_store::{ConfigStore, LocalConfigStore, LocalRegistryStore, SearchQuery, StoreManager};
use storage::{BookStorage, FilesystemStorage, NovelFilter};

use crate::cli::{Commands, FetchCommands, LibraryCommands};
use crate::store_commands::{handle_extension_command, handle_store_command};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = cli::Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize config store for persistence (using ./local for easier testing)
    let config_dir = PathBuf::from("./data");
    let config_file = config_dir.join("config.json");
    let config_store = LocalConfigStore::new(config_file).await?;

    // Initialize store manager
    let registry_dir = PathBuf::from("./data/registry");
    let registry_store = Box::new(LocalRegistryStore::new(registry_dir).await?);
    let mut store_manager = StoreManager::new(registry_store).await?;

    // Initialize storage for local library
    let storage_dir = PathBuf::from("./data/library");
    let storage = FilesystemStorage::new(&storage_dir);
    storage.initialize().await?;

    // Load configuration and apply to registry
    let config = config_store.load().await?;
    config.apply(&mut store_manager).await?;

    match cli.command {
        Commands::Fetch { command } => {
            handle_fetch_command(command, &mut store_manager, &storage).await?;
        }
        Commands::Search {
            query,
            author,
            tags,
            categories,
            limit,
        } => {
            handle_search_command(&store_manager, query, author, tags, categories, limit).await?;
        }
        Commands::List => {
            handle_list_command(&store_manager).await?;
        }
        Commands::Status => {
            handle_status_command(&store_manager).await?;
        }
        Commands::Store { command } => {
            handle_store_command(command, &config, &mut store_manager, &config_store).await?;
        }
        Commands::Library { command } => {
            handle_library_command(command, &storage).await?;
        }
        Commands::Extension { command } => {
            handle_extension_command(command, &config, &mut store_manager).await?;
        }
    }

    Ok(())
}

async fn handle_fetch_command(
    cmd: FetchCommands,
    store_manager: &mut StoreManager,
    storage: &FilesystemStorage,
) -> eyre::Result<()> {
    match cmd {
        FetchCommands::Novel { url } => {
            // Find extension that can handle this URL
            match find_extension_for_url(&url.to_string(), store_manager).await? {
                Some((extension_id, _store_name)) => {
                    println!("Found extension with ID: {}", extension_id);

                    // Install extension if not already installed
                    if store_manager.get_installed(&extension_id).await?.is_none() {
                        println!("Installing extension {}...", extension_id);
                        match store_manager.install(&extension_id, None, None).await {
                            Ok(installed) => {
                                println!(
                                    "✅ Installed {} ({}) v{}",
                                    installed.name, installed.id, installed.version
                                );
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to install {}: {}", extension_id, e);
                                return Err(e.into());
                            }
                        }
                    }

                    // Use the installed extension to fetch novel info
                    println!("📖 Fetching novel info from: {}", url);

                    if let Some(installed) = store_manager.get_installed(&extension_id).await? {
                        match fetch_novel_with_extension(&installed, &url.to_string()).await {
                            Ok(novel) => {
                                println!("✅ Successfully fetched novel information:");
                                println!("  Title: {}", novel.title);
                                println!("  Authors: {}", novel.authors.join(", "));
                                if !novel.description.is_empty() {
                                    println!("  Description: {}", novel.description.join(" "));
                                }
                                if let Some(cover) = &novel.cover {
                                    println!("  Cover URL: {}", cover);
                                }
                                let total_chapters: u32 =
                                    novel.volumes.iter().map(|v| v.chapters.len() as u32).sum();
                                println!("  Total chapters: {}", total_chapters);
                                println!("  Status: {:?}", novel.status);

                                // Save to local storage
                                match storage.store_novel(&novel).await {
                                    Ok(novel_id) => {
                                        println!("💾 Saved to local library with ID: {}", novel_id);
                                    }
                                    Err(e) => {
                                        eprintln!("⚠️  Failed to save to library: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to fetch novel info: {}", e);
                                return Err(e.into());
                            }
                        }
                    } else {
                        eprintln!("❌ Extension {} not found in registry", extension_id);
                    }
                }
                None => {
                    eprintln!("❌ No extension found that can handle URL: {}", url);
                    eprintln!("Try adding more extension stores with: quelle store add");
                }
            }
        }
        FetchCommands::Chapter { url } => {
            // Find extension that can handle this URL
            match find_extension_for_url(&url.to_string(), store_manager).await? {
                Some((extension_id, _store_name)) => {
                    println!("Found extension with ID: {}", extension_id);

                    // Install extension if not already installed
                    if store_manager.get_installed(&extension_id).await?.is_none() {
                        println!("Installing extension {}...", extension_id);
                        match store_manager.install(&extension_id, None, None).await {
                            Ok(installed) => {
                                println!(
                                    "✅ Installed {} ({}) v{}",
                                    installed.name, installed.id, installed.version
                                );
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to install {}: {}", extension_id, e);
                                return Err(e.into());
                            }
                        }
                    }

                    // Use the installed extension to fetch chapter
                    println!("📄 Fetching chapter from: {}", url);

                    if let Some(installed) = store_manager.get_installed(&extension_id).await? {
                        match fetch_chapter_with_extension(&installed, &url.to_string()).await {
                            Ok(chapter) => {
                                println!("✅ Successfully fetched chapter:");
                                println!("  Content length: {} characters", chapter.data.len());

                                // Show first few lines of content
                                let preview = if chapter.data.len() > 200 {
                                    format!("{}...", &chapter.data[..200])
                                } else {
                                    chapter.data.clone()
                                };
                                println!("  Preview: {}", preview);

                                // Try to save chapter to storage if we can find the novel
                                if let Ok(Some(novel)) =
                                    storage.find_novel_by_url(&url.to_string()).await
                                {
                                    // Find the chapter in the novel structure
                                    for volume in novel.volumes.iter() {
                                        if let Some(_ch) = volume
                                            .chapters
                                            .iter()
                                            .find(|ch| ch.url == url.to_string())
                                        {
                                            // Find the novel ID from the library listing
                                            let filter =
                                                storage::NovelFilter { source_ids: vec![] };
                                            if let Ok(novels) = storage.list_novels(&filter).await {
                                                if let Some(novel_summary) =
                                                    novels.iter().find(|n| n.title == novel.title)
                                                {
                                                    match storage
                                                        .store_chapter_content(
                                                            &novel_summary.id,
                                                            volume.index,
                                                            &url.to_string(),
                                                            &chapter,
                                                        )
                                                        .await
                                                    {
                                                        Ok(()) => {
                                                            println!(
                                                                "💾 Saved chapter content to local library"
                                                            );
                                                        }
                                                        Err(e) => {
                                                            eprintln!(
                                                                "⚠️  Failed to save chapter: {}",
                                                                e
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                } else {
                                    println!(
                                        "ℹ️  Chapter not saved - novel not found in library. Fetch the novel first."
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to fetch chapter: {}", e);
                                return Err(e.into());
                            }
                        }
                    } else {
                        eprintln!("❌ Extension {} not found in registry", extension_id);
                    }
                }
                None => {
                    eprintln!("❌ No extension found that can handle URL: {}", url);
                    eprintln!("Try adding more extension stores with: quelle store add");
                }
            }
        }
    }
    Ok(())
}

async fn handle_search_command(
    store_manager: &StoreManager,
    query: String,
    author: Option<String>,
    tags: Vec<String>,
    categories: Vec<String>,
    limit: Option<usize>,
) -> eyre::Result<()> {
    // Determine if we should use simple or complex search
    let is_complex = !tags.is_empty() || !categories.is_empty() || author.is_some();

    if is_complex {
        println!("🔍 Using complex search...");
    } else {
        println!("🔍 Using simple search...");
    }

    // Build search query
    let mut search_query = SearchQuery::new().with_text(query.clone());

    if let Some(author) = author {
        search_query = search_query.with_author(author);
    }

    if !tags.is_empty() {
        search_query = search_query.with_tags(tags);
    }

    if let Some(limit) = limit {
        search_query = search_query.limit(limit);
    }

    // Search across all stores
    match store_manager.search_all_stores(&search_query).await {
        Ok(results) => {
            if results.is_empty() {
                println!("No results found for: {}", query);
            } else {
                println!("Found {} results:", results.len());
                for (i, result) in results.iter().enumerate().take(limit.unwrap_or(10)) {
                    println!("{}. {} by {}", i + 1, result.name, result.author);
                    if let Some(desc) = &result.description {
                        let short_desc = if desc.len() > 100 {
                            format!("{}...", &desc[..97])
                        } else {
                            desc.clone()
                        };
                        println!("   {}", short_desc);
                    }
                    println!("   Store: {}", result.store_source);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Search failed: {}", e);
        }
    }

    Ok(())
}

async fn handle_list_command(store_manager: &StoreManager) -> eyre::Result<()> {
    let stores = store_manager.list_extension_stores();
    if stores.is_empty() {
        println!("No extension stores available.");
        return Ok(());
    }

    println!("Available extension stores:");
    for store in stores {
        let info = store.config();
        println!("  📦 {} ({})", info.store_name, info.store_type);

        match store.store().list_extensions().await {
            Ok(extensions) => {
                if extensions.is_empty() {
                    println!("     No extensions found");
                } else {
                    for ext in extensions.iter().take(5) {
                        println!("     - {}", ext.name);
                    }
                    if extensions.len() > 5 {
                        println!("     ... and {} more", extensions.len() - 5);
                    }
                }
            }
            Err(e) => {
                println!("     Error listing extensions: {}", e);
            }
        }
    }
    Ok(())
}

async fn handle_status_command(store_manager: &StoreManager) -> eyre::Result<()> {
    let stores = store_manager.list_extension_stores();
    println!("Registry Status:");
    println!("  Configured stores: {}", stores.len());

    for store in stores {
        let info = store.config();
        print!("  {} ({}): ", info.store_name, info.store_type);

        match store.store().health_check().await {
            Ok(health) => {
                if health.healthy {
                    println!("✅ Healthy");
                    if let Some(count) = health.extension_count {
                        println!("    Extensions: {}", count);
                    }
                } else {
                    println!("❌ Unhealthy");
                    if let Some(error) = &health.error {
                        println!("    Error: {}", error);
                    }
                }
            }
            Err(e) => {
                println!("❌ Check failed: {}", e);
            }
        }
    }
    Ok(())
}

/// Find an extension that can handle the given URL
async fn find_extension_for_url(
    url: &str,
    store_manager: &StoreManager,
) -> eyre::Result<Option<(String, String)>> {
    store_manager
        .find_extension_for_url(url)
        .await
        .map_err(|e| eyre::eyre!("Failed to find extension for URL: {}", e))
}

/// Fetch novel information using an installed extension
async fn fetch_novel_with_extension(
    installed: &quelle_store::models::InstalledExtension,
    url: &str,
) -> eyre::Result<quelle_engine::bindings::quelle::extension::novel::Novel> {
    use quelle_engine::http::HeadlessChromeExecutor;
    use std::sync::Arc;

    // Create HTTP executor
    let executor = Arc::new(HeadlessChromeExecutor::new());

    // Create extension engine
    let engine = ExtensionEngine::new(executor)?;

    // Get WASM component bytes
    let wasm_bytes = installed.get_wasm_bytes();

    // Create runner and fetch novel info
    let runner = engine.new_runner_from_bytes(wasm_bytes).await?;
    let (_, result) = runner.fetch_novel_info(url).await?;

    match result {
        Ok(novel) => Ok(novel),
        Err(wit_error) => Err(eyre::eyre!("Extension error: {:?}", wit_error)),
    }
}

/// Fetch chapter content using an installed extension
async fn fetch_chapter_with_extension(
    installed: &quelle_store::models::InstalledExtension,
    url: &str,
) -> eyre::Result<quelle_engine::bindings::quelle::extension::novel::ChapterContent> {
    use quelle_engine::http::HeadlessChromeExecutor;
    use std::sync::Arc;

    // Create HTTP executor
    let executor = Arc::new(HeadlessChromeExecutor::new());

    // Create extension engine
    let engine = ExtensionEngine::new(executor)?;

    // Get WASM component bytes
    let wasm_bytes = installed.get_wasm_bytes();

    // Create runner and fetch chapter content
    let runner = engine.new_runner_from_bytes(wasm_bytes).await?;
    let (_, result) = runner.fetch_chapter(url).await?;

    match result {
        Ok(chapter) => Ok(chapter),
        Err(wit_error) => Err(eyre::eyre!("Extension error: {:?}", wit_error)),
    }
}

async fn handle_library_command(
    cmd: LibraryCommands,
    storage: &FilesystemStorage,
) -> eyre::Result<()> {
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
                println!("No novels found in library.");
                println!("Use 'quelle fetch novel <url>' to add novels to your library.");
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
            // Try to find novel by ID or URL
            let novel = if novel_id.starts_with("http") {
                storage.find_novel_by_url(&novel_id).await?
            } else {
                let id = storage::NovelId::new(novel_id.clone());
                storage.get_novel(&id).await?
            };

            match novel {
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

                    let total_chapters: u32 =
                        novel.volumes.iter().map(|v| v.chapters.len() as u32).sum();
                    println!("Total chapters: {}", total_chapters);

                    println!("\nVolumes:");
                    for volume in &novel.volumes {
                        println!("  📚 {} ({} chapters)", volume.name, volume.chapters.len());
                    }
                }
                None => {
                    println!("Novel not found: {}", novel_id);
                }
            }
        }
        LibraryCommands::Chapters {
            novel_id,
            downloaded_only,
        } => {
            // Try to find novel by ID or URL
            let (novel, novel_storage_id) = if novel_id.starts_with("http") {
                match storage.find_novel_by_url(&novel_id).await? {
                    Some(novel) => {
                        // Find the storage ID from the library listing
                        let filter = storage::NovelFilter { source_ids: vec![] };
                        let novels = storage.list_novels(&filter).await?;
                        let storage_id = novels
                            .iter()
                            .find(|n| n.title == novel.title)
                            .map(|n| n.id.clone())
                            .unwrap_or_else(|| storage::NovelId::new(novel_id.clone()));
                        (Some(novel), storage_id)
                    }
                    None => (None, storage::NovelId::new(novel_id.clone())),
                }
            } else {
                let id = storage::NovelId::new(novel_id.clone());
                (storage.get_novel(&id).await?, id)
            };

            match novel {
                Some(novel) => {
                    let id = novel_storage_id;
                    let chapters = storage.list_chapters(&id).await?;

                    let filtered_chapters: Vec<_> = if downloaded_only {
                        chapters.into_iter().filter(|ch| ch.has_content()).collect()
                    } else {
                        chapters
                    };

                    if filtered_chapters.is_empty() {
                        if downloaded_only {
                            println!("No downloaded chapters found for '{}'", novel.title);
                        } else {
                            println!("No chapters found for '{}'", novel.title);
                        }
                    } else {
                        println!(
                            "📄 Chapters for '{}' ({} shown):",
                            novel.title,
                            filtered_chapters.len()
                        );
                        for chapter in filtered_chapters {
                            match &chapter.content_status {
                                storage::ChapterContentStatus::NotStored => {
                                    println!(
                                        "  ❌ {}: {}",
                                        chapter.chapter_index, chapter.chapter_title
                                    );
                                }
                                storage::ChapterContentStatus::Stored {
                                    content_size,
                                    stored_at,
                                    ..
                                } => {
                                    println!(
                                        "  ✅ {}: {} ({} chars, {})",
                                        chapter.chapter_index,
                                        chapter.chapter_title,
                                        content_size,
                                        stored_at.format("%Y-%m-%d")
                                    );
                                }
                            }
                        }
                    }
                }
                None => {
                    println!("Novel not found: {}", novel_id);
                }
            }
        }
        LibraryCommands::Read { novel_id, chapter } => {
            // Try to find novel by ID or URL
            let (novel, novel_storage_id) = if novel_id.starts_with("http") {
                match storage.find_novel_by_url(&novel_id).await? {
                    Some(novel) => {
                        // Find the storage ID from the library listing
                        let filter = storage::NovelFilter { source_ids: vec![] };
                        let novels = storage.list_novels(&filter).await?;
                        let storage_id = novels
                            .iter()
                            .find(|n| n.title == novel.title)
                            .map(|n| n.id.clone())
                            .unwrap_or_else(|| storage::NovelId::new(novel_id.clone()));
                        (Some(novel), storage_id)
                    }
                    None => (None, storage::NovelId::new(novel_id.clone())),
                }
            } else {
                let id = storage::NovelId::new(novel_id.clone());
                (storage.get_novel(&id).await?, id)
            };

            match novel {
                Some(novel) => {
                    let id = novel_storage_id;

                    // Find the chapter - either by number or URL
                    let mut found_chapter = None;
                    let mut chapter_url = None;
                    let mut volume_index = 0;

                    if chapter.starts_with("http") {
                        // Search by URL
                        for volume in &novel.volumes {
                            if let Some(ch) = volume.chapters.iter().find(|ch| ch.url == chapter) {
                                found_chapter = Some(ch);
                                chapter_url = Some(ch.url.clone());
                                volume_index = volume.index;
                                break;
                            }
                        }
                    } else if let Ok(chapter_num) = chapter.parse::<i32>() {
                        // Search by chapter number
                        for volume in &novel.volumes {
                            if let Some(ch) =
                                volume.chapters.iter().find(|ch| ch.index == chapter_num)
                            {
                                found_chapter = Some(ch);
                                chapter_url = Some(ch.url.clone());
                                volume_index = volume.index;
                                break;
                            }
                        }
                    }

                    match (found_chapter, chapter_url) {
                        (Some(ch), Some(url)) => {
                            match storage.get_chapter_content(&id, volume_index, &url).await? {
                                Some(content) => {
                                    println!("📖 {} - {}", novel.title, ch.title);
                                    println!("═══════════════════════════════════════");
                                    println!();
                                    println!("{}", content.data);
                                }
                                None => {
                                    println!(
                                        "Chapter '{}' found but content not downloaded.",
                                        ch.title
                                    );
                                    println!("Use 'quelle fetch chapter {}' to download it.", url);
                                }
                            }
                        }
                        _ => {
                            println!("Chapter not found: {}", chapter);
                            println!(
                                "Use 'quelle library chapters {}' to see available chapters.",
                                novel_id
                            );
                        }
                    }
                }
                None => {
                    println!("Novel not found: {}", novel_id);
                }
            }
        }
        LibraryCommands::Remove { novel_id, force } => {
            // Try to find novel by ID or URL
            let (novel, novel_storage_id) = if novel_id.starts_with("http") {
                match storage.find_novel_by_url(&novel_id).await? {
                    Some(novel) => {
                        // Find the storage ID from the library listing
                        let filter = storage::NovelFilter { source_ids: vec![] };
                        let novels = storage.list_novels(&filter).await?;
                        let storage_id = novels
                            .iter()
                            .find(|n| n.title == novel.title)
                            .map(|n| n.id.clone())
                            .unwrap_or_else(|| storage::NovelId::new(novel_id.clone()));
                        (Some(novel), storage_id)
                    }
                    None => (None, storage::NovelId::new(novel_id.clone())),
                }
            } else {
                let id = storage::NovelId::new(novel_id.clone());
                (storage.get_novel(&id).await?, id)
            };

            match novel {
                Some(novel) => {
                    if !force {
                        println!(
                            "⚠️  This will permanently remove '{}' and all its chapters.",
                            novel.title
                        );
                        println!("Use --force to confirm removal.");
                        return Ok(());
                    }

                    let id = novel_storage_id;
                    match storage.delete_novel(&id).await? {
                        true => {
                            println!("✅ Removed '{}' from library.", novel.title);
                        }
                        false => {
                            println!("Novel not found in library: {}", novel_id);
                        }
                    }
                }
                None => {
                    println!("Novel not found: {}", novel_id);
                }
            }
        }
        LibraryCommands::Cleanup => {
            println!("🧹 Running library cleanup...");
            let report = storage.cleanup_dangling_data().await?;

            println!("✅ Cleanup completed:");
            if report.orphaned_chapters_removed > 0 {
                println!(
                    "  Removed {} orphaned chapters",
                    report.orphaned_chapters_removed
                );
            }
            if report.novels_fixed > 0 {
                println!("  Fixed {} novels", report.novels_fixed);
            }
            if report.errors_encountered.is_empty() {
                println!("  No errors encountered");
            } else {
                println!("  {} errors encountered:", report.errors_encountered.len());
                for error in &report.errors_encountered {
                    println!("    - {}", error);
                }
            }
        }
        LibraryCommands::Stats => {
            let filter = NovelFilter { source_ids: vec![] };
            let novels = storage.list_novels(&filter).await?;

            let mut total_chapters = 0;
            let mut downloaded_chapters = 0;

            for novel_summary in &novels {
                total_chapters += novel_summary.total_chapters;
                downloaded_chapters += novel_summary.stored_chapters;
            }

            println!("📊 Library Statistics:");
            println!("  Novels: {}", novels.len());
            println!("  Total chapters: {}", total_chapters);
            println!("  Downloaded chapters: {}", downloaded_chapters);
            if total_chapters > 0 {
                let percentage = (downloaded_chapters as f64 / total_chapters as f64) * 100.0;
                println!("  Download progress: {:.1}%", percentage);
            }
        }
    }
    Ok(())
}
