# Area 06 — Rust design

Everything below is **Proposal** unless a sentence starts with "OBSERVED". The design obeys the standing requirements: GTK-free logic in `kuro-core` (no `gtk` imports), OS access only through `kuro-platform` traits (`plans/addenda/requirements-cross-platform.md`), no libadwaita.

## 1. Placement and module layout

Workspace (from area:01 `p1-topic-app-context-and-startup.md`): `kuro-core` (logic), `kuro-platform` (OS traits), `kurobayomi` (GTK binary = the existing `src/`).

```text
kuro-core/src/
  db/
    mod.rs            Db, DbKind, Schema (trait), DbError, Table, TableSet, Subscription        (T-06a-001..004)
    notify.rs         update_hook collection + broadcast                                         (T-06a-003)
    testkit.rs        open_temp(), seeded fixtures (cfg(test) or feature "testkit")               (T-06a-005)
  data/
    mod.rs            pub use crate::db as db;   // area:05 refers to kuro_core::data::db
    history/{mod,manga,anime,recorder}.rs                                                       (T-06a-010..014)
    updates/{mod,manga,anime}.rs                                                                (T-06a-020, 021)
    track/{mod,manga,anime}.rs                                                                  (T-06a-030..033)
    legacy.rs         ChapterKey, EpisodeKey                                                    (T-06a-034)
    source/{mod,manga,anime,stub,pager}.rs                                                      (T-06a-040..044)
    extension/
      store/{wire,mapping,service,repo}.rs   anime extension stores                             (T-06a-050..054)
      repo/{mod,service,interactors}.rs      manga extension repos                              (T-06a-055, 056)
    custom_button/{mod,seed,script}.rs                                                          (T-06a-060..062)
  release/{mod,github,mentions}.rs                                                              (T-06a-070..073)
kurobayomi/src/                      (existing mockup crate)
  data_bridge.rs      glue: owns AppContext handle, spawns subscriptions on the glib main context  (T-06a-080, 081)
  screens.rs          history(), updates() rebuilt on data_bridge; extension store/repo screens    (T-06a-057, 080)
```

Types shared with other areas (referenced, not defined here): `MangaCover`/`AnimeCover` (area:07), `Source`-related traits (`CatalogueSource`, `SourceManager`: area:15/16/17), `HttpClient` (area:12), `Notifier`, `Opener`, `Paths` (kuro-platform), `Settings` (area:02).

## 2. Dependencies (kuro-core)

| Crate | Features | Why (ladder: stdlib/native first) |
|---|---|---|
| `rusqlite` | `bundled`, `hooks`, `functions` | The mandated SQLite binding. `bundled` pins a modern SQLite (upserts need >= 3.24) independent of the distro; `hooks` for `update_hook`; `functions` for the Unicode `kuro_fold` search helper |
| `r2d2`, `r2d2_sqlite` | | Reader pool; ~30 lines of glue versus writing a pool. (Alternative `deadpool-sqlite` is async but wraps `interact` closures identically; r2d2 keeps the API synchronous inside `spawn_blocking`.) |
| `parking_lot` | | Non-poisoning `Mutex` for the writer connection and the dirty set |
| `tokio` | `rt-multi-thread`, `sync`, `macros`, `time` | `spawn_blocking`, `watch`, `broadcast`, `select!`. `tokio::sync` primitives work when polled from the glib executor |
| `futures` | | `Stream`, `join_all`, `buffered` for store fan-out |
| `serde`, `serde_json` | `derive` | Store/legacy/release JSON |
| `prost` | `derive` (no `prost-build`, no `protoc`) | Protobuf decode with explicit tag numbers on the Rust structs; avoids a build-time `protoc` dependency |
| `flate2` | | Gzip payload detection (`1f 8b`) |
| `url` | | OkHttp `HttpUrl` normalisation equivalent for repo URLs |
| `fancy-regex` | | Mention regex needs lookahead/lookbehind (the `regex` crate has none) |
| `thiserror` | | Error enums |
| `tracing` | | Replaces `logcat` (levels ERROR/WARN/INFO map one to one) |
| `async-trait` | | Only for the `CatalogueSource` trait object used by the pager (area:15/16/17 own it) |
| dev: `tempfile`, `tokio` with `test-util` | | Temp-file databases and paused-time tests |

