# Area 29-tracker-myanimelist, part 1: behaviour spec

Paths are relative to `/home/blackbox/aniyomi-src`. `OBSERVED` marks what the Kotlin does. `Proposal:` marks what Rust should do. Line numbers refer to the files named in the same paragraph.

## 0. Identity and constants

OBSERVED
- Tracker id: `TrackerManager.kt:27` builds `val myAnimeList = MyAnimeList(1L)`. The id is `1`. It is stored in `manga_sync.sync_id` and `anime_sync.sync_id` and in the preference keys (see `p1-data-and-config.md`).
- Name: `"MyAnimeList"` (`MyAnimeList.kt:28`).
- Logo: `R.drawable.ic_tracker_mal` (`MyAnimeList.kt:61`), file `app/src/main/res/drawable-nodpi/ic_tracker_mal.webp`.
- Logo colour: `Color.rgb(46, 81, 162)` (`MyAnimeList.kt:63`), which is `#2E51A2`.
- Capabilities: `supportsReadingDates = true` (`MyAnimeList.kt:59`). `supportsPrivateTracking` is inherited as `false` from `BaseTracker.kt:25`. MAL has no private-tracking flag in this code.
- Implements `MangaTracker`, `AnimeTracker`, `DeletableMangaTracker`, `DeletableAnimeTracker` (`MyAnimeList.kt:25-33`).
- Client id: `CLIENT_ID = "686b980ff4240fccce7f6a654cea07ce"` (`MyAnimeListApi.kt:383`). This is the Aniyomi project's registration. See section 4.1 and `p1-rust-design.md` section 5.
- Hosts: `BASE_OAUTH_URL = "https://myanimelist.net/v1/oauth2"` (`MyAnimeListApi.kt:385`), `BASE_API_URL = "https://api.myanimelist.net/v2"` (`MyAnimeListApi.kt:386`).
- Page size for list paging: `LIST_PAGINATION_AMOUNT = 250` (`MyAnimeListApi.kt:388`).
- Search prefixes: `SEARCH_ID_PREFIX = "id:"` and `SEARCH_LIST_PREFIX = "my:"` (`MyAnimeList.kt:46-47`).

## 1. Status model

### 1.1 Local status codes (Long)

`MyAnimeList.kt:36-44`:

| Constant | Value | Used by |
|---|---|---|
| `READING` | 1 | manga "Reading" |
| `WATCHING` | 11 | anime "Watching" |
| `COMPLETED` | 2 | both |
| `ON_HOLD` | 3 | both |
| `DROPPED` | 4 | both |
| `PLAN_TO_READ` | 6 | manga |
| `PLAN_TO_WATCH` | 16 | anime |
| `REREADING` | 7 | manga |
| `REWATCHING` | 17 | anime |

The values mirror the upstream Tachiyomi codes. They are stored in `manga_sync.status` and `anime_sync.status` as INTEGER.

Note: the code `1` (`READING`) is also used as the fall-back for unknown wire status strings in both manga and anime (see 1.3). An anime row with status `1` has no label (see 1.4).

### 1.2 Status lists (what the UI offers)

`MyAnimeList.kt:65-71`:
- `getStatusListManga()` returns `[READING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_READ, REREADING]`, in that order.
- `getStatusListAnime()` returns `[WATCHING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_WATCH, REWATCHING]`, in that order.

`getCompletionStatus()` returns `COMPLETED` (`MyAnimeList.kt:101`).
`getReadingStatus()` returns `READING`. `getRereadingStatus()` returns `REREADING`. `getWatchingStatus()` returns `WATCHING`. `getRewatchingStatus()` returns `REWATCHING` (`MyAnimeList.kt:93-99`).

### 1.3 Wire mapping (local to MAL string)

`MyAnimeListUtils.kt:6-14` (`MangaTrack.toMyAnimeListStatus()`):

| Local | MAL string sent in `status` form field |
|---|---|
| READING (1) | `reading` |
| COMPLETED (2) | `completed` |
| ON_HOLD (3) | `on_hold` |
| DROPPED (4) | `dropped` |
| PLAN_TO_READ (6) | `plan_to_read` |
| REREADING (7) | `reading` (and `is_rereading=true` is sent separately, see 5.5) |
| anything else | `null`, then the caller uses the default `"reading"` (`MyAnimeListApi.kt:184`) |

`MyAnimeListUtils.kt:16-27` (`AnimeTrack.toMyAnimeListStatus()`):

| Local | MAL string |
|---|---|
| WATCHING (11) | `watching` |
| READING (1) | `watching` |
| COMPLETED (2) | `completed` |
| ON_HOLD (3) | `on_hold` |
| DROPPED (4) | `dropped` |
| PLAN_TO_READ (6) | `plan_to_watch` |
| REREADING (7) | `watching` |
| PLAN_TO_WATCH (16) | `plan_to_watch` |
| REWATCHING (17) | `watching` |
| anything else | `null`, then `"watching"` default (`MyAnimeListApi.kt:211`) |

Observed: anime code `READING` (1) is sent as `watching`. Anime code `PLAN_TO_READ` (6) is sent as `plan_to_watch`. These cross-mappings only matter when a row has a manga code in an anime row.

### 1.4 Wire to local (`getStatus`)

`MyAnimeListUtils.kt:29-38`:

| MAL string | Local |
|---|---|
| `reading` | READING (1) |
| `watching` | WATCHING (11) |
| `completed` | COMPLETED (2) |
| `on_hold` | ON_HOLD (3) |
| `dropped` | DROPPED (4) |
| `plan_to_read` | PLAN_TO_READ (6) |
| `plan_to_watch` | PLAN_TO_WATCH (16) |
| anything else, including `null` | READING (1) |

The `is_rereading` / `is_rewatching` flag overrides this on read (see 5.6): if the flag is true, the local code becomes REREADING (7) or REWATCHING (17).

### 1.5 User-visible labels

`MyAnimeList.kt:73-91`:

Manga (`getStatusForManga`):
- READING -> `MR.strings.reading` ("Reading")
- COMPLETED -> `MR.strings.completed` ("Completed")
- ON_HOLD -> `MR.strings.on_hold` ("On hold")
- DROPPED -> `MR.strings.dropped` ("Dropped")
- PLAN_TO_READ -> `MR.strings.plan_to_read` ("Plan to read")
- REREADING -> `MR.strings.repeating` ("Rereading")
- else -> null (no label)

