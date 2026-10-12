//! Manga entries in the legacy shape: `Manga`, `MangaUpdate`, `LibraryManga` and `MangaRepo`
//! (MangaRepositoryImpl.kt, MangaMapper.kt) over the shared `Entry` and `EntryRepo`. `chapter_flags` is the manga
//! layout; the conversions translate it to and from the unified `item_flags` (`kuro_db::flags`). Phase E deletes this
//! file.

use std::collections::BTreeMap;

use kuro_db::codecs::UpdateStrategy;
use kuro_db::flags::{manga_item_flags_from_unified as from_unified, manga_item_flags_to_unified as to_unified};
use kuro_db::statements_manga::MangaRow;
use kuro_platform::kind::Kind;

use crate::DataResult;
use crate::entry::{Entry, EntryUpdate, LibraryEntry};
use crate::legacy::{LegacyEntry, LegacyRepo};

/// Upcoming-tab statuses for manga: ONGOING (1) and PUBLISHING_FINISHED (4), as `GetUpcomingManga` passes them.
pub const UPCOMING_STATUSES: [i64; 2] = {
    let statuses = Kind::Manga.upcoming_statuses();
    [statuses[0], statuses[1]]
};

/// One `mangas` row as the domain sees it (`Manga.kt`). `is_syncing` is not exposed.
#[derive(Clone, Debug, PartialEq)]
pub struct Manga {
    pub id: i64,
    pub source: i64,
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: i64,
    pub thumbnail_url: Option<String>,
    pub favorite: bool,
    pub last_update: i64,
    pub next_update: i64,
    pub fetch_interval: i32,
    pub date_added: i64,
    pub viewer_flags: i64,
    /// The manga layout (`kuro_db::flags::chapter`); the entry stores it as the unified `item_flags`.
    pub chapter_flags: i64,
    pub cover_last_modified: i64,
    pub update_strategy: UpdateStrategy,
    pub initialized: bool,
    pub last_modified_at: i64,
    pub favorite_modified_at: Option<i64>,
    pub version: i64,
}

impl Manga {
    /// `Manga.create()`: id -1, source -1, version 0, `ALWAYS_UPDATE`, every other default zero or empty.
    pub fn create() -> Self {
        Self::from(Entry::create(Kind::Manga))
    }
}

impl From<Entry> for Manga {
    fn from(e: Entry) -> Self {
        Self {
            id: e.id,
            source: e.source,
            url: e.url,
            title: e.title,
            artist: e.artist,
            author: e.author,
            description: e.description,
            genre: e.genre,
            status: e.status,
            thumbnail_url: e.thumbnail_url,
            favorite: e.favorite,
            last_update: e.last_update,
            next_update: e.next_update,
            fetch_interval: e.fetch_interval,
            date_added: e.date_added,
            viewer_flags: e.viewer_flags,
            chapter_flags: from_unified(e.item_flags),
            cover_last_modified: e.cover_last_modified,
            update_strategy: e.update_strategy,
            initialized: e.initialized,
            last_modified_at: e.last_modified_at,
            favorite_modified_at: e.favorite_modified_at,
            version: e.version,
        }
    }
}

impl From<&Manga> for Entry {
    fn from(m: &Manga) -> Self {
        Self {
            id: m.id,
            source: m.source,
            url: m.url.clone(),
            title: m.title.clone(),
            artist: m.artist.clone(),
            author: m.author.clone(),
            description: m.description.clone(),
            genre: m.genre.clone(),
            status: m.status,
            thumbnail_url: m.thumbnail_url.clone(),
            favorite: m.favorite,
            last_update: m.last_update,
            next_update: m.next_update,
            fetch_interval: m.fetch_interval,
            date_added: m.date_added,
            viewer_flags: m.viewer_flags,
            item_flags: to_unified(m.chapter_flags),
            cover_last_modified: m.cover_last_modified,
            update_strategy: m.update_strategy,
            initialized: m.initialized,
            last_modified_at: m.last_modified_at,
            favorite_modified_at: m.favorite_modified_at,
            version: m.version,
            ..Entry::create(Kind::Manga)
        }
    }
}

