# Area 30-tracker-kitsu: behaviour spec

Repo root: `/home/blackbox/aniyomi-src`. Paths are repo-relative. `OBSERVED` = what the Kotlin does. `Proposal:` = what the Rust port should do.

Sources used for this spec (all in scope unless marked):
- `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt` (K)
- `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt` (A)
- `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt` (I)
- `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuDateHelper.kt` (D)
- `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuUtils.kt` (U)
- `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/*.kt` (DTO)
- Outside scope, read for exact behaviour: `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt`, `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`, `app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt`, `app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt`, `app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt`, `app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt`, `app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt` (mapper), `app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrackImpl.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt`, `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt`, `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt`, `core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt`, `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt`, `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt`, `data/src/main/sqldelight/data/manga_sync.sq`, `data/src/main/sqldelightanime/dataanime/anime_sync.sq`, `i18n/src/commonMain/moko-resources/base/strings.xml`, `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`.

---

## 1. Identity and capabilities

OBSERVED (K:27-47, K:57-59, K:61-67, K:87-95)
- `Kitsu(id: Long)` extends `BaseTracker(id, "Kitsu")`. The id is `TrackerManager.KITSU = 3L` (`app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt`, the `companion object` constant `KITSU = 3L`).
- Implements `AnimeTracker`, `MangaTracker`, `DeletableMangaTracker`, `DeletableAnimeTracker`.
- `supportsReadingDates = true`, `supportsPrivateTracking = true`.
- `getLogo()` returns `R.drawable.ic_tracker_kitsu`; `getLogoColor()` returns `Color.rgb(51, 37, 50)` = `#332532`.
- `getRereadingStatus()` and `getRewatchingStatus()` return `-1` (no re-read/re-watch concept).
- `getCompletionStatus()` returns `COMPLETED` (2) for both kinds.
- `isLoggedIn` (from BaseTracker, `BaseTracker.kt:32-34`) is true iff `getUsername().isNotEmpty() && getPassword().isNotEmpty()`. It is credential-based, not token-based. The token may be absent while `isLoggedIn` is true, and the reverse is possible after a partial login (see section 4.4).

Proposal: `Kitsu` exposes `id = 3`, `name = "Kitsu"`, `supports_reading_dates = true`, `supports_private_tracking = true`, `kinds = [Manga, Anime]`, `deletable = true`, `completion_status = 2`, `rereading_status = None`, `rewatching_status = None`. Logo: see T-30a-030.

## 2. Status model

### 2.1 Constants (K:37-45)
| Constant | Value | Used for |
|---|---|---|
| `READING` | 1 | manga "currently reading" |
| `WATCHING` | 11 | anime "currently watching" |
| `COMPLETED` | 2 | both |
| `ON_HOLD` | 3 | both |
| `DROPPED` | 4 | both |
| `PLAN_TO_READ` | 5 | manga "want to read" |
| `PLAN_TO_WATCH` | 15 | anime "want to watch" |

### 2.2 Status lists shown to the user (K:61-67)
- Manga: `listOf(READING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_READ)` = `[1, 2, 3, 4, 5]`.
- Anime: `listOf(WATCHING, PLAN_TO_WATCH, COMPLETED, ON_HOLD, DROPPED)` = `[11, 15, 2, 3, 4]`.
- Order is the display order. Do not sort.

### 2.3 Labels (K:69-85)
Manga `getStatusForManga`:
| Status | String resource |
|---|---|
| 1 READING | `AYMR.strings.currently_reading` = "Currently reading" |
| 5 PLAN_TO_READ | `AYMR.strings.want_to_read` = "Want to read" |
| 2 COMPLETED | `MR.strings.completed` = "Completed" |
| 3 ON_HOLD | `MR.strings.on_hold` = "On hold" |
| 4 DROPPED | `MR.strings.dropped` = "Dropped" |
| else | `null` (no label) |

Anime `getStatusForAnime`:
| Status | String resource |
|---|---|
| 11 WATCHING | `AYMR.strings.currently_watching` = "Currently watching" |
| 15 PLAN_TO_WATCH | `AYMR.strings.want_to_watch` = "Want to watch" |
| 2 / 3 / 4 | same as manga |
| else | `null` |

Note: a status value is labelled per kind. The manga label function is never called with a watch status by the Kitsu code. A status not in the list yields `null`.

### 2.4 Wire mapping (U:6-22)
Local to API (`toApiStatus`):
- Manga: `READING`→`"current"`, `COMPLETED`→`"completed"`, `ON_HOLD`→`"on_hold"`, `DROPPED`→`"dropped"`, `PLAN_TO_READ`→`"planned"`, else → `throw Exception("Unknown status")`.
- Anime: `WATCHING`→`"current"`, `COMPLETED`→`"completed"`, `ON_HOLD`→`"on_hold"`, `DROPPED`→`"dropped"`, `PLAN_TO_WATCH`→`"planned"`, else → `throw Exception("Unknown status")`.
- Note the kind check: manga `toApiStatus` throws for `WATCHING` (11) and `PLAN_TO_WATCH` (15), and anime throws for `READING` (1) and `PLAN_TO_READ` (5).

API to local (DTO:36-43 manga, DTO:70-77 anime):
- Manga: `"current"`→1, `"completed"`→2, `"on_hold"`→3, `"dropped"`→4, `"planned"`→5, else → `throw Exception("Unknown status")`.
- Anime: `"current"`→11, `"completed"`→2, `"on_hold"`→3, `"dropped"`→4, `"planned"`→15, else → `throw Exception("Unknown status")`.
- Unknown API status strings throw. The surrounding call (bind, refresh) fails. No fallback.

