//! Housekeeping after bulk changes (T-03a-028). Each function runs on the writer connection after the change has
//! committed. Shutdown housekeeping (`PRAGMA optimize`, WAL checkpoint) is [`crate::runtime::Db::close`].

use rusqlite::Connection;

use crate::open::DbResult;

/// Chapters removed in one operation above which the WAL is checkpointed and VACUUM is offered.
pub const BULK_CHAPTER_DELETE: usize = 1000;
/// Entries restored in one operation above which statistics are refreshed with ANALYZE.
pub const BULK_ENTRY_RESTORE: usize = 1000;

/// What a bulk delete left to do. VACUUM is never run here: it rewrites the whole file, so the caller offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AfterDelete {
    pub checkpointed: bool,
    pub offer_vacuum: bool,
}

/// Call after "Clear database" removed `removed_chapters` chapters. Above [`BULK_CHAPTER_DELETE`] the WAL is folded
/// into the main file and VACUUM is offered.
pub fn after_chapter_delete(c: &Connection, removed_chapters: usize) -> DbResult<AfterDelete> {
    if removed_chapters <= BULK_CHAPTER_DELETE {
        return Ok(AfterDelete { checkpointed: false, offer_vacuum: false });
    }
    c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(AfterDelete { checkpointed: true, offer_vacuum: true })
}

/// Call after a legacy import. ANALYZE always runs: an imported file has no statistics for the new indexes.
pub fn after_legacy_import(c: &Connection) -> DbResult<()> {
    c.execute_batch("ANALYZE")?;
    Ok(())
}

/// Call after a restore. ANALYZE runs when more than [`BULK_ENTRY_RESTORE`] entries were restored. Returns whether
/// it ran.
pub fn after_restore(c: &Connection, restored_entries: usize) -> DbResult<bool> {
    if restored_entries <= BULK_ENTRY_RESTORE {
        return Ok(false);
    }
    c.execute_batch("ANALYZE")?;
    Ok(true)
}

/// Rewrites the file to reclaim free pages. Only run on the caller's confirmation.
pub fn vacuum(c: &Connection) -> DbResult<()> {
    c.execute_batch("VACUUM")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_stats(c: &Connection) -> bool {
        c.query_row("SELECT count(*) FROM sqlite_master WHERE name = 'sqlite_stat1'", [], |r| r.get::<_, i64>(0))
            .unwrap()
            == 1
    }

    #[test]
    fn small_deletes_do_not_checkpoint_or_offer_vacuum() {
        let c = Connection::open_in_memory().unwrap();
        assert_eq!(
            after_chapter_delete(&c, BULK_CHAPTER_DELETE).unwrap(),
            AfterDelete { checkpointed: false, offer_vacuum: false }
        );
    }

    #[test]
    fn a_bulk_delete_checkpoints_and_offers_vacuum() {
        let dir = tempfile::tempdir().unwrap();
        let c = Connection::open(dir.path().join("m.db")).unwrap();
        c.pragma_update(None, "journal_mode", "WAL").unwrap();
        c.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);").unwrap();
        assert_eq!(
            after_chapter_delete(&c, BULK_CHAPTER_DELETE + 1).unwrap(),
            AfterDelete { checkpointed: true, offer_vacuum: true }
        );
    }

    #[test]
    fn analyze_runs_after_a_legacy_import_and_after_a_large_restore_only() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE t(x); CREATE INDEX ti ON t(x);").unwrap();
        assert!(!after_restore(&c, 10).unwrap());
        assert!(!has_stats(&c), "a small restore does not analyze");
        assert!(after_restore(&c, BULK_ENTRY_RESTORE + 1).unwrap());
        assert!(has_stats(&c));
        let c2 = Connection::open_in_memory().unwrap();
        c2.execute_batch("CREATE TABLE t(x);").unwrap();
        after_legacy_import(&c2).unwrap();
        assert!(has_stats(&c2));
    }

    #[test]
    fn vacuum_runs_on_request() {
        let c = Connection::open_in_memory().unwrap();
        vacuum(&c).unwrap();
    }
}
