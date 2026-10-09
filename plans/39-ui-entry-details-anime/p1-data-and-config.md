# Area 39 part 1: data models, flags, preference keys, files, endpoints, tables

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Types are Kotlin unless marked "SQL" or "Rust".

## 1. Domain models used by the screens

### 1.1 `Anime` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:16-50`)

| Field | Kotlin type | Notes |
|---|---|---|
| `id` | `Long` | `animes._id` |
| `source` | `Long` | source id; `LocalAnimeSource.ID` marks local anime |
| `favorite` | `Boolean` | library membership |
| `lastUpdate` | `Long` | ms |
| `nextUpdate` | `Long` | ms |
| `fetchInterval` | `Int` | `< 0` means user-set interval mode (`AnimeScreen.kt:509`) |
| `dateAdded` | `Long` | ms |
| `viewerFlags` | `Long` | see section 2.3 (`viewer` column) |
| `episodeFlags` | `Long` | see section 2.1 (`episode_flags` column) |
| `coverLastModified` | `Long` | ms, cache-busting key |
| `backgroundLastModified` | `Long` | ms (`background_last_modified` column) |
| `url` | `String` | source-side URL |
| `title` | `String` | |
| `artist` | `String?` | |
| `author` | `String?` | |
| `description` | `String?` | |
| `genre` | `List<String>?` | SQL `genre TEXT AS List<String>` |
| `status` | `Long` | see section 3.1 |
| `thumbnailUrl` | `String?` | cover URL |
| `backgroundUrl` | `String?` | background URL |
| `updateStrategy` | `AnimeUpdateStrategy` | `ALWAYS_UPDATE` default in `create()` |
| `initialized` | `Boolean` | |
| `lastModifiedAt` | `Long` | |
| `favoriteModifiedAt` | `Long?` | |
| `version` | `Long` | |
| `fetchType` | `FetchType` | SQL `fetch_type INTEGER AS FetchType NOT NULL DEFAULT 1` |
| `parentId` | `Long?` | SQL `parent_id`; a season's parent entry |
| `seasonFlags` | `Long` | see section 2.2 (`season_flags` column) |
| `seasonNumber` | `Double` | SQL `season_number REAL` |
| `seasonSourceOrder` | `Long` | SQL `season_source_order` |
| `memo` | `JsonObject` | SQL `memo BLOB AS JsonObject NOT NULL DEFAULT '{}'` |

Derived properties used here (all in `Anime.kt` unless noted):
- `sorting` = `episodeFlags & EPISODE_SORTING_MASK`; `displayMode` = `episodeFlags & EPISODE_DISPLAY_MASK`.
- `downloadedFilter`, `unseenFilter`, `bookmarkedFilter`, `fillermarkedFilter`: `TriState` from the raw bits. `downloadedFilter` and `seasonDownloadedFilter` live in `app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:9-30` and return `ENABLED_IS` when `BasePreferences.downloadedOnly()` is true.
- `episodesFiltered()` = any of unseen, downloaded, bookmarked, fillermarked is not `DISABLED` (`app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:33-38`).
- `seasonsFiltered()` = any of the six season tri-states is not `DISABLED` (lines 40-48).
- `seasonDisplayGridMode: SeasonDisplayMode`, `seasonDisplayGridSize: Int`, the five `season*Overlay` booleans, `seasonDisplayMode: Long`.

### 1.2 `Episode` (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt`)

| Field | Type | SQL column |
|---|---|---|
| `id` | `Long` | `episodes._id` |
| `animeId` | `Long` | `anime_id` |
| `seen` | `Boolean` | `seen` |
| `bookmark` | `Boolean` | `bookmark` |
| `fillermark` | `Boolean` | `fillermark` |
| `lastSecondSeen` | `Long` | `last_second_seen` (**milliseconds**, see behaviour section 3.4) |
| `totalSeconds` | `Long` | `total_seconds` (**milliseconds**) |
| `dateFetch` | `Long` | `date_fetch` |
| `sourceOrder` | `Long` | `source_order` |
| `url` | `String` | `url` |
| `name` | `String` | `name` |
| `dateUpload` | `Long` | `date_upload` |
| `episodeNumber` | `Double` | `episode_number REAL` |
| `scanlator` | `String?` | `scanlator` |
| `summary` | `String?` | `summary` |
| `previewUrl` | `String?` | `preview_url` |
| `lastModifiedAt` | `Long` | `last_modified_at` |
| `version` | `Long` | `version` |
| `memo` | `JsonObject` | `memo BLOB AS JsonObject NOT NULL DEFAULT '{}'` |
| `isRecognizedNumber` | `Boolean` | derived from episode number parsing (not a column in the schema excerpt) |