Anime (`getStatusForAnime`):
- WATCHING -> `AYMR.strings.watching` ("Watching")
- COMPLETED -> `MR.strings.completed` ("Completed")
- ON_HOLD -> `MR.strings.on_hold` ("On hold")
- DROPPED -> `MR.strings.dropped` ("Dropped")
- PLAN_TO_WATCH -> `AYMR.strings.plan_to_watch` ("Plan to watch")
- REWATCHING -> `AYMR.strings.repeating_anime` ("Rewatching")
- else -> null. READING (1) has no anime label.

The string resource values were read from `i18n/src/commonMain/moko-resources/base/strings.xml` (lines 775-781) and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (lines 518-525).

## 2. Score model

`MyAnimeList.kt:49-51`: `SCORE_LIST = IntRange(0, 10).map(Int::toString)`, which is `["0","1","2","3","4","5","6","7","8","9","10"]`, 11 entries.

- `getScoreList()` returns that list (`MyAnimeList.kt:103`).
- `indexToScore(index) = index.toDouble()` (`MyAnimeList.kt:105-107`). The score equals the index.
- `displayScore(track) = track.score.toInt().toString()` for manga and anime (`MyAnimeList.kt:109-115`). A score of 8.0 is shown as `8`. A score of 0.0 is shown as `0`. A score of -1.0 is shown as `-1`.
- `get10PointScore` is not overridden, so the interface default returns `track.score` (`MangaTracker.kt:43-45`). MAL is already 10-point.
- Score sent to MAL is `track.score.toString()` (Kotlin `Double.toString`). A score of 8 is sent as `"8.0"`. A score of 0 is sent as `"0.0"`. See section 13, item D-11.
- Score read from MAL is `listStatus.score.toDouble()` (`MyAnimeListApi.kt:349` and `:360`).
- `setRemoteScore(track, scoreString)` sets `track.score = indexToScore(getScoreList().indexOf(scoreString))` (`MangaTracker.kt:91-94`). If the string is not in the list, `indexOf` returns -1 and the score becomes -1.0. See section 13, item D-12.

## 3. Date model

Local tracker fields are epoch milliseconds (`Long`). The MAL wire format is `yyyy-MM-dd`.

`MyAnimeListApi.kt:366-368` (`parseDate`):
- `SimpleDateFormat("yyyy-MM-dd", Locale.US).parse(isoDate)?.time ?: 0L`.
- The date is interpreted in the JVM default time zone, so the result is local midnight.
- `SimpleDateFormat` is lenient by default. `2020-13-45` is normalised, not rejected.
- A partial date such as `2020-05` throws `java.text.ParseException`. The exception is not caught in `parseDate`. It propagates out of `findListItem`, `updateItem` or `getListPage`. See section 13, item D-08.

`MyAnimeListApi.kt:370-380` (`convertToIsoDate(epochTime: Long): String?`):
1. If `epochTime == 0L`, return `""` (an empty string, not null). The caller then sends `start_date=` or `finish_date=`, which clears the date on MAL.
2. Otherwise format with `SimpleDateFormat("yyyy-MM-dd", Locale.US)` in the default time zone.
3. If formatting throws, return `null`. The caller then omits the form field.

Callers (`MyAnimeListApi.kt:188-193`, `:215-220`) add `start_date` and `finish_date` to the form only when the result is non-null. An empty string is non-null, so both fields are sent on every update. Unset local dates clear the remote dates.

The time of completion is stored as `System.currentTimeMillis()` in ms (for example `MyAnimeList.kt:130`, `:134`, `:148`, `:152`). It is not normalised to midnight. The MAL wire date is then the local calendar date of that instant. Round trip: local date -> epoch (midnight) -> same local date. Completion timestamps round-trip to the same date but not to the same epoch.

## 4. Authentication

### 4.1 Authorize URL (`MyAnimeListApi.kt:392-396`)

`authUrl()` returns:

```
https://myanimelist.net/v1/oauth2/authorize?client_id=686b980ff4240fccce7f6a654cea07ce&code_challenge=<verifier>&response_type=code
```

Built with `Uri.Builder` in the order `client_id`, `code_challenge`, `response_type`.

- No `redirect_uri` parameter is sent. MAL uses the redirect URI registered for the client.
- No `code_challenge_method` is sent. MAL then treats the challenge as `plain`, so `code_challenge == code_verifier`.
- Every call to `authUrl()` generates a new verifier and overwrites the companion variable `codeVerifier` (`MyAnimeListApi.kt:390`, `:425-428`).
- The Settings login row calls it and opens the URL in the default browser with `forceDefaultBrowser = true` (`SettingsTrackingScreen.kt:164-167`).

### 4.2 PKCE verifier (`PkceUtil.kt:8-14`)

1. Create 50 random bytes with `SecureRandom().nextBytes`.
2. Base64 URL-encode without padding (`Base64.getUrlEncoder().withoutPadding()`).
3. The result is 68 characters from `[A-Za-z0-9_-]`. That is within the RFC 7636 length range of 43 to 128.

### 4.3 Login completion (`MyAnimeList.kt:251-260`, `login(authCode)`)

Sequence, inside a coroutine on IO (`TrackLoginActivity.kt:46-57`):
1. `api.getAccessToken(authCode)` (see 5.1). On success returns `MALOAuth`.
2. `interceptor.setAuth(oauth)`. This sets the in-memory `oauth` field and calls `saveOAuth(oauth)` (`MyAnimeListInterceptor.kt:45-48`).
3. `api.getCurrentUser()` (see 5.2) returns the username.
4. `saveCredentials(username, oauth.accessToken)` (`BaseTracker.kt:49-51`). This sets the username pref, sets the password pref to the access token, and sets the auth-expired flag to false (`TrackPreferences.kt:29-33`).
5. If any step throws `Throwable`, `logout()` is called and the exception is swallowed. There is no toast and no log line. See section 13, item D-03.

`login(username, password)` is `login(password)` (`MyAnimeList.kt:249`). The password argument is the authorisation code. The generic login dialog is not used for MAL.

### 4.4 Deep-link return (`TrackLoginActivity.kt:9-17`, `:46-57`; `BaseOAuthLoginActivity.kt:19-36`)

