# Area 40 (part 2): implementation tasks

Format per `plans/_tools/tasks.py`. Scope: the 12 part-2 files (`p2-behavior.md` section 0). Part 1 presentation tasks are in `p1-tasks.md` (T-40a-*); this list refers to them by area only, so the file lints standalone.

Source paths are repo-relative to `/home/blackbox/aniyomi-src`, except the three plan-document paths, which are relative to `/home/blackbox/kurobayomi`.

Coverage map (file -> tasks): MigrateMangaSourceScreenModel.kt -> 072, 074; MigrateMangaSourceTab.kt -> 073, 074; MangaSourcesFilterScreen.kt -> 022; MangaSourcesFilterScreenModel.kt -> 020, 022; MangaSourcesScreenModel.kt -> 010, 011, 012, 013; MangaSourcesTab.kt -> 012, 014; BrowseMangaSourceScreen.kt -> 036, 038, 040, 044, 046, 047, 080, 081; BrowseMangaSourceScreenModel.kt -> 030, 032, 034, 035, 037, 039, 041, 042, 045; SourceFilterMangaDialog.kt -> 050, 051, 052, 053; GlobalMangaSearchScreen.kt -> 064, 065, 066; GlobalMangaSearchScreenModel.kt -> 062; MangaSearchScreenModel.kt -> 060, 061, 063.

---

## Foundation

### T-40b-001 — Source value type and pin codec
- Priority: P0
- Effort: S
- Depends: area:17-source-app-layer, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt
- Acceptance:
  - `SourceRow` has the fields of `Source.kt:3-14` and `visual_name`/`key` match `Source.kt:16-27` for fixtures (lang empty, lang "en", isUsedLast true).
  - `Pins` values are `unpinned=0`, `pinned=3` (`Pin.kt:26-28`); `Pin.Pinned=1`, `Pin.Actual=2`.
  - `Pins::xor(Actual)` on `pinned()` gives `1`, on `UNPINNED` gives `2` (Kotlin XOR quirk preserved, behavior 1.4).

### T-40b-002 — Primary collator and case-insensitive compare
- Priority: P0
- Effort: S
- Depends: T-40b-001
- Sources: core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `primary_cmp("é", "e") == Equal` and `primary_cmp("A", "a") == Equal` (PRIMARY strength).
  - `primary_cmp` and `case_insensitive_cmp` are total orders (property test, E-SL-06 note).
  - Fallback path (collator construction failure) compares `to_lowercase()` and returns the same ordering for ASCII fixtures.

### T-40b-003 — Registry readiness gate for source screens
- Priority: P0
- Effort: S
- Depends: area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/core/util/MangaSourceUtil.kt
- Acceptance:
  - Sources page, source filter, browse and global search show a spinner until `SourceRegistry::is_initialized()` is true (E-SL-08, D-SS-01).
  - An empty catalogue before init never renders "No source found".
  - Initialisation that completes later updates the open page without reopening it.

### T-40b-004 — Generation-guarded event loop and task cancellation
- Priority: P0
- Effort: M
- Depends: area:36-ui-shell-navigation-security-webview, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/presentation/util/Navigator.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt, plans/40-ui-browse-manga/p2-rust-design.md
- Acceptance:
  - Each page owns one `async_channel` pair; the GTK loop `spawn_future_local` applies events on the main thread only.
  - Events whose `generation` differs from the current one are dropped (paging and global search).
  - Popping a page aborts its loader and `JoinSet`; no task outlives the page (checked by a test that counts live tasks).

### T-40b-005 — Config keys for the browse state
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, plans/40-ui-browse-manga/p2-data-and-config.md
- Acceptance:
  - Every key in `p2-data-and-config.md` section 5 round-trips through `browse.toml` with the listed default (`last_catalogue_source=-1`, `default_category=-1`, `has_filters_toggle_state=false`, `browse_hide_in_library_items=false`, columns `0`).
  - Unknown `display_mode` deserializes to `COMPACT_GRID`; unknown migrate mode/direction to `ALPHABETICAL`/`ASCENDING`.
  - Writes are atomic (temp file then rename) and unknown TOML keys survive a save.

