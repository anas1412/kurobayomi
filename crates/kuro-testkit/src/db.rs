//! In-memory anime database for tests, and builders for its rows. The schema, the statements and
//! the row types are `kuro-db`'s. The builders start from the defaults of Aniyomi's
//! `Anime.create()`, `Episode.create()` and `AnimeHistory.create()`, so a test states only the
//! columns it reads. Builder inputs are plain Rust types, so they do not depend on which copy of
//! `kuro-db` a test links.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::open::BUSY_TIMEOUT;
use kuro_db::schema_anime::{self, Millis};
use kuro_db::statements_anime::{
    AnimeRow, EpisodeRow, insert_anime, insert_episode, upsert_history,
};
use rusqlite::Connection;

use crate::TempDir;

/// The anime database in memory.
pub struct TestDb;

impl TestDb {
    /// A fresh anime database at `user_version` 139 with every table, index, trigger and view of the
    /// anime schema. It has the foreign keys, busy timeout and synchronous mode of a file database.
    /// WAL does not apply to memory databases, so `journal_mode` stays `memory`.
    pub fn anime() -> Connection {
        let c = schema_anime::open_in_memory().expect("fresh anime schema");
        c.busy_timeout(BUSY_TIMEOUT).expect("busy timeout");
        c.pragma_update(None, "synchronous", "NORMAL")
            .expect("synchronous");
        c
    }

    /// A migrated manga database behind the shared handle `kuro_db::handle::Db`, in a file under `dir`. A `Db` reads
    /// through its own connections, so unlike [`TestDb::anime`] it cannot be a memory database. Keep `dir` alive.
    pub fn manga_handle(dir: &TempDir) -> Arc<Db> {
        let path = dir.path().join("tachiyomi.db");
        let writer = kuro_db::schema_manga::open(&path).expect("manga schema");
        Arc::new(Db::new(&path, writer).expect("manga handle"))
    }

    /// A migrated anime database behind the shared handle, in a file under `dir`. Keep `dir` alive.
    pub fn anime_handle(dir: &TempDir) -> Arc<Db> {
        let path = dir.path().join("tachiyomi.animedb");
        let writer = schema_anime::open(&path).expect("anime schema");
        Arc::new(Db::new(&path, writer).expect("anime handle"))
    }
}

/// One `animes` row. [`AnimeBuilder::new`] starts from `Anime.create()`.
#[derive(Clone, Debug)]
pub struct AnimeBuilder(AnimeRow);

impl AnimeBuilder {
    pub fn new() -> Self {
        Self(AnimeRow {
            id: -1,
            source: -1,
            season_number: -1.0,
            ..AnimeRow::default()
        })
    }

    pub fn url(mut self, url: &str) -> Self {
        self.0.url = url.to_owned();
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.0.title = title.to_owned();
        self
    }

    pub fn source(mut self, source: i64) -> Self {
        self.0.source = source;
        self
    }

    pub fn favorite(mut self, favorite: bool) -> Self {
        self.0.favorite = favorite;
        self
    }

    /// Overrides any other column, e.g. `with(|a| a.viewer = 2)`.
    pub fn with(mut self, set: impl FnOnce(&mut AnimeRow)) -> Self {
        set(&mut self.0);
        self
    }

    pub fn build(self) -> AnimeRow {
        self.0
    }

    /// Inserts the row and returns its `_id`.
    pub fn insert(self, c: &Connection) -> i64 {
        insert_anime(c, &self.0).expect("insert anime")
    }
}

impl Default for AnimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// One `episodes` row. [`EpisodeBuilder::new`] starts from `Episode.create()`.
#[derive(Clone, Debug)]
pub struct EpisodeBuilder(EpisodeRow);

impl EpisodeBuilder {
    /// An episode of `anime_id`, which must exist when the row is inserted.
    pub fn new(anime_id: i64) -> Self {
        Self(EpisodeRow {
            id: -1,
            anime_id,
            episode_number: -1.0,
            date_upload: -1,
            version: 1,
            ..EpisodeRow::default()
        })
    }

    pub fn url(mut self, url: &str) -> Self {
        self.0.url = url.to_owned();
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.0.name = name.to_owned();
        self
    }

    pub fn episode_number(mut self, number: f64) -> Self {
        self.0.episode_number = number;
        self
    }

    pub fn seen(mut self, seen: bool) -> Self {
        self.0.seen = seen;
        self
    }

    pub fn bookmark(mut self, bookmark: bool) -> Self {
        self.0.bookmark = bookmark;
        self
    }

    /// Overrides any other column, e.g. `with(|e| e.total_seconds = 1440)`.
    pub fn with(mut self, set: impl FnOnce(&mut EpisodeRow)) -> Self {
        set(&mut self.0);
        self
    }

    pub fn build(self) -> EpisodeRow {
        self.0
    }

    /// Inserts the row and returns its `_id`.
    pub fn insert(self, c: &Connection) -> i64 {
        insert_episode(c, &self.0).expect("insert episode")
    }
}

/// One `animehistory` row of an episode. Inserting again for the same episode moves its
/// `last_seen`, as the statement does.
#[derive(Clone, Debug)]
pub struct HistoryBuilder {
    episode_id: i64,
    seen_at: i64,
}

impl HistoryBuilder {
    /// History of `episode_id`. `seen_at` starts at 0, which is `AnimeHistory.create()`'s "no time"
    /// and the value the history views skip.
    pub fn new(episode_id: i64) -> Self {
        Self {
            episode_id,
            seen_at: 0,
        }
    }

    /// Epoch milliseconds of the last time the episode was seen.
    pub fn seen_at(mut self, ms: i64) -> Self {
        self.seen_at = ms;
        self
    }

