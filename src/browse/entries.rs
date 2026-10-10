//! What a browse page does to the library: the store it talks to, the add and remove sequences, and the mapping of a
//! listed entry to its library row (T-40b-033, T-40b-041, T-40b-042, T-40b-047). The sequences are the pure rules of
//! `favorite`, run against a `FavoriteStore`; the GTK side is `favorite_flow`.

use std::collections::BTreeSet;
use std::sync::Arc;

use kuro_domain::category::target::resolve_add_target;
use tokio::sync::watch;

use super::favorite::{AddOutcome, FavoriteWrite, add, cover_stamp_after_remove, toggle_write};

/// The library row of a listed entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalEntry {
    pub id: i64,
    pub favorite: bool,
    /// The title the row shows. An existing row shows its stored title; a row the listing just inserted has the listed one.
    pub title: String,
}

/// A category the chooser offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryChoice {
    pub id: i64,
    pub name: String,
}

/// Starts the enhanced-tracker bindings of a newly added entry: the entry id, its title and its source id. It must not
/// block: the add does not wait for the trackers (B§5.2).
pub type TrackerBinder = Arc<dyn Fn(i64, &str, i64) + Send + Sync>;

/// The library as the browse pages use it. Every call blocks, so the page runs them off the GTK thread.
pub trait FavoriteStore: Send + Sync + 'static {
    /// `NetworkToLocalManga`: the row of `(source_id, url)`, inserted as a non-favourite when none exists.
    fn local_entry(&self, source_id: i64, url: &str, title: &str, cover: Option<&str>) -> Result<LocalEntry, String>;
    /// The ids of the other library entries that carry `title`.
    fn duplicates(&self, id: i64, title: &str) -> Result<Vec<i64>, String>;
    /// The user's categories.
    fn categories(&self) -> Result<Vec<CategoryChoice>, String>;
    /// `default_category`: -1 asks, 0 is the Default category, anything else a category id.
    fn default_category(&self) -> i64;
    /// Writes `favorite` and `date_added` and no other column (B-EC-12).
    fn write_favorite(&self, id: i64, write: FavoriteWrite) -> Result<(), String>;
    /// Replaces the entry's category links; an empty list is the Default category.
    fn set_categories(&self, id: i64, ids: &[i64]) -> Result<(), String>;
    /// Deletes the entry's cached cover files and returns how many files went.
    fn clear_cover(&self, id: i64) -> usize;
    /// Writes `cover_last_modified`.
    fn stamp_cover(&self, id: i64, now_ms: i64) -> Result<(), String>;
    /// Deletes the entry's cached background files and returns how many went. Manga has none, so the default is 0.
    fn clear_background(&self, _id: i64) -> usize {
        0
    }
    /// Writes `background_last_modified`. Manga has none, so the default writes nothing.
    fn stamp_background(&self, _id: i64, _now_ms: i64) -> Result<(), String> {
        Ok(())
    }
    /// What an add does besides the favourite and the categories: the default episode flags and the enhanced-tracker
    /// bindings. A store with neither has nothing to do.
    fn after_add(&self, _id: i64) -> Result<(), String> {
        Ok(())
    }
    /// The urls of the source's library entries, published again after every change to them.
    fn favorite_urls(&self, source_id: i64) -> watch::Receiver<BTreeSet<String>>;
}

/// Adds the entry. A resolved target writes its categories first and the favourite second, then the defaults and
/// tracker bindings of `after_add` run; an `Ask` target writes
/// nothing and asks for the chooser. A category failure is logged and does not block the favourite (E-FAV-02).
pub fn add_entry(store: &dyn FavoriteStore, id: i64, now_ms: i64) -> AddOutcome<String> {
    let ids: Vec<i64> = store.categories().map(|all| all.into_iter().map(|c| c.id).collect()).unwrap_or_default();
    let target = resolve_add_target(store.default_category(), &ids);
    add_with(store, id, now_ms, &target)
}

/// Adds the entry to the categories the user picked in the chooser.
pub fn add_entry_to(store: &dyn FavoriteStore, id: i64, now_ms: i64, categories: Vec<i64>) -> AddOutcome<String> {
    add_with(store, id, now_ms, &kuro_domain::category::target::AddTarget::Categories(categories))
}

