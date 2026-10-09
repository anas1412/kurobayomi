# area:28-tracker-anilist, part 1 — data models, preferences, files, network, DB

Repo root: `/home/blackbox/aniyomi-src`. Paths repo-relative. Field names are as in source. Query text below is copied from the Kotlin raw strings after `trimMargin()`; indentation is normalised, token order and field names are exact. `${'$'}` in source is `$`.

---

## 1. Persisted OAuth record (`ALOAuth`)

File: `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALOAuth.kt`

| Kotlin field | JSON key (`@SerialName`) | Type | Nullable | Default | Meaning |
|---|---|---|---|---|---|
| `accessToken` | `access_token` | String | no | none | bearer token |
| `tokenType` | `token_type` | String | no | `"Bearer"` (set by `createOAuth`) | |
| `expires` | `expires` | Long | no | `now_ms + 31536000000` at login | absolute epoch **ms** (see behaviour §4.6 bug) |
| `expiresIn` | `expires_in` | Long | no | `31536000000` | duration **ms**, not the AniList `expires_in` seconds |

- Stored as JSON text in preference `track_token_2` (see §2).
- Written by `Anilist.saveOAuth` (`ANI:364-366`) as `json.encodeToString(alOAuth)`; when `alOAuth` is null the stored text is `null`.
- Read by `Anilist.loadOAuth` (`ANI:368-374`); any decode error → null.

Fuzzy function: `fun ALOAuth.isExpired() = System.currentTimeMillis() > expires` (`ALOAuth.kt:17`).

Proposal (Rust) struct: `OAuthRecord { access_token: String, token_type: String, expires_at_unix_s: i64, expires_in_s: i64 }`. Migration: on load, if a stored `expires` is > 10^12 treat it as ms and divide by 1000; this makes existing v1-of-this-app records usable.

---

## 2. Preferences (keys, defaults, scopes)

Preference store: `tachiyomi.core.common.preference.PreferenceStore` (the key-value store behind `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`). Android SharedPreferences-backed in the source; Rust replacement per area:02-preferences-system (TOML/SQLite). Keys used by AniList:

| Key (exact string) | Kotlin accessor | Type | Default | Scope | Read/write sites |
|---|---|---|---|---|---|
| `pref_mangasync_username_2` | `trackUsername(tracker)` = `Preference.privateKey("pref_mangasync_username_${tracker.id}")` | String | `""` | private (per-tracker) | `BaseTracker.getUsername` (`BaseTracker.kt:45`); AniList stores Viewer `id` as decimal string (`ANI:352`) |
| `pref_mangasync_password_2` | `trackPassword(tracker)` = `Preference.privateKey("pref_mangasync_password_${tracker.id}")` | String | `""` | private | Stores the **access token** for AniList (`ANI:352` `saveCredentials(username, oauth.accessToken)`); `BaseTracker.getPassword` |
| `pref_tracker_auth_expired_2` | `trackAuthExpired(tracker)` = `Preference.privateKey("pref_tracker_auth_expired_${tracker.id}")` | Boolean | `false` | private | Set false by `setCredentials`; set true only by MyAnimeList (`MyAnimeList.kt:273`); not used by AniList today |
| `track_token_2` | `trackToken(tracker)` = `Preference.privateKey("track_token_${tracker.id}")` | String (JSON) | `""` | private | OAuth record (§1) |
| `anilist_score_type` | `anilistScoreType()` = `preferenceStore.getString("anilist_score_type", Anilist.POINT_10)` (`TrackPreferences.kt:37`) | String | `"POINT_10"` | global | Written only by `login` from Viewer `scoreFormat` (`ANI:351`); no UI control |
| `pref_auto_update_manga_sync_key` | `autoUpdateTrack()` (`TrackPreferences.kt:39`) | Boolean | `true` | global | Gate for player/reader auto-update (`PlayerViewModel.kt:1949`, `ReaderViewModel.kt:920`, `ExternalIntents.kt:479,525`) |
| `track_on_adding_to_library` | `trackOnAddingToLibrary()` (`TrackPreferences.kt:41`) | Boolean | `true` | global | Opens track menu on add (`AnimeScreenModel.kt:190`) |
| `show_next_episode_airing_time` | `showNextEpisodeAiringTime()` (`TrackPreferences.kt:43-46`) | Boolean | `true` | global | AniChart airing display |
| `pref_auto_update_manga_on_mark_read` | `autoUpdateTrackOnMarkRead()` (`TrackPreferences.kt:48-51`) | enum `AutoTrackState` | `ALWAYS` | global | `AnimeScreenModel.kt:877-897` |

