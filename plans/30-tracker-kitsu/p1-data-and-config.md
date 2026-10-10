# Area 30-tracker-kitsu: data models, preferences, storage, endpoints

Repo root: `/home/blackbox/aniyomi-src`. Paths are repo-relative. Proposals are labelled `Proposal:`.

---

## 1. Kitsu OAuth token (`app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuOAuth.kt`)

`@Serializable data class KitsuOAuth` (kotlinx.serialization, `@SerialName` mapping):

| Kotlin field | JSON key (wire and stored) | Type | Nullable | Default | Notes |
|---|---|---|---|---|---|
| `accessToken` | `access_token` | String | no | none | Sent as `Authorization: Bearer <accessToken>` |
| `tokenType` | `token_type` | String | no | none | Stored, not used for the header (header is hard-coded `Bearer`) |
| `createdAt` | `created_at` | Long | no | none | Unix seconds, from the server |
| `expiresIn` | `expires_in` | Long | no | none | Seconds |
| `refreshToken` | `refresh_token` | String? | yes | `null` | Required by the refresh path (`!!`) |

Predicate: `fun KitsuOAuth.isExpired() = (System.currentTimeMillis() / 1000) > (createdAt + expiresIn - 3600)` (KitsuOAuth.kt:20).

Stored form: `json.encodeToString(oauth)` (default `Json` from the DI module, `ignoreUnknownKeys = true`, `explicitNulls = false`). With `explicitNulls = false` a null `refresh_token` is omitted on write.

Proposal (Rust, `src/track/kitsu/auth.rs`):
```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OAuth {
    #[serde(rename = "access_token")]  pub access_token: String,
    #[serde(rename = "token_type")]    pub token_type: String,
    #[serde(rename = "created_at")]    pub created_at: i64,
    #[serde(rename = "expires_in")]    pub expires_in: i64,
    #[serde(rename = "refresh_token", default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}
impl OAuth {
    pub fn is_expired(&self, now_unix_secs: i64) -> bool {
        now_unix_secs > self.created_at + self.expires_in - 3600
    }
}
```
Decode must ignore unknown keys (the server returns `scope`, and may return others). serde ignores unknown fields by default.

---

## 2. Kitsu DTOs (wire shapes)

All DTOs are `@Serializable` and decoded with the app `Json` (ignore unknown keys). Ids in Kitsu JSON:API are JSON strings; Aniyomi declares them as `Long`/`String` in Kotlin (see section 2.8).

### 2.1 `KitsuAddEntryResult` / `KitsuAddEntryItem` (`dto/KitsuAddEntry.kt`)
- `KitsuAddEntryResult { data: KitsuAddEntryItem }`
- `KitsuAddEntryItem { id: Long }`

### 2.2 `KitsuListSearchResult` (`dto/KitsuListSearch.kt`)
- `data: List<KitsuListSearchItemData>` (required)
- `included: List<KitsuListSearchItemIncluded>` = `emptyList()` (default when absent)

`KitsuListSearchItemData`:
| Field | Type | Nullable | Default |
|---|---|---|---|
| `id` | Long | no | — |
| `attributes` | KitsuListSearchItemDataAttributes | no | — |

`KitsuListSearchItemDataAttributes`:
| Field | Type | Nullable | Notes |
|---|---|---|---|
| `status` | String | no | `current`, `completed`, `on_hold`, `dropped`, `planned` (others throw) |
| `startedAt` | String? | yes | `yyyy-MM-dd'T'HH:mm:ss.SSS'Z'` expected |
| `finishedAt` | String? | yes | same |
| `ratingTwenty` | Int? | yes | 2..20 in practice; `score = ratingTwenty / 2.0` |
| `progress` | Int | no | chapters or episodes |
| `private` | Boolean | no | |

`KitsuListSearchItemIncluded`: `id: Long` (no default), `attributes: KitsuListSearchItemIncludedAttributes`.

