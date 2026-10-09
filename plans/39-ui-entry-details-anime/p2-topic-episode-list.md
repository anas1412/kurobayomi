# Topic: episode list pipeline (sort, filter, separators, selection, next unseen)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt` (lines 1369-1455, 1602-1760), `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt` (callee of the list pipeline), `app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/episode/EpisodeFilterDownloaded.kt`.
Callees: `domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt`, `domain/src/main/java/tachiyomi/domain/items/episode/service/MissingEpisodes.kt`, `domain/src/main/java/tachiyomi/domain/entries/TriState.kt`, `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`, `app/src/main/java/eu/kanade/core/util/CollectionUtils.kt`, `app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt`.

Repo-relative paths; `/home/blackbox/aniyomi-src` is the root.

---

## 1. Data the pipeline reads

Per episode (`Episode`, domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:6-25): `id`, `animeId`, `seen`, `bookmark`, `fillermark`, `lastSecondSeen`, `totalSeconds`, `dateFetch`, `sourceOrder`, `url`, `name`, `dateUpload`, `episodeNumber: Double`, `scanlator`, `summary`, `previewUrl`, `lastModifiedAt`, `version`, `memo`.

- `isRecognizedNumber` = `episodeNumber >= 0f` (Episode.kt:27-28). The sentinel for "unknown number" is `-1.0` (Episode.create, line 57).
- `dateUpload` default in `create()` is `-1` (Episode.kt:56).

Per anime: `episodeFlags` (bit layout in `p2-data-and-config.md` section 2.1) and the derived getters in `Anime.kt:55-122`.

Per item (`EpisodeList.Item`): `episode`, `downloadState`, `downloadProgress`, `selected`, plus the body members `id` (= `episode.id`) and `isDownloaded` (= `downloadState == DOWNLOADED`).

---

## 2. Sort

### 2.1 Key constants (domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:219-243)

| Constant | Value |
|---|---|
| `EPISODE_SORT_DESC` | `0x00000000L` (= 0) |
| `EPISODE_SORT_ASC` | `0x00000001L` |
| `EPISODE_SORT_DIR_MASK` | `0x00000001L` |
| `EPISODE_SORTING_SOURCE` | `0x00000000L` (= 0) |
| `EPISODE_SORTING_NUMBER` | `0x00000200L` |
| `EPISODE_SORTING_UPLOAD_DATE` | `0x00000400L` |
| `EPISODE_SORTING_ALPHABET` | `0x00000600L` |
| `EPISODE_SORTING_MASK` | `0x00000600L` |

`sortDescending()` = `episodeFlags and EPISODE_SORT_DIR_MASK == EPISODE_SORT_DESC` (Anime.kt:106-108). Because DESC is 0, a flags value of 0 means descending.

### 2.2 Comparator (EpisodeSorter.kt:7-30), exact

`getEpisodeSort(anime, sortDescending = anime.sortDescending())` returns `(Episode, Episode) -> Int` chosen by `anime.sorting` (the masked sort key):

| `anime.sorting` | `sortDescending == true` | `sortDescending == false` |
|---|---|---|
| `EPISODE_SORTING_SOURCE` (0) | `e1.sourceOrder.compareTo(e2.sourceOrder)` (ASCENDING by sourceOrder) | `e2.sourceOrder.compareTo(e1.sourceOrder)` (DESCENDING by sourceOrder) |
| `EPISODE_SORTING_NUMBER` (0x200) | `e2.episodeNumber.compareTo(e1.episodeNumber)` (descending number) | `e1.episodeNumber.compareTo(e2.episodeNumber)` (ascending number) |
| `EPISODE_SORTING_UPLOAD_DATE` (0x400) | `e2.dateUpload.compareTo(e1.dateUpload)` | `e1.dateUpload.compareTo(e2.dateUpload)` |
| `EPISODE_SORTING_ALPHABET` (0x600) | `e2.name.compareToWithCollator(e1.name)` | `e1.name.compareToWithCollator(e2.name)` |
| anything else | `throw NotImplementedError("Invalid episode sorting method: ${anime.sorting}")` | same |

