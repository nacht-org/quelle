use clap::Parser;
use eyre::Result;
use quelle_storage::backends::filesystem::FilesystemStorage;
use quelle_store::{StoreManager, registry::LocalRegistryStore};
use std::path::PathBuf;

mod cli;
mod commands;
mod config;
mod utils;

use cli::{Cli, Commands};
use commands::{
    handle_config_command, handle_export_command, handle_extension_command, handle_fetch_command,
    handle_library_command, handle_publish_command, handle_search_command, handle_store_command,
};
use config::Config;

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

    // Load configuration
    let mut config = Config::load().await?;

    // Initialize storage for local library
    let storage_path = match cli.storage_path.as_ref() {
        Some(path) => PathBuf::from(path),
        None => config.get_storage_path(),
    };
    let storage = FilesystemStorage::new(&storage_path);
    storage.initialize().await?;

    // Initialize store manager
    let registry_dir = config.get_registry_dir();
    let registry_store = Box::new(LocalRegistryStore::new(&registry_dir).await?);
    let mut store_manager = StoreManager::new(registry_store).await?;

    // Apply registry configuration to store manager
    // Handle store loading errors gracefully - invalid stores shouldn't prevent CLI startup
    if let Err(e) = config.apply(&mut store_manager).await {
        eprintln!("⚠️  Warning: Some extension stores could not be loaded:");
        eprintln!("   {}", e);
        eprintln!("💡 Use 'quelle store list' to see configured stores");
        eprintln!("   Invalid stores can be removed with 'quelle store remove <name>'");
    }

    // Handle commands
    match cli.command {
        Commands::Add {
            url,
            no_chapters,
            max_chapters,
        } => {
            handle_add_command(
                url,
                no_chapters,
                max_chapters,
                &mut store_manager,
                &storage,
                cli.dry_run,
            )
            .await
        }
        Commands::Update { novel, check_only } => {
            handle_update_command(novel, check_only, &storage, &mut store_manager, cli.dry_run)
                .await
        }
        Commands::Read {
            novel,
            chapter,
            list,
        } => {
            handle_read_command(
                novel,
                chapter,
                list,
                &storage,
                &mut store_manager,
                cli.dry_run,
            )
            .await
        }
        Commands::Remove { novel, force } => {
            handle_remove_command(novel, force, &storage, &mut store_manager, cli.dry_run).await
        }
        Commands::Fetch { command } => {
            handle_fetch_command(command, &mut store_manager, &storage, cli.dry_run).await
        }
        Commands::Search {
            query,
            author,
            tags,
            categories,
            limit,
        } => {
            handle_search_command(
                &store_manager,
                query,
                author,
                tags,
                categories,
                limit,
                cli.dry_run,
            )
            .await
        }
        Commands::Library { command } => {
            handle_library_command(command, &storage, &mut store_manager, cli.dry_run).await
        }
        Commands::List => handle_list_command(&store_manager).await,
        Commands::Status => handle_status_command(&store_manager).await,
        Commands::Store { command } => {
            handle_store_command(command, &mut config, &mut store_manager).await
        }
        Commands::Extension { command } => {
            handle_extension_command(command, &mut store_manager, cli.dry_run).await
        }
        Commands::Export { command } => handle_export_command(command, &storage, cli.dry_run).await,
        Commands::Config { command } => handle_config_command(command, cli.dry_run).await,
        Commands::Publish { command } => {
            handle_publish_command(command, &config.registry, &mut store_manager).await
        }
    }
}

