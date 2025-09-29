use eyre::Result;
use quelle_store::{RegistryConfig, StoreManager, StoreType};
use std::io::{self, Write};
use std::path::PathBuf;

use crate::{cli::StoreCommands, config::Config};

pub async fn handle_store_command(
    cmd: StoreCommands,
    config: &mut Config,
    store_manager: &mut StoreManager,
) -> Result<()> {
    match cmd {
        StoreCommands::Add {
            name,
            path,
            priority,
        } => handle_add_store(name, path, priority, config, store_manager).await,
        StoreCommands::Remove { name, force } => {
            handle_remove_store(name, force, config, store_manager).await
        }
        StoreCommands::List => handle_list_stores(&config.registry).await,
        StoreCommands::Update { name } => handle_update_store(name, &config.registry).await,
        StoreCommands::Info { name } => {
            handle_store_info(name, &config.registry, store_manager).await
        }
    }
}

async fn handle_add_store(
    name: String,
    path: String,
    priority: u32,
    config: &mut Config,
    store_manager: &mut StoreManager,
) -> Result<()> {
    // Check if store already exists
    if config.registry.has_source(&name) {
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
    let source = quelle_store::ExtensionSource::local(name.clone(), store_path.clone())
        .with_priority(priority);

    // Add to CLI configuration
    config.registry.add_source(source);

    // Save CLI configuration
    config.save().await?;

    // Apply the updated registry config to store manager
    // We need to clear and re-apply all sources since we can't easily add just one
    store_manager.clear_extension_stores().await?;
    config.registry.apply(store_manager).await?;

    println!("✅ Added store '{}'", name);
    println!("  Type: Local");
    println!("  Path: {}", path);
    println!("  Priority: {}", priority);

    Ok(())
}

async fn handle_remove_store(
    name: String,
    force: bool,
    config: &mut Config,
    store_manager: &mut StoreManager,
) -> Result<()> {
    // Check if store exists
    if !config.registry.has_source(&name) {
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

    // Remove the store from CLI configuration
    let removed = config.registry.remove_source(&name);
    if !removed {
        println!("❌ Failed to remove store '{}'", name);
        return Ok(());
    }

    // Save CLI configuration
    config.save().await?;

    // Apply the updated registry config to store manager
    // We need to clear and re-apply all sources since we can't easily remove just one
    store_manager.clear_extension_stores().await?;
    config.registry.apply(store_manager).await?;

    println!("✅ Removed store '{}'", name);
    Ok(())
}

async fn handle_list_stores(registry_config: &RegistryConfig) -> Result<()> {
    if registry_config.extension_sources.is_empty() {
        println!("📦 No extension stores configured");
        println!("💡 Use 'quelle store add <name> <location>' to add stores");
        return Ok(());
    }

    println!(
        "📦 Configured extension stores ({}):",
        registry_config.extension_sources.len()
    );
    for source in &registry_config.extension_sources {
        println!("  📍 {} (priority: {})", source.name, source.priority);
        println!("     Type: {:?}", source.store_type);
        match &source.store_type {
            StoreType::Local { path } => {
                println!("     Path: {}", path.display());
                println!(
                    "     Status: {}",
                    if source.enabled {
                        "✅ Enabled"
                    } else {
                        "❌ Disabled"
                    }
                );
                if source.trusted {
                    println!("     Trusted: ✅ Yes");
                }
            }
        }
        println!();
    }
    Ok(())
}

async fn handle_update_store(name: String, registry_config: &RegistryConfig) -> Result<()> {
    if name == "all" {
        println!("🔄 Refreshing all extension stores...");

        if registry_config.extension_sources.is_empty() {
            println!("📦 No stores configured");
            return Ok(());
        }

        let mut updated_count = 0;
        let mut failed_count = 0;

        for source in &registry_config.extension_sources {
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

        let source = registry_config
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
    registry_config: &RegistryConfig,
    _store_manager: &mut StoreManager,
) -> Result<()> {
    // Find the store in configuration
    let source = registry_config
        .extension_sources
        .iter()
        .find(|s| s.name == name);

    match source {
        Some(source) => {
            println!("📍 Store: {}", source.name);
            println!("Type: {:?}", source.store_type);
            println!("Priority: {}", source.priority);
            println!("Enabled: {}", source.enabled);
            println!("Trusted: {}", source.trusted);
            println!("Added: {}", source.added_at.format("%Y-%m-%d %H:%M:%S UTC"));

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
                                println!(
                                    "Last checked: {}",
                                    health.last_check.format("%Y-%m-%d %H:%M:%S UTC")
                                );
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
