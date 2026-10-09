# Area 04 — Anime database — Rust design

Targets: stable Rust (edition 2024), `rusqlite` (bundled SQLite), `tokio`, `serde_json`, gtk4-rs 0.11 for the UI crate only. Linux first; no OS-specific code in this area (file locations come from `kuro-platform::Paths`, see `plans/addenda/requirements-cross-platform.md`). Paths in code are the Proposal; paths in "Source" are repo-relative to the Aniyomi clone.

## 1. Crate and module layout

```text
kurobayomi/                     (workspace root, existing package `kurobayomi` = GTK binary in src/)
  crates/
    kuro-db/                    NEW, no GTK dependency
      Cargo.toml
      sql/
        anime/
          schema.sql            tables, indexes, triggers, seeds   (T-04a-003)
          views.sql             eight views                        (T-04a-004)
          queries/*.sql         one file per repository, named statements as `-- name: foo` blocks
          legacy/113.sql .. 138.sql                                 (T-04a-025)
          native/139.sql        Kurobayomi-only step 139 -> 140 (indexes, T-04a-036)
        manga/...               owned by area:03-db-manga-schema-migrations
      src/
        lib.rs                  re-exports
        core/                   SHARED with area:03-db-manga-schema-migrations: conn.rs, pool.rs, bus.rs, error.rs, migrate.rs, sqlfn.rs
        anime/
          mod.rs                `AnimeDb` facade (async) + `consts`
          codec.rs              Millis, StringList, UpdateStrategy, FetchType, Memo
          flags.rs              episode/season/viewer/category flag constants + helpers
          model.rs              AnimeRow, EpisodeRow, LibraryAnimeRow, SeasonAnimeRow, HistoryRow, ...
          sql/                  synchronous functions over &Connection / &Transaction
            entries.rs episodes.rs history.rs categories.rs tracks.rs updates.rs
            sources.rs relations.rs custom_buttons.rs extension_store.rs maintenance.rs
          legacy.rs             migration runner for copied Aniyomi files
    kuro-testkit/               NEW (also used by other areas): TestDb, builders
  src/                          existing GTK crate; consumes `kuro_db::anime::AnimeDb`
```

Why a separate crate: the database must be testable without GTK (CI runs `cargo test -p kuro-db` headless) and the manga database (area:03-db-manga-schema-migrations) shares everything under `core/`. `kuro-core` (domain logic of area:07-domain-entries-items and area:08-domain-library-category-history-updates) depends on `kuro-db` row types only through the facade, so the domain crate does not see `rusqlite`.

## 2. Dependencies (and the Android API they replace)

| Crate | Why | Replaces |
|---|---|---|
| `rusqlite` (features `bundled`, `hooks`, `functions`, `backup`) | SQLite binding, `update_hook`/`commit_hook`/`rollback_hook`, custom scalar function `kuro_lower`, online backup API for T-04a-031 | `AndroidSqliteDriver`, SQLDelight runtime, `RequerySQLiteOpenHelperFactory` |
| `r2d2` + `r2d2_sqlite` | read-connection pool (4) | SQLDelight's single driver with IO dispatcher |
| `tokio` (`rt-multi-thread`, `sync`, `time`) | `spawn_blocking`, `broadcast`, 50 ms coalescing timer | Kotlin coroutines `Dispatchers.IO`, `Flow` |
| `tokio-stream` | `Stream` adapters for subscriptions | `kotlinx.coroutines.flow` |
| `serde_json` (feature `preserve_order`) | `memo` object | `kotlinx.serialization.json.JsonObject` |
| `thiserror` | `DbError` | Kotlin exceptions |
| `bitflags` | `TableSet` mask of the ten tables | SQLDelight's per-table listener registry |
| `async-channel` | thread-safe hand-off from tokio tasks to the GLib main context | `withContext(Dispatchers.Main)` |
| `tracing` | logging at the places where Kotlin calls `logcat(LogPriority.ERROR, e)` | `logcat` |
| `chrono` | local start-of-day for the upcoming query (area:01-foundation-app-bootstrap may choose `time` instead; the facade takes epoch milliseconds so the choice does not leak) | `java.time.LocalDate/ZoneId` |

