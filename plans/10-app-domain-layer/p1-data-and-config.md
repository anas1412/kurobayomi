# Area 10 - data models, flags, preferences, files (part 1 of 1)

All paths repo-relative to `/home/blackbox/aniyomi-src`. The authoritative owners of the entity classes are area:07 (domain models) and area:03/area:04 (DDL); this file lists what this area reads/writes and every value it depends on.

## 1. Models touched by this area

### 1.1 Source-side models (extension API, area:15 / area:16)

| Model | Fields (Kotlin name: type, default of `create()`) | Source |
|---|---|---|
| `SManga` | `url: String`, `title: String` (lateinit; reading before set throws), `artist: String?`, `author: String?`, `description: String?`, `genre: String?` (comma+space separated), `status: Int`, `thumbnail_url: String?`, `update_strategy: UpdateStrategy` (default ALWAYS_UPDATE), `initialized: Boolean` (false) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt` |
| `SManga` status constants | `UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6` | `SManga.kt:48-54` |
| `SAnime` | `url, title, thumbnail_url?, background_url?, artist?, author?, status: Int, description?, genre?, update_strategy: AnimeUpdateStrategy, fetch_type: FetchType, season_number: Double, initialized: Boolean, memo: JsonObject` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt` |
| `SAnime` status constants | `UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6, UPCOMING 7` | `SAnime.kt:72-79` |
| `SChapter` | `url, name, date_upload: Long (0 = unknown), chapter_number: Float, scanlator: String?` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapter.kt` |
| `SEpisode` | `url, name, episode_number: Float, fillermark: Boolean, scanlator?, date_upload: Long, summary?, preview_url?, memo: JsonObject` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SEpisode.kt` |
| `UpdateStrategy` / `AnimeUpdateStrategy` | `ALWAYS_UPDATE`, `ONLY_FETCH_ONCE` (stored 0/1, see area:05 adapters) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt` |
| `FetchType` | `Seasons`, `Episodes` (stored 0/1, area:05) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt` |

`getGenres()` (SManga/SAnime): `null` if `genre` null or blank; else `genre.split(", ").map(trim).filterNot(isBlank).distinct()`. Note the separator is the two-character string `", "`; a source that returns `"A,B"` yields one genre `"A,B"`.

### 1.2 Domain entities used (fields only, see area:07)

`Manga` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`): `id: Long, source: Long, favorite: Boolean, lastUpdate: Long, nextUpdate: Long, fetchInterval: Int, dateAdded: Long, viewerFlags: Long, chapterFlags: Long, coverLastModified: Long, url: String, title: String, artist?, author?, description?, genre: List<String>?, status: Long, thumbnailUrl?, updateStrategy, initialized: Boolean, lastModifiedAt: Long, favoriteModifiedAt: Long?, version: Long`.

`Anime` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`): the manga fields with `episodeFlags` instead of `chapterFlags`, plus `backgroundLastModified: Long, backgroundUrl: String?, fetchType: FetchType, parentId: Long?, seasonFlags: Long, seasonNumber: Double, seasonSourceOrder: Long, memo: JsonObject`. Derived: `expectedNextUpdate = nextUpdate.takeIf { status != SAnime.COMPLETED }` as an `Instant`.

`Chapter` (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt`): `id, mangaId, read, bookmark, lastPageRead: Long, dateFetch, sourceOrder, url, name, dateUpload, chapterNumber: Double, scanlator?, lastModifiedAt, version`. `create()` defaults: `id -1, mangaId -1, read false, bookmark false, lastPageRead 0, dateFetch 0, sourceOrder 0, url "", name "", dateUpload -1, chapterNumber -1.0, scanlator null, lastModifiedAt 0, version 1`. `isRecognizedNumber = chapterNumber >= 0`.

`Episode` (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt`): `id, animeId, seen, bookmark, fillermark, lastSecondSeen: Long, totalSeconds: Long, dateFetch, sourceOrder, url, name, dateUpload, episodeNumber: Double, scanlator?, summary?, previewUrl?, lastModifiedAt, version, memo: JsonObject`; `create()` defaults as chapter plus `fillermark false, lastSecondSeen 0, totalSeconds 0, summary null, previewUrl null, memo {}`; `isRecognizedNumber = episodeNumber >= 0`.