/// Partial update (`MangaUpdate.kt`). `None` keeps the stored value, so a nullable column cannot be
/// cleared through this type. `id` is required: use [`MangaUpdate::new`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MangaUpdate {
    pub id: i64,
    pub source: Option<i64>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub favorite: Option<bool>,
    pub last_update: Option<i64>,
    pub next_update: Option<i64>,
    pub fetch_interval: Option<i32>,
    pub date_added: Option<i64>,
    pub viewer_flags: Option<i64>,
    pub chapter_flags: Option<i64>,
    pub cover_last_modified: Option<i64>,
    pub update_strategy: Option<UpdateStrategy>,
    pub initialized: Option<bool>,
    pub version: Option<i64>,
}

impl MangaUpdate {
    pub fn new(id: i64) -> Self {
        Self { id, ..Default::default() }
    }
}

impl From<&MangaUpdate> for EntryUpdate {
    fn from(u: &MangaUpdate) -> Self {
        Self {
            id: u.id,
            source: u.source,
            url: u.url.clone(),
            title: u.title.clone(),
            artist: u.artist.clone(),
            author: u.author.clone(),
            description: u.description.clone(),
            genre: u.genre.clone(),
            status: u.status,
            thumbnail_url: u.thumbnail_url.clone(),
            favorite: u.favorite,
            last_update: u.last_update,
            next_update: u.next_update,
            fetch_interval: u.fetch_interval,
            date_added: u.date_added,
            viewer_flags: u.viewer_flags,
            item_flags: u.chapter_flags.map(to_unified),
            cover_last_modified: u.cover_last_modified,
            update_strategy: u.update_strategy,
            initialized: u.initialized,
            version: u.version,
            ..Default::default()
        }
    }
}

/// One `libraryView` row (`LibraryManga.kt`): the manga, its category link and the aggregates.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryManga {
    pub manga: Manga,
    pub category: i64,
    pub total_chapters: i64,
    pub read_count: i64,
    pub bookmark_count: i64,
    pub latest_upload: i64,
    pub chapter_fetched_at: i64,
    pub last_read: i64,
}

impl LibraryManga {
    pub fn id(&self) -> i64 {
        self.manga.id
    }

    pub fn unread_count(&self) -> i64 {
        self.total_chapters - self.read_count
    }

    pub fn has_bookmarks(&self) -> bool {
        self.bookmark_count > 0
    }

    pub fn has_started(&self) -> bool {
        self.read_count > 0
    }
}

impl From<LibraryEntry> for LibraryManga {
    fn from(r: LibraryEntry) -> Self {
        Self {
            manga: r.entry.into(),
            category: r.category,
            total_chapters: r.total,
            read_count: r.done,
            bookmark_count: r.bookmarks,
            latest_upload: r.latest_upload,
            chapter_fetched_at: r.item_fetched_at,
            last_read: r.last_activity,
        }
    }
}

impl LegacyEntry for Manga {
    const KIND: Kind = Kind::Manga;
    type Update = MangaUpdate;
    type Library = LibraryManga;

    fn to_entry(&self) -> Entry {
        self.into()
    }

    fn to_update(u: &MangaUpdate) -> EntryUpdate {
        u.into()
    }
}

/// `MangaMapper.mapManga` over a `statements_manga` row (already in the manga layout): NULL `last_update` and
/// `next_update` read as 0; `calculate_interval` narrows to `i32`; `is_syncing` is dropped.
pub fn map_manga(r: &MangaRow) -> Manga {
    Manga {
        id: r.id,
        source: r.source,
        url: r.url.clone(),
        title: r.title.clone(),
        artist: r.artist.clone(),
        author: r.author.clone(),
        description: r.description.clone(),
        genre: r.genre.as_ref().map(|g| g.0.clone()),
        status: r.status,
        thumbnail_url: r.thumbnail_url.clone(),
        favorite: r.favorite,
        last_update: r.last_update.unwrap_or(0),
        next_update: r.next_update.unwrap_or(0),
        fetch_interval: r.calculate_interval as i32,
        date_added: r.date_added,
        viewer_flags: r.viewer,
        chapter_flags: r.chapter_flags,
        cover_last_modified: r.cover_last_modified,
        update_strategy: r.update_strategy,
        initialized: r.initialized,
        last_modified_at: r.last_modified_at,
        favorite_modified_at: r.favorite_modified_at,
        version: r.version,
    }
}

/// Groups library rows by category id, keeping each group in its input order.
pub fn library_by_category(rows: Vec<LibraryManga>) -> BTreeMap<i64, Vec<LibraryManga>> {
    let mut grouped: BTreeMap<i64, Vec<LibraryManga>> = BTreeMap::new();
    for row in rows {
        grouped.entry(row.category).or_default().push(row);
    }
    grouped
}

