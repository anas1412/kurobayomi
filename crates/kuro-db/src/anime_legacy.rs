//! The anime legacy chain: the 26 Aniyomi scripts `113.sql` to `138.sql` that upgrade a copied `tachiyomi.animedb`
//! (T-04a-025). `sql/anime/legacy/N.sql` upgrades `user_version` N to N+1; the files are the `.sqm` text with
//! [`crate::legacy::strip_annotations`] applied. Fresh databases never run them.
//!
//! [`upgrade_legacy`] copies the file to `<name>.bak-v<V>`, runs every step from the stored version to 138 in its own
//! transaction (stamping `user_version` inside it, so an interrupted run resumes at the failed step), adds the Kurobayomi-only column
//! `extension_store.resolved_index_url`, recreates the eight views from `views.sql`, drops the `animehistory_temp` table that step 115 leaves behind and stamps the
//! application id. Steps 132 and 135 run as written: every entry ends with `fetch_type = 1`.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{DbError, DbResult};
use crate::schema_anime::{ANIME_APPLICATION_ID, ANIME_SCHEMA_VERSION, NATIVE_139_TO_140};

/// The first and last step of the chain.
pub const FIRST_STEP: i64 = 113;
pub const LAST_STEP: i64 = 138;

/// `sql/anime/legacy/113.sql` .. `138.sql`; index `n - 113` upgrades version `n` to `n + 1`.
pub const STEPS: [&str; 26] = [
    include_str!("../sql/anime/legacy/113.sql"),
    include_str!("../sql/anime/legacy/114.sql"),
    include_str!("../sql/anime/legacy/115.sql"),
    include_str!("../sql/anime/legacy/116.sql"),
    include_str!("../sql/anime/legacy/117.sql"),
    include_str!("../sql/anime/legacy/118.sql"),
    include_str!("../sql/anime/legacy/119.sql"),
    include_str!("../sql/anime/legacy/120.sql"),
    include_str!("../sql/anime/legacy/121.sql"),
    include_str!("../sql/anime/legacy/122.sql"),
    include_str!("../sql/anime/legacy/123.sql"),
    include_str!("../sql/anime/legacy/124.sql"),
    include_str!("../sql/anime/legacy/125.sql"),
    include_str!("../sql/anime/legacy/126.sql"),
    include_str!("../sql/anime/legacy/127.sql"),
    include_str!("../sql/anime/legacy/128.sql"),
    include_str!("../sql/anime/legacy/129.sql"),
    include_str!("../sql/anime/legacy/130.sql"),
    include_str!("../sql/anime/legacy/131.sql"),
    include_str!("../sql/anime/legacy/132.sql"),
    include_str!("../sql/anime/legacy/133.sql"),
    include_str!("../sql/anime/legacy/134.sql"),
    include_str!("../sql/anime/legacy/135.sql"),
    include_str!("../sql/anime/legacy/136.sql"),
    include_str!("../sql/anime/legacy/137.sql"),
    include_str!("../sql/anime/legacy/138.sql"),
];

/// The views of the fresh schema, recreated after the chain.
const VIEWS_SQL: &str = include_str!("../sql/anime/views.sql");

/// What the fresh schema has beyond Aniyomi's version 139: `extension_store.resolved_index_url`, the document a
/// store's data came from. A legacy store resolves to its own key.
const KUROBAYOMI_ADDITIONS: &str = "ALTER TABLE extension_store ADD COLUMN resolved_index_url TEXT NOT NULL DEFAULT '';
UPDATE extension_store SET resolved_index_url = index_url;";

/// The SQL of step `n` (version `n` to `n + 1`), for `n` in 113..=138.
pub fn step(n: i64) -> Option<&'static str> {
    usize::try_from(n - FIRST_STEP).ok().and_then(|i| STEPS.get(i)).copied()
}

/// What [`upgrade_legacy`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyUpgrade {
    /// The version the file had.
    pub from: i64,
    /// The copy made before the first step.
    pub backup: PathBuf,
}

