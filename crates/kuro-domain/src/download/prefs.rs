//! `DownloadPreferences.kt`: the 23 download keys with their defaults. Keys and defaults are the ones registered in
//! `kuro_prefs::registry`; the test below compares every row with that registry, so they cannot drift apart.

use std::collections::BTreeSet;

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;
use kuro_prefs::values::{Decoded, PrefType, PrefValue};

/// A set of category ids, stored as decimal strings. Entries that are not numbers are skipped on read (Q-DL-2), so a
/// hand-edited preference file cannot make the download code fail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryIds(pub BTreeSet<i64>);

impl PrefType for CategoryIds {
    fn encode(&self) -> PrefValue {
        PrefValue::Set(self.0.iter().map(i64::to_string).collect())
    }

    fn decode(v: &PrefValue) -> Decoded<Self> {
        match v {
            PrefValue::Set(strings) => Decoded::Ok(CategoryIds(parse_ids(strings))),
            _ => Decoded::WrongType,
        }
    }
}

/// The numeric entries of `strings` (TC-DL-10: `{"1","x","3"}` gives `{1,3}`).
pub fn parse_ids(strings: &BTreeSet<String>) -> BTreeSet<i64> {
    strings.iter().filter_map(|s| s.parse().ok()).collect()
}

/// Typed handles for the download preferences. Each field is one key with its default.
#[derive(Clone)]
pub struct DownloadPrefs {
    pub download_only_over_wifi: Pref<bool>,
    pub use_external_downloader: Pref<bool>,
    pub external_downloader_selection: Pref<String>,
    pub save_chapters_as_cbz: Pref<bool>,
    pub split_tall_images: Pref<bool>,
    pub auto_download_while_reading: Pref<i32>,
    pub auto_download_while_watching: Pref<i32>,
    pub remove_after_read_slots: Pref<i32>,
    pub remove_after_marked_as_read: Pref<bool>,
    pub remove_bookmarked: Pref<bool>,
    pub download_fillermarked: Pref<bool>,
    pub remove_exclude_manga_categories: Pref<CategoryIds>,
    pub remove_exclude_anime_categories: Pref<CategoryIds>,
    pub download_new_chapters: Pref<bool>,
    pub download_new_episodes: Pref<bool>,
    pub download_new_chapter_categories: Pref<CategoryIds>,
    pub download_new_episode_categories: Pref<CategoryIds>,
    pub download_new_chapter_categories_exclude: Pref<CategoryIds>,
    pub download_new_episode_categories_exclude: Pref<CategoryIds>,
    /// The raw stored value. Read it through [`DownloadPrefs::number_of_downloads`], which clamps.
    pub number_of_downloads_pref: Pref<i32>,
    /// KiB per second; `0` is unthrottled.
    pub download_speed_limit: Pref<i32>,
    pub download_new_unread_chapters_only: Pref<bool>,
    pub download_new_unseen_episodes_only: Pref<bool>,
}

impl DownloadPrefs {
    pub fn new(store: &PrefStore) -> Self {
        Self {
            download_only_over_wifi: store.pref("pref_download_only_over_wifi_key", false),
            use_external_downloader: store.pref("use_external_downloader", false),
            external_downloader_selection: store.pref("external_downloader_selection", String::new()),
            save_chapters_as_cbz: store.pref("save_chapter_as_cbz", true),
            split_tall_images: store.pref("split_tall_images", true),
            auto_download_while_reading: store.pref("auto_download_while_reading", 0),
            auto_download_while_watching: store.pref("auto_download_while_watching", 0),
            remove_after_read_slots: store.pref("remove_after_read_slots", -1),
            remove_after_marked_as_read: store.pref("pref_remove_after_marked_as_read_key", false),
            remove_bookmarked: store.pref("pref_remove_bookmarked", false),
            download_fillermarked: store.pref("pref_download_fillermarked", false),
            remove_exclude_manga_categories: store.pref("remove_exclude_categories", CategoryIds::default()),
            remove_exclude_anime_categories: store.pref("remove_exclude_anime_categories", CategoryIds::default()),
            download_new_chapters: store.pref("download_new", false),
            download_new_episodes: store.pref("download_new_episode", false),
            download_new_chapter_categories: store.pref("download_new_categories", CategoryIds::default()),
            download_new_episode_categories: store.pref("download_new_anime_categories", CategoryIds::default()),
            download_new_chapter_categories_exclude: store
                .pref("download_new_categories_exclude", CategoryIds::default()),
            download_new_episode_categories_exclude: store
                .pref("download_new_anime_categories_exclude", CategoryIds::default()),
            number_of_downloads_pref: store.pref("download_slots", 1),
            download_speed_limit: store.pref("download_speed_limit", 0),
            download_new_unread_chapters_only: store.pref("download_new_unread_chapters_only", false),
            download_new_unseen_episodes_only: store.pref("download_new_unread_episodes_only", false),
        }
    }

