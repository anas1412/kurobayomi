# 25-downloads-shared — Rust design

Target crate: `kurobayomi` (edition 2024, see /home/blackbox/kurobayomi/Cargo.toml). Current deps: `gtk` (gtk4 0.11.5, feature `v4_16`), `serde`, `serde_json`. This design adds the crates listed in section 3.

Mockup hooks: `src/screens.rs::downloads()` (line 354) and `src/screens.rs::download_button()` (line 134) are replaced. `src/ui.rs::menu_button` (line 202) is not suitable for the queue rows (it cannot carry state or action ids); the queue uses `gio::Menu` + `gio::SimpleAction`.

---

## 1. Module layout

```
src/download/mod.rs        re-exports; no logic
src/download/kind.rs       EntryKind, DownloadState (numeric values from Kotlin)
src/download/prefs.rs      DownloadPrefs: typed view over the preference store, keys and defaults
src/download/policy.rs     pure delete filter (select_for_delete) and new-download filter inputs
src/download/queue.rs      pure QueueModel: sections, stable sort, reverse, move, series moves, removals
src/download/delete.rs     DeleteDownloads service: resolves source, filters, updates queue, removes dirs, cascades empty dirs
src/download/store.rs      DownloadStore trait + Fs implementation (series/item dirs, empty-dir cascade)
src/download/engine.rs     QueueEngine trait (implemented by area:23/24 engine) + command enum
src/download/tab.rs        GTK page: header, Notebook (Anime | Manga), FAB, sort menu, per-kind list
src/download/row.rs        GTK row: progress bar, move/cancel menu, drag and drop
```

Pure modules (`kind`, `prefs` keys table, `policy`, `queue`) have no GTK or tokio dependency and carry the unit tests (see p1-edge-cases-and-tests.md).

---

## 2. Key types

### 2.1 kind.rs

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind { Anime, Manga }

/// Numeric values match AnimeDownload.State / MangaDownload.State (both `(val value: Int)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum DownloadState {
    NotDownloaded = 0,
    Queue = 1,
    Downloading = 2,
    Downloaded = 3,
    Error = 4,
}

impl TryFrom<i64> for DownloadState {
    type Error = i64;
    fn try_from(v: i64) -> Result<Self, i64> {
        Ok(match v {
            0 => Self::NotDownloaded,
            1 => Self::Queue,
            2 => Self::Downloading,
            3 => Self::Downloaded,
            4 => Self::Error,
            other => return Err(other),
        })
    }
}
```

### 2.2 queue.rs (pure)

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct QueueItem {
    pub kind: EntryKind,
    pub item_id: i64,          // episode.id or chapter.id
    pub parent_id: i64,        // anime.id or manga.id (series key)
    pub source_id: i64,
    pub source_name: String,
    pub title: String,         // entry title
    pub item_name: String,     // "Episode 5" / "Chapter 12" label source
    pub date_upload: i64,      // sort key "By upload date"
    pub number: f64,           // sort key "By episode/chapter number"
    pub state: DownloadState,
    pub pages_done: u32,
    pub pages_total: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey { UploadDate, Number }

/// One section per source, in first-appearance order of the flat queue.
#[derive(Clone, Debug, PartialEq)]
pub struct Section { pub source_id: i64, pub source_name: String, pub items: Vec<QueueItem> }

#[derive(Default, Clone, Debug, PartialEq)]
pub struct QueueModel { flat: Vec<QueueItem> }   // queue order, the order the engine sees

impl QueueModel {
    pub fn from_flat(flat: Vec<QueueItem>) -> Self;
    pub fn flat(&self) -> &[QueueItem];
    pub fn sections(&self) -> Vec<Section>;                       // groupBy(source) preserving first occurrence
    pub fn sort_within_sections(&mut self, key: SortKey, reverse: bool); // see behaviour 4.3
    pub fn move_to_top(&mut self, item_id: i64);                  // within its own section, index 0
    pub fn move_to_bottom(&mut self, item_id: i64);               // within its own section, last
    pub fn move_series(&mut self, item_id: i64, to_top: bool);    // flat partition, see behaviour 4.5
    pub fn remove(&mut self, item_ids: &[i64]) -> Vec<i64>;       // returns ids actually removed
    pub fn remove_series(&mut self, parent_id: i64) -> Vec<i64>;
    pub fn position(&self, item_id: i64) -> Option<usize>;
}
```

