# Area 39 part 1: behaviour specification (anime entry details)

Sources are repo-relative to `/home/blackbox/aniyomi-src`. `OBSERVED` = what the Kotlin does. `Proposal:` = what the Rust port should do. Dimensions are Compose `dp` unless stated; on GTK they map to CSS px at scale 1.

## 1. Screen layout and routing

### 1.1 Entry point

OBSERVED. `app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt:110-273` `fun AnimeScreen(state: AnimeScreenModel.State.Success, ...)`.
- `val onSettingsClicked: (() -> Unit)? = { navigator.push(AnimeSourcePreferencesScreen(state.source.id)) }.takeIf { state.source is ConfigurableAnimeSource }` (lines 178-180). The "Settings" overflow item exists only for configurable sources.
- `isTabletUi` selects the implementation: `false` gives `AnimeScreenSmallImpl`, `true` gives `AnimeScreenLargeImpl`. The value is supplied by the caller.
- `onSearch: (query: String, global: Boolean) -> Unit` is passed to the info box.
- `onEpisodeClicked: (episode: Episode, alt: Boolean) -> Unit`. Both bottom-menu external and internal actions call it with `alt = true` (lines 1012-1017). The `alt` meaning is decided by the caller (part 2). Proposal: keep the two-argument shape. The Rust side passes `alt = true` for the menu actions and `alt = false` for a plain click (line 1149).

Proposal: Linux has no phone and tablet split as such. Use the large layout when the window is at least 900 px wide (GTK `gtk::Window` default-width or `adw`-free breakpoint via `gtk::Window::default_width` checks), otherwise the small layout. The min-width breakpoint is a configuration constant `ANIME_DETAILS_TWO_PANEL_MIN_WIDTH_PX = 900` (not from the source; chosen for desktop).

### 1.2 Small (single-column) layout

OBSERVED. `AnimeScreenSmallImpl` (lines 277-627).
- `val gridSize = remember(state.anime) { state.anime.seasonDisplayGridSize }` (line 340).
- `seasons = state.processedSeasons`, `episodes = state.processedEpisodes`, `listItem = state.episodeListItems` (lines 344-346).
- `hasFilters` (lines 347-352): `when (state.anime.fetchType) { FetchType.Seasons -> seasonsFiltered(); FetchType.Episodes -> episodesFiltered() }`. Passed to the toolbar, which uses it to show an active-filter state.
- `isAnySelected = episodes.fastAny { it.selected }` in `derivedStateOf` (lines 356-360).
- `BackHandler`: if `isAnySelected` then `onAllEpisodeSelected(false)` else `navigateUp()` (lines 362-368).
- Container height: `containerHeightPx = maxHeight.roundToPx()` from `BoxWithConstraints` (lines 370-372). Passed to season items as `containerHeight = containerHeightPx - toolbarHeight` (line 569).
- Scaffold top bar: `EntryToolbar` with title `state.anime.title`, `hasFilters`, and callbacks. `actionModeCounter = selectedEpisodeCount` (count of `episodes` with `selected`), `onCancelActionMode = { onAllEpisodeSelected(false) }`, `onSelectAll = { onAllEpisodeSelected(true) }`, `onInvertSelection = { onInvertSelection() }`, `isManga = false` (lines 392-412).
- Top bar title and background fade (lines 378-391):
  - `isFirstItemVisible = itemListState.firstVisibleItemIndex == 0`
  - `isFirstItemScrolled = itemListState.firstVisibleItemScrollOffset > 0`
  - `titleAlpha` animates to `1f` when `!isFirstItemVisible`, else `0f` (`animateFloatAsState`, label "Top Bar Title").
  - `backgroundAlpha` animates to `1f` when `!isFirstItemVisible || isFirstItemScrolled`, else `0f` (label "Top Bar Background").
- Bottom bar: `SharedAnimeBottomActionMenu(selected = episodes.filter { it.selected }, fillFraction = 1f, ...)` (lines 414-430).
- Snackbar host: `SnackbarHost(hostState = snackbarHostState)` (line 431).
- FAB rules: section 9.
- Body: `PullRefresh(refreshing = state.isRefreshingData, onRefresh = onRefresh, enabled = !isAnySelected, indicatorPadding = PaddingValues(top = topPadding))` (lines 466-471).
- Grid: `FastScrollLazyVerticalGrid`, `columns = if (gridSize == 0) GridCells.Adaptive(128.dp) else GridCells.Fixed(gridSize)`, `contentPadding.start = GRID_PADDING + contentPadding.calculateStartPadding(dir)`, `end` likewise, `bottom = contentPadding.calculateBottomPadding()` (lines 473-481). Top padding is not set on the grid, so the info box starts under the top bar via `appBarPadding = topPadding`.
- Grid items in this order (each full span `GridItemSpan(maxLineSpan)` except seasons):
  1. `INFO_BOX` (lines 483-498): `AnimeInfoBox(isTabletUi = false, appBarPadding = topPadding, ...)`.
  2. `ACTION_ROW` (lines 500-518): `AnimeActionRow(...)`.
  3. `DESCRIPTION_WITH_TAG` (lines 520-537): `ExpandableAnimeDescription(defaultExpandState = state.isFromSource, ...)`.
  4. `ITEM_HEADER` (lines 539-562): `ItemHeader(enabled = !isAnySelected, itemCount = seasons.size or episodes.size, missingItemsCount = maxOf(missingEpisodesCount, missingSeasonsCount), onClick = onFilterClicked, isManga = false, fetchType = ...)`. `missingEpisodesCount = episodes.map { it.episode.episodeNumber }.missingEntriesCount()` and `missingSeasonsCount = seasons.map { it.seasonAnime.anime.seasonNumber }.missingEntriesCount()` (section 8).
  5. Either `sharedSeasons(...)` (FetchType.Seasons, lines 564-574) or the episode branch (FetchType.Episodes, lines 576-621):
     - If `state.airingTime > 0L`, an `AIRING_TIME` item (lines 578-605). Full span.
     - `sharedEpisodeItems(...)` (lines 607-620).
- `ignorePadding(offsetGridPaddingPx)` (line 496 and onward) is applied to each child. Section 1.5.

### 1.3 Large (two-panel) layout

OBSERVED. `AnimeScreenLargeImpl` (lines 631-962).
- Differences from the small layout:
  - No `INFO_BOX`, `ACTION_ROW` or `DESCRIPTION_WITH_TAG` in the grid. They are placed in the start panel of `TwoPanelBox` (lines 816-860). The start panel is a `Column` with `verticalScroll(rememberScrollState())` and bottom padding `contentPadding.calculateBottomPadding()`.
  - Start panel contents in order: `AnimeInfoBox(isTabletUi = true, appBarPadding = contentPadding.calculateTopPadding(), ...)`, `AnimeActionRow(...)`, `ExpandableAnimeDescription(defaultExpandState = true, ...)`. Large always starts expanded (`defaultExpandState = true`, line 849), while small uses `state.isFromSource`.
  - End panel: `FastScrollLazyVerticalGrid` with `contentPadding = PaddingValues(start = GRID_PADDING, end = GRID_PADDING, top = contentPadding.calculateTopPadding(), bottom = ...)` (lines 866-871). Items: `ITEM_HEADER`, then seasons or episodes.
  - The large `AIRING_TIME` item (lines 910-938) has NO `span = { GridItemSpan(maxLineSpan) }` argument. The small version has it (lines 578-583). OBSERVED inconsistency: in the large grid the countdown occupies one column cell, not the full width. Proposal: use full width in both layouts.
  - Top bar: `titleAlphaProvider = { 1f }` and `backgroundAlphaProvider = { 1f }` (lines 751-752). No fade.
  - Bottom bar: wrapped in `Box(fillMaxWidth, contentAlignment = BottomEnd)` and `fillFraction = 0.5f` (lines 756-777). The action menu covers the right half only.
  - Pull refresh indicator padding uses `topBarHeight` (lines 810-814).
  - Season container height is `containerHeightPx - topBarHeight` (line 903).