Schema excerpt: `data/src/main/sqldelightanime/dataanime/episodes.sq:4-27`. `FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE`.

### 1.3 `SeasonAnime` (`domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt`)

`data class SeasonAnime(anime: Anime, totalCount: Long, seenCount: Long, bookmarkCount: Long, fillermarkCount: Long, latestUpload: Long, fetchedAt: Long, lastSeen: Long)`. Derived: `id = anime.id`; `seen = totalCount == seenCount`; `unseenCount = totalCount - seenCount`; `hasStarted = seenCount > 0`; `hasBookmarks = bookmarkCount > 0`; `hasFillermarks = fillermarkCount > 0`.

### 1.4 `AnimeSeasonItem` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeSeasonItem.kt`)

`data class AnimeSeasonItem(seasonAnime: SeasonAnime, downloadCount: Long = -1L, unseenCount: Long = -1L, isLocal: Boolean = false, sourceLanguage: String = "", showContinueOverlay: Boolean = false)`.

### 1.5 `EpisodeList` (declared in `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1743-1759`, part 2 owns the builder)

```kotlin
sealed class EpisodeList {
    @Immutable data class MissingCount(val id: String, val count: Int) : EpisodeList()
    @Immutable data class Item(
        val episode: Episode,
        val downloadState: AnimeDownload.State,
        val downloadProgress: Int,
        val selected: Boolean = false,
    ) : EpisodeList() {
        val id = episode.id          // Long
        val isDownloaded = downloadState == AnimeDownload.State.DOWNLOADED
    }
}
```

### 1.6 `AnimeDownload.State` (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt:62-68`)

| Name | `value: Int` |
|---|---|
| `NOT_DOWNLOADED` | 0 |
| `QUEUE` | 1 |
| `DOWNLOADING` | 2 |
| `DOWNLOADED` | 3 |
| `ERROR` | 4 |

### 1.7 `Video` and `Video.State` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt`)

`data class Video(videoUrl: String = "", videoTitle: String = "", resolution: Int? = null, bitrate: Int? = null, headers: Headers? = null, preferred: Boolean = false, subtitleTracks: List<Track> = emptyList(), audioTracks: List<Track> = emptyList(), timestamps: List<TimeStamp> = emptyList(), mpvArgs: List<Pair<String,String>> = emptyList(), ffmpegStreamArgs: ..., ffmpegVideoArgs: ...)`. Other members (`usesHttpServer()`, `Video.MPV_ARGS_TAG = "ANIYOMI_MPV_ARGS"`, `localUrl` regex) belong to the player area.

`enum class Video.State { QUEUE, LOAD_VIDEO, READY, ERROR }` (line 180). Order as declared. No explicit ordinals.

### 1.8 `Hoster` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt:10-17`)

`open class Hoster(hosterUrl: String = "", hosterName: String = "", videoList: List<Video>? = null, internalData: String = "", lazy: Boolean = false, memo: JsonObject = JsonObject.EMPTY)`. `Hoster.NO_HOSTER_LIST = "no_hoster_list"` (line 57). A hoster with that name represents a source that has videos but no hoster grouping (`EpisodeOptionsDialog.kt:573-576`).

### 1.9 `HosterState` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt:51-60`)

```kotlin
sealed class HosterState(open val name: String) {
    data class Idle(override val name: String) : HosterState(name)
    data class Loading(override val name: String) : HosterState(name)
    data class Error(override val name: String) : HosterState(name)
    data class Ready(override val name: String, val videoList: List<Video>, val videoState: List<Video.State>) : HosterState(name)
}
```
`fun HosterState.Ready.getChangedAt(index, newVideo, newState)` replaces one video and its state (lines 62-72).

### 1.10 `SAnime` status constants (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79`)

`UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2`, `LICENSED = 3`, `PUBLISHING_FINISHED = 4`, `CANCELLED = 5`, `ON_HIATUS = 6`, `UPCOMING = 7`. Stored as `Long` in `animes.status INTEGER NOT NULL`.

### 1.11 `FetchType` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:10-20`)

`enum class FetchType { Seasons, Episodes }`. Ordinals: `Seasons = 0`, `Episodes = 1`. The SQL default `DEFAULT 1` therefore means `Episodes`.

### 1.12 `SeasonDisplayMode` (`domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt`)

