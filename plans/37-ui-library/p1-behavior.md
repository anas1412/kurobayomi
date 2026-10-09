# 37-ui-library — Behavior spec (part 1 of 1)

Scope: Library screens (anime and manga), grid/list rendering, categories, filters, sorting, search, selection, bulk actions, per-category settings, and the category management screens.

Source root for every path below: `/home/blackbox/aniyomi-src/` (repo-relative paths).

Notation:
- **OBSERVED** = what the Kotlin does (cited path:line).
- **Proposal:** = what the Rust port should do.
- Anime and manga are parallel. Unless stated, the manga implementation is the same as the anime one with the names swapped (`episode`→`chapter`, `seen`→`read`, `unseen`→`unread`, `anime`→`manga`). Every manga-specific difference is listed in §B-14.

---

## B-0. Glossary

| Term | Meaning |
|---|---|
| Library entry | A row of `animes` (or `mangas`) with `favorite = 1`. |
| LibraryAnime / LibraryManga | Entry plus aggregate counts and `category` (see `p1-data-and-config.md`). |
| AnimeLibraryItem / MangaLibraryItem | UI wrapper: `libraryAnime` + badge values (`downloadCount`, `unseenCount`/`unreadCount`, `isLocal`, `sourceLanguage`). |
| Category page | One pager page = one category. `categories` = `library.keys` in map order. |
| System category | Category with `id == 0` (`UNCATEGORIZED_ID`, `domain/src/main/java/tachiyomi/domain/category/model/Category.kt:16`). Its UI name is `label_default` ("Default"). |
| Selection | Ordered list `PersistentList<LibraryAnime>` of selected entries across all categories. |

---

## B-1. Tab entry points

Files: `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt`.

### B-1.1 Tab options
- Anime: `TabOptions(index = 0u, title = stringResource(AYMR.strings.label_anime_library), icon = rememberAnimatedVectorPainter(R.drawable.anim_animelibrary_leave, isSelected))` (`AnimeLibraryTab.kt:75-84`). Title text "Anime" (`label_anime_library`).
- Manga: `index = if (fromMore) 5u else 1u`, icon `R.drawable.anim_library_enter` (`MangaLibraryTab.kt:74-81`). `fromMore = currentNavigationStyle() == NavStyle.MOVE_MANGA_TO_MORE`. Title "Manga" (`label_manga_library`).
- Proposal: keep tab order = Anime(0), Manga(1), and when the manga library is moved into More (`NavStyle.MOVE_MANGA_TO_MORE`), it is item 5. Icons: GTK symbolic icons (see `p1-ui-parity.md`).

### B-1.2 Reselect
- `onReselect(navigator)` calls `requestOpenSettingsSheet()` which sends `Unit` on the rendezvous channel `requestSettingsSheetEvent` (`AnimeLibraryTab.kt:87-89, 306-307`). In `Content()` that channel is collected and calls `screenModel.showSettingsDialog()` (`AnimeLibraryTab.kt:297`). Net effect: reselecting the Library tab opens the Filter/Sort/Display sheet.
- Proposal: reselecting the sidebar item opens the settings dialog.

### B-1.3 External search injection
- `search(query)` sends the query on rendezvous channel `queryEvent` (`AnimeLibraryTab.kt:302-303`); collected in `LaunchedEffect(Unit)` → `screenModel::search` (`:296`). Other screens use this to pre-fill search.
- Proposal: a `LibraryCommand::Search(String)` message on the library controller.

### B-1.4 Composition order and state
1. `screenModel = rememberScreenModel { AnimeLibraryScreenModel() }`, `settingsScreenModel = rememberScreenModel { AnimeLibrarySettingsScreenModel() }`, `state by screenModel.state` (`:99-101`).
2. Scaffold: topBar (LibraryToolbar), bottomBar (LibraryBottomActionMenu), snackbarHost (`:122-177`).
3. Body `when`:
   - `state.isLoading` → `LoadingScreen(Modifier.padding(contentPadding))` (`:180`).
   - `state.searchQuery.isNullOrEmpty() && !state.hasActiveFilters && state.isLibraryEmpty` → `EmptyScreen(information_empty_library)` with action `getting_started_guide` (icon `HelpOutline`) that opens `GETTING_STARTED_URL` via `LocalUriHandler` (`:181-194`).
   - else → `AnimeLibraryContent(...)` (`:195-231`).
4. Dialogs: `when (state.dialog)` (`:237-276`). `SettingsSheet` requires a non-null current category; if null the dialog is dismissed (`:238-243`).
5. `BackHandler(enabled = state.selectionMode || state.searchQuery != null)`: if selectionMode → clearSelection; else if searchQuery != null → search(null) (`:278-283`). Note the `when` only handles one: selection first.
6. `LaunchedEffect(state.selectionMode, state.dialog) { HomeScreen.showBottomNav(!state.selectionMode) }` (`:285-287`). Bottom navigation hides in selection mode.
7. `LaunchedEffect(state.isLoading) { if (!isLoading) (context as? MainActivity)?.ready = true }` (`:289-293`). This releases the splash.

Proposal: hide the sidebar/bottom bar in selection mode (GTK: a toolbar-level action bar; the sidebar is not hidden, but the library page shows its action bar). Splash ready: `B-1.4-7` is Android-only; drop it (see task T-37a-052).

---

## B-2. Screen model construction and the data pipeline

File: `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt`.

### B-2.1 Dependencies (constructor, `:84-101`)
Interactors and services (all Injekt-resolved): `GetLibraryAnime`, `GetVisibleAnimeCategories`, `GetTracksPerAnime`, `GetNextEpisodes`, `GetEpisodesByAnimeId`, `SetSeenStatus`, `UpdateAnime`, `SetAnimeCategories`, `BasePreferences`, `LibraryPreferences`, `AnimeCoverCache`, `AnimeBackgroundCache`, `AnimeSourceManager`, `AnimeDownloadManager`, `AnimeDownloadCache`, `TrackerManager`.

Manga counterparts: `GetLibraryManga`, `GetNextChapters`, `GetChaptersByMangaId`, `SetReadStatus`, `MangaBackgroundCache` (not in manga, see §B-14).

### B-2.2 Active category index
`var activeCategoryIndex: Int by libraryPreferences.lastUsedAnimeCategory().asState(screenModelScope)` (`:103-105`). Persisted int, key `__APP_STATE_last_used_anime_category` (default 0). Manga key: `__APP_STATE_last_used_category` (`LibraryPreferences.kt:156-157`). It is written on every page change (`AnimeLibraryContent.kt:115-117`) and never validated against `categories.size` (see E-17).

### B-2.3 Init pipeline A — library content (`:108-135`)
`screenModelScope.launchIO { combine(...) { ... }.collectLatest { mutableState.update { copy(isLoading=false, library=it) } } }` with inputs:
1. `state.map { it.searchQuery }.debounce(SEARCH_DEBOUNCE_MILLIS)` — `SEARCH_DEBOUNCE_MILLIS = 250L` (`app/src/main/java/eu/kanade/presentation/components/AppBar.kt:65`).
2. `getLibraryFlow()` (§B-3).
3. `getTracksPerAnime.subscribe()` — `Map<Long, List<AnimeTrack>>` keyed by anime id.
4. `getTrackingFilterFlow()` (§B-4.3) — `Map<Long, TriState>` keyed by tracker id (only logged-in trackers).
5. `downloadCache.changes` — signal only; any emission recomputes.

Transform: `library.applyFilters(tracks, trackingFilter).applySort(tracks, trackingFilter.keys).mapValues { searchQuery != null ? filter{ it.matches(q) } : it }`.

Important: the search filter is applied **after** sorting, per category, and is skipped when `searchQuery == null`. An empty string `""` is not null, so `matches("")` runs (title contains "" is true, so every entry passes; see E-03).

`collectLatest` means a newer emission cancels an in-progress transform. Proposal: same, via a generation counter.

### B-2.4 Init pipeline B — view options (`:137-151`)
`combine(categoryTabs(), categoryNumberOfItems(), showContinueViewingButton())` → `showCategoryTabs`, `showAnimeCount`, `showAnimeContinueButton`. Each change emits immediately (no debounce).

### B-2.5 Init pipeline C — hasActiveFilters (`:153-175`)
`combine(getAnimelibItemPreferencesFlow(), getTrackingFilterFlow())` → `listOf(filterDownloaded, filterUnseen, filterStarted, filterBookmarked, filterCompleted, filterIntervalCustom) + trackFilter.values` → `.any { it != TriState.DISABLED }`, then `distinctUntilChanged()`.

Note: `filterDownloaded` here is the raw preference, not the `globalFilterDownloaded` override. So when downloaded-only mode forces the download filter on, `hasActiveFilters` can be false (see E-22).

### B-2.6 Item preferences flow (`getAnimelibItemPreferencesFlow`, `:342-374`)
11 inputs combined into `ItemPreferences` (private data class, `:738-753`):

| Field | Source | Type |
|---|---|---|
| downloadBadge | `display_download_badge` | Boolean (default false) |
| unseenBadge | `display_unread_badge` (shared) | Boolean (default true) |
| localBadge | `display_local_badge` | Boolean (default true) |
| languageBadge | `display_language_badge` | Boolean (default false) |
| skipOutsideReleasePeriod | `library_update_manga_restriction` contains `"manga_outside_release_period"` | Boolean |
| globalFilterDownloaded | `preferences.downloadedOnly()` = `__APP_STATE_pref_downloaded_only` (default false) | Boolean |
| filterDownloaded | `pref_filter_animelib_downloaded_v2` | TriState |
| filterUnseen | `pref_filter_animelib_unread_v2` | TriState |
| filterStarted | `pref_filter_animelib_started_v2` | TriState |
| filterBookmarked | `pref_filter_animelib_bookmarked_v2` | TriState |
| filterCompleted | `pref_filter_animelib_completed_v2` | TriState |
| filterIntervalCustom | `pref_filter_library_interval_custom` (shared) | TriState |

