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
            location,
            store_type,
            priority,
        } => handle_add_store(name, location, store_type, priority, config_store).await,
        StoreCommands::Remove { name, force } => {
            handle_remove_store(name, force, config_store).await
        }
        StoreCommands::List => handle_list_stores(config).await,
        StoreCommands::Update { name } => handle_update_store(name, store_manager).await,
        StoreCommands::Info { name } => handle_store_info(name, config, store_manager).await,
    }
}

async fn handle_add_store(
    name: String,
    location: String,
    store_type: Option<String>,
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

    // Determine store type
    let store_type = if let Some(t) = store_type {
        match t.as_str() {
            "local" => StoreType::Local {
                path: PathBuf::from(&location),
            },
            "http" | "git" => {
                println!("❌ HTTP and Git stores are not yet implemented");
                return Ok(());
            }
            _ => {
                println!("❌ Invalid store type: {}", t);
                println!("💡 Valid types: local, http, git");
                return Ok(());
            }
        }
    } else {
        // For now, only support local stores
        let path = PathBuf::from(&location);
        if !path.exists() {
            println!("❌ Local path does not exist: {}", location);
            return Ok(());
        }
        StoreType::Local { path: path.clone() }
    };

    // Create extension source
    let source = match &store_type {
        StoreType::Local { path } => {
            ExtensionSource::local(name.clone(), path.clone()).with_priority(priority)
        }
    };

    // Add to configuration
    config.add_source(source);

    // Save configuration
    config_store.save(&config).await?;

    println!("✅ Added store '{}'", name);
    println!("  Type: {:?}", store_type);
    println!("  Location: {}", location);
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

async fn handle_update_store(name: String, store_manager: &mut StoreManager) -> Result<()> {
    if name == "all" {
        println!("🔄 Updating all extension stores...");
        let stores = store_manager.list_extension_stores();

        if stores.is_empty() {
            println!("📦 No stores configured");
            return Ok(());
        }

        let mut updated_count = 0;
        let mut failed_count = 0;

        for store in stores {
            let store_name = store.config().store_name.clone();
            print!("🔄 Updating {}...", store_name);
            io::stdout().flush()?;

            // Note: refresh method not available on ReadableStore trait
            match store.store().health_check().await {
                Ok(health) => {
                    if health.healthy {
                        println!(" ✅");
                    } else {
                        println!(" ❌");
                    }
                    updated_count += 1;
                }
                Err(e) => {
                    println!(" ❌ Failed: {}", e);
                    failed_count += 1;
                }
            }
        }

        println!(
            "📊 Update complete: {} updated, {} failed",
            updated_count, failed_count
        );
    } else {
        println!("🔄 Updating store '{}'...", name);

        let stores = store_manager.list_extension_stores();
        let store = stores.into_iter().find(|s| s.config().store_name == name);

        match store {
            Some(store) => match store.store().health_check().await {
                Ok(health) => {
                    if health.healthy {
                        println!("✅ Store '{}' is healthy", name);
                    } else {
                        println!("❌ Store '{}' is unhealthy", name);
                    }
                }
                Err(e) => {
                    println!("❌ Failed to check store '{}': {}", name, e);
                }
            },
            None => {
                println!("❌ Store '{}' not found", name);
                println!("💡 Use 'quelle store list' to see available stores");
            }
        }
    }
    Ok(())
}

async fn handle_store_info(
    name: String,
    config: &RegistryConfig,
    store_manager: &mut StoreManager,
) -> Result<()> {
    // Find the store in configuration
    let source = config.extension_sources.iter().find(|s| s.name == name);

    match source {
        Some(source) => {
            println!("📍 Store: {}", source.name);
            println!("Type: {:?}", source.store_type);
            println!("Priority: {}", source.priority);

            match &source.store_type {
                StoreType::Local { path } => {
                    println!("Path: {}", path.display());
                    println!("Exists: {}", path.exists());
                }
            }

            // Get runtime information from store manager
            let stores = store_manager.list_extension_stores();
            if let Some(store) = stores.into_iter().find(|s| s.config().store_name == name) {
                println!("\nRuntime Information:");

                // Check health
                match store.store().health_check().await {
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
                match store.store().list_extensions().await {
                    Ok(extensions) => {
                        if extensions.is_empty() {
                            println!("Extensions: None found");
                        } else {
                            println!("Sample Extensions:");
                            for ext in extensions.iter().take(5) {
                                println!("  - {} v{} by {}", ext.name, ext.version, ext.author);
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
            } else {
                println!("\nRuntime Information: Store not loaded");
            }
        }
        None => {
            println!("❌ Store '{}' not found", name);
            println!("💡 Use 'quelle store list' to see available stores");
        }
    }
    Ok(())
}
