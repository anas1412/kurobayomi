# Area 06 — Data models, preference keys, on-disk formats, endpoints, SQL

Paths are repo-relative to `/home/blackbox/aniyomi-src`. "Rust" columns are the Proposal types (module in `p1-rust-design.md`). All timestamps are epoch milliseconds in an `i64` unless stated.

## 1. Domain models owned or produced by this area

### 1.1 History

| Kotlin (`domain/src/main/java/tachiyomi/domain/history/{manga,anime}/model/*.kt`) | Field | Type | Null | Default / notes |
|---|---|---|---|---|
| `MangaHistory` (`manga/model/MangaHistory.kt`) | `id` | Long | no | `-1L` in `MangaHistory.create()` |
| | `chapterId` | Long | no | `-1L` in `create()` |
| | `readAt` | Date | yes | `null` in `create()`; from DB: `Date(ms)` or null; reset rows are `Date(0)` |
| | `readDuration` | Long (ms) | no | `-1L` in `create()` |
| `MangaHistoryUpdate` | `chapterId` Long, `readAt` Date, `sessionReadDuration` Long | | no | no defaults |
| `MangaHistoryWithRelations` | `id`, `chapterId`, `mangaId`, `title: String`, `chapterNumber: Double`, `readAt: Date?`, `readDuration: Long`, `coverData: MangaCover` | | | `coverData = MangaCover(mangaId, sourceId, isMangaFavorite, url = thumbnailUrl, lastModified = coverLastModified)` |
| `AnimeHistory` | `id`, `episodeId`, `seenAt: Date?` | | | `create()` = (-1, -1, null) |
| `AnimeHistoryUpdate` | `episodeId: Long`, `seenAt: Date` | | | |
| `AnimeHistoryWithRelations` | `id`, `episodeId`, `animeId`, `title`, `episodeNumber: Double`, `seenAt: Date?`, `coverData: AnimeCover` | | | `AnimeCover(animeId, sourceId, isAnimeFavorite, url, lastModified)` |

Rust (`kuro_core::data::history`):

```rust
pub struct MangaHistory { pub id: i64, pub chapter_id: i64, pub read_at: Option<i64>, pub read_duration: i64 }
pub struct MangaHistoryUpdate { pub chapter_id: i64, pub read_at: i64, pub session_read_duration: i64 }
pub struct MangaHistoryWithRelations {
    pub id: i64, pub chapter_id: i64, pub manga_id: i64, pub title: String,
    pub chapter_number: f64, pub read_at: Option<i64>, pub read_duration: i64, pub cover: MangaCover,
}
pub struct AnimeHistory { pub id: i64, pub episode_id: i64, pub seen_at: Option<i64> }
pub struct AnimeHistoryUpdate { pub episode_id: i64, pub seen_at: i64 }
pub struct AnimeHistoryWithRelations {
    pub id: i64, pub episode_id: i64, pub anime_id: i64, pub title: String,
    pub episode_number: f64, pub seen_at: Option<i64>, pub cover: AnimeCover,
}
```

`MangaCover` / `AnimeCover` are owned by area:07-domain-entries-items (`kuro_core::models::{MangaCover, AnimeCover}` with `manga_id`, `source_id`, `is_manga_favorite`, `url: Option<String>`, `last_modified: i64`).

### 1.2 Updates

| Kotlin | Fields (in order) |
|---|---|
| `MangaUpdatesWithRelations` (`domain/src/main/java/tachiyomi/domain/updates/manga/model/MangaUpdatesWithRelations.kt`) | `mangaId: Long, mangaTitle: String, chapterId: Long, chapterName: String, scanlator: String?, read: Boolean, bookmark: Boolean, lastPageRead: Long, sourceId: Long, dateFetch: Long, coverData: MangaCover` |
| `AnimeUpdatesWithRelations` | `animeId, animeTitle, episodeId, episodeName, scanlator: String?, seen: Boolean, bookmark: Boolean, fillermark: Boolean, lastSecondSeen: Long, totalSeconds: Long, sourceId: Long, dateFetch: Long, coverData: AnimeCover` |

