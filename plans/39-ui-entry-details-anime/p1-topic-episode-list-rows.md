# Topic: episode list rows, gestures and download indicator (anime)

Deep dive on the row and list pieces of the anime entry page. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Screen layout and bottom-bar predicates are in p1-behavior.md sections 1 and 1.4; this file gives the geometry, gesture thresholds and state tables needed to build the GTK row.

## 1. Files

- `app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt` (520 LOC): the row, swipe mapping, airing row, summary, info line, bookmark and download icons.
- `app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt` (251 LOC): download state button.
- `app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt` (lines 1046-1151): row callbacks (`sharedEpisodeItems`, `onEpisodeItemClick`).
- Shared (outside scope, referenced): `app/src/main/java/eu/kanade/presentation/entries/components/DotSeparatorText.kt` (text " • "), `app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt` (thumbnail), `app/src/main/java/eu/kanade/presentation/components/ItemDownloadIndicator.kt` (indicator constants and `commonClickable`), `presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt` (`selectedBackground`, `secondaryItemAlpha`).

## 2. Constants

| Name | Value | Where |
|---|---|---|
| Row padding | start 16, top 12, end 8, bottom 12 dp | AnimeEpisodeListItem.kt:123 |
| Swipe threshold | 56 dp | :357 `swipeActionThreshold` |
| Swipe backdrop until threshold | `surfaceContainerLowest` | :113 |
| Swipe action background | `primaryContainer` | :95, :104 |
| Swipe icon padding | 16 dp, tint `contentColorFor(background)` | :345 |
| Simple layout spacing | 6 dp (0 dp when filler) | :230 |
| Title (rich, no thumbnail) | `bodyMedium`, `lineHeight 14.sp`, 1 line | :158, :155 |
| Title (rich, with thumbnail) | 2 lines | :155 |
| Title alpha seen | DISABLED_ALPHA 0.38 | :163 |
| Info alpha | seen: 0.38; unseen: 0.78 (SECONDARY_ALPHA) | :395, :419 |
| Thumbnail width | `min(screenWidthDp * 0.4, 250)` dp | :363 |
| Thumbnail gap | 8 dp end padding | :368 |
| Rich row spacing | 2 dp between thumbnail and text column | :144 |
| Summary font | 10 sp, line height 11 sp, labelMedium | :387-392 |
| Summary lines | 3 collapsed (min 3, max 3); unlimited expanded | :388-389 |
| Summary padding | bottom 4, start 4, end 4 dp | :397 |
| Watch progress colour | DISABLED_ALPHA | :448 |
| Filler icon | `AutoMirrored.Filled.Label`, tint tertiary at text alpha, end padding 4 dp | :421-433 |
| Bookmark icon | `Filled.Bookmark`, primary, padding start 4 dp, `secondaryItemAlpha` | :473-480 |
| Download indicator slot | padding start 4 dp | :485-486 |
| Selection background | secondary at 0.16 (dark) or 0.22 (light) | presentation-core Modifier.kt:27-37 |

## 3. Row anatomy and branching

`AnimeEpisodeListItem(title, date, watchProgress, scanlator, summary, previewUrl, seen, bookmark, fillermark, selected, isAnyEpisodeSelected, downloadIndicatorEnabled, downloadStateProvider, downloadProgressProvider, episodeSwipeStartAction, episodeSwipeEndAction, onLongClick, onClick, onDownloadClick, onEpisodeSwipe, modifier)`.

Outer: `SwipeableActionsBox(startActions = [start?], endActions = [end?], swipeThreshold = 56 dp, backgroundUntilSwipeThreshold = surfaceContainerLowest)` with `clipToBounds`. Inside: a `Row` with `selectedBackground(selected)`, `combinedClickable(onClick, onLongClick)` and the padding above.

