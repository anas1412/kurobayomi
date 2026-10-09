# Area 42 part 1 — edge cases, failure modes, races, tests

No `*Test.kt` file exists in the 41 part-1 files (checked: none of the paths matches `*Test.kt`). Upstream test coverage for the features comes from the domain and data layers (area:06-data-repos-history-track-source-misc, area:08-domain-library-category-history-updates) and is referenced where relevant. New tests are specified in section 5. Fixtures in section 4 are taken from the Compose preview providers in scope, which are the only sample data the UI code carries.

ID scheme: `EC-42-H-nn` history, `EC-42-U-nn` updates, `EC-42-S-nn` stats, `EC-42-T-nn` storage. Each entry gives the condition, what Aniyomi does (OBSERVED, with `path:line`), and the required Kurobayomi behavior (PROPOSAL or "keep").

---

## 1. History (EC-42-H)

| ID | Condition | Aniyomi behavior (OBSERVED) | Kurobayomi requirement |
|---|---|---|---|
| EC-42-H-01 | A manga has several read chapters | Only the chapter with the max `last_read` is listed (`historyView.sq` `maxReadAtChapterId = chapterId`). | Keep. Test: manga with chapters 1,2,3 read at t1<t2<t3 → exactly one row, chapter 3. |
| EC-42-H-02 | Search text contains `%` or `_` | `LIKE ('%' || :query || '%')` treats them as wildcards; `"_"` matches any single character, `"%"` matches everything. | Use `instr(lower(title), lower(?))` (rust design 4.1). Test: query `"_"` on title "abc" → no match. |
| EC-42-H-03 | Search text with non-ASCII letters (e.g. "Ō") | SQLite `lower()` and `LIKE` fold ASCII only; "ō" does not match "Ō". | Keep ASCII folding by default OR register a Unicode-aware `lower` (rust design 4.1). Test both behaviors and document the choice. |
| EC-42-H-04 | Empty search query `""` | `_query` emits `""`; `LIKE '%%'` matches all; `searchQuery = ""` → empty state text uses `information_no_recent_manga` (not "No results"). | Keep (`isNullOrEmpty` rule, `MangaHistoryScreen.kt:43`). |
| EC-42-H-05 | Next-item when the history's chapter is not in the sorted list (scanlator filter or deleted chapter) | `currIdx = -1` → `subList(0, size)`; `fromChapter = null` → `drop(1)`: returns the SECOND chapter (`GetNextChapters.kt:33-56`, same in `GetNextEpisodes.kt`). | Fix: return the first unit of the list (or the first unread unit when onlyUnread). Test: list [A,B,C], from = Z → result starts with A. Record the deviation. |
| EC-42-H-06 | Resume on the last unit (already read) | `drop(1)` leaves empty list → "Next chapter not found" snackbar. | Keep; same snackbar text. |
| EC-42-H-07 | Resume on an unread unit | Returns list starting with the current unit; the open event uses `firstOrNull()` → current unit opened. | Keep. |
| EC-42-H-08 | Manga reselect of History tab | `HistoriesTab.onReselect` sends `resumeLastEpisodeSeenEvent` (anime) even when the manga sub-tab is visible; manga event never sent (`MangaHistoryTab.kt:168-172` collects a channel nothing sends to). | Fix: reselect acts on the visible sub-tab; manga resume is live. Test: reselect with manga page visible → manga next chapter is opened. |
| EC-42-H-09 | DB error while removing/clearing | Errors swallowed (`MangaHistoryRepositoryImpl.kt:16-60`); `removeAllHistory` returns without event when the delete fails. | Rust: surface a toast (rust design 5). Test: force `SQLITE_BUSY` → toast shown, list unchanged. |
| EC-42-H-10 | Clear all history | `DELETE FROM history` — erases `time_read` (hard delete). Snackbar "History deleted". | Keep (user intent: "All history will be lost"). Stats manga read duration drops to 0 (EC-42-S-01 interplay). |
| EC-42-H-11 | Remove one entry from history | `last_read = 0`; row kept; `time_read` kept; the entry disappears from the list (`readAt > 0` filter). | Keep. Test: remove → row not listed; total read duration unchanged. |
| EC-42-H-12 | Remove all of an entry with checkbox | All history rows of that manga reset (`resetHistoryByMangaId`); anime equivalent. | Keep. |
| EC-42-H-13 | Remove checkbox unchecked by default | `removeEverything = false` (`HistoryDialogs.kt:29`). | Keep. |
| EC-42-H-14 | Favorite from history, default category = −1 (unset) and ≥1 user category | Change-category dialog; the entry is NOT favorited until confirm (`addFavorite` `else` branch). Tracking bind runs before confirm (`:225`). | Keep the order exactly; document that bind-trackers runs before confirm. |
| EC-42-H-15 | Favorite from history, default category = 0 (Default) or no user categories | Favorited with no category (`SetMangaCategories(id, [])`). | Keep. |
| EC-42-H-16 | Change-category confirm on an entry that is already favorite | `moveMangaToCategoriesAndAddToLibrary` sets categories; `if (manga.favorite) return` (no favorite write). Tracking NOT bound. | Keep; flag tracking omission (EC-42-H-17b below). |
| EC-42-H-17 | Migrate action from anime duplicate dialog | `onPopScreen = navigator.replace(AnimeScreen(dialog.oldAnime.id))` — stays on OLD anime; manga version replaces with NEW manga (`MangaHistoryTab.kt:144`). | Port as observed; add a product question (asymmetry). Test documents both. |
| EC-42-H-17b | Change-category confirm does not bind enhanced trackers, but addFavorite does | `moveMangaToCategoriesAndAddToLibrary` (`MangaHistoryScreenModel.kt:169-177`) lacks `bindEnhancedTrackers`. | Proposal: call the same post-add hook on both paths (Tracker sync parity matters: area:27-tracking-core). |
| EC-42-H-18 | Edit categories from change dialog (anime) | Only `navigator.push(CategoriesTab)`; manga also calls `showMangaCategory()`. | Port with fix: open the anime categories page. |
| EC-42-H-19 | History row of a non-library entry (after un-favoriting) | Still listed (historyView does not filter `favorite`); favorite button shown. | Keep. |
| EC-42-H-20 | Subtitle when chapter number is −1 (unknown) or < −1 | `chapterNumber > -1` false → only time shown. -2.0 test fixture shows time only. | Keep. |
| EC-42-H-21 | Time-only subtitle across days | Date header shows the day; subtitle shows only the time (locale SHORT). Two rows from different days in the same list: each has its own header. | Keep. |
| EC-42-H-22 | Midnight boundary while the screen is open | Headers computed in composition with `LocalDate.now()` at compose time; not reactive. | Proposal: recompute headers when the date changes (timer at local midnight). Low priority. |
| EC-42-H-23 | Query subscription error | `catch` sends `InternalError`; the subscription ends; list kept. | Keep plus Rust toast; re-subscribe on next query change. |
| EC-42-H-24 | `distinctUntilChanged` on the list | Re-emissions with identical list do not recompose. | Keep (diff by `(history_id, at)`). |
| EC-42-H-25 | Two sub-tabs search independently | `TabbedScreen` picks `mangaSearchQuery` for page 1 and `animeSearchQuery` for page 0 (`TabbedScreen.kt:57-63`). | Keep per-sub-tab search state. |
| EC-42-H-26 | Empty history with a `query` set but `list` loaded empty | "No results found" (`MR.no_results_found`). | Keep. |
| EC-42-H-27 | `state.searchQuery` / `updateSearchQuery` | Never read by the UI (dead path, `MangaHistoryScreenModel.kt:141-143`). | Drop (inventory). |

