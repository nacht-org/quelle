//! Store management command handlers for extension repositories.

use eyre::{Context, Result};
use quelle_store::stores::local::LocalStore;
use quelle_store::{BaseStore, ExtensionSource, GitStore, RegistryConfig, StoreManager, StoreType};
use std::io::{self, Write};
use std::path::PathBuf;

use quelle_store::{GitAuth, GitReference};

use crate::{
    cli::{AddStoreCommands, StoreCommands},
    config::Config,
};

pub async fn handle_store_command(
    cmd: StoreCommands,
    config: &mut Config,
    store_manager: &mut StoreManager,
) -> Result<()> {
    match cmd {
        StoreCommands::Add { store_type } => {
            handle_add_store(store_type, config, store_manager).await
        }
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
    store_type: AddStoreCommands,
    config: &mut Config,
    store_manager: &mut StoreManager,
) -> Result<()> {
    let (name, source) = match store_type {
        AddStoreCommands::Local {
            name,
            path,
            priority,
        } => {
            let source = handle_add_local_store(name.clone(), path, priority, config).await?;
            (name, source)
        }
        AddStoreCommands::Git {
            name,
            url,
            priority,
            branch,
            tag,
            commit,
            token,
            ssh_key,
            ssh_pub_key,
            ssh_passphrase,
            username,
            password,
            cache_dir,
        } => {
            let source = handle_add_git_store(
                name.clone(),
                url,
                priority,
                branch,
                tag,
                commit,
                token,
                ssh_key,
                ssh_pub_key,
                ssh_passphrase,
                username,
                password,
                cache_dir,
                config,
            )
            .await?;
            (name, source)
        }
    };

    // Check if store already exists
    if config.registry.has_source(&name) {
        println!("Store '{}' already exists", name);
        return Ok(());
    }

    config.registry.add_source(source);

    // Save CLI configuration
    config.save().await?;

    println!("Added store '{}'", name);

    // Try to apply the updated registry config to store manager
    // If it fails (e.g., store doesn't have proper manifest), warn but don't fail
    store_manager.clear_extension_stores().await?;
    if let Err(e) = config.registry.apply(store_manager).await {
        println!("Warning: Store added but could not be loaded: {}", e);
    }

    Ok(())
}

async fn handle_add_local_store(
    name: String,
    path: Option<String>,
    priority: u32,
    config: &Config,
) -> Result<ExtensionSource> {
    let store_path = if let Some(path) = path {
        let provided_path = PathBuf::from(&path);
        if !provided_path.exists() {
            return Err(eyre::eyre!(
                "Local path does not exist: {}",
                provided_path.display()
            ));
        }
        provided_path
    } else {
        // Default to data_dir/stores/name
        let mut default_path = config.get_data_dir();
        default_path.push("stores");
        default_path.push(&name);

        // Create the default directory if it doesn't exist
        if !default_path.exists() {
            println!("📂 Creating store directory: {}", default_path.display());
            std::fs::create_dir_all(&default_path).map_err(|e| {
                eyre::eyre!(
                    "Failed to create store directory '{}': {}",
                    default_path.display(),
                    e
                )
            })?;
        }
        default_path
    };

    // If the directory exists but is empty, initialize it as a store
    if store_path.is_file() {
        return Err(eyre::eyre!(
            "Path '{}' is a file, expected a directory",
            store_path.display()
        ));
    }

    let is_empty = store_path.read_dir()?.next().is_none();
    if is_empty {
        println!(
            "📂 Initializing empty directory as a local store: {}",
            store_path.display()
        );

        let local_store = LocalStore::new(&store_path)
            .map_err(|e| eyre::eyre!("Failed to create local store: {}", e))?;

        local_store
            .initialize_store(name.clone(), None)
            .await
            .map_err(|e| eyre::eyre!("Failed to initialize store: {}", e))?;
    } else {
        println!(
            "📂 Using existing directory as local store: {}",
            store_path.display()
        );

        let local_store = LocalStore::new(&store_path)
            .map_err(|e| eyre::eyre!("Failed to create local store: {}", e))?;

        // Validate existing store - don't write anything to it
        match local_store.health_check().await {
            Ok(health) => {
                if !health.healthy {
                    let error_msg = health.error.unwrap_or_default();
                    tracing::error!("Existing store validation failed: {}", error_msg);
                    return Err(eyre::eyre!("Store validation failed: {}", error_msg));
                }

                if let Some(count) = health.extension_count {
                    tracing::info!("Validated existing store with {} extensions", count);
                } else {
                    tracing::info!("Validated existing store structure");
                }
            }
            Err(e) => {
                tracing::error!("Failed to validate existing store: {}", e);
                return Err(eyre::eyre!("Store validation failed: {}", e));
            }
        }
    }

    // Convert to absolute path to ensure consistency
    let absolute_path = store_path.canonicalize().map_err(|e| {
        eyre::eyre!(
            "Failed to resolve absolute path for '{}': {}",
            store_path.display(),
            e
        )
    })?;

    println!("  Type: Local");
    println!("  Path: {}", absolute_path.display());
    println!("  Priority: {}", priority);

    // Create extension source
    Ok(ExtensionSource::local(name, absolute_path).with_priority(priority))
}

async fn handle_add_git_store(
    name: String,
    url: String,
    priority: u32,
    branch: Option<String>,
    tag: Option<String>,
    commit: Option<String>,
    token: Option<String>,
    ssh_key: Option<String>,
    ssh_pub_key: Option<String>,
    ssh_passphrase: Option<String>,
    username: Option<String>,
    password: Option<String>,
    cache_dir: Option<String>,
    config: &Config,
) -> Result<ExtensionSource> {
    // Validate git reference options (only one should be specified)
    let ref_count = [&branch, &tag, &commit]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if ref_count > 1 {
        return Err(eyre::eyre!(
            "Only one of --branch, --tag, or --commit can be specified"
        ));
    }

    // Create git reference
    let reference = if let Some(branch) = branch {
        GitReference::Branch(branch)
    } else if let Some(tag) = tag {
        GitReference::Tag(tag)
    } else if let Some(commit) = commit {
        GitReference::Commit(commit)
    } else {
        GitReference::Default
    };

    // Validate auth options (only one type should be specified)
    let auth_count = [&token, &ssh_key, &username]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if auth_count > 1 {
        return Err(eyre::eyre!(
            "Only one authentication method can be specified (--token, --ssh-key, or --username)"
        ));
    }

    // Create git authentication
    let auth = if let Some(token) = token {
        GitAuth::Token { token }
    } else if let Some(ssh_key) = ssh_key {
        let private_key_path = PathBuf::from(ssh_key);
        let public_key_path = ssh_pub_key.map(PathBuf::from);
        GitAuth::SshKey {
            private_key_path,
            public_key_path,
            passphrase: ssh_passphrase,
        }
    } else if let Some(username) = username {
        let password = password.ok_or_else(|| {
            eyre::eyre!("Password is required when using username authentication")
        })?;
        GitAuth::UserPassword { username, password }
    } else {
        GitAuth::None
    };

    // Determine cache directory
    let cache_path = if let Some(cache_dir) = cache_dir {
        PathBuf::from(cache_dir)
    } else {
        // Use config's data directory + stores + store name
        let mut cache_path = config.get_data_dir();
        cache_path.push("stores");
        cache_path.push(&name);
        cache_path
    };

    // Ensure cache directory parent exists
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            eyre::eyre!(
                "Failed to create cache directory parent '{}': {}",
                parent.display(),
                e
            )
        })?;
    }

    let git_store = GitStore::builder(url.clone())
        .auth(auth.clone())
        .reference(reference.clone())
        .fetch_interval(std::time::Duration::from_secs(300))
        .shallow(true)
        .writable()
        .cache_dir(cache_path.clone())
        .name(name.clone())
        .build()?;

    git_store
        .ensure_synced()
        .await
        .map_err(|e| eyre::eyre!("Failed to sync git store: {}", e))?;

    let is_empty = cache_path
        .read_dir()?
        .filter_map(|r| r.ok())
        .filter(|e| {
            let file_name = e.file_name();
            let file_name_str = file_name.to_string_lossy();
            file_name_str != ".git" // Ignore .git directory
        })
        .next()
        .is_none();

    if is_empty {
        println!(
            "📂 Initializing empty directory as a git store: {}",
            cache_path.display()
        );

        git_store
            .initialize_store(name.clone(), None)
            .await
            .map_err(|e| eyre::eyre!(e))
            .wrap_err("Failed to initialize git store")?;
    } else {
        println!(
            "📂 Using existing directory as git store cache: {}",
            cache_path.display()
        );

        // Validate existing store - don't write anything to it
        match git_store.health_check().await {
            Ok(health) => {
                if !health.healthy {
                    let error_msg = health.error.unwrap_or_default();
                    tracing::error!("Existing store validation failed: {}", error_msg);
                    return Err(eyre::eyre!("Store validation failed: {}", error_msg));
                }

                if let Some(count) = health.extension_count {
                    tracing::info!("Validated existing store with {} extensions", count);
                } else {
                    tracing::info!("Validated existing store structure");
                }
            }
            Err(e) => {
                tracing::error!("Failed to validate existing store: {}", e);
                return Err(eyre::eyre!("Store validation failed: {}", e));
            }
        }
    }

    println!("  Type: Git");
    println!("  URL: {}", url);
    println!("  Reference: {:?}", reference);
    println!(
        "  Auth: {}",
        match &auth {
            GitAuth::None => "None".to_string(),
            GitAuth::Token { .. } => "Token".to_string(),
            GitAuth::SshKey { .. } => "SSH Key".to_string(),
            GitAuth::UserPassword { username, .. } => format!("Username ({})", username),
        }
    );
    println!("  Cache Dir: {}", cache_path.display());
    println!("  Priority: {}", priority);

    // Create extension source
    Ok(
        ExtensionSource::git_with_config(name, url, cache_path, reference, auth)
            .with_priority(priority),
    )
}

