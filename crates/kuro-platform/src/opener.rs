//! Hands a path to the desktop's default handler. A folder opens in the file manager.

use std::io;
use std::path::Path;

use crate::process::spawn_detached;

/// Opens `path` with the desktop's default handler and does not wait for it.
pub fn open(path: &Path) -> io::Result<()> {
    spawn_detached(Path::new("xdg-open"), &[path.as_os_str()])
}

/// Opens a URL or a path with the desktop handler. Callers hold an `Opener` so tests can record the calls.
pub trait Opener: Send + Sync {
    /// Starts the handler for `target` and does not wait for it.
    fn open_target(&self, target: &str) -> io::Result<()>;
}

/// The `xdg-open` implementation of [`Opener`].
#[derive(Debug, Default, Clone, Copy)]
pub struct XdgOpener;

impl Opener for XdgOpener {
    fn open_target(&self, target: &str) -> io::Result<()> {
        spawn_detached(Path::new("xdg-open"), &[std::ffi::OsStr::new(target)])
    }
}
