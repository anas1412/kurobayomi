# 39-ui-entry-details-anime (part 2): data models, bit layouts, preferences, storage, endpoints

Repo-relative paths; root `/home/blackbox/aniyomi-src`. Line numbers are `path:line`. Behavior is in `p2-behavior.md`; deep dives in the four topic files.

Sections:
1. Models (every field, type, nullability, default)
2. Bit-flag layouts (episode, season, viewer)
3. Enumerations with stored values
4. Preference keys used here (key, type, default, owner)
5. Files, folders, formats on disk
6. Network endpoints
7. Database tables and columns touched
8. Kurobayomi storage proposal

---

## 1. Models

### 1.1 `Anime` (domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:15-48, `@Immutable data class`, `Serializable`)

| Field | Type | Nullable | Default in `create()` (line 315-347) | Used by this scope for |
|---|---|---|---|---|
| `id` | Long | no | -1 | identity |
| `source` | Long | no | -1 | source lookup; `LocalAnimeSource.ID = 0` marks local |
| `favorite` | Boolean | no | false | library membership, non-library defaults (behavior 2.3.5) |
| `lastUpdate` | Long | no | 0 | `Anime.lastUpdate` for cover rules and fetch interval |
| `nextUpdate` | Long | no | 0 | `expectedNextUpdate` (Anime.kt:50-53) |
| `fetchInterval` | Int | no | 0 | 0 = smart; negative = custom days (-N) |
| `dateAdded` | Long | no | 0 | not used here |
| `viewerFlags` | Long | no | 0 | skip intro + airing (topic airing 1) |
| `episodeFlags` | Long | no | 0 | filter, sort, display, previews, summaries (section 2.1) |
| `coverLastModified` | Long | no | 0 | cover cache key |
| `backgroundLastModified` | Long | no | 0 | background cache key |
| `url` | String | no | "" | source URL; related-anime self-exclusion (SyncRelatedAnimeWithSource.kt:31) |
| `title` | String | no | "" | duplicate check (lowercase) |
| `artist` | String? | yes | null | presentation |
| `author` | String? | yes | null | presentation |
| `description` | String? | yes | null | presentation |
| `genre` | List<String>? | yes | null | presentation |
| `status` | Long | no | 0 | `SAnime.UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2` |
| `thumbnailUrl` | String? | yes | null | refresh rules (6.5 of behavior) |
| `backgroundUrl` | String? | yes | null | refresh rules |
| `updateStrategy` | AnimeUpdateStrategy | no | ALWAYS_UPDATE | not used here |
| `initialized` | Boolean | no | false | first-open detail fetch (behavior 2.3) |
| `lastModifiedAt` | Long | no | 0 | not used here |
| `favoriteModifiedAt` | Long? | yes | null | not used here |
| `version` | Long | no | 0 | not used here |
| `fetchType` | FetchType | no | Episodes | Seasons vs Episodes branch everywhere |
| `parentId` | Long? | yes | null | season rows: parent id |
| `seasonFlags` | Long | no | 0 | season filters/sort/display (topic seasons 2) |
| `seasonNumber` | Double | no | -1.0 | season sort (SEASON_SORT_SEASON) |
| `seasonSourceOrder` | Long | no | 0 | season sort (SEASON_SORT_SOURCE) |
| `memo` | JsonObject | no | `JsonObject.EMPTY` | not used here |

Derived members used here: `expectedNextUpdate` (Instant?, null when completed), `sorting`, `displayMode`, `unseenFilterRaw`, `downloadedFilterRaw`, `bookmarkedFilterRaw`, `fillermarkedFilterRaw`, `skipIntroLength`, `skipIntroDisable`, `nextEpisodeToAir`, `nextEpisodeAiringAt`, `unseenFilter`, `bookmarkedFilter`, `fillermarkedFilter`, `sortDescending()`, `showPreviewsRaw`, `showPreviews()`, `showSummariesRaw`, `showSummaries()`, the season getters, `seasonDisplayGridMode`, `seasonDisplayGridSize`, the five season overlay booleans, `seasonDisplayMode`. Extension properties (app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt): `Anime.downloadedFilter` (13-21), `Anime.seasonDownloadedFilter` (23-31), `Anime.episodesFiltered()` (33-38), `Anime.seasonsFiltered()` (line 40).

### 1.2 `Episode` (domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:6-25, `data class`)

| Field | Type | Nullable | Default in `create()` (lines 44-64) | Notes |
|---|---|---|---|---|
| `id` | Long | no | -1 | |
| `animeId` | Long | no | -1 | |
| `seen` | Boolean | no | false | |
| `bookmark` | Boolean | no | false | |
| `fillermark` | Boolean | no | false | |
| `lastSecondSeen` | Long | no | 0 | resume position in seconds (SetSeenStatus resets to 0 on unseen) |
| `totalSeconds` | Long | no | 0 | |
| `dateFetch` | Long | no | 0 | epoch millis |
| `sourceOrder` | Long | no | 0 | index in the source list (SyncEpisodesWithSource.kt:67) |
| `url` | String | no | "" | |
| `name` | String | no | "" | |
| `dateUpload` | Long | no | -1 | epoch millis; -1 = unknown |
| `episodeNumber` | Double | no | -1.0 | `isRecognizedNumber = episodeNumber >= 0f` |
| `scanlator` | String? | yes | null | |
| `summary` | String? | yes | null | |
| `previewUrl` | String? | yes | null | |
| `lastModifiedAt` | Long | no | 0 | |
| `version` | Long | no | 1 | |
| `memo` | JsonObject | no | `JsonObject.EMPTY` | |

