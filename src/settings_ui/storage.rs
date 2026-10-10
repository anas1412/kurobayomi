//! Storage location row (tasks T-47a-088 and T-48a-051): the subtitle rule, the folder check, the info text and the
//! "base folder not available" banner. The stored value is the base64 of the path bytes, as `StorageManager` writes
//! it, so a folder chosen here is the one the backup, download and mpv-config code resolve.
use std::path::{Path, PathBuf};
use std::rc::Rc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use kuro_domain::storage::{StorageManager, STORAGE_DIR_KEY};
use kuro_platform::paths::{path_from_bytes, Paths};
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;
use kuro_prefs::store::PrefStore;
use gtk::gio::prelude::FileExt;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::{info, text};

pub const NO_LOCATION: &str = "No storage location set";
pub const INFO_TEXT: &str = "Used for automatic backups, chapter downloads, and local source.";
pub const REFUSED: &str = "Failed to acquire persistent folder access. The app may behave unexpectedly.";

/// `No storage location set`, the stored path when it is usable, or `Invalid location: <path>`.
pub fn subtitle(stored: &str, usable: bool) -> String {
    if stored.is_empty() {
        NO_LOCATION.to_owned()
    } else if usable {
        stored.to_owned()
    } else {
        format!("Invalid location: {stored}")
    }
}

/// The banner shown on the page while a chosen base folder is gone (E-SY-05).
pub fn banner(path: &str) -> String {
    format!("Base folder not available: {path}")
}

/// The folder a stored value names; `None` for an empty value or one that does not decode.
pub fn decode(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() {
        return None;
    }
    STANDARD.decode(raw).ok().and_then(path_from_bytes)
}

/// A stored location is usable when it is a directory that can be listed.
pub fn is_readable_dir(path: &Path) -> bool {
    std::fs::read_dir(path).is_ok()
}

/// A chosen folder is accepted when it is a directory the app can write to (a probe file is created and removed).
pub fn accept_choice(path: &Path) -> Result<(), &'static str> {
    if !path.is_dir() {
        return Err(REFUSED);
    }
    tempfile::tempfile_in(path).map(drop).map_err(|_| REFUSED)
}

/// The storage manager over the stored base folder (the mpv-config tree and the downloads resolve through it).
pub fn manager(store: &PrefStore) -> Option<StorageManager> {
    let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
    Some(StorageManager::open(store, paths.data_dir()))
}

/// Stores `path` as the base folder and creates the mpv-config tree under it. A folder that cannot be used is refused.
pub fn apply_choice(store: &PrefStore, path: &Path) -> Result<(), &'static str> {
    accept_choice(path)?;
    let manager = manager(store).ok_or(REFUSED)?;
    manager.set_base(path).map_err(|err| {
        tracing::warn!(%err, "the storage location could not be saved");
        REFUSED
    })
}

/// Opens the folder chooser; a folder that passes [`apply_choice`] is stored and `changed` runs, any other one is
/// refused with a toast. A cancelled chooser changes nothing.
fn choose_folder(store: PrefStore, changed: Rc<dyn Fn()>) {
    gtk::FileDialog::new().select_folder(Some(&crate::ui::window()), None::<&gtk::gio::Cancellable>, move |result| {
        let Some(path) = result.ok().and_then(|file| file.path()) else { return };
        match apply_choice(&store, &path) {
            Ok(()) => changed(),
            Err(message) => crate::toast::notify(message),
        }
    });
}

/// The stored folder and whether it can be listed; `None` while nothing (decodable) is stored.
fn stored(prefs: &Prefs) -> Option<(String, bool)> {
    let raw = Pref::new(prefs.store().clone(), STORAGE_DIR_KEY, String::new()).get();
    let path = decode(&raw)?;
    Some((path.to_string_lossy().into_owned(), is_readable_dir(&path)))
}

/// The banner row for a chosen base folder that is gone, to put above the page (E-SY-05).
pub fn banner_row(prefs: &Prefs) -> Option<PreferenceItem> {
    stored(prefs).filter(|(_, usable)| !usable).map(|(shown, _)| info(&banner(&shown)))
}