`KitsuListSearchItemIncludedAttributes`:
| Field | Type | Nullable | Used by |
|---|---|---|---|
| `canonicalTitle` | String | no | title |
| `chapterCount` | Long? | yes | manga total (`?: 0`) |
| `episodeCount` | Long? | yes | anime total (`?: 0`) |
| `mangaType` | String? | yes | manga `publishing_type` (`?: ""`) |
| `showType` | String? | yes | anime `publishing_type` (`?: ""`) |
| `posterImage` | KitsuSearchItemCover? | yes | cover (`?.original ?: ""`) |
| `synopsis` | String? | yes | summary (`?: ""`) |
| `startDate` | String? | yes | **declared but not read** by the conversion (the conversion reads `userDataAttrs.startedAt`) |
| `status` | String | no | manga/anime `publishing_status` (raw Kitsu string, not localised) |

Note: `KitsuListSearchItemIncludedAttributes` declares `startDate` (String?), `status` (String, non-null). If `status` is absent in an included item, decoding fails.

### 2.3 `KitsuSearchResult` / `KitsuSearchResultData` (`dto/KitsuSearch.kt`)
- `KitsuSearchResult { media: KitsuSearchResultData }`
- `KitsuSearchResultData { key: String }` (the Algolia API key)

### 2.4 `KitsuAlgoliaSearchResult` / `KitsuAlgoliaSearchItem` (`dto/KitsuSearch.kt`)
- `KitsuAlgoliaSearchResult { hits: List<KitsuAlgoliaSearchItem> }` (other Algolia keys ignored)

`KitsuAlgoliaSearchItem`:
| Field | Type | Nullable | Notes |
|---|---|---|---|
| `id` | Long | no | media id |
| `canonicalTitle` | String | no | |
| `chapterCount` | Long? | yes | |
| `episodeCount` | Long? | yes | |
| `subtype` | String? | yes | `novel` filtered out |
| `posterImage` | KitsuSearchItemCover? | yes | |
| `synopsis` | String? | yes | |
| `averageRating` | Double? | yes | |
| `startDate` | Long? | yes | epoch **seconds** |
| `endDate` | Long? | yes | epoch seconds; null means "Publishing" |

### 2.5 `KitsuSearchItemCover` (`dto/KitsuSearchItemCover.kt`)
`{ original: String? }`

### 2.6 `KitsuCurrentUserResult` / `KitsuUser` (`dto/KitsuUser.kt`)
- `KitsuCurrentUserResult { data: List<KitsuUser> }`
- `KitsuUser { id: String }` (JSON:API string id)

### 2.7 Request bodies (built with `buildJsonObject`, KitsuApi.kt)
See section 4 for every field and type.

### 2.8 Id types on the wire
- Response ids are Kitsu JSON:API strings (e.g. `"id": "12345"`). Aniyomi declares `id: Long` for entries and media, `id: String` for users.
- Request bodies send the library-entry id and media id as **JSON numbers** (`put("id", track.remote_id)` where `remote_id: Long`), and the user id as a **JSON string** (`put("id", userId)` where `userId: String`).
- Observed: decoding a quoted id into `Long` works in production; the Kotlin source does not configure this (`Json` has no `isLenient`/`allowStringsForNonStringTypes`, AppModule.kt:174-177). Mechanism not verified here.
- Proposal (Rust): decode ids with a custom deserializer that accepts a JSON number or a numeric string. Request bodies: keep the Aniyomi shapes (numeric media/entry ids, string user id) for byte-compatibility with what is known to work; verify against the live API in the test plan (T-30a-033).

---

## 3. Local track models (Aniyomi data and domain layers, used by Kitsu)

These are defined outside scope. Kitsu reads and writes these fields.