### T-40b-015 — Live preference updates for source lists
- Priority: P1
- Effort: S
- Depends: T-40b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesFilterScreenModel.kt
- Acceptance:
  - A change to `hidden_catalogues`, `pinned_catalogues`, `source_languages` or `last_catalogue_source` updates an open Sources page or Sources filter page without reopening it (B§1.1).
  - Global search and browse keep the snapshot taken at creation (B§1.1, documented).

---

## Sources list

### T-40b-010 — Source list grouping and ordering
- Priority: P0
- Effort: M
- Depends: T-40b-001, T-40a-033
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt
- Acceptance:
  - Fixture `sources_mixed` yields headers in order: last used, pinned, then language codes by code point, `""` last (E-SL-05).
  - A pinned and last-used source appears twice, with the copy under last used (E-SL-04).
  - Hidden language, disabled source, and last-used-but-disabled behave as E-SL-01 and E-SL-02.
  - Latest button only when `supports_latest` (E-SL-07).

### T-40b-011 — Pin and disable toggles
- Priority: P0
- Effort: S
- Depends: T-40b-005
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSourcePin.kt, app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt
- Acceptance:
  - Pin adds or removes the id string in `pinned_catalogues`; disable adds or removes it in `hidden_catalogues` (B§2.5).
  - Two rapid toggles of different sources both persist (E-SL-09): writes are serialized.
  - Disabling the local source is not offered (P1 options rule).

### T-40b-012 — Options dialog actions and failure toast
- Priority: P0
- Effort: S
- Depends: T-40b-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt
- Acceptance:
  - Options dialog has "Pin"/"Unpin" (by `Pin.Pinned`) and "Disable" (hidden for id 0), each closing the dialog (B§2.5, S-B).
  - A failed source-list subscription shows toast `internal_error` and keeps the last list (E-SL-10).

### T-40b-013 — Drop data-saver state from the Sources model
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - No `dataSaverEnabled`, `isExcludedFromDataSaver` or `dataSaver*` key is read by Kurobayomi (B§15 drop reason: third-party image proxies).
  - Backup import keeps the `data_saver*` keys as passthrough data (p2-data-and-config section 9).

### T-40b-014 — Sources tab actions
- Priority: P0
- Effort: S
- Depends: T-40b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesTab.kt
- Acceptance:
  - "Global search" action (B§2.6) opens global search with an empty query and no extension filter.
  - "Filter" action opens the Sources filter page.
  - Both actions appear in the Browse page header, not in an overflow menu.

---

## Sources filter

### T-40b-020 — Languages with sources grouping
- Priority: P1
- Effort: M
- Depends: T-40b-001, T-40b-002
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - Only online (HTTP) sources are listed (E-SF-04).
  - Enabled languages first, then `LocaleHelper.comparator` order with "all" first among them; within a language enabled sources first, then name (B§3.2).
  - Two codes with equal display names are both kept (E-SF-03, D-SF-01).

### T-40b-021 — Language and source toggles
- Priority: P1
- Effort: S
- Depends: T-40b-020
- Sources: app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt, app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSource.kt
- Acceptance:
  - Language toggle adds or removes the code in `source_languages` (B§3.3).
  - Source toggle with no argument enables a disabled source and disables an enabled one (E-SF-06).
  - Turning a language off keeps per-source disabled state (E-SF-01).

### T-40b-022 — Sources filter state machine
- Priority: P1
- Effort: S
- Depends: T-40b-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesFilterScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesFilterScreen.kt
- Acceptance:
  - States Loading, Error, Success exactly as B§3.1.
  - Error shows toast `internal_error` and pops the page (E-SF-05).
  - Empty success shows `source_filter_empty_screen`.

---

## Browse one source

### T-40b-030 — Listing model and navigation argument
- Priority: P0
- Effort: S
- Depends: T-40b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetRemoteManga.kt
- Acceptance:
  - `Listing` has the three variants of p2-data-and-config 1.6; the navigation argument is an enum (B-EC-29), no sentinel string.
  - Search listing opened from a query keeps the query in the toolbar (B-EC-02).
  - Blank query sends `""` (B-EC-09); repeated search with the same query restarts (B-EC-08).
  - A stub source never starts a listing (B-EC-07).

