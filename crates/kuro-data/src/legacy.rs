//! The per-kind repositories of the old API: [`LegacyRepo`] (`MangaRepo`, `AnimeRepo`) over [`EntryRepo`] and
//! [`LegacyItemRepo`] (`ChapterRepo`, `EpisodeRepo`) over [`ItemRepo`]. Each turns the shared `Entry` or `Item` into the
//! legacy shape (`Manga`, `Chapter`, ...) and back. It carries no logic of its own; phase E
//! deletes it with the shapes. The kind-specific names (`set_chapter_flags_masked`, `seasons_by_parent`, ...) are in
//! `manga.rs` and `anime.rs`.

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_platform::kind::Kind;

use crate::entry::{Entry, EntryRepo, EntryUpdate, LibraryEntry};
use crate::item::{Item, ItemRepo, ItemUpdate};
use crate::{DataError, DataResult, LibraryState, LookupOrInsert};

/// A legacy entry shape: the conversions to and from the shared [`Entry`].
pub trait LegacyEntry: From<Entry> + Clone + PartialEq + Send + Sync + 'static {
    const KIND: Kind;
    /// The partial update of the shape (`MangaUpdate`).
    type Update;
    /// The library row of the shape (`LibraryManga`).
    type Library: From<LibraryEntry> + PartialEq + Send + Sync + 'static;

    fn to_entry(&self) -> Entry;
    fn to_update(u: &Self::Update) -> EntryUpdate;
}

fn lookup<T: LegacyEntry>(found: LookupOrInsert<Entry>) -> LookupOrInsert<T> {
    match found {
        LookupOrInsert::Found(e) => LookupOrInsert::Found(e.into()),
        LookupOrInsert::Inserted(id) => LookupOrInsert::Inserted(id),
    }
}

/// Typed access to the entries of one kind in the legacy shape. Shares one `Db` with the other repositories.
#[derive(Clone)]
pub struct LegacyRepo<T> {
    pub(crate) entries: EntryRepo,
    shape: PhantomData<fn() -> T>,
}

impl<T: LegacyEntry> LegacyRepo<T> {
    pub fn new(db: Arc<Db>) -> Self {
        Self { entries: EntryRepo::new(db), shape: PhantomData }
    }

    fn list(entries: Vec<Entry>) -> Vec<T> {
        entries.into_iter().map(T::from).collect()
    }

    /// `NotFound` when no row of this kind has `id`.
    pub fn by_id(&self, id: i64) -> DataResult<T> {
        let entry = self.entries.by_id(id)?;
        if entry.kind == T::KIND { Ok(entry.into()) } else { Err(DataError::NotFound) }
    }

    /// The first row (lowest `_id`) with this url and source.
    pub fn by_url_and_source(&self, url: &str, source: i64) -> DataResult<Option<T>> {
        Ok(self.entries.by_url_and_source(T::KIND, url, source)?.map(T::from))
    }

    pub fn favorites(&self) -> DataResult<Vec<T>> {
        Ok(Self::list(self.entries.favorites(T::KIND)?))
    }

    pub fn favorites_by_source(&self, source: i64) -> DataResult<Vec<T>> {
        Ok(Self::list(self.entries.favorites_by_source(T::KIND, source)?))
    }

    /// The row with `id`, published again after each change to it. `None` once the row is deleted.
    pub fn subscribe_by_id(&self, id: i64) -> Subscription<Option<T>> {
        self.entries.subscribe_by_id_as(id)
    }

    /// The first row (lowest `_id`) with this url and source, published again after each change.
    pub fn subscribe_by_url_and_source(&self, url: &str, source: i64) -> Subscription<Option<T>> {
        self.entries.subscribe_by_url_and_source_as(T::KIND, url, source)
    }

    pub fn subscribe_favorites_by_source(&self, source: i64) -> Subscription<Vec<T>> {
        self.entries.subscribe_favorites_by_source_as(T::KIND, source)
    }

    /// The `library()` rows, published again after a change to any table they read.
    pub fn subscribe_library(&self) -> Subscription<LibraryState<T::Library>> {
        self.entries.subscribe_library_as(T::KIND)
    }