Positional mapper arguments (SQLite column order from `SELECT *` over the view):
* manga view columns (`updatesView.sq:1-21`): `mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch` -> mapper params `(mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, lastPageRead, sourceId, favorite, thumbnailUrl, coverLastModified, dateUpload, dateFetch)` (`MUR:41-55`). `dateUpload` is received but not stored in the domain object.
* anime view columns (`animeupdatesView.sq:1-23`): `animeId, animeTitle, episodeId, episodeName, scanlator, seen, bookmark, fillermark, last_second_seen, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch` (`AUR:45-61`).

### 1.3 Tracks

Domain `MangaTrack` (`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt`): `id: Long, mangaId: Long, trackerId: Long, remoteId: Long, libraryId: Long?, title: String, lastChapterRead: Double, totalChapters: Long, status: Long, score: Double, remoteUrl: String, startDate: Long, finishDate: Long, private: Boolean`. `AnimeTrack` identical with `animeId`, `lastEpisodeSeen`, `totalEpisodes`.

Mapper column order (`MangaTrackMapper.kt:6-20`): `id, mangaId, syncId, remoteId, libraryId, title, lastChapterRead, totalChapters, status, score, remoteUrl, startDate, finishDate, private` — this equals `SELECT *` order of `manga_sync` (`_id, manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, status, score, remote_url, start_date, finish_date, private`). Note the SQL `status` precedes `score`.

Rust:

```rust
pub struct MangaTrack {
    pub id: i64,              // <= 0 means not persisted (Kotlin uses -1 from toDomainTrack(idRequired=false) and 0 in forComparison)
    pub manga_id: i64, pub tracker_id: i64, pub remote_id: i64, pub library_id: Option<i64>,
    pub title: String, pub last_chapter_read: f64, pub total_chapters: i64,
    pub status: i64, pub score: f64, pub remote_url: String,
    pub start_date: i64, pub finish_date: i64, pub private: bool,
}
impl MangaTrack {
    pub fn new(tracker_id: i64) -> Self;                       // MangaTrack.create(serviceId): defaults below
    pub fn copy_personal_from(&mut self, other: &Self, copy_remote_private: bool);
}
```

`new` defaults (from `MangaTrackImpl.kt`): `id = 0`, `manga_id = 0`, `remote_id = 0`, `library_id = None`, `title = ""` (Kotlin leaves it uninitialised and crashes on read; Rust uses empty), numbers 0, `remote_url = ""`, `private = false`.

### 1.4 Sources

| Kotlin | Fields |
|---|---|
| `Source` (manga, `domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt`) | `id: Long, lang: String, name: String, supportsLatest: Boolean, isStub: Boolean, pin: Pins = Pins.unpinned, isUsedLast: Boolean = false, isExcludedFromDataSaver: Boolean = false`; `visualName = if lang.isEmpty() name else "$name (${lang.uppercase()})"`; `key() = if isUsedLast "$id-lastused" else "$id"` |
| `AnimeSource` (domain, `domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt`) | same without `isExcludedFromDataSaver` |
| `MangaSourceWithCount` | `source: Source`, `count: Long`; `id`, `name` accessors |
| `StubMangaSource` / `StubAnimeSource` | `id: Long, lang: String, name: String`; invalid when `name.isBlank() || lang.isBlank()` (then `toString()` = id); `supportsLatest = false` (anime) |
| `Pins`/`Pin` | `Pin.Unpinned = 0b00`, `Pin.Pinned = 0b01`, `Pin.Actual = 0b10`; `Pins.unpinned = Pins(0)`, `Pins.pinned = Pins(0b11)`; `contains(pin) = pin.code and code == pin.code`; `plus = or`; `minus = xor` (owned by area:09; `PinsBuilder.unaryMinus` has a bug: it ORs instead of clearing) |

Rust: `Source`, `SourceWithCount`, `StubSource { id: i64, lang: String, name: String }` in `kuro_core::data::source`; `Pins(u8)` owned by area:09.

### 1.5 Extension store (anime) — domain and wire

Domain `AnimeExtensionStore` (`domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt`): `indexUrl: String, name: String, badgeLabel: String, signingKey: String, contact: Contact(website: String, discord: String?), isLegacy: Boolean, extensionListUrl: String?`.

Wire — new format (`NES:12-56`), field numbers are protobuf tags, field names are JSON keys (kotlinx camelCase, no renaming):