Branch 1 (simple): `previewUrl.isNullOrBlank() && summary.isNullOrBlank()`:
```
Row
 ├─ Column(weight 1, spacing 6 or 0)
 │   ├─ Text(title, bodyMedium, maxLines 1, ellipsis, alpha seen?0.38:1)
 │   └─ EpisodeInformation(seen, date, watchProgress, fillermark, scanlator)
 └─ BookmarkDownloadIcons(...)
```

Branch 2 (rich):
```
Column
 ├─ Row(spacing 2)
 │   ├─ EpisodeThumbnail(previewUrl)              # only when previewUrl != null
 │   └─ Column
 │       ├─ Row(SpaceBetween, fillMaxWidth)
 │       │   ├─ Text(title, lineHeight 14, maxLines = minLines = (previewUrl==null ? 1 : 2), weight 1)
 │       │   └─ BookmarkDownloadIcons(...)        # only when previewUrl == null
 │       └─ EpisodeSummary(seen, isAnyEpisodeSelected, summary)
 └─ Row(SpaceBetween, fillMaxWidth)
     ├─ EpisodeInformation(...)
     └─ BookmarkDownloadIcons(...)                # only when previewUrl != null
```

Note the icons move: with a thumbnail the icons sit on the info line; without, they sit on the title line. Rust: same placement rule.

## 4. Info line (`EpisodeInformation`)

Order and separators (exact):
1. Filler (if `fillermark`): tag icon + "Filler" (AYMR `filler`), padding end 4 dp each.
2. Date (if not null): `Text(date, maxLines 1, ellipsis)`. Then dot separator if `watchProgress != null || scanlator != null`.
3. Watch progress (if not null): `Text(progress, maxLines 1, ellipsis, DISABLED_ALPHA)`. Then dot separator if `scanlator != null`.
4. Scanlator (if not null): `Text(scanlator, maxLines 1, ellipsis)`.

Dot separator: `" • "` (U+2022 with spaces).

Date: `relativeDateTimeText(episode.dateUpload)` (outside scope formatting). Rust: same relative formatter from the shared date helper (area 37 or 35).

Watch progress: shown when `!seen && lastSecondSeen > 0`; `"%1$s/%2$s"` of `formatTime(lastSecondSeen)` and `formatTime(totalSeconds)`. Both are milliseconds.

## 5. Summary (`EpisodeSummary`)

- `var expandSummary by remember { mutableStateOf(false) }`: not saveable (E-6). Rust: keep in the row model keyed by episode id.
- Shown only if `summary != null` (the caller already filters blank and `showSummaries`).
- `maxLines = if (expanded) Int.MAX_VALUE else 3`, `minLines = 3`.
- Modifier: padding 4/4/4 and bottom 4. Clickable toggle only when `!isAnyEpisodeSelected`.

## 6. Bookmark and download icons (`BookmarkDownloadIcons`)

Row, vertically centred:
1. If bookmarked: bookmark icon (filled), `contentDescription = MR.strings.action_filter_bookmarked` ("Bookmarked"), padding start 4 dp, tint primary, `secondaryItemAlpha`.
2. `EpisodeDownloadIndicator(enabled = downloadIndicatorEnabled, padding start 4 dp, state, progress, onClick = { onDownloadClick?.invoke(it) })`.

`downloadIndicatorEnabled = !isAnyEpisodeSelected && !anime.isLocal()` (AnimeScreen.kt:1108). When disabled the icon still shows; the gesture is off.

## 7. Download indicator (`EpisodeDownloadIndicator`)

Common geometry (from `app/src/main/java/eu/kanade/presentation/components/ItemDownloadIndicator.kt:36-46` and `EpisodeDownloadIndicator.kt`):
- Touch target: `IconButtonTokens.StateLayerSize` = 40 dp square.
- Ripple: unbounded, radius 20 dp (`StateLayerSize / 2`).
- `IndicatorSize` = 26 dp; `IndicatorPadding` = 2 dp; `IndicatorStrokeWidth` = 2 dp; `IndicatorModifier` = size 26 with padding 2.
- Arrow: `ArrowModifier` = size 19 dp (26 − 7).
- Long press haptic: dropped on Linux (P3).

