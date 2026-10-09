# Area 40 (part 1): UI parity, manga browse screens

Every visible element, state, dialog, snackbar/toast, menu and gesture of the scope screens, the Linux (GTK4, no libadwaita) equivalent, and what the current Kurobayomi mockup lacks. Upstream sources are repo-relative to `/home/blackbox/aniyomi-src`. Behavioural detail is in p1-behavior.md (section numbers referenced as "B§n").

Conventions: `Proposal:` marks Kurobayomi decisions. Labels are message keys (p1-behavior.md section 18).

---

## 1. Screens in scope

| # | Screen | Entry point | Upstream file (presentation / state) |
|---|---|---|---|
| S1 | Browse tab (six sub-tabs) | bottom/side nav "Browse", index 3 | `ui/browse/BrowseTab.kt` |
| S2 | Browse a source (grid/list, search) | Sources list row click (Popular), Latest button | `presentation/browse/manga/BrowseMangaSourceScreen.kt`, `components/BrowseMangaSource*.kt` |
| S3 | Installed sources list | Browse > Manga Sources | `presentation/browse/manga/MangaSourcesScreen.kt` |
| S4 | Source options dialog | long-press on a source row | `MangaSourcesScreen.kt` (`MangaSourceOptionsDialog`) |
| S5 | Show/hide sources | Sources toolbar "Filter" (part 2 tab action) | `presentation/browse/manga/MangaSourcesFilterScreen.kt` |
| S6 | Global search | Browse search (part 2) | `presentation/browse/manga/GlobalMangaSearchScreen.kt` |
| S7 | Manga extensions list | Browse > Manga Extensions | `presentation/browse/manga/MangaExtensionsScreen.kt` |
| S8 | Extension trust dialog | tap an untrusted extension | `MangaExtensionsScreen.kt` (`ExtensionTrustDialog`) |
| S9 | Private extension uninstall confirm | long-press a non-installed-package extension | `ui/browse/manga/extension/MangaExtensionsTab.kt` |
| S10 | Extension language filter | Extensions overflow "Filter" | `presentation/browse/manga/MangaExtensionFilterScreen.kt` |
| S11 | Extension details | tap installed extension / Settings button | `presentation/browse/manga/MangaExtensionDetailsScreen.kt` |
| S12 | NSFW age-rating dialog | tap NSFW info cell in details | `MangaExtensionDetailsScreen.kt` (`NsfwWarningDialog`) |
| S13 | Source preferences | details source Settings icon | `ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt` |
| S14 | Migrate: pick old source | Browse > Migrate (part 2 tab) | `presentation/browse/manga/MigrateMangaSourceScreen.kt` |
| S15 | Migrate: favorites of old source | pick a source in S14 | `presentation/browse/manga/MigrateMangaScreen.kt` |
| S16 | Migrate: global search for a manga | tap a favorite in S15 | `presentation/browse/manga/MigrateMangaSearchScreen.kt` |
| S17 | Migrate: search one source | tap a source row in S16 | `ui/browse/manga/migration/search/MangaSourceSearchScreen.kt` |
| S18 | Migrate confirm dialog | tap a result in S16/S17 | `ui/browse/manga/migration/search/MigrateMangaDialog.kt` |
| S19 | Source row remove dialog | (shared dialog, used by part 2 / library) | `presentation/browse/BrowseSourceDialogs.kt` |

---

## 2. S1 Browse tab

Visible elements (OBSERVED, B§1):
- Tab bar with six tabs in order: anime sources, manga sources, anime extensions, manga extensions, anime migrate, manga migrate. Tabs are scrollable (`scrollable = true`).
- Tab icon `anim_browse_enter` (animated vector) in the main nav; selected state animates.
- Title `MR.strings.browse`.
- Shared search field in the app bar, bound to the extensions model (B§1). It is shown on the extensions tabs (`searchEnabled = true`).
- Badge on the Extensions tab: the update count (`ext_updates_count`), only when > 0 (B§5.12).

