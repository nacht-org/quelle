use clap::Parser;

use crate::{cli::Cli, commands::handle_command};

pub mod cli;
pub mod commands;
pub mod generator;
pub mod http_caching;
pub mod server;
pub mod utils;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else if cli.quiet {
            tracing::Level::ERROR
        } else {
            tracing::Level::WARN
        })
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    handle_command(cli.command).await?;

    Ok(())
}
