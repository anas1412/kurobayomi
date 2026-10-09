//! Typed download preferences (T-25a-001): one accessor per Kotlin `DownloadPreferences` accessor (23 in total), each
//! returning a [`Pref`] on the area 02 store. Nothing is cached here: every `get` reads the store, every `set` writes
//! through it, and `watch` reports changes made anywhere.
//!
//! The key strings and defaults are the ones of [`crate::settings::PREFERENCE_KEYS`]; the tests tie them together.

use std::collections::BTreeSet;

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;

/// The exact stored key of each preference (data section 3 of area 25, Kotlin `DownloadPreferences.kt`).
pub mod keys {
    pub const DOWNLOAD_ONLY_OVER_WIFI: &str = "pref_download_only_over_wifi_key";
    pub const USE_EXTERNAL_DOWNLOADER: &str = "use_external_downloader";
    pub const EXTERNAL_DOWNLOADER_SELECTION: &str = "external_downloader_selection";
    pub const SAVE_CHAPTER_AS_CBZ: &str = "save_chapter_as_cbz";
    pub const SPLIT_TALL_IMAGES: &str = "split_tall_images";
    pub const AUTO_DOWNLOAD_WHILE_READING: &str = "auto_download_while_reading";
    pub const AUTO_DOWNLOAD_WHILE_WATCHING: &str = "auto_download_while_watching";
    pub const REMOVE_AFTER_READ_SLOTS: &str = "remove_after_read_slots";
    pub const REMOVE_AFTER_MARKED_AS_READ: &str = "pref_remove_after_marked_as_read_key";
    pub const REMOVE_BOOKMARKED: &str = "pref_remove_bookmarked";
    pub const DOWNLOAD_FILLERMARKED: &str = "pref_download_fillermarked";
    pub const REMOVE_EXCLUDE_CATEGORIES: &str = "remove_exclude_categories";
    pub const REMOVE_EXCLUDE_ANIME_CATEGORIES: &str = "remove_exclude_anime_categories";
    pub const DOWNLOAD_NEW: &str = "download_new";
    pub const DOWNLOAD_NEW_EPISODE: &str = "download_new_episode";
    pub const DOWNLOAD_NEW_CATEGORIES: &str = "download_new_categories";
    pub const DOWNLOAD_NEW_ANIME_CATEGORIES: &str = "download_new_anime_categories";
    pub const DOWNLOAD_NEW_CATEGORIES_EXCLUDE: &str = "download_new_categories_exclude";
    pub const DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE: &str = "download_new_anime_categories_exclude";
    pub const DOWNLOAD_SLOTS: &str = "download_slots";
    pub const DOWNLOAD_SPEED_LIMIT: &str = "download_speed_limit";
    pub const DOWNLOAD_NEW_UNREAD_CHAPTERS_ONLY: &str = "download_new_unread_chapters_only";
    pub const DOWNLOAD_NEW_UNSEEN_EPISODES_ONLY: &str = "download_new_unread_episodes_only";
}

/// Handle to the download preferences of one store. Cheap to clone.
#[derive(Clone)]
pub struct DownloadPrefs {
    store: PrefStore,
}

impl DownloadPrefs {
    pub fn new(store: PrefStore) -> Self {
        Self { store }
    }