Implementation notes (normative):
- `sections()`: iterate `flat`, keep an `IndexMap`-style `Vec<(source_id, Section)>`; a new source appends a section; order of sections = first appearance. Use a `Vec` plus linear search (queue sizes are small; no hashing needed).
- `sort_within_sections`: rebuild sections, `items.sort_by(|a,b| key(a).partial_cmp(&key(b)).unwrap_or(Equal))` (stable; `sort_by` is stable in Rust), then `if reverse { items.reverse() }`, then flatten sections in order. This reproduces `sortedBy(selector).apply { if (reverse) reverse() }` exactly, including the reversal of equal keys.
- `move_to_top/bottom`: remove the item from its section, insert at 0 or push; flatten. Items in other sections keep their relative order.
- `move_series`: `let (sel, other): (Vec, Vec) = flat.partition(|i| i.parent_id == parent)`; `to_top` → `sel ++ other`; else `other ++ sel`. This is a flat operation and may reorder sections; the next `sections()` call regroups.
- `remove`: removes all matching `item_id`s, returns removed ids in queue order.

### 2.3 policy.rs (pure)

```rust
pub struct DeletePolicy {
    pub exclude_categories: BTreeSet<i64>,   // from remove_exclude_* (parsed to i64)
    pub remove_bookmarked: bool,              // pref_remove_bookmarked
}

#[derive(Clone, Copy, Debug)]
pub struct DeleteCandidate { pub item_id: i64, pub watched: bool, pub bookmarked: bool }

/// Mirrors getEpisodesToDelete / getChaptersToDelete (see behaviour 1.4).
pub fn select_for_delete(
    candidates: &[DeleteCandidate],
    entry_categories: &[i64],
    policy: &DeletePolicy,
) -> Vec<i64> {
    let cats: &[i64] = if entry_categories.is_empty() { &[0] } else { entry_categories };
    let protected = cats.iter().any(|c| policy.exclude_categories.contains(c));
    candidates
        .iter()
        .filter(|c| !(protected && c.watched))
        .filter(|c| policy.remove_bookmarked || !c.bookmarked)
        .map(|c| c.item_id)
        .collect()
}
```

Note: `watched` is `seen` for anime and `read` for manga.

### 2.4 prefs.rs

Keys and defaults as a single table (exact strings from DownloadPreferences.kt):

