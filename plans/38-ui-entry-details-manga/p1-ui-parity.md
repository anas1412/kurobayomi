# Area 38 part 1: UI parity (manga entry details)

Every visible element, state, dialog, menu, gesture and responsive rule from the Aniyomi scope files, the Linux equivalent, and what the existing Kurobayomi mockup lacks.

Mockup reference: `/home/blackbox/kurobayomi/src/entry.rs`, function `entry(title, anime)` (lines 81-242). Shared helpers in the same file: `screen`, `margins`, `cover`, `dim`, `heading`, `boxed_list`, `row`, `menu_button`, `icon_button`, `icon_label_button`, `page`, `tag`, `open`, `dialog_of`, `sheet_of`, `open_media`.

Legend: **Aniyomi** = what the Android app shows; **Linux** = the Proposal for Kurobayomi; **Mockup gap** = what `src/entry.rs` lacks.

---

## 1. Screen frame

| Element | Aniyomi | Linux (proposal) | Mockup gap |
|---|---|---|---|
| Top bar | `EntryToolbar`: title (fades in when info box scrolls away, small layout only), action icons, overflow menu (`presentation/entries/components/EntryToolbar.kt`) | Custom top bar (no libadwaita): title label, icon buttons, overflow `gtk::PopoverMenu`. Honour the window's titlebar if present. | Mockup has `page(title, body, end, true)` with a `menu_button` overflow; no title fade, no action-mode bar in the top bar. |
| Title fade | alpha 0→1 when first item not visible; background alpha 0→1 when first item not visible or scrolled (presentation MangaScreen.kt:295-308) | `gtk::Adjustment` on the scrolled window; title `opacity` = clamp(…) 150 ms transition | Missing. |
| Bottom action bar | `EntryBottomActionMenu` (only when selection non-empty), slides up from bottom (`expandVertically`) | Bottom `gtk::Revealer` with `transition-type = SlideUp`, duration 200 ms | Mockup: `selbar` revealer exists; it is shown on right-click (`GestureClick` button 3) and reveals on any row. Does not hide when selection becomes empty. Needs real selection state. |
| FAB "Start"/"Resume" | `ExtendedFloatingActionButton`, bottom end; label depends on read state; visible only when an unread item exists and no selection | Bottom-right `gtk::Button` pill, `suggested-action`; same visibility rule | Mockup always shows "Resume" and always opens chapter 20. Missing visibility and label rules. |
| Snackbar | `SnackbarHost` with dismiss action | In-window toast overlay (section 6) | Missing. |
| Pull to refresh | `PullRefresh` indicator with padding per layout; disabled while selection non-empty | Spinner row in header + `Ctrl+R` and toolbar "Refresh". Pointer pull is not required. | Missing. |
| Scrollbar | `VerticalFastScroller` (fast drag scroll) | GTK overlay scrollbar (default), plus a fast-scroll handle is optional (P2) | GTK default scrollbar. Acceptable. |
| Loading | `LoadingScreen()` until source registry ready and while `State.Loading` | Centered `gtk::Spinner` + label "Loading…" (exact string not in Aniyomi; use `MR` loading key if one exists in area:36) | Missing. |
| Empty chapter list | Not a separate screen. The list is empty, header shows "0 chapters" (plural `manga_num_chapters` with 0 → "0 chapters") | Same. Show the header and an empty-state label "No chapters" only as a proposal, labeled as new. | Mockup always has 24 rows. |
| Error on refresh | Snackbar with `formattedMessage` | Toast with the same message | Missing. |

---

## 2. Info header