### 3.1 `MangaTrack` (data, `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt` + `MangaTrackImpl.kt`)
| Field | Type | Default (Impl) |
|---|---|---|
| `id` | Long? | null |
| `manga_id` | Long | 0 |
| `tracker_id` | Long | 0 |
| `remote_id` | Long | 0 |
| `library_id` | Long? | null |
| `title` | String | lateinit (must be set) |
| `last_chapter_read` | Double | 0.0 |
| `total_chapters` | Long | 0 |
| `score` | Double | 0.0 |
| `status` | Long | 0 |
| `started_reading_date` | Long | 0 (epoch ms) |
| `finished_reading_date` | Long | 0 (epoch ms) |
| `tracking_url` | String | "" |
| `private` | Boolean | false |

`copyPersonalFrom(other, copyRemotePrivate = true)` copies, in order: `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date`, and `private` only if `copyRemotePrivate`.

### 3.2 `AnimeTrack` (data, `.../models/anime/AnimeTrack.kt` + `AnimeTrackImpl.kt`)
Same shape with `anime_id`, `last_episode_seen: Double` (0.0), `total_episodes: Long` (0), `score: Double` (0.0), `started_watching_date: Long` (0), `finished_watching_date: Long` (0), `tracking_url` (""), `private` (false). `copyPersonalFrom` copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, and `private` if requested.

### 3.3 Domain `MangaTrack` (`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt`)
`data class MangaTrack(id: Long, mangaId: Long, trackerId: Long, remoteId: Long, libraryId: Long?, title: String, lastChapterRead: Double, totalChapters: Long, status: Long, score: Double, remoteUrl: String, startDate: Long, finishDate: Long, private: Boolean)`

Domain `AnimeTrack` (`domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt`): `id, animeId, trackerId, remoteId, libraryId, title, lastEpisodeSeen: Double, totalEpisodes: Long, status, score, remoteUrl, startDate, finishDate, private`.

Mapping (`app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt`, and the manga twin): `toDbTrack()` maps `startDate ↔ started_watching_date` and `finishDate ↔ finished_watching_date`, `remoteUrl ↔ tracking_url`, `remoteId ↔ remote_id`. `toDomainTrack(idRequired)` returns `null` when `id == null` and `idRequired` is true; when `idRequired` is false the id is `-1`.

### 3.4 `MangaTrackSearch` / `AnimeTrackSearch` (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/`)
Extend the track interface. Extra fields (not in the DB):
| Field | Type | Default |
|---|---|---|
| `authors` | List<String> | emptyList() |
| `artists` | List<String> | emptyList() |
| `cover_url` | String | "" |
| `summary` | String | "" |
| `publishing_status` | String | "" |
| `publishing_type` | String | "" |
| `start_date` | String | "" |

Base defaults on these classes: `score = -1.0` (search objects only), `id = null`, `library_id = null`. `equals`/`hashCode` on `(manga_id|anime_id, tracker_id, remote_id)`. `create(serviceId)` sets `tracker_id`.

---

## 4. Network endpoints and exact request/response shapes

Base URL constants (KitsuApi.kt companion object, lines 427-452):

| Constant | Exact value |
|---|---|
| `CLIENT_ID` | `dd031b32d2f56c990b1425efe6c42ad847e7fe3ab46bf1299f05ecd856bdb7dd` |
| `CLIENT_SECRET` | `54d7307928f63414defd96399fc31ba847961ceaecef3a5fd93144e960c0e151` |
| `BASE_URL` | `https://kitsu.app/api/edge/` |
| `LOGIN_URL` | `https://kitsu.app/api/oauth/token` |
| `BASE_MANGA_URL` | `https://kitsu.app/manga/` |
| `BASE_ANIME_URL` | `https://kitsu.app/anime/` |
| `ALGOLIA_KEY_URL` | `https://kitsu.app/api/edge/algolia-keys/media/` |
| `ALGOLIA_URL` | `https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/` |
| `ALGOLIA_APP_ID` | `AWQO5J657S` |
| `ALGOLIA_FILTER` | see section 4.9 |
| `ALGOLIA_FILTER_ANIME` | see section 4.9 |
| `VND_API_JSON` | `application/vnd.api+json` |
| `VND_JSON_MEDIA_TYPE` | `application/vnd.api+json` (as `MediaType`) |

