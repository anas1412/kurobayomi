# Area 42 part 1 — behavior specification

Repo root for every path: `/home/blackbox/aniyomi-src`. Paths are repo-relative. Line references are `path:line`. Text labelled **OBSERVED** is what the Kotlin does; **PROPOSAL** is what Kurobayomi should do. Message keys are written `MR.strings.<key>` (shared i18n) or `AYMR.strings.<key>` (Aniyomi i18n); English values are given in quotes from `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`.

Shared terms used below:
- **entry** = a manga or an anime (the two kinds share the same behavior with different tables and keys).
- **history row** = one row of the `history` (manga) or `animehistory` (anime) table.
- **update row** = one row of `updatesView` / `animeupdatesView` (a chapter or episode of a favorite entry).

---

## 0. Screen map and navigation

### 0.1 Top-level tabs (OBSERVED)

| Tab object | File | Title (`TabOptions.title`) | Index (`TabOptions.index`) | Sub-tabs (order) | Reselect action |
|---|---|---|---|---|---|
| `HistoriesTab` | `app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt:29-50` | `MR.strings.history` ("History") | `MOVE_HISTORY_TO_MORE -> 5u`, `MOVE_BROWSE_TO_MORE -> 3u`, else `2u` (`:36-40`) | 0 = `animeHistoryTab` (anime), 1 = `mangaHistoryTab` (manga) (`:65-68`) | `resumeLastEpisodeSeenEvent.send(Unit)` (`:48-50`) — always the ANIME event, even when the manga sub-tab is selected |
| `UpdatesTab` | `app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt:24-45` | `MR.strings.label_recent_updates` ("Updates") | `MOVE_UPDATES_TO_MORE -> 5u`, `MOVE_HISTORY_TO_MORE -> 2u`, `MOVE_BROWSE_TO_MORE -> 2u`, `MOVE_MANGA_TO_MORE -> 1u` (`:31-36`) | 0 = anime updates, 1 = manga updates (`:54-57`) | `navigator.push(DownloadsTab)` (`:43-45`) |
| `StatsTab` | `app/src/main/java/eu/kanade/tachiyomi/ui/stats/StatsTab.kt:22-56` | `MR.strings.label_stats` ("Statistics") | `8u` (`:30`) | 0 = `animeStatsTab()`, 1 = `mangaStatsTab()` (`:40-43`) | none |
| `StorageTab` | `app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt:22-56` | `AYMR.strings.label_storage` ("Storage") | `8u` (`:30`) | 0 = anime storage, 1 = manga storage (`:40-43`) | none |

Icons: `HistoriesTab` uses `R.drawable.anim_history_enter`; `UpdatesTab`, `StatsTab`, `StorageTab` use `R.drawable.anim_updates_enter` (`HistoriesTab.kt:35`, `UpdatesTab.kt:30`, `StatsTab.kt:28`, `StorageTab.kt:28`). `isSelected` drives the animated vector (`rememberAnimatedVectorPainter(image, isSelected)`).

Note: `StatsTab` and `StorageTab` both use index `8u`. OBSERVED; the navigation layer (area:36-ui-shell-navigation-security-webview) must tolerate duplicate indices.

### 0.2 Sub-tab titles (OBSERVED)

- History sub-tabs: anime tab title `AYMR.strings.label_anime_history` ("Anime") (`app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryTab.kt:81`); manga tab title `AYMR.strings.label_history` ("Manga") (`app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt:78`). The manga tab's label is "Manga" (not "History") — keep as observed.
- Stats sub-tabs: `AYMR.strings.label_anime` ("Anime") and `AYMR.strings.label_manga` ("Manga") (`app/src/main/java/eu/kanade/tachiyomi/ui/stats/anime/AnimeStatsTab.kt:28`, `.../stats/manga/MangaStatsTab.kt:28`).
- Storage sub-tabs: `AYMR.strings.label_anime`, `AYMR.strings.label_manga` (`.../storage/anime/AnimeStorageTab.kt:22`, `.../storage/manga/MangaStorageTab.kt:22`).
- Updates sub-tabs: defined in part-2 files (`.../updates/anime/AnimeUpdatesTab.kt`, `.../updates/manga/MangaUpdatesTab.kt`).

### 0.3 TabbedScreen (OBSERVED; `app/src/main/java/eu/kanade/presentation/components/TabbedScreen.kt`)

- Top bar: `SearchToolbar` with `titleContent = AppBarTitle(stringResource(titleRes), ..., tab.numberTitle)`, `searchEnabled = tab.searchEnabled`, `searchQuery = if (searchEnabled) actualQuery else null`, actions = `tab.actions`, `navigateUp = tab.navigateUp` (`:54-79`).
- Which query/callback is used: `state.currentPage % 2 == 1` → manga query and manga callback; otherwise anime query and anime callback (`:44-63`). The page index is used modulo 2, so this assumes exactly two sub-tabs.
- Sub-tab row: `FlexibleTabRow(scrollable = scrollable)`; `scrollable` defaults to `false` (`:42`), so `PrimaryTabRow` is used. Each tab `onClick = animateScrollToPage(index)`; text = `TabText(text=stringResource(tab.titleRes), badgeCount = tab.badgeNumber)`; unselected content colour `onSurface` (`:91-109`).
- Body: `HorizontalPager(fillMaxSize, verticalAlignment = Top)`; each page renders `tabs[page].content(PaddingValues(bottom = contentPadding.calculateBottomPadding()), snackbarHostState)` (`:111-122`).
- `TabContent` fields (`:128-136`): `titleRes`, `badgeNumber: Int? = null`, `searchEnabled = false`, `actions = []`, `content`, `numberTitle = 0`, `cancelAction = {}`, `navigateUp: (() -> Unit)? = null`.

PROPOSAL: GTK equivalent is one `adw`-free header bar (`gtk::HeaderBar`) with a `gtk::SearchEntry` shown when `searchEnabled`, and a `gtk::Notebook`-style tab strip (`gtk::Box` of toggle buttons + `gtk::Stack`). Search state is kept per sub-tab (anime and manga independently) exactly as the Kotlin does.

---

## 1. History (manga and anime)

### 1.1 Data source and query semantics (OBSERVED)

Manga: `data/src/main/sqldelight/view/historyView.sq` defines `historyView`:
- `FROM mangas JOIN chapters ON mangas._id = chapters.manga_id JOIN history ON chapters._id = history.chapter_id`.
- Joined to `max_last_read`: `SELECT chapters.manga_id, chapters._id AS chapter_id, MAX(history.last_read) AS last_read FROM chapters JOIN history ON chapters._id = history.chapter_id GROUP BY chapters.manga_id`.
- Columns exposed: `id = history._id`, `mangaId`, `chapterId`, `title`, `thumbnailUrl`, `source`, `favorite`, `cover_last_modified`, `chapterNumber = chapters.chapter_number`, `readAt = history.last_read`, `readDuration = history.time_read`, `maxReadAt`, `maxReadAtChapterId`.

The `history` query (`data/src/main/sqldelight/data/history.sq`):
```
SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration
FROM historyView
WHERE historyView.readAt > 0
AND maxReadAtChapterId = historyView.chapterId
AND lower(historyView.title) LIKE ('%' || :query || '%')
ORDER BY readAt DESC;
```
Consequences (OBSERVED):
1. Only the most recently read chapter of each manga is listed (`maxReadAtChapterId = chapterId`). Older rows of the same manga are hidden.
2. Rows reset by "remove" (`last_read = 0`) are excluded by `readAt > 0`.
3. Search is a substring match on the title, `LIKE` with the raw user text between `%` wildcards. The user text is NOT escaped: `%` and `_` in the query act as SQL wildcards. `lower()` is applied to the title only; SQLite `LIKE` is ASCII-case-insensitive by default, so ASCII search is case-insensitive; non-ASCII case folding does not apply.
4. Ordering: `readAt DESC` (last-read first).

Latest-history query used by "resume last" (`historyView.sq`): `getLatestHistory`: same filter `readAt > 0`, no title filter, `ORDER BY readAt DESC LIMIT 1`. NOTE: this does not require `maxReadAtChapterId = chapterId`; the most recent read row overall wins.

Anime: `data/src/main/sqldelightanime/view/animehistoryView.sq` mirrors the manga view with `animes`, `episodes`, `animehistory` (`last_seen AS seenAt`, `episode_number AS episodeNumber`, `maxSeenAtEpisodeId`). Query `animehistory` (in the same file) has the same shape with `seenAt > 0 AND maxSeenAtEpisodeId = episodeId AND lower(title) LIKE ...  ORDER BY seenAt DESC`. `getLatestAnimeHistory` = `seenAt > 0 ORDER BY seenAt DESC LIMIT 1`.

Repository wiring (OBSERVED):
- `data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt:16-20`: `getMangaHistory(query)` = `handler.subscribeToList { historyViewQueries.history(query, mapper) }` (a reactive query; re-emits on table change).
- `getLastMangaHistory()` = `awaitOneOrNull { getLatestHistory }`.
- `getTotalReadDuration()` = `awaitOne { historyQueries.getReadDuration() }` where `getReadDuration` is `SELECT coalesce(sum(time_read), 0) FROM history;` (`history.sq` last query). It sums over ALL history rows, including rows reset to `last_read = 0` and rows of entries that are not in the library.
- Anime equivalents in `data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt`. There is no duration column for anime history.

Domain interactors (OBSERVED):
- `domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetMangaHistory.kt:17-19`: `subscribe(query: String) = repository.getMangaHistory(query)`.
- `GetAnimeHistory` is the same shape.
- `RemoveMangaHistory` (`domain/.../history/manga/interactor/RemoveMangaHistory.kt`): `awaitAll(): Boolean`, `await(history: MangaHistoryWithRelations)` = `resetMangaHistory(history.id)`, `await(mangaId: Long)` = `resetHistoryByMangaId(mangaId)`.

### 1.2 Subscription and state machine (OBSERVED; `app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt:68-91`)

State: `State(searchQuery: String?, list: List<MangaHistoryUiModel>?, dialog: Dialog?)`, default all null (`:250-255`). `list == null` means loading.

