mod cli;
mod store_commands;

use std::path::PathBuf;

use clap::Parser;

use quelle_store::{ConfigStore, LocalConfigStore, LocalRegistryStore, SearchQuery, StoreManager};

use crate::cli::{Commands, FetchCommands};
use crate::store_commands::{handle_extension_command, handle_store_command};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = cli::Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize config store for persistence (using ./local for easier testing)
    let config_dir = PathBuf::from("./local");
    let config_file = config_dir.join("config.json");
    let config_store = LocalConfigStore::new(config_file).await?;

    // Initialize store manager
    let registry_dir = PathBuf::from("./registry");
    let registry_store = Box::new(LocalRegistryStore::new(registry_dir).await?);
    let mut store_manager = StoreManager::new(registry_store).await?;

    // Load configuration and apply to registry
    let config = config_store.load().await?;
    config.apply(&mut store_manager).await?;

    match cli.command {
        Commands::Fetch { command } => {
            handle_fetch_command(command, &mut store_manager).await?;
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
            handle_store_command(command, &mut store_manager, &config_store).await?;
        }
        Commands::Extension { command } => {
            handle_extension_command(command, &mut store_manager).await?;
        }
    }

    Ok(())
}

async fn handle_fetch_command(
    cmd: FetchCommands,
    store_manager: &mut StoreManager,
) -> eyre::Result<()> {
    match cmd {
        FetchCommands::Novel { url } => {
            // Find extension that can handle this URL
            match find_extension_for_url(&url.to_string(), store_manager).await? {
                Some((extension_name, _store_name)) => {
                    println!("Using extension: {}", extension_name);

                    // Install extension if not already installed
                    if store_manager
                        .get_installed(&extension_name)
                        .await?
                        .is_none()
                    {
                        println!("Installing extension {}...", extension_name);
                        match store_manager.install(&extension_name, None, None).await {
                            Ok(installed) => {
                                println!("✅ Installed {}@{}", installed.name, installed.version);
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to install {}: {}", extension_name, e);
                                return Err(e.into());
                            }
                        }
                    }

                    // TODO: Use the installed extension to fetch novel info
                    println!("📖 Fetching novel info from: {}", url);
                    println!(
                        "This would use the {} extension to fetch novel information",
                        extension_name
                    );
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
                Some((extension_name, _store_name)) => {
                    println!("Using extension: {}", extension_name);

                    // Install extension if not already installed
                    if store_manager
                        .get_installed(&extension_name)
                        .await?
                        .is_none()
                    {
                        println!("Installing extension {}...", extension_name);
                        match store_manager.install(&extension_name, None, None).await {
                            Ok(installed) => {
                                println!("✅ Installed {}@{}", installed.name, installed.version);
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to install {}: {}", extension_name, e);
                                return Err(e.into());
                            }
                        }
                    }

                    // TODO: Use the installed extension to fetch chapter
                    println!("📄 Fetching chapter from: {}", url);
                    println!(
                        "This would use the {} extension to fetch chapter content",
                        extension_name
                    );
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
        let info = store.store_info();
        println!("  📦 {} ({})", info.name, info.store_type);

        match store.list_extensions().await {
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
        let info = store.store_info();
        print!("  {} ({}): ", info.name, info.store_type);

        match store.health_check().await {
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
