//! Reads the library for a pass from the shared database (T-26a-004 and T-26a-005). Rows come from `EntryRepo`
//! (`libraryView` and the season statement), one path for both kinds. Reads run on tokio's blocking pool.

use std::future::Future;
use std::sync::Arc;

use kuro_data::entry::{EntryRepo, LibraryEntry as DataEntry};
use kuro_data::DataResult;
use kuro_db::handle::Db;
use kuro_platform::kind::Kind;

use crate::library_update::error::UpdateError;
use crate::library_update::queue::{FetchType, LibraryEntry};

pub trait LibraryStore: Send + Sync {
    /// Every favourite entry, one row per category it is in.
    fn library(&self, kind: Kind) -> impl Future<Output = Result<Vec<LibraryEntry>, UpdateError>> + Send;
    /// The season children of an anime parent. Empty on a query error, which is logged (Q-16).
    fn season_children(&self, parent_id: i64) -> impl Future<Output = Vec<LibraryEntry>> + Send;
    fn is_favorite(&self, id: i64, kind: Kind) -> impl Future<Output = Result<bool, UpdateError>> + Send;
    /// The category ids the entry is in (empty when it has none).
    fn categories(&self, id: i64, kind: Kind) -> impl Future<Output = Result<Vec<i64>, UpdateError>> + Send;
    /// The numbers of the read chapters or seen episodes that have a recognised number.
    fn read_numbers(&self, id: i64, kind: Kind) -> impl Future<Output = Result<Vec<f64>, UpdateError>> + Send;
}

pub struct SqliteLibraryStore {
    db: Arc<Db>,
}

impl SqliteLibraryStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }
}

/// The pass's row of a library entry of either kind: a manga has no parent and the `Episodes` fetch type.
fn pass_entry(l: &DataEntry) -> LibraryEntry {
    let e = &l.entry;
    LibraryEntry {
        id: e.id,
        source: e.source,
        url: e.url.clone(),
        title: e.title.clone(),
        kind: e.kind,
        category: l.category,
        update_strategy: e.update_strategy,
        status: e.status,
        favorite: e.favorite,
        total: l.total,
        read: l.done,
        next_update_ms: e.next_update,
        fetch_type: e.fetch_type(),
        parent_id: e.parent_id,
        fetch_interval: e.fetch_interval,
        last_update_ms: e.last_update,
    }
}

/// Runs a repository read on the blocking pool.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> DataResult<T> + Send + 'static) -> Result<T, UpdateError> {
    Ok(tokio::task::spawn_blocking(work).await.map_err(|e| UpdateError::Other(e.to_string()))??)
}

impl LibraryStore for SqliteLibraryStore {
    async fn library(&self, kind: Kind) -> Result<Vec<LibraryEntry>, UpdateError> {
        let repo = EntryRepo::new(self.db.clone());
        let rows = blocking(move || repo.library(kind)).await?;
        Ok(rows.iter().map(pass_entry).collect())
    }

    async fn season_children(&self, parent_id: i64) -> Vec<LibraryEntry> {
        let repo = EntryRepo::new(self.db.clone());
        match blocking(move || repo.seasons_by_parent(parent_id)).await {
            Ok(children) => children.iter().map(|c| pass_entry(&c.to_library_entry())).collect(),
            Err(error) => {
                tracing::warn!(parent_id, ?error, "season children query failed; treating as none");
                Vec::new()
            }
        }
    }

    async fn is_favorite(&self, id: i64, kind: Kind) -> Result<bool, UpdateError> {
        let rows = self
            .db
            .read_async(move |c| {
                let mut statement = c.prepare("SELECT favorite FROM entries WHERE _id = ?1 AND kind = ?2")?;
                let mut rows: Vec<bool> = Vec::new();
                for row in statement.query_map((id, kind.as_str()), |r| r.get(0))? {
                    rows.push(row?);
                }
                Ok(rows)
            })
            .await?;
        Ok(rows.first().copied().unwrap_or(false))
    }