```rust
pub mod keys {
    pub const DOWNLOAD_ONLY_OVER_WIFI: &str = "pref_download_only_over_wifi_key";
    pub const USE_EXTERNAL_DOWNLOADER: &str = "use_external_downloader";
    pub const EXTERNAL_DOWNLOADER_SELECTION: &str = "external_downloader_selection";
    pub const SAVE_CHAPTER_AS_CBZ: &str = "save_chapter_as_cbz";
    pub const SPLIT_TALL_IMAGES: &str = "split_tall_images";
    pub const AUTO_DOWNLOAD_WHILE_READING: &str = "auto_download_while_reading";
    pub const AUTO_DOWNLOAD_WHILE_WATCHING: &str = "auto_download_while_watching";
    pub const REMOVE_AFTER_READ_SLOTS: &str = "remove_after_read_slots";
    pub const REMOVE_AFTER_MARKED_AS_READ: &str = "pref_remove_after_marked_as_read_key";
    pub const REMOVE_BOOKMARKED: &str = "pref_remove_bookmarked";
    pub const DOWNLOAD_FILLERMARKED: &str = "pref_download_fillermarked";
    pub const REMOVE_EXCLUDE_MANGA_CATEGORIES: &str = "remove_exclude_categories";
    pub const REMOVE_EXCLUDE_ANIME_CATEGORIES: &str = "remove_exclude_anime_categories";
    pub const DOWNLOAD_NEW: &str = "download_new";
    pub const DOWNLOAD_NEW_EPISODE: &str = "download_new_episode";
    pub const DOWNLOAD_NEW_MANGA_CATEGORIES: &str = "download_new_categories";
    pub const DOWNLOAD_NEW_ANIME_CATEGORIES: &str = "download_new_anime_categories";
    pub const DOWNLOAD_NEW_MANGA_CATEGORIES_EXCLUDE: &str = "download_new_categories_exclude";
    pub const DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE: &str = "download_new_anime_categories_exclude";
    pub const DOWNLOAD_SLOTS: &str = "download_slots";
    pub const DOWNLOAD_SPEED_LIMIT: &str = "download_speed_limit";
    pub const DOWNLOAD_NEW_UNREAD_CHAPTERS_ONLY: &str = "download_new_unread_chapters_only";
    pub const DOWNLOAD_NEW_UNREAD_EPISODES_ONLY: &str = "download_new_unread_episodes_only";

    /// DownloadPreferences.categoryPreferenceKeys, same order.
    pub const CATEGORY_KEYS: [&str; 6] = [
        REMOVE_EXCLUDE_MANGA_CATEGORIES, REMOVE_EXCLUDE_ANIME_CATEGORIES,
        DOWNLOAD_NEW_MANGA_CATEGORIES, DOWNLOAD_NEW_ANIME_CATEGORIES,
        DOWNLOAD_NEW_MANGA_CATEGORIES_EXCLUDE, DOWNLOAD_NEW_ANIME_CATEGORIES_EXCLUDE,
    ];
}
```

Typed accessors (one method per Kotlin accessor, same names in snake_case): `download_only_over_wifi() -> Pref<bool>` (default `true`), `save_chapters_as_cbz() -> Pref<bool>` (`true`), `split_tall_images() -> Pref<bool>` (`true`), `remove_after_read_slots() -> Pref<i64>` (`-1`), `download_slots() -> Pref<i64>` (`1`), `download_speed_limit() -> Pref<i64>` (`0`), `auto_download_while_reading() -> Pref<i64>` (`0`), `auto_download_while_watching() -> Pref<i64>` (`0`), all other booleans `false`, string `use_external_downloader`'s sibling `external_downloader_selection` default `""`, sets `BTreeSet<String>` default empty.

`Pref<T>` is the area:02 typed preference handle (`get()`, `set()`, `watch() -> tokio::sync::watch::Receiver<T>`). `Int` → `i64` (SQLite INTEGER). `StringSet` → `BTreeSet<String>`.

Category sets are parsed on read: `fn parse_category_ids(set: &BTreeSet<String>) -> Result<BTreeSet<i64>, ParseIdError>` (see edge case E-05).

### 2.5 engine.rs (trait the engine must offer)

```rust
#[async_trait::async_trait]   // or return Pin<Box<dyn Future>>; see 4.3
pub trait QueueEngine: Send + Sync + 'static {
    fn snapshot(&self) -> QueueModel;                       // latest queue, flat, queue order
    fn watch(&self) -> tokio::sync::watch::Receiver<QueueModel>;
    fn is_running(&self) -> tokio::sync::watch::Receiver<bool>;

    fn start(&self);                 // Kotlin: startDownloads()  (job start or downloader.start)
    fn stop(&self);                  // Kotlin: anime pauseDownloads()
    fn pause_and_stop(&self);        // Kotlin: manga pauseDownloads() = pause(); stop()
    fn clear(&self);                 // Kotlin: clearQueue() = clearQueue(); stop()
    fn reorder(&self, flat: Vec<i64>);                  // item_ids in new order
    fn cancel(&self, item_ids: Vec<i64>);               // remove from queue, no file delete
    fn remove_from_queue(&self, item_ids: &[i64]);      // used by cancel and delete paths; see below
}
```

