//! The database handle: one writer and `READERS` reader connections on one file, the transaction
//! API and the change publication that live queries wait on. The synchronous `read`/`write` are
//! what the repositories call; `read_async`/`write_async` run the same closures on tokio's blocking
//! pool. Nothing here needs a runtime, except those two and the subscriptions (`subscribe.rs`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use kuro_platform::paths::Paths;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use tokio::sync::broadcast;

use crate::error::{DbError, DbResult};
use crate::notify::{self, Changed, Pending};
use crate::open::{mark_clean_shutdown, open_connection};
use crate::schema_unified;

/// Reader connections per database file.
pub const READERS: usize = 4;

/// Messages a subscription may fall behind by. Past this it re-queries without knowing what changed.
const CHANGE_QUEUE: usize = 256;

/// Committed write transactions between two `PRAGMA wal_checkpoint(PASSIVE)` runs (T-04a-032).
pub const CHECKPOINT_EVERY: u64 = 1000;

/// Uptime between two `PRAGMA optimize` runs while the handle is open (T-04a-032).
pub const OPTIMIZE_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// Housekeeping counters of the writer. Runs after a commit, never inside a write transaction.
#[derive(Debug)]
struct Upkeep {
    commits: u64,
    checkpoints: u64,
    optimizes: u64,
    optimized_at: Instant,
}

/// One writer and [`READERS`] readers on one file. Share it behind an `Arc`.
pub struct Db {
    /// The database file, so `close` can take up its dirty marker.
    path: PathBuf,
    writer: Mutex<Connection>,
    readers: Vec<Mutex<Connection>>,
    next_reader: AtomicUsize,
    /// Tables the open write has touched, collected by the writer's hooks (see `notify`).
    pending: Arc<Pending>,
    /// One message per committed write that touched a table.
    pub(crate) changes: broadcast::Sender<Changed>,
    upkeep: Mutex<Upkeep>,
}

impl Db {
    /// Opens the one database (`<data_dir>/kurobayomi.db`) of `paths`, creating it through the schema's open gate.
    pub fn open(paths: &dyn Paths) -> DbResult<Arc<Self>> {
        let path = schema_unified::db_path(&paths.data_dir());
        let writer = schema_unified::open_file(&path)?;
        Ok(Arc::new(Self::new(&path, writer)?))
    }

    /// Wraps `writer`, which must already be open and migrated (see `schema_unified::open_file`), and opens
    /// the readers on `path`. Readers refuse writes (`query_only`). The hooks go on `writer` only.
    pub fn new(path: &Path, writer: Connection) -> DbResult<Self> {
        let readers = (0..READERS)
            .map(|_| -> DbResult<Mutex<Connection>> {
                let c = open_connection(path)?;
                c.pragma_update(None, "query_only", true)?;
                Ok(Mutex::new(c))
            })
            .collect::<DbResult<Vec<_>>>()?;
        let pending = notify::install(&writer)?;
        let (changes, _) = broadcast::channel(CHANGE_QUEUE);
        let upkeep = Upkeep { commits: 0, checkpoints: 0, optimizes: 0, optimized_at: Instant::now() };
        Ok(Self {
            path: path.to_path_buf(),
            writer: Mutex::new(writer),
            readers,
            next_reader: AtomicUsize::new(0),
            pending,
            changes,
            upkeep: Mutex::new(upkeep),
        })
    }