| Message | Field | Proto tag | JSON key | Type | Required / default |
|---|---|---|---|---|---|
| `NetworkAnimeExtensionStore` | `name` | 1 | `name` | string | required |
| | `badgeLabel` | 2 | `badgeLabel` | string | required |
| | `signingKey` | 3 | `signingKey` | string | required |
| | `contact` | 4 | `contact` | message `Contact` | required |
| | `extensionList` | 101 | `extensionList` | message `ExtensionList` | optional (null) |
| | `extensionListUrl` | 102 | `extensionListUrl` | string | optional (null) |
| `Contact` | `website` | 1 | `website` | string | required |
| | `discord` | 2 | `discord` | string | optional |
| `ExtensionList` | `extensions` | 1 | `extensions` | repeated `Extension` | required |
| `Extension` | `name` | 1 | `name` | string | required |
| | `packageName` | 2 | `packageName` | string | required |
| | `resources` | 3 | `resources` | message `Resources` | required |
| | `extensionLib` | 4 | `extensionLib` | string (decimal such as "1.5") | required |
| | `versionCode` | 5 | `versionCode` | int64 (varint) | required |
| | `versionName` | 6 | `versionName` | string | required |
| | `contentWarning` | 7 | `contentWarning` | enum | required |
| | `isTorrent` | 8 | `isTorrent` | bool | default false |
| | `sources` | 9 | `sources` | repeated `Source` | required |
| `Resources` | `apkUrl` | 1 | `apkUrl` | string | required |
| | `iconUrl` | 2 | `iconUrl` | string | required |
| `Source` | `id` | 1 | `id` | int64 | required |
| | `name` | 2 | `name` | string | required |
| | `language` | 3 | `language` | string | required |
| | `homeUrl` | 4 | `homeUrl` | string | default "" |
| | `mirrorUrls` | 5 | `mirrorUrls` | repeated string | default empty |
| | `message` | 6 | `message` | string | optional |

`ContentWarning` (`NES:59-75`): `UNSPECIFIED = 0` (JSON alt `CONTENT_WARNING_UNSPECIFIED`), `SAFE = 1` (`CONTENT_WARNING_SAFE`), `MIXED = 2` (`CONTENT_WARNING_MIXED`), `NSFW = 3` (`CONTENT_WARNING_NSFW`). JSON accepts the enum constant name or the alt name. Ordering used by the app: `contentWarning >= MIXED` means NSFW.

JSON decoding configuration (`AppModule.kt`): `Json { ignoreUnknownKeys = true; explicitNulls = false }` -> unknown keys ignored, absent nullable keys are null. ProtoBuf: `ProtoBuf` default instance (kotlinx.serialization: varint int64 not zigzag, strings UTF-8, nested messages length-delimited, `ProtoNumber` tags).

Wire — legacy repo (`NLR:10-21`): `{"index_v2": string?, "meta": {"name": string, "shortName": string?, "website": string, "signingKeyFingerprint": string}}`.
Wire — legacy index entry (`NLE:10-26`): `{"name": string, "pkg": string, "apk": string, "lang": string, "code": long, "version": string, "nsfw": int, "sources": [{"id": long, "lang": string, "name": string, "baseUrl": string}]?}`.

Conversions to the domain store:

| From | `indexUrl` | `name` | `badgeLabel` | `signingKey` | `contact.website` | `contact.discord` | `isLegacy` | `extensionListUrl` |
|---|---|---|---|---|---|---|---|---|
| `NetworkAnimeExtensionStore.toExtensionStore(indexUrl)` | the fetched URL | `name` | `badgeLabel` | `signingKey` | `contact.website` | `contact.discord` | false | `extensionListUrl` |
| `NetworkLegacyAnimeExtensionRepo.toExtensionStore(indexUrl)` | the fetched URL (a `repo.json` URL) | `meta.name` | `meta.shortName ?: meta.name` | `meta.signingKeyFingerprint` | `meta.website` | null | true | null |
| `insertFromPreference(indexUrl, name)` | given | given | given | `"NO_SIGNING_KEY"` | `indexUrl` | null | false | null |

Mapping to `AnimeExtension.Available` (`NES:93-120`, `NLE:28-61`):

