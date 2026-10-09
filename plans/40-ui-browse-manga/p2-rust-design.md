# Area 40 (part 2): Rust design for the manga browse models

Target: `gtk4` crate 0.11 (GTK 4.16+, no libadwaita), tokio, rusqlite, reqwest, serde, binary crate `kurobayomi`. Mockup in `/home/blackbox/kurobayomi/src/` (`entry.rs` builds the browse pages from static rows: `sources` at line 283, `migrate_sources` at 360, `browse_source` at 380, `global_search` at 432). This part gives the model layer that replaces the static rows; the GTK pages are part 1 (`p1-rust-design.md` sections 2, 7).

Reads with: `p2-behavior.md` (algorithms, cited by section below as "behavior N"), `p2-data-and-config.md` (keys, models, SQL), `p1-rust-design.md` (page modules, `Pins`, `display_name`, threading pattern, crates).

---

## 1. Principles

1. Models are GTK-free and runtime-free. Every algorithm of this part is a pure function or a plain state struct in `browse/model/`. Unit tests run without a display or a tokio runtime (p2-edge-cases-and-tests.md).
2. One writer per model. The GTK main thread owns every model value (`Rc<RefCell<Model>>`). Background tasks never mutate a model; they send events. This removes the lost-update race of behavior 9.4.
3. Generations, not equality. Kotlin relies on `equals` (`FilterList.equals` is always false, data-class equality) for re-querying. Rust uses explicit `generation: u64` counters and never compares filter trees for identity.
4. Source-of-truth ordering is stored as `Vec`, sorted with a stable sort. No map keyed by a comparator (behavior 9.5, D-GS-01).
5. No Android types. Preferences, DB and source access go through traits (section 4) so the models can be tested with fakes.

---

## 2. Crate and module layout

Part 1 owns the GTK page files; this part adds the `browse/model/` tree and the glue (`browse/services.rs`).

```
src/browse/
  mod.rs                      (part 1) SourceId, MangaId, Pins, pub use
  display_name.rs             (part 1) LocaleHelper port, Labels trait, compare_languages
  source_list.rs              (part 1 GTK page)  <- uses model::source_list
  source_filter.rs            (part 1 GTK page)  <- uses model::source_filter
  source_browse.rs            (part 1 GTK page)  <- uses model::browse_source, model::paging, model::filter_tree
  global_search.rs            (part 1 GTK page)  <- uses model::global_search
  migration/source_picker.rs  (part 1 GTK page)  <- uses model::migrate_sources
  model/
    mod.rs                    re-exports
    source_row.rs             SourceRow, status flags, key()                        (pure)
    source_list.rs            Sources page grouping, toggle plans                   (pure)
    source_filter.rs          languages with sources, enable/disable plans          (pure)
    migrate_sources.rs        favorite-count sort, mode/direction                   (pure)
    collate.rs                primary-strength collator (ICU4X) + CASE_INSENSITIVE_ORDER port (pure)
    listing.rs                Listing, RemoteQuery                                  (pure)
    filter_tree.rs            FilterNode, TriState, SortSelection, genre search,    (pure)
                              sort click, tri-state cycle, raw mapping
    paging.rs                 PAGE_SIZE, PageState, next-key rules, NoResults       (pure)
    browse_source.rs          BrowseSourceState: listing/filters/toolbar/dialog,    (pure)
                              search/reset/listing transitions, navigate-up
    favorites.rs              FavoriteChange, AddFavoritePlan, LongClickAction      (pure)
    global_search.rs          GlobalSearchState: items, generation, sort, visible   (pure)
    migrate_search.rs         origin-first comparator (fixed; behavior 10)          (pure)
  services.rs                 traits (section 4) + tokio glue (section 5)           (GTK-free, tokio)
```

Rule: `model/*` imports only `std`, `icu_collator`, `thiserror`, and `super::{SourceId, MangaId, Pins}`. `services.rs` imports tokio and the traits. GTK pages import both.

---

## 3. Core types

### 3.1 Source rows and pins

```rust
// src/browse/model/source_row.rs
use crate::browse::{Pins, SourceId};

/// Mirrors tachiyomi.domain.source.manga.model.Source (domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:3-14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRow {
    pub id: SourceId,
    pub lang: String,
    pub name: String,
    pub supports_latest: bool,
    pub is_stub: bool,
    pub pins: Pins,
    pub is_used_last: bool,
}

impl SourceRow {
    /// Source.visualName: name if lang is empty, else "name (LANG)" with lang uppercased.
    pub fn visual_name(&self) -> String {
        if self.lang.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.lang.to_uppercase())
        }
    }
    /// Source.key: "<id>-lastused" for the last-used copy, else "<id>".
    pub fn key(&self) -> String {
        if self.is_used_last { format!("{}-lastused", self.id) } else { self.id.to_string() }
    }
    /// Kotlin: "${name.lowercase()} (${lang})" (behavior 9.5 item 3).
    pub fn name_key(&self) -> String {
        format!("{} ({})", self.name.to_lowercase(), self.lang)
    }
}
```

`Pins` is the part 1 type (`p1-rust-design.md` section 3): `Pins(pub u32)` with `UNPINNED=0`, `PINNED=1`, `ACTUAL=2`, `contains`, `is_pinned`. This part adds the two operations the Kotlin uses:

```rust
impl Pins {
    /// Pins.plus(pin): code | pin.
    pub fn with(self, bit: u32) -> Pins { Pins(self.0 | bit) }
    /// Pins.minus(pin) — Kotlin uses XOR (Pin.kt:23). Kept as XOR on purpose: the only
    /// call site (GetEnabledMangaSources.kt:45) subtracts Actual from `Pins::pinned()` or
    /// `Pins::UNPINNED`. Document, do not "fix" silently.
    pub fn xor(self, bit: u32) -> Pins { Pins(self.0 ^ bit) }
    pub fn pinned() -> Pins { Pins(Pins::PINNED | Pins::ACTUAL) }   // Pins.pinned = 0b11
}
```

### 3.2 Listing and remote query

```rust
// src/browse/model/listing.rs
use super::filter_tree::FilterTree;

#[derive(Clone, Debug, PartialEq)]
pub enum Listing {
    Popular,
    Latest,
    /// `query` None = browse-with-filters (Kotlin sends "" to the source).
    Search { query: Option<String>, filters: FilterTree },
}

#[derive(Clone, Debug, PartialEq)]
pub enum RemoteQuery {
    Popular,
    Latest,
    Search { text: String, filters: FilterTree },   // text == "" when query is None
}

impl Listing {
    /// Listing.valueOf(query) for the navigation argument (behavior 4.1). The navigation
    /// argument is an enum in Rust; no sentinel strings are used.
    pub fn from_nav(arg: NavListing, default_filters: FilterTree) -> Listing {
        match arg {
            NavListing::Popular => Listing::Popular,
            NavListing::Latest => Listing::Latest,
            NavListing::Search(q) => Listing::Search { query: q, filters: default_filters },
        }
    }
    pub fn query(&self) -> Option<&str> {
        match self { Listing::Search { query, .. } => query.as_deref(), _ => None }
    }
    /// BrowseMangaSourceScreenModel.State.isUserQuery (line 368).
    pub fn is_user_query(&self) -> bool {
        matches!(self, Listing::Search { query: Some(q), .. } if !q.is_empty())
    }
    pub fn to_remote(&self) -> RemoteQuery {
        match self {
            Listing::Popular => RemoteQuery::Popular,
            Listing::Latest => RemoteQuery::Latest,
            Listing::Search { query, filters } => RemoteQuery::Search {
                text: query.clone().unwrap_or_default(),
                filters: filters.clone(),
            },
        }
    }
}

/// What the Sources page / global search hands to the browse page (`listing.query` in Kotlin).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavListing { Popular, Latest, Search(Option<String>) }
```

