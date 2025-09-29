use eyre::Result;
use std::io::{self, Write};

use crate::cli::ConfigCommands;
use crate::config::Config;

pub async fn handle_config_command(cmd: ConfigCommands, dry_run: bool) -> Result<()> {
    match cmd {
        ConfigCommands::Set { key, value } => handle_set_config(key, value, dry_run).await,
        ConfigCommands::Get { key } => handle_get_config(key).await,
        ConfigCommands::Show => handle_show_config().await,
        ConfigCommands::Reset { force } => handle_reset_config(force, dry_run).await,
    }
}

async fn handle_set_config(key: String, value: String, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would set config: {} = {}", key, value);
        return Ok(());
    }

    let mut config = Config::load().await?;

    match config.set_value(&key, &value) {
        Ok(_) => {
            config.save().await?;
            println!("✅ Configuration updated: {} = {}", key, value);
        }
        Err(e) => {
            println!("❌ Failed to set configuration: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn handle_get_config(key: String) -> Result<()> {
    let config = Config::load().await?;

    match config.get_value(&key) {
        Ok(value) => {
            println!("{}: {}", key, value);
        }
        Err(e) => {
            println!("❌ Failed to get configuration: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn handle_show_config() -> Result<()> {
    let config = Config::load().await?;
    println!("{}", config.show_all());
    Ok(())
}

async fn handle_reset_config(force: bool, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would reset configuration to defaults");
        return Ok(());
    }

    if !force {
        print!("Are you sure you want to reset all configuration? (y/N): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().to_lowercase().starts_with('y') {
            println!("❌ Cancelled");
            return Ok(());
        }
    }

    Config::reset().await?;
    println!("✅ Configuration reset to defaults");
    Ok(())
}
