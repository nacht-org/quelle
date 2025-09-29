use eyre::Result;
use quelle_store::{ConfigStore, ExtensionSource, RegistryConfig, StoreManager, StoreType};
use std::io::{self, Write};
use std::path::PathBuf;

use crate::cli::StoreCommands;

pub async fn handle_store_command(
    cmd: StoreCommands,
    config: &RegistryConfig,
    store_manager: &mut StoreManager,
    config_store: &dyn ConfigStore,
) -> Result<()> {
    match cmd {
        StoreCommands::Add {
            name,
            path,
            priority,
        } => handle_add_store(name, path, priority, config_store).await,
        StoreCommands::Remove { name, force } => {
            handle_remove_store(name, force, config_store).await
        }
        StoreCommands::List => handle_list_stores(config).await,
        StoreCommands::Update { name } => handle_update_store(name, config).await,
        StoreCommands::Info { name } => handle_store_info(name, config, store_manager).await,
    }
}

async fn handle_add_store(
    name: String,
    path: String,
    priority: u32,
    config_store: &dyn ConfigStore,
) -> Result<()> {
    // Load current configuration
    let mut config = config_store.load().await?;

    // Check if store already exists
    if config.extension_sources.iter().any(|s| s.name == name) {
        println!("❌ Store '{}' already exists", name);
        println!("💡 Use 'quelle store remove {}' to remove it first", name);
        return Ok(());
    }

    // Only support local stores
    let store_path = PathBuf::from(&path);
    if !store_path.exists() {
        println!("❌ Local path does not exist: {}", path);
        return Ok(());
    }

    // Create extension source
    let source = ExtensionSource::local(name.clone(), store_path.clone()).with_priority(priority);

    // Add to configuration
    config.add_source(source);

    // Save configuration
    config_store.save(&config).await?;

    println!("✅ Added store '{}'", name);
    println!("  Type: Local");
    println!("  Path: {}", path);
    println!("  Priority: {}", priority);

    Ok(())
}

async fn handle_remove_store(
    name: String,
    force: bool,
    config_store: &dyn ConfigStore,
) -> Result<()> {
    // Load current configuration
    let mut config = config_store.load().await?;

    // Check if store exists
    if !config.extension_sources.iter().any(|s| s.name == name) {
        println!("❌ Store '{}' not found", name);
        return Ok(());
    }

    if !force {
        print!("Are you sure you want to remove store '{}'? (y/N): ", name);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().to_lowercase().starts_with('y') {
            println!("❌ Cancelled");
            return Ok(());
        }
    }

    // Remove the store
    config.extension_sources.retain(|s| s.name != name);

    // Save configuration
    config_store.save(&config).await?;

    println!("✅ Removed store '{}'", name);
    Ok(())
}

async fn handle_list_stores(config: &RegistryConfig) -> Result<()> {
    if config.extension_sources.is_empty() {
        println!("📦 No extension stores configured");
        println!("💡 Use 'quelle store add <name> <location>' to add stores");
        return Ok(());
    }

    println!(
        "📦 Configured extension stores ({}):",
        config.extension_sources.len()
    );
    for source in &config.extension_sources {
        println!("  📍 {} (priority: {})", source.name, source.priority);
        println!("     Type: {:?}", source.store_type);
        match &source.store_type {
            StoreType::Local { path } => {
                println!("     Path: {}", path.display());
            }
        }
        println!();
    }
    Ok(())
}

