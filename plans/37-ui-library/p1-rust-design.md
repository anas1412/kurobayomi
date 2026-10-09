# 37-ui-library — Rust design (Kurobayomi, gtk4-rs 0.11, no libadwaita)

Target: GTK 4.16+, vanilla GTK widgets (theme applies), tokio for background I/O, rusqlite for the library database, serde for preferences/backup payloads. Everything here is a Proposal: unless marked "Observed", it does not exist in Kotlin.

Existing mockup: `src/screens.rs` (`library()` at lines 154-272) builds a `gtk::FlowBox` of placeholder cards from `spec/app.json` tabs/sheets, filters by label text, and shows a right-click/long-press action bar. `src/ui.rs` provides `page`, `linked`, `menu_button`, `show_sheet`, `show_dialog`, `icon_button`, `row`. `src/spec.rs` loads the spec JSON. There is no data layer. This design replaces the library screen internals and keeps the spec-driven chrome for everything outside this area.

---

## R-1. Module layout

```
src/
  library/
    mod.rs          // pub use; Kind, LibraryRow, LibraryState, LibraryCommand
    kind.rs         // enum Kind { Anime, Manga } and per-kind labels/keys
    model.rs        // LibraryRow, Counts, CategoryRef, visual_name()
    flags.rs        // SortCodec tables (anime/manga), TriState, bit merge, Serializer tokens
    prefs.rs        // typed LibraryPrefs over the existing settings store (src/settings.rs)
    filter.rs       // FilterSet, apply_filters()
    sort.rs         // SortSpec, comparator(), random_shuffle()
    search.rs       // Query::parse(), LibraryRow::matches()
    select.rs       // Selection (ordered Vec<i64> + index), toggle/range/select_all/invert
    pipeline.rs     // build_map(): rows + categories + prefs -> Vec<PageData>, pure
    db.rs           // rusqlite queries on animelibView/libraryView, category rows, writes
    actions.rs      // mark_read, download_next, set_categories_delta, remove_from_library, random_pick
    worker.rs       // single DB thread, request/response over async-channel
  category/
    mod.rs
    model.rs        // CategoryRecord { id, name, order, flags, hidden }, system() helper
    ops.rs          // create, rename, hide, delete (with pref cleanup), reorder (mutex-equivalent)
  ui/
    library/
      mod.rs        // LibraryView (gtk::Box) — owns toolbar, tabs, pages, action bar
      toolbar.rs    // title/count, search entry, filter button, overflow menu, selection header
      tabs.rs       // category tab strip with badge counts
      pages.rs      // gtk::Stack of CategoryPage (one per category) + lazy build of current±1
      grid.rs       // GridView factory: compact / cover-only / comfortable
      list.rs       // ColumnView-free ListView factory: row layout with list-size height
      card.rs       // shared cover+badges+continue widget builder
      settings.rs   // Filter / Sort / Display tabs (Notebook-like Stack + Switcher)
      delete_dialog.rs
      change_category_dialog.rs
      empty.rs
    category/
      mod.rs          // CategoriesView with Anime / Manga pages
      list.rs         // ListBox with drag handles (GestureDrag) and row actions
      dialogs.rs      // create / rename / delete confirmation
```

Rule: `library::*` and `category::*` contain no GTK types, so they are unit-testable without a display. Only `ui::*` imports `gtk4`.

---

## R-2. Core types

### R-2.1 Kind and labels

```rust
// src/library/kind.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind { Anime, Manga }

impl Kind {
    /// Column names differ per kind; see p1-data-and-config.md D-7.
    pub fn table(self) -> &'static str { match self { Kind::Anime => "animes", Kind::Manga => "mangas" } }
    pub fn categories_table(self) -> &'static str { match self { Kind::Anime => "categories", Kind::Manga => "categories" } }
    pub fn link_table(self) -> &'static str { match self { Kind::Anime => "animes_categories", Kind::Manga => "mangas_categories" } }
    pub fn view(self) -> &'static str { match self { Kind::Anime => "animelibView", Kind::Manga => "libraryView" } }
    pub fn completed_status(self) -> i64 { 2 }   // SAnime.COMPLETED == SManga.COMPLETED == 2
}
```
Note: the categories table name is the same for both kinds only if the DB has one table per kind. Upstream has `categories` in both `dataanime` and `data` sqldelight packages; verify the physical table names in the Rust migration design (area:03/04). Keep the `Kind` mapping in one place.

### R-2.2 LibraryRow (unified)

One struct for both kinds. Kind-specific fields are `Option` only where a kind lacks them; counts are unified names.

```rust
// src/library/model.rs
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryRow {
    pub kind: Kind,
    pub id: i64,                 // anime._id / manga._id
    pub source: i64,
    pub title: String,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,     // empty if NULL
    pub status: i64,             // 2 == completed
    pub favorite: bool,          // always true inside the library
    pub last_update: i64,        // 0 if NULL (see E-13)
    pub date_added: i64,
    pub next_airing_at: i64,     // anime only; 0 for manga
    pub fetch_interval: i32,     // anime only; < 0 = outside release period; manga: from calculate_interval
    pub fetch_type: i64,         // anime: 1 episodes, 0 seasons; manga: 1
    pub cover_url: Option<String>,
    pub cover_last_modified: i64,
    pub category: i64,           // 0 = system (uncategorized) — one LibraryRow per (entry, category link)
    pub total: i64,              // totalCount / totalChapters
    pub seen: i64,               // seenCount / readCount
    pub bookmarks: i64,
    pub fillermarks: i64,        // anime only
    pub latest_upload: i64,
    pub fetched_at: i64,         // episodeFetchedAt / chapterFetchedAt
    pub last_activity: i64,      // lastSeen / lastRead
    pub scanlator_filtered: bool,// manga: rows already exclude excluded scanlators in SQL
    // view-time badge values (filled by pipeline::decorate, never stored)
    pub badges: Badges,
}

impl LibraryRow {
    pub fn unseen(&self) -> i64 { self.total - self.seen }          // unseenCount / unreadCount
    pub fn has_started(&self) -> bool { self.seen > 0 }
    pub fn has_bookmarks(&self) -> bool { self.bookmarks > 0 }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Badges {
    pub download_count: i64,     // 0 when the download badge is off
    pub unseen_count: i64,       // 0 when the unread badge is off (drives continue-button coupling — see E-02)
    pub is_local: bool,          // false when the local badge is off
    pub source_language: String, // "" when the language badge is off
}
```