`copyFrom(other)` (lines 30-41) copies `name`, `url`, `dateUpload`, `episodeNumber`, `fillermark`, and sets blank `scanlator`/`summary`/`previewUrl` to null.

### 1.3 `EpisodeUpdate` (domain/src/main/java/tachiyomi/domain/items/episode/model/EpisodeUpdate.kt:5-24)

Every field after `id` is nullable and means "unchanged when null" (the SQL uses `coalesce`). Fields: `id: Long`, `animeId: Long?`, `seen: Boolean?`, `bookmark: Boolean?`, `fillermark: Boolean?`, `lastSecondSeen: Long?`, `totalSeconds: Long?`, `dateFetch: Long?`, `sourceOrder: Long?`, `url: String?`, `name: String?`, `dateUpload: Long?`, `episodeNumber: Double?`, `scanlator: String?`, `summary: String?`, `previewUrl: String?`, `version: Long?`, `memo: JsonObject?`.

Constructed in this scope with: `(id, seen, lastSecondSeen)` in SetSeenStatus (mapper, lines 22-26), `(id, bookmark)` (AnimeScreenModel.kt:955), `(id, fillermark)` (969).

Note `lastSecondSeen = if (!read) 0 else null` (SetSeenStatus.kt:24): the value `0` is a Long literal here; `null` means unchanged.

### 1.4 `SeasonAnime` (domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt:6-15)

`anime: Anime`, `totalCount: Long`, `seenCount: Long`, `bookmarkCount: Long`, `fillermarkCount: Long`, `latestUpload: Long`, `fetchedAt: Long`, `lastSeen: Long`. Derived (no storage): `id`, `seen`, `unseenCount`, `hasStarted`, `hasBookmarks`, `hasFillermarks`, `toLibraryAnime()`.

### 1.5 `AnimeSeasonItem` (app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeSeasonItem.kt:5-12)

`seasonAnime: SeasonAnime` (required), `downloadCount: Long = -1L`, `unseenCount: Long = -1L`, `isLocal: Boolean = false`, `sourceLanguage: String = ""`, `showContinueOverlay: Boolean = false`.

### 1.6 `EpisodeList` (AnimeScreenModel.kt:1742-1760, `@Immutable sealed class`)

- `MissingCount(id: String, count: Int)`.
- `Item(episode: Episode, downloadState: AnimeDownload.State, downloadProgress: Int, selected: Boolean = false)` with body `val id = episode.id` (Long) and `val isDownloaded = downloadState == AnimeDownload.State.DOWNLOADED`.

### 1.7 `AnimeScreenModel.State` (AnimeScreenModel.kt:1602-1739)

See `p2-behavior.md` section 3.1 for the complete field table with defaults. Sealed: `Loading` (data object), `Success` (data class).

### 1.8 `AnimeScreenModel.Dialog` (AnimeScreenModel.kt:1547-1562, `sealed interface`)

Variants and fields are listed in `p2-behavior.md` section 5.1.

### 1.9 Tracker models

- `AnimeTrackItem(track: AnimeTrack?, tracker: Tracker)` (AnimeTrackItem.kt:6).
- Domain `AnimeTrack` (`domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20`): `id: Long`, `animeId: Long`, `trackerId: Long`, `remoteId: Long`, `libraryId: Long?`, `title: String`, `lastEpisodeSeen: Double`, `totalEpisodes: Long`, `status: Long`, `score: Double`, `remoteUrl: String`, `startDate: Long`, `finishDate: Long`, `private: Boolean`. (Full field semantics in `p2-topic-track-sheet.md` section 1.1.)
- DB `DbAnimeTrack` (`eu.kanade.tachiyomi.data.database.models.anime.AnimeTrack`): mutable Java-style fields `id`, `anime_id`, `tracker_id`, `remote_id`, `library_id`, `title`, `last_episode_seen`, `total_episodes`, `status`, `score`, `tracking_url`, `started_watching_date`, `finished_watching_date`, `private` (the mapping is in `app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt:17-31`).
- `AnimeTrackSearch` (app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt:7-35): `tracking_url` is `lateinit var` (line 35). Accessing it before set throws `UninitializedPropertyAccessException` (defect D-8 in the track topic).

### 1.10 Result and status types

