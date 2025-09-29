use clap::Parser;
use eyre::Result;
use quelle_storage::backends::filesystem::FilesystemStorage;
use quelle_store::{StoreManager, registry::LocalRegistryStore};

mod cli;
mod commands;
mod config;
mod utils;

use cli::{Cli, Commands};
use commands::{
    handle_config_command, handle_export_command, handle_extension_command, handle_fetch_command,
    handle_library_command, handle_search_command, handle_store_command,
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
    let storage_path = utils::get_storage_path_from_args(cli.storage_path.as_ref(), &config);
    let storage = FilesystemStorage::new(&storage_path);
    storage.initialize().await?;

    // Initialize store manager
    let registry_dir = Config::get_registry_dir();
    let registry_store = Box::new(LocalRegistryStore::new(&registry_dir).await?);
    let mut store_manager = StoreManager::new(registry_store).await?;

    // Apply registry configuration to store manager
    config.apply(&mut store_manager).await?;

    // Handle commands
    match cli.command {
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
            handle_library_command(command, &storage, cli.dry_run).await
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
