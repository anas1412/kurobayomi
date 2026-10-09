# Area 41 part 2: implementation tasks (anime sources, browse, global search, source filters, migration sources)

Task IDs are `T-41b-NNN`. Views (widgets, layout, rows, toolbars, grids, card rows, migration picker layout) are owned by part 1 (`p1-tasks.md`, IDs `T-41a-NNN`). Where a task here builds on a part-1 view, the view task is listed in Depends and this task covers the model, wiring and behavior only.

Behavior references: `p2-behavior.md` (section numbers), `p2-data-and-config.md`, `p2-rust-design.md` (decisions D-1..D-8, open questions Q-1..Q-5), `p2-edge-cases-and-tests.md` (E-*, RC-*, T-TEST-*), `p2-topic-*.md`.

Priority: P0 = needed for daily reading and watching from one working source. P1 = parity. P2 = polish. P3 = Android-only, replacement or drop stated.

## Sources list

### T-41b-001 — Enabled-source selection model (GetEnabledAnimeSources port)
- Priority: P0
- Effort: S
- Depends: T-41a-043, area:17-source-app-layer, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt
- Acceptance:
  - `enabled_source_rows` (p2-rust-design.md 3.1) keeps a source when `lang ∈ enabled_langs || id == 0`, drops ids in `hidden_anime_catalogues`, sorts by lowercase name only.
  - The last-used source produces one extra row directly after its primary row with `is_last_used_copy = true`.
  - Unit test T-TEST-ENABLED-ROWS passes for the local-source rule (E-GS-05 and E-SRC-08 rows).

### T-41b-002 — Section grouping and order
- Priority: P0
- Effort: S
- Depends: T-41b-001, T-41a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - `group_sources` produces sections in order LastUsed, Pinned, languages (D-1 order: "all" first, then display name), then the empty language last (decision D-1, p2-topic-sources-list.md section 4).
  - The worked example in p2-topic-sources-list.md section 2 yields exactly the listed sections and members.
  - A pinned source appears in Pinned; if it is also last used it appears in LastUsed too (E-SRC-03).

### T-41b-003 — Sources tab model state
- Priority: P0
- Effort: S
- Depends: T-41b-002, T-41a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt
- Acceptance:
  - State has `loading`, `rows` (section headers and items), `dialog: Option<SourceRow>`, and `is_empty`, matching p2-behavior.md section 3.7.
  - Every emission of the enabled-source watch replaces `rows` and clears `loading`.
  - An empty emission yields `is_empty = true` and the message "No source found" (`source_empty_screen`).

### T-41b-004 — Sources tab failure path
- Priority: P0
- Effort: S
- Depends: T-41b-003, T-41a-045
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt
- Acceptance:
  - A failure of the source watch before the first emission clears `loading` and shows an inline row with "Retry" (decision E-SRC-02; Kotlin keeps the spinner forever).
  - A failure after the first emission keeps the last rows and shows the snackbar with text "InternalError: Check crash logs for further information" (`internal_error`) for 4 seconds.
  - "Retry" re-subscribes the watch and clears the error row.

### T-41b-005 — Sources tab header actions and navigation wiring
- Priority: P0
- Effort: S
- Depends: T-41a-001, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt
- Acceptance:
  - "Global search" pushes the global search page with an empty query; "Filter" pushes the visibility page.
  - Row click pushes the browse page for the source with the Popular listing; the Latest button pushes it with the Latest listing (p2-behavior.md section 1).
  - Back from either page returns to the Sources tab with its scroll position kept.

### T-41b-006 — Source options dialog actions and disable rules
- Priority: P1
- Effort: S
- Depends: T-41a-002, T-41b-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt, app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt
- Acceptance:
  - Dialog title is `visual_name()` (`Name (EN)` form, or the name when the language is empty).
  - "Pin"/"Unpin" toggles the pin and closes the dialog; "Disable" toggles the hidden flag and closes the dialog; "Disable" is absent for source id 0.
  - Cancel and Escape close the dialog without writing.

### T-41b-007 — Pin and disable toggle writers
- Priority: P1
- Effort: S
- Depends: T-41a-042, T-41a-043, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSourcePin.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `toggle_pin(id)` and `toggle_hidden(id)` flip membership in `pinned_anime_catalogues` and `hidden_anime_catalogues` (string ids in decimal) with a single writer.
  - Watchers receive the new set; the sources list re-groups within one emission.
  - Unparseable id strings are skipped on read and never rewritten (E-SRC-07).

