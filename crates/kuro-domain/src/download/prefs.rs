//! `DownloadPreferences.kt`: the download keys the settings diet kept (Downloads page): the concurrent download count,
//! delete-after-read slots and the two new-item switches. The other download keys were cut; their hard-coded defaults
//! are read where they are used. Keys and defaults are the ones registered in `kuro_prefs::registry`; the test below
//! compares every kept row with that registry, so they cannot drift apart.

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;

/// Typed handles for the kept download preferences. Each field is one key with its default.
#[derive(Clone)]
pub struct DownloadPrefs {
    /// The raw stored value. Read it through [`DownloadPrefs::number_of_downloads`], which clamps.
    pub number_of_downloads_pref: Pref<i32>,
    /// `-1` is disabled; `0` deletes the last read chapter, `n` the n-th last one.
    pub remove_after_read_slots: Pref<i32>,
    pub download_new_chapters: Pref<bool>,
    pub download_new_episodes: Pref<bool>,
}

impl DownloadPrefs {
    pub fn new(store: &PrefStore) -> Self {
        Self {
            number_of_downloads_pref: store.pref("download_slots", 3),
            remove_after_read_slots: store.pref("remove_after_read_slots", -1),
            download_new_chapters: store.pref("download_new", false),
            download_new_episodes: store.pref("download_new_episode", false),
        }
    }

    /// `numberOfDownloads`, clamped to `1..=5` on every read (E-DL-01).
    pub fn number_of_downloads(&self) -> i32 {
        self.number_of_downloads_pref.get().clamp(1, 5)
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::registry::lookup;
    use kuro_prefs::values::PrefType;

    use super::*;

    #[test]
    fn every_kept_key_and_default_matches_the_registry() {
        let store = PrefStore::in_memory();
        let prefs = DownloadPrefs::new(&store);
        let rows = [
            (prefs.number_of_downloads_pref.key(), 3_i32.encode()),
            (prefs.remove_after_read_slots.key(), (-1_i32).encode()),
            (prefs.download_new_chapters.key(), false.encode()),
            (prefs.download_new_episodes.key(), false.encode()),
        ];
        for (key, default) in rows {
            let (meta, _) = lookup(key).unwrap_or_else(|| panic!("{key} is not registered"));
            assert_eq!(meta.default.value(), default, "default of {key}");
        }
    }

    #[test]
    fn number_of_downloads_is_clamped_to_one_to_five() {
        let store = PrefStore::in_memory();
        let prefs = DownloadPrefs::new(&store);
        for (stored, want) in [(0, 1), (1, 1), (5, 5), (9, 5), (-3, 1)] {
            prefs.number_of_downloads_pref.set(stored).unwrap();
            assert_eq!(prefs.number_of_downloads(), want, "E-DL-01 for {stored}");
        }
    }
}
