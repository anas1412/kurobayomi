//! Unread-updates queries over `updatesView` and `animeupdatesView` (T-50a-005, T-50a-006) and the read-only database
//! open (T-50a-016). The CLI never creates, migrates or writes a database.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use super::{FeedError, Kind, UpdateRow};

/// Longest the CLI waits for a writer before answering `db_busy`.
pub const BUSY_TIMEOUT: Duration = Duration::from_millis(250);

// The ORDER BY is explicit: Android relied on the view order. LIMIT comes before the per-entry dedupe (E-5).
const MANGA_SQL: &str = "SELECT mangaId, mangaTitle, chapterId, thumbnailUrl, coverLastModified FROM updatesView \
     WHERE read = 0 AND dateUpload > ?1 ORDER BY datefetch DESC, chapterId DESC LIMIT ?2";
const ANIME_SQL: &str = "SELECT animeId, animeTitle, episodeId, thumbnailUrl, coverLastModified FROM animeupdatesView \
     WHERE seen = 0 AND dateUpload > ?1 ORDER BY datefetch DESC, episodeId DESC LIMIT ?2";

/// Opens an existing database read-only: no `SQLITE_OPEN_CREATE`, busy timeout 250 ms. A missing file is
/// [`FeedError::DbMissing`] and stays missing.
pub fn open_read_only(path: &Path) -> Result<Connection, FeedError> {
    if !path.is_file() {
        return Err(FeedError::DbMissing(path.display().to_string()));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(db_error)?;
    conn.busy_timeout(BUSY_TIMEOUT).map_err(db_error)?;
    Ok(conn)
}

/// Favourite entries with an unread (unseen) update uploaded strictly after `after_ms`, newest fetch first.
pub fn unread_updates(conn: &Connection, kind: Kind, after_ms: i64, limit: usize) -> Result<Vec<UpdateRow>, FeedError> {
    let sql = match kind {
        Kind::Manga => MANGA_SQL,
        Kind::Anime => ANIME_SQL,
    };
    let mut stmt = conn.prepare(sql).map_err(db_error)?;
    let rows = stmt
        .query_map(rusqlite::params![after_ms, limit as i64], |r| {
            Ok(UpdateRow {
                entry_id: r.get(0)?,
                title: r.get(1)?,
                update_id: r.get(2)?,
                thumbnail_url: r.get(3)?,
                cover_last_modified: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
            })
        })
        .map_err(db_error)?;
    rows.collect::<Result<_, _>>().map_err(db_error)
}

fn db_error(e: rusqlite::Error) -> FeedError {
    use rusqlite::ErrorCode::{DatabaseBusy, DatabaseLocked};
    match e.sqlite_error_code() {
        Some(DatabaseBusy | DatabaseLocked) => FeedError::DbBusy,
        _ => FeedError::Db(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANGA_FIXTURE: &str = include_str!("../../tests/fixtures/feed/manga.sql");
    const ANIME_FIXTURE: &str = include_str!("../../tests/fixtures/feed/anime.sql");
    const AFTER: i64 = 1_783_512_000_000;

    fn manga_db(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("manga.db");
        let c = kuro_db::schema_manga::open(&path).unwrap();
        c.execute_batch(MANGA_FIXTURE).unwrap();
        path
    }

    #[test]
    fn manga_filters_and_order() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_read_only(&manga_db(dir.path())).unwrap();
        let rows = unread_updates(&conn, Kind::Manga, AFTER, 500).unwrap();
        let got: Vec<(i64, i64)> = rows.iter().map(|r| (r.entry_id, r.update_id)).collect();
        // not favourite (c4), read (c6), upload == window start (c7) and fetched before added (c8) are out
        assert_eq!(got, vec![(1, 1), (1, 2), (2, 3), (4, 5), (5, 9), (6, 10)]);
        assert_eq!(rows[0].thumbnail_url.as_deref(), Some("file:///covers/a.jpg"));
        assert_eq!(rows[0].cover_last_modified, 5);
    }

    #[test]
    fn limit_applies_before_dedupe() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_read_only(&manga_db(dir.path())).unwrap();
        let rows = unread_updates(&conn, Kind::Manga, AFTER, 2).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].entry_id, rows[1].entry_id);
    }

    #[test]
    fn large_fixture_is_capped_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = manga_db(dir.path());
        let w = rusqlite::Connection::open(&path).unwrap();
        for i in 100..700 {
            w.execute(
                "INSERT INTO chapters(_id,manga_id,url,name,read,bookmark,last_page_read,chapter_number,source_order,date_fetch,date_upload) \
                 VALUES(?1,1,'/x','X',0,0,0,1.0,0,9000,?2)",
                rusqlite::params![i, AFTER + 5],
            )
            .unwrap();
        }
        drop(w);
        let conn = open_read_only(&path).unwrap();
        assert_eq!(unread_updates(&conn, Kind::Manga, AFTER, super::super::grid::DB_LIMIT).unwrap().len(), 500);
    }

    #[test]
    fn anime_uses_unseen_and_episode_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anime.db");
        kuro_db::schema_anime::open(&path).unwrap().execute_batch(ANIME_FIXTURE).unwrap();
        let conn = open_read_only(&path).unwrap();
        let rows = unread_updates(&conn, Kind::Anime, AFTER, 500).unwrap();
        let got: Vec<(i64, i64)> = rows.iter().map(|r| (r.entry_id, r.update_id)).collect();
        // anime 3 has only seen episodes; anime 1's newest unseen episode is 2
        assert_eq!(got, vec![(1, 2), (1, 3), (2, 4)]);
    }

    #[test]
    fn a_missing_database_is_not_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.db");
        assert!(matches!(open_read_only(&path), Err(FeedError::DbMissing(_))));
        assert!(!path.exists());
    }

    #[test]
    fn the_connection_cannot_write() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_read_only(&manga_db(dir.path())).unwrap();
        assert!(conn.execute("DELETE FROM chapters", []).is_err());
        let timeout: i64 = conn.query_row("PRAGMA busy_timeout", [], |r| r.get(0)).unwrap();
        assert_eq!(timeout, 250);
    }

    /// A writer holding a transaction for a second never gives a torn row: the reader answers within 300 ms
    /// with committed data or reports busy.
    #[test]
    fn a_held_write_transaction_gives_data_or_busy_in_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = manga_db(dir.path());
        let mut writer = rusqlite::Connection::open(&path).unwrap();
        let tx = writer.transaction().unwrap();
        tx.execute("UPDATE mangas SET title = 'Changed' WHERE _id = 1", []).unwrap();
        let started = std::time::Instant::now();
        let result = open_read_only(&path).and_then(|c| unread_updates(&c, Kind::Manga, AFTER, 500));
        assert!(started.elapsed() < Duration::from_millis(300));
        match result {
            Ok(rows) => assert!(rows.iter().filter(|r| r.entry_id == 1).all(|r| r.title == "Alpha")),
            Err(e) => assert_eq!(e.exit_code(), 3),
        }
        tx.commit().unwrap();
    }
}