No ORM and no query builder: the SQL is the contract and is stored verbatim in `sql/anime/queries/*.sql`.

## 3. Core types (`kuro-db::core`, shared)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DbKind { Anime, Manga }

impl DbKind {
    pub const fn file_name(self) -> &'static str { match self { Self::Anime => "anime.db", Self::Manga => "manga.db" } }
    pub const fn schema_version(self) -> i32 { match self { Self::Anime => 139, Self::Manga => 33 } }
}

/// One database: a writer connection, a reader pool and a change bus.
#[derive(Clone)]
pub struct Db { inner: Arc<DbInner> }

struct DbInner {
    kind: DbKind,
    writer: Mutex<Connection>,                       // std::sync::Mutex; never held across .await
    readers: r2d2::Pool<SqliteConnectionManager>,    // 4 connections, query_only = ON
    bus: broadcast::Sender<TableSet>,                // capacity 256
}

impl Db {
    pub fn open(kind: DbKind, paths: &dyn Paths) -> Result<Db, DbError>;     // T-04a-001/002
    pub fn open_in_memory(kind: DbKind) -> Result<Db, DbError>;              // tests only (shared-cache URI)
    /// Run `f` on a pooled read connection (spawn_blocking).
    pub async fn read<T: Send + 'static>(&self, f: impl FnOnce(&Connection) -> Result<T, DbError> + Send + 'static) -> Result<T, DbError>;
    /// Run `f` inside BEGIN IMMEDIATE .. COMMIT on the single writer. Err or panic rolls back.
    pub async fn write<T: Send + 'static>(&self, f: impl FnOnce(&Transaction) -> Result<T, DbError> + Send + 'static) -> Result<T, DbError>;
    /// Emit the current value, then a new value after each commit touching `tables` (50 ms coalescing, distinct-until-changed).
    pub fn subscribe<T: PartialEq + Send + 'static>(&self, tables: TableSet, f: impl Fn(&Connection) -> Result<T, DbError> + Send + Sync + 'static) -> impl Stream<Item = Result<T, DbError>> + Send + 'static;
}

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("sqlite: {0}")] Sqlite(#[from] rusqlite::Error),
    #[error("database is newer than this app (found {found}, supported {supported})")] NewerThanApp { found: i32, supported: i32 },
    #[error("database version {found} needs the legacy import")] NeedsLegacyImport { found: i32 },
    #[error("database version {found} is older than the oldest importable version 113")] TooOldForImport { found: i32 },
    #[error("system category can't be deleted")] SystemCategory,
    #[error("bad memo in {table} row {id}")] BadMemo { table: &'static str, id: i64 },
    #[error("database busy")] Busy,
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("task cancelled")] Cancelled,
}
```

Connection set-up (`core/conn.rs`), applied to every connection in this order: `PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;`, then `create_scalar_function("kuro_lower", 1, DETERMINISTIC | UTF8, |ctx| ctx.get::<Option<String>>(0).map(|s| s.map(|s| s.to_lowercase())))`. Readers add `PRAGMA query_only = ON`. `recursive_triggers` is never touched. rusqlite's bundled build already defaults foreign keys on; the explicit pragma stays so a system-SQLite build behaves the same.

Mapping of `AndroidAnimeDatabaseHandler` to this API: `await(inTransaction = true)` is `write`; `await()` read-only is `read`; `await()` that writes one statement is also `write` (a single-statement transaction, same atomicity as autocommit); `subscribeTo*` is `subscribe`.

## 4. Change bus (`core/bus.rs`)

```rust
bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct TableSet: u32 {
        const ANIMES = 1<<0; const EPISODES = 1<<1; const ANIMEHISTORY = 1<<2; const ANIMES_CATEGORIES = 1<<3;
        const CATEGORIES = 1<<4; const ANIME_SYNC = 1<<5; const ANIMESOURCES = 1<<6; const ANIME_RELATIONS = 1<<7;
        const CUSTOM_BUTTONS = 1<<8; const EXTENSION_STORE = 1<<9;
    }
}
fn table_bit(name: &str) -> TableSet { /* match on the ten table names; unknown (manga DB) -> a per-kind table */ }
```

Hook wiring on the writer (`hooks` feature): `conn.update_hook(Some(move |_, _, table, _| pending.lock().insert(table_bit(table))))`; `conn.commit_hook(Some(move || { let s = take(&mut *pending.lock()); if !s.is_empty() { let _ = bus.send(s); } false }))` (returning `false` lets the commit proceed); `conn.rollback_hook(Some(move || pending.lock().clear()))`. Pitfalls: (1) hooks run on the SQLite thread inside statement execution and must not touch the connection; (2) SQLite does not invoke the update hook for deletes that use the truncate optimisation (a `DELETE` without `WHERE` on a table with no triggers and, with foreign keys on, no foreign-key relation); none of this area's statements are expected to qualify (SQLite excludes tables that take part in foreign keys when foreign keys are on, and `DELETE FROM animehistory` is the only statement of this area without a `WHERE` on a table without triggers), and a test asserts that `delete_all_history` notifies; `Transaction` also exposes `tx.touch(TableSet)` as an explicit fallback that adds to `pending`; (3) the manga and anime DBs have separate buses.

## 5. Codecs (`anime/codec.rs`, T-04a-005)

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Millis(pub i64);                       // Date <-> INTEGER ms

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct StringList(pub Vec<String>);           // ", " separated; "" => empty list
impl StringList {
    pub const SEPARATOR: &'static str = ", ";
    pub fn encode(&self) -> String { self.0.join(Self::SEPARATOR) }
    pub fn decode(s: &str) -> Self { if s.is_empty() { Self(vec![]) } else { Self(s.split(Self::SEPARATOR).map(str::to_owned).collect()) } }
}
// nullable column => Option<StringList>

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)] #[repr(i64)]
pub enum UpdateStrategy { #[default] AlwaysUpdate = 0, OnlyFetchOnce = 1 }
impl UpdateStrategy { pub fn from_db(v: i64) -> Self { match v { 1 => Self::OnlyFetchOnce, _ => Self::AlwaysUpdate } } }

#[derive(Clone, Copy, PartialEq, Eq, Debug)] #[repr(i64)]
pub enum FetchType { Seasons = 0, Episodes = 1 }
impl FetchType { pub fn from_db(v: i64) -> Self { if v == 0 { Self::Seasons } else { Self::Episodes } } }   // unknown => Episodes

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Memo(pub serde_json::Map<String, serde_json::Value>);
impl FromSql for Memo { /* accepts ValueRef::Text and ValueRef::Blob; invalid or non-object => empty map + tracing::warn */ }
impl ToSql for Memo { /* ValueRef::Blob of compact UTF-8 JSON, "{}" for empty */ }
```

