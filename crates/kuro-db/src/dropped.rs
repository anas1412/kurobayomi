//! Database plumbing this crate does not port (T-03a-036 to T-03a-038, T-04a-033 to T-04a-035). Each piece belongs to the
//! Android build or to a query with no caller. The user loses nothing.
//!
//! * Build and schema tooling. The Gradle build file, the generated Kotlin API, the empty manifest and the schema
//!   snapshot directory are not ported. The Cargo manifest of this crate replaces them, and the SQL constants are
//!   kept by hand. SQL checking moves to two tests: every statement prepares (TC-S01) and the schema listing (TC-S00).
//!   The importer replay tests (T-03a-022) replace migration verification. The developer loses compile-time SQL
//!   checking. The `sqlite3` command line, or any SQLite browser, reads `manga.db` in WAL mode while the app runs.
//! * Drivers. The Android SQLite drivers, the requery and framework open-helper factories (the debug inspector) and
//!   the schema callbacks are replaced by `rusqlite` with bundled SQLite. The three connection pragmas are kept by
//!   `open::configure` (`runtime::tests` checks them).
//! * Paging. The paging source over SQL queries (`LIMIT`/`OFFSET` pages with invalidation) has no caller. Lists load in
//!   full. If a future screen needs paging, the replacement is keyset pagination on the view columns.
//! * Statements with no caller. `getAllAnime` and the anime favourite-timestamp listing served the removed sync feature.
//!   They are not ported. `update_all_anime` stays. The columns `version`, `is_syncing`, `last_modified_at` and
//!   `favorite_modified_at` stay in the schema, because backups carry them.
//!
//! The tests below keep these decisions true: a later change that adds one of these pieces fails them.

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Every `.rs` file of this crate's `src` directory, as text.
    fn sources() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect()
    }

    #[test]
    fn no_paging_source_is_built() {
        for text in sources() {
            assert!(!text.contains(concat!("Paging", "Source")));
            assert!(!text.contains(concat!("subscribe_to_", "paging")));
        }
    }

    #[test]
    fn the_statements_without_a_caller_are_not_ported() {
        for text in sources() {
            assert!(!text.contains(concat!("fn get_all_", "anime(")));
            assert!(!text.contains(concat!("fn get_animes_with_", "favorite_timestamp(")));
        }
    }

    #[test]
    fn the_sync_state_columns_stay_in_the_schema() {
        let schema: String = sources().into_iter().filter(|s| s.contains("is_syncing")).collect();
        for column in ["is_syncing", "last_modified_at", "favorite_modified_at"] {
            assert!(schema.contains(column), "{column} must stay for backups");
        }
    }
}
