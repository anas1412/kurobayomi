# Area 29-tracker-myanimelist, part 1: data models and configuration

Paths are relative to `/home/blackbox/aniyomi-src` unless marked otherwise. There are no bit-flag layouts in this scope. Enums are stored as their numeric codes (section 1).

## 1. Status codes (stored as INTEGER in `manga_sync.status` and `anime_sync.status`)

Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt:36-44`.

| Name | Value | Kind | Valid for |
|---|---|---|---|
| `READING` | 1 | Long | manga |
| `WATCHING` | 11 | Long | anime |
| `COMPLETED` | 2 | Long | both |
| `ON_HOLD` | 3 | Long | both |
| `DROPPED` | 4 | Long | both |
| `PLAN_TO_READ` | 6 | Long | manga |
| `PLAN_TO_WATCH` | 16 | Long | anime |
| `REREADING` | 7 | Long | manga |
| `REWATCHING` | 17 | Long | anime |

Other values can be stored by other trackers in the same column. This tracker maps unknown values as described in `p1-behavior.md` section 1.3.

Rust type proposal (`kuro-track::mal::status`): a `#[repr(i64)] enum MalStatus` with these discriminants, plus `TryFrom<i64>` that returns `Err` for unknown codes. The DB layer stores the raw `i64`. See `p1-rust-design.md` section 3.1.

## 2. Remote wire enums (strings)

These are the strings MAL uses in JSON responses and form fields. They are not stored locally.

| Wire string | Direction | Used for |
|---|---|---|
| `reading` | in and out | manga `status` |
| `watching` | in and out | anime `status` |
| `completed` | in and out | both |
| `on_hold` | in and out | both |
| `dropped` | in and out | both |
| `plan_to_read` | in and out | manga `status` |
| `plan_to_watch` | in and out | anime `status` |
| any string | in only | `publishing_status` (manga and anime), `publishing_type` (manga and anime) |

The Kotlin source does not enumerate the MAL values for `status` (of the search and details response) or `media_type`. It only applies `replace("_", " ")` to them (`MyAnimeListApi.kt:141-142`, `:172-173`). The one literal it checks is `novel` in `publishing_type` (`MyAnimeListApi.kt:95`). The MAL API documentation could not be fetched while writing this plan, so no further value list is given here. Verify the live values before adding any enum (see `p1-edge-cases-and-tests.md`, fixtures). Kurobayomi stores these as free text.

## 3. DTOs (JSON models)

All classes are Kotlin `data class` with `kotlinx.serialization` `@Serializable`. Deserialization uses the shared `Json` with `ignoreUnknownKeys = true` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:175`). Unknown keys are dropped. Missing non-nullable keys without a default cause a decode error.

### 3.1 `MALOAuth` (`app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALOAuth.kt`)

| Kotlin field | JSON key | Type | Nullable | Default | Notes |
|---|---|---|---|---|---|
| `tokenType` | `token_type` | String | no | none | Always `Bearer` in practice. Not used. |
| `refreshToken` | `refresh_token` | String | no | none | Used by refresh request |
| `accessToken` | `access_token` | String | no | none | Sent as `Authorization: Bearer` |
| `expiresIn` | `expires_in` | Long | no | none | Seconds |
| `createdAt` | `created_at` | Long | no | `System.currentTimeMillis() / 1000` | Seconds since epoch. `@EncodeDefault` so it is always written when encoding. Absent in token responses, so it defaults to parse time. |
| `adjustedExpiresIn` | (not serialized, private body property) | Long | no | `expiresIn - 60` | Early expiry margin |
| `isExpired()` | (function) | Boolean | no | | `createdAt + adjustedExpiresIn < now_seconds` |

Persisted form: the JSON of the object, including `created_at`, in preference `__PRIVATE_track_token_1` (section 5).

Rust (`kuro-track::mal::auth::token`):

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct MalToken {
    pub token_type: String,
    pub refresh_token: String,
    pub access_token: String,
    pub expires_in: i64,
    #[serde(default = "now_secs")]
    pub created_at: i64,
}
```

### 3.2 `MALUser` (`.../dto/MALUser.kt`)