Update objects (`MangaUpdate`, `AnimeUpdate`, `ChapterUpdate`, `EpisodeUpdate`): same field names, every field nullable with default `null` except `id`; `null` means "do not change" (SQL `coalesce`). `Chapter.toChapterUpdate()` copies every field including `scanlator` and `version`. `AnimeUpdate` field list: `id, source, favorite, lastUpdate, nextUpdate, fetchInterval, dateAdded, viewerFlags, episodeFlags, coverLastModified, backgroundLastModified, url, title, artist, author, description, genre, status, thumbnailUrl, backgroundUrl, updateStrategy, initialized, version, fetchType, parentId, seasonFlags, seasonNumber, seasonSourceOrder, memo` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt`).

### 1.3 Tracks

Domain `MangaTrack` (`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt`): `id: Long, mangaId: Long, trackerId: Long, remoteId: Long, libraryId: Long?, title: String, lastChapterRead: Double, totalChapters: Long, status: Long, score: Double, remoteUrl: String, startDate: Long, finishDate: Long, private: Boolean`. `AnimeTrack`: `animeId`, `lastEpisodeSeen: Double`, `totalEpisodes: Long`, otherwise identical.

Tracker DB models (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt`): `id: Long?, manga_id/anime_id, tracker_id: Long, remote_id, library_id: Long?, title, last_chapter_read/last_episode_seen: Double, total_chapters/total_episodes: Long, score: Double, status: Long, started_reading_date/started_watching_date, finished_reading_date/finished_watching_date, tracking_url, private`. In the Rust port the two layers merge into one `Track` struct (see p1-rust-design.md section 3.5) so `toDbTrack`/`toDomainTrack` become identity (the only behaviour kept is "id may be absent").

### 1.4 Delayed tracking queue item

`DelayedTrackingItem(trackId: Long, lastChapterRead: Float)` (manga, nested in `DelayedMangaTrackingStore`), `DelayedAnimeTrackingItem(trackId: Long, lastEpisodeSeen: Float)`. Precision is `Float` (32 bit): `12.3` is stored and replayed as `12.3000001907...`.

### 1.5 Extension models (`domain/src/main/java/eu/kanade/tachiyomi/extension/{anime,manga}/model/`)

`AnimeExtension` sealed class: common abstract `name, pkgName, versionName, versionCode: Long, libVersion: Double, lang: String?, isNsfw, isTorrent`.
- `Installed`: `lang: String, pkgFactory: String?, sources: List<AnimeSource>, icon: Drawable?, hasUpdate = false, isObsolete = false, isShared, store: AnimeExtensionStore? = null`.
- `Available`: `sources: List<Available.AnimeSource(id, lang, name, baseUrl)>, apkUrl, iconUrl, store: AnimeExtensionStore` (`toStubSource()`).
- `Untrusted`: `signatureHash: String, lang = null, isNsfw = false, isTorrent = false`.
`MangaExtension`: same without `isTorrent`, `Installed.repoUrl: String?` instead of `store`.
Bundles: `AnimeExtensions(updates, installed, available, untrusted)`, `MangaExtensions` identical shape.
`AnimeExtensionSourceItem(source: AnimeSource, enabled: Boolean, labelAsName: Boolean)`, `MangaExtensionSourceItem(source: MangaSource, enabled: Boolean, labelAsName: Boolean)`.

### 1.6 Source list models

`Source` (manga, `domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt`): `id: Long, lang: String, name: String, supportsLatest: Boolean, isStub: Boolean, pin: Pins = Pins.unpinned, isUsedLast: Boolean = false, isExcludedFromDataSaver: Boolean = false`. `AnimeSource` (`domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt`) without the data-saver field. `visualName = if (lang.isEmpty()) name else "$name (${lang.uppercase()})"`. `key()` = `"$id-lastused"` if `isUsedLast` else `"$id"` (lazy list key so the duplicated last-used row does not clash).

