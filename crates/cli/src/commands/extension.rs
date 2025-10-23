//! Extension management command handlers.

use eyre::Result;
use quelle_store::{StoreManager, UpdateInfo, UpdateOptions};
use semver::Version;
use std::io::{self, Write};

use crate::cli::ExtensionCommands;

pub async fn handle_extension_command(
    cmd: ExtensionCommands,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        ExtensionCommands::Install { id, version, force } => {
            handle_install_extension(id, version, force, store_manager, dry_run).await
        }
        ExtensionCommands::List { detailed } => {
            handle_list_extensions(detailed, store_manager).await
        }
        ExtensionCommands::Update {
            id,
            force,
            check_only,
        } => handle_update_extension(id, force, check_only, store_manager, dry_run).await,
        ExtensionCommands::Remove { id, force } => {
            handle_remove_extension(id, force, store_manager, dry_run).await
        }
        ExtensionCommands::Search {
            query,
            author,
            limit,
        } => handle_search_extensions(query, author, limit, dry_run).await,
        ExtensionCommands::Info { id } => handle_extension_info(id, store_manager).await,
    }
}

async fn handle_install_extension(
    id: String,
    version: Option<Version>,
    force: bool,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would install extension: {} (version: {:?})", id, version);
        return Ok(());
    }

    if !force && let Some(installed) = store_manager.get_installed(&id).await? {
        println!(
            "Extension {} v{} already installed",
            installed.name, installed.version
        );
        println!("Use --force to reinstall");
        return Ok(());
    }

    // Install the extension
    match store_manager.install(&id, version.as_ref(), None).await {
        Ok(installed) => {
            println!("Installed {} v{}", installed.name, installed.version);
        }
        Err(e) => {
            eprintln!("Failed to install {}: {}", id, e);
            return Err(e.into());
        }
    }
    Ok(())
}

async fn handle_list_extensions(detailed: bool, store_manager: &mut StoreManager) -> Result<()> {
    let installed = store_manager.list_installed().await?;

    if installed.is_empty() {
        println!("No extensions installed");
        return Ok(());
    }

    println!("Installed ({}):", installed.len());
    for ext in installed {
        if detailed {
            println!("  {} v{} ({})", ext.name, ext.version, ext.id);
            println!(
                "    Installed: {}",
                ext.installed_at.format("%Y-%m-%d %H:%M")
            );
            if !ext.source_store.is_empty() {
                println!("    Source: {}", ext.source_store);
            }
        } else {
            println!("  {} v{}", ext.name, ext.version);
        }
    }
    Ok(())
}

async fn handle_update_extension(
    id: String,
    force: bool,
    check_only: bool,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        if id == "all" {
            if check_only {
                println!("Would check for updates for all extensions");
            } else {
                println!("Would update all extensions");
            }
        } else if check_only {
            println!("Would check for updates for extension: {}", id);
        } else {
            println!("Would update extension: {}", id);
        }
        return Ok(());
    }

    if id == "all" {
        handle_update_extension_all(force, check_only, store_manager).await?;
    } else {
        handle_update_extension_specific(id, force, check_only, store_manager).await?;
    }

    Ok(())
}

async fn handle_update_extension_all(
    force: bool,
    check_only: bool,
    store_manager: &mut StoreManager,
) -> eyre::Result<()> {
    let installed = store_manager.list_installed().await?;
    if installed.is_empty() {
        println!("No extensions installed");
        return Ok(());
    }

    println!(
        "Checking for updates for {} extension(s)...",
        installed.len()
    );

    let result = store_manager.check_all_updates().await.map(|updates| {
        updates
            .into_iter()
            .flat_map(|v| match v {
                UpdateInfo::UpdateAvailable(update_available_info) => Some(update_available_info),
                _ => None,
            })
            .collect::<Vec<_>>()
    });

    let updates = match result {
        Ok(updates) => updates,
        Err(e) => {
            eprintln!("Failed to check for updates: {}", e);
            return Err(e.into());
        }
    };

    if updates.is_empty() {
        println!("All extensions are up to date");
        return Ok(());
    }

    println!("Found {} update(s) available:", updates.len());
    for update in &updates {
        println!(
            "  {} {} → {} (from {})",
            update.extension_id, update.current_version, update.latest_version, update.store_source
        );
    }

    if check_only {
        println!("\nTo update all: quelle extension update all");
    } else {
        println!("\nUpdating {} extension(s)...", updates.len());
        let mut success_count = 0;
        let mut failed_count = 0;

        for update in &updates {
            print!("Updating {}...", update.extension_id);
            std::io::stdout().flush().unwrap();

            let update_options = UpdateOptions {
                update_dependencies: false,
                force_update: force,
                backup_current: false,
            };

            match store_manager
                .update(&update.extension_id, Some(update_options))
                .await
            {
                Ok(_) => {
                    println!(" Success");
                    success_count += 1;
                }
                Err(e) => {
                    println!(" Failed: {}", e);
                    failed_count += 1;
                }
            }
        }

        println!(
            "\nUpdate complete: {} succeeded, {} failed",
            success_count, failed_count
        );
    }

    Ok(())
}