Proposal rationale: one struct means one pipeline and one set of tests. The Kotlin `AnimeLibraryItem` wrapper is the `Badges` field plus `LibraryRow`.

`LibraryRow` is one row per (entry, category link), matching the SQL view. Grouping by category happens in `pipeline::build_map` (R-4). Equality for selection uses `id` only (R-6), which is a deliberate deviation from Kotlin data-class equality (fixes E-09).

### R-2.3 Categories

```rust
// src/category/model.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryRecord { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
impl CategoryRecord {
    pub const UNCATEGORIZED_ID: i64 = 0;
    pub fn is_system(&self) -> bool { self.id == Self::UNCATEGORIZED_ID }
    /// Upstream: system -> "Default" (label_default). Others -> name.
    pub fn visual_name(&self) -> &str { if self.is_system() { "Default" } else { &self.name } }
}
```
`visual_name` returns the English fallback; the UI wraps it in gettext (`gettextrs` or the existing i18n approach in the repo). Keys: `label_default`, see p1-ui-parity.md §U-9.

---

## R-3. Flags and preferences

### R-3.1 Sort codec (`src/library/flags.rs`)

```rust
pub const SORT_TYPE_MASK: i64 = 0b0011_1100;
pub const SORT_DIR_MASK: i64 = 0b0100_0000;
pub const SORT_DIR_ASC: i64 = 0b0100_0000;

/// LocalAnimeSource.ID == LocalMangaSource.ID == 0 (source-local, area:14).
pub const LOCAL_SOURCE_ID: i64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortType { Alphabetical, LastSeenOrRead, LastUpdate, UnseenOrUnread, Total, LatestUpload, FetchDate, DateAdded, TrackerMean, AiringTime, Random }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortSpec { pub ty: SortType, pub ascending: bool }

pub fn anime_types() -> &'static [(SortType, i64, &'static str)] {   // (type, flag, token)
    &[
      (SortType::Alphabetical,    0b0000_0000, "ALPHABETICAL"),
      (SortType::LastSeenOrRead,  0b0000_0100, "LAST_SEEN"),
      (SortType::LastUpdate,      0b0000_1000, "LAST_ANIME_UPDATE"),
      (SortType::UnseenOrUnread,  0b0000_1100, "UNSEEN_COUNT"),
      (SortType::Total,           0b0001_0000, "TOTAL_EPISODES"),
      (SortType::LatestUpload,    0b0001_0100, "LATEST_EPISODE"),
      (SortType::FetchDate,       0b0001_1000, "EPISODE_FETCH_DATE"),
      (SortType::DateAdded,       0b0001_1100, "DATE_ADDED"),
      (SortType::TrackerMean,     0b0010_0000, "TRACKER_MEAN"),
      (SortType::AiringTime,      0b0011_0000, "AIRING_TIME"),
      (SortType::Random,          0b0011_1100, "RANDOM"),
    ]
}
pub fn manga_types() -> &'static [(SortType, i64, &'static str)] {
    &[
      (SortType::Alphabetical,    0, "ALPHABETICAL"),
      (SortType::LastSeenOrRead,  4, "LAST_READ"),
      (SortType::LastUpdate,      8, "LAST_MANGA_UPDATE"),
      (SortType::UnseenOrUnread,  12, "UNREAD_COUNT"),
      (SortType::Total,           16, "TOTAL_CHAPTERS"),
      (SortType::LatestUpload,    20, "LATEST_CHAPTER"),
      (SortType::FetchDate,       24, "CHAPTER_FETCH_DATE"),
      (SortType::DateAdded,       28, "DATE_ADDED"),
      (SortType::TrackerMean,     32, "TRACKER_MEAN"),
      (SortType::Random,          60, "RANDOM"),
    ]
}

/// Mirrors Kotlin Type.valueOf: find first type with flag == (flags & 0b0011_1100); else Alphabetical.
pub fn decode_flags(kind: Kind, flags: i64) -> SortSpec {
    let table = match kind { Kind::Anime => anime_types(), Kind::Manga => manga_types() };
    let ty = table.iter().find(|(_, f, _)| *f == (flags & SORT_TYPE_MASK)).map(|t| t.0).unwrap_or(SortType::Alphabetical);
    SortSpec { ty, ascending: flags & SORT_DIR_MASK != 0 }
}

/// Merge like Kotlin Long.plus(Flag): keep bits outside the mask, set the new masked bits.
pub fn merge_flags(old: i64, flag: i64, mask: i64) -> i64 { (old & !mask) | (flag & mask) }

pub fn encode_flags(kind: Kind, spec: SortSpec) -> i64 {
    let table = match kind { Kind::Anime => anime_types(), Kind::Manga => manga_types() };
    let f = table.iter().find(|(t, _, _)| *t == spec.ty).map(|t| t.1).unwrap_or(0);
    f | if spec.ascending { SORT_DIR_ASC } else { 0 }
}
```

Compatibility decision (T-37a-002): keep decoding of `flags == 0` as `Descending` (observed) so existing backups behave the same. The Default category therefore starts Z→A. Proposal: make it an explicit migration choice, not a silent change.

Notice that the manga table has no `AiringTime`. `decode_flags(Kind::Manga, 48)` returns `Alphabetical` (matches Kotlin, see D-2.5).

Random seed: `i32` in prefs. Rust RNG: `rand::rngs::StdRng::seed_from_u64(seed as u64)`. Exact shuffle parity with Kotlin's `Random(seed)` is **not** required (no cross-app state depends on it); document this as a deliberate deviation.

