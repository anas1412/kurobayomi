# Area 41 (part 1): Rust design for anime browse, extensions and migration

Target: crate `kurobayomi` (edition 2024, `Cargo.toml` today: `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde`, `serde_json`). Everything here is GTK 4.16+, no libadwaita. Behavior reference: p1-behavior.md. Data reference: p1-data-and-config.md.

## 1. Principles

1. Pure logic first. Filtering, grouping, sorting, migration flags, pin bits and the migrate engine are plain Rust with no GTK types. They are unit-tested (p1-edge-cases-and-tests.md).
2. One owner per piece of state. Each page has one model struct. The GTK widget tree only renders a snapshot.
3. Never block the GTK main loop. Network, DB and filesystem work run on the tokio runtime (owned by area:01); results come back through `async_channel`.
4. Typed pages, not string ids. The mockup navigates by spec string ids (`ui::open("browse_source")`). The real app opens typed page builders (section 4.4). The spec JSON stays for static text only.

## 2. Crate and module layout

```
src/
  browse/
    mod.rs                      // pub mod anime; (browse tab shell is area:40)
    anime/
      mod.rs                    // pub re-exports; BrowseAnime facade used by entry.rs
      model.rs                  // Pin, LibraryDisplayMode, FetchType, AnimeSourceFilter, InstallStep
      prefs.rs                  // typed accessors over area:02 prefs (keys in p1-data-and-config.md section 2)
      ext/
        mod.rs
        types.rs                // AnimeExtension (Installed/Available/Untrusted), AvailableSource, ExtensionSource trait
        query.rs                // queryFilter parity (behavior 1.4)
        group.rs                // GetAnimeExtensionsByType + grouping (behavior 1.2, 1.3)
        model.rs                // ExtensionsModel (state, search debounce, install-step map)
        filter_model.rs         // language filter model (behavior 2.6)
        details_model.rs        // details model (behavior 4.1)
        icons.rs                // installed icon from bundle, available icon cache (behavior via area:34)
      source/
        list.rs                 // source list rows (presentation of part 2 model)
        browse.rs               // browse state machine + grids/list (behavior 7.1)
        toolbar.rs              // browse toolbar actions (behavior 7.2)
        filter.rs               // source filter rows (behavior 9)
        icons.rs                // AnimeSourceIcon rules (stub warning, custom, local, default)
      global/
        toolbar.rs              // progress + chips (behavior 8.3)
        results.rs              // per-source result rows (behavior 8.1, 8.2)
      migration/
        flags.rs                // MigrateFlags bit layout (behavior 10, data 5.1)
        engine.rs               // migrate_anime(): steps 1-9 (behavior 12)
        dialog.rs               // confirm dialog (behavior 11)
        favorites.rs            // source favorites list (behavior 14)
        search.rs               // migrate search + per-source search (behavior 15, 16)
        seasons.rs              // season picker (behavior 17)
        source_picker.rs        // source picker + sort (behavior 13)
      sprefs.rs                 // source preferences page (behavior 5), native widgets
  widgets/
    banner.rs                   // InfoBanner (Revealer) for toast/snackbar parity (D17)
    cover_grid.rs               // shared GridView factory for covers (also area:40/37)
```

Entry integration (mockup): `src/entry.rs` functions `browse()`, `sources()`, `extensions()`, `migrate_sources()`, `browse_source()`, `seasons()`, `global_search()`, `migrate_entries()` are replaced by calls into `browse::anime::*`. Keep the function names during the transition so `src/main.rs` (`tabs()`, `sidebar()`) does not change.

## 3. Core types

### 3.1 Pins and enums (`browse/anime/model.rs`)

```rust
/// Mirrors tachiyomi.domain.source.anime.model.Pin (behavior 6, data 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin(pub u8);
impl Pin {
    pub const UNPINNED: Pin = Pin(0b00);
    pub const PINNED: Pin = Pin(0b01);
    pub const ACTUAL: Pin = Pin(0b10);
    pub fn is_pinned(self) -> bool { self.0 & 0b01 != 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryDisplayMode { CompactGrid, ComfortableGrid, List, CoverOnlyGrid }
impl LibraryDisplayMode {
    pub const DEFAULT: Self = Self::CompactGrid;          // data 4
    pub fn from_pref(s: &str) -> Self {                    // unknown -> default
        match s {
            "COMFORTABLE_GRID" => Self::ComfortableGrid,
            "COMPACT_GRID" => Self::CompactGrid,
            "COVER_ONLY_GRID" => Self::CoverOnlyGrid,
            "LIST" => Self::List,
            _ => Self::DEFAULT,
        }
    }
    pub fn to_pref(self) -> &'static str { /* inverse, same four strings */ unimplemented!() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchType { Seasons = 0, Episodes = 1 }      // DB ordinal, animes.fetch_type DEFAULT 1

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimeSourceFilter { All, PinnedOnly }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallStep { Idle, Pending, Downloading, Installing, Installed, Error }
impl InstallStep {
    /// behavior 1.11: Installed | Error | Idle are "completed"
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Installed | Self::Error | Self::Idle)
    }
}
```