States: none of its own (each sub-tab has states).

Gestures:
- Re-select the Browse tab: opens global search (B§1; upstream always opens anime, see deviation below).

Proposal (Kurobayomi):
- `gtk::Stack` with a header-bar tab strip (`gtk::StackSwitcher`) in one row; horizontal scroll when the window is narrow (`gtk::ScrolledWindow` with `policy: External, Automatic`).
- Tab badge: a `gtk::Label` with CSS class `badge` at the top-right of the Extensions tab button, showing the count; hidden at 0.
- Deviation D-UI-1: re-selecting Browse opens global search for the visible kind (manga or anime) instead of always anime (upstream comment at `BrowseTab.kt:51` marks this as unfinished). Documented in p1-behavior.md section 1.
- Keyboard: `Ctrl+Shift+F` focuses the search entry (global search) on this tab; `Ctrl+1..6` select sub-tabs (see section 9).

---

## 3. S2 Browse a source

### 3.1 Layout (top to bottom)

1. Header bar (`SearchToolbar` equivalent): back button (navigate up), title = source name (`AppBarTitle`), search toggle, display-mode button, overflow menu.
2. Optional progress/snackbar overlay.
3. Content: one of loading, empty-with-error, grid, list.

### 3.2 Header bar details

- Back button: `navigateUp` (Escape closes search first, then back).
- Title: `source.name` (B§2.12); empty when source is null.
- Search: when open, a `gtk::SearchEntry` replaces the title; `on_search_changed` does NOT search (upstream calls `onSearch` on IME action): Proposal: search on Enter (`activate`) and on a 400 ms idle timer; upstream has no idle search, so this is a Proposal beyond parity and can be dropped if the team wants strict parity.
- Display-mode button: icon `view-list-symbolic` when List, else `view-grid-symbolic`. Tooltip / accessible name `MR.strings.action_display_mode`. Click opens a `PopoverMenu` with three radio items (B§2.12): `action_display_comfortable_grid`, `action_display_grid`, `action_display_list`. The selected item shows a check.
- Overflow menu (three-dot, `gio::Menu`):
  - `label_help` (local source only) or `action_open_in_web_view` (other sources).
  - `action_settings` (only when the source is configurable).

### 3.3 Content states

| State | Condition (B§2.2-2.5) | Visual |
|---|---|---|
| Loading (first load) | items == 0 and refresh loading | centred `gtk::Spinner` (32 px) in the content area |
| Loaded | items > 0 | grid or list |
| Loading more (append) | `append` loading | full-width spinner row at the end (`BrowseSourceLoadingItem`, 16 px vertical padding) |
| Loading previous (prepend) | `prepend` loading | full-width spinner row at the top |
| Error with items | refresh or append error and items > 0 | toast-style bar at the bottom with message, action `action_retry` (no auto-hide); dismiss X |
| Error without items | items == 0 and error | empty state with message and actions (below) |
| Empty (no error, no items) | items == 0 and not loading and no error | upstream shows nothing (grid is blank). Proposal: show `EmptyScreen` text `empty_screen` centred. Parity deviation: documented as D-UI-2. |

Empty-with-error actions (B§2.4):
- Local source: one button `local_source_help_guide` (help icon) -> open `LocalMangaSource.HELP_URL`.
- Other sources: three buttons in order: `action_retry` (refresh icon) -> `refresh` (not retry: a new first-page load), `action_open_in_web_view` (globe icon) -> WebView (area 36), `label_help` (help icon) -> `Constants.URL_HELP`.

Stub source (not installed): full screen with message `source_not_installed` (argument: source string) and back button only (B§2.11).

### 3.4 Grids