Private-key helper: `Preference.privateKey(name)` marks the key as private. Its exact effect (backup exclusion or otherwise) is defined in area:02-preferences-system and was not verified in this area. **Proposal:** Keep credentials out of plain TOML; store the token in the OS keyring (`secret-service` via the `keyring` crate) and keep only the Viewer id in settings.

`AutoTrackState` enum (`app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt:6-10`): `ALWAYS(MR.strings.lock_always)`, `ASK(MR.strings.default_category_summary)`, `NEVER(MR.strings.lock_never)`. Persisted by enum name (`getEnum`).

Legacy key (not read anywhere now, documented for migration): `anilist_score_type` may be stored as an **int** by APIv1; reading it as String throws `ClassCastException` which triggers logout (`ANI:70-75`).

Non-preference state: `SharedPreferences("tracking_queue")` for delayed updates (`app/src/main/java/eu/kanade/domain/track/anime/store/DelayedAnimeTrackingStore.kt:92`): key = `track _id` as string, value = float last episode seen. Proposal: SQLite table `delayed_track_updates(track_id INTEGER PRIMARY KEY, last_progress REAL)` (area:27 owns the table).

---

## 3. Constants (complete)

From `ANI:38-54`:

| Symbol | Type | Value |
|---|---|---|
| `READING` | Long | 1 |
| `WATCHING` | Long | 11 |
| `COMPLETED` | Long | 2 |
| `ON_HOLD` | Long | 3 |
| `DROPPED` | Long | 4 |
| `PLAN_TO_READ` | Long | 5 |
| `PLAN_TO_WATCH` | Long | 15 |
| `REREADING` | Long | 6 |
| `REWATCHING` | Long | 16 |
| `POINT_100` | String | `"POINT_100"` |
| `POINT_10` | String | `"POINT_10"` |
| `POINT_10_DECIMAL` | String | `"POINT_10_DECIMAL"` |
| `POINT_5` | String | `"POINT_5"` |
| `POINT_3` | String | `"POINT_3"` |

From `API:569-574` (`private const val` unless noted):

| Symbol | Value |
|---|---|
| `CLIENT_ID` | `"5338"` |
| `API_URL` | `"https://graphql.anilist.co/"` |
| `BASE_URL` | `"https://anilist.co/api/v2/"` |
| `BASE_MANGA_URL` | `"https://anilist.co/manga/"` |
| `BASE_ANIME_URL` | `"https://anilist.co/anime/"` |
| rate limit | `permits = 85`, `period = 1.minutes` (`API:42`) |
| token lifetime literal | `31536000000` (ms) (`API:516`) |
| expiry margin literal | `60 * 1000` (`INT:20`) |
| search page size | `50` (`API:236,297`) |

Tracker id: `TrackerManager.ANILIST = 2L` (`app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:20`).

Intent / URL scheme: `aniyomi://anilist-auth` (`app/src/main/AndroidManifest.xml:250-257`). Proposal: `kurobayomi://anilist-auth`.

Tracking URL formats stored in `tracking_url`: `https://anilist.co/manga/{id}`, `https://anilist.co/anime/{id}`.

---

## 4. GraphQL operations (exact text, variables, types)

All POST bodies: `{"query": <text>, "variables": {…}}`. The HTTP method is POST to `https://graphql.anilist.co/`. Headers: `Authorization: Bearer <token>` (added by interceptor), `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`, `Content-Type: application/json; charset=utf-8`.

### 4.1 `AddManga` — `API:45-79` (`addLibManga`)

```graphql
mutation AddManga($mangaId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean) {
  SaveMediaListEntry (mediaId: $mangaId, progress: $progress, status: $status, private: $private) {
    id
    status
  }
}
```
Variables: `mangaId` = `track.remote_id` (Long); `progress` = `track.last_chapter_read.toInt()`; `status` = `track.toApiStatus()`; `private` = `track.private`.
Response parsed as `ALAddEntryResult` (`data.SaveMediaListEntry.id`). Return value: the input track (id discarded).

