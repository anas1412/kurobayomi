# Area 29-tracker-myanimelist, part 1: UI parity

The scope files do not draw screens. They supply values and actions that the tracking UI uses. The UI itself lives in files outside this scope (listed in `p1-inventory.md`). This document lists the MAL-visible parts only: the login row, the logout dialog, the login return screen, the delete dialog, the labels, the score picker, and the search result fields. Anything not listed here is owned by area:27-tracking-core.

Source paths are relative to `/home/blackbox/aniyomi-src`.

## 1. Settings row: "MyAnimeList" (Settings > Trackers)

Source: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt:161-168` builds `Preference.PreferenceItem.TrackerPreference(tracker = trackerManager.myAnimeList, login = {...}, logout = { dialog = LogoutDialog(trackerManager.myAnimeList) })`. Rendering: `app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt:219-228`.

Elements:
- Tracker logo tile (`ic_tracker_mal`, colour `#2E51A2`). Shown by `TrackingPreferenceWidget` (outside scope).
- Name "MyAnimeList" (`Tracker.name`, `MyAnimeList.kt:28`).
- Switch-style checked state = `isLoggedIn`, observed through `isLoggedInFlow` (`PreferenceItem.kt:220-222`). `isLoggedIn` is true when username and password are both non-empty. It stays true while the auth-expired flag is set (`p1-behavior.md` section 4.10).
- Tap when logged out: opens the MAL authorise page in the default browser (`context.openInBrowser(MyAnimeListApi.authUrl(), forceDefaultBrowser = true)`).
- Tap when logged in: opens the logout dialog (section 2).

States:
- Logged out: unchecked. Tap starts browser login.
- Logged in: checked. Tap asks to log out.
- Logged in but auth-expired (refresh returned 401): still checked, because the UI reads only username and password. API actions fail with `MAL: Login has expired`. Kurobayomi shows a warning state (proposal, section 7).
- Login in progress: no state. The browser is open. The app shows nothing until return (observed).

Proposal (Linux): the browser flow is replaced by the loopback flow in `p1-rust-design.md` section 3.6. The row shows "Log in" while the loopback server waits, with a "Paste redirect URL" fallback and a Cancel button. The row returns to its normal state on success, failure, or timeout (5 minutes).

## 2. Logout dialog

Source: `SettingsTrackingScreen.kt` `TrackingLogoutDialog` (lines 366-408). Observed:
- Title: `MR.strings.logout_title` formatted with the tracker name: "Log out from MyAnimeList?" (`logout_title` = `Log out from %1$s?`).
- No body text.
- Buttons, in order: `OutlinedButton` "Cancel" (`action_cancel`, dismisses) and `Button` "Log out" (`MR.strings.logout`, error container colours, dismisses, runs `tracker.logout()`, then shows toast "You are now logged out", `MR.strings.logout_success`). Both buttons have `Modifier.weight(1f)`, in a `Row` with `extraSmall` spacing.

Proposal: same dialog with `gtk::AlertDialog` (GTK 4.10+), buttons "Cancel" and "Log out" (destructive style). The toast text is the same.

