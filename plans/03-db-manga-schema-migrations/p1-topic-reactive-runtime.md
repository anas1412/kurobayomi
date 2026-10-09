# Area 03 / part 1 — Topic: open contract, transactions and reactive queries (SQLDelight handler -> rusqlite)

Ownership note: the Kotlin handler classes (`data/src/main/java/tachiyomi/data/handlers/manga/*.kt`) belong to area:06-data-repos-history-track-source-misc. This document defines only what area 03 must guarantee to them: the open contract (pragmas, version gate), the table dependency set of every subscribed query, and a verified reference implementation of change notification that area 06 may adopt.

## 1. OBSERVED behaviour of the Aniyomi runtime

### 1.1 Open (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-101`)

- File name `tachiyomi.db` (manga) and `tachiyomi.animedb` (anime), created by `AndroidSqliteDriver` in the app's database directory.
- `onOpen`: `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL`. No `busy_timeout`, no `temp_store`, no `cache_size`, no `recursive_triggers`.
- SQLite engine: `com.github.requery:sqlite-android:3.45.0` (`gradle/libs.versions.toml:39`), debug builds on API 30+ use the framework SQLite for Android Studio's inspector. SQLDelight 2.0.2, dialect `sqlite-3-38-dialect`.
- Two separate `Database` classes (`tachiyomi.data.Database`, package from `data/build.gradle.kts` `packageName.set("tachiyomi.data")`; `tachiyomi.mi.data.AnimeDatabase`), each with its own file and its own version counter.

### 1.2 Handler dispatch (`AndroidMangaDatabaseHandler.kt`, `MangaTransactionContext.kt`)

1. `await(inTransaction = false)`: if the caller is already on a transaction thread (`driver.currentTransaction() != null`) run inline; otherwise `withContext(Dispatchers.IO)`.
2. `await(inTransaction = true)`: `withMangaTransaction`: reuse the surrounding transaction context if one exists (nested calls join the outer transaction; a reference counter releases the thread when it reaches 0), otherwise acquire ONE thread from the query dispatcher and keep it for the duration; run `db.transactionWithResult { runBlocking(txContext) { block() } }`. SQLDelight commits on normal return and rolls back on exception or coroutine cancellation. "SQLDelight will only perform at most one transaction at a time, additional transactions are queued first come first served" (comment in the source).
3. `awaitOne` throws when no row (`executeAsOne`); `awaitOneOrNull` returns null; `awaitList` returns possibly empty.
4. `subscribeToList/One/OneOrNull` = `query.asFlow().mapToList/One/OneOrNull(queryDispatcher)`. The coroutine extension: emits once immediately, registers a `Query.Listener`, and emits again each time any table the query depends on is notified; the signalling channel is CONFLATED (bursts collapse into one re-run); `mapToOne` throws `NullPointerException` when the query returns no row.
5. Notification source: the generated Kotlin of each `.sq` statement calls `notifyQueries` for the tables NAMED in that statement's text, after the outermost transaction commits (or right after the statement outside a transaction). Rows changed only by triggers or foreign-key cascades are NOT announced (for example `chapters.update` announces `chapters`, although the trigger also changed `mangas`).
6. `subscribeToPagingSource` / `QueryPagingMangaSource` has no caller for the manga database (verified by grep); Paging 3 is Android-only.
7. No debounce anywhere in the handler; screens add their own `distinctUntilChanged`, `collectLatest`, `flowOn(Dispatchers.IO)` (example: `app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt:73-80`).

## 2. Proposal: open contract implemented by `kuro_db::open`

`open(paths: &dyn Paths, bundle: &SchemaBundle) -> Result<Db>` (the `Paths` trait is from `plans/addenda/requirements-cross-platform.md`, rule 4; the file is `<data dir>/manga.db`, the anime database is `<data dir>/anime.db`, owned by area:04-db-anime-schema-migrations).

Verified connection setup with `rusqlite 0.40.2` (feature set `bundled`, `hooks`, `functions`, `backup`):

```rust
use rusqlite::{Connection, OpenFlags};
use std::{path::Path, time::Duration};

pub fn open_connection(path: &Path) -> rusqlite::Result<Connection> {
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    c.busy_timeout(Duration::from_millis(5000))?;                 // new vs Aniyomi: no busy timeout there
    c.pragma_update(None, "foreign_keys", "ON")?;                  // verified: also the bundled default (SQLITE_DEFAULT_FOREIGN_KEYS=1)
    let _mode: String = c.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?; // returns "wal"; plain pragma_update errors because the pragma returns a row
    c.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(c)
}
```