### T-40b-031 — Source filter tree model
- Priority: P0
- Effort: M
- Depends: none
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/FilterList.kt
- Acceptance:
  - `FilterNode` covers Header, Separator, CheckBox, TriState, Text, Select, Sort, Group with the defaults of p2-data-and-config 1.7.
  - No code compares two filter trees for identity (generation counters only, B§4.4.1).
  - `source_defaults` is a fresh clone on every reset or search (B§4.6).

### T-40b-032 — Paged remote listing and prefetch
- Priority: P0
- Effort: M
- Depends: T-40b-004, T-40b-030
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt, data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt
- Acceptance:
  - Page size 25 and initial load 75 (B§4.3); prefetch when last visible index + 25 >= total.
  - Pages are demand-driven; no eager loading (R§3.11).
  - Empty page 1 shows `no_results_found` (B-EC-03); empty later page is an appended error (B-EC-04); `hasNextPage=false` ends the list (B-EC-05); source exception is an error state (B-EC-06).
  - Generation change drops late pages (E-PG-03).

### T-40b-033 — Map remote manga to local rows
- Priority: P0
- Effort: M
- Depends: area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/NetworkToLocalManga.kt, data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt
- Acceptance:
  - Unknown (url, source) inserts a non-favorite row and returns its id (B§4.4).
  - Existing non-favorite row returns with the remote title; existing favorite returns unchanged.
  - Each browsed row observes its favorite flag live (no page reload when the manga is added elsewhere).

### T-40b-034 — Hide-in-library filter
- Priority: P1
- Effort: S
- Depends: T-40b-033, T-40b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - With `browse_hide_in_library_items = true`, favorites are not shown in browse pages.
  - The flag is read at page creation (B§1.1); the behavior for a row favorited while on screen matches B-EC-15.

### T-40b-035 — Grid column preference
- Priority: P0
- Effort: S
- Depends: T-40b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Landscape (width >= height) reads the landscape key, else portrait (B§4.5).
  - `0` gives adaptive 128 px cells; `n > 0` gives exactly `n` columns (B-EC-18).

### T-40b-036 — Browse page transitions and navigate-up rule
- Priority: P0
- Effort: M
- Depends: T-40b-030, T-40b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt
- Acceptance:
  - `search(query, filters)` follows B§4.6.1 (defaults used when starting from Popular/Latest).
  - Popular and Latest chips call reset-then-set-listing (B§4.6).
  - Back with an active search and toolbar text clears the text first (B-EC-10); local source help goes to `LocalMangaSource.HELP_URL` (B-EC-14).

### T-40b-037 — Genre search
- Priority: P1
- Effort: M
- Depends: T-40b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Genre found in a group sets TriState to include (raw 1) or CheckBox to true, then searches with no query (B§4.7).
  - Not found: text search with the genre name (B-EC-11).
  - Select match index refers to the full `values` list (B-EC-22, D-BR-03).

### T-40b-038 — Direct method instead of a global query channel
- Priority: P1
- Effort: S
- Depends: T-40b-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - The companion `Channel<SearchType>` is replaced by a method on the open browse page (D-CH-01).
  - Genre and text search from a manga page reach the open browse page without suspending the caller.

### T-40b-039 — Last-used source write
- Priority: P0
- Effort: S
- Depends: T-40b-005, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaIncognitoState.kt
- Acceptance:
  - Opening a source writes `last_catalogue_source` unless incognito applies to that source (B-EC-17).
  - The write happens once per page open, not on each listing change.

### T-40b-040 — Stub source page and loading gate
- Priority: P0
- Effort: S
- Depends: T-40b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt
- Acceptance:
  - Stub source shows only the missing-source page with back (B§4.11, part 1 S-stub).
  - Registry not initialised shows a spinner (B-EC-28).

### T-40b-041 — Favorite toggle with partial writes
- Priority: P0
- Effort: M
- Depends: T-40b-033, area:08-domain-library-category-history-updates, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt, data/src/main/sqldelight/data/mangas.sq, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt
- Acceptance:
  - Add writes favorite true, `date_added` in ms; remove writes favorite false and `date_added` 0 (B§4.8.1).
  - Add writes only the columns favorite and date_added (partial update); chapter flags written by the default-flags step survive (B-EC-12).
  - Remove clears the cover cache and sets `cover_last_modified` when at least one file was deleted (B-EC-13).
  - A failed write shows toast `internal_error` and restores the previous favorite flag in the view (E-FAV-03, D-FAV-02).