| Available field | New format | Legacy index entry |
|---|---|---|
| `name` | `extension.name` | `name.substringAfter("Aniyomi: ")` (returns the whole string when the prefix is absent) |
| `pkgName` | `packageName` | `pkg` |
| `apkUrl` | `resources.apkUrl` | `"$storeBaseUrl/apk/$apk"` |
| `iconUrl` | `resources.iconUrl` | `"$storeBaseUrl/icon/$pkg.png"` |
| `libVersion: Double` | `extensionLib.toDouble()` | `version.substringBeforeLast('.').toDouble()` |
| `versionCode: Long` | `versionCode` | `code` |
| `versionName` | `versionName` | `version` |
| `lang` | `if (distinct source languages).size == 1 then that language else "all"` | `lang` |
| `isNsfw` | `contentWarning >= MIXED` | `nsfw == 1` |
| `isTorrent` | `isTorrent` | false |
| `sources` | map `Source(id, name, language, homeUrl)` -> `Available.AnimeSource(id, name, lang = language, baseUrl = homeUrl)`; `mirrorUrls` and `message` are dropped | if `sources` is null or empty: one source `(id = 0, name = <extension name as given>, lang = lang, baseUrl = "")`; else map `(id, name, lang, baseUrl)` |
| `store` | the store passed in | the store passed in |

`AnimeExtension.Available` (`domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt`): `name, pkgName, versionName, versionCode: Long, libVersion: Double, lang: String, isNsfw, isTorrent, sources: List<AnimeSource(id: Long, lang, name, baseUrl)>, apkUrl, iconUrl, store: AnimeExtensionStore`; `AnimeSource.toStubSource()` = `StubAnimeSource(id, lang, name)`.

Rust:

```rust
pub struct ExtensionStore { pub index_url: String, pub name: String, pub badge_label: String, pub signing_key: String,
                            pub contact: Contact, pub is_legacy: bool, pub extension_list_url: Option<String> }
pub struct Contact { pub website: String, pub discord: Option<String> }
pub struct AvailableExtension { pub name: String, pub pkg_name: String, pub version_name: String, pub version_code: i64,
                                pub lib_version: f64, pub lang: String, pub is_nsfw: bool, pub is_torrent: bool,
                                pub sources: Vec<AvailableSource>, pub apk_url: String, pub icon_url: String,
                                pub store: ExtensionStore }
pub struct AvailableSource { pub id: i64, pub lang: String, pub name: String, pub base_url: String }
```

(`apk_url` keeps its name for wire parity; area:20-extension-strategy-redesign may rename the semantic to "artifact URL".)

### 1.6 Manga extension repo

`ExtensionRepo` (`domain/src/main/java/mihon/domain/extensionrepo/model/ExtensionRepo.kt`): `baseUrl: String, name: String, shortName: String?, website: String, signingKeyFingerprint: String`. `ExtensionRepoMetaDto(meta: ExtensionRepoDto(name, shortName: String?, website, signingKeyFingerprint))` -> `toExtensionRepo(baseUrl)` (`ExtensionRepoDto.kt`).

### 1.7 Custom button

`CustomButton` (`domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt`): `id: Long, name: String, isFavorite: Boolean, sortIndex: Long, content: String, longPressContent: String, onStartup: String`. `CustomButtonUpdate(id: Long, name: String? = null, isFavorite: Boolean? = null, sortIndex: Long? = null, content: String? = null, longPressContent: String? = null, onStartup: String? = null)`.

Placeholder expansion (all three accessors identical): `isPrimary = if (primaryId == id) "true" else "false"`; result = `text.replace("$id", id.toString()).replace("$isPrimary", isPrimary)` — `$id` is replaced first, everywhere it occurs as a substring (so `$identity` would become `1entity`).

### 1.8 Release

`Release(version: String, info: String, releaseLink: String, downloadLink: String)`. `GithubRelease(tag_name -> version, body -> info, html_url -> releaseLink, assets: List<GitHubAsset(name, browser_download_url -> downloadLink)>)`. `GetApplicationRelease.Arguments(isPreview: Boolean, commitCount: Int, versionName: String, repository: String, forceCheck: Boolean = false)`; `Result = NewUpdate(release) | NoNewUpdate | OsTooOld`.

## 2. Preference keys touched

