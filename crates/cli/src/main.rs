use clap::Parser;
use eyre::Result;
use quelle_storage::backends::filesystem::FilesystemStorage;
use quelle_store::{StoreManager, registry::LocalInstallRegistry};
use std::path::PathBuf;

mod cli;
mod commands;
mod config;
mod engine;
mod resolve;

use cli::{Cli, Commands};
use commands::{
    handle_add_command, handle_config_command, handle_export_command, handle_extension_command,
    handle_fetch_command, handle_library_command, handle_publish_command, handle_read_command,
    handle_remove_command, handle_search_command, handle_status_command, handle_store_command,
    handle_update_command,
};
use config::Config;

struct AppContext {
    storage: FilesystemStorage,
    store_manager: StoreManager,
    config: Config,
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

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

    let config = if let Some(ref config_path) = cli.config {
        Config::load_from(std::path::Path::new(config_path)).await?
    } else {
        Config::load().await?
    };

    let storage_path = match cli.storage_path.as_ref() {
        Some(path) => PathBuf::from(path),
        None => config.get_storage_path(),
    };
    let storage = FilesystemStorage::new(&storage_path);
    storage.initialize().await?;

    let registry_dir = config.get_registry_dir();
    let registry = Box::new(LocalInstallRegistry::new(&registry_dir).await?);
    let mut store_manager = StoreManager::new(registry).await?;

    if let Err(e) = config.apply(&mut store_manager).await
        && !cli.quiet
    {
        eprintln!("Warning: Some extension stores could not be loaded: {}", e);
    }

    let mut ctx = AppContext {
        storage,
        store_manager,
        config,
        dry_run: cli.dry_run,
    };

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
                &mut ctx.store_manager,
                &ctx.storage,
                ctx.dry_run,
            )
            .await
        }
        Commands::Update { novel, check_only } => {
            handle_update_command(
                novel,
                check_only,
                &ctx.storage,
                &mut ctx.store_manager,
                ctx.dry_run,
            )
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
                &ctx.storage,
                &mut ctx.store_manager,
                ctx.dry_run,
            )
            .await
        }
        Commands::Remove { novel, force } => {
            handle_remove_command(
                novel,
                force,
                &ctx.storage,
                &mut ctx.store_manager,
                ctx.dry_run,
            )
            .await
        }
        Commands::Search {
            query,
            author,
            tags,
            categories,
            limit,
            page,
            advanced,
            simple,
        } => {
            handle_search_command(
                &ctx.store_manager,
                query,
                author,
                tags,
                categories,
                limit,
                page,
                advanced,
                simple,
                ctx.dry_run,
            )
            .await
        }
        Commands::Library { command } => {
            handle_library_command(command, &ctx.storage, &mut ctx.store_manager, ctx.dry_run).await
        }
        Commands::Extensions { command } => {
            handle_extension_command(command, &mut ctx.store_manager, ctx.dry_run).await
        }
        Commands::Export {
            novel,
            format,
            output,
            include_images,
        } => {
            handle_export_command(
                novel,
                format,
                output,
                include_images,
                &ctx.storage,
                ctx.dry_run,
            )
            .await
        }
        Commands::Config { command } => handle_config_command(command, ctx.dry_run).await,
        Commands::Store { command } => {
            handle_store_command(command, &mut ctx.config, &mut ctx.store_manager).await
        }
        Commands::Publish { command } => {
            handle_publish_command(command, &ctx.config.registry, &mut ctx.store_manager).await
        }
        Commands::Status => handle_status_command(&ctx.store_manager).await,
        Commands::Fetch { command } => {
            handle_fetch_command(command, &mut ctx.store_manager, &ctx.storage, ctx.dry_run).await
        }
    }
}
