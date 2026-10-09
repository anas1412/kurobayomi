# Area 38 part 1: Rust design (manga entry details)

Target: Kurobayomi (`/home/blackbox/kurobayomi`, crate `kurobayomi`, edition 2024). Current manifest: `gtk = gtk4 0.11.5` with feature `v4_16`, `serde`, `serde_json`. Current modules: `clamp`, `entry`, `player`, `screens`, `settings`, `spec`, `ui`. The mockup builds the manga details page in `src/entry.rs` (`pub fn entry(title, anime) -> GBox`, lines 81-242), with dialogs/sheets resolved through `src/screens.rs` (`dialog_of`, `sheet_of` in `entry.rs` lines 25-35) and copy from `spec/entries.json` (screen id `manga_entry_details`, sheets `chapter_settings`).

This document specifies a new module tree, the pure logic (testable without GTK), the async boundary, and the mapping of every Android API used by the scope files.

---

## 1. Crate and module layout

```
src/
  details/
    mod.rs              // pub fn entry_screen(entry_id: i64, from_source: bool) -> GBox; route registration
    model.rs            // EntryDetailsModel: state, command queue, derived views (GTK-free)
    store.rs            // StoreCmd / StoreEvent enums; worker glue (tokio side)
    chapters.rs         // ChapterItem, ListRow, process(), with_missing_markers(), missing_chapters_count()
    flags.rs            // ChapterFlags bit codec, SortMode, TriState
    compare.rs          // sort comparators (exact Kotlin table), collator trait
    numfmt.rs           // format_chapter_number(), missing count text helpers
    text.rs             // description whitespace collapse, tag normalization
    download.rs         // DownloadState, DownloadIndicatorView, action mapping
    swipe.rs            // SwipeAction, swipe threshold, action icon name mapping
    view/
      header.rs         // info box (cover, title, author, artist, status, source) + backdrop
      actions.rs        // action row (library, interval, tracking, webview)
      description.rs    // expandable summary + tag chips
      toolbar.rs        // top bar: title fade, actions, overflow, action mode
      chapter_list.rs   // gtk::ListView of ChapterRow objects, header, FAB, bottom bar
      chapter_row.rs    // row widget, download indicator, swipe gesture
      bottom_bar.rs     // bulk action bar with conditional buttons
      layout.rs         // single column vs two panel switch
    dialogs/
      settings.rs       // filter / sort / display sheet
      set_default.rs    // set-as-default confirm
      scanlators.rs     // exclude scanlators dialog
      interval.rs       // fetch interval dialog
      duplicate.rs      // duplicate entry sheet
      delete.rs         // delete downloads confirm
      cover.rs          // full cover window (zoom, share, save, edit menu)
    cover_model.rs      // CoverModel: save / share / edit / delete custom cover state machine
  ui.rs, screens.rs, spec.rs, entry.rs  // existing; entry.rs delegates manga details to details::entry_screen
```

Anime keeps the existing `entry.rs` path until area:39+ (anime) is designed. The manga module must not import anime-only code. `details::chapters` is generic enough to be reused for anime episodes with a different row type; do not generalize now (YAGNI, see task list).

---

## 2. Pure logic (GTK-free, unit-testable)

### 2.1 flags.rs

```rust
// Source: domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:75-103
pub const SORT_DIR_MASK: i64 = 0x0000_0001; // bit 0; 0 = DESC, 1 = ASC
pub const UNREAD_MASK: i64 = 0x0000_0006;
pub const DOWNLOADED_MASK: i64 = 0x0000_0018;
pub const BOOKMARKED_MASK: i64 = 0x0000_0060;
pub const SORTING_MASK: i64 = 0x0000_0300;
pub const DISPLAY_MASK: i64 = 0x0010_0000;

pub const SORT_DESC: i64 = 0x0;
pub const SORT_ASC: i64 = 0x1;
pub const SHOW_UNREAD: i64 = 0x2;
pub const SHOW_READ: i64 = 0x4;
pub const SHOW_DOWNLOADED: i64 = 0x8;
pub const SHOW_NOT_DOWNLOADED: i64 = 0x10;
pub const SHOW_BOOKMARKED: i64 = 0x20;
pub const SHOW_NOT_BOOKMARKED: i64 = 0x40;
pub const DISPLAY_NUMBER: i64 = 0x0010_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState { Disabled, EnabledIs, EnabledNot }

impl TriState {
    /// Same cycle as core/common TriState.next(): DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED.
    pub fn next(self) -> Self {
        match self { Self::Disabled => Self::EnabledIs, Self::EnabledIs => Self::EnabledNot, Self::EnabledNot => Self::Disabled }
    }
    /// domain/.../entries/TriState.kt applyFilter: DISABLED keeps all; IS keeps predicate; NOT keeps !predicate.
    pub fn keep(self, predicate: impl FnOnce() -> bool) -> bool {
        match self { Self::Disabled => true, Self::EnabledIs => predicate(), Self::EnabledNot => !predicate() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMode { Source, Number, UploadDate, Alphabet }

impl SortMode {
    pub const fn bits(self) -> i64 {
        match self { Self::Source => 0x000, Self::Number => 0x100, Self::UploadDate => 0x200, Self::Alphabet => 0x300 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChapterFlags(pub i64);

impl ChapterFlags {
    pub fn with(self, value: i64, mask: i64) -> Self {
        Self((self.0 & !mask) | (value & mask)) // Kotlin: this and mask.inv() or (flag and mask)
    }
    pub fn sort_mode(self) -> SortMode {
        match self.0 & SORTING_MASK { 0x000 => SortMode::Source, 0x100 => SortMode::Number, 0x200 => SortMode::UploadDate, _ => SortMode::Alphabet }
    }
    /// Manga.sortDescending(): true when bit 0 == 0.
    pub fn sort_descending(self) -> bool { self.0 & SORT_DIR_MASK == SORT_DESC }
    pub fn unread(self) -> TriState {
        match self.0 & UNREAD_MASK { SHOW_UNREAD => TriState::EnabledIs, SHOW_READ => TriState::EnabledNot, _ => TriState::Disabled }
    }
    pub fn downloaded_raw(self) -> TriState {
        match self.0 & DOWNLOADED_MASK { SHOW_DOWNLOADED => TriState::EnabledIs, SHOW_NOT_DOWNLOADED => TriState::EnabledNot, _ => TriState::Disabled }
    }
    pub fn bookmarked(self) -> TriState {
        match self.0 & BOOKMARKED_MASK { SHOW_BOOKMARKED => TriState::EnabledIs, SHOW_NOT_BOOKMARKED => TriState::EnabledNot, _ => TriState::Disabled }
    }
    pub fn display_number(self) -> bool { self.0 & DISPLAY_MASK != 0 }
    /// SetMangaChapterFlags.awaitSetSortingModeOrFlipOrder
    pub fn select_sort(self, mode: SortMode) -> Self {
        if self.sort_mode() == mode {
            let dir = if self.sort_descending() { SORT_ASC } else { SORT_DESC };
            self.with(dir, SORT_DIR_MASK)
        } else {
            self.with(mode.bits(), SORTING_MASK).with(SORT_ASC, SORT_DIR_MASK)
        }
    }
}
```

