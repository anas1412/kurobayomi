# Area 40 (part 2): edge cases, failure modes, races, tests

Scope: the 12 part-2 files (`p2-behavior.md` section 0). Behavior references: `B§n`. Rust references: `R§n` (`p2-rust-design.md`).

## 1. Upstream tests

Checked: every `*Test.kt` / `*Tests.kt` under `app/src/test`, `core/common/src/test`, `domain/src/test` (8 files). None references the classes, functions or files of this scope (`MangaSearchScreenModel`, `BrowseMangaSourceScreenModel`, `MangaSourcesScreenModel`, `MangaSourcesFilterScreenModel`, `MigrateMangaSourceScreenModel`, `GetEnabledMangaSources`, `GetLanguagesWithMangaSources`, `GetMangaSourcesWithFavoriteCount`, `SourcePagingSource`, `Pins`, `LocaleHelper`, `TriState`, `FilterList`, `NetworkToLocalManga`, `GetDuplicateLibraryManga`, `SetMigrateSorting`).

Result: no upstream test cases to port for this area. All tests in §7 are new.

## 2. Edge cases (by area)

ID format: `<area>-EC-<nn>` or the short IDs used by the other parts (`E-SL`, `E-SF`, `E-MS`, `B-EC`, `E-FL`, `GS-EC`, `E-MG`, `E-FAV`, `E-PG`). "Expected (Kurobayomi)" is the required Rust behavior; "Kotlin" is what the upstream code does.

### 2.1 Sources list (E-SL)

| ID | Input | Kotlin | Expected (Kurobayomi) | Task |
|---|---|---|---|---|
| E-SL-01 | Language not in `source_languages`, pinned | hidden from list (`GetEnabledMangaSources.kt:32`) | hidden; pin kept in config | T-40b-010 |
| E-SL-02 | last-used source disabled | last-used header disappears (`:33` before `:44`) | same | T-40b-010 |
| E-SL-03 | last-used id = -1 (default) | no copy | no copy | T-40b-010 |
| E-SL-04 | source both pinned and last-used | two entries: LAST_USED copy (pin code `0b01`) and PINNED original (`0b11`) | same; the copy's pin bits (`Pins::xor`) are not persisted | T-40b-010 |
| E-SL-05 | source with lang `""` | group `""` last (`MangaSourcesScreenModel.kt:80-81`) | same; header label = system language name | T-40b-010 |
| E-SL-06 | two language codes that differ only by UTF-16 vs UTF-8 order (supplementary plane vs U+E000..U+FFFF) | `String.compareTo` code units | Rust uses `str::cmp` (code point order). Difference only for such codes. Accepted; test documents it | T-40b-010 |
| E-SL-07 | `supportsLatest = false` | no Latest button | no Latest button; row click still Popular | T-40b-010 |
| E-SL-08 | pre-init empty catalogue (`sourcesMapFlow` empty) | empty state shown before extensions load (B§2.4) | spinner until `is_initialized()` | T-40b-003 |
| E-SL-09 | pin then unpin rapidly | two `getAndSet` calls on the same set (`ToggleMangaSourcePin.kt:11-16`); each reads `pinned` fresh | config writer serializes writes; last write wins; no lost toggle | T-40b-011 |
| E-SL-10 | `FailedFetchingSources` | snackbar `internal_error` | toast; list keeps previous items | T-40b-012 |

### 2.2 Sources filter (E-SF)

| ID | Input | Kotlin | Expected | Task |
|---|---|---|---|---|
| E-SF-01 | language off | its sources hidden from the Sources page too | same | T-40b-021 |
| E-SF-02 | all sources of a language disabled, language on | header on, no visible sources | same | T-40b-021 |
| E-SF-03 | two lang codes with equal localized display names | `TreeMap` merges groups; one group lost (`GetLanguagesWithMangaSources.kt:29-31`) | both kept, tie by code (D-SF-01) | T-40b-020 |
| E-SF-04 | local source | not listed (online only) | not listed | T-40b-020 |
| E-SF-05 | repository emits error | `State.Error`, toast, pop (`MangaSourcesFilterScreen.kt:30-37`) | same | T-40b-022 |
| E-SF-06 | `toggle_source` on an id not in set | adds to disabled | same | T-40b-021 |

