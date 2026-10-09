# Area 41 (part 1): UI parity spec (anime browse, extensions, migration)

Every visible element of the 36 scope files, its states, dialogs, menus, snackbars, gestures, keyboard/mouse equivalents, responsive behavior, and the gap list versus the existing mockup. Detailed logic is in p1-behavior.md; this file is what the screen looks like and how it is operated. Element names use the Kotlin identifiers so they can be cross-checked.

## 1. Screen inventory

| # | Screen (Kotlin) | Entry point | Kurobayomi page (BrowsePage) | Parity status |
|---|---|---|---|---|
| S1 | Sources list (`AnimeSourcesScreen`) | Browse tab, Sources sub-tab (part 2 tab) | `Sources` | P0 |
| S2 | Source filter (`AnimeSourcesFilterScreen`, presentation) | Browse toolbar Filter button | `SourceFilter` | P1 |
| S3 | Browse source (`BrowseAnimeSourceContent` in `BrowseAnimeSourceScreen`) | tap source row / Latest | `BrowseSource` | P0 |
| S4 | Global search (`GlobalAnimeSearchScreen`) | Browse toolbar search icon | `GlobalSearch` | P1 |
| S5 | Extensions list (`AnimeExtensionScreen`) | Browse tab, Extensions sub-tab | `Extensions` | P0 |
| S6 | Extension filter (`AnimeExtensionFilterScreen`) | Extensions overflow > Filter | `ExtensionFilter` | P1 |
| S7 | Extension details (`AnimeExtensionDetailsScreen`) | tap installed extension | `ExtensionDetails { pkg_name }` | P1 |
| S8 | Source preferences (`AnimeSourcePreferencesScreen`) | details row gear icon, or browse overflow Settings | `SourcePrefs { source_id }` | P1 |
| S9 | Migrate source picker (`MigrateAnimeSourceScreen`) | Browse tab, Migrate sub-tab | `MigrateSourcePicker` | P1 |
| S10 | Migrate favorites (`MigrateAnimeScreen`) | tap source in S9 | `MigrateFavorites { source_id }` | P1 |
| S11 | Migrate search (`MigrateAnimeSearchScreen`) | tap favorite in S10 | `MigrateSearch { anime_id }` | P1 |
| S12 | Migrate source search (`AnimeSourceSearchScreen`) | tap a source header in S11 | `MigrateSourceSearch` | P1 |
| S13 | Migrate seasons (`MigrateSeasonSelectScreen`) | "Show seasons" in dialog D2 | `MigrateSeasons` | P1 |
| D1 | Confirm migrate dialog (`MigrateAnimeDialog`) | tap result card in S11/S12/S13 | dialog | P1 |
| D2 | Extension remove confirm (`AnimeExtensionUninstallConfirmation`) | long-press non-available row that is not on disk | dialog | P2 |
| D3 | Extension trust dialog (`ExtensionTrustDialog`, shared) | tap untrusted row / Trust button | dialog | P0 |
| D4 | NSFW age warning (`NsfwWarningDialog`, shared) | tap "18+ ⓘ" in S7 | dialog | P1 |
| D5 | Source options (`AnimeSourceOptionsDialog`) | long-press source row in S1 | popover | P0 |
| D6 | Display mode radio menu (`BrowseAnimeSourceToolbar`) | display-mode toolbar button | popover menu | P0 |
| D7 | Migrate-search filter dialog (`SourceFilterAnimeDialog`, part 2) | FAB in S12 | dialog (part 2) | P1 |

Screen-level chrome common to all: header bar with back button (except root tabs), title, and actions as listed per screen.

## 2. S1 Sources list

Layout: vertical list, top padding 8 dp (`topSmallPaddingValues`), grouped by language.

- Header row per group: language display name (`header` typography: GTK `.heading`-like label), padding 16 horizontal, 8 vertical. Non-selectable.
- Source row: 40x40 icon (AnimeSourceIcon, behavior 4), name (one line, ellipsis), language line dimmed (alpha 0.78) when `lang != ""`, "Not installed" in error colour for stubs. Right side: `Latest` flat text button (only when `supportsLatest`), then pin toggle icon button (filled pushpin in accent colour when pinned; outline, dimmed to 78% alpha when not).
- Row activation (click) -> Popular listing. Latest button -> Latest listing. Pin button -> toggle pin without leaving the page.
- Long-press (touch) / right-click / `Menu` key -> D5 options popover.
- States:
  - Loading: centered spinner (part 2 model drives it).
  - Empty: "No source found" with no actions.