Note: `CLIENT_ID` and `CLIENT_SECRET` are the public credentials compiled into the Aniyomi client. They are reproduced here because the port must send them. They are not user secrets.

Common headers for `authClient` requests (set by `KitsuInterceptor`, I:38-43):
- `Authorization: Bearer <access_token>`
- `User-Agent: Aniyomi v<BuildConfig.VERSION_NAME> (<BuildConfig.APPLICATION_ID>)`
- `Accept: application/vnd.api+json`
- `Content-Type: application/vnd.api+json`

Proposal: `User-Agent: Kurobayomi v<CARGO_PKG_VERSION> (io.github.anas1412.Kurobayomi)`. Keep the other three headers.

### 4.1 Login (password grant)
- Method and URL: `POST https://kitsu.app/api/oauth/token`
- Auth: none. Uses base client (no bearer).
- Body: `application/x-www-form-urlencoded` (OkHttp `FormBody`), fields in this order: `username=<email>`, `password=<password>`, `grant_type=password`, `client_id=<CLIENT_ID>`, `client_secret=<CLIENT_SECRET>`.
- Success: 2xx with `KitsuOAuth` JSON (section 1).
- Failure: non-2xx → `HttpException(code)` message `HTTP error <code>`.

### 4.2 Refresh
- Method and URL: `POST https://kitsu.app/api/oauth/token`
- Body (form), fields in this order: `grant_type=refresh_token`, `refresh_token=<token>`, `client_id=<CLIENT_ID>`, `client_secret=<CLIENT_SECRET>`.
- Sent through `chain.proceed` inside the interceptor, so no bearer header and no Aniyomi User-Agent (section 4 of behaviour spec).
- Success: `KitsuOAuth` replaces the stored token. Failure: response closed, old token kept.

### 4.3 Current user
- `GET https://kitsu.app/api/edge/users?filter[self]=true` (`authClient`).
- Response: `{"data":[{"id":"<string>"}]}` (other keys ignored). Uses `data[0].id`.

### 4.4 Add library entry
- `POST https://kitsu.app/api/edge/library-entries`
- Headers: `Content-Type: application/vnd.api+json` (+ interceptor headers)
- Body (manga; anime identical with `"media"` type `"anime"`, `progress` from `last_episode_seen`):
```json
{"data":{"type":"libraryEntries","attributes":{"status":"<api status>","progress":<int>,"private":<bool>},"relationships":{"user":{"data":{"id":"<userId string>","type":"users"}},"media":{"data":{"id":<remote_id number>,"type":"manga"}}}}}
```
- Response: `{"data":{"id":<number>}}` (decoded as `KitsuAddEntryResult`; other keys ignored).
- Result: `track.remote_id = data.id`.

### 4.5 Update library entry
- `PATCH https://kitsu.app/api/edge/library-entries/<remote_id>` (`remote_id` = library-entry id)
- Headers: `Content-Type: application/vnd.api+json`
- Body (manga; anime uses `last_episode_seen`, `started_watching_date`, `finished_watching_date`):
```json
{"data":{"type":"libraryEntries","id":<remote_id number>,"attributes":{"status":"<api status>","progress":<int>,"ratingTwenty":"<string>" or null,"startedAt":"<iso>" or null,"finishedAt":"<iso>" or null,"private":<bool>}}}
```
- Response: not read. Success is 2xx.
- Null handling: `ratingTwenty` is null when score is 0 or negative; `startedAt`/`finishedAt` are null when the epoch is 0.

### 4.6 Remove library entry
- `DELETE https://kitsu.app/api/edge/library-entries/<remoteId>`
- Headers: `Content-Type: application/vnd.api+json`
- Body: empty form (`FormBody` with no fields). The interceptor's `Content-Type` overrides it.
- Response: not read.