### T-40b-042 — Add-to-library decision and category writes
- Priority: P0
- Effort: M
- Depends: T-40b-041, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetMangaCategories.kt, data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt
- Acceptance:
  - `add_favorite_plan` matches B§4.8.2 for default -1 (chooser when user categories exist), 0 (no category), and an existing category id (that id).
  - Missing default category id with categories present shows the chooser (B-EC-23).
  - Category links are written before the favorite row, in one transaction per write (E-FAV-01, D-FAV-01).
  - A category write failure is logged and does not block the favorite (E-FAV-02).

### T-40b-043 — Duplicate lookup
- Priority: P1
- Effort: S
- Depends: T-40b-033
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetDuplicateLibraryManga.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
  - Candidates are favorites with id not equal to the manga (`mangas.sq:117-122`).
  - Title equality uses Unicode `to_lowercase()` on both sides (B-EC-25, D-DUP-01).

### T-40b-044 — Long-click decision on a browse row
- Priority: P0
- Effort: S
- Depends: T-40b-043, T-40b-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt
- Acceptance:
  - Favorite row -> remove confirm; non-favorite with duplicate -> duplicate dialog; otherwise add (B§4.8.4, table in p2-ui-parity section 3).
  - Favorite with a duplicate shows the remove dialog only (B-EC-26).
  - A second long press while a dialog is open replaces the dialog (B-EC-27).

### T-40b-045 — Category chooser wiring
- Priority: P1
- Effort: M
- Depends: T-40b-042, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt
- Acceptance:
  - Checkboxes list user categories only (system id 0 excluded) with the preselected links checked (B§4.8.3).
  - Confirm favorites the manga, then sets the checked links; the exclude output is ignored (E-FAV-06).
  - "Edit categories" opens the Categories page on the manga section.

### T-40b-046 — Duplicate dialog and migrate hand-off
- Priority: P1
- Effort: S
- Depends: T-40b-044, T-40a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt, app/src/main/java/eu/kanade/presentation/entries/manga/DuplicateMangaDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - Confirm adds the manga anyway; "Open" opens the duplicate's page; "Migrate" opens the migrate dialog with old = duplicate, new = browsed manga (B§4.8.4).

### T-40b-047 — Remove-from-library confirm
- Priority: P0
- Effort: S
- Depends: T-40b-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt
- Acceptance:
  - Dialog title "Are you sure?" and text "You are about to remove "%s" from your library" with the manga title (keys `are_you_sure`, `remove_manga`).
  - Confirm runs the favorite-remove write; cancel changes nothing.

---

## Filter sheet

### T-40b-050 — Filter sheet container and buttons
- Priority: P0
- Effort: M
- Depends: T-40b-031, T-40b-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt
- Acceptance:
  - Reset replaces state with defaults, does not search, keeps the sheet open (E-FL-06).
  - Filter runs the search with current values and closes the sheet (E-FL-07).
  - Header and Separator render as heading and divider; Group expands recursively (E-FL-01, E-FL-02).

### T-40b-051 — TriState mapping and cycle
- Priority: P0
- Effort: S
- Depends: T-40b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt, core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt
- Acceptance:
  - Cycle Disabled -> EnabledIs -> EnabledNot -> Disabled (E-FL-08), raw 0 -> 1 -> 2 -> 0.
  - Raw 3 shows as Disabled and logs a warning (B-EC-20, D-BR-02).

### T-40b-052 — Sort filter semantics
- Priority: P1
- Effort: S
- Depends: T-40b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Click on selected item flips direction; click on another keeps direction (E-FL-03, E-FL-04).
  - Null selection: first click selects that index ascending, no crash (B-EC-19, D-BR-01).

### T-40b-053 — Select, Text and CheckBox widgets
- Priority: P0
- Effort: S
- Depends: T-40b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Select shows `values[state]`; an out-of-range state shows index 0 (B-EC-21).
  - Text updates state per keystroke and does not search (E-FL-05).
  - CheckBox toggles state and reports the change.

---

## Global search

