# Area 32: MangaUpdates + Simkl trackers, part 1 - UI parity

This area renders UI in five places. The Compose sources are outside the scope list (they belong to area 27 and the settings area), but every element below is what the two services add or change. Sources, repo-relative to `/home/blackbox/aniyomi-src`:

- Settings > Tracking rows and dialogs: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt`
- Tracker search screen: `app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt` (anime twin: `app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt`)
- Login activity (deep link): `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`
- Manifest intent filter: `app/src/main/AndroidManifest.xml`

Mockup source (Kurobayomi, `/home/blackbox/kurobayomi/src/entry.rs`): `tracking()` at lines 496-537, `tracker_search()` at lines 539-570.

Labels: OBSERVED = Kotlin; PROPOSAL = Linux port.

Contents: 1 settings rows; 2 MangaUpdates login dialog; 3 Simkl login (browser + paste fallback); 4 logout dialog; 5 tracker search screen; 6 search result item; 7 status and score pickers (shared); 8 remove-tracking dialog; 9 states (loading, empty, error); 10 keyboard and mouse; 11 responsive behaviour; 12 gap list versus mockup.

---

## 1. Settings > Tracking rows

OBSERVED (`SettingsTrackingScreen.kt:52-72` group `MR.strings.services`):
- Group title `MR.strings.services`.
- Rows in this order: MyAnimeList, AniList, Kitsu, **MangaUpdates** (line 37-41), Shikimori, **Simkl** (lines 52-61), Bangumi, then an info row `Preference.PreferenceItem.InfoPreference(stringResource(MR.strings.tracking_info))` (line 72).
- MangaUpdates row: `TrackerPreference(tracker = trackerManager.mangaUpdates, login = { dialog = LoginDialog(trackerManager.mangaUpdates, MR.strings.username) }, logout = { dialog = LogoutDialog(trackerManager.mangaUpdates) })`.
- Simkl row: `login = { context.openInBrowser(SimklApi.authUrl(), forceDefaultBrowser = true) }`, `logout = { dialog = LogoutDialog(trackerManager.simkl) }`.
- The `TrackerPreference` row shows the tracker logo (colour from `getLogoColor()`), the name, and the logged-in state (login or logout action). The exact text of the logged-in summary is in the external `TrackerPreference` composable (not in scope); PROPOSAL: show the username only for SK (the Simkl user id) and "Logged in" for MU (the uid is not user-facing).

PROPOSAL (Linux): the row is an `adw`-free `GtkListBox` row (no libadwaita): title, subtitle (state), a suffix button labelled Login or Logout. Logo 24 px from the embedded SVG.

---

## 2. MangaUpdates login dialog

OBSERVED (`SettingsTrackingScreen.kt:250-347`, `TrackingLoginDialog`):
- Title row: text `MR.strings.login_title` with argument `tracker.name` (`MangaUpdates`), weight 1; trailing `IconButton` with `Icons.Outlined.Close`, content description `MR.strings.action_close`, dismisses.
- Field 1: `OutlinedTextField`, single line, label `MR.strings.username` (MU passes this key; the Kitsu row uses `MR.strings.email`). `keyboardOptions = ImeAction.Next`. Pre-filled with `tracker.getUsername()`. Error styling when `inputError && !processing`.
- Field 2: `OutlinedTextField`, single line, label `MR.strings.password`. Pre-filled with `tracker.getPassword()`. Trailing eye toggle `IconButton` (icons `Visibility` / `VisibilityOff`, content description null) that switches `PasswordVisualTransformation` on and off; starts hidden. `keyboardType = Password`, `imeAction = Done`. Same error styling.
- Buttons: a single full-width `Button` in the confirm slot. Enabled when `!processing && username.isNotBlank() && password.isNotBlank()`. Label `MR.strings.loading` while processing, else `MR.strings.login`.
- On tap: `processing = true`; `checkLogin`:
  - success: toast `MR.strings.login_success`; `inputError = false`; dialog dismissed.
  - failure: `tracker.logout()`; toast `e.message.toString()`; `inputError = true`; dialog stays open.
  - finally `processing = false`.

**OBSERVED surprise**: the password field is pre-filled with the stored session token (because `getPassword()` returns the MU session token, B-2.5). The masked field therefore contains the token, and the username field contains the numeric uid. A user who opens the dialog while logged in sees their uid and token (masked). This is a leak in the UI sense. PROPOSAL: never pre-fill MU fields. Pre-fill the username with the last typed login name (stored separately, a new preference `track_login_name_7`, default empty) and leave the password empty.

PROPOSAL details (Linux):
- Use a `GtkWindow` modal dialog (transient for the main window), 420 px wide, with a `GtkGrid` of two `GtkEntry` (username `input-purpose` free-form, password `GtkPasswordEntry` with the built-in show/hide toggle, which replaces the eye `IconButton`).
- Enter in username moves focus to password; Enter in password triggers Login (same as ImeAction.Done). Escape closes (same as dismiss).
- Login button sensitivity follows the same predicate. A `GtkSpinner` replaces the label text while processing and the entries are insensitive during processing (Kotlin does not disable fields; PROPOSAL improvement, documented).
- Error: the entries get the `error` CSS class (GTK 4 standard) when `inputError`. The toast text comes from the error text (section 9 of the rust design). Use `adw`-free `GtkRevealer` + `GtkLabel` with the `error` class under the entries for persistent error text, because toasts vanish.

---

## 3. Simkl login (browser and paste fallback)

OBSERVED: no dialog. The row opens the browser at the authorise URL (section 1). The app then waits for the activity to return. The Kotlin `TrackLoginActivity` is launched by the redirect; `returnToSettings()` then re-reads login state.

PROPOSAL (Linux, `src/track/ui/login_simkl.rs`):
1. Clicking Login on the Simkl row: a small dialog titled `Log in to Simkl` (new string; the Kotlin uses the same `login_title` key with `Simkl`, so the title is `Log in to Simkl`) with:
   - primary button "Open Simkl in browser" (opens the authorise URL via `gio::AppInfo::launch_default_for_uri`);
   - a `GtkEntry` labelled "Paste the redirect URL or code" (placeholder `aniyomi://simkl-auth?code=...`);
   - button "Log in" enabled when the entry is non-blank; parses per rust design section 6.4.
