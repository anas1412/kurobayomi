//! Preference restore (area 22, T-22a-009 and T-22a-010). The write rules live in `kuro_prefs::import`; this module
//! converts the backup values, maps the category ids, and re-arms the schedulers after an app-preference section.

use std::collections::{BTreeMap, HashMap};

use kuro_prefs::import::{ANIME_CATEGORY_KEYS, CategoryMaps, ImportReport, MANGA_CATEGORY_KEYS, import_preferences};
use kuro_prefs::registry::{self, Kind};
use kuro_prefs::store::PrefStore;
use kuro_prefs::values::PrefValue as Value;
use kuro_db::DbResult;
use rusqlite::Connection;

use crate::schema::{BackupCategory, BackupPreference, BackupSourcePreferences, PrefValue};

/// The scheduler re-arm after app preferences change (p1-rust-design.md section 9). Implemented by the owning areas.
pub trait Scheduler {
    /// Reads the auto-update interval and restrictions. An interval of 0 disables the timer.
    fn rearm_library_updates(&self);
    /// Reads `backup_interval`. 0 disables the automatic backup.
    fn rearm_auto_backup(&self);
}

/// Reads the two interval preferences after a restore and hands each timer its state: `None` disables it (interval 0),
/// `Some(hours)` enables it with the restored value (T-22a-010). The callbacks belong to the owning areas; passing
/// `&|_| {}` makes this a no-op until they land.
pub struct IntervalScheduler<'a> {
    pub store: &'a PrefStore,
    pub library_timer: &'a dyn Fn(Option<u32>),
    pub backup_timer: &'a dyn Fn(Option<u32>),
}

impl IntervalScheduler<'_> {
    fn hours(&self, key: &str, default: i32) -> Option<u32> {
        let n = match self.store.get_raw(key) {
            Some(Value::Int(n)) => n,
            _ => default,
        };
        u32::try_from(n).ok().filter(|h| *h > 0)
    }
}

impl Scheduler for IntervalScheduler<'_> {
    fn rearm_library_updates(&self) {
        (self.library_timer)(self.hours("pref_library_update_interval_key", 0));
    }

    fn rearm_auto_backup(&self) {
        (self.backup_timer)(self.hours("backup_interval", 12));
    }
}

fn convert(v: &PrefValue) -> Value {
    match v {
        PrefValue::Int(i) => Value::Int(*i),
        PrefValue::Long(l) => Value::Long(*l),
        PrefValue::Float(f) => Value::Float(*f),
        PrefValue::String(s) => Value::Str(s.clone()),
        PrefValue::Boolean(b) => Value::Bool(*b),
        PrefValue::StringSet(s) => Value::Set(s.clone()),
    }
}

/// The two default-category keys hold one id, remapped (T-22a-009).
fn is_default_category(key: &str) -> bool {
    key == "default_category" || key == "default_anime_category"
}

/// The category id-set keys: the twelve category keys minus the two defaults.
fn is_category_set(key: &str) -> bool {
    MANGA_CATEGORY_KEYS[1..].contains(&key) || ANIME_CATEGORY_KEYS[1..].contains(&key)
}

/// Turns the backup entries into the writes that `import_preferences` performs. Three rules sit on top of it:
/// an unmapped default id (including -1) is dropped, so the setting stays; a set with no mapped id is dropped; and a
/// mapped set is cleared first, so the import replaces it instead of uniting it with the device set (D12).
fn prepare(store: &PrefStore, prefs: &[BackupPreference], maps: &CategoryMaps) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for p in prefs {
        let key = p.key.as_str();
        let value = convert(&p.value);
        // A stored value of another type wins, for registered and unregistered keys alike (E-53). An absent key is written.
        if store.get_raw(key).is_some_and(|stored| std::mem::discriminant(&stored) != std::mem::discriminant(&value)) {
            continue;
        }
        if is_default_category(key) {
            let map = map_for(key, maps);
            match value {
                Value::Int(old) if !map.contains_key(&i64::from(old)) => continue,
                _ => out.push((p.key.clone(), value)),
            }
        } else if is_category_set(key) {
            if let Value::Set(ids) = &value {
                let map = map_for(key, maps);
                let mapped = ids.iter().any(|id| id.parse::<i64>().is_ok_and(|old| map.contains_key(&old)));
                if !mapped {
                    continue;
                }
                if matches!(registry::lookup(key).map(|(m, _)| &m.kind), Some(Kind::Set)) {
                    store.remove(key);
                }
            }
            out.push((p.key.clone(), value));
        } else {
            out.push((p.key.clone(), value));
        }
    }
    out
}

fn map_for<'a>(key: &str, maps: &'a CategoryMaps) -> &'a BTreeMap<i64, i64> {
    if MANGA_CATEGORY_KEYS.contains(&key) { &maps.manga } else { &maps.anime }
}

