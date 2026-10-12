//! Add to library from a History row (T-42a-019) and the migrate hand-off (T-42a-020). The decisions are pure over the
//! browse [`FavoriteStore`], so the Browse add path and this one share the category rules (`add_entry`).
//!
//! Order of an add: the duplicate check first (a duplicate asks, nothing is written), then the default category: `-1`
//! with categories opens the chooser, `0` or no categories adds with no category, a user category adds to it.

use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::category::CategoryRepo;
use kuro_data::manga::MangaRepo;
use kuro_platform::kind::Kind;
use kuro_prefs::library::LibraryPrefs;

use crate::browse::entries::{CategoryChoice, FavoriteStore, add_entry, add_entry_to, chooser_rows};
use crate::browse::favorite::AddOutcome;
use crate::updates::track_binding;

/// Where the first step of an add lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Library duplicates of the entry exist: the duplicate dialog asks. The ids are the existing rows.
    Duplicate(Vec<i64>),
    /// The default category asks: the chooser opens and nothing is written yet.
    Chooser,
    /// The entry is in the library now.
    Added,
}

/// The first step of an add from a history row: duplicate check, then the default category.
pub fn start(store: &dyn FavoriteStore, id: i64, title: &str, now_ms: i64) -> Result<Step, String> {
    let duplicates = store.duplicates(id, title)?;
    if !duplicates.is_empty() {
        return Ok(Step::Duplicate(duplicates));
    }
    add_now(store, id, now_ms)
}

/// "Add anyway" of the duplicate dialog: the default category rules without the duplicate check.
pub fn add_anyway(store: &dyn FavoriteStore, id: i64, now_ms: i64) -> Result<Step, String> {
    add_now(store, id, now_ms)
}

fn add_now(store: &dyn FavoriteStore, id: i64, now_ms: i64) -> Result<Step, String> {
    match add_entry(store, id, now_ms) {
        AddOutcome::Chooser => Ok(Step::Chooser),
        AddOutcome::Added { .. } => Ok(Step::Added),
        AddOutcome::Failed(error) => Err(error),
    }
}

/// The chooser rows: the user's categories, each checked when the entry is linked to it now.
pub fn chooser_options(store: &dyn FavoriteStore, id: i64) -> Result<Vec<(CategoryChoice, bool)>, String> {
    let categories = store.categories()?;
    let linked = store.entry_categories(id)?;
    Ok(chooser_rows(&categories, &linked))
}

/// The chooser's OK: the picked categories are written, then the favourite (the Browse order).
pub fn confirm_chooser(store: &dyn FavoriteStore, id: i64, now_ms: i64, picked: Vec<i64>) -> Result<(), String> {
    match add_entry_to(store, id, now_ms, picked) {
        AddOutcome::Failed(error) => Err(error),
        _ => Ok(()),
    }
}

/// The production store of one kind over the app databases. An add also applies the default episode flags (anime) and
/// binds the enhanced trackers through [`track_binding`] (EC-42-H-14); the confirm path reaches both through the same
/// `after_add`.
pub fn store(kind: Kind) -> Arc<dyn FavoriteStore> {
    let app = crate::data::app();
    let db = app.db(kind);
    let library = LibraryPrefs::new(app.base.store().clone());
    match kind {
        Kind::Manga => {
            let manga = MangaRepo::new(db.clone());
            let inner = Arc::new(crate::browse::repo_store::RepoStore::new(
                manga.clone(),
                CategoryRepo::new(db, Kind::Manga),
                library.default_manga_category.clone(),
                Arc::new(|_| 0),
            ));
            let lookup: track_binding::EntryLookup = Arc::new(move |id| {
                manga.by_id(id).map(|m| (m.title, m.source)).map_err(|e| e.to_string())
            });
            Arc::new(track_binding::BindingStore::new(inner, lookup, track_binding::binder(kind)))
        }
        Kind::Anime => Arc::new(
            crate::browse::repo_store::AnimeStore::new(
                AnimeRepo::new(db.clone()),
                CategoryRepo::new(db, Kind::Anime),
                library.default_anime_category.clone(),
                Arc::new(|_| 0),
                Arc::new(|_| 0),
            )
            .with_add_effects(app.base.store().clone(), track_binding::binder(kind)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::entries::fake::FakeStore;

    fn category(id: i64) -> CategoryChoice {
        CategoryChoice { id, name: format!("c{id}") }
    }

    #[test]
    fn a_duplicate_asks_and_writes_nothing() {
        let store = FakeStore::default();
        *store.duplicates.lock().expect("lock") = vec![5];
        assert_eq!(start(&store, 3, "T", 100), Ok(Step::Duplicate(vec![5])));
        assert!(store.calls().iter().all(|c| !c.starts_with("favorite")), "no favourite write before the answer");
    }

    #[test]
    fn default_minus_one_with_categories_opens_the_chooser() {
        let store = FakeStore::default();
        *store.default_category.lock().expect("lock") = -1;
        *store.categories.lock().expect("lock") = vec![category(1), category(2)];
        assert_eq!(start(&store, 3, "T", 100), Ok(Step::Chooser));
        assert!(store.calls().iter().all(|c| !c.starts_with("favorite")), "the chooser writes nothing yet");
    }

    #[test]
    fn default_zero_adds_without_a_category() {
        let store = FakeStore::default();
        *store.default_category.lock().expect("lock") = 0;
        *store.categories.lock().expect("lock") = vec![category(1)];
        assert_eq!(start(&store, 3, "T", 100), Ok(Step::Added));
        assert!(store.calls().contains(&"categories 3 []".to_owned()));
        assert!(store.calls().contains(&"favorite 3 true 100".to_owned()));
    }

    #[test]
    fn a_user_default_category_adds_to_it() {
        let store = FakeStore::default();
        *store.default_category.lock().expect("lock") = 2;
        *store.categories.lock().expect("lock") = vec![category(1), category(2)];
        assert_eq!(start(&store, 3, "T", 100), Ok(Step::Added));
        assert!(store.calls().contains(&"categories 3 [2]".to_owned()));
    }

    #[test]
    fn add_anyway_skips_the_duplicate_check() {
        let store = FakeStore::default();
        *store.duplicates.lock().expect("lock") = vec![5];
        assert_eq!(add_anyway(&store, 3, 100), Ok(Step::Added));
    }

    #[test]
    fn chooser_is_pre_checked_from_the_current_links() {
        let store = FakeStore::default();
        *store.categories.lock().expect("lock") = vec![category(1), category(2), category(0)];
        *store.links.lock().expect("lock") = vec![2];
        let rows = chooser_options(&store, 3).expect("rows");
        let checked: Vec<(i64, bool)> = rows.iter().map(|(c, on)| (c.id, *on)).collect();
        assert_eq!(checked, vec![(1, false), (2, true)], "the system category is never offered");
    }

    #[test]
    fn confirm_writes_categories_then_the_favourite() {
        let store = FakeStore::default();
        confirm_chooser(&store, 3, 100, vec![1]).expect("confirm");
        let calls = store.calls();
        let at = |prefix: &str| calls.iter().position(|c| c.starts_with(prefix)).expect(prefix);
        assert!(at("categories 3 [1]") < at("favorite 3 true 100"));
    }

}
