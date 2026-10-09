# 37-ui-library — Edge cases, failure modes, races, tests

Sections: §E edge cases and defects (numbered E-01 … E-49, referenced from p1-behavior.md and p1-rust-design.md) · §T-1 upstream tests to port · §T-2 new unit tests · §T-3 fixtures · §T-4 integration and UI tests · §T-5 race conditions.

Each E-entry: **Upstream** (what the Kotlin does, cited), **Impact**, **Rust rule** (what the port must do), and **Test** (the test id in §T-2 that pins it, or "manual").

---

## §E. Edge cases and defects

### E-01 `id:` search prefix
- **Upstream:** `constraint.startsWith("id:", true)`; value after `"id:"` parsed with `toLongOrNull()`; a null id matches nothing (`AnimeLibraryItem.kt:25-28`).
- **Impact:** `id:` alone matches nothing; `id: 12` (space) does not parse (`" 12".toLongOrNull()` is null) and matches nothing. Other terms in the same query are ignored.
- **Rust rule:** `Query::parse` checks the 3-byte prefix case-insensitively; parse with `str::parse::<i64>` on the rest without trimming. Same results.
- **Test:** T2-SEARCH-03.

### E-02 Continue button depends on the unread badge
- **Upstream:** the item's `unseenCount` is `if (prefs.unseenBadge) libraryAnime.unseenCount else 0` (`AnimeLibraryScreenModel.kt:395`), and the continue button is shown only when `libraryItem.unseenCount > 0` (`AnimeLibraryList.kt:74`, `AnimeLibraryCompactGrid.kt:66`, `AnimeLibraryComfortableGrid.kt:65`).
- **Impact:** turning off "Unseen episodes" badge also hides every continue button, even with the continue option on.
- **Rust rule (fix):** the continue button uses the raw `row.unseen() > 0`; the badge value is only for display. Document the change in the release notes.
- **Test:** T2-SEL-09 (continue visibility independent of badge).

### E-03 Empty search string
- **Upstream:** `searchQuery != null` triggers filtering; `"".let { matches("") }` → title contains "" is true → all entries pass. The toolbar clears to `null` only through the search toolbar's clear action (area:36).
- **Impact:** an empty-but-non-null query behaves like no query, but the pager shows the search-specific empty message and global-search row if it ever becomes empty.
- **Rust rule:** keep `Some("")` = match-all for parity; the UI maps an empty entry to `None` before calling the pipeline (so the empty-state message stays "Category is empty"). Note: this is a deliberate normalisation.
- **Test:** T2-SEARCH-06.

### E-04 Bare minus sign in search
- **Upstream:** `"-"` → part `Negated("")` → `!predicate("")` → `!(sourceName.contains("") || genres.any{ equals "" })` → `!true` = false. So `"-"` matches only entries whose title/author/artist/description contains `"-"` (the whole-string path).
- **Impact:** typing a lone hyphen empties the grid unless titles contain hyphens.
- **Rust rule:** same algorithm; the UI shows the hint "Use -term to exclude" under the entry when the query starts with "-". Decided: keep semantics (T-37a-012 acceptance).
- **Test:** T2-SEARCH-07.

### E-05 Comma handling and whole-string match
- **Upstream:** `whole` path is checked first; comma split only matters when whole path fails. `"a,,b"` → parts `["a","","b"]`; the empty part matches every entry (source name contains "").
- **Impact:** `"one piece, naruto"` matches titles containing that exact phrase even though it has a comma; `"a,,b"` ignores the empty token.
- **Rust rule:** identical.
- **Test:** T2-SEARCH-08, T2-SEARCH-09.

### E-06 Asymmetric genre and source matching
- **Upstream:** genre must equal the term ignoring case (`genre.equals(it, true)`); source name is a substring match (`sourceName.contains(it, true)`) (`AnimeLibraryItem.kt:34-37`).
- **Impact:** searching "action" does not match a genre "Action Adventure"; searching "Hianime" matches source "HiAnime (EN)".
- **Rust rule:** identical.
- **Test:** T2-SEARCH-04.

### E-07 Hidden categories are dropped when moving entries
- **Upstream:** `openChangeCategoryDialog` computes `common`/`mix` from `getCategories.await(animeId)` (visible-only query, `categories.sq:59-72` `WHERE AC.anime_id = :animeId AND C.hidden = 0`). `setAnimeCategories` then computes `getCategories.await(id)` (visible-only) − remove + add and calls `setAnimeCategories.await(id, newIds)` which replaces **all** links (`AnimeRepositoryImpl.kt:95-102`).
- **Impact (data loss):** an entry in a hidden category loses that membership the first time the user applies any category change to it. Entries in hidden categories also never show in the library (E-08).
- **Rust rule (fix, T-37a-030):** build the next membership from ALL links (`load_all_category_ids_for_entry`), apply delta, replace-all. Never derive the base from the visible subset.
- **Test:** T2-CAT-02 (hidden membership preserved), T2-CAT-03 (mixed state unchanged).

