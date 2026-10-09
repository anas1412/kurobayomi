# Area 38 part 1: data models, flags, preferences, files, endpoints, tables

All paths repo-relative to `/home/blackbox/aniyomi-src`. Types are Kotlin unless stated. "Nullable" means the Kotlin type ends in `?`.

---

## 1. Domain models read by this area

### 1.1 Manga (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`)

`data class Manga` (lines 13-35), `Serializable`:

| Field | Type | Nullable | Used by | Notes |
|---|---|---|---|---|
| `id` | `Long` | no | all | primary key |
| `source` | `Long` | no | `isLocal()` | local source id is `LocalMangaSource.ID` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:385`) |
| `favorite` | `Boolean` | no | library button, gates edit category / interval / migrate | |
| `lastUpdate` | `Long` | no | not read here | |
| `nextUpdate` | `Long` | no | `expectedNextUpdate` | epoch ms |
| `fetchInterval` | `Int` | no | `isUserIntervalMode = fetchInterval < 0`, dialog initial value | negative = user-chosen |
| `dateAdded` | `Long` | no | not read here | |
| `viewerFlags` | `Long` | no | not read here | |
| `chapterFlags` | `Long` | no | sort / filter / display (section 2) | |
| `coverLastModified` | `Long` | no | cover cache busting | |
| `url` | `String` | no | `toSManga()` | |
| `title` | `String` | no | title text, share, cover file name | may be blank |
| `artist` | `String?` | yes | artist row | |
| `author` | `String?` | yes | author row | |
| `description` | `String?` | yes | description | |
| `genre` | `List<String>?` | yes | tag chips | |
| `status` | `Long` | no | status icon/text | matches `SManga` constants |
| `thumbnailUrl` | `String?` | yes | cover URL (via source) | |
| `updateStrategy` | `UpdateStrategy` | no | not read here | |
| `initialized` | `Boolean` | no | first-open refresh gate | |
| `lastModifiedAt` | `Long` | no | | |
| `favoriteModifiedAt` | `Long?` | yes | | |
| `version` | `Long` | no | | |

Derived members:

- `expectedNextUpdate: Instant?` = `nextUpdate.takeIf { status != SManga.COMPLETED.toLong() }?.let { Instant.ofEpochMilli(it) }` (Manga.kt:17-23). Returns null when `status == 2`.
- `sorting: Long` = `chapterFlags and CHAPTER_SORTING_MASK`.
- `displayMode: Long` = `chapterFlags and CHAPTER_DISPLAY_MASK`.
- `unreadFilterRaw`, `downloadedFilterRaw`, `bookmarkedFilterRaw`: `chapterFlags and <mask>`.
- `unreadFilter: TriState`, `bookmarkedFilter: TriState` (Manga.kt:57-70): `when(raw) { SHOW_UNREAD -> ENABLED_IS; SHOW_READ -> ENABLED_NOT; else -> DISABLED }` (unread); `SHOW_BOOKMARKED -> ENABLED_IS; SHOW_NOT_BOOKMARKED -> ENABLED_NOT; else -> DISABLED` (bookmarked).
- `sortDescending(): Boolean` = `(chapterFlags and CHAPTER_SORT_DIR_MASK) == CHAPTER_SORT_DESC` (Manga.kt:71-73). **Note: `true` when bit 0 is 0.**

Extension members (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt`):

- `Manga.downloadedFilter: TriState` (line 23-31): if `BasePreferences.downloadedOnly()` is true → `ENABLED_IS`. Else `when(downloadedFilterRaw) { SHOW_DOWNLOADED -> ENABLED_IS; SHOW_NOT_DOWNLOADED -> ENABLED_NOT; else -> DISABLED }`.
- `Manga.chaptersFiltered(): Boolean` (line 32-36) = unread ≠ DISABLED || downloaded ≠ DISABLED || bookmarked ≠ DISABLED.
- `Manga.toSManga()` (line 38+).

Companion constants (Manga.kt:75-103): see section 2.

### 1.2 Chapter (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt`)

