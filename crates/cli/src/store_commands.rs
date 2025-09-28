use std::path::PathBuf;
use std::time::Duration;

use clap::Subcommand;
use eyre::Context;
use quelle_store::{
    BaseStore, ConfigStore, ExtensionSource, ExtensionVisibility, RegistryConfig, SearchQuery,
    SearchSortBy, StoreManager,
    models::{ExtensionPackage, InstallOptions},
    publish::{PublishOptions, UnpublishOptions},
    registry_config::RegistryStoreConfig,
    stores::local::LocalStore,
    validation::{create_default_validator, create_strict_validator},
};
use tracing::{error, info, warn};

#[derive(Debug, Subcommand)]
pub enum StoreCommands {
    /// Add a new store
    Add {
        #[command(subcommand)]
        store_type: StoreType,
    },
    /// List all configured stores
    List,
    /// Remove a store
    Remove {
        /// Name of the store to remove
        name: String,
    },
    /// Check health of all stores
    Health,
    /// Search for extensions across all stores
    Search {
        /// Search query text
        query: String,
        /// Filter by author
        #[arg(long)]
        author: Option<String>,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Sort results by
        #[arg(long, default_value = "relevance")]
        sort: SortOption,
        /// Maximum number of results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// List all available extensions
    ListExtensions,
    /// Show publishing requirements for a store
    Requirements {
        /// Store name (optional, shows all if not specified)
        #[arg(long)]
        store: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum StoreType {
    /// Add a local file system store
    Local {
        /// Path to the local store directory
        path: PathBuf,
        /// Custom name for the store
        #[arg(long)]
        name: Option<String>,
    },
    // Future store types will be added here:
    // Git, Http, S3, etc.
}

#[derive(Debug, Subcommand)]
pub enum ExtensionCommands {
    /// Install an extension
    Install {
        /// Extension ID
        id: String,
        /// Specific version to install
        #[arg(long)]
        version: Option<String>,
        /// Force reinstallation
        #[arg(long)]
        force: bool,
    },
    /// Update an extension
    Update {
        /// Extension ID (or 'all' for all extensions)
        id: String,
        /// Include pre-release versions
        #[arg(long)]
        prerelease: bool,
        /// Force update even if no new version
        #[arg(long)]
        force: bool,
    },
    /// Uninstall an extension
    Uninstall {
        /// Extension ID
        id: String,
        /// Remove all files (not just registry entry)
        #[arg(long)]
        remove_files: bool,
    },
    /// List installed extensions
    List,
    /// Show extension information
    Info {
        /// Extension ID
        id: String,
    },
    /// Check for available updates
    CheckUpdates,
    /// Publish an extension (new or updated version)
    Publish {
        /// Path to extension package or directory
        package_path: PathBuf,
        /// Target store name
        #[arg(long)]
        store: String,
        /// Mark as pre-release
        #[arg(long)]
        pre_release: bool,
        /// Extension visibility
        #[arg(long, default_value = "public")]
        visibility: VisibilityOption,
        /// Overwrite existing version
        #[arg(long)]
        overwrite: bool,
        /// Skip validation checks
        #[arg(long)]
        skip_validation: bool,
        /// Release notes
        #[arg(long)]
        notes: Option<String>,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Access token for authentication
        #[arg(long)]
        token: Option<String>,
        /// Timeout in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,
        /// Use development defaults (overwrite, skip validation, etc.)
        #[arg(long)]
        dev: bool,
    },

    /// Remove a published extension version
    Unpublish {
        /// Extension ID
        id: String,
        /// Version to unpublish
        version: String,
        /// Target store name
        #[arg(long)]
        store: String,
        /// Reason for unpublishing
        #[arg(long)]
        reason: Option<String>,
        /// Keep tombstone record
        #[arg(long)]
        keep_record: bool,
        /// Notify users who installed this version
        #[arg(long)]
        notify_users: bool,
        /// Access token for authentication
        #[arg(long)]
        token: Option<String>,
    },
    /// Validate an extension package (dry-run)
    Validate {
        /// Path to extension package or directory
        package_path: PathBuf,
        /// Target store name (optional)
        #[arg(long)]
        store: Option<String>,
        /// Use strict validation rules
        #[arg(long)]
        strict: bool,
        /// Show detailed validation results
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SortOption {
    Relevance,
    Name,
    Version,
    Author,
    Updated,
    Downloads,
    Size,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum VisibilityOption {
    Public,
    Private,
    Unlisted,
    Organization,
}

impl From<SortOption> for SearchSortBy {
    fn from(sort: SortOption) -> Self {
        match sort {
            SortOption::Relevance => SearchSortBy::Relevance,
            SortOption::Name => SearchSortBy::Name,
            SortOption::Version => SearchSortBy::Version,
            SortOption::Author => SearchSortBy::Author,
            SortOption::Updated => SearchSortBy::LastUpdated,
            SortOption::Downloads => SearchSortBy::DownloadCount,
            SortOption::Size => SearchSortBy::Size,
        }
    }
}

pub async fn handle_store_command(
    cmd: StoreCommands,
    config: &RegistryConfig,
    manager: &mut StoreManager,
    config_store: &dyn ConfigStore,
) -> eyre::Result<()> {
    match cmd {
        StoreCommands::Add { store_type } => {
            handle_add_store(store_type, manager, config_store).await?;
        }
        StoreCommands::List => {
            handle_list_stores(manager, config_store).await?;
        }
        StoreCommands::Remove { name } => {
            handle_remove_store(name, manager, config_store).await?;
        }
        StoreCommands::Health => {
            handle_health_check(manager).await?;
        }
        StoreCommands::Search {
            query,
            author,
            tags,
            sort,
            limit,
        } => {
            handle_search(query, author, tags, sort, limit, manager).await?;
        }
        StoreCommands::ListExtensions => {
            handle_list_extensions(manager).await?;
        }
        StoreCommands::Requirements { store } => {
            handle_requirements(store, config).await?;
        }
    }
    Ok(())
}

pub async fn handle_extension_command(
    command: ExtensionCommands,
    config: &RegistryConfig,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    match command {
        ExtensionCommands::Install { id, version, force } => {
            handle_install_extension(id, version, force, manager).await
        }
        ExtensionCommands::Update {
            id,
            prerelease,
            force,
        } => handle_update_extension(id, prerelease, force, manager).await,
        ExtensionCommands::Uninstall { id, remove_files } => {
            handle_uninstall_extension(id, remove_files, manager).await
        }
        ExtensionCommands::List => handle_list_installed(manager).await,
        ExtensionCommands::Info { id } => handle_extension_info(id, manager).await,
        ExtensionCommands::CheckUpdates => handle_check_updates(manager).await,
        ExtensionCommands::Publish {
            package_path,
            store,
            pre_release,
            visibility,
            overwrite,
            skip_validation,
            notes,
            tags,
            token,
            timeout,
            dev,
        } => {
            handle_publish_extension(
                package_path,
                store,
                pre_release,
                visibility,
                overwrite,
                skip_validation,
                notes,
                tags,
                token,
                timeout,
                dev,
                config,
            )
            .await
        }
        ExtensionCommands::Unpublish {
            id,
            version,
            store,
            reason,
            keep_record,
            notify_users,
            token,
        } => {
            handle_unpublish_extension(
                id,
                version,
                store,
                reason,
                keep_record,
                notify_users,
                token,
                config,
            )
            .await
        }
        ExtensionCommands::Validate {
            package_path,
            store,
            strict,
            verbose,
        } => handle_validate_extension(package_path, store, strict, verbose, manager).await,
    }
}

async fn handle_add_store(
    store_type: StoreType,
    manager: &mut StoreManager,
    config_store: &dyn ConfigStore,
) -> eyre::Result<()> {
    match store_type {
        StoreType::Local { path, name } => {
            let store_name = name.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("local")
                    .to_string()
            });

            info!("Adding local store '{}' at path: {:?}", store_name, path);

            // Check if store already exists in config
            let mut config = config_store.load().await?;
            if config.has_source(&store_name) {
                error!("Store '{}' already exists", store_name);
                return Err(eyre::eyre!("Store '{}' already exists", store_name));
            }

            // Handle path according to its current state
            if path.exists() {
                // Check if path is a file - this is an error
                if path.is_file() {
                    error!("Path is a file, not a directory: {:?}", path);
                    return Err(eyre::eyre!("Path must be a directory, not a file"));
                }

                // Path exists and is a directory - check if it's empty or has store content
                let mut dir_entries = std::fs::read_dir(&path)
                    .map_err(|e| eyre::eyre!("Cannot read directory: {}", e))?;

                let is_empty = dir_entries.next().is_none();

                if is_empty {
                    // Empty directory - initialize it
                    info!("Directory is empty, initializing as new store...");

                    let local_store = LocalStore::new(&path)
                        .map_err(|e| eyre::eyre!("Failed to create local store: {}", e))?;

                    local_store
                        .initialize_store(store_name.clone(), None)
                        .await
                        .map_err(|e| eyre::eyre!("Failed to initialize store: {}", e))?;

                    info!("Successfully initialized new store");
                } else {
                    // Directory has content - validate it as an existing store
                    info!("Directory exists with content, validating as existing store...");

                    let local_store = LocalStore::new(&path)
                        .map_err(|e| eyre::eyre!("Failed to create local store: {}", e))?;

                    // Validate existing store - don't write anything to it
                    match local_store.health_check().await {
                        Ok(health) => {
                            if !health.healthy {
                                let error_msg = health.error.unwrap_or_default();
                                error!("Existing store validation failed: {}", error_msg);
                                return Err(eyre::eyre!("Store validation failed: {}", error_msg));
                            }

                            if let Some(count) = health.extension_count {
                                info!("Validated existing store with {} extensions", count);
                            } else {
                                info!("Validated existing store structure");
                            }
                        }
                        Err(e) => {
                            error!("Failed to validate existing store: {}", e);
                            return Err(eyre::eyre!("Store validation failed: {}", e));
                        }
                    }
                }
            } else {
                // Path doesn't exist - create the directory and initialize
                info!("Path doesn't exist, creating directory and initializing store...");

                std::fs::create_dir_all(&path)
                    .map_err(|e| eyre::eyre!("Failed to create directory: {}", e))?;

                let local_store = LocalStore::new(&path)
                    .map_err(|e| eyre::eyre!("Failed to create local store: {}", e))?;

                local_store
                    .initialize_store(store_name.clone(), None)
                    .await
                    .map_err(|e| eyre::eyre!("Failed to initialize store: {}", e))?;

                info!("Successfully created and initialized new store");
            }

            // Create the final store instance for adding to manager
            let local_store = LocalStore::new(&path)
                .map_err(|e| eyre::eyre!("Failed to create local store: {}", e))?;

            // Create source configuration
            let source = ExtensionSource::local(store_name.clone(), path);

            // Create registry config
            let registry_config = RegistryStoreConfig::new(store_name.clone(), "local".to_string());

            // Add to manager
            manager
                .add_extension_store(local_store, registry_config)
                .await?;

            // Persist the configuration
            config.add_source(source);
            config_store
                .save(&config)
                .await
                .map_err(|e| eyre::eyre!("Failed to save store configuration: {}", e))?;

            println!("✅ Successfully added local store '{}'", store_name);
        }
    }
    Ok(())
}

async fn handle_list_stores(
    manager: &StoreManager,
    config_store: &dyn ConfigStore,
) -> eyre::Result<()> {
    let config = config_store.load().await?;
    let sources = &config.extension_sources;
    let active_stores = manager.list_extension_stores();

    if sources.is_empty() {
        println!("No extension stores configured.");
        println!("Use 'quelle store add' to add a store.");
        return Ok(());
    }

    println!("Configured extension stores:");
    for source in sources {
        let status = if active_stores
            .iter()
            .any(|s| s.config().store_name == source.name)
        {
            "✅ Active"
        } else if !source.enabled {
            "⏸️  Disabled"
        } else {
            "❌ Failed to load"
        };

        println!("  📦 {} ({}) - {}", source.name, source.store_type, status);

        if let Some(path) = source.get_path() {
            println!("     Path: {}", path.display());
        }

        println!(
            "     Priority: {} | Trusted: {} | Added: {}",
            source.priority,
            if source.trusted { "Yes" } else { "No" },
            source.added_at.format("%Y-%m-%d %H:%M UTC")
        );
    }
    Ok(())
}

async fn handle_remove_store(
    name: String,
    manager: &mut StoreManager,
    config_store: &dyn ConfigStore,
) -> eyre::Result<()> {
    let removed_from_manager = manager.remove_extension_store(&name);

    let mut config = config_store.load().await?;
    let removed_from_config = config.remove_source(&name);

    if removed_from_config {
        config_store
            .save(&config)
            .await
            .map_err(|e| eyre::eyre!("Failed to save store configuration: {}", e))?;
    }

    if removed_from_manager || removed_from_config {
        println!("✅ Successfully removed store '{}'", name);
    } else {
        println!("❌ Store '{}' not found", name);
    }
    Ok(())
}

async fn handle_health_check(manager: &mut StoreManager) -> eyre::Result<()> {
    info!("Checking health of all stores...");

    // Check registry store health
    println!("Registry Store:");
    match manager.get_registry_health().await {
        Ok(health) => {
            let status = if health.healthy {
                "✅ Healthy"
            } else {
                "❌ Unhealthy"
            };
            println!("  Status: {}", status);
            println!("  Extensions: {}", health.total_extensions);
            println!("  Last updated: {:?}", health.last_updated);
            for (key, value) in &health.implementation_info {
                println!("  {}: {}", key, value);
            }
        }
        Err(e) => {
            error!("Failed to get registry health: {}", e);
        }
    }

    // Check extension stores health
    println!("\nExtension Stores:");
    let failed_stores = manager.refresh_stores().await?;

    let stores = manager.list_extension_stores();
    for store in stores {
        let info = store.config();
        let status = if failed_stores.contains(&info.store_name) {
            "❌ Unhealthy"
        } else {
            "✅ Healthy"
        };
        println!("  {}: {}", info.store_name, status);
    }

    if !failed_stores.is_empty() {
        warn!("Some stores are unhealthy. Check logs for details.");
    }

    Ok(())
}

async fn handle_search(
    query: String,
    author: Option<String>,
    tags: Option<String>,
    sort: SortOption,
    limit: usize,
    manager: &StoreManager,
) -> eyre::Result<()> {
    info!("Searching for extensions: '{}'", query);

    let mut search_query = SearchQuery::new().with_text(query).limit(limit);

    if let Some(author) = author {
        search_query = search_query.with_author(author);
    }

    if let Some(tags_str) = tags {
        let tag_list: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        search_query = search_query.with_tags(tag_list);
    }

    search_query = search_query.sort_by(sort.into());

    match manager.search_all_stores(&search_query).await {
        Ok(results) => {
            if results.is_empty() {
                println!("No extensions found matching your criteria.");
                return Ok(());
            }

            println!("Found {} extension(s):\n", results.len());

            for ext in results {
                println!("📦 {} v{}", ext.name, ext.version);
                println!("   By: {}", ext.author);
                if let Some(desc) = &ext.description {
                    println!("   {}", desc);
                }
                if !ext.tags.is_empty() {
                    println!("   Tags: {}", ext.tags.join(", "));
                }
                println!("   Size: {}", format_size(ext.size.unwrap_or(0)));
                println!();
            }
        }
        Err(e) => {
            error!("Search failed: {}", e);
        }
    }

    Ok(())
}

async fn handle_list_extensions(manager: &StoreManager) -> eyre::Result<()> {
    info!("Listing available extensions from all stores...");

    // Use an empty search to get all extensions
    let query = SearchQuery::new().limit(1000); // Large limit to get most extensions

    match manager.search_all_stores(&query).await {
        Ok(extensions) => {
            if extensions.is_empty() {
                println!("No extensions available in configured stores.");
                println!("Add some stores using 'quelle store add' first.");
                return Ok(());
            }

            println!("Available extensions ({}):\n", extensions.len());

            // Group by store
            use std::collections::HashMap;
            let mut by_store: HashMap<String, Vec<_>> = HashMap::new();

            for ext in extensions {
                by_store
                    .entry(ext.store_source.clone())
                    .or_default()
                    .push(ext);
            }

            for (store_name, exts) in by_store {
                println!("📦 From store '{}':", store_name);
                for ext in exts {
                    println!("   {} v{} - {}", ext.name, ext.version, ext.author);
                    if let Some(desc) = &ext.description {
                        let short_desc = if desc.len() > 80 {
                            format!("{}...", &desc[..77])
                        } else {
                            desc.clone()
                        };
                        println!("     {}", short_desc);
                    }
                }
                println!();
            }
        }
        Err(e) => {
            error!("Failed to list extensions: {}", e);
        }
    }

    Ok(())
}

async fn handle_install_extension(
    id: String,
    version: Option<String>,
    force: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Installing extension: {}", id);

    let options = InstallOptions {
        auto_update: false,
        force_reinstall: force,
        skip_verification: false,
    };

    println!("Installing extension '{}'...", id);
    if let Some(v) = &version {
        println!("  Requested version: {}", v);
    }
    println!("  Force reinstall: {}", force);

    match manager
        .install(&id, version.as_deref(), Some(options))
        .await
    {
        Ok(installed) => {
            println!("✅ Successfully installed extension:");
            println!("  ID: {}", installed.id);
            println!("  Name: {}", installed.name);
            println!("  Version: {}", installed.version);
            println!("  Source store: {}", installed.source_store);
            println!("  Size: {}", format_size(installed.calculate_size()));
        }
        Err(e) => {
            error!("Failed to install extension '{}': {}", id, e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_update_extension(
    id: String,
    prerelease: bool,
    force: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Updating extension: {}", id);

    println!("Updating extension '{}'...", id);
    println!("  Include prerelease: {}", prerelease);
    println!("  Force update: {}", force);

    // Check if extension is installed first
    match manager.get_installed(&id).await? {
        Some(installed) => {
            let options = quelle_store::models::UpdateOptions {
                include_prereleases: prerelease,
                force_update: force,
                ..Default::default()
            };

            match manager.update(&id, Some(options)).await {
                Ok(updated) => {
                    println!("✅ Successfully updated extension:");
                    println!("  ID: {}", updated.id);
                    println!("  Name: {}", updated.name);
                    println!("  Version: {} → {}", installed.version, updated.version);
                    println!("  Source store: {}", updated.source_store);
                    println!("  Size: {}", format_size(updated.calculate_size()));
                    println!(
                        "  Updated: {}",
                        updated
                            .last_updated
                            .unwrap_or(updated.installed_at)
                            .format("%Y-%m-%d %H:%M:%S")
                    );
                }
                Err(e) => {
                    error!("Failed to update extension '{}': {}", id, e);
                    return Err(e.into());
                }
            }
        }
        None => {
            error!("Extension '{}' is not installed", id);
            println!("Run 'quelle extension install {}' to install it first.", id);
            return Err(eyre::eyre!("Extension '{}' is not installed", id));
        }
    }

    Ok(())
}

async fn handle_uninstall_extension(
    id: String,
    remove_files: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Uninstalling extension: {}", id);

    println!("Uninstalling extension '{}'...", id);
    println!("  Remove files: {}", remove_files);

    // Check if extension is installed
    match manager.get_installed(&id).await? {
        Some(installed) => {
            println!("  Current version: {}", installed.version);
            println!("  Size: {}", format_size(installed.calculate_size()));

            match manager.uninstall(&id).await {
                Ok(removed) => {
                    if removed {
                        println!("✅ Successfully uninstalled extension '{}'", id);
                        if remove_files {
                            println!("  Extension data removed");
                        } else {
                            info!(
                                "Note: Files may remain on disk depending on store configuration"
                            );
                        }
                    } else {
                        error!("Extension '{}' was not found in registry", id);
                        return Err(eyre::eyre!("Extension '{}' was not found in registry", id));
                    }
                }
                Err(e) => {
                    error!("Failed to uninstall extension '{}': {}", id, e);
                    return Err(e.into());
                }
            }
        }
        None => {
            error!("Extension '{}' is not installed", id);
            return Err(eyre::eyre!("Extension '{}' is not installed", id));
        }
    }

    Ok(())
}

async fn handle_list_installed(manager: &StoreManager) -> eyre::Result<()> {
    info!("Listing installed extensions...");

    match manager.list_installed().await {
        Ok(installed) => {
            if installed.is_empty() {
                println!("No extensions installed.");
            } else {
                println!("Installed extensions ({}):", installed.len());
                for ext in &installed {
                    println!("  {} ({}) v{}", ext.name, ext.id, ext.version);
                    println!("    Source: {}", ext.source_store);
                    println!(
                        "    Installed: {}",
                        ext.installed_at.format("%Y-%m-%d %H:%M:%S")
                    );
                    if let Some(updated) = ext.last_updated {
                        println!("    Last updated: {}", updated.format("%Y-%m-%d %H:%M:%S"));
                    }
                    println!("  Size: {}", format_size(ext.calculate_size()));
                    println!("    Auto-update: {}", ext.auto_update);
                    println!();
                }
            }
        }
        Err(e) => {
            error!("Failed to list installed extensions: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_extension_info(id: String, manager: &StoreManager) -> eyre::Result<()> {
    info!("Getting extension info: {}", id);

    println!("Extension information for '{}':", id);

    // Check if extension is installed
    match manager.get_installed(&id).await {
        Ok(Some(installed)) => {
            println!("  Status: ✅ Installed");
            println!("  ID: {}", installed.id);
            println!("  Name: {}", installed.name);
            println!("  Version: {}", installed.version);
            println!("  Source store: {}", installed.source_store);
            println!(
                "  Installed: {}",
                installed.installed_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(updated) = installed.last_updated {
                println!("  Last updated: {}", updated.format("%Y-%m-%d %H:%M:%S"));
            }
            println!("  Size: {}", format_size(installed.calculate_size()));
            println!("  Auto-update: {}", installed.auto_update);
        }
        Ok(None) => {
            println!("  Status: ❌ Not installed");

            // Search for extension in available stores
            println!("  Searching in available stores...");
            match manager.get_extension_info(&id).await {
                Ok(infos) if !infos.is_empty() => {
                    let info = &infos[0]; // Use the first (best) match
                    println!("  Found in store: {}", info.store_source);
                    println!("  ID: {}", info.id);
                    println!("  Name: {}", info.name);
                    println!("  Latest version: {}", info.version);
                    println!("  Author: {}", info.author);
                    if let Some(desc) = &info.description {
                        println!("  Description: {}", desc);
                    }
                    if let Some(size) = info.size {
                        println!("  Package size: {}", format_size(size));
                    }
                }
                Ok(_) => {
                    println!("  ❌ Extension not found in any configured store");
                }
                Err(e) => {
                    error!("Failed to search for extension: {}", e);
                    return Err(e.into());
                }
            }
        }
        Err(e) => {
            error!("Failed to check installed extensions: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_check_updates(manager: &StoreManager) -> eyre::Result<()> {
    info!("Checking for extension updates...");

    println!("Checking for available updates...");

    match manager.list_installed().await {
        Ok(installed) => {
            if installed.is_empty() {
                println!("No extensions installed.");
                return Ok(());
            }

            match manager.check_all_updates().await {
                Ok(updates) => {
                    if updates.is_empty() {
                        println!("✅ All extensions are up to date!");
                    } else {
                        println!("Found {} update(s) available:", updates.len());
                        for update in &updates {
                            println!(
                                "  📦 {} {} → {}",
                                update.extension_name,
                                update.current_version,
                                update.latest_version
                            );
                            println!("    Store: {}", update.store_source);
                            if let Some(changelog) = &update.changelog_url {
                                println!("    Changelog: {}", changelog);
                            }
                            if let Some(size) = update.update_size {
                                println!("    Download size: {}", format_size(size));
                            }
                            if update.breaking_changes {
                                println!("    ⚠️  Breaking changes");
                            }
                            if update.security_update {
                                println!("    🔒 Security update");
                            }
                            println!();
                        }
                        println!(
                            "Run 'quelle extension update <name>' to update individual extensions."
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to check for updates: {}", e);
                    return Err(e.into());
                }
            }
        }
        Err(e) => {
            error!("Failed to get installed extensions: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

// New publishing functionality

async fn handle_requirements(
    store_name: Option<String>,
    config: &RegistryConfig,
) -> eyre::Result<()> {
    if let Some(store_name) = store_name {
        let writable_store = config
            .get_writable_source(&store_name)
            .map_err(|e| eyre::eyre!(e))
            .wrap_err("Failed to get writable store configuration")?;

        let Some(store) = writable_store else {
            error!(
                "Store '{}' not found or is not configured as writable ",
                store_name
            );
            return Err(eyre::eyre!(
                "Store '{}' not found or is not configured as writable ",
                store_name
            ));
        };

        println!("Publishing requirements for store '{}':", store_name);
        if let Ok(manifest) = store.get_store_manifest().await {
            println!("  Store type: {}", manifest.store_type);
            println!(
                "  Description: {}",
                manifest.description.as_deref().unwrap_or("None")
            );
        }

        let requirements = store.publish_requirements();

        println!("  Publishing supported: Yes");
        println!(
            "  Authentication required: {}",
            requirements.requires_authentication
        );
        println!("  Signing required: {}", requirements.requires_signing);
        if let Some(max_size) = requirements.max_package_size {
            println!("  Max package size: {} MB", max_size / (1024 * 1024));
        }
        if !requirements.supported_visibility.is_empty() {
            let visibility_options: Vec<String> = requirements
                .supported_visibility
                .iter()
                .map(|v| format!("{:?}", v))
                .collect();
            println!("  Supported visibility: {}", visibility_options.join(", "));
        }
    } else {
        println!("Publishing requirements for all stores:");
        for store in config.list_writable_sources()? {
            let info = store.get_store_manifest().await?;
            println!("\nStore: {}", info.name);
            println!("  Type: {}", info.store_type);
            println!("  Status: Check individual store capabilities");
        }
    }
    Ok(())
}

async fn handle_publish_extension(
    package_path: PathBuf,
    store_name: String,
    pre_release: bool,
    visibility: VisibilityOption,
    overwrite: bool,
    skip_validation: bool,
    notes: Option<String>,
    tags: Option<String>,
    token: Option<String>,
    timeout: u64,
    dev: bool,
    config: &RegistryConfig,
) -> eyre::Result<()> {
    let Some(mut store) = config
        .get_writable_source(&store_name)
        .map_err(|e| eyre::eyre!(e))
        .wrap_err("Failed to get writable store manager")?
    else {
        error!("No writable stores configured in registry");
        return Err(eyre::eyre!("No writable stores configured in registry"));
    };

    info!(
        "Publishing extension from {:?} to store '{}'",
        package_path, store_name
    );

    // Load the extension package
    let package = load_extension_package(&package_path).await?;

    // Create publish options
    let mut options = if dev {
        PublishOptions::dev_defaults()
    } else {
        PublishOptions::production_defaults()
    };

    options.pre_release = pre_release;
    options.visibility = match visibility {
        VisibilityOption::Public => ExtensionVisibility::Public,
        VisibilityOption::Private => ExtensionVisibility::Private,
        VisibilityOption::Unlisted => ExtensionVisibility::Unlisted,
        VisibilityOption::Organization => ExtensionVisibility::Organization,
    };
    options.overwrite_existing = overwrite;
    options.skip_validation = skip_validation;
    options.access_token = token;
    options.timeout = Some(Duration::from_secs(timeout));
    options.release_notes = notes;

    if let Some(tags_str) = tags {
        options.tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
    }

    // Display publishing configuration
    println!("Publishing configuration:");
    println!("  Package: {}", package.manifest.name);
    println!("  Version: {}", package.manifest.version);
    println!("  Store: {}", store_name);
    println!("  Pre-release: {}", pre_release);
    println!("  Visibility: {:?}", visibility);
    println!("  Overwrite: {}", overwrite);
    println!("  Skip validation: {}", skip_validation);

    if !options.tags.is_empty() {
        println!("  Tags: {}", options.tags.join(", "));
    }

    // Actually publish the extension
    match store.publish(package, options).await {
        Ok(publish_result) => {
            println!("✅ Successfully published extension:");
            println!("  Version: {}", publish_result.version);
            println!("  Download URL: {}", publish_result.download_url);
            println!(
                "  Published at: {}",
                publish_result.published_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!("  Publication ID: {}", publish_result.publication_id);
            println!(
                "  Package size: {}",
                format_size(publish_result.package_size)
            );
            println!("  Content hash: {}", publish_result.content_hash);
            if !publish_result.warnings.is_empty() {
                println!("  Warnings:");
                for warning in &publish_result.warnings {
                    println!("    - {}", warning);
                }
            }
        }
        Err(e) => {
            error!("Failed to publish extension: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_unpublish_extension(
    id: String,
    version: String,
    store_name: String,
    reason: Option<String>,
    keep_record: bool,
    notify_users: bool,
    token: Option<String>,
    config: &RegistryConfig,
) -> eyre::Result<()> {
    let Some(store) = config
        .get_writable_source(&store_name)
        .map_err(|e| eyre::eyre!(e))
        .wrap_err("Failed to get writable store manager")?
    else {
        error!("No writable stores configured in registry");
        return Err(eyre::eyre!("No writable stores configured in registry"));
    };

    info!(
        "Unpublishing extension: {} v{} from store: {}",
        id, version, store_name
    );

    println!("Unpublish configuration:");
    println!("  Extension: {}", id);
    println!("  Version: {}", version);
    println!("  Store: {}", store_name);
    println!("  Keep record: {}", keep_record);
    println!("  Notify users: {}", notify_users);

    if let Some(ref r) = reason {
        println!("  Reason: {}", r);
    }

    warn!("Are you sure you want to unpublish this extension version?");
    warn!("This action may break installations that depend on this version.");

    // Create unpublish options
    let unpublish_options = UnpublishOptions {
        access_token: token,
        version: Some(version.clone()),
        reason,
        keep_record,
        notify_users,
    };

    // Actually unpublish the extension
    match store.unpublish(&id, unpublish_options).await {
        Ok(unpublish_result) => {
            println!("✅ Successfully unpublished extension version:");
            println!("  ID: {}", id);
            println!("  Version: {}", unpublish_result.version);
            println!(
                "  Unpublished at: {}",
                unpublish_result.unpublished_at.format("%Y-%m-%d %H:%M:%S")
            );
            println!(
                "  Tombstone created: {}",
                unpublish_result.tombstone_created
            );
            if let Some(users_notified) = unpublish_result.users_notified {
                if users_notified > 0 {
                    println!("  Users notified: {}", users_notified);
                }
            }
        }
        Err(e) => {
            error!("Failed to unpublish extension: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_validate_extension(
    package_path: PathBuf,
    store_name: Option<String>,
    strict: bool,
    verbose: bool,
    _manager: &StoreManager,
) -> eyre::Result<()> {
    info!("Validating extension package at {:?}", package_path);

    // Load the extension package
    let package = load_extension_package(&package_path).await?;

    // Create validator
    let validator = if strict {
        create_strict_validator()
    } else {
        create_default_validator()
    };

    println!("Validating extension package:");
    println!("  Name: {}", package.manifest.name);
    println!("  Version: {}", package.manifest.version);
    println!(
        "  Package has {} files",
        package.assets.len()
            + if !package.wasm_component.is_empty() {
                1
            } else {
                0
            }
    );
    println!(
        "  Validation mode: {}",
        if strict { "Strict" } else { "Standard" }
    );

    if let Some(store) = &store_name {
        println!("  Target store: {}", store);
    }

    // Run validation
    match validator.validate(&package).await {
        Ok(report) => {
            if report.passed {
                println!("✅ Validation passed!");
                println!("  Duration: {:?}", report.validation_duration);
                println!("  Rules run: {}", report.summary.rules_run);

                if !report.issues.is_empty() {
                    println!("  Warnings: {}", report.issues.len());
                    if verbose {
                        for issue in &report.issues {
                            println!("    - {:?}: {}", issue.severity, issue.description);
                        }
                    }
                }
            } else {
                error!("❌ Validation failed!");
                println!("  Duration: {:?}", report.validation_duration);
                println!(
                    "  Critical issues: {}",
                    report
                        .issues
                        .iter()
                        .filter(|i| matches!(
                            i.severity,
                            quelle_store::registry::IssueSeverity::Critical
                        ))
                        .count()
                );

                if verbose || report.summary.has_blocking_failures {
                    for issue in &report.issues {
                        let icon = match issue.severity {
                            quelle_store::registry::IssueSeverity::Critical => "🚨",
                            quelle_store::registry::IssueSeverity::Warning => "⚠️",
                            quelle_store::registry::IssueSeverity::Info => "ℹ️",
                            quelle_store::registry::IssueSeverity::Error => "❌",
                        };
                        println!("  {} {:?}: {}", icon, issue.severity, issue.description);
                    }
                }
            }

            if verbose {
                println!("\nDetailed validation report:");
                for (rule_name, rule_result) in &report.rule_results {
                    let status = if rule_result.issues.is_empty() {
                        "PASSED"
                    } else {
                        "FAILED"
                    };

                    println!("  {}: {} ({:?})", rule_name, status, rule_result.duration);

                    if !rule_result.issues.is_empty() {
                        for issue in &rule_result.issues {
                            println!("    - {}", issue.description);
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("Validation error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn load_extension_package(package_path: &PathBuf) -> eyre::Result<ExtensionPackage> {
    if !package_path.exists() {
        return Err(eyre::eyre!(
            "Package path does not exist: {:?}",
            package_path
        ));
    }

    if package_path.is_dir() {
        info!(
            "Loading extension package from directory: {:?}",
            package_path
        );

        // Try to load from directory with manifest
        match ExtensionPackage::from_directory(package_path, "cli".to_string()).await {
            Ok(package) => {
                info!(
                    "Successfully loaded package '{}' v{} from directory",
                    package.manifest.name, package.manifest.version
                );
                return Ok(package);
            }
            Err(e) => {
                warn!("Failed to load from directory with manifest: {}", e);

                // Fallback: look for a .wasm file in the directory
                if let Ok(entries) = std::fs::read_dir(package_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(extension) = path.extension() {
                            if extension == "wasm" {
                                info!("Found WASM file, attempting to load: {:?}", path);
                                return ExtensionPackage::from_wasm_file(path, "cli".to_string())
                                    .await
                                    .map_err(|e| {
                                        eyre::eyre!("Failed to load from WASM file: {}", e)
                                    });
                            }
                        }
                    }
                }

                return Err(eyre::eyre!("Could not load package from directory: {}", e));
            }
        }
    } else {
        info!("Loading extension package from file: {:?}", package_path);

        // Check if it's a WASM file
        if let Some(extension) = package_path.extension() {
            if extension == "wasm" {
                info!("Loading package from WASM file using engine metadata extraction");
                return ExtensionPackage::from_wasm_file(package_path, "cli".to_string())
                    .await
                    .map_err(|e| eyre::eyre!("Failed to create package from WASM file: {}", e));
            }
        }

        // Handle other package formats
        if let Some(extension) = package_path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            match ext_str.as_str() {
                "zip" => {
                    error!("ZIP package support not yet implemented");
                    return Err(eyre::eyre!(
                        "ZIP packages are not yet supported. Please extract the package and use the directory, or use a .wasm file directly."
                    ));
                }
                "tar" | "gz" | "tgz" => {
                    error!("TAR/GZ package support not yet implemented");
                    return Err(eyre::eyre!(
                        "TAR/GZ packages are not yet supported. Please extract the package and use the directory, or use a .wasm file directly."
                    ));
                }
                _ => {
                    error!("Unknown file extension: {}", ext_str);
                    return Err(eyre::eyre!(
                        "Unsupported file type '{}'. Currently supported: .wasm files and directories with manifest.json",
                        ext_str
                    ));
                }
            }
        } else {
            error!("File has no extension");
            return Err(eyre::eyre!(
                "File has no extension. Currently supported: .wasm files and directories with manifest.json"
            ));
        }
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}
