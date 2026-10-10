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
///
/// A file that fails its check, or is not a database at all, is renamed to `<file>.corrupt-<UTC>` (with its
/// `-wal` and `-shm` siblings) and the call returns [`DbError::Corrupt`] naming where it went. The path is then
/// free: the caller restores [`restore_newest_backup`] or opens again for an empty database. Files above
/// [`QUICK_CHECK_MAX_BYTES`] are not checked.
pub fn open_and_migrate(path: &Path, bundle: &SchemaBundle) -> DbResult<Connection> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let was_dirty = dirty_marker(path).exists();
    if is_corrupt(path, was_dirty)? {
        let moved = quarantine(path)?;
        // the marker belonged to the file that was moved; the next open starts a new one
        mark_clean_shutdown(path)?;
        return Err(DbError::Corrupt(format!("failed its integrity check, moved to {}", moved.display())));
    }
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
    // The marker goes down before the first write. A crash leaves it behind, and the next open then
    // runs the full integrity check.
    std::fs::write(dirty_marker(path), b"")?;
    configure(&c)?;
    if tables == 0 {
        create_baseline(&mut c, bundle)?;
    } else {
        migrate::run(&mut c, path, user_version, bundle.migrations)?;
    }
    Ok(c)
}

/// Suffix of the marker file that says a database is open or was not closed cleanly.
pub const DIRTY_SUFFIX: &str = ".dirty";

/// The dirty marker of the database at `path`: `<file>.dirty` beside it.
pub fn dirty_marker(path: &Path) -> PathBuf {
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{file}{DIRTY_SUFFIX}"))
}

/// Removes the dirty marker. Call on clean shutdown, after the connection is closed. A missing marker is fine.
pub fn mark_clean_shutdown(path: &Path) -> DbResult<()> {
    match std::fs::remove_file(dirty_marker(path)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// True when the file fails a check, or is not a database at all. A dirty file (`was_dirty`) gets
/// `PRAGMA integrity_check`, any other gets `quick_check`. A missing, empty or oversized file is not
/// checked (false).
fn is_corrupt(path: &Path, was_dirty: bool) -> DbResult<bool> {
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
    let pragma = if was_dirty { "PRAGMA integrity_check" } else { "PRAGMA quick_check" };
    match c.query_row(pragma, [], |r| r.get::<_, String>(0)) {
        Ok(first) => Ok(first != "ok"),
        Err(rusqlite::Error::SqliteFailure(e, _))
            if matches!(e.code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) =>
        {
            Ok(true)
        }
        Err(e) => Err(e.into()),
    }
}

/// Moves a corrupt database and its WAL and shared-memory files aside, to `<file>.corrupt-<UTC>` (a counter is
/// added when that name is taken). Nothing is deleted and nothing is overwritten.
fn quarantine(path: &Path) -> DbResult<PathBuf> {
    let stamp = migrate::utc_stamp(
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0)),
    );
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::other("database file name is not UTF-8"))?;
    let taken = |name: &str| ["", "-wal", "-shm"].iter().any(|side| path.with_file_name(format!("{name}{side}")).exists());
    let mut name = format!("{file}.corrupt-{stamp}");
    let mut n = 1;
    while taken(&name) {
        name = format!("{file}.corrupt-{stamp}-{n}");
        n += 1;
    }
    let dest = path.with_file_name(&name);
    std::fs::rename(path, &dest)?;
    for side in ["-wal", "-shm"] {
        let from = path.with_file_name(format!("{file}{side}"));
        if from.exists() {
            std::fs::rename(&from, path.with_file_name(format!("{name}{side}")))?;
        }
    }
    tracing::error!(from = %path.display(), to = %dest.display(), "database failed its integrity check; moved aside");
    Ok(dest)
}

