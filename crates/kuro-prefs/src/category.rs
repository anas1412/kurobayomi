//! Preference side of the category rules (T-05a-025). Deleting a category clears its id from the category
//! sets and resets the default category. A sort-mode change writes the sort string. The Random sort seed is
//! app memory (the settings diet cut its keys), one per kind. The category rows are in `kuro-data::category`, and the sort codec is `kuro_platform::sort`.

use kuro_platform::kind::Kind;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::PrefError;
use crate::library::LibraryPrefs;
use crate::store::PrefStore;

static RANDOM_SEEDS: [AtomicI32; 2] = [AtomicI32::new(0), AtomicI32::new(0)];

fn seed_slot(kind: Kind) -> &'static AtomicI32 {
    match kind {
        Kind::Manga => &RANDOM_SEEDS[0],
        Kind::Anime => &RANDOM_SEEDS[1],
    }
}

/// The seed of the Random sort of `kind`, `0` until a Random sort is picked. It lives in memory only.
pub fn random_sort_seed(kind: Kind) -> i32 {
    seed_slot(kind).load(Ordering::Relaxed)
}

/// Remembers `seed` as the Random sort seed of `kind` until the app closes.
pub fn set_random_sort_seed(kind: Kind, seed: i32) {
    seed_slot(kind).store(seed, Ordering::Relaxed);
}

/// The six preference keys of one kind that hold category ids: the default category and the five id sets (library
/// update include and exclude, remove-exclude, download-new include and exclude). The strings are the Kotlin key
/// names, kept for backup parity. This is the ONE list: backup restore, domain and the importer read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategoryKeys {
    pub default: &'static str,
    pub update: &'static str,
    pub update_exclude: &'static str,
    pub remove_exclude: &'static str,
    pub download_new: &'static str,
    pub download_new_exclude: &'static str,
}

/// The category-id keys of `kind`.
pub const fn category_keys(kind: Kind) -> CategoryKeys {
    match kind {
        Kind::Manga => CategoryKeys {
            default: "default_category",
            update: "library_update_categories",
            update_exclude: "library_update_categories_exclude",
            remove_exclude: "remove_exclude_categories",
            download_new: "download_new_categories",
            download_new_exclude: "download_new_categories_exclude",
        },
        Kind::Anime => CategoryKeys {
            default: "default_anime_category",
            update: "animelib_update_categories",
            update_exclude: "animelib_update_categories_exclude",
            remove_exclude: "remove_exclude_anime_categories",
            download_new: "download_new_anime_categories",
            download_new_exclude: "download_new_anime_categories_exclude",
        },
    }
}

impl CategoryKeys {
    /// The default-category key and the five set keys.
    pub const fn all(&self) -> [&'static str; 6] {
        [self.default, self.update, self.update_exclude, self.remove_exclude, self.download_new, self.download_new_exclude]
    }

    /// The five category-id sets (without the default). Manga's exclude set is cleaned too (EC-11).
    pub const fn sets(&self) -> [&'static str; 5] {
        [self.update, self.update_exclude, self.remove_exclude, self.download_new, self.download_new_exclude]
    }
}

/// The ten category-id preference sets of both kinds: the library update include and exclude sets, the
/// remove-exclude set and the download-new include and exclude sets. Startup cleanup and backup restore walk them.
pub fn category_id_set_keys() -> Vec<&'static str> {
    Kind::ALL.into_iter().flat_map(|kind| category_keys(kind).sets()).collect()
}

/// `DeleteMangaCategory` and `DeleteAnimeCategory`, preference side. The default category is reset when it
/// is `id`, and `id` leaves each of the five category sets.
pub fn forget_category(store: &PrefStore, lib: &LibraryPrefs, kind: Kind, id: i64) -> Result<(), PrefError> {
    let default = lib.default_category(kind);
    if i64::from(default.get()) == id {
        default.delete()?;
    }
    let needle = id.to_string();
    for key in category_keys(kind).sets() {
        let set = store.pref::<BTreeSet<String>>(key, BTreeSet::new());
        let mut ids = set.get();
        if ids.remove(&needle) {
            set.set(ids)?;
        }
    }
    Ok(())
}

/// `CategoryPreferencesCleanupMigration` (T-11a-008). `manga_ids` and `anime_ids` are the decimal ids of the categories
/// that exist in each database, the system category `"0"` included. A default category preference that names an id
/// in neither set is deleted, which reads back as `-1` ("always ask"). Each of the ten category-id sets loses
/// `ids - manga_ids - anime_ids`; a set with nothing to lose is not written. An id is checked against both sets, not
/// the set of its own kind, as in the upstream migration. Returns how many ids the sets lost.
pub fn prune_dangling_category_ids(
    store: &PrefStore,
    lib: &LibraryPrefs,
    manga_ids: &BTreeSet<String>,
    anime_ids: &BTreeSet<String>,
) -> Result<usize, PrefError> {
    let known = |id: &String| manga_ids.contains(id) || anime_ids.contains(id);
    for default in Kind::ALL.map(|kind| lib.default_category(kind)) {
        let id = default.get();
        if default.is_set() && id != -1 && !known(&id.to_string()) {
            default.delete()?;
        }
    }
    let mut removed = 0;
    for key in category_id_set_keys() {
        let set = store.pref::<BTreeSet<String>>(key, BTreeSet::new());
        let ids = set.get();
        let kept: BTreeSet<String> = ids.iter().filter(|id| known(id)).cloned().collect();
        if kept.len() != ids.len() {
            removed += ids.len() - kept.len();
            set.set(kept)?;
        }
    }
    Ok(removed)
}