### E-08 Entries only in hidden categories
- **Upstream:** hidden categories are excluded from `displayCategories` (`getAllVisibleAnimeCategories`), so their entries disappear from the library (`AnimeLibraryScreenModel.kt:407-415`). If that leaves no visible entry, `isLibraryEmpty` is true and the library-empty screen is shown (not the pager).
- **Rust rule:** identical. The category count and "Show number of items" count exclude hidden categories.
- **Test:** T2-PIPE-06.

### E-09 Range selection with a stale or filtered-out anchor
- **Upstream:** `items.indexOf(lastSelected)` uses data-class equality of `LibraryAnime`, which includes all counts. After a count change (new chapter fetched, episode marked seen by another path), the stored anchor no longer equals the item, `indexOf` returns −1, and `IntRange(-1, cur)` leads to `items[-1]` → `IndexOutOfBoundsException` (`AnimeLibraryScreenModel.kt:640-656`). The same happens when the anchor was hidden by the search or filter.
- **Rust rule (fix):** compare by id; if either index is missing, add only the clicked row (`Selection::toggle_range`, design R-6).
- **Test:** T2-SEL-04.

### E-10 Selection survives filter, search and page changes
- **Upstream:** selection is a global list; neither search nor page change clears it (`AnimeLibraryScreenModel.kt:610-612`). Bulk actions act on every selected entry even if it is not visible.
- **Impact:** a user can select entries on one page, change the filter, and then "Delete" entries they can no longer see.
- **Rust rule:** keep upstream behavior (T-37a-025) but show "N selected, M hidden by current filter" in the action bar header; confirmation dialogs list the count. Decision recorded.
- **Test:** T2-SEL-07.

### E-11 Entries in several categories
- **Upstream:** `animelibView` emits one row per category link (`LEFT JOIN animes_categories`). Each category page shows the entry; `libraryCount` is distinct; per-tab counts are not distinct; "select all" on two pages selects the same id once because selection is keyed by id.
- **Rust rule:** identical; tab counts may sum above the distinct count (documented).
- **Test:** T2-PIPE-04.

### E-12 Default category not offered in the change-categories dialog
- **Upstream:** `categories = state.categories.filter { it.id != 0L }` (`AnimeLibraryScreenModel.kt:701`). Consequence: a user cannot move an entry *into* Default from the dialog; Default membership only exists implicitly (no `animes_categories` row).
- **Rust rule:** identical (documented). Removing all categories from an entry puts it in Default.
- **Test:** T2-CAT-01.

### E-13 `last_update` NULL
- **Upstream:** `animes.last_update` is nullable; `LibraryAnime.anime.lastUpdate` is `Long` in the mapped domain object (mapper default); sorting by LastUpdate compares the mapped value.
- **Rust rule:** map NULL → 0 in `LibraryRow.last_update`; sort order is then "never updated first".
- **Test:** T2-SORT-08.

### E-14 Tracker mean NaN sorts last
- **Upstream:** `trackMap` includes tracks from all trackers; `trackerScores` maps each entry to `mapNotNull{ logged-in tracker score }.average()`. When an entry has tracks only from logged-out trackers, the list is empty and `average()` returns NaN (Kotlin `Iterable<Double>.average()` of empty is NaN). `?: -1.0` does not apply to NaN. `NaN.compareTo(x)` puts NaN above all numbers (`AnimeLibraryScreenModel.kt:265-312`).
- **Impact:** such entries appear last in ascending order and first in descending, unlike entries with no track at all (which get −1).
- **Rust rule (fix):** `track_scores` holds `None` when there is no logged-in score; `score_or_default` maps `None` to −1.0. (T-37a-058 records the choice.)
- **Test:** T2-SORT-06.

### E-15 Zero counts sort last in both directions
- **Upstream:** for UnseenCount and AiringTime the zero branch returns `if (asc) 1 else -1` before the reversal; reversal flips it back → zero entries last in both directions (`:289-295`, `:313-322`). The source comment "Ensure unseen content comes first" is wrong.
- **Rust rule:** identical (`zero_last`). Document it in the sort help text.
- **Test:** T2-SORT-03, T2-SORT-04.

### E-16 Alphabetical tiebreaker is always ascending
- **Upstream:** `.thenComparator(sortAlphabetically)` is applied after `reversed()` (`:334-336`).
- **Impact:** with Date Added Descending, entries added in the same second are alphabetised A→Z.
- **Rust rule:** identical.
- **Test:** T2-SORT-09.

### E-17 Unchecked indices from UI actions
- **Upstream:** `state.categories[screenModel.activeCategoryIndex]` (refresh button, `AnimeLibraryTab.kt:144`), `categories[index]` in `invertSelection` (`:679`), `categories[activeCategoryIndex]` in `getRandomAnimelibItemForCurrentCategory` (guarded only by `isEmpty`, `:601`). `activeCategoryIndex` is persisted and may exceed the current size after a category is deleted or hidden.
- **Impact:** IndexOutOfBounds crash when the saved index exceeds the number of pages.
- **Rust rule:** clamp the index to `pages.len()-1` on load and on every rebuild; use `get()` everywhere.
- **Test:** T2-PIPE-08.

