# Topic: seasons (entries whose `fetchType` is `Seasons`)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt` (lines 642-653, 670-702, 1143-1367, 1627-1629, 1701-1737), `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeSeasonItem.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt` (lines 212-214, 215-222, 287-316 and 297-315 in the dialog host).
Callees: `domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt`, `domain/src/main/java/tachiyomi/domain/items/season/service/SeasonSorter.kt`, `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeSeasonFlags.kt`, `domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt`, `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt` (season getters lines 124-208, season constants lines 257-308), `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` (season prefs lines 262-335, 353-367), `domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt`, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt`, `domain/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt` (the file is at `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt`), `data/src/main/sqldelightanime/view/animeseasonsView.sq`, `data/src/main/sqldelightanime/view/animeseasonstatsView.sq`.

---

## 1. What a season is

- A season is an `animes` row whose `parent_id` is the parent entry's `_id` (animes.sq:34, index `animes_parent_id` at line 45).
- Season rows carry their own `source`, `url`, `title`, `thumbnail_url`, `status`, `fetch_type`, plus `season_number` (REAL, NOT NULL, animes.sq:36) and `season_source_order` (INTEGER NOT NULL, animes.sq:37).
- A season's `fetchType` is `Episodes` (it has an episode list, fetched on demand) for the normal case. The view also handles `fetch_type = 0` children (the `ASS` branch of the view, section 1 table), but this screen lists only the direct children of the open parent (`WHERE parent_id = :parentId`), so nested-season display is not implemented here and is not part of this scope.
- The parent has `fetchType = Seasons`. Season flags (`seasonFlags`) live on the parent row, because `SetAnimeSeasonFlags` writes `anime.seasonFlags` for the anime whose screen is open (SetAnimeSeasonFlags.kt:11-189).

Season listing query (animeseasonsView.sq:41-44): `SELECT * FROM animeseasonsView WHERE parent_id = :parentId;`. The view (animeseasonsView.sq:1-39) is `animes` joined to three aggregates:

| View column | Source when `M.fetch_type = 1` (season has episodes) | Source when `M.fetch_type = 0` (season has nested seasons) |
|---|---|---|
| `totalCount` | `ES.total` (episodestatsView: `count(*)`) | `ASS.child_count` |
| `seenCount` | `ES.seenCount` (`sum(seen)`) | `ASS.fully_seen_seasons` |
| `latestUpload` | `ES.latestUpload` (`max(date_upload)`, default 0) | `ASS.max_latest_upload` |
| `episodeFetchedAt` | `ES.fetchedAt` (`max(date_fetch)`, default 0) | `ASS.max_fetched_at` |
| `lastSeen` | `AHS.lastSeen` (`max(animehistory.last_seen)` per anime) | `ASS.max_last_seen` |
| `bookmarkCount` | `ES.bookmarkCount` (`sum(bookmark)`) | `ASS.total_bookmarks` |
| `fillermarkCount` | `ES.fillermarkCount` (`sum(fillermark)`) | `ASS.total_fillermarks` |

`coalesce(..., 0)` is applied to every value. The `episodestatsView` is at data/src/main/sqldelightanime/view/episodestatsView.sq:1-12. The `animehistorystatsView` is at animehistorystatsView.sq:1-7. The `animeseasonstatsView` provides the `ASS` aggregates.

The mapping to `SeasonAnime` (domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt:6-44):

| Field | Type | Meaning |
|---|---|---|
| `anime` | `Anime` | the season row |
| `totalCount` | Long | episodes (or child seasons) |
| `seenCount` | Long | seen episodes (or fully seen child seasons) |
| `bookmarkCount` | Long | bookmarked episodes |
| `fillermarkCount` | Long | fillermarked episodes |
| `latestUpload` | Long | max upload date |
| `fetchedAt` | Long | max fetch date |
| `lastSeen` | Long | last seen timestamp |

Derived (SeasonAnime.kt:16-30):
- `id` = `anime.id`.
- `seen` = `totalCount == seenCount`. A season with zero episodes is therefore `seen == true`.
- `unseenCount` = `totalCount - seenCount`.
- `hasStarted` = `seenCount > 0`.
- `hasBookmarks` = `bookmarkCount > 0`; `hasFillermarks` = `fillermarkCount > 0`.

---

## 2. Season flag layout (`Anime.kt:257-308`, `seasonFlags`)

| Bits | Mask | Meaning | Values |
|---|---|---|---|
| 0 | `SEASON_SORT_DIR_MASK = 0x1` | direction | `SEASON_SORT_DESC = 0`, `SEASON_SORT_ASC = 1` |
| 1-2 | `SEASON_DOWNLOADED_MASK = 0x6` | downloaded filter | `SEASON_SHOW_DOWNLOADED = 0x2`, `SEASON_SHOW_NOT_DOWNLOADED = 0x4` |
| 3-4 | `SEASON_UNSEEN_MASK = 0x18` | unseen filter | `SEASON_SHOW_UNSEEN = 0x8`, `SEASON_SHOW_SEEN = 0x10` |
| 5-6 | `SEASON_STARTED_MASK = 0x60` | started filter | `SEASON_SHOW_STARTED = 0x20`, `SEASON_SHOW_NOT_STARTED = 0x40` |
| 7-8 | `SEASON_COMPLETED_MASK = 0x180` | completed filter | `SEASON_SHOW_COMPLETED = 0x80`, `SEASON_SHOW_NOT_COMPLETED = 0x100` |
| 9-10 | `SEASON_BOOKMARKED_MASK = 0x600` | bookmarked filter | `SEASON_SHOW_BOOKMARKED = 0x200`, `SEASON_SHOW_NOT_BOOKMARKED = 0x400` |
| 11-12 | `SEASON_FILLERMARKED_MASK = 0x1800` | fillermarked filter | `SEASON_SHOW_FILLERMARKED = 0x800`, `SEASON_SHOW_NOT_FILLERMARKED = 0x1000` |
| 13-15 | `SEASON_SORT_MASK = 0xE000` | sort key | `SEASON_SORT_SOURCE = 0x0`, `SEASON_SORT_SEASON = 0x2000`, `SEASON_SORT_UPLOAD = 0x4000`, `SEASON_SORT_ALPHABET = 0x6000`, `SEASON_SORT_COUNT = 0x8000`, `SEASON_SORT_LAST_SEEN = 0xA000`, `SEASON_SORT_FETCHED = 0xC000` |
| 16-17 | `SEASON_GRID_DISPLAY_MODE_MASK = 0x30000` | grid mode (`shr 16`) | 0 CompactGrid, 1 ComfortableGrid, 2 CoverOnlyGrid, 3 List |
| 18-21 | `SEASON_GRID_DISPLAY_SIZE_MASK = 0x3C0000` | grid items per row (`shr 18`) | 0..15 |
| 22 | `SEASON_OVERLAY_DOWNLOADED_MASK = 0x400000` | overlay: downloaded count | boolean |
| 23 | `SEASON_OVERLAY_UNSEEN_MASK = 0x800000` | overlay: unseen count | boolean |
| 24 | `SEASON_OVERLAY_LOCAL_MASK = 0x1000000` | overlay: local badge | boolean |
| 25 | `SEASON_OVERLAY_LANG_MASK = 0x2000000` | overlay: language badge | boolean |
| 26 | `SEASON_OVERLAY_CONT_MASK = 0x4000000` | overlay: continue button | boolean |
| 27 | `SEASON_DISPLAY_MODE_MASK = 0x8000000` | display: title source vs number | `SEASON_DISPLAY_MODE_SOURCE = 0`, `SEASON_DISPLAY_MODE_NUMBER = 0x8000000` |

The filter constants are `SHOW_ALL = 0` for DISABLED. Each filter field is a mask field, and writing one filter never changes another (SetAnimeSeasonFlags.kt:167-174 `setFlag` uses `this and mask.inv() or (flag and mask)`).

Getters on `Anime` (Anime.kt:124-208):
- `seasonUnseenFilter` etc.: `seasonXFilterRaw` mapped to `TriState` (`ENABLED_IS` for the "show" constant, `ENABLED_NOT` for the "not" constant, `DISABLED` otherwise).
- `seasonSorting` = `seasonFlags & SEASON_SORT_MASK`.
- `seasonSortDescending()` = `seasonFlags & SEASON_SORT_DIR_MASK == SEASON_SORT_DESC`.
- `seasonDisplayGridMode` = `SeasonDisplayMode.fromLong((flags & 0x30000) shr 16)`.
- `seasonDisplayGridSize` = `((flags & 0x3C0000) shr 18).toInt()`.
- Overlay getters are `(flags & MASK) == MASK` (boolean, Anime.kt:192-205).
- `seasonDisplayMode` = `flags & 0x8000000` (raw Long, Anime.kt:207-208).

### 2.1 SeasonDisplayMode (domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt:3-28)

`sealed interface` with `CompactGrid`, `ComfortableGrid`, `CoverOnlyGrid`, `List`. `toLong` gives 0, 1, 2, 3. `fromLong` maps 0-3, throws `IllegalArgumentException("Invalid sorting value")` otherwise (the message is wrong; observed, not to be copied).

---

## 3. Season filter and sort pipeline

### 3.1 `applySeasonFilters` (AnimeScreenModel.kt:1701-1737), exact order

```
unseenFilter = anime.seasonUnseenFilter
downloadedFilter = anime.seasonDownloadedFilter        // includes the downloadedOnly override (Anime.kt:23-31)
startedFilter = anime.seasonStartedFilter
completedFilter = anime.seasonCompletedFilter
bookmarkedFilter = anime.seasonBookmarkedFilter
fillermarkedFilter = anime.seasonFillermarkedFilter