Consequences that the Rust port must reproduce:
- SOURCE + DESC (the **default**) sorts by ascending `sourceOrder`. `sourceOrder` is the index in the source's list (SyncEpisodesWithSource.kt:67) and sources return newest first (SyncEpisodesWithSource.kt:194 comment). So the default display is newest first. The inversion in the table is therefore intentional: "descending" means "display in source order".
- NUMBER + DESC sorts high to low.
- The comparator is a total order only if values are distinct. Equal numbers (duplicates) fall back to the stable sort of the input order (`sortedWith` is stable in Kotlin).
- `compareToWithCollator` = `java.text.Collator.getInstance(Locale.getDefault())` with `strength = PRIMARY` (core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:5-15). PRIMARY ignores case and diacritics. **Proposal:** use `icu_collator` with primary strength and the user's locale, falling back to `en`.

### 2.3 Sorting invariants and where sorting happens

- `State.Success.processedEpisodes` (AnimeScreenModel.kt:1693-1698) sorts with `getEpisodeSort(anime)` (default `sortDescending = anime.sortDescending()`).
- `getUnseenEpisodesSorted()` (AnimeScreenModel.kt:770-774) sorts again with the same comparator and then reverses when `anime.sortDescending()`.
- `EpisodeFilter.kt:37` and `EpisodeFilter.kt:55` also sort with `getEpisodeSort(anime)` in the domain-level overloads used by `getNextUnseen` and by the player.

### 2.4 Setting the sort (SetAnimeEpisodeFlags.kt:56-79)

`setSorting(sort)` calls `awaitSetSortingModeOrFlipOrder(anime, sort)`:
- If `anime.sorting == sort`: flip direction. New direction = `EPISODE_SORT_ASC` when currently descending, else `EPISODE_SORT_DESC`.
- Else: set sort key to `sort`, and set direction to `EPISODE_SORT_ASC`.

So selecting a new key always gives ascending. Selecting the current key toggles direction. **Proposal:** keep this exact behavior (users expect "tap the same key to flip").

---

## 3. Filters

### 3.1 Order and semantics

`applyFilters` on `List<EpisodeList.Item>` (AnimeScreenModel.kt:1682-1699; the same logic exists in EpisodeFilter.kt:44-56):

```
val isLocalAnime = anime.isLocal()
val unseenFilter = anime.unseenFilter
val downloadedFilter = anime.downloadedFilter
val bookmarkedFilter = anime.bookmarkedFilter
val fillermarkedFilter = anime.fillermarkedFilter
asSequence()
  .filter { (episode) -> applyFilter(unseenFilter) { !episode.seen } }
  .filter { (episode) -> applyFilter(bookmarkedFilter) { episode.bookmark } }
  .filter { (episode) -> applyFilter(fillermarkedFilter) { episode.fillermark } }
  .filter { applyFilter(downloadedFilter) { it.isDownloaded || isLocalAnime } }
  .sortedWith { (e1), (e2) -> getEpisodeSort(anime).invoke(e1, e2) }
```

`applyFilter(filter: TriState, predicate)` (TriState.kt:5-9): `DISABLED -> true`, `ENABLED_IS -> predicate()`, `ENABLED_NOT -> !predicate()`.

The `EpisodeFilter.kt:16-38` overload for `List<Episode>` differs in one way: the downloaded predicate calls `downloadManager.isEpisodeDownloaded(name, scanlator, title, source)` directly (not `isDownloaded` on the item) and it ends with `.toList()`. Used by `getNextUnseen(anime, downloadManager)` only.

### 3.2 Each filter's source flag

`Anime.unseenFilter` (Anime.kt:85-90): `unseenFilterRaw` (= `episodeFlags & 0x6`) maps `EPISODE_SHOW_UNSEEN (0x2) -> ENABLED_IS`, `EPISODE_SHOW_SEEN (0x4) -> ENABLED_NOT`, else `DISABLED`.

`Anime.bookmarkedFilter` (92-97): `0x20 -> ENABLED_IS`, `0x40 -> ENABLED_NOT`.

`Anime.fillermarkedFilter` (99-104): `0x80 -> ENABLED_IS`, `0x100 -> ENABLED_NOT`.

