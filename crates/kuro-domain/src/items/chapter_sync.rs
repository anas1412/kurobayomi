//! `SyncChaptersWithSource` (T-10a-008): the applier of the chapter list refresh. It sanitises the source list, prepares
//! each row, calls the pure planner ([`crate::items::sync_plan`]), asks the download port to rename folders, then writes
//! the plan in ONE transaction: delete, insert, update, fetch interval and last update (the Kotlin runs these as
//! separate calls; one transaction is the deliberate change of area 10 section 5).

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Local};
use kuro_data::chapter::{Chapter, map_chapter};
use kuro_data::manga::Manga;
use kuro_db::Db;
use kuro_db::statements_manga::{ChapterPatch, ChapterRow, MangaPatch};
use kuro_db::sql::{chapters, excluded_scanlators, mangas};
use kuro_source::model::{SChapter, normalize_scanlator};

use crate::entries::fetch_interval::{ItemDates, Stored, to_update};
use crate::error::NoChapters;
use crate::items::recognition::parse_chapter_number;
use crate::items::should_update::should_update_db_chapter;
use crate::items::source_sync::MangaSyncLocks;
use crate::items::model::widen_source_number;
use kuro_data::sanitize::sanitize_item_name;
use crate::items::sync_plan::{PlanInput, SyncRow, plan_sync};

/// The download folder rename of a changed chapter (`downloadManager.renameChapter`). The implementation decides whether a
/// folder exists and whether its name changed. It logs its own failures: a failed rename never fails the sync (EC-11).
pub trait ChapterRenames: Send + Sync {
    fn rename(&self, manga: &Manga, stored: &Chapter, prepared: &Chapter);
}

/// A download port that does nothing (no downloads on disk, or the download layer is not wired yet).
pub struct NoRenames;

impl ChapterRenames for NoRenames {
    fn rename(&self, _manga: &Manga, _stored: &Chapter, _prepared: &Chapter) {}
}