    /// Runs `f` on one reader. Readers are taken in turn; a reader still in use makes the caller
    /// wait for that slot.
    // ponytail: round-robin slots, not a checkout pool; add a real pool if readers contend.
    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> DbResult<T>) -> DbResult<T> {
        let slot = self.next_reader.fetch_add(1, Ordering::Relaxed) % READERS;
        let conn = lock(&self.readers[slot]);
        f(&conn)
    }

    /// Runs `f` in one `BEGIN IMMEDIATE` transaction on the writer. The transaction commits when `f`
    /// returns `Ok`. An `Err`, or a panic inside `f`, drops it, which rolls everything back.
    pub fn write<T>(&self, f: impl FnOnce(&Transaction<'_>) -> DbResult<T>) -> DbResult<T> {
        let mut conn = lock(&self.writer);
        // Drops what an earlier write touched and then failed, so only this write's tables publish.
        self.pending.begin();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let value = f(&tx)?;
        tx.commit()?;
        // Published after the commit, so a re-query on a reader sees the change.
        if let Some(changed) = self.pending.publish() {
            // A send with no subscriber is the normal case, so its error is ignored.
            let _ = self.changes.send(changed);
        }
        self.after_commit(&conn);
        Ok(value)
    }

    /// Periodic upkeep after a committed write: a passive checkpoint every [`CHECKPOINT_EVERY`] commits and
    /// `PRAGMA optimize` once per [`OPTIMIZE_EVERY`] of uptime. The write already committed, so a failure here
    /// is logged, never returned.
    fn after_commit(&self, conn: &Connection) {
        let mut up = lock(&self.upkeep);
        up.commits += 1;
        if up.commits % CHECKPOINT_EVERY == 0 {
            match conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |_| Ok(())) {
                Ok(()) => up.checkpoints += 1,
                Err(err) => tracing::warn!(error = %err, "passive WAL checkpoint failed"),
            }
        }
        if up.optimized_at.elapsed() >= OPTIMIZE_EVERY {
            match conn.execute_batch("PRAGMA optimize") {
                Ok(()) => up.optimizes += 1,
                Err(err) => tracing::warn!(error = %err, "PRAGMA optimize failed"),
            }
            up.optimized_at = Instant::now();
        }
    }

    /// `(checkpoints, optimizes)` run by the upkeep so far, for tests and diagnostics.
    pub fn upkeep_counts(&self) -> (u64, u64) {
        let up = lock(&self.upkeep);
        (up.checkpoints, up.optimizes)
    }

    /// Shutdown housekeeping on the writer: lets SQLite refresh its statistics and folds the WAL into the
    /// main file (`PRAGMA optimize`, `PRAGMA wal_checkpoint(TRUNCATE)`), then removes the `.dirty` marker so the
    /// next launch runs the quick check only. The handle stays usable.
    pub fn close(&self) -> DbResult<()> {
        let conn = lock(&self.writer);
        // Optimize first: it may write statistics, and the checkpoint should fold those in too.
        conn.execute_batch("PRAGMA optimize")?;
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        mark_clean_shutdown(&self.path)
    }

    /// [`Db::read`] on tokio's blocking pool, for callers on an async task.
    pub async fn read_async<T, F>(self: &Arc<Self>, f: F) -> DbResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> DbResult<T> + Send + 'static,
    {
        let db = Arc::clone(self);
        tokio::task::spawn_blocking(move || db.read(f)).await.map_err(|e| DbError::Task(e.to_string()))?
    }

    /// [`Db::write`] on tokio's blocking pool, for callers on an async task.
    pub async fn write_async<T, F>(self: &Arc<Self>, f: F) -> DbResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> DbResult<T> + Send + 'static,
    {
        let db = Arc::clone(self);
        tokio::task::spawn_blocking(move || db.write(f)).await.map_err(|e| DbError::Task(e.to_string()))?
    }
}

/// What a write closure can do beyond SQL: report tables explicitly and open a savepoint. A write has
/// no nested transaction (`Transaction::transaction` needs `&mut`, which the closure never has).
pub trait TxExt {
    /// Marks `tables` as changed by this write, for a statement SQLite's update hook may not report
    /// (a `DELETE` without `WHERE` can take the truncate optimisation). Published after the commit.
    fn touch(&self, tables: &[&str]) -> DbResult<()>;

    /// Opens a savepoint. `release` keeps its changes; dropping the guard rolls them back.
    fn savepoint(&self) -> DbResult<Savepoint<'_>>;
}

impl TxExt for Transaction<'_> {
    fn touch(&self, tables: &[&str]) -> DbResult<()> {
        for table in tables {
            self.query_row("SELECT kuro_touch(?1)", [table], |_| Ok(()))?;
        }
        Ok(())
    }

    fn savepoint(&self) -> DbResult<Savepoint<'_>> {
        self.execute_batch("SAVEPOINT kuro_sp")?;
        Ok(Savepoint { conn: self, done: false })
    }
}

