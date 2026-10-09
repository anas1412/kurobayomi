# Area 07 — data models, flag layouts, preference keys, endpoints (part 1 of 1)

All paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" is what the Kotlin does. "Proposal:" is what the Rust port should do. Column names and SQL belong to area:05-data-repos-entries-items (repositories), area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations (DDL); they are quoted here only to tie a domain field to its column. The Rust structs for these models live in `kuro-core/src/data/models.rs` (owned by area:05-data-repos-entries-items, tasks T-05a-004 and T-05a-011); this area owns the behaviour (`impl` blocks, constants) in `kuro-core/src/domain/` and requires the field sets below to be identical.

Types: `Long` = `i64`, `Int` = `i32`, `Double` = `f64`, `Boolean` = `bool`, `String?` = `Option<String>`, `JsonObject` = `serde_json::Map<String, Value>`.

## 1. Entry models

### 1.1 Manga (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:11-35`)

| # | Field | Type | Default in `Manga.create()` (line 105-129) | Meaning |
|---|---|---|---|---|
| 1 | id | Long | -1 | primary key; -1 means "not yet inserted" |
| 2 | source | Long | -1 | source id (the source API (no plans folder yet)) |
| 3 | favorite | Boolean | false | in library |
| 4 | lastUpdate | Long | 0 | epoch ms of last time the chapter list changed (NOT last fetch) |
| 5 | nextUpdate | Long | 0 | epoch ms predicted next release check (section 5 of p1-topic-fetch-interval.md) |
| 6 | fetchInterval | Int | 0 | days; 0 = never computed, > 0 = computed, < 0 = user override of `abs(value)` days |
| 7 | dateAdded | Long | 0 | epoch ms when added to library; 0 when removed |
| 8 | viewerFlags | Long | 0 | reader settings: reading mode bits 0-2 (`ReadingMode.MASK = 0x00000007`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt:60`), orientation bits 3-5 (`ReaderOrientation.MASK = 0x00000038`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt:65`) |
| 9 | chapterFlags | Long | 0 | list filter/sort/display bits (section 3.1) |
| 10 | coverLastModified | Long | 0 | epoch ms; cache-buster for the cover |
| 11 | url | String | "" | source-relative URL, identity together with `source` |
| 12 | title | String | "" | |
| 13 | artist | String? | null | |
| 14 | author | String? | null | |
| 15 | description | String? | null | |
| 16 | genre | List<String>? | null | stored joined by `", "` |
| 17 | status | Long | 0 | 0 UNKNOWN, 1 ONGOING, 2 COMPLETED, 3 LICENSED, 4 PUBLISHING_FINISHED, 5 CANCELLED, 6 ON_HIATUS (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:369-375`) |
| 18 | thumbnailUrl | String? | null | |
| 19 | updateStrategy | UpdateStrategy | ALWAYS_UPDATE | 0 ALWAYS_UPDATE, 1 ONLY_FETCH_ONCE |
| 20 | initialized | Boolean | false | details fetched from the source at least once |
| 21 | lastModifiedAt | Long | 0 | epoch SECONDS, trigger-maintained (area:05-data-repos-entries-items) |
| 22 | favoriteModifiedAt | Long? | null | epoch SECONDS |
| 23 | version | Long | 0 | trigger-maintained |

`MangaUpdate` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt:5-27`): `id: Long` plus nullable-default-null versions of fields 2-12 and 13-20 and 23, i.e. exactly: source, favorite, lastUpdate, nextUpdate, fetchInterval, dateAdded, viewerFlags, chapterFlags, coverLastModified, url, title, artist, author, description, genre, status, thumbnailUrl, updateStrategy, initialized, version (20 optional fields; `lastModifiedAt` and `favoriteModifiedAt` are absent). `Manga.toMangaUpdate()` (lines 29-52) fills all 20 from the entry, so a nullable text field that is `null` in the entry stays `null` in the update, which the repository treats as "do not touch" (area:05-data-repos-entries-items): a `to_update()` round trip can never clear `artist`, `author`, `description`, `genre` or `thumbnailUrl`.

`MangaCover` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaCover.kt:8-14`): `mangaId: Long`, `sourceId: Long`, `isMangaFavorite: Boolean`, `url: String?`, `lastModified: Long`; implements marker `EntryCover` (`domain/src/main/java/tachiyomi/domain/entries/EntryCover.kt:3`, an empty interface). `Manga.asMangaCover()` maps `id, source, favorite, thumbnailUrl, coverLastModified`.

### 1.2 Anime (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:16-48`)

