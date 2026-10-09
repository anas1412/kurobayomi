# area:27-tracking-core part 2 — UI parity (track dialogs, track row menu, search page, login feedback)

Scope UI files: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt`, `.../manga/track/MangaTrackInfoDialog.kt` (both Voyager screens that render Compose dialogs and one full-screen search page), and the login activity pair `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt`, `TrackLoginActivity.kt` (a spinner page with no content). The Compose composables those screens call are part 1 (`app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt`, `.../track/anime/AnimeTrackInfoDialogHome.kt`, `.../track/anime/AnimeTrackerSearch.kt`, and manga counterparts). Their visible strings and states are listed here because the dialogs cannot be described without them.

Design target: Kurobayomi GTK4 (no libadwaita), Tokyo Night theme (user preference: match the current theme). Desktop: keyboard and mouse. There is no phone layout; the minimum usable window width is 360 px (responsive rule below).

Strings are the English values from `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (cited per row).

---

## 1. Container decision (Proposal)

Kotlin pushes Voyager screens and shows Compose `AlertDialogContent` / `BaseSelector` / `DatePicker` as their content. Rust options:

| Option | Used for | Reason |
|---|---|---|
| Modal `gtk::Window` (transient for the main window, `set_modal(true)`, `set_default_size(400, -1)`, no header bar except a close button) | Status, Progress, Score, Date picker, Date remover, Remove tracking | Matches the Kotlin dialog look (floating card over the entry). Escape closes. |
| Full page in the main content stack (`ui::push` / `ui::pop` in `src/ui.rs:34-47`) | Remote search (B§4) | The Kotlin search is a full screen with a toolbar, results list and a bottom bar. |
| Popover | not used | Popovers cannot hold a date picker with confirm and cancel buttons cleanly. |

Stacking: a child modal opened from another modal is `set_transient_for(parent_modal)`. Closing the child returns focus to the parent. `pop_until_home` (B§8.4, B§9) closes every modal above the Home window.

Home = the tracking sheet. Kotlin shows it as a bottom sheet/dialog (`AnimeTrackInfoDialogHomeScreen`). Rust: modal window of 420 px width, title "Tracking". Mockup uses the same name (`spec/entries.json:1387`, `"title": "Tracking"`).

---

## 2. Home: tracking sheet (B§3, part 1 composable `AnimeTrackInfoDialogHome.kt`)

### 2.1 Layout (one block per assembled `TrackItem`, in `TrackerManager` order)
Observed structure (`AnimeTrackInfoDialogHome.kt:81-127`, `TrackInfoItem` at 135-266):
1. Tracker logo tile, 48 × 48 px, rounded (part 1 `TrackLogoIcon`, `app/src/main/java/eu/kanade/presentation/track/components/TrackLogoIcon.kt:60`). Rust: image from the bundled logo set; fallback letter tile (the mockup already draws a letter tile, `src/entry.rs:509-512`).
2. Tracker name and remote title: the title is `track.title` (B§3.5 search prefill uses the same value).
3. "Tracked privately" icon (eye-off glyph) with content description `tracked_privately` = "Tracked privately" (`AnimeTrackInfoDialogHome.kt:162-170`), only when `track.private`.
4. Trailing "⋮" menu button (`TrackInfoItemMenu`, shown when a track exists, part 1 `AnimeTrackInfoDialogHome.kt:205`).
5. Status row: text = status label (or empty), tap → status dialog (B§5). Shown when a track exists.
6. Progress row: text = `"<progress> / <total>"` style is what the mockup shows (`spec/entries.json:1441`, subtitle "Shows '12 / 24'; opens number wheel; 'Episodes' for anime"). Kotlin shows episode/chapter count (part 1 `onEpisodesClick`). Tap → progress dialog (B§6).
7. Score row: shown when `supportsScoring && track.score != 0.0` (text = `displayScore`), placeholder "Score" otherwise when the score click is available (`AnimeTrackInfoDialogHome.kt:99-102`, `:235-241`). Tap → score dialog (B§7).
8. Start date row: text `dateFormat.format(startDate.toLocalDate())` when `supportsReadingDates && startDate != 0`; placeholder "Start date" (`track_started_reading_date`) when the date is 0 and the row is clickable (`:249-256`). Tap → date dialog with start (B§8).
9. Finish date row: same with `finishDate`, placeholder "Finish date" (`track_finished_reading_date`) (`:257-263`).