/// An open savepoint of a write. Dropping it without `release` rolls back to where it was opened.
pub struct Savepoint<'a> {
    conn: &'a Connection,
    done: bool,
}

impl Savepoint<'_> {
    /// Keeps the changes made since the savepoint was opened.
    pub fn release(mut self) -> DbResult<()> {
        self.done = true;
        Ok(self.conn.execute_batch("RELEASE kuro_sp")?)
    }
}

impl Drop for Savepoint<'_> {
    fn drop(&mut self) {
        if !self.done {
            if let Err(e) = self.conn.execute_batch("ROLLBACK TO kuro_sp; RELEASE kuro_sp") {
                tracing::error!(error = %e, "savepoint rollback failed");
            }
        }
    }
}

/// Locks, recovering from poisoning: a panic in an earlier closure already rolled its transaction back.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::*;
    use crate::open::DbError;
    use crate::statements_manga::{CategoryPatch, categories};

    /// A unified file in a fresh directory, with a handle on it.
    fn fixture() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kurobayomi.db");
        let writer = schema_unified::open_file(&path).unwrap();
        let db = Db::new(&path, writer).unwrap();
        (dir, db)
    }

    fn names(db: &Db) -> Vec<String> {
        db.read(|c| Ok(categories::get_categories(c)?.into_iter().map(|r| r.name).collect()))
            .unwrap()
    }

    fn id_of(c: &Connection, name: &str) -> rusqlite::Result<i64> {
        c.query_row("SELECT _id FROM categories WHERE name = ?1", [name], |r| r.get(0))
    }

    #[test]
    fn the_dirty_marker_is_down_after_open_and_up_after_close() {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        let db = Db::open(&paths).unwrap();
        let marker = crate::open::dirty_marker(&schema_unified::db_path(&paths.data_dir()));
        assert!(marker.is_file(), "created on open");
        db.close().unwrap();
        assert!(!marker.exists(), "removed on clean shutdown");
    }

    #[test]
    fn a_committed_write_is_seen_by_every_reader() {
        let (_dir, db) = fixture();
        db.write(|tx| Ok(categories::insert(tx, "Reading", 1, 0)?)).unwrap();
        for _ in 0..=READERS {
            assert!(names(&db).contains(&"Reading".to_owned()));
        }
    }

    #[test]
    fn one_error_rolls_back_insert_update_and_delete_from_three_helpers() {
        let (_dir, db) = fixture();
        db.write(|tx| {
            categories::insert(tx, "Keep", 1, 0)?;
            categories::insert(tx, "Drop", 2, 0)?;
            Ok(())
        })
        .unwrap();
        let before = names(&db);
        let result: DbResult<()> = db.write(|tx| {
            categories::insert(tx, "New", 3, 0)?;
            let keep = id_of(tx, "Keep")?;
            categories::update(
                tx,
                &CategoryPatch {
                    id: keep,
                    name: Some("Renamed".into()),
                    ..Default::default()
                },
            )?;
            categories::delete(tx, id_of(tx, "Drop")?)?;
            Err(DbError::NotFound)
        });
        assert!(matches!(result, Err(DbError::NotFound)));
        assert_eq!(names(&db), before);
    }

    #[test]
    fn a_panic_inside_a_write_rolls_it_back_and_the_handle_still_works() {
        let (_dir, db) = fixture();
        let before = names(&db);
        let panicked = catch_unwind(AssertUnwindSafe(|| {
            db.write(|tx| -> DbResult<()> {
                categories::insert(tx, "Panicky", 1, 0)?;
                panic!("boom");
            })
        }));
        assert!(panicked.is_err());
        assert_eq!(names(&db), before);
        db.write(|tx| Ok(categories::insert(tx, "After", 1, 0)?)).unwrap();
        assert!(names(&db).contains(&"After".to_owned()));
    }

    #[test]
    fn readers_refuse_writes() {
        let (_dir, db) = fixture();
        let result = db.read(|c| Ok(categories::insert(c, "Sneaky", 1, 0)?));
        assert!(result.is_err());
        assert!(!names(&db).contains(&"Sneaky".to_owned()));
    }
}