`Anime.downloadedFilter` (app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:13-21):
```
if (BasePreferences.downloadedOnly().get()) return ENABLED_IS      // global override
return when (downloadedFilterRaw) {
  EPISODE_SHOW_DOWNLOADED (0x8) -> ENABLED_IS
  EPISODE_SHOW_NOT_DOWNLOADED (0x10) -> ENABLED_NOT
  else -> DISABLED
}
```

**Important:** when the global "downloaded only" mode is on (`BasePreferences.downloadedOnly`, key `Preference.appStateKey("pref_downloaded_only")`, default false, BasePreferences.kt:16-19), the downloaded filter is forced to `ENABLED_IS` regardless of the per-entry setting. The per-entry setting is not changed; the UI shows the filter as active. Note that `downloadedFilter` is also forced for `seasonDownloadedFilter` (Anime.kt:23-31).

### 3.3 Filtering downloaded episodes for other consumers

`filterDownloadedEpisodes(anime)` (EpisodeFilterDownloaded.kt:31-44): returns the list unchanged for local anime; else keeps episodes where `downloadCache.isEpisodeDownloaded(name, scanlator, title, source, skipCache = false)`. Consumer: `PlayerViewModel.kt:1343`.

### 3.4 The `isLocal` rule

For local anime (`anime.isLocal()`, source id `LocalAnimeSource.ID = 0L`), every episode is treated as downloaded. Consequences: the downloaded filter never removes local episodes (`downloaded || isLocalAnime`, EpisodeFilter.kt:34 and AnimeScreenModel.kt:1692).

---

## 4. Separators (missing episodes)

### 4.1 Generic helper (app/src/main/java/eu/kanade/core/util/CollectionUtils.kt:8-24)

`List<T>.insertSeparators(generator: (before: T?, after: T?) -> R?): List<R>`:
- Empty list returns empty.
- For `i` from `-1` to `lastIndex`:
  1. `before = getOrNull(i)`; if non-null add it.
  2. `after = getOrNull(i + 1)`.
  3. `separator = generator(before, after)`; if non-null add it.

So the generator is called for: `(null, first)`, then `(first, second)`, ..., `(last-1, last)`, then `(last, null)`. Each separator lands between its `before` and `after`.

### 4.2 Generator (AnimeScreenModel.kt:1636-1659), exact

```
processedEpisodes.insertSeparators { before, after ->
    val (lowerEpisode, higherEpisode) = if (anime.sortDescending()) {
        after to before
    } else {
        before to after
    }
    if (higherEpisode == null) return@insertSeparators null

    if (lowerEpisode == null) {
        floor(higherEpisode.episode.episodeNumber).toInt().minus(1).coerceAtLeast(0)
    } else {
        calculateEpisodeGap(higherEpisode.episode, lowerEpisode.episode)
    }
        .takeIf { it > 0 }
        ?.let { missingCount ->
            EpisodeList.MissingCount(
                id = "${lowerEpisode?.id}-${higherEpisode.id}",
                count = missingCount,
            )
        }
}
```

Derivation:
- Ascending display: `lower = before`, `higher = after`.
- Descending display: `lower = after`, `higher = before`. The higher-numbered episode is the one displayed first.
- The null-`higher` case (end of list in the "higher" direction) yields no separator.
- When `lower` is null (there is no lower-numbered neighbour), the missing count is `floor(higher) - 1`, clamped at 0. In ascending display this is the gap before the first item (`(null, first)`). In descending display it is the gap after the last item (`(last, null)`).
- Otherwise the gap is `calculateEpisodeGap(higher, lower)`.
- A separator is produced only when the count is > 0.

### 4.3 Gap calculation (MissingEpisodes.kt:40-49)

- `calculateEpisodeGap(higherEpisode: Episode?, lowerEpisode: Episode?)`: returns 0 if either is null, or either `!isRecognizedNumber`; otherwise `calculateEpisodeGap(higher.episodeNumber, lower.episodeNumber)`.
- `calculateEpisodeGap(higherNumber: Double, lowerNumber: Double)`: returns 0 if either is < 0; otherwise `floor(higher).toInt() - floor(lower).toInt() - 1`.

