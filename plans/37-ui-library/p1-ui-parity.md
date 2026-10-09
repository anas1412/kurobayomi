# 37-ui-library — UI parity (what must be visible and how it behaves on the desktop)

Source: Compose screens in `app/src/main/java/eu/kanade/presentation/library/**`, `app/src/main/java/eu/kanade/presentation/category/**`, and the tabs in `app/src/main/java/eu/kanade/tachiyomi/ui/library/**` and `ui/category/**`. Paths are repo-relative to `/home/blackbox/aniyomi-src/`. Behavior details are in `p1-behavior.md`; this file lists visible elements, states and the desktop equivalent.

Notation: **[S]** = state, **[K]** = keyboard/mouse equivalent, **[G]** = GTK widget to use, **[M]** = what the existing mockup has or lacks.

---

## U-1. Library page (anime and manga; one implementation parameterised by `Kind`)

### U-1.1 Header row
Upstream: `LibraryToolbar` (`app/src/main/java/eu/kanade/presentation/library/components/LibraryToolbar.kt:28-63`).

| Element | Upstream | Desktop [G] |
|---|---|---|
| Title text | `getToolbarTitle` (B-7.3): "Anime"/"Manga" when tabs shown, else category name | `gtk::Label` in the page header, ellipsised, max 1 line |
| Count pill | `Pill(text = count, 14 sp, onBackground α 0.08/0.12)` when `numberOfEntries != null` | `gtk::Label` with CSS class `pill` (background `alpha(@window_fg_color, 0.08)`, dark 0.12) |
| Search | `SearchToolbar` (expand search field inline) | `gtk::SearchBar` + `gtk::ToggleButton` (`system-search-symbolic`), placeholder "Search…" (mockup already does this) |
| Filter button | `Icons.Outlined.FilterList`, tint `active` when `hasActiveFilters` | `gtk::Button` `view-filter-symbolic`; add CSS class `accent` when filters active |
| Overflow menu | `action_update_library` (global), `action_update_category`, `action_open_random_manga` | `gtk::MenuButton` `view-more-symbolic` with three items (same labels) |
| Back arrow | only when `navigateUp != null` (manga tab from More) | `gtk::Button` `go-previous-symbolic`, shown when `navigate_up` is set |

Selection state header (replaces the regular header while `selectedCount > 0`): count text, `action_select_all` (icon `edit-select-all-symbolic`), `action_select_inverse` (icon `object-select-symbolic`-family; use `view-refresh` if the theme lacks it), cancel (`window-close-symbolic`, tooltip `action_cancel`). [K] Escape = cancel; Ctrl+A = select all in current page; Ctrl+I = invert.

[M] Mockup: title and header actions exist, but no count pill, no selection header, no update menu entries, no random entry, no back arrow.

### U-1.2 Category tabs (when shown)
Upstream: `LibraryTabs` (`LibraryTabs.kt:17-52`).
- Visible when `showPageTabs && categories.size > 1` (`showPageTabs = showCategoryTabs || searchQuery non-empty`).
- Each tab: label `visualName` (Default for system) + count badge when `numberOfEntries != null`.
- Selected tab uses the primary style; unselected uses `onSurface`.
- Horizontal scrolling when tabs overflow; edge padding 0.
- Divider line under the tab strip.
- [G] `gtk::ScrolledWindow` (horizontal, `policy: External, Never`) containing a `gtk::Box` (linked) of `gtk::ToggleButton` in one group; each button = `gtk::Box(label + badge label)`. Divider = `gtk::Separator`.
- [K] Ctrl+PageUp / Ctrl+PageDown and Alt+Left/Right switch pages; mouse wheel over the tab strip scrolls the strip.
- [M] Mockup: `linked()` with category names (no counts, no scroll, no keyboard).

### U-1.3 Pages
- One page per category (`categories`). [G] `gtk::Stack` with `transition-type = SlideLeftRight`, `transition-duration = 200`, `hhomogeneous = true`.
- Only current ±1 pages are populated (others are placeholder `gtk::Box`).
- Swipe gesture: touch-pad horizontal scroll switches pages (GTK `GestureSwipe` on the stack, or `gtk::EventControllerScroll` with horizontal delta > threshold).
- Persist the page index (`last_used_category`) on each switch.
- Empty categories show the empty state (U-1.6).