async fn handle_update_extension_specific(
    id: String,
    force: bool,
    check_only: bool,
    store_manager: &mut StoreManager,
) -> Result<(), eyre::Error> {
    let Some(installed) = store_manager.get_installed(&id).await? else {
        println!("Extension not installed: {}", id);
        return Ok(());
    };

    println!(
        "Checking updates for {} v{}",
        installed.name, installed.version
    );

    let result = store_manager.check_all_updates().await.map(|updates| {
        updates
            .into_iter()
            .flat_map(|v| match v {
                UpdateInfo::UpdateAvailable(update_available_info) => Some(update_available_info),
                _ => None,
            })
            .collect::<Vec<_>>()
    });

    let updates = match result {
        Ok(updates) => updates,
        Err(e) => {
            eprintln!("Failed to check for updates: {}", e);
            return Err(e.into());
        }
    };

    let Some(update) = updates.iter().find(|u| u.extension_id == id) else {
        println!("{} is up to date", installed.name);
        return Ok(());
    };

    println!(
        "Update available: {} → {} (from {})",
        update.current_version, update.latest_version, update.store_source
    );

    if check_only {
        println!("To update: quelle extension update {}", id);
    } else {
        print!("Updating {}...", update.extension_id);
        std::io::stdout().flush().unwrap();

        let update_options = UpdateOptions {
            update_dependencies: false,
            force_update: force,
            backup_current: false,
        };

        match store_manager.update(&id, Some(update_options)).await {
            Ok(_) => {
                println!(" Successfully updated to v{}", update.latest_version);
            }
            Err(e) => {
                println!(" Update failed: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}

async fn handle_remove_extension(
    id: String,
    force: bool,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would remove extension: {}", id);
        return Ok(());
    }

    match store_manager.get_installed(&id).await? {
        Some(installed) => {
            if !force {
                print!(
                    "Are you sure you want to remove '{}'? (y/N): ",
                    installed.name
                );
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("Cancelled");
                    return Ok(());
                }
            }

            match store_manager.uninstall(&id).await {
                Ok(_) => {
                    println!("Removed: {}", installed.name);
                }
                Err(e) => {
                    eprintln!("Failed to remove {}: {}", installed.name, e);
                    return Err(e.into());
                }
            }
        }
        None => {
            println!("Extension not installed: {}", id);
        }
    }
    Ok(())
}

async fn handle_search_extensions(
    query: String,
    author: Option<String>,
    limit: usize,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would search for extensions: {}", query);
        return Ok(());
    }

    let _search_query = quelle_store::models::SearchQuery::new()
        .with_text(query)
        .with_author(author.unwrap_or_default())
        .limit(limit);

    println!("Extension search not yet implemented");
    Ok(())
}

async fn handle_extension_info(id: String, store_manager: &mut StoreManager) -> Result<()> {
    match store_manager.get_installed(&id).await? {
        Some(ext) => {
            println!("{} v{} ({})", ext.name, ext.version, ext.id);
            println!("Author: {}", ext.manifest.author);
            println!("Installed: {}", ext.installed_at.format("%Y-%m-%d"));

            if !ext.source_store.is_empty() {
                println!("Source: {}", ext.source_store);
            }
            if !ext.manifest.langs.is_empty() {
                println!("Languages: {}", ext.manifest.langs.join(", "));
            }
        }
        None => {
            println!("Extension not installed: {}", id);
        }
    }
    Ok(())
}
