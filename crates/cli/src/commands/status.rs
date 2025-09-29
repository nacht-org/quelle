use eyre::Result;
use quelle_store::StoreManager;

/// Handle the status command - show system status and health
pub async fn handle_status_command(store_manager: &StoreManager) -> Result<()> {
    let stores = store_manager.list_extension_stores();
    println!("📊 Registry Status:");
    println!("  Configured stores: {}", stores.len());

    if stores.is_empty() {
        println!("💡 No stores configured. Add stores with: quelle store add <name> <location>");
        return Ok(());
    }

    for store in stores {
        let info = store.config();
        print!("  📍 {} ({}): ", info.store_name, info.store_type);

        match store.store().health_check().await {
            Ok(health) => {
                if health.healthy {
                    println!("✅ Healthy");
                    if let Some(count) = health.extension_count {
                        println!("    Extensions available: {}", count);
                    }
                    println!(
                        "    Last checked: {}",
                        health.last_check.format("%Y-%m-%d %H:%M")
                    );
                } else {
                    println!("❌ Unhealthy");
                    if let Some(error) = &health.error {
                        println!("    Error: {}", error);
                    }
                }
            }
            Err(e) => {
                println!("❌ Health check failed: {}", e);
            }
        }
    }

    // Show installed extensions count
    match store_manager.list_installed().await {
        Ok(installed) => {
            println!("  📦 Installed extensions: {}", installed.len());
        }
        Err(e) => {
            println!("  📦 Could not count installed extensions: {}", e);
        }
    }

    Ok(())
}