### U-1.4 Grid (compact, cover-only, comfortable)
Upstream: `LazyLibraryGrid` (`components/LazyLibraryGrid.kt:14-29`), items in `CommonEntryItem.kt`.

| Property | Value | Desktop [G] |
|---|---|---|
| Columns | auto (0): adaptive min 128 dp; fixed N | `gtk::GridView` `min_columns`/`max_columns` (computed from width) |
| Cover ratio | 2:3 (`ItemCover.Book`) | `gtk::Picture` with `set_content_fit(Cover)` in a `gtk::AspectFrame` ratio 0.6667 |
| Spacing | 4 dp h and v; grid padding +8 dp | `GridView` `set-row-spacing(4)`, `column-spacing(4)`, margin 8 |
| Selected | cover α 0.76, background secondary α 0.22/0.16, content onSecondary | CSS `.library-item.selected` (background `alpha(@accent_bg_color, 0.22)`; `picture { opacity: 0.76 }`) |
| Compact title | overlay at bottom, gradient transparent → #AA000000 over 33 % of cover; `titleSmall` white with shadow, 12 sp, 2 lines max | `gtk::Overlay` with a bottom `gtk::Box` CSS class `title-scrim` (`linear-gradient(to bottom, transparent, rgba(0,0,0,0.67))`), label white with `text-shadow: 0 0 4px black` |
| Cover-only | no title | same without the scrim |
| Comfortable title | below cover; 12 sp; 2 lines; padding 4 dp | `gtk::Label` `wrap`, `lines 2`, `ellipsize End`, below the picture |
| Badges (start) | downloads (tertiary), unread (secondary) | top-left `gtk::Box` (linked) of labels with classes `badge`, `badge-accent` |
| Badges (end) | language or local folder icon (tertiary) | top-right |
| Continue | compact+title: 28 dp; cover-only and comfortable: 32 dp, icon 20 dp, bottom-right, 6 dp inset | `gtk::Button` `circular` `osd`, `media-playback-start-symbolic`, tooltip "Resume" |
| Global-search row | full-width first item when search non-empty | a `gtk::Button` spanning full width (GridView header is not available; use a leading item with `GridItemSpan`-like wrapper: a separate `gtk::Box` above the `GridView` inside the page) |

Interaction:
- Click: open entry (selection empty) or toggle (selection non-empty).
- Long-press or right-click: range select (U-4).
- Continue button: resume (B-8.10 / B-8.11). Its click does not open the entry.
- Keyboard: arrow keys move focus in the grid; Enter opens; Space toggles selection; Shift+arrow extends selection (proposal).

[M] Mockup: a `FlowBox` of placeholder cards; cards show a numeric "badge" and a resume button; no real cover, no grid mode switch, no title overlay, no selection state per item, no range select, no global-search row.

### U-1.5 List mode
Upstream: `EntryListItem` (`CommonEntryItem.kt:333-389`), `AnimeLibraryList` (`AnimeLibraryList.kt:23-84`).
- Row: cover (full row height, aspect 2:3), title (`bodyMedium`, wraps), badges group, continue button (28 dp) after badges.
- Row height: `(list_height / list_size) - 3/list_size` px. When `list_size == 0`, height 76 dp. Clamp to ≥ 48 px in the desktop port (proposal).
- Row padding 16 dp horizontal, 3 dp vertical.
- Selected: background `alpha(@accent_bg_color, 0.22)`.
- [G] `gtk::ListView` + `SignalListItemFactory`; the row box is a `gtk::Box` with `hexpand`, `set_size_request(-1, row_height)`.
- The "List size" slider (U-3.3) controls how many rows fit on screen.

### U-1.6 Empty and loading states
| State | Upstream | Desktop |
|---|---|---|
| Loading (first load) | `LoadingScreen` (spinner) | `gtk::Spinner` centered, 32 px |
| Library empty (no search, no filters) | `EmptyScreen(information_empty_library)` + action `getting_started_guide` (`HelpOutline` icon) | `adw`-less status page: `gtk::Box` vertical, icon `dialog-information-symbolic`, label "Your library is empty", button "Getting started guide" → `gtk::UriLauncher` (GETTING_STARTED_URL) |
| No results (search) | `EmptyScreen(no_results_found)` + `GlobalSearchItem` | label "No results found" + global search button |
| Filters match nothing | `EmptyScreen(error_no_match)` | "No match found" |
| Category empty | `EmptyScreen(information_no_manga_category)` | "Category is empty" |
| Random entry none | snackbar `information_no_entries_found` | toast "No entries found in this category" |

