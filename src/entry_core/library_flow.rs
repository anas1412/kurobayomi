//! The library button of an entry page, one flow for both kinds (behaviour 4.2 and 4.3, `toggleFavorite`): an entry in the
//! library is removed (the cover files go, and the page asks about the downloads); one that is not is checked for a
//! same-named library entry, routed by the default category, and added with its categories and the library defaults in
//! one transaction, after which the enhanced trackers bind. The manga runner and the anime runner run these on the
//! blocking pool and turn the outcome into their own page events.

use kuro_data::DataResult;
use kuro_data::category::{Category, CategoryRepo};
use kuro_data::entry::{Entry, EntryRepo};
use kuro_prefs::store::PrefStore;

use crate::entry_core::entry_ops;
use crate::entry_core::favorite::{AddRoute, route_add};

/// What the page's store offers the flow. Each store answers over the shared `entry_ops`.
pub trait LibraryStore {
    /// Whether the entry is in the library, or `None` when the row is gone.
    fn favorite(&self, id: i64) -> DataResult<Option<bool>>;
    /// The library entries with the same title (E-FAV-1).
    fn duplicates(&self, id: i64) -> DataResult<Vec<Entry>>;
    /// The user's categories, without the default row.
    fn user_categories(&self) -> DataResult<Vec<Category>>;
    /// Every category (the default row included) and the ids the entry is linked to.
    fn categories(&self, id: i64) -> DataResult<(Vec<(i64, String)>, Vec<i64>)>;
    /// Links the categories, writes the favorite and the library defaults in one transaction. `false` when the row is gone.
    fn add(&self, id: i64, category_ids: &[i64], now_ms: i64) -> DataResult<bool>;
    /// Writes `favorite = false` and `date_added = 0`. `false` when the row is gone.
    fn remove(&self, id: i64, now_ms: i64) -> DataResult<bool>;
}

/// The repositories the flow reads and writes, for an entry of either kind: the `entry_ops` over them are the same.
pub struct LibraryRepos {
    entries: EntryRepo,
    categories: CategoryRepo,
    prefs: PrefStore,
}

impl LibraryRepos {
    pub fn new(entries: EntryRepo, categories: CategoryRepo, prefs: PrefStore) -> Self {
        Self { entries, categories, prefs }
    }
}

impl LibraryStore for LibraryRepos {
    fn favorite(&self, id: i64) -> DataResult<Option<bool>> {
        Ok(entry_ops::entry_or_none(&self.entries, id)?.map(|entry| entry.favorite))
    }

    fn duplicates(&self, id: i64) -> DataResult<Vec<Entry>> {
        entry_ops::duplicates(&self.entries, id)
    }

    fn user_categories(&self) -> DataResult<Vec<Category>> {
        entry_ops::user_categories(&self.categories)
    }

    fn categories(&self, id: i64) -> DataResult<(Vec<(i64, String)>, Vec<i64>)> {
        entry_ops::categories(&self.categories, id)
    }

    fn add(&self, id: i64, category_ids: &[i64], now_ms: i64) -> DataResult<bool> {
        entry_ops::add_to_library(&self.entries, &self.prefs, id, category_ids, now_ms)
    }

    fn remove(&self, id: i64, now_ms: i64) -> DataResult<bool> {
        entry_ops::set_favorite(&self.entries, id, false, now_ms)
    }
}

/// What the backend does outside the database.
pub struct Hooks<'a> {
    /// Deletes the cover files of an entry that just left the library (`removeCovers`); `true` when a file was removed.
    pub remove_cover: &'a dyn Fn() -> bool,
    /// How many downloaded items the entry has.
    pub downloaded_count: &'a dyn Fn() -> usize,
    /// Binds the enhanced trackers of an entry that joined the library (`bindEnhancedTrackers`).
    pub bind_enhanced: &'a dyn Fn(),
}

/// What a successful remove still has to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoveOutcome {
    /// The cover file was deleted.
    pub cover_removed: bool,
    /// Downloads exist, so the delete prompt runs. Dismissing it keeps them.
    pub ask_delete_downloads: bool,
}

