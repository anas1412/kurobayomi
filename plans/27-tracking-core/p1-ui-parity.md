# Area 27-tracking-core, part 1: UI parity

Scope: the UI files in part 1 (`presentation/track/*`, `presentation/more/settings/screen/SettingsTrackingScreen.kt`, `presentation/more/settings/widget/TrackingPreferenceWidget.kt` via its caller). Behaviour detail is in `p1-behavior.md` and is referenced by section number, not repeated. Target is GTK 4.16 with no libadwaita, using the user's GTK theme (Tokyo Night per the user's memory; never reset to stock colours).

Contents: 1 screens and where they live; 2 Settings > Tracking; 3 track info sheet; 4 logo tile; 5 selectors; 6 track search; 7 login and logout dialogs; 8 remove dialogs; 9 snackbars and toasts; 10 states; 11 keyboard and mouse; 12 responsive behaviour; 13 theme and colour roles; 14 gaps in the current mockup.

---

## 1. Screens and where they live

| Screen | Android file (part 1) | Kurobayomi module | Opened from |
|---|---|---|---|
| Settings > Tracking | `presentation/more/settings/screen/SettingsTrackingScreen.kt` | `src/track/settings.rs` | Settings menu, `settings_tracking` |
| Per-entry tracking sheet (anime and manga) | `presentation/track/anime/AnimeTrackInfoDialogHome.kt`, `presentation/track/manga/MangaTrackInfoDialogHome.kt` | `src/track/ui/info.rs` | entry details "Tracking" button, "Add tracking" after add-to-library |
| Track search (anime and manga) | `presentation/track/anime/AnimeTrackerSearch.kt`, `presentation/track/manga/MangaTrackerSearch.kt` | `src/track/ui/search.rs` | "Add tracking", row title tap, auto-open |
| Pickers (status, number, score, date) | `presentation/track/TrackInfoDialogSelector.kt` | `src/track/ui/selectors.rs` | info sheet rows |
| Logo tile | `presentation/track/components/TrackLogoIcon.kt` | `src/track/ui/logo.rs` | info sheet, settings rows |
| Login and logout dialogs | `SettingsTrackingScreen.kt` (private composables) | `src/track/ui/login.rs` | settings tracker rows |
| Remove tracking dialog | part 2 (`ui/entries/*/track/*`) | `src/track/ui/remove.rs` | info sheet overflow menu |
| Date removal dialog | part 2 | `src/track/ui/remove.rs` | date picker "Remove" |

Part 2 files (`ui/entries/anime/track/AnimeTrackInfoDialog.kt`, `ui/entries/manga/track/MangaTrackInfoDialog.kt`) host the screens above; their state flows are in `p1-behavior.md` sections 13.8, 15.1 and 16.

---

## 2. Settings > Tracking

### 2.1 Layout (top to bottom)

1. App bar: title "Tracking" (`MR.strings.pref_category_tracking`). Trailing help button (question-mark icon, tooltip "Tracking guide"). **Parity decision:** drop the external guide link (P3, task T-27a-047). Keep the button only if Kurobayomi has its own docs.
2. Four preference rows, in order (behaviour 12.2):
   - Switch "Update progress after reading/watching", default on.
   - Switch "Open track menu on adding to library", default on.
   - Switch "Show next episode's airing time " (trailing space in the source string; trim it when rendering), default on.
   - List "Update progress when marked as read/watched": Always / Always ask / Never, default Always. Shown as a combo row with the current value as subtitle.
3. Group header "Trackers" (`MR.strings.services`). Rows for MyAnimeList, AniList, Kitsu, MangaUpdates, Shikimori, Simkl, Bangumi (behaviour 12.3). Each row: 48 px logo tile, tracker name (title style, one line, ellipsis), a green check icon when logged in. Whole row tappable. Tap while logged out: login (browser or dialog). Tap while logged in: logout dialog.
4. Info text under the group (`MR.strings.tracking_info`, secondary text, wrapped).
5. Group header "Enhanced trackers" (`MR.strings.enhanced_services`). Rows only for enhanced trackers whose source is installed (behaviour 12.4). Tap: no-op login (`loginNoop`), logout dialog when logged in.
6. Info text under the group: `MR.strings.enhanced_tracking_info`, plus the "not installed" line when any tracker is missing a source.

### 2.2 Row states

