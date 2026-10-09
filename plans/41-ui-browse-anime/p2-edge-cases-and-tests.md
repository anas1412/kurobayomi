# Area 41 part 2: edge cases, race conditions, tests

Scope recap: sources list, visibility page, browse one source, source filter sheet, global search, migration sources tab (`p2-inventory.md`). IDs in this file are referenced from `p2-behavior.md`, `p2-rust-design.md`, `p2-ui-parity.md` and `p2-tasks.md`.

Kind column: **U** = pure unit test (no GTK, no DB), **I** = integration test with fake catalog, fake source and in-memory repo, **M** = manual check on the GTK build.

## 1. Upstream tests

No `*Test.kt` file in this scope. A repository-wide search for the classes and helpers in scope (`AnimeSourcesScreenModel`, `BrowseAnimeSourceScreenModel`, `AnimeSearchScreenModel`, `GlobalAnimeSearchScreenModel`, `GetEnabledAnimeSources`, `GetLanguagesWithAnimeSources`, `GetAnimeSourcesWithFavoriteCount`, `AnimeFilter`, `TriState`, `LocaleHelper`, `Pins`, `LibraryDisplayMode`, `GetRemoteAnime`, `SourcePreferences`) found no test. Nothing to port. Behavior is pinned by the new tests listed in section 6.

## 2. Sources list (Feature A)

| ID | Scenario | Expected in Kurobayomi | Kotlin reference | Kind |
|---|---|---|---|---|
| E-SRC-01 | No installed source has an enabled language and local source is absent | Empty state "No source found" | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:51-54` | U |
| E-SRC-02 | Upstream sources flow fails before its first emission | Show inline error row with "Retry"; loading indicator cleared. Kotlin keeps `isLoading = true` forever plus a snackbar (OBSERVED) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt:43-49`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt:53-88` | I |
| E-SRC-03 | Last-used source is also pinned | Appears in "Last used" and in "Pinned"; both rows open the same browse | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:30-38` | U |
| E-SRC-04 | Source with empty language `""` | Grouped last; header text is the system language display name (for example "Arabic" on an Arabic system) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt:62-63`, `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:61` | U |
| E-SRC-05 | Source disabled while its browse page is open | Browse page stays usable; the row disappears from the list on return | `app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt:15-19` | M |
| E-SRC-06 | Pinned id stored for a source that is no longer installed | Ignored (no row); id stays in settings (no cleanup) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:18-41` | U |
| E-SRC-07 | Unparseable id string in `hidden_anime_catalogues` / `pinned_anime_catalogues` | Skip the entry (PROPOSAL). Kotlin keeps the string inert; no visible difference | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:56,62` | U |
| E-SRC-08 | Local source disabled id in settings (`"0"`) | Local source hidden in list (Kotlin filter applies to all ids, only the Disable menu item is hidden) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:28` | U |
| E-SRC-09 | Two sources with the same name and language | Both rows, order by stable input order (name equal) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:29` | U |
| E-SRC-10 | Name-only sort with mixed case ("beta" vs "Alpha") | "Alpha" before "beta" (case-insensitive) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:29` | U |
| E-SRC-11 | Pin toggle on a last-used copy | Toggles pin for the source id (both copies update) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSourcePin.kt:11-16` | I |
| E-SRC-12 | Preference write race from two windows | Last write wins (single writer in Kurobayomi) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt:16-18` (getAndSet) | I |

## 3. Visibility page (Feature B)