- Comfortable grid: cell = cover (2:3 portrait, `coverAlpha` 0.34 for favorites) + title below (`EntryComfortableGridItem`, area 37). In-library badge (bookmark icon) at the top-start of the cover.
- Compact grid: cover-first; title overlay (area 37 cell).
- `CoverOnlyGrid` stored preference renders as compact grid (B§2.6); not offered in the menu.
- Cell gap 4 px visually equal to Compose 4.dp at 96 dpi; padding 8 px around the grid.
- Column count: from the library column preference (portrait/landscape), `0` = adaptive 128 px min width. Proposal: landscape = window width >= height.
- Click: open manga (B§2.7 `onMangaClick`). Long click: passed through to the handler the part-2 page supplies as `onMangaLongClick` (B§2.7). This area does not define that handler's action; the migrate search path uses preview (`MangaScreen(id, true)`, B§13.1).

### 3.5 List mode

- Row: cover (height = content height minus top bar, capped by area 37 `EntryListItem` sizing), title, in-library badge (B§2.8).
- List padding 8 px vertical.

### 3.6 Source row (in S3, S5, S14) — `BaseMangaSourceItem`

- Icon 40 px high, aspect 1:1 (`defaultModifier = height(40.dp).aspectRatio(1f)`); stub source shows a red warning triangle; a local source shows the local icon; a source with no icon shows the default icon.
- Name (single line, ellipsis, `bodyMedium`).
- Language under the name (`bodySmall`, secondary alpha), hidden where `showLanguageInContent = false` (S5).
- Action slot: right-aligned.

---

## 4. S3 Installed sources list and S4 options

### 4.1 Layout

- Sticky? No: language headers scroll with rows (`stickyHeader` is used only in S14).
- Header row: language display name, `header` style (bold 16 px on GTK theme), padding 16 px horizontal, 8 px vertical.
- Item row (B§3.2): icon, name, language, then actions: `latest` text button (primary colour; only if `supportsLatest`), pin icon button.

### 4.2 Pin icon

- Pinned: filled pin icon, primary colour. Unpinned: outline pin icon, `onBackground` at 0.5 alpha (`SECONDARY_ALPHA`, value from area 37 `SECONDARY_ALPHA`).
- Accessible name: `action_unpin` when pinned, else `action_pin`.
- Tooltip: same text.

### 4.3 Options dialog (S4)

- Presentation: a `gtk::AlertDialog` is not suitable (no buttons). Use a `gtk::Window` (modal, transient, 320 px wide) with a `gtk::ListBox` of three rows, each full width, 16 px vertical padding:
  1. `action_unpin` or `action_pin` (depending on the current state).
  2. `action_disable` (not shown for the local source, id 0).
- Title: `source.visualName`.
- Dismiss: Escape or click outside (modal window closes on focus loss when `close-on-focus-out`; use `gtk::Window` default: Escape closes).
- Data-saver row: dropped (D11 in p1-behavior.md section 16).

---

## 5. S5 Show/hide sources

- Header bar: back button, title `label_sources`.
- Rows: language header (switch) then source rows (checkbox) for each enabled language.
  - Language switch row: label = language display name, switch on = language enabled.
  - Source row: indented 16 px, icon, name, checkbox = visible (not in `hidden_catalogues`).
- Empty: `source_filter_empty_screen` centred.
- Click on a source row toggles it; the checkbox is display-only (`can_target(false)`).
- Scrolling: the list scrolls; the fast-scroll rail of upstream (`FastScrollLazyColumn`) is not reproduced; GTK's kinetic scroll is enough. Proposal: none.

---

## 6. S14-S18 Migration screens and dialog

### 6.1 S14 Pick the old source

- Header bar: back, title `MR.strings.migrate` (part 2 title; the screen title is set by part 2).
- Sticky sort row (kept fixed above the list, see section 7 of p1-rust-design.md):
  - Label `migration_selection_prompt` (expanding).
  - Mode button: icon `view-sort-ascending-symbolic` for alphabetical (tooltip `action_sort_alpha`), a bundled "123" icon for count (tooltip `action_sort_count`). Click toggles mode.
  - Direction button: `pan-up-symbolic` (tooltip `action_asc`) or `pan-down-symbolic` (tooltip `action_desc`). Click toggles direction.
