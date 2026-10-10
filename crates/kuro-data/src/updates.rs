//! Updates repositories (`MangaUpdatesRepositoryImpl.kt`, `AnimeUpdatesRepositoryImpl.kt`, T-06a-020): the
//! recent-chapter and recent-episode feeds, read from `updatesView` and `animeupdatesView`.
//!
//! Both views already keep favourites only and `date_fetch > date_added`. The feeds add a strict
//! `date_upload > after` and an optional read or seen flag. Rows are ordered by `date_fetch DESC, id DESC`
//! and the limit applies after that order, so the rows kept are the newest by fetch date.

use std::sync::Arc;

use chrono::{DateTime, Duration, LocalResult, Months, NaiveDateTime, TimeZone};
use kuro_db::handle::Db;
use kuro_db::models::UpdateViewRow;
use kuro_db::statements_anime::AnimeUpdateRow;
use kuro_db::subscribe::Subscription;
use rusqlite::{Connection, Row, named_params};

use crate::{DataResult, ENTRY_DEBOUNCE};

/// Rows an Updates feed returns at most (`GetMangaUpdates`, `GetAnimeUpdates`).
pub const UPDATES_LIMIT: i64 = 500;

/// Tables a manga updates feed reads.
const MANGA_UPDATE_TABLES: &[&str] = &["chapters", "mangas"];

/// Tables an anime updates feed reads.
const ANIME_UPDATE_TABLES: &[&str] = &["episodes", "animes"];

/// Manga feed. A `NULL` `:read` matches every row, so one statement serves the unfiltered and filtered feeds.
const MANGA_UPDATES: &str = "SELECT * FROM updatesView \
     WHERE (:read IS NULL OR read = :read) AND dateUpload > :after \
     ORDER BY datefetch DESC, chapterId DESC LIMIT :limit";

/// Anime feed. A `NULL` `:seen` matches every row.
const ANIME_UPDATES: &str = "SELECT * FROM animeupdatesView \
     WHERE (:seen IS NULL OR seen = :seen) AND dateUpload > :after \
     ORDER BY datefetch DESC, episodeId DESC LIMIT :limit";

/// Epoch ms of the instant three months before `now`, on the same local wall clock (`ZonedDateTime.minusMonths(3)`).
/// A day missing in the target month is clamped to its last day (31 May gives 28 Feb). A local time in a DST gap
/// moves forward by the gap, one hour at a time.
pub fn three_months_ago_ms<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    let zone = now.timezone();
    let mut local = now.naive_local().checked_sub_months(Months::new(3)).unwrap_or(NaiveDateTime::MIN);
    loop {
        match zone.from_local_datetime(&local) {
            LocalResult::Single(t) => return t.timestamp_millis(),
            LocalResult::Ambiguous(earliest, _) => return earliest.timestamp_millis(),
            LocalResult::None => local += Duration::hours(1),
        }
    }
}

