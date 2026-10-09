//! The Aniyomi `tachiyomi.db` importer (T-03a-023; steps 1 to 9 of `p1-topic-migration-history.md` section 5).
//!
//! The source file is never opened by SQLite: it is copied (with `-wal` and `-shm`) into a work directory next to
//! the destination, and only the copy is migrated and read. The new database is built at the work directory and
//! moved to `dest` as the last step, so a failure leaves nothing at `dest`, and the user's live `manga.db` is
//! never touched by [`import_aniyomi_db`]. [`install_imported`] swaps the result in after the user confirmed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::error::DbError;
use crate::legacy::{self, NEWEST_IMPORTABLE, OLDEST_IMPORTABLE};
use crate::open::APPLICATION_ID;
use crate::schema::{BASELINE_SQL, MANGA_MIGRATIONS};

/// Tables copied into the new file, in dependency order.
const COPY_ORDER: [&str; 9] =
    ["categories", "mangas", "chapters", "mangas_categories", "history", "manga_sync", "excluded_scanlators", "sources", "extension_repos"];

/// A deviation of the old file that the importer saw or applied (reported, never silently fixed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quirk {
    /// Migration 10 gave every manga the same `date_added`. The dates are kept as they are; the updates feed is
    /// wrong for such rows until the entry is added again.
    DateAddedBackfillBug,
    /// Migration 15 left a stale `history_temp` table; it was dropped.
    HistoryTempDropped,
    /// `mangas_categories.last_modified_at` (added by migration 26) is not part of the new schema and was dropped.
    MangasCategoriesTimestampDropped,
}

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    /// `PRAGMA user_version` of the source.
    pub source_version: i64,
    /// Rows copied per table.
    pub copied: BTreeMap<&'static str, u64>,
    /// Orphan rows deleted from the copy before the rows moved, per child table.
    pub orphans_removed: BTreeMap<&'static str, u64>,
    pub quirks: Vec<Quirk>,
}

impl ImportReport {
    /// All orphan rows that were dropped.
    pub fn orphans_total(&self) -> u64 {
        self.orphans_removed.values().sum()
    }
}

/// Where the import is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    CopyingSource,
    Migrating { from: i64, to: i64 },
    RemovingOrphans,
    BuildingTarget,
    CopyingTable(&'static str),
    Finishing,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("This database comes from a newer Aniyomi version")]
    TooNew { found: i64 },
    #[error("Database too old; open it once in Aniyomi/Mihon first")]
    TooOld { found: i64 },
    #[error("This file is not an Aniyomi manga database")]
    NotAnAniyomiDatabase,
    #[error("The import target already exists: {0}")]
    DestinationExists(PathBuf),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Removes the work directory on every exit path.
struct WorkDir(PathBuf);

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Imports the Aniyomi database `src` into a new Kurobayomi manga database at `dest` (which must not exist).
/// Versions 14 to 33 are accepted. `progress` is called as each step starts.
pub fn import_aniyomi_db(src: &Path, dest: &Path, progress: &mut dyn FnMut(Progress)) -> Result<ImportReport, ImportError> {
    if dest.exists() {
        return Err(ImportError::DestinationExists(dest.to_owned()));
    }
    let parent = dest.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let work = WorkDir(parent.join(format!(
        ".{}.import-work",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("manga.db")
    )));
    let _ = std::fs::remove_dir_all(&work.0);
    std::fs::create_dir_all(&work.0)?;

    progress(Progress::CopyingSource);
    let copy = work.0.join("source.db");
    copy_with_siblings(src, &copy)?;

    let c = Connection::open(&copy)?;
    c.pragma_update(None, "foreign_keys", "OFF")?;
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let application_id: i64 = c.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let has_mangas: i64 = c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'mangas'", [], |r| r.get(0))?;
    if application_id == APPLICATION_ID || has_mangas == 0 {
        return Err(ImportError::NotAnAniyomiDatabase);
    }
    if version > NEWEST_IMPORTABLE {
        return Err(ImportError::TooNew { found: version });
    }
    if version < OLDEST_IMPORTABLE {
        return Err(ImportError::TooOld { found: version });
    }
    let mut report = ImportReport { source_version: version, ..ImportReport::default() };

    let had_history_temp = table_exists(&c, "history_temp")?;
    legacy::replay_with(&c, version, true, &mut |n| progress(Progress::Migrating { from: n, to: n + 1 }))?;
    let history_temp_dropped = had_history_temp || version <= 15;
    c.execute_batch("DROP TABLE IF EXISTS history_temp")?;
    if history_temp_dropped {
        report.quirks.push(Quirk::HistoryTempDropped);
    }

    progress(Progress::RemovingOrphans);
    remove_orphans(&c, &mut report)?;
    if column_exists(&c, "mangas_categories", "last_modified_at")? {
        report.quirks.push(Quirk::MangasCategoriesTimestampDropped);
    }
    if shared_date_added(&c)? {
        report.quirks.push(Quirk::DateAddedBackfillBug);
    }

    progress(Progress::BuildingTarget);
    let built = work.0.join("new.db");
    build_target(&copy, &built, &mut report, progress)?;
    drop(c);

    progress(Progress::Finishing);
    std::fs::rename(&built, dest)?;
    Ok(report)
}