`Pin` codes: `Unpinned 0b00`, `Pinned 0b01`, `Actual 0b10`. `Pins(code: Int = 0)`: `contains(pin) = (pin.code and code) == pin.code` (so `Unpinned in anyPins` is always true), `plus(pin) = Pins(code or pin.code)`, `minus(pin) = Pins(code xor pin.code)` (XOR, not AND-NOT: subtracting `Actual` from a value without that bit SETS it). Constants: `Pins.unpinned = 0b00`, `Pins.pinned = Pinned or Actual = 0b11`. UI meaning (`MangaSourcesScreenModel.kt:86-91`): `isUsedLast` -> group "last_used"; `Actual in pin` -> group "pinned"; else group by `lang`. The last-used duplicate of a pinned source has `0b01` (still shows "Unpin"), of an unpinned source `0b10` (never examined because `isUsedLast` wins).

## 2. Enumerations with stored values

| Enum | Values and stored form | Where stored |
|---|---|---|
| `TriState` | `DISABLED, ENABLED_IS, ENABLED_NOT` (derived from flag bits, never stored by name) | derived |
| `AutoTrackState` | `ALWAYS, ASK, NEVER` stored as the enum NAME string | pref `pref_auto_update_manga_on_mark_read` |
| `SetMigrateSorting.Mode` | `ALPHABETICAL, TOTAL` stored as NAME | pref `pref_migration_sorting` |
| `SetMigrateSorting.Direction` | `ASCENDING, DESCENDING` stored as NAME | pref `pref_migration_direction` |
| `SourcePreferences.DataSaver` | `NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT` stored as NAME | pref `data_saver` |
| `LibraryDisplayMode` (sourceDisplayMode) | `COMPACT_GRID, COMFORTABLE_GRID, COVER_ONLY_GRID, LIST` (strings); unknown -> `COMPACT_GRID` | pref `pref_display_mode_catalogue` |
| `ComicInfoPublishingStatus` | `Ongoing 1, Completed 2, Licensed 3, "Publishing finished" 4, Cancelled 5, "On hiatus" 6, Unknown 0` (text <-> `SManga` int) | ComicInfo.xml element `PublishingStatusTachiyomi` |
| `ReadingMode.flagValue` | `DEFAULT 0, LEFT_TO_RIGHT 1, RIGHT_TO_LEFT 2, VERTICAL 3, WEBTOON 4, CONTINUOUS_VERTICAL 5` | `mangas.viewer` bits 0-2 |
| `ReaderOrientation.flagValue` | `DEFAULT 0x00, FREE 0x08, PORTRAIT 0x10, LANDSCAPE 0x18, LOCKED_PORTRAIT 0x20, LOCKED_LANDSCAPE 0x28, REVERSE_PORTRAIT 0x30` | `mangas.viewer` bits 3-5 |
| `SetReadStatus.Result` | `Success, NoChapters, InternalError(error)` | in memory |
| `SetSeenStatus.Result` | `Success, NoEpisodes, InternalError(error)` | in memory |

Fallbacks: `getEnum` stores `it.name` and on read tries `enumValueOf`, returning the default on `IllegalArgumentException` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:27`).

## 3. Bit-flag layouts

### 3.1 `mangas.viewer` (manga `viewerFlags`)
```
bit  5 4 3 | 2 1 0
     orient | reading mode
mask 0x38  | 0x07
```
`ReadingMode.MASK = 0x00000007`, `ReaderOrientation.MASK = 0x00000038` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`, `ReaderOrientation.kt`). Bits 6 and above are unused. `setFlag(flag, mask) = (v & ~mask) | (flag & mask)`.

### 3.2 `animes.viewer` (anime `viewerFlags`, 64-bit)
```
bit 56         : ANIME_INTRO_DISABLE_MASK      = 0x100000000000000  (1 = skip-intro button disabled)
bits 24..55    : ANIME_AIRING_TIME_MASK        = 0x0FFFFFFFF000000  (32-bit unsigned: next airing epoch seconds)
bits 8..23     : ANIME_AIRING_EPISODE_MASK     = 0x000000000FFFF00  (16-bit: next airing episode number)
bits 0..7      : ANIME_INTRO_MASK              = 0x0000000000000FF  (skip-intro length in seconds, 0-255)
```
Source `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:310-313`. Write: episode `* 16^2`, airing time `* 16^6`, disable bit `1 * 16^14`. Read: divide by the same powers after masking. Edge values: intro length input is taken `& 0xFF` (300 -> 44); episode `& 0xFFFF`; airing time `& 0xFFFFFFFF`. Bits 57-63 are unused; the value is a signed `INTEGER` in SQLite and never reaches bit 63.