| Key (exact) | Type | Default | Owner / use | Source |
|---|---|---|---|---|
| `__APP_STATE_last_app_check` (`Preference.appStateKey("last_app_check")`, prefix constant `"__APP_STATE_"`) | Long (epoch ms) | 0 | `GetApplicationRelease.lastChecked`; set only when a release was fetched | `domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt:13-16` |
| `last_ext_check` (plain key; a migration renames old values to `__APP_STATE_last_ext_check` but the anime/manga extension APIs still read the plain key) | Long | 0 | `AnimeExtensionApi`/`MangaExtensionApi` 1-day throttle | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt:26-28`, `mihon/core/migration/migrations/NoAppStateMigration.kt:17-30` |
| `__APP_STATE_incognito_mode` | Boolean | false | Read at history-record time (history is not written when true) | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21` |
| `anime_extension_repos` | StringSet | empty | Legacy; read once by `TrustExtensionRepositoryMigration` then deleted | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:40` |
| `extension_repos` | StringSet | empty | Legacy manga repo URLs; same migration | `SourcePreferences.kt:42` |
| `__APP_STATE_library_update_last_timestamp` | Long | 0 | Shown on the Updates tab (area:42/26) | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:39` |

Proposal: all of these live in the preference registry of area:02-preferences-system (`kuro_core::prefs`); this area reads/writes only `last_app_check`, `last_ext_check`, `incognito_mode`.

## 3. SQL quoted verbatim (what the Rust repositories execute)

### 3.1 History (manga) — `data/src/main/sqldelight/data/history.sq`

```sql
CREATE TABLE history(
    _id INTEGER NOT NULL PRIMARY KEY,
    chapter_id INTEGER NOT NULL UNIQUE,
    last_read INTEGER AS Date,
    time_read INTEGER NOT NULL,
    FOREIGN KEY(chapter_id) REFERENCES chapters (_id)
    ON DELETE CASCADE
);
CREATE INDEX history_history_chapter_id_index ON history(chapter_id);

getHistoryByMangaId:
SELECT H._id, H.chapter_id, H.last_read, H.time_read
FROM history H JOIN chapters C ON H.chapter_id = C._id
WHERE C.manga_id = :mangaId AND C._id = H.chapter_id;

getHistoryByChapterUrl:
SELECT H._id, H.chapter_id, H.last_read, H.time_read
FROM history H JOIN chapters C ON H.chapter_id = C._id
WHERE C.url = :chapterUrl AND C._id = H.chapter_id;

resetHistoryById:    UPDATE history SET last_read = 0 WHERE _id = :historyId;
resetHistoryByMangaId:
UPDATE history SET last_read = 0
WHERE _id IN (
    SELECT H._id FROM mangas M
    INNER JOIN chapters C ON M._id = C.manga_id
    INNER JOIN history H ON C._id = H.chapter_id
    WHERE M._id = :mangaId
);
removeAllHistory:       DELETE FROM history;
removeResettedHistory:  DELETE FROM history WHERE last_read = 0;
upsert:
INSERT INTO history(chapter_id, last_read, time_read)
VALUES (:chapterId, :readAt, :time_read)
ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read
WHERE chapter_id = :chapterId;
getReadDuration:        SELECT coalesce(sum(time_read), 0) FROM history;
```

Anime (`data/src/main/sqldelightanime/dataanime/animehistory.sq`): table `animehistory(_id, episode_id UNIQUE, last_seen INTEGER AS Date)` + index `animehistory_history_episode_id_index`; queries `getHistoryByAnimeId` (join `episodes EP`, `WHERE EP.anime_id = :animeId`), `getHistoryByEpisodeUrl`, `resetAnimeHistoryById`, `resetHistoryByAnimeId`, `removeAllHistory`, `removeResettedHistory` (`WHERE last_seen = 0`), `upsert(episode_id, last_seen)` with `DO UPDATE SET last_seen = :seenAt`.

`historyView` (`view/historyView.sq:1-27`):

```sql
CREATE VIEW historyView AS
SELECT history._id AS id, mangas._id AS mangaId, chapters._id AS chapterId, mangas.title,
       mangas.thumbnail_url AS thumbnailUrl, mangas.source, mangas.favorite, mangas.cover_last_modified,
       chapters.chapter_number AS chapterNumber, history.last_read AS readAt, history.time_read AS readDuration,
       max_last_read.last_read AS maxReadAt, max_last_read.chapter_id AS maxReadAtChapterId
FROM mangas
JOIN chapters ON mangas._id = chapters.manga_id
JOIN history ON chapters._id = history.chapter_id
JOIN (
    SELECT chapters.manga_id, chapters._id AS chapter_id, MAX(history.last_read) AS last_read
    FROM chapters JOIN history ON chapters._id = history.chapter_id
    GROUP BY chapters.manga_id
) AS max_last_read ON chapters.manga_id = max_last_read.manga_id;

history:
SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration
FROM historyView
WHERE historyView.readAt > 0
AND maxReadAtChapterId = historyView.chapterId
AND lower(historyView.title) LIKE ('%' || :query || '%')
ORDER BY readAt DESC;

getLatestHistory:
SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration
FROM historyView WHERE historyView.readAt > 0 ORDER BY readAt DESC LIMIT 1;
```