2. If the desktop handler delivers `aniyomi://simkl-auth?code=...` while the dialog is open, the dialog closes and login starts automatically (the paste field is not required).
3. Waiting state: after "Open Simkl in browser" the dialog shows a spinner with the text "Waiting for Simkl ..." and a Cancel button. Cancel closes the dialog without changing login state (the Kotlin has no waiting state; this is a Linux addition because there is no return path otherwise).
4. Result: success returns to the tracking page with the Simkl row updated (logged in). Failure: the Kotlin shows nothing (B-3.7 swallows the error). PROPOSAL: show the error text "Simkl login failed: <reason>" in the dialog, and keep the dialog open. This is a deliberate change (the Kotlin hides the error).

Open point: `forceDefaultBrowser = true` has no equivalent on Linux (no browser chooser in the Kotlin either). Document in the release notes.

---

## 4. Logout dialog (both services)

OBSERVED (`SettingsTrackingScreen.kt:366-407`, `TrackingLogoutDialog`):
- Title `MR.strings.logout_title` with argument `tracker.name`, centred, full width.
- Confirm row with two equal-weight buttons:
  - `OutlinedButton` text `MR.strings.action_cancel`: dismiss.
  - `Button` with error container colour (`containerColor = error`, `contentColor = onError`) text `MR.strings.logout`: calls `tracker.logout()`, dismisses, toasts `MR.strings.logout_success`.
- No text body and no state (cannot fail).

PROPOSAL (Linux): `GtkAlertDialog` (GTK 4.10+, available in 4.16) with buttons "Cancel" (default, Escape) and "Log out" (destructive style `destructive-action` class). Title `Log out of MangaUpdates?` built from `logout_title` with the name. Toast `logout_success` after action. Logout is local only for both services (the Kotlin does not revoke tokens remotely; B-3.12 and B-2.12 show no revoke endpoint). The dialog text must say so, because users may expect remote sign-out: PROPOSAL body text "This signs out of Kurobayomi only. Your list on <service> is not changed."

---

## 5. Tracker search screen (manga, MU)