### 4.2 `UpdateManga` — `API:81-115` (`updateLibManga`)

```graphql
mutation UpdateManga(
  $listId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean,
  $score: Int, $startedAt: FuzzyDateInput, $completedAt: FuzzyDateInput
) {
  SaveMediaListEntry(
    id: $listId, progress: $progress, status: $status, private: $private,
    scoreRaw: $score, startedAt: $startedAt, completedAt: $completedAt
  ) {
    id
    status
    progress
  }
}
```
Variables: `listId` = `track.library_id` (Long?, may be null → JSON `null`); `progress` = `last_chapter_read.toInt()`; `status` = `toApiStatus()`; `score` = `track.score.toInt()` (raw 0–100, may be negative, see edge cases); `startedAt` = `createDate(started_reading_date)`; `completedAt` = `createDate(finished_reading_date)`; `private` = `track.private`.
Response: not read.

### 4.3 `DeleteManga` — `API:119-136` (`deleteLibManga`)

```graphql
mutation DeleteManga($listId: Int) {
  DeleteMediaListEntry(id: $listId) {
    deleted
  }
}
```
Variables: `listId` = `track.libraryId` (Domain track field). Response: not read.

### 4.4 `AddAnime` — `API:140-172` (`addLibAnime`)

```graphql
mutation AddAnime($animeId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean) {
  SaveMediaListEntry (mediaId: $animeId, progress: $progress, status: $status, private: $private) {
    id
    status
  }
}
```
Variables: `animeId` = `remote_id`; `progress` = `last_episode_seen.toInt()`; `status`; `private`. Response `ALAddEntryResult`.

### 4.5 `UpdateAnime` — `API:174-208` (`updateLibAnime`)

Same text as UpdateManga with `UpdateAnime`. Variables: `listId` = `library_id`; `progress` = `last_episode_seen.toInt()`; `score` = `score.toInt()`; `startedAt` = `createDate(started_watching_date)`; `completedAt` = `createDate(finished_watching_date)`; `private`.

### 4.6 `DeleteAnime` — `API:210-229` (`deleteLibAnime`)

Same as DeleteManga with name `DeleteAnime`; variable `listId` = `track.libraryId`.

### 4.7 `Search` (manga) — `API:231-291` (`search`)

```graphql
query Search($query: String) {
  Page (perPage: 50) {
    media(search: $query, type: MANGA, format_not_in: [NOVEL]) {
      id
      staff {
        edges {
          role
          id
          node {
            name {
              full
              userPreferred
              native
            }
          }
        }
      }
      title {
        userPreferred
      }
      coverImage {
        large
      }
      format
      status
      chapters
      description
      startDate {
        year
        month
        day
      }
      averageScore
    }
  }
}
```
Variables: `{"query": <search text>}`. Response `ALSearchResult`; maps `data.Page.media` through `toALManga().toTrack()`.

### 4.8 `Search` (anime) — `API:293-348` (`searchAnime`)

```graphql
query Search($query: String) {
  Page (perPage: 50) {
    media(search: $query, type: ANIME) {
      id
      studios {
        edges {
          isMain
          node {
            name
          }
        }
      }
      title {
        userPreferred
      }
      coverImage {
        large
      }
      format
      status
      episodes
      description
      startDate {
        year
        month
        day
      }
      averageScore
    }
  }
}
```

### 4.9 Find manga entry — `API:350-429` (`findLibManga`)

```graphql
query ($id: Int!, $manga_id: Int!) {
  Page {
    mediaList(userId: $id, type: MANGA, mediaId: $manga_id) {
      id
      status
      scoreRaw: score(format: POINT_100)
      progress
      private
      startedAt {
        year
        month
        day
      }
      completedAt {
        year
        month
        day
      }
      media {
        id
        title {
          userPreferred
        }
        coverImage {
          large
        }
        format
        status
        chapters
        description
        startDate {
          year
          month
          day
        }
        staff {
          edges {
            role
            id
            node {
              name {
                full
                userPreferred
                native
              }
            }
          }
        }
      }
    }
  }
}
```
Variables: `id` = `userid` (Int, from Viewer id), `manga_id` = `track.remote_id`.
Response: `ALUserListEntryQueryResult` → `data.Page.mediaList` → `toALUserManga()` → `toTrack()`; `firstOrNull()`.