- `SetSeenStatus.Result` (SetSeenStatus.kt:75-79): `Success`, `NoEpisodes`, `InternalError(error: Throwable)`.
- `UpdateAnimeFromRemote` result: `Result<RemoteAnimeEpisodeUpdate>` / `Result<RemoteAnimeSeasonUpdate>` (Kotlin `Result`).
- `RemoteAnimeEpisodeUpdate(anime: Anime, newEpisodes: List<Episode>)` and `RemoteAnimeSeasonUpdate(anime: Anime, newSeasons: List<Anime>)` (package `mihon.domain.source.models`, not opened; field names are used at UpdateAnimeFromRemote.kt:82 and :135).
- `RefreshAnimeTracks.await` returns `List<Pair<Tracker?, Throwable>>`.

### 1.11 Model classes of the tracking child screens

Each child screen is a `Screen` with a `Model : StateScreenModel<State>` or `ScreenModel`:

| Screen | State | Model fields |
|---|---|---|
| `AnimeTrackInfoDialogHomeScreen` | `State(trackItems: List<AnimeTrackItem>)` | `animeId`, `sourceId`, `getTracks` |
| `TrackStatusSelectorScreen` | `State(selection: Long)` | `track: DbAnimeTrack`, `tracker: Tracker` |
| `TrackEpisodeSelectorScreen` | `State(selection: Int)` | same |
| `TrackScoreSelectorScreen` | `State(selection: String)` | same |
| `TrackDateSelectorScreen` | none (`ScreenModel`) | `track`, `tracker`, `start: Boolean` |
| `TrackDateRemoverScreen` | none (`ScreenModel`) | same |
| `TrackServiceSearchScreen` | `State(queryResult: Result<List<AnimeTrackSearch>>? = null, selected: AnimeTrackSearch? = null)` | `animeId`, `currentUrl: String?`, `tracker` |
| `TrackerAnimeRemoveScreen` | none (`ScreenModel`) | `animeId`, `track: AnimeTrack`, `tracker`, `deleteTrack: DeleteAnimeTrack` |

Screen constructor parameters are `private val` data fields, serialized by Voyager (`@Transient` on `selectableDates` at line 465).

---

## 2. Bit-flag layouts

### 2.1 `episodeFlags` (Anime.kt:219-255)

| Bits | Mask (hex) | Name | Value constants |
|---|---|---|---|
| 0 | `0x00000001` | `EPISODE_SORT_DIR_MASK` | `EPISODE_SORT_DESC = 0x0`, `EPISODE_SORT_ASC = 0x1` |
| 1-2 | `0x00000006` | `EPISODE_UNSEEN_MASK` | `EPISODE_SHOW_UNSEEN = 0x2`, `EPISODE_SHOW_SEEN = 0x4` |
| 3-4 | `0x00000018` | `EPISODE_DOWNLOADED_MASK` | `EPISODE_SHOW_DOWNLOADED = 0x8`, `EPISODE_SHOW_NOT_DOWNLOADED = 0x10` |
| 5-6 | `0x00000060` | `EPISODE_BOOKMARKED_MASK` | `EPISODE_SHOW_BOOKMARKED = 0x20`, `EPISODE_SHOW_NOT_BOOKMARKED = 0x40` |
| 7-8 | `0x00000180` | `EPISODE_FILLERMARKED_MASK` | `EPISODE_SHOW_FILLERMARKED = 0x80`, `EPISODE_SHOW_NOT_FILLERMARKED = 0x100` |
| 9-10 | `0x00000600` | `EPISODE_SORTING_MASK` | `EPISODE_SORTING_SOURCE = 0x0`, `EPISODE_SORTING_NUMBER = 0x200`, `EPISODE_SORTING_UPLOAD_DATE = 0x400`, `EPISODE_SORTING_ALPHABET = 0x600` |
| 11 | `0x00000800` | `EPISODE_PREVIEWS_MASK` | `EPISODE_SHOW_PREVIEWS = 0x0`, `EPISODE_SHOW_NOT_PREVIEWS = 0x800` |
| 12 | `0x00001000` | `EPISODE_SUMMARIES_MASK` | `EPISODE_SHOW_SUMMARIES = 0x0`, `EPISODE_SHOW_NOT_SUMMARIES = 0x1000` |
| 20 | `0x00100000` | `EPISODE_DISPLAY_MASK` | `EPISODE_DISPLAY_NAME = 0x0`, `EPISODE_DISPLAY_NUMBER = 0x100000` |

Filter constant `SHOW_ALL = 0x0` clears the bits of its group. The episode flags word has no other bits in use.

Tri-state readers (Anime.kt:85-104): `unseenFilter`: `0x2 -> ENABLED_IS`, `0x4 -> ENABLED_NOT`, else DISABLED. Same pattern for the others.

Default flags for a new episode-list entry: 0 (SOURCE sorting, DESC, all filters off, name display, previews and summaries shown) when no library defaults have been applied.

### 2.2 `seasonFlags` (Anime.kt:257-308)

See `p2-topic-seasons.md` section 2 (complete table, bits 0-27).

### 2.3 `viewerFlags` (Anime.kt:310-313)