Manga names: `pref_filter_library_downloaded_v2`, `pref_filter_library_unread_v2`, `pref_filter_library_started_v2`, `pref_filter_library_bookmarked_v2`, `pref_filter_library_completed_v2` (`LibraryPreferences.kt:110-138`).

Note the `combine` with `transform` — anime uses the `transform = { ... it[n] as T }` form; the manga `combine` uses the trailing-lambda form. Same values.

### B-2.7 Tracking-filter flow (`getTrackingFilterFlow`, `:423-436`)
`trackerManager.loggedInTrackersFlow().flatMapLatest { loggedIn -> if empty → flowOf(emptyMap()); else combine(loggedIn.map { libraryPreferences.filterTrackedAnime(tracker.id.toInt()).changes() }) { map tracker.id → value } }`.

Keys: `pref_filter_animelib_tracked_${id}_v2` (anime) and `pref_filter_library_tracked_${id}_v2` (manga) (`LibraryPreferences.kt:140-144`). Default `TriState.DISABLED`.

---

## B-3. Library map construction (`getLibraryFlow`, `:379-416`)

1. `animelibAnimesFlow = combine(getLibraryAnime.subscribe(), getAnimelibItemPreferencesFlow(), downloadCache.changes) { list, prefs, _ -> list.map { ... }.groupBy { it.libraryAnime.category } }`.
2. Each `AnimeLibraryItem` is built as:
   - `downloadCount = if (prefs.downloadBadge) downloadManager.getDownloadCount(anime).toLong() else 0`.
   - `unseenCount = if (prefs.unseenBadge) animelibAnime.unseenCount else 0`.
   - `isLocal = if (prefs.localBadge) anime.isLocal() else false`.
   - `sourceLanguage = if (prefs.languageBadge) sourceManager.getOrStub(anime.source).lang else ""`.
   (`:387-402`.)
3. `groupBy { it.libraryAnime.category }` — a `LibraryAnime` with `category = 3` lands in key 3. The `category` column is `coalesce(MC.category_id, 0)`, so uncategorized entries go to key 0 (`animelibView.sq:32`). One entry in N categories is emitted N times (one per `animes_categories` row; see E-11).
4. `combine(getCategories.subscribe(), animelibAnimesFlow) { categories, map -> displayCategories.associateWith { map[it.id].orEmpty() } }` (`:407-415`):
   - `displayCategories = if (map.isNotEmpty() && !map.containsKey(0)) categories.filterNot(isSystemCategory) else categories`.
   - `categories` = `GetVisibleAnimeCategories.subscribe()` = visible categories only (`hidden = 0`), **including** the system category row `id = 0` (seeded row: `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)`, `categories.sq:10`).
   - Rule in words: the Default page is shown when there are entries in it, or when the library is empty. When the library has entries but none is uncategorized, the Default page is hidden.
5. The map key order is the `categories` order (`associateWith` preserves it). The DB order is `ORDER BY sort` (`categories.sq:44`). Note the system category has `sort = -1`, so it sorts first when present.

Result type: `AnimeLibraryMap = Map<Category, List<AnimeLibraryItem>>` (`:82`). The map is an ordered `LinkedHashMap`; all later stages depend on that order.

Entries in **hidden** categories do not appear: they are not in `displayCategories` (`getAllVisibleAnimeCategories`), so they are invisible in the library and not counted (E-08).

---

## B-4. Filters

### B-4.1 TriState semantics
`TriState` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`): `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`. `next()` cycles `DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED`.

`applyFilter(filter, predicate)` (`domain/src/main/java/tachiyomi/domain/entries/TriState.kt:5-9`):
- DISABLED → true.
- ENABLED_IS → predicate().
- ENABLED_NOT → !predicate().

### B-4.2 Filter pipeline (`applyFilters`, `:178-254`)
Input: the library map, tracks map, tracking filter map. Output: each category list filtered (`fastFilter`), order preserved.

Per-entry predicate = AND of:
1. `filterFnDownloaded`: `applyFilter(filterDownloaded) { anime.isLocal() || it.downloadCount > 0 || downloadManager.getDownloadCount(anime) > 0 }` (`:198-204`).
   - `filterDownloaded` = `if (globalFilterDownloaded) ENABLED_IS else prefs.filterDownloaded` (`:185`).
   - Note: the predicate calls the download manager directly, so it works even when the downloaded badge is off (`downloadCount` is 0 in that case).
   - Local entries always count as downloaded.
2. `filterFnUnseen`: `applyFilter(filterUnseen) { libraryAnime.unseenCount > 0 }` (`:206-208`). Uses the raw `unseenCount` (not the badge-gated item value).
3. `filterFnStarted`: `hasStarted` (`seenCount > 0`).
4. `filterFnBookmarked`: `hasBookmarks` (`bookmarkCount > 0`).
5. `filterFnCompleted`: `anime.status.toInt() == SAnime.COMPLETED` where `SAnime.COMPLETED = 2` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:74`). Manga: `SManga.COMPLETED = 2` (`SManga.kt:50`).
6. `filterFnIntervalCustom`: if `skipOutsideReleasePeriod` → `applyFilter(filterIntervalCustom) { anime.fetchInterval < 0 }`; else `true` (the filter is ignored when the restriction is off, even if the preference is set) (`:222-228`).
7. `filterFnTracking` (`:230-241`):
   - If `trackingFilter.isEmpty()` (no logged-in tracker) or both include/exclude lists are empty → true.
   - `excludedTracks = trackingFilter.filter { ENABLED_NOT }.keys`; `includedTracks = filter { ENABLED_IS }.keys`.
   - `animeTracks = trackMap.mapValues { it.value.map { t -> t.trackerId } }[item.id].orEmpty()`.
   - `isExcluded = excludedTracks.isNotEmpty() && animeTracks.any { it in excludedTracks }`.
   - `isIncluded = includedTracks.isEmpty() || animeTracks.any { it in includedTracks }`.
   - Result: `!isExcluded && isIncluded`.
   - Note: `trackMap.mapValues { ... }` rebuilds the whole map for every item (O(n²)); see T-37a-054.

Per category: `value.fastFilter(filterFn)`.

### B-4.3 Filter toggling (`AnimeLibrarySettingsScreenModel.toggleFilter`, `AnimeLibrarySettingsScreenModel.kt:38-46`)
`preference(libraryPreferences).getAndSet { it.next() }`. Tap cycles DISABLED→IS→NOT→DISABLED. Writes are immediate and the library pipeline reacts to the preference change.

### B-4.4 hasActiveFilters
See §B-2.5. Drives the filter icon tint (`LibraryToolbar.kt:100`: active color when true) and the empty-state message ("No match found" vs "Category is empty", §B-9).

### B-4.5 Filter rows in settings (§B-12.1).

---

## B-5. Sorting

### B-5.1 Stored sort (per category or global)
`Category.sort` (`AnimeLibrarySortMode.kt:144`): `AnimeLibrarySort.valueOf(this?.flags)` — `null` category → `default` = `(Alphabetical, Ascending)`.

Flag layout is in `p1-data-and-config.md` §D-4 (the mask/shift table). In short: type in bits 2–5 (mask `0b00111100`), direction in bit 6 (`0b01000000`, set = Ascending).

**Observed subtlety:** a category row whose `flags` is `0` decodes to `(Alphabetical, Descending)`, because flag `0` has direction bit clear = Descending. The system category row is seeded with `flags = 0` (`categories.sq:10`), so the Default page sorts Z→A until the user changes it. Rust must reproduce this (or consciously change the seed; see T-37a-002).

### B-5.2 Sort application (`applySort`, `:256-340`)
1. `sortAlphabetically(i1, i2) = i1.title.lowercase().compareToWithCollator(i2.title.lowercase())` (`:260-262`). `compareToWithCollator` is `tachiyomi.core.common.util.lang.compareToWithCollator` (locale collator; see §B-5.4).
2. `trackerScores` is lazy (only when TrackerMean is selected); `defaultTrackerScoreSortValue = -1.0` (`:264`).
3. For each category key: if `key.sort.type == Random` → `value.shuffled(Random(libraryPreferences.randomAnimeSortSeed().get()))`, **no** tiebreak and no direction (`:329-332`).
4. Otherwise `comparator = key.sort.comparator().let { if (asc) it else it.reversed() }.thenComparator(sortAlphabetically)` (`:334-336`). The alphabetical tiebreak is always ascending, regardless of direction.

### B-5.3 Comparators (exact, `:278-327`)
Let `i1`, `i2` be items; `la = i.libraryAnime`.

| Type | Comparison (ascending sense before reversal) |
|---|---|
| Alphabetical | `sortAlphabetically(i1, i2)` (lowercase, collator). |
| LastSeen | `la.lastSeen.compareTo(...)` (Long). |
| LastUpdate | `la.anime.lastUpdate.compareTo(...)`. `lastUpdate` is `Long` (nullable in the DB, mapped via `mapLibraryAnime` lastUpdate param). |
| UnseenCount | Special, below. |
| TotalEpisodes | `la.totalCount.compareTo(...)`. |
| LatestEpisode | `la.latestUpload.compareTo(...)`. |
| EpisodeFetchDate | `la.episodeFetchedAt.compareTo(...)`. |
| DateAdded | `la.anime.dateAdded.compareTo(...)`. |
| TrackerMean | `(trackerScores[id] ?: -1.0).compareTo(...)`. See B-5.5. |
| AiringTime | Special, below. |
| Random | `error("Why Are We Still Here? Just To Suffer?")` (unreachable; handled in B-5.2-3). |