### 2.3 Migrate source picker (E-MS)

| ID | Input | Kotlin | Expected | Task |
|---|---|---|---|---|
| E-MS-01 | no favorites anywhere | empty list, empty state | same | T-40b-070 |
| E-MS-02 | local source has favorites | filtered out (`GetMangaSourcesWithFavoriteCount.kt:25`) | same | T-40b-070 |
| E-MS-03 | stub source with favorites | sorted first in ASC and TOTAL (`:37-39`, `:45-47`) | same; in DESC last | T-40b-071 |
| E-MS-04 | equal counts or equal names | input order (SQL group order, no ORDER BY) | `ORDER BY source` in query; stable sort | T-40b-070 |
| E-MS-05 | toggle while preferences not yet emitted | state defaults used (`MigrateMangaSourceScreenModel.kt:61-81`) | read from config store | T-40b-072 |
| E-MS-06 | DB error on favorite count | `FailedFetchingSourcesWithCount` not shown | toast `internal_error` (D-MG-02) | T-40b-074 |
| E-MS-07 | name collation | `compareToWithCollator`, locale-dependent, primary strength | ICU primary collator, same locale | T-40b-071 |

### 2.4 Browse one source (B-EC)

| ID | Input | Kotlin | Expected | Task |
|---|---|---|---|---|
| B-EC-01 | open from Sources with `Listing.Popular` | `listing = Popular`, filters = source defaults in state only | same | T-40b-030 |
| B-EC-02 | open with a stored search query (global search row) | listing = `Search(query, defaults)`, toolbar = query | same | T-40b-030 |
| B-EC-03 | page 1 returns zero items | paging error `NoChaptersException` -> `no_results_found` | same | T-40b-032 |
| B-EC-04 | page 2 returns zero items with `hasNextPage = true` | error appended (`MangaSourcePagingSource.kt:46-54`) | `ErrorAppend`; retry = that page | T-40b-032 |
| B-EC-05 | `hasNextPage = false` on the last page | `nextKey = null` | `EndOfList` | T-40b-032 |
| B-EC-06 | source exception on page 1 | `LoadResult.Error` | `ErrorFirst(Source(msg))` | T-40b-032 |
| B-EC-07 | source is not a `CatalogueSource` (e.g. `StubMangaSource`) | `MangaSourceRepositoryImpl` casts `as CatalogueSource` -> `ClassCastException`, uncaught (`MangaSourceRepositoryImpl.kt:76,81,86`) | the stub page never starts a listing (stub branch first, B§4.11) | T-40b-030 |
| B-EC-08 | same query twice | new `Search` each time, restart (B§4.4.1) | restart (generation bump) | T-40b-030 |
| B-EC-09 | search with blank query `""` | sent as `""` (browse-with-filters) | same | T-40b-030 |
| B-EC-10 | navigate-up with active search and toolbar text | clears toolbar text first (B§4.6.2) | same | T-40b-036 |
| B-EC-11 | genre matches nothing | text search with the genre (B§4.7) | same | T-40b-037 |
| B-EC-12 | add to library: `changeMangaFavorite` after `setMangaDefaultChapterFlags` | `updateManga` writes the stale `chapterFlags` back (`MangaUpdate.kt`, `mangas.sq:156-178`) | partial write: favorite, dateAdded, coverLastModified only | T-40b-041 |
| B-EC-13 | remove favorite while covers are cached | `removeCovers` deletes files; sets `coverLastModified` if count > 0 (`MangaExtensions.kt:43-50`) | same | T-40b-041 |
| B-EC-14 | local source (`lang = "other"`) browse | help link `LocalMangaSource.HELP_URL` (B§4.11) | same | T-40b-036 |
| B-EC-15 | `hideInLibraryItems = true`, a row becomes favorite while on screen | filter applied per collected generation (B§4.4) | row stays until next listing; documented, identical | T-40b-034 |
| B-EC-16 | display mode stored as an unknown string | `COMPACT_GRID` (`LibraryDisplayMode.kt:24-31`) | same | T-40b-005 |
| B-EC-17 | last-used write in incognito | not written (`BrowseMangaSourceScreenModel.kt:100`) | same | T-40b-039 |
| B-EC-18 | columns pref = 0 | adaptive 128 px (B§4.5) | same | T-40b-035 |
| B-EC-19 | Sort filter with no state, first click | NPE crash (`SourceFilterMangaDialog.kt:138`) | select clicked, ascending (D-BR-01) | T-40b-052 |
| B-EC-20 | TriState raw 3 | IllegalStateException (`SourceFilterMangaDialog.kt:171`) | Disabled + log (D-BR-02) | T-40b-051 |
| B-EC-21 | Select state out of range | `options[selectedIndex]` throws (`SettingsItems.kt:316`) | clamp to 0 | T-40b-053 |
| B-EC-22 | `searchGenre` with a Select whose values mix strings and other types | index into string-filtered list (`:203-208`) | index into `values` (D-BR-03) | T-40b-037 |
| B-EC-23 | add to library with default category id that no longer exists, categories non-empty | falls to chooser (`:262-285`) | same; plus a log line | T-40b-042 |
| B-EC-24 | add with default `0` and no user categories | favorite with no categories | same | T-40b-042 |
| B-EC-25 | duplicate title with different case | SQL `LOWER` ASCII vs Kotlin `lowercase()` | Unicode compare in Rust (D-DUP-01) | T-40b-043 |
| B-EC-26 | long press on favorite with a duplicate | remove dialog wins (`:235-236`) | same | T-40b-044 |
| B-EC-27 | dialog opened twice quickly | `setDialog` overwrites | same | T-40b-044 |
| B-EC-28 | sources not initialised | `LoadingScreen` (`:83-86`) | same | T-40b-040 |
| B-EC-29 | `Listing` navigation arg is a string sentinel (`QUERY_POPULAR`) | matched by string | enum in Rust, no string | T-40b-030 |

