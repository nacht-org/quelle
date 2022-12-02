use std::{
    fs::{self, OpenOptions},
    io::BufWriter,
    path::{Path, PathBuf},
};

use fenster_engine::Runner;
use url::Url;

pub fn compile_epub(url: Url, wasm_path: PathBuf) -> anyhow::Result<()> {
    let mut runner = Runner::new(&wasm_path)?;

    let novel = runner.fetch_novel(url.as_str())?;

    let data_dir = Path::new("data");
    if !data_dir.exists() {
        fs::create_dir(&data_dir)?;
    }

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(data_dir.join("novel.json"))?;

    let file = BufWriter::new(file);
    serde_json::to_writer_pretty(file, &novel)?;

    Ok(())
}