Kotlin `removeFromDownloadQueue` (AnimeDownloadManager.kt:315-327) is implemented in the Rust engine as:
```rust
fn remove_from_queue(&self, ids: &[i64]) {
    let was_running = self.is_running_now();
    if was_running { self.pause_internal(); }
    self.remove_internal(ids);     // sets state NotDownloaded for DOWNLOADING/QUEUE items
    if was_running {
        if self.queue_is_empty() { self.stop_internal() } else { self.start_internal() }
    }
}
```

Command channel: the engine runs on tokio; these methods push a command into an `mpsc::UnboundedSender<EngineCmd>` (no blocking), matching Kotlin's fire-and-forget calls.

---

## 3. Crates

| Crate | Version (proposal) | Why | Replaces (Android) |
|---|---|---|---|
| gtk4 | 0.11.5, feature v4_16 (already in Cargo.toml) | widgets | Compose / RecyclerView |
| tokio | 1 (features: rt-multi-thread, sync, time, fs, macros) | engine + delete IO | kotlinx.coroutines (`launchIO`, `delay`, `debounce` via `tokio::time`) |
| async-channel | 2 | bridge tokio → GLib main loop without a GLib runtime | Flow collected on Main |
| thiserror | 2 | typed errors in delete/store | `Exception` |
| tracing | 0.1 | logs replacing `logcat` | `logcat(LogPriority.ERROR, e)` |
| rusqlite | (area:02) | preference store | SharedPreferences |
| serde / serde_json | already present | category id sets in backups | kotlinx.serialization (`JsonObject` not used here) |
| async-trait | 0.1 (only if trait objects with async fns are needed) | `QueueEngine` async methods | suspend functions |

No new crate for sorting, grouping, or set operations; use `std`.

---

## 4. Threading and async model

### 4.1 Threads
- GTK main thread: all widgets, `QueueModel` view copy (`Rc<RefCell<QueueModel>>`), menus, FAB state.
- tokio multi-thread runtime (started once in `main`, handle stored in a `OnceLock`): engine loop, queue persistence (owned by area:23/24), delete work (`spawn_blocking` for FS), HTTP (area:12).

### 4.2 Message flow
- Engine → UI: `watch::Receiver<QueueModel>` is forwarded by a tokio task into an `async_channel::Sender<QueueModel>`. The GTK side runs `glib::spawn_future_local` (or `MainContext::spawn_local`) that `recv().await`s and rebuilds sections. This avoids calling GTK from a tokio thread.
- UI → engine: commands in section 2.5 are non-blocking sends.
- Progress: a per-row update is a separate `async_channel` of `(item_id, pages_done, pages_total)`; the GTK side coalesces with a 50 ms debounce (matching the manga progress job) using `glib::timeout_add_local_once`.

### 4.3 Async trait shape
Delete entry points are `async fn` on `DeleteDownloads` (called from SetSeenStatus / SetReadStatus equivalents in area:07 once ported). Use plain `async fn` in the inherent impl; only the `QueueEngine` trait needs object safety, and it has no async methods (all commands are sync and non-blocking). This avoids `async-trait` entirely.

---

## 5. Delete service

```rust
pub struct EntryRef { pub kind: EntryKind, pub id: i64, pub title: String, pub source_id: i64 }

pub struct DeleteDownloads {
    pub sources: Arc<dyn SourceResolver>,      // source_id -> Option<Arc<dyn Source>>
    pub categories: Arc<dyn CategoryLookup>,   // entry_id -> Vec<i64>  (area:08)
    pub prefs: Arc<DownloadPrefs>,
    pub engine: Arc<dyn QueueEngine>,
    pub store: Arc<dyn DownloadStore>,
    pub cache: Arc<dyn DownloadCache>,         // area:23/24
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error("download store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("preference {key} holds a non-numeric category id: {value}")]
    BadCategoryId { key: &'static str, value: String },
}

impl DeleteDownloads {
    /// Mirrors DeleteEpisodeDownload.awaitAll / DeleteChapterDownload.awaitAll.
    /// Returns the number of items whose downloads were removed (0 when the source is unknown).
    pub async fn await_items(
        &self,
        entry: &EntryRef,
        items: Vec<DeleteCandidate>,
        item_names: &[(i64, String, Option<String>)],  // (item_id, name, scanlator) for dir lookup
    ) -> Result<usize, DeleteError>;
}
```

