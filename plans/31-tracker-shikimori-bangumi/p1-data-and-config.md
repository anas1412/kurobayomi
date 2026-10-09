# Area 31-tracker-shikimori-bangumi, part 1: data models, preferences, storage, endpoints

Source root `/home/blackbox/aniyomi-src`. Repo-relative paths. Server facts are marked **[verified]** and cite the OpenAPI file `bgm_v0.yaml` (bangumi/api `open-api/v0.yaml`), the Bangumi auth doc `How-to-Auth.md` (bangumi/api `docs-raw/How-to-Auth.md`), or Shikimori source (shikimori/shikimori, cited by path). p1-topic-remote-api-verification.md lists how each was checked.

Contents: 1 DTOs (wire shapes); 2 track models (in-app, database, domain, search); 3 status and score tables; 4 preference keys; 5 SQL tables; 6 backup encoding; 7 network endpoints; 8 OAuth parameters; 9 logo assets; 10 Linux storage mapping.

---

## 1. DTOs (wire shapes)

The app's JSON decoder is `Json { ignoreUnknownKeys = true; explicitNulls = false }` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:173-177`). Consequences used everywhere below:
- Unknown keys are ignored.
- A nullable property may be absent from the JSON and decodes as `null`.
- A non-null property with no default must be present, or decoding fails with `MissingFieldException`.
- A property with a default may be absent.

Column meanings: **Req** = must be present (non-null, no default). **Opt** = nullable, may be absent. **Def** = has a default, may be absent.

### 1.1 Shikimori

`SMOAuth` (`app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMOAuth.kt:7-18`):

| Kotlin | JSON name | Type | Req/Opt/Def | Notes |
|---|---|---|---|---|
| `accessToken` | `access_token` | String | Req | |
| `tokenType` | `token_type` | String | Req | always `Bearer` in practice |
| `createdAt` | `created_at` | Long | Req | epoch seconds, from the server |
| `expiresIn` | `expires_in` | Long | Req | 86400 for Shikimori **[verified]** `doorkeeper.rb:70` |
| `refreshToken` | `refresh_token` | String? | Opt | Doorkeeper `use_refresh_token` **[verified]** `doorkeeper.rb:175` |

Expiry helper: `isExpired() = now_s > createdAt + expiresIn - 3600` (`SMOAuth.kt:21`).

`SMEntry` (`app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMEntry.kt:9-22`) from `GET /api/mangas/{id}`, `GET /api/animes/{id}`, and each element of the search arrays:

| Kotlin | JSON name | Type | Req/Opt | Notes |
|---|---|---|---|---|
| `id` | `id` | Long | Req | catalogue id |
| `name` | `name` | String | Req | romanised or original title |
| `chapters` | `chapters` | Long? | Opt | used only for manga; `!!` in mapping (D21) |
| `episodes` | `episodes` | Long? | Opt | used only for anime; `!!` in mapping |
| `image` | `image` | `SUEntryCover` | Req | object with `preview` |
| `score` | `score` | Double | Req | catalogue average, e.g. 7.84 |
| `url` | `url` | String | Req | path such as `/mangas/<slug>`, prefixed with `https://shikimori.one` |
| `status` | `status` | String | Req | catalogue status string (e.g. `released`, `ongoing`); passed through |
| `kind` | `kind` | String | Req | e.g. `manga`, `tv`; passed through as `publishing_type` |
| `airedOn` | `aired_on` | String? | Opt | `@SerialName("aired_on")`; `start_date = airedOn ?: ""` |

`SUEntryCover` (`SMEntry.kt:54-57`): `preview: String` (Req). Cover URL = `"https://shikimori.one" + preview`.

`SMUserListEntry` (`app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMUserListEntry.kt:9-16`), element of `GET /api/v2/user_rates`:

| Kotlin | JSON name | Type | Req/Opt | Notes |
|---|---|---|---|---|
| `id` | `id` | Long | Req | user-rate id (library id) |
| `chapters` | `chapters` | Double | Req | progress for manga (Double, not Int) |
| `episodes` | `episodes` | Double | Req | progress for anime |
| `score` | `score` | Int | Req | 0..10; server clamps (D8) |
| `status` | `status` | String | Req | one of the six API strings (section 3.1) |

`SMAddEntryResponse` (`app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMAddEntryResponse.kt:5-8`): `id: Long` (Req). The POST response of `/api/v2/user_rates` is the user-rate object, so `id` is the library id.