Empty/status widgets use a shared `ui::status(icon, text)` helper that already exists in the mockup (`src/ui.rs:166`).

### U-1.7 Action bar (selection)
Upstream: `LibraryBottomActionMenu` (visible while `selectionMode`).
- Left-to-right buttons: Change category (`tag-symbolic`), Mark viewed/read (`object-select-symbolic`), Mark unviewed/unread (`edit-undo-symbolic`), Download (`folder-download-symbolic`, menu: Next 1 / 5 / 10 / 25 / Unviewed; hidden if any selected entry is local), Delete (`user-trash-symbolic`).
- Selection count visible in the header (U-1.1).
- [G] `gtk::ActionBar` at the bottom of the library page, shown with `gtk::Revealer` (`SlideUp`, 200 ms).
- [M] Mockup has a `Revealer` with the same five actions (good) but triggers only on right-click or long-press of the grid; no selection state, and the download menu is a stub.

### U-1.8 Toasts and snackbars
- Upstream snackbar: `updating_category`, `update_already_running`, `information_no_entries_found`, `no_next_chapter`.
- Desktop: in-window toast (`gtk::Revealer` at the bottom centre, `gtk::Label` with CSS `toast`, auto-hide 4 s, one at a time; a newer toast replaces the older one).
- Messages verbatim: "Updating category", "An update is already running", "No entries found in this category", "Next chapter not found" (B-7/B-11/B-16).

### U-1.9 Pull-to-refresh
Upstream only when `selection.isEmpty()`. Desktop: no pull gesture (drop); provide `F5` and the overflow menu entry (U-1.1). Spinner indicator for 1 s after a successful start (upstream fake spinner) is replaced by a toast "Updating category". [M] Mockup: none.

---

## U-2. Category management screens

Upstream: `CategoriesTab` (`ui/category/CategoriesTab.kt`), `AnimeCategoryScreen` (`presentation/category/AnimeCategoryScreen.kt`), `CategoryListItem`, `CategoryDialogs`, `CategoryFloatingActionButton`.

### U-2.1 Container
- Two sub-pages: Anime and Manga (`TabbedScreen`). [G] `gtk::Notebook` or a `gtk::StackSwitcher` above a `gtk::Stack`.
- Opening the Manga sub-page from the manga library "Edit categories" flow (B-15.1, B-14.12).

### U-2.2 List
- Vertical list of cards (`ElevatedCard` look): drag handle, name, rename button, hide/show button, delete button. Whole-row click = rename.
- [G] `gtk::ListBox` (`selection-mode: None`) with rows as `gtk::Box` + `gtk::Image` `list-drag-handle-symbolic`. Drag reorder via `gtk::DragSource`/`gtk::DropTarget` on the handle (MIME type `application/x-kurobayomi-category-id`). Insert line indicator on drop.
- Hidden categories: name rendered dimmed (CSS `dim-label`) and icon shows the action to show (`view-reveal-symbolic`); visible ones show `view-conceal-symbolic`. Tooltip "Hide" for both (upstream `action_hide`).
- Empty state: "You have no categories. Tap the plus button to create one for organizing your library." with the Add button.
- Floating add button: upstream `ExtendedFloatingActionButton` expands/collapses on scroll. Desktop: a bottom-right `gtk::Button` labelled "Add" with icon `list-add-symbolic` (always expanded; no collapse on desktop).

### U-2.3 Dialogs
| Dialog | Title (key) | Fields and buttons |
|---|---|---|
| Create | "Add category" (`action_add_category`) | Entry labelled "Name" (`name`), helper "*required" (`information_required_plain`) or error "A category with this name already exists!" (`error_category_exists`). Buttons: Cancel, Add (`action_add`, enabled iff non-empty and unique). Focus entry on open. |
| Rename | "Rename category" (`action_rename_category`) | Entry prefilled. Buttons: Cancel, OK (`action_ok`, enabled iff changed and unique). |
| Delete | "Delete category" (`delete_category`) | Body "Do you wish to delete the category "%s"?" (`delete_category_confirmation`). Buttons: Cancel, OK. |

