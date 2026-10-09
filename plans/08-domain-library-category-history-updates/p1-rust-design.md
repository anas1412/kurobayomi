# Area 08 — Domain: library, category, history, updates — Rust design

Everything here is Proposal unless marked OBSERVED. It implements the contract of `p1-behavior.md` and the formats of `p1-data-and-config.md`, follows the cross-platform rules of `plans/addenda/requirements-cross-platform.md` (no `cfg(target_os)` in this crate) and the crate split announced by area:01-foundation-app-bootstrap (`kuro-core`, `kuro-platform`, `kuro-testkit`; `kuro-data`, `kuro-domain`, `kuro-source`, `kuro-i18n` created by their areas) and area:02-preferences-system (`kuro-prefs`).

## 1. Crate and module layout

New workspace crate `crates/kuro-domain` (no GTK, no `cfg(target_os)`, unit-testable headless). This area owns the following modules; sibling areas add their own (`entries`, `items`, `track`, `download`, ...).

```
crates/kuro-domain/src/
  lib.rs                      pub mod category; pub mod history; pub mod library; pub mod updates; pub mod kind; pub mod error;
  kind.rs                     EntryKind { Manga, Anime }, table-name tables, ItemLabels (chapter/episode wording)
  error.rs                    DomainError (thiserror)
  category/
    model.rs                  Category, CategoryUpdate, SYSTEM_CATEGORY_ID
    repo.rs                   CategoryRepo (SQL, concrete struct over kuro_db::Db)
    read.rs                   all/visible/by_entry, subscribe_*
    write.rs                  create/rename/toggle_hidden/move_before/delete (+ pref cleanup hook)
    membership.rs             set_entry_categories, resolve_add_target, bulk_change
    sort_mode.rs              set_sort_mode, reset_flags, initial_flags
  library/
    flags.rs                  merge/contains, masks
    sort.rs                   SortType, Direction, LibrarySort, codecs
    display_mode.rs           LibraryDisplayMode + codec
    model.rs                  LibraryRow (LibraryManga/LibraryAnime), derived counts
    repo.rs                   LibraryRepo::all(), subscribe()
    pipeline.rs               build_library: group -> filter -> sort -> search
    search.rs                 matches(query, item, ctx)
    selection.rs              Selection
    prefs.rs                  LibraryPrefs definition (macro invocation; re-exported by kuro-prefs)
    restrictions.rs           DEVICE_* / ENTRY_* constants, candidate selection helper
    service.rs                LibraryService (tokio task, watch channel)
  history/
    model.rs                  HistoryRow, HistoryWithRelations, HistoryUpdate
    repo.rs                   HistoryRepo (upsert, list(query), latest, by_entry, reset_one, reset_entry, clear_all, total_duration)
    recorder.rs               HistoryRecorder (session timer, throttle, incognito gate)
    next_item.rs              next_items(...) for chapters and episodes
    view_model.rs             date separators, resume, remove actions
  updates/
    model.rs                  UpdateRow
    repo.rs                   UpdatesRepo
    service.rs                UpdatesService + counters
    window.rs                 three_months_ago
```

Dependencies (all stable, no `unsafe`):

| Crate | Where | Why (and what it replaces) |
|---|---|---|
| `kuro-db` (area:03/04/05/06) | everywhere | `Db` handle, `read`/`write` closures on rusqlite connections, `ChangeBus` (replaces SQLDelight `subscribeToList` and `AndroidMangaDatabaseHandler`) |
| `kuro-prefs` (area:02) | prefs | typed `LibraryPrefs`, change channels (replaces `PreferenceStore` + `Preference.changes()` flows) |
| `tokio` (`sync`, `rt`, `time`) | services | `watch` channels (replace `Flow`), `Mutex` (replaces `kotlinx.coroutines.sync.Mutex`), `spawn`/`spawn_blocking` (replace `withNonCancellableContext`/`Dispatchers.IO`) |
| `rusqlite` | via `kuro-db` | statements stay SQL text under `crates/kuro-db/sql/{manga,anime}/` for views; this crate owns its CRUD statements in `include_str!` files `crates/kuro-domain/sql/*.sql` |
| `icu_collator` + `icu_locale` (ICU4X, pure Rust) | library sort | replaces `java.text.Collator` at PRIMARY strength; `CollatorOptions { strength: Some(Strength::Primary) }`, locale from `LC_COLLATE`/`LANG` via `kuro-platform::Locale` |
| `rand` + `rand_chacha` | random sort | seeded deterministic shuffle replacing `kotlin.random.Random(seed)` |
| `time` (`formatting`, `local-offset`, `macros`) | dates | replaces `java.time`/`Date`/`ZonedDateTime` (also used by area:01) |
| `thiserror`, `tracing` | errors/logs | replace the `logcat(LogPriority.ERROR, e)` pattern |
| `serde` | prefs enums | only for enum codecs that must keep Kotlin spellings |

