# 38 part 2: UI parity (what the scope's models drive on screen)

Scope files that render UI: `MangaTrackInfoDialog.kt` (Compose screens for the tracking sheet and sub-screens) and the view-model side of `MangaScreenModel.kt` (state that MangaScreen.kt, owned by part 1, renders). This document lists every state, dialog, snackbar, gesture and control that the scope's logic drives. Layout of the details page itself (cover, info box, action row, description) is part 1.

Labels come from the upstream string keys (exact text in p2-behavior.md section 9.3). Where upstream has no text for a control, this doc says so and gives a Proposal.

Legend: **Upstream** = what Aniyomi shows. **Desktop** = Linux GTK4 equivalent. **Proposal** = change proposed for Kurobayomi.

---

## 1. Screen states (chapter list and header driven by MangaScreenModel)

| State | Trigger | Upstream | Desktop | Proposal |
|---|---|---|---|---|
| Loading | `State.Loading` until `awaitManga` + `awaitChapters` complete (MSM:215-239) | Compose placeholder (part 1) | Spinner in the page body, header skeleton | Same, plus a 300 ms delay before showing the spinner to avoid flicker |
| Loading stuck | `getMangaById` throws (E-LOAD-1) | none (stays Loading) | none | Show error state with Retry (re-run load) |
| Ready, not refreshing | `isRefreshingData == false` | list rendered | list | same |
| Ready, refreshing | `isRefreshingData == true` (init fetch or manual) | refresh indicator | GTK header spinner (`gtk::Spinner`) at the top of the list | same; the list stays interactive |
| Empty chapters after fetch | `chapters.isEmpty()` (source returned none for a non-local source the fetch fails with `NoChaptersException`, shown as snackbar) | empty list (part 1 text) | empty-state label | same; keep the snackbar `no_chapters_error` |
| Fetch error | fetch throws | snackbar with `formattedMessage`, list keeps old data | banner (section 6) | same |
| Filtered to empty | view has 0 rows but raw has rows | part 1 empty text | label "No chapters match the current filters" (proposal text; upstream text is part 1) | Proposal: add "Clear filters" button in that empty state |
| Local manga | `manga.isLocal()` | all rows `DOWNLOADED`, download action hidden for rows | same | same |

### 1.1 Chapter row contents (from `ChapterList.Item` and `MissingCount`)

Driven fields per row:
- Name (`chapter.name`), scanlator (`chapter.scanlator`, may be absent), chapter number (for number display mode: `displayMode == CHAPTER_DISPLAY_NUMBER`).
- Read state (`chapter.read`), last page read (`lastPageRead`, shown by part 1 as progress).
- Bookmark (`chapter.bookmark`).
- Download indicator: state NOT_DOWNLOADED / QUEUE / DOWNLOADING (with progress `downloadProgress`) / DOWNLOADED / ERROR. The indicator is a button; its action is `ChapterDownloadAction` chosen from the state (behavior 6.9.1 mapping: NOT_DOWNLOADED or ERROR -> START_NOW; QUEUE or DOWNLOADING -> CANCEL; DOWNLOADED -> DELETE).
  - Desktop: 16 px icon-only button with tooltip. Tooltips: Proposal, since upstream's text for this icon is in `ChapterDownloadIndicator.kt` (part 1 owns the labels).
- Selected state (`item.selected`) -> checkbox-style highlight.
- Separator rows `MissingCount(count)`: a non-interactive row between two chapters (or at top/bottom) showing that `count` chapters are missing. Text: part 1 owns the label. Proposal text: "{count} chapters missing" (en), since upstream's count label is part 1's string and not in scope.

### 1.2 Header/toolbar state driven by scope

- Filter active indicator: `State.Success.filterActive` (true when any filter is set or a scanlator exclusion matches an available scanlator; MSM:1195-1196). Desktop: filter button gets the `suggested-action` style or a dot badge. Proposal: a 6 px dot on the filter button.
- Tracking indicator: `trackingAvailable` (count > 0). Upstream part 1.
- "Next unread" action: `getNextUnreadChapter()` (null -> disabled/hidden action). Proposal: disabled with tooltip "No unread chapters" (upstream text unverified in scope; treat as Proposal).

---

## 2. Chapter selection UI (driven by toggleSelection / toggleAllSelection / invertSelection)

### 2.1 Entering selection

