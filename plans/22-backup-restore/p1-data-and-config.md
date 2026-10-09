# 22-backup-restore — Part 1 data models, keys, files, endpoints

Wire models are owned by area:21-backup-format-and-create. They are listed here because the restore path reads every field. All paths are repo-relative to `/home/blackbox/aniyomi-src`.

---

## 1. Backup container (protobuf, kotlinx `ProtoBuf`)

Source: `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt`.

### 1.1 `Backup` (new format, Mihon fields + Aniyomi 5xx)

| Field | Proto # | Kotlin type | Default | Restore use |
|---|---|---|---|---|
| backupManga | 1 | `List<BackupManga>` | empty | manga entries |
| backupCategories | 2 | `List<BackupCategory>` (var) | empty | manga categories |
| backupSources | 101 | `List<BackupSource>` (var) | empty | validation (missing manga sources) and error labels |
| backupPreferences | 104 | `List<BackupPreference>` (var) | empty | app preferences |
| backupSourcePreferences | 105 | `List<BackupSourcePreferences>` (var) | empty | per-source preferences |
| backupMangaExtensionRepo | 106 | `List<BackupExtensionRepos>` (var) | empty | manga extension repos |
| isLegacy | 500 | `Boolean` (val) | `true` | legacy detection (read by `BackupDetector` as well) |
| backupAnime | 501 | `List<BackupAnime>` | empty | anime entries (incl. seasons) |
| backupAnimeCategories | 502 | `List<BackupCategory>` (var) | empty | anime categories |
| backupAnimeSources | 503 | `List<BackupAnimeSource>` (var) | empty | validation (missing anime sources) and error labels |
| backupExtensions | 504 | `List<BackupExtension>` (var) | empty | APK extensions (Android) |
| backupAnimeExtensionStores | 505 | `List<BackupExtensionStore>` (var) | empty | anime extension stores |
| backupCustomButton | 506 | `List<BackupCustomButtons>` (var) | empty | custom buttons |

### 1.2 `LegacyBackup` (Aniyomi 0.x files; converted with `toBackup()`)

| Field | Proto # | Maps to |
|---|---|---|
| backupManga | 1 | `Backup.backupManga` |
| backupCategories | 2 | `Backup.backupCategories` |
| backupAnime | 3 | `Backup.backupAnime` (note: 3 here, 501 in the new format) |
| backupAnimeCategories | 4 | `Backup.backupAnimeCategories` (note: 4 here, 502 new) |
| backupSources | 101 | `Backup.backupSources` |
| backupAnimeSources | 103 | `Backup.backupAnimeSources` (note: 103 here, 503 new) |
| backupPreferences | 104 | `Backup.backupPreferences` |
| backupSourcePreferences | 105 | `Backup.backupSourcePreferences` |
| backupExtensions | 106 | `Backup.backupExtensions` (`toBackup()` passes the list through unchanged) |
| backupAnimeExtensionStores | 107 | `Backup.backupAnimeExtensionStores` |
| backupMangaExtensionRepo | 108 | `Backup.backupMangaExtensionRepo` |
| backupCustomButton | 109 | `Backup.backupCustomButton` |

`toBackup()` sets `isLegacy = false` ("Only used for detection"). Numbers 100 and 102 are reserved (commented out in the source: legacy source model with non-compliant proto number). Source: `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt:6-42`.

### 1.3 Detector (`BackupDetector.kt:15-25`)
`BackupDetector.BackupDetector(backupAnimeSources: List<DetectAnimeSource> @103, isLegacy: Boolean @500 default true)`. `DetectAnimeSource(name: String @1 default "", sourceId: Long @2)`. Decoding ignores every other field.

---

## 2. Item and entry models

### 2.1 `BackupManga` (manga entry)

| Field | Proto # | Kotlin type | Default |
|---|---|---|---|
| source | 1 | `Long` (var) | required (no default) |
| url | 2 | `String` (var) | required |
| title | 3 | `String` | `""` |
| artist | 4 | `String?` | `null` |
| author | 5 | `String?` | `null` |
| description | 6 | `String?` | `null` |
| genre | 7 | `List<String>` | empty |
| status | 8 | `Int` | `0` |
| thumbnailUrl | 9 | `String?` | `null` |
| dateAdded | 13 | `Long` | `0` |
| viewer | 14 | `Int` | `0` (replaced by viewer_flags) |
| chapters | 16 | `List<BackupChapter>` | empty |
| categories | 17 | `List<Long>` | empty (values are backup category `order`s) |
| tracking | 18 | `List<BackupTracking>` | empty |
| favorite | 100 | `Boolean` | **`true`** (D22: absent means favourite) |
| chapterFlags | 101 | `Int` | `0` |
| viewer_flags | 103 | `Int?` | `null` |
| history | 104 | `List<BackupHistory>` | empty |
| updateStrategy | 105 | `UpdateStrategy` (enum) | `ALWAYS_UPDATE` |
| lastModifiedAt | 106 | `Long` | `0` |
| favoriteModifiedAt | 107 | `Long?` | `null` |
| excludedScanlators | 108 | `List<String>` | empty |
| version | 109 | `Long` | `0` |