| Element | Aniyomi behaviour | Linux (proposal) | Mockup gap |
|---|---|---|---|
| Backdrop | Cover image, `Crop`, blur 4dp, alpha 0.2, gradient transparent→background (MangaInfoHeader.kt:118-135) | `gtk::Picture` in a `gtk::Overlay` child, CSS `opacity: 0.2`, `filter: blur()` is not supported in GTK 4 CSS, so use a pre-blurred pixbuf (gaussian via `image` crate, radius ~4 px at 1x) cached per cover. Gradient as CSS `linear-gradient`. | Missing backdrop. |
| Cover | `ItemCover.Book` (2:3 aspect), crossfade, click → full cover dialog, `sizeIn(maxWidth = 100.dp)` small, `fillMaxWidth(0.65f)` large | `gtk::Picture` with `content-fit: cover`, aspect 2:3, click → full cover window; small max width 100 px at 1x, large 65% width | Mockup: cover(130, 190) fixed; click → `dialog_of(id, "full_cover")`. Size differs; should be responsive. |
| Cover placeholder | Solid `#1F888888` tile, error image `cover_error` | CSS background `rgba(136,136,136,0.12)`; error icon `image-missing` | Missing placeholder/error. |
| Title | `titleLarge`; "Unknown title" if blank; click = global search; long-press = copy | `gtk::Label` `title-1`, wrap; click = search; right-click or long-press = copy | Mockup: title label only, no click actions. |
| Author | Icon `PersonOutline`; "Unknown author" fallback; click = global search; long-press = copy | Label with `avatar-default-symbolic` icon (mockup has this icon for Author) | Mockup shows "Author" as static text ("Studio" for anime) — placeholder, not real author. Missing click/copy. |
| Artist | Shown only if non-blank and ≠ author; `Brush` icon | Same rule | Missing. |
| Status | Icon by status + text ("Ongoing" etc.) | Icon map + text | Mockup hard-codes "Ongoing". Missing status map. |
| Source name | Click = non-global search; stub-source warning icon | Label with `network-server-symbolic`; click = search; warning icon `dialog-warning-symbolic` for stub source | Mockup: "Source A (EN)". Missing click and stub warning. |
| Dot separator | " • " between status and source | Same text | Missing. |

---

## 3. Action row

| Element | Aniyomi | Linux (proposal) | Mockup gap |
|---|---|---|---|
| Library button | Label "Add to library"/"In library"; icon Favorite/FavoriteBorder; primary color when favorite; long-press → edit categories (favorite only) | Button with `emblem-favorite-symbolic`; label toggles; secondary-click/long-press → categories | Mockup toggles label only; no long-press; no real favorite state; no category flow. |
| Interval button | Label "N/A"/"Soon"/"N days"; `HourglassEmpty`; primary when user interval; click → interval dialog (favorite only) | `alarm-symbolic`; label from `nextUpdateDays` | Mockup: static "2 days" → `dialog_of(set_fetch_interval)`. Label not computed. |
| Tracking button | Label "Tracking" or "N trackers"; `Sync`/`Done` icon; click → tracker sheet, or to settings when no logged-in tracker | `emblem-synchronizing-symbolic`; same rules | Mockup: "Tracking" → `open("tracking_sheet")`, always opens sheet (no logged-in check, no count). |
| WebView button | Only for HTTP source; click open; long-press copy URL | `web-browser-symbolic`; shown only for HTTP sources | Mockup always shows WebView. |

Layout: four equal-weight buttons across the width with 16 px horizontal padding and 8 px top padding (Aniyomi). Mockup uses an `actions` `GBox` with buttons appended (no equal weights).

---

## 4. Description and tags