| Field | JSON | Type | Nullable | Default |
|---|---|---|---|---|
| `name` | `name` | String | no | none |

### 3.3 `MALSearchResult`, `MALSearchResultNode`, `MALSearchResultItem` (`.../dto/MALSearch.kt`)

- `MALSearchResult { data: List<MALSearchResultNode> }` (JSON `data`, no default, required)
- `MALSearchResultNode { node: MALSearchResultItem }` (JSON `node`, required)
- `MALSearchResultItem { id: Int }` (JSON `id`, required, 32-bit)

### 3.4 `MALManga` and `MALMangaCovers` (`.../dto/MALManga.kt`)

| Field | JSON | Type | Nullable | Default |
|---|---|---|---|---|
| `id` | `id` | Long | no | none |
| `title` | `title` | String | no | none |
| `synopsis` | `synopsis` | String | no | `""` |
| `numChapters` | `num_chapters` | Long | no | none (0 means unknown on MAL) |
| `mean` | `mean` | Double | no | `-1.0` |
| `covers` | `main_picture` | MALMangaCovers | yes | null |
| `status` | `status` | String | no | none |
| `mediaType` | `media_type` | String | no | none |
| `startDate` | `start_date` | String | yes | null |

`MALMangaCovers { large: String = "" }` (JSON `large`).

### 3.5 `MALAnime` and `MALAnimeCovers` (`.../dto/MALAnime.kt`)

Same as 3.4 with `numEpisodes` (JSON `num_episodes`, Long, required) in place of `numChapters`, and `MALAnimeCovers`.

### 3.6 `MALListMangaItem`, `MALListMangaItemStatus` (`.../dto/MALList.kt`)

`MALListMangaItem`:
| Field | JSON | Type | Nullable | Default |
|---|---|---|---|---|
| `numChapters` | `num_chapters` | Long | no | none |
| `myListStatus` | `my_list_status` | MALListMangaItemStatus | yes | null |

`MALListMangaItemStatus`:
| Field | JSON | Type | Nullable | Default |
|---|---|---|---|---|
| `isRereading` | `is_rereading` | Boolean | no | none |
| `status` | `status` | String | no | none |
| `numChaptersRead` | `num_chapters_read` | Double | no | none |
| `score` | `score` | Int | no | none (0 to 10) |
| `startDate` | `start_date` | String | yes | null |
| `finishDate` | `finish_date` | String | yes | null |

### 3.7 `MALListAnimeItem`, `MALListAnimeItemStatus` (`.../dto/MALList.kt`)

`MALListAnimeItem`: `numEpisodes` (`num_episodes`, Long, required), `myListStatus` (`my_list_status`, nullable).

`MALListAnimeItemStatus`:
| Field | JSON | Type | Nullable | Default |
|---|---|---|---|---|
| `isRewatching` | `is_rewatching` | Boolean | no | none |
| `status` | `status` | String | no | none |
| `numEpisodesWatched` | `num_episodes_watched` | Double | no | none |
| `score` | `score` | Int | no | none |
| `startDate` | `start_date` | String | yes | null |
| `finishDate` | `finish_date` | String | yes | null |

Note: the response key is `num_episodes_watched`. The form field used when writing is `num_watched_episodes`. Both are as written in source.

### 3.8 `MALUserSearchResult`, `MALUserSearchItem`, `MALUserSearchItemNode`, `MALUserSearchPaging` (`.../dto/MALUserListSearch.kt`)

- `MALUserSearchResult { data: List<MALUserSearchItem>, paging: MALUserSearchPaging }` (both required)
- `MALUserSearchItem { node: MALUserSearchItemNode }` (required)
- `MALUserSearchItemNode { id: Int, title: String }` (both required)
- `MALUserSearchPaging { next: String? }` (JSON `next`, nullable without default; **the key itself is required**, see `p1-edge-cases-and-tests.md`)

The `list_status` field requested in the URL is not declared here, so it is ignored by `ignoreUnknownKeys`.

## 4. Local track rows (domain and DB)

### 4.1 Database tables touched by MAL values

Schema source: `data/src/main/sqldelight/data/manga_sync.sq` and `data/src/main/sqldelightanime/dataanime/anime_sync.sq`.

