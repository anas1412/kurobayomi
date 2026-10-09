# Area 39 part 1: UI parity for the anime entry details screen

This file lists every visible element, state, dialog, snackbar, menu and gesture in scope, the Linux equivalent, and what the current mockup lacks. Source paths are repo-relative to `/home/blackbox/aniyomi-src`. Mockup paths are relative to `/home/blackbox/kurobayomi`.

Colour and type tokens: the Kotlin uses Material 3 roles (`primary`, `onSurface`, `surfaceContainerHighest`, `tertiary`, `error`, `secondary`). Linux: use the user's GTK theme colours (`@accent_color`, `@window_bg_color`, `@card_bg_color`, `@error_color`, `@success_color`). Do not hard-code palette values. Opacities from `tachiyomi.presentation.core.components.material.Constants.kt`: `DISABLED_ALPHA = .38`, `SECONDARY_ALPHA = .78`.

Spacing tokens (`presentation-core/.../material/Constants.kt`): `extraLarge 32`, `large 24`, `medium 16`, `mediumSmall 12`, `small 8`, `extraSmall 4` (dp). Linux: the same numbers as CSS px at scale 1.

## 1. Screen states

| State | Kotlin | Visible | Linux |
|---|---|---|---|
| Loading | `AnimeScreenModel.State.Loading` (part 2) | the page is not built until `Success` | spinner in the page body (`gtk::Spinner`, centred) |
| Success | `State.Success` | full page | full page |
| Refreshing | `state.isRefreshingData` | pull indicator at top (`PullRefresh`) | header refresh button shows a spinner; the list stays visible |
| Selecting | `episodes.any { selected }` | action-mode toolbar with count; bottom action bar; FAB hidden; pull-to-refresh disabled | same; action bar revealed; FAB hidden |
| Empty episodes | `episodes` empty | `ItemHeader` shows "0 episodes"-style count (strings in the shared header component); no rows | same header; empty-state label "No episodes" is NOT a key in the Kotlin, so the Rust port shows only the header (no invented string) |
| Empty seasons | `seasons` empty | same as empty episodes | same |
| Stub source | `state.source is StubAnimeSource` | red warning icon in the status row | warning icon (`dialog-warning-symbolic`, colour `@error_color`) after the status text |
| Local anime | `anime.isLocal()` | episode download indicator is shown but disabled (`enabled = !isAnySelected && !isLocal`) | same (insensitive button) |
| Airing | `state.airingTime > 0` and `showNextEpisodeAirTime` and status != COMPLETED | airing row above the episode list | airing row above the list |
| Error (quality dialog) | `resultList.isFailure` | toast "No available videos", dialog dismissed | same |

## 2. Header and toolbar (title bar)

The toolbar is `EntryToolbar` (presentation component, outside this scope). Its items as wired from this scope:

| Item | Shown when | Callback from this scope | Linux |
|---|---|---|---|
| Title | always; alpha fades in on scroll (small layout) | `state.anime.title` | header title; on the small layout fade via `gtk::Label` opacity bound to scroll position |
| Back | `navigateUp` | `navigateUp` | header back button (`ui::page(..., back = true)`) |
| Filter | `onFilterClicked` | opens the settings sheet for the fetch type | header button with `view-filter-symbolic` (mockup already uses `icon_button("view-filter-symbolic", "Filter")`) |
| Active filter marker | `hasFilters` | toolbar uses it to show a filtered state | a dot badge on the filter button when `hasFilters` (CSS class `kb-has-filter`) |
| Share | `onShareClicked` | share the entry | menu item |
| Download menu | `onDownloadActionClicked` | `DownloadAction` values `NEXT_1_ITEM`, `NEXT_5_ITEMS`, `NEXT_10_ITEMS`, `NEXT_25_ITEMS`, `UNVIEWED_ITEMS` (`app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt`). `AnimeScreenModel.kt:837-841` maps them to the next 1, 5, 10, 25 unseen episodes or all unseen. Menu labels come from the toolbar component (outside this scope). | menu |
| Edit categories | `onEditCategoryClicked` | opens category dialog | menu item |
| Refresh | `onRefresh` | refresh the entry | menu item and F5 |
| Migrate | `onMigrateClicked` | migrate dialog | menu item |
| Settings | `onSettingsClicked` only when `source is ConfigurableAnimeSource` | opens `AnimeSourcePreferencesScreen(source.id)` | menu item "Settings" shown only for configurable sources |
| Change intro length | `changeAnimeSkipIntro` | dialog | menu item |
| Action-mode counter | `actionModeCounter = selectedCount` | cancel, select all, invert | action-mode header: "{n} selected" with cancel, select all, invert buttons |