`SMUser` (`app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMUser.kt:5-8`): `id: Int` (Req). Response of `GET /api/users/whoami`. Only `id` is read; `Int` is enough for Shikimori ids in practice, Rust should use `i64`.

Server user-rate object **[verified]** `app/models/user_rate.rb` `enum status: { planned: 0, watching: 1, rewatching: 9, completed: 2, on_hold: 3, dropped: 4 }`; validations `validates :user_id, uniqueness: { scope: %i[target_id target_type] }`; `MAXIMUM_SCORE = 10`, `MAXIMUM_EPISODES = 9999`, `MAXIMUM_TEXT_SIZE = 4096`.

### 1.2 Bangumi

`BGMOAuth` (`app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMOAuth.kt:8-22`):

| Kotlin | JSON name | Type | Req/Opt/Def | Notes |
|---|---|---|---|---|
| `accessToken` | `access_token` | String | Req | |
| `tokenType` | `token_type` | String | Req | `Bearer` |
| `createdAt` | `created_at` | Long | Def | default `System.currentTimeMillis() / 1000`, `@EncodeDefault` (always written when encoding) |
| `expiresIn` | `expires_in` | Long | Req | 604800 **[verified]** `How-to-Auth.md` |
| `refreshToken` | `refresh_token` | String? | Opt | |
| `userId` | `user_id` | Long? | Opt | present on the token response example, absent on the refresh example; never read by the app (D15) |

Expiry helper: `isExpired() = now_s > createdAt + expiresIn - 3600` (`BGMOAuth.kt:25`).

Note: because `created_at` is a default that is computed at decode time, a decoded response gets `createdAt` = time of decoding. Stored tokens keep the value they were encoded with.

`BGMCollectionResponse` (`app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMCollectionResponse.kt:7-17`), response of `GET /v0/users/{username}/collections/{subject_id}` **[verified]** `UserSubjectCollection` schema (`bgm_v0.yaml` around line 3982), required keys there: `subject_id`, `subject_type`, `rate`, `type`, `tags`, `ep_status`, `vol_status`, `updated_at`, `private`:

| Kotlin | JSON name | Type | Req/Opt/Def | Notes |
|---|---|---|---|---|
| `rate` | `rate` | Int? | Opt | 0..10; 0 means none |
| `type` | `type` | Int? | Opt | collection type 1..5 (3.2) |
| `epStatus` | `ep_status` | Int? | Def = 0 | watched progress for the subject; for books, pages or chapters |
| `volStatus` | `vol_status` | Int? | Def = 0 | not used |
| `private` | `private` | Boolean | Def = false | parsed, never mapped (D14) |
| `subject` | `subject` | `BGMSlimSubject?` | Def = null | |

`BGMSlimSubject` (`BGMCollectionResponse.kt:30-33`): `volumes: Int?`, `eps: Int?` (both Opt, no default). The server's `SlimSubject` schema lists more keys, which are ignored.

`BGMSearchResult` (`app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMSearch.kt:8-14`), response of `POST /v0/search/subjects`:

| Kotlin | JSON name | Type | Req/Def |
|---|---|---|---|
| `total` | `total` | Int | Req |
| `limit` | `limit` | Int | Req |
| `offset` | `offset` | Int | Req |
| `data` | `data` | `List<BGMSubject>` | Def = emptyList() |

`BGMSubject` (`BGMSearch.kt:16-30`):

| Kotlin | JSON name | Type | Req/Def | Notes |
|---|---|---|---|---|
| `id` | `id` | Long | Req | subject id |
| `nameCn` | `name_cn` | String | Req | Chinese name; blank is allowed (`ifBlank` picks `name`) |
| `name` | `name` | String | Req | original name |
| `summary` | `summary` | String? | Opt | free text, may contain newlines |
| `date` | `date` | String? | Opt | `YYYY-MM-DD` (comment in source) |
| `images` | `images` | `BGMSubjectImages?` | Opt | |
| `volumes` | `volumes` | Long | Def = 0 | |
| `eps` | `eps` | Long | Def = 0 | episodes or chapters count |
| `rating` | `rating` | `BGMSubjectRating?` | Opt | |
| `platform` | `platform` | String? | Opt | manga search keeps only results whose `platform` is null or `"漫画"` (`BangumiApi.kt:152`) |