**UnseenCount** (`:289-295`):
```
when {
  u1 == u2                -> 0
  u1 == 0L                -> if (asc) 1 else -1
  u2 == 0L                -> if (asc) -1 else 1
  else                    -> u1.compareTo(u2)
}
```
Because the outer `reversed()` is applied for descending, the effect is: **entries with zero unseen count always sort last**, in both directions. The source comment ("Ensure unseen content comes first") is misleading; the observable behavior is "zero last".

**AiringTime** (`:313-322`): primary key is `unseenCount` (not airing time); then `nextEpisodeAiringAt`:
```
when {
  u1 != u2                              -> u1.compareTo(u2)
  n1 == n2                              -> 0
  n1 == 0L                              -> if (asc) 1 else -1
  n2 == 0L                              -> if (asc) -1 else 1
  else                                  -> n1.compareTo(n2)
}
```
Same zero-last rule for `nextEpisodeAiringAt == 0`. Manga has no AiringTime (§B-14).

### B-5.4 Collation
`compareToWithCollator` (`core/common/src/main/java/tachiyomi/core/common/util/lang/...`, declared outside scope) uses the platform `java.text.Collator` for the device locale. Proposal: `icu` collation (rust `icu_collator` or `unicode-collation`), locale from `LANG`; fallback: `to_lowercase` + Unicode default ordering. The title is lowercased first, then collated.

### B-5.5 TrackerMean (`:265-276`)
```
trackerScores = lazy { trackerMap = trackerManager.getAll(loggedInTrackerIds).associateBy(id)
  trackMap.mapValues { entry ->
     if entry.value.isEmpty() -> null
     else entry.value.mapNotNull { trackerMap[it.trackerId]?.animeService?.get10PointScore(it) }.average()
  } }
```
- `trackerMap` only contains logged-in trackers.
- `average()` of an empty list is `NaN` in Kotlin. If an entry has tracks but none from a logged-in tracker, the score is `NaN`, not `null`, so `?: -1.0` does not apply. `NaN.compareTo(x)` in Kotlin's `Double` total order places NaN **above** every number, so such entries sort **last** in ascending order (E-14). Proposal: treat "no logged-in track scores" as `-1.0` consistently (T-37a-058).
- `get10PointScore` is per-tracker (`app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt:43`, `Anilist.kt:141`), converting each tracker's native score onto 0–10.

### B-5.6 Random
- `Random` is a sort type with a persisted int seed: `library_random_anime_sort_seed` (default 0; manga `library_random_manga_sort_seed`).
- The seed changes **only** when a Random sort is chosen (`SetSortModeForAnimeCategory.await`: `if (type == Random) randomAnimeSortSeed().set(Random.nextInt())`, `domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetSortModeForAnimeCategory.kt:23-25`).
- Same seed for every category that is Random-sorted. Two Random categories therefore shuffle with the same RNG seed (different lists, same generator state).
- Re-selecting Random (via the refresh icon) draws a new seed → reshuffles.

### B-5.7 Setting a sort (`SetSortModeForAnimeCategory`, `:16-37`)
Inputs: `categoryId?`, `type`, `direction`. Algorithm:
1. `category = categoryId?.let { getAnimeCategory(it) }`.
2. `flags = (category?.flags ?: 0) + type + direction` (uses `Long.plus(Flag)` masked merge, `domain/.../library/model/Flag.kt:21-27`).
3. If `type == Random` → reseed.
4. If `category != null && categorizedDisplaySettings()` (pref `categorized_display`, default false) → `updatePartialAnimeCategory(id, flags)` (only this category changes).
5. Else → `animeSortingMode().set(AnimeLibrarySort(type, direction))` (pref `animelib_sorting_mode`) **and** `updateAllAnimeCategoryFlags(flags)` — every category row gets the same flags, including the system category.

Observed consequence: with categorized display off (the default), changing the sort on any page changes the sort of every page. The `flags` computed from the current category are merged onto the category's own flags, but since every category is overwritten with that value, the other categories' bits are lost (E-18).

### B-5.8 Direction toggling in the UI (`AnimeLibrarySettingsDialog.kt:163-225`)
- Options list (see §B-12.2).
- Clicking the currently sorted option toggles direction.
- Clicking a different option keeps the current direction: `direction = if (sortDescending) Descending else Ascending` (`:215-219`).
- Random: `BaseSortItem` with refresh icon when selected; click always calls `setSort(category, Random, Ascending)` (`:192-202`).

### B-5.9 Sort-related settings writes
`AnimeLibrarySettingsScreenModel.setSort` launches on IO: `setSortModeForCategory.await(category, mode, direction)` (`AnimeLibrarySettingsScreenModel.kt:52-60`). `category` is `state.categories[activeCategoryIndex]` as passed from the Tab (`AnimeLibraryTab.kt:238-248`); the dialog's sort page uses the same category (`AnimeLibrarySettingsDialog.kt:66-69`).

---

## B-6. Search

### B-6.1 Entry
- Toolbar search: `SearchToolbar(searchQuery, onChangeSearchQuery = onSearchQueryChange)` → `screenModel.search(query)` → `mutableState.update { searchQuery = query }` (`AnimeLibraryScreenModel.kt:691-693`). `null` clears.
- Debounce: 250 ms on the `state.map { searchQuery }` input (B-2.3).
- Search state is not persisted.

### B-6.2 Matching (`AnimeLibraryItem.matches`, `AnimeLibraryItem.kt:23-39`)
```
fun matches(constraint: String): Boolean {
  val sourceName by lazy { sourceManager.getOrStub(anime.source).getNameForAnimeInfo() }
  if (constraint.startsWith("id:", ignoreCase = true)) {
     val id = constraint.substringAfter("id:").toLongOrNull()
     return libraryAnime.id == id
  }
  return title.contains(constraint, true)
     || (author?.contains(constraint, true) ?: false)
     || (artist?.contains(constraint, true) ?: false)
     || (description?.contains(constraint, true) ?: false)
     || constraint.split(",").map { it.trim() }.all { sub ->
          checkNegatableConstraint(sub) {
             sourceName.contains(it, true) || (genre?.any { g -> g.equals(it, true) } ?: false)
          }
        }
}

private fun checkNegatableConstraint(c, predicate) =
   if (c.startsWith("-")) !predicate(c.substringAfter("-").trimStart()) else predicate(c)
```

Rules, in order:
1. Prefix `id:` (case-insensitive) → exact id match only. `"id:abc"` → `toLongOrNull()` = null → `id == null` → false. `"id:"` alone matches nothing.
2. Whole-string substring on title, author, artist, description (case-insensitive). A comma-separated query with a title containing a comma still matches via this path.
3. Otherwise every comma-separated token (trimmed) must match, each token is either:
   - a plain term: source name contains it (case-insensitive) **or** a genre equals it (case-insensitive exact);
   - a negated term `-x`: NOT(source name contains `x`, or genre equals `x`). Note `substringAfter("-")` returns everything after the first `-`, then `trimStart()`.
4. The OR chain means tokens are AND-ed only inside step 3; step 2 short-circuits the whole query.

Edge behaviors are catalogued in the edge-case file (E-03 to E-06).

Proposal: keep these semantics exactly for compatibility with users' muscle memory. Collator-insensitive "contains ignore case" (`to_lowercase` on both sides) is acceptable; full Unicode case folding is an improvement, not a requirement.

### B-6.3 Search effect on UI
- Pager empty state: `!searchQuery.isNullOrEmpty()` → message `no_results_found` + GlobalSearchItem (§B-9).
- Category counts: `getAnimeCountForCategory(c) = if (showAnimeCount || !searchQuery.isNullOrEmpty()) library[c]?.size else null` (`:788-790`). Searching always shows per-tab counts.
- Tabs visibility while searching: `showPageTabs = state.showCategoryTabs || !state.searchQuery.isNullOrEmpty()` (`AnimeLibraryTab.kt:203`), so searching shows tabs even when the tabs option is off.
- Toolbar scroll behavior: `tabVisible = state.showCategoryTabs && state.categories.size > 1` (`:129`), which ignores the search-forced tabs (see E-19).

### B-6.4 Global search row
- Shown as the first item of the grid/list when `searchQuery` is non-empty: `GlobalSearchItem` = `TextButton` with text `action_global_search_query` = `Search for "%1$s" globally`. Click → `navigator.push(GlobalAnimeSearchScreen(searchQuery ?: ""))` (`AnimeLibraryTab.kt:219-223`).
- In the grid it spans the full row (`GridItemSpan(maxLineSpan)`, `LazyLibraryGrid.kt:36-43`).

---

## B-7. Categories: pages, tabs, toolbar title and counts

### B-7.1 Page model
- Pager state: `rememberPagerState(coercedCurrentPage) { categories.size }` (`AnimeLibraryContent.kt:57-58`), `coercedCurrentPage = currentPage().coerceAtMost(categories.lastIndex)` evaluated once (`remember { }` without keys, `:57`).
- Off-screen composition: only pages `current-1 .. current+1` are composed (`AnimeLibraryPager.kt:55-58`). Other pages are blank.
- Page change → `onChangeCurrentPage(pagerState.currentPage)` → `activeCategoryIndex` is written (B-2.2).
- `LaunchedEffect(categories)`: if `categories.size <= pagerState.currentPage` → `scrollToPage(categories.size - 1)` (`:64-68`), only when tabs are shown.