## Source visibility page

### T-41b-008 — Visibility page data model and order
- Priority: P1
- Effort: M
- Depends: T-41b-001, area:16-source-api-anime, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreenModel.kt, data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt
- Acceptance:
  - Only installed `AnimeHttpSource` instances are listed (local and stubs excluded, E-VIS-04).
  - Language sections: enabled first, then "all" first, then display names (`visibility_language_cmp`, p2-topic-sources-list.md section 5); sources within a language: enabled first, then name.
  - Model state is `Loading`, `Error`, or `Success { groups, enabled_langs, disabled }`; `is_empty` when there are no groups (E-VIS-03).

### T-41b-009 — Visibility page wiring
- Priority: P1
- Effort: S
- Depends: T-41b-008, T-41a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreenModel.kt
- Acceptance:
  - Language switch calls `toggle_language(lang)`; source checkbox calls `toggle_hidden(id)`; rows of disabled languages are not rendered (E-VIS-02).
  - Error state shows the toast-equivalent snackbar with `internal_error` and pops the page (E-VIS-07).
  - Empty state shows "No installed source found" (`source_filter_empty_screen`).

## Browse one source

### T-41b-010 — Browse gating and missing-source page
- Priority: P0
- Effort: S
- Depends: T-41a-008, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/core/util/AnimeSourceUtil.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - The page shows loading until `SourceCatalog::is_initialized()` is true.
  - `get_or_stub(id)` returns a stub for uninstalled ids; the stub page shows "Source not installed: NAME (LANG)" or the id when name and language are empty (E-BR-01).
  - Opening a browse page for any source (installed or stub) writes last-used unless incognito (T-41b-025).

### T-41b-011 — Listing model and navigation sentinels
- Priority: P0
- Effort: S
- Depends: T-41b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt
- Acceptance:
  - `Listing::from_nav_arg` maps the two sentinel strings to Popular and Latest and any other value (including none) to `Search`.
  - `Listing` compares by value; the browse listing restarts only when the value changes, except for an explicit toolbar submit (D-7, E-BR-25).
  - Test T-TEST-LISTING passes, including E-BR-03 and E-BR-26.

### T-41b-012 — SourcePager
- Priority: P0
- Effort: M
- Depends: T-41b-011, area:16-source-api-anime
- Sources: data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt
- Acceptance:
  - Refresh requests page 1; append requests `next_page` only in state Idle; prefetch starts when the last visible index is within 25 items of the end (`pageSize = 25`).
  - Terminal rules match the truth tables in p2-topic-browse-listing.md section 2 (empty first page is the empty state, empty later page is an append error, `hasNextPage` controls `next_page`).
  - Results whose generation is not current are dropped (RC-2). Tests T-TEST-PAGER-TERMINAL pass.

### T-41b-013 — Network-to-library upsert
- Priority: P0
- Effort: M
- Depends: T-41b-012, area:05-data-repos-entries-items, area:07-domain-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - Lookup by `(url, source)` uses `SELECT * FROM animes WHERE url = ? AND source = ? LIMIT 1`; missing rows are inserted with the `Anime.create()` defaults (p2-data-and-config.md 1.4).
  - Existing non-favorite rows are returned with the source title in memory, while the UI shows the stored title (Q-1 default; E-BR-10 test).
  - A page of 25 entries is written in one transaction and returns row ids in input order.

### T-41b-014 — Hide-in-library filter
- Priority: P1
- Effort: S
- Depends: T-41b-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - With `browse_hide_in_anime_library_items = true`, favorites are dropped before they reach the list store.
  - The preference is read once per browse page, as in Kotlin (`BrowseAnimeSourceScreenModel.kt:108`); a live change applies on the next open.
  - When every loaded item is hidden and no pages remain, the page shows "No results found" with the empty-state actions (E-BR-21, PROPOSAL).

### T-41b-015 — Popular, Latest and Filter chip wiring
- Priority: P0
- Effort: S
- Depends: T-41b-011, T-41a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt
- Acceptance:
  - Popular chip always shown; Latest chip shown only when `supports_latest`; Filter chip shown only when the source defines filters (p2-behavior.md 5.7).
  - Popular and Latest chips reset filters to source defaults and switch the listing; the Filter chip opens the sheet (T-41b-030) without changing the listing.
  - Chip selection state follows the listing type (Filter is selected when the listing is Search).