### 4.10 Find anime entry — `API:431-505` (`findLibAnime`)

Same as 4.9 with `query ($id: Int!, $anime_id: Int!)`, `mediaList(userId: $id, type: ANIME, mediaId: $anime_id)`, media fields `episodes` instead of `chapters`, and `studios { edges { isMain node { name } } }` instead of `staff`.

### 4.11 Viewer — `API:519-550` (`getCurrentUser`)

```graphql
query User {
  Viewer {
    id
    mediaListOptions {
      scoreFormat
    }
  }
}
```
No `variables` key in payload. Response `ALCurrentUserResult` → `(viewer.id, viewer.mediaListOptions.scoreFormat)`.

### 4.12 AniSkip (outside scope, `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt:47-51`)

```graphql
query{
Media(id:$id){idMal}
}
```
Raw, no auth, endpoint `https://graphql.anilist.co`, no variables.

### 4.13 AniChart (outside scope, `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:53-59,80-88`)

- `query { Media(idMal:$idMal,type: ANIME) { id } }`
- `query { Media(id:$id) { nextAiringEpisode { episode airingAt } } }`
Plain, no auth.

---

## 5. Request/response shapes (DTO field types)

### 5.1 `ALAddEntryResult` (`dto/ALAddEntry.kt`)
- `ALAddEntryResult { data: ALAddEntryData }`
- `ALAddEntryData { @SerialName("SaveMediaListEntry") entry: ALAddEntryEntry }`
- `ALAddEntryEntry { id: Long }`

### 5.2 `ALSearchResult` (`dto/ALSearch.kt`)
- `ALSearchResult { data: ALSearchPage }`
- `ALSearchPage { @SerialName("Page") page: ALSearchMedia }`
- `ALSearchMedia { media: List<ALSearchItem> }`

### 5.3 `ALSearchItem` (`dto/ALSearchItem.kt:5-19`)
| Field | Type | Nullable |
|---|---|---|
| `id` | Long | no |
| `title` | `ALItemTitle { userPreferred: String }` | no |
| `coverImage` | `ItemCover { large: String }` | no |
| `description` | String | yes |
| `format` | String | yes |
| `status` | String | yes |
| `startDate` | `ALFuzzyDate` | no (object; fields nullable) |
| `chapters` | Long | yes |
| `episodes` | Long | yes |
| `averageScore` | Int | yes |
| `staff` | `ALStaff` | yes (NPE in `toALManga` via `staff!!`) |
| `studios` | `ALStudios` | yes (NPE in `toALAnime` via `studios!!`) |

Sub-models:
- `ALStaff { edges: List<ALStaffEdge> }`
- `ALStaffEdge { role: String, id: Int, node: ALStaffNode }`
- `ALStaffNode { name: ALStaffName }`
- `ALStaffName { userPreferred: String?, native: String?, full: String? }` with `operator fun invoke(): String? = userPreferred ?: full ?: native`
- `ALStudios { edges: List<ALStudiosEdge> }`
- `ALStudiosEdge { isMain: Boolean, node: ALStudiosNode }`
- `ALStudiosNode { name: String }`

### 5.4 `ALFuzzyDate` (`dto/ALFuzzyDate.kt`)
`{ year: Int?, month: Int?, day: Int? }`. `toEpochMilli()` → `LocalDate.of(year!!, month!!, day!!).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()`, catch → `0L`.

### 5.5 `ALUserListEntryQueryResult` (`dto/ALUserList.kt`)
- `{ data: ALUserListEntryPage }`
- `ALUserListEntryPage { @SerialName("Page") page: ALUserListMediaList }`
- `ALUserListMediaList { mediaList: List<ALUserListItem> }`
- `ALUserListItem { id: Long, status: String, scoreRaw: Int, progress: Int, startedAt: ALFuzzyDate, completedAt: ALFuzzyDate, media: ALSearchItem, private: Boolean }`

