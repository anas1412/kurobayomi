# Area 38 part 1: implementation tasks (manga entry details)

Task IDs use the prefix T-38a-. Every scope file and every behaviour in `p1-behavior.md` maps to at least one task (see the mapping in `p1-inventory.md`). Priorities follow the project rule: P0 = daily reading usability, P1 = parity most users rely on, P2 = long tail and polish, P3 = Android-only drop or replacement decision.

Dependency tokens: `T-38a-nnn` (this file) or `area:<id>`.

### T-38a-001 — Manga details route and loading gate
- Priority: P0
- Effort: S
- Depends: area:17-source-app-layer, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Opening a manga id routes to the details page; the page shows a centered loading state until the source registry has loaded (equivalent of ifMangaSourcesLoaded, ui MangaScreen.kt:82-85).
  - While the model is Loading, only the loading placeholder is shown; no toolbar actions are interactive.
  - The `fromSource` flag is passed to the model and is observable (used by T-38a-015).
  - Routing reuses `ui::push`; the mockup id `manga_entry_details` still resolves through `crate::ui::screen`.

### T-38a-002 — EntryDetailsModel and store protocol
- Priority: P0
- Effort: L
- Depends: T-38a-001, area:05-data-repos-entries-items, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - A GTK-free `EntryState` holds manga, source info, chapters, available and excluded scanlators, tracking count, login flag, refreshing flag, dialog, downloaded-only and skip-filtered values, mirroring `State.Success` (MangaScreenModel.kt:1144-1155).
  - `StoreCmd` and `StoreEvent` enums exist; commands run on tokio; snapshots are delivered to the GTK thread through async-channel with latest-wins coalescing.
  - Selection is owned by the model (`selected_ids`) and survives snapshots (MangaScreenModel.kt:563).
  - Subscriptions are dropped on page pop; flag writes are not cancelled on page pop (launchNonCancellable semantics).
  - Unit test: applying two snapshots in a row keeps the selected flags for chapter ids that still exist.

### T-38a-003 — ChapterFlags codec, SortMode, TriState
- Priority: P0
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/SetMangaChapterFlags.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt
- Acceptance:
  - Constants match Manga.kt:75-103 exactly (masks 0x1, 0x6, 0x18, 0x60, 0x300, 0x100000; values as in data doc section 2).
  - `sort_descending()` is true when bit 0 is 0.
  - `select_sort` flips when the mode is unchanged and sets ASC when the mode changes (SetMangaChapterFlags.kt:47-68).
  - `with(value, mask)` never changes bits outside the mask (test with bits 10-19 and 21-63 set).
  - `TriState::next` cycles DISABLED→ENABLED_IS→ENABLED_NOT→DISABLED; `keep` implements applyFilter.

### T-38a-004 — Chapter sort comparator and collation
- Priority: P0
- Effort: M
- Depends: T-38a-003, area:13-core-util-storage-archive-torrent
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt
- Acceptance:
  - The 4×2 table in behavior section 6.3 is implemented literally (source/number/date/alpha × desc/asc), including the inverted SOURCE naming.
  - Sorting uses a stable sort; equal keys keep input order.
  - Alphabetical uses an ICU primary-strength collator for the user locale; accents and case compare equal at primary strength; test with Japanese, Arabic, Cyrillic and accented Latin fixtures.
  - NaN chapter numbers never panic (total_cmp).

### T-38a-005 — Chapter view pipeline (filter, sort, local rules)
- Priority: P0
- Effort: M
- Depends: T-38a-003, T-38a-004, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt
- Acceptance:
  - Order is unread → bookmarked → downloaded → sort (MangaScreenModel.kt:1196-1222).
  - Downloaded filter keeps everything for local manga; the downloaded-only preference forces EnabledIs.
  - `processed` and `chapters` are both available; `selected_total` and `selected_visible` are computed separately (edge E-04).
  - A 10 000 item fixture processes under 20 ms in release.