Flow:
1. `_query: MutableStateFlow<String?>(null)` (`:68`).
2. `init` launches (on `screenModelScope`, i.e. dispatched on Main, cancelled when the screen model is disposed): `_query.collectLatest { query -> getHistory.subscribe(query ?: "").distinctUntilChanged().catch { logcat; _events.send(InternalError) }.map { toHistoryUiModels() }.flowOn(Dispatchers.IO).collect { mutableState.update { it.copy(list = newList) } } }`.
3. `search(query)` launches IO and emits to `_query`. `collectLatest` cancels the previous subscription when a new query arrives (no debounce in the Kotlin; PROPOSAL: 250 ms debounce, see T-42a-023).
4. On a DB error the `catch` emits `InternalError` and the subscription ends; the list keeps its last value and the next `search` restarts it.

`updateSearchQuery(query)` (`:141-143`) writes `state.searchQuery` only; nothing in the part-1 UI reads `state.searchQuery` for filtering. OBSERVED dead path; the empty-state text uses the `searchQuery` argument from `query` (see 1.5).

### 1.3 Date grouping (OBSERVED)

`toHistoryUiModels()` (`:93-104`):
- Map each row to `Item(row)`, then `insertSeparators { before, after -> ... }`.
- `beforeDate = before?.item?.readAt?.time?.toLocalDate()` (`Long.toLocalDate()` = `LocalDate.ofInstant(Instant.ofEpochMilli(ms), ZoneId.systemDefault())`, `app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:39-41`); same for `afterDate`.
- Return `Header(afterDate)` when `beforeDate != afterDate && afterDate != null`; otherwise `null` (no separator).
- Consequence: a header is inserted before the first item (because `before == null`), and again whenever the local calendar date changes between consecutive rows. Rows are ordered `readAt DESC`, so headers appear newest first.

Header rendering: `ListGroupHeader(text = relativeDateText(item.date))` (`app/src/main/java/eu/kanade/presentation/history/manga/MangaHistoryScreen.kt:89-93`). `relativeDateText(LocalDate)` is in 1.6.

### 1.4 Row rendering and actions (OBSERVED)

Manga row (`app/src/main/java/eu/kanade/presentation/history/manga/components/MangaHistoryItem.kt`):
- Row height `96.dp` (`HISTORY_ITEM_HEIGHT`, `:35`); the whole row is clickable and performs `onClickResume` (`:46-53`). Padding: `horizontal = MaterialTheme.padding.medium`, `vertical = MaterialTheme.padding.small`.
- Cover: `ItemCover.Book(modifier = fillMaxHeight, data = coverData, onClick = onClickCover)` (`:56-60`).
- Text column (weight 1, padding start `medium`, end `small`):
  - Title: `history.title`, `FontWeight.SemiBold`, `maxLines = 2`, `TextOverflow.Ellipsis`, style `bodyMedium` (`:67-73`).
  - Subtitle (top padding 4 dp, style `bodyMedium`): `val readAt = remember { history.readAt?.toTimestampString() ?: "" }`. If `history.chapterNumber > -1` → `stringResource(MR.strings.recent_manga_time, formatChapterNumber(chapterNumber), readAt)` (English "Ch. %1$s - %2$s"); else just `readAt` (`:74-87`).
  - `toTimestampString()` = `DateFormat.getTimeInstance(DateFormat.SHORT).format(this)` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:25-27`). TIME ONLY (no date), locale-dependent. Under a date header this is intended.
- Favorite button: shown only if `!history.coverData.isMangaFavorite`; `IconButton` with `Icons.Outlined.FavoriteBorder`, `contentDescription = MR.strings.add_to_library` ("Add to library"), tint `onSurface` (`:90-98`).
- Delete button: always shown; `Icons.Outlined.Delete`, `contentDescription = MR.strings.action_delete` ("Delete") (`:100-106`).

Anime row (`app/src/main/java/eu/kanade/presentation/history/anime/components/AnimeHistoryItem.kt`): identical layout (`HistoryItemHeight = 96.dp`, `:36`), cover `ItemCover.Book`, subtitle = `AYMR.strings.recent_anime_time` ("Ep. %1$s - %2$s") with `formatEpisodeNumber(episodeNumber)` when `episodeNumber > -1`, else `seenAt` only (`:75-85`). Favorite shown when `!coverData.isAnimeFavorite` (`:91`).

Row click handlers (OBSERVED, `MangaHistoryScreen.kt:95-104`, `AnimeHistoryScreen.kt:96-105`):
- cover → `onClickCover(mangaId)` → `navigator.push(MangaScreen(it))` (`MangaHistoryTab.kt:85`), or `AnimeScreen(animeId)` (`AnimeHistoryTab.kt:88`).
- row → `onClickResume(mangaId, chapterId)` → `screenModel.getNextChapterForManga` (1.7).
- delete → `onDialogChange(Dialog.Delete(item))` (1.8).
- favorite → `onClickFavorite(mangaId)` → `screenModel.addFavorite(mangaId)` (1.9).

List keys: `key = { "history-${it.hashCode()}" }` (`MangaHistoryScreen.kt:80`). PROPOSAL: key by `(history.id)` for rows and by date for headers (hashCode of data classes is unstable across changes).

Lists use `FastScrollLazyColumn(contentPadding)` (`MangaHistoryScreen.kt:75-77`) and `animateItemFastScroll()` on each row (`:91`, `:98`).

### 1.5 Loading and empty states (OBSERVED, `MangaHistoryScreen.kt:39-62`)

- `list == null` → `LoadingScreen`.
- `list.isEmpty()`:
  - if `searchQuery` non-null and non-empty → `MR.strings.no_results_found` ("No results found");
  - otherwise → manga: `MR.strings.information_no_recent_manga` ("Nothing read recently"); anime: `AYMR.strings.information_no_recent_anime` ("Nothing watched recently") (`AnimeHistoryScreen.kt:43-48`).
- Otherwise the list.

`searchQuery` passed to the screen is the hoisted `query` flow value (`MangaHistoryTab.kt:53`, `:83`).

`MainActivity.ready = true` is set in a `LaunchedEffect(state.list)` when list != null (`MangaHistoryTab.kt:150-154`). Android splash handoff; dropped (see inventory).

### 1.6 Relative date text (OBSERVED)

`relativeDateText(localDate)` (`app/src/main/java/eu/kanade/presentation/components/DateText.kt:45-63`):
- Reads `UiPreferences.relativeTime()` (bool) and `UiPreferences.dateFormat()` (string → `UiPreferences.dateFormat(string)` DateTimeFormatter) once per composition (`remember`).
- Returns `localDate.toRelativeString(context, relative, dateFormat)` or `MR.strings.not_applicable` ("N/A") when null.

`LocalDate.toRelativeString(context, relative = true, dateFormat = DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT))` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:52-80`):
1. If `!relative` → `dateFormat.format(this)`.
2. `difference = ChronoUnit.DAYS.between(this, LocalDate.now())` (positive for past dates).
3. `difference < -7` → formatted date.
4. `difference < 0` → `pluralStringResource(MR.plurals.upcoming_relative_time, |difference|, |difference|)`.
5. `difference < 1` → `MR.strings.relative_time_today` ("Today" in English).
6. `difference < 7` → `pluralStringResource(MR.plurals.relative_time, difference, difference)` (e.g. "1 day ago" / "N days ago" in English; exact plural text is in the i18n resources).
7. otherwise → formatted date.

`LocalDateTime.toRelativeString` (same file, `:82-133`) is used for chapter/episode release dates (part 2 / area:38-ui-entry-details-manga, area:39-ui-entry-details-anime, area:40-ui-browse-manga, area:41-ui-browse-anime), not for history.

PROPOSAL: keep these exact thresholds. Use `chrono::Local::now().date_naive()` for `now`. Formatting: `UiPreferences.dateFormat` values are Java patterns; port with a small pattern translator or use `chrono` `format` with equivalent strftime (see p1-rust-design.md section 7).

### 1.7 Resume (open next) — OBSERVED

Manga row click → `getNextChapterForManga(mangaId, chapterId)` (`MangaHistoryScreenModel.kt:110-114`): launches IO → `sendNextChapterEvent(getNextChapters.await(mangaId, chapterId, onlyUnread = false))` → `_events.send(Event.OpenChapter(chapters.firstOrNull()))`.

`GetNextChapters.await(mangaId, fromChapterId, onlyUnread = false)` (`domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt:33-56`):
1. `chapters = await(mangaId, onlyUnread = false)` = `getChaptersByMangaId(mangaId, applyScanlatorFilter = true)` sorted by `getChapterSort(manga, sortDescending = false)` (`GetNextChapters.kt:21-31`). The sort is the manga's own chapter sort (area:07-domain-entries-items).
2. `currIdx = chapters.indexOfFirst { it.id == fromChapterId }`.
3. `nextChapters = chapters.subList(max(0, currIdx), chapters.size)`.
4. `fromChapter = chapters.getOrNull(currIdx)`.
5. If `fromChapter != null && !fromChapter.read` → `nextChapters` (the current chapter is first). Else `nextChapters.drop(1)`.

Edge (OBSERVED BUG): when `currIdx == -1` (the history chapter is not in the list, e.g. filtered out by the scanlator filter or deleted), `max(0, -1) = 0` so `nextChapters` is the whole list; `fromChapter` is null, so `drop(1)` removes the FIRST chapter. The result is the second chapter. PROPOSAL: if `currIdx == -1` return the first chapter (or the first unread one) and document it; see T-42a-015.

Anime: `AnimeHistoryScreenModel.getNextEpisodeForAnime` → `GetNextEpisodes.await(animeId, episodeId, onlyUnseen = false)` (`domain/.../history/anime/interactor/GetNextEpisodes.kt:33-50`), same algorithm with `seen` and `getEpisodeSort(anime, sortDescending = false)`, and the same `currIdx == -1` quirk.

Manga "resume last" (`getNextChapter()` at `MangaHistoryScreenModel.kt:106-108`): `getNextChapters.await(onlyUnread = false).firstOrNull()` → uses `getLastMangaHistory()` (the newest history row, 1.1) and then the same algorithm. Anime: `getNextEpisode()` identical with `getLastAnimeHistory()`.

Opening (`MangaHistoryTab.kt:55-62`): if `chapter != null` → `ReaderActivity.newIntent(context, chapter.mangaId, chapter.id)` + `startActivity`; else `snackbarHostState.showSnackbar(MR.strings.no_next_chapter)` = "Next chapter not found".

Anime opening (`AnimeHistoryTab.kt:57-65`): reads `PlayerPreferences.alwaysUseExternalPlayer()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt:67`) then `MainActivity.startPlayerActivity(context, episode.animeId, episode.id, extPlayer)`; else snackbar `AYMR.strings.no_next_episode` = "Next Episode not found!".

