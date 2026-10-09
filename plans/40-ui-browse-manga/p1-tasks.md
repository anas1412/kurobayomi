# Area 40 (part 1): implementation tasks, manga browse UI

Task IDs `T-40a-nnn`. Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/40-ui-browse-manga/p1-tasks.md`.

Priority rules (from the project brief): P0 = needed to read manga and use one source daily; P1 = parity features most users rely on; P2 = polish and long tail; P3 = Android-only or obsolete, with a drop decision.

Sources cite repo-relative paths in `/home/blackbox/aniyomi-src`. Specs: p1-behavior.md (B§), p1-ui-parity.md (U§), p1-rust-design.md (R§), p1-data-and-config.md (D§), p1-edge-cases-and-tests.md (E-40 / TC-40).

Dependencies on other areas use `area:<id>`. Part 2 of this area is `p2-tasks.md` in this folder (IDs `T-40b-*`); depend on those IDs directly.

---

## P0: needed for daily use (reading from one source)

### T-40a-001 — Browse tab shell, six sub-pages, tab-switch channel
- Priority: P0
- Effort: M
- Depends: T-40a-033, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt
- Acceptance:
  - Browse shows six pages in this order: anime sources, manga sources, anime extensions, manga extensions, anime migrate, manga migrate (B§1). Manga pages are index 1, 3, 5.
  - `show_extension()` selects page 3 and `show_anime_extension()` selects page 2 through a bounded channel of capacity 1 with drop-oldest behaviour (E-40 R-40-08 test).
  - Re-selecting the Browse tab opens global search for the visible kind (manga or anime), not always anime (D-UI-1).
  - The shared search entry is wired to the visible extensions page; a debounce of 250 ms applies (B§5.1).

### T-40a-002 — Browse source header bar and display-mode menu
- Priority: P0
- Effort: S
- Depends: T-40a-033, T-40a-003, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaSourceToolbar.kt
- Acceptance:
  - Header shows back, source name, search toggle, display-mode button, overflow (B§2.12, U§3.2).
  - Display-mode button icon is list when mode is List, grid otherwise; the popover has exactly three radio items: comfortable grid, grid (compact), list (B§2.12).
  - Overflow shows help (local source) or open-in-web-view (other sources), and settings only for configurable sources.
  - Display mode is stored under `display_mode` in `browse.toml` (D§10), default `COMPACT_GRID`, and persists across restarts.

### T-40a-003 — Browse source content: loading, error, empty, stub and dispatch
- Priority: P0
- Effort: M
- Depends: T-40a-030, T-40a-004, T-40b-030, T-40b-032, T-40b-040
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/BrowseMangaSourceScreen.kt, app/src/main/java/eu/kanade/presentation/browse/BrowseSourceLoadingItem.kt
- Acceptance:
  - Error selection prefers refresh over append (B§2.2). With items present, a bottom toast bar shows the message with `action_retry` and does not auto-hide (B§2.3).
  - With no items and an error: empty state with three actions in order (retry = refresh, open in web view, help), or the local-source help action only for `LocalMangaSource` (B§2.4).
  - First load with no items shows a centred spinner (B§2.5).
  - Empty without error shows `empty_screen` text (D-UI-2).
  - Stub source shows `source_not_installed` with the source string and only back navigation (B§2.11).
  - Display mode `ComfortableGrid`, `List`, `CompactGrid`, `CoverOnlyGrid` dispatch as in B§2.6.

### T-40a-004 — Browse grids and list cells with favorite alpha and in-library badge
- Priority: P0
- Effort: M
- Depends: T-40a-005, T-40a-006, area:37-ui-library, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaSourceComfortableGrid.kt, app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaSourceCompactGrid.kt, app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaSourceList.kt, app/src/main/java/eu/kanade/presentation/browse/BrowseBadges.kt, app/src/main/java/eu/kanade/presentation/browse/GlobalSerachCard.kt
- Acceptance:
  - Grid padding is 8 px around the content and 4 px between cells (B§2.7).
  - Favorite covers render at alpha 0.34; non-favorites at 1.0 (B§2.7, D§13).
  - The bookmark badge shows only for favorites (B§2.10).
  - Items are paged through a `gio::ListStore` with a single in-flight page request (R§3, E-40 R-40-09); a null placeholder renders nothing.
  - List mode uses container height minus top bar height (B§2.8).
  - Verify whether `GlobalSearchCard` (GlobalSerachCard.kt) has any caller; if none, delete it in T-40a-048 (D13).

### T-40a-005 — Browse row primitives (base row, source row)
- Priority: P0
- Effort: S
- Depends: T-40a-033
- Sources: app/src/main/java/eu/kanade/presentation/browse/BaseBrowseItem.kt, app/src/main/java/eu/kanade/presentation/browse/manga/components/BaseMangaSourceItem.kt
- Acceptance:
  - Row padding 16 px horizontal and 8 px vertical (B§2.7 / U§3.6 mapping of `medium` and `small`).
  - Primary click and long press are distinct; right-click is an alias for long press on desktop (U§9).
  - Source row shows icon (40 px high, 1:1), name (one line, ellipsis), language (hidden when `showLanguageInContent` is false), and an action slot.

### T-40a-006 — Source icon resolution
- Priority: P0
- Effort: S
- Depends: T-40a-033, area:14-source-local-and-metadata
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaIcons.kt
- Acceptance:
  - Icon order: stub with no icon -> warning glyph in error colour; custom icon if present; local source -> local icon; otherwise default source icon (U§3.6).
  - Icons are 40 px high and square (`height(40) aspectRatio(1)`).
  - Source icon lookup is cached per source id; a missing icon never blocks the row.

### T-40a-007 — Installed sources list grouped by language
- Priority: P0
- Effort: M
- Depends: T-40a-005, T-40a-033, T-40b-010, T-40b-011
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt
- Acceptance:
  - Language header rows (display name) then source rows (B§3.1).
  - Click opens popular listing; the Latest button appears only when `supportsLatest` and opens latest (B§3.2).
  - Pin button: filled primary icon when pinned, outline at 50% alpha otherwise; accessible name `action_pin` / `action_unpin` (B§3.2).
  - Empty shows `source_empty_screen`; loading shows a spinner (B§3.1).

### T-40a-008 — Source options dialog (pin/unpin and disable)
- Priority: P1
- Effort: S
- Depends: T-40a-007
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt
- Acceptance:
  - Long-press a source opens a modal with title `visualName` and rows: pin/unpin, and disable (hidden for the local source id 0) (B§3.3, U§4.3).
  - Escape closes; no confirm button.
  - The data-saver row is not implemented (see T-40a-042).

### T-40a-009 — Show/hide sources (language switches and source checkboxes)
- Priority: P1
- Effort: M
- Depends: T-40a-007, T-40a-032
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesFilterScreen.kt
- Acceptance:
  - Language header switch toggles membership in `source_languages`; off hides that language's sources (B§4, E-40 E-40-100).
  - Source checkbox is checked when the id string is not in `hidden_catalogues` (B§4).
  - Empty shows `source_filter_empty_screen`.
  - Ids are stored as strings to match upstream backups (D§5 notes).

### T-40a-010 — Global search toolbar with progress bar and filter chips
- Priority: P1
- Effort: S
- Depends: T-40a-005, T-40a-033
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/components/GlobalMangaSearchToolbar.kt
- Acceptance:
  - Progress bar visible only when `1 <= progress < total` (E-40-57..59).
  - Chips: `pinned_sources` (selected when PinnedOnly), `all` (selected when All), separator, `has_results` (toggle) (B§10.1).
  - Chip change does not rerun the query (D-UI-10).

### T-40a-011 — Global search result rows, cards and per-source states
- Priority: P1
- Effort: M
- Depends: T-40a-010, T-40a-004
- Sources: app/src/main/java/eu/kanade/presentation/browse/GlobalSearchResultItems.kt, app/src/main/java/eu/kanade/presentation/browse/manga/GlobalMangaSearchScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/components/GlobalMangaSearchCardRow.kt
- Acceptance:
  - Header row per source: title (with `"▶ "` prefix when it is the origin source), subtitle = `getLocalizedDisplayName`, arrow button (B§10.2).
  - Loading: 16 px spinner; success: horizontal cards 96 px wide with 3-line titles; empty: `no_results_found`; error: error icon with raw message or `unknown_error` (B§10.2-10.3).
  - Card click opens the manga; long-press opens preview (B§10.3).

### T-40a-012 — Migration source picker with sort header
- Priority: P1
- Effort: M
- Depends: T-40a-005, T-40a-032
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MigrateMangaSourceScreen.kt, app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt
- Acceptance:
  - Sticky sort row with prompt `migration_selection_prompt`, mode toggle (alphabetical / total), direction toggle (asc / desc) (B§11).
  - Mode and direction persist to `migrate_sort_mode` and `migrate_sort_direction` (D§5). Defaults ALPHABETICAL and ASCENDING.
  - Row count badge; `not_installed` in error colour for stub sources; language shown only when non-empty (B§11).
  - Long-press copies the source id to the clipboard (B§11).
  - Empty shows `information_empty_library`.

### T-40a-013 — Migration favorites list of the old source
- Priority: P1
- Effort: S
- Depends: T-40a-005, T-40a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/manga/MigrateMangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/manga/MigrateMangaScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MigrateMangaScreen.kt
- Acceptance:
  - Favorites sorted by case-insensitive title (B§12.1; E-40-93, E-40-94).
  - Row click opens the migrate search for that manga; cover click opens the manga (B§12.2).
  - Load failure shows toast `internal_error` and an empty list (B§12.2).
  - Title is the source name; a missing source does not panic (no `!!`).

### T-40a-014 — Migrate search (global and per-source) with origin-first ordering
- Priority: P1
- Effort: L
- Depends: T-40a-010, T-40a-011, T-40a-015, T-40b-067
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaSearchScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaSearchScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MangaSourceSearchScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MangaMigrateSearchScreenDialogScreenModel.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MigrateMangaSearchScreen.kt
- Acceptance:
  - Query prefilled with the manga title and searched on open (B§13.1).
  - Source order: origin first, then pinned, then `"name (lang)"` lowercased; PinnedOnly filters first (B§13.1; TC-40-025).
  - Per-source search uses the browse list with migrate routing (tap = dialog, long-press = preview) and a filter button in the header when filters exist (U§6.4).
  - Post-migrate navigation follows B§13.1 and B§13.2 (TC-40-114, TC-40-115).
  - A missing manga shows an error page, not a crash (E-40-63).

### T-40a-015 — Migration flags codec and confirm dialog
- Priority: P1
- Effort: M
- Depends: T-40a-016, T-40a-032, area:23-downloads-manga, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/MangaMigrationFlags.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - Flag values 1 (chapters), 2 (categories), 8 (custom cover, shown only if the old manga has one), 16 (delete downloaded, shown only if downloads exist) (D§4, B§14.2).
  - Default selection from `migrate_flags`, default `Int.MAX_VALUE`; the stored value is updated on every migrate action (B§14.2-14.3).
  - Buttons: show manga, copy (replace = false), migrate (replace = true) (B§14.1).
  - While migrating, the dialog body is a spinner over a 70% tint.
  - Failures: dialog stays open; toast `internal_error` (D-UI-4).
  - Disclosure line "Tracking data is always transferred." (key `migrate_tracks_always`, U§6.5, T-40a-045).

### T-40a-016 — Migration engine (migrateMangaInternal) with pure chapter merge
- Priority: P1
- Effort: L
- Depends: T-40a-038, area:03-db-manga-schema-migrations, area:05-data-repos-entries-items, area:07-domain-entries-items, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, app/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt
- Acceptance:
  - Step order exactly as B§15: sync chapters (errors ignored), chapter state copy (flag 1), categories (flag 2), tracks (always), delete downloads (flag 16, only if old source present), unfavorite old (replace only), custom cover (flag 8 and old has cover), favorite flip with `chapterFlags`/`viewerFlags` copied and `dateAdded` (old if replace, else now ms).
  - `merge_chapter_state` matches R§3 pure rules; TC-40-020..024 pass.
  - Migration aborts before writes when the chapter fetch fails (TC-40-037).
  - Missing new source returns `NewSourceMissing` without writes (TC-40-036).

### T-40a-017 — Extensions tab: grouped list, headers, rows, refresh
- Priority: P0
- Effort: L
- Depends: T-40a-024, T-40a-018, T-40a-025, T-40a-030, T-40a-033, area:18-extensions-manga, area:20-extension-strategy-redesign
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsTab.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt
- Acceptance:
  - Groups in order: updates pending (only when updates exist), installed plus untrusted, one per language in `compare_languages` order (B§5.3, TC-40-006..008).
  - Row shows icon 40 px with busy spinner, name, secondary line (language, version, warning, status text) (B§5.8, U§7.1).
  - Action buttons follow the table in U§7.2 (B§5.8.1).
  - Refresh runs `findAvailableExtensions`; spinner minimum 1 s; spinner always resets (E-40-28 fix) (B§5.5).
  - Empty states: `no_results_found` with the repos button when a query exists, `empty_screen` otherwise (B§5.4).
  - Tab overflow has filter and extension repos (B§5.14).

### T-40a-018 — Extension install, update, uninstall, cancel state machine
- Priority: P0
- Effort: L
- Depends: area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt
- Acceptance:
  - `InstallStep` has six values and `is_completed` = Idle | Installed | Error (B§6).
  - Per-pkg download map is set on each step and cleared on completion, including cancel (TC-40-011, TC-40-012, TC-40-105).
  - A second concurrent job for the same pkg does not clear the map early (E-40-23 fix, reference count).
  - Update-all runs sequentially over installed extensions with `hasUpdate` (B§5.6, E-40-26).
  - Uninstall removes the package; the details page pops when the extension disappears (B§7.5).

### T-40a-019 — Trust dialog for untrusted extensions
- Priority: P1
- Effort: S
- Depends: T-40a-017
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt
- Acceptance:
  - Tapping an untrusted row (or its trust button) opens an alert: title `untrusted_extension`, message `untrusted_extension_message`, confirm `ext_trust`, cancel-role `ext_uninstall` (B§5.11, U§7.4).
  - Confirm trusts the extension; the cancel button uninstalls it; dismissing does nothing.
  - A trust failure is logged and shows a toast (E-40-29 fix).

### T-40a-020 — Extension language filter page
- Priority: P1
- Effort: S
- Depends: T-40a-032, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionFilterScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionFilterScreenModel.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionFilterScreen.kt
- Acceptance:
  - One switch per extension language; on = language in `source_languages` (B§8).
  - Toggle writes the set (add or remove); the extension list updates.
  - A failed language fetch shows toast `internal_error`.
  - Empty shows `empty_screen`.

### T-40a-021 — Extension details page (header, info, incognito, repo link, debug copy)
- Priority: P1
- Effort: M
- Depends: T-40a-031, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaExtensionDetailsScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaExtensionDetailsScreenModel.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt
- Acceptance:
  - Header: 112 px icon, name, stripped package name, info row (version, language, NSFW with age-rating dialog) (B§7.2, U§8.2).
  - Repo link: raw GitHub URL converted with the exact regex; open-repo action only when a repo URL exists (E-40-32..35).
  - Header click copies the debug block (exact text, B§7.2) and shows toast "Copied" (D-UI-5).
  - Incognito switch writes `incognito_extensions` (B§7.1).
  - Obsolete banner shows `obsolete_extension_message` (B§7.2).
  - Empty extension shows `empty_screen`; the page pops on uninstall (E-40-31).

### T-40a-022 — Extension details: source rows, enable/disable all, cookie clearing
- Priority: P1
- Effort: M
- Depends: T-40a-021, area:12-network-stack, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaExtensionDetailsScreenModel.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt
- Acceptance:
  - Source rows sorted by `(!enabled, label)` where label is raw name if label-as-name else lowercased display name (B§7.1, E-40-39).
  - Row click toggles the source; read-only switch; settings icon only for configurable sources (B§7.2).
  - Enable all / disable all act on every source id of the extension (E-40-38).
  - Clear cookies removes cookies for each distinct non-empty HTTP base URL and logs the count; no UI feedback beyond the log (B§7.1, E-40-36, E-40-37).
  - Overflow actions are enable all, disable all, clear cookies (B§7.2).

### T-40a-023 — Source preference page (declarative rows)
- Priority: P1
- Effort: L
- Depends: T-40a-022, area:17-source-app-layer, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt
- Acceptance:
  - Rows are built from the source's declared preference list (R§3 trait `ConfigurableSource`, B§9).
  - Dialog preferences use their title as dialog title when no dialog title is set (B§9).
  - Edit-text rows use the incognito input hint (no spellcheck, no emoji) and no history (U§8.3).
  - Shows a spinner until the source registry has loaded (B§9).
  - Stored values go to the source's own preference store (`sourcePreferences`).

### T-40a-024 — Extension query filter (comma-separated OR)
- Priority: P0
- Effort: S
- Depends: T-40a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt
- Acceptance:
  - `matches_query` matches TC-40-001..005 and TC-40-009 exactly (B§5.2).
  - Installed non-HTTP sources do not match on base URL (E-40-07).
  - A numeric term matches source ids (E-40-04).

### T-40a-025 — Extensions updates badge and update-all button
- Priority: P1
- Effort: S
- Depends: T-40a-032, T-40a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsTab.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt
- Acceptance:
  - Tab badge shows `ext_updates_count` only when greater than zero (B§5.12).
  - The "updates pending" header carries the `ext_update_all` button (B§5.6).
  - Badge in the main nav is driven by this count (replaces the constant "1" in `src/main.rs:175`).

### T-40a-026 — Package-install permission banner (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt
- Acceptance:
  - Drop decision recorded: the `ext_permission_install_apps_warning` banner and the `requiresSystemPermission` check are not implemented on Linux (D2 in p1-behavior section 16).
  - The screen model field `installer` is kept as `None` for parity and never shown.

### T-40a-027 — Extension icons (Available via URL, Installed from bundle, Untrusted glyph)
- Priority: P1
- Effort: M
- Depends: T-40a-006, area:18-extensions-manga, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaIcons.kt
- Acceptance:
  - Available icons fetched from `iconUrl` with a 0x1F888888 placeholder and a bundled error glyph (D6, D8).
  - Installed icons read from the extension bundle at the requested size (40 px rows, 112 px details) (D7).
  - Untrusted extensions show the dangerous glyph in error colour (B§5.8).
  - Missing icon falls back to the default source icon without an error dialog.

### T-40a-028 — Long-press semantics and private extension uninstall confirm
- Priority: P1
- Effort: S
- Depends: T-40a-017, T-40a-029, T-40a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsTab.kt
- Acceptance:
  - Long-press on Available installs; on Installed or Untrusted: uninstall directly if the package is installed, else show the confirm (B§5.9).
  - Confirm dialog: title `ext_confirm_remove`, message `remove_private_extension_message` with name, confirm `ext_remove`, cancel `action_cancel` (B§5.9).
  - Right-click shows the same actions in a context menu (U§7.3).

### T-40a-029 — Shared dialogs: remove-entry and generic confirm
- Priority: P1
- Effort: S
- Depends: T-40a-030
- Sources: app/src/main/java/eu/kanade/presentation/browse/BrowseSourceDialogs.kt
- Acceptance:
  - Remove dialog: title `are_you_sure`, message `remove_manga` with entry title, confirm `action_remove` (confirm runs after dismiss), cancel `action_cancel` (U§S19 in p1-ui-parity.md).
  - Built on `gtk::AlertDialog`; Escape cancels.

### T-40a-030 — Loading, empty, error and toast primitives
- Priority: P0
- Effort: S
- Depends: T-40a-033
- Sources: app/src/main/java/eu/kanade/presentation/browse/BrowseSourceLoadingItem.kt, app/src/main/java/eu/kanade/presentation/browse/GlobalSearchResultItems.kt
- Acceptance:
  - Spinner row: 16 px vertical padding, centred; per-row spinner 16 px with 2 px stroke (D§13).
  - Empty-screen widget: centred message plus action buttons (max three), with accessible names.
  - Toast overlay: bottom centre, 3 s default, Indefinite option with action, dismiss button.
  - Warning banner for obsolete extensions (not closable).

### T-40a-031 — Extension app-info button (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt
- Acceptance:
  - Drop decision recorded (D1, D10): the app-info button (shown only for shared Android packages) is not implemented.
  - The details page never shows it; `is_shared` stays a data field for parity.

### T-40a-032 — Browse preferences: typed config with upstream keys and defaults
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt, app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/MangaMigrationFlags.kt
- Acceptance:
  - `BrowsePrefs` load/save to `browse.toml` with defaults from D§5: `display_mode = COMPACT_GRID`, `enabled_languages = all,en,<system>`, `migrate_flags = i32::MAX`, sort `ALPHABETICAL`, direction `ASCENDING`, `show_nsfw_sources = true`.
  - Unknown enum or flag values fall back to defaults (TC-40-031).
  - Unknown keys survive a save (TC-40-032).
  - Id sets are stored as strings (`hidden_sources`, `pinned_sources`).
  - Atomic write through a temp file and rename.

### T-40a-033 — Display names, language ordering and labels
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - `source_display_name` handles `last_used`, `pinned`, `other`, `all` with the four labels (R§3, TC-40-029).
  - `localized_display_name` maps `zh-CN` to `zh-Hans` and `zh-TW` to `zh-Hant` (TC-40-028).
  - `compare_languages` puts `all` first, is reflexive, and is a total order (TC-40-010).
  - `C` and `POSIX` system locales map to `en` for default languages (E-40-102).

### T-40a-034 — Kurobayomi browse route table and mockup replacement
- Priority: P0
- Effort: M
- Depends: T-40a-001, T-40a-007, T-40a-017
- Sources: src/entry.rs (Kurobayomi mockup), spec/entries.json (Kurobayomi), src/main.rs (Kurobayomi)
- Acceptance:
  - `BrowseRoute` enum (R§12) replaces the string routes for manga browse.
  - Static builders `sources`, `extensions`, `migrate_sources`, `browse_source`, `global_search`, `migrate_entries` in `src/entry.rs` take model snapshots; no hard-coded "Source A" or "Extension A" rows remain (G1, G3, G9, G11, G12).
  - Anime branches are left to the anime areas; the manga branch is the only one changed here.
  - No sample data is added for empty states (user rule: no invented scaffolding).

### T-40a-035 — Keyboard map for browse pages
- Priority: P1
- Effort: S
- Depends: T-40a-034
- Sources: src/entry.rs (Kurobayomi mockup)
- Acceptance:
  - Ctrl+F focuses search; Escape closes search then navigates back; F5 refreshes the extension list; Ctrl+1..6 select Browse sub-tabs (U§9).
  - Migrate dialog: Ctrl+Enter migrates, Ctrl+Shift+Enter copies (U§9).
  - Every shortcut is listed in the help overlay (area 36) and is rebindable through the shared key table (area 02).

### T-40a-036 — Context menus and right-click for rows
- Priority: P1
- Effort: M
- Depends: T-40a-017, T-40a-007, T-40a-004
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaSourceList.kt
- Acceptance:
  - Right-click on a source row opens the source options menu (T-40a-008 actions) (U§4.3).
  - Right-click on an extension row opens the row actions (U§7.3).
  - Right-click on a manga cell opens the long-press action of that cell.
  - Context menus close on Escape and on selection.

### T-40a-037 — Responsive layout for browse pages
- Priority: P1
- Effort: M
- Depends: T-40a-034
- Sources: src/entry.rs (Kurobayomi mockup), src/clamp.rs (Kurobayomi mockup)
- Acceptance:
  - Phone width (360 px) has a 16 px gutter and no horizontal page scroll; only chips and card rows scroll horizontally (U§11, TC-40-119).
  - Grid column count follows the library portrait/landscape preference; `0` means adaptive 128 px minimum; landscape means width >= height (D§4).
  - Wide layouts clamp content width (reuse `src/clamp.rs` if it fits).

### T-40a-038 — Pure-logic unit tests for this area
- Priority: P0
- Effort: M
- Depends: T-40a-024, T-40a-033, T-40a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/MangaMigrationFlags.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaExtensionDetailsScreenModel.kt
- Acceptance:
  - Test IDs TC-40-001 to TC-40-032 exist and pass with `cargo test`.
  - Each test has at least one assertion that fails if its logic is broken (checked by a deliberate mutation pass).
  - No test needs GTK or a display server.

### T-40a-039 — GTK and integration fixtures for browse
- Priority: P1
- Effort: M
- Depends: T-40a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - Fixture files from p1-edge-cases-and-tests.md section 4 exist under `tests/fixtures/browse/`.
  - Fixtures use generic names only (no real titles or covers).
  - TC-40-101 to TC-40-120 are runnable against a fake repository layer.

### T-40a-040 — Message key lint for browse UI
- Priority: P1
- Effort: S
- Depends: T-40a-033
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt
- Acceptance:
  - Every key in p1-behavior.md section 18 exists in the Kurobayomi i18n table; a build check fails on a missing key (TC-40-121).
  - Keys added by Kurobayomi (`action_refresh`, `copied`, `migrate_tracks_always`, `same_manga`, `copy_debug_info`) are defined in the table.

### T-40a-041 — Migration atomicity and ordering fixes
- Priority: P1
- Effort: L
- Depends: T-40a-016, area:03-db-manga-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - DB steps (chapters, categories, tracks, favorite flip) run in one SQLite transaction (E-40-85, E-40-86).
  - Favorite flip for the new manga happens before the old manga is un-favorited in that transaction (E-40-86).
  - File operations (downloads delete, cover copy) run after commit; their failure is logged and reported as a warning (D-UI-4).
  - The cover input stream is closed (B§15 step 7).

### T-40a-042 — Data saver row (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt
- Acceptance:
  - Drop decision recorded (D11): the SY data-saver exclusion row is not implemented; the callback parameter is absent in Kurobayomi.
  - If the network layer later has data saver (area 12), a P2 follow-up is opened.

### T-40a-043 — Fragment-based preference host (drop of the implementation)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt
- Acceptance:
  - Drop decision recorded (D4): `FragmentContainer`, `PreferenceFragmentCompat`, and reflection on `onContainerAvailable` are not ported.
  - Feature parity is delivered by T-40a-023.

---

## P1 and P2: parity and polish

### T-40a-044 — Copy-to-clipboard helpers and feedback
- Priority: P2
- Effort: S
- Depends: T-40a-030
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MigrateMangaSourceScreen.kt
- Acceptance:
  - Clipboard write uses `gdk::Clipboard` (no label parameter on Linux).
  - Debug copy and source-id copy show a toast "Copied" (D-UI-3, D-UI-5).

### T-40a-045 — Migration disclosure line for always-copied tracks
- Priority: P1
- Effort: S
- Depends: T-40a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - A dim line below the checkboxes states that tracking data is always transferred (key `migrate_tracks_always`) (U§6.5, D-UI-9).
  - The line is not a checkbox and does not change the flag value.

### T-40a-046 — Extension update count from the extension manager
- Priority: P1
- Effort: S
- Depends: T-40a-032, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `ext_updates_count` is written by the extension manager and read by the Browse badge and tab badge (B§5.12).
  - Count 0 hides the badge.

### T-40a-047 — Extension install and removal confirm: installed-package check
- Priority: P1
- Effort: S
- Depends: T-40a-017, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsTab.kt
- Acceptance:
  - `isPackageInstalled(pkg)` is replaced by the local registry lookup `has_installed(pkg)` (D3).
  - The decision between direct uninstall and confirm matches B§5.9.

### T-40a-048 — Remove dead GlobalSearchCard if unused
- Priority: P2
- Effort: S
- Depends: T-40a-004
- Sources: app/src/main/java/eu/kanade/presentation/browse/GlobalSerachCard.kt
- Acceptance:
  - Verified by grep across the Aniyomi source (no callers in manga or anime presentation) before deletion in the Kurobayomi port.
  - If a caller exists, port it under T-40a-011 instead.

### T-40a-049 — Cover-only grid mode
- Priority: P2
- Effort: S
- Depends: T-40a-004, T-40a-002
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/BrowseMangaSourceScreen.kt
- Acceptance:
  - `CoverOnlyGrid` renders covers without titles (new behaviour, not parity; upstream falls through to compact grid, B§2.6).
  - The mode is offered in the display-mode menu only when the library also offers it.

### T-40a-050 — Browse tab re-select opens matching global search
- Priority: P2
- Effort: S
- Depends: T-40a-001, T-40b-065
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt
- Acceptance:
  - Reselect on manga pages opens manga global search; anime pages open anime global search (D-UI-1).
  - Documented as a deliberate deviation for the upstream comment at BrowseTab.kt:51 that marks this as unfinished.

### T-40a-051 — Migrate same-manga guard
- Priority: P2
- Effort: S
- Depends: T-40a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - Migrating a manga onto itself (old id == new id) is rejected with `same_manga` (E-40-88) and writes nothing.

### T-40a-052 — Migrate dialog double-submit guard
- Priority: P1
- Effort: S
- Depends: T-40a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt
- Acceptance:
  - Copy and migrate buttons are disabled as soon as one migration starts (E-40-90).
  - A second request while one runs is ignored.

### T-40a-053 — Source row and stub handling for missing extensions
- Priority: P1
- Effort: S
- Depends: T-40a-006, T-40a-007
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaIcons.kt
- Acceptance:
  - Stub sources (uninstalled extensions with library entries) show a not-installed label and warning icon (B§11, E-40-53).
  - Their browse page shows the stub screen (T-40a-003).

### T-40a-054 — Global search and migrate search error and cancellation
- Priority: P1
- Effort: S
- Depends: T-40a-011, T-40a-014
- Sources: app/src/main/java/eu/kanade/presentation/browse/manga/GlobalMangaSearchScreen.kt
- Acceptance:
  - Leaving global search cancels in-flight source searches (tokio abort handles, R§5).
  - A stale result for an earlier query is dropped (generation counter).

---

## Coverage note

- Every file in the scope list (p1-inventory.md) has at least one task ID (see the Task IDs column there).
- Every behaviour section B§1 to B§19 has a task.
- P3 drop tasks: T-40a-026, T-40a-031, T-40a-042, T-40a-043. Each records the reason in p1-behavior.md section 16.
