# Area 03 / part 1 — Rust design: `kuro-db` (manga database schema, statements, migrations)

All "Proposal". Facts that justify each choice are in the companion files and were verified with `sqlite3` 3.53 and `rusqlite 0.40.2` (`libsqlite3-sys 0.38.2`, bundled SQLite 3.53.2) compiled against this machine's cargo registry.

## 1. Where this lives

Workspace layout (the workspace itself is defined by area:01-foundation-app-bootstrap; the platform rules are in `plans/addenda/requirements-cross-platform.md`):

```
crates/
  kuro-db/                 <- THIS AREA (schema, statements, migrations, importer, notification hooks)
    Cargo.toml
    sql/
      schema_v33.sql       <- baseline (p1-topic-baseline-schema.md section 2), include_str!
      migrations/          <- 34.sql and later (Kurobayomi's own)
      legacy/              <- 1.sql .. 32.sql (stripped Aniyomi migrations, importer only)
    src/
      lib.rs               pub use of the modules below
      error.rs             DbError
      open.rs              open_connection, pragmas, version gate
      schema.rs            SCHEMA_VERSION, BASELINE_SQL, SchemaBundle
      migrate.rs           runner for 34.. (user_version driven)
      legacy.rs            Aniyomi tachiyomi.db importer (feature "legacy-import")
      codec.rs             GenreList, UpdateStrategy, ms/secs helpers, Bool
      flags.rs             chapter flags, category sort flags, viewer flags
      models.rs            row structs + patch structs
      notify.rs            Tables bit set, hook installation, publisher
      runtime.rs           Db (writer + readers), Tx, subscribe
      sql/                 one module per .sq file: mangas.rs chapters.rs categories.rs
                           mangas_categories.rs history.rs manga_sync.rs excluded_scanlators.rs
                           extension_repos.rs sources.rs views.rs
  kuro-data/               <- areas 05 and 06 (repositories built on kuro-db)
  kuro-domain/             <- areas 07 and 08 (models, interactors)
  kuro-platform/           <- OS traits (`Paths`, `SecretStore`, `Notifier`, `MediaControls`, `WebViewHost`, `Opener`, `IdleInhibitor`, `SingleInstance`, `SchedulerBackend`, `FileAssociations`, `Autostart`, `ExternalPlayer`, `Biometrics/AppLock`)
```

`kuro-db` is `platform-neutral`: it receives the database path from `kuro-platform::Paths` as an argument and never calls `directories`, `xdg` or `cfg(target_os)` itself (rule 1 and 4 of the cross-platform addendum).