- Rows (B§11): icon, name (`ifBlank { id }`), language (if non-empty), `not_installed` in error colour for stubs, count badge on the right (`BadgeGroup { Badge(text = count) }`).
- Long-press: copies the source id to clipboard (no toast upstream; Proposal: show toast `copied`? The upstream key is not known; keep silent parity, D-UI-3 optional P2).
- Empty: `information_empty_library`.

### 6.2 S15 Favorites of the old source

- Title: source name (B§12.2).
- Rows: `BaseMangaListItem`-equivalent (area 38/37): cover (click = open manga, `MangaScreen(id)`), title (click = migrate search). Two click targets per row: cover and body.
- Sort: case-insensitive title (B§12.1).
- Empty: `empty_screen`.
- Error: toast `internal_error` plus empty list.

### 6.3 S16 Global search for the manga (migrate)

- Same toolbar and result rows as global search (section 10), plus:
  - Origin source row shows the prefix `"▶ "` on the title (B§10.2).
  - Origin source is first; then pinned; then name (B§13.1).
  - Query prefilled with the manga title and searched immediately (B§13.1).
- Tap a source header (or its arrow) -> S17 with the current query.
- Tap a result card -> S18 dialog (B§13.1 `onClickItem`).
- Long-press a result card -> open manga (preview), B§13.1.

### 6.4 S17 Search one source (migrate)

- Header: search entry (initial text = query), back.
- Body: same grid/list as S2 (B§2), with the migrate-specific routing: tap = S18 dialog; long-press = open manga (preview).
- FAB bottom-right: "Filter" (`action_filter`, filter icon), visible only when the source has filters (`filters.isNotEmpty()`), opens the source's filter dialog (part 2 `SourceFilterMangaDialog`).
- Snackbar host for error messages (same error rules as S2).
- Web view action in the overflow (HttpSource only).

Proposal: FAB becomes a header-bar button with the same label, to avoid a floating overlay on GTK (the menu bar pattern is the desktop norm).

### 6.5 S18 Migrate confirm dialog

Layout (B§14.1):
- Title `migration_dialog_what_to_include`.
- Body: one checkbox per flag in this order: `chapters`, `categories`, `custom_cover` (only if the old manga has a custom cover), `delete_downloaded` (only if the old manga has downloads). Default checked states from the `migrate_flags` preference (default all set).
- Footer (left to right): `action_show_manga` (flat button, closes the dialog and opens the new manga, B§13.2 `onClickTitle`), spacer, `copy` (suggested action = no), `migrate` (suggested action = yes, destructive colour is not used; Migrate removes the old entry from the library, so mark it with the "destructive-action" CSS class to warn, Proposal).
- While migrating: the dialog content is replaced by a centred spinner over a 70% background tint (`background.copy(alpha = 0.7f)`). Buttons are hidden. Escape is blocked while migrating (Proposal: upstream does not dismiss mid-run because the dialog is replaced; keep that).
- After success: the dialog closes and the navigation pops to the new manga (B§13.1 `onPopScreen`, B§13.2).
- After failure (fetch error, no source): the dialog returns to the flag view with no message (upstream silent). Proposal (D-UI-4): show toast `internal_error` for `FetchChapters` and `NewSourceMissing`; this is a deliberate deviation. Keep the dialog open in both cases.

Disclosure (Proposal, required by the engine behaviour): tracks are always copied, regardless of the flag checkboxes (B§15 step 4). Show a dim line under the checkbox list: "Tracking data is always transferred." (new key `migrate_tracks_always`, English text is ours). This makes an upstream hidden behaviour visible.

### 6.6 Navigation after migrate (summary)

- From S18 opened by S17: popUntilRoot, open Browse tab, push new manga details (B§13.2).
- From S18 opened by S16: if the last stack item is the manga details page, pop to it and push a fresh details page for the new id; otherwise replace the top with the new details page (B§13.1).
- Kurobayomi: same rule using the `NavigationStack` (p1-rust-design.md section 7). Both paths end on the new manga's details page.

---

## 7. S7-S9 Extensions list