| Bits | Mask | Meaning |
|---|---|---|
| 0-7 | `0x00000000000000FF` | skip-intro length, seconds (0..255) |
| 8-23 | `0x00000000000FFFF00` | next airing episode number (0..65535) |
| 24-55 | `0x0FFFFFFFF000000` | next airing time, epoch seconds (32 bits) |
| 56 | `0x0100000000000000` | skip-intro disabled |

`viewerFlags` also carries flags used by other areas (not present in these constants); they must be preserved on write (every write here uses `and mask.inv()`).

### 2.4 Season/episode flag write semantics (common helper)

`private fun Long.setFlag(flag: Long, mask: Long): Long = this and mask.inv() or (flag and mask)` (SetAnimeEpisodeFlags.kt:127-129; SetAnimeSeasonFlags.kt:186-188; SetAnimeViewerFlags.kt:51-53). Bits outside `mask` are preserved; bits in `mask` take the value of `flag and mask`.

---

## 3. Enumerations with stored values

| Enum | Values and stored numbers | Source |
|---|---|---|
| `TriState` | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`; in-memory only, never stored (flags store the raw masks) | core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:3-6 |
| `FetchType` | `Seasons` (ordinal 0), `Episodes` (ordinal 1); stored as `fetch_type` INTEGER, default 1 | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:10-19; animes.sq:33; SQL `CASE M.fetch_type WHEN 1 THEN ES ... WHEN 0 THEN ASS` in animeseasonsView.sq:4-23 |
| `AnimeDownload.State` | `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)` (`value: Int`) | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt:62-68 |
| `LibraryPreferences.EpisodeSwipeAction` | `ToggleSeen`, `ToggleBookmark`, `ToggleFillermark`, `Download`, `Disabled` (stored by name via `getEnum`) | domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:383-389 |
| `AutoTrackState` | `ALWAYS`, `ASK`, `NEVER` (stored by name) | app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt:6-10 |
| `SeasonDisplayMode` | `CompactGrid = 0`, `ComfortableGrid = 1`, `CoverOnlyGrid = 2`, `List = 3` | domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt:3-28 |
| `DownloadAction` (presentation enum) | `NEXT_1_ITEM`, `NEXT_5_ITEMS`, `NEXT_10_ITEMS`, `NEXT_25_ITEMS`, `UNVIEWED_ITEMS` | app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt:3-9 |
| `EpisodeDownloadAction` | `START`, `START_NOW`, `CANCEL`, `DELETE`, `SHOW_QUALITIES` | app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt:42-48 |
| `EditCoverAction` | `EDIT`, `DELETE` | EntryScreenConstants.kt:11-14 |
| `EntryScreenItem` (presentation) | `INFO_BOX`, `ACTION_ROW`, `DESCRIPTION_WITH_TAG`, `ITEM_HEADER`, `ITEM`, `AIRING_TIME` | EntryScreenConstants.kt:16-23 |
| `SAnime` status | `UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2` (Int constants; stored as Long `status`) | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-74 |
| `FetchType` and `SAnime` completed codes for trackers | per tracker, e.g. AniList `COMPLETED = 2L` (Anilist.kt:41) | areas 28-33 |
| `TrackerManager` ids | see topic track 1.3 | TrackerManager.kt:20-37 |
| `LibraryPreferences.ENTRY_*` (library update restrictions, shared string values) | `ENTRY_NON_COMPLETED = "manga_ongoing"`, `ENTRY_HAS_UNVIEWED = "manga_fully_read"`, `ENTRY_NON_VIEWED = "manga_started"`, `ENTRY_OUTSIDE_RELEASE_PERIOD = "manga_outside_release_period"` | domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:403-406 |
| `MARK_DUPLICATE_EPISODE_SEEN_*` | `"new_episode"`, `"existing_episode"` (library preference values, not used by this scope) | LibraryPreferences.kt:410-411 |

---

## 4. Preference keys used here

Format: key (as a string literal or constant), storage type, default, reader function, owner file. "Owner" is the Kotlin file that declares the accessor. `appState` keys are prefixed with `__APP_STATE_` (core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-45, `APP_STATE_PREFIX = "__APP_STATE_"`).

### 4.1 Library episode and season defaults (LibraryPreferences.kt)

| Key | Type | Default | Reader |
|---|---|---|---|
| `default_episode_filter_by_seen` | Long | `Anime.SHOW_ALL` (0) | `filterEpisodeBySeen()` (173-175) |
| `default_episode_filter_by_downloaded` | Long | 0 | `filterEpisodeByDownloaded()` (179-181) |
| `default_episode_filter_by_bookmarked` | Long | 0 | `filterEpisodeByBookmarked()` (185-187) |
| `default_episode_filter_by_fillermarked` | Long | 0 | `filterEpisodeByFillermarked()` (191-193) |
| `default_episode_sort_by_source_or_number` | Long | `EPISODE_SORTING_SOURCE` (0) | `sortEpisodeBySourceOrNumber()` (195-198) |
| `default_chapter_sort_by_ascending_or_descending` | Long | `EPISODE_SORT_DESC` (0) | `sortEpisodeByAscendingOrDescending()` (215-218). **Shared with manga `sortChapterByAscendingOrDescending` (defect in behavior 23.10).** |
| `default_chapter_display_by_name_or_number` | Long | `EPISODE_DISPLAY_NAME` (0) | `displayEpisodeByNameOrNumber()` (205-208). **Shared with manga `displayChapterByNameOrNumber`.** |
| `default_episode_show_thumbnail_previews` | Long | `EPISODE_SHOW_PREVIEWS` (0) | `showEpisodeThumbnailPreviews()` (225-228) |
| `default_episode_show_summaries` | Long | `EPISODE_SHOW_SUMMARIES` (0) | `showEpisodeSummaries()` (230-233) |
| `default_season_filter_by_downloaded` | Long | 0 | `filterSeasonByDownload()` (262-263) |
| `default_season_filter_by_unseen` | Long | 0 | `filterSeasonByUnseen()` (265-266) |
| `default_season_filter_by_started` | Long | 0 | `filterSeasonByStarted()` (268-269) |
| `default_season_filter_by_completed` | Long | 0 | `filterSeasonByCompleted()` (271-272) |
| `default_season_filter_by_bookmarked` | Long | 0 | `filterSeasonByBookmarked()` (274-275) |
| `default_season_filter_by_fillermarked` | Long | 0 | `filterSeasonByFillermarked()` (277-278) |
| `default_season_sort_by_source_or_number` | Long | `SEASON_SORT_SOURCE` (0) | `sortSeasonBySourceOrNumber()` (280-283) |
| `default_season_sort_by_ascending_or_descending` | Long | `SEASON_SORT_DESC` (0) | `sortSeasonByAscendingOrDescending()` (285-288) |
| `default_season_grid_display_mode` | Long | `SeasonDisplayMode.toLong(CompactGrid)` (0) | `seasonDisplayGridMode()` (290-293) |
| `default_season_grid_display_size` | Int | 0 | `seasonDisplayGridSize()` (295-298) |
| `default_season_download_overlay` | Boolean | false | `seasonDownloadOverlay()` (300-303) |
| `default_season_unseen_overlay` | Boolean | true | `seasonUnseenOverlay()` (305-308) |
| `default_season_local_overlay` | Boolean | true | `seasonLocalOverlay()` (310-313) |
| `default_season_lang_overlay` | Boolean | false | `seasonLangOverlay()` (315-318) |
| `default_season_continue_overlay` | Boolean | true | `seasonContinueOverlay()` (320-323) |
| `default_season_display_mode` | Long | `SEASON_DISPLAY_MODE_SOURCE` (0) | `seasonDisplayMode()` (325-328) |
| `pref_update_season_on_refresh` | Boolean | false | `updateSeasonOnRefresh()` (353-354) |
| `pref_update_season_on_library_update` | Boolean | false | `updateSeasonOnLibraryUpdate()` (357-358) |
| `pref_episode_swipe_end_action` | enum by name | `ToggleSeen` | `swipeEpisodeStartAction()` (361-362). Assigned to the model field `episodeSwipeEndAction` (AnimeScreenModel.kt:172). |
| `pref_episode_swipe_start_action` | enum by name | `ToggleBookmark` | `swipeEpisodeEndAction()` (364-367). Assigned to the model field `episodeSwipeStartAction` (AnimeScreenModel.kt:171). |
| `library_update_manga_restriction` | Set<String> | `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` | `autoUpdateItemRestrictions()` (49-57). Read by `isUpdateIntervalEnabled`. |
| `default_anime_category` | Int | -1 | `defaultAnimeCategory()` (153). Key constant `DEFAULT_ANIME_CATEGORY_PREF_KEY` (LibraryPreferences.kt:414). -1 = no default category. |
| `animelib_update_categories` | Set<String> | empty | `animeUpdateCategories()` (159-160). Not used here. |
| `animelib_update_categories_exclude` | Set<String> | empty | `animeUpdateCategoriesExclude()` (165-166). Not used here. |

Model-side mapping of the two swipe keys (AnimeScreenModel.kt:171-172):
- `episodeSwipeStartAction` (model) <- `swipeEpisodeEndAction()` <- key `pref_episode_swipe_start_action`, default `ToggleBookmark`.
- `episodeSwipeEndAction` (model) <- `swipeEpisodeStartAction()` <- key `pref_episode_swipe_end_action`, default `ToggleSeen`.

### 4.2 Track preferences (app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt)

| Key | Type | Default | Reader |
|---|---|---|---|
| `track_on_adding_to_library` | Boolean | true | `trackOnAddingToLibrary()` (41) |
| `show_next_episode_airing_time` | Boolean | true | `showNextEpisodeAiringTime()` (43-46) |
| `pref_auto_update_manga_on_mark_read` | enum by name, `AutoTrackState` | `ALWAYS` | `autoUpdateTrackOnMarkRead()` (48-51). The key name says "manga" but it is read for anime. |

### 4.3 Download preferences (domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt)

| Key | Type | Default | Reader | Used here for |
|---|---|---|---|---|
| `pref_remove_after_marked_as_read_key` | Boolean | false | `removeAfterMarkedAsRead()` (30-33) | delete downloads on mark seen (SetSeenStatus.kt:49) |
| `pref_remove_bookmarked` | Boolean | false | `removeBookmarkedChapters()` (line 35) | delete filter, not in this scope |
| `pref_download_fillermarked` | Boolean | false | `downloadFillermarkedItems()` (line 37) | download filter, not in this scope |
| `use_external_downloader` | Boolean | false | `useExternalDownloader()` (14) | quality dialog |
| `download_new_episode` | Boolean | false | `downloadNewEpisodes()` (49) | auto-download after manual refresh |
| `download_new_unread_episodes_only` | Boolean | false | `downloadNewUnseenEpisodesOnly()` (73) | auto-download filter |
| `download_new_anime_categories` | Set<String> | empty | `downloadNewEpisodeCategories()` (line 55) | category include list |
| `download_new_anime_categories_exclude` | Set<String> | empty | `downloadNewEpisodeCategoriesExclude()` (line 64) | category exclude list |
| `remove_exclude_anime_categories` | Set<String> | empty | `removeExcludeAnimeCategories()` (line 43) | delete-on-read exclusions (AnimeDownloadManager.kt:408-427) |

### 4.4 Player, gesture, source, torrent, base preferences

| Key | Type | Default | Reader |
|---|---|---|---|
| `pref_always_use_external_player` | Boolean | false | `PlayerPreferences.alwaysUseExternalPlayer()` (app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt:67-70) |
| `pref_default_intro_length` | Int | 85 | `GesturePreferences.defaultIntroLength()` (app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt:21) |
| `pref_display_mode_catalogue` | object (serialized `LibraryDisplayMode`) | `LibraryDisplayMode.default` | `SourcePreferences.sourceDisplayMode()` (app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:16-20) |
| `pref_torrserver_enable` | Boolean | false | `TorrentPreferences.torrServerEnable()` (core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt:9) |
| `__APP_STATE_pref_downloaded_only` | Boolean | false | `BasePreferences.downloadedOnly()` (app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-19), via `Preference.appStateKey("pref_downloaded_only")` |
| `app_date_format` | String | "" | `UiPreferences.dateFormat()` (app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:35), used for the track date display |

### 4.5 Kurobayomi key proposal (for the Rust settings store)

Keep the Android semantics, fix the collisions:

| Kurobayomi key | Replaces | Default |
|---|---|---|
| `entry.episode.filter.seen` | `default_episode_filter_by_seen` | 0 |
| `entry.episode.filter.downloaded` | `default_episode_filter_by_downloaded` | 0 |
| `entry.episode.filter.bookmarked` | `default_episode_filter_by_bookmarked` | 0 |
| `entry.episode.filter.fillermarked` | `default_episode_filter_by_fillermarked` | 0 |
| `entry.episode.sort.key` | `default_episode_sort_by_source_or_number` | 0 (source) |
| `entry.episode.sort.desc` | `default_chapter_sort_by_ascending_or_descending` (collision fixed) | true |
| `entry.episode.display.number` | `default_chapter_display_by_name_or_number` (collision fixed) | false |
| `entry.episode.previews` | `default_episode_show_thumbnail_previews` | true |
| `entry.episode.summaries` | `default_episode_show_summaries` | true |
| `entry.season.*` | the `default_season_*` set above, with `entry.season.filter.downloaded` reading its own value (defect fix) | as above |
| `entry.swipe.start` / `entry.swipe.end` | the two swipe keys, named by direction | `Bookmark` / `Seen` (same effective defaults) |
| `entry.refresh.seasons.episodes` | `pref_update_season_on_refresh` | false |
| `track.auto_on_mark_seen` | `pref_auto_update_manga_on_mark_read` | `always` |
| `track.on_add_open_sheet` | `track_on_adding_to_library` | true |
| `track.show_airing` | `show_next_episode_airing_time` | true |
| `player.always_external` | `pref_always_use_external_player` | false |
| `player.intro_default_seconds` | `pref_default_intro_length` | 85 |
| `downloads.remove_after_read` | `pref_remove_after_marked_as_read_key` | false |
| `downloads.new_episodes` | `download_new_episode` | false |
| `downloads.new_unseen_only` | `download_new_unread_episodes_only` | false |
| `library.default_category` | `default_anime_category` | -1 |

---

## 5. Files, folders, formats on disk

This scope writes no files directly.

Indirect file effects:
- Covers and backgrounds: `AnimeCoverCache` and `AnimeBackgroundCache` delete cached files on refresh and on library removal (UpdateAnimeFromRemote.kt:164-189; AnimeScreenModel.kt:403). Storage layout is area 34-images-covers-cache.
- Downloads: `downloadManager.deleteEpisodes`, `deleteAnime` remove episode and anime directories (AnimeDownloadManager.kt:269-333). Directory naming and formats are area 24/25.
- Local anime: `getDownloadCount` for local anime counts archive files in the anime directory (AnimeDownloadManager.kt:228-238), supported by `ArchiveAnime.isSupported`.
- Torrent server: data directory owned by the torrent service (area 13).

Export format: none in scope.

---

## 6. Network endpoints

### 6.1 AniList GraphQL (`AniChartApi`, owner area:52-util-app)

- URL: `https://graphql.anilist.co`. Method: `POST`. Content type: `jsonMime` (JSON). Auth: none. Body: JSON object `{"query": "<graphql>"}` built with `buildJsonObject { put("query", query) }.toString()`.
- Query A (MAL to AniList id): `query { Media(idMal:<long>,type: ANIME) { id } }`. Response: `{"data":{"Media":{"id":<long>}}}` on success; `{"data":{"Media":null}}` when not found.
- Query B (airing): `query { Media(id:<long>) { nextAiringEpisode { episode airingAt } } }`. Response: `{"data":{"Media":{"nextAiringEpisode":{"episode":<int>,"airingAt":<epoch seconds>}}}}`; `nextAiringEpisode` is `null` when nothing is scheduled (observed read-only for id 1).
- Timeouts: OkHttp default (no explicit timeout in AniChartApi.kt:20).

