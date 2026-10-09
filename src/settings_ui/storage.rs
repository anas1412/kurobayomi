//! Storage location row (task T-47a-088): the subtitle rule, the folder check and the info text.
use std::path::Path;

use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::{info, text};

/// Stored key of the chosen folder (`__APP_STATE_storage_dir`, shared with the backup code).
pub const STORAGE_KEY: &str = "__APP_STATE_storage_dir";
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

/// The row and its info text, from the stored folder.
pub fn rows(prefs: &Prefs) -> Vec<PreferenceItem> {
    let stored = Pref::new(prefs.store().clone(), STORAGE_KEY, String::new()).get();
    let usable = !stored.is_empty() && is_readable_dir(Path::new(&stored));
    vec![text("Storage location", Some(&subtitle(&stored, usable)), true), info(INFO_TEXT)]
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

    #[test]
    fn the_row_reads_the_stored_folder_and_flags_a_missing_one() {
        let prefs = Prefs::in_memory();
        let first = rows(&prefs);
        let PreferenceItem::Text { common, .. } = &first[0] else { panic!("text row") };
        assert_eq!(common.subtitle.as_deref(), Some(NO_LOCATION));
        prefs.store().set_raw(STORAGE_KEY, kuro_prefs::values::PrefValue::Str("/no/such/kuro".into()));
        let PreferenceItem::Text { common, .. } = &rows(&prefs)[0] else { panic!("text row") };
        assert_eq!(common.subtitle.as_deref(), Some("Invalid location: /no/such/kuro"));
        assert_eq!(first.len(), 2);
    }
}
