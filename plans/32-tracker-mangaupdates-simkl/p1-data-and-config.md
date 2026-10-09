# Area 32: MangaUpdates + Simkl trackers, part 1 - data models, config, endpoints

All source paths are repo-relative to `/home/blackbox/aniyomi-src`. Labels: OBSERVED = Kotlin; PROPOSAL = Rust.

Contents: 1 Track models; 2 preference keys; 3 enums and value tables; 4 network endpoints (MU, SK) with request and response shapes; 5 DB tables; 6 files on disk; 7 JSON serialisation rules; 8 Rust type sketches for the DTOs.

---

## 1. Track models

### 1.1 Database model (mutable, Java-style fields)
`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt` (interface, default impl `MangaTrackImpl`) and `.../anime/AnimeTrack.kt`.

| Field | Kotlin type | Manga semantics | Anime semantics |
|---|---|---|---|
| id | `Long?` | DB `_id` | DB `_id` |
| manga_id / anime_id | `Long` | FK to mangas / animes | FK to animes |
| tracker_id | `Long` | service id (7 for MU, 101 for SK) | service id (101 for SK) |
| remote_id | `Long` | MU `series_id` | SK `ids.simkl` |
| library_id | `Long?` | unused by MU (see below) | unused by SK (always null) |
| title | `String` | title from remote | title from remote |
| last_chapter_read / last_episode_seen | `Double` | MU `status.chapter` | SK `watched_episodes_count` (show) or 0/1 (movie) |
| total_chapters / total_episodes | `Long` | 0 from search; not set by MU bind (remains 0) | SK `total_episodes_count` (show) or 1 (movie) |
| score | `Double` | MU 0.0-10.0 with 1 decimal (or 0.0 for none) | SK 0-10 integer stored as Double, default 0.0 |
| status | `Long` | MU list id 0-4 (see section 3.1) | SK status 1-5 (section 3.2) |
| started_reading_date / started_watching_date | `Long` | epoch millis, 0 = unset | same |
| finished_reading_date / finished_watching_date | `Long` | epoch millis | same |
| tracking_url | `String` | MU series page URL from search; not updated by MU bind | SK `/<type>/<id>` after bind; absolute URL from search before bind (see behaviour D-SK-2) |
| private | `Boolean` | local flag, copied from remote by `copyPersonalFrom` (default true) | same |

Default `copyPersonalFrom(other, copyRemotePrivate = true)`:
- MangaTrack (`MangaTrack.kt:37-44`): copies `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date`, and `private` when `copyRemotePrivate`.
- AnimeTrack (`AnimeTrack.kt:37-44`): copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, `private`. Does NOT copy `total_episodes` or `title`.

The app-level extension `copyPersonalFrom` (`app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt:6-15`, manga equivalent) copies the same six fields for the domain model.

`create(serviceId)` (`MangaTrack.kt:47-49`, `AnimeTrack.kt:47-49`) builds the impl with `tracker_id = serviceId`.

### 1.2 Domain model (immutable)
`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt` (data class):
`id: Long, mangaId: Long, trackerId: Long, remoteId: Long, libraryId: Long?, title: String, lastChapterRead: Double, totalChapters: Long, status: Long, score: Double, remoteUrl: String, startDate: Long, finishDate: Long, private: Boolean` (all non-null except libraryId). Implements `Serializable`.

Anime domain (`domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt`): same with `animeId`, `lastEpisodeSeen`, `totalEpisodes`.

Conversions (`app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt:17-51`, anime equivalent):
- DB -> domain `toDomainTrack(idRequired)`: if `id == null` and `idRequired` is true returns null; if `idRequired` is false, uses id `-1`. Maps `remote_url <- tracking_url`, `start <- started_*_date`, `finish <- finished_*_date`, `trackerId <- tracker_id.toLong()` (anime) and direct (manga).
- domain -> DB `toDbTrack()`: sets `tracking_url = remoteUrl`, etc.