No new dependency is introduced for flags or comparators (plain integers and `slice::sort_by`).

Ponytail notes (simplifications, with their ceiling): repositories are concrete structs, not traits (one implementation each; tests use an in-memory SQLite database via `Db::in_memory()`); the only trait seams are the three data sources the pipeline cannot own (`DownloadIndex`, `SourceLookup`, `TrackScores`). Upgrade path: if a second backend ever appears, extract a trait at that point.

## 2. Handling manga/anime twins

One runtime enum, not generics, because both kinds share SQL shapes and differ only in table/column names and wording:

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum EntryKind { Manga, Anime }

impl EntryKind {
    pub const ALL: [EntryKind; 2] = [EntryKind::Manga, EntryKind::Anime];
    pub fn db(self) -> DbKind { match self { Self::Manga => DbKind::Manga, Self::Anime => DbKind::Anime } }
    pub fn membership_table(self) -> &'static str { match self { Self::Manga => "mangas_categories", Self::Anime => "animes_categories" } }
    pub fn entry_fk(self) -> &'static str { match self { Self::Manga => "manga_id", Self::Anime => "anime_id" } }
    pub fn history_table(self) -> &'static str { match self { Self::Manga => "history", Self::Anime => "animehistory" } }
    pub fn item_fk(self) -> &'static str { match self { Self::Manga => "chapter_id", Self::Anime => "episode_id" } }
    pub fn timestamp_col(self) -> &'static str { match self { Self::Manga => "last_read", Self::Anime => "last_seen" } }
}
```

Statements that differ only by identifiers are produced with `format!` from these accessors **once at startup** into `&'static str` (use `OnceLock<Statements>`), never per call and never from user input (no SQL injection surface: the identifiers come from the enum). Placeholders (`?1`) carry all values.

Sort types: one enum for both kinds (`SortType::LastActivity` = LastRead/LastSeen, `UnreadCount` = UnreadCount/UnseenCount, `TotalItems`, `LatestItem`, `ItemFetchDate`, plus `AiringTime` valid only for `Anime`); `fn token(self, kind) -> &'static str` returns the Kotlin string token (`LAST_READ` vs `LAST_SEEN`, `LAST_MANGA_UPDATE` vs `LAST_ANIME_UPDATE`, ...), `fn from_token(kind, &str) -> Option<Self>`; `fn valid_for(self, kind) -> bool` (AiringTime false for Manga). Flag numbers are shared (`p1-topic-flag-algebra-and-sort-modes.md`).

## 3. Key types (signatures)