`BGMSubjectImages` (`BGMSearch.kt:64-66`): `common: String?` (Opt). `BGMSubjectRating` (`BGMSearch.kt:70-72`): `score: Double?` (Opt).

`BGMUser` (`app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMUser.kt:7-9`), response of `GET /v0/me`: `username: String` (Req). The server's `User` schema (`bgm_v0.yaml` around line 2052) also has `id`, `nickname`, `user_group`, `avatar`, `sign`; the app ignores them, and `/v0/me` adds `email`, `reg_time`, `time_offset`.

---

## 2. Track models

### 2.1 In-app database model (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt` and `app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt`)

`MangaTrack` interface (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:7-35`) and `MangaTrackImpl` defaults:

| Field | Kotlin type | Default in `MangaTrackImpl` | Notes |
|---|---|---|---|
| `id` | Long? | null | DB `_id` |
| `manga_id` | Long | 0 | |
| `tracker_id` | Long | 0 | 4 Shikimori, 5 Bangumi |
| `remote_id` | Long | 0 | catalogue id (see D17 for the overwrite) |
| `library_id` | Long? | null | user-rate id (Shikimori); null for Bangumi |
| `title` | String | lateinit | |
| `last_chapter_read` | Double | 0.0 | REAL |
| `total_chapters` | Long | 0 | |
| `score` | Double | 0.0 | `MangaTrackSearch` overrides to -1.0 (`MangaTrackSearch.kt`, line 101) |
| `status` | Long | 0 | per tracker (section 3) |
| `started_reading_date` | Long | 0 | epoch millis |
| `finished_reading_date` | Long | 0 | epoch millis |
| `tracking_url` | String | "" | |
| `private` | Boolean | false | |

`AnimeTrack` interface (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt:7-35`) and `AnimeTrackImpl` defaults: same with `last_episode_seen`, `total_episodes`, `started_watching_date`, `finished_watching_date`; `score` default 0.0 (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrackImpl.kt:23`). `AnimeTrackSearch` overrides `score = -1.0` (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt:25`).

