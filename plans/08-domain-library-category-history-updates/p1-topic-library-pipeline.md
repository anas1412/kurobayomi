# Area 08 — Topic: library list pipeline (rows -> categories -> filter -> sort -> search -> page model)

Sources: `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt` (all of it), `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryItem.kt`, `data/src/main/sqldelight/view/libraryView.sq`, `data/src/main/sqldelightanime/view/animelibView.sq`. Behavioural rules are in `p1-behavior.md` section 2; this file fixes the *order of operations*, the data flow and the Rust structure.

## 1. Observed data flow (one screen model per kind)

```
libraryView rows (Flow, re-emits on any change of mangas / chapters / history / mangas_categories / excluded_scanlators)
   |  + item preferences (badges, filters, downloaded-only, ENTRY_OUTSIDE_RELEASE_PERIOD in restrictions)   [Flow, combine of 12 prefs]
   |  + downloadCache.changes (tick)
   v
getLibraryFlow():  rows -> MangaLibraryItem(+badges) -> groupBy category id
   |  + GetVisibleMangaCategories.subscribe()
   v
Map<Category, List<Item>>   (display-category rule: drop Default if library non-empty and has no Default rows)
   |  combine with:  searchQuery.debounce(250 ms), tracks per manga (GetTracksPerManga.subscribe()), tracking filter flow, downloadCache.changes
   v
applyFilters(trackMap, trackingFilter)   -> mapValues { it.filter(filterFn) }
applySort(trackMap, loggedInTrackerIds)  -> mapValues { sorted by that category's own flags }
search (if query != null)               -> mapValues { it.filter { item.matches(query) } }
   v
State.library  (collectLatest: a newer emission cancels an older, unfinished computation)
```

Order matters: filters -> sort -> search. Search runs last, so an `id:` or text query only reduces a list that is already filtered and sorted. `collectLatest` means the heavy `applyFilters/applySort` of an outdated emission is cancelled when a newer input arrives.

Important detail: `applyFilters` re-reads the item preferences with `getLibraryItemPreferencesFlow().first()` inside the combine, so a preference change both re-triggers `getLibraryFlow` (it also combines the same flow) and the final combine through `getTrackingFilterFlow`/`downloadCache`. The result is idempotent; Rust computes everything from one immutable snapshot (`LibraryInputs`) instead.

## 2. Rust structure (Proposal)

```rust
// kuro-domain::library::pipeline  (pure functions, no I/O, no async)
pub struct LibraryInputs<'a, K: EntryKind> {
    pub rows: &'a [LibraryRow<K>],            // one per (entry, category)
    pub categories: &'a [Category],            // visible categories in order
    pub prefs: &'a ItemPrefs,                  // badges + filter states + downloaded_only + skip_outside_release_period
    pub tracking_filter: &'a BTreeMap<i64, TriState>,   // logged-in trackers only
    pub tracks: &'a HashMap<i64, Vec<TrackRef>>,        // entry id -> tracker ids + scores
    pub random_seed: i32,
    pub query: Option<&'a str>,
    pub downloads: &'a dyn DownloadIndex,      // count(entry) -> u32, is_local via source id 0
    pub sources: &'a dyn SourceLookup,         // lang + display name (get_or_stub semantics)
    pub collator: &'a Collator,
}
pub struct LibraryPage { pub category: Category, pub items: Vec<LibraryItem> }
pub fn build_library<K: EntryKind>(i: LibraryInputs<K>) -> Vec<LibraryPage>;
```

Steps inside `build_library`:

1. `group`: `BTreeMap`-free grouping by `row.category` into `HashMap<i64, Vec<usize>>` (indices into `rows`).
2. `display_categories`: apply the Default-hiding rule (behaviour 2.3 step 4).
3. For each page build items with the badge fields (only compute `download_count`, `source_language` when the corresponding badge preference is on; those lookups are not free).
4. `filter` (short-circuit order: downloaded, unread/unseen, started, bookmarked, completed, interval-custom, tracking).
5. `sort` per page with its own `LibrarySort::from_flags(category.flags)`.
6. `search` last.

Complexity: O(rows) grouping, O(n log n) per page; with 10,000 library rows (an extreme user) the whole pipeline is a few milliseconds plus the collator cost; precompute the lowercase title and its collation key once per item (`Vec<(Box<[u8]>, usize)>`) to avoid O(n log n) collator calls (a collation sort key makes alphabetical a byte compare).

## 3. Triggers and threading (Proposal)

- Source of truth: SQLite. A `LibraryService` task (tokio) listens to the change bus tables `{mangas, chapters, history, mangas_categories, categories, excluded_scanlators, manga_sync}` (anime: `{animes, episodes, animehistory, animes_categories, categories, anime_sync}`), the `LibraryPrefs` change channel, the download-index change channel and the search channel.
- Coalescing: wait 30 ms after the first trigger to merge bursts (a library update touches thousands of rows), then run `build_library` on a blocking thread (`spawn_blocking`), never on the GTK main loop.
- Publish the immutable result as `Arc<LibraryState>` through `tokio::sync::watch`; the GTK side holds a `glib::MainContext::spawn_local` receiver that swaps the model of the page widgets. A newer trigger arriving during computation sets a `dirty` flag and the task loops (equivalent of `collectLatest`).
- Search debounce 250 ms (`SEARCH_DEBOUNCE_MS`) is applied in the UI layer before sending the query to the service.

## 4. Filter and sort inputs not stored in `libraryView`

- Tracks: area:27-tracking-core provides `tracks_per_entry() -> HashMap<i64, Vec<Track>>` with `tracker_id` and `score`; the 10-point score is `tracker.get10PointScore(track)` of the tracker implementation (area:27/28-33).
- Download counts: area:23/24 download index; local entries (`source == 0`) always count as downloaded for the filter.
- `fetchInterval`, `lastUpdate`, `dateAdded`, `nextEpisodeAiringAt`: columns of the entry row embedded in the library row.
- Source display name for search: `source.getNameForMangaInfo()` = the source's display name (for a source with a language the extension name is combined with the language per area:17); in Rust `SourceLookup::display_name(source_id)`; for a missing extension the stub name `"<id>"` style value from `getOrStub` (area:17).

## 5. Library selection helpers (domain-pure, UI-agnostic)

```rust
pub struct Selection { ids: Vec<(i64 /*entry id*/, i64 /*category id*/)> }   // ordered
impl Selection {
    pub fn toggle(&mut self, entry: (i64, i64));
    pub fn range(&mut self, entry: (i64, i64), page_items: &[i64]);     // toggleRangeSelection semantics
    pub fn select_all(&mut self, page_items: &[(i64, i64)]);
    pub fn invert(&mut self, page_items: &[(i64, i64)]);
}
```
`range` appends the clicked entry only when the last selected entry belongs to a different category; otherwise selects the inclusive index span between them in the page's current order skipping already selected ids. Identity for membership checks is the entry id (an entry in two categories cannot be selected twice), but `(entry, category)` is retained because `range` needs the category of the last selected row.
