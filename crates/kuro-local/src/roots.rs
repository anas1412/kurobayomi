//! `manga_root` and `anime_root`: the `Local/Manga` and `Local/Anime` folders under the storage base, created on first access
//! (area:14 behavior section 2). The base comes from `kuro-domain`'s `StorageManager`, which owns the folder names.

use std::path::PathBuf;

use kuro_domain::storage::StorageManager;

/// `<storage base>/Local/Manga`. `None` when the base is missing, so every listing is empty rather than an error.
pub fn manga_root(storage: &StorageManager) -> Option<PathBuf> {
    storage.local_manga_dir().ok()
}

/// `<storage base>/Local/Anime`. `None` when the base is missing.
pub fn anime_root(storage: &StorageManager) -> Option<PathBuf> {
    storage.local_anime_dir().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    #[test]
    fn the_roots_are_the_two_local_folders_and_are_made_on_access() {
        let base = tempfile::tempdir().expect("temp dir");
        let store = PrefStore::in_memory();
        let storage = StorageManager::open(&store, base.path().to_path_buf());
        storage.ensure_tree().expect("tree");
        std::fs::remove_dir(base.path().join("Local/Manga")).expect("remove");
        let manga = manga_root(&storage).expect("manga root");
        assert_eq!(manga, base.path().join("Local/Manga"));
        assert!(manga.is_dir(), "the accessor creates a missing folder");
        assert_eq!(anime_root(&storage), Some(base.path().join("Local/Anime")));
    }

    #[test]
    fn a_missing_base_gives_no_root() {
        let base = tempfile::tempdir().expect("temp dir");
        let store = PrefStore::in_memory();
        let storage = StorageManager::open(&store, base.path().join("not-yet"));
        assert_eq!(manga_root(&storage), None);
        assert_eq!(anime_root(&storage), None);
        assert!(!base.path().join("not-yet").exists(), "opening creates nothing");
    }
}
