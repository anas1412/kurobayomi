# area:28-tracker-anilist, part 1 — UI parity (AniList-visible surfaces)

None of the 13 scope files is a Compose screen or Activity body. `TrackLoginActivity.kt` is an Activity but renders only a `LoadingScreen` (`app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt:22-24`). The visible AniList UI lives in presentation and entry files owned by area:27-tracking-core and the settings area. This file lists the AniList-specific visible behaviour those files produce, so the Rust UI can match it. Source references are repo-relative.

Desktop mapping: GTK 4 (vanilla, no libadwaita), keyboard and mouse. Dialogs become `gtk::Window` modals or `adw`-free `gtk::Dialog`-style windows.

## 1. Settings: AniList row

Source: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt:171-180`, `app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt:219-230`.

- Row inside group "Trackers" (`MR.strings.services`), between MyAnimeList and Kitsu.
- Visible: tracker logo, name "AniList", a switch.
- Switch checked = `isLoggedInFlow` (username and password non-empty). Live-updating via the flow.
- Switch off → `login()` → opens the authorize URL in the default browser (`forceDefaultBrowser = true`). Nothing else is shown; the user completes login in the browser and returns via the custom scheme.
- Switch on → `dialog = LogoutDialog(aniList)`.
- Logout dialog: title `MR.strings.logout_title` formatted with name → "Log out from AniList?"; buttons Cancel (`MR.strings.action_cancel`) and a red destructive confirm button; on confirm: `tracker.logout()`, dismiss, toast `MR.strings.logout_success` "You are now logged out".
- The "Tracking guide" action in the app bar opens `https://aniyomi.org/help/guides/tracking/` (`SettingsTrackingScreen.kt:80`). Rust: keep the link (points to the Aniyomi guide; decide replacement in area:settings).
- No score-format control exists. Do not add one; the format comes from AniList (p1-behavior §3.1).

Rust parity requirements:
- The row must show "logged out" state with no pending login spinner; there is no in-app progress indicator during OAuth.
- After returning from the browser, the settings window must refresh the switch state (Kotlin does so by `MainActivity` re-entry; Rust: subscribe to the auth-state change signal).
- Failed login (token missing or Viewer query failed): the Kotlin silently stays logged out. Rust: show a toast `anilist_err_login_failed` with the error text (p1-edge-cases E-01). This is a deliberate improvement.

## 2. Redirect landing

- The Kotlin Activity has only a `LoadingScreen` while it processes the URI, then returns to `MainActivity` (`BaseOAuthLoginActivity.kt:19-36`). No success toast is shown by the redirect Activity; the settings screen shows the new state.
- Rust: while the token exchange (Viewer query) is pending, show the app window with a `gtk::Spinner` in the settings row labelled "Logging in…" (new string). This is a proposal; the Kotlin shows a blank loading screen.

## 3. Tracker info panel (per entry): AniList card

Source: `app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt` (manga equivalent `app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackInfoDialogHome.kt`).

Layout (vertical, 24 dp gaps, 16 dp padding, scrollable; `AnimeTrackInfoDialogHome.kt:71-79`):
1. Header: logo, tracker name, title of the bound track (`item.track.title`), status label, overflow menu (Open in browser, Copy link, Search again, Remove).
2. Rows (each tappable):
   - Status: label from `getStatusForAnime(status)`; tap → status picker.
   - Episodes (anime) / Chapters (manga): `"12 / 24"` when total > 0, else `"12"`. Tap → progress picker.
   - Score: shown only when `track.score != 0.0`; text `displayScore`. Tap → score picker. Row is always tappable (`onScoreClick` set) even when no score is shown.
   - Start date: shown only when `startDate != 0`; tap → date picker (always tappable).
   - End date: shown only when `finishDate != 0`; tap → date picker.
   - Private: toggle row, shown because `supportsPrivateTracking`. Label/icon `VisibilityOff`; on tap → `setRemotePrivate(!private)`.