[G] `gtk::Window` modal (transient for the main window) with `gtk::Entry`. Enter = confirm when enabled; Escape = cancel.

### U-2.4 Errors
`InternalError` → toast "InternalError: Check crash logs for further information" (`internal_error`). Desktop: keep the string but route to the log path: "Internal error — see logs" is a desktop-only proposal (T-37a-041 acceptance note).

[M] Mockup: `categories()` in `src/screens.rs:404-421` lists names with rename/delete and a FAB, opening spec dialogs. Missing: drag reorder, hide/show, anime and manga split, empty state text, name uniqueness validation, data source.

---

## U-3. Settings dialog (Filter / Sort / Display)

Upstream: `AnimeLibrarySettingsDialog` (`presentation/library/anime/AnimeLibrarySettingsDialog.kt`), `TabbedDialog`.

Desktop container: a modal `gtk::Window` 520 px wide, `gtk::Notebook` with three tabs "Filter", "Sort", "Display" (upstream labels `action_filter`, `action_sort`, `action_display`), each page a scrolled `gtk::Box` vertical with 12 px margins. Opened from the header filter button and from a sidebar re-click (B-1.2).

### U-3.1 Filter tab
Rows (in order, B-12.1):
1. Downloaded (`label_downloaded`) — tri-state. Disabled and shows Include when global downloaded-only is on.
2. Unseen (anime, `action_filter_unseen`) / Unread (manga, `action_filter_unread`).
3. Started (`label_started`).
4. Bookmarked (`action_filter_bookmarked`).
5. Completed (`completed`).
6. Customized update frequency (`action_filter_interval_custom`) — only when the restriction is on and build is not release (desktop: never shown; drop, see E-45 / T-37a-057).
7. Tracked: one row (`action_filter_tracked`) for 1 tracker, a heading + one row per logged-in tracker for more.

Tri-state row control: [G] `gtk::CheckButton` with `set_inconsistent(true)` for the `Not` state (show a minus indicator), and an adjacent state label: "Off", "Is", "Not" (translatable; the upstream uses only the checkbox glyph). Click cycles DISABLED → IS → NOT → DISABLED (B-4.3). Show the state label because a tri-state checkbox alone is ambiguous on desktop (proposal).

[M] Mockup: `sheets` from spec (`spec/app.json`), generic. Missing: real tri-state rows, tracker rows, downloaded-only lock.

### U-3.2 Sort tab
Rows: radio-like list (upstream `SortItem`): label + arrow icon on the active row (`go-up-symbolic` for ascending, `go-down-symbolic` for descending). Click active row flips direction; click another row keeps the direction (B-5.8). Random row: refresh icon when active; click reshuffles (B-5.6).
Option set: depends on kind (B-12.2) and on whether any tracker is logged in (TrackerMean row shown only then).
[G] `gtk::ListBox` with `gtk::CheckButton` group styled as radio (or `gtk::ToggleButton` rows), arrow image at end.

### U-3.3 Display tab
1. Mode chips (upstream `FilterChip` row): "Grid" (compact), "Comfortable grid", "Cover only grid", "List". [G] linked `gtk::ToggleButton` group.
2. Columns slider: upstream `SliderItem` 0…10, label "Items per row" (grids) or "List size" (list), value text "Auto" at 0. [G] `gtk::Scale` (horizontal, digits 0, draw-value true, `value-pos` right) with a formatter that returns "Auto" for 0. Persist to portrait/landscape key (desktop: one key, because orientation is not a concept on desktop; proposal — T-37a-039 records the decision).
3. Heading "Overlay" then checkboxes: Download badge (`action_display_download_badge_anime` / `action_display_download_badge`), Unseen/Unread badge (`action_display_unseen_badge` / `action_display_unread_badge`), Local source (`action_display_local_badge`), Language (`action_display_language_badge`), Continue watching/reading button (`action_display_show_continue_reading_button`).
4. Heading "Tabs" then: Show category tabs (`action_display_show_tabs`), Show number of items (`action_display_show_number_of_items`).

