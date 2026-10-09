# Area 41 (part 1): implementation tasks

Task format is checked by `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint`. Every task names the Kotlin sources it implements (repo-relative under `/home/blackbox/aniyomi-src`) and the behavior sections in p1-behavior.md, p1-ui-parity.md and p1-rust-design.md it realizes in its acceptance text. Owners of the dependency areas are listed in p1-inventory.md (cross-area dependencies).

Scope coverage: every one of the 36 files in p1-inventory.md maps to at least one task (see the coverage table in the inventory, column "Tasks"). The P3 drop tasks (T-41a-024, T-41a-025, T-41a-046) state the replacement or the drop decision.

### T-41a-001 — Sources list screen (grouped rows, Latest and pin buttons)
- Priority: P0
- Effort: M
- Depends: T-41a-003, T-41a-004, T-41a-043, area:17-source-app-layer, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt
- Acceptance:
  - Rows are grouped under language headers in the order the model provides; header text is `LocaleHelper.getSourceDisplayName(lang)` (p1-behavior.md section 0).
  - A `Latest` flat button appears only when `supportsLatest`; activating it opens the Latest listing; activating the row opens the Popular listing (behavior section 6).
  - Pin button shows the filled pushpin when `Pin.Pinned` bit is set and the outline dimmed to 78 % otherwise; click toggles pin without leaving the page.
  - Empty model shows "No source found"; loading shows a spinner (behavior section 6, ui-parity S1).
  - Mockup gaps G1 to G3 (src/entry.rs:283-309) are closed: no placeholder names, no hard-coded sections.

### T-41a-002 — Source options popover (pin, unpin, disable)
- Priority: P0
- Effort: S
- Depends: T-41a-001, T-41a-042
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt
- Acceptance:
  - Long-press, right-click and Menu key open a popover titled with `visualName` (`name` or `"name (LANG)"`) containing "Pin"/"Unpin" and "Disable".
  - "Disable" is absent for `LocalAnimeSource.ID == 0`.
  - Choosing an item closes the popover before applying; disable writes `hidden_anime_catalogues` (behavior section 6, ui-parity D5).
  - Mockup gap G4 (src/entry.rs:301 dialog stub) is closed.

### T-41a-003 — Source row base widget
- Priority: P0
- Effort: S
- Depends: area:16-source-api-anime, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/BaseAnimeSourceItem.kt
- Acceptance:
  - Row layout is icon slot (40x40), name column (one line, ellipsis), trailing action slot; language line shown only when `showLanguageInContent` is true and `lang != ""`.
  - Stub sources show "Not installed" in error colour (behavior section 13.2, used by migrate picker too).
  - Row click, long-press and right-click callbacks are wired (behavior section 20).

### T-41a-004 — Source and extension icon loading
- Priority: P0
- Effort: M
- Depends: area:34-images-covers-cache, area:19-extensions-anime, T-41a-045
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeIcons.kt
- Acceptance:
  - Source icon rules, in order (Kotlin `BrowseAnimeIcons.kt:42-80`): stub with no icon -> warning glyph tinted error; custom icon -> that icon; local source id 0 -> local icon; else default icon. Decisions D7/D8 in p1-behavior.md section 26.
  - Available extension icon loads `iconUrl` through the cover pipeline with placeholder fill `rgba(136,136,136,0.122)` (0x1F888888) and error fallback image.
  - Installed icon is read from the extension bundle with `zip` and decoded with `image` on a blocking thread; any failure falls back to the default source icon (edge E50).
  - Untrusted extensions show a `dangerous` glyph tinted error at 40x40.
  - Decoding never runs on the GTK thread (p1-rust-design.md section 4.2).

