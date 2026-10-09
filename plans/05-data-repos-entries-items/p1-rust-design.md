# Area 05 — Rust design (part 1 of 1)

Target: stable Rust, `rusqlite`, `tokio`, `gtk4-rs 0.11` (no libadwaita). Everything in this area is GTK-free and lives in the `kuro-core` crate (workspace layout owned by area:01-foundation-app-bootstrap; OS code only in `kuro-platform` per plans/addenda/requirements-cross-platform.md rule 1, so this area contains no `cfg(target_os)`). Paths of Kotlin sources are repo-relative to `/home/blackbox/aniyomi-src`.

## 1. Module layout

```
kuro-core/src/data/
  mod.rs               // pub use of everything below
  db.rs                // Db, open(), pragmas, reader/writer connections, Notifier, TableSet, Subscription
  error.rs             // DataError
  time.rs              // now_secs(), now_millis(), start_of_local_day_millis()
  sqlutil.rs           // in_clause(), chunked ids, lower_unicode scalar function, column-name map
  adapters.rs          // DatabaseAdapter.kt: StringList, strategies, FetchType, Memo codecs
  models.rs            // row structs shared with area:07-domain-entries-items (see 2) : Manga, Anime, Chapter, Episode, Category, *Update, Library*, SeasonAnime, DeletableAnime, AnimeRelationGroup
  category.rs          // CategoryRepo (manga + anime via EntryKind)
  manga/
    mapper.rs          // MangaMapper.kt
    repo.rs            // MangaRepo (MangaRepositoryImpl.kt)
    chapters.rs        // ChapterRepo (ChapterRepositoryImpl.kt)
  anime/
    mapper.rs          // AnimeMapper.kt
    repo.rs            // AnimeRepo (AnimeRepositoryImpl.kt) incl. seasons
    episodes.rs        // EpisodeRepo (EpisodeRepositoryImpl.kt)
    relations.rs       // AnimeRelationRepo
  items/
    sanitize.rs        // ChapterSanitizer.kt + EpisodeSanitizer.kt (one function)
```

Ownership boundary: the type definitions in `models.rs` are the Rust twins of the domain classes catalogued by area:07-domain-entries-items (flag-mask accessors such as `Manga::unread_filter()` are implemented there as `impl` blocks in `kuro-core/src/domain/`); this area only requires that the field set equals `p1-data-and-config.md` section 2. Schema DDL, migrations and the `PRAGMA user_version` handling are owned by area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations; `db::open()` calls their `migrate(&mut Connection)` before handing connections out.

## 2. Crates and why

| Crate | Features | Replaces | Why |
|---|---|---|---|
| `rusqlite` | `bundled`, `functions`, `hooks`, `serde_json` (optional) | SQLDelight + `AndroidSqliteDriver` + `RequerySQLiteOpenHelperFactory` | The Kotlin app ships its own SQLite through Requery; `bundled` gives one SQLite version on every OS (requirement file rule 7, Windows later), a known `SQLITE_MAX_VARIABLE_NUMBER`, and trigger/hook behaviour identical to Android's. `functions` for `lower_unicode`, `hooks` for update/commit/rollback hooks. |
| `tokio-rusqlite` | default | `Dispatchers.IO` + transaction thread leasing in `AndroidMangaDatabaseHandler` | One dedicated OS thread per `Connection`, async `call(|conn| ...)` closure API, runtime-agnostic futures (they can be awaited from `glib::spawn_future_local` and from tokio tasks). A transaction is simply a closure that runs to completion on that thread, which removes the coroutine transaction-context machinery entirely. |
| `tokio` | `rt-multi-thread`, `sync`, `time` | coroutines | `watch`/`broadcast` for subscriptions, `time::sleep` for debounce. |
| `serde_json` | | kotlinx.serialization `JsonObject` | `memo` BLOB is a JSON object; `serde_json::Map<String, Value>` compact serialisation equals `JsonObject.toString()` for objects built by this program (key order preserved with the `preserve_order` feature; enable it). |
| `thiserror` | | exceptions | `DataError`. |
| `tracing` | | `logcat(LogPriority.ERROR, e)` | structured logs; area:01-foundation-app-bootstrap owns the subscriber. |
| `async-channel` | | `Channel`/`callbackFlow` | pushing results to the GTK main context. |
| `bitflags` | | the `Set<String>` of table names in `asFlow()` | `TableSet` bitset for the notifier. |
| `time` | `local-offset` | `LocalDate.now().atStartOfDay(ZoneId.systemDefault())` | local midnight for the upcoming queries. |