Reselect (OBSERVED): `HistoriesTab.onReselect` sends to `resumeLastEpisodeSeenEvent` (anime) regardless of the selected sub-tab (`HistoriesTab.kt:48-50`). `AnimeHistoryTab` collects it and calls `openEpisode(context, getNextEpisode())` (`AnimeHistoryTab.kt:171-175`). `resumeLastChapterReadEvent` (manga) is collected in `MangaHistoryTab.kt:168-172` but NO code sends to it (verified by grep of `app/src/main/java`), so the manga resume-on-reselect path is dead. PROPOSAL: reselect resumes the sub-tab that is currently visible (T-42a-021).

Events channels: `val resumeLastEpisodeSeenEvent = Channel<Unit>()` (`AnimeHistoryTab.kt:43`) and `val resumeLastChapterReadEvent = Channel<Unit>()` (`MangaHistoryTab.kt:41`). Rendezvous channels (capacity 0).

### 1.8 Remove from history (OBSERVED)

Dialog `Dialog.Delete(history)` is opened from the trash button (`MangaHistoryScreen.kt:58`).

`HistoryDeleteDialog(onDismissRequest, onDelete: (Boolean) -> Unit, isManga)` (`app/src/main/java/eu/kanade/presentation/history/HistoryDialogs.kt:24-72`):
- `var removeEverything by remember { mutableStateOf(false) }` (unchecked by default).
- Title: `MR.strings.action_remove` ("Remove").
- Text: a `Column` with `Arrangement.spacedBy(MaterialTheme.padding.small)`:
  - description: manga `MR.strings.dialog_with_checkbox_remove_description` = "This will remove the read date of this chapter. Are you sure?"; anime `AYMR.strings.dialog_with_checkbox_remove_description_anime` = "This will remove the watch date of this episode. Are you sure?".
  - `LabeledCheckbox`: manga `AYMR.strings.dialog_with_checkbox_reset` = "Reset all chapters for this entry"; anime `AYMR.strings.dialog_with_checkbox_reset_anime` = "Reset all episodes for this anime".
- Confirm button (`TextButton`): `MR.strings.action_remove` → `onDelete(removeEverything)` then `onDismissRequest()`.
- Dismiss button: `MR.strings.action_cancel` ("Cancel").

Callback (`MangaHistoryTab.kt:93-105`):
- `all == true` → `screenModel.removeAllFromHistory(dialog.history.mangaId)` → `RemoveMangaHistory.await(mangaId)` → `resetHistoryByMangaId(mangaId)`.
- `all == false` → `screenModel.removeFromHistory(dialog.history)` → `RemoveMangaHistory.await(history)` → `resetMangaHistory(history.id)`.
- Both use `screenModelScope.launchIO`; the repository swallows exceptions (logcat only), returns nothing; no snackbar.

SQL effect (OBSERVED, `history.sq`): `resetHistoryById` = `UPDATE history SET last_read = 0 WHERE _id = :historyId`; `resetHistoryByMangaId` = `UPDATE history SET last_read = 0 WHERE _id IN (SELECT H._id FROM mangas M INNER JOIN chapters C ON M._id = C.manga_id INNER JOIN history H ON C._id = H.chapter_id WHERE M._id = :mangaId)`. The row is KEPT with `last_read = 0` and `time_read` is preserved, so the reading time still counts in the stats total (see 5.2). Anime equivalent: `resetAnimeHistoryById` / `resetHistoryByAnimeId` set `last_seen = 0` (`data/src/main/sqldelightanime/dataanime/animehistory.sq`).

### 1.9 Clear all history (OBSERVED)

App bar action (`MangaHistoryTab.kt:174-181`): `AppBar.Action(title = MR.strings.pref_clear_history ("Clear history"), icon = Icons.Outlined.DeleteSweep, onClick = setDialog(DeleteAll))`. Anime: same with `AnimeHistoryTab.kt:177-184`.

`HistoryDeleteAllDialog` (`HistoryDialogs.kt:74-101`): title `MR.strings.action_remove_everything` ("Remove everything"); text `MR.strings.clear_history_confirmation` ("Are you sure? All history will be lost."); confirm `MR.strings.action_ok` ("OK") → `onDelete()` then dismiss; dismiss `MR.strings.action_cancel`.

`removeAllHistory()` (`MangaHistoryScreenModel.kt:133-139`): `launchIO { val result = removeHistory.awaitAll(); if (!result) return; _events.send(HistoryCleared) }`.
- `awaitAll()` → `deleteAllMangaHistory()` → `DELETE FROM history;` (HARD delete of every row including `time_read`), returns `true`; on exception returns `false` with a logcat error (`MangaHistoryRepositoryImpl.kt:52-60`).
- On success: snackbar `MR.strings.clear_history_completed` ("History deleted"). On failure: no snackbar (silent).
- Scope: manga clears only manga history; anime clears only `animehistory`.

Consequence (OBSERVED): after clear-all the manga "Read duration" stat drops to 0 (its source is `SUM(time_read)`); after per-row remove it does not change.

### 1.10 Add to library from history (OBSERVED)

`addFavorite(mangaId)` (`MangaHistoryScreenModel.kt:184-196`):
1. `manga = getManga.await(mangaId) ?: return`.
2. `duplicate = getDuplicateLibraryManga.await(manga).getOrNull(0)`. If not null → `Dialog.DuplicateManga(manga, duplicate)` and return.
3. Else `addFavorite(manga)`.

`DuplicateMangaDialog` (`app/src/main/java/eu/kanade/presentation/entries/manga/...`, out of scope; wired at `MangaHistoryTab.kt:112-123`): `onConfirm = addFavorite(dialog.manga)` (the overload below, duplicate check skipped); `onOpenManga = push(MangaScreen(duplicate.id))`; `onMigrate = showMigrateDialog(dialog.manga, dialog.duplicate)`.

`addFavorite(manga)` (`:198-227`), inside `launchIO`:
1. `categories = getCategories()` = `GetMangaCategories.await().filterNot { it.isSystemCategory }`.
2. `defaultCategoryId = libraryPreferences.defaultMangaCategory().get().toLong()` (preference default `-1`, `domain/.../library/service/LibraryPreferences.kt:154`).
3. `defaultCategory = categories.find { it.id == defaultCategoryId }`.
4. when:
   - `defaultCategory != null` → `updateManga.awaitUpdateFavorite(id, true)`; if it returns false, stop; then `moveMangaToCategory(id, defaultCategory)` = `SetMangaCategories.await(id, [defaultCategory.id])`.
   - `defaultCategoryId == 0L || categories.isEmpty()` → set favorite; then `SetMangaCategories.await(id, emptyList())` (move to the Default/uncategorized bucket).
   - else → `showChangeCategoryDialog(manga)` (favorite is NOT set yet).
5. Always (unless returned early): `addTracks.bindEnhancedTrackers(manga, sourceManager.getOrStub(manga.source))` — binds enhanced trackers (area:27-tracking-core, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin). Note: this runs even in the change-category branch, before the user chooses.

`showChangeCategoryDialog(manga)` (`:235-248`): `categories = getCategories()` (non-system); `selection = getCategories.await(manga.id).map { it.id }` (categories the manga is already in); dialog `ChangeCategory(manga, initialSelection = categories.mapAsCheckboxState { it.id in selection }.toImmutableList())`. `mapAsCheckboxState` produces `CheckboxState.State` with checked = membership (`core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt:45`).

`ChangeCategoryDialog` (`app/src/main/java/eu/kanade/presentation/category/components/ChangeCategoryDialog.kt`, out of scope): `onConfirm(include, _)` → `moveMangaToCategoriesAndAddToLibrary(manga, include)`; `onEditCategories` → manga: `navigator.push(CategoriesTab); CategoriesTab.showMangaCategory()` (`MangaHistoryTab.kt:128-131`); anime: `navigator.push(CategoriesTab)` only (`AnimeHistoryTab.kt:131`). OBSERVED asymmetry: the anime path does not switch the Categories tab to the anime list.

`moveMangaToCategoriesAndAddToLibrary(manga, categories)` (`:169-177`): `moveMangaToCategory(manga.id, categories)` (launchIO, `SetMangaCategories.await`), then `if (manga.favorite) return`, else launchIO `updateManga.awaitUpdateFavorite(manga.id, true)`. The two launches are unordered. This path does NOT call `bindEnhancedTrackers` (OBSERVED divergence from 1.10 step 5).

`showMigrateDialog(current, duplicate)` (`:229-233`) → `Dialog.Migrate(newManga = current, oldManga = duplicate)`; `MigrateMangaDialog(oldManga, newManga, screenModel = MigrateMangaDialogScreenModel(), onClickTitle = push(MangaScreen(oldManga.id)), onPopScreen = navigator.replace(MangaScreen(newManga.id)))` (`MangaHistoryTab.kt:137-146`). Anime: `onClickSeasons = push(MigrateSeasonSelectScreen(oldAnime, newAnime))` and `onPopScreen = navigator.replace(AnimeScreen(dialog.oldAnime.id))` (`AnimeHistoryTab.kt:137-148`). OBSERVED ASYMMETRY: manga replaces with the NEW entry's screen, anime replaces with the OLD entry's screen. Keep as-is until verified; flagged in edge cases (EC-42-H-17).

### 1.11 Snackbar and internal errors (OBSERVED)

`Event.InternalError` → snackbar `MR.strings.internal_error` = "InternalError: Check crash logs for further information". `Event.HistoryCleared` → `MR.strings.clear_history_completed`. `Event.OpenChapter(null)` → `MR.strings.no_next_chapter`. Anime analogues: `AYMR.strings.no_next_episode` for a null episode. Snackbar host: `SnackbarHost(snackbarHostState)` in `Scaffold` (`MangaHistoryScreen.kt:36-38`). Snackbars are shown with `showSnackbar(message)`; duration default (Compose: Short).

### 1.12 Tab-level dialogs (OBSERVED)

All dialogs in history are routed through `state.dialog` with a `when` (`MangaHistoryTab.kt:91-148`): `Delete`, `DeleteAll`, `DuplicateManga`, `ChangeCategory`, `Migrate`, `null`. Dismiss = `setDialog(null)`. Only one dialog can be open at a time.

---

## 2. Updates feed (part 1 presentation; model owned by part 2)

The feed query and the screen models (`AnimeUpdatesScreenModel.kt`, `MangaUpdatesScreenModel.kt`) are part-2 files. The behavior below is the part of their behavior that the part-1 presentation depends on; part 2 owns the full specification.

### 2.1 Query (OBSERVED)