### 7.1 Layout

- Header bar: back is not shown (top-level page), title `label_manga_extensions`, search entry (shared, section 2), overflow menu:
  - `action_filter` (opens S10).
  - `label_extension_repos` (opens the repos page, area 18/36).
- Refresh: header button with icon `view-refresh-symbolic`; tooltip `action_refresh` (new key, Proposal; upstream has no header refresh button). Pull-to-refresh is replaced by this button and F5 (section 9). Spinner in the button while `is_refreshing`.
- Update-all button: in the "updates pending" group header, primary button text `ext_update_all`.
- Grouped list with headers (`ExtensionHeader`, `header` style, 16 px horizontal, 8 px vertical):
  1. `ext_updates_pending` (only if updates exist) with `ext_update_all` button.
  2. `ext_installed` (installed + untrusted).
  3. One header per language (display name), in `compare_languages` order.
- Row (B§5.8):
  - Icon 40 px (40 px box); busy spinner (40 px ring, 2 px stroke) behind it while not complete; icon inset animates 0 to 8 px.
  - Name (one line, ellipsis).
  - Secondary line (wrapping FlowRow, `bodySmall`, 60% alpha): language, version, warning (uppercased, error colour), `ext_pending` / `ext_downloading` / `ext_installing` after a dot separator while busy.
  - Action buttons (right): per B§5.8.1 table.

### 7.2 Row actions table (visible buttons)

| Row state | Buttons (left to right) | Keys |
|---|---|---|
| Busy (Pending/Downloading/Installing) | Cancel (X) | `action_cancel` |
| Error | Retry | `action_retry` |
| Idle, Installed with update | Settings, Update | `action_settings`, `ext_update` |
| Idle, Installed no update | Settings | `action_settings` |
| Idle, Untrusted | Trust | `ext_trust` |
| Idle, Available with sources | Web view, Install | `action_open_in_web_view`, `ext_install` |
| Idle, Available without sources | Install | `ext_install` |

Icons: settings `emblem-system-symbolic`, update/install `folder-download-symbolic`, trust `security-high-symbolic`, web `web-browser-symbolic`, cancel `window-close-symbolic`, retry `view-refresh-symbolic`.

Tooltip = the accessible name (same key).

### 7.3 Click and gestures (desktop mapping)

| Gesture | Available | Installed | Untrusted |
|---|---|---|---|
| Primary click on row | install | open details | trust dialog (S8) |
| Primary click on action button | install | update if has update, else details | trust dialog |
| Secondary action (web / settings) | open WebView | open details | none |
| Long press (touch) / right-click (mouse) | install (upstream long click) | uninstall if package installed, else S9 confirm | same as Installed |

Proposal: right-click opens a context menu with the same actions as the long-press column (`gio::Menu`), because GTK has no long-press convention on desktop. Long press still works on touch screens.

### 7.4 S8 Trust dialog

- `gtk::AlertDialog`: title `untrusted_extension`, message `untrusted_extension_message`, buttons: `ext_uninstall` (cancel-role), `ext_trust` (default, suggested). Dismiss (Escape or X) = nothing (B§5.11).
- Note: upstream the dismiss button labelled "Uninstall" removes the extension; the GTK version keeps the same mapping: the "cancel" button is the uninstall.

### 7.5 S9 Private extension confirm

- `gtk::AlertDialog`: title `ext_confirm_remove`, message `remove_private_extension_message` with the name, buttons `action_cancel` (cancel) and `ext_remove` (destructive).

### 7.6 Empty and loading

- Loading: centred spinner in the content (B§5.4).
- Empty (no results, query present): message `no_results_found`, button `label_extension_repos`.
- Empty (no extensions at all): message `empty_screen`, button `label_extension_repos`.

### 7.7 Toasts

- None on success (upstream has no toast on install).
- `internal_error` toast on failing extension events is in S10 only.

---

## 8. S10-S13 Extension details, filter, source prefs

### 8.1 S10 Language filter