### E-18 Sorting one category changes all categories (categorized display off)
- **Upstream:** `SetSortModeForAnimeCategory` writes `updateAllAnimeCategoryFlags(flags)` when `categorized_display` is off (`SetSortModeForAnimeCategory.kt:33-36`), so every category row gets the same flags, overwriting their own sort.
- **Impact:** by design of the setting ("one sort for all") but it erases per-category sorts silently when the user turns categorized display off and on.
- **Rust rule:** identical; toggling `categorized_display` off does NOT restore previous per-category flags (documented). Proposal: keep a snapshot of per-category flags when the setting turns off (T-37a-040 acceptance).
- **Test:** T2-SORT-10.

### E-19 Searching forces tabs visible; toolbar scroll uses a different rule
- **Upstream:** `showPageTabs = showCategoryTabs || searchQuery nonempty` (`AnimeLibraryTab.kt:203`); toolbar `tabVisible = showCategoryTabs && categories.size > 1` (`:129`) ignores the search case, so the toolbar scroll overlay can be wrong while searching.
- **Rust rule:** tab visibility uses the same expression for both places (single source of truth).
- **Test:** T2-PIPE-09.

### E-20 Flags `0` decode to Descending
- **Upstream:** direction bit clear → Descending (`AnimeLibrarySortMode.kt:57`). Category rows default to flags 0 (system row seed `categories.sq:10`). Default category therefore sorts Z→A alphabetically until changed.
- **Rust rule:** keep decoding; decide the seed in migration (T-37a-002). Documented in help as "Default category sorts Z→A until changed".
- **Test:** T2-CODEC-03.

### E-21 Random sort seed shared
- **Upstream:** one seed per kind (`library_random_anime_sort_seed`), reseeded only when Random is chosen (`SetSortModeForAnimeCategory.kt:23-25`). Random shuffles identically for categories with equal lists.
- **Rust rule:** identical (per kind). Random order is stable across restarts until re-chosen.
- **Test:** T2-SORT-07.

### E-22 hasActiveFilters ignores the downloaded-only override
- **Upstream:** `hasActiveFilters` uses the raw `filterDownloaded` pref (`AnimeLibraryScreenModel.kt:157-167`), while filtering uses `globalFilterDownloaded` forcing `ENABLED_IS`. With downloaded-only on and no other filters, the filter icon is not tinted, and an empty result shows "Category is empty" instead of "No match found".
- **Rust rule (fix):** include the forced state in `has_active_filters` (T-37a-056). Document.
- **Test:** T2-PIPE-07.

### E-23 List row height integer arithmetic
- **Upstream:** `(containerHeight / entries).toDp() - (3 / entries).dp` where `3 / entries` is Int division: 3 for 1 entry, 1 for 2–3 entries, 0 for ≥4 (`CommonEntryItem.kt:354`).
- **Rust rule:** compute `height = page_height / n − 3.0 / n` in floating point, clamp ≥ 48 px. Intentional difference (smoother rows).
- **Test:** T2-UI-02.

### E-24 Orientation read once
- **Upstream:** `remember { if (landscape) animeLandscapeColumns() else animePortraitColumns() }` in the settings dialog has no key, and the pager `remember(isLandscape)` is keyed (`AnimeLibraryPager.kt:74`). The dialog keeps the orientation from when it opened.
- **Rust rule:** single columns preference on desktop (U-3.3); no orientation concept.
- **Test:** manual.

### E-25 Continue with nothing to continue
- **Upstream (anime):** nothing happens (`AnimeLibraryTab.kt:208-209`). **Upstream (manga):** snackbar "Next chapter not found" (`:216-226`).
- **Rust rule:** both show the toast "Next chapter not found" for anime too (proposal; same key `no_next_chapter` is generic in wording only). Decision recorded in T-37a-033.
- **Test:** T2-CONT-02.

### E-26 Initial page computed once
- **Upstream:** `coercedCurrentPage` is `remember {}` without keys (`AnimeLibraryContent.kt:57`); later category-count changes are handled only by the `LaunchedEffect(categories)` clamp.
- **Rust rule:** keyed on the category id list; clamp on every rebuild.
- **Test:** T2-PIPE-08.

### E-27 Remove-from-library drops cache timestamps
- **Upstream:** `it.removeCovers(coverCache)` returns a copy with `coverLastModified` updated, but the copy is discarded (`AnimeLibraryScreenModel.kt:533-540`); the `AnimeUpdate` only sets `favorite = false`.
- **Impact:** the cover cache file is deleted, but the DB keeps the old `cover_last_modified`. If the entry is re-added, the old timestamp may let a stale cached image be served.
- **Rust rule (fix):** persist `cover_last_modified = now` in the same update (area:34 cache-busting rule).
- **Test:** T2-DEL-03.

