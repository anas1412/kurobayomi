# 25-downloads-shared — UI parity

Source of truth: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt (page), plus the row/menu resources and holders it hosts (AnimeDownloadHolder.kt, MangaDownloadHolder.kt, app/src/main/res/menu/download_single.xml). Behaviour detail is in p1-behavior.md; this file is the visible surface and its states.

Mockup under review: /home/blackbox/kurobayomi/src/screens.rs `fn downloads()` (line 354) and `fn download_button()` (line 134); spec entry `download_queue` in /home/blackbox/kurobayomi/spec/app.json (line 732).

---

## 1. Screen anatomy (Kotlin, OBSERVED)

From top to bottom, in DownloadsTab.kt:

1. **Top app bar** (lines 134-166), pinned (does not collapse on scroll).
   - Left: back arrow (`navigateUp = navigator::pop`).
   - Title: "Download queue" (`MR.strings.label_download_queue`), one line, ellipsis.
   - Count pill to the right of the title, ONLY when anime queue count > 0. Text = count. Font 14 sp. Colour = onBackground at alpha 0.12 (dark) / 0.08 (light). Padding start 4 dp.
   - Actions, depend on the current pager page:
     - Anime page: Sort (icon `Sort`, label "Sort") and overflow "Cancel all". Shown only when the anime list is non-empty.
     - Manga page: same two actions for manga. Shown only when the manga list is non-empty.
2. **Floating action button** (lines 167-230), bottom end. Shown only when the current page's list is non-empty. Fade in/out. Expanded (icon + label) or collapsed (icon only) per scroll.
3. **Tab row** (lines 239-267), `PrimaryTabRow` (full-width, underline indicator). Two tabs: "Anime" with badge = anime queue count, "Manga" with badge = manga queue count. Tapping a tab scrolls the pager.
4. **Pager** (lines 269-289), two pages. Each page is a list of source sections.
5. **Per-page body**: the list, with section headers and rows; the empty state from `EmptyScreen(information_no_downloads)` when the list is empty (AnimeDownloadQueueScreen.kt:38-41, MangaDownloadQueueScreen.kt:38-42), with padding from the Scaffold content padding.

Pages are independent: each has its own list and own state; the FAB and the actions follow the current page.

---

## 2. Visible elements and their texts

| Element | Text / icon | Condition |
|---|---|---|
| Title | "Download queue" | always |
| Anime count pill (title) | number | anime count > 0 |
| Sort action | icon `Icons.AutoMirrored.Outlined.Sort`, title "Sort" | page list non-empty |
| Overflow "Cancel all" | "Cancel all" | page list non-empty |
| Tab "Anime" | "Anime" (AYMR) + badge | always |
| Tab "Manga" | "Manga" (AYMR, not "Library entries") + badge | always |
| Badge | `Pill`, font 10 sp, same alpha rule as title pill | always (even "0", see 5.1) |
| FAB (anime, running) | "Stop", icon `Pause` | anime running |
| FAB (anime, stopped) | "Continue", icon `PlayArrow` | anime not running |
| FAB (manga, running) | "Pause", icon `Pause` | manga running |
| FAB (manga, stopped) | "Resume", icon `PlayArrow` | manga not running |
| Sort menu, first row | "By upload date" (submenu) | |
| Sort submenu | "Newest", "Oldest" | |
| Sort menu, second row | "By episode number" (anime) / "By chapter number" (manga) (submenu) | |
| Sort submenu | "Ascending", "Descending" | |
| Section header | source name | one per source in queue |
| Row title | episode name (anime, `binding.chapterTitle.text = download.episode.name`) / chapter name (manga, `download.chapter.name`) | AnimeDownloadHolder.kt:41, MangaDownloadHolder.kt:38 |
| Row subtitle | anime title (`download.anime.title`) / manga title (`download.manga.title`) | AnimeDownloadHolder.kt:44, MangaDownloadHolder.kt:41 |
| Row progress text | anime: `Downloading…` when progress is 0, else `NN%`; manga: `downloadedImages/pages` or empty when pages unknown | see 3.4 |
| Row menu | six items, see 4 | |
| Empty state | "No downloads" | page list empty |