/// Why a chapter refresh failed.
#[derive(Debug, thiserror::Error)]
pub enum ChapterSyncError {
    /// A non-local source returned no chapters. Nothing was written (EC-01).
    #[error(transparent)]
    NoChapters(#[from] NoChapters),
    #[error("database error: {0}")]
    Db(#[from] kuro_db::DbError),
}

/// The options of one refresh: the manual flag, the fetch window (`(0, 0)` means "now"), the duplicate-read preference
/// and whether the source is the local one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChapterSyncOptions {
    /// The refresh time, epoch milliseconds (the clock is an input, so the sync is deterministic under test).
    pub now_ms: i64,
    pub manual: bool,
    pub window: (i64, i64),
    pub mark_duplicate_as_read: bool,
    pub is_local: bool,
}

/// The duplicate rule from its multi-select preference: on when the set holds `"new"` (`MARK_DUPLICATE_CHAPTER_READ_NEW`). The `"existing"`
/// value is a separate rule and does not turn this one on.
pub fn mark_duplicate_as_read_from_pref(rule: &std::collections::BTreeSet<String>) -> bool {
    rule.contains(kuro_prefs::library::MARK_DUPLICATE_CHAPTER_READ_NEW)
}

/// The source chapter as a stored-row shape: name sanitised, url, date, number and a trimmed scanlator (`copyFromSChapter`).
fn chapter_from_source(manga: &Manga, source: &SChapter, position: usize) -> Chapter {
    Chapter {
        manga_id: manga.id,
        name: sanitize_item_name(&source.name, &manga.title),
        url: source.url.clone(),
        date_upload: source.date_upload,
        chapter_number: widen_source_number(source.chapter_number),
        scanlator: normalize_scanlator(source.scanlator.as_deref()),
        source_order: i64::try_from(position).unwrap_or(i64::MAX),
        ..Chapter::create()
    }
}

/// `toSChapter`: the stored-row shape back to the source shape, for the prepare hook.
fn to_s_chapter(chapter: &Chapter) -> SChapter {
    SChapter {
        url: chapter.url.clone(),
        name: chapter.name.clone(),
        date_upload: chapter.date_upload,
        chapter_number: chapter.chapter_number as f32,
        scanlator: chapter.scanlator.clone(),
    }
}

/// `copyFromSChapter` of a prepared source chapter onto the row it came from. Only the source fields change.
fn copy_from_s_chapter(base: &Chapter, source: &SChapter) -> Chapter {
    Chapter {
        name: source.name.clone(),
        url: source.url.clone(),
        date_upload: source.date_upload,
        chapter_number: widen_source_number(source.chapter_number),
        scanlator: normalize_scanlator(source.scanlator.as_deref()),
        ..base.clone()
    }
}

impl SyncRow for Chapter {
    fn id(&self) -> i64 {
        self.id
    }
    fn url(&self) -> &str {
        &self.url
    }
    fn number(&self) -> f64 {
        self.chapter_number
    }
    fn read(&self) -> bool {
        self.read
    }
    fn bookmark(&self) -> bool {
        self.bookmark
    }
    fn date_fetch(&self) -> i64 {
        self.date_fetch
    }
    fn date_upload(&self) -> i64 {
        self.date_upload
    }
    fn set_source_order(&mut self, order: i64) {
        self.source_order = order;
    }
    fn set_date_upload(&mut self, date: i64) {
        self.date_upload = date;
    }
    fn set_date_fetch(&mut self, date: i64) {
        self.date_fetch = date;
    }
    fn set_read(&mut self, read: bool) {
        self.read = read;
    }
    fn set_bookmark(&mut self, bookmark: bool) {
        self.bookmark = bookmark;
    }
    fn needs_update(db: &Self, src: &Self) -> bool {
        should_update_db_chapter(db, src)
    }
    /// `toChange`: name, number, scanlator and source order from the source; the upload date only when the source has one.
    fn merged(db: &Self, src: &Self) -> Self {
        Chapter {
            name: src.name.clone(),
            chapter_number: src.chapter_number,
            scanlator: src.scanlator.clone(),
            source_order: src.source_order,
            date_upload: if src.date_upload != 0 { src.date_upload } else { db.date_upload },
            ..db.clone()
        }
    }
}

/// The row the insert statement takes.
fn chapter_row(c: &Chapter) -> ChapterRow {
    ChapterRow {
        id: c.id,
        manga_id: c.manga_id,
        url: c.url.clone(),
        name: c.name.clone(),
        scanlator: c.scanlator.clone(),
        read: c.read,
        bookmark: c.bookmark,
        last_page_read: c.last_page_read,
        chapter_number: c.chapter_number,
        source_order: c.source_order,
        date_fetch: c.date_fetch,
        date_upload: c.date_upload,
        last_modified_at: 0,
        version: c.version,
        is_syncing: false,
    }
}

/// The columns a refresh owns. Read and bookmark are NOT written, so a read made while the sync runs is not lost (race 1).
/// A `None` scanlator keeps the stored one (coalesce, EC-08).
fn chapter_patch(c: &Chapter) -> ChapterPatch {
    ChapterPatch {
        id: c.id,
        manga_id: None,
        url: None,
        name: Some(c.name.clone()),
        scanlator: c.scanlator.clone(),
        read: None,
        bookmark: None,
        last_page_read: None,
        chapter_number: Some(c.chapter_number),
        source_order: Some(c.source_order),
        date_fetch: None,
        date_upload: Some(c.date_upload),
        version: None,
        is_syncing: Some(false),
    }
}

/// The chapter sync of one manga. Clones share the database handle and the per-manga locks.
#[derive(Clone)]
pub struct SyncChapters {
    db: Arc<Db>,
    locks: Arc<MangaSyncLocks>,
    renames: Arc<dyn ChapterRenames>,
}

impl SyncChapters {
    pub fn new(db: Arc<Db>, locks: Arc<MangaSyncLocks>, renames: Arc<dyn ChapterRenames>) -> Self {
        Self { db, locks, renames }
    }