No ORM, no query builder, no `r2d2`. SQL stays as `&'static str` constants copied from the `.sq` files so a reviewer can diff them against Kotlin.

## 3. Core types (signatures)

```rust
// db.rs
pub struct Db { writer: tokio_rusqlite::Connection, reader: tokio_rusqlite::Connection, notifier: Notifier, kind: DbKind }
pub enum DbKind { Manga, Anime }                       // files "tachiyomi.db" / "tachiyomi.animedb"

impl Db {
    pub async fn open(paths: &dyn kuro_platform::Paths, kind: DbKind) -> Result<Db, DataError>;
    pub async fn open_in_memory(kind: DbKind) -> Result<Db, DataError>;   // tests; same schema + pragmas + functions
    pub(crate) async fn read<T: Send + 'static>(&self, f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static) -> Result<T, DataError>;
    pub(crate) async fn write<T: Send + 'static>(&self, f: impl FnOnce(&mut rusqlite::Transaction) -> rusqlite::Result<T> + Send + 'static) -> Result<T, DataError>; // always a transaction
    pub(crate) async fn write_no_tx<T: Send + 'static>(&self, f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static) -> Result<T, DataError>;
    pub fn subscribe<T, F>(&self, tables: TableSet, debounce: Duration, query: F) -> Subscription<T>
        where T: PartialEq + Send + Sync + 'static,
              F: Fn(&rusqlite::Connection) -> rusqlite::Result<T> + Send + Sync + 'static;
}

pub struct Subscription<T> { rx: tokio::sync::watch::Receiver<Option<Arc<T>>> /* None until first load */ }
impl<T> Subscription<T> {
    pub async fn changed(&mut self) -> Option<Arc<T>>;   // None when the Db is dropped
    pub fn current(&self) -> Option<Arc<T>>;
}

bitflags::bitflags! { pub struct TableSet: u32 {
    const MANGAS=1<<0; const CHAPTERS=1<<1; const CATEGORIES=1<<2; const MANGAS_CATEGORIES=1<<3; const EXCLUDED_SCANLATORS=1<<4; const HISTORY=1<<5;
    const ANIMES=1<<6; const EPISODES=1<<7; const ANIMES_CATEGORIES=1<<8; const ANIME_RELATIONS=1<<9; const ANIMEHISTORY=1<<10;
} }

// error.rs
#[derive(thiserror::Error, Debug)]
pub enum DataError {
    #[error("not found")] NotFound,
    #[error("sqlite: {0}")] Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")] Json(#[from] serde_json::Error),
    #[error("db task closed")] Closed,
}
```

Repositories (concrete structs holding `Arc<Db>`; no trait because each has exactly one implementation; tests use `Db::open_in_memory`):

