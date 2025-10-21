//! Publish command handlers for extension publication and validation.

use std::path::PathBuf;
use std::time::Duration;

use eyre::{Context, Result};
use quelle_store::{
    ExtensionVisibility, RegistryConfig, StoreManager,
    manager::{PublishOptions, UnpublishOptions},
    models::ExtensionPackage,
    registry::create_default_validator,
};
use tracing::{error, info, warn};

use crate::cli::{PublishCommands, VisibilityOption};

pub async fn handle_publish_command(
    cmd: PublishCommands,
    config: &RegistryConfig,
    manager: &mut StoreManager,
) -> Result<()> {
    match cmd {
        PublishCommands::Extension {
            package_path,
            store,
            visibility,
            overwrite,
            skip_validation,
            timeout,
            dev,
        } => {
            handle_publish_extension(
                package_path,
                store,
                visibility,
                overwrite,
                skip_validation,
                timeout,
                dev,
                config,
            )
            .await
        }
        PublishCommands::Unpublish { id, version, store } => {
            handle_unpublish_extension(id, version, store, config).await
        }
        PublishCommands::Validate {
            package_path,
            store,
            strict,
            verbose,
        } => handle_validate_extension(package_path, store, strict, verbose, manager).await,
        PublishCommands::Requirements { store } => handle_requirements(store, config).await,
    }
}

async fn handle_publish_extension(
    package_path: PathBuf,
    store_name: String,
    visibility: VisibilityOption,
    overwrite: bool,
    skip_validation: bool,
    timeout: u64,
    dev: bool,
    config: &RegistryConfig,
) -> Result<()> {
    let Some(store) = config
        .get_writable_source(&store_name)
        .map_err(|e| eyre::eyre!(e))
        .context("Failed to get writable store manager")?
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

    let mut options = if dev {
        PublishOptions::dev_defaults()
    } else {
        PublishOptions::production_defaults()
    };

    options.visibility = match visibility {
        VisibilityOption::Public => ExtensionVisibility::Public,
        VisibilityOption::Private => ExtensionVisibility::Private,
        VisibilityOption::Unlisted => ExtensionVisibility::Unlisted,
    };
    options.overwrite_existing = overwrite;
    options.skip_validation = skip_validation;
    options.timeout = Some(Duration::from_secs(timeout));

    // Display publishing configuration
    println!("Publishing configuration:");
    println!("  Package: {}", package.manifest.name);
    println!("  Version: {}", package.manifest.version);
    println!("  Store: {}", store_name);
    println!("  Visibility: {:?}", visibility);
    println!("  Overwrite: {}", overwrite);
    println!("  Skip validation: {}", skip_validation);

    // Store package name before moving package to publish
    let package_name = package.manifest.name.clone();

    // Actually publish the extension
    match store.publish(package, options).await {
        Ok(publish_result) => {
            println!("Published {} v{}", package_name, publish_result.version);
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
    config: &RegistryConfig,
) -> Result<()> {
    let Some(store) = config
        .get_writable_source(&store_name)
        .map_err(|e| eyre::eyre!(e))
        .context("Failed to get writable store manager")?
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

    warn!("Are you sure you want to unpublish this extension version?");
    warn!("This action may break installations that depend on this version.");

    let unpublish_options = UnpublishOptions {
        version: Some(version.clone()),
    };

    // Actually unpublish the extension
    match store.unpublish(&id, unpublish_options).await {
        Ok(unpublish_result) => {
            println!("Unpublished {} v{}", id, unpublish_result.version);
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
) -> Result<()> {
    info!("Validating extension package at {:?}", package_path);

    let package = load_extension_package(&package_path).await?;

    let validator = create_default_validator();

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
                println!("Validation passed");
                if !report.issues.is_empty() {
                    println!("Warnings: {}", report.issues.len());
                    if verbose {
                        for issue in &report.issues {
                            println!("  {:?}: {}", issue.severity, issue.description);
                        }
                    }
                }
            } else {
                error!("Validation failed!");
                println!("  Duration: {:?}", report.validation_duration);
                println!(
                    "  Critical issues: {}",
                    report
                        .issues
                        .iter()
                        .filter(|i| matches!(i.severity, quelle_store::IssueSeverity::Critical))
                        .count()
                );

                for issue in &report.issues {
                    if matches!(
                        issue.severity,
                        quelle_store::IssueSeverity::Critical | quelle_store::IssueSeverity::Error
                    ) {
                        let icon = match issue.severity {
                            quelle_store::IssueSeverity::Critical => "[CRITICAL]",
                            quelle_store::IssueSeverity::Warning => "[WARNING]",
                            quelle_store::IssueSeverity::Info => "[INFO]",
                            quelle_store::IssueSeverity::Error => "[ERROR]",
                        };
                        println!("  {} {:?}: {}", icon, issue.severity, issue.description);
                    }
                }
            }

            if verbose {
                println!("\nValidation completed in {:?}", report.validation_duration);
                if report.passed {
                    println!("All validation rules passed ✓");
                } else {
                    println!("Validation failed ✗");
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

async fn handle_requirements(store_name: Option<String>, config: &RegistryConfig) -> Result<()> {
    if let Some(store_name) = store_name {
        let writable_store = config
            .get_writable_source(&store_name)
            .map_err(|e| eyre::eyre!(e))
            .context("Failed to get writable store configuration")?;

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

async fn load_extension_package(package_path: &PathBuf) -> Result<ExtensionPackage> {
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
                Ok(package)
            }
            Err(e) => {
                warn!("Failed to load from directory with manifest: {}", e);

                // Fallback: look for a .wasm file in the directory
                if let Ok(entries) = std::fs::read_dir(package_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(extension) = path.extension()
                            && extension == "wasm"
                        {
                            info!("Found WASM file, attempting to load: {:?}", path);
                            return ExtensionPackage::from_wasm_file(path, "cli".to_string())
                                .await
                                .map_err(|e| eyre::eyre!("Failed to load from WASM file: {}", e));
                        }
                    }
                }

                Err(eyre::eyre!("Could not load package from directory: {}", e))
            }
        }
    } else {
        info!("Loading extension package from file: {:?}", package_path);

        // Check if it's a WASM file
        if let Some(extension) = package_path.extension()
            && extension == "wasm"
        {
            info!("Loading package from WASM file using engine metadata extraction");
            return ExtensionPackage::from_wasm_file(package_path, "cli".to_string())
                .await
                .map_err(|e| eyre::eyre!("Failed to create package from WASM file: {}", e));
        }

        // Handle other package formats
        if let Some(extension) = package_path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            match ext_str.as_str() {
                "zip" => {
                    error!("ZIP package support not yet implemented");
                    Err(eyre::eyre!(
                        "ZIP packages are not yet supported. Please extract the package and use the directory, or use a .wasm file directly."
                    ))
                }
                "tar" | "gz" | "tgz" => {
                    error!("TAR/GZ package support not yet implemented");
                    Err(eyre::eyre!(
                        "TAR/GZ packages are not yet supported. Please extract the package and use the directory, or use a .wasm file directly."
                    ))
                }
                _ => {
                    error!("Unknown file extension: {}", ext_str);
                    Err(eyre::eyre!(
                        "Unsupported file type '{}'. Currently supported: .wasm files and directories with manifest.json",
                        ext_str
                    ))
                }
            }
        } else {
            error!("File has no extension");
            Err(eyre::eyre!(
                "File has no extension. Currently supported: .wasm files and directories with manifest.json"
            ))
        }
    }
}