`sealed interface SeasonDisplayMode` with data objects `CompactGrid`, `ComfortableGrid`, `CoverOnlyGrid`, `List`. `toLong`: CompactGrid 0, ComfortableGrid 1, CoverOnlyGrid 2, List 3. `fromLong(v)`: 0..3 map back; any other value throws `IllegalArgumentException("Invalid sorting value")`.

### 1.13 `LibraryDisplayMode` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt:3-8`)

`sealed interface` with data objects `CompactGrid`, `ComfortableGrid`, `List`, `CoverOnlyGrid`. Serialised with its `Serializer` object (lines 10-...). Used by related rows.

### 1.14 `TriState` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:3-7`)

`enum class TriState { DISABLED, ENABLED_IS, ENABLED_NOT }`. Shown as the three-state checkbox in filter pages.

### 1.15 `LibraryPreferences.EpisodeSwipeAction` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:383-389`)

`enum class EpisodeSwipeAction { ToggleSeen, ToggleBookmark, ToggleFillermark, Download, Disabled }`.

### 1.16 `EpisodeDownloadAction` (`app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt:42-48`)

`enum class EpisodeDownloadAction { START, START_NOW, CANCEL, DELETE, SHOW_QUALITIES }`.

### 1.17 Other enums in the caller signatures

- `DownloadAction` (`app/src/main/java/eu/kanade/presentation/entries/...`, declared in the same package as `EntryScreenConstants.kt`): `NEXT_1_ITEM, NEXT_5_ITEMS, NEXT_10_ITEMS, NEXT_25_ITEMS, UNVIEWED_ITEMS`.
- `EditCoverAction` (same file): `EDIT, DELETE`.
- `EntryScreenItem` (`app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt`): `INFO_BOX, ACTION_ROW, DESCRIPTION_WITH_TAG, ITEM_HEADER, ITEM, AIRING_TIME` (Compose keys, dropped in Rust).

### 1.18 `AnimeRelationGroup` and `AnimeCover`