---

## 2. Updates (EC-42-U)

| ID | Condition | Aniyomi behavior (OBSERVED) | Kurobayomi requirement |
|---|---|---|---|
| EC-42-U-01 | Window uses upload date; grouping uses fetch date | Filter `dateUpload > now-3mo`; group headers by `dateFetch` (`updatesView.sq`; `MangaUpdatesScreenModel.kt:376`). | Keep both. Test: chapter uploaded 4 months ago but fetched yesterday → excluded (upload filter). |
| EC-42-U-02 | More than 500 updates in 3 months | Silent truncation at 500 (`GetMangaUpdates.kt`). No pagination or "more" indicator. | Keep the limit (parity); proposal: show footer "Showing latest 500" when count == 500. |
| EC-42-U-03 | Query error | `catch` sends `InternalError`; `isLoading` stays true → loading spinner forever (`MangaUpdatesScreenModel.kt:82-86`). | Fix: `isLoading = false` + error row with retry. Test: inject error → not stuck loading. |
| EC-42-U-04 | Selection id set never pruned | `selectedChapterIds` keeps ids of rows that left the list (no removal on refresh). Harmless (ids unique) but unbounded growth. | Rust: prune on each emission to ids present in the list. |
| EC-42-U-05 | Selection bounds after the list shrinks | `selectedPositions` indices are not revalidated when `items` is replaced; a later long press may compute a range using stale indices (`MangaUpdatesScreenModel.kt:292-299`). | Rust: recompute bounds from `selected` flags on every list replacement. Test: select 3..5, shrink list to 4 items, long press index 0 → selects 0 only (no crash, no wrong range). |
| EC-42-U-06 | Range select across date headers | Headers are not in `items`; ranges operate on data indices, so headers are skipped correctly. | Keep (same index space). |
| EC-42-U-07 | Cancel download while queued | `cancelDownload` mutates the shared `MangaDownload` status to `NOT_DOWNLOADED` (`MangaUpdatesScreenModel.kt:190`); queue state may still hold it briefly. | Rust: cancel through the queue API and derive status from the queue snapshot; no in-place mutation. |
| EC-42-U-08 | Per-row DELETE from the download indicator | Deletes downloaded chapters/episodes without confirmation (`downloadEpisodes` → `DELETE` branch, `AnimeUpdatesScreenModel.kt:179-181`). Bulk delete does confirm (`showConfirmDeleteEpisodes`). | Parity by default; flag as a product question (data-loss risk). Proposal: confirm for every delete. |
| EC-42-U-09 | Library refresh while already running | `startNow` returns `started = false`; `LibraryUpdateTriggered(false)` emitted; no spinner (`MangaUpdatesScreen.kt:76-77`). | Keep; show toast "library update already running" (part-2 string). |
| EC-42-U-10 | Fake spinner | 1 s spinner regardless of job (`MangaUpdatesScreen.kt:81-82`). | Rust: real job-state spinner (rust design 9). |
| EC-42-U-11 | Mark-read from bulk bar clears selection before DB finishes | `markUpdatesRead` launches IO then `toggleAllSelection(false)` immediately (`MangaUpdatesScreenModel.kt:198-207`). A failure is silent. | Rust: clear selection on success; keep selection and show toast on failure. |
| EC-42-U-12 | Bookmark bulk filter uses the snapshot | `filterNot { it.update.bookmark == bookmark }` uses the list snapshot, not DB state; a concurrent change elsewhere is overwritten. | Keep (last-writer-wins, same as Aniyomi); document. |
| EC-42-U-13 | Mark-unread visibility | Shown when `read || lastPageRead > 0` (manga); anime `seen || lastSecondSeen > 0` (`AnimeUpdatesScreen.kt:145`). A partially-read unseen episode can be "unviewed" (no-op on seen=false). | Keep. |
| EC-42-U-14 | Anime progress at exactly one hour | `formatProgress(3_600_000)` → `"60:00"` (strict `>`), `3_600_001` → `"1:00:00"`. | Keep (test both). |
| EC-42-U-15 | Manga progress display | `lastPageRead + 1` ("Page: 5" when stored 4). Shown only when unread and `lastPageRead > 0`. | Keep. |
| EC-42-U-16 | Anime `total_seconds` naming | Column name says seconds; values are milliseconds (formatProgress and stats both treat as ms). | Rust: name `total_ms` / `last_ms`; document the migration of area:04-db-anime-schema-migrations. |
| EC-42-U-17 | Date header uses local zone at compose time | `toLocalDate()` system zone; a chapter fetched 23:59 and shown after midnight moves groups on next compose. | Keep; compute groups per emission. |
| EC-42-U-18 | Download state for names that changed | `isChapterDownloaded(chapterName, scanlator, mangaTitle, sourceId)` — renaming a source title makes an existing download look not downloaded. | Keep for parity (area:23-downloads-manga owns the naming); flag as risk. |
| EC-42-U-19 | `useExternalDownloader` read once | Captured at model creation (`AnimeUpdatesScreenModel.kt:70`). | Rust: read on each action. |
| EC-42-U-20 | Anime bottom-bar external/internal player | Both branches call `onOpenEpisode(selected[0], true)`; the choice between external and internal is made by the predicate, not by the callback argument (`AnimeUpdatesScreen.kt:154-159`). | Keep the predicate, document that the `altPlayer` argument is `true` in both (part-2 decides the player). |
| EC-42-U-21 | Long press on the row while already in selection mode | `onUpdateSelected(item, !selected, true, true)`: toggling off if already selected; range fill applies only when selecting. | Keep. |
| EC-42-U-22 | `toggleSelection` on a row not in the list | `indexOfFirst` = −1 → no-op. | Keep. |
| EC-42-U-23 | Back gesture while selection active | `BackHandler(enabled = selectionMode) { onSelectAll(false) }` (`MangaUpdatesScreen.kt:49`). | Keep as Escape; ensure Escape does not also go back (rust design 9). |
| EC-42-U-24 | Two entries with the same title in updates | Keys use ids (`mangaUpdates-${mangaId}-${chapterId}`), no collision. | Keep: key by `(entry_id, unit_id)`. |

