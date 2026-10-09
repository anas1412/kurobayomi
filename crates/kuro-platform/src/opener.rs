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

    /// Shows `path` in the file manager. The default opens its folder, which every desktop can do; an implementation
    /// that can select the file itself overrides it.
    fn reveal(&self, path: &Path) -> io::Result<()> {
        let folder = path.parent().unwrap_or(path);
        self.open_target(&folder.to_string_lossy())
    }
}

/// The `xdg-open` implementation of [`Opener`].
#[derive(Debug, Default, Clone, Copy)]
pub struct XdgOpener;

impl Opener for XdgOpener {
    fn open_target(&self, target: &str) -> io::Result<()> {
        spawn_detached(Path::new("xdg-open"), &[std::ffi::OsStr::new(target)])
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl Opener for Recorder {
        fn open_target(&self, target: &str) -> io::Result<()> {
            self.0.lock().unwrap().push(target.to_owned());
            Ok(())
        }
    }

    #[test]
    fn reveal_opens_the_folder_of_the_file() {
        let rec = Recorder::default();
        rec.reveal(Path::new("/home/u/Downloads/Naruto/12/001.cbz")).unwrap();
        assert_eq!(*rec.0.lock().unwrap(), vec!["/home/u/Downloads/Naruto/12".to_owned()]);
    }

    #[test]
    fn reveal_of_a_root_path_opens_the_path_itself() {
        let rec = Recorder::default();
        rec.reveal(Path::new("/")).unwrap();
        assert_eq!(*rec.0.lock().unwrap(), vec!["/".to_owned()]);
    }
}