Booleans use rusqlite's built-in `bool` support (non-zero is true). Epoch values and ids are plain `i64`.

## 6. Row models (`anime/model.rs`, T-04a-009 and following)

```rust
#[derive(Clone, PartialEq, Debug)]
pub struct AnimeRow {
    pub id: i64, pub source: i64, pub url: String,
    pub artist: Option<String>, pub author: Option<String>, pub description: Option<String>,
    pub genre: Option<StringList>, pub title: String, pub status: i64, pub thumbnail_url: Option<String>,
    pub favorite: bool, pub last_update: Option<i64>, pub next_update: Option<i64>, pub initialized: bool,
    pub viewer: i64, pub episode_flags: i64, pub cover_last_modified: i64, pub date_added: i64,
    pub update_strategy: UpdateStrategy, pub calculate_interval: i64,
    pub last_modified_at: i64, pub favorite_modified_at: Option<i64>, pub version: i64, pub is_syncing: i64,
    pub fetch_type: FetchType, pub parent_id: Option<i64>, pub season_flags: i64, pub season_number: f64,
    pub season_source_order: i64, pub background_url: Option<String>, pub background_last_modified: i64,
    pub memo: Memo,
}
impl AnimeRow { pub fn from_row(r: &Row) -> rusqlite::Result<Self> { /* r.get("title")? by NAME for all 32 columns */ } }

pub struct LibraryAnimeRow { pub anime: AnimeRow, pub category: i64, pub total_count: i64, pub seen_count: i64,
    pub latest_upload: i64, pub episode_fetched_at: i64, pub last_seen: i64, pub bookmark_count: i64, pub fillermark_count: i64 }
pub struct SeasonAnimeRow  { pub anime: AnimeRow, pub total_count: i64, pub seen_count: i64, pub latest_upload: i64,
    pub fetched_at: i64, pub last_seen: i64, pub bookmark_count: i64, pub fillermark_count: i64 }
pub struct EpisodeRow { pub id: i64, pub anime_id: i64, pub url: String, pub name: String, pub scanlator: Option<String>,
    pub seen: bool, pub bookmark: bool, pub last_second_seen: i64, pub total_seconds: i64, pub episode_number: f64,
    pub source_order: i64, pub date_fetch: i64, pub date_upload: i64, pub last_modified_at: i64, pub version: i64,
    pub is_syncing: i64, pub summary: Option<String>, pub preview_url: Option<String>, pub fillermark: bool, pub memo: Memo }
pub struct HistoryRow { pub id: i64, pub episode_id: i64, pub seen_at: Option<i64> }
pub struct HistoryEntry { pub id: i64, pub anime_id: i64, pub episode_id: i64, pub title: String, pub thumbnail_url: Option<String>,
    pub source: i64, pub favorite: bool, pub cover_last_modified: i64, pub episode_number: f64, pub seen_at: i64 }
pub struct UpdateEntry { pub anime_id: i64, pub anime_title: String, pub episode_id: i64, pub episode_name: String, pub scanlator: Option<String>,
    pub seen: bool, pub bookmark: bool, pub fillermark: bool, pub last_second_seen: i64, pub total_seconds: i64,
    pub source: i64, pub favorite: bool, pub thumbnail_url: Option<String>, pub cover_last_modified: i64, pub date_upload: i64, pub date_fetch: i64 }
pub struct CategoryRow { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
pub struct TrackRow { pub id: i64, pub anime_id: i64, pub tracker_id: i64, pub remote_id: i64, pub library_id: Option<i64>, pub title: String,
    pub last_episode_seen: f64, pub total_episodes: i64, pub status: i64, pub score: f64, pub remote_url: String,
    pub start_date: i64, pub finish_date: i64, pub private: bool }
pub struct CustomButtonRow { pub id: i64, pub name: String, pub is_favorite: bool, pub sort_index: i64,
    pub content: String, pub long_press_content: String, pub on_startup: String }
pub struct ExtensionStoreRow { pub index_url: String, pub name: String, pub badge_label: String, pub signing_key: String,
    pub contact_website: String, pub contact_discord: Option<String>, pub is_legacy: bool, pub extension_list_url: Option<String> }
```

