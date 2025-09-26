use std::path::PathBuf;

use clap::Subcommand;
use quelle_store::{
    InstallOptions, SearchQuery, SearchSortBy, StoreManager, UpdateOptions, local::LocalStore,
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
    // Git {
    // /// Git repository URL
    // url: String,
    // /// Branch to use
    // #[arg(long, default_value = "main")]
    // branch: String,
    // /// Custom name for the store
    // #[arg(long)]
    // name: Option<String>,
    // },
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

            let local_store = LocalStore::new(&path)?;
            manager.add_store(local_store);

            println!("✓ Added local store '{}' successfully", store_name);
        } // StoreType::Git { url, branch, name } => {
          //     let store_name = name.unwrap_or_else(|| {
          //         url.split('/')
          //             .last()
          //             .unwrap_or("git")
          //             .trim_end_matches(".git")
          //             .to_string()
          //     });

          //     info!(
          //         "Adding git store '{}' from URL: {} (branch: {})",
          //         store_name, url, branch
          //     );

          //     // Git store implementation would go here
          //     println!("✗ Git stores not yet implemented");
          // }
    }
    Ok(())
}

async fn handle_list_stores(manager: &StoreManager) -> eyre::Result<()> {
    let stores = manager.list_stores();

    if stores.is_empty() {
        println!("No stores configured.");
        return Ok(());
    }

    println!("Configured stores:");
    println!(
        "{:<20} {:<10} {:<10} {:<50}",
        "NAME", "TYPE", "TRUSTED", "URL"
    );
    println!("{}", "-".repeat(90));

    for store in stores {
        let info = store.store_info();
        println!(
            "{:<20} {:<10} {:<10} {:<50}",
            info.name,
            info.store_type,
            if info.trusted { "Yes" } else { "No" },
            info.url.as_deref().unwrap_or("-")
        );
    }

    Ok(())
}

async fn handle_remove_store(name: String, manager: &mut StoreManager) -> eyre::Result<()> {
    if manager.remove_store(&name) {
        println!("✓ Removed store '{}'", name);
    } else {
        println!("✗ Store '{}' not found", name);
    }
    Ok(())
}

