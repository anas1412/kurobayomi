//! SQLite store for the remove-after-watched list (T-24a-032, data 5.2; port of `AnimeDownloadPendingDeleter`).
//! Episodes are deduplicated by id. [`AnimePendingStore::take_for_installed`] removes only the rows whose source is
//! installed, so a missing source keeps its rows for the next flush (E-P01). Only the queue actor calls it.

use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, params};

use crate::anime::model::{AnimeRef, EpisodeRef};
use crate::anime::store::AnimeStoreError;

/// DDL for `anime_download_pending_delete` and its `anime_download_pending_corrupt` log table.
pub const SCHEMA: &str = include_str!("../../sql/anime_download_pending_delete.sql");

/// The episodes pending for one anime, in insertion order. Only the fields the delete needs are stored; the rest of
/// each [`EpisodeRef`] is left at its default.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingGroup {
    pub anime: AnimeRef,
    pub episodes: Vec<EpisodeRef>,
}

pub struct AnimePendingStore {
    conn: Mutex<Connection>,
}

impl AnimePendingStore {
    pub fn open(conn: Connection) -> Result<Self, AnimeStoreError> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn open_in_memory() -> Result<Self, AnimeStoreError> {
        Self::open(Connection::open_in_memory()?)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, AnimeStoreError> {
        self.conn.lock().map_err(|_| AnimeStoreError::Poisoned)
    }

    /// `addEpisodes`: inserts the episodes not stored yet, deduplicated within the batch too. Returns how many were added.
    pub fn add_episodes(&self, anime: &AnimeRef, episodes: &[EpisodeRef]) -> Result<usize, AnimeStoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let mut seq: i64 = tx.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM anime_download_pending_delete WHERE anime_id = ?1",
            params![anime.id],
            |row| row.get(0),
        )?;
        let mut added = 0;
        for e in episodes {
            let n = tx.execute(
                "INSERT OR IGNORE INTO anime_download_pending_delete(episode_id, anime_id, episode_url, episode_name, \
                 scanlator, seen, bookmark, anime_url, anime_title, anime_source, seq) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    e.id,
                    anime.id,
                    e.url,
                    e.name,
                    e.scanlator,
                    e.seen,
                    e.bookmark,
                    anime.url,
                    anime.title,
                    anime.source_id,
                    seq
                ],
            )?;
            if n > 0 {
                seq += 1;
                added += n;
            }
        }
        tx.commit()?;
        Ok(added)
    }

    /// `getPendingEpisodes`: returns the groups (by anime id, then insertion order) of the rows whose source `installed`
    /// accepts, and deletes exactly those rows in the same transaction. Rows of other sources stay (E-P01). A row that
    /// cannot be decoded moves to `anime_download_pending_corrupt` and is logged, never silently cleared (E-P04).
    pub fn take_for_installed(&self, installed: impl Fn(i64) -> bool) -> Result<Vec<PendingGroup>, AnimeStoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let rows: Vec<(i64, rusqlite::Result<(AnimeRef, EpisodeRef)>)> = {
            let mut stmt = tx.prepare(
                "SELECT rowid, episode_id, anime_id, episode_url, episode_name, scanlator, anime_url, anime_title, \
                 anime_source, seen, bookmark FROM anime_download_pending_delete ORDER BY anime_id, seq",
            )?;
            let mapped = stmt.query_map([], |row| {
                let rowid: i64 = row.get(0)?;
                let decoded = (|| {
                    let anime = AnimeRef { id: row.get(2)?, source_id: row.get(8)?, url: row.get(6)?, title: row.get(7)? };
                    let episode = EpisodeRef {
                        id: row.get(1)?,
                        anime_id: row.get(2)?,
                        url: row.get(3)?,
                        name: row.get(4)?,
                        scanlator: row.get(5)?,
                        seen: row.get(9)?,
                        bookmark: row.get(10)?,
                        fillermark: false,
                        source_order: 0,
                        upload_date: 0,
                        episode_number: 0.0,
                    };
                    Ok((anime, episode))
                })();
                Ok((rowid, decoded))
            })?;
            mapped.collect::<Result<_, _>>()?
        };
        let mut groups: Vec<PendingGroup> = Vec::new();
        for (rowid, decoded) in rows {
            let (anime, episode) = match decoded {
                Ok(pair) => pair,
                Err(err) => {
                    tracing::error!(target: "kuro::downloads", rowid, error = %err, "pending delete row is corrupt, moved aside");
                    tx.execute(
                        "INSERT INTO anime_download_pending_corrupt(source_rowid, reason, recorded_at) \
                         VALUES (?1, ?2, strftime('%s','now'))",
                        params![rowid, err.to_string()],
                    )?;
                    tx.execute("DELETE FROM anime_download_pending_delete WHERE rowid = ?1", params![rowid])?;
                    continue;
                }
            };
            if !installed(anime.source_id) {
                continue;
            }
            tx.execute("DELETE FROM anime_download_pending_delete WHERE rowid = ?1", params![rowid])?;
            match groups.last_mut() {
                Some(g) if g.anime.id == anime.id => g.episodes.push(episode),
                _ => groups.push(PendingGroup { anime, episodes: vec![episode] }),
            }
        }
        tx.commit()?;
        Ok(groups)
    }

    /// Number of stored rows. Used by tests.
    pub fn len(&self) -> Result<usize, AnimeStoreError> {
        Ok(self.lock()?.query_row("SELECT COUNT(*) FROM anime_download_pending_delete", [], |r| r.get::<_, i64>(0))? as usize)
    }

    pub fn is_empty(&self) -> Result<bool, AnimeStoreError> {
        Ok(self.len()? == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(id: i64, source_id: i64) -> AnimeRef {
        AnimeRef { id, source_id, url: format!("/a/{id}"), title: format!("Anime {id}") }
    }

    fn ep(id: i64, anime_id: i64) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id,
            url: format!("/e/{id}"),
            name: format!("Ep {id}"),
            scanlator: Some("grp".into()),
            seen: true,
            bookmark: false,
            fillermark: false,
            source_order: id,
            upload_date: 0,
            episode_number: 0.0,
        }
    }

    #[test]
    fn add_episodes_dedupes_within_the_batch_and_against_stored_rows() {
        let s = AnimePendingStore::open_in_memory().unwrap();
        let a = show(1, 10);
        assert_eq!(s.add_episodes(&a, &[ep(1, 1), ep(1, 1), ep(2, 1)]).unwrap(), 2);
        assert_eq!(s.add_episodes(&a, &[ep(2, 1)]).unwrap(), 0, "already stored");
        assert_eq!(s.len().unwrap(), 2);
    }

    #[test]
    fn take_keeps_rows_of_uninstalled_sources_and_groups_in_insertion_order() {
        let s = AnimePendingStore::open_in_memory().unwrap();
        s.add_episodes(&show(2, 20), &[ep(5, 2)]).unwrap();
        s.add_episodes(&show(1, 10), &[ep(3, 1), ep(4, 1)]).unwrap();
        s.add_episodes(&show(1, 10), &[ep(1, 1)]).unwrap();
        let groups = s.take_for_installed(|source| source == 10).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].anime, show(1, 10));
        let ids: Vec<i64> = groups[0].episodes.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![3, 4, 1]);
        assert_eq!(groups[0].episodes[0].scanlator.as_deref(), Some("grp"));
        assert_eq!(s.len().unwrap(), 1, "the row of the uninstalled source stays (E-P01)");
        assert_eq!(s.take_for_installed(|_| true).unwrap()[0].episodes[0].id, 5);
        assert!(s.is_empty().unwrap());
    }

    #[test]
    fn a_corrupt_row_moves_to_the_corrupt_table_and_is_not_cleared() {
        let s = AnimePendingStore::open_in_memory().unwrap();
        s.add_episodes(&show(1, 10), &[ep(1, 1), ep(2, 1)]).unwrap();
        s.conn
            .lock()
            .unwrap()
            .execute("UPDATE anime_download_pending_delete SET seen = 'yes' WHERE episode_id = 1", [])
            .unwrap();
        let groups = s.take_for_installed(|_| true).unwrap();
        assert_eq!(groups[0].episodes.iter().map(|e| e.id).collect::<Vec<_>>(), vec![2], "the good row is taken");
        assert!(s.is_empty().unwrap(), "the corrupt row leaves the pending list");
        let kept: i64 = s
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM anime_download_pending_corrupt", [], |r| r.get(0))
            .unwrap();
        assert_eq!(kept, 1, "the corrupt row is logged in its own table (E-P04)");
    }
}
