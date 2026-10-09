//! Opens SQLite connections: per-connection pragmas, the application id check, the version gate,
//! and creation of the baseline. Migrations are applied by `migrate`. The error type and the schema
//! description live in `error` and `schema`; they are re-exported here for existing callers.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, ErrorCode};

pub use crate::error::{DbError, DbResult};
pub use crate::schema::SchemaBundle;

use crate::migrate;

/// `PRAGMA application_id` of every Kurobayomi database: ASCII "KUR0". Aniyomi files keep 0.
pub const APPLICATION_ID: i64 = 0x4B55_5230;

/// Busy timeout of every connection, in milliseconds (`PRAGMA busy_timeout`).
pub const BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

/// Files above this size skip the `quick_check` at open, because the check reads every page.
pub const QUICK_CHECK_MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Prepared-statement cache capacity per connection; the manga statements alone exceed the default 16.
const STATEMENT_CACHE: usize = 64;

/// Opens `path` and applies the per-connection pragmas. Use it for every pooled connection.
pub fn open_connection(path: &Path) -> DbResult<Connection> {
    let c = Connection::open(path)?;
    configure(&c)?;
    Ok(c)
}

/// Sets `foreign_keys = ON` (per connection), `journal_mode = WAL`, `synchronous = NORMAL`,
/// the busy timeout and the statement cache. `recursive_triggers` stays at its default 0.
pub fn configure(c: &Connection) -> DbResult<()> {
    c.busy_timeout(BUSY_TIMEOUT)?;
    c.set_prepared_statement_cache_capacity(STATEMENT_CACHE);
    c.pragma_update(None, "foreign_keys", "ON")?;
    c.pragma_update(None, "synchronous", "NORMAL")?;
    let mode: String = c.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    if mode != "wal" {
        return Err(DbError::JournalMode(mode));
    }
    register_functions(c)
}

/// Registers the scalar SQL function `kuro_lower(text)`, which lowercases with Rust's
/// `str::to_lowercase`. SQLite's `lower()` only folds ASCII, so "Éclair" and "éCLAIR" would not match
/// through it. `configure` calls this on every file connection; the in-memory helpers call it too.
pub fn register_functions(c: &Connection) -> DbResult<()> {
    c.create_scalar_function(
        "kuro_lower",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let text: Option<String> = ctx.get(0)?;
            Ok(text.map(|t| t.to_lowercase()))
        },
    )?;
    Ok(())
}

/// Opens (creating the file and its parent directory when needed) and brings the database to the
/// bundle's latest version. The gate runs before any write: a `user_version` above the bundle, or a
/// file with another application id (an Aniyomi file), is refused and left untouched.
pub fn open_and_migrate(path: &Path, bundle: &SchemaBundle) -> DbResult<Connection> {
    Ok(open_and_migrate_report(path, bundle)?.conn)
}

/// The result of [`open_and_migrate_report`].
pub struct Opened {
    pub conn: Connection,
    /// Where a corrupt file was moved. The connection is then on a new, empty database, and the caller shows
    /// the restore-from-backup dialog.
    pub quarantined: Option<PathBuf>,
}

/// [`open_and_migrate`] with the corruption report. A file whose `quick_check` fails is renamed to
/// `<file>.corrupt-<unix seconds>` (with its `-wal` and `-shm` siblings) and a new database is created in its
/// place. Files above [`QUICK_CHECK_MAX_BYTES`] are not checked.
pub fn open_and_migrate_report(path: &Path, bundle: &SchemaBundle) -> DbResult<Opened> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let quarantined = if is_corrupt(path)? { Some(quarantine(path)?) } else { None };
    let mut c = Connection::open(path)?;
    let tables: i64 = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
        [],
        |r| r.get(0),
    )?;
    let user_version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if tables > 0 {
        let latest = migrate::latest_version(bundle.baseline_version, bundle.migrations);
        if user_version > latest {
            return Err(DbError::TooNew { found: user_version, supported: latest });
        }
        let application_id: i64 = c.pragma_query_value(None, "application_id", |r| r.get(0))?;
        if application_id != bundle.application_id {
            return Err((bundle.refuse_foreign)(user_version));
        }
        if user_version < bundle.baseline_version {
            return Err(DbError::Corrupt(format!(
                "user_version {user_version} is below the baseline {}",
                bundle.baseline_version
            )));
        }
    }
    configure(&c)?;
    if tables == 0 {
        create_baseline(&mut c, bundle)?;
    } else {
        migrate::run(&mut c, path, user_version, bundle.migrations)?;
    }
    Ok(Opened { conn: c, quarantined })
}

