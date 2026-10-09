//! Category data in backups, the remap of category-id preference sets, and the startup cleanup (T-08a-035). Pure:
//! the backup and restore areas read and write the files; this module decides what changes.

use std::collections::{BTreeMap, BTreeSet};

use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::store::PrefStore;

use super::model::Category;
use crate::SYSTEM_CATEGORY_ID;

/// A category as the backup stores it: name, order and flags. Hidden is not exported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupCategory {
    pub name: String,
    pub order: i64,
    pub flags: i64,
}

/// Every non-system category of the library, in its order.
pub fn export(categories: &[Category]) -> Vec<BackupCategory> {
    categories
        .iter()
        .filter(|c| !c.is_system())
        .map(|c| BackupCategory { name: c.name.clone(), order: c.order, flags: c.flags })
        .collect()
}

/// What a restore inserts. Categories match by exact name; the rest are appended in backup order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestorePlan {
    pub inserts: Vec<BackupCategory>,
    /// `categorized_display` is set when the categories carry more than one distinct flag value.
    pub categorized_display: bool,
}

/// `max(existing order) + 1` for the first new category, then consecutive. 0 when no order exists yet.
pub fn plan_restore(existing: &[Category], backup: &[BackupCategory]) -> RestorePlan {
    let names: BTreeSet<&str> = existing.iter().filter(|c| !c.is_system()).map(|c| c.name.as_str()).collect();
    let mut next_order = existing.iter().map(|c| c.order).max().map_or(0, |m| m + 1);
    let mut inserts = Vec::new();
    for cat in backup {
        if names.contains(cat.name.as_str()) || inserts.iter().any(|i: &BackupCategory| i.name == cat.name) {
            continue;
        }
        inserts.push(BackupCategory { name: cat.name.clone(), order: next_order, flags: cat.flags });
        next_order += 1;
    }
    let flags: BTreeSet<i64> = existing.iter().map(|c| c.flags).chain(inserts.iter().map(|i| i.flags)).collect();
    RestorePlan { inserts, categorized_display: flags.len() > 1 }
}

/// Maps a preference set of category ids from the backup to this database. Each backup id is looked up by name, and
/// the name is looked up in each kind's new ids (`new_ids_by_name`: manga, then anime; both are tried). Ids that match
/// nothing are dropped. The result is appended to `current`.
pub fn remap_ids(
    current: &[i64],
    backup_ids: &[i64],
    backup_names: &BTreeMap<i64, String>,
    new_ids_by_name: &[&BTreeMap<String, i64>],
) -> Vec<i64> {
    let mut out: BTreeSet<i64> = current.iter().copied().collect();
    for id in backup_ids {
        let Some(name) = backup_names.get(id) else { continue };
        for by_name in new_ids_by_name {
            if let Some(new) = by_name.get(name) {
                out.insert(*new);
            }
        }
    }
    out.into_iter().collect()
}

/// Restore side of the ten category-id sets (T-08a-035). `backup_sets` maps a preference key to the backup ids it held;
/// each key present in it is remapped with [`remap_ids`] and written back. Keys absent from the backup are left alone.
pub fn remap_preference_sets(
    store: &PrefStore,
    backup_sets: &BTreeMap<String, Vec<i64>>,
    backup_names: &BTreeMap<i64, String>,
    manga_by_name: &BTreeMap<String, i64>,
    anime_by_name: &BTreeMap<String, i64>,
) {
    for key in kuro_prefs::category::category_id_set_keys() {
        let Some(backup_ids) = backup_sets.get(key) else { continue };
        let set = store.pref::<BTreeSet<String>>(key, BTreeSet::new());
        let current: Vec<i64> = set.get().iter().filter_map(|s| s.parse().ok()).collect();
        let merged = remap_ids(&current, backup_ids, backup_names, &[manga_by_name, anime_by_name]);
        set.set(merged.iter().map(i64::to_string).collect());
    }
}

/// Whether `id` is a category of either database, or the Default id, which is never stale.
fn is_known(id: i64, known_manga: &BTreeSet<i64>, known_anime: &BTreeSet<i64>) -> bool {
    id == SYSTEM_CATEGORY_ID || known_manga.contains(&id) || known_anime.contains(&id)
}

/// Startup cleanup (E19): the ids of a category-id set that exist in neither database. The Default id is never
/// removed.
pub fn stale_ids(set: &[i64], known_manga: &BTreeSet<i64>, known_anime: &BTreeSet<i64>) -> Vec<i64> {
    set.iter().copied().filter(|id| !is_known(*id, known_manga, known_anime)).collect()
}