- Upstream: long press on a row (`fromLongPress = true`) enters selection and selects the row; subsequent long press on another row selects the range between the anchors (MSM:989-1014). Tap on a row in selection mode toggles it (`userSelected = true`, `fromLongPress = false`).
- Desktop mapping (Proposal):
  - Touch: long press (GtkGestureLongPress, 500 ms default in GTK) = same as upstream.
  - Mouse: right-click (button 3) enters selection (no long press on mouse); left-click on a row in selection mode toggles.
  - Mouse range: Shift+click = LongPress range semantics (extend from anchor). Ctrl+click = toggle one row (Tap).
  - Keyboard: Space toggles the focused row (Tap). Shift+Space = range (LongPress semantics). Enter opens the chapter (reader, part 36/reader).
- Entering selection with no rows: not possible (rows must exist).

### 2.2 Selection toolbar actions (model-supported; labels from part 1)

The model supports these exact operations; the toolbar in part 1 maps to them:
1. Mark read / Mark unread: `markChaptersRead(selected, read = true|false)`. Upstream shows both; the direction depends on the selection (part 1 decides).
2. Mark previous as read: `markPreviousChapterRead(pointer)` where pointer = the single selected chapter. Upstream: only meaningful with one selected chapter (part 1 enables only then). Desktop: keep as a toolbar button with the selection-count guard.
3. Bookmark / Remove bookmark: `bookmarkChapters(selected, bookmarked = !allBookmarked)`. Proposal: the toggle direction is "bookmark if any selected is not bookmarked". Upstream uses per-item `filterNot { it.bookmark == bookmarked }` so already-bookmarked rows are skipped; the direction is chosen by part 1 (behavior-compatible).
4. Download (START for not-downloaded selection): `runChapterDownloadActions(selected, START)`.
5. Delete downloads: `deleteChapters(selected)` after `DeleteChapters` dialog (section 3.5).
6. Select all: `toggleAllSelection(true)`. Acts on the RAW list (includes hidden chapters, behavior 6.8). Proposal: keep (see behavior decision D-2 in section 9).
7. Invert: `invertSelection()` (RAW list, same caveat).
8. Clear selection: `toggleAllSelection(false)`. Also happens automatically after mark-read, bookmark and download (MSM:756, 825, 838).

Count shown in the toolbar title: number of selected rows. Count source: the `selected` flags (raw list, behavior caveat). Proposal: count `selection.ids.len()`, which is the same set.

Keyboard: Escape clears the selection (Proposal). Ctrl+A select all (RAW), Ctrl+I invert (Proposal, chosen to avoid clashing with GTK defaults).

### 2.3 Swipe actions on rows

- Two edges: start edge action `chapterSwipeStartAction` (key `pref_chapter_swipe_start_action`, default ToggleBookmark) and end edge action `chapterSwipeEndAction` (key `pref_chapter_swipe_end_action`, default ToggleRead) (MSM:144-145; see behavior 2.1 for the key inversion).
- Actions: ToggleRead, ToggleBookmark, Download (per the state mapping in 1.1), Disabled (no swipe).
- Upstream: touch swipe. Desktop: `GtkGestureSwipe` for touchscreens only. Mouse/keyboard equivalent (Proposal): a hover-revealed two-button strip at the row end (read toggle, bookmark toggle, download), reachable by Tab.
- Swipe on a row in selection mode: Proposal, disabled (upstream: part 1 decides; keep the same).

---

## 3. Dialogs (MangaScreenModel.Dialog)

Each dialog is opened by a model action and closed by `dismissDialog()` (sets `dialog = null`, MSM:1108-1110). Dismiss via outside click, Escape, or Cancel.

### 3.1 ChangeCategory

- Opened by: `showChangeCategoryDialog()` (MSM:382-396) from the favorite flow (branch C) or the category action.
- Content: list of user categories (not system category), each with a two-state checkbox (`CheckboxState.State.Checked` or `None`), initial values from `getMangaCategoryIds` (MSM:450-453).
- Confirm: `moveMangaToCategoriesAndAddToLibrary(manga, selectedIds)` for the add flow (MSM:455-462). Only `Checked` items are written; the upstream `CheckboxState.State` has no indeterminate value, so the port has none either.
- Dismiss after add branch: favorite is not written; `onDismissRequest` then opens the track sheet if `autoOpenTrack && isFromChangeCategory` (MangaScreen.kt:176-184).
- Desktop: modal window, list with `gtk::CheckButton`, Confirm/Cancel buttons; Enter confirms.

