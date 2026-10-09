# Deep dive: global search (fan-out, ordering, visibility, auto-open)

Covers `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt` (wrapper) and the part-1 view files `app/src/main/java/eu/kanade/presentation/browse/anime/GlobalAnimeSearchScreen.kt`, `components/GlobalAnimeSearchToolbar.kt`, `components/GlobalAnimeSearchCardRow.kt`. Behavior summary: `p2-behavior.md` section 7. Rust module: `browse/anime/global_search.rs`.

## 1. Model state (Kotlin → Rust)

| Kotlin field | Type | Rust field | Notes |
|---|---|---|---|
| `State.searchQuery` | `String?` | `query: Option<String>` | updated per keystroke, no search |
| `State.sourceFilter` | `AnimeSourceFilter` | `mode: SourceFilterMode` | default `PinnedOnly` (`AnimeSearchScreenModel.kt:202`) |
| `State.onlyShowHasResults` | `Boolean` | `only_with_results: bool` | mirrors `__APP_STATE_has_filters_toggle_state` |
| `State.items` | `PersistentMap<AnimeSource, AnimeSearchItemResult>` | `items: Vec<(SourceRef, SearchResult)>` sorted | sorted on every write |
| `State.fromSourceId` | `Long?` | not stored | never set by global search (only migrate search) |
| `lastQuery` | `String?` | `last_query` | exact string, untrimmed |
| `lastSourceFilter` | `AnimeSourceFilter?` | `last_filter` | |
| `enabledLanguages` | `Set<String>` | `enabled_langs` | snapshot at creation (E-GS-07) |
| `disabledSources` | `Set<String>` | `disabled` | snapshot |
| `pinnedSources` | `Set<String>` | `pinned` | snapshot |
| `extensionFilter` | `String?` | `extension_filter` | pkg name, set by subclass init |
| `searchJob` | `Job?` | `cancel: CancellationToken` | Kotlin never cancels (E-GS-03) |
| `coroutineDispatcher` | 5-thread pool | shared `Semaphore(5)` | D-6 |

## 2. Source selection (exact)

Kotlin base (`AnimeSearchScreenModel.kt:85-94`):

```
candidates = sourceManager.getAll()                             // installed incl. local; stubs excluded
  .filter { it.lang in enabledLanguages && "${it.id}" !in disabledSources }
  .sortedWith(compareBy({ "${it.id}" !in pinnedSources }, { "${it.name.lowercase()} (${it.lang})" }))
```

Subclass (`GlobalAnimeSearchScreenModel.kt:25-28`): when `sourceFilter == PinnedOnly`, keep only pinned ids.

Extension filter (`AnimeSearchScreenModel.kt:96-108`): when the pkg name is non-empty, keep only sources of the installed extension with that pkg name, intersected with `candidates`.

Note: the local source has `lang = "other"`. It is candidate-eligible only if `"other"` is in `enabledLanguages` (E-GS-05). The default language set does not include `other`.

Rust:

```rust
pub fn select_sources(
    installed: &[SourceRef],
    enabled_langs: &HashSet<String>,
    disabled: &HashSet<SourceId>,
    pinned: &HashSet<SourceId>,
    mode: SourceFilterMode,
    extension_sources: Option<&HashSet<SourceId>>,   // Some when extension_filter is non-empty
) -> Vec<SourceRef> {
    let mut out: Vec<SourceRef> = installed
        .iter()
        .filter(|s| enabled_langs.contains(&s.lang) && !disabled.contains(&s.id))
        .filter(|s| mode != SourceFilterMode::PinnedOnly || pinned.contains(&s.id))
        .filter(|s| extension_sources.map_or(true, |set| set.contains(&s.id)))
        .cloned()
        .collect();
    out.sort_by_key(|s| (!pinned.contains(&s.id), format!("{} ({})", s.name.to_lowercase(), s.lang)));
    out
}
```

Note: `sort_by_key` with `(bool, String)` sorts `false` (pinned) before `true` (not pinned) because `!pinned.contains` is `false` for pinned. ✓ same as Kotlin `"${it.id}" !in pinnedSources`.

Extension-filter note: Kotlin intersects the extension's sources with the enabled set (`filter { it in enabledSources }`), which is the same as the code above when `extension_sources` is applied after the language filter.

## 3. Search algorithm (exact)

Kotlin (`AnimeSearchScreenModel.kt:123-179`), steps numbered as in the source:

1. `query = state.searchQuery`, `sourceFilter = state.sourceFilter`.
2. If `query` is null or blank → return (no state change).
3. `sameQuery = (lastQuery == query)`.
4. If `sameQuery && lastSourceFilter == sourceFilter` → return.
5. `lastQuery = query`, `lastSourceFilter = sourceFilter`.
6. `sources = getSelectedSources()`.
7. New items:
   - `sameQuery`: for each source, keep its existing result if present, else `Loading`.
   - otherwise: every source is `Loading`.
   - write (sorted).
8. Launch fan-out. For each source (concurrently):
   - If its state is not `Loading` → skip (already has a result).
   - `page = source.getSearchAnime(1, query, source.getFilterList())` on the pool.
   - `titles = page.animes.map { networkToLocalAnime.await(it.toDomainAnime(source.id)) }`.
   - If the job is active → write `Success(titles)`.
   - On `Exception` → if active write `Error(e)`.

Rust (`global_search.rs`):

```rust
pub fn search(&mut self, catalog: &dyn SourceCatalog, net: Arc<NetworkToLocal>, tx: async_channel::Sender<UiMsg>) {
    let q = match &self.query { Some(q) if !q.trim().is_empty() => q.clone(), _ => return };   // step 2
    let same_query = self.last_query.as_deref() == Some(q.as_str());                          // step 3
    if same_query && self.last_filter == Some(self.mode) { return; }                          // step 4
    self.last_query = Some(q.clone());                                                        // step 5
    self.last_filter = Some(self.mode);
    self.generation += 1;                                                                      // D-5
    self.cancel.cancel();
    self.cancel = CancellationToken::new();
    let selected = self.selected_sources(catalog);                                             // step 6
    let previous = std::mem::take(&mut self.items);
    self.items = selected.iter().map(|s| {                                                     // step 7
        let kept = previous.iter().find(|(p, _)| p.id == s.id).map(|(_, r)| r.clone());
        let r = if same_query { kept.unwrap_or(SearchResult::Loading) } else { SearchResult::Loading };
        (s.clone(), r)
    }).collect();
    self.sort_items();
    for s in selected {                                                                        // step 8
        if !matches!(self.result_of(s.id), Some(SearchResult::Loading)) { continue; }
        let Some(src) = catalog.get(s.id) else { continue };
        let (gen, q, tx, cancel, net) = (self.generation, q.clone(), tx.clone(), self.cancel.clone(), net.clone());
        tokio::spawn(async move {
            let _permit = global_permits().acquire_owned().await;
            let outcome = tokio::select! {
                _ = cancel.cancelled() => return,
                r = src.search(1, &q, &src.filter_list()) => r,
            };
            let result = match outcome {
                Ok(page) => SearchResult::Ok { anime: net.upsert_all(s.id, &page.animes).await },
                Err(e) => SearchResult::Failed { message: e.raw_message() },
            };
            let _ = tx.send(UiMsg::Search { generation: gen, source: s.id, result }).await;
        });
    }
}
```

`global_permits()` returns the process-wide `Arc<Semaphore>` with 5 permits (D-6). The `select!` guards the wait; a cancelled search does not take a permit for its request. Results are applied on the main thread through `on_result` (`p2-rust-design.md` 4.3).

Important: `search()` runs only on submit. Changing text alone does nothing (OBSERVED).

## 4. Ordering (exact)

Kotlin comparator (`AnimeSearchScreenModel.kt:58-64`):

```
key(source) = (
    (result as? Success)?.isEmpty ?: true,      // false (non-empty Success) first
    "${id}" !in pinned,                         // pinned first
    "${name.lowercase()} (${lang})"              // ascending plain string compare
)
```

Rust: `search_sort_key(s, Some(result), pinned.contains(&s.id))` (`p2-rust-design.md` 3.4). Order of results arriving: the map is re-sorted on every write, so a non-empty success moves to the top immediately.

Worked example (all sources selected, pinned = {A}):

| Source | Result | Sort key (empty?, not pinned?, name) | Position |
|---|---|---|---|
| A (pinned) | Ok, 3 items | (false, false, "a (en)") | 1 |
| C | Ok, 2 items | (false, true, "c (en)") | 2 |
| B | Ok, 0 items | (true, true, "b (en)") | 4 |
| D | Loading | (true, true, "d (en)") | 5 |
| E | Failed | (true, true, "e (en)") | 6 |
| F | Ok, 1 item (not pinned) | (false, true, "f (en)") | 3 |