Downloaded filter at the UI level is overridden when the global "downloaded only" preference is on: `effective_downloaded(flags, downloaded_only) = if downloaded_only { EnabledIs } else { flags.downloaded_raw() }` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:23-31`). Put this in `model.rs`, not in `flags.rs`, so `flags.rs` stays a pure codec.

### 2.2 compare.rs

Exact Kotlin table (behavior spec section 6.3). Collator is injected:

```rust
use std::cmp::Ordering;
use super::flags::SortMode;

pub trait Collate { fn primary_cmp(&self, a: &str, b: &str) -> Ordering; }

pub struct ChapterKey<'a> { pub source_order: i64, pub number: f64, pub date_upload: i64, pub name: &'a str }

/// `desc` = Manga.sortDescending(). Port literally: the SOURCE branch is inverted relative to its name.
pub fn compare(mode: SortMode, desc: bool, a: &ChapterKey, b: &ChapterKey, c: &dyn Collate) -> Ordering {
    match (mode, desc) {
        (SortMode::Source, true) => a.source_order.cmp(&b.source_order),
        (SortMode::Source, false) => b.source_order.cmp(&a.source_order),
        (SortMode::Number, true) => b.number.total_cmp(&a.number),
        (SortMode::Number, false) => a.number.total_cmp(&b.number),
        (SortMode::UploadDate, true) => b.date_upload.cmp(&a.date_upload),
        (SortMode::UploadDate, false) => a.date_upload.cmp(&b.date_upload),
        (SortMode::Alphabet, true) => c.primary_cmp(b.name, a.name),
        (SortMode::Alphabet, false) => c.primary_cmp(a.name, b.name),
    }
}
```

Collator: Java `Collator.getInstance(Locale.getDefault())` with `PRIMARY` strength (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:6-15`). Rust proposal: `icu_collator` (ICU4X) with `CollatorOptions { strength: Some(Strength::Primary) }` for the user's locale (from `LANG`), falling back to root. Known parity risk: Java's RuleBasedCollator tables differ from CLDR root for some scripts. Mark as a pitfall; accept small ordering differences, verify with the test list in `p1-edge-cases-and-tests.md`.

### 2.3 chapters.rs