`animehistoryView` is the same with `animehistory`, `episodes`, `episodeNumber`, `seenAt`, `maxSeenAt`, `maxSeenAtEpisodeId`, no duration (`view/animehistoryView.sq:1-61`); its two queries are named `animehistory` and `getLatestAnimeHistory` and select `id, animeId, episodeId, title, thumbnailUrl, source, favorite, cover_last_modified, episodeNumber, seenAt`.

Mapper column order: manga `(historyId, mangaId, chapterId, title, thumbnailUrl, sourceId, isFavorite, coverLastModified, chapterNumber, readAt, readDuration)` (`MangaHistoryMapper.kt:21-32`); anime `(historyId, animeId, episodeId, title, thumbnailUrl, sourceId, isFavorite, coverLastModified, episodeNumber, seenAt)` (`AnimeHistoryMapper.kt:19-30`).

### 3.2 Updates — see `p1-topic-history-and-updates-views.md` section 4 for the view and queries (quoted from `updatesView.sq`, `animeupdatesView.sq`).

### 3.3 Tracks

```sql
-- manga_sync.sq (anime_sync.sq is identical with anime_id / last_episode_seen / total_episodes)
CREATE TABLE manga_sync(
    _id INTEGER NOT NULL PRIMARY KEY, manga_id INTEGER NOT NULL, sync_id INTEGER NOT NULL, remote_id INTEGER NOT NULL,
    library_id INTEGER, title TEXT NOT NULL, last_chapter_read REAL NOT NULL, total_chapters INTEGER NOT NULL,
    status INTEGER NOT NULL, score REAL NOT NULL, remote_url TEXT NOT NULL, start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL, private INTEGER AS Boolean DEFAULT 0 NOT NULL,
    UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);
delete:             DELETE FROM manga_sync WHERE manga_id = :mangaId AND sync_id = :syncId;
getTracks:          SELECT * FROM manga_sync;
getTrackById:       SELECT * FROM manga_sync WHERE _id = :id;
getTracksByMangaId: SELECT * FROM manga_sync WHERE manga_id = :mangaId;
insert:             INSERT INTO manga_sync(manga_id,sync_id,remote_id,library_id,title,last_chapter_read,total_chapters,status,score,remote_url,start_date,finish_date,private)
                    VALUES (:mangaId,:syncId,:remoteId,:libraryId,:title,:lastChapterRead,:totalChapters,:status,:score,:remoteUrl,:startDate,:finishDate,:private);
update:             UPDATE manga_sync SET manga_id = coalesce(:mangaId, manga_id), sync_id = coalesce(:syncId, sync_id),
                    remote_id = coalesce(:mediaId, remote_id), library_id = coalesce(:libraryId, library_id), title = coalesce(:title, title),
                    last_chapter_read = coalesce(:lastChapterRead, last_chapter_read), total_chapters = coalesce(:totalChapter, total_chapters),
                    status = coalesce(:status, status), score = coalesce(:score, score), remote_url = coalesce(:trackingUrl, remote_url),
                    start_date = coalesce(:startDate, start_date), finish_date = coalesce(:finishDate, finish_date), private = coalesce(:private, private)
                    WHERE _id = :id;
```

Anime names: `getAnimeTracks`, `getTrackByAnimeId` (`WHERE _id = :id`), `getTracksByAnimeId`. Both `update` statements take 14 parameters in this order: entry id, sync id, media id, library id, title, last read/seen, total, status, score, tracking url, start date, finish date, private, row id.

### 3.4 Sources