The Kotlin passes `isManga = false` to the toolbar, so the manga-only items are absent.

Mockup today: `src/entry.rs` builds the anime menu with `menu_button` entries from `spec/entries.json` (`anime_entry_details.menu`: "Change intro length", "Refresh", "Edit categories", "Migrate", "Share", "Settings"). These are wired to `dialog_of`/`open`, not to real actions. The "Settings" item is not restricted to configurable sources.

## 3. Info box

| Element | Kotlin | Linux | Notes |
|---|---|---|---|
| Backdrop | cover-background image, `useBackground(true)`, alpha 0.2, blur 4 dp, gradient to background | `gtk::Picture` in a `gtk::Overlay` under the header, CSS opacity 0.2, gradient overlay | section 6.1 of p1-behavior.md |
| Cover | `ItemCover.Book`, `sizeIn(maxWidth = 100.dp)` small; `fillMaxWidth(0.65f)` large | `gtk::Picture` 100 px wide (small) or 65% of the column (large), aspect 2:3, rounded corners via CSS | tap opens images dialog (section 9.4) |
| Title | `titleLarge`; "Unknown title" when blank | `title-1` style label, wraps | tap: global search; long press: copy |
| Author | `titleSmall` with `PersonOutline` icon; "Unknown author" | label with `avatar-default-symbolic` | tap: search (global); long press: copy |
| Artist | `titleSmall` with `Brush` icon, only when different from author | label with `applications-graphics-symbolic` | tap: search; long press: copy |
| Status | icon + label (behaviour section 6.3 table) | label with symbolic icon | not clickable |
| Source name | clickable, search this source only | link-styled label | tap: search (this source) |
| Stub warning | `Warning` icon, error colour | `dialog-warning-symbolic` | |

Text alignment: centred in large layout, start-aligned in small.

## 4. Action row

Four `TextButton`s of equal width (weight 1). Each: 20 px icon, 4 px gap, 12 px label, centred.

| Button | Labels | Active colour | Click | Long press |
|---|---|---|---|---|
| Library | "In library" / "Add to library" | `primary` when in library | add or remove (part 2) | category dialog (only when `onEditCategory` set) |
| Interval | "N/A", "Soon", "1 day", "N days" | `primary` in user-interval mode | interval dialog | none |
| Tracking | "Tracking", "1 tracker", "N trackers" | `primary` when count > 0 | tracking sheet (only when callback set) | none |
| WebView | "WebView" | inactive | open in webview | long press: webview option (callback set only when available) |

Inactive colour: `onSurface` at 0.38 opacity. Linux: `alpha(@window_fg_color, 0.38)`.

Mockup today (`src/entry.rs` lines for `lib`, `next`, `track`, `web`): buttons exist, but the label toggles by string comparison with no state; the interval label is hard-coded "2 days"; there is no tracking count, no `N/A` / `Soon` logic, and no active colour.

## 5. Description, tags and related