OBSERVED (`MangaTrackerSearch.kt`):
- Top bar: `BasicTextField` search (single line, `ImeAction.Search`, placeholder `MR.strings.action_search_hint` when empty). Enter (`runOnEnterKeyPressed`) or IME action dispatches the query (`dispatchQueryAndClearFocus`). The field is auto-focused via `focusRequester` (initial query is the entry title, external).
- Bottom bar: primary `Button` `MR.strings.action_track` (elevated, weight 1) that confirms the selected result (`onConfirmSelection(false)`). If `supportsPrivateTracking` (false for MU and SK), a second button with `Icons.Filled.VisibilityOff` and `MR.strings.action_toggle_private_on`. For MU and SK this second button never appears.
- Body states (`queryResult`): `null` -> `LoadingScreen` (spinner, centred); `Result.Success(list)` with empty list -> `EmptyScreen(MR.strings.no_results_found)`; success with items -> `ScrollbarLazyColumn` with 12 dp vertical padding, 12 dp item spacing, keyed by `hashCode()`; failure -> `EmptyScreen(message = exception.message ?: MR.strings.unknown_error)`.
- Selecting an item: `onSelectedChange(it)`; the selected row is highlighted (`selected = it == selected`).
- Item actions (`SearchResultItemDropDownMenu`): `MR.strings.action_copy_to_clipboard` (copies the title), `MR.strings.action_open_in_browser` (opens `tracking_url` if not blank).

PROPOSAL (Linux, `src/track/ui/search.rs`, replaces mockup `tracker_search()`):
- Header: `GtkSearchEntry` (placeholder "Search MangaUpdates" / "Search Simkl", title from the service), debounce 300 ms (PROPOSAL; the Kotlin search is on demand, Enter dispatches; keep Enter as the immediate trigger and add the debounce as optional).
- States: spinner (`GtkSpinner`) while the request runs; "No results found" (`GtkStatusPage` icon `system-search-symbolic`); error page with the error text and a "Try again" button; list.
- List: `GtkListView` (virtual) with rows: 48x72 cover (async image, placeholder icon `image-missing`), title (bold, max 2 lines, ellipsised), a meta line (type, start year, status), and a summary line (first 2 lines of `summary`).
- Bottom action bar: "Track" suggested-action button (enabled when a row is selected). No private button (MU and SK do not support private tracking).
- Row actions: context menu (right-click, or the Menu key, or a `gio::SimpleActionGroup` with `copy-title` and `open-in-browser`). Open uses `source_url` for SK (rust design 5.7) and `tracking_url` for MU.

Visible text rules (OBSERVED, must be preserved in the port):
- Type line shown only when `publishing_type` is not blank: label `MR.strings.track_type`.
- Start line only when `start_date` not blank: label `MR.strings.label_started`.
- Status line: `MR.strings.track_status` (only for status text not blank).
- Score line: `MR.strings.score`.
- Authors and artists line: shown only when either list is non-empty; `(authors + artists).distinct().joinToString()`. MU and SK fill neither, so this line never appears for them.

---

## 6. Search result item (detail of section 5)

OBSERVED (`MangaTrackerSearch.kt:280-350` approximately; `SearchResultItem` composable):
- Title `Text`, `maxLines = 2`, `TextOverflow.Ellipsis`, `titleMedium`, end padding 28 dp (room for the menu button).
- Dropdown button at the row end (`SearchResultItemDropDownMenu`).
- Detail rows as `SearchResultItemDetails(title, text)` for type, started, status, score.

PROPOSAL: match the same rows; the cover is an extra column that the Kotlin shows on the left (cover_url); the port must not block the list on cover downloads (placeholder then image).

Summary text for MU includes HTML-decoded description (`summary`); for SK summary is `All titles:\n<titles>` (B-3.14). The UI shows the summary text as plain text, not Markdown. PROPOSAL: show the first 2 lines only in the list row; the full summary appears in the item details only if the port has such a view (not in the Kotlin search screen; not required).

---

## 7. Status and score pickers (shared, used after bind)

OBSERVED: the status and score pickers come from `getStatusListManga()` / `getStatusListAnime()` and `getScoreList()`. Their labels come from `getStatusForManga` / `getStatusForAnime`.
- MU status list order: Reading, Complete, On hold, Unfinished, Wish (B-2.1). 
- SK status list order: Watching, Completed, On hold, Not interesting, Plan to watch (B-3.1).
- MU score picker: 92 entries (`-`, `1.0` .. `10.0`) (B-2.2).
- SK score picker: `0` .. `10` (B-3.2).

PROPOSAL (Linux): status = `GtkDropDown` with the labels in the list order; score = `GtkDropDown` for MU (92 entries, scrolls) and for SK a `GtkSpinButton` 0-10 is NOT used (the Kotlin uses a list; keep the list). Display of score in the row: MU `displayScore` returns `score.toString()` in the Kotlin, the port shows `-` for zero (fix D-MU-8, documented). SK shows integer.

---

## 8. Remove tracking dialog (mockup `remove_tracking`)

OBSERVED: the Kotlin shows a remove-tracking action in the tracking sheet (external); for MU the remote delete (`DeletableMangaTracker.delete`) is an option of that flow. SK has no delete (B-3.16).