### 3.2 DuplicateManga

- Opened by: `toggleFavorite` when `getDuplicateLibraryManga` finds a library manga with the same lowercased title and a different id (MSM:334-345).
- Content: part 1 (`DuplicateMangaDialog`, presentation/entries/manga/DuplicateMangaDialog.kt). It receives `manga` and `duplicate` from the dialog state.
- Upstream routes: "Migrate" (-> Migrate dialog, below) or "Add anyway" (part 1 decides; the scope only sets the state).
- Proposal: keep the duplicate check on by default; no setting in this part.

### 3.3 Migrate

- Opened by: `showMigrateDialog(duplicate)` (MSM:1128-1131). Also reachable from the browse/migration flow (other area).
- Content (part 1 `MangaScreen.kt:223`): `Migrate(newManga = currentManga, oldManga = duplicate)`.
- Desktop: confirmation dialog; the actual migration runs in the migration area (other part).

### 3.4 SetMangaFetchInterval

- Opened by: `showSetMangaFetchIntervalDialog()` (MSM:398-403). Content: part 1 dialog. Scope contract: `setFetchInterval(manga, interval)` with `interval` in days (negative stored); `0` resets to automatic (behavior 4.5).
- Desktop: `gtk::SpinButton` (integer, range from part 1) + Reset-to-automatic button (Proposal, making the `0` case explicit; upstream's `0` behavior is silent).

### 3.5 DeleteChapters

- Opened by: `showDeleteChapterDialog(chapters)` (MSM:1112-1114).
- Confirm: `deleteChapters(chapters)` (MSM:847-861) and clear selection (part 1 calls `toggleAllSelection(false)`? Not verified in scope. Proposal: clear selection after confirm.)
- Message text: part 1 owns the text. Scope facts: list is the snapshot passed when the dialog opened.

### 3.6 SettingsSheet (chapter settings bottom sheet)

- Opened by: `showSettingsDialog()` (MSM:1116-1118).
- Driven values: unread/downloaded/bookmarked filters (TriState), sort (source/number/upload date/alphabet, with direction), display mode (name/number). The model setters are behavior 6.5 and 6.6. Whether the sheet writes on each change (as the setters imply) or behind an Apply control is wired in part 1's ChapterSettingsDialog, which was not read in this part.
- "Set as default" action: `setCurrentSettingsAsDefault(applyToExisting)` (MSM:951-962). Sub-option "apply to existing" (upstream: part 1 control). Snackbar `chapter_settings_updated` after.
- "Reset to default": `resetToDefaultSettings()` (MSM:964-969).
- Desktop: `gtk::Popover` or bottom-attached sheet (proposal: right-side panel, 360 px, so that the list stays visible).
- Keyboard: Escape closes.

### 3.7 TrackSheet (tracking home)

- Opened by: `showTrackDialog()` (MSM:1120-1122) and automatically after an add when `autoOpenTrack` (MSM:375-377, MangaScreen.kt:176-184).
- Content: one row per logged-in manga tracker that accepts the source (behavior 8.1 `mapToTrackItem`). Row elements: tracker name, status label (if linked), chapter progress (`lastChapterRead` / `totalChapters`), score (`displayScore`), start/end dates, actions. Exact row layout: `MangaTrackInfoDialogHome` (presentation/track/manga/, part 1 or area:27 UI).
- Actions per row and their availability: section 4.
- Desktop: modal window 560 x 640 px (proposal); rows in a `gtk::ListBox`.

### 3.8 FullCover

- Opened by: `showCoverDialog()` (MSM:1124-1126). Content: full-size cover (part 1). Escape or click closes.

---

## 4. Tracking sub-screens (MangaTrackInfoDialog.kt)

Upstream uses a navigator stack (push/pop). Desktop: each sub-screen is a modal child window (`gtk::Window` transient for the sheet). Pop = close the child. Proposal for `popUntil` (remover -> home): close all children up to the home window.

### 4.1 Home row actions and availability

| Action | Upstream trigger | Available when | Result | Desktop |
|---|---|---|---|---|
| Status | row click (MTID:113-120) | track != null | status selector | button "Status" |
| Chapter progress | click (MTID:121-128) | track != null | chapter selector | button |
| Score | click (MTID:129-136) | track != null | score selector | button |
| Start date | edit (MTID:137-145) | track != null | date selector (start) | button |
| End date | edit (MTID:146-154) | track != null | date selector (finish) | button |
| New search / re-search | (MTID:155-168) | any | enhanced: auto-match; otherwise search screen | button "Search" |
| Open in browser | (MTID:169, 187-192) | track != null and remoteUrl not blank | opener | icon button |
| Copy link | (MTID:179, 194-199) | same | clipboard | icon button |
| Remove | (MTID:170-178) | track != null | remover (local + optional remote) | button |
| Private toggle | (MTID:180, 262-266) | track != null AND `supportsPrivateTracking` (Proposal; upstream shows it per tracker, part 1) | toggle | switch |

Upstream crashes if "private toggle" runs with track == null (E-TRK-1). Proposal: not shown when track is null.

### 4.2 Status selector (MTID:287-340)

- Control: single-choice list of the tracker's status options (labels from `getStatusForManga`).
- Initial selection: current status. Confirm writes and closes; Cancel closes.
- Upstream uses `TrackStatusSelector` (presentation). Desktop: `gtk::ListBox` with single selection; Enter confirms.

### 4.3 Chapter selector (MTID:342-403)

- Control: `TrackItemSelector(isManga = true)` (presentation): a scroll list of integers 0..max. Max = `totalChapters` when > 0, else 10000.
- Desktop (Proposal): `gtk::SpinButton` 0..=max (upstream's list of 10001 entries is a performance cost), with +/- buttons. Confirm writes `lastChapterRead` as an integer.
- Note: fractional chapter progress is not offered by the picker (`selection: Int`). The home list may show a fractional value from the tracker.

### 4.4 Score selector (MTID:405-457)

- Control: single-choice list from `getScoreList()`; initial = `displayScore(track)`.
- Desktop: `gtk::ListBox` single selection. Confirm writes the string.

### 4.5 Date selector (MTID:459-589)

- Control: date picker, title "Start date" / "Finish date" (MR keys), initial = track date or today.
- Constraints (behavior 8.5): no future dates; start <= finish; finish >= start. Disallowed dates are shown as disabled (Compose `SelectableDates`).
- Remove button: visible only when a date is set (`canRemove`), labelled "Remove" (MR `action_remove`) with error-container color.
- Desktop: `gtk::Calendar` has no disabled days. Proposal: a `gtk::Calendar` plus an inline error label ("Date must be on or before the finish date"/"after the start date"/"in the future") when the chosen day is invalid, and the Save button disabled until the day is valid. Error texts are Proposal (upstream's disabled-day UI has no text).
- Remove flow: opens the remover dialog (below).

### 4.6 Date remover dialog (MTID:591-677)

- Title "Remove date?", text "This will remove your previously selected start date from {service}" / "...finish date from {service}", buttons Cancel and Remove (error-container), Remove calls `setRemoteStartDate(0)` or `setRemoteFinishDate(0)` then closes to the home.

### 4.7 Tracker search (MTID:679-770)

- Layout (upstream `MangaTrackerSearch`): search text field, submit on IME action or search button (`onDispatchQuery`), result list (`queryResult`), selected item, confirm action, private checkbox when supported, dismiss.
- States: Loading (`queryResult == null`), results list, failure (`Result.failure`), no results (empty success list). Upstream text for failure/no results: part of the presentation layer. Proposal: failure shows the formatted error with a Retry button (re-run the same query).
- Auto-search on open when the initial query is not blank (MTID:727-732).
- Preselection: the result whose `tracking_url` equals the current link is selected on load.
- Confirm: writes `private` onto the selected result, registers, closes (behavior 8.7).
- Desktop: 560 px wide window; text entry with `gtk::SearchEntry`; results `gtk::ListBox`; Enter in the entry runs the search; Enter on a selected row confirms.

### 4.8 Tracker remove dialog (MTID:772-874)

- Title "Remove {service} tracking?", text "This will remove the tracking locally.", optional checkbox "Also remove from {service}" (only if the tracker is deletable), buttons Cancel and OK (error-container).
- Upstream OK: local unlink always; remote delete only if checkbox checked; remote failure is silent (behavior 8.8). Proposal: show a banner on remote failure (behavior decision D-5).

---

## 5. Snackbars and toasts (complete list from the scope)

| # | Trigger | Text key | Action | Duration | Dismissible | Desktop | Source |
|---|---|---|---|---|---|---|---|
| S1 | fetch details error | `formattedMessage` | none | Short (default) | no (upstream passes no withDismissAction) | banner, no action | MSM:282-289 |
| S2 | fetch chapters error | `no_chapters_error` or `formattedMessage` | none | Short | no | banner | MSM:586-596 |
| S3 | remove from library with downloads | `delete_downloads_for_manga` | `action_delete` | Indefinite (action present) | yes | banner with button | MSM:296-307 |
| S4 | after enqueue for a non-library manga (first time this session) | `snack_add_to_manga_library` (AYMR) | `action_add` | Indefinite | yes | banner with button | MSM:684-691 |
| S5 | mark read, ASK mode, tracker behind | `confirm_tracker_update(n)` | `action_ok` | Short (explicit) | yes | banner with button | MSM:785-790 |
| S6 | settings saved as default | `chapter_settings_updated` | none | Short (default) | yes | banner | MSM:958-960 |
| T1 | tracker refresh failure | `track_error(name, msg)` | none | toast (Android Toast short) | n/a | toast | MSM:806-813, MTID:250-257 |
| T2 | mark read, ALWAYS mode, trackers updated | `trackers_updated_summary_manga(n)` (AYMR) | none | toast | n/a | toast | MSM:777-781 |
| T3 | enhanced auto-match failure | `error_no_match` | none | toast | n/a | toast | MTID:235 |

Banner rules (Proposal, replacing Compose SnackbarHostState):
- One visible banner at a time. A new banner replaces the current one (upstream queues; Proposal: queue max 3, drop the oldest).
- Short = 4000 ms, Indefinite = until dismissed or action taken.
- Position: bottom center of the window, 16 px margin, max width 560 px. Text wraps to 2 lines max.
- Action button is the only focusable element that is not the dismiss button; Tab order: action, dismiss.
- Toasts (T1-T3) use the same banner component with Short duration and no action (upstream Android Toast is non-focusable; Proposal: non-focusable too).

---

## 6. Gestures and keyboard map (desktop)

| Gesture/key | Where | Effect |
|---|---|---|
| Click row | chapter list | open chapter (reader area) when not in selection mode |
| Click row in selection | chapter list | Tap toggle |
| Long press (touch) | chapter row | enter selection + select (LongPress); next long press = range |
| Right click | chapter row | Proposal: same as long press (enter/select) |
| Shift+click | chapter row in selection | range from anchor (LongPress semantics) |
| Ctrl+click | chapter row | Tap toggle |
| Space | focused row | Tap toggle |
| Shift+Space | focused row | LongPress range |
| Ctrl+A | page | select all (RAW list, behavior 6.8) |
| Ctrl+I | page | invert selection (RAW list) |
| Escape | page in selection | clear selection |
| Escape | dialog | close dialog |
| Enter | dialog | default button |
| Horizontal swipe (touch) | row | swipe actions (2.3) |
| Hover reveal (mouse) | row | Proposal: action strip (read, bookmark, download) |

---

## 7. Responsive behavior (desktop)

- Minimum window width supported: 360 px (phone-like narrow layout). Below 720 px the chapter list uses a single column; above it, the details header and list sit side by side (part 1 decides the header; this part only requires that the list uses the full remaining width).
- Dialogs: fixed max widths: ChangeCategory 480 px, Duplicate/Migrate 480 px, SetFetchInterval 360 px, DeleteChapters 480 px, SettingsSheet 360 px panel, TrackSheet 560 px, tracker search 560 px, status/score selectors 400 px, chapter selector 360 px, date selector 380 px.
- No horizontal page scroll; tables and long names wrap or ellipsize at 1 line in the chapter row (name ellipsized, scanlator on second line).
- Font scaling: all sizes in relative units via the GTK theme (no fixed px fonts), so the user's GTK theme applies.
- Dark/light: follows the GTK theme (vanilla GTK, no libadwaita). Kurobayomi ships Tokyonight CSS per the cross-platform addendum (rule 6); this part adds no colors of its own beyond the error-container color for destructive buttons, which maps to `@error_color` in the bundled CSS.

---

## 8. Mockup gap analysis (`/home/blackbox/kurobayomi/src`, spec `/home/blackbox/kurobayomi/spec/entries.json`)

| Upstream behavior (this part) | Mockup today | Gap |
|---|---|---|
| Chapter rows from a model | `src/entry.rs:170, 287, 363`: sample names "Chapter N", literal subtitle "Scanlator" | no model; sample data |
| Missing-count separator rows | none | missing entirely |
| Filter sheet bound to flags | `src/entry.rs:223-226` opens `sheet_of(id, sh)` from spec | static options; no TriState, no persistence |
| Sort direction + key | none | missing |
| Display mode name/number | none | missing |
| Selection (tap, long press range, select all, invert) | `src/entry.rs:193-203`: static action bar with fixed actions; `src/screens.rs:215-230` selection bar (static) | no selection set, no anchors, no range |
| Mark read/unread writes | static buttons (`screens.rs:224-226`) | no write, no tracker prompt |
| Mark previous as read | `src/entry.rs:195` label only | no logic |
| Download actions NEXT_N / UNVIEWED | `src/entry.rs` labels not present; `screens.rs` download sheet static | no logic |
| Download state per row and progress | none | missing |
| Swipe actions | none | missing |
| Add/remove favorite, category routing, duplicate check | `src/entry.rs:122` "Library" button (static) | no routing, no dialogs |
| Category dialog with two-state checkboxes | static dialog from spec | no checkbox state logic |
| Fetch interval dialog | `src/entry.rs:121` opens spec dialog | no value write |
| Scanlator filter | `spec/entries.json` has "Scanlator", "Scanlators" labels | no excluded set, no availability |
| Chapter settings as default / apply to all | static sheet text | no write |
| Tracking home list | `src/entry.rs:496-537`: static rows, "Add tracking" button | no mapping, no per-row availability rules |
| Tracker status/chapter/score selectors | none as separate screens (only spec dialog IDs) | missing |
| Date selectors with constraints | none | missing |
| Date remover | `src/entry.rs:514` `dialog_of(id, "remove_tracking")` | wrong dialog ("remove tracking" not "remove date") |
| Tracker search | `src/entry.rs:539-570` static list + "Track" + "Track privately" | no query, no auto-search, no preselection, no state |
| Tracker remove with remote checkbox | spec dialog `remove_tracking` | no checkbox logic, no deletable check |
| Snackbars S1-S6, toasts T1-T3 | none | missing (the banner component does not exist) |
| Track toast texts | none | missing |
| Loading/refresh/error states | none | missing |
| Local manga rules | none | missing |

Recommendation: replace the manga branch of `src/entry.rs` with the model-bound page (p2-rust-design.md section 9). Keep spec-driven sheets only for static layout, as the spec's own notes say (`spec/SCHEMA.md`).

---

## 9. Behavior decisions for the user

These are choices where the upstream behavior is surprising or a bug. The port should not silently change them. The user decides; defaults below are what this plan assumes until answered.

| ID | Decision | Upstream | Plan default |
|---|---|---|---|
| D-1 | Non-library manga opened again: reset its chapter filters/sort to the saved defaults | yes (MSM:220-222, every open) | keep (matches upstream) but document; alternative: apply defaults only once when first opened |
| D-2 | Select all and invert act on hidden (filtered-out) chapters too | yes (MSM:1035-1057 raw list) | keep for parity; alternative: act on visible only (safer for bulk delete/download) |
| D-3 | "Next N" downloads oldest unread first | yes (double-reverse, behavior 6.9.6) | keep |
| D-4 | "Continue reading" (next unread) returns the oldest unread in descending mode | yes | keep |
| D-5 | Remote tracker delete failure is silent | yes (MTID:860-868) | Proposal: show banner (fix) |
| D-6 | Enhanced tracker auto-match shows "No match found" for any error, including network errors | yes (MTID:232-236) | Proposal: show the real error text in addition |
| D-7 | Chapter settings sort direction key is shared with the anime default | yes (LibraryPreferences.kt:215-223) | Proposal: separate key for anime (see E-DEF-1); this changes backup compatibility for anime settings |
| D-8 | Marking already-read chapters read again can trigger the tracker prompt | yes (MSM:763-773) | Proposal: prompt only when at least one chapter actually changed |
| D-9 | Duplicate detection ignores source (same title in another source counts) | yes (mangas.sq:117-122) | keep |
| D-10 | Filter changes can be lost when made quickly (stale flags) | yes (E-FLAG-1) | Proposal: fix (atomic masked write) with no visible change otherwise |