`manga_sync`:
```
_id INTEGER NOT NULL PRIMARY KEY
manga_id INTEGER NOT NULL                  -- FK mangas(_id) ON DELETE CASCADE
sync_id INTEGER NOT NULL                   -- tracker id; MAL = 1
remote_id INTEGER NOT NULL                 -- MAL numeric id
library_id INTEGER                         -- null for MAL
title TEXT NOT NULL
last_chapter_read REAL NOT NULL
total_chapters INTEGER NOT NULL
status INTEGER NOT NULL                    -- section 1 codes
score REAL NOT NULL                        -- 0..10, -1 = unset in search results
remote_url TEXT NOT NULL                   -- https://myanimelist.net/manga/{id}
start_date INTEGER NOT NULL                -- epoch ms, local midnight, 0 = unset
finish_date INTEGER NOT NULL
private INTEGER AS Boolean DEFAULT 0 NOT NULL
UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE
FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
```

`anime_sync`: same shape with `anime_id`, `last_episode_seen REAL`, `total_episodes INTEGER`, `remote_url = https://myanimelist.net/anime/{id}`, `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`, `FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE`.

Queries used by callers (not in scope, shown for the mapping): `insert`, `update` (with `coalesce(:x, column)` for every field), `delete` (by `manga_id` and `sync_id`), `getTracksByMangaId`, `getTrackById`. The `update` query coalesces each field, so passing null leaves the column unchanged.

Rust mapping (proposal, owned by area:05-data-repos-entries-items and area:04-db-anime-schema-migrations tables): the same columns, with `sync_id` = 1 for MAL and `remote_url` as `TEXT`.

### 4.2 Domain model fields (`app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt` and `.../anime/model/AnimeTrack.kt`)

Manga domain track: `id: Long?`, `mangaId: Long`, `trackerId: Long`, `remoteId: Long`, `libraryId: Long?`, `title: String`, `lastChapterRead: Double`, `totalChapters: Long`, `status: Long`, `score: Double`, `remoteUrl: String`, `startDate: Long`, `finishDate: Long`, `private: Boolean`.

Anime domain track: `id`, `animeId`, `trackerId`, `remoteId`, `libraryId`, `title`, `lastEpisodeSeen: Double`, `totalEpisodes: Long`, `status`, `score`, `remoteUrl`, `startDate`, `finishDate`, `private`.

Mapping: `toDbTrack()` and `toDomainTrack(idRequired)`. `toDomainTrack` returns null when `id` is null and `idRequired` is true (`MangaTrack.kt` `toDomainTrack`). Anime `tracker_id` is converted with `toLong()` (`domain/track/anime/model/AnimeTrack.kt`), manga keeps the Long.

### 4.3 Search result holders (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt`, `AnimeTrackSearch.kt`)

Extra fields beyond the DB track: `authors: List<String> = []`, `artists: List<String> = []`, `cover_url: String = ""`, `summary: String = ""`, `publishing_status: String = ""`, `publishing_type: String = ""`, `start_date: String = ""`. Defaults on create: `score = -1.0`, `status = 0`, `tracker_id = 1`, `last_*` = 0.0, `total_* = 0`, dates = 0. `equals`/`hashCode` use `(manga_id|anime_id, tracker_id, remote_id)`.

Note: `tracking_url` and `title` are `lateinit`. The MAL code always sets them before returning.

### 4.4 `copyPersonalFrom` fields

