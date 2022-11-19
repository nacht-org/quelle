use std::{
    collections::HashMap,
    fs::{self, File},
};

use anyhow::{anyhow, bail};
use fenster_engine::Runner;
use itertools::Itertools;
use serde::Serialize;

#[derive(Serialize, Debug)]
struct Lock {
    version: usize,
    extensions: HashMap<String, Extension>,
}

#[derive(Serialize, Debug)]
struct Extension {
    pub name: String,
    pub version: String,
    pub base_urls: Vec<String>,
    pub lang: String,
    pub path: String,
}

pub fn lock() -> anyhow::Result<()> {
    let mut extensions = HashMap::new();

    for entry in fs::read_dir("dist")? {
        let entry = entry?;
        let mut runner = Runner::new(&entry.path()).map_err(|e| anyhow!(e.to_string()))?;
        let meta = runner.meta().map_err(|e| anyhow!(e.to_string()))?;

        if let Some(Extension { name, .. }) = extensions.get(&meta.id) {
            bail!("both '{}' and '{}' have the same id", name, &meta.name);
        }

        let extension = Extension {
            name: meta.name,
            version: meta.version.into_iter().join("."),
            base_urls: meta.base_urls,
            lang: meta.lang,
            path: entry.path().as_os_str().to_string_lossy().to_string(),
        };

        extensions.insert(meta.id, extension);
    }

    let lock = Lock {
        version: 1,
        extensions,
    };

    {
        let mut file = File::options()
            .write(true)
            .create(true)
            .open("dist/lock.json")?;

        serde_json::to_writer_pretty(&mut file, &lock)?;
    }

    Ok(())
}