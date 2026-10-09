# area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin: data models, config, endpoints

Source root: `/home/blackbox/aniyomi-src`. Labels: **OBSERVED** (from Kotlin), **Proposal:** (Rust), **UPSTREAM** (read-only check of the server repository). Nullability: Kotlin `T?` = nullable. "req" = non-null with no default, so the JSON key must be present. "nullable" = may be absent or `null` (the app's `Json` has `explicitNulls = false`, so a missing nullable key decodes to `null`). Shared `Json` config: `ignoreUnknownKeys = true`, `explicitNulls = false` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:173-178`).

---

## 1. Domain and DB track models (OBSERVED)

These are the shapes the four trackers read and write. The full models are owned by area:27-tracking-core.

### 1.1 `MangaTrack` (DB model, `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt`)

| Field | Kotlin type | Used by these trackers | Notes |
|---|---|---|---|
| `id` | `Long?` | yes (row id) | `null` before insert |
| `manga_id` | `Long` | yes | set in `bindEnhancedTrackers` (`AddMangaTracks.kt:89`) |
| `tracker_id` | `Long` | yes | 8 (Kavita), 6 (Komga), 9 (Suwayomi) |
| `remote_id` | `Long` | **never set** by these trackers, stays `0` | `MangaTrackSearch.kt` default `0` |
| `library_id` | `Long?` | never set, stays `null` | |
| `title` | `String` (lateinit) | yes | from the search result |
| `last_chapter_read` | `Double` | yes | |
| `total_chapters` | `Long` | yes | |
| `score` | `Double` | copied by `copyPersonalFrom`; search default `-1.0` | D-03 |
| `status` | `Long` | yes | see section 3 |
| `started_reading_date` | `Long` | copied by `copyPersonalFrom`; search default `0` | D-03 |
| `finished_reading_date` | `Long` | same | D-03 |
| `tracking_url` | `String` (lateinit) | yes | Kavita/Komga/Suwayomi: resource URL. Jellyfin: anime variant |
| `private` | `Boolean` | copied; search default `false` | D-03 |

### 1.2 `AnimeTrack` (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt`)

Same as 1.1 with `anime_id`, `last_episode_seen: Double`, `total_episodes: Long`, `started_watching_date: Long`, `finished_watching_date: Long`. `copyPersonalFrom` (`AnimeTrack.kt:37-44`) copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, and `private`.

### 1.3 Search results (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt`, `AnimeTrackSearch.kt`)

Base fields (defaults): `id: Long? = null`, `manga_id: Long = 0`, `tracker_id: Long = 0`, `remote_id: Long = 0`, `library_id: Long? = null`, `title: lateinit String`, `last_chapter_read: Double = 0.0`, `total_chapters: Long = 0`, `score: Double = -1.0`, `status: Long = 0`, `started_reading_date: Long = 0`, `finished_reading_date: Long = 0`, `private: Boolean = false`, `tracking_url: lateinit String`.

Extra fields: `authors: List<String> = emptyList()`, `artists: List<String> = emptyList()`, `cover_url: String = ""`, `summary: String = ""`, `publishing_status: String = ""`, `publishing_type: String = ""`, `start_date: String = ""`.

`equals`/`hashCode` (`MangaTrackSearch.kt:51-69`) use `manga_id`, `tracker_id`, `remote_id` only. `create(serviceId)` sets `tracker_id` (`:72-74`).

Fields set by each tracker on its search result:

| Field | Kavita | Komga series | Komga readlist | Suwayomi | Jellyfin |
|---|---|---|---|---|---|
| `title` | `name` | `metadata.title` | `name` | `title` | `name` |
| `summary` | `""` (`KavitaModels.kt:27`) | `metadata.summary` | unset (`""`) | `description ?: ""` | unset |
| `cover_url` | `thumbnail_url.toString()` | `"$url/thumbnail"` | `"$url/thumbnail"` | `"$url/thumbnail"` | unset |
| `publishing_status` | unset | `metadata.status` | unset | `status` | unset |
| `tracking_url` | the series URL | the series or readlist URL | same | the `"$baseUrl/api/v1/manga/{id}"` URL or the given track URL | the item URL with fragment |
| `total_chapters` / `total_episodes` | volumes walk | `maxNumberSort.toLong()` | same | `chapterCount` | `1`, or episodes' last `IndexNumber` for a season |
| `last_chapter_read` / `last_episode_seen` | latest chapter (204 → 0.0) | `lastReadContinuousNumberSort` | same | `lastChapterRead?.chapterNumber ?: 0.0` | `1.0` or `0.0` for a movie; continuous rule for a season |
| `status` | pages rule | booksCount rule | same | unreadCount rule | played flag / continuous rule |

---

## 2. DTO field tables (OBSERVED)

Kotlin property names are the JSON keys when there is no `@SerialName`. For the Kavita DTO that means `thumbnail_url`, `stringResourcedName` and `pagesRead` in camelCase or snake_case as written. Every key is compared exactly.

### 2.1 Kavita (`app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaModels.kt`)

`SeriesDto` (`:8-23`):

| Kotlin property | Type | Nullability / default | Note |
|---|---|---|---|
| `id` | `Int` | req | |
| `name` | `String` | req | upstream sends nullable; a `null` breaks parsing |
| `originalName` | `String` | default `""` | |
| `thumbnail_url` | `String?` | default `""` | not sent by upstream (D-01) |
| `stringResourcedName` | `String?` | default `""` | |
| `sortName` | `String?` | default `""` | |
| `pages` | `Int` | req | |
| `coverImageLocked` | `Boolean` | default `true` | |
| `pagesRead` | `Int` | req | |
| `userRating` | `Int?` | default `0` | |
| `userReview` | `String?` | default `""` | |
| `format` | `Int` | req | |
| `created` | `String?` | default `""` | |
| `libraryId` | `Int` | req | |
| `libraryName` | `String?` | default `""` | |

`VolumeDto` (`:32-43`): `id: Int` (req), `number: Int` (req), `name: String` (req), `pages: Int` (req), `pagesRead: Int` (req), `lastModified: String` (req), `created: String` (req), `seriesId: Int` (req), `chapters: List<ChapterDto> = emptyList()`.

`ChapterDto` (`:45-57`), every field nullable with a default:
`id: Int? = -1`, `range: String? = ""`, `number: String? = "-1"`, `pages: Int? = 0`, `isSpecial: Boolean? = false`, `title: String? = ""`, `pagesRead: Int? = 0`, `coverImageLocked: Boolean? = false`, `volumeId: Int? = -1`, `created: String? = ""`.

`AuthenticationDto` (`:59-63`): `username: String` (req), `token: String` (req), `apiKey: String` (req).

`OAuth` (`:65-70`): `authentications: List<SourceAuth> = listOf(SourceAuth(1), SourceAuth(2), SourceAuth(3))`.

`SourceAuth` (`:82-86`): `sourceId: Int` (var, no default, passed explicitly), `apiUrl: String = ""`, `jwtToken: String = ""`.

### 2.2 Komga (`app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaModels.kt`)

`SeriesDto` (`:6-20`): `id: String` (req), `libraryId: String` (req), `name: String` (req), `created: String?` (nullable), `lastModified: String?` (nullable), `fileLastModified: String` (req), `booksCount: Int` (req), `booksReadCount: Int` (req), `booksUnreadCount: Int` (req), `booksInProgressCount: Int` (req), `metadata: SeriesMetadataDto` (req), `booksMetadata: BookMetadataAggregationDto` (req).

`SeriesMetadataDto` (`:22-43`): `status: String` (req), `created: String?`, `lastModified: String?`, `title: String` (req), `titleSort: String` (req), `summary: String` (req), `summaryLock: Boolean` (req), `readingDirection: String` (req), `readingDirectionLock: Boolean` (req), `publisher: String` (req), `publisherLock: Boolean` (req), `ageRating: Int?` (nullable), `ageRatingLock: Boolean` (req), `language: String` (req), `languageLock: Boolean` (req), `genres: Set<String>` (req), `genresLock: Boolean` (req), `tags: Set<String>` (req), `tagsLock: Boolean` (req).

`BookMetadataAggregationDto` (`:45-54`): `authors: List<AuthorDto> = emptyList()`, `releaseDate: String?` (nullable), `summary: String` (req), `summaryNumber: String` (req), `created: String` (req), `lastModified: String` (req).

`AuthorDto` (`:56-60`): `name: String`, `role: String` (both req).

`ReadProgressUpdateDto` (`:62-65`): `lastBookRead: Int` (req). Serialised as `{"lastBookRead": N}`.

`ReadProgressUpdateV2Dto` (`:67-70`): `lastBookNumberSortRead: Double` (req). Serialised as `{"lastBookNumberSortRead": 12.0}`. Kotlin `Double` encoding uses `Double.toString()` rules (so `12.0`, `1.5`, `1.0E-4`).

`ReadListDto` (`:72-80`): `id: String`, `name: String`, `bookIds: List<String>`, `createdDate: String`, `lastModifiedDate: String`, `filtered: Boolean`. All req.

`ReadProgressDto` (`:82-98`): `booksCount: Int`, `booksReadCount: Int`, `booksUnreadCount: Int`, `booksInProgressCount: Int`, `lastReadContinuousIndex: Int`. All req. `toV2()` = `ReadProgressV2Dto(booksCount, booksReadCount, booksUnreadCount, booksInProgressCount, lastReadContinuousIndex.toDouble(), booksCount.toFloat())` (`:89-99`).

`ReadProgressV2Dto` (`:100-107`): `booksCount: Int`, `booksReadCount: Int`, `booksUnreadCount: Int`, `booksInProgressCount: Int`, `lastReadContinuousNumberSort: Double`, `maxNumberSort: Float`. All req.

UPSTREAM (`komga/src/main/kotlin/org/gotson/komga/interfaces/api/rest/SeriesController.kt:749-801`, `ReadListController.kt:411-438`): the v2 PUT body is `TachiyomiReadProgressUpdateV2Dto` (`lastBookNumberSortRead`). The readlist PUT body is `TachiyomiReadProgressUpdateDto` (`lastBookRead`, `@Valid`). Both responses are `204`.

### 2.3 Suwayomi (`app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiModels.kt`)

`SourceDataClass` (`:5-23`): `id: String`, `name: String`, `lang: String`, `iconUrl: String`, `supportsLatest: Boolean`, `isConfigurable: Boolean`, `isNsfw: Boolean`, `displayName: String`. All req. Comments at `:12-21`. The tracker never reads it. It is embedded in `MangaDataClass.source`.

`MangaDataClass` (`:25-59`), all req unless marked:

| Property | Type | Nullability |
|---|---|---|
| `id` | `Int` | req |
| `sourceId` | `String` | req |
| `url` | `String` | req |
| `title` | `String` | req |
| `thumbnailUrl` | `String?` | nullable |
| `initialized` | `Boolean` | req |
| `artist` | `String?` | nullable |
| `author` | `String?` | nullable |
| `description` | `String?` | nullable |
| `genre` | `List<String>` | req |
| `status` | `String` | req |
| `inLibrary` | `Boolean` | req |
| `inLibraryAt` | `Long` | req |
| `source` | `SourceDataClass?` | nullable |
| `meta` | `Map<String, String>` | req |
| `realUrl` | `String?` | nullable |
| `lastFetchedAt` | `Long?` | nullable |
| `chaptersLastFetchedAt` | `Long?` | nullable |
| `freshData` | `Boolean` | req |
| `unreadCount` | `Long?` | nullable |
| `downloadCount` | `Long?` | nullable |
| `chapterCount` | `Long` | req (comment: "actually is nullable server side") |
| `lastChapterRead` | `ChapterDataClass?` | nullable |
| `age` | `Long?` | nullable |
| `chaptersAge` | `Long?` | nullable |

`ChapterDataClass` (`:61-100`):

| Property | Type | Nullability | Comment in source |
|---|---|---|---|
| `id` | `Int` | req | |
| `url` | `String` | req | |
| `name` | `String` | req | |
| `uploadDate` | `Long` | req | |
| `chapterNumber` | `Double` | req | |
| `scanlator` | `String?` | nullable | |
| `mangaId` | `Int` | req | |
| `read` | `Boolean` | req | "chapter is read" |
| `bookmarked` | `Boolean` | req | "chapter is bookmarked" |
| `lastPageRead` | `Int` | req | "zero means not read/no data" |
| `lastReadAt` | `Long` | req | |
| `index` | `Int` | req | "this chapter's index, starts with 1" |
| `fetchedAt` | `Long` | req | |
| `downloaded` | `Boolean` | req | |
| `pageCount` | `Int` | req | |
| `chapterCount` | `Int?` | nullable | |
| `meta` | `Map<String, String>` | req | |

Because `MangaDataClass` has many required fields, a server response that omits one of them makes `GET …/full` fail for the whole tracker (Proposal: ignore unknown keys and default missing optional fields, but keep the required set documented).

### 2.4 Jellyfin (`app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/dto/JFItem.kt`)

`JFItem` (`:7-12`): `@SerialName("Name") name: String` (req), `@SerialName("Id") id: String` (req), `@SerialName("UserData") userData: JFUserData` (req), `@SerialName("IndexNumber") indexNumber: Long? = null`.

`JFUserData` (`:15-17`): `@SerialName("Played") played: Boolean` (req).

`JFItemList` (`:20-22`): `@SerialName("Items") items: List<JFItem>` (req).

---

## 3. Status and enum values (OBSERVED)

| Tracker | Kotlin constant | Value | Label | Stored as |
|---|---|---|---|---|
| Kavita | `UNREAD` | 1 | `MR.strings.unread` | `manga_sync.status` |
| Kavita | `READING` | 2 | `MR.strings.reading` | |
| Kavita | `COMPLETED` | 3 | `MR.strings.completed` | |
| Komga | `UNREAD` / `READING` / `COMPLETED` | 1 / 2 / 3 | as Kavita | |
| Suwayomi | `UNREAD` / `READING` / `COMPLETED` | 1 / 2 / 3 | as Kavita | |
| Jellyfin | `UNSEEN` | 1 | `AYMR.strings.unseen` | `anime_sync.status` |
| Jellyfin | `WATCHING` | 2 | `AYMR.strings.watching` | |
| Jellyfin | `COMPLETED` | 3 | `MR.strings.completed` | |
| All four | rereading / rewatching | -1 | not in list | |

Stored values are written by `anime_sync` / `manga_sync` as `INTEGER` (`status INTEGER NOT NULL`). These numbers are not shared with other trackers, so a Kavita status `2` is not AniList status `2`.

Upstream note: Kavita's `SeriesDto.Format` is of type `MangaFormat` (`Kavita.Models/DTOs/SeriesDto.cs:63`). The Kotlin `format: Int` is kept as is. Whether the server sends the enum as an integer was not verified; if it sends a string, parsing `SeriesDto` fails (open question).

---

## 4. Source id algorithm and worked vectors (OBSERVED; same algorithm as area:15-source-api-manga, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:87-91`)

```
key      = "<name>/<lang>/<versionId>"           (Kavita, Jellyfin: lang "all", versionId 1)
bytes    = MD5(UTF-8(key))                         (16 bytes)
id       = OR over i=0..7 of ((bytes[i] & 0xff) << (8 * (7 - i)))     (big-endian, first 8 bytes)
id       = id & 0x7FFF_FFFF_FFFF_FFFF              (sign bit cleared)
```

Kotlin precedence: `bytes[it].toLong() and 0xff shl 8 * (7 - it)` = `((bytes[it].toLong() and 0xff) shl (8 * (7 - it)))`, because the infix functions `and` and `shl` share precedence and are left-associative, and `*` binds tighter.

Worked vectors (computed with Python `hashlib.md5`, verified by this algorithm):

| Key | Used by | Id |
|---|---|---|
| `kavita_1/all/1` | `Kavita.kt:122-129` slot 1 | 7266875667263635239 |
| `kavita_2/all/1` | slot 2 | 3764816291200082561 |
| `kavita_3/all/1` | slot 3 | 778637366235559582 |
| `jellyfin/all/1` | `JellyfinInterceptor.kt:44-49`, slot 1 | 1100359934660540567 |
| `jellyfin (2)/all/1` | slot 2 | 5716273013801838310 |
| `jellyfin (3)/all/1` | slot 3 | 7066619062139039107 |
| `jellyfin (10)/all/1` | slot 10 | 3559930647551045450 |
| `tachidesk/en/1` | `SuwayomiApi.kt:103-107` | 3100117499901280806 |

Note that the Kavita key is `kavita_<n>/all/1` (underscore, not a space), and the Jellyfin key uses a space and parentheses for slots 2 to 10. The Suwayomi key is `tachidesk/en/1`. The Kotlin uses these keys literally, so the port must too.

---

## 5. Preference keys (OBSERVED)

### 5.1 Source preference files (external extensions, read by the trackers)

Each file is named `source_<id>` (`ConfigurableSource.kt:20-22`, `ConfigurableAnimeSource.kt:25-27`) and opened with `Context.MODE_PRIVATE`.

| Source | Source id key | Key | Type | Default | Read at | Written by |
|---|---|---|---|---|---|---|
| Kavita slot n (1..3) | `kavita_<n>/all/1` | `APIURL` | String | `""` | `Kavita.kt:130` | the Kavita extension settings ("Address"). Must end in `/api` (inferred: `Kavita.kt:130` + `KavitaApi.kt:43`) |
| Kavita slot n | same | `APIKEY` | String | `""` | `Kavita.kt:131` | the Kavita extension settings (API key) |
| Jellyfin slot n (1..10) | `jellyfin[ (n)]/all/1` | `user_id` | String | `""` | `JellyfinInterceptor.kt:55` | Jellyfin extension |
| Jellyfin slot n | same | `api_key` | String | `""` | `JellyfinInterceptor.kt:62` | Jellyfin extension |
| Suwayomi | `tachidesk/en/1` | `Server URL Address` | String | `""` | `SuwayomiApi.kt:113` | Tachidesk extension settings |
| Suwayomi | same | `Login (Basic Auth)` | String | `""` | `SuwayomiApi.kt:114` | Tachidesk extension settings |
| Suwayomi | same | `Password (Basic Auth)` | String | `""` | `SuwayomiApi.kt:115-118` | Tachidesk extension settings |

Notes:
- These preference files are owned by external extensions (not in this repo). `catalogs/preferences-01.md:32-42` and `:85-90` describe the same keys.
- The Rust port reads them through the per-source preference store of area:02-preferences-system (`source_prefs[(source_key, name)]`, see `catalogs/preferences-01.md:85-86`).

### 5.2 Tracker credential keys (`app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`, `TrackPreferences`)

| Key | Type | Default | Value for enhanced trackers | Source |
|---|---|---|---|---|
| `__PRIVATE_pref_mangasync_username_<trackerId>` | String | `""` | `"user"` after `loginNoop()` | `catalogs/preferences-06.md:170` |
| `__PRIVATE_pref_mangasync_password_<trackerId>` | String | `""` | `"pass"` after `loginNoop()` | `catalogs/preferences-06.md:171` |

Tracker ids in scope: Komga 6, Kavita 8, Suwayomi 9, Jellyfin 102 (`TrackerManager.kt:22-24`, `:32`, `:35`).

Logout (`BaseTracker.kt:27-30`) sets both to `""`.

Proposal: store a boolean `enhanced_tracker_enabled_<id>` (default `false`) instead of dummy credentials, so no credential-shaped value is written for a tracker that has none. `isLoggedIn` then reads that flag.

### 5.3 Tracking settings and filters that touch these trackers

- `pref_auto_update_manga_sync_key` (Boolean, default `true`) gates auto-tracking for manga and anime (catalogs section for area:27-tracking-core).
- `pref_filter_library_tracked_<id>` (`TriState`) for each tracker id, including 6, 8, 9, 102 (`catalogs/preferences-01.md:95`).

No keys are specific to these four trackers beyond the credential and source keys above.

### 5.4 Network configuration that applies to these trackers (OBSERVED)

From `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt`:
- Default user agent preference `default_user_agent` (`NetworkHelper.kt:73`). Suwayomi sends it (`SuwayomiApi.kt:36`). Kavita, Komga, Jellyfin send the fixed Aniyomi UA (see 6).
- DoH provider preference `doh_provider` (`NetworkHelper.kt:42-57`). All four trackers override it with `dns(Dns.SYSTEM)`, except Kavita's `getNewToken` (D-04).
- Timeouts: connect 30 s, read 30 s, call 2 min (`NetworkHelper.kt:23-25`).
- Cache: 5 MiB at `cacheDir/network_cache` (`NetworkHelper.kt:26-31`).

---

## 6. Network endpoints (OBSERVED, with UPSTREAM confirmation where noted)

Common headers: none unless stated. `User-Agent` is `Aniyomi v{BuildConfig.VERSION_NAME} ({BuildConfig.APPLICATION_ID})` for Kavita, Komga and Jellyfin; for Suwayomi it is the user default UA preference.

### 6.1 Kavita

| Id | Method | URL | Query / body | Headers | Expected response | Codes handled |
|---|---|---|---|---|---|---|
| K1 | POST | `{APIURL}/Plugin/authenticate` | query `apiKey={APIKEY}&pluginName=Tachiyomi-Kavita`; body `{}`; `Content-Type: application/json; charset=utf-8` | none (no bearer) | `AuthenticationDto` (`token`) | 200 token; 401 and 500 throw (D-06); other: null (`KavitaApi.kt:40-84`) |
| K2 | GET | `{api}/Series/volumes?seriesId={id}` | — | `Authorization: Bearer {jwt}`, UA | `List<VolumeDto>` | any non-2xx: exception (`awaitSuccess`-like via `execute()`; see note) |
| K3 | GET | `{api}/Tachiyomi/latest-chapter?seriesId={id}` | — | Bearer, UA | 200 `ChapterDto`; 204 empty | 200 parse; 204 → 0.0; other codes → 0.0 (`KavitaApi.kt:126-148`) |
| K4 | GET | the series URL (`{host}/api/series/{id}`) | — | Bearer, UA | `SeriesDto` | `awaitSuccess()` (`KavitaApi.kt:152-156`) |
| K5 | POST | `{api}/Tachiyomi/mark-chapter-until-as-read?seriesId={id}&chapterNumber={Double}` | body `{}`, `application/json; charset=utf-8` | Bearer, UA | `ActionResult<bool>` (ignored) | `awaitSuccess()` (`KavitaApi.kt:179-191`) |

`{api}` = `getApiFromUrl(url)` = `url.split("/api/").first() + "/api"` (`KavitaApi.kt:30-32`). `{id}` = last path segment of the series URL, parsed as `Int` (`KavitaApi.kt:92-94`).

Note on K2 and K3: `getTotalChapters` and `getLatestChapterRead` use `execute()` and check only `it.code` (K3), or rely on `parseAs` throwing for a non-JSON error body (K2). A 401 on K2 therefore surfaces as a deserialization exception (Proposal: check the status first).

UPSTREAM confirmation: `Kavita.Server/Controllers/TachiyomiController.cs:26-44` (`GET latest-chapter?seriesId`, `POST mark-chapter-until-as-read?seriesId&chapterNumber&generateReadingSessions=true`). The authenticate route is in `Kavita.Server/Controllers/AccountController.cs` (not read; the path is used as-is by the client).

### 6.2 Komga

| Id | Method | URL | Body | Headers | Expected response | Codes handled |
|---|---|---|---|---|---|---|
| M1 | GET | series URL `{host}/api/v1/series/{id}` | — | UA (`KomgaApi.kt:29-31`) | `SeriesDto` | `awaitSuccess()` (`:41-50`) |
| M2 | GET | readlist URL `{host}/api/v1/readlists/{id}` | — | UA | `ReadListDto` | `awaitSuccess()` |
| M3 | GET | `…/api/v2/series/{id}/read-progress/tachiyomi` (rewritten from M1's URL) | — | UA | `ReadProgressV2Dto` | `awaitSuccess()`; parse by `contains("/api/v1/series/")` (`:53-65`) |
| M4 | GET | `{readlist url}/read-progress/tachiyomi` (not rewritten) | — | UA | `ReadProgressDto` | `awaitSuccess()` |
| M5 | PUT | `…/api/v2/series/{id}/read-progress/tachiyomi` | `{"lastBookNumberSortRead": Double}`; `application/json` (no charset) | UA | 204 | `awaitSuccess()` (`:84-99`) |
| M6 | PUT | `{readlist url}/read-progress/tachiyomi` | `{"lastBookRead": Int}`; `application/json` | UA | 204 | `awaitSuccess()` |

Upstream checks (Komga repo, read-only):
- `SeriesController.kt:773-801`: M3 (`GET v2/series/{seriesId}/read-progress/tachiyomi`) and M5 (`PUT …`), marked "Mihon specific".
- `ReadListController.kt:411-438`: M4 and M6 (`{id}/read-progress/tachiyomi`), marked "Mihon specific".
- The v1 series path has no read-progress endpoint in upstream. Therefore M5 must use v2 (the Kotlin rewrite is required).

Komga headers: `Komga.kt:28-31` creates a client with no interceptor. No `Authorization` header is sent. Auth depends on cookies (D-19).

### 6.3 Suwayomi

| Id | Method | URL | Body | Headers | Expected response | Codes handled |
|---|---|---|---|---|---|---|
| S1 | GET | `{base}/api/v1/manga/{id}/full` | — | UA (default preference), `Authorization: Basic …` if both credentials set (`SuwayomiApi.kt:35-43`) | `MangaDataClass` | `awaitSuccess()` (`:58-62`) |
| S2 | GET | `{base}/api/v1/manga/{id}/chapters` | — | same | `List<ChapterDataClass>` | `awaitSuccess()` (`:82-86`) |
| S3 | PUT | `{base}/api/v1/manga/{id}/chapter/{index}` | form: `markPrevRead=true`, `read=true` (charset `utf8`) | same | any 2xx | `awaitSuccess()` (`:89-98`) |

`{base}` = `baseUrl`, the "Server URL Address" preference. `{id}` = the numeric id. Where the `tracking_url` is not numeric, it is used as the base instead of `{base}/api/v1/manga/{id}`, so the path is whatever the stored URL is (`SuwayomiApi.kt:49-56`).

UPSTREAM (`server/src/main/kotlin/suwayomi/tachidesk/manga/controller/MangaController.kt:386-402`): the chapter form parameters are `read`, `bookmarked`, `markPrevRead`, `lastPageRead` (`formParam<Boolean?>`).

### 6.4 Jellyfin

| Id | Method | URL | Query | Headers | Expected response | Codes handled |
|---|---|---|---|---|---|---|
| J1 | GET | tracking URL `https://{h}/Users/{U}/Items/{I}` | `api_key={key}` added by interceptor | UA (`JellyfinInterceptor.kt:23-24`) | `JFItem` | `awaitSuccess()` (`JellyfinApi.kt:33-37`) |
| J2 | GET | `https://{h}/Shows/{S}/Episodes` | `seasonId={I}&userId={U}&Fields=Overview,MediaSources&api_key={key}` | UA | `JFItemList` | `awaitSuccess()` (`:88-93`, `:131-136`) |
| J3 | POST | `https://{h}/Users/{U}/PlayedItems/{E}` | `DatePlayed={yyyy-MM-ddTHH:mm:ss.SSSZ}&api_key={key}` | UA | any 2xx | `awaitSuccess()` (`:153-155`) |

`{S}` = `fragment.split(",").last()` (the series id). `{I}` = last path segment of the tracking URL. `{U}` = `userId` query or `pathSegments[1]`. `{E}` = the matched episode's `Id`. `{key}` = `api_key` value from the source preference (`JellyfinInterceptor.kt:36-38`).

`encodedPath("/")` in `getEpisodesUrl` (`JellyfinApi.kt:72`) resets the path. Any reverse-proxy prefix on the server is therefore lost (D-13).

UPSTREAM (Jellyfin OpenAPI, read-only knowledge, not fetched): `POST /Users/{userId}/PlayedItems/{itemId}` with query `datePlayed`. ASP.NET binds query names case-insensitively, so `DatePlayed` works.

---

## 7. Database rows written by these trackers (OBSERVED, schema owned by area:27-tracking-core section 6)

Table `anime_sync` (Jellyfin) and `manga_sync` (Kavita, Komga, Suwayomi). Column values written by these trackers:

| Column | Value | Notes |
|---|---|---|
| `anime_id` / `manga_id` | the entry id | set by `bindEnhancedTrackers` |
| `sync_id` | 102 (Jellyfin), 8 (Kavita), 6 (Komga), 9 (Suwayomi) | `UNIQUE (manga_id, sync_id)` |
| `remote_id` | 0 | never set by these trackers |
| `library_id` | NULL | never set |
| `title` | the search title | |
| `last_chapter_read` (manga) / `last_episode_seen` (anime) | REAL | from remote |
| `total_chapters` / `total_episodes` | INTEGER | from remote |
| `status` | 1, 2, 3 (section 3) | from remote (D-02) |
| `score` | -1.0 | default; never set |
| `remote_url` | `tracking_url` | the source's URL (Komga/Kavita series or readlist URL; Suwayomi base URL; Jellyfin item URL with fragment) |
| `start_date` / `finish_date` (manga: `started_reading_date` / `finished_reading_date`) | 0 | never set by these trackers; `copyPersonalFrom` overwrites them with 0 (D-03) |
| `private` | 0 | never set |

Repository: `data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt` (area:27-tracking-core owns it).

---

## 8. Date and number formats (OBSERVED)

| Use | Format | Source | Port |
|---|---|---|---|
| Jellyfin `DatePlayed` | `yyyy-MM-dd'T'HH:mm:ss.SSS'Z'` in **default time zone** with literal `Z` (`JellyfinApi.kt:166`) | Locale.US | Format UTC: `YYYY-MM-DDTHH:MM:SS.sssZ` (D-10) |
| Komga `created`, `lastModified` | opaque `String?` | not parsed | keep as string |
| Kavita `created`, `lastModified` | `String` | not parsed | keep as string |
| Chapter number for Kavita | `replace(",", ".").toDouble()` (`KavitaApi.kt:133`) | locale-independent | same |
| Kavita `chapterNumber` query | Kotlin `Double.toString()` (`12.0`) | not encoded | Format the same way; do not use `{:e}` |
| Komga `lastBookNumberSortRead` | JSON `Double` (`12.0`) | `kotlinx.serialization` | serde_json `f64` (`12.0`) |
| Komga `lastBookRead` | `toInt()` | truncates | same |

---

## 9. Enum and bit layouts

None. The four trackers store no bit flags. Status is an integer set documented in section 3.

---

## 10. Rust type mapping (Proposal)

| Kotlin | Rust | Note |
|---|---|---|
| `Long` (status, ids) | `i64` | status values are small; keep `i64` for DB parity |
| `Int` (DTO ids, counts) | `i32` | `serde` |
| `Long?` / `Int?` | `Option<i64>` / `Option<i32>` | `#[serde(default)]` |
| `Double` | `f64` | |
| `Float` | `f32` | `maxNumberSort` (`KomgaModels.kt:104`) |
| `String` / `String?` | `String` / `Option<String>` | `#[serde(default)]` for defaulted ones |
| `Map<String,String>` | `BTreeMap<String,String>` | sorted for stable output |
| `Set<String>` | `BTreeSet<String>` | |
| `List<T> = emptyList()` | `Vec<T>` with `#[serde(default)]` | |
| `Boolean` | `bool` | |
| `Interceptor` | not ported | replaced by an explicit auth context (see `p1-rust-design.md` section 3) |
| `SharedPreferences` | `kuro_prefs::SourcePrefs` (area:02-preferences-system) | read-only from trackers |
| `lazy` | `OnceLock` / `tokio::sync::OnceCell` | |

Serde attributes: Jellyfin keys use `#[serde(rename = "Name")]` etc. Kavita and Komga names are used as-is with `#[serde(rename_all = "camelCase")]` where the Kotlin name is camelCase and `#[serde(rename = "thumbnail_url")]` for the one snake-case name.

---

## 11. Summary of config that the port must provide

1. Four tracker ids and their logo asset keys (T-33a-100).
2. Source-id derivation for Kavita slots 1..3, Jellyfin slots 1..10, and Suwayomi (section 4).
3. Reading of the external source preference keys (section 5.1), per request.
4. Dummy credential replacement with a flag (Proposal, section 5.2).
5. System DNS for all four trackers (section 5.4, D-04).
6. Endpoint table (section 6).
7. `anime_sync` / `manga_sync` row mapping (section 7).