### E-28 Local entries are never deleted from disk
- **Upstream:** `removeAnimes` deletes downloads only for `AnimeHttpSource` (`:546-547`). `LocalAnimeSource` is not one. The checkbox for downloaded items is hidden for local selections (`DeleteLibraryEntryDialog.kt:31-35`).
- **Rust rule:** identical. The local folder is user data and is never deleted by this area.
- **Test:** T2-DEL-04.

### E-29 Category screen dialog closes on any emission
- **Upstream:** the categories flow maps every emission to `Success(categories)` with `dialog = null` (`AnimeCategoryScreenModel.kt:49-56`).
- **Impact:** a background category change (e.g., the library update job or a reorder from another window) closes an open rename dialog and discards the typed text.
- **Rust rule (fix):** keep the open dialog across refreshes when its target id still exists.
- **Test:** T2-CATSCR-02.

### E-30 Global update message
- **Upstream:** `onClickGlobalUpdate` → `onClickRefresh(null)` → same message `updating_category` ("Updating category") (`AnimeLibraryTab.kt:105-112, 147`).
- **Rust rule (proposal):** global → "Updating library". (T-37a-050.) Keep the upstream key for the category case.
- **Test:** T2-UPD-01.

### E-31 Category deletion fallback and cleanup
- **Upstream:** deleted category's memberships are removed by `ON DELETE`/`DELETE` cascade (`animes_categories.category_id FK`); entries with no remaining links fall to category 0 (`coalesce`). Preferences referencing the id are cleaned (D-11). `default_anime_category` is reset if it equals the id.
- **Rust rule:** identical; plus re-index orders for all categories (system included) (B-15.5).
- **Test:** T2-CAT-06.

### E-32 A local entry counts as downloaded
- **Upstream:** `filterFnDownloaded` returns true for `isLocal()` (`:200`).
- **Rust rule:** identical; the "Downloaded" filter therefore includes local entries.
- **Test:** T2-PIPE-03.

### E-33 Refresh feedback when the job is already running
- **Upstream:** pull-to-refresh: if `started == false`, nothing is shown (no spinner, no snackbar from the pull path itself); the toolbar path shows `update_already_running`.
- **Rust rule:** every trigger shows a toast (the pull path is removed on desktop, so this is consistent).
- **Test:** T2-UPD-02.

### E-34 Range selection only spans displayed items
- **Upstream:** the range uses `getAnimelibItemsByCategoryId`, i.e., the post-filter, post-search list (`:640-641`).
- **Rust rule:** identical; items hidden by the current filter inside the range are not selected.
- **Test:** T2-SEL-05.

### E-35 Bulk mark runs in the background without feedback
- **Upstream:** `markSeenSelection` launches the writes and clears the selection immediately (`:508-519`). Failures are logged by the interactor, not shown.
- **Rust rule:** clear the selection after the writes complete (or show a spinner in the action bar); on partial failure toast "N of M updated" (proposal).
- **Test:** T2-BULK-02.

### E-36 Download N after skipping already-downloaded
- **Upstream:** the filter (queued or downloaded) runs before `take(amount)` (`:489-498`), so "Next 5" picks the next five **not yet downloaded/queued** episodes, which may be further along the list than the next five unseen ones.
- **Rust rule:** identical (documented behavior); the help tooltip says "next N not downloaded".
- **Test:** T2-BULK-04.

### E-37 Delete dialog combinations
- **Upstream:** checkboxes are independent; confirm requires at least one checked; "downloaded" can be deleted while the entry stays in the library, and vice versa (`DeleteLibraryEntryDialog.kt:46-58`; `AnimeLibraryScreenModel.kt:528-552`).
- **Rust rule:** identical.
- **Test:** T2-DEL-01, T2-DEL-02.

### E-38 Long-press with no selection
- **Upstream:** long-press → `toggleRangeSelection`; with empty selection, `lastSelected` is null → single add (`:637-638`). So a long-press on an unselected item selects it and enters selection mode.
- **Rust rule:** identical.
- **Test:** T2-SEL-02.

### E-39 Mixed category left unchanged on apply
- **Upstream:** a category shown as `TriState.Exclude` (mixed) is in neither the include nor the exclude list, so each entry keeps its own membership in it (`CategoryDialogs.kt:245-249`).
- **Rust rule:** identical.
- **Test:** T2-CAT-03.

### E-40 Category name not trimmed; uniqueness case-sensitive
- **Upstream:** `categories.contains(name)` exact match; name not trimmed (`CategoryDialogs.kt:49`).
- **Impact:** "Action" and "action" both allowed; "Action " (trailing space) allowed.
- **Rust rule:** identical in storage (no trim). Proposal: trim on save and treat case-insensitive duplicates as errors (T-37a-042 acceptance says "decision recorded").
- **Test:** T2-CAT-07.

### E-41 Reorder includes hidden categories when the setting is off
- **Upstream:** the management list shows all non-system categories when `hidden_categories` is off; `ReorderAnimeCategory` reindexes over all non-system (`ReorderAnimeCategory.kt:20-37`), so the hidden ones keep their relative place.
- **Rust rule:** identical.
- **Test:** T2-CAT-05.