fn add_with(
    store: &dyn FavoriteStore,
    id: i64,
    now_ms: i64,
    target: &kuro_domain::category::target::AddTarget,
) -> AddOutcome<String> {
    let outcome = add(target, now_ms, |ids| store.set_categories(id, ids), |write| store.write_favorite(id, write));
    if let AddOutcome::Added { category_error } = &outcome {
        if let Some(error) = category_error {
            tracing::warn!(entry = id, %error, "the categories of an added entry could not be written");
        }
        if let Err(error) = store.after_add(id) {
            tracing::warn!(entry = id, %error, "the defaults of an added entry could not be applied");
        }
    }
    outcome
}

/// Removes the entry from the library: the favourite write, then the cover cache, then `cover_last_modified` when at
/// least one file was deleted (B-EC-13). The background cache follows the same rule (anime only).
pub fn remove_entry(store: &dyn FavoriteStore, id: i64, now_ms: i64) -> Result<(), String> {
    store.write_favorite(id, toggle_write(true, now_ms))?;
    if let Some(stamp) = cover_stamp_after_remove(store.clear_cover(id), now_ms) {
        store.stamp_cover(id, stamp)?;
    }
    if let Some(stamp) = cover_stamp_after_remove(store.clear_background(id), now_ms) {
        store.stamp_background(id, stamp)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod fake {
    use std::sync::Mutex;

    use super::*;

    /// A store that records every call as text, and fails the calls named in `fail`.
    #[derive(Default)]
    pub struct FakeStore {
        pub log: Mutex<Vec<String>>,
        pub fail: Mutex<Vec<&'static str>>,
        pub default_category: Mutex<i64>,
        pub categories: Mutex<Vec<CategoryChoice>>,
        pub duplicates: Mutex<Vec<i64>>,
        pub covers: Mutex<usize>,
        pub known: Mutex<Vec<(i64, String, bool)>>,
        /// The ids `after_add` ran for.
        pub added: Mutex<Vec<i64>>,
    }

    impl FakeStore {
        pub fn calls(&self) -> Vec<String> {
            self.log.lock().expect("lock").clone()
        }

        fn note(&self, call: &'static str, text: String) -> Result<(), String> {
            self.log.lock().expect("lock").push(text);
            if self.fail.lock().expect("lock").contains(&call) { Err(format!("{call} failed")) } else { Ok(()) }
        }
    }

    impl FavoriteStore for FakeStore {
        fn local_entry(&self, source_id: i64, url: &str, title: &str, _cover: Option<&str>) -> Result<LocalEntry, String> {
            self.note("local_entry", format!("local {source_id} {url}"))?;
            let known = self.known.lock().expect("lock");
            Ok(match known.iter().find(|(s, u, _)| *s == source_id && u == url) {
                Some((_, _, favorite)) => LocalEntry { id: 7, favorite: *favorite, title: title.to_owned() },
                None => LocalEntry { id: 8, favorite: false, title: title.to_owned() },
            })
        }
        fn duplicates(&self, id: i64, _title: &str) -> Result<Vec<i64>, String> {
            self.note("duplicates", format!("duplicates {id}"))?;
            Ok(self.duplicates.lock().expect("lock").clone())
        }
        fn categories(&self) -> Result<Vec<CategoryChoice>, String> {
            Ok(self.categories.lock().expect("lock").clone())
        }
        fn default_category(&self) -> i64 {
            *self.default_category.lock().expect("lock")
        }
        fn write_favorite(&self, id: i64, write: FavoriteWrite) -> Result<(), String> {
            self.note("write_favorite", format!("favorite {id} {} {}", write.favorite, write.date_added))
        }
        fn set_categories(&self, id: i64, ids: &[i64]) -> Result<(), String> {
            self.note("set_categories", format!("categories {id} {ids:?}"))
        }
        fn clear_cover(&self, id: i64) -> usize {
            self.log.lock().expect("lock").push(format!("clear_cover {id}"));
            *self.covers.lock().expect("lock")
        }
        fn stamp_cover(&self, id: i64, now_ms: i64) -> Result<(), String> {
            self.note("stamp_cover", format!("stamp {id} {now_ms}"))
        }
        fn after_add(&self, id: i64) -> Result<(), String> {
            self.added.lock().expect("lock").push(id);
            Ok(())
        }
        fn favorite_urls(&self, _source_id: i64) -> watch::Receiver<BTreeSet<String>> {
            watch::channel(BTreeSet::new()).1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeStore;
    use super::*;

    #[test]
    fn a_default_category_adds_with_the_categories_before_the_favourite() {
        let store = FakeStore::default();
        *store.default_category.lock().expect("lock") = 0;
        assert_eq!(add_entry(&store, 7, 1_000), AddOutcome::Added { category_error: None });
        assert_eq!(store.calls(), ["categories 7 []", "favorite 7 true 1000"]);
    }

    #[test]
    fn an_ask_target_writes_nothing_until_the_chooser_answers() {
        let store = FakeStore::default();
        *store.default_category.lock().expect("lock") = -1;
        *store.categories.lock().expect("lock") = vec![CategoryChoice { id: 3, name: "Reading".into() }];
        assert_eq!(add_entry(&store, 7, 1_000), AddOutcome::Chooser);
        assert!(store.calls().is_empty());
        assert_eq!(add_entry_to(&store, 7, 1_000, vec![3]), AddOutcome::Added { category_error: None });
        assert_eq!(store.calls(), ["categories 7 [3]", "favorite 7 true 1000"]);
    }

    #[test]
    fn a_saved_category_id_resolves_without_asking() {
        let store = FakeStore::default();
        *store.default_category.lock().expect("lock") = 3;
        *store.categories.lock().expect("lock") = vec![CategoryChoice { id: 3, name: "Reading".into() }];
        assert_eq!(add_entry(&store, 7, 5), AddOutcome::Added { category_error: None });
        assert_eq!(store.calls()[0], "categories 7 [3]");
    }

    #[test]
    fn a_category_failure_does_not_block_the_favourite_and_a_favourite_failure_is_reported() {
        let store = FakeStore::default();
        store.fail.lock().expect("lock").push("set_categories");
        assert_eq!(add_entry(&store, 7, 1), AddOutcome::Added { category_error: Some("set_categories failed".into()) });
        assert!(store.calls().contains(&"favorite 7 true 1".to_owned()));
        store.fail.lock().expect("lock").push("write_favorite");
        assert_eq!(add_entry(&store, 7, 1), AddOutcome::Failed("write_favorite failed".into()));
    }

    #[test]
    fn removing_writes_only_the_favourite_columns_then_clears_covers_and_stamps_when_files_went() {
        let store = FakeStore::default();
        *store.covers.lock().expect("lock") = 2;
        remove_entry(&store, 7, 9_000).expect("removed");
        assert_eq!(store.calls(), ["favorite 7 false 0", "clear_cover 7", "stamp 7 9000"]);
    }

    #[test]
    fn no_deleted_cover_file_means_no_stamp_and_a_failed_write_stops_before_the_cache() {
        let store = FakeStore::default();
        remove_entry(&store, 7, 9_000).expect("removed");
        assert_eq!(store.calls(), ["favorite 7 false 0", "clear_cover 7"]);
        let failing = FakeStore::default();
        failing.fail.lock().expect("lock").push("write_favorite");
        assert!(remove_entry(&failing, 7, 1).is_err());
        assert_eq!(failing.calls(), ["favorite 7 false 0"], "the cover cache is untouched when the write fails");
    }

    #[test]
    fn the_add_side_effects_run_only_for_an_add_that_was_written() {
        let store = FakeStore::default();
        *store.default_category.lock().expect("lock") = -1;
        *store.categories.lock().expect("lock") = vec![CategoryChoice { id: 3, name: "Reading".into() }];
        assert_eq!(add_entry(&store, 7, 1), AddOutcome::Chooser);
        assert!(store.added.lock().expect("lock").is_empty(), "the chooser writes nothing, so nothing follows");
        add_entry_to(&store, 7, 1, vec![3]);
        assert_eq!(*store.added.lock().expect("lock"), [7], "the chosen add runs them once");
        store.fail.lock().expect("lock").push("write_favorite");
        add_entry_to(&store, 8, 1, vec![]);
        assert_eq!(*store.added.lock().expect("lock"), [7], "a failed favourite write binds and defaults nothing");
    }
}