/// Typed access to the manga rows of `entries`.
pub type MangaRepo = LegacyRepo<Manga>;

impl MangaRepo {
    /// Non-favourites with a chapter that is read or has progress, and nothing else.
    pub fn read_not_in_library(&self) -> DataResult<Vec<Manga>> {
        Ok(self.entries.watched_not_in_library(Kind::Manga)?.into_iter().map(Manga::from).collect())
    }

    /// Sets the `mask` bits of `chapter_flags` (the manga layout) to those of `flag` in one UPDATE. Returns whether the
    /// row changed.
    pub fn set_chapter_flags_masked(&self, manga_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.entries.set_item_flags_masked(manga_id, to_unified(flag), to_unified(mask))
    }

    /// Reads `chapter_flags` (the manga layout), applies `change` and stores the result in the writer transaction.
    pub fn update_chapter_flags(&self, manga_id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        self.entries.update_item_flags(manga_id, |word| to_unified(change(from_unified(word))))
    }
}

#[cfg(test)]
mod excluded_scanlator_tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use std::time::Duration;

    use kuro_db::statements_manga::{ChapterRow, chapters};

    use super::*;
    use crate::tests::test_db;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn chapter(manga_id: i64, url: &str, scanlator: Option<&str>) -> ChapterRow {
        ChapterRow {
            id: -1,
            manga_id,
            url: url.to_owned(),
            name: url.to_owned(),
            scanlator: scanlator.map(str::to_owned),
            read: false,
            bookmark: false,
            last_page_read: 0,
            chapter_number: 1.0,
            source_order: 0,
            date_fetch: 0,
            date_upload: 0,
            last_modified_at: 0,
            version: 0,
            is_syncing: false,
        }
    }

    #[test]
    fn set_makes_the_excluded_set_equal_to_the_wanted_one() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let id = repo.insert(&Manga::create()).expect("insert");
        repo.set_excluded_scanlators(id, &set(&["A", "B"])).expect("set");
        assert_eq!(repo.excluded_scanlators(id).expect("get"), set(&["A", "B"]));
        repo.set_excluded_scanlators(id, &set(&["B", "C"])).expect("replace");
        assert_eq!(repo.excluded_scanlators(id).expect("get"), set(&["B", "C"]), "A removed, C added");
        repo.set_excluded_scanlators(id, &set(&[])).expect("clear");
        assert!(repo.excluded_scanlators(id).expect("get").is_empty());
    }

    #[test]
    fn the_excluded_list_is_published_after_each_set() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _guard = rt.enter();
        let (_d, db) = test_db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let id = repo.insert(&Manga::create()).expect("insert");
        let mut sub = repo.subscribe_excluded_scanlators(id);
        rt.block_on(async {
            let first = tokio::time::timeout(Duration::from_secs(5), sub.changed()).await.expect("first").expect("open");
            assert!(first.expect("query").is_empty());
        });
        repo.set_excluded_scanlators(id, &set(&["X"])).expect("set");
        rt.block_on(async {
            let next = tokio::time::timeout(Duration::from_secs(5), sub.changed()).await.expect("after set").expect("open");
            assert_eq!(*next.expect("query"), set(&["X"]));
        });
    }

    #[test]
    fn available_scanlators_are_distinct_and_drop_blank_names() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let id = repo.insert(&Manga::create()).expect("insert");
        db.write(|tx| {
            chapters::insert(tx, &chapter(id, "c1", Some("G1")))?;
            chapters::insert(tx, &chapter(id, "c2", Some("G1")))?;
            chapters::insert(tx, &chapter(id, "c3", Some("  ")))?;
            chapters::insert(tx, &chapter(id, "c4", None))?;
            chapters::insert(tx, &chapter(id, "c5", Some("G2")))?;
            Ok(())
        })
        .expect("chapters");
        assert_eq!(repo.available_scanlators(id).expect("available"), set(&["G1", "G2"]));
    }

    #[test]
    fn excluding_one_of_several_names_keeps_the_others() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let id = repo.insert(&Manga::create()).expect("insert");
        repo.set_excluded_scanlators(id, &set(&["G1", "G2"])).expect("both");
        repo.set_excluded_scanlators(id, &set(&["G1"])).expect("one");
        assert_eq!(repo.excluded_scanlators(id).expect("get"), set(&["G1"]));
    }
}