Fields 1-23 as Manga with these renames: `chapterFlags` becomes `episodeFlags` (field 9); `update_strategy` is `AnimeUpdateStrategy` (same two values); status additionally has `7 UPCOMING` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:519-526`). Additional fields (declaration order in the Kotlin class; `Anime.create()` lines 315-347):

| # | Field | Type | `Anime.create()` default | Meaning |
|---|---|---|---|---|
| 24 | backgroundLastModified | Long | 0 | epoch ms; cache-buster for the background image (declared right after `coverLastModified`) |
| 25 | backgroundUrl | String? | null | wide background image URL (declared right after `thumbnailUrl`) |
| 26 | fetchType | FetchType | `Episodes` | 0 `Seasons`, 1 `Episodes`; `Seasons` means the entry is a parent whose children are fetched with `getSeasonList`, `Episodes` means `getEpisodeList` (`animesource/model/FetchType.kt:437-447`) |
| 27 | parentId | Long? | null | id of the parent anime when this row is a season; null for top-level entries |
| 28 | seasonFlags | Long | 0 | season list filter/sort/display bits (section 3.3) on the PARENT entry |
| 29 | seasonNumber | Double | -1.0 | recognised season number of THIS row (a child) |
| 30 | seasonSourceOrder | Long | 0 | index of this child in the parent's source list |
| 31 | memo | JsonObject | `{}` | opaque source data (extensions-lib 17), stored as JSON bytes |

Declaration order of the Kotlin primary constructor (this order matters only for `copy`-free Rust builders, but is listed so the struct is complete): id, source, favorite, lastUpdate, nextUpdate, fetchInterval, dateAdded, viewerFlags, episodeFlags, coverLastModified, backgroundLastModified, url, title, artist, author, description, genre, status, thumbnailUrl, backgroundUrl, updateStrategy, initialized, lastModifiedAt, favoriteModifiedAt, version, fetchType, parentId, seasonFlags, seasonNumber, seasonSourceOrder, memo (31 fields).

`AnimeUpdate` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt:7-37`): `id` plus 28 optional fields: source, favorite, lastUpdate, nextUpdate, fetchInterval, dateAdded, viewerFlags, episodeFlags, coverLastModified, backgroundLastModified, url, title, artist, author, description, genre, status, thumbnailUrl, backgroundUrl, updateStrategy, initialized, version, fetchType, parentId, seasonFlags, seasonNumber, seasonSourceOrder, memo. `Anime.toAnimeUpdate()` copies all 28. Note `parentId` is `Long?` in both the entity and the update, so `toAnimeUpdate()` with `parentId = null` means "do not touch", not "clear the parent" (clearing goes through `removeParentIdByIds`, area:05-data-repos-entries-items).

`AnimeCover` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeCover.kt:8-14`): `animeId`, `sourceId`, `isAnimeFavorite`, `url`, `lastModified`; `Anime.asAnimeCover()` maps `id, source, favorite, thumbnailUrl, coverLastModified`. There is no cover descriptor for the background image.

`AnimeRelationGroup` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeRelationGroup.kt:3-6`): `name: String`, `anime: List<Anime>`.

`NoSeasonsException` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/NoSeasonsException.kt:3`): `class NoSeasonsException : Exception()` with no message.

### 1.3 SeasonAnime (`domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt:6-45`)

| Field | Type | Meaning |
|---|---|---|
| anime | Anime | the child row |
| totalCount | Long | episodes of the child |
| seenCount | Long | seen episodes |
| bookmarkCount | Long | bookmarked episodes |
| fillermarkCount | Long | fillermarked episodes |
| latestUpload | Long | max `date_upload` of its episodes, 0 when none |
| fetchedAt | Long | max `date_fetch` of its episodes |
| lastSeen | Long | last seen time (history) |

Derived: `id = anime.id`; `seen = totalCount == seenCount` (so a child with 0 episodes is "seen"); `unseenCount = totalCount - seenCount`; `hasStarted = seenCount > 0` (evaluated once at construction); `hasBookmarks = bookmarkCount > 0`; `hasFillermarks = fillermarkCount > 0`; `toLibraryAnime()` builds `LibraryAnime(anime, category = -1L, totalCount, seenCount, bookmarkCount, fillermarkCount, latestUpload, episodeFetchedAt = fetchedAt, lastSeen)`.

`SeasonDisplayMode` (`domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt:3-29`): sealed interface with `CompactGrid`, `ComfortableGrid`, `CoverOnlyGrid`, `List`; `toLong`: CompactGrid 0, ComfortableGrid 1, CoverOnlyGrid 2, List 3; `fromLong` returns the matching object and throws `IllegalArgumentException("Invalid sorting value")` for any other value (unreachable from flags because the field is 2 bits wide).

## 2. Item models

### 2.1 Chapter (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:3-50`)

