//! Legacy per-kind history names over `entry_history::HistoryRepo` (`MangaHistoryRepo`, `AnimeHistoryRepo`) and the
//! session timer that writes them (`HistoryRecorder`). Phase E deletes the wrappers; the SQL lives in `kuro-db`.
//!
//! The write methods that Kotlin makes infallible log their failure and return `false` or nothing; their `try_*`
//! twins return the error for callers and tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_platform::kind::Kind;
use rusqlite::Connection;

pub use kuro_db::schema_anime::Millis;

use crate::entry_history::{self as shared, HistoryRepo};
use crate::DataResult;

/// One `history` row of a chapter.
#[derive(Clone, Debug, PartialEq)]
pub struct MangaHistory {
    pub id: i64,
    pub chapter_id: i64,
    /// Epoch milliseconds of the last read. Zero after a reset: the row stays, the list hides it.
    pub last_read: i64,
    /// Accumulated read time in milliseconds. Reset rows keep it.
    pub time_read: i64,
}

impl From<shared::History> for MangaHistory {
    fn from(r: shared::History) -> Self {
        Self { id: r.id, chapter_id: r.item_id, last_read: r.last_at, time_read: r.duration_ms }
    }
}

/// One line of the manga history list: the latest-read chapter of a manga, with its manga's cover data.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryItem {
    pub id: i64,
    pub manga_id: i64,
    pub chapter_id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub source: i64,
    /// The `favorite` flag of the manga: the cover's `is_manga_favorite`.
    pub favorite: bool,
    pub cover_last_modified: i64,
    pub chapter_number: f64,
    pub read_at: i64,
    pub read_duration: i64,
}

impl From<shared::HistoryItem> for HistoryItem {
    fn from(r: shared::HistoryItem) -> Self {
        Self {
            id: r.id,
            manga_id: r.entry_id,
            chapter_id: r.item_id,
            title: r.title,
            thumbnail_url: r.thumbnail_url,
            source: r.source,
            favorite: r.favorite,
            cover_last_modified: r.cover_last_modified,
            chapter_number: r.item_number,
            read_at: r.last_at,
            read_duration: r.duration_ms,
        }
    }
}

/// Manga history access (`MangaHistoryRepositoryImpl.kt`).
#[derive(Clone)]
pub struct MangaHistoryRepo {
    repo: HistoryRepo,
}

impl MangaHistoryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { repo: HistoryRepo::new(db) }
    }

    /// The history list for `query`, published again after a change to history, chapters or mangas.
    pub fn watch(&self, query: &str) -> Subscription<Vec<HistoryItem>> {
        self.repo.watch_mapped(Kind::Manga, query, HistoryItem::from)
    }

    /// The history list for `query`, newest read first, ties broken by the higher id.
    pub fn list(&self, query: &str) -> DataResult<Vec<HistoryItem>> {
        Ok(self.repo.list(Kind::Manga, query)?.into_iter().map(HistoryItem::from).collect())
    }

    /// The row with the greatest `readAt > 0`, or `None` when nothing was read.
    pub fn latest(&self) -> DataResult<Option<HistoryItem>> {
        Ok(self.repo.latest(Kind::Manga)?.map(HistoryItem::from))
    }

    /// Accumulated read time over every history row, reset rows included. Zero when there are none.
    pub fn total_read_duration(&self) -> DataResult<i64> {
        self.repo.total_duration(Kind::Manga)
    }

    /// Every history row of one manga, reset rows included.
    pub fn by_manga_id(&self, manga_id: i64) -> DataResult<Vec<MangaHistory>> {
        Ok(self.repo.by_entry_id(manga_id)?.into_iter().map(MangaHistory::from).collect())
    }

    /// The history row of the chapter with this url, as the backup restorer reads it.
    pub fn by_chapter_url(c: &Connection, url: &str) -> DataResult<Option<MangaHistory>> {
        Ok(HistoryRepo::by_item_url(c, Kind::Manga, url)?.map(MangaHistory::from))
    }

    /// Sets `last_read = 0` on one row, logging a failure.
    pub fn reset_by_id(&self, history_id: i64) -> bool {
        logged("reset_by_id", self.try_reset_by_id(history_id))
    }

    pub fn try_reset_by_id(&self, history_id: i64) -> DataResult<()> {
        self.repo.reset_by_id(history_id)
    }

    /// Resets every history row of one manga, logging a failure.
    pub fn reset_by_manga_id(&self, manga_id: i64) -> bool {
        logged("reset_by_manga_id", self.try_reset_by_manga_id(manga_id))
    }

    pub fn try_reset_by_manga_id(&self, manga_id: i64) -> DataResult<()> {
        self.repo.reset_by_entry_id(manga_id)
    }

    /// Deletes every manga history row, time included. Returns `true` on success; a failure is logged.
    pub fn delete_all(&self) -> bool {
        logged("delete_all", self.try_delete_all())
    }

    pub fn try_delete_all(&self) -> DataResult<()> {
        self.repo.delete_all(Kind::Manga)
    }

    /// Records a read of `chapter_id`: sets `last_read` to `read_at` and adds `duration` to `time_read`.
    /// A failure (for example a chapter that does not exist) is logged and not returned.
    pub fn upsert(&self, chapter_id: i64, read_at: i64, duration: i64) -> bool {
        logged("upsert", self.try_upsert(chapter_id, read_at, duration))
    }

    pub fn try_upsert(&self, chapter_id: i64, read_at: i64, duration: i64) -> DataResult<()> {
        self.repo.upsert(chapter_id, read_at, duration)
    }

    /// Deletes the reset rows (`last_read = 0`) inside an open transaction, for the restore path.
    pub fn remove_resetted(c: &Connection) -> DataResult<usize> {
        HistoryRepo::remove_resetted(c, Kind::Manga)
    }
}