State table (exact; the `when` in `EpisodeDownloadIndicator` dispatches on `AnimeDownload.State`):

| State | Composable | Icon / visual | Tap (`commonClickable.onClick`) | Long press (`onLongClick`) | Menu items |
|---|---|---|---|---|---|
| NOT_DOWNLOADED | `NotDownloadedIndicator` | `ic_download_item_24dp` (26 dp) tinted `onSurfaceVariant`, description `MR.strings.manga_download` ("Download") | `START` | `SHOW_QUALITIES` | none |
| QUEUE | `DownloadingIndicator` (`downloadState` QUEUE) | indeterminate ring + down arrow | opens menu | `CANCEL` | "Start downloading now" → `START_NOW`; "Cancel" (`MR.strings.action_cancel`) → `CANCEL` |
| DOWNLOADING, progress 0 | `DownloadingIndicator` | indeterminate ring + down arrow | opens menu | `CANCEL` | same |
| DOWNLOADING, progress > 0 | `DownloadingIndicator` | determinate ring; arrow colour `onSurfaceVariant` while animated progress < 0.5, else `background` | opens menu | `CANCEL` | same |
| DOWNLOADED | `DownloadedIndicator` | `CheckCircle` 26 dp, `onSurfaceVariant` | opens menu | opens menu | "Delete" (`MR.strings.action_delete`) → `DELETE` |
| ERROR | `ErrorIndicator` | `ErrorOutline` 26 dp, `error` colour, description `AYMR.strings.download_error` ("Error") | `START` | `START` | none |

Determinate ring: `CircularProgressIndicator(progress = { animatedProgress }, strokeWidth = 26/2 = 13 dp? ...)`. Exact code: `strokeWidth = IndicatorSize / 2` (13 dp), `gapSize = 0.dp`, `trackColor = Transparent`, `strokeCap = Butt`, `progress = downloadProgress / 100f` animated with `ProgressIndicatorDefaults.ProgressAnimationSpec`. Indeterminate ring: `strokeWidth = IndicatorStrokeWidth` (2 dp), same colours and cap.

Note the two stroke widths: the indeterminate ring is 2 dp thin; the determinate ring is 13 dp thick, which reads as a filled disc that grows with progress. Reproduce both (`EpisodeDownloadIndicator.kt:138-164`).

Menu anchor: the `DropdownMenu` is inside the indicator box (the indicator is the anchor).

Rust mapping: `download_click_action` in `src/anime_details/download_action.rs` (p1-rust-design.md section 3.3). The menu is a `gtk::PopoverMenu` anchored to the indicator button. The determinate ring is a `gtk::DrawingArea` with `cairo` arc: start at 12 o'clock, sweep `progress * 2π`, line width 13 px for determinate and 2 px for indeterminate; indeterminate uses a rotating 270° arc with a fixed period (Proposal: 1.4 s per revolution).

Progress animation: the Compose progress uses a spring tween (`ProgressAnimationSpec`). Rust: linear interpolation over 200 ms on each progress update (Proposal).

## 8. Swipe (`SwipeableActionsBox` plus `getSwipeAction`)

Start and end are both optional. Each action (`SwipeAction`) has an icon (16 dp padding around a 24 dp glyph; `Icon` with padding 16 dp), a background, `isUndo`, and an `onSwipe` callback.

Mapping table (`getSwipeAction`, AnimeEpisodeListItem.kt:258-297):

| Action enum | Icon when not set | Icon when set | isUndo | Callback argument |
|---|---|---|---|---|
| ToggleSeen | `Outlined.Done` (seen false) | `Outlined.RemoveDone` (seen true) | = seen | the enum value |
| ToggleBookmark | `Outlined.BookmarkAdd` | `Outlined.BookmarkRemove` | = bookmark | the enum value |
| ToggleFillermark | `Outlined.NewLabel` | `AutoMirrored.Outlined.LabelOff` | = fillermark | the enum value |
| Download | `Download` (NOT_DOWNLOADED or ERROR) | `FileDownloadOff` (QUEUE or DOWNLOADING), `Delete` (DOWNLOADED) | default false | the enum value |
| Disabled | no action | | | |