Rows 7-9 use placeholders only when the tracker supports the field.

### 2.2 Row gestures (`AnimeTrackInfoDialogHome.kt:177-181`)
- Click on the tracker row body: open in browser (B§3.6).
- Long press on the body: copy link (B§3.7).

Rust: single click = open in browser; secondary click (right) and long press = menu; Ctrl+C is not bound (not in Kotlin).

### 2.3 Row menu (`TrackInfoItemMenu`, manga file lines 340-395, used by anime too)
Items in this order, labels exact:
1. "Open in browser" (`action_open_in_browser`).
2. "Copy link" (`action_copy_link`, `i18n/.../strings.xml:113`).
3. Only when `supportsPrivateTracking`: "Track publicly" when `track.private` is true, otherwise "Track privately" (`action_toggle_private_off` / `action_toggle_private_on`, strings.xml: "Track publicly" / "Track privately").
4. "Remove" (`action_remove`).

Rust: `gtk::PopoverMenu` with `gio::Menu` items; the order and labels above are the parity target.

### 2.4 Bottom of sheet
- Each bound tracker row is followed by its own divider; untracked trackers show one row "Add tracking" (`add_tracking` = "Add tracking", `AnimeTrackInfoDialogHome.kt:284`) with a bordered button. Click → remote search (B§3.5 `onNewSearch`). For enhanced trackers, click performs the auto-match instead (B§3.9); the button label stays "Add tracking".

### 2.5 States
| State | Condition | What is shown |
|---|---|---|
| Loading | Initial frame before first subscription emission | Empty sheet (Kotlin shows nothing until `state.trackItems` is emitted). Rust: show a spinner row for ≤ 300 ms before first emit. |
| Empty | No logged-in trackers for the kind | Kotlin shows an empty list. Rust: one dim line "No tracker is logged in. Log in under Settings > Tracking." (Proposal; the sheet must not be blank). |
| Populated | ≥ 1 logged-in tracker | One block per tracker. |
| Refresh error | a tracker's refresh failed | Toast (B§3.3), no inline error. The row keeps its old values. |
| Private | `track.private` | "Tracked privately" icon and menu label "Track publicly". |

### 2.6 Refresh feedback
None while refreshing (Kotlin has no indicator). Proposal: a 2 px `gtk::ProgressBar` at the top of the sheet while `refresh_all` is running. Not in the mockup. Required for "daily use" usability (user can see when data is stale).

---

## 3. Status dialog (B§5; part 1 `TrackStatusSelector`, `TrackInfoDialogSelector.kt:48-94`)
- Title: "Status" (`status`, strings.xml:789).
- Body: vertical list of radio rows. Each row: radio, label text (`stringResource(label)` or empty). Row is clickable across the full width (`minimumInteractiveComponentSize`, 8 dp clip).
- Selected row = the radio. Initial selection = current status. Selection changes only the radio (no write).
- Buttons: "Cancel" (left) and "OK" (right) (`BaseSelector`, `TrackInfoDialogSelector.kt:203-231`). Rust: `gtk::Button` row at the bottom, Cancel then OK, OK is `suggested-action`.
- Scroll: list is scrollable; a divider appears at the top/bottom when more items exist (`canScrollBackward/Forward`, lines 88-89). Rust: `gtk::ScrolledWindow` with `max-content-height` 420.
- Keyboard: Up/Down move radio selection; Enter = OK; Escape = Cancel.
- Confirm closes the dialog immediately; remote write continues in background (B§14).
- Error after confirm: toast with `track_error` text (B§3.3 format) if the remote fails (part 1 `updateRemote` toasts the raw message, B§13).

## 4. Progress dialog (B§6; part 1 `TrackItemSelector`, `TrackInfoDialogSelector.kt:97-118`)
- Title: "Chapters" (manga) or "Episodes" (anime) (`MR.strings.chapters` = "Chapters"; `AYMR.strings.episodes` = "Episodes", i18n-aniyomi strings.xml:408).
- Body: Kotlin `WheelNumberPicker` centred on the current value, range `0..total` or `0..10000`.
- Rust (Proposal, no wheel widget in GTK): `gtk::SpinButton` with `adjustment(value = current, lower = 0, upper = range_max, step_increment = 1, page_increment = 10)`, plus a `gtk::Label` showing " / <total>" when total is known. Rule: when `total == 0`, show no total.
- Buttons: "Cancel", "OK" (same as status).
- Validation: none (B§6). Values below the current progress are allowed.
- Keyboard: Up/Down ±1, PageUp/PageDown ±10, Home = 0, End = max, Enter = OK, Escape = Cancel.
- The spin button clamps to the range; typing an out-of-range number is clamped on focus-out.

