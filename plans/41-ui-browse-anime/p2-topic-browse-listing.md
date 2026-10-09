# Deep dive: browse listing, paging, network-to-library mapping, favorites

Covers `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt` (listing, pager, favorite flow), `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt` (chips, dialogs, navigate-up), `data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt` (paging rules) and `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt` (row mapping). Behavior summary: `p2-behavior.md` section 5. Rust module: `browse/anime/source_browse.rs`, `browse/anime/pager.rs`, `browse/anime/library_add.rs`.

## 1. Listing state machine

States of the listing (what the user chooses):

```
             chip Popular / nav "Popular"
   +----------+ ------------------------------> +----------+
   | Popular  |                                 |  Latest  | (only if supportsLatest)
   +----------+ <------------------------------ +----------+
        |  ^   chip Popular                         |
        |  |                                        |
        v  |  search(query) or genre                |
   +--------------------------------------------------------+
   | Search { query: Option<String>, filters }              |
   +--------------------------------------------------------+
```

Transitions (exact, from `BrowseAnimeSourceScreenModel.kt` and `BrowseAnimeSourceScreen.kt`):

| From | Event | To | Side effects |
|---|---|---|---|
| any | chip Popular | Popular | `resetFilters()` then `setListing(Popular)`; `toolbarQuery = null`; pager restarts |
| any | chip Latest | Latest | same, with Latest |
| Popular or Latest | toolbar submit `q` | Search { query: q, filters: source defaults } | `toolbarQuery = q`; pager restarts |
| Search { q, f } | toolbar submit `q2` | Search { query: q2, filters: f } | `toolbarQuery = q2`; pager restarts |
| Search { q, f } | Filter pressed (filters `f2`) | Search { query: q, filters: f2 } | pager restarts |
| any | `searchGenre(g)` | Search { None or Some(g), filters: defaults with genre applied } | pager restarts |
| Search { q, f } | clear toolbar text | unchanged listing | `toolbarQuery = null`; no reload |
| any | Escape or back | see section 4 (navigate-up rule) | |

Important: a Search transition always builds a new `Search` value (OBSERVED `search()` lines 163-176), and `AnimeFilterList` equality is always false (`AnimeFilterList.kt:10-12`). So every Search transition restarts the pager, including when nothing changed (E-BR-06).

Rust rule (D-7): the listing compares by value. Restart only when the value differs. A toolbar submit of the same text while the same listing is shown is still a deliberate refresh request, so it restarts explicitly (`search_refresh()`), not through equality.

## 2. Pager (exact rules)

Source: `data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt:34-65`.

Inputs for one load: `key` (page number, `null` on the first load), `page = key ?: 1`.

| Step | Condition | Result |
|---|---|---|
| 1 | request `requestNextPage(page)` returns `AnimesPage(animes, hasNextPage)` | |
| 2 | `animes.isEmpty()` | `LoadResult.Error(NoEpisodesException)` (formatted "No results found") |
| 3 | `animes` not empty | `LoadResult.Page(data = animes, prevKey = null, nextKey = if (hasNextPage) page + 1 else null)` |
| 4 | request throws | `LoadResult.Error(e)` |
| 5 | refresh key | `getRefreshKey(state)`: anchor page `prevKey ?: nextKey`; Rust: restart from page 1 on refresh |

Truth table for the first load (page 1):

| Source response | UI result |
|---|---|
| 0 animes, `hasNextPage` any | empty state "No results found" + actions (Retry, Open in WebView, Help) |
| n > 0 animes, `hasNextPage = false` | list of n; no more loads |
| n > 0 animes, `hasNextPage = true` | list of n; append when near the end |
| request error | empty state with the formatted message and actions |

Truth table for an append (page k > 1):

| Source response | UI result |
|---|---|
| 0 animes | append error "No results found" as snackbar (Indefinite) with Retry; list keeps existing items |
| n > 0, `hasNextPage = true` | items appended; next page scheduled |
| n > 0, `hasNextPage = false` | items appended; end |
| request error | snackbar with the formatted message, Retry |

Prefetch: `PagingConfig(pageSize = 25)` (`BrowseAnimeSourceScreenModel.kt:112`). Androidx defaults around it: `prefetchDistance = 25`, `initialLoadSize = 75`, `enablePlaceholders = true`. The source chooses its own page size. Rust: request the next page when the last visible index is within 25 items of the end and the state is Idle with `next_page` set.

Rust pager state machine (`pager.rs`):