Search-result-only fields (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt` and `AnimeTrackSearch.kt`): `authors: List<String>` (empty), `artists: List<String>` (empty), `cover_url: String` (""), `summary: String` (""), `publishing_status: String` (""), `publishing_type: String` (""), `start_date: String` (""). Equality and hashing use `(manga_id|anime_id, tracker_id, remote_id)` only (`MangaTrackSearch.kt:127-145`).

Copy helpers (`MangaTrack.copyPersonalFrom(other, copyRemotePrivate = true)`, `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:36-44`): copies `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date`, and `private` when `copyRemotePrivate`. Anime (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt:37-44`): `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, `private`.

### 2.2 Domain model (`domain/src/main/java/tachiyomi/domain/track/...`)

`data class MangaTrack` (`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt:5-20`): `id: Long`, `mangaId: Long`, `trackerId: Long`, `remoteId: Long`, `libraryId: Long?`, `title: String`, `lastChapterRead: Double`, `totalChapters: Long`, `status: Long`, `score: Double`, `remoteUrl: String`, `startDate: Long`, `finishDate: Long`, `private: Boolean`. Implements `Serializable`.

`data class AnimeTrack` (`domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20`): same with `animeId`, `lastEpisodeSeen: Double`, `totalEpisodes: Long`.

Conversion (`app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt`): `toDbTrack()` and `toDomainTrack(idRequired)` map names one to one. `toDomainTrack(idRequired = false)` returns `id = -1` when the database id is null. `toDomainTrack(idRequired = true)` returns null when id is null.

---

## 3. Status and score tables

### 3.1 Shikimori

Internal (persisted in `status`):

| Name | Value | API string (`ShikimoriUtils.kt`) | Server enum value (`user_rate.rb`) | Manga label key | Anime label key |
|---|---|---|---|---|---|
| READING | 1 | `watching` | 1 | `MR.strings.reading` | `AYMR.strings.watching` |
| COMPLETED | 2 | `completed` | 2 | `MR.strings.completed` | `MR.strings.completed` |
| ON_HOLD | 3 | `on_hold` | 3 | `MR.strings.on_hold` | `MR.strings.on_hold` |
| DROPPED | 4 | `dropped` | 4 | `MR.strings.dropped` | `MR.strings.dropped` |
| PLAN_TO_READ | 5 | `planned` | 0 | `MR.strings.plan_to_read` | `AYMR.strings.plan_to_watch` |
| REREADING | 6 | `rewatching` | 9 | `MR.strings.repeating` | `AYMR.strings.repeating_anime` |

Status list shown to the user: `[1, 2, 3, 4, 5, 6]`. Rereading id and rewatching id: `6`. Completion id: `2`. Reading id and watching id: `1`.

### 3.2 Bangumi

Internal (persisted in `status`):

| Name | Value | Collection `type` (both directions) | Server meaning (`SubjectCollectionType`) | Manga label key | Anime label key |
|---|---|---|---|---|---|
| PLAN_TO_READ | 1 | 1 | 想看 (wish) | `MR.strings.plan_to_read` | `AYMR.strings.plan_to_watch` |
| COMPLETED | 2 | 2 | 看过 (done) | `MR.strings.completed` | `MR.strings.completed` |
| READING | 3 | 3 | 在看 (doing) | `MR.strings.reading` | `AYMR.strings.watching` |
| ON_HOLD | 4 | 4 | 搁置 (on hold) | `MR.strings.on_hold` | `MR.strings.on_hold` |
| DROPPED | 5 | 5 | 抛弃 (dropped) | `MR.strings.dropped` | `MR.strings.dropped` |

Status list shown to the user: `[3, 2, 4, 5, 1]`. Rereading and rewatching: `-1` (not supported). Completion: `2`. Reading and watching: `3`.

Subject types **[verified]** `SubjectType` (`bgm_v0.yaml` line 3806): `1` Book, `2` Anime, `3` Music, `4` Game, `6` Real (no `5`). Manga search uses `type: [1]`; anime search uses `[2]`.

Episode collection enum **[verified]** `EpisodeCollectionType` (`bgm_v0.yaml` around line 2340): `0` none, `1` wish, `2` done, `3` dropped. Used only by the episode endpoints in D7.

### 3.3 Scores

Both: `getScoreList()` = `["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]` (11 items), `indexToScore(i) = i.toDouble()`, `displayScore(t) = t.score.toInt().toString()`, `get10PointScore(t) = t.score` (default in `MangaTracker.kt:43-45`, `AnimeTracker.kt:43-45`).

Write coercion: Bangumi `rate = score.toInt().coerceIn(0, 10)` **[verified]** `0` removes the rating. Shikimori `score = score.toInt()`, no coercion. Server clamping **[verified]** `user_rate.rb` `score_changed`: blank → 0; `score > 10` or negative → previous value.

---

## 4. Preference keys

Stored through `PreferenceStore` (`core/common/src/main/java/tachiyomi/core/common/preference/`). Keys built in `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`.

Private prefix: `Preference.privateKey(key)` prepends `__PRIVATE_` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:32-34`, constant at line 48). These keys are internal state and are excluded from backups (`Preference.kt:36-38`). Kurobayomi must keep the same exclusion.

Per tracker (`tracker.id` is 4 for Shikimori, 5 for Bangumi; `TrackerManager.kt:30-31`):

| Key (as built) | Type | Default | Set by | Read by |
|---|---|---|---|---|
| `__PRIVATE_pref_mangasync_username_4` (Shikimori) and `__PRIVATE_pref_mangasync_username_5` (Bangumi) | String | `""` | `setCredentials` (`TrackPreferences.kt:29-33`) | `getUsername()` (`BaseTracker.kt:45`) |
| `__PRIVATE_pref_mangasync_password_4` and `__PRIVATE_pref_mangasync_password_5` | String | `""` | `setCredentials` | `getPassword()` (`BaseTracker.kt:47`); `isLoggedIn` (`BaseTracker.kt:32-34`) |
| `__PRIVATE_pref_tracker_auth_expired_4` and `__PRIVATE_pref_tracker_auth_expired_5` | Boolean | `false` | `setCredentials` (reset to false); never set true by this area | area:27-tracking-core UI |
| `__PRIVATE_track_token_4` and `__PRIVATE_track_token_5` | String (JSON or `"null"` after logout, D10) | `""` | `saveToken` (`Shikimori.kt:234-236`, `Bangumi.kt:204-206`) | `restoreToken` (`Shikimori.kt:238-244`, `Bangumi.kt:208-214`) |

Note: `trackToken(tracker)` is built with `Preference.privateKey("track_token_${tracker.id}")` (`TrackPreferences.kt:35`), so the stored key is `__PRIVATE_track_token_4`. The username and password keys use the same prefix (`TrackPreferences.kt:14-22`).

Global tracking preferences that affect these trackers (owned by area:27-tracking-core, listed for completeness): `track_on_adding_to_library` (default `true`), `pref_auto_update_manga_sync_key` (default `true`), `pref_auto_update_manga_on_mark_read` (enum `AutoTrackState`, default `ALWAYS`), `show_next_episode_airing_time` (default `true`), `anilist_score_type` (irrelevant here). Source: `TrackPreferences.kt:37-51`.

Kurobayomi mapping (proposal, detail in p1-rust-design.md section 5): the username and the auth-expired flag stay in preferences (`kuro-prefs`); the token JSON and the password move to the secret store. The stored password is the access token and is not needed after login.

---

## 5. SQL tables touched

`manga_sync` (`data/src/main/sqldelight/data/manga_sync.sq`), `anime_sync` (`data/src/main/sqldelightanime/dataanime/anime_sync.sq`), created by migrations `data/src/main/sqldelight/migrations/12.sqm` and `14.sqm` and `data/src/main/sqldelightanime/migrations/114.sqm`:

`manga_sync`:

| Column | Type | Null | Default | Notes |
|---|---|---|---|---|
| `_id` | INTEGER PRIMARY KEY | NOT NULL | | |
| `manga_id` | INTEGER | NOT NULL | | FK `mangas(_id)` ON DELETE CASCADE |
| `sync_id` | INTEGER | NOT NULL | | tracker id (4 or 5 here) |
| `remote_id` | INTEGER | NOT NULL | | catalogue id |
| `library_id` | INTEGER | NULL | | user-rate id (Shikimori) |
| `title` | TEXT | NOT NULL | | |
| `last_chapter_read` | REAL | NOT NULL | | |
| `total_chapters` | INTEGER | NOT NULL | | |
| `status` | INTEGER | NOT NULL | | per-tracker status (section 3) |
| `score` | REAL | NOT NULL | | 0..10, or -1 only in unsaved search results |
| `remote_url` | TEXT | NOT NULL | | tracking URL |
| `start_date` | INTEGER | NOT NULL | | epoch millis |
| `finish_date` | INTEGER | NOT NULL | | epoch millis |
| `private` | INTEGER AS Boolean | NOT NULL | 0 | |
| constraint | | | | `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE` |

`anime_sync` is the same with `anime_id`, `last_episode_seen REAL`, `total_episodes INTEGER`, `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`, FK `animes(_id)`.

Writes from this area never touch SQL directly. `updateRemote` calls `insertTrack.await(it)` after a successful remote update (`MangaTracker.kt:114-116`, `AnimeTracker.kt:113-115`). The `ON CONFLICT REPLACE` clause makes that an upsert per (entry, tracker).

---

## 6. Backup encoding of tracks (area:21-backup-format-and-create and 22 own the format)

Track rows are backed up as protobuf messages, not as the token. Field numbers, `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupTracking.kt:8-29`:

| Field | Number | Type | Notes |
|---|---|---|---|
| `syncId` | 1 | Int | tracker id (4 or 5) |
| `libraryId` | 2 | Long | user-rate id |
| `mediaIdInt` | 3 | Int | deprecated, 0 |
| `trackingUrl` | 4 | String | |
| `title` | 5 | String | |
| `lastChapterRead` | 6 | Float | Float on the wire; converted to Double |
| `totalChapters` | 7 | Int | |
| `score` | 8 | Float | |
| `status` | 9 | Int | per tracker |
| `startedReadingDate` | 10 | Long | |
| `finishedReadingDate` | 11 | Long | |
| `private` | 12 | Boolean | |
| `mediaId` | 100 | Long | remote id (catalogue) |

Anime (`BackupAnimeTracking.kt:8-29`): same numbers; field 6 is `lastEpisodeSeen` (Float), 10 and 11 are `startedWatchingDate` and `finishedWatchingDate`.

Tokens and the username are **not** backed up. After a restore, the user must log in again to these trackers (area:22-backup-restore displays the list of missing trackers). Backup `libraryId` for Shikimori is the user-rate id, so restored Shikimori entries can be deleted remotely without a lookup.

---

## 7. Network endpoints

Common:
- Shikimori base `https://shikimori.one`; API prefix `/api`; OAuth paths `/oauth/authorize` and `/oauth/token`.
- Bangumi API `https://api.bgm.tv`; OAuth paths `https://bgm.tv/oauth/authorize` and `https://bgm.tv/oauth/access_token` (different host, `How-to-Auth.md`: "注意用户验证使用的 Base URL 与 API 不同").
- `Authorization: Bearer <access_token>` on every authenticated call (`ShikimoriInterceptor.kt:37-38`, `BangumiInterceptor.kt:40-41`). The bearer is never in the query string; Bangumi v0 forbids it **[verified]** (`How-to-Auth.md`: "新的 API(`/v0/`) 不再允许使用 query string 传递 Access Token").
- User-Agent for authenticated calls: Shikimori `Aniyomi v${VERSION_NAME} (${APPLICATION_ID})` (`ShikimoriInterceptor.kt:39`); Bangumi `jmir1/Aniyomi/v${VERSION_NAME} (Android) (https://github.com/aniyomiorg/aniyomi)` (`BangumiInterceptor.kt:37-38`). Bangumi asks for a descriptive UA with a developer id and app name, and forbids generic ones such as `Bangumi/1.0` **[verified]** (`docs-raw/user agent.md`).
- Default GET cache-control: `maxAge(10, MINUTES)` (`core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:16`), applied to every GET built with `GET(url)` unless overridden.

### 7.1 Shikimori

| # | Method | URL (exact) | Query params (order) | Headers | Body | Success | Errors | Cache | Auth | Code |
|---|---|---|---|---|---|---|---|---|---|---|
| S1 | POST | `https://shikimori.one/oauth/token` | none | form (`FormBody`) | `grant_type=authorization_code&client_id=<client_id>&client_secret=<client_secret>&code=<code>&redirect_uri=aniyomi://shikimori-auth` | 2xx `SMOAuth` JSON | non-2xx → `HttpException` | none | none | `ShikimoriApi.kt:229-248` |
| S2 | POST | `https://shikimori.one/oauth/token` | none | form | `grant_type=refresh_token&client_id=<client_id>&client_secret=<client_secret>&refresh_token=<token>` | 2xx `SMOAuth` JSON | ignored by interceptor | none | none | `ShikimoriApi.kt:267-275` |
| S3 | GET | `https://shikimori.one/api/users/whoami` | none | bearer, UA | none | `SMUser` | exception | default 10 min | bearer | `ShikimoriApi.kt:220-227` |
| S4 | GET | `https://shikimori.one/api/mangas?order=popularity&search=<q>&limit=20` | order, search, limit | bearer, UA | none | JSON array of `SMEntry` | exception | default 10 min | bearer (not needed by server) | `ShikimoriApi.kt:124-138` |
| S5 | GET | `https://shikimori.one/api/animes?order=popularity&search=<q>&limit=20` | same | same | none | array of `SMEntry` | exception | default | same | `ShikimoriApi.kt:140-154` |
| S6 | GET | `https://shikimori.one/api/mangas/{remote_id}` | none | bearer, UA | none | `SMEntry` | 404 → `HttpException(404)` | default | bearer | `ShikimoriApi.kt:158-165` |
| S7 | GET | `https://shikimori.one/api/animes/{remote_id}` | none | bearer, UA | none | `SMEntry` | same | default | bearer | `ShikimoriApi.kt:190-197` |
| S8 | GET | `https://shikimori.one/api/v2/user_rates?user_id=<uid>&target_id=<rid>&target_type=Manga` | user_id, target_id, target_type | bearer, UA | none | array of `SMUserListEntry` | exception | default | bearer | `ShikimoriApi.kt:167-185` |
| S9 | GET | `https://shikimori.one/api/v2/user_rates?user_id=<uid>&target_id=<rid>&target_type=Anime` | same, Anime | same | none | same | same | default | bearer | `ShikimoriApi.kt:199-217` |
| S10 | POST | `https://shikimori.one/api/v2/user_rates` | none | `Content-Type: application/json; charset=utf-8` (`jsonMime`), bearer, UA | the body given in section 7.3 | 2xx `SMAddEntryResponse` | exception | none | bearer | `ShikimoriApi.kt:42-68`, `83-109` |
| S11 | DELETE | `https://shikimori.one/api/v2/user_rates/{libraryId}` | none | bearer, UA | none | 2xx | exception | none | bearer | `ShikimoriApi.kt:75-81`, `116-122` |

Shikimori server facts **[verified]** from shikimori/shikimori `app/controllers/api/v2/user_rates_controller.rb`: index with `user_id`, `target_id`, `target_type` filters (`params[:target_type]` must be `Anime` or `Manga` when `target_id` is given; otherwise `MissingApiParameter`); create is an upsert by (user_id, target_id, target_type); destroy requires `user_rates` scope. Scopes come from `doorkeeper.rb:191`: `default_scopes :user_rates, :comments, :topics`.

### 7.2 Bangumi

| # | Method | URL (exact) | Headers | Body | Success | Errors | Cache | Auth | Code |
|---|---|---|---|---|---|---|---|---|---|
| B1 | POST | `https://bgm.tv/oauth/access_token` | form | `grant_type=authorization_code&client_id=<client_id>&client_secret=<client_secret>&code=<code>&redirect_uri=aniyomi://bangumi-auth` | 2xx `BGMOAuth` | exception | none | none | `BangumiApi.kt:238-254` |
| B2 | POST | `https://bgm.tv/oauth/access_token` | form | `grant_type=refresh_token&client_id=<client_id>&client_secret=<client_secret>&refresh_token=<token>&redirect_uri=aniyomi://bangumi-auth` | 2xx `BGMOAuth` | ignored by interceptor | none | none | `BangumiApi.kt:286-295` |
| B3 | GET | `https://api.bgm.tv/v0/me` | bearer, UA | none | `BGMUser` | exception | default | bearer | `BangumiApi.kt:256-265` |
| B4 | POST | `https://api.bgm.tv/v0/search/subjects?limit=20` | `Content-Type: application/json`, bearer, UA | `{"keyword":"<q>","sort":"match","filter":{"type":[1]}}` (manga) or `[2]` (anime) | 2xx `BGMSearchResult` | exception | none | bearer | `BangumiApi.kt:129-184` |
| B5 | GET | `https://api.bgm.tv/v0/users/{username}/collections/{subject_id}` | bearer, UA, `Cache-Control: no-cache` (FORCE_NETWORK) | none | `BGMCollectionResponse` | 404 → null; others rethrown | FORCE_NETWORK | bearer | `BangumiApi.kt:186-236` |
| B6 | POST | `https://api.bgm.tv/v0/users/-/collections/{subject_id}` | `Content-Type: application/json`, bearer, UA | `{"type","rate","ep_status","private"}` | 202 (no body) | exception | none | bearer | `BangumiApi.kt:43-77` |
| B7 | PATCH | `https://api.bgm.tv/v0/users/-/collections/{subject_id}` | `Content-Type: application/json`, bearer, UA | same body | 204 (no body) | exception | none | bearer | `BangumiApi.kt:79-127` |

Note on B5: `FORCE_NETWORK` is the OkHttp `CacheControl.FORCE_NETWORK` constant. The request still goes through the HTTP cache store, so cached responses are not used.

Bangumi server facts **[verified]** from `bgm_v0.yaml`: B5 `security: OptionalHTTPBearer`, 404 for "用户不存在或者条目未收藏，或者条目为私有收藏" (not collected, or private). Note the 404 also covers *private* collections for other users. B6 is documented as "修改条目收藏状态, 如果不存在则创建，如果存在则修改" (create or modify); the body fields are all optional. B7 requires `write:collection`. The B6 and B7 bodies both accept `type`, `rate` (0..10, `0` removes), `ep_status` (book only), `vol_status`, `comment`, `private`, `tags`.

### 7.3 Request bodies

Shikimori POST (S10), key order as built:
```
{"user_rate":{"user_id":"<getUsername()>","target_id":<remote_id>,"target_type":"Manga","chapters":<int>,"score":<int>,"status":"<api string>"}}
```
For anime: `"target_type":"Anime"`, `"episodes":<int>` instead of `"chapters"`. `user_id` is a JSON string. Integers are JSON numbers. `score` is `score.toInt()`.

Bangumi B6/B7, key order as built:
```
{"type":<1..5>,"rate":<0..10>,"ep_status":<int>,"private":<bool>}
```
`ep_status` is `last_chapter_read.toInt()` (manga) or `last_episode_seen.toInt()` (anime).

Bangumi B4 body:
```
{"keyword":"<query>","sort":"match","filter":{"type":[1]}}
```
Server `sort` enum **[verified]**: `match`, `heat`, `rank`, `score`. Limit query param enforced by server: `limit` integer (no max stated in the operation; the `default_query_limit` used elsewhere is max 50, `bgm_v0.yaml` line 2016).

### 7.4 Result sizes and paging

- Shikimori search: first 20 results only (`limit=20`). No `page` parameter is sent.
- Bangumi search: first 20 (`limit=20`). `offset` is not sent. `total` and `offset` in the response are ignored.
- Shikimori user_rates GET: no `limit` sent. The server default applies (**[verified]** `MAX_LIMIT = 1000` on the server, default not stated in this path, and `limit` is ignored when `user_id` is present).

---

## 8. OAuth parameters (exact)

| Tracker | Authorise URL (`authUrl()`) | Parameter order | Redirect URI | Token URL | Token body keys (order) |
|---|---|---|---|---|---|
| Shikimori | `https://shikimori.one/oauth/authorize` | `client_id`, `redirect_uri`, `response_type=code` | `aniyomi://shikimori-auth` | `https://shikimori.one/oauth/token` | `grant_type`, `client_id`, `client_secret`, `code`, `redirect_uri` |
| Bangumi | `https://bgm.tv/oauth/authorize` | `client_id`, `response_type=code`, `redirect_uri` | `aniyomi://bangumi-auth` | `https://bgm.tv/oauth/access_token` | `grant_type`, `client_id`, `client_secret`, `code`, `redirect_uri` |

Refresh bodies: Shikimori `grant_type`, `client_id`, `client_secret`, `refresh_token`. Bangumi `grant_type`, `client_id`, `client_secret`, `refresh_token`, `redirect_uri`.

The client id and secret values are in `ShikimoriApi.kt:258-259` and `BangumiApi.kt:268-269`. They belong to the Aniyomi project. Decision and proposal: p1-behavior.md section 8.1 (D-OPEN-1). Do not copy them into Kurobayomi source.

Redirect parsing (area:27-tracking-core, `TrackLoginActivity.kt:33-44`, `59-70`): `code` from the query string. Missing `code` means logout.

Bangumi authorisation codes live 60 seconds **[verified]** (`How-to-Auth.md`). Shikimori codes live 1800 seconds **[verified]** (`doorkeeper.rb:65`, `authorization_code_expires_in 30.minutes`).

---

## 9. Logo assets

| Tracker | File | Size (bytes) | Colour | Source |
|---|---|---|---|---|
| Shikimori | `app/src/main/res/drawable-nodpi/ic_tracker_shikimori.webp` | 2948 | `Color.rgb(40, 40, 40)` = `#282828` (`Shikimori.kt:181`) | `Shikimori.kt:179` |
| Bangumi | `app/src/main/res/drawable-nodpi/ic_tracker_bangumi.webp` | 1354 | `Color.rgb(240, 145, 153)` = `#F09199` (`Bangumi.kt:150`) | `Bangumi.kt:148` |

Both are WEBP with alpha (`ALPH` chunk present in the Bangumi file header). Kurobayomi uses its own asset format (p1-rust-design.md section 7).

---

## 10. Storage mapping on Linux (proposal, summary)

| Aniyomi storage | Kurobayomi target | Keys / table |
|---|---|---|
| `__PRIVATE_pref_mangasync_username_N` | secret store (username is not secret, but area:27-tracking-core keeps all three credential values together) | service `kurobayomi.track`, account `N.username` |
| `__PRIVATE_pref_mangasync_password_N` | secret store (`keyring` crate, Secret Service) | service `kurobayomi.track`, account `N.password` (holds the access token, as in the source) |
| `__PRIVATE_track_token_N` (JSON) | secret store | service `kurobayomi.track`, account `N.token` |
| `__PRIVATE_pref_tracker_auth_expired_N` | `kuro-prefs` | `track.N.auth_expired` |
| `manga_sync` / `anime_sync` rows | SQLite via `rusqlite` (area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc) | same columns, `sync_id` = 4 or 5 |
| backup protobuf track messages | area:21-backup-format-and-create | field numbers in section 6 |

Secret store names follow area:27-tracking-core (`plans/27-tracking-core/p1-rust-design.md` section 5.1): service `kurobayomi.track`, accounts `<tracker_id>.username`, `<tracker_id>.password`, `<tracker_id>.token`. The auth-expired flag stays in `kuro-prefs` as `track.<id>.auth_expired`. Final naming is area:27-tracking-core's decision (p1-behavior.md D-OPEN-2).

Tracker ids are part of the persisted format: 4 for Shikimori and 5 for Bangumi must not change in Kurobayomi, because backups and `sync_id` columns use them.