- `data class AnimeRelationGroup(val name: String, val anime: List<Anime>)` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeRelationGroup.kt:3-6`).
- `AnimeCover(animeId: Long, sourceId: Long, isAnimeFavorite: Boolean, url: String?, lastModified: Long)` built in `AnimeSeasonListItem.kt:43-49`. `anime.asAnimeCover()` for related items.

## 2. Bit-flag layouts

All flags are `Long` values. Getter pattern: `(flags & MASK)`. Setter pattern used by the dialogs: `flags = (flags and MASK.inv()) or VALUE` (the dialogs send the `VALUE`; see p1-topic-flags-filters-and-seasons.md).

### 2.1 `episodeFlags` (`episode_flags` column). Source: `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:217-255`

| Constant | Value | Bits | Meaning |
|---|---|---|---|
| `SHOW_ALL` | `0x00000000` | | filter off (for any filter group) |
| `EPISODE_SORT_DESC` | `0x00000000` | | sort direction descending |
| `EPISODE_SORT_ASC` | `0x00000001` | bit 0 | sort direction ascending |
| `EPISODE_SORT_DIR_MASK` | `0x00000001` | bit 0 | |
| `EPISODE_SHOW_UNSEEN` | `0x00000002` | bit 1 | unseen filter "is" |
| `EPISODE_SHOW_SEEN` | `0x00000004` | bit 2 | seen filter "is" |
| `EPISODE_UNSEEN_MASK` | `0x00000006` | bits 1-2 | |
| `EPISODE_SHOW_DOWNLOADED` | `0x00000008` | bit 3 | downloaded "is" |
| `EPISODE_SHOW_NOT_DOWNLOADED` | `0x00000010` | bit 4 | downloaded "not" |
| `EPISODE_DOWNLOADED_MASK` | `0x00000018` | bits 3-4 | |
| `EPISODE_SHOW_BOOKMARKED` | `0x00000020` | bit 5 | bookmark "is" |
| `EPISODE_SHOW_NOT_BOOKMARKED` | `0x00000040` | bit 6 | bookmark "not" |
| `EPISODE_BOOKMARKED_MASK` | `0x00000060` | bits 5-6 | |
| `EPISODE_SHOW_FILLERMARKED` | `0x00000080` | bit 7 | filler "is" |
| `EPISODE_SHOW_NOT_FILLERMARKED` | `0x00000100` | bit 8 | filler "not" |
| `EPISODE_FILLERMARKED_MASK` | `0x00000180` | bits 7-8 | |
| `EPISODE_SORTING_SOURCE` | `0x00000000` | | sort by source order |
| `EPISODE_SORTING_NUMBER` | `0x00000200` | bits 9-10 value 1 | sort by episode number |
| `EPISODE_SORTING_UPLOAD_DATE` | `0x00000400` | bits 9-10 value 2 | sort by upload date |
| `EPISODE_SORTING_ALPHABET` | `0x00000600` | bits 9-10 value 3 | sort alphabetically |
| `EPISODE_SORTING_MASK` | `0x00000600` | bits 9-10 | |
| `EPISODE_SHOW_PREVIEWS` | `0x00000000` | | thumbnails shown |
| `EPISODE_SHOW_NOT_PREVIEWS` | `0x00000800` | bit 11 | thumbnails hidden |
| `EPISODE_PREVIEWS_MASK` | `0x00000800` | bit 11 | |
| `EPISODE_SHOW_SUMMARIES` | `0x00000000` | | summaries shown |
| `EPISODE_SHOW_NOT_SUMMARIES` | `0x00001000` | bit 12 | summaries hidden |
| `EPISODE_SUMMARIES_MASK` | `0x00001000` | bit 12 | |
| `EPISODE_DISPLAY_NAME` | `0x00000000` | | title shown as name |
| `EPISODE_DISPLAY_NUMBER` | `0x00100000` | bit 20 | title shown as "Episode n" |
| `EPISODE_DISPLAY_MASK` | `0x00100000` | bit 20 | |

Important: `EPISODE_SHOW_PREVIEWS` and `EPISODE_SHOW_SUMMARIES` are zero. "Shown" is the all-zero state, and "hidden" sets the bit. Bits 13-19 and 21-63 are unused in this layout.

Getter behaviour: `sortDescending() = (episodeFlags & EPISODE_SORT_DIR_MASK) == EPISODE_SORT_DESC`. So a zero direction bit means descending (the default, matching `default_chapter_sort_by_ascending_or_descending` = `EPISODE_SORT_DESC`).

`unseenFilter` maps `EPISODE_UNSEEN_MASK` values: `EPISODE_SHOW_UNSEEN` → `ENABLED_IS`, `EPISODE_SHOW_SEEN` → `ENABLED_NOT`, else `DISABLED`. Same pattern for `downloadedFilter` (`SHOW_DOWNLOADED`/`SHOW_NOT_DOWNLOADED`), `bookmarkedFilter`, `fillermarkedFilter`.

### 2.2 `seasonFlags` (`season_flags` column). Source: `Anime.kt:257-308`

| Constant | Value | Bits | Meaning |
|---|---|---|---|
| `SEASON_SORT_DESC` | `0x00000000` | | direction descending |
| `SEASON_SORT_ASC` | `0x00000001` | bit 0 | direction ascending |
| `SEASON_SORT_DIR_MASK` | `0x00000001` | bit 0 | |
| `SEASON_SHOW_DOWNLOADED` | `0x00000002` | bit 1 | downloaded "is" |
| `SEASON_SHOW_NOT_DOWNLOADED` | `0x00000004` | bit 2 | downloaded "not" |
| `SEASON_DOWNLOADED_MASK` | `0x00000006` | bits 1-2 | |
| `SEASON_SHOW_UNSEEN` | `0x00000008` | bit 3 | unseen "is" |
| `SEASON_SHOW_SEEN` | `0x00000010` | bit 4 | unseen "not" (seen) |
| `SEASON_UNSEEN_MASK` | `0x00000018` | bits 3-4 | |
| `SEASON_SHOW_STARTED` | `0x00000020` | bit 5 | started "is" |
| `SEASON_SHOW_NOT_STARTED` | `0x00000040` | bit 6 | started "not" |
| `SEASON_STARTED_MASK` | `0x00000060` | bits 5-6 | |
| `SEASON_SHOW_COMPLETED` | `0x00000080` | bit 7 | completed "is" |
| `SEASON_SHOW_NOT_COMPLETED` | `0x00000100` | bit 8 | completed "not" |
| `SEASON_COMPLETED_MASK` | `0x00000180` | bits 7-8 | |
| `SEASON_SHOW_BOOKMARKED` | `0x00000200` | bit 9 | bookmarked "is" |
| `SEASON_SHOW_NOT_BOOKMARKED` | `0x00000400` | bit 10 | bookmarked "not" |
| `SEASON_BOOKMARKED_MASK` | `0x00000600` | bits 9-10 | |
| `SEASON_SHOW_FILLERMARKED` | `0x00000800` | bit 11 | filler "is" |
| `SEASON_SHOW_NOT_FILLERMARKED` | `0x00001000` | bit 12 | filler "not" |
| `SEASON_FILLERMARKED_MASK` | `0x00001800` | bits 11-12 | |
| `SEASON_SORT_SOURCE` | `0x00000000` | | by source order |
| `SEASON_SORT_SEASON` | `0x00002000` | bits 13-15 value 1 | by season number |
| `SEASON_SORT_UPLOAD` | `0x00004000` | value 2 | by upload date |
| `SEASON_SORT_ALPHABET` | `0x00006000` | value 3 | alphabetically |
| `SEASON_SORT_COUNT` | `0x00008000` | value 4 | by unseen count |
| `SEASON_SORT_LAST_SEEN` | `0x0000A000` | value 5 | by last seen |
| `SEASON_SORT_FETCHED` | `0x0000C000` | value 6 | by episode fetch date |
| `SEASON_SORT_MASK` | `0x0000E000` | bits 13-15 | |
| `SEASON_GRID_DISPLAY_MODE_BIT_OFFSET` | `16` | | |
| `SEASON_GRID_DISPLAY_MODE_MASK` | `0x00030000` | bits 16-17 | `SeasonDisplayMode` value 0..3 |
| `SEASON_GRID_DISPLAY_SIZE_BIT_OFFSET` | `18` | | |
| `SEASON_GRID_DISPLAY_SIZE_MASK` | `0x003C0000` | bits 18-21 | grid size 0..15 (UI slider caps at 10) |
| `SEASON_OVERLAY_DOWNLOADED_MASK` | `0x00400000` | bit 22 | overlay on when set |
| `SEASON_OVERLAY_UNSEEN_MASK` | `0x00800000` | bit 23 | |
| `SEASON_OVERLAY_LOCAL_MASK` | `0x01000000` | bit 24 | |
| `SEASON_OVERLAY_LANG_MASK` | `0x02000000` | bit 25 | |
| `SEASON_OVERLAY_CONT_MASK` | `0x04000000` | bit 26 | continue-watching button |
| `SEASON_DISPLAY_MODE_SOURCE` | `0x00000000` | | title from source |
| `SEASON_DISPLAY_MODE_NUMBER` | `0x08000000` | bit 27 | title "Season n" |
| `SEASON_DISPLAY_MODE_MASK` | `0x08000000` | bit 27 | |

Overlays use "set bit means on" (`(seasonFlags & MASK) == MASK`). Unlike the episode previews/summaries, zero means off.

Season filter values for `SEASON_*_FILTER` tri-states follow the same pattern as episodes (`_SHOW_X` = "is", `_SHOW_NOT_X` = "not").

### 2.3 `viewerFlags` (`viewer` column). Source: `Anime.kt:310-313`, getters at `Anime.kt:82-95`

| Constant | Value | Bits | Getter |
|---|---|---|---|
| `ANIME_INTRO_MASK` | `0x0000000000000FF` | bits 0-7 | `skipIntroLength: Int = (viewerFlags & 0xFF).toInt()` |
| `ANIME_AIRING_EPISODE_MASK` | `0x000000000FFFF00` | bits 8-23 | `nextEpisodeToAir: Int = (viewerFlags & MASK).removeHexZeros(2).toInt()` (divide by 256) |
| `ANIME_AIRING_TIME_MASK` | `0x0FFFFFFFF000000` | bits 24-55 | `nextEpisodeAiringAt: Long = (viewerFlags & MASK).removeHexZeros(6)` (divide by 2^24) |
| `ANIME_INTRO_DISABLE_MASK` | `0x100000000000000` | bit 56 | `skipIntroDisable = (viewerFlags & MASK) == MASK` |

Verified: the airing-time mask spans bits 24 to 55 (eight hex F digits from position 6 to 13 counting from the right). It does not overlap bit 56.

`removeHexZeros` divides as a `Double` and then converts with `toLong()`. Values under 2^53 are exact. The airing timestamp unit is decided by the writer (part 2 and the AniChart tracker, `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:27`); the UI reads it into `state.airingTime` (ms per the countdown code).

Proposal (Rust): model these as typed accessors over `i64` with `const` masks, plus property tests that the three airing-related masks are disjoint from `ANIME_INTRO_DISABLE_MASK`.

## 3. Enumerated values used at runtime

### 3.1 Status values displayed (see section 1.10).

### 3.2 `EpisodeSwipeAction` stored values

`LibraryPreferences.swipeEpisodeStartAction()` = `preferenceStore.getEnum("pref_episode_swipe_end_action", EpisodeSwipeAction.ToggleSeen)`. `swipeEpisodeEndAction()` = `preferenceStore.getEnum("pref_episode_swipe_start_action", EpisodeSwipeAction.ToggleBookmark)` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:361-367`). The stored form is produced by the preference store (outside this scope).