    /// `numberOfDownloads`, clamped to `1..=5` on every read (E-DL-01).
    pub fn number_of_downloads(&self) -> i32 {
        self.number_of_downloads_pref.get().clamp(1, 5)
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::category::{CategoryKind, forget_category};
    use kuro_prefs::registry::lookup;

    use super::*;

    /// Every accessor's key and encoded default, in the order of `p1-data-and-config.md` section 4.1 (TC-DL-11).
    fn table(p: &DownloadPrefs) -> Vec<(&str, PrefValue)> {
        vec![
            (p.download_only_over_wifi.key(), false.encode()),
            (p.use_external_downloader.key(), false.encode()),
            (p.external_downloader_selection.key(), String::new().encode()),
            (p.save_chapters_as_cbz.key(), true.encode()),
            (p.split_tall_images.key(), true.encode()),
            (p.auto_download_while_reading.key(), 0_i32.encode()),
            (p.auto_download_while_watching.key(), 0_i32.encode()),
            (p.remove_after_read_slots.key(), (-1_i32).encode()),
            (p.remove_after_marked_as_read.key(), false.encode()),
            (p.remove_bookmarked.key(), false.encode()),
            (p.download_fillermarked.key(), false.encode()),
            (p.remove_exclude_manga_categories.key(), CategoryIds::default().encode()),
            (p.remove_exclude_anime_categories.key(), CategoryIds::default().encode()),
            (p.download_new_chapters.key(), false.encode()),
            (p.download_new_episodes.key(), false.encode()),
            (p.download_new_chapter_categories.key(), CategoryIds::default().encode()),
            (p.download_new_episode_categories.key(), CategoryIds::default().encode()),
            (p.download_new_chapter_categories_exclude.key(), CategoryIds::default().encode()),
            (p.download_new_episode_categories_exclude.key(), CategoryIds::default().encode()),
            (p.number_of_downloads_pref.key(), 1_i32.encode()),
            (p.download_speed_limit.key(), 0_i32.encode()),
            (p.download_new_unread_chapters_only.key(), false.encode()),
            (p.download_new_unseen_episodes_only.key(), false.encode()),
        ]
    }

    #[test]
    fn every_key_and_default_matches_the_registry_and_the_table() {
        let store = PrefStore::in_memory();
        let prefs = DownloadPrefs::new(&store);
        let rows = table(&prefs);
        assert_eq!(rows.len(), 23, "23 accessors");
        for (key, default) in rows {
            let (meta, _) = lookup(key).unwrap_or_else(|| panic!("{key} is not registered"));
            assert_eq!(meta.default.value(), default, "default of {key} (TC-DL-11)");
        }
    }

    #[test]
    fn number_of_downloads_is_clamped_to_one_to_five() {
        let store = PrefStore::in_memory();
        let prefs = DownloadPrefs::new(&store);
        for (stored, want) in [(0, 1), (1, 1), (5, 5), (9, 5), (-3, 1)] {
            prefs.number_of_downloads_pref.set(stored);
            assert_eq!(prefs.number_of_downloads(), want, "E-DL-01 for {stored}");
        }
    }

    #[test]
    fn category_sets_skip_non_numeric_entries_on_read() {
        let set: BTreeSet<String> = ["1", "x", "3"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse_ids(&set), BTreeSet::from([1, 3]), "TC-DL-10");
        let store = PrefStore::in_memory();
        let prefs = DownloadPrefs::new(&store);
        store.set_raw("download_new_categories", PrefValue::Set(set));
        assert_eq!(prefs.download_new_chapter_categories.get(), CategoryIds(BTreeSet::from([1, 3])));
    }

    #[test]
    fn deleting_a_category_strips_its_id_from_all_six_category_sets() {
        let store = PrefStore::in_memory();
        let lib = kuro_prefs::library::LibraryPrefs::new(store.clone());
        let six = [
            "remove_exclude_categories",
            "remove_exclude_anime_categories",
            "download_new_categories",
            "download_new_anime_categories",
            "download_new_categories_exclude",
            "download_new_anime_categories_exclude",
        ];
        for key in six {
            store.pref::<BTreeSet<String>>(key, BTreeSet::new()).set(BTreeSet::from(["3".to_owned(), "4".to_owned()]));
        }
        forget_category(&store, &lib, CategoryKind::Manga, 3);
        forget_category(&store, &lib, CategoryKind::Anime, 3);
        for key in six {
            let left = store.pref::<BTreeSet<String>>(key, BTreeSet::new()).get();
            assert_eq!(left, BTreeSet::from(["4".to_owned()]), "{key} still holds the deleted id");
        }
    }
}
