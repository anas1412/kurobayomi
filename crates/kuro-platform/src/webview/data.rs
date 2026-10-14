//! `clear_data` (T-12a-034): removes the WebKit data and cache directories. Cookies are not touched here
//! (T-12a-035 owns the cookie jar).

use std::io;
use std::path::PathBuf;

use crate::paths::Paths;

/// The WebKit data directory under the app data dir (`paths.data()/webkit`).
pub fn webkit_data_dir(paths: &dyn Paths) -> PathBuf {
    paths.web_pages_data_dir()
}

/// The WebKit cache directory under the app cache dir (`paths.cache()/webkit`).
pub fn webkit_cache_dir(paths: &dyn Paths) -> PathBuf {
    paths.web_pages_cache_dir()
}

/// Deletes the WebKit data and cache directories. A directory that does not exist yet is not an error.
/// The failure is logged at error level and returned.
pub fn clear_data(paths: &dyn Paths) -> io::Result<()> {
    for dir in [webkit_data_dir(paths), webkit_cache_dir(paths)] {
        if let Err(err) = remove_if_present(&dir) {
            tracing::error!(path = %dir.display(), error = %err, "Error occurred while clearing WebView data");
            return Err(err);
        }
    }
    Ok(())
}

fn remove_if_present(dir: &std::path::Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux::paths::{LinuxPaths, Roots};

    fn paths_under(base: &std::path::Path) -> LinuxPaths {
        LinuxPaths::new(Roots {
            data: base.join("data"),
            config: base.join("config"),
            cache: base.join("cache"),
            state: base.join("state"),
            home: base.join("home"),
            downloads: base.join("home/Downloads"),
        })
    }

    #[test]
    fn removes_both_webkit_directories_and_keeps_siblings() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_under(dir.path());
        std::fs::create_dir_all(webkit_data_dir(&paths).join("cookies")).unwrap();
        std::fs::create_dir_all(webkit_cache_dir(&paths)).unwrap();
        std::fs::write(paths.data_dir().join("test.db"), b"db").unwrap();

        clear_data(&paths).unwrap();

        assert!(!webkit_data_dir(&paths).exists());
        assert!(!webkit_cache_dir(&paths).exists());
        assert!(paths.data_dir().join("test.db").exists());
    }

    #[test]
    fn missing_directories_are_success() {
        let dir = tempfile::tempdir().unwrap();
        assert!(clear_data(&paths_under(dir.path())).is_ok());
    }

    #[test]
    fn a_file_where_the_directory_belongs_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_under(dir.path());
        std::fs::create_dir_all(paths.data_dir()).unwrap();
        std::fs::write(webkit_data_dir(&paths), b"not a directory").unwrap();
        assert!(clear_data(&paths).is_err());
    }
}