    /// One row per (favourite entry, category link); an entry with no link appears once with category 0.
    pub fn library(&self) -> DataResult<Vec<T::Library>> {
        Ok(self.entries.library(T::KIND)?.into_iter().map(Into::into).collect())
    }

    /// Library entries other than `id` whose title equals `title`, both lowercased. Non-favourites never match.
    pub fn duplicates(&self, id: i64, title: &str) -> DataResult<Vec<T>> {
        Ok(Self::list(self.entries.duplicates(T::KIND, id, title)?))
    }

    /// Favourites whose `next_update` is at or after local midnight of `now_ms`'s day, soonest first. The statuses are
    /// the kind's (`Kind::upcoming_statuses`); the argument is the caller's old list and is not read.
    pub fn upcoming(&self, _statuses: &[i64], now_ms: i64) -> DataResult<Vec<T>> {
        Ok(Self::list(self.entries.upcoming(T::KIND, now_ms)?))
    }

    /// Favourites due from local midnight today, soonest first, published again after a change and at each local
    /// midnight.
    pub fn subscribe_upcoming(&self, _statuses: &[i64]) -> Subscription<Vec<T>> {
        self.subscribe_upcoming_with_clock(&[], || crate::time::now_millis())
    }

    /// `subscribe_upcoming` with the clock injected, as epoch milliseconds.
    pub fn subscribe_upcoming_with_clock(&self, _statuses: &[i64], now: impl Fn() -> i64 + Send + Sync + 'static) -> Subscription<Vec<T>> {
        self.entries.subscribe_upcoming_as(T::KIND, Arc::new(now))
    }

    /// The excluded scanlators of an entry.
    pub fn excluded_scanlators(&self, id: i64) -> DataResult<BTreeSet<String>> {
        self.entries.excluded_scanlators(id)
    }

    /// Makes the excluded set of an entry equal to `wanted`, inserts and deletes in one transaction.
    pub fn set_excluded_scanlators(&self, id: i64, wanted: &BTreeSet<String>) -> DataResult<()> {
        self.entries.set_excluded_scanlators(id, wanted)
    }

    pub fn subscribe_excluded_scanlators(&self, id: i64) -> Subscription<BTreeSet<String>> {
        self.entries.subscribe_excluded_scanlators(id)
    }

    /// The distinct, non-blank scanlators of an entry's items.
    pub fn available_scanlators(&self, id: i64) -> DataResult<BTreeSet<String>> {
        self.entries.available_scanlators(id)
    }

    pub fn subscribe_available_scanlators(&self, id: i64) -> Subscription<BTreeSet<String>> {
        self.entries.subscribe_available_scanlators(id)
    }

    /// Writes `value` under `mask` into the viewer flags of one entry, in one statement. `Ok(false)` when it does not
    /// exist.
    pub fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool> {
        self.entries.set_viewer_bits(id, value, mask)
    }

    /// Sets `viewer = 0` on every entry of the kind.
    pub fn reset_viewer_flags(&self) -> DataResult<()> {
        self.entries.reset_viewer_flags(T::KIND)
    }

    /// Replaces the category links of one entry in one transaction.
    pub fn set_categories(&self, id: i64, category_ids: &[i64]) -> DataResult<()> {
        self.entries.set_categories(id, category_ids)
    }

    /// Inserts `m` and returns the new id.
    pub fn insert(&self, m: &T) -> DataResult<i64> {
        self.entries.insert(&m.to_entry())
    }

    /// Looks up `(url, source)` and inserts `m` when no row has it, in one writer transaction. Callers that race on the
    /// same key leave one row and get the same id.
    pub fn lookup_or_insert(&self, m: &T) -> DataResult<LookupOrInsert<T>> {
        Ok(lookup(self.entries.lookup_or_insert(&m.to_entry())?))
    }

    /// Coalesce update of one row. A missing id is not an error.
    pub fn update(&self, u: &T::Update) -> DataResult<()> {
        self.entries.update(&T::to_update(u))
    }

    /// Every update in one transaction: one failure rolls back all of them.
    pub fn update_all(&self, updates: &[T::Update]) -> DataResult<()> {
        self.entries.update_all(&updates.iter().map(T::to_update).collect::<Vec<_>>())
    }