### 3.3 `TriState` mapping for the Anime flag getters

| Raw masked value | Tri-state |
|---|---|
| the `..._SHOW_X` constant | `ENABLED_IS` |
| the `..._SHOW_NOT_X` constant | `ENABLED_NOT` |
| 0 | `DISABLED` |

## 4. Preference keys in scope

Keys are the exact strings. Defaults are the Kotlin defaults. "Module" is the Kotlin accessor. "Used by" is where this scope reads them.

| Key | Default | Module | Used by |
|---|---|---|---|
| `pref_downloaded_only` (wrapped in `Preference.appStateKey(...)`) | `false` | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-19` | `EpisodeSettingsDialog`, `SeasonSettingsDialog`, `Anime.downloadedFilter` |
| `show_next_episode_airing_time` | `true` | `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:43-46` (`showNextEpisodeAiringTime()`) | `AnimeScreen` airing row gate (passed as `showNextEpisodeAirTime`) |
| `pref_episode_swipe_end_action` | `EpisodeSwipeAction.ToggleSeen` | `LibraryPreferences.swipeEpisodeStartAction()` (name inverted) | row swipe, first (start) action |
| `pref_episode_swipe_start_action` | `EpisodeSwipeAction.ToggleBookmark` | `LibraryPreferences.swipeEpisodeEndAction()` (name inverted) | row swipe, second (end) action |
| `use_external_downloader` | `false` | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:14` | `EpisodeOptionsDialogScreen` download buttons |
| `default_episode_filter_by_seen` | `Anime.SHOW_ALL` (0) | `LibraryPreferences.kt:174` (`filterEpisodeBySeen`) | Set-as-default write |
| `default_episode_filter_by_downloaded` | 0 | `LibraryPreferences.kt:179-180` | Set-as-default write |
| `default_episode_filter_by_bookmarked` | 0 | `LibraryPreferences.kt:185-186` | Set-as-default write |
| `default_episode_filter_by_fillermarked` | 0 | `LibraryPreferences.kt:191-192` | Set-as-default write |
| `default_episode_sort_by_source_or_number` | `Anime.EPISODE_SORTING_SOURCE` (0) | `LibraryPreferences.kt:195-198` | Set-as-default write |
| `default_chapter_display_by_name_or_number` | `Anime.EPISODE_DISPLAY_NAME` (0) | `LibraryPreferences.kt:205-208` (`displayEpisodeByNameOrNumber`) | **shared key with manga chapters (defect D-1)** |
| `default_chapter_sort_by_ascending_or_descending` | `Anime.EPISODE_SORT_DESC` (0) | `LibraryPreferences.kt:215-218` (`sortEpisodeByAscendingOrDescending`) | **shared key with manga chapters (defect D-1)** |
| `default_episode_show_thumbnail_previews` | `Anime.EPISODE_SHOW_PREVIEWS` (0) | `LibraryPreferences.kt:225-228` | Set-as-default write |
| `default_episode_show_summaries` | `Anime.EPISODE_SHOW_SUMMARIES` (0) | `LibraryPreferences.kt:230-233` | Set-as-default write |
| `default_season_filter_by_downloaded` | 0 | `LibraryPreferences.kt:263-264` | Season set-as-default |
| `default_season_filter_by_unseen` | 0 | `LibraryPreferences.kt:265-266` | |
| `default_season_filter_by_started` | 0 | `LibraryPreferences.kt:268-269` | |
| `default_season_filter_by_completed` | 0 | `LibraryPreferences.kt:271-272` | |
| `default_season_filter_by_bookmarked` | 0 | `LibraryPreferences.kt:274-275` | |
| `default_season_filter_by_fillermarked` | 0 | `LibraryPreferences.kt:277-278` | |
| `default_season_sort_by_source_or_number` | `Anime.SEASON_SORT_SOURCE` (0) | `LibraryPreferences.kt:280-283` | |
| `default_season_sort_by_ascending_or_descending` | `Anime.SEASON_SORT_DESC` (0) | `LibraryPreferences.kt:285-288` | |
| `default_season_grid_display_mode` | `SeasonDisplayMode.toLong(CompactGrid)` (0) | `LibraryPreferences.kt:290-293` | |
| `default_season_grid_display_size` | `0` | `LibraryPreferences.kt:295-298` | |
| `default_season_download_overlay` | `false` | `LibraryPreferences.kt:300-303` | |
| `default_season_unseen_overlay` | `true` | `LibraryPreferences.kt:305-308` | |
| `default_season_local_overlay` | `true` | `LibraryPreferences.kt:310-313` | |
| `default_season_lang_overlay` | `false` | `LibraryPreferences.kt:315-318` | |
| `default_season_continue_overlay` | `true` | `LibraryPreferences.kt:320-323` | |
| `default_season_display_mode` | `Anime.SEASON_DISPLAY_MODE_SOURCE` (0) | `LibraryPreferences.kt:325-330` | |