```rust
// category/model.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
impl Category { pub fn is_system(&self) -> bool { self.id == SYSTEM_CATEGORY_ID } }
#[derive(Clone, Debug, Default)]
pub struct CategoryUpdate { pub id: i64, pub name: Option<String>, pub order: Option<i64>, pub flags: Option<i64>, pub hidden: Option<bool> }

// category/repo.rs   (all methods are `async fn` over Db::read / Db::write; writes never cancel mid-transaction)
pub struct CategoryRepo { db: Db, kind: EntryKind, stmts: &'static CategoryStatements }
impl CategoryRepo {
    pub async fn get(&self, id: i64) -> Result<Option<Category>, DomainError>;
    pub async fn all(&self) -> Result<Vec<Category>, DomainError>;                    // ORDER BY sort, _id
    pub async fn visible(&self) -> Result<Vec<Category>, DomainError>;
    pub async fn by_entry(&self, entry_id: i64, visible_only: bool) -> Result<Vec<Category>, DomainError>;  // ORDER BY C.sort, C._id
    pub async fn insert(&self, name: &str, order: i64, flags: i64) -> Result<i64, DomainError>;            // returns the new id
    pub async fn update_partial(&self, u: &CategoryUpdate) -> Result<(), DomainError>;
    pub async fn update_partial_many(&self, us: &[CategoryUpdate]) -> Result<(), DomainError>;             // one transaction
    pub async fn update_all_flags(&self, flags: i64) -> Result<(), DomainError>;
    pub async fn delete(&self, id: i64) -> Result<(), DomainError>;                                         // system row -> DomainError::SystemCategory
    pub fn watch_all(&self) -> watch::Receiver<Arc<Vec<Category>>>;       // re-queried on change of `categories`
    pub fn watch_visible(&self) -> watch::Receiver<Arc<Vec<Category>>>;
}

// category/write.rs
pub struct CategoryService { repo: CategoryRepo, prefs: Arc<Prefs>, reorder_lock: tokio::sync::Mutex<()> }
impl CategoryService {
    pub async fn create(&self, name: &str) -> Result<Category, DomainError>;       // behaviour 1.3 + validation
    pub async fn rename(&self, id: i64, name: &str) -> Result<(), DomainError>;
    pub async fn toggle_hidden(&self, id: i64) -> Result<(), DomainError>;
    pub async fn move_before(&self, id: i64, before: Option<i64>) -> Result<(), DomainError>;   // None = end; reorder_lock held
    pub async fn delete(&self, id: i64) -> Result<(), DomainError>;                // behaviour 1.6, strips the 5 pref sets
}

// library/sort.rs
#[derive(Copy, Clone, Eq, PartialEq, Debug)] pub struct LibrarySort { pub ty: SortType, pub dir: Direction }
impl LibrarySort {
    pub const DEFAULT: Self = Self { ty: SortType::Alphabetical, dir: Direction::Ascending };
    pub fn flag(self) -> i64;                                  // type + direction
    pub fn from_flags(kind: EntryKind, flags: Option<i64>) -> Self;     // valueOf; AiringTime unknown to Manga -> Alphabetical
    pub fn serialize(self, kind: EntryKind) -> String;         // "LAST_READ,ASCENDING"
    pub fn deserialize(kind: EntryKind, s: &str) -> Self;      // lenient
    pub fn is_ascending(self) -> bool;
}

// library/model.rs
pub struct LibraryRow {                       // one per (favourite entry, category); mirrors LibraryManga / LibraryAnime
    pub entry: EntrySummary,                  // the fields area 07 exposes: id, source, title, author, artist, description, genre, status, last_update, date_added, fetch_interval, viewer_flags, thumbnail_url, cover_last_modified, favorite
    pub category: i64,
    pub total: i64, pub done: i64,            // totalChapters/readCount | totalCount/seenCount
    pub bookmarks: i64, pub fillermarks: i64, // fillermarks always 0 for manga
    pub latest_upload: i64, pub item_fetched_at: i64, pub last_activity: i64,
}
impl LibraryRow { pub fn pending(&self) -> i64 { self.total - self.done } pub fn has_started(&self) -> bool { self.done > 0 } pub fn has_bookmarks(&self) -> bool { self.bookmarks > 0 } }

// history/model.rs
pub struct HistoryWithRelations {
    pub id: i64, pub item_id: i64, pub entry_id: i64, pub title: String, pub item_number: f64,
    pub at: Option<EpochMs>, pub duration_ms: i64 /* 0 for anime */, pub cover: CoverKey,
}
pub struct HistoryUpdate { pub item_id: i64, pub at: EpochMs, pub session_ms: i64 /* 0 for anime */ }

// updates/model.rs
pub struct UpdateRow {
    pub entry_id: i64, pub entry_title: String, pub item_id: i64, pub item_name: String, pub scanlator: Option<String>,
    pub done: bool, pub bookmark: bool, pub fillermark: bool /* anime */,
    pub progress: i64 /* last_page_read | last_second_seen */, pub total_seconds: i64 /* anime */,
    pub source_id: i64, pub date_fetch: EpochMs, pub cover: CoverKey,
}
```