### 6.2 Simkl calendar (`AniChartApi`, owner area:52-util-app)

- URLs in order: `https://data.simkl.in/calendar/anime.json`, `https://data.simkl.in/calendar/tv.json`, `https://data.simkl.in/calendar/movie_release.json`. Method `GET`, no auth.
- Response: JSON array. Each element (observed, abbreviated): `{"title": str, "poster": str, "date": ISO-8601 with offset, "release_date": "YYYY-MM-DD", "rank": int, "ratings": {"simkl": {"rating": float, "votes": int}}, "url": str, "ids": {"simkl_id": int, "slug": str, "tmdb": int|null, "mal": str|null}, "episode": {"episode": int, "url": str}, "anime_type": str}`.

### 6.3 Source endpoints used by this scope

None directly. All source calls go through `AnimeSource` (`getAnimeEpisodeUpdate`, `getAnimeSeasonUpdate`, `getRelatedAnimeList`, `getAnimeUrl`, `supportsRelatedAnime`), implemented in the source-api and extension areas.

### 6.4 Tracker endpoints used by this scope

Only through `AnimeTracker` methods: `bind`, `refresh`, `update`, `searchAnime`, `register` (via `AddAnimeTracks`), `setRemote*` (via `updateRemote`), `delete` (DeletableAnimeTracker), `match` (EnhancedAnimeTracker). The endpoint URLs are in the tracker areas 28-33.