### 3.3 `chapterFlags` / `episodeFlags` / `seasonFlags` (read-only here, set by area:07)
Manga `chapterFlags` (`Manga.kt:77-103`): `SORT_DESC 0x0 / SORT_ASC 0x1 (mask 0x1)`, `SHOW_UNREAD 0x2 / SHOW_READ 0x4 (mask 0x6)`, `SHOW_DOWNLOADED 0x8 / SHOW_NOT_DOWNLOADED 0x10 (mask 0x18)`, `SHOW_BOOKMARKED 0x20 / SHOW_NOT_BOOKMARKED 0x40 (mask 0x60)`, `SORTING_SOURCE 0x000 / NUMBER 0x100 / UPLOAD_DATE 0x200 / ALPHABET 0x300 (mask 0x300)`, `DISPLAY_NAME 0x0 / DISPLAY_NUMBER 0x100000 (mask 0x100000)`.
Anime `episodeFlags` (`Anime.kt:219-255`): `SORT 0x0/0x1 (mask 0x1)`, `UNSEEN 0x2 / SEEN 0x4 (mask 0x6)`, `DOWNLOADED 0x8 / NOT 0x10 (mask 0x18)`, `BOOKMARKED 0x20 / NOT 0x40 (mask 0x60)`, `FILLERMARKED 0x80 / NOT 0x100 (mask 0x180)`, `SORTING SOURCE 0x000 / NUMBER 0x200 / UPLOAD_DATE 0x400 / ALPHABET 0x600 (mask 0x600)`, `PREVIEWS SHOW 0x0 / NOT 0x800 (mask 0x800)`, `SUMMARIES SHOW 0x0 / NOT 0x1000 (mask 0x1000)`, `DISPLAY NAME 0x0 / NUMBER 0x100000 (mask 0x100000)`.
Anime `seasonFlags` (`Anime.kt:257-308`): `SORT_DESC 0x0 / ASC 0x1`, `DOWNLOADED 0x2 / NOT 0x4 (mask 0x6)`, `UNSEEN 0x8 / SEEN 0x10 (mask 0x18)`, `STARTED 0x20 / NOT 0x40 (mask 0x60)`, `COMPLETED 0x80 / NOT 0x100 (mask 0x180)`, `BOOKMARKED 0x200 / NOT 0x400 (mask 0x600)`, `FILLERMARKED 0x800 / NOT 0x1000 (mask 0x1800)`, `SORT SOURCE 0x0 / SEASON 0x2000 / UPLOAD 0x4000 / ALPHABET 0x6000 / COUNT 0x8000 / LAST_SEEN 0xA000 / FETCHED 0xC000 (mask 0xE000)`, grid mode shift 16 mask `0x30000`, grid size shift 18 mask `0x3C0000`, overlays `DOWNLOADED 0x400000, UNSEEN 0x800000, LOCAL 0x1000000, LANG 0x2000000, CONT 0x4000000`, display mode `SOURCE 0x0 / NUMBER 0x8000000 (mask 0x8000000)`.

## 4. Preference keys (this area)

`Preference.appStateKey(k) = "__APP_STATE_" + k` (excluded from backups), `Preference.privateKey(k) = "__PRIVATE_" + k` (credentials, excluded unless "include private" is chosen). Defaults below are the exact Kotlin defaults.

### 4.1 SourcePreferences (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt`)