Examples:
- Episodes 3 and 5 adjacent: gap = 5 - 3 - 1 = 1 (episode 4 is missing).
- Episodes 3 and 3.5: gap = 3 - 3 - 1 = -1, so no separator. Fractional episodes do not create gaps.
- Episodes 2 and 2.5 then 4: gap between 2.5 and 4 = floor(4) - floor(2.5) - 1 = 4 - 2 - 1 = 1 (episode 3 missing).

### 4.4 `missingEntriesCount()` (MissingEpisodes.kt:6-38)

A separate function in the same file that counts missing numbers across the whole list (ignores `-1.0`, converts to Int, distinct, sorted, counts gaps from 0). **It is not called from this scope.** It is consumed by the presentation header "Missing N items" (part 1). Its unit test is in `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt` (area 07).

### 4.5 MissingCount identity

`id = "${lowerEpisode?.id}-${higherEpisode.id}"`. When `lower` is null the id begins with `null-`. The id is used as the list key by the presentation. **Proposal:** use `format!("{}-{}", lower_id.map_or("start".into(), ...), higher_id)`, which keeps uniqueness and is stable.

---

## 5. Selection

### 5.1 State

- `selectedPositions: Array<Int> = arrayOf(-1, -1)` (AnimeScreenModel.kt:184) -- the first and last selected index into `processedEpisodes` (the list the user sees, after filters).
- `selectedEpisodeIds: HashSet<Long>` (line 185) -- the global selection set by episode id.
- Every `Item.selected` flag is kept in sync with `selectedEpisodeIds` when items are created (`toEpisodeListItems`, line 637).

Note the two sources of truth for "selected": the flag on each item and the id set. `toggleAllSelection` and `invertSelection` update both. `toggleSelection` updates both for the single item and, for range selection, for each in-between item (lines 1408-1409).

### 5.2 `toggleSelection(item, selected, userSelected = false, fromLongPress = false)` (1369-1431), exact

Runs inside `updateSuccessState`. The lambda is `successState -> ...`.

```
L = successState.processedEpisodes.toMutableList()            // filtered + sorted view
selectedIndex = processedEpisodes.indexOfFirst { it.id == item.episode.id }
if selectedIndex < 0: return (no change)
selectedItem = L[selectedIndex]
if (selectedItem.selected && selected) || (!selectedItem.selected && !selected): return (no change)
firstSelection = L.none { it.selected }        // computed BEFORE the set
L[selectedIndex] = selectedItem.copy(selected = selected)
selectedEpisodeIds.addOrRemove(item.id, selected)

if selected && userSelected && fromLongPress:
    if firstSelection:
        pos[0] = selectedIndex; pos[1] = selectedIndex
    else:
        if selectedIndex < pos[0]:
            range = (selectedIndex + 1) until pos[0]      // exclusive upper bound
            pos[0] = selectedIndex
        else if selectedIndex > pos[1]:
            range = (pos[1] + 1) until selectedIndex
            pos[1] = selectedIndex
        else:
            range = EMPTY                                  // inside the span: only itself
        for i in range:
            inbetween = L[i]
            if !inbetween.selected:
                selectedEpisodeIds.add(inbetween.id)
                L[i] = inbetween.copy(selected = true)

else if userSelected && !fromLongPress:
    if !selected:
        if selectedIndex == pos[0]: pos[0] = L.indexOfFirst { it.selected }
        else if selectedIndex == pos[1]: pos[1] = L.indexOfLast { it.selected }
    else:
        if selectedIndex < pos[0]: pos[0] = selectedIndex
        else if selectedIndex > pos[1]: pos[1] = selectedIndex

return successState.copy(episodes = L)
```

Observed properties:
1. Long-press selection extends the selection between the current anchors and the tapped item, in display order, and marks the in-between items selected.
2. A plain tap (`userSelected && !fromLongPress`) only moves the anchors; it does not select in-between items.
3. Long-press deselect (`userSelected && fromLongPress && !selected`) does not update the anchors (no branch). The anchors may then point at an unselected item. **Proposal:** keep this (it is harmless because the next long-press re-derives from `firstSelection`), or recompute anchors from the selected set; document the choice.
4. `firstSelection` is computed on the list before the tapped item is changed. Consequence: if the tapped item is the only one selected, `firstSelection` is false on a second long-press elsewhere (anchors are then used).
5. The anchor indices refer to `processedEpisodes`. If filters change after anchors are set, the anchors are stale (see edge cases in `p2-edge-cases-and-tests.md`).
6. **DEFECT:** the return writes `episodes = L`, the filtered list. See 23.2 in `p2-behavior.md`.