- `isAnySelected`, `hasFilters`, `BackHandler` are the same as small (lines 699-724).

### 1.4 Shared bottom action menu and its predicates

OBSERVED. `SharedAnimeBottomActionMenu` (lines 964-1020) calls `EntryBottomActionMenu(visible = selected.isNotEmpty(), modifier = modifier.fillMaxWidth(fillFraction), ...)`. Each action is passed as a lambda plus a `takeIf` predicate. `null` hides the action. Predicates (exact):

| Action (callback) | Shown when (exact predicate) | Callback invoked with |
|---|---|---|
| `onBookmarkClicked` | `selected.fastAny { !it.episode.bookmark }` | `onMultiBookmarkClicked(list, true)` |
| `onRemoveBookmarkClicked` | `selected.fastAll { it.episode.bookmark }` | `onMultiBookmarkClicked(list, false)` |
| `onFillermarkClicked` | `selected.fastAny { !it.episode.fillermark }` | `onMultiFillermarkClicked(list, true)` |
| `onRemoveFillermarkClicked` | `selected.fastAll { it.episode.fillermark }` | `onMultiFillermarkClicked(list, false)` |
| `onMarkAsViewedClicked` | `selected.fastAny { !it.episode.seen }` | `onMultiMarkAsSeenClicked(list, true)` |
| `onMarkAsUnviewedClicked` | `selected.fastAny { it.episode.seen \|\| it.episode.lastSecondSeen > 0L }` | `onMultiMarkAsSeenClicked(list, false)` |
| `onMarkPreviousAsViewedClicked` | `selected.size == 1` | `onMarkPreviousAsSeenClicked(selected[0].episode)` |
| `onDownloadClicked` | `onDownloadEpisode != null && selected.fastAny { it.downloadState != DOWNLOADED }` | `onDownloadEpisode!!(selected.toList(), START)` |
| `onDeleteClicked` | `onDownloadEpisode != null && selected.fastAny { it.downloadState == DOWNLOADED }` | `onMultiDeleteClicked(list)` (all selected, not only the downloaded ones) |
| `onExternalClicked` | `!alwaysUseExternalPlayer && selected.size == 1` | `onEpisodeClicked(first, true)` |
| `onInternalClicked` | `alwaysUseExternalPlayer && selected.size == 1` | `onEpisodeClicked(first, true)` |

Notes:
- `list` is `selected.fastMap { it.episode }` (an `Episode` list), except download, which passes `EpisodeList.Item` objects.
- `fastAny`, `fastAll`, `fastMap` are Compose inline helpers with the same semantics as `any`, `all`, `map`. An empty `selected` never reaches these predicates, because `visible = selected.isNotEmpty()`.
- The external and internal actions both pass `alt = true`. Only the visibility differs. Proposal: keep `alt = true` for both and make the caller's `alwaysUseExternalPlayer` setting decide the player.
- `isManga = false` is passed; the button order and icons come from `EntryBottomActionMenu`, which is outside this scope.

Proposal (Rust): the same predicates in `src/anime_details/selection.rs::bottom_actions(selected: &[EpisodeItem], ctx: &MenuContext) -> Vec<BottomAction>`. Return only enabled actions in the order given by the shared menu component. Unit tests cover each row in the table (see p1-edge-cases-and-tests.md).

### 1.5 `ignorePadding` full-bleed trick

OBSERVED. `private fun Modifier.ignorePadding(gridPadding: Int) = layout { measurable, constraints -> val looseConstraints = constraints.offset(gridPadding * 2, 0); val placeable = measurable.measure(looseConstraints); layout(placeable.width, placeable.height) { placeable.placeRelative(0, 0) } }` (lines 1185-1192). It widens the child's max width by `2 * GRID_PADDING` px and reports the child's measured width. Its purpose is to let info header, action row, description, headers and episode rows extend into the grid's 14 dp side gutter.

Proposal: GTK has no equivalent and no need for it. Use `gtk::ScrolledWindow` with a child box that has 14 px side margins and a full-width child for each row. The result is the same visible width for each row without the layout hack.

## 2. Selection (action mode)

OBSERVED. Callbacks from the caller: `onEpisodeSelected: (EpisodeList.Item, Boolean, Boolean, Boolean) -> Unit`, `onAllEpisodeSelected: (Boolean) -> Unit`, `onInvertSelection: () -> Unit`.

- Long press on a row (`AnimeEpisodeListItem.onLongClick`, `AnimeScreen.kt:1113-1116`): `onEpisodeSelected(episodeItem, !episodeItem.selected, true, true)` and a long-press haptic.
- Click on a row (`AnimeScreen.kt:1117-1124`) goes through `onEpisodeItemClick` (lines 1140-1151):
  - if `episodeItem.selected`: `onToggleSelection(false)`, which calls `onEpisodeSelected(item, false, true, false)`.
  - else if `isAnyEpisodeSelected`: `onToggleSelection(true)`, which calls `onEpisodeSelected(item, true, true, false)`.
  - else: `onEpisodeClicked(item.episode, false)` (open the episode).
- The 4th parameter of `onEpisodeSelected` is `fromLongPress`. The 3rd is `userSelected`. Both are `true` for user gestures (part 2 uses them for range selection).
- Back gesture: if any row is selected, clear selection (`onAllEpisodeSelected(false)`), else navigate up (lines 362-368).
- Top bar in action mode: `actionModeCounter` shows the selected count. Cancel clears the selection. "Select all" calls `onAllEpisodeSelected(true)`. "Invert" calls `onInvertSelection()`. The toolbar component is outside this scope.
- Pull to refresh is disabled while selecting (`enabled = !isAnySelected`).
- FAB is hidden while selecting (section 9).
- Row selection background: `Modifier.selectedBackground(selected)`. Colour is `MaterialTheme.colorScheme.secondary` with alpha 0.16 in dark theme and 0.22 in light theme (`presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt:27-37`).
- Summary expansion is disabled while any episode is selected (`EpisodeSummary`, `if (isAnyEpisodeSelected) Modifier else Modifier.clickable {...}`).
- Download indicator is disabled while any episode is selected: `downloadIndicatorEnabled = !isAnyEpisodeSelected && !anime.isLocal()` (`AnimeScreen.kt:1108`).

Proposal (GTK): `gtk::ListView` with `gtk::MultiSelection` is not used, because episodes carry extra per-row widgets. Keep a `Vec<bool>` (or a `HashSet<i64>` of episode ids) in the model. Ctrl-click toggles one row (matches the non-long-press path). Shift-click selects a range (desktop addition, no Kotlin equivalent). Long press (about 500 ms, GTK default) enters selection like the source. Esc clears selection.

## 3. Episode row (`AnimeEpisodeListItem`)

### 3.1 Title text

OBSERVED. `AnimeScreen.kt:1082-1089`: when `anime.displayMode == Anime.EPISODE_DISPLAY_NUMBER` (`0x00100000L`), title = `stringResource(AYMR.strings.display_mode_episode, formatEpisodeNumber(episode.episodeNumber))` which renders "Episode {n}". Otherwise title = `episode.name`.

`formatEpisodeNumber(d: Double)` = `DecimalFormat("#.###", DecimalFormatSymbols() with decimalSeparator '.').format(d)` (`app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt:6-17`). See section 10 for rounding details.

### 3.2 Layout selection

OBSERVED. `AnimeEpisodeListItem.kt:125-140`:
- `previewUrl.isNullOrBlank() && summary.isNullOrBlank()` gives the simple layout: `SimpleEpisodeListItemImpl`.
- Otherwise the rich layout: an outer `Column` with
  - a `Row(spacedBy 2 dp)` containing `EpisodeThumbnail(previewUrl)` and a `Column` with the title row and `EpisodeSummary`;
  - a bottom `Row(SpaceBetween)` with `EpisodeInformation` and, when `previewUrl != null`, the bookmark and download icons.