```rust
use super::flags::{ChapterFlags, TriState};
use super::download::DownloadState;

#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {               // mirror of tachiyomi.domain.items.chapter.model.Chapter
    pub id: i64,
    pub manga_id: i64,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,       // 0-based page index
    pub source_order: i64,
    pub name: String,
    pub date_upload: i64,          // epoch ms
    pub chapter_number: f64,       // -1.0 = unknown
    pub scanlator: Option<String>,
}

impl Chapter {
    pub fn is_recognized_number(&self) -> bool { self.chapter_number >= 0.0 } // Kotlin: chapterNumber >= 0f
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChapterItem {
    pub chapter: Chapter,
    pub download: DownloadState,
    pub progress: i32,             // Kotlin Int; see view note on clamping
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ListRow {
    Missing { id: String, count: u32 },
    Item { index: usize },         // index into the processed Vec
}

/// Pipeline order is exact (MangaScreenModel.kt:1196-1222): unread, bookmarked, downloaded, sort.
pub fn process(items: &[ChapterItem], flags: ChapterFlags, downloaded_filter: TriState, is_local: bool, cmp: &dyn Fn(&ChapterItem, &ChapterItem) -> std::cmp::Ordering) -> Vec<ChapterItem> {
    let unread = flags.unread();
    let bookmarked = flags.bookmarked();
    let mut out: Vec<ChapterItem> = items.iter()
        .filter(|i| unread.keep(|| !i.chapter.read))
        .filter(|i| bookmarked.keep(|| i.chapter.bookmark))
        .filter(|i| downloaded_filter.keep(|| i.download == DownloadState::Downloaded || is_local))
        .cloned()
        .collect();
    out.sort_by(|a, b| cmp(a, b)); // stable, like Kotlin sortedWith
    out
}

/// Missing-count separators (MangaScreenModel.kt:1164-1189). `desc` = manga.sortDescending().
pub fn with_missing_markers(list: &[ChapterItem], desc: bool) -> Vec<ListRow> {
    let mut rows = Vec::with_capacity(list.len() * 2 + 1);
    for i in 0..=list.len() {
        let before = if i == 0 { None } else { list.get(i - 1) };
        let after = list.get(i);
        if let Some((id, count)) = gap_between(before, after, desc) {
            rows.push(ListRow::Missing { id, count });
        }
        if i < list.len() { rows.push(ListRow::Item { index: i }); }
    }
    rows
}

fn gap_between(before: Option<&ChapterItem>, after: Option<&ChapterItem>, desc: bool) -> Option<(String, u32)> {
    let (lower, higher) = if desc { (after, before) } else { (before, after) };
    let higher = higher?;
    let count: i64 = match lower {
        None => ((higher.chapter.chapter_number.floor() as i64) - 1).max(0),
        Some(lo) => calculate_chapter_gap(&higher.chapter, &lo.chapter),
    };
    if count <= 0 { return None; }
    let lower_id = lower.map(|l| l.chapter.id.to_string()).unwrap_or_else(|| "null".to_string());
    Some((format!("{}-{}", lower_id, higher.chapter.id), count as u32))
}

/// domain/.../items/chapter/service/MissingChapters.kt:40-49
pub fn calculate_chapter_gap(higher: &Chapter, lower: &Chapter) -> i64 {
    if !higher.is_recognized_number() || !lower.is_recognized_number() { return 0; }
    let (h, l) = (higher.chapter_number, lower.chapter_number);
    if h < 0.0 || l < 0.0 { return 0; }
    (h.floor() as i64) - (l.floor() as i64) - 1
}

/// domain/.../items/chapter/service/MissingChapters.kt:6-38
pub fn missing_chapters_count(numbers: &[f64]) -> u32 {
    let mut v: Vec<i64> = numbers.iter().filter(|&&n| n != -1.0).map(|&n| n as i64).collect();
    v.sort_unstable();
    v.dedup();
    let mut missing: u32 = 0;
    let mut prev: i64 = 0;
    for c in v {
        if c > prev + 1 { missing += (c - prev - 1) as u32; }
        prev = c;
    }
    missing
}
```

Notes:
- Rust `f64 as i64` saturates and truncates toward zero, matching Kotlin `Double.toInt()` for in-range values.
- The Kotlin `ChapterList.Item.id` is `chapter.id`. In Rust the `Missing.id` string must equal Kotlin's exactly, because it is the list key and some tests compare it.
- `process` takes a `cmp` closure so the caller builds the `ChapterKey` from `compare::compare(mode, desc, ...)`.

### 2.4 numfmt.rs

```rust
/// presentation/util/ItemNumberFormatter.kt: DecimalFormat("#.###") with '.' decimal separator, HALF_EVEN.
pub fn format_chapter_number(n: f64) -> String { /* see below */ }
```

Implementation contract (proposal): use `rust_decimal::Decimal::from_f64_retain(n)` (exact binary expansion, matching Java's HALF_EVEN on the exact double), then `round_dp_with_strategy(3, RoundingStrategy::MidpointNearestEven)`, then `normalize()` and format without grouping, without trailing zeros, without trailing dot. `-0.0` must print `0`. Non-finite values: Java prints `NaN` / `∞`; chapter numbers are never non-finite in practice, so treat NaN as "NaN" and infinity as "∞" to match Java (edge E-07).

Examples: `1.0 → "1"`, `1.5 → "1.5"`, `1.2345 → "1.234"` (exact binary of 1.2345 is slightly below? verify in test; HALF_EVEN on exact expansion), `10.0005 → "10"` or `"10.001"` per exact binary; the test suite must assert these with the Java output generated once by running `DecimalFormat` in the scratchpad (edge E-07).

### 2.5 download.rs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState { NotDownloaded = 0, Queue = 1, Downloading = 2, Downloaded = 3, Error = 4 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChapterDownloadAction { Start, StartNow, Cancel, Delete }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IndicatorView {
    Download,                              // NOT_DOWNLOADED
    Spinner,                               // QUEUE, or DOWNLOADING with progress 0
    Ring { fraction: f64, arrow_on_fill: bool }, // DOWNLOADING with progress > 0
    Downloaded,                            // DOWNLOADED
    Error,                                 // ERROR
}

