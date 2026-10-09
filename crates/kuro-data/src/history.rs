//! Read history (`MangaHistoryRepositoryImpl.kt`, `AnimeHistoryRepositoryImpl.kt`): the history list and its
//! search (T-06a-012), the per-chapter and per-episode repositories (T-06a-010, T-06a-011), and the session
//! timer that writes them (`HistoryRecorder`, T-06a-014).
//!
//! The `*_repo` write methods that Kotlin makes infallible log their failure and return `false` or nothing;
//! their `try_*` twins return the error for callers and tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kuro_db::handle::Db;
use kuro_db::models::{HistoryRow, HistoryViewRow};
use kuro_db::schema_anime::Millis;
use kuro_db::statements_anime as anime;
use kuro_db::statements_manga::{history as manga_history, views as manga_views};
use kuro_db::subscribe::Subscription;
use rusqlite::{Connection, named_params};

use crate::{DataResult, ENTRY_DEBOUNCE};

/// Tables a manga history list reads.
const MANGA_HISTORY_TABLES: &[&str] = &["history", "chapters", "mangas"];

/// Tables an anime history list reads.
const ANIME_HISTORY_TABLES: &[&str] = &["animehistory", "episodes", "animes"];

/// The newest history row of each manga, as the history screen lists it. The columns of `historyView`.
const HISTORY_COLUMNS: &str = "id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, \
     chapterNumber, readAt, readDuration";

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

impl From<&HistoryRow> for MangaHistory {
    fn from(r: &HistoryRow) -> Self {
        Self { id: r.id, chapter_id: r.chapter_id, last_read: r.last_read, time_read: r.time_read }
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

impl From<HistoryViewRow> for HistoryItem {
    fn from(r: HistoryViewRow) -> Self {
        Self {
            id: r.id,
            manga_id: r.manga_id,
            chapter_id: r.chapter_id,
            title: r.title,
            thumbnail_url: r.thumbnail_url,
            source: r.source,
            favorite: r.favorite,
            cover_last_modified: r.cover_last_modified,
            chapter_number: r.chapter_number,
            read_at: r.read_at,
            read_duration: r.read_duration,
        }
    }
}

/// `HISTORY_SEARCH_PARITY=1` restores Kotlin's search: the text goes to `LIKE` as typed, so `%` and `_` act
/// as wildcards and only ASCII letters fold case.
fn parity_search() -> bool {
    std::env::var("HISTORY_SEARCH_PARITY").is_ok_and(|v| v == "1")
}

/// The manga history list for `query`, newest read first, ties broken by the higher id.
///
/// Default search: a substring match on the title with Unicode case folding (`str::to_lowercase`), so
/// "éclair" finds "Éclair" and "100%" matches literally. Parity search passes the raw text to `LIKE`.
pub(crate) fn manga_list(c: &Connection, query: &str, parity: bool) -> DataResult<Vec<HistoryItem>> {
    let mut rows = if parity {
        let mut st = c.prepare_cached(manga_views::HISTORY)?;
        st.query_map(named_params! {":query": query}, HistoryViewRow::from_row)?.collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        let needle = query.to_lowercase();
        manga_views::history(c, "")?.into_iter().filter(|r| r.title.to_lowercase().contains(&needle)).collect()
    };
    rows.sort_by(|a, b| b.read_at.cmp(&a.read_at).then(b.id.cmp(&a.id)));
    Ok(rows.into_iter().map(HistoryItem::from).collect())
}

/// The newest history row with `readAt > 0`, or `None`. Ties broken by the higher id, so it equals the first
/// row of [`manga_list`] with an empty query.
fn manga_latest(c: &Connection) -> DataResult<Option<HistoryItem>> {
    let sql = format!("SELECT {HISTORY_COLUMNS} FROM historyView WHERE readAt > 0 ORDER BY readAt DESC, id DESC LIMIT 1");
    let mut st = c.prepare_cached(&sql)?;
    let mut rows = st.query_map([], HistoryViewRow::from_row)?;
    Ok(rows.next().transpose()?.map(HistoryItem::from))
}

/// Manga history access (`MangaHistoryRepositoryImpl.kt`).
#[derive(Clone)]
pub struct MangaHistoryRepo {
    db: Arc<Db>,
}

impl MangaHistoryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The history list for `query`, published again after a change to history, chapters or mangas.
    pub fn watch(&self, query: &str) -> Subscription<Vec<HistoryItem>> {
        let query = query.to_owned();
        let parity = parity_search();
        self.db.subscribe(MANGA_HISTORY_TABLES, ENTRY_DEBOUNCE, move |c| manga_list(c, &query, parity))
    }

    /// The history list for `query` (see [`manga_list`]).
    pub fn list(&self, query: &str) -> DataResult<Vec<HistoryItem>> {
        let parity = parity_search();
        self.db.read(|c| manga_list(c, query, parity))
    }

    /// The row with the greatest `readAt > 0`, or `None` when nothing was read.
    pub fn latest(&self) -> DataResult<Option<HistoryItem>> {
        self.db.read(manga_latest)
    }

    /// Accumulated read time over every history row, reset rows included. Zero when there are none.
    pub fn total_read_duration(&self) -> DataResult<i64> {
        self.db.read(|c| Ok(manga_history::get_read_duration(c)?))
    }

    /// Every history row of one manga, reset rows included.
    pub fn by_manga_id(&self, manga_id: i64) -> DataResult<Vec<MangaHistory>> {
        let rows = self.db.read(|c| Ok(manga_history::get_by_manga_id(c, manga_id)?))?;
        Ok(rows.iter().map(MangaHistory::from).collect())
    }

    /// The history row of the chapter with this url, as the backup restorer reads it.
    pub fn by_chapter_url(c: &Connection, url: &str) -> DataResult<Option<MangaHistory>> {
        Ok(manga_history::get_by_chapter_url(c, url)?.as_ref().map(MangaHistory::from))
    }

    /// Sets `last_read = 0` on one row, logging a failure.
    pub fn reset_by_id(&self, history_id: i64) -> bool {
        logged("reset_by_id", self.try_reset_by_id(history_id))
    }

    pub fn try_reset_by_id(&self, history_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(manga_history::reset_by_id(tx, history_id).map(drop)?))
    }