### 6.5 Torrent server

`TorrentServerService.api.echo()` (local HTTP echo, `TorrentServerApi`, owner area 13). Polled once per second up to 11 attempts (behavior section 22).

---

## 7. Database tables and columns touched

Table and column names from the sqldelight files. Each row lists the operation performed in this scope.

### 7.1 `animes` (data/src/main/sqldelightanime/dataanime/animes.sq)

| Column | Type (from CREATE/INSERT) | Operation here |
|---|---|---|
| `_id` | INTEGER PK | identity |
| `source` | INTEGER | read (source lookup) |
| `url` | TEXT | read (related self-exclusion, source call) |
| `title` | TEXT | read, duplicate check via `LOWER(title) = :title` (animes.sq:132) |
| `favorite` | INTEGER (0/1) | read, write (`awaitUpdateFavorite`); duplicate query filters `favorite = 1` |
| `last_update` | INTEGER (nullable, animes.sq:20) | read (cover rules, fetch interval), write (`awaitUpdateLastUpdate`) |
| `next_update` | INTEGER (nullable, animes.sq:21) | write (fetch interval, `coalesce(:nextUpdate, next_update)` at animes.sq:183) |
| `calculate_interval` | INTEGER NOT NULL DEFAULT 0 (animes.sq:30) | the DB column for the domain `fetchInterval` (AnimeMapper.kt:52 `fetchInterval = calculateInterval.toInt()`; AnimeRepositoryImpl.kt:120 and 205). Written by `setFetchInterval` through `AnimeUpdate.fetchInterval`. The name `fetch_interval` does not exist in this table. |
| `initialized` | INTEGER | read/write (first-open fetch) |
| `viewer` | INTEGER NOT NULL (animes.sq:23) | read/write (viewer flags) |
| `episode_flags` | INTEGER NOT NULL (animes.sq:24) | read/write (episode settings) |
| `update_strategy` | INTEGER (animes.sq:27) | written by refresh |
| `fetch_type` | INTEGER NOT NULL DEFAULT 1 (animes.sq:33) | read (branch) |
| `parent_id` | INTEGER (animes.sq:34) | read (season children), write (`removeParentIdByIds`, animes.sq:154-160) |
| `season_flags` | INTEGER NOT NULL (animes.sq:35) | read/write (season settings) |
| `season_number` | REAL NOT NULL (animes.sq:36) | read (SEASON_SORT_SEASON) |
| `season_source_order` | INTEGER NOT NULL (animes.sq:37) | read (SEASON_SORT_SOURCE), write (sync) |
| `thumbnail_url`, `background_url` | TEXT | write (refresh) |
| `cover_last_modified`, `background_last_modified` | INTEGER | write (refresh) |
| `status` | INTEGER | read (completed checks), write (refresh) |
| `author`, `artist`, `description`, `genre` | TEXT | write (refresh) |
| `memo` | BLOB JSON | write (refresh) |