comparator = getSeasonSortComparator(anime)
    .let { if (anime.seasonSortDescending()) it.reversed() else it }
    .thenComparator(seasonSortAlphabetically)

asSequence()
  .filter { (season) -> applyFilter(unseenFilter) { !season.seen } }
  .filter { (season) -> applyFilter(startedFilter) { season.hasStarted } }
  .filter { (season) -> applyFilter(completedFilter) { season.anime.status.toInt() == SAnime.COMPLETED } }
  .filter { (season) -> applyFilter(bookmarkedFilter) { season.hasBookmarks } }
  .filter { (season) -> applyFilter(fillermarkedFilter) { season.hasFillermarks } }
  .filter { applyFilter(downloadedFilter) { it.downloadCount > 0 || it.seasonAnime.anime.isLocal() } }
  .sortedWith(compareBy(comparator) { it.seasonAnime })
  .map { AnimeSeasonItem(seasonAnime, downloadCount = if (overlayDownloaded) it.downloadCount else -1L, unseenCount = if (overlayUnseen) it.unseenCount else -1L, isLocal = overlayLocal && it.isLocal, sourceLanguage = if (overlayLang) it.sourceLanguage else "", showContinueOverlay = overlayContinue && it.unseenCount > 0 && itemAnime.fetchType == FetchType.Episodes) }