Note: `Listing` has no `PartialEq` shortcut for "same search". The generation counter (section 6.2) decides restarts.

### 3.3 Filter tree

```rust
// src/browse/model/filter_tree.rs

/// Filter.TriState raw states (source-api Filter.kt:17-21).
pub const STATE_IGNORE: i32 = 0;
pub const STATE_INCLUDE: i32 = 1;
pub const STATE_EXCLUDE: i32 = 2;

/// UI tri-state (core TriState.kt).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState { Disabled, EnabledIs, EnabledNot }

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown TriState state {0}")]
pub struct InvalidTriState(pub i32);

impl TriState {
    /// TriState.next(): Disabled -> EnabledIs -> EnabledNot -> Disabled.
    pub fn next(self) -> TriState {
        match self { Self::Disabled => Self::EnabledIs, Self::EnabledIs => Self::EnabledNot, Self::EnabledNot => Self::Disabled }
    }
    /// Kotlin Int.toTriStateFilter. Behavior 6.2: unknown raw -> Err (Kotlin throws;
    /// Kurobayomi shows Disabled and logs, decision D-BR-02).
    pub fn from_raw(raw: i32) -> Result<TriState, InvalidTriState> {
        match raw { STATE_IGNORE => Ok(Self::Disabled), STATE_INCLUDE => Ok(Self::EnabledIs),
                    STATE_EXCLUDE => Ok(Self::EnabledNot), other => Err(InvalidTriState(other)) }
    }
    pub fn to_raw(self) -> i32 {
        match self { Self::Disabled => STATE_IGNORE, Self::EnabledIs => STATE_INCLUDE, Self::EnabledNot => STATE_EXCLUDE }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectValue { pub text: String, pub is_string: bool }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortSelection { pub index: usize, pub ascending: bool }

#[derive(Clone, Debug, PartialEq)]
pub enum FilterNode {
    Header { name: String },
    Separator,
    CheckBox { name: String, state: bool },
    /// Raw state kept as i32 so an out-of-range value from a source is representable.
    TriState { name: String, raw: i32 },
    Text { name: String, state: String },
    /// `state` indexes into `values` (full list). Behavior 4.7 / D-BR-03.
    Select { name: String, values: Vec<SelectValue>, state: usize },
    Sort { name: String, values: Vec<String>, state: Option<SortSelection> },
    Group { name: String, children: Vec<FilterNode> },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilterTree(pub Vec<FilterNode>);

impl FilterTree {
    /// FilterList.isNotEmpty() (count includes headers and separators; behavior 6.4).
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
}

/// FilterItem Sort click (SourceFilterMangaDialog.kt:136-147), with D-BR-01 for None.
pub fn sort_click(state: Option<SortSelection>, clicked: usize) -> SortSelection {
    match state {
        // same index: flip direction; other index: keep the current direction
        Some(s) if s.index == clicked => SortSelection { index: clicked, ascending: !s.ascending },
        Some(s) => SortSelection { index: clicked, ascending: s.ascending },
        // D-BR-01: Kotlin throws NPE here. Kurobayomi selects ascending.
        None => SortSelection { index: clicked, ascending: true },
    }
}

/// Sort arrow shown for an item (SortItem sortDescending arg, SourceFilterMangaDialog.kt:132-135).
/// Some(true) = arrow down (descending), Some(false) = arrow up, None = no arrow.
pub fn sort_arrow(state: Option<SortSelection>, item: usize) -> Option<bool> {
    match state {
        Some(s) if s.index == item => Some(!s.ascending),
        _ => None,
    }
}

/// Select dropdown value (SelectItem). Out-of-range index is clamped to 0 (Kotlin throws).
pub fn select_text(values: &[SelectValue], state: usize) -> Option<&str> {
    values.get(state).or_else(|| values.first()).map(|v| v.text.as_str())
}

/// searchGenre algorithm (behavior 4.7). Returns true when a match was found.
/// `ignore_case` compares Unicode-lowercased strings (Kotlin equals(ignoreCase = true)).
pub fn apply_genre(tree: &mut FilterTree, genre: &str) -> bool {
    let target = genre.to_lowercase();
    for node in tree.0.iter_mut() {
        match node {
            FilterNode::Group { children, .. } => {
                for child in children.iter_mut() {
                    let name = match child {
                        FilterNode::CheckBox { name, .. } | FilterNode::TriState { name, .. }
                        | FilterNode::Text { name, .. } | FilterNode::Select { name, .. } => name.clone(),
                        _ => continue,
                    };
                    if name.to_lowercase() == target {
                        match child {
                            FilterNode::TriState { raw, .. } => *raw = STATE_INCLUDE,
                            FilterNode::CheckBox { state, .. } => *state = true,
                            _ => {}           // other types: match counts, state untouched
                        }
                        return true;          // labelled break@filter exits the outer loop too
                    }
                }
            }
            FilterNode::Select { values, state, .. } => {
                // Kotlin indexes the string-filtered list (quirk). D-BR-03: index into `values`.
                let hit = values.iter().position(|v| v.is_string && v.text.to_lowercase() == target);
                if let Some(idx) = hit {
                    *state = idx;
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}
```

Note on `apply_genre` and D-BR-03: the Kotlin code sets `state` to the position in `values.filterIsInstance<String>()`. Rust sets the position in `values`. For arrays with only strings both are equal; this is the intended fix.

### 3.4 Paging