### 7.2 `episodes` (data/src/main/sqldelightanime/dataanime/episodes.sq:4-26)

| Column | Type | Operation here |
|---|---|---|
| `_id` | INTEGER PK | identity |
| `anime_id` | INTEGER NOT NULL, FK -> animes(_id) ON DELETE CASCADE | read (`getEpisodesByAnimeId`: `SELECT * FROM episodes WHERE anime_id = :animeId;`, episodes.sq:62-65) |
| `url`, `name`, `scanlator` | TEXT, TEXT NOT NULL, TEXT | read (display, download-cache lookup by name and scanlator) |
| `seen` | INTEGER AS Boolean NOT NULL | read, write (SetSeenStatus) |
| `bookmark` | INTEGER AS Boolean NOT NULL | read, write |
| `fillermark` | INTEGER AS Boolean NOT NULL | read, write |
| `last_second_seen` | INTEGER NOT NULL | read, write (reset to 0 on unseen) |
| `total_seconds` | INTEGER NOT NULL | read |
| `episode_number` | REAL NOT NULL | read (sort, gap), write (refresh) |
| `source_order` | INTEGER NOT NULL | read (sort), write (refresh) |
| `date_fetch` | INTEGER NOT NULL | read (fetched-at aggregate) |
| `date_upload` | INTEGER NOT NULL | read (upload sort) |
| `last_modified_at`, `version`, `is_syncing` | INTEGER | not read here |
| `summary`, `preview_url` | TEXT | read (rich rows) |
| `memo` | BLOB JSON | not read here |