Proposal: keep the same tables. Unknown status from the server returns `KitsuError::UnknownStatus(String)`, and the UI shows the error (T-30a-015).

## 3. Score model

### 3.1 Score list and index (K:97-104)
`getScoreList()` = `["0"] + [format(i / 2f) for i in 2..20]` with `DecimalFormat("0.#")`. Result (20 entries, index 0..19):
`["0", "1", "1.5", "2", "2.5", "3", "3.5", "4", "4.5", "5", "5.5", "6", "6.5", "7", "7.5", "8", "8.5", "9", "9.5", "10"]`

`indexToScore(index)` = `if (index > 0) (index + 1) / 2.0 else 0.0`.
- index 0 → 0.0; index 1 → 1.0; 2 → 1.5; 3 → 2.0; 4 → 2.5; 5 → 3.0; 6 → 3.5; 7 → 4.0; 8 → 4.5; 9 → 5.0; 10 → 5.5; 11 → 6.0; 12 → 6.5; 13 → 7.0; 14 → 7.5; 15 → 8.0; 16 → 8.5; 17 → 9.0; 18 → 9.5; 19 → 10.0.

`setRemoteScore(track, scoreString)` (in the `AnimeTracker` / `MangaTracker` defaults, outside scope) computes `indexToScore(getScoreList().indexOf(scoreString))`. If the string is absent, `indexOf` = -1 and `indexToScore(-1)` = `0.0`.

### 3.2 Display (K:106-114)
`displayScore(track)` = `DecimalFormat("0.#").format(track.score)`.
- 7.0 → `"7"`, 7.5 → `"7.5"`, 0.0 → `"0"`. A score of -1.0 would format as `"-1"`.
- `DecimalFormat` without an explicit locale uses the default locale's decimal separator. On a German device this is `"7,5"`. OBSERVED.

Proposal: format with `.` always (T-30a-035).

### 3.3 Wire (U:24-30)
`toApiScore()`: `if (score > 0) (score * 2).toInt().toString() else null`.
- 1.0 → `"2"`, 1.5 → `"3"`, 10.0 → `"20"`, 0.0 → `null`.
- `toInt()` truncates toward zero. Scores are multiples of 0.5 so this is exact.
- A `null` is sent as a JSON `null` in the PATCH body. Sending `null` clears the remote rating (see section 6.3).

### 3.4 Read (DTO:44 and DTO:78)
`score = userDataAttrs.ratingTwenty?.let { it / 2.0 } ?: 0.0`.
- `ratingTwenty` 2 → 1.0, 20 → 10.0, null → 0.0, 1 → 0.5.

### 3.5 Search score (DTO:48 and DTO:66)
`score = averageRating ?: -1.0`. This value is the raw Kitsu `averageRating` and is not divided by 2. It is NOT on the 0-10 scale used by the entry score. Kitsu's documented scale for `averageRating` is not confirmed by this source. Treat it as opaque; do not feed it into `toApiScore`. (Unverified against the live API.)

## 4. Authentication lifecycle