Seam traits consumed by the pipeline:

```rust
pub trait DownloadIndex: Send + Sync { fn count(&self, kind: EntryKind, entry_id: i64) -> u32; }   // area:23/24
pub trait SourceLookup: Send + Sync {
    fn lang(&self, source_id: i64) -> String;                                                       // "" when unknown
    fn display_name(&self, source_id: i64) -> String;                                               // getNameForMangaInfo()
    fn is_local(&self, source_id: i64) -> bool { source_id == 0 }
}
pub trait TrackScores: Send + Sync {
    fn tracker_ids(&self, entry_id: i64) -> &[i64];
    fn mean_10pt(&self, entry_id: i64) -> Option<f64>;      // None when no tracks / none scored; never NaN
}
```

## 4. Pipeline and services

`library::pipeline::build_library` is a pure function (signature in `p1-topic-library-pipeline.md` section 2). `LibraryService` per kind:

```rust
pub struct LibraryService { /* tokio task handle */ }
impl LibraryService {
    pub fn spawn(kind: EntryKind, db: Db, prefs: Arc<Prefs>, ext: Arc<LibraryExternals>) -> Self;
    pub fn state(&self) -> watch::Receiver<Arc<LibraryState>>;       // Loading | Ready(Vec<LibraryPage>) | Error(String)
    pub fn set_query(&self, q: Option<String>);                      // UI debounces 250 ms before calling
}
```

The task selects over `change_bus.subscribe(&tables)`, `prefs.library.changes()`, `ext.downloads.changed()`, `ext.tracks.changed()`, and a `mpsc` of search queries; on any event it waits 30 ms, drains further events, loads rows + categories with one `Db::read` closure (one consistent snapshot, avoiding the torn read Kotlin could see between the library flow and the category flow), runs `build_library` in `spawn_blocking`, and `send_replace`s the new `Arc<LibraryState>`.

History, updates and categories use `watch` receivers fed by `ChangeBus` (`categories`, `mangas_categories` for category-by-entry; `history`/`animehistory`, `chapters`/`episodes`, `mangas`/`animes` for the history list; `chapters`/`episodes`, `mangas`/`animes` for updates). The re-query closure is the same SQL as the repository method.

## 5. Error handling

```rust
#[derive(thiserror::Error, Debug)]
pub enum DomainError {
    #[error("database error: {0}")] Db(#[from] kuro_db::DbError),
    #[error("invalid category name")] InvalidName,
    #[error("a category with this name already exists")] DuplicateName,
    #[error("category not found")] NotFound,
    #[error("the default category cannot be changed")] SystemCategory,
    #[error("background task failed: {0}")] Join(#[from] tokio::task::JoinError),
}
```

UI mapping: `Db` -> toast text of `MR.strings.internal_error` ("InternalError: Check crash logs for further information") and `tracing::error!` with the source chain; `DuplicateName` -> inline field error "A category with this name already exists!"; `InvalidName` -> disabled confirm. Operations that Kotlin swallowed (`SetCategories`, history upserts, history reset) return `Result` and the caller logs and continues; the history writes never surface UI errors.

## 6. Non-cancellable writes

Kotlin wraps writes in `NonCancellable` so a closing screen cannot abort them. In Rust a GTK callback typically does `glib::spawn_future_local(async move { svc.delete(id).await })`; dropping that future (window closed) would drop the `.await`. All service write methods therefore do:

```rust
pub async fn delete(&self, id: i64) -> Result<(), DomainError> {
    let this = self.clone();                       // Arc inside
    tokio::spawn(async move { this.delete_inner(id).await }).await.map_err(DomainError::Join)?
}
```
The spawned task is detached from the caller's cancellation (the `JoinHandle` can be dropped without aborting). The same wrapper is used for `HistoryRecorder::flush` so a closing reader window still saves history. `DomainError::Join` (add to the enum) covers task panics.