| ID | Scenario | Expected | Kotlin reference | Kind |
|---|---|---|---|---|
| E-VIS-01 | Disable every source of a language, keep the language on | Header shows, no rows under it | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt:78-91` | I |
| E-VIS-02 | Disable a language | Its sources are hidden, per-source disabled flags unchanged | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt:78-91` | I |
| E-VIS-03 | Zero online sources | Empty state "No installed source found" | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt:39-45` | I |
| E-VIS-04 | Local source and stubs | Not listed on this page (only `AnimeHttpSource`) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt:20`, `data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt:31-37` | U |
| E-VIS-05 | Language order | Enabled languages first; "all" first within enabled; then display name | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt:27-31`, `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:17-25` | U |
| E-VIS-06 | Source order inside a language | Enabled sources first, then name (case-insensitive) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt:22-25` | U |
| E-VIS-07 | Error from the combined flow | Toast "InternalError: Check crash logs for further information" and pop | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt:30-37` (wrapper) | I |

## 4. Browse one source (Feature C)

| ID | Scenario | Expected in Kurobayomi | Kotlin reference | Kind |
|---|---|---|---|---|
| E-BR-01 | Browse opened for an uninstalled source id | Missing-source screen "Source not installed: NAME (LANG)" or the id when name/lang unknown; last-used still written | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:101-107`, `domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:101-102`, `app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:117-126` | I |
| E-BR-02 | Favorite toggle while the list item snapshot is stale (another write changed the title or lastUpdate after the item was loaded) | Only `favorite`, `date_added`, cover and background timestamps change (D-3). Kotlin overwrites the full row with the snapshot | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:243`, `domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt:39-60`, `data/src/main/sqldelightanime/dataanime/animes.sq` update | I |
| E-BR-03 | Text search equal to `eu.kanade.domain.source.anime.interactor.POPULAR` or `eu.kanade.domain.source.anime.interactor.LATEST` | Treated as the Popular or Latest listing (sentinel collision). Kurobayomi keeps the Kotlin behavior only at the nav-arg boundary and sends user text as Search; PROPOSAL: never map user text to a sentinel | `domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt:12-16`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:336-344` | U |
| E-BR-04 | Genre match on a Text, Select or Group child (non-TriState, non-CheckBox child with a matching name) | `genreExists = true` with no state change, listing `Search(query = null)` → empty query sent to the source. PROPOSAL: treat as "not found", use the genre as text query | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:182-217` | U |
| E-BR-05 | Back pressed while Popular is shown and toolbar text was typed but not submitted | Clears toolbar text first (listing unchanged), second back pops | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:92-99` | M |
| E-BR-06 | Re-submit the same search text | Pager restarts from page 1 (no dedupe) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:163-176`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilterList.kt:10-12` | I |
| E-BR-07 | Source returns an empty page after a non-empty page with `hasNextPage = true` | Append error "No results found" with Retry snackbar; list keeps the items loaded so far | `data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt:43-51` | I |
| E-BR-08 | Incognito on (global or per extension) | Last-used not written; favorites still written | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeIncognitoState.kt:15-20`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:100-102` | I |
| E-BR-09 | Long list (thousands of entries) | Covers and per-row DB subscriptions only for bound rows. Kotlin creates one eager StateFlow per loaded item that lives until screen disposal | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:114-121` | I |
| E-BR-10 | Non-favorite title changed by source after first browse | Kotlin shows the stored title (in-memory override discarded by the re-read). Default in Kurobayomi: same (Q-1). | `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt:19-23`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:117` | I |
| E-BR-11 | Hide-in-library enabled; a visible item is favorited while the screen is open | The item stays visible until the screen is recreated | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:121` | M |
| E-BR-12 | Favorite database write fails | Kotlin: silent (returns false, ignored). Kurobayomi: snackbar "InternalError: Check crash logs for further information" and the row keeps its previous state | `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:140-148`, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt:15-17` | I |
| E-BR-13 | More than one favorite duplicate exists | Dialog shows the first row returned by SQL (no ORDER BY, undefined). Kurobayomi: order by `_id` ascending (deterministic) | `data/src/main/sqldelightanime/dataanime/animes.sq:128-133` | I |
| E-BR-14 | Long press on a favorite that also has a duplicate | Remove dialog (favorite check wins) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:232-249` | I |
| E-BR-15 | Default category id points to a deleted category | Resolves through case (b) or (c), never to a missing category | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:249-276`, `domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt:33-35` | I |
| E-BR-16 | Two long presses in quick succession | The second dialog replaces the first (single `dialog` field) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:314-316` | I |
| E-BR-17 | A source returns the same url twice on one page | Two rows for one anime (Kotlin). PROPOSAL: keep the first, drop repeats by url within a session | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:114-121` | I |
| E-BR-18 | Source returns `hasNextPage = true` forever with new urls | Paging continues until the user stops scrolling (no cap). Kurobayomi keeps no cap (parity) | `data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt:56` | I |
| E-BR-19 | `hasNextPage = false` on page 1 with items | Single page, no append requests | `data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt:53-57` | I |
| E-BR-20 | Source uninstalled while its browse page is open | Next request fails with `NotInstalled`, message "Source not found" | `data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt:49-51`, `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:33-35` | I |
| E-BR-21 | Hide-in-library hides every item on the loaded pages and no more pages exist | Kotlin: blank list (no empty message, the state machine only handles error or loading). PROPOSAL: show "No results found" empty state | `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:57-151` (presentation), `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:121` | I |
| E-BR-22 | Stored display mode `COVER_ONLY_GRID` | Rendered as compact grid; the menu has no selection for it | `domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt:26-41`, presentation `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:141` | U |
| E-BR-23 | Column preference negative | Kotlin `GridCells.Fixed(-1)` throws. PROPOSAL: values <= 0 mean adaptive | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:127-135` | U |
| E-BR-24 | Column preference very large | GridView shows at least one column; no clamp needed | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:134` | M |
| E-BR-25 | Popular chip pressed while Popular is already active | `resetFilters()` and `setListing(Popular)` run again; the pager restarts (Kotlin object identity: no change, distinctUntilChanged suppresses it). PROPOSAL: no restart when the listing is equal | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:157-163` | I |
| E-BR-26 | Open browse from global search with a null query | `Listing.valueOf(null)` gives Search(null), then the source receives `""` (empty search). PROPOSAL: open Popular when the query is blank | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:75`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:336-344` | I |
| E-BR-27 | Sort filter with `state == null` and the user clicks any value | Kotlin throws `NullPointerException`. Kurobayomi: treat as index 0 ascending (D-4) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt:137-141` | U |
| E-BR-28 | TriState stored value outside 0..2 | Kotlin throws `IllegalStateException`. Kurobayomi: `Ignore` plus a warning (D-4) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt:170-172` | U |

## 5. Global search (Feature E) and filter model (Feature D)

| ID | Scenario | Expected in Kurobayomi | Kotlin reference | Kind |
|---|---|---|---|---|
| E-GS-01 | Submit a blank query | No-op: results kept, no request | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:127` | U |
| E-GS-02 | Same query, source filter changes | Results for sources already present are reused; new sources start Loading; sources not in the new selection are removed. PinnedOnly → All adds only the newly selected sources | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:129-150` | I |
| E-GS-03 | Query A starts, query B starts before A finishes, A's result arrives after B started | A's result is dropped (generation guard, D-5). Kotlin writes it into B's state | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:152-178, 191-196` | I |
| E-GS-04 | Progress bar with one source failing | Failed counts toward progress. Bar hidden when progress is 0 or equal to total | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:206-207`, `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt:59` | U |
| E-GS-05 | Local source (lang "other") with "other" not enabled | Not searched (Kotlin search filter). Listed in the Sources tab (not filtered by language there) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:85-94`, `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:27` | U |
| E-GS-06 | Source error with `null` message (for example `NoEpisodesException` or a bare exception) | Error item with empty text (raw message). PROPOSAL: show the formatted message | `app/src/main/java/eu/kanade/presentation/browse/GlobalSearchResultItems.kt:82-94`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:170-173` | U |
| E-GS-07 | Enabled languages or pinned set changed while the global search screen is open | Not reflected until reopened (snapshot). PROPOSAL: keep the snapshot (documented) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:49-51` | M |
| E-GS-08 | Many global search screens opened in one session | One shared semaphore (D-6). Kotlin creates one 5-thread pool per screen and never shuts it down | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:46` | I |
| E-GS-09 | Extension filter names a package that is not installed | No sources selected: empty results, no spinner, single-result auto-open does not fire | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:96-108`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:41-64` | I |
| E-GS-10 | Results arrive while sorted order is shown | Sections move (non-empty first) as results arrive; no duplicate sections | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:181-196` | I |
| E-GS-11 | "Has results" enabled, app restarted | Still enabled (app-state key) | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:49-52` | I |
| E-GS-12 | Switch from PinnedOnly to All with no pinned sources | Full list is searched; the Pinned chip still shows as unselected | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreenModel.kt:25-28` | I |
| E-GS-13 | Source not found (`getOrStub`) during search | Not possible for installed sources. Stub sources are not in `getAll()` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:85-87` | U |
| E-GS-14 | Two results with the same name and language | Sort is stable by key; the order of the two is input order | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:58-64` | U |
| E-FD-01 | Stored TriState value outside 0..2 | See E-BR-28 | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt:170-172` | U |
| E-FD-02 | Cycle TriState: ignored → include → exclude → ignored | Exactly that cycle | `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:9-15` | U |
| E-FD-03 | Sort: click the selected index toggles direction; click another index keeps direction; arrow shows only on the selected index | As OBSERVED; null state treated as index 0 ascending (D-4) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt:126-151` | U |
| E-FD-04 | Reset in the sheet | Filters become source defaults; sheet stays open; listing unchanged until Filter | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:259`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:147-149` | I |
| E-FD-05 | Cancel the sheet after editing | Edits kept in memory (shared list); used by the next search. Kotlin behavior, documented | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt:41-43` | M |
| E-FD-06 | Group with a non-filter child (never happens in valid sources) | Ignored | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt:157-158` | U |
| E-FD-07 | Equality of two identical filter lists | Equal (Kurobayomi). Kotlin `AnimeFilterList.equals` is always false | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilterList.kt:10-12` | U |

## 6. Migration sources tab (Feature F)

| ID | Scenario | Expected in Kurobayomi | Kotlin reference | Kind |
|---|---|---|---|---|
| E-MS-01 | No favorites at all | Empty state "Your library is empty" (`information_empty_library`) | `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt:56-59` | I |
| E-MS-02 | Favorite count query fails before the first emission | Kotlin: `isLoading` stays true and the error event is never observed by the tab. Kurobayomi: error row with Retry | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceScreenModel.kt:36-50`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt` (no channel collection) | I |
| E-MS-03 | Stub source with favorites | Labeled "Not installed"; first in Ascending, last in Descending | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt:38-39`, `:53-56` | U |
| E-MS-04 | Tie in TOTAL mode | Kotlin keeps SQL GROUP BY order (undefined) and reverses it for Descending. Kurobayomi: ties ordered by source id ascending in both directions (deterministic; deviates from Kotlin's reversed tie order) | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt:47`, `:54-55` | U |
| E-MS-05 | Alphabetical with accents or non-Latin names | Locale collation (ICU, optional). Fallback: lowercase Unicode order | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt:40` | U |
| E-MS-06 | Blank source name | Shows the numeric id | `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt:173` | U |
| E-MS-07 | Toggle sort mode | Writes both preferences (mode and current direction) | `app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:9-12` | I |
| E-MS-08 | Local source has favorites | Excluded from the list | `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt:25` | U |
| E-MS-09 | Long press on a row | Copies the source id to the clipboard | `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt:65-68` | M |

## 7. Race conditions and concurrency

| ID | Race | Kotlin behavior | Kurobayomi rule | Test |
|---|---|---|---|---|
| RC-1 | Global search: late result from a superseded query | Written into the new state (E-GS-03) | Generation guard; cancel token drops pending requests | I |
| RC-2 | Browse: refresh while an append is in flight | Paging library discards the old append | `SourcePager` generation; `apply` drops stale results | U |
| RC-3 | Browse: favorite toggle races a page load for the same row | Full-row write can revert fields loaded by the page (E-BR-02) | Narrow update (D-3); row reloaded from DB after the write | I |
| RC-4 | Add flow: categories written after favorite | `moveAnimeToCategories` is launched, not awaited (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:301-308`); the category write can finish after `changeAnimeFavorite`. Visible result: the row appears in the library before its category | Kurobayomi awaits the category write, then the favorite write, in that order, on the DB worker | I |
| RC-5 | Genre search: channel send before the composed screen collects | `Channel` is rendezvous; the sender suspends until the screen is composed (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:328`) | Direct call on the browse page object; no channel | M |
| RC-6 | Screen popped while its coroutines still run | Kotlin keeps global search tasks running (no cancel on `searchJob`) | Cancel token on page drop; tasks check token before writing | I |
| RC-7 | Two browse pages for the same source open at once | Independent models; last-used written by both | Same | I |
| RC-8 | Preference write racing a read in the sources list | `getAndSet` on the preference | Single writer; watch channel emits the new snapshot | I |

## 8. Fixtures needed

| Fixture | Content | Used by |
|---|---|---|
| `FakeSource` (implements `AnimeSource`) | scripted popular, latest and search pages; `has_next_page` per page; `filter_list` with every `Filter` variant; `supports_latest` configurable; can return `SourceError` per call | E-BR-*, E-GS-*, E-FD-* |
| `FakeCatalog` (implements `SourceCatalog`) | installed sources: local (id 0, lang "other", name "Local anime source"), English (`en`), Multi (`all`), Arabic (`ar`), an empty-lang source (`""`); one stub (id 999, lang "", name ""); extension packages with source ids; `is_initialized` toggle | E-SRC-*, E-VIS-*, E-GS-* |
| `InMemoryAnimeRepo` (implements `AnimeRepo`) | rows with url, source, favorite, title; duplicate query using ASCII lower; per-source favorite counts; categories and links | E-BR-*, E-MS-* |
| `RecordingCover` / `RecordingBackground` | records `delete_for` calls and returns a configurable count | E-BR-02, RC-3 |
| `RecordingTrackBinder`, `RecordingEpisodeFlags` | record calls | E-BR-08 |
| `FixedClock` | returns a fixed ms epoch | E-BR-02 (date_added) |
| `StubLocaleText` | key to English text (from `p2-data-and-config.md` section 7); language display map for `ar`, `en`, `all`, `""` | E-SRC-04, E-VIS-05 |
| `Collate` fake | byte order (for determinism) and ICU (optional) | E-MS-05 |

Fixture files live under `tests/browse_anime/` in the Kurobayomi repo (PROPOSAL). No network. No real source binary.

## 9. Test list (new tests to write)

| Test ID | Covers | Kind |
|---|---|---|
| T-TEST-SECTIONS | E-SRC-01..E-SRC-04, E-SRC-09, E-SRC-10 (grouping, section order, empty language, duplicates) | U |
| T-TEST-LAST-USED-DUP | E-SRC-03, E-SRC-11 | U |
| T-TEST-ENABLED-ROWS | E-SRC-06, E-SRC-08, E-GS-05 (local rules) | U |
| T-TEST-VIS-ORDER | E-VIS-04, E-VIS-05, E-VIS-06 | U |
| T-TEST-LISTING | E-BR-03, E-BR-26, `Listing::from_nav_arg` | U |
| T-TEST-PAGER-TERMINAL | E-BR-07, E-BR-18, E-BR-19, E-BR-20, RC-2 | U (with fake source) |
| T-TEST-NETWORK-TO-LOCAL | E-BR-10, E-BR-17 (dedupe by url), row reuse | I |
| T-TEST-FAVORITE-WRITE | E-BR-02, RC-3, E-BR-12, E-BR-14 | I |
| T-TEST-DUPLICATE | E-BR-13, E-BR-14 | I |
| T-TEST-CATEGORY-RESOLUTION | E-BR-15, RC-4 | I |
| T-TEST-GENRE | E-BR-04 | U |
| T-TEST-FILTER-TRISTATE | E-FD-01, E-FD-02, E-BR-28 | U |
| T-TEST-FILTER-SORT | E-FD-03, E-BR-27 | U |
| T-TEST-FILTER-EQUALITY | E-FD-07 | U |
| T-TEST-GS-SORT | E-GS-04, E-GS-10, E-GS-14, `search_sort_key` | U |
| T-TEST-GS-REUSE | E-GS-01, E-GS-02, E-GS-12 | I |
| T-TEST-GS-STALE | E-GS-03, RC-1 | I |
| T-TEST-GS-SEMAPHORE | E-GS-08 (at most 5 concurrent calls, verified with a counting fake source) | I |
| T-TEST-GS-SINGLE | E-GS-09 and the auto-open rule | I |
| T-TEST-MS-SORT | E-MS-03, E-MS-04, E-MS-06, E-MS-08 | U |
| T-TEST-MS-PREFS | E-MS-07 | I |
| T-TEST-LOCALE-DISPLAY | E-SRC-04, `section_title` for "all", "other", "", zh-CN, zh-TW | U |
| T-TEST-ERROR-TEXT | `SourceError::user_message` for each variant (texts in `p2-data-and-config.md` 7) | U |
| T-TEST-UI-SMOKE | Manual: E-BR-05, E-BR-11, E-FD-05, E-MS-09, E-SRC-05 | M |

## 10. Manual checks on the GTK build

Each manual check is listed with the expected visible result in `p2-ui-parity.md`. Run on the Kurobayomi build with a fake source installed (section 8 fixture), in light and dark GTK themes:
1. Sources tab: sections and headers render; pin toggles; right-click shows the options dialog; Disable hidden for local.
2. Browse: chips switch listing; Filter chip appears only when the fixture defines filters; display mode radio persists after restart.
3. Long press (right-click) on a favorite → Remove dialog; on a non-favorite with a duplicate → duplicate dialog; else added instantly.
4. Global search: progress bar moves; sections reorder; "Has results" persists after restart.
5. Migrate tab: sort buttons persist; stub row shows "Not installed"; right-click copies id.
6. Narrow window (480 px): chip rows scroll; header actions collapse.
7. Keyboard: Ctrl+F, Escape, Enter, Shift+F10 behave as in `p2-ui-parity.md` section 7.

## 11. Open decisions that change tests

- Q-1 (non-favorite title refresh) changes E-BR-10 expected result if chosen.
- D-2 (empty first page as empty state) changes the error path in T-TEST-PAGER-TERMINAL.
- D-3 (narrow favorite update) changes E-BR-02 expectation from "overwrite" to "only four columns change".