/// What a press of the library button did.
#[derive(Clone, Debug, PartialEq)]
pub enum Toggled {
    /// The row is gone, or it joined the library meanwhile: nothing to do.
    Nothing,
    Removed(RemoveOutcome),
    /// A same-named entry is in the library; nothing is written until the page answers.
    Duplicate(Vec<Entry>),
    /// No default category: the page asks which. Favorite is not written yet (E-FAV-3).
    ChooseCategory { all: Vec<(i64, String)>, linked: Vec<i64> },
    Added,
}

/// The library button. `default_category` is the library's default (`-1` unset, `0` uncategorized, else a category id).
/// With `check_duplicates` false (Add anyway) the duplicate check is skipped.
pub fn toggle(
    store: &dyn LibraryStore,
    hooks: &Hooks<'_>,
    id: i64,
    default_category: i64,
    check_duplicates: bool,
    now_ms: i64,
) -> DataResult<Toggled> {
    match store.favorite(id)? {
        None => Ok(Toggled::Nothing),
        Some(true) => {
            // a failed favorite write does nothing: no cover is deleted and no prompt shows (E-LIB-9)
            if !store.remove(id, now_ms)? {
                return Ok(Toggled::Nothing);
            }
            let cover_removed = (hooks.remove_cover)();
            Ok(Toggled::Removed(RemoveOutcome { cover_removed, ask_delete_downloads: (hooks.downloaded_count)() > 0 }))
        }
        Some(false) => {
            if check_duplicates {
                let existing = store.duplicates(id)?;
                if !existing.is_empty() {
                    return Ok(Toggled::Duplicate(existing));
                }
            }
            match route_add(default_category, &store.user_categories()?) {
                AddRoute::AddDefaultCategory(category) => {
                    let added = add(store, hooks, id, &category.into_iter().collect::<Vec<_>>(), now_ms)?;
                    Ok(if added { Toggled::Added } else { Toggled::Nothing })
                }
                AddRoute::ChooseCategory => {
                    let (all, linked) = store.categories(id)?;
                    Ok(Toggled::ChooseCategory { all, linked })
                }
            }
        }
    }
}