- Empty-language groups never appear.

## 3. S2 Source filter

- Title "Sources". Empty: "No installed source found".
- Rows: language header row with switch (title = language display name, state = enabled language). Under an enabled language: one row per source with name and language hidden (showLanguageInContent false), trailing checkbox (checked = not hidden).
- Clicking a language switch hides or shows that language's sources; does NOT change their checkboxes.
- Keyboard: Up/Down move, Space toggles the focused switch/checkbox.

## 4. S3 Browse source

Layout: header bar (back, title = source name, search toggle, display-mode button, overflow menu), then content.

Toolbar (behavior 7.2):
- Search: a search bar slides under the header (Ctrl+F). Enter submits; Esc closes search and clears query (`onClickCloseSearch = navigateUp` in Kotlin; desktop: Esc closes search first, a second Esc goes back).
- Display mode button icon: list icon when List mode, grid icon otherwise. Popover radio: "Comfortable grid", "Compact grid", "List" (CoverOnlyGrid not offered).
- Overflow: "Open in WebView" (or "Help" for the local source), "Settings" when configurable.

Content:
- Error-only state (no items, refresh or append error): centered icon + error message + buttons in this order: "Retry", "Open in WebView", "Help" (or only "Local source guide" for the local source).
- Refresh error with items present: bottom info banner with message and "Retry" button that does not auto-hide (`Indefinite`). Dismiss button closes it.
- Loading (no items): centered spinner.
- Grid (comfortable): cover aspect and title placement from `EntryComfortableGridItem` (area:37), title max 2 lines by default, in-library badge top-start (bookmark icon) on favorites, favorite covers dimmed to 34 % opacity. Spacing 4 dp. Outer padding 8 dp.
- Grid (compact / cover-only): cover-first grid built on `EntryCompactGridItem` (area:37 defines the title placement on the cover); same badge and favorite dimming as comfortable.
- List: row per item, cover size from `EntryListItem` (area:37), title, badge; height fits the container (`containerHeight - topBarHeight`).
- Prepend/append loading: a full-width spinner row (16 dp vertical padding).

Keyboard/mouse: arrow keys move focus in the grid; Enter opens details (AnimeScreen); Shift+Enter or middle-click opens in background is NOT supported (Kotlin has no equivalent). Right-click or Menu key -> long-click action (AnimeScreen with fromSource = true, Kotlin `onAnimeLongClick`).

## 5. S4 Global search

Layout: header bar with a search entry (title hidden), progress bar at its bottom while `1 <= progress < total`, chips row, then results.

Chips row (in order): "Pinned" (PinnedOnly, pushpin icon), "All" (DoneAll icon), divider, "Has results" (filter icon; persistent toggle). Chips are toggle buttons with pill style; selected = filled accent.

Results: one block per source, in model order (behavior 8.4 sort: sources with results first, then pinned, then name):
- Block header: source name (or "▶ name" for the migrate origin, behavior 8.1), subtitle = localized language (`getLocalizedDisplayName`), trailing nothing.
- Loading: spinner row.
- Success with results: horizontal row of 96 dp cards (cover, title max 3 lines, in-library badge).
- Success with zero results: text "No results found" (padding 16/8).
- Error: text with the throwable message (plain, no retry).
- Clicking a block header opens the source's Browse page with the same query.

States: no query -> empty content (no placeholder text in Kotlin; Proposal: show "Search a source by title" hint in the dim label style; flagged as addition).

## 6. S5 Extensions list (most used page; detailed)

Header bar: title "Anime Extensions" (badge with the number of pending updates, shown in the tab button as a badge), overflow menu ("Filter", "Extension stores"), refresh button (Proposal, replaces pull-to-refresh; keyboard F5).

Search: search bar (same as S3), comma-separated OR terms (behavior 1.4). Shows the typed text as-is.