## 3. Login return screen

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt:22-24` shows a full-screen `LoadingScreen` while `handleResult` runs. `TrackLoginActivity.kt:46-57` then calls `login(code)` (network) and returns to Settings.

Observed user experience: a loading spinner, then the Settings page with the row checked (success) or unchecked (failure, silent). There is no error message on failure (behaviour defect D-03).

Proposal: a spinner in the Settings row ("Logging in...") during the exchange. On failure, a toast with the error text (for example "HTTP error 400" or "MAL: Failed to refresh account token"). On denial (no `code`), a toast "Login was not completed" and the existing login stays (D-04).

## 4. Delete dialog (from the tracking info sheet)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt:789-840` (manga counterpart `MangaTrackInfoDialog.kt`). Observed:
- Title: `MR.strings.track_delete_title` = "Remove %s tracking?" with the tracker name, so "Remove MyAnimeList tracking?".
- Text: `MR.strings.track_delete_text` = "This will remove the tracking locally."
- Checkbox (shown only when `isDeletable()`, which is true for MAL because it implements `DeletableAnimeTracker` and `DeletableMangaTracker`): `MR.strings.track_delete_remote_text` = "Also remove from %s", so "Also remove from MyAnimeList". Default unchecked (`AnimeTrackInfoDialog.kt:789`, `removeRemoteTrack` starts false).
- Buttons: `TextButton` "Cancel" (`action_cancel`, pops the sheet) and `FilledTonalButton` "OK" (`action_ok`, error container colours). OK runs `unregisterTracking(serviceId)` (local row delete), then `deleteAnimeFromService()` if the checkbox is on (MAL DELETE), then pops.
- Failure of the remote DELETE is logged only; the local row is already gone.

Proposal: same text and order. The "OK" button is labelled "Remove" in the mockup spec (`spec/entries.json`, `remove_tracking`, buttons `["Cancel","OK"]`). Keep "OK" to match the source. Use "Also remove from MyAnimeList" as the checkbox label.

## 5. Status labels and picker

Labels (`p1-behavior.md` section 1.5):

Manga status picker (in this order, `MyAnimeList.kt:66`): Reading, Completed, On hold, Dropped, Plan to read, Rereading.
Anime status picker (`MyAnimeList.kt:70`): Watching, Completed, On hold, Dropped, Plan to watch, Rewatching.

Strings: "Reading" (`MR.strings.reading`), "Completed" (`MR.strings.completed`), "On hold" (`MR.strings.on_hold`), "Dropped" (`MR.strings.dropped`), "Plan to read" (`MR.strings.plan_to_read`), "Rereading" (`MR.strings.repeating`), "Watching" (`AYMR.strings.watching`), "Plan to watch" (`AYMR.strings.plan_to_watch`), "Rewatching" (`AYMR.strings.repeating_anime`).

A stored code that is not in the list has no label (`getStatusForManga`/`getStatusForAnime` return null). An anime row with code 1 (Reading) has no label. Observed.

## 6. Score picker and display

- Picker entries: "0" to "10" (11 entries, `MyAnimeList.kt:49-51`). The value shown in the list is `displayScore` (score truncated to integer, `MyAnimeList.kt:109-115`).
- A score of 0 is shown as "0", not as "-" or "unscored". Observed.
- Selecting an entry sets the score to its index (`indexToScore`). Selecting "10" stores 10.0.
- Search results carry `score = mean` from MAL (a decimal such as 8.27). The search card shows that value, not the integer. `displayScore` is for the local row only. Observed from the search fields (`MyAnimeListApi.kt:138`).

## 7. Search result card fields (MAL search and "my:" search)

Fields filled by `getMangaDetails` / `getAnimeDetails` (`p1-behavior.md` section 5.5) and used by the tracker search screen (`spec/entries.json`, `tracker_search`):
- Cover: `cover_url` (`main_picture.large`, empty string if absent).
- Title: `title`.
- Summary: `summary` (synopsis; may be empty).
- Type: `publishing_type` (media type, underscores replaced by spaces, for example "one shot", "light novel" for manga, "tv" and "movie" for anime).
- Status: `publishing_status` (for example "finished", "currently publishing", "currently airing").
- Start date: `start_date` (MAL date string, may be "" or partial).
- Count: `total_chapters` (manga) or `total_episodes` (anime). 0 means unknown.
- Score: `score` = MAL mean, -1.0 when absent.
- Authors and artists: not filled by MAL (empty lists).

Manga search hides any result whose `publishing_type` contains "novel" (`MyAnimeListApi.kt:95`). Anime search does not hide anything. Observed.

Mockup gap: `spec/entries.json` `tracker_search` shows "Author name" and "Score: 8.4" as fixed text and no progress count. The MAL card should show type, status, start date, count, and mean score. Authors are empty for MAL, so the author line is hidden for MAL.