### R-3.2 TriState

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TriState { #[default] Disabled, Is, Not }
impl TriState {
    pub fn next(self) -> Self { match self { Self::Disabled => Self::Is, Self::Is => Self::Not, Self::Not => Self::Disabled } }
    pub fn apply(self, pred: impl FnOnce() -> bool) -> bool {
        match self { Self::Disabled => true, Self::Is => pred(), Self::Not => !pred() }
    }
    pub fn as_pref_str(self) -> &'static str { match self { Self::Disabled => "DISABLED", Self::Is => "ENABLED_IS", Self::Not => "ENABLED_NOT" } }
}
```
Stored names match Kotlin enum names (D-2.2).

### R-3.3 Typed preferences (`src/library/prefs.rs`)

```rust
pub struct LibraryPrefs {
    pub display_mode: DisplayMode,                  // shared (T-37a-049 decision: default shared)
    pub columns: [i32; 2],                          // [portrait, landscape] per Kind -> see ColumnsPrefs
    pub download_badge: bool, pub unread_badge: bool, pub local_badge: bool, pub language_badge: bool,
    pub continue_button: bool, pub category_tabs: bool, pub category_counts: bool,
    pub categorized_sort: bool, pub hidden_categories_visible: bool,
    pub restrictions: BTreeSet<String>,            // library_update_manga_restriction
    pub downloaded_only: bool,                      // app-state, read-only here
    pub filters: FilterPrefs,                       // per kind (see R-3.4)
    pub sort_global: SortSpec,                      // animelib_sorting_mode / library_sorting_mode
    pub random_seed: i32,                           // per kind
    pub last_category_index: usize,                 // per kind, app-state
}
```
Persistence: the existing settings store in `src/settings.rs` (check its API). Keys are exactly those in p1-data-and-config.md D-3. A typed wrapper is `get(&self) -> T` plus `set(&mut self, T)`; each setter emits a `PrefChanged(Key)` on a `tokio::sync::broadcast` channel so the pipeline recomputes.

Shared vs split (T-37a-049): Proposal = keep **display mode, badges, tabs, counts, continue button** shared (matches upstream muscle memory), split **per-kind filters, sort, seed, columns, last category**. Document in settings UI as "applies to both libraries".

### R-3.4 Filter prefs

```rust
#[derive(Clone, Copy, Default)]
pub struct FilterPrefs {
    pub downloaded: TriState, pub unseen_or_unread: TriState, pub started: TriState,
    pub bookmarked: TriState, pub completed: TriState, pub interval_custom: TriState,
}
pub struct TrackerFilters(pub BTreeMap<i64, TriState>); // keyed by tracker id (logged-in only)
```

---

## R-4. Pure pipeline

```rust
// src/library/pipeline.rs
pub struct Snapshot<'a> {
    pub categories: &'a [CategoryRecord],           // visible categories (hidden = 0), ORDER BY sort
    pub rows: &'a [LibraryRow],                     // from db::load_rows(kind)
    pub track_ids: &'a HashMap<i64, Vec<i64>>,      // entry id -> tracker ids that have a track (all trackers)
    pub track_scores: &'a HashMap<i64, Option<f64>>,// entry id -> mean of logged-in trackers' 10-point scores; None if no logged-in track
    pub prefs: &'a LibraryPrefs,
    pub kind: Kind,
    pub downloaded_only: bool,
    pub local_download_count: &'a dyn Fn(i64) -> i64,  // downloads on disk for an entry (cache)
    pub source_lang: &'a dyn Fn(i64) -> String,
    pub source_name: &'a dyn Fn(i64) -> String,
    pub tracker_filter: &'a TrackerFilters,
    pub query: Option<&'a Query>,
    pub random_seed: i32,
    pub collator: &'a icu_collator::Collator,
}

pub struct Page { pub category: CategoryRecord, pub items: Vec<LibraryRow>, pub visible_count: usize }
pub struct Built { pub pages: Vec<Page>, pub has_active_filters: bool, pub library_count: usize }

pub fn build(s: &Snapshot) -> Built {
    // 1. decorate badges (upstream getLibraryFlow, AnimeLibraryScreenModel.kt:387-402)
    let decorated: Vec<LibraryRow> = s.rows.iter().map(|r| {
        let mut r = r.clone();
        r.badges = Badges {
            download_count: if s.prefs.download_badge { (s.local_download_count)(r.id) } else { 0 },
            unseen_count: if s.prefs.unread_badge { r.unseen() } else { 0 },
            is_local: s.prefs.local_badge && r.source == LOCAL_SOURCE_ID,
            source_language: if s.prefs.language_badge { (s.source_lang)(r.source) } else { String::new() },
        };
        r
    }).collect();

    // 2. group by category id, preserving DB row order inside each group
    let mut grouped: BTreeMap<i64, Vec<LibraryRow>> = BTreeMap::new();
    for r in decorated { grouped.entry(r.category).or_default().push(r); }

    // 3. display categories: drop the system category when entries exist and none is uncategorized
    let shown: Vec<CategoryRecord> = if !grouped.is_empty() && !grouped.contains_key(&0) {
        s.categories.iter().filter(|c| !c.is_system()).cloned().collect()
    } else {
        s.categories.to_vec()
    };

    let downloaded_filter = if s.downloaded_only { TriState::Is } else { s.prefs.filters.downloaded };
    let skip_outside = s.prefs.restrictions.contains("manga_outside_release_period");
    let tracker_lists = TrackerLists::new(s.tracker_filter);
    let f = s.prefs.filters;
    let mut has_active = s.tracker_filter.0.values().any(|t| *t != TriState::Disabled);
    has_active |= [f.downloaded, f.unseen_or_unread, f.started, f.bookmarked, f.completed, f.interval_custom]
        .iter().any(|t| *t != TriState::Disabled);

    let mut pages = Vec::with_capacity(shown.len());
    let mut distinct = BTreeSet::new();
    for cat in shown {
        // 4. filter: AND of all predicates (upstream applyFilters, AnimeLibraryScreenModel.kt:198-251)
        let mut items: Vec<LibraryRow> = grouped.get(&cat.id).cloned().unwrap_or_default()
            .into_iter()
            .filter(|r| {
                downloaded_filter.apply(|| r.badges.is_local || r.badges.download_count > 0 || (s.local_download_count)(r.id) > 0)
                    && f.unseen_or_unread.apply(|| r.unseen() > 0)
                    && f.started.apply(|| r.has_started())
                    && f.bookmarked.apply(|| r.has_bookmarks())
                    && f.completed.apply(|| r.status == 2)
                    && (!skip_outside || f.interval_custom.apply(|| r.fetch_interval < 0))
                    && tracker_lists.accepts(r.id, s.track_ids)
            })
            .collect();
        // 5. sort: each category uses its own flags (upstream Category.sort)
        let spec = decode_flags(s.kind, cat.flags);
        if spec.ty == SortType::Random {
            items.shuffle(&mut StdRng::seed_from_u64(s.random_seed as u64));
        } else {
            items.sort_by(|a, b| compare(spec, a, b, s.track_scores, s.collator));
        }
        // 6. search after sort; None skips, Some("") matches every row (E-03)
        if let Some(q) = s.query {
            items.retain(|r| q.matches(r, &(s.source_name)(r.source)));
        }
        for r in &items { distinct.insert(r.id); }
        let visible_count = items.len();
        pages.push(Page { category: cat, items, visible_count });
    }
    // 7. active flag from RAW prefs (E-22: upstream ignores the downloaded-only override here)
    // 8. distinct entries across pages (upstream libraryCount, E-11)
    Built { pages, has_active_filters: has_active, library_count: distinct.len() }
}

