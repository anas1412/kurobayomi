//! The anime database: constants defined once, the flag and codec modules under their plan names, and
//! the `AnimeDb` facade the app uses (T-04a-028). Everything else in the crate reads the constants here.

/// The anime file and its schema, shared by `schema_anime` and the `DbKind` of the handle.
pub mod consts {
    /// File name of the anime database inside the data directory.
    pub const FILE_NAME: &str = "anime.db";
    /// `PRAGMA user_version` of the fresh anime schema: the Aniyomi schema 139 plus the native step 139 to 140.
    pub const SCHEMA_VERSION: i64 = 140;
    /// The Aniyomi schema version the baseline SQL is compatible with; files at this version run the native steps.
    pub const BASELINE_VERSION: i64 = 139;
}

/// Flag bit layouts of the anime tables (T-04a-006): the constants, the decoders and `set_flag`.
pub mod flags {
    pub use crate::flags::set_flag;
    pub use crate::schema_anime::flags::*;
}

/// Column codecs of the anime tables (T-04a-005). `StringList` is the Aniyomi name of `GenreList`.
pub mod codec {
    pub use crate::codec::{GenreList, GenreList as StringList, UpdateStrategy};
    pub use crate::schema_anime::{FetchType, Memo, Millis};
}

use std::sync::Arc;
use std::time::Duration;

use kuro_platform::paths::Paths;

use crate::error::DbResult;
use crate::runtime::{Db, DbKind, TxExt};
use crate::statements_anime::{
    self as sql, AnimeUpdate, EpisodeRow, EpisodeUpdate, HistoryEntry, LibraryAnimeRow,
};
use crate::subscribe::Subscription;

/// Quiet time after a commit before a live query runs again, so a burst of writes costs one query
/// (the player writes progress every few seconds while the library is open).
pub const COALESCE: Duration = Duration::from_millis(50);

/// Tables `animelibView` reads.
const LIBRARY_TABLES: &[&str] = &["animes", "episodes", "animehistory", "animes_categories"];
/// Tables `animehistoryView` reads.
const HISTORY_TABLES: &[&str] = &["animes", "episodes", "animehistory"];
const EPISODE_TABLES: &[&str] = &["episodes"];

/// The anime database as the app uses it (`ctx.anime_db`): the shared handle plus the repository
/// calls and live queries of the anime tables. The Kotlin repositories that swallow errors keep that
/// shape here: they log and return `false` or an empty list. Subscriptions need a tokio runtime.
#[derive(Clone)]
pub struct AnimeDb {
    db: Arc<Db>,
}

impl AnimeDb {
    /// Opens `<data_dir>/anime.db`, creating the fresh schema in an empty file.
    pub fn open(paths: &dyn Paths) -> DbResult<Self> {
        Ok(Self { db: Db::open(DbKind::Anime, paths)? })
    }

    /// Wraps a handle on an anime file (the test kit builds those over temporary files).
    pub fn from_handle(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The shared handle, for the statement functions of `statements_anime`.
    pub fn handle(&self) -> &Arc<Db> {
        &self.db
    }

    /// Shutdown: WAL checkpoint and `PRAGMA optimize` (see [`Db::close`]).
    pub fn close(&self) -> DbResult<()> {
        self.db.close()
    }

    /// Library rows now (`animelibView`, one per category membership).
    pub async fn library(&self) -> DbResult<Vec<LibraryAnimeRow>> {
        Ok(self.db.read_async(|c| Ok(sql::library(c)?)).await?)
    }

    /// The library, again after every commit to `animes`, `episodes`, `animehistory` or `animes_categories`.
    pub fn library_stream(&self) -> Subscription<Vec<LibraryAnimeRow>> {
        self.db.subscribe(LIBRARY_TABLES, COALESCE, |c| Ok(sql::library(c)?))
    }

    /// The newest history row of each anime whose title contains `query`, newest first.
    pub fn history_stream(&self, query: &str) -> Subscription<Vec<HistoryEntry>> {
        let query = query.to_owned();
        self.db.subscribe(HISTORY_TABLES, COALESCE, move |c| Ok(sql::history(c, &query)?))
    }

    /// The episodes of one anime by `source_order`, then `_id` (the statement leaves the order open).
    pub fn episodes_by_anime_stream(&self, anime_id: i64) -> Subscription<Vec<EpisodeRow>> {
        self.db.subscribe(EPISODE_TABLES, COALESCE, move |c| {
            let mut rows = sql::episodes_by_anime_id(c, anime_id)?;
            rows.sort_by_key(|e| (e.source_order, e.id));
            Ok(rows)
        })
    }

    /// `updateAllAnime`: one transaction, `false` after logging when any update fails.
    pub async fn update_all_anime(&self, updates: Vec<AnimeUpdate>) -> bool {
        self.swallow(false, move |tx| Ok(sql::update_all_anime(tx, &updates))).await
    }

    /// `addAllEpisodes`: one transaction, each episode back with its new `_id`; empty after logging on failure.
    pub async fn add_all_episodes(&self, episodes: Vec<EpisodeRow>) -> Vec<EpisodeRow> {
        self.swallow(Vec::new(), move |tx| Ok(sql::add_all_episodes(tx, &episodes))).await
    }

    /// `updateAllEpisodes`: the coalesce update of a list in one transaction. Returns the rows written.
    pub async fn update_all_episodes(&self, updates: Vec<EpisodeUpdate>) -> DbResult<usize> {
        self.db.write_async(move |tx| Ok(sql::update_all_episodes(tx, &updates)?)).await
    }

    /// `removeEpisodesWithIds`: deletes in chunks of 500 ids; an error is logged and swallowed.
    pub async fn remove_episodes_with_ids(&self, ids: Vec<i64>) {
        self.swallow((), move |tx| {
            if let Err(err) = sql::remove_episodes_with_ids(tx, &ids) {
                tracing::error!(error = %err, "remove_episodes_with_ids failed");
            }
            Ok(())
        })
        .await;
    }

    /// Moves the history row of an episode to `seen_at_ms`, creating it when there is none.
    pub async fn upsert_history(&self, episode_id: i64, seen_at_ms: i64) -> DbResult<()> {
        self.db
            .write_async(move |tx| {
                sql::upsert_history(tx, episode_id, crate::schema_anime::Millis(seen_at_ms))?;
                Ok(())
            })
            .await
    }

    /// `deleteAllAnimeHistory`: `true` on success, `false` after logging on error. Reports `animehistory`
    /// itself (`touch`), because a `DELETE` without `WHERE` can take SQLite's truncate optimisation.
    pub async fn delete_all_history(&self) -> bool {
        self.swallow(false, |tx| {
            let done = sql::delete_all_history_logged(tx);
            if done {
                tx.touch(&["animehistory"])?;
            }
            Ok(done)
        })
        .await
    }

    /// Runs `f` as one write; a failure of the write itself is logged and `fallback` returned.
    async fn swallow<T, F>(&self, fallback: T, f: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(&rusqlite::Transaction<'_>) -> DbResult<T> + Send + 'static,
    {
        match self.db.write_async(f).await {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(error = %err, "anime write failed");
                fallback
            }
        }
    }
}