### B-7.2 Tab strip
- Shown when `showPageTabs && categories.size > 1` (`AnimeLibraryContent.kt:63`).
- `LibraryTabs` (`LibraryTabs.kt:17-52`): `PrimaryScrollableTabRow(selectedTabIndex = currentPageIndex, edgePadding = 0.dp, divider = {})`; each `Tab(selected, onClick = onTabItemClick(index), text = TabText(text = category.visualName, badgeCount = getNumberOfItemsForCategory(category)), unselectedContentColor = onSurface)`; followed by `HorizontalDivider()`. Tab zIndex 1f.
- `visualName`: system → `label_default` ("Default"), else `name` (`CategoryExtensions.kt:10-15`).
- `getNumberOfItemsForCategory` = `getAnimeCountForCategory` (B-6.3).
- Tab click → `pagerState.animateScrollToPage(index)`.

### B-7.3 Toolbar title (`getToolbarTitle`, `AnimeLibraryScreenModel.kt:792-810`)
```
category = categories.getOrNull(page) ?: return LibraryToolbarTitle(defaultTitle)
categoryName = if (system) defaultCategoryTitle else name   // defaultCategoryTitle = label_default
title = if (showCategoryTabs) defaultTitle else categoryName  // defaultTitle = label_anime_library ("Anime")
count = when {
  !showAnimeCount -> null
  !showCategoryTabs -> getAnimeCountForCategory(category)     // null unless showAnimeCount || searching
  else -> libraryCount                                        // distinct entries across all categories
}
```
`libraryCount` = distinct `anime.id` across all page lists (`:767-772`). `isLibraryEmpty` = `libraryCount == 0` (`:774`).

Note `page` = `activeCategoryIndex`. If the index is out of range, the default title is used with no count (E-17).

### B-7.4 Category counts and pill
- `LibraryToolbarTitle(text, numberOfEntries)`; the toolbar draws a `Pill` with `numberOfEntries` at 14 sp, `onBackground` at alpha 0.12 (dark) / 0.08 (light) (`LibraryToolbar.kt:78-94`).

---

## B-8. Item rendering and interactions

### B-8.1 Layout modes
`LibraryDisplayMode` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`): `CompactGrid` (default), `ComfortableGrid`, `List`, `CoverOnlyGrid`. Shared between anime and manga (key `pref_display_mode_library`, §D-3). Per-page lookup: `getDisplayMode(page)` returns the same preference state for every page (`AnimeLibraryScreenModel.kt:580-582`), so the mode is global.

Pager dispatch (`AnimeLibraryPager.kt:76-120`):
- `List` → `AnimeLibraryList(entries = columns, containerHeight = containerHeightPx)`.
- `CompactGrid`, `CoverOnlyGrid` → `AnimeLibraryCompactGrid(showTitle = displayMode is CompactGrid)`.
- `ComfortableGrid` → `AnimeLibraryComfortableGrid`.

`columns` = `getColumnsForOrientation(isLandscape)` (portrait key `pref_animelib_columns_portrait_key`, landscape key `pref_animelib_columns_landscape_key`, default 0 = auto) (`AnimeLibraryPager.kt:73-74`). `remember(isLandscape)`.

### B-8.2 Grid geometry (`LazyLibraryGrid.kt:14-29`)
- Columns: `columns == 0 → GridCells.Adaptive(128.dp)`, else `GridCells.Fixed(columns)`.
- `contentPadding + PaddingValues(8.dp)`.
- Spacing: `CommonEntryItemDefaults.GridVerticalSpacer = 4.dp`, `GridHorizontalSpacer = 4.dp`.
- Scrollbar: `FastScrollLazyVerticalGrid`.

### B-8.3 Compact grid item (`EntryCompactGridItem`, `CommonEntryItem.kt:73-119`)
- Wrapper `GridItemSelectable`: clip `MaterialTheme.shapes.small`, `combinedClickable(onClick, onLongClick)`, selected outline (secondary color rect behind), padding 4 dp, content color `onSecondary` when selected.
- Cover: `ItemCover.Book` (ratio 2/3, `app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt:18-20`), width fill, alpha `0.76` when selected else `coverAlpha = 1f`.
- Title overlay when `title != null` (compact grid only): bottom gradient `0f → Transparent`, `1f → #AA000000` over `fillMaxHeight(0.33f)`, clipped bottom corners 4 dp. Title: `titleSmall` white with shadow (black, blur 4), `minLines = 1`, `maxLines = 2`, `fontSize 12 sp`, `lineHeight 18 sp`, padding 8 dp, `Ellipsis`.
- Continue button when `onClickContinueViewing != null`:
  - with title → small (28 dp, icon 16 dp), bottom-end padding 6 dp (inside the overlay row).
  - cover-only (title null) → large (32 dp, icon 20 dp), padding 6 dp, aligned BottomEnd.
- Badges: start (top-start) = download count + unread count; end (top-end) = language/local. Badge group padding 4 dp.

### B-8.4 Cover-only grid
Same as compact grid with `title = null`; the title overlay is not drawn and the large continue button is used (§B-8.3).

### B-8.5 Comfortable grid (`EntryComfortableGridItem`, `CommonEntryItem.kt:178-229`)
- Column: cover (ratio 2/3, `ItemCover.Book`) then a title below.
- Title: `fontSize 12 sp`, `lineHeight 18 sp`, `minLines = 2`, `maxLines = titleMaxLines` (default 2), `titleSmall` style, padding 4 dp.
- Continue button: large (32 dp), BottomEnd of the cover, padding 6 dp.
- Badges start: download + unread; end: language/local.

### B-8.6 List item (`EntryListItem`, `CommonEntryItem.kt:333-389`)
- Row height: `entries == 0 → 76 dp`; else `(containerHeight / entries).toDp() - (3 / entries).dp` (Int division; see E-23).
- Row padding: horizontal 16 dp, vertical 3 dp. Vertical centering.
- Selected background: `selectedBackground(isSelected)`: secondary at alpha 0.22 (light) / 0.16 (dark) (`presentation-core/.../util/Modifier.kt:27-36`).
- Cover: fill height, alpha `coverAlpha = 1f`.
- Title: `bodyMedium`, `weight(1f)`, padding horizontal 16 dp, Ellipsis (one line; no maxLines, so it may wrap, see parity doc).
- Badges: `BadgeGroup(content = badge)` (download, unread, language/local in that order).
- Continue: small (28 dp), start padding 8 dp, after badges.
- Global search row precedes items (`item { if (!searchQuery.isNullOrEmpty()) GlobalSearchItem(...) }`, `AnimeLibraryList.kt:39-47`).

### B-8.7 Badges (`LibraryBadges.kt:12-48`)
- `DownloadsBadge(count)`: shown when `count > 0`; `Badge(text = "$count", color = tertiary, textColor = onTertiary)`.
- `UnviewedBadge(count)` (the name is "Unviewed" for both kinds): shown when `count > 0`; `Badge(text = "$count")` with default secondary/onSecondary.
- `LanguageBadge(isLocal, sourceLanguage)`:
  - `isLocal` → `Badge(imageVector = Icons.Outlined.Folder, color = tertiary, iconColor = onTertiary)`.
  - else if `sourceLanguage.isNotEmpty()` → `Badge(text = sourceLanguage.uppercase(), tertiary/onTertiary)`.
  - else nothing.
- Badge metrics (`presentation-core/.../components/Badges.kt:38-57`): padding h 3 dp / v 1 dp, `bodySmall`, `FontWeight.Medium`, `maxLines = 1`, no shape (rectangle). Group: `Row` clipped with `extraSmall` shape.
- Badge values come from `AnimeLibraryItem` (B-3): download count is `0` when `downloadBadge` is off; unseen count is `0` when `unseenBadge` is off; `isLocal` and `sourceLanguage` are blanked when their toggles are off.

### B-8.8 Click and long-press
- Click (`onClickAnime`, `AnimeLibraryContent.kt:77-83`):
  - selection empty → `onAnimeClicked(anime.id)` → `navigator.push(AnimeScreen(id))`.
  - selection non-empty → `onToggleSelection(anime)`.
- Long-press (`onLongClick = onToggleRangeSelection`, `AnimeLibraryPager.kt` → `AnimeLibraryContent` → `Tab`): `toggleRangeSelection(anime)` + `HapticFeedbackType.LongPress` (`AnimeLibraryTab.kt:214-217`). See §B-10.2 for the range algorithm.
- Badge/continue button clicks are independent (they consume their own click).
- Cover clicks in the Android UI are the item click (no separate cover action).

### B-8.9 Continue button visibility (`onClickContinueViewing`)
Built in each item:
```
onClickContinueViewing = if (onClickContinueWatching != null && libraryItem.unseenCount > 0) { ... } else null
```
`libraryItem.unseenCount` is the badge-gated value (B-3), so **the button disappears when the unread badge is off** (E-02). `onClickContinueWatching` is `null` unless `showAnimeContinueButton` is on (`AnimeLibraryTab.kt:212`).

### B-8.10 Continue action (`onContinueWatchingClicked`, `AnimeLibraryTab.kt:206-212`)
- `scope.launchIO { val episode = screenModel.getNextUnseenEpisode(it.anime); if (episode != null) openEpisode(episode) }`.
- `getNextUnseenEpisode(anime)` = `getEpisodesByAnimeId.await(anime.id).getNextUnseen(anime, downloadManager)` (`AnimeLibraryScreenModel.kt:450-452`).
- `getNextUnseen` (`app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt:12-20`): `applyFilters(anime, downloadManager)` (per-entry episode filter/sort from `eu.kanade.domain.items.episode.model`, out of scope, area 07) then `if (anime.sortDescending()) findLast { !seen } else find { !seen }`.
- `openEpisode`: `playerPreferences.alwaysUseExternalPlayer()` (`pref_always_use_external_player`, default false) → `MainActivity.startPlayerActivity(context, episode.animeId, episode.id, extPlayer)`.
- If no episode → nothing happens (no message; E-25).