### T-38a-006 — Missing chapter markers and header warning
- Priority: P1
- Effort: M
- Depends: T-38a-005
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/components/ItemHeader.kt, app/src/main/java/eu/kanade/presentation/entries/components/MissingItemCountListItem.kt
- Acceptance:
  - Markers are produced per behavior section 6.4, with the id format `"{lower|null}-{higher}"` and the descending swap.
  - Header count = `missing_chapters_count` over processed numbers (ignores exact -1.0, truncates fractions, dedups).
  - Warning text uses the plural `missing_items` ("Missing 1 item" / "Missing N items") and is hidden at 0.
  - Test cases from edge-cases 4.3 pass, including the descending case.

### T-38a-007 — Chapter number formatting
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt
- Acceptance:
  - `format_chapter_number` matches Java `DecimalFormat("#.###")` with HALF_EVEN on the exact double for: 1.0→"1", 1.5→"1.5", -1.0→"-1", and every value in the generated fixture (edge E-07).
  - -0.0, NaN and infinity match Java output captured once and stored in the fixture.

### T-38a-008 — Chapter row widget
- Priority: P0
- Effort: M
- Depends: T-38a-005, T-38a-007
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaChapterListItem.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Title switches between chapter name and "Chapter N" by display mode (MangaScreen.kt:790-797).
  - Unread dot (8 px, primary), bookmark icon (primary), read rows dimmed to the disabled alpha, title one line with ellipsis.
  - Subtitle shows date, "Page: N" only when `!read && lastPageRead > 0` (value N = lastPageRead + 1), and scanlator; separators appear only between present parts; blank scanlator hidden.
  - Row paddings 16/12/8/12 px at 1x.

### T-38a-009 — Chapter list widget
- Priority: P0
- Effort: M
- Depends: T-38a-005, T-38a-006, T-38a-008
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - A `gtk::ListView` backed by a list store; rows keyed `chapter-{id}` and missing markers keyed `missing-count-{id}`.
  - Header row shows "N chapters" (plural) and the missing warning; clicking the header opens the filter/sort sheet, disabled while any row is selected (ItemHeader enabled = !isAnySelected).
  - Updating one row's download state patches in place (no full rebuild); scroll position survives snapshots.
  - Empty processed list shows the count "0 chapters" and no markers (edge E-02); an optional hint is labelled as a new proposal string.

### T-38a-010 — Row open, selection model and gestures
- Priority: P0
- Effort: M
- Depends: T-38a-002, T-38a-009
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - Click outside selection opens the reader for (manga id, chapter id); click in selection mode toggles; click on selected row deselects (presentation MangaScreen.kt:842-853).
  - Long-press (and right-click on Linux) selects the row and enters selection mode (E-51, E-52).
  - Select all, invert, and clear are available; clearing selection leaves selection mode.
  - Range selection behaviour matches part 2 (`selectedPositions`), verified with a fixture of 10 rows.

### T-38a-011 — Resume / Start floating button
- Priority: P0
- Effort: S
- Depends: T-38a-005, T-38a-009
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Visible iff processed has an unread item and nothing is selected (total count) (MangaScreen.kt:346-348).
  - Label "Resume" iff unfiltered chapters contain a read item, else "Start" (MangaScreen.kt:355-361).
  - Click opens `getNextUnreadChapter()`; does nothing when it is null (ui MangaScreen.kt:307-309).
  - Fade in/out over 150 ms; expanded label follows `shouldExpandFAB()`, whose exact rule lives in presentation-core and must be read before implementing (not read in this pass).

### T-38a-012 — Info header: cover, title, author, artist, source
- Priority: P0
- Effort: M
- Depends: T-38a-002, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - Small layout: cover max 100 px beside text; large layout: cover 65% of left column, centered text (MangaInfoHeader.kt:336-414).
  - Title blank → "Unknown title"; author blank → "Unknown author"; artist shown only when non-blank and different from author.
  - Click title → global search; click author → global search (non-blank); click artist → global search; click source → non-global search; long-press/right-click copies the text (non-blank only).
  - Stub source shows a warning icon in error color next to the source name.
  - Cover click opens the full cover (T-38a-037).

### T-38a-013 — Status icon and text mapping
- Priority: P0
- Effort: S
- Depends: T-38a-012
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - Status values 1..6 map to the icon and text in behavior section 3.4; 0 and unknown values map to Block icon and "Unknown".
  - `expected_next_update` is None when status == 2 (used by T-38a-018).
  - Icon names are a proposal mapped from Material icons; verified present in the installed icon theme with `gtk::IconTheme::has_icon` at startup.