`GetMangaUpdates.subscribe(instant)` = `repository.subscribeAllMangaUpdates(instant.toEpochMilli(), limit = 500)` (`domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt:17-19`). `GetAnimeUpdates` analogous.

The screen model passes `limit = ZonedDateTime.now().minusMonths(3).toInstant()` (`MangaUpdatesScreenModel.kt:75-76`, `AnimeUpdatesScreenModel.kt:80`). SQL `updatesView` (`data/src/main/sqldelight/view/updatesView.sq`):
```
SELECT mangas._id AS mangaId, mangas.title AS mangaTitle, chapters._id AS chapterId, chapters.name AS chapterName,
 chapters.scanlator, chapters.read, chapters.bookmark, chapters.last_page_read, mangas.source, mangas.favorite,
 mangas.thumbnail_url AS thumbnailUrl, mangas.cover_last_modified AS coverLastModified,
 chapters.date_upload AS dateUpload, chapters.date_fetch AS datefetch
FROM mangas JOIN chapters ON mangas._id = chapters.manga_id
WHERE favorite = 1 AND date_fetch > date_added
ORDER BY date_fetch DESC;
-- getRecentUpdates:  SELECT * FROM updatesView WHERE dateUpload > :after LIMIT :limit;
```
So: favorites only; chapter fetched after the entry was added to the library; ordered by `date_fetch DESC`; window is `date_upload > now - 3 months` (the window uses the UPLOAD date, the grouping uses FETCH date); at most 500 rows.

Anime: `data/src/main/sqldelightanime/view/animeupdatesView.sq` identical with `episodes.seen`, `bookmark`, `fillermark`, `last_second_seen`, `total_seconds AS totalSeconds`.

Mapping (OBSERVED, `data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt:41-74`): `MangaUpdatesWithRelations(mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, lastPageRead, sourceId, dateFetch, coverData = MangaCover(mangaId, sourceId, isMangaFavorite = favorite, url = thumbnailUrl, lastModified = coverLastModified))`. Note `dateUpload` is NOT carried into the domain model.

### 2.2 Presentation state (OBSERVED, from the models and part-1 UI)

- `State(isLoading = true, items = [], dialog = null)` (`MangaUpdatesScreenModel.kt:363-368`). `selected = items.filter { selected }`; `selectionMode = selected.isNotEmpty()`.
- `getUiModel()` groups by `item.update.dateFetch.toLocalDate()` (NOT upload date), inserting headers the same way as history (1.3).
- `isLoading` becomes false at the first emission of the combined flow (`combine(getUpdates.subscribe(limit).distinctUntilChanged(), downloadCache.changes, downloadManager.queueState)`; `collectLatest` → `isLoading = false, items = updates.toUpdateItems()`). Error path: `catch` → `InternalError` event; `isLoading` stays true (OBSERVED: the loading screen then stays forever on error). PROPOSAL: set `isLoading = false` and show the error message on error (T-42a-025).
- Every emission recomputes all rows' download state (section 2.4), so each DB or cache change costs one disk lookup per row.
- `selectedChapterIds` / `selectedEpisodeIds` (a `HashSet<Long>`) survive list refreshes: each new item gets `selected = id in set`.

### 2.3 Last-updated header (OBSERVED)

`lastUpdated by libraryPreferences.lastUpdatedTimestamp().asState(screenModelScope)` (`MangaUpdatesScreenModel.kt:66`). Preference key `library_update_last_timestamp` (app-state key, `LibraryPreferences.kt:39`, default `0`). The row is `item(key = "mangaUpdates-lastUpdated")` at the top of the list (`MangaUpdatesUiItem.kt:52-67`): text `MR.strings.updates_last_update_info` = "Library last updated: %s" with the argument `relativeTimeSpanString(lastUpdated)`, italic.

`relativeTimeSpanString(epochMillis)` (`app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt:35-44`):
- `epochMillis <= 0` → `MR.strings.relative_time_span_never` = "Never".
- `now - epochMillis < 1 minute` → `MR.strings.updates_last_update_info_just_now` = "Just now".
- else → `android.text.format.DateUtils.getRelativeTimeSpanString(epochMillis, now, DateUtils.MINUTE_IN_MILLIS)` (Android relative string such as "5 minutes ago", "1 hour ago", "Yesterday", or a date for older times).

The header is computed during composition, so it does not tick while the screen is open (OBSERVED). PROPOSAL: recompute every 30 s while visible (T-42a-026).

### 2.4 Download state of a row (OBSERVED, `MangaUpdatesScreenModel.kt:103-126`, `AnimeUpdatesScreenModel.kt:107-130`)

For each update row:
1. `activeDownload = downloadManager.getQueuedDownloadOrNull(chapterId)` (or `episodeId`).
2. `downloaded = downloadManager.isChapterDownloaded(chapterName, scanlator, mangaTitle, sourceId)` (anime: `isEpisodeDownloaded(episodeName, scanlator, animeTitle, sourceId)`). Disk/cache lookup by names (area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared).
3. `downloadState = activeDownload?.status ?: (DOWNLOADED if downloaded else NOT_DOWNLOADED)`.
4. Providers: `downloadStateProvider = { downloadState }`, `downloadProgressProvider = { activeDownload?.progress ?: 0 }`.

Live updates (OBSERVED, `updateDownloadState`, `:141-159`): `merge(downloadManager.statusFlow(), downloadManager.progressFlow())` → for the row whose `chapterId == download.chapter.id` (anime: `episode.id`), replace both providers with `{ download.status }` and `{ download.progress }`. Rows not present are ignored. The update is applied via `state.items.mutate` (copy-on-write).