The callback is `onEpisodeSwipe(action)` (AnimeScreen passes the row's episode and the action to part 2).

Gesture (Rust proposal, matching the threshold):
- `gtk::GestureDrag` on the row. Horizontal drag only (vertical movement beyond 12 px cancels the swipe, so vertical list scrolling still works).
- Reveal the start action on drag right, the end action on drag left. Revealed width follows the drag.
- Release with offset ≥ 56 px fires the action; below 56 px snaps back.
- Context menu (right click) lists the same actions in the same order: start action then end action.

## 9. Missing-count row

`MissingItemCountListItem(count, modifier)`: full width, `labelMedium`, text `pluralStringResource(AYMR.plurals.missing_items, count, count)` ("Missing 1 item" / "Missing 2 items"). Keyed `missing-count-{id}` in the list. Span full width.

## 10. Airing row

`NextEpisodeAiringListItem(title, date)`:
- Padding start 16, top 12, end 8, bottom 12 dp.
- Title `bodyMedium.copy(lineHeight = 14.sp)`, 1 line, colour primary, `alpha(SECONDARY_ALPHA)`.
- Spacer 6 dp.
- Date `bodySmall`, 1 line, colour primary, `alpha(SECONDARY_ALPHA)`.
- Title text: `AYMR.strings.display_mode_episode` with `formatEpisodeNumber(airingEpisodeNumber)` ("Episode 7").
- Date text: `formatTime(airingTime, useDayFormat = true)` ("Airing in 01d 01h 01m 01s").

Timer (`AnimeScreen.kt:583-590` small, `917-923` large): per-second decrement. Rust: derive from absolute time (defect D-8).

## 11. Selection visuals and gestures

- Background: `selectedBackground(selected)` (rect fill, alpha by theme).
- Long press: `onEpisodeSelected(item, !selected, true, true)` and haptic (dropped).
- Click while selecting: `onEpisodeSelected(item, !selected, true, false)`.
- Click while not selecting and not selected: `onEpisodeClicked(episode, false)`.
- Summary click is disabled while selecting; download indicator is disabled while selecting; swipe stays active (E-5).

## 12. Scroll and list behaviour

- The episode list is one `LazyGrid` column (`GridItemSpan(maxLineSpan)`), so it is effectively a list.
- Rust: `gtk::ListView` + `gio::ListStore<EpisodeRowObject>` (virtualised). Set `single-click-activate` false; handle `GestureClick` per row.
- `gtk::ListView` re-binds row widgets on scroll; the summary expansion and the download menu state must live in the model (E-6, R-3).
- Each row's visible height changes when the summary expands; the list must re-measure. Use `gtk::ListView` with `show-separators` false and no fixed height.

## 13. States per row (summary table)

| Visible state | Title alpha | Info alpha | Bookmark icon | Filler label | Progress text | Download icon |
|---|---|---|---|---|---|---|
| Unseen, no progress | 1.0 | 0.78 | only if bookmarked | only if filler | no | per state |
| Unseen, started (`last_second_seen > 0`) | 1.0 | 0.78 | only if bookmarked | only if filler | yes | per state |
| Seen | 0.38 | 0.38 | only if bookmarked | only if filler | no (hidden when seen) | per state |
| Selected | as above | as above | as above | as above | as above | disabled |
| Downloading | as above | as above | as above | as above | as above | ring |
| Download error | as above | as above | as above | as above | as above | error icon |

## 14. Tests (links)

- T-DL1, T-DL2: download matrix.
- T-S1..T-S8: bottom menu and selection rules.
- T-W1: swipe while selected.
- T-L6: summary click ignored while selected.
- T-P1: 3000-row performance.
- T-A1..T-A4: airing row.
- T-N1..T-N4: time and number formats.
- T-R3: summary expansion survives recycling.