### T-38a-014 — Backdrop blur and gradient
- Priority: P2
- Effort: S
- Depends: T-38a-012
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - A cover-derived backdrop at alpha 0.2 with a transparent→window-background gradient is drawn behind the header.
  - Blur uses a pre-blurred image (radius about 4 px at 1x) cached per cover; the header remains readable at 200% scale.

### T-38a-015 — Description: expand/collapse, whitespace rule, defaults
- Priority: P0
- Effort: M
- Depends: T-38a-012
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Collapsed shows 3 lines with a bottom 24 px scrim; expanded shows all text; the caret button and the text both toggle (E-31).
  - Blank description shows "No description".
  - `[\r\n]{2,}` collapses to `\n` and `trim_end` is applied in the collapsed text only.
  - Default expanded: small layout = `fromSource`, large layout = true (E-61).
  - Expanded state lasts for the page session only.

### T-38a-016 — Genre chips with search and copy menu
- Priority: P1
- Effort: S
- Depends: T-38a-015
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - Collapsed: horizontal row; expanded: wrapping row; hidden entirely when genres are null or empty (E-70).
  - Chip click opens a menu with "Search" (global-false search per performGenreSearch rules, T-38a-041) and "Copy to clipboard".
  - Chips never overflow the window width (E-71).

### T-38a-017 — Action row: library, interval, tracking, WebView
- Priority: P0
- Effort: M
- Depends: T-38a-012, T-38a-018, T-38a-032, T-38a-040
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Four equal-weight buttons, 16 px horizontal and 8 px top padding.
  - Library button label "In library" / "Add to library" with filled/outlined icon and primary/disabled color; click toggles favorite; long-press opens categories only when favorite (E-13).
  - Interval button: click opens the interval dialog only when favorite (E-14); color primary for user interval (fetch_interval < 0).
  - Tracking button label "Tracking" or "N trackers"; no logged-in tracker → opens tracking settings, else opens the tracking sheet (E-25).
  - WebView button is shown only for HTTP sources (E-26).

### T-38a-018 — Next update label and time rules
- Priority: P1
- Effort: S
- Depends: T-38a-013
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt, app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt
- Acceptance:
  - `days = max(0, trunc((next - now) in days))`; null → "N/A"; 0 → "Soon"; otherwise "1 day" / "N days" (plural).
  - Test: +36 h → 1; +23 h → "Soon"; past → "Soon"; completed → "N/A" (E-16, E-60).

### T-38a-019 — Top toolbar with title fade, actions and overflow
- Priority: P0
- Effort: M
- Depends: T-38a-011, T-38a-036
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/EntryToolbar.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Small layout: title alpha 1 only when the first item is not visible; background alpha 1 when first item not visible or scrolled; large layout always opaque (MangaScreen.kt:295-308, 567-568).
  - Actions in order: Download menu (non-local), Filter (tinted when filter active), overflow.
  - Overflow order: Refresh (always), Edit categories (favorite), Migrate (favorite), Share (HTTP), Settings (ConfigurableSource).
  - Action mode (selected_visible > 0) shows the count and only "Select all" and "Select inverse".

### T-38a-020 — Back and Escape behaviour
- Priority: P0
- Effort: S
- Depends: T-38a-010
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Back and Escape with any selection (selected_total > 0) clear selection and do not pop.
  - Otherwise they pop the page.
  - Test with a filtered-out selection: back still clears it (E-04).

### T-38a-021 — Filter / Sort / Display sheet
- Priority: P0
- Effort: M
- Depends: T-38a-003, T-38a-005
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/ChapterSettingsDialog.kt
- Acceptance:
  - Three tabs "Filter", "Sort", "Display"; overflow menu "Set as default", "Reset".
  - Filter: Downloaded, Unread, Bookmarked tri-state rows cycling per TriState.next; Scanlator row with active tint.
  - Downloaded row is disabled when the downloaded-only preference is on (E-15).
  - Sort: four rows (source, number, upload date, alphabetical) with arrow only on the active row; clicking calls `select_sort` (flip or ASC).
  - Display: two radio rows (title, chapter number) writing DISPLAY_MASK only.
  - Sheet content is spec-driven where the spec already has it (spec/entries.json sheets `chapter_settings`).