Note: the `unimplemented!()` in `to_pref` above is a signature sketch only; the implementation is the obvious inverse.

### 3.2 Extension model (`browse/anime/ext/types.rs`)

```rust
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct SourceRef {               // an installed source (AnimeSource)
    pub id: i64,
    pub name: String,
    pub lang: String,
    pub home_url: Option<String>,    // AnimeHttpSource.getHomeUrl(); None for non-HTTP sources
    pub base_url: Option<String>,
    pub configurable: bool,          // ConfigurableAnimeSource
    pub supports_latest: bool,
}

#[derive(Clone, Debug)]
pub struct StoreRef { pub index_url: String, pub name: String }     // AnimeExtensionStore subset

#[derive(Clone, Debug)]
pub struct Installed {
    pub name: String,
    pub pkg_name: String,            // real package id, e.g. "eu.kanade.tachiyomi.animeextension.en.foo"
    pub version_name: String,
    pub version_code: i64,
    pub lib_version: f64,
    pub lang: String,
    pub is_nsfw: bool,
    pub is_torrent: bool,
    pub sources: Vec<SourceRef>,
    pub has_update: bool,
    pub is_obsolete: bool,
    pub is_shared: bool,             // Android "shared uid"; Linux: always false, see section 8
    pub store: Option<StoreRef>,
    pub dir: std::path::PathBuf,     // extension bundle directory (data p1 section 1)
}

#[derive(Clone, Debug)]
pub struct AvailableSource { pub id: i64, pub lang: String, pub name: String, pub base_url: String }

#[derive(Clone, Debug)]
pub struct Available {
    pub name: String,
    pub pkg_name: String,
    pub version_name: String,
    pub version_code: i64,
    pub lib_version: f64,
    pub lang: String,
    pub is_nsfw: bool,
    pub is_torrent: bool,
    pub sources: Vec<AvailableSource>,
    pub apk_url: String,             // bundle URL (name kept for parity with the index)
    pub icon_url: String,
    pub store: StoreRef,
}

#[derive(Clone, Debug)]
pub struct Untrusted {
    pub name: String,
    pub pkg_name: String,
    pub version_name: String,
    pub version_code: i64,
    pub lib_version: f64,
    pub signature_hash: String,
    pub lang: Option<String>,
    pub is_nsfw: bool,               // always false (behavior 1.2, data 3.1)
    pub is_torrent: bool,            // always false
}

/// Shared handle types; models hold these, widgets never own them.
pub type InstalledRef = Arc<Installed>;
pub type AvailableRef = Arc<Available>;
pub type UntrustedRef = Arc<Untrusted>;

#[derive(Clone, Debug)]
pub enum AnyExt { Installed(InstalledRef), Available(AvailableRef), Untrusted(UntrustedRef) }
impl AnyExt {
    pub fn pkg_name(&self) -> &str { /* match each variant */ unimplemented!() }
    pub fn name(&self) -> &str { unimplemented!() }
    pub fn is_nsfw(&self) -> bool { unimplemented!() }
    pub fn is_torrent(&self) -> bool { unimplemented!() }
}
```

Every `unimplemented!()` above is a sketch of a trivial match; implementations must be written out.

### 3.3 Source service boundary (what the browse UI calls)

```rust
/// Implemented by the extension manager (area:19). Models depend only on this trait.
#[async_trait::async_trait]    // or native async fn in trait; pick one crate-wide
pub trait AnimeExtensionService: Send + Sync + 'static {
    fn installed(&self) -> tokio::sync::watch::Receiver<Vec<InstalledRef>>;
    fn available(&self) -> tokio::sync::watch::Receiver<Vec<AvailableRef>>;
    fn untrusted(&self) -> tokio::sync::watch::Receiver<Vec<UntrustedRef>>;

    /// behavior 1.6: network refresh of every store index. Errors become a banner.
    async fn refresh_available(&self) -> Result<(), ExtError>;

    /// behavior 1.8: progress as a stream; completes on Installed or Error.
    fn install(&self, ext: AvailableRef) -> futures::stream::BoxStream<'static, InstallStep>;
    fn update(&self, ext: InstalledRef) -> futures::stream::BoxStream<'static, InstallStep>;
    fn cancel_install(&self, pkg_name: &str);
    fn uninstall(&self, pkg_name: &str) -> Result<(), ExtError>;
    async fn trust(&self, ext: UntrustedRef) -> Result<(), ExtError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ExtError {
    #[error("network: {0}")] Network(String),
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("invalid bundle: {0}")] InvalidBundle(String),
    #[error("not found: {0}")] NotFound(String),
}
```