### 4.1 Login (K:226-231; A:395-410; A:412-425)
Steps, in order:
1. `api.login(username, password)` → `POST https://kitsu.app/api/oauth/token` with form body, using `client` (the base client, not `authClient`, so no bearer header). Form fields in order: `username`, `password`, `grant_type=password`, `client_id`, `client_secret`. Response parsed into `KitsuOAuth`. Any non-2xx → `HttpException(code)` (`"HTTP error <code>"`).
2. `interceptor.newAuth(token)` → sets the in-memory token and calls `kitsu.saveToken(token)`, which writes the JSON to preference `track_token_3` (see data doc).
3. `api.getCurrentUser()` → `GET https://kitsu.app/api/edge/users?filter[self]=true` through `authClient` (bearer injected). Returns `data[0].id` (string). If `data` is empty → `IndexOutOfBoundsException` (not caught by Kitsu code).
4. `saveCredentials(username, userId)` → `BaseTracker.saveCredentials` → `trackPreferences.setCredentials(this, username, userId)`, which sets username, password (= **userId**, not the user's password), and `pref_tracker_auth_expired_3 = false`.

OBSERVED consequence: the user's Kitsu password is never persisted. The "password" slot holds the Kitsu numeric user id and is later read as the user id (`getUserId() = getPassword()`, K:238-240). Proposal: store the user id in its own field (T-30a-007).

Caller behaviour (SettingsTrackingScreen.kt:250-364, outside scope):
- `TrackingLoginDialog` with title `MR.strings.login_title` ("Log in to %1$s"), username field labelled `MR.strings.email` ("Email address") for Kitsu, password field labelled `MR.strings.password` ("Password").
- Confirm button: label `MR.strings.login` ("Login") while idle, `MR.strings.loading` ("Loading…") while `processing`. Enabled only when `!processing && username.isNotBlank() && password.isNotBlank()`.
- Click runs on IO: `processing = true`, then `checkLogin`. On success: toast `MR.strings.login_success` ("Logged in") and close. On failure: `tracker.logout()` (clears everything, see 4.5), toast `e.message.toString()` (for HTTP errors this is `"HTTP error <code>"`), and the dialog stays open with `inputError = true` (both fields red).
- The username field's `ImeAction.Next`, password field's `ImeAction.Done`. Password field has a visibility toggle (initially hidden: `PasswordVisualTransformation`).

### 4.2 Token persistence (K:242-252)
- `saveToken(oauth: KitsuOAuth?)`: `trackPreferences.trackToken(this).set(json.encodeToString(oauth))`. Passing `null` stores the four-character string `null`.
- `restoreToken()`: `try { json.decodeFromString<KitsuOAuth>(pref.get()) } catch (_: Exception) { null }`. The default pref value is `""`, which fails to decode and returns `null`. The string `"null"` fails the non-nullable decode and also returns `null`.
- `restoreToken()` runs once, when `KitsuInterceptor` is constructed (`private var oauth: KitsuOAuth? = kitsu.restoreToken()`, I:18). The interceptor is `by lazy` in `Kitsu` (K:53) and the API is `by lazy` (K:55), so the first access creates it.

### 4.3 Refresh-before-expiry (I:20-35; DTO KitsuOAuth.kt:20)
`KitsuOAuth.isExpired() = (System.currentTimeMillis() / 1000) > (createdAt + expiresIn - 3600)`.
Values are seconds. The token counts as expired one hour before its real expiry.

`intercept(chain)` in order:
1. `originalRequest = chain.request()`.
2. `currAuth = oauth ?: throw Exception("Not authenticated with Kitsu")`.
3. `refreshToken = currAuth.refreshToken!!` (Kotlin non-null assertion; throws NullPointerException if null, **before** the expiry check).
4. If `currAuth.isExpired()`:
   a. `response = chain.proceed(KitsuApi.refreshTokenRequest(refreshToken))`.
   b. If `response.isSuccessful`: `newAuth(json.decodeFromString(response.body.string()))` (a `KitsuOAuth`; `newAuth` saves it).
   c. Else: `response.close()`. The old, expired token stays in use.
5. `authRequest = originalRequest.newBuilder()` with: `Authorization: Bearer <oauth!!.accessToken>` (added, via `addHeader`), `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)` (replaced), `Accept: application/vnd.api+json` (replaced), `Content-Type: application/vnd.api+json` (replaced).
6. `chain.proceed(authRequest)`.

`refreshTokenRequest(token)` (A:462-470): `POST https://kitsu.app/api/oauth/token`, form body in order `grant_type=refresh_token`, `refresh_token=<token>`, `client_id`, `client_secret`.

Observed details:
- On a successful refresh, the new token replaces the old one entirely. If the refresh response has no `refresh_token`, the stored token has `refreshToken = null`, and the next expiry will throw the NullPointerException of step 3.
- The refresh request is issued with `chain.proceed`, so it runs only the interceptors after `KitsuInterceptor` in `authClient`. It does not pass through the base client's `UncaughtExceptionInterceptor` or `UserAgentInterceptor` (these come earlier in the chain). So the refresh request carries OkHttp's default User-Agent and no `Accept` header. Observed, low severity.
- Nothing serialises refreshes. Two concurrent requests that both see an expired token will both POST the refresh. Kitsu's server-side behaviour under a reused refresh token is not visible in this source (see T-30a-027).

### 4.4 Partial login failure (K:226-231)
If step 3 or 4 of 4.1 fails after step 2 has run, the token is persisted but the credentials are not. `isLoggedIn` is false, and the token survives. `checkLogin` (SettingsTrackingScreen.kt:349-364) then calls `tracker.logout()`, which clears both. Other callers do not exist. Proposal: run login as a transaction (store token, fetch user, then write credentials; roll the token back on any failure). T-30a-007.

### 4.5 Logout (K:233-236; BaseTracker.kt:27-30)
`logout()`:
1. `super.logout()` → `trackPreferences.setCredentials(this, "", "")`, which sets username `""`, password `""` and `pref_tracker_auth_expired_3 = false`.
2. `interceptor.newAuth(null)` → in-memory token = null, and `saveToken(null)` stores the string `"null"`.

The logout dialog (SettingsTrackingScreen.kt:367-420) shows title `MR.strings.logout_title` ("Log out from %1$s?"), a `MR.strings.action_cancel` ("Cancel") outlined button and a `MR.strings.logout` ("Log out") error-coloured button. On confirm: `tracker.logout()`, close the dialog, toast `MR.strings.logout_success` ("You are now logged out"). No network call is made on logout; the Kitsu token is not revoked server-side.

### 4.6 Auth-expired flag
`pref_tracker_auth_expired_3` is set false by `setCredentials` (login and logout). Kitsu never sets it to true. Nothing in scope reads it. (It is read elsewhere, outside scope.)

## 5. Request pipeline

### 5.1 Two clients (A:43, A:259, A:290, A:313-425)
- `authClient = client.newBuilder().addInterceptor(interceptor).build()`. Used for: POST/PATCH/DELETE library entries, GET library entries, GET algolia key, GET users, and the refresh request.
- `client` (base) is used for: login POST (`LOGIN_URL`), and Algolia POST (`ALGOLIA_URL`).

Consequence (observed): `search()` and `searchAnime()` first call `authClient.newCall(GET(ALGOLIA_KEY_URL))`. With no token, `intercept` throws `Exception("Not authenticated with Kitsu")` before any network call. Kitsu search therefore requires a logged-in session. The Algolia POST itself carries no bearer token.

### 5.2 Base client properties (NetworkHelper.kt:20-35)
- `cookieJar = AndroidCookieJar()`; connect timeout 30 s; read timeout 30 s; `callTimeout` 2 minutes; OkHttp disk cache at `context.cacheDir/network_cache`, max 5 MiB.
- Application interceptors in order: `UncaughtExceptionInterceptor`, `UserAgentInterceptor(::defaultUserAgentProvider)`, then (on `authClient` only) `KitsuInterceptor`.
- DNS-over-HTTPS is optional (`NetworkPreferences.dohProvider`); not used by Kitsu code.

### 5.3 Cache caveat (GET helpers, Requests.kt:20-41)
Every `GET(url)` helper sets `CacheControl.Builder().maxAge(10, MINUTES)` by default (`DEFAULT_CACHE_CONTROL`). Kitsu's `GET` calls (`GET(ALGOLIA_KEY_URL)`, `GET(url)` for library entries and users) use this default. The OkHttp cache may return a response up to the server's freshness rules. Observed risk: a library-entry GET made right after a PATCH could be served from cache if Kitsu marks the response cacheable. Not verified against the live API. Proposal: send `Cache-Control: no-cache` on every authenticated request (T-30a-025).

### 5.4 Error wrapping (OkHttpExtensions.kt:109-117, HttpException.kt:13)
`awaitSuccess()` throws `HttpException(code)` when `!response.isSuccessful`. Its message is `"HTTP error <code>"`, e.g. `"HTTP error 404"`. Cancellation and transport failures surface as `IOException` with the original message. `parseAs<T>()` uses the app `Json` instance (`ignoreUnknownKeys = true`, `explicitNulls = false`; AppModule.kt:174-177).

Note on `explicitNulls = false`: when a `null` field is serialised, its key is omitted. This does not apply to the request bodies here, which are built manually with `buildJsonObject`, where `put(key, null as String?)` writes a JSON `null`. Kotlin's `JsonObjectBuilder.put(String, String?)` writes `JsonNull`. Observed: `ratingTwenty`, `startedAt`, `finishedAt` are sent as JSON `null` when empty.

## 6. Library entry operations

All endpoint details, bodies and response shapes are in `p1-data-and-config.md`. This section gives the behaviour.

### 6.1 Add (A:45-88 manga, A:90-133 anime)
- Body: `attributes.status = toApiStatus()`, `attributes.progress = <progress>.toInt()` (manga `last_chapter_read`, anime `last_episode_seen`), `attributes.private = track.private`; relationships `user` (id = userId string, type `users`) and `media` (id = `track.remote_id`, number, type `manga` or `anime`).
- The request is `POST library-entries` with `Content-Type: application/vnd.api+json` and body `application/vnd.api+json`.
- Response: `KitsuAddEntryResult { data: { id: Long } }`. On success `track.remote_id = data.id` (the new library-entry id). The track is returned.
- Dates, rating and the `startedAt`/`finishedAt` fields are NOT sent on add.
- `track.remote_id` before the call must be the media id (from search). Pre-condition is enforced only by call order (`bind` in section 7).

### 6.2 Update (A:135-165 manga, A:167-197 anime)
- `PATCH library-entries/{track.remote_id}`. `remote_id` must be the library-entry id.
- Body attributes: `status` (toApiStatus), `progress` (`last_chapter_read.toInt()` / `last_episode_seen.toInt()`), `ratingTwenty` (toApiScore, string or null), `startedAt` (KitsuDateHelper.convert of started date), `finishedAt` (same for finished), `private`.
- The body also carries `data.id = track.remote_id` (a JSON number) and `data.type = "libraryEntries"`.
- Response body is ignored. `awaitSuccess()` is called and its return value dropped (the response body is never read or closed explicitly). The function returns the input track unchanged.
- Dates: `KitsuDateHelper.convert(0)` returns null, so a zero date is sent as JSON `null`. This clears the remote date. Observed.
- Failure: non-2xx throws `HttpException`. `updateRemote` (outside scope) catches it, logs, and toasts `e.message`. The local DB row is not written in that path (see 8.3).

### 6.3 Clearing semantics
- `score == 0` → `ratingTwenty: null` → clears remote rating.
- start/finish date `0` → `startedAt`/`finishedAt: null` → clears remote date.
- These follow from every PATCH sending all fields. A PATCH from a track whose dates are 0 erases remote dates. Observed and relied upon by `bind` (section 7).

### 6.4 Remove (A:199-221)
- `DELETE library-entries/{track.remoteId}` with `Content-Type: application/vnd.api+json` and an empty body (`DEFAULT_BODY` = empty `FormBody`). The interceptor overwrites the `Content-Type` header anyway.
- `track.remoteId` on the domain track is the library-entry id (set by bind/add). Calling delete on an unbound track sends `DELETE library-entries/0`.
- Response body ignored.

### 6.5 Find by media id (A:311-330 manga, A:332-351 anime)
- `GET library-entries?filter[manga_id]=<track.remote_id>&filter[user_id]=<userId>&include=manga` (anime: `filter[anime_id]`, `include=anime`). `track.remote_id` = media id.
- Returns `null` when `data` is empty OR `included` is empty. Otherwise `firstToMangaTrack()` / `firstToAnimeTrack()` (section 9).
- Query string is built with `encodedQuery("filter[manga_id]=<id>&filter[user_id]=<userId>")` and then `appendQueryParameter("include", "manga")`. Brackets are literal.

### 6.6 Get by entry id (A:353-372 manga, A:374-393 anime)
- `GET library-entries?filter[id]=<track.remote_id>&include=manga` (anime: `include=anime`). `track.remote_id` = library-entry id.
- If `data` or `included` is empty: `throw Exception("Could not find manga")` for manga and `throw Exception("Could not find manga")` for anime. **Quirk:** the anime path has the same copy-pasted English message "Could not find manga" (A:388).

### 6.7 Current user (A:412-425)
- `GET users?filter[self]=true`, returns `data[0].id`. Empty `data` → exception (not caught).

## 7. bind (register a track) (K:168-202 manga, K:186-202 anime)

Called by `AddMangaTracks.bind` / `AddAnimeTracks.bind` (outside scope), which first computes `hasReadChapters` = any chapter `read` (`AddMangaTracks.kt:35`), or `hasWatchedEpisodes` = any episode `seen` (`AddAnimeTracks.kt:35`). Argument `track` is the item created from a search hit, so `track.remote_id` is the **media id**.

Algorithm (manga; anime is identical with the anime names and statuses):
```
remote = api.findLibManga(track, userId)        // section 6.5, GET filter[manga_id]=track.remote_id
if remote != null:
    track.copyPersonalFrom(remote, copyRemotePrivate = false)
        // copies: last_chapter_read, score, status, started_reading_date, finished_reading_date
        // does NOT copy: private
    track.remote_id = remote.remote_id              // library-entry id
    if track.status != COMPLETED:
        track.status = hasReadChapters ? READING : track.status
    return update(track)                            // update(track, didReadChapter = false) => PATCH only
else:
    track.status = hasReadChapters ? READING : PLAN_TO_READ
    track.score = 0.0
    return add(track)                               // POST; sets track.remote_id = new entry id
```
Anime: `hasWatchedEpisodes ? WATCHING : PLAN_TO_WATCH`; `copyPersonalFrom` copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`.

OBSERVED details:
- In the found branch, `track.tracking_url` is not changed. It keeps the URL from the search hit (media id).
- `remote.private` is NOT copied. The local `private` flag (what the user chose in the search sheet) is preserved and sent in the PATCH.
- `hasReadChapters == false` and remote status `READING` → status stays `READING` (copied). `hasReadChapters == false` and remote status `DROPPED` → stays `DROPPED`.
- `hasReadChapters == true` and status `DROPPED`/`ON_HOLD`/`PLAN_TO_READ` (not completed) → forced to `READING`.
- Remote `COMPLETED` is never overridden by bind.
- In the add branch, `track.private` as chosen locally is sent. `track.score = 0.0` and dates are not sent.
- Return value: the mutated track. The caller then does `insertTrack.await(track)` (outside scope), and for `hasRead`: `setRemoteLastChapterRead` (which calls `update` again), and `setRemoteStartDate` if `startDate <= 0` (outside scope).

Caller sequence after bind (AddMangaTracks.kt:30-79, AddAnimeTracks.kt:32-79; outside scope, recorded so the Kitsu call count is known):
1. `tracker.bind(item, hasRead)`.
2. `insertTrack.await(track)`.
3. If `hasRead`: `latest = max chapter number among read chapters`, taking the chapters sorted by number while read (`takeWhile { it.read }.lastOrNull()`), default `-1.0`. If `latest > track.lastChapterRead`: `setRemoteLastChapterRead(track, latest.toInt())` (note `toInt()` truncation). If `track.startDate <= 0` and a first history `readAt` exists: `setRemoteStartDate(track, startDate)` where startDate = `readAt` converted from system zone to UTC.
4. `syncChapterProgressWithTrack.await(mangaId, track, tracker)`. For Kitsu this returns immediately because Kitsu is not an `EnhancedMangaTracker` / `EnhancedAnimeTracker` (`SyncChapterProgressWithTrack.kt:26-28`).

Errors: exceptions from `bind` propagate to `register` (`MangaTracker.register`, outside scope), which toasts `e.message` and does not insert the track row.

## 8. update (on read/watch events) (K:124-140 manga, K:142-158 anime)

```
update(track, didReadChapter):
    if track.status != COMPLETED:
        if didReadChapter:
            if track.last_chapter_read.toLong() == track.total_chapters && track.total_chapters > 0:
                track.status = COMPLETED
                track.finished_reading_date = now_ms()
            else:
                track.status = READING
                if track.last_chapter_read == 1.0:
                    track.started_reading_date = now_ms()
    return api.updateLibManga(track)                 // PATCH, section 6.2
```
Anime: `last_episode_seen`, `total_episodes`, `WATCHING`, `finished_watching_date`, `started_watching_date`.

Observed:
- `didReadChapter == false` (the default, used by `setRemote*` helpers and `bind`): the status is untouched; only PATCH is sent.
- `didReadChapter == true` is passed only by `TrackChapter` (`TrackChapter.kt:43`: `service.mangaService.update(updatedTrack.toDbTrack(), true)`), and by no other scope path.
- `total_chapters == 0` (unknown total) never completes the entry automatically.
- `last_chapter_read.toLong()` truncates. `12.5` → 12 compared with total 12 → completes.
- A status of `DROPPED` or `ON_HOLD` becomes `READING` on a read event. Only `COMPLETED` is protected.
- Re-reading chapter 1 (`last_chapter_read == 1.0`) resets `started_reading_date`. A later read does not clear `finished_reading_date`.
- The `now` values are `System.currentTimeMillis()` (epoch millis, UTC-based).

### 8.1 Setter helpers that call update (outside scope, MangaTracker.kt:57-121, AnimeTracker.kt:58-120)
- `setRemoteMangaStatus(track, status)`: sets status; if `status == COMPLETED && total_chapters != 0`, sets `last_chapter_read = total_chapters`; then `updateRemote`.
- `setRemoteLastChapterRead(track, n)`: if `last_chapter_read == 0.0 && 0.0 < n && status != rereadingStatus(-1)` → `status = READING`; `last_chapter_read = n`; if `total != 0 && last == total` → `status = COMPLETED`, `finished = now`; then `updateRemote`.
- `setRemoteScore(track, s)`: `score = indexToScore(indexOf(s))`; `updateRemote`.
- `setRemoteStartDate` / `setRemoteFinishDate` / `setRemotePrivate`: set one field, `updateRemote`.
- `updateRemote(track)`: `update(track)` (didRead = false), then `insertTrack.await(track.toDomainTrack(idRequired = false))`. On exception: `logcat(ERROR)` and toast `e.message`; the DB row is not written.

### 8.2 Read-event sequence (`TrackChapter.await`, outside scope, TrackChapter.kt:24-60)
Per track whose service is logged in and where `chapterNumber > track.lastChapterRead`:
1. `refresh(track)` (section 9).
2. `copy(lastChapterRead = chapterNumber)`.
3. `update(updatedTrack, true)`.
4. `insertTrack.await(updatedTrack)`, `delayedTrackingStore.removeMangaItem(track.id)`.
On failure: `delayedTrackingStore.addManga(track.id, chapterNumber)`, `DelayedMangaTrackingUpdateJob.setupTask(context)` if `setupJobOnFailure`, rethrow. Each track is processed in its own `async` and `awaitAll`; errors are logged at INFO.

### 8.3 Error behaviour
- A failed PATCH leaves the local DB row unchanged on the update path, and the caller stores the chapter in the delayed-update store (TrackChapter path) so a background job retries it (area:27-tracking-core).
- The Kitsu code itself does not retry.

## 9. refresh (K:212-224 manga, K:219-224 anime)

```
refresh(track):
    remote = api.getLibManga(track)                  // GET filter[id]=track.remote_id&include=manga
    track.copyPersonalFrom(remote)                   // copyRemotePrivate = true (default)
    track.total_chapters = remote.total_chapters
    return track
```
- Copies `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date`, `private`.
- `total_chapters` = remote `chapterCount ?: 0`. Ongoing titles with a null count reset the local total to 0.
- Does NOT copy `title`, `tracking_url`, `remote_id`, `library_id`. The DB write is the caller's job.
- Anime: `total_episodes = remote.total_episodes` (`episodeCount ?: 0`); `copyPersonalFrom` for anime copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, `private`.
- Failure: "Could not find manga" exception (section 6.6) when the entry does not exist remotely.

## 10. delete (remote removal) (K:160-166)
- `delete(track: DomainTrack)` → `api.removeLibManga(track)` / `removeLibAnime(track)` → `DELETE library-entries/{track.remoteId}`.
- UI (AnimeTrackInfoDialog.kt:811-840; MangaTrackInfoDialog.kt same): the "Remove %s tracking?" dialog (`MR.strings.track_delete_title`) shows text `MR.strings.track_delete_text` ("This will remove the tracking locally.") and, when `tracker is DeletableXTracker`, a checkbox `MR.strings.track_delete_remote_text` ("Also remove from %s"). Buttons: `MR.strings.action_cancel` ("Cancel") and `MR.strings.action_ok` ("OK").
- On OK: `unregisterTracking(serviceId)` (deletes the local row, launched non-cancellable), then, if the checkbox was ticked, `deleteAnimeFromService()` (launched non-cancellable, catches `Exception`, logs at ERROR, no toast). Order: local first, remote second, and a remote failure does not restore the local row.

## 11. Search (A:223-247 manga & anime, A:249-309)

### 11.1 Algorithm
```
search(query):   // manga; anime is searchAnime
    key = GET https://kitsu.app/api/edge/algolia-keys/media/   (authClient; requires login)
          -> KitsuSearchResult.media.key
    body = {"params": "query=" + URLEncoder.encode(query, UTF-8) + ALGOLIA_FILTER}
    hits = POST https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/
           headers: X-Algolia-Application-Id: AWQO5J657S, X-Algolia-API-Key: key
           body: JSON, media type application/json; charset=utf-8  (jsonMime)
          -> KitsuAlgoliaSearchResult.hits
    return hits.filter { subtype != "novel" }.map { toMangaTrack() | toAnimeTrack() }
```
- `URLEncoder.encode(query, UTF-8)`: space → `+`, `*-._` and alphanumerics unchanged, all others `%XX` uppercase.
- The `params` string is itself a URL query string. Only the `query` value is encoded here; `ALGOLIA_FILTER*` constants are already percent-encoded and must be sent verbatim.
- `query` is not trimmed, and an empty query is sent as `query=` (not guarded).
- The `novel` filter is case-sensitive; a missing `subtype` (null) passes the filter.
- No paging: Algolia's default page size applies (not set in the request; not confirmed here). Only the first page is returned.
- No debounce or cancellation inside Kitsu. The caller (track search UI, outside scope) is responsible.

### 11.2 Exact constants (A:439-449)
`ALGOLIA_FILTER` (manga) = `&facetFilters=%5B%22kind%3Amanga%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22chapterCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D`

Decoded for reading only: `&facetFilters=["kind:manga"]&attributesToRetrieve=["synopsis","averageRating","canonicalTitle","chapterCount","posterImage","startDate","subtype","endDate", "id"]` (note the space before `"id"`, from `%2C%20`).

`ALGOLIA_FILTER_ANIME` = `&facetFilters=%5B%22kind%3Aanime%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22episodeCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D`

Decoded: `&facetFilters=["kind:anime"]&attributesToRetrieve=["synopsis","averageRating","canonicalTitle","episodeCount","posterImage","startDate","subtype","endDate", "id"]`.

Note: the manga filter asks for `chapterCount`; the anime filter asks for `episodeCount`. Neither asks for `status`, so Algolia hits do not carry a Kitsu status.

### 11.3 Key handling
- No caching. Each search performs one key GET first.
- An Algolia 403 (rotated key) surfaces as `HttpException(403)`; there is no refetch.

## 12. Search hit mapping (DTO:40-74)

For each hit `h` (manga `toMangaTrack`; anime `toAnimeTrack`):

| Track-search field | Manga source | Anime source |
|---|---|---|
| `tracker_id` | `TrackerManager.KITSU` (3) | same |
| `remote_id` | `h.id` (media id) | same |
| `title` | `canonicalTitle` | same |
| `total_chapters` / `total_episodes` | `chapterCount ?: 0` | `episodeCount ?: 0` |
| `cover_url` | `posterImage?.original ?: ""` | same |
| `summary` | `synopsis ?: ""` | same |
| `tracking_url` | `https://kitsu.app/manga/<remote_id>` | `https://kitsu.app/anime/<remote_id>` |
| `score` | `averageRating ?: -1.0` | same |
| `publishing_status` | `if (endDate == null) "Publishing" else "Finished"` | same |
| `publishing_type` | `subtype ?: ""` | same |
| `start_date` | `startDate?.let { SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date(it * 1000)) } ?: ""` | same |

- `startDate` is epoch **seconds** in Algolia; multiplied by 1000 for `Date`.
- The date formatter uses the JVM default time zone (not UTC). A date at UTC midnight shows the previous day in a negative-offset zone. Observed.
- `publishing_status` strings are English literals, not localised.
- Search items are not persisted. The track search screen (outside scope) turns the chosen item into a track via `toDomainTrack`-style mapping.

## 13. List-entry conversion (DTO:16-48 manga, DTO:50-82 anime)

`firstToMangaTrack()`:
- `require(data.isNotEmpty()) { "Missing User data from Kitsu" }`, `require(included.isNotEmpty()) { "Missing Manga data from Kitsu" }` (IllegalArgumentException; messages as shown, English, not localised).
- `remote_id = data[0].id` (library-entry id).
- `title = included[0].canonicalTitle`; `total_chapters = chapterCount ?: 0`; `cover_url`, `summary`.
- `tracking_url = mangaUrl(remote_id)`: **uses the library-entry id, not the media id.** Latent bug: wrong URL. Not persisted by any caller (bind uses the original search track's URL; refresh does not copy URL). See T-30a-029.
- `publishing_status = included[0].status` (raw Kitsu status string, not the English "Publishing"/"Finished" used by search). Not persisted by bind/refresh; present on the transient track.
- `publishing_type = mangaType ?: ""`.
- `start_date = attributes.startedAt ?: ""` (raw string, not parsed).
- `started_reading_date = KitsuDateHelper.parse(startedAt)`; `finished_reading_date = KitsuDateHelper.parse(finishedAt)`.
- `status` mapping per section 2.4 (throws for unknown).
- `score = ratingTwenty?.let { it / 2.0 } ?: 0.0`.
- `last_chapter_read = progress.toDouble()`.
- `private = attributes.private`.

`firstToAnimeTrack()` is the same with: `title = included[0].canonicalTitle`, `total_episodes = episodeCount ?: 0`, `publishing_type = showType ?: ""`, `started_watching_date`, `finished_watching_date`, status statuses per 2.4 anime, `last_episode_seen = progress.toDouble()`, `tracking_url = animeUrl(remote_id)` (same library-id quirk).

## 14. Dates (D:1-25)

`convert(dateValue: Long): String?`:
- `0L` → `null`.
- Otherwise `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.ENGLISH).format(Date(dateValue))`.

`parse(dateString: String?): Long`:
- `null` → `0L`.
- Otherwise `formatter.parse(dateString)?.time ?: 0`. A non-matching string throws `ParseException`. An empty string throws.

**Defect (observed):** the pattern has no time zone field. The `'Z'` is a quoted literal, and the formatter has the JVM default time zone. So `convert` writes the device's **local wall-clock time** with a literal `Z`. Example: epoch for 2024-05-01 00:00 local at UTC+02:00 is written as `2024-05-01T00:00:00.000Z`, which Kitsu reads as 00:00 UTC (2 hours off). `parse` reads the same literal back in local time, so a local round trip is consistent but the server value is offset by the device's UTC offset.

Proposal (behaviour change, flagged): convert and parse in UTC. Wire format `%Y-%m-%dT%H:%M:%S%.3fZ`. Parse of remote values as UTC. Decision in T-30a-016.

Thread-safety (observed): `formatter` is a shared, non-thread-safe `SimpleDateFormat` singleton. Concurrent `convert`/`parse` from two coroutines can corrupt output. Rust: stateless formatting (no shared formatter).

## 15. Concurrency and dispatch

- Every API method wraps its body in `withIOContext { }` (A:45 addLibManga, A:90 addLibAnime, A:135 updateLibManga, A:167 updateLibAnime, A:199 removeLibManga, A:211 removeLibAnime, A:223 search, A:236 searchAnime, A:249 algoliaSearch, A:280 algoliaSearchAnime, A:311 findLibManga, A:332 findLibAnime, A:353 getLibManga, A:374 getLibAnime, A:395 login, A:412 getCurrentUser), which dispatches to `Dispatchers.IO`. Kotlin coroutines; no explicit timeout on the Kitsu call beyond the client's 2-minute `callTimeout` (section 5.2).
- Interceptor `oauth` is a plain `var` with no synchronisation. Read and written from multiple coroutines.
- `bind` / `update` / `refresh` are called from `withNonCancellableContext { withIOContext {} }` in the add-track interactors (AddMangaTracks.kt:30-33), so a UI cancel does not cancel a bind mid-flight.
- `setRemote*` helpers catch `Exception` and toast. Errors from `update` in the delayed path are re-thrown (section 8.2).

Proposal (T-30a-026, T-30a-027): one tokio runtime, all Kitsu I/O on it; token state behind a `tokio::sync::Mutex`; GTK main thread only receives results via a channel.

## 16. Timings and limits (complete list)

| Item | Value | Source |
|---|---|---|
| Token refresh margin | 3600 s before `created_at + expires_in` | I:28 / DTO:20 |
| Connect timeout | 30 s | NetworkHelper.kt:23 |
| Read timeout | 30 s | NetworkHelper.kt:24 |
| Call timeout | 2 min | NetworkHelper.kt:25 |
| Default GET cache max-age | 10 min | Requests.kt:12 |
| HTTP disk cache | 5 MiB, `network_cache` | NetworkHelper.kt:29-31 |
| Algolia hits page | Algolia default (not set in request) | A:262-272 |
| Retries | none in Kitsu code | — |
| Rate limiting | none in Kitsu code | — |
| Score steps | 0.5, range 0 or 1.0 to 10.0 | K:97-104 |
| Progress precision | `toInt()` (truncation) | A:52,97,143,175 |

## 17. Edge cases (behavioural, cross-referenced to p1-edge-cases-and-tests.md)

1. Search without a session throws `Not authenticated with Kitsu`.
2. Token expired and refresh fails (non-2xx) → request sent with the expired token → server returns 401 → `HTTP error 401`.
3. Token expired and no refresh token → NPE in `intercept` (before the expiry check, so also for a non-expired token with no refresh token).
4. Refresh response decodes but has no `refresh_token` → stored token lacks one → item 3 on next expiry.
5. Login fails after token save → token remains until `logout()`.
6. `data` empty in `users` → IndexOutOfBounds.
7. Remote status string not in the 5-value table → `Unknown status` exception from `firstToX` (bind and refresh fail).
8. Remote date not matching `yyyy-MM-dd'T'HH:mm:ss.SSS'Z'` (e.g. no milliseconds, or date-only) → `ParseException` (bind and refresh fail).
9. Remote `ratingTwenty` values 1..20 map to 0.5..10.0; 0 → 0.0.
10. `progress` is `Int` on the wire; fractional chapter numbers are truncated on write, and the remote value is read back as `Double`.
11. `total_chapters = 0` disables auto-completion.
12. PATCH to a removed remote entry → 404 `HTTP error 404`; local row unchanged.
13. Unbound track (`remote_id = 0`) → `PATCH library-entries/0` → 404.
14. Anime refresh error text says "Could not find manga".
15. `tracking_url` after find uses library id (latent; never persisted).
16. Score display depends on the device decimal separator.
17. Search `start_date` is shown in the local zone (may be off by one day).
18. Local `private` survives bind; remote `private` is never copied in bind but is copied in refresh.
19. Concurrent refresh race (section 4.3).
20. `updateLibManga` leaves the response unread.
21. `KitsuDateHelper` singleton formatter shared across threads.

## 18. Interaction map (what calls Kitsu, and what Kitsu calls)

Calls INTO Kitsu (outside scope, via the tracker interfaces):
- `bind`, `update(track, didRead)`, `refresh`, `delete`, `searchManga`/`searchAnime`, `login`, `logout`, `isLoggedIn`, `getScoreList`, `indexToScore`, `displayScore`, `getStatusListManga/Anime`, `getStatusForManga/Anime`, `getCompletionStatus`, `getReadingStatus`, `getWatchingStatus`, `getRereadingStatus`, `getRewatchingStatus`, `supportsReadingDates`, `supportsPrivateTracking`, `getLogo`, `getLogoColor`, `saveCredentials`, `getUsername`, `getPassword`, `isLoggedInFlow`.

Calls OUT of Kitsu:
- `TrackPreferences.trackToken(this)`, `trackUsername`, `trackPassword`, `setCredentials` (area:02-preferences-system, area:27-tracking-core).
- `NetworkHelper.client` (area:12-network-stack).
- `TrackerManager.KITSU` (area:27-tracking-core).
- `MangaTrackSearch.create` / `AnimeTrackSearch.create` (`tracker_id = serviceId`) (area:27-tracking-core).
- `copyPersonalFrom` (defined on the track interfaces, area:27-tracking-core).
- `json` from DI (`Json { ignoreUnknownKeys = true; explicitNulls = false }`, AppModule.kt:174-177).
- No notifications, no work scheduling, no file writes, and no DB writes made directly by Kitsu. DB writes happen in the caller (`insertTrack`, area:06-data-repos-history-track-source-misc) and the delayed-update job runs in area:26-library-update-jobs / area:27-tracking-core.

## 19. Rust behaviour summary (decisions)

- Same endpoints, same bodies, same status and score tables, same error texts where they reach the UI (T-30a-024).
- Deviations (each a flagged proposal): user id stored in its own field; partial login rolled back; refresh serialised; refresh needs a refresh token only when expired; refresh keeps the old refresh token when the new one lacks one; dates in UTC; fixed `.` decimal; anime refresh error text corrected; `tracking_url` built from media id; numeric/string id tolerance on decode; Algolia key cached per session with one refetch on 401/403.
- Every deviation has a task in `p1-tasks.md` and a row in `p1-edge-cases-and-tests.md`.