### 5.3 `toggleAllSelection(selected)` (1433-1443)

```
newEpisodes = successState.episodes.map { selectedEpisodeIds.addOrRemove(it.id, selected); it.copy(selected = selected) }
pos = [-1, -1]
successState.copy(episodes = newEpisodes)
```

Operates on the **full** `episodes` list (not the filtered view). So "select all" also selects episodes hidden by filters. Callers: `markEpisodesSeen`, `bookmarkEpisodes`, `fillermarkEpisodes`, `downloadEpisodes`, and `onAllEpisodeSelected`.

### 5.4 `invertSelection()` (1445-1455)

Same shape. For each item in `episodes`: `addOrRemove(it.id, !it.selected)` and `copy(selected = !it.selected)`. Anchors reset to `-1, -1`. Operates on the full list.

### 5.5 Clearing selection

`toggleAllSelection(false)` is called from: `markEpisodesSeen` (first statement, line 869), `bookmarkEpisodes` (958), `fillermarkEpisodes` (972), `downloadEpisodes` (944), and the delete confirm in AnimeScreen.kt:255.

### 5.6 Selection and bulk action inputs

The scope's bulk actions take `List<Episode>` (mark seen, bookmark, fillermark, delete) or `List<EpisodeList.Item>` (download actions). The presentation (part 1) decides which items to pass. This scope does not compute "the selected list" itself. **Proposal:** expose `selected_items()` from the model as `episodes.filter(selected)` on the full list, which matches `toggleAllSelection`, and document that filtered-out selected items are included.

---

## 6. Next unseen ("continue watching")

### 6.1 Overloads (EpsiodeGetNextUnseen.kt:12-31)

```
fun List<Episode>.getNextUnseen(anime, downloadManager): Episode? {
    return applyFilters(anime, downloadManager).let { episodes ->
        if (anime.sortDescending()) episodes.findLast { !it.seen } else episodes.find { !it.seen }
    }
}
fun List<EpisodeList.Item>.getNextUnseen(anime): Episode? {
    return applyFilters(anime).let { episodes ->
        if (anime.sortDescending()) episodes.findLast { !it.episode.seen } else episodes.find { !it.episode.seen }
    }?.episode
}
```

Algorithm: filter and sort as in sections 2-3, then take the first unseen in display order when ascending, or the last unseen when descending. For the default (SOURCE + DESC) that is the last unseen in the display list, which is the oldest unseen in source terms (display is newest first). So "continue" resumes the oldest unseen episode. That matches the intent ("next to watch").

### 6.2 Entry points

- `AnimeScreenModel.getNextUnseenEpisode()` (non-suspend, 757-760): uses `successState.episodes` (full list) with the `EpisodeList` overload.
- `AnimeScreenModel.getNextUnseenEpisode(anime)` (suspend, 750-752): loads `getEpisodesByAnimeId.await(anime.id)` and uses the `Episode` overload with `downloadManager`. Used for season cards and the library.

### 6.3 Downloaded-only interaction

The `Episode` overload uses `applyFilters(anime, downloadManager)`, so with the global downloaded-only override (3.2) the candidate set is restricted to downloaded episodes. The `EpisodeList` overload uses `it.isDownloaded`, which reflects the queue and cache. Both behave the same for the default settings.

---

## 7. Next-N and unseen download selection

- `getUnseenEpisodes()` (762-768): `processedEpisodes.filter { !episode.seen && dlStatus == NOT_DOWNLOADED }.map { episode }`. Display order, filtered view.
- `getUnseenEpisodesSorted()` (770-774): `getUnseenEpisodes().sortedWith(getEpisodeSort(anime))`, reversed when `anime.sortDescending()`.

For a descending display, the resulting list is oldest-unseen first (see 6.1). `take(N)` therefore downloads the next N episodes to be watched. **Proposal:** keep.

---

## 8. Episode row state derivation

