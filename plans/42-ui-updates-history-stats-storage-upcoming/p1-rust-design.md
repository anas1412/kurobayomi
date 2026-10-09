# Area 42 part 1 — Rust design (Updates, History, Stats, Storage)

Proposal only. Section numbers are referenced from p1-behavior.md (1.6, 1.8 use section 7 and 9) and p1-data-and-config.md (section 3, section 4). Existing mockup: `src/main.rs`, `src/screens.rs` (`updates()` line 277, `history()` line 305, `stats()` line 443, `storage()` after line 470, `stat_card` line 426), `src/spec.rs` (spec loader), `spec/app.json`, `spec/settings.json`. `Cargo.toml` currently has `gtk4 = 0.11.5` (feature `v4_16`), `serde`, `serde_json` only. No database code exists yet.

Stack constraints from the project brief: stable Rust, gtk4-rs 0.11 on GTK 4.16+, vanilla GTK (no libadwaita), tokio, rusqlite, reqwest, serde.

---

## 1. Crate and module layout

Workspace layout follows area:01-foundation-app-bootstrap / T-08a-001 (`crates/kuro-*`). Area 42 adds modules; it does not own crates.

```
crates/kuro-domain/src/
  history/mod.rs          HistoryRow, EntryKind, next-item algorithm (GetNextChapters/GetNextEpisodes port)
  history/date_group.rs   insert-separator grouping by local date
  updates/mod.rs          UpdateRow, UpdateWindow, UpdateItem<T>, DownloadSnapshot, grouping
  updates/selection.rs    SelectionState (range select engine, positions[0..1])
  updates/actions.rs      bookmark/fillermark/mark-read planning (filterNot + update batch)
  stats/mod.rs            StatsSnapshot, MangaStats, AnimeStats, TrackerStats
  stats/mean.rs           mean-of-means over scored tracks
  stats/global_update.rs  global-update count with category include/exclude + restrictions
  storage/mod.rs          StorageRow, StorageSnapshot, sort by size desc
  storage/color.rs        deterministic per-entry colour
  storage/size.rs         Long.toSize() port (SI 1000)
  format/mod.rs           duration_string, progress_mmss, relative_date, relative_time_span
crates/kuro-data/src/
  history.rs              rusqlite queries against historyView / animehistoryView
  updates.rs              rusqlite queries against updatesView / animeupdatesView
  stats_queries.rs        aggregate SQL used by stats (one query per kind, see section 4.4)
  storage_queries.rs      library rows for storage + categories
src/ (GTK, root crate for now; move to crates/kuro-ui when the workspace split happens)
  ui/tabbed.rs            TabbedScreen port (header bar + search entry + sub-tab strip + stack)
  ui/history/mod.rs       HistoriesTab port (anime + manga sub-tabs)
  ui/history/list.rs      date-grouped ListView (headers + rows)
  ui/history/row.rs       96 px row widget
  ui/history/dialogs.rs   HistoryDeleteDialog, HistoryDeleteAllDialog
  ui/updates/mod.rs       UpdatesTab port
  ui/updates/list.rs      grouped list with selection and pull-to-refresh replacement
  ui/updates/row.rs       56 px row (manga + anime variants)
  ui/updates/bottom_bar.rs selection action bar
  ui/updates/dialogs.rs   UpdatesDeleteConfirmationDialog
  ui/stats/mod.rs         StatsTab port (two pages)
  ui/stats/tile.rs        StatTile (value + caption + optional icon)
  ui/stats/card.rs        SectionCard port
  ui/storage/mod.rs       StorageTab port
  ui/storage/donut.rs     half-donut DrawingArea (cairo)
  ui/storage/row.rs       storage row + delete confirm
  ui/storage/category.rs  category DropDown with All/Default mapping
  ui/shared/selection.rs  generic gesture->selection adapter (long press, shift-range, ctrl-toggle)
  ui/shared/relative.rs   glue to format::relative_date with preferences
```

Dependency direction: `kuro-domain` has no GTK and no rusqlite; `kuro-data` depends on `kuro-domain` and `rusqlite`; the root crate depends on both. Unit tests for every module in `kuro-domain` run without a display.

---

## 2. Threading and async model

Three execution contexts:

1. **GTK main thread** (`glib::MainContext::default()`): all widgets, all view-model state mutation, all `Channel` receives. Use `glib::MainContext::default().spawn_local(async move { ... })` for UI tasks.
2. **Database thread(s)**: rusqlite is synchronous. One dedicated OS thread per database file (`manga.db`, `anime.db`, names owned by area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations) owns the write `Connection`; reads use a small pool of read-only connections (`rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY`). Requests are sent over `async_channel::Sender<DbRequest>` with a `oneshot` reply. No `tokio::spawn_blocking` in the hot path (keeps one ordering guarantee per DB).
3. **tokio runtime** (multi-thread, 2 workers) for network-free background work that is async: library update triggers (area:26-library-update-jobs), download manager (area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared), tracker calls (stats mean score, area:27-tracking-core, area:28-tracker-anilist, area:29-tracker-myanimelist, area:30-tracker-kitsu, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin).