    /// `await(rawSourceChapters, manga, source, manualFetch, fetchWindow)`. Returns the NEW chapters that should notify and
    /// auto-download. `prepare` is the source's `prepareNewChapter` hook; the caller passes a no-op for the local source.
    pub async fn run(
        &self,
        raw: Vec<SChapter>,
        manga: &Manga,
        opts: ChapterSyncOptions,
        prepare: &(dyn Fn(&mut SChapter) + Sync),
    ) -> Result<Vec<Chapter>, ChapterSyncError> {
        if raw.is_empty() && !opts.is_local {
            return Err(NoChapters.into());
        }
        let now_ms = opts.now_ms;
        let now = DateTime::from_timestamp_millis(now_ms).map(|utc| utc.with_timezone(&Local)).unwrap_or_else(Local::now);
        let _guard = self.locks.lock(manga.id).await;

        let stored = self.db.read(|c| Ok(chapters::get_by_manga_id(c, manga.id, false)?.iter().map(map_chapter).collect::<Vec<_>>()))?;

        let source: Vec<Chapter> = raw.iter().enumerate().map(|(i, s)| chapter_from_source(manga, s, i)).collect();
        // The hook sees the source shape of each row. The date of a new row is judged on the raw row (see the planner).
        let prepare_row = |row: Chapter| -> Chapter {
            let mut s = to_s_chapter(&row);
            prepare(&mut s);
            let prepared = copy_from_s_chapter(&row, &s);
            let number = parse_chapter_number(&manga.title, &prepared.name, Some(prepared.chapter_number));
            Chapter { chapter_number: number, ..prepared }
        };
        let plan = plan_sync(PlanInput {
            source,
            db: &stored,
            now_ms,
            mark_duplicate_as_read: opts.mark_duplicate_as_read,
            prepare: &prepare_row,
        });

        for (db_row, prepared) in &plan.renames {
            self.renames.rename(manga, db_row, prepared);
        }

        let window = opts.window;
        let fetch_needed = opts.manual || manga.fetch_interval == 0 || manga.next_update < window.0;
        let changed = !plan.is_empty();
        let (inserted, excluded) = self.db.write(|tx| {
            if changed {
                if !plan.removed_ids.is_empty() {
                    chapters::remove_with_ids(tx, &plan.removed_ids)?;
                }
            }
            let mut inserted: Vec<Chapter> = Vec::new();
            if changed && !plan.added.is_empty() {
                let rows: Vec<ChapterRow> = plan.added.iter().map(chapter_row).collect();
                let ids = chapters::insert_all(tx, &rows)?;
                inserted = plan.added.iter().zip(ids).map(|(c, id)| Chapter { id, ..c.clone() }).collect();
            }
            for updated in &plan.updated {
                chapters::update(tx, &chapter_patch(updated))?;
            }
            if changed || fetch_needed {
                let items: Vec<ItemDates> = if manga.fetch_interval < 0 {
                    Vec::new()
                } else {
                    chapters::get_by_manga_id(tx, manga.id, true)?.iter().map(map_chapter).map(|c| ItemDates::from(&c)).collect()
                };
                let stored_interval = Stored {
                    fetch_interval: manga.fetch_interval,
                    next_update: manga.next_update,
                    last_update: manga.last_update,
                };
                let out = to_update(stored_interval, &items, &now, window, now_ms);
                let patch = MangaPatch {
                    next_update: Some(out.next_update),
                    calculate_interval: Some(i64::from(out.fetch_interval)),
                    ..empty_manga_patch(manga.id)
                };
                mangas::update(tx, &patch)?;
            }
            if changed {
                let patch = MangaPatch { last_update: Some(now_ms), ..empty_manga_patch(manga.id) };
                mangas::update(tx, &patch)?;
            }
            let excluded: HashSet<String> = if changed {
                excluded_scanlators::get_by_manga_id(tx, manga.id)?.into_iter().collect()
            } else {
                HashSet::new()
            };
            Ok((inserted, excluded))
        })?;

        Ok(inserted
            .into_iter()
            .filter(|c| !plan.changed_or_duplicate_urls.contains(&c.url))
            .filter(|c| !c.scanlator.as_ref().is_some_and(|s| excluded.contains(s)))
            .collect())
    }
}

/// A manga patch that writes nothing: every column is `None`, so only the named fields change.
fn empty_manga_patch(id: i64) -> MangaPatch {
    MangaPatch {
        id,
        source: None,
        url: None,
        artist: None,
        author: None,
        description: None,
        genre: None,
        title: None,
        status: None,
        thumbnail_url: None,
        favorite: None,
        last_update: None,
        next_update: None,
        initialized: None,
        viewer: None,
        chapter_flags: None,
        cover_last_modified: None,
        date_added: None,
        update_strategy: None,
        calculate_interval: None,
        version: None,
        is_syncing: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_duplicate_preference_set_maps_to_the_flag() {
        let set = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<std::collections::BTreeSet<String>>();
        assert!(mark_duplicate_as_read_from_pref(&set(&["new"])), "on");
        assert!(mark_duplicate_as_read_from_pref(&set(&["new", "existing"])));
        assert!(!mark_duplicate_as_read_from_pref(&set(&[])), "off");
        assert!(!mark_duplicate_as_read_from_pref(&set(&["existing"])), "existing is another rule");
    }
    use crate::testdb::manga_db;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingRenames {
        calls: Mutex<Vec<(String, String)>>,
    }

    impl ChapterRenames for RecordingRenames {
        fn rename(&self, _manga: &Manga, stored: &Chapter, prepared: &Chapter) {
            self.calls.lock().expect("lock").push((stored.url.clone(), prepared.name.clone()));
        }
    }

    fn fixture(renames: Arc<RecordingRenames>) -> (tempfile::TempDir, Arc<Db>, SyncChapters, i64) {
        let (dir, db) = manga_db();
        let manga_id = db.write(|tx| Ok(mangas::insert(tx, &empty_insert_row())?)).expect("manga");
        let sync = SyncChapters::new(db.clone(), Arc::new(MangaSyncLocks::default()), renames);
        (dir, db, sync, manga_id)
    }

    fn empty_insert_row() -> kuro_db::statements_manga::MangaRow {
        let m = Manga { title: "Foo".into(), ..Manga::create() };
        kuro_db::statements_manga::MangaRow {
            id: 0,
            source: m.source,
            url: m.url.clone(),
            artist: None,
            author: None,
            description: None,
            genre: None,
            title: m.title.clone(),
            status: 0,
            thumbnail_url: None,
            favorite: false,
            last_update: Some(0),
            next_update: Some(0),
            initialized: false,
            viewer: 0,
            chapter_flags: 0,
            cover_last_modified: 0,
            date_added: 0,
            update_strategy: kuro_db::codecs::UpdateStrategy::AlwaysUpdate,
            calculate_interval: 7,
            last_modified_at: 0,
            favorite_modified_at: None,
            version: 1,
            is_syncing: false,
        }
    }

    fn manga_of(db: &Db, id: i64) -> Manga {
        kuro_data::manga::map_manga(&db.read(|c| Ok(mangas::get_by_id(c, id)?)).expect("manga row"))
    }


    fn insert_db_chapter(db: &Db, c: &Chapter) -> i64 {
        db.write(|tx| Ok(chapters::insert(tx, &chapter_row(c))?)).expect("insert")
    }

    fn s(url: &str, name: &str, date: i64) -> SChapter {
        SChapter { url: url.into(), name: name.into(), date_upload: date, chapter_number: -1.0, scanlator: None }
    }

    fn opts(manual: bool) -> ChapterSyncOptions {
        ChapterSyncOptions { now_ms: NOW_MS, manual, window: (0, 0), mark_duplicate_as_read: true, is_local: false }
    }

    fn no_hook(_: &mut SChapter) {}

    const NOW_MS: i64 = 1_000_000;

    #[tokio::test]
    async fn an_empty_remote_list_is_an_error_and_writes_nothing() {
        let renames = Arc::new(RecordingRenames::default());
        let (_dir, db, sync, id) = fixture(renames.clone());
        let manga = manga_of(&db, id);
        let err = sync.run(Vec::new(), &manga, opts(false), &no_hook).await.expect_err("error");
        assert!(matches!(err, ChapterSyncError::NoChapters(_)), "EC-01");
        assert!(db.read(|c| Ok(chapters::get_by_manga_id(c, id, false)?)).expect("read").is_empty());
    }

    #[tokio::test]
    async fn an_empty_local_list_is_fine() {
        let (_dir, db, sync, id) = fixture(Arc::new(RecordingRenames::default()));
        let manga = manga_of(&db, id);
        let local = ChapterSyncOptions { is_local: true, ..opts(false) };
        assert!(sync.run(Vec::new(), &manga, local, &no_hook).await.expect("ok").is_empty());
    }

    /// The worked example of `p1-topic-chapter-episode-sync.md` section 3, against the database.
    #[tokio::test]
    async fn worked_example() {
        let renames = Arc::new(RecordingRenames::default());
        let (_dir, db, sync, id) = fixture(renames.clone());
        insert_db_chapter(&db, &Chapter { manga_id: id, url: "/c1".into(), name: "Ch 1".into(), chapter_number: 1.0, read: true, date_fetch: 100, source_order: 4, ..Chapter::create() });
        insert_db_chapter(&db, &Chapter { manga_id: id, url: "/c2".into(), name: "Ch 2".into(), chapter_number: 2.0, date_fetch: 200, source_order: 1, ..Chapter::create() });
        insert_db_chapter(&db, &Chapter { manga_id: id, url: "/c3".into(), name: "Ch 3".into(), chapter_number: 3.0, read: true, bookmark: true, date_fetch: 500, source_order: 2, ..Chapter::create() });
        let raw = vec![s("/c5", "Ch 5", 0), s("/c4", "Ch 4", 900), s("/c3b", "Ch 3", 0), s("/c2", "Ch 2", 0), s("/c1", "Ch 1", 0)];
        let manga = manga_of(&db, id);
        let returned = sync.run(raw, &manga, opts(false), &no_hook).await.expect("sync");
        let urls: Vec<&str> = returned.iter().map(|c| c.url.as_str()).collect();
        assert_eq!(urls, ["/c5", "/c4"], "c3b is a duplicate of a removed read chapter, not new");
        let all = db.read(|c| Ok(chapters::get_by_manga_id(c, id, false)?)).expect("read");
        let by_url = |u: &str| all.iter().find(|r| r.url == u).expect("row").clone();
        assert_eq!(by_url("/c5").date_fetch, NOW_MS + 3, "newest first: the first listed new chapter is now + 3");
        assert_eq!(by_url("/c5").date_upload, NOW_MS, "no date and no earlier date: now");
        assert_eq!(by_url("/c4").date_fetch, NOW_MS + 2);
        assert!(by_url("/c3b").read && by_url("/c3b").bookmark);
        assert_eq!(by_url("/c3b").date_fetch, 500);
        assert!(all.iter().all(|r| r.url != "/c3"), "the removed row is gone");
        assert_eq!(renames.calls.lock().expect("lock").len(), 1, "only /c2 changes: its stored source order is 1, the source says 3");
    }

    #[tokio::test]
    async fn an_unchanged_second_sync_writes_nothing_and_keeps_last_update() {
        let (_dir, db, sync, id) = fixture(Arc::new(RecordingRenames::default()));
        let raw = vec![s("/a", "Ch 1", 10), s("/b", "Ch 2", 20)];
        let manga = manga_of(&db, id);
        let first = sync.run(raw.clone(), &manga, opts(false), &no_hook).await.expect("first");
        assert_eq!(first.len(), 2);
        let after_first = manga_of(&db, id);
        let second = sync.run(raw, &after_first, opts(false), &no_hook).await.expect("second");
        assert!(second.is_empty(), "EC-09: nothing new");
        assert_eq!(manga_of(&db, id).last_update, after_first.last_update, "no last_update bump without changes");
    }

    #[tokio::test]
    async fn a_scanlator_that_disappears_keeps_the_stored_one() {
        let (_dir, db, sync, id) = fixture(Arc::new(RecordingRenames::default()));
        let _ = insert_db_chapter(&db, &Chapter { manga_id: id, url: "/c".into(), name: "Ch 1".into(), chapter_number: 1.0, scanlator: Some("Team".into()), ..Chapter::create() });
        let manga = manga_of(&db, id);
        sync.run(vec![s("/c", "Ch 1", 0)], &manga, opts(false), &no_hook).await.expect("sync");
        let row = db.read(|c| Ok(chapters::get_by_manga_id(c, id, false)?)).expect("read").remove(0);
        assert_eq!(row.scanlator.as_deref(), Some("Team"), "EC-08");
    }

    #[tokio::test]
    async fn duplicate_urls_are_dropped_before_the_order_is_assigned() {
        let (_dir, db, sync, id) = fixture(Arc::new(RecordingRenames::default()));
        let manga = manga_of(&db, id);
        let raw = vec![s("/x", "Ch 1", 0), s("/x", "Ch 1 again", 0), s("/y", "Ch 2", 0)];
        let returned = sync.run(raw, &manga, opts(false), &no_hook).await.expect("sync");
        assert_eq!(returned.iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), ["/x", "/y"]);
        assert_eq!(returned[1].source_order, 1, "EC-03: the second distinct URL is at position 1");
    }

    /// The delete, insert and the rest run in one transaction: a failing insert rolls back the delete.
    #[tokio::test]
    async fn a_failing_insert_rolls_back_the_delete() {
        let (_dir, db, sync, id) = fixture(Arc::new(RecordingRenames::default()));
        insert_db_chapter(&db, &Chapter { manga_id: id, url: "/gone".into(), name: "Ch 1".into(), chapter_number: 1.0, ..Chapter::create() });
        db.write(|tx| {
            tx.execute_batch("CREATE TRIGGER boom BEFORE INSERT ON chapters WHEN NEW.url = '/boom' BEGIN SELECT RAISE(ABORT, 'boom'); END;")?;
            Ok(())
        })
        .expect("trigger");
        let manga = manga_of(&db, id);
        let urls = |db: &Db| -> Vec<String> { db.read(|c| Ok(chapters::get_by_manga_id(c, id, false)?)).expect("read").into_iter().map(|r| r.url).collect() };
        let err = sync.run(vec![s("/boom", "Ch 2", 0)], &manga, opts(false), &no_hook).await.expect_err("the insert fails");
        assert!(matches!(err, ChapterSyncError::Db(_)));
        assert_eq!(urls(&db), ["/gone"], "the removed row is still stored");
        assert_eq!(manga_of(&db, id).last_update, manga.last_update, "no last_update write survived either");
        // Control: without the failing insert the same plan does delete the row.
        db.write(|tx| {
            tx.execute_batch("DROP TRIGGER boom;")?;
            Ok(())
        })
        .expect("drop");
        sync.run(vec![s("/boom", "Ch 2", 0)], &manga, opts(false), &no_hook).await.expect("sync");
        assert_eq!(urls(&db), ["/boom"]);
    }

    /// RC-08: `run` takes the per-manga lock, so a held lock stops it before any write.
    #[tokio::test]
    async fn run_waits_for_the_per_manga_lock() {
        let (_dir, db, _sync, id) = fixture(Arc::new(RecordingRenames::default()));
        let locks = Arc::new(MangaSyncLocks::default());
        let sync = SyncChapters::new(db.clone(), locks.clone(), Arc::new(NoRenames));
        let manga = manga_of(&db, id);
        let held = locks.lock(id).await;
        let blocked = tokio::time::timeout(std::time::Duration::from_millis(50), sync.run(vec![s("/a", "Ch 1", 0)], &manga, opts(false), &no_hook)).await;
        assert!(blocked.is_err(), "the sync waits while the lock is held");
        assert!(db.read(|c| Ok(chapters::get_by_manga_id(c, id, false)?)).expect("read").is_empty(), "nothing was written");
        drop(held);
        assert_eq!(sync.run(vec![s("/a", "Ch 1", 0)], &manga, opts(false), &no_hook).await.expect("sync").len(), 1);
    }

    /// A stored manga with an automatic interval and a `next_update` no recompute would produce, so a recompute shows.
    const SENTINEL_NEXT: i64 = 7_777_777;
    const WINDOW: (i64, i64) = (1_000_000, 2_000_000);

    fn set_interval(db: &Db, id: i64, interval: i64, next_update: i64) {
        let patch = MangaPatch { next_update: Some(next_update), calculate_interval: Some(interval), ..empty_manga_patch(id) };
        db.write(|tx| Ok(mangas::update(tx, &patch)?)).expect("patch");
    }

    /// Syncs once (changes), pins the interval fields, then syncs the same list again with the given options.
    async fn second_sync_row(interval: i64, next_update: i64, manual: bool) -> (Manga, Manga) {
        let (_dir, db, sync, id) = fixture(Arc::new(RecordingRenames::default()));
        let raw = vec![s("/a", "Ch 1", 10), s("/b", "Ch 2", 20)];
        let first = ChapterSyncOptions { window: WINDOW, ..opts(false) };
        sync.run(raw.clone(), &manga_of(&db, id), first, &no_hook).await.expect("first");
        set_interval(&db, id, interval, next_update);
        let before = manga_of(&db, id);
        let again = ChapterSyncOptions { window: WINDOW, ..opts(manual) };
        let out = sync.run(raw, &before, again, &no_hook).await.expect("second");
        assert!(out.is_empty());
        (before, manga_of(&db, id))
    }

    /// S-11: no change, interval set, next update not before the window start, not manual: nothing is written.
    #[tokio::test]
    async fn s11_no_change_and_no_trigger_recomputes_nothing() {
        let (before, after) = second_sync_row(7, SENTINEL_NEXT, false).await;
        assert_eq!(after.next_update, SENTINEL_NEXT);
        assert_eq!(after.fetch_interval, 7);
        assert_eq!(after.last_update, before.last_update);
    }

    /// S-12: no change, but a manual refresh, interval 0, or a next update before the window start recomputes. `last_update` stays.
    #[tokio::test]
    async fn s12_no_change_recomputes_on_manual_interval_zero_or_overdue() {
        for (interval, next_update, manual, why) in [(7, SENTINEL_NEXT, true, "manual"), (0, SENTINEL_NEXT, false, "interval 0"), (7, 5, false, "next update before the window start")] {
            let (before, after) = second_sync_row(interval, next_update, manual).await;
            assert!(after.next_update != next_update || after.fetch_interval != i32::try_from(interval).unwrap(), "{why}: recomputed");
            assert_eq!(after.last_update, before.last_update, "{why}: last_update is not touched");
        }
    }

    /// S-13: with changes the interval is recomputed even when no trigger holds, and `last_update` becomes now.
    #[tokio::test]
    async fn s13_changes_recompute_and_set_last_update_to_now() {
        let (_dir, db, sync, id) = fixture(Arc::new(RecordingRenames::default()));
        set_interval(&db, id, 7, SENTINEL_NEXT);
        let before = manga_of(&db, id);
        let o = ChapterSyncOptions { window: WINDOW, ..opts(false) };
        assert_eq!(sync.run(vec![s("/a", "Ch 1", 10)], &before, o, &no_hook).await.expect("sync").len(), 1);
        let after = manga_of(&db, id);
        assert_eq!(after.last_update, NOW_MS);
        assert_ne!(after.next_update, SENTINEL_NEXT, "recomputed");
    }
}