### T-40b-060 — Global search engine
- Priority: P1
- Effort: L
- Depends: T-40b-004, T-40b-001, T-40b-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt
- Acceptance:
  - Blank query is a no-op; same query and same filter is a no-op (GS-EC-01, GS-EC-02).
  - Filter change with same query fetches only newly selected sources (GS-EC-03).
  - Each source fetches page 1 only, with default filters, at most 5 in flight (R§5).
  - A new search aborts old requests; late results are dropped (GS-EC-06).
  - Results are applied by one writer on the main thread (GS-EC-05, D-GS-02); failures become per-source Error rows (GS-EC-07, GS-EC-08).

### T-40b-061 — Result ordering without a comparator-keyed map
- Priority: P1
- Effort: M
- Depends: T-40b-060, T-40b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt
- Acceptance:
  - Order: non-empty success first, then pinned, then `"name (lang)"` lowercased (B§9.5).
  - Two sources with equal keys are both kept (GS-EC-04, D-GS-01).
  - Order is recomputed on every applied result; stable sort.

### T-40b-062 — Source filter, pinned default and extension filter
- Priority: P1
- Effort: M
- Depends: T-40b-060, T-40b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/GlobalMangaSearchScreenModel.kt
- Acceptance:
  - Default `PinnedOnly`; zero pinned sources gives an empty selection, no fallback (GS-EC-10).
  - Extension filter forces `All` and selects that extension's enabled sources (GS-EC-11, GS-EC-12).
  - Chip change calls search (B§9.7); part 1 text is corrected (T-40b-087).

### T-40b-063 — Has-results toggle
- Priority: P1
- Effort: S
- Depends: T-40b-005, T-40b-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Toggle persists `has_filters_toggle_state` and filters to non-empty successes only (GS-EC-09).
  - The same key is used by migrate search, so the state is shared.

### T-40b-064 — Single-result auto-open
- Priority: P1
- Effort: S
- Depends: T-40b-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/GlobalMangaSearchScreen.kt
- Acceptance:
  - Query plus extension filter plus exactly one source: loading screen until the result; one manga opens the manga page replacing the search (GS-EC-13).
  - Zero, several, or error results show the results screen (GS-EC-14).

### T-40b-065 — Entry points and deep links
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview, T-40b-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/GlobalMangaSearchScreen.kt
- Acceptance:
  - Intent action `eu.kanade.tachiyomi.SEARCH` with extras `query` and `filter` opens global search with the extension filter (GS-EC-11).
  - Deep link with no result replaces the page with global search for the query (GS-EC-16).
  - A search row tap with unsubmitted text opens browse with that text (GS-EC-15, documented).

### T-40b-066 — Global search wrapper gate
- Priority: P1
- Effort: S
- Depends: T-40b-003, T-40b-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/GlobalMangaSearchScreen.kt
- Acceptance:
  - The page shows a loading state until the registry is initialised (B§7 step 1).
  - Results per source keep the card row, header and arrow actions of part 1 (P1§10).

---

## Migrate search subclass

### T-40b-067 — Migrate search origin-first and null-safe load
- Priority: P1
- Effort: S
- Depends: T-40b-061, T-40a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaSearchScreenModel.kt
- Acceptance:
  - Origin source sorts first, then the base order (E-MG-01, D-MG-01).
  - Missing manga id shows an error state, not a panic (E-MG-02).
  - Empty title does not search (E-MG-03).

---

## Migrate source picker

### T-40b-070 — Favorite-count query and source list
- Priority: P1
- Effort: M
- Depends: T-40b-001, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt, data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
  - Query is `SELECT source, count(*) ... WHERE favorite = 1 GROUP BY source ORDER BY source` (E-MS-04).
  - Local source (id 0) is excluded (E-MS-02); empty library shows the empty state (E-MS-01).
  - Stub sources are flagged `is_stub` from the registry.

### T-40b-071 — Migrate sort comparator
- Priority: P1
- Effort: S
- Depends: T-40b-002, T-40b-070
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt
- Acceptance:
  - Alphabetical uses primary collation of lowercased names; total mode uses count (B§11.2).
  - Stubs first in ascending and last in descending (E-MS-03).
  - Ties keep the input order (stable sort), in both directions (E-MS-04).