| State | Logo | Name | Trailing |
|---|---|---|---|
| Logged out | normal | normal | none |
| Logged in | normal | normal | green check (16 px visual, 32 px box, tint #4CAF50, a11y "Logged in") |
| Logged out, source missing (enhanced) | not shown in the list; named in the info text | | |
| Auth expired (MyAnimeList only) | normal | normal | none (the flag is not shown in the source UI; **Proposal:** show a warning dot, since the flag exists) |

### 2.3 Interactions

- Switches toggle immediately and persist (no confirm).
- The list shows a popup of three options. Selecting one persists it immediately.
- Login via browser: the row opens the default browser on the authorise URL and shows no spinner. Return is through the redirect (behaviour 17). The check appears when the redirect completes.
- Login via dialog: see section 7.

### 2.4 Missing and empty states

- No enhanced tracker with an installed source: the "Enhanced trackers" group still exists, with only the info text (source behaviour: the group is always rendered).
- No logged-in tracker: the check marks are all absent. No empty-state illustration.

---

## 3. Track info sheet (per-entry)

### 3.1 Container

- Presented as a dialog-like page with a scrollable column, 16 dp padding, 24 dp spacing between tracker cards (behaviour 13.1). In the Kurobayomi mockup the sheet is a page reached from the entry details (`open("tracking_sheet")`).
- One card per tracker (logged-in trackers only, filtered by kind). Order is registry order (behaviour 1.2).

### 3.2 Card with a binding (behaviour 13.2)

Header row, left to right:
1. Logo tile (48 px). Badge overlay with a "Tracked privately" eye-off icon (14 px, primary container) when private. Click opens the remote page. Long-press copies the link.
2. Title (title-medium, one line, ellipsis). Height 48. Click re-opens search pre-filled with the title. Long-press copies the title.
3. Vertical divider, then the overflow button (three vertical dots, tooltip "More").

Detail block (rounded card, surface-container-highest background, padding 8 dp, clip 6 dp):
- Row 1, equal-width cells separated by vertical dividers: Status | Progress | Score (score cell only when the tracker has a score list).
- Row 2 (only when the tracker supports reading dates): Start date | Finish date, separated by a horizontal divider above.
- Each cell: centred text, body-medium, max 2 lines, ellipsis, 12 dp padding, min height from the row.
- Unset cell: placeholder text at 50% alpha (behaviour 13.7).

Cell content:
- Status: the tracker's label, or empty.
- Progress: "12" or "12 / 24" when the total is known. For anime the label is the episode count; for manga the chapter count.
- Score: the tracker's display string, or the placeholder "Score" when unset or 0.0.
- Start date / Finish date: localised date from the user's date format (`UiPreferences.dateFormat`), or placeholder "Start date" / "Finish date".

### 3.3 Card without a binding (behaviour 13.6)

Logo tile (not clickable), then a text button "Add tracking" filling the rest of the row, 16 dp start padding. Click opens search (or enhanced registration for enhanced trackers).

### 3.4 Overflow menu items (behaviour 13.5)

In order: "Open in browser", "Copy link", "Track privately" or "Track publicly" (only for trackers with private support), "Remove". The menu closes after each item.

### 3.5 Loading and refresh

- On open, the sheet shows the local data immediately from the database, then refreshes each binding in the background. No spinner is shown per card (source behaviour). **Proposal:** a thin progress bar under the title row while refresh is running, so the user knows values may still change.
- A failed refresh shows a toast "<tracker> error: <message>" (behaviour 13.8). The card keeps local data.

### 3.6 Interactions summary

| Gesture | Target | Result |
|---|---|---|
| Click | logo | open remote page |
| Long-press | logo | copy link, toast "Copied to clipboard:\n<link>" (source: toast only on Android 12 and lower, `ContextExtensions.kt:46-50`; Kurobayomi always shows it) |
| Click | title | open search pre-filled |
| Long-press | title | copy title, same toast with the title (`copied_to_clipboard`) |
| Click | status cell | status picker |
| Click | progress cell | number picker |
| Click | score cell | score picker |
| Click | start/finish cell | date picker |
| Click | "Add tracking" | search, or enhanced match |

---

## 4. Logo tile (behaviour 13.3)

- 48 x 48 px, rounded with the medium shape (the mockup uses 8 px corners), background = the tracker's brand colour, 4 px padding, logo centred.
- Logo image is the tracker's own asset (areas 28 to 33). If the asset is missing, render the first letter of the name in title-large, white.
- Clickable variant has a pointer cursor and a press highlight, with no ripple indication (the source uses no indication).

---

## 5. Pickers

All pickers are modal windows with title, body, and a button row: Cancel (text), OK (text). The optional third button (Remove, for dates) sits at the start, followed by a flexible spacer. Button row padding and spacing as in behaviour 14.1.

### 5.1 Status (behaviour 14.2)

- Title "Status".
- Body: scrollable list of radio rows, each 48 px minimum height, label body-large, 24 dp start indent. The whole row is the hit target.
- Dividers appear at the top and bottom only when the list scrolls.
- OK applies the selected status. Cancel closes without change.
- Keyboard: Up and Down move the selection, Space selects, Enter confirms.

### 5.2 Number (behaviour 14.3)

- Title "Chapters" or "Episodes".
- Body: vertically scrolling wheel, centred, value-large. Range `0..=max` where `max = total` when known, else 10000.
- Desktop equivalent: a spin button with an adjustment. Up and Down arrows step by 1, Page Up and Page Down step by 10, Home and End go to the range limits, mouse wheel scrolls.
- Starting value: the current progress.

### 5.3 Score (behaviour 14.4)

- Title "Score".
- Body: a drop-down list of the tracker's score strings (`getScoreList()`), current value selected.
- Starting selection: the current display score. If it is not in the list, the middle entry (source rule; the defect D8 fix is to start at the actual index, including 0).

### 5.4 Date (behaviour 14.5, 14.6)

- Title "Start date" or "Finish date".
- Body: calendar (month grid, prev/next month, year drop-down). Future dates disabled. Start date may not be after the finish date, end date may not be before the start date.
- Desktop: `gtk::Calendar` inside a popover, with days outside the allowed range rendered insensitive. Keyboard: arrow keys move by day, Page Up/Down by month, Shift plus Page Up/Down by year.
- Buttons: "Remove" (only when a date is set), Cancel, OK.

### 5.5 Remove date confirmation (behaviour 14.7)

- Icon: trash (16 px) above the title, centred title "Remove date?".
- Text: "This will remove your previously selected start date from <tracker>" (or finish date).
- Buttons: Cancel (text), Remove (filled tonal, error tint).

---

## 6. Track search (behaviour 15)

### 6.1 Header

- Left: back arrow (returns to the info sheet).
- Centre: single-line search field, placeholder "Search…", prefilled with the title. Enter or IME search runs the query.
- Right: clear (X) icon appears when the field has text. Clear empties the field and focuses it.
- Divider under the header.

### 6.2 Body states (behaviour 15.3)

| State | Rendering |
|---|---|
| Loading | centred spinner (`gtk::Spinner`), no text. Keep the header usable. |
| Results | vertical list of cards, 12 dp gaps, 12 dp side margins, 12 dp vertical padding. |
| No results | centred empty state: text "No results found". |
| Error | centred text with the error message, or "Unknown error" when the message is empty. |

### 6.3 Result card (behaviour 15.4)

- Container: rounded 16 px, surface background, 2 px border in the outline colour when selected (transparent otherwise), padding 12 px.
- Selected mark: check-circle icon at the top right in primary colour.
- Left: cover, 96 px tall, 2:3 book ratio, rounded 4 px.
- Right column, 12 px gap from the cover:
  - Title (title-medium, max 2 lines, ellipsis, 28 px right padding so it clears the check icon).
  - Authors and artists joined with ", " (body-small, secondary, 1 line).
  - Detail lines "Type: …", "Started: …", "Status: …", "Score: …". Label title-small, value body-medium secondary, 1 line each. Hidden when empty. Status and type are shown capitalised after lower-casing (behaviour 15.4).
- Below the row: summary, trimmed, body-small, secondary, max 4 lines, 24 px top gap.
- Click: selects the card (and clears focus so the IME hides). Clicking a selected card keeps it selected (source does not toggle).
- Long-press or right-click: menu with "Copy to clipboard" and "Open in browser".

### 6.4 Bottom bar

- Hidden until a card is selected, then slides up with a fade (behaviour 15.2).
- Row, 8 px padding: "Track" (filled, flexible width) and, when the tracker supports private tracking, an icon button with the eye-off icon and tooltip "Track privately".

### 6.5 Keyboard and focus

- The search field has focus on open (source requests focus on the field).
- Tab order: back, field, clear, results list, bottom bar buttons.
- Enter in the field dispatches the query. A blank query is **not** sent (Proposal, defect D11).

---

## 7. Login and logout dialogs (behaviour 12.5, 12.6)

### 7.1 Login (credential trackers: Kitsu, MangaUpdates)

- Window title: "Log in to <tracker>".
- Close (X) icon in the header.
- Field 1: label "Email address" (Kitsu) or "Username" (MangaUpdates). Single line. Pre-filled with the stored value. Red outline on failed login.
- Field 2: label "Password", masked, with an eye toggle (initially masked). Red outline on failed login.
- Single action button "Login" (filled, full width). Disabled until both fields are non-blank. Shows "Loading…" and is disabled while the request runs.
- Enter in field 1 moves to field 2. Enter in field 2 submits.
- Success: toast "Logged in", window closes. Failure: toast with the error text, fields marked, window stays open with the typed values.
- Proposal: keep the window open with an inline error label under the fields (not only a toast), so the message is visible while the window is open.

### 7.2 Login (browser trackers)

No window. The row opens the browser on the authorise page. While waiting, show a non-modal banner at the top of the settings page: "Waiting for <tracker> in your browser…" with a Cancel button. Cancel discards the pending auth state (behaviour 17). **Proposal:** the source has no waiting state; this is added so the Linux flow is understandable.

### 7.3 Logout

- Title: "Log out from <tracker>?" centred.
- Buttons: Cancel (text) and "Log out" (filled, error colour). Cancel is the default focused button.
- Success: toast "You are now logged out", the row's check disappears.

---

## 8. Remove tracking dialog (part 2 host, behaviour 16)

- Title: "Remove <tracker> tracking?". Body: "This will remove the tracking locally.".
- Checkbox "Also remove from <tracker>", shown only for trackers that can delete remotely (MangaUpdates; both kinds).
- Buttons: Cancel (text), OK (filled tonal, error tint).
- Ordering: see behaviour defect D16. **Proposal:** remote delete first, then local; if the remote delete fails show "Remote removal failed: <message>" and keep the local binding.

---

## 9. Snackbars and toasts

| Message | Trigger | Duration | Action | Source key |
|---|---|---|---|---|
| "Logged in" | login success | 3 s toast | none | `MR.strings.login_success` |
| "You are now logged out" | logout | 3 s toast | none | `MR.strings.logout_success` |
| "<tracker> error: <message>" | refresh failure | 3 s toast | none | `MR.strings.track_error` |
| raw message | register or remote update failure | 3 s toast | none | `e.message` (no key) |
| "No match found" | enhanced match failure | 3 s toast | none | `MR.strings.error_no_match` |
| "Update trackers to chapter N?" | ASK prompt after mark read | 4 s, dismissible | OK | `MR.strings.confirm_tracker_update` |
| "Update trackers to episode N?" | ASK prompt after mark watched | 4 s, dismissible | OK | `AYMR.strings.confirm_tracker_update_anime` |
| "Trackers updated to chapter N" | ALWAYS after mark read | 3 s toast | none | `AYMR.strings.trackers_updated_summary_manga` |
| "Trackers updated to episode N" | ALWAYS after mark watched | 3 s toast | none | `AYMR.strings.trackers_updated_summary_anime` |

Presentation: an in-app overlay at the bottom centre (area 36 owns the component). The ASK prompt is a banner with the message, an OK text button, and a close (X) icon. Toasts stack. Do not use desktop notifications for these (they are transient and in-window).

| "Copied to clipboard:\n%1$s" (`MR.strings.copied_to_clipboard`, `i18n/.../strings.xml`) | copy link, copy title, search card "Copy to clipboard" | 3 s toast; `%1$s` is the content truncated in the middle to 50 characters with "..." (`truncateCenter(50)`, `core/common/.../StringExtensions.kt:24-32`) | none | source shows it on Android 12 and lower only (`ContextExtensions.kt:46-50`) |

The toast after copy is shown on every Kurobayomi build. Keyboard and mouse users get no other visual confirmation.

---

## 10. States per screen

| Screen | Loading | Empty | Error | Selection | Offline |
|---|---|---|---|---|---|
| Settings > Tracking | not applicable (synchronous prefs) | no trackers logged in: every row shows no check | not applicable | not applicable | login by browser still opens; the redirect fails later |
| Info sheet | local data shown immediately; refresh runs in background | "Add tracking" card for each logged-in tracker without binding | per-tracker toast | not applicable | local data shown; refresh toasts "<tracker> error: …" |
| Search | spinner in body | "No results found" | message or "Unknown error" | one card selected at a time; bottom bar appears | search fails, error state |
| Status picker | not applicable | not applicable (list comes from tracker, never empty in practice) | not applicable | one radio | not applicable |
| Number picker | not applicable | not applicable | not applicable | current value | not applicable |
| Score picker | not applicable | list empty only if the tracker has no scores; then the score cell is hidden (`supportsScoring` false) | not applicable | one row | not applicable |
| Date picker | not applicable | not applicable | not applicable | one date | not applicable |

Offline: the ask-before-update path needs no network until OK is pressed. Progress updates are queued (behaviour 9, 10). The UI does not show a queue indicator in the source. **Proposal:** a small "pending" badge on the info sheet title row when `track_retry_queue` has items for the entry.

---

## 11. Keyboard and mouse

- Every row, card and cell is focusable and activates with Enter or Space.
- Context menus: right-click on a search card, on a tracker logo, and on the title. The long-press behaviour of the source maps to right-click on desktop; long-press is also supported with touch.
- Double-click on a title in the info sheet: no effect in the source (single click opens search). Keep single click.
- Escape closes every dialog and picker. In the search page Escape returns to the sheet.
- Mouse wheel on number and score pickers: changes the value by one step (the source's wheel is touch-scroll). Do not change the value on wheel over a list that also scrolls; use the scroll direction only when the pointer is over the control.

---

## 12. Responsive behaviour

- The info sheet keeps a maximum content width of 720 px on wide windows, centred. The source uses the full width of the window.
- Under 480 px width, the detail block switches from three equal cells to a vertical stack (status, progress, score), each a full-width row. The source does not reflow; this is needed for narrow windows and is a Proposal.
- The search result card keeps the cover at 96 px. Under 480 px, the detail lines wrap beneath the title rather than beside the cover (Proposal).
- Pickers: width 420 px portrait-equivalent (the source uses a centred alert dialog with the window inset). Height grows to content, max 80% of the window.

---

## 13. Theme and colour roles

Map the source's Material 3 roles to GTK CSS classes so the user's GTK theme drives colours (Tokyo Night, per the user's memory note). No hard-coded colours except the logo brand colours and the logged-in green.

| Source role | Used for | GTK mapping |
|---|---|---|
| `surface` | search card background | `.card` background (theme) |
| `surfaceContainerHighest` | info detail block | `.view` or `.background` one step darker than the card (theme token) |
| `onSurface` | titles | default foreground |
| `onSurfaceVariant` | secondary text and icons | `.dim-label` |
| `primary` / `onPrimary` | private badge, selected mark, primary buttons | `.suggested-action` tint |
| `errorContainer` / `onErrorContainer` | remove and logout buttons | `.destructive-action` |
| `outline` | selected search card border | `border-color` from theme `borders` |
| Logged-in green `#4CAF50` | check icon | the only hard-coded colour in the tracking UI |
| Tracker brand colour | logo tile background | `Color(tracker.getLogoColor())` as inline CSS per tile |

Dark and light: follow the GTK theme. No separate tracking palette.

---

## 14. Gaps in the current mockup (Kurobayomi `src/entry.rs`, `spec/entries.json`, `spec/settings.json`)

Current state from the source files:

1. `tracking()` (`src/entry.rs:496-537`): three hard-coded trackers ("AniList" bound, "MyAnimeList" and "Kitsu" unbound). Values "Watching", "12 / 24", "8.0", "Start date", "Finish date" are fixed. Missing: real data, logos, private badge, the 48 px logo tile behaviour, the click and long-press actions, title copy, the detail block layout rules, placeholders at 50% alpha, the empty "Add tracking" card for each tracker.
2. Missing: refresh on open, the failure toast, the pending-queue indicator.
3. `tracker_search()` (`src/entry.rs:539-568`): four static results ("Title 1".."Title 4" via `title_n`, "Author name", fixed detail lines). Missing: search, loading, empty, error states; cover images; selection and bottom bar wiring to real data; private track button shown only when supported; long-press menu; copy and open actions; description with four-line clamp; the blank-query rule.
4. Pickers are spec-driven (`spec/entries.json` dialogs `status`, `chapters`, `episodes`, `score`, `start_date`, `finish_date`, `remove_date`, `remove_tracking`). Gaps: chapter and episode lists are fixed 0..24 (real range `0..total` or `0..10000`); no date constraints; the score picker has no start-index rule; the date remover text has no tracker name.
5. Settings (`spec/settings.json` `settings_tracking`): the spec lists dialogs `tracker_login` and `tracker_logout` with names of trackers hard-coded ("Log in to Kitsu"). The real rows come from the tracker list and the dialogs take the tracker name. The enhanced-services group, the not-installed text and the auto-update list are not in the spec.
6. The mockup has no OAuth waiting state, no redirect handling, and no "Remove from <tracker>" checkbox logic.
7. No `Tracked privately` badge, no "Track publicly" menu item toggling.
8. No copy confirmations.
9. No narrow-width layout for the detail block and search card.

Required change (summary): replace the static bodies of `tracking()` and `tracker_search()` with the `track::ui` modules in section 1. Keep `open()` and `dialog_of()` for navigation only.
