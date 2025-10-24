use clap::Parser;
use tracing_subscriber::EnvFilter;

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

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    handle_command(cli.command).await?;

    Ok(())
}