### E-42 HideAnimeCategory returns the Rename result type
- **Upstream:** `HideAnimeCategory.await` returns `RenameAnimeCategory.Result.Success` (`HideAnimeCategory.kt:22`).
- **Impact:** none at runtime (the `when` checks only `InternalError`); it is a type slip.
- **Rust rule:** one result type `CategoryResult`.
- **Test:** T2-CAT-08 (compile-time).

### E-43 `hidden_categories` decides management visibility only
- **Upstream:** `AnimeCategoryScreenModel` uses visible categories when the setting is on (`:43-47`); the library page always uses visible categories (B-3).
- **Rust rule:** identical.
- **Test:** T2-CATSCR-03.

### E-44 Searching shows tabs with the tabs option off
- **Upstream:** E-19 above.
- **Test:** T2-PIPE-09.

### E-45 Interval filter only in non-release builds with the restriction on
- **Upstream:** `if ((!isReleaseBuildType) && ENTRY_OUTSIDE_RELEASE_PERIOD in restrictions)` (`AnimeLibrarySettingsDialog.kt:121`). Filtering itself is applied when the restriction is on regardless of build (`filterFnIntervalCustom`, `:222-228`), but the user cannot set it in release builds.
- **Rust rule:** the filter is evaluated (parity) but the row is hidden in the desktop UI (T-37a-057).
- **Test:** T2-PIPE-05.

### E-46 Tracker filter with the tracker logged out
- **Upstream:** `getTrackingFilterFlow` only lists logged-in trackers; the preference values for logged-out trackers are retained and ignored (`trackingFilter.isEmpty()` → ignore, `:231`). When the user logs in again, the old filter reapplies.
- **Rust rule:** identical; the settings UI shows only logged-in trackers.
- **Test:** T2-PIPE-10.

### E-47 Categorized display toggle
- **Upstream:** toggling `categorized_display` changes which write path sort changes take (B-5.7) but does not rewrite stored flags.
- **Rust rule:** identical; see E-18.
- **Test:** T2-SORT-10.

### E-48 Season-parent counts (anime)
- **Upstream:** for `fetch_type = 0`, `totalCount` = number of child seasons; `seenCount` = fully seen seasons; `lastSeen` = max over children; the unseen badge counts unseen seasons (`animelibView.sq:6-23`, `animeseasonstatsView.sq`).
- **Rust rule:** identical; the badge tooltip says "unseen seasons" for these entries (proposal).
- **Test:** T2-PIPE-11.

### E-49 Library non-empty but all entries hidden
- **Upstream:** `isLibraryEmpty` is true (E-08), so the library-empty screen shows unless a search is active. With an active search, the pager shows the "No results found" state with the global-search row, and `categories` may be empty (no visible non-empty category), so `coercedCurrentPage` is computed from `categories.lastIndex = -1` (see E-17, E-26).
- **Rust rule:** if `pages` is empty, show the library-empty state regardless of search (never create a Stack with zero pages).
- **Test:** T2-PIPE-12.

---

## §T-1. Upstream tests to port

Only one in-scope test file exists: `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt` (103 lines). Port every assertion to `library::flags` tests:

| Upstream test | Assertion | Rust test id |
|---|---|---|
| `Check the amount of flags` | `LibraryDisplayMode.values.size == 4`; `MangaLibrarySort.types.size == 10`; `MangaLibrarySort.directions.size == 2`; `AnimeLibrarySort.types.size == 11`; `AnimeLibrarySort.directions.size == 2` | T2-CODEC-01 |
| `Test Flag plus operator (LibrarySort)` | `MangaLibrarySort(LastRead, Asc) + MangaLibrarySort(DateAdded, Asc).flag == 0b01011100`; same for anime (`LastSeen`, `DateAdded`) | T2-CODEC-02 |
| `Test Flag plus operator` | `MangaLibrarySort(DateAdded, Asc).flag == 0b01011100`; anime same | T2-CODEC-02 |
| `Test Flag plus operator with old flag as base` | base `UnreadCount, Desc` = `0b00001100`; merging `DateAdded, Asc` yields `0b01011100`, differing from base | T2-CODEC-04 |
| `Test default flags` | `default.type + default.direction == 0b01000000` for both kinds | T2-CODEC-05 |

The Rust codec must reproduce each value exactly.

---

## §T-2. New unit tests (Rust, pure modules; no GTK)

Naming: `T2-<AREA>-<NN>`. Each test is one `#[test]` (or a table-driven one).