#[cfg(test)]
mod publication_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::notify::{LIBRARY_DEPS, Tables};
    use crate::statements_manga::fixtures::{chapter, manga};
    use crate::statements_manga::{ChapterPatch, categories, chapters, mangas};
    use kuro_testkit::TempPaths;

    fn fixture() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kurobayomi.db");
        let writer = schema_unified::open_file(&path).unwrap();
        (dir, Db::new(&path, writer).unwrap())
    }

    /// Every publication since the last call, as the table bits it names.
    fn published(rx: &mut broadcast::Receiver<Changed>) -> Vec<Tables> {
        let mut out = Vec::new();
        while let Ok(changed) = rx.try_recv() {
            out.push(Tables::of_changed(&changed));
        }
        out
    }

    #[test]
    fn a_single_insert_publishes_the_entries_bit() {
        let (_d, db) = fixture();
        let mut rx = db.changes.subscribe();
        db.write(|tx| Ok(mangas::insert(tx, &manga("/a"))?)).unwrap();
        assert_eq!(published(&mut rx), [Tables::ENTRIES], "TC-N01");
    }

    #[test]
    fn a_rolled_back_write_publishes_nothing() {
        let (_d, db) = fixture();
        let mut rx = db.changes.subscribe();
        let result: DbResult<()> = db.write(|tx| {
            let id = mangas::insert(tx, &manga("/a"))?;
            chapters::insert(tx, &chapter(id, "/a/1"))?;
            Err(DbError::NotFound)
        });
        assert!(result.is_err());
        assert_eq!(published(&mut rx), Vec::<Tables>::new(), "TC-N02");
    }

    #[test]
    fn a_chapter_read_update_publishes_items_and_the_trigger_driven_entries() {
        let (_d, db) = fixture();
        let chapter_id = db
            .write(|tx| {
                let m = mangas::insert(tx, &manga("/a"))?;
                Ok(chapters::insert(tx, &chapter(m, "/a/1"))?)
            })
            .unwrap();
        let mut rx = db.changes.subscribe();
        db.write(|tx| {
            Ok(chapters::update(tx, &ChapterPatch { id: chapter_id, read: Some(true), ..Default::default() })?)
        })
        .unwrap();
        assert_eq!(published(&mut rx), [Tables::ITEMS | Tables::ENTRIES], "TC-N03");
    }

    #[test]
    fn a_cascade_delete_publishes_entries_items_and_history() {
        let (_d, db) = fixture();
        let manga_id = db
            .write(|tx| {
                let m = mangas::insert(tx, &manga("/a"))?;
                let c = chapters::insert(tx, &chapter(m, "/a/1"))?;
                tx.execute("INSERT INTO history(item_id, last_at, duration_ms) VALUES (?1, 1, 0)", [c])?;
                Ok(m)
            })
            .unwrap();
        let mut rx = db.changes.subscribe();
        db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [manga_id]).map(drop)?)).unwrap();
        assert_eq!(published(&mut rx), [Tables::ENTRIES | Tables::ITEMS | Tables::HISTORY], "TC-N09");
    }

    #[test]
    fn category_writes_publish_only_the_categories_bit() {
        let (_d, db) = fixture();
        let mut rx = db.changes.subscribe();
        db.write(|tx| Ok(categories::insert(tx, "Reading", 1, 0)?)).unwrap();
        assert_eq!(published(&mut rx), [Tables::CATEGORIES]);
    }

    #[tokio::test]
    async fn a_library_subscription_runs_for_a_chapter_write_but_not_a_category_write() {
        let (_d, db) = fixture();
        let db = Arc::new(db);
        let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let mut sub = db.subscribe_tables(LIBRARY_DEPS, Duration::ZERO, move |c| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(mangas::get_favorites(c)?.len())
        });
        assert_eq!(sub.changed().await.map(|r| r.map(|v| *v)), Some(Ok(0)), "emits once immediately");
        db.write_async(|tx| Ok(categories::insert(tx, "Reading", 1, 0)?)).await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(300), sub.changed()).await.is_err());
        assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1, "a category write does not run the library query");
        db.write_async(|tx| {
            let m = mangas::insert(tx, &manga("/a"))?;
            Ok(chapters::insert(tx, &chapter(m, "/a/1"))?)
        })
        .await
        .unwrap();
        let got = tokio::time::timeout(Duration::from_secs(2), sub.changed()).await.expect("re-run in time");
        assert_eq!(got.map(|r| r.map(|v| *v)), Some(Ok(1)), "the manga is a favourite and now listed");
        assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 2, "start and the manga write");
    }

    #[test]
    fn open_creates_the_one_file_in_the_data_dir_with_the_connection_pragmas() {
        let paths = TempPaths::new().unwrap();
        let db = Db::open(&paths).unwrap();
        assert!(schema_unified::db_path(&paths.data_dir()).is_file());
        assert!(schema_unified::legacy_files(&paths.data_dir()).is_empty());
        db.read(|c| {
            let pragma = |name: &str| -> DbResult<i64> { Ok(c.pragma_query_value(None, name, |r| r.get(0))?) };
            assert_eq!(pragma("foreign_keys")?, 1);
            assert_eq!(pragma("synchronous")?, 1);
            assert_eq!(pragma("busy_timeout")?, 5000);
            assert_eq!(pragma("recursive_triggers")?, 0);
            assert_eq!(pragma("user_version")?, schema_unified::SCHEMA_VERSION);
            assert_eq!(pragma("application_id")?, schema_unified::APPLICATION_ID);
            let mode: String = c.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
            assert_eq!(mode, "wal");
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn read_async_and_write_async_run_the_same_closures() {
        let (_d, db) = fixture();
        let db = Arc::new(db);
        db.write_async(|tx| Ok(categories::insert(tx, "Async", 1, 0)?)).await.unwrap();
        let count = |db: &Arc<Db>| {
            let db = Arc::clone(db);
            async move { db.read_async(|c| Ok(categories::get_categories(c)?.len())).await.unwrap() }
        };
        assert_eq!(count(&db).await, 2, "the system row and the new category");
        let rolled_back: DbResult<()> = db
            .write_async(|tx| {
                categories::insert(tx, "Gone", 2, 0)?;
                Err(DbError::NotFound)
            })
            .await;
        assert!(matches!(rolled_back, Err(DbError::NotFound)));
        assert_eq!(count(&db).await, 2, "the failed write left nothing behind");
    }

    #[tokio::test]
    async fn read_async_and_write_async_run_the_closure_off_the_calling_thread() {
        let (_d, db) = fixture();
        let db = Arc::new(db);
        let caller = std::thread::current().id();
        let on_read = db.read_async(|_| Ok(std::thread::current().id())).await.unwrap();
        let on_write = db.write_async(|_| Ok(std::thread::current().id())).await.unwrap();
        assert_ne!(on_read, caller, "read_async runs on the blocking pool");
        assert_ne!(on_write, caller, "write_async runs on the blocking pool");
        let inline = db.read(|_| Ok(std::thread::current().id())).unwrap();
        assert_eq!(inline, caller, "the synchronous read runs on the calling thread");
    }
}

#[cfg(test)]
mod upkeep_tests {
    use super::*;

    #[test]
    fn a_passive_checkpoint_runs_after_every_thousand_commits_and_not_before() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kurobayomi.db");
        let db = Db::new(&path, schema_unified::open_file(&path).unwrap()).unwrap();
        for _ in 0..(CHECKPOINT_EVERY - 1) {
            db.write(|_tx| Ok(())).unwrap();
        }
        assert_eq!(db.upkeep_counts(), (0, 0), "no checkpoint before the 1000th commit");
        db.write(|_tx| Ok(())).unwrap();
        assert_eq!(db.upkeep_counts().0, 1, "the 1000th commit checkpoints once");
    }

    #[test]
    fn a_failed_write_is_not_counted_as_a_commit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kurobayomi.db");
        let db = Db::new(&path, schema_unified::open_file(&path).unwrap()).unwrap();
        for _ in 0..CHECKPOINT_EVERY {
            let _ = db.write::<()>(|_tx| Err(DbError::Task("boom".into())));
        }
        assert_eq!(db.upkeep_counts(), (0, 0));
    }
}