Section headers: Kotlin uses the source name only (`AnimeDownloadHeaderItem(entry.key.id, entry.key.name, entry.value.size)`); the count is `subItems.size` per section. Proposal: "Source name (count)" as in the mockup.

---

## 3. States

### 3.1 Loading
None. The queue state is an in-memory `MutableStateFlow` initialised to an empty list (AnimeDownloadQueueScreenModel.kt:25), so a fresh page shows the empty state until the first emission (synchronous from `queueState`). Proposal: render the empty state directly; no spinner.

### 3.2 Empty
- Per page: `EmptyScreen("No downloads")`.
- FAB hidden; sort and cancel-all hidden; count pill hidden (anime) and badges show "0" (see 5.1).

### 3.3 Non-empty, idle (not running)
- FAB: "Continue" (anime) / "Resume" (manga), play icon.
- Rows show their state; a row in `QUEUE` state shows no progress.

### 3.4 Non-empty, running
- FAB: "Stop" (anime) / "Pause" (manga), pause icon.
- Rows in `DOWNLOADING` state show progress that updates.
- Anime row (AnimeDownloadHolder.kt:62-84):
  - `notifyProgress()`: if `download.progress == 0` → progress bar indeterminate; else indeterminate off and `setProgressCompat(download.progress, true)` on a bar whose max is 100 (set at the first call if it was 1).
  - `notifyDownloadedPages()`: text = `MR.strings.update_check_notification_download_in_progress` ("Downloading…") when progress == 0; otherwise `AYMR.strings.episode_download_progress` = `%1$d%%` → "NN%".
- Manga row (MangaDownloadHolder.kt:43-72):
  - Bar `max = pages.size * 100` (set when pages are known; progress set from per-page sums); when pages are null → progress 0, text "".
  - Text = `"${download.downloadedImages}/${pages.size}"` (for example "12/30").
- The mockup texts "45%" (anime) and "12/30" (manga) match these two formats.
- Manga rows subscribe to per-page progress (`launchProgressJob`, 50 ms poll + 50 ms debounce). Anime rows update only on status changes (see behaviour 4.7).

### 3.5 Row states
| Download state | Row visual (Kotlin, observed by holder behaviour) | Rust |
|---|---|---|
| NOT_DOWNLOADED (0) | not normally in queue | row shown with progress text from the holder rules above (progress 0) |
| QUEUE (1) | progress 0 → anime "Downloading…" text with indeterminate bar (the holder does not distinguish QUEUE) | same as Kotlin |
| DOWNLOADING (2) | progress text and bar update (3.4) | same as Kotlin |
| DOWNLOADED (3) | the models call `cancelProgressJob` then one last `onUpdateProgress` (behaviour 4.7); removal from the list is the engine's job (area:23/24) | same as Kotlin |
| ERROR (4) | `cancelProgressJob` only; no further progress updates from the model | same as Kotlin; the holder shows no dedicated error visual in this scope. Proposal: an error suffix icon `dialog-warning-symbolic` (not in Kotlin, new) |

Note: the Kotlin progress job is cancelled on ERROR, so an error row does not keep updating. Rust mirrors this.

### 3.6 Drag state
- Dragging a row: the row follows the pointer (Compose reorder handle; adapter `onItemReleased` on release). Rust: GTK DragSource; the drop indicator is GTK's built-in highlight on the target `ListBox` row. No Kotlin drop indicator exists; this is a Proposal.

### 3.7 Error and failure states
- No error UI exists in DownloadsTab for queue operations. Delete failures are logged (area:23/24), not shown. Rust: same; log with `tracing::error!` and keep the list correct by taking the engine snapshot after the failure.

---

## 4. Row menu (OBSERVED)

Opened from the row's overflow button (`view-more` icon, `R.menu.download_single`). Order and labels:

| # | Label (English) | Action | Visibility |
|---|---|---|---|
| 1 | Move to top | move item to index 0 of its section, reorder engine | shown when `bindingAdapterPosition > 1` (Kotlin); Rust: when flat index > 0 |
| 2 | Move series to top | move all items of the same series to the front of the flat queue | always |
| 3 | Move to bottom | move item to end of its section | shown when `bindingAdapterPosition != itemCount - 1`; Rust: when flat index < last |
| 4 | Move series to bottom | move all items of the same series to the end | always |
| 5 | Cancel | cancel this download (no file delete) | always |
| 6 | Cancel all for this series | cancel all download items of the same series in the current page | always (the action is a no-op when no items match) |

Rust mapping: `gio::Menu` with the six items; visibility by setting the `gio::MenuItem` out (rebuild the menu on open) because `gio::Menu` has no per-item visibility.

---

## 5. Sort and overflow menus (OBSERVED)

### 5.1 Badges and count pill
- Tab badge: always shown (Kotlin passes a non-null Int). Proposal: hidden at 0.
- Title pill: shown only when anime count > 0 (Kotlin). Proposal: keep this rule (anime only); manga count is not in the title (documented as-is).

### 5.2 Sort menu (anime and manga identical in shape)
Click on Sort (icon `view-sort-*`) opens:
```
By upload date        >   Newest     (reverse = true,  key = dateUpload)
                          Oldest     (reverse = false, key = dateUpload)
By episode/chapter    >   Ascending  (reverse = false, key = number)
number                    Descending (reverse = true,  key = number)
```
Each action sorts each section (source group) and sends the flat result to the engine. Sort applies to the queue order, so the engine downloads in that order.

### 5.3 Overflow
"Cancel all" calls `clearQueue()` immediately (no confirmation dialog). Engine: clear queue and stop (anime and manga; manga does not pause first; the difference is internal).

---

## 6. Keyboard and mouse (desktop equivalents)

Kotlin is touch-first. The Linux equivalents:

| Action | Kotlin (touch/long-press) | Rust (desktop) |
|---|---|---|
| Open row menu | tap overflow button | tap overflow button; Menu key or Shift+F10 while row focused |
| Reorder | drag handle | drag handle with mouse; Alt+Up / Alt+Down while focused (Proposal) |
| Cancel one | menu item | menu item; Delete key while focused (Proposal) |
| Cancel series | menu item | menu item |
| Switch Anime/Manga | tap tab or swipe pager | click tab; Ctrl+PageUp / Ctrl+PageDown (Proposal); horizontal swipe on touchpad via `GestureSwipe` (P2) |
| Start / pause | FAB | FAB click; Space while the page is focused (Proposal) |
| Sort | sort icon | sort icon; menu accelerator none |
| Scroll | list scroll | scroll wheel; FAB collapse on content scrolling up |

All interactive widgets must be reachable with Tab. Row buttons must have tooltips: "More" (overflow), and each row-menu item uses its menu label as tooltip ("Move to top", "Move series to top", "Move to bottom", "Move series to bottom", "Cancel", "Cancel all for this series").

---

## 7. Responsive behaviour

- Kotlin: `Scaffold` padding handling for system bars; list content is full width; FAB is inset by the bottom padding.
- Rust: the window may be narrow (phone-size or tiled). Rules:
  - Title truncates with ellipsis (`set_ellipsize(Pango::EllipsizeMode::End)`).
  - Sort and overflow menus stay in the header; they do not collapse into a second bar.
  - Row title and subtitle ellipsise; the progress text is right-aligned and never wraps.
  - FAB collapses to icon-only below 480 px content width (Proposal), regardless of scroll.
  - Tab labels "Anime" / "Manga" with badges stay on one line.

---

## 8. Animations and motion