- Row padding: `start = 16.dp, top = 12.dp, end = 8.dp, bottom = 12.dp` (line 123).

Simple layout (`SimpleEpisodeListItemImpl`, lines 214-256):
- `Column(modifier.weight(1f), verticalArrangement = spacedBy(if (fillermark) 0.dp else 6.dp))`.
- Title: `bodyMedium`, `maxLines = 1`, ellipsis, colour alpha `DISABLED_ALPHA` (0.38) when seen else `1f`.
- `EpisodeInformation` below the title.
- Trailing `BookmarkDownloadIcons`.

Rich layout title row (lines 149-175):
- Title `Text`: `bodyMedium.copy(lineHeight = 14.sp)`, `maxLines = titleLines`, `minLines = titleLines` where `titleLines = if (previewUrl == null) 1 else 2`, ellipsis, colour alpha as above.
- Trailing `BookmarkDownloadIcons` only when `previewUrl == null`.

### 3.3 Episode information line

OBSERVED. `EpisodeInformation` (lines 410-461) inside `ProvideTextStyle(bodySmall merged with colour)`. Colour alpha: `DISABLED_ALPHA` when seen, else `SECONDARY_ALPHA` (0.78).
1. If `fillermark`: an icon `Icons.AutoMirrored.Filled.Label` (16 dp by default icon size) with `contentDescription = stringResource(AYMR.strings.filler)` ("Filler"), tint `MaterialTheme.colorScheme.tertiary` with the text alpha, then `Text("Filler")` with end padding 4 dp, same tint.
2. If `date != null`: `Text(date, maxLines = 1, ellipsis)`. Then a `DotSeparatorText()` (" • ") if `watchProgress != null || scanlator != null`.
3. If `watchProgress != null`: `Text(watchProgress, maxLines = 1, ellipsis, colour alpha DISABLED_ALPHA)`. Then a dot separator if `scanlator != null`.
4. If `scanlator != null`: `Text(scanlator, maxLines = 1, ellipsis)`.

The date is `relativeDateTimeText(episode.dateUpload)` (`AnimeScreen.kt:1090`). Its formatting is in `app/src/main/java/eu/kanade/presentation/components/...` (outside scope).

### 3.4 Watch progress string

OBSERVED. `AnimeScreen.kt:1091-1099`: `watchProgress = episode.lastSecondSeen.takeIf { !episode.seen && it > 0L }?.let { stringResource(AYMR.strings.episode_progress, formatTime(it), formatTime(episode.totalSeconds)) }`. `episode_progress` = `"%1$s/%2$s"`. Example: "12:34/24:00".

