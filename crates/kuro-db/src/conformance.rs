//! Schema conformance (T-03a-021) and trigger and versioning fidelity (T-03a-020), run on an
//! in-memory database built from the embedded baseline. These replace SQLDelight's compile-time
//! checks: every statement must prepare, and the object inventory must match the area docs.

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::schema_manga::{BASELINE_SQL, test_db};
    use crate::statements_manga::{
        categories, chapters, excluded_scanlators, extension_repos, fixtures, manga_sync, mangas,
        mangas_categories, sources, views,
    };

    /// Every statement constant of the manga module. Anime statements are checked by their own module.
    const ALL_STATEMENTS: &[&str] = &[
        mangas::GET_BY_ID,
        mangas::GET_BY_URL_AND_SOURCE,
        mangas::GET_FAVORITES,
        mangas::GET_READ_NOT_IN_LIBRARY,
        mangas::GET_ALL,
        mangas::GET_ALL_SOURCE_AND_URL,
        mangas::GET_WITH_FAVORITE_TIMESTAMP,
        mangas::GET_SOURCE_ID_WITH_FAVORITE_COUNT,
        mangas::GET_FAVORITE_BY_SOURCE_ID,
        mangas::GET_DUPLICATE_LIBRARY_MANGA,
        mangas::GET_UPCOMING_MANGA,
        mangas::RESET_VIEWER_FLAGS,
        mangas::RESET_IS_SYNCING,
        mangas::GET_SOURCE_IDS_WITH_NON_LIBRARY_MANGA,
        mangas::DELETE_NOT_IN_LIBRARY_BY_SOURCE_IDS,
        mangas::INSERT,
        mangas::UPDATE,
        mangas::LAST_INSERT_ROWID,
        chapters::GET_BY_ID,
        chapters::GET_BY_MANGA_ID,
        chapters::GET_SCANLATORS_BY_MANGA_ID,
        chapters::GET_BOOKMARKED_BY_MANGA_ID,
        chapters::GET_BY_URL,
        chapters::GET_BY_URL_AND_MANGA_ID,
        chapters::REMOVE_WITH_IDS,
        chapters::RESET_IS_SYNCING,
        chapters::INSERT,
        chapters::UPDATE,
        chapters::LAST_INSERT_ROWID,
        categories::GET_CATEGORY,
        categories::GET_CATEGORIES,
        categories::GET_VISIBLE_CATEGORIES,
        categories::GET_CATEGORIES_BY_MANGA_ID,
        categories::GET_VISIBLE_CATEGORIES_BY_MANGA_ID,
        categories::INSERT,
        categories::DELETE,
        categories::UPDATE,
        categories::UPDATE_ALL_FLAGS,
        categories::LAST_INSERT_ROWID,
        categories::IDS_BY_ORDER,
        categories::SET_ORDER,
        mangas_categories::INSERT,
        mangas_categories::DELETE_BY_MANGA_ID,
        crate::statements_manga::history::GET_BY_MANGA_ID,
        crate::statements_manga::history::GET_BY_CHAPTER_URL,
        crate::statements_manga::history::RESET_BY_ID,
        crate::statements_manga::history::RESET_BY_MANGA_ID,
        crate::statements_manga::history::REMOVE_ALL,
        crate::statements_manga::history::REMOVE_RESETTED,
        crate::statements_manga::history::UPSERT,
        crate::statements_manga::history::GET_READ_DURATION,
        crate::statements_manga::views::LIBRARY,
        crate::statements_manga::views::HISTORY,
        crate::statements_manga::views::GET_LATEST_HISTORY,
        extension_repos::FIND_ONE,
        extension_repos::FIND_ONE_BY_SIGNING_KEY_FINGERPRINT,
        extension_repos::FIND_ALL,
        extension_repos::COUNT,
        extension_repos::INSERT,
        extension_repos::UPSERT,
        extension_repos::REPLACE,
        extension_repos::DELETE,
        manga_sync::DELETE,
        manga_sync::GET_TRACKS,
        manga_sync::GET_TRACK_BY_ID,
        manga_sync::GET_TRACKS_BY_MANGA_ID,
        manga_sync::INSERT,
        manga_sync::UPDATE,
        excluded_scanlators::INSERT,
        excluded_scanlators::REMOVE,
        excluded_scanlators::GET_BY_MANGA_ID,
        sources::FIND_ALL,
        sources::FIND_ONE,
        sources::UPSERT,
        views::GET_RECENT_UPDATES,
        views::GET_UPDATES_BY_READ_STATUS,
    ];

    /// `type|name|tbl_name` for every object of a fresh baseline (tables, the 6 named and 4 automatic
    /// indexes, 7 triggers, 3 views), as listed in `p1-topic-baseline-schema.md` section 3.
    const EXPECTED_OBJECTS: &[&str] = &[
        "index|chapters_manga_id_index|chapters",
        "index|chapters_unread_by_manga_index|chapters",
        "index|excluded_scanlators_manga_id_index|excluded_scanlators",
        "index|history_history_chapter_id_index|history",
        "index|library_favorite_index|mangas",
        "index|mangas_url_index|mangas",
        "index|sqlite_autoindex_extension_repos_1|extension_repos",
        "index|sqlite_autoindex_extension_repos_2|extension_repos",
        "index|sqlite_autoindex_history_1|history",
        "index|sqlite_autoindex_manga_sync_1|manga_sync",
        "table|categories|categories",
        "table|chapters|chapters",
        "table|excluded_scanlators|excluded_scanlators",
        "table|extension_repos|extension_repos",
        "table|history|history",
        "table|manga_sync|manga_sync",
        "table|mangas|mangas",
        "table|mangas_categories|mangas_categories",
        "table|sources|sources",
        "trigger|insert_manga_category_update_version|mangas_categories",
        "trigger|system_category_delete_trigger|categories",
        "trigger|update_chapter_and_manga_version|chapters",
        "trigger|update_last_favorite_at_mangas|mangas",
        "trigger|update_last_modified_at_chapters|chapters",
        "trigger|update_last_modified_at_mangas|mangas",
        "trigger|update_manga_version|mangas",
        "view|historyView|historyView",
        "view|libraryView|libraryView",
        "view|updatesView|updatesView",
    ];

    fn scalar_i64(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn column_names(c: &Connection, table: &str) -> Vec<String> {
        let mut st = c
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid"))
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
    }

    fn versions(c: &Connection, manga_id: i64, chapter_id: i64) -> (i64, i64) {
        let m = scalar_i64(c, &format!("SELECT version FROM mangas WHERE _id = {manga_id}"));
        let ch = scalar_i64(c, &format!("SELECT version FROM chapters WHERE _id = {chapter_id}"));
        (m, ch)
    }

    #[test]
    fn rusqlite_bundles_sqlite_3_53_or_newer() {
        assert!(rusqlite::version_number() >= 3_053_000, "{}", rusqlite::version());
    }

    #[test]
    fn baseline_has_no_double_quoted_strings() {
        assert!(!BASELINE_SQL.contains('"'));
    }

    #[test]
    fn object_inventory_matches_the_area_doc() {
        let c = test_db();
        let mut st = c
            .prepare("SELECT type, name, tbl_name FROM sqlite_master ORDER BY type, name")
            .unwrap();
        let mut found: Vec<String> = st
            .query_map([], |r| {
                Ok(format!("{}|{}|{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let mut expected: Vec<String> = EXPECTED_OBJECTS.iter().map(|s| (*s).to_owned()).collect();
        found.sort();
        expected.sort();
        assert_eq!(found, expected);
        assert_eq!(expected.len(), 29);
    }

    /// `type|name|tbl_name|sql` of every object, sql with whitespace collapsed, in name order.
    fn pinned_dump(c: &Connection) -> String {
        let mut st = c
            .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name")
            .unwrap();
        let lines: Vec<String> = st
            .query_map([], |r| {
                let sql: Option<String> = r.get(3)?;
                let sql = sql.unwrap_or_default();
                Ok(format!(
                    "{}|{}|{}|{}",
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    sql.split_whitespace().collect::<Vec<_>>().join(" ")
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        lines.join("\n") + "\n"
    }

    #[test]
    fn fresh_schema_objects_match_the_pinned_dump() {
        // TC-S00: fixtures/schema_v33.expected.txt is the object list of a fresh manga file.
        let c = test_db();
        assert_eq!(pinned_dump(&c), include_str!("../tests/fixtures/schema_v33.expected.txt"));
    }

    #[test]
    fn system_category_seed_row_exists() {
        let c = test_db();
        let row: (i64, String, i64, i64, i64) = c
            .query_row("SELECT * FROM categories", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap();
        assert_eq!(row, (0, String::new(), -1, 0, 0), "TC-O01: the baseline seed row");
    }

    #[test]
    fn column_order_matches_the_mapped_tables() {
        let c = test_db();
        assert_eq!(
            column_names(&c, "mangas"),
            [
                "_id", "source", "url", "artist", "author", "description", "genre", "title", "status",
                "thumbnail_url", "favorite", "last_update", "next_update", "initialized", "viewer",
                "chapter_flags", "cover_last_modified", "date_added", "update_strategy",
                "calculate_interval", "last_modified_at", "favorite_modified_at", "version", "is_syncing",
            ]
        );
        assert_eq!(
            column_names(&c, "chapters"),
            [
                "_id", "manga_id", "url", "name", "scanlator", "read", "bookmark", "last_page_read",
                "chapter_number", "source_order", "date_fetch", "date_upload", "last_modified_at",
                "version", "is_syncing",
            ]
        );
        assert_eq!(column_names(&c, "categories"), ["_id", "name", "sort", "flags", "hidden"]);
        assert_eq!(column_names(&c, "history"), ["_id", "chapter_id", "last_read", "time_read"]);
        assert_eq!(column_names(&c, "mangas_categories"), ["_id", "manga_id", "category_id"]);
    }

    #[test]
    fn history_last_read_is_not_null() {
        let c = test_db();
        let notnull: i64 = c
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('history') WHERE name = 'last_read'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notnull, 1);
    }

    #[test]
    fn every_statement_prepares_against_the_baseline() {
        let c = test_db();
        for sql in ALL_STATEMENTS {
            if let Err(e) = c.prepare(sql) {
                panic!("statement does not prepare: {e}\n{sql}");
            }
        }
        // The manga module has 76 statement constants; all are listed above. TC-S01 in the plan
        // says 74, which does not match the constants that exist.
        assert_eq!(ALL_STATEMENTS.len(), 76);
    }

    #[test]
    fn indexed_lookups_use_their_indexes() {
        let c = test_db();
        let plan = |sql: &str| -> String {
            let mut st = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let unbound = vec![None::<i64>; st.parameter_count()];
            st.query_map(rusqlite::params_from_iter(unbound), |r| r.get::<_, String>(3))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .join(" | ")
        };
        assert!(plan(mangas::GET_BY_URL_AND_SOURCE).contains("mangas_url_index"));
        assert!(plan(mangas::GET_FAVORITES).contains("library_favorite_index"));
        assert!(plan(chapters::GET_BY_MANGA_ID).contains("chapters_manga_id_index"));
    }

    /// Manga 1 (not a favourite, description 'd') and chapter 1, with categories 1..3.
    fn matrix_db() -> Connection {
        let c = test_db();
        let mut m = fixtures::manga("/a");
        m.favorite = false;
        assert_eq!(mangas::insert(&c, &m).unwrap(), 1);
        assert_eq!(chapters::insert(&c, &fixtures::chapter(1, "/c1")).unwrap(), 1);
        for name in ["one", "two", "three"] {
            categories::insert(&c, name, 0, 0).unwrap();
        }
        c
    }

    #[test]
    fn trigger_version_matrix_t01_to_t10() {
        let c = matrix_db();
        let steps: [(&str, (i64, i64)); 10] = [
            ("UPDATE chapters SET read = 1 WHERE _id = 1", (1, 1)),
            ("UPDATE chapters SET name = 'n2' WHERE _id = 1", (1, 1)),
            ("UPDATE chapters SET last_page_read = 5, bookmark = 1 WHERE _id = 1", (2, 2)),
            ("UPDATE mangas SET favorite = 1, url = '/a9' WHERE _id = 1", (3, 2)),
            ("UPDATE mangas SET title = 'T2' WHERE _id = 1", (3, 2)),
            ("INSERT INTO mangas_categories(manga_id, category_id) VALUES (1,1),(1,2),(1,3)", (6, 2)),
            ("UPDATE mangas SET is_syncing = 1 WHERE _id = 1", (6, 2)),
            ("UPDATE chapters SET read = 0, is_syncing = 0 WHERE _id = 1", (6, 3)),
            ("UPDATE chapters SET read = 1, is_syncing = 1 WHERE _id = 1", (6, 3)),
            ("UPDATE mangas SET favorite = 0, is_syncing = 0 WHERE _id = 1", (7, 3)),
        ];
        for (i, (sql, expected)) in steps.iter().enumerate() {
            c.execute(sql, []).unwrap_or_else(|e| panic!("TC-T{:02}: {e}", i + 1));
            assert_eq!(versions(&c, 1, 1), *expected, "TC-T{:02} after {sql}", i + 1);
        }
    }

    #[test]
    fn timestamps_are_seconds_and_refreshed_on_update() {
        let c = matrix_db();
        assert_eq!(
            scalar_i64(&c, "SELECT last_modified_at FROM chapters WHERE _id = 1"),
            0,
            "insert writes 0"
        );
        c.execute("UPDATE chapters SET read = 1 WHERE _id = 1", []).unwrap();
        let fresh = scalar_i64(
            &c,
            "SELECT abs(last_modified_at - CAST(strftime('%s','now') AS INTEGER)) <= 2 FROM chapters WHERE _id = 1",
        );
        assert_eq!(fresh, 1);
        c.execute("UPDATE mangas SET favorite = 1 WHERE _id = 1", []).unwrap();
        assert!(scalar_i64(&c, "SELECT favorite_modified_at FROM mangas WHERE _id = 1") > 0);
    }

    /// NULL-blind description check (TC-T11): NULL -> 'x' and 'd' -> NULL leave the version at 0,
    /// 'd' -> 'y' bumps it once.
    #[test]
    fn description_null_rule_t11() {
        let version = |c: &Connection| scalar_i64(c, "SELECT version FROM mangas WHERE _id = 1");

        let c = matrix_db();
        c.execute("UPDATE mangas SET description = NULL WHERE _id = 1", []).unwrap();
        c.execute("UPDATE mangas SET description = 'x' WHERE _id = 1", []).unwrap();
        assert_eq!(version(&c), 0, "NULL -> 'x'");

        let c = matrix_db();
        c.execute("UPDATE mangas SET description = NULL WHERE _id = 1", []).unwrap();
        assert_eq!(version(&c), 0, "'d' -> NULL");

        let c = matrix_db();
        c.execute("UPDATE mangas SET description = 'y' WHERE _id = 1", []).unwrap();
        assert_eq!(version(&c), 1, "'d' -> 'y'");
    }

    #[test]
    fn system_category_cannot_be_deleted_t12() {
        let c = matrix_db();
        let err = c.execute("DELETE FROM categories WHERE _id = 0", []).unwrap_err();
        assert!(err.to_string().contains("System category can't be deleted"), "{err}");
        assert_eq!(scalar_i64(&c, "SELECT count(*) FROM categories WHERE _id = 0"), 1);
        c.execute("INSERT INTO categories(_id, name, sort, flags) VALUES (-1, 'x', -2, 0)", [])
            .unwrap();
        assert!(c.execute("DELETE FROM categories WHERE _id = -1", []).is_err());
    }

    #[test]
    fn deleting_a_user_category_drops_links_without_bumping_version_t13() {
        let c = matrix_db();
        mangas_categories::insert(&c, 1, 1).unwrap();
        mangas_categories::insert(&c, 1, 2).unwrap();
        let before = versions(&c, 1, 1).0;
        categories::delete(&c, 1).unwrap();
        assert_eq!(scalar_i64(&c, "SELECT count(*) FROM mangas_categories WHERE category_id = 1"), 0);
        assert_eq!(scalar_i64(&c, "SELECT count(*) FROM mangas_categories"), 1);
        assert_eq!(versions(&c, 1, 1).0, before);
    }

    #[test]
    fn triggers_are_not_recursive() {
        let c = matrix_db();
        assert_eq!(scalar_i64(&c, "PRAGMA recursive_triggers"), 0);
    }
}