/// Include/exclude tracker ids, computed once per build (fixes the O(n^2) rebuild upstream, T-37a-054).
pub struct TrackerLists { include: Vec<i64>, exclude: Vec<i64>, active: bool }

impl TrackerLists {
    pub fn new(f: &TrackerFilters) -> Self {
        let include: Vec<i64> = f.0.iter().filter(|(_, s)| **s == TriState::Is).map(|(k, _)| *k).collect();
        let exclude: Vec<i64> = f.0.iter().filter(|(_, s)| **s == TriState::Not).map(|(k, _)| *k).collect();
        let active = !include.is_empty() || !exclude.is_empty();
        Self { include, exclude, active }
    }

    pub fn accepts(&self, row_id: i64, track_ids: &HashMap<i64, Vec<i64>>) -> bool {
        if !self.active { return true; }
        let have: &[i64] = track_ids.get(&row_id).map(|v| v.as_slice()).unwrap_or(&[]);
        let is_excluded = !self.exclude.is_empty() && have.iter().any(|t| self.exclude.contains(t));
        let is_included = self.include.is_empty() || have.iter().any(|t| self.include.contains(t));
        !is_excluded && is_included
    }
}
```

Algorithm (must match Kotlin order; see p1-behavior.md §B-2 to §B-6):
1. `decorate`: `download_count = if prefs.download_badge { local_download_count(id) } else { 0 }`; `unseen_count = if prefs.unread_badge { row.unseen() } else { 0 }`; `is_local = if prefs.local_badge { source == LOCAL_SOURCE_ID } else { false }`; `source_language = if prefs.language_badge { source_lang(source) } else { "" }`.
2. Group: `BTreeMap<i64, Vec<LibraryRow>>` keyed by `row.category`, insertion order = row order from DB.
3. Display categories: `if !rows.is_empty() && !grouped.contains_key(&0) { categories.filter(|c| !c.is_system()) } else { categories.to_vec() }`. Each page's items = `grouped.get(&c.id).cloned().unwrap_or_default()`.
4. Filter predicate per row (AND):
   - downloaded: `TriState::apply(if downloaded_only { Is } else { prefs.downloaded }, || is_local || download_count > 0 || local_download_count(id) > 0)` (note: second check always calls the cache, same as Kotlin).
   - unseen: `apply(prefs.unseen, || row.unseen() > 0)`.
   - started: `apply(prefs.started, || row.has_started())`.
   - bookmarked: `apply(prefs.bookmarked, || row.has_bookmarks())`.
   - completed: `apply(prefs.completed, || row.status == 2)`.
   - interval: `if restrictions.contains("manga_outside_release_period") { apply(prefs.interval_custom, || row.fetch_interval < 0) } else { true }`.
   - tracking: see R-4.1.
5. Sort per page (R-5). Random: shuffle with `StdRng::seed_from_u64(seed)`; no tiebreak.
6. Search per page after sort (R-7). If `query` is `None`, skip; `Some("")` matches all (E-03 kept for parity).
7. `has_active_filters = [downloaded, unseen, started, bookmarked, completed, interval].any(!= Disabled) || tracker_filter.any(!= Disabled)` using the **raw** prefs (E-22: upstream ignores downloaded-only here; proposal fixes it, T-37a-056).
8. `library_count = distinct row.id across all pages` (E-11).

### R-4.1 Tracker filter

```rust
fn tracker_ok(row_id: i64, f: &TrackerFilters, track_ids: &HashMap<i64, Vec<i64>>) -> bool {
    if f.0.is_empty() { return true; }                               // no logged-in tracker
    let excluded: Vec<i64> = f.0.iter().filter(|(_, s)| **s == TriState::Not).map(|(k,_)| *k).collect();
    let included: Vec<i64> = f.0.iter().filter(|(_, s)| **s == TriState::Is).map(|(k,_)| *k).collect();
    if excluded.is_empty() && included.is_empty() { return true; }
    let have = track_ids.get(&row_id).map(|v| v.as_slice()).unwrap_or(&[]);
    let is_excluded = !excluded.is_empty() && have.iter().any(|t| excluded.contains(t));
    let is_included = included.is_empty() || have.iter().any(|t| included.contains(t));
    !is_excluded && is_included
}
```
Precompute the include/exclude lists once per build (fix for the O(n²) in upstream; T-37a-054).

---

## R-5. Sorting (`src/library/sort.rs`)

```rust
pub fn compare(spec: SortSpec, a: &LibraryRow, b: &LibraryRow, scores: &HashMap<i64, Option<f64>>, collator: &icu_collator::Collator) -> Ordering {
    let asc = spec.ascending;
    let base = match spec.ty {
        SortType::Alphabetical => collate(a, b, collator),
        SortType::LastSeenOrRead => a.last_activity.cmp(&b.last_activity),
        SortType::LastUpdate => a.last_update.cmp(&b.last_update),
        SortType::UnseenOrUnread => zero_last(a.unseen(), b.unseen(), asc),
        SortType::Total => a.total.cmp(&b.total),
        SortType::LatestUpload => a.latest_upload.cmp(&b.latest_upload),
        SortType::FetchDate => a.fetched_at.cmp(&b.fetched_at),
        SortType::DateAdded => a.date_added.cmp(&b.date_added),
        SortType::TrackerMean => score_or_default(a.id, scores).total_cmp(&score_or_default(b.id, scores)),
        SortType::AiringTime => airing(a, b, asc),
        SortType::Random => Ordering::Equal,   // handled by shuffle
    };
    let directed = if asc { base } else { base.reverse() };
    directed.then_with(|| collate(a, b, collator))   // tiebreak always ascending, like upstream
}