- Description: 3 lines collapsed, all lines expanded, "More" / "Less" toggle (`manga_info_expand` / `manga_info_collapse`). Empty description shows "No description". Default expanded: `isFromSource` (small), always true (large).
- The Linux toggle is a flat button under the text (no whole-block click, see p1-behavior.md 19.2). The text is `selectable`.
- Tags: chips (`SuggestionChip`-like). Tap opens a popover with "Search" and "Copy to clipboard". Collapsed row is horizontal scroll; expanded wraps (`gtk::FlowBox`).
- Related section: section 18 of p1-behavior.md. Linux: small groups in a 3-slot grid with group title and centred items; multi groups in horizontal scrollers. Long press on a related item: `onRelatedLongClick`.

Mockup today: a static 2-line summary string (`Summary of the series goes here...`) with a "More" button that toggles `set_lines`. Genres are hard-coded `["Action", "Adventure", "Comedy", "Fantasy"]` and their menu is a no-op. Related rows are absent.

## 6. Item header

Shown above the episode or season list. Contents: count of items, missing count, and a filter button.

- Count: `pluralStringResource(AYMR.plurals.anime_num_episodes, itemCount)` ("%1$s episode" / "%1$s episodes") for `FetchType.Episodes`, or `AYMR.plurals.anime_num_seasons` ("%d season" / "%d seasons") for `FetchType.Seasons` (`app/src/main/java/eu/kanade/presentation/entries/components/ItemHeader.kt`).
- Missing count: `missingItemsCount = max(missingEpisodes, missingSeasons)` (behaviour section 8). Text: `AYMR.plurals.missing_items` ("Missing %1$s item" / "Missing %1$s items"), shown only when > 0 (`app/src/main/java/eu/kanade/presentation/entries/components/MissingItemCountListItem.kt`).
- Click (whole header or filter icon): opens the settings sheet (`onFilterClicked`).
- Disabled while selecting (`enabled = !isAnySelected`).

Mockup today: header text is `"24 episodes"` / `"24 chapters"` from `heading(&format!("24 {}s", unit))` (hard-coded count).

## 7. Episode list

### 7.1 Row anatomy (simple layout)

```
[ title (1 line, dimmed when seen)                         [bookmark] [download] ]
[ Filler  date  •  12:34/24:00  •  scanlator                                      ]
```

Padding: start 16, top 12, end 8, bottom 12. Vertical spacing 6 between title and info (0 when filler).

Rich layout (previews or summary present): thumbnail on the left (width `min(40% of window width, 250)`, aspect 16:9 image), title (2 lines when thumbnail, 1 otherwise), summary (3 lines collapsed, 10 px font, line-height 11 px, click to expand), info line at the bottom.

### 7.2 Row states

| State | Visual |
|---|---|
| Seen | title and info at 0.38 opacity |
| Unseen | normal |
| Bookmarked | `bookmark` icon (filled, `primary`) before the download icon |
| Filler | "Filler" label with a tertiary-coloured tag icon |
| Selected | background `alpha(@accent_bg_color, 0.22)` (0.16 dark) |
| Watch progress | "m:ss/m:ss" text, dimmed; hidden when seen or when `lastSecondSeen <= 0` |
| Scanlator | plain text after a dot separator |
| Download | 40 px state layer; icon 26 px; see section 7.4 |

### 7.3 Interaction

| Gesture | Not selecting | Selecting |
|---|---|---|
| Click | open episode (`onEpisodeClicked(ep, false)`) | toggle selection |
| Long press | enter selection, select this row, haptic (dropped) | toggle this row |
| Swipe (start or end) | action from preferences (behaviour 3.6) | same action (OBSERVED: the Kotlin does not gate swipe on selection) |
| Summary click | expand or collapse | disabled |
| Download indicator click | action from section 7.4 | disabled (insensitive) |

Linux: the swipe stays available in selection mode, matching the source.

### 7.4 Download indicator

