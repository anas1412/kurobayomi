# Area 41 part 2: UI parity (anime sources, browse, global search, source filters, migration sources)

This file lists every visible element, state, dialog, snackbar, menu, gesture and animation of the scope screens, the desktop equivalent of each, and what the current Kurobayomi mockup lacks. Behavior details are in `p2-behavior.md`; widget visuals owned by part 1 are cited by path and marked "(part 1)".

Mockup reference: `src/entry.rs` (functions `browse`, `sources`, `browse_source`, `global_search`, `migrate_sources`, lines 244, 283, 380, 432, 360) and `src/ui.rs` (helpers `page` line 67, `row` line 147, `status` line 166, `cover` line 177, `chip_row`, `menu_button` line 202, `show_sheet` line 391, `show_dialog` line 406).

Conventions:
- "Desktop" column = the GTK 4 widget or input equivalent to use in Kurobayomi (no libadwaita).
- Long press on touch maps to: right-click (`gtk::GestureClick` button 3), the Menu key, and Shift+F10. Every long-press action is therefore reachable without a pointer hold.
- Keyboard shortcuts are proposals unless marked OBSERVED.

---

## 1. Browse tab: "Anime Sources" page (Sources tab)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt` (wiring), `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt` (part 1 view).

### 1.1 Elements