pub fn indicator_view(state: DownloadState, progress: i32) -> IndicatorView {
    match state {
        DownloadState::NotDownloaded => IndicatorView::Download,
        DownloadState::Queue => IndicatorView::Spinner,
        DownloadState::Downloading if progress == 0 => IndicatorView::Spinner,
        DownloadState::Downloading => {
            let fraction = (progress as f64 / 100.0).clamp(0.0, 1.0);
            IndicatorView::Ring { fraction, arrow_on_fill: fraction >= 0.5 }
        }
        DownloadState::Downloaded => IndicatorView::Downloaded,
        DownloadState::Error => IndicatorView::Error,
    }
}

/// Click and long-press mapping (ChapterDownloadIndicator.kt).
pub enum Gesture { Click, LongPress }

pub enum IndicatorOutcome { Action(ChapterDownloadAction), Menu(&'static [ChapterDownloadAction]) }

pub fn outcome(state: DownloadState, g: Gesture) -> IndicatorOutcome {
    use ChapterDownloadAction::*;
    match (state, g) {
        (DownloadState::NotDownloaded, Gesture::Click) => IndicatorOutcome::Action(Start),
        (DownloadState::NotDownloaded, Gesture::LongPress) => IndicatorOutcome::Action(StartNow),
        (DownloadState::Queue | DownloadState::Downloading, Gesture::Click) => IndicatorOutcome::Menu(&[StartNow, Cancel]),
        (DownloadState::Queue | DownloadState::Downloading, Gesture::LongPress) => IndicatorOutcome::Action(Cancel),
        (DownloadState::Downloaded, _) => IndicatorOutcome::Menu(&[Delete]),
        (DownloadState::Error, _) => IndicatorOutcome::Action(Start),
    }
}
```

Note: the Kotlin `DownloadingIndicator` long-press is `CANCEL` for both QUEUE and DOWNLOADING, the click opens a menu with START_NOW and CANCEL.

### 2.6 swipe.rs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeAction { ToggleRead, ToggleBookmark, Download, Disabled }
pub const SWIPE_THRESHOLD_PX: f64 = 56.0; // 56.dp at 1x (Compose dp == px at scale 1.0)

pub struct SwipeVisual { pub icon: &'static str, pub undo: bool }

pub fn visual(action: SwipeAction, read: bool, bookmark: bool, dl: DownloadState) -> Option<SwipeVisual> {
    match action {
        SwipeAction::ToggleRead => Some(SwipeVisual { icon: if !read { "object-select-symbolic" } else { "edit-undo-symbolic" }, undo: read }),
        SwipeAction::ToggleBookmark => Some(SwipeVisual { icon: if !bookmark { "bookmark-new-symbolic" } else { "user-bookmarks-symbolic" }, undo: bookmark }),
        SwipeAction::Download => Some(SwipeVisual {
            icon: match dl {
                DownloadState::NotDownloaded | DownloadState::Error => "folder-download-symbolic",
                DownloadState::Queue | DownloadState::Downloading => "process-stop-symbolic",
                DownloadState::Downloaded => "user-trash-symbolic",
            },
            undo: false,
        }),
        SwipeAction::Disabled => None,
    }
}
```

Icon names are proposals mapped from Material icons; check availability in the Adwaita/Papirus set on the target system (`gtk::IconTheme::has_icon`). Material `FileDownloadOff` → `process-stop-symbolic` is a proposal.

### 2.7 text.rs

```rust
use regex::Regex;
use std::sync::OnceLock;

pub fn collapse_blank_lines(desc: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[\r\n]{2,}").unwrap());
    re.replace_all(desc, "\n").trim_end().to_string()
}

pub fn description_or_placeholder(desc: Option<&str>) -> &str {
    match desc { Some(d) if !d.trim().is_empty() => d, _ => "No description" }
}
```

Note: Kotlin `isNullOrBlank()` uses `isWhitespace` for all chars; Rust `trim().is_empty()` is Unicode whitespace; equivalent enough, verify edge E-05.

### 2.8 model.rs (GTK-free state)

```rust
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct EntryState {
    pub manga: MangaSnapshot,         // id, title, artist, author, description, genres, status, favorite, fetch_interval, next_update, source_id, source_kind, cover_last_modified, initialized, chapter_flags
    pub source: SourceInfo,           // id, name_for_info, is_http, is_stub, is_configurable, is_local
    pub is_from_source: bool,
    pub chapters: Vec<ChapterItem>,   // unprocessed (from store)
    pub available_scanlators: BTreeSet<String>,
    pub excluded_scanlators: BTreeSet<String>,
    pub tracking_count: u32,
    pub has_logged_in_trackers: bool,
    pub refreshing: bool,
    pub dialog: Option<Dialog>,
    pub downloaded_only: bool,        // from BasePreferences
    pub skip_filtered: bool,          // reader preference, for next-unread
}

#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    SettingsSheet,
    SetDefault,
    Scanlators,
    Interval,
    DuplicateEntry { duplicate_id: i64 },
    DeleteDownloads { chapter_ids: Vec<i64> },
    FullCover,
    ChangeCategory,
    Track,
    Migrate { old_id: i64, new_id: i64 },
}

impl EntryState {
    pub fn is_any_selected(&self) -> bool { self.chapters.iter().any(|c| c.selected) }      // unprocessed, as Kotlin
    pub fn filter_active(&self) -> bool { self.scanlator_filter_active() || chapters_filtered(self) }
    pub fn scanlator_filter_active(&self) -> bool {
        self.excluded_scanlators.intersection(&self.available_scanlators).next().is_some()
    }
    pub fn tracking_available(&self) -> bool { self.tracking_count > 0 }
}
```

Rust store commands (`store.rs`), sent over `async_channel` from GTK to the worker:

```rust
pub enum StoreCmd {
    Subscribe { manga_id: i64 },
    SetChapterFlags { manga_id: i64, flags: ChapterFlags },   // one write per interaction
    SetFavorite { manga_id: i64, favorite: bool, check_duplicate: bool },
    SetFetchInterval { manga_id: i64, interval: i32 },        // stored as -interval
    SetExcludedScanlators { manga_id: i64, set: BTreeSet<String> },
    SetRead { chapter_ids: Vec<i64>, read: bool },
    SetBookmark { chapter_ids: Vec<i64>, bookmark: bool },
    MarkPreviousRead { anchor_chapter_id: i64 },
    Download { chapter_ids: Vec<i64>, action: ChapterDownloadAction },
    DeleteDownloads { chapter_ids: Vec<i64> },
    RefreshFromSource { manga_id: i64, manual: bool },
    SaveDefaultChapterSettings { manga_id: i64, apply_to_library: bool },
    ResetChapterSettings { manga_id: i64 },
    NextUnread { manga_id: i64, skip_filtered: bool },
}

pub enum StoreEvent {
    Snapshot(Box<EntryState>),  // full state from the subscription
    Error(StoreError),          // rendered as toast with the message key
    Done(u64),                  // correlates to a command id if needed
}
```

Each `StoreCmd` is handled on the tokio worker. The GTK side applies `StoreEvent::Snapshot` by replacing the state and calling `view.rebuild()`. Optimistic updates are not used: Aniyomi relies on the DB flow, so the GTK side does the same.

Non-GTK pieces: `EntryDetailsModel` is `Rc<RefCell<EntryState>>` plus `async_channel::Sender<StoreCmd>`. Methods mirror the Kotlin functions: `toggle_favorite`, `show_dialog`, `dismiss_dialog`, `set_sorting`, `set_display_mode`, `set_filter(kind, TriState)`, `toggle_selection(id, selected, user_selected, from_long_press)`, `toggle_all_selection(bool)`, `invert_selection`, `chapter_swipe(id, action)`, `next_unread()`, `fetch_all(manual)`. The range selection algorithm (`selectedPositions`, MangaScreenModel.kt:985-995) must be ported in part 2 with the same semantics; the Rust API accepts it verbatim.

---

## 3. Threading and async model

| Concern | Thread | Mechanism |
|---|---|---|
| GTK widgets, `EntryState` mutation, dialogs | GTK main thread | `Rc<RefCell<…>>`, never `Send` |
| DB reads and writes, source calls, downloads | tokio multi-thread runtime (worker threads) | `StoreCmd` over `async_channel::unbounded()` |
| Snapshot delivery | tokio → GTK | `StoreEvent` over `async_channel`, consumed by `glib::spawn_future_local` on the main context |
| Image decode (cover, backdrop blur) | rayon-free: `gio::spawn_blocking` / `std::thread` | results delivered via `async_channel` to main |
| Timers (1 s label reveal, 150 ms fades) | GTK frame clock / `glib::timeout_add_local_once` | main thread only |

Rules:
- Never block the GTK thread on DB or network. `StoreCmd` sends are non-blocking.
- Every `StoreCmd` that corresponds to a Kotlin `launchNonCancellable` must not be cancelled when the page is closed (proposal: keep the worker task alive; only `Subscribe` is dropped on page close). This mirrors Aniyomi `launchNonCancellable` for flag writes.
- Subscriptions are dropped when the entry page is popped. Re-opening creates a new subscription.
- Snapshot ordering: the worker coalesces snapshots (`latest-wins`, buffer size 1 via `async_channel::bounded(1)` with `try_send` replacing the value) to prevent UI backlog when many chapters update in one tick.

Cancellation semantics to match Kotlin:
- Chapter swipe, bulk mark, bookmark, delete: `launchNonCancellable` → run to completion even if the page is closed.
- Fetch from source: `screenModelScope.launch` → cancelled on page close (`isActive` checks in `fetchFromSourceTasks`).

---

## 4. Android API → Linux / Rust mapping (for every API touched in scope)

| Android / Kotlin API used | Where | Rust / Linux replacement |
|---|---|---|
| Jetpack Compose (`Composable`, `remember`, `LazyColumn`, `Scaffold`) | all presentation files | GTK 4 widget tree, `gtk::ListView` + `gio::ListStore` for the chapter list (virtualized, handles thousands of rows) |
| `rememberSaveable` | expand state, dialog checkboxes, wheel index | struct field in the view object (session only, same lifetime as Kotlin's saveable across config changes) |
| `derivedStateOf` / `animateFloatAsState` | title fade, FAB, download ring | GTK `Adjustment` / CSS transition / `glib::timeout` ticks (behavior section 13) |
| Voyager `Navigator`, `Screen`, `ScreenModel`, `rememberScreenModel` | ui MangaScreen.kt | `crate::ui::push/pop`, `EntryDetailsModel` owned by the page |
| `StateScreenModel` + `StateFlow` | MangaScreenModel, MangaCoverScreenModel | `Rc<RefCell<EntryState>>` + `async_channel` events |
| `screenModelScope.launch / launchIO / launchNonCancellable` | ScreenModels | tokio `spawn` (cancellable via `JoinHandle::abort`), or `spawn` without abort for non-cancellable |
| `SnackbarHostState.showSnackbar` | cover, fetch errors | in-window toast widget (`gtk::Overlay` + `gtk::Revealer`), queue of at most 1 visible + dismiss |
| `LocalHapticFeedback.performHapticFeedback` | long-press | no-op on Linux (no haptic API on the desktop); keep the call site so a future touch build can wire `gdk::Device` rumble, none exists today |
| `combinedClickable` / `clickableNoIndication` | rows, labels | `gtk::GestureClick` (button 1, button 3 for secondary), `gtk::GestureLongPress` |
| `SwipeableActionsBox` (me.saket.swipe) | chapter row | custom `gtk::GestureDrag` + `gtk::Overlay` background + threshold 56 px |
| `Coil AsyncImage` / `ImageRequest` / `crossfade` | cover, backdrop | `gdk::Texture` loaded on a worker thread; crossfade via `gtk::Picture` opacity transition 150 ms; caching in area:34's Rust cover cache |
| `ImageRequest.size(Size.ORIGINAL)` | full cover | load full-resolution `gdk::Texture` via `gdk_pixbuf` (`gio::File::read` + `Pixbuf::from_stream`) |
| `ReaderPageImageView` (SSIV zoom view) | full cover | `gtk::ScrolledWindow` + `gtk::Picture` with `GestureZoom` (ctrl+wheel and pinch), `zoomDuration = 500 ms` → 500 ms ease-out zoom transition |
| `Bitmap.Config.HARDWARE` copy | full cover | not needed; GPU textures via `gdk::Texture` |
| `ImageSaver.save(Image.Cover)` / `Location.Pictures` | cover save | `glib::user_special_dir(UserDirectory::Pictures)` + `Kurobayomi/covers/<sanitized-title>.<ext>`; write via `std::fs` on a worker thread (proposal; matches area:34's file naming) |
| `Location.Cache` + `Uri.toShareIntent` | cover share | write to `glib::user_cache_dir()/kurobayomi/share/`, then `gtk::UriLauncher` / `gio::File` → portal `org.freedesktop.portal.OpenURI` via `ashpd` for "Open with…" (proposal), or copy path to clipboard as fallback |
| `Intent.ACTION_SEND` (share URL) | shareManga | `gdk::Clipboard::set_text(url)` + toast "Copied…" (proposal; Linux has no generic share sheet without a portal; `ashpd::desktop::email` not suitable) |
| `Intent` to WebView (`WebViewScreen`) | openMangaInWebView | in-app WebView page from area:36 (webkitgtk-6.0 via `webkit6` crate) when available, otherwise `gio::AppInfo::launch_default_for_uri` |
| `ClipboardManager` (`copyToClipboard`) | copy title/author/artist/tag/URL | `gtk::prelude::WidgetExt::clipboard()` → `gdk::Clipboard::set_text` |
| `ActivityResultContracts.GetContent("image/*")` | edit cover | `gtk::FileDialog::open` with filter `image/*` (GTK 4.10+) |
| `Toast` (`context.toast(e.message)`) | share error | toast widget |
| `startActivity(ReaderActivity.newIntent(...))` | open chapter | `crate::player` / reader route with `(manga_id, chapter_id)`; reader area owns the route |
| `BackHandler` | back during selection | `gtk::EventControllerKey` on the page: Escape → clear selection, else `ui::pop()` |
| `PullRefresh` | refresh | not implemented as gesture; `Ctrl+R`, header spinner, toolbar item |
| `VerticalFastScroller` | chapter list fast scroll | `gtk::ScrolledWindow` overlay scrollbar (default) |
| `WindowInsets` / `navigationBarsPadding` | bottom bar | not applicable (no system nav bar); use `gtk::Box` bottom margin 0 |
| `isTabletUi()` | layout | `gtk::Widget::width()` on `notify::width` vs breakpoint from area:36 |
| `DotSeparatorText` | subtitles | `gtk::Label` with " • " |
| `DecimalFormat("#.###")` | chapter number | `numfmt::format_chapter_number` (rust_decimal) |
| `java.text.Collator` (PRIMARY) | alphabetical sort | `icu_collator` (ICU4X) primary strength |
| `Injekt.get<BasePreferences>()` | downloaded only | `settings::get_bool("pref_downloaded_only")` in the Rust settings layer (area:02) |
| `Preference.appStateKey` | pref namespace | same key string in Rust settings file (area:02) |
| `logcat` | errors | `tracing` (`tracing::error!`) |
| `MR.strings.*` / `AYMR.strings.*` | every message | Rust `i18n` table keyed by the same identifiers (e.g. `"action_resume"`), English defaults copied from `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`. Add a `strings.rs` in `src/details/` that maps identifiers to English text (see section 8). |
| Plural resources `pluralStringResource` | day, chapters, trackers, missing items | small `plural(key, n)` helper with the English one/other rule (`n == 1` → one, else other) |
| `Instant.now().until(next, DAYS)` | nextUpdateDays | `chrono::Utc::now()` and `(next - now).num_days().max(0)`, Kotlin `until` truncation equals chrono `num_days` truncation toward zero |
| `kotlinx.collections.immutable` | lists | `Vec<T>` / `Arc<[T]>` |
| `@Immutable` / `@Stable` | compose recomposition | not needed |
| `StateFlow.distinctUntilChanged` | chapter list | snapshot equality check: `if new == old { return }` on `EntryState` (derive `PartialEq` on the snapshot, or compare a hash) |

---

## 5. Crates to add (and why)

| Crate | Version (proposal, verify on add) | Why |
|---|---|---|
| `tokio` | 1.x with `rt-multi-thread`, `macros`, `sync`, `time` | DB/network worker |
| `async-channel` | 2.x | `Send` channel bridge tokio ↔ GTK main context |
| `rusqlite` | 0.3x with `bundled` | local DB (area:03/05) |
| `reqwest` | 0.12.x, `rustls-tls`, `json` | source HTTP (area:12) |
| `serde` / `serde_json` | already present | |
| `thiserror` | 2.x | `StoreError` enum |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | logging (replaces `logcat`) |
| `regex` | 1.x | description collapse |
| `chrono` | 0.4 | relative days and dates |
| `rust_decimal` | 1.x | HALF_EVEN number formatting matching DecimalFormat |
| `icu_collator` | 2.x | primary-strength collation |
| `image` | 0.25 (only `jpeg`, `png`, `webp`) | backdrop blur (gaussian on 1/4 scale) and cover resize when needed |
| `ashpd` | 0.9+ | portal OpenURI for "Open with…", optional |
| `webkit6` | 0.4+ (optional feature `webview`) | in-app WebView (area:36), optional |

Avoid: `libadwaita` (`adw`), `async_trait` (not needed: use channels), `relm4` (out of scope: mockup is plain gtk4-rs), `diesel` (rusqlite chosen).

---

## 6. Integration with the existing mockup

1. `src/entry.rs::entry(title, anime)` remains the public entry point for the anime side. For manga, call `details::entry_screen(manga_id, from_source)` from the same route. Keep the `tag(&p, id)` convention (`manga_entry_details` id) so existing `screen(id)` lookups work.
2. `spec/entries.json` remains the source of truth for menu labels: `screen("manga_entry_details").menu` (Refresh, Edit categories, Migrate, Share, Settings) and sheet copy (`chapter_settings` tabs Filter/Sort/Display). Keep reading it in `details::view::toolbar` through `crate::ui::screen`. Do not duplicate strings in Rust beyond `strings.rs`, which only holds keys Aniyomi uses and the English text; where the spec and strings.rs differ, spec wins until the spec is regenerated.
3. `dialog_of` / `sheet_of` in `entry.rs` become thin wrappers that open the dialog modules in `details/dialogs/` for manga and keep the mockup behaviour for anime.
4. Replace the 24-row fake list and hard-coded genres/author with `EntryState` derived values.
5. `ui::page(title, content, end, back)` is reused for the top bar; its `end` vector receives the download menu button, filter button, and overflow menu button in that order (matches `entry.rs` line 226-240 order).

---

## 7. Error handling

- `StoreError` (thiserror):

```rust
#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("database: {0}")] Db(#[from] rusqlite::Error),
    #[error("source: {0}")] Source(String),        // message from source; shown as-is (Aniyomi shows e.formattedMessage)
    #[error("no chapters")] NoChapters,            // NoChaptersException equivalent
    #[error("io: {0}")] Io(#[from] std::io::Error),
}
```

- Toast mapping:
  - `Source` → toast with the message (Aniyomi `formattedMessage`).
  - `NoChapters` → toast with the text of the Aniyomi message key for `NoChaptersException` (part 2 identifies it; use the key from the existing strings table).
  - `Db`, `Io` → toast "Error" plus `tracing::error!`. Do not expose SQL text.
- HTTP 103 (early hints) is ignored silently (MangaScreenModel.kt:286-288).
- Cover save/share/edit errors use the four cover messages (behavior 8.5). Map `Io` and any other error to the same messages Aniyomi uses (`error_saving_cover`, `error_sharing_cover`, `notification_cover_update_failed`).
- Panics: none in pure logic. Use `checked` integer conversions where a chapter number is converted to `i64` for huge values (> 2^53), saturating like Kotlin's `toInt()`.

---

## 8. strings.rs

- A `pub fn t(key: &'static str) -> &'static str` table keyed by the Aniyomi resource names used in this area, English text copied from the base XML.
- `pub fn plural(key: &'static str, n: i64) -> String` supporting `one` (n == 1) and `other`. Plural keys used here: `day`, `manga_num_chapters`, `num_trackers`, `missing_items` (AYMR).
- Keys (complete for this area): `action_resume`, `action_start`, `display_mode_chapter`, `chapter_progress`, `unread`, `action_filter_bookmarked`, `manga_cover`, `in_library`, `add_to_library`, `not_applicable`, `manga_interval_expected_update_soon`, `manga_tracking_tab`, `action_web_view`, `action_search`, `action_copy_to_clipboard`, `description_placeholder`, `manga_info_collapse`, `manga_info_expand`, `unknown_title`, `unknown_author`, `ongoing`, `completed`, `licensed`, `publishing_finished`, `cancelled`, `on_hiatus`, `unknown`, `action_start_downloading_now`, `action_download`, `action_delete`, `manga_download`, `action_mark_as_read`, `action_mark_as_unread`, `action_mark_previous_as_read`, `action_bookmark`, `action_remove_bookmark`, `action_move_category`, `action_select_all`, `action_select_inverse`, `action_filter`, `action_edit_categories`, `action_migrate`, `action_share`, `action_webview_refresh`, `chapter_settings`, `confirm_set_chapter_settings`, `set_chapter_settings_as_default`, `action_reset`, `label_downloaded`, `action_filter_unread`, `scanlator`, `sort_by_source`, `sort_by_number`, `sort_by_upload_date`, `action_sort_alpha`, `show_title`, `show_chapter_number`, `label_default`, `manga_interval_expected_update`, `manga_interval_expected_update_null`, `manga_interval_custom_amount`, `pref_library_update_smart_update`, `are_you_sure`, `confirm_delete_chapters`, `confirm_add_duplicate_manga`, `action_show_manga` (AYMR: "Show entry"), `action_migrate_duplicate`, `action_add_anyway`, `action_cancel`, `action_ok`, `exclude_scanlators`, `no_scanlators_found`, `cover_saved`, `error_saving_cover`, `cover_updated`, `notification_cover_update_failed`, `error_sharing_cover`, `action_close`, `action_save`, `action_edit_cover`, `action_edit`, `chapters`, `delete_downloads_for_manga`, `action_sort`, `action_display`, `also_set_chapter_settings_for_library` (AYMR), `download_error` (AYMR: "Error"), `settings` (AYMR), `missing_items` (AYMR plural), `chapter_settings_updated` (text not read in this pass; verify in part 2). Anime-only AYMR keys (`action_bookmark_episode`, `action_remove_bookmark_episode`, `action_fillermark_episode`, `action_remove_fillermark_episode`, `action_mark_as_seen`, `action_mark_as_unseen`, `action_mark_previous_as_seen`, `action_play_externally`, `action_play_internally`, `action_change_intro_length`, `confirm_delete_episodes`, `anime_interval_expected_update`, `anime_interval_expected_update_null`) are not added to this area's table.

Full English values are in `p1-data-and-config.md` and `p1-behavior.md`. Use exactly those strings. Labelled proposals (e.g. "Copied to clipboard") must be listed in `strings.rs` with a `// proposal` comment.

---

## 9. Testing hooks

- All of `flags`, `compare`, `chapters`, `numfmt`, `text`, `download`, `swipe` are pure. Test them with `cargo test` in `src/details/**` with `#[cfg(test)]`. Required cases are in `p1-edge-cases-and-tests.md` section 4.
- The GTK views are checked by the existing smoke approach (`KUROBAYOMI_SMOKE`, `KUROBAYOMI_SHOT` env vars in `src/main.rs`).

---

## 10. Pitfalls

1. **Name inversion in Kotlin.** `Manga.sortDescending()` is true when bit 0 is 0. `getChapterSort(manga, sortDescending)` for SOURCE with `true` produces ascending `sourceOrder`. Porting the names "naturally" flips the behaviour. Port the table literally (section 2.2).
2. **Swipe key swap.** `swipeChapterStartAction()` reads the *end* key. Use the key names, not the getter names.
3. **Filtered vs unfiltered.** The FAB label reads unfiltered chapters; the FAB visibility and the header count read filtered chapters; `isAnySelected` reads unfiltered. Keep three different inputs.
4. **Stable sort.** `Vec::sort_by` is stable, matching Kotlin `sortedWith`. Do not use `sort_unstable_by` for the chapter list.
5. **Floating-point compare.** Use `total_cmp` for chapter numbers. `partial_cmp().unwrap()` panics on NaN.
6. **Missing marker id string** is part of the list identity; keep the exact `"{lower}-{higher}"` format including the literal `null`.
7. **HALF_EVEN and binary doubles.** `format_chapter_number(1.005)` must match Java output. Use the exact double (`from_f64_retain`), not `from_f64` shortest-repr, which can round differently.
8. **Collator drift.** ICU and Java differ for some punctuation. Accept, test, document.
9. **Snapshot storms.** A library refresh can emit many chapter updates; coalesce (section 3).
10. **Stale timers.** The bottom bar's 1 s label reveal must cancel the previous timer on each long-press (Kotlin's `resetJob` is effectively broken; the Rust port must cancel correctly, see E-12).
11. **Downloaded filter lock.** When `downloaded_only` is on, the Downloaded filter reads `EnabledIs` regardless of stored flags, and its row is disabled. Do not persist changes while locked.
12. **Local manga.** `is_local` keeps all chapters for the downloaded filter and disables download indicator, bulk Download, and swipe Download (`downloadIndicatorEnabled = !isAnyChapterSelected && !manga.isLocal()`). Enforce in view code, not only in filters.
13. **Non-cancellable writes.** Do not cancel flag writes when the page is closed (section 3).
14. **Right-click vs long-press.** On Linux a right-click must not also fire the primary click handler.