```rust
// src/browse/model/paging.rs

/// PagingConfig(pageSize = 25) (BrowseMangaSourceScreenModel.kt:112).
pub const PAGE_SIZE: usize = 25;
/// androidx.paging default initialLoadSize = 3 * pageSize (not overridden in Kotlin).
pub const INITIAL_LOAD: usize = 3 * PAGE_SIZE;

#[derive(Clone, Debug, PartialEq)]
pub struct RemoteManga { pub url: String, pub title: String, pub thumbnail_url: Option<String> /* + fields from SManga, see p2-data-and-config 1.5 */ }

#[derive(Clone, Debug, PartialEq)]
pub struct MangasPage { pub mangas: Vec<RemoteManga>, pub has_next_page: bool }

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PageError {
    /// Empty page: SourcePagingSource.load throws NoChaptersException (behavior 4.3).
    #[error("no results found")]
    NoResults,
    #[error("source error: {0}")]
    Source(String),
}

/// Result of loading page `page` (1-based) — mirrors SourcePagingSource.load (MangaSourcePagingSource.kt:43-61).
/// Returns (items, next_page). `next_page` = Some(page + 1) iff has_next_page.
pub fn load_outcome(page: u32, res: Result<MangasPage, String>) -> Result<(Vec<RemoteManga>, Option<u32>), PageError> {
    match res {
        Err(e) => Err(PageError::Source(e)),
        Ok(p) if p.mangas.is_empty() => Err(PageError::NoResults),
        Ok(p) => Ok((p.mangas, if p.has_next_page { Some(page + 1) } else { None })),
    }
}

/// Load state of one listing generation.
#[derive(Clone, Debug, PartialEq)]
pub enum PageState {
    /// Nothing loaded yet; page 1 is in flight.
    Initial,
    /// `next` is the page to load next; `None` = end of list.
    Idle { next: Option<u32> },
    Loading { page: u32 },
    /// Page 1 failed or came back empty (refresh error).
    ErrorFirst(PageError),
    /// Later page failed; list keeps its items (append error).
    ErrorAppend { page: u32, error: PageError },
    EndOfList,
}

/// Prefetch trigger: request the next page when the last visible item index is within
/// PAGE_SIZE of the end (androidx prefetchDistance = pageSize).
pub fn should_prefetch(last_visible: usize, total_items: usize) -> bool {
    total_items > 0 && last_visible + PAGE_SIZE >= total_items
}
```

### 3.5 Browse source state

```rust
// src/browse/model/browse_source.rs
use super::{filter_tree::{apply_genre, FilterTree}, listing::Listing, favorites::*};

#[derive(Clone, Debug, PartialEq)]
pub enum BrowseDialog {
    Filter,
    RemoveManga { manga: MangaId, title: String },
    AddDuplicateManga { manga: MangaId, duplicate: MangaId, duplicate_title: String },
    ChangeMangaCategory { manga: MangaId, initial: Vec<(CategoryId, bool)> },
    Migrate { new_manga: MangaId, old_manga: MangaId },
}

pub type MangaId = i64;
pub type CategoryId = i64;

#[derive(Clone, Debug, PartialEq)]
pub struct BrowseSourceState {
    pub listing: Listing,
    pub filters: FilterTree,
    pub toolbar_query: Option<String>,
    pub dialog: Option<BrowseDialog>,
    /// Bumped by every change that restarts the page loader (behavior 4.4.1).
    pub generation: u64,
    /// Source-supplied defaults, cloned on demand (Kotlin source.getFilterList()).
    pub source_defaults: FilterTree,
    pub supports_latest: bool,
    pub is_stub: bool,
}

impl BrowseSourceState {
    pub fn new(nav: NavListing, defaults: FilterTree, supports_latest: bool, is_stub: bool) -> Self {
        let listing = Listing::from_nav(nav.clone(), defaults.clone());
        // init (BrowseMangaSourceScreenModel.kt:81-98): for Search, filters = defaults, toolbar = query.
        let (listing, toolbar_query) = match (&nav, listing) {
            (NavListing::Search(q), Listing::Search { query, .. }) => (
                Listing::Search { query: query.clone(), filters: defaults.clone() },
                query.clone(),
            ),
            (_, l) => (l, None),
        };
        Self { listing, filters: defaults.clone(), toolbar_query, dialog: None, generation: 0,
               source_defaults: defaults, supports_latest, is_stub }
    }

    /// resetFilters (line 147-151): fresh defaults; no listing change, no search.
    pub fn reset_filters(&mut self) { self.filters = self.source_defaults.clone(); }

    /// setListing (line 153-155): listing = l, toolbarQuery = null, new generation.
    pub fn set_listing(&mut self, l: Listing) {
        // Kotlin dedupes an identical Popular/Latest singleton (distinctUntilChanged); a
        // Search listing is always a new value there, so it always restarts.
        let restart = self.listing != l || matches!(l, Listing::Search { .. });
        self.listing = l;
        self.toolbar_query = None;
        if restart { self.generation += 1; }
    }

    /// setFilters (line 157-165): state only.
    pub fn set_filters(&mut self, f: FilterTree) { self.filters = f; }

    /// search(query, filters) (lines 167-182). `None` keeps current values.
    /// The base for a non-search listing is the source defaults, NOT self.filters (behavior 4.6.1).
    pub fn search(&mut self, query: Option<String>, filters: Option<FilterTree>) {
        let input = match &self.listing {
            Listing::Search { query, filters } => (query.clone(), filters.clone()),
            _ => (None, self.source_defaults.clone()),
        };
        let new_query = query.clone().or(input.0);
        self.listing = Listing::Search { query: new_query.clone(), filters: filters.unwrap_or(input.1) };
        self.toolbar_query = query.or(new_query);
        self.generation += 1;           // always restarts (Kotlin: Search never equals)
    }

    /// searchGenre (lines 184-227).
    pub fn search_genre(&mut self, genre: &str) {
        let mut defaults = self.source_defaults.clone();
        let exists = apply_genre(&mut defaults, genre);
        self.filters = defaults.clone();
        self.listing = if exists {
            Listing::Search { query: None, filters: defaults }
        } else {
            Listing::Search { query: Some(genre.to_string()), filters: defaults }
        };
        self.toolbar_query = self.listing.query().map(str::to_string);
        self.generation += 1;
    }

    /// Chip actions: Popular / Latest = resetFilters() then setListing(...).
    pub fn select_chip(&mut self, l: Listing) { self.reset_filters(); self.set_listing(l); }

    /// Navigate-up rule (BrowseMangaSourceScreen.kt:92-99). Returns true if the page should pop.
    pub fn navigate_up(&mut self) -> bool {
        if !self.listing.is_user_query() && self.toolbar_query.is_some() {
            self.toolbar_query = None;
            false
        } else {
            true
        }
    }
}
```

The favorite decisions use the pure functions of section 3.6. The GTK page calls them and dispatches the writes through the services of section 4.

### 3.6 Favorites