```sql
-- sources.sq (manga DB) / animesources.sq (anime DB: table animesources)
CREATE TABLE sources(_id INTEGER NOT NULL PRIMARY KEY, lang TEXT NOT NULL, name TEXT NOT NULL);
findAll:  SELECT * FROM sources;
findOne:  SELECT * FROM sources WHERE _id = :id;
upsert:   INSERT INTO sources(_id, lang, name) VALUES (:id, :lang, :name)
          ON CONFLICT(_id) DO UPDATE SET lang = :lang, name = :name WHERE _id = :id;
-- mangas.sq:103-109, 141-145
getSourceIdWithFavoriteCount:      SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source;
getSourceIdsWithNonLibraryManga:   SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source;
-- animes.sq:114-120
getAnimeSourceIdWithFavoriteCount: SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source;
```

### 3.5 Extension stores and repos

```sql
-- extension_store.sq (anime DB)
CREATE TABLE extension_store(
    index_url TEXT NOT NULL PRIMARY KEY, name TEXT NOT NULL, badge_label TEXT NOT NULL, signing_key TEXT NOT NULL,
    contact_website TEXT NOT NULL, contact_discord TEXT, is_legacy INTEGER AS Boolean NOT NULL, extension_list_url TEXT);
get:     SELECT * FROM extension_store WHERE index_url = :indexUrl;
getAll:  SELECT * FROM extension_store;
getCount:SELECT COUNT(*) FROM extension_store;
upsert:  INSERT INTO extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url)
         VALUES (:indexUrl, :name, :badgeLabel, :signingKey, :contactWebsite, :contactDiscord, :isLegacy, :extensionListUrl)
         ON CONFLICT(index_url) DO UPDATE SET name = :name, badge_label = :badgeLabel, signing_key =: signingKey,
         contact_website = :contactWebsite, contact_discord = :contactDiscord, is_legacy = :isLegacy, extension_list_url = :extensionListUrl
         WHERE index_url = :indexUrl;
delete:  DELETE FROM extension_store WHERE index_url = :indexUrl;
-- migration 137.sqm: repos -> stores
INSERT INTO extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url)
SELECT base_url || '/repo.json', name, coalesce(short_name, name), signing_key_fingerprint, website, NULL, 1, NULL FROM extension_repos;
DROP TABLE extension_repos;

-- extension_repos.sq (manga DB)
CREATE TABLE extension_repos (base_url TEXT NOT NULL PRIMARY KEY, name TEXT NOT NULL, short_name TEXT, website TEXT NOT NULL,
                              signing_key_fingerprint TEXT UNIQUE NOT NULL);
findOne:  SELECT * FROM extension_repos WHERE base_url = :base_url;
findOneBySigningKeyFingerprint: SELECT * FROM extension_repos WHERE signing_key_fingerprint = :fingerprint;
findAll:  SELECT * FROM extension_repos;     count: SELECT COUNT(*) FROM extension_repos;
insert:   INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint);
upsert:   INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint)
          ON CONFLICT(base_url) DO UPDATE SET name = :name, short_name = :short_name, website =: website, signing_key_fingerprint = :fingerprint WHERE base_url = base_url;
replace:  INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint)
          ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name = :name, short_name = :short_name, website =: website WHERE signing_key_fingerprint = signing_key_fingerprint;
delete:   DELETE FROM extension_repos WHERE base_url = :base_url;
```

Note: the anime DB's migration 128 created `extension_repos` and 137 turned it into `extension_store` (`data/src/main/sqldelightanime/migrations/128.sqm`, `137.sqm`), so the anime DB has no `extension_repos` table any more.

### 3.6 Custom buttons — `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`

```sql
CREATE TABLE custom_buttons (
    _id INTEGER NOT NULL PRIMARY KEY, name TEXT NOT NULL, isFavorite INTEGER AS Boolean NOT NULL,
    sortIndex INTEGER NOT NULL, content TEXT NOT NULL, longPressContent TEXT NOT NULL, onStartup TEXT NOT NULL);
INSERT OR IGNORE INTO custom_buttons (_id, name, isFavorite, sortIndex, content, longPressContent, onStartup) VALUES (1, '+85 s', 1, 0, <content>, <longPressContent>, <onStartup>);
findAll:  SELECT * FROM custom_buttons ORDER BY sortIndex;
insert:   INSERT INTO custom_buttons(name, isFavorite, sortIndex, content, longPressContent, onStartup) VALUES (:name, :isFavorite, :sortIndex, :content, :longPressContent, :onStartup);
delete:   DELETE FROM custom_buttons WHERE _id = :customButtonId;
update:   UPDATE custom_buttons SET name = coalesce(:name, name), isFavorite = coalesce(:isFavorite, isFavorite), sortIndex = coalesce(:sortIndex, sortIndex),
          content = coalesce(:content, content), longPressContent = coalesce(:longPressContent, longPressContent), onStartup = coalesce(:onStartup, onStartup)
          WHERE _id = :customButtonId;
selectLastInsertedRowId: SELECT last_insert_rowid();
```