## 7. Threading model

- Tokio multi-thread runtime owned by `kuro-core` (area:01); all DB I/O through `Db::read/write` (rusqlite blocking calls on `spawn_blocking`/dedicated writer thread per area:03/04).
- GTK main loop: only widget updates. Services publish `Arc<T>` snapshots through `tokio::sync::watch`; a helper in the UI crate `bind_watch(rx, move |snap| ...)` runs `glib::spawn_future_local(async move { while rx.changed().await.is_ok() { f(&rx.borrow_and_update()) } })`. `Arc<T>` snapshots are `Send + Sync`; GTK objects never cross threads.
- Reorder: one `tokio::sync::Mutex` per kind (same scope as the Kotlin per-instance `Mutex`).

## 8. Android API to Linux mapping (this area)

| Android/Kotlin | Linux/Rust |
|---|---|
| `kotlinx.coroutines.flow.Flow` from SQLDelight | `tokio::sync::watch` fed by `ChangeBus` |
| `withNonCancellableContext` | detached `tokio::spawn` + await (section 6) |
| `kotlinx.coroutines.sync.Mutex` | `tokio::sync::Mutex` |
| `Injekt.get()` / `app/src/main/java/eu/kanade/domain/DomainModule.kt` factories | explicit `Domain` struct built in `kuro-core::AppContext` (area:01); no service locator |
| `logcat(LogPriority.ERROR, e)` | `tracing::error!(error = ?e, ...)` |
| `Preference<T>.changes()` / `asState` | `kuro-prefs` change `watch`/`broadcast` per key |
| `java.text.Collator` primary | `icu_collator` primary strength |
| `kotlin.random.Random(seed).shuffled` | `ChaCha8Rng` + Fisher-Yates |
| `java.util.Date`, `ZonedDateTime.minusMonths` | `i64` epoch ms, `time::OffsetDateTime` + month arithmetic with day clamp |
| `DateFormat.getTimeInstance(SHORT)`, `DateTimeFormatter.ofLocalizedDate` | `kuro-platform` locale formatting via `icu_datetime` short formats (area:52) |
| `HorizontalPager` + `PrimaryScrollableTabRow` | `gtk::Notebook` (scrollable, tabs-at-top, one page per category) or a `gtk::Stack` + custom tab strip |
| `LazyVerticalGrid` covers | `gtk::GridView` with a `gio::ListStore` per page and a recycling `SignalListItemFactory` |
| `PullRefresh` | toolbar button + F5 (no pull gesture on desktop); optional `GestureDrag` for touch screens |
| `LongPress` haptics | none (desktop); long-press via `GestureLongPress` toggles selection on touch |
| `BackHandler` | `Escape` key: clear selection, then clear search |
| `Toast`/`Snackbar` | `gtk::Overlay` with an in-app toast widget (`Revealer` + label + optional action), no libadwaita `ToastOverlay` |
| `WorkManager` update scheduling | area:26 (systemd user timer / in-process timer) |
| `presentation-widget` Glance widgets | dropped (T-08a-038) |