`ExtError::Network` on refresh is shown as the banner text `Failed to fetch available extensions` (`MR.strings.extension_api_error`).

## 4. Models and threading

### 4.1 Model pattern

```rust
pub struct ExtensionsModel {
    svc: Arc<dyn AnimeExtensionService>,
    prefs: Arc<prefs::BrowsePrefs>,
    state: std::cell::RefCell<ExtensionsState>,          // GTK thread only
    // events back to the view
    tx: async_channel::Sender<ExtensionsEvent>,
}

#[derive(Clone, Default)]
pub struct ExtensionsState {
    pub is_loading: bool,            // starts true (behavior 1.5 State.isLoading = true)
    pub is_refreshing: bool,
    pub items: Vec<ExtGroup>,        // ItemGroups, ordered
    pub updates: usize,              // animeext_updates_count
    pub installer: Option<InstallerKind>,
    pub search: String,              // "" means none (behavior 1.5)
    pub install_steps: std::collections::HashMap<String, InstallStep>, // keyed by synthetic pkg
}

#[derive(Clone)]
pub struct ExtGroup { pub header: Header, pub items: Vec<ExtItem> }
#[derive(Clone)]
pub enum Header { Resource(ResKey), Text(String) }
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResKey { UpdatesPending, Installed }     // MR.strings.ext_updates_pending / ext_installed
#[derive(Clone)]
pub struct ExtItem { pub ext: AnyExt, pub step: InstallStep }

pub enum ExtensionsEvent { Changed, Toast(String), RefreshDone }
```

Event flow: the model subscribes to the three `watch` receivers and to a 250 ms debounce timer on the search text (behavior 0: `SEARCH_DEBOUNCE_MILLIS = 250`). On each change it recomputes groups with the pure function `group::build(...)` (section 5.2) and sends `Changed` on the channel. The view `glib::spawn_future_local`s the receiver and re-renders.

### 4.2 Threading

| Work | Where | Mechanism |
|---|---|---|
| Widget creation, state mutation | GTK main thread | `Rc<RefCell<..>>`; `glib::spawn_future_local` |
| `watch` receiver forwarding | tokio task | `tokio::spawn`; sends into `async_channel` |
| Network (refresh, bundle download) | tokio | `reqwest` (area:12 client) |
| DB reads/writes (migrate, prefs) | tokio blocking pool | `tokio::task::spawn_blocking` around a rusqlite connection owned by area:05 |
| Icon decode (zip + image) | tokio blocking pool | `spawn_blocking`, result `gdk::Texture` created on main thread |
| Timers (debounce, 1 s min refresh spinner) | glib | `glib::timeout_add_local_once(Duration::from_millis(250), ..)` |

Rule: `gdk::Texture`, `gtk::Widget` and `Rc` never leave the GTK thread. Send `Vec<u8>` / `Arc<Installed>` across threads.

### 4.3 Concurrency semantics that must be kept

- `update_all` spawns one independent task per installed extension with `hasUpdate` in the CURRENT filtered set (behavior 1.7). Implementation: `for ext in visible_updates { tokio::spawn(svc.update(ext)) }`.
- Source search pool: `tokio::task::JoinSet` bounded by a `tokio::sync::Semaphore::new(5)` (part 2 owns; mention here because migrate search inherits the bound).
- Migration (behavior 12) is one sequential task per dialog confirm. The dialog disables both buttons while running (fixes the silent-return bug, T-41a-035).

### 4.4 Page construction (replaces Voyager `Screen` + `ScreenModel`)

```rust
pub enum BrowsePage {
    Sources,                                     // sources list (part 2 model provides items)
    SourceFilter,
    Extensions,
    ExtensionFilter,
    ExtensionDetails { pkg_name: String },
    SourcePrefs { source_id: i64 },
    BrowseSource { source_id: i64, listing: Listing, query: Option<String> },   // part 2 Listing
    GlobalSearch { query: String, ext_filter: Option<String> },
    MigrateFavorites { source_id: i64 },
    MigrateSourcePicker,
    MigrateSearch { anime_id: i64 },
    MigrateSourceSearch { old_anime_id: i64, source_id: i64, query: Option<String> },
    MigrateSeasons { old_anime_id: i64, parent_anime_id: i64 },
}
pub fn open(page: BrowsePage) -> gtk::Widget { /* builds the widget + model, pushes via ui::push */ unimplemented!() }
```

`ui::push` (mockup `src/ui.rs:34`) is reused as the navigation primitive; `ui::pop` (`:40`) and `pop_to_root` (`:49`) cover `popUntilRoot`. `popUntil lastItem` (behavior 11.2) needs a page id stack: add `ui::pop_until(predicate)` (Proposal).