Algorithm (normative; mirrors behaviour 1.5):
1. If `sources.resolve(entry.source_id)` is `None` → `return Ok(0)`. No deletion, no error (Kotlin behaviour; see E-02).
2. `policy = DeletePolicy { exclude_categories: parse(prefs.remove_exclude(kind)), remove_bookmarked: prefs.remove_bookmarked().get() }`.
3. `cats = categories.ids_for(entry.id)`.
4. `ids = select_for_delete(&items, &cats, &policy)`; if empty → `Ok(0)`.
5. `engine.remove_from_queue(&ids)` (section 2.5 logic).
6. `(series_dir, item_dirs) = store.find_item_dirs(kind, entry, &selected_items)` (returns `(None, vec![])` when the series dir is absent).
7. `tokio::task::spawn_blocking` → remove each item dir recursively. Errors on individual dirs are collected and logged, not fatal (Proposal; Kotlin would throw inside `launchIO`).
8. `cache.remove_items(kind, entry, &ids)`.
9. If the series dir is now empty → `delete_series(kind, entry, remove_queued = false)`; within it, if the source dir is empty → remove it and `cache.remove_source(source_id)`.
10. Return `Ok(ids.len())`.

Deviation (intentional, recorded in E-03): Kotlin returns before step 6-9 finish. The Rust `await_items` awaits the removal so the UI can refresh the row state deterministically. The UI does not block: the call is spawned from the GTK side with `glib::spawn_future_local`.

---

## 6. Store trait (filesystem)

```rust
pub trait DownloadStore: Send + Sync {
    /// (series_dir, item_dirs). If series_dir is None, item_dirs is empty.
    fn find_item_dirs(&self, kind: EntryKind, entry: &EntryRef, items: &[ItemName]) -> (Option<PathBuf>, Vec<PathBuf>);
    fn series_dir(&self, kind: EntryKind, entry: &EntryRef) -> Option<PathBuf>;
    fn source_dir(&self, source_id: i64, source_name: &str) -> Option<PathBuf>;
    fn is_empty_dir(&self, path: &Path) -> Option<bool>;   // None if not a directory
    fn remove_dir_all(&self, path: &Path) -> std::io::Result<()>;
}

pub struct ItemName { pub item_id: i64, pub name: String, pub scanlator: Option<String> }
```

Provider rule (owned by area:23/24, referenced here): candidate directory names are produced from the item's name and scanlator (`getValidEpisodeDirNames(name, scanlator)` / `getValidChapterDirNames(name, scanlator)`). Implement the same candidate list, first match wins per item.

Empty-directory test: `is_empty_dir(p) == Some(true)` only. A `None` (not a dir / unreadable) never triggers cascade (matches `listFiles()?.isEmpty() == true`).

---

## 7. GTK page (tab.rs)

Widgets (GTK 4.16, no libadwaita):
- Header: the window's `gtk::HeaderBar` (owned by area:36). The page contributes:
  - title label "Download queue" (`gettext`-less English string) with an optional count pill (anime count only, see behaviour 3.5) — pill is a `gtk::Label` with CSS class `count-pill`.
  - `gtk::MenuButton` "Sort" (icon `view-sort-symbolic`), menu model:
    ```
    By upload date  > Newest | Oldest
    By episode number (anime) / By chapter number (manga) > Ascending | Descending
    ```
    Actions: `queue.sort-upload-desc`, `queue.sort-upload-asc`, `queue.sort-number-asc`, `queue.sort-number-desc` (action names are per kind page).
  - `gtk::MenuButton` overflow (icon `view-more-symbolic`) with "Cancel all" (action `queue.cancel-all`).
