# Area 42 (part 2): UI parity, Updates and Upcoming

Scope files render UI, so this file is required. Behaviour rules are in `p2-behavior.md`; this file lists every visible element, state, dialog, snackbar, menu, gesture, animation, the desktop equivalent, responsive rules, and the gaps in the current mockup. Section numbers in `p2-behavior.md` are cited as "B§n".

Sources: `app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt`, `.../updates/anime/AnimeUpdatesTab.kt`, `app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt`, `.../updates/anime/AnimeUpdatesScreen.kt`, `.../updates/manga/MangaUpdatesUiItem.kt`, `.../updates/anime/AnimeUpdatesUiItem.kt`, `app/src/main/java/eu/kanade/presentation/updates/UpdatesDialog.kt`, and the eight upcoming files under `app/src/main/java/mihon/feature/upcoming/`.

Mockup references: `/home/blackbox/kurobayomi/spec/app.json` (ids `updates`, `updates_delete_episodes`, `updates_delete_chapters`), `/home/blackbox/kurobayomi/src/screens.rs:277` (`fn updates()`).

---

## 1. Updates screen (manga and anime, one screen per kind, selected by the top Anime | Manga switch)

### 1.1 Frame
- Top app bar (no search). Title: the tab title (`AYMR.strings.label_updates` / `label_anime_updates`). Note: the Anime | Manga switch (`kind_tabs`) is the top-level switch in the mockup; in Aniyomi the switch is the tab row (B§11).
- Back arrow: only when opened from More (`fromMore = true`). Action: pop, or open the Anime library when the last item is Home (B§11).
- Bottom bar: action menu, hidden when no selection (B§12.4).
- Snackbar host at the bottom, above the bottom bar.
- Content: pull-to-refresh container (disabled while selecting) around a fast-scroll list.

### 1.2 Toolbar
Normal mode (no selection), in order:
1. Calendar icon (`Icons.Outlined.CalendarMonth`), label "View Upcoming Updates". Opens the Upcoming screen for the same kind.
2. Refresh icon (`Icons.Outlined.Refresh`), label "Update library". Calls `updateLibrary()`.

Selection mode (at least one selected), in order:
1. SelectAll icon (`Icons.Outlined.SelectAll`), label "Select all". `toggleAllSelection(true)`.
2. FlipToBack icon (`Icons.Outlined.FlipToBack`), label "Select inverse". `invertSelection()`.

Mockup gap: the mockup toolbar (`toolbar(&s)` in `src/ui.rs:469`) shows generic actions. Needs the two normal actions and the two selection actions, and a close (X) control that clears selection (Aniyomi relies on the BackHandler, B§5.5).

### 1.3 List body
States:
| State | Condition | Rendering |
|---|---|---|
| Loading | `isLoading == true` | `LoadingScreen` (centred spinner) filling the content area with the padding |
| Empty | `!isLoading && items.isEmpty()` | `EmptyScreen` with text "No recent updates" (`MR.strings.information_no_recent`), centred. No action button. |
| Populated | `items` non-empty | list (below) |
| Error | `InternalError` event | Snackbar "InternalError: Check crash logs for further information". List state unchanged (Aniyomi keeps the last items; B§1.2 notes the flow stops, the Rust port keeps refreshing). |

Populated list, top to bottom:
1. "Library last updated: <relative time>" row. Italic. Padding horizontal 16 dp (`padding.medium`), vertical 8 dp (`padding.small`). Text from `relativeTimeSpanString(lastUpdated)` (B§6.5): "Never", "Just now", or relative. Scrolls with the list (not sticky).
2. Date group header (`ListGroupHeader`): text = `relativeDateText(date)` (B§6.2): "Today", "Yesterday", "N days ago", or the short date. One header per local date of `dateFetch`.
3. Item rows (section 1.4), in list order.

Gaps in the mockup: no loading state, no empty state, no "Library last updated" row (mockup shows a hard-coded "Library last updated: 5 minutes ago"), headers are hard-coded "Today / Yesterday / 3 days ago", rows are not from data, no selection, no download indicator.

### 1.4 Item row (Manga: `MangaUpdatesUiItem`; Anime: `AnimeUpdatesUiItem`)