## 5. Pure logic (no GTK), exact signatures

### 5.1 Query (behavior 1.4)

```rust
/// Exact parity with queryFilter. Empty query matches everything.
pub fn matches_query(ext: &AnyExt, query: &str) -> bool;
```

Implementation contract (write it exactly this way):
- `if query.is_empty() { return true }`
- `query.split(',').any(|raw| { let input = raw.trim(); if input.is_empty() { return false } ... })`
- `let id_match = input.parse::<i64>().ok();` (Kotlin `toLongOrNull`)
- Available: `sources.iter().any(|s| contains_ci(&s.name, input) || contains_ci(&s.base_url, input) || Some(s.id) == id_match) || contains_ci(&ext.name, input)`
- Installed: `sources.iter().any(|s| contains_ci(&s.name, input) || Some(s.id) == id_match || s.home_url.as_deref().is_some_and(|u| contains_ci(u, input))) || contains_ci(&ext.name, input)`
- Untrusted: `contains_ci(&ext.name, input)`
- `contains_ci(h, n) = h.to_lowercase().contains(&n.to_lowercase())` (Kotlin `ignoreCase`; use Unicode lowercasing, as Kotlin does for ASCII+Unicode; document in tests).

### 5.2 Grouping (behavior 1.2, 1.3)

```rust
pub struct ExtensionSnapshot {
    pub installed: Vec<InstalledRef>,
    pub untrusted: Vec<UntrustedRef>,
    pub available: Vec<AvailableRef>,
}

pub fn build(
    snap: &ExtensionSnapshot,
    show_nsfw: bool,
    enabled_langs: &std::collections::HashSet<String>,
    lang_display: &dyn Fn(&str) -> String,       // LocaleHelper.getSourceDisplayName
    query: &str,
    steps: &std::collections::HashMap<String, InstallStep>,
) -> Vec<ExtGroup>;
```

Algorithm (exact):
1. Installed: filter `show_nsfw || !is_nsfw`; sort by `(!is_obsolete, name.to_lowercase())`; partition `has_update` -> `(updates, installed)`.
2. Untrusted: sort by `name.to_lowercase()` (no NSFW filter).
3. Available: keep `ext` iff `!installed.any(pkg == ext.pkg) && !untrusted.any(pkg == ext.pkg) && (show_nsfw || !ext.is_nsfw)`; then flat-map each source `s` with `enabled_langs.contains(&s.lang)` to `Available { sources: vec![s.clone()], name: s.name, lang: s.lang, pkg_name: format!("{}-{}", ext.pkg, s.id), ..ext.clone() }`; sort by `name.to_lowercase()`.
4. Apply `matches_query` as a filter on each list (updates, installed, untrusted, available) BEFORE grouping, as in the Kotlin.
5. Output order: `[UpdatesPending (if updates non-empty)] ++ [Installed (if installed ++ untrusted non-empty; items = installed then untrusted)] ++ per-language Text groups (keys = lang_display(lang), sorted by the language comparator)`.
6. Language comparator (`LocaleHelper.comparator`): `"all"` first; else compare `lang_display(a)` vs `lang_display(b)` lexicographically (Kotlin `String.compareTo`; Rust: `str::cmp` on the display strings).
7. Each `ExtItem.step = steps.get(pkg_name).copied().unwrap_or(InstallStep::Idle)` (keyed by the synthetic name for Available rows).

Complexity: O(n log n) per recompute. n is in the low thousands at most; recompute on each change is fine. Proposal: skip recompute when `search` and snapshot are unchanged (cache key = snapshot generation + query + prefs generation).

### 5.3 Get-by-type (domain-level query, behavior 1.2)

Implemented inside `ExtensionsModel::recompute` by calling `build` on the latest snapshot. The enabled-language set comes from `prefs.enabled_languages()` (data 2.1). Subscribe to pref changes with `glib`-thread notifications (area:02 provides a `watch` for each typed pref).

### 5.4 Migration flags (behavior 10, data 5.1)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrateFlags(pub u32);
impl MigrateFlags {
    pub const EPISODES: u32 = 0b00001;
    pub const CATEGORIES: u32 = 0b00010;
    pub const CUSTOM_BACKGROUND: u32 = 0b00100;
    pub const CUSTOM_COVER: u32 = 0b01000;
    pub const DELETE_DOWNLOADED: u32 = 0b10000;
    pub const DEFAULT_SAVED: u32 = i32::MAX as u32;   // Int.MAX_VALUE, behavior 10
    pub fn has(self, bit: u32) -> bool { self.0 & bit != 0 }
}