    /// Resets every history row of one manga, logging a failure.
    pub fn reset_by_manga_id(&self, manga_id: i64) -> bool {
        logged("reset_by_manga_id", self.try_reset_by_manga_id(manga_id))
    }

    pub fn try_reset_by_manga_id(&self, manga_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(manga_history::reset_by_manga_id(tx, manga_id).map(drop)?))
    }

    /// Deletes every history row, time included. Returns `true` on success; a failure is logged.
    pub fn delete_all(&self) -> bool {
        logged("delete_all", self.try_delete_all())
    }

    pub fn try_delete_all(&self) -> DataResult<()> {
        self.db.write(|tx| Ok(manga_history::remove_all(tx).map(drop)?))
    }

    /// Records a read of `chapter_id`: sets `last_read` to `read_at` and adds `duration` to `time_read`.
    /// A failure (for example a chapter that does not exist) is logged and not returned.
    pub fn upsert(&self, chapter_id: i64, read_at: i64, duration: i64) -> bool {
        logged("upsert", self.try_upsert(chapter_id, read_at, duration))
    }

    pub fn try_upsert(&self, chapter_id: i64, read_at: i64, duration: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(manga_history::upsert(tx, chapter_id, read_at, duration).map(drop)?))
    }

    /// Deletes the reset rows (`last_read = 0`) inside an open transaction, for the restore path.
    pub fn remove_resetted(c: &Connection) -> DataResult<usize> {
        Ok(manga_history::remove_resetted(c)?)
    }
}

/// One `animehistory` row, as the anime history list shows it (the `animehistoryView` columns).
pub type AnimeHistoryItem = anime::HistoryEntry;

/// Anime history access (`AnimeHistoryRepositoryImpl.kt`). Same as the manga side, without durations.
#[derive(Clone)]
pub struct AnimeHistoryRepo {
    db: Arc<Db>,
}

/// One `animehistory` row of an episode.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimeHistory {
    pub id: i64,
    pub episode_id: i64,
    /// Epoch milliseconds of the last view; `None` or zero after a reset.
    pub last_seen: Option<i64>,
}