```
Idle --refresh--> Loading(Refresh) --ok, animes>0--> Idle (or Done if !hasNext)
                              \--ok, animes==0 & page 1--> Failed(Refresh, NoEpisodes)  [empty state]
                              \--error--> Failed(Refresh, e)
Idle --append (next_page=Some)--> Loading(Append) --ok animes>0--> Idle / Done
                                                  \--ok animes==0--> Failed(Append, NoEpisodes) [snackbar]
                                                  \--error--> Failed(Append, e) [snackbar]
Failed(Append, _) --Retry--> Loading(Append) for the same page
Failed(Refresh, _) --Retry--> Loading(Refresh)
any --refresh (new listing)--> generation += 1; cancel in-flight; Loading(Refresh)
```

A result whose generation is not current is dropped (RC-2).

## 3. Network-to-library mapping (per item)

Source: `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt:12-27`.

| Lookup `(url, source)` result | Action | Returned row | Stored row changed? |
|---|---|---|---|
| none | insert from the source entry (defaults from `Anime.create()`, `favorite = false`, `dateAdded = 0`) | inserted row with new id | yes (insert) |
| found, `favorite = false` | none | stored row with `title = source title` in memory | no. The UI re-reads the stored row, so the stored title is shown (E-BR-10) |
| found, `favorite = true` | none | stored row unchanged | no |

After mapping, the UI subscribes to the row by `(url, source)` (`GetAnime.subscribe`, `GetAnime.kt:26-28`) and filters it with `hideInLibraryItems`. Rust (D-8 / Q-1): default is the same (show the stored title). The insert path for a page of 25 can be one transaction (PROPOSAL, performance only).

## 4. Navigate-up rule

Source: `BrowseAnimeSourceScreen.kt:92-99`:

```
navigateUp():
    if (!state.isUserQuery && state.toolbarQuery != null) -> setToolbarQuery(null)
    else                                                 -> navigator.pop()
```

`isUserQuery` = `listing is Search && listing.query is non-empty` (`BrowseAnimeSourceScreenModel.kt:365`).

Consequences:
- Typed-but-not-submitted text on Popular: first back clears it, second back pops (E-BR-05).
- Search with a submitted query: back pops immediately (the user query is kept, the page is left).
- Close-search button of the toolbar calls `navigateUp` too (part 1 toolbar).

## 5. Favorite flow (decision tree)

Long press (or the desktop equivalent) on an item with snapshot `a`:

```
duplicate = repo.duplicate_favorites(a.id, a.title.to_lower())    // SQL in p2-data-and-config.md 6.2
if a.favorite:
    show Remove dialog (a)                                          // E-BR-14: favorite wins over duplicate
else if duplicate.first() exists:
    show Duplicate dialog (a, duplicate.first())                    // E-BR-13: first by _id ascending in Rust
else:
    add_favorite(a)
```

Dialog outcomes:

| Dialog | Button | Effect |
|---|---|---|
| Remove | Cancel | none |
| Remove | Remove | `change_favorite(a)` (removes) |
| Duplicate | Show anime | open `AnimeScreen(duplicate.id)` |
| Duplicate | Migrate existing entry | show Migrate dialog (new = a, old = duplicate) |
| Duplicate | Add anyway | `add_favorite(a)` (categories still apply) |
| Duplicate | Cancel | none |
| Category picker | OK | `change_favorite(a)` then set categories to the selection |
| Category picker | Cancel | none (nothing written) |

### 5.1 `add_favorite(a)` (category resolution)

Source: `BrowseAnimeSourceScreenModel.kt:247-279`.

Inputs: `categories` = user categories (system category id 0 excluded, `BrowseAnimeSourceScreenModel.kt:286-291`), `default_id` = `default_anime_category` (default -1).

| Condition (evaluated in order) | Action |
|---|---|
| `categories.find(id == default_id)` is Some(c) | set categories = [c.id] (vararg path filters id != 0), then `change_favorite(a)` |
| `default_id == 0` OR `categories.is_empty()` | set categories = [] , then `change_favorite(a)` |
| otherwise | show Category picker with preselection = `repo.categories_of(a.id)` ids |

Table of concrete cases:

| default_id | user categories | Result |
|---|---|---|
| -1 (unset, default) | none | add with no category |
| -1 | [3 Action, 7 Favorites] | picker, preselect from `categories_of(a.id)` |
| 0 (Uncategorized) | [3, 7] | add with no category |
| 7 | [3, 7] | add into category 7 directly |
| 9 (deleted) | [3, 7] | picker (default not found; not a special case in Kotlin) |
| 9 (deleted) | none | add with no category |