Mapping functions `toALUserManga()` / `toALUserAnime()` as in behaviour §7.5.

### 5.6 `ALUserViewer` (`dto/ALUser.kt`)
- `ALCurrentUserResult { data: ALUserViewer }`
- `ALUserViewer { @SerialName("Viewer") viewer: ALUserViewerData }`
- `ALUserViewerData { id: Int, mediaListOptions: ALUserListOptions }`
- `ALUserListOptions { scoreFormat: String }`

### 5.7 Domain/DB track models used
- `eu.kanade.tachiyomi.data.database.models.manga.MangaTrack` (interface; impl `MangaTrackImpl`): `id: Long?`, `manga_id: Long`, `tracker_id: Long`, `remote_id: Long`, `library_id: Long?`, `title: String`, `last_chapter_read: Double`, `total_chapters: Long`, `score: Double`, `status: Long`, `started_reading_date: Long`, `finished_reading_date: Long`, `tracking_url: String`, `private: Boolean`. Impl defaults: id null, ids 0, library_id null, title lateinit, progress 0.0, total 0, score 0.0, status 0, dates 0, tracking_url "", private false.
- Anime: `AnimeTrack` / `AnimeTrackImpl` analog with `anime_id`, `last_episode_seen`, `total_episodes`, `started_watching_date`, `finished_watching_date`.
- `MangaTrackSearch` / `AnimeTrackSearch`: subclasses with extra `authors: List<String> = emptyList()`, `artists: List<String>` (manga only uses artists; anime `artists` field exists but is never set), `cover_url: String = ""`, `summary: String = ""`, `publishing_status: String = ""`, `publishing_type: String = ""`, `start_date: String = ""`, and `score` default **-1.0**. Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt`, `AnimeTrackSearch.kt`.
- Domain `tachiyomi.domain.track.manga.model.MangaTrack` (`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt`): `id: Long, mangaId: Long, trackerId: Long, remoteId: Long, libraryId: Long?, title: String, lastChapterRead: Double, totalChapters: Long, status: Long, score: Double, remoteUrl: String, startDate: Long, finishDate: Long, private: Boolean`. Anime: `animeId`, `lastEpisodeSeen`, `totalEpisodes`, `startDate`, `finishDate`.

Mapping DB ↔ domain: `app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt:17-51` (`toDbTrack`, `toDomainTrack(idRequired)` returns null when id null and idRequired true; uses -1 otherwise). Anime analog at `app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt:68-102`.

---

## 6. Database tables touched

### 6.1 `manga_sync` (`data/src/main/sqldelight/data/manga_sync.sq`)
```
CREATE TABLE manga_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,          -- tracker id (2 for AniList)
    remote_id INTEGER NOT NULL,
    library_id INTEGER,                -- AniList MediaList entry id
    title TEXT NOT NULL,
    last_chapter_read REAL NOT NULL,
    total_chapters INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,               -- 0..100
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,       -- epoch ms
    finish_date INTEGER NOT NULL,      -- epoch ms
    private INTEGER AS Boolean DEFAULT 0 NOT NULL,
    UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);
```
Queries: `getTracksByMangaId`, `insert` (no `_id`), `update` (coalesce), `delete` by `(manga_id, sync_id)`.

### 6.2 `anime_sync` (`data/src/main/sqldelightanime/dataanime/anime_sync.sq`)
Same shape with `anime_id`, `last_episode_seen REAL`, `total_episodes INTEGER`, `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`, FK `animes (_id)`.

Note for AniList: `insert` is how updates are persisted (`insertValues` in `data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt:43-71`), and `ON CONFLICT REPLACE` means the `_id` changes on every write. Anything keyed by track `_id` (delayed queue `tracking_queue`) must tolerate this (the framework calls `getTracks` each time, so it does).

Column → field mapping for AniList-relevant fields:
- `sync_id` ↔ `trackerId` (value 2)
- `remote_id` ↔ AniList media id
- `library_id` ↔ AniList MediaList id (`SaveMediaListEntry.id`)
- `remote_url` ↔ `https://anilist.co/{manga|anime}/{id}` (set on search, not on refresh)