3. Empty state for an unbound tracker: `TrackInfoItemEmpty` with a "Search" action (`onNewSearch`).

Rust mapping: a `gtk::ListBox` with `boxed-list` style; date and score rows as `gtk::ActionRow`-like `gtk::Box` rows (no libadwaita; build with `gtk::Label`s). The mockup already has the grid (`src/entry.rs:509-520`); replace with the list (requirement: status, progress, score, start, finish, private, open-in-browser, copy-link, search, remove).

Date display: the Kotlin uses a `dateFormat` value defined in the info dialog file (pattern not checked in this area; treat as a localized short date). Rust: use the system locale via `glib::DateTime::format("%x")`; fall back to ISO `yyyy-MM-dd` when the locale format is unavailable.

## 4. Status picker

Source: `AnimeTrackInfoDialog.kt:287-340`; widget `BaseSelector` in `app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt:195-233`.

- Title: "Status" (`MR.strings.status`).
- Radio list in the order of p1-behavior §2.2; selected row shows a filled radio.
- Buttons: Cancel (`action_cancel`), OK (`action_ok`). Confirm → `setRemoteAnimeStatus`.
- Selecting COMPLETED also sets progress to total (when total > 0) — shown only after confirm, not in the picker.
- Labels: §2.3.

## 5. Progress picker

- Title: "Episodes" (`AYMR.strings.episodes`) for anime, "Chapters" (`MR.strings.chapters`) for manga (`TrackInfoDialogSelector.kt:105`).
- Wheel number picker with range `0..total` (total > 0) or `0..10000` (unknown). Initial = current progress (integer part).
- Confirm → `setRemoteLastEpisodeSeen` / `setRemoteLastChapterRead`.
- Rust: `gtk::SpinButton` with `adjustment(0, 0, max, 1)`; keyboard arrows and typed input. The 10 000 cap applies only when total unknown.

## 6. Score picker

- Title "Score" (`MR.strings.score`).
- Wheel text picker of `getScoreList()`; initial = index of `displayScore` text, falling back to middle (`TrackInfoDialogSelector.kt:135`). Rust fix: initial = `score_to_index` (p1-rust-design §5). Visible difference: unscored POINT_3 opens at 0 ("-") in Rust, at 😐 in Kotlin.
- Format-specific widgets (Rust, proposal in p1-rust-design §5):
  - POINT_3: 4 radio buttons: "-", 😦, 😐, 😊.
  - POINT_5: 6 radio buttons: "0 ★" … "5 ★".
  - POINT_10 / POINT_100 / POINT_10_DECIMAL: spin button with suffix showing the decimal (e.g. "8.5" for POINT_10_DECIMAL index 85).
- Confirm → `setRemoteScore`.

## 7. Date picker

- Title "Start date" or "Finish date" (`track_started_reading_date`, `track_finished_reading_date`).
- Calendar date picker (`DatePicker` in Compose). Rust: `gtk::Calendar` (GTK 4 built-in) plus OK/Cancel.
- Disabled dates (Kotlin `SelectableDates`, `AnimeTrackInfoDialog.kt:465-520`):
  - any date after today (local)
  - start picker: dates after the finish date when finish is set
  - finish picker: dates before the start date when start is set
- Remove button (only when the date is set) → confirm dialog:
  - Title `track_remove_date_conf_title` "Remove date?"
  - Text `track_remove_start_date_conf_text` / `track_remove_finish_date_conf_text` with the service name.
  - Buttons: Cancel, Remove (`action_remove`, error-container colour).
- Rust: match the same disabled-day rules by setting `gtk::Calendar` day-selection constraints (not built-in; implement by reverting invalid selections and disabling OK). Acceptable degradation: show invalid days greyed via CSS class `dim-label`.

## 8. Search screen (AniList results)

Source: `app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt:89-419`, `manga/MangaTrackerSearch.kt` (same structure).

