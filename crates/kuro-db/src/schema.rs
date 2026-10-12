//! The `SchemaBundle` type that a database file is described with. Nothing here knows about a particular file;
//! the unified file's bundle is `schema_unified::UNIFIED_SCHEMA`.

use crate::error::DbError;

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
    /// The refusal for a non-empty file whose application id is not this bundle's.
    pub refuse_foreign: fn(i64) -> DbError,
}

#[cfg(test)]
mod tests {
    use crate::schema_unified::{UNIFIED_SCHEMA, open_in_memory};

    #[test]
    fn the_bundle_names_exactly_the_tables_its_baseline_creates() {
        let c = open_in_memory().unwrap();
        let mut in_file: Vec<String> = c
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut named: Vec<String> = UNIFIED_SCHEMA.tables.iter().map(|s| (*s).to_owned()).collect();
        in_file.sort();
        named.sort();
        assert_eq!(named, in_file);
    }
}
