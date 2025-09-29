use clap::Parser;
use eyre::{Context, Result};
use quelle_storage::backends::filesystem::FilesystemStorage;

mod cli;
mod commands;
mod config;
mod utils;

use cli::{Cli, Commands};
use commands::{
    handle_config_command, handle_export_command, handle_extension_command, handle_fetch_command,
    handle_library_command, handle_search_command,
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
    let storage_path = utils::get_storage_path_from_args(cli.storage_path.as_ref());

    let storage = FilesystemStorage::new(&storage_path);
    storage.initialize().await?;

    // Initialize store manager
    let mut store_manager = utils::create_store_manager_with_path(storage_path.clone())
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
