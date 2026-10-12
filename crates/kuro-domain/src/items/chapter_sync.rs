//! `SyncChaptersWithSource` (T-10a-008): the manga names of the shared item sync ([`crate::items::sync`]). Everything
//! the refresh does is in [`SyncItems`]; this file turns the source chapter, the legacy `Manga` and `Chapter` shapes and
//! the old option and error types into it and back, and carries no logic of its own. Phase E deletes it.

use std::sync::Arc;

use kuro_data::chapter::Chapter;
use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_data::manga::Manga;
use kuro_db::Db;
use kuro_platform::kind::Kind;
use kuro_source::model::SChapter;

use crate::error::NoChapters;
use crate::items::source_sync::MangaSyncLocks;
use crate::items::sync::{
    ItemRenames, SourceItem, SyncItems, SyncItemsError, SyncOptions, mark_duplicate_done_from_pref, source_item_from_chapter,
};

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
    mark_duplicate_done_from_pref(Kind::Manga, rule)
}

/// A [`ChapterRenames`] as the shared [`ItemRenames`].
struct RenameAdapter(Arc<dyn ChapterRenames>);

impl ItemRenames for RenameAdapter {
    fn rename(&self, e: &Entry, stored: &Item, prepared: &Item) {
        self.0.rename(&Manga::from(e.clone()), &Chapter::from(stored.clone()), &Chapter::from(prepared.clone()));
    }
}

/// The chapter sync of one manga. Clones share the database handle and the per-manga locks.
#[derive(Clone)]
pub struct SyncChapters {
    sync: SyncItems,
}

impl SyncChapters {
    pub fn new(db: Arc<Db>, locks: Arc<MangaSyncLocks>, renames: Arc<dyn ChapterRenames>) -> Self {
        Self { sync: SyncItems::new(db, locks, Arc::new(RenameAdapter(renames))) }
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
        let hook = |s: &mut SourceItem| {
            let mut chapter = SChapter {
                url: s.url.clone(),
                name: s.name.clone(),
                date_upload: s.date_upload,
                chapter_number: s.number,
                scanlator: s.scanlator.clone(),
            };
            prepare(&mut chapter);
            s.url = chapter.url;
            s.name = chapter.name;
            s.date_upload = chapter.date_upload;
            s.number = chapter.chapter_number;
            s.scanlator = chapter.scanlator;
        };
        let options = SyncOptions {
            now_ms: opts.now_ms,
            manual: opts.manual,
            window: opts.window,
            mark_duplicate_done: opts.mark_duplicate_as_read,
            is_local: opts.is_local,
        };
        let items = raw.iter().map(source_item_from_chapter).collect();
        match self.sync.run(items, &Entry::from(manga), options, Some(&hook)).await {
            Ok(new) => Ok(new.into_iter().map(Chapter::from).collect()),
            Err(SyncItemsError::NoItems) => Err(NoChapters.into()),
            Err(SyncItemsError::Db(e)) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_db::Db;
    use kuro_db::sql::{chapters, mangas};
    use kuro_db::statements_manga::{ChapterRow, MangaPatch};

    /// The row the legacy insert statement takes.
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

    #[test]
    fn the_duplicate_preference_set_maps_to_the_flag() {
        let set = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<std::collections::BTreeSet<String>>();
        assert!(mark_duplicate_as_read_from_pref(&set(&["new"])), "on");
        assert!(mark_duplicate_as_read_from_pref(&set(&["new", "existing"])));
        assert!(!mark_duplicate_as_read_from_pref(&set(&[])), "off");
        assert!(!mark_duplicate_as_read_from_pref(&set(&["existing"])), "existing is another rule");
    }
    use crate::testdb::test_db;
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
        let (dir, db) = test_db();
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
            tx.execute_batch("CREATE TRIGGER boom BEFORE INSERT ON items WHEN NEW.url = '/boom' BEGIN SELECT RAISE(ABORT, 'boom'); END;")?;
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