HTTP is `reqwest` through area:12's `HttpClient` wrapper (this area never builds its own client). No `chrono` in repositories (see the topic file section 5).

## 3. API surface

### 3.1 Db (`p1-topic-db-handle-and-reactive-queries.md` section 2)

`Db::{open, open_temp, read, write, read_blocking, write_blocking, watch, watch_distinct, notify_all}`, `Table`, `TableSet`, `Subscription<T>`, `DbError`, `Schema`.

### 3.2 History

```rust
pub struct MangaHistoryRepo { db: Db }
impl MangaHistoryRepo {
    pub fn watch(&self, query: &str) -> Subscription<Vec<MangaHistoryWithRelations>>;     // list, latest-per-manga, readAt DESC
    pub async fn latest(&self) -> Result<Option<MangaHistoryWithRelations>, DbError>;
    pub async fn total_read_duration(&self) -> Result<i64, DbError>;
    pub async fn by_manga_id(&self, manga_id: i64) -> Result<Vec<MangaHistory>, DbError>;
    pub async fn reset_by_id(&self, history_id: i64);                // logs, never fails (Kotlin parity)
    pub async fn reset_by_manga_id(&self, manga_id: i64);
    pub async fn delete_all(&self) -> bool;
    pub async fn upsert(&self, u: MangaHistoryUpdate);
    // maintenance / restore support (sync, take &Connection so area:22 can call them inside its transaction)
    pub fn remove_resetted(c: &Connection) -> Result<usize, DbError>;
    pub fn by_chapter_url(c: &Connection, url: &str) -> Result<Option<MangaHistory>, DbError>;
}
pub struct AnimeHistoryRepo { db: Db }   // same minus total_read_duration; upsert takes AnimeHistoryUpdate
pub struct HistoryRecorder { /* start: Option<i64>, incognito: Arc<AtomicBool> */ }
impl HistoryRecorder {
    pub fn start(&mut self, now_ms: i64);
    pub async fn flush(&mut self, repo: &MangaHistoryRepo, chapter_id: i64, now_ms: i64);  // no-op in incognito; duration = max(0, now - start); start = None afterwards
}
```

### 3.3 Updates

```rust
pub const UPDATES_LIMIT: i64 = 500;
pub struct MangaUpdatesRepo { db: Db }
impl MangaUpdatesRepo {
    pub fn watch_recent(&self, after_ms: i64) -> Subscription<Vec<MangaUpdatesWithRelations>>;        // limit = UPDATES_LIMIT
    pub fn watch_with_read(&self, read: bool, after_ms: i64) -> Subscription<Vec<MangaUpdatesWithRelations>>;
    pub async fn with_read(&self, read: bool, after_ms: i64) -> Result<Vec<MangaUpdatesWithRelations>, DbError>;
}
pub fn three_months_ago_ms(now: chrono::DateTime<chrono::Local>) -> i64;   // calendar months, ZonedDateTime.minusMonths(3) semantics (clamps day-of-month)
```

### 3.4 Tracks

```rust
pub struct MangaTrackRepo { db: Db }
impl MangaTrackRepo {
    pub async fn by_id(&self, track_row_id: i64) -> Result<Option<MangaTrack>, DbError>;   // NOT by manga id
    pub async fn by_manga_id(&self, manga_id: i64) -> Result<Vec<MangaTrack>, DbError>;
    pub fn watch_all(&self) -> Subscription<Vec<MangaTrack>>;
    pub fn watch_by_manga_id(&self, manga_id: i64) -> Subscription<Vec<MangaTrack>>;
    pub async fn delete(&self, manga_id: i64, tracker_id: i64) -> Result<(), DbError>;
    pub async fn insert(&self, t: MangaTrack) -> Result<(), DbError>;                     // replace semantics, new row id
    pub async fn insert_all(&self, ts: Vec<MangaTrack>) -> Result<(), DbError>;           // one transaction
    pub fn update_coalesced(c: &Connection, t: &MangaTrackPatch) -> Result<(), DbError>;  // for backup restore
}
pub fn group_by_manga(tracks: Vec<MangaTrack>) -> HashMap<i64, Vec<MangaTrack>>;          // GetTracksPerManga
```