### B-8.11 Manga continue
- `onContinueReadingClicked` → `getNextUnreadChapter(manga)` = `getChaptersByMangaId.await(id, applyScanlatorFilter = true).getNextUnread(manga, downloadManager)`.
- If found → `context.startActivity(ReaderActivity.newIntent(context, chapter.mangaId, chapter.id))`.
- If not → `snackbarHostState.showSnackbar(context.stringResource(MR.strings.no_next_chapter))` ("Next chapter not found").

---

## B-9. Pager empty states (`LibraryPagerEmptyScreen`, `AnimeLibraryPager.kt:126-159`)

Message selection:
```
msg = when {
  !searchQuery.isNullOrEmpty() -> no_results_found      // "No results found"
  hasActiveFilters             -> error_no_match        // "No match found"
  else                         -> information_no_manga_category  // "Category is empty"
}
```
(The "no manga" key is used for both anime and manga; there is no `information_no_anime_category`.)

Layout: scrollable column with `contentPadding + 8 dp`, optional `GlobalSearchItem` when searching (centered), then `EmptyScreen(stringRes = msg, Modifier.weight(1f))`.

Library-wide empty (`AnimeLibraryTab.kt:181-194`): shown only when `searchQuery` is empty AND `!hasActiveFilters` AND `isLibraryEmpty`. Message `information_empty_library` ("Your library is empty") + action `getting_started_guide` → `GETTING_STARTED_URL`. In every other case the content is shown and the pager shows the empty-state messages above.

---

## B-10. Selection

### B-10.1 State (`AnimeLibraryScreenModel.kt:610-689`)
- `selection: PersistentList<LibraryAnime>`; `selectionMode = selection.isNotEmpty()` (`:776`).
- Selection is keyed by `anime.id` for membership checks, but stores the `LibraryAnime` snapshot (data class equality over all counts).
- Not cleared on page change; cleared after bulk actions (mark, download, delete, category change), by the cancel button, or by Back.

### B-10.2 Toggle (`toggleSelection`, `:614-625`)
`if (any { id == anime.id }) removeAll { id == anime.id } else add(anime)`.

### B-10.3 Range (`toggleRangeSelection`, `:631-659`)
```
lastSelected = list.lastOrNull()
if (lastSelected?.category != anime.category) { list.add(anime); return }      // different category or none → single add
items = state.getAnimelibItemsByCategoryId(anime.category)?.map { it.libraryAnime } ?: []
lastIdx = items.indexOf(lastSelected)        // data-class equality
curIdx  = items.indexOf(anime)
selectedIds = list.map { id }
range = when {
  lastIdx < curIdx -> lastIdx..curIdx
  curIdx < lastIdx -> curIdx..lastIdx
  else -> return                             // same index: nothing
}
list.addAll(range.mapNotNull { items[it].takeUnless { it.id in selectedIds } })
```
Notes:
- Range never deselects, and it only spans one category (the category of the **anime** that was long-pressed).
- `items` is the post-filter, post-search, post-sort list of that category.
- `indexOf` uses data-class equality (`LibraryAnime` all fields). A stale `lastSelected` snapshot (counts changed since it was selected) yields `-1`, and `items[-1]` throws (E-09). Proposal: compare by id.

### B-10.4 Select all (`selectAll(index)`, `:661-674`)
`categoryId = categories.getOrNull(index)?.id ?: -1`; appends every item in that category whose id is not already selected. Index is the active page (`screenModel.activeCategoryIndex`).

### B-10.5 Invert (`invertSelection(index)`, `:676-689`)
`categoryId = categories[index].id` (unchecked index, E-17). Partition the category's items into selected and not; remove the selected ones and add the unselected ones (in list order).

### B-10.6 Selection toolbar (`LibrarySelectionToolbar`, `LibraryToolbar.kt:130-157`)
- Title: the count as text.
- Actions: `action_select_all` (SelectAll icon) → `selectAll(activePage)`; `action_select_inverse` (FlipToBack icon) → `invertSelection(activePage)`.
- Cancel (action mode close) → `clearSelection()`.
- The regular toolbar is replaced entirely while `selectedCount > 0` (`LibraryToolbar.kt:45-50`).

### B-10.7 Selection bottom bar (`LibraryBottomActionMenu`, declared at `app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt:292`, area 36-ui-shell; wiring in `AnimeLibraryTab.kt:165-176`)
Visible while `selectionMode`. Buttons in wiring order:
1. Change category → `openChangeCategoryDialog`.
2. Mark viewed/read → `markSeenSelection(true)`.
3. Mark unviewed/unread → `markSeenSelection(false)`.
4. Download → `runDownloadActionSelection`, **only when** `selection.fastAll { !isLocal }` (`takeIf`); otherwise the button is absent.
5. Delete → `openDeleteAnimeDialog`.
`isManga = false` (anime) / `true` (manga) only changes labels.

---

## B-11. Bulk actions

### B-11.1 Mark seen/read (`markSeenSelection(seen)`, `:508-519`)
```
animes = state.value.selection.toList()
launchNonCancellable { animes.forEach { setSeenStatus.await(anime = it.anime, seen = seen) } }
clearSelection()
```
The clear runs immediately after the coroutine is launched, not after completion. The domain interactor `SetSeenStatus` (area 07) updates episodes and history; it is not re-checked here.

Manga: `markReadSelection(read)` with `setReadStatus.await(manga, read)` (`MangaLibraryScreenModel.kt:494-502`).

### B-11.2 Download next N (`runDownloadActionSelection(action)`, `:466-477`)
| Action | amount |
|---|---|
| NEXT_1_ITEM | 1 |
| NEXT_5_ITEMS | 5 |
| NEXT_10_ITEMS | 10 |
| NEXT_25_ITEMS | 25 |
| UNVIEWED_ITEMS | null (all) |

`downloadUnseenEpisodes(animes, amount)` (`:485-503`), inside `launchNonCancellable`, for each anime:
1. `episodes = getNextEpisodes.await(anime.id)` (domain, area 06/07: the next N episodes to fetch, ordering is in the domain).
2. Remove any episode that is already queued (`downloadManager.getQueuedDownloadOrNull(episode.id) != null`) **or** already downloaded (`downloadManager.isEpisodeDownloaded(episode.name, episode.scanlator, anime.title, anime.source)`).
3. `.take(amount)` when `amount != null`.
4. `downloadManager.downloadEpisodes(anime, episodes)`.

Then `clearSelection()`. Manga counterpart: `downloadUnreadChapters`, `getNextChapters`, `isChapterDownloaded`, `downloadChapters` (`MangaLibraryScreenModel.kt:474-486`).

Download labels (`DownloadAction` enum, `app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt`, area 36): NEXT_1_ITEM, NEXT_5_ITEMS, NEXT_10_ITEMS, NEXT_25_ITEMS, UNVIEWED_ITEMS. The `LibraryBottomActionMenu` presents them as a menu.

### B-11.3 Change categories
See §B-13 (the membership algorithm is in its own topic file).

### B-11.4 Delete from library (`removeAnimes`, `:528-553`)
Dialog: `DeleteLibraryEntryDialog` (B-12.5). Confirm → `removeAnimes(list, deleteFromLibrary, deleteEpisodes)`; the caller closes the dialog and clears the selection (`AnimeLibraryTab.kt:264-272`).
```
animeToDelete = animeList.distinctBy(id)
launchNonCancellable {
  if (deleteFromLibrary) {
     toDelete = animeToDelete.map {
        it.removeCovers(coverCache)          // side effect: deletes cached cover; result discarded
        it.removeBackgrounds(backgroundCache)// side effect: deletes cached background; result discarded
        AnimeUpdate(favorite = false, id = it.id)
     }
     updateAnime.awaitAll(toDelete)
  }
  if (deleteEpisodes) {
     animeToDelete.forEach { anime ->
        val source = sourceManager.get(anime.source) as? AnimeHttpSource
        if (source != null) downloadManager.deleteAnime(anime, source)
     }
  }
}
```
- `removeCovers`/`removeBackgrounds` return a copy with `coverLastModified` updated, but the copy is discarded, so the DB keeps the old timestamp (E-27).
- Local-source entries: `LocalAnimeSource` is not an `AnimeHttpSource`, so `deleteEpisodes` never removes local files (E-28). The dialog hides the "downloaded" checkbox for local entries anyway (B-12.5).
- `deleteEpisodes` is honored even when `deleteFromLibrary` is false (the two checkboxes are independent).

### B-11.5 Open random entry (`getRandomAnimelibItemForCurrentCategory`, `:596-604`)
```
if (categories.isEmpty()) return null
withIOContext { state.getAnimelibItemsByCategoryId(categories[activeCategoryIndex].id)?.randomOrNull() }
```
The list is the post-filter, post-search, post-sort list of the current page. Click → `navigator.push(AnimeScreen(item.id))`, or snackbar `information_no_entries_found` ("No entries found in this category") when null (`AnimeLibraryTab.kt:148-158`). The random pick is not seeded (uses `kotlin.random`).

---

## B-12. Settings dialog (Filter / Sort / Display)

File: `app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt`.

Container: `TabbedDialog` with tabs `action_filter` (page 0), `action_sort` (page 1), `action_display` (page 2) (`:49-75`). Each page is in a vertically scrollable column with `TabbedDialogPaddings.Vertical`.

Opened by: the toolbar filter button (`onClickFilter = showSettingsDialog`), tab reselect (B-1.2).