`data class Chapter` (lines 3-17):

| Field | Type | Nullable | Notes |
|---|---|---|---|
| `id` | `Long` | no | `-1` in `Chapter.create()` |
| `mangaId` | `Long` | no | used for reader intent |
| `read` | `Boolean` | no | |
| `bookmark` | `Boolean` | no | |
| `lastPageRead` | `Long` | no | 0-based page index; UI shows `+1` |
| `dateFetch` | `Long` | no | epoch ms, not shown here |
| `sourceOrder` | `Long` | no | source list index, used by SOURCE sort |
| `url` | `String` | no | |
| `name` | `String` | no | shown when display mode is NAME; alphabetical sort key |
| `dateUpload` | `Long` | no | epoch ms; upload-date sort key; relative date text |
| `chapterNumber` | `Double` | no | `-1.0` = unknown |
| `scanlator` | `String?` | yes | blank treated as absent in UI |
| `lastModifiedAt` | `Long` | no | |
| `version` | `Long` | no | |

Members: `isRecognizedNumber: Boolean = chapterNumber >= 0f` (Chapter.kt:19-20).

### 1.3 ChapterList (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:1226-1243`)

```
@Immutable sealed class ChapterList
  @Immutable data class MissingCount(val id: String, val count: Int) : ChapterList()
  @Immutable data class Item(
      val chapter: Chapter,
      val downloadState: MangaDownload.State,
      val downloadProgress: Int,
      val selected: Boolean = false,
  ) : ChapterList() {
      val id = chapter.id                 // Long
      val isDownloaded = downloadState == MangaDownload.State.DOWNLOADED
  }
```

`MissingCount.id` format: `"${lower?.id}-${higher.id}"` (MangaScreenModel.kt:1182-1185). With `lower == null` the string starts with `"null-"`.

### 1.4 MangaScreenModel.State (MangaScreenModel.kt:1138-1225)

- `sealed interface State`: `data object Loading`, `data class Success(...)` (fields in behavior section 6.1).
- `State.Success` is the only state the UI renders; it is replaced wholesale on each update through `updateSuccessState { copy(...) }`.

### 1.5 Dialog (`MangaScreenModel.kt:1094-1106`)

```
sealed interface Dialog {
  data class ChangeCategory(val manga: Manga, val initialSelection: List<CheckboxState.State<Category>>) : Dialog   // lines 1095-1098, fields partially shown
  data class DeleteChapters(val chapters: List<Chapter>) : Dialog
  data class DuplicateManga(val manga: Manga, val duplicate: Manga) : Dialog
  data class Migrate(val newManga: Manga, val oldManga: Manga) : Dialog
  data class SetMangaFetchInterval(val manga: Manga) : Dialog
  data object SettingsSheet : Dialog
  data object TrackSheet : Dialog
  data object FullCover : Dialog
}
```

`ChangeCategory` fields 1096-1097 were not fully read; the UI uses `dialog.initialSelection` and `dialog.manga` (ui MangaScreen.kt:190, 197). The exact `initialSelection` element type is `CheckboxState.State<Category>` per the import list (`tachiyomi.core.common.preference.CheckboxState`, MangaScreenModel.kt:60). Verify in part 2.

Dialog setters: `showChangeCategoryDialog` (382), `showSetMangaFetchIntervalDialog` (398), `dismissDialog` (1108), `showDeleteChapterDialog` (1112), `showSettingsDialog` (1116), `showTrackDialog` (1120), `showCoverDialog` (1124), `showMigrateDialog(duplicate)` (1128-1130).

### 1.6 MangaCoverScreenModel state

`StateScreenModel<Manga?>(null)`. Holds the latest `Manga` from `GetManga.subscribe(mangaId)`. `null` until first emission (drives the loading placeholder).

---

## 2. Chapter flags bit layout (Manga.chapterFlags, Long)