fn manga_feed(c: &Connection, read: Option<bool>, after: i64) -> DataResult<Vec<UpdateViewRow>> {
    let mut st = c.prepare_cached(MANGA_UPDATES)?;
    let rows = st
        .query_map(
            named_params! {":read": read, ":after": after, ":limit": UPDATES_LIMIT},
            UpdateViewRow::from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn anime_update_row(r: &Row<'_>) -> rusqlite::Result<AnimeUpdateRow> {
    Ok(AnimeUpdateRow {
        anime_id: r.get("animeId")?,
        anime_title: r.get("animeTitle")?,
        episode_id: r.get("episodeId")?,
        episode_name: r.get("episodeName")?,
        scanlator: r.get("scanlator")?,
        seen: r.get("seen")?,
        bookmark: r.get("bookmark")?,
        fillermark: r.get("fillermark")?,
        last_second_seen: r.get("last_second_seen")?,
        total_seconds: r.get("totalSeconds")?,
        source: r.get("source")?,
        favorite: r.get("favorite")?,
        thumbnail_url: r.get("thumbnailUrl")?,
        cover_last_modified: r.get("coverLastModified")?,
        date_upload: r.get("dateUpload")?,
        date_fetch: r.get("datefetch")?,
    })
}

fn anime_feed(c: &Connection, seen: Option<bool>, after: i64) -> DataResult<Vec<AnimeUpdateRow>> {
    let mut st = c.prepare_cached(ANIME_UPDATES)?;
    let rows = st
        .query_map(named_params! {":seen": seen, ":after": after, ":limit": UPDATES_LIMIT}, anime_update_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Manga chapters of library entries fetched after they were added (`MangaUpdatesRepositoryImpl.kt`).
#[derive(Clone)]
pub struct MangaUpdatesRepo {
    db: Arc<Db>,
}

impl MangaUpdatesRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The recent feed (any read state) with `date_upload > after_ms`, republished after a change to a
    /// chapter's read flag or to a manga.
    pub fn watch_recent(&self, after_ms: i64) -> Subscription<Vec<UpdateViewRow>> {
        self.db.subscribe(MANGA_UPDATE_TABLES, ENTRY_DEBOUNCE, move |c| manga_feed(c, None, after_ms))
    }

    /// The recent feed restricted to chapters whose read flag equals `read`.
    pub fn watch_with_read(&self, read: bool, after_ms: i64) -> Subscription<Vec<UpdateViewRow>> {
        self.db.subscribe(MANGA_UPDATE_TABLES, ENTRY_DEBOUNCE, move |c| manga_feed(c, Some(read), after_ms))
    }

    /// The recent feed (any read state), as one read.
    pub fn recent(&self, after_ms: i64) -> DataResult<Vec<UpdateViewRow>> {
        self.db.read(|c| manga_feed(c, None, after_ms))
    }

    /// The recent feed restricted to chapters whose read flag equals `read`, as one read.
    pub fn with_read(&self, read: bool, after_ms: i64) -> DataResult<Vec<UpdateViewRow>> {
        self.db.read(|c| manga_feed(c, Some(read), after_ms))
    }
}

/// Anime episodes of library entries fetched after they were added (`AnimeUpdatesRepositoryImpl.kt`).
#[derive(Clone)]
pub struct AnimeUpdatesRepo {
    db: Arc<Db>,
}

impl AnimeUpdatesRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The recent feed (any seen state) with `date_upload > after_ms`.
    pub fn watch_recent(&self, after_ms: i64) -> Subscription<Vec<AnimeUpdateRow>> {
        self.db.subscribe(ANIME_UPDATE_TABLES, ENTRY_DEBOUNCE, move |c| anime_feed(c, None, after_ms))
    }

    /// The recent feed restricted to episodes whose seen flag equals `seen`.
    pub fn watch_with_seen(&self, seen: bool, after_ms: i64) -> Subscription<Vec<AnimeUpdateRow>> {
        self.db.subscribe(ANIME_UPDATE_TABLES, ENTRY_DEBOUNCE, move |c| anime_feed(c, Some(seen), after_ms))
    }

    /// The recent feed (any seen state), as one read.
    pub fn recent(&self, after_ms: i64) -> DataResult<Vec<AnimeUpdateRow>> {
        self.db.read(|c| anime_feed(c, None, after_ms))
    }

    /// The recent feed restricted to episodes whose seen flag equals `seen`, as one read.
    pub fn with_seen(&self, seen: bool, after_ms: i64) -> DataResult<Vec<AnimeUpdateRow>> {
        self.db.read(|c| anime_feed(c, Some(seen), after_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::{anime_db, insert_anime, insert_chapter, insert_episode, insert_manga, manga_db};
    use chrono::{FixedOffset, NaiveDate, Utc};
    use rusqlite::params;

    /// Sets the upload and fetch dates of one chapter.
    fn set_chapter_dates(db: &Arc<Db>, id: i64, upload: i64, fetch: i64) {
        db.write(|tx| {
            tx.execute("UPDATE chapters SET date_upload = ?1, date_fetch = ?2 WHERE _id = ?3", params![upload, fetch, id])?;
            Ok(())
        })
        .unwrap();
    }

    fn set_chapter_read(db: &Arc<Db>, id: i64, read: bool) {
        db.write(|tx| {
            tx.execute("UPDATE chapters SET read = ?1 WHERE _id = ?2", params![read, id])?;
            Ok(())
        })
        .unwrap();
    }

    fn set_episode_dates(db: &Arc<Db>, id: i64, upload: i64, fetch: i64) {
        db.write(|tx| {
            tx.execute("UPDATE episodes SET date_upload = ?1, date_fetch = ?2 WHERE _id = ?3", params![upload, fetch, id])?;
            Ok(())
        })
        .unwrap();
    }

    fn ids(rows: &[UpdateViewRow]) -> Vec<i64> {
        rows.iter().map(|r| r.chapter_id).collect()
    }

    /// U1: a chapter with `date_upload` 0 is never listed, whatever the window.
    #[test]
    fn u1_zero_upload_date_is_excluded() {
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", true);
        let zero = insert_chapter(&db, m, "zero");
        let real = insert_chapter(&db, m, "real");
        set_chapter_dates(&db, zero, 0, 100);
        set_chapter_dates(&db, real, 5, 100);
        let repo = MangaUpdatesRepo::new(db.clone());
        assert_eq!(ids(&repo.recent(0).unwrap()), vec![real]);
    }

    /// U2: both comparisons are strict. An upload equal to `after` is out, one millisecond later is in, and a
    /// fetch equal to `date_added` is out (the view condition).
    #[test]
    fn u2_comparisons_are_strict() {
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", true);
        let equal = insert_chapter(&db, m, "equal");
        let later = insert_chapter(&db, m, "later");
        set_chapter_dates(&db, equal, 1000, 2000);
        set_chapter_dates(&db, later, 1001, 2000);
        let repo = MangaUpdatesRepo::new(db.clone());
        assert_eq!(ids(&repo.recent(1000).unwrap()), vec![later]);
    }

    /// U3: chapters of non-favourite manga are never listed.
    #[test]
    fn u3_only_favourites_are_listed() {
        let (_d, db) = manga_db();
        let fav = insert_manga(&db, "fav", true);
        let other = insert_manga(&db, "other", false);
        let kept = insert_chapter(&db, fav, "kept");
        let hidden = insert_chapter(&db, other, "hidden");
        set_chapter_dates(&db, kept, 10, 10);
        set_chapter_dates(&db, hidden, 10, 10);
        let repo = MangaUpdatesRepo::new(db.clone());
        assert_eq!(ids(&repo.recent(0).unwrap()), vec![kept]);
    }

    /// U4: the limit keeps the newest rows by fetch date, not the first rows by id.
    #[test]
    fn u4_limit_keeps_the_newest_by_fetch_date() {
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", true);
        let mut first_ids = Vec::new();
        for i in 0..(UPDATES_LIMIT + 2) {
            let id = insert_chapter(&db, m, &format!("c{i}"));
            // The oldest fetch dates belong to the first inserted rows.
            set_chapter_dates(&db, id, 10, 100 + i);
            first_ids.push(id);
        }
        let repo = MangaUpdatesRepo::new(db.clone());
        let rows = repo.recent(0).unwrap();
        assert_eq!(rows.len(), UPDATES_LIMIT as usize);
        assert!(!rows.iter().any(|r| r.chapter_id == first_ids[0]), "the oldest fetch is dropped");
        assert_eq!(rows[0].chapter_id, *first_ids.last().unwrap(), "the newest fetch comes first");
    }

    /// U5: rows are ordered by fetch date, newest first, and equal fetch dates fall back to the higher id.
    #[test]
    fn u5_order_is_date_fetch_desc_then_id_desc() {
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", true);
        let a = insert_chapter(&db, m, "a");
        let b = insert_chapter(&db, m, "b");
        let c = insert_chapter(&db, m, "c");
        set_chapter_dates(&db, a, 10, 500);
        set_chapter_dates(&db, b, 10, 700);
        set_chapter_dates(&db, c, 10, 700);
        let repo = MangaUpdatesRepo::new(db.clone());
        assert_eq!(ids(&repo.recent(0).unwrap()), vec![c, b, a]);
    }

    /// U6: the read filter selects by the chapter's read flag, and both feeds agree on the unfiltered list.
    #[test]
    fn u6_read_filter_and_unfiltered_feed() {
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", true);
        let unread = insert_chapter(&db, m, "unread");
        let read = insert_chapter(&db, m, "read");
        set_chapter_dates(&db, unread, 10, 10);
        set_chapter_dates(&db, read, 10, 20);
        set_chapter_read(&db, read, true);
        let repo = MangaUpdatesRepo::new(db.clone());
        assert_eq!(ids(&repo.with_read(true, 0).unwrap()), vec![read]);
        assert_eq!(ids(&repo.with_read(false, 0).unwrap()), vec![unread]);
        assert_eq!(ids(&repo.recent(0).unwrap()), vec![read, unread]);
    }

    /// The watcher publishes again when a chapter's read flag changes.
    #[tokio::test]
    async fn watcher_republishes_when_a_read_flag_changes() {
        let (_d, db) = manga_db();
        let m = insert_manga(&db, "m", true);
        let ch = insert_chapter(&db, m, "ch");
        set_chapter_dates(&db, ch, 10, 10);
        let repo = MangaUpdatesRepo::new(db.clone());
        let mut sub = repo.watch_with_read(false, 0);
        let wait = std::time::Duration::from_secs(2);
        let first = tokio::time::timeout(wait, sub.changed()).await.unwrap().unwrap().expect("query ok");
        assert_eq!(ids(&first), vec![ch]);
        set_chapter_read(&db, ch, true);
        let second = tokio::time::timeout(wait, sub.changed()).await.unwrap().unwrap().expect("query ok");
        assert!(second.is_empty());
    }

    /// The anime feed mirrors the manga rules: favourites only, strict window, newest fetch first, seen filter.
    #[test]
    fn anime_feed_filters_by_seen_and_favourite() {
        let (_d, db) = anime_db();
        let fav = insert_anime(&db, "fav", true);
        let other = insert_anime(&db, "other", false);
        let seen_ep = insert_episode(&db, fav, "seen");
        let new_ep = insert_episode(&db, fav, "new");
        let hidden = insert_episode(&db, other, "hidden");
        set_episode_dates(&db, seen_ep, 10, 30);
        set_episode_dates(&db, new_ep, 10, 40);
        set_episode_dates(&db, hidden, 10, 50);
        db.write(|tx| {
            tx.execute("UPDATE episodes SET seen = 1 WHERE _id = ?1", params![seen_ep])?;
            Ok(())
        })
        .unwrap();
        let repo = AnimeUpdatesRepo::new(db.clone());
        let all: Vec<i64> = repo.recent(0).unwrap().iter().map(|r| r.episode_id).collect();
        assert_eq!(all, vec![new_ep, seen_ep]);
        let unseen: Vec<i64> = repo.with_seen(false, 0).unwrap().iter().map(|r| r.episode_id).collect();
        assert_eq!(unseen, vec![new_ep]);
    }

    /// The Kotlin window: calendar months, day clamped to the target month, and fixed-offset wall clocks kept.
    #[test]
    fn three_months_ago_follows_minus_months_semantics() {
        fn at<Tz: TimeZone>(zone: &Tz, y: i32, m: u32, d: u32, h: u32) -> DateTime<Tz> {
            let naive = NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, 0, 0).unwrap();
            zone.from_local_datetime(&naive).single().unwrap()
        }
        fn naive_of(ms: i64) -> NaiveDateTime {
            DateTime::from_timestamp_millis(ms).unwrap().naive_utc()
        }
        let expect = |y, m, d, h| NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, 0, 0).unwrap();
        assert_eq!(naive_of(three_months_ago_ms(&at(&Utc, 2025, 5, 31, 10))), expect(2025, 2, 28, 10));
        assert_eq!(naive_of(three_months_ago_ms(&at(&Utc, 2024, 5, 31, 10))), expect(2024, 2, 29, 10));
        assert_eq!(naive_of(three_months_ago_ms(&at(&Utc, 2025, 1, 15, 0))), expect(2024, 10, 15, 0));
        let zone = FixedOffset::east_opt(3600).unwrap();
        let back = DateTime::from_timestamp_millis(three_months_ago_ms(&at(&zone, 2025, 5, 31, 10))).unwrap();
        assert_eq!(back.with_timezone(&zone).naive_local(), expect(2025, 2, 28, 10));
    }
}
