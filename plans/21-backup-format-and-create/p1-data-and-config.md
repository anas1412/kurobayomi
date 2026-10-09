# Area 21: Data models, wire layout, preferences, files and storage

Source tree: `/home/blackbox/aniyomi-src`. All field numbers below are copied from the `@ProtoNumber` annotations. "Wire" is the protobuf wire type (0 varint, 1 fixed64, 2 length-delimited, 5 fixed32). "Kotlin type" is the declared type. "Null" means `?` (absent on the wire when null). "Default" is the Kotlin default, which decoders use when the field is absent. Non-null fields without a default are required: a missing one makes kotlinx throw `MissingFieldException` (a `SerializationException`, reported as "Backup file is corrupted").

Encoding notes that apply to every table (see topic file `p1-topic-protobuf-wire-format.md` for the reasoning and the fixture plan):
- `Long`, `Int`, `Boolean`: varint (wire 0). `Boolean` is 0 or 1. Negative `Int`/`Long` take 10 bytes (protobuf int32/int64 semantics; no zigzag).
- `Float`: fixed32, wire 5, IEEE 754 little-endian. `Double`: fixed64, wire 1.
- `String`: length-delimited UTF-8 (wire 2). `ByteArray`: length-delimited (wire 2).
- `List<T>` of messages or strings: one tag per element (wire 2 each). `List<Long>` (`categories`) is NOT packed (one varint tag per element) because kotlinx packs only with `@ProtoPacked`, which is not used here.
- `Set<String>`: same as `List<String>` (repeated, unpacked).
- Enum: varint of the ordinal (declaration index). Verify with a fixture (topic file).
- Null fields are omitted. Non-null fields are written including their defaults, assuming kotlinx's default `encodeDefaults` for the `ProtoBuf` object registered at `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:190-191` (not visible in the source text; verify with a fixture, topic file rule K-6).
- Unknown field numbers are skipped on decode.

---

## 1. Container messages (`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt`)

### 1.1 `Backup` (current format, written by the app)

| Field no. | Name | Kotlin type | Wire | Default | Null | Written by | Notes |
|---|---|---|---|---|---|---|---|
| 1 | backupManga | `List<BackupManga>` | 2 | `emptyList()` | no | manga creator | |
| 2 | backupCategories | `List<BackupCategory>` | 2 | `emptyList()` | no | manga categories | `var` |
| 101 | backupSources | `List<BackupSource>` | 2 | `emptyList()` | no | manga sources | `var`; comment: "Bump by 100 to specify this is a 0.x value" |
| 104 | backupPreferences | `List<BackupPreference>` | 2 | `emptyList()` | no | app preferences | `var` |
| 105 | backupSourcePreferences | `List<BackupSourcePreferences>` | 2 | `emptyList()` | no | source preferences | `var` |
| 106 | backupMangaExtensionRepo | `List<BackupExtensionRepos>` | 2 | `emptyList()` | no | manga repos | `var`; NOTE: number 106 means `backupExtensions` in `LegacyBackup` |
| 500 | isLegacy | `Boolean` | 0 | `true` | no | always `false` | `val`; the detector reads this |
| 501 | backupAnime | `List<BackupAnime>` | 2 | `emptyList()` | no | anime creator | `val` |
| 502 | backupAnimeCategories | `List<BackupCategory>` | 2 | `emptyList()` | no | anime categories | `var` |
| 503 | backupAnimeSources | `List<BackupAnimeSource>` | 2 | `emptyList()` | no | anime sources | `var` |
| 504 | backupExtensions | `List<BackupExtension>` | 2 | `emptyList()` | no | extensions | `var` |
| 505 | backupAnimeExtensionStores | `List<BackupExtensionStore>` | 2 | `emptyList()` | no | anime stores | `var` |
| 506 | backupCustomButton | `List<BackupCustomButtons>` | 2 | `emptyList()` | no | custom buttons | `var` |

Field 103 and field 3 are NOT used by `Backup`. A current file never contains them.

### 1.2 `LegacyBackup` (pre-1.0 Aniyomi format, read only)