```rust
// manga/repo.rs
pub struct MangaRepo(Arc<Db>);
impl MangaRepo {
    pub async fn by_id(&self, id: i64) -> Result<Manga, DataError>;                       // NotFound when absent
    pub fn subscribe_by_id(&self, id: i64) -> Subscription<Option<Manga>>;
    pub async fn by_url_and_source(&self, url: &str, source: i64) -> Result<Option<Manga>, DataError>;
    pub fn subscribe_by_url_and_source(&self, url: &str, source: i64) -> Subscription<Option<Manga>>;
    pub async fn favorites(&self) -> Result<Vec<Manga>, DataError>;
    pub async fn read_not_in_library(&self) -> Result<Vec<Manga>, DataError>;
    pub async fn library(&self) -> Result<Vec<LibraryManga>, DataError>;
    pub fn subscribe_library(&self) -> Subscription<Vec<LibraryManga>>;                   // debounce 100 ms
    pub fn subscribe_favorites_by_source(&self, source: i64) -> Subscription<Vec<Manga>>;
    pub async fn duplicates(&self, id: i64, title: &str) -> Result<Vec<Manga>, DataError>; // lowercases internally
    pub fn subscribe_upcoming(&self, statuses: &[i64]) -> Subscription<Vec<Manga>>;
    pub async fn reset_viewer_flags(&self) -> Result<(), DataError>;
    pub async fn set_categories(&self, manga_id: i64, category_ids: &[i64]) -> Result<(), DataError>;
    pub async fn insert(&self, m: &Manga) -> Result<i64, DataError>;
    pub async fn update(&self, u: &MangaUpdate) -> Result<(), DataError>;
    pub async fn update_all(&self, us: &[MangaUpdate]) -> Result<(), DataError>;          // one transaction
}

// manga/chapters.rs
pub struct ChapterRepo(Arc<Db>);
impl ChapterRepo {
    pub async fn add_all(&self, chapters: Vec<Chapter>) -> Result<Vec<Chapter>, DataError>; // atomic, ids filled
    pub async fn update(&self, u: &ChapterUpdate) -> Result<(), DataError>;
    pub async fn update_all(&self, us: &[ChapterUpdate]) -> Result<(), DataError>;
    pub async fn remove_with_ids(&self, ids: &[i64]) -> Result<(), DataError>;             // chunked in one tx
    pub async fn by_manga_id(&self, manga_id: i64, apply_scanlator_filter: bool) -> Result<Vec<Chapter>, DataError>;
    pub fn subscribe_by_manga_id(&self, manga_id: i64, apply_scanlator_filter: bool) -> Subscription<Vec<Chapter>>;
    pub async fn scanlators(&self, manga_id: i64) -> Result<Vec<String>, DataError>;        // not distinct, NULL -> ""
    pub fn subscribe_scanlators(&self, manga_id: i64) -> Subscription<Vec<String>>;
    pub async fn bookmarked(&self, manga_id: i64) -> Result<Vec<Chapter>, DataError>;
    pub async fn by_id(&self, id: i64) -> Result<Option<Chapter>, DataError>;
    pub async fn by_url_and_manga_id(&self, url: &str, manga_id: i64) -> Result<Option<Chapter>, DataError>;
    // beyond the Kotlin repository: the excluded_scanlators table access that Kotlin does through raw handler calls
    pub async fn excluded_scanlators(&self, manga_id: i64) -> Result<Vec<String>, DataError>;
    pub async fn set_excluded_scanlators(&self, manga_id: i64, scanlators: &HashSet<String>) -> Result<(), DataError>; // diff algorithm, one tx
}
// anime/repo.rs
pub struct AnimeRepo(Arc<Db>);
impl AnimeRepo {
    pub async fn by_id(&self, id: i64) -> Result<Anime, DataError>;
    pub fn subscribe_by_id(&self, id: i64) -> Subscription<Option<Anime>>;
    pub async fn by_url_and_source(&self, url: &str, source: i64) -> Result<Option<Anime>, DataError>;
    pub fn subscribe_by_url_and_source(&self, url: &str, source: i64) -> Subscription<Option<Anime>>;
    pub async fn favorites(&self) -> Result<Vec<Anime>, DataError>;
    pub async fn watched_not_in_library(&self) -> Result<Vec<Anime>, DataError>;
    pub async fn library(&self) -> Result<Vec<LibraryAnime>, DataError>;
    pub fn subscribe_library(&self) -> Subscription<Vec<LibraryAnime>>;                    // debounce 100 ms
    pub fn subscribe_favorites_by_source(&self, source: i64) -> Subscription<Vec<Anime>>;
    pub async fn duplicates(&self, id: i64, title: &str) -> Result<Vec<Anime>, DataError>;
    pub fn subscribe_upcoming(&self, statuses: &[i64]) -> Subscription<Vec<Anime>>;
    pub async fn reset_viewer_flags(&self) -> Result<(), DataError>;                         // repository only; no UI
    pub async fn set_categories(&self, anime_id: i64, category_ids: &[i64]) -> Result<(), DataError>;
    pub async fn insert(&self, a: &Anime) -> Result<i64, DataError>;
    pub async fn update(&self, u: &AnimeUpdate) -> Result<(), DataError>;
    pub async fn update_all(&self, us: &[AnimeUpdate]) -> Result<(), DataError>;
    pub async fn seasons_by_parent(&self, parent_id: i64) -> Result<Vec<SeasonAnime>, DataError>;
    pub fn subscribe_seasons_by_parent(&self, parent_id: i64) -> Subscription<Vec<SeasonAnime>>;
    pub async fn remove_parent_ids(&self, ids: &[i64]) -> Result<(), DataError>;
    pub fn subscribe_deletable_parents(&self) -> Subscription<Vec<DeletableAnime>>;
    pub async fn children_by_parent(&self, parent_id: i64) -> Result<Vec<Anime>, DataError>;
}

// anime/episodes.rs
pub struct EpisodeRepo(Arc<Db>);
impl EpisodeRepo {
    pub async fn add_all(&self, episodes: Vec<Episode>) -> Result<Vec<Episode>, DataError>;
    pub async fn update(&self, u: &EpisodeUpdate) -> Result<(), DataError>;
    pub async fn update_all(&self, us: &[EpisodeUpdate]) -> Result<(), DataError>;
    pub async fn remove_with_ids(&self, ids: &[i64]) -> Result<(), DataError>;
    pub async fn by_anime_id(&self, anime_id: i64) -> Result<Vec<Episode>, DataError>;
    pub fn subscribe_by_anime_id(&self, anime_id: i64) -> Subscription<Vec<Episode>>;
    pub async fn bookmarked(&self, anime_id: i64) -> Result<Vec<Episode>, DataError>;
    pub async fn by_id(&self, id: i64) -> Result<Option<Episode>, DataError>;
    pub async fn by_url_and_anime_id(&self, url: &str, anime_id: i64) -> Result<Option<Episode>, DataError>;
}

// anime/relations.rs
pub struct AnimeRelationRepo(Arc<Db>);
impl AnimeRelationRepo {
    pub async fn last_fetched_at(&self, anime_id: i64) -> Result<Option<i64>, DataError>;
    pub async fn replace_relations(&self, anime_id: i64, groups: &[(String, Vec<i64>)], fetched_at: i64) -> Result<(), DataError>;
    pub fn subscribe_related(&self, anime_id: i64) -> Subscription<Vec<AnimeRelationGroup>>;
}

// Helper queries that the .sq files declare and other areas consume with raw handler calls (T-05a-031).
// getSourceIdWithFavoriteCount and getSourceIdsWithNonLibraryManga are consumed only by the source repositories (area:06-data-repos-history-track-source-misc).
impl MangaRepo { pub async fn all_source_and_url(&self) -> Result<Vec<(i64, String)>, DataError>;           // getAllMangaSourceAndUrl
                 pub async fn delete_non_library_by_sources(&self, source_ids: &[i64]) -> Result<(), DataError>; // deleteMangasNotInLibraryBySourceIds
                 }
impl AnimeRepo { pub async fn all_source_and_url(&self) -> Result<Vec<(i64, String)>, DataError>;
                 pub async fn delete_non_library_by_ids(&self, ids: &[i64]) -> Result<(), DataError>; }          // deleteAnimesNotInLibraryByAnimeIds (favorite = 0 guard stays in SQL)
impl ChapterRepo { pub async fn by_url(&self, url: &str) -> Result<Vec<Chapter>, DataError>; }                   // getChapterByUrl
impl EpisodeRepo { pub async fn by_url(&self, url: &str) -> Result<Vec<Episode>, DataError>; }                   // getEpisodeByUrl
```