Geometry: row height 56 dp, horizontal padding 16 dp (`padding.medium`), content vertically centred. Background tint when selected (`selectedBackground`).

Left to right:
1. Cover, `ItemCover.Square`: fills row height minus 6 dp top and bottom (`padding(vertical = 6.dp)`, `fillMaxHeight()`). Click opens the entry (manga: `MangaScreen(mangaId)`; anime: `AnimeScreen(animeId)`), only when NOT selecting (B§12.3).
2. Text column, `weight(1f)`, horizontal padding 16 dp:
   - Line 1: entry title, `bodyMedium`, 1 line, ellipsis. Colour alpha 0.38 when read/seen, else 1.0.
   - Line 2: inline row, vertically centred, in this order:
     a. Unread dot (`Icons.Filled.Circle`, 8 dp tall, end padding 4 dp, `colorScheme.primary`) when not read / not seen. Content description "Unread".
     b. Bookmark icon (`Icons.Filled.Bookmark`, `colorScheme.primary`) when bookmarked, then a 2 dp spacer. Description "Bookmarked".
     c. Fillermark icon (anime only; `Icons.AutoMirrored.Filled.Label`, `colorScheme.tertiary`) when fillermarked, then 2 dp spacer. Description "Fillermarked".
     d. Chapter or episode name, `bodySmall`, 1 line, ellipsis, `weight(1f, fill = false)`. Alpha 0.38 when read/seen.
     e. When progress text exists: dot separator (`DotSeparatorText`), then progress text in `DISABLED_ALPHA` (0.38), 1 line, ellipsis. Manga "Page: N" (`chapter_progress`, N = lastPageRead + 1). Anime "12:34/24:00" (`episode_progress`).
   - Icon max height = measured text height minus 2 dp (`sizeIn(maxHeight = textHeight - 2)`).
3. Download indicator (`ChapterDownloadIndicator` / `EpisodeDownloadIndicator`), start padding 4 dp. Enabled only when NOT selecting. States (owned by area:38-ui-entry-details-manga, area:39-ui-entry-details-anime): outline download arrow (NOT_DOWNLOADED), indeterminate ring (QUEUE), progress ring with percent (DOWNLOADING), filled check (DOWNLOADED), error icon (ERROR, retry). Tap opens its menu: Start / Cancel / Delete (B§3.1).

Gestures:
| Gesture | Not selecting | Selecting |
|---|---|---|
| Tap row | open chapter in reader (manga) or episode in player (anime, B§12.5) | toggle selection (userSelected = true, fromLongPress = false) |
| Long press row | start selection (userSelected = true, fromLongPress = true), with haptic on Android | same; range extends from the anchor |
| Tap cover | open entry | nothing (cover not clickable) |
| Tap download indicator | menu | disabled |

Desktop equivalents (`p2-rust-design.md` section 7): Ctrl+click toggles, Shift+click extends range, click on cover opens entry, Enter opens the update.

Text strings in row: none beyond data. Accessibility descriptions as listed above.

### 1.5 Bottom action bar (`EntryBottomActionMenu`, owned by area:38-ui-entry-details-manga, area:39-ui-entry-details-anime for the component; the set of actions is this scope's)
Shown when `selected.isNotEmpty()`. Items appear only when their predicate holds (B§12.4). Manga, in order:
1. Bookmark (when any not bookmarked).
2. Remove bookmark (when all bookmarked).
3. Mark as read (`onMarkAsViewedClicked`, when any unread).
4. Mark as unread (when any read or any with progress).
5. Download (when any not downloaded).
6. Delete (when any downloaded).

Anime, in order: Bookmark, Remove bookmark, Fillermark (when any not fillermarked), Remove fillermark (when all fillermarked), Mark as viewed (when any unseen), Mark as unviewed (when any seen or with progress), Download, Delete, Play externally (when always-external pref is off and exactly one selected), Play internally (when always-external pref is on and exactly one selected).

Label text is taken from `EntryBottomActionMenu` (area:38-ui-entry-details-manga, area:39-ui-entry-details-anime). Icons are the same set used there.

Desktop: the action bar becomes a `gtk::ActionBar` at the bottom of the window, or a toolbar row under the title when selecting. Buttons with the same labels and tooltips.

