//! Platform directory resolution. Callers ask for a role and never build a path from a literal.

use std::path::{Path, PathBuf};
use std::{fs, io};

use url::Url;

/// Directories the app reads and writes, resolved for the current platform.
pub trait Paths {
    /// Downloaded data, databases and other durable app state.
    fn data_dir(&self) -> PathBuf;
    /// User-editable settings.
    fn config_dir(&self) -> PathBuf;
    /// App state that survives restarts but is not user-editable (`state.toml`, logs).
    fn state_dir(&self) -> PathBuf;
    /// Disposable cache data.
    fn cache_dir(&self) -> PathBuf;
    /// Log files.
    fn log_dir(&self) -> PathBuf;
    /// Scratch space for in-progress work.
    fn temp_dir(&self) -> PathBuf;
    /// Crash reports written by the panic hook.
    fn crash_dir(&self) -> PathBuf;
    /// Where downloads go when the user has not picked a folder.
    fn default_downloads_dir(&self) -> PathBuf;
    /// Where backups go when the user has not picked a folder.
    fn default_backups_dir(&self) -> PathBuf;
    /// Cached cover images.
    fn covers_dir(&self) -> PathBuf;
    /// Installed plugins (extensions).
    fn plugin_dir(&self) -> PathBuf;
}

/// The default download folder as a path and as a `file://` URL.
pub struct FolderProvider<'a> {
    paths: &'a dyn Paths,
}

impl<'a> FolderProvider<'a> {
    pub fn new(paths: &'a dyn Paths) -> Self {
        Self { paths }
    }

    /// The default download folder.
    pub fn directory(&self) -> PathBuf {
        self.paths.default_downloads_dir()
    }

    /// The download folder as a `file://` URL. `None` when the folder path is not absolute.
    pub fn url(&self) -> Option<Url> {
        Url::from_file_path(self.directory()).ok()
    }
}

/// A first-run directory could not be created. The message names the path so the crash window explains it.
#[derive(Debug, thiserror::Error)]
#[error("cannot create {}: {source}", path.display())]
pub struct DirError {
    pub path: PathBuf,
    #[source]
    pub source: io::Error,
}

/// Creates the config, state, data and cache directories when they are missing, with mode 0700 on Unix.
/// Existing directories keep their mode. Stops at the first failure.
pub fn create_app_dirs(paths: &dyn Paths) -> Result<(), DirError> {
    for dir in [paths.config_dir(), paths.state_dir(), paths.data_dir(), paths.cache_dir()] {
        create_private_dir(&dir).map_err(|source| DirError { path: dir, source })?;
    }
    Ok(())
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// The raw bytes of a path, so a stored preference can round-trip any OS path, including non-UTF-8 ones (E-STO-12).
#[cfg(unix)]
pub fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

/// The path with exactly these bytes. Off Unix the bytes must be UTF-8, the port's fallback, else `None`.
#[cfg(unix)]
pub fn path_from_bytes(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// The raw bytes of a path. Off Unix the path is stored as UTF-8 (lossy for non-UTF-8 paths).
#[cfg(not(unix))]
pub fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

/// The path with these bytes, or `None` when they are not UTF-8 (off Unix only).
#[cfg(not(unix))]
pub fn path_from_bytes(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(all(test, unix))]
mod byte_path_tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn a_non_utf8_path_round_trips_through_its_bytes() {
        let original = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', b'x', 0xff, b'/', b'y']));
        assert_eq!(path_from_bytes(path_bytes(&original)), Some(original));
    }
}
