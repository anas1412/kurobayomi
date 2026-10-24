//! The folders a reveal may show besides the app's own (`router::reveal_path_ok`): the folders of files the app itself
//! just wrote and named in a notice, such as a backup in a folder the user chose.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Folders of files the app itself just wrote and announced. They join the folders a reveal may name
/// (`notify::GtkHost::reveal_roots`), so the file the notice names can be shown while a path from anywhere else is still
/// refused.
static ANNOUNCED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
const MAX_ANNOUNCED: usize = 16;

/// Lets a reveal name files in the folder of `file`, which the app has just written.
pub fn allow_folder_of(file: &Path) {
    let Some(folder) = file.parent() else { return };
    let mut folders = ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner());
    folders.retain(|known| known != folder);
    folders.push(folder.to_path_buf());
    if folders.len() > MAX_ANNOUNCED {
        folders.remove(0);
    }
}

/// The folders [`allow_folder_of`] has recorded.
pub fn announced_folders() -> Vec<PathBuf> {
    ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::router::reveal_path_ok;

    #[test]
    fn a_path_outside_the_allowed_roots_is_refused_when_revealed() {
        // T-35a-023: the reveal is checked against the allowed roots (design 6.1).
        use crate::testing::abs;
        let roots = vec![PathBuf::from(abs("/home/u/downloads"))];
        assert!(reveal_path_ok(Path::new(&abs("/home/u/downloads/manga/page.png")), &roots));
        assert!(!reveal_path_ok(Path::new(&abs("/home/u/.ssh/page.png")), &roots));
    }

    #[test]
    fn a_saved_file_makes_its_folder_revealable_and_nothing_else() {
        // T-35a-023, T-35a-024
        allow_folder_of(Path::new("/home/u/Pictures/Manga/page.png"));
        assert!(announced_folders().contains(&PathBuf::from("/home/u/Pictures/Manga")));
        assert!(!announced_folders().contains(&PathBuf::from("/home/u/.ssh")));
    }
}