/// One line of the anime history list (the `animehistoryView` columns).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimeHistoryItem {
    pub id: i64,
    pub anime_id: i64,
    pub episode_id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub source: i64,
    pub favorite: bool,
    pub cover_last_modified: i64,
    pub episode_number: f64,
    pub seen_at: Millis,
}

impl From<shared::HistoryItem> for AnimeHistoryItem {
    fn from(r: shared::HistoryItem) -> Self {
        Self {
            id: r.id,
            anime_id: r.entry_id,
            episode_id: r.item_id,
            title: r.title,
            thumbnail_url: r.thumbnail_url,
            source: r.source,
            favorite: r.favorite,
            cover_last_modified: r.cover_last_modified,
            episode_number: r.item_number,
            seen_at: Millis(r.last_at),
        }
    }
}

/// One `animehistory` row of an episode.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimeHistory {
    pub id: i64,
    pub episode_id: i64,
    /// Epoch milliseconds of the last view; zero after a reset.
    pub last_seen: Option<i64>,
}

impl From<shared::History> for AnimeHistory {
    fn from(r: shared::History) -> Self {
        Self { id: r.id, episode_id: r.item_id, last_seen: Some(r.last_at) }
    }
}

/// Anime history access (`AnimeHistoryRepositoryImpl.kt`). The playback time is not recorded yet, so every upsert adds 0.
#[derive(Clone)]
pub struct AnimeHistoryRepo {
    repo: HistoryRepo,
}

