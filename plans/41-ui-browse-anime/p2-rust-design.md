# Area 41 part 2: Rust design (sources list, browse, global search, source filters, migration sources)

Target: Kurobayomi (`/home/blackbox/kurobayomi`), Rust 2024 edition, `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }` (from `Cargo.toml`), vanilla GTK (no libadwaita), `serde`, `serde_json`. Behavior is defined in `p2-behavior.md`; this file defines how it is built. Part 1 owns the widgets; the module tree below matches the module names used in `p1-inventory.md` (for example `browse/anime/source_browse.rs`, `browse/anime/global_search.rs`, `browse/anime/migration/sources.rs`).

Rust type names are proposals. Items marked **PROPOSAL** are new. Items marked **OBSERVED** restate Kotlin behavior with a citation in `p2-behavior.md`.

---

## 1. Decisions (summary)

| ID | Decision | Rationale | Behavior change? |
|---|---|---|---|
| D-1 | Language order is display-name order with "all" first, in both the Sources list and the Visibility page (Kotlin uses raw code order in the list and display order on the Visibility page) | One ordering rule across screens | Yes, list order changes for non-English locales. Recommended. |
| D-2 | Empty first page shows the "No results found" empty state with Retry, Open in WebView, Help (no error snackbar) | Same text and actions, avoids the snackbar-plus-retry flow for an expected result | No visible text change; the error path becomes an empty state |
| D-3 | Favorite toggle writes only `favorite`, `date_added`, `cover_last_modified`, `background_last_modified` (narrow update), not the full row | Kotlin rewrites every column from a stale snapshot (E-BR-02). Narrow update removes the clobber | Yes, fixes a data-loss race |
| D-4 | `TriState` unknown int maps to `Ignore` with a logged warning; Sort with no selection is treated as index 0, ascending | Kotlin crashes (E-FD-01, E-FD-03) | Yes, no crash |
| D-5 | Global search generation guard: results from a superseded query are dropped | Kotlin can write stale results (E-GS-03) | Yes, fixes a race |
| D-6 | Global search concurrency uses one shared semaphore of 5 permits per app, not a thread pool per screen | Kotlin leaks a pool per screen (E-GS-08) | No visible change |
| D-7 | Listing equality is by value (`query` + filter snapshot), not by object identity | Kotlin reloads on unrelated state changes (E-BR-06) | Yes, no spurious reloads |
| D-8 | Non-favorite title refresh on browse: **open question** (see section 12, Q-1). Default: keep Kotlin behavior (display DB title) | Needs a product decision | No, by default |

---

## 2. Module layout

```
src/
  browse/
    mod.rs                      // pub mod anime;  (browse tab entry points used by entry.rs)
    anime/
      mod.rs                    // re-exports; pub fn sources_page(), browse_page(), global_search_page(), migrate_sources_page()
      model.rs                  // PURE (no gtk): SourceRef, SourceRow, SourceSection, section ordering, Listing, FilterList, SearchResult, comparators
      sources.rs                // Sources tab: SourcesModel (controller) + view (ListBox with headers)
      source_filter.rs          // Sources visibility page: VisibilityModel + view
      source_browse.rs          // Browse page: BrowseModel + view (GridView/ListView, toolbar, chips, dialogs)
      pager.rs                  // SourcePager (replaces androidx.paging Pager + AnimeSourcePagingSource)
      library_add.rs            // favorite toggle, duplicate lookup, default-category resolution, category picker model
      source_filters.rs         // filter sheet view and FilterList mutation rules (TriState, Sort, Group)
      global_search.rs          // GlobalSearchModel (fan-out) + view (toolbar, progress, chips, sections, card rows)
      widgets.rs                // (part 1) BaseAnimeSourceItem, AnimeSourceIcon, EntryComfortableGridItem, InLibraryBadge, BrowseSourceLoadingItem
      migration/
        mod.rs                  // (part 1 adds favorites.rs, seasons.rs, search.rs)
        sources.rs              // MigrateSourcesModel (sort) + view
  domain/
    anime_source.rs             // AnimeSource trait, SourceRef, AnimesPage, SAnime (provisional, owned by area:16/17/20)
    anime_filter.rs             // Filter enum tree (AnimeFilter/AnimeFilterList equivalent) (area:16)
  data/
    anime_repo.rs               // AnimeRepo trait + rusqlite implementation (area:05/07)
    anime_source_repo.rs        // stub source table, favorite counts (area:17)
  settings/
    browse_prefs.rs             // typed accessors for the keys in p2-data-and-config.md section 3 (area:02)
```

Existing mockup wiring to replace (function names from the current code):
- `src/entry.rs::browse()` (line 244): keep the Browse page scaffold; replace the hard-coded `sources(anime)` / `browse_source(id, migrating)` / `global_search(id, migrating)` / `migrate_sources(anime)` calls with the new modules above.
- `src/entry.rs::sources(anime: bool)` (line 283): replace the hard-coded sections (`"Last used"`, `"Pinned"`, `"English"`, `"Other"`) with `sources::view(&SourcesModel)`.
- `src/entry.rs::browse_source(id, migrating)` (line 380): replace the cover-grid placeholder with `source_browse::view`.
- `src/entry.rs::global_search(id, migrating)` (line 432): replace the placeholder sections with `global_search::view`.
- `src/entry.rs::migrate_sources(anime)` (line 360): replace with `migration::sources::view`.
- Keep `ui::push`, `ui::pop`, `ui::open(id)` (`src/ui.rs:34, 40, 88`) for navigation. The spec (`spec/app.json`) stays for static dialog copy only.
- `src/ui.rs::show_sheet` (line 391) and `show_dialog` (line 406) are reused for the filter sheet and simple dialogs, or replaced by the helpers in section 6.

---

## 3. Core types

### 3.1 Pure model (`browse/anime/model.rs`) — no GTK imports, unit-testable