    pub fn download_only_over_wifi(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_ONLY_OVER_WIFI, true)
    }

    pub fn use_external_downloader(&self) -> Pref<bool> {
        self.store.pref(keys::USE_EXTERNAL_DOWNLOADER, false)
    }

    pub fn external_downloader_selection(&self) -> Pref<String> {
        self.store.pref(keys::EXTERNAL_DOWNLOADER_SELECTION, String::new())
    }

    pub fn save_chapter_as_cbz(&self) -> Pref<bool> {
        self.store.pref(keys::SAVE_CHAPTER_AS_CBZ, true)
    }

    pub fn split_tall_images(&self) -> Pref<bool> {
        self.store.pref(keys::SPLIT_TALL_IMAGES, true)
    }

    pub fn auto_download_while_reading(&self) -> Pref<i32> {
        self.store.pref(keys::AUTO_DOWNLOAD_WHILE_READING, 0)
    }

    pub fn auto_download_while_watching(&self) -> Pref<i32> {
        self.store.pref(keys::AUTO_DOWNLOAD_WHILE_WATCHING, 0)
    }

    pub fn remove_after_read_slots(&self) -> Pref<i32> {
        self.store.pref(keys::REMOVE_AFTER_READ_SLOTS, -1)
    }

    pub fn remove_after_marked_as_read(&self) -> Pref<bool> {
        self.store.pref(keys::REMOVE_AFTER_MARKED_AS_READ, false)
    }

    pub fn remove_bookmarked_chapters(&self) -> Pref<bool> {
        self.store.pref(keys::REMOVE_BOOKMARKED, false)
    }

    pub fn download_fillermarked_items(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_FILLERMARKED, false)
    }

    pub fn remove_exclude_categories(&self) -> Pref<BTreeSet<String>> {
        self.store.pref(keys::REMOVE_EXCLUDE_CATEGORIES, BTreeSet::new())
    }

    pub fn remove_exclude_anime_categories(&self) -> Pref<BTreeSet<String>> {
        self.store.pref(keys::REMOVE_EXCLUDE_ANIME_CATEGORIES, BTreeSet::new())
    }

    pub fn download_new_chapters(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_NEW, false)
    }

    pub fn download_new_episodes(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_NEW_EPISODE, false)
    }

    pub fn download_new_chapter_categories(&self) -> Pref<BTreeSet<String>> {
        self.store.pref(keys::DOWNLOAD_NEW_CATEGORIES, BTreeSet::new())
    }

    pub fn download_new_episode_categories(&self) -> Pref<BTreeSet<String>> {
        self.store.pref(keys::DOWNLOAD_NEW_ANIME_CATEGORIES, BTreeSet::new())
    }

    pub fn download_new_chapter_categories_exclude(&self) -> Pref<BTreeSet<String>> {
        self.store.pref(keys::DOWNLOAD_NEW_CATEGORIES_EXCLUDE, BTreeSet::new())
    }

    pub fn download_new_episode_categories_exclude(&self) -> Pref<BTreeSet<String>> {
        self.store.pref(keys::DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE, BTreeSet::new())
    }

    /// Stored and written by the settings screen, read by nothing (E-12, [`crate::settings::DOWNLOAD_SLOTS_HIDDEN`]).
    pub fn number_of_downloads(&self) -> Pref<i32> {
        self.store.pref(keys::DOWNLOAD_SLOTS, 1)
    }

    /// Speed limit in KiB/s. Convert with [`crate::settings::speed_limit_bytes_per_sec`].
    pub fn download_speed_limit(&self) -> Pref<i32> {
        self.store.pref(keys::DOWNLOAD_SPEED_LIMIT, 0)
    }

    pub fn download_new_unread_chapters_only(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_NEW_UNREAD_CHAPTERS_ONLY, false)
    }

    pub fn download_new_unseen_episodes_only(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_NEW_UNSEEN_EPISODES_ONLY, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::default_of;

    fn prefs() -> DownloadPrefs {
        DownloadPrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn every_accessor_uses_the_exact_key_and_default_of_the_spec_table() {
        let p = prefs();
        // Rows 1 to 23 of the table; each key and its default text come from settings::PREFERENCE_KEYS.
        assert_eq!(p.download_only_over_wifi().key(), "pref_download_only_over_wifi_key");
        let checks: Vec<(&str, bool)> = vec![
            (keys::DOWNLOAD_ONLY_OVER_WIFI, p.download_only_over_wifi().default_value()),
            (keys::USE_EXTERNAL_DOWNLOADER, p.use_external_downloader().default_value()),
            (keys::SAVE_CHAPTER_AS_CBZ, p.save_chapter_as_cbz().default_value()),
            (keys::SPLIT_TALL_IMAGES, p.split_tall_images().default_value()),
            (keys::REMOVE_AFTER_MARKED_AS_READ, p.remove_after_marked_as_read().default_value()),
            (keys::REMOVE_BOOKMARKED, p.remove_bookmarked_chapters().default_value()),
            (keys::DOWNLOAD_FILLERMARKED, p.download_fillermarked_items().default_value()),
            (keys::DOWNLOAD_NEW, p.download_new_chapters().default_value()),
            (keys::DOWNLOAD_NEW_EPISODE, p.download_new_episodes().default_value()),
            (keys::DOWNLOAD_NEW_UNREAD_CHAPTERS_ONLY, p.download_new_unread_chapters_only().default_value()),
            (keys::DOWNLOAD_NEW_UNSEEN_EPISODES_ONLY, p.download_new_unseen_episodes_only().default_value()),
        ];
        for (key, default) in checks {
            assert_eq!(default_of(key), Some(if default { "true" } else { "false" }), "default of {key}");
        }
        let ints: Vec<(&str, i32)> = vec![
            (keys::AUTO_DOWNLOAD_WHILE_READING, p.auto_download_while_reading().default_value()),
            (keys::AUTO_DOWNLOAD_WHILE_WATCHING, p.auto_download_while_watching().default_value()),
            (keys::REMOVE_AFTER_READ_SLOTS, p.remove_after_read_slots().default_value()),
            (keys::DOWNLOAD_SLOTS, p.number_of_downloads().default_value()),
            (keys::DOWNLOAD_SPEED_LIMIT, p.download_speed_limit().default_value()),
        ];
        for (key, default) in ints {
            assert_eq!(default_of(key), Some(default.to_string().as_str()), "default of {key}");
        }
        let strings: Vec<(&str, String)> = vec![
            (keys::EXTERNAL_DOWNLOADER_SELECTION, p.external_downloader_selection().default_value()),
        ];
        for (key, default) in strings {
            assert_eq!(default_of(key), Some(default.as_str()), "default of {key}");
        }
        let sets = [
            keys::REMOVE_EXCLUDE_CATEGORIES,
            keys::REMOVE_EXCLUDE_ANIME_CATEGORIES,
            keys::DOWNLOAD_NEW_CATEGORIES,
            keys::DOWNLOAD_NEW_ANIME_CATEGORIES,
            keys::DOWNLOAD_NEW_CATEGORIES_EXCLUDE,
            keys::DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE,
        ];
        for key in sets {
            assert_eq!(default_of(key), Some(""), "an empty set is stored as the empty text: {key}");
        }
    }

    #[test]
    fn the_accessors_cover_the_23_spec_keys_exactly() {
        let mut all: Vec<&str> = vec![
            keys::DOWNLOAD_ONLY_OVER_WIFI,
            keys::USE_EXTERNAL_DOWNLOADER,
            keys::EXTERNAL_DOWNLOADER_SELECTION,
            keys::SAVE_CHAPTER_AS_CBZ,
            keys::SPLIT_TALL_IMAGES,
            keys::AUTO_DOWNLOAD_WHILE_READING,
            keys::AUTO_DOWNLOAD_WHILE_WATCHING,
            keys::REMOVE_AFTER_READ_SLOTS,
            keys::REMOVE_AFTER_MARKED_AS_READ,
            keys::REMOVE_BOOKMARKED,
            keys::DOWNLOAD_FILLERMARKED,
            keys::REMOVE_EXCLUDE_CATEGORIES,
            keys::REMOVE_EXCLUDE_ANIME_CATEGORIES,
            keys::DOWNLOAD_NEW,
            keys::DOWNLOAD_NEW_EPISODE,
            keys::DOWNLOAD_NEW_CATEGORIES,
            keys::DOWNLOAD_NEW_ANIME_CATEGORIES,
            keys::DOWNLOAD_NEW_CATEGORIES_EXCLUDE,
            keys::DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE,
            keys::DOWNLOAD_SLOTS,
            keys::DOWNLOAD_SPEED_LIMIT,
            keys::DOWNLOAD_NEW_UNREAD_CHAPTERS_ONLY,
            keys::DOWNLOAD_NEW_UNSEEN_EPISODES_ONLY,
        ];
        assert_eq!(all.len(), 23);
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 23, "no two accessors share a key");
        for (key, _) in crate::settings::PREFERENCE_KEYS.iter().filter(|(k, _)| *k != "hide_notification_content") {
            assert!(all.contains(key), "{key} has an accessor");
        }
    }

    #[test]
    fn a_set_is_visible_to_a_watcher_and_a_read_sees_it() {
        let p = prefs();
        let speed = p.download_speed_limit();
        let watch = speed.watch();
        speed.set(512).unwrap();
        assert_eq!(p.download_speed_limit().get(), 512, "reads go to the store, nothing is cached");
        assert_eq!(watch.latest(), 512, "a watcher sees the write");
        assert_eq!(crate::settings::speed_limit_bytes_per_sec(i64::from(speed.get())), Some(512 * 1024));
    }

    #[test]
    fn categories_are_string_sets_and_an_unset_key_reads_the_default() {
        let p = prefs();
        assert!(p.remove_exclude_categories().get().is_empty());
        let mut set = BTreeSet::new();
        set.insert("3".to_owned());
        p.remove_exclude_categories().set(set.clone()).unwrap();
        assert_eq!(p.remove_exclude_categories().get(), set);
    }
}