/// Copies `src` and its `-wal` / `-shm` siblings to `dest` and its siblings. `src` is only read.
fn copy_with_siblings(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dest)?;
    for side in ["-wal", "-shm"] {
        let from = sibling(src, side);
        if from.is_file() {
            std::fs::copy(&from, sibling(dest, side))?;
        }
    }
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

fn table_exists(c: &Connection, name: &str) -> rusqlite::Result<bool> {
    c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |r| r.get::<_, i64>(0)).map(|n| n > 0)
}

fn column_exists(c: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    c.query_row("SELECT count(*) FROM pragma_table_info(?1) WHERE name = ?2", [table, column], |r| r.get::<_, i64>(0)).map(|n| n > 0)
}

/// Step 4: deletes the rows `PRAGMA foreign_key_check` would report, child tables after their parents.
fn remove_orphans(c: &Connection, report: &mut ImportReport) -> rusqlite::Result<()> {
    let rules: [(&'static str, &str); 5] = [
        ("chapters", "DELETE FROM chapters WHERE manga_id NOT IN (SELECT _id FROM mangas)"),
        ("history", "DELETE FROM history WHERE chapter_id NOT IN (SELECT _id FROM chapters)"),
        ("manga_sync", "DELETE FROM manga_sync WHERE manga_id NOT IN (SELECT _id FROM mangas)"),
        (
            "mangas_categories",
            "DELETE FROM mangas_categories WHERE manga_id NOT IN (SELECT _id FROM mangas) OR category_id NOT IN (SELECT _id FROM categories)",
        ),
        ("excluded_scanlators", "DELETE FROM excluded_scanlators WHERE manga_id NOT IN (SELECT _id FROM mangas)"),
    ];
    for (table, sql) in rules {
        let removed = c.execute(sql, [])? as u64;
        if removed > 0 {
            report.orphans_removed.insert(table, removed);
        }
    }
    Ok(())
}

/// The migration 10 signature: two or more mangas that all carry one non-zero `date_added`.
fn shared_date_added(c: &Connection) -> rusqlite::Result<bool> {
    let (distinct, total): (i64, i64) =
        c.query_row("SELECT count(DISTINCT date_added), count(*) FROM mangas WHERE date_added <> 0", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(total >= 2 && distinct == 1 && c.query_row("SELECT count(*) FROM mangas", [], |r| r.get::<_, i64>(0))? == total)
}

/// Steps 5 to 7: a new file with the current schema, the rows copied in before the triggers exist (so `version`,
/// `last_modified_at` and `favorite_modified_at` arrive unchanged), the triggers back, then `ANALYZE`.
fn build_target(
    migrated_path: &Path,
    built: &Path,
    report: &mut ImportReport,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), ImportError> {
    let t = Connection::open(built)?;
    t.pragma_update(None, "foreign_keys", "OFF")?;
    t.execute_batch(BASELINE_SQL)?;
    for (_, sql) in MANGA_MIGRATIONS {
        t.execute_batch(sql)?;
    }
    // triggers out while the rows move
    let triggers: Vec<String> = {
        let mut st = t.prepare("SELECT sql FROM sqlite_master WHERE type = 'trigger' ORDER BY rowid")?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    let names: Vec<String> = {
        let mut st = t.prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'")?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    for name in &names {
        t.execute_batch(&format!("DROP TRIGGER \"{name}\""))?;
    }
    t.execute("ATTACH DATABASE ?1 AS src", [migrated_path.to_string_lossy().as_ref()])?;
    t.execute_batch("BEGIN IMMEDIATE")?;
    for table in COPY_ORDER {
        progress(Progress::CopyingTable(table));
        let columns = target_columns(&t, table)?;
        let list = columns.join(", ");
        let select = columns
            .iter()
            .map(|c| if table == "history" && c == "last_read" { "coalesce(last_read, 0)".to_owned() } else { c.clone() })
            .collect::<Vec<_>>()
            .join(", ");
        let filter = if table == "categories" { " WHERE _id <> 0" } else { "" };
        let rows = t.execute(&format!("INSERT INTO main.{table} ({list}) SELECT {select} FROM src.{table}{filter}"), [])?;
        report.copied.insert(table, rows as u64);
    }
    // the system category keeps its sort flags and visibility
    t.execute_batch(
        "UPDATE main.categories SET
            flags = coalesce((SELECT flags FROM src.categories WHERE _id = 0), flags),
            hidden = coalesce((SELECT hidden FROM src.categories WHERE _id = 0), hidden)
         WHERE _id = 0",
    )?;
    for sql in &triggers {
        t.execute_batch(sql)?;
    }
    t.execute_batch("COMMIT")?;
    t.execute_batch("DETACH DATABASE src")?;
    let broken: i64 = t.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
    if broken > 0 {
        return Err(DbError::Corrupt(format!("{broken} rows break a foreign key after the import")).into());
    }
    t.pragma_update(None, "application_id", APPLICATION_ID)?;
    t.pragma_update(None, "user_version", crate::migrate::latest_version(crate::schema::BASELINE_VERSION, MANGA_MIGRATIONS))?;
    t.execute_batch("ANALYZE")?;
    Ok(())
}

fn target_columns(t: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut st = t.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
    st.query_map([table], |r| r.get(0))?.collect()
}

/// Step 9: puts the imported file in place of the live database. The live file (with `-wal` and `-shm`) is renamed to
/// `<file>.bak-<UTC yyyyMMddHHmmss>` and never deleted; the caller asks for confirmation first when the live database
/// has entries. When the move fails the live file is put back. Returns the backup path, or `None` when there was no
/// live file.
pub fn install_imported(live: &Path, imported: &Path) -> Result<Option<PathBuf>, ImportError> {
    let backup = if live.exists() {
        let stamp = utc_stamp(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
        let name = live.file_name().and_then(|n| n.to_str()).unwrap_or("manga.db");
        let backup = live.with_file_name(format!("{name}.bak-{stamp}"));
        std::fs::rename(live, &backup)?;
        for side in ["-wal", "-shm"] {
            let from = sibling(live, side);
            if from.exists() {
                std::fs::rename(&from, sibling(&backup, side))?;
            }
        }
        Some(backup)
    } else {
        None
    };
    if let Err(e) = std::fs::rename(imported, live) {
        if let Some(backup) = &backup {
            let _ = std::fs::rename(backup, live);
            for side in ["-wal", "-shm"] {
                let from = sibling(backup, side);
                if from.exists() {
                    let _ = std::fs::rename(&from, sibling(live, side));
                }
            }
        }
        return Err(e.into());
    }
    Ok(backup)
}

/// `yyyyMMddHHmmss` of a Unix time in UTC.
pub fn utc_stamp(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}