### T-38a-022 — Set as default and reset
- Priority: P1
- Effort: S
- Depends: T-38a-021, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/ChapterSettingsDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - Confirm dialog with title "Chapter settings", text "Are you sure you want to save these settings as default?", checkbox "Also apply to all manga in my library" (default unchecked), Cancel and OK.
  - OK stores current flags as library default; checkbox also applies to all manga (part 2 write).
  - Success shows the `chapter_settings_updated` message (text to verify in part 2).
  - Reset applies library defaults to this manga.

### T-38a-023 — Scanlator exclusion dialog and filter row
- Priority: P1
- Effort: M
- Depends: T-38a-021, T-38a-002
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/ScanlatorFilterDialog.kt, app/src/main/java/eu/kanade/presentation/entries/manga/ChapterSettingsDialog.kt
- Acceptance:
  - Title "Exclude scanlators"; list sorted case-insensitively; each row toggles membership in the excluded set.
  - Empty available set → "No scanlators found" and only Cancel (E-42).
  - Footer "Select all" when excluded set is empty (adds all available), otherwise "Reset"; then Cancel and OK; OK writes the excluded set.
  - `scanlator_filter_active` is an intersection test (E-43); stale excluded names are kept.

### T-38a-024 — Bulk action bottom bar
- Priority: P1
- Effort: M
- Depends: T-38a-010, T-38a-025
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Visible iff selected_visible > 0; slides up 200 ms; fill 100% small, 50% large.
  - Buttons in order with exact conditions: Bookmark (any not bookmarked), Unbookmark (all bookmarked), Mark read (any unread), Mark unread (any read or progress > 0), Mark previous (exactly one selected), Download (non-local and any not downloaded), Delete (any downloaded) (behavior 7.7).
  - Tap executes immediately; long-press reveals the label for 1000 ms; a new long-press cancels the previous timer (E-12).
  - Each button has an accessible name equal to its label.

### T-38a-025 — Download indicator state machine
- Priority: P1
- Effort: M
- Depends: T-38a-008, area:23-downloads-manga, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt
- Acceptance:
  - Five states map to visuals per behavior 7.5; progress ring with arrow color switch at 0.5 (fraction clamped to [0,1], E-39).
  - Click and long-press map per `download::outcome` table (test all 10 combinations).
  - Indicator is disabled while any row is selected and for local manga (E-27).
  - Menus: downloading → "Start downloading now", "Cancel"; downloaded → "Delete"; not downloaded and error have no menu.

### T-38a-026 — Chapter swipe actions
- Priority: P1
- Effort: M
- Depends: T-38a-008, T-38a-027
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaChapterListItem.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Start (left-to-right) and end (right-to-left) actions from preferences; `Disabled` disables a side.
  - Icons per `swipe::visual` (toggle read: Done / RemoveDone; bookmark: BookmarkAdd / BookmarkRemove; download: Download / FileDownloadOff / Delete).
  - Drag past 56 px fires the action; below the threshold the row snaps back.
  - Download swipe on a downloaded chapter calls the Delete path (E-64).

### T-38a-027 — Swipe preference wiring
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - Start side reads key `pref_chapter_swipe_start_action` with default ToggleBookmark; end side reads `pref_chapter_swipe_end_action` with default ToggleRead.
  - A unit test asserts these keys and defaults (guards the cross-wired getter names).
  - Enum values ToggleRead, ToggleBookmark, Download, Disabled are stored as their names.

### T-38a-028 — Fetch interval dialog
- Priority: P1
- Effort: S
- Depends: T-38a-018
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Title "Smart update"; body shows the expected-update text when days known and interval ≥ 0, else the null text.
  - The wheel (0..MAX_INTERVAL, 0 = "Default") appears only in debug builds and only when interval changes are allowed (E-46, E-47); initial index = |interval| when interval < 0, else 0 (E-45).
  - OK stores the interval as its negative (behavior 8.7); Cancel dismisses.
  - MAX_INTERVAL is read from the domain interactor (verify value in part 2).