/// Adds the entry in these categories, then binds the enhanced trackers. Every add branch ends here: the default category,
/// no categories, and the category dialog's confirm. `false` when the row is gone.
pub fn add(store: &dyn LibraryStore, hooks: &Hooks<'_>, id: i64, category_ids: &[i64], now_ms: i64) -> DataResult<bool> {
    let added = store.add(id, category_ids, now_ms)?;
    if added {
        (hooks.bind_enhanced)();
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    #[derive(Default)]
    struct Fake {
        favorite: Cell<Option<bool>>,
        duplicates: Vec<Entry>,
        user_categories: Vec<Category>,
        added: RefCell<Vec<(i64, Vec<i64>)>>,
        removed: Cell<u32>,
        fail_remove: bool,
    }

    impl LibraryStore for Fake {
        fn favorite(&self, _: i64) -> DataResult<Option<bool>> {
            Ok(self.favorite.get())
        }
        fn duplicates(&self, _: i64) -> DataResult<Vec<Entry>> {
            Ok(self.duplicates.clone())
        }
        fn user_categories(&self) -> DataResult<Vec<Category>> {
            Ok(self.user_categories.clone())
        }
        fn categories(&self, _: i64) -> DataResult<(Vec<(i64, String)>, Vec<i64>)> {
            Ok((vec![(1, "A".into())], vec![]))
        }
        fn add(&self, id: i64, category_ids: &[i64], _: i64) -> DataResult<bool> {
            self.added.borrow_mut().push((id, category_ids.to_vec()));
            Ok(true)
        }
        fn remove(&self, _: i64, _: i64) -> DataResult<bool> {
            self.removed.set(self.removed.get() + 1);
            Ok(!self.fail_remove)
        }
    }

    fn category(id: i64) -> Category {
        Category { id, name: format!("c{id}"), order: id, flags: 0, hidden: false }
    }

    fn entry(id: i64) -> Entry {
        Entry { id, ..Entry::create(kuro_platform::kind::Kind::Manga) }
    }

    struct Counts {
        cover: Cell<u32>,
        bound: Cell<u32>,
        downloads: usize,
        cover_changed: bool,
    }

    impl Counts {
        fn new(downloads: usize, cover_changed: bool) -> Self {
            Self { cover: Cell::new(0), bound: Cell::new(0), downloads, cover_changed }
        }
    }

    fn run<T>(counts: &Counts, f: impl FnOnce(&Hooks<'_>) -> T) -> T {
        let remove_cover = || {
            counts.cover.set(counts.cover.get() + 1);
            counts.cover_changed
        };
        let downloaded_count = || counts.downloads;
        let bind = || counts.bound.set(counts.bound.get() + 1);
        f(&Hooks { remove_cover: &remove_cover, downloaded_count: &downloaded_count, bind_enhanced: &bind })
    }

    #[test]
    fn removing_deletes_the_cover_after_the_write_and_asks_only_when_downloads_exist() {
        let store = Fake { favorite: Cell::new(Some(true)), ..Fake::default() };
        let counts = Counts::new(2, true);
        let out = run(&counts, |h| toggle(&store, h, 1, 0, true, 5)).unwrap();
        assert_eq!(out, Toggled::Removed(RemoveOutcome { cover_removed: true, ask_delete_downloads: true }));
        assert_eq!((store.removed.get(), counts.cover.get()), (1, 1));
        let counts = Counts::new(0, false);
        let out = run(&counts, |h| toggle(&store, h, 1, 0, true, 5)).unwrap();
        assert_eq!(out, Toggled::Removed(RemoveOutcome { cover_removed: false, ask_delete_downloads: false }));
    }

    #[test]
    fn a_failed_favorite_write_deletes_no_cover_and_asks_nothing() {
        let store = Fake { favorite: Cell::new(Some(true)), fail_remove: true, ..Fake::default() };
        let counts = Counts::new(3, true);
        assert_eq!(run(&counts, |h| toggle(&store, h, 1, 0, true, 5)).unwrap(), Toggled::Nothing);
        assert_eq!(counts.cover.get(), 0);
    }

    #[test]
    fn a_duplicate_stops_before_any_write_and_add_anyway_skips_the_check() {
        let store = Fake { favorite: Cell::new(Some(false)), duplicates: vec![entry(9)], ..Fake::default() };
        let counts = Counts::new(0, false);
        assert_eq!(run(&counts, |h| toggle(&store, h, 1, 0, true, 5)).unwrap(), Toggled::Duplicate(vec![entry(9)]));
        assert!(store.added.borrow().is_empty());
        assert_eq!(run(&counts, |h| toggle(&store, h, 1, 0, false, 5)).unwrap(), Toggled::Added);
        assert_eq!(counts.bound.get(), 1);
    }

    #[test]
    fn the_default_category_adds_at_once_and_binds_and_an_unset_default_asks_first() {
        let store = Fake { favorite: Cell::new(Some(false)), user_categories: vec![category(4)], ..Fake::default() };
        let counts = Counts::new(0, false);
        assert_eq!(run(&counts, |h| toggle(&store, h, 1, 4, true, 5)).unwrap(), Toggled::Added);
        assert_eq!(*store.added.borrow(), vec![(1, vec![4])]);
        assert_eq!(counts.bound.get(), 1);
        assert_eq!(
            run(&counts, |h| toggle(&store, h, 1, -1, true, 5)).unwrap(),
            Toggled::ChooseCategory { all: vec![(1, "A".into())], linked: vec![] }
        );
        assert_eq!(store.added.borrow().len(), 1, "nothing is written until the dialog answers");
        assert!(run(&counts, |h| add(&store, h, 1, &[4], 5)).unwrap());
        assert_eq!(counts.bound.get(), 2, "the dialog's confirm binds too");
    }

    #[test]
    fn uncategorized_adds_without_a_category_and_a_missing_row_does_nothing() {
        let store = Fake { favorite: Cell::new(Some(false)), user_categories: vec![category(4)], ..Fake::default() };
        let counts = Counts::new(0, false);
        assert_eq!(run(&counts, |h| toggle(&store, h, 1, 0, true, 5)).unwrap(), Toggled::Added);
        assert_eq!(*store.added.borrow(), vec![(1, vec![])]);
        store.favorite.set(None);
        assert_eq!(run(&counts, |h| toggle(&store, h, 1, 0, true, 5)).unwrap(), Toggled::Nothing);
    }
}