impl AnimeHistoryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { repo: HistoryRepo::new(db) }
    }

    /// The anime history list for `query`, published again after a change to history, episodes or animes.
    pub fn watch(&self, query: &str) -> Subscription<Vec<AnimeHistoryItem>> {
        self.repo.watch_mapped(Kind::Anime, query, AnimeHistoryItem::from)
    }

    /// The anime history list for `query`, newest first, ties broken by the higher id.
    pub fn list(&self, query: &str) -> DataResult<Vec<AnimeHistoryItem>> {
        Ok(self.repo.list(Kind::Anime, query)?.into_iter().map(AnimeHistoryItem::from).collect())
    }

    /// The history row of the episode with this url (used by the restorer). The lowest history id wins when rows
    /// repeat. `None` when no episode has that url.
    pub fn by_episode_url(c: &Connection, url: &str) -> DataResult<Option<AnimeHistory>> {
        Ok(HistoryRepo::by_item_url(c, Kind::Anime, url)?.map(AnimeHistory::from))
    }

    /// Deletes the reset anime rows (`last_seen = 0`) inside an open transaction, for the restore path.
    pub fn remove_resetted(c: &Connection) -> DataResult<usize> {
        HistoryRepo::remove_resetted(c, Kind::Anime)
    }

    /// Every history row of one anime, reset rows included.
    pub fn by_anime_id(&self, anime_id: i64) -> DataResult<Vec<AnimeHistory>> {
        Ok(self.repo.by_entry_id(anime_id)?.into_iter().map(AnimeHistory::from).collect())
    }

    /// Sets `last_seen = 0` on one row, logging a failure.
    pub fn reset_by_id(&self, history_id: i64) -> bool {
        logged("anime reset_by_id", self.try_reset_by_id(history_id))
    }

    pub fn try_reset_by_id(&self, history_id: i64) -> DataResult<()> {
        self.repo.reset_by_id(history_id)
    }

    /// Resets every history row of one anime, logging a failure.
    pub fn reset_by_anime_id(&self, anime_id: i64) -> bool {
        logged("anime reset_by_anime_id", self.try_reset_by_anime_id(anime_id))
    }

    pub fn try_reset_by_anime_id(&self, anime_id: i64) -> DataResult<()> {
        self.repo.reset_by_entry_id(anime_id)
    }

    /// Deletes every anime history row. Returns `true` on success; a failure is logged.
    pub fn delete_all(&self) -> bool {
        logged("anime delete_all", self.try_delete_all())
    }

    pub fn try_delete_all(&self) -> DataResult<()> {
        self.repo.delete_all(Kind::Anime)
    }

    /// Records that `episode_id` was seen at `seen_at`. An older row is overwritten, as Kotlin does. A failure is
    /// logged and not returned.
    pub fn upsert(&self, episode_id: i64, seen_at: i64) -> bool {
        logged("anime upsert", self.try_upsert(episode_id, seen_at))
    }

    pub fn try_upsert(&self, episode_id: i64, seen_at: i64) -> DataResult<()> {
        self.repo.upsert(episode_id, seen_at, 0)
    }
}

/// Logs a failed write (Kotlin catches and logs these) and reports whether it succeeded.
fn logged(op: &str, r: DataResult<()>) -> bool {
    match r {
        Ok(()) => true,
        Err(e) => {
            tracing::error!(operation = op, error = %e, "history write failed");
            false
        }
    }
}

/// The session timer of one open chapter or episode (`ReaderViewModel.restartReadTimer` and `flushReadTimer`).
///
/// Lifecycle, as the hooks drive it: `start` when the reader shows a chapter (window shown, or a new chapter
/// loads after a flush); `flush` when the window is hidden, or before a new chapter loads. A flush writes one
/// upsert with the time since `start`, and clears `start`. In incognito a flush writes nothing and keeps
/// `start`, as `ReaderViewModel.updateHistory` returns early before it clears the start time.
///
/// The reader and player lanes own the hooks; this type holds only the state machine.
#[derive(Debug)]
pub struct HistoryRecorder {
    started_at: Option<i64>,
    incognito: Arc<AtomicBool>,
}

impl HistoryRecorder {
    pub fn new(incognito: Arc<AtomicBool>) -> Self {
        Self { started_at: None, incognito }
    }

    /// Starts the session at `now_ms`.
    pub fn start(&mut self, now_ms: i64) {
        self.started_at = Some(now_ms);
    }

    /// Ends the session for `chapter_id` at `now_ms`. Stores `max(0, now - start)` milliseconds of read time, or 0
    /// when no session was started. Clock steps backwards therefore never subtract time.
    pub fn flush(&mut self, repo: &MangaHistoryRepo, chapter_id: i64, now_ms: i64) {
        if self.incognito.load(Ordering::Relaxed) {
            return;
        }
        let duration = self.started_at.map_or(0, |t0| (now_ms - t0).max(0));
        repo.upsert(chapter_id, now_ms, duration);
        self.started_at = None;
    }

    /// Records an anime episode as seen at `now_ms`. No duration is kept for anime. Does nothing in incognito.
    pub fn record_episode(&self, repo: &AnimeHistoryRepo, episode_id: i64, now_ms: i64) {
        if self.incognito.load(Ordering::Relaxed) {
            return;
        }
        repo.upsert(episode_id, now_ms);
    }