`MangaTrack` interface (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:37-44`) copies: `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date`, and `private` (when `copyRemotePrivate` is true, the default).

`AnimeTrack` interface (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt:37-44`) copies: `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, and `private`.

Domain `copyPersonalFrom` (`domain/track/manga/model/MangaTrack.kt:6-15`, `domain/track/anime/model/AnimeTrack.kt:6-15`) does the same for the domain types.

## 5. Preferences (keys, types, defaults)

Preference store: `core/common/src/main/java/tachiyomi/core/common/preference/`. Keys beginning with `__PRIVATE_` are excluded from backups (`Preference.kt:29-31` `isPrivate`; prefix constant `PRIVATE_PREFIX = "__PRIVATE_"` at `Preference.kt:48`). Source of the keys: `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`.

### 5.1 MAL-specific keys (tracker id = 1)

| Stored key (after `privateKey`) | Type | Default | Written by | Read by |
|---|---|---|---|---|
| `__PRIVATE_pref_mangasync_username_1` | String | `""` | `setCredentials` (`TrackPreferences.kt:29-33`) | `getUsername`, `isLoggedIn` |
| `__PRIVATE_pref_mangasync_password_1` | String | `""` | `setCredentials`. Holds the **OAuth access token**, not a password. | `getPassword`, `isLoggedIn` |
| `__PRIVATE_pref_tracker_auth_expired_1` | Boolean | `false` | `setAuthExpired`, `setCredentials` (set to false) | `getIfAuthExpired` |
| `__PRIVATE_track_token_1` | String | `""` | `saveOAuth`, `logout` (delete, then `"null"` written, see behaviour 4.11) | `loadOAuth` |

The key format is `Preference.privateKey("pref_mangasync_username_${tracker.id}")` (`TrackPreferences.kt:14-16`). The token key is `Preference.privateKey("track_token_${tracker.id}")` (`TrackPreferences.kt:35`). The auth-expired key is `Preference.privateKey("pref_tracker_auth_expired_${tracker.id}")` (`TrackPreferences.kt:24-27`).

Note: the `pref_mangasync_password_1` key name is misleading. It holds the access token (`MyAnimeList.kt:256`).

### 5.2 Shared tracker preferences that change MAL behaviour (owned by area:27-tracking-core, listed here because they gate MAL calls)

| Key | Type | Default | Getter | Effect on MAL |
|---|---|---|---|---|
| `pref_auto_update_manga_sync_key` | Boolean | `true` | `autoUpdateTrack()` (`TrackPreferences.kt:39`) | If false, no progress is pushed to MAL from readers or players (`ReaderViewModel.kt:920`, `PlayerViewModel.kt:1949`, `ExternalIntents.kt:479`, `:525`) |
| `track_on_adding_to_library` | Boolean | `true` | `trackOnAddingToLibrary()` (`TrackPreferences.kt:41`) | Opens the track menu on add |
| `show_next_episode_airing_time` | Boolean | `true` | `showNextEpisodeAiringTime()` (`TrackPreferences.kt:43-46`) | Shows airing time (uses AniChart, which uses the MAL id) |
| `pref_auto_update_manga_on_mark_read` | enum `AutoTrackState`, values `ALWAYS`, `ASK`, `NEVER` | `ALWAYS` | `autoUpdateTrackOnMarkRead()` (`TrackPreferences.kt:48-51`) | Controls progress push on "mark as read/watched" |

`AutoTrackState` (`app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt`):
- `ALWAYS` label `MR.strings.lock_always` = "Always"
- `ASK` label `MR.strings.default_category_summary` = "Always ask"
- `NEVER` label `MR.strings.lock_never` = "Never"

Stored with `PreferenceStore.getEnum`; the encoding is owned by area:02-preferences-system.

### 5.3 Keys for the OAuth client (not preferences)

`CLIENT_ID = "686b980ff4240fccce7f6a654cea07ce"` (`MyAnimeListApi.kt:383`) is a constant, not a preference.

## 6. Files, folders and formats on disk

Android side (for reference):
- The preference store is an Android SharedPreferences file (managed by the app, not in scope). MAL values live there under the keys in section 5.
- Deep link: `app/src/main/AndroidManifest.xml`, `TrackLoginActivity` (`android:exported="true"`), `<data android:scheme="aniyomi"/>` plus hosts `myanimelist-auth`, `anilist-auth`, `bangumi-auth`, `shikimori-auth`, `simkl-auth`.
- Logo: `app/src/main/res/drawable-nodpi/ic_tracker_mal.webp`.
- No MAL-specific files are written by the source.

Kurobayomi side (proposal):
- Non-secret MAL settings: `$XDG_CONFIG_HOME/kurobayomi/tracking/myanimelist.toml` (default `~/.config/kurobayomi/tracking/myanimelist.toml`). Keys: `username` (string), `auth_expired` (bool, default false), `pending_verifier` (string, present only during a login, see T-29a-038). File mode 0600.
- Token JSON (same fields as 3.1): Linux Secret Service item (libsecret, `keyring` crate). Attributes `{"app":"kurobayomi","tracker":"myanimelist","kind":"oauth-token"}`. Fallback when no secret service is running: `$XDG_DATA_HOME/kurobayomi/secrets/myanimelist.json` (default `~/.local/share/kurobayomi/secrets/myanimelist.json`), mode 0600, directory mode 0700.
- Tracker row data: in the Kurobayomi SQLite database, tables `manga_sync` / `anime_sync` as in section 4.1 (owned by area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations).

## 7. Network endpoints

All URLs exactly as the source builds them. `{id}` is `remote_id` (Long) or the search id (Int). Query parameters are shown in the order the source adds them. Encoding is done by Android `Uri.Builder` (spaces as `%20`) in the source. The Rust port uses `url::form_urlencoded` (spaces as `+`). MAL accepts both.

| # | Method | URL | Query / body (exact keys in order) | Headers | Auth | Response (fields used) | Source |
|---|---|---|---|---|---|---|---|
| E1 | GET (browser, not HTTP from app) | `https://myanimelist.net/v1/oauth2/authorize` | `client_id=686b980ff4240fccce7f6a654cea07ce`, `code_challenge=<verifier>`, `response_type=code` | none | none | redirect with `code` query param | `MyAnimeListApi.kt:392-396` |
| E2 | POST | `https://myanimelist.net/v1/oauth2/token` | form: `client_id`, `code`, `code_verifier`, `grant_type=authorization_code` | none | none | `MALOAuth` JSON | `MyAnimeListApi.kt:50-64` |
| E3 | POST | `https://myanimelist.net/v1/oauth2/token` | form: `client_id`, `refresh_token`, `grant_type=refresh_token` | `Authorization: Bearer <old access token>` | manual | `MALOAuth` JSON | `MyAnimeListApi.kt:408-423` |
| E4 | GET | `https://api.myanimelist.net/v2/users/@me` | none | none (Bearer added) | Bearer | `{"name":"..."}` | `MyAnimeListApi.kt:66-79` |
| E5 | GET | `https://api.myanimelist.net/v2/manga` | `q=<query.take(64)>`, `nsfw=true` | Bearer | Bearer | `{"data":[{"node":{"id":N}}]}` | `MyAnimeListApi.kt:81-98` |
| E6 | GET | `https://api.myanimelist.net/v2/anime` | `q=<query.take(64)>`, `q=<query>`, `nsfw=true` | Bearer | Bearer | same shape | `MyAnimeListApi.kt:100-117` |
| E7 | GET | `https://api.myanimelist.net/v2/manga/{id}` | `fields=id,title,synopsis,num_chapters,mean,main_picture,status,media_type,start_date` | Bearer | Bearer | `MALManga` | `MyAnimeListApi.kt:119-148` |
| E8 | GET | `https://api.myanimelist.net/v2/anime/{id}` | `fields=id,title,synopsis,num_episodes,mean,main_picture,status,media_type,start_date` | Bearer | Bearer | `MALAnime` | `MyAnimeListApi.kt:150-179` |
| E9 | GET | `https://api.myanimelist.net/v2/manga/{id}` | `fields=num_chapters,my_list_status{start_date,finish_date}` | Bearer | Bearer | `MALListMangaItem` | `MyAnimeListApi.kt:251-267` |
| E10 | GET | `https://api.myanimelist.net/v2/anime/{id}` | `fields=num_episodes,my_list_status{start_date,finish_date}` | Bearer | Bearer | `MALListAnimeItem` | `MyAnimeListApi.kt:269-285` |
| E11 | GET | `https://api.myanimelist.net/v2/users/@me/mangalist` | `fields=list_status{start_date,finish_date}`, `limit=250`, `offset=<n>` (only if n > 0) | Bearer | Bearer | `MALUserSearchResult` | `MyAnimeListApi.kt:323-342` |
| E12 | PUT | `https://api.myanimelist.net/v2/manga/{remote_id}/my_list_status` | form: `status`, `is_rereading`, `score`, `num_chapters_read`, [`start_date`], [`finish_date`] | Bearer | Bearer | `MALListMangaItemStatus` | `MyAnimeListApi.kt:181-206` |
| E13 | PUT | `https://api.myanimelist.net/v2/anime/{remote_id}/my_list_status` | form: `status`, `is_rewatching`, `score`, `num_watched_episodes`, [`start_date`], [`finish_date`] | Bearer | Bearer | `MALListAnimeItemStatus` | `MyAnimeListApi.kt:208-233` |
| E14 | DELETE | `https://api.myanimelist.net/v2/manga/{remote_id}/my_list_status` | none | Bearer | Bearer | ignored | `MyAnimeListApi.kt:235-241` |
| E15 | DELETE | `https://api.myanimelist.net/v2/anime/{remote_id}/my_list_status` | none | Bearer | Bearer | ignored | `MyAnimeListApi.kt:243-249` |