| Element | Aniyomi | Linux (proposal) | Mockup gap |
|---|---|---|---|
| Description, collapsed | 3-line visual height via "\n\n" spacer; ellipsis not used; full text clipped; gradient scrim 24dp at bottom with caret | `gtk::Label` `lines=3`, ellipsize end, scrim overlay 24 px | Mockup: `lines(3)`, ellipsize end, toggles to `-1`. Scrim missing (mockup uses a "More" button). Mockup text is placeholder. |
| Expand control | Tap anywhere on description toggles; caret icon animates; content description "More"/"Less" | Click on label toggles; caret button for keyboard (`Enter`) | Mockup: separate "More" button. Both exist, which is acceptable; the description click is missing. |
| Expanded state persistence | `rememberSaveable` (session only); default expanded = `isFromSource` (small) or `true` (large) | Store per-entry in memory for the session | Missing default rule. |
| Whitespace collapse | `[\r\n]{2,}` → `\n` when collapsed | Same regex in `regex` crate | Missing. |
| Selectable text | `SelectionContainer` (text is selectable) | `gtk::Label` `selectable = true` (note: conflicts with click-to-toggle; use the caret button for toggling while text is selectable; proposal) | Missing. |
| "No description" | `description_placeholder` text when blank | Same string | Missing. |
| Genre chips | Collapsed: horizontal LazyRow; expanded: wrapping FlowRow; chip click → menu Search / Copy | Collapsed: horizontal `gtk::ScrolledWindow`; expanded: `gtk::FlowBox`; click → `gtk::PopoverMenu` with "Search", "Copy to clipboard" | Mockup: `gtk::FlowBox` with fixed genres `Action, Adventure, Comedy, Fantasy`; menu has Search/Copy stubs (`|_| {}`). Genres are not real manga data. Missing collapsed/expanded modes. |

---

## 5. Chapter list

| Element | Aniyomi | Linux (proposal) | Mockup gap |
|---|---|---|---|
| Header | "N chapters" (plural `manga_num_chapters`) or "Chapters" when count is null; clickable to open filter/sort sheet (disabled during selection); "Missing N items" warning below in error color | `gtk::Label` title-3 + click → sheet; warning label | Mockup: `heading("24 chapters")` with fixed count; no click, no missing warning. |
| Row title | "Chapter N" when display mode = number (formatted with `DecimalFormat("#.###")`, HALF_EVEN); else chapter name | Same | Mockup: `Chapter {n}` fixed. |
| Unread dot | 8dp primary dot before title when unread | Same | Mockup: "● " text prefix in subtitle when unread. Visual mismatch. |
| Bookmark icon | `Bookmark` filled, primary, before title | Same | Missing. |
| Read dimming | Title and subtitle alpha `DISABLED_ALPHA` when read | Opacity 0.6 on the row (mockup does this for read rows) | Mockup uses `set_opacity(0.6)`. Close enough; prefer CSS class. |
| Subtitle | date • "Page: N" (if partially read) • scanlator; dots between present fields only | Same | Mockup: fake "M/D/2026 • Scanlator". Missing progress and optional dots. |
| Row height | Padding 16/12/8/12 dp, min content height by text | Same paddings in px at 1x | Mockup uses `row()` default. |
| Download indicator | Right side, 48 dp state layer; states NOT_DOWNLOADED / QUEUE / DOWNLOADING / DOWNLOADED / ERROR (section 7.5 of behavior) | `gtk::Button` flat, 48 px, icons: `folder-download-symbolic`, spinner or ring (`gtk::DrawingArea` with arc), `emblem-ok-symbolic`, `dialog-error-symbolic`; menus as `gtk::PopoverMenu` | Mockup: `menu_button("folder-download-symbolic", ["Download","Delete"])` for every row, always. Missing all five states and progress. |
| Swipe | Start/end swipe actions with icons and 56 dp threshold | Drag gesture on row with action background; keyboard equivalents | Missing. |
| Long-press | Enter selection, select this row | Right-click or long-press → select this row | Mockup: right-click shows selection bar but does not select a row. Missing selection. |
| Click | Opens reader when not in selection; toggles when in selection | Same | Mockup: `open_media` (player/reader mock). |
| Keyboard | (Android: none) | Up/Down/Enter/Space/Ctrl+A/Esc (behavior section 16) | Missing. |

---

## 6. Snackbar / toast

