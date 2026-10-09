# Area 40 (part 1): Rust design for manga browse UI

Target: `gtk4` crate 0.11 (GTK 4.16+, no libadwaita), tokio, rusqlite, reqwest, serde. Binary crate `kurobayomi` (existing mockup in `/home/blackbox/kurobayomi/src/`). Specs: p1-behavior.md (behaviour), p1-data-and-config.md (models and keys), p1-ui-parity.md (widgets and states).

## 1. Principles

1. Pure logic first. Every algorithm in this area (query filter, grouping, language sort, migration flag codec, chapter-state merge, migration sequence) lives in GTK-free modules with unit tests (p1-edge-cases-and-tests.md). GTK code only renders results.
2. Models are plain structs, immutable snapshots (`Arc<State>`), replaced wholesale on change (mirrors `StateFlow` updates in the Kotlin model). No shared mutable state between threads.
3. GTK main thread owns every widget. Background work (tokio) sends typed events over `async_channel`; a `glib::spawn_future_local` loop on the main thread applies them.
4. No Android types leak into `browse::*`. Display-name, icon, and package lookups go through traits so they can be faked.
5. Keep the shape of the Kotlin code where it helps parity (same sort keys, same group order), but drop Compose/Voyager structure.

## 2. Crate and module layout

```
src/
  main.rs                       (existing) registers the Browse page into the window
  browse/
    mod.rs                      pub use; BrowseHandle (channels + navigation hooks)
    tab.rs                      BrowseTab page: Stack of 6 sub-pages, tab-switch channel
    widgets.rs                  BaseBrowseItem-like row, in-library badge, loading row, empty/error state
    dialogs.rs                  remove-entry dialog, untrusted-extension dialog, nsfw dialog, uninstall-private dialog
    icons.rs                    source icon resolution (stub, custom, local, default), extension icon loader
    display_name.rs             LocaleHelper.getSourceDisplayName / getLocalizedDisplayName / comparator (pure)
    source_list.rs              Sources page (grouping by language, pin, latest, options)
    source_filter.rs            Show/hide sources page (languages + checkboxes)
    source_browse.rs            Browse a source: toolbar, grid/list dispatch, error/empty/loading states
    global_search.rs            Global search page + result rows (pure part: result ordering in migrate model)
    extensions/
      mod.rs
      model.rs                  ExtensionsModel (pure): query filter, grouping, state, update-all planning
      install.rs                InstallStep state machine + download-state map (pure)
      list.rs                   Extensions tab page (GTK)
      filter.rs                 Extension language filter page (model + GTK)
      details.rs                Extension details page (model + GTK)
      source_prefs.rs           Source preference page renderer (declarative prefs -> GTK rows)
    migration/
      mod.rs
      flags.rs                  MigrateFlags codec (pure)
      engine.rs                 migrate_manga() + merge_chapter_state() (pure merge + async I/O via traits)
      source_picker.rs          migrate source list + sort header (pure sort + GTK)
      favorites.rs              favorites of one source (pure sort + GTK)
      search.rs                 migrate search page, per-source search page, ordering (pure order)
      dialog.rs                 migrate confirm dialog (GTK), calls engine
    prefs.rs                    BrowsePrefs struct: typed view over config keys (section 10)
```

Cargo features: none. Dependencies (Section 8).

## 3. Core types

Naming: Rust names mirror Kotlin where practical. All ids are `i64` (Kotlin `Long`). Pins and flags are `i32`/`u32` newtypes with explicit bit constants.

```rust
// src/browse/mod.rs
pub type SourceId = i64;
pub type MangaId = i64;

/// Mirrors tachiyomi.domain.source.manga.model.Pin (0b00, 0b01, 0b10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pins(pub u32);
impl Pins {
    pub const UNPINNED: u32 = 0b00;
    pub const PINNED: u32 = 0b01;
    pub const ACTUAL: u32 = 0b10;
    pub fn contains(self, bit: u32) -> bool { self.0 & bit == bit }   // Pins.contains
    pub fn is_pinned(self) -> bool { self.contains(Self::PINNED) }
}
```