Rules:
1. `foreign_keys = ON` for every connection, set again after migrations (migrations themselves run with it OFF only in the legacy importer; Kurobayomi's own future migrations run with it ON and must be written accordingly).
2. `recursive_triggers` stays at its default 0 (see `p1-topic-triggers-and-versioning.md`).
3. Version gate right after open: `PRAGMA user_version`: `0` and no tables -> create baseline (`T-03a-003`); `33` -> ready; `34..` handled by the migration runner (`T-03a-004`); greater than the app's `SCHEMA_VERSION` -> refuse to open and show an error dialog "This library was created by a newer Kurobayomi" (never downgrade silently; Android would throw `Can't downgrade database`). `user_version` `1..32` is an Aniyomi legacy file and is routed to the importer, never opened for writing by the app.
4. Reader/writer split: one writer connection (all writes, all transactions) behind `std::sync::Mutex` executed on a dedicated blocking thread (or `tokio::task::spawn_blocking`), N=4 reader connections for `SELECT` (WAL allows concurrent readers). Reason: SQLite has one writer; Aniyomi serialises through one transaction thread too.
5. A "transaction" in Rust is a closure `FnOnce(&rusqlite::Transaction) -> Result<T>` run on the writer; nesting is expressed by passing the same `&Transaction` down (repositories take `&Connection`-like `&Tx` arguments), replacing the coroutine-context trick of `TransactionElement`. Panics and `Err` roll back (drop of `Transaction`), the equivalent of "rolled back when an exception is thrown or the coroutine is cancelled".

## 3. Proposal: reactive queries

### 3.1 Why hooks and not per-statement table names

SQLDelight announces only the tables named in the statement text; rusqlite offers `Connection::update_hook` (fires for every row insert/update/delete on that connection, including rows changed by triggers and by foreign-key cascades; VERIFIED below) plus `commit_hook`/`rollback_hook`. Using hooks gives a superset of SQLDelight's notifications, which can only cause an extra re-query, never a missed refresh.

Verified output (rusqlite 0.40.2, baseline schema): an `UPDATE chapters SET read = 1` produced hook events `UPDATE chapters` x4 and `UPDATE mangas` x2 (T3, T4, T5, T6); `DELETE FROM mangas WHERE _id = 1` produced `DELETE mangas`, `DELETE chapters`, `DELETE history` (cascade).

### 3.2 Reference implementation (compiled and run)

```rust
use rusqlite::{Connection, hooks::Action};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Tables(pub u16);
impl Tables {
    pub const MANGAS: Tables = Tables(1);
    pub const CHAPTERS: Tables = Tables(2);
    pub const HISTORY: Tables = Tables(4);
    pub const CATEGORIES: Tables = Tables(8);
    pub const MANGAS_CATEGORIES: Tables = Tables(16);
    pub const EXCLUDED_SCANLATORS: Tables = Tables(32);
    pub const MANGA_SYNC: Tables = Tables(64);
    pub const SOURCES: Tables = Tables(128);
    pub const EXTENSION_REPOS: Tables = Tables(256);
    pub fn from_name(n: &str) -> Tables {
        match n {
            "mangas" => Self::MANGAS,
            "chapters" => Self::CHAPTERS,
            "history" => Self::HISTORY,
            "categories" => Self::CATEGORIES,
            "mangas_categories" => Self::MANGAS_CATEGORIES,
            "excluded_scanlators" => Self::EXCLUDED_SCANLATORS,
            "manga_sync" => Self::MANGA_SYNC,
            "sources" => Self::SOURCES,
            "extension_repos" => Self::EXTENSION_REPOS,
            _ => Tables(0),
        }
    }
}

struct Notifier { pending: u16, published: Vec<u16> }   // published = a tokio::sync::watch / broadcast sender in the real code

fn install_hooks(c: &Connection, n: Arc<Mutex<Notifier>>) -> rusqlite::Result<()> {
    let (a, b, r) = (n.clone(), n.clone(), n.clone());
    c.update_hook(Some(move |_a: Action, _db: &str, t: &str, _rowid: i64| {
        a.lock().unwrap().pending |= Tables::from_name(t).0;      // accumulate only; no SQLite calls inside hooks
    }))?;
    c.commit_hook(Some(move || {
        let mut g = b.lock().unwrap();
        let p = std::mem::take(&mut g.pending);
        if p != 0 { g.published.push(p); }                         // publish after commit decision
        false                                                      // MUST return false: true would turn the commit into a rollback
    }))?;
    c.rollback_hook(Some(move || { r.lock().unwrap().pending = 0; }))?;
    Ok(())
}
```

Observed results of running it: a single auto-commit `INSERT INTO mangas` published `[1]`; a transaction with `INSERT chapters` + `UPDATE chapters` that was rolled back published nothing; the same transaction committed published `[3]` = mangas | chapters (the `mangas` bit comes from trigger T6, proving trigger-driven changes are seen).

Hooks are installed on the WRITER connection only (readers never write). The `commit_hook` closure runs while SQLite holds the commit; it must only flip atomics/send on a channel. The real publisher is `tokio::sync::watch::Sender<u64>` per table (a monotonically increasing generation number): `watch` is naturally conflating, matching SQLDelight's `Channel.CONFLATED`.

### 3.3 Subscription API

```rust
pub struct Subscription<T> { /* stream of T */ }
impl Db {
    /// Runs `query` on a reader now, then again after every commit that touched any table in `deps`.
    pub fn subscribe<T: Send + 'static>(
        &self,
        deps: Tables,
        query: impl Fn(&Connection) -> rusqlite::Result<T> + Send + Sync + 'static,
    ) -> impl futures_core::Stream<Item = rusqlite::Result<T>>;
}
```

Semantics copied from SQLDelight: first emission immediately; conflated re-runs; the one-row variant yields `Option<T>` (instead of throwing like `mapToOne`) and the stream stays alive when the row disappears. Delivery to GTK: a `glib::MainContext::spawn_local` task awaits the stream and updates widgets on the main thread (`area:01-foundation-app-bootstrap` defines the bridge).

### 3.4 Dependency sets (what `deps` each subscribed query passes)

| Kotlin flow (file) | Rust subscription | `deps` (tables) |
|---|---|---|
| `MangaRepositoryImpl.getMangaByIdAsFlow`, `getMangaByUrlAndSourceIdAsFlow`, `getMangaFavoritesBySourceId`, `getUpcomingManga` | by id / by url+source / favorites of source / upcoming | mangas |
| `getLibraryMangaAsFlow` | library | mangas, chapters, excluded_scanlators, history, mangas_categories |
| `MangaCategoryRepositoryImpl.getAllMangaCategoriesAsFlow`, `getAllVisibleMangaCategoriesAsFlow` | categories | categories |
| `getCategoriesByMangaIdAsFlow`, `getVisibleCategoriesByMangaIdAsFlow` | categories of a manga | categories, mangas_categories |
| `ChapterRepositoryImpl.getChapterByMangaIdAsFlow` | chapters of a manga | chapters, excluded_scanlators |
| `getScanlatorsByMangaIdAsFlow` | scanlator names | chapters |
| `MangaHistoryRepositoryImpl.getMangaHistory` | history screen | mangas, chapters, history |
| `GetExcludedScanlators.subscribe` | excluded scanlators | excluded_scanlators |
| `MangaTrackRepositoryImpl.getMangaTracksAsFlow`, `getTracksByMangaIdAsFlow` | trackers | manga_sync |
| `MangaUpdatesRepositoryImpl.subscribeAllMangaUpdates`, `subscribeWithRead` | updates | mangas, chapters |
| `MangaExtensionRepoRepositoryImpl.subscribeAll`, `getCount` | repos | extension_repos |
| `MangaStubSourceRepositoryImpl.subscribeAllManga` | stub sources | sources |
| `MangaSourceRepositoryImpl.getMangaSourcesWithFavoriteCount`, `getMangaSourcesWithNonLibraryManga` | per-source counts | mangas |

## 4. Verified helper behaviours used by the design

| Fact | Evidence |
|---|---|
| Named parameter used twice binds once | `INSERT INTO h(chapter_id,last_read,time_read) VALUES (:chapterId,:readAt,:time_read) ON CONFLICT(chapter_id) DO UPDATE SET last_read=:readAt, time_read=time_read+:time_read WHERE chapter_id=:chapterId` executed twice with `:time_read = 7` gave `time_read = 14` |
| JSON1 in-list | `SELECT count(*) FROM h WHERE chapter_id IN (SELECT value FROM json_each(:ids))` with `":ids": "[1,2,3]"` works |
| `PRAGMA user_version` round trip | write 33, read 33 |
| `journal_mode` pragma returns a row | `pragma_update_and_check` returned `"wal"` |
| `foreign_keys` default | `1` under `libsqlite3-sys` bundled; set explicitly anyway |

## 5. Pitfalls

1. `commit_hook` returning `true` rolls the transaction back: return `false`.
2. Hooks are per connection; opening a second writer connection (for example the legacy importer) must not install the app's notifier unless the data shown must refresh.
3. WAL files (`manga.db-wal`, `manga.db-shm`) must be copied together with the main file when backing up the raw database; use `rusqlite::backup::Backup` or `VACUUM INTO` instead of copying files while the app runs.
4. SQLite in WAL mode allows readers during a write; but a reader started before a commit does not see it. Subscriptions therefore always re-run on a NEW read transaction after the notification (the reader connection is not kept inside an open transaction between runs).
5. Do not run SQLite calls from inside `update_hook` / `commit_hook` closures (SQLite forbids it).