### T-38a-029 — Duplicate-entry sheet
- Priority: P1
- Effort: S
- Depends: T-38a-002
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/DuplicateMangaDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Title "Are you sure?" and body "You have an entry in your library with the same name.\n\nDo you still wish to continue?".
  - Rows: "Show entry" (pushes the duplicate page on top), "Migrate existing entry" (opens migrate flow), "Add anyway" (adds without duplicate check); Cancel button.
  - Each row dismisses the sheet first (E-24).

### T-38a-030 — Delete chapter downloads confirmation
- Priority: P1
- Effort: S
- Depends: T-38a-010, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Title "Are you sure?", body "Are you sure you want to delete the selected chapters?", OK and Cancel.
  - OK clears selection then deletes the chapters passed in the dialog state.

### T-38a-031 — Category change trigger and auto-open tracking chain
- Priority: P1
- Effort: M
- Depends: T-38a-017, area:08-domain-library-category-history-updates, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Opens the change-category dialog with the manga's current selection; OK moves the manga to the chosen categories and adds it to library.
  - "Edit categories" in the dialog opens the categories page.
  - When auto-open tracking is on and the change came from this flow, the tracking sheet opens after dismissal (E-74).

### T-38a-032 — Tracking trigger
- Priority: P1
- Effort: S
- Depends: T-38a-017, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - No logged-in tracker → open tracking settings; otherwise open the tracking sheet (part 2 owns the sheet).
  - Button label reflects the tracker count (0 → "Tracking", N → "N trackers").

### T-38a-033 — Migrate trigger and replace on completion
- Priority: P1
- Effort: S
- Depends: T-38a-029, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Overflow "Migrate" is shown only for favorites (E-65) and opens the migrate search for this manga id.
  - When migration finishes, the old details page is replaced by the new one (E-23).
  - Clicking the old title inside the migrate dialog opens the old entry.

### T-38a-034 — In-window toast / snackbar host
- Priority: P1
- Effort: M
- Depends: T-38a-001
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt
- Acceptance:
  - Overlay at bottom center; one visible message at a time; optional action button; dismiss icon; auto-dismiss 4 s when there is no action (labelled proposal).
  - Carries messages: cover saved / errors, cover updated, failed cover update, fetch errors, clipboard copy confirmation (new string, labelled).

### T-38a-035 — Refresh from source
- Priority: P1
- Effort: S
- Depends: T-38a-002, area:38-ui-entry-details-manga, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Toolbar "Refresh", Ctrl+R, and the pull gesture where available all call the same action; the header shows a spinner while `refreshing`.
  - First open refreshes info when `!initialized` and chapters when the list is empty, in parallel; spinner clears after both finish.
  - HTTP 103 errors are ignored; other errors show a toast with the message.
  - Refresh is disabled while any row is selected for the pull gesture (E-04 total counter).

### T-38a-036 — Small and large layout switch
- Priority: P1
- Effort: M
- Depends: T-38a-009, T-38a-012, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Breakpoint is the same width threshold as `isTabletUi()` (owned by area:36).
  - Large layout: info column (1/2 width) and list column; small: one column.
  - Switching keeps the scroll offset of the list and the selection (E-59).

### T-38a-037 — Full cover window
- Priority: P2
- Effort: M
- Depends: T-38a-038, T-38a-039, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Full-screen window with zoom (pinch, ctrl+wheel), close on click of the image background and Escape.
  - Pill bars: close; share, save; edit (menu Edit/Delete when custom cover; direct edit otherwise).
  - Edit opens a file picker filtered to `image/*`; cancel is a no-op.
  - Original-size image load; zoom transition 500 ms.

### T-38a-038 — Cover model (save, share, edit, delete)
- Priority: P2
- Effort: M
- Depends: T-38a-034, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt
- Acceptance:
  - Save writes to the pictures location and shows "Cover saved" or "Error saving cover"; share writes to the cache and opens the share path or clipboard fallback, error "Error sharing cover".
  - Edit: null stream shows the failure message (E-18); success shows "Cover updated"; failure "Failed to update cover".
  - Delete custom cover updates `cover_last_modified` and shows the same messages.
  - Animated covers: static frame only (source TODO at MangaCoverScreenModel.kt:102).