/// Startup cleanup (E19, T-08a-035). Runs once both databases are open. Every set of the ten category-id preference
/// keys loses the entries that name a category in neither database (a non-numeric entry is dropped too), and a default
/// category preference that names such a category is deleted, which reads back as "always ask". The Default id 0 and
/// the "always ask" value -1 are kept.
pub fn startup_cleanup(store: &PrefStore, lib: &LibraryPrefs, known_manga: &BTreeSet<i64>, known_anime: &BTreeSet<i64>) {
    for key in kuro_prefs::category::category_id_set_keys() {
        let set = store.pref::<BTreeSet<String>>(key, BTreeSet::new());
        let current = set.get();
        let kept: BTreeSet<String> = current
            .iter()
            .filter(|entry| matches!(entry.parse::<i64>(), Ok(id) if is_known(id, known_manga, known_anime)))
            .cloned()
            .collect();
        if kept.len() != current.len() {
            set.set(kept);
        }
    }
    for default in [&lib.default_manga_category, &lib.default_anime_category] {
        let id = i64::from(default.get());
        if default.is_set() && id >= 1 && !is_known(id, known_manga, known_anime) {
            default.delete();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    fn cat(id: i64, name: &str, order: i64, flags: i64, hidden: bool) -> Category {
        Category { id, name: name.into(), order, flags, hidden }
    }

    #[test]
    fn export_skips_the_system_category_and_hidden_is_not_written() {
        let all = vec![cat(0, "Default", 0, 0, false), cat(1, "Read", 1, 4, true)];
        assert_eq!(export(&all), vec![BackupCategory { name: "Read".into(), order: 1, flags: 4 }]);
    }

    #[test]
    fn restore_appends_new_names_after_the_highest_order() {
        let existing = vec![cat(0, "Default", 0, 64, false), cat(1, "A", 3, 64, false)];
        let backup = vec![
            BackupCategory { name: "A".into(), order: 0, flags: 64 },
            BackupCategory { name: "B".into(), order: 1, flags: 4 },
            BackupCategory { name: "C".into(), order: 2, flags: 64 },
        ];
        let plan = plan_restore(&existing, &backup);
        let got: Vec<(String, i64)> = plan.inserts.iter().map(|c| (c.name.clone(), c.order)).collect();
        assert_eq!(got, vec![("B".to_owned(), 4), ("C".to_owned(), 5)]);
        assert!(plan.categorized_display, "flags 64 and 4 differ");
        let none = plan_restore(&[], &backup[..1]);
        assert_eq!(none.inserts[0].order, 0, "no existing order gives 0");
        assert!(!none.categorized_display);
    }

    #[test]
    fn category_id_sets_are_remapped_through_names_and_stale_ids_are_dropped() {
        let backup_names = BTreeMap::from([(7, "Read".to_owned()), (8, "Gone".to_owned())]);
        let new_by_name = BTreeMap::from([("Read".to_owned(), 21)]);
        assert_eq!(remap_ids(&[5], &[7, 8], &backup_names, &[&new_by_name]), vec![5, 21]);
    }

    #[test]
    fn remap_tries_both_kinds_and_keeps_the_current_members() {
        let backup_names = BTreeMap::from([(7, "Read".to_owned())]);
        let manga = BTreeMap::from([("Read".to_owned(), 21)]);
        let anime = BTreeMap::from([("Read".to_owned(), 31)]);
        assert_eq!(remap_ids(&[5], &[7], &backup_names, &[&manga, &anime]), vec![5, 21, 31]);
    }

    #[test]
    fn remap_preference_sets_writes_back_only_the_keys_in_the_backup() {
        let store = PrefStore::in_memory();
        store.pref::<BTreeSet<String>>("library_update_categories", BTreeSet::new()).set(BTreeSet::from(["5".to_owned()]));
        let backup_sets = BTreeMap::from([("library_update_categories".to_owned(), vec![7])]);
        let backup_names = BTreeMap::from([(7, "Read".to_owned())]);
        let manga = BTreeMap::from([("Read".to_owned(), 21)]);
        remap_preference_sets(&store, &backup_sets, &backup_names, &manga, &BTreeMap::new());
        let set = store.pref::<BTreeSet<String>>("library_update_categories", BTreeSet::new());
        assert_eq!(set.get(), BTreeSet::from(["5".to_owned(), "21".to_owned()]));
        let untouched = store.pref::<BTreeSet<String>>("download_new_categories", BTreeSet::new());
        assert!(!untouched.is_set());
    }

    #[test]
    fn startup_cleanup_clears_stale_entries_from_every_set_and_resets_the_defaults() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        let manga: BTreeSet<i64> = [1, 2].into_iter().collect();
        let anime: BTreeSet<i64> = [3].into_iter().collect();
        store.pref::<BTreeSet<String>>("library_update_categories", BTreeSet::new()).set(
            ["0", "1", "3", "9", "x"].into_iter().map(str::to_owned).collect(),
        );
        store.pref::<BTreeSet<String>>("download_new_anime_categories_exclude", BTreeSet::new()).set(
            ["9"].into_iter().map(str::to_owned).collect(),
        );
        lib.default_manga_category.set(9);
        lib.default_anime_category.set(3);
        startup_cleanup(&store, &lib, &manga, &anime);
        let update = store.pref::<BTreeSet<String>>("library_update_categories", BTreeSet::new());
        assert_eq!(update.get(), BTreeSet::from(["0".to_owned(), "1".to_owned(), "3".to_owned()]));
        let exclude = store.pref::<BTreeSet<String>>("download_new_anime_categories_exclude", BTreeSet::new());
        assert!(exclude.get().is_empty(), "a set that held only stale ids is emptied");
        assert_eq!(lib.default_manga_category.get(), -1, "a stale default is reset");
        assert_eq!(lib.default_anime_category.get(), 3, "a live default is kept");
    }

    #[test]
    fn startup_cleanup_removes_ids_present_in_neither_database() {
        let manga: BTreeSet<i64> = [1, 2].into_iter().collect();
        let anime: BTreeSet<i64> = [3].into_iter().collect();
        assert_eq!(stale_ids(&[0, 1, 3, 9], &manga, &anime), vec![9]);
    }
}