Source: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:75-103` (companion object). Values verbatim:

| Constant | Hex value | Meaning |
|---|---|---|
| `SHOW_ALL` | `0x00000000L` | generic "no filter" |
| `CHAPTER_SORT_DESC` | `0x00000000L` | direction bit 0 = 0 |
| `CHAPTER_SORT_ASC` | `0x00000001L` | direction bit 0 = 1 |
| `CHAPTER_SORT_DIR_MASK` | `0x00000001L` | bit 0 |
| `CHAPTER_SHOW_UNREAD` | `0x00000002L` | bits 1-2 value |
| `CHAPTER_SHOW_READ` | `0x00000004L` | bits 1-2 value |
| `CHAPTER_UNREAD_MASK` | `0x00000006L` | bits 1-2 |
| `CHAPTER_SHOW_DOWNLOADED` | `0x00000008L` | bits 3-4 value |
| `CHAPTER_SHOW_NOT_DOWNLOADED` | `0x00000010L` | bits 3-4 value |
| `CHAPTER_DOWNLOADED_MASK` | `0x00000018L` | bits 3-4 |
| `CHAPTER_SHOW_BOOKMARKED` | `0x00000020L` | bits 5-6 value |
| `CHAPTER_SHOW_NOT_BOOKMARKED` | `0x00000040L` | bits 5-6 value |
| `CHAPTER_BOOKMARKED_MASK` | `0x00000060L` | bits 5-6 |
| `CHAPTER_SORTING_SOURCE` | `0x00000000L` | bits 8-9 value 0 |
| `CHAPTER_SORTING_NUMBER` | `0x00000100L` | bits 8-9 value 1 |
| `CHAPTER_SORTING_UPLOAD_DATE` | `0x00000200L` | bits 8-9 value 2 |
| `CHAPTER_SORTING_ALPHABET` | `0x00000300L` | bits 8-9 value 3 |
| `CHAPTER_SORTING_MASK` | `0x00000300L` | bits 8-9 |
| `CHAPTER_DISPLAY_NAME` | `0x00000000L` | bit 20 = 0 |
| `CHAPTER_DISPLAY_NUMBER` | `0x00100000L` | bit 20 = 1 |
| `CHAPTER_DISPLAY_MASK` | `0x00100000L` | bit 20 |

Bit map (LSB = bit 0):

```
bit  20          9 8     6 5     4 3     2 1     0
     display     sort    bkmk    dl      unread  dir