impl AnimeHistoryRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The anime history list for `query`, published again after a change to history, episodes or animes.
    pub fn watch(&self, query: &str) -> Subscription<Vec<AnimeHistoryItem>> {
        let query = query.to_owned();
        self.db.subscribe(ANIME_HISTORY_TABLES, ENTRY_DEBOUNCE, move |c| anime_list(c, &query))
    }

    /// The anime history list for `query`, newest first, ties broken by the higher id.
    pub fn list(&self, query: &str) -> DataResult<Vec<AnimeHistoryItem>> {
        self.db.read(|c| anime_list(c, query))
    }

    /// The history row of the episode with this url (`AnimeHistory.kt` lookup by episode url, used by the restorer).
    /// The lowest history id wins when rows repeat. `None` when no episode has that url.
    pub fn by_episode_url(c: &Connection, url: &str) -> DataResult<Option<AnimeHistory>> {
        let mut st = c.prepare_cached(
            "SELECT H._id, H.episode_id, H.last_seen FROM animehistory H \
             JOIN episodes E ON E._id = H.episode_id WHERE E.url = :url ORDER BY H._id LIMIT 1",
        )?;
        let mut rows = st.query_map(named_params! {":url": url}, |r| {
            Ok(AnimeHistory { id: r.get(0)?, episode_id: r.get(1)?, last_seen: r.get(2)? })
        })?;
        Ok(rows.next().transpose()?)
    }

    /// Deletes the reset anime rows (`last_seen = 0`) inside an open transaction, for the restore path.
    pub fn remove_resetted(c: &Connection) -> DataResult<usize> {
        Ok(anime::remove_resetted_history(c)?)
    }

    /// Every history row of one anime, reset rows included.
    pub fn by_anime_id(&self, anime_id: i64) -> DataResult<Vec<AnimeHistory>> {
        self.db.read(|c| {
            let mut st = c.prepare_cached(
                "SELECT H._id, H.episode_id, H.last_seen FROM animehistory H \
                 JOIN episodes E ON E._id = H.episode_id WHERE E.anime_id = :animeId ORDER BY H._id",
            )?;
            let rows = st.query_map(named_params! {":animeId": anime_id}, |r| {
                Ok(AnimeHistory { id: r.get(0)?, episode_id: r.get(1)?, last_seen: r.get(2)? })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Sets `last_seen = 0` on one row, logging a failure.
    pub fn reset_by_id(&self, history_id: i64) -> bool {
        logged("anime reset_by_id", self.try_reset_by_id(history_id))
    }

    pub fn try_reset_by_id(&self, history_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(anime::reset_history(tx, history_id).map(drop)?))
    }

    /// Resets every history row of one anime, logging a failure.
    pub fn reset_by_anime_id(&self, anime_id: i64) -> bool {
        logged("anime reset_by_anime_id", self.try_reset_by_anime_id(anime_id))
    }

    pub fn try_reset_by_anime_id(&self, anime_id: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(anime::reset_history_by_anime(tx, anime_id).map(drop)?))
    }

    /// Deletes every anime history row. Returns `true` on success; a failure is logged.
    pub fn delete_all(&self) -> bool {
        logged("anime delete_all", self.try_delete_all())
    }

    pub fn try_delete_all(&self) -> DataResult<()> {
        self.db.write(|tx| Ok(anime::delete_all_history(tx).map(drop)?))
    }

    /// Records that `episode_id` was seen at `seen_at`. An older row is overwritten, as Kotlin does. A failure is
    /// logged and not returned.
    pub fn upsert(&self, episode_id: i64, seen_at: i64) -> bool {
        logged("anime upsert", self.try_upsert(episode_id, seen_at))
    }

    pub fn try_upsert(&self, episode_id: i64, seen_at: i64) -> DataResult<()> {
        self.db.write(|tx| Ok(anime::upsert_history(tx, episode_id, Millis(seen_at)).map(drop)?))
    }
}

fn anime_list(c: &Connection, query: &str) -> DataResult<Vec<AnimeHistoryItem>> {
    let mut rows = anime::history(c, query)?;
    rows.sort_by(|a, b| b.seen_at.cmp(&a.seen_at).then(b.id.cmp(&a.id)));
    Ok(rows)
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
/// The reader and player lanes own the hooks; this type holds only the state machine. [`HistoryWrites`] is the shutdown wait.
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

/// The history writes that left the UI thread and have not finished yet. App shutdown calls [`drain`](Self::drain)
/// so no flushed read time is lost (T-06a-014). Must be used inside a tokio runtime.
#[derive(Debug, Default)]
pub struct HistoryWrites {
    pending: tokio::task::JoinSet<()>,
}

impl HistoryWrites {
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `write` (a repository call) on a blocking thread and tracks it until it finishes.
    pub fn spawn(&mut self, write: impl FnOnce() + Send + 'static) {
        self.pending.spawn_blocking(write);
    }

    /// Waits for every tracked write. A panicked write is logged, not propagated.
    pub async fn drain(&mut self) {
        while let Some(done) = self.pending.join_next().await {
            if let Err(e) = done {
                tracing::error!(error = %e, "history write task failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::{anime_db, insert_anime, insert_chapter, insert_episode, insert_manga, manga_db};

    fn repo_with_chapters(titles: &[(&str, &[&str])]) -> (tempfile::TempDir, Arc<Db>, Vec<(i64, i64)>) {
        let (dir, db) = manga_db();
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
        // Parity search passes the text to LIKE unescaped: `%` is a wildcard, so "100%" matches every title
        // that starts with "100", and "Éclair" stays case-sensitive for non-ASCII letters.
        let parity: Vec<String> =
            db.read(|c| manga_list(c, "100%", true)).unwrap().into_iter().map(|h| h.title).collect();
        assert_eq!(parity, ["100% Pure"]);
        let parity_ascii: Vec<String> =
            db.read(|c| manga_list(c, "PLAIN", true)).unwrap().into_iter().map(|h| h.title).collect();
        assert_eq!(parity_ascii, ["Plain"]);
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", false);
        let repo = MangaHistoryRepo::new(db.clone());
        repo.upsert(insert_chapter(&db, m, "c"), 10, 500);
        assert!(repo.delete_all());
        assert_eq!(repo.total_read_duration().unwrap(), 0);
        assert!(repo.by_manga_id(m).unwrap().is_empty());
    }

    #[test]
    fn a_failing_upsert_is_logged_and_does_not_propagate() {
        let (_d, db) = manga_db();
        let repo = MangaHistoryRepo::new(db.clone());
        // Chapter 999 does not exist: the foreign key fails and the call returns normally.
        assert!(!repo.upsert(999, 10, 0));
        assert!(repo.try_upsert(999, 10, 0).is_err());
        assert!(repo.list("").unwrap().is_empty());
    }

    #[test]
    fn a_manga_history_watch_re_emits_after_upsert_reset_and_chapter_delete() {
        let (_d, db) = manga_db();
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
            db.write(|tx| Ok(tx.execute("DELETE FROM chapters WHERE _id = ?1", [chapter]).map(drop)?)).unwrap();
            assert!(sub.changed().await.unwrap().unwrap().is_empty(), "cascade removes the history row");
        });
    }

    #[test]
    fn delete_all_notifies_a_history_watch() {
        let (_d, db) = manga_db();
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
        let (_d, db) = anime_db();
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
        let (_d, db) = anime_db();
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
        let (_d, db) = anime_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = anime_db();
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

    #[tokio::test(flavor = "multi_thread")]
    async fn drain_waits_for_every_outstanding_write() {
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", false);
        let first = insert_chapter(&db, m, "first");
        let second = insert_chapter(&db, m, "second");
        let repo = MangaHistoryRepo::new(db.clone());
        let mut writes = HistoryWrites::new();
        let slow = MangaHistoryRepo::new(db.clone());
        writes.spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            slow.upsert(first, 1, 10);
        });
        let fast = MangaHistoryRepo::new(db.clone());
        writes.spawn(move || {
            fast.upsert(second, 2, 20);
        });
        writes.drain().await;
        assert_eq!(repo.by_manga_id(m).unwrap().len(), 2, "both writes finished before drain returned");
        writes.drain().await;
    }
}

#[cfg(test)]
mod maintenance_tests {
    use super::*;
    use crate::support::{anime_db, insert_anime, insert_chapter, insert_episode, insert_manga, manga_db};

    #[test]
    fn remove_resetted_deletes_only_reset_manga_rows_and_returns_the_count() {
        let (_d, db) = manga_db();
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
        let (_d, db) = manga_db();
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
        let (_d, db) = anime_db();
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
