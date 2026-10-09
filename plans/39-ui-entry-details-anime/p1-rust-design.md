# Area 39 part 1: Rust design for the anime entry details screen

Target: Linux, GTK 4.16+ through `gtk4` 0.11.5 with the `v4_16` feature (as in `/home/blackbox/kurobayomi/Cargo.toml`), no libadwaita, edition 2024. The mockup's entry page is `src/entry.rs::entry(title, anime)`, which builds one page for manga and anime with hard-coded data. This design replaces the anime branch with a real module and keeps the manga branch untouched.

## 1. Crate and module layout

```
src/anime_details/
  mod.rs                  pub fn build(details: &AnimeDetails, cx: &ScreenCx) -> gtk::Widget
  model.rs                AnimeDetails, EpisodeItem, EpisodeRow, SeasonItem, RelationGroup, Airing
  flags.rs                EpisodeFlags, SeasonFlags, ViewerFlags, TriState (all const masks)
  format.rs               format_episode_number(f64), format_time_ms(i64), format_airing(i64)
  missing.rs              missing_entries_count(&[f64]) -> u32
  selection.rs            SelectionState, bottom_actions(&[EpisodeItem], &MenuCtx) -> Vec<BottomAction>
  related_pack.rs         pack_rows(&[RelationGroup], max_per_row) -> Vec<Vec<usize>>  (exact port)
  download_action.rs      EpisodeDownloadAction, download_click_action(state, click) -> EpisodeDownloadAction
  status.rs               status_icon(i64) -> &'static str, status_label_key(i64) -> &'static str
  source_name.rs          display_source_name(&SourceInfo, &[String] enabled_langs) -> String
  view/
    mod.rs
    info_box.rs           backdrop, cover, title, author, artist, status row, source
    action_row.rs         four action buttons
    description.rs        collapsible description + More/Less
    tags.rs               genre chips + Search/Copy menu
    related.rs            small packed rows + multi horizontal scrollers
    episode_list.rs       gio::ListStore<EpisodeRowObject> + ListView factory
    episode_row.rs        one row (simple and rich layouts), selection background
    airing_row.rs         countdown row (glib timeout)
    swipe.rs              GestureDrag reveal + threshold
    download_indicator.rs five visual states, gestures, progress ring (DrawingArea)
    season_grid.rs        gtk::GridView of season tiles, adaptive columns
    season_item.rs        four display modes
    bottom_bar.rs         selection action bar
    fab.rs                Start / Resume overlay button
  dialogs/
    episode_settings.rs   Filter / Sort / Display window (gtk::Notebook)
    season_settings.rs    Filter / Sort / Display window
    set_default.rs        "Set as default" confirm + checkbox
    duplicate.rs          Duplicate-add choice window
  quality/
    model.rs              HosterState, QualityModel state machine (no GTK)
    view.rs               quality window, hoster list, video list, actions
  images/
    model.rs              ImageModel: save, share, edit, delete (no GTK except Texture)
    dialog.rs             two-page image viewer window
```

Rule: everything under `model.rs`, `flags.rs`, `format.rs`, `missing.rs`, `selection.rs`, `related_pack.rs`, `download_action.rs`, `status.rs`, `source_name.rs`, `quality/model.rs`, `images/model.rs` has no `gtk` import. It is unit-testable without a display.

Wiring into the mockup (`src/main.rs` `mod` list, `src/entry.rs`):
- Add `mod anime_details;` to `src/main.rs`.
- In `src/entry.rs::entry(title, anime)`: when `anime` is true, return `crate::anime_details::build(&details, &cx)` wrapped in the same `page(title, &body, end, true)` and `tag(&p, "anime_entry_details")`. The `end` menu items (Next N episodes, Filter, overflow menu) move into `anime_details::build`.
- `src/entry.rs::body()` keeps its match arms. Add `"anime_entry_details" => anime_details::page_body(...)` only if the spec-driven route is needed. Prefer a direct call.
- `src/spec.rs` / `spec/entries.json` provide dialog and sheet texts for the mockup only. The real screen uses the message keys from p1-behavior.md, so these strings move to a Rust string table (`src/i18n_anime.rs`, section 9).

## 2. Crates and why

| Crate | Version (proposal) | Used for | Why this one |
|---|---|---|---|
| `gtk4` (already) | 0.11.5, features `v4_16` | all widgets | already in the project; gives `gtk::Revealer`, `gtk::ListView`, `gtk::GridView`, `gtk::AlertDialog` (4.10+), `gtk::FileLauncher` (4.10+), `gtk::Overlay` |
| `glib` (re-export `gtk::glib`) | via gtk4 | main-loop spawn, timeouts | `glib::spawn_future_local`, `glib::timeout_add_local` |
| `async-channel` | 2 | tokio to GTK-thread messages | bounded/unbounded channels usable from both sides without tokio in the GTK thread |
| `tokio` | 1, features `rt-multi-thread,macros,sync,time` | background work (source calls, file IO) | the project spec mandates tokio |
| `rusqlite` | 0.32 (`bundled`) | local DB (area 04 schema) | spec mandates rusqlite; `bundled` avoids system libsqlite version drift |
| `reqwest` | 0.12 (`rustls-tls`) | HTTP for sources, covers (area 12) | spec mandates reqwest; rustls avoids OpenSSL linkage issues on CachyOS |
| `serde`, `serde_json` | already present | JSON (backup, `memo`, `genre` JSON fallback) | already present |
| `md-5` | 0.10 | cache file names (`md5(anime_id)`) | keeps Android cache file names so migrated custom covers are found without rename |
| `infer` | 0.16 | image type from magic bytes (`ImageUtil.findImageType` equivalent) | tiny, no image decoding needed just for the type |
| `tempfile` | 3 | share temp files | auto-cleanup |
| `thiserror` | 2 | `DetailsError` | ergonomic error enum |
| `tracing`, `tracing-subscriber` | 0.1 / 0.3 | error log (`logcat` equivalent) | structured logs, `RUST_LOG` filter |

