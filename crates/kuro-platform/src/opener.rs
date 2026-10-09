//! Hands a path to the desktop's default handler. A folder opens in the file manager.

use std::io;
use std::path::Path;

use crate::process::spawn_detached;

/// Opens `path` with the desktop's default handler and does not wait for it.
pub fn open(path: &Path) -> io::Result<()> {
    spawn_detached(Path::new("xdg-open"), &[path.as_os_str()])
}
