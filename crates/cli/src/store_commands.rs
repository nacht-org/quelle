use std::path::PathBuf;
use std::time::Duration;

use clap::Subcommand;
use quelle_store::{
    ConfigStore, ExtensionSource, ExtensionVisibility, SearchQuery, SearchSortBy, Store,
    StoreManager,
    local::LocalStore,
    models::{ExtensionPackage, InstallOptions},
    publish::{PublishOptions, PublishUpdateOptions, UnpublishOptions},
    registry_config::RegistryStoreConfig,
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
    /// Check publishing permissions
    Permissions {
        /// Store name
        store: String,
        /// Extension name (optional)
        #[arg(long)]
        extension: Option<String>,
    },
    /// Show publishing statistics and quotas
    Stats {
        /// Store name
        store: String,
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
        /// Extension name
        name: String,
        /// Specific version to install
        #[arg(long)]
        version: Option<String>,
        /// Force reinstallation
        #[arg(long)]
        force: bool,
        /// Skip dependency installation
        #[arg(long)]
        no_deps: bool,
    },
    /// Update an extension
    Update {
        /// Extension name (or 'all' for all extensions)
        name: String,
        /// Include pre-release versions
        #[arg(long)]
        prerelease: bool,
        /// Force update even if no new version
        #[arg(long)]
        force: bool,
    },
    /// Uninstall an extension
    Uninstall {
        /// Extension name
        name: String,
        /// Remove all files (not just registry entry)
        #[arg(long)]
        remove_files: bool,
    },
    /// List installed extensions
    List,
    /// Show extension information
    Info {
        /// Extension name
        name: String,
    },
    /// Check for available updates
    CheckUpdates,
    /// Publish a new extension version
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
    /// Update an existing published extension
    PublishUpdate {
        /// Extension name
        name: String,
        /// Path to extension package or directory
        package_path: PathBuf,
        /// Target store name
        #[arg(long)]
        store: String,
        /// Update reason
        #[arg(long)]
        reason: Option<String>,
        /// Preserve existing metadata
        #[arg(long)]
        preserve_metadata: bool,
        /// Merge tags instead of replacing
        #[arg(long)]
        merge_tags: bool,
        /// Access token for authentication
        #[arg(long)]
        token: Option<String>,
    },
    /// Remove a published extension version
    Unpublish {
        /// Extension name
        name: String,
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
            handle_requirements(store, manager).await?;
        }
        StoreCommands::Permissions { store, extension } => {
            handle_permissions(store, extension, manager).await?;
        }
        StoreCommands::Stats { store } => {
            handle_stats(store, manager).await?;
        }
    }
    Ok(())
}

