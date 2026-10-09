//! Manga entries: the `Manga` and `MangaUpdate` shapes, `libraryView` rows and `MangaRepo`
//! (MangaRepositoryImpl.kt and MangaMapper.kt).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use kuro_db::codecs::{GenreList, UpdateStrategy};
use kuro_db::handle::Db;

use crate::flags_write;
use kuro_db::statements_manga::{self as sm, LibraryRow, MangaPatch, MangaRow};
use kuro_db::subscribe::Subscription;

use crate::{
    DataResult, ENTRY_DEBOUNCE, LIBRARY_DEBOUNCE, LIBRARY_RETRIES, LIBRARY_RETRY_DELAY, LibraryState, none_if_not_found,
    retrying, time,
};

/// Tables of one manga row (`subscribe_by_id`, `subscribe_by_url_and_source`, `subscribe_favorites_by_source`).
const MANGA_TABLE: &[&str] = &["mangas"];
const CHAPTER_TABLE: &[&str] = &["chapters"];
const EXCLUDED_TABLE: &[&str] = &["excluded_scanlators"];

/// The non-blank scanlators of a list (`GetAvailableScanlators` drops blank names).
fn available(scanlators: Vec<String>) -> BTreeSet<String> {
    scanlators.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

/// Tables a `libraryView` row reads (`subscribe_library`).
const LIBRARY_TABLES: &[&str] = &["mangas", "chapters", "mangas_categories", "excluded_scanlators", "history"];

/// Upcoming-tab statuses for manga: ONGOING (1) and PUBLISHING_FINISHED (4), as `GetUpcomingManga`
/// passes them.
pub const UPCOMING_STATUSES: [i64; 2] = [1, 4];

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
        Self {
            id: -1,
            source: -1,
            url: String::new(),
            title: String::new(),
            artist: None,
            author: None,
            description: None,
            genre: None,
            status: 0,
            thumbnail_url: None,
            favorite: false,
            last_update: 0,
            next_update: 0,
            fetch_interval: 0,
            date_added: 0,
            viewer_flags: 0,
            chapter_flags: 0,
            cover_last_modified: 0,
            update_strategy: UpdateStrategy::AlwaysUpdate,
            initialized: false,
            last_modified_at: 0,
            favorite_modified_at: None,
            version: 0,
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

/// `MangaMapper.mapManga`: NULL `last_update` and `next_update` read as 0; `calculate_interval` narrows
/// to `i32`; `is_syncing` is dropped.
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

/// `MangaMapper.mapLibraryManga`: the seven aggregate columns of `libraryView`.
pub fn map_library_manga(r: &LibraryRow) -> LibraryManga {
    LibraryManga {
        manga: map_manga(&r.manga),
        category: r.category,
        total_chapters: r.total_count,
        read_count: r.read_count,
        bookmark_count: r.bookmark_count,
        latest_upload: r.latest_upload,
        chapter_fetched_at: r.chapter_fetched_at,
        last_read: r.last_read,
    }
}

/// Row for the insert statement. The statement ignores `id`, `last_modified_at` and
/// `favorite_modified_at`, so their values here do not matter.
fn to_row(m: &Manga) -> MangaRow {
    MangaRow {
        id: m.id,
        source: m.source,
        url: m.url.clone(),
        artist: m.artist.clone(),
        author: m.author.clone(),
        description: m.description.clone(),
        genre: m.genre.clone().map(GenreList),
        title: m.title.clone(),
        status: m.status,
        thumbnail_url: m.thumbnail_url.clone(),
        favorite: m.favorite,
        last_update: Some(m.last_update),
        next_update: Some(m.next_update),
        initialized: m.initialized,
        viewer: m.viewer_flags,
        chapter_flags: m.chapter_flags,
        cover_last_modified: m.cover_last_modified,
        date_added: m.date_added,
        update_strategy: m.update_strategy,
        calculate_interval: i64::from(m.fetch_interval),
        last_modified_at: 0,
        favorite_modified_at: None,
        version: m.version,
        is_syncing: false,
    }
}

/// Patch for the coalesce statement. `is_syncing` is always written as 0 (never mark a row as syncing
/// from a repository write). Writing it makes the patch non-empty, so the statement runs even when
/// every other field is `None`: the triggers must fire on every update (EC-04).
fn to_patch(u: &MangaUpdate) -> MangaPatch {
    MangaPatch {
        id: u.id,
        source: u.source,
        url: u.url.clone(),
        artist: u.artist.clone(),
        author: u.author.clone(),
        description: u.description.clone(),
        genre: u.genre.clone().map(GenreList),
        title: u.title.clone(),
        status: u.status,
        thumbnail_url: u.thumbnail_url.clone(),
        favorite: u.favorite,
        last_update: u.last_update,
        next_update: u.next_update,
        initialized: u.initialized,
        viewer: u.viewer_flags,
        chapter_flags: u.chapter_flags,
        cover_last_modified: u.cover_last_modified,
        date_added: u.date_added,
        update_strategy: u.update_strategy,
        calculate_interval: u.fetch_interval.map(i64::from),
        version: u.version,
        is_syncing: Some(false),
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

/// Typed access to the manga tables. Shares one `Db` with the other repositories of the database.
#[derive(Clone)]
pub struct MangaRepo {
    db: Arc<Db>,
}

impl MangaRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// `NotFound` when no row has `id`.
    pub fn by_id(&self, id: i64) -> DataResult<Manga> {
        let row = self.db.read(|c| sm::mangas::get_by_id(c, id))?;
        Ok(map_manga(&row))
    }

    /// The first row (lowest `_id`) with this url and source.
    pub fn by_url_and_source(&self, url: &str, source: i64) -> DataResult<Option<Manga>> {
        let row = self.db.read(|c| Ok(sm::mangas::get_by_url_and_source(c, url, source)?))?;
        Ok(row.as_ref().map(map_manga))
    }

    pub fn favorites(&self) -> DataResult<Vec<Manga>> {
        self.read_all(sm::mangas::get_favorites)
    }

    pub fn favorites_by_source(&self, source: i64) -> DataResult<Vec<Manga>> {
        let rows = self.db.read(|c| Ok(sm::mangas::get_favorite_by_source_id(c, source)?))?;
        Ok(rows.iter().map(map_manga).collect())
    }

    /// The row with `id`, published again after each change to it. `None` once the row is deleted (EC-01).
    pub fn subscribe_by_id(&self, id: i64) -> Subscription<Option<Manga>> {
        self.db.subscribe(MANGA_TABLE, ENTRY_DEBOUNCE, move |c| {
            none_if_not_found(sm::mangas::get_by_id(c, id).map(|row| map_manga(&row)))
        })
    }

    /// The first row (lowest `_id`) with this url and source, published again after each change.
    pub fn subscribe_by_url_and_source(&self, url: &str, source: i64) -> Subscription<Option<Manga>> {
        let url = url.to_owned();
        self.db.subscribe(MANGA_TABLE, ENTRY_DEBOUNCE, move |c| {
            Ok(sm::mangas::get_by_url_and_source(c, &url, source)?.as_ref().map(map_manga))
        })
    }

    pub fn subscribe_favorites_by_source(&self, source: i64) -> Subscription<Vec<Manga>> {
        self.db.subscribe(MANGA_TABLE, ENTRY_DEBOUNCE, move |c| {
            Ok(sm::mangas::get_favorite_by_source_id(c, source)?.iter().map(map_manga).collect::<Vec<_>>())
        })
    }

    /// The `library()` rows, published again after a change to any table they read. A transient lock is retried
    /// (see `retrying`); after the last retry the error text is published instead of the rows.
    pub fn subscribe_library(&self) -> Subscription<LibraryState<LibraryManga>> {
        self.db.subscribe(
            LIBRARY_TABLES,
            LIBRARY_DEBOUNCE,
            retrying(LIBRARY_RETRIES, LIBRARY_RETRY_DELAY, |c| {
                Ok(sm::views::library(c)?.iter().map(map_library_manga).collect::<Vec<_>>())
            }),
        )
    }

    /// Non-favourites with a chapter that is read or has progress, and nothing else.
    pub fn read_not_in_library(&self) -> DataResult<Vec<Manga>> {
        self.read_all(sm::mangas::get_read_not_in_library)
    }

    /// One row per (favourite manga, category link); a manga with no link appears once with category 0.
    pub fn library(&self) -> DataResult<Vec<LibraryManga>> {
        let rows = self.db.read(|c| Ok(sm::views::library(c)?))?;
        Ok(rows.iter().map(map_library_manga).collect())
    }

    /// Library manga other than `id` whose title equals `title`, both lowercased with
    /// `str::to_lowercase` (the stored side through `lower_unicode`). Non-favourites never match.
    pub fn duplicates(&self, id: i64, title: &str) -> DataResult<Vec<Manga>> {
        let lowered = title.to_lowercase();
        let rows = self.db.read(|c| Ok(sm::mangas::get_duplicate_library_manga(c, &lowered, id)?))?;
        Ok(rows.iter().map(map_manga).collect())
    }

    /// Favourites whose `next_update` is at or after local midnight of `now_ms`'s day, soonest first.
    pub fn upcoming(&self, statuses: &[i64], now_ms: i64) -> DataResult<Vec<Manga>> {
        let start_of_day = time::start_of_local_day_millis(now_ms);
        let rows = self.db.read(|c| Ok(sm::mangas::get_upcoming_manga(c, start_of_day, statuses)?))?;
        Ok(rows.iter().map(map_manga).collect())
    }

    /// Favourites due from local midnight of the day `now` falls in, soonest first, published again after a
    /// change and at each local midnight. The cut-off is recomputed on every run (EC-03).
    pub fn subscribe_upcoming(&self, statuses: &[i64]) -> Subscription<Vec<Manga>> {
        self.subscribe_upcoming_with_clock(statuses, || chrono::Utc::now().timestamp_millis())
    }

    /// `subscribe_upcoming` with the clock injected, as epoch milliseconds.
    pub fn subscribe_upcoming_with_clock(
        &self,
        statuses: &[i64],
        now: impl Fn() -> i64 + Send + Sync + 'static,
    ) -> Subscription<Vec<Manga>> {
        let statuses = statuses.to_vec();
        let now = Arc::new(now);
        let clock = Arc::clone(&now);
        self.db.subscribe_timed(
            MANGA_TABLE,
            ENTRY_DEBOUNCE,
            move || time::until_next_local_midnight((*clock)()),
            move |c| {
                let start_of_day = time::start_of_local_day_millis((*now)());
                Ok(sm::mangas::get_upcoming_manga(c, start_of_day, &statuses)?.iter().map(map_manga).collect::<Vec<_>>())
            },
        )
    }

    /// The excluded scanlators of a manga.
    pub fn excluded_scanlators(&self, manga_id: i64) -> DataResult<BTreeSet<String>> {
        self.db.read(|c| Ok(sm::excluded_scanlators::get_by_manga_id(c, manga_id)?.into_iter().collect()))
    }

    /// Makes the excluded set of a manga equal to `wanted`, inserts and deletes in one transaction (`SetExcludedScanlators`).
    pub fn set_excluded_scanlators(&self, manga_id: i64, wanted: &BTreeSet<String>) -> DataResult<()> {
        self.db.write(|tx| Ok(sm::excluded_scanlators::set_excluded(tx, manga_id, wanted)?))
    }

    /// The excluded set, published again after each change of the table (`GetExcludedScanlators.subscribe`).
    pub fn subscribe_excluded_scanlators(&self, manga_id: i64) -> Subscription<BTreeSet<String>> {
        self.db.subscribe(EXCLUDED_TABLE, ENTRY_DEBOUNCE, move |c| {
            Ok(sm::excluded_scanlators::get_by_manga_id(c, manga_id)?.into_iter().collect())
        })
    }

    /// The distinct, non-blank scanlators of a manga's chapters (`GetAvailableScanlators`). A chapter without a
    /// scanlator adds nothing.
    pub fn available_scanlators(&self, manga_id: i64) -> DataResult<BTreeSet<String>> {
        self.db.read(|c| Ok(available(sm::chapters::get_scanlators_by_manga_id(c, manga_id)?)))
    }

    /// The available scanlators, published again after each change of the chapters.
    pub fn subscribe_available_scanlators(&self, manga_id: i64) -> Subscription<BTreeSet<String>> {
        self.db.subscribe(CHAPTER_TABLE, ENTRY_DEBOUNCE, move |c| {
            Ok(available(sm::chapters::get_scanlators_by_manga_id(c, manga_id)?))
        })
    }

    /// Writes `value` under `mask` into the viewer flags of one manga, in one statement (EC-23). `Ok(false)` when the
    /// manga does not exist (EC-24).
    pub fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool> {
        self.db.write(|tx| Ok(sm::mangas::set_viewer_bits(tx, id, value, mask)? > 0))
    }

    /// Sets `viewer = 0` on every manga.
    pub fn reset_viewer_flags(&self) -> DataResult<()> {
        self.db.write(|tx| Ok(sm::mangas::reset_viewer_flags(tx).map(drop)?))
    }

    /// Replaces the category links of one manga in one transaction. A missing category id rolls back
    /// the whole call and leaves the old links in place (EC-22).
    /// Sets the `mask` bits of `chapter_flags` to those of `flag` in one UPDATE. Returns whether the row changed.
    pub fn set_chapter_flags_masked(&self, manga_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        flags_write::masked(&self.db, flags_write::MANGA_CHAPTER_MASKED, manga_id, flag, mask)
    }

    /// Reads `chapter_flags`, applies `change` and stores the result in the writer transaction.
    pub fn update_chapter_flags(&self, manga_id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        flags_write::read_modify_write(
            &self.db,
            flags_write::MANGA_CHAPTER_READ,
            flags_write::MANGA_CHAPTER_WRITE,
            manga_id,
            change,
        )
    }

    pub fn set_categories(&self, manga_id: i64, category_ids: &[i64]) -> DataResult<()> {
        self.db.write(|tx| Ok(sm::mangas_categories::set_categories(tx, manga_id, category_ids)?))
    }

    /// Inserts `m` and returns the new id.
    pub fn insert(&self, m: &Manga) -> DataResult<i64> {
        let row = to_row(m);
        self.db.write(|tx| Ok(sm::mangas::insert(tx, &row)?))
    }

    /// Coalesce update of one row. A missing id is not an error (EC-27).
    pub fn update(&self, u: &MangaUpdate) -> DataResult<()> {
        let patch = to_patch(u);
        self.db.write(|tx| Ok(sm::mangas::update(tx, &patch).map(drop)?))
    }

    /// Every update in one transaction: one failure rolls back all of them.
    pub fn update_all(&self, updates: &[MangaUpdate]) -> DataResult<()> {
        self.db.write(|tx| {
            for u in updates {
                sm::mangas::update(tx, &to_patch(u))?;
            }
            Ok(())
        })
    }

    /// `(source, url)` of every manga, library or not.
    pub fn all_source_and_url(&self) -> DataResult<Vec<(i64, String)>> {
        self.db.read(|c| Ok(sm::mangas::get_all_source_and_url(c)?))
    }

    /// Deletes the non-library manga of `source_ids`. Favourites are never deleted, even when their
    /// source is listed.
    /// Deletes the non-favourite manga of these sources in one transaction. Consumed by the clear-database
    /// screen (area:47) and restore (area:22).
    pub fn delete_non_library_by_sources(&self, source_ids: &[i64]) -> DataResult<()> {
        self.db.write(|tx| Ok(sm::mangas::delete_not_in_library_by_source_ids(tx, source_ids).map(drop)?))
    }

    /// `(source, favourite count)` for every source with at least one favourite.
    pub fn favorite_counts_by_source(&self) -> DataResult<Vec<(i64, i64)>> {
        self.db.read(|c| Ok(sm::mangas::get_source_id_with_favorite_count(c)?))
    }

    /// `(source, non-library count)` for every source with at least one non-favourite manga.
    pub fn non_library_counts_by_source(&self) -> DataResult<Vec<(i64, i64)>> {
        self.db.read(|c| Ok(sm::mangas::get_source_ids_with_non_library_manga(c)?))
    }

    /// `favorite_counts_by_source`, published again after a change to a manga.
    pub fn subscribe_favorite_counts(&self) -> Subscription<Vec<(i64, i64)>> {
        self.db.subscribe(MANGA_TABLE, ENTRY_DEBOUNCE, |c| Ok(sm::mangas::get_source_id_with_favorite_count(c)?))
    }

    /// `non_library_counts_by_source`, published again after a change to a manga.
    pub fn subscribe_non_library_counts(&self) -> Subscription<Vec<(i64, i64)>> {
        self.db.subscribe(MANGA_TABLE, ENTRY_DEBOUNCE, |c| Ok(sm::mangas::get_source_ids_with_non_library_manga(c)?))
    }

    fn read_all(&self, f: fn(&rusqlite::Connection) -> rusqlite::Result<Vec<MangaRow>>) -> DataResult<Vec<Manga>> {
        let rows = self.db.read(|c| Ok(f(c)?))?;
        Ok(rows.iter().map(map_manga).collect())
    }
}

impl MangaRepo {
    /// Looks up `(url, source)` and inserts `m` when no row has it, in one writer transaction. Callers that
    /// race on the same key leave one row and get the same id (`NetworkToLocalManga`).
    pub fn lookup_or_insert(&self, m: &Manga) -> DataResult<crate::LookupOrInsert<Manga>> {
        let row = to_row(m);
        self.db.write(|tx| match sm::mangas::get_by_url_and_source(tx, &m.url, m.source)? {
            Some(found) => Ok(crate::LookupOrInsert::Found(map_manga(&found))),
            None => Ok(crate::LookupOrInsert::Inserted(sm::mangas::insert(tx, &row)?)),
        })
    }
}

#[cfg(test)]
mod excluded_scanlator_tests {
    use std::collections::BTreeSet;
    use std::time::Duration;

    use kuro_db::statements_manga::{ChapterRow, chapters};

    use super::*;
    use crate::tests::manga_db;

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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(Arc::clone(&db));
        let id = repo.insert(&Manga::create()).expect("insert");
        repo.set_excluded_scanlators(id, &set(&["G1", "G2"])).expect("both");
        repo.set_excluded_scanlators(id, &set(&["G1"])).expect("one");
        assert_eq!(repo.excluded_scanlators(id).expect("get"), set(&["G1"]));
    }
}