### Codec (`library::flags`)
- **T2-CODEC-01** counts: 4 display modes, 10 manga types, 11 anime types, 2 directions.
- **T2-CODEC-02** merge: `merge_flags(old, DateAdded|Asc, mask)` keeps bits outside the mask; results `0b01011100`.
- **T2-CODEC-03** decode table (D-2.5): `""` → (Alpha, Asc); `"UNSEEN_COUNT,DESCENDING"` → (Unseen, Desc); `"FOO,ASCENDING"` → (Alpha, Asc); no comma → default; flags `0` → (Alpha, Desc); flags `0x40` → (Alpha, Asc); flags `124` → (Random, Asc); anime flags `48` → (AiringTime, Desc); manga flags `48` → (Alphabetical, Descending) (the type bits `110000` match no manga type, and the direction bit is clear).
- **T2-CODEC-04** merge with base: base flags `0b00001100` (UnreadCount, Descending) merged with DateAdded+Ascending `0b01011100` → `0b01011100`, not equal to the base (§T-1 row 4).
- **T2-CODEC-05** default flags `0b01000000`.
- **T2-CODEC-06** round trip of every (type, direction) for anime and manga: `decode(encode(x)) == x` (Random included).
- **T2-CODEC-07** token round trip: `deserialize(serialize(x)) == x` for every token in D-2.3/D-2.4; `"RANDOM,ASCENDING"` → Random Asc.
- **T2-CODEC-08** display mode tokens: `COMPACT_GRID`, `COMFORTABLE_GRID`, `COVER_ONLY_GRID`, `LIST`; unknown → CompactGrid.

### Filter (`library::filter`)
- **T2-FILTER-01** TriState cycle: Disabled→Is→Not→Disabled.
- **T2-FILTER-02** apply truth table: Disabled → true for any predicate; Is → predicate; Not → !predicate.
- **T2-FILTER-03** downloaded: local entry passes `Is`; entry with badge count 0 but cache count 2 passes `Is` (cache consulted even when badge off).
- **T2-FILTER-04** unseen uses raw `unseen() > 0` regardless of unread-badge setting (E-02 guard).
- **T2-FILTER-05** completed uses `status == 2`.
- **T2-FILTER-06** interval: restriction off → filter ignored even when `Is` set; restriction on + `Is` → only `fetch_interval < 0`.
- **T2-FILTER-07** AND semantics across two active filters.

### Tracker filter
- **T2-TRACK-01** no logged-in trackers (empty filter map) → everything passes.
- **T2-TRACK-02** `Is` for tracker 1 with entry tracked on 1 → passes; tracked on 2 → fails.
- **T2-TRACK-03** `Not` for tracker 1 with entry tracked on 1 → fails.
- **T2-TRACK-04** include and exclude together: `Is` tracker 1, `Not` tracker 2; entry on 1 and 2 → fails (excluded wins).
- **T2-TRACK-05** entry with no tracks and `Is` filter → fails; with only `Not` filter → passes.

### Sort (`library::sort`)
- **T2-SORT-01** Alphabetical: `"apple" < "Banana"` (lowercase collation); descending reverses; ties fall back to ascending alphabetical.
- **T2-SORT-02** LastSeen / LastUpdate / TotalEpisodes / LatestEpisode / FetchDate / DateAdded use plain numeric compare; descending reverses.
- **T2-SORT-03** UnseenCount: values `[0, 3, 1, 0, 2]` ascending → `[1, 2, 3, 0, 0]` with zeros last; descending → `[3, 2, 1, 0, 0]` (zeros still last).
- **T2-SORT-04** AiringTime: primary unseen (plain), secondary `nextEpisodeAiringAt` zero-last in both directions.
- **T2-SORT-05** Random: same seed → same permutation; different seed → different (for n ≥ 10); no tiebreak applied.
- **T2-SORT-06** TrackerMean: scores `{1: Some(7.5), 2: None, 3: Some(9.0)}` ascending → `[2, 1, 3]` (None = −1.0); entry with tracks from logged-out trackers only → `None` (not NaN) (E-14).
- **T2-SORT-07** Random seed persisted per kind; anime seed unaffected by manga reseed.
- **T2-SORT-08** `last_update` NULL → 0 sorts first ascending.
- **T2-SORT-09** tiebreak: equal primary keys in descending order still sort titles ascending (E-16).
- **T2-SORT-10** categorized display off: `set_sort` writes the same flags to every category; categorized on: only the target category changes (E-18/E-47).

### Search (`library::search`)
- **T2-SEARCH-01** `"id:42"` matches id 42 only; `"ID:42"` also matches.
- **T2-SEARCH-02** `"id:"` and `"id:abc"` match nothing; `"id: 42"` matches nothing (E-01).
- **T2-SEARCH-03** (alias of 02 for the parse table) — `"id:"` matches nothing.
- **T2-SEARCH-04** genre exact: genres `["Action Adventure"]`, term `action` → no match; term `Action Adventure` → match; source name `"HiAnime (EN)"` contains `hianime` → match (E-06).
- **T2-SEARCH-05** negation: `"-romance"` excludes entries whose genre equals `romance` (ignoring case), keeps others.
- **T2-SEARCH-06** `Some("")` matches all rows (E-03).
- **T2-SEARCH-07** `"-"` alone matches only rows whose title/author/artist/description contains `-` (E-04).
- **T2-SEARCH-08** whole-string match with a comma: title `"Re:Zero, Starting Life"` and query `"zero, starting"` → match via whole path.
- **T2-SEARCH-09** `"a,,b"`: empty token matches every row; result equals `"a,b"` result (E-05).