### 1.3 Search model
`app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt` and `AnimeTrackSearch.kt` (extend the DB track interface). Extra fields: `authors: List<String> = []`, `artists: List<String> = []`, `cover_url: String = ""`, `summary: String = ""`, `publishing_status: String = ""`, `publishing_type: String = ""`, `start_date: String = ""`. Defaults: `score = -1.0` (AnimeTrackSearch.kt:25), `status = 0`, `remote_id = 0`, `total_episodes = 0`, `last_episode_seen = 0.0`. `title` and `tracking_url` are `lateinit` (must be set by the service).

### 1.4 Preference-layer models
`TrackPreferences` values are plain strings (section 2). No serialised object other than the SK token JSON (section 2.3).

---

## 2. Preference keys (PreferenceStore)

Source: `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`. Keys are built from `Preference.privateKey(...)` (private keys, not exported by key name in backups beyond the store's own rules; area:02) and the plain key `track_token_<id>`.

| Key (exact string) | Type | Default | Read by | Written by | Used by MU/SK? |
|---|---|---|---|---|---|
| `pref_mangasync_username_7` | String | `""` | BaseTracker.getUsername | BaseTracker.saveCredentials / setCredentials | MU: uid as decimal string after login |
| `pref_mangasync_password_7` | String | `""` | BaseTracker.getPassword, MangaUpdates.restoreSession | setCredentials | MU: session token |
| `pref_tracker_auth_expired_7` | Boolean | `false` | MyAnimeList only | setCredentials resets to false | No (never set for MU) |
| `pref_mangasync_username_101` | String | `""` | BaseTracker | setCredentials | SK: user id (decimal) after login |
| `pref_mangasync_password_101` | String | `""` | BaseTracker | setCredentials | SK: OAuth access token (duplicate of token JSON) |
| `pref_tracker_auth_expired_101` | Boolean | `false` | not read | setCredentials | No |
| `track_token_101` | String | `""` (no explicit default; `get()` returns the default `""`) | Simkl.restoreToken | Simkl.saveToken (JSON), Simkl.logout delete then `"null"` write | SK: `SimklOAuth` JSON |
| `track_token_7` | String | `""` | nothing in scope | nothing in scope | No (MU token is in `pref_mangasync_password_7`) |

Key construction: `Preference.privateKey("pref_mangasync_username_${tracker.id}")` (TrackPreferences.kt:15). The `privateKey` wrapper marks the key as private; the Rust port keeps the same string keys so migrations from Aniyomi backups/exports are simple, but the password-type keys move to the secret store (PROPOSAL section 8).

Other tracking preferences in TrackPreferences.kt that are NOT used by MU or SK (listed for completeness of the file, owned by area:27): `anilist_score_type` (default `Anilist.POINT_10`), `pref_auto_update_manga_sync_key` (default true), `track_on_adding_to_library` (default true), `show_next_episode_airing_time` (default true), `pref_auto_update_manga_on_mark_read` (enum `AutoTrackState`, default `ALWAYS`).

---

## 3. Enums and value tables

### 3.1 MangaUpdates list ids (`list_id` values)
`READING_LIST = 0`, `WISH_LIST = 1`, `COMPLETE_LIST = 2`, `UNFINISHED_LIST = 3`, `ON_HOLD_LIST = 4` (MangaUpdates.kt:23-27). Rereading `-1` (never sent). Stored in `manga_sync.status`. `status` 0 is also the default for a fresh search model.

Labels (MR keys): 0 `reading_list`, 1 `wish_list`, 2 `complete_list`, 3 `unfinished_list`, 4 `on_hold_list`.

### 3.2 Simkl statuses
`WATCHING = 1`, `COMPLETED = 2`, `ON_HOLD = 3`, `NOT_INTERESTING = 4`, `PLAN_TO_WATCH = 5` (Simkl.kt:23-27). Rewatching `0` (not in list). Stored in `anime_sync.status`.

Wire strings (SimklUtils.kt):
- to Simkl: `1 -> "watching"`, `2 -> "completed"`, `3 -> "hold"`, `4 -> "notinteresting"`, `5 -> "plantowatch"`.
- from Simkl: `"watching" -> 1`, `"completed" -> 2`, `"hold" -> 3`, `"dropped" -> 4`, `"notinteresting" -> 4`, `"plantowatch" -> 5`.

### 3.3 Score encodings
- MU: 92 strings (see behaviour 2.2). Stored value is a Double (`0.0`, `1.0`, `1.1` ... `9.9`, `10.0`). The MU rating request body is a JSON number with the same Double value.
- SK: 11 strings `"0"`..`"10"`. Stored as Double (integer valued). Wire value `score.toInt()`.

### 3.4 Remote-setter enums (generic, external)
`AutoTrackState` (domain, read by the auto-update preference): `ALWAYS` default. Not used by MU/SK code directly.

### 3.5 Bit flags
None in scope.

---

## 4. Network endpoints

### 4.1 MangaUpdates (base `https://api.mangaupdates.com`, MangaUpdatesApi.kt:194)
Common headers: `Authorization: Bearer <session_token>` (only authClient), `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)` (interceptor). Bodies use `Content-Type: application/vnd.api+json` (no charset). Responses are JSON; parsed by kotlinx.serialization with default settings (`ignoreUnknownKeys` as configured in the app's `Json` injection; the DTOs have no `@JsonNames`, so unknown keys must be tolerated; PROPOSAL: `#[serde(default)]` and ignore unknown fields).

| # | Method + path | Auth | Request body | Success response parsed | Caller (line) |
|---|---|---|---|---|---|
| M1 | GET `/v1/lists/series/{remote_id}` | Bearer (authClient) | none | `MUListItem` | `getSeriesListItem` (MangaUpdatesApi.kt:45) |
| M2 | GET `/v1/series/{remote_id}/rating` | Bearer | none | `MURating` | `getSeriesRating` (119) |
| M3 | POST `/v1/lists/series` | Bearer | JSON array: `[{"series":{"id":<remote_id>},"list_id":<0 or 1>}]` | ignored (only code 200 checked) | `addSeriesToList` (65-70) |
| M4 | POST `/v1/lists/series/update` | Bearer | JSON array: `[{"series":{"id":<remote_id>},"list_id":<status>,"status":{"chapter":<int>}}]` | ignored | `updateSeriesListItem` (92-97) |
| M5 | PUT `/v1/series/{remote_id}/rating` | Bearer | `{"rating": <double>}` | ignored | `updateSeriesRating` (134-139) |
| M6 | DELETE `/v1/series/{remote_id}/rating` | Bearer | none (`DELETE(url)` with default body, an empty FormBody) | ignored | `updateSeriesRating` (142-145) |
| M7 | POST `/v1/lists/series/delete` | Bearer | JSON array: `[<remote_id>]` | ignored | `deleteSeriesFromList` (107-112) |
| M8 | POST `/v1/series/search` | none (client) | `{"search": "<query>", "filter_types": ["drama cd", "novel"]}` | `MUSearchResult` | `search` (162-167) |
| M9 | PUT `/v1/account/login` | none (client) | `{"username": "<u>", "password": "<p>"}` | `MULoginResponse` | `authenticate` (181-186) |

Notes:
- M3 and M4 body key order: `series` before `list_id`, and `status` after `list_id`. The `series` object contains only `id`.
- M4 `status.chapter` is `track.last_chapter_read.toInt()`.
- M9 accepts `username`, which after login is the uid; the login form uses the typed username, not the stored uid.
- Status 401 or 403 on any authenticated call throws `HttpException` (no re-login).
- No Accept header is set in these helpers; the default headers from `Requests.kt` are empty (`DEFAULT_HEADERS = Headers.Builder().build()`).

Response shapes (kotlinx field names; the Kotlin names match the wire names except where `@SerialName` is given):
- `MUListItem`: `series: {id: Long?, title: String?}?`, `list_id: Long?`, `status: {volume: Int?, chapter: Int?}?`, `priority: Int?`. All nullable with defaults `null`.
- `MURating`: `rating: Double?`.
- `MULoginResponse`: `context: {session_token: String, uid: Long}` (both required, non-null, no defaults).
- `MUSearchResult`: `results: [ { record: MURecord } ]` (both required).
- `MURecord`: `series_id: Long?`, `title: String?`, `url: String?`, `description: String?`, `image: MUImage?`, `type: String?`, `year: String?`, `bayesian_rating: Double?`, `rating_votes: Int?`, `latest_chapter: Int?`.
- `MUImage`: `url: MUUrl?`, `height: Int?`, `width: Int?`.
- `MUUrl`: `original: String?`, `thumb: String?`.
- `MUSeries`: `id: Long?`, `title: String?`.
- `MUStatus`: `volume: Int?`, `chapter: Int?`.

### 4.2 Simkl (API base `https://api.simkl.com`, web base `https://simkl.com`)
Headers on authenticated calls (SimklInterceptor.kt:22-26): `Authorization: Bearer <access_token>`, `simkl-api-key: <CLIENT_ID>`, `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`. Unauthenticated calls (search, token exchange) send no Simkl headers except the `client_id` query/body value. JSON bodies use `Content-Type: application/json; charset=utf-8` (`jsonMime`, `core/common/.../OkHttpExtensions.kt:22`).

| # | Method + URL | Auth | Request body / query | Response parsed | Caller (line) |
|---|---|---|---|---|---|
| S1 | GET `https://simkl.com/oauth/authorize?response_type=code&client_id=<CLIENT_ID>&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth` | browser | query only | (browser redirect) | `authUrl()` (SimklApi.kt:248-253) |
| S2 | POST `https://api.simkl.com/oauth/token` | none (client) | JSON object: `{"code":"<code>","client_id":"<CLIENT_ID>","client_secret":"<CLIENT_SECRET>","redirect_uri":"aniyomi://simkl-auth","grant_type":"authorization_code"}` | `SimklOAuth` `{"access_token": "<string>"}` | `accessTokenRequest` (SimklApi.kt:225-234) |
| S3 | GET `https://api.simkl.com/users/settings` | Bearer | none | `SimklUser` `{"account": {"id": <int>}}` | `getCurrentUser` (206-210) |
| S4 | POST `https://api.simkl.com/sync/watched` | Bearer | JSON array `[{"simkl": <remote_id>}]` | `List<SimklSyncWatched>` | `findLibAnime` (171-174) |
| S5 | GET `https://api.simkl.com/sync/all-items/<queryType>/<status>?date_from=<last_watched>` | Bearer | query `date_from` | `SimklSyncResult` | `findLibAnime` (191-193) |
| S6 | POST `https://api.simkl.com/sync/add-to-list` | Bearer | `{"<shows|movies>": [{"ids": {"simkl": <id>}, "to": "<simkl status string>"}]}` | ignored | `addToList` (59-61) |
| S7 | POST `https://api.simkl.com/sync/history/remove` | Bearer | `{"shows": [{"ids": {"simkl": <id>}, "seasons": [{"number": 1}]}]}` | ignored | `updateProgress` (89-91) |
| S8 | POST `https://api.simkl.com/sync/history` | Bearer | `{"shows": [{"ids": {"simkl": <id>}, "seasons": [{"number": 1, "episodes": [{"number": 1}, ..., {"number": N}]}]}]}` | ignored | `updateProgress` (93-95) |
| S9 | POST `https://api.simkl.com/sync/ratings` | Bearer | `{"<shows|movies>": [{"ids": {"simkl": <id>}, "rating": <int 0-10>}]}` | ignored | `updateRating` (81-83) |
| S10 | POST `https://api.simkl.com/sync/ratings/remove` | Bearer | same shape as S9, rating included (value 0) | ignored | `updateRating` (77-79) |
| S11 | GET `https://api.simkl.com/search/<anime|tv|movie>?q=<query>&extended=full&client_id=<CLIENT_ID>` | none (client) | query only | `List<SimklSearchResult>` | `searchAnime` (144-155) |
| S12 | GET (poster image) `https://simkl.in/posters/<poster>_m.webp` | none | none | image (loaded by the image loader, external) | `SimklSearchResult.toTrackSearch` (SimklSearch.kt:30) |

Notes on the S5 URL: `queryType` is `shows`, `anime` or `movies`. The `status` path segment is the `list` string from S4 (a Simkl wire status, for example `watching` or `plantowatch`). The URL is built with `Uri.Builder` so `date_from` is percent-encoded.

Response shapes:
- `SimklOAuth`: `access_token: String` (required, serial name `access_token`).
- `SimklUser`: `account: SimklUserAccount` (required); `SimklUserAccount.id: Int` (required).
- `SimklSyncWatched`: `result: Boolean?`, `last_watched: String?`, `list: String?`. All nullable, no defaults in the Kotlin (kotlinx treats a missing nullable field as null only when it has a default; here they have no default, so a missing key fails: PROPOSAL: treat missing as null, matching the intent).
- `SimklSyncResult`: `anime: List<SimklSyncItem>?`, `tv: List<SimklSyncItem>?`, `movies: List<SimklSyncItem>?`. No defaults.
- `SimklSyncItem`: `show: SimklSyncResultItem?`, `movie: SimklSyncResultItem?`, `total_episodes_count: Long?`, `watched_episodes_count: Double?`, `user_rating: Int?`. No defaults.
- `SimklSyncResultItem`: `title: String` (required), `ids: {simkl: Long}` (required).
- `SimklSearchResult`: `ids: {simkl_id: Long}` (required), `title_romaji: String?`, `title: String?`, `ep_count: Long?`, `poster: String?`, `all_titles: List<String>?`, `url: String` (required), `status: String?`, `type: String?`, `year: Int?`. Note the Kotlin class has no default values, so for kotlinx a missing nullable key is an error unless `explicitNulls = false` or `coerceInputValues` is set in the app Json; treat missing as null in Rust.

Nested naming: `SimklSyncWatched.lastWatched` maps `last_watched`; `SimklSyncItem.totalEpisodesCount` maps `total_episodes_count`, `watchedEpisodesCount` maps `watched_episodes_count`, `userRating` maps `user_rating`. `SimklSearchResult.titleRomaji` maps `title_romaji`, `epCount` maps `ep_count`, `allTitles` maps `all_titles`. `SimklSearchResult.ids.simklId` maps `simkl_id`.

---

## 5. Database tables touched (owned by area:05/03/04)

### 5.1 `manga_sync` (MU rows; `sync_id = 7`)
Source: `data/src/main/sqldelight/data/manga_sync.sq:3-21`.

```
CREATE TABLE manga_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_chapter_read REAL NOT NULL,
    total_chapters INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER AS Boolean DEFAULT 0 NOT NULL,
    UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);
```
Named queries used by the tracker flow: `insert` (13 columns, line 42-43), `update` (coalesce-based, lines 45-61, `WHERE _id = :id`), `delete` by `(manga_id, sync_id)`, `getTracksByMangaId`. Note the `update` uses `coalesce(:x, column)`, so a null never clears a column through this statement.

### 5.2 `anime_sync` (SK rows; `sync_id = 101`)
Source: `data/src/main/sqldelightanime/dataanime/anime_sync.sq:3-21`. Same columns; `last_episode_seen` and `total_episodes` instead of chapter names; `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`; `FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE`. Queries `insert`, `update` (coalesce), `delete`, `getTracksByAnimeId`.

Column mapping summary:
| Column | Track field (MU) | Track field (SK) |
|---|---|---|
| `_id` | id | id |
| `manga_id` / `anime_id` | manga_id / anime_id | anime_id |
| `sync_id` | tracker_id (7) | tracker_id (101) |
| `remote_id` | remote_id | remote_id |
| `library_id` | library_id (null for MU) | library_id (null for SK) |
| `title` | title | title |
| `last_chapter_read` | last_chapter_read | - |
| `last_episode_seen` | - | last_episode_seen |
| `total_chapters` | total_chapters (0) | - |
| `total_episodes` | - | total_episodes |
| `status` | status (0-4) | status (1-5) |
| `score` | score | score |
| `remote_url` | tracking_url (MU series URL) | tracking_url (`/type/id` after bind) |
| `start_date` | started_reading_date | started_watching_date |
| `finish_date` | finished_reading_date | finished_watching_date |
| `private` | private | private |

Note: `remote_url` is `NOT NULL`, so the Rust default is `""`, never null.

### 5.3 Other tables
- None are written by the two service files directly. `InsertMangaTrack` / `InsertAnimeTrack` (domain interactors) write these tables (external).

---

## 6. Files and folders on disk
- None written by the two services. No cache files, no export.
- Token and password live in the preference store (SQLite or XML in the Android app; for Kurobayomi, the preference DB of area:02).
- Covers: Simkl cover URLs are remote (`https://simkl.in/posters/...`); the app image loader (external) caches them. MU cover URLs are the `image.url.original` string from search.

---

## 7. JSON serialisation rules
- The Kotlin code uses kotlinx.serialization `Json` injected via injekt (`private val json: Json by injectLazy()`).
- Outgoing JSON is built with `buildJsonObject` / `buildJsonArray` (so key order equals insertion order). Keys appear exactly as written in section 4.
- Numbers: `put("id", track.remote_id)` writes a Long integer. `put("rating", track.score)` writes a Double (`7.5` is written as `7.5`; `7.0` is written as `7.0`). `put("rating", track.score.toInt())` (Simkl) writes an integer.
- Strings are JSON-escaped (for example `"drama cd"` stays as is).
- Incoming JSON: parsed via `parseAs` (`core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:134-143`), which streams from the response body and consumes it (`use`).
- The SK token is stored with `json.encodeToString(oauth)` (`{"access_token":"..."}`, compact).

---

## 8. Rust type sketches (for the DTO layer)

These are the wire types. Keep field names exactly as on the wire using `#[serde(rename = "...")]` where the Rust name differs.

```rust
// kuro-track/src/trackers/mangaupdates/dto.rs
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MuContext {
    pub session_token: String,
    pub uid: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MuLoginResponse {
    pub context: MuContext,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MuImage {
    pub url: Option<MuUrl>,
    pub height: Option<i32>,
    pub width: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MuUrl {
    pub original: Option<String>,
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MuSeries {
    pub id: Option<i64>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MuStatus {
    pub volume: Option<i32>,
    pub chapter: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MuListItem {
    pub series: Option<MuSeries>,
    #[serde(default)]
    pub list_id: Option<i64>,       // wire: list_id
    pub status: Option<MuStatus>,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MuRating {
    pub rating: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MuRecord {
    pub series_id: Option<i64>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub image: Option<MuImage>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub year: Option<String>,
    pub bayesian_rating: Option<f64>,
    pub rating_votes: Option<i32>,
    pub latest_chapter: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MuSearchResultItem {
    pub record: MuRecord,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MuSearchResult {
    pub results: Vec<MuSearchResultItem>,
}

// Request bodies (Serialize). Built as serde_json::json! to keep key order.
```

```rust
// kuro-track/src/trackers/simkl/dto.rs
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SimklOAuth {
    pub access_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimklUserAccount { pub id: i32 }

#[derive(Debug, Clone, Deserialize)]
pub struct SimklUser { pub account: SimklUserAccount }

#[derive(Debug, Clone, Deserialize)]
pub struct SimklSyncWatched {
    #[serde(default)] pub result: Option<bool>,
    #[serde(default)] pub last_watched: Option<String>,
    #[serde(default)] pub list: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimklIds { pub simkl: i64 }

#[derive(Debug, Clone, Deserialize)]
pub struct SimklSyncResultItem {
    pub title: String,
    pub ids: SimklIds,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimklSyncItem {
    #[serde(default)] pub show: Option<SimklSyncResultItem>,
    #[serde(default)] pub movie: Option<SimklSyncResultItem>,
    #[serde(default)] pub total_episodes_count: Option<i64>,
    #[serde(default)] pub watched_episodes_count: Option<f64>,
    #[serde(default)] pub user_rating: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SimklSyncResult {
    #[serde(default)] pub anime: Option<Vec<SimklSyncItem>>,
    #[serde(default)] pub tv: Option<Vec<SimklSyncItem>>,
    #[serde(default)] pub movies: Option<Vec<SimklSyncItem>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimklSearchIds { pub simkl_id: i64 }

#[derive(Debug, Clone, Deserialize)]
pub struct SimklSearchResult {
    pub ids: SimklSearchIds,
    #[serde(default)] pub title_romaji: Option<String>,
    #[serde(default)] pub title: Option<String>,
    #[serde(default)] pub ep_count: Option<i64>,
    #[serde(default)] pub poster: Option<String>,
    #[serde(default)] pub all_titles: Option<Vec<String>>,
    pub url: String,
    #[serde(default)] pub status: Option<String>,
    #[serde(default, rename = "type")] pub kind: Option<String>,
    #[serde(default)] pub year: Option<i32>,
}
```

Notes on the sketches:
- `SimklSyncItem` `watched_episodes_count` is `Double` on the wire (`Option<f64>`), while `total_episodes_count` is `Long`.
- `MuListItem.list_id` is nullable on the wire; the Kotlin field name is `listId` with `@SerialName("list_id")`.
- `SimklSyncResult::get_from_type` must accept only the response keys `anime`, `tv`, `movies` (see behaviour D-SK-1).
- Bodies for POST/PUT are built as `serde_json::Value` with insertion order preserved (enable `serde_json` `preserve_order` feature) so key order matches the Kotlin payloads, which keeps server-side behaviour identical and makes tests byte-comparable.

---

## 9. Value domains and ranges (for validation)

| Value | Domain | Enforced in Kotlin? |
|---|---|---|
| MU list id | {0,1,2,3,4} (-1 for rereading, never sent) | No (any Long is sent) |
| MU score | 0.0 or 1.0..10.0 in 0.1 steps (92 picker values) | Only through the picker list; `indexToScore(-1)` throws |
| MU chapter sent | `Int`, truncation of `last_chapter_read` | No range check |
| SK status | {1,2,3,4,5} | Yes for to-wire (throws); from-wire throws for unknown strings |
| SK rating | integer 0..10 | Not checked; `score.toInt()` |
| SK episodes sent | 1..`last_episode_seen.toInt()` | Not checked; 0 gives empty array |
| SK season | always 1 | constant |
| `total_episodes` | >= 0 (0 = unknown) | Not checked |

---

## 10. Secrets (security-relevant data)
- MU password (typed by the user) is sent over HTTPS once in M9 and never stored. The session token is stored (`pref_mangasync_password_7`).
- SK access token: stored in two places (`pref_mangasync_password_101` and the JSON under `track_token_101`). PROPOSAL: keep one copy in the secret store.
- SK `CLIENT_SECRET` is a constant in the binary (SimklApi.kt:238). It is sent in S2 over HTTPS.
- The Simkl user id (username field) is not secret.