pub struct FlagOption { pub bit: u32, pub default_selected: bool, pub label: &'static str }

/// Order is fixed: EPISODES (only if fetch == Episodes), CATEGORIES, CUSTOM_COVER, CUSTOM_BACKGROUND, DELETE_DOWNLOADED.
pub fn options_for(anime: &AnimeInfo, saved: MigrateFlags) -> Vec<FlagOption>;

pub fn selected_bitmap(selected: &[bool], options: &[FlagOption]) -> u32;
```

`AnimeInfo` is the subset {fetch_type, has_custom_cover, has_custom_background, download_count}.

`label` values: `"Episodes"`, `"Categories"`, `"Custom cover"`, `"Custom background"`, `"Delete downloaded"` (behavior 10 table).

### 5.5 Sort helpers

- Extension groups: `(!is_obsolete, name.to_lowercase())`.
- Migrate favorites: `title.to_lowercase()` (behavior 14; Kotlin `CASE_INSENSITIVE_ORDER`).
- Migrate source picker (owned by part 2 data but rendered here): `GetAnimeSourcesWithFavoriteCount` semantics in behavior 13.1 (`stub` first; ALPHABETICAL by `name.to_lowercase()` with collator; TOTAL by count asc; DESC is reversed comparator including the stub rule reversed too (parity: `Collections.reverseOrder(sortFn)` reverses the whole comparator, so stubs end up LAST in DESCENDING; document in tests)).

## 6. Migration engine (`browse/anime/migration/engine.rs`)

```rust
pub struct MigrateCtx<'a> {
    pub db: &'a DbPool,                         // area:05 handle
    pub net: &'a dyn SourceRuntime,             // area:16 source calls (episode/season fetch)
    pub downloads: &'a dyn DownloadService,     // area:24 (deleteAnime, downloadCount)
    pub covers: &'a dyn CoverCacheService,      // area:34
    pub backgrounds: &'a dyn CoverCacheService, // area:34 (same trait, different dir)
    pub trackers: &'a dyn EnhancedTrackers,     // area:27: filter + migrate_track
    pub prefs: &'a BrowsePrefs,
}

pub struct MigrateReq {
    pub old: AnimeRow,
    pub new: AnimeRow,
    pub replace: bool,            // false = "Copy", true = "Migrate"
    pub flags: MigrateFlags,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("source not installed: {0}")] SourceMissing(i64),
    #[error("episode fetch failed: {0}")] FetchFailed(String),
    #[error("database: {0}")] Db(String),
    #[error("io: {0}")] Io(#[from] std::io::Error),
}