- Body: `gtk::Notebook` with two pages (Anime, Manga). Tab label = `gtk::Box[Label, count-pill]` (pill always shown per Kotlin; proposal hides at 0).
- Per page: `gtk::ScrolledWindow` > `gtk::Box(vertical)` of section headers (`Label` heading with `"{source_name} ({count})"`) and one `gtk::ListBox` per section. Empty state: `gtk::Box` with `gtk::Image` icon `folder-download-symbolic` and label "No downloads".
- FAB: `gtk::Overlay` child; a `gtk::Button` with `Box[Image, Label]`, CSS class `pill-fab`, anchored bottom-end. Collapse = hide the `Label` (keep icon). Visible iff current page's `QueueModel` is non-empty.

Row (row.rs):
- `gtk::ListBoxRow` with: drag handle (`list-drag-handle-symbolic`), title (item name: episode name or chapter name), subtitle (entry title), progress text (anime: `Downloading…` at progress 0, else `NN%`; manga: `done/total`, empty when pages unknown; formats from AnimeDownloadHolder.kt:62-84 and MangaDownloadHolder.kt:43-72), and a `gtk::ProgressBar` beneath the row (anime: indeterminate at 0, else fraction = progress/100; manga: fraction = summed page progress / (pages*100)).
- Row menu: `gtk::MenuButton` (icon `view-more-symbolic`) with `gio::Menu` items in this order: Move to top (`row.move-top`), Move series to top (`row.move-top-series`), Move to bottom (`row.move-bottom`), Move series to bottom (`row.move-bottom-series`), Cancel (`row.cancel`), Cancel all for this series (`row.cancel-series`). Visibility: `Move to top` only when `index > 0` in the flat queue; `Move to bottom` only when `index != len - 1` (behaviour 5).
- Drag and drop: `gtk::DragSource` on the handle, `gtk::DropTarget` on each `ListBox` with `glib::Type::I64` payload (item_id). Drop → compute the new flat order: remove dragged id, insert at drop index in the target section, flatten, send `reorder`.
- Keyboard: Alt+Up / Alt+Down on a focused row = move up/down one (same as drag); Delete = cancel row. (Proposal; the Kotlin app has no keyboard path. Recorded in T-25a-026.)

FAB and running state: subscribe to `is_running` watch; label text per kind (behaviour 3.6): Anime running "Stop" / stopped "Continue"; Manga running "Pause" / stopped "Resume".

---

## 8. Mockup replacement plan

| Mockup (current) | Replacement |
|---|---|
| `src/screens.rs::downloads()` (354-402): fake "Source A/B" rows, fixed 45%/12/30, sort menu with four labels | `download::tab::DownloadsPage::new(engine)`; sections from the real `QueueModel`; sort menu per kind (two sub-menus, not four flat labels) |
| `src/screens.rs::downloads()` FAB toggles label on click only | FAB label/action driven by `is_running` watch |
| `src/screens.rs::download_button()` (134): "Start downloading now", "Cancel", "Delete" | Owned by entry screens (areas 37-41); calls `engine.start()` / `engine.cancel()` / `delete.await_items()` |
| `spec/app.json` screen `download_queue` | Keep the screen id; update `tabs`, `toolbar`, `menu` to match section 7; mark the screen `kind: "custom"` as today |

The spec-driven pieces of the mockup (`spec::Screen` / `ui::page`) keep working; the download page is built from code (`kind: "custom"`), as the mockup already does.

---

## 9. Android to Linux mapping (this scope)