Per item, the presentation needs:
- `downloadState` (from `toEpisodeListItems`, section 13.5 of `p2-behavior.md`).
- `downloadProgress` (0..100 int from the downloader; `AnimeDownload.progress`).
- `selected`.
- `episode.seen`, `episode.lastSecondSeen` (for "partially seen" progress), `episode.bookmark`, `episode.fillermark`.

Visible grouping:
- Previews and summaries are per-entry flags (`showPreviews()` at Anime.kt:113-115; `showSummaries()` at 120-122). When either is enabled, the row uses the rich layout (part 1).
- Display name vs number: `displayMode` (Anime.kt:58-59, 254-255). Display NUMBER is `EPISODE_DISPLAY_NUMBER = 0x100000`. The ShowQualities dialog title uses `AYMR.strings.display_mode_episode` ("Episode %1$s") when display is NUMBER, else `episode.name` (AnimeScreen.kt:391-398).

---

## 9. Edge cases (summary; full list in `p2-edge-cases-and-tests.md`)

1. Empty list: `insertSeparators` returns empty (CollectionUtils.kt:11). `episodeListItems` is empty and no separator is shown.
2. Single item: generator called for `(null, x)` and `(x, null)` only. For ascending, a missing count before it is possible (`floor(x) - 1`). For descending, a missing count after it is possible.
3. Episode number `-1.0` (unknown): `calculateEpisodeGap` returns 0 on either side; the leading/trailing branch clamps `floor(-1) - 1 = -2` to 0. No separator.
4. Duplicate numbers (two episodes numbered 5): gap = 5 - 5 - 1 = -1, no separator.
5. Fractional numbers (5.5 between 5 and 6): gap = 6 - 5 - 1 = 0, no separator.
6. Filters that remove every item: `processedEpisodes` empty, separators empty, `getNextUnseen` null.
7. `sortedWith` on a Sequence is evaluated when `toList()` is called in `processedEpisodes`; the lazy `by lazy` caches per State instance. A new State (copy) recomputes.
8. Alphabet sort with equal names: order is the input order (stable).

---

## 10. Rust mapping

| Kotlin | Rust |
|---|---|
| `Anime.EPISODE_*` constants | `kuro_core::anime::flags::episode::{SORT_DESC, SORT_ASC, SORT_DIR_MASK, SORTING_SOURCE, SORTING_NUMBER, SORTING_UPLOAD_DATE, SORTING_ALPHABET, SORTING_MASK, SHOW_UNSEEN, ...}` as `u64` consts |
| `TriState` | `enum TriState { Disabled, EnabledIs, EnabledNot }` |
| `getEpisodeSort` | `fn episode_sort(key: SortKey, desc: bool) -> impl Fn(&Episode, &Episode) -> Ordering` |
| `applyFilter` | `fn apply(filter: TriState, pred: impl FnOnce() -> bool) -> bool` |
| `applyFilters` | `fn process(items: &[EpisodeItem], anime: &AnimeView) -> Vec<EpisodeItem>` (pure) |
| `insertSeparators` | `fn with_separators<T, R>(items: &[T], gen: impl FnMut(Option<&T>, Option<&T>) -> Option<R>) -> Vec<R>` |
| `calculateEpisodeGap` | `fn episode_gap(higher: &Episode, lower: &Episode) -> i64` |
| `selectedPositions`, `selectedEpisodeIds` | a `Selection { anchors: [i64; 2], ids: HashSet<i64> }` owned by the model (section 5 algorithm verbatim) |
| `getNextUnseen` | `fn next_unseen(items: &[EpisodeItem], anime: &AnimeView) -> Option<&Episode>` |
| collator | `icu_collator::Collator` with `PRIMARY` strength |

---

## 11. Tests required (summary; full in `p2-edge-cases-and-tests.md`)

- Sort: each key x direction, using the table in 2.2 as the oracle, including the SOURCE inversion.
- Filter: each tri-state x predicate; downloaded-only override; local anime override.
- Separators: ascending and descending, leading and trailing, `-1.0` numbers, duplicates, fractional, empty, single.
- Selection: each branch of section 5.2, with a table of (anchors, action) -> (selected set, anchors).
- Next unseen: both overloads, ascending and descending, all-seen, none-seen.