| State | Icon | Primary click | Long press | Menu |
|---|---|---|---|---|
| Not downloaded | download icon, `manga_download` ("Download") | Start | Show qualities | none |
| Queued | indeterminate ring with down arrow | opens menu | Cancel | "Start downloading now", "Cancel" |
| Downloading, progress 0 | indeterminate ring | opens menu | Cancel | same |
| Downloading, progress > 0 | ring filled to progress, arrow colour flips to background at >= 50% | opens menu | Cancel | same |
| Downloaded | filled check circle | opens menu | opens menu | "Delete" |
| Error | error icon in `error` colour, `download_error` ("Error") | Start (retry) | Start | none |

Ring size 26 px, stroke 2 px, state layer 40 px, arrow 19 px (`IndicatorSize - 7.dp`).

Linux: `gtk::DrawingArea` with Cairo arc for the ring; the animation of progress uses `ProgressIndicatorDefaults.ProgressAnimationSpec` (Compose default spring), replaced by a 200 ms linear CSS-free interpolation in Rust (Proposal).

### 7.5 Missing-count row

Full-width separator showing the number of missing episodes between existing ones. Text: `AYMR.plurals.missing_items` ("Missing 3 items" / "Missing 1 item"), `labelMedium` style. Linux: a centred dim label with the same text. The row is `EpisodeRow::Missing`.

### 7.6 Airing row

Full-width row above the list: "Episode n" title in primary, then "Airing in %02dd %02dh %02dm %02ds" in primary, both at 0.78 opacity. Updates every second.

## 8. Season grid

| Mode | Kotlin | Linux |
|---|---|---|
| Compact grid | `EntryCompactGridItem` with title | cover card with title under it, 2 lines |
| Comfortable grid | `EntryComfortableGridItem` with title | cover card with title, 3 lines in related context |
| Cover-only | `EntryCompactGridItem` without title | cover card only |
| List | `EntryListItem` full-width rows, row height from `entries` and container height | horizontal row: cover left, title and badges right |

Badges (all modes): downloads count, unseen count, language (`LanguageBadge`). Continue-watching overlay button when `showContinueOverlay` and the callback exists (the button is the Kotlin `onClickContinueViewing`).

Column rule: `gridSize == 0` gives adaptive columns (128 px min); `gridSize > 0` gives that many columns. The mockup's `cover_grid(n, w, on)` helper in `src/entry.rs` uses `FlowBox` with min 2 and max 12 children per line, which does not match either rule.

Cover card size: 130 x 190 px in the mockup's `cover()` helper; Kotlin cover aspect is the component's; use 2:3 at the grid column width.

## 9. Dialogs and sheets

### 9.1 Episode settings (sheet, three tabs)

| Tab | Controls (in order) |
|---|---|
| Filter | 4 tri-state rows: Downloaded (disabled when Downloaded-only mode), Unseen, Bookmarked, Fillermarked |
| Sort | 4 radio-like rows: By source, By episode number, By upload date, Alphabetically; the active row shows a direction arrow |
| Display | 2 radios (Source title, Episode number); 2 checkboxes (Episode thumbnails, Episode summaries) |

Overflow menu (top right of the sheet): "Set as default". Tabs: "Filter", "Sort", "Display". Sheet padding vertical 8, horizontal 24.

Linux: a `gtk::Window` 420 px wide (modal) with `gtk::Notebook` (tabs at top). Tri-state rows: a `gtk::CheckButton` cycling Off, On (include) and Exclude. Proposal: Exclude is shown with the GTK inconsistent state (`set_inconsistent(true)`), which matches the indeterminate look of the Kotlin `TriStateItem` checkbox. No text suffix is added, because the Kotlin shows none.

### 9.2 Season settings (sheet, three tabs)

Filter: 6 tri-states (Downloaded, Unseen, Started, Completed, Bookmarked, Fillermarked).
Sort: 7 rows (By source, By season number, By upload date, Alphabetically, Unseen count, Last seen, Episode fetch date).
Display: chip row "Display mode" (4 chips), slider 0..10 ("List size" in list mode, else "Items per row", value "Auto" at 0), heading "Overlay" with 5 checkboxes, heading "Grid display mode" with 2 radios.