Partial update types mirror `AnimeUpdate` (`domain/.../entries/anime/model/AnimeUpdate.kt`): every field `Option<_>`; `None` binds NULL so `coalesce` keeps the column.

```rust
#[derive(Default)]
pub struct AnimeUpdate { pub id: i64, pub source: Option<i64>, pub favorite: Option<bool>, pub last_update: Option<i64>,
    pub next_update: Option<i64>, pub fetch_interval: Option<i64>, pub date_added: Option<i64>, pub viewer_flags: Option<i64>,
    pub episode_flags: Option<i64>, pub cover_last_modified: Option<i64>, pub background_last_modified: Option<i64>,
    pub url: Option<String>, pub title: Option<String>, pub artist: Option<String>, pub author: Option<String>,
    pub description: Option<String>, pub genre: Option<StringList>, pub status: Option<i64>, pub thumbnail_url: Option<String>,
    pub background_url: Option<String>, pub update_strategy: Option<UpdateStrategy>, pub initialized: Option<bool>,
    pub version: Option<i64>, pub fetch_type: Option<FetchType>, pub parent_id: Option<i64>, pub season_flags: Option<i64>,
    pub season_number: Option<f64>, pub season_source_order: Option<i64>, pub memo: Option<Memo> }
```

(`is_syncing` is not in the struct; the repository always binds `0`, except `restore::update_anime_syncing` which binds `1`.)