### T-38a-039 — Cover placeholder and error rendering
- Priority: P2
- Effort: S
- Depends: T-38a-012
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt
- Acceptance:
  - Covers use aspect ratios Square 1:1, Book 2:3, Thumb 16:9 and are cropped to fill.
  - Placeholder is a flat tile with color 0x1F888888 (12 % gray); failure shows the cover-error icon.
  - Clickable covers have the button role and a focus ring.

### T-38a-040 — Share, copy and WebView actions
- Priority: P2
- Effort: S
- Depends: T-38a-041, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - Share (HTTP only) copies the URL to the clipboard with a toast (Linux replacement; labelled proposal) and optionally opens the portal share.
  - Copy URL (WebView long-press) guards exceptions (E-17).
  - WebView opens the in-app WebView page with the URL and manga title, or the default browser when the WebView feature is off.
  - `getMangaUrl` returns null on error and the action silently does nothing (E-20).

### T-38a-041 — Search actions and navigation rules
- Priority: P2
- Effort: S
- Depends: T-38a-012, area:37-ui-library, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Global search pushes the global search page; non-global search from a Home page pops and searches the library; from a Browse source page pops and searches there; otherwise no-op (E-21, E-22).
  - Genre search from a Browse source page for an HTTP source uses the source's genre search; otherwise non-global search.

### T-38a-042 — Copy-to-clipboard guards
- Priority: P2
- Effort: S
- Depends: T-38a-012, T-38a-016
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Copy title, author, artist, source name, and tag only when the text is non-blank.
  - Copy is a GDK clipboard write; the copy confirmation toast appears once per copy.

### T-38a-043 — Dot separator primitives
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/DotSeparatorText.kt
- Acceptance:
  - Separator text " • " (space, U+2022, space) and a no-space "•" variant exist as reusable labels; the spacing is covered by a snapshot test.

### T-38a-044 — Base manga list item
- Priority: P2
- Effort: S
- Depends: T-38a-039
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/BaseMangaListItem.kt
- Acceptance:
  - Row of fixed height 76 px with 16 px horizontal and 3 px vertical padding; cover slot (book ratio, full height), content slot (title, ellipsized, weight 1), actions slot.
  - Click on the row triggers the row action; click on the cover triggers the cover action defaulting to the row action.

### T-38a-045 — Shared constant enums
- Priority: P2
- Effort: S
- Depends: T-38a-025, T-38a-037
- Sources: app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt
- Acceptance:
  - Rust enums `DownloadAction` (NEXT_1_ITEM, NEXT_5_ITEMS, NEXT_10_ITEMS, NEXT_25_ITEMS, UNVIEWED_ITEMS), `EditCoverAction` (EDIT, DELETE) and `EntryScreenItem` exist with the same variant names.
  - `DownloadAction` maps to the next N unread items or all unread items (contract used by part 2).

### T-38a-046 — Keyboard and pointer shortcuts
- Priority: P2
- Effort: M
- Depends: T-38a-010, T-38a-011, T-38a-021
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Chapter list: Up/Down focus, Enter opens, Space toggles, Shift+click range, Ctrl+click toggle, Ctrl+A select all, Ctrl+I invert, Escape clears.
  - Header: F opens filter sheet; R refreshes; Ctrl+M migrates; W opens WebView when available.
  - Shortcut list is shown in an in-app help dialog (new, labelled proposal).

### T-38a-047 — Drop: Android haptic feedback
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaChapterListItem.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt
- Acceptance:
  - Decision recorded: haptics are dropped on Linux desktop; call sites removed or made no-ops.
  - User loses: vibration feedback on touch hardware. Replacement: the visual 1-second label reveal (T-38a-024).

### T-38a-048 — Drop: Android assist content URL
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - Decision recorded: AssistContent (assist URL) is dropped; no code path.
  - User loses: the Android assistant can read the source URL. Replacement: the Share and Copy URL actions (T-38a-040).

### T-38a-049 — Replace: Android UI and DI plumbing
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt
- Acceptance:
  - Decision recorded: Voyager navigator, Compose runtime, Injekt, Coil, Android Intents and ContentResolver are replaced per rust design section 4; no code path depends on them.
  - Each replaced API has a named Rust equivalent listed in the rust design table.