/// Applies steps `from..until` (each in `BEGIN` / `COMMIT`, stamping `user_version = n + 1`). `foreign_keys` must be
/// off, as Android ran the scripts.
pub fn apply_steps(c: &Connection, from: i64, until: i64) -> rusqlite::Result<()> {
    for n in from..until.min(LAST_STEP + 1) {
        let Some(sql) = step(n) else { continue };
        c.execute_batch("BEGIN")?;
        let done = c.execute_batch(sql).and_then(|()| c.pragma_update(None, "user_version", n + 1));
        match done {
            Ok(()) => c.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = c.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Drops every view, so the renames of steps 114 and 115 do not rewrite them.
pub fn drop_all_views(c: &Connection) -> rusqlite::Result<()> {
    let views: Vec<String> = {
        let mut st = c.prepare("SELECT name FROM sqlite_master WHERE type = 'view'")?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for view in views {
        c.execute_batch(&format!("DROP VIEW IF EXISTS \"{view}\""))?;
    }
    Ok(())
}

/// Upgrades the Aniyomi anime database at `path` in place to schema 139. A file already at 139 is left untouched
/// (`None`); a version below 113 is refused with `TooOldForImport` and a newer one with `TooNew`, both before any write.
pub fn upgrade_legacy(path: &Path) -> DbResult<Option<LegacyUpgrade>> {
    let c = Connection::open(path)?;
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version >= ANIME_SCHEMA_VERSION {
        if version > ANIME_SCHEMA_VERSION {
            return Err(DbError::TooNew { found: version, supported: ANIME_SCHEMA_VERSION });
        }
        return Ok(None);
    }
    if version < FIRST_STEP {
        return Err(DbError::TooOldForImport { found: version });
    }
    drop(c);
    let backup = backup_copy(path, version)?;
    let c = Connection::open(path)?;
    c.pragma_update(None, "foreign_keys", "OFF")?;
    c.pragma_update(None, "legacy_alter_table", "OFF")?;
    drop_all_views(&c)?;
    apply_steps(&c, version, LAST_STEP + 1)?;
    c.execute_batch("BEGIN")?;
    let finished = (|| {
        drop_all_views(&c)?;
        c.execute_batch("DROP TABLE IF EXISTS animehistory_temp")?;
        c.execute_batch(KUROBAYOMI_ADDITIONS)?;
        c.execute_batch(NATIVE_139_TO_140)?;
        c.execute_batch(VIEWS_SQL)?;
        c.pragma_update(None, "user_version", ANIME_SCHEMA_VERSION)?;
        c.pragma_update(None, "application_id", ANIME_APPLICATION_ID)
    })();
    match finished {
        Ok(()) => c.execute_batch("COMMIT")?,
        Err(e) => {
            let _ = c.execute_batch("ROLLBACK");
            return Err(e.into());
        }
    }
    let broken: i64 = c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
    let integrity: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if broken > 0 || integrity != "ok" {
        return Err(DbError::Corrupt(format!("after the legacy chain: {broken} foreign key violations, integrity {integrity}")));
    }
    c.pragma_update(None, "foreign_keys", "ON")?;
    Ok(Some(LegacyUpgrade { from: version, backup }))
}

/// Copies the file (and its `-wal` / `-shm`) to `<name>.bak-v<version>`. An existing copy of that version is kept, so
/// resuming an interrupted import never overwrites the first one.
fn backup_copy(path: &Path, version: i64) -> std::io::Result<PathBuf> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("anime.db");
    let backup = path.with_file_name(format!("{name}.bak-v{version}"));
    if !backup.exists() {
        std::fs::copy(path, &backup)?;
        for side in ["-wal", "-shm"] {
            let from = path.with_file_name(format!("{name}{side}"));
            if from.is_file() {
                std::fs::copy(&from, path.with_file_name(format!("{name}.bak-v{version}{side}")))?;
            }
        }
    }
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chain_has_one_file_per_step() {
        assert_eq!(STEPS.len() as i64, LAST_STEP - FIRST_STEP + 1);
        assert!(step(112).is_none() && step(139).is_none());
        assert!(step(115).unwrap().contains("animehistory_temp"));
    }

    #[test]
    fn the_committed_files_match_a_regeneration_from_the_sqm_files() {
        let root = std::env::var("KURO_ANIYOMI_SRC").unwrap_or_else(|_| "/home/blackbox/aniyomi-src".into());
        let dir = Path::new(&root).join("data/src/main/sqldelightanime/migrations");
        if !dir.is_dir() {
            eprintln!("skipped: {} is not there", dir.display());
            return;
        }
        for n in FIRST_STEP..=LAST_STEP {
            let sqm = std::fs::read_to_string(dir.join(format!("{n}.sqm"))).unwrap();
            assert_eq!(crate::legacy::strip_annotations(&sqm), step(n).unwrap(), "{n}.sql is stale");
        }
    }
}