1. Android delivers `aniyomi://myanimelist-auth?code=...` to `TrackLoginActivity` (manifest intent filter, see `p1-data-and-config.md` section 6).
2. `onCreate` shows `LoadingScreen` and calls `handleResult(intent.data)`.
3. Host `myanimelist-auth`: read query parameter `code`.
   - If `code != null`: `launchIO { trackerManager.myAnimeList.login(code); returnToSettings() }`.
   - If `code == null` (the user denied consent, or MAL returned an error): `trackerManager.myAnimeList.logout()` then `returnToSettings()`. This clears any existing MAL login. See section 13, item D-04.
4. `returnToSettings()` calls `finish()` then starts `MainActivity` with `FLAG_ACTIVITY_CLEAR_TOP | FLAG_ACTIVITY_SINGLE_TOP`.

### 4.5 Token exchange (`MyAnimeListApi.kt:50-64`)

- Method POST to `https://myanimelist.net/v1/oauth2/token`.
- Form body in this order: `client_id=<CLIENT_ID>`, `code=<authCode>`, `code_verifier=<codeVerifier>`, `grant_type=authorization_code`.
- Uses the shared client `client` (no Bearer interceptor).
- `awaitSuccess()` throws `HttpException` ("HTTP error <code>") on non-2xx.
- The response body is parsed as `MALOAuth` (`token_type`, `refresh_token`, `access_token`, `expires_in`, `created_at` optional, see `p1-data-and-config.md`).
- `codeVerifier` is read from the companion. If the process was restarted between `authUrl()` and the return, the verifier is `""` and MAL rejects the exchange.

### 4.6 Token expiry rule (`MALOAuth.kt:21-24`)

- `adjustedExpiresIn = expiresIn - 60`. The token is treated as expired one minute early.
- `isExpired() = createdAt + adjustedExpiresIn < System.currentTimeMillis() / 1000`. Comparison is strict. Seconds, not milliseconds.
- `createdAt` defaults to `System.currentTimeMillis() / 1000` at construction (`MALOAuth.kt:18-19`). Responses from the token endpoint do not carry `created_at`, so the default is "now" at parse time. The field is annotated `@EncodeDefault` so the value is written when the token is saved.

### 4.7 Per-request authentication (`MyAnimeListInterceptor.kt:18-39`)

Applied to every request made through `authClient` (`MyAnimeListApi.kt:48`). That client is the shared client plus this interceptor. It is used for everything except the token exchange (`MyAnimeListApi.kt:50-64`, which uses the shared client) and the refresh request (which is built manually).

Algorithm `intercept(chain)`:
1. If `trackPreferences.trackAuthExpired(this).get()` is true, throw `MALTokenExpired` (message `MAL: Login has expired`). Stop.
2. Let `originalRequest = chain.request()`.
3. If `oauth?.isExpired() == true`, call `refreshToken(chain)`. The return value is ignored. `refreshToken` updates `this.oauth` on success.
4. If `oauth == null`, throw `IOException("MAL: User is not authenticated")`.
5. Build `originalRequest` plus header `Authorization: Bearer <oauth.accessToken>`.
6. Return `chain.proceed(authRequest)`.

Note: `oauth` is read from the preference at construction (`MyAnimeListInterceptor.kt:15` calls `myanimelist.loadOAuth()`). `tokenExpired` is re-read from the preference on every request.

### 4.8 Refresh (`MyAnimeListInterceptor.kt:50-79`)

Algorithm `refreshToken(chain)` (the whole body runs inside `synchronized(this)`):
1. If `tokenExpired` is true, throw `MALTokenExpired`.
2. If `oauth` is non-null and not expired (another thread refreshed while this one waited), return it. This is the double-check.
3. Build the refresh request (see 4.9). Call `chain.proceed(...)`. Any `Throwable` from it is turned into `MALTokenRefreshFailed` (message `MAL: Failed to refresh account token`).
4. If `response.code == 401`: call `setAuthExpired()` (writes the auth-expired flag to true) and throw `MALTokenExpired`. The response body is not closed on this branch.
5. Otherwise, inside `runCatching`:
   - If `response.isSuccessful`, parse the body as `MALOAuth`.
   - Otherwise, close the response and return null.
6. If the parse gave a value: `this.oauth = it` and `saveOAuth(it)`, then return it.
7. Otherwise throw `MALTokenRefreshFailed`.

Outcome table:
- Refresh succeeds: the new token is stored and the request proceeds with the new Bearer.
- Refresh returns 401: auth-expired flag becomes true. Every later request throws `MALTokenExpired` until the next successful login. See section 13, item D-05.
- Refresh returns any other non-2xx (for example 400 `invalid_grant`): the request fails with `MALTokenRefreshFailed`. The flag is not set. The next request tries the refresh again. There is no backoff.
- Refresh throws (network error): `MALTokenRefreshFailed`. Same as above.

### 4.9 Refresh request (`MyAnimeListApi.kt:408-423`)

- Method POST to `https://myanimelist.net/v1/oauth2/token`.
- Form body in this order: `client_id=<CLIENT_ID>`, `refresh_token=<oauth.refreshToken>`, `grant_type=refresh_token`.
- Header `Authorization: Bearer <oauth.accessToken>` (the old access token, added by hand because this request does not pass through the Bearer step).

### 4.10 Auth-expired flag

- Storage: `trackPreferences.trackAuthExpired(this)`, a Boolean preference, default false (`TrackPreferences.kt:24-27`).
- Set to true only by `setAuthExpired()` (`MyAnimeList.kt:272-274`), which is called only on a 401 from the refresh endpoint.
- Reset to false by `setCredentials` (`TrackPreferences.kt:29-33`). That happens on successful login (`saveCredentials`), on `logout` (`BaseTracker.kt:28-30`), and nowhere else.
- While true, `isLoggedIn` still returns true (`BaseTracker.kt:32-34` checks only the username and password). The Settings row still shows "logged in". Only API calls fail.

### 4.11 Logout (`MyAnimeList.kt:262-266`)

1. `super.logout()` (`BaseTracker.kt:28-30`): `setCredentials(this, "", "")`. This clears username and password and sets auth-expired to false.
2. `trackPreferences.trackToken(this).delete()`.
3. `interceptor.setAuth(null)`, which calls `saveOAuth(null)`, which writes the string `"null"` to the token preference (because `json.encodeToString` of a null `MALOAuth?` is `"null"`). Net effect: the preference holds `"null"`, which `loadOAuth` decodes to null. See section 13, item D-07.