```rust
// src/browse/model/favorites.rs

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryRef { pub id: CategoryId, pub name: String }
pub type CategoryId = i64;

/// Changes applied by changeMangaFavorite (BrowseMangaSourceScreenModel.kt:234-253).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FavoriteChange {
    pub favorite: bool,
    /// ms since epoch when adding; 0 when removing (line 238-241).
    pub date_added_ms: i64,
    /// Remove covers from cache (MangaExtensions.kt:43-50). Local manga: never.
    pub remove_covers: bool,
    /// Add path only: SetMangaDefaultChapterFlags then bind enhanced trackers.
    pub on_add_flags_and_trackers: bool,
}

pub fn favorite_change(currently_favorite: bool, now_ms: i64) -> FavoriteChange {
    if currently_favorite {
        FavoriteChange { favorite: false, date_added_ms: 0, remove_covers: true, on_add_flags_and_trackers: false }
    } else {
        FavoriteChange { favorite: true, date_added_ms: now_ms, remove_covers: false, on_add_flags_and_trackers: true }
    }
}

/// addFavorite decision (BrowseMangaSourceScreenModel.kt:255-288).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddFavoritePlan {
    /// Write these category links (empty = remove all), then favorite.
    SetCategoriesThenFavorite(Vec<CategoryId>),
    /// Show the chooser with these checkbox rows (preselected = current links).
    ShowChooser { categories: Vec<CategoryRef>, preselected: Vec<CategoryId> },
}

/// `default_category_id` = pref default_category (-1 default). `user_categories` excludes id 0.
pub fn add_favorite_plan(
    default_category_id: i64,
    user_categories: &[CategoryRef],
    current_links: &[CategoryId],
) -> AddFavoritePlan {
    if let Some(c) = user_categories.iter().find(|c| c.id == default_category_id) {
        return AddFavoritePlan::SetCategoriesThenFavorite(vec![c.id]);   // vararg drops id 0
    }
    if default_category_id == 0 || user_categories.is_empty() {
        return AddFavoritePlan::SetCategoriesThenFavorite(Vec::new());
    }
    AddFavoritePlan::ShowChooser { categories: user_categories.to_vec(), preselected: current_links.to_vec() }
}

/// Long-click decision (BrowseMangaSourceScreen.kt:232-249).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LongClickAction {
    ConfirmRemove,
    AskDuplicate { duplicate: MangaId },
    AddFavorite,
}
pub type MangaId = i64;

pub fn long_click_action(is_favorite: bool, duplicate: Option<MangaId>) -> LongClickAction {
    match (is_favorite, duplicate) {
        (true, _) => LongClickAction::ConfirmRemove,
        (false, Some(d)) => LongClickAction::AskDuplicate { duplicate: d },
        (false, None) => LongClickAction::AddFavorite,
    }
}

/// Duplicate lookup key. Kotlin: SQL LOWER(title) = manga.title.lowercase() (ASCII-only in SQLite).
/// Kurobayomi: fetch favorites with id != manga.id, then compare `to_lowercase()` in Rust (D-DUP-01).
pub fn is_duplicate_title(candidate_title: &str, title: &str) -> bool {
    candidate_title.to_lowercase() == title.to_lowercase()
}
```

### 3.7 Sources list, sources filter, migrate sources

```rust
// src/browse/model/source_list.rs
use super::source_row::SourceRow;
use crate::browse::display_name::{Labels};

pub const LAST_USED_KEY: &str = crate::browse::display_name::LAST_USED_KEY; // "last_used"
pub const PINNED_KEY: &str = crate::browse::display_name::PINNED_KEY;       // "pinned"

#[derive(Clone, Debug, PartialEq)]
pub enum SourceListItem {
    Header { key: String },        // key = group key (LAST_USED_KEY | PINNED_KEY | lang)
    Item(SourceRow),
}

/// Group + order exactly as MangaSourcesScreenModel.collectLatestSources (lines 71-107).
/// `rows` must already be the output of GetEnabledMangaSources (section 3.7 of behavior: filter,
/// name sort, pin flags, last-used copy). `group_key` = LAST_USED if is_used_last, PINNED if
/// pins has Actual, else lang.
pub fn build_source_list(rows: &[SourceRow]) -> Vec<SourceListItem> {
    let mut groups: Vec<(String, Vec<SourceRow>)> = Vec::new();
    for r in rows {
        let key = if r.is_used_last { LAST_USED_KEY.to_string() }
            else if r.pins.contains(Pins::ACTUAL) { PINNED_KEY.to_string() }
            else { r.lang.clone() };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(r.clone()),
            None => groups.push((key, vec![r.clone()])),
        }
    }
    groups.sort_by(|(a, _), (b, _)| group_key_cmp(a, b));
    let mut out = Vec::new();
    for (k, v) in groups {
        out.push(SourceListItem::Header { key: k });
        out.extend(v.into_iter().map(SourceListItem::Item));
    }
    out
}

/// TreeMap comparator of collectLatestSources (lines 73-84). Java String.compareTo for langs.
pub fn group_key_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a, b) {
        _ if a == b => Equal,
        (LAST_USED_KEY, _) => Less,
        (_, LAST_USED_KEY) => Greater,
        (PINNED_KEY, _) => Less,
        (_, PINNED_KEY) => Greater,
        ("", _) => Greater,
        (_, "") => Less,
        _ => a.cmp(b),              // UTF-16 vs UTF-8 order differs only for supplementary chars (edge E-SL-06)
    }
}
```

Note: `group_key_cmp` returns `Equal` for `a == b` (Kotlin's comparator is not reflexive for "all", see behavior 1.5; here the keys are groups, so it is not an issue).

```rust
// src/browse/model/source_filter.rs
use super::source_row::SourceRow;
use crate::browse::SourceId;
use super::collate::case_insensitive_cmp;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct LanguageGroup { pub lang: String, pub enabled: bool, pub sources: Vec<SourceRow>, pub disabled_ids: BTreeSet<SourceId> }

/// GetLanguagesWithMangaSources.subscribe (behavior 3.2). `online` = HttpSource rows only.
pub fn languages_with_sources(
    online: &[SourceRow],
    enabled_langs: &BTreeSet<String>,
    disabled_ids: &BTreeSet<SourceId>,
) -> Vec<LanguageGroup> {
    // 1. per-source order: disabled last, then case-insensitive name (GetLanguagesWithMangaSources.kt:22-25)
    let mut sorted: Vec<SourceRow> = online.to_vec();
    sorted.sort_by(|a, b| {
        disabled_ids.contains(&a.id).cmp(&disabled_ids.contains(&b.id))
            .then_with(|| case_insensitive_cmp(&a.name, &b.name))
    });
    // 2. group by lang, keeping the per-source order inside each group
    let mut groups: Vec<LanguageGroup> = Vec::new();
    for s in sorted {
        match groups.iter_mut().find(|g| g.lang == s.lang) {
            Some(g) => g.sources.push(s),
            None => groups.push(LanguageGroup {
                lang: s.lang.clone(),
                enabled: enabled_langs.contains(&s.lang),
                sources: vec![s],
                disabled_ids: disabled_ids.clone(),
            }),
        }
    }
    // 3. enabled languages first, then display-name order (compare_languages); lang code breaks ties
    groups.sort_by(|a, b| {
        (!a.enabled).cmp(&(!b.enabled))
            .then_with(|| crate::browse::display_name::compare_languages(&a.lang, &b.lang))
            .then_with(|| a.lang.cmp(&b.lang))
    });
    groups
}

```rust
/// Toggle plans. Each returns the new set; the caller writes it to config.
pub fn toggle_language(enabled: &BTreeSet<String>, lang: &str) -> BTreeSet<String> {
    let mut s = enabled.clone();
    if !s.remove(lang) { s.insert(lang.to_string()); }   // ToggleLanguage.kt:11-14
    s
}
/// ToggleMangaSource.await(source) default: enable = currently disabled (ToggleMangaSource.kt:11,34-36).
pub fn toggle_source(disabled: &BTreeSet<SourceId>, id: SourceId) -> BTreeSet<SourceId> {
    let mut s = disabled.clone();
    if !s.remove(&id) { s.insert(id); }                  // remove when present = enable
    s
}
```

```rust
// src/browse/model/migrate_sources.rs
use super::source_row::SourceRow;
use super::collate::primary_cmp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrateSortMode { Alphabetical, Total }        // part 1 name
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection { Ascending, Descending }         // part 1 name

#[derive(Clone, Debug, PartialEq)]
pub struct MigrateSourceRow { pub source: SourceRow, pub favorite_count: i64 }

/// GetMangaSourcesWithFavoriteCount.sortFn + direction (behavior 11.2).
/// Input order is the tie-break order; the caller passes rows ordered by source id (ORDER BY source).
pub fn sort_migrate_sources(rows: &mut Vec<MigrateSourceRow>, mode: MigrateSortMode, dir: SortDirection) {
    let base = |a: &MigrateSourceRow, b: &MigrateSourceRow| -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (a.source.is_stub, b.source.is_stub) {
            (true, false) => return Less,       // stubs first (ASC)
            (false, true) => return Greater,
            _ => {}
        }
        match mode {
            MigrateSortMode::Alphabetical => primary_cmp(&a.source.name.to_lowercase(), &b.source.name.to_lowercase()),
            MigrateSortMode::Total => a.favorite_count.cmp(&b.favorite_count),
        }
    };
    // stable sort; DESCENDING reverses the whole comparator (stubs last), ties keep input order
    match dir {
        SortDirection::Ascending => rows.sort_by(|a, b| base(a, b)),
        SortDirection::Descending => rows.sort_by(|a, b| base(b, a)),
    }
}
```

Check: DESCENDING with `base(b, a)` reverses; ties equal → stable keeps input order (Kotlin `reverseOrder` keeps input order for ties too, behavior 11.2). ✓

### 3.8 Collation

```rust
// src/browse/model/collate.rs