Enum (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt:67-73`; anime identical in `.../download/anime/model/AnimeDownload.kt:62-68`): `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)`.

### 2.5 Selection engine (OBSERVED, `MangaUpdatesScreenModel.kt:266-353`; anime identical with episode ids)

State: `selectedPositions: Array<Int>(2) = [-1, -1]` (first and last selected index), `selectedChapterIds: HashSet<Long>`.

`toggleSelection(item, selected, userSelected = false, fromLongPress = false)`:
1. `selectedIndex = indexOfFirst { chapterId == item.chapterId }`; if < 0 → no-op. If the current `selected` equals the requested value → no-op.
2. `firstSelection = none { it.selected }` (computed BEFORE the update).
3. Set `items[selectedIndex].selected = selected`; add/remove id in the set.
4. If `selected && userSelected && fromLongPress` (long press to select):
   - if `firstSelection` → `positions = [selectedIndex, selectedIndex]`.
   - else if `selectedIndex < positions[0]` → range = `(selectedIndex + 1) until positions[0]`; `positions[0] = selectedIndex`.
   - else if `selectedIndex > positions[1]` → range = `(positions[1] + 1) until selectedIndex`; `positions[1] = selectedIndex`.
   - else range = empty.
   - Every item in range that is not selected becomes selected (and is added to the id set). Range selection = fill the gap between the existing bounds and the long-pressed row.
5. Else if `userSelected && !fromLongPress` (plain tap in selection mode):
   - deselect (`!selected`): if the row was `positions[0]` → `positions[0] = indexOfFirst { selected }`; else if it was `positions[1]` → `positions[1] = indexOfLast { selected }`.
   - select: if `selectedIndex < positions[0]` → `positions[0] = selectedIndex`; else if `> positions[1]` → `positions[1] = selectedIndex`.

Note: `indexOfFirst` returns -1 when nothing is left selected; the bounds are then stale-but-harmless because the next long press with `firstSelection == true` resets them.

`toggleAllSelection(selected)`: sets every row and every id; resets `positions = [-1, -1]`. `invertSelection()`: flips every row, resets positions.

Row gestures (OBSERVED, `MangaUpdatesUiItem.kt` callers at `MangaUpdatesScreen.kt`/`MangaUpdatesUiItem.kt:113-126`):
- Long press → `onUpdateSelected(item, !item.selected, true, true)` + haptic.
- Tap → if selection mode: `onUpdateSelected(item, !item.selected, true, false)`; otherwise `onClickUpdate(item)` (open the chapter/episode; part 2 wiring).
- Cover tap → `onClickCover(item)` only when NOT in selection mode (`onClickCover = ... .takeIf { !selectionMode }`).
- Download indicator → only when NOT in selection mode.
- `BackHandler(enabled = state.selectionMode) { onSelectAll(false) }` (`MangaUpdatesScreen.kt:49`): back clears the selection.
- Pull-to-refresh disabled while in selection mode (`enabled = !state.selectionMode`, `:85`).

### 2.6 Bottom action bar (OBSERVED, `EntryBottomActionMenu(visible = selected.isNotEmpty(), ...)`)

Manga (`MangaUpdatesScreen.kt:116-140`), each action present only when its `takeIf` predicate holds:

| Action | Predicate on the selected list | Callback (ScreenModel) |
|---|---|---|
| Bookmark | `any { !bookmark }` | `bookmarkUpdates(selected, true)` |
| Remove bookmark | `all { bookmark }` | `bookmarkUpdates(selected, false)` |
| Mark as read | `any { !read }` | `markUpdatesRead(selected, true)` |
| Mark as unread | `any { read || lastPageRead > 0 }` | `markUpdatesRead(selected, false)` |
| Download | `any { state != DOWNLOADED }` | `downloadChapters(selected, START)` |
| Delete | `any { state == DOWNLOADED }` | `onMultiDeleteClicked(selected)` (part 2 shows the confirm dialog) |

Anime (`AnimeUpdatesScreen.kt:125-161`): Bookmark / Remove bookmark (same predicates); Fillermark (`any { !fillermark }`) / Remove fillermark (`all { fillermark }`); Mark as viewed (`any { !seen }`); Mark as unviewed (`any { seen || lastSecondSeen > 0 }`); Download (`any != DOWNLOADED`); Delete (`any == DOWNLOADED`); External player (`!alwaysUseExternalPlayer && size == 1`) → `onOpenEpisode(selected[0], true)`; Internal player (`alwaysUseExternalPlayer && size == 1`) → `onOpenEpisode(selected[0], true)`. `isManga = false`.

Bulk operations (OBSERVED):
- `bookmarkUpdates(updates, bookmark)`: `filterNot { it.bookmark == bookmark }` → `ChapterUpdate(id, bookmark)` list → `UpdateChapter.awaitAll` (anime: `EpisodeUpdate(id, bookmark)`); then `toggleAllSelection(false)`.
- `fillermarkUpdates` (anime): same with `fillermark`.
- `markUpdatesRead(updates, read)` (manga): `SetReadStatus.await(read = read, chapters = updates.mapNotNull { getChapter.await(chapterId) }.toTypedArray())`; then clear selection. Anime `markUpdatesSeen` → `SetSeenStatus.await(seen, episodes = ...)`. These run `launchIO` and clear selection immediately (not after completion).

### 2.7 Downloads from updates (OBSERVED)


`ChapterDownloadAction` (`app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadAction`) values used: `START`, `START_NOW`, `CANCEL`, `DELETE`. `EpisodeDownloadAction` adds `SHOW_QUALITIES`.

`downloadChapters(items, action)` (`MangaUpdatesScreenModel.kt:157-181`), `if (items.isEmpty()) return`, `launch`:
- `START`: `downloadChapters(items)` (grouped per manga, below); then `if (items.any { state == ERROR }) downloadManager.startDownloads()`.
- `START_NOW`: `items.singleOrNull()` required; `downloadManager.startDownloadNow(chapterId)`.
- `CANCEL`: single item; `activeDownload = getQueuedDownloadOrNull(id) ?: return`; `cancelQueuedDownloads(listOf(activeDownload))`; `updateDownloadState(activeDownload.apply { status = NOT_DOWNLOADED })` (mutates the shared download object).
- `DELETE`: `deleteChapters(items)` directly (NO confirmation on this per-row path).
- Always: `toggleAllSelection(false)` after the `when`.

`downloadChapters(items)` (`:228-240`, `launchNonCancellable`): group by `mangaId`; for each group: `manga = getManga.await(mangaId) ?: continue`; `sourceManager.get(manga.source) ?: continue` (uninstalled source → skipped silently); `chapters = updates.mapNotNull { getChapter.await(chapterId) }`; `downloadManager.downloadChapters(manga, chapters)`.

Anime `downloadEpisodes(items, alt = false)` (`AnimeUpdatesScreenModel.kt:250-262`) is the same and calls `downloadManager.downloadEpisodes(anime, episodes, true, alt)`. Anime `START_NOW`, `CANCEL` and `SHOW_QUALITIES` (`showQualitiesDialog(update)` → `Dialog.ShowQualities(episodeName, episodeId, animeId, sourceId)`, `:288-297`) follow the same pattern.

Per-row indicator gestures (OBSERVED):
- Manga `ChapterDownloadIndicator` (`app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt`): NOT_DOWNLOADED: tap → `START`, long press → `START_NOW` (`:95-96`); QUEUE/DOWNLOADING: long press → `CANCEL` (`:125`); DOWNLOADED: long press opens a menu containing `DELETE` (`:203-219`); ERROR: tap and long press → `START` (`:238-240`).
- Anime `EpisodeDownloadIndicator` (`app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt`): NOT_DOWNLOADED: tap → `START`, long press → `SHOW_QUALITIES` (`:96-97`) (DIFFERENT from manga: long press shows qualities, not start-now); QUEUE/DOWNLOADING: long press → `CANCEL` (`:126`); DOWNLOADED: long press menu with `DELETE` (`:204-219`); ERROR: tap and long press → `START` (`:239-240`). Start-now is in the menu at `:170`.
- Indicator is disabled (not clickable) when `onDownloadChapter == null` (i.e. in selection mode) (`MangaUpdatesUiItem.kt:235-241`).

### 2.8 Confirm-delete dialog (OBSERVED)

`UpdatesDeleteConfirmationDialog(onDismissRequest, onConfirm, isManga)` (`app/src/main/java/eu/kanade/presentation/updates/UpdatesDialog.kt:11-37`): text = `MR.strings.confirm_delete_chapters` ("Are you sure you want to delete the selected chapters?") or `AYMR.strings.confirm_delete_episodes` ("Are you sure you want to delete the selected episodes?"); confirm `MR.strings.action_ok` → `onConfirm()` then dismiss; dismiss `MR.strings.action_cancel`. No title.

Model: `showConfirmDeleteChapters(items)` / `showConfirmDeleteEpisodes(items)` → `Dialog.DeleteConfirmation(items)`. `deleteChapters(items)` (`:247-260`, `launchNonCancellable`): group by `mangaId`; `manga = getManga.await(mangaId) ?: return@forEach`; `source = sourceManager.get(manga.source) ?: return@forEach`; `chapters = updates.mapNotNull { getChapter }`; `downloadManager.deleteChapters(chapters, manga, source)`; then `toggleAllSelection(false)`. Anime `deleteEpisodes` analogous with `downloadManager.deleteEpisodes(episodes, anime, source)`.

### 2.9 Pull to refresh (OBSERVED, `MangaUpdatesScreen.kt:70-90`)

`PullRefresh(refreshing = isRefreshing, onRefresh, enabled = !selectionMode, indicatorPadding = contentPadding)`. On refresh: `started = onUpdateLibrary()` → `MangaUpdatesScreenModel.updateLibrary()` = `MangaLibraryUpdateJob.startNow(app)`; emits `Event.LibraryUpdateTriggered(started)` (`:128-134`). If not started → return (no spinner). If started → `isRefreshing = true`, `delay(1.seconds)`, `isRefreshing = false`. This is a FAKE spinner: the library update takes minutes but the spinner stops after 1 s (OBSERVED; comment: "Fake refresh status but hide it after a second as it's a long running task"). The `LibraryUpdateTriggered` event is consumed in the part-2 tab (not in part 1).

### 2.10 Empty and loading (OBSERVED)

`isLoading` → `LoadingScreen(Modifier.padding(contentPadding))`; `items.isEmpty()` → `EmptyScreen(MR.strings.information_no_recent)` = "No recent updates" (`MangaUpdatesScreen.kt:63-68`). Note: updates use `information_no_recent` (no per-kind text), history uses per-kind texts.

### 2.11 Badge reset (OBSERVED)

`resetNewUpdatesCount()` writes `0` to `library_unread_updates_count` (manga) or `library_unseen_updates_count` (anime) (`MangaUpdatesScreenModel.kt:359-361`, `AnimeUpdatesScreenModel.kt:392-394`). Called from part-2 `AnimeUpdatesTab.kt:150` and `:153` (not from part 1). Preference defaults `0` (`LibraryPreferences.kt:148-149`).

### 2.12 Row content (OBSERVED)

Manga row (`MangaUpdatesUiItem.kt:140-243`):
- Row `height(56.dp)`, horizontal padding `medium`; `selectedBackground(selected)`; `combinedClickable(onClick, onLongClick + haptic LongPress)`.
- Cover: `ItemCover.Square(modifier = padding(vertical = 6.dp).fillMaxHeight(), data = coverData, onClick = onClickCover)`.
- Column (padding horizontal `medium`, weight 1):
  - Title: `mangaTitle`, `bodyMedium`, `maxLines = 1`, ellipsis, colour alpha = `DISABLED_ALPHA` if `read` else 1.
  - Second line row: unread dot (`Icons.Filled.Circle`, height 8 dp, padding end 4 dp, primary) if `!read` (content description `MR.strings.unread` "Unread"); bookmark icon (`Icons.Filled.Bookmark`, primary, height `textHeight - 2 dp`, then spacer 2 dp) if `bookmark` (content description `MR.strings.action_filter_bookmarked` "Bookmarked"); chapter name (`chapterName`, `bodySmall`, `maxLines = 1`, weight 1 `fill = false`, reports its height via `onTextLayout`); if `readProgress != null`: `DotSeparatorText()` + text (DISABLED_ALPHA).
  - `readProgress = lastPageRead.takeIf { !read && it > 0 }?.let { stringResource(MR.strings.chapter_progress, it + 1) }` → "Page: N" where N = `lastPageRead + 1` (`MangaUpdatesUiItem.kt:105-112`; note the +1: the stored value is 0-based).
- Download indicator (padding start 4 dp).

Anime row (`AnimeUpdatesUiItem.kt:147-263`): identical, plus fillermark icon `Icons.AutoMirrored.Filled.Label` tint `tertiary` (content description `AYMR.strings.action_filter_fillermarked` "Fillermarked") placed after the bookmark icon (`:221-232`); the title/unread dot logic is identical; the watch progress text is `AYMR.strings.episode_progress` = "%1$s/%2$s" with `formatProgress(lastSecondSeen)` and `formatProgress(totalSeconds)`, shown only when `!seen && lastSecondSeen > 0` (`AnimeUpdatesUiItem.kt:111-119`, `:243-251`).

`formatProgress(milliseconds: Long)` (`AnimeUpdatesUiItem.kt:265-283`), OBSERVED:
- If `ms > 3_600_000` → `String.format("%d:%02d:%02d", hours, minutes_in_hour, seconds_in_minute)` where hours = `ms / 3_600_000`, minutes_in_hour = `(ms / 60_000) - hours * 60`, seconds_in_minute = `(ms / 1000) - (ms / 60_000) * 60` (integer math).
- Else → `String.format("%d:%02d", minutes, seconds_in_minute)` with minutes = `ms / 60_000` (no hour part, minutes not zero-padded).
- Note the threshold is strict `>`: exactly 3600000 ms prints `M:SS` with minutes 60 (e.g. "60:00").
- Values are MILLISECONDS even though the column is named `total_seconds` (the anime stats code treats it as ms too, section 5.3). OBSERVED naming hazard.

Last-updated item and the row list keys: `"animeUpdates-lastUpdated"`, `"animeUpdatesHeader-${hashCode}"`, `"animeUpdates-${animeId}-${episodeId}"` (`AnimeUpdatesUiItem.kt:58, 91-95`); manga analogues use `chapterId`.

---

## 3. Tab-level behavior of Updates (part 1 file `UpdatesTab.kt`)

- `Content()` renders `TabbedScreen(titleRes = label_recent_updates, tabs = [animeUpdatesTab(context, fromMore), mangaUpdatesTab(context, fromMore)])` (`UpdatesTab.kt:52-58`). No search (search flags not set).
- `fromMore = currentNavigationStyle() == NavStyle.MOVE_UPDATES_TO_MORE` (`:50`); passed to the sub-tabs for their back-navigation (part 2).
- `LaunchedEffect(Unit) { (context as? MainActivity)?.ready = true }` (`:60-62`) — dropped on Linux.
- `onReselect` pushes `DownloadsTab` (`:43-45`).

---

## 4. Anime and manga tab wrappers (history)

Covered in sections 1.4 to 1.12. The top-level `HistoriesTab.Content()` (`HistoriesTab.kt:52-78`):
- Hoists both screen models (`rememberScreenModel { MangaHistoryScreenModel() }`, `rememberScreenModel { AnimeHistoryScreenModel() }`) so the top bar's search field can read `query` (`:57-61`).
- `TabbedScreen(titleRes = MR.strings.label_recent_manga, tabs = [animeHistoryTab(context, fromMore), mangaHistoryTab(context, fromMore)], mangaSearchQuery, onChangeMangaSearchQuery = mangaHistoryScreenModel::search, animeSearchQuery, onChangeAnimeSearchQuery = animeHistoryScreenModel::search)` (`:63-73`).
- The title key is `MR.strings.label_recent_manga` = "History" (`:64`), so the top-bar title is "History".
- `fromMore = currentNavigationStyle() == NavStyle.MOVE_HISTORY_TO_MORE` (`:55`).

`navigateUp` (manga and anime tabs, `MangaHistoryTab.kt:65-75`, `AnimeHistoryTab.kt:68-78`): only when `fromMore`; if `navigator.lastItem == HomeScreen` → `HomeScreen.openTab(HomeScreen.Tab.AnimeLib())` (manga tab has this exact call, `MangaHistoryTab.kt:68`; the anime tab uses the same call), else `navigator.pop()`.

---

## 5. Statistics

### 5.1 Screen structure (OBSERVED)

`StatsTab` (1 pager, 2 pages). Each page: `AnimeStatsTab`/`MangaStatsTab` wrap a `TabContent` whose content is:
- `StatsScreenState.Loading` → `LoadingScreen()`;
- otherwise `AnimeStatsScreenContent(state as SuccessAnime, paddingValues)` / `MangaStatsScreenContent(state as SuccessManga, ...)` (`AnimeStatsTab.kt:29-38`, `MangaStatsTab.kt:29-38`). `navigateUp = navigator::pop`.

Section order (OBSERVED, `AnimeStatsScreenContent.kt:38-56`, `MangaStatsScreenContent.kt:36-52`), in a `LazyColumn` with `verticalArrangement = spacedBy(MaterialTheme.padding.small)`:
1. Overview.
2. Titles (anime: "Entries"; manga: "Entries").
3. Episodes (anime: `AYMR.strings.episodes` = "Episodes") / Chapters (manga: `MR.strings.chapters` = "Chapters").
4. Trackers.

Section headers: `SectionCard(MR.strings.label_overview_section)` = "Overview"; `SectionCard(MR.strings.label_titles_section)` = "Entries"; anime `SectionCard(AYMR.strings.episodes)`; manga `SectionCard(MR.strings.chapters)`; `SectionCard(MR.strings.label_tracker_section)` = "Trackers".

Each section is a `SectionCard` (`tachiyomi.presentation.core.components.SectionCard`) containing a `Row` of `StatsItem` tiles (equal width, `weight(1f)` each).

Overview row: `Row(modifier = Modifier.height(IntrinsicSize.Min))` with three `StatsOverviewItem` tiles (title style `titleLarge` bold, subtitle `bodyMedium`, plus an icon at the bottom):
- tile 1: value `libraryAnimeCount` / `libraryMangaCount`, caption `MR.strings.in_library` = "In library", icon `Icons.Outlined.CollectionsBookmark`;
- tile 2: value = duration string (5.4), caption anime `AYMR.strings.label_watched_duration` = "Watched duration" / manga `MR.strings.label_read_duration` = "Read duration", icon `Icons.Outlined.Schedule` (anime) — (manga: `Schedule`) ;
- tile 3: value `completedAnimeCount` / `completedMangaCount`, caption `MR.strings.label_completed_titles` = "Completed entries", icon `Icons.Outlined.LocalLibrary`.

Titles row (`StatsItem`, no icon, title `bodyMedium` bold, caption `labelSmall`): `globalUpdateItemCount` caption `MR.strings.label_titles_in_global_update` = "In global update"; `startedXCount` caption `MR.strings.label_started` = "Started"; `localXCount` caption `MR.strings.label_local` = "Local".

Episodes/Chapters row: anime: `totalEpisodeCount` caption `MR.strings.label_total_chapters` = "Total" (sic: uses the chapter key for episodes), `readEpisodeCount` caption `AYMR.strings.label_watched_episodes` = "Watched", `downloadCount` caption `MR.strings.label_downloaded` = "Downloaded". Manga: `totalChapterCount` "Total", `readChapterCount` caption `MR.strings.label_read_chapters` = "Read", `downloadCount` "Downloaded".

Trackers row: `trackedTitleCount` caption `MR.strings.label_tracked_titles` = "Tracked entries"; mean score caption `MR.strings.label_mean_score` = "Mean score"; `trackerCount` caption `MR.strings.label_used` = "Used".

Mean score string (`AnimeStatsScreenContent.kt:141-149`):
- if `trackedTitleCount > 0 && !meanScore.isNaN()` → `"%.2f ★".format(Locale.ENGLISH, meanScore)` (two decimals, English decimal point, U+2605 BLACK STAR);
- else → `MR.strings.not_applicable` = "N/A".

### 5.2 Manga statistics algorithm (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt:42-88`)