| Function | Key | Type | Default |
|---|---|---|---|
| `sourceDisplayMode` | `pref_display_mode_catalogue` | `LibraryDisplayMode` string | `COMPACT_GRID` |
| `enabledLanguages` | `source_languages` | string set | `LocaleHelper.getDefaultEnabledLanguages()` = `{"all", "en", Locale.getDefault().language}` |
| `showNsfwSource` | `show_nsfw_source` | bool | `true` |
| `migrationSortingMode` | `pref_migration_sorting` | enum | `ALPHABETICAL` |
| `migrationSortingDirection` | `pref_migration_direction` | enum | `ASCENDING` |
| `animeExtensionRepos` | `anime_extension_repos` | string set | empty |
| `mangaExtensionRepos` | `extension_repos` | string set | empty |
| `trustedExtensions` | `__APP_STATE_trusted_extensions` | string set | empty |
| `globalSearchFilterState` | `__APP_STATE_has_filters_toggle_state` | bool | `false` |
| `disabledAnimeSources` | `hidden_anime_catalogues` | string set of decimal ids | empty |
| `disabledMangaSources` | `hidden_catalogues` | string set | empty |
| `incognitoAnimeExtensions` | `incognito_anime_extensions` | string set of package names | empty |
| `incognitoMangaExtensions` | `incognito_manga_extensions` | string set | empty |
| `pinnedAnimeSources` | `pinned_anime_catalogues` | string set of decimal ids | empty |
| `pinnedMangaSources` | `pinned_catalogues` | string set | empty |
| `lastUsedAnimeSource` | `__APP_STATE_last_anime_catalogue_source` | long | `-1` |
| `lastUsedMangaSource` | `__APP_STATE_last_catalogue_source` | long | `-1` |
| `animeExtensionUpdatesCount` | `animeext_updates_count` | int | `0` |
| `mangaExtensionUpdatesCount` | `ext_updates_count` | int | `0` |
| `hideInAnimeLibraryItems` | `browse_hide_in_anime_library_items` | bool | `false` |
| `hideInMangaLibraryItems` | `browse_hide_in_library_items` | bool | `false` |
| `dataSaver` | `data_saver` | enum DataSaver | `NONE` |
| `dataSaverIgnoreJpeg` | `ignore_jpeg` | bool | `false` |
| `dataSaverIgnoreGif` | `ignore_gif` | bool | `true` |
| `dataSaverImageQuality` | `data_saver_image_quality` | int | `80` |
| `dataSaverImageFormatJpeg` | `data_saver_image_format_jpeg` | bool | `false` |
| `dataSaverServer` | `data_saver_server` | string | `""` |
| `dataSaverColorBW` | `data_saver_color_bw` | bool | `false` |
| `dataSaverExcludedSources` | `data_saver_excluded` | string set of decimal ids | empty |
| `dataSaverDownloader` | `data_saver_downloader` | bool | `true` |

Commented-out in source (do not port): `eh_enable_source_blacklist`, `sources_tab_categories`, `sources_tab_categories_filter`, `sources_tab_source_categories`.

### 4.2 TrackPreferences (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`)

| Function | Key | Type | Default |
|---|---|---|---|
| `trackUsername(tracker)` | `__PRIVATE_pref_mangasync_username_<trackerId>` | string | `""` |
| `trackPassword(tracker)` | `__PRIVATE_pref_mangasync_password_<trackerId>` | string | `""` |
| `trackAuthExpired(tracker)` | `__PRIVATE_pref_tracker_auth_expired_<trackerId>` | bool | `false` |
| `trackToken(tracker)` | `__PRIVATE_track_token_<trackerId>` | string | `""` |
| `anilistScoreType` | `anilist_score_type` | string | `Anilist.POINT_10` = `"POINT_10"` (others `POINT_100`, `POINT_10_DECIMAL`, `POINT_5`, `POINT_3`) |
| `autoUpdateTrack` | `pref_auto_update_manga_sync_key` | bool | `true` |
| `trackOnAddingToLibrary` | `track_on_adding_to_library` | bool | `true` |
| `showNextEpisodeAiringTime` | `show_next_episode_airing_time` | bool | `true` |
| `autoUpdateTrackOnMarkRead` | `pref_auto_update_manga_on_mark_read` | enum AutoTrackState | `ALWAYS` |

### 4.3 Preferences of other areas read here

| Key | Owner | Type | Default | Used by |
|---|---|---|---|---|
| `__APP_STATE_pref_downloaded_only` | `BasePreferences.downloadedOnly()` `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16` | bool | `false` | downloaded filter |
| `__APP_STATE_incognito_mode` | `BasePreferences.incognitoMode()` `:21` | bool | `false` | incognito state |
| `pref_remove_after_marked_as_read_key` | `DownloadPreferences.removeAfterMarkedAsRead()` `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:30` | bool | `false` | SetReadStatus, SetSeenStatus |
| `mark_duplicate_read_chapter_read` | `LibraryPreferences.markDuplicateReadChapterAsRead()` `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:377` | string set; values `"new"` / `"existing"` | empty | SyncChaptersWithSource |
| `mark_duplicate_seen_episode_seen` | `LibraryPreferences.markDuplicateSeenEpisodeAsSeen()` `:379` | string set; values `"new_episode"` / `"existing_episode"` | empty | SyncEpisodesWithSource |