| Field | Type | `Chapter.create()` default | Column (area:05-data-repos-entries-items) |
|---|---|---|---|
| id | Long | -1 | `_id` |
| mangaId | Long | -1 | `manga_id` |
| read | Boolean | false | `read` |
| bookmark | Boolean | false | `bookmark` |
| lastPageRead | Long | 0 | `last_page_read` |
| dateFetch | Long | 0 | `date_fetch` (epoch ms) |
| sourceOrder | Long | 0 | `source_order` (0 = first element the source returned = newest) |
| url | String | "" | `url` |
| name | String | "" | `name` |
| dateUpload | Long | -1 | `date_upload` (epoch ms; 0 or -1 = unknown) |
| chapterNumber | Double | -1.0 | `chapter_number`; -1 = unrecognised; -2 = "recognised as intentionally numberless" sentinel accepted by the recognition shortcut |
| scanlator | String? | null | `scanlator` |
| lastModifiedAt | Long | 0 | `last_modified_at` (epoch seconds) |
| version | Long | 1 | `version` |

Derived: `isRecognizedNumber = chapterNumber >= 0f` (so 0.0 is recognised, -1 and -2 are not). `copyFrom(other)` copies name, url, dateUpload, chapterNumber and `scanlator?.ifBlank { null }` and keeps every other field.

`ChapterUpdate` (`domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt:3-17`): `id` plus optional mangaId, read, bookmark, lastPageRead, dateFetch, sourceOrder, url, name, dateUpload, chapterNumber, scanlator, version (12 optional fields); `Chapter.toChapterUpdate()` fills all 12 positionally. `NoChaptersException` (`domain/src/main/java/tachiyomi/domain/items/chapter/model/NoChaptersException.kt:3`): `class NoChaptersException : Exception()`.

### 2.2 Episode (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:6-66`)

Fields and `Episode.create()` defaults: id -1, animeId -1, seen false, bookmark false, fillermark false, lastSecondSeen 0, totalSeconds 0, dateFetch 0, sourceOrder 0, url "", name "", dateUpload -1, episodeNumber -1.0, scanlator null, summary null, previewUrl null, lastModifiedAt 0, version 1, memo `{}` (19 fields). `lastSecondSeen` and `totalSeconds` are playback position and duration in whole seconds (consumed by the player, the player series (no plans folder yet) series). `isRecognizedNumber = episodeNumber >= 0f`. `copyFrom(other)` copies name, url, dateUpload, episodeNumber, **fillermark**, and `ifBlank { null }` of scanlator, summary, previewUrl (the chapter version has no fillermark/summary/previewUrl).

`EpisodeUpdate` (`domain/src/main/java/tachiyomi/domain/items/episode/model/EpisodeUpdate.kt:5-24`): `id` plus optional animeId, seen, bookmark, fillermark, lastSecondSeen, totalSeconds, dateFetch, sourceOrder, url, name, dateUpload, episodeNumber, scanlator, summary, previewUrl, version, memo (17 optional fields); `Episode.toEpisodeUpdate()` fills all 17. `NoEpisodesException` (`domain/src/main/java/tachiyomi/domain/items/episode/model/NoEpisodesException.kt:3`).

## 3. Bit-flag layouts

`setFlag(v, flag, mask) = (v & !mask) | (flag & mask)`. The same one-liner is copy-pasted privately in every Set*Flags class, in `SetAnimeViewerFlags` and as the public `Long.plus(Flag)` in `domain/src/main/java/tachiyomi/domain/library/model/Flag.kt:21-27` (area:08-domain-library-category-history-updates). A filter field is a 2-bit group where `00` = no filter, the lower bit = "is" and the higher bit = "not"; the value `11` is representable but matches neither constant, so every `when` falls to `else -> DISABLED`.

### 3.1 Manga `chapterFlags` (`Manga.kt:75-103`)

