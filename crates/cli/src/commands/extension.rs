//! Extension management command handlers.

use eyre::Result;
use quelle_store::StoreManager;
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
            prerelease,
            force,
        } => handle_update_extension(id, prerelease, force, store_manager, dry_run).await,
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
    version: Option<String>,
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
    match store_manager.install(&id, version.as_deref(), None).await {
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
    prerelease: bool,
    force: bool,
    store_manager: &mut StoreManager,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        if id == "all" {
            println!("Would update all extensions");
        } else {
            println!("Would update extension: {}", id);
        }
        return Ok(());
    }

    if id == "all" {
        let installed = store_manager.list_installed().await?;
        if installed.is_empty() {
            println!("No extensions installed");
            return Ok(());
        }

        let updated_count = 0;
        let failed_count = 0;

        for ext in installed {
            println!("Checking {}... update not implemented", ext.name);
            // TODO: Implement actual update checking once store supports it
        }

        println!(
            "Complete: {} updated, {} failed",
            updated_count, failed_count
        );
    } else {
        match store_manager.get_installed(&id).await? {
            Some(installed) => {
                println!("Current: {} v{}", installed.name, installed.version);
                println!("Update checking not implemented");
            }
            None => {
                println!("Extension not installed: {}", id);
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
