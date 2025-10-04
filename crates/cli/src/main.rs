//! Quelle CLI - A novel scraper and library manager.
//!
//! This is the main entry point for the Quelle command-line interface,
//! which provides novel fetching, library management, and export functionality.

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
    handle_add_command, handle_config_command, handle_export_command, handle_extension_command,
    handle_fetch_command, handle_library_command, handle_publish_command, handle_read_command,
    handle_remove_command, handle_search_command, handle_status_command, handle_store_command,
    handle_update_command,
};
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle dev commands early to avoid store initialization
    if let Commands::Dev { command } = &cli.command {
        return quelle_dev::handle_dev_command(command.clone()).await;
    }

    // Initialize tracing
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else if cli.quiet {
            tracing::Level::ERROR
        } else {
            tracing::Level::WARN
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
    if let Err(e) = config.apply(&mut store_manager).await
        && !cli.quiet {
            eprintln!("Warning: Some extension stores could not be loaded: {}", e);
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
        Commands::Extensions { command } => {
            handle_extension_command(command, &mut store_manager, cli.dry_run).await
        }
        Commands::Export {
            novel,
            format,
            output,
            include_images,
        } => {
            handle_export_command(novel, format, output, include_images, &storage, cli.dry_run)
                .await
        }
        Commands::Config { command } => handle_config_command(command, cli.dry_run).await,
        Commands::Store { command } => {
            handle_store_command(command, &mut config, &mut store_manager).await
        }
        Commands::Publish { command } => {
            handle_publish_command(command, &config.registry, &mut store_manager).await
        }
        Commands::Status => handle_status_command(&store_manager).await,
        Commands::Fetch { command } => {
            handle_fetch_command(command, &mut store_manager, &storage, cli.dry_run).await
        }
        Commands::Dev { .. } => {
            // This case is handled early in main() to avoid store initialization
            unreachable!("Dev commands should be handled before reaching this point")
        }
    }
}