### B-12.1 Filter page (`FilterPage`, `:79-160`)
Order, top to bottom:
1. `TriStateItem(label = label_downloaded, state, enabled, onClick)`. `downloadedOnly` (global) → state displayed = ENABLED_IS and `enabled = false` (non-interactive). Otherwise state = `filterDownloadedAnime`.
2. `TriStateItem(label = action_filter_unseen [anime] / action_filter_unread [manga], state = filterUnseen)`.
3. `TriStateItem(label = label_started, state = filterStarted)`.
4. `TriStateItem(label = action_filter_bookmarked, state = filterBookmarked)`.
5. `TriStateItem(label = completed, state = filterCompleted)`.
6. Interval filter — only if `!isReleaseBuildType && ENTRY_OUTSIDE_RELEASE_PERIOD in autoUpdateItemRestrictions` (`:120-128`): `action_filter_interval_custom`. In release builds this item never appears.
7. Trackers (`trackersFlow`, `stateIn(WhileSubscribed(5 s), initial = loggedInTrackers())`):
   - 0 trackers → nothing.
   - 1 tracker → `TriStateItem(label = action_filter_tracked, state = filterTracked(id))`.
   - >1 → `HeadingItem(action_filter_tracked)` then one `TriStateItem(label = service.name, ...)` per logged-in tracker, in `trackers` order.

Tap = `toggleFilter` (cycle) for every tristate.

### B-12.2 Sort page (`SortPage`, `:163-225`)
Options, in this order (`:177-189`), `listOfNotNull`:
1. `action_sort_alpha` → Alphabetical
2. `AYMR action_sort_total_episodes` → TotalEpisodes (manga: `action_sort_total` → TotalChapters)
3. `AYMR action_sort_last_seen` → LastSeen (manga: `action_sort_last_read` → LastRead)
4. `AYMR action_sort_last_anime_update` → LastUpdate (manga: `action_sort_last_manga_update`)
5. `AYMR action_sort_unseen_count` → UnseenCount (manga: `action_sort_unread_count`)
6. `AYMR action_sort_latest_episode` → LatestEpisode (manga: `action_sort_latest_chapter`)
7. `AYMR action_sort_episode_fetch_date` → EpisodeFetchDate (manga: `action_sort_chapter_fetch_date`)
8. `MR action_sort_date_added` → DateAdded
9. `MR action_sort_tracker_score` → TrackerMean — **only when trackers is non-empty** (`remember(trackers.isEmpty())`, `:171-176`)
10. `AYMR action_sort_airing_time` → AiringTime (anime only)
11. `MR action_sort_random` → Random

Interaction (`:192-224`):
- Random row: `BaseSortItem(label, icon = Refresh if current type is Random, onClick = setSort(category, Random, Ascending))`.
- Other rows: `SortItem(label, sortDescending = sortDescending.takeIf { currentType == mode }, onClick)`:
  - same type as current → flip direction.
  - different type → direction = current direction (`sortDescending` → Descending else Ascending).

Current values: `sortingMode = category.sort.type`, `sortDescending = !category.sort.isAscending` where `category` is the active page's category (`:167-169`).

### B-12.3 Display page (`DisplayPage`, `:235-318`)
1. `SettingsChipRow(action_display_mode)` with four `FilterChip`s in this order: `action_display_grid`→CompactGrid, `action_display_comfortable_grid`→ComfortableGrid, `action_display_cover_only_grid`→CoverOnlyGrid, `action_display_list`→List (`displayModes`, `:227-232`). Selected chip = current `displayMode` preference. Click → `setDisplayMode` (writes the shared preference).
2. Columns slider:
   - `columnPreference` chosen by `LocalConfiguration.orientation` (`remember { }` without key; `:249-256`).
   - `SliderItem(value = columns, valueRange = 0..10, label, valueText = if (columns > 0) "$columns" else label_auto ("Auto"), onChange = columnPreference::set, pillColor = surfaceContainerHighest)`.
   - Label: `displayMode == List` → `AYMR pref_library_rows` ("List size"); otherwise `MR pref_library_columns` ("Items per row").
3. Heading `overlay_header` ("Overlay") then checkboxes bound directly to preferences:
   - `AYMR action_display_download_badge_anime` (anime) / `MR action_display_download_badge` (manga) → `downloadBadge()` (default false).
   - `AYMR action_display_unseen_badge` (anime) / `MR action_display_unread_badge` (manga) → `unreadBadge()` (default true).
   - `MR action_display_local_badge` → `localBadge()` (default true).
   - `MR action_display_language_badge` → `languageBadge()` (default false).
   - `AYMR action_display_show_continue_reading_button` → `showContinueViewingButton()` (default false).
4. Heading `tabs_header` ("Tabs") then:
   - `MR action_display_show_tabs` → `categoryTabs()` (default true).
   - `MR action_display_show_number_of_items` → `categoryNumberOfItems()` (default false).

All checkboxes write immediately. There is no "apply" button.

### B-12.4 Settings writes are global
Display mode, badges, tabs, counts, continue button, and columns are not per-category. Sort is per-category only when `categorized_display` is on (B-5.7).

### B-12.5 Delete dialog (`DeleteLibraryEntryDialog`, `presentation/library/DeleteLibraryEntryDialog.kt:19-81`)
- Title `action_remove` ("Remove").
- Checkbox 1: `AYMR manga_from_library` / `AYMR anime_from_library` ("Manga from library"/"Anime from library"), default unchecked.
- Checkbox 2 (only when no selected entry is local): `MR downloaded_chapters` / `AYMR downloaded_episodes`, default unchecked.
- Each row toggles (`CheckboxState.State.None ↔ Checked` via `next()`).
- Confirm (`action_ok`) disabled unless at least one checkbox is checked. Confirm closes the dialog and calls `onConfirm(list[0].isChecked, list.getOrElse(1) { None }.isChecked)`.
- Cancel (`action_cancel`) closes without action.

### B-12.6 Change category dialog (`ChangeCategoryDialog`, `presentation/category/components/CategoryDialogs.kt:197-303`) — see §B-13.

### B-12.7 Delete category/rename/create dialogs — §B-16.

---

## B-13. Change categories (membership)

Full algorithm and worked examples are in `p1-topic-category-membership.md`. Summary:

### B-13.1 Opening (`openChangeCategoryDialog`, `AnimeLibraryScreenModel.kt:695-718`)
In `launchIO`:
1. `animeList = selection.map { it.anime }`.
2. `categories = state.categories.filter { it.id != 0L }`. **The Default category is never offered** (E-12).
3. `common = intersect over animes of (GetVisibleAnimeCategories.await(animeId).toSet())`. Empty selection → empty.
4. `mix = (union of the per-anime sets) − common`.
5. Preselect each `categories` entry: `in common → CheckboxState.State.Checked(c)`; `in mix → CheckboxState.TriState.Exclude(c)`; else `CheckboxState.State.None(c)`. The list is `ImmutableList`.
6. Dialog = `ChangeCategory(animeList, preselected)`.

Hidden categories are excluded from `common`/`mix` (the set comes from the visible-only query, E-07).

If `initialSelection` is empty (the library has no non-default categories), the dialog shows the message `information_empty_category_dialog` ("You don't have any categories yet.") with the single button `action_edit_categories` → `clearSelection(); navigator.push(CategoriesTab)` (`ChangeCategoryDialog`, `CategoryDialogs.kt:203-224`).

### B-13.2 Dialog controls (`ChangeCategoryDialog`)
- Title `action_move_category` ("Set categories"). Body: scrollable column of rows; each row toggles on click of the row or its control.
- Control per type:
  - `CheckboxState.State` → `Checkbox`.
  - `CheckboxState.TriState` → `TriStateCheckbox` with `checkbox.asToggleableState()`.
- Row click → `it.next()` (`CheckboxState.next()`):
  - `State`: None ↔ Checked.
  - `TriState`: Exclude → None → Include → Exclude.
- Buttons (row): `action_edit` (closes, navigates to CategoriesTab), spacer, `action_cancel`, `action_ok`.
- OK: closes, then `onConfirm(include, exclude)` where
  - `include = selection.filter { it is State.Checked || it is TriState.Include }.map { id }`
  - `exclude = selection.filter { it is State.None || it is TriState.None }.map { id }`
  - A `TriState.Exclude` (mixed) row is in neither list, so its membership is left unchanged.

### B-13.3 Apply (`setAnimeCategories(animeList, addCategories = include, removeCategories = exclude)`, `:562-578`)
```
launchNonCancellable {
  animeList.forEach { anime ->
     categoryIds = getCategories.await(anime.id)        // VISIBLE categories only
        .map { id }.subtract(removeCategories.toSet()).plus(addCategories).toList()
     setAnimeCategories.await(anime.id, categoryIds)     // deletes ALL memberships, inserts categoryIds
  }
}
```
Then the caller calls `clearSelection()` immediately (`AnimeLibraryTab.kt:258-261`).

Observed defect: the replacement drops memberships in hidden categories (E-07). Required fix in T-37a-030: read all memberships (not only visible) when computing the new set, or change the repository call to a delta update.