- Header: back, title `label_extensions`.
- Rows: `gtk::Switch` row per language, label = language display name, switch = enabled.
- Empty: `empty_screen` centred.
- Failure: toast `internal_error` (B§8).

### 8.2 S11 Extension details

Header bar: back, title `label_extension_info`, actions:
- Open repo icon button (`external-link-symbolic`, tooltip `action_open_repo`), shown when a repo URL exists.
- Overflow: `action_enable_all`, `action_disable_all`, `pref_clear_cookies`.

Body (scrollable):
1. Warning banner when obsolete: `obsolete_extension_message` (`gtk::InfoBar` with warning style, not closable).
2. Header block (centred, clickable to copy debug info, tooltip explains it: Proposal key `copy_debug_info`, D-UI-5):
   - Icon 112 px.
   - Name (`headlineSmall` = 24 px bold on GTK).
   - Stripped package name (dim, small).
3. Info row (three cells separated by vertical separators): version, language, and (NSFW only) age rating cell in error colour; the age-rating cell is a button and opens S12 when activated.
4. Buttons row: `ext_uninstall` (outlined, destructive styling) and, only for shared packages, `ext_app_info` (dropped on Linux, D1, so this button never appears).
5. Incognito row: `pref_incognito_mode` title, `pref_incognito_mode_extension_summary` subtitle, `gtk::Switch`.
6. Separator.
7. Source list: one row per source: title = source name or language display name (label-as-name rule), `gtk::Switch` read-only at the end (click the row to toggle), settings button (`label_settings`) only for configurable sources.

S12 NSFW dialog: `gtk::AlertDialog` with message `ext_nsfw_warning` and `action_ok`.

Debug copy: on click of the header block, copy the debug text (B§7.2) to the clipboard. Toast: none upstream. Proposal: toast "Copied" (key `copied`, new).

### 8.3 S13 Source preference page

- Header bar: back, title = source string (`getOrStub(sourceId).toString()`).
- Body: native preference rows built from the source declaration (area:17-source-app-layer declaration API; p1-behavior.md section 9; T-40a-023).
- Edit-text preferences: incognito entry (no suggestions, `input-purpose: free-form`, hidden history). Proposal: GTK `gtk::Entry` with `input-hints: no-spellcheck | no-emoji`.
- Dialog preferences: title defaults to the preference title when empty (B§9).
- Loading state: `ifMangaSourcesLoaded()` -> spinner until the source registry has loaded.

---

## 9. Keyboard and mouse (desktop)

Upstream has no keyboard map; these are Proposals.

| Action | Keys | Notes |
|---|---|---|
| Open search on the current page | `Ctrl+F` | focuses the header search entry |
| Close search / back | `Escape` | closes search first, then back |
| Refresh extension list | `F5` | same as refresh button |
| Context menu on a row | right-click, `Menu` key, `Shift+F10` | same actions as long press |
| Activate row | `Enter` | primary click |
| Select next/prev row | arrow keys in `ListView`/`GridView` | built in |
| Grid: move by column | arrows | built in |
| Browse tab sub-tabs | `Ctrl+1` ... `Ctrl+6` | six pages, same order |
| Migrate dialog: copy / migrate | `Ctrl+Enter` = migrate, `Ctrl+Shift+Enter` = copy | proposal; default button = migrate |
| Alert dialogs | `Escape` cancels | GTK default |
| Mouse wheel | scroll | native |
| Middle click on a manga | open in a new window | not supported (Kurobayomi is single-window) |

Focus order: header search -> content -> action buttons. Every icon-only button has an accessible name and tooltip.

---

## 10. S6 Global search

Layout:
- Header: back, search entry (`searchQuery`), progress bar under the header while `1 <= progress < total` (height 3 px).
- Filter chips row (horizontal scroll, 8 px gap): `pinned_sources` (pin icon, selected when filter = pinned), `all` (done-all icon, selected when filter = all), separator, `has_results` (filter icon, toggles only-with-results). Chips are `gtk::ToggleButton` with CSS class `chip`.
- Result list: one row per source (header line with the source name and language, arrow button), followed by the cards row (horizontal scroll) or state line.