Not used: `bitflags` (a newtype with consts is shorter and exactly matches the Kotlin masks), `chrono`/`time` (plain `i64` milliseconds are enough; formatting is in `format.rs`), `rust_decimal` (see section 8 for number formatting), `libadwaita` (forbidden by the target).

## 3. Core data types

### 3.1 Flags (`flags.rs`)

```rust
//! Exact port of eu.kanade...Anime flag masks. Values are the Kotlin constants.
use std::ops::BitAnd;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EpisodeFlags(pub i64);

impl EpisodeFlags {
    pub const SORT_DIR_MASK: i64 = 0x0000_0001;   // 0 = desc, 1 = asc
    pub const SORT_ASC: i64 = 0x0000_0001;
    pub const SORT_DESC: i64 = 0x0000_0000;
    pub const UNSEEN_MASK: i64 = 0x0000_0006;
    pub const SHOW_UNSEEN: i64 = 0x0000_0002;
    pub const SHOW_SEEN: i64 = 0x0000_0004;
    pub const DOWNLOADED_MASK: i64 = 0x0000_0018;
    pub const SHOW_DOWNLOADED: i64 = 0x0000_0008;
    pub const SHOW_NOT_DOWNLOADED: i64 = 0x0000_0010;
    pub const BOOKMARKED_MASK: i64 = 0x0000_0060;
    pub const SHOW_BOOKMARKED: i64 = 0x0000_0020;
    pub const SHOW_NOT_BOOKMARKED: i64 = 0x0000_0040;
    pub const FILLERMARKED_MASK: i64 = 0x0000_0180;
    pub const SHOW_FILLERMARKED: i64 = 0x0000_0080;
    pub const SHOW_NOT_FILLERMARKED: i64 = 0x0000_0100;
    pub const SORTING_MASK: i64 = 0x0000_0600;
    pub const SORTING_SOURCE: i64 = 0x0000_0000;
    pub const SORTING_NUMBER: i64 = 0x0000_0200;
    pub const SORTING_UPLOAD_DATE: i64 = 0x0000_0400;
    pub const SORTING_ALPHABET: i64 = 0x0000_0600;
    pub const PREVIEWS_MASK: i64 = 0x0000_0800;
    pub const SHOW_NOT_PREVIEWS: i64 = 0x0000_0800;   // 0 means previews shown
    pub const SUMMARIES_MASK: i64 = 0x0000_1000;
    pub const SHOW_NOT_SUMMARIES: i64 = 0x0000_1000;  // 0 means summaries shown
    pub const DISPLAY_MASK: i64 = 0x0010_0000;
    pub const DISPLAY_NUMBER: i64 = 0x0010_0000;      // 0 means display name

    pub fn sort_descending(self) -> bool { self.0 & Self::SORT_DIR_MASK == Self::SORT_DESC }
    pub fn sorting(self) -> i64 { self.0 & Self::SORTING_MASK }
    pub fn display_by_number(self) -> bool { self.0 & Self::DISPLAY_MASK == Self::DISPLAY_NUMBER }
    pub fn show_previews(self) -> bool { self.0 & Self::PREVIEWS_MASK == 0 }
    pub fn show_summaries(self) -> bool { self.0 & Self::SUMMARIES_MASK == 0 }
    pub fn unseen(self) -> TriState { tri(self.0 & Self::UNSEEN_MASK, Self::SHOW_UNSEEN, Self::SHOW_SEEN) }
    pub fn downloaded(self) -> TriState { tri(self.0 & Self::DOWNLOADED_MASK, Self::SHOW_DOWNLOADED, Self::SHOW_NOT_DOWNLOADED) }
    pub fn bookmarked(self) -> TriState { tri(self.0 & Self::BOOKMARKED_MASK, Self::SHOW_BOOKMARKED, Self::SHOW_NOT_BOOKMARKED) }
    pub fn fillermarked(self) -> TriState { tri(self.0 & Self::FILLERMARKED_MASK, Self::SHOW_FILLERMARKED, Self::SHOW_NOT_FILLERMARKED) }

    /// Replace the bits under `mask` with `value` (value must already be inside `mask`).
    pub fn with(self, mask: i64, value: i64) -> Self { Self((self.0 & !mask) | value) }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeasonFlags(pub i64);

impl SeasonFlags {
    pub const SORT_DIR_MASK: i64 = 0x0000_0001;
    pub const SORT_DESC: i64 = 0x0000_0000;
    pub const DOWNLOADED_MASK: i64 = 0x0000_0006;
    pub const SHOW_DOWNLOADED: i64 = 0x0000_0002;
    pub const SHOW_NOT_DOWNLOADED: i64 = 0x0000_0004;
    pub const UNSEEN_MASK: i64 = 0x0000_0018;
    pub const SHOW_UNSEEN: i64 = 0x0000_0008;
    pub const SHOW_SEEN: i64 = 0x0000_0010;
    pub const STARTED_MASK: i64 = 0x0000_0060;
    pub const SHOW_STARTED: i64 = 0x0000_0020;
    pub const SHOW_NOT_STARTED: i64 = 0x0000_0040;
    pub const COMPLETED_MASK: i64 = 0x0000_0180;
    pub const SHOW_COMPLETED: i64 = 0x0000_0080;
    pub const SHOW_NOT_COMPLETED: i64 = 0x0000_0100;
    pub const BOOKMARKED_MASK: i64 = 0x0000_0600;
    pub const SHOW_BOOKMARKED: i64 = 0x0000_0200;
    pub const SHOW_NOT_BOOKMARKED: i64 = 0x0000_0400;
    pub const FILLERMARKED_MASK: i64 = 0x0000_1800;
    pub const SHOW_FILLERMARKED: i64 = 0x0000_0800;
    pub const SHOW_NOT_FILLERMARKED: i64 = 0x0000_1000;
    pub const SORT_MASK: i64 = 0x0000_E000;
    pub const SORT_SOURCE: i64 = 0x0000_0000;
    pub const SORT_SEASON: i64 = 0x0000_2000;
    pub const SORT_UPLOAD: i64 = 0x0000_4000;
    pub const SORT_ALPHABET: i64 = 0x0000_6000;
    pub const SORT_COUNT: i64 = 0x0000_8000;
    pub const SORT_LAST_SEEN: i64 = 0x0000_A000;
    pub const SORT_FETCHED: i64 = 0x0000_C000;
    pub const GRID_MODE_SHIFT: u32 = 16;
    pub const GRID_MODE_MASK: i64 = 0x0003_0000;
    pub const GRID_SIZE_SHIFT: u32 = 18;
    pub const GRID_SIZE_MASK: i64 = 0x003C_0000;
    pub const OVERLAY_DOWNLOADED: i64 = 0x0040_0000;
    pub const OVERLAY_UNSEEN: i64 = 0x0080_0000;
    pub const OVERLAY_LOCAL: i64 = 0x0100_0000;
    pub const OVERLAY_LANG: i64 = 0x0200_0000;
    pub const OVERLAY_CONT: i64 = 0x0400_0000;
    pub const DISPLAY_MODE_MASK: i64 = 0x0800_0000;
    pub const DISPLAY_MODE_NUMBER: i64 = 0x0800_0000;

    pub fn grid_mode(self) -> Result<SeasonDisplayMode, FlagError> {
        SeasonDisplayMode::from_i64((self.0 & Self::GRID_MODE_MASK) >> Self::GRID_MODE_SHIFT)
    }
    pub fn grid_size(self) -> u8 { ((self.0 & Self::GRID_SIZE_MASK) >> Self::GRID_SIZE_SHIFT) as u8 }
    pub fn overlay(self, bit: i64) -> bool { self.0 & bit == bit }
    pub fn display_by_number(self) -> bool { self.0 & Self::DISPLAY_MODE_MASK == Self::DISPLAY_MODE_NUMBER }
    pub fn sort_descending(self) -> bool { self.0 & Self::SORT_DIR_MASK == Self::SORT_DESC }
    pub fn sorting(self) -> i64 { self.0 & Self::SORT_MASK }
    pub fn with(self, mask: i64, value: i64) -> Self { Self((self.0 & !mask) | value) }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewerFlags(pub i64);

impl ViewerFlags {
    // Literals copied digit for digit from Anime.kt:310-313 (no regrouping).
    pub const INTRO_MASK: i64 = 0x0000000000000FF;
    pub const AIRING_EPISODE_MASK: i64 = 0x000000000FFFF00;
    pub const AIRING_TIME_MASK: i64 = 0x0FFFFFFFF000000;
    pub const INTRO_DISABLE_MASK: i64 = 0x100000000000000;

    pub fn skip_intro_length(self) -> i64 { self.0 & Self::INTRO_MASK }
    pub fn next_episode_to_air(self) -> i64 { (self.0 & Self::AIRING_EPISODE_MASK) >> 8 }
    pub fn next_episode_airing_at(self) -> i64 { (self.0 & Self::AIRING_TIME_MASK) >> 24 }
    pub fn skip_intro_disabled(self) -> bool { self.0 & Self::INTRO_DISABLE_MASK == Self::INTRO_DISABLE_MASK }
}
```