Sections (behavior 1.3), each a header row:
1. "Updates pending" header with a filled "Update all" button on the right (text colour `onPrimary`). Hidden when there are no updates.
2. "Installed" header (contains installed items then untrusted items). Hidden when both empty.
3. One header per language (display name), ordered by the language comparator ("all" first).

Row anatomy (behavior 2.4):
- Left: 40x40 icon. While installing: a 40 dp spinner under the icon, icon padded 8 dp.
- Middle: name (one line, ellipsis). If torrent: a magnet glyph before the name, the text "(Torrent)" is an accessibility label only. Subline (dimmed 78 %): language (installed only), version, then one warning in red uppercase ("UNTRUSTED", "ORPHANED", "18+"), then install step text ("Pending", "Downloading", "Installing") after a dot separator.
- Right: action buttons per the table in behavior 2.4.

Row click = primary action (install / open details / trust). Row secondary buttons are individually clickable.

States:
- Loading (first load): centered spinner.
- Empty with no search: "Well, this is awkward" plus "Extension stores" action button.
- Empty with search: "No results found" plus "Extension stores" button.
- Permission banner: absent on Linux (D2).
- Refreshing: spinner in the header refresh button (replaces pull indicator), disabled during loading.
- Network error on refresh: banner "Failed to fetch available extensions" with "Retry" (Proposal).

Long-press / right-click:
- Available -> start install immediately (no confirm; parity).
- Installed or Untrusted not on disk -> D2 confirm "Remove Extension?" with "Remove" (destructive style) and "Cancel".
- Installed on disk -> uninstall immediately (parity with `isPackageInstalled == true` branch). Proposal: this skips confirm on purpose in Kotlin; keep parity, but show the undo-less banner "Uninstalled <name>".

Keyboard: Up/Down traverse rows; Enter = primary action; Shift+F10 / Menu key = long-press; Delete = uninstall (Proposal, not in Kotlin, flagged). Ctrl+U = Update all (Proposal).

## 7. S6 Extension filter

Title "Extensions". Rows: language switch rows (enabled languages first, then comparator order). Empty: "Well, this is awkward" (`empty_screen`) for the empty filter list (no search box on this page). Error: toast-equivalent banner "InternalError: Check crash logs for further information".

## 8. S7 Extension details

Header bar: back, title "Extension info", open-repo icon button (external link icon, only when a store URL exists), overflow: "Enable all", "Disable all", "Clear cookies".

Content (scroll):
1. Warning banner (orphaned only): "This extension is not associated with any store. It will not receive updates, may not function properly, and can cause issues with the app. Uninstalling it is recommended." (amber/red banner, no action).
2. Hero block (tap = copy debug info to clipboard; a transient "Copied" banner, Proposal): 112 dp icon, name (headline), stripped package name (dim).
3. Info row (three cells separated by vertical dividers): "Version" | "Language" | "18+ ⓘ" (age rating; only when NSFW; tap opens D4).
4. Button row: "Uninstall" (outlined, full width half) | "Open folder" (filled, replaces "App info"; only when the extension dir exists).
5. Incognito switch row: icon (glasses), title "Incognito mode", subtitle "Pause reading history for extension", switch.
6. Divider.
7. Source rows (sorted: enabled first, then name/language): title (`Name (LANG)` when labelAsName, else localized language), trailing gear (if configurable) and a read-only switch; activating the row toggles the source.

States: while loading -> spinner. Extension missing -> "Well, this is awkward".

## 9. S8 Source preferences

Header: title = source `toString()` (e.g. `Name (EN)`), back. Body: native settings page (Proposal, D4) with groups from the source's preference declarations. Widgets: switch rows, entry rows (EditText), list rows (dropdown-like `gtk::DropDown`), multi-select (dialog with checkboxes), and action rows. Empty (non-configurable) source: "Well, this is awkward" (EmptyScreen parity). Full widget list is area:16-owned; this page must render at least: switch, entry, list, multi-select, and info text.

## 10. S9 Migrate source picker

Header: title "Migrate" (Kotlin: part of tab; Proposal title "Migrate sources"). Sticky header row in the list (not scrolling away): text "Select a source to migrate from" (header style), then two icon buttons: sort mode (alphabetical icon, or count icon), direction (up arrow for ascending, down arrow for descending). Tooltips: "Alphabetically" / "Total entries"; "Ascending" / "Descending".