Update SQL: `update:` starts at episodes.sq:98 and uses `coalesce(:field, field)` for each column.

### 7.3 Views read here

| View | File | Used for |
|---|---|---|
| `episodestatsView` | data/src/main/sqldelightanime/view/episodestatsView.sq:1-12 | season aggregates (`ES`) |
| `animehistorystatsView` | animehistorystatsView.sq:1-7 | season `lastSeen` (`AHS`) |
| `animeseasonstatsView` | data/src/main/sqldelightanime/view/animeseasonstatsView.sq | season aggregates (`ASS`) for nested seasons |
| `animeseasonsView` | data/src/main/sqldelightanime/view/animeseasonsView.sq:1-44 | `getAnimeSeasonsById` (`WHERE parent_id = :parentId`) |
| `animehistoryView` | animehistoryView.sq | not read here (history screen, other area) |

### 7.4 Other tables touched via repositories

| Table | Operation here | Owner |
|---|---|---|
| `animes_categories` | read (`getCategories.await(animeId)`), write (`setAnimeCategories`) | area 08 / 05 |
| `anime_sync` (tracks) | read (`getTracks.subscribe`), write (`insertTrack`), delete (`deleteTrack`) | area 06 |
| `animehistory` | read (first history entry for start date, `GetAnimeHistory.await(animeId)`) | area 06 |
| `anime_relations` | read (`getRelatedAnime.subscribe`, `getLastFetchedAt`), write (`replaceRelations`) | area 05 |
| `categories` | read (`getCategories`) | area 08 |

### 7.5 Constraints that matter for this scope

- `episodes.anime_id` has ON DELETE CASCADE. Removing an anime row removes its episodes.
- `animes.parent_id` is set to NULL for children when a parent's season is removed (`removeParentIdByIds`, animes.sq:154-160).
- No unique constraint on `(anime_id, url)` is applied in the SQL shown; duplicates are prevented in the sync layer.

---

## 8. Kurobayomi storage proposal

- Use one SQLite database (rusqlite, bundled) with tables mirroring the above, using the same integer encodings for flags, so that a future import from an Aniyomi backup can copy values without transformation. Column names keep the Android spelling where the value is a copied flag.
- Keep `viewer`, `episode_flags`, `season_flags` as INTEGER (i64). Use `u64` in Rust with explicit masks; never use signed shifts on these values.
- Store `season_number` as REAL and `episode_number` as REAL (f64). The `-1.0` sentinel for unknown is kept.
- `fetch_type`: store 0 for Seasons and 1 for Episodes (matching the enum ordinals and the SQL CASE).
- Add `entries_fetch_log(anime_id, kind, fetched_at)` for the empty-list refresh guard (behavior 2.4 proposal) and for the 10-minute refresh floor.
- Add `anime_relations` GC marker column `stub_related INTEGER NOT NULL DEFAULT 0` (related-anime proposal R-2).
- Preferences: a `settings` table (`key TEXT PRIMARY KEY, value TEXT NOT NULL`) with the Kurobayomi keys of section 4.5 and typed accessors.