### 2.5 Filter sheet (E-FL)

| ID | Input | Expected | Task |
|---|---|---|---|
| E-FL-01 | Header and Separator only | chip shown, sheet has headings and dividers only | T-40b-050 |
| E-FL-02 | Group inside Group | recursive rendering | T-40b-050 |
| E-FL-03 | Sort click on the selected item | flips ascending | T-40b-052 |
| E-FL-04 | Sort click on another item | keeps direction, selects item | T-40b-052 |
| E-FL-05 | Text filter typing | state updates per keystroke; no search | T-40b-053 |
| E-FL-06 | Reset | state = defaults; no search; sheet stays | T-40b-050 |
| E-FL-07 | Filter button | search with current state, then sheet closes | T-40b-050 |
| E-FL-08 | TriState cycle of 3 clicks | Disabled -> EnabledIs -> EnabledNot -> Disabled | T-40b-051 |

### 2.6 Global search (GS-EC)

| ID | Input | Kotlin | Expected | Task |
|---|---|---|---|---|
| GS-EC-01 | blank query on submit | nothing happens (`MangaSearchScreenModel.kt:128`) | same | T-40b-060 |
| GS-EC-02 | submit same query + same filter | nothing happens (`:129-130`) | same; errored sources are not retried | T-40b-060 |
| GS-EC-03 | filter change with same query | only newly selected sources fetched (`:139-145`, `:156-158`) | same | T-40b-060 |
| GS-EC-04 | two sources with equal `(empty, pinned, name (lang))` | TreeMap collision, one dropped (`:186`) | both kept in a `Vec` (D-GS-01) | T-40b-061 |
| GS-EC-05 | two results finish at the same time | lost update possible (`:192-197`) | single writer on the GTK thread (D-GS-02) | T-40b-060 |
| GS-EC-06 | new search while old requests run | `searchJob.cancel()` (`:135`); late results dropped by `isActive` | `JoinSet::abort_all` + generation check | T-40b-060 |
| GS-EC-07 | source returns error | `Error(e)`; no retry | same | T-40b-060 |
| GS-EC-08 | DB insert of a result fails | caught as source error (inside `try`, `:164-175`) | `Error(store message)` for that source | T-40b-060 |
| GS-EC-09 | `onlyShowHasResults` on, all sources empty | empty list | empty state | T-40b-063 |
| GS-EC-10 | `PinnedOnly` with zero pinned sources | nothing searched | empty state, no fallback (R§10 item 7) | T-40b-062 |
| GS-EC-11 | extension filter `""` | `extensionFilter` non-null -> All; `getSelectedSources` falls to enabled list (`:97-99`) | same | T-40b-062 |
| GS-EC-12 | extension filter names a package with no installed sources | empty selection | empty state | T-40b-062 |
| GS-EC-13 | single-result auto-open with one source returning one manga | `replace(MangaScreen(id, true))` | same | T-40b-064 |
| GS-EC-14 | single-result auto-open with error | results screen | same | T-40b-064 |
| GS-EC-15 | source row click with unsubmitted text | browse opens with the typed text (`GlobalMangaSearchScreen.kt:75`) | same (documented quirk) | T-40b-065 |
| GS-EC-16 | global search from deep link with no results | `GlobalMangaSearchScreen(query)` replace (`DeepLinkMangaScreen.kt:49`) | same | T-40b-065 |

