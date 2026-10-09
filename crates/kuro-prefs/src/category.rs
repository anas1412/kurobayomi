//! Preference side of the category rules (T-05a-025). Deleting a category clears its id from the category
//! sets and resets the default category. A sort-mode change writes the sort string and, for Random, a new
//! seed. The category rows are in `kuro-data::category`, and the sort codec is `kuro_db::flags::library_sort`.

use std::collections::BTreeSet;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use crate::PrefError;
use crate::library::{LibraryPrefs, SortSetting};
use crate::store::PrefStore;

/// The kind of category. It selects the keys of its preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CategoryKind {
    Manga,
    Anime,
}

/// The five category-id sets of a kind: library update include and exclude, the remove-exclude set, and the
/// download-new include and exclude sets. Manga's exclude set is cleaned too (EC-11).
fn category_sets(kind: CategoryKind) -> [&'static str; 5] {
    match kind {
        CategoryKind::Manga => [
            "library_update_categories",
            "library_update_categories_exclude",
            "remove_exclude_categories",
            "download_new_categories",
            "download_new_categories_exclude",
        ],
        CategoryKind::Anime => [
            "animelib_update_categories",
            "animelib_update_categories_exclude",
            "remove_exclude_anime_categories",
            "download_new_anime_categories",
            "download_new_anime_categories_exclude",
        ],
    }
}

/// The ten category-id preference sets of both kinds: the library update include and exclude sets, the
/// remove-exclude set and the download-new include and exclude sets. Startup cleanup and backup restore walk them.
pub fn category_id_set_keys() -> Vec<&'static str> {
    category_sets(CategoryKind::Manga).into_iter().chain(category_sets(CategoryKind::Anime)).collect()
}

/// `DeleteMangaCategory` and `DeleteAnimeCategory`, preference side. The default category is reset when it
/// is `id`, and `id` leaves each of the five category sets.
pub fn forget_category(store: &PrefStore, lib: &LibraryPrefs, kind: CategoryKind, id: i64) -> Result<(), PrefError> {
    let default = match kind {
        CategoryKind::Manga => &lib.default_manga_category,
        CategoryKind::Anime => &lib.default_anime_category,
    };
    if i64::from(default.get()) == id {
        default.delete()?;
    }
    let needle = id.to_string();
    for key in category_sets(kind) {
        let set = store.pref::<BTreeSet<String>>(key, BTreeSet::new());
        let mut ids = set.get();
        if ids.remove(&needle) {
            set.set(ids)?;
        }
    }
    Ok(())
}

/// The preference writes of `SetSortModeForMangaCategory`. `serialized_sort` is the stored sort string such
/// as `RANDOM,ASCENDING`. `random` draws a new seed for the Random sort. `global` writes the sort into the
/// global preference, which is false when a category takes the change instead.
pub fn write_sort_prefs(lib: &LibraryPrefs, kind: CategoryKind, serialized_sort: &str, random: bool, global: bool) -> Result<(), PrefError> {
    if random {
        match kind {
            CategoryKind::Manga => lib.random_manga_sort_seed.set(new_random_seed())?,
            CategoryKind::Anime => lib.random_anime_sort_seed.set(new_random_seed())?,
        }
    }
    if global {
        match kind {
            CategoryKind::Manga => lib.manga_sorting_mode.set(SortSetting(serialized_sort.to_owned()))?,
            CategoryKind::Anime => lib.anime_sorting_mode.set(SortSetting(serialized_sort.to_owned()))?,
        }
    }
    Ok(())
}

/// A new seed for the Random sort, the `Random.nextInt()` of Aniyomi. Each call takes fresh keys from the
/// std hasher, so consecutive seeds differ.
pub fn new_random_seed() -> i32 {
    RandomState::new().build_hasher().finish() as i32
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
        fill(&store, category_sets(CategoryKind::Manga), &["5", "7"]);
        fill(&store, category_sets(CategoryKind::Anime), &["5"]);

        forget_category(&store, &lib, CategoryKind::Manga, 5).unwrap();

        for key in category_sets(CategoryKind::Manga) {
            assert_eq!(set_of(&store, key), ids(&["7"]), "{key} lost the deleted id");
        }
        for key in category_sets(CategoryKind::Anime) {
            assert_eq!(set_of(&store, key), ids(&["5"]), "{key} belongs to the other kind");
        }
    }

    #[test]
    fn the_manga_exclude_set_loses_the_deleted_id_too() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        store.pref::<BTreeSet<String>>("library_update_categories", BTreeSet::new()).set(ids(&["5"])).unwrap();
        store.pref::<BTreeSet<String>>("library_update_categories_exclude", BTreeSet::new()).set(ids(&["5", "6"])).unwrap();

        forget_category(&store, &lib, CategoryKind::Manga, 5).unwrap();

        assert_eq!(set_of(&store, "library_update_categories_exclude"), ids(&["6"]));
        assert!(set_of(&store, "library_update_categories").is_empty());
    }

    #[test]
    fn deleting_a_category_resets_only_a_default_that_names_it() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        lib.default_manga_category.set(5).unwrap();
        lib.default_anime_category.set(3).unwrap();

        forget_category(&store, &lib, CategoryKind::Manga, 5).unwrap();
        forget_category(&store, &lib, CategoryKind::Anime, 5).unwrap();

        assert_eq!(lib.default_manga_category.get(), -1, "the default named the deleted category");
        assert_eq!(lib.default_anime_category.get(), 3, "another category is still the default");
    }

    #[test]
    fn a_sort_write_sets_the_global_sort_and_a_new_seed_only_for_random() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());

        write_sort_prefs(&lib, CategoryKind::Manga, "LAST_MANGA_UPDATE,DESCENDING", false, true).unwrap();
        assert_eq!(lib.manga_sorting_mode.get().0, "LAST_MANGA_UPDATE,DESCENDING");
        assert_eq!(lib.random_manga_sort_seed.get(), 0);

        write_sort_prefs(&lib, CategoryKind::Manga, "RANDOM,ASCENDING", true, false).unwrap();
        assert_eq!(lib.manga_sorting_mode.get().0, "LAST_MANGA_UPDATE,DESCENDING", "a category took the change");
        assert_ne!(lib.random_manga_sort_seed.get(), 0);
    }

    #[test]
    fn consecutive_random_seeds_differ() {
        assert_ne!(new_random_seed(), new_random_seed());
    }
}