### Pipeline (`library::pipeline`)
- **T2-PIPE-01** badges: download badge off → `download_count == 0` but filter still uses cache; unread badge off → `unseen_count == 0`, filter uses raw.
- **T2-PIPE-02** grouping keeps DB order within a category.
- **T2-PIPE-03** local entry passes Downloaded filter (E-32).
- **T2-PIPE-04** entry in two categories appears on both pages; `library_count` counts it once (E-11).
- **T2-PIPE-05** interval gating (see FILTER-06) through the full pipeline.
- **T2-PIPE-06** entry only in a hidden category: not present in pages; `library_count == 0` (E-08).
- **T2-PIPE-07** downloaded-only on + no other filter → `has_active_filters == true` (E-22 fix).
- **T2-PIPE-08** saved page index greater than page count clamps to last page (E-17, E-26).
- **T2-PIPE-09** tabs visibility rule: `show_tabs = category_tabs || search_active`; the toolbar uses the same expression (E-19, E-44).
- **T2-PIPE-10** tracker filter for a logged-out tracker is ignored; re-login restores (E-46).
- **T2-PIPE-11** season-parent row: `unseen()` = unseen seasons; `has_started` = fully seen seasons > 0 (E-48).
- **T2-PIPE-12** zero visible categories with rows → `pages` empty → library-empty state, no Stack built (E-49).
- **T2-PIPE-13** system category is dropped from display when rows exist and none is uncategorized (B-3 step 4).

### Selection (`library::select`)
- **T2-SEL-01** toggle add then remove returns to empty.
- **T2-SEL-02** range with empty selection adds only the clicked row (E-38).
- **T2-SEL-03** range within a category: click row 1, then row 4 → selection is {1,2,3,4}; then click row 0 → the anchor is the last inserted row (4), so the range 0..=4 is applied and the result is {0,1,2,3,4}.
- **T2-SEL-04** stale anchor (count changed so id absent from `page_items` positions) → only clicked row added, no panic (E-09).
- **T2-SEL-05** range skips items hidden by the filter (they are not in `page_items`) (E-34).
- **T2-SEL-06** select_all adds only unselected page rows; invert flips the page subset only.
- **T2-SEL-07** selection survives a page change and search change (E-10): build pipeline with new query; selection unchanged; `hidden_count` reports 1 when a selected row is filtered out.
- **T2-SEL-08** categories of different kind do not mix (anime selection cannot contain manga rows).
- **T2-SEL-09** continue visibility independent of badge (E-02) — UI-level, asserted on the view-model flag `show_continue = continue_enabled && row.unseen() > 0`.

### Category membership (`library::actions` + `db`)
- **T2-CAT-01** Default category id 0 not offered in dialog options (E-12).
- **T2-CAT-02** delta with a hidden membership: entry in {1 (visible), 9 (hidden)}; apply add 2, remove 1 → result {2, 9} (E-07 fix).
- **T2-CAT-03** mixed categories: entry A in {1}, entry B in {2}; category 3 mixed → unchanged for both (E-39).
- **T2-CAT-04** common/mix computation: A∈{1,2}, B∈{2}: common={2}, mix={1}.
- **T2-CAT-05** reorder over non-system categories only; system keeps order; hidden categories keep relative positions (E-41).
- **T2-CAT-06** delete: memberships removed; default pref cleared if equal; id removed from 5 StringSets; orders re-indexed (E-31).
- **T2-CAT-07** create: name not trimmed; duplicate check is exact; flags = global sort encoding; order = max+1 (E-40).
- **T2-CAT-08** all three result paths return one `CategoryResult` type (E-42) — compile check.

### Bulk and delete (`library::actions`)
- **T2-BULK-01** download next N: skips queued and downloaded before `take(N)` (E-36).
- **T2-BULK-02** mark selection: clears selection after writes; partial failure reports count (E-35).
- **T2-BULK-03** download action hidden when any selected row is local.
- **T2-BULK-04** "Unviewed" downloads all unseen not downloaded/queued (amount None).
- **T2-DEL-01** delete from library only: favorite false; downloads untouched.
- **T2-DEL-02** delete downloads only: favorite stays true; downloads deleted via manager for HTTP sources.
- **T2-DEL-03** removal writes `cover_last_modified = now` (E-27 fix).
- **T2-DEL-04** local entries: downloads never deleted; downloaded checkbox not offered (E-28).

### Continue (`library::actions`)
- **T2-CONT-01** anime: next unseen by sort order (`findLast` when descending) after entry-level episode filters.
- **T2-CONT-02** none found → toast "Next chapter not found" (E-25).
- **T2-CONT-03** manga: scanlator-filtered list is used (excluded scanlators never chosen).

### Updates (`library::actions`)
- **T2-UPD-01** global update message is "Updating library" (E-30 proposal); category message "Updating category".
- **T2-UPD-02** already running → "An update is already running" (E-33).