/// True when `PRAGMA quick_check` reports anything but `ok`, or the file is not a database at all. A missing,
/// empty or oversized file is not checked (false).
fn is_corrupt(path: &Path) -> DbResult<bool> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Ok(false);
    };
    if meta.len() == 0 {
        return Ok(false);
    }
    if meta.len() > QUICK_CHECK_MAX_BYTES {
        tracing::info!(file = %path.display(), bytes = meta.len(), "skipping quick_check on a large database");
        return Ok(false);
    }
    let c = Connection::open(path)?;
    match c.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0)) {
        Ok(first) => Ok(first != "ok"),
        Err(rusqlite::Error::SqliteFailure(e, _))
            if matches!(e.code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) =>
        {
            Ok(true)
        }
        Err(e) => Err(e.into()),
    }
}

/// Moves a corrupt database and its WAL and shared-memory files aside. Nothing is deleted.
fn quarantine(path: &Path) -> DbResult<PathBuf> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::other("database file name is not UTF-8"))?;
    let dest = path.with_file_name(format!("{file}.corrupt-{stamp}"));
    std::fs::rename(path, &dest)?;
    for side in ["-wal", "-shm"] {
        let from = path.with_file_name(format!("{file}{side}"));
        if from.exists() {
            std::fs::rename(&from, dest.with_file_name(format!("{file}.corrupt-{stamp}{side}")))?;
        }
    }
    tracing::error!(from = %path.display(), to = %dest.display(), "database failed quick_check; moved aside");
    Ok(dest)
}