```

Setting a field: `newFlags = (flags and mask.inv()) or (value and mask)` (`SetMangaChapterFlags.kt`'s private `Long.setFlag(flag, mask)`; the same helper pattern is shown in `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeSeasonFlags.kt:186-188`; the manga version lives in `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/SetMangaChapterFlags.kt`, private, same body).

Defaults: a manga whose `chapterFlags == 0` has: unread filter DISABLED, bookmarked DISABLED, downloaded DISABLED, sorting SOURCE, direction DESC (`sortDescending() == true`), display NAME. Effective order: `getChapterSort` with `sortDescending = true` on SOURCE → ascending `sourceOrder`. Default lists show source order as delivered (newest first on most sources).

Unused bit ranges (must be preserved on write): bits 10-19, bits 21-63. Writes must never clear them. The `setFlag` pattern guarantees this.

Interactor functions and the bits they touch (`SetMangaChapterFlags.kt`):

- `awaitSetUnreadFilter(manga, flag)` (lines 20-27): mask `CHAPTER_UNREAD_MASK`.
- `awaitSetBookmarkFilter(manga, flag)` (29-36): mask `CHAPTER_BOOKMARKED_MASK`.
- `awaitSetDownloadedFilter(...)`: mask `CHAPTER_DOWNLOADED_MASK` (not read in full; same pattern).
- `awaitSetDisplayMode(manga, flag)` (38-45): mask `CHAPTER_DISPLAY_MASK`.
- `awaitSetSortingModeOrFlipOrder(manga, flag)` (47-68): if `manga.sorting == flag` → flip bit 0 (`orderFlag = if (sortDescending()) ASC else DESC`, masked by DIR_MASK). Else → set `flag` under `CHAPTER_SORTING_MASK` and `CHAPTER_SORT_ASC` under `CHAPTER_SORT_DIR_MASK`.
- `awaitSetAllFlags(...)` (line 70+): writes all of them at once (used by defaults; part 2).

Each write goes through `mangaRepository.updateManga(MangaUpdate(id, chapterFlags = newFlags))`.

---

## 3. Download state values

`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt:67-73`:

| Enum | Value | UI use |
|---|---|---|
| `NOT_DOWNLOADED` | 0 | download icon, start |
| `QUEUE` | 1 | spinner (indeterminate) |
| `DOWNLOADING` | 2 | ring with progress, or indeterminate when progress is 0 |
| `DOWNLOADED` | 3 | check icon, delete |
| `ERROR` | 4 | error icon, retry |

`MangaDownload.totalProgress: Int` = sum of page progress (`pages?.sumOf(Page::progress) ?: 0`). The UI's `downloadProgress` is expected to be 0-100 (`downloadProgress / 100f`). `progress` for the chapter row comes from `ChapterList.Item.downloadProgress` (computed in part 2 `toChapterListItems`, MangaScreenModel.kt:533-567).

Download progress flow (for reference): `MangaDownload.progressFlow` averages page progress with `combine(...).average().toInt()` after `pages` becomes non-null (`MangaDownload.kt:~50-60`).

---

## 4. Enums and constant tables (presentation)

| Enum / value set | Source | Values (in order) |
|---|---|---|
| `DownloadAction` | `app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt:3-9` | `NEXT_1_ITEM`, `NEXT_5_ITEMS`, `NEXT_10_ITEMS`, `NEXT_25_ITEMS`, `UNVIEWED_ITEMS` |
| `EditCoverAction` | same, 11-14 | `EDIT`, `DELETE` |
| `EntryScreenItem` | same, 16-23 | `INFO_BOX`, `ACTION_ROW`, `DESCRIPTION_WITH_TAG`, `ITEM_HEADER`, `ITEM`, `AIRING_TIME` (AIRING_TIME is anime only) |
| `ChapterDownloadAction` | `app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt:42-47` | `START`, `START_NOW`, `CANCEL`, `DELETE` |
| `ItemCover` | `app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt:18-22` | `Square(1f/1f)`, `Book(2f/3f)`, `Thumb(16f/9f)`, each with `ratio: Float` |
| `LibraryPreferences.ChapterSwipeAction` | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:391-396` | `ToggleRead`, `ToggleBookmark`, `Download`, `Disabled` |
| `TriState` | `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:3-7` | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` |
| `SManga` status | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54` | `UNKNOWN=0`, `ONGOING=1`, `COMPLETED=2`, `LICENSED=3`, `PUBLISHING_FINISHED=4`, `CANCELLED=5`, `ON_HIATUS=6` |
| `MangaDownload.State` | see section 3 | 0..4 |
| `Manga.CHAPTER_DISPLAY_*` | section 2 | |

