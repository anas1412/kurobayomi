//! Replacing the running AppImage with a newer file (the app's own update, `appimage_update` in the GTK crate).

use std::io;
use std::path::{Path, PathBuf};

/// The AppImage file this process runs from: the AppImage runtime sets `APPIMAGE` to its path. `None` for any other build.
pub fn appimage_file() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|path| path.is_file())
}

/// Makes `new` executable and moves it over `target`. Keep both in one folder so the move is atomic; the running copy keeps
/// its old file until it exits.
pub fn replace_file(new: &Path, target: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(new, target)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_new_file_replaces_the_old_one_and_is_executable() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("App.AppImage"), dir.path().join("App.AppImage.update"));
        std::fs::write(&old, "old").unwrap();
        std::fs::write(&new, "new").unwrap();
        replace_file(&new, &old).unwrap();
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "new");
        assert!(!new.exists());
        assert_eq!(std::fs::metadata(&old).unwrap().permissions().mode() & 0o111, 0o111);
    }
}