All changes apply immediately (no OK button). [G] `gtk::CheckButton` rows; heading = `gtk::Label` `heading` CSS class.

[M] Mockup: none of these settings exist.

---

## U-4. Selection and gestures (desktop mapping)

| Upstream gesture | Desktop mapping | Note |
|---|---|---|
| Tap entry (no selection) | Click → open entry | |
| Tap entry (selection mode) | Click → toggle | |
| Long-press entry | Long-press (500 ms, GTK default) → range toggle | Matches Android. |
| — | Right-click → range toggle | Desktop convention; mockup already uses right-click. |
| — | Ctrl+click → toggle; Shift+click → range | Desktop extras. |
| Back (system) | Escape → clear selection, else clear search | |
| Select all / invert (header) | Ctrl+A / Ctrl+I (page scope) | |

Range rule: same category only, anchored at last selected (B-10.3). Visual: selected items show the selected style; the action bar shows the count.

[M] Mockup: right-click and long-press reveal the bar, but no per-item selection and no range.

---

## U-5. Badges and labels

| Badge | Text | Colors (theme tokens) | Shown when |
|---|---|---|---|
| Downloads | number | tertiary bg, onTertiary fg | count > 0 and download badge on |
| Unseen/unread | number | secondary bg, onSecondary fg | count > 0 and unread badge on |
| Local | folder icon | tertiary bg, onTertiary | local badge on and entry is local |
| Language | uppercase lang, e.g. "EN" | tertiary | language badge on, not local, lang non-empty |

Metrics: horizontal padding 3 px, vertical 1 px (dp in upstream), font bodySmall medium, single line; group inset 4 px.
[G] CSS classes `.badge`, `.badge-accent` (tertiary), `.badge-osd` (translucent black), each with `border-radius: 4px`.

---

## U-6. Typography and spacing reference

| Use | Upstream | Desktop |
|---|---|---|
| Grid title | titleSmall 12 sp, line 18 sp | 12 pt (GTK font scale) |
| List title | bodyMedium | GTK body |
| Badge | bodySmall medium | 0.85 em, weight 600 |
| Category tab | default label | GTK label |
| Dialog title | titleLarge | GTK heading class |

Desktop: never hard-code px for text; use `em` in CSS and let GTK's font scale apply.

---

## U-7. Responsive behaviour

- Window width changes → GridView recomputes columns (adaptive mode) without rebuilding items.
- Min window width 480 px: below that, hide the tab strip labels and show badges only (proposal; upstream phone width).
- Landscape/portrait distinction (upstream columns per orientation) → one column preference on desktop (U-3.3).
- Keep the action bar usable at 480 px: buttons collapse into an overflow `gtk::MenuButton` when width < 560 px (proposal).

---

## U-8. Animations and transitions

| Upstream | Desktop |
|---|---|
| Tab icon animated vector (`anim_animelibrary_leave`, `anim_library_enter`, `anim_updates_enter`) | static symbolic icon; no animation (drop; T-37a-052) |
| Pager swipe | Stack slide 200 ms |
| Action bar slide | Revealer SlideUp 200 ms |
| Dialogs | GTK default |
| Item selection | immediate CSS change |

---

## U-9. Strings (English, verbatim; keys in parentheses)

Library and categories (all keys verified in `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`):