Defect D-1 (OBSERVED): `displayEpisodeByNameOrNumber()` and `sortEpisodeByAscendingOrDescending()` read the manga keys `default_chapter_display_by_name_or_number` and `default_chapter_sort_by_ascending_or_descending`. Saving episode defaults overwrites the manga chapter defaults and vice versa. Proposal: use the separate keys `default_episode_display_by_name_or_number` and `default_episode_sort_by_ascending_or_descending` in the Rust build, and migrate the value on first run from the chapter key only if the user has never set an episode default (documented in p1-edge-cases-and-tests.md E-3).

Note on `LibraryPreferences.setEpisodeSettingsDefault(anime)` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt`, the function after `showEpisodeSummaries()`): it writes nine values in one call (filter-by-seen, filter-by-downloaded, filter-by-bookmarked, filter-by-fillermarked, sort-by-source-or-number, display, sort direction, previews, summaries). The display and sort-direction writes go to the chapter keys (D-1), so saving episode defaults changes the manga chapter defaults too.

## 5. Files and folders on disk

| Artifact | Path (Android) | Naming | Source |
|---|---|---|---|
| Custom cover | `getExternalFilesDir("animecovers/custom")` or `filesDir/animecovers/custom` | `md5(animeId.toString())` (no extension) | `app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt:21-51`, `DiskUtil.hashKeyForDisk` = `Hash.md5` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:32-34`) |
| Cover thumbnail cache | `animecovers/` (same base) | managed by the cover cache (area 34) | `AnimeCoverCache.kt:21` |
| Custom background | `animebackgrounds/custom/<md5(animeId)>` | same scheme | `app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeBackgroundCache.kt:21-52` (`getCustomBackgroundFile`) |
| Share temp image | `context.cacheImageDir` (`Location.Cache`) | `"${name}.${ext}"` where name is `cover` or `background` | `ImageSaver.kt:35-47`, `Location.kt`/`ImageSaver.kt:186-210` |
| Saved image (API 29+) | `MediaStore` `DIRECTORY_PICTURES`, or `DIRECTORY_DOCUMENTS` when MIME type unsupported, relative path `Location.Pictures(relativePath)` | `"${name}.${ext}"` | `ImageSaver.kt:65-90` (`saveApi29`) |
| Saved image (API < 29) | `Environment.getExternalStoragePublicDirectory(DIRECTORY_PICTURES)/<app_name>/<relativePath>/` | `"${name}.${ext}"` | `ImageSaver.kt:186-210` |