/// Mirrors upstream: a zero count sorts after non-zero counts in BOTH directions (see E-15).
/// This is the pre-reversal comparison; `compare` reverses it for descending.
fn zero_last(x: i64, y: i64, asc: bool) -> Ordering {
    match (x == y, x == 0, y == 0) {
        (true, _, _) => Ordering::Equal,
        (false, true, _) => if asc { Ordering::Greater } else { Ordering::Less },
        (false, false, true) => if asc { Ordering::Less } else { Ordering::Greater },
        (false, false, false) => x.cmp(&y),
    }
}

/// Upstream AiringTime: unseen count first (plain compare), then nextEpisodeAiringAt with zero-last.
fn airing(a: &LibraryRow, b: &LibraryRow, asc: bool) -> Ordering {
    match a.unseen().cmp(&b.unseen()) {
        Ordering::Equal => zero_last(a.next_airing_at, b.next_airing_at, asc),
        other => other,
    }
}

fn score_or_default(id: i64, scores: &HashMap<i64, Option<f64>>) -> f64 {
    scores.get(&id).copied().flatten().unwrap_or(-1.0)
}

fn collate(a: &LibraryRow, b: &LibraryRow, c: &icu_collator::Collator) -> Ordering {
    c.compare(&a.title.to_lowercase(), &b.title.to_lowercase())
}
```

Exact implementations (must match p1-behavior.md §B-5.3):
- `zero_last(u1, u2, asc)` (pre-reversal): equal → Equal; `u1 == 0` → `if asc { Greater } else { Less }`; `u2 == 0` → `if asc { Less } else { Greater }`; else `u1.cmp(&u2)`.
- `airing(a, b, asc)`: first `unseen` compare (plain, not zero-last), then zero-last on `next_airing_at`.
- `score_or_default`: `scores.get(&id).copied().flatten().unwrap_or(-1.0)`; **deliberate fix** of the NaN case (E-14). Upstream would yield NaN for "tracks exist, none logged in" — Rust returns `-1.0` via the `None` path because `track_scores` only holds `Some` for a logged-in mean.
- `collate(a, b, c)`: `c.compare(&a.title.to_lowercase(), &b.title.to_lowercase())`.

Collator: `icu_collator::Collator::try_new(locale, options)` with the locale from `LANG` (fallback `und`). Build once in the worker.

The `Random` branch: `let mut v = items.to_vec(); v.shuffle(&mut StdRng::seed_from_u64(seed))`. Per-kind seed.

Stable sort: Rust `sort_by` is stable; upstream `sortedWith` is stable too. Keep the same input order (DB order) for ties.

---

## R-6. Selection (`src/library/select.rs`)

```rust
#[derive(Default, Clone)]
pub struct Selection { order: Vec<i64>, snapshot: HashMap<i64, LibraryRow> }  // order = insertion; snapshot keeps the row for bulk actions

impl Selection {
    pub fn contains(&self, id: i64) -> bool { self.snapshot.contains_key(&id) }

    pub fn len(&self) -> usize { self.order.len() }

    pub fn is_empty(&self) -> bool { self.order.is_empty() }

    fn add(&mut self, row: &LibraryRow) {
        if !self.snapshot.contains_key(&row.id) {
            self.order.push(row.id);
            self.snapshot.insert(row.id, row.clone());
        }
    }

    fn remove(&mut self, id: i64) {
        self.order.retain(|x| *x != id);
        self.snapshot.remove(&id);
    }

    /// Plain click while selection mode is on: flip membership.
    pub fn toggle(&mut self, row: &LibraryRow) {
        if self.contains(row.id) { self.remove(row.id) } else { self.add(row) }
    }

    /// Long-press / right-click. `page_items` is the displayed list (filtered, sorted, searched) of the
    /// clicked row's category. Adds the inclusive range from the last selected row; never removes.
    pub fn toggle_range(&mut self, row: &LibraryRow, page_items: &[LibraryRow]) {
        let last = self.order.last().and_then(|id| self.snapshot.get(id)).cloned();
        let same_category = last.as_ref().map(|l| l.category == row.category).unwrap_or(false);
        let Some(last) = last.filter(|_| same_category) else {
            self.add(row);
            return;
        };
        let a = page_items.iter().position(|r| r.id == last.id);
        let b = page_items.iter().position(|r| r.id == row.id);
        let (Some(a), Some(b)) = (a, b) else {
            // E-09 fix: an anchor or target that is not on the displayed page falls back to a single add.
            self.add(row);
            return;
        };
        if a == b { return; }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        for r in &page_items[lo..=hi] {
            self.add(r);
        }
    }

    pub fn select_all(&mut self, page_items: &[LibraryRow]) {
        for r in page_items { self.add(r); }
    }

    /// Invert over the displayed page: selected page rows are removed, unselected ones are added.
    pub fn invert(&mut self, page_items: &[LibraryRow]) {
        for r in page_items {
            if self.contains(r.id) { self.remove(r.id) } else { self.add(r) }
        }
    }

    pub fn clear(&mut self) {
        self.order.clear();
        self.snapshot.clear();
    }