Numbers 102 and 15 are not used (1.x fields not kept). `getMangaImpl()` maps to `Manga` as in §4.1.

### 2.2 `BackupChapter`

| Field | Proto # | Type | Default |
|---|---|---|---|
| url | 1 | String | required |
| name | 2 | String | required |
| scanlator | 3 | String? | null |
| read | 4 | Boolean | false |
| bookmark | 5 | Boolean | false |
| lastPageRead | 6 | Long | 0 |
| dateFetch | 7 | Long | 0 |
| dateUpload | 8 | Long | 0 |
| chapterNumber | 9 | Float | 0F |
| sourceOrder | 10 | Long | 0 |
| lastModifiedAt | 11 | Long | 0 |
| version | 12 | Long | 0 |

`toChapterImpl()` converts `chapterNumber` with `toDouble()` (Float to Double widening: 1.1f becomes 1.100000023841858; the restored value keeps the float artefact. Edge E-27).

### 2.3 `BackupEpisode`

| Field | Proto # | Type | Default |
|---|---|---|---|
| url | 1 | String | required |
| name | 2 | String | required |
| scanlator | 3 | String? | null |
| seen | 4 | Boolean | false |
| bookmark | 5 | Boolean | false |
| lastSecondSeen | 6 | Long | 0 |
| totalSeconds | 16 | Long | 0 |
| dateFetch | 7 | Long | 0 |
| dateUpload | 8 | Long | 0 |
| episodeNumber | 9 | Float | 0F |
| sourceOrder | 10 | Long | 0 |
| lastModifiedAt | 11 | Long | 0 |
| version | 12 | Long | 0 |
| memo | 13 | ByteArray | `JsonObjectEmptyBytes` = `{}` (bytes `0x7B 0x7D`) |
| fillermark | 501 | Boolean | false |
| summary | 502 | String? | null |
| previewUrl | 503 | String? | null |

`toEpisodeImpl()` uses `episodeNumber.toDouble()` and `MemoColumnAdapter.decode(memo)`.

### 2.4 `BackupAnime` (anime entry)