/// Restores the app preferences (T-22a-009), writes `categorized_display` when the categories section computed it,
/// then re-arms both schedulers once each (T-22a-010, X-57).
pub fn restore_app_prefs(
    store: &PrefStore,
    prefs: &[BackupPreference],
    maps: &CategoryMaps,
    categorized_display: Option<bool>,
    scheduler: &dyn Scheduler,
) -> ImportReport {
    let entries = prepare(store, prefs, maps);
    // No credentials are passed, so `import_preferences` has no error path. Private keys are counted as skipped.
    let report = import_preferences(store, entries, maps, None, false).unwrap_or_default();
    if let Some(flag) = categorized_display {
        store.set_raw("categorized_display", Value::Bool(flag));
    }
    scheduler.rearm_library_updates();
    scheduler.rearm_auto_backup();
    report
}

/// Restores each source's preferences into the store `source_<sourceId>` (T-22a-009, E-52). `open` returns the store
/// for a name. Source files carry no category ids, so no maps apply.
pub fn restore_source_prefs(
    open: &dyn Fn(&str) -> PrefStore,
    sources: &[BackupSourcePreferences],
) -> Vec<ImportReport> {
    let none = CategoryMaps::default();
    sources
        .iter()
        .map(|s| {
            let store = open(&format!("source_{}", s.source_key));
            let entries = prepare(&store, &s.prefs, &none);
            import_preferences(&store, entries, &none, None, false).unwrap_or_default()
        })
        .collect()
}