Repository: `AnimeRepositoryImpl.setAnimeCategories` runs in a transaction: `deleteAnimeCategoryByAnimeId(animeId)` then `insert` per id (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:95-102`).

---

## B-14. Manga-specific differences (complete list)

Derived from the anime/manga diffs of every file in scope:

1. Names: `Chapter` replaces `Episode`; `Read` replaces `Seen`; `Unread` replaces `Unseen`; `LibraryManga`/`MangaLibraryItem`; `MangaLibraryMap` typealias; `MangaLibraryScreenModel`.
2. Constructor: no `backgroundCache` (`MangaLibraryScreenModel.kt:93`); `getNextChapters`, `getChaptersByMangaId`, `setReadStatus` instead of the episode interactors.
3. `filterUnread` key `pref_filter_library_unread_v2` (anime `pref_filter_animelib_unread_v2`); the shared-preference map is in §D-3.
4. Sort types: `LastRead`, `UnreadCount`, `TotalChapters`, `LatestChapter`, `ChapterFetchDate`, plus `LastUpdate`, `DateAdded`, `TrackerMean`, `Alphabetical`, `Random`. **No `AiringTime`** (`MangaLibraryScreenModel.kt:280-322`).
5. Unread-count comparator: same logic as anime, names renamed.
6. Count source: `totalChapters` and `readCount` (`LibraryManga`), `lastRead`, `chapterFetchedAt`.
7. Next-up: `getChaptersByMangaId.await(manga.id, applyScanlatorFilter = true)` (`MangaLibraryScreenModel.kt:436-438`). Anime does not pass a scanlator filter.
8. Manga library view excludes chapters whose scanlator is in `excluded_scanlators` (`libraryView.sq:22-27`). Anime has no such exclusion.
9. Download action uses `isChapterDownloaded(name, scanlator, title, source)` and `downloadChapters`.
10. Settings labels: `action_display_download_badge` ("Downloaded chapters"), `action_display_unread_badge` ("Unread chapters"), `action_filter_unread`, `action_sort_total`, `action_sort_last_read`, `action_sort_unread_count`, `action_sort_latest_chapter`, `action_sort_chapter_fetch_date`, `action_sort_last_manga_update`.
11. Tab: index `1` (or `5` when `NavStyle.MOVE_MANGA_TO_MORE`), `navigateUp` callback provided only when `fromMore` (`MangaLibraryTab.kt:113-124`): if `navigator.lastItem == HomeScreen` → `HomeScreen.openTab(HomeScreen.Tab.AnimeLib())` (opens the **anime** library) else `navigator.pop()`. Passed to `LibraryToolbar(navigateUp = navigateUp)` → shows a back arrow in the title row (`SearchToolbar(navigateUp)`).
12. Tab: the "Edit categories" flow additionally calls `CategoriesTab.showMangaCategory()` (`MangaLibraryTab.kt:277`), so the categories screen opens on the Manga page.
13. Tab: continue action opens the reader; not the player (B-8.11).
14. Delete dialog: `isManga = true`, label keys `manga_from_library` and `downloaded_chapters`.
15. Bottom bar labels: `markReadSelection` (manga) vs `markSeenSelection` (anime).
16. `MangaLibraryContent` names the continue parameter `onContinueReadingClicked` / `onClickContinueReading`. Same behavior.
17. Manga `LibraryPagerEmptyScreen`, the pager, the content composable, the list, and both grids are identical in logic to the anime versions (only names differ).
18. `MangaLibraryCompactGrid` is `internal`, anime's is `public` — no behavioral effect.
19. The manga ChangeCategory dialog exposes `manga: List<Manga>` (`MangaLibraryScreenModel.kt:717`).

Shared (not split) preferences between anime and manga are listed in §D-3 and §D-6 (for example `display_download_badge`, `display_unread_badge`, `display_local_badge`, `display_language_badge`, `display_category_tabs`, `display_number_of_items`, `display_continue_reading_button`, `hidden_categories`, `pref_display_mode_library`, `pref_filter_library_interval_custom`, `library_update_manga_restriction`).

---

## B-15. Category management screens

Files: `presentation/category/*` and `tachiyomi/ui/category/*`.

### B-15.1 Screen structure
- `CategoriesTab` (`app/src/main/java/eu/kanade/tachiyomi/ui/category/CategoriesTab.kt`): `TabbedScreen(titleRes = general_categories, tabs = [animeCategoryTab(), mangaCategoryTab()])`. Tab options `index = 7u`, title `general_categories` ("Categories"), icon `anim_updates_enter` (`:34-44`).
- Pages: `label_anime` ("Anime") and `label_manga` ("Manga") (`AnimeCategoryTab.kt:28`, `MangaCategoryTab.kt`).
- `showMangaCategory()` sends on a `Channel(1, DROP_OLDEST)` → `state.scrollToPage(1)` (`CategoriesTab.kt:46-74`).
- Events: both screen models' `events` are collected in `LaunchedEffect(Unit)`; `LocalizedMessage` → `context.toast(stringRes)` (`:80-91`). Only `InternalError` (`internal_error`: "InternalError: Check crash logs for further information") exists.
- `(context as? MainActivity)?.ready = true` (`:76-78`) — Android splash, drop (T-37a-052).

### B-15.2 Per-kind screen (`AnimeCategoryTab.kt`)
- `animeCategoryTab()` returns `TabContent(titleRes = label_anime, searchEnabled = false, navigateUp = navigator::pop)`.
- Loading state → `LoadingScreen()`; success → `AnimeCategoryScreen(state, onClickCreate, onClickRename, onClickHide, onClickDelete, onChangeOrder)`.
- Dialogs, from `successState.dialog`:
  - `Create` → `CategoryCreateDialog(categories = names)`.
  - `Rename(category)` → `CategoryRenameDialog(category = name, categories = names)`.
  - `Delete(category)` → `CategoryDeleteDialog(category = name)`, confirm → `deleteCategory(category.id)`.

### B-15.3 Screen contents (`AnimeCategoryScreen.kt:29-111`)
- `Scaffold` with `ExtendedFloatingActionButton(text = action_add, icon = Add)`; expanded by `lazyListState.shouldExpandFAB()` (expanded when at the top, scrolled backward, or cannot scroll forward) (`CategoryFloatingActionButton.kt`, `LazyListState.kt:10-19`).
- Empty (`state.isEmpty`): `EmptyScreen(information_empty_category)` ("You have no categories. Tap the plus button to create one for organizing your library.").
- Non-empty: `LazyColumn` with `contentPadding = medium`, spacing `small`, keys `"category-$id"`; each item a `ReorderableItem` (drag by handle) rendering `CategoryListItem`.
- Reorder: `rememberReorderableLazyListState` → local move in `categoriesState` then `onChangeOrder(item, to.index)` (`AnimeCategoryScreen.kt:77-81`). `categoriesState` resynchronises from `categories` only when no item is being dragged (`:83-88`).

### B-15.4 Category row (`CategoryListItem.kt:29-86`)
`ElevatedCard` → `Row`: drag handle (`DragHandle`, `draggableHandle()`), `category.name` (weight 1), then icon buttons:
1. Rename (`Edit`, `action_rename_category`).
2. Hide/show: icon `Visibility` when `category.hidden` else `VisibilityOff`; description `action_hide` ("Hide") for both (`AYMR`).
3. Delete (`Delete`, `action_delete`).
The whole row click (`clickable(onClick = onRename)`) also renames.

### B-15.5 Operations
| Action | Domain interactor (anime; manga identical with names) | Behavior |
|---|---|---|
| Create | `CreateAnimeCategoryWithName.await(name)` | `nextOrder = max(order)+1` (or 0); flags = global `animelib_sorting_mode` sort type+direction; `hidden = false`; `id = 0` (auto); `Internal error` on exception. |
| Rename | `RenameAnimeCategory.await(category, name)` | Partial update name. |
| Hide/Show | `HideAnimeCategory.await(category)` | Toggles `hidden`. |
| Delete | `DeleteAnimeCategory.await(categoryId)` | See below. |
| Reorder | `ReorderAnimeCategory.await(category, newIndex)` | Mutex-guarded. Non-system categories re-indexed `0..n-1`. |

`DeleteAnimeCategory.await` (`domain/.../DeleteAnimeCategory.kt:17-59`):
1. Delete row → on error InternalError.
2. Re-read all categories (including the system row) and set `order = index` for each (system category gets index 0 among everything, so the system row's stored order becomes 0 and the rest shift).
3. If `defaultAnimeCategory == categoryId` → delete `defaultAnimeCategory` pref (fallback to "uncategorized").
4. Remove the id from: `animeUpdateCategories`, `animeUpdateCategoriesExclude`, `removeExcludeAnimeCategories`, `downloadNewEpisodeCategories`, `downloadNewEpisodeCategoriesExclude` (string-set membership, `"$id"`).
5. Persist reorder updates.
Note: entries that were in the deleted category lose that membership (cascade in `animes_categories`) and fall back to the system category when they have no other category (the `coalesce(MC.category_id, 0)` view rule).

`ReorderAnimeCategory` (`ReorderAnimeCategory.kt:18-46`): loads non-system categories, removes the item at `currentIndex`, inserts at `newIndex`, re-indexes to `0..n-1`. Unknown id → `Result.Unchanged`.

`HideAnimeCategory` returns `RenameAnimeCategory.Result.Success` in its success path (`HideAnimeCategory.kt:22`). The Result types are compatible at runtime because the `when` only checks `InternalError`; Rust should use one result type.

`GetVisibleAnimeCategories` only ever returns `hidden = 0` rows.

### B-15.6 Category screen model (`AnimeCategoryScreenModel.kt:41-59`)
- Init: subscribes to `getVisibleCategories.subscribe()` when `hidden_categories` pref (default false) is **on**, else `getAllCategories.subscribe()`. Filter out system category. `collectLatest` → `Success(categories)` with `dialog = null`.
- **Observed:** every emission replaces the state with a `Success` whose `dialog` is `null`, so a category change while a dialog is open closes that dialog (E-29).
- `createCategory(name)`, `hideCategory`, `deleteCategory(id)`, `changeOrder`, `renameCategory`: each runs in `screenModelScope.launch` and sends `InternalError` on the events channel if the interactor returns `InternalError` (`:61-115`).
- `showDialog`/`dismissDialog` are no-ops while `Loading` (`:117-133`).
- Events channel is a rendezvous `Channel()` (`:38`), so the event sender suspends until the Tab collects it.

Hidden-category setting `hidden_categories` decides whether the management list shows hidden categories (B-15.6). The library-page visibility is always visible-only (B-3).

### B-15.7 Create/rename/delete dialogs (`CategoryDialogs.kt:40-194`)
- Create: title `action_add_category` ("Add category"). `OutlinedTextField` label `name` ("Name"), `singleLine`, focus requested after 100 ms (`delay(0.1.seconds)`, `:95-99`). Supporting text: `error_category_exists` ("A category with this name already exists!") if `name.isNotEmpty() && categories.contains(name)` (case-sensitive exact), otherwise `information_required_plain` ("*required"). Confirm `action_add` enabled iff `name.isNotEmpty() && !exists`. Dismiss `action_cancel`. Confirm → `onCreate(name)` then dismiss. Name is not trimmed.
- Rename: initial value = current name; `valueHasChanged` flips on any edit that changes the text; confirm `action_ok` enabled iff `valueHasChanged && !exists`. Error only shown when changed and exists.
- Delete: title `delete_category` ("Delete category"); text `delete_category_confirmation` with `%s` = name; confirm `action_ok`.

### B-15.8 Move-to-category (ChangeCategoryDialog) — §B-13.

---

## B-16. Library-level update triggers (from the toolbar)

- Overflow menu: `action_update_library` (global) → `onClickRefresh(null)`; `action_update_category` → `onClickRefresh(categories[activeIndex])`; `action_open_random_manga` (the shared key is used for both kinds) → random (B-11.5). Labels from `LibraryToolbar.kt:110-120`. Both kinds use the key `action_open_random_manga` in `LibraryRegularToolbar` (`LibraryToolbar.kt:118`).
- `onClickRefresh(category)` (`AnimeLibraryTab.kt:105-112`): `started = AnimeLibraryUpdateJob.startNow(context, category)`; then a snackbar: `updating_category` ("Updating category") if started, else `update_already_running` ("An update is already running"). Returns `started`.
- `AnimeLibraryUpdateJob.startNow` (`app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt:500-521`): returns false if the unique tag is already running; otherwise enqueues a one-time `KEEP` job with input `category?.id` (null = whole library).
- **Observed:** the global update also shows `updating_category` ("Updating category"), not a global-specific message (E-30).
- Pull-to-refresh (`AnimeLibraryContent.kt:85-96`): only when `selection.isEmpty()`. It calls `onRefresh(categories[currentPage()])` (current category). If the job was not started, nothing else happens (no spinner). If started, a fake spinner `isRefreshing = true` for 1 second.

---

## B-17. Shell and navigation interactions

- Library tab index and label: §B-1.1.
- Selection hides the bottom navigation bar: `HomeScreen.showBottomNav(!selectionMode)` (`AnimeLibraryTab.kt:285-287`).
- Global search opens `GlobalAnimeSearchScreen(query)` (`AnimeLibraryTab.kt:219-223`); area 33/36.
- The entry screen (`AnimeScreen(id)`) is opened by click and random pick (area 13/07 details).
- Manga "Set categories" opens `CategoriesTab`.
- Splash `ready` flag: Android-only (drop).

---

## B-18. Concurrency and dispatchers

| Operation | Dispatcher / scope | Note |
|---|---|---|
| Library pipeline | `screenModelScope.launchIO` (IO) | `collectLatest`: newest input cancels older work. |
| Preference-derived state | `screenModelScope` via `asState` | Writes update immediately. |
| Selection, filter toggles, search | `mutableState.update` (atomic CAS on `StateFlow`) | Sync on main. |
| Bulk actions (mark, download, delete, categories) | `screenModelScope.launchNonCancellable` | Continue after screen dismissal. Selection is cleared synchronously before the work finishes. |
| `openChangeCategoryDialog` | `launchIO` | DB reads off main. |
| Random pick | `withIOContext` | DB read. |
| Continue (open episode) | `scope.launchIO` in the composable (`rememberCoroutineScope`) | Opens activity from IO. |
| Category ops | `screenModelScope.launch` + `withNonCancellableContext` in interactors | |
| `ReorderAnimeCategory` | `Mutex` | Serializes concurrent reorders. |

Proposal (Rust): a single DB worker thread owns the `rusqlite::Connection` (see the design doc). Pipeline recompute runs on the worker. Bulk actions run as tokio tasks, not cancelled on window close. UI updates via `glib::MainContext`.

---

## B-19. Timings and limits

| Name | Value | Source |
|---|---|---|
| Search debounce | 250 ms | `AppBar.kt:65` |
| Pull-refresh fake spinner | 1 s | `AnimeLibraryContent.kt:93` |
| Category dialog focus delay | 0.1 s | `CategoryDialogs.kt:97, 161` |
| Trackers flow sharing timeout | 5 s | `AnimeLibrarySettingsScreenModel.kt:34` |
| `GetLibraryAnime` retry on `NullPointerException` | retry after 0.5 s | `domain/.../GetLibraryAnime.kt:23-29` |
| Grid adaptive min width | 128 dp | `LazyLibraryGrid.kt:22` |
| Grid spacing | 4 dp h / 4 dp v; content padding +8 dp | `LazyLibraryGrid.kt:24-26`, `CommonEntryItem.kt:50-51` |
| Row height (list, size 0) | 76 dp | `CommonEntryItem.kt:351` |
| Row padding | 16 dp h, 3 dp v | `CommonEntryItem.kt:362` |
| Slider range | 0…10 | `AnimeLibrarySettingsDialog.kt:262, 275` |
| Cover ratio | 2:3 (w/h = 0.6667) | `ItemCover.kt:20` |
| Gradient overlay height | 33 % of cover | `CommonEntryItem.kt:138` |
| Selected cover alpha | 0.76 | `CommonEntryItem.kt:66` |
| Selected background alpha | 0.22 light / 0.16 dark | `presentation-core/.../Modifier.kt:27-36` |
| Pill alpha | 0.08 light / 0.12 dark | `LibraryToolbar.kt:78` |
| Pager composition window | current ±1 | `AnimeLibraryPager.kt:55` |
| Download N choices | 1, 5, 10, 25, all | `AnimeLibraryScreenModel.kt:470-474` |
| Sort reseed | `Random.nextInt()` (full int range) | `SetSortModeForAnimeCategory.kt:24` |

---

## B-20. Interactions with other areas (for the integrator)

| Dependency | Used for | Area |
|---|---|---|
| `GetLibraryAnime` / `animelibView` | Library rows | area:05-data-repos-entries-items, area:04-db-anime-schema-migrations |
| `libraryView` / manga library rows | Library rows | area:03-db-manga-schema-migrations |
| `GetVisibleAnimeCategories`, category repos | Categories | area:08-domain-library-category-history-updates |
| `GetTracksPerAnime`, `TrackerManager` | Tracker filter/sort | area:27-tracking-core |
| `AnimeDownloadManager`, `AnimeDownloadCache`, `MangaDownloadManager` | Badges, download actions, deletes | area:24-downloads-anime, area:23-downloads-manga, area:25-downloads-shared |
| `AnimeSourceManager.getOrStub`, `getNameForAnimeInfo`, `lang` | Source name/lang badge and search | area:17-source-app-layer |
| `isLocal()` | Local badge/filter | area:14-source-local-and-metadata |
| `AnimeCoverCache`, `AnimeBackgroundCache` | Remove-from-library cleanup | area:34-images-covers-cache |
| `AnimeLibraryUpdateJob`, `MangaLibraryUpdateJob` | Update buttons | area:26-library-update-jobs |
| `GetNextEpisodes`, `GetEpisodesByAnimeId`, `GetNextChapters`, `GetChaptersByMangaId` | Download N, continue | area:06-data-repos-history-track-source-misc, area:07-domain-entries-items |
| `SetSeenStatus`, `SetReadStatus` | Mark selection | area:07-domain-entries-items |
| `UpdateAnime`, `AnimeUpdate` | Remove from library | area:05-data-repos-entries-items |
| `AnimeScreen`, `MangaScreen`, `GlobalAnimeSearchScreen`, `ReaderActivity`, `MainActivity.startPlayerActivity` | Navigation | area:36-ui-shell-navigation-security-webview, area:13-core-util-storage-archive-torrent |
| `LibraryBottomActionMenu`, `DownloadAction`, `SearchToolbar`, `AppBar`, `TabbedDialog`, `SettingsChipRow`, `SliderItem`, `CheckboxItem`, `TriStateItem`, `SortItem`, `BaseSortItem`, `LabeledCheckbox`, `EmptyScreen`, `LoadingScreen`, `Pill`, `TabText`, `Badge`, `BadgeGroup`, `PullRefresh`, `FastScrollLazyColumn`/`FastScrollLazyVerticalGrid`, `ItemCover` | UI primitives | area:36-ui-shell-navigation-security-webview, area:02-preferences-system (preference binding) |
| `PlayerPreferences.alwaysUseExternalPlayer` | Continue opens external vs internal player | area:02-preferences-system (player area) |
| `BasePreferences.downloadedOnly` | Global downloaded-only override | area:02-preferences-system |
| `NavStyle`, `currentNavigationStyle()` | Manga tab position and back | area:36-ui-shell-navigation-security-webview |
| `compareToWithCollator` | Title sort collation | area:01-foundation-app-bootstrap / core util |

---

## B-21. Behavior-level open questions (resolved by proposal)

1. Global update message: keep the upstream wording but name the target ("Updating library" for global). Proposal (T-37a-050).
2. Downloaded filter when downloaded-only mode is on: upstream forces ENABLED_IS. Keep, but reflect it in `hasActiveFilters` (T-37a-056).
3. Continue button coupling with the unread badge: fix (T-37a-033).
4. Hidden categories dropped on category apply: fix (T-37a-030).
5. TrackerMean NaN ordering: fix to `-1.0` (T-37a-058).
6. Range-select crash on stale snapshot: compare by id (T-37a-055).