| Field | Proto # | Type | Default |
|---|---|---|---|
| source | 1 | Long | required |
| url | 2 | String | required |
| title | 3 | String | `""` |
| artist | 4 | String? | null |
| author | 5 | String? | null |
| description | 6 | String? | null |
| genre | 7 | List<String> | empty |
| status | 8 | Int | 0 |
| thumbnailUrl | 9 | String? | null |
| dateAdded | 13 | Long | 0 |
| episodes | 16 | List<BackupEpisode> | empty |
| categories | 17 | List<Long> | empty (anime category `order`s) |
| tracking | 18 | List<BackupAnimeTracking> | empty |
| favorite | 100 | Boolean | **true** |
| episodeFlags | 101 | Int | 0 |
| viewer_flags | 103 | Int | 0 (non-null here, nullable in manga) |
| history | 104 | List<BackupAnimeHistory> | empty |
| updateStrategy | 105 | AnimeUpdateStrategy (enum) | ALWAYS_UPDATE |
| lastModifiedAt | 106 | Long | 0 |
| favoriteModifiedAt | 107 | Long? | null |
| version | 109 | Long | 0 |
| memo | 112 | ByteArray | `{}` |
| backgroundUrl | 500 | String? | null |
| parentId | 502 | Long? | null (season's parent backup id) |
| id | 503 | Long? | null (backup-side id; used only for season association; exported as `anime.id`, `AnimeBackupCreator.kt:98`) |
| seasonFlags | 504 | Long | 0 |
| seasonNumber | 505 | Double | -1.0 |
| seasonSourceOrder | 506 | Long | 0 |
| fetchType | 507 | FetchType (enum) | `Episodes` |

Note: proto 501 is "Broken, do not use" (`BackupAnime.kt:175`). `getAnimeImpl()` maps as in §4.2.

### 2.5 `BackupAnimeHistory` and `BackupHistory`

Both: `url: String` @1 (required), `lastRead: Long` @2 (required), `readDuration: Long` @3 default 0. Only `BackupHistory` (manga) uses `readDuration`; `BackupAnimeHistory.getHistoryImpl()` sets only `seenAt = Date(lastRead)`.

### 2.6 `BackupTracking` (manga tracker)

| Field | Proto # | Type | Default |
|---|---|---|---|
| syncId | 1 | Int | required (tracker id) |
| libraryId | 2 | Long | required (1.x non-null) |
| mediaIdInt | 3 | Int | 0 (deprecated; used only if non-zero) |
| trackingUrl | 4 | String | `""` |
| title | 5 | String | `""` |
| lastChapterRead | 6 | Float | 0F |
| totalChapters | 7 | Int | 0 |
| score | 8 | Float | 0F |
| status | 9 | Int | 0 |
| startedReadingDate | 10 | Long | 0 |
| finishedReadingDate | 11 | Long | 0 |
| private | 12 | Boolean | false |
| mediaId | 100 | Long | 0 |

`getTrackImpl()`: `MangaTrack(id=-1, mangaId=-1, trackerId=syncId.toLong(), remoteId = if (mediaIdInt != 0) mediaIdInt.toLong() else mediaId, libraryId, title, lastChapterRead = toDouble(), totalChapters = toLong(), score = toDouble(), status = toLong(), startDate = startedReadingDate, finishDate = finishedReadingDate, remoteUrl = trackingUrl, private)`.

### 2.7 `BackupAnimeTracking`

Same numbers as §2.6 with names `lastEpisodeSeen` (6, Float), `totalEpisodes` (7, Int), `startedWatchingDate` (10), `finishedWatchingDate` (11). `getTrackImpl()` produces `AnimeTrack(id=-1, animeId=-1, trackerId, remoteId, libraryId, title, lastEpisodeSeen = toDouble(), totalEpisodes = toLong(), score = toDouble(), status = toLong(), startDate, finishDate, remoteUrl, private)`.

### 2.8 `BackupCategory`

| Field | Proto # | Type | Default |
|---|---|---|---|
| name | 1 | String | required |
| order | 2 | Long | 0 |
| id | 3 | Long | 0 (source-device DB id; used for default_category mapping) |
| flags | 100 | Long | 0 (category display/sort flags, 0.x-compatible numbering) |

`toCategory(id)` = `Category(id, name, flags, order, hidden = false)`.

### 2.9 Extension related

- `BackupExtension`: `pkgName` @1 String (required), `apk` @2 ByteArray (required). Equality is by name and bytes.
- `BackupExtensionRepos` (manga repo): `baseUrl` @1, `name` @2, `shortName` @3 (String?), `website` @4, `signingKeyFingerprint` @5. Only `shortName` is nullable; all others are required without defaults (a missing one is a decode error).
- `BackupExtensionStore` (anime store): `indexUrl` @1, `name` @2, `badgeLabel` @3 (String?), `contactWebsite` @4 (String, required), `signingKey` @5 (String, required), `contactDiscord` @6 (String?), `isLegacy` @7 (Boolean?), `extensionListUrl` @8 (String?).

### 2.10 Custom buttons

`BackupCustomButtons`: `name` @1 String, `isFavorite` @2 Boolean, `sortIndex` @3 Long, `content` @4 String, `longPressContent` @5 String, `onStartup` @6 String. All required (no defaults).

### 2.11 Preferences

- `BackupPreference(key @1 String, value @2 PreferenceValue)`.
- `BackupSourcePreferences(sourceKey @1 String, prefs @2 List<BackupPreference>)`.
- `sealed class PreferenceValue` with subclasses:
  - `IntPreferenceValue(value: Int)`
  - `LongPreferenceValue(value: Long)`
  - `FloatPreferenceValue(value: Float)`
  - `StringPreferenceValue(value: String)`
  - `BooleanPreferenceValue(value: Boolean)`
  - `StringSetPreferenceValue(value: Set<String>)`
  (`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt:6-37`). The polymorphic wire layout for `value` is kotlinx's sealed-class encoding; the exact bytes must be taken from a real backup fixture (T-22a-051).
- A second definition exists under `app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/BackupPreference.kt`; the restore files import only `...backup.models.*`.

### 2.12 Sources

- `BackupSource(name @1 String default "", sourceId @2 Long required)` (manga). `BrokenBackupSource(name @0, sourceId @1)` is a migration helper with `toBackupSource()`.
- `BackupAnimeSource(name @1 default "", sourceId @2 required)` (anime). `BrokenBackupAnimeSource` is the same helper pattern.

### 2.13 Enums (encoded as ordinal integers by kotlinx `ProtoBuf`; verify with fixture T-22a-051)

| Enum | Constants (ordinal) | Source |
|---|---|---|
| `eu.kanade.tachiyomi.source.model.UpdateStrategy` | `ALWAYS_UPDATE` (0), `ONLY_FETCH_ONCE` (1) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt:15,22` |
| `eu.kanade.tachiyomi.animesource.model.AnimeUpdateStrategy` | `ALWAYS_UPDATE` (0), `ONLY_FETCH_ONCE` (1) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt:15,22` |
| `eu.kanade.tachiyomi.animesource.model.FetchType` | `Seasons` (0), `Episodes` (1) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:14,19` |

Database encodings used when writing: `MangaUpdateStrategyColumnAdapter.encode`, `AnimeUpdateStrategyColumnAdapter.encode`, `FetchTypeColumnAdapter.encode`, `MemoColumnAdapter.encode/decode` (owners: area:05/06/07). The memo is stored as the JSON text of the bytes.

---

## 3. Wire-format rules that the Rust decoder must reproduce

Derived from the Kotlin field types (kotlinx.serialization `ProtoBuf`, default config):

| Kotlin type | Wire type | Notes |
|---|---|---|
| `Int` | varint (int32) | not zigzag (no `@ProtoType(SINT)` in these models); negative values sign-extend to 10 bytes |
| `Long` | varint (int64) | |
| `Boolean` | varint | 0 or 1 |
| `Float` | fixed32 (IEEE 754) | used for chapter/episode numbers, track scores, lastChapterRead/lastEpisodeSeen |
| `Double` | fixed64 | `seasonNumber` |
| `String`, `ByteArray` | length-delimited (UTF-8 for String) | |
| enum | varint (ordinal) | §2.13 |
| `T?` | field absent when null | a missing field takes the Kotlin default, see tables above |
| `List<T>` (message) | repeated length-delimited | one field per element |
| `List<Long>`, `List<String>` | repeated scalar | not `@ProtoPacked`; the decoder must accept both packed and unpacked forms for primitives |
| `Set<String>` (in StringSet preference) | repeated string | |
| sealed `PreferenceValue` | nested message, polymorphic | verify layout with fixture |

Defaults with observable effect when absent on the wire (must be replicated):
- `BackupManga.favorite` absent means `true` (D22).
- `BackupAnime.seasonNumber` absent means `-1.0`.
- `BackupAnime.fetchType` absent means `Episodes`.
- `BackupManga.updateStrategy` / `BackupAnime.updateStrategy` absent means `ALWAYS_UPDATE`.
- `BackupDetector.isLegacy` absent means `true` (with non-empty field 103 the file is treated as legacy).
- `BackupEpisode.memo` and `BackupAnime.memo` absent means `{}`.
- Required fields without defaults (`BackupManga.source`, `BackupManga.url`, `BackupChapter.url/name`, `BackupExtensionRepos.*`, `BackupExtensionStore.*`, `BackupCustomButtons.*`, `BackupTracking.syncId/libraryId`, `BackupExtension.*`, `BackupPreference.key/value`) cause a decode failure when absent (`SerializationException`, mapped to "Backup file is corrupted").

---

## 4. Domain mappings used by restore

### 4.1 `BackupManga` -> `Manga` (`BackupManga.kt:106-126`)
`Manga.create().copy(url, title, artist, author, description, genre = genre (List<String>), status = status.toLong(), thumbnailUrl, favorite, source, dateAdded, viewerFlags = (viewer_flags ?: viewer).toLong(), chapterFlags = chapterFlags.toLong(), updateStrategy, lastModifiedAt, favoriteModifiedAt, version)`.

Fields not set by the backup keep `Manga.create()` defaults: `id = -1`, `source` set, `favorite` set, `lastUpdate = 0`, `nextUpdate = 0`, `fetchInterval = 0`, `coverLastModified = 0`, `initialized = false`, `dateAdded` set. (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:105-128`.)

