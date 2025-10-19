use std::path::{Path, PathBuf};

use crate::StoreError;

/// Resolves a sibling path relative to the parent directory of `base_path`.
///
/// # Arguments
///
/// * `base_path` - The base path whose parent directory will be used.
/// * `relative_sibling_path` - The path to the sibling file or directory.
///
/// # Returns
///
/// Returns the resolved sibling path as a `PathBuf`, or an error if `base_path` has no parent.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use crate::utils::resolve_sibling_path;
///
/// let base = Path::new("/foo/bar/baz.txt");
/// let sibling = Path::new("qux.txt");
/// let resolved = resolve_sibling_path(base, sibling).unwrap();
/// assert_eq!(resolved, Path::new("/foo/bar/qux.txt"));
/// ```
pub fn resolve_sibling_path(
    base_path: impl AsRef<Path>,
    relative_sibling_path: impl AsRef<Path>,
) -> crate::error::Result<PathBuf> {
    fn inner(base_path: &Path, relative_sibling_path: &Path) -> crate::error::Result<PathBuf> {
        let base_dir = base_path.parent().ok_or_else(|| {
            StoreError::InvalidPath(format!("No parent directory for {}", base_path.display()))
        })?;

        Ok(base_dir.join(relative_sibling_path))
    }
    inner(base_path.as_ref(), relative_sibling_path.as_ref())
}