### 2.7 Migrate search subclass (E-MG)

| ID | Input | Kotlin | Expected | Task |
|---|---|---|---|---|
| E-MG-01 | origin source has results and is not pinned | origin ordered by base comparator (B§10) | origin first (D-MG-01) | T-40b-067 |
| E-MG-02 | manga id not found | `getManga.await(mangaId)!!` NPE (`MigrateMangaSearchScreenModel.kt:22`) | error state, no panic | T-40b-067 |
| E-MG-03 | manga title empty | search with `""` -> ignored by `isNullOrBlank` | no search, empty state | T-40b-067 |

### 2.8 Favorites (E-FAV)

| ID | Input | Kotlin | Expected | Task |
|---|---|---|---|---|
| E-FAV-01 | add: `moveMangaToCategories` and `changeMangaFavorite` race | order undefined | sequential (D-FAV-01) | T-40b-042 |
| E-FAV-02 | `setMangaCategories` fails | logged, swallowed (`SetMangaCategories.kt:14-16`) | logged; favorite still set; no toast | T-40b-042 |
| E-FAV-03 | `updateManga` fails | not caught (`BrowseMangaSourceScreenModel.kt:251`) | toast `internal_error`, row rolled back (D-FAV-02) | T-40b-041 |
| E-FAV-04 | `dateAdded` | ms; `0` on remove | same | T-40b-041 |
| E-FAV-05 | `favorite_modified_at` trigger | seconds, set by trigger | not written by app | T-40b-041 |
| E-FAV-06 | choose categories with exclude list | exclude ignored (`BrowseMangaSourceScreen.kt:306`) | same | T-40b-045 |

### 2.9 Paging (E-PG)

| ID | Input | Expected | Task |
|---|---|---|---|
| E-PG-01 | 25 items/page, last visible index 49 of 75 | prefetch triggers (`49 + 25 >= 75`) | T-40b-032 |
| E-PG-02 | empty list | no prefetch | T-40b-032 |
| E-PG-03 | generation change during a page request | late page dropped | T-40b-032 |
| E-PG-04 | page exceeds `u32` | not possible; `page + 1` with `u32::MAX` unreachable in practice | T-40b-032 |

## 3. Race conditions