Color and size constants (presentation, not in this area's files):

- `ItemCover.CoverPlaceholderColor = Color(0x1F888888)` (ItemCover.kt:55): ARGB, alpha 0x1F (≈12%) gray.
- Error image resource `R.drawable.cover_error` (ItemCover.kt:35).
- `DefaultTagChipModifier = padding(vertical = 4.dp)` (MangaInfoHeader.kt:642).
- `whitespaceLineRegex = Regex("[\\r\\n]{2,}", RegexOption.MULTILINE)` (MangaInfoHeader.kt:99).
- Scrim height 24dp (MangaInfoHeader.kt:625).
- Blur 4dp and alpha 0.2 on backdrop (MangaInfoHeader.kt:133-134).
- Action button label `fontSize = 12.sp`; icon 20dp; gap 4dp (MangaInfoHeader.kt:679-683).
- Chip label `bodySmall`.
- Swipe threshold 56dp (MangaChapterListItem.kt:242). Swipe icon padding 16dp (line 230).
- Row padding start 16 / top 12 / end 8 / bottom 12 (MangaChapterListItem.kt:100).
- Downloaded/unread dot size 8dp (MangaChapterListItem.kt:117).
- Bookmark icon max height = `textHeight - 2.dp` (line 127).
- Bottom bar: row padding horizontal 8dp, vertical 12dp; button size 48dp; confirm label reveal 1000 ms (`1.seconds`, EntryBottomActionMenu.kt:115).
- Cover dialog pills: alpha 0.95 background (MangaCoverDialog.kt:204); dropdown offset 8dp; zoom duration 500 ms (line 185).
- Scanlator dialog: no explicit size; `DialogProperties(usePlatformDefaultWidth = true)`.
- Interval wheel: `DpSize(width = maxWidth / 2, height = 128.dp)` (ItemsDialogs.kt:133).
- Duplicate dialog: `PaddingSize = 16.dp`, `ButtonPadding = PaddingValues(top = 16.dp, bottom = 16.dp)`, `TitlePadding = PaddingValues(bottom = 16.dp, top = 8.dp)` (DuplicateMangaDialog.kt:124-127).
- Chapter settings: sort-item and radio item sizes come from presentation-core (not read).
- Summary animation label `"summary"`; title fade labels `"Top Bar Title"` and `"Top Bar Background"`; progress animation `ProgressIndicatorDefaults.ProgressAnimationSpec`.

---

## 5. Preference keys used in this area

Preference store: `tachiyomi.core.common.preference` (`PreferenceStore`). Keys are literal strings.

| Key (literal) | Owner class / getter | Type | Default | Read by | Behaviour |
|---|---|---|---|---|---|
| `pref_chapter_swipe_end_action` | `LibraryPreferences.swipeChapterStartAction()` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:369-370`) | enum `ChapterSwipeAction` | `ToggleRead` | `MangaScreenModel.kt:145` as `chapterSwipeEndAction` | **Swipe to the end (right-to-left).** Name-swap note: the getter name `swipeChapterStartAction` reads the *end* key. |
| `pref_chapter_swipe_start_action` | `LibraryPreferences.swipeChapterEndAction()` (lines 372-374) | enum `ChapterSwipeAction` | `ToggleBookmark` | `MangaScreenModel.kt:144` as `chapterSwipeStartAction` | **Swipe to the start (left-to-right).** |
| `skip_filtered` | `ReaderPreferences.skipFiltered()` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:85`) | Boolean | `true` | `MangaScreenModel.kt:148`, controls next-unread (part 2) | |
| `pref_downloaded_only` (via `Preference.appStateKey`) | `BasePreferences.downloadedOnly()` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-18`) | Boolean | `false` | ChapterSettingsDialog.kt:70, Manga.downloadedFilter | Locks the Downloaded filter to ENABLED_IS and disables its toggle. |
| `library_update_manga_restriction` | `LibraryPreferences.autoUpdateItemRestrictions()` (`LibraryPreferences.kt:49-57`) | Set<String> | `{ENTRY_HAS_UNVIEWED, ENTRY_NON_COMPLETED, ENTRY_NON_VIEWED, ENTRY_OUTSIDE_RELEASE_PERIOD}` | `MangaScreenModel.kt:150-151` → `isUpdateIntervalEnabled` | Interval dialog's confirm is wired only when `"manga_outside_release_period"` is in the set. |
| `manga_outside_release_period` (`LibraryPreferences.ENTRY_OUTSIDE_RELEASE_PERIOD`) | `LibraryPreferences.kt:406` | string set member | member of the default set | same | |
| `default_manga_category` (via `DEFAULT_MANGA_CATEGORY_PREF_KEY`) | `LibraryPreferences.defaultMangaCategory()` (LibraryPreferences.kt:154) | Int | `-1` | `MangaScreenModel.kt:349` (toggleFavorite) | `-1` means "no default". Part 2. |
| (literal not read) via `TrackPreferences.trackOnAddingToLibrary()` | TrackPreferences (not read here) | Boolean | not read | `autoOpenTrack` (MangaScreenModel.kt:158-160) | Key literal to be confirmed in part 2. |
| (literal not read) via `TrackPreferences.autoUpdateTrackOnMarkRead()` | TrackPreferences | Boolean | not read | `autoTrackState` (MangaScreenModel.kt:146) | Part 2. |
| `pref_chapter_settings_default*` (via `libraryPreferences.setChapterSettingsDefault(manga)`) | LibraryPreferences | — | — | MangaScreenModel.kt:953 | Writes the default chapter flags. Key name not read; part 2. |
| `isReleaseBuildType` (build flag, not a preference) | `eu.kanade.tachiyomi.util.system.isReleaseBuildType` | Boolean | — | ItemsDialogs.kt:126 | Gates the custom interval wheel to debug builds. |

Other preference getters touched only by the anime side of shared components: `PlayerPreferences.alwaysUseExternalPlayer()` (EntryBottomActionMenu.kt:222, 231). Not used for manga.

Setting write paths: `chapter_settings_as_default` and the "apply to all manga" option write `LibraryPreferences` via `setChapterSettingsDefault` and `setMangaDefaultChapterFlags.awaitAll()` (part 2).

---

## 6. Files, folders, formats on disk

This area itself writes no files directly. Indirect writes:

| Trigger | Written by | Location / format | Owner |
|---|---|---|---|
| Save cover | `ImageSaver.save(Image.Cover(bitmap, name, Location.Pictures.create()))` (MangaCoverScreenModel.kt:104-110) | Pictures location; image file | area:34-images-covers-cache (`app/src/main/java/eu/kanade/tachiyomi/data/saver/`) |
| Share cover | same with `Location.Cache` (temporary) | cache dir image file, shared via `Uri.toShareIntent` | area:34 |
| Edit cover | `manga.editCover(Injekt.get(), stream, updateManga, coverCache)` (MangaCoverScreenModel.kt:125; def `app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt:52`) | custom cover file in cover cache | area:34 |
| Delete custom cover | `coverCache.deleteCustomCover(mangaId)` (MangaCoverScreenModel.kt:138) | removes the custom cover file | area:34 |
| Download chapter | `runChapterDownloadActions` → download manager | downloads folder, CBZ/dir layout | area:23, area:25 |
| Delete chapter downloads | part 2 `deleteChapters` | removes downloaded chapter files | area:23, area:25 |

Linux mapping for files:

- Cover share temp: `$XDG_CACHE_HOME/kurobayomi/share/` (proposal).
- Cover save: `glib::user_special_dir(UserDirectory::Pictures)` + `/Kurobayomi/` (proposal; the Android location string is not confirmed in this pass).

---

## 7. Network endpoints touched

This area does **not** call network APIs directly. All network work is behind the source interface:

| Call | Source reference | Protocol | Notes |
|---|---|---|---|
| `source.getMangaDetails(sManga)` | MangaScreenModel.kt:279 | source-defined (HTTP for `HttpSource`) | part 2; errors shown as snackbar |
| `source.getChapterList(sManga)` | MangaScreenModel.kt:573 | source-defined | part 2 |
| `source.getMangaUrl(sManga)` | ui MangaScreen.kt:320 and 410; `HttpSource` | pure URL construction, no I/O | Used for WebView, share, copy. Defined in `source-api` (area:15/16/17). |
| Cover image | Coil `ImageRequest.Builder(...).data(manga)` (MangaInfoHeader.kt:118-135, 350-355, 389-396; MangaCoverDialog.kt:170-173; MangaCoverScreenModel.kt:94-97) | HTTPS GET of `manga.thumbnailUrl` (via Coil fetcher; `data(manga)` uses Coil's `Manga` keyer, defined in area:34) | Coil `crossfade(true)`; full cover uses `Size.ORIGINAL` and `CachePolicy.DISABLED` for memory cache in the viewer. |

Coil keyers/fetchers for `Manga`: location not verified in this pass (owned by area:34-images-covers-cache). Their behaviour on `coverLastModified` (cache key) matters for cover refresh after edit; see area:34.

No endpoint in this scope uses auth headers. Source-level HTTP headers are owned by area:12 and area:15.

---

## 8. Database tables and columns touched

This area reads and writes through repositories/interactors. The table and column names below are the ones the interactors use (verified where read; others marked).

| Table (SQLDelight / DB) | Columns touched | Operation | Via | Owner |
|---|---|---|---|---|
| `mangas` | `chapter_flags` | update | `MangaRepository.updateManga(MangaUpdate(id, chapterFlags))` (SetMangaChapterFlags.kt:20-68) | area:03/05 |
| `mangas` | `favorite`, `favorite_modified_at`, `date_added`, `fetch_interval`, `initialized`, ... | update | `updateManga.awaitUpdateFavorite`, `awaitUpdateFromSource`, `setFetchInterval` | area:05 |
| `mangas` | `cover_last_modified` | update | `updateManga.awaitUpdateCoverLastModified(mangaId)` (MangaCoverScreenModel.kt:139) | area:05 |
| `chapters` | `read`, `last_page_read`, `bookmark` | update | `setReadStatus`, `updateChapter`, `bookmarkChapters` (part 2) | area:05 |
| `chapters` | — (read) | select | `getMangaAndChapters.subscribe(mangaId, applyScanlatorFilter = true)` (MangaScreenModel.kt:182-188) | area:05 |
| `excluded_scanlators` (or equivalent) | read/write | select / replace | `getExcludedScanlators.subscribe`, `setExcludedScanlators` (MangaScreenModel.kt:1133) | area:05 / area:07 — table name not read |
| `scanlators` (available set) | read | select | `getAvailableScanlators.subscribe(mangaId)` | area:05 — table name not read |

Query semantics relevant to the UI (from the flow): `applyScanlatorFilter = true` means the repo excludes chapters whose scanlator is in the excluded set before the list reaches the ScreenModel. Exact SQL: part 2 / area:05.

---

## 9. Data flow diagram (textual)

```
DB (chapters, mangas, scanlators)
  -> GetMangaWithChapters.subscribe(mangaId, applyScanlatorFilter = true)     [flow]
  -> combine(..., downloadCache.changes, downloadManager.queueState)          [flow]
  -> toChapterListItems(manga)   // download state per chapter (part 2)
  -> State.Success.chapters: List<ChapterList.Item>
  -> State.Success.processedChapters = applyFilters(manga)                   [lazy]
       unread -> bookmark -> downloaded -> sort(getChapterSort(manga))
  -> State.Success.chapterListItems = insertSeparators(MissingCount)         [lazy]
  -> UI: LazyColumn items; ItemHeader count = processedChapters.size
