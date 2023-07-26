use log::{trace, LevelFilter};
use quelle_core::prelude::ExtensionConfig;
use quelle_engine::Runtime;
use std::{error, path::Path};

fn main() -> Result<(), Box<dyn error::Error>> {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .filter_module("quelle_engine", LevelFilter::Trace)
        .parse_default_env()
        .init();

    trace!("initializing the wasm engine...");
    let mut runner = Runtime::new(Path::new("extensions/extension_royalroad.wasm"))?;

    let memloc = unsafe { runner.meta_memloc() }?;
    println!("{:?}", &memloc);

    runner.setup(&ExtensionConfig {
        level_filter: LevelFilter::Trace,
    })?;

    // runner.setup(LevelFilter::Trace)?;

    // info!("Calling exposed wasm 'meta' function");
    // let meta = runner.meta()?;
    // println!("{meta:#?}");

    // info!("Calling exposed wasm 'fetch_novel' function");
    // let novel = runner.fetch_novel("https://www.royalroad.com/fiction/21220/mother-of-learning")?;
    // println!("{novel:#?}");

    // info!("Calling exposed wasm 'fetch_chapter_content' function");
    // let content = runner.fetch_chapter_content("https://www.royalroad.com/fiction/21220/mother-of-learning/chapter/301778/1-good-morning-brother")?;
    // println!("{content:#?}");
    Ok(())
}