Note: `genre` is a `List<String>` on the domain model; the DB stores it joined by `", "` (§4.3).

### 4.2 `BackupAnime` -> `Anime` (`BackupAnime.kt:183-210`)
`Anime.create().copy(url, title, artist, author, description, genre, status = toLong(), thumbnailUrl, backgroundUrl, favorite, source, dateAdded, viewerFlags = viewer_flags.toLong(), episodeFlags = episodeFlags.toLong(), updateStrategy, lastModifiedAt, favoriteModifiedAt, version, fetchType, parentId, seasonFlags, seasonNumber, seasonSourceOrder, memo = MemoColumnAdapter.decode(memo))`. Note: `id` is not set (it is the DB id).

### 4.3 `BackupChapter` -> `Chapter` (`BackupChapter.kt:26-41`)
`Chapter.create().copy(url, name, chapterNumber = chapterNumber.toDouble(), scanlator, read, bookmark, lastPageRead, dateFetch, dateUpload, sourceOrder, lastModifiedAt, version)`.

`Chapter.create()` defaults (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:32-47`): `id = -1`, `mangaId = -1`, `read = false`, `bookmark = false`, `lastPageRead = 0`, `dateFetch = 0`, `sourceOrder = 0`, `url = ""`, `name = ""`, `dateUpload = -1`, `chapterNumber = -1.0`, `scanlator = null`, `lastModifiedAt = 0`, `version = 1`.

### 4.4 `BackupEpisode` -> `Episode` (`BackupEpisode.kt:39-59`)
`Episode.create().copy(url, name, episodeNumber = toDouble(), scanlator, summary, previewUrl, seen, bookmark, fillermark, lastSecondSeen, totalSeconds, dateFetch, dateUpload, sourceOrder, lastModifiedAt, version, memo = MemoColumnAdapter.decode(memo))`.

`Episode.create()` defaults (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt`): `id = -1`, `animeId = -1`, `seen = false`, `bookmark = false`, `fillermark = false`, `lastSecondSeen = 0`, `totalSeconds = 0`, `dateFetch = 0`, `sourceOrder = 0`, `url = ""`, `name = ""`, `dateUpload = -1`, `episodeNumber = -1.0`, `scanlator = null`, `summary = null`, `previewUrl = null`.