| Kotlin | Rust |
|---|---|
| `AnimatedVisibility(fadeIn/fadeOut)` for FAB | `gtk::Revealer` with `transition-type: crossfade`, duration 200 ms (Proposal) |
| `rememberAnimatedVectorPainter(anim_history_enter)` tab icon | none; static icon (P3 drop, see p1-rust-design.md section 9) |
| `ExtendedFloatingActionButton` expand/collapse | label `set_visible` with `gtk::Revealer` `transition-type: slide-left`, 150 ms (Proposal) |
| Progress bar updates (no animation spec in scope) | `gtk::ProgressBar` set_fraction, no transition |
| Pager scroll (`animateScrollToPage`) | `gtk::Notebook` `set_current_page`; GTK default animation |

---

## 9. Dark / light and theme

- Kotlin: Material 3 colours plus `isSystemInDarkTheme()` for the pill alpha.
- Rust: GTK theme applies (no libadwaita). Pill colour via CSS `alpha(@theme_fg_color, 0.08)`; dark theme contrast comes from the GTK theme, no separate dark rule.
- Icons: freedesktop symbolic names only (see p1-rust-design.md section 9).

---

## 10. Accessibility

- Kotlin: `contentDescription = null` on the FAB icon (text is the label).
- Rust: FAB button has an accessible label (the text); progress bar `set_accessible_role` default; row buttons have accessible names matching their tooltips.
- Badge counts should be read as part of the tab label: accessible name "Anime, 3 downloads" (Proposal).

---

## 11. What the existing mockup lacks

Mockup (src/screens.rs:354-402) vs Kotlin:

| Gap | Detail |
|---|---|
| Fake data | "Source A" (3) and "Source B" (2) with hard-coded titles and 45% / 12/30 progress; no real queue |
| Tab badges | none; Kotlin shows counts on both tabs |
| Title count pill | none; Kotlin shows anime count pill |
| Sort menu shape | mockup: four flat labels ("By upload date: Newest", ...). Kotlin: two submenus with 2 + 2 children |
| Overflow | mockup has a Sort toolbar button only; Kotlin has "Cancel all" in the overflow next to Sort |
| Row menu | mockup: six labels (including "Cancel" and "Delete" variants); Kotlin: six fixed items with visibility rules and the exact strings in section 4 |
| Row visibility rules | mockup shows all items always; Kotlin hides Move-to-top on the first row and Move-to-bottom on the last |
| FAB | mockup: Stop/Continue label only for anime; manga label "Pause"/"Resume" present; no hide-when-empty; no collapse-on-scroll; no running-state icon swap (mockup toggles only text) |
| FAB visibility | mockup always visible; Kotlin hides when page list empty |
| Empty state | mockup none; Kotlin "No downloads" |
| Drag reorder | mockup none |
| Source headers | mockup "Source A (3)" format matches Kotlin's header item only partly; Kotlin shows source name, count from subItems |
| Pill text colour | mockup none |
| Tab labels | mockup: "Anime"/"Manga" (matches); Kotlin uses AYMR "Manga" (matches) |
| Move-series actions | none in mockup |
| Status-driven progress | mockup static |
| Error row | none |
| Running state | mockup FAB text toggles only on click; Kotlin reads `isDownloaderRunning` from the engine |

---

## 12. Parity checklist (for review)

- [ ] Title "Download queue" with anime count pill (anime > 0 only).
- [ ] Two tabs with badges; labels "Anime" and "Manga".
- [ ] Empty state "No downloads" per page; FAB and actions hidden when empty.
- [ ] FAB label and icon follow running state per kind (Stop/Continue; Pause/Resume).
- [ ] FAB collapses on content scrolling up and expands otherwise.
- [ ] Sort menu: two submenus, four leaves, semantics per section, reverse on equal keys.
- [ ] Overflow "Cancel all" with no confirmation.
- [ ] Section per source with name and count.
- [ ] Row progress text formats: anime `NN%`, manga `done/total`.
- [ ] Row menu: six items, exact labels, visibility rules.
- [ ] Drag reorder across sections allowed; flat order sent to engine.
- [ ] Error rows stop updating.