### T-38a-050 — Replace: pull-to-refresh gesture
- Priority: P3
- Effort: S
- Depends: T-38a-035
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Decision recorded: the Compose pull gesture is replaced by Ctrl+R, the toolbar item and a header spinner; the pull gesture is kept only if a touch device is present.
  - User loses: pull-down on touch screens. Replacement: Refresh action.

### T-38a-051 — Test suite for pure logic and fixtures
- Priority: P1
- Effort: M
- Depends: T-38a-003, T-38a-004, T-38a-005, T-38a-006, T-38a-007
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt, app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt
- Acceptance:
  - Every test listed in edge-cases section 4 exists and passes with `cargo test`.
  - Fixtures from edge-cases section 5 are committed under tests/fixtures/details/ and contain no real user data.
  - Java-generated expected strings for formatting are recorded once and checked in.

### T-38a-052 — Missing-marker and separator display
- Priority: P1
- Effort: S
- Depends: T-38a-006, T-38a-009
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/MissingItemCountListItem.kt
- Acceptance:
  - A divider-text-divider row with the plural text appears between chapters at each marker; it is not selectable or clickable.
  - Its accessible label reads the full plural text.

### T-38a-053 — Loading, empty and error states
- Priority: P1
- Effort: S
- Depends: T-38a-001, T-38a-034
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/presentation/entries/components/ItemHeader.kt
- Acceptance:
  - Loading: centered spinner. Zero chapters: header "0 chapters" and an empty list. Source error: toast; page stays usable with cached data.
  - Filter hides all chapters: header "0 chapters" and proposal hint "No chapters match the current filters" (labelled new).

### T-38a-054 — Gesture policy for mouse and touch
- Priority: P2
- Effort: S
- Depends: T-38a-010
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaChapterListItem.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Right-click and long-press are the same action on every combined-click control in this area.
  - A right-click never also fires the left-click handler (E-52).

### T-38a-055 — Mark previous as read
- Priority: P1
- Effort: S
- Depends: T-38a-024, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - Shown only with exactly one selected chapter (E-32); action calls the part 2 `mark previous` with the anchor chapter.
  - Respects descending order when computing "previous" (E-56).

### T-38a-056 — Local manga rules
- Priority: P1
- Effort: S
- Depends: T-38a-005, T-38a-025
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - Local manga: download indicator disabled, bulk Download hidden, all chapters shown as downloaded, Delete available in the bulk bar (E-27).
  - Downloaded filter keeps all local chapters.

### T-38a-057 — Library bottom action bar (shared component)
- Priority: P1
- Effort: M
- Depends: T-38a-025, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt
- Acceptance:
  - The library bar offers Set categories, Mark read, Mark unread, Download (dropdown), Delete with the same long-press label reveal as the entry bar.
  - Download opens the shared download dropdown instead of acting directly.
  - Visibility uses the 300 ms delayed expand and no delay on collapse.

### T-38a-058 — Download dropdown (shared)
- Priority: P1
- Effort: S
- Depends: T-38a-045, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt, app/src/main/java/eu/kanade/presentation/entries/components/EntryToolbar.kt
- Acceptance:
  - Items: Next 1, Next 5, Next 10, Next 25, Unread (labels to be verified against the shared `EntryDownloadDropdownMenu` in part 2; provisional labels in the mockup spec).
  - Shown in the toolbar for non-local manga and in the library bar.

### T-38a-059 — Cover and backdrop image pipeline
- Priority: P1
- Effort: M
- Depends: area:34-images-covers-cache, T-38a-012
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt
- Acceptance:
  - Cover loads with crossfade 150 ms; a cover change (cover_last_modified) reloads without restarting the app.
  - Loads happen off the GTK thread; the placeholder shows until the texture is ready.

### T-38a-060 — Drop: stale timer and confirm-by-long-press (Android quirk)
- Priority: P3
- Effort: S
- Depends: T-38a-024
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt
- Acceptance:
  - Decision recorded: the Android reset-job bug (E-12) is not ported; the Rust timer is cancelled on every long-press.
  - Tap always executes, as in Aniyomi (no confirmation step is added).