### T-40b-072 — Sort mode and direction persistence
- Priority: P1
- Effort: S
- Depends: T-40b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt
- Acceptance:
  - Toggles read the config store, not the cached state, so fast double taps apply in order (E-MS-05, R-04).
  - Values persist as `ALPHABETICAL`/`TOTAL` and `ASCENDING`/`DESCENDING`.

### T-40b-073 — Migration help link
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceTab.kt
- Acceptance:
  - Help action opens `https://aniyomi.org/help/guides/source-migration/` in the default browser (`gtk::UriLauncher`).
  - Tooltip text is "Source migration guide".

### T-40b-074 — Migrate picker failure toast
- Priority: P1
- Effort: S
- Depends: T-40b-070
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceTab.kt
- Acceptance:
  - A failed favorite-count subscription shows toast `internal_error` (E-MS-06, D-MG-02), not silently.
  - The error is logged at error level.

---

## Drops and replacements

### T-40b-080 — Drop Android assist-content URL
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt
- Acceptance:
  - No assist-content hook exists in Kurobayomi. Reason: Android assist API only (B§15). Users lose the Android assistant integration; nothing else changes.

### T-40b-081 — Drop haptic feedback on long press
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt
- Acceptance:
  - Long press has no haptic call. Reason: no portable Linux haptic API in GTK (B§15). Users lose the vibration cue only.

### T-40b-082 — Drop data-saver rows in the Sources UI
- Priority: P3
- Effort: S
- Depends: T-40b-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesTab.kt
- Acceptance:
  - Options dialog has no data-saver row (`onClickToggleDataSaver` absent).
  - Reason recorded: the feature routes images through third-party proxies (B§15). Users lose exclusion toggles for proxy data saver.

### T-40b-083 — Replace the Voyager navigator with GTK stack navigation
- Priority: P3
- Effort: M
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/GlobalMangaSearchScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesTab.kt
- Acceptance:
  - push, pop and replace of the browse, global search and migrate pages map to the existing `gtk::Stack` helpers (`src/screens.rs`, `src/ui.rs`).
  - Back behavior follows B§4.6.2 at every level.
  - Reason: Voyager is Compose-only (B§15, R§4).

---

## Tests and verification

### T-40b-084 — Unit tests for the pure models
- Priority: P1
- Effort: M
- Depends: T-40b-010, T-40b-020, T-40b-030, T-40b-032, T-40b-042, T-40b-061, T-40b-071
- Sources: plans/40-ui-browse-manga/p2-edge-cases-and-tests.md, plans/40-ui-browse-manga/p2-rust-design.md
- Acceptance:
  - Every unit test in p2-edge-cases-and-tests section 7.1 exists and passes.
  - Property tests (`proptest`) cover total order of `group_key_cmp`, `primary_cmp`, and that `sort_items` is idempotent and never changes the item count.
  - No test needs a display, a tokio runtime, or the network.

### T-40b-085 — Integration tests with fakes and in-memory SQLite
- Priority: P1
- Effort: M
- Depends: T-40b-084, T-40b-060, T-40b-041
- Sources: plans/40-ui-browse-manga/p2-edge-cases-and-tests.md
- Acceptance:
  - Every integration test in section 7.2 exists and passes.
  - The favorite sequence test records the order links, flags, favorite, trackers (D-FAV-01).
  - The migrate counts test runs against the `mangas.sq` schema in memory.

### T-40b-086 — Manual acceptance script for the browse flows
- Priority: P2
- Effort: S
- Depends: T-40b-010, T-40b-060, T-40b-044
- Sources: plans/40-ui-browse-manga/p2-edge-cases-and-tests.md, plans/40-ui-browse-manga/p2-ui-parity.md
- Acceptance:
  - The twelve checks of p2-edge-cases-and-tests section 7.3 are run on a real desktop build and each is recorded pass or fail with the build id.

### T-40b-087 — Correct the part 1 chip statement
- Priority: P2
- Effort: S
- Depends: T-40b-062, T-40a-010
- Sources: plans/40-ui-browse-manga/p1-ui-parity.md, plans/40-ui-browse-manga/p2-ui-parity.md
- Acceptance:
  - The statement in part 1 section 10.1 that a chip change does not rerun the query is changed to match B§9.7 (the chip calls search and fetches newly selected sources).
  - Part 1 section 10.4 is updated to say the migrate origin-first order is only implemented after T-40b-067.
