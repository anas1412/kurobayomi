//! Storage location row (tasks T-47a-088 and T-48a-051): the subtitle rule, the folder check, the info text and the
//! "base folder not available" banner. The stored value is the base64 of the path bytes, as `StorageManager` writes
//! it, so a folder chosen here is the one the backup, download and mpv-config code resolve.
use kuro_i18n::{Arg, MsgKey, tr, trf};
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

pub fn no_location() -> String {
    tr(MsgKey::no_location_set)
}
pub fn info_text() -> String {
    tr(MsgKey::settings_storage_info)
}
pub fn refused() -> String {
    tr(MsgKey::file_picker_uri_permission_unsupported)
}

/// `No storage location set`, the stored path when it is usable, or `Invalid location: <path>`.
pub fn subtitle(stored: &str, usable: bool) -> String {
    if stored.is_empty() {
        no_location().to_owned()
    } else if usable {
        stored.to_owned()
    } else {
        trf(MsgKey::invalid_location, &[("arg1", Arg::Str(&stored.to_string()))])
    }
}

/// The banner shown on the page while a chosen base folder is gone (E-SY-05).
pub fn banner(path: &str) -> String {
    trf(MsgKey::storage_base_folder_unavailable, &[("path", Arg::Str(&path.to_string()))])
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
pub fn accept_choice(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(refused());
    }
    tempfile::tempfile_in(path).map(drop).map_err(|_| refused())
}

/// The storage manager over the stored base folder: the one resolver of the storage folders (downloads per kind,
/// backups, local folders). Nothing stored means the default storage folder. The download managers and indexes resolve
/// their folders once at startup, so a new location applies to downloads after a restart.
pub fn manager(store: &PrefStore) -> Option<StorageManager> {
    let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
    Some(StorageManager::open(store, paths.default_storage_dir()))
}

/// Stores `path` as the base folder and creates the mpv-config tree under it. A folder that cannot be used is refused.
pub fn apply_choice(store: &PrefStore, path: &Path) -> Result<(), String> {
    accept_choice(path)?;
    let manager = manager(store).ok_or_else(refused)?;
    manager.set_base(path).map_err(|err| {
        tracing::warn!(%err, "the storage location could not be saved");
        refused()
    })
}

/// Opens the folder chooser; a folder that passes [`apply_choice`] is stored and `changed` runs, any other one is
/// refused with a toast. A cancelled chooser changes nothing.
/// A folder chooser that failed (not dismissed) shows the unavailable toast (task T-47b-055).
fn chooser_failed(err: &gtk::glib::Error) {
    if let Some(message) = super::chooser::failure_toast(err) {
        crate::toast::notify(message);
    }
}

fn choose_folder(store: PrefStore, changed: Rc<dyn Fn()>) {
    gtk::FileDialog::new().select_folder(Some(&crate::ui::window()), None::<&gtk::gio::Cancellable>, move |result| {
        let path = match result {
            Ok(file) => file.path(),
            Err(err) => return chooser_failed(&err),
        };
        let Some(path) = path else { return };
        match apply_choice(&store, &path) {
            Ok(()) => changed(),
            Err(message) => crate::toast::notify(message),
        }
    });
}

/// The folder in use and whether it can be listed. Nothing stored means the default storage folder, which is created on
/// first use, so it counts as usable.
fn stored(prefs: &Prefs) -> Option<(String, bool)> {
    let raw = Pref::new(prefs.store().clone(), STORAGE_DIR_KEY, String::new()).get();
    match decode(&raw) {
        Some(path) => Some((path.to_string_lossy().into_owned(), is_readable_dir(&path))),
        None => default_storage().map(|path| (path.to_string_lossy().into_owned(), true)),
    }
}

/// The default storage folder, `None` when no home folder exists.
fn default_storage() -> Option<PathBuf> {
    kuro_platform::linux::LinuxPaths::system().ok().map(|paths| paths.default_storage_dir())
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
    let mut location = text(&tr(MsgKey::pref_storage_location), Some(&subtitle(&shown, usable)), true);
    if let PreferenceItem::Text { on_click, .. } = &mut location {
        *on_click = Some(Rc::new(move || choose_folder(store.clone(), Rc::clone(&changed))));
    }
    vec![location, info(&info_text())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_covers_unset_valid_and_invalid() {
        assert_eq!(subtitle("", false), no_location());
        assert_eq!(subtitle("/data/kuro", true), "/data/kuro");
        assert_eq!(subtitle("/gone", false), "Invalid location: /gone");
    }

    #[test]
    fn a_chosen_writable_folder_is_accepted_and_a_file_or_missing_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(accept_choice(dir.path()), Ok(()));
        let file = dir.path().join("plain.txt");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(accept_choice(&file), Err(refused()));
        assert_eq!(accept_choice(&dir.path().join("missing")), Err(refused()));
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
        let default = default_storage().map(|path| path.to_string_lossy().into_owned());
        assert_eq!(subtitle_of(&first), default, "nothing stored shows the default storage folder");
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
        assert_eq!(apply_choice(&store, &dir.path().join("missing")), Err(refused()));
    }
}