Linux: `gtk::Scale` 0..10 with `draw-value` and a value label; chips as `gtk::ToggleButton` group with CSS `pill`.

### 9.3 Set as default (confirm)

Title "Episode settings" or "Season settings". Body "Are you sure you want to save these settings as default?". Checkbox "Also apply to all anime in my library" (default unchecked). Buttons: Cancel, OK. Linux: `gtk::Window` modal with a `gtk::CheckButton` and two buttons; Enter = OK, Escape = Cancel.

### 9.4 Images dialog

Full-screen two-page viewer with bottom bar:
- Left pill: Close (`action_close`); arrow to switch page (label: "Edit background" on cover page, "Edit cover" on background page).
- Right pill: Share (`action_share`), Save (`action_save`), Edit (label "Edit cover" / "Edit background"; dropdown with "Edit" and "Delete" when a custom image exists on the current page).
- Tap on the image area or Escape closes (`onDismissRequest`).

Icons: close `window-close-symbolic`, arrows `go-previous-symbolic` / `go-next-symbolic`, share `document-send-symbolic`, save `document-save-symbolic`, edit `document-edit-symbolic`. (Kotlin uses `KeyboardArrowRight`/`Left`, `Share`, `Save`, `Edit`; Adwaita equivalents are the nearest stock icons.)

### 9.5 Duplicate-add choice

Title "Are you sure?" (headline). Body: "You have an entry in your library with the same name. Do you still wish to continue?" (the Kotlin text has a blank line; keep the paragraph break). Three rows: "Show anime" (icon `Book`), "Migrate existing entry" (icon `SwapVert`), "Add anyway" (icon `Add`). Cancel button at the bottom, `primary` text, 16 px, `titleLarge`. Linux: modal `gtk::Window` (360 px wide) with a `gtk::ListBox` of three rows (each row activatable, icon + title) and a Cancel button. Each row closes the window and then acts.

### 9.6 Episode options (quality) dialog

Opened by long press on a not-downloaded episode. Layout (`EpisodeOptionsDialog`):
- Title: the episode title (bold, one line, ellipsis).
- Subtitle: "Choose video quality:" (italic).
- Current video row: `currentVideo.videoTitle` with a dropdown arrow (`NavigateNext`). Click: shows all qualities.
- Action rows (in order): "Copy" (copy icon), "Use internal downloader", "Use external downloader", "Play externally", "Play internally". Each closes the dialog after running (except copy, which the Kotlin does not close: `onCopyClicked` is not followed by `closeMenu()`. OBSERVED.)
- All qualities view: hoster list with expandable hosters (`QualitySheetHosterContent`) or a flat video list when there is one `NO_HOSTER_LIST` hoster (`QualitySheetVideoContent`). Max height 600 dp. Each video row shows state (ready, loading, error).
- Slide transition: content slides in from the right by half width (`slideInHorizontally(initialOffsetX = { it / 2 })`).

Linux: `gtk::Window` 420 x auto, `gtk::Stack` with `slide-left-right` transition between "current" and "all" pages. Rows: `gtk::ListBox` with activatable rows.

Loading: until the current video is known, a centred spinner (`LoadingScreen()`).

### 9.7 Video list rows (inside 9.6)

- Hoster row: name, expand arrow; states Idle/Loading/Error (`QualitySheetHosterContent` shared sheet components).
- Video row: `videoTitle`; state icon per `Video.State`: Queue and LoadVideo show a spinner; Ready shows the check/selected marker when selected; Error shows an error icon.
- Selected video: marked (shared component).

### 9.8 Dialog dismissal rules

