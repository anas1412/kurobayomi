# Area 40 (part 2) topic: the global search engine

Covers `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt` (235 lines), its subclass `GlobalMangaSearchScreenModel.kt`, and the wrapper `GlobalMangaSearchScreen.kt`. The migrate subclass (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaSearchScreenModel.kt`) shares the engine. Algorithms are in `p2-behavior.md` section 9; this file gives the state machine, worked examples, and the concurrency design.

## 1. State

| Field | Kotlin | Rust (`browse/model/global_search.rs`) |
|---|---|---|
| query as typed | `State.searchQuery` | `search_query: Option<String>` |
| last searched query | `lastQuery` | `last_query` |
| last searched filter | `lastSourceFilter` | `last_filter` |
| source filter | `State.sourceFilter` (`PinnedOnly` default) | `source_filter` |
| has-results toggle | `State.onlyShowHasResults` (pref-backed) | `only_has_results` |
| per-source results | `PersistentMap<CatalogueSource, MangaSearchItemResult>` sorted by comparator | `Vec<(SourceRow, SearchResult)>` sorted by `sort_items` |
| job | `searchJob: Job?` | `JoinSet` owned by the page |
| generation | none (implicit in Job) | `generation: u64` |

## 2. Transitions

```
            updateSearchQuery(q)            (state only)
  Idle ----------------------------------> Idle (query changed, nothing fetched)

  Idle --search()--> [blank query]  -------> Idle (no-op)
  Idle --search()--> [same query, same filter] -> Idle (no-op)
  Idle --search()--> [new query or new filter]
        gen += 1
        results: keep Success/Error for still-selected sources if same query; else all Loading
        spawn one fetch per source still Loading
        -> Running(gen)

  Running(gen) --apply(gen, source, result)--> Running(gen)   (only if source is Loading)
  Running(gen) --apply(old gen, ...)-------> Running(gen)     (dropped)
  Running(gen) --all sources non-Loading--> Done(gen)
  Running(gen) --search() with new query--> Running(gen+1)    (JoinSet aborted)
  any --page pop--> (JoinSet aborted, state dropped)
```

Notes:
- `Done` has no event; "done" is `progress == total` (`progress` counts non-Loading entries).
- Errors do not return to Loading. Only a new query or a new source filter that selects a source again re-fetches it (because `same_query` keeps Error entries, and the Loading check at `MangaSearchScreenModel.kt:156-158` skips them). Re-fetching an errored source therefore requires a different query.

## 3. Worked example: query and filter changes

Setup: pinned = {A}; enabled sources = {A (en, pinned), B (en), C (ja)}; Kotlin default `PinnedOnly`.

| Step | Action | Sources selected | Result map after step | Fetches started |
|---|---|---|---|---|
| 1 | open global search with query "one" (MangaScreen title search) | `PinnedOnly` -> {A} | `{A: Loading}` | A |
| 2 | A returns 3 manga | {A} | `{A: Success(3)}` | none |
| 3 | `setSourceFilter(All)` | {A, B, C} (sorted: pinned first, then name) | `{A: Success(3), B: Loading, C: Loading}` (same query -> A kept) | B, C |
| 4 | B fails with `IOException` | | `{A: Success(3), B: Error, C: Loading}` | none |
| 5 | `search()` again, same query, same filter | | no change (GS-EC-02) | none |
| 6 | edit query to "two", `search()` | {A, B, C} | `{A: Loading, B: Loading, C: Loading}` (new query -> all reset) | A, B, C |

Display order after step 3 (has-results off): non-empty success first (A), then the Loading ones: pinned first (none pinned among B, C), then name. Final order at step 2 is A alone.

## 4. Worked example: ordering

Input after all results (has-results off):

| Source | Pinned | Name | Lang | Result |
|---|---|---|---|---|
| Zed | no | Zed | en | Success(0) |
| Alpha | no | Alpha | ja | Success(2) |
| Beta | yes | Beta | en | Loading |
| Gamma | yes | Gamma | en | Success(1) |
| Delta | no | delta | en | Error |

Key per row: `(non-empty? , not pinned, "name (lang)" lowercased)`.

| Row | non-empty key | pinned key | name key |
|---|---|---|---|
| Alpha | 0 (non-empty) | 1 | "alpha (ja)" |
| Gamma | 0 | 0 (pinned) | "gamma (en)" |
| Beta | 1 | 0 | "beta (en)" |
| Delta | 1 | 1 | "delta (en)" |
| Zed | 1 | 1 | "zed (en)" |

Sorted: Gamma comes first. Alpha and Gamma both have 0 for the first key (non-empty), and the pinned key (0 for Gamma, 1 for Alpha) puts Gamma before Alpha. Among the rest, Beta comes next because its pinned key is 0, then Delta and Zed by name. Result: **Gamma, Alpha, Beta, Delta, Zed**.

Kotlin produces the same order (`MangaSearchScreenModel.kt:58-64`). Its `TreeMap` would also merge two rows with identical keys (see section 6).

## 5. Concurrency design

Kotlin:
- Per-source `async` blocks run on `ioCoroutineScope` (IO dispatcher). The fetch itself runs inside `withContext(Executors.newFixedThreadPool(5))` (`MangaSearchScreenModel.kt:46,160-162`).
- `updateItem` reads `state.value.items`, builds a copy, writes it back (`:192-197`). Many coroutines call it.
- Race: two results completing together can both copy the same snapshot; the second write drops the first result. The dropped source stays Loading until the next search.

Kurobayomi:
- The GTK main thread is the only writer. Each fetch task sends `SearchEvent { generation, source, result }` over an `async_channel`.
- The fetch task: `permit = semaphore.acquire()` (5 permits), `source.search(1, query, defaults)`, drop permit; then for each returned manga `store.network_to_local(...)` (inside `spawn_blocking` in the store) and send the event. The permit is held only for the source call, so DB work does not block other fetches. Kotlin's pool covers only the source call (`:160-162`); the DB insert runs on IO outside the pool (`:164-166`).
- `apply(generation, source, result)` (`global_search.rs`): if `generation != self.generation` return false; if the source's entry is not `Loading` return false; else set and re-sort (stable). Returns true for a visible change.
- `begin_search` aborts the previous `JoinSet` before creating the new plan (equivalent to `searchJob?.cancel()`, `:135`).

Failure of a single fetch: it sends `Error` for its source; other fetches continue (Kotlin `awaitAll` with per-async try/catch, `:171-175`).

## 6. Collision example (why `Vec` and not a map)

Two installed extensions both provide a source named "MangaX" in language "en", both pinned or both not pinned, both with results of the same emptiness. The sort key `(empty?, pinned, "mangax (en)")` is identical for both.

- Kotlin `toSortedMap(comparator)`: the second entry replaces the first. One source disappears from the results and `total` is one lower.
- Rust `Vec` + stable sort: both rows are kept in insertion order.

Test: `global_sort_keeps_collisions` (`p2-edge-cases-and-tests.md` 7.1, GS-EC-04).

## 7. Filter interaction with extension filter

`GlobalMangaSearchScreenModel` with an extension package:
1. Constructor sets `extensionFilter` (may be `""`).
2. If `extensionFilter != null` the source filter becomes `All` and `search()` runs (only if the query is not blank).
3. `getSelectedSources()` returns the extension's catalogue sources intersected with `getEnabledSources()`.

GS-EC-11: extension filter `""` means "no extension restriction" (`isNullOrEmpty`, `:99-101`) but still forces `All`. Rust: an empty package string is treated as None for selection, and still sets `All`, to keep the Kotlin observable behavior.

## 8. Single-result auto-open (wrapper)

`GlobalMangaSearchScreen.kt:41-64` shows a loading screen when all three hold: query not empty, extension filter not empty, and `state.total == 1` at first composition. It then watches `state.items`:
- `Loading` -> keep waiting.
- `Success` with exactly one manga -> replace the page with the manga page (no back entry to the search).
- `Success` with zero or several manga, `Error`, or more than one source -> show the results.

Rust: the same decision runs on each applied event for the single source. The page replaces itself with the manga page only when the search was opened by a deep link or extension intent (the only callers that pass an extension filter, `MainActivity.kt:530`).

## 9. Test vectors

| Vector | Input | Expected |
|---|---|---|
| V1 | Blank query `"  "` | `begin_search` returns None |
| V2 | Same query, same filter, twice | second returns None |
| V3 | Same query, filter All after PinnedOnly (section 3, step 3) | plan fetches B and C; A keeps its Success result |
| V4 | Two applies, same source, generations 3 and 2 | only gen 3 changes state |
| V5 | Apply to a source that is already Success | returns false |
| V6 | Example of section 4 | order Gamma, Alpha, Beta, Delta, Zed |
| V7 | Collision pair | both kept |
| V8 | `only_has_results = true` with Loading, Error, Success(0), Success(1) | only Success(1) visible |
| V9 | progress with 1 Loading, 2 Success, 1 Error of 4 | progress 3, total 4 |
