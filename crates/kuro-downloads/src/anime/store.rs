//! SQLite store for the anime queue (T-24a-009, data 5.1). The manager writes the whole queue after each membership or
//! order change, so `ord` stays dense. Only the queue actor calls it.

use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, params};

/// DDL for `anime_download_queue`.
pub const SCHEMA: &str = include_str!("../../sql/anime_download_queue.sql");

#[derive(Debug, thiserror::Error)]
pub enum AnimeStoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error("anime queue store lock was poisoned")]
    Poisoned,
}

pub struct AnimeQueueStore {
    conn: Mutex<Connection>,
}

impl AnimeQueueStore {
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

    /// Inserts or replaces `(episode_id, anime_id, ord)` rows in one transaction.
    pub fn add_all(&self, rows: &[(i64, i64, i64)]) -> Result<(), AnimeStoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        for (episode_id, anime_id, ord) in rows {
            tx.execute(
                "INSERT OR REPLACE INTO anime_download_queue(episode_id, anime_id, ord) VALUES (?1, ?2, ?3)",
                params![episode_id, anime_id, ord],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove(&self, episode_id: i64) -> Result<(), AnimeStoreError> {
        self.lock()?.execute("DELETE FROM anime_download_queue WHERE episode_id = ?1", params![episode_id])?;
        Ok(())
    }

    pub fn remove_all(&self, episode_ids: &[i64]) -> Result<(), AnimeStoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        for id in episode_ids {
            tx.execute("DELETE FROM anime_download_queue WHERE episode_id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), AnimeStoreError> {
        self.lock()?.execute("DELETE FROM anime_download_queue", [])?;
        Ok(())
    }

    /// Rewrites the whole queue in one transaction, in the given order (`ord` = position).
    pub fn replace_all(&self, episode_ids: &[(i64, i64)]) -> Result<(), AnimeStoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM anime_download_queue", [])?;
        for (ord, (episode_id, anime_id)) in episode_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO anime_download_queue(episode_id, anime_id, ord) VALUES (?1, ?2, ?3)",
                params![episode_id, anime_id, ord as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Startup restore (data 5.1, E-Q02, E-Q03). Reads the rows in `ord` order and keeps those for which
    /// `keep(anime_id, episode_id)` is true. The rest are deleted. The kept rows are rewritten in one transaction with
    /// `ord` from 0, and their `(anime_id, episode_id)` pairs are returned in queue order. A row whose source is not
    /// installed is kept by the caller's `keep`, so it is not lost.
    pub fn restore(&self, keep: impl Fn(i64, i64) -> bool) -> Result<Vec<(i64, i64)>, AnimeStoreError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let rows: Vec<(i64, i64)> = {
            let mut stmt = tx.prepare("SELECT episode_id, anime_id FROM anime_download_queue ORDER BY ord ASC")?;
            let mapped = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;
            mapped.collect::<Result<_, _>>()?
        };
        tx.execute("DELETE FROM anime_download_queue", [])?;
        let mut kept = Vec::new();
        for (episode_id, anime_id) in rows {
            if keep(anime_id, episode_id) {
                tx.execute(
                    "INSERT INTO anime_download_queue(episode_id, anime_id, ord) VALUES (?1, ?2, ?3)",
                    params![episode_id, anime_id, kept.len() as i64],
                )?;
                kept.push((anime_id, episode_id));
            }
        }
        tx.commit()?;
        Ok(kept)
    }

    /// Every stored `(episode_id, anime_id, ord)` in order. Used by tests and by the manager's diagnostics.
    pub fn rows(&self) -> Result<Vec<(i64, i64, i64)>, AnimeStoreError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT episode_id, anime_id, ord FROM anime_download_queue ORDER BY ord ASC")?;
        let mapped = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(mapped.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> AnimeQueueStore {
        AnimeQueueStore::open_in_memory().unwrap()
    }

    #[test]
    fn add_remove_and_clear_follow_the_table_operations() {
        let s = store();
        s.add_all(&[(1, 10, 0), (2, 10, 1), (3, 20, 2)]).unwrap();
        assert_eq!(s.rows().unwrap(), vec![(1, 10, 0), (2, 10, 1), (3, 20, 2)]);
        s.remove(2).unwrap();
        s.remove_all(&[1, 99]).unwrap();
        assert_eq!(s.rows().unwrap(), vec![(3, 20, 2)]);
        s.add_all(&[(3, 20, 5)]).unwrap();
        assert_eq!(s.rows().unwrap(), vec![(3, 20, 5)], "INSERT OR REPLACE keeps one row per episode");
        s.clear().unwrap();
        assert!(s.rows().unwrap().is_empty());
    }

    #[test]
    fn restore_keeps_order_drops_missing_rows_and_renumbers() {
        let s = store();
        s.add_all(&[(1, 10, 7), (2, 11, 3), (3, 12, 9), (4, 10, 4)]).unwrap();
        // Episode 2's anime is gone (E-Q03). The others resolve.
        let kept = s.restore(|anime, episode| !(anime == 11 && episode == 2)).unwrap();
        assert_eq!(kept, vec![(10, 4), (10, 1), (12, 3)], "ord order: 3, 4, 7, 9 minus the dropped row");
        assert_eq!(s.rows().unwrap(), vec![(4, 10, 0), (1, 10, 1), (3, 12, 2)], "fresh ord values from 0");
    }

    #[test]
    fn replace_all_writes_positions_in_one_go() {
        let s = store();
        s.add_all(&[(1, 10, 0), (2, 10, 1)]).unwrap();
        s.replace_all(&[(2, 10), (1, 10)]).unwrap();
        assert_eq!(s.rows().unwrap(), vec![(2, 10, 0), (1, 10, 1)]);
    }

    #[test]
    fn restore_keeps_rows_whose_source_is_not_installed() {
        // E-Q02: the caller's predicate only checks anime and episode, so an uninstalled source does not drop the row.
        let s = store();
        s.add_all(&[(5, 50, 0)]).unwrap();
        assert_eq!(s.restore(|_, _| true).unwrap(), vec![(50, 5)]);
        assert_eq!(s.rows().unwrap(), vec![(5, 50, 0)]);
    }
}