```rust
pub type SourceId = i64;
pub const LOCAL_ANIME_SOURCE_ID: SourceId = 0;          // OBSERVED LocalAnimeSource.ID

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRef {
    pub id: SourceId,
    pub lang: String,            // "" = system, "all", "other", or BCP-47
    pub name: String,            // may be empty for stubs
    pub supports_latest: bool,
    pub is_stub: bool,
}

impl SourceRef {
    /// OBSERVED AnimeSource.visualName
    pub fn visual_name(&self) -> String {
        if self.lang.is_empty() { self.name.clone() }
        else { format!("{} ({})", self.name, self.lang.to_uppercase()) }
    }
    /// OBSERVED AnimeSource.key: "$id-lastused" for the Last-used copy, else "$id"
    pub fn row_key(&self, is_last_used_copy: bool) -> String {
        if is_last_used_copy { format!("{}-lastused", self.id) } else { self.id.to_string() }
    }
}

/// One row in the Sources list before grouping (OBSERVED GetEnabledAnimeSources output).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRow {
    pub source: SourceRef,
    pub pinned: bool,               // Pin.Actual / Pins.pinned
    pub is_last_used_copy: bool,    // isUsedLast
}

/// Section keys. OBSERVED LAST_USED_KEY / PINNED_KEY are "last_used" / "pinned".
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SourceSection {
    LastUsed,
    Pinned,
    Language(String),           // "" for sources without a language
}

impl SourceSection {
    /// OBSERVED ordering (app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt:55-66), rules 1-7.
    /// LastUsed < Pinned < Language(non-empty, in order) < Language("").
    pub fn sort_rank(&self) -> (u8, String) {
        match self {
            SourceSection::LastUsed => (0, String::new()),
            SourceSection::Pinned => (1, String::new()),
            SourceSection::Language(l) if l.is_empty() => (3, String::new()),
            SourceSection::Language(l) => (2, l.clone()),   // compared as UTF-8 bytes == code point order
        }
    }
}

pub fn section_of(row: &SourceRow) -> SourceSection {
    if row.is_last_used_copy { SourceSection::LastUsed }
    else if row.pinned { SourceSection::Pinned }
    else { SourceSection::Language(row.source.lang.clone()) }
}

/// Header label for a section (OBSERVED LocaleHelper.getSourceDisplayName).
pub fn section_title(section: &SourceSection, ctx: &dyn LocaleText) -> String {
    match section {
        SourceSection::LastUsed => ctx.text("last_used_source"),   // "Last used"
        SourceSection::Pinned => ctx.text("pinned_sources"),       // "Pinned"
        SourceSection::Language(l) => match l.as_str() {
            "other" => ctx.text("other_source"),                   // "Other"
            "all" => ctx.text("multi_lang"),                       // "Multi"
            "" => ctx.system_language_display_name(),              // OBSERVED getLocalizedDisplayName("")
            other => ctx.language_display_name(other),
        },
    }
}

/// Group rows into sections in display order. Input order within a section is preserved
/// (OBSERVED groupByTo). Input must already be sorted (GetEnabledAnimeSources step 4).
pub fn group_sources(rows: &[SourceRow]) -> Vec<(SourceSection, Vec<SourceRow>)> {
    let mut groups: Vec<(SourceSection, Vec<SourceRow>)> = Vec::new();
    for row in rows {
        let sec = section_of(row);
        match groups.iter_mut().find(|(s, _)| *s == sec) {
            Some((_, v)) => v.push(row.clone()),
            None => groups.push((sec, vec![row.clone()])),
        }
    }
    groups.sort_by(|a, b| a.0.sort_rank().cmp(&b.0.sort_rank()));
    groups
}

/// Enabled-source selection (OBSERVED app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:18-41).
pub fn enabled_source_rows(
    sources: &[SourceRef],
    pinned: &HashSet<SourceId>,
    enabled_langs: &HashSet<String>,
    disabled: &HashSet<SourceId>,
    last_used: Option<SourceId>,
) -> Vec<SourceRow> {
    let mut picked: Vec<&SourceRef> = sources
        .iter()
        .filter(|s| enabled_langs.contains(&s.lang) || s.id == LOCAL_ANIME_SOURCE_ID)
        .filter(|s| !disabled.contains(&s.id))
        .collect();
    picked.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())); // stable, name only
    let mut out = Vec::with_capacity(picked.len() + 1);
    for s in picked {
        let is_pinned = pinned.contains(&s.id);
        out.push(SourceRow { source: s.clone(), pinned: is_pinned, is_last_used_copy: false });
        if Some(s.id) == last_used {
            out.push(SourceRow { source: s.clone(), pinned: is_pinned, is_last_used_copy: true });
        }
    }
    out
}
```

Notes:
- `String::to_lowercase` is Unicode-aware. Kotlin `CASE_INSENSITIVE_ORDER` is per-char ASCII-and-Unicode simple folding. For Arabic names this makes no difference. For mixed-case Latin names it is the same.
- `HashSet<String>` for `enabled_langs` (the stored form is the set of strings).
- Pinned/disabled sets store ids as strings in settings; parse with `str::parse::<i64>()` and skip invalid entries (PROPOSAL: log and skip; Kotlin keeps unparsable strings inert).

### 3.2 Listing and filters

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listing {
    Popular,
    Latest,
    Search { query: Option<String>, filters: FilterList },
}

impl Listing {
    /// OBSERVED Listing.valueOf: sentinels map to Popular/Latest; everything else is Search.
    pub fn from_nav_arg(arg: Option<&str>) -> Listing {
        match arg {
            Some(POPULAR_SENTINEL) => Listing::Popular,
            Some(LATEST_SENTINEL) => Listing::Latest,
            other => Listing::Search { query: other.map(str::to_owned), filters: FilterList::default() },
        }
    }
    /// OBSERVED Listing.query: Popular/Latest carry the sentinel; Search carries its query.
    pub fn query_for_source(&self) -> &str {
        match self {
            Listing::Popular => "",         // handled by kind, see SourcePager::new
            Listing::Latest => "",
            Listing::Search { query, .. } => query.as_deref().unwrap_or(""),
        }
    }
    pub fn is_user_query(&self) -> bool {
        matches!(self, Listing::Search { query: Some(q), .. } if !q.is_empty())
    }
}