```

---

## 10. Settings-screen destinations referenced

- `SettingsScreen.Destination.Tracking` (ui MangaScreen.kt:147) — tracking settings page (area:36 / area:27).
- `MangaSourcePreferencesScreen(sourceId)` (presentation MangaScreen.kt:145) — per-source preferences, area:17.
- `CategoriesTab` / `CategoriesTab.showMangaCategory()` (ui MangaScreen.kt:193-194) — library categories, area:08 / area:37.

---

## 11. Identifiers and keys used for UI state

- LazyColumn keys: `EntryScreenItem.INFO_BOX`, `ACTION_ROW`, `DESCRIPTION_WITH_TAG`, `ITEM_HEADER` (for header items) and `"missing-count-${MissingCount.id}"` / `"chapter-${Item.id}"` for rows (presentation MangaScreen.kt:774-778).
- Content types: `EntryScreenItem.ITEM` for rows.
- `rememberSaveable` keys: `expanded` (description) and dialog-local `showSetAsDefaultDialog`, `optionalChecked`, `selectedInterval`.

---

## 12. Serialization

No JSON or protobuf is involved in this area. Backup/restore of chapter flags and fetch intervals is in area:21/22; the flag layout in section 2 is what those backups store (as a Long). The Rust port must keep the same numeric layout so that imported Aniyomi backups produce identical behaviour.