- Aniyomi: `SnackbarHost` bottom center, message plus optional action; `withDismissAction = true` for cover messages.
- Linux proposal: `gtk::Overlay` child at bottom center, `gtk::Revealer` (SlideUp, 200 ms), label, optional action button, dismiss `×` icon. Auto-dismiss after 4 s when there is no action (proposal; Android's `SnackbarDuration.Short` is ~4 s, verify with the Compose source when available).
- Messages used in this area: "Cover saved", "Error saving cover", "Cover updated", "Failed to update cover", "Error sharing cover", "Delete downloaded chapters?" (with action "Delete"), "Chapter settings updated" (key `chapter_settings_updated`), fetch errors (dynamic), "Copied" is not an Aniyomi string; the copy feedback in Aniyomi is the platform clipboard toast. Proposal: show "Copied to clipboard" as a new string (labeled new).
- Missing in mockup entirely.

---

## 7. Dialogs and sheets (by trigger)

| Dialog / sheet | Trigger | Aniyomi content | Linux (proposal) | Mockup state |
|---|---|---|---|---|
| Filter/Sort/Display sheet (`ChapterSettingsDialog`) | Filter icon, header click | Tabs "Filter" / "Sort" / "Display"; overflow "Set as default", "Reset"; TriState rows Downloaded/Unread/Bookmarked; Scanlator row; sort rows with arrows; radio rows | Modal `adw`-free `gtk::Window` (transient) with `gtk::Notebook` tabs or `gtk::StackSwitcher`; `gtk::CheckButton` tri-state (three-state via `inconsistent` or three toggle buttons) | `sheet_of(id, "chapter_settings")` exists; content in `screens.rs` not verified in this pass. |
| Set as default | overflow in filter sheet | "Chapter settings" title, text, checkbox "Also apply to all manga in my library", Cancel/OK | `gtk::AlertDialog` (GTK 4.10+) or custom modal | `dialog_of`? Not in mockup; the sheet is `chapter_settings`. Missing. |
| Scanlator exclude | Scanlator row | Title "Exclude scanlators"; list with checkboxes; "Select all"/"Reset", Cancel, OK; "No scanlators found" when empty | Modal with `gtk::ListBox` of `gtk::CheckButton`; sorted case-insensitive | Not in mockup. |
| Delete chapters confirm | Bulk Delete, when downloads selected | "Are you sure?" + "Are you sure you want to delete the selected chapters?" OK/Cancel | `gtk::AlertDialog` | `dialog_of(id, "delete_items")` present in mockup. Missing real text and effect. |
| Interval | Interval button (favorite only) | "Smart update" title; expected-update text; (debug build only) wheel 0..MAX with "Default" at 0; OK/Cancel | `gtk::SpinButton` 0..MAX (debug only) + text | `dialog_of(id, "set_fetch_interval")` present. Content not verified. |
| Change categories | Library long-press / overflow "Edit categories" | `ChangeCategoryDialog` (area:08 / area:37) | Same dialog from area:37 | `dialog_of(id, "set_categories")` present. |
| Duplicate entry | Add to library when a duplicate exists (part 2 decides when) | "Are you sure?", "You have an entry in your library with the same name.\n\nDo you still wish to continue?"; three rows (Show entry, Migrate existing entry, Add anyway); Cancel | Modal sheet with three `gtk::Button` flat rows and a Cancel button | Not in mockup. |
| Migrate | Overflow "Migrate" (favorite only) or duplicate "Migrate existing entry" | Migrate search (area:37/38-part2) | Navigation to migrate search | `dialog_of(id, "migrate_manga")` present. |
| Track sheet | Tracking button | Tracker info sheet (part 2) | Sheet | `open("tracking_sheet")` present. |
| Full cover | Cover click | Full-screen image with zoom, close pill, share/save pill, edit pill with menu (Edit / Delete when custom) | `gtk::Window` fullscreen-capable, `gtk::Picture` with zoom via scroll and `GestureZoom`, header bar pills with buttons | `dialog_of(id, "full_cover")` present. Edit/delete/save/share missing. |
| Source preferences | Overflow "Settings" (ConfigurableSource only) | Pushes source preferences | Navigation | `open("source_preferences")` present; always shown in mockup. |
| Skip-intro length | Anime only | n/a | n/a | `dialog_of(id, "skip_intro_length")` anime only. |

Dialog dismissal (Aniyomi): `onDismissRequest` runs `dismissDialog()`. If `autoOpenTrack && isFromChangeCategory`, the track sheet opens right after (chain). Port this chain exactly.

---

## 8. Menus

| Menu | Items (in order) | Conditions | Mockup |
|---|---|---|---|
| Download dropdown (toolbar + library bar) | Next 1 chapter, Next 5, Next 10, Next 25, Unread (labels from `EntryDownloadDropdownMenu`, unverified; see data doc 7.6) | Shown in toolbar when `onClickDownload != null` (not local) | Mockup: `menu_button` with those five labels (`"Next {unit}"`, `"Next 5 {unit}s"`, ...). Labels match the mockup, not verified against Compose. |
| Overflow (toolbar) | Refresh; Edit categories; Migrate; Share; Settings | Each conditional (section 5.2 of behavior) | Mockup: Edit categories, Migrate, Settings (with `open`/`dialog_of`). Missing Refresh and Share, and the conditions. |
| Chapter download (downloading) | Start downloading now; Cancel | Click on downloading indicator | Missing. |
| Chapter download (downloaded) | Delete | Click on downloaded indicator | Missing. |
| Bottom bar | Bookmark / Unbookmark / Mark read / Mark unread / Mark previous read / Download / Delete (conditions in behavior 7.7) | Selection non-empty | Mockup: `sel` box with 5 icon buttons + delete + close. Missing per-item conditions and labels. |
| Tag menu | Search; Copy to clipboard | Chip click | Mockup: stubs (`|_| {}`). |
| Cover edit menu | Edit; Delete | Cover is custom | Missing. |
| Scanlator | (dialog, not menu) | Scanlator row | Missing. |

---

## 9. Gestures and pointer

| Gesture | Aniyomi | Linux (proposal) | Mockup |
|---|---|---|---|
| Tap row | open/toggle | open/toggle | open |
| Long-press row | select, range-select (part 2) | long-press or right-click select | right-click shows bar only |
| Swipe row | start/end actions (56 dp threshold) | drag with the same threshold; both directions | Missing |
| Tap title/author/artist/source | global search / non-global search | same | Missing |
| Long-press title/author/artist/tag | copy | right-click / long-press copy | Missing |
| Tap cover | full cover | same | `dialog_of(full_cover)` |
| Long-press library button | categories | right-click / long-press | Missing |
| Long-press WebView button | copy URL | right-click / long-press | Missing |
| Long-press download indicator | START_NOW (not downloaded) / CANCEL (downloading) / START (error) | right-click / long-press same mapping | Missing |
| Pull to refresh | refresh | none (use Ctrl+R) | Missing |
| Back | clear selection if any, else pop | Escape (behavior 16) and Alt+Left/back button | GTK nav default |

---

## 10. Responsive behaviour

- Aniyomi: `isTabletUi()` picks the large two-panel layout (behavior section 2). Port: same breakpoint from area:36, and two panels when wide.
- Small: header block (cover 100 px max + text) then action row, description, genres, header, list.
- Large: left column 1/2 width (info + actions + description), right column the list.
- Cover: small width max 100 px; large 65% of left column.
- Action row: four equal buttons in both layouts.
- Mockup: `clamp(&col, 760)` — a fixed 760 px column. Replace with the two layouts above.

---

## 11. Keyboard, focus, accessibility

- Aniyomi (Compose): semantics via `contentDescription` on icons (e.g. "Unread", "Bookmarked", "Download", "Error", "Cover", "More", "Less", "Close", "Edit cover"). Keep the same strings as accessible labels on Linux (`gtk::Accessible` label) for every icon-only button in this area.
- Proposal: every chapter row focusable; focus ring from theme; Enter/Space activation.
- The mockup gives rows `set_activatable(true)` (Enter on ListBox row); keep, and add keyboard selection (section 16 of behavior).

---

## 12. Theme and colors

- Aniyomi uses Material 3 color roles: `primary` (active), `onSurface` (disabled alpha 0.38-ish), `surfaceContainerHigh` (bottom bar), `background` (backdrop gradient end), `error` (missing count, stub warning, download error), `surfaceContainerLowest` (swipe background), `primaryContainer` (swipe action background).
- Linux: the user's GTK theme applies (requirement: vanilla GTK, no libadwaita). Use named CSS colors `@accent_color` (primary), `@window_bg_color` (background), `@card_bg_color` (surface containers), `@error_color` (error), `@theme_fg_color` (on-surface) so dark/light and the user's accent follow the desktop.
- Alpha constants: `DISABLED_ALPHA` and `SECONDARY_ALPHA` from presentation-core (not read in this pass). Proposal: 0.38 and 0.7 as CSS opacities, labeled as proposal until presentation-core is read.

---

## 13. Motion

| Motion | Aniyomi | Linux |
|---|---|---|
| Bottom bar show/hide | `expandVertically` / `shrinkVertically` from bottom | `gtk::Revealer` SlideUp, 200 ms |
| FAB show/hide | `fadeIn` / `fadeOut` | `gtk::Revealer` crossfade, 150 ms |
| Description expand | `animateFloatAsState` + `animateContentSize(spring())` | `gtk::Revealer` or height transition 200 ms |
| Caret | `AnimatedImageVector` | Rotate 180° via CSS transform, 200 ms (proposal) |
| Title fade | `animateFloatAsState` default | 150 ms opacity |
| Swipe | spring | follow the finger; release at threshold → action; snap back below threshold |
| Download ring | `animateFloatAsState(ProgressAnimationSpec)` | interpolate arc over 300 ms when progress changes |
| Row weight (bottom bar) | `animateFloatAsState` weight 1→2 when confirm | Not needed on Linux (no confirm reveal; see E-12), keep label reveal for 1 s on long-press if the port keeps the tooltip behaviour. |

Reduced motion: Linux GTK honours `gtk-enable-animations`. When it is false, use zero durations.

---

## 14. What the mockup lacks (summary, actionable)

1. Real data model: chapters, flags, selection, download state, missing count (no hard-coded 24 rows).
2. Chapter ordering/filter/sort engine and sort/display/filter sheet content (behavior 6, 8.2).
3. FAB rules (Start vs Resume, visibility).
4. Title fade and background alpha on scroll.
5. Download indicator state machine (five states, menus, progress ring).
6. Swipe actions with settings.
7. Selection model: long-press/right-click selects; range select; select all/invert; Esc; bottom bar actions with conditional visibility.
8. Missing-chapter warning and separators.
9. Responsive two-panel layout.
10. Info header: author/artist/status/source with search/copy actions; stub-source warning; artist-hidden-when-equal rule.
11. Description: default expansion rules, whitespace regex, scrim, selectable text, "No description".
12. Tag chips: collapsed/expanded modes, Search/Copy menu.
13. Action row: library long-press to categories; interval label from dates; tracking count and login gate; WebView only for HTTP source.
14. Cover dialog: zoom, share, save, edit (pick file), delete custom, edit menu with conditions.
15. Duplicate entry dialog and its three actions.
16. Scanlator exclude dialog.
17. Set-as-default dialog with library-wide option.
18. Interval dialog: "Smart update", expected text, debug-only wheel.
19. Snackbar/toast system and messages.
20. Loading, empty, error states.
21. Keyboard shortcuts (new, proposal).
22. Overflow menu conditions (favorite-only, HTTP-only, configurable-only).
23. Pull-to-refresh replacement (Ctrl+R and menu).