`name` is `"cover"` or `"background"` (`AnimeImageScreenModel.kt:130`). `ext` comes from `ImageUtil.findImageType(data)` (magic-byte detection); unknown content throws `IllegalArgumentException("Not an image")`. `relativePath` for saves is the anime title (`Location.Pictures(anime.title)`, `AnimeImageScreenModel.kt:131`). The file name is passed through `DiskUtil.buildValidFilename`, but the directory name is not (defect D-4 in p1-edge-cases-and-tests.md).

Proposal (Linux): `$XDG_DATA_HOME/kurobayomi/anime/covers/custom/<md5(id)>` for custom covers and `.../backgrounds/custom/<md5(id)>` for custom backgrounds, to keep the Android naming. Saved images go to `$XDG_PICTURES_DIR/Kurobayomi/<sanitised title>/<name>.<ext>` (default `~/Pictures/Kurobayomi/...`). Share uses `$XDG_RUNTIME_DIR` or a `tempfile` directory and the `gio open` / `gtk::FileLauncher` path.

## 6. Network endpoints

This scope makes **no direct HTTP calls**. The network-related reads are:

| What | Who | Where |
|---|---|---|
| Cover image | Coil `AsyncImage` / `ImageRequest.data(anime)` | `anime.thumbnailUrl` (cover), `anime.backgroundUrl` with `useBackground(true)` (backdrop and background page) |
| Episode thumbnail | Coil `ImageRequest.data(previewUrl)` | `episode.previewUrl` (`AnimeEpisodeListItem.kt:369-372`) |
| Hoster list | `EpisodeLoader.getHosters(episode, anime, source)` | player area (not in this scope); HTTP sources call `AnimeHttpSource` methods |
| Hoster videos | `EpisodeLoader.loadHosterVideos(source, hoster, force = false)` | player area |
| Resolved video | `HosterLoader.getResolvedVideo(source, video)` | player area |
| Source preferences screen | `navigator.push(AnimeSourcePreferencesScreen(state.source.id))` | `AnimeScreen.kt:178-180` |