async fn handle_remove_store(
    name: String,
    force: bool,
    config: &mut Config,
    store_manager: &mut StoreManager,
) -> Result<()> {
    // Check if store exists
    if !config.registry.has_source(&name) {
        println!("Store '{}' not found", name);
        return Ok(());
    }

    if !force {
        print!("Are you sure you want to remove store '{}'? (y/N): ", name);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().to_lowercase().starts_with('y') {
            println!("Cancelled");
            return Ok(());
        }
    }

    // Remove the store from CLI configuration
    let removed = config.registry.remove_source(&name);
    if !removed {
        println!("Failed to remove store '{}'", name);
        return Ok(());
    }

    // Save CLI configuration
    config.save().await?;

    println!("Removed store '{}'", name);

    // Try to apply the updated registry config to store manager
    // If it fails, warn but don't fail the removal operation
    store_manager.clear_extension_stores().await?;
    if let Err(e) = config.registry.apply(store_manager).await {
        println!("Warning: Error reloading remaining stores: {}", e);
    }
    Ok(())
}

async fn handle_list_stores(registry_config: &RegistryConfig) -> Result<()> {
    if registry_config.extension_sources.is_empty() {
        println!("No extension stores configured");
        return Ok(());
    }

    println!(
        "Configured extension stores ({}):",
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
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
                if source.trusted {
                    println!("     Trusted: Yes");
                }
            }
            StoreType::Git {
                url,
                cache_dir,
                reference,
                auth,
            } => {
                println!("     URL: {}", url);
                println!("     Cache Dir: {}", cache_dir.display());
                println!("     Reference: {:?}", reference);
                println!(
                    "     Auth: {}",
                    match auth {
                        GitAuth::None => "None".to_string(),
                        GitAuth::Token { .. } => "Token".to_string(),
                        GitAuth::SshKey { .. } => "SSH Key".to_string(),
                        GitAuth::UserPassword { username, .. } =>
                            format!("Username ({})", username),
                    }
                );
                println!(
                    "     Status: {}",
                    if source.enabled {
                        "Enabled"
                    } else {
                        "Disabled"
                    }
                );
                if source.trusted {
                    println!("     Trusted: Yes");
                }
            }
        }
        println!();
    }
    Ok(())
}