Reactive queries (replace `subscribeToList`): each DB connection installs `Connection::update_hook` (rusqlite feature `hooks`) and publishes a `TableChanged { table: &'static str }` on a `tokio::sync::broadcast::Sender<TableChanged>` (capacity 256). The view model re-runs its query when a relevant table name arrives, debounced by 50 ms (coalesce bursts from batch updates). Initial load runs once immediately.

```rust
// crates/kuro-data/src/reactive.rs
pub struct TableChanges(tokio::sync::broadcast::Receiver<TableChanged>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableChanged(pub &'static str);

pub fn watch_tables(
    rx: tokio::sync::broadcast::Receiver<TableChanged>,
    wanted: &'static [&'static str],
) -> impl futures_core::Stream<Item = ()> { /* filter + 50 ms coalesce */ }
```

Download-cache change fan-out (fixes the single-consumer hazard in behavior 6.8 and 9.1): the Kotlin `Channel.receiveAsFlow()` delivers each element to ONE collector. Kurobayomi must use broadcast semantics:

```rust
// crates/kuro-domain/src/download/cache.rs (area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared owns the type; this area is a consumer)
pub struct CacheSignals {
    pub changed: tokio::sync::broadcast::Sender<()>,          // every subscriber sees every change
    pub initializing: tokio::sync::watch::Sender<bool>,       // latest value always readable
}
impl CacheSignals {
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<()> { self.changed.subscribe() }
}
```

Each view model subscribes separately. A `watch` value is read with `borrow()`, so a late subscriber still sees "initializing = false" immediately (this removes the storage page hang).

Cancellation: view models hold a `CancellationToken` (from `tokio_util`) cancelled on widget destroy. Bulk download/delete operations from updates and storage run as detached tasks (`tokio::spawn`) that ignore cancellation once started (equivalent of `launchNonCancellable`, behavior section 8).

Ordering: all writes for one entry (e.g. mark-read then download) are sent to the same DB thread, so they keep submission order.

---

## 3. Key types and signatures

### 3.1 Shared entry kind

```rust
// crates/kuro-domain/src/kind.rs (area:01-foundation-app-bootstrap, area:08-domain-library-category-history-updates owns; shown for reference)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind { Manga, Anime }

impl EntryKind {
    pub fn history_table(self) -> &'static str { match self { Self::Manga => "history", Self::Anime => "animehistory" } }
    pub fn timestamp_col(self) -> &'static str { match self { Self::Manga => "last_read", Self::Anime => "last_seen" } }
}
```

### 3.2 History

```rust
// crates/kuro-domain/src/history/mod.rs
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq)]
pub struct CoverRef {
    pub entry_id: i64,
    pub source_id: i64,
    pub is_favorite: bool,
    pub thumbnail_url: Option<String>,
    pub last_modified_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryRow {
    pub history_id: i64,
    pub entry_id: i64,        // mangaId or animeId
    pub unit_id: i64,         // chapterId or episodeId
    pub title: String,
    pub unit_number: f64,     // chapterNumber / episodeNumber; -1.0 = unknown
    pub at: Option<DateTime<Utc>>, // readAt / seenAt (None only if column NULL)
    pub read_duration_ms: i64,     // manga only; 0 for anime
    pub cover: CoverRef,
}

/// Date-grouped list element (behavior 1.3).
#[derive(Clone, Debug, PartialEq)]
pub enum HistoryListItem {
    Header(chrono::NaiveDate),
    Item(HistoryRow),
}

pub fn group_by_local_date(rows: &[HistoryRow]) -> Vec<HistoryListItem>; // ported insertSeparators semantics

/// Port of GetNextChapters / GetNextEpisodes with onlyUnread = false (behavior 1.7).
/// `ordered` must already be sorted ascending by the entry's unit sort.
pub fn next_units_from(ordered: &[UnitRef], from_unit_id: i64) -> Vec<UnitRef>;

#[derive(Clone, Debug, PartialEq)]
pub struct UnitRef { pub id: i64, pub read: bool /* seen for anime */ }
```

`next_units_from` contract: find `idx = position(id == from)`. If `idx == None` → return `ordered[1..]` (preserves the Kotlin result, see EC-42-H-05) or, with the fix (recommended, T-42a-015), return `ordered` from the first unread unit. If found: `tail = ordered[idx..]`; if `!tail[0].read` return `tail` else `tail[1..]`.

### 3.3 Updates