| Field no. | Name | Kotlin type | Default | Maps to (`toBackup()`) |
|---|---|---|---|---|
| 1 | backupManga | `List<BackupManga>` | `emptyList()` | backupManga |
| 2 | backupCategories | `List<BackupCategory>` | `emptyList()` | backupCategories |
| 3 | backupAnime | `List<BackupAnime>` | `emptyList()` | backupAnime |
| 4 | backupAnimeCategories | `List<BackupCategory>` | `emptyList()` | backupAnimeCategories |
| 101 | backupSources | `List<BackupSource>` | `emptyList()` | backupSources |
| 103 | backupAnimeSources | `List<BackupAnimeSource>` | `emptyList()` | backupAnimeSources |
| 104 | backupPreferences | `List<BackupPreference>` | `emptyList()` | backupPreferences |
| 105 | backupSourcePreferences | `List<BackupSourcePreferences>` | `emptyList()` | backupSourcePreferences |
| 106 | backupExtensions | `List<BackupExtension>` | `emptyList()` | backupExtensions |
| 107 | backupAnimeExtensionStores | `List<BackupExtensionStore>` | `emptyList()` | backupAnimeExtensionStores |
| 108 | backupMangaExtensionRepo | `List<BackupExtensionRepos>` | `emptyList()` | backupMangaExtensionRepo |
| 109 | backupCustomButton | `List<BackupCustomButtons>` | `emptyList()` | backupCustomButton |

`toBackup()` sets `isLegacy = false` ("Only used for detection") and copies every list. Conversion is lossless. Field 102 (broken history) and 100 (broken sources) are not declared; they are skipped on decode.

Conflicts to remember: field 3 is `backupAnime` in legacy and is not declared at all in current `Backup`; field 106 is `backupExtensions` in legacy and `backupMangaExtensionRepo` in current; legacy 107/108/109 (anime stores, manga repos, custom buttons) are current 505/106/506. Decoding must therefore pick the message type from the detector, never from field numbers alone.

### 1.3 `full/models/Backup.kt` (dead, not written or read; DROP)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/Backup.kt`: fields 1 backupManga, 2 backupCategories, 3 backupAnime, 4 backupAnimeCategories, 101 backupSources, 103 backupAnimeSources, 104 backupPreferences. Not referenced outside its package. The same field numbers are in the current `Backup` except 3 and 103.

---

## 2. Entry messages

### 2.1 `BackupManga` (field 1 of `Backup` is list of this)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupManga.kt`. Annotation `@Suppress("DEPRECATION")`, `@Serializable data class`.