| Android / Kotlin | Linux / GTK / Rust | Note |
|---|---|---|
| Voyager `Tab` (`DownloadsTab`, index 6) | GTK page registered in the app's sidebar / stack (area:36) | index 6 becomes sidebar order position |
| `AnimatedImageVector` tab icon `anim_history_enter` | static symbolic icon `document-open-recent-symbolic` | **P3 drop of the animation**; the static icon replaces it |
| `ScreenModel` (`rememberScreenModel`) | a struct owned by the page; one per kind | fixes the double-instance quirk |
| `screenModelScope` | tokio task + glib local future | |
| `Scaffold` / `AppBar` (`pinnedScrollBehavior`) | `gtk::HeaderBar` + `ScrolledWindow` | pinned top bar = header never scrolls |
| `ExtendedFloatingActionButton` | `gtk::Button` with icon + label, collapsible via label visibility | |
| `PrimaryTabRow` + `HorizontalPager` | `gtk::Notebook` (tabs at top) | swipe gesture: GTK notebook has no swipe; a `GestureDrag` is optional (P2) |
| `NestedScrollConnection` | `gtk::EventControllerScroll` `scroll` signal sign of `dy` | collapse FAB on dy > 0 (content up) |
| `DropdownMenu` + `NestedMenuItem` | `gtk::PopoverMenu` with submenus (`gio::Menu::append_submenu`) | |
| `AppBarActions` overflow | `gtk::MenuButton` with `gio::Menu` | |
| `Pill` | `gtk::Label` CSS `count-pill`: `background-color: alpha(currentColor, 0.08)`; dark variant follows the GTK theme, no `isSystemInDarkTheme()` | GTK theme applies (user requirement) |
| `isSystemInDarkTheme()` alpha 0.12 / 0.08 | none; GTK dark theme provides contrast. Keep 0.08 with `alpha()` in CSS | |
| `SnackbarHostState` | `gtk::Overlay` with a transient `gtk::Revealer` label (owned by area:36) | this scope shows no snackbar of its own |
| `EmptyScreen` | `gtk::Box` with icon and label | |
| `ProgressBar` (Compose) | `gtk::ProgressBar` | |
| `RecyclerView` holder lookup `findViewHolderForItemId` | `HashMap<i64, RowHandle>` in the page | O(1) row update |
| `lateinit var controllerBinding` | the page's `ListBox` widgets | no lateinit |
| `FlexibleAdapter` headers | one `gtk::ListBox` per section + section label | avoids the flat-position quirk |
| `DownloadListBinding` | not needed | |
| `Icons.Outlined.Pause` / `Icons.Filled.PlayArrow` / `Icons.AutoMirrored.Outlined.Sort` | `media-playback-pause-symbolic`, `media-playback-start-symbolic`, `view-sort-descending-symbolic` | freedesktop names |
| `folder-download-symbolic` (mockup icon) | kept | |
| `Injekt.get()` | constructor injection | |

---

## 10. Pitfalls

1. Kotlin's `sortedBy(...).reverse()` reverses equal keys too. The Rust sort must do stable sort then reverse, not `sort_by(reverse comparator)` (that would keep equal keys in original order). Test case E-07.
2. `groupBy` order is first-appearance. Do not use a `BTreeMap` for sections; it would sort sections by source id.
3. Flat queue vs section view: the engine only knows the flat list. Every mutation must rebuild flat in section order before calling `reorder`.
4. GTK widgets are not `Send`. All `QueueModel` mutations happen on the main thread; the tokio side only sends snapshots.
5. A `ListBox` cannot host rows from two sections with one DropTarget unless each section has its own target. Use one target per section.
6. Pill count at 0: Kotlin shows it; Rust hides it (documented deviation).
7. `alpha()` in GTK CSS needs a colour; `currentColor` is not accepted by all GTK versions — use `@theme_fg_color` (GTK 4.16 named colour) instead: `background-color: alpha(@theme_fg_color, 0.08)`.
8. `set_visible(false)` on the FAB label must not change the button's natural width jump; set a min width on the button to avoid jitter (Kotlin's `expanded` animates).
9. The Kotlin "Cancel all" has no confirmation. Rust keeps no confirmation (matches) but must not run it when the queue is already empty (action disabled).
10. Source name changes after first load: `QueueItem.source_name` is a snapshot. Section headers are rebuilt on every snapshot, so no stale name.

---

## 11. Persistence

The queue itself is persisted by the engine (area:23/24). This scope adds no tables and no files. The category id sets are persisted by the preference store (area:02) under the six keys in section 2.4.