/// Runs the baseline and stamps the application id and version in one transaction, so a failing
/// baseline leaves no tables and version 0.
/// Creates a fresh file: the baseline, then every migration, so a new file has the same shape as an upgraded
/// one. `user_version` is the latest version of the bundle.
pub(crate) fn create_baseline(c: &mut Connection, bundle: &SchemaBundle) -> DbResult<()> {
    let tx = c.transaction()?;
    tx.execute_batch(bundle.baseline_sql)?;
    for (_, sql) in bundle.migrations {
        tx.execute_batch(sql)?;
    }
    tx.pragma_update(None, "application_id", bundle.application_id)?;
    tx.pragma_update(None, "user_version", migrate::latest_version(bundle.baseline_version, bundle.migrations))?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{BASELINE_SQL, MANGA_SCHEMA};

    /// Baseline 33 plus one test migration that creates a table.
    const WITH_34: SchemaBundle = SchemaBundle {
        baseline_sql: BASELINE_SQL,
        baseline_version: 33,
        migrations: &[(34, "CREATE TABLE t(x);")],
        application_id: APPLICATION_ID,
        tables: &[],
        refuse_foreign: |found| DbError::LegacyFile { found },
    };

    /// Migration 34 that leaves a deferred foreign key violation, caught only by foreign_key_check.
    const BAD_34: SchemaBundle = SchemaBundle {
        baseline_sql: BASELINE_SQL,
        baseline_version: 33,
        migrations: &[(
            34,
            "CREATE TABLE p(id INTEGER PRIMARY KEY); \
             CREATE TABLE c(pid INTEGER REFERENCES p(id) DEFERRABLE INITIALLY DEFERRED); \
             INSERT INTO c VALUES (5);",
        )],
        application_id: APPLICATION_ID,
        tables: &[],
        refuse_foreign: |found| DbError::LegacyFile { found },
    };

    /// The baseline alone (version 33, no migrations): a file that still needs migration 34.
    const BASELINE_ONLY: SchemaBundle = SchemaBundle {
        baseline_sql: BASELINE_SQL,
        baseline_version: 33,
        migrations: &[],
        application_id: APPLICATION_ID,
        tables: &[],
        refuse_foreign: |found| DbError::LegacyFile { found },
    };

    fn pragma_i64(c: &Connection, name: &str) -> i64 {
        c.pragma_query_value(None, name, |r| r.get(0)).unwrap()
    }

    fn backup_files(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".pre-v"))
            .collect()
    }

    /// Creates a Kurobayomi file at version 33 (baseline only) and closes it.
    fn kur_file(dir: &Path) -> std::path::PathBuf {
        let p = dir.join("manga.db");
        drop(open_and_migrate(&p, &BASELINE_ONLY).unwrap());
        p
    }

    #[test]
    fn new_file_gets_baseline_pragmas_and_seed_row() {
        let dir = tempfile::tempdir().unwrap();
        let c = open_and_migrate(&dir.path().join("manga.db"), &MANGA_SCHEMA).unwrap();
        assert_eq!(pragma_i64(&c, "user_version"), 34);
        assert_eq!(pragma_i64(&c, "application_id"), APPLICATION_ID);
        assert_eq!(pragma_i64(&c, "foreign_keys"), 1);
        assert_eq!(pragma_i64(&c, "recursive_triggers"), 0);
        assert_eq!(pragma_i64(&c, "busy_timeout"), 5000);
        assert_eq!(pragma_i64(&c, "synchronous"), 1);
        let mode: String = c.pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
        assert_eq!(mode, "wal");
        let seed: (i64, String, i64, i64, i64) = c
            .query_row("SELECT * FROM categories", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap();
        assert_eq!(seed, (0, String::new(), -1, 0, 0), "TC-O01: the baseline seed row");
    }

    #[test]
    fn a_healthy_file_is_not_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        let opened = open_and_migrate_report(&p, &MANGA_SCHEMA).unwrap();
        assert!(opened.quarantined.is_none());
    }

    #[test]
    fn a_corrupt_file_is_moved_aside_and_a_fresh_database_takes_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manga.db");
        std::fs::write(&p, b"this is not a sqlite database, just bytes ".repeat(200)).unwrap();
        let opened = open_and_migrate_report(&p, &MANGA_SCHEMA).unwrap();
        let moved = opened.quarantined.expect("corrupt file is quarantined");
        assert!(moved.file_name().unwrap().to_string_lossy().starts_with("manga.db.corrupt-"));
        assert!(std::fs::read(&moved).unwrap().starts_with(b"this is not"), "bytes are kept, not deleted");
        assert_eq!(pragma_i64(&opened.conn, "user_version"), 34);
    }

    #[test]
    fn an_empty_file_is_not_treated_as_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manga.db");
        std::fs::write(&p, b"").unwrap();
        assert!(open_and_migrate_report(&p, &MANGA_SCHEMA).unwrap().quarantined.is_none());
    }

    #[test]
    fn creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("nested").join("manga.db");
        drop(open_and_migrate(&p, &MANGA_SCHEMA).unwrap());
        assert!(p.exists());
    }

    #[test]
    fn reopen_keeps_version_and_writes_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manga.db");
        drop(open_and_migrate(&p, &MANGA_SCHEMA).unwrap());
        let c = open_and_migrate(&p, &MANGA_SCHEMA).unwrap();
        assert_eq!(pragma_i64(&c, "user_version"), 34);
        assert!(backup_files(dir.path()).is_empty());
    }

    #[test]
    fn every_connection_enforces_foreign_keys() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        let reader = open_connection(&p).unwrap();
        assert_eq!(pragma_i64(&reader, "foreign_keys"), 1);
        let orphan = reader.execute(
            "INSERT INTO chapters(manga_id, url, name, read, bookmark, last_page_read, chapter_number, \
             source_order, date_fetch, date_upload) VALUES (99, 'u', 'n', 0, 0, 0, 1, 0, 0, 0)",
            [],
        );
        assert!(orphan.is_err());
    }

    #[test]
    fn kuro_lower_is_registered_on_every_file_connection() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        let writer = crate::schema_manga::open(&p).unwrap();
        for c in [writer, open_connection(&p).unwrap()] {
            let folded: String = c.query_row("SELECT kuro_lower('ÉCLAIR')", [], |r| r.get(0)).unwrap();
            assert_eq!(folded, "éclair");
            let nothing: Option<String> = c.query_row("SELECT kuro_lower(NULL)", [], |r| r.get(0)).unwrap();
            assert_eq!(nothing, None);
        }
    }

    #[test]
    fn aniyomi_file_is_refused_without_writes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tachiyomi.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(BASELINE_SQL).unwrap();
            c.pragma_update(None, "user_version", 33).unwrap();
        }
        let before = std::fs::read(&p).unwrap();
        let err = open_and_migrate(&p, &MANGA_SCHEMA).unwrap_err();
        assert!(matches!(err, DbError::LegacyFile { found: 33 }), "TC-O03: {err}");
        assert_eq!(std::fs::read(&p).unwrap(), before);
    }

    #[test]
    fn newer_file_is_refused_without_writes() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        Connection::open(&p).unwrap().pragma_update(None, "user_version", 99).unwrap();
        let before = std::fs::read(&p).unwrap();
        let err = open_and_migrate(&p, &MANGA_SCHEMA).unwrap_err();
        assert!(matches!(err, DbError::TooNew { found: 99, supported: 34 }), "TC-O04: {err}");
        assert_eq!(std::fs::read(&p).unwrap(), before);
    }

    #[test]
    fn version_below_baseline_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        Connection::open(&p).unwrap().pragma_update(None, "user_version", 20).unwrap();
        let err = open_and_migrate(&p, &MANGA_SCHEMA).unwrap_err();
        assert!(matches!(err, DbError::Corrupt(_)), "{err}");
    }

    #[test]
    fn pending_migration_runs_once_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        let c = open_and_migrate(&p, &WITH_34).unwrap();
        assert_eq!(pragma_i64(&c, "user_version"), 34);
        let t: i64 = c
            .query_row("SELECT count(*) FROM sqlite_master WHERE name = 't'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(t, 1);
        drop(c);
        assert_eq!(backup_files(dir.path()).len(), 1);
        let again = open_and_migrate(&p, &WITH_34).unwrap();
        assert_eq!(pragma_i64(&again, "user_version"), 34);
        assert_eq!(backup_files(dir.path()).len(), 1);
    }

    #[test]
    fn aniyomi_fresh_ddl_fixture_is_refused_as_a_legacy_file_without_writes() {
        // TC-O03 with the Aniyomi `.sq` text (history.last_read nullable, as upstream writes it).
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tachiyomi.db");
        Connection::open(&p)
            .unwrap()
            .execute_batch(include_str!("../tests/fixtures/aniyomi_fresh_v33.sql"))
            .unwrap();
        let before = std::fs::read(&p).unwrap();
        let err = open_and_migrate(&p, &MANGA_SCHEMA).unwrap_err();
        assert!(matches!(err, DbError::LegacyFile { found: 33 }), "TC-O03: {err}");
        assert_eq!(std::fs::read(&p).unwrap(), before, "the Aniyomi file stays byte-identical");
    }

    #[test]
    fn migration_with_foreign_key_violation_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        let err = open_and_migrate(&p, &BAD_34).unwrap_err();
        assert!(matches!(err, DbError::Constraint(_)), "{err}");
        let c = Connection::open(&p).unwrap();
        assert_eq!(pragma_i64(&c, "user_version"), 33);
        let p_table: i64 = c
            .query_row("SELECT count(*) FROM sqlite_master WHERE name = 'p'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(p_table, 0);
    }
}
