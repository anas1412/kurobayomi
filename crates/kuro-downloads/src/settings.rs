//! Download preference keys, defaults and the two conversions the engine uses (T-24a-054, T-25a-002, T-25a-003,
//! T-25a-005; data sections 3 of `25-downloads-shared` and `24-downloads-anime`).
//!
//! The keys are the Kotlin preference names. They are kept unchanged so that a backup restores (area 22). The values are
//! the text each default must have. The typed store (area 02) reads them; this crate only owns the table and the rules
//! that turn a stored value into engine behaviour.

/// Every download preference key, with the text of its default. `hide_notification_content` is read from the security
/// preferences but belongs to the anime notifications, so it is listed here too.
pub const PREFERENCE_KEYS: [(&str, &str); 24] = [
    ("pref_download_only_over_wifi_key", "true"),
    ("use_external_downloader", "false"),
    ("external_downloader_selection", ""),
    ("save_chapter_as_cbz", "true"),
    ("split_tall_images", "true"),
    ("auto_download_while_reading", "0"),
    ("auto_download_while_watching", "0"),
    ("remove_after_read_slots", "-1"),
    ("pref_remove_after_marked_as_read_key", "false"),
    ("pref_remove_bookmarked", "false"),
    ("pref_download_fillermarked", "false"),
    ("remove_exclude_categories", ""),
    ("remove_exclude_anime_categories", ""),
    ("download_new", "false"),
    ("download_new_episode", "false"),
    ("download_new_categories", ""),
    ("download_new_anime_categories", ""),
    ("download_new_categories_exclude", ""),
    ("download_new_anime_categories_exclude", ""),
    ("download_slots", "1"),
    ("download_speed_limit", "0"),
    ("download_new_unread_chapters_only", "false"),
    ("download_new_unread_episodes_only", "false"),
    ("hide_notification_content", "false"),
];

/// Decision (T-25a-003, E-12): the Kotlin tree stores `download_slots` (default 1, range 1..=5) but never reads it. The
/// engine ignores it, and the settings screen does not offer the control. The manga queue always lets
/// `scheduler::MAX_ACTIVE_SOURCES` sources run at once. The key stays in [`PREFERENCE_KEYS`] so a backup restores.
pub const DOWNLOAD_SLOTS_HIDDEN: bool = true;

/// `download_speed_limit` in bytes per second. The stored value is KiB/s and is multiplied by 1024 (MangaDownloader.kt:525).
/// `0` means unlimited, and so does any negative value.
pub fn speed_limit_bytes_per_sec(stored_kib_per_sec: i64) -> Option<u64> {
    if stored_kib_per_sec <= 0 { None } else { Some(stored_kib_per_sec as u64 * 1024) }
}

/// The default text of a key, or `None` for a key this crate does not know.
pub fn default_of(key: &str) -> Option<&'static str> {
    PREFERENCE_KEYS.iter().find(|(k, _)| *k == key).map(|(_, default)| *default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_set_matches_the_kotlin_names_and_defaults() {
        assert_eq!(PREFERENCE_KEYS.len(), 24);
        // The data table of 25-downloads-shared section 3 (rows 1 to 23) and hide_notification_content from 24-downloads.
        let spec = [
            ("pref_download_only_over_wifi_key", "true"),
            ("use_external_downloader", "false"),
            ("external_downloader_selection", ""),
            ("save_chapter_as_cbz", "true"),
            ("split_tall_images", "true"),
            ("auto_download_while_reading", "0"),
            ("auto_download_while_watching", "0"),
            ("remove_after_read_slots", "-1"),
            ("pref_remove_after_marked_as_read_key", "false"),
            ("pref_remove_bookmarked", "false"),
            ("pref_download_fillermarked", "false"),
            ("remove_exclude_categories", ""),
            ("remove_exclude_anime_categories", ""),
            ("download_new", "false"),
            ("download_new_episode", "false"),
            ("download_new_categories", ""),
            ("download_new_anime_categories", ""),
            ("download_new_categories_exclude", ""),
            ("download_new_anime_categories_exclude", ""),
            ("download_slots", "1"),
            ("download_speed_limit", "0"),
            ("download_new_unread_chapters_only", "false"),
            ("download_new_unread_episodes_only", "false"),
            ("hide_notification_content", "false"),
        ];
        assert_eq!(PREFERENCE_KEYS.as_slice(), spec.as_slice(), "every key and default matches the spec table");
        assert_eq!(default_of("downloadSlots"), None, "a camel-case name is not a key");
    }

    #[test]
    fn storage_and_transport_defaults_match_the_spec() {
        assert_eq!(default_of("pref_download_only_over_wifi_key"), Some("true"));
        assert_eq!(default_of("use_external_downloader"), Some("false"));
        assert_eq!(default_of("external_downloader_selection"), Some(""));
        assert_eq!(default_of("save_chapter_as_cbz"), Some("true"));
        assert_eq!(default_of("split_tall_images"), Some("true"));
    }

    #[test]
    fn remove_after_read_defaults_and_the_bookmark_rule() {
        assert_eq!(default_of("remove_after_read_slots"), Some("-1"), "-1 disables the slots");
        assert_eq!(default_of("pref_remove_after_marked_as_read_key"), Some("false"));
        assert_eq!(default_of("pref_remove_bookmarked"), Some("false"), "bookmarked items are protected by default");
        assert_eq!(default_of("pref_download_fillermarked"), Some("false"));
    }

    #[test]
    fn speed_limit_is_kib_per_second_and_zero_is_unlimited() {
        assert_eq!(speed_limit_bytes_per_sec(0), None);
        assert_eq!(speed_limit_bytes_per_sec(-5), None);
        assert_eq!(speed_limit_bytes_per_sec(1), Some(1024));
        assert_eq!(speed_limit_bytes_per_sec(512), Some(512 * 1024));
    }

    #[test]
    fn download_slots_is_stored_but_hidden_and_not_read_by_the_engine() {
        assert!(DOWNLOAD_SLOTS_HIDDEN, "the control is hidden");
        assert_eq!(default_of("download_slots"), Some("1"), "the key and its default are still stored");
        // The engine's parallel count is the scheduler limit, whatever `download_slots` holds.
        assert_eq!(crate::scheduler::MAX_ACTIVE_SOURCES, 5);
    }
}