    pub fn rows(&self) -> impl Iterator<Item = &LibraryRow> {
        self.order.iter().filter_map(|id| self.snapshot.get(id))
    }
}
```

Rules (upstream behavior, with the fixes called out):
- `toggle_range`: `last = order.last()`; if `last` is missing **or** its row's category != `row.category` → add `row` only (upstream `:635-638`). Otherwise `a = index_of(last.id in page_items)`, `b = index_of(row.id in page_items)`; if either index is missing → add `row` only (**fix** for E-09; upstream would throw). Else if `a == b` → nothing. Else add ids in `min(a,b)..=max(a,b)` that are not selected, preserving order.
- `select_all`: append page items not yet selected.
- `invert`: remove selected ids present on the page; append unselected page items.
- Range never deselects (upstream parity).
- Selection survives page change and search change (upstream parity, E-10). Proposal: keep, but show a count of hidden-selected entries in the action bar tooltip. T-37a-025 acceptance.

Snapshot: store the `LibraryRow` clone at selection time, so bulk actions can run even if the view changes. Refresh the snapshot from the current pipeline output after each recompute, dropping ids that disappeared from the library.

---

## R-7. Search (`src/library/search.rs`)

```rust
pub enum Query { Id(Option<i64>), Terms { whole: String, parts: Vec<Term> } }
pub enum Term { Plain(String), Negated(String) }

impl Query {
    pub fn parse(raw: &str) -> Query {
        if raw.len() >= 3 && raw.is_char_boundary(3) && raw[..3].eq_ignore_ascii_case("id:") {
            return Query::Id(raw[3..].parse::<i64>().ok());
        }
        let parts = raw.split(',').map(|t| t.trim()).map(|t| match t.strip_prefix('-') {
            Some(rest) => Term::Negated(rest.trim_start().to_string()),
            None => Term::Plain(t.to_string()),
        }).collect();
        Query::Terms { whole: raw.to_string(), parts }
    }