The three seed texts are reproduced byte for byte in `p1-topic-custom-buttons.md` section 2.

## 4. Network endpoints (this area)

| Endpoint | Method | Params / headers | Auth | Response | Source |
|---|---|---|---|---|---|
| `<indexUrl>` of an extension store (any URL the user typed; new format) | GET | default client headers (area:12); `Accept-Encoding` handled by OkHttp transparently, the body may additionally be gzip-compressed *payload* (magic `1f 8b`) | none | JSON object, protobuf message, or legacy JSON array/object (sniffed by first byte, `p1-topic-extension-store-index.md` section 3) | `ESS:28-68` |
| `<indexUrl with "/index.min.json" replaced by "/repo.json">` | GET | as above | none | `NetworkLegacyAnimeExtensionRepo` JSON | `ESS:39-43` |
| `<index_v2>` from a legacy `repo.json` | GET (recursive `fetch`) | as above | none | any store format | `ESS:53-55` |
| `<store.extensionListUrl>` | GET | as above | none | `ExtensionList` JSON (`{`) or protobuf; gzip allowed | `ESS:72-83` |
| `<store.indexUrl>` (non-legacy store without extensionListUrl) | GET | as above | none | full store with inline `extensionList` | `ESS:84-94` |
| `<storeBaseUrl>/index.min.json` (legacy store; `storeBaseUrl = indexUrl.removeSuffix("/repo.json")`) | GET | as above | none | JSON array of `NetworkLegacyAnimeExtension` (no gzip detection) | `ESS:95-101` |
| `<baseUrl>/repo.json` (manga repos) | GET | as above | none | `{"meta": {"name": string, "shortName": string?, "website": string, "signingKeyFingerprint": string}}` | `domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt:19-27` |
| `https://api.github.com/repos/<owner>/<repo>/releases/latest` | GET | default headers (the shared client sets a User-Agent, area:12); no `Accept` override; unauthenticated (60 requests/hour/IP limit applies; the 3-day throttle makes this irrelevant) | none | `GithubRelease` JSON; non-2xx = failure | `RSI:18-35` |
| Extension artifact / icon URLs from the index | GET | handled by area:19/20 | none | n/a | out of scope |

Failure handling: `awaitSuccess()` raises `HttpException(code)` for any non-2xx (area:12); store/repo code converts every exception into `Result.failure`/null and logs at ERROR; there is no retry, no backoff, no timeout override (client defaults: area:12).

## 5. Files and folders on disk

| What | Kotlin location | Rust location (Proposal) | Notes |
|---|---|---|---|
| Manga DB | `tachiyomi.db` in the app databases dir (+ `-wal`, `-shm`) | `paths.data()/manga.db` | opened by `Db::open`; schema area:03 |
| Anime DB | `tachiyomi.animedb` | `paths.data()/anime.db` | schema area:04; holds `extension_store`, `custom_buttons`, `animesources` |
| Generated player script | `<filesDir>/mpv/scripts/custombuttons.lua` (`MPV_DIR`, `MPV_SCRIPTS_DIR` constants; `PlayerActivity.kt:535-580`) | `paths.config()/mpv/scripts/custombuttons.lua` (regenerated on every player start; area:45 owns the mpv config dir) | overwritten each time (`createFile` replaces) |
| Release asset | none persisted by this area | none | the APK download is area:35 |

No cache files are written by this area; no settings are written except the two timestamp preferences in section 2.

## 6. DB tables touched (summary)

Read/write: `history`, `animehistory`, `manga_sync`, `anime_sync`, `sources`, `animesources`, `extension_repos` (manga DB), `extension_store` (anime DB), `custom_buttons`. Read only: `mangas`, `chapters`, `animes`, `episodes` (through `historyView`, `animehistoryView`, `updatesView`, `animeupdatesView` and the favourite-count queries). Cascades into this area: deleting a `chapters`/`episodes` row deletes its history row; deleting a `mangas`/`animes` row deletes its track rows.
