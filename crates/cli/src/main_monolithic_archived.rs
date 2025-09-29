use std::path::PathBuf;

use clap::Parser;
use eyre::{Context, Result};
use quelle_storage::{
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};
use quelle_store::{StoreManager, registry::LocalRegistryStore};
use tracing::info;

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

    // Handle commands
    match cli.command {
        Commands::Fetch { command } => {
            handle_fetch_command(command, &mut store_manager, &storage, cli.dry_run).await
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
    _store_manager: &mut StoreManager,
    _storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        FetchCommands::Novel { url } => {
            if dry_run {
                println!("Would fetch novel from: {}", url);
                return Ok(());
            }

            info!("📖 Fetching novel from: {}", url);
            println!("🚧 Novel fetching is not yet fully implemented");
            println!("📖 URL: {}", url);

            // TODO: Implement actual fetching
            // 1. Find or install appropriate extension
            // 2. Use extension to fetch novel
            // 3. Store novel and cover in storage

            println!("💡 This would:");
            println!("  1. Find an extension that supports {}", url);
            println!("  2. Fetch the novel metadata");
            println!("  3. Download the cover image automatically");
            println!("  4. Store everything in your library");
        }

        FetchCommands::Chapter { url } => {
            if dry_run {
                println!("Would fetch chapter from: {}", url);
                return Ok(());
            }

            info!("📄 Fetching chapter from: {}", url);
            println!("🚧 Chapter fetching is not yet fully implemented");
            println!("📄 URL: {}", url);

            println!("💡 This would:");
            println!("  1. Find an extension that supports {}", url);
            println!("  2. Fetch the chapter content");
            println!("  3. Download any embedded images automatically");
            println!("  4. Store everything in your library");
        }

        FetchCommands::Chapters { novel_id } => {
            if dry_run {
                println!("Would fetch all chapters for: {}", novel_id);
                return Ok(());
            }

            info!("📚 Fetching all chapters for novel: {}", novel_id);
            println!("🚧 Bulk chapter fetching is not yet fully implemented");
            println!("📚 Novel ID: {}", novel_id);
        }

        FetchCommands::All { url } => {
            if dry_run {
                println!("Would fetch everything from: {}", url);
                return Ok(());
            }

            info!("🚀 Fetching everything from: {}", url);
            println!("🚧 Complete fetching is not yet fully implemented");
            println!("🚀 URL: {}", url);
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
        ExportCommands::Epub { novel_id, .. } => {
            if dry_run {
                println!("Would export to EPUB: {}", novel_id);
                return Ok(());
            }

            println!("🚧 EPUB export is not yet fully implemented");
            println!("📚 Novel ID: {}", novel_id);

            // Check if novel exists
            if novel_id != "all" {
                let id = NovelId::new(novel_id.clone());
                match storage.get_novel(&id).await? {
                    Some(novel) => {
                        println!("💡 Would export: {}", novel.title);
                        println!("  With cover image (if available)");
                        println!("  With all downloaded chapters");
                        println!(
                            "  To current directory as {}.epub",
                            sanitize_filename(&novel.title)
                        );
                    }
                    None => {
                        println!("❌ Novel not found: {}", id.as_str());
                    }
                }
            } else {
                let novels = storage.list_novels(&NovelFilter::default()).await?;
                println!("💡 Would export {} novels to EPUB", novels.len());
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

    println!("🚧 Novel search is not yet fully implemented");
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

    println!("\n💡 This would search across all installed extensions");
    println!("💡 To search for extensions instead, use:");
    println!("  quelle extension search {}", query);

    Ok(())
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
                    if !ext.source_store.is_empty() {
                        println!("     Source: {}", ext.source_store);
                    }
                    println!();
                } else {
                    println!("  📦 {} v{} - {}", ext.name, ext.version, ext.id);
                }
            }
        }

        ExtensionCommands::Update { id, .. } => {
            if dry_run {
                println!("Would update extension: {}", id);
                return Ok(());
            }

            if id == "all" {
                println!("🚧 Update all extensions is not yet implemented");
            } else {
                println!("🚧 Update extension is not yet implemented");
                println!("📦 Extension ID: {}", id);
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

        ExtensionCommands::Search { query, .. } => {
            if dry_run {
                println!("Would search for extensions: {}", query);
                return Ok(());
            }

            println!("🚧 Extension search is not yet fully implemented");
            println!("🔍 Query: {}", query);
            println!("💡 This would search across all configured extension stores");
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

                    if !ext.source_store.is_empty() {
                        println!("Source: {}", ext.source_store);
                    }

                    // Show manifest information if available
                    println!("\nManifest Information:");
                    println!("  Name: {}", ext.manifest.name);
                    println!("  Version: {}", ext.manifest.version);
                    println!("  Author: {}", ext.manifest.author);

                    if !ext.manifest.langs.is_empty() {
                        println!("  Languages: {}", ext.manifest.langs.join(", "));
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

async fn handle_config_command(cmd: ConfigCommands, dry_run: bool) -> Result<()> {
    match cmd {
        ConfigCommands::Set { key, value } => {
            if dry_run {
                println!("Would set config: {} = {}", key, value);
            } else {
                println!("🚧 Configuration management is not yet implemented");
                println!("Would set: {} = {}", key, value);
            }
        }

        ConfigCommands::Get { key } => {
            println!("🚧 Configuration management is not yet implemented");
            println!("Would get: {}", key);
        }

        ConfigCommands::Show => {
            println!("🚧 Configuration management is not yet implemented");
            println!("Current configuration would be shown here");
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

            println!("🚧 Configuration management is not yet implemented");
            println!("Configuration would be reset to defaults");
        }
    }

    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '_',
            c => c,
        })
        .collect()
}