### 4.7 Find library entry by media id
- `GET https://kitsu.app/api/edge/library-entries?filter[manga_id]=<media id>&filter[user_id]=<userId>&include=manga`
- Anime: `filter[anime_id]=<media id>`, `include=anime`.
- Response: `KitsuListSearchResult` (section 2.2). `null` when `data` or `included` is empty.

### 4.8 Get library entry by entry id
- `GET https://kitsu.app/api/edge/library-entries?filter[id]=<library-entry id>&include=manga` (anime: `include=anime`).
- Response: `KitsuListSearchResult`. Empty → exception.

### 4.9 Algolia search
Step 1: `GET https://kitsu.app/api/edge/algolia-keys/media/` (authClient). Response: `{"media":{"key":"<algolia search key>"}}`.

Step 2: `POST https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/`
- Headers: `X-Algolia-Application-Id: AWQO5J657S`, `X-Algolia-API-Key: <key>`. Content-Type `application/json; charset=utf-8` (`jsonMime`). No bearer.
- Body: `{"params":"query=<URLEncoder(query, UTF-8)><ALGOLIA_FILTER>"}`.

Exact constants (copy verbatim; `%` sequences are part of the value):
- `ALGOLIA_FILTER` = `&facetFilters=%5B%22kind%3Amanga%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22chapterCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D`
- `ALGOLIA_FILTER_ANIME` = `&facetFilters=%5B%22kind%3Aanime%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22episodeCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D`

(In the Kotlin source these are split across two string literals joined by `+`. Joined result is as above.)

Response: `{"hits":[<hit objects>]}`; each hit object carries the fields of `KitsuAlgoliaSearchItem` (section 2.4). Other keys are ignored.

Filter: `hits.filter { it.subtype != "novel" }`.

### 4.10 Errors from every call
Non-2xx → `HttpException(code)`; transport → `IOException(message)`; decode → `kotlinx.serialization` exception (`SerializationException`); logic → the literal `Exception` messages listed in section 6.2.

---

## 5. Preferences (`TrackPreferences`, `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`)

Kitsu-owned keys (Tracker id = 3; `Preference.privateKey(...)` = private key namespace):

| Key | Type | Default | Read/written by |
|---|---|---|---|
| `pref_mangasync_username_3` | String | `""` | `trackUsername(kitsu)`: login stores email; logout clears |
| `pref_mangasync_password_3` | String | `""` | `trackPassword(kitsu)`: login stores Kitsu **user id** (not a password); logout clears |
| `pref_tracker_auth_expired_3` | Boolean | `false` | `setCredentials` resets to `false`; never set true by Kitsu |
| `track_token_3` | String | `""` | `trackToken(kitsu)`: JSON of `KitsuOAuth`, or the string `null` after logout, or `""` when never set |

Keys are generated as `pref_mangasync_username_${tracker.id}`, `pref_mangasync_password_${tracker.id}`, `pref_tracker_auth_expired_${tracker.id}`, `track_token_${tracker.id}`. Note the first two use `Preference.privateKey(...)`, the token key too.

Keys read by the track flows that Kitsu participates in (owned by area:27-tracking-core / area:02-preferences-system; defaults listed for completeness):

| Key | Type | Default |
|---|---|---|
| `pref_auto_update_manga_sync_key` | Boolean | `true` (`autoUpdateTrack()`) |
| `track_on_adding_to_library` | Boolean | `true` (`trackOnAddingToLibrary()`) |
| `show_next_episode_airing_time` | Boolean | `true` (`showNextEpisodeAiringTime()`) |
| `pref_auto_update_manga_on_mark_read` | enum `AutoTrackState` | `AutoTrackState.ALWAYS` (`autoUpdateTrackOnMarkRead()`) |