Units: `lastSecondSeen` and `totalSeconds` are **milliseconds** despite the field names. The player stores `position * 1000L` and `duration * 1000L` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1664-1668`). `formatTime` takes milliseconds.

If `totalSeconds == 0` the string is "12:34/0:00". OBSERVED, not special-cased.

### 3.5 Bookmark and download icons

OBSERVED. `BookmarkDownloadIcons` (lines 464-492):
- If `bookmark`: `Icons.Filled.Bookmark`, `contentDescription = stringResource(MR.strings.action_filter_bookmarked)` ("Bookmarked"), `Modifier.secondaryItemAlpha().padding(start = 4.dp)`, tint `primary`.
- Always: `EpisodeDownloadIndicator(enabled = downloadIndicatorEnabled, modifier = Modifier.padding(start = 4.dp), ...)`. Click forwards to `onDownloadClick?.invoke(action)`.

### 3.6 Swipe actions

OBSERVED. Lines 89-106 and 258-297.
- Start and end actions come from `episodeSwipeStartAction` and `episodeSwipeEndAction`. The preference-key naming is inverted (see section 11 and p1-data-and-config.md).
- Each action is built by `getSwipeAction(action, seen, bookmark, fillermark, downloadState, background = primaryContainer, onSwipe)`:

| `LibraryPreferences.EpisodeSwipeAction` | Icon (outlined unless noted) | `isUndo` | Callback |
|---|---|---|---|
| `ToggleSeen` | `if (!seen) Done else RemoveDone` | `seen` | `onEpisodeSwipe(action)` |
| `ToggleBookmark` | `if (!bookmark) BookmarkAdd else BookmarkRemove` | `bookmark` | same |
| `ToggleFillermark` | `if (!fillermark) NewLabel else LabelOff` (AutoMirrored) | `fillermark` | same |
| `Download` | `NOT_DOWNLOADED, ERROR -> Download`; `QUEUE, DOWNLOADING -> FileDownloadOff`; `DOWNLOADED -> Delete` | (default false) | same |
| `Disabled` | none (`null`) | | |

- Background for each action: `MaterialTheme.colorScheme.primaryContainer`. Icon padding 16 dp, tint `contentColorFor(background)`.
- Thresholds: `swipeActionThreshold = 56.dp`. Background behind the row until threshold: `surfaceContainerLowest`.
- The `SwipeableActionsBox` clips to bounds (`modifier.clipToBounds()`).

Proposal: on desktop, a horizontal drag gesture (`gtk::GestureDrag`) reveals the action on the left or right edge. Release past 56 px triggers the action. Also expose the same five actions through the row context menu (mouse right click) so they are reachable without dragging.

### 3.7 Summary and thumbnail (rich layout)

OBSERVED.
- `EpisodeThumbnail(previewUrl)` (lines 360-375): `targetWidth = (LocalConfiguration.current.screenWidthDp * 0.4f).coerceAtMost(250f)` dp. If `previewUrl != null` it renders `ItemCover.Thumb` with a Coil request `crossfade(true)`. Pass `previewUrl` only when `showPreviews` is on (`AnimeScreen.kt:1102`: `previewUrl = episode.previewUrl.takeIf { !it.isNullOrBlank() && showPreviews }`).
- `EpisodeSummary` (lines 377-407): `var expandSummary by remember { mutableStateOf(false) }` (not saveable; resets when the row leaves composition). If `summary != null`:
  - `Text(summary, labelMedium, maxLines = if (expandSummary) Int.MAX_VALUE else 3, minLines = 3, fontWeight = Normal, fontSize = 10.sp, lineHeight = 11.sp, ellipsis, colour alpha = DISABLED_ALPHA if seen else SECONDARY_ALPHA)`.
  - Padding `bottom 4 dp, start 4 dp, end 4 dp`.
  - Click toggles `expandSummary` only when no episode is selected.
- Summary passed as `episode.summary.takeIf { !it.isNullOrBlank() && showSummaries }` (`AnimeScreen.kt:1101`).

### 3.8 Next-episode airing row

OBSERVED. `NextEpisodeAiringListItem(title, date, modifier)` (lines 299-334):
- Padding `start 16, top 12, end 8, bottom 12`.
- Title: `bodyMedium.copy(lineHeight = 14.sp)`, `maxLines = 1`, ellipsis, `Modifier.alpha(SECONDARY_ALPHA)`, colour `primary`.
- Spacer 6 dp.
- Date: `bodySmall`, `maxLines = 1`, ellipsis, same alpha, colour `primary`.
- Title = `stringResource(AYMR.strings.display_mode_episode, formatEpisodeNumber(state.airingEpisodeNumber))` ("Episode {n}"). Date = `formatTime(state.airingTime, useDayFormat = true)`.

Visibility (`AnimeScreen.kt:577-605` small, `911-937` large): shown only when `timer > 0L && showNextEpisodeAirTime && state.anime.status.toInt() != SAnime.COMPLETED` where `showNextEpisodeAirTime` is the preference `show_next_episode_airing_time` (default `true`, see p1-data-and-config.md). The row is only composed when `state.airingTime > 0L`.

Countdown: `var timer by remember { mutableLongStateOf(state.airingTime) }`, and `LaunchedEffect(key1 = timer) { if (timer > 0L) { delay(1000L); timer -= 1000L } }`. This decrements once per second and re-keys the effect.

OBSERVED defect: `remember { mutableLongStateOf(state.airingTime) }` is not keyed on `state.airingTime`. If the ScreenModel later emits a new airing time (after a refresh), the countdown keeps its old value. The large-layout comment says "reseting" but nothing resets. Proposal: compute the remaining time from an absolute target timestamp on every tick (`remaining = target_ms - now_ms`), with a `glib::timeout_add_local(1 s)` that is cancelled when the widget is destroyed. Re-derive on every model update.

### 3.9 Airing time text format (`formatTime(ms, useDayFormat = true)`)

OBSERVED. `AnimeScreen.kt:1153-1182`:
- `useDayFormat = true`: `String.format("Airing in %02dd %02dh %02dm %02ds", days, hours, minutes, seconds)` where:
  - `days = TimeUnit.MILLISECONDS.toDays(ms)`
  - `hours = toHours(ms) - DAYS.toHours(days)`
  - `minutes = toMinutes(ms) - HOURS.toMinutes(toHours(ms))`
  - `seconds = toSeconds(ms) - MINUTES.toSeconds(toMinutes(ms))`
  - Note the literal text "Airing in" is not a string resource (hard-coded English in the Kotlin). Proposal: Rust uses the message key `AYMR.strings.airing_in_format` (not present in the source; add a Rust-side resource with the same English text, and document the gap).
- `useDayFormat = false` and `ms > 3600000L`: `"%d:%02d:%02d"` (H:MM:SS, hours not zero-padded).
- else: `"%d:%02d"` (M:SS, minutes not zero-padded; `ms = 3600000` exactly gives `60:00`).

Ms-to-seconds truncation uses integer division. `formatTime(0)` is `"0:00"`.

### 3.10 Missing-episode separator row

OBSERVED. `EpisodeList.MissingCount(id, count)` rows are rendered by `MissingItemCountListItem(count, modifier = itemModifier)` (lines 1074-1078). The grid key is `"missing-count-${episodeItem.id}"` (line 1064). The separator is full width (`GridItemSpan(maxLineSpan)`). The row is produced by the ScreenModel (part 2) and counted by `missingEntriesCount` (section 8).

## 4. Episode grid key and content types

OBSERVED. `sharedEpisodeItems` keys: `"missing-count-${id}"` for `MissingCount` and `"episode-${id}"` for `Item` (lines 1062-1067). `contentType` is `EntryScreenItem.ITEM` (line 1068). Season keys are the whole `Anime` object (`season.seasonAnime.anime`, line 1032). Proposal: key seasons by `anime.id` and episodes by `episode.id`, which avoids hashing whole objects.

`EntryScreenItem` (declared in `app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt`): `INFO_BOX, ACTION_ROW, DESCRIPTION_WITH_TAG, ITEM_HEADER, ITEM, AIRING_TIME`. Compose-only keys; Rust drops them (P3, task T-39a-059).

## 5. Season tiles

OBSERVED. `sharedSeasons` (lines 1022-1044): items in the grid with span `if (anime.seasonDisplayGridMode == SeasonDisplayMode.List) maxLineSpan else 1`. `AnimeSeasonListItem(anime, item, containerHeight, onSeasonClicked, onClickContinueWatching, listItemModifier)`.

`AnimeSeasonListItem.kt:21-129`:
- `itemAnime = item.seasonAnime.anime`.
- Title: if `anime.seasonDisplayMode == Anime.SEASON_DISPLAY_MODE_NUMBER` (`0x08000000L`) then `stringResource(AYMR.strings.display_mode_season, formatEpisodeNumber(itemAnime.seasonNumber))` ("Season {n}"); else `itemAnime.title`.
- `SeasonDisplayMode.ComfortableGrid`: `EntryComfortableGridItem(title, coverData = AnimeCover(animeId, sourceId, isAnimeFavorite, url = thumbnailUrl, lastModified = coverLastModified), coverBadgeStart = { DownloadsBadge(count = item.downloadCount); UnviewedBadge(count = item.unseenCount) }, coverBadgeEnd = { LanguageBadge(isLocal = item.isLocal, sourceLanguage = item.sourceLanguage) }, onLongClick = onSeasonClicked(item.seasonAnime), onClick = same, onClickContinueViewing = if (onClickContinueWatching != null && item.showContinueOverlay) { ... } else null)`.
- `CompactGrid` and `CoverOnlyGrid`: `EntryCompactGridItem` with `title = title.takeIf { anime.seasonDisplayGridMode is SeasonDisplayMode.CompactGrid }` (so no title in cover-only), same badges and continue overlay.
- `List`: `EntryListItem(title, coverData, badge = { DownloadsBadge; UnviewedBadge; LanguageBadge }, onLongClick, onClick, onClickContinueViewing, entries = anime.seasonDisplayGridSize, containerHeight, modifier = listItemModifier)`. In List mode all three badges go in one slot.

Season item model (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeSeasonItem.kt`): `seasonAnime: SeasonAnime`, `downloadCount: Long = -1L`, `unseenCount: Long = -1L`, `isLocal: Boolean = false`, `sourceLanguage: String = ""`, `showContinueOverlay: Boolean = false`. The `-1` default means "not computed" (the badge component decides how to show it; outside this scope).

`SeasonAnime` (`domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt`): `seen = totalCount == seenCount`; `unseenCount = totalCount - seenCount`; `hasStarted = seenCount > 0`; `hasBookmarks = bookmarkCount > 0`; `hasFillermarks = fillermarkCount > 0`.

Click and long-press both call `onSeasonClicked(item.seasonAnime)`. Seasons have no multi-select in this screen.

## 6. Info box (`AnimeInfoBox`)

### 6.1 Backdrop

OBSERVED. `AnimeInfoHeader.kt:117-141`: `AsyncImage(model = ImageRequest.Builder(ctx).data(anime).useBackground(true).crossfade(true).build(), contentScale = Crop, modifier = matchParentSize().drawWithContent { drawContent(); drawRect(brush = verticalGradient(Transparent, colorScheme.background)) }.blur(4.dp).alpha(0.2f))`.

Proposal: a `gtk::Picture` with `content-fit: cover`, opacity 0.2, a CSS gradient overlay from transparent to the window background, and a blur from a pre-blurred downscaled copy (Compose `blur` has no direct equivalent in GTK4 without a shader; `gtk::GraphicsOffload` is not needed). Blur radius 4 dp approximated by downscale to 1/8 then upscale with bilinear filter.

### 6.2 Column layout

OBSERVED.
- Small: `Row(Modifier.fillMaxWidth().padding(start 16, top appBarPadding + 16, end 16), spacedBy 16, CenterVertically)`. Cover `ItemCover.Book(modifier = sizeIn(maxWidth = 100.dp).align(Top), ...)`. Right column `spacedBy(2.dp)` with the text block.
- Large: `Column(fillMaxWidth, padding(start 16, top appBarPadding + 16, end 16), CenterHorizontally)`. Cover `fillMaxWidth(0.65f)`, spacer 16, then the text block with `TextAlign.Center`.
- Cover contentDescription: `stringResource(MR.strings.manga_cover)` ("Cover"). Cover click: `onCoverClick` (opens the images dialog). The cover request uses `crossfade(true)` only (no `useBackground`).

### 6.3 Text block (`AnimeContentInfo`)

OBSERVED. `AnimeInfoHeader.kt:433-579`.

1. Title: `Text(title.ifBlank { "Unknown title" }, titleLarge, modifier = clickableNoIndication(onLongClick = { if (title.isNotBlank()) copyToClipboard(title, title) }, onClick = { if (title.isNotBlank()) doSearch(title, true) }), textAlign)`.
   - `doSearch(query, global = true)`: global search across all sources.