| Group | Mask | Values |
|---|---|---|
| Sort direction | `CHAPTER_SORT_DIR_MASK = 0x00000001` | `CHAPTER_SORT_DESC = 0x00000000`, `CHAPTER_SORT_ASC = 0x00000001` |
| Unread filter | `CHAPTER_UNREAD_MASK = 0x00000006` | `CHAPTER_SHOW_UNREAD = 0x00000002`, `CHAPTER_SHOW_READ = 0x00000004` |
| Downloaded filter | `CHAPTER_DOWNLOADED_MASK = 0x00000018` | `CHAPTER_SHOW_DOWNLOADED = 0x00000008`, `CHAPTER_SHOW_NOT_DOWNLOADED = 0x00000010` |
| Bookmarked filter | `CHAPTER_BOOKMARKED_MASK = 0x00000060` | `CHAPTER_SHOW_BOOKMARKED = 0x00000020`, `CHAPTER_SHOW_NOT_BOOKMARKED = 0x00000040` |
| Sorting | `CHAPTER_SORTING_MASK = 0x00000300` | `CHAPTER_SORTING_SOURCE = 0x00000000`, `CHAPTER_SORTING_NUMBER = 0x00000100`, `CHAPTER_SORTING_UPLOAD_DATE = 0x00000200`, `CHAPTER_SORTING_ALPHABET = 0x00000300` |
| Display | `CHAPTER_DISPLAY_MASK = 0x00100000` | `CHAPTER_DISPLAY_NAME = 0x00000000`, `CHAPTER_DISPLAY_NUMBER = 0x00100000` |
| Generic | | `SHOW_ALL = 0x00000000` |

Bits 10-19 and 21-63 are unused.