## 5. Score dialog (B§7; part 1 `TrackScoreSelector`, `TrackInfoDialogSelector.kt:121-141`)
- Title: "Score" (`score`).
- Body: Kotlin `WheelTextPicker(items = selections)`. Rust (Proposal): `gtk::DropDown` with the same strings in the same order (`StringList`), selected item = `display_score` of the current record when it matches an option, else the first item.
- Buttons: "Cancel", "OK".
- Selecting does not write until OK.
- Known Kotlin quirk: the initial wheel index is `selections.size / 2` when the current score is at index 0 (`TrackInfoDialogSelector.kt:135`). Rust starts at the real index (defect D-9 fixed, task T-27b-035). Parity note: this is a deliberate divergence.

## 6. Date dialog and remover (B§8, B§9; part 1 `TrackDateSelector`, `TrackInfoDialogSelector.kt:144-193`)

### 6.1 Date picker
- Title: "Start date" or "Finish date" (strings.xml:791-792).
- Body: date picker, no headline, no mode toggle (Kotlin `showModeToggle = false`).
- Buttons, in this order: "Remove" (left, only when a date is set; `canRemove`, B§8.1), spacer, "Cancel", "OK" (`TrackInfoDialogSelector.kt:168-188`).
- Initial day: the stored date's local calendar day, or today when unset (B§8.2).
- Future days: not selectable (B§8.3 rule 2). Start: days after the finish date are not selectable. Finish: days before the start date are not selectable.

Rust (Proposal): `gtk::Calendar`. GTK Calendar cannot disable days. Behavior: when the user selects a disallowed day, the model clamps the selection to the nearest allowed day (for a future date, today; for start after finish, the finish date; for finish before start, the start date) and shows a dim inline hint "Date out of range" for 2 s. Year-level bounds (B§8.3) are enforced the same way (the year arrows clamp the visible month to the allowed range). Keyboard: arrows move the day, PageUp/PageDown the month, Enter = OK.

Alternative considered: a text entry "YYYY-MM-DD" with validation. Rejected for the first version because the mockup and the user's daily flow use the picker; the text entry can be added as a P2 task.

### 6.2 Date remover
- Dialog title "Remove date?" (strings.xml:801), body text per start/finish (strings.xml:802-803), buttons "Cancel" and "Remove" (red, `errorContainer` style equivalent: `destructive-action` CSS class).
- On "Remove": closes the remover and the date picker (B§9).
- The mockup's `remove_date` dialog (`spec/entries.json` id `remove_date`) already shows these buttons (Cancel, Remove); keep.

## 7. Remove tracking dialog (B§10; part 1 `AlertDialogContent`)
- Title: "Remove <tracker> tracking?" (`track_delete_title`, strings.xml:804).
- Text: "This will remove the tracking locally." (strings.xml:805).
- Checkbox (only when the tracker is deletable): "Also remove from <tracker>" (strings.xml:806), unchecked by default. Rust: `gtk::CheckButton`.
- Buttons: "Cancel" and the confirm button. Kotlin label is "OK" (`action_ok`, strings.xml:136). Parity target: "OK". Proposal (defect D-14): use "Remove" (destructive style) because the action is destructive, matching the date remover. The mockup `remove_tracking` dialog uses "OK" (`spec/entries.json:1657-1680`). Decision: "Remove" in Rust; logged as a parity divergence.
- Destructive style: confirm button uses `destructive-action`.

## 8. Remote search page (B§4; part 1 `AnimeTrackerSearch`, `AnimeTrackerSearch.kt:89-236`)