```rust
// crates/kuro-domain/src/updates/mod.rs
pub struct UpdateWindow { pub upload_after_ms: i64, pub limit: u32 }   // { now - 3 months, 500 }
impl UpdateWindow {
    pub fn default_at(now: chrono::DateTime<chrono::Local>) -> Self {
        let after = now.checked_sub_months(chrono::Months::new(3)).expect("3 months back");
        Self { upload_after_ms: after.timestamp_millis(), limit: 500 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateRow {
    pub entry_id: i64,
    pub entry_title: String,
    pub unit_id: i64,
    pub unit_name: String,
    pub scanlator: Option<String>,
    pub read_or_seen: bool,       // chapters.read / episodes.seen
    pub bookmark: bool,
    pub fillermark: bool,         // anime only; false for manga
    pub progress: i64,            // manga: last_page_read (0-based); anime: last_second_seen (ms)
    pub total_ms: i64,            // anime: total_seconds (ms); manga: 0
    pub source_id: i64,
    pub date_fetch_ms: i64,       // grouping key
    pub cover: CoverRef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadStatus { NotDownloaded, Queued, Downloading, Downloaded, Error }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DownloadSnapshot { pub status: DownloadStatus, pub progress: i32 }

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateItem { pub row: UpdateRow, pub download: DownloadSnapshot, pub selected: bool }
```

`DownloadStatus` numeric values are the Kotlin enum ordinals `0..4` (`NOT_DOWNLOADED`..`ERROR`) and must be preserved if persisted (they are not persisted in this area).

Selection engine (port of behavior 2.5):

```rust
// crates/kuro-domain/src/updates/selection.rs
#[derive(Default, Debug)]
pub struct Selection {
    selected_ids: std::collections::HashSet<i64>,
    /// Indices into the current item list: [first, last]; -1 means unset.
    bounds: [isize; 2],
}

impl Selection {
    pub fn new() -> Self { Self { selected_ids: Default::default(), bounds: [-1, -1] } }

    /// `items` are in display order. Returns the indices whose state changed.
    pub fn toggle(
        &mut self,
        items: &mut [UpdateItem],
        unit_id: i64,
        selected: bool,
        user_initiated: bool,
        from_long_press: bool,
    ) -> Vec<usize>;

    pub fn toggle_all(&mut self, items: &mut [UpdateItem], selected: bool);
    pub fn invert(&mut self, items: &mut [UpdateItem]);
    pub fn is_selection_mode(items: &[UpdateItem]) -> bool { items.iter().any(|i| i.selected) }
}
```

`toggle` must reproduce behavior 2.5 exactly: `firstSelection` is computed before mutation; the long-press branch extends `bounds` outward and selects the gap (exclusive of the existing bound); the tap branch updates bounds only when the toggled row is at a bound (deselect) or outside the bounds (select). `toggle_all` and `invert` reset bounds to `[-1, -1]`.

### 3.4 Stats

```rust
// crates/kuro-domain/src/stats/mod.rs
#[derive(Clone, Debug, PartialEq)]
pub struct MangaStats {
    pub overview: MangaOverview,
    pub titles: Titles,
    pub chapters: Counts,        // total, read, downloaded
    pub trackers: TrackerStats,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AnimeStats {
    pub overview: AnimeOverview,
    pub titles: Titles,
    pub episodes: Counts,
    pub trackers: TrackerStats,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MangaOverview { pub in_library: u32, pub completed: u32, pub read_duration_ms: i64 }
#[derive(Clone, Debug, PartialEq)]
pub struct AnimeOverview { pub in_library: u32, pub completed: u32, pub watched_duration_ms: i64 }
#[derive(Clone, Debug, PartialEq)]
pub struct Titles { pub global_update_count: u32, pub started: u32, pub local: u32 }
#[derive(Clone, Debug, PartialEq)]
pub struct Counts { pub total: u32, pub read: u32, pub downloaded: u32 }
#[derive(Clone, Debug, PartialEq)]
pub struct TrackerStats { pub tracked_titles: u32, pub mean_score: Option<f64>, pub tracker_count: u32 }

/// Library row as needed by stats (both kinds); produced by kuro-data.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryEntryStat {
    pub entry_id: i64,
    pub category_id: i64,
    pub status: i64,           // SManga/SAnime status; COMPLETED = 2
    pub total_units: i64,      // totalChapters / totalCount
    pub read_units: i64,       // readCount / seenCount
    pub is_local: bool,
}
impl LibraryEntryStat {
    pub fn unread(&self) -> i64 { self.total_units - self.read_units }
    pub fn has_started(&self) -> bool { self.read_units > 0 }
}

pub const COMPLETED_STATUS: i64 = 2;

pub fn global_update_count(
    rows: &[LibraryEntryStat],
    include_cats: &std::collections::HashSet<i64>,
    exclude_cats: &std::collections::HashSet<i64>,
    restrictions: &std::collections::HashSet<RestrictionToken>,
) -> u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RestrictionToken { NonCompleted /* manga_ongoing */, HasUnviewed /* manga_fully_read */, NonViewed /* manga_started */, OutsideReleasePeriod /* manga_outside_release_period, NOT evaluated here */ }

/// mean of per-entry means of 10-point scores; None when no entry has a score.
pub fn mean_of_entry_means(per_entry_scores: &[Vec<f64>]) -> Option<f64>;
```

`mean_of_entry_means` must skip empty entries (Kotlin: `tracks.map(get10PointScore).average()` on a non-empty list, then `filter { !isNaN }`), and return `None` for an empty input (Kotlin NaN).

Stats computation runs on the tokio runtime; the GTK page shows a spinner until `Ready`.