## 5. On-disk state

| What | Kotlin | Format | Proposal for Rust |
|---|---|---|---|
| Delayed tracking queue | `SharedPreferences` file named `tracking_queue` (Android `shared_prefs/tracking_queue.xml`), key = track id string, value = float | Android XML map | `<data>/tracking_queue.json`: `{"manga": {"<trackId>": 12.5}, "anime": {"<trackId>": 3.0}}` written atomically (write temp file then rename); see p1-topic-tracking-orchestration.md |
| Trusted extension triples | pref set `__APP_STATE_trusted_extensions` of `"<pkg>:<versionCode>:<signatureHash>"` | area:02 store | unchanged, area:02 `Pref<Set<String>>` |
| Custom cover / background files | `MangaCoverCache.getCustomCoverFile(id)`, `AnimeBackgroundCache.getCustomBackgroundFile(id)` | image files (area:34) | `CoverCache` trait |
| ComicInfo.xml | written inside each downloaded chapter folder / CBZ (`COMIC_INFO_FILE = "ComicInfo.xml"`) | XML (area:14/23) | same |

## 6. Database tables and columns touched

| Table (DB) | Operation | By | Notes |
|---|---|---|---|
| `mangas` (manga DB, `data/src/main/sqldelight/data/mangas.sq`) | partial UPDATE via coalesce | UpdateManga, SetMangaViewerFlags | columns `viewer`, `chapter_flags`, `favorite`, `date_added`, `last_update`, `next_update`, `calculate_interval` (the `fetchInterval` field), `cover_last_modified`, `title`, `author`, `artist`, `description`, `genre`, `status`, `thumbnail_url`, `update_strategy`, `initialized`, `version` |
| `animes` (anime DB) | partial UPDATE, parent link detach `removeParentIdByIds`, INSERT via NetworkToLocal | UpdateAnime, SetAnimeViewerFlags, SyncSeasons, SyncRelated | extra columns `background_last_modified`, `background_url`, `fetch_type`, `parent_id`, `season_flags`, `season_number`, `season_source_order`, `memo` |
| `chapters` / `episodes` | INSERT many, UPDATE many (coalesce), DELETE by ids | Sync*WithSource, Set*Status, Sync*ProgressWithTrack | `chapters.update:` statement in `data/src/main/sqldelight/data/chapters.sq` is the template |
| `excluded_scanlators(manga_id, scanlator)` + index `excluded_scanlators_manga_id_index` | SELECT, INSERT, DELETE ... IN | Get/SetExcludedScanlators, backup | FK `manga_id -> mangas._id ON DELETE CASCADE` |
| `manga_sync` / `anime_sync` | SELECT by entry, UPSERT | Track*, Refresh*, AddTracks | area:06 `InsertMangaTrack` |
| `history`, `animehistory` | SELECT | AddTracks.bind (first read date) | area:06/08 |
| `extension_repos` / anime extension store tables | SELECT signing keys | TrustMangaExtension / TrustAnimeExtension | area:20 |
| `sources` (stub sources) | SELECT | via repositories in source listing | area:06 |

## 7. Network endpoints

This scope issues no HTTP request directly. Indirect network use is entirely behind collaborator interfaces:
- `Tracker.refresh/update/bind/setRemote*` (area:27 and following).
- `HttpSource.getChapterList` and `AnimeHttpSource.getEpisodeList` are called by the CALLERS of the sync interactors, not by the interactors.
- `AniChartApi` (`app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt`, area:52) produces the `(episode, airingAt)` pair that `SetAnimeViewerFlags.awaitSetNextEpisodeAiring` stores: `POST https://graphql.anilist.co` with a JSON body `{"query": "query { Media(id:<id>) { nextAiringEpisode { episode airingAt } } }"}`; for MyAnimeList entries first `query { Media(idMal:<idMal>,type: ANIME) { id } }`. `airingAt` is epoch seconds.