Logout is invoked from three places: the Settings logout dialog (`SettingsTrackingScreen.kt:392` calls `tracker.logout()`), the deep-link handler when `code` is missing (`TrackLoginActivity.kt:54`), and the failed-login path (`MyAnimeList.kt:258`).

### 4.12 Loading the saved token (`MyAnimeList.kt:280-286`)

`loadOAuth()` decodes `trackToken` as `MALOAuth`. Any exception (for example the empty string default, or `"null"`) returns null. Called once when the interceptor is constructed.

## 5. HTTP API catalogue

Unless noted, requests go through `authClient` (Bearer). Response bodies are decoded with the shared `Json` (`ignoreUnknownKeys = true`, `AppModule.kt:175`). Every call is wrapped in `withIOContext` (`MyAnimeListApi.kt`). Non-2xx status throws `HttpException("HTTP error <code>")` (`HttpException.kt:13`).

### 5.1 Token exchange: see 4.5.
### 5.2 Current user

`getCurrentUser()` (`MyAnimeListApi.kt:66-79`):
- GET `https://api.myanimelist.net/v2/users/@me`. No query. Built with `Request.Builder().url(...).get()`, so no cache-control header.
- Response: `{"name": "<username>"}`. Decoded as `MALUser`. Only `name` is used.

### 5.3 Search (manga)

`search(query)` (`MyAnimeListApi.kt:81-98`):
1. URL `https://api.myanimelist.net/v2/manga` with query parameters `q=<query.take(64)>` and `nsfw=true`. The comment in the source says MAL returns 400 for queries longer than 64 characters. `take(64)` counts UTF-16 code units.
2. GET via the `GET(...)` helper (default cache-control `max-age=10 min`, `Requests.kt:16`).
3. Decode `MALSearchResult`. Take `data[i].node.id` (Int).
4. For each id, start an `async` that calls `getMangaDetails(id)` (see 5.6). All run in parallel, with no concurrency limit.
5. `awaitAll()`. If any child fails, the whole call fails.
6. Filter: keep items where `!publishing_type.contains("novel")`. `publishing_type` is already `media_type` with `_` replaced by a space, so `novel` and `light novel` are removed.
7. Return in MAL result order.

### 5.4 Search (anime)

`searchAnime(query)` (`MyAnimeListApi.kt:100-117`):
- URL `https://api.myanimelist.net/v2/anime` with query parameters in this exact order: `q=<query.take(64)>`, `q=<query>` (the full query, not truncated), `nsfw=true`. The second `q` is a duplicate key. See section 13, item D-09.
- Same steps 2 to 5 as manga, using `getAnimeDetails`.
- No novel filter.

### 5.5 Details (manga and anime)

`getMangaDetails(id: Int)` (`MyAnimeListApi.kt:119-148`):
- GET `https://api.myanimelist.net/v2/manga/{id}?fields=id,title,synopsis,num_chapters,mean,main_picture,status,media_type,start_date`. The `fields` value is exactly that string, with commas and no spaces.
- Decode `MALManga`.
- Build `MangaTrackSearch.create(trackId)` (`tracker_id = 1`) with:
  - `remote_id = id`
  - `title = title`
  - `summary = synopsis`
  - `total_chapters = numChapters`
  - `score = mean` (default -1.0 if absent)
  - `cover_url = main_picture.large` or `""`
  - `tracking_url = "https://myanimelist.net/manga/" + remote_id`
  - `publishing_status = status.replace("_", " ")`
  - `publishing_type = media_type.replace("_", " ")`
  - `start_date = start_date ?: ""`
- Other fields keep their defaults: `last_chapter_read = 0.0`, `status = 0`, `started_reading_date = 0`, `finished_reading_date = 0`, `private = false`, `library_id = null`.

`getAnimeDetails(id: Int)` (`MyAnimeListApi.kt:150-179`): same with `/anime/`, `fields=id,title,synopsis,num_episodes,mean,main_picture,status,media_type,start_date`, `total_episodes = numEpisodes`, `tracking_url = https://myanimelist.net/anime/{id}`. Other fields as above, with `last_episode_seen = 0.0`, `started_watching_date = 0`, `finished_watching_date = 0`.

Note: the details response does not contain the user's list status. Search results and `my:` results therefore carry status `0`, `score = mean`, and progress `0`.

### 5.6 List entry lookup (`findListItem`)