    async fn categories(&self, id: i64, kind: Kind) -> Result<Vec<i64>, UpdateError> {
        let ids = self
            .db
            .read_async(move |c| {
                let mut statement = c.prepare("SELECT category_id FROM entry_categories WHERE entry_id = ?1 AND kind = ?2 ORDER BY category_id")?;
                let mut ids: Vec<i64> = Vec::new();
                for row in statement.query_map((id, kind.as_str()), |r| r.get(0))? {
                    ids.push(row?);
                }
                Ok(ids)
            })
            .await?;
        Ok(ids)
    }

    async fn read_numbers(&self, id: i64, kind: Kind) -> Result<Vec<f64>, UpdateError> {
        let numbers = self
            .db
            .read_async(move |c| {
                let mut statement = c.prepare(
                    "SELECT i.number FROM items i JOIN entries e ON e._id = i.entry_id \
                     WHERE i.entry_id = ?1 AND e.kind = ?2 AND i.done = 1 AND i.number >= 0",
                )?;
                let mut numbers: Vec<f64> = Vec::new();
                for row in statement.query_map((id, kind.as_str()), |r| r.get(0))? {
                    numbers.push(row?);
                }
                Ok(numbers)
            })
            .await?;
        Ok(numbers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_db::codec::UpdateStrategy as DbUpdateStrategy;
    use kuro_db::schema_anime::FetchType as DbFetchType;
    use kuro_testkit::{AnimeBuilder, TempDir, TestDb};

    #[test]
    fn stored_update_strategy_maps_zero_and_one() {
        // T-26a-004: 0 ALWAYS_UPDATE, 1 ONLY_FETCH_ONCE.
        assert_eq!(DbUpdateStrategy::from_db(0), DbUpdateStrategy::AlwaysUpdate);
        assert_eq!(DbUpdateStrategy::from_db(1), DbUpdateStrategy::OnlyFetchOnce);
        assert_eq!(DbUpdateStrategy::OnlyFetchOnce.to_db(), 1);
    }

    #[test]
    fn stored_fetch_type_maps_zero_to_seasons_and_one_to_episodes() {
        assert_eq!(DbFetchType::from_db(0), DbFetchType::Seasons);
        assert_eq!(DbFetchType::from_db(1), DbFetchType::Episodes);
        // Shared with the queue model.
        assert_eq!(FetchType::from_db(0), FetchType::Seasons);
    }

    #[tokio::test]
    async fn calculate_interval_round_trips_a_negative_override() {
        // T-26a-004: -3 written, -3 read.
        let dir = TempDir::new().expect("temp dir");
        let db = TestDb::handle(&dir);
        db
            .write(|tx| {
                AnimeBuilder::new().title("Show").source(1).url("/s").favorite(true).with(|a| a.calculate_interval = -3).insert(tx);
                Ok(())
            })
            .expect("insert anime");
        let store = SqliteLibraryStore::new(db);
        let rows = store.library(Kind::Anime).await.expect("library");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].fetch_interval, -3);
        assert_eq!(rows[0].fetch_type, FetchType::Episodes);
    }

    #[tokio::test]
    async fn library_rows_carry_counts_and_favourite_checks_read_correctly() {
        let dir = TempDir::new().expect("temp dir");
        let db = TestDb::handle(&dir);
        db
            .write(|tx| {
                AnimeBuilder::new().title("Fav").source(1).url("/f").favorite(true).insert(tx);
                AnimeBuilder::new().title("NotFav").source(1).url("/n").favorite(false).insert(tx);
                Ok(())
            })
            .expect("insert anime");
        let store = SqliteLibraryStore::new(db.clone());
        let rows = store.library(Kind::Anime).await.expect("library");
        // Only favourites are in the library view.
        assert_eq!(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), vec!["Fav"]);
        let fav_id = rows[0].id;
        assert!(store.is_favorite(fav_id, Kind::Anime).await.expect("favourite read"));
        assert!(!store.is_favorite(999_999, Kind::Anime).await.expect("missing row"));
        assert_eq!(rows[0].unread_or_unseen(), rows[0].total - rows[0].read);
        assert!(store.season_children(999_999).await.is_empty());
    }
}