    pub fn matches(&self, row: &LibraryRow, source_name: &str) -> bool {
        match self {
            Query::Id(id) => *id == Some(row.id),
            Query::Terms { whole, parts } => {
                let contains = |hay: &str, needle: &str| hay.to_lowercase().contains(&needle.to_lowercase());
                let genre_eq = |needle: &str| row.genres.iter().any(|g| g.eq_ignore_ascii_case(needle));
                let whole_hit = contains(&row.title, whole)
                    || row.author.as_deref().map(|a| contains(a, whole)).unwrap_or(false)
                    || row.artist.as_deref().map(|a| contains(a, whole)).unwrap_or(false)
                    || row.description.as_deref().map(|d| contains(d, whole)).unwrap_or(false);
                whole_hit || parts.iter().all(|t| match t {
                    Term::Plain(x) => contains(source_name, x) || genre_eq(x),
                    Term::Negated(x) => !(contains(source_name, x) || genre_eq(x)),
                })
            }
        }
    }
}
```
Semantics (must equal upstream, p1-behavior.md §B-6.2):
- `Id(opt)`: `opt == Some(row.id)`. Parsing: `id:` + rest; `rest.parse::<i64>().ok()`. `"id:"` → `Id(None)` → matches nothing.
- Terms: `whole` = raw query (before splitting). Match = `contains_ci(title, whole) || contains_ci(author, whole) || contains_ci(artist, whole) || contains_ci(description, whole) || parts.iter().all(|t| term_ok(t))`.
- `term_ok(Plain(x))`: `contains_ci(source_name, x) || genres.any(|g| g.eq_ignore_ascii_case(x))` (exact equal, case-insensitive).
- `term_ok(Negated(x))`: `!(contains_ci(source_name, x) || genres.any(eq_ci x))`.
- `contains_ci(h, n)`: `h.to_lowercase().contains(&n.to_lowercase())`. Upstream uses `String.contains(_, ignoreCase = true)`; Unicode-aware lowercasing is a documented superset.

Edge handling kept on purpose for parity: `""` matches all (E-03); a lone `-` excludes everything except whole-string matches (E-04). Proposal: keep both, but show a hint in the search popover (U-4).

Source name: `source_name(row.source)` = `getNameForAnimeInfo()` of the resolved source (area:17). Cache per recompute.

---

## R-8. Database (`src/library/db.rs`)

Reads (run on the DB worker):
```rust
pub fn load_rows(conn: &Connection, kind: Kind) -> rusqlite::Result<Vec<LibraryRow>>;  // SELECT * FROM <view>
pub fn load_visible_categories(conn: &Connection, kind: Kind) -> rusqlite::Result<Vec<CategoryRecord>>; // WHERE hidden = 0 ORDER BY sort
pub fn load_all_categories(conn: &Connection, kind: Kind) -> rusqlite::Result<Vec<CategoryRecord>>; // ALL rows incl. hidden, for management
pub fn load_category_ids_for_entry(conn: &Connection, kind: Kind, entry_id: i64) -> rusqlite::Result<Vec<i64>>;                     // VISIBLE only, see E-07 note
pub fn load_all_category_ids_for_entry(conn: &Connection, kind: Kind, entry_id: i64) -> rusqlite::Result<Vec<i64>>;                 // ALL (fix)
pub fn load_track_ids(conn: &Connection) -> rusqlite::Result<HashMap<i64, Vec<i64>>>;                                    // GetTracksPerAnime analogue (area:27)
```
Column mapping: `animelibView` and `libraryView` columns are named exactly as in p1-data-and-config.md D-7 (`totalCount`, `seenCount`, `latestUpload`, `episodeFetchedAt`, `lastSeen`, `bookmarkCount`, `fillermarkCount`, `category`; manga `totalChapters`, `readCount`, `chapterFetchedAt`, `lastRead`). Use column aliases in SQL so `rusqlite::Row::get("totalCount")` works.

Writes (run on the DB worker; each in one transaction):
- `set_categories(entry_id, ids)`: `DELETE FROM animes_categories WHERE anime_id = ?;` followed by one `INSERT INTO animes_categories(anime_id, category_id) VALUES (?, ?)` per id (replace-all; upstream `AnimeRepositoryImpl.setAnimeCategories`).
- `set_categories_delta(entry_id, add, remove)`: **fixed** version used by `actions::set_categories_delta` (T-37a-030): `current = load_all_category_ids_for_entry`, `new = (current − remove) ∪ add`, then replace-all with `new`. Hidden memberships are preserved because `current` is read from ALL links.
- Category CRUD and reorder: `category::ops` (R-9).
- Favorite toggle: `UPDATE animes SET favorite = 0 WHERE _id = ?` (through the entries area's write path; area:05).

Watch: the pipeline recomputes on `DbChanged(kind)` events; the write functions emit them after commit. No SQLite update hooks (they do not fire across the connection pool we will not use; avoid).

---

## R-9. Category operations (`src/category/ops.rs`)

Mirror `Create/Rename/Hide/Delete/Reorder` (p1-behavior.md §B-15.5). Specific Rust rules:
- `create(name)`: `order = max(order)+1` or 0; flags = `encode_flags(kind, global_sort)`; hidden = false. Name not trimmed (upstream). Return `Err(CategoryError::Internal)` on DB error.
- `rename(id, name)`, `hide(id)` (toggle), `reorder(id, new_index)`: non-system only; under a `tokio::sync::Mutex` (upstream `Mutex` parity).
- `delete(id)`: DELETE row; re-index all categories (including system) `0..n-1`; if `default_category == id` → clear it; remove `id` string from the 5 StringSet keys (D-11).
- All failures become `CategoryError::Internal(String)` → toast `internal_error`.

Mutual exclusion with the library pipeline: none needed; the pipeline recomputes on `DbChanged`.

---

## R-10. Background execution model

Threads:
- **GTK main thread**: all widgets, `gio::ListStore` mutations, GestureClick handlers.
- **DB worker** (one `std::thread`, owns the `rusqlite::Connection` in WAL mode, `busy_timeout 5000ms`): receives `DbRequest` over `async_channel::Sender` and replies on oneshot. The library pipeline also runs here so the UI never blocks on sort/filter for large libraries (5k+ entries).
- **tokio runtime** (multi-thread, 2 workers): network (library update, downloads, tracker sync) — owned by areas 12/26/23/24. This area only spawns: `download_next` (enqueue via the download manager API) and the update job trigger.

Bridge: a future spawned on the main context awaits the worker reply and applies it. Pattern:
```rust
let (tx, rx) = async_channel::bounded(1);
db.send(DbRequest::BuildLibrary { kind, snapshot_inputs, reply: tx });
glib::spawn_future_local(async move { if let Ok(built) = rx.recv().await { view.apply(built); } });
```
Generation counter: each request carries `gen: u64`; the UI discards replies with `gen < latest_gen` (equivalent to upstream `collectLatest`).

Debounce: search input → `glib::timeout_add_local_once(Duration::from_millis(250), move || view.request_search(text))`, cancel the previous `SourceId` on each keystroke (`SEARCH_DEBOUNCE_MILLIS = 250`).

Cancellation: long bulk actions are not cancelled on window close (upstream `launchNonCancellable`). The worker finishes queued writes before shutdown (drain on exit, 5 s cap).

---

## R-11. UI composition (`src/ui/library/*`)

Widgets (GTK 4.16 API):
- `LibraryView` = `gtk::Box` (vertical):
  - Header: `gtk::HeaderBar`-free; the app chrome is the existing `ui::page()` (keeps the spec-driven title). Title label = `Title` built from `toolbar::title_for(page)`.
  - `gtk::SearchBar` + `gtk::SearchEntry` (existing mockup pattern) for search; toggle button in header.
  - Tabs: `tabs::CategoryTabs` (horizontal `gtk::ScrolledWindow` with a `gtk::Box` of `gtk::ToggleButton` in a group, each with a label + count badge). Visible when `category_tabs && categories.len() > 1` OR `search active` (upstream E-19/E-44 parity).
  - Pages: `gtk::Stack` with `transition-type = SlideLeftRight`, `transition-duration = 200` (Adwaita-like; theme-neutral), one child per category page. Build content lazily for current ±1 (upstream composition window) and destroy others.
  - Action bar (selection): `gtk::ActionBar` at bottom, revealed when selection non-empty (`gtk::Revealer` with `SlideUp`).
- Grid: `gtk::GridView` with `gio::ListStore<glib::BoxedAnyObject>` (boxed `LibraryRow`), `SignalListItemFactory`. `min_columns` / `max_columns`: if `columns == 0` → `max_columns = u32::MAX`, cell width ≥ 128 px (scaled by `gtk::Widget::scale_factor`); else both = columns. Use `gtk::GridView::set_single_click_activate(false)`; handle clicks with `GestureClick` (button 1 press → select-or-open; button 3 → range toggle; long-press → range toggle).
- List: `gtk::ListView` with the same factory; row height = `(page_height / list_size) - (3 / list_size)` px (see E-23 — reproduce or fix; the Rust version computes in float and clamps to ≥ 48 px).
- Selection visuals: CSS class `selected` on the item (background `@theme_selected_bg_color` at 22 % / 16 % dark via `alpha()`), and `opacity: 0.76` on the cover.
- Badges: `gtk::Label` with CSS classes `badge`, `badge-accent` (tertiary), `badge-osd`. Language local badge: `gtk::Image` `folder-symbolic`.
- Continue button: `gtk::Button` `circular`, icon `media-playback-start-symbolic`, tooltip "Resume" (upstream `action_resume`).

Cover loading: `ui::cover::load(url, size) -> gdk::Texture` from area:34 cache; placeholder `gtk::Image` with `image-missing-symbolic`.

Dialogs: `gtk::Window` (modal, transient for main window) for settings (`TabbedDialog` analogue = `gtk::Notebook` with three pages), change category (`gtk::ListBox` of `gtk::CheckButton`s with `inconsistent` state for TriState-mixed), delete (`gtk::Window` with `gtk::CheckButton`s). For simple confirms `gtk::AlertDialog` (GTK ≥ 4.10) with buttons.

Toasts: an in-window `gtk::Revealer` at the bottom of the page with `gtk::Label`, auto-hide after 4 s (proposal; upstream Compose snackbar default is short).

Keyboard: `gtk::EventControllerKey` on the page: `Ctrl+A` select all in page, `Ctrl+I` invert, `Escape` clear selection (or clear search), `Ctrl+F` open search, `F5` update category, `Shift+F5` update library. Mouse: Ctrl+click toggle, Shift+click range (same category), right-click or long-press range (upstream).

Splash / ready: not needed (drop; T-37a-052).

---

## R-12. Android API → Linux mapping (library scope)

| Android / Kotlin | Linux / Rust replacement | Note |
|---|---|---|
| Compose `LazyVerticalGrid` (`GridCells.Fixed/Adaptive`) | `gtk::GridView` + `min/max_columns` | Adaptive 128 dp → computed columns. |
| `LazyColumn` list | `gtk::ListView` | Row height from list-size. |
| `HorizontalPager` | `gtk::Stack` (SlideLeftRight) | Compose offscreen window ±1 → lazy child creation. |
| `PrimaryScrollableTabRow` + `Tab` | horizontal `gtk::ScrolledWindow` + toggle buttons | Badge count in label. |
| `combinedClickable` (click + long-press) | `GestureClick` (button 1 and 3) + `GestureLongPress` | Right-click mapped to range (desktop convention). |
| `HapticFeedback` | none | Drop (no haptics on desktop). |
| `PullRefresh` | none; refresh button + F5 | No pull gesture on desktop (T-37a-036). |
| `Snackbar` | in-window revealer toast | 4 s proposal. |
| `AlertDialog` / `Dialog` / `TabbedDialog` | `gtk::AlertDialog`, `gtk::Window`, `gtk::Notebook` | Modal, transient. |
| `Checkbox` / `TriStateCheckbox` | `gtk::CheckButton` with `set_inconsistent(true)` for Exclude | Tri-state cycle handled in code. |
| `FilterChip` | `gtk::ToggleButton` in linked box | |
| `Slider` 0..10 | `gtk::Scale` (range 0..10, step 1, `draw-value`) with value text "Auto" at 0 | |
| `BackHandler` | `Escape` shortcut | |
| `LocalUriHandler.openUri(GETTING_STARTED_URL)` | `gtk::UriLauncher` (GTK 4.10+) or `gio::AppInfo::launch_default_for_uri` | Drop of Android intent path. |
| `LocalContext.startActivity(ReaderActivity.newIntent(context, mangaId, chapterId))` | open the reader page in the app (area:36 navigation) | |
| `MainActivity.startPlayerActivity` | app player route (area:player) | Respect `alwaysUseExternalPlayer` as `xdg-open`/mpv choice. |
| Voyager `ScreenModel` + `StateFlow` | `LibraryController` struct + `glib` main-context signals | |
| Injekt DI | constructor injection; one `AppServices` struct | |
| `DataStore`/`PreferenceStore` | existing `src/settings.rs` JSON/TOML store (see R-3.3) | |
| `ContentResolver` covers | file/URL cover cache (area:34) | |
| `java.text.Collator` | `icu_collator` | |
| `kotlin.random.Random` | `rand::rngs::StdRng` | Parity of sequence not required. |
| `Long.plus(Flag)` | `merge_flags` | |
| `WorkManager` (update job) | tokio task (area:26) | |

---

## R-13. Error handling

- Pure modules return `Result<T, LibraryError>` where I/O is involved; `LibraryError = Db(rusqlite::Error) | Category(CategoryError) | Source(String)`.
- Bulk actions: per-entry error is logged (`tracing::error!`) and counted; the action completes for others; UI toast "N of M updated" (proposal; upstream is silent — E-35).
- Index access: never `[i]` on user-driven indices; use `get()` (E-17). Panics in pipeline code are not allowed (`#![deny(clippy::indexing_slicing)]` in `library/` modules recommended).
- Database busy: `busy_timeout`; on final failure show toast `internal_error` (upstream string).

---

## R-14. Pitfalls

1. GTK widgets are not `Send`. Every `LibraryRow` crossing to the UI is wrapped in `BoxedAnyObject` on the main thread only.
2. `gio::ListStore` replace (`splice`) for a full page refresh; do not rebuild the whole `Stack` on every filter change (causes scroll reset). Keep `ScrolledWindow` vadjustment per page.
3. Recompute cost: keep the pipeline O(n log n). Do not clone `LibraryRow` per filter stage; filter indices and clone once for the UI.
4. The tracker-id map is rebuilt when tracker logins change; do not rebuild per row.
5. `rusqlite::Connection` is not `Sync`; keep it on the worker.
6. Theme: never hardcode colors; use `@theme_selected_bg_color`, `@accent_bg_color`, `@card_bg_color`, `@view_fg_color` in CSS. KDE with GTK apps: Breeze/Adwaita CSS names are available via the GTK theme (`gtk-application-prefer-dark-theme` follows the portal).
7. Wayland vs X11: no differences in this area; avoid `gtk::Window::move` (not supported on Wayland).
8. `GridView` virtualization needs `gtk::Widget` size requests; set `set_size_request(128, -1)` on the card to keep the adaptive math stable.
9. Keep `LibraryRow` small: cover URL and genres as `Arc<str>` / `Arc<[String]>` if the library exceeds 10k entries.

---

## R-15. Dependencies (new crates)

| Crate | Version (proposal) | Why |
|---|---|---|
| `gtk4` | `0.11` | required (mockup already uses it) |
| `glib` / `gio` | matching gtk4 0.11 | ListStore, signals |
| `rusqlite` | `0.32` with `bundled` | DB (project-wide choice) |
| `async-channel` | `2` | worker bridge |
| `tokio` | `1` (`rt-multi-thread`, `sync`, `time`) | background jobs |
| `icu_collator` | `2` | locale-aware title sort |
| `rand` | `0.8` (`StdRng`, `SliceRandom`) | Random sort |
| `serde` / `serde_json` | `1` | prefs, backup payloads |
| `tracing` | `0.1` | logs |

Exact versions are a proposal; pin when the workspace is created (area:01).

---

## R-16. Testing hooks

- All of `library::{flags, filter, sort, search, select, pipeline}` are pure: `cargo test --lib library::` without GTK.
- `db` tests use an in-memory SQLite with the seeded `categories` row and the views (area:03/04 migrations).
- UI smoke: `cargo run -- --library-demo` loads fixtures from `tests/fixtures/library/*.json` (see p1-edge-cases-and-tests.md §T-3) without a database.