Result row states:
- Loading: 16 px spinner, 16 px vertical padding (B§10.2).
- Success: horizontal card row; empty list -> `no_results_found` line.
- Error: error icon + message (raw error text) or `unknown_error`.

Cards: 96 px wide, cover on top, title up to 3 lines (`titleMaxLines = 3`), in-library badge, favorite cover at 34% alpha.

Interactions:
- Click header or arrow: S2 for that source with the query (B§10.2 `onClickSource`).
- Click card: open manga (B§10.3 `onClickItem`).
- Long-press card: open manga preview (`MangaScreen(id, true)`).
- Chip click: change filter only; query is not rerun (B§10.1, upstream comment).

Note: the header line uses `LocaleHelper.getLocalizedDisplayName(source.lang)` as its subtitle (not `getSourceDisplayName`), so the `all` language shows as the Java locale name upstream. Kurobayomi uses the same function for parity (p1-rust-design.md section 9).

---

## 11. Responsive behaviour and sizing

Targets: window width 360 px to 2560 px. The mockup window minimum is 800 px wide (existing `main.rs`); this area must also work at 360 px for the phone-width rule if the GTK app runs in a narrow tile.

| Element | Wide (>= 900 px) | Narrow (< 900 px) |
|---|---|---|
| Browse sub-tabs | all six visible | scrollable strip |
| Grid columns | per preference (`0` = adaptive 128 px) | per preference; minimum 2 columns |
| Source row | icon 40 px, name + language | same, language may be dropped below 500 px |
| Global search cards | horizontal scroll | same |
| Migrate dialog | 480 px wide | 90% of window width |
| Extension info row | three cells | cells wrap to two rows below 420 px |
| Extension details header icon | 112 px | 96 px below 420 px |

Text: all single-line labels ellipsize; multi-line labels wrap. No fixed-width text containers.

No horizontal page scroll: only the explicit horizontal strips (chips, cards row) scroll horizontally.

Gutter: 16 px side padding on all pages (matches the Compose `medium` padding of 16 dp).

---

## 12. Mockup gap list (current `/home/blackbox/kurobayomi`)

Verified against `src/entry.rs` (lines 242-400), `src/main.rs:175`, `spec/entries.json`.

| # | Gap | Where in mockup | Required |
|---|---|---|---|
| G1 | Sources list uses hard-coded "Source A/B/C" rows and fixed sections ("Last used", "Pinned", "English", "Other") | `entry.rs:283-309` | Real data: languages from `enabled` sources, pin state, `supportsLatest` |
| G2 | No option to hide languages/sources (S5 missing) | none | Implement S5 |
| G3 | Extensions list uses static groups and names ("Extension A".."F") | `entry.rs:311-358` | Real groups (B§5.3), query filter (B§5.2), install states (B§6) |
| G4 | Extension rows show a fixed "Update" / "Settings" / "Install" text button, not icon actions | `entry.rs:323-352` | Icon action set (section 7.2) |
| G5 | No busy state, no cancel, no retry | none | Implement (section 7.2) |
| G6 | Trust dialog exists as a mock (`dialog_of("browse","extension_trust")`) but is not wired to a model | `entry.rs:337` | Model-driven trust dialog (S8) |
| G7 | Extension details (S11) is a static screen name, no state, no source toggles, no enable/disable all, no clear cookies | `entry.rs` route `extension_details`; `spec/entries.json` | Full S11 |
| G8 | Extension filter (S10) and repos link missing data | route `extension_filter` | Real languages list |
| G9 | Browse source (S2) grid uses static cards; no real paging, no error states, no empty states, no display-mode menu | `entry.rs:380-400` | Section 3 |
| G10 | Source options dialog (S4) missing | none | Implement |
| G11 | Global search (S6) static | `entry.rs` `global_search` | Section 10 |
| G12 | Migrate source picker (S14) static counts 12/4/1, no sort controls | `entry.rs:360-378` | Real counts, sort header |
| G13 | Migrate favorites (S15) and per-source search (S17) missing | route `migrate_entries` is static; `migrate_source_search` opens `browse_source` | Implement |
| G14 | Migrate confirm dialog (S18) is a static `migrate_manga` dialog without flags | `entry.rs:230`, `dialog_of(...)` | Flags, copy/migrate, spinner, navigation |
| G15 | Browse badge is a constant `"1"` | `main.rs:175` | From `ext_updates_count` |
| G16 | Browse tab re-select behaviour not implemented | none | D-UI-1 |
| G17 | No keyboard map (section 9) | none | Implement |
| G18 | No context menu (right-click) on rows | none | Implement |
| G19 | No empty-state for lists (`EmptyScreen` equivalent) | none | Implement |
| G20 | Source preference page (S13) missing | none | Implement with area 17/18 declaration API |
| G21 | Mock uses "Source A" names even in the anime path; anime and manga share one builder | `entry.rs:283` (`anime` parameter) | Split: manga here, anime in areas 39/41 |