### 1.6 Dialogs

#### 1.6.1 Delete confirmation (`UpdatesDeleteConfirmationDialog`)
- Trigger: bottom bar Delete (B§4.3).
- Title/body: "Are you sure you want to delete the selected chapters?" (manga) or "Are you sure you want to delete the selected episodes?" (anime). Body text only, no title.
- Buttons: "OK" (primary, text button) runs delete then closes. "Cancel" (text button) closes.
- Dismiss by Escape or click outside = Cancel.
- Desktop: `gtk::AlertDialog` (GTK 4.10+) or `gtk::Window` modal with two buttons. Default button: Cancel (keyboard Enter should not delete). Mark Cancel as the default for safety.

#### 1.6.2 Episode qualities sheet (anime only, `ShowQualities`)
- Trigger: `EpisodeDownloadAction.SHOW_QUALITIES` on a single row (B§3.1).
- Content: `EpisodeOptionsDialogScreen` (area:39-ui-entry-details-anime), a bottom sheet on phone, a modal on desktop. Parameters: `useExternalDownloader`, `episodeTitle`, `episodeId`, `animeId`, `sourceId`.
- Dismiss: `onDismissDialog` sets dialog null.

### 1.7 Snackbars (Compose Material3 SnackbarHost, duration Short = 4000 ms)
| Trigger | Message |
|---|---|
| Library update started | "Updating library" |
| Library update already running | "An update is already running" |
| Internal error (flow failure) | "InternalError: Check crash logs for further information" |

Only one snackbar is visible at a time; new ones queue (Compose behaviour). Desktop: `gtk::Revealer` toast with the same texts, 4 s, queue of up to 3.

### 1.8 Pull to refresh
- Enabled only when not selecting.
- Gesture triggers `updateLibrary()`. If started, shows a spinner for 1 second (fake, B§10.2). If not started, no spinner.
- Desktop: no pull gesture. The Refresh toolbar button and F5 both call `updateLibrary()`. Show the spinner state only on the button (`gtk::Spinner` for 1 s) to mirror the 1 s behaviour, or omit. Decision: spinner on the button for 1 s.