Headers, auth and response shapes for hoster and video calls are defined by the source API and are not repeated here.

## 7. Database tables and columns touched

Reads and writes happen in part 2 (ScreenModel) and in the image model here. Columns are listed to make the port exact.

### 7.1 `animes` (`data/src/main/sqldelightanime/dataanime/animes.sq:8-44`)

Columns used by this scope: `_id`, `source`, `url`, `artist`, `author`, `description`, `genre` (List<String>), `title`, `status`, `thumbnail_url`, `favorite`, `last_update`, `next_update`, `initialized`, `viewer` (viewerFlags), `episode_flags`, `cover_last_modified`, `date_added`, `update_strategy`, `calculate_interval`, `last_modified_at`, `favorite_modified_at`, `version`, `is_syncing`, `fetch_type`, `parent_id`, `season_flags`, `season_number` (REAL), `season_source_order`, `background_url`, `background_last_modified`, `memo`.

Indexes: `animelib_favorite_index ON animes(favorite) WHERE favorite = 1`, `animes_url_index`, `animes_parent_id`, `animes_fetch_type`.

Writes from the image model: `UpdateAnime.awaitUpdateCoverLastModified(animeId)` (sets `cover_last_modified`) and `UpdateAnime.awaitUpdateBackgroundLastModified(animeId)` (sets `background_last_modified`). Edits: `editCover` and `editBackground` (`app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt:36-...`) write the file and update the matching last-modified column.

### 7.2 `episodes` (`data/src/main/sqldelightanime/dataanime/episodes.sq:4-27`)

Columns: `_id, anime_id, url, name, scanlator, seen, bookmark, last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing, summary, preview_url, fillermark, memo`. Foreign key `anime_id` to `animes(_id)` with `ON DELETE CASCADE`.

Writes from the list UI (mark seen, bookmark, fillermark) use the episode update interactors (part 2).

### 7.3 Other tables reached through interactors (outside this scope)

`animehistory`, `anime_sync`, `anime_relations` (related groups), `animes_categories`, `categories`, `animesources`. See area 04 for the schema.

## 8. Rust representations

Proposal (summary; full design in p1-rust-design.md):

```rust
// flags: newtype over i64 with const masks; no bitflags crate dependency needed
pub struct EpisodeFlags(pub i64);
pub struct SeasonFlags(pub i64);
pub struct ViewerFlags(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState { Disabled, EnabledIs, EnabledNot }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchType { Seasons = 0, Episodes = 1 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonDisplayMode { CompactGrid = 0, ComfortableGrid = 1, CoverOnlyGrid = 2, List = 3 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState { NotDownloaded = 0, Queue = 1, Downloading = 2, Downloaded = 3, Error = 4 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoState { Queue, LoadVideo, Ready, Error }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpisodeSwipeAction { ToggleSeen, ToggleBookmark, ToggleFillermark, Download, Disabled }
```

Stored string names for the swipe enum and the `LibraryDisplayMode` serialiser are defined by the existing preference store in the Kotlin (not in scope); the Rust store uses the same enum names as text (documented in area 02).

## 9. Data that the Rust port must NOT invent

- No new network call. Hoster and video loading comes from the source API (area 16 and the player area).
- The `fetchType` default in SQL is `1` (Episodes). Do not default new anime to Seasons.
- Overlay defaults for a brand-new anime come from the preferences in section 4 at creation time (`LibraryPreferences.setSeasonSettingsDefault`, `LibraryPreferences.kt:336-348`), not from the dialog's `?: true` fallbacks.