| # | Element | Visual (OBSERVED) | Desktop equivalent | Source |
|---|---|---|---|---|
| S1 | Tab title | "Anime Sources" (`AYMR.strings.label_anime_sources`) | Page title in the Browse tab's `gtk::Stack` page; tab label from the Browse tab (area:36) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:34` |
| S2 | Header action "Global search" | icon `TravelExplore`, description "Global search" | `gtk::Button` with icon `system-search-symbolic`, tooltip "Global search", shortcut Ctrl+Shift+F (PROPOSAL) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:36-40` |
| S3 | Header action "Filter" | icon `FilterList`, description "Filter" | `gtk::Button` with icon `view-filter-symbolic`, tooltip "Filter" | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:41-45` |
| S4 | Section header | text, `header` typography, padding 16 dp horizontal, 8 dp vertical (part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:96-110`) | `gtk::Label` (`heading` CSS class) as a `gtk::ListBox` header row; not focusable | section 3.3 of `p2-behavior.md` |
| S5 | Source row | icon 40 px (part 1 `AnimeSourceIcon`), name (one line, ellipsis), language line (dim, 0.78 alpha), action area | `gtk::ListBoxRow` with `gtk::Box` (horizontal): icon, vertical box (title, subtitle), suffix buttons | part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/components/BaseAnimeSourceItem.kt` |
| S6 | "Latest" text button | primary color, label "Latest" (`MR.strings.latest`), only if `supportsLatest` | `gtk::Button` flat, label "Latest", tooltip "Latest updates" | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:126-134` |
| S7 | Pin button | filled `PushPin` in primary color if pinned, outlined at 0.78 alpha otherwise; description "Pin"/"Unpin" | `gtk::ToggleButton` with icon `view-pin-symbolic` (active = pinned), tooltip "Pin"/"Unpin" | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:145-165` |
| S8 | Options dialog | title = visual name, entries "Pin"/"Unpin" and "Disable" (not for local) | `gtk::AlertDialog` (buttons listed below) | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:167-202` |
| S9 | Loading state | `LoadingScreen` centered | `gtk::Spinner` (`start`) centered in the page | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:50` |
| S10 | Empty state | text "No source found" (`MR.strings.source_empty_screen`), centered | `gtk::Box` with `dim-label` label, centered | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:51-54` |
| S11 | Failure snackbar | text "InternalError: Check crash logs for further information" | Overlay snackbar (see 8.1), 4 s | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:74-83` |

### 1.2 Dialog buttons (S8)
| Button | Shown when | Action |
|---|---|---|
| "Pin" or "Unpin" | always | toggle pin, close dialog |
| "Disable" | `source.id != 0` | toggle disabled, close dialog |
| Cancel (AlertDialog default, Escape) | always | close dialog |

Title: `source.visualName` (for example `Name (EN)`). No confirm button (OBSERVED `confirmButton = {}`), so the desktop dialog uses "Cancel" only.

### 1.3 States and transitions
| State | Entry | Visible | Exit |
|---|---|---|---|
| Loading | page open | S9 | first emission (any list, including empty) |
| Empty | emission with zero rows | S10 | next emission with rows |
| List | emission with rows | S4, S5 | any emission |
| Failed (Kotlin) | failure before first emission | S9 + S11 (Kotlin stays here) | Kurobayomi: inline error row with "Retry" (PROPOSAL, E-SRC-02) |

### 1.4 Gestures and input
| Input | Result | Desktop |
|---|---|---|
| Click on row | open browse at Popular | `ListBox::row-activated` |
| Click "Latest" | open browse at Latest | button `clicked` |
| Click pin | toggle pin | toggle `toggled` |
| Long press / right-click / Menu key / Shift+F10 on row | options dialog | `GestureClick` button 3 + `ListBoxRow` context menu action |
| Up/Down | move selection | `gtk::ListBox` default |
| Enter | open browse at Popular | `row-activated` (default) |
| Ctrl+F | focus filter field (PROPOSAL, not in Kotlin) | `gtk::ShortcutController` |

### 1.5 Animation
Kotlin uses `Modifier.animateItem()` on rows and headers (part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:77,82`). Dropped (see `p2-behavior.md` 12). Desktop: no row animation. `ListBox` rebuilds only when the section set changes.

### 1.6 Responsive behavior
- Window width below 480 px: the header actions collapse into an overflow `gtk::MenuButton` ("Global search", "Filter"). The rows keep their layout (name ellipsizes first).
- Rows never scroll horizontally.

### 1.7 What the mockup lacks (`src/entry.rs:283-309`)
- Sections are hard-coded ("Last used", "Pinned", "English", "Other") with fixed names. No grouping logic, no language headers from data, no `Multi` header for `all`.
- The "Latest" button and the pin toggle are on every row regardless of `supportsLatest` or actual pin state (the `active(section == "Pinned")` is a fixed value).
- The local source row is hard-coded to the "Other" section; the Disable item is not hidden for it.
- Long press opens the spec dialog `browse/source_options` (`entry.rs:301`), not the real dialog with `visualName` and Pin/Unpin/Disable.
- No failure snackbar, no empty state, no loading state.
- No pin or disable persistence.

---

## 2. Visibility page (Sources filter)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt` (wrapper), `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt` (part 1 view).

### 2.1 Elements

| # | Element | Visual (OBSERVED) | Desktop | Source |
|---|---|---|---|---|
| V1 | Title | "Sources" (`MR.strings.label_sources`) with back arrow | Header bar with back button | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt:32-36` |
| V2 | Language header | `SwitchPreferenceWidget`: title = display name, switch = language enabled | `gtk::ListBoxRow` with `gtk::Label` and `gtk::Switch` (`set-active`), activatable | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt:103-108` (part 1 file) |
| V3 | Source row | `BaseAnimeSourceItem(showLanguageInContent=false)`: icon, name only, trailing `Checkbox(checked = enabled, onCheckedChange = null)` | `gtk::ListBoxRow` with `gtk::CheckButton` (inactive, click on row toggles) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt:112-127` |
| V4 | Empty state | "No installed source found" (`MR.strings.source_filter_empty_screen`) | Centered label | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt:39-45` |
| V5 | Error toast | "InternalError: Check crash logs for further information", then back navigation | Overlay snackbar, then pop | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt:30-37` |
| V6 | Loading | `LoadingScreen` | Spinner | wrapper line 25-28 |

### 2.2 States, gestures, responsive
- Language row switch disabled → its source rows are removed from the list (not greyed out). The desktop view matches (rows removed).
- Click on the language row or its switch toggles the language.
- Click on a source row toggles the source. Keyboard: Space toggles the focused row (`ListBox` plus `gtk::CheckButton` focus behavior).
- No pin indicator on this page.
- Window width below 480 px: no change (single column).

### 2.3 What the mockup lacks
The mockup's "Filter" chip on browse opens `sources_filter` (`entry.rs:272`), but there is no visibility model behind it. The page has no language switches, no per-source checkboxes, and no empty or error state.

---

## 3. Browse page (one source)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt` (wiring, chips, dialogs), `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt` (part 1 body state machine), part 1 components for toolbar and grids.

### 3.1 Header area (top to bottom)

| # | Element | Visual | Desktop | Source |
|---|---|---|---|---|
| B1 | Back button / close-search | navigate up, or close search | `gtk::HeaderBar` back button; Escape also closes search first | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:92-99` |
| B2 | Title | `source.name` (`AppBarTitle`) | `gtk::HeaderBar` title (`set_title_widget` label) | part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt:50` |
| B3 | Search field | `SearchToolbar`, query = `toolbarQuery`, submit on IME action | `gtk::SearchBar` + `gtk::SearchEntry` (already in mockup `entry.rs:434`); toggled by the search button | part 1 toolbar; section 5.8 of behavior |
| B4 | Display-mode button | icon `ViewList` when list mode, else `ViewModule`; description "Display mode" | `gtk::MenuButton` with icon `view-grid-symbolic` or `view-list-symbolic` | part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt:61-69` |
| B5 | Display-mode menu | radio items: "Comfortable grid", "Compact grid", "List" (no "Cover only" item) | popover with three `gtk::CheckButton` (radio group) rows | part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt:97-122` |
| B6 | Overflow menu | "Open in WebView" (or "Help" for local), "Settings" if configurable | `gtk::MenuButton` `view-more-symbolic` with a `gio::Menu` | part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt:70-92` |
| B7 | Chips row | "Popular", "Latest" (if supported), "Filter" (if filters exist) | `gtk::Box` (horizontal) of `gtk::ToggleButton` with icon and label; Popular/Latest are a radio group, Filter is a plain toggle that opens the sheet | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:151-212` |
| B8 | Divider | `HorizontalDivider` | `gtk::Separator` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:214` |

Chip icons (OBSERVED): Popular `Favorite` (outlined), Latest `NewReleases` (outlined), Filter `FilterList` (outlined). Desktop icons: `emblem-favorite-symbolic`, `document-new-symbolic`, `view-filter-symbolic`.

### 3.2 Body states
| State | Condition (OBSERVED) | Visible | Desktop |
|---|---|---|---|
| Loading initial | `itemCount == 0 && refresh is Loading` | `LoadingScreen` | centered `gtk::Spinner` |
| Items | `itemCount > 0` | grid or list (B9) | `gtk::GridView` or `gtk::ListView` |
| Loading top | `prepend is Loading` | `BrowseSourceLoadingItem` first | spinner row at top of list |
| Loading bottom | `refresh or append is Loading` | `BrowseSourceLoadingItem` last | spinner row at bottom |
| Empty result | `itemCount == 0` and error is `NoEpisodes` (text "No results found") | `EmptyScreen` with actions (section 3.4) | centered label and buttons |
| Error (first page) | `itemCount == 0` and any other error | `EmptyScreen` with the formatted message and actions | centered label and buttons |
| Error (append) | `itemCount > 0` and error | snackbar, indefinite, with "Retry" (section 3.5) | overlay snackbar, no timeout |

### 3.3 Items (B9)

Grid cell (comfortable and compact, part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceComfortableGrid.kt`, `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceCompactGrid.kt`):
- Cover image, title (comfortable: overlay at the bottom; compact: below cover, 1 line; part 1 defines exact layout).
- In-library badge at cover start when `favorite == true` (`InLibraryBadge`).
- Favorite cover alpha 0.34.
- Grid content padding 8 px, spacing 4 px.
- Adaptive columns: cell minimum 128 px; fixed when the preference is set.

List row (part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceList.kt`): row height = window height minus top bar height; uses `EntryListItem` (area:37/part 1).

Desktop: `gtk::GridView` with `min-columns` set from the preference, and child `width_request` 128. Badge: `gtk::Overlay` child.

### 3.4 Empty and error actions (first page)
| Button | Shown when | Action |
|---|---|---|
| "Local source guide" | source is local | open `https://aniyomi.org/help/guides/local-anime/` (`gtk::UriLauncher`) |
| "Retry" | any other source | reload the pager (`refresh`) |
| "Open in WebView" | any other source | open the WebView page (area:36) |
| "Help" | any other source | open `https://aniyomi.org/docs/guides/troubleshooting/` |

Message text = formatted error (for `NoEpisodes`: "No results found").

### 3.5 Snackbar (append error)
- Text: formatted error.
- Action button: "Retry".
- Timeout: none (Kotlin `SnackbarDuration.Indefinite`). The snackbar leaves only through its Retry action or its close button (desktop equivalent of the Kotlin `Dismissed` result, which calls `currentSnackbarData.dismiss()`).

### 3.6 Long-press flow (desktop equivalent)
Trigger: right-click on a cell or row, or the Menu key, or Shift+F10 on a focused cell. Opens a `gtk::PopoverMenu`. The menu item depends on the same background lookup as Kotlin (duplicate check, `p2-behavior.md` 5.11):
- If favorite → "Remove from library" → Remove dialog.
- If a duplicate exists → open the duplicate dialog.
- Else → add directly (or open the category picker, section 3.8).

Plain left click on a cell opens the anime (`AnimeScreen(id, fromSource = true)`).

### 3.7 Dialogs (desktop)
| Dialog | Desktop widget | Buttons (in order) | Notes |
|---|---|---|---|
| Remove | `gtk::AlertDialog` | "Cancel" (cancel, default-cancel), "Remove" | title "Are you sure?"; detail "You are about to remove \"%s\" from your library" |
| Duplicate | modal `gtk::Window` 420 px | "Show anime", "Migrate existing entry", "Add anyway" (each dismisses first), then "Cancel" | title "Are you sure?"; body text per `p2-behavior.md` 5.12 |
| Migrate | part 1 | part 1 | |
| Category picker | modal `gtk::Window` 420 px | checkbox per user category, "Edit categories" (opens Categories page), "Cancel", "OK" | preselection per `getCategories.await(anime.id)` |
| Filter sheet | modal `gtk::Window` 480 x 640 | "Reset" (stays open), "Filter" (applies and closes) | section 3.9 |

### 3.8 Category picker detail
Kotlin `ChangeCategoryDialog` (`app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt:197`): rows are the user categories (system category excluded). Confirm dismisses the dialog and then calls `onConfirm(include, exclude)`. Desktop: OK closes the window and runs `changeAnimeFavorite` then `moveAnimeToCategories(include)`.

### 3.9 Filter sheet detail (desktop)
Layout: header row (left "Reset" flat button, right "Filter" suggested-action button), then a `gtk::ScrolledWindow` with a `gtk::ListBox`.

Per filter type (OBSERVED rendering in `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt:79-164`, presentation widgets from `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt`):

| Filter | Desktop widget | Visual | Interaction |
|---|---|---|---|
| Header | `gtk::Label` (`heading` class) row | bold text | none |
| Separator | `gtk::Separator` | line | none |
| CheckBox | `gtk::CheckButton` with label | checked or not | click toggles |
| TriState | `gtk::Button` (flat) with a row: icon + label | icon `CheckBoxOutlineBlank` (ignored), `CheckBox` (include), `DisabledByDefault` (exclude); icon tint primary when on | click cycles ignored → include → exclude → ignored |
| Text | `gtk::Entry` with a label above | text | every edit writes the filter state (OBSERVED onValueChange) |
| Select | `gtk::DropDown` with a label | options list | selection writes the index |
| Sort | `gtk::Expander` with one row per value | arrow icon on the selected value: "descending" arrow when `!ascending` | click value: same index toggles direction, other index keeps direction (section 6.4 of behavior) |
| Group | `gtk::Expander` with children | children rendered recursively | children behave as above |

Expanded state of expanders is per open sheet (not saved). Default collapsed (PROPOSAL; Kotlin's `CollapsibleBox` default is per composition).

Keyboard: Tab moves between controls; Space toggles checkboxes and TriState; arrows change Select; Enter on the Filter button applies.

### 3.10 Gestures and input (browse)
| Input | Result |
|---|---|
| Click cell or row | open anime (fromSource) |
| Right-click, Menu key, Shift+F10 | long-press menu (3.6) |
| Ctrl+F | open search bar (PROPOSAL) |
| Escape | if search open and not user query: close search; else pop (`navigateUp` rule, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:92-99`) |
| Enter in search | submit (`search`) |
| Alt+Left, Backspace on empty header | pop (PROPOSAL) |
| Mouse wheel / trackpad | scroll the grid (GTK default) |
| Scroll near end | `pager.append()` (prefetch 25 items before the end) |

### 3.11 Animation and haptics
- Haptics (`LongPress`) dropped (see `p2-behavior.md` 12).
- Compose list animations dropped.
- Desktop: no transitions beyond GTK's default page transition.

### 3.12 Responsive behavior
- Narrow windows (< 480 px): chips row scrolls horizontally (Kotlin `horizontalScroll`). Desktop: `gtk::ScrolledWindow` (horizontal `Automatic`, vertical `Never`).
- Grid columns: adaptive (128 px cells) when the preference is 0. Fixed columns when set. Fixed column count is not clamped; GTK keeps at least 1 column.
- Landscape vs portrait preference: on Linux, "landscape" means window width >= height (PROPOSAL; Kotlin uses device orientation).

### 3.13 What the mockup lacks (`src/entry.rs:380-430`)
- Real source data (the cover grid is 24 placeholder covers titled by `title_n`).
- Popular/Latest/Filter chips do not change the listing (`browse_source` only sets the first toggle active and opens `filter_sheet`).
- No paging, no append spinner, no empty or error states, no snackbar.
- Display mode button opens the spec dialog `browse_source/display_mode` (`entry.rs:397`), not the real radio group with persistence.
- Overflow menu entries are real `open` calls only for "Open in WebView" and "Settings" (`entry.rs:416-419`), with no Help and no local-source branch.
- Long press and right-click on covers are not implemented (`open_entry` on click only).
- Favorite add/remove flow (remove confirm, duplicate dialog, category picker) is missing.
- The in-library badge is not rendered.
- The "Filter" chip opens a generic spec sheet, not the source's filter list.

---

## 4. Filter sheet (standalone entry from the browse chip)

The sheet is the same widget as section 3.9. Opened from the "Filter" chip only. Behavior: filter edits stay in memory until "Filter" is pressed or the sheet is reopened and reset (see `p2-behavior.md` 6.2).

---

## 5. Global search

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt` (wiring), `app/src/main/java/eu/kanade/presentation/browse/anime/GlobalAnimeSearchScreen.kt` (part 1 view), `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt`, `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt` (part 1).

### 5.1 Header
| # | Element | Visual (OBSERVED) | Desktop |
|---|---|---|---|
| G1 | Back button | `navigateUp` (pop) | `gtk::HeaderBar` back |
| G2 | Search field | `SearchToolbar`, query text, submit on IME action (`onSearch = search()`) | `gtk::SearchBar` + `gtk::SearchEntry`, search-changed updates the query, activate runs `search()` |
| G3 | Progress bar | `LinearProgressIndicator`, full width, bottom of the search row, shown when `progress in 1..<total`, fraction `progress / total` | `gtk::ProgressBar` with `set_fraction`, hidden otherwise |
| G4 | Chip "Pinned" | icon `PushPin`, selected when filter is Pinned | toggle (radio group with "All") |
| G5 | Chip "All" | icon `DoneAll` | toggle (radio group) |
| G6 | Divider (vertical) | `VerticalDivider` | `gtk::Separator` vertical |
| G7 | Chip "Has results" | icon `FilterList`, selected = only-with-results | toggle (not in radio group) |
| G8 | Divider | `HorizontalDivider` | `gtk::Separator` |

Placeholder text: Kotlin `SearchToolbar` default (part 1 owns the exact string). Desktop: "Search".

### 5.2 Body (per source)
One section per source in `filteredItems` order:
- Header row: title (source name, heading style), subtitle (language display name, dim), trailing icon button "Open source" (`go-next-symbolic`). Click on the header row opens the per-source browse with the current query.
- Body by result:
  - Loading: a spinner row (`GlobalSearchLoadingResultItem`).
  - Success with items: horizontal card row (5.3).
  - Success empty: text "No results found" (`no_results_found`), padding 16 px horizontal, 8 px vertical.
  - Failed: error icon (`dialog-error-symbolic`), then the raw error message (may be empty when none). Centered.

### 5.3 Card row
- Horizontal `gtk::ScrolledWindow` (horizontal scroll, no vertical). Padding 8 px, spacing 4 px.
- Card: 96 px wide; cover; title up to 3 lines (ellipsis); in-library badge; favorite cover alpha 0.34.
- Click: open anime (`AnimeScreen(id, fromSource=true)`). Right-click/Menu: same (Kotlin long press also navigates; no favorite toggle here).

### 5.4 Single-result auto open (extension URL handler flow)
While `showSingleLoadingScreen` is true, show a centered spinner and no list. When the one source returns exactly one anime, replace the page with the anime page. Otherwise show the normal list.

### 5.5 States
| State | Visible |
|---|---|
| Idle (no query) | header only; body empty (Kotlin shows nothing until search) |
| Searching | progress bar and per-source spinners; results appear as they arrive; sections reorder (non-empty first) |
| Done | progress bar hidden; all sections final |
| Has results filter on | only sections with at least one result; progress still counts all |

### 5.6 Gestures and keyboard
| Input | Result |
|---|---|
| Enter in search | start search (or no-op if blank; blank does not clear results) |
| Ctrl+F | focus search (PROPOSAL) |
| Click section header | open per-source browse with the query |
| Click card | open anime |
| Escape | closes the search bar; a second Escape pops the page (PROPOSAL) |

### 5.7 Responsive
- Each section is a full-width row; card rows scroll horizontally.
- Header chips wrap by scrolling horizontally below 480 px.

### 5.8 What the mockup lacks (`src/entry.rs:432-480`)
- Fixed three placeholder sources with fake titles; no fan-out, no per-source states, no errors, no empty result from data.
- Progress is fixed at 0.6 (`progress.set_fraction(0.6)`) and never updates.
- Chips are not wired.
- "Has results" toggle missing (no persisted state).
- Result order is fixed, not by pinned/success/name.
- No single-result auto-open.
- Migrating variant (`migrate_search`) uses "▶" marker on the first source; the real marker applies only in migrate search (part 1 / behavior 7.12).

---

## 6. Migration sources tab

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt` (wiring), `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt` (part 1 view).

### 6.1 Header and body
| # | Element | Visual (OBSERVED) | Desktop | Source |
|---|---|---|---|---|
| M1 | Title | "Migrate Anime" (`AYMR.strings.label_migration_anime`) | page title | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:30` |
| M2 | Help action | icon `HelpOutline`, "Source migration guide" (`MR.strings.migration_help_guide`) | `gtk::Button` icon `help-about-symbolic` in header, tooltip | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:31-38` |
| M3 | Sticky header row | text "Select a source to migrate from" (`MR.strings.migration_selection_prompt`), sort-mode button, direction button | `gtk::Box` header row pinned on top of the `ScrolledWindow` (not inside the scroller) | part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt:91-128` |
| M4 | Sort-mode button | `SortByAlpha` ("Alphabetically") or `Numbers` ("Total entries") | `gtk::Button` icon `view-sort-ascending-symbolic` / `view-list-ordered-symbolic`, tooltip = description | part 1 lines 104-115 |
| M5 | Direction button | `ArrowUpward` ("Ascending") or `ArrowDownward` ("Descending") | `gtk::Button` icon `view-sort-ascending-symbolic`/`view-sort-descending-symbolic`, tooltip | part 1 lines 116-127 |
| M6 | Source row | icon, name (blank → id), language (if `lang != ""`), trailing count badge | `gtk::ListBoxRow` with icon, two-line label, `gtk::Label` pill (count) | part 1 lines 131-142, 146-205 |
| M7 | Not installed label | text "Not installed" in error color (only when stub) | dim label with `error` CSS class | part 1 lines 191-200 |
| M8 | Empty | "Your library is empty" (`information_empty_library`) | centered label | part 1 lines 56-59 |
| M9 | Loading | `LoadingScreen` | spinner | part 1 line 55 |

### 6.2 Gestures
| Input | Result |
|---|---|
| Click row | open `MigrateAnimeScreen(source.id)` (part 1) |
| Right-click or long press row | copy the source id to the clipboard (`copyToClipboard(id, id)`; part 1 call site) |
| Click sort-mode button | toggle Alphabetical ↔ Total entries and persist both prefs |
| Click direction button | toggle ascending ↔ descending and persist |

### 6.3 Desktop notes
- Count badge: pill-shaped label with the count number.
- The sticky header stays visible while the list scrolls (layout outside the scroller).
- Kotlin's `copyToClipboard` gives Android's clipboard feedback. PROPOSAL: after copying, show a snackbar "Copied" with the id.

### 6.4 What the mockup lacks (`src/entry.rs:360-378`)
- Static three sources with fixed counts (12, 4, 1).
- No sort buttons, no sticky header, no stub label, no copy-id gesture, no empty or loading state.
- Row click always goes to `migrate_entries` (spec screen) and does not pass the source id.

---

## 7. Keyboard and pointer summary (new in Kurobayomi)

| Context | Key or pointer | Action |
|---|---|---|
| Any browse page | Ctrl+F | focus search (PROPOSAL) |
| Any browse page | Escape | close search, then pop |
| Any list | Up/Down/Home/End/PageUp/PageDown | GTK default |
| Source row | Enter | open browse (Popular) |
| Source row | Shift+F10, Menu key | options dialog |
| Browse grid cell | Enter | open anime |
| Browse grid cell | Shift+F10, Menu key | favorite menu |
| Filter sheet | Tab, Space, Arrows | GTK default controls |
| Global search | Enter | run search |
| Migrate row | Enter | open migrate entries |
| Migrate row | Shift+F10 | copy id |

---

## 8. Shared visual feedback

### 8.1 Snackbar (overlay)
- Position: bottom center of the window.
- Background: `background` CSS color with 0.9 opacity; text uses the theme's foreground (follows the user's GTK theme).
- Timeout: 4000 ms for the sources failure and visibility error; no timeout for paging append errors (section 3.5).
- Action button: optional text button on the right.
- Implementation: `gtk::Overlay` child `gtk::Revealer` (`transition-type = slide-up`).

### 8.2 Loading
- Full-page loading: centered `gtk::Spinner` (`spinning = true`, size 32 px).
- Inline loading rows: spinner plus label "Loading" (PROPOSAL; Kotlin uses a plain loading item).

### 8.3 Empty state
- Icon (48 px) plus primary label plus up to three buttons, centered. Text per 3.4.

### 8.4 Theme
- Use the GTK theme colors only (`@theme_fg_color`, `@theme_bg_color`, `@accent_color`). Do not hard-code colors. Project user theme is Tokyo Night (per memory note "match-current-theme"): rely on GTK theme colors so the user's theme applies.

---

## 9. Parity checklist (for review)

| Item | Kotlin | Kurobayomi target | Task |
|---|---|---|---|
| Sections on sources list | 3.2 | identical rules | T-41b-002 |
| Sources failure snackbar | 3.8 | snackbar + inline retry (new) | T-41b-004 |
| Pin and disable in options dialog | 3.5-3.6 | identical | T-41b-006, T-41b-007 |
| Visibility page controls | 4.3 | identical | T-41b-009 |
| Browse chips | 5.7 | identical | T-41b-015 |
| Browse empty and error states | 5.10 | identical with D-2 | T-41b-018 |
| Display modes and columns | 5.9 | identical (CoverOnly as compact) | T-41b-017 |
| Favorite flow dialogs | 5.11-5.13 | identical | T-41b-019 to T-41b-022 |
| Genre search | 5.15 | identical | T-41b-024 |
| Filter sheet rendering | 6.3-6.4 | identical with D-4 | T-41b-030 |
| Global search progress, chips, sections | 7.7-7.12 | identical | T-41b-035 |
| Global search single-result open | 7.13 | identical | T-41b-037 |
| Migration sort and picker | 8.3-8.5 | identical | T-41b-039, T-41b-040 |