async fn handle_update_store(name: String, config: &RegistryConfig) -> Result<()> {
    if name == "all" {
        println!("🔄 Refreshing all extension stores...");

        if config.extension_sources.is_empty() {
            println!("📦 No stores configured");
            return Ok(());
        }

        let mut updated_count = 0;
        let mut failed_count = 0;

        for source in &config.extension_sources {
            if !source.enabled {
                continue;
            }

            print!("🔄 Refreshing {}...", source.name);
            io::stdout().flush()?;

            match source.as_cacheable() {
                Ok(Some(cacheable_store)) => match cacheable_store.refresh_cache().await {
                    Ok(_) => {
                        println!(" ✅ Refreshed");
                        updated_count += 1;
                    }
                    Err(e) => {
                        println!(" ❌ Failed: {}", e);
                        failed_count += 1;
                    }
                },
                Ok(None) => {
                    println!(" ℹ️ Store does not support caching");
                    updated_count += 1;
                }
                Err(e) => {
                    println!(" ❌ Failed to create store: {}", e);
                    failed_count += 1;
                }
            }
        }

        println!(
            "📊 Refresh complete: {} processed, {} failed",
            updated_count, failed_count
        );
    } else {
        println!("🔄 Refreshing store '{}'...", name);

        let source = config
            .extension_sources
            .iter()
            .find(|s| s.name == name && s.enabled);

        match source {
            Some(source) => match source.as_cacheable() {
                Ok(Some(cacheable_store)) => match cacheable_store.refresh_cache().await {
                    Ok(_) => {
                        println!("✅ Store '{}' refreshed successfully", name);
                    }
                    Err(e) => {
                        println!("❌ Failed to refresh store '{}': {}", name, e);
                    }
                },
                Ok(None) => {
                    println!("ℹ️ Store '{}' does not support caching", name);
                }
                Err(e) => {
                    println!("❌ Failed to create store '{}': {}", name, e);
                }
            },
            None => {
                println!("❌ Store '{}' not found or disabled", name);
                println!("💡 Use 'quelle store list' to see available stores");
            }
        }
    }
    Ok(())
}

async fn handle_store_info(
    name: String,
    config: &RegistryConfig,
    _store_manager: &mut StoreManager,
) -> Result<()> {
    // Find the store in configuration
    let source = config.extension_sources.iter().find(|s| s.name == name);

    match source {
        Some(source) => {
            println!("📍 Store: {}", source.name);
            println!("Type: {:?}", source.store_type);
            println!("Priority: {}", source.priority);
            println!("Enabled: {}", source.enabled);
            println!("Trusted: {}", source.trusted);

            match &source.store_type {
                StoreType::Local { path } => {
                    println!("Path: {}", path.display());
                    println!("Exists: {}", path.exists());
                }
            }

            // Get runtime information by creating a store from the source
            if source.enabled {
                match source.as_readable() {
                    Ok(store) => {
                        println!("\nRuntime Information:");

                        // Check health
                        match store.health_check().await {
                            Ok(health) => {
                                println!(
                                    "Status: {}",
                                    if health.healthy {
                                        "✅ Healthy"
                                    } else {
                                        "❌ Unhealthy"
                                    }
                                );
                                if let Some(count) = health.extension_count {
                                    println!("Extensions: {}", count);
                                }
                                if let Some(error) = &health.error {
                                    println!("Error: {}", error);
                                }
                            }
                            Err(e) => {
                                println!("Status: ❌ Health check failed: {}", e);
                            }
                        }

                        // List a few extensions
                        match store.list_extensions().await {
                            Ok(extensions) => {
                                if extensions.is_empty() {
                                    println!("Extensions: None found");
                                } else {
                                    println!("Sample Extensions:");
                                    for ext in extensions.iter().take(5) {
                                        println!(
                                            "  - {} v{} by {}",
                                            ext.name, ext.version, ext.author
                                        );
                                    }
                                    if extensions.len() > 5 {
                                        println!("  ... and {} more", extensions.len() - 5);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Extensions: Failed to list: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("\nRuntime Information: Failed to create store: {}", e);
                    }
                }
            } else {
                println!("\nRuntime Information: Store is disabled");
            }
        }
        None => {
            println!("❌ Store '{}' not found", name);
            println!("💡 Use 'quelle store list' to see available stores");
        }
    }
    Ok(())
}