2. Spacer 2 dp.
3. Author row (`secondaryItemAlpha`, `spacedBy(extraSmall = 4 dp)`):
   - Icon `PersonOutline` 16 dp.
   - `Text(author?.takeIf { it.isNotBlank() } ?: "Unknown author", titleSmall, clickableNoIndication(onLongClick = copy if !isNullOrBlank, onClick = doSearch(author, true) if !isNullOrBlank))`.
4. Artist row shown only if `!artist.isNullOrBlank() && author != artist` (exact string inequality, so `author = null` with `artist = "X"` shows the artist). Icon `Brush` 16 dp. `Text(artist, titleSmall, long-press copy, click doSearch(artist, true))`.
5. Spacer 2 dp.
6. Status row (`secondaryItemAlpha`, `CenterVertically`):
   - Icon and text by status (table below). Icon size 16 dp, end padding 4 dp.
   - `DotSeparatorText()` (" • ").
   - If `isStubSource`: `Icons.Filled.Warning` 16 dp, tint `colorScheme.error`, end padding 4 dp.
   - Source name `Text(sourceName, clickableNoIndication { doSearch(sourceName, false) }, ellipsis, maxLines = 1)`. `global = false` means search only this source.

Status mapping (`status: Long`, values from `SAnime`):

| SAnime constant | value | Icon | Label (MR/AYMR key, English text) |
|---|---|---|---|
| `UNKNOWN` | 0 | `Icons.Outlined.Block` (else branch) | `MR.strings.unknown` "Unknown" |
| `ONGOING` | 1 | `Icons.Outlined.Schedule` | `MR.strings.ongoing` "Ongoing" |
| `COMPLETED` | 2 | `Icons.Outlined.DoneAll` | `MR.strings.completed` "Completed" |
| `LICENSED` | 3 | `Icons.Outlined.AttachMoney` | `MR.strings.licensed` "Licensed" |
| `PUBLISHING_FINISHED` | 4 | `Icons.Outlined.Done` | `MR.strings.publishing_finished` "Publishing finished" |
| `CANCELLED` | 5 | `Icons.Outlined.Close` | `MR.strings.cancelled` "Cancelled" |
| `ON_HIATUS` | 6 | `Icons.Outlined.Pause` | `MR.strings.on_hiatus` "On hiatus" |
| `UPCOMING` | 7 | `ImageVector.vectorResource(R.drawable.ic_calendar_clock_24dp)` | `AYMR.strings.upcoming` "Upcoming" |
| any other | | `Block` | `MR.strings.unknown` |