### 4.5 `BackupHistory` -> `MangaHistory` (`BackupHistory.kt:14-19`)
`MangaHistory.create().copy(readAt = Date(lastRead), readDuration)`. `MangaHistory.create()` = `id = -1, chapterId = -1, readAt = null, readDuration = -1` (`domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistory.kt:12-17`).

### 4.6 `BackupAnimeHistory` -> `AnimeHistory` (`BackupAnimeHistory.kt:14-18`)
`AnimeHistory.create().copy(seenAt = Date(lastRead))`.

### 4.7 `BackupCategory` -> `Category` (`BackupCategory.kt:16-22`)
`Category(id, name, flags, order, hidden = false)`.

### 4.8 `BackupExtensionRepos` -> `ExtensionRepo` (inverse direction used only for backup; restore writes the row directly, §5.3).

### 4.9 `BackupExtensionStore` -> `AnimeExtensionStore` row (restore writes the row directly, §5.4).

### 4.10 `BackupCustomButtons` -> `CustomButton` (restore writes the row directly; the mapper in `BackupCustomButtons.kt:17-25` is the inverse).

---

## 5. Database tables and columns written by restore

All writes go through SQLDelight generated queries. Column names are the SQL column names in the `.sq` files. Parameter order below is the order of the named arguments at the call site.

### 5.1 `mangas` (`data/src/main/sqldelight/data/mangas.sq`)
- INSERT (`mangasQueries.insert`, call at `MangaRestorer.kt:243-264`): 20 named parameters: `source, url, artist, author, description, genre, title, status, thumbnailUrl, favorite, lastUpdate, nextUpdate (=0), calculateInterval (=0), initialized, viewerFlags, chapterFlags, coverLastModified, dateAdded, updateStrategy, version`. Then `selectLastInsertedRowId`.
- UPDATE (`mangas.sq:156-179`), every column is `coalesce(:param, column)`, `WHERE _id = :mangaId`: `source, url, artist, author, description, genre, title, status, thumbnail_url (thumbnailUrl), favorite, last_update (lastUpdate), next_update (nextUpdate), initialized, viewer (viewer), chapter_flags (chapterFlags), cover_last_modified (coverLastModified), date_added (dateAdded), update_strategy (updateStrategy), calculate_interval (calculateInterval), version, is_syncing (isSyncing)`. Restore passes `nextUpdate = null` and `calculateInterval = null` (keep), `isSyncing = 1`.
- Lookup: `mangasQueries.getMangaByUrlAndSource(url, source)` = `SELECT * FROM mangas WHERE url = :url AND source = :source LIMIT 1` (`mangas.sq:70-74`).
- Source-url list: `getAllMangaSourceAndUrl` = `SELECT source, url FROM mangas` (`mangas.sq:94-96`).

### 5.2 `chapters` (`data/src/main/sqldelight/data/chapters.sq`)
- INSERT (`MangaRestorer.kt:195-208`): 12 params `mangaId, url, name, scanlator, read, bookmark, lastPageRead, chapterNumber, sourceOrder, dateFetch, dateUpload, version`.
- UPDATE (`MangaRestorer.kt:216-231`): `mangaId=null, url=null, name=null, scanlator=null, read, bookmark, lastPageRead, chapterNumber=null, sourceOrder=null, dateFetch=null, dateUpload=null, chapterId, version, isSyncing=0` (`chapters.sq:103`).
- Lookups: `getChaptersByMangaId` (per manga), `getChapterByUrl` = `SELECT * FROM chapters WHERE url = :chapterUrl` (`chapters.sq:79-82`, global: D9).

