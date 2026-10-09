//! Chapter cache cleared on launch (T-08a-037, preference `auto_clear_chapter_cache`, default off). The caller runs
//! [`clear_on_launch`] once, after the main window is shown, with the page cache directory.

use std::io;
use std::path::Path;

/// Empties `cache_dir` when `enabled` is set and returns `true` when it ran. The directory itself stays. A missing
/// directory is nothing to clear. An entry that cannot be removed is logged and skipped, and does not stop the rest.
pub fn clear_on_launch(enabled: bool, cache_dir: &Path) -> bool {
    if !enabled {
        return false;
    }
    match std::fs::read_dir(cache_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let removed = if is_dir { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
                if let Err(e) = removed {
                    tracing::warn!(path = %path.display(), error = %e, "could not clear a cache entry");
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(path = %cache_dir.display(), error = %e, "could not read the cache directory"),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_leaves_every_file_in_place() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("page.jpg"), b"x").unwrap();
        assert!(!clear_on_launch(false, dir.path()));
        assert!(dir.path().join("page.jpg").exists());
    }

    #[test]
    fn enabled_empties_the_directory_and_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("page.jpg"), b"x").unwrap();
        std::fs::create_dir(dir.path().join("chapter")).unwrap();
        std::fs::write(dir.path().join("chapter").join("1.jpg"), b"y").unwrap();
        assert!(clear_on_launch(true, dir.path()));
        assert!(dir.path().is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_missing_directory_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(clear_on_launch(true, &dir.path().join("absent")));
        assert!(!dir.path().join("absent").exists());
    }
}