/// The preference writes of `SetSortModeForMangaCategory`. `serialized_sort` is the stored sort string such
/// as `RANDOM,ASCENDING`. `global` writes the sort into the global preference, which is false when a category
/// takes the change instead.
pub fn write_sort_prefs(lib: &LibraryPrefs, kind: Kind, serialized_sort: &str, global: bool) -> Result<(), PrefError> {
    if global {
        lib.set_sorting_mode(kind, serialized_sort)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn set_of(store: &PrefStore, key: &'static str) -> BTreeSet<String> {
        store.pref::<BTreeSet<String>>(key, BTreeSet::new()).get()
    }

    fn fill(store: &PrefStore, keys: [&'static str; 5], value: &[&str]) {
        for key in keys {
            store.pref::<BTreeSet<String>>(key, BTreeSet::new()).set(ids(value)).unwrap();
        }
    }

    #[test]
    fn deleting_a_manga_category_removes_its_id_from_all_five_manga_sets() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        fill(&store, category_keys(Kind::Manga).sets(), &["5", "7"]);
        fill(&store, category_keys(Kind::Anime).sets(), &["5"]);

        forget_category(&store, &lib, Kind::Manga, 5).unwrap();

        for key in category_keys(Kind::Manga).sets() {
            assert_eq!(set_of(&store, key), ids(&["7"]), "{key} lost the deleted id");
        }
        for key in category_keys(Kind::Anime).sets() {
            assert_eq!(set_of(&store, key), ids(&["5"]), "{key} belongs to the other kind");
        }
    }

    #[test]
    fn the_manga_exclude_set_loses_the_deleted_id_too() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        store.pref::<BTreeSet<String>>("library_update_categories", BTreeSet::new()).set(ids(&["5"])).unwrap();
        store.pref::<BTreeSet<String>>("library_update_categories_exclude", BTreeSet::new()).set(ids(&["5", "6"])).unwrap();

        forget_category(&store, &lib, Kind::Manga, 5).unwrap();

        assert_eq!(set_of(&store, "library_update_categories_exclude"), ids(&["6"]));
        assert!(set_of(&store, "library_update_categories").is_empty());
    }

    #[test]
    fn deleting_a_category_resets_only_a_default_that_names_it() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        lib.default_manga_category.set(5).unwrap();
        lib.default_anime_category.set(3).unwrap();

        forget_category(&store, &lib, Kind::Manga, 5).unwrap();
        forget_category(&store, &lib, Kind::Anime, 5).unwrap();

        assert_eq!(lib.default_manga_category.get(), -1, "the default named the deleted category");
        assert_eq!(lib.default_anime_category.get(), 3, "another category is still the default");
    }

    fn id_set(items: &[&str]) -> BTreeSet<String> {
        ids(items)
    }

    #[test]
    fn pruning_after_deleting_category_7_clears_it_from_every_set_and_keeps_the_system_id() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        for key in category_id_set_keys() {
            store.pref::<BTreeSet<String>>(key, BTreeSet::new()).set(ids(&["0", "7", "8"])).unwrap();
        }
        let manga = id_set(&["0", "8"]);
        let anime = id_set(&["0"]);

        let removed = prune_dangling_category_ids(&store, &lib, &manga, &anime).unwrap();

        assert_eq!(removed, 10, "one dangling id in each of the ten sets");
        for key in category_id_set_keys() {
            assert_eq!(set_of(&store, key), ids(&["0", "8"]), "{key}");
        }
    }

    #[test]
    fn pruning_resets_a_dangling_default_and_keeps_a_known_one() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        lib.default_manga_category.set(7).unwrap();
        lib.default_anime_category.set(3).unwrap();
        // 3 exists in the manga database only: the check looks at both sets.
        prune_dangling_category_ids(&store, &lib, &id_set(&["0", "3"]), &id_set(&["0"])).unwrap();
        assert!(!lib.default_manga_category.is_set(), "7 is in neither set");
        assert_eq!(lib.default_manga_category.get(), -1);
        assert_eq!(lib.default_anime_category.get(), 3);
    }

    #[test]
    fn pruning_leaves_sets_without_dangling_ids_unwritten() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        store.pref::<BTreeSet<String>>("library_update_categories", BTreeSet::new()).set(ids(&["1"])).unwrap();
        let removed = prune_dangling_category_ids(&store, &lib, &id_set(&["0", "1"]), &id_set(&["0"])).unwrap();
        assert_eq!(removed, 0);
        assert!(store.get_raw("download_new_categories").is_none(), "an absent set is not created");
        assert!(!lib.default_manga_category.is_set(), "the unset default stays unset");
    }

    #[test]
    fn a_sort_write_sets_the_global_sort_only_when_global() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());

        write_sort_prefs(&lib, Kind::Manga, "LAST_MANGA_UPDATE,DESCENDING", true).unwrap();
        assert_eq!(lib.manga_sorting_mode.get().0, "LAST_MANGA_UPDATE,DESCENDING");

        write_sort_prefs(&lib, Kind::Manga, "RANDOM,ASCENDING", false).unwrap();
        assert_eq!(lib.manga_sorting_mode.get().0, "LAST_MANGA_UPDATE,DESCENDING", "a category took the change");
    }

    #[test]
    fn the_random_seed_is_kept_in_memory_per_kind() {
        set_random_sort_seed(Kind::Anime, 4242);
        assert_eq!(random_sort_seed(Kind::Anime), 4242);
        set_random_sort_seed(Kind::Manga, -7);
        assert_eq!(random_sort_seed(Kind::Manga), -7);
        assert_eq!(random_sort_seed(Kind::Anime), 4242, "the other kind keeps its own seed");
    }
}