PROPOSAL (Linux, `src/track/ui/remove.rs`):
- MU entries: a check button "Also remove from my MangaUpdates list" (default unchecked). When checked, the remote delete (M7) is sent; when unchecked, only the local tracker row is removed. The Kotlin text for this is the external remove dialog; the port must show the same two choices.
- SK entries: no checkbox; the dialog text states "This only removes the link in Kurobayomi. Your Simkl list is not changed."
- Confirm button destructive style. Cancel default.

---

## 9. States: loading, empty, error (per screen)

| Screen | Loading | Empty | Error |
|---|---|---|---|
| Settings row | none | n/a | none (login errors show in dialog) |
| MU login dialog | button label "Loading" and disabled | n/a | toast + inline error, dialog open |
| SK browser login | "Waiting for Simkl" (PROPOSAL) | n/a | PROPOSAL: error text in dialog |
| Search | `LoadingScreen` (spinner) | `no_results_found` | `exception.message` or `unknown_error` |
| Bind (after Track) | PROPOSAL: spinner on the Track button, disabled | n/a | toast with the `TrackError` text (rust design section 9) |
| Refresh | PROPOSAL: spinner on the tracking sheet | n/a | toast |
| Logged-out state (row) | n/a | "Log in" action | n/a |
| Auth expired (HTTP 401/403) | n/a | n/a | PROPOSAL: row subtitle "Session expired" with a Login action |

Strings used (keys in Kotlin; English text in the port must match): `no_results_found`, `unknown_error`, `login_success`, `logout_success`, `loading`, `action_track`, `action_search_hint`, `action_copy_to_clipboard`, `action_open_in_browser`, `track_type`, `label_started`, `track_status`, `score`, `login_title`, `logout_title`, `username`, `password`, `action_close`, `action_cancel`, `login`, `logout`, `services`, `tracking_info`, `action_toggle_private_on` (not shown for MU/SK).

---

## 10. Keyboard and mouse equivalents (desktop)

- Login dialog: Tab order username, password, Login; Enter in password submits; Escape closes.
- Logout dialog: Escape cancels; Enter activates the default (Cancel). Destructive button needs a click or Tab+Space.
- Search: `Ctrl+F` focuses the search entry (mockup-wide convention, area 01); Enter searches; Up/Down move selection in the list; Enter on a selected row triggers Track (PROPOSAL); Ctrl+C copies the title; Right-click or `Shift+F10` opens the row menu.
- Deep link: `aniyomi://simkl-auth` is opened by the desktop; no keyboard action.
- Drag and drop: none in the Kotlin; none in the port.
- Touch: the Linux build is desktop-first; touch works through GTK's default gestures (list scroll), no custom gestures.

---

## 11. Responsive behaviour

- Login dialog: fixed 420 px; on widths under 460 px the dialog takes the window width minus 24 px margin on each side (no horizontal page scroll).
- Search: list rows are fluid; the cover column keeps 48 px width; title ellipsised at 2 lines.
- Settings rows: stack the subtitle under the title below 520 px width (GTK box orientation switch via a size-based breakpoint, GTK 4.16 `GtkBreakpoint` or a window-width property watcher).

---

## 12. Gap list: what the mockup lacks (for this area)

Mockup facts (from `src/entry.rs`): `tracking()` (lines 496-537) builds a static sheet with a remove dialog (`remove_tracking`, line 514) and an "Add tracking" button that opens `tracker_search` (lines 530-531). `tracker_search()` (lines 539-570) is a static search page with a "Track" suggested button (line 563) and a "Track privately" icon button (line 564).

Missing in the mockup for area 32:
1. No Settings > Tracking page rows for MangaUpdates and Simkl (login state, logo, logout action).
2. No MangaUpdates login dialog (username, password, error, pre-fill rules).
3. No Simkl browser login flow, no waiting state, no paste-code fallback.
4. No logout dialog (both services) with the local-only text.
5. `tracker_search()` has no live query, no loading, empty, or error states, no result list rows, no context menu.
6. "Track privately" button is shown in the mockup for both services; the Kotlin hides it because `supportsPrivateTracking` is false for both. Remove it for these two services.
7. The remove dialog has no MU checkbox ("Also remove from my MangaUpdates list") and no SK text.
8. No status or score picker wired to the service lists (MU 92 scores, SK 0-10).
9. No auth-expired row state.
10. No warning when the secret store is unavailable (rust design section 7).
11. No "Waiting for Simkl" state.
12. The mockup's tracking sheet shows no per-service score display rule (MU "-" for zero, SK integer).