async fn handle_health_check(manager: &mut StoreManager) -> eyre::Result<()> {
    println!("Checking store health...");

    let failed_stores = manager.refresh_stores().await?;
    let all_stores = manager.list_stores();

    println!("{:<20} {:<10} {:<20}", "STORE", "STATUS", "EXTENSIONS");
    println!("{}", "-".repeat(50));

    for store in all_stores {
        let info = store.store_info();
        let _is_failed = failed_stores.contains(&info.name);

        match store.health_check().await {
            Ok(health) if health.healthy => {
                println!(
                    "{:<20} {:<10} {:<20}",
                    info.name,
                    "✓ Healthy",
                    health
                        .extension_count
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
            Ok(health) => {
                println!(
                    "{:<20} {:<10} {:<20}",
                    info.name,
                    "✗ Unhealthy",
                    health.error.as_deref().unwrap_or("Unknown error")
                );
            }
            Err(e) => {
                println!("{:<20} {:<10} {:<20}", info.name, "✗ Error", e);
            }
        }
    }

    if !failed_stores.is_empty() {
        warn!("{} stores are experiencing issues", failed_stores.len());
    }

    Ok(())
}

async fn handle_search(
    query_text: String,
    author: Option<String>,
    tags: Option<String>,
    sort: SortOption,
    limit: usize,
    manager: &StoreManager,
) -> eyre::Result<()> {
    let tags_vec = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let mut query = SearchQuery::new()
        .with_text(query_text)
        .with_tags(tags_vec)
        .sort_by(sort.into())
        .limit(limit);

    if let Some(author_name) = author {
        query = query.with_author(author_name);
    }

    println!("Searching extensions...");

    let results = manager.search_all_stores(&query).await?;

    if results.is_empty() {
        println!("No extensions found matching your search.");
        return Ok(());
    }

    println!("Found {} extension(s):", results.len());
    println!(
        "{:<20} {:<10} {:<15} {:<50}",
        "NAME", "VERSION", "AUTHOR", "DESCRIPTION"
    );
    println!("{}", "-".repeat(95));

    for ext in results {
        println!(
            "{:<20} {:<10} {:<15} {:<50}",
            ext.name,
            ext.version,
            ext.author,
            ext.description
                .as_deref()
                .unwrap_or("-")
                .chars()
                .take(47)
                .collect::<String>()
        );
    }

    Ok(())
}

async fn handle_list_extensions(manager: &StoreManager) -> eyre::Result<()> {
    println!("Loading extensions from all stores...");

    let extensions = manager.list_all_extensions().await?;

    if extensions.is_empty() {
        println!("No extensions found in any configured store.");
        return Ok(());
    }

    println!("Available extensions ({} total):", extensions.len());
    println!(
        "{:<25} {:<10} {:<15} {:<10} {:<30}",
        "NAME", "VERSION", "AUTHOR", "STORE", "DESCRIPTION"
    );
    println!("{}", "-".repeat(100));

    for ext in extensions {
        println!(
            "{:<25} {:<10} {:<15} {:<10} {:<30}",
            ext.name,
            ext.version,
            ext.author,
            ext.store_source,
            ext.description
                .as_deref()
                .unwrap_or("-")
                .chars()
                .take(27)
                .collect::<String>()
        );
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
    let options = InstallOptions {
        install_dependencies: install_deps,
        allow_downgrades: false,
        force_reinstall: force,
        skip_verification: false,
        target_directory: None,
    };

    println!("Installing extension '{}'...", name);

    match manager
        .install(&name, version.as_deref(), Some(options))
        .await
    {
        Ok(installed) => {
            println!(
                "✓ Successfully installed {}@{} from store '{}'",
                installed.name, installed.version, installed.installed_from
            );

            if install_deps && !installed.dependencies.is_empty() {
                println!("  Dependencies: {}", installed.dependencies.join(", "));
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
    _prerelease: bool,
    force: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    let options = UpdateOptions {
        include_prereleases: _prerelease,
        update_dependencies: true,
        force_update: force,
        backup_current: true,
    };

    if name == "all" {
        println!("Checking for updates to all installed extensions...");

        match manager.update_all(Some(options)).await {
            Ok(results) => {
                let mut updated_count = 0;
                let mut failed_count = 0;

                for result in results {
                    match result {
                        Ok(updated) => {
                            println!("✓ Updated {} to version {}", updated.name, updated.version);
                            updated_count += 1;
                        }
                        Err(e) => {
                            println!("✗ Update failed: {}", e);
                            failed_count += 1;
                        }
                    }
                }

                println!(
                    "Update complete: {} updated, {} failed",
                    updated_count, failed_count
                );
            }
            Err(e) => {
                error!("Failed to update extensions: {}", e);
                return Err(e.into());
            }
        }
    } else {
        println!("Updating extension '{}'...", name);

        match manager.update(&name, Some(options)).await {
            Ok(updated) => {
                println!("✓ Updated {} to version {}", updated.name, updated.version);
            }
            Err(e) => {
                error!("Failed to update extension '{}': {}", name, e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

async fn handle_uninstall_extension(
    name: String,
    remove_files: bool,
    manager: &mut StoreManager,
) -> eyre::Result<()> {
    println!("Uninstalling extension '{}'...", name);

    match manager.uninstall(&name, remove_files).await {
        Ok(()) => {
            println!("✓ Successfully uninstalled '{}'", name);
        }
        Err(e) => {
            error!("Failed to uninstall extension '{}': {}", name, e);
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_list_installed(manager: &StoreManager) -> eyre::Result<()> {
    let installed = manager.list_installed().await;

    if installed.is_empty() {
        println!("No extensions installed.");
        return Ok(());
    }

    println!("Installed extensions ({} total):", installed.len());
    println!(
        "{:<20} {:<10} {:<15} {:<12} {:<20}",
        "NAME", "VERSION", "STORE", "SIZE", "INSTALLED"
    );
    println!("{}", "-".repeat(77));

    for (_, ext) in installed {
        let size_str = if ext.install_size > 0 {
            format_size(ext.install_size)
        } else {
            "-".to_string()
        };

        println!(
            "{:<20} {:<10} {:<15} {:<12} {:<20}",
            ext.name,
            ext.version,
            ext.installed_from,
            size_str,
            ext.installed_at.format("%Y-%m-%d %H:%M")
        );
    }

    Ok(())
}

async fn handle_extension_info(name: String, manager: &StoreManager) -> eyre::Result<()> {
    // Check if installed first
    if let Some(installed) = manager.get_installed(&name).await {
        println!("Extension: {}", installed.name);
        println!("Version: {}", installed.version);
        println!("Author: {}", installed.manifest.author);
        println!("Installed from: {}", installed.installed_from);
        println!("Install path: {}", installed.install_path.display());
        println!(
            "Installed at: {}",
            installed.installed_at.format("%Y-%m-%d %H:%M")
        );

        if installed.install_size > 0 {
            println!("Size: {}", format_size(installed.install_size));
        }

        if !installed.dependencies.is_empty() {
            println!("Dependencies: {}", installed.dependencies.join(", "));
        }

        println!("Languages: {}", installed.manifest.langs.join(", "));
        println!("Base URLs: {}", installed.manifest.base_urls.join(", "));
    }

    // Also show available versions
    match manager.get_extension_info(&name).await {
        Ok(versions) if !versions.is_empty() => {
            println!("\nAvailable versions:");
            for version in versions.iter().take(5) {
                println!(
                    "  {} - {} (from {})",
                    version.version, version.author, version.store_source
                );
            }

            if versions.len() > 5 {
                println!("  ... and {} more versions", versions.len() - 5);
            }
        }
        Ok(_) => {
            if manager.get_installed(&name).await.is_none() {
                println!("Extension '{}' not found in any store.", name);
            }
        }
        Err(e) => {
            warn!("Failed to fetch extension info: {}", e);
        }
    }

    Ok(())
}

async fn handle_check_updates(manager: &StoreManager) -> eyre::Result<()> {
    println!("Checking for updates...");

    match manager.check_all_updates().await {
        Ok(updates) => {
            if updates.is_empty() {
                println!("All extensions are up to date.");
            } else {
                println!("Updates available for {} extension(s):", updates.len());
                println!(
                    "{:<20} {:<12} {:<12} {:<10}",
                    "NAME", "CURRENT", "LATEST", "STORE"
                );
                println!("{}", "-".repeat(54));

                for update in updates {
                    let indicator = if update.security_update {
                        " 🔒"
                    } else if update.breaking_changes {
                        " ⚠️"
                    } else {
                        ""
                    };

                    println!(
                        "{:<20} {:<12} {:<12} {:<10}{}",
                        update.extension_name,
                        update.current_version,
                        update.latest_version,
                        update.store_source,
                        indicator
                    );
                }

                println!("\nLegend: 🔒 = Security update, ⚠️ = Breaking changes");
                println!("Run 'quelle extension update all' to update all extensions.");
            }
        }
        Err(e) => {
            error!("Failed to check for updates: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
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
