# 39-ui-entry-details-anime (part 2): UI parity

This part renders UI only through its host and its dialogs. The screen body (header, rows, grid, related rows, info box, FAB, selection bar) is composed in part 1 (`presentation/entries/anime/*`). This document lists every visible element **that the scope files drive** (host, dialogs, sheets, snackbars, toasts, navigation, state visibility), states what the Android app does, gives the desktop equivalent, and says what the current Kurobayomi mockup lacks.

Sources read for the mockup: `/home/blackbox/kurobayomi/src/entry.rs` (entry details 81-240, tracking 498-537, tracker search 539-570), `src/ui.rs` (`show_sheet` 391-404, `show_dialog` 406-430, `resolve` 437-467, `toolbar` 469-487), `src/screens.rs` (route 39, `open_entry` 62), `spec/entries.json` (screens `anime_entry_details`, `tracking_sheet`, `tracker_search`, `browse`, read as parsed JSON), `spec/SCHEMA.md`.

Repo-relative Android sources: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt` (host), `.../AnimeScreenModel.kt` (state), `.../track/AnimeTrackInfoDialog.kt` (tracking screens), `app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt` (part 1 body, read only for callback types).

---

## 1. Page states

| State | Android | Kurobayomi target |
|---|---|---|
| Sources not loaded | `LoadingScreen()` (AnimeScreen.kt:91-94) | centered `gtk::Spinner` with label "Loading" |
| Entry loading | `State.Loading` -> `LoadingScreen()` (106-109) | same spinner |
| Success, not refreshing | `isRefreshingData == false` | full page |
| Success, refreshing (first open or manual) | `isRefreshingData == true` (pull-to-refresh indicator, part 1) | page visible; top `gtk::ProgressBar` (indeterminate, 2 px, below header) and the refresh toolbar button shows a spinner |
| Related loading | `isLoadingRelatedAnime` | spinner in the related section header only |
| Empty episodes (episode type) | empty-state text in the part 1 body; the mockup spec for `anime_entry_details` sets `empty` to "No episodes found" (same wording as `AYMR no_episodes_error`) | label "No episodes found" with a "Refresh" button |
| Empty seasons | "No seasons found" | label "No seasons found" with a "Refresh" button |
| Error on refresh | snackbar (6.3 of behavior) and page stays | same, via the snackbar layer (section 4) |
| Entry deleted or not found | not handled in scope (crash-prone, `getAnimeAndEpisodesAndSeasons.awaitAnime` throws) | Proposal: page-level message "This entry no longer exists" with a back button |

---

## 2. Visible controls driven by the host (AnimeScreen.kt callbacks)

Each row: control, visibility rule (exact from the host), action, desktop equivalent.

| Control | Visible when (AnimeScreen.kt) | Action | Desktop control |
|---|---|---|---|
| Add to library button | always (`onAddToLibraryClicked`, 148-151) | `toggleFavorite()` then haptic (dropped) | toggle button in action row, label "Add to library" / "In library" |
| Download (dropdown) | `!successState.source.isLocalOrStub() && fetchType == Episodes` (191-193 and 145-147) | `runDownloadAction(DownloadAction)` for NEXT_1/5/10/25 and UNVIEWED | `gtk::MenuButton` with the five items (labels: "Next episode", "Next 5 episodes", "Next 10 episodes", "Next 25 episodes", "Unseen"; plural forms follow the mockup spec text) |
| Filter (opens episode or season settings) | always (`onFilterButtonClicked = showSettingsDialog`, 174) | `showSettingsDialog()` picks `EpisodeSettingsSheet` or `SeasonSettingsSheet` by `fetchType` (1572-1579) | toolbar icon `view-filter-symbolic`, tooltip "Filter" |
| Refresh | always (`onRefresh = fetchAllFromSource`, 175) | manual refresh | overflow menu item "Refresh" and toolbar `view-refresh-symbolic` |
| Tracking | `onTrackingClicked` only when `fetchType == Episodes` (166-172) | no logged-in trackers: open Settings > Tracking; else open sheet | action row button, label shows tracker count (`trackingCount`) as "Track" or "N trackers" (AnimeInfoHeader.kt:221-228) |
| WebView | only for `AnimeHttpSource` (152-158) | open WebView; long-press copies URL (159-165) | action row button "WebView" |
| Share | only for `AnimeHttpSource` (184-190) | share (Dropped to clipboard, section 5) | overflow item "Share" (copies URL) |
| Edit categories | `anime.favorite` (194) | `showChangeCategoryDialog()` | overflow item "Edit categories" |
| Fetch interval button | `anime.favorite` (195-197) | `showSetAnimeFetchIntervalDialog()` | action row button, label is the next update in days (part 1) |
| Migrate | `anime.favorite` (198-200) | push migrate search screen | overflow item "Migrate" |
| Change intro length | `anime.favorite && fetchType == Episodes` (201-202) | `showAnimeSkipIntroDialog()` | overflow item "Change intro length" |
| Settings | always (overflow, part 1 menu) | source preferences | overflow item "Settings" |
| Cover tap | always (`onCoverClicked`, 183) | `showImagesDialog()` | click on cover |
| Multi bookmark / fillermark / mark seen / mark previous / delete / select all / invert | selection bar (part 1) | 203-211 | selection toolbar buttons |
| Episode tap | always (`onEpisodeClicked`, 136-144) | open player (XOR rule, behavior 17.4) | row activation |
| Episode long-press | selection (part 1) | `toggleSelection(item, true, userSelected = true, fromLongPress = true)` | row long-press or Shift+click for range, Ctrl+click for toggle |
| Episode swipe start/end | per-anime action from preferences (`episodeSwipeStartAction/EndAction`) | `episodeSwipe(item, action)` | no swipe on desktop: right-click context menu with the same actions; keyboard `B` bookmark, `F` fillermark, `S` toggle seen, `D` download |
| Season card tap | `fetchType == Seasons` | push `AnimeScreen(id)` | double-click or Enter |
| Season continue overlay button | `showContinueOverlay` | open next unseen of the season | play button on the card |
| Related row tap | related present | open related anime (`fromSource = !favorite`) | click |
| Related row long-press | related present | global search by title | right-click |
| FAB | episodes present (part 1) | open next unseen or first | toolbar button "Start"/"Resume" (no FAB on desktop; the button is in the header) |

---

## 3. Dialogs and sheets (Dialog variants, AnimeScreen.kt 239-410)

For each variant: title, body, buttons, dismiss rule, and the desktop window type.

| Variant | Title (source) | Body | Buttons (source) | Dismiss | Desktop window |
|---|---|---|---|---|---|
| `ChangeCategory` | categories title (ChangeCategoryDialog, part 1) | checkbox list of user categories, "Edit categories" link | Confirm, Dismiss, Edit | `onDismissRequest` (5.3 of behavior) | modal `gtk::Window` 420 px, checkbox rows (`gtk::CheckButton`) |
| `DeleteEpisodes` | "Are you sure?" text from `DeleteItemsDialog(isManga=false)` | "Are you sure you want to delete the selected episodes?" (`AYMR confirm_delete_episodes`, part 1) | OK, Cancel | cancel | `gtk::AlertDialog` (detail text, buttons Cancel and OK, OK is destructive) |
| `DuplicateAnime` | title from `DuplicateAnimeDialog` (part 1) | "You have an entry in your library with the same name. Do you still wish to continue?" (mockup spec text) | Show anime, Migrate existing entry, Add anyway | dismiss | `gtk::AlertDialog` with three buttons (Cancel is the cancel index) |
| `Migrate` | "Select data to include" (mockup spec) | switches and info rows | Show anime, Show seasons, Copy, Migrate | dismiss | modal window (migration area owns the body) |
| `SetAnimeFetchInterval` | title and subtitle from `SetIntervalDialog` (ItemsDialogs.kt:70-170, part 1 presentation; the mockup spec title "Smart update" is a placeholder) | expected next update text (subtitle), custom list 0..28 (index 0 = `label_default`) | Cancel, OK | dismiss | modal window 420 px with `gtk::DropDown` (29 items) |
| `ShowQualities` | episode title (number form "Episode %1$s" or name) | "Choose video quality:" and hosters (part 1) | none (rows are actions) | dismiss | modal window (part 1 body) |
| `ChangeAnimeSkipIntro` | "Skip intro length" | `gtk::SpinButton` 0..255 seconds, initial value rule (E-AIR-2) | Cancel, OK | dismiss; OK also dismisses | modal window with spin button |
| `EpisodeSettingsSheet` | "Episode settings" (sheet, 3 tabs) | Filter (4 tristate), Sort (4 keys), Display (name or number; thumbnails; summaries), "Set as default" (behavior 9.1) | none (switches apply at once) | close button | modal sheet 520x620 with `gtk::Stack` + `gtk::StackSwitcher` (existing `show_sheet`) |
| `SeasonSettingsSheet` | "Season settings" (sheet) | Filter (6 tristate), Sort (7 keys), Display (grid mode chips, items per row slider), Overlay (5 switches), Grid display mode, "Set as default" | none | close | modal sheet, same as above |
| `TrackSheet` | "Tracking" | bound rows and unbound rows (section 4 of track topic) | none | close; swipe dismissal dropped | modal sheet 520x620 |
| `FullImages` | "Cover" (mockup) | image pager, edit/delete (part 1 `AnimeImagesDialog`) | Edit, Delete (mockup spec) | close | modal window (part 1 body; `AnimeImageScreenModel` owned by part 1) |

Track child screens (AnimeTrackInfoDialog.kt) are full-screen pushes inside the sheet on Android and are shown as nested pages of the same modal window on desktop (the `Vec<TrackPage>` stack of the design):

| Child | Title (source) | Content | Buttons | Result |
|---|---|---|---|---|
| Status | list of statuses (labels from tracker) | radio list | Cancel, OK (`TrackStatusSelector`) | `setStatus()` then pop |
| Episodes | "Episodes" wheel (part 1 `TrackItemSelector`) | number range 0..total or 0..10000 | Cancel, OK | `setEpisode()` then pop |
| Score | score list (tracker) | wheel of strings | Cancel, OK | `setScore()` then pop |
| Start / Finish date | `track_started_reading_date` "Start date" / `track_finished_reading_date` "Finish date" | calendar with validity (topic track 6) | Remove (when set), Cancel, OK | `setDate` then pop |
| Remove date | `track_remove_date_conf_title` "Remove date?" | text with service name | Cancel, Remove (destructive) | `removeDate` then popUntil home |
| Tracker search | "Track search" (mockup spec) | search field prefilled with title, results list | Track, Track privately (if supported) | bind then pop |
| Remove tracking | `track_delete_title` "Remove %s tracking?" | "This will remove the tracking locally." and a checkbox "Also remove from %s" when deletable | Cancel, OK (destructive) | local delete, optional remote delete, then pop |

Scope note: the exact text of these labels comes from the string resources listed in `p2-behavior.md` section 19, not from the mockup spec. The mockup's spec strings differ in places (for example "Unseen" versus the Android "Unseen episodes" overlay label); Kurobayomi uses the Android strings.

---

## 4. Snackbars and toasts

| Message (key) | English text | Kind | Action | Duration | Trigger | Desktop |
|---|---|---|---|---|---|---|
| `AYMR no_episodes_error` | No episodes found | snackbar | none | default (Short) | refresh | overlay revealer, 4 s |
| `AYMR no_seasons_error` | No seasons found | snackbar | none | default | refresh | same |
| generic | exception message | snackbar | none | default | refresh error | same |
| `AYMR delete_downloads_for_anime` | Delete downloaded episodes? | snackbar, dismiss button | `MR action_delete` "Delete" | default | remove from library with downloads | same, action button |
| `AYMR snack_add_to_anime_library` | Add anime to library? | snackbar, dismiss button | `MR action_add` "Add" | default | first download of non-library entry | same |
| `AYMR confirm_tracker_update_anime` (%d) | Update trackers to episode %d? | snackbar, dismiss button | `MR action_ok` "OK" | Short | ASK after mark seen | same |
| `AYMR trackers_updated_summary_anime` (%d) | Trackers updated to episode %d | toast | none | Short | ALWAYS after mark seen | toast, 2 s |
| `AYMR episode_settings_updated` | Updated default episode settings | snackbar | none | default | set as default | same |
| `AYMR season_settings_updated` | Updated default season settings | snackbar | none | default | set as default | same |
| `MR track_error` (%1$s, %2$s) | "%1$s error: %2$s" | toast | none | Short | refresh of a tracker failed | toast |
| `MR error_no_match` | No match found | toast | none | Short | enhanced match failed | toast |
| share failure | exception message | toast | none | Short | share failure | toast |
| copy URL failure (new, fix 23.15) | "Could not copy the link" (Kurobayomi text) | toast | none | Short | copy failure | toast |
| copied link (new, desktop share replacement) | "Link copied" (Kurobayomi text) | toast | none | Short | share or copy URL | toast |

Behavior of the snack layer (desktop):
- One message at a time (E-UI-3).
- Dismiss button only on the three prompts that call `withDismissAction = true` (delete downloads, add to library, tracker update confirm). The error snack and the two "settings updated" snacks have neither an action nor a dismiss button (AnimeScreenModel.kt:363, 1133-1135, 1363-1365).
- Action button returns the action to the model: `Delete` runs `deleteDownloads()`, `Add` runs `toggleFavorite()`, `OK` runs `trackEpisode`.
- Duration Short = 4 s (Compose Short; no Long durations occur in this scope).

---

## 5. Gestures and inputs

| Android gesture | Where | Desktop equivalent |
|---|---|---|
| Tap row | episode row | single click plays (if not in selection mode); in selection mode, click toggles selection |
| Long press row | episode row | Shift+click selects range from anchor (E-SEL-2); Ctrl+click toggles one |
| Swipe row start/end | episode row | none; right-click context menu shows the two configured actions plus the rest |
| Long press cover | not in scope | none |
| Long press related card | related row | right-click opens global search |
| Long press WebView button | action row | right-click copies URL |
| Long press tracker logo | tracking row (track topic) | right-click menu: Open in browser, Copy link |
| Swipe to dismiss sheet | `NavigatorAdaptiveSheet` (AnimeScreen.kt:324) | dropped (task 068); Escape closes |
| Haptic on library add | AnimeScreen.kt:150 | dropped (task 067) |
| Pull to refresh | part 1 (`onRefresh`) | toolbar refresh button; F5 refreshes the page |

Keyboard shortcuts proposed for the desktop (not in the Android app; tasks 051 and 063):
- Enter: play the selected or first unseen episode.
- Space: toggle seen on the focused episode.
- B: bookmark; F: fillermark; S: seen/unseen; D: download (start or cancel per state).
- Ctrl+A: select all (full list, E-SEL-6); Escape: clear selection, then close the page.
- Ctrl+F: focus the filter button; Ctrl+R: refresh.

---

## 6. Responsive behavior

- Android tablet layout (`isTabletUi()`) is not applicable; Kurobayomi uses one layout with a maximum content width of 760 px (matches the mockup `clamp(&col, 760)`).
- Minimum content width 280 px (E-UI-6). Below that the action row wraps to two lines.
- Season grid columns: `gtk::GridView::set_min_columns(items_per_row)` with `items_per_row` from the setting (0 = automatic; automatic uses `(width / 150).clamp(2, 12)`, which mirrors the mockup `cover_grid` FlowBox min 2 and max 12 children per line).
- Episode rows use the rich layout (thumbnails) only when previews or summaries are on and the source has them (part 1 rule).
- Dialogs: 420 px wide for confirmations and pickers, 520 px for sheets, height up to 620 px, scroll inside.
- Scale: respect the system font scale; no fixed-height rows except the 48 px tracker logo.

---

## 7. Motion and feedback

| Android | Kurobayomi |
|---|---|
| Selection toolbar slide-in (part 1 AnimatedVisibility) | `gtk::Revealer` `SlideUp`, 200 ms |
| Snackbar slide-in and fade-out | `gtk::Revealer` `SlideUp`, 150 ms in, 150 ms out |
| Dialog open | GTK window default (no animation) |
| Progress for downloads | `gtk::ProgressBar` inside the row, updated from download events (30 Hz max) |
| Loading placeholder | `gtk::Spinner` |

No infinite animations. Reduce-motion: when `gtk-enable-animations` is false, all revealers use `TransitionType::None`.

---

## 8. Mockup gap analysis (current `src/entry.rs` and `src/ui.rs`)

| Area | Mockup today | Needed (this scope) | Gap |
|---|---|---|---|
| Entry data | hard-coded: title from argument, "Source A (EN)", "Ongoing", 24 episodes, dates "{n % 12 + 1}/0{n % 9 + 1}/2026" | model-driven from `AnimeEntryModel` | everything |
| Episode list | `for n in (1..=24).rev()` with fixed read/unread (`n < 20`) and fixed opacity | `EpisodeItem` list, filters, sort, separators, selection | full rewrite |
| Missing-episode rows | none | `ListRow::MissingCount` | missing |
| Download state per row | a dropdown with Download/Delete (no state) | state-driven icon and progress (`DownloadState`) | missing |
| Selection | a `Revealer` toolbar with buttons that do nothing; the "Clear selection" button hides the bar | `Selection` model; bulk actions wired | nothing wired |
| Mark seen / unseen | buttons with no action | `mark_seen`, `mark_previous_seen` | missing |
| Filter sheet | from `spec/entries.json` `episode_settings` (static) | typed sheet bound to flags | spec not bound to state |
| Sort | spec picker, not bound | `set_sort` (flip rule) | missing |
| Download menu | static menu with "Next episode" etc. | `runDownloadAction` | missing |
| Fetch interval | button text "2 days", spec dialog with static list | `set_fetch_interval` with real expected date | missing |
| Library button | toggles its own label | `toggle_favorite` with category and duplicate flow | missing |
| Category dialog | spec dialog "Set categories" with static "Default" | real categories | missing |
| Duplicate dialog | spec dialog | `Dialog::DuplicateAnime` | wiring missing |
| Tracking button | opens static `tracking_sheet` (spec) with fixed "AniList" row and "8.0" score | `TrackSheet` with real rows | full rewrite |
| Tracker search | static four results, static "Track" button | `TrackPage::Search` with real search | full rewrite |
| Tracker remove | spec dialog "Remove AniList tracking?" | typed window with checkbox when deletable | wiring missing |
| Skip intro | spec dialog with 256 entries | `SpinButton` 0..255 | simplify |
| Seasons | `seasons()` grid of 6 fixed covers in migrate context only | season list and grid from model | missing for anime details |
| Airing | "Next episode airs in 2d 4h" text | live countdown from airing data | missing |
| Related | none | related groups | missing |
| Toolbar | `Download`, `Filter` | plus Refresh, overflow items as in section 2 | partial |
| Snackbar | none (dialogs only) | snack layer (section 4) | missing |
| Share / WebView | WebView static page; no share | copy-link and `gtk::UriLauncher` | partial |
| Keyboard | none | section 5 shortcuts | missing |
| Loading state | none | spinner (section 1) | missing |
| Error state | none | snackbars and empty labels | missing |
| Empty state | none (always 24 rows) | "No episodes found" / "No seasons found" | missing |
| Spec-driven dialogs | `ui::show_dialog`, `show_sheet` from `spec/entries.json` | typed dialogs for anime (`dialogs.rs`), spec kept for settings | migration (task 065) |
| `resolve()` routing | maps toolbar labels to sheets/dialogs by normalized label | replaced by typed actions for anime | migration |

Items the mockup gets right and this part keeps: the two-level page structure with a back button, the modal window style (`ui::modal`), the 760 px clamp, the `Stack`/`StackSwitcher` sheet tabs, the FlowBox-based cover grid as a starting point for seasons.

---

## 9. Strings in this part

Every English string shown by the scope files is in `p2-behavior.md` section 19 (keys and English values verified in the moko-resources files). Strings invented for Kurobayomi (not in Android) are marked in section 4 as "(new)". No other user-visible text is introduced by this part.

Translation: the Kurobayomi build uses the English base strings. Locale files from Aniyomi are not copied in this part.

---

## 10. Accessibility

- Every icon button has a tooltip equal to the Android `contentDescription` or the visible label (for example "Filter", "Refresh", "Clear selection").
- Episode rows expose the title, seen state, download state and bookmark state as the accessible label.
- Focus order: header buttons, action row, description, related, episode list, selection toolbar, FAB-equivalent button.
- Color is never the only indicator: downloaded, bookmarked and seen have icons and labels.
- Dialog text uses `gtk::Label` with `wrap` and `max_width_chars` 50 (existing mockup behavior).