`MangaTrackPatch` has every field as `Option<_>` plus the row `id` (SQL `coalesce(?, col)`).

### 3.5 Sources

`SourceRepo`, `StubSourceRepo`, `RemotePager`, `Listing`, `PagerError` (topic file `p1-topic-source-paging-and-stubs.md`).

### 3.6 Extension stores and repos

```rust
pub struct ExtensionStoreRepo { db: Db, service: ExtensionStoreService }
impl ExtensionStoreRepo {
    pub async fn add(&self, index_url: &str) -> Result<(), StoreError>;
    pub async fn insert_from_preference(&self, index_url: &str, name: &str) -> Result<(), DbError>;
    pub async fn refresh_all(&self);
    pub async fn fetch_extensions(&self) -> Vec<AvailableExtension>;
    pub async fn all(&self) -> Result<Vec<ExtensionStore>, DbError>;
    pub fn watch_all(&self) -> Subscription<Vec<ExtensionStore>>;
    pub fn watch_count(&self) -> Subscription<i64>;
    pub async fn remove(&self, index_url: &str) -> Result<(), DbError>;
}
pub struct ExtensionRepoRepo { db: Db, service: ExtensionRepoService }        // manga repos
pub enum CreateRepoResult { Success, InvalidUrl, RepoAlreadyExists, DuplicateFingerprint { old: ExtensionRepo, new: ExtensionRepo }, Error }
impl ExtensionRepoRepo {
    pub async fn create(&self, index_url: &str) -> CreateRepoResult;
    pub async fn update_all(&self);
    pub async fn replace(&self, new_repo: ExtensionRepo) -> Result<(), DbError>;
    pub async fn delete(&self, base_url: &str) -> Result<(), DbError>;
    pub async fn all(&self) -> Result<Vec<ExtensionRepo>, DbError>;
    pub fn watch_all(&self) -> Subscription<Vec<ExtensionRepo>>;
    pub fn watch_count(&self) -> Subscription<i64>;
    pub async fn upsert_raw(&self, repo: ExtensionRepo) -> Result<(), RepoError>;      // migration path (NOFINGERPRINT-n)
}
```

### 3.7 Custom buttons and release

```rust
pub struct CustomButtonRepo { db: Db }       // all(), watch_all(), create(..), delete(id), reorder(id, new_index), toggle_favorite(id), update(u), update_many(us), insert_raw(..)
pub fn expand_placeholders(text: &str, id: i64, primary_id: i64) -> String;
pub fn build_player_scripts(buttons: &[CustomButton], primary_id: i64, scripts_dir: &Path) -> Vec<(String /*file name*/, String /*lua source*/)>;
pub struct ReleaseClient { .. }               // latest(repository) -> Result<Option<Release>, ReleaseError>
```

## 4. Threading model

* One tokio multi-thread runtime owned by `AppContext` (area:01). All `Db` calls hop to `spawn_blocking` threads; repositories are `async` and return plain data (`Send`, `'static`).
* GTK widgets live on the main thread. A subscription is consumed on the main thread with `glib::spawn_future_local`; each `Arc<Vec<Row>>` is turned into widgets there. The data never touches GTK objects off-thread.
* Fire-and-forget operations from UI callbacks (`history.reset_by_id`, `custom_button.toggle_favorite`): `rt.spawn(async move { ... })`, the result (if the UI needs it, e.g. `CreateRepoResult`) comes back through a one-shot `glib::MainContext` future: `let (tx, rx) = futures::channel::oneshot::channel(); rt.spawn(async move { let _ = tx.send(repo.create(&url).await); }); glib::spawn_future_local(async move { if let Ok(r) = rx.await { show(r) } })`. `data_bridge::spawn_ui<T>(fut, on_done)` wraps this pattern once.
* Library-update workers (area:26) run on blocking threads and use the `*_blocking` Db methods and the sync `fn(&Connection)` repository functions inside their own `write_blocking` transactions.
* Store/repo refresh and release checks run in the background on the runtime; the UI observes results through the watched tables (`extension_store` rows) or one-shot futures.