Update structs: every field `Option<T>`, `#[derive(Default)]`, so call sites write `MangaUpdate { id, favorite: Some(true), ..Default::default() }` (the Kotlin named-argument style). `id` is required and has no default, so use `MangaUpdate::new(id)`.

The legacy adapter (`data::compat`): `pub async fn update_manga_bool(repo, u) -> bool` and the equivalent wrappers for the other swallowing methods, for the interactor layer (area:10-app-domain-layer) to preserve the Kotlin `Boolean`/empty-list return conventions (`p1-behavior.md` section 0).

## 4. Row mapping

```rust
fn map_manga(row: &rusqlite::Row, c: &Cols) -> rusqlite::Result<Manga> {
    Ok(Manga {
        id: row.get(c.id)?, source: row.get(c.source)?, url: row.get(c.url)?,
        last_update: row.get::<_, Option<i64>>(c.last_update)?.unwrap_or(0),
        next_update: row.get::<_, Option<i64>>(c.next_update)?.unwrap_or(0),
        fetch_interval: row.get::<_, i64>(c.calculate_interval)? as i32,
        genre: row.get::<_, Option<String>>(c.genre)?.map(|s| adapters::decode_string_list(&s)),
        update_strategy: UpdateStrategy::from_i64(row.get(c.update_strategy)?),
        /* remaining fields one to one; is_syncing is not read */
    })
}
```

