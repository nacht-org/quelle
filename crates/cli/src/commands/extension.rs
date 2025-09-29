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

    println!("📦 Installing extension: {}", id);

    // Check if already installed
    if !force
        && let Some(installed) = store_manager.get_installed(&id).await? {
            println!(
                "⚠️ Extension {} v{} is already installed",
                installed.name, installed.version
            );
            println!("💡 Use --force to reinstall");
            return Ok(());
        }

    // Install the extension
    match store_manager.install(&id, version.as_deref(), None).await {
        Ok(installed) => {
            println!("✅ Installed {} v{}", installed.name, installed.version);
        }
        Err(e) => {
            eprintln!("❌ Failed to install {}: {}", id, e);
            return Err(e.into());
        }
    }
    Ok(())
}

async fn handle_list_extensions(detailed: bool, store_manager: &mut StoreManager) -> Result<()> {
    let installed = store_manager.list_installed().await?;

    if installed.is_empty() {
        println!("📦 No extensions installed");
        println!("💡 Use 'quelle extension search <query>' to find extensions");
        return Ok(());
    }

    println!("📦 Installed extensions ({}):", installed.len());
    for ext in installed {
        if detailed {
            println!("  📦 {} v{}", ext.name, ext.version);
            println!("     ID: {}", ext.id);
            println!(
                "     Installed: {}",
                ext.installed_at.format("%Y-%m-%d %H:%M")
            );
            if !ext.source_store.is_empty() {
                println!("     Source: {}", ext.source_store);
            }
            println!();
        } else {
            println!("  📦 {} v{} - {}", ext.name, ext.version, ext.id);
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
        println!("📦 Updating all extensions...");

        let installed = store_manager.list_installed().await?;
        if installed.is_empty() {
            println!("📦 No extensions installed");
            return Ok(());
        }

        let updated_count = 0;
        let failed_count = 0;

        for ext in installed {
            print!("📦 Checking {} for updates...", ext.name);
            io::stdout().flush()?;

            // For now, just report that we would check for updates
            println!(" 🚧 Update checking not yet implemented");
            // TODO: Implement actual update checking once store supports it
        }

        println!(
            "📊 Update complete: {} updated, {} failed",
            updated_count, failed_count
        );
    } else {
        println!("📦 Updating extension: {}", id);

        match store_manager.get_installed(&id).await? {
            Some(installed) => {
                println!("  Current version: {}", installed.version);
                println!("  🚧 Update checking not yet implemented");
                println!("  💡 This would check for newer versions and install if available");

                if prerelease {
                    println!("  📋 Would include pre-release versions");
                }
                if force {
                    println!("  🔧 Would force update even if no newer version");
                }
            }
            None => {
                println!("❌ Extension not installed: {}", id);
                println!("💡 Use 'quelle extension install {}' to install it", id);
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
                    println!("❌ Cancelled");
                    return Ok(());
                }
            }

            match store_manager.uninstall(&id).await {
                Ok(_) => {
                    println!("✅ Removed extension: {}", installed.name);
                }
                Err(e) => {
                    eprintln!("❌ Failed to remove {}: {}", installed.name, e);
                    return Err(e.into());
                }
            }
        }
        None => {
            println!("❌ Extension not installed: {}", id);
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

    println!("🔍 Searching for extensions: {}", query);

    // Build search query
    let _search_query = quelle_store::models::SearchQuery::new()
        .with_text(query)
        .with_author(author.unwrap_or_default())
        .limit(limit);

    println!("🚧 Extension search across stores is not yet fully implemented");
    println!("💡 This would search across all configured extension stores");
    println!("💡 Currently only local installed extensions are supported");
    Ok(())
}

async fn handle_extension_info(id: String, store_manager: &mut StoreManager) -> Result<()> {
    match store_manager.get_installed(&id).await? {
        Some(ext) => {
            println!("📦 {}", ext.name);
            println!("ID: {}", ext.id);
            println!("Version: {}", ext.version);
            println!(
                "Installed: {}",
                ext.installed_at.format("%Y-%m-%d %H:%M:%S")
            );

            if !ext.source_store.is_empty() {
                println!("Source: {}", ext.source_store);
            }

            // Show manifest information if available
            println!("\nManifest Information:");
            println!("  Name: {}", ext.manifest.name);
            println!("  Version: {}", ext.manifest.version);
            println!("  Author: {}", ext.manifest.author);

            if !ext.manifest.langs.is_empty() {
                println!("  Languages: {}", ext.manifest.langs.join(", "));
            }
        }
        None => {
            println!("❌ Extension not installed: {}", id);
            println!("💡 Use 'quelle extension search {}' to find it", id);
        }
    }
    Ok(())
}