Runs once in `init` inside `launchIO`. No flow; the page never refreshes until the screen model is recreated (OBSERVED: stats are a snapshot).

1. `libraryManga = getLibraryManga.await()` (all library rows; one row per (entry, category) pair as produced by the library query, area:37-ui-library, area:08-domain-library-category-history-updates).
2. `distinctLibraryManga = libraryManga.fastDistinctBy { it.id }`.
3. `loggedInTrackers = trackerManager.loggedInTrackers().filter { it is MangaTracker }` (lazy, `:40`).
4. `mangaTrackMap = associate { manga.id -> getMangaTracks.await(manga.id).filter { it.trackerId in loggedInTrackerIds } }` over `distinctLibraryManga` (`:118-126`).
5. `scoredMangaTrackerMap = mangaTrackMap.mapNotNull { (id, tracks) -> tracks.filter { score > 0.0 } → drop empty }` (`:128-136`).
6. `meanScore = scoredMap.map { (_, tracks) -> tracks.map(get10PointScore).average() }.filter { !isNaN }.average()` (`:138-145`). `get10PointScore(track) = trackerManager.get(track.trackerId)!!.mangaService.get10PointScore(track)` (`:147-150`). `average()` of an empty list is NaN.
7. `overview`:
   - `libraryMangaCount = distinct.size`;
   - `completedMangaCount = distinct.count { status.toInt() == SManga.COMPLETED (= 2) && unreadCount == 0L }`;
   - `totalReadDuration = getTotalReadDuration.await()` (sum of `history.time_read`, all rows, see 1.1).
8. `titles`:
   - `globalUpdateItemCount = getGlobalUpdateItemCount(libraryManga)` (5.5);
   - `startedMangaCount = distinct.count { hasStarted }` where `hasStarted = readCount > 0`;
   - `localMangaCount = distinct.count { manga.isLocal() }` (`tachiyomi.source.local.entries.manga.isLocal`, area:14-source-local-and-metadata).
9. `chapters`:
   - `totalChapterCount = distinct.sumOf { totalChapters }.toInt()`;
   - `readChapterCount = distinct.sumOf { readCount }.toInt()`;
   - `downloadCount = downloadManager.getDownloadCount()` = total number of chapter directories across ALL source directories in the download cache, not restricted to the library (`MangaDownloadManager.getDownloadCount()` → `cache.getTotalDownloadCount()`, `MangaDownloadCache.kt:169-176`).
10. `trackers`:
    - `trackedTitleCount = mangaTrackMap.count { it.value.isNotEmpty() }` (entries with at least one logged-in tracker link, scored or not);
    - `meanScore`;
    - `trackerCount = loggedInTrackers.size`.
11. `mutableState.update { SuccessManga(overview, titles, chapters, trackers) }`.

Anime (`app/src/main/java/eu/kanade/tachiyomi/ui/stats/anime/AnimeStatsScreenModel.kt:42-88`) is identical except:
- `libraryAnimeCount`; `completedAnimeCount = status == SAnime.COMPLETED (= 2) && unseenCount == 0L`;
- `totalSeenDuration = getWatchTime(distinct)` (5.3) instead of the history sum;
- `startedAnimeCount = count { hasStarted }` where `hasStarted = seenCount > 0`; `localAnimeCount` with `isLocal()` from `tachiyomi.source.local.entries.anime`;
- episodes: `totalEpisodeCount = sumOf { totalCount }.toInt()`, `readEpisodeCount = sumOf { seenCount }.toInt()`, `downloadCount = downloadManager.getDownloadCount()` (anime cache: `AnimeDownloadCache.getTotalDownloadCount()`, `:165-172`).

Constants: `SManga.COMPLETED = 2` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:50`), `SAnime.COMPLETED = 2` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:74`).

### 5.3 Anime watch time (OBSERVED, `AnimeStatsScreenModel.kt:128-141`)

`watchTime = Σ over distinct library anime, Σ over GetEpisodesByAnimeId(anime.id): if episode.seen then episode.totalSeconds else episode.lastSecondSeen` (Long, milliseconds). One DB query per anime (N+1 pattern). Only library anime are counted. Episodes that were never opened contribute 0. Episodes seen to the end contribute their full `totalSeconds`.

### 5.4 Duration string (OBSERVED, `app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt:14-32`)

`Duration.toDurationString(context, fallback)`, applied to `totalSeenDuration.toDuration(DurationUnit.MILLISECONDS)` (`AnimeStatsScreenContent.kt:65-69`):
- `toComponents { days, hours, minutes, seconds, _ -> ... }` builds a list of parts:
  - if `days != 0` → `MR.strings.day_short` ("%dd");
  - if `hours != 0` → `MR.strings.hour_short` ("%dh");
  - if `minutes != 0 && (days == 0 || hours == 0)` → `MR.strings.minute_short` ("%dm");
  - if `seconds != 0 && days == 0 && hours == 0` → `MR.strings.seconds_short` ("%ds");
- joined with a single space; if the joined string is blank → `fallback` = `MR.strings.none` ("None").
- Examples (derived from the rules): 0 ms → "None"; 5 s → "5s"; 61 min → "1h 1m"; 25 h 3 m → "1d 1h" (minutes dropped because days != 0 and hours != 0); 25 h → "1d 1h"; 1 d 0 h 5 m → "1d 5m"; 1 h 0 m 5 s → "1h".

### 5.5 Global update item count (OBSERVED, `MangaStatsScreenModel.kt:90-116`; anime identical with `anime`, `unseenCount`)