`AutoTrackState` (`app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt`): `ALWAYS`, `ASK`, `NEVER`.

Preference UI rows for the Kitsu login (SettingsTrackingScreen.kt:182-185): `TrackerPreference(tracker = trackerManager.kitsu, login = { LoginDialog(kitsu, MR.strings.email) }, logout = { LogoutDialog(kitsu) })`.

Proposal (Rust): stored in the app settings table of the Kurobayomi SQLite database (area:02-preferences-system) under these keys, with the user id in a separate key `track_user_id_3` (instead of the password key). Token storage: see section 7.

---

## 6. Message keys and literal messages

### 6.1 Localised (MR / AYMR strings), value from `base/strings.xml`
| Key | Module | English value | Used by |
|---|---|---|---|
| `currently_reading` | AYMR | Currently reading | `getStatusForManga(1)` |
| `want_to_read` | AYMR | Want to read | `getStatusForManga(5)` |
| `currently_watching` | AYMR | Currently watching | `getStatusForAnime(11)` |
| `want_to_watch` | AYMR | Want to watch | `getStatusForAnime(15)` |
| `completed` | MR | Completed | status 2 (both) |
| `on_hold` | MR | On hold | status 3 (both) |
| `dropped` | MR | Dropped | status 4 (both) |
| `email` | MR | Email address | login username label (Kitsu) |
| `password` | MR | Password | login password label |
| `username` | MR | Username | login label for other trackers (not Kitsu) |
| `login_title` | MR | Log in to %1$s | login dialog title |
| `login` | MR | Login | login confirm button |
| `loading` | MR | Loading… | login button while processing |
| `login_success` | MR | Logged in | toast on success |
| `logout_title` | MR | Log out from %1$s? | logout dialog title |
| `logout` | MR | Log out | logout confirm button |
| `logout_success` | MR | You are now logged out | toast on logout |
| `action_cancel` | MR | Cancel | dialog buttons |
| `action_close` | MR | Close | login dialog close icon description |
| `action_ok` | MR | OK | delete dialog button |
| `track_delete_title` | MR | Remove %s tracking? | delete dialog title |
| `track_delete_text` | MR | This will remove the tracking locally. | delete dialog body |
| `track_delete_remote_text` | MR | Also remove from %s | delete dialog checkbox |
| `pref_auto_update_manga_sync` | AYMR | Update progress after reading/watching | settings |
| `pref_track_on_add_library` | AYMR | Open track menu on adding to library | settings |