```rust
// src/browse/display_name.rs
pub const LAST_USED_KEY: &str = /* LocaleHelper.LAST_USED_KEY, value from upstream constant */;
pub const PINNED_KEY: &str = /* LocaleHelper.PINNED_KEY */;

/// LocaleHelper.getSourceDisplayName. `other` and `all` have fixed labels (strings keys
/// other_source, multi_lang); everything else is the localized language name.
pub fn source_display_name(lang: &str, labels: &dyn Labels) -> String;
/// LocaleHelper.getLocalizedDisplayName (no special cases; "" for None).
pub fn localized_display_name(lang: Option<&str>) -> String;
/// LocaleHelper.comparator: "all" first, else by localized display name.
pub fn compare_languages(a: &str, b: &str) -> std::cmp::Ordering;

pub trait Labels {
    fn last_used(&self) -> String;   // MR.strings.last_used_source
    fn pinned(&self) -> String;      // MR.strings.pinned_sources
    fn other(&self) -> String;       // MR.strings.other_source
    fn multi_lang(&self) -> String;  // MR.strings.multi_lang
}
```

Note `compare_languages` must be a total order: `"all"` vs `"all"` returns `Equal`. The Kotlin comparator returns `-1` for `a == "all"` even when `b == "all"`; Rust must return `Equal` for the reflexive case (a sort with an inconsistent comparator is a bug in Rust's `sort_by`, which may panic on newer Rust versions).

```rust
// src/browse/extensions/model.rs
#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionKind { Installed, Available, Untrusted }

#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionView {
    pub kind: ExtensionKind,
    pub name: String,
    pub pkg_name: String,
    pub version_name: String,
    pub version_code: i64,
    pub lib_version: f64,
    pub lang: Option<String>,     // None for Untrusted
    pub is_nsfw: bool,
    pub has_update: bool,         // Installed only
    pub is_obsolete: bool,        // Installed only
    pub is_shared: bool,          // Installed only (drop: never shown on Linux)
    pub repo_url: Option<String>,
    pub sources: Vec<ExtensionSourceView>,
    pub icon_url: Option<String>, // Available only
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionSourceView {
    pub id: SourceId,
    pub name: String,
    pub base_url: String,         // "" for non-HTTP sources (Installed); required on Available
    pub lang: String,
    pub is_http: bool,           // Installed: `it is HttpSource`; Available: always true
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupHeader {
    Resource(HeaderKey),          // MR.strings.ext_updates_pending | ext_installed
    Text(String),                 // language display name
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderKey { ExtUpdatesPending, ExtInstalled }

#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionItem { pub ext: ExtensionView, pub step: InstallStep }

/// Query filter (MangaExtensionsScreenModel.queryFilter).
pub fn matches_query(query: &str, ext: &ExtensionView) -> bool;

/// Groups in insertion order: updates, installed (+untrusted), then per-language available.
pub fn group(
    query: &str,
    updates: &[ExtensionView],
    installed: &[ExtensionView],
    untrusted: &[ExtensionView],
    available: &[ExtensionView],
    steps: &std::collections::HashMap<String, InstallStep>,
    labels: &dyn display_name::Labels,
) -> Vec<(GroupHeader, Vec<ExtensionItem>)>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtensionsState {
    pub is_loading: bool,            // true until first group() result
    pub is_refreshing: bool,
    pub groups: Vec<(GroupHeader, Vec<ExtensionItem>)>,
    pub updates_count: u32,          // ext_updates_count preference
    pub installer_requires_system_permission: bool, // always false on Linux (drop)
    pub search_query: Option<String>,
}
impl ExtensionsState {
    pub fn is_empty(&self) -> bool { self.groups.is_empty() }
}
```

```rust
// src/browse/extensions/install.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallStep { Idle, Pending, Downloading, Installing, Installed, Error }
impl InstallStep {
    /// InstallStep.isCompleted: Installed | Error | Idle.
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Idle | Self::Installed | Self::Error)
    }
}

/// Map keyed by pkgName; entry present only while an install/update job runs.
#[derive(Default)]
pub struct DownloadStates(std::collections::HashMap<String, InstallStep>);
impl DownloadStates {
    pub fn set(&mut self, pkg: &str, step: InstallStep);   // addDownloadState
    pub fn clear(&mut self, pkg: &str);                    // removeDownloadState (onCompletion)
    pub fn get(&self, pkg: &str) -> InstallStep;           // downloads[pkg] ?: Idle
}
```

Install job contract (pure driver, the GTK side subscribes to events):

```rust
pub enum InstallEvent { Step(String /*pkg*/, InstallStep), Finished(String /*pkg*/, Result<(), String>) }
// onCompletion always runs: Finished (ok or err) clears the map entry.
```

```rust
// src/browse/extensions/details.rs (model)
#[derive(Clone, Debug, Default)]
pub struct DetailsState {
    pub extension: Option<ExtensionView>,   // Installed only
    pub is_incognito: bool,
    pub sources: Option<Vec<ExtensionSourceItem>>, // None = loading
}
#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionSourceItem { pub source: ExtensionSourceView, pub enabled: bool, pub label_as_name: bool }

/// Sort key of MangaExtensionDetailsScreenModel: (!enabled, label) with
/// label = name if label_as_name else display_name(lang).to_lowercase().
pub fn sort_details_sources(items: &mut [ExtensionSourceItem], labels: &dyn display_name::Labels);

/// Repo link conversion (regex from the Kotlin screen).
pub fn repo_web_url(repo_url: Option<&str>) -> Option<String>;
```

```rust
// src/browse/migration/flags.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrateFlags(pub i32);
impl MigrateFlags {
    pub const CHAPTERS: i32 = 0b00001;          // 1
    pub const CATEGORIES: i32 = 0b00010;        // 2
    pub const CUSTOM_COVER: i32 = 0b01000;      // 8
    pub const DELETE_DOWNLOADED: i32 = 0b10000; // 16
    pub const DEFAULT: i32 = i32::MAX;          // migrate_flags default (0x7FFF_FFFF)

    pub fn has_chapters(self) -> bool { self.0 & Self::CHAPTERS != 0 }
    pub fn has_categories(self) -> bool { self.0 & Self::CATEGORIES != 0 }
    pub fn has_custom_cover(self) -> bool { self.0 & Self::CUSTOM_COVER != 0 }
    pub fn has_delete_downloaded(self) -> bool { self.0 & Self::DELETE_DOWNLOADED != 0 }
}

pub struct FlagOption { pub flag: i32, pub is_default_selected: bool, pub label: LabelKey }
pub enum LabelKey { Chapters, Categories, CustomCover, DeleteDownloaded }

/// getFlags(manga, defaultBitmap). `show_custom_cover` / `download_count` are facts
/// the caller computes (Manga.hasCustomCover(cache), downloadCache.getDownloadCount).
pub fn flag_options(default_bitmap: i32, show_custom_cover: bool, download_count: u32) -> Vec<FlagOption>;

/// getSelectedFlagsBitMap: OR of the flag values whose checkbox is ticked; 0 when none.
pub fn selected_bitmap(selected: &[bool], options: &[FlagOption]) -> i32;
```

Note: `selected` and `options` are zipped by index. If the lengths differ, zip stops at the shorter one (Kotlin `zip` semantics). Keep.

```rust
// src/browse/migration/engine.rs
pub struct MigrationDeps {
    pub chapters: Arc<dyn ChapterRepo>,       // get_by_manga_id, update_all, sync_with_source
    pub categories: Arc<dyn CategoryRepo>,    // get(old) -> Vec<CategoryId>, set(new, ids)
    pub tracks: Arc<dyn TrackRepo>,           // get(old) -> Vec<Track>, insert_all
    pub enhanced_trackers: Arc<dyn EnhancedTrackers>, // is_track_from, migrate_track
    pub manga: Arc<dyn MangaRepo>,            // update_favorite(id,false), update(MangaUpdate)
    pub downloads: Arc<dyn DownloadRemover>,  // delete_manga(old, old_source)
    pub covers: Arc<dyn CoverCache>,          // has_custom_cover(old), set_custom_cover(new, reader)
    pub sources: Arc<dyn SourceResolver>,     // get(id) -> Option<Source>
    pub clock: Arc<dyn Clock>,                // now_epoch_ms()
}

pub struct MigratePlan { pub replace: bool, pub flags: MigrateFlags }

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("new source not installed")] NewSourceMissing,   // silent in UI (see behavior 14.3)
    #[error("fetch chapters failed: {0}")] FetchChapters(String),
    #[error("step failed: {0}")] Step(String),
}

pub async fn migrate_manga(
    deps: &MigrationDeps,
    old: &Manga,
    new: &Manga,
    plan: MigratePlan,
    new_source_chapters: Vec<SChapter>, // fetched by caller (getChapterList) before step 1
) -> Result<(), MigrateError>;

/// Pure: chapter state copy (behavior section 15, step 2).
pub fn merge_chapter_state(prev: &[Chapter], new: &[Chapter]) -> Vec<Chapter>;
```

Pure merge rules (implement exactly):
```rust
pub fn merge_chapter_state(prev: &[Chapter], new: &[Chapter]) -> Vec<Chapter> {
    let max_read = prev.iter().filter(|c| c.read).map(|c| c.chapter_number).reduce(f32::max);
    new.iter().map(|c| {
        let mut u = c.clone();
        if c.is_recognized_number() {                       // chapter_number >= 0.0
            if let Some(p) = prev.iter().find(|p| p.is_recognized_number() && p.chapter_number == c.chapter_number) {
                u.date_fetch = p.date_fetch;
                u.bookmark = p.bookmark;
            }
            if let Some(m) = max_read { if u.chapter_number <= m { u.read = true; } }
        }
        u
    }).collect()
}
```
Note `max_read` uses `filter(read)` without the recognised-number guard, matching upstream (edge case E-40-14 in p1-edge-cases-and-tests.md). `reduce(f32::max)` on NaN is not a concern (chapter numbers are finite).

`MigrateError::NewSourceMissing` is returned before `is_migrating` would be set; the dialog maps it to "stay open, no message" (upstream behavior). Kurobayomi may show a toast instead (P2 proposal, p1-ui-parity.md section 6).

```rust
// src/browse/migration/search.rs (pure ordering)
pub struct SourceRank<'a> { pub id: SourceId, pub name: &'a str, pub lang: &'a str, pub pinned: bool }
/// getEnabledSources override: origin first, pinned first, then "name (lang)" lowercased.
pub fn order_migrate_sources<'a>(sources: &mut [SourceRank<'a>], from_source: SourceId, pinned_only: bool);
```
Sort: `sort_by_key(|s| (s.id != from_source, !s.pinned, format!("{} ({})", s.name.to_lowercase(), s.lang)))`. `pinned_only` removes unpinned entries first (filter, then sort).

Note Kotlin `compareBy` puts `false` before `true`. `(s.id != from_source)` is false for the origin source, so origin sorts first. `!s.pinned` is false for pinned sources, so pinned sort first. Matches.

```rust
// src/browse/migration/favorites.rs (pure)
pub fn sort_favorites(mut titles: Vec<Manga>) -> Vec<Manga>; // by (title.to_lowercase(), title) ascending
```

```rust
// src/browse/source_list.rs (pure)
pub enum SourceRow { Header(String /*language display*/), Item(Source) }
/// Built by the part-2 model; area 40 only needs the row kind.
```

```rust
// src/browse/prefs.rs
pub struct BrowsePrefs {
    pub display_mode: LibraryDisplayMode,          // pref_display_mode_catalogue
    pub hidden_sources: BTreeSet<String>,          // hidden_catalogues (ids as strings)
    pub pinned_sources: BTreeSet<String>,          // pinned_catalogues
    pub enabled_languages: BTreeSet<String>,       // source_languages
    pub incognito_extensions: BTreeSet<String>,    // incognito_manga_extensions (pkg names)
    pub migrate_sort_mode: MigrateSortMode,        // pref_migration_sorting
    pub migrate_sort_direction: SortDirection,     // pref_migration_direction
    pub migrate_flags: MigrateFlags,               // migrate_flags
    pub show_nsfw_sources: bool,                   // show_nsfw_source
    pub extension_updates_count: u32,              // ext_updates_count (written by area 18)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum MigrateSortMode { Alphabetical, Total }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum SortDirection { Ascending, Descending }
```

Storage: `load(&Path) -> BrowsePrefs` with defaults; `save(&self)` writes the TOML file. Each write is one atomic rename (`tempfile` + `rename`). Default `enabled_languages = {"all", "en", <system lang>}` (from `LocaleHelper.getDefaultEnabledLanguages`).

## 4. Traits used by pure modules (dependency inversion)

```rust
pub trait ExtensionIconSource {
    /// Installed: decoded icon at `px` pixels, or None (use default). Available: None (use url).
    fn installed_icon(&self, pkg: &str, px: u32) -> Option<gdk_pixbuf::Pixbuf>;
}
pub trait SourceIconSource {
    fn source_icon(&self, source: SourceId) -> Option<gdk_pixbuf::Pixbuf>; // custom icon if the source has one
}
pub trait CoverLoader {
    fn load(&self, url: &str, lazy_key: &str) -> glib::JoinHandle;      // see section 6
}
```

Use `gdk_pixbuf::Pixbuf` only at the GTK boundary; the pure modules never import gtk.

## 5. Threading and async model

Components:
- GTK main context (glib): owns all widgets and every `Rc<RefCell<...>>`.
- Tokio runtime: `tokio::runtime::Builder::new_multi_thread().worker_threads(2)` (I/O bound: repo index, DB, network). Started once in `main.rs`, handle stored in `BrowseHandle`.
- Blocking DB work: `tokio::task::spawn_blocking` for rusqlite (area 03 owns the connection pool).

Pattern for every screen:

```rust
// On screen creation (main thread)
let (tx, rx) = async_channel::unbounded::<BrowseEvent>();
handle.runtime.spawn(load_task(deps.clone(), tx.clone()));     // tokio side
glib::spawn_future_local(async move {                           // main side
    while let Ok(ev) = rx.recv().await { apply(ev); }
});
```

- Event enums per screen (`ExtensionsEvent`, `MigrateEvent`, `BrowseEvent`). Events are small (ids and snapshots). Large data (covers) never travels in events.
- Stale-result rule (replaces Kotlin `collectLatest`): each request carries a `generation: u64`; the main thread drops events whose generation is older than the current one.
- Debounce (search, 250 ms): `glib::timeout_add_local_once(Duration::from_millis(250), ...)` storing a `SourceId` (glib) and removing the previous one on each keystroke.
- Cancellation: screens hold a `tokio::task::AbortHandle`/`JoinHandle` list and abort on pop.

Recompute on change (extensions list): the grouping function is pure and O(n log n) for a few hundred extensions; run it on the main thread directly (no task) to avoid event ordering issues. Only the repo fetch and installs go to tokio.

Install flow: `tokio::spawn(install(ext, tx))` emits `InstallEvent::Step(pkg, step)` for each phase and `Finished(pkg, res)` at the end. The main thread updates `DownloadStates` and redraws the row. `cancel` sends a `CancellationToken` (tokio-util).

Update all: a single tokio task that loops the installed-with-update list sequentially (matching the Kotlin `forEach`), emitting `Step` events per extension.

## 6. Covers and icons

- Available extension icons: `reqwest` GET `icon_url` (area 12 client), decode with `gdk_pixbuf::PixbufLoader` at 40 px or 112 px, cache file under `$XDG_CACHE_HOME/kurobayomi/ext-icons/`. While loading show a placeholder rectangle with colour `#1F888888` (`ColorPainter(Color(0x1F888888))`, i.e. ARGB `0x1F888888`, alpha 0x1F, RGB 0x888888).
- Error placeholder: bundled `data/icons/cover-error.svg` (replaces `R.drawable.cover_error`).
- Manga covers: area 34 cache. This area calls `CoverLoader::load(url, key)` and receives a `gdk::Texture` on the main thread.
- Image widgets: `gtk::Picture` with `content-fit: cover`, rounded via CSS class `.cover-clip` (replaces `clip(MaterialTheme.shapes.extraSmall)`, radius 4 px).

## 7. GTK widget mapping (Compose to GTK4)

| Compose / Material3 | GTK4 (0.11) |
|---|---|
| `Scaffold` + `AppBar` + `scrollBehavior` | `gtk::Box` vertical: `gtk::HeaderBar` (title, back button, actions) + content; `scroll` via `gtk::ScrolledWindow` |
| `SearchToolbar` | `gtk::SearchEntry` in the header bar's title area, shown on toggle; Escape closes (`on_stop_search`) |
| `LazyColumn` / `LazyRow` / `LazyVerticalGrid` | `gtk::ListView` / `gtk::GridView` with `gio::ListStore` and `SignalListItemFactory` (recycling) |
| Paging `LazyPagingItems` | `BrowseModel` with `gio::ListStore` appended in pages; request next page when the last visible index is within 6 items of the end (`ListView` `scroll-to` signal, or `adjustment` value-changed threshold) |
| `StickyHeader` | `gtk::ListView` header via `GridView` section header not available: render the sort row as a fixed `gtk::Box` above the list (it does not scroll), which is the same visible behaviour |
| `AlertDialog` | `gtk::AlertDialog` (GTK 4.10+) with `buttons`, `cancel_button`, `default_button` |
| `SnackbarHost` | custom `ToastOverlay` (`gtk::Overlay` + `gtk::Revealer` at bottom centre); action button; Indefinite = no auto-hide |
| `DropdownMenu` + `RadioMenuItem` | `gtk::PopoverMenu` built from `gio::Menu` with a radio action (`gio::SimpleAction::new_stateful`) |
| `FilterChip` | `gtk::ToggleButton` with CSS class `chip`; leading icon as `gtk::Image` |
| `Switch` | `gtk::Switch` (read-only in the source row: `set_sensitive(false)` is wrong, use `set_can_focus(false)` and ignore the state-set signal) |
| `Checkbox` | `gtk::CheckButton` (read-only in the source filter row: `can_target(false)`) |
| `LinearProgressIndicator` | `gtk::ProgressBar` with CSS class `osd` at the bottom of the header |
| `CircularProgressIndicator` | `gtk::Spinner` |
| `ModalBottomSheet` (filter sheet, part 2) | `gtk::Window` transient, modal |
| `combinedClickable` | `gtk::GestureClick` (button 1 = click, button 3 = long-click equivalent, see p1-ui-parity.md section 9); long press (touch) via `gtk::GestureLongPress` |
| `animateItem()` | `gtk::ListView` default animation is none; use a 150 ms CSS transition on opacity for newly inserted rows (optional) |
| `Voyager Navigator` | `NavigationStack` struct: `Vec<Page>`, `push/pop/replace/pop_until_root`, backed by `gtk::Stack` with named children |
| `Voyager Tab` | `gtk::Stack` + `gtk::StackSwitcher` (or header bar tabs) |
| `rememberScreenModel` | state struct owned by the page object, created once per page instance |
| `LaunchedEffect(Unit) { events.collect }` | `glib::spawn_future_local` loop on the page's event channel |
| `Toast` (`context.toast`) | `ToastOverlay::show(text)` (timeout 3 s) |
| `copyToClipboard(label, text)` | `gdk::Display::default().clipboard().set_text(text)` (label is not stored; Linux clipboard has no label) |
| `LocalUriHandler.openUri` | `gtk::UriLauncher::new(uri).launch(...)` (GTK 4.10+) |
| `WebViewScreen` | area 36 (WebKitGTK or external browser) |
| `Injekt.get()` | constructor injection (`BrowseDeps` struct passed to each page) |
| Moko `MR.strings.*` | `i18n::t("key")` lookup table (area 02/36) |

Note: `gtk::AlertDialog` requires the parent window; pass the main window weak ref.

## 8. Crates

| Crate | Version (pin at implementation) | Used for | Why |
|---|---|---|---|
| `gtk4` | `0.11` | all widgets | required by the target |
| `glib`, `gio`, `gdk-pixbuf`, `gdk4` | match gtk4 0.11 | main loop, list models, decode | transitively pulled |
| `tokio` | `1` (`rt-multi-thread`, `sync`, `time`, `macros`) | background I/O | target |
| `tokio-util` | `0.7` | `CancellationToken` | install cancel |
| `async-channel` | `2` | tokio to GTK events | works without a tokio runtime on the receiving side |
| `rusqlite` | `0.31`+ with `bundled` (area 03 decides) | DB | target |
| `reqwest` | `0.12` (`rustls-tls`, `json`, `gzip`) | icons, repo index | target |
| `serde`, `serde_json`, `toml` | latest 1.x | config files, repo JSON | target |
| `thiserror` | `1` | error enums | small |
| `async-trait` | `0.1` | async repo traits (object safe) | `dyn` traits in `MigrationDeps` |
| `tempfile` | `3` | atomic writes of config | safety |
| `regex` | `1` | repo URL conversion | same regex as Kotlin |
| `unicode-normalization` | not needed | | do not add |
| `icu_displaynames` (ICU4X) | `1.x` | `localized_display_name` | replaces `Locale.forLanguageTag(tag).displayName`; fallback = BCP-47 tag if the data is missing |

Rejected: `libadwaita` (forbidden), `relm4` (adds a component framework; plain gtk-rs is enough for this scope), `diesel` (rusqlite already chosen).

## 9. Localization and display names

- `display_name.rs` maps tags as in `LocaleHelper.getLocalizedDisplayName`: `zh-CN -> zh-Hans`, `zh-TW -> zh-Hant` (`LocaleHelper.getDisplayName` lines 40-47 in `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt`).
- `""` (empty) and `None` -> `""`. `"all"` -> the `multi_lang` label in `source_display_name`, but `localized_display_name("all")` returns the Java locale name for `all` (observed upstream: global search subtitles show it through `getLocalizedDisplayName`).
- `"other"` -> `other_source` label.
- Sorting uses the localized display name, lowercased comparison is not used upstream (raw `compareTo`). Keep raw compare (`str::cmp`).

## 10. Config and persistence

- File: `$XDG_CONFIG_HOME/kurobayomi/browse.toml` (layout in p1-data-and-config.md section 10).
- Loaded once at startup into `BrowsePrefs`; each setter writes the whole file atomically (small file, no need for a DB).
- Writes happen on the main thread; they are fast (< 1 ms); no task.
- Unknown keys are preserved on save (read into `toml::Table`, edit the known keys, write back).

## 11. Error handling

- Pure modules return values, never panic on bad input (no `unwrap` on user data; the Kotlin `!!` sites become `Option` handling: `MigrateMangaSearchScreen` (`getManga(...)!!`), `MigrateMangaScreen` (`source!!`), `BrowseMangaSourceScreenModel` pieces).
- Background errors become `Result<T, String>` for display, with the user-visible message from the key table, not the raw error, except the global-search error row which shows the raw message (behavior 10.2).
- The migration engine returns `MigrateError`. Steps 1 (chapter sync) and 2 (chapter fetch from the new source) keep upstream semantics: sync errors are ignored; a fetch error stops the migration before any write.
- Logging: `tracing` crate at `error` for upstream `logcat(ERROR)` sites (list in p1-edge-cases-and-tests.md). Do not add `tracing` to pure modules' public API.

## 12. Mockup integration (existing code in `/home/blackbox/kurobayomi`)

Current state (verified): `src/entry.rs` lines 242-400 build the browse UI from static lists:
- `browse()` (line 244) builds a tabbed screen; `sources(anime)` (283) uses hard-coded rows "Source A", "Source B"; `extensions(anime)` (311) uses rows "Extension A".."F"; `migrate_sources(anime)` (360) uses "Source A/B/C" with counts 12/4/1; `browse_source(id, migrating)` (380) builds a grid with static cards and routes to `open("browse_source")`; `global_search(id, migrating)` and `migrate_entries()` also static.
- Routing is by string keys in `spec/entries.json` through `open(...)`, `dialog_of(...)`, `sheet_of(...)` (`src/screens.rs`).
- `src/main.rs:175`: the Browse sidebar badge is the constant `"1"`; it must come from `BrowsePrefs::extension_updates_count` (Aniyomi shows the extension update count, `ext_updates_count`).

Plan:
1. Keep `spec/entries.json` as the screen list for layout reference only; the real route table moves to `src/browse/mod.rs` as an enum `BrowseRoute` (`Browse`, `BrowseSource { id, listing, migrating }`, `GlobalSearch { from_source, migrating }`, `ExtensionDetails { pkg }`, `ExtensionFilter`, `ExtensionTrust { pkg }`, `MigrateSources`, `MigrateEntries { manga_id }`, `MigrateSearch { manga_id }`, `MigrateSourceSearch { manga_id, source_id, query }`).
2. Replace static builders in `entry.rs` (`sources`, `extensions`, `migrate_sources`, `browse_source`, `global_search`, `migrate_entries`) with calls into `browse::*` that take a state snapshot. The pure structure (sections, rows, headers) stays; only data sources change.
3. Static sample data is removed from `src/entry.rs`. Per the user's rule "no invented scaffolding", empty states are shown when the model has no data; no sample sources are added.
4. Anime variants (`anime: bool` parameter) stay in the anime area (area 41/39); this area only handles `anime == false`. The `anime` branch in `browse()` (`0 | 1 => sources(anime)`, `2 | 3 => extensions(anime)`) is replaced by the manga branch here and an anime stub that the anime area fills.
5. `src/ui.rs` and `src/clamp.rs` (shared widgets): reuse `clamp` for the content width (max 1200 px for lists) if it already implements that; do not duplicate.

## 13. Android API to Linux mapping (for this area)

| Android API used | Where | Linux replacement |
|---|---|---|
| `Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS)` | details app-info | dropped (p1-behavior section 16, D1) |
| `PackageManager.getResourcesForApplication`, `getDrawableForDensity` | extension icon | read icon file from extension bundle (`ExtensionIconSource`) |
| `context.isPackageInstalled(pkg)` | long-click on extension | `ExtensionRegistry::has_installed(pkg)` |
| `Uri.fromParts("package", ...)` | app info | dropped |
| `DisplayMetrics.DENSITY_*` | icon size | pixel size param |
| `Context.copyToClipboard` (util) | source id, debug info | `gdk::Clipboard::set_text` |
| `Context.toast` | error toasts | `ToastOverlay` |
| `Context.stringResource` (moko) | labels | `i18n::t` |
| `AndroidView` + `FragmentContainerView` | source prefs host | native GTK rows |
| `ContextThemeWrapper` / `R.attr.preferenceTheme` | source prefs | not needed |
| `ComponentActivity`/`MainActivity.ready` | Browse tab ready signal | `window` present signal (`gtk::Window::present` then `ready` flag) |
| `Configuration.orientation` | grid columns | window width vs height (`orientation = width >= height ? Landscape : Portrait`); used by column preference |
| `BoxWithConstraints` height | list cover sizing | `gtk::Widget::height()` of the list |
| `LocalDensity.roundToPx()` | list cover sizing | GTK scale factor (`widget.scale_factor()`) |
| `Dispatchers` (`launchIO`, `withUIContext`) | all | tokio (I/O), glib main context (UI) |
| `Channel(BufferOverflow.DROP_OLDEST)` | tab switch | `async_channel::bounded(1)` + drain-then-send |

## 14. Pitfalls

1. Sort comparators: Kotlin `compareBy` with booleans is `false < true`. Rust tuples of bool sort the same way; do not invert.
2. `LocaleHelper.comparator` returns `-1` for `a == "all"` even when `b == "all"`. Rust must return `Equal` in that case, or `sort_by` can panic on Rust 1.81+ when the comparator is inconsistent.
3. Kotlin `and` precedence: `defaultSelectionMap and flag != 0` is `(a and b) != 0`. In Rust write the parentheses explicitly.
4. `Int.MAX_VALUE` as default: an `i32`. Do not use `u32` for `MigrateFlags` (the stored value is signed in the preference).
5. `toSortedMap` keeps keys sorted; the Rust `Vec<(GroupHeader, _)>` must be built in the same order (language groups sorted by `compare_languages`).
6. The `"▶ "` prefix is U+25B6 followed by a space. Use the exact char.
7. `mapNotNull` on `migrateTrack` drops null results. Do not insert a default track.
8. Chapter merge reads chapters after the sync step. Re-fetch from the DB after step 1; do not reuse the list from before the sync.
9. The Kotlin code updates all chapters on migrate even when unchanged; the Rust version may skip unchanged rows (an optimisation) but must produce the same final state.
10. GTK: `gtk::ListView` recycles rows. Bind handlers must not capture the row widget; use the `ListItem` item's data each bind.
11. `gio::ListStore` changes must happen on the main thread; the paging request is sent to tokio and the append happens on the main thread.
12. `async_channel` receivers keep the sender alive; drop the page's sender on pop or the loop never ends (memory leak per page).
13. Covers: a `gtk::Picture` widget that is unparented while loading must not be updated after drop; use `WeakRef` in the load callback.
14. The migration engine's file operations (download delete, cover copy) run after the DB writes in Kurobayomi (proposal), so a failure there does not leave a half-migrated DB state.
15. Preference keys in Kotlin are the source of truth for the hidden/pinned encodings (string ids). Do not switch to numeric ids without a converter.