## 5. Error handling

* `DbError` (topic file) is the only error type for pure-DB functions. UI never shows a raw `DbError`: it maps to the toast text `MR.strings.internal_error` (area:36 `Toast` helper) after logging, as the Kotlin screen models do on `Event.InternalError`.
* Functions that Kotlin makes infallible (`reset_*`, `delete_all`, `upsert` of history, track insert/delete interactors) keep that contract: they log with `tracing::error!` and return `()`/`bool`; their `Result`-returning twins exist as `try_*` for tests.
* `StoreError` and `RepoError` implement `Display` with the exact user-visible strings listed in `p1-topic-extension-store-index.md` section 3.
* No panics in repository code; `expect` only on programmer invariants (e.g. static SQL compile in tests).

## 6. SQL management

* All SQL is `const &str` next to the function, using numbered parameters (`?1`), never `format!` with user input. Statements are executed with `Connection::prepare_cached` so the per-connection statement cache (default capacity 16, raise to 64 with `set_prepared_statement_cache_capacity`) reuses plans.
* Views (manga DB `historyView`, `updatesView`, `libraryView`; anime DB `animehistoryView`, `animeupdatesView`, `animelibView`, `animeseasonsView`, `animeseasonstatsView`, `episodestatsView`, `animehistorystatsView`, `animedeletableView`) are created by the schema crates (areas 03/04) and are part of their snapshot tests; this area's tests run the real schema through `Db::open_temp`.
* Booleans: store as INTEGER 0/1; read via rusqlite's `bool` `FromSql` (any non-zero is true, matching SQLDelight's `AS Boolean` adapter reading `!= 0`).
* NULL-vs-0 for `last_read`: write `0` for reset, never NULL; read as `Option<i64>`.

## 7. Plugging into the existing mockup

OBSERVED: `src/screens.rs` builds `history()` (line 305) and `updates()` (line 277) with hard-coded rows (`title_n(i)`, `thumb(48, 70)`, `list_row`), `kind_tabs` for the Anime/Manga tabs, `dialog_of("history", "history_remove_manga")` for the spec-driven remove dialog, `built(content, end)` to return the page. `src/spec.rs` loads `spec/*.json` (`Screen`, `Dialog`, `Item`); `src/ui.rs` has `boxed_list`, `row`, `show_dialog`, `toolbar`, `page`, `open`.

Proposal:

| Mockup piece | Change |
|---|---|
| `kurobayomi/src/data_bridge.rs` (new) | Holds `Rc<AppContext>` handle (area:01) and helpers: `bind_list<T>(sub: Subscription<Vec<T>>, render: impl Fn(&[T]) -> gtk::Widget, target: &gtk::Box)` that clears and refills `target` on every emission (main thread), and `spawn_ui` (section 4) |
| `screens::history()` | Replace the loops with `bind_list(manga_history.watch(&query), render_history_rows, &col)` inside `kind_tabs`; one search entry per tab (spec: "SearchToolbar (separate query per tab)"). `render_history_rows` groups by local date: header text "Today"/"Yesterday"/formatted date (area:42 formatting rules), row via the existing `list_row(Some(thumb), title, "Ch. {n} - {HH:mm}", vec![heart, trash])`. Row click -> `HistoryRecorder`-independent resume: `GetNextChapters` (area:08) then `open_media`. Trash button shows the existing spec dialog `history_remove_manga`/`history_remove_anime`; its switch ("Reset all chapters for this manga" / "Reset all episodes for this anime") selects `reset_by_manga_id` (on) or `reset_by_id` (off) on confirm. Clear button -> dialog `history_remove_everything` -> `delete_all()`, success -> snackbar "History deleted" (`clear_history_completed`). Empty states from the spec notes ("Nothing read recently" / "Nothing watched recently", "No results found" while searching) |
| `screens::updates()` | Same with `MangaUpdatesRepo::watch_recent(three_months_ago_ms(now))`; day headers by local date of `date_fetch`; "Library last updated" line from `__APP_STATE_library_update_last_timestamp` (area:26) |
| Browse (area:40/41, not in the mockup yet) | `SourceRepo::pager` + `RemotePager`; the grid calls `append()` within 25 items of the end |
| Extension store/repo screens (`spec/settings.json` ids `settings_anime_extension_stores`, `settings_manga_extension_repos`; `spec/entries.json` id `extension_repos`) | Replace the generic spec renderer with two custom builders (list of cards + FAB + dialogs) backed by `ExtensionStoreRepo`/`ExtensionRepoRepo`; dialog texts stay the spec/`strings.xml` texts quoted in `p1-behavior.md` section 12 |
| Custom buttons screen (`settings_player_custom_buttons`) | Backed by `CustomButtonRepo::watch_all()` with drag-reorder via `reorder(id, new_index)`; editor dialog fields from the strings in the topic file |
| `spec/SCHEMA.md` | No schema change; the data comes from repositories, not the spec. The spec remains the source of labels, dialog texts and layout notes |

## 8. Android API -> Linux/Rust mapping (this area)

| Android/Kotlin facility | Used for | Rust/Linux replacement |
|---|---|---|
| SQLDelight + `AndroidSqliteDriver` + Requery SQLite | storage | `rusqlite` (bundled) |
| Kotlin coroutines `Dispatchers.IO`, `withContext`, `Flow`, `StateFlow`, `collectLatest`, `combine` | async + reactive | `tokio::task::spawn_blocking`, `tokio::sync::{watch, broadcast}`, `futures::Stream`, hand-written `combine_latest` |
| AndroidX Paging3 | browse paging | `RemotePager` + UI prefetch rule (25 items) |
| `java.util.Date` | history timestamps | `i64` ms |
| kotlinx.serialization JSON / ProtoBuf / okio gzip | store indexes | `serde_json` / `prost` / `flate2` |
| OkHttp `HttpUrl` | repo URL normalisation | `url::Url` |
| OkHttp client + `awaitSuccess` | store/release fetch | area:12 `HttpClient` (reqwest) |
| logcat | logging | `tracing` |
| Injekt singletons | wiring | `AppContext` fields |
| Android `Build.SUPPORTED_ABIS` | APK pick | `std::env::consts::ARCH` + asset name tags |
| Android notification | update prompt | `Notifier` trait |
| `MPVLib.command` | custom buttons | libmpv `mpv_command` through area:45's wrapper |

## 9. Pitfalls

1. `update_hook` runs on the writer thread while it holds SQLite's mutex: the hook body must only touch the dirty set (no queries, no allocation beyond the set insert).
2. A `watch` closure must be pure and idempotent: it may run many times and from different pooled connections.
3. Do not hold the `Subscription` task alive after a GTK widget is destroyed: tie `Subscription` lifetime to the widget by storing it in the widget's `connect_destroy` closure (drop on destroy).
4. SQLite `LIKE` is case-insensitive only for ASCII by default; the optional `kuro_fold` function must be registered on every pooled connection, not only the writer.
5. Do not use `INSERT OR REPLACE` where cascades matter (it deletes the row and fires `ON DELETE CASCADE` on children): the track table's `ON CONFLICT REPLACE` is part of the schema and intentionally preserved; history uses `ON CONFLICT DO UPDATE` (no delete).
6. Keep `Db` clones cheap (`Arc`) and pass them to repositories; never open a second connection to the same file with different PRAGMAs.
7. `date_fetch`/`date_added` are milliseconds. A seconds value written by a bug would make Updates empty; the fixture builder rejects any `date_added`/`date_fetch` value between 1 and 10^11 (0 is legal: it means not favourited / unknown) as "probably seconds".
