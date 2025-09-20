use std::sync::Arc;

use quelle_engine::{ExtensionEngine, error, http::HeadlessChromeExecutor};

fn main() -> error::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let engine = ExtensionEngine::new(Arc::new(HeadlessChromeExecutor::new()))?;
    let runner = engine
        .new_runner_from_file("target/wasm32-unknown-unknown/release/extension_scribblehub.wasm")?;

    let (runner, extension_meta) = runner.meta()?;
    println!("Extension: {:?}", extension_meta);

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <url>", args[0]);
        std::process::exit(1);
    }

    let url = &args[1];
    let (_runner, result) = runner.fetch_novel_info(url)?;

    println!("Novel: {:?}", result);

    Ok(())
}