Note: the second row is the common case. The picker only appears when the user has categories and no default.

### 5.2 `change_favorite(a)` (add or remove)

Source: `BrowseAnimeSourceScreenModel.kt:225-245`.

```
new_fav = !a.favorite
date_added = if a.favorite { 0 } else { now_ms }
if !new_fav:                                     // remove
    cover: if cover_cache.delete_for(a) > 0 { cover_last_modified = now_ms }   // skipped for local source
    background: if background_cache.delete_for(a) > 0 { background_last_modified = now_ms }
else:                                             // add
    set_default_episode_flags(a)
    bind_enhanced_trackers(a, source)
write:
    Kotlin: full row (all fields of the snapshot, E-BR-02)   -> Kurobayomi: narrow patch (D-3)
```

Narrow patch written by Kurobayomi (D-3):

```rust
FavoritePatch {
    favorite: new_fav,
    date_added,
    cover_last_modified: cover_changed.then_some(now_ms),
    background_last_modified: background_changed.then_some(now_ms),
}
```

Result: exactly four columns are touched (`favorite`, `date_added`, and optionally the two timestamps). Titles, statuses and episode data keep their stored values (E-BR-02 fixed).

Failure: Kotlin returns `false` and writes an error log (`AnimeRepositoryImpl.kt:140-148`). Kurobayomi shows a snackbar and reverts the in-memory row (E-BR-12).

### 5.3 Category write ordering

Kotlin launches the category write and the favorite write without awaiting each other (`BrowseAnimeSourceScreenModel.kt:301-308`, RC-4). Kurobayomi: the DB worker runs the category write first and then the favorite patch, in one request, so the row never appears in the library before its categories.

## 6. Hide-in-library

Preference `browse_hide_in_anime_library_items` (default false), read once at construction (`BrowseAnimeSourceScreenModel.kt:108`). When true, items with `favorite == true` are removed after mapping (`BrowseAnimeSourceScreenModel.kt:121`). The paging count shrinks; the pager keeps loading.

Rust: apply the filter in the pager's `apply` step (drop favorites before appending), so the ListStore never contains them. Fixes the stale-favorite case (E-BR-11) only when the screen is recreated; a live change would need an explicit refresh (documented).

Blank list case (E-BR-21): with every item on the loaded pages filtered out and no more pages, Kotlin shows nothing (no empty message). PROPOSAL: show "No results found" with the same actions as the empty state.

## 7. Display modes and columns

- Display mode: global preference (`pref_display_mode_catalogue`), default `CompactGrid`. Stored strings `COMPACT_GRID`, `COMFORTABLE_GRID`, `COVER_ONLY_GRID`, `LIST` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`).
- `CoverOnlyGrid` renders as compact grid (part 1 `BrowseAnimeSourceScreen.kt:141`).
- Columns: landscape or portrait preference (default 0 = adaptive 128 px). Negative values are not used by Kotlin; PROPOSAL: values <= 0 mean adaptive (E-BR-23).

## 8. Last used and incognito

- On model construction (`BrowseAnimeSourceScreenModel.kt:100-102`): if not incognito, write `last_anime_catalogue_source = source.id`. This also runs for stub sources.
- Incognito: `basePreferences.incognitoMode()` OR the source's extension package is in `incognito_anime_extensions`.

## 9. Genre entry

Covered in `p2-topic-source-filters.md` section 4.

## 10. Tests

| Test | Covers |
|---|---|
| T-TEST-LISTING | `Listing::from_nav_arg`, value equality, `is_user_query`, Popular restart rule (E-BR-25) |
| T-TEST-PAGER-TERMINAL | every row of the page-1 and append truth tables (section 2) |
| T-TEST-NETWORK-TO-LOCAL | the three rows of section 3, including stored title kept (E-BR-10) |
| T-TEST-FAVORITE-WRITE | `change_favorite` patch contents for add, remove, local source (no cache calls), and cache count 0 (E-BR-02, RC-3) |
| T-TEST-DUPLICATE | decision tree of section 5 with duplicates present and absent (E-BR-13, E-BR-14) |
| T-TEST-CATEGORY-RESOLUTION | the six rows of section 5.1 table (E-BR-15) |
| T-TEST-NAV-UP | navigate-up rule with user query, toolbar text, neither (E-BR-05) |