Known DB-side issue (cross-area): `manga_sync.sq` `update:` uses `:mediaId` and `:totalChapter` names that differ from the insert parameter names. Owned by area:03; affects AniList only if `update` is ever used (the framework uses `insert`).

---

## 7. Files and folders on disk

AniList itself writes no files. Relevant on-disk state:
- Android app-private `SharedPreferences` (XML under `shared_prefs/`) for `tracking_queue`, and the preference store file for the keys in §2. Rust: the preference store file per area:02 and SQLite `delayed_track_updates` table (proposal §2).
- Cover images are loaded over HTTP from `coverImage.large` by Coil (Android). No local cache policy specific to AniList. Rust: `cover_url` is fetched by the image loader in area:14/app-level; no AniList-specific cache.

---

## 8. Network endpoints summary

| Purpose | Method | URL | Auth | Rate-limited | Source |
|---|---|---|---|---|---|
| OAuth authorize (browser) | GET (navigation) | `https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token` | none | no | `API:584-587` |
| GraphQL (all data) | POST | `https://graphql.anilist.co/` | bearer for tracker calls; none for AniSkip/AniChart | tracker calls: 85/60s | `API:571` |
| Browser links | GET | `https://anilist.co/manga/{id}`, `https://anilist.co/anime/{id}` | none | no | `API:573-574,576-582` |

Redirect: `aniyomi://anilist-auth#access_token=…&token_type=…&expires_in=…` (fragment; not sent to server).

---

## 9. Error payload shapes

- HTTP non-2xx → `HttpException(code)`; body discarded (`OkHttpExtensions.kt:112-115`).
- GraphQL errors (`{"errors":[{"message":…,"status":…,…}]}`) are **not** parsed by any call in this area; mutations ignore the body; queries fail with a serialization exception when `data` is missing. Proposal: parse `errors[].message` and `errors[].status` (AniList error objects include `message`; this is the documented shape of GraphQL errors; not verified live here because the AniList docs site returned 403 to the research tool).
- Serialization failure → kotlinx `SerializationException` (message text is library-defined). Proposal: map to `AniListError::Decode { context }`.

---

## 10. Enumerations (complete)

| Enum | Values (stored) | Where |
|---|---|---|
| Local manga status | 1,2,3,4,5,6 (READING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_READ, REREADING) | `ANI` |
| Local anime status | 11,2,3,4,15,16 (WATCHING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_WATCH, REWATCHING) | `ANI` |
| AniList MediaListStatus (API strings) | `CURRENT`, `PLANNING`, `COMPLETED`, `DROPPED`, `PAUSED`, `REPEATING` | `UTL`, `DTO/ALManga.kt`, `DTO/ALAnime.kt` |
| AniList ScoreFormat (API strings) | `POINT_100`, `POINT_10_DECIMAL`, `POINT_10`, `POINT_5`, `POINT_3` | `ANI` |
| AniList MediaFormat (strings received; full set not enumerated in source) | Code only handles the string as opaque text; underscores replaced with `-` (`DTO/ALSearchItem.kt:25,38`). Examples used in this doc (`MANGA`, `NOVEL`, `ONE_SHOT`, `TV`) are from AniList's public schema as known to the author, NOT verified in this environment | `DTO/ALSearchItem.kt` |
| AniList MediaStatus (strings received; not enumerated in source) | Opaque text, displayed after lowercasing and capitalising (`AnimeTrackerSearch.kt:247`). Examples: `FINISHED`, `RELEASING`, `NOT_YET_RELEASED` (unverified in this environment) | `DTO/ALSearchItem.kt` (`status`) |
| AniList MediaType filter | `MANGA`, `ANIME` | `API` |
| AniList MediaFormat filter | `format_not_in: [NOVEL]` (manga search only) | `API:236` |
| FuzzyDateInput | `{year, month, day}` nullable ints | `API:88,107` |
| AutoTrackState | `ALWAYS`, `ASK`, `NEVER` | `app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt` |

The MediaFormat and MediaStatus lists above are the values the code can receive; the source does not enumerate them. They are observed from UI mapping and the `publishing_type`/`publishing_status` display path, not from an AniList schema file.

---

