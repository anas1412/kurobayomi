//! Schema descriptions: the manga baseline at version 33 and the `SchemaBundle` type that every
//! database file (manga, anime) is described with. Nothing here knows about a particular file.

use crate::error::DbError;

/// `PRAGMA user_version` of the manga baseline, the Aniyomi schema version 33.
pub const SCHEMA_VERSION: i64 = 33;

/// Manga baseline text (`sql/schema_v33.sql`): tables, indexes, triggers, views and the system
/// category row, exactly as the plan's section 2 lists them.
pub const BASELINE_SQL: &str = include_str!("../sql/schema_v33.sql");

/// Table names of the manga file. A `Tables` bit N is the N-th name here.
pub const MANGA_TABLES: &[&str] = &[
    "mangas",
    "chapters",
    "history",
    "categories",
    "mangas_categories",
    "excluded_scanlators",
    "manga_sync",
    "sources",
    "extension_repos",
];

/// One database file: a baseline script that creates `baseline_version`, then `migrations`
/// (`(target version, SQL)`, ascending and contiguous from `baseline_version + 1`). The file
/// carries `application_id`, which tells it apart from files of other schemas.
pub struct SchemaBundle {
    pub baseline_sql: &'static str,
    pub baseline_version: i64,
    pub migrations: &'static [(i64, &'static str)],
    pub application_id: i64,
    /// Names of the tables this file has, in the order of the notification bits (see `notify`).
    pub tables: &'static [&'static str],
    /// The refusal for a non-empty file whose application id is not this bundle's. The manga file
    /// always refuses an Aniyomi file with `LegacyFile`; the anime file tells legacy versions apart.
    pub refuse_foreign: fn(i64) -> DbError,
}

/// The manga file: the baseline at version 33 and no migrations yet.
pub const MANGA_SCHEMA: SchemaBundle = SchemaBundle {
    baseline_sql: BASELINE_SQL,
    baseline_version: SCHEMA_VERSION,
    migrations: &[],
    application_id: crate::open::APPLICATION_ID,
    tables: MANGA_TABLES,
    refuse_foreign: |found| DbError::LegacyFile { found },
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_anime::{ANIME_SCHEMA, ANIME_TABLES};

    fn has_table(c: &rusqlite::Connection, name: &str) -> bool {
        c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
            == 1
    }

    #[test]
    fn each_bundle_names_exactly_the_tables_its_baseline_creates() {
        let manga = crate::schema_manga::test_db();
        for name in MANGA_TABLES {
            assert!(has_table(&manga, name), "{name} missing from the manga baseline");
        }
        let anime = crate::schema_anime::open_in_memory().unwrap();
        for name in ANIME_TABLES {
            assert!(has_table(&anime, name), "{name} missing from the anime baseline");
        }
        assert_eq!(ANIME_SCHEMA.tables, ANIME_TABLES);
        assert_eq!(MANGA_SCHEMA.tables, MANGA_TABLES);
    }

    #[test]
    fn the_manga_bundle_refuses_every_aniyomi_version_as_a_legacy_file() {
        for found in [1, 33, 113, 139] {
            assert!(matches!((MANGA_SCHEMA.refuse_foreign)(found), DbError::LegacyFile { found: f } if f == found));
        }
    }
}