### 5.3 `episodes` (`data/src/main/sqldelightanime/dataanime/episodes.sq`)
- INSERT (`AnimeRestorer.kt:220-238`): 17 params `animeId, url, name, scanlator, seen, bookmark, lastSecondSeen, totalSeconds, episodeNumber, sourceOrder, dateFetch, dateUpload, version, summary, previewUrl, fillermark, memo`.
- UPDATE (`AnimeRestorer.kt:246-266`, SQL `episodes.sq:98-106`): `animeId, url, name, scanlator, summary, previewUrl` null; `seen, bookmark, fillermark, lastSecondSeen, totalSeconds` values; `episodeNumber, sourceOrder, dateFetch, dateUpload` null; `episodeId, version, isSyncing = 0, memo`.
- Lookup: `getEpisodeByUrl` = `SELECT * FROM episodes WHERE url = :episodeUrl` (`episodes.sq:74-77`, global).

### 5.4 `animes` (`data/src/main/sqldelightanime/dataanime/animes.sq`)
- INSERT (`AnimeRestorer.kt:278-308`): named arguments `source, url, artist, author, description, genre, title, status, thumbnailUrl, favorite, lastUpdate, nextUpdate (=0), calculateInterval (=0), initialized, viewerFlags, episodeFlags, coverLastModified, dateAdded, updateStrategy, version, fetchType, parentId, seasonFlags, seasonNumber, seasonSourceOrder, backgroundUrl, backgroundLastModified, memo`, then `selectLastInsertedRowId`.
- UPDATE (`AnimeRestorer.kt:125-156`): as manga plus `episodeFlags` (named `episodeFlags`), `fetchType` (encoded), `parentId`, `seasonFlags`, `seasonNumber`, `seasonSourceOrder`, `backgroundUrl`, `backgroundLastModified`, `memo` (encoded). `isSyncing = 1`.
- Lookups: `getAnimeByUrlAndSource`, `getAllAnimeSourceAndUrl` = `SELECT source, url FROM animes` (`animes.sq:105-107`).

### 5.5 `history` (`data/src/main/sqldelight/data/history.sq`)
- UPSERT (`history.sq:61-69`): `INSERT INTO history(chapter_id, last_read, time_read) VALUES (:chapterId, :readAt, :time_read) ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read WHERE chapter_id = :chapterId`.
- Lookup: `getHistoryByChapterUrl` (`history.sq:25-31`) returns `H._id, H.chapter_id, H.last_read, H.time_read`.

### 5.6 `animehistory` (`data/src/main/sqldelightanime/dataanime/animehistory.sq`)
- UPSERT (`animehistory.sq:58-65`): `INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt) ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt WHERE episode_id = :episodeId`.
- Lookup: `getHistoryByEpisodeUrl` returns `H._id, H.episode_id, H.last_seen` joined to `episodes` on url.

### 5.7 `manga_sync` and `anime_sync` (tracks)
- Update (`manga_sync.sq:45`, call at `MangaRestorer.kt:394-409`): 14 params `mangaId, trackerId, remoteId, libraryId, title, lastChapterRead, totalChapters, status, score, remoteUrl, startDate, finishDate, private, trackId`.
- Anime: `anime_sync` update (`AnimeRestorer.kt:433-448`) same 14 params with `animeId`, `lastEpisodeSeen`, `totalEpisodes`.
- Insert: `InsertMangaTrack.awaitAll` / `InsertAnimeTrack.awaitAll` (owner: area:06 track data; columns not restated here).

### 5.8 Categories
- `categories` (manga): `categoriesQueries.insert(name, order, flags)` then `selectLastInsertedRowId()` (`MangaCategoriesRestorer.kt:225-228`).
- `categories` (anime): `categoriesQueries.insert(name, order, flags)` on the anime database (`AnimeCategoriesRestorer.kt:183-186`).
- `mangas_categories`: `DELETE` by `manga_id`, then `INSERT(manga_id, category_id)` (`mangas_categories.sq:18`, `MangaRestorer.kt:312-317`).
- `animes_categories`: `deleteAnimeCategoryByAnimeId(anime_id)` then `insert(anime_id, category_id)` (`AnimeRestorer.kt:352-358`).

### 5.9 `excluded_scanlators` (`data/src/main/sqldelight/data/excluded_scanlators.sq:10-12,19-22`)
- INSERT `(manga_id, scanlator)`; lookup `SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId`.

### 5.10 `extension_repos` (manga, `data/src/main/sqldelight/data/extension_repos.sq:27-45`)
- INSERT `(base_url, name, short_name, website, signing_key_fingerprint)`. The `upsert` and `replace` queries exist but restore uses `insert` (`MangaExtensionRepoRestorer.kt:127-135`).
- Lookup: `SELECT *` (`getAll`), and count query.

### 5.11 `extension_store` (anime, `data/src/main/sqldelightanime/dataanime/extension_store.sq:25-45`)
- UPSERT on `index_url` with the 8 columns `index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url`; `DO UPDATE SET name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url`.