TriState mapping (`Manga.kt:57-69`): unread: `SHOW_UNREAD` -> `ENABLED_IS`, `SHOW_READ` -> `ENABLED_NOT`; bookmarked: `SHOW_BOOKMARKED` -> `ENABLED_IS`, `SHOW_NOT_BOOKMARKED` -> `ENABLED_NOT`; anything else `DISABLED`. The downloaded filter has no domain property; the app layer adds `downloadedFilter` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt`, area:10-app-domain-layer) which returns `ENABLED_IS` when the global "downloaded only" mode is on, otherwise maps `SHOW_DOWNLOADED`/`SHOW_NOT_DOWNLOADED`. `sortDescending() = chapterFlags & DIR_MASK == CHAPTER_SORT_DESC` (the Kotlin precedence is `(flags and mask) == DESC`, written without parentheses because `and` is an infix function with higher precedence than `==`).

### 3.2 Anime `episodeFlags` (`Anime.kt:216-255`)

| Group | Mask | Values |
|---|---|---|
| Sort direction | `EPISODE_SORT_DIR_MASK = 0x00000001` | `EPISODE_SORT_DESC = 0x0`, `EPISODE_SORT_ASC = 0x1` |
| Unseen filter | `EPISODE_UNSEEN_MASK = 0x00000006` | `EPISODE_SHOW_UNSEEN = 0x2`, `EPISODE_SHOW_SEEN = 0x4` |
| Downloaded filter | `EPISODE_DOWNLOADED_MASK = 0x00000018` | `EPISODE_SHOW_DOWNLOADED = 0x8`, `EPISODE_SHOW_NOT_DOWNLOADED = 0x10` |
| Bookmarked filter | `EPISODE_BOOKMARKED_MASK = 0x00000060` | `EPISODE_SHOW_BOOKMARKED = 0x20`, `EPISODE_SHOW_NOT_BOOKMARKED = 0x40` |
| Fillermarked filter | `EPISODE_FILLERMARKED_MASK = 0x00000180` | `EPISODE_SHOW_FILLERMARKED = 0x80`, `EPISODE_SHOW_NOT_FILLERMARKED = 0x100` |
| Sorting | `EPISODE_SORTING_MASK = 0x00000600` | `EPISODE_SORTING_SOURCE = 0x0`, `EPISODE_SORTING_NUMBER = 0x200`, `EPISODE_SORTING_UPLOAD_DATE = 0x400`, `EPISODE_SORTING_ALPHABET = 0x600` |
| Thumbnail previews | `EPISODE_PREVIEWS_MASK = 0x00000800` | `EPISODE_SHOW_PREVIEWS = 0x0`, `EPISODE_SHOW_NOT_PREVIEWS = 0x800` |
| Summaries | `EPISODE_SUMMARIES_MASK = 0x00001000` | `EPISODE_SHOW_SUMMARIES = 0x0`, `EPISODE_SHOW_NOT_SUMMARIES = 0x1000` |
| Display | `EPISODE_DISPLAY_MASK = 0x00100000` | `EPISODE_DISPLAY_NAME = 0x0`, `EPISODE_DISPLAY_NUMBER = 0x100000` |

Different from manga: the sorting group moved up one bit (0x600 vs 0x300) to make room for the fillermark filter, and previews/summaries are additional single-bit groups. Importantly the previews/summaries constants are "show" = 0 / "not" = 1, so a zeroed flags word shows both. Raw getters: `sorting`, `displayMode`, `unseenFilterRaw`, `downloadedFilterRaw`, `bookmarkedFilterRaw`, `fillermarkedFilterRaw`, `showPreviewsRaw`, `showSummariesRaw`; booleans `showPreviews() = (flags & PREVIEWS_MASK) == SHOW_PREVIEWS`, `showSummaries() = (flags & SUMMARIES_MASK) == SHOW_SUMMARIES`. TriState: unseen `SHOW_UNSEEN` -> IS, `SHOW_SEEN` -> NOT; bookmarked IS/NOT as manga; fillermarked `SHOW_FILLERMARKED` -> IS, `SHOW_NOT_FILLERMARKED` -> NOT.

### 3.3 Anime `seasonFlags` (`Anime.kt:257-308`)

Stored on the PARENT anime (the entry whose `fetchType == Seasons`); it controls how that parent's season list is displayed.

| Group | Bits | Mask | Values |
|---|---|---|---|
| Sort direction | 0 | `SEASON_SORT_DIR_MASK = 0x00000001` | `SEASON_SORT_DESC = 0x0`, `SEASON_SORT_ASC = 0x1` |
| Downloaded filter | 1-2 | `SEASON_DOWNLOADED_MASK = 0x00000006` | `SEASON_SHOW_DOWNLOADED = 0x2`, `SEASON_SHOW_NOT_DOWNLOADED = 0x4` |
| Unseen filter | 3-4 | `SEASON_UNSEEN_MASK = 0x00000018` | `SEASON_SHOW_UNSEEN = 0x8`, `SEASON_SHOW_SEEN = 0x10` |
| Started filter | 5-6 | `SEASON_STARTED_MASK = 0x00000060` | `SEASON_SHOW_STARTED = 0x20`, `SEASON_SHOW_NOT_STARTED = 0x40` |
| Completed filter | 7-8 | `SEASON_COMPLETED_MASK = 0x00000180` | `SEASON_SHOW_COMPLETED = 0x80`, `SEASON_SHOW_NOT_COMPLETED = 0x100` |
| Bookmarked filter | 9-10 | `SEASON_BOOKMARKED_MASK = 0x00000600` | `SEASON_SHOW_BOOKMARKED = 0x200`, `SEASON_SHOW_NOT_BOOKMARKED = 0x400` |
| Fillermarked filter | 11-12 | `SEASON_FILLERMARKED_MASK = 0x00001800` | `SEASON_SHOW_FILLERMARKED = 0x800`, `SEASON_SHOW_NOT_FILLERMARKED = 0x1000` |
| Sort type | 13-15 | `SEASON_SORT_MASK = 0x0000E000` | `SEASON_SORT_SOURCE = 0x0`, `SEASON_SORT_SEASON = 0x2000`, `SEASON_SORT_UPLOAD = 0x4000`, `SEASON_SORT_ALPHABET = 0x6000`, `SEASON_SORT_COUNT = 0x8000`, `SEASON_SORT_LAST_SEEN = 0xA000`, `SEASON_SORT_FETCHED = 0xC000` (0xE000 is unassigned) |
| Grid display mode | 16-17 | `SEASON_GRID_DISPLAY_MODE_MASK = 0x00030000`, `..._BIT_OFFSET = 16` | value = `SeasonDisplayMode.toLong(mode) << 16` |
| Grid size | 18-21 | `SEASON_GRID_DISPLAY_SIZE_MASK = 0x003C0000`, `..._BIT_OFFSET = 18` | value = `size << 18`, size 0..15 (0 means "auto/default columns") |
| Overlay: downloaded badge | 22 | `SEASON_OVERLAY_DOWNLOADED_MASK = 0x00400000` | bit set = on |
| Overlay: unseen badge | 23 | `SEASON_OVERLAY_UNSEEN_MASK = 0x00800000` | |
| Overlay: local badge | 24 | `SEASON_OVERLAY_LOCAL_MASK = 0x01000000` | |
| Overlay: language badge | 25 | `SEASON_OVERLAY_LANG_MASK = 0x02000000` | |
| Overlay: continue button | 26 | `SEASON_OVERLAY_CONT_MASK = 0x04000000` | |
| Display (title mode) | 27 | `SEASON_DISPLAY_MODE_MASK = 0x08000000` | `SEASON_DISPLAY_MODE_SOURCE = 0x0`, `SEASON_DISPLAY_MODE_NUMBER = 0x08000000` |

`SEASON_SORT_DESC = 0` and the default sort mode is `SEASON_SORT_SOURCE`, so a zeroed word is "source order, descending, no filters, compact grid, size 0, no overlays, title display". The preference defaults for the overlay bits are NOT all zero (section 4), so `setAllFlags` called with default preferences yields a non-zero word.

Getters on `Anime` (lines 124-208): `seasonDownloadedFilterRaw`, `seasonUnseenFilterRaw`, `seasonStartedFilterRaw`, `seasonCompletedFilterRaw`, `seasonBookmarkedFilterRaw`, `seasonFillermarkedFilterRaw`, the matching TriState `seasonUnseenFilter` (IS = `SHOW_UNSEEN`, NOT = `SHOW_SEEN`), `seasonStartedFilter` (IS = `SHOW_STARTED`, NOT = `SHOW_NOT_STARTED`), `seasonCompletedFilter` (IS = `SHOW_COMPLETED`, NOT = `SHOW_NOT_COMPLETED`), `seasonBookmarkedFilter`, `seasonFillermarkedFilter` (IS/NOT with SHOW/SHOW_NOT); `seasonSorting = flags & 0xE000`; `seasonSortDescending() = (flags & 0x1) == 0`; `seasonDisplayGridMode = SeasonDisplayMode.fromLong((flags & 0x30000) >> 16)`; `seasonDisplayGridSize = ((flags & 0x3C0000) >> 18).toInt()`; boolean overlays `(flags & MASK) == MASK`; `seasonDisplayMode = flags & 0x08000000`. The app layer adds `seasonDownloadedFilter` with the same downloaded-only override (`app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:190-198`).

### 3.4 Anime `viewerFlags` (`Anime.kt:310-313`, setters in `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt`)

| Field | Bits | Mask | Encoding |
|---|---|---|---|
| Skip-intro length (seconds) | 0-7 | `ANIME_INTRO_MASK = 0x0000000000000FF` | plain value 0..255 |
| Next episode to air | 8-23 | `ANIME_AIRING_EPISODE_MASK = 0x000000000FFFF00` | `episode * 16^2` (shift left 8) |
| Next episode airing time | 24-55 | `ANIME_AIRING_TIME_MASK = 0x0FFFFFFFF000000` | `airingAtSeconds * 16^6` (shift left 24), 32-bit seconds |
| Skip-intro disabled | 56 | `ANIME_INTRO_DISABLE_MASK = 0x100000000000000` | bit set = disabled |

Getters: `skipIntroLength = (flags & INTRO_MASK).toInt()`; `skipIntroDisable = (flags & DISABLE_MASK) == DISABLE_MASK`; `nextEpisodeToAir = ((flags & AIRING_EPISODE_MASK) / 16^2).toInt()`; `nextEpisodeAiringAt = (flags & AIRING_TIME_MASK) / 16^6` (the division is done in `Double`: `private fun Long.removeHexZeros(zeros)` = `this.div(16.0.pow(zeros)).toLong()`, which is exact here because the masked values have at most 32 significant bits). Setters (`SetAnimeViewerFlags`): `awaitSetSkipIntroLength(id, flag)` sets the intro bits and sets the disable bit to `(flag == 0)` (times `16^14` = `1 << 56`), so length 0 disables the button and any other length re-enables it; `awaitSetNextEpisodeAiring(id, Pair(episode: Int, airingAt: Long))` writes the two airing groups as two separate read-modify-write updates (episode first, then time), each re-reading the anime. The airing data comes from AniList / Simkl (`app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt`, area:10-app-domain-layer and the tracker series (no plans folder yet)) and is displayed and sorted (`AnimeLibrarySort.Type.AiringTime`, area:08-domain-library-category-history-updates). Manga `viewerFlags` has no domain accessors (reader code uses `ReadingMode`/`ReaderOrientation`, the reader series (no plans folder yet) reader series).

## 4. Preference keys used by this area

All in `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` (`Preference` type is `Long` unless noted; registry owned by area:02-preferences-system; the keys are read here by the SetDefault*Flags interactors).

| Function (line) | Key | Default | Used for |
|---|---|---|---|
| `filterEpisodeBySeen` (173) | `default_episode_filter_by_seen` | `Anime.SHOW_ALL` = 0 | default unseen filter |
| `filterChapterByRead` (176) | `default_chapter_filter_by_read` | `Manga.SHOW_ALL` = 0 | default unread filter |
| `filterEpisodeByDownloaded` (179) | `default_episode_filter_by_downloaded` | 0 | |
| `filterChapterByDownloaded` (182) | `default_chapter_filter_by_downloaded` | 0 | |
| `filterEpisodeByBookmarked` (185) | `default_episode_filter_by_bookmarked` | 0 | |
| `filterChapterByBookmarked` (188) | `default_chapter_filter_by_bookmarked` | 0 | |
| `filterEpisodeByFillermarked` (191) | `default_episode_filter_by_fillermarked` | 0 | |
| `sortEpisodeBySourceOrNumber` (195) | `default_episode_sort_by_source_or_number` | `EPISODE_SORTING_SOURCE` = 0 | default sort type |
| `sortChapterBySourceOrNumber` (200) | `default_chapter_sort_by_source_or_number` | `CHAPTER_SORTING_SOURCE` = 0 | |
| `displayEpisodeByNameOrNumber` (205) | `default_chapter_display_by_name_or_number` (SAME KEY as the chapter one) | `EPISODE_DISPLAY_NAME` = 0 | |
| `displayChapterByNameOrNumber` (210) | `default_chapter_display_by_name_or_number` | `CHAPTER_DISPLAY_NAME` = 0 | |
| `sortEpisodeByAscendingOrDescending` (215) | `default_chapter_sort_by_ascending_or_descending` (SAME KEY as the chapter one) | `EPISODE_SORT_DESC` = 0 | |
| `sortChapterByAscendingOrDescending` (220) | `default_chapter_sort_by_ascending_or_descending` | `CHAPTER_SORT_DESC` = 0 | |
| `showEpisodeThumbnailPreviews` (225) | `default_episode_show_thumbnail_previews` | `EPISODE_SHOW_PREVIEWS` = 0 | |
| `showEpisodeSummaries` (230) | `default_episode_show_summaries` | `EPISODE_SHOW_SUMMARIES` = 0 | |
| `filterSeasonByDownload` (262) | `default_season_filter_by_downloaded` | 0 | |
| `filterSeasonByUnseen` (265) | `default_season_filter_by_unseen` | 0 | |
| `filterSeasonByStarted` (268) | `default_season_filter_by_started` | 0 | |
| `filterSeasonByCompleted` (271) | `default_season_filter_by_completed` | 0 | |
| `filterSeasonByBookmarked` (274) | `default_season_filter_by_bookmarked` | 0 | |
| `filterSeasonByFillermarked` (277) | `default_season_filter_by_fillermarked` | 0 | |
| `sortSeasonBySourceOrNumber` (280) | `default_season_sort_by_source_or_number` | `SEASON_SORT_SOURCE` = 0 | |
| `sortSeasonByAscendingOrDescending` (285) | `default_season_sort_by_ascending_or_descending` | `SEASON_SORT_DESC` = 0 | |
| `seasonDisplayGridMode` (290) | `default_season_grid_display_mode` | `SeasonDisplayMode.toLong(CompactGrid)` = 0 | |
| `seasonDisplayGridSize` (295) | `default_season_grid_display_size` (Int) | 0 | |
| `seasonDownloadOverlay` (300) | `default_season_download_overlay` (Boolean) | false | |
| `seasonUnseenOverlay` (305) | `default_season_unseen_overlay` (Boolean) | true | |
| `seasonLocalOverlay` (310) | `default_season_local_overlay` (Boolean) | true | |
| `seasonLangOverlay` (315) | `default_season_lang_overlay` (Boolean) | false | |
| `seasonContinueOverlay` (320) | `default_season_continue_overlay` (Boolean) | true | |
| `seasonDisplayMode` (325) | `default_season_display_mode` | `SEASON_DISPLAY_MODE_SOURCE` = 0 | |
| `updateSeasonOnRefresh` (353) | `pref_update_season_on_refresh` (Boolean) | false | read by the sync flow (area:10-app-domain-layer), listed because it governs season fetching |
| `updateSeasonOnLibraryUpdate` (356) | `pref_update_season_on_library_update` (Boolean) | false | same |
| `markDuplicateReadChapterAsRead` (377) | `mark_duplicate_read_chapter_read` (StringSet) | empty; values `"new"` (`MARK_DUPLICATE_CHAPTER_READ_NEW`), `"existing"` | read by `SyncChaptersWithSource` (area:10-app-domain-layer) |
| `markDuplicateSeenEpisodeAsSeen` (379) | `mark_duplicate_seen_episode_seen` (StringSet) | empty; values `"new_episode"`, `"existing_episode"` | read by `SyncEpisodesWithSource` |
| (GetApplicationRelease) | `__APP_STATE_last_app_check` = `Preference.appStateKey("last_app_check")` (prefix `__APP_STATE_`, `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`) | 0 (Long, epoch ms) | 3-day throttle of the update check |

The "set as default" writers (`setEpisodeSettingsDefault` line 235, `setChapterSettingsDefault` 249, `setSeasonSettingsDefault` 330) persist the current entry's raw groups into these keys. Because the episode and chapter functions share two keys, "make default" on an anime overwrites the manga defaults for display mode and sort direction and vice versa (OBSERVED bug; handling in p1-topic-quirks-and-decisions.md Q3).

Proposal: store these keys in the same `settings` store as area:02-preferences-system with identical names so that a copied Aniyomi preference file or backup (area:02-preferences-system p1-topic-backup-interop.md) round-trips; for the two shared keys keep sharing them (parity, and backups contain exactly these key names) but document it; decision record in p1-topic-quirks-and-decisions.md.

## 5. Constants

| Constant | Value | Source |
|---|---|---|
| `MangaFetchInterval.MAX_INTERVAL` / `AnimeFetchInterval.MAX_INTERVAL` | 28 | `MangaFetchInterval.kt:126`, `AnimeFetchInterval.kt:126` |
| `GRACE_PERIOD` | 1 day | `...FetchInterval.kt:128` |
| `increaseWhenOver` | 10 | `...FetchInterval.kt:108` |
| default interval | 7 days | `...FetchInterval.kt:83` |
| minimum computed interval | 1 | `coerceIn(1, MAX_INTERVAL)` line 86 |
| episode/chapter window | 3 when `size <= 8`, else 10 | line 44 |
| minimum distinct dates for a computed interval | 3 | lines 71, 77 |
| `GetLibraryManga/Anime` retry delay | 0.5 s on `NullPointerException` | `GetLibraryManga.kt:23-29` |
| release check throttle | 3 days | `GetApplicationRelease.kt:84` |
| recognition alpha suffix | `a`..`i` -> 0.1..0.9, `j` and beyond -> 0.0 | `ChapterRecognition.kt:122-126` |
| recognition specials | extra 0.99, omake 0.98, special 0.97 | `ChapterRecognition.kt:98-108` |
| `Manga/Anime.SHOW_ALL` | 0 | |
| missing-items sentinel | numbers `!= -1.0` participate (note -2.0 and other negatives are NOT filtered) | `MissingChapters.kt:13` |

## 6. Regular expressions (verbatim)

`NUMBER_PATTERN = ([0-9]+)(\.[0-9]+)?(\.?[a-z]+)?` (all three recognisers).

Chapter (`ChapterRecognition.kt:14-31`): `basic = (?<=ch\.) *<NUMBER_PATTERN>`; `number = <NUMBER_PATTERN>`; `unwanted = \b(?:v|ver|vol|version|volume|season|s)[^a-z]?[0-9]+`; `unwantedWhiteSpace = \s(?=extra|special|omake)`.

Episode (`EpisodeRecognition.kt:14-38`): `basic = (?<=\be\.|\be|episode|\bep) *<NUMBER_PATTERN>`; `tagRegex = ^\[[^\]]+\]|\[[^\]]+\]\s*$|^\([^\)]+\)|\([^\)]+\)\s*$` (the Kotlin source writes the dollar as `${'$'}` inside a raw string); `unwanted = \b(?:v|ver|version|season|s)[^a-z]?[0-9]+|\b\d+p\b|hi10`; `unwantedWhiteSpace` as chapter.

Season (`SeasonRecognition.kt:14-37`): `basic = (?<=\bs\.|\bs|season) *<NUMBER_PATTERN>`; `tagRegex` as episode; `unwanted = \b\d+p\b|\d+x\d+|Hi10|\(\d+\)` (capital `H`; the input is lower-cased first so this alternative can never match, OBSERVED defect Q5); `unwantedWhiteSpace` as chapter.

Release (`ReleaseServiceImpl.kt`, area:06-data-repos-history-track-source-misc; `GetApplicationRelease.kt:114,122`): version cleanup `[^\d.]`.

## 7. Files and folders on disk

This area performs no file I/O. It computes cache-buster values (`coverLastModified`, `backgroundLastModified`) that name nothing on disk by themselves; the cover and background caches are in area:13-core-util-storage-archive-torrent and area:10-app-domain-layer.

## 8. Network endpoints

None used by the entry/item domain. The only network access in the scope files is the release check: `GET https://api.github.com/repos/{repository}/releases/latest` (`data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:21`), no auth header, response `tag_name`, `body`, `html_url`, `assets[].name`, `assets[].browser_download_url` (`GithubRelease.kt`). `repository` is `aniyomiorg/aniyomi` (release builds) or `aniyomiorg/aniyomi-preview` (preview builds) (`app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt:44-50`). Details and the Kurobayomi decision are in p1-topic-release-check.md.

## 9. DB tables touched (through repositories only)

`mangas`, `chapters`, `mangas_categories`, `animes`, `episodes`, `animes_categories`, `anime_relations` (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:05-data-repos-entries-items). Columns that carry the flag words: `mangas.chapter_flags`, `mangas.viewer`, `animes.episode_flags`, `animes.viewer`, `animes.season_flags`, `animes.parent_id`. The reset of viewer flags is `UPDATE mangas SET viewer = 0` / `UPDATE animes SET viewer = 0` (area:05-data-repos-entries-items).