## 11. Mapping table: AniList score `scoreRaw` ↔ local score ↔ UI string

| scoreRaw (AniList, 0–100) | Local `score` (Double) | POINT_10 display | POINT_100 display | POINT_5 display | POINT_3 display | POINT_10_DECIMAL display |
|---|---|---|---|---|---|---|
| 0 | 0.0 | `0` | `0` | `0 ★` | `0` | `0.0` |
| 10 | 10.0 | `1` | `10` | `1 ★` | `😦` | `1.0` |
| 30 | 30.0 | `3` | `30` | `2 ★` | `😦` | `3.0` |
| 35 | 35.0 | `3` | `35` | `2 ★` | `😦` | `3.5` |
| 36 | 36.0 | `3` | `36` | `2 ★` | `😐` | `3.6` |
| 50 | 50.0 | `5` | `50` | `3 ★` | `😐` | `5.0` |
| 60 | 60.0 | `6` | `60` | `3 ★` | `😐` | `6.0` |
| 61 | 61.0 | `6` | `61` | `3 ★` | `😊` | `6.1` |
| 70 | 70.0 | `7` | `70` | `4 ★` | `😊` | `7.0` |
| 90 | 90.0 | `9` | `90` | `5 ★` | `😊` | `9.0` |
| 100 | 100.0 | `10` | `100` | `5 ★` | `😊` | `10.0` |

Derived from `displayScore`/`toApiScore` rules (behaviour §3.5). Rounding and truncation are as coded: POINT_10 uses integer division; POINT_5 uses `(score+10)/20` integer division. The POINT_10_DECIMAL column is `score / 10` as Double.toString (e.g. 35.0/10 = `3.5`).

Index → stored (picker confirm), behaviour §3.4. Round trip check: picking POINT_10 index 3 stores 30; redisplay gives `3` (consistent). Picking POINT_5 index 2 stores 30; redisplay `2 ★` (consistent). Picking POINT_3 index 2 (😐) stores 60; redisplay `😐` (consistent: 60 ≤ 60).

---

## 12. Dates

- Local track dates are epoch **milliseconds** (Long), `0` = unset.
- Outgoing (`createDate`, `API:552-567`): `0` → `{year:null, month:null, day:null}`; else `ZonedDateTime.ofInstant(ms, ZoneId.systemDefault())` → `{year, month, day}` integer fields in **local** calendar. Time of day discarded.
- Incoming (`ALFuzzyDate.toEpochMilli`): local start-of-day for the given Y-M-D. Partial dates → 0.
- Round trip: 2025-03-10 23:30 local → sent as 2025-03-10 → read back as 2025-03-10 00:00 local. Date identity preserved; time lost.
- Search `start_date` display string: `yyyy-MM-dd` (local zone of the epoch value; because the epoch is local-midnight, the date is the same day).
- Date picker sends UTC midnight millis; `setDate` converts UTC→local via `convertEpochMillisZone` (behaviour §14).

Proposal (Rust): use `chrono::NaiveDate` for fuzzy dates; convert to local midnight with `Local.from_local_datetime(...).earliest()`; if local midnight doesn't exist (DST gap), use 01:00 local. Keep i64 ms in DB for compatibility.

---

## 13. Remote identifiers and linkage

| Concept | Field | Meaning |
|---|---|---|
| AniList media id | `remote_id` (DB) / `remoteId` (domain) | Stable across users |
| AniList MediaList entry id | `library_id` (DB) / `libraryId` (domain) | Per-user; required for update/delete mutations |
| AniList user id | username pref (string) | Used in `mediaList(userId:)` |
| Local entity id | `manga_id` / `anime_id` | Local |
| Local track row id | `_id` / `id` | Changes on every framework insert (REPLACE) |

Observed bug surface: `delete` writes the MediaList id into the local-row `id` field instead of `libraryId` (behaviour §10.5).

---

## 14. Logging

Framework logs with tag from `logcat` (`LogPriority.ERROR` on update failure in `AnimeTracker.kt:117`; `INFO` on refresh/track failures in `TrackEpisode.kt:210`). AniList does not log itself. Rust: use `tracing` with target `kurobayomi::tracker::anilist`; never log the access token (the `User-Agent` and token must be redacted).