### 5.12 `custom_buttons` (anime DB, `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`)
- INSERT with args `(name, isFavorite, sortIndex, content, longPressContent, onStartup)` then `selectLastInsertedRowId` (`CustomButtonRestorer.kt:55-63`).
- Lookup: `getCustomButtons.getAll()`.

### 5.13 Transaction boundary
One `handler.await(inTransaction = true)` per manga or anime entry (§6 of behavior). Category, preference, extension store, custom button and extension sections run without an enclosing transaction; each write is its own statement (the preferences use `PreferenceStore`).

---

## 6. Preference keys used by restore, with defaults

Owner of the preference classes: area:02-preferences-system. Keys below are the literal strings.

### 6.1 Category-keyed (remapped by name)
| Key | Owner | Type | Default | Note |
|---|---|---|---|---|
| `default_category` (`LibraryPreferences.DEFAULT_MANGA_CATEGORY_PREF_KEY`) | LibraryPreferences | Int | -1 | id remapped via backup category id -> name -> manga category id |
| `default_anime_category` (`DEFAULT_ANIME_CATEGORY_PREF_KEY`) | LibraryPreferences | Int | -1 | id remapped via anime categories |
| `library_update_categories` (manga) | LibraryPreferences | StringSet | (per owner) | category ids, remapped, appended (D12) |
| `animelib_update_categories` (anime) | LibraryPreferences | StringSet | (per owner) | remapped, appended |
| `library_update_categories_exclude` (manga) | LibraryPreferences | StringSet | (per owner) | remapped, appended |
| `animelib_update_categories_exclude` (anime) | LibraryPreferences | StringSet | (per owner) | remapped, appended |
| `remove_exclude_categories` | DownloadPreferences | StringSet | (per owner) | remapped, appended |
| `remove_exclude_anime_categories` | DownloadPreferences | StringSet | (per owner) | remapped, appended |
| `download_new_categories` | DownloadPreferences | StringSet | (per owner) | remapped, appended |
| `download_new_anime_categories` | DownloadPreferences | StringSet | (per owner) | remapped, appended |
| `download_new_categories_exclude` | DownloadPreferences | StringSet | (per owner) | remapped, appended |
| `download_new_anime_categories_exclude` | DownloadPreferences | StringSet | (per owner) | remapped, appended |

Sources: `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:413-424` (`categoryPreferenceKeys` = the first six keys), `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:76-88` (`categoryPreferenceKeys` = the last six). Note: the library-update keys are defined as `private const val` with the literals above (`LibraryPreferences.kt:~400-412`).

### 6.2 Other keys referenced by this area
| Key | Owner | Type | Default | Used by |
|---|---|---|---|---|
| `categorized_display` | LibraryPreferences (`LibraryPreferences.kt:70`) | Boolean | false | categories restorer sets it |
| `backup_interval` | BackupPreferences (`BackupPreferences.kt:10`) | Int (hours) | 12 | `BackupCreateJob.setupTask` after app-prefs restore |
| `__APP_STATE_last_auto_backup_timestamp` | BackupPreferences (`Preference.appStateKey("last_auto_backup_timestamp")`, `BackupPreferences.kt:12`) | Long | 0 | written by auto backup; restored only if present in a backup (type gate) |
| `backup_flags` | BackupPreferences (`BackupPreferences.kt:14-17`) | StringSet | {categories, chapters, history, track} | create side (area:21); restored if present |
| library auto-update interval | LibraryPreferences (`autoUpdateInterval()`) | Int (hours) | owner area:08/02 | re-arm after restore |
| library auto-update device restrictions | LibraryPreferences (`autoUpdateDeviceRestrictions()`) | StringSet | owner | re-arm constraints |
| hide notification content | SecurityPreferences `hideNotificationContent()` | Boolean | owner area:02 | progress notification text |
| source preference file | `"source_" + sourceId` (`MangaSourceExtensions.kt:14`, `AnimeSourceExtensions.kt:14`) | per-source | per source | `restoreSource` target |