### T-41b-016 — Toolbar search and navigate-up rule
- Priority: P0
- Effort: S
- Depends: T-41b-011, T-41a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt
- Acceptance:
  - Submit runs `search(query)` with the new listing `Search { query, filters }` and restarts the pager (even for the same text).
  - Clearing the field sets `toolbar_query = None` without reloading.
  - Back: clears toolbar text first when `!is_user_query && toolbar_query.is_some()`, otherwise pops (E-BR-05 test).

### T-41b-017 — Display mode and column preferences wiring
- Priority: P0
- Effort: M
- Depends: T-41b-012, T-41a-006, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - `pref_display_mode_catalogue` default CompactGrid; stored strings COMPACT_GRID, COMFORTABLE_GRID, COVER_ONLY_GRID, LIST; unknown values read as CompactGrid (E-BR-22).
  - Column preference 0 gives adaptive 128 px cells; positive values give that many fixed columns; values <= 0 are adaptive (E-BR-23).
  - Changing the mode writes the global preference and the grid re-renders without re-querying the source.

### T-41b-018 — Body states and append-error snackbar wiring
- Priority: P0
- Effort: M
- Depends: T-41b-012, T-41a-005, T-41a-045
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt
- Acceptance:
  - First-page error or empty result shows the empty state with the formatted message and actions: Retry, Open in WebView, Help (or "Local source guide" for the local source) (p2-behavior.md 5.10).
  - Append error shows an indefinite snackbar with the formatted message and "Retry"; Retry reloads the failed page only.
  - Loading shows the spinner row at the top (prepend) and bottom (refresh or append); the full-page spinner shows only when the list is empty.

### T-41b-019 — Long-press favorite: remove confirm and direct add
- Priority: P0
- Effort: M
- Depends: T-41b-020, T-41a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/presentation/browse/BrowseSourceDialogs.kt
- Acceptance:
  - Long press (right-click, Menu key, or Shift+F10) on a favorite opens the remove dialog with title "Are you sure?" and text "You are about to remove "%s" from your library"; Cancel closes; Remove closes and removes.
  - Long press on a non-favorite with no duplicate adds it directly (no confirm), and then offers the category picker only when the default-category rule requires it (T-41b-022).
  - Plain click opens the anime with `fromSource = true`.

### T-41b-020 — change_favorite side effects and narrow write
- Priority: P0
- Effort: M
- Depends: T-41b-013, area:34-images-covers-cache, area:08-domain-library-category-history-updates, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt
- Acceptance:
  - Add sets `favorite = true`, `date_added = now`, sets default episode flags and binds enhanced trackers; Remove sets `favorite = false`, `date_added = 0`, and deletes covers and backgrounds (timestamps set when files were deleted; local source skipped).
  - The database write touches only `favorite`, `date_added`, and the timestamps listed (D-3); a test with a stale snapshot shows the stored title and status unchanged (E-BR-02).
  - A failed write shows the snackbar (E-BR-12) and restores the row in the list.

### T-41b-021 — Duplicate detection and duplicate dialog
- Priority: P1
- Effort: S
- Depends: T-41b-020, T-41a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/anime/DuplicateAnimeDialog.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetDuplicateLibraryAnime.kt
- Acceptance:
  - The duplicate query is favorites with `LOWER(title) = lowercase(title)` and `_id != id`, ordered by `_id` ascending (E-BR-13).
  - The dialog has the rows "Show anime", "Migrate existing entry", "Add anyway", and a Cancel button; each row dismisses first and then runs its action.
  - "Add anyway" calls the add flow (T-41b-022), so the category rule still applies.

### T-41b-022 — Category resolution and category picker
- Priority: P1
- Effort: M
- Depends: T-41b-020, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetAnimeCategories.kt
- Acceptance:
  - Resolution follows p2-topic-browse-listing.md 5.1: default category present → that category; default 0 or no user categories → no category; otherwise the picker (default unset = -1).
  - The system category (id 0) is never offered in the picker; preselection comes from the anime's current categories.
  - Writes run category first, then favorite, in one DB request (RC-4); OK in the picker sets the favorite and the chosen categories.