pub const POPULAR_SENTINEL: &str = "eu.kanade.domain.source.anime.interactor.POPULAR";
pub const LATEST_SENTINEL: &str = "eu.kanade.domain.source.anime.interactor.LATEST";
```

`Listing` equality is derived. `FilterList` must implement `PartialEq` by value (D-7). `query_for_source` is kept only so the nav-arg sentinels stay byte-identical with Kotlin for any saved link (OBSERVED domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt:19-22). Internally, prefer the enum match in `SourcePager::new`.

```rust
/// Source-defined filters. Mirrors AnimeFilter (source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    Header { name: String },
    Separator { name: String },
    Select { name: String, values: Vec<String>, state: usize },
    Text { name: String, state: String },
    CheckBox { name: String, state: bool },
    TriState { name: String, state: TriStateState },
    Group { name: String, children: Vec<Filter> },
    Sort { name: String, values: Vec<String>, state: Option<SortSelection> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriStateState { Ignore, Include, Exclude }  // ints 0, 1, 2 (AnimeFilter.TriState.STATE_*)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortSelection { pub index: usize, pub ascending: bool }

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilterList(pub Vec<Filter>);   // value equality (D-7); never "always unequal"
```

`Select` stores an index into `values`. Kotlin's `Select<V>` is generic; in the Rust port all selectable values are strings (OBSERVED use: `values.filterIsInstance<String>()` in genre search).

### 3.3 Search results (global)

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchResult {
    Loading,
    Failed { message: Option<String> },   // OBSERVED GlobalSearchErrorResultItem shows raw message (may be null)
    Ok { anime: Vec<AnimeId> },           // AnimeId = i64 (library row id after network_to_local)
}

impl SearchResult {
    pub fn is_success_non_empty(&self) -> bool { matches!(self, SearchResult::Ok { anime } if !anime.is_empty()) }
    /// OBSERVED isVisible: !onlyShowHasResults || (Success && !isEmpty)
    pub fn is_visible(&self, only_with_results: bool) -> bool {
        !only_with_results || self.is_success_non_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFilterMode { All, PinnedOnly }  // OBSERVED AnimeSourceFilter, default PinnedOnly in State
```

### 3.4 Sort comparators (pure)

```rust
/// OBSERVED AnimeSearchScreenModel.sortComparator (lines 58-64):
/// (1) non-empty Success first, (2) pinned first, (3) "name_lower (lang)" ascending.
pub fn search_sort_key(s: &SourceRef, result: Option<&SearchResult>, pinned: bool) -> (bool, bool, String) {
    let empty = !matches!(result, Some(r) if r.is_success_non_empty());
    (empty, !pinned, format!("{} ({})", s.name.to_lowercase(), s.lang))
}
```

Note the first key is `isEmpty ?: true`: `Loading`, `Failed`, and an empty `Ok` all sort last. `false < true` in Rust bool ordering, matching Kotlin.

### 3.5 Migration sort (pure)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrateSortMode { Alphabetical, Total }          // OBSERVED SetMigrateSorting.Mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection { Ascending, Descending }          // OBSERVED SetMigrateSorting.Direction

pub struct MigrateRow { pub source: SourceRef, pub favorites: i64 }

/// OBSERVED GetAnimeSourcesWithFavoriteCount.sortFn + reverseOrder for DESCENDING.
pub fn sort_migrate_rows(rows: &mut Vec<MigrateRow>, mode: MigrateSortMode, dir: SortDirection, collate: &dyn Collate) {
    rows.retain(|r| r.source.id != LOCAL_ANIME_SOURCE_ID);
    let cmp = |a: &MigrateRow, b: &MigrateRow| -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (a.source.is_stub, b.source.is_stub) {
            (true, false) => return Less,
            (false, true) => return Greater,
            _ => {}
        }
        match mode {
            MigrateSortMode::Alphabetical => collate.compare(&a.source.name.to_lowercase(), &b.source.name.to_lowercase()),
            MigrateSortMode::Total => a.favorites.cmp(&b.favorites),
        }
    };
    // Stable sort: ties keep input order (Kotlin behavior). PROPOSAL: input order is ORDER BY source id (deterministic).
    rows.sort_by(|a, b| cmp(a, b));
    if dir == SortDirection::Descending { rows.reverse(); }   // equivalent to Collections.reverseOrder(sortFn) for total orders
}

/// Locale collation (java.text.Collator equivalent). PROPOSAL: icu_collator, fallback to byte compare.
pub trait Collate { fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering; }
```

Note: `rows.reverse()` after a stable ascending sort is not identical to `Collections.reverseOrder` on ties (Kotlin reverses tie order too, since reverseOrder inverts the comparator result for ties as well — it keeps elements equal under comparator but changes their relative order because the sort is stable on the reversed comparator). For strict determinism, add a tie-breaker `source.id` ascending in both directions (PROPOSAL, documented as a deviation in E-MS-04).

---

### 3.6 Collaborator traits and records (provisional)

Owned by the areas named in each comment. The signatures are the minimum this scope needs.

```rust
pub type AnimeId = i64;

/// Library row as used by the browse UI (OBSERVED subset of Anime, p2-data-and-config.md 1.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeRow {
    pub id: AnimeId,
    pub source: SourceId,
    pub url: String,
    pub title: String,
    pub favorite: bool,
    pub date_added: i64,
    pub cover_last_modified: i64,
    pub background_last_modified: i64,
    pub thumbnail_url: Option<String>,
}

/// Narrow favorite write (D-3). None = column unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FavoritePatch {
    pub favorite: bool,
    pub date_added: i64,
    pub cover_last_modified: Option<i64>,
    pub background_last_modified: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
