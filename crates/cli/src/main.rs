mod cli;
mod store_commands;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use quelle_engine::{ExtensionEngine, bindings::SimpleSearchQuery, http::HeadlessChromeExecutor};
use quelle_store::{LocalSourceStore, SourceStore, StoreManager, create_store_from_source};

use crate::cli::Commands;
use crate::store_commands::{handle_extension_command, handle_store_command};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = cli::Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize source store for persistence (using ./local for easier testing)
    let config_dir = PathBuf::from("./local");
    let config_file = config_dir.join("sources.json");
    let source_store = LocalSourceStore::new(config_file).await?;

    // Initialize store manager
    let registry_dir = PathBuf::from("./registry");
    let registry_store = Box::new(quelle_store::LocalRegistryStore::new(registry_dir).await?);
    let mut store_manager = StoreManager::new(registry_store).await?;

    // Load and restore previously configured stores
    let saved_sources = source_store.load_sources().await?;
    for source in saved_sources {
        if source.enabled {
            match create_store_from_source(&source).await {
                Ok(store) => {
                    tracing::info!("Restored store: {} ({})", source.name, source.store_type);
                    store_manager.add_boxed_extension_store(store);
                }
                Err(e) => {
                    tracing::warn!("Failed to restore store '{}': {}", source.name, e);
                }
            }
        }
    }

    match cli.command {
        Commands::Store { command } => {
            handle_store_command(command, &mut store_manager, &source_store).await?;
        }
        Commands::Extension { command } => {
            handle_extension_command(command, &mut store_manager).await?;
        }
        Commands::Novel { url } => {
            let engine = ExtensionEngine::new(Arc::new(HeadlessChromeExecutor::new()))?;
            let runner = engine.new_runner_from_file(
                "target/wasm32-unknown-unknown/release/extension_scribblehub.wasm",
            )?;

            let (runner, extension_meta) = runner.meta()?;
            println!("Extension: {:?}", extension_meta);

            let (_runner, result) = runner.fetch_novel_info(url.as_str())?;

            println!("Novel: {:?}", result);
        }
        Commands::Chapter { url } => {
            let engine = ExtensionEngine::new(Arc::new(HeadlessChromeExecutor::new()))?;
            let runner = engine.new_runner_from_file(
                "target/wasm32-unknown-unknown/release/extension_scribblehub.wasm",
            )?;

            let (runner, extension_meta) = runner.meta()?;
            println!("Extension: {:?}", extension_meta);

            let (_runner, result) = runner.fetch_chapter(url.as_str())?;

            println!("Chapter: {:?}", result);
        }
        Commands::Search { query } => {
            let engine = ExtensionEngine::new(Arc::new(HeadlessChromeExecutor::new()))?;
            let runner = engine.new_runner_from_file(
                "target/wasm32-unknown-unknown/release/extension_scribblehub.wasm",
            )?;

            let (runner, extension_meta) = runner.meta()?;
            println!("Extension: {:?}", extension_meta);

            let (_runner, result) = runner.simple_search(&SimpleSearchQuery {
                query,
                limit: None,
                page: None,
            })?;

            println!("Search Result: {:?}", result);
        }
    }

    Ok(())
}