| Key | English |
|---|---|
| label_anime_library | Anime |
| label_manga_library | Manga |
| label_default | Default |
| label_downloaded | Downloaded |
| label_started | Started |
| label_auto | Auto |
| completed | Completed |
| name | Name |
| action_filter | Filter |
| action_sort | Sort |
| action_display | Display |
| action_display_mode | Display mode |
| action_display_grid | Compact grid |
| action_display_comfortable_grid | Comfortable grid |
| action_display_cover_only_grid | Cover-only grid |
| action_display_list | List |
| action_display_download_badge | Downloaded chapters |
| action_display_download_badge_anime | Downloaded episodes |
| action_display_unread_badge | Unread chapters |
| action_display_unseen_badge | Unseen episodes |
| action_display_local_badge | Local source |
| action_display_language_badge | Language |
| action_display_show_continue_reading_button | Continue watching/reading button |
| action_display_show_tabs | Show category tabs |
| action_display_show_number_of_items | Show number of items |
| pref_library_rows | List size |
| pref_library_columns | Items per row |
| overlay_header | Overlay |
| tabs_header | Tabs |
| action_filter_unread | Unread |
| action_filter_unseen | Unseen |
| action_filter_bookmarked | Bookmarked |
| action_filter_interval_custom | Customized update frequency |
| action_filter_tracked | Tracked |
| action_sort_alpha | Alphabetically |
| action_sort_total | Total chapters |
| action_sort_total_episodes | Total episodes |
| action_sort_last_read | Last read |
| action_sort_last_seen | Last seen |
| action_sort_last_anime_update | Last anime update |
| action_sort_last_manga_update | Last manga update |
| action_sort_unread_count | Unread count |
| action_sort_unseen_count | Unseen count |
| action_sort_latest_chapter | Latest chapter |
| action_sort_latest_episode | Latest episode |
| action_sort_chapter_fetch_date | Chapter fetch date |
| action_sort_episode_fetch_date | Episode fetch date |
| action_sort_date_added | Date added |
| action_sort_tracker_score | Tracker score |
| action_sort_airing_time | Airing time |
| action_sort_random | Random |
| action_select_all | Select all |
| action_select_inverse | Select inverse |
| action_move_category | Set categories |
| action_edit_categories | Edit categories |
| action_edit | Edit |
| action_open_random_manga | Open random entry |
| action_update_library | Update library |
| action_update_category | Update category |
| action_global_search_query | Search for "%1$s" globally |
| action_remove | Remove |
| anime_from_library | Anime from library |
| manga_from_library | Manga from library |
| downloaded_episodes | Downloaded episodes |
| downloaded_chapters | Downloaded chapters |
| action_resume | Resume |
| action_add | Add |
| action_add_category | Add category |
| action_rename_category | Rename category |
| action_hide | Hide |
| action_delete | Delete |
| action_cancel | Cancel |
| action_ok | OK |
| delete_category | Delete category |
| delete_category_confirmation | Do you wish to delete the category "%s"? |
| error_category_exists | A category with this name already exists! |
| information_required_plain | *required |
| information_empty_category | You have no categories. Tap the plus button to create one for organizing your library. |
| information_empty_category_dialog | You don't have any categories yet. |
| information_empty_library | Your library is empty |
| information_no_manga_category | Category is empty |
| information_no_entries_found | No entries found in this category |
| error_no_match | No match found |
| no_results_found | No results found |
| getting_started_guide | Getting started guide |
| no_next_chapter | Next chapter not found |
| update_already_running | An update is already running |
| updating_category | Updating category |
| general_categories | Categories |
| internal_error | InternalError: Check crash logs for further information |

Note the string `action_display_show_continue_reading_button` reads "reading" in the key but "Continue watching/reading button" in English. Desktop copy: "Continue button" (proposal, neutral wording).

---

## U-10. Gaps in the existing mockup (`src/screens.rs`, `src/ui.rs`)

1. No data layer: cards are placeholder labels from `title_n(i)`; the library is not loaded from a database.
2. Grid is a `FlowBox` with label-text filtering (`set_filter_func` on the label), not a model-driven search.
3. Mockup search matches only the visible label, not author/genre/source (B-6.2 requires them).
4. No list mode, no grid mode switch, no columns setting, no title overlay, no cover-only mode.
5. No per-item selection; selection bar is global and only reveals.
6. No tri-state filters; filters come from spec sheets.
7. No category tabs with counts, no keyboard paging, no swipe.
8. No continue-button behaviour (`open_media` placeholder).
9. No empty states with the upstream messages; no toasts.
10. Category screen: no drag reorder, no hide/show, no anime/manga split, no validation.
11. The mockup creates both anime and manga pages from one `library()` function (the `kind_stack` approach is reusable; keep the idea, replace internals).

Keep: the header structure (`page()`), the `kind_stack` switch between anime and manga, `icon_button`, `linked` for tabs, the action-bar revealer, and the `open_entry` / `dialog_of` plumbing for non-library screens.