- All dialogs close on Escape, on click outside (sheet), and on their own button.
- Quality dialog closes after any action button except Copy (9.6).
- Images dialog: no confirm for delete (the Kotlin shows the dropdown with Delete directly; no confirmation). Proposal: add a confirmation for Delete on Linux because deletion is destructive; document as a deviation.

## 10. Snackbars and toasts (exact messages)

Snackbar (AnimeImageScreenModel, all with dismiss action):
- Save cover success: "Cover saved". Save background success: "Background saved".
- Save cover failure: "Error saving cover". Save background failure: "Error saving background".
- Share failure: "Error sharing cover" / "Error sharing background". There is no share success message.
- Edit or delete success: "Cover updated" / "Background updated".
- Edit or delete failure: "Failed to update cover" / "Failed to update background".

Toast (`context.toast`):
- Quality dialog failure: "No available videos".
- Copy video link: "Copied video quality link to clipboard".

Observed quirk: `saveImage` shows the success snackbar even when no image was saved, because `saveImageInternal` returns `null` instead of throwing when the bitmap is missing (defect D-2 in p1-edge-cases-and-tests.md).

## 11. Gestures and keyboard / mouse equivalents (desktop)

| Action | Touch (Kotlin) | Linux mouse | Linux keyboard |
|---|---|---|---|
| Open episode | tap | single click (on row) | Enter on focused row |
| Start selection | long press | long press, or Ctrl+click | Space toggles focused row |
| Toggle one | tap (in selection) | click (in selection) or Ctrl+click | Space |
| Range select | (none) | Shift+click (desktop addition) | Shift+arrow |
| Select all | toolbar "select all" | Ctrl+A | Ctrl+A |
| Invert | toolbar "invert" | Ctrl+I (desktop addition) | Ctrl+I |
| Clear selection / back | Back | Escape (when a selection exists) | Escape |
| Back (no selection) | Back | Escape or Alt+Left / back button | Escape |
| Resume or start | FAB | button or Ctrl+Return | Ctrl+Return |
| Filter | toolbar | filter button or Ctrl+F | Ctrl+F |
| Refresh | pull | refresh button | F5 |
| Delete selected | bottom menu | Delete key (when bottom action "Delete" is available) | Delete |
| Bookmark selected | bottom menu | Ctrl+B | Ctrl+B |
| Mark seen selected | bottom menu | M | M |
| Mark unseen selected | bottom menu | Shift+M | Shift+M |
| Previous as seen | bottom menu (single selection) | Ctrl+P | Ctrl+P |
| Download selected | bottom menu | Ctrl+D | Ctrl+D |
| Row context menu | long press | right click | Menu key |
| Summary expand | tap summary | click the More button | Enter on the button |
| Swipe actions | swipe | drag with the mouse across the row, or context menu | context menu |

Ctrl+Return is used for Resume; Mark seen uses plain `M` to avoid clashing with GTK defaults. These are proposals.

Right click on a row opens the context menu with the same bottom-action set as the selection bar, applied to the clicked row (not the selection).

## 12. Responsive behaviour

- Width >= 900 px: two panes (`gtk::Paned`): info and actions on the left (position 420 px), episode or season list on the right. Selection bar spans the right pane.
- Width < 900 px: single column scroll. Info row cover 100 px wide; list full width.
- Phone width is not a target (desktop app), but the single-column layout must work down to 360 px wide with no horizontal page scroll (the 14 px gutter stays).
- Grid columns: `width / 128` for adaptive; fixed when set.

## 13. Message keys and English text used by this scope

Every key below is copied from the source resource files. Keys marked (literal) are not resource keys; they are literals in Kotlin code and must be kept as Rust constants.

### 13.1 MR keys (`i18n/src/commonMain/moko-resources/base/strings.xml`)

