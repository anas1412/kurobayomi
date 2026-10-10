//! Manga entry restore (area 22, T-22a-011 to T-22a-021). Each entry runs inside the caller's transaction, so a failure
//! anywhere in it rolls the whole entry back. Rules follow `plans/22-backup-restore/p1-behavior.md` section 4.3 with the
//! fixes named in the task blocks.

use std::collections::{HashMap, HashSet};

use kuro_db::codec::GenreList;
use kuro_db::models::{ChapterPatch, ChapterRow, HistoryRow, MangaPatch, MangaRow, TrackPatch, TrackRow};
use kuro_db::statements_manga::{chapters, excluded_scanlators, history, manga_sync, mangas, mangas_categories};
use kuro_db::{DbResult, Transaction};
use kuro_domain::entries::fetch_interval::{ItemDates, Stored, to_update};

use super::categories::{differs_from_stored, membership_ids};
use super::merge::{RunClock, merge_progress, round6, sort_new_first};
use crate::schema::{BackupChapter, BackupHistory, BackupManga, BackupTracking};

/// The `(source, url)` pairs already in `mangas`, read once per section (T-22a-011).
pub fn existing_pairs(tx: &Transaction<'_>) -> DbResult<HashSet<(i64, String)>> {
    Ok(mangas::get_all_source_and_url(tx)?.into_iter().collect())
}

/// Entries not in the database first, newest `lastModifiedAt` first, ties in backup order (T-22a-011).
pub fn sort_manga(entries: Vec<BackupManga>, existing: &HashSet<(i64, String)>) -> Vec<BackupManga> {
    sort_new_first(entries, |m| (m.source, m.url.as_str(), m.last_modified_at), existing)
}

/// Restores one manga entry: row, categories, chapters, tracks, history, excluded scanlators, fetch interval.
/// `categories` maps backup category order to database id (empty when the categories option is off).
pub fn restore_manga_entry(
    tx: &Transaction<'_>,
    clock: &RunClock,
    entry: &BackupManga,
    categories: &HashMap<i64, i64>,
) -> DbResult<()> {
    let id = match mangas::get_by_url_and_source(tx, &entry.url, entry.source)? {
        None => insert_new(tx, entry)?,
        Some(existing) => {
            update_existing(tx, entry, &existing)?;
            existing.id
        }
    };
    if let Some(ids) = membership_ids(categories, &entry.categories) {
        if memberships_differ(tx, id, &ids)? {
            // The membership insert trigger bumps `version` unless the row is syncing; the restore keeps the version it set.
            let version = mangas::get_by_id(tx, id)?.version;
            mangas_categories::set_categories(tx, id, &ids)?;
            tx.execute("UPDATE mangas SET version = ?1 WHERE _id = ?2", [version, id])?;
        }
    }
    restore_chapters(tx, id, &entry.chapters)?;
    restore_tracks(tx, id, &entry.tracking)?;
    restore_history(tx, id, &entry.history)?;
    restore_scanlators(tx, id, &entry.excluded_scanlators)?;
    refresh_interval(tx, clock, id)
}