Result order: A, C, F, B, D, E.

## 5. Progress and visibility

- `progress = count(result != Loading)`; `total = count(all results)`.
- Progress bar shown when `progress in 1..<total`, fraction `progress / total`.
- `filteredItems` with `only_with_results = true`: keep only `Ok` results with at least one item. `false`: keep everything.
- Toggling "Has results" does not search and does not change `progress` or `total`.

Worked example: total 6 (from section 4), progress 5 (Loading D excluded). Bar shown (1 <= 5 < 6). With "Has results" on, visible: A, C, F (B empty, D loading, E failed hidden).

## 6. Chips

| Chip | Event | Effect |
|---|---|---|
| Pinned | pressed | `setSourceFilter(PinnedOnly)`: set mode, then `search()` |
| All | pressed | `setSourceFilter(All)`: set mode, then `search()` |
| Has results | pressed | toggle `only_with_results`, write `__APP_STATE_has_filters_toggle_state`, no search |

Reuse rules (E-GS-02):
- Switching PinnedOnly → All with the same query: sources already present keep results; new ones start Loading; nothing is cancelled.
- Switching All → PinnedOnly with the same query: sources no longer selected are removed; remaining results kept; no request.

## 7. Single-result auto-open

Source: `GlobalAnimeSearchScreen.kt:41-64` (wrapper). Applies only when the screen was opened with a non-empty query and a non-empty extension filter (`MainActivity.kt:539`).

Rule (exact):
- Initial flag: `query non-empty && extension filter non-empty && total == 1`, evaluated at first composition after the model's init search (so `total` already counts the one selected source).
- While the flag is set: show a spinner, and on each items change:
  - the only item is `Loading` → wait.
  - the only item is `Ok` with exactly one anime → open that anime (replacing the page).
  - otherwise (zero or many anime, `Failed`, or more than one source selected) → clear the flag and show the normal list.

Decision table (single source selected):

| Source result | Action |
|---|---|
| Loading | wait |
| Ok, 1 anime | replace page with that anime (`AnimeScreen(id, fromSource = true)`) |
| Ok, 0 anime | show list (empty card row "No results found") |
| Ok, 2+ anime | show list |
| Failed | show list with error item |

Rust: the spinner is shown while `auto_open_pending` is true; `on_result` clears it or calls `open_anime(id)`. The flag is set at construction only (not re-evaluated), as in Kotlin.

## 8. Failure text

`SearchResult::Failed { message }` stores the raw message (`Throwable.message`, may be None). The error item shows the raw message (`GlobalSearchResultItems.kt:82-94`). E-GS-06 proposes the formatted message for consistency with browse (`p2-data-and-config.md` 7.2 and `p2-behavior.md` 9.2); decision recorded in tasks.

## 9. Result rows

- Header: title = source name; subtitle = `getLocalizedDisplayName(lang)` (not `getSourceDisplayName`), so `all` shows "All".
- Click header → browse of that source with the current query (`BrowseAnimeSourceScreen(source.id, state.searchQuery)`).
- Body: Loading spinner; Ok with items → card row; Ok empty → "No results found"; Failed → error item.
- Cards: 96 px wide; title up to 3 lines; in-library badge; favorite cover alpha 0.34; live favorite state through a DB subscription on `(url, source)` (`AnimeSearchScreenModel.getAnime`, lines 74-83).
- Card click and long click: open the anime (`AnimeScreen(id, true)`). No favorite toggle here (OBSERVED).

## 10. Tests

| Test | Covers |
|---|---|
| T-TEST-GS-SORT | section 4 worked example; ties by name |
| T-TEST-GS-REUSE | section 6 reuse rules (E-GS-02, E-GS-12) |
| T-TEST-GS-STALE | a result with an old generation is dropped (E-GS-03, RC-1) |
| T-TEST-GS-SEMAPHORE | with a counting fake source, never more than 5 `search` calls in flight (E-GS-08) |
| T-TEST-GS-SINGLE | section 7 decision table |
| T-TEST-GS-SELECT | `select_sources` for: pinned-only, all, extension filter present but package missing (E-GS-09), local source with and without "other" enabled (E-GS-05) |
| T-TEST-GS-VISIBILITY | section 5 worked example |
