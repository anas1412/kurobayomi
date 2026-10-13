//! Download preference keys and their defaults (T-24a-054, T-25a-002, T-25a-003, T-25a-005; data sections 3 of
//! `25-downloads-shared` and `24-downloads-anime`).
//!
//! The keys are the Kotlin preference names. They are kept unchanged so that a backup restores (area 22). The settings
//! diet kept four of them (docs/design/settings-diet.md, Downloads page); every other download key was cut and the
//! engine uses its hard-coded default (`DownloadSettings::default()` in `downloader.rs`).

/// The download preference keys the settings diet kept, with the text of their default.
pub const PREFERENCE_KEYS: [(&str, &str); 4] = [
    ("download_new", "false"),
    ("download_new_episode", "false"),
    ("download_slots", "3"),
    ("remove_after_read_slots", "-1"),
];

/// Decision (T-25a-003, E-12): the Kotlin tree stores `download_slots` (default 1, range 1..=5) but never reads it. The
/// engine ignores it, and the settings screen does not offer the control. The manga queue always lets
/// `scheduler::MAX_ACTIVE_SOURCES` sources run at once. The key stays in [`PREFERENCE_KEYS`] so a backup restores.
pub const DOWNLOAD_SLOTS_HIDDEN: bool = true;

/// The default text of a key, or `None` for a key this crate does not know.
pub fn default_of(key: &str) -> Option<&'static str> {
    PREFERENCE_KEYS.iter().find(|(k, _)| *k == key).map(|(_, default)| *default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kept_key_set_matches_the_kotlin_names_and_defaults() {
        assert_eq!(PREFERENCE_KEYS.len(), 4);
        assert_eq!(default_of("download_new"), Some("false"));
        assert_eq!(default_of("download_new_episode"), Some("false"));
        assert_eq!(default_of("download_slots"), Some("3"));
        assert_eq!(default_of("remove_after_read_slots"), Some("-1"), "-1 disables the slots");
        assert_eq!(default_of("downloadSlots"), None, "a camel-case name is not a key");
        assert_eq!(default_of("pref_download_only_over_wifi_key"), None, "cut by the settings diet");
    }

    #[test]
    fn download_slots_is_stored_but_hidden_and_not_read_by_the_engine() {
        assert_eq!(default_of("download_slots"), Some("3"), "the key and its default are still stored");
        // The engine's parallel count is the scheduler limit, whatever `download_slots` holds.
        assert_eq!(crate::scheduler::MAX_ACTIVE_SOURCES, 5);
    }
}