Form field values:
- `status`: wire string from `p1-behavior.md` section 1.3 (manga) or 1.3 anime table. Default `reading` / `watching`.
- `is_rereading` / `is_rewatching`: `"true"` or `"false"`.
- `score`: `Double.toString()`, for example `"0.0"`, `"8.0"`, `"-1.0"`.
- `num_chapters_read`: `last_chapter_read.toInt().toString()`, for example `"12"`.
- `num_watched_episodes`: `last_episode_seen.toInt().toString()`.
- `start_date` / `finish_date`: `yyyy-MM-dd` in local time, or the empty string `""` when the local epoch is 0. Omitted only when formatting throws.

Caching headers: `GET(...)` helper sets `Cache-Control: max-age=600` (`Requests.kt:16`). Used by E5, E6, E7, E8, E9, E10. Raw `Request.Builder().get()` (E4, E11) and `put` (E12, E13) have no cache header. The OkHttp cache is 5 MiB (`NetworkHelper.kt:26-29`).

Responses that are JSON but use fields not in the DTO are ignored (`ignoreUnknownKeys`).

Error responses: any non-2xx is `HttpException(code)` with message `HTTP error <code>` (`core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:109-118`). The token endpoint returns JSON error bodies that are not parsed.

Cross-area endpoints that consume MAL ids (not in scope, listed for completeness):
- E16 POST `https://graphql.anilist.co` (AniChart airing lookup). Body: JSON `{"query": "query { Media(idMal:<id>,type: ANIME) { id } }"}` (built with `buildJsonObject` in `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:53-68`). Response parsed by substring, not JSON (`AniChartApi.kt:72-74`).
- AniSkip lookup (`PlayerViewModel.kt:2044`, `AniSkipApi`) uses the MAL id as an integer.

