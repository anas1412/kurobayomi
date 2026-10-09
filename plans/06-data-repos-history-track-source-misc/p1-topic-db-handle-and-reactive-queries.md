# Topic: replacing the SQLDelight database handlers (`Db`, transactions, reactive queries)

Scope files: `data/src/main/java/tachiyomi/data/handlers/{manga,anime}/{Android*DatabaseHandler,*DatabaseHandler,*TransactionContext,QueryPaging*Source}.kt` (8 files, 1,157 LOC of near-identical manga/anime twins). Behaviour is described in `p1-behavior.md` section 1; this file is the design that replaces it and the reasoning.

## 1. What the Kotlin handler actually provides (so nothing is lost)

| Capability | Kotlin mechanism | Needed by Kurobayomi? |
|---|---|---|
| Run a query off the main thread | `withContext(Dispatchers.IO)` | Yes: GTK main thread must never block on SQLite |
| Run several statements atomically | `await(inTransaction = true)`, SQLDelight `transactionWithResult` | Yes (track insert-all, custom button renumbering, library sync in area:05/10/26) |
| Nested transactions from inside a suspending block | `TransactionElement` coroutine context element + reference count | Not as such: Rust closures are synchronous, nesting is a function call |
| One writer, many readers | SQLDelight/Android driver: one connection, transactions queue first-come-first-served | Improve: WAL allows concurrent readers while one writer runs; reads must not queue behind a long library-update transaction |
| Reactive queries (`asFlow().mapToList`) | SQLDelight query listeners keyed by table, notified after commit | Yes: History, Updates, Library, Track list and Extension stores all refresh live |
| Paging3 source over limit/offset | `QueryPagingMangaSource` | No (no caller) |
| Cancellation safety | context job tied to `controlJob`; rollback on cancel | Rust: closures run to completion once started; dropping the awaiting future does not abort a write (documented below) |

## 2. API (module `kuro_core::db`)

```rust
#[derive(Clone)]
pub struct Db { inner: Arc<Inner> }

pub enum DbKind { Manga, Anime }

pub trait Schema: Sync + Send {          // implemented by area:03 (manga) and area:04 (anime)
    fn version(&self) -> i64;                                          // SQLDelight Schema.version, stored in PRAGMA user_version
    fn create(&self, conn: &Connection) -> Result<(), DbError>;        // runs in one transaction on a fresh file
    fn migrate(&self, conn: &Connection, from: i64, to: i64) -> Result<(), DbError>; // runs in one transaction
}

impl Db {
    /// Opens (creating/migrating) the file. Sets PRAGMAs on EVERY connection (foreign_keys is per connection).
    pub fn open(path: &Path, kind: DbKind, schema: &'static dyn Schema, rt: tokio::runtime::Handle) -> Result<Db, DbError>;
    /// Test helper: creates a fresh database in a temp directory (real file, WAL works, no `:memory:` because readers need a second connection).
    #[cfg(any(test, feature = "testkit"))]
    pub fn open_temp(kind: DbKind, schema: &'static dyn Schema) -> (Db, tempfile::TempDir);

    pub async fn read<T: Send + 'static>(&self, f: impl FnOnce(&Connection) -> Result<T, DbError> + Send + 'static) -> Result<T, DbError>;
    pub async fn write<T: Send + 'static>(&self, f: impl FnOnce(&Connection) -> Result<T, DbError> + Send + 'static) -> Result<T, DbError>;
    pub fn read_blocking<T>(&self, f: impl FnOnce(&Connection) -> Result<T, DbError>) -> Result<T, DbError>;
    pub fn write_blocking<T>(&self, f: impl FnOnce(&Connection) -> Result<T, DbError>) -> Result<T, DbError>;

    /// Reactive query: emits once immediately, then again after every committed write that touched one of `tables`.
    pub fn watch<T, F>(&self, tables: TableSet, f: F) -> Subscription<T>
        where T: Send + Sync + 'static, F: Fn(&Connection) -> Result<T, DbError> + Send + Sync + 'static;
    pub fn watch_distinct<T: PartialEq, F>(&self, tables: TableSet, f: F) -> Subscription<T> /* skips equal consecutive values (Kotlin's distinctUntilChanged) */;
}

pub struct Subscription<T> { /* tokio::sync::watch::Receiver<Option<Result<Arc<T>, DbError>>> + abort handle */ }
impl<T> Subscription<T> {
    pub async fn next(&mut self) -> Option<Result<Arc<T>, DbError>>;   // None when the Db is dropped; dropping the Subscription stops the task
}
```

Why no repository trait and no `Handler` trait: there is exactly one implementation of each; mocks are replaced by real temp-file databases in tests (a real SQLite round trip costs microseconds). Mapping of the Kotlin methods:

| Kotlin | Rust |
|---|---|
| `await { queries.x() }` (write) | `db.write(|c| ...)` |
| `awaitList` | `db.read(|c| stmt.query_map(...)?.collect())` |
| `awaitOne` | `c.query_row(...)` (error `DbError::Sqlite(QueryReturnedNoRows)`; helper `one()` maps it to `DbError::NotFound`) |
| `awaitOneOrNull` | `c.query_row(...).optional()` |
| `awaitOneExecutable` (insert then `last_insert_rowid`) | `c.last_insert_rowid()` inside the same closure |
| `subscribeToList/One/OneOrNull` | `db.watch(tables, |c| ...)` |
| `subscribeToPagingSource` | dropped |

## 3. Transaction model

* One writer connection: `parking_lot::Mutex<Connection>` (no lock poisoning if a closure panics). `write` runs `spawn_blocking`, locks the writer, `conn.transaction_with_behavior(TransactionBehavior::Immediate)`, calls the closure with `&Transaction` deref'd to `&Connection`, then `commit()` on `Ok`, drops (rolls back) on `Err` or panic. IMMEDIATE takes the write lock at `BEGIN`, so there is no read-then-upgrade deadlock window (SQLite returns `SQLITE_BUSY` immediately for upgrades inside WAL).
* Every `write`, even a single statement, is a transaction: equivalent to autocommit for single statements and gives atomicity to composed repository functions for free.
* Composition: repository code is written as plain functions over `&Connection` (`fn upsert_history(c: &Connection, u: &MangaHistoryUpdate) -> Result<()>`) plus a thin async wrapper (`pub async fn upsert(&self, u) { self.db.write(move |c| upsert_history(c, &u)).await }`). Two repositories can then run inside one `write` closure by calling both functions. This replaces `TransactionElement` + reference counting.
* Nested `write`: `write_blocking` inside a closure already running on the writer would deadlock on the mutex. A thread-local `IN_WRITE: Cell<bool>` is set around the closure; `write_blocking` checks it and returns `DbError::NestedWrite` (plus a `debug_assert!`). Reads are not checked: they use a pooled connection and cannot deadlock (they see the pre-transaction snapshot, which is the documented WAL behaviour).
* Cancellation: `spawn_blocking` tasks cannot be aborted once running. If the awaiting future is dropped (screen closed), the closure still completes and commits. This matches the Kotlin interactors that wrap writes in `withNonCancellableContext` and is the desired behaviour for "save progress when the player closes".
* `busy_timeout = 5000`: only relevant if another process (a second app instance, `sqlite3` CLI) holds the lock.
* Reader pool: `r2d2` with `r2d2_sqlite::SqliteConnectionManager::file(path).with_init(apply_pragmas)`, `max_size = 4`, `min_idle = Some(1)`. WAL readers see the last committed snapshot and never block the writer.
* PRAGMAs applied to every connection (`apply_pragmas`): `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL` (the three from `AppModule.kt`), plus `busy_timeout`. `journal_mode = WAL` is persistent in the file but is re-issued harmlessly.
* Savepoints: if a repository function needs partial rollback inside a transaction, use `conn.unchecked_transaction()` (savepoint-style nested transaction in rusqlite) — none of the ported code needs it today.

## 4. Reactive queries without SQLDelight

SQLDelight: every generated query registers a listener on the set of tables it reads; after a transaction commits it calls `notifyQueries` for the tables the executed statements wrote; listeners re-run (library behaviour).

Rust design:

1. `Table` is an enum of every table that appears in a watch (exactly the tables of the two schemas: manga DB `mangas, chapters, history, categories, mangas_categories, excluded_scanlators, manga_sync, sources, extension_repos`; anime DB `animes, episodes, animehistory, categories, animes_categories, anime_sync, animesources, extension_store, custom_buttons, anime_relations`; Rust variants `Mangas, Chapters, History, Categories, MangasCategories, ExcludedScanlators, MangaSync, Sources, ExtensionRepos, Animes, Episodes, AnimeHistory, AnimesCategories, AnimeSync, AnimeSources, ExtensionStore, CustomButtons, AnimeRelations`; `categories` exists in both files and maps to the single variant `Categories`), each a bit of `TableSet(u64)` (at most 64). `Table::from_sql_name("history")` maps SQLite's name to the enum; unknown names (sqlite internal) map to `None`.
2. The writer connection registers `conn.update_hook(Some(|_action, _db, table, _rowid| dirty.lock().insert(Table::from_sql_name(table))))` once at open. rusqlite feature `hooks` is required. The hook fires for INSERT, UPDATE and DELETE statements **including rows deleted by `ON DELETE CASCADE`** (a deleted chapter dirties `History`; a deleted manga dirties `MangaSync`, `Chapters`, `History` transitively), which is the property that makes this approach more robust than "each repository declares the tables it touches".
3. Known SQLite gaps (documented in the SQLite manual for `sqlite3_update_hook`): rows deleted by `INSERT OR REPLACE`/`ON CONFLICT REPLACE` do not fire the hook (the accompanying INSERT does, so the table is still dirtied), and **`DELETE FROM t` without a WHERE clause may use the truncate optimisation and fire no hook**. Rule for this codebase: every "delete everything" statement is written `DELETE FROM <table> WHERE 1` (this disables the optimisation); a unit test asserts that `history::delete_all` notifies (T-06a-003 acceptance).
4. `write` after a successful `commit()` takes the dirty set (`std::mem::take`) and publishes it on a `tokio::sync::broadcast::Sender<TableSet>` (capacity 64). After a rollback the set is cleared without publishing. Publishing **after** commit (not in a commit hook) guarantees a re-query on a reader connection sees the committed data.
5. `watch(tables, f)` spawns a task on the runtime: subscribe to the broadcast first, run `f` once through `read` and publish `Some(Ok(Arc<T>))`, then loop: wait for a `TableSet`; `RecvError::Lagged` counts as "everything changed"; drain any already-queued notifications (coalescing: a burst of 100 chapter inserts causes one re-run, not 100; Kotlin's `collectLatest` consumers get the same effect); if the union intersects `tables`, re-run `f` and publish. A failed run publishes `Some(Err(e))` and the task continues (Kotlin: the flow terminates on error and the screen model's `.catch` sends `Event.InternalError`; Rust keeps the subscription alive so the next successful commit recovers the screen — the UI still shows the error event once).
6. `Subscription::next` is executor-agnostic (tokio `watch::Receiver::changed` does not need a tokio runtime context), so GTK code can `glib::spawn_future_local(async move { while let Some(v) = sub.next().await { ... } })` and update widgets on the main thread without a channel bridge.
7. Dropping the `Subscription` closes the watch sender; the task notices via `tx.closed()` in its `select!` and exits. No leaks when a screen is closed.
8. Writes performed by another process (e.g. restoring a backup by replacing the file) are not observed; backup restore (area:22) must run through `Db::write` or call `Db::notify_all()` (a public method that publishes `TableSet::ALL`) after swapping data.

## 5. Dates and ids

`Date`-typed columns (`history.last_read`, `animehistory.last_seen`) are plain `Option<i64>` milliseconds (`DateColumnAdapter`). A helper `fn millis_now() -> i64` (`SystemTime::now()` since UNIX_EPOCH) is the only clock source used by repositories, so tests can pass explicit values. No `chrono` type crosses the repository boundary; the UI converts with `chrono::DateTime::from_timestamp_millis` and `Local`.

## 6. Pitfalls

* `foreign_keys` is a per-connection pragma. Forgetting it on pool connections would silently disable cascades only for reads — harmless — but forgetting on the writer would leave orphaned history/track rows. A test asserts `PRAGMA foreign_keys` returns 1 on the writer and on a pooled connection.
* `r2d2` connections hold a WAL read snapshot only while a statement is executing; do not keep a `Statement` alive across `.await` (closures are synchronous so this cannot happen in the API as designed).
* `rusqlite::Connection` is `Send` but not `Sync`; the closure signature `FnOnce(&Connection)` is enough, never share it.
* The `Schema` migrations of area:03/04 run on the writer connection inside `BEGIN EXCLUSIVE` (SQLDelight runs them inside one transaction; failures leave the file at the old version).
* Opening both databases happens on the blocking pool during warm-up (area:01 `p1-topic-app-context-and-startup.md` section 4); `Db::open` is synchronous and must not be called on the GTK main thread after startup.
* A missing `ORDER BY` is never relied upon: every list query in this area has an explicit order (see `p1-topic-history-and-updates-views.md`).

## 7. What is deliberately not ported

* `QueryPagingMangaSource`/`QueryPagingAnimeSource` (Paging3 over offset/limit, `jumpingSupported = true`, `itemsBefore/After` placeholders): zero callers. If a very long list ever needs windowing, the GTK answer is a `gio::ListModel` that fetches pages on demand — a different mechanism, not a port. User loses nothing today.
* `suspendingTransactionId` thread-local and `TransactionElement`: Kotlin-coroutine plumbing.
* Android `SupportSQLiteDatabase`/Requery factory/database inspector switch: replaced by rusqlite `bundled`.