The `id:` search returns exactly one result (the id is looked up directly). The `my:` search returns results with status 0, progress 0 and score = mean, because the details call does not include the user's list status (`p1-behavior.md` D-15). The UI must not show "Plan to read" for these rows. Show the card without a status, and offer "Refresh" to fetch the real status (proposal).

## 8. Track button and private track button (tracker search)

Mockup (`src/entry.rs`, `tracker_search()`): a "Track" button and a "Track privately" icon button.

For MAL: `supportsPrivateTracking` is false (`BaseTracker.kt:25`). MAL has no private flag in the code. The private button must be hidden for MAL, or disabled with the tooltip "MyAnimeList does not support private tracking" (proposal). The source does not show this button for MAL in the scope files; whether the tracking UI outside scope hides it is not verified here (see `p1-edge-cases-and-tests.md`, E-UI-1).

"Track" calls `bind` with the chosen hit, then pops the sheet. On failure the toast shows the error message (`register` in `MangaTracker.kt:59-63`).

## 9. Toasts and messages (MAL-related)

| Trigger | Text | Source |
|---|---|---|
| Login completed | "Logged in" (`login_success`) is shown by the generic login dialog only. MAL's browser login shows nothing (observed). | `SettingsTrackingScreen.kt` (generic login), `TrackLoginActivity.kt` |
| Logout confirmed | "You are now logged out" | `SettingsTrackingScreen.kt` logout dialog |
| Remote update failed | the error's `message`, for example "HTTP error 401", "MAL: Login has expired" | `MangaTracker.kt:117-119`, `AnimeTracker.kt:116-118` |
| Register (bind) failed | the error's `message` | `MangaTracker.kt:59-63`, `AnimeTracker.kt:60-64` |
| Remote delete failed | none (logged only) | `AnimeTrackInfoDialog.kt:863-866` |

Proposal: keep the same messages. Add the proposed messages from section 3 (login failure, denial) and the warning state in section 1.

## 10. Keyboard and mouse on desktop (Proposal)

Everything in this section is a proposal for the GTK build. The source is Android and has no keyboard model in scope.

- Settings row: Tab focus, Enter or Space activates. Pointer click activates.
- Logout dialog: Escape cancels, Enter confirms the focused button. Default focus on "Cancel" (destructive action should not be the default).
- Delete dialog: same. The checkbox is toggled with Space.
- Score and status pickers: arrow keys move, Enter selects. Typing a digit jumps to the matching score (proposal).
- Search field: Enter runs the search. Ctrl+F focuses it (proposal, matches the tracking screen).
- The loopback login has no keyboard path beyond the paste field. The paste field accepts Ctrl+V.

## 11. Responsive behaviour

The scope has no layout. The tracker search card is a row with a 67x96 cover (mockup `src/entry.rs`, `tracker_search()`, `cover(67, 96)`). The MAL card keeps the same size. The Settings row and dialogs follow the window width. No MAL-specific breakpoints.

## 12. Things the mockup lacks for MAL

1. Real login: the Settings row and the tracking sheet do not start a login.
2. Real logout dialog text for MAL. The mockup `remove_tracking` says "AniList".
3. Real status, progress, score and dates from MAL (the mockup shows fixed "Watching", "12 / 24", "8.0", "Start date", "Finish date").
4. A "logged in but auth-expired" warning state.
5. Real search results. The mockup builds four fake items with "Author name" and "Score: 8.4".
6. The card fields in section 7 (type, status, count, mean).
7. The "Track privately" button hidden or disabled for MAL.
8. Loading and empty states: spinner in the search while the request runs, "No results" text when the list is empty, and an error line with the error text when a search fails. The source does not define text for these in MAL code; the empty text is owned by area:27-tracking-core.
9. Toast for failed login and for the denied login (section 3).
10. The "Refresh" action for a `my:` result without status (section 7).