### T-41a-005 — Browse-source page state machine and typed navigation
- Priority: P0
- Effort: M
- Depends: T-41a-006, T-41a-007, T-41a-008, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt
- Acceptance:
  - Error-only state (no items, refresh or append error) shows the message and buttons "Retry", "Open in WebView", "Help" in that order; the local source shows only "Local source guide" (behavior 7.1 rules 1-4).
  - Items present with an error shows a bottom banner with "Retry" that does not auto-hide (`duration = Indefinite`).
  - Loading with no items shows a centered spinner; dispatch by display mode matches behavior 7.1 rule 4.
  - `BrowsePage` enum and `open(BrowsePage)` replace string ids for browse pages; `ui::pop_until` added for the migrate "popUntil lastItem" path (rust design 4.4).
  - Mockup gaps G13 and G21 are closed.

### T-41a-006 — Browse grids and list (comfortable, compact, cover-only, list)
- Priority: P0
- Effort: M
- Depends: area:37-ui-library, area:39-ui-entry-details-anime, T-41a-004
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceComfortableGrid.kt, app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceCompactGrid.kt, app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceList.kt
- Acceptance:
  - Grid spacing 4 dp both axes, outer padding 8 dp; list vertical padding 8 dp (behavior section 0 and 7.1).
  - Favorite covers at 34 % opacity and an in-library badge on favorites only (behavior section 0).
  - `CoverOnlyGrid` renders as the compact grid.
  - Prepend and append loading rows are full-width 16 dp padded spinners; list rows get `containerHeight = window height - top bar height`.
  - Cover recycle guard: a stale bind never applies a texture (edge R10, test T51).

### T-41a-007 — Browse toolbar (search, display mode, overflow)
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Display-mode popover shows exactly "Comfortable grid", "Compact grid", "List" with the current one checked; choosing one persists `pref_display_mode_catalogue` with serialized values `COMFORTABLE_GRID`, `COMPACT_GRID`, `LIST` (data 4).
  - Overflow order: "Open in WebView" (or "Help" for local), then "Settings" only for configurable sources (behavior 7.2).
  - Unknown persisted value falls back to `CompactGrid` (test T46).

### T-41a-008 — Stub source ("not installed") page
- Priority: P0
- Effort: S
- Depends: area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt
- Acceptance:
  - Page title is the stub's name; body is an empty state with text "Source not installed: <name (LANG)>" or the bare id when name or lang is blank (behavior section 7.1, data 3.9).
  - No retry action on this page.

### T-41a-009 — Extension grouping, query filter and debounce model
- Priority: P0
- Effort: L
- Depends: T-41a-016, T-41a-017, area:19-extensions-anime, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt, app/src/main/java/eu/kanade/presentation/components/AppBar.kt
- Acceptance:
  - Groups and order exactly as behavior 1.2-1.3: updates header (only if non-empty), installed+untrusted header, one text header per language (comparator with "all" first).
  - Installed sort `(!isObsolete, name ci)`; untrusted `name ci`; available split per source with synthetic pkg `<pkg>-<id>` and filter `enabled langs`.
  - Query filter matches behavior 1.4 exactly, including comma OR, numeric id exact match, `getHomeUrl` for HTTP installed sources, and no store-name search (tests T4-T7, T9).
  - Search text is debounced 250 ms (`SEARCH_DEBOUNCE_MILLIS`); recompute uses the latest snapshot and query (race R1).
  - NSFW filter applies to installed and available but not to untrusted (test T3).

### T-41a-010 — Extensions list UI (sections, rows, states)
- Priority: P0
- Effort: M
- Depends: T-41a-009, T-41a-015, T-41a-004
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt
- Acceptance:
  - Section headers render as in ui-parity S5; "Update all" button appears only on the updates header.
  - Row anatomy and warnings (UNTRUSTED / ORPHANED / 18+) and install step text match behavior 2.4.
  - Action buttons follow the state table in behavior 2.4 (including `Installed` treated as idle, test T16).
  - Empty states: "Well, this is awkward" without search, "No results found" with search; both carry an "Extension stores" action (behavior 2.2).
  - Mockup gaps G6, G7, G11, G19 closed.