## 8. Limits and constants (quick reference)

| Constant | Value | Source |
|---|---|---|
| Query length cap | 64 UTF-16 units | `MyAnimeListApi.kt:85`, `:104` |
| List page size | 250 | `MyAnimeListApi.kt:388` |
| Early expiry margin | 60 s | `MALOAuth.kt:22` |
| Code verifier bytes | 50 (68 chars base64url) | `PkceUtil.kt:9` |
| Score list | `"0"` to `"10"` | `MyAnimeList.kt:49-51` |
| Tracker id | 1 | `TrackerManager.kt:27` |
| Logo colour | `#2E51A2` | `MyAnimeList.kt:63` |
| Search id prefix | `id:` | `MyAnimeList.kt:46` |
| Search list prefix | `my:` | `MyAnimeList.kt:47` |
| Default score on bind-new | 0.0 | `MyAnimeList.kt:184`, `:204` |
| Default score on search result | `mean`, or -1.0 when absent | `MyAnimeListApi.kt:138`, `:169` |

## 9. Config file example (Kurobayomi, proposal)

```toml
# $XDG_CONFIG_HOME/kurobayomi/tracking/myanimelist.toml
username = "example_user"
auth_expired = false
# pending_verifier is present only between "open browser" and "redirect received"
# pending_verifier = "..."
```

The token is never in this file. It is in the secret store (section 6).