| Key | English text |
|---|---|
| `action_filter` | Filter |
| `action_sort` | Sort |
| `action_display` | Display |
| `set_chapter_settings_as_default` | Set as default |
| `label_downloaded` | Downloaded |
| `action_filter_bookmarked` | Bookmarked |
| `action_sort_alpha` | Alphabetically |
| `sort_by_source` | By source |
| `sort_by_upload_date` | By upload date |
| `show_title` | Source title |
| `action_display_grid` | Compact grid |
| `action_display_comfortable_grid` | Comfortable grid |
| `action_display_cover_only_grid` | Cover-only grid |
| `action_display_list` | List |
| `action_display_mode` | Display mode |
| `label_started` | Started |
| `completed` | Completed |
| `overlay_header` | Overlay |
| `label_auto` | Auto |
| `pref_library_columns` | Items per row |
| `confirm_set_chapter_settings` | Are you sure you want to save these settings as default? |
| `action_cancel` | Cancel |
| `action_ok` | OK |
| `action_resume` | Resume |
| `action_start` | Start |
| `copy` | Copy |
| `action_start_downloading_now` | Start downloading now |
| `action_delete` | Delete |
| `action_add_anyway` | Add anyway |
| `action_migrate_duplicate` | Migrate existing entry |
| `are_you_sure` | Are you sure? |
| `confirm_add_duplicate_manga` | You have an entry in your library with the same name.\n\nDo you still wish to continue? |
| `in_library` | In library |
| `add_to_library` | Add to library |
| `not_applicable` | N/A |
| `manga_interval_expected_update_soon` | Soon |
| `manga_tracking_tab` | Tracking |
| `action_web_view` (translatable=false) | WebView |
| `description_placeholder` | No description |
| `action_search` | Search |
| `action_copy_to_clipboard` | Copy to clipboard |
| `unknown_title` | Unknown title |
| `manga_cover` | Cover |
| `unknown_author` | Unknown author |
| `ongoing` | Ongoing |
| `licensed` | Licensed |
| `publishing_finished` | Publishing finished |
| `cancelled` | Cancelled |
| `on_hiatus` | On hiatus |
| `unknown` | Unknown |
| `manga_info_collapse` | Less |
| `manga_info_expand` | More |
| `manga_download` | Download |
| `action_close` | Close |
| `action_share` | Share |
| `action_save` | Save |
| `action_edit_cover` | Edit cover |
| `action_edit` | Edit |
| `cover_saved` | Cover saved |
| `error_saving_cover` | Error saving cover |
| `error_sharing_cover` | Error sharing cover |
| `notification_cover_update_failed` | Failed to update cover |
| `cover_updated` | Cover updated |
| `action_display_local_badge` | Local source |
| `action_display_language_badge` | Language |
| plural `day` | one: 1 day; other: %d days |
| plural `num_trackers` | one: %d tracker; other: %d trackers |

### 13.2 AYMR keys (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`)

| Key | English text |
|---|---|
| `display_mode_episode` | Episode %1$s |
| `display_mode_season` | Season %1$s |
| `choose_video_quality` | Choose video quality: |
| `copied_video_link_to_clipboard` | Copied video quality link to clipboard |
| `action_start_download_internally` | Use internal downloader |
| `action_start_download_externally` | Use external downloader |
| `action_play_externally` | Play externally |
| `action_play_internally` | Play internally |
| `episode_progress` | %1$s/%2$s |
| `filler` | Filler |
| `download_error` | Error |
| `sort_by_episode_number` | By episode number |
| `show_episode_number` | Episode number |
| `show_episode_previews` | Episode thumbnails |
| `show_episode_summaries` | Episode summaries |
| `episode_settings` | Episode settings |
| `season_settings` | Season settings |
| `also_set_episode_settings_for_library` | Also apply to all anime in my library |
| `action_filter_unseen` | Unseen |
| `action_filter_fillermarked` | Fillermarked |
| `sort_by_season_number` | By season number |
| `action_sort_unseen_count` | Unseen count |
| `action_sort_last_seen` | Last seen |
| `action_sort_episode_fetch_date` | Episode fetch date |
| `action_display_download_badge_anime` | Downloaded episodes |
| `action_display_unseen_badge` | Unseen episodes |
| `action_display_show_continue_watching_button` | Continue watching button |
| `action_display_grid_mode` | Grid display mode |
| `show_season_number` | Season number |
| `pref_library_rows` | List size |
| `action_edit_background` | Edit background |
| `background_saved` | Background saved |
| `error_saving_background` | Error saving background |
| `error_sharing_background` | Error sharing background |
| `background_updated` | Background updated |
| `notification_background_update_failed` | Failed to update background |
| `upcoming` | Upcoming |
| `action_show_anime` | Show anime |

