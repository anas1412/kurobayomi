//! Anime SQL views (T-04a-004): the eight views of Aniyomi `view/*.sq` in dependency order, embedded
//! from `sql/anime/views.sql`. `schema_anime::open` creates them after the tables.
//!
//! The view text is the Aniyomi text. Columns are read by name, never by position, so a view that
//! gains a column does not shift the readers.

/// `CREATE VIEW` statements of the eight anime views.
pub const VIEWS_SQL: &str = include_str!("../sql/anime/views.sql");

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use crate::schema_anime::open_in_memory;
    use crate::statements_anime::{recent_updates, updates_by_seen, upcoming_animes, AnimeUpdateRow};

    /// Inserts an anime with the fixture defaults of `p1-edge-cases-and-tests.md` section 3.
    fn add_anime(c: &Connection, id: i64, favorite: i64, fetch_type: i64, parent_id: Option<i64>, date_added: i64) {
        c.execute(
            "INSERT INTO animes(_id, source, url, title, status, favorite, initialized, viewer,
                episode_flags, cover_last_modified, date_added, season_flags, season_number,
                season_source_order, background_last_modified, fetch_type, parent_id)
             VALUES (?1, 1, ?2, ?3, 1, ?4, 1, 0, 0, 0, ?5, 0, -1.0, 0, 0, ?6, ?7)",
            params![id, format!("u{id}"), format!("T{id}"), favorite, date_added, fetch_type, parent_id],
        )
        .unwrap();
    }

    /// Inserts an episode whose `episode_number` is its id. Sets `date_fetch`, `date_upload`, `seen`
    /// and `bookmark`.
    fn add_episode(c: &Connection, id: i64, anime_id: i64, date_fetch: i64, date_upload: i64, seen: i64, bookmark: i64) {
        c.execute(
            "INSERT INTO episodes(_id, anime_id, url, name, seen, bookmark, last_second_seen,
                total_seconds, episode_number, source_order, date_fetch, date_upload, fillermark)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, 0, ?8, ?9, 0)",
            params![id, anime_id, format!("e{id}"), format!("E{id}"), seen, bookmark, id as f64, date_fetch, date_upload],
        )
        .unwrap();
    }

    fn add_history(c: &Connection, episode_id: i64, seen_at: i64) {
        c.execute("INSERT INTO animehistory(episode_id, last_seen) VALUES (?1, ?2)", params![episode_id, seen_at])
            .unwrap();
    }

    /// `(totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen, bookmarkCount,
    /// fillermarkCount, category)`.
    type LibraryRow = (i64, i64, i64, i64, i64, i64, i64, i64);

    /// Every `animelibView` row of `anime_id`, ordered by category.
    fn library_rows(c: &Connection, anime_id: i64) -> Vec<LibraryRow> {
        let mut st = c
            .prepare(
                "SELECT totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen,
                    bookmarkCount, fillermarkCount, category
                 FROM animelibView WHERE _id = ?1 ORDER BY category",
            )
            .unwrap();
        st.query_map([anime_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    }

    /// Anime 1 `Alpha` (favorite, episodes) with episodes 1 to 3 and history 1000, 3000, 2000.
    fn fixture_v1(c: &Connection) {
        add_anime(c, 1, 1, 1, None, 100);
        add_episode(c, 1, 1, 200, 150, 1, 0);
        add_episode(c, 2, 1, 200, 150, 1, 0);
        add_episode(c, 3, 1, 200, 150, 0, 0);
        add_history(c, 1, 1000);
        add_history(c, 2, 3000);
        add_history(c, 3, 2000);
    }

    #[test]
    fn library_row_of_an_entry_with_episodes_and_history() {
        let c = open_in_memory().unwrap();
        fixture_v1(&c);
        assert_eq!(library_rows(&c, 1), vec![(3, 2, 150, 200, 3000, 0, 0, 0)]);
    }

    #[test]
    fn season_parent_counts_children_and_empty_seasons_as_fully_seen() {
        let c = open_in_memory().unwrap();
        add_anime(&c, 10, 1, 0, None, 100);
        add_anime(&c, 11, 0, 1, Some(10), 100);
        add_anime(&c, 12, 0, 1, Some(10), 100);
        add_anime(&c, 13, 0, 1, Some(10), 100);
        add_episode(&c, 20, 11, 200, 150, 1, 0);
        add_episode(&c, 21, 11, 200, 150, 1, 0);
        add_episode(&c, 22, 12, 200, 150, 1, 0);
        add_episode(&c, 23, 12, 200, 150, 0, 1);
        // Children 11 and 12 get the same episode dates; child 13 has no episodes.
        assert_eq!(library_rows(&c, 10), vec![(3, 2, 150, 200, 0, 1, 0, 0)]);
        let mut st = c
            .prepare("SELECT _id, totalCount, seenCount, bookmarkCount FROM animeseasonsView WHERE parent_id = 10 ORDER BY _id")
            .unwrap();
        let seasons: Vec<(i64, i64, i64, i64)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(seasons, vec![(11, 2, 2, 0), (12, 2, 1, 1), (13, 0, 0, 0)]);
    }

    #[test]
    fn library_has_one_row_per_category_membership() {
        let c = open_in_memory().unwrap();
        fixture_v1(&c);
        add_anime(&c, 10, 1, 1, None, 100);
        c.execute_batch(
            "INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'a', 0, 0), (2, 'b', 1, 0);
             INSERT INTO animes_categories(anime_id, category_id) VALUES (1, 1), (1, 2);",
        )
        .unwrap();
        let categories: Vec<i64> = library_rows(&c, 1).iter().map(|r| r.7).collect();
        assert_eq!(categories, vec![1, 2]);
        assert_eq!(library_rows(&c, 10).iter().map(|r| r.7).collect::<Vec<_>>(), vec![0]);
        c.execute("DELETE FROM animes_categories WHERE anime_id = 1", []).unwrap();
        assert_eq!(library_rows(&c, 1).iter().map(|r| r.7).collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn library_contains_favorites_only() {
        let c = open_in_memory().unwrap();
        add_anime(&c, 10, 1, 0, None, 100);
        add_anime(&c, 11, 0, 1, Some(10), 100);
        let ids: Vec<i64> = c
            .prepare("SELECT _id FROM animelibView ORDER BY _id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(ids, vec![10]);
    }

    #[test]
    fn library_and_season_views_expose_the_computed_columns_last_in_order() {
        let c = open_in_memory().unwrap();
        let names = |view: &str| -> Vec<String> {
            c.prepare(&format!("PRAGMA table_info({view})"))
                .unwrap()
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        let lib = names("animelibView");
        assert_eq!(lib.len(), 32 + 8);
        assert_eq!(lib[0], "_id");
        assert_eq!(
            lib[32..],
            ["totalCount", "seenCount", "latestUpload", "episodeFetchedAt", "lastSeen", "bookmarkCount", "fillermarkCount", "category"]
        );
        let seasons = names("animeseasonsView");
        assert_eq!(seasons.len(), 32 + 7);
        assert_eq!(seasons[32..], ["totalCount", "seenCount", "latestUpload", "episodeFetchedAt", "lastSeen", "bookmarkCount", "fillermarkCount"]);
    }

    #[test]
    fn updates_feed_shows_only_episodes_fetched_after_the_entry_was_added() {
        let c = open_in_memory().unwrap();
        add_anime(&c, 30, 1, 1, None, 100);
        add_episode(&c, 1, 30, 100, 1, 0, 0);
        add_episode(&c, 2, 30, 101, 1, 0, 0);
        add_episode(&c, 3, 30, 99, 1, 0, 0);
        let ids: Vec<i64> = c
            .prepare("SELECT episodeId FROM animeupdatesView WHERE animeId = 30")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(ids, vec![2]);
    }

    #[test]
    fn deletable_parents_are_non_favorite_non_children() {
        let c = open_in_memory().unwrap();
        add_anime(&c, 10, 1, 0, None, 100);
        add_anime(&c, 40, 0, 1, None, 100);
        add_anime(&c, 41, 0, 1, Some(10), 100);
        add_anime(&c, 42, 1, 1, None, 100);
        let rows: Vec<(i64, i64, i64)> = c
            .prepare("SELECT _id, source, fetch_type FROM animedeletableView ORDER BY _id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        // Anime 40 is the only row; its source is 1 and its fetch type is the 1 it was inserted with.
        assert_eq!(rows, vec![(40, 1, 1)]);
    }

    #[test]
    fn updates_statements_order_filter_and_limit_the_feed() {
        let c = open_in_memory().unwrap();
        add_anime(&c, 30, 1, 1, None, 100);
        add_episode(&c, 1, 30, 100, 1, 0, 0);
        add_episode(&c, 2, 30, 101, 1, 0, 0);
        add_episode(&c, 3, 30, 99, 1, 0, 0);
        add_anime(&c, 31, 1, 1, None, 10);
        add_episode(&c, 4, 31, 50, 500, 0, 0);
        add_episode(&c, 5, 31, 70, 300, 1, 0);
        add_episode(&c, 6, 31, 60, 900, 0, 0);
        add_anime(&c, 32, 0, 1, None, 10);
        add_episode(&c, 7, 32, 200, 999, 0, 0);
        let fetched = |rows: Vec<AnimeUpdateRow>| rows.iter().map(|r| r.date_fetch).collect::<Vec<_>>();
        // Anime 30 episode B (fetched 101, upload 1) is in the feed too: its upload is after 0.
        assert_eq!(fetched(recent_updates(&c, 0, 500).unwrap()), [101, 70, 60, 50]);
        assert_eq!(fetched(recent_updates(&c, 350, 500).unwrap()), [60, 50]);
        assert_eq!(fetched(recent_updates(&c, 0, 1).unwrap()), [101]);
        assert_eq!(fetched(updates_by_seen(&c, true, 0, 500).unwrap()), [70]);
        assert_eq!(fetched(updates_by_seen(&c, false, 0, 500).unwrap()), [101, 60, 50]);
    }

    #[test]
    fn upcoming_returns_library_anime_with_a_listed_status_soonest_first() {
        let c = open_in_memory().unwrap();
        let start: i64 = c
            .query_row("SELECT CAST(strftime('%s', 'now', 'localtime', 'start of day', 'utc') AS INTEGER) * 1000", [], |r| r.get(0))
            .unwrap();
        let day = 86_400_000_i64;
        let set = |c: &Connection, id: i64, favorite: i64, status: i64, next: Option<i64>| {
            add_anime(c, id, favorite, 1, None, 100);
            c.execute("UPDATE animes SET status = ?1, next_update = ?2 WHERE _id = ?3", params![status, next, id]).unwrap();
        };
        set(&c, 60, 1, 1, Some(start));
        set(&c, 61, 1, 7, Some(start + day));
        set(&c, 62, 1, 2, Some(start + day));
        set(&c, 63, 1, 1, None);
        set(&c, 64, 0, 1, Some(start + day));
        let ids = |statuses: &[i64]| -> Vec<i64> {
            upcoming_animes(&c, statuses, start).unwrap().iter().map(|a| a.id).collect()
        };
        assert_eq!(ids(&[1, 4, 7]), [60, 61]);
        assert!(ids(&[]).is_empty(), "an empty status set matches nothing");
    }
}