Rows: 40x40 icon, name (or id when blank), language dimmed (if non-empty), count badge on the right ("12"), "Not installed" in error colour for stubs. Click = open favorites (S10). Long-press / right-click = copy source id to clipboard (no feedback in Kotlin; Proposal: transient "Copied source id" banner).

States: loading spinner; empty -> "Your library is empty" (AYMR reuse, behavior 13.2) — flagged as a text bug in Kotlin: the empty migrate list says library empty; Proposal: "No source has favorites to migrate" (new key, T-41a-036 Acceptance).

## 11. S10 Migrate favorites

Header: title = source name, back. List rows: `BaseAnimeListItem` (cover thumbnail left, title, subtitle with dimmed state). Click row -> S11 search for that anime. Click cover -> open entry details (AnimeScreen, area:39). Sorted case-insensitively. Empty -> "Well, this is awkward". Failure -> banner "InternalError...".

## 12. S11 Migrate search

Header: search bar (query seeded with the old anime's title), progress bar, chips ("Pinned" default selected, "All", "Has results"), results block list (behavior 8.1). Origin source block shows "▶ <name>" and is listed first (behavior 15). Clicking a block header opens S12 for that source. Clicking a card opens D1. Long-press card -> entry details with fromSource = true.

## 13. S12 Migrate source search

Header: search toolbar; FAB "Filter" (extended, bottom-right, visible only when the source has filters). Body = the same browse content as S3 (`BrowseAnimeSourceContent`). Its toolbar is a plain `SearchToolbar` in Kotlin (`AnimeSourceSearchScreen.kt`): no display-mode button and no overflow menu. Kurobayomi keeps that exact toolbar. Click card -> D1. Long-press -> entry details. After a successful migrate: window pops to root, switches to Browse tab, opens the new entry (behavior 11.2).

## 14. S13 Migrate seasons

Header: title = the target (parent) anime title, back. Body: season grid (covers + titles, column count from prefs; default adaptive 128 dp). Click a season -> D1 with `newAnime = season`. Long-press -> entry details (fromSource true). Dialog dismiss only (no navigation after migrate, behavior 11.2).

## 15. D1 Migrate confirm dialog

Modal dialog (`gtk::Window` transient for main, or `gtk::AlertDialog` with custom child; Proposal: custom `gtk::Window` to host checkboxes).

Content:
- Title: "Select data to include".
- Checkboxes (when `canMigrate`): one per flag in order: "Episodes" (anime with episode fetch type only), "Categories", "Custom cover" (if any), "Custom background" (if any), "Delete downloaded" (if downloads exist). Pre-selected per the saved bitmap.
- Error row (when `!canMigrate`): red icon + red text "Unable to migrate season to episode" or "Unable to migrate episode to season, select season to continue".

Buttons, left to right: "Show anime" (always), "Show seasons" (when the target is a Seasons parent), spacer, "Copy" (when canMigrate), "Migrate" (when canMigrate, suggested-action style).

Running state: the dialog body is replaced by a centered spinner over a 70 % background; buttons hidden; cannot be dismissed while running (Escape ignored).

Failure (Kurobayomi, Proposal): spinner replaced by red text "Migration failed: <reason>" and "Close" button; nothing was written (transaction rolled back, behavior 6).

Done: dialog closes, navigation per behavior 11.2.

## 16. D2 / D3 / D4 confirm dialogs

- D2 "Remove Extension?" / body `Do you really want to remove "<name>" extension?` / buttons "Cancel" (default focus) and "Remove" (destructive).
- D3 "Untrusted extension" / body (verbatim: "Malicious extensions can read any stored login credentials or execute arbitrary code.\n\nBy trusting this extension, you accept these risks.") / buttons: "Trust" (suggested), "Uninstall" (destructive). Dismiss (Esc, click outside) = close without action.
- D4 NSFW: body "Sources from this extension may contain NSFW (18+) content" / button "OK". Dismiss = same as OK.

## 17. D5 Source options popover

Anchored to the row. Items: "Pin" or "Unpin" (pin toggles), "Disable" (hidden for the local source). Title row shows `visualName` (bold). Activation closes the popover then applies.

## 18. D6 Display mode radio menu

Popover radio list: "Comfortable grid", "Compact grid", "List". Current mode checked. Changing closes the menu and re-lays out the browse content immediately (no reload of data).

## 19. Snackbars, toasts and banners

| Message key | Text | Kind | Trigger | Auto-hide |
|---|---|---|---|---|
| `internal_error` | "InternalError: Check crash logs for further information" | toast | filter/favorites load failure | 4 s |
| `extension_api_error` | "Failed to fetch available extensions" | toast (Proposal: banner with Retry) | refresh failure | 4 s |
| load error (paging) | error message text + "Retry" | snackbar | browse append/refresh failure with items present | never (Indefinite) |
| `source_not_installed` | (screen body) "Source not installed: <name>" | empty state | stub source open | n/a |
| copy source id / debug info | "Copied" (Proposal) | banner | long-press / hero tap | 2 s |
| migration failure (Proposal) | "Migration failed: <reason>" | dialog body | error in engine | n/a |
| uninstall (Proposal) | "Uninstalled <name>" | banner | uninstall | 4 s |

## 20. Gestures

| Gesture | Where | Action |
|---|---|---|
| Tap | any row | primary action (table in section 6) |
| Long-press (touch) / right-click | extension row | install / uninstall / remove-confirm (section 6) |
| Long-press / right-click | source row (S1) | options popover (D5) |
| Long-press / right-click | browse card (S3, S11, S12, S13) | entry details (fromSource true) |
| Long-press / right-click | migrate source (S9) | copy id |
| Tap hero block | S7 | copy debug info |
| Tap "18+ ⓘ" | S7 | D4 |
| Pull down | S5 | refresh (touch; desktop uses button and F5) |
| Swipe (touch) | none | none in Kotlin; none here |
| Scroll wheel with Ctrl | browse grids | zoom columns (Proposal; not in Kotlin) |

Desktop mouse: hover highlight on rows (GTK default). Tooltips on all icon-only buttons with the same text as the Kotlin content description (table in p1-behavior.md section 24).

## 21. Keyboard map (Proposal for Kurobayomi, all pages in this part)

| Keys | Action |
|---|---|
| Ctrl+F | focus search (opens search bar) |
| Esc | close search bar / dialog / popover; on bare page go back |
| Alt+Left, Backspace (when not in an entry) | back |
| F5, Ctrl+R | refresh (S5, S1) |
| Ctrl+U | Update all (S5) |
| Ctrl+1..Ctrl+6 | switch Browse sub-tabs (area:40 defines tab order) |
| Enter | primary action on focused row or card |
| Shift+F10, Menu | long-press action |
| Space | toggle switch/checkbox on focused row |
| Ctrl+Shift+C | copy debug info (S7) |

## 22. Responsive behavior

- Window width < 600 px: list rows keep icon + name + actions; warnings line wraps; chip row scrolls horizontally; grids use min 2 columns.
- Width 600..900 px: default layout.
- Width >= 900 px: extension list centred with max width 960 px; details page max content width 640 px.
- Cover grid column count: from prefs (`pref_animelib_columns_landscape_key` when width >= 720 px, else `..._portrait_key`); 0 = adaptive 128 dp (parity).
- Minimum supported width: 360 px (phone-like tiling; text ellipsises, never clipped).

## 23. Text, accessibility and i18n

- All strings come from the MR/AYMR English base (p1-behavior.md section 24). Translations: Kurobayomi uses Fluent or gettext files generated from those keys (T-41a-045).
- Icons without visible text carry `accessible-name` / tooltip with the Kotlin contentDescription text.
- Colour is never the only signal: warnings are uppercase text, not colour alone.
- Focus ring visible on all rows and buttons (GTK default theme).
- The theme is the user's GTK theme (KDE Plasma with Tokyo Night per user memory); no hard-coded colours, use `@theme_fg_color`-style CSS names only through GTK classes (`.dim-label`, `.error`, `.accent`, `.pill`, `.flat`, `.suggested-action`, `.destructive-action`).

## 24. Animations

- Kotlin animates the install-icon padding from 8 dp to 0 dp (`animateDpAsState`, `AnimeExtensionsScreen.kt:311-314`). Kurobayomi: instant change, no animation (documented loss, D9).
- Cross-fade between display modes: `gtk::Stack` crossfade (150 ms) is acceptable parity.

## 25. Gap list: existing Kurobayomi mockup vs this spec (`src/entry.rs`, `src/ui.rs`, `src/settings.rs`)

| # | Gap | Where in mockup | Spec reference | Task |
|---|---|---|---|---|
| G1 | Source sections are hard-coded ("Last used", "Pinned", "English", "Other") with placeholder names "Source A/B/C/D" | `src/entry.rs:283-309` | S1, behavior 6 | T-41a-001 |
| G2 | Latest and row-activate always open `browse_source` with no source id | `src/entry.rs:291,299` | S1, S3 | T-41a-001, T-41a-005 |
| G3 | Pin ToggleButton not bound to model; no unpin/disable options | `src/entry.rs:293-294` | D5, behavior 6 | T-41a-002 |
| G4 | Right-click options dialog is a dialog stub (`dialog_of("browse","source_options")`) not a popover with pin/disable | `src/entry.rs:301` | D5 | T-41a-002 |
| G5 | Filter button closure has dead code and always opens `sources_filter` | `src/entry.rs:266-272` | S2 | T-41a-031 |
| G6 | Extensions list is hard-coded `("", vec![("Extension A", "1.4.13", "Update")])` | `src/entry.rs:311-330` | S5 | T-41a-010 |
| G7 | No install/cancel/retry states, no progress spinner | `src/entry.rs:311-330` | 1.11, S5 | T-41a-015 |
| G8 | No untrusted state, no trust dialog | (absent) | D3 | T-41a-012 |
| G9 | No long-press uninstall/install semantics | (absent) | S5 | T-41a-011 |
| G10 | No "Update all" button or updates section header | (absent) | 1.7, S5 | T-41a-014 |
| G11 | No search on extensions list | `src/entry.rs:311` | S5 | T-41a-009 |
| G12 | Migrate source picker: sort header missing; counts hard-coded | `src/entry.rs:360-378` | S9 | T-41a-036 |
| G13 | Browse source (`browse_source`) shows no grid/list states, no error/empty/loading, no display mode | `src/entry.rs:380-426` | S3 | T-41a-005, T-41a-006 |
| G14 | Season page (`seasons()`) is a stub | `src/entry.rs:427-431` | S13 | T-41a-040 |
| G15 | Global search has no progress bar, no chip semantics, no origin marker | `src/entry.rs:432-481` | S4, S11 | T-41a-027, T-41a-028, T-41a-030 |
| G16 | Migrate entries page (`migrate_entries`) has no favorites load or click semantics | `src/entry.rs:482-497` | S10 | T-41a-037 |
| G17 | Migrate dialog (`dialog_of(id, "migrate_anime")`) has no flags, no canMigrate rule, no running state | `src/entry.rs:230` | D1 | T-41a-035 |
| G18 | Extension stores link opens an unrelated settings stub | `src/entry.rs:277`, `src/settings.rs:13` | section 1 (out of scope, area:19) | T-41a-053 |
| G19 | No empty states, no "Extension stores" empty action | (absent) | S5 | T-41a-010 |
| G20 | No desktop keyboard map (Ctrl+F, F5, Esc) | (absent) | section 21 | T-41a-044 |
| G21 | `ui::open` takes string ids; no typed pages, no pop-until | `src/ui.rs:88` | rust design 4.4 | T-41a-005 |
| G22 | No toast/snackbar banner primitive | (absent) | section 19 | T-41a-045 |
| G23 | Global search page not wired to a model; `global_search(id, migrating)` ignores the id | `src/entry.rs:432` | S4 | T-41a-027 |
| G24 | Source preferences are a placeholder cards page (`settings_anime_extension_stores`, `cards`) | `src/settings.rs:13` | S8 | T-41a-023 |
| G25 | Widths: 6-tab switcher forced wide; uses scroll only (acceptable) — no width breakpoints for grids | `src/entry.rs:250-262` | section 22 | T-41a-050 |

Items present in the mockup and kept: the 6-tab `gtk::Stack` + `gtk::StackSwitcher` with horizontal scroll (`src/entry.rs:244-262`), the global-search and filter header icons, the `gtk::ScrolledWindow` + `gtk::ListBox` row helpers (`src/ui.rs:102-160`).