Inputs: `libraryManga` (not distinct), preferences:
- `includedCategories = mangaUpdateCategories()` (string set → Long); `includedManga = libraryManga.filter { category in includedCategories }` if non-empty, else all.
- `excludedCategories = mangaUpdateCategoriesExclude()`; `excludedIds = libraryManga.filter { category in excluded }.map { id }` if non-empty, else empty.
- `updateRestrictions = autoUpdateItemRestrictions()` (string set of `ENTRY_*` tokens; default `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}`, `LibraryPreferences.kt:49-56`).
- Result = `includedManga.filterNot { id in excludedIds }.distinctBy { id }.countNot { (ENTRY_NON_COMPLETED in r && status == COMPLETED) || (ENTRY_HAS_UNVIEWED in r && unreadCount != 0) || (ENTRY_NON_VIEWED in r && totalChapters > 0 && !hasStarted) }`.
- Token values (`LibraryPreferences.kt:403-406`): `ENTRY_NON_COMPLETED = "manga_ongoing"`, `ENTRY_HAS_UNVIEWED = "manga_fully_read"`, `ENTRY_NON_VIEWED = "manga_started"`, `ENTRY_OUTSIDE_RELEASE_PERIOD = "manga_outside_release_period"`. The last token is NOT evaluated here (OBSERVED; a feature gap relative to the auto-update job, area:26-library-update-jobs).
- Anime token equivalents are in `AnimeStatsScreenModel` via the same `LibraryPreferences` constants with `unseenCount` and `seenCount`.
- Semantics: "count of entries that the global update would refresh", approximately (the stats ignore the release-period token).

### 5.6 Tracker mean score (OBSERVED)

See 5.2 steps 4–6. The per-entry average is taken over the entry's logged-in trackers with `score > 0.0`; then the average of entry averages. Scores are normalized to a 10-point scale by the tracker service (`get10PointScore`, areas 27–33). Score-less entries are excluded from the mean but counted in `trackedTitleCount` if linked.

### 5.7 Stats error behavior (OBSERVED)

No `try/catch` in the stats models. An exception in `init` cancels the `launchIO` block; the state stays `Loading` forever (OBSERVED). `get10PointScore` uses `!!` on `trackerManager.get(trackerId)`; a track whose tracker is unknown throws NPE → same forever-Loading outcome. PROPOSAL: catch per-section, show "N/A" for the failing section, log.

---

## 6. Storage

### 6.1 Structure (OBSERVED)

`StorageTab` (2 pages: anime, manga). Each page `AnimeStorageTab`/`MangaStorageTab` renders `StorageScreenContent(state, isManga, contentPadding, onCategorySelected = setSelectedCategory, onDelete = deleteEntry)`.

### 6.2 Aggregation (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt`)

Constructor inputs (`:22-34`): `downloadCacheChanges: SharedFlow<Unit>` (passed `downloadCache.changes`, a `Channel.receiveAsFlow()`), `downloadCacheIsInitializing: StateFlow<Boolean>`, `libraries: Flow<List<T>>` (`getLibraries.subscribe()`), `categories: (Boolean) -> Flow<List<Category>>` (visible vs all, by the hidden-categories setting), `getDownloadSize: T.() -> Long`, `getDownloadCount: T.() -> Int`, `getId`, `getCategoryId`, `getTitle`, `getThumbnail`.

`selectedCategory = MutableStateFlow(AllCategory)` (`:36`). `AllCategory = Category(id = -1L, name = "All", order = 0L, flags = 0L, hidden = false)` (`:114-120`).

`init` (`:38-102`), `launchIO`:
1. `hideHiddenCategories = libraryPreferences.hideHiddenCategoriesSettings().get()` — read ONCE (pref `hidden_categories`, default `false`, `LibraryPreferences.kt:72`). Changing it later has no effect until the screen is reopened (OBSERVED).
2. `combine(downloadCacheChanges, downloadCacheIsInitializing, libraries, categories(hideHidden), selectedCategory) { ... }.collectLatest {}`. For each combined emission:
   a. `mutableState.update { Success(selectedCategory, categories = listOf(AllCategory, *categories), items = emptyList()) }` (resets the list);
   b. `distinctLibraries = libraries.distinctBy { getId() }.filter { ... }`: if `selectedCategory == AllCategory` → keep rows whose `getCategoryId()` matches a category in `categories` (visible/all list; entries in hidden categories are dropped when the setting is on); otherwise keep rows with `categoryId == selectedCategory.id`;
   c. for each library row (`CommonStorageScreenModel.kt:74-87`): `val random = Random(library.getId())`, then `StorageItem(id, title, size = getDownloadSize(), thumbnail, entriesCount = getDownloadCount(), color = Color(random.nextInt(255), random.nextInt(255), random.nextInt(255)))`. The colour is the first three draws of a `kotlin.random.Random` seeded with the entry id, so it is stable across runs for the same id. Each channel is in `0..254`; alpha is 1.0;
   d. `items = (items + item).sortedByDescending { size }` (stable sort: equal sizes keep insertion order, which is the library order).

Note: the per-row `getDownloadSize()` calls are made in step (c) inside the `combine` lambda (main dispatcher of `launchIO`), i.e. blocking disk walks on each emission (OBSERVED; PROPOSAL: compute off the UI thread, cache sizes by download-cache generation).

`setSelectedCategory(category)` = `selectedCategory.update { category }` (`:104-106`). Selection is not persisted.

`deleteEntry(id)` (abstract, implemented in `AnimeStorageScreenModel.kt:41-49` and `MangaStorageScreenModel.kt:41-49`): `launchNonCancellable { anime = getLibraries.await().find { it.id == id }?.anime ?: return; source = sourceManager.get(anime.source) ?: return; downloadManager.deleteAnime(anime, source) }`. There is no confirmation inside the model (the UI confirms, 6.5). The list updates when the download cache emits a change.