    /// The start of the open session, if one is running.
    pub fn started_at(&self) -> Option<i64> {
        self.started_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::{test_db, insert_anime, insert_chapter, insert_episode, insert_manga};

    fn repo_with_chapters(titles: &[(&str, &[&str])]) -> (tempfile::TempDir, Arc<Db>, Vec<(i64, i64)>) {
        let (dir, db) = test_db();
        let mut ids = Vec::new();
        for (title, urls) in titles {
            let m = insert_manga(&db, title, true);
            for url in urls.iter() {
                ids.push((m, insert_chapter(&db, m, url)));
            }
        }
        (dir, db, ids)
    }

    /// The five-row scenario of `p1-topic-history-and-updates-views.md` section 1 with two manga.
    #[test]
    fn list_shows_the_latest_chapter_per_manga_and_follows_resets() {
        let (_d, db, ids) = repo_with_chapters(&[("Alpha", &["a1", "a2"]), ("Beta", &["b1"])]);
        let (a1, a2, b1) = (ids[0].1, ids[1].1, ids[2].1);
        let repo = MangaHistoryRepo::new(db.clone());
        assert!(repo.upsert(a1, 100, 0));
        assert!(repo.upsert(a2, 300, 0));
        assert!(repo.upsert(b1, 200, 0));
        let listed = repo.list("").unwrap();
        // Alpha shows a2 (readAt 300), Beta shows b1 (readAt 200): newest first.
        assert_eq!(listed.iter().map(|h| (h.title.as_str(), h.read_at)).collect::<Vec<_>>(), [("Alpha", 300), ("Beta", 200)]);
        assert_eq!(listed[0].chapter_id, a2);

        // Resetting a2 reveals a1 for Alpha.
        let a2_history = repo.by_manga_id(ids[0].0).unwrap().into_iter().find(|h| h.chapter_id == a2).unwrap();
        assert!(repo.reset_by_id(a2_history.id));
        let after = repo.list("").unwrap();
        assert_eq!(after.iter().find(|h| h.title == "Alpha").unwrap().chapter_id, a1);

        // Resetting every row of Beta hides it, but keeps its time.
        assert!(repo.reset_by_manga_id(ids[2].0));
        let hidden = repo.list("").unwrap();
        assert!(hidden.iter().all(|h| h.title != "Beta"));
        assert_eq!(repo.by_manga_id(ids[2].0).unwrap().len(), 1);
    }

    #[test]
    fn latest_equals_the_first_list_row_and_is_none_when_empty() {
        let (_d, db, ids) = repo_with_chapters(&[("Alpha", &["a1"]), ("Beta", &["b1"])]);
        let repo = MangaHistoryRepo::new(db.clone());
        assert_eq!(repo.latest().unwrap(), None);
        repo.upsert(ids[0].1, 10, 0);
        repo.upsert(ids[1].1, 20, 0);
        assert_eq!(repo.latest().unwrap(), repo.list("").unwrap().into_iter().next());
    }

    #[test]
    fn equal_read_times_are_ordered_by_higher_id_first() {
        let (_d, db, ids) = repo_with_chapters(&[("Alpha", &["a1"]), ("Beta", &["b1"])]);
        let repo = MangaHistoryRepo::new(db.clone());
        repo.upsert(ids[0].1, 50, 0);
        repo.upsert(ids[1].1, 50, 0);
        let listed = repo.list("").unwrap();
        assert!(listed[0].id > listed[1].id);
    }

    #[test]
    fn search_folds_unicode_case_and_matches_percent_literally() {
        let (_d, db, ids) = repo_with_chapters(&[("Éclair", &["e1"]), ("Plain", &["p1"]), ("100% Pure", &["x1"])]);
        let repo = MangaHistoryRepo::new(db.clone());
        for (_, c) in &ids {
            repo.upsert(*c, 10, 0);
        }
        let found: Vec<String> = repo.list("éclair").unwrap().into_iter().map(|h| h.title).collect();
        assert_eq!(found, ["Éclair"]);
        let literal: Vec<String> = repo.list("100%").unwrap().into_iter().map(|h| h.title).collect();
        assert_eq!(literal, ["100% Pure"]);
    }

    #[test]
    fn empty_query_returns_every_entry() {
        let (_d, db, ids) = repo_with_chapters(&[("Alpha", &["a1"]), ("Beta", &["b1"])]);
        let repo = MangaHistoryRepo::new(db.clone());
        for (_, c) in &ids {
            repo.upsert(*c, 10, 0);
        }
        assert_eq!(repo.list("").unwrap().len(), 2);
    }

    #[test]
    fn list_rows_carry_the_manga_favorite_flag() {
        let (_d, db) = test_db();
        let fav = insert_manga(&db, "Fav", true);
        let not = insert_manga(&db, "Not", false);
        let repo = MangaHistoryRepo::new(db.clone());
        repo.upsert(insert_chapter(&db, fav, "f"), 10, 0);
        repo.upsert(insert_chapter(&db, not, "n"), 20, 0);
        let listed = repo.list("").unwrap();
        assert_eq!(listed.iter().map(|h| (h.title.as_str(), h.favorite)).collect::<Vec<_>>(), [("Not", false), ("Fav", true)]);
    }

    /// The worked example of section 3: upserts accumulate time, resets keep it, a new read adds to it.
    #[test]
    fn upsert_accumulates_read_time_across_reset_and_reread() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let c10 = insert_chapter(&db, m, "c10");
        let repo = MangaHistoryRepo::new(db.clone());
        repo.upsert(c10, 61_000, 60_000);
        repo.upsert(c10, 200_000, 30_000);
        let row = repo.by_manga_id(m).unwrap().remove(0);
        assert_eq!((row.time_read, row.last_read), (90_000, 200_000));
        repo.reset_by_id(row.id);
        let reset = repo.by_manga_id(m).unwrap().remove(0);
        assert_eq!((reset.last_read, reset.time_read), (0, 90_000));
        assert_eq!(repo.total_read_duration().unwrap(), 90_000);
        repo.upsert(c10, 300_000, 5_000);
        let reread = repo.by_manga_id(m).unwrap().remove(0);
        assert_eq!((reread.last_read, reread.time_read), (300_000, 95_000));
    }

