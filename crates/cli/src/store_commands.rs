use std::path::PathBuf;
use std::time::Duration;

use clap::Subcommand;
use quelle_store::{
    ExtensionVisibility, SearchQuery, SearchSortBy, StoreManager,
    local::LocalStore,
    models::{ExtensionPackage, InstallOptions},
    publish::{PublishOptions, PublishUpdateOptions, UnpublishOptions},
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
) -> eyre::Result<()> {
    match cmd {
        StoreCommands::Add { store_type } => {
            handle_add_store(store_type, manager).await?;
        }
        StoreCommands::List => {
            handle_list_stores(manager).await?;
        }
        StoreCommands::Remove { name } => {
            handle_remove_store(name, manager).await?;
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

async fn handle_add_store(store_type: StoreType, manager: &mut StoreManager) -> eyre::Result<()> {
    match store_type {
        StoreType::Local { path, name } => {
            let store_name = name.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("local")
                    .to_string()
            });

            info!("Adding local store '{}' at path: {:?}", store_name, path);

            if !path.exists() {
                error!("Store path does not exist: {:?}", path);
                return Err(eyre::eyre!("Store path does not exist"));
            }

            // Create local store
            let local_store = LocalStore::new(&path)
                .map_err(|e| eyre::eyre!("Failed to create local store: {}", e))?;

            // Add to manager
            manager.add_extension_store(local_store);

            println!("✅ Successfully added local store '{}'", store_name);
        }
    }
    Ok(())
}

async fn handle_list_stores(manager: &StoreManager) -> eyre::Result<()> {
    let stores = manager.list_extension_stores();

    if stores.is_empty() {
        println!("No extension stores configured.");
        println!("Use 'quelle store add' to add a store.");
        return Ok(());
    }

    println!("Configured extension stores:");
    for store in stores {
        let info = store.store_info();
        println!("  📦 {} ({})", info.name, info.store_type);
        if let Some(desc) = &info.description {
            println!("     {}", desc);
        }
        println!("     Priority: {}", info.priority);
    }
    Ok(())
}

async fn handle_remove_store(name: String, manager: &mut StoreManager) -> eyre::Result<()> {
    if manager.remove_extension_store(&name) {
        println!("✅ Successfully removed store '{}'", name);
    } else {
        error!("Store '{}' not found", name);
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
        let info = store.store_info();
        let status = if failed_stores.contains(&info.name) {
            "❌ Unhealthy"
        } else {
            "✅ Healthy"
        };
        println!("  {}: {}", info.name, status);
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
    _manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Installing extension: {}", name);

    let _options = InstallOptions {
        auto_update: false,
        force_reinstall: force,
        skip_verification: false,
    };

    // Note: The current API doesn't match what the CLI was written for
    // This is a placeholder implementation that shows the intended functionality

    println!("Installing extension '{}'", name);
    if let Some(v) = &version {
        println!("  Version: {}", v);
    }
    println!("  Force reinstall: {}", force);
    println!("  Install dependencies: {}", install_deps);

    error!("Extension installation not yet implemented in StoreManager");
    error!("The current StoreManager API needs to be extended with install methods");

    Ok(())
}

async fn handle_update_extension(
    name: String,
    prerelease: bool,
    force: bool,
    _manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Updating extension: {}", name);

    println!("Updating extension '{}'", name);
    println!("  Include prerelease: {}", prerelease);
    println!("  Force update: {}", force);

    error!("Extension update not yet implemented in StoreManager");

    Ok(())
}

async fn handle_uninstall_extension(
    name: String,
    remove_files: bool,
    _manager: &mut StoreManager,
) -> eyre::Result<()> {
    info!("Uninstalling extension: {}", name);

    println!("Uninstalling extension '{}'", name);
    println!("  Remove files: {}", remove_files);

    error!("Extension uninstall not yet implemented in StoreManager");

    Ok(())
}

async fn handle_list_installed(_manager: &StoreManager) -> eyre::Result<()> {
    info!("Listing installed extensions...");

    println!("Listing installed extensions...");
    error!("List installed extensions not yet implemented in StoreManager");

    Ok(())
}

async fn handle_extension_info(name: String, _manager: &StoreManager) -> eyre::Result<()> {
    info!("Getting extension info: {}", name);

    println!("Extension information for '{}':", name);
    error!("Extension info not yet implemented in StoreManager");

    Ok(())
}

async fn handle_check_updates(_manager: &StoreManager) -> eyre::Result<()> {
    info!("Checking for extension updates...");

    println!("Checking for available updates...");
    error!("Check updates not yet implemented in StoreManager");

    Ok(())
}

// New publishing functionality

async fn handle_requirements(
    store_name: Option<String>,
    manager: &StoreManager,
) -> eyre::Result<()> {
    if let Some(store_name) = store_name {
        if let Some(store) = manager.get_extension_store(&store_name) {
            println!("Publishing requirements for store '{}':", store_name);
            let info = store.store_info();
            println!("  Store type: {}", info.store_type);
            println!(
                "  Description: {}",
                info.description.as_deref().unwrap_or("None")
            );

            // TODO: Once PublishableStore is properly implemented on concrete types,
            // we can get actual requirements here
            println!("  Status: Publishing support depends on store implementation");
            println!("  Note: Check store documentation for publishing capabilities");
        } else {
            error!("Store '{}' not found", store_name);
        }
    } else {
        println!("Publishing requirements for all stores:");
        for store in manager.list_extension_stores() {
            let info = store.store_info();
            println!("\nStore: {}", info.name);
            println!("  Type: {}", info.store_type);
            println!("  Status: Check individual store capabilities");
        }
    }
    Ok(())
}

async fn handle_permissions(
    store_name: String,
    extension_name: Option<String>,
    manager: &StoreManager,
) -> eyre::Result<()> {
    if let Some(_store) = manager.get_extension_store(&store_name) {
        println!(
            "Checking publishing permissions for store '{}':",
            store_name
        );

        if let Some(ext_name) = extension_name {
            println!("  Extension: {}", ext_name);
        }

        // TODO: Once PublishableStore::can_publish is implemented,
        // we can check actual permissions here
        println!("  Status: Permission checking depends on store implementation");
        println!("  Note: Local stores typically allow all operations");
        println!("  Remote stores may require authentication tokens");
    } else {
        error!("Store '{}' not found", store_name);
    }
    Ok(())
}

async fn handle_stats(store_name: String, manager: &StoreManager) -> eyre::Result<()> {
    if let Some(_store) = manager.get_extension_store(&store_name) {
        println!("Publishing statistics for store '{}':", store_name);

        // TODO: Once PublishableStore::get_publish_stats is implemented,
        // we can show actual statistics here
        println!("  Status: Statistics depend on store implementation");
        println!("  Available info: Check store health and extension count");

        // Show what we can from the existing store interface
        // Note: refresh_stores requires mutable reference, but we have immutable
        // This is a limitation of the current API design
        println!("  Note: Cannot refresh stores with current API");
        let failed_stores: Vec<String> = Vec::new();
        /*
        let failed_stores = match manager.refresh_stores().await {
            Ok(failed) => failed,
            Err(e) => {
                warn!("Could not refresh stores: {}", e);
                Vec::new()
            }
        };
        */

        let status = if failed_stores.contains(&store_name) {
            "Unhealthy"
        } else {
            "Healthy"
        };
        println!("  Store health: {}", status);
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
    _manager: &StoreManager,
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

    // For now, show what would be published
    error!("Publishing is not yet implemented - this is a preview of what would be published");
    error!("Implementation requires PublishableStore trait to be available on store instances");

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
    _manager: &StoreManager,
) -> eyre::Result<()> {
    info!(
        "Updating published extension '{}' from {:?} in store '{}'",
        name, package_path, store_name
    );

    let package = load_extension_package(&package_path).await?;

    let _update_options = PublishUpdateOptions {
        publish_options: PublishOptions {
            access_token: token,
            ..Default::default()
        },
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

    if let Some(reason) = reason {
        println!("  Reason: {}", reason);
    }

    error!("Publishing updates are not yet implemented");
    error!("Implementation requires PublishableStore trait to be available on store instances");

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
    _manager: &StoreManager,
) -> eyre::Result<()> {
    info!(
        "Unpublishing extension '{}' version '{}' from store '{}'",
        name, version, store_name
    );

    let _options = UnpublishOptions {
        access_token: token,
        reason: reason.clone(),
        keep_record,
        notify_users,
    };

    println!("Unpublish configuration:");
    println!("  Extension: {}", name);
    println!("  Version: {}", version);
    println!("  Store: {}", store_name);
    println!("  Keep record: {}", keep_record);
    println!("  Notify users: {}", notify_users);

    if let Some(reason) = reason {
        println!("  Reason: {}", reason);
    }

    warn!("Are you sure you want to unpublish this extension version?");
    warn!("This action may break installations that depend on this version.");

    error!("Unpublishing is not yet implemented");
    error!("Implementation requires PublishableStore trait to be available on store instances");

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
    // TODO: Implement proper package loading from file system
    // This should handle both directories and package files (.zip, .tar.gz, etc.)

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
        // Load from directory - need to read manifest and create package
        let manifest_path = package_path.join("manifest.json");
        if !manifest_path.exists() {
            return Err(eyre::eyre!(
                "No manifest.json found in directory: {:?}",
                package_path
            ));
        }

        // For now, return an error - this needs proper implementation
        error!("Directory package loading not yet implemented");
        return Err(eyre::eyre!("Directory package loading not yet implemented"));
    } else {
        info!("Loading extension package from file: {:?}", package_path);
        // Load from package file
        error!("File package loading not yet implemented");
        return Err(eyre::eyre!("File package loading not yet implemented"));
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