### 1.9 Animations
- Row selection: `selectedBackground` changes instantly (no animation).
- Item insertion/removal: `Modifier.animateItemFastScroll` (fade/size) in Aniyomi (`FastScrollAnimateItem.kt`). Desktop: none (drop, `p2-behavior.md` section 14).
- List scroll: fast-scroll thumb with momentum. Desktop: GTK kinetic scrolling and overlay scrollbar.
- Bottom bar: `EntryBottomActionMenu(visible = ...)` uses Compose `AnimatedVisibility` (`expandVertically(expandFrom = Top) + fadeIn()`, the component is owned by area:38-ui-entry-details-manga, area:39-ui-entry-details-anime). Desktop: `gtk::Revealer` with `SlideUp`, 200 ms.
- Bottom navigation (the app's bottom bar) hides in selection mode (B§13). Desktop: no bottom bar exists in the Kurobayomi shell (the sidebar stays); keep the sidebar and do not hide it.

### 1.10 Responsive
- Phone and tablet use the same list. No two-panel layout on Updates.
- Desktop: width ≥ 720 px still one column list (rows are fixed 56 px). No change.

### 1.11 Keyboard and focus (desktop)
- Tab order: toolbar buttons, then list rows (`gtk::ListBox` focus), then bottom bar.
- Up/Down moves focus; Space toggles selection of focused row (as tap in selection mode); Shift+Up/Down extends the range.
- Enter opens the focused update.
- Escape clears selection (replaces BackHandler, B§5.5).
- Delete key with selection = Delete action (confirm dialog first).
- Ctrl+A select all, Ctrl+I invert.

### 1.12 Tooltips and accessibility
- Every icon-only button has a tooltip equal to its label above (for the toolbar and bottom bar).
- Row accessible name: "<title>, <episode or chapter name>, <progress if any>, unread|read, bookmarked if bookmarked". GTK `update_property(AccessibleProperty::Label)`.

---

## 2. Upcoming screen (manga and anime, same layout; `mihon/feature/upcoming/*/`)

### 2.1 Frame
- Top app bar: title "Upcoming" (`MR.strings.label_upcoming`). Navigation: back arrow (`navigator::pop`). Action: help icon (`Icons.AutoMirrored.Outlined.HelpOutline`), description "Upcoming Guide", opens `https://aniyomi.org/docs/faq/updates/upcoming`.
- Body: calendar and upcoming list (layouts in 2.3).
- No empty state. When there are no upcoming items, the calendar still shows and the list area is blank (B§7, observed). Keep this parity (no invented empty text). Proposal: none.
- No snackbar, no dialog, no selection, no bottom bar, no pull-to-refresh.

### 2.2 Calendar (`Calendar`, composable)
Elements, top to bottom:
1. Month header row (section 2.4).
2. Week-day header row: 7 cells, narrow weekday names (for English "M T W T F S S" for Monday-first; "S M T W T F S" for Sunday-first), 16 sp, SemiBold, centred. Names from `DayOfWeek.getDisplayName(NARROW, Locale)`.
3. Day grid: 7 columns. Leading blank cells (count = position of the 1st weekday in the week order), then day cells 1..N.

Grid width: full width; if the window width is > 600 dp and ≤ 840 dp, max width 360 dp (B§9).
Vertical spacing between header and grid: 4 dp. Between month header and week-day row: 4 dp.

Day cell (`CalendarDay`):
- Size: square, diameter = max(text width, text height) (circular hit area).
- Today: 1 dp ring in `onBackground` (circle border). Today is computed on composition (B§8.3), a Rust port recomputes on each render.
- Past days (before today): number text at alpha 0.38. Today and future: full alpha.
- Number: 16 sp, SemiBold, centred.
- Event dots: row placed 12 dp below the number's centre-line offset (`offset(y = 12.dp)`), min(count, 3) dots. Dot i: diameter 56/12 dp (≈4.67 dp), 1 dp horizontal padding, colour `colorScheme.primary` at alpha (i+1)*0.3 (0.3, 0.6, 0.9).
- Click: calls `onClickDay(date)` (B§7.6). Ripple: circular, default clickable ripple. Desktop: hover state (background tint at 8% of primary) and focus ring.

Day without events: no dots. Day with events on a past date (before today): still has dots at the same alpha, number dimmed.

### 2.3 Layout
Phone (`isTabletUi()` false, smallest width < 720 dp):
- One scroll column. Item 0: calendar (key `upcoming-calendar`). Then date headers and item rows (2.5). Fast-scroll list.
- Day click scrolls the list to the header of that day with the calendar index offset (offset 1, B§7.6).

Tablet and desktop (two panels):
- Left: calendar (`TwoPanelBox.startContent`). Aniyomi `TwoPanelBox` (`presentation-core/src/main/java/tachiyomi/presentation/core/components/TwoPanelBox.kt:31`) gives the start panel `min(width / 2, 450 dp)`. Desktop: `gtk::Paned` with the start panel at `min(content_width / 2, 450 px)` initially, resizable.
- Right: list of headers and rows (`endContent`), fast-scroll list, no calendar inside.
- Day click scrolls the right list to the header index (offset 0).

Desktop rule (B§9 proposal): two panels when the content width ≥ 720 px; single column below.

### 2.4 Month header (`CalenderHeader`)
- Row: title on the left, prev/next buttons on the right.
- Title: "<Month name> <Year>" (`MMMM yyyy`, e.g. "October 2026"), `titleLarge` style. Animated on change (2.6).
- Prev button: left chevron (`Icons.Default.KeyboardArrowLeft`), description "Previous Month". Click: previous month.
- Next button: right chevron (`Icons.Default.KeyboardArrowRight`), description "Next Month". Click: next month.
- Padding: vertical 8 dp (`padding.small`), start 16 dp (`padding.medium`) on the row.
- No "Today" button exists in Aniyomi. Parity: do not add one. Proposal (not required): keyboard shortcut `Home` returns to the current month. Recorded as an optional addition, default off.

### 2.5 Date header and item row
Date header (`DateHeading`):
- Row, vertically centred, full width.
- Text: `relativeDateText(date)` (B§6.2) for upcoming dates: "Today", "Tomorrow", "In N days" (N ≤ 7), or the short date. `bodyMedium`, SemiBold, colour `onSurfaceVariant`, padding 8 dp, start 8 dp.
- Badge: count of items on that date, `Badge` with `primary` container and `onPrimary` content. Text: the number.

Item row (`UpcomingItem`):
- Height 96 dp. Horizontal padding 16 dp, vertical 8 dp. Spacing 24 dp (`padding.large`) between cover and text.
- Cover: `ItemCover.Book` (book aspect ratio), `fillMaxHeight()`. Source: `asMangaCover()` / `asAnimeCover()` (area:34-images-covers-cache).
- Title: `bodyMedium`, SemiBold, max 2 lines, ellipsis, `weight(1f)`.
- Click: opens the entry (`MangaScreen(id)` / `AnimeScreen(id)`).
- No other elements (no progress, no bookmark, no status text). Parity.
- Desktop: hover highlight, keyboard Enter opens.

### 2.6 Animations
- Month title change: `AnimatedContent` with `getAnimation()` (B§8.2). Duration 200 ms each for slide and fade. Forward: enter from below, exit upward. Backward: reverse. Size transform without clip.
- Desktop: `gtk::Stack` with `SlideUp` (forward) / `SlideDown` (backward), `transition-duration` 200 ms. Only the title label is inside the stack, the grid swaps instantly (Aniyomi does not animate the grid).
- Day jump: `animateScrollToItem` (smooth scroll). Desktop: `gtk::ScrolledWindow` `scroll_to` with an animated adjustment (`adw` not available; use a 250 ms adjustment tween or instant jump).

### 2.7 Keyboard and mouse (desktop)
- Left/Right arrow in the calendar: previous/next day (Proposal). Up/Down: previous/next week.
- PageUp/PageDown: previous/next month (Proposal).
- Enter on a focused day: scroll to that day's header.
- Click on a day: same. Click on a day with no header: no scroll (Aniyomi).
- Prev/next buttons: clickable and focusable.
- Escape: back (navigator pop).

### 2.8 Help link
- Click help icon: opens `https://aniyomi.org/docs/faq/updates/upcoming` in the default browser (`gtk::UriLauncher`). Tooltip "Upcoming Guide".

---

## 3. Upcoming states

| State | Condition | Rendering |
|---|---|---|
| Initial | state before first subscription emission | calendar with current month, no dots, empty list (Aniyomi shows the same) |
| Populated | list non-empty | dots and list |
| Empty | list empty | calendar with no dots, empty list, no text |
| Error | flow failure | Aniyomi: no handler; the list stays at its last value (no catch in UMSM/UASM). Rust: show toast "InternalError: Check crash logs for further information" and keep last list. (Proposal, recorded.) |

---

## 4. Mockup gap analysis

### 4.1 Updates (`/home/blackbox/kurobayomi/src/screens.rs:277` `fn updates()`, spec ids in `/home/blackbox/kurobayomi/spec/app.json`)

| Requirement | Mockup today | Needed |
|---|---|---|
| Data | hard-coded `title_n(i)`, `Episode N` strings | live rows from `UpdatesStore` (p2-rust-design 3.1-3.2) |
| Date headers | hard-coded "Today", "Yesterday", "3 days ago" | `group_by_day` with relative date (time_fmt) |
| Last updated line | hard-coded "Library last updated: 5 minutes ago" | `relative_time_span(lastUpdated, now)` |
| Row layout | `list_row` with a 40×40 thumb and a download button | 56 px row, cover 40 px wide, two-line text with icons and progress, download indicator |
| Read styling | none | alpha 0.38 for read/seen rows |
| Unread dot / bookmark / fillermark | "●" glyph in subtitle | proper icons; fillermark anime only |
| Progress text | `12:34/24:00` and `Page: 5` fixed | from data via `fmt::format_progress` |
| Selection | none | Selection engine, Ctrl/Shift/Escape, selection toolbar, action bar |
| Bottom action bar | none | section 1.5 |
| Delete confirm | none in `updates()` (the only dialogs in the mockup are for history) | `updates_delete_chapters` / `updates_delete_episodes` from spec |
| Empty state | none | "No recent updates" |
| Loading state | none | spinner |
| Toolbar | generic `toolbar(&s)` | View Upcoming Updates, Update library, Select all/inverse |
| Open entry on cover | none | open entry page |
| Open update on row | `open_media(anime, title, unit)` | reader or player routing (B§12.5) |
| Pull to refresh | none | Refresh button + F5 |
| Anime play routing | none | internal/external per pref (B§12.5) |
| Snackbar | none | toast with three messages |
| Counter reset | none | reset on enter and leave |

Note: the mockup's `updates()` is called from `screens.rs:22` for the `updates` id. Keep the id.

### 4.2 Upcoming (no mockup screen exists)
The mockup has no Upcoming page, no calendar widgets, and no route from the Updates toolbar. Everything in section 2 is missing. The mockup spec `spec/app.json` line ~503 has the toolbar entry "View Upcoming Updates" (`x-office-calendar-symbolic`), so the action exists in the spec but has no target.

---

## 5. Icon mapping (Material → GTK symbolic icon names)

Use GTK symbolic icons with the Adwaita/breeze names available on KDE. The user's Tokyo Night theme recolours them.

| Aniyomi icon | Use | GTK symbolic name (fallback) |
|---|---|---|
| CalendarMonth (outlined) | View Upcoming Updates | `x-office-calendar-symbolic` (already in spec) |
| Refresh (outlined) | Update library | `view-refresh-symbolic` |
| SelectAll (outlined) | Select all | `edit-select-all-symbolic` |
| FlipToBack (outlined) | Select inverse | `object-flip-horizontal-symbolic` |
| Circle (filled, 8 dp) | Unread dot | drawn with Cairo (no icon) |
| Bookmark (filled) | Bookmarked | `bookmark-new-symbolic` |
| Label (filled) | Fillermarked | `tag-symbolic` |
| KeyboardArrowLeft / Right | Prev / next month | `go-previous-symbolic` / `go-next-symbolic` |
| HelpOutline | Upcoming guide | `help-browser-symbolic` |
| Download (indicator) | Download states | owned by area:38-ui-entry-details-manga, area:39-ui-entry-details-anime |

Colours: all icon colours come from the theme (`primary` = accent, `tertiary` = secondary accent). In CSS: `.updates-unread { color: @accent_color; }`, `.updates-fillermark { color: @warning_color; }`.

---

## 6. Typography and spacing tokens

| Token | Value | Use |
|---|---|---|
| `padding.small` | 8 dp | vertical padding in month header, date header padding |
| `padding.medium` | 16 dp | horizontal row padding, last-updated row padding |
| `padding.large` | 24 dp | spacing between cover and text in upcoming row |
| Row height (updates) | 56 dp | |
| Row height (upcoming) | 96 dp | |
| Cover square inset | 6 dp vertical | updates row |
| Calendar grid spacing | 4 dp | Column spacing |
| Dot size | 56/12 dp | calendar |
| Calendar max width (medium) | 360 dp | window 600–840 dp |
| Month transition | 200 ms | |
| Snackbar duration | 4000 ms | |
| Library-update fake spinner | 1000 ms | pull-to-refresh only |
| Disabled alpha | 0.38 | read rows, past days, progress text |
| Font: list title | body medium | |
| Font: list subtitle | body small | |
| Font: calendar number | 16 sp SemiBold | |
| Font: month title | title large | |
| Font: date header | body medium SemiBold | |

Desktop scale: the values above are logical px at 1x. GTK scales them with the monitor scale factor, so use px as written and let the GTK scale apply.

---

## 7. Interaction summary table (Aniyomi → Kurobayomi)

| Interaction | Aniyomi | Kurobayomi |
|---|---|---|
| Tap row (normal) | open chapter/episode | same |
| Tap row (selecting) | toggle | same |
| Long press row | start selection, range on second long press | same; Shift+click range on desktop |
| Tap cover | open entry (normal only) | same |
| Pull down | refresh library | Refresh button / F5 |
| Back | clear selection, else pop | Escape clears selection, else pop |
| Bottom bar | action menu | action bar |
| Calendar day tap | scroll to header | same |
| Month prev/next | change month | same, plus keyboard PgUp/PgDn |
| Help | open URL | open URL |
| Snackbar | 4 s | 4 s toast |