Source of truth: `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR). `currently_reading`, `want_to_read`, `currently_watching`, `want_to_watch` are AYMR, the others MR (the Kitsu source imports both `tachiyomi.i18n.MR` and `tachiyomi.i18n.aniyomi.AYMR`).

### 6.2 Literal (non-localised) strings produced by Kitsu code
| Text | Where | Trigger |
|---|---|---|
| `Not authenticated with Kitsu` | KitsuInterceptor.kt:23 | any authed request with no token |
| `HTTP error <code>` | OkHttpExtensions/HttpException.kt:13 | any non-2xx |
| `Could not find manga` | KitsuApi.kt:367 | get-by-entry-id, no data (manga) |
| `Could not find manga` | KitsuApi.kt:388 | get-by-entry-id, no data (anime; copy-paste) |
| `Missing User data from Kitsu` | KitsuListSearch.kt:17, 51 | `firstTo*` with empty `data` |
| `Missing Manga data from Kitsu` | KitsuListSearch.kt:18, 52 | `firstTo*` with empty `included` (anime path also says Manga) |
| `Unknown status` | KitsuUtils.kt:12, 21; KitsuListSearch.kt:42, 76 | status not in mapping table |
| `Publishing` / `Finished` | KitsuSearch.kt:49, 67 | search `publishing_status` (not shown as UI text by Kitsu code) |
| `null` (the word) | stored token after logout | pref value |

Proposal: keep the English texts for these (they reach the UI through `e.message`), but give each a stable key in area:27-tracking-core so they can be localised. Fix the anime "Could not find manga" text (T-30a-024).

---

## 7. Token and credential storage

On disk (Aniyomi): the preferences file of the app (SharedPreferences backend of `PreferenceStore`). Kitsu holds no separate file.

Proposal (Kurobayomi):
- Primary: Secret Service via the `keyring` crate (`service = "kurobayomi.track.kitsu"`, `user = "oauth"`, value = the `OAuth` JSON). On KDE Plasma, KWallet provides the Secret Service interface.
- Fallback when no Secret Service is reachable (`keyring::Error::NoStorageAccess` / `PlatformFailure`): store the same JSON in the settings table under `track_token_3`, file mode 0600 for the database (area:02-preferences-system). The fallback matches the Aniyomi key name so that a backup import (T-30a-032) can read it.
- User id: settings key `track_user_id_3` (plain text, not secret).
- Username (email): settings key `pref_mangasync_username_3`.
- Password: never stored.

Remember-me: none. Logout removes all four (keyring entry, `track_token_3`, `track_user_id_3`, username, auth-expired flag).

---

## 8. Database tables (touched by the Kitsu flows through the caller)

Kitsu itself does not run SQL. Tracks are written by `InsertMangaTrack` / `InsertAnimeTrack` (outside scope). The tables:

### 8.1 `manga_sync` (`data/src/main/sqldelight/data/manga_sync.sq`)
```
_id INTEGER NOT NULL PRIMARY KEY,
manga_id INTEGER NOT NULL,
sync_id INTEGER NOT NULL,         -- tracker id; Kitsu = 3
remote_id INTEGER NOT NULL,       -- media id before add, library-entry id after add/find
library_id INTEGER,               -- unused by Kitsu (always null)
title TEXT NOT NULL,
last_chapter_read REAL NOT NULL,
total_chapters INTEGER NOT NULL,
status INTEGER NOT NULL,
score REAL NOT NULL,              -- 0.0..10.0 (0.5 steps)
remote_url TEXT NOT NULL,         -- tracking URL
start_date INTEGER NOT NULL,      -- epoch ms
finish_date INTEGER NOT NULL,     -- epoch ms
private INTEGER AS Boolean DEFAULT 0 NOT NULL,
UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
```
Queries used for Kitsu rows: `insert`, `update` (COALESCE per column), `delete` (by `manga_id`, `sync_id`), `getTracksByMangaId`.

### 8.2 `anime_sync` (`data/src/main/sqldelightanime/dataanime/anime_sync.sq`)
Same shape: `anime_id`, `last_episode_seen REAL NOT NULL`, `total_episodes INTEGER NOT NULL`, `start_date`, `finish_date`, `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`, `FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE`.

Column-to-field map (both tables): `remote_id ↔ remote_id`, `remote_url ↔ tracking_url`, `start_date ↔ started_*_date / started_watching_date`, `finish_date ↔ finished_*_date`, `sync_id ↔ tracker_id`.

Kurobayomi (Proposal): the same two tables in the Rust schema (area:03/04), `CHECK (score >= 0 AND score <= 10)`, and `private` stored as `INTEGER 0/1`.

---

## 9. Enumerations and stored numeric values

### 9.1 Kitsu status (stored in `status` column)
| Value | Name | Kind | API string |
|---|---|---|---|
| 1 | READING | manga | `current` |
| 11 | WATCHING | anime | `current` |
| 2 | COMPLETED | both | `completed` |
| 3 | ON_HOLD | both | `on_hold` |
| 4 | DROPPED | both | `dropped` |
| 5 | PLAN_TO_READ | manga | `planned` |
| 15 | PLAN_TO_WATCH | anime | `planned` |
| -1 | (rereading / rewatching) | — | not supported |

### 9.2 Score (stored in `score` column, Double)
Valid stored values: `0.0` and the values `1.0` to `10.0` in 0.5 steps (the 20 entries of the score list in behaviour section 3.1, with index 0 = `0.0`). Wire `ratingTwenty` = `score * 2` (truncated), as a string. Score index `0..19` ↔ values as in behaviour section 3.1.

### 9.3 Progress and totals
Stored as Double (`last_chapter_read`, `last_episode_seen`) and Long (`total_*`). Wire progress is `Int`. Total `0` means unknown.

### 9.4 Dates
Stored as Long epoch milliseconds (`start_date`, `finish_date`). Wire: string (section 10). `0` means unset.

### 9.5 Tracker id
`KITSU = 3L` (TrackerManager.kt companion object). Tracker ids of the other services are outside scope.

### 9.6 Private flag
Boolean. Stored in `private` (0/1). Wire `private` boolean.

---

## 10. Date wire format

Pattern: `yyyy-MM-dd'T'HH:mm:ss.SSS'Z'` (KitsuDateHelper.kt:9). Locale ENGLISH. Time zone: JVM default (defect; see behaviour section 14).

Example wire value: `2024-05-01T12:34:56.789Z`.

Proposal: `%Y-%m-%dT%H:%M:%S%.3fZ` in UTC (chrono), epoch 0 → JSON null.

Search start date format (display only): `yyyy-MM-dd` with `Locale.US`, default zone.

---

## 11. Files, folders, formats on disk

Kitsu code itself writes no files. Network cache: `context.cacheDir/network_cache`, 5 MiB, OkHttp format (shared with the rest of the app; area:12-network-stack). Kurobayomi (Proposal): no HTTP disk cache for Kitsu (`reqwest` has no cache), tokens in the keyring or DB (section 7).

---

## 12. Rust data model (Proposal, for `src/track/kitsu/mod.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Manga, Anime }