    #[test]
    fn reset_by_manga_id_touches_only_that_manga_rows() {
        let (_d, db) = test_db();
        let a = insert_manga(&db, "a", false);
        let b = insert_manga(&db, "b", false);
        let repo = MangaHistoryRepo::new(db.clone());
        repo.upsert(insert_chapter(&db, a, "a1"), 10, 100);
        repo.upsert(insert_chapter(&db, b, "b1"), 20, 200);
        assert!(repo.reset_by_manga_id(a));
        assert_eq!(repo.by_manga_id(a).unwrap()[0].last_read, 0);
        assert_eq!(repo.by_manga_id(b).unwrap()[0].last_read, 20);
        assert_eq!(repo.total_read_duration().unwrap(), 300, "reset rows keep their time");
    }

    #[test]
    fn delete_all_removes_rows_and_time_and_reports_success() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = MangaHistoryRepo::new(db.clone());
        repo.upsert(insert_chapter(&db, m, "c"), 10, 500);
        assert!(repo.delete_all());
        assert_eq!(repo.total_read_duration().unwrap(), 0);
        assert!(repo.by_manga_id(m).unwrap().is_empty());
    }

    #[test]
    fn a_failing_upsert_is_logged_and_does_not_propagate() {
        let (_d, db) = test_db();
        let repo = MangaHistoryRepo::new(db.clone());
        // Chapter 999 does not exist: the foreign key fails and the call returns normally.
        assert!(!repo.upsert(999, 10, 0));
        assert!(repo.try_upsert(999, 10, 0).is_err());
        assert!(repo.list("").unwrap().is_empty());
    }

    #[test]
    fn a_manga_history_watch_re_emits_after_upsert_reset_and_chapter_delete() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let chapter = insert_chapter(&db, m, "c");
        let repo = MangaHistoryRepo::new(db.clone());
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut sub = repo.watch("");
            assert!(sub.changed().await.unwrap().unwrap().is_empty());
            repo.upsert(chapter, 10, 0);
            assert_eq!(sub.changed().await.unwrap().unwrap().len(), 1);
            repo.reset_by_manga_id(m);
            assert!(sub.changed().await.unwrap().unwrap().is_empty());
            repo.try_upsert(chapter, 20, 0).unwrap();
            assert_eq!(sub.changed().await.unwrap().unwrap().len(), 1);
            db.write(|tx| Ok(tx.execute("DELETE FROM items WHERE _id = ?1", [chapter]).map(drop)?)).unwrap();
            assert!(sub.changed().await.unwrap().unwrap().is_empty(), "cascade removes the history row");
        });
    }

    #[test]
    fn delete_all_notifies_a_history_watch() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let chapter = insert_chapter(&db, m, "c");
        let repo = MangaHistoryRepo::new(db.clone());
        repo.upsert(chapter, 10, 0);
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut sub = repo.watch("");
            assert_eq!(sub.changed().await.unwrap().unwrap().len(), 1);
            assert!(repo.delete_all());
            assert!(sub.changed().await.unwrap().unwrap().is_empty(), "truncate-style delete still notifies");
        });
    }

    #[test]
    fn anime_upsert_overwrites_last_seen_even_when_older() {
        let (_d, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let e = insert_episode(&db, a, "e1");
        let repo = AnimeHistoryRepo::new(db.clone());
        assert!(repo.upsert(e, 500));
        assert!(repo.upsert(e, 200));
        let rows = repo.by_anime_id(a).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].last_seen, Some(200));
    }

    #[test]
    fn anime_reset_keeps_the_row_and_delete_all_removes_it() {
        let (_d, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let e = insert_episode(&db, a, "e1");
        let repo = AnimeHistoryRepo::new(db.clone());
        repo.upsert(e, 500);
        let id = repo.by_anime_id(a).unwrap()[0].id;
        assert!(repo.reset_by_id(id));
        assert_eq!(repo.by_anime_id(a).unwrap()[0].last_seen, Some(0));
        assert!(repo.reset_by_anime_id(a));
        assert!(repo.delete_all());
        assert!(repo.by_anime_id(a).unwrap().is_empty());
    }

    #[test]
    fn anime_list_is_newest_first() {
        let (_d, db) = test_db();
        let a = insert_anime(&db, "Show", false);
        let b = insert_anime(&db, "Other", false);
        let repo = AnimeHistoryRepo::new(db.clone());
        repo.upsert(insert_episode(&db, a, "a1"), 100);
        repo.upsert(insert_episode(&db, b, "b1"), 300);
        let listed = repo.list("").unwrap();
        assert_eq!(listed.iter().map(|h| h.seen_at.0).collect::<Vec<_>>(), [300, 100]);
    }

    #[test]
    fn recorder_stores_elapsed_time_then_zero_and_clamps_negative_spans() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let chapter = insert_chapter(&db, m, "c");
        let repo = MangaHistoryRepo::new(db.clone());
        let mut rec = HistoryRecorder::new(Arc::new(AtomicBool::new(false)));
        rec.start(1_000);
        rec.flush(&repo, chapter, 61_000);
        assert_eq!(rec.started_at(), None);
        assert_eq!(repo.by_manga_id(m).unwrap()[0].time_read, 60_000);
        rec.flush(&repo, chapter, 62_000);
        assert_eq!(repo.by_manga_id(m).unwrap()[0].time_read, 60_000, "a flush without start adds nothing");
        rec.start(100_000);
        rec.flush(&repo, chapter, 90_000);
        assert_eq!(repo.by_manga_id(m).unwrap()[0].time_read, 60_000, "a backwards clock adds nothing");
    }

    #[test]
    fn recorder_writes_nothing_in_incognito_and_keeps_its_start() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let chapter = insert_chapter(&db, m, "c");
        let repo = MangaHistoryRepo::new(db.clone());
        let incognito = Arc::new(AtomicBool::new(true));
        let mut rec = HistoryRecorder::new(Arc::clone(&incognito));
        rec.start(0);
        rec.flush(&repo, chapter, 5_000);
        assert!(repo.by_manga_id(m).unwrap().is_empty());
        assert_eq!(rec.started_at(), Some(0));
        incognito.store(false, Ordering::Relaxed);
        rec.flush(&repo, chapter, 7_000);
        assert_eq!(repo.by_manga_id(m).unwrap()[0].time_read, 7_000);
    }

    #[test]
    fn reader_hooks_flush_then_restart_when_a_new_chapter_loads() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let first = insert_chapter(&db, m, "first");
        let second = insert_chapter(&db, m, "second");
        let repo = MangaHistoryRepo::new(db.clone());
        let mut rec = HistoryRecorder::new(Arc::new(AtomicBool::new(false)));
        rec.start(0);
        // Loading the second chapter at t=40 s flushes the first, then restarts the timer.
        rec.flush(&repo, first, 40_000);
        rec.start(40_000);
        rec.flush(&repo, second, 100_000);
        let rows = repo.by_manga_id(m).unwrap();
        let time_of = |c: i64| rows.iter().find(|h| h.chapter_id == c).unwrap().time_read;
        assert_eq!((time_of(first), time_of(second)), (40_000, 60_000));
    }

    #[test]
    fn record_episode_writes_last_seen_and_nothing_in_incognito() {
        let (_d, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let e = insert_episode(&db, a, "e");
        let repo = AnimeHistoryRepo::new(db.clone());
        let rec = HistoryRecorder::new(Arc::new(AtomicBool::new(true)));
        rec.record_episode(&repo, e, 10);
        assert!(repo.by_anime_id(a).unwrap().is_empty());
        let open = HistoryRecorder::new(Arc::new(AtomicBool::new(false)));
        open.record_episode(&repo, e, 10);
        assert_eq!(repo.by_anime_id(a).unwrap()[0].last_seen, Some(10));
    }
}