`Preference.appStateKey(key)` prefixes with `__APP_STATE_` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`).

### 6.3 Restore-written preference values
- `default_category`, `default_anime_category`: Int (remapped id or unchanged).
- Category string sets: appended ids (§5.3 of behaviour).
- `categorized_display`: Boolean.
- Any other key from the file: type-gated write.

---

## 7. Files and folders on disk

| Path | Created by | Lifetime | Notes |
|---|---|---|---|
| `externalCacheDir/aniyomi_restore_error.txt` | `writeErrorLog` (`BackupRestorer.kt:415`), via `createFileInCacheDir` | overwritten by each restore with errors | Lines `[yyyy-MM-dd HH:mm:ss.SSS] message`; not created when there are no errors. Linux: `$XDG_CACHE_HOME/kurobayomi/aniyomi_restore_error.txt` (see p1-rust-design.md). |
| `cacheDir/<pkgName>.apk` | `ExtensionsRestorer` (Android only) | per call | Dropped on Linux (T-22a-029). |
| `<selected>.tachibk` (input) | created by Create-backup (area:21) | user-owned | Restore reads it in place; no copy. Name pattern: `${APPLICATION_ID}_yyyy-MM-dd_HH-mm.tachibk` (`BackupCreator.kt:232-235`). |
| `ComicInfo.xml` (inside CBZ/EPUB) | local-source metadata (area:14) | not part of backup | Name constant `COMIC_INFO_FILE` (`ComicInfo.kt:9`). |

No other files are written by the restore path.

---

## 8. Network endpoints

None. The restore path performs no HTTP requests. Specifically:
- Adding an extension repo (`MangaExtensionRepoRestorer`) inserts a row only; it does not fetch the repo index.
- Adding an anime extension store (`AnimeExtensionStoreRestorer`) upserts a row only.
- The library update and tracker refresh are not triggered; a source comment at `BackupRestorer.kt:249` records this as not implemented.
- Validation checks the tracker login state only (`TrackerManager.get(id).isLoggedIn`), no request.

---

## 9. WorkManager names and tags (for the Linux job model)

| Name | Kind | Policy | Source |
|---|---|---|---|
| `BackupRestore` (unique work name = tag) | one-time | `ExistingWorkPolicy.KEEP` | `BackupRestoreJob.kt:91-95,104` |
| `BackupCreator` | periodic (auto backup) | `ExistingPeriodicWorkPolicy.UPDATE` | `BackupCreateJob.kt:108-110,131` |
| `BackupCreator:manual` | one-time | `ExistingWorkPolicy.KEEP` | `BackupCreateJob.kt:126,132` |

Restore's `isRunning` checks the tag `BackupRestore` with state `RUNNING` (`WorkManagerExtensions.kt:14-17`). Auto backup returns `Result.retry()` when a restore is running (`BackupCreateJob.kt:42`).

---

## 10. Notification ids, channels and actions

| Item | Value | Source |
|---|---|---|
| Progress id | `-503` (`ID_RESTORE_PROGRESS`) | `Notifications.kt:61` |
| Completion/error id | `-504` (`ID_RESTORE_COMPLETE`) | `Notifications.kt:64` |
| Progress channel | `backup_restore_progress_channel`, importance LOW | `Notifications.kt:59,165-167` |
| Completion channel | `backup_restore_complete_channel_v2`, importance HIGH | `Notifications.kt:62,170-172` |
| Cancel action | broadcast `"$ID.$NAME.CANCEL_RESTORE"` with extra `EXTRA_NOTIFICATION_ID` | `NotificationReceiver.kt:385,980-993` |
| Open error log action | `ACTION_VIEW`, `text/plain`, `FLAG_ACTIVITY_NEW_TASK | FLAG_GRANT_READ_URI_PERMISSION` | `NotificationReceiver.kt:970-978` |
| Share action | (create side) | area:21 |

Notification icons: `R.drawable.ic_ani` (small), `R.mipmap.ic_launcher` (large) (`BackupNotifier.kt:165-166`). Linux: the app icon name registered with GTK (see p1-rust-design.md §8).

---

## 11. Bit-flag and numeric layouts touched by restore

| Field | Layout | Owner |
|---|---|---|
| `RestoreOptions` BooleanArray | index 0 libraryEntries, 1 categories, 2 appSettings, 3 extensionStores, 4 customButtons, 5 sourceSettings, 6 extensions (length 7) | this area (`RestoreOptions.kt:448-456,505-513`) |
| `Manga.viewerFlags`, `Manga.chapterFlags`, `Anime.viewerFlags`, `Anime.episodeFlags`, `Anime.seasonFlags` | Long pass-through; interpreted by domain/UI | area:07 / area:08 |
| `Category.flags` | Long pass-through | area:08 |
| `Chapter.lastPageRead`, `Episode.lastSecondSeen`, `Episode.totalSeconds` | Long, seconds for episodes, page index for chapters | area:07 |
| `MangaHistory.readDuration`, `history.time_read` | Long milliseconds (readDuration summed) | area:06 |
| `seasonNumber` | Double; `-1.0` means not a season | area:16/07 |

No bit layout is decoded by restore itself.

---

## 12. Enumerated fields of the restore output (log and error text)

- Error entry: `"${title} [${sourceName}]: ${message}"`.
- Log line: `"[${timestamp}] ${message}\n"`, timestamp `yyyy-MM-dd HH:mm:ss.SSS`.
- Extension store error: `"Error Adding Anime Repo: ${name} : ${message}"`.
- Extension repo error: `"Error Adding Manga Repo: ${name} : ${message}"`.
- Repo conflict messages: `"Already Exists with different signing key fingerprint"`, `"${name} has the same signing key fingerprint"`.