### T-41a-011 — Extensions tab wiring (actions, badge, long-press, uninstall confirm)
- Priority: P0
- Effort: S
- Depends: T-41a-010, T-41a-012, area:19-extensions-anime, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsTab.kt
- Acceptance:
  - Overflow actions in order "Filter" and "Extension stores"; tab badge shows `animeext_updates_count` when > 0.
  - Long-press: Available -> install immediately; Installed or Untrusted whose directory exists -> uninstall immediately; otherwise the remove confirm dialog "Remove Extension?" / "Do you really want to remove "<name>" extension?" with "Remove" and "Cancel" (behavior 2.1, D3 replacement for `isPackageInstalled`, test in T-41a-011 acceptance only).
  - Secondary action for Available opens WebView on `sources[0].baseUrl` with the source name and id (behavior 2.1, task T-41a-026 for the button itself).
  - Mockup gap G9 closed.

### T-41a-012 — Untrusted extension trust dialog and flow
- Priority: P0
- Effort: S
- Depends: T-41a-009, area:19-extensions-anime, area:40-ui-browse-manga
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - Dialog title "Untrusted extension", body exactly "Malicious extensions can read any stored login credentials or execute arbitrary code.\n\nBy trusting this extension, you accept these risks.", buttons "Trust" and "Uninstall" (behavior 2.5).
  - Trust on a vanished untrusted entry is a no-op (test T28, race R13).
  - Dismiss without a button press closes the dialog and changes nothing.
  - Mockup gap G8 closed.

### T-41a-013 — Extension list refresh
- Priority: P0
- Effort: S
- Depends: T-41a-009, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - Refresh button and F5 start one fetch; `isRefreshing` stays true at least 1 s (behavior 1.6); a second request during refresh is ignored (test T17, edge E44).
  - Fetch failure shows the banner "Failed to fetch available extensions" with Retry and keeps the previous snapshot (data 6).
  - Refresh runs on first open as in Kotlin `init`.