Source name: `AnimeSource.getNameForAnimeInfo()` (`app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt:18-31`). Let `enabled` = `SourcePreferences.enabledLanguages()` minus `"all"` and `"other"`. If `enabled.size == 1` and the source language is not in `enabled`: `toString()`. If `enabled.size == 1` and the language is in `enabled`: `name` (language tag hidden). Otherwise `toString()`. `AnimeHttpSource.toString()` is `"$name (${lang.uppercase()})"` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:117`).

`isStubSource = state.source is StubAnimeSource` (`tachiyomi.domain.source.anime.model.StubAnimeSource`). A stub is a source whose extension is not installed.

## 7. Action row (`AnimeActionRow`)

OBSERVED. `AnimeInfoHeader.kt:169-243`. Padding `start 16, top 8, end 16`. Each button is `TextButton(weight 1f)` containing a 20 dp icon, a 4 dp spacer and 12sp centred text.

Default colour for inactive buttons: `onSurface.copy(alpha = DISABLED_ALPHA)` (0.38).

| # | Condition | Title | Icon | Colour | Click | Long click |
|---|---|---|---|---|---|---|
| 1 | always | `in_library` "In library" if `favorite`, else `add_to_library` "Add to library" | `Favorite` if favorite, else `FavoriteBorder` | `primary` if favorite, else inactive | `onAddToLibraryClicked` | `onEditCategory` (may be null) |
| 2 | always | `not_applicable` "N/A" if `nextUpdateDays == null`; `manga_interval_expected_update_soon` "Soon" if 0; else plural `day` "1 day" / "%d days" | `HourglassEmpty` | `primary` if `isUserIntervalMode` (`fetchInterval < 0`), else inactive | `onEditIntervalClicked?.invoke()` | none |
| 3 | only if `onTrackingClicked != null` | `manga_tracking_tab` "Tracking" if `trackingCount == 0`; else plural `num_trackers` "1 tracker" / "%d trackers" | `Sync` if 0, else `Done` | inactive if 0, else `primary` | `onTrackingClicked` | none |
| 4 | only if `onWebViewClicked != null` | `action_web_view` "WebView" | `Public` | inactive | `onWebViewClicked` | `onWebViewLongClicked` |

`nextUpdateDays`: `if (nextUpdate != null) now.until(nextUpdate, ChronoUnit.DAYS).toInt().coerceAtLeast(0) else null`. Truncation toward zero of whole days; past dates give 0, which shows "Soon". `nextUpdate` is `Instant?` computed by the caller (part 2).

Note: the source comment at line 184 says the interval label should be improved for custom intervals (unfinished in the source).

## 8. Missing-count algorithm

OBSERVED. `List<Double>.missingEntriesCount()` (`domain/src/main/java/tachiyomi/domain/items/episode/service/MissingEpisodes.kt:6-34`):
1. Empty list gives 0.
2. Drop values equal to `-1.0` (unknown numbers).
3. `toInt()` each value (truncation: `16.5` becomes `16`), then `distinct()`, then `sorted()`.
4. Empty gives 0.
5. Walk the sorted list with `previousEntry = 0`. For each `currentEntry`, if `currentEntry > previousEntry + 1` add `currentEntry - previousEntry - 1`. Set `previousEntry = currentEntry`.

Example: episode numbers `[1.0, 2.0, 4.0, 4.5, 7.0, -1.0]` become `[1, 2, 4, 4, 7]`, distinct `[1, 2, 4, 7]`, missing = (4-2-1) + (7-4-1) = 1 + 2 = 3.

Note that `previousEntry` starts at 0, so if the first number is 3 the missing count includes 1 and 2 (`3 - 0 - 1 = 2`). The count is over the whole list passed in. For episodes, the caller passes `processedEpisodes` (filtered and sorted), so an active filter changes the missing count (OBSERVED behaviour; see p1-edge-cases-and-tests.md).

The `ItemHeader` shows `missingItemsCount = maxOf(missingEpisodesCount, missingSeasonsCount)`. For `FetchType.Seasons` the episode count is still computed but the header uses the max.

## 9. FAB (`ExtendedFloatingActionButton`)

OBSERVED. Small `AnimateVisibility` (`AnimeScreen.kt:432-462`), large identical (`779-804`).
- Visible: `episodes.fastAny { !it.episode.seen } && !isAnySelected`. `episodes` = `state.processedEpisodes` (filtered and sorted; `EpisodeList.Item` only).
- Enter: `fadeIn()`; exit: `fadeOut()` (Compose default tween).
- Label: `if (isWatching) MR.strings.action_resume else MR.strings.action_start`, where `isWatching = state.episodes.fastAny { it.episode.seen }` (`state.episodes` is the unfiltered list). So visibility uses the filtered list but the label uses the unfiltered list. OBSERVED inconsistency. Proposal: use the unfiltered list for both the label and the "any unseen" check, so the FAB never disappears because of a filter. Record in p1-edge-cases-and-tests.md.
- Icon: `Icons.Filled.PlayArrow`, content description null.
- Click: `onContinueWatching` (`() -> Unit`, supplied by the caller; the episode it opens is decided in part 2).
- `expanded = itemListState.shouldExpandFAB()`. `shouldExpandFAB` (`presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt:22-30`) is true when `(firstVisibleItemIndex == 0 && firstVisibleItemScrollOffset == 0) || lastScrolledBackward || !canScrollForward`. Collapsed FAB shows only the icon.

Proposal: GTK `gtk::Button` pill "Start"/"Resume" in an overlay at the bottom end, with 18 px margins (matches the mockup). Collapsed-on-scroll behaviour: hide the label when the list scrolls forward (not last scrolled backward), to match `shouldExpandFAB`.

## 10. Number and time formatting details

- `formatEpisodeNumber`: `DecimalFormat("#.###")`, decimal separator forced to `.`. Rounding mode is the `DecimalFormat` default `HALF_EVEN`. Values have at most three fraction digits; no grouping separator. Examples: `1.0` gives `1`, `12.5` gives `12.5`, `3.14159` gives `3.142`, `2.0005` gives `2` (half-even rounds 2.0005 to 2.000 because the digit before is even; the exact binary value decides). Digits follow the default locale's `DecimalFormatSymbols` (for example Arabic-Indic digits for some `ar` locales on some JDKs). Proposal: always ASCII digits and round-half-even on the decimal representation (`rust_decimal` or manual `format!("{:.3}")` with trailing zero trimming). Document the locale difference in the release notes.
- `formatTime`: uses `String.format` with the default locale (`%d` and `%02d`), so localized digits are possible. Proposal: ASCII digits (see above).

## 11. Preferences read in this scope (behaviour)

Full keys and defaults are in p1-data-and-config.md. Behaviour summary:
- `show_next_episode_airing_time` (`TrackPreferences`, default `true`): gates the airing row.
- `pref_downloaded_only` (`BasePreferences.downloadedOnly()`, default `false`): when `true`, the "Downloaded" filter in the episode and season filter pages is forced `ENABLED_IS` and cannot be changed (`onDownloadFilterChanged.takeUnless { downloadedOnly }` gives `null`, which disables the item).
- Episode swipe actions: the Kotlin key names are swapped relative to the property names (`swipeEpisodeStartAction()` reads `pref_episode_swipe_end_action`, default `ToggleSeen`; `swipeEpisodeEndAction()` reads `pref_episode_swipe_start_action`, default `ToggleBookmark`). Proposal: keep the stored key names (backup and config compatibility) and name the Rust accessors to match what the user sees: left-swipe uses `pref_episode_swipe_end_action`, right-swipe uses `pref_episode_swipe_start_action`, and document that the Kotlin property names are inverted. Verify the mapping of "start" and "end" to left and right against the swipe container in part 2 before shipping.
- `use_external_downloader` (`DownloadPreferences`, default `false`): decides which download button is primary in the quality dialog (`downloadEpisode(useExternalDownloader)` and `downloadEpisode(!useExternalDownloader)`).
- `alwaysUseExternalPlayer` (passed in): decides which of the two play actions is shown in the bottom menu.

## 12. Images dialog and `AnimeImageScreenModel`

Full state and flow are in p1-topic-cover-background-dialog.md. Summary:
- Pager has 2 pages (`PagerState(pageCount = { 2 })`). `isCover = pagerState.currentPage != 1`, so page 0 is the cover and page 1 the background.
- Bottom bar left pill: Close, and an arrow that scrolls to the other page. Right pill: Share, Save, and Edit (dropdown with Edit and Delete when a custom image exists on the current page).
- Save and share use the image as loaded at original size (`Size.ORIGINAL`). The Kotlin background save does not call `useBackground(true)`; see defect D-3 in p1-edge-cases-and-tests.md.

## 13. Quality (`EpisodeOptionsDialogScreen`) summary

Full state machine is in p1-topic-quality-options-dialog.md. Summary:
- Opened from the episode row's long press on a not-downloaded episode (`NotDownloadedIndicator` long press emits `EpisodeDownloadAction.SHOW_QUALITIES`). The ScreenModel handles it in `AnimeScreenModel.kt:828-830` by calling `showQualitiesDialog(episode)` (`AnimeScreenModel.kt:1598-1600`), which sets `Dialog.ShowQualities(episode, anime, source)`.
- Loads every hoster (`EpisodeLoader.getHosters`) and then each hoster's videos in parallel (`async` per hoster, `awaitAll`).
- Auto-selects the preferred video (`HosterLoader.selectBestVideo`) once a hoster with `preferred` video becomes Ready.
- Toast on failure: `"No available videos"` (English literal, not a resource key; `EpisodeOptionsDialog.kt:438`). Log: `"Error getting links"`.

## 14. Duplicate-anime sheet

OBSERVED. `DuplicateAnimeDialog.kt:35-122`.
- Title: `MR.strings.are_you_sure` ("Are you sure?"), `headlineMedium`, padding bottom 16 dp, top 8 dp.
- Body: `MR.strings.confirm_add_duplicate_manga`: "You have an entry in your library with the same name.\n\nDo you still wish to continue?". The key name says "manga" but the English text is generic and is used for anime too.
- Spacer 16 dp.
- Three rows separated by `HorizontalDivider`, each `TextPreferenceWidget` (icon + title). Each row runs `onDismissRequest()` first and then its action:
  1. `AYMR.strings.action_show_anime` "Show anime" (icon `Book`) then `onOpenAnime()`.
  2. `MR.strings.action_migrate_duplicate` "Migrate existing entry" (icon `SwapVert`) then `onMigrate()`.
  3. `MR.strings.action_add_anyway` "Add anyway" (icon `Add`) then `onConfirm()`.
- Cancel: `OutlinedButton` with `MR.strings.action_cancel` "Cancel", text colour `primary`, `titleLarge`, 16 sp. The button sits in a `Row` clickable that also dismisses. Button padding top and bottom 16 dp.
- Container: `AdaptiveSheet` (bottom sheet on phones, dialog on wide windows), padding vertical 8 dp and horizontal 24 dp (`TabbedDialogPaddings`).

Proposal (GTK): a modal `gtk::Window` titled empty, containing the title, body and three `gtk::ListBox` rows, then Cancel. `Escape` dismisses.

## 15. Episode settings sheet (Filter, Sort, Display)

OBSERVED. `EpisodeSettingsDialog.kt:40-119`. Tabs: `MR.strings.action_filter` ("Filter"), `MR.strings.action_sort` ("Sort"), `MR.strings.action_display` ("Display"). Overflow menu item `MR.strings.set_chapter_settings_as_default` ("Set as default") opens `SetAsDefaultDialog(isEpisode = true)`.

Defaults when `anime == null` (used only before the anime loads): filters `DISABLED`, `sortingMode = 0`, `sortDescending = false`, `displayMode = 0`, `showPreviews = true`, `showSummaries = true`.

### 15.1 Filter page (`FilterPage`, lines 122-152)

Four tri-state rows, in this order. Each row cycles the tri-state through the caller's callback.
1. `MR.strings.label_downloaded` "Downloaded". Callback is `onDownloadFilterChanged.takeUnless { downloadedOnly }` (disabled when the global Downloaded-only mode is on). Value `anime.downloadedFilter` (returns `ENABLED_IS` when `downloadedOnly` is on, regardless of the stored value).
2. `AYMR.strings.action_filter_unseen` "Unseen". Value `anime.unseenFilter`.
3. `MR.strings.action_filter_bookmarked` "Bookmarked". Value `anime.bookmarkedFilter`.
4. `AYMR.strings.action_filter_fillermarked` "Fillermarked". Value `anime.fillermarkedFilter`.

### 15.2 Sort page (`SortPage`, lines 155-172)

Four `SortItem`s, in this order. `sortDescending = sortDescending.takeIf { sortingMode == mode }` (direction indicator only on the active mode). Click calls `onItemSelected(mode)`.

| Label key | English | Mode constant | Value |
|---|---|---|---|
| `MR.strings.sort_by_source` | "By source" | `EPISODE_SORTING_SOURCE` | `0x00000000L` |
| `AYMR.strings.sort_by_episode_number` | "By episode number" | `EPISODE_SORTING_NUMBER` | `0x00000200L` |
| `MR.strings.sort_by_upload_date` | "By upload date" | `EPISODE_SORTING_UPLOAD_DATE` | `0x00000400L` |
| `MR.strings.action_sort_alpha` | "Alphabetically" | `EPISODE_SORTING_ALPHABET` | `0x00000600L` |

Direction is `sortDescending()` = `episodeFlags & EPISODE_SORT_DIR_MASK == EPISODE_SORT_DESC` (`0x1` clear means descending). Toggling direction on re-click is in part 2.

### 15.3 Display page (`DisplayPage`, lines 175-205)

Two radio items, then two checkboxes:
1. `MR.strings.show_title` "Source title": `displayMode == EPISODE_DISPLAY_NAME (0x00000000L)`.
2. `AYMR.strings.show_episode_number` "Episode number": `EPISODE_DISPLAY_NUMBER (0x00100000L)`.
3. Checkbox `AYMR.strings.show_episode_previews` "Episode thumbnails", checked when `showPreviews()`. Click sends `EPISODE_SHOW_NOT_PREVIEWS (0x00000800L)` if currently shown, else `EPISODE_SHOW_PREVIEWS (0x00000000L)`.
4. Checkbox `AYMR.strings.show_episode_summaries` "Episode summaries", checked when `showSummaries()`. Click sends `EPISODE_SHOW_NOT_SUMMARIES (0x00001000L)` if currently shown, else `EPISODE_SHOW_SUMMARIES (0x00000000L)`.

The checkbox callbacks send the mask value to set, not a boolean. The consumer writes `flags = (flags & ~MASK) | value` (see p1-topic-flags-filters-and-seasons.md).

### 15.4 Set as default (`SetAsDefaultDialog`, lines 207-256)

- `rememberSaveable` checkbox state, default `false`.
- Title: `AYMR.strings.episode_settings` "Episode settings" (or `season_settings` "Season settings" when `isEpisode = false`).
- Text: `MR.strings.confirm_set_chapter_settings` "Are you sure you want to save these settings as default?", then a labelled checkbox `AYMR.strings.also_set_episode_settings_for_library` "Also apply to all anime in my library".
- Buttons: Cancel (`MR.strings.action_cancel`) dismisses. OK (`MR.strings.action_ok`) calls `onConfirmed(optionalChecked)` then dismisses.
- Effect of confirm: writes the current flags into the `default_episode_*` (or `default_season_*`) preferences (`LibraryPreferences.setEpisodeSettingsDefault` / `setSeasonSettingsDefault`, `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:341-347`). With the checkbox on, the same is applied to every anime in the library (part 2).

## 16. Season settings sheet

OBSERVED. `SeasonSettingsDialog.kt:41-340`. Tabs identical to episode tabs.

### 16.1 Season filter page (`SeasonFilterPage`, lines 153-198)

Six tri-states in this order:
1. `label_downloaded` "Downloaded" (same downloaded-only gating as episodes), value `anime.seasonDownloadedFilter`.
2. `action_filter_unseen` "Unseen", value `seasonUnseenFilter`.
3. `label_started` "Started", value `seasonStartedFilter`.
4. `completed` "Completed", value `seasonCompletedFilter`.
5. `action_filter_bookmarked` "Bookmarked", value `seasonBookmarkedFilter`.
6. `action_filter_fillermarked` "Fillermarked", value `seasonFillermarkedFilter`.

### 16.2 Season sort page (`SeasonSortPage`, lines 200-221)

Seven items:

| Label key | English | Value |
|---|---|---|
| `sort_by_source` | "By source" | `SEASON_SORT_SOURCE = 0x00000000L` |
| `sort_by_season_number` | "By season number" | `SEASON_SORT_SEASON = 0x00002000L` |
| `sort_by_upload_date` | "By upload date" | `SEASON_SORT_UPLOAD = 0x00004000L` |
| `action_sort_alpha` | "Alphabetically" | `SEASON_SORT_ALPHABET = 0x00006000L` |
| `action_sort_unseen_count` | "Unseen count" | `SEASON_SORT_COUNT = 0x00008000L` |
| `action_sort_last_seen` | "Last seen" | `SEASON_SORT_LAST_SEEN = 0x0000A000L` |
| `action_sort_episode_fetch_date` | "Episode fetch date" | `SEASON_SORT_FETCHED = 0x0000C000L` |

Direction: `seasonSortDescending()` = `seasonFlags & SEASON_SORT_DIR_MASK == SEASON_SORT_DESC` (`0x1`).

### 16.3 Season display page (`SeasonDisplayPage`, lines 230-340)

Order of controls:
1. `SettingsChipRow(MR.strings.action_display_mode)`: four `FilterChip`s from `displayModes` (lines 223-228): `action_display_grid` "Compact grid" → `CompactGrid`; `action_display_comfortable_grid` "Comfortable grid" → `ComfortableGrid`; `action_display_cover_only_grid` "Cover-only grid" → `CoverOnlyGrid`; `action_display_list` "List" → `List`. Selected chip is `displayGridMode`.
2. Size slider `SliderItem(value = displayGridModeSize, valueRange = 0..10, ...)`:
   - When `displayGridMode == List`: label `AYMR.strings.pref_library_rows` "List size".
   - Otherwise: label `MR.strings.pref_library_columns` "Items per row".
   - `valueText`: the number if `> 0`, else `MR.strings.label_auto` "Auto". Pill colour `surfaceContainerHighest`.
3. `HeadingItem(MR.strings.overlay_header)` "Overlay". Five `LabeledCheckbox`es with `horizontal = SettingsItemsPaddings.Horizontal` padding, in this order:
   - `AYMR.strings.action_display_download_badge_anime` "Downloaded episodes", default `false`.
   - `AYMR.strings.action_display_unseen_badge` "Unseen episodes", default `true`.
   - `MR.strings.action_display_local_badge` "Local source", default `true`.
   - `MR.strings.action_display_language_badge` "Language", default `false`.
   - `AYMR.strings.action_display_show_continue_watching_button` "Continue watching button", default `true`.
   The defaults apply only when `anime == null` (`?: false/true` in lines 134-142). When the anime exists, the getter returns the stored bit, so a flag-less anime shows `false` for every overlay (see p1-edge-cases-and-tests.md, E-7).
4. `HeadingItem(AYMR.strings.action_display_grid_mode)` "Grid display mode". Two radios: `MR.strings.show_title` "Source title" → `SEASON_DISPLAY_MODE_SOURCE (0x0)`; `AYMR.strings.show_season_number` "Season number" → `SEASON_DISPLAY_MODE_NUMBER (0x08000000L)`. Default is `anime?.seasonDisplayMode ?: 0L`.

## 17. Image and cover dialog details

See p1-topic-cover-background-dialog.md (complete). Behavioural summary already in section 12.

## 18. Related anime section

### 18.1 Partition

OBSERVED. `RelatedAnimeRows` (lines 37-144). `val (smalls, multis) = relations.partition { it.anime.size <= MAX_ITEMS_PER_ROW }` with `MAX_ITEMS_PER_ROW = 3`. A group with 1, 2 or 3 items is "small"; 4 or more is "multi".

Outer `Column(modifier.padding(bottom = 16.dp))`. Empty `relations` returns immediately.

### 18.2 Small groups: packing (`packRows`)

OBSERVED. `List<AnimeRelationGroup>.packRows(maxPerRow = 3)` (lines 191-219):
```
if empty -> []
total   = sum(group.anime.size)
rowCount = ceil(total / 3.0)
base     = total / rowCount            (integer division)
remainder = total % rowCount
rows = []; current = []; filled = 0
cap = base + (remainder > 0 ? 1 : 0)
for relation in groups:
    size = relation.anime.size
    if current nonEmpty and filled + size > cap:
        rows += current; current = []; filled = 0
        cap = base + (rows.size < remainder ? 1 : 0)
        if cap < size: cap = maxPerRow
    current += relation; filled += size