async fn handle_list_command(store_manager: &StoreManager) -> Result<()> {
    let stores = store_manager.list_extension_stores();
    if stores.is_empty() {
        println!("📦 No extension stores configured");
        println!("💡 Use 'quelle store add <name> <location>' to add stores");
        return Ok(());
    }

    println!("📦 Available extension stores ({}):", stores.len());
    for store in stores {
        let info = store.config();
        println!("  📍 {} ({})", info.store_name, info.store_type);

        match store.store().list_extensions().await {
            Ok(extensions) => {
                if extensions.is_empty() {
                    println!("     No extensions found");
                } else {
                    for ext in extensions.iter().take(5) {
                        println!("     - {} v{} by {}", ext.name, ext.version, ext.author);
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
        println!();
    }
    Ok(())
}

async fn handle_add_command(
    url: url::Url,
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

async fn handle_update_command(
    novel: String,
    check_only: bool,
    storage: &FilesystemStorage,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    use crate::cli::LibraryCommands;
    use crate::commands::library::handle_library_command;
    use crate::utils::{resolve_novel_id, show_novel_not_found_help};

    if dry_run {
        println!("Would update novel(s): {}", novel);
        return Ok(());
    }

    // Handle "all" case
    if novel == "all" {
        if check_only {
            println!("🔍 Checking all novels for new chapters");
        } else {
            println!("🔄 Updating all novels with new chapters");
        }
        let cmd = if check_only {
            LibraryCommands::Sync {
                novel_id: "all".to_string(),
            }
        } else {
            LibraryCommands::Update {
                novel_id: "all".to_string(),
            }
        };
        return handle_library_command(cmd, storage, store_manager, false).await;
    }

    // Resolve the novel identifier
    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            if check_only {
                println!("🔍 Checking for new chapters in: {}", novel);
                let sync_cmd = LibraryCommands::Sync {
                    novel_id: novel_id.as_str().to_string(),
                };
                handle_library_command(sync_cmd, storage, store_manager, false).await
            } else {
                println!("🔄 Updating novel: {}", novel);
                let update_cmd = LibraryCommands::Update {
                    novel_id: novel_id.as_str().to_string(),
                };
                handle_library_command(update_cmd, storage, store_manager, false).await
            }
        }
        None => {
            show_novel_not_found_help(&novel, storage).await;
            Ok(())
        }
    }
}

async fn handle_read_command(
    novel: String,
    chapter: Option<String>,
    list: bool,
    storage: &FilesystemStorage,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    use crate::cli::LibraryCommands;
    use crate::commands::library::handle_library_command;
    use crate::utils::{resolve_novel_id, show_novel_not_found_help};

    if dry_run {
        println!("Would read from novel: {}", novel);
        return Ok(());
    }

    // Resolve the novel identifier
    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            let novel_id_str = novel_id.as_str().to_string();

            if list {
                println!("📚 Listing chapters for: {}", novel);
                let chapters_cmd = LibraryCommands::Chapters {
                    novel_id: novel_id_str,
                    downloaded_only: true,
                };
                handle_library_command(chapters_cmd, storage, store_manager, false).await
            } else {
                match chapter {
                    Some(chapter_id) => {
                        println!("📖 Reading chapter: {}", chapter_id);
                        let read_cmd = LibraryCommands::Read {
                            novel_id: novel_id_str,
                            chapter: chapter_id,
                        };
                        handle_library_command(read_cmd, storage, store_manager, false).await
                    }
                    None => {
                        println!(
                            "📚 Please specify a chapter to read, or use --list to see available chapters"
                        );
                        let chapters_cmd = LibraryCommands::Chapters {
                            novel_id: novel_id_str,
                            downloaded_only: true,
                        };
                        handle_library_command(chapters_cmd, storage, store_manager, false).await
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

async fn handle_remove_command(
    novel: String,
    force: bool,
    storage: &FilesystemStorage,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    use crate::cli::LibraryCommands;
    use crate::commands::library::handle_library_command;
    use crate::utils::{resolve_novel_id, show_novel_not_found_help};

    if dry_run {
        println!("Would remove novel: {}", novel);
        return Ok(());
    }

    // Resolve the novel identifier
    match resolve_novel_id(&novel, storage).await? {
        Some(novel_id) => {
            println!("🗑️  Removing novel: {}", novel);
            let remove_cmd = LibraryCommands::Remove {
                novel_id: novel_id.as_str().to_string(),
                force,
            };
            handle_library_command(remove_cmd, storage, store_manager, false).await
        }
        None => {
            show_novel_not_found_help(&novel, storage).await;
            Ok(())
        }
    }
}

async fn handle_status_command(store_manager: &StoreManager) -> Result<()> {
    let stores = store_manager.list_extension_stores();
    println!("📊 Registry Status:");
    println!("  Configured stores: {}", stores.len());

    if stores.is_empty() {
        println!("💡 No stores configured. Add stores with: quelle store add <name> <location>");
        return Ok(());
    }

    for store in stores {
        let info = store.config();
        print!("  📍 {} ({}): ", info.store_name, info.store_type);

        match store.store().health_check().await {
            Ok(health) => {
                if health.healthy {
                    println!("✅ Healthy");
                    if let Some(count) = health.extension_count {
                        println!("    Extensions available: {}", count);
                    }
                    println!(
                        "    Last checked: {}",
                        health.last_check.format("%Y-%m-%d %H:%M")
                    );
                } else {
                    println!("❌ Unhealthy");
                    if let Some(error) = &health.error {
                        println!("    Error: {}", error);
                    }
                }
            }
            Err(e) => {
                println!("❌ Health check failed: {}", e);
            }
        }
    }

    // Show installed extensions count
    match store_manager.list_installed().await {
        Ok(installed) => {
            println!("  📦 Installed extensions: {}", installed.len());
        }
        Err(e) => {
            println!("  📦 Could not count installed extensions: {}", e);
        }
    }

    Ok(())
}