/// Behavior 12. Order of side effects is part of the contract.
pub async fn migrate_anime(ctx: &MigrateCtx<'_>, req: MigrateReq) -> Result<MigrateOutcome, MigrateError>;

pub struct MigrateOutcome { pub new_anime_id: i64, pub skipped_tracks: Vec<i64> }
```

Required order inside `migrate_anime` (behavior 12, this exact sequence):
1. `ctx.prefs.save_migrate_flags(req.flags)` (always, before anything else; behavior 11.3 step 1). On the anime side write to the new key `anime_migrate_flags` (data 2.5).
2. Resolve `new_source = ctx.net.source(req.new.source)` else `Err(SourceMissing)`. (Kotlin silently returns; Kurobayomi returns an error and shows it, T-41a-035.)
3. `ctx.net.refresh_episodes(new_source, &req.new).await.map_err(FetchFailed)?` (behavior 12 step 1). Nothing has been written yet, so a failure leaves the DB untouched.
4. Begin one DB transaction (Proposal; Kotlin has none, data 7 note).
5. If `flags.has(EPISODES)`: read episodes of old and new; compute `max_seen`; for each new episode with `is_recognized_number`: copy `date_fetch` and `bookmark` from the matching old episode (same recognized episode number), set `seen = true` if `episode_number <= max_seen`; ALSO copy `last_second_seen` and `fillermark` from the matching old episode (E20 fix, Proposal; the Kotlin does not; flagged as a deliberate deviation in T-41a-034). Write all episodes.
6. If `flags.has(CATEGORIES)`: `set_categories(new.id, categories_of(old.id))`.
7. Tracks (always): for each track of old: copy with `anime_id = new.id`; if an enhanced tracker `accepts` the old track (`is_track_from`), use `migrate_track(track, new, new_source)`; `None` drops the track (record in `skipped_tracks`); else keep. Insert all remaining with `insert_tracks`.
8. If `flags.has(DELETE_DOWNLOADED)` and old source resolvable: `downloads.delete_anime(old, old_source)`. (Outside the transaction; a file-system side effect cannot roll back. Proposal: run after commit.)
9. If `replace`: set old `favorite = false`, `date_added = 0`.
10. If `flags.has(CUSTOM_COVER)` and `covers.has_custom(old.id)`: `covers.copy_custom(old.id, new.id)` (file copy, no leaked handle).
11. If `flags.has(CUSTOM_BACKGROUND)` and `backgrounds.has_custom(old.id)`: same.
12. Set new `favorite = true`, `episode_flags = old.episode_flags`, `viewer = old.viewer_flags`, `date_added = if replace { old.date_added } else { now_ms() }`.
13. Commit transaction; then step 8 (downloads) if it was deferred.

Error policy (Proposal, departs from observed silent behavior): any `Err` aborts before step 12, rolls back the transaction, leaves old and new favorites unchanged, and returns the error to the dialog, which shows `"Migration failed: <reason>"` inline and re-enables the buttons (behavior 11.3).

## 7. Views (GTK widgets)

Mapping of Compose constructs (widgets are GTK4 only; no libadwaita):

| Compose construct (Kotlin) | GTK4 replacement | Notes |
|---|---|---|
| `Scaffold` + `AppBar` | `gtk::Box` (vertical) with `gtk::HeaderBar` (title, back button, end actions) | `ui::page` in mockup (`src/ui.rs:67`) already does this |
| `SearchToolbar` | `gtk::SearchBar` + `gtk::SearchEntry` (toggle via Ctrl+F) | `search-changed` debounce 250 ms in model, not in widget |
| `LazyColumn` / `FastScrollLazyColumn` | `gtk::ListBox` for small lists (extensions, source filter), `gtk::ListView` for large | Section headers: `gtk::ListBox::set_header_func` for ListBox; for ListView use one model per section (see pitfalls) |
| `LazyVerticalGrid` (browse covers) | `gtk::GridView` with `gtk::SignalListItemFactory` + `gtk::FlowBox` for 128 dp adaptive | Column count: `gtk::GridView::set_max_columns` |
| `LazyRow` (global search cards) | `gtk::ScrolledWindow` (horizontal) + `gtk::Box` (horizontal) | 96 dp cards |
| `AlertDialog` | `gtk::AlertDialog` (GTK 4.10+, async `choose`) | confirm dialogs (remove, trust, NSFW, migrate) |
| Options dialog (`AnimeSourceOptionsDialog`) | `gtk::Popover` anchored to row, or `gtk::AlertDialog` with buttons | Popover is the GNOME-style choice |
| `DropdownMenu` + `RadioMenuItem` | `gtk::PopoverMenu` from `gio::Menu` with a stateful action `browse.display-mode` (string state) | Radio items = `gio::MenuItem` with target |
| Overflow `AppBar.OverflowAction` | `gtk::MenuButton` with `gio::Menu` | |
| `FilterChip` | `gtk::ToggleButton` with css class `pill` | `set_active` mirrors selected |
| `Switch` / `SwitchPreferenceWidget` | `gtk::Switch` in `gtk::ListBoxRow` | |
| `Checkbox` | `gtk::CheckButton` (`set_sensitive(false)` for read-only display) | |
| `LinearProgressIndicator` | `gtk::ProgressBar` (`set_fraction`) overlay at bottom | Visible when `1 <= progress < total` |
| `CircularProgressIndicator` | `gtk::Spinner` (`start()`) | 40 dp box |
| `LoadingScreen` / `EmptyScreen` | `gtk::Box` centered with `gtk::Spinner` / `gtk::Label` + `gtk::Button` actions | Empty actions = `EmptyScreenAction` list |
| Snackbar (Retry) | `widgets::banner::InfoBanner` (`gtk::Revealer` + label + button); `duration = Indefinite` -> no auto-hide | D17 |
| Toast | same banner, 4 s auto-hide | |
| `PullRefresh` | `gtk::Button` (refresh icon) + F5 accelerator | D11 |
| `animateItem` | none (D9) | |
| `ScrollbarLazyColumn` | `gtk::ScrolledWindow` (overlay scrollbars default) | |
| `copyToClipboard` | `gdk::Display::default().unwrap().clipboard().set_text(text)` | D12 |
| `UriHandler.openUri` | `gtk::UriLauncher::new(uri).launch(win, None, |r| ..)` (GTK 4.10+) | D13 |

## 8. Android -> Linux mapping (everything touched by this part)

| Android/Kotlin | Linux/Rust replacement | Decision |
|---|---|---|
| `PackageManager` installed check, `isPackageInstalled` | `Path::exists()` on extension dir (`Installed::dir`) | Replace (D3) |
| `PackageManager.getResourcesForApplication` icon (density) | `zip` crate reads bundle icon entry; `image` decodes to 96 px PNG cache | Replace (D8) |
| `Settings.ACTION_APPLICATION_DETAILS_SETTINGS` (app info) | `gtk::FileLauncher` on `Installed::dir` ("Open folder") | Replace (D1) |
| "Install unknown apps" permission | none | Drop (D2) |
| `Installed.isShared` (Android shared uid) | always `false` on Linux; field kept for parity with data | Drop-with-stub (no user-visible effect) |
| AndroidX `PreferenceFragmentCompat` | native `gtk::ListBox` settings page from a Rust `SourcePrefSpec` enum | Rewrite (D4) |
| Coil `AsyncImage` | `reqwest` + on-disk cache + `gdk::Texture::from_bytes` | Replace (D7) |
| Android `Toast` | `InfoBanner` | Replace (D17) |
| Android clipboard | GDK clipboard | Replace (D12) |
| `Intent`/`Uri` | `gtk::UriLauncher` | Replace (D13) |
| `LocaleHelper` (`Locale.getDisplayName`) | `icu_displaynames` (ICU4X) `DisplayNames::try_new` with `LanguageDisplay::Dialect` (fallback table for `zh-Hans`/`zh-Hant`) | Replace |
| `Executors.newFixedThreadPool(5)` | tokio `Semaphore::new(5)` | Replace |
| `Channel` events | `async_channel` | Replace |
| `StateFlow` / `Flow` | `tokio::sync::watch` + `async_channel` to GTK | Replace |
| Voyager `Screen`, `ScreenModel`, `Navigator` | `BrowsePage` enum + `ui::push/pop` + page-id stack | Replace (section 4.4) |
| `Injekt.get()` DI | constructor injection (`Arc<dyn ...>`) from the app root (area:01) | Replace |
| `kotlinx.collections.immutable` | `Arc<[T]>` or `Vec<T>` snapshots | Replace |
| `ScreenModel.screenModelScope` | `glib::MainContext::default().spawn_local` for GTK; tokio for I/O | Replace |
| `ioCoroutineScope` | `tokio::runtime::Handle` (area:01) | Replace |
| `withUIContext` | `glib::idle_add_once` or `async_channel` send | Replace |
| `LocalConfiguration.orientation` | window width breakpoint 720 px (`ui::window().width()` on resize) | Replace (D14) |
| `LocalContext` / `LocalUriHandler` | none needed (pass handles explicitly) | n/a |
| Fragment + reflection | none | Drop (D4, D18) |
| WebView | area:36 owns (`webkit6` if chosen there) | Replace (area:36) |

## 9. Crates (add to Cargo.toml; versions pinned at implementation time from crates.io, not guessed here)

| Crate | Use | Why this one |
|---|---|---|
| `gtk4` 0.11 (already, feature `v4_16`) | UI | the target; `gtk::AlertDialog`, `gtk::UriLauncher`, `gtk::FileLauncher` need 4.10+/4.16 |
| `glib` (transitive) | main loop, `spawn_future_local` | |
| `async-channel` | tokio -> GTK event hop | works without a GLib main context |
| `tokio` (`rt-multi-thread`, `sync`, `time`, `macros`) | I/O runtime | area:01 owns the runtime |
| `futures` (`stream`) | `BoxStream` for install progress | |
| `reqwest` (`rustls-tls`, `stream`) | icon/APK downloads | area:12 owns the client |
| `rusqlite` (`bundled`) | DB | area:05 owns the connection |
| `serde`, `serde_json` (already) | index and prefs JSON | |
| `zip` | read icon out of extension bundle | std-only fallback impossible |
| `image` (`png`, `jpeg`, `webp`) | decode/resize icons and covers | |
| `thiserror` | error enums | |
| `icu_displaynames` (ICU4X) | locale display names | parity with `Locale.getDisplayName` |
| `async-trait` (optional) | trait objects with async methods | or use `Pin<Box<dyn Future>>` |
| `im` (optional) | persistent maps for UI snapshots | optional; `Vec`/`Arc` is enough |

Rejected: libadwaita (`adw`) — out of scope by requirement; `relm4` — adds a second component model on top of the mockup, not needed; `rayon` — not needed (I/O bound).

## 10. Responsive layout

- Column count in covers grids: `GridCells.Fixed(n)` when the prefs value is `n > 0` (`pref_animelib_columns_*`), else adaptive 128 dp (behavior 17). In GTK: `gtk::GridView::set_min_columns(1)`, and when `n > 0` set both min and max columns to `n`; when `0`, use `set_max_columns(u32::MAX)` with a child `width_request(128)`.
- Landscape vs portrait prefs (`animeLandscapeColumns`, `animePortraitColumns`) become a width breakpoint: width >= 720 px uses the landscape key, else portrait (D14). Documented in T-41a-040.
- Extension list max width 960 px centred (GNOME-style); details page `set_max_content_width(640)`.
- Global search chips row: `gtk::ScrolledWindow` with `policy: (Automatic, Never)`.

## 11. Pitfalls

1. `gtk::ListView` sections: GTK 4.16 has no built-in section headers for every model type. The extension list uses `gtk::ListBox` with `set_header_func` (headers are non-selectable rows drawn by the func). Rebuild on each recompute (list is small).
2. Covers grid recycling: `SignalListItemFactory` must rebind `gtk::Picture` textures; cancel the in-flight cover future when the item is unbound (store a `glib::JoinHandle`-like guard) or the wrong cover can flash.
3. `gdk::Texture` must be created on the main thread: decode on tokio, create texture in `spawn_future_local`.
4. `gtk::SearchEntry::search-changed` fires after 150 ms internally; the 250 ms debounce is applied in the model, not in the widget.
5. `gtk::AlertDialog::choose` is asynchronous: do not call `run` expecting a blocking answer (there is no `gtk::Dialog::run` in GTK4).
6. `gtk::PopoverMenu` with radio items needs a stateful action of type `s` (string) and `gio::MenuItem::set_action_and_target_value`.
7. Migration dialog must disable its own buttons while `migrate_anime` runs, otherwise a double click starts two migrations (Kotlin's `isMigrating` overlay prevents this; Rust must too).
8. Extension bundle ZIP can be huge: read only the icon entry, never extract the whole archive for an icon.
9. Synthetic `pkg-<id>` keys must never be written to the DB or prefs; only the real `pkg` goes to `disabled_anime_sources` / `incognito_anime_extensions` (the source ids are numeric; `incognito` uses the real pkg).
10. Source toggle ids are decimal strings in prefs (`"$sourceId"`); parse back with `i64::from_str`, never with `f64`.
11. `enabled_languages` default depends on the device locale: compute once at first run and store; do not recompute on every launch (Kotlin recomputes only when the pref is absent).
12. `lang` may be `""` for local sources; `display name` for `""` is the system language (behavior 0). Guard against infinite recursion in `icu` lookups.
13. `Installed` InstallStep transient state: render like Idle (behavior 4.4 note).
14. `pkg_name.substringAfter("eu.kanade.tachiyomi.animeextension.")` must be applied to the REAL pkg (header display), not the synthetic.

## 12. Integration with the existing mockup

| Mockup (file:function) | Becomes | Notes |
|---|---|---|
| `src/entry.rs:244 browse()` | `browse::anime::facade::browse_tabs()` returns the 6-tab stack (2 sources, 2 extensions, 2 migrate sources) | Keep stack/switcher code; swap contents |
| `src/entry.rs:283 sources(anime)` | `source::list::build(model)` | mock "Last used/Pinned/English/Other" groups replaced by real `Header.Text` groups from part 2 |
| `src/entry.rs:311 extensions(anime)` | `ext::list::build(model)` | |
| `src/entry.rs:360 migrate_sources(anime)` | `migration::source_picker::build(model)` | |
| `src/entry.rs:380 browse_source(id, migrating)` | `BrowsePage::BrowseSource` / `BrowsePage::MigrateSourceSearch` | |
| `src/entry.rs:427 seasons()` | `BrowsePage::MigrateSeasons` | |
| `src/entry.rs:432 global_search(id, migrating)` | `BrowsePage::GlobalSearch` / `MigrateSearch` | |
| `src/entry.rs:482 migrate_entries()` | `BrowsePage::MigrateFavorites` | |
| `src/entry.rs:23-24` dialog ids `migrate_anime`/`migrate_manga` | `migration::dialog::confirm(...)` | |
| `src/settings.rs:13` `settings_anime_extension_stores` | area:19 stores page (out of scope here) | link only |
| `src/ui.rs:88 open(id)` | `open(BrowsePage)` for browse pages; spec ids remain for other areas | |
| `spec/*.json` | static copy only; source of truth for strings is the MR/AYMR English list (p1-behavior.md section 24) | |

Mockup bugs to fix while integrating (observed in `src/entry.rs:244-280`): the Filter button closure calls `open("sources_filter")` unconditionally after a dead `ext` variable (lines 268-272), so the extension-tab "Filter" menu item and the sources filter button do different things only by accident; the Latest and row-activate handlers always open `browse_source` without a source id; the pin ToggleButton does not reflect model state.

## 13. Error handling policy

- Library errors -> typed enums (`ExtError`, `MigrateError`) via `thiserror`.
- Only the GTK boundary converts errors to text: banner or dialog message. Never `unwrap` on user data (Kotlin `!!` on `getAnime(...)!!`, `state.source!!`, `anime.title`; Rust must return a `NotFound` banner instead).
- Network errors in the extension list: banner `Failed to fetch available extensions`, list shows the last good snapshot.
- Log with `tracing` (area:01).

## 14. Testing hooks

- All pure functions (sections 5.1-5.5, 6 with a fake `MigrateCtx`) are testable without GTK. Test plan in p1-edge-cases-and-tests.md.
- GTK widgets tested with `gtk::init` in a headless test only for construction (no display in CI: `GDK_BACKEND=broadway` or skip).