#[cfg(test)]
mod maintenance_tests {
    use super::*;
    use crate::support::{test_db, insert_anime, insert_chapter, insert_episode, insert_manga};

    #[test]
    fn remove_resetted_deletes_only_reset_manga_rows_and_returns_the_count() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let (a, b) = (insert_chapter(&db, m, "a"), insert_chapter(&db, m, "b"));
        let repo = MangaHistoryRepo::new(db.clone());
        assert!(repo.upsert(a, 100, 5));
        assert!(repo.upsert(b, 200, 5));
        let a_id = repo.by_manga_id(m).unwrap().into_iter().find(|h| h.chapter_id == a).unwrap().id;
        assert!(repo.reset_by_id(a_id));
        let removed = db.write(|tx| Ok(MangaHistoryRepo::remove_resetted(tx)?)).unwrap();
        assert_eq!(removed, 1);
        let left = repo.by_manga_id(m).unwrap();
        assert_eq!(left.iter().map(|h| h.chapter_id).collect::<Vec<_>>(), [b]);
        assert_eq!(db.write(|tx| Ok(MangaHistoryRepo::remove_resetted(tx)?)).unwrap(), 0);
    }

    #[test]
    fn by_chapter_url_returns_the_row_of_that_url_or_none() {
        let (_d, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let a = insert_chapter(&db, m, "a");
        let repo = MangaHistoryRepo::new(db.clone());
        assert!(repo.upsert(a, 100, 0));
        let found = db.read(|c| Ok(MangaHistoryRepo::by_chapter_url(c, "a")?)).unwrap().expect("row for a");
        assert_eq!((found.chapter_id, found.last_read), (a, 100));
        assert!(db.read(|c| Ok(MangaHistoryRepo::by_chapter_url(c, "missing")?)).unwrap().is_none());
    }

    #[test]
    fn anime_remove_resetted_deletes_only_reset_rows_and_by_episode_url_finds_the_row() {
        let (_d, db) = test_db();
        let a = insert_anime(&db, "Show", false);
        let (e1, e2) = (insert_episode(&db, a, "e1"), insert_episode(&db, a, "e2"));
        let repo = AnimeHistoryRepo::new(db.clone());
        assert!(repo.upsert(e1, 100));
        assert!(repo.upsert(e2, 200));
        let e1_id = repo.by_anime_id(a).unwrap().into_iter().find(|h| h.episode_id == e1).unwrap().id;
        assert!(repo.reset_by_id(e1_id));
        let found = db.read(|c| Ok(AnimeHistoryRepo::by_episode_url(c, "e1")?)).unwrap().expect("row for e1");
        assert_eq!((found.episode_id, found.last_seen), (e1, Some(0)));
        assert!(db.read(|c| Ok(AnimeHistoryRepo::by_episode_url(c, "nope")?)).unwrap().is_none());
        assert_eq!(db.write(|tx| Ok(AnimeHistoryRepo::remove_resetted(tx)?)).unwrap(), 1);
        let left = repo.by_anime_id(a).unwrap();
        assert_eq!(left.iter().map(|h| h.episode_id).collect::<Vec<_>>(), [e2]);
    }
}