### 8.1 Layout
- Top bar: back arrow (pops the page), search entry prefilled with the title (`initialQuery`), placeholder "Search…" (`action_search_hint`, strings.xml). IME action Search (Enter) runs the search (`AnimeTrackerSearch.kt:128-129`, `onDispatchQuery`, B§4.2). A clear (X) icon appears while the text is non-empty and clears the text and refocuses the field (`AnimeTrackerSearch.kt:150-160`). Rust: `gtk::SearchEntry` with `activate` (Enter) wired to the search and `search-changed` NOT wired, because each search is a remote call; the SearchEntry's built-in clear icon matches the X.
- Results list: one card per `SearchHit` (cover, title, authors, type/status/score/start date details). Part 1 `SearchResultItem` (`AnimeTrackerSearch.kt:238-358`); labels: "Type" (`track_type`), "Started" (`label_started`), "Status" (`track_status`), "Score" (`score`).
- Result long press (`combinedClickable(onLongClick = …)`, `AnimeTrackerSearch.kt:263-264`): a dropdown with "Copy to clipboard" (`action_copy_to_clipboard`) and "Open in browser" (`action_open_in_browser`) (`AnimeTrackerSearch.kt:359-385`). A plain click selects the card.
- Bottom bar: `AnimatedVisibility(visible = selected != null)` (`AnimeTrackerSearch.kt` bottom bar). Hidden, not disabled, until a result is selected. Contents when shown:
  - "Track" button (`action_track`, "Track") → confirm with `private = false`.
  - When `supportsPrivateTracking`: an eye-off icon button with tooltip "Track privately" (`action_toggle_private_on`) → confirm with `private = true`.
- Rust: the mockup's bottom bar shows a Revealer that opens on selection (`src/entry.rs:563-575`). Keep the Revealer; wire the two buttons to `SearchModel::confirm(private)`.

### 8.2 States
| State | Condition | Shown |
|---|---|---|
| Loading | `queryResult == null` (B§4.2 step 2) | `LoadingScreen` (spinner centred). Rust: `gtk::Spinner` centred in the results area. |
| Results | success and non-empty | list |
| Empty | success and empty list | "No results found" (`no_results_found`, strings.xml) centred |
| Error | failure | the raw error message (`queryResult.exceptionOrNull()?.message`), fallback "Unknown error" (`unknown_error`, strings.xml) centred (`AnimeTrackerSearch.kt:228-232`). Rust: same text plus a "Retry" button (Proposal). |
| Idle (no query) | blank query | Kotlin runs the search with an empty string (B§4.2). Rust (Proposal): show the hint "Type a title and press Enter", do not call the remote. |

### 8.3 Selection
- Clicking a card selects it (`onSelectedChange`); the selected card is highlighted (`selected = it == selected`).
- Auto-select: when the bound tracker's `tracking_url` matches a result, it is pre-selected (B§4.2 step 4). Rust: same.
- Confirm with no selection: not reachable in the UI because the bar is hidden until a selection exists. The B§4.3 early return is kept in the model for safety.

### 8.4 Keyboard and mouse
| Action | Keyboard | Mouse |
|---|---|---|
| Run search | Enter in entry | Click Search button |
| Move selection | Up/Down in list | Click card |
| Track | Tab to the Track button and Enter (Proposal: Ctrl+Enter shortcut) | Click Track |
| Track privately | Ctrl+Shift+Enter (Proposal) | Click eye-off button |
| Context menu | Menu key / Shift+F10 on the focused card | Secondary click or long press |
| Back | Escape or Alt+Left | Back arrow |

---

## 9. Login feedback (B§12; replaces `TrackLoginActivity` spinner `LoadingScreen`)

Kotlin shows a full-screen spinner until `returnToSettings()` finishes the activity (`BaseOAuthLoginActivity.kt:34-36`). The Linux app has no separate window.

| Outcome (kuro-track `CallbackOutcome`) | Visible result (Proposal) |
|---|---|
| Logged in (token or code exchange succeeded) | Toast "<tracker> logged in" (new key `TrackerLoggedIn`, Proposal) and the Settings > Tracking row shows the check mark (area:47) |
| Login failed (exchange or identity call failed) | Toast "<tracker> login failed" (new key `TrackerLoginFailed`, Proposal), the row keeps its previous state (Kotlin logs out here, defect D-3 changed) |
| User denied or cancelled (no code, no token) | Toast "<tracker> login cancelled" (Proposal). Kotlin logs out here (defect D-2 changed; token kept) |
| Unknown host or malformed URI | Toast "Unrecognized login link" (Proposal). Kotlin hangs on the spinner (defect D-1 changed) |
| Tracker not configured (missing client secret) | Toast "<tracker> is not configured" (new key `TrackerNotConfigured`, Proposal) |