Top bar:
- Back arrow (dismiss).
- Text field, single line, `ImeAction.Search`, placeholder `action_search_hint` "Search…"; Enter triggers search (`runOnEnterKeyPressed`).
- Clear (×) button when text not empty.
- Divider below.

Body:
- Loading state while `queryResult == null`: centred loading indicator (`LoadingScreen`).
- Success with rows: vertical list, 12 dp gaps, 12 dp horizontal padding, 12 dp vertical content padding.
- Empty: "No results found" (`no_results_found`), centred.
- Error: centred text = error message or "Unknown error".

Row (`SearchResultItem`, `AnimeTrackerSearch.kt:237-356`):
- Rounded 16 dp card, surface colour, 12 dp padding, 2 dp border (outline colour when selected, transparent otherwise).
- Selected: check-circle icon top-end (primary colour).
- Left: cover 96 dp tall (`ItemCover.Book`), 12 dp gap.
- Right column:
  1. Title, 2 lines max, ellipsis, title-medium, 28 dp end padding (room for the check icon).
  2. Authors + artists joined by ", " with distinct, 1 line, body-small, secondary alpha. Shown only if non-empty.
  3. "Type: <type>" (`track_type`) — e.g. "Manga", "One-shot", "Tv". Only if not blank.
  4. "Started: <yyyy-MM-dd>" (`label_started`). Only if not blank.
  5. "Status: <status>" (`track_status`). Only if not blank.
  6. "Score: <score as Double text>" (`score`). Only if score != -1.0. Example: "72.0".
  7. Summary: trimmed, body-small, max 4 lines, 24 dp top spacing, secondary alpha. Only if not blank.
- Long-press (or context menu key) → menu: "Copy to clipboard" (`action_copy_to_clipboard`, copies title), "Open in browser" (`action_open_in_browser`, opens `tracking_url` if not blank).

Rust parity notes:
- Type and status strings: `publishing_type.to_lowercase()` then capitalise first letter. Status keeps underscores (Kotlin behaviour, e.g. "Not_yet_released"); decide whether to replace `_` with space in Rust (proposal: yes, labelled as a deliberate fix in release notes).
- Score text: Kotlin shows Double.toString ("72.0"). Rust proposal: show integer ("72") — deliberate difference; keep the value.
- Selection: single-select; tapping a selected item keeps it selected (Kotlin `onSelectedChange` sets same item).
- Bottom bar (appears with slide-up + fade when an item is selected): "Track" button (`action_track`) and, when `supportsPrivateTracking`, a "Track privately" icon button (`action_toggle_private_on`, VisibilityOff icon). Both call `register` with `private` false / true.
- Keyboard: Enter in the search field searches; Up/Down move selection (Kotlin does not provide this; add as a desktop affordance); Enter on a selected row confirms "Track".

Preselection: when opened from an existing binding, the search pre-selects the result whose `tracking_url` equals the bound track's URL (`AnimeTrackInfoDialog.kt:750`). If the bound track has no URL (empty), nothing is pre-selected.

## 9. Remove tracking dialog

Source: `AnimeTrackInfoDialog.kt:772-847`.

- Title: "Remove <service> tracking?" (`track_delete_title`).
- Body: "This will remove the tracking locally." (`track_delete_text`).
- Checkbox "Also remove from AniList" (`track_delete_remote_text`), shown only when the tracker is deletable (AniList is).
- Buttons: Cancel; OK (`action_ok`) styled as tonal destructive.
- On OK: local unregister always; remote delete only if the checkbox is ticked. The remote delete happens after the dialog closes (fire-and-forget); its errors are logged only (no toast). Rust: show a toast on remote-delete failure (Kotlin gap, p1-edge-cases E-19).

## 10. Toasts and snackbars (AniList-related)