Categories shown: `StorageScreenState.Success.categories` = `[All] + categories` where categories come from `GetAnimeCategories`/`GetMangaCategories` or the visible variants. Note that system categories are not filtered here (unlike history's `getCategories()`), so `Category.UNCATEGORIZED_ID` (id 0) appears as an item named "Default" via the mapping in 6.3.

### 6.3 Category selector (OBSERVED, `SelectStorageCategory.kt:12-41`)

`SelectItem(label = AYMR.strings.label_category ("Category"), selectedIndex = mapped.indexOfFirst { id == selectedCategory.id }, options = mapped, toString = { it.name })`. Mapping: `id == -1L` → name `AYMR.strings.label_all` ("All"); `id == Category.UNCATEGORIZED_ID (0)` → name `MR.strings.label_default` ("Default"); else the category name. The mapped array is `remember(categories)`.

### 6.4 Half-donut chart (OBSERVED, `CumulativeStorage.kt:16-75`)

- Parameters: `items`, `modifier`, `borderWidth: Float = 15f`.
- `totalSize = items.sumOf { size }.toFloat()` (remembered); `totalSizeString = totalSize.toLong().toSize()` (6.7).
- Layout: a `Canvas(aspectRatio = 1f)` plus a `Text(totalSizeString, style = displaySmall)`. Measure policy: children measured with `minWidth = 0, minHeight = 0`; layout size = `(constraints.maxWidth, canvas.height / 2)`; canvas placed at (0,0); text placed at `((canvas.width / 2) - text.width/2, (canvas.height / 4) - text.height/2)`.
- Drawing (`onDraw`): if `totalSize == 0f` → draw nothing (the text still shows, "0 B"). Otherwise `totalAngle = 180f`; `rotate(180f) { for item in items: itemAngle = item.size / totalSize * 180; drawArc(color = item.color, startAngle = currentAngle, sweepAngle = itemAngle, useCenter = false, style = Stroke(width = borderWidth, cap = StrokeCap.Round)); currentAngle += itemAngle }`.
- Visual result: arcs occupy the upper half-ring (the 180° rotation maps the 0..180° sweep to the top half), segments in the order of `items` (largest first, as sorted), starting at the left end. Items with size 0 get a zero-length arc with round caps (may render as a dot, OBSERVED design, not verified visually).
- Gaps: none between segments (adjacent arcs with round caps may overlap by one half-stroke width).

### 6.5 Per-entry row (OBSERVED, `StorageItem.kt:51-138`)

- Row (`Arrangement.spacedBy(MaterialTheme.padding.medium)`, vertically centred):
  - `ItemCover.Square(modifier = height(48.dp), data = thumbnail, contentDescription = title)`;
  - Column (weight 1):
    - title: `bodyMedium`, `FontWeight.W700`, `maxLines = 1`, ellipsis;
    - second row (centred): a 12 dp circle filled with `color` (`Modifier.background(color, CircleShape).size(12.dp)`); spacer `padding.small`; size text `item.size.toSize()` (`bodySmall`); a separator dot (`size = padding.small / 2`, horizontal padding `padding.small / 2`, colour `onSurface`, circle); count text = `pluralStringResource(pluralCount, count = entriesCount, entriesCount)` where `pluralCount = MR.plurals.manga_num_chapters` (manga) or `AYMR.plurals.anime_num_episodes` (anime) (`bodySmall`).
  - `IconButton(Icons.Default.Delete, contentDescription = MR.strings.action_delete)` → `showDeleteDialog = true`.
- After each row: `Spacer(height = padding.medium)` (`StorageScreenContent.kt:105`).
- Delete dialog (`ItemDeleteDialog`, `:140-181`): title `MR.strings.delete_downloads_for_manga` ("Delete downloaded chapters?") or `AYMR.strings.delete_downloads_for_anime` ("Delete downloaded episodes?"); text `AYMR.strings.delete_confirmation` formatted with the entry title = `Are you sure you wish to delete "%s"?`; confirm `MR.strings.action_ok` → `onDelete(item.id)` then dismiss; dismiss `MR.strings.action_cancel`.

### 6.6 Page layout (OBSERVED, `StorageScreenContent.kt:36-114`)

- `Loading` → `LoadingScreen(modifier)`.
- `Success` → `Row(padding horizontal small, padding(contentPadding))`:
  - tablet (`isTabletUi()`): left `Info` column with `weight(2f)`, `padding(end = extraLarge)`, full height; right `LazyColumn(weight(3f))`;
  - phone: a single `LazyColumn`: spacer (small), then `Info` (category select + chart), then the rows.
  - `Info` = `Column(centred)`: `SelectStorageCategory` then `CumulativeStorage(modifier = padding(horizontal small, vertical medium) + (padding(bottom medium) on non-tablet))`.
- An empty `items` list shows only `Info` (no empty-state message; OBSERVED gap).

### 6.7 Size format (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt:8-19`)

`Long.toSize()`: decimal (SI) units with 1000 steps: `>= 1000^3` → `"%.2f GB"` of `toFloat()/gb`; `>= 1000^2` → `"%.2f MB"`; `>= 1000` → `"%.2f KB"`; else `"$this B"` (no decimals). Uses the default locale for `String.format` (decimal separator follows the device locale). Note: `KB` (not `kB`).

### 6.8 Storage error and empty behavior

- No error state exists. A thrown exception in `combine` ends the collection; the state stays at the last value (OBSERVED).
- Readiness hazard (OBSERVED, see 9.1): `downloadCacheChanges` is a single-consumer `Channel`. `combine` does not emit until `downloadCacheChanges` has emitted at least once. If the cache already emitted before this screen subscribed and another collector consumed that element, the storage page stays on `Loading` (inferred from `Channel.receiveAsFlow` semantics, `AnimeDownloadCache.kt:75-76`, `:417`). Verify by test (T-42a-054).

---

## 7. Shared UI helpers used by the part-1 files (summary; implementations are area:52-util-app / area:13-core-util-storage-archive-torrent)

| Helper | Location (OBSERVED) | Behavior used here |
|---|---|---|
| `insertSeparators` | `app/src/main/java/eu/kanade/core/util/CollectionUtils.kt:8-21` | Iterates `-1..lastIndex`, adds `before` then the separator returned by `generator(before, after)`. |
| `addOrRemove` | `CollectionUtils.kt:41-47` | Set add or remove. |
| `fastFilterNot`, `fastCountNot` | `CollectionUtils.kt:57`, `:94` | List helpers. |
| `Date.toTimestampString` | `app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:25-27` | Time-only, SHORT, locale. |
| `Long.toLocalDate` / `Instant.toLocalDate` | `DateExtensions.kt:39-45` | System zone. |
| `LocalDate.toRelativeString` | `DateExtensions.kt:52-80` | See 1.6. |
| `relativeDateText` | `app/src/main/java/eu/kanade/presentation/components/DateText.kt:45-63` | See 1.6. |
| `relativeTimeSpanString` | `app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt:35-44` | See 2.3. |
| `Duration.toDurationString` | `TimeUtils.kt:14-32` | See 5.4. |
| `Long.toSize` | `app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt:8-19` | See 6.7. |
| `formatChapterNumber` / `formatEpisodeNumber` | `app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt:11`, `:15` | Used in history subtitles; formatting rules owned by area:52-util-app. |
| `isTabletUi()` | `app/src/main/java/eu/kanade/presentation/util/WindowSize.kt:10` | Storage layout switch only. |
| `selectedBackground(isSelected)` | `presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt:27` | Row highlight in selection. |
| `EntryBottomActionMenu` | `app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt:72` | Bottom bar for selection (2.6). Action labels: bookmark `MR.strings.action_bookmark` / `AYMR.strings.action_bookmark_episode`; remove bookmark (`EntryBottomActionMenu.kt:139-146`); fillermark `AYMR.strings.action_fillermark_episode`; remove fillermark `AYMR.strings.action_remove_fillermark_episode`; mark viewed `MR.strings.action_mark_as_read` / `AYMR.strings.action_mark_as_seen`; mark unviewed `MR.strings.action_mark_as_unread` / `AYMR.strings.action_mark_as_unseen`; download `MR.strings.action_download`; delete `MR.strings.action_delete`; external `AYMR.strings.action_play_externally`; internal `AYMR.strings.action_play_internally`; move category `MR.strings.action_move_category` (`:337`). |
| `PullRefresh` | `presentation-core/.../components/material/PullRefresh.kt:23` | Pull gesture; disabled in selection mode. |
| `FastScrollLazyColumn` | `presentation-core/.../components/LazyList.kt:60` | List with fast-scroll handle. |
| `ListGroupHeader` | `presentation-core/.../components/ListGroupHeader.kt:12` | Date header. |
| `EmptyScreen` / `LoadingScreen` | `presentation-core/.../screens/EmptyScreen.kt:39`, `LoadingScreen.kt:11` | Empty and loading. |
| `SectionCard` | `presentation-core/.../components/SectionCard.kt:18` | Stats card. |
| `SelectItem` | `presentation-core/.../components/SettingsItems.kt:302` / `:408` | Storage category dropdown. |
| `mapAsCheckboxState` | `core/common/.../preference/CheckboxState.kt:45` | Change-category dialog initial state. |
| `Preference.asState` | `app/src/main/java/eu/kanade/core/preference/PreferenceMutableState.kt:38` | Last-updated timestamp as state. |
| `animateItemFastScroll` | `app/src/main/java/eu/kanade/presentation/util/FastScrollAnimateItem.kt:7` | Item placement animation. |

---

## 8. Concurrency and dispatch (OBSERVED summary)

| Work | Dispatcher / scope | Cancellation |
|---|---|---|
| History query subscription | `screenModelScope.launch` with `flowOn(Dispatchers.IO)` on the upstream | Cancelled with the screen model; restarted by `collectLatest` on each new query |
| History search emit | `screenModelScope.launchIO` | Not cancellable on query change (emit only) |
| Resume next (`getNextChapterForManga` and `getNextEpisodeForAnime`) | `launchIO` | Normal |
| Remove, clear, favorite, category moves | `launchIO` | Normal |
| Updates combine (`getUpdates`, `downloadCache.changes`, `queueState`) | `launchIO`, `collectLatest` | Cancels previous compute when a newer emission arrives |
| Updates download-status merge | `launchIO` | Normal |
| Bulk download / delete from updates | `launchNonCancellable` (per-group loop) | Survives screen close (intended: finish the batch) |
| Stats computation | `launchIO` in `init` | Normal; no retry |
| Storage combine | `launchIO`, `collectLatest {}` | Normal |
| Storage delete | `launchNonCancellable` | Survives screen close |
| Snackbar / events | `Channel(UNLIMITED)` or `Channel(Int.MAX_VALUE)` `receiveAsFlow`, collected in `LaunchedEffect(Unit)` in the tab | Events buffered if the tab is not composed |

Mutable state is `MutableStateFlow` inside `StateScreenModel` (Voyager). Updates to state use `update { }` (atomic copy).

---

## 9. Interactions with other areas

- Repositories and SQL: area:06-data-repos-history-track-source-misc (history/updates repositories), area:08-domain-library-category-history-updates (domain interactors, models), area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations (schema, views).
- Downloads: area:23-downloads-manga (manga download manager and cache), area:24-downloads-anime (anime), area:25-downloads-shared (shared queue and status flows). This area reads `getQueuedDownloadOrNull`, `isChapterDownloaded` / `isEpisodeDownloaded`, `statusFlow()`, `progressFlow()`, `queueState`, `cache.changes`, `cache.isInitializing`, `getDownloadCount()`, `getDownloadSize(entry)`, `deleteChapters/deleteEpisodes/deleteManga/deleteAnime`, `startDownloads()`, `startDownloadNow()`, `cancelQueuedDownloads()`, `downloadChapters/downloadEpisodes`.
- Library update job: area:26-library-update-jobs (`startNow(app): Boolean`, `lastUpdatedTimestamp`, `newXUpdatesCount`, restriction tokens).
- Categories: area:08-domain-library-category-history-updates (`GetMangaCategories`, `GetVisibleMangaCategories`, `SetMangaCategories`, `Category` model).
- Entries: area:05-data-repos-entries-items / area:07-domain-entries-items (`GetManga`, `GetLibraryManga`, `GetDuplicateLibraryManga`, `UpdateManga.awaitUpdateFavorite`), area:38-ui-entry-details-manga, area:39-ui-entry-details-anime, area:40-ui-browse-manga, area:41-ui-browse-anime (`MangaScreen`, `AnimeScreen`, migration dialogs, duplicate dialog, change-category dialog, `ItemCover`, indicators).
- Chapters and episodes: area:07-domain-entries-items (`GetChaptersByMangaId`, `GetEpisodesByAnimeId`, `getChapterSort`, `getEpisodeSort`, `UpdateChapter`, `UpdateEpisode`, `ChapterUpdate`, `EpisodeUpdate`, `SetReadStatus`, `SetSeenStatus`).
- Tracking: area:27-tracking-core (`AddMangaTracks`, `AddAnimeTracks`, `bindEnhancedTrackers`, `GetMangaTracks`, `GetAnimeTracks`, `TrackerManager.loggedInTrackers()`), area:28-tracker-anilist, area:29-tracker-myanimelist, area:30-tracker-kitsu, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin (`get10PointScore`).
- Sources: area:17-source-app-layer (`MangaSourceManager.get`, `getOrStub`).
- Player: area:45-player-core (`MainActivity.startPlayerActivity`, external vs internal), area:46-player-controls-ui if present (`PlayerPreferences`).
- Reader: area:36-ui-shell-navigation-security-webview / area:43-reader-core, area:44-reader-viewers (`ReaderActivity.newIntent`).
- Navigation: area:36-ui-shell-navigation-security-webview (`TabContent`, `Tab`, `NavStyle`, `HomeScreen`, `CategoriesTab`, `DownloadsTab`).
- Preferences: area:02-preferences-system (`LibraryPreferences`, `UiPreferences`, `DownloadPreferences`, `PlayerPreferences`).
- Local source detection: area:14-source-local-and-metadata (`isLocal()`).

---

## 10. Dropped Android behavior (summary; full list in p1-rust-design.md section 9)

- Haptic feedback on long press (`LocalHapticFeedback`, `HapticFeedbackType.LongPress`) in `AnimeUpdatesUiItem.kt:170` and `MangaUpdatesUiItem.kt:163`. Linux: no equivalent; keep the long-press gesture only.
- `MainActivity.ready` splash hand-off (`MangaHistoryTab.kt:150-154`, `AnimeHistoryTab.kt:153-157`, `HistoriesTab.kt:75-77`, `UpdatesTab.kt:60-62`, `StatsTab.kt:52-54`, `StorageTab.kt:52-54`).
- Android `DateUtils.getRelativeTimeSpanString` (replaced; see rust design).
- `DateFormat.getTimeInstance(SHORT)` locale time (replaced by chrono localised time; see rust design).
- `android.text.format` and `java.util.Locale` `String.format` with device locale (replaced by explicit formatting).
- `PreviewParameterProvider` previews (Compose tooling only).
- Voyager `Screen`, `LocalNavigator`, `TabOptions`, `AnimatedImageVector` (replaced by the GTK navigation of area:36-ui-shell-navigation-security-webview).
