mod cli;

use std::sync::Arc;

use clap::Parser;
use quelle_engine::{ExtensionEngine, bindings::SimpleSearchQuery, http::HeadlessChromeExecutor};

use crate::cli::Commands;

fn main() -> eyre::Result<()> {
    let cli = cli::Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let engine = ExtensionEngine::new(Arc::new(HeadlessChromeExecutor::new()))?;
    let runner = engine
        .new_runner_from_file("target/wasm32-unknown-unknown/release/extension_scribblehub.wasm")?;

    match cli.command {
        Commands::Novel { url } => {
            let (runner, extension_meta) = runner.meta()?;
            println!("Extension: {:?}", extension_meta);

            let (_runner, result) = runner.fetch_novel_info(url.as_str())?;

            println!("Novel: {:?}", result);
        }
        Commands::Chapter { url } => {
            let (runner, extension_meta) = runner.meta()?;
            println!("Extension: {:?}", extension_meta);

            let (_runner, result) = runner.fetch_chapter(url.as_str())?;

            println!("Chapter: {:?}", result);
        }
        Commands::Search { query } => {
            let (runner, extension_meta) = runner.meta()?;
            println!("Extension: {:?}", extension_meta);

            let (_runner, result) = runner.simple_search(&SimpleSearchQuery {
                query,
                limit: None,
                page: None,
            })?;

            println!("Search Result: {:?}", result);
        }
    }

    Ok(())
}
