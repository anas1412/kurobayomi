//! Test fixtures for the anime kind: an in-memory unified database with the production pragmas, and
//! builders for anime `entries`, `items` and `history` rows. The defaults are those of
//! `Anime.create()` and `Episode.create()` (p1-edge-cases-and-tests.md section 3), so a test sets
//! only the columns it reads.

use rusqlite::Connection;

use crate::open::BUSY_TIMEOUT;
use crate::schema_anime::Millis;
use crate::schema_unified;
use crate::statements_anime::{AnimeRow, EpisodeRow, insert_anime, insert_episode, upsert_history};

/// The unified schema in a fresh in-memory database.
pub struct TestDb;

impl TestDb {
    /// The fresh unified schema with the busy timeout and synchronous mode of a file database.
    pub fn anime() -> Connection {
        let c = schema_unified::open_in_memory().expect("fresh unified schema");
        c.busy_timeout(BUSY_TIMEOUT).expect("busy timeout");
        c.pragma_update(None, "synchronous", "NORMAL")
            .expect("synchronous");
        c
    }
}

/// One `animes` row with the `Anime.create()` defaults.
pub struct AnimeBuilder(AnimeRow);

impl AnimeBuilder {
    pub fn new(url: &str) -> Self {
        Self(AnimeRow {
            source: 1,
            url: url.into(),
            title: format!("T {url}"),
            status: 1,
            initialized: true,
            season_number: -1.0,
            ..Default::default()
        })
    }

    pub fn insert(self, c: &Connection) -> i64 {
        insert_anime(c, &self.0).expect("insert anime")
    }
}

/// One `episodes` row of `anime_id` with the `Episode.create()` defaults.
pub struct EpisodeBuilder(EpisodeRow);

impl EpisodeBuilder {
    pub fn new(anime_id: i64, url: &str) -> Self {
        Self(EpisodeRow {
            anime_id,
            url: url.into(),
            name: format!("E {url}"),
            episode_number: 1.0,
            ..Default::default()
        })
    }

    pub fn insert(self, c: &Connection) -> i64 {
        insert_episode(c, &self.0).expect("insert episode")
    }
}

/// One `animehistory` row for `episode_id`, seen at `seen_at` (epoch milliseconds).
pub struct HistoryBuilder {
    episode_id: i64,
    seen_at: i64,
}

impl HistoryBuilder {
    pub fn new(episode_id: i64) -> Self {
        Self {
            episode_id,
            seen_at: 1000,
        }
    }

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
    use super::*;

    #[test]
    fn builders_fill_every_not_null_column_with_the_create_defaults() {
        let c = TestDb::anime();
        let anime = AnimeBuilder::new("/a").insert(&c);
        let episode = EpisodeBuilder::new(anime, "/e1").insert(&c);
        HistoryBuilder::new(episode).seen_at(5000).insert(&c);

        let anime_defaults: (i64, i64, i64, i64, i64, i64, i64, f64, i64, i64, String) = c
            .query_row(
                "SELECT source, status, initialized, viewer, item_flags, cover_last_modified,
                    season_flags, season_number, season_source_order, background_last_modified,
                    CAST(memo AS TEXT)
                 FROM entries WHERE _id = ?1 AND kind = 'anime'",
                [anime],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?,
                        r.get(9)?,
                        r.get(10)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            anime_defaults,
            (1, 1, 1, 0, 0, 0, 0, -1.0, 0, 0, "{}".to_owned())
        );

        let episode_defaults: (i64, i64, i64, i64) = c
            .query_row(
                "SELECT progress, total_seconds, source_order, date_fetch FROM items WHERE _id = ?1",
                [episode],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(episode_defaults, (0, 0, 0, 0));

        let seen: i64 = c
            .query_row(
                "SELECT last_at FROM history WHERE item_id = ?1",
                [episode],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(seen, 5000);
    }
}