### 3.5 Storage

```rust
// crates/kuro-domain/src/storage/mod.rs
#[derive(Clone, Debug, PartialEq)]
pub struct StorageRow {
    pub entry_id: i64,
    pub title: String,
    pub size_bytes: u64,
    pub thumbnail_url: Option<String>,
    pub entries_count: u32,
    pub color: [u8; 3],        // see storage/color.rs
    pub category_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StorageCategory { pub id: i64, pub name: String, pub hidden: bool }

pub const ALL_CATEGORY_ID: i64 = -1;       // AllCategory
pub const UNCATEGORIZED_ID: i64 = 0;       // Category.UNCATEGORIZED_ID

pub fn build_snapshot(
    libraries: &[(i64 /*entry id*/, i64 /*category id*/, String, Option<String>)],
    categories: &[StorageCategory],
    selected: i64,
    sizes: &impl Fn(i64) -> (u64, u32),   // (bytes, count) per entry, from the download cache
) -> Vec<StorageRow>;
```

`build_snapshot` must: dedupe by `entry_id` (first occurrence), filter by `selected` (ALL keeps only rows whose category is in `categories`; otherwise equality), compute colour, and sort by `size_bytes` descending with a stable sort (equal sizes keep library order).

```rust
// crates/kuro-domain/src/storage/size.rs
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1000; const MB: u64 = KB * KB; const GB: u64 = MB * KB;
    if bytes >= GB { format!("{:.2} GB", bytes as f32 / GB as f32) }
    else if bytes >= MB { format!("{:.2} MB", bytes as f32 / MB as f32) }
    else if bytes >= KB { format!("{:.2} KB", bytes as f32 / KB as f32) }
    else { format!("{bytes} B") }
}
```

Note: the Kotlin uses `Float` division (`this.toFloat() / gb`); `f32` reproduces this exactly. Use `f32` on purpose.

Donut geometry (port of `CumulativeStorage.kt`):

```rust
// crates/kuro-domain/src/storage/donut.rs (pure geometry, testable)
pub struct Arc { pub start_deg: f64, pub sweep_deg: f64, pub color: [u8; 3] }
pub fn donut_arcs(rows: &[StorageRow]) -> Vec<Arc> {
    // total = sum(size_bytes) as f64; if total == 0 -> empty
    // angle_i = size_i / total * 180; start accumulates; rotated by 180 when drawing
}
```

Drawing (`ui/storage/donut.rs`): cairo `arc(cx, cy, r, start, end)` on a `DrawingArea` with `set_content_width/height` 1:1; line width 15 logical px; `LineCap::Round`; the 180° rotation is applied with `cairo::Context::translate(cx, cy); rotate(PI)` before drawing arcs whose angles are in `[0, PI]`. Center text at `(w/2 - tw/2, h/4 - th/2)` as in Kotlin, layout height `h/2`.

### 3.6 Formatting helpers

```rust
// crates/kuro-domain/src/format/mod.rs
pub fn duration_string(ms: i64, none: &str) -> String;         // behavior 5.4
pub fn progress_mmss(ms: i64) -> String;                       // behavior 2.12 formatProgress
pub fn relative_date(date: chrono::NaiveDate, today: chrono::NaiveDate, relative: bool, absolute: &str) -> RelativeDate;
pub fn relative_time_span(epoch_ms: i64, now_ms: i64) -> RelativeTime; // Never / JustNow / Ago(...)
pub enum RelativeDate { Today, Ago(i64), InDays(i64), Absolute(String) }
pub enum RelativeTime { Never, JustNow, Ago(String) }
```

`duration_string` takes `ms` and reproduces `toComponents` with the four `if` rules of behavior 5.4 and `joined(" ")`, `none` on blank.

`progress_mmss(ms)`: if `ms > 3_600_000` → `format!("{}:{:02}:{:02}", h, m, s)` with `h = ms / 3_600_000`, `m = ms / 60_000 - h * 60`, `s = ms / 1000 - (ms / 60_000) * 60`; else `format!("{}:{:02}", ms / 60_000, ms / 1000 - (ms / 60_000) * 60)`. Use `i64` integer division (matches Kotlin `TimeUnit` truncation for non-negative input; negative input is not produced by the DB).

---

## 4. Repository and SQL plan

### 4.1 History

```rust
// crates/kuro-data/src/history.rs
pub trait HistoryRepo: Send + Sync {
    fn list_manga(&self, query: &str) -> rusqlite::Result<Vec<HistoryRow>>;
    fn list_anime(&self, query: &str) -> rusqlite::Result<Vec<HistoryRow>>;
    fn latest_manga(&self) -> rusqlite::Result<Option<HistoryRow>>;
    fn latest_anime(&self) -> rusqlite::Result<Option<HistoryRow>>;
    fn total_manga_read_ms(&self) -> rusqlite::Result<i64>;
    fn reset_manga_row(&self, history_id: i64) -> rusqlite::Result<()>;
    fn reset_manga_entry(&self, entry_id: i64) -> rusqlite::Result<()>;
    fn delete_all_manga(&self) -> rusqlite::Result<usize>;
    fn reset_anime_row(&self, history_id: i64) -> rusqlite::Result<()>;
    fn reset_anime_entry(&self, entry_id: i64) -> rusqlite::Result<()>;
    fn delete_all_anime(&self) -> rusqlite::Result<usize>;
}
```