## 7. SQL functions (synchronous, one module per table family)

Pure functions over `&Connection`/`&Transaction` so they are unit-testable on `TestDb` without tokio. Example (`anime/sql/entries.rs`):

```rust
const UPDATE_SQL: &str = include_str!("../../../sql/anime/queries/animes_update.sql"); // the coalesce statement, named params

pub fn update_one(tx: &Transaction, u: &AnimeUpdate) -> rusqlite::Result<usize> {
    tx.prepare_cached(UPDATE_SQL)?.execute(named_params! {
        ":source": u.source, ":url": u.url, /* ... 29 parameters ... */
        ":genre": u.genre.as_ref().map(StringList::encode),
        ":calculateInterval": u.fetch_interval,
        ":updateStrategy": u.update_strategy.map(|s| s as i64),
        ":fetchType": u.fetch_type.map(|f| f as i64),
        ":memo": u.memo.as_ref(),
        ":isSyncing": 0_i64,                     // ALWAYS 0 (AnimeRepositoryImpl.partialUpdateAnime)
        ":animeId": u.id,
    })
}
pub fn insert(tx: &Transaction, a: &AnimeRow) -> rusqlite::Result<i64> { /* 28 params, then Ok(tx.last_insert_rowid()) */ }
pub fn get_by_id(c: &Connection, id: i64) -> rusqlite::Result<Option<AnimeRow>>;
pub fn get_by_url_and_source(c: &Connection, url: &str, source: i64) -> rusqlite::Result<Option<AnimeRow>>;   // LIMIT 1
pub fn favorites(c: &Connection) -> rusqlite::Result<Vec<AnimeRow>>;
pub fn favorites_by_source(c: &Connection, source: i64) -> rusqlite::Result<Vec<AnimeRow>>;
pub fn duplicates(c: &Connection, id: i64, title_lower: &str) -> rusqlite::Result<Vec<AnimeRow>>;             // kuro_lower(title) = :title
pub fn upcoming(c: &Connection, statuses: &[i64], start_of_day_ms: i64) -> rusqlite::Result<Vec<AnimeRow>>;
pub fn remove_parent_ids(tx: &Transaction, ids: &[i64]) -> rusqlite::Result<()>;                            // chunks of 500
pub fn delete_not_in_library(tx: &Transaction, ids: &[i64]) -> rusqlite::Result<()>;                        // chunks of 500
```

`IN` helper: `fn in_chunks(ids: &[i64]) -> impl Iterator<Item = (String, &[i64])>` (SQL text `"?,?,?"` for the chunk length; empty slice yields nothing).

Library read model:

```rust
pub fn library(c: &Connection) -> rusqlite::Result<Vec<LibraryAnimeRow>> {
    let mut st = c.prepare_cached("SELECT * FROM animelibView")?;
    st.query_map([], |r| Ok(LibraryAnimeRow {
        anime: AnimeRow::from_row(r)?, category: r.get("category")?,
        total_count: r.get("totalCount")?,
        seen_count: r.get::<_, f64>("seenCount")? as i64,            // REAL in the typed layer, truncate
        bookmark_count: r.get::<_, f64>("bookmarkCount")? as i64,
        fillermark_count: r.get::<_, f64>("fillermarkCount")? as i64,
        latest_upload: r.get("latestUpload")?, episode_fetched_at: r.get("episodeFetchedAt")?, last_seen: r.get("lastSeen")?,
    }))?.collect()
}
```

(Reading an INTEGER SQLite value as `f64` works in rusqlite; reading by name avoids the column-shift problem when `animes` gains a column.)

