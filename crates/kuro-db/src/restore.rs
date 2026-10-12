//! DB-level statements for backup restore (T-04a-023): one function per statement the anime restorers run
//! directly, plus the version rule of `AnimeRestorer.restoreExistingAnime` as a pure function. The
//! restorer runs these inside one `Db::write`, so a failure or a panic in the middle rolls the whole
//! restore step back and no row keeps `is_syncing = 1`.

use rusqlite::{named_params, Connection};

use crate::statements_anime::{self as sql, AnimeRow, EpisodeRow, EpisodeUpdate};

/// Which side supplies the fields of an anime that already exists (`restoreExistingAnime`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The backup row wins: its `version` is newer than the database row's.
    Backup,
    /// The database row keeps its fields; the backup row only fills what it can.
    Database,
}

/// The version rule of `restoreExistingAnime`: the backup supplies the fields only when its version is
/// strictly newer than the database row's, so an equal version keeps the database side.
pub fn merge_existing(backup_version: i64, db_version: i64) -> Side {
    if backup_version > db_version { Side::Backup } else { Side::Database }
}

/// `(source, url)` of every anime row, the set the restorer checks a backup entry against.
pub fn all_source_and_url(c: &Connection) -> rusqlite::Result<Vec<(i64, String)>> {
    c.prepare("SELECT source, url FROM animes")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}

/// One history row found by episode url: the episode it belongs to and its `last_seen`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreHistory {
    pub episode_id: i64,
    pub last_seen: Option<i64>,
}

/// The history rows whose episode has `url`. Several episodes of different anime may share a url.
pub fn history_by_episode_url(c: &Connection, url: &str) -> rusqlite::Result<Vec<RestoreHistory>> {
    c.prepare(
        "SELECT H.episode_id, H.last_seen FROM animehistory AS H JOIN episodes AS E ON E._id = H.episode_id WHERE E.url = :url",
    )?
    .query_map(named_params! {":url": url}, |r| Ok(RestoreHistory { episode_id: r.get(0)?, last_seen: r.get(1)? }))?
    .collect()
}

/// Every episode with `url`, in any anime.
pub fn episode_by_url(c: &Connection, url: &str) -> rusqlite::Result<Vec<EpisodeRow>> {
    sql::episodes_by_url(c, url)
}

/// Sets `is_syncing` and the supplied `version` of an anime row. The version is written as given and not
/// bumped: the version trigger does not fire while `is_syncing` is 1, and the restorer sets it explicitly.
pub fn update_anime_syncing(c: &Connection, id: i64, version: i64, syncing: bool) -> rusqlite::Result<usize> {
    c.prepare_cached("UPDATE animes SET is_syncing = :syncing, version = :version WHERE _id = :id")?.execute(
        named_params! {":syncing": i64::from(syncing), ":version": version, ":id": id},
    )
}

/// Inserts an anime for restore: the row as given, with `next_update` and `calculate_interval` forced to
/// 0 so the next library update fetches it. Returns the new `_id`.
pub fn insert_anime_for_restore(c: &Connection, a: &AnimeRow) -> rusqlite::Result<i64> {
    let id = sql::insert_anime(c, a)?;
    c.prepare_cached("UPDATE animes SET next_update = 0, calculate_interval = 0 WHERE _id = :id")?
        .execute(named_params! {":id": id})?;
    Ok(id)
}

/// Writes the restored progress of an episode: `seen`, `bookmark`, `fillermark`, the progress seconds,
/// `version` and `memo`. Every other column keeps its stored value, and `is_syncing` is cleared.
#[allow(clippy::too_many_arguments)]
pub fn update_episode_restore(
    c: &Connection,
    id: i64,
    seen: bool,
    bookmark: bool,
    fillermark: bool,
    last_second_seen: i64,
    total_seconds: i64,
    version: i64,
    memo: crate::schema_anime::Memo,
) -> rusqlite::Result<usize> {
    sql::update_episode(
        c,
        &EpisodeUpdate {
            id,
            seen: Some(seen),
            bookmark: Some(bookmark),
            fillermark: Some(fillermark),
            last_second_seen: Some(last_second_seen),
            total_seconds: Some(total_seconds),
            version: Some(version),
            memo: Some(memo),
            ..EpisodeUpdate::default()
        },
    )
}

