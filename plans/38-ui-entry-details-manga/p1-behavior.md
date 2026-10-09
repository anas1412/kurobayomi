# Area 38 part 1: Manga entry details screen, behavior spec

Scope: the 19 files listed in `p1-inventory.md`. All paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`).

Boundary with part 2 (`area:38-ui-entry-details-manga`, part 2): `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt` (1243 lines), `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackItem.kt`, and `app/src/main/java/eu/kanade/tachiyomi/util/chapter/*.kt`. This document specifies the **chapter-list state pipeline** (it lives in `State.Success` inside `MangaScreenModel.kt` and defines what the UI shows) because the UI cannot be implemented without it. Everything else in `MangaScreenModel.kt` (download orchestration, tracker sync, mark-read side effects, fetch from source, duplicate detection, migration) is only referenced here by callback name and is specified in part 2.

Legend: **OBSERVED** = what the Kotlin does (cited). **Proposal:** = what the Rust port should do.

---

## 1. Screen entry and loading gate

### 1.1 OBSERVED: ui-level screen

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt`

- Class `MangaScreen(private val mangaId: Long, val fromSource: Boolean = false) : Screen(), AssistContentScreen` (line 71-74).
- Line 82: `if (!ifMangaSourcesLoaded()) { LoadingScreen(); return }`. The screen shows a loading placeholder until the manga source registry is ready.
- Line 92-93: `rememberScreenModel { MangaScreenModel(context, lifecycleOwner.lifecycle, mangaId, fromSource) }`.
- Line 97-100: while `state is MangaScreenModel.State.Loading`, render `LoadingScreen()`.
- Line 103: `isHttpSource = remember { successState.source is HttpSource }`. This flag gates WebView, share, and copy-URL actions. Local and stub sources never get those actions.
- Line 105-115: `LaunchedEffect(successState.manga, screenModel.source)`; if HTTP source, `assistUrl = getMangaUrl(...)` on IO context; errors logged as `"Failed to get manga URL"` (logcat, no UI).
- Line 117-175: passes every callback to the presentation screen (see section 9 for the complete wiring table).
- Line 177-304: dialog switch on `successState.dialog` (see section 8).
- Line 297-304: scanlator dialog is a local `remember` boolean `showScanlatorsDialog`, not a ScreenModel dialog.

### 1.2 OBSERVED: presentation root

Source: `app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt`

- Line 88-134: `fun MangaScreen(state, snackbarHostState, nextUpdate, isTabletUi, chapterSwipeStartAction, chapterSwipeEndAction, ...)`.
- Line 136-141: `onCopyTagToClipboard` copies only when `it.isNotEmpty()`.
- Line 144-146: `onSettingsClicked` is non-null only when `state.source is ConfigurableSource`; click pushes `MangaSourcePreferencesScreen(state.source.id)`.
- Line 148: `if (!isTabletUi)` → `MangaScreenSmallImpl` else `MangaScreenLargeImpl`.

### 1.3 Proposal: Rust loading gate

- Show a spinner page while `MangaDetailsState::Loading`. The Rust equivalent of `ifMangaSourcesLoaded()` is "source registry (from area:17/18/19) has completed its first load". Until then, do not construct the details model.
- Route: `entry_details(mangaId, fromSource)`. Keep `fromSource` as a bool that is passed into the model and used for default description expansion (section 4.2).

---

## 2. Layout selection and responsive behavior

### 2.1 OBSERVED

- `isTabletUi` is computed in the UI-level screen with `isTabletUi()` from `eu.kanade.presentation.util` (imported at ui MangaScreen.kt line 42). Its definition is outside the scope (area:36-ui-shell-navigation-security-webview owns the window-size helpers).
- Small layout (`MangaScreenSmallImpl`, presentation MangaScreen.kt:224-470): one `Scaffold` with an `EntryToolbar` top bar, `EntryBottomActionMenu` bottom bar with `fillFraction = 1f`, a `SnackbarHost`, an `ExtendedFloatingActionButton`, a `PullRefresh`, a `VerticalFastScroller`, and one `LazyColumn` containing, in order: INFO_BOX, ACTION_ROW, DESCRIPTION_WITH_TAG, ITEM_HEADER, then the chapter rows.
- Large layout (`MangaScreenLargeImpl`, presentation MangaScreen.kt:473-716): same scaffold, but the bottom bar is wrapped in a `Box(contentAlignment = BottomEnd)` and uses `fillFraction = 0.5f`. The body is a `TwoPanelBox` where the start panel is a vertically scrollable `Column` containing MangaInfoBox (isTabletUi=true), MangaActionRow, and ExpandableMangaDescription (defaultExpandState=true). The end panel is a `VerticalFastScroller` + `LazyColumn` containing ITEM_HEADER and the chapter rows.
- Large layout top bar uses `titleAlphaProvider = { 1f }` and `backgroundAlphaProvider = { 1f }` (always opaque, title always shown). Small layout fades them (section 5.1).
- Large layout `PullRefresh` indicator padding uses start/end insets from `WindowInsets.systemBars.only(Horizontal)` and `top = topBarHeight` measured with `Modifier.onSizeChanged`.

### 2.2 Proposal

- Two layouts, selected by window width: a single column when the content area is narrower than the threshold that `isTabletUi()` uses (value owned by area:36; the port must reuse that exact threshold). Two-panel above it: left 1/2 of the width for info, right for the list. Implement as a `gtk::Paned` or a `gtk::Box` with fixed proportions chosen on `notify::default-width`; the GTK 4 `gtk::Stack` can switch layout.
- The model is shared between both layouts. Only the view tree differs. Do not duplicate state.

---

## 3. Info header (MangaInfoBox)

Source: `app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt`

### 3.1 Backdrop

- `MangaInfoBox` (line 102-160) draws the cover as a backdrop: `AsyncImage(model = ImageRequest.Builder(...).data(manga).crossfade(true))`, `ContentScale.Crop`, `matchParentSize()`, then `drawWithContent { drawContent(); drawRect(Brush.verticalGradient(listOf(Color.Transparent, MaterialTheme.colorScheme.background))) }`, `.blur(4.dp)`, `.alpha(0.2f)`.
- Foreground content is wrapped in `CompositionLocalProvider(LocalContentColor provides onSurface)`.
- `if (!isTabletUi)` → `MangaAndSourceTitlesSmall`, else `MangaAndSourceTitlesLarge`.

### 3.2 Small titles block (line 374-414)

- `Row`, padding `start = 16.dp, top = appBarPadding + 16.dp, end = 16.dp`, `horizontalArrangement = spacedBy(16.dp)`, `verticalAlignment = CenterVertically`.
- Cover: `ItemCover.Book` with `Modifier.sizeIn(maxWidth = 100.dp).align(Alignment.Top)`, contentDescription `MR.strings.manga_cover` ("Cover"), `onClick = onCoverClick`.
- Right column: `spacedBy(2.dp)` containing `MangaContentInfo(...)` with `doSearch` but no `textAlign` override.

### 3.3 Large titles block (line 336-371)

- `Column`, `fillMaxWidth`, padding `start = 16.dp, top = appBarPadding + 16.dp, end = 16.dp`, `horizontalAlignment = CenterHorizontally`.
- Cover: `ItemCover.Book(modifier = fillMaxWidth(0.65f))`.
- `Spacer(16.dp)`, then `MangaContentInfo(... textAlign = TextAlign.Center)`.

### 3.4 MangaContentInfo (line 417-560)

Rows in this order:

1. **Title**: `Text(text = title.ifBlank { "Unknown title" }, style = titleLarge)`.
   - `clickableNoIndication(onLongClick = copy title if not blank, onClick = doSearch(title, global = true) if not blank)`.
   - Note: title click performs a **global** search (`global = true`).
2. `Spacer(2.dp)`.
3. **Author row** (`secondaryItemAlpha`): icon `PersonOutline` 16dp, then text `author` if not blank else `"Unknown author"`, style `titleSmall`.
   - Click → `doSearch(author, true)` only when `!author.isNullOrBlank()`. Long-press → copy author when not blank.
4. **Artist row** (only if `!artist.isNullOrBlank() && author != artist`): icon `Brush` 16dp, text `artist`.
   - Click → `doSearch(artist, true)` (no blank check; artist already non-blank). Long-press → copy artist.
   - Note the comparison `author != artist` is a null-safe string equality. If artist equals author it is hidden.
5. `Spacer(2.dp)`.
6. **Status + source row** (`secondaryItemAlpha`, `CenterVertically`):
   - Icon by status (`status` is `Long`, compared to `SManga.*.toLong()`):
     - `1` ONGOING → `Icons.Outlined.Schedule`
     - `2` COMPLETED → `Icons.Outlined.DoneAll`
     - `3` LICENSED → `Icons.Outlined.AttachMoney`
     - `4` PUBLISHING_FINISHED → `Icons.Outlined.Done`
     - `5` CANCELLED → `Icons.Outlined.Close`
     - `6` ON_HIATUS → `Icons.Outlined.Pause`
     - else (including `0` UNKNOWN) → `Icons.Outlined.Block`
     - Size 16dp, `padding(end = 4.dp)`.
   - Text (style `bodyMedium`, `maxLines = 1`, ellipsis): `ongoing` "Ongoing", `completed` "Completed", `licensed` "Licensed", `publishing_finished` "Publishing finished", `cancelled` "Cancelled", `on_hiatus` "On hiatus", else `unknown` "Unknown".
   - `DotSeparatorText()` (" • ").
   - If `isStubSource`: `Icons.Filled.Warning` 16dp in `colorScheme.error`, `padding(end = 4.dp)`, placed before the source name.
   - Source name `Text(sourceName)`, `maxLines = 1`, ellipsis, click → `doSearch(sourceName, global = false)` (non-global).

Source constants: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54` (`UNKNOWN=0, ONGOING=1, COMPLETED=2, LICENSED=3, PUBLISHING_FINISHED=4, CANCELLED=5, ON_HIATUS=6`).

### 3.5 Search semantics (what `doSearch` does)

Implemented in `performSearch` (ui MangaScreen.kt:359-379), see section 10.

### 3.6 Proposal

- Title, author, artist, and source name are clickable labels. Click = search, long-press = copy (Linux: right-click or long-press via `gtk::GestureLongPress`; keyboard: focusable label with Enter = search, Shift+Enter = copy; not in Aniyomi, add).
- Copy action writes the text to the GDK clipboard (`gdk::Display::default().clipboard().set_text`). Show an in-window toast (section 13). Aniyomi does not show a toast in this code path (the `copyToClipboard` helper is used directly; see `eu.kanade.tachiyomi.util.system.copyToClipboard`, not read in this pass).

---

## 4. Description, tags, and summary expansion

### 4.1 Action row (MangaActionRow)

Source: `MangaInfoHeader.kt:163-234`

- `Row(padding start 16dp, top 8dp, end 16dp)`. Each button is `MangaActionButton` with `weight(1f)`, `TextButton` (Material3 TextButton from presentation-core), icon 20dp, 4dp gap, label `fontSize 12.sp`, centered.
- `defaultActionButtonColor = onSurface.copy(alpha = DISABLED_ALPHA)`. `DISABLED_ALPHA` is defined in `presentation-core` (not read).
- `nextUpdateDays = remember(nextUpdate) { if (nextUpdate != null) now.until(nextUpdate, ChronoUnit.DAYS).toInt().coerceAtLeast(0) else null }`. Note: `until` truncates toward zero, so a time 36 hours ahead gives 1, a time 23 hours ahead gives 0. Past times give negative and are coerced to 0.

Buttons, left to right:

1. **Library**
   - Title: `"In library"` if `favorite`, else `"Add to library"`.
   - Icon: `Icons.Filled.Favorite` (favorite) or `Icons.Outlined.FavoriteBorder`.
   - Color: `colorScheme.primary` if favorite, else disabled alpha.
   - `onClick = onAddToLibraryClicked`. `onLongClick = onEditCategory` (null when not favorite, so no long-press).
   - The UI-level `onAddToLibraryClicked` (ui MangaScreen.kt:127-130) calls `screenModel.toggleFavorite()` and then `haptic.performHapticFeedback(LongPress)`.
2. **Next update / interval**
   - Title: `null` → `"N/A"` (`not_applicable`); `0` → `"Soon"` (`manga_interval_expected_update_soon`); else `pluralStringResource(MR.plurals.day, n)` → `"1 day"` / `"%d days"`.
   - Icon: `Icons.Default.HourglassEmpty`. Color: primary if `isUserIntervalMode` (`manga.fetchInterval < 0`), else disabled.
   - `onClick = { onEditIntervalClicked?.invoke() }`. Null when not favorite, so no-op.
   - Source comment (MangaInfoHeader.kt:178): `// TODO: show something better when using custom interval`.
3. **Tracking**
   - Title: `trackingCount == 0` → `"Tracking"` (`manga_tracking_tab`); else plural `num_trackers` → `"1 tracker"` / `"%d trackers"`.
   - Icon: `Icons.Outlined.Sync` when 0, else `Icons.Outlined.Done`. Color: disabled when 0, primary otherwise.
   - `onClick = onTrackingClicked`.
4. **WebView** (only when `onWebViewClicked != null`, i.e. HTTP source)
   - Title: `"WebView"` (`action_web_view`, translatable=false). Icon: `Icons.Outlined.Public`. Disabled color.
   - `onClick = onWebViewClicked`, `onLongClick = onWebViewLongClicked` (copy URL).

Presentation-level gating (ui MangaScreen.kt, lines 145-166):
- Tracking: if `!successState.hasLoggedInTrackers` → `navigator.push(SettingsScreen(SettingsScreen.Destination.Tracking))`; else `screenModel.showTrackDialog()`.
- Edit category, fetch interval, migrate are only non-null when `successState.manga.favorite`.

### 4.2 ExpandableMangaDescription

Source: `MangaInfoHeader.kt:236-333`

- `defaultExpandState` is passed in: small layout = `state.isFromSource`; large layout = `true`.
- `expanded` is `rememberSaveable { mutableStateOf(defaultExpandState) }`. It persists across recomposition and configuration changes but not across app restarts.
- `desc = description.takeIf { !it.isNullOrBlank() } ?: stringResource(MR.strings.description_placeholder)` → `"No description"`.
- `trimmedDescription = desc.replace(whitespaceLineRegex, "\n").trimEnd()` where `whitespaceLineRegex = Regex("[\\r\\n]{2,}", setOf(RegexOption.MULTILINE))` (MangaInfoHeader.kt:99). This collapses any run of two or more CR/LF characters into a single `\n`. Single newlines are preserved.
- `MangaSummary(expandedDescription = desc, shrunkDescription = trimmedDescription, expanded = expanded, modifier = padding(top=8dp).padding(horizontal=16dp).clickableNoIndication { expanded = !expanded })`.

MangaSummary (line 563-640):

- Uses `androidx.compose.ui.layout.Layout` with four slot children:
  1. `"\n\n"` text (bodyMedium): forces the shrunk height to be at least 3 lines.
  2. `expandedDescription` text (bodyMedium): measured to get `expandedHeight`.
  3. `SelectionContainer { Text(if (expanded) expandedDescription else shrunkDescription, maxLines = Int.MAX_VALUE, color = onBackground, modifier = secondaryItemAlpha()) }` (the actual visible text; selectable).
  4. Scrim: `Box(background(Brush.verticalGradient(listOf(Transparent, background))), contentAlignment = Center)` containing an `Icon` drawn from `R.drawable.anim_caret_down` as an animated vector (`rememberAnimatedVectorPainter(image, !expanded)`), with `contentDescription` `"Less"` (`manga_info_collapse`) if expanded else `"More"` (`manga_info_expand`), and a radial-gradient background.
- `animProgress = animateFloatAsState(if (expanded) 1f else 0f, label = "summary")`.
- Layout math: `shrunkHeight` = measured height of slot 1 at `constraints`; `expandedHeight` = slot 2 height; `heightDelta = expandedHeight - shrunkHeight`; `scrimHeight = 24.dp.roundToPx()`; `currentHeight = shrunkHeight + ((heightDelta + scrimHeight) * animProgress).roundToInt()`. The scrim is placed at `y = currentHeight - scrimHeight`, width = `constraints.maxWidth`, height fixed 24dp.
- The whole thing is `clipToBounds()`.

Tags (ExpandableMangaDescription, MangaInfoHeader.kt:267-331):

- `tags = tagsProvider()` = `manga.genre` (List<String>?). Rendered only when `!tags.isNullOrEmpty()`.
- Container: `Box(padding(top = 8.dp).padding(vertical = 12.dp).animateContentSize(spring()).fillMaxWidth())`.
- Each tag is `TagsChip` = `SuggestionChip(label = Text(bodySmall))` inside `LocalMinimumInteractiveComponentSize provides 0.dp`, `modifier = padding(vertical = 4.dp)`.
- Chip click: sets `tagSelected = it` and `showMenu = true`. A `DropdownMenu` (shared presentation component `eu.kanade.presentation.components.DropdownMenu`) shows two items:
  - `"Search"` (`action_search`) → `onTagSearch(tagSelected)` then closes.
  - `"Copy to clipboard"` (`action_copy_to_clipboard`) → `onCopyTagToClipboard(tagSelected)` then closes.
- Collapsed (`expanded == false`): `LazyRow(contentPadding = horizontal MaterialTheme.padding.medium, spacedBy(MaterialTheme.padding.extraSmall))`.
- Expanded: `FlowRow(padding horizontal 16.dp, spacedBy(extraSmall))`.
- `MaterialTheme.padding.*` values are defined in presentation-core (not read). Use the same dp values when porting, after checking the presentation-core source in the next pass.

### 4.3 Proposal

- Description: `gtk::Label` with `lines`, `ellipsize`, and `wrap`. Expand/collapse toggles `set_lines(-1)` / `set_lines(3)` as the mockup already does (`src/entry.rs` lines 130-141). Replace the mockup's `set_ellipsize(EllipsizeMode::None)` pairing with the same toggle. Use a transition of 200 ms height (`gtk::Revealer`) to match the Compose spring. The 24 px gradient scrim is drawn with a `gtk::Overlay` child that has a CSS `linear-gradient` background from transparent to the window background color (using the theme's `@window_bg_color` CSS named color so the user's GTK theme applies).
- Keep the regex `[\r\n]{2,}` → `\n` and `trim_end()`. Use the `regex` crate with the same pattern.
- Tags: `gtk::FlowBox` (expanded) or horizontal `gtk::ScrolledWindow` with `gtk::Box` (collapsed). Chips are `gtk::Button` with CSS class `pill`. Click shows a `gtk::PopoverMenu` with "Search" and "Copy to clipboard".

---

## 5. Top toolbar (EntryToolbar) and title/background fading

### 5.1 Small layout fading

Source: presentation MangaScreen.kt:290-328

- `isFirstItemVisible = chapterListState.firstVisibleItemIndex == 0` (derivedStateOf).
- `isFirstItemScrolled = chapterListState.firstVisibleItemScrollOffset > 0`.
- `titleAlpha = animateFloatAsState(if (!isFirstItemVisible) 1f else 0f, label = "Top Bar Title")`. The title appears only after the info box has scrolled off-screen.
- `backgroundAlpha = animateFloatAsState(if (!isFirstItemVisible || isFirstItemScrolled) 1f else 0f, label = "Top Bar Background")`.
- The toolbar background color is `surfaceColorAtElevation(3.dp).copy(alpha = if (isActionMode) 1f else backgroundAlpha)` (EntryToolbar.kt:63-66).

### 5.2 EntryToolbar

Source: `app/src/main/java/eu/kanade/presentation/entries/components/EntryToolbar.kt`

- Title area: in action mode (`actionModeCounter > 0`) shows the count as text (`actionModeCounter.toString()`); otherwise shows `title` with `alpha(titleAlphaProvider())`.
- Actions (EntryToolbar.kt:68-166), in this exact order:
  - (only if `onClickDownload != null`) a download icon button that opens `EntryDownloadDropdownMenu` (shared, see section 7.6) — shown in the bar with title `manga_download` ("Download"), icon `Icons.Outlined.Download`. Note: the dropdown anchor is the download action, and `EntryDownloadDropdownMenu` is rendered inside `actions` before the AppBarActions.
  - Filter action: title `action_filter` ("Filter"), icon `Icons.Outlined.FilterList`, `iconTint = filterTint` where `filterTint = colorScheme.active if hasFilters else LocalContentColor.current`. Click → `onClickFilter`.
  - Overflow (`AppBar.OverflowAction`) entries, in this order, each only when its callback is non-null:
    1. `changeAnimeSkipIntro` (anime only; always null for manga).
    2. `action_webview_refresh` "Refresh" → `onClickRefresh` (always present for manga).
    3. `action_edit_categories` "Edit categories" → `onClickEditCategory` (favorite only).
    4. `action_migrate` "Migrate" → `onClickMigrate` (favorite only).
    5. `action_share` "Share" → `onClickShare` (HTTP source only).
    6. `AYMR.strings.settings` "Settings" → `onClickSettings` (ConfigurableSource only).
- Action mode actions (EntryToolbar.kt:83-98): when `isActionMode`, the list is exactly two items: `action_select_all` "Select all" (`Icons.Outlined.SelectAll`) → `onSelectAll`, then `action_select_inverse` "Select inverse" (`Icons.Outlined.FlipToBack`) → `onInvertSelection`. The filter, download, and overflow are not shown in action mode.
- `isActionMode` is passed to `AppBar(isActionMode = ..., onCancelActionMode = onCancelActionMode)`. Presumably the AppBar shows the close affordance; `AppBar` is in presentation-core (not read).
- Back/cancel: `BackHandler` in each layout (presentation MangaScreen.kt:282-288 and 537-543): if `isAnySelected` → `onAllChapterSelected(false)`, else `navigateUp()`.

### 5.3 Proposal

- Top bar: `gtk::HeaderBar`-free custom bar (no libadwaita). Use a `gtk::Box` with `gtk::Label` title and `gtk::MenuButton`. Use the window's own titlebar (`gtk::Window::set_titlebar`) where the window manager gives it; the bar must still collapse when the page is scrolled as above. Implement the fade with `widget.set_opacity()` driven by `gtk::Adjustment` value, interpolated linearly over 150 ms to approximate `animateFloatAsState`'s default spring.
- Overflow menu: `gtk::PopoverMenu` built from `gio::Menu` with actions `entry.refresh`, `entry.edit-categories`, `entry.migrate`, `entry.share`, `entry.settings`. Only add menu items for which the callback exists (same gating as above).
- Keyboard: Escape in action mode clears selection; Escape otherwise pops (matches BackHandler). `Ctrl+A` = select all (action mode only; proposal). `Ctrl+I` = invert (proposal).

---

## 6. Chapter list state pipeline (owned by ScreenModel, specified here because the UI depends on it)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt`

### 6.1 State shape

- `sealed class State` has `Loading` (object) and `Success` (data class). `Success` fields (lines 1144-1155, read in full): `manga: Manga`, `source: MangaSource`, `isFromSource: Boolean`, `chapters: List<ChapterList.Item>`, `availableScanlators: Set<String>`, `excludedScanlators: Set<String>`, `trackingCount: Int = 0`, `hasLoggedInTrackers: Boolean = false`, `isRefreshingData: Boolean = false`, `dialog: Dialog? = null`, `hasPromptedToAddBefore: Boolean = false`.
- Derived (lazy) properties:
  - `processedChapters` = `chapters.applyFilters(manga).toList()` (line 1150-1152).
  - `isAnySelected` = `chapters.fastAny { it.selected }` (of the **unprocessed** list, line 1155-1157).
  - `chapterListItems` = `processedChapters.insertSeparators { before, after -> ... }` (section 6.4).
  - `scanlatorFilterActive` = `excludedScanlators.intersect(availableScanlators).isNotEmpty()` (line 1190-1191). Note the intersection: an excluded scanlator that no longer exists in the available set does not make the filter "active".
  - `filterActive` = `scanlatorFilterActive || manga.chaptersFiltered()` (line 1193-1194).
  - `trackingAvailable` = `trackingCount > 0`.

### 6.2 Filter and sort pipeline (applyFilters, lines 1196-1222)

Order of application is exact:

1. `unreadFilter = manga.unreadFilter`, `downloadedFilter = manga.downloadedFilter`, `bookmarkedFilter = manga.bookmarkedFilter`, `isLocalManga = manga.isLocal()`.
2. `.filter { applyFilter(unreadFilter) { !chapter.read } }`
3. `.filter { applyFilter(bookmarkedFilter) { chapter.bookmark } }`
4. `.filter { applyFilter(downloadedFilter) { it.isDownloaded || isLocalManga } }`
5. `.sortedWith { c1, c2 -> getChapterSort(manga).invoke(c1.chapter, c2.chapter) }`

`applyFilter` (domain: `domain/src/main/java/tachiyomi/domain/entries/TriState.kt:5-9`):
- `DISABLED` → `true` (keep).
- `ENABLED_IS` → `predicate()`.
- `ENABLED_NOT` → `!predicate()`.

Note that the **downloaded** predicate also keeps every chapter of a local manga (`isLocalManga`), because local chapters are always "downloaded".

Note: the filters are applied to `chapters` produced by `getMangaAndChapters.subscribe(mangaId, applyScanlatorFilter = true)` (subscription at MangaScreenModel.kt:182-188; first snapshot at line 217), which already removes excluded-scanlator chapters at the repo level. The scanlator exclusion is therefore not re-applied here.

Note: `sortedWith` is stable (Kotlin `sortedWith` on a List is stable), so chapters comparing equal retain the order from the DB subscription.

### 6.3 Sort comparator (getChapterSort)

Source: `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt:7-33`. Call is `getChapterSort(manga)` with default `sortDescending = manga.sortDescending()`.

| `manga.sorting` | `sortDescending == true` | `sortDescending == false` |
|---|---|---|
| `CHAPTER_SORTING_SOURCE` (0x000) | `c1.sourceOrder.compareTo(c2.sourceOrder)` (ascending by sourceOrder) | `c2.sourceOrder.compareTo(c1.sourceOrder)` (descending by sourceOrder) |
| `CHAPTER_SORTING_NUMBER` (0x100) | `c2.chapterNumber.compareTo(c1.chapterNumber)` | `c1.chapterNumber.compareTo(c2.chapterNumber)` |
| `CHAPTER_SORTING_UPLOAD_DATE` (0x200) | `c2.dateUpload.compareTo(c1.dateUpload)` | `c1.dateUpload.compareTo(c2.dateUpload)` |
| `CHAPTER_SORTING_ALPHABET` (0x300) | `c2.name.compareToWithCollator(c1.name)` | `c1.name.compareToWithCollator(c2.name)` |
| anything else | throws `NotImplementedError("Invalid chapter sorting method: ...")` | same |

Important: the naming is inverted relative to intuition. `sortDescending == true` means the Kotlin code produces **ascending** source order (the "descending" flag is for the chapter sort direction as the user sees the list; for the SOURCE branch the list is intentionally produced in the order the source gave it, which is newest first, as shown in section 6.5). Port the table literally; do not "fix" it.

`compareToWithCollator` (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:13`): `java.text.Collator.getInstance(Locale.getDefault())` with `strength = Collator.PRIMARY` (lazy singleton, lines 6-11). PRIMARY strength ignores case and accents.

`sourceOrder` is `Chapter.sourceOrder: Long` (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt`).

Chapter number is `Double` (`Chapter.chapterNumber`). `-1.0` means "not recognized" (see `isRecognizedNumber` = `chapterNumber >= 0f`, Chapter.kt:19-20).

### 6.4 Missing-chapter separators (chapterListItems, MangaScreenModel.kt:1164-1189)

`processedChapters.insertSeparators { before, after -> ... }`. For each adjacent pair, the separator is computed:

1. `(lowerChapter, higherChapter) = if (manga.sortDescending()) (after to before) else (before to after)`.
2. If `higherChapter == null` → no separator (this is the boundary after the last item).
3. If `lowerChapter == null` → `missingCount = floor(higherChapter.chapterNumber).toInt() - 1` coerced to `>= 0`. (Chapters below the first visible one.)
4. Otherwise `missingCount = calculateChapterGap(higherChapter.chapter, lowerChapter.chapter)`.
5. Emit `ChapterList.MissingCount(id = "${lowerChapter?.id}-${higherChapter.id}", count = missingCount)` only when `missingCount > 0`.

`calculateChapterGap(higher: Chapter?, lower: Chapter?)` (`domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt:40-43`): returns 0 if either is null or either `!isRecognizedNumber`; else `calculateChapterGap(higher.chapterNumber, lower.chapterNumber)` (line 46-49) which returns `0` if either < 0, else `floor(higher).toInt() - floor(lower).toInt() - 1`.

Note the `id` string uses `lowerChapter?.id` which prints `null` when lower is null. This id is unique per adjacent pair and is used as the LazyColumn key (`"missing-count-${item.id}"`, presentation MangaScreen.kt:776).

### 6.5 Missing count shown in the item header

`ItemHeader`'s `missingItemsCount` (presentation MangaScreen.kt:443-445, 687-689) is computed independently:

- `chapters.map { it.chapter.chapterNumber }.missingChaptersCount()` over the **processed** list.
- `List<Double>.missingChaptersCount()` (`domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt:6-38`): empty → 0; drop values equal to `-1.0`; `toInt()` each (truncation, so 16.5 → 16); `distinct()`; `sorted()` ascending; then, starting with `previousChapter = 0`, for each value `c`: if `c > previousChapter + 1` add `c - previousChapter - 1`; set `previousChapter = c`. Result is the count of integer chapter numbers absent between 1 and the highest value.
- Note: this uses the processed (filtered) list, so filters change the missing count. Preserve this.

The header text is `pluralStringResource(AYMR.plurals.missing_items, count)` → `"Missing 1 item"` / `"Missing %1$s items"`, style bodySmall, error color at `SECONDARY_ALPHA`, shown only if count != 0 (ItemHeader.kt:65-78).

### 6.6 Selection state

Selection state lives in the ScreenModel, not in the UI (`toggleSelection` at MangaScreenModel.kt:971, `toggleAllSelection` at 1035, `invertSelection` at 1047). Part 2 owns the internals. What the UI relies on:

- `ChapterList.Item.selected: Boolean` is set per processed item.
- `isAnySelected` is computed from the **unprocessed** list.
- The UI calls `onChapterSelected(item, selected, userSelected, fromLongPress)`:
  - Long-press on a row → `onChapterSelected(item, !item.selected, true, true)` (presentation MangaScreen.kt:816-818).
  - Click while any selected → `onChapterSelected(item, !item.selected, true, false)` (line 824).
  - `onAllChapterSelected(true|false)`: select all / clear.
  - `onInvertSelection()`.
- Range selection on long-press: part 2 (`selectedPositions`, MangaScreenModel.kt:153 and the range logic at 985-995).

### 6.7 Chapter click (presentation MangaScreen.kt:842-853)

```
when {
    chapterItem.selected -> onToggleSelection(false)     // deselect
    isAnyChapterSelected -> onToggleSelection(true)      // extend selection
    else -> onChapterClicked(chapterItem.chapter)        // open reader
}
```

`onChapterClicked` → ui MangaScreen.kt:125 → `openChapter(context, it)` → `context.startActivity(ReaderActivity.newIntent(context, chapter.mangaId, chapter.id))` (ui MangaScreen.kt:311-313). Proposal: open the reader route for `(mangaId, chapterId)` (area:36 / reader area).

### 6.8 Resume and Start

- `getNextUnreadChapter()` (MangaScreenModel.kt:648-665) returns the first unread chapter from the sorted list, honouring `skipFiltered` (reader preference `skip_filtered`, default `true`, `ReaderPreferences.kt:85`). Part 2 owns this function; the UI only calls it.
- UI `onContinueReading = { continueReading(context, screenModel.getNextUnreadChapter()) }` (ui MangaScreen.kt:155). `continueReading` does nothing when the result is null (ui MangaScreen.kt:307-309).
- FAB label (presentation MangaScreen.kt:355-361 small, 602-611 large): `if (state.chapters.any { it.chapter.read }) "Resume" else "Start"` — note this uses the **unprocessed** `state.chapters`.
- FAB visibility (presentation MangaScreen.kt:346-348 small, 593-595 large): `processedChapters.any { !it.chapter.read } && !isAnySelected`. Note the mixed use: visibility uses processed, label uses unprocessed.
- FAB is an `AnimatedVisibility(enter = fadeIn(), exit = fadeOut())`. `ExtendedFloatingActionButton(expanded = chapterListState.shouldExpandFAB())`. `shouldExpandFAB()` is in presentation-core (not read).
- Icon: `Icons.Filled.PlayArrow`, `contentDescription = null`.

---

## 7. Chapter row (MangaChapterListItem) and related components

### 7.1 Row content

Source: `app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaChapterListItem.kt`

Call site (presentation MangaScreen.kt:786-836):

- **Title**: if `manga.displayMode == Manga.CHAPTER_DISPLAY_NUMBER` (`0x00100000`) → `stringResource(MR.strings.display_mode_chapter, formatChapterNumber(chapterNumber))` → `"Chapter %1$s"`. Else `chapter.name`.
- **Date**: `relativeDateTimeText(chapter.dateUpload)` (`app/src/main/java/eu/kanade/presentation/components/DateText.kt:32-41`). Takes a millisecond epoch; `takeIf { dateEpochMillis > 0L }` produces `null` for zero dates, which renders through `toRelativeString(null)`; the output for that case is defined by `eu.kanade.tachiyomi.util.lang` (not read in this pass; see area:36 / area:37 for the format rules).
- **Read progress**: `lastPageRead.takeIf { !read && it > 0L }?.let { stringResource(MR.strings.chapter_progress, it + 1) }`. Message `"Page: %1$d"`. Note the `+ 1`: `lastPageRead` is a 0-based index.
- **Scanlator**: `chapter.scanlator.takeIf { !it.isNullOrBlank() }`.
- **read**, **bookmark**, **selected**: direct booleans.
- **downloadIndicatorEnabled** = `!isAnyChapterSelected && !manga.isLocal()`.
- `downloadStateProvider = { item.downloadState }`, `downloadProgressProvider = { item.downloadProgress }`.

Layout (MangaChapterListItem.kt:86-184):

- Outer: `SwipeableActionsBox(modifier = clipToBounds(), startActions = listOfNotNull(start), endActions = listOfNotNull(end), swipeThreshold = 56.dp, backgroundUntilSwipeThreshold = surfaceContainerLowest)`.
- Row: `selectedBackground(selected)`, `combinedClickable(onClick, onLongClick)`, padding `start 16dp, top 12dp, end 8dp, bottom 12dp`.
- Left column (`weight(1f)`, `spacedBy(6.dp)`):
  - Line 1 (`spacedBy(2.dp)`, `CenterVertically`):
    - If `!read`: `Icons.Filled.Circle` of height 8dp, `padding(end = 4.dp)`, tint `colorScheme.primary`, contentDescription `MR.strings.unread` ("Unread").
    - If `bookmark`: `Icons.Filled.Bookmark` with `sizeIn(maxHeight = textHeight - 2.dp)` (textHeight measured via `onTextLayout`), tint primary, contentDescription `"Bookmarked"`.
    - Title: `bodyMedium`, `maxLines = 1`, ellipsis, color = `LocalContentColor.current.copy(alpha = if (read) DISABLED_ALPHA else 1f)`.
  - Line 2 (`Row`): `ProvideTextStyle(bodySmall.merge(color = alpha(read ? DISABLED : SECONDARY)))`:
    - date (if non-null) → `DotSeparatorText()` if readProgress or scanlator non-null.
    - readProgress (if non-null, color alpha DISABLED) → `DotSeparatorText()` if scanlator non-null.
    - scanlator (if non-null).
    - Each is `maxLines = 1`, ellipsis.
- Right: `ChapterDownloadIndicator(enabled = downloadIndicatorEnabled, modifier = padding(start = 4.dp), ...)`, `onClick = { onDownloadClick?.invoke(it) }`.

Presentation-level constants: `swipeActionThreshold = 56.dp` (MangaChapterListItem.kt:242).

### 7.2 Swipe actions

Source: MangaChapterListItem.kt:187-240.

- Start and end each resolve via `getSwipeAction(action, read, bookmark, downloadState, background = primaryContainer, onSwipe)`.
- `ToggleRead`:
  - icon `Icons.Outlined.Done` if `!read`, else `Icons.Outlined.RemoveDone`.
  - `isUndo = read`.
- `ToggleBookmark`:
  - icon `Icons.Outlined.BookmarkAdd` if `!bookmark`, else `Icons.Outlined.BookmarkRemove`.
  - `isUndo = bookmark`.
- `Download`:
  - `NOT_DOWNLOADED`, `ERROR` → `Icons.Outlined.Download`.
  - `QUEUE`, `DOWNLOADING` → `Icons.Outlined.FileDownloadOff`.
  - `DOWNLOADED` → `Icons.Outlined.Delete`.
  - `isUndo = false`.
- `Disabled` → `null` (no action on that side).
- Each swipe action icon is `Modifier.padding(16.dp)`, tint `contentColorFor(background)`.
- `onSwipe` for start calls `onChapterSwipe(chapterSwipeStartAction)`, for end `onChapterSwipe(chapterSwipeEndAction)` (lines 75 and 83). The item is passed with the action to `screenModel.chapterSwipe(item, action)` (ui MangaScreen.kt:171, MangaScreenModel.kt:605).

Pref wiring (cross-wired names, verified):
- `LibraryPreferences.swipeChapterStartAction()` reads key `pref_chapter_swipe_end_action`, default `ToggleRead` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:369-370`).
- `LibraryPreferences.swipeChapterEndAction()` reads key `pref_chapter_swipe_start_action`, default `ToggleBookmark` (lines 372-374).
- `MangaScreenModel.kt:144-145`: `chapterSwipeStartAction = libraryPreferences.swipeChapterEndAction().get()` and `chapterSwipeEndAction = libraryPreferences.swipeChapterStartAction().get()`.
- Net effect: the **start** (left-to-right) swipe uses key `pref_chapter_swipe_start_action` (default `ToggleBookmark`) and the **end** swipe uses key `pref_chapter_swipe_end_action` (default `ToggleRead`). Port the net effect and use the correct key names; do not replicate the swapped getter names.

Enum values (`LibraryPreferences.kt:391-396`): `ToggleRead`, `ToggleBookmark`, `Download`, `Disabled`.

### 7.3 Long-press and click

Described in section 6.6 and 6.7.

### 7.4 MissingItemCountListItem

Source: `app/src/main/java/eu/kanade/presentation/entries/components/MissingItemCountListItem.kt`

- Row padding `horizontal = MaterialTheme.padding.medium`, `vertical = MaterialTheme.padding.small`, `secondaryItemAlpha()`, `spacedBy(MaterialTheme.padding.medium)`, `CenterVertically`.
- Two `HorizontalDivider(weight(1f))` with the text between.
- Text: `pluralStringResource(AYMR.plurals.missing_items, count, count)`, `labelMedium`.
- Rendered as a LazyColumn item with key `"missing-count-${id}"` and `contentType = EntryScreenItem.ITEM`.
- Not clickable, not selectable.

### 7.5 ChapterDownloadIndicator (state machine)

Source: `app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt`

Enum `ChapterDownloadAction { START, START_NOW, CANCEL, DELETE }` (lines 42-47).

Dispatch by `downloadStateProvider()` (lines 57-80):

| State (`MangaDownload.State`, value) | Composable | Visual |
|---|---|---|
| `NOT_DOWNLOADED` (0) | `NotDownloadedIndicator` | `R.drawable.ic_download_item_24dp`, contentDescription `"Download"` (`manga_download`), tint `onSurfaceVariant`, size `IndicatorSize`, wrapped in `secondaryItemAlpha()`. |
| `QUEUE` (1) or `DOWNLOADING` (2) | `DownloadingIndicator` | Ring + arrow (see below). |
| `DOWNLOADED` (3) | `DownloadedIndicator` | `Icons.Filled.CheckCircle`, tint `onSurfaceVariant`, size `IndicatorSize`. |
| `ERROR` (4) | `ErrorIndicator` | `Icons.Outlined.ErrorOutline`, contentDescription `"Error"` (`download_error`), tint `colorScheme.error`. |

Container for all: `Box(modifier.size(IconButtonTokens.StateLayerSize).commonClickable(enabled, hapticFeedback, onLongClick, onClick), contentAlignment = Center)`.

Click semantics:

- `NOT_DOWNLOADED`: click → `START`; long-press → `START_NOW`.
- `DownloadingIndicator`: click → opens a menu (`isMenuExpanded = true`); long-press → `CANCEL` immediately (no menu). Menu items: `"Start downloading now"` (`action_start_downloading_now`) → `START_NOW`; `"Cancel"` (`action_cancel`) → `CANCEL`. Menu closes after selection.
- `DownloadedIndicator`: click and long-press both open a menu with one item `"Delete"` (`action_delete`) → `DELETE`.
- `ErrorIndicator`: click and long-press → `START` (retry).

Progress visuals (`DownloadingIndicator`, lines 110-188):

- `downloadProgress = downloadProgressProvider()` (Int 0-100 in the UI's contract; `MangaDownload.totalProgress`).
- `indeterminate = downloadState == QUEUE || (downloadState == DOWNLOADING && downloadProgress == 0)`.
- Indeterminate: `CircularProgressIndicator(modifier = IndicatorModifier, color = strokeColor, strokeWidth = IndicatorStrokeWidth, trackColor = Transparent, strokeCap = Butt)`, arrow color = `strokeColor`.
- Determinate: `animatedProgress by animateFloatAsState(downloadProgress / 100f, animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec)`. `CircularProgressIndicator(progress = { animatedProgress }, strokeWidth = IndicatorSize / 2, gapSize = 0.dp, trackColor = Transparent, strokeCap = Butt)`. Arrow color is `strokeColor` while `animatedProgress < 0.5f`, else `colorScheme.background` (so the arrow stays visible on the filled ring).
- Arrow: `Icons.Outlined.ArrowDownward`, `ArrowModifier`.

Constants come from `eu.kanade.presentation.components` (`IndicatorSize`, `IndicatorStrokeWidth`, `IndicatorModifier`, `ArrowModifier`, `commonClickable`), defined outside scope (not read in this pass). `IconButtonTokens.StateLayerSize` is from presentation-core.

Download state values: `MangaDownload.State` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt:67-73`): `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)`.

Where the action is executed: `runChapterDownloadActions(items, action)` (MangaScreenModel.kt:696-719). Part 2.

### 7.6 Shared download dropdown (EntryDownloadDropdownMenu)

Used by the toolbar (`EntryToolbar.kt:70-78`) and by `LibraryBottomActionMenu` (`EntryBottomActionMenu.kt:359-375`). The composable is in `app/src/main/java/eu/kanade/presentation/components/` (not in scope; area:36 owns generic components). Its menu items are the `DownloadAction` enum values: `NEXT_1_ITEM`, `NEXT_5_ITEMS`, `NEXT_10_ITEMS`, `NEXT_25_ITEMS`, `UNVIEWED_ITEMS` (`EntryScreenConstants.kt:3-9`). In the toolbar the callback is `onClickDownload: ((DownloadAction) -> Unit)?`, and `screenModel.runDownloadAction(action)` (MangaScreenModel.kt:721) maps each enum to the next N unread chapters or all unread chapters (part 2).

Strings (from the mockup and the `isManga` branch of the Compose source): the mockup's `entry()` builds these labels as `"Next chapter"`, `"Next 5 chapters"`, `"Next 10 chapters"`, `"Next 25 chapters"`, `"Unread"`. The exact Compose labels come from `EntryDownloadDropdownMenu`, which is out of scope; treat the mockup labels as provisional until area:36 confirms them.

### 7.7 Bottom action bar (EntryBottomActionMenu, manga branch)

Source: `app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt`

- `EntryBottomActionMenu(visible, isManga = true, ...)` (lines 72-243):
  - `AnimatedVisibility(visible, enter = expandVertically(expandFrom = Bottom), exit = shrinkVertically(shrinkTowards = Bottom))`.
  - Visible in the page when `selected.isNotEmpty()` (`SharedMangaBottomActionMenu` line 730).
  - `Surface(modifier, shape = MaterialTheme.shapes.large with bottomStart/bottomEnd = 0, color = surfaceContainerHigh)`.
  - Row: `padding(WindowInsets.navigationBars.only(Bottom))`, then `padding(horizontal = 8dp, vertical = 12dp)`.
- Buttons, in order, each only if its callback is non-null (manga cases):
  1. Bookmark: `MR.strings.action_bookmark` ("Bookmark chapter"), `Icons.Outlined.BookmarkAdd`, `onLongClick` index 0. Shown when `selected.any { !bookmark }`. Click → `onMultiBookmarkClicked(selectedChapters, true)`.
  2. Unbookmark: `MR.strings.action_remove_bookmark` ("Unbookmark chapter"), `Icons.Outlined.BookmarkRemove`, index 1. Shown when `selected.all { bookmark }`. Click → `(…, false)`.
  3. Mark as read: `MR.strings.action_mark_as_read` ("Mark as read"), `Icons.Outlined.DoneAll`, index 4. Shown when `selected.any { !read }`. Click → `onMultiMarkAsReadClicked(…, true)`.
  4. Mark as unread: `MR.strings.action_mark_as_unread` ("Mark as unread"), `Icons.Outlined.RemoveDone`, index 5. Shown when `selected.any { read || lastPageRead > 0L }`. Click → `(…, false)`.
  5. Mark previous as read: `MR.strings.action_mark_previous_as_read` ("Mark previous as read"), `ImageVector.vectorResource(R.drawable.ic_done_prev_24dp)`, index 6. Shown only when `selected.size == 1`. Click → `onMarkPreviousAsReadClicked(selected[0].chapter)`.
  6. Download: `MR.strings.action_download` ("Download"), `Icons.Outlined.Download`, index 7. Shown when `onDownloadChapter != null && selected.any { downloadState != DOWNLOADED }`. Click → `onDownloadChapter(selected.toList(), ChapterDownloadAction.START)`.
  7. Delete: `MR.strings.action_delete` ("Delete"), `Icons.Outlined.Delete`, index 8. Shown when `selected.any { downloadState == DOWNLOADED }`. Click → `onMultiDeleteClicked(selected.map { chapter })`.
- Not shown for manga: fillermark (indices 2, 3), external and internal play (indices 9, 10); those are anime-only.
- Each `Button`: `Column(size 48dp, weight(animatedWeight), combinedClickable(onClick, onLongClick))`, `animatedWeight = if (toConfirm) 2f else 1f`, icon, and the title text appears below the icon only while `toConfirm` (`expandVertically` + `fadeIn`). Label `labelSmall`, `maxLines = 1`.
- **Long-press behaviour**: `onLongClickItem(i)` sets `confirm[i] = true` for that index and all others to false, plays a LongPress haptic, cancels the previous `resetJob` and launches a new job that after `1.seconds` sets `confirm[i] = false` if still active. The click action **runs immediately** on normal tap; long-press only reveals the label for one second. There is no confirmation step. See edge case E-12 in `p1-edge-cases-and-tests.md`.
- `confirm` is `mutableStateListOf(false x 11)` for the single-entry bar and `(false x 5)` for the library bar. `confirmRange = 0..<11`.
- The `resetJob` is declared as `var resetJob: Job? = remember { null }` (line 109), so it is recreated as null on each recomposition; cancellation of the previous job therefore does not work reliably (see E-12).

### 7.8 Library bottom bar (LibraryBottomActionMenu)

Not part of the entry details screen. It is in the same file (lines 291-387) and is used by the library screen (area:37). Document only that it exists: five buttons "Set categories" (`action_move_category`, `Icons.AutoMirrored.Outlined.Label`), "Mark as read", "Mark as unread", "Download" (dropdown `EntryDownloadDropdownMenu`), "Delete". Its visibility uses `AnimatedVisibility(visible, enter = expandVertically(animationSpec = tween(delayMillis = 300)), exit = shrinkVertically(animationSpec = tween()))`. Its download button opens `EntryDownloadDropdownMenu` instead of acting directly.

---

## 8. Dialogs and sheets

### 8.1 Dispatch (ui MangaScreen.kt:186-295)

`when (val dialog = successState.dialog)`:

- `null` → nothing.
- `Dialog.ChangeCategory` → `ChangeCategoryDialog(initialSelection, onDismissRequest, onEditCategories = { navigator.push(CategoriesTab); CategoriesTab.showMangaCategory() }, onConfirm = { include, _ -> screenModel.moveMangaToCategoriesAndAddToLibrary(dialog.manga, include) })`.
- `Dialog.DeleteChapters` → `DeleteItemsDialog(isManga = true, onConfirm = { toggleAllSelection(false); deleteChapters(dialog.chapters) })`.
- `Dialog.DuplicateManga` → `DuplicateMangaDialog` (section 8.4).
- `Dialog.Migrate` → `MigrateMangaDialog(oldManga, newManga, screenModel = MigrateMangaDialogScreenModel(), onClickTitle = push MangaScreen(oldManga.id), onPopScreen = replace MangaScreen(newManga.id))`.
- `Dialog.SettingsSheet` → `ChapterSettingsDialog` (section 8.2).
- `Dialog.TrackSheet` → `NavigatorAdaptiveSheet(MangaTrackInfoDialogHomeScreen(mangaId, title, sourceId), enableSwipeDismiss = lastItem is MangaTrackInfoDialogHomeScreen)`. Track dialog internals are part 2.
- `Dialog.FullCover` → cover dialog (section 8.5). Creates `MangaCoverScreenModel(manga.id)` per show.
- `Dialog.SetMangaFetchInterval` → `SetIntervalDialog(interval = manga.fetchInterval, nextUpdate = manga.expectedNextUpdate, isManga = true, onValueChanged = setFetchInterval(manga, interval) .takeIf { screenModel.isUpdateIntervalEnabled })`.

`onDismissRequest` for all dialogs (ui MangaScreen.kt:179-185): `screenModel.dismissDialog()`; then if `screenModel.autoOpenTrack && screenModel.isFromChangeCategory`, reset the flag and `showTrackDialog()`. `autoOpenTrack = trackingAvailable && trackPreferences.trackOnAddingToLibrary().get()` (MangaScreenModel.kt:158-160).

### 8.2 ChapterSettingsDialog (filter / sort / display sheet)

Source: `app/src/main/java/eu/kanade/presentation/entries/manga/ChapterSettingsDialog.kt`

- `TabbedDialog` with tabs (titles from `MR.strings`): `action_filter` "Filter" (page 0), `action_sort` "Sort" (page 1), `action_display` "Display" (page 2).
- Overflow menu (`tabOverflowMenuContent`):
  - `set_chapter_settings_as_default` "Set as default" → opens `SetAsDefaultDialog` (local boolean `showSetAsDefaultDialog`).
  - `action_reset` "Reset" → `onResetToDefault()` then close menu.
- `downloadedOnly = remember { Injekt.get<BasePreferences>().downloadedOnly().get() }` (ChapterSettingsDialog.kt:70).

Filter page (`FilterPage`, lines 133-163):

- `TriStateItem(label = "Downloaded", state = downloadFilter, onClick = onDownloadFilterChanged.takeUnless { downloadedOnly })`. When the global "downloaded only" preference is on, the item is not clickable (`enabled = false`), and the domain getter `Manga.downloadedFilter` returns `ENABLED_IS` regardless of stored flags (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:23-31`).
- `TriStateItem(label = "Unread", state = unreadFilter, onClick = onUnreadFilterChanged)` (`action_filter_unread`).
- `TriStateItem(label = "Bookmarked", state = bookmarkedFilter, ...)` (`action_filter_bookmarked`).
- `ScanlatorFilterItem(active = scanlatorFilterActive, onClick = ...)`: row with `Icons.Outlined.PeopleAlt` (tint `colorScheme.active` when active, else `LocalContentColor`), label `"Scanlator"` (`scanlator`), padding `horizontal = TabbedDialogPaddings.Horizontal`, `vertical = 12.dp`, `spacedBy(24.dp)`. Click → opens the scanlator dialog (section 8.3).
- TriState cycle (presentation-core `TriStateItem`, `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt:355-370`): `DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED`. Same as `TriState.next()` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:9-15`).
- Filter semantics: `setUnreadFilter`, `setDownloadedFilter`, `setBookmarkedFilter` (MangaScreenModel.kt:878, 895, 913) write the `CHAPTER_*` bits; part 2.

Sort page (`SortPage`, lines 195-212):

- Four `SortItem`s in this order: `sort_by_source` "By source" (`Manga.CHAPTER_SORTING_SOURCE`), `sort_by_number` "By chapter number" (`CHAPTER_SORTING_NUMBER`), `sort_by_upload_date` "By upload date" (`CHAPTER_SORTING_UPLOAD_DATE`), `action_sort_alpha` "Alphabetically" (`CHAPTER_SORTING_ALPHABET`).
- Each item receives `sortDescending = sortDescending.takeIf { sortingMode == mode }`: so the arrow shows only on the active mode. `true` → `Icons.Default.ArrowDownward`, `false` → `Icons.Default.ArrowUpward`, `null` → no arrow.
- `sortDescending` passed from the sheet is `manga?.sortDescending() ?: false`.
- Click → `onSortModeChanged(mode)` → `screenModel.setSorting(mode)` → `setMangaChapterFlags.awaitSetSortingModeOrFlipOrder(manga, sort)` (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/SetMangaChapterFlags.kt:47-68`):
  - If `manga.sorting == mode`: flip the direction bit. `orderFlag = if (manga.sortDescending()) CHAPTER_SORT_ASC else CHAPTER_SORT_DESC`.
  - Else: set sorting bits to `mode` and set the direction bit to `CHAPTER_SORT_ASC`. So choosing a new mode always resets to ascending (`sortDescending() == false`).

Display page (`DisplayPage`, lines 215-229):

- `RadioItem`s: `show_title` "Source title" → `CHAPTER_DISPLAY_NAME` (0x0), `show_chapter_number` "Chapter number" → `CHAPTER_DISPLAY_NUMBER` (0x100000). Selected when `displayMode == mode`.
- `displayMode` default when manga is null: `0`.
- Click → `setDisplayMode(mode)` → `awaitSetDisplayMode` (`SetMangaChapterFlags.kt:38-45`) which writes the display mask only.

Dialog content padding: `Column(padding(vertical = TabbedDialogPaddings.Vertical).verticalScroll(rememberScrollState()))`.

### 8.3 ScanlatorFilterDialog

Source: `app/src/main/java/eu/kanade/presentation/entries/manga/components/ScanlatorFilterDialog.kt`

- Title: `exclude_scanlators` "Exclude scanlators".
- `sortedAvailableScanlators = availableScanlators.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it })` (remembered on the input).
- `mutableExcludedScanlators = excludedScanlators.toMutableStateList()` (remembered on the input).
- Empty `availableScanlators`: body text `no_scanlators_found` "No scanlators found"; only a Cancel button.
- Otherwise: a `LazyColumn` of rows. Each row is `minimumInteractiveComponentSize`, `clip(MaterialTheme.shapes.small)`, `fillMaxWidth`, `padding(horizontal = MaterialTheme.padding.small)`, `clickable` toggling membership. Icon: `Icons.Rounded.DisabledByDefault` (primary tint) when excluded, `Icons.Rounded.CheckBoxOutlineBlank` (`LocalContentColor`) when not. Label `bodyMedium`, `padding(start = 24.dp)`.
- Dividers: `HorizontalDivider` at top if `state.canScrollBackward`, at bottom if `state.canScrollForward`.
- Buttons (`FlowRow`):
  - If `mutableExcludedScanlators.isEmpty()`: "Select all" (`action_select_all`) → `addAll(availableScanlators)` (this is the **unsorted** input set, but the result is a set so order does not matter).
  - Else: "Reset" (`action_reset`) → `clear()`.
  - `Spacer(weight 1f)`, "Cancel", "OK" (`action_ok`) → `onConfirm(mutableExcludedScanlators.toSet())` then dismiss.
- `DialogProperties(usePlatformDefaultWidth = true)`.
- Semantics: the stored set is the **excluded** set. Excluded scanlators are hidden (applied in the repo subscription, MangaScreenModel.kt:217 and the `getExcludedScanlators.subscribe` at 260ff). `setExcludedScanlators` (MangaScreenModel.kt:1133) is part 2.

### 8.4 DuplicateMangaDialog

Source: `app/src/main/java/eu/kanade/presentation/entries/manga/DuplicateMangaDialog.kt`

- Presented as an `AdaptiveSheet` (bottom sheet on phones; the Linux port uses a modal `gtk::Window` or `gtk::Popover`).
- Title: `are_you_sure` "Are you sure?" (headlineMedium), body `confirm_add_duplicate_manga`: "You have an entry in your library with the same name.\n\nDo you still wish to continue?" (`bodyMedium`).
- Three list rows, separated by `HorizontalDivider`:
  1. `action_show_manga` "Show entry" (AYMR; the source text in strings is "Show manga" in the MR base file, but the code uses `AYMR.strings.action_show_manga` which is "Show entry") → dismiss, then `onOpenManga()` which pushes `MangaScreen(dialog.duplicate.id)`. Icon `Icons.Outlined.Book`.
  2. `action_migrate_duplicate` "Migrate existing entry" → dismiss, then `onMigrate()` → `screenModel.showMigrateDialog(dialog.duplicate)`. Icon `Icons.Outlined.SwapVert`.
  3. `action_add_anyway` "Add anyway" → dismiss, then `onConfirm()` → `screenModel.toggleFavorite(onRemoved = {}, checkDuplicate = false)`. Icon `Icons.Outlined.Add`.
- Cancel: a full-width `OutlinedButton` (`action_cancel`), `titleLarge` at 16sp, primary color.
- Spacing: title padding `bottom 16dp, top 8dp`; button row padding `top 16dp, bottom 16dp`; `PaddingSize = 16.dp`.
- `minHeight = LocalPreferenceMinHeight.current` for the cancel row.

### 8.5 MangaCoverDialog and MangaCoverScreenModel

Presentation: `app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt`

- `Dialog(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)` full screen.
- Bottom bar (`Scaffold(containerColor = Transparent)`), padding `4.dp`, `navigationBarsPadding()`:
  - Left pill (`ActionsPill`: `clip(MaterialTheme.shapes.extraLarge)`, background `colorScheme.background` at alpha 0.95): Close icon (`Icons.Outlined.Close`, `action_close`) → `onDismissRequest`.
  - Right pill: `AppBarActions` with two actions: `action_share` (`Icons.Outlined.Share`) → `onShareClick`; `action_save` (`Icons.Outlined.Save`) → `onSaveClick`.
  - Then, only if `onEditClick != null`: an `IconButton` with `Icons.Outlined.Edit` (`action_edit_cover` "Edit cover"):
    - If `isCustomCover` → open a `DropdownMenu` (offset 8dp) with `action_edit` "Edit" (→ `EditCoverAction.EDIT`) and `action_delete` "Delete" (→ `EditCoverAction.DELETE`).
    - Else → `onEditClick(EditCoverAction.EDIT)` directly.
- Body: `Box(fillMaxSize, clickableNoIndication(onDismissRequest))` (tap anywhere on the image to close) containing `AndroidView { ReaderPageImageView }` (Android-only SSIV-like zoom view; see Rust mapping in part `p1-rust-design.md`):
  - `onViewClicked = onDismissRequest`, `clipToPadding = false`, `clipChildren = false`.
  - `update`: loads `ImageRequest.Builder(view.context).data(manga).size(Size.ORIGINAL).memoryCachePolicy(CachePolicy.DISABLED)`; on target: if BitmapDrawable, copies the bitmap with `Bitmap.Config.HARDWARE` (comment: SSIV needs to read the image thoroughly); `view.setImage(copy, ReaderPageImageView.Config(zoomDuration = 500))`.
  - `view.updatePadding(top = statusBarPx, bottom = bottomPaddingPx)` so the image is not under the bars.

ScreenModel: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt`

- `MangaCoverScreenModel(mangaId, getManga, imageSaver, coverCache, updateManga, snackbarHostState)` extends `StateScreenModel<Manga?>(null)`.
- `init`: `getManga.subscribe(mangaId).collect { mutableState.update { it } }` on IO (so the displayed manga updates when the cover changes).
- `saveCover(context)` (lines 51-67): `saveCoverInternal(temp = false)`; success → snackbar `cover_saved` "Cover saved"; any `Throwable` → logcat ERROR and snackbar `error_saving_cover` "Error saving cover". Snackbar `withDismissAction = true`.
- `shareCover(context)` (lines 69-84): `saveCoverInternal(temp = true)` (returns `null` → `return@launch`, no share); then `withUIContext { context.startActivity(uri.toShareIntent(context)) }`. Failure → snackbar `error_sharing_cover` "Error sharing cover".
- `saveCoverInternal(context, temp)` (lines 92-112):
  - `manga = state.value ?: return null`.
  - `ImageRequest.Builder(context).data(manga).size(Size.ORIGINAL).build()`.
  - `context.imageLoader.execute(req).image?.asDrawable(resources)`; `bitmap = result?.getBitmapOrNull() ?: return null`.
  - `imageSaver.save(Image.Cover(bitmap, name = manga.title, location = if (temp) Location.Cache else Location.Pictures.create()))`.
  - `// TODO: Handle animated cover` (line 102). Animated covers are not handled.
- `editCover(context, data: Uri)` (lines 120-132): `openInputStream(uri)?.use { manga.editCover(Injekt.get(), it, updateManga, coverCache); notifyCoverUpdated }`, failure → `notifyFailedCoverUpdate`. Note: if `openInputStream` returns null, nothing happens (no message).
- `deleteCustomCover(context)` (lines 134-145): `coverCache.deleteCustomCover(mangaId)`; `updateManga.awaitUpdateCoverLastModified(mangaId)`; `notifyCoverUpdated`; failure → `notifyFailedCoverUpdate`.
- `notifyCoverUpdated` → snackbar `cover_updated` "Cover updated". `notifyFailedCoverUpdate` → snackbar `notification_cover_update_failed` "Failed to update cover" and logs the exception.
- Edit cover picker: `rememberLauncherForActivityResult(ActivityResultContracts.GetContent())` with mime `"image/*"` (ui MangaScreen.kt:261-275). Null result → no-op.
- `MangaCoverDialog`'s `isCustomCover = remember(manga) { manga!!.hasCustomCover() }` (ui MangaScreen.kt:270).
- Loading placeholder while `manga == null`: `LoadingScreen(Modifier.systemBarsPadding())`.

Cover image pipeline (mapping only): Coil `ImageRequest` with `Size.ORIGINAL`. Storage of covers and `editCover` internals: `app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt:52` (not read; area:34-images-covers-cache). `ImageSaver`/`Location`: `app/src/main/java/eu/kanade/tachiyomi/data/saver/` (not read; area:34 owns).

### 8.6 DeleteItemsDialog

Source: `app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt:36-67`

- Title `are_you_sure` "Are you sure?"; text `confirm_delete_chapters` "Are you sure you want to delete the selected chapters?" (manga branch; anime uses `AYMR.strings.confirm_delete_episodes`).
- Confirm (`action_ok` "OK"): `onDismissRequest(); onConfirm()`. Cancel (`action_cancel`).
- The ui layer runs `screenModel.toggleAllSelection(false)` then `screenModel.deleteChapters(dialog.chapters)`. So deleting clears selection.

### 8.7 SetIntervalDialog (fetch interval)

Source: `app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt:69-172`

- `selectedInterval = rememberSaveable { mutableIntStateOf(if (interval < 0) -interval else 0) }`. Stored interval is negative for user-chosen intervals. Initial wheel value is `|interval|` when user mode, else 0.
- `nextUpdateDays` as in section 4.1.
- Title `pref_library_update_smart_update` "Smart update".
- Body text:
  - If `nextUpdateDays != null && nextUpdateDays >= 0 && interval >= 0`: `manga_interval_expected_update` = "New chapters predicted to be released in around %1$s, checking around every %2$s." with `pluralStringResource(day, nextUpdateDays)` and `pluralStringResource(day, |interval|)`.
  - Else: `manga_interval_expected_update_null` = "This manga is either completed, or there is no predicted release date."
- Wheel: shown only when `onValueChanged != null && !isReleaseBuildType`. So the custom-interval picker is **debug-build only**. In release builds, the dialog is informational only.
  - Label: `manga_interval_custom_amount` "Custom update frequency:".
  - `BoxWithConstraints`, `size = DpSize(maxWidth / 2, 128.dp)`, `maxInterval = MangaFetchInterval.MAX_INTERVAL` (domain; value not read in this pass, located in `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt`).
  - Items: `(0..maxInterval).map { if (it == 0) "Default" (label_default) else it.toString() }`.
  - `WheelTextPicker(items, size, startIndex = selectedInterval, onSelectionChanged = { selectedInterval = it })`.
- Confirm (`action_ok`): `onValueChanged?.invoke(selectedInterval)` then dismiss. Cancel: dismiss.
- Negative values: `setFetchInterval(manga, interval)` (MangaScreenModel.kt:405-420) stores `-interval` (`manga.copy(fetchInterval = -interval)`), so selecting 0 ("Default") stores `0` (automatic). Part 2.
- Interaction: `onValueChanged` is `null` when `!screenModel.isUpdateIntervalEnabled`, which is `LibraryPreferences.ENTRY_OUTSIDE_RELEASE_PERIOD in libraryPreferences.autoUpdateItemRestrictions().get()` (MangaScreenModel.kt:150-151). Default restriction set includes `ENTRY_OUTSIDE_RELEASE_PERIOD` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:49-57`). When disabled, the wheel is not composed and confirm only dismisses.
- The `isUpdateIntervalEnabled` gate does not affect the display text.

### 8.8 Dialog stubs for migrate and track

- `MigrateMangaDialog`, `MigrateMangaDialogScreenModel`, `MigrateMangaSearchScreen`: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/` (part 2 / area:38 part 2 and area:37 browse). Triggered from section 9.
- `MangaTrackInfoDialogHomeScreen` and `MangaTrackInfoDialog.kt`: part 2.

---

## 9. Callback wiring table (UI-level to ScreenModel/Navigator)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt:117-175`

| Presentation param | UI expression | Target |
|---|---|---|
| `onChapterClicked` | `openChapter(context, it)` | `ReaderActivity.newIntent(context, chapter.mangaId, chapter.id)` |
| `onDownloadChapter` | `screenModel::runChapterDownloadActions` when `!source.isLocalOrStub()`, else null | part 2 |
| `onAddToLibraryClicked` | `toggleFavorite()` + LongPress haptic | part 2 |
| `onWebViewClicked` | `openMangaInWebView(navigator, manga, source)` when HTTP | `WebViewScreen(url, initialTitle = title, sourceId)` |
| `onWebViewLongClicked` | `copyMangaUrl(context, manga, source)` when HTTP | clipboard |
| `onTrackingClicked` | see 4.1 | `SettingsScreen(Tracking)` or `showTrackDialog()` |
| `onTagSearch` | `scope.launch { performGenreSearch(navigator, it, screenModel.source!!) }` | section 10 |
| `onFilterButtonClicked` | `screenModel::showSettingsDialog` | `Dialog.SettingsSheet` |
| `onRefresh` | `screenModel::fetchAllFromSource` | part 2 |
| `onContinueReading` | `continueReading(context, getNextUnreadChapter())` | reader |
| `onSearch` | `scope.launch { performSearch(navigator, query, global) }` | section 10 |
| `onCoverClicked` | `screenModel::showCoverDialog` | `Dialog.FullCover` |
| `onShareClicked` | `shareManga(context, manga, source)` when HTTP | `Intent.createChooser(... ACTION_SEND text/plain)` |
| `onDownloadActionClicked` | `screenModel::runDownloadAction` when not local | part 2 |
| `onEditCategoryClicked` | `screenModel::showChangeCategoryDialog` when favorite | `Dialog.ChangeCategory` |
| `onEditFetchIntervalClicked` | `screenModel::showSetMangaFetchIntervalDialog` when favorite | `Dialog.SetMangaFetchInterval` |
| `onMigrateClicked` | `navigator.push(MigrateMangaSearchScreen(manga.id))` when favorite | migrate search |
| `onMultiBookmarkClicked` | `screenModel::bookmarkChapters` | part 2 |
| `onMultiMarkAsReadClicked` | `screenModel::markChaptersRead` | part 2 |
| `onMarkPreviousAsReadClicked` | `screenModel::markPreviousChapterRead` | part 2 |
| `onMultiDeleteClicked` | `screenModel::showDeleteChapterDialog` | `Dialog.DeleteChapters` |
| `onChapterSwipe` | `screenModel::chapterSwipe` | part 2 |
| `onChapterSelected` | `screenModel::toggleSelection` | section 6.6 |
| `onAllChapterSelected` | `screenModel::toggleAllSelection` | part 2 |
| `onInvertSelection` | `screenModel::invertSelection` | part 2 |

Note on `onMarkPreviousAsReadClicked`: the bottom bar only shows it for a single selection (section 7.7).

---

## 10. Navigation and search semantics

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt:359-412`

### 10.1 performSearch(navigator, query, global)

1. If `global == true` → `navigator.push(GlobalMangaSearchScreen(query))` and return.
2. If `navigator.size < 2` → return (no-op).
3. `previousController = navigator.items[navigator.size - 2]`.
4. If previous is `HomeScreen` → `navigator.pop()` then `MangaLibraryTab.search(query)` (library search).
5. If previous is `BrowseMangaSourceScreen` → `navigator.pop()` then `previousController.search(query)`.
6. Otherwise nothing (no pop, no search).

Callers: `doSearch(title, true)` for title and author (global), `doSearch(artist, true)` for artist (global), `doSearch(sourceName, false)` for source name (non-global).

### 10.2 performGenreSearch(navigator, genreName, source)

1. If `navigator.size < 2` → return.
2. `previous = navigator.items[size - 2]`.
3. If `previous is BrowseMangaSourceScreen && source is HttpSource` → `navigator.pop(); previous.searchGenre(genreName)`.
4. Else → `performSearch(navigator, genreName, global = false)`.

### 10.3 Other navigation

- Edit categories → `navigator.push(CategoriesTab)` and `CategoriesTab.showMangaCategory()` (from change category dialog).
- Source settings → `navigator.push(MangaSourcePreferencesScreen(state.source.id))` (presentation MangaScreen.kt:144-146), only for `ConfigurableSource`.
- Tracking settings → `SettingsScreen(SettingsScreen.Destination.Tracking)` when no logged-in trackers.
- Show duplicate entry → `navigator.push(MangaScreen(dialog.duplicate.id))`.
- Migrate result → `navigator.replace(MangaScreen(dialog.newManga.id))` (so the old screen is replaced).

---

## 11. Share, copy URL, assist URL

### 11.1 getMangaUrl (ui MangaScreen.kt:315-324)

- `manga_ ?: return null`; `source_ as? HttpSource ?: return null`; `try { source.getMangaUrl(manga.toSManga()) } catch (e: Exception) { null }`.
- `Manga.toSManga()` is in `app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:38`.
- `HttpSource.getMangaUrl` is defined in the source API (area:15-source-api-manga).

### 11.2 shareManga (ui MangaScreen.kt:338-352)

- `try { getMangaUrl(...)?.let { url -> val intent = url.toUri().toShareIntent(context, type = "text/plain"); context.startActivity(Intent.createChooser(intent, context.stringResource(MR.strings.action_share))) } } catch (e: Exception) { context.toast(e.message) }`.
- Null URL → no-op (no toast).

### 11.3 copyMangaUrl (ui MangaScreen.kt:407-412)

- `val url = source.getMangaUrl(manga.toSManga())` **without** try/catch. If the source throws, the exception propagates into the `Composable` callback. Port with a guarded version (see edge case E-17).
- `context.copyToClipboard(url, url)`.

### 11.4 openMangaInWebView (ui MangaScreen.kt:326-336)

- Uses `getMangaUrl`; null → no-op. Otherwise `navigator.push(WebViewScreen(url, initialTitle = manga.title, sourceId = source.id))`.

### 11.5 AssistContent (ui MangaScreen.kt:76-115)

- `onProvideAssistUrl()` returns `assistUrl`, which is filled in a `LaunchedEffect` (only for HTTP sources). This is the Android "assist" (long-press home) integration. Drop on Linux (P3), see task list.

---

## 12. Refresh and fetch

- `onRefresh = screenModel::fetchAllFromSource` (manual fetch, default `manualFetch = true`, MangaScreenModel.kt:258-273). Sets `isRefreshingData = true`, runs `fetchMangaFromSource` and `fetchChaptersFromSource` in parallel, sets `isRefreshingData = false`.
- `PullRefresh(refreshing = state.isRefreshingData, onRefresh, enabled = !isAnySelected, indicatorPadding = ...)` (presentation MangaScreen.kt:372-377 and 619-628). Pull-to-refresh is disabled while selection is active.
- Toolbar "Refresh" overflow item calls the same `onRefresh`.
- First open (MangaScreenModel.kt:216-257): `needRefreshInfo = !manga.initialized`; `needRefreshChapter = chapters.isEmpty()`; `isRefreshingData = needRefreshInfo || needRefreshChapter`; both fetches run in parallel if needed; then `isRefreshingData = false`.
- `fetchMangaFromSource` (MangaScreenModel.kt:275-291): on error, `HttpException` with code `103` is ignored (early hints); every other `Throwable` is logged and shown as a snackbar with `e.formattedMessage`.
- `fetchChaptersFromSource` (569-603): error message `NoChaptersException` case at line 587; part 2.

Proposal: Refresh = `Ctrl+R` shortcut in addition to the pull gesture (a mouse user has no pull gesture). Show the spinner in the list header while `isRefreshingData`.

---

## 13. Snackbars and messages

- The screen owns a `SnackbarHostState` (`screenModel.snackbarHostState`) and renders `SnackbarHost(hostState)` in the scaffold (presentation MangaScreen.kt:344 and 591).
- Messages used by this area (all come from the ScreenModel or cover model):
  - Cover: `cover_saved` "Cover saved", `error_saving_cover` "Error saving cover", `cover_updated` "Cover updated", `notification_cover_update_failed` "Failed to update cover", `error_sharing_cover` "Error sharing cover". All snackbars use `withDismissAction = true`.
  - Chapter settings: `chapter_settings_updated` (used in `setCurrentSettingsAsDefault`, MangaScreenModel.kt:951-962; text not read in this pass).
  - Fetch error: `e.formattedMessage` (dynamic).
  - Delete downloads on removing from library: `delete_downloads_for_manga` "Delete downloaded chapters?" with action `action_delete` (MangaScreenModel.kt:293-304, see part 2).
- Haptics: `HapticFeedbackType.LongPress` on long-press of chapter rows, library button, and download indicator (`commonClickable(hapticFeedback = ...)`). See section 15 for the Linux mapping.

---

## 14. Concurrency and dispatch (UI side)

- All ScreenModel work uses `screenModelScope`. Dialog content uses `rememberCoroutineScope()` only for the bottom bar auto-reset job (`EntryBottomActionMenu.kt:93`).
- `MangaCoverScreenModel` uses `launchIO` for DB and file IO and `withUIContext` only for the share intent (MangaCoverScreenModel.kt:73-75).
- `saveCoverInternal` runs in `withIOContext` (line 99).
- Chapter list recompute is on `distinctUntilChanged` + `collectLatest` (MangaScreenModel.kt:182-200 region); the UI recomputes `processedChapters` lazily when `State.Success` is replaced.
- Rust: keep all state mutation on the GTK main thread; run DB and network on tokio; send results back via `glib::MainContext::channel` or `glib::spawn_future_local` as described in `p1-rust-design.md`.

---

## 15. Haptics and gestures (mapping)

Android-only input feedback used here:

- `LocalHapticFeedback.performHapticFeedback(LongPress)`: called on library button click (ui MangaScreen.kt:129), chapter long-press (presentation MangaScreen.kt:818), and any `commonClickable(hapticFeedback)` (download indicator). Linux: no haptics. Replace with nothing, or with a short CSS flash of the pressed row (proposal). Drop with a note (P3).
- `combinedClickable(onClick, onLongClick)`: Linux: `gtk::GestureClick` (button 1 = click) plus `gtk::GestureLongPress` (touch and pen long-press) plus a secondary-button click (button 3) that acts as long-press for mouse users. Proposal: right-click = long-press for every `combinedClickable` in this area.
- Swipe (`me.saket.swipe.SwipeableActionsBox`): Linux: `gtk::GestureDrag` on touch screens and horizontal trackpad scroll; threshold 56 px (`swipeActionThreshold` 56.dp). The row shows the action background as it is dragged, and on passing the threshold the swipe action fires. Mouse drag must work too. Keyboard equivalent: shortcut keys on focused row (section 16).

---

## 16. Keyboard and pointer equivalents (proposal, not in Aniyomi)

Aniyomi's Compose screen is touch-first; the mockup is GTK. Define:

- Chapter list: `Up`/`Down` move focus; `Enter` open chapter (or toggle selection while in selection mode); `Space` toggle selection; `Shift+Click` range select; `Ctrl+Click` toggle; `Ctrl+A` select all; `Ctrl+Shift+A` clear; `Ctrl+I` invert; `Escape` clear selection (or go back when none); `Delete` delete downloads for selected (if any downloaded); `B` bookmark toggle on selected; `M` toggle read on selected; `D` download selected.
- Header: `F` opens the filter/sort sheet; `R` refreshes; `Ctrl+M` migrate; `W` opens WebView when available.

These are proposals. Each must appear in `p1-tasks.md` as an acceptance criterion where it matters.

---

## 17. Preferences touched by this area

See `p1-data-and-config.md` section 5 for the full table. Behaviourally: `pref_chapter_swipe_start_action` (default ToggleBookmark), `pref_chapter_swipe_end_action` (default ToggleRead), `skip_filtered` (true), `pref_downloaded_only` (false), `library_update_manga_restriction` (default set including `manga_outside_release_period`), `default_manga_category` (-1), plus the track and reader preferences listed.