`Cargo.toml` (versions are those present in the local registry; the app's existing `Cargo.toml` has only `gtk4`, `serde`, `serde_json`):

```toml
[package]
name = "kuro-db"
version = "0.1.0"
edition = "2024"

[features]
default = []
legacy-import = []            # embeds sql/legacy/*.sql and the importer (T-03a-022/023)

[dependencies]
rusqlite   = { version = "0.40", features = ["bundled", "hooks", "functions", "backup"] }
thiserror  = "2"
serde_json = "1"              # builds the JSON text bound to json_each(:ids)
tokio      = { version = "1", features = ["rt", "sync", "macros"] }
```

Why `bundled`: the app must not depend on the distro's `libsqlite3` version/compile flags (the Arch `sqlite3` binary was observed with double-quoted string literals disabled, the bundled build with them enabled; JSON1 and the `ON CONFLICT(col) DO UPDATE` upsert clause need SQLite 3.24 or newer; `RETURNING`-free SQL here). Why `hooks`: reactive notifications (`update_hook`, `commit_hook`, `rollback_hook`). Why `functions`: Unicode `lower` (`T-03a-025`). Why `backup`: online backup / pre-migration copy (alternative `VACUUM INTO`). Alternatives rejected: `sqlx` (async, macro-time checking needs a live DB and `DATABASE_URL`, no update-hook story), `diesel` (ORM and codegen overhead for a schema that is already hand-written SQL), `refinery`/`rusqlite_migration` (our migration scheme is "file N upgrades N to N+1" keyed on `user_version`, ten lines of code; adding a crate buys nothing), `sea-query` (the SQL is fixed text).

## 2. Core types

```rust
// error.rs
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("constraint violation: {0}")]
    Constraint(String),              // ExtensionRepo insert/upsert conflicts map here (Kotlin: SaveExtensionRepoException)
    #[error("library was created by a newer version (schema {found}, supported {supported})")]
    TooNew { found: i64, supported: i64 },
    #[error("database must be imported first (schema {found} is an Aniyomi file)")]
    LegacyFile { found: i64 },
    #[error("integrity check failed: {0}")]
    Corrupt(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found")]
    NotFound,                        // replaces SQLDelight executeAsOne() throwing
}
pub type DbResult<T> = Result<T, DbError>;

// schema.rs
pub const SCHEMA_VERSION: i64 = 33;               // grows with each Kurobayomi migration (34 and every later number)
pub const BASELINE_SQL: &str = include_str!("../sql/schema_v33.sql");

pub struct SchemaBundle {
    pub file_name: &'static str,                  // "manga.db"
    pub baseline_sql: &'static str,
    pub baseline_version: i64,                    // 33
    pub migrations: &'static [(i64, &'static str)], // (target version, sql), ascending, contiguous
}
pub const MANGA_SCHEMA: SchemaBundle = SchemaBundle {
    file_name: "manga.db",
    baseline_sql: BASELINE_SQL,
    baseline_version: 33,
    migrations: &[],                              // 34.sql appended by T-03a-024 when needed
};
// area:04-db-anime-schema-migrations defines ANIME_SCHEMA with the same struct (file "anime.db", its own baseline/version).
```

```rust
// codec.rs
/// TEXT column holding genres joined by ", " (DatabaseAdapter.kt StringListColumnAdapter).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GenreList(pub Vec<String>);
impl rusqlite::types::FromSql for GenreList {
    fn column_result(v: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = v.as_str()?;
        Ok(GenreList(if s.is_empty() { Vec::new() } else { s.split(", ").map(str::to_owned).collect() }))
    }
}
impl rusqlite::types::ToSql for GenreList {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.0.join(", ").into())
    }
}
// Option<GenreList> maps NULL <-> None through rusqlite's blanket Option impls.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum UpdateStrategy { AlwaysUpdate = 0, OnlyFetchOnce = 1 }
impl UpdateStrategy {
    pub fn from_db(v: i64) -> Self { if v == 1 { Self::OnlyFetchOnce } else { Self::AlwaysUpdate } } // unknown -> AlwaysUpdate (MangaUpdateStrategyColumnAdapter)
    pub fn to_db(self) -> i64 { self as i64 }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
```

```rust
// flags.rs  (values from p1-data-and-config.md section 4)
pub mod chapter {
    pub const SHOW_ALL: i64 = 0x0;
    pub const SORT_DESC: i64 = 0x0;      pub const SORT_ASC: i64 = 0x1;       pub const SORT_DIR_MASK: i64 = 0x1;
    pub const SHOW_UNREAD: i64 = 0x2;    pub const SHOW_READ: i64 = 0x4;      pub const UNREAD_MASK: i64 = 0x6;
    pub const SHOW_DOWNLOADED: i64 = 0x8; pub const SHOW_NOT_DOWNLOADED: i64 = 0x10; pub const DOWNLOADED_MASK: i64 = 0x18;
    pub const SHOW_BOOKMARKED: i64 = 0x20; pub const SHOW_NOT_BOOKMARKED: i64 = 0x40; pub const BOOKMARKED_MASK: i64 = 0x60;
    pub const SORTING_SOURCE: i64 = 0x000; pub const SORTING_NUMBER: i64 = 0x100;
    pub const SORTING_UPLOAD_DATE: i64 = 0x200; pub const SORTING_ALPHABET: i64 = 0x300; pub const SORTING_MASK: i64 = 0x300;
    pub const DISPLAY_NAME: i64 = 0x0;   pub const DISPLAY_NUMBER: i64 = 0x100000; pub const DISPLAY_MASK: i64 = 0x100000;
}
pub mod library_sort {
    pub const TYPE_MASK: i64 = 0b0011_1100;
    pub const DIR_MASK: i64 = 0b0100_0000;
    pub const ALPHABETICAL: i64 = 0;
    pub const LAST_READ: i64 = 4;
    pub const LAST_UPDATE: i64 = 8;
    pub const UNREAD_COUNT: i64 = 12;
    pub const TOTAL_CHAPTERS: i64 = 16;
    pub const LATEST_CHAPTER: i64 = 20;
    pub const CHAPTER_FETCH_DATE: i64 = 24;
    pub const DATE_ADDED: i64 = 28;
    pub const TRACKER_MEAN: i64 = 32;
    pub const RANDOM: i64 = 60;
    pub const ASCENDING: i64 = 0x40;
    pub const DESCENDING: i64 = 0x00;
}
pub mod viewer {
    pub const READING_MODE_MASK: i64 = 0x7;
    pub const MODE_DEFAULT: i64 = 0;
    pub const MODE_LEFT_TO_RIGHT: i64 = 1;
    pub const MODE_RIGHT_TO_LEFT: i64 = 2;
    pub const MODE_VERTICAL: i64 = 3;
    pub const MODE_WEBTOON: i64 = 4;
    pub const MODE_CONTINUOUS_VERTICAL: i64 = 5;
    pub const ORIENTATION_MASK: i64 = 0x38;
    pub const ORIENTATION_DEFAULT: i64 = 0x00;
    pub const ORIENTATION_FREE: i64 = 0x08;
    pub const ORIENTATION_PORTRAIT: i64 = 0x10;
    pub const ORIENTATION_LANDSCAPE: i64 = 0x18;
    pub const ORIENTATION_LOCKED_PORTRAIT: i64 = 0x20;
    pub const ORIENTATION_LOCKED_LANDSCAPE: i64 = 0x28;
    pub const ORIENTATION_REVERSE_PORTRAIT: i64 = 0x30;
}
/// (value and !mask) or (flag and mask) — FlagWithMask.plus / SetMangaChapterFlags.setFlag
pub const fn set_flag(value: i64, flag: i64, mask: i64) -> i64 { (value & !mask) | (flag & mask) }
```

Only the types and flag helpers appear here because the domain models (`Manga`, `Chapter`, `Category`, `MangaTrack`, `MangaHistory`) belong to area:07-domain-entries-items/08; `kuro-db::models` holds the storage-level rows and patches:

```rust
// models.rs (storage rows; one struct per table/view, field order == column order)
pub struct MangaRow {
    pub id: i64, pub source: i64, pub url: String,
    pub artist: Option<String>, pub author: Option<String>, pub description: Option<String>,
    pub genre: Option<GenreList>, pub title: String, pub status: i64, pub thumbnail_url: Option<String>,
    pub favorite: bool, pub last_update: Option<i64>, pub next_update: Option<i64>, pub initialized: bool,
    pub viewer: i64, pub chapter_flags: i64, pub cover_last_modified: i64, pub date_added: i64,
    pub update_strategy: UpdateStrategy, pub calculate_interval: i64,
    pub last_modified_at: i64, pub favorite_modified_at: Option<i64>, pub version: i64, pub is_syncing: bool,
}
pub struct MangaPatch {                       // coalesce(:x, col) semantics: None = unchanged
    pub id: i64,
    pub source: Option<i64>, pub url: Option<String>, pub artist: Option<String>, pub author: Option<String>,
    pub description: Option<String>, pub genre: Option<GenreList>, pub title: Option<String>, pub status: Option<i64>,
    pub thumbnail_url: Option<String>, pub favorite: Option<bool>, pub last_update: Option<i64>, pub next_update: Option<i64>,
    pub initialized: Option<bool>, pub viewer: Option<i64>, pub chapter_flags: Option<i64>,
    pub cover_last_modified: Option<i64>, pub date_added: Option<i64>, pub update_strategy: Option<UpdateStrategy>,
    pub calculate_interval: Option<i64>, pub version: Option<i64>, pub is_syncing: Option<bool>,
}
pub struct ChapterRow {
    pub id: i64, pub manga_id: i64, pub url: String, pub name: String, pub scanlator: Option<String>,
    pub read: bool, pub bookmark: bool, pub last_page_read: i64, pub chapter_number: f64, pub source_order: i64,
    pub date_fetch: i64, pub date_upload: i64, pub last_modified_at: i64, pub version: i64, pub is_syncing: bool,
}
pub struct ChapterPatch {
    pub id: i64,
    pub manga_id: Option<i64>, pub url: Option<String>, pub name: Option<String>, pub scanlator: Option<String>,
    pub read: Option<bool>, pub bookmark: Option<bool>, pub last_page_read: Option<i64>,
    pub chapter_number: Option<f64>, pub source_order: Option<i64>, pub date_fetch: Option<i64>,
    pub date_upload: Option<i64>, pub version: Option<i64>, pub is_syncing: Option<bool>,
}
pub struct CategoryRow { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
pub struct CategoryPatch { pub id: i64, pub name: Option<String>, pub order: Option<i64>, pub flags: Option<i64>, pub hidden: Option<bool> }
pub struct HistoryRow { pub id: i64, pub chapter_id: i64, pub last_read: i64, pub time_read: i64 }
pub struct TrackRow {
    pub id: i64, pub manga_id: i64, pub sync_id: i64, pub remote_id: i64, pub library_id: Option<i64>,
    pub title: String, pub last_chapter_read: f64, pub total_chapters: i64, pub status: i64, pub score: f64,
    pub remote_url: String, pub start_date: i64, pub finish_date: i64, pub private: bool,
}
pub struct ExtensionRepoRow { pub base_url: String, pub name: String, pub short_name: Option<String>, pub website: String, pub signing_key_fingerprint: String }
pub struct StubSourceRow { pub id: i64, pub lang: String, pub name: String }
pub struct LibraryRow { pub manga: MangaRow, pub total_count: i64, pub read_count: i64, pub latest_upload: i64,
                        pub chapter_fetched_at: i64, pub last_read: i64, pub bookmark_count: i64, pub category: i64 }
pub struct HistoryViewRow { pub id: i64, pub manga_id: i64, pub chapter_id: i64, pub title: String, pub thumbnail_url: Option<String>,
                            pub source: i64, pub favorite: bool, pub cover_last_modified: i64, pub chapter_number: f64,
                            pub read_at: i64, pub read_duration: i64 }
pub struct UpdateViewRow { pub manga_id: i64, pub manga_title: String, pub chapter_id: i64, pub chapter_name: String,
                           pub scanlator: Option<String>, pub read: bool, pub bookmark: bool, pub last_page_read: i64,
                           pub source: i64, pub favorite: bool, pub thumbnail_url: Option<String>,
                           pub cover_last_modified: i64, pub date_upload: i64, pub date_fetch: i64 }
```

Row mapping by NAME, not position (`row.get("title")`), because `SELECT *` order is guaranteed by the baseline but name access fails loudly if a later migration reorders columns; the library view exposes the extra columns by name (`totalCount`, `readCount`, `latestUpload`, `chapterFetchedAt`, `lastRead`, `bookmarkCount`, `category`). `sum()` results are read as `i64` (SQLite returns integers for integer sums; the Kotlin `Double` was a SQLDelight typing artefact).

## 3. Statement modules

Each module exposes `pub const` SQL strings copied from `p1-topic-query-catalog.md` (SQL identical, `IN :list` replaced by `IN (SELECT value FROM json_each(:ids))`) and small typed functions taking `&rusqlite::Connection` (works for both `&Connection` and `&Transaction` through `Deref`):

```rust
// sql/chapters.rs
pub const GET_BY_MANGA_ID: &str = "SELECT C.* FROM chapters C LEFT JOIN excluded_scanlators ES \
    ON C.manga_id = ES.manga_id AND C.scanlator = ES.scanlator \
    WHERE C.manga_id = :mangaId AND (:applyScanlatorFilter = 0 OR ES.scanlator IS NULL)";
pub const REMOVE_WITH_IDS: &str = "DELETE FROM chapters WHERE _id IN (SELECT value FROM json_each(:ids))";

pub fn get_by_manga_id(c: &rusqlite::Connection, manga_id: i64, apply_scanlator_filter: bool) -> DbResult<Vec<ChapterRow>> {
    let mut st = c.prepare_cached(GET_BY_MANGA_ID)?;
    let rows = st.query_map(
        rusqlite::named_params! { ":mangaId": manga_id, ":applyScanlatorFilter": apply_scanlator_filter as i64 },
        ChapterRow::from_row,
    )?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
}
pub fn remove_with_ids(c: &rusqlite::Connection, ids: &[i64]) -> DbResult<usize> {
    let json = serde_json::to_string(ids).expect("i64 slice serialises");
    Ok(c.prepare_cached(REMOVE_WITH_IDS)?.execute(rusqlite::named_params! { ":ids": json })?)
}
```

`prepare_cached` replaces SQLDelight's prepared-statement cache (per connection LRU, default capacity 16: raise with `Connection::set_prepared_statement_cache_capacity(64)` because the app has about 74 statements).

Statement-level rules to keep:
1. Every `coalesce(:x, col)` update stays a single static statement; `Option::None` binds NULL (rusqlite maps `None` to NULL). The patch types therefore reproduce "cannot set NULL".
2. Repositories pass `is_syncing = Some(false)` in ordinary updates (as Kotlin passes 0) and `Some(true)` only for backup restore updates.
3. Statements that return `last_insert_rowid()` use `Connection::last_insert_rowid()` immediately after the insert on the same connection inside the same transaction (replaces the `SELECT last_insert_rowid()` second statement).
4. `getMangaById` and similar `executeAsOne` calls return `DbError::NotFound` on no row; `OrNull` variants return `Option`.

## 4. Open, migrate, import

```rust
// open.rs
/// PRAGMA application_id of every Kurobayomi database: ASCII "KUR0" = 0x4B555230.
/// Aniyomi/Mihon files have application_id 0 (never set), which is how a user_version-33 Aniyomi file is told apart.
pub const APPLICATION_ID: i64 = 0x4B55_5230;

pub fn open_connection(path: &std::path::Path) -> DbResult<rusqlite::Connection>;   // pragmas of p1-topic-reactive-runtime.md section 2

pub fn open_and_migrate(path: &std::path::Path, bundle: &SchemaBundle) -> DbResult<rusqlite::Connection> {
    let mut c = open_connection(path)?;
    let user_version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let application_id: i64 = c.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let tables: i64 = c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table'", [], |r| r.get(0))?;
    let latest = bundle.migrations.last().map_or(bundle.baseline_version, |m| m.0);
    if tables == 0 {
        create_baseline(&mut c, bundle)?;                                           // one transaction
    } else if application_id != APPLICATION_ID {
        return Err(DbError::LegacyFile { found: user_version });                    // Aniyomi file at any version 1..=33, or a foreign database
    } else if user_version > latest {
        return Err(DbError::TooNew { found: user_version, supported: latest });
    } else if user_version < latest {
        run_migrations(&mut c, bundle, user_version)?;                              // each N.sql in its own transaction + foreign_key_check
    }
    c.pragma_update(None, "foreign_keys", "ON")?;
    Ok(c)
}
```

`create_baseline`: `let tx = c.transaction()?; tx.execute_batch(bundle.baseline_sql)?; tx.pragma_update(None, "application_id", APPLICATION_ID)?; tx.pragma_update(None, "user_version", bundle.baseline_version)?; tx.commit()?;` (verified: `user_version` can be set and read back through `pragma_update`/`pragma_query_value`; `application_id` is the standard SQLite header field at offset 68).

`run_migrations` pre-step: `VACUUM INTO` backup when `v < latest` (see `p1-behavior.md` 1.2). Migration files may contain several statements; they run through `execute_batch`. A failing migration rolls back its transaction, leaves `user_version` unchanged and surfaces `DbError::Sqlite` with the migration number in the log (not in the dialog).

`legacy.rs` (feature `legacy-import`): algorithm in `p1-topic-migration-history.md` section 5; public entry `pub fn import_aniyomi_db(src: &Path, dest: &Path, progress: impl FnMut(ImportStep)) -> DbResult<ImportReport>` where `ImportReport { source_version: i64, rows_copied: BTreeMap<&'static str, u64>, orphans_dropped: BTreeMap<&'static str, u64>, quirks: Vec<ImportQuirk> }` and `ImportQuirk::{ DateAddedBackfillBug, HistoryTempDropped, MangasCategoriesTimestampDropped }`.

## 5. Concurrency model

```rust
// runtime.rs
pub struct Db { inner: std::sync::Arc<Inner> }
struct Inner {
    writer: std::sync::Mutex<rusqlite::Connection>,          // hooks installed here
    readers: Vec<std::sync::Mutex<rusqlite::Connection>>,     // 4 connections, round-robin
    changes: [tokio::sync::watch::Sender<u64>; 9],            // one per table, generation counter
}
impl Db {
    pub async fn read<T: Send + 'static>(&self, f: impl FnOnce(&rusqlite::Connection) -> DbResult<T> + Send + 'static) -> DbResult<T>;
    pub async fn write<T: Send + 'static>(&self, f: impl FnOnce(&rusqlite::Transaction) -> DbResult<T> + Send + 'static) -> DbResult<T>;
    pub fn subscribe<T: Send + 'static>(&self, deps: Tables, q: impl Fn(&rusqlite::Connection) -> DbResult<T> + Send + Sync + 'static) -> tokio::sync::mpsc::Receiver<DbResult<T>>;
}
```

- `read`/`write` run the closure inside `tokio::task::spawn_blocking`; `write` locks the writer mutex, opens `BEGIN IMMEDIATE`-style via `Connection::transaction_with_behavior(TransactionBehavior::Immediate)` (avoids upgrade deadlocks in WAL), commits on `Ok`, rolls back on `Err`/panic.
- `subscribe` spawns a task that loops `{ run q on a reader; send; wait until any watch in deps changes }`; because `watch` coalesces, bursts collapse like SQLDelight's conflated channel. The receiver is consumed on the GTK main thread (area:01-foundation-app-bootstrap provides the bridge; with gtk4-rs 0.11 the pattern is `glib::spawn_future_local(async move { while let Some(v) = rx.recv().await { update_widgets(v) } })`, which needs no extra crate because `tokio::sync::mpsc::Receiver::recv` is runtime-agnostic).
- tokio is NEVER run on the GTK main thread; GTK widgets are NEVER touched from tokio threads. The DB layer exposes only `Send` data (plain structs of `String`/`i64`).

## 6. How it plugs into the existing mockup

The mockup (`/home/blackbox/kurobayomi/src`) has no data layer: `src/screens.rs` fabricates `SAMPLE = 18` cards via `title_n(i)` and `format!` strings, `src/settings.rs::clear_db` fabricates 4 sources with counts `12 * (i + 1)`, `src/entry.rs` fabricates title grids. Screen ids come from `spec/app.json` (`manga_library`, `anime_library`, `updates`, `history`, `categories`), `spec/settings.json` (`settings_clear_manga_database`, `settings_manga_extension_repos`) and `spec/entries.json` (`tracking_sheet`, `extension_repos`); each has an `empty` string (for example "Your library is empty", "Nothing to clear", "You have no repos set.") that is the real empty state.

Seams (replace the fabricated loop, keep the widget construction):

| Mockup code | Replace with | Data source in `kuro-db` |
|---|---|---|
| `screens::library(id, anime)` loop `for i in 1..=SAMPLE` | iterate the grouped library rows of the selected category tab (`cats` from `s.tabs` becomes the real category list); empty -> `s.empty` | `sql::views::library` -> `Vec<LibraryRow>` grouped by `category`; subscription deps = library set |
| `screens::history()` day groups | group `HistoryViewRow` by local date of `read_at` | `sql::views::history(query)` |
| `screens::updates()` day groups | group `UpdateViewRow` by local date of `date_fetch` | `sql::views::recent_updates(after, 500)` |
| `screens::categories()` fixed names "Reading", "Plan to read", "Finished" | real list incl. hidden toggle, drag reorder, FAB create | `sql::categories::*` |
| `settings::clear_db(id)` 4 fake sources | per-source counts and delete | `sql::mangas::get_source_ids_with_non_library_manga`, `delete_mangas_not_in_library_by_source_ids` + `history::remove_resetted` in one transaction |
| extension repos screen | list/add/delete repos | `sql::extension_repos::*` |

Rule from the project owner's memory ("build the mechanism, not sample content"): no seeded demo rows; a fresh database shows the empty states above. For UI development the developer imports a real Aniyomi database or backup instead.

The `Db` handle is created once in `main.rs::build_ui` (after `gtk::Application` activation) through the app context of area:01-foundation-app-bootstrap and stored where `ui.rs` keeps its `thread_local!` `SPEC`; screens obtain it via `ui::db()` (to be added by `T-03a-034`).

## 7. Android/Kotlin -> Linux/Rust mapping for everything touched

| Android/Kotlin | Replacement |
|---|---|
| SQLDelight 2.0.2 Gradle plugin + generated `Database`, `*Queries`, `*Adapter` | hand-kept SQL constants + `from_row` mappers (`kuro-db::sql`, `models`) |
| `AndroidSqliteDriver` + `SupportSQLiteOpenHelper` | `rusqlite::Connection` + `open.rs` |
| `RequerySQLiteOpenHelperFactory` (requery sqlite-android 3.45.0), `FrameworkSQLiteOpenHelperFactory` (debug inspector) | `libsqlite3-sys` bundled 3.53.2; use the `sqlite3` CLI or DB Browser for inspection |
| `ColumnAdapter` (Date, List<String>, UpdateStrategy, FetchType, Memo) | `codec.rs` (`FetchType`/`Memo` are anime-only: area:04-db-anime-schema-migrations) |
| `Query.Listener` + `asFlow()` + coroutines extensions | `update_hook`/`commit_hook` + `tokio::sync::watch` (`notify.rs`, `runtime.rs`) |
| Coroutine transaction context (`TransactionElement`, thread acquisition) | `Db::write` closure with `&Transaction` |
| `androidx.paging` `QueryPagingMangaSource` | dropped (no caller) |
| `AndroidManifest.xml` (empty) / `build.gradle.kts` / `consumer-rules.pro` | Cargo manifest; nothing to port |
| `schemaOutputDirectory` + `verifyMigration` | `kuro-db` conformance tests (`T-03a-021`) |
| `strftime('%s','now')` triggers | unchanged (SQLite function) |
| `Dispatchers.IO` | `tokio::task::spawn_blocking` |
| `logcat` of swallowed DB exceptions | `tracing` events (area:01-foundation-app-bootstrap chooses the logging crate) |

## 8. Pitfalls checklist for implementers

1. Do not enable `recursive_triggers`.
2. `commit_hook` must return `false`.
3. `journal_mode` must be set with `pragma_update_and_check`.
4. Write `website = :website` (not `=: website`).
5. Use single-quoted SQL strings only.
6. `foreign_keys` is per connection: set it on EVERY connection including readers.
7. WAL sidecar files must travel with the database when copying.
8. Never run migrations 1..32 on a fresh database.
9. Never open an Aniyomi file for writing without importing it (it may have `history_temp`, a differently named trigger, and `NULL` history dates).
10. `sum()` over an empty set is NULL: libraryView wraps it in `coalesce`; any new query must do the same.
11. `chapters.insert` / `mangas.insert` write `last_modified_at = 0`; do not "fix" it, the trigger sets it on the first update.
12. Reading `lower(title)` for search only folds ASCII; do not promise Unicode-insensitive search until `T-03a-025` ships.