pub async fn handle_extension_command(
    cmd: ExtensionCommands,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    match cmd {
        ExtensionCommands::Install {
            name,
            version,
            force,
            no_deps,
        } => {
            handle_install_extension(name, version, force, !no_deps, manager).await?;
        }
        ExtensionCommands::Update {
            name,
            prerelease,
            force,
        } => {
            handle_update_extension(name, prerelease, force, manager).await?;
        }
        ExtensionCommands::Uninstall { name, remove_files } => {
            handle_uninstall_extension(name, remove_files, manager).await?;
        }
        ExtensionCommands::List => {
            handle_list_installed(manager).await?;
        }
        ExtensionCommands::Info { name } => {
            handle_extension_info(name, manager).await?;
        }
        ExtensionCommands::CheckUpdates => {
            handle_check_updates(manager).await?;
        }
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
                manager,
            )
            .await?;
        }
        ExtensionCommands::PublishUpdate {
            name,
            package_path,
            store,
            reason,
            preserve_metadata,
            merge_tags,
            token,
        } => {
            handle_publish_update(
                name,
                package_path,
                store,
                reason,
                preserve_metadata,
                merge_tags,
                token,
                manager,
            )
            .await?;
        }
        ExtensionCommands::Unpublish {
            name,
            version,
            store,
            reason,
            keep_record,
            notify_users,
            token,
        } => {
            handle_unpublish_extension(
                name,
                version,
                store,
                reason,
                keep_record,
                notify_users,
                token,
                manager,
            )
            .await?;
        }
        ExtensionCommands::Validate {
            package_path,
            store,
            strict,
            verbose,
        } => {
            handle_validate_extension(package_path, store, strict, verbose, manager).await?;
        }
    }
    Ok(())
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
    name: String,
    version: Option<String>,
    force: bool,
    install_deps: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Installing extension: {}", name);

    let options = InstallOptions {
        auto_update: false,
        force_reinstall: force,
        skip_verification: false,
    };

    println!("Installing extension '{}'...", name);
    if let Some(v) = &version {
        println!("  Requested version: {}", v);
    }
    println!("  Force reinstall: {}", force);
    println!("  Install dependencies: {}", install_deps);

    match manager
        .install(&name, version.as_deref(), Some(options))
        .await
    {
        Ok(installed) => {
            println!("✅ Successfully installed extension:");
            println!("  Name: {}", installed.name);
            println!("  Version: {}", installed.version);
            println!("  Source store: {}", installed.source_store);
            println!("  Install path: {}", installed.install_path.display());
            if let Some(size) = installed.size {
                println!("  Size: {}", format_size(size));
            }

            if install_deps {
                info!("Note: Dependency installation is not yet implemented");
                println!("  Dependencies: Auto-install not yet supported");
            }
        }
        Err(e) => {
            error!("Failed to install extension '{}': {}", name, e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_update_extension(
    name: String,
    prerelease: bool,
    force: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Updating extension: {}", name);

    println!("Updating extension '{}'...", name);
    println!("  Include prerelease: {}", prerelease);
    println!("  Force update: {}", force);

    // Check if extension is installed
    match manager.get_installed(&name).await? {
        Some(installed) => {
            let options = quelle_store::models::UpdateOptions {
                include_prereleases: prerelease,
                force_update: force,
                ..Default::default()
            };

            match manager.update(&name, Some(options)).await {
                Ok(updated) => {
                    println!("✅ Successfully updated extension:");
                    println!("  Name: {}", updated.name);
                    println!("  Version: {} → {}", installed.version, updated.version);
                    println!("  Source store: {}", updated.source_store);
                    if let Some(size) = updated.size {
                        println!("  Size: {}", format_size(size));
                    }
                    println!(
                        "  Updated: {}",
                        updated
                            .last_updated
                            .unwrap_or(updated.installed_at)
                            .format("%Y-%m-%d %H:%M:%S")
                    );
                }
                Err(e) => {
                    error!("Failed to update extension '{}': {}", name, e);
                    return Err(e.into());
                }
            }
        }
        None => {
            error!("Extension '{}' is not installed", name);
            println!(
                "Run 'quelle extension install {}' to install it first.",
                name
            );
            return Err(eyre::eyre!("Extension '{}' is not installed", name));
        }
    }

    Ok(())
}

async fn handle_uninstall_extension(
    name: String,
    remove_files: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Uninstalling extension: {}", name);

    println!("Uninstalling extension '{}'...", name);
    println!("  Remove files: {}", remove_files);

    // Check if extension is installed
    match manager.get_installed(&name).await? {
        Some(installed) => {
            println!("  Current version: {}", installed.version);
            println!("  Install path: {}", installed.install_path.display());

            match manager.uninstall(&name).await {
                Ok(removed) => {
                    if removed {
                        println!("✅ Successfully uninstalled extension '{}'", name);
                        if remove_files {
                            println!("  Files removed from disk");
                        } else {
                            info!(
                                "Note: Files may remain on disk depending on store configuration"
                            );
                        }
                    } else {
                        error!("Extension '{}' was not found in registry", name);
                        return Err(eyre::eyre!(
                            "Extension '{}' was not found in registry",
                            name
                        ));
                    }
                }
                Err(e) => {
                    error!("Failed to uninstall extension '{}': {}", name, e);
                    return Err(e.into());
                }
            }
        }
        None => {
            error!("Extension '{}' is not installed", name);
            return Err(eyre::eyre!("Extension '{}' is not installed", name));
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
                    println!("  {} v{}", ext.name, ext.version);
                    println!("    Source: {}", ext.source_store);
                    println!(
                        "    Installed: {}",
                        ext.installed_at.format("%Y-%m-%d %H:%M:%S")
                    );
                    if let Some(updated) = ext.last_updated {
                        println!("    Last updated: {}", updated.format("%Y-%m-%d %H:%M:%S"));
                    }
                    if let Some(size) = ext.size {
                        println!("    Size: {}", format_size(size));
                    }
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

async fn handle_extension_info(name: String, manager: &StoreManager) -> eyre::Result<()> {
    info!("Getting extension info: {}", name);

    println!("Extension information for '{}':", name);

    // Check if extension is installed
    match manager.get_installed(&name).await {
        Ok(Some(installed)) => {
            println!("  Status: ✅ Installed");
            println!("  Version: {}", installed.version);
            println!("  Source store: {}", installed.source_store);
            println!(
                "  Installed: {}",
                installed.installed_at.format("%Y-%m-%d %H:%M:%S")
            );
            if let Some(updated) = installed.last_updated {
                println!("  Last updated: {}", updated.format("%Y-%m-%d %H:%M:%S"));
            }
            if let Some(size) = installed.size {
                println!("  Size: {}", format_size(size));
            }
            println!("  Auto-update: {}", installed.auto_update);
            println!("  Install path: {}", installed.install_path.display());
        }
        Ok(None) => {
            println!("  Status: ❌ Not installed");

            // Search for extension in available stores
            println!("  Searching in available stores...");
            match manager.get_extension_info(&name).await {
                Ok(infos) if !infos.is_empty() => {
                    let info = &infos[0]; // Use the first (best) match
                    println!("  Found in store: {}", info.store_source);
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
    manager: &StoreManager,
) -> eyre::Result<()> {
    if let Some(store_name) = store_name {
        if let Some(managed_store) = manager.get_extension_store(&store_name) {
            println!("Publishing requirements for store '{}':", store_name);
            if let Ok(manifest) = managed_store.store().get_store_manifest().await {
                println!("  Store type: {}", manifest.store_type);
                println!(
                    "  Description: {}",
                    manifest.description.as_deref().unwrap_or("None")
                );
            }

            // Get actual publishing requirements if store supports it
            if let Some(requirements) = manager.get_store_publish_requirements(&store_name) {
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
                println!("  Publishing supported: No");
                println!("  Note: This store type does not support publishing operations");
            }
        } else {
            error!("Store '{}' not found", store_name);
        }
    } else {
        println!("Publishing requirements for all stores:");
        for store in manager.list_extension_stores() {
            let info = store.config();
            println!("\nStore: {}", info.store_name);
            println!("  Type: {}", info.store_type);
            println!("  Status: Check individual store capabilities");
        }
    }
    Ok(())
}

async fn handle_permissions(
    store_name: String,
    extension_name: Option<String>,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    if let Some(_managed_store) = manager.get_extension_store(&store_name) {
        println!(
            "Checking publishing permissions for store '{}':",
            store_name
        );

        if let Some(ref ext_name) = extension_name {
            println!("  Extension: {}", ext_name);
        }

        // Check actual publishing permissions if store supports it
        if let Some(permissions_result) = manager
            .check_store_publish_permissions(&store_name, extension_name.as_deref().unwrap_or("*"))
            .await
        {
            match permissions_result {
                Ok(permissions) => {
                    println!("  Publishing supported: Yes");
                    println!("  Can publish: {}", permissions.can_publish);
                    println!("  Can update: {}", permissions.can_update);
                    println!("  Can unpublish: {}", permissions.can_unpublish);

                    if let Some(max_size) = permissions.max_package_size {
                        println!("  Max package size: {} MB", max_size / (1024 * 1024));
                    }

                    if permissions.rate_limits.publications_per_hour.is_some()
                        || permissions.rate_limits.publications_per_day.is_some()
                    {
                        println!("  Rate limits apply: Yes");
                        if let Some(per_hour) = permissions.rate_limits.publications_per_hour {
                            println!("    Max per hour: {}", per_hour);
                        }
                        if let Some(per_day) = permissions.rate_limits.publications_per_day {
                            println!("    Max per day: {}", per_day);
                        }
                    } else {
                        println!("  Rate limits: None");
                    }
                }
                Err(e) => {
                    println!("  Error checking permissions: {}", e);
                }
            }
        } else {
            println!("  Publishing supported: No");
            println!("  Note: This store type does not support publishing operations");
        }
    } else {
        error!("Store '{}' not found", store_name);
    }
    Ok(())
}

async fn handle_stats(store_name: String, manager: &StoreManager) -> eyre::Result<()> {
    if let Some(managed_store) = manager.get_extension_store(&store_name) {
        println!("Publishing statistics for store '{}':", store_name);

        // Get actual publishing statistics if store supports it
        if let Some(stats_result) = manager.get_store_publish_stats(&store_name).await {
            match stats_result {
                Ok(stats) => {
                    println!("  Publishing supported: Yes");
                    println!("  Total extensions: {}", stats.total_extensions);
                    if stats.total_storage_used > 0 {
                        println!(
                            "  Storage used: {} MB",
                            stats.total_storage_used / (1024 * 1024)
                        );
                    }
                    if let Some(quota) = stats.storage_quota {
                        println!("  Storage quota: {} MB", quota / (1024 * 1024));
                    }
                    if stats.recent_publications > 0 {
                        println!("  Recent publications: {}", stats.recent_publications);
                    }

                    // Rate limit status
                    if stats.rate_limit_status.is_limited {
                        println!("  Rate limited: Yes");
                        if let Some(remaining) = stats.rate_limit_status.publications_remaining {
                            println!("    Publications remaining: {}", remaining);
                        }
                        if let Some(reset_time) = stats.rate_limit_status.reset_time {
                            println!("    Reset time: {}", reset_time);
                        }
                    } else {
                        println!("  Rate limited: No");
                    }
                }
                Err(e) => {
                    println!("  Error getting statistics: {}", e);
                }
            }
        } else {
            println!("  Publishing supported: No");
            println!("  Note: This store type does not support publishing operations");
        }

        // Show general store health
        match managed_store.store().health_check().await {
            Ok(health) => {
                println!(
                    "  Store health: {}",
                    if health.healthy {
                        "Healthy"
                    } else {
                        "Unhealthy"
                    }
                );
                if let Some(count) = health.extension_count {
                    println!("  Available extensions: {}", count);
                }
                if let Some(error) = health.error {
                    println!("  Health error: {}", error);
                }
            }
            Err(e) => {
                println!("  Store health: Error checking - {}", e);
            }
        }
    } else {
        error!("Store '{}' not found", store_name);
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
    manager: &mut StoreManager,
) -> eyre::Result<()> {
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
    match manager
        .publish_extension_to_store(&store_name, package, &options)
        .await
    {
        Some(result) => match result {
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
        },
        None => {
            error!(
                "Store '{}' does not support publishing or was not found",
                store_name
            );
            return Err(eyre::eyre!(
                "Store '{}' does not support publishing operations",
                store_name
            ));
        }
    }

    Ok(())
}

async fn handle_publish_update(
    name: String,
    package_path: PathBuf,
    store_name: String,
    reason: Option<String>,
    preserve_metadata: bool,
    merge_tags: bool,
    token: Option<String>,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!(
        "Updating published extension '{}' from {:?} in store '{}'",
        name, package_path, store_name
    );

    let package = load_extension_package(&package_path).await?;

    // Create update options
    let mut publish_options = PublishOptions::production_defaults();
    publish_options.access_token = token.clone();
    let update_options = PublishUpdateOptions {
        publish_options,
        preserve_metadata,
        merge_tags,
        update_reason: reason.clone(),
    };

    println!("Update configuration:");
    println!("  Extension: {}", name);
    println!("  New version: {}", package.manifest.version);
    println!("  Store: {}", store_name);
    println!("  Preserve metadata: {}", preserve_metadata);
    println!("  Merge tags: {}", merge_tags);

    if let Some(ref r) = reason {
        println!("  Reason: {}", r);
    }

    // Actually update the published extension
    match manager
        .update_published_extension_in_store(&store_name, &name, package, &update_options)
        .await
    {
        Some(result) => match result {
            Ok(publish_result) => {
                println!("✅ Successfully updated published extension:");
                println!("  Name: {}", name);
                println!("  New version: {}", publish_result.version);
                println!("  Download URL: {}", publish_result.download_url);
                println!(
                    "  Updated at: {}",
                    publish_result.published_at.format("%Y-%m-%d %H:%M:%S")
                );
                println!("  Publication ID: {}", publish_result.publication_id);
                println!(
                    "  Package size: {}",
                    format_size(publish_result.package_size)
                );
            }
            Err(e) => {
                error!("Failed to update published extension: {}", e);
                return Err(e.into());
            }
        },
        None => {
            error!(
                "Store '{}' does not support publishing or was not found",
                store_name
            );
            return Err(eyre::eyre!(
                "Store '{}' does not support publishing operations",
                store_name
            ));
        }
    }

    Ok(())
}

async fn handle_unpublish_extension(
    name: String,
    version: String,
    store_name: String,
    reason: Option<String>,
    keep_record: bool,
    notify_users: bool,
    token: Option<String>,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!(
        "Unpublishing extension '{}' version '{}' from store '{}'",
        name, version, store_name
    );

    println!("Unpublish configuration:");
    println!("  Extension: {}", name);
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
        reason,
        keep_record,
        notify_users,
    };

    // Actually unpublish the extension
    match manager
        .unpublish_extension_from_store(&store_name, &name, &version, &unpublish_options)
        .await
    {
        Some(result) => match result {
            Ok(unpublish_result) => {
                println!("✅ Successfully unpublished extension version:");
                println!("  Name: {}", name);
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
        },
        None => {
            error!(
                "Store '{}' does not support publishing or was not found",
                store_name
            );
            return Err(eyre::eyre!(
                "Store '{}' does not support publishing operations",
                store_name
            ));
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