No window is created for the callback. The main window is presented (`window.present()`).

## 10. Toasts and snackbars
- Kotlin uses Android `Toast` for all messages in scope (`context.toast(...)`, `Application.toast(...)`, `withUIContext`).
- Rust: `src/track/toast.rs` overlay at the bottom centre of the active window, auto-hide 3500 ms, max 3 stacked, click to dismiss. Not a `gtk::Notification` (desktop notifications are reserved for background jobs, area:35).
- Texts in scope: "<name> error: <message>" (`track_error`, strings.xml:800), "No match found" (`error_no_match`, strings.xml:799), "Copied to clipboard:\n<url truncated to 50 chars>" (`copied_to_clipboard`, strings.xml:728; Kotlin shows it only on API <= 32 (`S_V2`, `ContextExtensions.kt:48`); Rust always shows it, because GTK has no system confirmation).

## 11. Keyboard and mouse summary for every dialog
- Escape = Cancel in every modal.
- Enter = OK/Confirm (the default button is OK; `gtk::Widget::set_default_widget`).
- Tab moves focus in order: list/control, Cancel, OK.
- Mouse: a click outside a modal does not dismiss it (Proposal: GTK modal keeps focus; closing is by Cancel or Escape). Kotlin's Android dialog dismiss-on-outside-tap is not reproduced.

## 12. Responsive behaviour
- Minimum window width supported: 360 px. Every dialog content fits at 360 px: dialogs use `set_default_size(400, -1)` and `set_size_request(320, -1)`.
- No horizontal scroll: the search result card wraps the title (2 lines max, ellipsis) and the detail chips wrap.
- Home sheet max height: 80% of the window height, then scrolls vertically (`gtk::ScrolledWindow`).
- Calendar: 7 columns, fixed cell size, so it fits 320 px.

## 13. Visual rules
- Colours: Tokyo Night tokens from the app stylesheet (area:49 theme). No custom gradients.
- Corners: 8 px radius on rows (Kotlin uses `RoundedCornerShape(8.dp)` for radio rows, `TrackInfoDialogSelector.kt:67`).
- Destructive buttons: the Tokyo Night red token on a dark red tint, using the app stylesheet tokens owned by area:49 (no hex value is fixed in this area).
- Typography: row title 15 px semibold, subtitle 12 px dim; dialog titles 16 px bold.

## 14. Animations
- Kotlin uses Compose defaults (none specified in scope). Rust: none, except the toast fade (150 ms) and the Revealer on the search page (`gtk::Revealer` default 250 ms slide).

## 15. Mockup gap list (current `src/entry.rs` and `spec/entries.json`)

| Mockup today | Required by this area | Task |
|---|---|---|
| `tracking()` builds three fixed rows "AniList" (bound), "MyAnimeList", "Kitsu" (`src/entry.rs:498-537`) | rows from `HomeModel` for logged-in trackers only | T-27b-003, T-27b-004, T-27b-006 |
| Status, progress, score, dates show static text "Watching", "12 / 24", "8.0", "Start date" (`src/entry.rs:519-523`) | live values from records; placeholders only when unset | T-27b-006 |
| `tracker_search()` four fixed results and a fixed "Type/Started/Status/Score" detail (`src/entry.rs:539-566`) | live `SearchHit`s, loading/empty/error states | T-27b-012 |
| "Track privately" and "Track publicly" as two separate menu entries (`spec/entries.json:1392-1398`) | one toggle with state-dependent label | T-27b-017 |
| Dialog "remove_tracking" button "OK" | "Remove" (destructive) per D-14 | T-27b-014 |
| No loading indicator on Home while refreshing | 2 px progress bar | T-27b-005 |
| No toast surface | `src/track/toast.rs` | T-27b-039 |
| Date picker is "Date picker" text placeholder (`spec/entries.json:1607-1626`) | `gtk::Calendar` with clamp | T-27b-010 |
| Login has no feedback | section 9 outcomes | T-27b-023, T-27b-024 |
| No "Retry" in search error | Retry button | T-27b-012 |
| Mockup uses `title_n(1)` and `Author name` placeholders | real data only | T-27b-012 |