### 13.3 Literal strings (not resources)

| Where | Text | Rust constant |
|---|---|---|
| `AnimeScreen.kt:1156` | `Airing in %02dd %02dh %02dm %02ds` | `AIRING_FMT` |
| `EpisodeOptionsDialog.kt:438` | `No available videos` | `TOAST_NO_VIDEOS` |
| `EpisodeOptionsDialogScreen.kt:437` (log) | `Error getting links` | log only |
| `EpisodeOptionsDialogScreen.kt:289` (exception message) | `No available videos` | error text |
| `AnimeScreen.kt:1156` / `formatTime` | (numeric format only) | none |

## 14. What the mockup lacks (gap list)

Mockup files: `src/entry.rs` (`entry()`), `src/screens.rs` (`info_screen`, `open_media`), `spec/entries.json` (`anime_entry_details`).

1. Data: all episode rows, dates, titles and read states are generated from `1..=24` (`src/entry.rs` loop `for n in (1..=24).rev()`). No `Episode` model, no DB.
2. Info: the status line is "Studio / Ongoing / Source A (EN)" with fixed strings. No author or artist distinction, no "Unknown" placeholders, no status icon mapping, no stub warning, no copy or search gestures.
3. Action row: fixed labels; no `N/A` / `Soon` / day-count logic; no tracking count; no WebView long press.
4. Description: fixed sentence, no empty placeholder, no `isFromSource` default, no `SelectionContainer` text.
5. Genres: four fixed chips; the menu does nothing.
6. Airing: fixed text "Next episode airs in 2d 4h"; no countdown, no preference gate.
7. Item header: "24 episodes" fixed; no missing count.
8. Episode rows: no bookmark, no filler, no watch progress, no summaries or thumbnails, no swipe, no download states, no missing-count separators, no selected background.
9. Selection: a revealer toolbar with buttons, but no selection state, no predicates, no counter, no select all or invert.
10. FAB: fixed "Resume" label, always visible, no visibility rule, no expand-on-scroll.
11. Season layout: absent in the anime branch (the only season UI is `seasons()` for migration).
12. Filter and sort: opens the `episode_settings` sheet from the spec, but no filter or sort is applied, and the sheet is spec-driven text.
13. Related rows: absent.
14. Images dialog: `full_images` dialog from the spec (static); no pager, no zoom, no save or share, no edit.
15. Episode quality dialog: `episode_options` from the spec (static); no hosters or videos.
16. Duplicate dialog: `duplicate_anime` from the spec (static); no actions.
17. Large layout: absent (only one column).
18. Pull to refresh and refresh shortcut: absent.
19. Toolbar fade on scroll: absent.
20. Keyboard and right-click: absent.
21. Toasts and snackbars: absent.
22. Set as default: spec dialog only; no write.

## 15. Accessibility notes

- Every icon-only button has a tooltip and accessible label from the message key in section 13 (`gtk::Widget::set_tooltip_text`, `update_property(gtk::accessible::Property::Label)`).
- Dimmed (seen) rows keep full contrast for the title in high-contrast themes: use CSS class `kb-seen` with opacity only on the secondary text (Proposal; the Kotlin dims the title too).
- Focus order follows the visual order: info, actions, description, tags, related, header, list.