Desktop adaptations of library UI inputs (all in `kuro-ui`, driven by this crate's pure types): columns `0 = auto` computes `max(2, width / 160 px)` per window width, replacing portrait/landscape; the two orientation keys are both kept in the registry and the desktop uses the portrait key when the window is narrower than 900 px and the landscape key otherwise (keeps backups meaningful); `Ctrl+click` toggles selection, `Shift+click` = `Selection::range`, `Ctrl+A` = select all in the page, `Ctrl+Shift+A`/`Ctrl+I` = invert, `Escape` clears, `/` or `Ctrl+F` focuses search, `F5` refreshes the active category, `Shift+F5` global update, `Ctrl+PgUp/PgDn` and `Alt+Left/Right` switch category tabs, middle click on a cover opens the entry's context menu, `Delete` on a history row opens the remove dialog.

## 9. Integration with the existing mockup (`/home/blackbox/kurobayomi/src`)

OBSERVED mockup state: `src/screens.rs` builds every page from hard-coded sample data (`SAMPLE` count, `title_n(i)`), `src/ui.rs` provides widgets (`linked`, `row`, `menu_button`, `show_sheet`, `show_dialog`, `generic`), and `spec/*.json` (loaded by `src/spec.rs`) carries labels, sheets and dialogs extracted from Aniyomi. Specific functions to rewire:

| Mockup function (`src/screens.rs`) | Today | Becomes |
|---|---|---|
| `libraries()` / `library(id, anime)` (lines 141-275) | tab labels from `spec.tabs`, `FlowBox` of `SAMPLE` fake covers, filter func on label text, right-click/long-press revealer | page per `LibraryPage` from `LibraryService::state()`; tab strip from `state.pages.map(category.name or "Default")`; cover cards from `LibraryItem` (badges: `unread_count` accent badge, `download_count` osd badge, local/language badges); the `Revealer` action bar stays but drives `Selection` and real actions; `menu_button` entries "Update library", "Update category", "Open random entry" call the services; the filter button opens the existing `show_sheet(manga_library_settings)` whose `tristate`/`sort`/`chips`/`slider`/`switch` items are bound to `LibraryPrefs` and `CategoryService::set_sort_mode` (binding layer: `ui::bind`, area:02) |
| `dialog_of(id, "manga_change_category")` | static dialog from spec | dialog populated from `bulk_change::initial_states()` with the tri-state checkboxes; buttons Edit/Cancel/OK |
| `categories()` (lines 404-425) | three fake rows | list from `CategoryService::watch_all`, drag-and-drop via `gtk::DragSource`/`DropTarget` on rows calling `move_before`, rename/hide/delete buttons calling the service, FAB-like "Add" button |
| `history()` | fake rows by day | `HistoryViewModel` list with date headers, per-row actions (resume, add to library, remove dialog `history_remove_*`), clear-all dialog `history_remove_everything` |
| `updates()` | fake rows by day | `UpdatesService` list with headers, state dot/bookmark, download button (area:23/24) |
| `kind_tabs` / `kind_stack` | Anime/Manga switch | unchanged; each kind gets its own service instance |

`spec/app.json` entries `manga_library`, `anime_library`, `categories`, `history`, `updates` stay as the source of titles, empty-state texts and dialog texts so the strings keep their Aniyomi wording; strings that need arguments (`delete_category_confirmation` with `%s`) are formatted at the call site.

## 10. Pitfalls

1. SQLite rowid reuse: after deleting the highest category id, a new category receives the same id (no AUTOINCREMENT). Stale ids in preference sets (see Q8) then bind to a different category. The delete path must clean all five sets (fixed in Rust) and restore must remap sets (area:22).
2. `ORDER BY sort` ties (section 1.2 of behaviour): always add `_id` as the secondary key.
3. A library with 3 categories and an entry in each shows the entry three times: count "library size" with `DISTINCT entry id` (Kotlin `fastDistinctBy`), never with row counts.
4. Never compute the Default-category visibility rule from the *visible categories* list; it depends on the presence of rows with `category = 0` in the library rows (behaviour 2.3).
5. Dates are epoch milliseconds everywhere; `0` is a sentinel for "never"/"reset" in `history.last_read`, `lastUpdate`, `nextEpisodeAiringAt`; do not use `Option<i64>` for them in storage (the column is nullable only for history timestamps; read with `COALESCE(last_read, 0)`).
6. Collation keys: build them once per item, not inside the comparator; and lowercase with Unicode `to_lowercase()` (Kotlin `lowercase()` uses the default locale; ignore the Turkish-dotless-i locale special case).
7. `history` upsert must run inside the writer connection with `foreign_keys=ON` (the chapter must exist); a `FOREIGN KEY constraint failed` error is swallowed like Kotlin (logged) when the chapter was deleted while reading.
8. The `watch` channel only keeps the latest value: consumers that need every event (toasts) must use `mpsc`, not `watch` (history "cleared" and "internal error" events use `mpsc::unbounded_channel`, matching Kotlin's `Channel.UNLIMITED`).
9. Do not hold a `watch::Ref` across an `.await`.