Manga list SQL (port of `history.sq`, with the escaping fix):

```sql
SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified,
       chapterNumber, readAt, readDuration
FROM historyView
WHERE readAt > 0
  AND maxReadAtChapterId = chapterId
  AND instr(lower(title), lower(?1)) > 0     -- replaces LIKE '%' || :query || '%'
ORDER BY readAt DESC;
```

`instr(lower(title), lower(?1)) > 0` is substring search without LIKE wildcards (fixes `%`/`_` injection, EC-42-H-02). `lower()` is applied to both sides; for non-ASCII the result is Unicode-aware only if the `unicode` feature (`lower` is ASCII-only in SQLite core). Decide: keep ASCII-only folding to match Kotlin behavior, OR register a Unicode-aware `lower_uni` function via `Connection::create_scalar_function` (recommended: `caseless` folding using `str::to_lowercase`). Empty query → `instr(x, '') = 1` (true) so all rows match, matching the Kotlin `LIKE '%%'`.

Note: `historyView` and the `history` table keep the names of area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations schema. The view definition is owned there; this area only queries it.

### 4.2 Resume queries

Latest manga history (port of `getLatestHistory`): same WHERE without `maxReadAtChapterId`, `ORDER BY readAt DESC LIMIT 1`.

### 4.3 Updates

```sql
SELECT mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read,
       source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch
FROM updatesView
WHERE dateUpload > ?1          -- upload_after_ms
ORDER BY datefetch DESC
LIMIT ?2;                      -- 500
```

Anime: same over `animeupdatesView`, selecting `seen`, `bookmark`, `fillermark`, `last_second_seen`, `totalSeconds`.

Reactivity: watch tables `mangas`, `chapters`, `history` (manga) / `animes`, `episodes`, `animehistory` (anime) plus the download cache signal.

### 4.4 Stats

One aggregate query per kind instead of the Kotlin N+1 (`getEpisodesByAnimeId` per anime, `getTracks` per entry):

```sql
-- anime watch time (behavior 5.3), one statement:
SELECT COALESCE(SUM(CASE WHEN e.seen THEN e.total_seconds ELSE e.last_second_seen END), 0)
FROM episodes e JOIN animes a ON a._id = e.anime_id
WHERE a.favorite = 1;      -- NOTE: Kotlin sums over LIBRARY anime, not favorites
```

Correctness note: Kotlin's library list (`GetLibraryAnime.await()`) is not identical to `favorite = 1` (library = favorites shown in categories); the Rust query must use the same library predicate as area:37-ui-library (`library_entries` view). Do not use `favorite = 1` unless area:37-ui-library defines it that way (EC-42-S-03).

Manga read duration (behavior 5.2 step 7): `SELECT COALESCE(SUM(time_read), 0) FROM history;` (all rows; keep the Kotlin semantics; EC-42-S-01).

Library rows for counts: `LibraryEntryStat` from area:08-domain-library-category-history-updates's library query (one row per entry per category).

### 4.5 Storage

Library rows for storage: same library query as 4.4 plus `thumbnail_url`, `title`, `category`. Per-entry sizes come from the download cache (`DownloadCache::size_of(entry)`, `count_of(entry)`), not SQL. Categories from area:08-domain-library-category-history-updates (`visible` when `hidden_categories` is true).

### 4.6 Writes

| Action | Rust call | SQL |
|---|---|---|
| Remove one | `reset_manga_row(history_id)` | `UPDATE history SET last_read = 0 WHERE _id = ?1;` |
| Remove all of entry | `reset_manga_entry(entry_id)` | `UPDATE history SET last_read = 0 WHERE _id IN (SELECT H._id FROM mangas M JOIN chapters C ON M._id = C.manga_id JOIN history H ON C._id = H.chapter_id WHERE M._id = ?1);` |
| Clear all | `delete_all_manga()` | `DELETE FROM history;` |

Anime: identical shape with `animehistory`, `last_seen`, `episodes`.

All writes run on the DB writer thread inside one transaction per action, returning `Result`; the UI shows an error toast on `Err` (the Kotlin swallows errors; Rust surfaces them, see section 5).

---

## 5. Error handling