/// The category maps of the preference restore (T-22a-009): backup category id (the device id) to the database id of
/// the same name, once for manga and once for anime. Both lists come from the backup's manga categories.
pub fn category_maps(
    manga: &Connection,
    anime: &Connection,
    backup: &[BackupCategory],
) -> DbResult<CategoryMaps> {
    let manga_by_name: HashMap<String, i64> = kuro_db::statements_manga::categories::get_categories(manga)?
        .into_iter()
        .map(|r| (r.name, r.id))
        .collect();
    let anime_by_name: HashMap<String, i64> = kuro_db::statements_anime::categories(anime)?
        .into_iter()
        .map(|r| (r.name, r.id))
        .collect();
    let mut maps = CategoryMaps::default();
    for b in backup {
        if let Some(&id) = manga_by_name.get(&b.name) {
            maps.manga.insert(b.id, id);
        }
        if let Some(&id) = anime_by_name.get(&b.name) {
            maps.anime.insert(b.id, id);
        }
    }
    Ok(maps)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeSet;

    use super::*;

    #[derive(Default)]
    struct Count {
        library: Cell<u32>,
        backup: Cell<u32>,
    }

    impl Scheduler for Count {
        fn rearm_library_updates(&self) {
            self.library.set(self.library.get() + 1);
        }
        fn rearm_auto_backup(&self) {
            self.backup.set(self.backup.get() + 1);
        }
    }

    fn pref(key: &str, value: PrefValue) -> BackupPreference {
        BackupPreference { key: key.into(), value }
    }

    fn set(ids: &[&str]) -> PrefValue {
        PrefValue::StringSet(ids.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>())
    }

    fn maps() -> CategoryMaps {
        CategoryMaps { manga: BTreeMap::from([(5, 9), (6, 10)]), anime: BTreeMap::from([(5, 20)]) }
    }

    #[test]
    fn stored_type_mismatch_is_skipped_and_absent_key_is_written() {
        let store = PrefStore::in_memory();
        store.set_raw("show_nsfw_source", Value::Bool(false));
        let sched = Count::default();
        let prefs = vec![pref("show_nsfw_source", PrefValue::String("yes".into()))];
        restore_app_prefs(&store, &prefs, &CategoryMaps::default(), None, &sched);
        assert_eq!(store.get_raw("show_nsfw_source"), Some(Value::Bool(false)));
    }

    #[test]
    fn unregistered_key_with_a_different_stored_type_is_skipped() {
        let store = PrefStore::in_memory();
        store.set_raw("not_in_registry", Value::Bool(true));
        let prefs = vec![
            pref("not_in_registry", PrefValue::String("x".into())),
            pref("other_unknown", PrefValue::String("y".into())),
        ];
        restore_app_prefs(&store, &prefs, &CategoryMaps::default(), None, &Count::default());
        assert_eq!(store.get_raw("not_in_registry"), Some(Value::Bool(true)));
        assert_eq!(store.get_raw("other_unknown"), Some(Value::Str("y".into())), "absent key is written");
    }

    #[test]
    fn absent_registry_key_with_matching_kind_is_written() {
        let store = PrefStore::in_memory();
        restore_app_prefs(
            &store,
            &[pref("show_nsfw_source", PrefValue::Boolean(true))],
            &CategoryMaps::default(),
            None,
            &Count::default(),
        );
        assert_eq!(store.get_raw("show_nsfw_source"), Some(Value::Bool(true)));
    }

    #[test]
    fn default_category_is_remapped_and_unmapped_or_minus_one_keeps_the_setting() {
        let store = PrefStore::in_memory();
        store.set_raw("default_category", Value::Int(3));
        let sched = Count::default();
        restore_app_prefs(&store, &[pref("default_category", PrefValue::Int(5))], &maps(), None, &sched);
        assert_eq!(store.get_raw("default_category"), Some(Value::Int(9)));

        restore_app_prefs(&store, &[pref("default_category", PrefValue::Int(-1))], &maps(), None, &sched);
        restore_app_prefs(&store, &[pref("default_category", PrefValue::Int(77))], &maps(), None, &sched);
        assert_eq!(store.get_raw("default_category"), Some(Value::Int(9)));
    }

    #[test]
    fn anime_default_uses_the_anime_map() {
        let store = PrefStore::in_memory();
        restore_app_prefs(
            &store,
            &[pref("default_anime_category", PrefValue::Int(5))],
            &maps(),
            None,
            &Count::default(),
        );
        assert_eq!(store.get_raw("default_anime_category"), Some(Value::Int(20)));
    }

    #[test]
    fn category_set_is_replaced_not_united_and_untouched_when_nothing_maps() {
        let store = PrefStore::in_memory();
        store.set_raw("library_update_categories", Value::Set(BTreeSet::from(["1".to_string()])));
        restore_app_prefs(
            &store,
            &[pref("library_update_categories", set(&["5", "6"]))],
            &maps(),
            None,
            &Count::default(),
        );
        assert_eq!(
            store.get_raw("library_update_categories"),
            Some(Value::Set(BTreeSet::from(["10".to_string(), "9".to_string()])))
        );

        restore_app_prefs(
            &store,
            &[pref("library_update_categories", set(&["99"]))],
            &maps(),
            None,
            &Count::default(),
        );
        assert_eq!(
            store.get_raw("library_update_categories"),
            Some(Value::Set(BTreeSet::from(["9".to_string(), "10".to_string()])))
        );
    }

    #[test]
    fn both_schedulers_are_rearmed_exactly_once_per_app_section() {
        let store = PrefStore::in_memory();
        let sched = Count::default();
        restore_app_prefs(&store, &[], &CategoryMaps::default(), None, &sched);
        assert_eq!((sched.library.get(), sched.backup.get()), (1, 1));
    }

    #[test]
    fn interval_zero_disables_the_timer_and_non_zero_enables_it() {
        let store = PrefStore::in_memory();
        let calls = std::cell::RefCell::new(Vec::new());
        let library = |h: Option<u32>| calls.borrow_mut().push(("library", h));
        let backup = |h: Option<u32>| calls.borrow_mut().push(("backup", h));
        let sched = IntervalScheduler { store: &store, library_timer: &library, backup_timer: &backup };
        let prefs = [
            pref("pref_library_update_interval_key", PrefValue::Int(0)),
            pref("backup_interval", PrefValue::Int(0)),
        ];
        restore_app_prefs(&store, &prefs, &CategoryMaps::default(), None, &sched);
        let prefs = [
            pref("pref_library_update_interval_key", PrefValue::Int(24)),
            pref("backup_interval", PrefValue::Int(6)),
        ];
        restore_app_prefs(&store, &prefs, &CategoryMaps::default(), None, &sched);
        assert_eq!(
            calls.into_inner(),
            vec![("library", None), ("backup", None), ("library", Some(24)), ("backup", Some(6))]
        );
    }

    #[test]
    fn absent_backup_interval_uses_the_default_of_twelve_hours() {
        let store = PrefStore::in_memory();
        let got = std::cell::Cell::new(None);
        let backup = |h: Option<u32>| got.set(h);
        IntervalScheduler { store: &store, library_timer: &|_| {}, backup_timer: &backup }.rearm_auto_backup();
        assert_eq!(got.get(), Some(12));
    }

    #[test]
    fn categorized_display_is_written_only_when_computed() {
        let store = PrefStore::in_memory();
        restore_app_prefs(&store, &[], &CategoryMaps::default(), Some(true), &Count::default());
        assert_eq!(store.get_raw("categorized_display"), Some(Value::Bool(true)));
    }

    #[test]
    fn source_preferences_go_to_the_store_named_for_each_source() {
        let names = std::cell::RefCell::new(Vec::new());
        let open = |name: &str| {
            names.borrow_mut().push(name.to_owned());
            PrefStore::in_memory()
        };
        let sources = vec![
            BackupSourcePreferences { source_key: "1234".into(), prefs: vec![pref("show_nsfw_source", PrefValue::Boolean(true))] },
            BackupSourcePreferences { source_key: "99".into(), prefs: vec![] },
        ];
        let reports = restore_source_prefs(&open, &sources);
        assert_eq!(names.into_inner(), vec!["source_1234".to_string(), "source_99".to_string()]);
        assert_eq!(reports.len(), 2);
    }
}