fn memberships_differ(tx: &Transaction<'_>, manga_id: i64, ids: &[i64]) -> DbResult<bool> {
    let stored: Vec<i64> = tx
        .prepare("SELECT category_id FROM mangas_categories WHERE manga_id = ?1")?
        .query_map([manga_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(differs_from_stored(stored, ids))
}

/// True when applying `p` would leave `d` as it is. The write is skipped then, so a repeat restore writes nothing.
fn update_is_noop(p: &MangaPatch, d: &MangaRow) -> bool {
    fn same<T: PartialEq>(p: &Option<T>, cur: &T) -> bool {
        p.as_ref().is_none_or(|v| v == cur)
    }
    fn same_opt<T: PartialEq>(p: &Option<T>, cur: &Option<T>) -> bool {
        p.as_ref().is_none_or(|v| Some(v) == cur.as_ref())
    }
    same(&p.favorite, &d.favorite)
        && same(&p.initialized, &d.initialized)
        && same(&p.is_syncing, &d.is_syncing)
        && same(&p.version, &d.version)
        && same(&p.status, &d.status)
        && same(&p.title, &d.title)
        && same(&p.viewer, &d.viewer)
        && same(&p.chapter_flags, &d.chapter_flags)
        && same(&p.update_strategy, &d.update_strategy)
        && same(&p.date_added, &d.date_added)
        && same(&p.cover_last_modified, &d.cover_last_modified)
        && same_opt(&p.artist, &d.artist)
        && same_opt(&p.author, &d.author)
        && same_opt(&p.description, &d.description)
        && same_opt(&p.genre, &d.genre)
        && same_opt(&p.thumbnail_url, &d.thumbnail_url)
        && same_opt(&p.last_update, &d.last_update)
}

/// A new row (T-22a-014). `initialized` is `description != null` (D6 fixed). `next_update` and `calculate_interval`
/// start at 0 and are set by `refresh_interval`.
fn insert_new(tx: &Transaction<'_>, b: &BackupManga) -> DbResult<i64> {
    let row = MangaRow {
        id: 0,
        source: b.source,
        url: b.url.clone(),
        artist: b.artist.clone(),
        author: b.author.clone(),
        description: b.description.clone(),
        genre: Some(GenreList(b.genre.clone())),
        title: b.title.clone(),
        status: i64::from(b.status),
        thumbnail_url: b.thumbnail_url.clone(),
        favorite: b.favorite,
        last_update: Some(0),
        next_update: Some(0),
        initialized: b.description.is_some(),
        viewer: i64::from(b.effective_viewer_flags()),
        chapter_flags: i64::from(b.chapter_flags),
        cover_last_modified: 0,
        date_added: b.date_added,
        update_strategy: b.update_strategy,
        calculate_interval: 0,
        last_modified_at: 0,
        favorite_modified_at: None,
        version: b.version,
        is_syncing: false,
    };
    Ok(mangas::insert(tx, &row)?)
}

/// The merge of an existing row (T-22a-012, T-22a-013). The newer side owns the metadata and the version; the other
/// side owns title, viewer, flags, strategy and dates (D5, parity). `favorite` and `initialized` are ORed. The write
/// is a coalesce update: a field left `None` keeps the stored value, so `next_update` and `calculate_interval` stay.
fn update_existing(tx: &Transaction<'_>, b: &BackupManga, d: &MangaRow) -> DbResult<()> {
    let mut p = MangaPatch {
        id: d.id,
        favorite: Some(d.favorite || b.favorite),
        initialized: Some(d.initialized || b.description.is_some()),
        // E-25: the restore never leaves a row marked as syncing.
        is_syncing: Some(false),
        ..Default::default()
    };
    if b.version > d.version {
        // Backup is newer: metadata and version from the backup. Title, viewer, flags, strategy, dates keep the row.
        p.artist = b.artist.clone();
        p.author = b.author.clone();
        p.description = b.description.clone();
        p.genre = Some(GenreList(b.genre.clone()));
        p.thumbnail_url = b.thumbnail_url.clone();
        p.status = Some(i64::from(b.status));
        p.version = Some(b.version);
    } else {
        // Row is newer or equal: metadata and version stay as they are. The other fields come from the backup. The
        // backup carries no last-update or cover date, so those become 0 (parity with the Kotlin mapping).
        p.title = Some(b.title.clone());
        p.viewer = Some(i64::from(b.effective_viewer_flags()));
        p.chapter_flags = Some(i64::from(b.chapter_flags));
        p.update_strategy = Some(b.update_strategy);
        p.date_added = Some(b.date_added);
        p.last_update = Some(0);
        p.cover_last_modified = Some(0);
    }
    if !update_is_noop(&p, d) {
        mangas::update(tx, &p)?;
    }
    Ok(())
}

/// Chapters (T-22a-015, T-22a-016). Lookup is by url within this manga. An unchanged chapter is skipped.
fn restore_chapters(tx: &Transaction<'_>, manga_id: i64, backup: &[BackupChapter]) -> DbResult<()> {
    let db = chapters::get_by_manga_id(tx, manga_id, false)?;
    let by_url: HashMap<&str, &ChapterRow> = db.iter().map(|r| (r.url.as_str(), r)).collect();
    for b in backup {
        let row = ChapterRow {
            id: 0,
            manga_id,
            url: b.url.clone(),
            name: b.name.clone(),
            scanlator: b.scanlator.clone(),
            read: b.read,
            bookmark: b.bookmark,
            last_page_read: b.last_page_read,
            chapter_number: round6(b.chapter_number),
            source_order: b.source_order,
            date_fetch: b.date_fetch,
            date_upload: b.date_upload,
            last_modified_at: 0,
            version: b.version,
            is_syncing: false,
        };
        match by_url.get(row.url.as_str()) {
            None => {
                chapters::insert(tx, &row)?;
            }
            Some(d) if same_chapter_state(&row, d) => {}
            Some(d) => {
                let (read, last_page_read) = merge_progress(row.read, row.last_page_read, d.read, d.last_page_read);
                chapters::update(
                    tx,
                    &ChapterPatch {
                        id: d.id,
                        read: Some(read),
                        bookmark: Some(row.bookmark || d.bookmark),
                        last_page_read: Some(last_page_read),
                        version: Some(row.version),
                        is_syncing: Some(false),
                        ..Default::default()
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// `forComparison`: every field but the ids, the fetch and upload dates, `lastModifiedAt` and `version`.
fn same_chapter_state(a: &ChapterRow, d: &ChapterRow) -> bool {
    a.url == d.url
        && a.name == d.name
        && a.scanlator == d.scanlator
        && a.read == d.read
        && a.bookmark == d.bookmark
        && a.last_page_read == d.last_page_read
        && a.chapter_number == d.chapter_number
        && a.source_order == d.source_order
}

/// Tracks (T-22a-019). Lookup is by tracker id within the entry. An existing track changes only the remote id, the
/// library id and the last chapter read (the larger of the two).
fn restore_tracks(tx: &Transaction<'_>, manga_id: i64, backup: &[BackupTracking]) -> DbResult<()> {
    let db = manga_sync::get_tracks_by_manga_id(tx, manga_id)?;
    let by_tracker: HashMap<i64, &TrackRow> = db.iter().map(|r| (r.sync_id, r)).collect();
    for b in backup {
        let row = TrackRow {
            id: 0,
            manga_id,
            sync_id: i64::from(b.sync_id),
            remote_id: b.remote_id(),
            library_id: Some(b.library_id),
            title: b.title.clone(),
            last_chapter_read: f64::from(b.last_chapter_read),
            total_chapters: i64::from(b.total_chapters),
            status: i64::from(b.status),
            score: f64::from(b.score),
            remote_url: b.tracking_url.clone(),
            start_date: b.started_reading_date,
            finish_date: b.finished_reading_date,
            private: b.private,
        };
        match by_tracker.get(&row.sync_id) {
            None => {
                manga_sync::insert(tx, &row)?;
            }
            Some(d) if same_track_state(&row, d) => {}
            Some(d) => {
                manga_sync::update(
                    tx,
                    &TrackPatch {
                        id: d.id,
                        remote_id: Some(row.remote_id),
                        library_id: row.library_id,
                        last_chapter_read: Some(d.last_chapter_read.max(row.last_chapter_read)),
                        ..Default::default()
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// `forComparison` for a track: every field but the id and the entry id.
fn same_track_state(a: &TrackRow, d: &TrackRow) -> bool {
    TrackRow { id: d.id, manga_id: d.manga_id, ..a.clone() } == *d
}

/// Read history (T-22a-018). The chapter is looked up in this manga only (D9 fixed). A history row for a chapter
/// that does not exist here is skipped. `time_read` ends at the larger of the two values, and `last_read` at the later.
fn restore_history(tx: &Transaction<'_>, manga_id: i64, backup: &[BackupHistory]) -> DbResult<()> {
    let chapter_ids: HashMap<String, i64> =
        chapters::get_by_manga_id(tx, manga_id, false)?.into_iter().map(|r| (r.url, r.id)).collect();
    let existing: HashMap<i64, HistoryRow> =
        history::get_by_manga_id(tx, manga_id)?.into_iter().map(|h| (h.chapter_id, h)).collect();
    for h in backup {
        let Some(&chapter_id) = chapter_ids.get(&h.url) else {
            continue;
        };
        let (read_at, duration) = match existing.get(&chapter_id) {
            None => (h.last_read, h.read_duration),
            Some(d) => (h.last_read.max(d.last_read), h.read_duration.max(d.time_read) - d.time_read),
        };
        if existing.get(&chapter_id).is_some_and(|d| d.last_read == read_at) && duration == 0 {
            continue;
        }
        history::upsert(tx, chapter_id, read_at, duration)?;
    }
    Ok(())
}

/// Excluded scanlators (T-22a-020): only the ones not stored yet. A repeated name in the backup is written once.
fn restore_scanlators(tx: &Transaction<'_>, manga_id: i64, backup: &[String]) -> DbResult<()> {
    if backup.is_empty() {
        return Ok(());
    }
    let mut seen: HashSet<String> = excluded_scanlators::get_by_manga_id(tx, manga_id)?.into_iter().collect();
    for s in backup {
        if seen.insert(s.clone()) {
            excluded_scanlators::insert(tx, manga_id, s)?;
        }
    }
    Ok(())
}

/// Recomputes `next_update` and `fetch_interval` from the chapters and the run clock (T-22a-021). Nothing else is
/// written. The window is the one captured in `clock`, so it is the same for every entry of a run (E-47).
pub fn refresh_interval(tx: &Transaction<'_>, clock: &RunClock, manga_id: i64) -> DbResult<()> {
    let row = mangas::get_by_id(tx, manga_id)?;
    let items: Vec<ItemDates> = chapters::get_by_manga_id(tx, manga_id, true)?
        .iter()
        .map(|r| ItemDates { date_upload: r.date_upload, date_fetch: r.date_fetch })
        .collect();
    let stored = Stored {
        fetch_interval: row.calculate_interval as i32,
        next_update: row.next_update.unwrap_or(0),
        last_update: row.last_update.unwrap_or(0),
    };
    let out = to_update(stored, &items, &clock.now, clock.window, clock.clock_now_ms);
    if row.next_update == Some(out.next_update) && row.calculate_interval == i64::from(out.fetch_interval) {
        return Ok(());
    }
    mangas::update(
        tx,
        &MangaPatch {
            id: manga_id,
            next_update: Some(out.next_update),
            calculate_interval: Some(i64::from(out.fetch_interval)),
            ..Default::default()
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};
    use kuro_db::schema_manga;
    use kuro_db::statements_manga::categories;
    use rusqlite::Connection;

    use super::*;
    use crate::schema::BackupCategory;

    fn clock() -> RunClock {
        RunClock::new(Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).single().unwrap())
    }

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_manga::open(&dir.path().join("manga.db")).unwrap();
        (dir, c)
    }

    fn manga(url: &str) -> BackupManga {
        BackupManga { source: 7, url: url.into(), title: format!("T {url}"), ..Default::default() }
    }

    fn chapter(url: &str, read: bool, bookmark: bool, last_page_read: i64) -> BackupChapter {
        BackupChapter {
            url: url.into(),
            name: format!("name {url}"),
            read,
            bookmark,
            last_page_read,
            chapter_number: 1.1,
            version: 1,
            ..Default::default()
        }
    }

    fn row(c: &Connection, url: &str) -> MangaRow {
        mangas::get_by_url_and_source(c, url, 7).unwrap().unwrap()
    }

    /// T-20a-094: 2499283573021220255 is the id Tachiyomi and Aniyomi backups carry for MangaDex (English, version 1). The
    /// id rule reproduces it from the name, language and version, and a restore keeps a backup's source integer as is.
    #[test]
    fn a_backup_source_id_equals_the_generated_id_and_survives_a_restore_unchanged() {
        const MANGADEX_BACKUP_ID: i64 = 2499283573021220255;
        assert_eq!(kuro_source::generate_id("MangaDex", "en", 1), MANGADEX_BACKUP_ID);
        let (_d, mut c) = db();
        let tx = c.transaction().unwrap();
        let mut m = manga("md");
        m.source = MANGADEX_BACKUP_ID;
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let r = mangas::get_by_url_and_source(&c, "md", MANGADEX_BACKUP_ID).unwrap().unwrap();
        assert_eq!(r.source, kuro_source::generate_id("MangaDex", "en", 1));
    }

    #[test]
    fn new_row_stores_initialized_from_description_and_zeroed_schedule() {
        let (_d, mut c) = db();
        let tx = c.transaction().unwrap();
        let mut m = manga("a");
        m.description = Some("text".into());
        m.version = 4;
        m.date_added = 1234;
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let r = row(&c, "a");
        assert!(r.initialized, "D6: initialized follows description");
        assert_eq!(r.version, 4);
        assert_eq!(r.date_added, 1234);
    }

    /// E-25 and E-26: a new row is written not syncing, without the backup's `lastModifiedAt` and `favoriteModifiedAt`.
    /// (Triggers set both times on their own, so the test asserts only that the backup values are not copied.) The backup values only drive the sort.
    #[test]
    fn new_row_is_not_syncing_and_ignores_the_backup_modified_times() {
        let (_d, mut c) = db();
        let tx = c.transaction().unwrap();
        let mut m = manga("k");
        m.last_modified_at = 500;
        m.favorite_modified_at = Some(900);
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let r = row(&c, "k");
        assert!(!r.is_syncing);
        assert_ne!(r.last_modified_at, 500);
        assert_ne!(r.favorite_modified_at, Some(900));
    }

    #[test]
    fn existing_row_marked_syncing_is_cleared_and_backup_modified_times_are_not_written() {
        for backup_version in [1, 9] {
            let (_d, mut c) = db();
            {
                let tx = c.transaction().unwrap();
                let mut first = manga("s");
                first.version = 5;
                restore_manga_entry(&tx, &clock(), &first, &HashMap::new()).unwrap();
                tx.commit().unwrap();
            }
            let id = row(&c, "s").id;
            mangas::update(&c, &MangaPatch { id, is_syncing: Some(true), ..Default::default() }).unwrap();
            assert!(row(&c, "s").is_syncing);

            let tx = c.transaction().unwrap();
            let mut b = manga("s");
            b.version = backup_version;
            b.last_modified_at = 500;
            b.favorite_modified_at = Some(900);
            restore_manga_entry(&tx, &clock(), &b, &HashMap::new()).unwrap();
            tx.commit().unwrap();
            let r = row(&c, "s");
            assert!(!r.is_syncing, "version {backup_version}: E-25");
            assert_ne!(r.last_modified_at, 500, "E-26: the backup value is not copied");
            assert_ne!(r.favorite_modified_at, Some(900));
        }
    }

    #[test]
    fn new_row_without_description_is_not_initialized() {
        let (_d, mut c) = db();
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &manga("b"), &HashMap::new()).unwrap();
        tx.commit().unwrap();
        assert!(!row(&c, "b").initialized);
    }

    #[test]
    fn existing_row_keeps_favorite_and_takes_metadata_from_the_newer_side() {
        let (_d, mut c) = db();
        {
            let tx = c.transaction().unwrap();
            let mut db_side = manga("e");
            db_side.version = 5;
            db_side.title = "db title".into();
            db_side.author = Some("db author".into());
            db_side.favorite = true;
            restore_manga_entry(&tx, &clock(), &db_side, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        // backup older (version 2) and not favourite: metadata from the row, favourite stays true
        let tx = c.transaction().unwrap();
        let mut b = manga("e");
        b.version = 2;
        b.title = "backup title".into();
        b.author = Some("backup author".into());
        b.favorite = false;
        b.date_added = 77;
        restore_manga_entry(&tx, &clock(), &b, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let r = row(&c, "e");
        assert!(r.favorite, "favorite is always the OR");
        assert_eq!(r.author.as_deref(), Some("db author"));
        assert_eq!(r.title, "backup title", "older backup owns title");
        assert_eq!(r.date_added, 77);
        assert_eq!(r.version, 5, "older backup keeps the row version");
    }

    #[test]
    fn newer_backup_owns_metadata_and_keeps_the_row_title() {
        let (_d, mut c) = db();
        {
            let tx = c.transaction().unwrap();
            let mut db_side = manga("n");
            db_side.version = 1;
            db_side.title = "db title".into();
            restore_manga_entry(&tx, &clock(), &db_side, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let tx = c.transaction().unwrap();
        let mut b = manga("n");
        b.version = 9;
        b.title = "backup title".into();
        b.author = Some("new author".into());
        restore_manga_entry(&tx, &clock(), &b, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let r = row(&c, "n");
        assert_eq!(r.author.as_deref(), Some("new author"));
        assert_eq!(r.title, "db title");
        assert_eq!(r.version, 9);
    }

    #[test]
    fn sort_puts_new_rows_first_then_newest_modified() {
        let (_d, mut c) = db();
        {
            let tx = c.transaction().unwrap();
            restore_manga_entry(&tx, &clock(), &manga("old"), &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let existing = existing_pairs(&c.transaction().unwrap()).unwrap();
        let mut a = manga("old");
        a.last_modified_at = 999;
        let mut b = manga("new-1");
        b.last_modified_at = 10;
        let mut d = manga("new-2");
        d.last_modified_at = 20;
        let sorted = sort_manga(vec![a, b, d], &existing);
        let urls: Vec<&str> = sorted.iter().map(|m| m.url.as_str()).collect();
        assert_eq!(urls, vec!["new-2", "new-1", "old"]);
    }

    #[test]
    fn chapters_insert_new_and_merge_progress_on_existing() {
        let (_d, mut c) = db();
        {
            let tx = c.transaction().unwrap();
            let mut m = manga("ch");
            m.chapters = vec![chapter("c1", true, false, 0), chapter("c2", false, false, 7)];
            restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let tx = c.transaction().unwrap();
        let mut m = manga("ch");
        // c1: backup unread at 0, db read -> read stays true. c2: backup read at 5 with bookmark, db at 7 unread.
        m.chapters = vec![chapter("c1", false, true, 0), chapter("c2", true, true, 5), chapter("c3", false, false, 0)];
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let id = row(&c, "ch").id;
        let rows = chapters::get_by_manga_id(&c, id, false).unwrap();
        let by = |u: &str| rows.iter().find(|r| r.url == u).unwrap().clone();
        assert_eq!(rows.len(), 3);
        assert!(by("c1").read && by("c1").bookmark);
        assert_eq!(by("c2").last_page_read, 5, "backup position wins when smaller and read differs");
        assert!(by("c2").read);
        assert!(!by("c3").read);
        assert_eq!(by("c1").chapter_number, 1.1, "float chapter numbers stored as 1.1");
    }

    #[test]
    fn unchanged_chapter_is_skipped() {
        let (_d, mut c) = db();
        let mut m = manga("same");
        m.chapters = vec![chapter("x", false, false, 0)];
        {
            let tx = c.transaction().unwrap();
            restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let id = row(&c, "same").id;
        let before = chapters::get_by_manga_id(&c, id, false).unwrap();
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let after = chapters::get_by_manga_id(&c, id, false).unwrap();
        assert_eq!(before[0].version, after[0].version, "no write for an unchanged chapter");
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn categories_replace_memberships_only_when_an_order_maps() {
        let (_d, mut c) = db();
        let read = categories::insert(&c, "Read", 0, 0).unwrap();
        let later = categories::insert(&c, "Later", 1, 0).unwrap();
        let mut m = manga("cat");
        m.categories = vec![0];
        let map = HashMap::from([(0, read)]);
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &map).unwrap();
        tx.commit().unwrap();
        let id = row(&c, "cat").id;
        let names: Vec<i64> = categories::get_categories_by_manga_id(&c, id).unwrap().into_iter().map(|r| r.id).collect();
        assert_eq!(names, vec![read]);

        // unmapped order: memberships untouched
        m.categories = vec![9];
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &HashMap::from([(0, later)])).unwrap();
        tx.commit().unwrap();
        let names: Vec<i64> = categories::get_categories_by_manga_id(&c, id).unwrap().into_iter().map(|r| r.id).collect();
        assert_eq!(names, vec![read]);
    }

    #[test]
    fn new_row_with_mapped_categories_keeps_the_backup_version() {
        let (_d, mut c) = db();
        let a = categories::insert(&c, "A", 0, 0).unwrap();
        let b = categories::insert(&c, "B", 1, 0).unwrap();
        let mut m = manga("ver");
        m.version = 4;
        m.categories = vec![0, 1];
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &HashMap::from([(0, a), (1, b)])).unwrap();
        tx.commit().unwrap();
        assert_eq!(row(&c, "ver").version, 4);
    }

    #[test]
    fn history_is_scoped_to_the_manga_and_time_read_takes_the_max() {
        let (_d, mut c) = db();
        {
            let tx = c.transaction().unwrap();
            let mut other = manga("other");
            other.chapters = vec![chapter("shared", false, false, 0)];
            restore_manga_entry(&tx, &clock(), &other, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let mut m = manga("hist");
        m.chapters = vec![chapter("shared", false, false, 0)];
        m.history = vec![BackupHistory { url: "shared".into(), last_read: 100, read_duration: 50 }];
        {
            let tx = c.transaction().unwrap();
            restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let id = row(&c, "hist").id;
        let mine = chapters::get_by_manga_id(&c, id, false).unwrap();
        let h = history::get_by_manga_id(&c, id).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].chapter_id, mine[0].id, "history attaches to this manga's chapter");
        assert_eq!((h[0].last_read, h[0].time_read), (100, 50));

        // restoring again keeps the same values (idempotent)
        {
            let tx = c.transaction().unwrap();
            restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let h = history::get_by_manga_id(&c, id).unwrap();
        assert_eq!((h[0].last_read, h[0].time_read), (100, 50));
    }

    #[test]
    fn history_for_a_missing_chapter_is_skipped() {
        let (_d, mut c) = db();
        let mut m = manga("skip");
        m.history = vec![BackupHistory { url: "ghost".into(), last_read: 5, read_duration: 1 }];
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let id = row(&c, "skip").id;
        assert!(history::get_by_manga_id(&c, id).unwrap().is_empty());
    }

    #[test]
    fn track_new_is_inserted_and_existing_changes_only_remote_library_and_progress() {
        let (_d, mut c) = db();
        let mut m = manga("tr");
        m.tracking = vec![BackupTracking { sync_id: 2, media_id: 55, library_id: 9, last_chapter_read: 3.0, score: 4.0, status: 1, title: "t".into(), ..Default::default() }];
        {
            let tx = c.transaction().unwrap();
            restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        let id = row(&c, "tr").id;
        let first = manga_sync::get_tracks_by_manga_id(&c, id).unwrap();
        assert_eq!((first.len(), first[0].remote_id, first[0].score), (1, 55, 4.0));

        // DB side has a higher progress and a different score; the backup changes remote id and library id only
        c.execute("UPDATE manga_sync SET last_chapter_read = 8.0, score = 9.0, status = 2 WHERE manga_id = ?1", [id]).unwrap();
        m.tracking[0].media_id = 66;
        m.tracking[0].library_id = 11;
        m.tracking[0].last_chapter_read = 5.0;
        m.tracking[0].score = 1.0;
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let t = &manga_sync::get_tracks_by_manga_id(&c, id).unwrap()[0];
        assert_eq!(t.remote_id, 66);
        assert_eq!(t.library_id, Some(11));
        assert_eq!(t.last_chapter_read, 8.0, "max of DB and backup");
        assert_eq!((t.score, t.status), (9.0, 2), "score and status keep the DB values");
    }

    #[test]
    fn excluded_scanlators_insert_only_the_missing_ones() {
        let (_d, mut c) = db();
        let mut m = manga("sc");
        m.excluded_scanlators = vec!["A".into()];
        {
            let tx = c.transaction().unwrap();
            restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
            tx.commit().unwrap();
        }
        m.excluded_scanlators = vec!["A".into(), "B".into(), "B".into()];
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let id = row(&c, "sc").id;
        let mut got = excluded_scanlators::get_by_manga_id(&c, id).unwrap();
        got.sort();
        assert_eq!(got, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn fetch_interval_is_recomputed_and_only_its_two_fields_change() {
        let (_d, mut c) = db();
        let mut m = manga("fi");
        m.chapters = (0..4).map(|i| BackupChapter { date_upload: i * 86_400_000, ..chapter(&format!("f{i}"), false, false, 0) }).collect();
        let tx = c.transaction().unwrap();
        restore_manga_entry(&tx, &clock(), &m, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        let r = row(&c, "fi");
        assert!((1..=28).contains(&r.calculate_interval), "interval clamped to 1..28");
        assert!(r.next_update.unwrap_or(0) > 0);
        assert!(r.title == "T fi", "title untouched by the interval step");
    }

    #[test]
    fn the_category_order_map_comes_from_the_backup_list() {
        // Shape check for the caller: a backup category order maps to the id of the row of the same name.
        let (_d, c) = db();
        let out = super::super::categories::restore_categories(
            &c,
            super::super::categories::Kind::Manga,
            &[BackupCategory { name: "Z".into(), order: 3, id: 1, flags: 0 }],
        )
        .unwrap();
        assert!(out.by_order.contains_key(&3));
    }
}