`findListItem(track: MangaTrack): MangaTrack?` (`MyAnimeListApi.kt:251-267`):
1. GET `https://api.myanimelist.net/v2/manga/{track.remote_id}?fields=num_chapters,my_list_status{start_date,finish_date}` via `GET(...)` (cache-control helper).
2. Decode `MALListMangaItem`.
3. `track.total_chapters = item.numChapters` (always, even if the entry is not in the user's list).
4. If `item.myListStatus != null`, return `parseMangaItem(myListStatus, track)`, which is `track` itself, mutated. Otherwise return null.

`findListItem(track: AnimeTrack): AnimeTrack?` (`MyAnimeListApi.kt:269-285`): same with `/anime/`, `fields=num_episodes,my_list_status{start_date,finish_date}`, `track.total_episodes = numEpisodes`.

Because the returned object is the same instance as the argument, callers that do `copyPersonalFrom(remote)` copy onto themselves. See section 13, item D-02.

### 5.7 Entry update (PUT)

`updateItem(track: MangaTrack)` (`MyAnimeListApi.kt:181-206`):
- Method PUT, URL `https://api.myanimelist.net/v2/manga/{track.remote_id}/my_list_status`. Built with `Request.Builder().url(...).put(form)`, so no cache-control header.
- Form, in this order:
  1. `status` = `track.toMyAnimeListStatus() ?: "reading"`
  2. `is_rereading` = `(track.status == REREADING).toString()` (`"true"` or `"false"`)
  3. `score` = `track.score.toString()`
  4. `num_chapters_read` = `track.last_chapter_read.toInt().toString()` (truncates toward zero, so 12.7 becomes `12`)
  5. `start_date` = `convertToIsoDate(track.started_reading_date)`, only if non-null
  6. `finish_date` = `convertToIsoDate(track.finished_reading_date)`, only if non-null
- Response decoded as `MALListMangaItemStatus`, then `parseMangaItem(response, track)` (see 5.8).

`updateItem(track: AnimeTrack)` (`MyAnimeListApi.kt:208-233`):
- URL `https://api.myanimelist.net/v2/anime/{track.remote_id}/my_list_status`.
- Form, in this order: `status` (default `"watching"`), `is_rewatching` (`(track.status == REWATCHING).toString()`), `score`, `num_watched_episodes` (= `last_episode_seen.toInt()`), `start_date` (from `started_watching_date`), `finish_date` (from `finished_watching_date`).
- Response decoded as `MALListAnimeItemStatus`, then `parseAnimeItem`.

Note the form key `num_watched_episodes` for anime. The response key is `num_episodes_watched` (see `p1-data-and-config.md`). The two names differ on purpose in the source.

### 5.8 Response application

`parseMangaItem(listStatus, track)` (`MyAnimeListApi.kt:344-353`):
- `status = if (listStatus.isRereading) REREADING else getStatus(listStatus.status)`
- `last_chapter_read = listStatus.numChaptersRead` (Double)
- `score = listStatus.score.toDouble()`
- If `startDate != null`: `started_reading_date = parseDate(startDate)`
- If `finishDate != null`: `finished_reading_date = parseDate(finishDate)`
- Dates that are null in the response keep the previous local value.

`parseAnimeItem(listStatus, track)` (`MyAnimeListApi.kt:355-364`): same with `isRewatching`, `last_episode_seen = numEpisodesWatched`, `started_watching_date`, `finished_watching_date`.

### 5.9 Delete

`deleteMangaItem(track)` (`MyAnimeListApi.kt:235-241`): DELETE `https://api.myanimelist.net/v2/manga/{track.remoteId}/my_list_status` through `authClient`, then `awaitSuccess()`. Response body is ignored.

`deleteAnimeItem(track)` (`MyAnimeListApi.kt:243-249`): same with `/anime/`.

A 404 (entry not on the list) throws `HttpException("HTTP error 404")`. The caller logs it and does nothing else (see 9).

### 5.10 My-list paging (`getListPage`, `MyAnimeListApi.kt:323-342`)

- GET `https://api.myanimelist.net/v2/users/@me/mangalist` with query `fields=list_status{start_date,finish_date}`, `limit=250`, and `offset=<offset>` only when offset is greater than 0. Built with `Request.Builder().get()`, so no cache-control header.
- Decode `MALUserSearchResult` with `data` (list of `node{id,title}`) and `paging.next` (String?).
- The endpoint is always the manga list. The anime variant uses it too. See section 13, item D-01.

## 6. Tracker operations (`MyAnimeList.kt`)

### 6.1 Progress-driven update (`update`)

`update(track: MangaTrack, didReadChapter: Boolean = false)` (`MyAnimeList.kt:125-141`):

```
if track.status != COMPLETED:
    if didReadChapter:
        if track.last_chapter_read.toLong() == track.total_chapters and track.total_chapters > 0:
            track.status = COMPLETED
            track.finished_reading_date = now_ms
        else if track.status != REREADING:
            track.status = READING
            if track.last_chapter_read == 1.0:
                track.started_reading_date = now_ms
return api.updateItem(track)
```

`update(track: AnimeTrack, didWatchEpisode: Boolean = false)` (`MyAnimeList.kt:143-159`): same with `last_episode_seen`, `total_episodes`, `WATCHING` (instead of READING), `REWATCHING` (instead of REREADING), `finished_watching_date`, `started_watching_date`.

Notes:
- When the entry becomes complete, the completion branch runs before the rereading check. A re-read that reaches the last chapter becomes COMPLETED.
- A `total_chapters` of 0 (unknown) never triggers completion.
- `started_*_date` is set only when progress equals exactly 1.0. Reading chapter 1 later does not reset it.

Callers:
- `TrackChapter` and `TrackEpisode` call `update(track, true)` after `refresh` (`domain/track/manga/interactor/TrackChapter.kt:40-43`, `domain/track/anime/interactor/TrackEpisode.kt:39-42`).
- Everything else goes through the interface defaults, which call `update(track)` with `didReadChapter = false`.

### 6.2 Setters through the interface (`MangaTracker.kt`, `AnimeTracker.kt`)

These run for MAL unchanged:
- `setRemoteMangaStatus(track, status)` / `setRemoteAnimeStatus`: set status; if status == COMPLETED and total != 0, set progress to total; then `updateRemote`.
- `setRemoteLastChapterRead(track, n)` (`MangaTracker.kt:74-89`):
  - If `last_chapter_read == 0.0`, `0.0 < n`, and `status != REREADING`, set `status = READING` (`getReadingStatus()`).
  - `last_chapter_read = n.toDouble()`.
  - If `total_chapters != 0` and `last_chapter_read.toLong() == total_chapters`, set `status = COMPLETED` and `finished_reading_date = now_ms`.
  - `updateRemote(track)`.
- `setRemoteLastEpisodeSeen(track, n)` (`AnimeTracker.kt:75-88`): same with `WATCHING`, `REWATCHING`.
- `setRemoteScore(track, scoreString)`: see section 2.
- `setRemoteStartDate` / `setRemoteFinishDate`: set the epoch and call `updateRemote`.
- `setRemotePrivate(track, private)`: sets `track.private` and calls `updateRemote`. MAL has no private flag, so `private` is stored locally and is not sent to MAL. The field is ignored in the PUT form.
- `updateRemote(track)` (`MangaTracker.kt:111-121`): `update(track)`, then `insertTrack.await(track.toDomainTrack(idRequired = false))`. Any `Exception` is logged at ERROR with `"Failed to update remote track data id=<id>"` and the message is shown as a toast.

### 6.3 Bind a new title (`bind`)

`bind(track: MangaTrack, hasReadChapters: Boolean)` (`MyAnimeList.kt:169-187`):

```
remote = api.findListItem(track)           // 5.6, mutates track
if remote != null:                         // entry is on the user's list
    track.copyPersonalFrom(remote)         // no-op, same instance (D-02)
    track.remote_id = remote.remote_id     // no-op
    if track.status != COMPLETED:
        isRereading = track.status == REREADING
        track.status = if (!isRereading and hasReadChapters) READING else track.status
    return update(track)                   // didReadChapter = false -> PUT (5.7)
else:                                      // not on the list
    track.status = if hasReadChapters then READING else PLAN_TO_READ
    track.score = 0.0
    return add(track)                      // PUT (5.7), creates the entry
```

`add(track)` is `api.updateItem(track)` (`MyAnimeList.kt:117-119`).

Anime `bind` (`MyAnimeList.kt:189-207`): same with `hasSeenEpisodes`, `WATCHING`, `PLAN_TO_WATCH`, `REWATCHING`, and `update(track)` / `add(track)` on the anime path.

Caller: `AddMangaTracks.bind` (`domain/track/manga/interactor/AddMangaTracks.kt:36`) passes `hasReadChapters = allChapters.any { it.read }`. After bind it inserts the row and, if local progress is ahead, calls `setRemoteLastChapterRead`. `AddAnimeTracks.bind` is the anime counterpart (`domain/track/anime/interactor/AddAnimeTracks.kt:36`).

### 6.4 Refresh (`refresh`)

`refresh(track)` (`MyAnimeList.kt:241-247`): `api.findListItem(track) ?: add(track)`.

- If the title is on the user's list, return the list state (the track mutated by `findListItem`).
- If not on the list, PUT the local state, which creates the entry on MAL. Observed: a refresh recreates a title that was removed on the website. See section 13, item D-10.

Callers: `TrackChapter` / `TrackEpisode` (before every progress write) and `RefreshMangaTracks` / `RefreshAnimeTracks`.

### 6.5 Delete (`delete`)

`delete(track: DomainMangaTrack)` and `delete(track: DomainAnimeTrack)` (`MyAnimeList.kt:161-167`) call `api.deleteMangaItem(track)` and `api.deleteAnimeItem(track)`, which use `track.remoteId`.

The UI decides whether to call it (see `p1-ui-parity.md`). The local row removal is done by the caller, not here.

### 6.6 Login state

- `isLoggedIn`: username and password non-empty (`BaseTracker.kt:32-34`).
- `isLoggedInFlow`: a combine of username and password change flows (`BaseTracker.kt:36-43`).
- `getUsername()`, `getPassword()`, `saveCredentials`: pass-through to `TrackPreferences`.

## 7. Search routing (`searchManga`, `searchAnime`, `MyAnimeList.kt:209-239`)

Manga routing in order:
1. If `query.startsWith("id:")` and `query.substringAfter("id:").toIntOrNull()` is non-null, return `listOf(api.getMangaDetails(id))`. A single result with no novel filter. `"id:"` followed by a non-integer falls through to step 2.
2. If `query.startsWith("my:")`, return `api.findListItems(query.substringAfter("my:"))`.
3. Otherwise return `api.search(query)`.

Anime routing: the same three steps. Step 1 calls `getAnimeDetails`. Step 2 calls `api.findListItemsAnime(...)`. Step 3 calls `api.searchAnime(query)`.

Empty `my:` query: `substringAfter` returns `""`, `contains("")` is true for every title, so every page of the list is fetched and every entry's details are requested. This is N+1 requests with no cap. See section 11.

### 7.1 `my:` search for manga (`findListItems`, `MyAnimeListApi.kt:287-303`)

```
findListItems(query, offset = 0):
    page = getListPage(offset)                      // 5.10
    matches = page.data
        .filter { node.title contains query, ignoreCase = true }
        .map { async getMangaDetails(node.id) }
        .awaitAll()
    if page.paging.next is not null and not blank:
        return matches + findListItems(query, offset + 250)
    return matches
```

Results are in list order, page by page. Details come from `getMangaDetails`, so status and progress are not included (see 5.5).

### 7.2 `my:` search for anime (`findListItemsAnime`, `MyAnimeListApi.kt:305-321`)

Same as 7.1 but:
- `getListPage` still fetches the manga list (`users/@me/mangalist`).
- Matches are passed to `getAnimeDetails`, which uses the manga ids as anime ids.

Result: the anime search returns anime details for ids taken from the manga list. Most ids do not exist as anime, so `getAnimeDetails` throws `HTTP error 404` and the whole search fails (`awaitAll` rethrows). When the title filter matches nothing, the result is empty and no detail call is made. See section 13, item D-01.

## 8. Errors and user-visible strings

### 8.1 Errors raised by this code

| Where | Exception | Message (exact) |
|---|---|---|
| `MyAnimeListInterceptor.kt:29` | `IOException` | `MAL: User is not authenticated` |
| `MyAnimeListInterceptor.kt:82` | `MALTokenRefreshFailed : IOException` | `MAL: Failed to refresh account token` |
| `MyAnimeListInterceptor.kt:83` | `MALTokenExpired : IOException` | `MAL: Login has expired` |
| `core/common/.../HttpException.kt:13` | `HttpException : IllegalStateException` | `HTTP error <code>` |
| `SimpleDateFormat.parse` | `java.text.ParseException` | JDK message (for example `Unparseable date: "2020-05"`) |

These messages are English literals. No `MR.strings` or `AYMR.strings` key is used for them. The ParseException text is the JDK text.

### 8.2 Where errors become UI

- `updateRemote` (`MangaTracker.kt:117-119`, `AnimeTracker.kt:116-118`): logs at ERROR and shows `e.message` as a toast.
- `register` (`MangaTracker.kt:59-63`, `AnimeTracker.kt:60-64`): toast with `e.message` on failure.
- `TrackLoginActivity`: no toast. Login failures are silent.
- Delete (`AnimeTrackInfoDialog.kt:860-868`, `MangaTrackInfoDialog.kt` counterpart): exceptions are logged at ERROR with `"Failed to delete anime entry from service"` (or manga counterpart). No toast.
- Settings login toggle: no error path (browser flow).
- Logout: toast `logout_success` ("You are now logged out"), from `SettingsTrackingScreen.kt` logout dialog.

### 8.3 Other user-visible strings used around MAL

Strings from `i18n/src/commonMain/moko-resources/base/strings.xml`:
- `logout_title` = `Log out from %1$s?` (tracker name)
- `logout_success` = `You are now logged out`
- `logout` = `Log out` (button)
- `action_cancel` = `Cancel`
- `login_success` = `Logged in` (generic login dialog; not used for MAL)
- `track_delete_title` = `Remove %s tracking?`
- `track_delete_text` = `This will remove the tracking locally.`
- `track_delete_remote_text` = `Also remove from %s`
- `action_ok` = `OK`, `action_remove` = `Remove`
- `services` = `Trackers` (settings group title)
- `enhanced_services_not_installed` = `Available but source not installed: %s` (not MAL)

From `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`:
- `pref_auto_update_manga_sync` = `Update progress after reading/watching`
- `pref_track_on_add_library` = `Open track menu on adding to library`
- `pref_auto_update_manga_on_mark_read` = `Update progress when marked as read/watched`

## 9. Side effects

- Network: see section 5. Every mutating call is a PUT or DELETE on MAL, sent immediately. There is no local queue inside the MAL code.
- Local database: MAL code itself writes nothing to the database. Callers write the row with `InsertMangaTrack` / `InsertAnimeTrack` after `update`, `bind`, `refresh` (see 6.2 and 6.3). Field mapping is in `domain/track/manga/model/MangaTrack.kt` (`toDbTrack`, `toDomainTrack`).
- Preferences: credentials, the token JSON, and the auth-expired flag (see `p1-data-and-config.md`).
- Offline behaviour (owned by the callers, area:27-tracking-core): `TrackEpisode.await` checks `context.isOnline()`. If offline it writes `delayedTrackingStore.addAnime(...)` and, when `setupJobOnFailure` is true, calls `DelayedAnimeTrackingUpdateJob.setupTask`. If online and the MAL call throws, the same store is used (`TrackEpisode.kt:46-48` covers the offline branch and the same calls). `TrackChapter.await` has no online check. Any exception in its MAL path goes to `delayedTrackingStore.addManga(...)` and `DelayedMangaTrackingUpdateJob.setupTask` (`TrackChapter.kt:47-49`). MAL code does not schedule work itself.
- Notifications: none from MAL code.
- Logs: `logcat` at ERROR in `updateRemote`, and in the delete dialog. Interceptor errors are not logged.

## 10. Concurrency and dispatchers

- All MAL API functions use `withIOContext` (`MyAnimeListApi.kt`).
- `search`, `searchAnime`, `findListItems`, `findListItemsAnime` use `async` / `awaitAll` with no concurrency limit. Each `getMangaDetails` / `getAnimeDetails` call is a separate HTTP request.
- `findListItems` / `findListItemsAnime` are recursive over pages, so pages are fetched strictly one after another.
- `MyAnimeListInterceptor.refreshToken` is `synchronized(this)`. Multiple requests that find the token expired wait for one refresh. The check for `tokenExpired` and `oauth` before the lock is unsynchronised, so there is a benign race.
- `MyAnimeListApi.codeVerifier` is a companion `var` with no lock. `authUrl()` and the token exchange are not synchronised.
- `MyAnimeList.login` runs on `lifecycleScope.launchIO` (`TrackLoginActivity.kt:23`, `:36`, `:49`, `:62`, `:75`). `returnToSettings()` runs after `login` returns.
- Delete and local unregister run in parallel: `AnimeTrackInfoDialog.kt:833-834` calls `unregisterTracking` and `deleteAnimeFromService` back to back, each in `launchNonCancellable`.

## 11. Timings and limits

| Item | Value | Source |
|---|---|---|
| Connect timeout | 30 s | `NetworkHelper.kt:23` (`connectTimeout(30, SECONDS)`) |
| Read timeout | 30 s | `NetworkHelper.kt:24` |
| Call timeout | 2 min | `NetworkHelper.kt:25` |
| HTTP cache | 5 MiB | `NetworkHelper.kt:26-29` |
| GET cache-control hint | `max-age=10 min` | `Requests.kt:16` (applied only by `GET(...)` helper) |
| Token early-expiry margin | 60 s | `MALOAuth.kt:22` |
| Title query max length | 64 characters (UTF-16 units) | `MyAnimeListApi.kt:85`, `:104` |
| List page size | 250 | `MyAnimeListApi.kt:388` |
| Score range | 0 to 10 integer | `MyAnimeList.kt:49-51` |
| Score sent to MAL | decimal string such as `"8.0"` | `MyAnimeListApi.kt:186` |
| Token refresh attempts | one per request while expired (no backoff) | `MyAnimeListInterceptor.kt:50-79` |
| Concurrent detail requests | unlimited | `MyAnimeListApi.kt:93`, `:113`, `:293`, `:311` |
| Max results | not set by the client (server default) | URLs contain no `limit` for search |

## 12. Interactions with other areas

- area:27-tracking-core: owns the generic `Tracker` interface, the default `setRemote*` methods, `TrackChapter`, `TrackEpisode`, `AddMangaTracks`, `AddAnimeTracks`, `RefreshMangaTracks`, `RefreshAnimeTracks`, the delayed-tracking store and jobs, and the Settings tracking screen. MAL implements their contracts.
- area:12-network-stack: owns the shared OkHttp client, DoH, user agent, Cloudflare handling, and the request builders. The MAL code assumes `GET`, `POST`, `DELETE` builders and `awaitSuccess`.
- area:02-preferences-system: owns `PreferenceStore`, `Preference.privateKey`, and the `getEnum` implementation used by `autoUpdateTrackOnMarkRead`.
- area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations: own `manga_sync` and `anime_sync`.
- area:26-library-update-jobs: owns WorkManager jobs used by delayed tracking (`DelayedMangaTrackingUpdateJob`, `DelayedAnimeTrackingUpdateJob`).
- area:22-backup-restore: MAL credentials are private keys and are excluded from backups (`Preference.kt:32-34` for `privateKey`, `Preference.kt:48` for `PRIVATE_PREFIX`). A restored backup has the MAL row but no login.
- AniChart (`app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:41`): converts a MAL id to an AniList id through `POST https://graphql.anilist.co` with the query `Media(idMal:$idMal,type: ANIME) { id }`. It uses `remoteId` as the MAL id. Owned by the anime detail area.
- AniSkip (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2044`): uses `track.remoteId` as the MAL id for the skip-times lookup. Owned by the player area.
- TrackLoginActivity and the manifest: Android-only, see section 14.

## 13. Defects and quirks found in the source

Each item is labelled `D-nn`. The Proposal says what Kurobayomi should do. The default is to keep upstream behaviour unless the Proposal says otherwise.

- D-01 (anime `my:` search fetches the manga list). `MyAnimeListApi.kt:305-321` calls `getListPage`, which always uses `/users/@me/mangalist` (`:325`). Observed result: the anime `my:` search returns `HTTP error 404` or unrelated anime. Proposal: pass the list type into `getListPage` (`animelist` for anime). Use `GET /users/@me/animelist` with the same `fields`, `limit`, `offset` and `paging.next` rules. Verify the endpoint shape against a live account before shipping.
- D-02 (self-copy in bind). `MyAnimeList.kt:172-173` and `:192-193` call `copyPersonalFrom(remoteTrack)` where `remoteTrack === track`. The copy is a no-op. Behaviour is still correct because `findListItem` already wrote the remote values. Proposal: have `find_list_item` return a fresh value and keep the copy, or drop the copy. Same observable result.
- D-03 (login failure is silent). `MyAnimeList.kt:257-259` swallows every error. Observed: a failed login clears credentials, shows no error, and returns to Settings with the row logged out. Proposal: return `Result<(), MalError>` and show a toast with the error text.
- D-04 (cancel clears an existing login). `TrackLoginActivity.kt:53-55`: if the redirect has no `code` (consent denied), `logout()` runs. Observed: a user who cancels a re-login loses the working login. Proposal: on missing `code`, show a message and keep the login.
- D-05 (401 from refresh sets a permanent flag). Once `setAuthExpired()` is set, the only way out is a new login (`TrackPreferences.kt:29-33`). Proposal: keep the flag but show "Log in again" in the Settings row (see `p1-ui-parity.md`), and allow re-login without logout.
- D-06 (repeated refresh with no backoff). A non-401 refresh failure is retried on every request. Proposal: remember the last refresh failure time and wait 30 s before retrying. The upper bound is one retry per 30 s.
- D-07 (logout writes `"null"`). `MyAnimeList.kt:264-265`: the token is deleted, then `setAuth(null)` writes `"null"` back. Harmless because `loadOAuth` decodes it as null. Proposal: delete the key only, do not write `"null"`.
- D-08 (partial dates crash the call). `parseDate` throws on `YYYY-MM` or `YYYY`. Observed: `findListItem`, `updateItem` and `getListPage` fail. Proposal: accept `YYYY-MM-DD`, `YYYY-MM` (day 1) and `YYYY` (January 1), treat other input as 0.
- D-09 (duplicate `q` in anime search). `MyAnimeListApi.kt:104-105` adds `q` twice. Observed: for queries longer than 64 characters the server receives two `q` values, one of which is 64 characters and one of which is longer. MAL's handling of duplicate keys is not documented in the source. Proposal: send one `q=<query.take(64)>`, matching manga.
- D-10 (refresh recreates removed entries). `MyAnimeList.kt:241-247` re-adds any title that is missing on MAL. Proposal: keep the upstream behaviour, but make it visible: after a refresh that re-adds, show a toast "Re-added to MyAnimeList".
- D-11 (score as decimal string). The PUT form sends `"8.0"`. Upstream ships this and it is accepted in production. Proposal: send the integer `8` (`score.toInt()`), which matches MAL's integer score field. Verify with one live PUT before switching.
- D-12 (score -1 sent). `setRemoteScore` with a string that is not in the list yields -1.0. Proposal: reject with an error instead of sending -1.
- D-13 (N+1 detail requests). Search and `my:` search fetch details per result, unbounded. Proposal: bounded concurrency of 4 with `buffer_unordered(4)`, and use the `fields` parameter on the list endpoint where MAL supports it (verify first).
- D-14 (manga search filters novels, anime does not). `MyAnimeListApi.kt:95` filters `novel`. Anime has no filter. Keep. Proposal: document in the UI that novel titles are hidden.
- D-15 (`my:` search returns no list status). Details do not carry `my_list_status`. The result rows show default status and score. Proposal: when the result comes from `my:`, fetch `my_list_status` for each id in the same call (`fields` on the details endpoint). Verify first.
- D-16 (local read forces READING over on-hold). `bind` sets READING when local chapters are read, even when remote status is ON_HOLD or DROPPED (`MyAnimeList.kt:175-179`). Keep upstream behaviour, document it.
- D-17 (partial `status` string). `getStatus` maps unknown strings to READING, including for anime (`MyAnimeListUtils.kt:37`). Proposal: map unknown values to the lowest-risk local code and log a warning.
- D-18 (MAL response body not closed on 401). `MyAnimeListInterceptor.kt:60-63` returns without closing the response. Proposal: always close it (Rust drops the response, so this disappears).
- D-19 (token lifetime shorter than 60 s). If `expires_in < 60`, `adjustedExpiresIn` is negative and the token is always expired (`MALOAuth.kt:22-24`). Proposal: clamp the margin to half of `expires_in`.
- D-20 (verifier lost on restart). `codeVerifier` is in memory only (`MyAnimeListApi.kt:390`). Proposal: store the verifier with the pending login in the config directory until the redirect is handled (see T-29a-038).
- D-21 (`MALUser` requires `name`). If MAL returns no `name`, `getCurrentUser` throws and the login is rolled back. Proposal: keep, since a login without a username cannot show who is logged in.
- D-22 (`MALOAuth.createdAt` default). The default is "now" at parse time. This is correct for responses without `created_at`. Keep.
- D-23 (`toMyAnimeListStatus` default). Unknown local status becomes `reading` or `watching` on the wire, which silently changes the remote state. Proposal: refuse to PUT an unknown code, and return an error.

## 14. Android-only behaviour that does not carry over

- `TrackLoginActivity` as an exported Android activity with the `aniyomi://myanimelist-auth` intent filter. Replaced by a loopback listener or a custom URI handler (see `p1-rust-design.md` section 6).
- `BaseOAuthLoginActivity` loading screen and `MainActivity` return. Replaced by a GTK dialog.
- `context.openInBrowser(url, forceDefaultBrowser = true)`. Replaced by `gio::AppInfo::launch_default_for_uri` (or `gtk::UriLauncher`) with the default browser.
- `Toast` calls. Replaced by `adw`-free GTK toast overlay or an `AdwToast`-like inline message (no libadwaita, so a plain GTK `Revealer` overlay). See `p1-ui-parity.md`.
- `Context.isOnline()` is in callers, not MAL code. Replaced by the network-state helper of area:12-network-stack.
- WorkManager (`DelayedMangaTrackingUpdateJob`) is owned by area:26-library-update-jobs. Replaced by a tokio task.