/// Puts the newest pre-migration backup back at `path`. The current file, if any, is moved aside
/// first, so nothing is deleted. Returns the backup that was restored, or `NotFound` when there is none.
pub fn restore_newest_backup(path: &Path) -> DbResult<PathBuf> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::other("database file name is not UTF-8"))?;
    let (_, backup) = migrate::backups_of(dir, file)?.pop().ok_or(DbError::NotFound)?;
    if path.exists() {
        quarantine(path)?;
    }
    let tmp = path.with_file_name(format!("{file}.restore-tmp"));
    std::fs::copy(&backup, &tmp)?;
    std::fs::rename(&tmp, path)?;
    Ok(backup)
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

    fn corrupt_siblings(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("manga.db.corrupt-"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_healthy_file_is_not_quarantined() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        drop(open_and_migrate(&p, &MANGA_SCHEMA).unwrap());
        assert!(corrupt_siblings(dir.path()).is_empty());
    }

    #[test]
    fn a_corrupt_file_is_moved_aside_and_the_open_returns_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manga.db");
        std::fs::write(&p, b"this is not a sqlite database, just bytes ".repeat(200)).unwrap();
        let err = open_and_migrate(&p, &MANGA_SCHEMA).unwrap_err();
        let DbError::Corrupt(msg) = err else { panic!("expected Corrupt, got {err}") };
        let kept = corrupt_siblings(dir.path());
        assert_eq!(kept.len(), 1);
        assert!(msg.contains(&kept[0]), "the error names the quarantine path: {msg}");
        let stamp = kept[0].strip_prefix("manga.db.corrupt-").unwrap();
        assert_eq!(stamp.len(), 16, "UTC stamp like the backup names: {stamp}");
        assert!(stamp.ends_with('Z') && stamp.as_bytes()[8] == b'T');
        assert!(std::fs::read(dir.path().join(&kept[0])).unwrap().starts_with(b"this is not"), "bytes are kept");
        assert!(!p.exists() && !dirty_marker(&p).exists(), "the path is free for a new database");
        let c = open_and_migrate(&p, &MANGA_SCHEMA).unwrap();
        assert_eq!(pragma_i64(&c, "user_version"), 34, "the next open creates an empty database");
    }

    #[test]
    fn quarantine_never_overwrites_an_earlier_one() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manga.db");
        let junk = b"not a database ".repeat(300);
        for round in 0..3u8 {
            let mut bytes = junk.clone();
            bytes.push(round);
            std::fs::write(&p, &bytes).unwrap();
            assert!(matches!(open_and_migrate(&p, &MANGA_SCHEMA), Err(DbError::Corrupt(_))));
        }
        let kept = corrupt_siblings(dir.path());
        assert_eq!(kept.len(), 3, "{kept:?}");
        let mut last_bytes: Vec<u8> = kept.iter().map(|n| *std::fs::read(dir.path().join(n)).unwrap().last().unwrap()).collect();
        last_bytes.sort();
        assert_eq!(last_bytes, [0, 1, 2]);
    }

    #[test]
    fn an_empty_file_is_not_treated_as_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manga.db");
        std::fs::write(&p, b"").unwrap();
        drop(open_and_migrate(&p, &MANGA_SCHEMA).unwrap());
        assert!(corrupt_siblings(dir.path()).is_empty());
    }

    #[test]
    fn open_puts_the_dirty_marker_down_and_a_clean_shutdown_takes_it_up() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("manga.db");
        let c = open_and_migrate(&p, &MANGA_SCHEMA).unwrap();
        assert!(dirty_marker(&p).is_file(), "marker exists while the database is open");
        assert_eq!(dirty_marker(&p).file_name().unwrap(), "manga.db.dirty");
        drop(c);
        mark_clean_shutdown(&p).unwrap();
        assert!(!dirty_marker(&p).exists());
        mark_clean_shutdown(&p).unwrap();
    }

    #[test]
    fn a_truncated_file_left_dirty_is_moved_aside_by_the_integrity_check() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        std::fs::write(dirty_marker(&p), b"").unwrap();
        let bytes = std::fs::read(&p).unwrap();
        std::fs::write(&p, &bytes[..bytes.len().min(100)]).unwrap();
        assert!(matches!(open_and_migrate(&p, &MANGA_SCHEMA), Err(DbError::Corrupt(_))), "a truncated dirty file is quarantined");
        assert_eq!(corrupt_siblings(dir.path()).len(), 1);
    }

    #[test]
    fn restore_newest_backup_brings_back_the_pre_migration_copy_of_a_truncated_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        drop(open_and_migrate(&p, &WITH_34).unwrap());
        assert_eq!(backup_files(dir.path()).len(), 1, "migrating 33 to 34 kept one pre-migration backup");
        let bytes = std::fs::read(&p).unwrap();
        std::fs::write(&p, &bytes[..100]).unwrap();

        let restored = restore_newest_backup(&p).unwrap();
        assert!(restored.file_name().unwrap().to_string_lossy().contains(".pre-v33-"));
        let quarantined = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .any(|n| n.starts_with("manga.db.corrupt-"));
        assert!(quarantined, "the truncated file is kept aside, not deleted");
        let c = open_and_migrate(&p, &BASELINE_ONLY).unwrap();
        assert_eq!(pragma_i64(&c, "user_version"), 33, "the restored copy is the version it was taken at");
    }

    #[test]
    fn restore_without_a_backup_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let p = kur_file(dir.path());
        assert!(matches!(restore_newest_backup(&p), Err(DbError::NotFound)));
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