---

## 3. Stats (EC-42-S)

| ID | Condition | Aniyomi behavior (OBSERVED) | Kurobayomi requirement |
|---|---|---|---|
| EC-42-S-01 | Manga "Read duration" | `SUM(history.time_read)` over ALL history rows (including reset rows and entries not in library), `MangaStatsScreenModel.kt:58`. | Keep (parity). Test: reset row keeps its contribution. Document as a known over-count. |
| EC-42-S-02 | Parity fixture for stats | Expected outputs must match Aniyomi on the same DB. | Test vector: 3 manga (completed, started, untouched), 2 trackers (one scored), expected numbers computed by hand (section 4.3). |
| EC-42-S-03 | Anime watch time population | Sum over LIBRARY anime (`GetLibraryAnime`), not favorites. | Rust SQL must use the area:37-ui-library library predicate. Test: a non-library anime with seen episodes is NOT counted. |
| EC-42-S-04 | "Completed" count | `status == COMPLETED && unreadCount == 0`; an entry with 0 chapters and COMPLETED status counts as completed (`totalChapters = 0` → unread 0). | Keep. Test: empty completed entry counted. |
| EC-42-S-05 | "Tracked entries" | Counts entries with ≥1 logged-in tracker link, scored or not (`mangaTrackMap.count { isNotEmpty() }`). | Keep. |
| EC-42-S-06 | Exception during computation | No try/catch; state stays Loading forever (`MangaStatsScreenModel.kt:43-88`); `get10PointScore` uses `!!`. | Fix: per-section error → "N/A", log. Test: tracker id unknown → other sections still shown. |
| EC-42-S-07 | Global update count and release-period restriction | `ENTRY_OUTSIDE_RELEASE_PERIOD` is not evaluated by stats (`MangaStatsScreenModel.kt:111-115`) but IS a restriction in the updater (area:26-library-update-jobs). | Keep (parity) or reuse the updater predicate (flag). |
| EC-42-S-08 | Mean score weighting | Mean of per-entry means: each entry weighs 1 regardless of number of trackers. | Keep. Test: entry A tracks 8,10 (mean 9), entry B tracks 6 → overall 7.5. |
| EC-42-S-09 | Scores of 0 or negative | Excluded by `score > 0.0` (`MangaStatsScreenModel.kt:131`). | Keep. Test: score 0.0 excluded from mean but entry still counted in tracked titles. |
| EC-42-S-10 | No scored tracks | `average()` of empty → NaN → "N/A" (`TrackerStats`). | Keep: `None` → "N/A". |
| EC-42-S-11 | Duration display rules | `1d 1h` for 25 h (minutes dropped when days and hours shown); `1h` for 1 h 0 m 5 s (seconds suppressed when hours > 0). | Keep the exact `duration_string` rules (behavior 5.4). Test vectors in section 4.3. |
| EC-42-S-12 | Download count includes orphans | `getDownloadCount()` counts every chapter directory in the cache, including directories of entries not in the library. | Keep; flag (Stats download count can exceed the library's downloaded count). |
| EC-42-S-13 | Stats freshness | Snapshot at open; no subscription. | Keep; P2 refresh button. |
| EC-42-S-14 | Anime stats vs manga stats on mixed data | Each page only reads its kind. | Keep. |

---

## 4. Storage (EC-42-T)

| ID | Condition | Aniyomi behavior (OBSERVED) | Kurobayomi requirement |
|---|---|---|---|
| EC-42-T-01 | Storage page never leaves loading when the download cache already initialized | `combine` requires a value from `downloadCache.changes` (single-consumer `Channel`), `AnimeDownloadCache.kt:75-76`, `:417`. A second collector can miss the element (behavior 6.8). | Fix with broadcast + watch (rust design 2). Test: two subscribers after one change; both receive it; late subscriber sees `initializing == false` and still gets an initial snapshot. |
| EC-42-T-02 | Hidden categories setting toggled while storage is open | Read once at start (`CommonStorageScreenModel.kt:40`). | Rust: observe the pref (watch) — proposal; parity alternative documented. |
| EC-42-T-03 | Storage with no entries (or category with no entries) | Only the dropdown and "0 B" chart; no empty message. | Keep by default; decision: add helper text (rust/UI parity 5.6). |
| EC-42-T-04 | "Default" category (id 0) | Category list shows the system category as "Default" (mapping in `SelectStorageCategory.kt:22-28`) even though `GetCategories` filters system categories elsewhere. | Keep the mapping; confirm the list source in area:08-domain-library-category-history-updates (whether id 0 is included). |
| EC-42-T-05 | Entries with equal sizes | `sortedByDescending` is stable; original library order kept. | Keep. Test: ties keep insertion order. |
| EC-42-T-06 | Colour of an entry | Deterministic per id (`Random(id)`). | Keep determinism; exact Kotlin RNG sequence not required (rust design 9). |
| EC-42-T-07 | Entry deleted while selected category shows it | The row disappears when the download cache emits a change; the confirm dialog closes immediately (`StorageItem.kt:133-135`). | Keep; show toast "Deleted" only if the user asks (not in Aniyomi). |
| EC-42-T-08 | Entries with 0 bytes | Zero-length arc (`CumulativeStorage.kt:41-47`); may render a round-cap dot. | Keep geometry; skip zero arcs in Rust (no dot) — flag as deliberate deviation. |
| EC-42-T-09 | "All" with hidden categories hidden | Entries whose category is not in the visible list are dropped (`CommonStorageScreenModel.kt:65-68`). Entries in category 0 are kept only if category 0 is present in the list. | Keep exact rule. Test: entry in a hidden category, setting on → not listed in All. |
| EC-42-T-10 | Size formatting | SI units: `1000` steps, `"%.2f GB"`, `"%.2f MB"`, `"%.2f KB"` and `"N B"`; `Float` math (`StorageUtil.kt:8-19`). `1_000_000_000` → `"1.00 GB"`; `999_999` → `"1000.00 KB"`? (`999_999 / 1000 = 999.999` → `"1000.00 KB"`, a known rounding artefact). | Keep exact formula (f32); test the artefact explicitly. |
| EC-42-T-11 | Total is 0 | No arcs; text "0 B". | Keep. |
| EC-42-T-12 | Recompute cost | Every combined emission rebuilds the list and recomputes all sizes; `(items + item).sortedByDescending` per row (O(n² log n)). | Rust: build once per emission, sort once. Test: 2,000 entries under 50 ms (perf test, section 5). |
| EC-42-T-13 | Size lookups on the UI path | `getDownloadSize` walks directories inside `combine` (`CommonStorageScreenModel.kt:79-81`). | Rust: compute on background task; cache by download-cache generation. |
| EC-42-T-14 | Delete of a local-source entry | `deleteAnime`/`deleteManga` (area:23-downloads-manga, area:24-downloads-anime) for `LocalMangaSource` removes the files in the local folder (irreversible). | Keep parity; the confirm dialog text is the only guard — flag as data-loss risk (UI copy: "Delete downloaded chapters?" is misleading for local entries). |
| EC-42-T-15 | Delete for an entry whose source is uninstalled | `sourceManager.get(source) ?: return` → nothing deleted, no message (`AnimeStorageScreenModel.kt:46`). | Proposal: toast "Source not installed; files not deleted". |

---

## 5. Tests to write (new)

All tests live in `crates/kuro-domain/tests/` (pure) and `crates/kuro-data/tests/` (SQLite on an in-memory DB with the schema of area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations applied). Names are the test function names.

### 5.1 kuro-domain

1. `history::group_by_local_date_inserts_leading_header` — rows [today, today, yesterday] → [H(today), I, I, H(yesterday), I].
2. `history::group_by_local_date_no_header_for_null_date` — `at = None` rows never create headers (EC-42-H-20 family).
3. `history::next_units_unread_current_is_first` — EC-42-H-07.
4. `history::next_units_read_current_drops_it` — EC-42-H-06 style.
5. `history::next_units_missing_from_id_returns_first_unit` — EC-42-H-05 fixed behavior; plus `legacy_mode` test asserting Aniyomi's second-unit result when the compat flag is on.
6. `updates::window_is_three_calendar_months` — `checked_sub_months(3)`; leap-year: 2024-05-31 → 2024-02-29.
7. `updates::selection_long_press_fills_gap_outward` — select 2 (long), then 6 (long) → 2..=6 selected, bounds [2,6].
8. `updates::selection_long_press_upward` — select 6 then 2 → 2..=6.
9. `updates::selection_tap_deselect_bound_recomputes` — bounds update to next selected.
10. `updates::selection_stale_bounds_after_shrink` — EC-42-U-05.
11. `updates::toggle_all_resets_bounds` and `invert_resets_bounds`.
12. `format::progress_mmss_boundaries` — 0 → "0:00"; 59_999 → "0:59"; 3_600_000 → "60:00"; 3_600_001 → "1:00:00"; 3_661_000 → "1:01:01".
13. `format::duration_string_rules` — the vectors of behavior 5.4: 0 → "None"; 5 s → "5s"; 61 min → "1h 1m"; 25 h → "1d 1h"; 1 d 5 m → "1d 5m"; 1 h 0 m 5 s → "1h".
14. `format::relative_date_thresholds` — difference −8 → absolute; −1 → upcoming; 0 → Today; 1 → 1 day ago; 6 → "6 days ago"; 7 → absolute.
15. `format::relative_time_span_never_and_just_now` — `epoch = 0` → Never; `now - epoch = 59_999` → JustNow; 60_000 → minute tier.
16. `stats::mean_of_entry_means_example` — EC-42-S-08 numbers (7.5).
17. `stats::mean_zero_scores_excluded` — EC-42-S-09.
18. `stats::mean_none_when_empty` — EC-42-S-10.
19. `stats::global_update_include_exclude_and_restrictions` — fixture: 5 rows, expected count (section 4.3).
20. `stats::completed_empty_entry_counts` — EC-42-S-04.
21. `storage::size_format_vectors` — 0 → "0 B"; 999 → "999 B"; 1000 → "1.00 KB"; 999_999 → "1000.00 KB" (EC-42-T-10); 1_000_000 → "1.00 MB"; 1_500_000_000 → "1.50 GB".
22. `storage::build_snapshot_sort_is_stable` — EC-42-T-05.
23. `storage::build_snapshot_all_drops_hidden_category` — EC-42-T-09.
24. `storage::color_is_deterministic` — same id → same colour across two calls and processes.
25. `storage::donut_empty_total_has_no_arcs` — EC-42-T-11.
26. `storage::donut_angles_sum_to_180` — sum of sweeps = 180 for non-zero total.
27. `storage::perf_2000_entries` — EC-42-T-12 (under 50 ms, `#[ignore]` on slow CI).

### 5.2 kuro-data (SQLite, in-memory)

28. `history::manga_list_latest_chapter_only` — EC-42-H-01.
29. `history::manga_list_search_escapes_wildcards` — EC-42-H-02.
30. `history::manga_list_search_ascii_case_insensitive` — EC-42-H-03.
31. `history::reset_row_hides_from_list_keeps_time` — EC-42-H-11 + EC-42-S-01.
32. `history::reset_entry_resets_all_rows` — EC-42-H-12.
33. `history::delete_all_drops_time_read` — EC-42-H-10 (manga read duration becomes 0).
34. `history::latest_manga_not_limited_to_max_row` — behavior 1.1 note (`getLatestHistory` semantics).
35. `updates::window_uses_upload_date` — EC-42-U-01.
36. `updates::favorites_only_and_fetched_after_added` — `favorite = 1 AND date_fetch > date_added`.
37. `updates::order_by_fetch_desc_and_limit_500` — 600 rows → 500 returned, newest first.
38. `stats::anime_watch_time_uses_library_predicate` — EC-42-S-03.
39. `stats::manga_read_duration_sums_all_history` — EC-42-S-01.
40. `storage::library_rows_dedupe_across_categories` — entry in two categories → one storage row (distinctBy).

### 5.3 Behavioral / integration (GTK, under Xvfb or `GDK_BACKEND=broadway`; optional in CI)

41. `ui::history::resume_opens_next_unit` — click row → reader opened with the expected chapter id (mock opener).
42. `ui::history::remove_dialog_checkbox_maps_to_entry_reset` — confirm with checkbox → `reset_manga_entry` called; without → `reset_manga_row`.
43. `ui::updates::selection_mode_hides_cover_and_indicator_actions`.
44. `ui::updates::escape_clears_selection_before_navigation`.
45. `ui::storage::delete_confirm_calls_delete_once` — double-click OK → single deletion call.
46. `ui::stats::loading_then_values` — snapshot injection.

### 5.4 Event-channel tests

47. `shared::download_cache_broadcast_two_subscribers` — EC-42-T-01.
48. `shared::reselect_resumes_visible_subtab` — EC-42-H-08.

---

## 6. Fixtures

Derived from the Compose preview providers in scope (values are the examples used by the Kotlin previews; they are reused as deterministic test data, not as shipped sample content).

### 6.1 History fixtures (from `MangaHistoryWithRelationsProvider.kt:9-58`, `AnimeHistoryWithRelationsProvider.kt:9-55`)

- `simple`: `id = 1, chapterId = 2, mangaId = 3, title = "Test Title", chapterNumber = 10.2, readAt = 1697247357000 ms, readDuration = 123, cover { mangaId 3, sourceId 4, favorite false, url "https://example.com/cover.png", lastModified 5 }`.
- `historyWithoutReadAt`: as `simple` with `readAt = null` (exercises the empty-timestamp path).
- `historyWithNegativeChapterNumber`: as `simple` with `chapterNumber = -2.0` (exercises time-only subtitle, EC-42-H-20).
- Anime: `episodeNumber` same values; no `readDuration`.

### 6.2 History screen states (from `MangaHistoryScreenModelStateProvider.kt:13-70`, anime identical)

1. `multiPage`: header(today), 3 items; header(today−1), 1 item; header(today−2), 7 items (titles "Example Title N" with N = 1..).
2. `shortRecent`: header(today), 1 item.
3. `shortFuture`: header(tomorrow), 1 item (exercises the "upcoming" relative text).
4. `empty`: list = [].
5. `loadingWithSearchQuery`: list = null, searchQuery = "Example Search Query".
6. `loading`: list = null.

Random fields in the Kotlin fixtures (`Random.nextLong()` ids, `Random.nextDouble()` chapter numbers, `Random.nextBoolean()` favorite) are replaced by fixed values in Rust fixtures so tests are deterministic.

### 6.3 Other fixtures

- Updates: 12 rows over 4 days (fixed fetch times at local noon of today−0..3), 2 manga, 1 anime with fillermark, progress values: manga `lastPageRead` 0, 4; anime `lastSecondSeen` 0, 754_000 (12:34), `totalSeconds` 1_440_000 (24:00).
- Storage: 20 entries with sizes `index * 10_000_000` (Kotlin `StorageScreenContentPreview`), titles "Title N", counts `100 * index`, 10 categories (`Category N`), selected category index 0 (All). Tablet and phone variants use the same data.
- Stats: manga `libraryMangaCount = 3`, `completed = 1`, `read = 2h 5m` (7,500,000 ms); titles `global = 2, started = 2, local = 0`; chapters `total 120, read 45, downloaded 12`; trackers `tracked 2, mean 7.40, used 2`. Expected display: "3", "2h 5m", "1", "2", "2", "0", "120", "45", "12", "2", "7.40 ★", "2". (These are PROPOSAL test values, not shipped data.)

---

## 7. Failure-mode matrix

| Failure | Where | Aniyomi result | Kurobayomi result |
|---|---|---|---|
| DB read error (history, updates) | view models | InternalError snackbar; history keeps last list; updates stuck loading | toast + retry; updates shows error row |
| DB write error (remove, clear) | repositories | silent; clear shows no snackbar | toast; no state change |
| Download cache not ready | storage/updates | storage stuck loading (EC-42-T-01) | snapshot shows "Scanning downloads…" spinner, then data |
| Tracker unknown id | stats | forever loading (EC-42-S-06) | N/A for that section |
| Source uninstalled | updates/storage delete | silently skipped | toast (EC-42-T-15) |
| Library update already running | updates | no spinner, event `started=false` | toast (EC-42-U-09) |
| Disk full during download delete | downloads (area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared) | error via download manager | toast with path |
| Concurrent select + refresh | updates | stale bounds (EC-42-U-05) | recomputed bounds |

---

## 8. Race conditions

1. **History query vs removal**: removal writes, query re-runs on `history` table change (50 ms coalesce). A removed row can flash for one frame (Kotlin: same). Keep.
2. **Updates combine**: `downloadCache.changes`, `queueState`, DB updates all trigger recomputation; `collectLatest` cancels the previous compute. Rust: `tokio::select!` with generation counter; discard stale results (rust design 2).
3. **Download status vs row replacement**: a progress event for a row not yet in the list is dropped (Kotlin: index −1 → no-op). Rust: keep the same, but store the last status in a map so a later list build picks it up (fixes a flicker).
4. **Storage delete vs emission**: `deleteEntry` runs `launchNonCancellable` and the list updates later via cache signal. Rust: optimistic removal from the local list plus cache signal to confirm (proposal).
5. **Stats compute vs navigation away**: Kotlin `launchIO` in `init` keeps running (screen model cancelled after dispose; not a leak). Rust: `CancellationToken` cancelled on widget destroy; results dropped.
6. **Two rapid reselects** (EC-42-H-08 area): `resumeLastEpisodeSeenEvent` is a rendezvous channel; a second reselect while the first is resolving suspends the sender. Rust: `async_channel::bounded(1)` with `try_send` (drop duplicate).
7. **Category move + favorite** (EC-42-H-16): two launches without ordering. Rust: one transaction (set favorite and categories together).

---

## 9. Items to decide with the product owner (not blocking)

1. EC-42-H-17: migrate navigation asymmetry (manga → new entry, anime → old entry).
2. EC-42-U-08: confirm per-row delete from the download indicator (data-loss risk).
3. EC-42-T-03: empty-state text for storage.
4. EC-42-T-14: local-source delete copy ("Delete downloaded chapters?" is misleading for local entries).
5. EC-42-S-12: download count includes orphan directories (show "in library" count instead?).
6. EC-42-H-05: confirm the fixed next-item behavior versus Aniyomi parity.