### T-41a-014 — Update all
- Priority: P1
- Effort: S
- Depends: T-41a-015, T-41a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt
- Acceptance:
  - Starts an update for each installed extension with `hasUpdate` that is in the CURRENT filtered set (edge E2, test T14).
  - Starts at most 4 installs at once (Proposal over Kotlin's unbounded launch; edge E17).
  - Ctrl+U triggers it on the list page.

### T-41a-015 — Install, update, cancel and retry lifecycle with install-step map
- Priority: P0
- Effort: M
- Depends: T-41a-009, area:19-extensions-anime, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt
- Acceptance:
  - `InstallStep` enum values `Idle, Pending, Downloading, Installing, Installed, Error` with `isCompleted` = Installed|Error|Idle (behavior 1.11).
  - Step map keyed by synthetic pkg; entry removed on stream completion (`onCompletion`); the row shows a 40 dp spinner and "Pending"/"Downloading"/"Installing" text while not completed (test T15).
  - Cancel button calls cancel-by-pkg and the stream ends; the map entry is removed (test T19, race R3/R4).
  - Error step shows the Retry (refresh icon) action that re-runs install (behavior 2.4).

### T-41a-016 — Extension list model state and preference wiring
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Reads `show_nsfw_source` (default true), `source_languages` (default `all`, `en`, device language), `animeext_updates_count` (default 0), and `ExtensionInstaller` from base prefs (data 2.1, 2.3).
  - Pref change recomputes groups (race R8).
  - Installer kind (`requiresSystemPermission`) is kept as data for parity but produces no banner on Linux (T-41a-025).

### T-41a-017 — Language display names, comparator and default enabled languages
- Priority: P0
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - `getSourceDisplayName` mapping for `last_used`, `pinned`, `other`, `all`, else localized (behavior section 0); `all` displays "Multi".
  - `zh-CN` -> `zh-Hans` and `zh-TW` -> `zh-Hant` locale mapping; `""` uses the system language, falling back to "Other" (test T43).
  - Comparator puts `all` first, then localized names (test T41 and grouping tests).
  - Default enabled set is `{all, en, device language}` created once on first run (rust design section 12, pitfall 11).

### T-41a-018 — Extension language filter (page and model)
- Priority: P1
- Effort: S
- Depends: T-41a-017, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionFilterScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionFilterScreenModel.kt, app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionFilterScreen.kt
- Acceptance:
  - Languages come from available extensions, enabled first then comparator order (behavior 2.6).
  - Each row is a switch titled with the localized display name; toggling flips membership in `source_languages`.
  - Failure shows the banner "InternalError: Check crash logs for further information" (MR internal_error).
  - Empty list shows "Well, this is awkward".

### T-41a-019 — Extension details model
- Priority: P1
- Effort: M
- Depends: T-41a-009, T-41a-021, area:19-extensions-anime, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeExtensionDetailsScreenModel.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeIncognito.kt
- Acceptance:
  - Source list sorted enabled first, then by `name` (labelAsName) or localized language, lower-cased (behavior 4.1).
  - `labelAsName = multiSource && !multiLangSingleSource` (behavior 4.1).
  - Enable all / disable all write the whole id list in one update; per-source toggle flips; incognito writes the real pkg (edge E55).
  - Package vanishing from the installed list pops the page (edge E18, race R7).

### T-41a-020 — Extension details page
- Priority: P1
- Effort: M
- Depends: T-41a-019, T-41a-022, T-41a-024, T-41a-047, area:40-ui-browse-manga
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt
- Acceptance:
  - Header anatomy, info row (Version, Language, 18+ with age-rating dialog), buttons, incognito switch and source rows match ui-parity S7 and behavior 4.2-4.4.
  - Orphaned warning banner text is exactly `obsolete_extension_message` (behavior 4.2 item 1).
  - App bar shows open-repo icon only when a store URL exists; overflow has "Enable all", "Disable all", "Clear cookies" in that order.
  - Debug info text matches the template in behavior 4.3 byte for byte (test T24).

### T-41a-021 — Clear cookies for an extension
- Priority: P1
- Effort: S
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeExtensionDetailsScreenModel.kt
- Acceptance:
  - URLs = base URL and home URL of each HTTP source, non-empty, distinct; cookies removed per URL; per-URL failure counted as 0 and logged (behavior 4.1, test T25).
  - User sees a banner "Cleared <n> cookies" (Proposal over Kotlin's silent log; edge E16).

### T-41a-022 — Open source repo link
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt
- Acceptance:
  - Regex `https://raw.githubusercontent.com/(.+?)/(.+?)/.+` on the store index URL yields `https://github.com/<user>/<repo>`; otherwise the raw index URL is opened (behavior 4.2; test T24, edge E49).
  - Opens with `gtk::UriLauncher`.

### T-41a-023 — Source preferences page (native renderer)
- Priority: P1
- Effort: L
- Depends: area:16-source-api-anime, area:02-preferences-system, T-41a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt
- Acceptance:
  - Page title is the source `toString()`; body built from the source's declared preference spec; non-configurable source shows "Well, this is awkward" (behavior 5).
  - Renders at least: switch, entry (single-line text), list (drop-down), multi-select (dialog with checkboxes), info text, action row.
  - Entry fields use `gtk::Entry` with `InputHints::NO_SPELLCHECK | InputHints::PRIVATE` (D6); dialog-type prefs use a dialog titled with the preference title when no explicit dialog title (behavior 5).
  - Writes go to the source's own preference store and are read back immediately (race R11, test T52).
  - Mockup gap G24 closed.

### T-41a-024 — Replace "App info" with "Open folder"
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt
- Acceptance:
  - Decision: Android app-info page is Dropped (D1); the Linux replacement is an "Open folder" button that opens the extension bundle directory with `gtk::FileLauncher`.
  - The button is shown only when the bundle directory exists (behavior 4.3, ui-parity S7 button row).
  - User loss documented in behavior section 26 (D1).

### T-41a-025 — Drop the "install unknown apps" permission banner
- Priority: P3
- Effort: S
- Depends: T-41a-010
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt
- Acceptance:
  - Decision: Dropped (D2). No banner, no permission request, no `launchRequestPackageInstallsPermission` on Linux.
  - The installer preference is still read for export parity (T-41a-016) but has no effect on the list.
  - Behavior section 26 records the user loss: none on Linux (no such permission).

### T-41a-026 — Available extension "Open in WebView" action
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview, T-41a-010
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsTab.kt
- Acceptance:
  - Globe button appears on Available rows only when `sources` is non-empty; opens the WebView page with `sources[0].baseUrl`, title `sources[0].name`, source id (behavior 2.1, 2.4).
  - Hidden for Installed and Untrusted rows.

### T-41a-027 — Global search page (per-source blocks)
- Priority: P1
- Effort: M
- Depends: T-41a-029, T-41a-030, T-41b-033
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/GlobalAnimeSearchScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt
- Acceptance:
  - One block per source in model order; loading = spinner row, success = card row or "No results found", error = message text (behavior 8.1).
  - Block click opens the source's Browse page with the same query.
  - Global search honors the "Pinned" filter through the model (part 2 owns the model; this task renders it).
  - Mockup gaps G15 and G23 closed (src/entry.rs:432 ignores its id today).
  - Empty query shows the hint "Search a source by title" (Proposal, ui-parity S4).

### T-41a-028 — Global search toolbar (progress, chips, persisted "Has results")
- Priority: P1
- Effort: S
- Depends: T-41a-027, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Progress bar visible iff `1 <= progress < total` (test T48, edge E39-E40).
  - Chips "Pinned" (default selected in migrate), "All", "Has results" in that order; "Has results" writes `has_filters_toggle_state` (app state key).
  - Source filter change re-runs the search immediately (behavior 8.3); a chip shows a spinner while running (Proposal for the unresolved UX note in GlobalAnimeSearchToolbar.kt:75).

### T-41a-029 — Global search result card row
- Priority: P1
- Effort: S
- Depends: area:37-ui-library, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt
- Acceptance:
  - Cards 96 dp wide, title max 3 lines, favorite badge and dimming, titles resolved live from the DB row (behavior 8.2).
  - Empty result list shows "No results found" with padding 16/8 dp.
  - Click opens the entry; long-press opens the entry with `fromSource = true`.

### T-41a-030 — Global search source header, origin marker and subtitle
- Priority: P1
- Effort: S
- Depends: T-41a-017, area:40-ui-browse-manga
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/GlobalAnimeSearchScreen.kt, app/src/main/java/eu/kanade/presentation/browse/GlobalSearchResultItems.kt
- Acceptance:
  - Header title is `"▶ <name>"` only for the migrate origin source, else the plain name (behavior 8.1).
  - Subtitle uses `getLocalizedDisplayName(lang)` (not the "all" special case): "all" shows localized "All" (edge E19, test T41).
  - Error body shows `throwable.message`; no retry on this row.

### T-41a-031 — Source filter page (language switches and checkboxes)
- Priority: P1
- Effort: S
- Depends: T-41a-017, T-41a-042, T-41b-008
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt
- Acceptance:
  - Language header switch toggles `source_languages`; sources of an enabled language are listed with checkboxes (checked = not in `hidden_anime_catalogues`) (behavior 9).
  - Disabling a language hides its sources and leaves their checkbox state untouched (edge E15, test T23).
  - Empty state text "No installed source found".
  - Mockup gap G5 (src/entry.rs:266-272 dead closure) closed.

### T-41a-032 — Migration flags bitmap and option list
- Priority: P1
- Effort: S
- Depends: area:34-images-covers-cache, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/AnimeMigrationFlags.kt
- Acceptance:
  - Bits: EPISODES 0x01, CATEGORIES 0x02, CUSTOM_BACKGROUND 0x04, CUSTOM_COVER 0x08, DELETE_DOWNLOADED 0x10 (data 5.1); option order EPISODES (episodes only), CATEGORIES, CUSTOM_COVER, CUSTOM_BACKGROUND, DELETE_DOWNLOADED (only when downloads > 0).
  - Default selection from the saved bitmap; saved default `0x7FFFFFFF` selects all five (test T55).
  - Saved bitmap stores exactly the selected bits; unused bits preserved (test T54).
  - Decision recorded: "Delete downloaded" keeps its parity default (pre-selected) per behavior Q2; the change to default-off is a user decision and is NOT applied silently.

### T-41a-033 — Separate anime migrate flags key
- Priority: P1
- Effort: S
- Depends: T-41a-032, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - Anime writes `anime_migrate_flags`; manga keeps `migrate_flags` (edge E9, test T33).
  - First run imports the old `migrate_flags` value once into `anime_migrate_flags` when present, then never reads it again.
  - Migration writes the flags BEFORE any work (behavior 11.3 step 1).

### T-41a-034 — Migration engine (ordered side effects, transactional)
- Priority: P1
- Effort: L
- Depends: T-41a-032, T-41a-033, area:05-data-repos-entries-items, area:07-domain-entries-items, area:08-domain-library-category-history-updates, area:27-tracking-core, area:24-downloads-anime, area:34-images-covers-cache, area:16-source-api-anime, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt
- Acceptance:
  - Order exactly as behavior 12 steps 1-9 (episode fetch first, then episodes, categories, tracks, delete downloads, unfavorite old when replace, covers, backgrounds, favorite new with flags, dateAdded rule).
  - Episode transfer: `seen` by max-seen rule, `date_fetch` and `bookmark` copied from the recognized-number match (test T31).
  - Proposal deviation E20: `last_second_seen` and `fillermark` also copied; documented in data 7.
  - Transaction: episode fetch before the DB transaction; all DB steps in one transaction; download deletion after commit; any error rolls back (test T30, E34).
  - Tracks: enhanced tracker rejection drops the track and the outcome lists it (test T32); non-enhanced trackers keep their remote id.
  - `flags == 0` still refreshes episodes, moves tracks and flips favorite (test T35).

### T-41a-035 — Migration confirm dialog
- Priority: P1
- Effort: M
- Depends: T-41a-034, T-41a-041, T-41a-045
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt
- Acceptance:
  - Title "Select data to include"; checkboxes only when `canMigrate`; error row messages exactly `label_cant_migrate_season` / `label_cant_migrate_episode` when not (behavior 11.1).
  - Buttons: "Show anime" always; "Show seasons" when target is not Episodes; "Copy" and "Migrate" only when `canMigrate`; "Migrate" is the suggested action.
  - Running state: spinner over a 70 % background, buttons hidden, Escape ignored; double start refused (test T37, race R5/R6).
  - Errors are shown ("Migration failed: <reason>") and nothing is written (Proposal over the silent path, behavior 11.3; test T30).
  - Mockup gap G17 closed.

### T-41a-036 — Migrate source picker (sorting, counts, copy id)
- Priority: P1
- Effort: S
- Depends: T-41b-039, T-41a-003, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceScreenModel.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt
- Acceptance:
  - Sticky header: prompt "Select a source to migrate from"; sort-mode button (alphabetical icon or count icon) and direction button (up/down) with the tooltips of behavior 13.2.
  - Sort rule per behavior 13.1; DESCENDING reverses stubs too (test T45, edge E36); Local source (id 0) never listed (behavior 13.1).
  - Count badge shows the favorite count; long-press copies the source id (behavior 13.2).
  - Empty state text is "No source has favorites to migrate" (Proposal replacing the Kotlin reuse of "Your library is empty", which is a text bug; recorded in ui-parity S9).

### T-41a-037 — Migrate favorites list
- Priority: P1
- Effort: S
- Depends: T-41a-003, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/MigrateAnimeScreenModel.kt
- Acceptance:
  - Rows sorted by `title` case-insensitively; row click opens the migrate search; cover click opens the entry (behavior 14).
  - Empty state "Well, this is awkward"; load failure shows the internal error banner.
  - Title is the source name; a missing source shows the stub name instead of crashing (replaces Kotlin `state.source!!`).

### T-41a-038 — Migrate search (seeded query, origin-first, navigation after migrate)
- Priority: P1
- Effort: M
- Depends: T-41a-035, T-41a-030, T-41b-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeSearchScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeSearchScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/AnimeMigrateSearchScreenDialogScreenModel.kt
- Acceptance:
  - Query seeded with the old anime's title; search starts automatically; origin source first, then pinned, then name (behavior 15).
  - Default filter is PinnedOnly; "All" chip widens it (edge E13, test T40).
  - Card click opens D1; after migrate: when the top item is an entry page, pop to it then push the new entry, else replace (behavior 11.2).
  - Long-press opens the entry with `fromSource = true`.

### T-41a-039 — Per-source migrate search
- Priority: P1
- Effort: M
- Depends: T-41a-035, T-41a-005, area:36-ui-shell-navigation-security-webview, T-41b-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/AnimeSourceSearchScreen.kt
- Acceptance:
  - Toolbar is a plain search bar (no display-mode button, no overflow) per ui-parity S12.
  - Filter FAB visible only when the source has filters; opens the filter dialog from part 2.
  - After migrate: pop to root, open the Browse tab, push the new entry (behavior 11.2).
  - WebView and help actions are offered from the empty states (behavior 7.1).

### T-41a-040 — Season migration picker
- Priority: P1
- Effort: M
- Depends: T-41a-035, T-41a-050, area:34-images-covers-cache, area:16-source-api-anime, area:10-app-domain-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/season/MigrateSeasonSelectScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/season/MigrateSeasonSelectScreenModel.kt
- Acceptance:
  - Seasons fetched in one call through `awaitSeasonsUpdate`; each season mapped to a local row; favorites hidden when `browse_hide_in_anime_library_items` is true (behavior 17, edge E14, test T50).
  - Columns: `pref_animelib_columns_landscape_key` / `_portrait_key`, 0 = adaptive 128 dp, n = fixed n (edge E38, test T47); orientation becomes a width breakpoint (T-41a-050).
  - Click opens D1 with the season as the new anime; "Show seasons" from D1 pushes this page.
  - Mockup gap G14 closed (`seasons()` stub).

### T-41a-041 — Fetch-type rules for migrate
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/AnimeMigrationFlags.kt
- Acceptance:
  - `canMigrate = old.fetchType == new.fetchType` (behavior 11.1); when false only "Show anime"/"Show seasons" are shown and the matching error message appears (tests T36, edges E41-E43).
  - "Show seasons" visible iff new.fetchType != Episodes.
  - Episodes flag shown only when old.fetchType == Episodes (behavior 10).

### T-41a-042 — Source enable, hidden and pinned id sets
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `hidden_anime_catalogues` and `pinned_anime_catalogues` are string sets of decimal ids; parsing back uses integer parsing and skips invalid entries with a log line (edge E54, test T38).
  - Toggle semantics: enable removes, disable adds; bulk list variant for enable/disable all (behavior 4.1).
  - Writes use read-modify-write under one lock so concurrent toggles do not lose updates.

### T-41a-043 — Pin bits and pinned ordering
- Priority: P0
- Effort: S
- Depends: T-41a-042
- Sources: domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `Pin(u8)` with `UNPINNED=0b00`, `PINNED=0b01`, `ACTUAL=0b10`; `is_pinned` tests bit 0 (data 5.2).
  - Pinned sources sort first (`!pinned`, then name) in the list, global search and migrate search (behavior 6, 8.4, 15).

### T-41a-044 — Desktop keyboard and mouse map for this part
- Priority: P1
- Effort: M
- Depends: T-41a-005, T-41a-010, T-41a-027
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt, app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt
- Acceptance:
  - Keys from ui-parity section 21 work on the list, browse and global pages: Ctrl+F, Esc, F5/Ctrl+R, Ctrl+U, Enter, Shift+F10/Menu, Space on switches.
  - Right-click and Menu key trigger the long-press action on every row and card.
  - Esc closes the search bar first, a second Esc goes back.

### T-41a-045 — Banner/toast primitive and generated string table
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - `InfoBanner` widget (gtk::Revealer + label + optional button) supports indefinite (no auto-hide) and 4 s and 2 s durations (ui-parity section 19).
  - String table generated from the two English base files; every key in p1-behavior.md section 24 exists with the exact English text (test in T-41a-049).
  - Mockup gap G22 closed.

### T-41a-046 — Drop Fragment, reflection and Compose-only mechanics
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt, app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt
- Acceptance:
  - Decision: Dropped (D4, D5, D9, D18). The FragmentContainer, `onContainerAvailable` reflection, `ContextThemeWrapper`, `animateItem`, `animateItemFastScroll`, `FastScrollLazyColumn` fast-scroll thumb are NOT ported.
  - User loss documented in behavior section 26 (no animation of list reorders, no fast-scroll letter preview).
  - No code path uses reflection or AndroidX preference classes in Kurobayomi.

### T-41a-047 — Clipboard actions (copy source id, copy debug info)
- Priority: P2
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt, app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt
- Acceptance:
  - Copy uses `gdk::Clipboard::set_text`; source id copied as decimal text; debug info copied with label-free text (behavior 4.3).
  - A transient "Copied" banner (2 s) confirms the action (Proposal over Kotlin's silent copy).

### T-41a-048 — Help and local-source guide actions
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - "Help" opens `https://aniyomi.org/docs/guides/troubleshooting/` (`Constants.URL_HELP`).
  - "Local source guide" opens `https://aniyomi.org/help/guides/local-anime/` (`LocalAnimeSource.HELP_URL`) in the external browser via `gtk::UriLauncher`.
  - Shown exactly where behavior 7.1 and 7.2 say (local source only in its empty state and toolbar).

### T-41a-049 — Unit and integration tests for the part
- Priority: P0
- Effort: M
- Depends: T-41a-009, T-41a-032, T-41a-034, T-41a-045
- Sources: app/src/test/java/mihon/core/migration/MigratorTest.kt
- Acceptance:
  - All tests T1-T56 in p1-edge-cases-and-tests.md section 4 exist and pass under `cargo test`.
  - Fixtures from p1-edge-cases-and-tests.md section 5 are checked in.
  - Integration tests I1-I4 run where a display is available; skipped otherwise, with the skip reason printed.
  - String test: every key in p1-behavior.md section 24 matches the English base text (drift fails the test).

### T-41a-050 — Responsive layout and width breakpoints for part-1 pages
- Priority: P1
- Effort: S
- Depends: T-41a-006
- Sources: app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceComfortableGrid.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/season/MigrateSeasonSelectScreenModel.kt
- Acceptance:
  - Width >= 720 px uses the landscape columns pref, else portrait (D14; rust design section 10).
  - Extension list max width 960 px centred; details max content width 640 px.
  - At 360 px width no text is clipped and no page scrolls horizontally except the chip rows (ui-parity section 22).
  - Mockup gap G25 addressed.

### T-41a-053 — Extension stores navigation link
- Priority: P1
- Effort: S
- Depends: area:19-extensions-anime, T-41a-011
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt, app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt
- Acceptance:
  - "Extension stores" appears in the list overflow menu and in both empty states; it opens the stores page owned by area:19 (mockup gap G18 closed by linking the real page).
  - The link is not duplicated in the details page.