    /// `(source, url)` of every entry of the kind, library or not.
    pub fn all_source_and_url(&self) -> DataResult<Vec<(i64, String)>> {
        self.entries.all_source_and_url(T::KIND)
    }

    /// `(source, favourite count)` for every source with at least one favourite.
    pub fn favorite_counts_by_source(&self) -> DataResult<Vec<(i64, i64)>> {
        self.entries.favorite_counts_by_source(T::KIND)
    }

    /// `(source, non-library count)` for every source with at least one non-favourite.
    pub fn non_library_counts_by_source(&self) -> DataResult<Vec<(i64, i64)>> {
        self.entries.non_library_counts_by_source(T::KIND)
    }

    pub fn subscribe_favorite_counts(&self) -> Subscription<Vec<(i64, i64)>> {
        self.entries.subscribe_favorite_counts(T::KIND)
    }

    pub fn subscribe_non_library_counts(&self) -> Subscription<Vec<(i64, i64)>> {
        self.entries.subscribe_non_library_counts(T::KIND)
    }

    /// Clear database: deletes the non-library entries of `source_ids`, then the history rows left reset, in one
    /// transaction. Library entries are untouched. An empty list changes nothing and returns 0. Returns the number of
    /// history rows removed.
    pub fn clear_non_library_by_sources(&self, source_ids: &[i64]) -> DataResult<usize> {
        self.entries.clear_non_library(T::KIND, source_ids)
    }

    /// As [`Self::clear_non_library_by_sources`], without the count.
    pub fn delete_non_library_by_sources(&self, source_ids: &[i64]) -> DataResult<()> {
        self.clear_non_library_by_sources(source_ids).map(drop)
    }
}

/// A legacy item shape: the conversions to and from the shared [`Item`].
pub trait LegacyItem: From<Item> + Clone + PartialEq + Send + Sync + 'static {
    const KIND: Kind;
    /// The partial update of the shape (`ChapterUpdate`).
    type Update;

    fn to_item(&self) -> Item;
    fn to_update(u: &Self::Update) -> ItemUpdate;
}

/// Typed access to the items of one kind in the legacy shape. Phase E deletes it with the shapes.
#[derive(Clone)]
pub struct LegacyItemRepo<T> {
    pub(crate) items: ItemRepo,
    shape: PhantomData<fn() -> T>,
}

impl<T: LegacyItem> LegacyItemRepo<T> {
    pub fn new(db: Arc<Db>) -> Self {
        Self { items: ItemRepo::new(db), shape: PhantomData }
    }

    pub(crate) fn list(items: Vec<Item>) -> Vec<T> {
        items.into_iter().map(T::from).collect()
    }

    /// The row with `id`, or `None` when no row has it.
    pub fn by_id(&self, id: i64) -> DataResult<Option<T>> {
        Ok(self.items.by_id(id)?.filter(|i| i.kind == T::KIND).map(T::from))
    }

    /// Every item of the kind with this url, in any entry. Empty when none matches.
    pub fn by_url(&self, url: &str) -> DataResult<Vec<T>> {
        Ok(Self::list(self.items.by_url(T::KIND, url)?))
    }

    /// The bookmarked items of an entry.
    pub fn bookmarked(&self, entry_id: i64) -> DataResult<Vec<T>> {
        Ok(Self::list(self.items.bookmarked(entry_id)?))
    }

    /// Inserts the items in one transaction, in order. Returns them with their new ids, in the same order. A failure on
    /// any row leaves the table unchanged.
    pub fn add_all(&self, new: &[T]) -> DataResult<Vec<T>> {
        Ok(Self::list(self.items.add_all(&new.iter().map(T::to_item).collect::<Vec<_>>())?))
    }

    /// Deletes the items with these ids, in chunks, in one transaction.
    pub fn remove_with_ids(&self, ids: &[i64]) -> DataResult<()> {
        self.items.remove_with_ids(ids)
    }

    /// Updates one item. A missing id matches no row and is not an error.
    pub fn update(&self, u: &T::Update) -> DataResult<()> {
        self.items.update(&T::to_update(u))
    }

    /// Every update in one transaction: one failure rolls back all of them.
    pub fn update_all(&self, updates: &[T::Update]) -> DataResult<()> {
        self.items.update_all(&updates.iter().map(T::to_update).collect::<Vec<_>>())
    }
}