async fn handle_update_store(name: String, registry_config: &RegistryConfig) -> Result<()> {
    if name == "all" {
        if registry_config.extension_sources.is_empty() {
            println!("No stores configured");
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
                        println!(" Refreshed");
                        updated_count += 1;
                    }
                    Err(e) => {
                        println!(" Failed: {}", e);
                        failed_count += 1;
                    }
                },
                Ok(None) => {
                    println!(" No caching support");
                    updated_count += 1;
                }
                Err(e) => {
                    println!(" Failed to create store: {}", e);
                    failed_count += 1;
                }
            }
        }

        println!(
            "Refresh complete: {} processed, {} failed",
            updated_count, failed_count
        );
    } else {
        let source = registry_config
            .extension_sources
            .iter()
            .find(|s| s.name == name && s.enabled);

        match source {
            Some(source) => match source.as_cacheable() {
                Ok(Some(cacheable_store)) => match cacheable_store.refresh_cache().await {
                    Ok(_) => {
                        println!("Store '{}' refreshed", name);
                    }
                    Err(e) => {
                        println!("Failed to refresh store '{}': {}", name, e);
                    }
                },
                Ok(None) => {
                    println!("Store '{}' has no caching support", name);
                }
                Err(e) => {
                    println!("Failed to create store '{}': {}", name, e);
                }
            },
            None => {
                println!("Store '{}' not found or disabled", name);
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
            println!("Store: {}", source.name);
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
                StoreType::Git {
                    url,
                    cache_dir,
                    reference,
                    auth,
                } => {
                    println!("URL: {}", url);
                    println!("Cache Dir: {}", cache_dir.display());
                    println!("Cache Exists: {}", cache_dir.exists());
                    println!("Reference: {:?}", reference);
                    println!(
                        "Auth: {}",
                        match auth {
                            GitAuth::None => "None (public repository)".to_string(),
                            GitAuth::Token { .. } => "Token authentication".to_string(),
                            GitAuth::SshKey {
                                private_key_path, ..
                            } => {
                                format!("SSH key ({})", private_key_path.display())
                            }
                            GitAuth::UserPassword { username, .. } => {
                                format!("Username/password ({})", username)
                            }
                        }
                    );
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
                                        "Healthy"
                                    } else {
                                        "Unhealthy"
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
                                println!("Status: Health check failed: {}", e);
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
            println!("Store '{}' not found", name);
            println!("Use 'quelle store list' to see available stores");
        }
    }
    Ok(())
}