| ID | Where | Race | Mitigation |
|---|---|---|---|
| R-01 | `updateItem` (`MangaSearchScreenModel.kt:192-197`) | concurrent read-modify-write of `items` | single writer on main; `apply(generation, ...)` (R§3.9) |
| R-02 | `addFavorite` default-category branch (`BrowseMangaSourceScreenModel.kt:264-266`) | `moveMangaToCategories` (IO) and `changeMangaFavorite` (main) unordered | sequential task (R§3.10) |
| R-03 | `changeMangaFavorite` vs `setMangaDefaultChapterFlags` | stale whole-row write (B-EC-12) | partial `set_favorite` |
| R-04 | `toggleSortingMode` / `toggleSortingDirection` (`MigrateMangaSourceScreenModel.kt:61-81`) | toggle from stale state after two fast taps | read from config store; apply in order |
| R-05 | `search()` vs `setSourceFilter()` | both call `search()`; second call sees the first's `lastQuery` | single-threaded model on main (R§1.2) |
| R-06 | `BrowseMangaSourceScreen.queryEvent` (companion `Channel`) | a send with no collector suspends the caller | direct method on the page (D-CH-01) |
| R-07 | `GlobalMangaSearchScreen` single-result auto-open | `LaunchedEffect(state.items)` may run after the result already changed | decision on the result value at the time of apply |
| R-08 | `SourceRegistry` changes during a search | source removed while its request is in flight | result for an unknown source is dropped (`apply` finds no Loading entry) |
| R-09 | config writes from two pages | both write `browse.toml` | writes serialized on main; atomic rename |

## 4. Failure modes and user-visible result

| Failure | Where | Kotlin visible | Kurobayomi visible |
|---|---|---|---|
| Source `getSearchManga` throws | global search | per-source error row with message (`:171-175`) | same |
| Source `getPopularManga` throws | browse | error state (paging) | same (P1§3.3) |
| Empty page | browse | `no_results_found` | same |
| Source not installed (stub) | browse | `MissingSourceScreen` | same |
| Category write fails | add to library | nothing | log only |
| Favorite write fails | add/remove | nothing (exception swallowed in coroutine) | toast + rollback |
| Preference write fails | toggles | nothing | log, UI unchanged |
| DB favorite count fails | migrate picker | nothing (event not shown) | toast `internal_error` |
| Sources flow fails | Sources page | snackbar `internal_error` | toast `internal_error` |
| `SourceNotInstalledException` | browse/global | `loader_not_implemented_error` text | same text |

## 5. Logging (ERROR sites in scope)

| Location | Kotlin | Kurobayomi |
|---|---|---|
| `MangaSourcesScreenModel.kt:52-55` | `logcat(LogPriority.ERROR, it)` | `tracing::error!` |
| `MigrateMangaSourceScreenModel.kt:38-40` | `logcat(LogPriority.ERROR, it)` | `tracing::error!` |
| `SetMangaCategories.kt:15` (domain, called from scope) | `logcat(LogPriority.ERROR, e)` | `tracing::error!` |
| Others in scope | no logging | add `tracing::warn!` only for D-BR-01/02 fallbacks |

## 6. Fixtures

| Fixture | Content | Used by |
|---|---|---|
| `sources_mixed` | 9 sources: en x3, ja x2, all x1, other (local, id 0), pinned id 101 (en), last-used id 205 (ja), disabled id 310 (en), stub id 999 (lang "") | E-SL, E-SF, E-MS |
| `langs_equal_display` | two codes with equal display names (synthetic: `xx-A`, `xx-B`) | E-SF-03 |
| `filters_genre` | `Group("Genres", [CheckBox("Action"), TriState("Drama"), Text("Tag")])`, `Select("Status", ["Any", "Ongoing", 5]/*mixed*/)`, `Sort("Order", ["Latest", "Name"], None)` | B-EC-19, B-EC-20, B-EC-22, E-FL-* |
| `filters_empty_headers` | `Header`, `Separator` only | E-FL-01 |
| `manga_page_1`, `manga_page_2_empty`, `manga_page_2_next_no` | 25, 0, 3 items with `has_next_page` true/false | B-EC-03..05, E-PG-01 |
| `favorites_titles` | `["One Piece", "one piece", "ＯＮＥ ＰＩＥＣＥ", "Berserk"]` | B-EC-25 |
| `migrate_counts` | sources with counts 0, 3, 12, stub with 5 | E-MS-03, E-MS-04 |
| `global_results_matrix` | 5 sources: Success non-empty, Success empty, Error, Loading, duplicate-name collision pair | GS-EC-04, sort order, visibility |
| `now_ms = 1_700_000_000_000` | fixed clock | favorite_change |