impl Category {
    /// OBSERVED Category.isSystemCategory (id == UNCATEGORIZED_ID = 0)
    pub fn is_system(&self) -> bool { self.id == 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryDisplayMode { CompactGrid, ComfortableGrid, List, CoverOnlyGrid }   // OBSERVED LibraryDisplayMode

/// The source API (area:16). Methods mirror eu.kanade.tachiyomi.animesource.AnimeSource.
#[async_trait::async_trait]
pub trait AnimeSource: Send + Sync {
    fn id(&self) -> SourceId;
    fn name(&self) -> &str;
    fn lang(&self) -> &str;
    fn supports_latest(&self) -> bool;
    fn is_stub(&self) -> bool { false }
    fn home_url(&self) -> Option<String> { None }        // AnimeHttpSource.getHomeUrl(); None for local/stub
    fn filter_list(&self) -> FilterList;                 // getFilterList(), defaults
    async fn popular(&self, page: u32) -> Result<AnimesPageOut, SourceError>;
    async fn latest(&self, page: u32) -> Result<AnimesPageOut, SourceError>;
    async fn search(&self, page: u32, query: &str, filters: &FilterList) -> Result<AnimesPageOut, SourceError>;
}

/// Source registry (area:17). OBSERVED AndroidAnimeSourceManager.
pub trait SourceCatalog: Send + Sync {
    /// Installed sources, including the local source (id 0). Excludes stubs.
    fn installed(&self) -> Vec<SourceRef>;
    fn get(&self, id: SourceId) -> Option<Arc<dyn AnimeSource>>;
    /// Installed source, else stub (`StubAnimeSource` with empty lang/name when unknown).
    fn get_or_stub(&self, id: SourceId) -> Arc<dyn AnimeSource>;
    /// Source ids that belong to the installed extension with this package name.
    fn installed_extension_sources(&self, pkg: &str) -> Vec<SourceId>;
    /// Package name of the extension that provides this source (for incognito checks).
    fn extension_package_of(&self, id: SourceId) -> Option<String>;
    /// True once the first installed-extension scan finished (OBSERVED isInitialized).
    fn is_initialized(&self) -> bool;
}

/// Anime table access (area:05/07). Implemented on the DB worker thread.
#[async_trait::async_trait]
pub trait AnimeRepo: Send + Sync {
    async fn find_by_url_and_source(&self, url: &str, source: SourceId) -> Result<Option<AnimeRow>, DbError>;
    async fn insert(&self, row: &NewAnime) -> Result<AnimeId, DbError>;
    async fn update_favorite(&self, id: AnimeId, patch: &FavoritePatch) -> Result<(), DbError>;
    async fn duplicate_favorites(&self, id: AnimeId, lower_title: &str) -> Result<Vec<AnimeRow>, DbError>;
    async fn favorite_counts_by_source(&self) -> Result<Vec<(SourceId, i64)>, DbError>;
    async fn set_categories(&self, anime: AnimeId, category_ids: &[i64]) -> Result<(), DbError>;
    async fn categories_of(&self, anime: AnimeId) -> Result<Vec<Category>, DbError>;
    async fn user_categories(&self) -> Result<Vec<Category>, DbError>;        // system category removed by caller
}

/// Row written for a network entry not yet in the DB (OBSERVED toDomainAnime + Anime.create defaults).
/// favorite = false, date_added = 0, and every field not listed here takes its Anime.create() default
/// (p2-data-and-config.md section 1.4).
pub struct NewAnime {
    pub source: SourceId,
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,         // SAnime.getGenres()
    pub status: i64,                        // SAnime.status
    pub thumbnail_url: Option<String>,
    pub background_url: Option<String>,
    pub update_strategy: i64,               // AnimeUpdateStrategy ordinal, 0 = ALWAYS_UPDATE
    pub fetch_type: i64,                    // FetchType ordinal, 1 = Episodes
    pub season_number: f64,
    pub initialized: bool,
    pub memo: serde_json::Map<String, serde_json::Value>,
}
```

`NetworkToLocal` (OBSERVED `NetworkToLocalAnime`, `p2-behavior.md` 5.5):

```rust
pub struct NetworkToLocal { pub repo: Arc<dyn AnimeRepo> }
impl NetworkToLocal {
    /// Upserts one page of source entries and returns their row ids in input order.
    pub async fn upsert_all(&self, source: SourceId, animes: &[SAnime]) -> Vec<AnimeId>;
    /// Returns the stored row for a source entry, inserting it when no row exists (url + source key).
    pub async fn upsert_one(&self, source: SourceId, anime: &SAnime) -> Result<AnimeRow, DbError>;
}
```

`upsert_one` steps (OBSERVED): (1) look up by url and source; (2) if none, insert and return the new row; (3) if found and `favorite == false`, return the stored row with the source's title applied in memory only (Q-1); (4) if found and favorite, return the stored row unchanged.

`CoverCache`, `BackgroundCache`, `TrackBinder` and `EpisodeFlagDefaults` are thin traits over areas 34, 27 and 24. `library_add::change_favorite` takes them as `&dyn` parameters, with the signatures:

```rust
pub trait CoverCache: Send + Sync { fn delete_for(&self, anime: &AnimeRow) -> usize; }        // OBSERVED deleteFromCache(anime, true)
pub trait BackgroundCache: Send + Sync { fn delete_for(&self, anime: &AnimeRow) -> usize; }
pub trait TrackBinder: Send + Sync { fn bind_enhanced(&self, anime: &AnimeRow, source: SourceId); }
pub trait EpisodeFlagDefaults: Send + Sync { fn apply(&self, anime: &AnimeRow); }
```

`SourceRef` for the source in `change_favorite` is taken from `catalog.get_or_stub(anime.source)`.

`LocaleText` is a small trait over the translated strings (keys in `p2-data-and-config.md` section 7), with methods `text(key)`, `format(key, args)`, `language_display_name(code)`, `system_language_display_name()`. It is implemented once in `src/i18n.rs` (area:01/51).

## 4. Concurrency and threading

### 4.1 Threads

| Thread | Owns | Enters via |
|---|---|---|
| GTK main thread | all widgets, `GObject` list models, `BrowseModel`, `SourcesModel`, `GlobalSearchModel`, `VisibilityModel`, `MigrateSourcesModel` (all `Rc<RefCell<_>>` or plain structs owned by the page) | `glib::spawn_future_local`, GTK signal handlers |
| tokio runtime (multi-thread, 4 workers) | source calls (`AnimeSource::popular/latest/search`), network, pager tasks, global search fan-out | `tokio::spawn`, `Handle::spawn` |
| DB worker (one dedicated OS thread, owns `rusqlite::Connection`) | all SQL in section 7 of `p2-data-and-config.md` | `async_channel::Sender<DbRequest>` with `oneshot` replies |
| Preference watchers | settings changes | `tokio::sync::watch` (Receiver polled on the main thread through `glib::spawn_future_local`) |

Rule: no GTK object is touched off the main thread. Tokio tasks send results to the main thread through an `async_channel::Sender<UiMsg>`; the main thread drains the receiver in `glib::spawn_future_local`. Do not use `glib::MainContext::channel` (deprecated in glib 0.20+).

```rust
pub enum UiMsg {
    Page(PageResult),              // browse pager
    Favorite(FavoriteOutcome),     // favorite toggle done
    Search { generation: u64, source: SourceId, result: SearchResult }, // global search
    Visibility(VisibilitySnapshot),
    Migrate(MigrateSnapshot),
    SourcesSnapshot(Vec<SourceRow>),
    SourcesFailed,
}
```

### 4.2 Pager (`browse/anime/pager.rs`)

Replaces `androidx.paging.Pager` + `AnimeSourcePagingSource` (OBSERVED in `p2-behavior.md` 5.4).

```rust
pub struct SourcePager {
    source: Arc<dyn AnimeSource>,
    kind: PagerKind,                 // Popular | Latest | Search { query: String, filters: FilterList }
    next_page: Option<u32>,          // None = end of list (OBSERVED nextKey = null)
    state: PageState,
    generation: u64,                 // increments on refresh; stale page results are dropped (D-5 pattern)
    cancel: tokio_util::sync::CancellationToken,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageState {
    Idle,
    Loading { kind: LoadKind },
    Done,
    Failed { kind: LoadKind, error: SourceError },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind { Refresh, Append }

pub struct PageResult {
    pub generation: u64,
    pub kind: LoadKind,
    pub page: u32,
    pub outcome: Result<AnimesPageOut, SourceError>,
}

pub struct AnimesPageOut { pub animes: Vec<SAnime>, pub has_next_page: bool }

impl SourcePager {
    pub fn new(source: Arc<dyn AnimeSource>, kind: PagerKind) -> SourcePager;
    /// Begins a refresh. Returns the new generation.
    pub fn refresh(&mut self, tx: async_channel::Sender<UiMsg>) -> u64;
    /// Appends the next page when the state is Idle and next_page is Some.
    pub fn append(&mut self, tx: async_channel::Sender<UiMsg>);
    /// Applies a PageResult on the main thread. Returns the new animes to append, or None if dropped.
    pub fn apply(&mut self, r: PageResult) -> Option<Vec<SAnime>>;
}

pub enum PagerKind {
    Popular,
    Latest,
    Search { query: String, filters: FilterList },
}
```

`refresh` steps: (1) `generation += 1`; (2) `cancel.cancel()` and replace the token; (3) `next_page = Some(1)`; (4) `state = Loading { kind: Refresh }`; (5) spawn a task that calls `popular(1)`, `latest(1)`, or `search(1, query, filters)` according to `PagerKind`, races the call against the cancel token, and sends `UiMsg::Page(PageResult { generation, kind: Refresh, page: 1, outcome })`.

`append` steps: (1) return unless `state == Idle` and `next_page` is `Some(p)`; (2) `state = Loading { kind: Append }`; (3) spawn the same request for page `p` with `kind: Append`.

`apply` steps: (1) return `None` when `r.generation != self.generation` (stale); (2) on `Ok` with an empty `animes` list, set the error to `SourceError::NoEpisodes` (rules below); (3) on `Ok` with animes, set `next_page = if has_next_page { Some(page + 1) } else { None }`, set `state = Done` when `next_page` is `None` and `Idle` otherwise, and return `Some(animes)`; (4) on `Err(e)`, set `state = Failed { kind, error: e }` and return `None`.

Terminal rules (OBSERVED `AnimeSourcePagingSource.load`):
- `animes.is_empty()` → the page is an error `SourceError::NoEpisodes` (`NoEpisodesException`). For the first page this is the empty state (D-2). For later pages it is an append error (snackbar + Retry).
- `has_next_page == true` → `next_page = Some(page + 1)`; else `None`.
- Source error → `PageState::Failed`; the message is `SourceError::user_message()` (9.2 of `p2-behavior.md`).

### 4.3 Global search fan-out (`browse/anime/global_search.rs`)

```rust
pub struct GlobalSearchModel {
    generation: u64,                       // D-5 guard, incremented on every accepted search
    last_query: Option<String>,            // OBSERVED lastQuery (exact string compare)
    last_filter: Option<SourceFilterMode>, // OBSERVED lastSourceFilter
    query: Option<String>,                 // State.searchQuery (text field, updated per keystroke)
    mode: SourceFilterMode,                // default PinnedOnly
    only_with_results: bool,               // mirrors __APP_STATE_has_filters_toggle_state
    extension_filter: Option<String>,      // pkg name
    enabled_langs: HashSet<String>,        // snapshot at construction (OBSERVED)
    disabled: HashSet<SourceId>,           // snapshot
    pinned: HashSet<SourceId>,             // snapshot
    items: Vec<(SourceRef, SearchResult)>, // kept sorted by search_sort_key
    cancel: tokio_util::sync::CancellationToken,
}

static GLOBAL_SEARCH_PERMITS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
fn permits() -> Arc<tokio::sync::Semaphore> {
    GLOBAL_SEARCH_PERMITS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(5))).clone()
}

impl GlobalSearchModel {
    /// OBSERVED AnimeSearchScreenModel.search()
    pub fn search(&mut self, sources: &dyn SourceCatalog, net: Arc<NetworkToLocal>, tx: async_channel::Sender<UiMsg>) {
        let Some(q) = self.query.clone().filter(|q| !q.trim().is_empty()) else { return }; // blank -> no-op (OBSERVED isNullOrBlank)
        let same_query = self.last_query.as_deref() == Some(q.as_str());
        if same_query && self.last_filter == Some(self.mode) { return; }
        self.last_query = Some(q.clone());
        self.last_filter = Some(self.mode);
        self.generation += 1;
        let gen = self.generation;
        self.cancel.cancel();                       // PROPOSAL: Kotlin never cancels (E-GS-03)
        self.cancel = tokio_util::sync::CancellationToken::new();
        let selected = self.selected_sources(sources);
        // reuse results for the same query (OBSERVED sameQuery branch)
        let old = std::mem::take(&mut self.items);
        self.items = selected.iter().map(|s| {
            let prev = old.iter().find(|(o, _)| o.id == s.id).map(|(_, r)| r.clone());
            (s.clone(), if same_query { prev.unwrap_or(SearchResult::Loading) } else { SearchResult::Loading })
        }).collect();
        self.sort_items();
        for s in selected {
            if matches!(self.result_of(s.id), Some(SearchResult::Loading)) {
                let (cancel, q, tx, permits, net) = (self.cancel.clone(), q.clone(), tx.clone(), permits(), net.clone());
                let Some(src) = sources.get(s.id) else { continue };   // Arc<dyn AnimeSource>
                tokio::spawn(async move {
                    let _p = permits.acquire_owned().await.ok();     // OBSERVED pool of 5
                    let res = tokio::select! {
                        _ = cancel.cancelled() => return,
                        r = src.search(1, &q, &src.filter_list()) => r,   // page 1 only (OBSERVED)
                    };
                    let msg = match res {
                        Ok(page) => {
                            let ids = net.upsert_all(s.id, &page.animes).await;
                            UiMsg::Search { generation: gen, source: s.id, result: SearchResult::Ok { anime: ids } }
                        }
                        Err(e) => UiMsg::Search { generation: gen, source: s.id,
                                                 result: SearchResult::Failed { message: e.raw_message() } },
                    };
                    let _ = tx.send(msg).await;
                });
            }
        }
    }

    /// Main thread only. Drops results whose generation is not current (D-5).
    pub fn on_result(&mut self, gen: u64, source: SourceId, result: SearchResult) {
        if gen != self.generation { return; }
        if let Some(slot) = self.items.iter_mut().find(|(s, _)| s.id == source) { slot.1 = result; }
        self.sort_items();
    }
    /// Implements 4.3.1 (OBSERVED getEnabledSources + getSelectedSources).
    fn selected_sources(&self, catalog: &dyn SourceCatalog) -> Vec<SourceRef> {
        let mut picked: Vec<SourceRef> = catalog
            .installed()
            .into_iter()
            .filter(|s| self.enabled_langs.contains(&s.lang) && !self.disabled.contains(&s.id))
            .filter(|s| self.mode != SourceFilterMode::PinnedOnly || self.pinned.contains(&s.id))
            .collect();
        if let Some(pkg) = self.extension_filter.as_deref().filter(|p| !p.is_empty()) {
            let allowed: HashSet<SourceId> = catalog.installed_extension_sources(pkg).into_iter().collect();
            picked.retain(|s| allowed.contains(&s.id));
        }
        picked.sort_by_key(|s| search_sort_key(s, None, self.pinned.contains(&s.id)));
        picked
    }
    fn result_of(&self, id: SourceId) -> Option<&SearchResult> {
        self.items.iter().find(|(s, _)| s.id == id).map(|(_, r)| r)
    }
    /// Keeps items ordered by search_sort_key with the current results.
    fn sort_items(&mut self) {
        let pinned = self.pinned.clone();
        self.items.sort_by_cached_key(|(s, r)| search_sort_key(s, Some(r), pinned.contains(&s.id)));
    }
}
```

4.3.1 Source selection (OBSERVED `getEnabledSources` + `getSelectedSources`):
1. `candidates = catalog.installed()` (installed sources including local; stubs excluded).
2. Keep when `lang in enabled_langs && !disabled.contains(id)`.
3. If `mode == PinnedOnly` keep only `pinned.contains(id)` (subclass override in Kotlin; here one function).
4. If `extension_filter` is non-empty: keep only sources of the installed extension with that pkg name, and only those that passed steps 2-3.
5. Sort by `search_sort_key` (pinned first, then name).

Important: the `"all"`/`"other"` language check uses the snapshot taken when the model is created (E-GS-07 documents the staleness).

Fan-out concurrency: at most 5 `search` calls run at once (shared semaphore). Pending tasks wait on `acquire_owned`. The cancel token stops a superseded search before its request starts (the `select!` in step 5 returns before the permit is held for long).

### 4.4 Browse model (`browse/anime/source_browse.rs`)

```rust
pub struct BrowseModel {
    pub source_id: SourceId,
    pub source: Arc<dyn AnimeSource>,      // from catalog.get_or_stub(id)
    pub listing: Listing,
    pub filters: FilterList,               // State.filters (user-edited in place)
    pub toolbar_query: Option<String>,
    pub dialog: Option<BrowseDialog>,
    pub display_mode: LibraryDisplayMode,  // global pref (OBSERVED)
    pub hide_library_items: bool,          // snapshot (OBSERVED)
    pub pager: SourcePager,
    pub items: gio::ListStore,             // AnimeItem GObjects (see 4.5)
    pub incognito: bool,
}

pub enum BrowseDialog {
    Filter,
    RemoveAnime { anime: AnimeRow },
    AddDuplicateAnime { anime: AnimeRow, duplicate: AnimeRow },
    ChangeAnimeCategory { anime: AnimeRow, choices: Vec<CategoryChoice> },
    Migrate { new_anime: AnimeRow, old_anime: AnimeRow },
}

pub struct CategoryChoice { pub category: Category, pub checked: bool }   // checked = preselected
```

Init (OBSERVED `BrowseAnimeSourceScreenModel` init, lines 83-103):
1. `source = catalog.get_or_stub(source_id)`.
2. `listing = Listing::from_nav_arg(nav_arg)`; if Search: `query = listing.query; listing = Search{query, filters: source.filter_list()}`; `filters = source.filter_list()`; `toolbar_query = query`.
3. If `!incognito_for(source_id)` → `prefs.set_last_used_source(source_id)`.
4. Start the pager with `refresh()`.

### 4.5 Items (GObject)

```rust
// browse/anime/source_browse.rs (or widgets.rs, part 1 owns the card visuals)
mod imp {
    use super::*;
    #[derive(glib::Properties, Default)]
    #[properties(wrapper_type = super::AnimeItem)]
    pub struct AnimeItemImp {
        #[property(get, set)] pub anime_id: std::cell::Cell<i64>,
        #[property(get, set)] pub title: std::cell::RefCell<String>,
        #[property(get, set)] pub favorite: std::cell::Cell<bool>,
        #[property(get, set)] pub cover_last_modified: std::cell::Cell<i64>,
        #[property(get, set)] pub thumbnail_url: std::cell::RefCell<Option<String>>,
    }
}
glib::wrapper! { pub struct AnimeItem(ObjectSubclass<imp::AnimeItemImp>); }
```

Each pager page appends new items to the `ListStore` (append only, no re-sort). Covers are loaded through area:34 when the row binds; rows that are not bound never subscribe (fixes E-BR-09).

### 4.6 Sources model (`browse/anime/sources.rs`)

```rust
pub struct SourcesModel {
    pub rows: Vec<SourceRow>,
    pub loading: bool,               // OBSERVED isLoading; stays true if the first emission fails (E-SRC-02 fixed: see 4.6.1)
    pub dialog: Option<SourceRow>,   // OBSERVED Dialog(source)
}
```

4.6.1 Failure path. OBSERVED: upstream exception → one-shot event; `loading` never clears when the first emission fails. PROPOSAL: on the first failure set `loading = false` and show an inline error row with Retry (new behavior, documented as E-SRC-02).

Subscription: the model subscribes to `(pinned_sources, enabled_languages, disabled_sources, last_used_source, catalog.installed())` with `tokio::sync::watch` combined in a task, and posts `UiMsg::SourcesSnapshot`. On the main thread, `group_sources` produces headers and rows.

### 4.7 Visibility model (`browse/anime/source_filter.rs`)

Subscribes to `(online_sources, enabled_langs, disabled_sources)`. Produces `VisibilitySnapshot { groups: Vec<(String /*lang*/, Vec<SourceRef>)>, enabled_langs, disabled }` ordered per `visibility_order` (enabled first; "all" first; then display name). Sorting per `p2-behavior.md` 4.1. Local source and stubs excluded.

### 4.8 Library add (`browse/anime/library_add.rs`)

```rust
pub async fn change_favorite(repo: &dyn AnimeRepo, anime: &AnimeRow, now_ms: i64, covers: &dyn CoverCache, backgrounds: &dyn BackgroundCache, tracks: &dyn TrackBinder) -> Result<(), DbError>;
pub fn resolve_add_path(categories: &[Category], default_category_id: i64) -> AddPath;
pub enum AddPath {
    /// (a) default category exists: set categories to [default], then favorite
    ToCategory(i64),
    /// (b) default is "Uncategorized" (0) or no user categories: favorite with no category
    NoCategory,
    /// (c) show picker with preselection
    Pick,
}
```

`change_favorite` (D-3 narrow update):
1. `new_fav = !anime.favorite`; `date_added = if anime.favorite { 0 } else { now_ms }`.
2. If `!new_fav`: `covers.delete_for(anime, keep_dir=true) > 0` → `cover_last_modified = now_ms`; same for backgrounds → `background_last_modified = now_ms` (local source: skip, OBSERVED `isLocal()`).
3. Else: `set_default_episode_flags(anime)` then `tracks.bind_enhanced(anime)` (areas 24/27).
4. `repo.update_favorite(anime.id, FavoritePatch { favorite, date_added, cover_last_modified, background_last_modified })` (narrow SQL `UPDATE animes SET favorite=?, date_added=?, cover_last_modified=COALESCE(?, cover_last_modified), background_last_modified=COALESCE(?, background_last_modified) WHERE _id=?`).

The Kotlin path writes all columns (E-BR-02). D-3 replaces it.

### 4.9 Migration sources (`browse/anime/migration/sources.rs`)

```rust
pub struct MigrateSourcesModel {
    pub rows: Vec<MigrateRow>,
    pub loading: bool,
    pub mode: MigrateSortMode,         // default Alphabetical (pref)
    pub direction: SortDirection,      // default Ascending (pref)
}
```
Source: `repo.favorite_counts()` (SQL in `p2-data-and-config.md` 6.2) joined with catalog stub/installed info. Toggles write both prefs (OBSERVED `SetMigrateSorting.await(mode, direction)` writes both keys).

---

## 5. Crates

| Crate | Version (proposal) | Used for | Why this and not something else |
|---|---|---|---|
| `gtk4` (existing) | 0.11.5, feature `v4_16` | all UI | Existing dependency. `v4_16` already enabled. |
| `glib` (transitive) | as pulled by gtk4 | `glib::spawn_future_local`, `glib::Properties`, `glib::wrapper!` | Standard GTK main-loop integration |
| `tokio` | `1`, features `rt-multi-thread`, `sync`, `time`, `macros` | source calls, semaphore, watch channels | Standard async runtime; required by reqwest anyway |
| `tokio-util` | `0.7`, feature `sync` | `CancellationToken` | Standard cancellation primitive |
| `async-trait` | `0.1` | `dyn AnimeSource` with async methods | Native `async fn` in traits is not object-safe for `dyn` without boxing; `async-trait` boxes it. Alternative: `futures::future::BoxFuture` by hand (more code). |
| `async-channel` | `2` | tokio to GTK main-thread messages and DB requests | Works from both threads and from `glib::spawn_future_local` (`glib::MainContext::channel` is deprecated) |
| `rusqlite` | `0.32` or newer, feature `bundled` | DB worker | Synchronous API suits a dedicated DB thread. `bundled` avoids system SQLite version drift. |
| `reqwest` | (area:12) | source HTTP (not used directly in scope) | Owned by area:12 |
| `serde`, `serde_json` | existing | settings JSON, `memo` | Existing |
| `thiserror` | `2` | `SourceError`, `DbError` | Derive `Display` for messages |
| `icu_collator` (optional) | `2` | locale-aware alphabetical sort in migration | Replaces `java.text.Collator`. If not added, fall back to Unicode-lowercase byte order and document it (E-MS-05). |
| `sys-locale` (not added) | - | system language | Use `glib::language_names()` instead (no new crate). |

No new crate for: clipboard (`gdk::Clipboard`), URL opening (`gtk::UriLauncher`), snackbar (custom `gtk::Revealer`), dialogs (`gtk::AlertDialog`, GTK 4.10+), windows (`gtk::Window`).

---

## 6. Dialogs and windows (no libadwaita)

| Kotlin | Rust | Widget |
|---|---|---|
| `RemoveEntryDialog` (AlertDialog) | `gtk::AlertDialog` with buttons "Cancel", "Remove" | `AlertDialog::builder().modal(true).message("Are you sure?").detail("You are about to remove \"TITLE\" from your library").buttons(["Cancel","Remove"]).cancel_button(0).default_button(1)` |
| `DuplicateAnimeDialog` (AdaptiveSheet) | modal `gtk::Window` 420 px wide, vertical `gtk::Box` with three `gtk::Button` rows and a Cancel button | |
| `ChangeCategoryDialog` (checkbox list) | modal `gtk::Window` with `gtk::CheckButton` rows, "Edit categories" button, OK, Cancel | |
| `SourceFilterAnimeDialog` (AdaptiveSheet) | modal `gtk::Window` 480 x 640 with header bar row (Reset, Filter) and `gtk::ScrolledWindow` | |
| `AnimeSourceOptionsDialog` (AlertDialog) | `gtk::AlertDialog` with buttons "Pin"/"Unpin", "Disable", "Cancel" (no Disable for id 0) | |
| Migrate dialog (part 1) | part 1 | |

Rules:
- All dialogs are modal to the main window, transient for it, and close on Escape (GTK default for `gtk::Window` with `set_modal(true)`; `AlertDialog` handles Escape as Cancel).
- Each dialog returns a value through a callback closure captured with `glib::clone!` and a `Weak` reference to the page (avoid cycles).

### 6.1 Snackbar (no libadwaita)

Shared with area:36. A `gtk::Overlay` child `gtk::Revealer` (`transition_type = SlideUp`) containing a `gtk::Box` with a `gtk::Label` and an optional `gtk::Button` (action label). Default timeout 4000 ms (Compose `SnackbarDuration.Short` equivalent). Indefinite mode: no timer, dismiss only on click or action.

---

## 7. Android-API → Linux mapping (for this scope)

| Android / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| Voyager `Screen`, `Navigator`, `TabContent`, `Tab` | GTK page stack: `ui::push(&page)`, `ui::pop()` (mockup `src/ui.rs:34, 40`) | Back gesture → Escape or the header back button |
| Voyager `StateScreenModel` + `screenModelScope` | Plain struct owned by the page on the main thread + `glib::spawn_future_local` | No lifecycle framework; dispose = drop the page; cancel tokens on drop |
| `ioCoroutineScope` (`Dispatchers.IO + SupervisorJob`) | `tokio::spawn` on the shared runtime; `CancellationToken` per model | |
| `StateFlow` / `MutableStateFlow` | Model field + `UiMsg` redraw | Redraw is explicit: each `UiMsg` handler updates the widgets it owns |
| `Flow.combine` | `tokio::sync::watch` channels combined in a small task (or `futures::stream::select`) | |
| `Flow.distinctUntilChanged` | Compare the previous `Listing` value (D-7) before restarting the pager | |
| AndroidX Paging 3 `Pager`, `PagingConfig(pageSize=25)`, `LazyPagingItems`, `cachedIn` | `SourcePager` (4.2) with prefetch when the last visible row index is `>= len - 25` | Keep `pageSize = 25` as the prefetch distance (OBSERVED) |
| Injekt (`Injekt.get()`) | Explicit `AppServices` struct created in `main.rs`, passed by `Rc`/`Arc` | |
| `Executors.newFixedThreadPool(5)` | Shared `tokio::sync::Semaphore::new(5)` (4.3) | D-6 |
| `LaunchedEffect` / `collectLatest` | Subscription task + `glib::spawn_future_local` drain loop | Cancel on page drop |
| `androidx.compose.material3.SnackbarHost` | Overlay revealer (6.1) | |
| `AlertDialog`, `AdaptiveSheet` | `gtk::AlertDialog`, modal `gtk::Window` (6) | |
| `LazyColumn` with sticky headers | `gtk::ListBox` with `set_header_func` per section header row, or `gtk::ListView` with a header item type | Use `ListBox` for the Sources list; sticky header via `gtk::ScrolledWindow` + `gtk::ListBox` header rows |
| `LazyVerticalGrid(GridCells.Adaptive(128.dp))` | `gtk::GridView` with child `width_request(128)`, `min_columns = 1`, `max_columns = unbounded` | |
| `GridCells.Fixed(n)` | `gtk::GridView` with `min_columns = max_columns = n` | |
| `LazyColumn` (list mode) | `gtk::ListView` | |
| `LazyRow` (card row, global search) | `gtk::ScrolledWindow` (horizontal policy `Automatic`, vertical `Never`) with `gtk::Box` (horizontal, spacing 4) | |
| `FilterChip` | `gtk::ToggleButton` in a `gtk::Box` (horizontal) with `set_group` for radio behavior | Popular/Latest/Pinned/All are radio, "Filter" and "Has results" are toggles |
| `CollapsibleBox` | `gtk::Expander` | |
| `SearchToolbar` | `gtk::SearchBar` + `gtk::SearchEntry` (already used in mockup `src/entry.rs:434`) | `activate` signal = IME submit |
| `LinearProgressIndicator` | `gtk::ProgressBar` (`fraction = progress/total`) | Shown only when `progress in 1..<total` |
| `HapticFeedback` | none | Dropped (12 in `p2-behavior.md`) |
| `Toast` | Snackbar (6.1) | |
| `ClipboardManager.setPrimaryClip` (`copyToClipboard`) | `gdk::Display::default().clipboard().set_text(text)` | Part 1 call site |
| `UriHandler.openUri` | `gtk::UriLauncher::new(uri).launch(Some(&window), None::<&gio::Cancellable>, |_| {})` | Opens the default handler through the desktop portal |
| `WebViewScreen` | area:36 (WebKitGTK 6 via the `webkit6` crate, owned there) | Call `webview::open(url, title, source_id)` |
| `Locale.getDefault()` / `LocaleListCompat` | `glib::language_names()` (first entry), `std::env::var("LANG")` fallback | Used for default enabled languages and "" language display |
| `Locale.forLanguageTag(tag).displayName` | `icu_locid`/`icu_displaynames` if added; else a small static table for the codes Aniyomi ships (PROPOSAL: start with `icu` for the 40 most common codes, fall back to the raw code) | Display names are presentation-only |
| `java.text.Collator` (`compareToWithCollator`) | `icu_collator` (optional) | E-MS-05 |
| `AssistContent` | none | Dropped |
| `Intent` / deep links (`MainActivity` INTENT_ANIMESEARCH) | CLI args or `x-scheme-handler` (see 8) | Part 1 / area:36 |

---

## 8. Entry points on Linux (PROPOSAL)

- Global search with a query and an extension filter (Android INTENT_ANIMESEARCH): accept `kurobayomi://animesearch?query=<q>&filter=<pkg>` through a `.desktop` `MimeType=x-scheme-handler/kurobayomi` entry and `gio::Application::open`. Parsing and the auto-open rule live in `global_search.rs` (OBSERVED 7.13).
- Launch with `--anime-search <query>` for scripting. Optional; P2.

---

## 9. Error handling

### 9.1 Typed errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("no results")]
    NoEpisodes,                       // OBSERVED NoEpisodesException
    #[error("source not found")]
    NotInstalled,                     // OBSERVED AnimeSourceNotInstalledException
    #[error("HTTP {0}")]
    Http(u16),                        // OBSERVED HttpException
    #[error("unknown host {0}")]
    UnknownHost(String),              // OBSERVED UnknownHostException
    #[error("offline")]
    Offline,                          // derived: UnknownHost while offline
    #[error("{0}")]
    Other(String),                    // any other error; message = source message or class name
}

impl SourceError {
    /// OBSERVED ExceptionFormatter.formattedMessage (p2-behavior.md 9.2)
    pub fn user_message(&self, t: &dyn LocaleText) -> String {
        match self {
            SourceError::NoEpisodes => t.text("no_results_found"),
            SourceError::NotInstalled => t.text("loader_not_implemented_error"),
            SourceError::Http(code) => t.format("exception_http", &[&code.to_string()]),
            SourceError::UnknownHost(h) => t.format("exception_unknown_host", &[h]),
            SourceError::Other(m) => m.clone(),
        }
    }
    /// OBSERVED GlobalSearchErrorResultItem uses raw throwable.message (may be None)
    pub fn raw_message(&self) -> Option<String> {
        match self { SourceError::Other(m) => Some(m.clone()), other => Some(other.to_string()).filter(|s| !s.is_empty()) }
    }
}
```

### 9.2 Panics

No `unwrap()` on user data. Settings parse failures fall back to defaults. `TriState` unknown ints map to `Ignore` (D-4).

### 9.3 DB errors

`DbError` is a `thiserror` enum. Repository methods return `Result`. Failed favorite writes show a snackbar with the text "InternalError: Check crash logs for further information" (the Kotlin path is silent; D-3 and E-BR-12 propose a visible error instead).

---

## 10. Testing hooks

- Every function in `model.rs` is pure and tested in `#[cfg(test)]` modules next to it (no display needed).
- `SourcePager` is tested with a fake `AnimeSource` (`p2-edge-cases-and-tests.md`, fixtures section).
- `GlobalSearchModel::on_result` and generation guard tested without GTK.
- GTK views are not unit-tested; they are checked by the manual steps in `p2-ui-parity.md`.

---

## 11. Pitfalls

1. Holding a `RefCell` borrow across an `.await`. Clone the data out, drop the borrow, then await.
2. Creating `GObject` wrappers off the main thread. The `AnimeItem` type is main-thread only; send plain structs through `UiMsg`.
3. `GridView` with a `ListStore` that is rebuilt each page. Append only. Rebuilding resets scroll position and selection (Kotlin keeps position).
4. `gtk::ListBox` header functions run during measure. Keep them cheap; do not query the DB inside.
5. Dropping a `gtk::Window` while an `AlertDialog` callback still holds a strong `Rc` to the page: capture `Weak`.
6. The Kotlin `AnimeFilterList` identity bug must not be copied. Equality is by value (D-7).
7. The Kotlin `Sort` null-state NPE and the TriState throw must not be copied (D-4).
8. `async_channel::Sender::send` from tokio must not block; use `try_send` where the channel is bounded, or make it unbounded (PROPOSAL: unbounded, because message volume is one per source per search).
9. `glib::language_names()` returns the environment languages in preference order. Use the first element for `getDefaultEnabledLanguages` (Kotlin `Locale.getDefault().language` is the first).
10. Blank query in global search is a no-op (no clearing). Do not clear results on blank input (OBSERVED).

---

## 12. Open questions

| ID | Question | Recommendation |
|---|---|---|
| Q-1 | Should browse refresh the displayed title of non-favorite rows when the source's title changes (Kotlin keeps the DB title)? | Keep Kotlin behavior for now. Revisit with area:05 (title freshness). |
| Q-2 | Should the browse "Filter" chip show only when filters are non-default? (Kotlin shows it whenever the source defines any filter.) | Keep Kotlin behavior. |
| Q-3 | Does the migration tab need ICU collation or is Unicode-lowercase byte order enough? | Add `icu_collator` only if non-Latin names sort badly in testing. |
| Q-4 | Global search: keep `PinnedOnly` as the default mode? | Keep (OBSERVED default). |
| Q-5 | Filter sheet: apply edits to the shared list immediately (Kotlin) or only on Filter? | Keep Kotlin behavior (immediate). |