`Cols` is built once per prepared statement from `Statement::column_names()` (`HashMap<&str, usize>` then a struct of indexes) so column order changes cannot corrupt data. `favorite`, `initialized`, `read`, `seen`, `bookmark`, `fillermark` are read as `bool` (rusqlite maps nonzero integers to true; same as SQLDelight `INTEGER AS Boolean`, which treats `1` as true). `hidden` is read as `i64 == 1`. Aggregates read as `f64` then `as i64`. Genre round-trip: `decode_string_list("")` = `vec![]`, otherwise `split(", ")`; `encode_string_list` = `join(", ")`.

## 5. Async and threading model

- One tokio multi-thread runtime owned by the app (area:01-foundation-app-bootstrap) for network and background work. Database access does not need it: `tokio-rusqlite::Connection::call` returns a future driven by a oneshot channel from the connection thread.
- GTK main thread never blocks on the database. UI code calls `glib::spawn_future_local(async move { let items = repo.library().await?; model.splice(...) })`. Because `tokio-rusqlite` futures are executor-agnostic, no `tokio::Runtime::enter` is needed on the GTK thread.
- Two connections per file: the writer (all writes; hooks installed) and one reader (all SELECTs and subscriptions). WAL lets the reader run while the writer commits. If profiling shows reader contention (a slow library view blocking an entry lookup), add a second reader and round-robin; not needed up front.
- Every write method is a single `Db::write` call = one transaction, matching `handler.await(inTransaction = true)`. Methods that were non-transactional in Kotlin (`updatePartialMangaCategory`, `updateAllMangaCategoryFlags`, `deleteMangaCategory`, `resetMangaViewerFlags`, `removeChaptersWithIds` single call) also use a transaction (a single statement is atomic anyway); this changes no observable behaviour.
- Cancellation: Kotlin wraps category writes in `withNonCancellableContext`. A Rust future that is dropped after being handed to `tokio-rusqlite` still runs to completion on the connection thread (the closure was already queued), which gives the same guarantee for free; document it, and never call `abort()` on tasks that await writes.

## 6. GTK bridge for subscriptions

```rust
// kurobayomi/src/data_bridge.rs (GTK crate)
pub fn bind<T: Send + Sync + 'static>(mut sub: Subscription<T>, apply: impl Fn(&T) + 'static) -> glib::JoinHandle<()> {
    glib::spawn_future_local(async move {
        while let Some(v) = sub.changed().await { apply(&v); }
    })
}
```

