//! Status command handlers for system health and configuration reporting.

use eyre::Result;
use quelle_store::StoreManager;

/// Handle the status command - show system status and health
pub async fn handle_status_command(store_manager: &StoreManager) -> Result<()> {
    let stores = store_manager.list_extension_stores();
    println!("Stores: {}", stores.len());

    if stores.is_empty() {
        println!("No stores configured");
        return Ok(());
    }

    for store in stores {
        let info = store.config();
        print!("{}: ", info.store_name);

        match store.store().health_check().await {
            Ok(health) => {
                if health.healthy {
                    print!("OK");
                    if let Some(count) = health.extension_count {
                        print!(" ({} extensions)", count);
                    }
                    println!();
                } else {
                    println!("Error");
                    if let Some(error) = &health.error {
                        println!("  {}", error);
                    }
                }
            }
            Err(e) => {
                println!("Health check failed: {}", e);
            }
        }
    }

    match store_manager.list_installed().await {
        Ok(installed) => {
            println!("Installed extensions: {}", installed.len());
        }
        Err(e) => {
            println!("Could not count extensions: {}", e);
        }
    }

    Ok(())
}