### T-41b-023 — Browse overflow and display-mode menus
- Priority: P0
- Effort: S
- Depends: T-41b-017, T-41a-007, T-41b-044
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt
- Acceptance:
  - Overflow shows "Open in WebView" for non-local sources (WebView page with the home URL) or "Help" for the local source (`https://aniyomi.org/help/guides/local-anime/`).
  - "Settings" appears only for configurable sources and opens the source preferences page (part 1 T-41a-023).
  - The display-mode menu has three radio items; choosing one writes the preference and closes the menu.

### T-41b-024 — Genre search and text-search entry
- Priority: P1
- Effort: M
- Depends: T-41b-011, T-41b-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - `apply_genre` (p2-topic-source-filters.md section 3) runs on the defaults; a match sets the listing to `Search { query: None }`, otherwise `Search { query: Some(genre) }`; tests T-TEST-FILTER-GENRE pass.
  - The AnimeScreen genre and text entries call the browse page directly (no rendezvous channel, RC-5) when the previous page is a browse page for an HTTP source.
  - The fix for E-BR-04 (only TriState, CheckBox, Select count as matches) is applied and noted in the release.

### T-41b-025 — Last-used write and incognito gating
- Priority: P1
- Effort: S
- Depends: T-41b-010, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeIncognitoState.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt
- Acceptance:
  - Opening a browse page writes `__APP_STATE_last_anime_catalogue_source` unless incognito (global mode or the source's extension package is in `incognito_anime_extensions`).
  - Incognito does not block favorites or pins; only the last-used write is gated.
  - Writes happen for stub sources too (E-BR-08 test).

### T-41b-026 — Browse list virtualization and per-row subscriptions
- Priority: P1
- Effort: M
- Depends: T-41b-012, T-41a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceList.kt
- Acceptance:
  - Pager results append to a `gio::ListStore`; the store is never rebuilt on append (keeps scroll position).
  - Row objects subscribe to their library row only while bound; unbinding cancels the subscription (E-BR-09).
  - A list of 2,000 items scrolls without a visible hitch on the reference machine (manual check, p2-edge-cases-and-tests.md section 10).

### T-41b-027 — Browse dialog state machine and Migrate hand-off
- Priority: P1
- Effort: S
- Depends: T-41b-021, T-41a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt
- Acceptance:
  - Exactly one browse dialog at a time; a second request replaces the first (E-BR-16).
  - "Migrate existing entry" opens the migrate dialog with `new = browse anime`, `old = library duplicate` (part 1 T-41a-035 owns the dialog body).
  - Season selection from the migrate dialog opens the season picker (part 1 T-41a-040) with the same two anime.

### T-41b-028 — Browse listing filter model sync
- Priority: P1
- Effort: S
- Depends: T-41b-030, T-41b-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt
- Acceptance:
  - Sheet edits mutate the shared filter list immediately (Q-5 default), so Cancel keeps them (E-FD-05).
  - The next Filter press or text search uses the edited list; Reset restores source defaults without touching the listing.
  - Equality of two identical filter lists is true; an edited list is unequal to the original (E-FD-07).

## Source filter sheet

### T-41b-029 — Filter data model
- Priority: P1
- Effort: M
- Depends: T-41b-011, area:16-source-api-anime
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilterList.kt
- Acceptance:
  - `Filter` enum covers Header, Separator, Select, Text, CheckBox, TriState, Group, Sort with the fields in p2-data-and-config.md 1.9.
  - `FilterList` derives value equality; no variant compares unequal to itself (D-7).
  - Source defaults are produced by the source trait's `filter_list()` and are fresh per call.

### T-41b-030 — Filter rendering and mutation rules
- Priority: P1
- Effort: L
- Depends: T-41b-029, T-41a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - The pure functions in p2-topic-source-filters.md section 3 (`cycle_tristate`, `tristate_from_int`, `select_sort`, `sort_arrow`, `filter_name`, `apply_genre`) exist and match the test vectors of section 7.
  - Unknown TriState ints map to Ignore and log a warning (D-4, E-BR-28); a Sort with no selection behaves as index 0 ascending (E-BR-27).
  - Each variant renders with the widget in p2-ui-parity.md 3.9 (TriState icons: unchecked, checked, disabled-by-default).

### T-41b-031 — Filter sheet window actions
- Priority: P1
- Effort: S
- Depends: T-41b-030, T-41a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt
- Acceptance:
  - Reset restores source defaults and keeps the window open.
  - Filter applies the search with the current filters and closes the window; Escape closes without reverting edits.
  - The window is modal and transient for the main window; it opens at 480 x 640 (or smaller when the window is smaller).

## Global search

### T-41b-032 — Global search source selection and snapshots
- Priority: P1
- Effort: M
- Depends: T-41b-001, area:17-source-app-layer, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreenModel.kt
- Acceptance:
  - `select_sources` (p2-topic-global-search.md section 2) keeps enabled-language, not-hidden sources; PinnedOnly keeps pinned ones; a non-empty extension filter keeps only that extension's sources.
  - Enabled languages, hidden and pinned sets are snapshotted at screen creation (E-GS-07).
  - The local source is searched only when "other" is an enabled language (E-GS-05 test).

### T-41b-033 — Fan-out search with a shared 5-permit semaphore
- Priority: P1
- Effort: M
- Depends: T-41b-032, T-41b-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt
- Acceptance:
  - Each selected source gets one request for page 1 with its default filters; at most 5 requests are in flight across all global search screens (E-GS-08).
  - A source result is stored as Ok (with library ids via the upsert), or Failed (raw message, may be empty).
  - Sources that already have a result for the same query are skipped (E-GS-02).

### T-41b-034 — Search reuse rules and blank-query no-op
- Priority: P1
- Effort: S
- Depends: T-41b-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt
- Acceptance:
  - A blank or whitespace-only query does nothing and keeps results (E-GS-01).
  - The same query with the same mode does nothing; the same query with a changed mode reuses results and starts only the new sources (E-GS-02, E-GS-12).
  - The query comparison is exact (no trimming, case-sensitive), as in Kotlin.

### T-41b-035 — Global search view wiring
- Priority: P1
- Effort: M
- Depends: T-41a-027, T-41a-028, T-41a-029, T-41a-030, T-41b-033
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/GlobalAnimeSearchScreen.kt, app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt
- Acceptance:
  - Submit runs `search`; the Pinned, All and Has-results chips drive the model; the progress bar is shown when `1 <= progress < total` with fraction `progress / total`.
  - Source header click opens the browse page for that source with the current query; card click or long press opens the anime with `fromSource = true`.
  - The result sections and card rows show the states listed in p2-ui-parity.md 5.2.

### T-41b-036 — Stale-result guard and cancellation on page drop
- Priority: P1
- Effort: S
- Depends: T-41b-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt
- Acceptance:
  - Results with a superseded generation are dropped (E-GS-03 test; decision D-5).
  - Dropping the page cancels its outstanding requests before they take a permit (RC-6).
  - The test T-TEST-GS-STALE uses two queries and a delayed first response.

### T-41b-037 — Single-result auto-open and extension entry point
- Priority: P2
- Effort: S
- Depends: T-41b-035, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - With a query and an extension filter, a single selected source that returns exactly one anime replaces the page with that anime; zero, several, or failed results show the list (p2-topic-global-search.md section 7).
  - A `kurobayomi://animesearch?query=<text>&filter=<package name>` URL (p2-rust-design.md section 8) opens global search with those arguments.
  - Test T-TEST-GS-SINGLE covers each row of the decision table.

## Migration sources tab

### T-41b-038 — Migration source query
- Priority: P1
- Effort: S
- Depends: T-41b-013, area:09-domain-source-track-download-release-misc
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt, data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - Query `SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source` maps each source id to its installed or stub source; `is_stub` is set for stubs.
  - The local source (id 0) is excluded (E-MS-08).
  - Sources with no favorites are not listed.

### T-41b-039 — Migration sort and preference persistence
- Priority: P1
- Effort: S
- Depends: T-41b-038, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceScreenModel.kt
- Acceptance:
  - `sort_migrate_rows` implements alphabetical (lowercase, collated) and total modes, ascending and descending, with stubs first in ascending and last in descending (E-MS-03).
  - Ties are ordered by source id ascending in both directions (E-MS-04, deterministic deviation from Kotlin).
  - Toggling mode or direction writes both `pref_migration_sorting` and `pref_migration_direction` (E-MS-07).

### T-41b-040 — Migration tab wiring
- Priority: P1
- Effort: S
- Depends: T-41a-036, T-41a-047, T-41a-048, T-41b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt, app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt
- Acceptance:
  - Header help action opens `https://aniyomi.org/help/guides/source-migration/`.
  - Row click opens `MigrateAnimeScreen(source.id)` (part 1 T-41a-037); the sort buttons call the toggles in T-41b-039.
  - Long press copies the source id; the empty state shows "Your library is empty" (E-MS-01); a failed load shows Retry (E-MS-02).

## Shared utilities for this scope

### T-41b-041 — Section titles and language order helpers
- Priority: P0
- Effort: S
- Depends: T-41a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - `section_title` returns "Last used", "Pinned", "Multi" (all), "Other" (other), the system language name for "", and the display name otherwise (p2-topic-sources-list.md section 6).
  - `zh-CN` and `zh-TW` map to Hans and Hant before display (E-SRC-04 table).
  - `language_rank_cmp` (D-1) is covered by T-TEST-LOCALE-DISPLAY.

### T-41b-042 — SourceError user messages
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - `SourceError::NoEpisodes` → "No results found"; `NotInstalled` → "Source not found"; `Http(code)` → "HTTP %d, check website in WebView" with the code; `UnknownHost` → "Couldn't reach %s"; `Offline` → "No Internet connection"; `Other` → message or class name.
  - Global search error items use the raw message (may be empty); browse uses the formatted message (p2-behavior.md 9.2, E-GS-06).
  - Test T-TEST-ERROR-TEXT covers each variant.

### T-41b-043 — Preference keys and typed accessors for this scope
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Every key in p2-data-and-config.md section 3 has a typed accessor with the listed default (for example `source_languages` default `{all, en, system}`, `default_anime_category` default -1).
  - App-state keys carry the `__APP_STATE_` prefix; user keys do not.
  - Unknown enum strings fall back to the default (E-MS-07 and display mode).

### T-41b-044 — External links and WebView handoff
- Priority: P0
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview, T-41a-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt
- Acceptance:
  - External URLs open with `gtk::UriLauncher` (no libadwaita dependency).
  - "Open in WebView" opens the WebView page (area 36) with the source home URL and the source name; no page is opened for a missing home URL.
  - The help URLs match p2-data-and-config.md section 5 exactly.

### T-41b-045 — Keyboard and pointer map for this scope
- Priority: P1
- Effort: M
- Depends: T-41a-044, T-41b-016, T-41b-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt
- Acceptance:
  - Ctrl+F focuses search on browse and global search; Escape follows the navigate-up rule; Enter submits search.
  - Right-click, Menu key, and Shift+F10 open the long-press menu on cells and rows.
  - Every entry in p2-ui-parity.md section 7 works with the keyboard alone.

### T-41b-046 — Narrow-window behavior for this scope
- Priority: P2
- Effort: S
- Depends: T-41a-050
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt, app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt
- Acceptance:
  - Below 480 px the header actions collapse into an overflow menu; chip rows scroll horizontally with no page-level horizontal scroll.
  - Sources rows never scroll horizontally; names ellipsize first.
  - Checked in the manual list of p2-edge-cases-and-tests.md section 10 item 6.

## Tests and fixtures

### T-41b-047 — Test fixtures
- Priority: P0
- Effort: S
- Depends: T-41b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt
- Acceptance:
  - `FakeSource`, `FakeCatalog`, `InMemoryAnimeRepo`, `RecordingCover`, `RecordingBackground`, `RecordingTrackBinder`, `RecordingEpisodeFlags`, `FixedClock`, `StubLocaleText`, `Collate` fakes exist as described in p2-edge-cases-and-tests.md section 8.
  - `FakeSource` can script pages, `has_next_page`, empty pages, and errors per call.
  - The fixtures live under `tests/browse_anime/` (no network).

### T-41b-048 — Unit tests: sources list and visibility order
- Priority: P0
- Effort: S
- Depends: T-41b-047, T-41b-002, T-41b-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt
- Acceptance:
  - T-TEST-SECTIONS, T-TEST-LAST-USED-DUP, T-TEST-ENABLED-ROWS, and T-TEST-VIS-ORDER pass.
  - E-SRC-01 through E-SRC-04, E-SRC-09, E-SRC-10, E-VIS-04 through E-VIS-06 are covered.

### T-41b-049 — Unit tests: pager and listing
- Priority: P0
- Effort: S
- Depends: T-41b-047, T-41b-012, T-41b-011
- Sources: data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt
- Acceptance:
  - T-TEST-PAGER-TERMINAL covers every row of both truth tables in p2-topic-browse-listing.md section 2.
  - T-TEST-LISTING covers E-BR-03, E-BR-25, and E-BR-26.
  - E-BR-07, E-BR-18, E-BR-19, E-BR-20, RC-2 pass.

### T-41b-050 — Unit tests: filters
- Priority: P1
- Effort: S
- Depends: T-41b-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt
- Acceptance:
  - T-TEST-FILTER-TRISTATE, T-TEST-FILTER-SORT, T-TEST-FILTER-GENRE, and T-TEST-FILTER-EQUALITY pass against the vectors of p2-topic-source-filters.md section 7.
  - E-FD-01 through E-FD-03 and E-FD-06 are covered.

### T-41b-051 — Unit tests: global search
- Priority: P1
- Effort: S
- Depends: T-41b-047, T-41b-033, T-41b-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt
- Acceptance:
  - T-TEST-GS-SORT, T-TEST-GS-REUSE, T-TEST-GS-STALE, T-TEST-GS-SEMAPHORE, T-TEST-GS-SELECT, and T-TEST-GS-VISIBILITY pass.
  - E-GS-01 through E-GS-05, E-GS-08 through E-GS-10, E-GS-12 through E-GS-14 are covered.

### T-41b-052 — Unit tests: migration sort and error text
- Priority: P1
- Effort: S
- Depends: T-41b-039, T-41b-042
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt
- Acceptance:
  - E-MS-03, E-MS-04, E-MS-06, and E-MS-08 pass.
  - T-TEST-ERROR-TEXT and T-TEST-LOCALE-DISPLAY pass.

### T-41b-053 — Integration tests: favorites, duplicates, categories
- Priority: P0
- Effort: M
- Depends: T-41b-047, T-41b-020, T-41b-021, T-41b-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt
- Acceptance:
  - T-TEST-FAVORITE-WRITE, T-TEST-DUPLICATE, and T-TEST-CATEGORY-RESOLUTION pass against the in-memory repository.
  - E-BR-02, E-BR-12, E-BR-13, E-BR-14, E-BR-15, E-BR-16, E-BR-08 are covered.
  - RC-3 and RC-4 are covered with a delayed fake write.

### T-41b-054 — Manual GTK checklist for this scope
- Priority: P1
- Effort: S
- Depends: T-41b-045, T-41b-046, T-41b-035, T-41b-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt
- Acceptance:
  - The checklist in p2-edge-cases-and-tests.md section 10 is run in light and dark GTK themes and each item is recorded as pass or fail.
  - E-BR-05, E-BR-11, E-FD-05, E-MS-09, E-SRC-05 are checked by hand.
  - Any fail opens a follow-up task before the area is marked done.

## Drops and replacements (P3)

### T-41b-055 — Drop: haptic feedback on browse long press
- Priority: P3
- Effort: S
- Depends: T-41b-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt
- Acceptance:
  - No haptic call exists in the Kurobayomi source tree for this scope.
  - The user loss (tactile confirmation on long press) is recorded in p2-behavior.md section 12.
  - The visual result (dialog or added badge) is unchanged.

### T-41b-056 — Drop: Assist content URL
- Priority: P3
- Effort: S
- Depends: T-41b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt
- Acceptance:
  - No assist-content hook is implemented; the home URL is still available through "Open in WebView".
  - The drop is recorded in p2-behavior.md section 12 with the loss (Android assist only).

### T-41b-057 — Drop: Compose item animations
- Priority: P3
- Effort: S
- Depends: T-41b-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt
- Acceptance:
  - No row movement animation is implemented; results reorder immediately as they arrive.
  - The drop is recorded with the loss in p2-behavior.md section 12.

### T-41b-058 — Replace: Voyager navigation with the GTK page stack
- Priority: P3
- Effort: S
- Depends: T-41b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt, src/ui.rs
- Acceptance:
  - All navigation in this scope uses `ui::push` and `ui::pop` (mockup src/ui.rs), with no Voyager types.
  - Back and Escape behavior matches section 4 of p2-topic-browse-listing.md.

### T-41b-059 — Replace: Injekt with an explicit services struct
- Priority: P3
- Effort: S
- Depends: T-41b-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt
- Acceptance:
  - Every collaborator in p2-inventory.md's constructor lists is passed explicitly through an `AppServices` value.
  - No global service locator is used by the browse or global search modules.