```

Notes:
- `season.seen` is `totalCount == seenCount` (SeasonAnime.kt:18-19).
- `completedFilter` tests `season.anime.status.toInt() == SAnime.COMPLETED` (= 2). `status` is the stored Long.
- The downloaded filter uses the **unclamped** `it.downloadCount` (the overlay-gated value is only set in `.map`).
- `showContinueOverlay` uses `it.unseenCount` (unclamped).
- The alphabetical tie-break is always ascending, even when the primary key is reversed (`thenComparator` is applied after the reversal).

### 3.2 Sort comparator (SeasonSorter.kt:11-35), exact

`getSeasonSortComparator(anime)` (ascending for each key):

| `anime.seasonSorting` | Comparison |
|---|---|
| `SEASON_SORT_SOURCE` (0) | `s1.anime.seasonSourceOrder.compareTo(s2.anime.seasonSourceOrder)` |
| `SEASON_SORT_SEASON` (0x2000) | `s1.anime.seasonNumber.compareTo(s2.anime.seasonNumber)` (Double) |
| `SEASON_SORT_UPLOAD` (0x4000) | `s1.latestUpload.compareTo(s2.latestUpload)` |
| `SEASON_SORT_ALPHABET` (0x6000) | `seasonSortAlphabetically(s1, s2)` |
| `SEASON_SORT_COUNT` (0x8000) | `s1.unseenCount.compareTo(s2.unseenCount)` |
| `SEASON_SORT_LAST_SEEN` (0xA000) | `s1.lastSeen.compareTo(s2.lastSeen)` |
| `SEASON_SORT_FETCHED` (0xC000) | `s1.fetchedAt.compareTo(s2.fetchedAt)` |
| other | `throw NotImplementedError("Invalid season sorting method: ...")` |

`seasonSortAlphabetically` (SeasonSorter.kt:7-9): `i1.anime.title.lowercase().compareToWithCollator(i2.anime.title.lowercase())`.

Default: `seasonSorting = SOURCE` and `seasonSortDescending = true` (flags 0). The comparator is therefore reversed: **season source order is descending by default**, the opposite of the episode default (see `p2-topic-episode-list.md` section 2.2 for episodes, which are ascending on the raw key). **DEFECT (flagged, to be verified in the running app):** the two defaults produce opposite on-screen orders for "source" sorting. Proposal: define one rule for both and document it.

### 3.3 Season sort setter (SetAnimeSeasonFlags.kt:35-58)

Same shape as episodes: same key flips direction; a new key starts ascending. Flags:
- New key: `setFlag(flag, SEASON_SORT_MASK)` and `setFlag(SEASON_SORT_ASC, SEASON_SORT_DIR_MASK)`.
- Same key: `setFlag(orderFlag, SEASON_SORT_DIR_MASK)` where `orderFlag = if (seasonSortDescending()) SEASON_SORT_ASC else SEASON_SORT_DESC`.

---

## 4. Season setters (AnimeScreenModel.kt:1143-1367)

Each setter: `val anime = successState?.anime ?: return` then `launchNonCancellable { setAnimeSeasonFlags.awaitX(anime, flag) }`. No optimistic state.

| Method | Input | Flag mapping | Interactor |
|---|---|---|---|
| `setSeasonDownloadedFilter(TriState)` (1143-1155) | DISABLED / IS / NOT | 0 / `SEASON_SHOW_DOWNLOADED` / `SEASON_SHOW_NOT_DOWNLOADED` | `awaitSetDownloadedFilter` (mask 0x6) |
| `setSeasonUnseenFilter(TriState)` (1161-1173) | | 0 / `SEASON_SHOW_UNSEEN` / `SEASON_SHOW_SEEN` | `awaitSetUnseenFilter` (mask 0x18) |
| `setSeasonStartedFilter(TriState)` (1179-1191) | | 0 / `SEASON_SHOW_STARTED` / `SEASON_SHOW_NOT_STARTED` | `awaitSetStartedFilter` (mask 0x60) |
| `setSeasonCompletedFilter(TriState)` (1233-1245) | | 0 / `SEASON_SHOW_COMPLETED` / `SEASON_SHOW_NOT_COMPLETED` | `awaitSetCompletedFilter` (mask 0x180) |
| `setSeasonBookmarkedFilter(TriState)` (1197-1209) | | 0 / `SEASON_SHOW_BOOKMARKED` / `SEASON_SHOW_NOT_BOOKMARKED` | `awaitSetBookmarkedFilter` (mask 0x600) |
| `setSeasonFillermarkedFilter(TriState)` (1215-1227) | | 0 / `SEASON_SHOW_FILLERMARKED` / `SEASON_SHOW_NOT_FILLERMARKED` | `awaitSetFillermarkedFilter` (mask 0x1800) |
| `setSeasonSorting(sort: Long)` (1251-1257) | sort key | value | `awaitSetSortingModeOrFlipOrder` |
| `setSeasonDisplayGridMode(mode: SeasonDisplayMode)` (1263-1269) | mode | `toLong(mode) shl 16` | `awaitSetGridMode` (mask 0x30000) |
| `setSeasonDisplayGridSize(size: Int)` (1275-1281) | 0..15 | `size shl 18` | `awaitSetGridSize` (mask 0x3C0000) |
| `setSeasonDownloadOverlay(Boolean)` (1287-1293) | | bit 22 | `awaitSetDownloadedOverlay` |
| `setSeasonUnseenOverlay(Boolean)` (1299-1305) | | bit 23 | `awaitSetUnseenOverlay` |
| `setSeasonLocalOverlay(Boolean)` (1311-1317) | | bit 24 | `awaitSetLocalOverlay` |
| `setSeasonLangOverlay(Boolean)` (1323-1329) | | bit 25 | `awaitSetLangOverlay` |
| `setSeasonContinueOverlay(Boolean)` (1335-1341) | | bit 26 | `awaitSetContinueOverlay` |
| `setSeasonDisplayMode(mode: Long)` (1347-1353) | `SEASON_DISPLAY_MODE_*` | value | `awaitSetDisplayMode` (mask 0x8000000) |

`setBooleanFlag(anime, value, mask)`: `flag = if (value) mask else 0` (SetAnimeSeasonFlags.kt:176-184).

### 4.1 Season settings default (`setSeasonCurrentSettingsAsDefault`, 1355-1367)

1. `libraryPreferences.setSeasonSettingsDefault(anime)` (LibraryPreferences.kt:326-347):
   - `filterSeasonByDownload().set(anime.seasonUnseenFilterRaw)` **<- DEFECT: writes the unseen raw value into the downloaded-filter preference.** The correct source is `seasonDownloadedFilterRaw`. (See behavior 23.9.)
   - `filterSeasonByUnseen` <- `seasonUnseenFilterRaw`; `filterSeasonByStarted` <- `seasonStartedFilterRaw`; `filterSeasonByCompleted` <- `seasonCompletedFilterRaw`; `filterSeasonByBookmarked` <- `seasonBookmarkedFilterRaw`; `filterSeasonByFillermarked` <- `seasonFillermarkedFilterRaw`.
   - `sortSeasonBySourceOrNumber` <- `seasonSorting`; `sortSeasonByAscendingOrDescending` <- `if (seasonSortDescending()) SEASON_SORT_DESC else SEASON_SORT_ASC`.
   - `seasonDisplayGridMode` <- `toLong(seasonDisplayGridMode)`; `seasonDisplayGridSize` <- `seasonDisplayGridSize`.
   - `seasonDownloadOverlay`, `seasonUnseenOverlay`, `seasonLocalOverlay`, `seasonLangOverlay`, `seasonContinueOverlay` <- the booleans.
   - `seasonDisplayMode` <- `seasonDisplayMode`.
2. If `applyToExisting`: `setAnimeDefaultSeasonFlags.awaitAll()` (copies to every favorite).
3. Snackbar `AYMR.strings.season_settings_updated` ("Updated default season settings").

Season defaults (LibraryPreferences.kt:262-335):

| Preference key | Default |
|---|---|
| `default_season_filter_by_downloaded` | `SHOW_ALL` (0) |
| `default_season_filter_by_unseen` | `SHOW_ALL` |
| `default_season_filter_by_started` | `SHOW_ALL` |
| `default_season_filter_by_completed` | `SHOW_ALL` |
| `default_season_filter_by_bookmarked` | `SHOW_ALL` |
| `default_season_filter_by_fillermarked` | `SHOW_ALL` |
| `default_season_sort_by_source_or_number` | `SEASON_SORT_SOURCE` (0) |
| `default_season_sort_by_ascending_or_descending` | `SEASON_SORT_DESC` (0) |
| `default_season_grid_display_mode` | `SeasonDisplayMode.toLong(CompactGrid)` (0) |
| `default_season_grid_display_size` | 0 (Int) |
| `default_season_download_overlay` | false |
| `default_season_unseen_overlay` | true |
| `default_season_local_overlay` | true |
| `default_season_lang_overlay` | false |
| `default_season_continue_overlay` | true |
| `default_season_display_mode` | `SEASON_DISPLAY_MODE_SOURCE` (0) |

`SetAnimeDefaultSeasonFlags.await(anime)` (domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt:15-39) writes all of these in one `awaitSetAllFlags` call (SetAnimeSeasonFlags.kt:104-165), building the value from zero: every bit not listed is cleared.

---

## 5. Season list model (`toAnimeSeasonItems`, AnimeScreenModel.kt:642-653)

```
seasonAnime -> AnimeSeasonItem(
    seasonAnime = seasonAnime,
    downloadCount = downloadManager.getDownloadCount(seasonAnime.anime).toLong(),
    unseenCount = seasonAnime.unseenCount,
    isLocal = seasonAnime.anime.isLocal(),
    sourceLanguage = sourceManager.getOrStub(seasonAnime.anime.source).lang,
    showContinueOverlay = false,
)
```

- `downloadManager.getDownloadCount(anime)` (AnimeDownloadManager.kt:228-238): for local season (source `0`) counts archive files in the anime directory; otherwise `cache.getDownloadCount(anime)`.
- `sourceManager.getOrStub(...).lang` is the source language code (for the language badge).
- The season list is recomputed only when the DB subscription emits (init subscription, AnimeScreenModel.kt:216-217). `updateDownloadState` (595-607) touches only `episodes`, never `seasons`. Downloads of season children therefore do not refresh the season download count until the next DB emission. **Proposal:** recompute season counts when the download queue or cache changes.

`AnimeSeasonItem` (AnimeSeasonItem.kt:5-12): `seasonAnime`, `downloadCount = -1L`, `unseenCount = -1L`, `isLocal = false`, `sourceLanguage = ""`, `showContinueOverlay = false`. The `-1` defaults mean "hidden". The `.map` in `applySeasonFilters` is the only place overlays are applied.

---

## 6. Opening a season

- Card tap: `onSeasonClicked = { navigator.push(AnimeScreen(it.id)) }` (AnimeScreen.kt:212-214). `it` is `SeasonAnime` (the presentation passes `SeasonAnime`, see AnimeScreen.kt:162 in part 1). `AnimeScreen(id)` is constructed with `fromSource = false` (default). The season's own screen then loads the season row from the DB (it is in the library only if it was favorited, otherwise `fromSource` is false and `init` applies the non-library default flags, see behavior 2.3.5).
- Continue overlay: `onContinueWatchingClicked = { scope.launchIO { val episode = screenModel.getNextUnseenEpisode(it.anime); episode?.let { openEpisode(...) } } }` (AnimeScreen.kt:215-222). It opens the next unseen episode of the season directly, without navigating.

---

## 7. Season refresh (summary)

Covered in `p2-behavior.md` section 6.4. Key points for seasons:
- `awaitSeasonsUpdate` returns **all** season anime, not only new ones (SyncSeasonsWithSource.kt:90 and :107).
- Episode fetch for each season runs only when `updateSeasonOnRefresh()` is true (default false, LibraryPreferences.kt:353-354, key `pref_update_season_on_refresh`). The per-season filter is `fetchType == Episodes && (lastUpdate == 0 || status != COMPLETED)` (AnimeScreenModel.kt:676).
- `UnmeteredSource` seasons are fetched in parallel (AnimeScreenModel.kt:690-695); others sequentially.
- New episodes of child seasons are not downloaded (23.7).
- Library update of seasons: `updateSeasonOnLibraryUpdate()` (key `pref_update_season_on_library_update`, default false, LibraryPreferences.kt:357-359). Owned by area 26-library-update-jobs.

---

## 8. Season dialog host (AnimeScreen.kt:297-316)

`SeasonSettingsDialog` receives one callback per setter in section 4 (`onDownloadFilterChanged`, `onUnseenFilterChanged`, `onStartedFilterChanged`, `onCompletedFilterChanged`, `onBookmarkedFilterChanged`, `onFillermarkedFilterChanged`, `onSortModeChanged`, `onDisplayGridModeChanged`, `onDisplayGridSizeChanged`, `onOverlayDownloadedChanged`, `onOverlayUnseenChanged`, `onOverlayLocalChanged`, `onOverlayLangChanged`, `onOverlayContinueChanged`, `onDisplayModeChanged`, `onSetAsDefault`). The sheet itself is part 1 (`presentation/entries/anime/SeasonSettingsDialog.kt`, 340 lines).

Season sheet content as specified by the mockup spec (spec/entries.json, `season_settings`): Filter (Downloaded, Unseen, Started, Completed, Bookmarked, Fillermarked; tristate), Sort (`By source`, `By season number`, `By upload date`, `Alphabetically`, `Unseen count`, `Last seen`, `Episode fetch date`), Display (`Display mode` chips Compact grid / Comfortable grid / Cover-only grid / List; `Items per row` slider 0), Overlay (Downloaded episodes, Unseen episodes, Local source, Language, Continue watching button), Grid display mode (`Source title`, `Season number`). These labels come from the part-1 presentation and are listed here only to make the mockup gap explicit (see `p2-ui-parity.md`).

---

## 9. Rust mapping

| Kotlin | Rust |
|---|---|
| `Anime.season*` constants and getters | `kuro_core::anime::flags::season::{...}` plus `SeasonFlags(u64)` newtype with typed getters |
| `SeasonDisplayMode` | `enum SeasonDisplayMode { CompactGrid = 0, ComfortableGrid = 1, CoverOnlyGrid = 2, List = 3 }` with `TryFrom<u64>` (error variant instead of exception) |
| `SeasonAnime` | `struct SeasonAnime { anime: Anime, total: i64, seen: i64, bookmarks: i64, fillermarks: i64, latest_upload: i64, fetched_at: i64, last_seen: i64 }` with `fn seen(&self) -> bool`, `unseen_count`, `has_started`, etc. |
| `AnimeSeasonItem` | `struct SeasonItem { season: SeasonAnime, download_count: i64 /* -1 hidden */, unseen_count: i64, is_local: bool, source_language: String, show_continue: bool }` |
| `applySeasonFilters` | `fn process_seasons(seasons: &[SeasonAnime], anime: &AnimeView, dl: &dyn DownloadCount) -> Vec<SeasonItem>` (pure, same order) |
| `getSeasonSortComparator` | `fn season_comparator(key, desc) -> impl Fn(&SeasonAnime, &SeasonAnime) -> Ordering` plus a final ascending alphabetical comparator with `icu_collator` |
| `SetAnimeSeasonFlags` | `fn set_season_flag(flags: u64, flag: u64, mask: u64) -> u64` and the same `setFlag` semantics |

---

## 10. Tests (summary)

- Flag codec: every setter round-trip; setting one filter does not change others; grid size values 0..15.
- Filter truth table: each of six filters x three states, against fixtures with `total`, `seen`, `bookmarks`, `fillermarks`, `started`.
- Sort: each key x direction; default (SOURCE + desc) gives descending source order; alphabetical tie-break always ascending.
- Overlay gating: each overlay bit off hides the field (`-1` or empty string) and does not affect filters.
- `showContinueOverlay`: requires `unseenCount > 0` and `fetchType == Episodes`.
- Season with zero episodes: `seen == true`, `unseen == 0`.
- Defaults round-trip through `setSeasonSettingsDefault` then `SetAnimeDefaultSeasonFlags`, with the fixed downloaded-filter mapping.