`apply` updates a `gio::ListStore` (item type = a `glib::Object` wrapper `BoxedAnyObject`) with `splice(0, n_items, &new_items)` for lists, or sets labels for single entities. Stop a binding by calling `handle.abort()` when the screen is destroyed (connect to the widget's `unrealize`/`destroy`). A screen holds the handle in a struct field; dropping the handle does not abort (glib semantics), so abort explicitly. Replaces: Compose `collectAsState`, `StateScreenModel`, Paging.

## 7. Time and ids

`i64` everywhere for ids and epoch values. `time.rs`: `now_millis() -> i64` (`SystemTime::now()` since epoch), `now_secs()`, `start_of_local_day_millis()` using `chrono::Local`? Avoid a new dependency: GLib is already linked in the GTK crate but `kuro-core` is GTK-free, so use the `time` crate with `local-offset` feature (`OffsetDateTime::now_local()`), falling back to UTC when the offset cannot be determined (multi-threaded `local-offset` can fail on Linux; compute it once at startup on the main thread and cache it).

## 8. Android API / library to Rust mapping (touched by this area)

| Kotlin / Android | Rust / Linux |
|---|---|
| SQLDelight codegen (`Database`, `AnimeDatabase`, `*Queries`) | hand-written `const` SQL + rusqlite (`p1-data-and-config.md` section 5 is the source text) |
| `AndroidSqliteDriver` + `RequerySQLiteOpenHelperFactory` / `FrameworkSQLiteOpenHelperFactory` | `rusqlite` with `bundled`; no Android SQLite |
| `ColumnAdapter` (`DatabaseAdapter.kt`) | `adapters.rs` functions + `FromSql`/`ToSql` newtypes |
| `kotlinx.coroutines.flow.Flow`, `asFlow().mapToList` | `Subscription<T>` over `tokio::sync::watch` |
| `Dispatchers.IO`, transaction dispatcher | `tokio-rusqlite` connection threads |
| `androidx.paging.PagingSource`, `QueryPagingSource` | Dropped for this area (no in-scope user); lists are fully loaded and shown in a `gio::ListStore` with `gtk::ListView` virtualisation (T-05a-027) |
| `logcat` | `tracing` |
| `java.util.Date` | `i64` epoch millis newtype-free |
| `kotlinx.serialization.json.JsonObject` | `serde_json::Map` |
| `LocalDate.now().atStartOfDay(ZoneId.systemDefault())` | `time::OffsetDateTime::now_local().date().midnight().assume_offset(offset)` |
| Injekt singletons `Database`, `MangaDatabaseHandler` | fields of the app context (area:01-foundation-app-bootstrap) holding `Arc<Db>` and the repo structs |

## 9. Plugging into the existing mockup (`/home/blackbox/kurobayomi/src`, `spec/*.json`)

Current state: `src/screens.rs`, `src/entry.rs`, `src/ui.rs` build screens from `spec/*.json` (UI strings extracted from Aniyomi) and fill lists with placeholder data (`title_n(i)` in `screens.rs`, `cover_grid` in `entry.rs`); `categories()` at `screens.rs:404-422` renders the hard-coded names "Reading", "Plan to read", "Finished" under `kind_tabs` (Anime/Manga). There is no data layer, no Tokio, no SQLite dependency (`Cargo.toml` has only `gtk4`, `serde`, `serde_json`).

Integration steps (each is one task):

1. Add `kuro-core` (this area's modules) to the workspace; keep `spec/*.json` as the string source only. Nothing in `spec/` changes: it contains dialog titles/buttons such as the `categories` screen's "Add category", "Rename category", "Delete category" and the error text `A category with this name already exists!`, which stay UI strings.
2. `categories()`: replace the literal name list with `CategoryRepo::subscribe_all()` bound through `data_bridge::bind`; filter `is_system()`; the rename/hide/delete buttons call the use-case layer of area:08-domain-library-category-history-updates (create = `CreateMangaCategoryWithName` equivalent, which reads `nextOrder` and the global sort flags, then `CategoryRepo::insert`); the dialogs `category_create`/`category_rename` validate against the displayed names with exact case-sensitive comparison before enabling OK.
3. Library screens (`anime_library`, `manga_library` in `spec/app.json`): `subscribe_library()` grouped by `category` in the UI crate; the mockup's `cover_grid(n, w, on)` takes a count; change it to take `&[LibraryItem]`.
4. Entry details (`entry()` in `entry.rs:~100`): `by_id`/`subscribe_by_id`, `ChapterRepo::subscribe_by_manga_id`, `CategoryRepo::subscribe_by_entry` for the `set_categories` dialog (dialog id `set_categories` is opened from `entry.rs:229`), `set_categories` on confirm.
5. Replace nothing else: browse/search/migration screens depend on areas 40/41.

## 10. Pitfalls (Rust-specific and port-specific)

1. rusqlite named parameters: use `:name` placeholders and `named_params!`; do not reuse a positional style for the 29-column insert, a transposed column silently stores garbage.
2. `Option<bool>` binding in rusqlite: `None` binds NULL (correct for `coalesce(:favorite, favorite)`); verify with a test that `favorite: None` leaves the value unchanged.
3. Statement cache: `prepare_cached` on the long coalesce updates; the default cache size (16) is enough (one per table).
4. `last_insert_rowid()` must be read from the same connection immediately after the INSERT inside the same closure.
5. Never hold a `rusqlite::Statement`/`Rows` across an `.await`; closures run synchronously on the connection thread and return owned data only.
6. `update_hook` callbacks run during statement execution; they must only push into a `Vec` guarded by a `Mutex`, never touch the database.
7. Schema compatibility: because users may import an existing Aniyomi database, never alter column order or types in this area; additive migrations are area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations's job.
8. `strftime('%s','now')` stays in SQL (triggers); do not duplicate in Rust.
9. Do not enable `PRAGMA recursive_triggers`.
10. Large `IN` lists: chunk by 500 and keep one transaction.
11. JSON `memo` order: enable `serde_json/preserve_order`, otherwise rewritten BLOBs reorder keys (harmless but makes backup diffs noisy).
12. `i32` narrowing of `fetch_interval` and `Direction::valueOf` flag math: use `i64` bit operations (`& !mask | (flag & mask)`), never `u64`, because stored flags can be negative if a future bit 63 is set (the anime viewer flag `ANIME_INTRO_DISABLE_MASK = 0x100000000000000` is bit 56, so positive; stay `i64`).