Fixtures live in `src/browse/model/tests/fixtures.rs` (test-only module, `#[cfg(test)]`). Store them as Rust constructors, not JSON.

## 7. Tests to write

### 7.1 Unit tests (pure, `src/browse/model/*`)

| Test | Module | Asserts |
|---|---|---|
| `build_source_list_groups_and_order` | source_list | order: last-used, pinned, languages (code order), `""` last; header keys exact |
| `build_source_list_pinned_and_last_used_duplicate` | source_list | E-SL-04 |
| `group_key_cmp_is_total_order` | source_list | antisymmetry, transitivity over fixtures; `Equal` for equal keys |
| `languages_with_sources_enabled_first` | source_filter | enabled groups first, then `compare_languages` (`all` first among enabled) |
| `languages_with_sources_no_merge` | source_filter | E-SF-03: both groups kept |
| `toggle_language_and_source` | source_filter | add/remove; idempotent pairs |
| `sort_migrate_sources_asc_desc_stub` | migrate_sources | E-MS-03: stubs first ASC, last DESC; ties keep input order |
| `sort_migrate_total_mode` | migrate_sources | counts ascending/descending |
| `primary_cmp_ignores_case_and_accents` | collate | `"é" == "e"` at primary; `"A" == "a"` |
| `case_insensitive_cmp_matches_java_fixtures` | collate | pairs `("abc","ABD")` -> Less; `("a","A")` -> Equal; `("Straße","strasse")` -> Greater (per-char fold: U+00DF is above U+0073, same as Java `String.CASE_INSENSITIVE_ORDER`) |
| `listing_from_nav` | listing | Popular/Latest/Search mapping; `is_user_query` truth table |
| `to_remote_blank_query` | listing | `Search{None}` -> `text == ""` (B-EC-09) |
| `tristate_cycle_and_raw` | filter_tree | next() cycle; from_raw 0..2; 3 -> Err (B-EC-20) |
| `sort_click_matrix` | filter_tree | E-FL-03/04, D-BR-01 for None |
| `sort_arrow_matrix` | filter_tree | arrow per item |
| `select_text_clamp` | filter_tree | B-EC-21 |
| `apply_genre_group_include` | filter_tree | TriState set to INCLUDE; CheckBox true; return true |
| `apply_genre_select_index_into_values` | filter_tree | B-EC-22 (D-BR-03): index in full `values` |
| `apply_genre_first_match_wins` | filter_tree | outer loop exit (labelled break) |
| `apply_genre_no_match` | filter_tree | returns false, tree unchanged |
| `browse_state_search_restarts_generation` | browse_source | B-EC-08: generation increments on each search |
| `browse_state_search_uses_defaults_not_state_filters` | browse_source | B§4.6.1: from Popular, filters = defaults |
| `browse_state_chip_resets_filters` | browse_source | B§4.6 chips |
| `browse_state_navigate_up` | browse_source | B-EC-10 truth table |
| `browse_state_init_search_listing` | browse_source | B-EC-02 |
| `paging_load_outcome` | paging | empty -> NoResults (B-EC-03); error -> Source; next key rule (B-EC-05) |
| `should_prefetch_matrix` | paging | E-PG-01/02 |
| `favorite_change_add_remove` | favorites | dateAdded 0 on remove; now on add; cover removal flag |
| `add_favorite_plan_matrix` | favorites | default -1 + categories -> chooser; 0 -> empty; existing id -> set [id]; missing id + categories -> chooser (B-EC-23) |
| `long_click_matrix` | favorites | favorite wins over duplicate |
| `duplicate_title_unicode` | favorites | B-EC-25 (Unicode lowercase compare) |
| `global_sort_items_nonempty_first` | global_search | order: non-empty, pinned, name |
| `global_sort_keeps_collisions` | global_search | GS-EC-04: two entries kept |
| `global_begin_search_noop_rules` | global_search | GS-EC-01/02 |
| `global_begin_search_same_query_keeps_results` | global_search | GS-EC-03 |
| `global_apply_stale_generation_dropped` | global_search | R-01, R-08 |
| `global_visible_has_results` | global_search | GS-EC-09 |
| `global_progress_total` | global_search | progress counts non-Loading |
| `migrate_search_origin_first` | migrate_search | E-MG-01 (D-MG-01) |
| `display name` tests | part 1 | owned by part 1 (not duplicated) |