if current nonEmpty: rows += current
```

QUIRK (OBSERVED): the first `cap` uses `remainder > 0 ? 1 : 0`, but later caps use `rows.size < remainder`. The number of rows can exceed `rowCount`. Worked example, sizes `[2, 2, 1]`: total 5, rowCount 2, base 2, remainder 1, first cap 3. Group 1 (2) fits. Group 2: 2+2 > 3, close row 1, rows.size = 1, cap = 2 + (1 < 1 ? 1 : 0) = 2. Group 2 added, filled 2. Group 3: 2+1 > 2, close row 2, cap = 2 + (2 < 1 ? ..) = 2. Group 3 added. Result: 3 rows, not 2.

Proposal: port the algorithm exactly (for parity and to keep the same layout the user sees), and add a unit test with this example asserting 3 rows. Track the quirk as an open decision in p1-edge-cases-and-tests.md (E-12).

### 18.3 Small group rendering

OBSERVED. For each packed row (lines 65-111): a `Row(fillMaxWidth)`. For each group:
- `startSlot = prefix sum of sizes before it`, `endSlot = startSlot + size - 1`.
- `Box(weight(size))` with alignment: `startSlot == 0 -> CenterStart`; `endSlot == 2 -> CenterEnd`; else `Center`.
- Inside: `Text(relation.name, titleSmall, maxLines = 1, ellipsis)`, spacer 4 dp, then `Row(spacedBy 4 dp)` with each anime in `Box(widthIn(max = 96.dp))` containing `RelatedAnimeItem`.
- Trailing `Spacer(weight(3 - filled))` when `filled < 3`.
- Row container: `Column(fillMaxWidth, padding h 8 v 8, spacedBy 8)`.

### 18.4 Multi groups

For each group: `Text(relation.name, titleSmall, padding start 8, end 8, top 8)`, then `LazyRow(contentPadding = 8 dp, spacedBy 4 dp)` with `items(relation.anime, key = { it.id })` each in `Box(width(96.dp))`.

### 18.5 Related item display

`RelatedAnimeItem(anime, displayMode, onClick, onLongClick)` (lines 147-189):
- `coverAlpha = if (anime.favorite) CommonEntryItemDefaults.BrowseFavoriteCoverAlpha (0.34) else 1f`.
- `ComfortableGrid` and `List`: `EntryComfortableGridItem(title, titleMaxLines = 3, coverData = anime.asAnimeCover(), coverAlpha, coverBadgeStart = { InLibraryBadge(enabled = anime.favorite) }, onClick, onLongClick)`.
- `CompactGrid`: `EntryCompactGridItem(title, coverData, coverAlpha, coverBadgeStart = InLibraryBadge, onClick, onLongClick)`.
- `CoverOnlyGrid`: `EntryCompactGridItem` without `title`.

Click: `onRelatedClick(anime)`; long press: `onRelatedLongClick(anime)` (opens the related entry; behaviour in part 2).

### 18.6 Data model

`AnimeRelationGroup` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeRelationGroup.kt`, outside scope): `name: String`, `anime: List<Anime>`. Full field list in p1-data-and-config.md.