| Event | Kotlin text (key) | Rust |
|---|---|---|
| Tracker refresh failed on info open | `track_error`: "AniList error: <message>" | same |
| Login succeeded (generic dialog only) | `login_success` "Logged in" | not used for AniList; Rust shows "Logged in to AniList" (new key) |
| Logout success | `logout_success` | same |
| Update failure after a progress change | `e.message` in toast (framework `updateRemote`) | `anilist_err_*` keys |
| Auto-update summary (ALWAYS) | `trackers_updated_summary_anime` "Trackers updated to episode %d" | same |
| Auto-update prompt (ASK) | snackbar `confirm_tracker_update_anime` "Update trackers to episode %d?" with action "OK" (`MR.strings.action_ok`), `SnackbarDuration.Short`, dismiss action enabled (`AnimeScreenModel.kt:899-904`) | same; action "OK" triggers `trackEpisode` |
| Enhanced match failure | `error_no_match` "No match found" | not AniList |

## 11. Keyboard and mouse equivalents

| Kotlin gesture | Desktop equivalent |
|---|---|
| Tap row | click |
| Long-press search row | right-click or Menu key → same menu |
| Tap track info row | click; keyboard focus via Tab; Enter activates |
| Swipe to dismiss sheets | Escape closes dialogs |
| Pull-to-refresh (not present in tracking) | none |

## 12. Responsive behaviour

- Kotlin: bottom sheet / full-screen dialog on phones.
- Rust: window minimum width 360 px (phone-like). The search results list uses full width; the cover is fixed 96 px tall (width derived). Below 480 px, the summary may drop to 2 lines (Rust-only tweak).

## 13. Empty, loading and error states (AniList)

| Surface | Loading | Empty | Error |
|---|---|---|---|
| Search | `LoadingScreen` centred | "No results found" | error text or "Unknown error" |
| Info panel (refresh on open) | silent (no spinner; refresh runs in background) | unbound: "Add tracking" card | toast `track_error` |
| Settings row | none | logged out | logged out (Kotlin); Rust shows error toast |
| Status/progress/score picker | immediate (no network before confirm) | n/a | confirm failure → toast `e.message` (framework); Rust keeps the picker closed and shows toast |

## 14. What the existing mockup lacks (src/entry.rs, src/settings.rs, spec/*.json)

1. Real data: the tracking sheet shows hard-coded `"Watching"`, `"12 / 24"`, `"8.0"` (`src/entry.rs:509`). Needs `TrackerService` data.
2. Score row is always shown; Kotlin hides it when unscored (`score != 0`).
3. Missing rows: "Open in browser", "Copy link", "Search again" are absent from the menu (`menu_button` with only "Remove" at `src/entry.rs:514`).
4. Private toggle row is absent from the info panel (Kotlin shows it for AniList).
5. Date rows are shown even when unset (Kotlin hides the date value unless set, but keeps the tap target).
6. Tracker search results are fake (`src/entry.rs:546-553`); no selection → confirm wiring to `bind`.
7. The "Track privately" button does nothing (`src/entry.rs:564`).
8. Search row lacks: authors/artists line, type/status/started/score detail lines, summary, long-press menu, check indicator.
9. The score picker and status picker dialogs are not implemented (dialog ids `status`, `chapters`, `score`, `start_date`, `finish_date` exist only as names in `src/entry.rs:509-520`).
10. Settings: no AniList login/logout row in `spec/settings.json` `settings_tracking` (only the screen title and subtitle exist, lines 34-38).
11. No redirect handling: the `kurobayomi://anilist-auth` scheme is not registered (no `.desktop` file, no argv handling in `src/main.rs`).
12. No "Logging in…" state and no auth-expired state.
13. Dates are not formatted via locale.

Covering these gaps is tracked in p1-tasks.md (T-28a-031 to T-28a-058; mapping: gap 1-5 and 8 → T-28a-031, 030, 051; gap 6-7 → T-28a-057; gap 9 → T-28a-053, T-28a-054, T-28a-055, T-28a-056; gap 10-11 → T-28a-047, T-28a-040; gap 12 → T-28a-048; gap 13 → T-28a-050).