| No. | Name | Kotlin type | Wire | Default | Null | Written by creator | Notes |
|---|---|---|---|---|---|---|---|
| 1 | source | `Long` | 0 | none (required) | no | `source` | `var` |
| 2 | url | `String` | 2 | none (required) | no | `url` | "url is called key in 1.x" |
| 3 | title | `String` | 2 | `""` | no | `title` | |
| 4 | artist | `String?` | 2 | `null` | yes | `artist` | |
| 5 | author | `String?` | 2 | `null` | yes | `author` | |
| 6 | description | `String?` | 2 | `null` | yes | `description` | |
| 7 | genre | `List<String>` | 2 | `emptyList()` | no | `genre.orEmpty()` | repeated |
| 8 | status | `Int` | 0 | `0` | no | `status.toInt()` | |
| 9 | thumbnailUrl | `String?` | 2 | `null` | yes | `thumbnailUrl` | "cover in 1.x" |
| 13 | dateAdded | `Long` | 0 | `0` | no | `dateAdded` | |
| 14 | viewer | `Int` | 0 | `0` | no | `viewerFlags and 0x7` | "Replaced by viewer_flags" |
| 16 | chapters | `List<BackupChapter>` | 2 | `emptyList()` | no | chapters | |
| 17 | categories | `List<Long>` | 0 (each) | `emptyList()` | no | category `order` values | unpacked |
| 18 | tracking | `List<BackupTracking>` | 2 | `emptyList()` | no | tracking | |
| 100 | favorite | `Boolean` | 0 | `true` | no | `favorite` | "Bump by 100 ... 0.x" |
| 101 | chapterFlags | `Int` | 0 | `0` | no | `chapterFlags.toInt()` | |
| 103 | viewer_flags | `Int?` | 0 | `null` | yes | `viewerFlags.toInt()` | full reader flags |
| 104 | history | `List<BackupHistory>` | 2 | `emptyList()` | no | history | |
| 105 | updateStrategy | `UpdateStrategy` (enum, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt`) | 0 | `ALWAYS_UPDATE` | no | `updateStrategy` | ordinal: ALWAYS_UPDATE=0, ONLY_FETCH_ONCE=1 |
| 106 | lastModifiedAt | `Long` | 0 | `0` | no | `lastModifiedAt` | |
| 107 | favoriteModifiedAt | `Long?` | 0 | `null` | yes | `favoriteModifiedAt` | |
| 108 | excludedScanlators | `List<String>` | 2 | `emptyList()` | no | excluded scanlators | |
| 109 | version | `Long` | 0 | `0` | no | `version` | |

Decode mapping (`getMangaImpl()`): `viewerFlags = (viewer_flags ?: viewer).toLong()`. So `viewer_flags` (103) wins; `viewer` (14) is the fallback for old files.

Encode mapping (creator): `viewer = viewerFlags and 0x7`, `viewer_flags = viewerFlags` (full value). Both are written.

Manga `chapterFlags` for the manga `Manga` entity is `Long`; the backup stores `Int` (truncation for values above 2^31-1).

### 2.2 `BackupChapter` (field 16 of `BackupManga`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupChapter.kt`.

| No. | Name | Kotlin type | Wire | Default | Null |
|---|---|---|---|---|---|
| 1 | url | `String` | 2 | none (required) | no |
| 2 | name | `String` | 2 | none (required) | no |
| 3 | scanlator | `String?` | 2 | `null` | yes |
| 4 | read | `Boolean` | 0 | `false` | no |
| 5 | bookmark | `Boolean` | 0 | `false` | no |
| 6 | lastPageRead | `Long` | 0 | `0` | no |
| 7 | dateFetch | `Long` | 0 | `0` | no |
| 8 | dateUpload | `Long` | 0 | `0` | no |
| 9 | chapterNumber | `Float` | 5 | `0F` | no |
| 10 | sourceOrder | `Long` | 0 | `0` | no |
| 11 | lastModifiedAt | `Long` | 0 | `0` | no |
| 12 | version | `Long` | 0 | `0` | no |

Decode: `chapterNumber.toDouble()`, `sourceOrder = source_order`, and the other fields 1:1. Comments in source: "url is called key in 1.x"; "lastPageRead is called progress in 1.x"; "chapterNumber is called number in 1.x".

Encode from the SQL row (`backupChapterMapper`, 15 positional columns: 2 ignored ids, url, name, scanlator, read, bookmark, lastPageRead, chapterNumber (Double -> Float), source_order, dateFetch, dateUpload, lastModifiedAt, version, one ignored trailing column).

### 2.3 `BackupHistory` (manga, field 104 of `BackupManga`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupHistory.kt`.

| No. | Name | Kotlin type | Wire | Default |
|---|---|---|---|---|
| 1 | url | `String` | 2 | none (required) (chapter URL) |
| 2 | lastRead | `Long` | 0 | none (required) (epoch ms) |
| 3 | readDuration | `Long` | 0 | `0` |

Decode: `MangaHistory(readAt = Date(lastRead), readDuration)`.

### 2.4 `BackupTracking` (manga, field 18 of `BackupManga`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupTracking.kt`.

| No. | Name | Kotlin type | Wire | Default | Null | Notes |
|---|---|---|---|---|---|---|
| 1 | syncId | `Int` | 0 | none (required) | no | tracker id, e.g. the tracker's numeric id |
| 2 | libraryId | `Long` | 0 | none (required) | no | "not null in 1.x"; creator writes `libraryId ?: 0` |
| 3 | mediaIdInt | `Int` | 0 | `0` | no | DEPRECATED (`@Deprecated`, WARNING); only read: used if non-zero |
| 4 | trackingUrl | `String` | 2 | `""` | no | "mediaUrl in 1.x" |
| 5 | title | `String` | 2 | `""` | no | |
| 6 | lastChapterRead | `Float` | 5 | `0F` | no | Double in DB |
| 7 | totalChapters | `Int` | 0 | `0` | no | |
| 8 | score | `Float` | 5 | `0F` | no | Double in DB |
| 9 | status | `Int` | 0 | `0` | no | |
| 10 | startedReadingDate | `Long` | 0 | `0` | no | "startReadTime in 1.x" |
| 11 | finishedReadingDate | `Long` | 0 | `0` | no | "endReadTime in 1.x" |
| 12 | private | `Boolean` | 0 | `false` | no | |
| 100 | mediaId | `Long` | 0 | `0` | no | authoritative media id |

Decode `remoteId = if (mediaIdInt != 0) mediaIdInt else mediaId`. Encode writes `mediaId` (100) only; `mediaIdInt` is never written by the creator.

SQL row mapper (`backupMangaTrackMapper`): `_id, manga_id, syncId, mediaId, libraryId (nullable), title, lastChapterRead (Double), totalChapters (Long), status (Long), score (Double), remoteUrl, startDate, finishDate, private`.

### 2.5 `BackupAnime` (field 501 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnime.kt`. `@Suppress("DEPRECATION")`, `@Serializable class`.

| No. | Name | Kotlin type | Wire | Default | Null | Written by creator | Notes |
|---|---|---|---|---|---|---|---|
| 1 | source | `Long` | 0 | none (required) | no | `source` | |
| 2 | url | `String` | 2 | none (required) | no | `url` | "key in 1.x" |
| 3 | title | `String` | 2 | `""` | no | `title` | |
| 4 | artist | `String?` | 2 | `null` | yes | `artist` | |
| 5 | author | `String?` | 2 | `null` | yes | `author` | |
| 6 | description | `String?` | 2 | `null` | yes | `description` | |
| 7 | genre | `List<String>` | 2 | `emptyList()` | no | `genre.orEmpty()` | |
| 8 | status | `Int` | 0 | `0` | no | `status.toInt()` | |
| 9 | thumbnailUrl | `String?` | 2 | `null` | yes | `thumbnailUrl` | "cover in 1.x" |
| 13 | dateAdded | `Long` | 0 | `0` | no | `dateAdded` | |
| 16 | episodes | `List<BackupEpisode>` | 2 | `emptyList()` | no | episodes | |
| 17 | categories | `List<Long>` | 0 (each) | `emptyList()` | no | category `order`s | unpacked |
| 18 | tracking | `List<BackupAnimeTracking>` | 2 | `emptyList()` | no | tracking | |
| 100 | favorite | `Boolean` | 0 | `true` | no | `favorite` | |
| 101 | episodeFlags | `Int` | 0 | `0` | no | `episodeFlags.toInt()` | |
| 103 | viewer_flags | `Int` | 0 | `0` | no | `viewerFlags.toInt()` | NOT nullable here (manga's is `Int?`) |
| 104 | history | `List<BackupAnimeHistory>` | 2 | `emptyList()` | no | history | |
| 105 | updateStrategy | `AnimeUpdateStrategy` (enum, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt`) | 0 | `ALWAYS_UPDATE` | no | `updateStrategy` | ordinal: ALWAYS_UPDATE=0, ONLY_FETCH_ONCE=1 |
| 106 | lastModifiedAt | `Long` | 0 | `0` | no | `lastModifiedAt` | |
| 107 | favoriteModifiedAt | `Long?` | 0 | `null` | yes | `favoriteModifiedAt` | |
| 109 | version | `Long` | 0 | `0` | no | `version` | |
| 112 | memo | `ByteArray` | 2 | `JsonObjectEmptyBytes` (`7B 7D`) | no | `MemoColumnAdapter.encode(memo)` | JSON text bytes |
| 500 | backgroundUrl | `String?` | 2 | `null` | yes | NOT WRITTEN by creator | "Aniyomi specific values" |
| 502 | parentId | `Long?` | 0 | `null` | yes | `parentId` | season parent |
| 503 | id | `Long?` | 0 | `null` | yes | `id` | "Used to associate seasons with parents. Do not use for anything else." |
| 504 | seasonFlags | `Long` | 0 | `0` | no | `seasonFlags` | |
| 505 | seasonNumber | `Double` | 1 | `-1.0` | no | `seasonNumber` | fixed64 |
| 506 | seasonSourceOrder | `Long` | 0 | `0` | no | `seasonSourceOrder` | |
| 507 | fetchType | `FetchType` (enum, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt`) | 0 | `Episodes` | no | `fetchType` | ordinal: Seasons=0, Episodes=1 |

Not declared (skipped on decode): 10, 11, 12, 15 (1.x values), 102 (broken history), 501 (broken, "do not use").

Decode mapping (`getAnimeImpl`): `viewerFlags = viewer_flags.toLong()`; `episodeFlags = episodeFlags.toLong()`; `status.toLong()`; `memo = MemoColumnAdapter.decode(memo)` (`JsonObject` from UTF-8 text); other fields 1:1.

### 2.6 `BackupEpisode` (field 16 of `BackupAnime`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupEpisode.kt`.

| No. | Name | Kotlin type | Wire | Default | Null | Notes |
|---|---|---|---|---|---|---|
| 1 | url | `String` | 2 | none (required) | no | "key in 1.x" |
| 2 | name | `String` | 2 | none (required) | no | |
| 3 | scanlator | `String?` | 2 | `null` | yes | |
| 4 | seen | `Boolean` | 0 | `false` | no | |
| 5 | bookmark | `Boolean` | 0 | `false` | no | |
| 6 | lastSecondSeen | `Long` | 0 | `0` | no | "progress in 1.x" |
| 16 | totalSeconds | `Long` | 0 | `0` | no | |
| 7 | dateFetch | `Long` | 0 | `0` | no | |
| 8 | dateUpload | `Long` | 0 | `0` | no | |
| 9 | episodeNumber | `Float` | 5 | `0F` | no | "number in 1.x" |
| 10 | sourceOrder | `Long` | 0 | `0` | no | |
| 11 | lastModifiedAt | `Long` | 0 | `0` | no | |
| 12 | version | `Long` | 0 | `0` | no | |
| 13 | memo | `ByteArray` | 2 | `7B 7D` | no | JSON text |
| 501 | fillermark | `Boolean` | 0 | `false` | no | Aniyomi-specific |
| 502 | summary | `String?` | 2 | `null` | yes | Aniyomi-specific |
| 503 | previewUrl | `String?` | 2 | `null` | yes | Aniyomi-specific |

SQL row mapper (`backupEpisodeMapper`, 20 positional columns; `memo: JsonObject` encoded by `MemoColumnAdapter`).

### 2.7 `BackupAnimeTracking` (field 18 of `BackupAnime`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeTracking.kt`. Same numbers as `BackupTracking` with these differences: 6 `lastEpisodeSeen: Float` ("last seen ... changed to a float in 1.x"), 7 `totalEpisodes: Int`, 10 `startedWatchingDate: Long`, 11 `finishedWatchingDate: Long`. All other numbers, defaults and decode rules are as in 2.4 (`remoteId = mediaIdInt if != 0 else mediaId`).

SQL row mapper (`backupAnimeTrackMapper`): `_id, anime_id, syncId, mediaId, libraryId (nullable), title, lastEpisodeSeen (Double), totalEpisodes (Long), status (Long), score (Double), remoteUrl, startDate, finishDate, private`.

### 2.8 `BackupAnimeHistory` (field 104 of `BackupAnime`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeHistory.kt`: 1 url (String, required), 2 lastRead (Long, required, epoch ms), 3 readDuration (Long, default 0). Decode: `AnimeHistory(seenAt = Date(lastRead))` (readDuration is not carried for anime).

---

## 3. Category, source and store messages

### 3.1 `BackupCategory` (fields 2 and 502 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupCategory.kt`: `class`, `@Serializable`.

| No. | Name | Kotlin type | Wire | Default | Null |
|---|---|---|---|---|---|
| 1 | name | `String` | 2 | none (required) | no |
| 2 | order | `Long` | 0 | `0` | no |
| 3 | id | `Long` | 0 | `0` | no |
| 100 | flags | `Long` | 0 | `0` | no |

Field 3 comment: `// @ProtoNumber(3) val updateInterval: Int = 0, 1.x value not used in 0.x`. Decode `toCategory(id)` builds `Category(id, name, flags, order, hidden = false)`.

The entries reference categories by `order` (manga `categories`, anime `categories`), not by `id`.

### 3.2 `BackupSource` (field 101 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupSource.kt`: 1 name (`String`, default `""`), 2 sourceId (`Long`, required). The `BrokenBackupSource` in the same file (field 0 name, field 1 sourceId) is unused and is dropped.

### 3.3 `BackupAnimeSource` (field 503 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeSource.kt`: 1 name (`String`, default `""`), 2 sourceId (`Long`, required). `BrokenBackupAnimeSource` (field 0 name, 1 sourceId) is unused and dropped.

Legacy anime sources are field 103 of `LegacyBackup` with the same message shape. The detector reads this message with its own partial declaration (`DetectAnimeSource`, name=1, sourceId=2).

### 3.4 `BackupExtensionRepos` (field 106 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionRepos.kt`: 1 baseUrl (`String`, required), 2 name (`String`, required), 3 shortName (`String?`, null), 4 website (`String`, required), 5 signingKeyFingerprint (`String`, required). Mapper from `ExtensionRepo` (`domain/src/main/java/mihon/domain/extensionrepo/model/ExtensionRepo.kt`): `baseUrl, name, shortName, website, signingKeyFingerprint`.

### 3.5 `BackupExtensionStore` (field 505 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionStore.kt`:

| No. | Name | Kotlin type | Wire | Default | Null |
|---|---|---|---|---|---|
| 1 | indexUrl | `String` | 2 | required | no |
| 2 | name | `String` | 2 | required | no |
| 3 | badgeLabel | `String?` | 2 | `null` | yes |
| 5 | signingKey | `String` | 2 | required | no |
| 4 | contactWebsite | `String` | 2 | required | no |
| 6 | contactDiscord | `String?` | 2 | `null` | yes |
| 7 | isLegacy | `Boolean?` | 0 | `null` | yes |
| 8 | extensionListUrl | `String?` | 2 | `null` | yes |

Mapper from `AnimeExtensionStore` (`domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt`): `indexUrl, name, badgeLabel, signingKey, contact.website, contact.discord, isLegacy, extensionListUrl`. The model's `badgeLabel`, `signingKey`, `contact` are non-null in the domain model and become nullable on the wire.

### 3.6 `BackupExtension` (field 504 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtension.kt`: 1 pkgName (`String`, required), 2 apk (`ByteArray`, required). Custom `equals`/`hashCode` compare `pkgName` and the byte content.

### 3.7 `BackupCustomButtons` (field 506 of `Backup`)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupCustomButtons.kt`: 1 name (`String`, required), 2 isFavorite (`Boolean`, required), 3 sortIndex (`Long`, required), 4 content (`String`, required), 5 longPressContent (`String`, required), 6 onStartup (`String`, required). Note: these have no defaults, so an entry missing any of them fails decode. Mapper from `CustomButton` (`domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt`): `name, isFavorite, sortIndex, content, longPressContent, onStartup` (the `id` is not written).

---

## 4. Preference messages

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt`.

### 4.1 `BackupPreference`

| No. | Name | Kotlin type | Wire | Default | Null |
|---|---|---|---|---|---|
| 1 | key | `String` | 2 | required | no |
| 2 | value | `PreferenceValue` (sealed) | 2 (polymorphic message) | required | no |

### 4.2 `BackupSourcePreferences` (field 105 of `Backup`)

| No. | Name | Kotlin type | Wire | Default |
|---|---|---|---|---|
| 1 | sourceKey | `String` | 2 | required (`source_<id>`) |
| 2 | prefs | `List<BackupPreference>` | 2 | required (may be non-empty only; creator filters empty) |

### 4.3 `PreferenceValue` (sealed) and its six subclasses

| Subclass | Kotlin payload | Stored key type (SharedPreferences / PreferenceStore) |
|---|---|---|
| `IntPreferenceValue(value: Int)` | `Int` | Int |
| `LongPreferenceValue(value: Long)` | `Long` | Long |
| `FloatPreferenceValue(value: Float)` | `Float` (fixed32) | Float |
| `StringPreferenceValue(value: String)` | `String` | String |
| `BooleanPreferenceValue(value: Boolean)` | `Boolean` | Boolean |
| `StringSetPreferenceValue(value: Set<String>)` | repeated string | Set<String> |

The wire encoding of a sealed hierarchy is kotlinx-specific (the subclass is identified by a field number derived from its position in the sealed hierarchy). The exact numbers are NOT in the source text; they must be read from the generated schema (the debug screen `BackupSchemaScreen` calls `ProtoBufSchemaGenerator.generateSchemaText(Backup.serializer().descriptor)`) or from a fixture file produced by the app. Topic file section 4 has the procedure.

---

## 5. Field-number summary (current format)

```
message Backup {
  repeated BackupManga manga = 1;
  repeated BackupCategory categories = 2;
  repeated BackupSource sources = 101;
  repeated BackupPreference preferences = 104;
  repeated BackupSourcePreferences source_preferences = 105;
  repeated BackupExtensionRepos manga_extension_repos = 106;
  bool is_legacy = 500;            // always false when written
  repeated BackupAnime anime = 501;
  repeated BackupCategory anime_categories = 502;
  repeated BackupAnimeSource anime_sources = 503;
  repeated BackupExtension extensions = 504;
  repeated BackupExtensionStore anime_extension_stores = 505;
  repeated BackupCustomButtons custom_buttons = 506;
}
```
(Names are Kurobayomi's; the numbers and types are the Aniyomi ones. The Kotlin property names are in sections 1-3.)

---

## 6. Kurobayomi configuration keys (Proposal, data side of this area)

Aniyomi keys read or written by this area, with defaults:

| Key (Aniyomi literal) | Type | Default | Kind | Read/written by | Kurobayomi key (Proposal) |
|---|---|---|---|---|---|
| `backup_interval` | Int | `12` | normal | `BackupPreferences.kt:8` (read in `setupTask`, written by UI and `EnableAutoBackupMigration`) | `backup.interval_hours: u32 = 12`, allowed {0, 6, 12, 24, 48, 168} |
| `__APP_STATE_last_auto_backup_timestamp` (`Preference.appStateKey("last_auto_backup_timestamp")`) | Long | `0` | app state | `BackupCreator.kt:150`, `SettingsDataScreen.kt:195` | `state.backup.last_auto_epoch_ms: i64 = 0` |
| `backup_flags` | StringSet | `{"1","2","4","8"}` (`FLAG_CATEGORIES`, `FLAG_CHAPTERS`, `FLAG_HISTORY`, `FLAG_TRACK`) | normal | NONE (defined only; `BackupPreferences.kt:14-17`) | DROP (dead) |
| `hide_notification_content` (`SecurityPreferences.kt:19`) | Boolean | `false` | normal | `BackupNotifier.kt:103` | `notifications.hide_content: bool = false` (owned by area:02) |
| `__PRIVATE_*` keys (tracker tokens and credentials) | various | various | private | `PreferenceBackupCreator` (`withPrivatePreferences`) | excluded unless `privateSettings` |
| `__APP_STATE_*` keys | various | various | app state | `isAppState` filter | never exported |
| `storage_dir` (`__APP_STATE_storage_dir`) | String | folder provider path | app state | `StoragePreferences.kt:12` | base directory; automatic backups go to `<base>/autobackup` |

Kotlin `FLAG_*` constants (`domain/src/main/java/tachiyomi/domain/backup/service/PreferenceValues.kt`): `FLAG_CATEGORIES = "1"`, `FLAG_CHAPTERS = "2"`, `FLAG_HISTORY = "4"`, `FLAG_TRACK = "8"`, `FLAG_SETTINGS = "10"`, `FLAG_EXT_SETTINGS = "20"`, `FLAG_EXTENSIONS = "40"`. Only the first four are referenced (by the dead `backup_flags` default). Note the last three are written as decimal strings of hexadecimal values; they are unused.

Create options (`BackupOptions`) are NOT persisted. Each Create Backup screen starts from the defaults in B-02 of p1-behavior.md.

---

## 7. Files, folders and formats on disk

| Item | Location | Format | Written by | Notes |
|---|---|---|---|---|
| Manual backup | user-chosen SAF document (Kurobayomi: GTK save dialog) | gzip(protobuf `Backup`), extension `.tachibk`, suggested name `xyz.jmir.tachiyomi.mi_yyyy-MM-dd_HH-mm.tachibk` (Aniyomi) | `BackupCreator.backup` | Truncated before write. Deleted on any failure. |
| Automatic backup | `<base storage>/autobackup/` (`StorageManager.kt`, constant `AUTOMATIC_BACKUPS_PATH = "autobackup"`) | same | `BackupCreator.backup` (auto) | Up to 4 kept. Created with `createDirectory`, so the directory is created on demand. |
| Base storage directory | `storage_dir` pref (SAF tree or path) | directory tree | `StorageManager` | If missing (`UniFile.exists()` false) `getAutomaticBackupsDirectory()` returns null and the job fails with `create_backup_file_error` (manual) or fails silently (auto). |
| Restore staging | none in this area | | | |
| Notification icons | `ic_ani`, `ic_share_24dp`, `ic_folder_24dp`, `ic_close_24dp`, launcher mipmap | Android resources | | Replaced by freedesktop icon names on Linux (p1-rust-design.md). |

Kurobayomi (Proposal):
- Manual: user-chosen path through `gtk::FileDialog::save`, extension forced to `.tachibk`.
- Automatic: `$XDG_DATA_HOME/kurobayomi/autobackup/` (fallback `~/.local/share/kurobayomi/autobackup/`), files `kurobayomi_<yyyy-MM-dd_HH-mm-ss>.tachibk`.
- Temporary file `<name>.tachibk.part` in the same directory, renamed on success (atomic on the same filesystem).

---

## 8. Network endpoints

None. This area performs no HTTP. The extension APKs (`ExtensionsBackupCreator`) are read from the local installation (`PackageManager.getApplicationInfo(pkg).publicSourceDir`).

## 9. Database tables read (creator side)

| Table (Aniyomi) | Queries used | Columns used |
|---|---|---|
| `mangas` | `getReadMangaNotInLibrary` (`data/src/main/sqldelight/data/mangas.sq:81-88`), favourites via `GetMangaFavorites` | all |
| `animes` | `getWatchedAnimeNotInLibrary` (`data/src/main/sqldelightanime/dataanime/animes.sq:92-99`), favourites via `GetAnimeFavorites` | all |
| `chapters` | `getChaptersByMangaId` (`chapters.sq:56-69`), `getChapterById` (`chapters.sq:51-54`) | all |
| `episodes` | `getEpisodesByAnimeId` (`episodes.sq:62-64`), `getEpisodeById` (`episodes.sq:57-60`) | all |
| `history` | `getHistoryByMangaId` (`data/src/main/sqldelight/data/history.sq:14-23`), anime twin via `GetAnimeHistory` | `chapter_id`/`episode_id`, `last_read`, `time_read` |
| `excluded_scanlators` | `getExcludedScanlatorsByMangaId` (`excluded_scanlators.sq:19-22`) | `scanlator` |
| `manga_sync` | `getTracksByMangaId` (`manga_sync.sq:36-39`) | all |
| `anime_sync` | `getTracksByAnimeId` (`anime_sync.sq:37-40`) | all |
| `mangas_categories` / categories | `GetMangaCategories.await(mangaId)`, `await()` | `order`, `id`, `name`, `flags` |
| animes categories | `GetAnimeCategories.await(animeId)`, `await()` | same |
| `extension_store` | `getAll` (`data/src/main/sqldelightanime/dataanime/extension_store.sq:19-20`) | all |
| `extension_repos` | `getAll` via `GetMangaExtensionRepo` | all |
| custom buttons table | `GetCustomButtons.getAll()` | all |

Ordering: no ORDER BY in the backup queries. Row order is the SQLite scan order.

## 10. Round-trip contract (what must hold between this area and restore)

- Every field above that the restore side reads must be written by this area with the same number and wire type.
- `Backup.isLegacy` must be written as `false` by Kurobayomi, so Aniyomi's detector classifies Kurobayomi files as current.
- Category links (`categories`) use `order`. Kurobayomi must write the category's order, not its id, to stay readable by Aniyomi.
- Kurobayomi writes `viewer` (14) = `viewer_flags & 0x7` as well as `viewer_flags` (103) for manga, so old Aniyomi readers still see the mode.
- Kurobayomi writes `chapterNumber`/`episodeNumber` as float32, same as Aniyomi.