The viewer masks are the Kotlin literals copied digit for digit. `AIRING_TIME_MASK` covers bits 24 to 55 (eight `F` digits), so it does not overlap `INTRO_DISABLE_MASK` (bit 56). Test T-F1 in p1-edge-cases-and-tests.md asserts this.

`TriState` and the `tri` helper:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState { Disabled, EnabledIs, EnabledNot }

fn tri(masked: i64, is_bit: i64, not_bit: i64) -> TriState {
    if masked == is_bit { TriState::EnabledIs }
    else if masked == not_bit { TriState::EnabledNot }
    else { TriState::Disabled }
}

#[derive(Debug, thiserror::Error)]
pub enum FlagError {
    #[error("Invalid sorting value")]   // same text as the Kotlin exception
    InvalidSeasonDisplayMode(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonDisplayMode { CompactGrid, ComfortableGrid, CoverOnlyGrid, List }

impl SeasonDisplayMode {
    pub fn from_i64(v: i64) -> Result<Self, FlagError> {
        Ok(match v {
            0 => Self::CompactGrid,
            1 => Self::ComfortableGrid,
            2 => Self::CoverOnlyGrid,
            3 => Self::List,
            other => return Err(FlagError::InvalidSeasonDisplayMode(other)),
        })
    }
    pub fn to_i64(self) -> i64 { self as i64 }
}
```

Note: `SeasonDisplayMode` is a Rust enum here (the Kotlin is a sealed interface). Its discriminants 0..3 match `toLong`. `grid_mode` on a flags value with the two bits set to 0..3 can never fail; the `Err` path is only reachable with a corrupt value, which the mask cannot produce. The `Result` keeps the Kotlin behaviour explicit.

### 3.2 Model (`model.rs`)

```rust
pub struct AnimeDetails {
    pub anime: Anime,                      // see p1-data-and-config.md section 1.1
    pub source: SourceInfo,
    pub is_from_source: bool,              // ScreenModel decides (part 2)
    pub episodes: Vec<EpisodeRow>,         // processed (filtered + sorted + separators), part 2
    pub seasons: Vec<SeasonItem>,          // processed, part 2
    pub tracking_count: u32,
    pub has_logged_in_trackers: bool,
    pub related: Vec<RelationGroup>,
    pub airing: Option<Airing>,            // None when airingTime <= 0 (state.airingTime)
    pub next_update_ms: Option<i64>,       // Instant? in Kotlin, converted to unix ms
    pub refreshing: bool,
}

pub struct SourceInfo {
    pub id: i64,
    pub name: String,
    pub lang: String,
    pub is_stub: bool,                     // StubAnimeSource
    pub is_configurable: bool,             // ConfigurableAnimeSource
    pub is_local: bool,                    // LocalAnimeSource.ID
}

#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeItem {
    pub episode: Episode,                  // see p1-data-and-config.md section 1.2
    pub download: DownloadState,
    pub download_progress: i32,            // 0..=100 (Kotlin Int)
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EpisodeRow {
    Missing { id: String, count: u32 },
    Item(EpisodeItem),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeasonItem {
    pub anime: Anime,                      // the season entry (SeasonAnime.anime)
    pub total: i64,
    pub seen: i64,
    pub bookmarks: i64,
    pub fillermarks: i64,
    pub download_count: i64,               // -1 = not computed
    pub unseen_count: i64,                 // -1 = not computed
    pub is_local: bool,
    pub source_language: String,
    pub show_continue_overlay: bool,
}

impl SeasonItem {
    pub fn is_seen(&self) -> bool { self.total == self.seen }
    pub fn unseen(&self) -> i64 { self.total - self.seen }
    pub fn has_started(&self) -> bool { self.seen > 0 }
}

pub struct RelationGroup { pub name: String, pub anime: Vec<Anime> }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Airing { pub episode_number: f64, pub at_ms: i64 }  // at_ms: absolute unix ms
```

`Airing::at_ms` replaces the Kotlin `airingTime` countdown. The countdown computes `remaining = at_ms - now_ms` each tick instead of decrementing a stored value (fixes defect D-8 in p1-edge-cases-and-tests.md).

### 3.3 Download state (`download_action.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState { NotDownloaded = 0, Queue = 1, Downloading = 2, Downloaded = 3, Error = 4 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpisodeDownloadAction { Start, StartNow, Cancel, Delete, ShowQualities }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadClick { Primary, Secondary /* long press */, MenuStartNow, MenuCancel, MenuDelete }

/// Mirrors EpisodeDownloadIndicator.kt. Returns None when the click opens a menu only.
pub fn download_click_action(state: DownloadState, click: DownloadClick) -> Option<EpisodeDownloadAction> {
    use DownloadClick::*;
    use DownloadState::*;
    match (state, click) {
        (NotDownloaded, Primary) => Some(EpisodeDownloadAction::Start),
        (NotDownloaded, Secondary) => Some(EpisodeDownloadAction::ShowQualities),
        (Queue | Downloading, Primary) => None,                 // opens the menu
        (Queue | Downloading, Secondary) => Some(EpisodeDownloadAction::Cancel),
        (Queue | Downloading, MenuStartNow) => Some(EpisodeDownloadAction::StartNow),
        (Queue | Downloading, MenuCancel) => Some(EpisodeDownloadAction::Cancel),
        (Downloaded, Primary | Secondary) => None,              // opens the delete menu
        (Downloaded, MenuDelete) => Some(EpisodeDownloadAction::Delete),
        (Error, Primary | Secondary) => Some(EpisodeDownloadAction::Start),
        _ => None,
    }
}
```

Exactness check against the source: `EpisodeDownloadIndicator.kt` lines 84-109 (not downloaded: tap START, long press SHOW_QUALITIES), 111-189 (queue or downloading: tap opens menu, long press CANCEL, menu START_NOW and CANCEL), 191-225 (downloaded: tap and long press open menu with DELETE), 227-251 (error: tap and long press START).

### 3.4 Selection and bottom actions (`selection.rs`)

```rust
pub struct MenuCtx { pub always_use_external_player: bool, pub download_available: bool }

#[derive(Clone, Debug, PartialEq)]
pub enum BottomAction {
    Bookmark(bool),          // true = bookmark, false = remove
    Fillermark(bool),
    MarkSeen(bool),          // true = mark viewed, false = mark unviewed
    MarkPreviousSeen,        // only when exactly one selected
    Download,                // START for all selected
    Delete,                  // delete all selected
    PlayExternal,            // alt = true
    PlayInternal,            // alt = true
}

/// Predicates copied from AnimeScreen.kt:964-1020.
pub fn bottom_actions(selected: &[&EpisodeItem], ctx: &MenuCtx) -> Vec<BottomAction> {
    if selected.is_empty() { return vec![]; }
    let any = |f: fn(&EpisodeItem) -> bool| selected.iter().any(|e| f(e));
    let all = |f: fn(&EpisodeItem) -> bool| selected.iter().all(|e| f(e));
    let mut out = Vec::new();
    if any(|e| !e.episode.bookmark) { out.push(BottomAction::Bookmark(true)); }
    if all(|e| e.episode.bookmark) { out.push(BottomAction::Bookmark(false)); }
    if any(|e| !e.episode.fillermark) { out.push(BottomAction::Fillermark(true)); }
    if all(|e| e.episode.fillermark) { out.push(BottomAction::Fillermark(false)); }
    if any(|e| !e.episode.seen) { out.push(BottomAction::MarkSeen(true)); }
    if any(|e| e.episode.seen || e.episode.last_second_seen > 0) { out.push(BottomAction::MarkSeen(false)); }
    if selected.len() == 1 { out.push(BottomAction::MarkPreviousSeen); }
    if ctx.download_available && any(|e| e.download != DownloadState::Downloaded) { out.push(BottomAction::Download); }
    if ctx.download_available && any(|e| e.download == DownloadState::Downloaded) { out.push(BottomAction::Delete); }
    if !ctx.always_use_external_player && selected.len() == 1 { out.push(BottomAction::PlayExternal); }
    if ctx.always_use_external_player && selected.len() == 1 { out.push(BottomAction::PlayInternal); }
    out
}
```

Note: `fn` pointers are used so the closures stay non-capturing. The Kotlin `fastAny` on `selected` returns false for empty, but the early return keeps the same visible result.

Selection state:

```rust
#[derive(Default)]
pub struct SelectionState {
    selected: std::collections::HashSet<i64>,   // episode ids
    anchor: Option<i64>,                        // last clicked id for Shift range
}

impl SelectionState {
    pub fn is_any(&self) -> bool { !self.selected.is_empty() }
    pub fn count(&self) -> usize { self.selected.len() }
    pub fn toggle(&mut self, id: i64, selected: bool) { if selected { self.selected.insert(id); } else { self.selected.remove(&id); } self.anchor = Some(id); }
    pub fn select_all(&mut self, ids: impl Iterator<Item = i64>, on: bool) { if on { self.selected.extend(ids) } else { self.selected.clear() } }
    pub fn invert(&mut self, all_ids: &[i64]) { for id in all_ids { if !self.selected.remove(id) { self.selected.insert(*id); } } }
    pub fn clear(&mut self) { self.selected.clear(); self.anchor = None; }
}
```

Shift-range (desktop addition, not in Kotlin): when Shift-click, select from `anchor` to clicked id in the displayed order (inclusive). Kept out of the Kotlin parity list; document in ui-parity.

### 3.5 Related packing (`related_pack.rs`)

```rust
/// Exact port of RelatedAnimeRows.kt packRows. Returns indexes into `sizes`.
pub fn pack_rows(sizes: &[usize], max_per_row: usize) -> Vec<Vec<usize>> {
    if sizes.is_empty() { return vec![]; }
    let total: usize = sizes.iter().sum();
    let row_count = (total as f64 / max_per_row as f64).ceil() as usize;   // ceil(total / 3.0)
    let base = total / row_count;
    let remainder = total % row_count;
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut filled = 0usize;
    let mut cap = base + if remainder > 0 { 1 } else { 0 };
    for (i, &size) in sizes.iter().enumerate() {
        if !current.is_empty() && filled + size > cap {
            rows.push(std::mem::take(&mut current));
            filled = 0;
            cap = base + if rows.len() < remainder { 1 } else { 0 };
            if cap < size { cap = max_per_row; }
        }
        current.push(i);
        filled += size;
    }
    if !current.is_empty() { rows.push(current); }
    rows
}
```

`row_count` is never 0 when `sizes` is non-empty and every size is at least 1. Guard: if a group has `size == 0`, Kotlin's `total` can still be non-zero; if `total == 0` (all groups empty), `row_count` is 0 and `total / row_count` panics in Rust and throws `ArithmeticException` in Kotlin. The Rust port returns `vec![]` when `total == 0`. Add that guard (it is a deliberate deviation for an input the UI never produces, because empty relation groups are not returned by the source; see E-12).

### 3.6 Missing count (`missing.rs`)

```rust
pub fn missing_entries_count(numbers: &[f64]) -> u32 {
    let mut ints: Vec<i64> = numbers.iter().filter(|&&n| n != -1.0).map(|&n| n as i64).collect();
    ints.sort_unstable();
    ints.dedup();
    let mut missing: u32 = 0;
    let mut prev: i64 = 0;
    for &cur in &ints {
        if cur > prev + 1 { missing += (cur - prev - 1) as u32; }
        prev = cur;
    }
    missing
}
```

`n as i64` truncates toward zero, like Kotlin `Double.toInt()`. The Kotlin code returns 0 early for an empty input and for an input whose filtered list is empty; the loop above gives the same result. `NaN as i64` is 0 in Rust and `NaN.toInt()` is 0 in Kotlin, so it matches.

### 3.7 Formatting (`format.rs`)

```rust
/// DecimalFormat("#.###") with '.' decimal separator, HALF_EVEN, ASCII digits (see section 8).
pub fn format_episode_number(n: f64) -> String {
    if !n.is_finite() { return "0".to_string(); }   // unreachable in the UI; keeps Rust total
    let s = format!("{:.3}", n);                    // Rust rounds the exact binary value
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" { "0".into() } else { s.into() }
}

/// formatTime(milliseconds, useDayFormat = false) from AnimeScreen.kt:1153-1182.
pub fn format_time_ms(ms: i64) -> String {
    if ms > 3_600_000 {
        let h = ms / 3_600_000;
        let m = (ms / 60_000) % 60;
        let s = (ms / 1_000) % 60;
        format!("{h}:{m:02}:{s:02}")
    } else {
        let m = ms / 60_000;
        let s = (ms / 1_000) % 60;
        format!("{m}:{s:02}")
    }
}

/// formatTime(milliseconds, useDayFormat = true): the Airing string.
pub fn format_airing(ms: i64) -> String {
    let days = ms / 86_400_000;
    let hours = (ms / 3_600_000) - days * 24;
    let minutes = (ms / 60_000) - (ms / 3_600_000) * 60;
    let seconds = (ms / 1_000) - (ms / 60_000) * 60;
    format!("Airing in {days:02}d {hours:02}h {minutes:02}m {seconds:02}s")
}
```

Checks: `format_time_ms(3_600_000)` = `"60:00"` (matches Kotlin); `format_time_ms(3_600_001)` = `"1:00:00"`. For negative `ms` Kotlin gives signed output and Rust integer division truncates toward zero, which matches Java's `TimeUnit` truncation for small negatives; the UI never passes negatives (guarded by `> 0`). Format width: `{m}` is unpadded, matching `%d`.

### 3.8 Status and source name (`status.rs`, `source_name.rs`)

```rust
pub fn status_label_key(status: i64) -> &'static str {
    match status {
        1 => "ongoing", 2 => "completed", 3 => "licensed", 4 => "publishing_finished",
        5 => "cancelled", 6 => "on_hiatus", 7 => "upcoming", _ => "unknown",
    }
}
pub fn status_icon_name(status: i64) -> &'static str {        // GTK symbolic icon names
    match status {
        1 => "document-open-recent-symbolic",     // Schedule
        2 => "emblem-ok-symbolic",                // DoneAll
        3 => "emblem-money-symbolic",             // AttachMoney
        4 => "object-select-symbolic",            // Done
        5 => "window-close-symbolic",             // Close
        6 => "media-playback-pause-symbolic",     // Pause
        7 => "alarm-symbolic",                    // ic_calendar_clock (closest stock icon)
        _ => "action-unavailable-symbolic",       // Block
    }
}
```

`status_label_key` keys map to `MR.strings.<key>` for values 1..6 and `AYMR.strings.upcoming` for 7; `unknown` is `MR.strings.unknown`. Icon names are stock GNOME/Adwaita symbolic icons; the Kotlin icon set is Material. Documented as a visual difference in ui-parity.

```rust
/// getNameForAnimeInfo (AnimeSourceExtensions.kt:18-31).
pub fn display_source_name(name: &str, lang: &str, enabled_langs: &[String]) -> String {
    let enabled: Vec<&String> = enabled_langs.iter().filter(|l| l.as_str() != "all" && l.as_str() != "other").collect();
    let one_active = enabled.len() == 1;
    let in_enabled = enabled.iter().any(|l| l.as_str() == lang);
    if one_active && in_enabled { name.to_string() }
    else { format!("{name} ({})", lang.to_uppercase()) }
}
```

Mapping check: Kotlin `hasOneActiveLanguages && !isInEnabledLanguages -> toString()` and `else -> toString()` both give `"{name} ({LANG})"`; the only `name`-only case is `hasOneActiveLanguages && isInEnabledLanguages`. The Rust version has the same three-way result. `toUpperCase` vs `uppercase`: Rust `to_uppercase()` is Unicode-aware like Kotlin `uppercase()` in ordinary cases.

## 4. Threading and async model

Three execution contexts:

1. GTK main thread (the default `glib::MainContext`). Owns all widgets, `Rc<RefCell<ScreenState>>`, the `SelectionState`, and every `gtk::` call.
2. tokio runtime (multi-thread, created once in `main` and stored in a `OnceLock<tokio::runtime::Runtime>`). Runs source calls, image downloads, file writes, and DB work through `spawn_blocking` for rusqlite.
3. Dedicated DB thread (optional, recommended): one `rusqlite::Connection` owned by a thread that reads commands from a `std::sync::mpsc` channel. Avoids `Send` issues with `Connection` across await points.

Bridge: `async_channel::unbounded::<UiEvent>()`. Background tasks `send` `UiEvent`s; the GTK side drains them with:

```rust
let (tx, rx) = async_channel::unbounded::<UiEvent>();
glib::spawn_future_local(async move {
    while let Ok(ev) = rx.recv().await {
        handle_ui_event(&state, ev);
    }
});
```

`UiEvent` is a closed enum (`EpisodesChanged`, `SeasonsChanged`, `HosterStateChanged { idx, state }`, `VideoSelected { hoster, video }`, `ImageUpdated(ImageKind)`, `Snackbar(String)`, `Toast(String)`, `ConfirmDuplicate`, ...). It must be `Send`.

Command path: GTK handler builds a `DetailsCommand` and `spawn`s it on the tokio runtime: `runtime.spawn(async move { let res = handler.run(cmd).await; tx.send(UiEvent::from(res)).await })`.

Concurrency rules:
- Never block the GTK thread on tokio or the DB. No `block_on` in signal handlers.
- Hoster loading fans out with `futures::future::join_all` (or `tokio::task::JoinSet`) and each completion sends one `HosterStateChanged`. This mirrors `mapIndexed { async {...} }.awaitAll()` and the per-hoster `updateAt`.
- Cancel-on-leave: store a `tokio_util::sync::CancellationToken` in the screen state and cancel it on widget destroy (equivalent of `screenModelScope` cancellation). Add `tokio-util` 0.7.
- Countdown: `glib::timeout_add_local(Duration::from_secs(1), ...)` returns `SourceId`; remove it on `destroy`. It reads `Airing::at_ms` and the system clock.

## 5. Quality (`quality/model.rs`) state machine (pure Rust)

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum HosterState {
    Idle { name: String },
    Loading { name: String },
    Error { name: String },
    Ready { name: String, videos: Vec<Video>, video_states: Vec<VideoState> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoState { Queue, LoadVideo, Ready, Error }

pub struct QualityModel {
    pub hosters: Option<Result<Vec<HosterState>, QualityError>>,  // Result<List<HosterState>>? in Kotlin
    pub expanded: Vec<bool>,
    pub selected: Option<(usize, usize)>,       // (-1,-1) in Kotlin is None
    pub current: Option<Video>,
    pub show_all_qualities: bool,
    hoster_list: Vec<Hoster>,
    found_preferred: bool,                        // AtomicBoolean in Kotlin
}

#[derive(Debug, thiserror::Error)]
pub enum QualityError {
    #[error("No available videos")] NoAvailableVideos,   // NoSuchElementException("No available videos")
    #[error("{0}")] Load(String),
}

/// HosterLoader.selectBestVideo (HosterLoader.kt:27-67). Returns (hoster_idx, video_idx) or None.
pub fn select_best_video(states: &[HosterState]) -> Option<(usize, usize)>;
```

`select_best_video` must reproduce the two passes exactly:
1. Pass 1: first hoster (among `Ready`) that has a video with `preferred && (state == READY || state == QUEUE)`; within it, the first such video.
2. Pass 2: first hoster that has a video with `!url.is_empty() && (state == READY || state == QUEUE)`; within it, the first such video.
3. Otherwise `None` (Kotlin `Pair(-1, -1)`).

`load_video` logic (Kotlin `loadVideo`, EpisodeOptionsDialogScreen.kt:262-316) is a method returning `Result<bool>` and is recursive in the Kotlin code. Rust: convert the recursion to a loop with a `max_attempts = number of videos` guard, because each failure marks one video `Error`, so the loop terminates. Preserve the exact order: set selected index, set `LoadVideo`, resolve if `video_state != Ready`, if resolve is None or URL empty then (if `current` is None) mark Error and select next best (recurse) else restore previous selection and mark Error and return false; on success mark `Ready` and set `current`.

Full behaviour is in p1-topic-quality-options-dialog.md.

## 6. Images (`images/model.rs`)

```rust
pub enum ImageKind { Cover, Background }

pub struct ImageModel {
    pub anime_id: i64,
    pub kind: ImageKind,                // derived from the current page: page 0 = Cover, page 1 = Background
}

pub enum ImageAction { Save, Share, EditFrom(std::path::PathBuf), Delete }

pub async fn save(model: &ImageModel, ctx: &ImageCtx) -> Result<PathBuf, ImageError>;
pub async fn share_temp(model: &ImageModel, ctx: &ImageCtx) -> Result<PathBuf, ImageError>;
pub async fn edit(model: &ImageModel, ctx: &ImageCtx, src: &Path) -> Result<(), ImageError>;
pub async fn delete_custom(model: &ImageModel, ctx: &ImageCtx) -> Result<(), ImageError>;
```

`ImageCtx` holds the cover cache dir, background cache dir, DB handle, and a `reqwest::Client`. Flow and messages: p1-topic-cover-background-dialog.md.

## 7. Widgets and layout (GTK mapping)

Layout skeleton (phone width):

```
gtk::Box (vertical)
 ├─ gtk::Overlay
 │   ├─ gtk::ScrolledWindow (vertical, overlay scrollbars)
 │   │   └─ gtk::Box (vertical, margin 14 px sides)
 │   │       ├─ info_box (Picture backdrop in gtk::Overlay under the cover row)
 │   │       ├─ action_row
 │   │       ├─ description + tags
 │   │       ├─ related (optional)
 │   │       ├─ item_header (count + missing count + filter button)
 │   │       ├─ airing_row (optional, glib timeout)
 │   │       └─ episode list: gtk::ListView (single column) or season GridView
 │   └─ fab (gtk::Button, halign End, valign End, margin 18 px)
 ├─ selection_bar (gtk::Revealer, bottom, shown when count > 0)
 └─ toast overlay (gtk::Revealer, bottom centre)
```

Wide layout (window width >= 900 px): `gtk::Paned` (`orientation Horizontal`, position 420). Start pane: scrolled box with info_box, action_row, description, tags, related. End pane: header plus list/grid. The selection bar spans the end pane only (proposal: matches the large layout's half-width bottom bar).

Episode list: `gtk::ListView` with `gio::ListStore<EpisodeRowObject>` where `EpisodeRowObject` is a `glib::Object` subclass holding `EpisodeRow` (missing rows and items). Virtualised by GTK; handles thousands of rows without the Compose lazy-key work. Use `set_show_separators(false)`, `set_single_click_activate(false)` (clicks are handled per row to match `combinedClickable`).

Season grid: `gtk::GridView` with `min_columns`/`max_columns` set from a size-allocate handler:
- `grid_size == 0`: `cols = max(1, width_px / 128)` (Compose `GridCells.Adaptive(128.dp)` equivalent, 128 px at scale 1).
- `grid_size > 0`: `cols = grid_size` (Compose `GridCells.Fixed`).
- List mode: `gtk::ListView` single column with span 1 per row (`GridItemSpan(maxLineSpan)`).

Row widgets: `gtk::Box` horizontal with cover `gtk::Picture` (`content-fit: cover`), text labels with `ellipsize: End`, `lines: 1` or `2`, and a `gtk::Overlay` for badges. Selected background: CSS class `kb-selected` with `background-color: alpha(@accent_bg_color, 0.22)` (dark theme 0.16 via `@media (prefers-color-scheme: dark)` or the GTK `dark` style class). Reuse the mockup's existing badge helper `badge(text, class)` in `src/screens.rs`.

Swipe: `gtk::GestureDrag` on the row. Offset > 56 px triggers the action; the action background is a `gtk::Revealer` child with the icon. Also right-click context menu with the same five actions (`gtk::PopoverMenu` built from `gio::Menu`).

Dialogs: `gtk::Window` with `set_modal(true)`, `set_transient_for(Some(&main_window))`, `set_hide_on_close(true)`, `Escape` closes via `gtk::EventControllerKey`. The mockup's `show_dialog`/`show_sheet` (src/ui.rs) render spec-driven dialogs; the anime dialogs in this area are hand-built, not spec-driven, because they depend on live data.

Images dialog: `gtk::Window` full-screen-like (`set_default_size` from monitor), inside a `gtk::Overlay` with a `gtk::Stack` of two pages (cover/background), each a `gtk::Picture` in a `gtk::ScrolledWindow` for zoom. Zoom with `gtk::GestureZoom` and Ctrl+scroll.

Toast/snackbar: a `gtk::Revealer` with a `gtk::Label` and optional "Dismiss" button, auto-hide after 4 seconds (GTK has no built-in toast widget without libadwaita; this mirrors `SnackbarHost(withDismissAction = true)`; Snackbar durations are Material defaults, Proposal: 4 s short, 10 s with action).

Shortcuts (`gtk::ShortcutController` on the window): see ui-parity.

## 8. Number and locale choices

- `format_episode_number`: `format!("{:.3}", n)` rounds the exact binary value half-to-even on ties and then trims zeros. Java's `DecimalFormat` default HALF_EVEN operates on the shortest decimal representation of the double. For ties that are not exact in binary, results can differ in the last digit. Accepted deviation; covered by a test with `2.0005` and `2.0015` expectations derived from the binary value (see E-9 in p1-edge-cases-and-tests.md).
- Digits: ASCII always. Kotlin on an `ar` locale can print localized digits (for example "١٢:٣٤"). Proposal: ASCII to keep the UI consistent. Track as an open decision (E-10).

## 9. User-visible strings (Rust string table)

Use the Kotlin message keys as identifiers in a `const` table (`src/i18n_anime.rs`), English text copied from the Kotlin resources (p1-ui-parity.md section 13 lists every key and text). Format placeholders: `%1$s` maps to `{0}` and is substituted by `fn fmt1(template: &str, a: &str) -> String`.

Keys that are literal in Kotlin code (not in resources), which need Rust constants: "Airing in %02dd %02dh %02dm %02ds" (`AnimeScreen.kt:1156`), "No available videos" (`EpisodeOptionsDialog.kt:438`), "Error getting links" (log text, `EpisodeOptionsDialog.kt:437`), "Copied video link to clipboard" is a key (`copied_video_link_to_clipboard`).

## 10. Errors and logging

```rust
#[derive(Debug, thiserror::Error)]
pub enum DetailsError {
    #[error("episode {0} not found")] EpisodeNotFound(i64),
    #[error("anime {0} not found")] AnimeNotFound(i64),
    #[error("source {0} unavailable")] SourceUnavailable(i64),
    #[error(transparent)] Quality(#[from] QualityError),
    #[error(transparent)] Image(#[from] ImageError),
    #[error(transparent)] Db(#[from] rusqlite::Error),
    #[error(transparent)] Io(#[from] std::io::Error),
}
```

Kotlin `!!` on `GetEpisode.await(episodeId)` and `GetAnime.await(animeId)` (EpisodeOptionsDialogScreen.kt:172-173) becomes `ok_or(DetailsError::EpisodeNotFound)`, shown as an error state, not a crash.

Logging: `tracing::error!` replaces `logcat(LogPriority.ERROR)` with the same message text.

## 11. Android to Linux mapping (this area)

| Android/Kotlin/Compose | Linux / GTK / Rust replacement | Notes |
|---|---|---|
| `LazyVerticalGrid` / `FastScrollLazyVerticalGrid` | `gtk::GridView` in `gtk::ScrolledWindow` | GTK overlay scrollbars replace fast scroller (fast scroller dropped, P3) |
| `LazyRow` | `gtk::ScrolledWindow` (horizontal policy) + `gtk::Box` | |
| `FlowRow` | `gtk::FlowBox` (`selection-mode: none`) | |
| `TwoPanelBox` | `gtk::Paned` | |
| `Scaffold`, `TopAppBar` | `gtk::HeaderBar` from the page helper `ui::page()` | |
| `BoxWithConstraints` | size-allocate on the widget | |
| `AnimatedVisibility` / `fadeIn` | `gtk::Revealer` with `transition-type: crossfade` | |
| `animateFloatAsState` | `gtk::Revealer` transition or CSS transition | |
| `SnackbarHost` | custom revealer toast (section 7) | |
| `BackHandler` | `gtk::EventControllerKey` (Escape) | |
| `DropdownMenu` | `gtk::PopoverMenu` / `gtk::MenuButton` (mockup `menu_button`) | |
| `Dialog`, `AlertDialog`, `AdaptiveSheet` | `gtk::Window` (modal, transient) | `gtk::AlertDialog` only for plain text prompts |
| `SwipeableActionsBox` | `gtk::GestureDrag` + revealer | |
| `combinedClickable` (click + long press) | `gtk::GestureClick` (button 1) + `gtk::GestureLongPress` | |
| `clickableNoIndication` | `gtk::GestureClick` without focus ring | |
| `Modifier.blur(4.dp)` | pre-downscaled picture (section 6.1 of behaviour) | GTK 4 has no per-widget blur |
| `drawWithContent` gradient | CSS `linear-gradient` background on an overlay | |
| `AsyncImage` / Coil | `gdk::Texture` loaded from the cover cache (area 34) | `crossfade` becomes a 150 ms CSS opacity transition (Proposal) |
| `ImageRequest.useBackground(true)` | separate loader call with `kind = Background` | explicit instead of coil extras |
| `Bitmap.Config.HARDWARE` copy | none | GTK textures are GPU-side already |
| `ReaderPageImageView` (SSIV) | `gtk::Picture` in `gtk::ScrolledWindow` + gesture zoom | |
| `ClipboardManager` / `copyToClipboard` | `gdk::Display::default().clipboard().set_text()` | |
| `context.toast` | the toast overlay | |
| `Intent` share (`toShareIntent`) | `gtk::FileLauncher` for the temp file, or `gio::AppInfo::launch_default_for_uri` | |
| `MediaStore` / `Environment.getExternalStoragePublicDirectory` | `directories` crate or `glib::user_special_dir(UserDirectory::Pictures)` | Dependency: `glib` only |
| `Injekt.get<…>()` | constructor injection through `ScreenCx` | no service locator |
| `screenModelScope` | `CancellationToken` per screen | |
| `StateFlow` / `MutableStateFlow` | `Rc<RefCell<State>>` on the GTK thread + `UiEvent` channel | |
| `rememberSaveable` | plain struct fields in the screen state (window lifetime) | |
| `HapticFeedback` | none (Linux desktop has no haptics) | P3 drop |
| `LocalConfiguration.screenWidthDp` | window width | |
| `Preview` composables | none | P3 drop |

## 12. Pitfalls

1. Large episode lists: never append thousands of `gtk::Box` rows. Use `ListView` plus `ListStore` (section 7). Check with a 3000-episode fixture.
2. `Rc<RefCell<…>>` borrows across `await` panic at runtime. Clone data out of the borrow before any `await`. Use `try_borrow_mut` in event handlers that may re-enter.
3. GTK dialogs must be created on the GTK thread. Background results go through `UiEvent`.
4. Season grid column count must be recomputed on resize; otherwise the adaptive layout keeps the old count.
5. `gdk::Texture::from_file` on a 4K background decodes on the GTK thread. Load and downscale on tokio (`image` crate with `ImageReader`), then pass bytes to `Texture::from_bytes` (`glib::Bytes`) on the GTK thread. Keep the full-size image for the dialog only.
6. Timeouts: the countdown `SourceId` must be removed in `connect_destroy` or it keeps the closure (and widget) alive.
7. `gtk::Revealer` with `reveal-child` false still measures its child in some layouts; set `visible(false)` on the selection bar when it is hidden if measurements matter.
8. Escape in dialogs must not also clear the episode selection (the Kotlin `BackHandler` order is: selection first). Route Escape to the window, not the list, when a dialog is open.
9. Flags are stored as `i64` in SQLite (`INTEGER`). Do not use `u64` or `i32`: `season_flags` uses bit 27 and `viewer` uses bit 56.
10. Keep `SeasonDisplayMode::from_i64` fallible. Do not `unwrap` it on a value read from the DB; log and fall back to `CompactGrid` (the Kotlin would throw).