/// Java String.CASE_INSENSITIVE_ORDER (per char: upper then lower compare). Approximated with
/// Unicode lowercase per char; matches for BMP text used by source names.
pub fn case_insensitive_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let la = a.chars().flat_map(char::to_lowercase);
    let lb = b.chars().flat_map(char::to_lowercase);
    la.cmp(lb)
}

/// Collator.getInstance(locale) with strength PRIMARY (SortUtil.kt:5-15): case and accents ignored.
/// Body: build once per process with `icu_collator::CollatorBorrowed::try_new(locale.into(), opts)`
/// where `opts.strength = Some(Strength::Primary)` and `locale` is the user's locale from
/// `icu_locale` (LANG / LC_COLLATE, fallback "und"). `compare(a, b)` is the result. If the
/// collator cannot be constructed, fall back to `a.to_lowercase().cmp(&b.to_lowercase())`.
/// Both branches are total orders (Equal for equal inputs).
pub fn primary_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::sync::OnceLock;
    static COLLATOR: OnceLock<Option<icu_collator::CollatorBorrowed<'static>>> = OnceLock::new();
    let collator = COLLATOR.get_or_init(|| {
        let mut opts = icu_collator::CollatorOptions::default();
        opts.strength = Some(icu_collator::options::Strength::Primary);
        icu_collator::CollatorBorrowed::try_new(Default::default(), opts).ok()
    });
    match collator {
        Some(c) => c.compare(a, b),
        None => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

Note: `primary_cmp` is the only function in this part whose body depends on a crate's runtime data (ICU locale data). Its behavior is fixed by the comment above; the test list in p2-edge-cases-and-tests.md covers it.

### 3.9 Global search

```rust
// src/browse/model/global_search.rs
use super::source_row::SourceRow;
use crate::browse::{SourceId, Pins};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum SourceFilter { All, PinnedOnly }   // default PinnedOnly (behavior 9.1, 9.7)

#[derive(Clone, Debug, PartialEq)]
pub enum SearchResult {
    Loading,
    Error(String),                         // throwable.message ?: unknown_error (part 1 display)
    Success(Vec<LocalManga>),
}
impl SearchResult {
    pub fn is_empty_success(&self) -> bool { matches!(self, SearchResult::Success(v) if v.is_empty()) }
    /// isVisible(onlyShowHasResults) (behavior 9.1): !only || (Success && !empty).
    pub fn is_visible(&self, only_has_results: bool) -> bool {
        !only_has_results || matches!(self, SearchResult::Success(v) if !v.is_empty())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LocalManga { pub id: MangaId, pub source: SourceId, pub url: String, pub title: String, pub favorite: bool, pub thumbnail_url: Option<String> }
pub type MangaId = i64;

#[derive(Clone, Debug)]
pub struct GlobalSearchState {
    pub from_source_id: Option<SourceId>,            // migrate only
    pub search_query: Option<String>,                // as typed (may differ from last searched)
    pub source_filter: SourceFilter,
    pub only_has_results: bool,
    /// Insertion-ordered, sorted on every change by `sort_items`. Never a comparator-keyed map.
    pub items: Vec<(SourceRow, SearchResult)>,
    /// Bumped by every `begin_search`. Results carry it; stale results are dropped.
    pub generation: u64,
    last_query: Option<String>,
    last_filter: Option<SourceFilter>,
}

impl GlobalSearchState {
    /// GlobalMangaSearchScreenModel(initialQuery, extensionFilter) (behavior 8): query as given,
    /// filter given (the extension filter forces `All`, applied by the caller).
    pub fn new(initial_query: Option<String>, source_filter: SourceFilter) -> Self {
        Self {
            from_source_id: None,
            search_query: initial_query,
            source_filter,
            only_has_results: false,
            items: Vec::new(),
            generation: 0,
            last_query: None,
            last_filter: None,
        }
    }
}
```

Algorithm functions for the global search model:

```rust
pub struct SearchPlan { pub generation: u64, pub sources: Vec<SourceRow> }

impl GlobalSearchState {
    /// behavior 9.3. Returns None when the search is a no-op (blank query or same query+filter).
    pub fn begin_search(&mut self, selected: Vec<SourceRow>) -> Option<SearchPlan> {
        let q = self.search_query.clone().filter(|q| !q.trim().is_empty())?;     // isNullOrBlank
        let same_query = self.last_query.as_deref() == Some(q.as_str());
        if same_query && self.last_filter == Some(self.source_filter) { return None; }
        self.last_query = Some(q);
        self.last_filter = Some(self.source_filter);
        self.generation += 1;
        let kept: Vec<(SourceRow, SearchResult)> = std::mem::take(&mut self.items);
        self.items = selected.into_iter().map(|s| {
            let prev = if same_query { kept.iter().find(|(k, _)| k.id == s.id).map(|(_, r)| r.clone()) } else { None };
            (s, prev.unwrap_or(SearchResult::Loading))
        }).collect();
        sort_items(&mut self.items);
        Some(SearchPlan { generation: self.generation, sources: self.items.iter().filter(|(_, r)| matches!(r, SearchResult::Loading)).map(|(s, _)| s.clone()).collect() })
    }

    /// Single writer (GTK main thread). Drops stale generations and non-Loading targets (behavior 9.3 line 156).
    pub fn apply(&mut self, generation: u64, source: SourceId, result: SearchResult) -> bool {
        if generation != self.generation { return false; }
        match self.items.iter_mut().find(|(s, _)| s.id == source) {
            Some((_, r)) if matches!(r, SearchResult::Loading) => { *r = result; sort_items(&mut self.items); true }
            _ => false,
        }
    }
    pub fn progress(&self) -> usize { self.items.iter().filter(|(_, r)| !matches!(r, SearchResult::Loading)).count() }
    pub fn total(&self) -> usize { self.items.len() }
    pub fn filtered(&self) -> impl Iterator<Item=&(SourceRow, SearchResult)> { self.items.iter().filter(move |(_, r)| r.is_visible(self.only_has_results)) }
}

/// behavior 9.5 ordering: non-empty success first, pinned first, then "name (lang)".
pub fn sort_items(items: &mut Vec<(SourceRow, SearchResult)>) {
    items.sort_by(|(a, ra), (b, rb)| {
        let ka = !(matches!(ra, SearchResult::Success(v) if !v.is_empty()));
        let kb = !(matches!(rb, SearchResult::Success(v) if !v.is_empty()));
        ka.cmp(&kb)
            .then_with(|| (!a.pins.is_pinned()).cmp(&!b.pins.is_pinned()))
            .then_with(|| a.name_key().cmp(&b.name_key()))
    });
}
```

Wait: Kotlin `!pinned` key: `"${id}" !in pinnedSources` → pinned → false → first. `(!is_pinned).cmp` → pinned gives false (first). ✓. And key1: `(map[it] as? Success)?.isEmpty ?: true`: non-empty → false (first) ✓; ka = !(non-empty) = false for nonempty ✓.

Stable sort: Kotlin toSortedMap collision merges; Rust keeps both ✓.

Also `pins` for global search: `Pins.is_pinned` uses PINNED bit; but pinned-ness in global search uses `pinnedSources` set (id strings), which equals the Pin bit only if the row's pins come from the pinned set. For the global search, the SourceRow `pins` must be set from the pinned set (`Pins::pinned()` if pinned). Document in the services: build SourceRow with pins from config; `is_pinned()` = bit0.

`begin_search` uses `selected` already filtered by source filter and extension filter (section 4 services). 

Migrate subclass origin-first (fixed): `migrate_search.rs`: `fn migrate_order(items, from_source)`: sort_items with leading key `(id != from)`.

```rust
pub fn sort_items_migrate(items: &mut Vec<(SourceRow, SearchResult)>, from: SourceId) {
    items.sort_by(|(a, ra), (b, rb)| {
        (a.id != from).cmp(&(b.id != from))
            .then_with(|| sort_key_cmp(a, ra, b, rb))
    });
}
```
where `sort_key_cmp` is the comparator body of `sort_items` (non-empty first, pinned, name). The origin source is the leading key, so it is first even when its results are empty; this is the documented intent of the Kotlin subclass (behavior 10, decision D-MG-01).

### 3.10 Services and traits

```rust
// src/browse/services.rs (GTK-free)
use async_trait::async_trait;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum SourceError {
    #[error("source not installed")]  NotInstalled,
    #[error("{0}")]                   Message(String),        // error text from the source / network
}

#[async_trait]
pub trait CatalogueSourceApi: Send + Sync {
    fn id(&self) -> SourceId;
    fn supports_latest(&self) -> bool;
    /// source.getFilterList(): fresh defaults each call (behavior 4.6, 9.3).
    fn default_filters(&self) -> FilterTree;
    async fn popular(&self, page: u32) -> Result<MangasPage, SourceError>;
    async fn latest(&self, page: u32) -> Result<MangasPage, SourceError>;
    async fn search(&self, page: u32, query: &str, filters: &FilterTree) -> Result<MangasPage, SourceError>;
}

#[async_trait]
pub trait SourceRegistry: Send + Sync {
    /// MangaSourceManager.isInitialized (AndroidMangaSourceManager.kt:35-36,70).
    fn is_initialized(&self) -> bool;
    fn catalogue_sources(&self) -> Vec<SourceRow>;   // getCatalogueSources() as rows
    fn get(&self, id: SourceId) -> Option<std::sync::Arc<dyn CatalogueSourceApi>>;
    fn get_or_stub(&self, id: SourceId) -> SourceRow; // stub row (is_stub = true) when unknown
}

#[async_trait]
pub trait MangaStore: Send + Sync {
    /// NetworkToLocalManga.await (behavior 4.4): insert or return local row; title rule for non-favorites.
    async fn network_to_local(&self, source: SourceId, remote: &RemoteManga) -> Result<LocalManga, StoreError>;
    /// getDuplicateLibraryManga candidates: favorites with id != manga, any title; caller compares titles.
    async fn favorite_titles_except(&self, id: MangaId) -> Result<Vec<(MangaId, String)>, StoreError>;
    /// Partial favorite write (proposal; replaces full-row updateManga for toggles, behavior 4.8.1).
    async fn set_favorite(&self, id: MangaId, change: &FavoriteChange) -> Result<(), StoreError>;
    async fn cover_cache_remove(&self, id: MangaId) -> Result<usize, StoreError>;
    async fn set_default_chapter_flags(&self, id: MangaId) -> Result<(), StoreError>;
    async fn bind_enhanced_trackers(&self, id: MangaId, source: SourceId) -> Result<(), StoreError>;
}

#[async_trait]
pub trait CategoryStore: Send + Sync {
    async fn user_categories(&self) -> Result<Vec<CategoryRef>, StoreError>;    // excludes id 0
    async fn links_of(&self, manga: MangaId) -> Result<Vec<CategoryId>, StoreError>;
    async fn set_links(&self, manga: MangaId, ids: &[CategoryId]) -> Result<(), StoreError>;
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
#[error("store: {0}")]
pub struct StoreError(pub String);
```

Note: `set_favorite` is partial; `set_default_chapter_flags` runs first, then `set_favorite` (sequential, D-FAV-01).

Favorite write sequence (add): 
```
set_links(manga, plan.ids)            // from add_favorite_plan
set_default_chapter_flags(manga)
set_favorite(manga, change)           // favorite=true, date_added=now
bind_enhanced_trackers(manga, source)
```
Kotlin runs the category write and the favorite write without an order (behavior 4.8.2). Rust runs them in the order shown: links, then default chapter flags, then the favorite row, then enhanced trackers (decision D-FAV-01).

Remove: `cover_cache_remove` first, then `set_favorite(false, date 0)`. If the cache removal deleted at least one file, the same partial write also sets `cover_last_modified = now` (`MangaExtensions.kt:45-46`). `FavoriteChange` therefore carries `cover_last_modified_ms: Option<i64>`, filled by the service after the cache call.

Now also the mapping for DB store implementation: `rusqlite` in area 03/05; `favorite_titles_except` uses `SELECT _id, title FROM mangas WHERE favorite = 1 AND _id != ?1`; the title compare in Rust.

### 3.11 Tokio glue

Pattern (part 1 §5) applied:
- `BrowseSourcePage` owns `Rc<RefCell<BrowseSourceState>>` and `PageLoader` with `JoinHandle`s.
- On `generation` change: abort the previous loader task (`AbortHandle::abort`) and start a new loader for `(source, remote_query, generation)`. The loader answers each `Demand(page)` with `load_page(...)` (below):

```rust
/// Event sent to the GTK thread for one loaded page.
pub struct PagingEvent { pub gen: u64, pub page: u32, pub result: Result<(Vec<RemoteManga>, Option<u32>), PageError> }

async fn load_page(src: Arc<dyn CatalogueSourceApi>, q: RemoteQuery, gen: u64, page: u32) -> PagingEvent {
    let res = match &q {
        RemoteQuery::Popular => src.popular(page).await,
        RemoteQuery::Latest => src.latest(page).await,
        RemoteQuery::Search { text, filters } => src.search(page, text, filters).await,
    }
    .map_err(|e| e.to_string());
    PagingEvent { gen, page, result: load_outcome(page, res) }   // paging::load_outcome
}
```
Pages are fetched on demand, never eagerly: the loader waits on a demand channel. The main thread sends `Demand(next_page)` when `should_prefetch(last_visible, total)` is true and `PageState::Idle { next: Some(_) }`; the task loads exactly that page, sends one `PagingEvent`, and waits for the next demand. Page 1 is demanded by the page on creation.

Browse with `search`: `text` may be "". Map `SourceError::NotInstalled` → Kotlin's `SourceNotInstalledException` message (`loader_not_implemented_error`).

- Global search: on `begin_search` returns SearchPlan; main spawns one task per source, each acquires a `Semaphore::new(5)` permit (permits held only during the source call, not during DB writes — matches Kotlin pool scope at `MangaSearchScreenModel.kt:160-162`), then `search(1, query, default_filters)`, then `network_to_local` for each result (DB via spawn_blocking inside the store), then sends `SearchEvent { gen, source, result }`. Main applies via `GlobalSearchState::apply`. Cancellation: on a new `begin_search`, `JoinSet::abort_all()` (Kotlin `searchJob?.cancel()`).

- Sources list: a `watch`/channel subscription on `SourceRegistry` changes plus config changes → rebuild `Vec<SourceRow>` on main thread → `build_source_list`.

- Migrate picker: async query of favorite counts (`SELECT source, count(*) ... ORDER BY source`), then `sort_migrate_sources` on main thread.

Generation rules table:

| Event | Generation field | Stale rule |
|---|---|---|
| Browse paging | `BrowseSourceState.generation` | page events with other gen dropped |
| Global search | `GlobalSearchState.generation` | `apply` returns false when stale |
| Sources list rebuild | none (snapshot rebuild) | last snapshot wins |

### 3.12 Config (BrowsePrefs)

Owned by part 1 (`browse::prefs::BrowsePrefs`, `p1-rust-design.md` section 10). This part adds the fields listed in `p2-data-and-config.md` section 6:

```rust
pub struct BrowsePrefsExt {
    pub global_search_has_results_only: bool,  // default false
    pub hide_in_library_items: bool,           // default false
    pub last_used_source: i64,                 // default -1
    pub default_manga_category: i64,           // default -1 (area 37 key)
    pub library_columns_portrait: i32,         // default 0 (area 37)
    pub library_columns_landscape: i32,        // default 0 (area 37)
}
```

Writes: atomic (temp + rename), main thread (part 1 rule).

---

## 4. Mapping of Kotlin concepts to Rust

| Kotlin | Rust | Note |
|---|---|---|
| `StateScreenModel` + `MutableStateFlow` | `Rc<RefCell<State>>` on the GTK main thread; repaint explicitly | no `equals` based dedupe |
| `screenModelScope` (Main) | `glib::MainContext` futures (`spawn_future_local`) | |
| `ioCoroutineScope` (IO + SupervisorJob) | tokio multi-thread runtime; per-screen `JoinSet` | cancellation on drop |
| `launchIO` / `withIOContext` | `tokio::task::spawn_blocking` for rusqlite; async reqwest in source calls | |
| `Executors.newFixedThreadPool(5)` | `tokio::sync::Semaphore::new(5)` | no thread leak |
| `Channel(Int.MAX_VALUE)` events | `async_channel::unbounded` | |
| `Channel()` rendezvous for browse query | direct method call on the open page | D-CH-01 |
| `Pager` + `PagingConfig(25)` + `cachedIn` | `paging::PageState` + demand channel | section 3.11 |
| `collectAsLazyPagingItems` | `gio::ListStore` appended on the main thread | part 1 §6 |
| `stateIn(ioCoroutineScope, Lazily, ...)` | page loader started on first show, stopped on pop | |
| `distinctUntilChanged` on listing | `generation` counter | section 1.3 |
| `FilterList.equals` = false | never compare filter trees | |
| `TreeMap` with comparator (`toSortedMap`) | `Vec` + `sort_by` (stable) | D-GS-01 |
| `Collator.getInstance(...).strength = PRIMARY` | `icu_collator` `Strength::Primary` | section 8 |
| `LocaleHelper.comparator` | `display_name::compare_languages` (part 1) | |
| `Preference.appStateKey(...)` | TOML key without prefix (section 6 of p2-data) | |
| `HapticFeedback` | no-op (dropped) | behavior 15 |
| `LocalUriHandler.openUri` | `gtk::UriLauncher::new(url).launch(...)` (GTK 4.10+) | |
| `context.toast(MR.strings.internal_error)` | in-window toast (part 1 overlay) | no libadwaita |
| `ifMangaSourcesLoaded()` | `SourceRegistry::is_initialized()` gate on page creation | |
| Voyager `navigator.push/pop/replace` | part 1 `browse::nav` (`gtk::Stack` push/pop helpers in `src/screens.rs`/`src/ui.rs`) | |
| `rememberScreenModel` | model owned by the page struct, dropped on pop | |

---

## 5. Threading and cancellation (concrete)

1. Page creation (main): build state, `services` handles (Arc), `tx/rx` channels, spawn `glib::spawn_future_local(recv_loop)`.
2. Browse: main sends `Demand(page)` on the demand channel when `should_prefetch`; the tokio loader task answers with `PagingEvent { gen, page, result }`; main applies it (`PageState` transition, appends to `ListStore`). Loader aborted on generation change or pop.
3. Global search: `begin_search` (main) returns `SearchPlan`; main creates a `JoinSet`, one task per source: `permit = semaphore.acquire()` → `search(1, q, defaults)` → drop permit → for each manga `network_to_local` (store uses `spawn_blocking`) → `SearchEvent { gen, source, result }`. Main `apply`s each event.
4. Migrate picker and Sources list: a single snapshot query each, then pure sort on main.
5. Favorite writes: one tokio task per user action, sequential steps (section 3.10), result posted back as `FavoriteEvent { manga, result }`; main updates the row (live favorite flag via the store's change notifications, area 05).
6. Dropping a page aborts its JoinSet and loader; `glib` source ids removed.

Timing budget: none specified in Kotlin (no debounce). Kurobayomi keeps none for browse search and global search (submit-only).

---

## 6. Error handling

- Pure modules return values; no `unwrap()` on user data. The Kotlin `!!` sites: `MigrateMangaSearchScreenModel.kt:22` (`getManga.await(mangaId)!!`) and `MigrateMangaScreen` (part 1). Rust: `Option` + error toast.
- `SourceError` and `StoreError` are `thiserror` enums. Display text for the user: `SourceError::Message(text)` shows the raw text in the global search error row (behavior 9.3, part 1 §10.2); browse error rows use `ExceptionFormatter` rules (`p1-behavior`). `NotInstalled` -> `loader_not_implemented_error`.
- Category writes never surface errors to the UI (Kotlin catches and logs). Rust logs with `tracing::error!` and continues, matching behavior 1.6.
- Favorite write failure: show toast `internal_error` (D-FAV-02, new), state rolled back to the previous favorite flag in the view.
- Logging: `tracing` at `error` for every Kotlin `logcat(ERROR)` site (list in p2-edge-cases-and-tests.md section 5).

---

## 7. Android API -> Linux mapping (this area)

| Android / Compose / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| Voyager `Screen`, `Navigator` | `browse::nav` push/pop on the existing `gtk::Stack` (`src/screens.rs`, `src/ui.rs`) | Back = `navigate_up` rule (behavior 4.6.2) |
| `AppBar` with actions | `gtk::HeaderBar` with `gtk::Button`s | part 1 |
| `SearchToolbar` | `gtk::SearchEntry` in the header bar | part 1 |
| `FilterChip` | `gtk::ToggleButton` in a `gtk::Box` (horizontal `gtk::ScrolledWindow`) | part 1 |
| `AdaptiveSheet` (filter sheet) | `gtk::Window` modal, transient for the app window, width 480 px, or a `gtk::Popover` | part 1 |
| `HapticFeedback.LongPress` | none (dropped) | behavior 15 |
| `AssistContentScreen` | none (dropped) | behavior 15 |
| `LocalUriHandler.openUri` | `gtk::UriLauncher` | |
| `Toast` | in-window overlay label with timeout 3 s | part 1 (no libadwaita) |
| `ClipboardManager` (migrate picker long-press copy, part 1) | `gdk::Clipboard::set_text` | part 1 |
| `WebViewScreen` | `webkit6` (WebKitGTK 6) page or external browser `gtk::UriLauncher` | area 36 |
| `SharedPreferences` / `PreferenceStore` | `browse.toml` (section 3.12) | area 02 |
| Room / SQLDelight | rusqlite (area 03/05) | |
| `Executors.newFixedThreadPool(5)` | `tokio::sync::Semaphore(5)` | |
| `ioCoroutineScope` | tokio runtime + `JoinSet` | |
| `Collator` (java.text) | `icu_collator` | |
| `Locale.getDefault()` | `LANG`/`LC_*` via `icu_locale` (`sys_locale` fallback) | |
| `Instant.now().toEpochMilli()` | `std::time::SystemTime::now()` as ms | |

---

## 8. Crates (this part)

| Crate | Version | Used for | Notes |
|---|---|---|---|
| `icu_collator` | 2.x (pin at implementation) | `primary_cmp` | ICU4X; `CollatorBorrowed`, `CollatorOptions { strength: Some(Strength::Primary) }` |
| `icu_locale` | 2.x (same release train) | locale for the collator | |
| `async-trait` | 0.1 | dyn-safe async traits | part 1 uses it too |
| `thiserror` | 1 | error enums | part 1 |
| `async-channel` | 2 | events | part 1 |
| `tokio` | 1 (`rt-multi-thread`, `sync`, `time`, `macros`) | loaders, semaphore, JoinSet | part 1 |
| `tokio-util` | 0.7 | `CancellationToken` for loader cancellation (optional) | |
| `tracing` | 0.1 | error logging | |

No new crates for paging, sorting or filter trees. `std::collections::{BTreeSet, BTreeMap}` cover the sets.

Rejected: `unicode-collation`-style crates (not maintained for this purpose), `sorted-vec`/`indexmap` for the result list (a `Vec` plus stable sort is enough for <= a few hundred sources).

---

## 9. Mockup integration

Current static code (verified): `src/entry.rs`
- `sources(anime)` line 283: rows "Source A", "Source B". Replace with `source_list` page on `build_source_list(...)`.
- `migrate_sources(anime)` line 360: rows "Source A/B/C" counts 12/4/1. Replace with `sort_migrate_sources` output.
- `browse_source(id, migrating)` line 380: static cards. Replace with the browse page on `BrowseSourceState` + paging loader.
- `global_search(id, migrating)` line 432: static. Replace with `GlobalSearchState` + `begin_search` / `apply`.

`spec/app.json` declares only the top-level `browse` screen (no per-page ids for these), so the page data comes from the models, not from the spec. The spec's string keys still apply.

Order of replacement (no change to other pages): (1) `model/*` + unit tests, (2) services with fakes, (3) source list, (4) browse page, (5) global search, (6) migrate picker. Each step is wired into `entry.rs` behind the existing function names so the mockup builds at every step.

---

## 10. Pitfalls (for the implementer)

1. Do not compare `FilterTree` for "same listing" anywhere; use the generation (section 1.3). Kotlin's `distinctUntilChanged` only dedupes singleton `Popular`/`Latest`.
2. Do not key a map by the comparator (`toSortedMap`); collisions drop sources (D-GS-01). Use `Vec` + stable sort.
3. `primary_cmp` and `group_key_cmp` must be total orders with `Equal` for equal inputs (Rust `sort_by` may panic on inconsistent comparators in newer std versions).
4. The favorite write must not send the whole manga row (behavior 4.8.1; `coalesce` writes every snapshot field). Use the partial `set_favorite`.
5. `Pins::xor` for `minus` exists only for the Kotlin-equivalent last-used copy; the copy is never persisted.
6. `search()` return-early rules (blank query; same query and same filter) must be preserved, or repeated Enter re-runs every request.
7. A global search started with `PinnedOnly` and zero pinned sources has an empty selection. Show the empty state; do not fall back to All (behavior 9.7).
8. Genre search must be applied to the source defaults clone; never mutate `self.filters` in place (Kotlin replaces the filter list).
9. `TriState` raw values other than 0..2 must not panic (D-BR-02).
10. The migrate origin-first key is the leading sort key (D-MG-01); the base key follows.
11. Browse search and global search take the text as typed; trim only for the blank check (`isNullOrBlank`), not for the query sent (Kotlin sends the untrimmed text).
12. `Listing::Search { query: None }` sends `""` to the source. Some sources reject `""`; this is Kotlin behaviour; keep it.
13. The Sources list must not show "empty" before `SourceRegistry::is_initialized()` (D-SS-01).
14. Group key `"pinned"` and `"last_used"` can collide with a real language code only if a source declares such a language. Keep the key as a typed enum internally (`GroupKey::{LastUsed, Pinned, Lang(String)}`) if the implementation is refactored; the string form is kept for display only.
