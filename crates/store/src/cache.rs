use crate::error::Result;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

#[derive(Debug)]
pub enum CacheDir {
    Temp(TempDir),
    Path(PathBuf),
}

impl CacheDir {
    pub fn new_temp() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        Ok(CacheDir::Temp(temp_dir))
    }

    pub fn path(&self) -> &Path {
        match self {
            CacheDir::Temp(temp_dir) => temp_dir.path(),
            CacheDir::Path(path_buf) => path_buf.as_path(),
        }
    }
}

impl From<PathBuf> for CacheDir {
    fn from(path_buf: PathBuf) -> Self {
        CacheDir::Path(path_buf)
    }
}

impl From<&Path> for CacheDir {
    fn from(path: &Path) -> Self {
        CacheDir::Path(path.to_path_buf())
    }
}