Non-gaps (things the mockup already has that match): tab order concept, the route key names in `spec/entries.json` (`browse`, `browse_source`, `global_search`, `migrate_search`, `migrate_entries`, `extension_details`, `extension_filter`, `extension_trust`).

---

## 13. State matrix (summary)

| Screen | Loading | Empty | Error | Selection | Busy |
|---|---|---|---|---|---|
| S2 browse | centred spinner | blank (parity) / empty text (D-UI-2) | toast bar (with items) / empty actions (no items) | none | append spinner |
| S3 sources | centred spinner | `source_empty_screen` | none | row click = popular | none |
| S5 source filter | none | `source_filter_empty_screen` | none | toggles | none |
| S6 global search | per-source spinner | `no_results_found` per source | per-source error row | filter chips | progress bar |
| S7 extensions | centred spinner | `empty_screen` / `no_results_found` + repos button | none | none | per-row spinner + Cancel |
| S10 ext filter | centred spinner | `empty_screen` | toast `internal_error` | toggles | none |
| S11 ext details | centred spinner (until state loaded) | `empty_screen` if extension gone (pops) | none | toggles | none |
| S13 source prefs | spinner until registry loaded | none | none | per pref | none |
| S14 migrate sources | centred spinner | `information_empty_library` | none | row click | none |
| S15 favorites | centred spinner | `empty_screen` | toast `internal_error` | row click | none |
| S16 migrate search | per-source spinner | `no_results_found` | per-source error | chips | progress bar |
| S18 migrate dialog | dialog content replaced by spinner | n/a | silent (parity) / toast (D-UI-4) | checkboxes | spinner overlay |

---

## 14. Deviations (decision list)

| ID | Upstream | Kurobayomi | Why |
|---|---|---|---|
| D-UI-1 | Browse re-select always opens anime global search | opens the visible kind's global search | upstream comment marks it unfinished |
| D-UI-2 | Browse source with no items and no error shows a blank grid | show `empty_screen` text | a blank page reads as broken on desktop |
| D-UI-3 | Long-press a source's language label copies source id silently | optional toast | desktop users expect feedback (P2) |
| D-UI-4 | Migration failure: dialog stays open with no message | toast `internal_error`, dialog stays open | silent failure is hard to diagnose |
| D-UI-5 | Debug copy has no feedback | toast "Copied" | consistency with D-UI-3 |
| D-UI-6 | Pull-to-refresh | refresh button + F5 | no pull gesture on desktop |
| D-UI-7 | Fast-scroll rail | none | GTK kinetic scroll |
| D-UI-8 | FAB "Filter" on migrate search | header-bar button | desktop convention |
| D-UI-9 | Migrate tracks always copied (hidden) | disclosed line under checkboxes | transparency |
| D-UI-10 | Chip filters apply only on next search | same (kept) | parity; flagged as UX debt upstream |