    pub fn insert(self, c: &Connection) {
        upsert_history(c, self.episode_id, Millis(self.seen_at)).expect("upsert history");
    }
}

#[cfg(test)]
mod tests {
    use kuro_db::schema_anime::{ANIME_APPLICATION_ID, ANIME_SCHEMA, ANIME_SCHEMA_VERSION};
    use kuro_db::statements_anime::{anime_by_id, episode_by_id};

    use super::*;

    /// The names after each `keyword` at the start of a line, e.g. `CREATE TABLE ` in the schema.
    fn names_after(sql: &str, keyword: &str) -> Vec<String> {
        sql.lines()
            .filter_map(|line| line.strip_prefix(keyword))
            .map(|rest| {
                rest.chars()
                    .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                    .collect()
            })
            .collect()
    }

    #[test]
    fn every_table_and_view_of_the_anime_schema_exists() {
        let tables = names_after(ANIME_SCHEMA.baseline_sql, "CREATE TABLE ");
        let views = names_after(ANIME_SCHEMA.baseline_sql, "CREATE VIEW ");
        assert_eq!(tables.len(), 10, "tables in schema.sql: {tables:?}");
        assert_eq!(views.len(), 8, "views in views.sql: {views:?}");

        let c = TestDb::anime();
        for (kind, names) in [("table", &tables), ("view", &views)] {
            for name in names {
                let found: i64 = c
                    .query_row(
                        "SELECT count(*) FROM sqlite_master WHERE type = ?1 AND name = ?2",
                        [kind, name.as_str()],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(found, 1, "{kind} {name} is missing");
            }
        }
    }

    #[test]
    fn test_db_has_the_file_pragmas_and_the_fresh_version() {
        let c = TestDb::anime();
        let pragma =
            |name: &str| -> i64 { c.pragma_query_value(None, name, |r| r.get(0)).unwrap() };
        assert_eq!(pragma("foreign_keys"), 1);
        assert_eq!(pragma("synchronous"), 1, "NORMAL");
        assert_eq!(pragma("busy_timeout"), 5000);
        assert_eq!(pragma("user_version"), ANIME_SCHEMA_VERSION);
        assert_eq!(pragma("application_id"), ANIME_APPLICATION_ID);
    }

    #[test]
    fn foreign_keys_refuse_an_episode_without_its_anime() {
        let c = TestDb::anime();
        let orphan = EpisodeBuilder::new(999).build();
        assert!(insert_episode(&c, &orphan).is_err());
    }

    #[test]
    fn anime_defaults_are_anime_create_and_the_row_round_trips() {
        let c = TestDb::anime();
        let id = AnimeBuilder::new().insert(&c);
        let row = anime_by_id(&c, id).unwrap().unwrap();
        assert_eq!(
            (
                row.source,
                row.status,
                row.favorite,
                row.initialized,
                row.version
            ),
            (-1, 0, false, false, 0)
        );
        assert_eq!(row.season_number, -1.0);
        assert_eq!(row.fetch_type, schema_anime::FetchType::Episodes);

        let mut expected = AnimeBuilder::new().build();
        expected.id = id;
        assert_eq!(row, expected);
    }

    #[test]
    fn anime_overrides_replace_only_the_named_columns() {
        let c = TestDb::anime();
        let id = AnimeBuilder::new()
            .url("/one")
            .title("One")
            .source(7)
            .favorite(true)
            .with(|a| a.viewer = 2)
            .insert(&c);
        let row = anime_by_id(&c, id).unwrap().unwrap();
        assert_eq!(
            (
                row.url.as_str(),
                row.title.as_str(),
                row.source,
                row.favorite,
                row.viewer
            ),
            ("/one", "One", 7, true, 2)
        );
        assert_eq!((row.initialized, row.season_number), (false, -1.0));
    }

    #[test]
    fn episode_defaults_are_episode_create_and_the_row_round_trips() {
        let c = TestDb::anime();
        let anime = AnimeBuilder::new().insert(&c);
        let id = EpisodeBuilder::new(anime).insert(&c);
        let row = episode_by_id(&c, id).unwrap().unwrap();
        assert_eq!(
            (row.episode_number, row.date_upload, row.version, row.seen),
            (-1.0, -1, 1, false)
        );

        let mut expected = EpisodeBuilder::new(anime).build();
        expected.id = id;
        assert_eq!(row, expected);
    }

    #[test]
    fn episode_overrides_are_stored() {
        let c = TestDb::anime();
        let anime = AnimeBuilder::new().insert(&c);
        let id = EpisodeBuilder::new(anime)
            .url("/ep-3")
            .name("Episode 3")
            .episode_number(3.0)
            .seen(true)
            .bookmark(true)
            .with(|e| e.total_seconds = 1440)
            .insert(&c);
        let row = episode_by_id(&c, id).unwrap().unwrap();
        assert_eq!(
            (
                row.url.as_str(),
                row.name.as_str(),
                row.episode_number,
                row.seen,
                row.bookmark,
                row.total_seconds
            ),
            ("/ep-3", "Episode 3", 3.0, true, true, 1440)
        );
    }

    #[test]
    fn history_builder_inserts_then_moves_last_seen() {
        let c = TestDb::anime();
        let anime = AnimeBuilder::new().insert(&c);
        let episode = EpisodeBuilder::new(anime).insert(&c);
        HistoryBuilder::new(episode).seen_at(5000).insert(&c);
        HistoryBuilder::new(episode).seen_at(9000).insert(&c);

        let rows: Vec<(i64, i64)> = c
            .prepare("SELECT episode_id, last_seen FROM animehistory")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(rows, vec![(episode, 9000)]);
    }
}