History and updates: `history(c, query: &str)` binds `query` after escaping (`%`, `_`, `\` prefixed by `\`, `ESCAPE '\'`) and wraps it as `'%' || :q || '%'` with `kuro_lower` on both sides; `recent_updates(c, after_ms, limit)` and `updates_by_seen(c, seen, after_ms, limit)` add `ORDER BY datefetch DESC`.

Category writes that need ordering rules (reorder, delete re-index) are in `anime/sql/categories.rs` as `reorder(tx, id, new_index)`, `delete_and_reindex(tx, id)` implementing `p1-behavior.md` section 7.2; the preference clean-up (five sets, default category) is done by the caller in `kuro-core` after the transaction commits, because preferences are not in this database.

## 8. Schema creation and upgrades (`core/migrate.rs`, `anime/legacy.rs`)

```rust
pub fn prepare(conn: &mut Connection, kind: DbKind) -> Result<(), DbError> {
    let v: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let has_tables: bool = conn.query_row("SELECT count(*) FROM sqlite_master WHERE type='table'", [], |r| r.get::<_, i64>(0))? > 0;
    match (v, has_tables) {
        (0, false) => create_fresh(conn, kind),                      // one transaction: schema.sql, views.sql, user_version = 139
        (v, _) if v == kind.schema_version() => Ok(()),
        (v, _) if v > kind.schema_version() => Err(DbError::NewerThanApp { found: v, supported: kind.schema_version() }),
        (v, _) if (113..139).contains(&v) => legacy::upgrade(conn, v),          // T-04a-025; Err(NeedsLegacyImport) until implemented
        (v, _) => Err(DbError::TooOldForImport { found: v }),
    }
}
```

`create_fresh`: `BEGIN; execute_batch(include_str!("schema.sql")); execute_batch(include_str!("views.sql")); PRAGMA user_version = 139; COMMIT`. After `prepare` every open runs the two safety statements of T-04a-024. Kurobayomi-native steps use `native/<N>.sql` with the same runner (step 139 adds the two indexes of T-04a-036 and sets `user_version = 140`, then `schema_version()` becomes 140 and the fresh schema file is regenerated to include them).

Legacy upgrade runner: see `p1-topic-legacy-migration-chain.md` section 4; it needs `PRAGMA foreign_keys = OFF` outside any transaction, therefore it runs on a dedicated connection opened before the pool exists.

## 9. Async and threading model

* `Db::read` and `Db::write` hop to `tokio::task::spawn_blocking`; the writer is a `std::sync::Mutex<Connection>` locked only inside the blocking closure (never across `.await`), so writers serialise exactly like SQLDelight's one-transaction-at-a-time rule.
* GTK main loop: widgets are touched only on the GLib main context. `ui::db_bridge::spawn_stream(stream, f)` creates an `async_channel::unbounded`, forwards stream items from a tokio task, and `glib::MainContext::default().spawn_local(async move { while let Ok(v) = rx.recv().await { f(v) } })` applies them; the returned `StreamHandle` aborts the tokio task on drop, so leaving a screen cancels its subscription (replaces Compose `collectAsState` lifecycle).
* The app owns one multi-thread tokio runtime created in `main` (area:01-foundation-app-bootstrap); `AnimeDb` holds a `tokio::runtime::Handle`.
* Pagination replacement (T-04a-033): none needed today; if a list view needs it, use keyset paging (`WHERE (sort_col, _id) > (:last_sort, :last_id) ORDER BY sort_col, _id LIMIT :n`) on the view columns, not `LIMIT/OFFSET`.

## 10. Error handling

* SQL functions return `rusqlite::Result`; the facade maps to `DbError` (`SQLITE_BUSY` to `Busy`; the trigger message `System category can't be deleted` to `SystemCategory`; all else `Sqlite`).
* Where Kotlin swallows errors (`updateAnime`, `updateAllAnime`, `addAllEpisodes`, history resets, `removeEpisodesWithIds`, `removeParentIdByIds`) the facade methods keep the same shape: log with `tracing::error!` and return `false` / empty / `()`; all other methods return `Result`.
* A panic inside a write closure rolls the transaction back (`Transaction` drop) and surfaces as `DbError::Cancelled`.
* User-visible text comes from the UI layer: generic failures use `MR.strings.internal_error` ("InternalError: Check crash logs for further information").

## 11. Plugging into the existing mockup

The mockup (`src/*.rs`, `spec/*.json`) has no persistence; its list screens build rows from literals. Replacement map (only the data source changes, the widgets stay):

| Mock function | Where | Becomes |
|---|---|---|
| `library(id, anime)` | `src/screens.rs:154` | anime branch subscribes to `AnimeDb::library_stream()` (T-04a-010); sort/filter/grouping logic is area:37-ui-library |
| `history()` | `src/screens.rs:305` | anime branch of `kind_tabs` subscribes to `AnimeDb::history_stream(query)` (T-04a-012); the day headers are built in the UI crate from `seen_at` |
| `updates()` | `src/screens.rs:277` | `AnimeDb::updates_stream(after = now - 3 months, limit = 500)` (T-04a-015) |
| `categories()` | `src/screens.rs:404` | `AnimeDb::categories_stream()` for the anime tab; the hard-coded names `Reading`, `Plan to read`, `Finished` are mock data and are deleted; the dialogs (`category_create`, `category_rename`, `category_delete`) call the category functions of T-04a-013 |
| `clear_db(id)` | `src/settings.rs:223` | for `settings_clear_anime_database` list `AnimeDb::deletable_parents_stream()` grouped by source and call `clear_non_library` (T-04a-021); the four fake "Source n" rows go away |
| player custom buttons screen (`settings_player_custom_buttons`, spec `spec/settings.json`) | built by `settings.rs` generic builder | `AnimeDb::custom_buttons_stream()` and the T-04a-018 functions |
| `entry(title, anime)` | `src/entry.rs:81` | anime branch reads `get_by_id` + `episodes_by_anime_stream` + `seasons_stream` + `related_stream` through `AnimeDb` (area:39-ui-entry-details-anime) |

`ctx.anime_db` is created in `main()` before `build_ui` (T-04a-028). The spec files stay UI text only; no database content is stored in `spec/*.json`.

## 12. Android-API to Linux mapping for this area

| Android / Kotlin | Linux / Rust |
|---|---|
| `AndroidSqliteDriver`, `SupportSQLiteOpenHelper`, `RequerySQLiteOpenHelperFactory` | `rusqlite` bundled |
| `Database Inspector` (debug) | `sqlite3` CLI / DB Browser on `~/.local/share/kurobayomi/anime.db` |
| `Context.getDatabasePath` | `Paths::data_dir()` |
| SQLDelight codegen, `.sq` compile-time checking | SQL in `.sql` files; compile-time checking replaced by the golden-schema test and one prepared-statement smoke test per statement (`Connection::prepare` of every named statement in `cargo test`) |
| `ColumnAdapter` | codec types of section 5 |
| `Flow` / `asFlow()` / `mapToList` | `Db::subscribe` stream |
| `Dispatchers.IO`, `withContext` | `spawn_blocking` |
| `ThreadLocal<Int>` transaction ids, `AnimeTransactionElement` | none (closure receives `&Transaction`) |
| `logcat` | `tracing` |
| Paging 3 | dropped (T-04a-033) |

## 13. Pitfalls

1. Never build statements with string formatting of user values; only the `IN` helper builds placeholder text.
2. `rusqlite::Connection` is `Send` but not `Sync`; never put it in `Arc` without a Mutex; reader connections are checked out from the pool per call.
3. Hooks fire inside `execute`; do not call `bus.send` while holding the writer Mutex longer than needed (the commit hook runs under it; `broadcast::send` is non-blocking).
4. `PRAGMA foreign_keys` is silently ignored inside a transaction: set it at connection creation, before `BEGIN`.
5. `last_insert_rowid()` is per connection: only valid on the writer and right after the insert, inside the same transaction.
6. `strftime('%s','now')` returns TEXT; the columns have INTEGER affinity so it is stored as an integer, but a raw `SELECT` of the expression is a string.
7. A `REAL` column compared with an integer literal works, but `episode_number` equality in Rust uses `f64` bit equality only for round-trip tests; business comparisons use the domain epsilon rules of area:07-domain-entries-items.
8. `user_version` is a header field, not a table: changing it inside a rolled-back transaction rolls back too (desired).
9. WAL mode needs a writable directory for `-wal`/`-shm`; a read-only data dir surfaces as `DbError::Io` at open and the UI shows a blocking error.
10. On Windows (later) `rename` of a database file that is open fails; T-04a-031 quarantine closes the connection first.