/// One library entry, both kinds. Field names follow the DB columns.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackRecord {
    pub id: Option<i64>,          // local row id
    pub item_id: i64,             // manga_id or anime_id
    pub kind: Kind,
    pub remote_id: i64,           // media id before add/find, library-entry id after
    pub library_id: Option<i64>,  // always None for Kitsu
    pub title: String,
    pub progress: f64,            // last_chapter_read / last_episode_seen
    pub total: i64,               // total_chapters / total_episodes (0 = unknown)
    pub score: f64,               // 0.0 or 0.5..=10.0
    pub status: i64,              // section 9.1 values
    pub started_ms: i64,          // 0 = unset
    pub finished_ms: i64,         // 0 = unset
    pub tracking_url: String,
    pub private: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub kind: Kind,
    pub remote_id: i64,
    pub title: String,
    pub total: i64,
    pub cover_url: String,
    pub summary: String,
    pub tracking_url: String,
    pub score: f64,               // raw averageRating or -1.0
    pub publishing_status: String,
    pub publishing_type: String,
    pub start_date: String,       // "yyyy-MM-dd" or ""
}
```

Deviation from Aniyomi: one `TrackRecord` for both kinds (the Kotlin keeps two near-identical classes). This is a Rust-side simplification and does not change behaviour; the per-kind field mapping (`progress`, `total`, date fields) is the same as section 3.

---

## 13. Config summary for the Rust port

| Setting | Value | Owner |
|---|---|---|
| Kitsu tracker id | 3 | area:27-tracking-core |
| Base URL | `https://kitsu.app/api/edge/` | this area |
| Login URL | `https://kitsu.app/api/oauth/token` | this area |
| Algolia URL / app id | see 4.9 | this area |
| Connect timeout | 30 s | Proposal: reqwest `connect_timeout(Duration::from_secs(30))` |
| Read timeout | 30 s | Proposal: reqwest `read_timeout` (feature needed) |
| Total timeout | 120 s | Proposal: reqwest `timeout(Duration::from_secs(120))` |
| Token refresh margin | 3600 s | this area |
| Retries | none | this area |