- Domain functions are pure and infallible.
- Repository functions return `rusqlite::Result`; the view model maps them to `UiError` with a user message key:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    #[error("internal error")] Db(#[from] rusqlite::Error),          // -> MR.strings.internal_error text
    #[error("download cache not ready")] CacheNotReady,
    #[error("tracker not registered for id {0}")] UnknownTracker(i64),
}
```

- Clear-history and remove-history: on `Err`, show toast `"InternalError: Check crash logs for further information"` (same text as `MR.strings.internal_error`) and log with `tracing::error!`. This intentionally differs from Kotlin (silent) — documented in EC-42-H-09.
- Stats: each section computed independently; a failing section becomes `None` / "N/A" and is logged (differs from Kotlin forever-Loading, EC-42-S-06).
- Storage: `build_snapshot` cannot fail; a missing size → 0 bytes, count 0 (Kotlin: `getDownloadSize` returns 0 for missing dirs, `?: 0L`).
- Logging: `tracing` with `tracing-subscriber` (env filter), replacing `logcat`.

---

## 6. Plugging into the existing mockup

Current behavior of `src/screens.rs`:
- `updates()` (line 277) and `history()` (line 305) build fixed rows from `title_n(i)` with hard-coded text ("Library last updated: 5 minutes ago", "Ch. N - 21:30"). No data.
- `kind_tabs(|anime| ...)` (line 70) is the Anime | Manga switcher; reuse it as the base of `TabbedScreen`.
- `dialog_of("history", d)` opens a spec-driven dialog from `spec/app.json` (`history_remove_anime`, `history_remove_manga`, `history_remove_everything`, `updates_delete_chapters`, `updates_delete_episodes`, `storage_delete_anime`, `storage_delete_manga`, `history_duplicate_entry`). Keep the spec ids as the dialog registry keys so the copy stays data-driven; bind their confirm actions to the view models in section 3.
- `stats()` (line 443) uses a literal array `values = [["42","2d 4h 10m","7"], ...]` and `stat_card` (line 426). Replace the array with `StatsSnapshot` values; keep `stat_card` as `ui/stats/card.rs` (`SectionCard` port) with `StatTile` children.
- `storage()` (after line 470): placeholder; replace with `ui/storage/*` (section 1).

Migration steps (each is a mergeable unit):
1. Add `kuro-domain` pure functions and their unit tests (no GTK change).
2. Add `kuro-data` queries behind a `Repo` trait with a fake implementation for the mock UI.
3. Replace `updates()`/`history()` bodies with view-model-driven lists; keep the same visual structure (headers, 56/96 px rows).
4. Replace `stats()` and `storage()`.
5. Wire dialogs to view models, delete the literal arrays.

`spec/app.json` screen ids `history`, `updates`, `stats`, `storage` stay; `spec/SCHEMA.md` gets no new fields. Sidebar badge counts (`src/main.rs:175`, `"updates" => "8"`) become `library_unread_updates_count` / `library_unseen_updates_count` sums (area:26-library-update-jobs, area:02-preferences-system prefs), read through the same pref store.

Nav index mapping (behavior 0.1) is implemented in the navigation layer (area:36-ui-shell-navigation-security-webview); this area supplies `NavStyle` only.

---

## 7. Date, time and number formatting

Use `chrono` (`features = ["clock"]`, default local zone via `chrono::Local`) for all calendar math. No `chrono-tz`.

- Local date of an epoch: `chrono::DateTime::<chrono::Local>::from(UNIX_EPOCH + Duration::from_millis(ms as u64)).date_naive()` (equivalent to `LocalDate.ofInstant(ms, systemDefault)`).
- `difference = (today - date).num_days()` (equivalent of `ChronoUnit.DAYS.between(date, now)` for date-only values; positive for past dates). Thresholds exactly as behavior 1.6: `< -7` absolute; `< 0` upcoming; `< 1` "Today"; `< 7` "N days ago"; else absolute.
- Absolute date with `app_date_format`:
  - empty → localized SHORT date (Java `FormatStyle.SHORT`); Rust proposal: `chrono` `%x` on the active locale, falling back to `%Y-%m-%d` when no locale is set (document as a deviation);
  - non-empty → Java pattern translated to chrono strftime (table: `yyyy`→`%Y`, `MM`→`%m`, `dd`→`%d`, `HH`→`%H`, `mm`→`%M`, `ss`→`%S`, `MMM`→`%b`, `EEE`→`%a`; anything else rejected at settings time by area:02-preferences-system).
- Time of day (`toTimestampString`): `DateFormat.getTimeInstance(SHORT)` → Rust: `%X`-equivalent short time for the locale, fallback `%H:%M`. Only time is shown under date headers (behavior 1.4).
- Relative time span (`relativeTimeSpanString`, behavior 2.3): `epoch <= 0` → "Never"; `now - epoch < 60_000` → "Just now"; else a humanized string with Android's `DateUtils` semantics (minute resolution). Proposal: implement `relative_time_span` locally: `< 60 min` → "N minute(s) ago"; `< 24 h` → "N hour(s) ago"; `< 48 h` → "Yesterday"; `< 7 d` → "N days ago"; otherwise absolute date. Mark as a deviation: Android's exact strings are locale-generated; the list above is English only and must go through the i18n table.
- Numbers: `format!("{:.2}", x)` for mean score, with `"{:.2} ★"`; decimal point always `.` (Kotlin uses `Locale.ENGLISH` for this string only; size strings use the device locale in Kotlin, Rust uses `.` consistently — deviation recorded in EC-42-T-10).
- Chapter/episode number formatting is area:52-util-app's `formatChapterNumber` / `formatEpisodeNumber`; this area consumes the function.

---

## 8. Android → Linux mapping (for every Android/Kotlin API touched by part 1)

| Android / Kotlin API (file where used) | Linux / Rust replacement | Notes |
|---|---|---|
| Compose `LazyColumn` + `stickyHeader`-style `ListGroupHeader` (`MangaHistoryScreen.kt:75-109`) | `gtk::ListView` with `gtk::SignalListItemFactory`; headers as a non-selectable row type | one model with `HistoryListItem` enum; `gio::ListStore` of `glib::BoxedAnyObject` |
| `FastScrollLazyColumn` (`presentation-core`) | `gtk::ListView` (GTK provides scrollbar; fast scroll not needed) | drop custom fast-scroll |
| `animateItemFastScroll`, `animateItem` | none (no animation) or `gtk::Revealer` for insert | drop |
| `combinedClickable` (long-press + click) | `gtk::GestureClick` (button 1, `released` = click) + `gtk::GestureLongPress` (`pressed` after 600 ms delay) | keep 600 ms default of GTK |
| `HapticFeedbackType.LongPress` | none | drop |
| `BackHandler` (selection mode) | `EventControllerKey` on the window: Escape clears selection when `selection.is_selection_mode()`. Conflict: `src/main.rs:78` maps Escape to back — route through the same handler: selection-mode first, then navigation | |
| `PullRefresh` | `gtk::ScrolledWindow` overshoot is not a refresh gesture; use a header button "Update library" (`view-refresh-symbolic`) and a `gtk::Spinner` while the job runs; no fake 1 s spinner (proposal, see T-42a-033) | |
| `EntryBottomActionMenu` (bottom sheet of icon buttons) | `gtk::ActionBar` shown when selection non-empty, with `gtk::Button`s; overflow in `gtk::MenuButton` | |
| `AlertDialog` | `gtk::AlertDialog` (GTK 4.10+) or `gtk::Window` modal with `gtk::Button`s; `gtk::CheckButton` for "remove everything" | no libadwaita |
| `SnackbarHost` / `showSnackbar` | `gtk::Overlay` + toast label with 4 s timer (GTK has no toast without libadwaita) | |
| `SearchToolbar` | `gtk::SearchEntry` in `gtk::HeaderBar`, `search-changed` debounced 250 ms (proposal; Kotlin has no debounce) | |
| `HorizontalPager` + `PrimaryTabRow` | `gtk::Stack` + `gtk::StackSwitcher` (or a row of `gtk::ToggleButton`) | |
| `Voyager` `TabContent`/`Tab`/`LocalNavigator` | area:36-ui-shell-navigation-security-webview navigation (`gtk::Stack` page per screen) | |
| `DateFormat.getTimeInstance(SHORT)`, `DateTimeFormatter.ofLocalizedDate` | `chrono` + locale table (section 7) | |
| `android.text.format.DateUtils.getRelativeTimeSpanString` | `format::relative_time_span` (section 3.6) | |
| `String.format(Locale.ENGLISH, "%.2f ★")` | `format!("{:.2} ★")` | |
| `Long.toSize()` (`String.format` with device locale) | `storage::size::format_size` (decimal point `.`) | deviation EC-42-T-10 |
| `Canvas` + `drawArc` + `Stroke(cap = Round)` (`CumulativeStorage.kt`) | cairo `arc` + `set_line_cap(Round)` in a `gtk::DrawingArea` with `set_draw_func` | |
| `Modifier.background(color, CircleShape)` | `gtk::DrawingArea` 12x12 circle or a CSS `border-radius: 50%` `box` with `background` from a generated CSS class | pick CSS class per colour via `gtk::CssProvider` |
| `ItemCover.Book` / `ItemCover.Square` (`presentation/entries/components`) | `gtk::Picture` with `content-fit: cover`, loader from area:34-images-covers-cache cover cache | |
| `Icons.Outlined.*` (Material) | GTK symbolic icons (`user-trash-symbolic`, `emblem-favorite-symbolic`, `edit-clear-all-symbolic`, `view-refresh-symbolic`, `bookmark-new-symbolic`, `tag-symbolic`, `document-edit-symbolic`) | the icon set is part of area:36-ui-shell-navigation-security-webview theme work |
| `Preference.asState(scope)` | `tokio::sync::watch` fed by the pref store (area:02-preferences-system) | |
| `Channel(UNLIMITED)` events (`Event.OpenChapter`, `HistoryCleared`) | `async_channel::unbounded()` consumed on the GTK main context | |
| `Channel<Unit>()` rendezvous resume events | `async_channel::bounded(1)` with `try_send` (drops duplicate reselects) | |
| `screenModelScope` / `launchIO` / `launchNonCancellable` | tokio task on the runtime + `CancellationToken`; non-cancellable = no token | section 2 |
| `combine(...)` / `collectLatest` | `tokio::select!` over broadcast receivers + cancel-previous `JoinHandle::abort` | |
| `MainActivity.startPlayerActivity` | area:45-player-core (`player::open(entry, unit, external: bool)`) | |
| `ReaderActivity.newIntent` | area:43-reader-core, area:44-reader-viewers (`reader::open(manga_id, chapter_id)`) | |
| `Injekt.get()` (DI) | explicit constructor injection; `struct Services` built in `main` | area:01-foundation-app-bootstrap |
| `logcat` | `tracing` | |

---

## 9. Dropped Android behavior

Each drop is a deliberate decision with the user-visible effect stated.

| Dropped | Where (Kotlin) | Reason | What the user loses |
|---|---|---|---|
| Haptic feedback on long press | `AnimeUpdatesUiItem.kt:170`, `MangaUpdatesUiItem.kt:163` | no Linux haptic API | none visible on desktop |
| Fake 1-second refresh spinner | `MangaUpdatesScreen.kt:79-83`, `AnimeUpdatesScreen.kt:85-89` | replaced by a real job-state spinner (improvement) | the spinner now shows for the real job duration |
| Splash hand-off `MainActivity.ready` | `MangaHistoryTab.kt:150-154`, `AnimeHistoryTab.kt:153-157`, `HistoriesTab.kt:75-77`, `UpdatesTab.kt:60-62`, `StatsTab.kt:52-54`, `StorageTab.kt:52-54` | no Android splash | none |
| `TAB_ANIME` / `TAB_MANGA` constants | `HistoriesTab.kt:81-82`, `UpdatesTab.kt:66-67` | unused in Kotlin | none |
| `onInvertSelection` parameter | `AnimeUpdatesScreen.kt:43`, `MangaUpdatesScreen.kt:40` | parameter unused in the composable body | none (invert is still reachable via the model) |
| `State.searchQuery` on the updates/history state | `MangaHistoryScreenModel.kt:141-143` (`updateSearchQuery`) | never driven by UI; search text lives in `query` | none |
| Compose previews and `PreviewParameterProvider` fixtures | `*StateProvider.kt`, `*WithRelationsProvider.kt`, `*Preview*` functions | tooling only | the same fixtures become test fixtures (p1-edge-cases-and-tests.md) |
| Voyager `Screen` / `LocalNavigator` / `rememberScreenModel` | all `*Tab.kt` | replaced by GTK stack | none |
| Android `DateUtils` relative text | `TimeUtils.kt:35-44` | replaced by a deterministic English table | non-English relative strings come from the i18n table only; exact wording differs from Android's generated strings |
| `UiPreferences.relativeTime` and `dateFormat` as Java patterns | `UiPreferences.kt:33-48` | kept, but the pattern is translated (section 7) | patterns with unsupported Java letters are rejected at settings time |
| Per-entry random colour via `kotlin.random.Random` | `CommonStorageScreenModel.kt:74-87` | colours need only be stable, not identical | the same entry gets a different colour than in Aniyomi |
| Stats N+1 queries | `AnimeStatsScreenModel.kt:130-136`, `MangaStatsScreenModel.kt:118-126` | replaced by aggregate SQL (4.4) | none (same numbers, EC-42-S-02 documents parity checks) |

---

## 10. Pitfalls and risks (ranked)

1. **Single-consumer channel fan-out** (behavior 6.8, 9.1): storage and updates both consume `downloadCache.changes`. Use the broadcast design (section 2). Test: two subscribers both receive one change.
2. **Storage page stuck loading** if the cache signal was consumed before subscription. Fixed by `watch` for `initializing` and by a broadcast with an initial `()` sent on subscribe when the cache is ready.
3. **LIKE wildcard injection** in history search (`%`, `_`). Use `instr` (4.1).
4. **Next-item index -1** (`next_units_from`): fix as described in 3.2 (test EC-42-H-05).
5. **Clear-all is a hard delete** that also erases manga read-duration (`time_read`). Keep behavior but confirm in UI copy ("All history will be lost"); the Kotlin text already says so.
6. **Asymmetric migrate navigation** (manga replaces with new entry, anime with old entry; behavior 1.10). Port as observed, flag for product decision.
7. **Stats forever-loading on exception** (`!!` in `get10PointScore`). Rust: `Option` and per-section error handling.
8. **Download-state snapshot cost**: Kotlin recomputes all rows per emission with disk lookups. Rust: cache `is_downloaded` by `(source, entry title, unit name, scanlator)` key and invalidate on cache signal.
9. **GTK ListView recycling**: row widgets must not hold `HistoryRow` by value across rebinds; bind via `glib::BoxedAnyObject` and reset in `unbind`.
10. **Clock**: `now` captured per computation (update window, relative dates); avoid calling `Local::now()` inside row bind closures.
11. **Escape key conflict** with navigation (`src/main.rs:78`): selection-mode first.
12. **Units**: anime `total_seconds`/`last_second_seen` are milliseconds; keep ms in Rust names (`total_ms`, `last_ms`) and document in the migration of area:04-db-anime-schema-migrations.
13. **Number formatting** of `toSize` uses device locale in Kotlin; Rust uses `.`; add a setting only if requested (non-goal now).