/// The row and its info text, from the stored folder. Tapping the row opens the folder chooser; `changed` runs after a
/// new folder is stored.
pub fn rows(prefs: &Prefs, changed: Rc<dyn Fn()>) -> Vec<PreferenceItem> {
    let store = prefs.store().clone();
    let (shown, usable) = stored(prefs).unwrap_or_default();
    let mut location = text("Storage location", Some(&subtitle(&shown, usable)), true);
    if let PreferenceItem::Text { on_click, .. } = &mut location {
        *on_click = Some(Rc::new(move || choose_folder(store.clone(), Rc::clone(&changed))));
    }
    vec![location, info(INFO_TEXT)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_covers_unset_valid_and_invalid() {
        assert_eq!(subtitle("", false), NO_LOCATION);
        assert_eq!(subtitle("/data/kuro", true), "/data/kuro");
        assert_eq!(subtitle("/gone", false), "Invalid location: /gone");
    }

    #[test]
    fn a_chosen_writable_folder_is_accepted_and_a_file_or_missing_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(accept_choice(dir.path()), Ok(()));
        let file = dir.path().join("plain.txt");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(accept_choice(&file), Err(REFUSED));
        assert_eq!(accept_choice(&dir.path().join("missing")), Err(REFUSED));
    }

    fn encoded(path: &str) -> kuro_prefs::values::PrefValue {
        kuro_prefs::values::PrefValue::Str(STANDARD.encode(path))
    }

    fn subtitle_of(items: &[PreferenceItem]) -> Option<String> {
        let PreferenceItem::Text { common, .. } = &items[0] else { panic!("text row") };
        common.subtitle.clone()
    }

    #[test]
    fn the_row_reads_the_stored_folder_and_a_gone_one_shows_a_banner() {
        let prefs = Prefs::in_memory();
        let first = rows(&prefs, Rc::new(|| {}));
        assert_eq!(subtitle_of(&first).as_deref(), Some(NO_LOCATION));
        assert_eq!(first.len(), 2);
        assert!(banner_row(&prefs).is_none(), "no banner while nothing is chosen");
        prefs.store().set_raw(STORAGE_DIR_KEY, encoded("/no/such/kuro"));
        assert_eq!(subtitle_of(&rows(&prefs, Rc::new(|| {}))).as_deref(), Some("Invalid location: /no/such/kuro"));
        assert_eq!(banner_row(&prefs).expect("banner").title(), banner("/no/such/kuro"));
        let dir = tempfile::tempdir().unwrap();
        prefs.store().set_raw(STORAGE_DIR_KEY, encoded(&dir.path().to_string_lossy()));
        assert_eq!(subtitle_of(&rows(&prefs, Rc::new(|| {}))), Some(dir.path().to_string_lossy().into_owned()));
        assert!(banner_row(&prefs).is_none());
    }

    #[test]
    fn a_value_that_does_not_decode_is_no_location() {
        assert_eq!(decode(""), None);
        assert_eq!(decode("%%%not base64"), None);
        assert_eq!(decode(&STANDARD.encode("/a/b")), Some(PathBuf::from("/a/b")));
    }

    #[test]
    fn a_chosen_folder_is_stored_for_the_storage_manager_and_gets_the_mpv_config_tree() {
        let dir = tempfile::tempdir().unwrap();
        let store = PrefStore::in_memory();
        assert_eq!(apply_choice(&store, dir.path()), Ok(()));
        for sub in ["mpv-config/fonts", "mpv-config/scripts", "mpv-config/script-opts", "mpv-config/shaders"] {
            assert!(dir.path().join(sub).is_dir(), "{sub}");
        }
        let raw = Pref::new(store.clone(), STORAGE_DIR_KEY, String::new()).get();
        assert_eq!(decode(&raw).as_deref(), Some(dir.path()));
        assert_eq!(apply_choice(&store, &dir.path().join("missing")), Err(REFUSED));
    }
}
