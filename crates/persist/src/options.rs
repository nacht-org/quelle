use std::path::PathBuf;

#[derive(Debug)]
pub struct PersistOptions {
    pub base_dir: PathBuf,
    pub global_path: PathBuf,
    pub novel: NovelOptions,
}

#[derive(Debug)]
pub struct NovelOptions {
    pub dir: PathBuf,
    pub filename: PathBuf,
    pub events: PathBuf,
}

impl PersistOptions {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }
}

impl Default for PersistOptions {
    fn default() -> Self {
        let base_dir = PathBuf::from("data");
        Self {
            global_path: base_dir.join("global.json"),
            novel: NovelOptions {
                dir: base_dir.join("novels"),
                filename: PathBuf::from("novel.json"),
                events: PathBuf::from("log.jsonl"),
            },
            base_dir,
        }
    }
}