## 19. Description and tags

### 19.1 Description text

OBSERVED. `ExpandableAnimeDescription` (`AnimeInfoHeader.kt:246-350`):
- `expanded` is `rememberSaveable { mutableStateOf(defaultExpandState) }` (survives configuration change).
- `desc = description.takeIf { !it.isNullOrBlank() } ?: "No description"` (`MR.strings.description_placeholder`).
- `trimmedDescription = desc.replace(Regex("[\\r\\n]{2,}", MULTILINE), "\n").trimEnd()`. Any run of two or more CR or LF characters becomes one LF. Single line breaks are kept. Trailing whitespace is removed.
- `AnimeSummary(expandedDescription = desc, shrunkDescription = trimmedDescription, expanded, relations, ...)` with modifier `padding(top = 8).padding(horizontal = 16).clickableNoIndication { onExpanded(!expanded) }`.

### 19.2 Collapse and expand

OBSERVED. `AnimeSummary` (lines 581-682) is a custom `Layout` with five children (`shrunk`, `expanded`, `actual`, `related`, `scrim`):
- `animProgress = animateFloatAsState(if (expanded) 1f else 0f)` (default spring spec).
- `shrunk` = `Text("\n\n", bodyMedium)`: measures three lines ("Shows at least 3 lines").
- `expanded` = `Text(expandedDescription, bodyMedium)`: measures full height.
- `actual` = `SelectionContainer { Text(if (expanded) expandedDescription else shrunkDescription, maxLines = Int.MAX_VALUE, color = onBackground, Modifier.secondaryItemAlpha()) }`.
- `related` = `Column { if relations not empty: RelatedAnimeRows }`.
- `scrim` = `Box(background = verticalGradient(Transparent, background), contentAlignment = Center)` with an `AnimatedImageVector` caret (`R.drawable.anim_caret_down`) in `rememberAnimatedVectorPainter(image, !expanded)`, and `contentDescription` `MR.strings.manga_info_collapse` "Less" when expanded else `manga_info_expand` "More". Background is a reversed radial gradient.
- Layout math: `shrunkHeight`, `expandedHeight`, `heightDelta = expandedHeight - shrunkHeight`, `scrimHeight = 24.dp` px. `descriptionHeight = shrunkHeight + round((heightDelta + scrimHeight) * progress)`. `relatedHeight = round(relatedHeight * progress)`. `bottomPadding = if relatedHeight > 0 then round(16.dp * progress) else 0`. Scrim is placed at `descriptionHeight + relatedHeight + bottomPadding - scrimHeight`.

Conflict (OBSERVED): the whole block is clickable to toggle, and the text inside is in `SelectionContainer`, so a click on selectable text both starts selection and toggles. Proposal: GTK `gtk::Label` with `selectable: true` and a separate "More/Less" button (no gesture on the text).

Collapsed description shows 3 lines of the trimmed text. Expanded shows all of it.

### 19.3 Tags (genres)

OBSERVED. Lines 284-347.
- `tagsProvider()` = `anime.genre` (list of strings). If null or empty, nothing is shown.
- Container `Box(padding top 8, vertical 12, animateContentSize(spring), fillMaxWidth)`.
- Collapsed: `LazyRow(contentPadding = horizontal 16 dp, spacedBy 4 dp)`.
- Expanded: `FlowRow(padding horizontal 16 dp, spacedBy 4 dp)`.
- Chip: `SuggestionChip(modifier = padding vertical 4, label = Text(text, bodySmall))` inside `LocalMinimumInteractiveComponentSize provides 0.dp`. Click sets `tagSelected = text` and `showMenu = true`.
- Menu (`DropdownMenu`): "Search" (`MR.strings.action_search`) calls `onTagSearch(tagSelected)`; "Copy to clipboard" (`MR.strings.action_copy_to_clipboard`) calls `onCopyTagToClipboard(tagSelected)`. Both close the menu.
- `onCopyTagToClipboard` in `AnimeScreen.kt:171-175` copies only if the string is non-empty, via `context.copyToClipboard(it, it)`.

Note: the Compose `FlowRow` (expanded) and `LazyRow` (collapsed) show the same chips; the toggle is the description, not the tags.

## 20. Top-level key behaviours that cross areas

- Back press, title tap and search: `doSearch` and `onTagSearch` are the global search entry points (area owned by part 2 or the browse area).
- `onFilterButtonClicked` and `ItemHeader.onClick` open the settings sheets. The sheet name is `episode_settings` or `season_settings` in the mockup spec (`spec/entries.json` `anime_entry_details.sheets`).
- Refresh: `onRefresh` from `PullRefresh`. Proposal: header refresh button and F5.
- Share: `onShareClicked` in the toolbar (outside scope).