/// Inserts a category and returns its `_id`.
pub fn insert_category_returning_id(c: &Connection, name: &str, order: i64, flags: i64) -> rusqlite::Result<i64> {
    sql::insert_category(c, name, order, flags)
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::Arc;

    use super::*;
    use crate::runtime::Db;
    use crate::schema_anime;

    fn sample(url: &str) -> AnimeRow {
        AnimeRow { source: 7, url: url.into(), title: "Alpha".into(), favorite: true, ..AnimeRow::default() }
    }

    #[test]
    fn the_backup_wins_only_when_its_version_is_newer() {
        assert_eq!(merge_existing(5, 4), Side::Backup);
        assert_eq!(merge_existing(4, 4), Side::Database);
        assert_eq!(merge_existing(3, 4), Side::Database);
    }

    #[test]
    fn restore_statements_write_what_the_restorer_writes() {
        let c = schema_anime::open_in_memory().unwrap();
        let id = insert_anime_for_restore(&c, &sample("/a")).unwrap();
        let (next, interval): (i64, i64) = c
            .query_row("SELECT next_update, calculate_interval FROM animes WHERE _id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((next, interval), (0, 0));
        assert_eq!(all_source_and_url(&c).unwrap(), vec![(7, "/a".to_owned())]);

        update_anime_syncing(&c, id, 9, true).unwrap();
        let (sync, version): (i64, i64) =
            c.query_row("SELECT is_syncing, version FROM animes WHERE _id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((sync, version), (1, 9), "version written as given, not bumped");
        update_anime_syncing(&c, id, 9, false).unwrap();
        let sync: i64 = c.query_row("SELECT is_syncing FROM animes WHERE _id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(sync, 0);
    }

    #[test]
    fn episode_restore_touches_only_progress_flags_version_and_memo() {
        let c = schema_anime::open_in_memory().unwrap();
        let anime = insert_anime_for_restore(&c, &sample("/a")).unwrap();
        let ep = sql::insert_episode(
            &c,
            &EpisodeRow { anime_id: anime, url: "/e1".into(), name: "Keep me".into(), summary: Some("s".into()), ..EpisodeRow::default() },
        )
        .unwrap();
        update_episode_restore(&c, ep, true, false, true, 120, 1400, 4, crate::schema_anime::Memo::default()).unwrap();
        let (name, summary, seen, fill, secs, version, syncing): (String, Option<String>, i64, i64, i64, i64, i64) = c
            .query_row(
                "SELECT name, summary, seen, fillermark, last_second_seen, version, is_syncing FROM episodes WHERE _id = ?1",
                [ep],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .unwrap();
        assert_eq!(name, "Keep me");
        assert_eq!(summary.as_deref(), Some("s"));
        // The episode trigger adds one to the written version when `seen` changes, as the Kotlin trigger does.
        assert_eq!((seen, fill, secs, version, syncing), (1, 1, 120, 5, 0));
        let history = history_by_episode_url(&c, "/e1").unwrap();
        assert!(history.is_empty());
        assert_eq!(episode_by_url(&c, "/e1").unwrap().len(), 1);
    }

    #[test]
    fn a_crash_in_the_middle_of_a_restore_leaves_no_row_syncing() {
        let dir = tempfile::tempdir().unwrap();
        let path = schema_anime::db_path(dir.path());
        let db = Arc::new(Db::new(&path, schema_anime::open(&path).unwrap()).unwrap());
        let id = db.write(|tx| Ok(insert_anime_for_restore(tx, &sample("/a"))?)).unwrap();
        let crashed = catch_unwind(AssertUnwindSafe(|| {
            db.write(|tx| {
                update_anime_syncing(tx, id, 9, true)?;
                panic!("crash in the middle of the restore");
                #[allow(unreachable_code)]
                Ok(())
            })
        }));
        assert!(crashed.is_err());
        let (sync, version): (i64, i64) = db
            .read(|c| Ok(c.query_row("SELECT is_syncing, version FROM animes WHERE _id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?))
            .unwrap();
        assert_eq!((sync, version), (0, 0), "the transaction rolled back");
    }
}