### Category screen model (`category::ops` + view)
- **T2-CATSCR-01** create dialog: Add disabled for empty and for existing names; error text switches.
- **T2-CATSCR-02** dialog survives a categories refresh (E-29 fix).
- **T2-CATSCR-03** hidden setting decides management visibility (E-43).

### UI (manual + snapshot where practical)
- **T2-UI-01** grid columns: `columns == 0` → computed from width with 128 px minimum; `columns == n` → n.
- **T2-UI-02** list height formula (E-23).

---

## §T-3. Fixtures

Directory `tests/fixtures/library/` (JSON, loaded by `cargo run -- --library-demo` and by unit tests through `include_str!`).

Files:
1. `anime_rows.json` — 60 rows: 20 entries in category 1, 15 in category 2, 10 in category 3 (hidden), 5 in category 0 (system), 10 entries in two categories (duplicated rows). Include: 3 with `fetch_type = 0` (season parent); 4 local (`source = 0`); 2 with `last_update = null`; 3 with genres containing "Action Adventure"; 2 with titles containing a comma; 5 with `next_airing_at = 0`; 6 with tracker tracks (ids 1 and 2).
2. `manga_rows.json` — 50 rows: scanlator-excluded chapters reflected in counts; 2 entries with `total = 0`; 1 completed (`status = 2`).
3. `categories.json` — `[ {id:0,name:"",order:-1,flags:0,hidden:false}, {id:1,name:"Watching",order:0,flags:124,hidden:false}, {id:2,name:"Later",order:1,flags:12,hidden:false}, {id:3,name:"Secret",order:2,flags:0,hidden:true} ]`.
4. `trackers.json` — `{ "1": {"logged_in": true, "name": "AniList"}, "2": {"logged_in": false, "name": "MyAnimeList"} }`; `tracks.json` mapping entry ids to `[{tracker_id, score}]`.
5. `prefs_default.json` — default values from p1-data-and-config.md D-3 (every key).
6. `sort_vectors.json` — the codec vectors from D-2.5 and T2-CODEC-03.
7. `selection_scenarios.json` — ordered action scripts (toggle, range, select_all, invert, filter change) with expected selections.
8. `category_delta.json` — entry → membership cases for T2-CAT-02..04.

Each fixture carries an `expected` block for the derived counts (`library_count`, per-page sizes) so tests assert against numbers, not against a re-implementation.

---

## §T-4. Integration and UI tests

- **T4-DB-01** in-memory SQLite with the real `animelibView`/`libraryView` SQL and the area:03/04 migrations; insert fixtures; `load_rows` returns the expected counts (season parent counts, manga exclusion).
- **T4-DB-02** `set_categories_delta` on a DB with a hidden link keeps it (E-07 at DB level).
- **T4-DB-03** deleting a category cascades links and re-indexes orders.
- **T4-UI-01** (GTK, headless with `GDK_BACKEND=broadway` or `xvfb-run`): open library with fixtures, switch categories with keyboard, assert page count, selection bar visibility, toast text.
- **T4-UI-02** change category dialog: TriState cycling shows Include/Exclude/None; OK sends the expected include/exclude lists.
- **T4-UI-03** settings dialog: each filter row cycles; downloaded row disabled when downloaded-only is on.

---

## §T-5. Race conditions and concurrency

| ID | Race | Upstream handling | Rust rule |
|---|---|---|---|
| R-01 | Search typed quickly | 250 ms debounce + `collectLatest` cancels older computations | Debounce timer + generation counter; discard stale replies (design R-10). Test T4-UI-01. |
| R-02 | Library data changes during a bulk action | Pipeline recomputes; selection snapshot is stale | Selection snapshot refreshed from the new pipeline output; actions use ids. |
| R-03 | Two category reorders at once | `Mutex` in `ReorderAnimeCategory` | `tokio::sync::Mutex` in `category::ops`. |
| R-04 | Category deleted while its page is open | Stack/pager clamps index on next emission | Clamp on rebuild (E-17). |
| R-05 | Update job started while another runs | `isRunning` check → "already running" | Same check in the job trigger (area:26). |
| R-06 | Bulk action on window close | `launchNonCancellable` | Worker drains queued writes before exit (5 s cap). |
| R-07 | Dialog open while categories change | Dialog cleared (E-29) | Keep dialog if target id still exists (fix). |
| R-08 | Preference change while the pipeline is running | `combine` restarts the pipeline | Pref change event bumps generation; new request supersedes. |
| R-09 | Tracker login/logout while filter dialog is open | `trackersFlow` (stateIn 5 s) | Rows rebuilt on `TrackerChanged`. |
| R-10 | Download manager cache change during filter | `downloadCache.changes` triggers recompute | Same: `DownloadCacheChanged` event triggers recompute. |
| R-11 | Continue pressed twice quickly | Two `launchIO` coroutines open the reader twice | Debounce continue per entry (300 ms) in the UI. |
| R-12 | SQLite write vs read on the same connection | Room/SQLDelight transactions | Single worker connection serialises reads and writes. |