Every test is a `#[test]`. Property tests (`proptest`, dev-dependency) for: `group_key_cmp` total order; `primary_cmp` total order; `sort_items` idempotent; `apply` never changes item count.

### 7.2 Integration tests (services with fakes, tokio `#[tokio::test]`)

| Test | Fake | Asserts |
|---|---|---|
| `global_search_five_sources_one_fails` | fake sources (one Err, others ok) | results arrive; the failing row is `Error`; progress = total |
| `global_search_semaphore_limits_to_5` | fake source sleeping, counting concurrency | max in-flight = 5 |
| `global_search_abort_on_new_query` | fake source blocking | old requests aborted; late results ignored |
| `browse_loader_demand_driven` | fake paged source | only requested pages fetched; no eager fetch |
| `add_favorite_sequence` | fake stores recording calls | order: links, flags, favorite, trackers (D-FAV-01) |
| `remove_favorite_clears_cover_flag` | fake cover cache returns 2 | `cover_last_modified_ms` set |
| `favorite_write_failure_rolls_back` | fake store fails on favorite | row flag restored; toast event emitted (D-FAV-02) |
| `migrate_counts_query_orders_by_source` | rusqlite in-memory DB with fixture rows | `ORDER BY source`; favorite-only counts |
| `pre_init_sources_spinner` | registry with `is_initialized=false` | list stays loading (D-SS-01) |

### 7.3 Manual acceptance checks (GTK, real desktop)

Each check lists the screen and the expected visible result.
1. Sources page: a fresh profile with 0 extensions shows the spinner, not "No source found", until the registry is initialised (E-SL-08).
2. Pin a source: moves to the Pinned group immediately; restart keeps it.
3. Browse a source: Popular loads 25 items on the first screen; scrolling to the 20th item from the end loads the next page.
4. Search with a term; change the filter with a Select; press Filter: results change; the sheet closes.
5. Sort filter: click the selected item twice: arrow flips; the list reloads only after Filter.
6. Global search with two pinned sources and one unpinned: only pinned are searched by default; "All" chip then searches the unpinned one too (B§9.7).
7. Long press a non-library row whose title matches a library row in another source: duplicate dialog; "Migrate" opens the migrate confirm.
8. Add to library with default "Always ask" and two categories: chooser shows; confirm adds to the checked category; the chapter filter defaults of the library apply (B-EC-12 check: open the manga, chapter filter equals the default).
9. Remove from library: confirm dialog; cover is removed from cache; row shows no favorite badge.
10. Migrate picker: sort toggles reorder; restart keeps the sort choice; stub sources at the top (ascending) and at the bottom (descending).
11. Navigate-up with a search active and toolbar text cleared: back clears search, second back pops.
12. Back from a stub source: the Missing screen; back pops.

## 8. Test data policy

No network in unit tests. Source responses are fixtures. No real manga covers in tests. The rusqlite test DB is in-memory (`Connection::open_in_memory`), applying the same `mangas.sq` schema as area 03.
