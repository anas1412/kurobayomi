# Area 31-tracker-shikimori-bangumi, part 1: behaviour spec

Source root: `/home/blackbox/aniyomi-src`. All citations are repo-relative `path:line`. Remote-API facts marked **[verified]** were checked against the server-side source (Shikimori `shikimori/shikimori`, file cited in p1-topic-remote-api-verification.md) or the Bangumi OpenAPI spec (`bangumi/api`, `open-api/v0.yaml`). Facts marked **[observed]** come only from the Kotlin. **Proposal:** marks what Kurobayomi should do differently.

Contents: 1 shared framework contract; 2 Shikimori; 3 Bangumi; 4 side-by-side comparison; 5 user-visible messages; 6 call paths from other areas; 7 timings, limits, concurrency; 8 defects and proposals.

---

## 1. Shared framework contract (what the framework expects from both trackers)

Both trackers extend `BaseTracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt:11-52`) and implement `MangaTracker` and/or `AnimeTracker`. The framework calls these methods; the bodies are in this area.

| Framework method | Declared at | Shikimori | Bangumi |
|---|---|---|---|
| `getScoreList()` | `Tracker.kt:31` | `SCORE_LIST` (see 1.2) | `SCORE_LIST` |
| `indexToScore(index)` | `MangaTracker.kt:31-33` | `index.toDouble()` (`Shikimori.kt:57-59`) | `index.toDouble()` (`Bangumi.kt:36-38`) |
| `displayScore(track)` | `MangaTracker.kt:47` | `track.score.toInt().toString()` (`Shikimori.kt:61-67`) | same (`Bangumi.kt:40-46`) |
| `update(track, didRead)` | `MangaTracker.kt:49` | `Shikimori.kt:77-89` / `91-103` | `Bangumi.kt:56-68` / `70-82` |
| `bind(track, hasRead)` | `MangaTracker.kt:51` | `Shikimori.kt:113-131` / `133-151` | `Bangumi.kt:84-104` / `106-126` |
| `searchManga` / `searchAnime` | `MangaTracker.kt:53`, `AnimeTracker.kt:53` | `Shikimori.kt:153-159` | `Bangumi.kt:128-134` |
| `refresh(track)` | `MangaTracker.kt:55` | `Shikimori.kt:161-177` | `Bangumi.kt:136-146` |
| `delete(track)` | `DeletableMangaTracker.kt:10`, `DeletableAnimeTracker.kt:21` | `Shikimori.kt:105-111` | not implemented (Bangumi is not Deletable) |
| `supportsPrivateTracking` | `Tracker.kt:21` | not overridden: `false` (`BaseTracker.kt:25`) | `true` (`Bangumi.kt:32`) |
| `supportsReadingDates` | `Tracker.kt:19` | `false` (BaseTracker.kt:23, not overridden) | `false` |
| `getLogo()` / `getLogoColor()` | `Tracker.kt:24-27` | `R.drawable.ic_tracker_shikimori`, `Color.rgb(40, 40, 40)` (`Shikimori.kt:179-181`) | `R.drawable.ic_tracker_bangumi`, `Color.rgb(240, 145, 153)` (`Bangumi.kt:148-150`) |
| `login(username, password)` | `Tracker.kt:33` | `= login(password)` (`Shikimori.kt:221`) | `= login(password)` (`Bangumi.kt:188`) |
| `logout()` | `Tracker.kt:36` | `Shikimori.kt:246-250` | `Bangumi.kt:216-220` |

### 1.1 Status identifiers (internal, persisted)

The numeric status is persisted in `manga_sync.status` / `anime_sync.status` (see p1-data-and-config.md section 5). The same number means different things per tracker. Both sets must be preserved for backup compatibility (area:21-backup-format-and-create).

Shikimori (`Shikimori.kt:37-42`):

| Constant | Value |
|---|---|
| `READING` | 1 |
| `COMPLETED` | 2 |
| `ON_HOLD` | 3 |
| `DROPPED` | 4 |
| `PLAN_TO_READ` | 5 |
| `REREADING` | 6 |

Bangumi (`Bangumi.kt:223-227`):

| Constant | Value |
|---|---|
| `PLAN_TO_READ` | 1 |
| `COMPLETED` | 2 |
| `READING` | 3 |
| `ON_HOLD` | 4 |
| `DROPPED` | 5 |

Bangumi has no rereading status: `getRereadingStatus()` and `getRewatchingStatus()` return `-1` (`Bangumi.kt:182`, `184`). Shikimori returns `REREADING` (6) for both (`Shikimori.kt:215`, `217`).

### 1.2 Score list

`SCORE_LIST = IntRange(0, 10).map(Int::toString).toImmutableList()` in both trackers (`Shikimori.kt:44-46`, `Bangumi.kt:229-231`). That is 11 entries: `"0"`, `"1"`, ..., `"10"`. Index equals score. `"0"` means "no rating" in both services (Shikimori: blank becomes 0, see `UserRate.score_changed` in 3.2 below; Bangumi: `rate: 0` removes the rating, `bgm_v0.yaml` `UserSubjectCollectionModifyPayload.rate`).

---

## 2. Shikimori

### 2.1 Endpoints and base URLs (constants)

`ShikimoriApi.kt:251-259`:
- `BASE_URL = "https://shikimori.one"` (public `const`).
- `API_URL = "$BASE_URL/api"` (line 252).
- `OAUTH_URL = "$BASE_URL/oauth/token"` (line 253).
- `LOGIN_URL = "$BASE_URL/oauth/authorize"` (line 254).
- `REDIRECT_URL = "aniyomi://shikimori-auth"` (line 256).
- `CLIENT_ID = "aOAYRqOLwxpA8skpcQIXetNy4cw2rn2fRzScawlcQ5U"` (line 258).
- `CLIENT_SECRET = "jqjmORn6bh2046ulkm4lHEwJ3OA1RmO3FD2sR9f6Clw"` (line 259).

Proposal on the credentials: see section 8, D-OPEN-1 (ownership of the OAuth client). The Kurobayomi value comes from configuration, never from a copy of these constants.

### 2.2 Status mapping (`ShikimoriUtils.kt`)

Internal to API string (`toShikimoriStatus`, used for both manga and anime, lines 6-24):

| Internal | API string |
|---|---|
| `READING` (1) | `"watching"` |
| `COMPLETED` (2) | `"completed"` |
| `ON_HOLD` (3) | `"on_hold"` |
| `DROPPED` (4) | `"dropped"` |
| `PLAN_TO_READ` (5) | `"planned"` |
| `REREADING` (6) | `"rewatching"` |
| anything else | throws `NotImplementedError("Unknown status: $status")` |

Note that manga "reading" is sent as `"watching"`. The Shikimori API uses one status vocabulary for both kinds (`UserRate` enum, see 2.11).

API string to internal (`toTrackStatus`, lines 26-34): the inverse table, with the same six strings. Any other string throws `NotImplementedError("Unknown status: $status")`.

Server-side enum for reference **[verified]**: `UserRate.statuses` = `planned: 0, watching: 1, rewatching: 9, completed: 2, on_hold: 3, dropped: 4` (`app/models/user_rate.rb` lines 8-15 in shikimori/shikimori).

### 2.3 Score

- `displayScore` = `score.toInt().toString()` (`Shikimori.kt:61-67`).
- Writes send `put("score", track.score.toInt())` (`ShikimoriApi.kt:51`, `92`). No clamping is applied on the client. Search results carry `score` = the catalogue average (`SMEntry.score`, a Double), not a user score.
- Bind for a new remote entry sets `track.score = 0.0` (`Shikimori.kt:128`, `148`).
- `UserRate.score_changed` **[verified]** (`app/models/user_rate.rb:162-166`): blank score becomes 0; a score greater than `MAXIMUM_SCORE` (10) or negative is replaced by the previous value (`changes['score'].first`). The server does not return 422 for -1; it silently keeps the old value. See D8 in section 8.

### 2.4 Login by OAuth code (the only login path)

Entry point: `TrackLoginActivity` routes `shikimori-auth` (area:27-tracking-core). The code path in this area:

1. Authorise URL (`ShikimoriApi.authUrl()`, `ShikimoriApi.kt:261-265`): `https://shikimori.one/oauth/authorize?client_id=<CLIENT_ID>&redirect_uri=aniyomi%3A%2F%2Fshikimori-auth&response_type=code` (parameters appended in this order: `client_id`, `redirect_uri`, `response_type`). The user's browser opens it through `openInBrowser(..., forceDefaultBrowser = true)` (`SettingsTrackingScreen.kt:193-198`). No `scope` parameter is sent; the server then applies `default_scopes :user_rates, :comments, :topics` **[verified]** (`config/initializers/doorkeeper.rb:191`).
2. The provider redirects to `aniyomi://shikimori-auth?code=<code>`. Area:27 extracts `code` with `data.getQueryParameter("code")`. If `code` is null, area:27-tracking-core calls `logout()` and does not log in (`TrackLoginActivity.kt:59-69`).
3. `login(code)` (`Shikimori.kt:223-232`):
   a. `api.accessToken(code)` (step 3a, below).
   b. `interceptor.newAuth(oauth)` (sets the in-memory token and persists it, see 2.5).
   c. `api.getCurrentUser()` returns `SMUser.id` (an `Int`) from `GET /api/users/whoami`.
   d. `saveCredentials(user.toString(), oauth.accessToken)`: username is the numeric id as a decimal string, password is the access token.
   e. Any `Throwable` in a-d goes to `catch (e: Throwable) { logout() }`. No message is shown to the user; the function returns normally and area:27-tracking-core returns to settings.
3a. `accessToken(code)` (`ShikimoriApi.kt:229-248`): `POST https://shikimori.one/oauth/token`, form body in this order: `grant_type=authorization_code`, `client_id`, `client_secret`, `code`, `redirect_uri=aniyomi://shikimori-auth`. Runs on the base `client` (not `authClient`), so no bearer header is attached. Response body parsed as `SMOAuth` with the app `Json` (`ignoreUnknownKeys = true`, `explicitNulls = false`, `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:173-177`). A non-2xx status throws `HttpException(code)`, which is caught in step 3e.

Observed: `saveCredentials` stores the access token in the plain password preference (area:27-tracking-core `TrackPreferences.trackPassword`). Proposal: store it in the secret store (p1-rust-design.md section 5).

### 2.5 Token persistence

- `saveToken(oauth: SMOAuth?)` writes `json.encodeToString(oauth)` to the track-token preference (`Shikimori.kt:234-236`). The key is `track_token_4`, created by `TrackPreferences.trackToken(this)` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:35`).
- `restoreToken()` decodes that string to `SMOAuth`; any exception returns `null` (`Shikimori.kt:238-244`).
- Shape on disk: `{"access_token":"...","token_type":"Bearer","created_at":<epoch seconds>,"expires_in":<seconds>,"refresh_token":"..."}`. `refresh_token` is nullable (`SMOAuth.kt:17`).
- The interceptor restores once, lazily, on first use: `private var oauth: SMOAuth? = shikimori.restoreToken()` (`ShikimoriInterceptor.kt:18`).

### 2.6 Request interceptor (`ShikimoriInterceptor.intercept`, lines 20-43)

Applied to every request made through `authClient` (`ShikimoriApi.kt:40`: `client.newBuilder().addInterceptor(interceptor).build()`). Every call in 2.7 to 2.13 except `accessToken` and `refreshTokenRequest` goes through it.

Algorithm, exact order:
1. `originalRequest = chain.request()`.
2. `currAuth = oauth ?: throw Exception("Not authenticated with Shikimori")` (line 23). This is the error for any authenticated call while logged out.
3. `refreshToken = currAuth.refreshToken!!` (line 25). **Evaluated for every request, not only when expired.** A null refresh token throws `NullPointerException` on every request (defect D3).
4. If `currAuth.isExpired()` (lines 28-35):
   a. `response = chain.proceed(ShikimoriApi.refreshTokenRequest(refreshToken))`.
   b. If `response.isSuccessful`: `newAuth(json.decodeFromString<SMOAuth>(response.body.string()))` (replaces in-memory token and persists it).
   c. Else: `response.close()` and continue with the old token. No exception is thrown here (defect D1).
5. Build the request: `originalRequest.newBuilder().addHeader("Authorization", "Bearer ${oauth!!.accessToken}").header("User-Agent", "Aniyomi v${BuildConfig.VERSION_NAME} (${BuildConfig.APPLICATION_ID})")` (lines 37-40). Uses the field `oauth`, not `currAuth`, so after a successful refresh the new token is used.
6. `chain.proceed(authRequest)`.

`isExpired()` (`SMOAuth.kt:21`): `(System.currentTimeMillis() / 1000) > (createdAt + expiresIn - 3600)`, with the comment "Access token lives 1 day". So refresh starts when less than 1 hour remains. Server-side **[verified]**: `access_token_expires_in 1.day` (`config/initializers/doorkeeper.rb:70`, so 86400 seconds), `use_refresh_token` (line 175).

`refreshTokenRequest(token)` (`ShikimoriApi.kt:267-275`): `POST https://shikimori.one/oauth/token`, form body: `grant_type=refresh_token`, `client_id`, `client_secret`, `refresh_token=<token>`. No `redirect_uri` (unlike Bangumi). Because it runs via `chain.proceed` inside this interceptor, it bypasses the outer application interceptors of the base client (see D16).

`newAuth(oauth)` (lines 45-48): sets `this.oauth` and calls `shikimori.saveToken(oauth)`.

Thread safety: `oauth` is a plain `var`, with no lock (defect D2).

### 2.7 Logout (`Shikimori.kt:246-250`)

Order is fixed:
1. `super.logout()` → `trackPreferences.setCredentials(this, "", "")`, which also sets `pref_tracker_auth_expired_4` to `false` (`TrackPreferences.kt:29-33`).
2. `trackPreferences.trackToken(this).delete()` removes `track_token_4`.
3. `interceptor.newAuth(null)` → `this.oauth = null`, then `saveToken(null)`, which writes the four-character string `null` back to `track_token_4` (defect D10).

`isLoggedIn` (`BaseTracker.kt:32-34`) is `username.isNotEmpty() && password.isNotEmpty()`. Both are cleared, so the tracker reports logged out.

### 2.8 Search (`Shikimori.kt:153-159`, `ShikimoriApi.kt:124-154`)

- `searchManga(query)`: `GET https://shikimori.one/api/mangas?order=popularity&search=<query>&limit=20` (query encoded by `Uri.appendQueryParameter`). `searchAnime(query)`: same with `/api/animes`.
- Goes through `authClient`, so it throws `Not authenticated with Shikimori` when logged out (defect D9). The endpoint itself is public.
- Response: JSON array parsed as `List<SMEntry>`, then mapped element by element:
  - Manga: `toMangaTrack(trackId)` (`SMEntry.kt:23-36`). Anime: `toAnimeTrack(trackId)` (`SMEntry.kt:38-51`).
  - `remote_id = id`, `title = name`, `total_chapters = chapters!!` (manga) or `total_episodes = episodes!!` (anime) (**throws `NullPointerException` if the field is null**, which fails the whole search), `cover_url = "https://shikimori.one" + image.preview`, `summary = ""`, `score = score` (catalogue average), `tracking_url = "https://shikimori.one" + url`, `publishing_status = status`, `publishing_type = kind`, `start_date = aired_on ?: ""`.
- No client-side paging: `limit=20` only.

### 2.9 Library lookup (`findLibManga`, `ShikimoriApi.kt:156-186`; `findLibAnime`, lines 188-218)

Two sequential calls, in this order:
1. `GET https://shikimori.one/api/mangas/{remote_id}` (anime: `/api/animes/{remote_id}`), parsed as a full `SMEntry`. A 404 throws `HttpException(404)` and aborts the lookup. Default cache-control applies (see 7.3).
2. `GET https://shikimori.one/api/v2/user_rates?user_id=<userId>&target_id=<remote_id>&target_type=Manga` (anime: `target_type=Anime`), parsed as `List<SMUserListEntry>`.
   - If `entries.size > 1`: `throw Exception("Too many manga in response")`. The same text is used for anime (defect D13).
   - Else `entries.map { it.toMangaTrack(trackId, manga) }.firstOrNull()`. An empty list gives `null` (entry not in the user's list).

`SMUserListEntry.toMangaTrack(trackId, manga)` (`SMUserListEntry.kt:17-28`): `title = manga.name`, `remote_id = id` (the user-rate id is copied into `remote_id`; the catalogue id is NOT kept, see the note below), `total_chapters = manga.chapters!!`, `library_id = id` (user-rate id), `last_chapter_read = chapters`, `score = score.toDouble()`, `status = toTrackStatus(status)`, `tracking_url = "https://shikimori.one" + manga.url`. Anime (`SMUserListEntry.kt:30-41`): `total_episodes = anime.episodes!!`, `last_episode_seen = episodes`.

Observed note: `remote_id` is overwritten with the user-rate id (`SMUserListEntry.kt:20`, `33`). Later calls use `track.remote_id` as the catalogue id (`ShikimoriApi.kt:159`, `169`) when the track came from a lookup. This is a latent inconsistency. The track's `remote_id` is only the catalogue id when the track was created by search (`SMEntry.toMangaTrack`, `remote_id = id`). Proposal: keep `remote_id` = catalogue id and `library_id` = user-rate id in every path (D17).

### 2.10 Bind (`Shikimori.kt:113-131` manga, `133-151` anime)

Input `track` (from search, `remote_id` = catalogue id), `hasReadChapters` (or `hasSeenEpisodes`) computed by area:27-tracking-core (`AddMangaTracks.kt:35` `allChapters.any { it.read }`).

Found path (`findLib...` returns non-null):
1. `track.copyPersonalFrom(remote)`. The database model copy is `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:36-44`: it copies `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date` and, because the default is `copyRemotePrivate = true`, `private`. The anime copy (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt:37-44`) copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date` and `private`.
   **Observed defect (D22):** the remote track built by `SMUserListEntry.to*Track` never sets the start or finish dates, so they are `0`. Step 1 therefore sets the local start and finish dates to `0` on every Shikimori bind that finds the entry. Bind restores only the start date afterwards (`AddMangaTracks.kt:58-74` and the anime twin, which use the reading history), and the finish date stays `0`.
2. `track.library_id = remote.library_id`.
3. If `track.status != COMPLETED`:
   - `isRereading = track.status == REREADING`
   - `track.status = if (!isRereading && hasReadChapters) READING else track.status`.
   So a remote `PLAN_TO_READ`, `ON_HOLD` or `DROPPED` becomes `READING` when local chapters are read. `REREADING` is kept.
4. `update(track)` (defaults `didRead = false`, see 2.11).

Not-found path (`findLib...` returned null):
1. `track.status = if (hasReadChapters) READING else PLAN_TO_READ`.
2. `track.score = 0.0`.
3. `add(track)` → `api.addLibManga(track, username)` (2.11). `library_id` is set from the POST response.

Note: when the entry is not found remotely, `hasReadChapters == true` still produces `READING` even if the local progress already equals the total (no `COMPLETED` transition in bind).

### 2.11 Write path: add, update, and the POST upsert

`addLibManga(track, userId)` (`ShikimoriApi.kt:42-68`) and `addLibAnime` (lines 83-109):
- `POST https://shikimori.one/api/v2/user_rates`, `Content-Type: application/json; charset=utf-8` (`jsonMime`, `OkHttpExtensions.kt:22`).
- Body exactly: `{"user_rate":{"user_id":"<userId string>","target_id":<remote_id>,"target_type":"Manga"|"Anime","chapters":<last_chapter_read.toInt()>|"episodes":<last_episode_seen.toInt()>,"score":<score.toInt()>,"status":"<toShikimoriStatus>"}}`. Key order as written; `user_id` is a JSON string (from `getUsername()`), other numbers are JSON numbers.
- Response: `awaitSuccess()` then parse `SMAddEntryResponse { id: Long }`; `track.library_id = id`.
- Returns the same `track` object (mutated).

`updateLibManga` and `updateLibAnime` are the same call (`ShikimoriApi.kt:70-73`, `111-114`). There is no PATCH and no PUT.

**[verified]** the server treats POST as an upsert: `Api::V2::UserRatesController#create` looks up `UserRate.find_by(user_id, target_id, target_type)` and calls `update_rate` if found, else `create_rate` (`app/controllers/api/v2/user_rates_controller.rb` in shikimori/shikimori, `create` action). So the Kotlin "add" and "update" are the same request, and the response `id` is the same user-rate id.

`update(track, didRead)` transitions (`Shikimori.kt:77-89` manga, `91-103` anime), exact:
```
if (track.status != COMPLETED) {
    if (didRead) {
        if (track.last_chapter_read.toLong() == track.total_chapters && track.total_chapters > 0) {
            track.status = COMPLETED
        } else if (track.status != REREADING) {
            track.status = READING
        }
    }
}
return api.updateLibManga(track, getUsername())
```
- `didRead = true` is passed only by `TrackChapter.await` (`app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt:43`) and `TrackEpisode.await` (`app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt:42`).
- Compare: `toLong()` truncates the Double. `last == total` with `total > 0` is the completion rule.

Server behaviour on completion **[verified]**: `UserRate.manga_status_changed` sets `chapters = target.chapters` and `volumes = target.volumes` when the status becomes completed and the target counts are nonzero (`app/models/user_rate.rb`); the anime equivalent sets `episodes = target.episodes`. The client value is sent anyway.

### 2.12 Delete (`Shikimori.kt:105-111`; `ShikimoriApi.kt:75-81`, `116-122`)

- `DELETE https://shikimori.one/api/v2/user_rates/<track.libraryId>` (string interpolation of a nullable `Long`; a null `libraryId` gives the literal path `user_rates/null`, which returns 404; defect D18). Requires `user_rates` scope (`UserRatesController#destroy`, `before_action` with `doorkeeper_authorize! :user_rates`).
- Success requires a 2xx status (`awaitSuccess`). Failures propagate to the caller.
- The caller (`AnimeTrackInfoDialog.kt:858-868`, `MangaTrackInfoDialog.kt:858-868`) runs `deleteAnimeFromService` in a non-cancellable coroutine and catches `Exception`, logs it with `logcat(LogPriority.ERROR)` and shows nothing to the user. Area:27 owns the dialog; this area owns the HTTP call.

### 2.13 Refresh (`Shikimori.kt:161-177` manga, `170-177` anime)

```
api.findLibManga(track, getUsername())?.let { remoteTrack ->
    track.library_id = remoteTrack.library_id
    track.copyPersonalFrom(remoteTrack)     // copyRemotePrivate defaults to true
    track.total_chapters = remoteTrack.total_chapters
} ?: throw Exception("Could not find manga")   // "Could not find anime" for anime
return track
```
- No write is sent. `refresh` does not call `update`.
- `copyPersonalFrom(remote)` sets `private` from the remote value, which is always `false` because `SMUserListEntry` never sets `private` (`SMUserListEntry.kt:17-28` leaves the default `false`). Shikimori does not support private tracking, so the local value is normally `false` anyway (see D14).
- The same copy sets `started_reading_date` (or `started_watching_date`) and the finish date from the remote track, which are `0` (D22). Refresh therefore clears the local dates. Area:27 restores nothing after a refresh.
- Message: `Could not find manga` / `Could not find anime`, shown as a raw toast (area:27-tracking-core `toast(e.message)`).

### 2.14 Status labels and lists (`Shikimori.kt:183-219`)

`getStatusListManga()` and `getStatusListAnime()` both return `listOf(READING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_READ, REREADING)` = `[1, 2, 3, 4, 5, 6]`.

`getStatusForManga(status)` (lines 191-199): `READING`→`MR.strings.reading`, `PLAN_TO_READ`→`MR.strings.plan_to_read`, `COMPLETED`→`MR.strings.completed`, `ON_HOLD`→`MR.strings.on_hold`, `DROPPED`→`MR.strings.dropped`, `REREADING`→`MR.strings.repeating`, else `null`.

`getStatusForAnime(status)` (lines 201-209): `READING`→`AYMR.strings.watching`, `PLAN_TO_READ`→`AYMR.strings.plan_to_watch`, `COMPLETED`→`MR.strings.completed`, `ON_HOLD`→`MR.strings.on_hold`, `DROPPED`→`MR.strings.dropped`, `REREADING`→`AYMR.strings.repeating_anime`, else `null`.

English values (`i18n/src/commonMain/moko-resources/base/strings.xml`, `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`): reading "Reading", completed "Completed", on_hold "On hold", dropped "Dropped", plan_to_read "Plan to read", repeating "Rereading", watching "Watching", plan_to_watch "Plan to watch", repeating_anime "Rewatching".

Reading and rewatching ids: `getReadingStatus()` = `READING`; `getWatchingStatus()` = `READING`; `getRereadingStatus()` = `REREADING`; `getRewatchingStatus()` = `REREADING`; `getCompletionStatus()` = `COMPLETED` (`Shikimori.kt:211-219`).

### 2.15 Remote-track errors on the Shikimori path

| Situation | Where | Result |
|---|---|---|
| Logged out, any authenticated call | interceptor line 23 | `Exception("Not authenticated with Shikimori")` |
| Unknown internal status on write | `toShikimoriStatus` | `NotImplementedError("Unknown status: N")` (an `Error`, see D4) |
| Unknown API status string on read | `toTrackStatus` | `NotImplementedError("Unknown status: <s>")` |
| HTTP 404 on `/api/mangas/{id}` | `findLib` step 1 | `HttpException(404)` |
| Two user-rate rows for one target | `findLib` step 2 | `Exception("Too many manga in response")` |
| Missing `chapters` / `episodes` in catalogue | `SMEntry.to*Track` | `NullPointerException` (`!!`) |
| Refresh response not 2xx | interceptor step 4c | silently ignored, request sent with expired token → server 401 → `HttpException(401)` |
| Refresh response not JSON-decodable | interceptor step 4b | `kotlinx.serialization` exception, propagates |
| Refresh token null and expiry check | interceptor step 3 | `NullPointerException` on every request |

---

## 3. Bangumi

### 3.1 Endpoints and constants (`BangumiApi.kt:267-296`)

- `CLIENT_ID = "bgm293165b66d7e58156"` (line 268), `CLIENT_SECRET = "21d5f5c19ac24b4bc9c855ffa2387030"` (line 269).
- `API_URL = "https://api.bgm.tv"` (line 271). `OAUTH_URL = "https://bgm.tv/oauth/access_token"` (line 272). `LOGIN_URL = "https://bgm.tv/oauth/authorize"` (line 273).
- `REDIRECT_URL = "aniyomi://bangumi-auth"` (line 275). `APP_JSON = "application/json"` (line 277).

Proposal on the credentials: section 8, D-OPEN-1.

### 3.2 Status mapping (`BangumiUtils.kt`)

Internal to collection `type` (`toApiStatus`, identical for manga and anime, lines 6-22):

| Internal | API `type` |
|---|---|
| `PLAN_TO_READ` (1) | 1 |
| `COMPLETED` (2) | 2 |
| `READING` (3) | 3 |
| `ON_HOLD` (4) | 4 |
| `DROPPED` (5) | 5 |
| anything else | throws `NotImplementedError("Unknown status: $status")` |

Read direction: `BGMCollectionResponse.getStatus()` (`BGMCollectionResponse.kt:18-25`) maps `1→PLAN_TO_READ`, `2→COMPLETED`, `3→READING`, `4→ON_HOLD`, `5→DROPPED`, else `NotImplementedError("Unknown status: $type")`. A null `type` also reaches the else branch.

Server **[verified]**: `SubjectCollectionType` enum `1` 想看 (wish), `2` 看过 (done), `3` 在看 (doing), `4` 搁置 (on hold), `5` 抛弃 (dropped) (`bgm_v0.yaml`, schema `SubjectCollectionType`, around line 2309).

### 3.3 Score

- `displayScore` = `score.toInt().toString()` (`Bangumi.kt:40-46`).
- Writes send `put("rate", track.score.toInt().coerceIn(0, 10))` (`BangumiApi.kt:48`, `66`, `84`, `109`). So a score of `-1` is sent as `0`, which the server reads as "delete rating" (`bgm_v0.yaml`, `UserSubjectCollectionModifyPayload.rate`: "`0` 表示删除评分"). This is the reverse of Shikimori (D8).
- Bind not-found sets `score = 0.0` (`Bangumi.kt:101`, `123`).
- Read: `rate` nullable, `rate?.toDouble() ?: 0.0` (`BangumiApi.kt:197`, `223`).

### 3.4 Login by OAuth code

1. Authorise URL (`BangumiApi.authUrl()`, lines 279-284): `https://bgm.tv/oauth/authorize?client_id=bgm293165b66d7e58156&response_type=code&redirect_uri=aniyomi%3A%2F%2Fbangumi-auth`. Parameter order: `client_id`, `response_type`, `redirect_uri`. No `scope`, no `state`. Opened via `openInBrowser(..., forceDefaultBrowser = true)` (`SettingsTrackingScreen.kt:213-218`).
2. Redirect `aniyomi://bangumi-auth?code=<code>` (the Bangumi authorisation code is valid for 60 seconds **[verified]**, `docs-raw/How-to-Auth.md` step 2). `TrackLoginActivity.handleBangumi` (lines 33-44) calls `login(code)` or `logout()` if the code is missing.
3. `login(code)` (`Bangumi.kt:190-202`):
   a. `oauth = api.accessToken(code)`.
   b. `interceptor.newAuth(oauth)`.
   c. `username = api.getUsername()` → `GET /v0/me` → `BGMUser.username`.
   d. `saveCredentials(username, oauth.accessToken)`.
   e. `catch (_: Throwable) { logout() }`. No message shown.
3a. `accessToken(code)` (`BangumiApi.kt:238-254`): `POST https://bgm.tv/oauth/access_token`, form body in order: `grant_type=authorization_code`, `client_id`, `client_secret`, `code`, `redirect_uri=aniyomi://bangumi-auth`. Runs on the base `client`. Parsed as `BGMOAuth`.

Note the Bangumi username is the account's login name (`BGMUser.username`), not the numeric id. The server returns the numeric id as a string if the user never set a username (comment `Bangumi.kt:194-196`). Shikimori uses the numeric id instead (2.4).

`getUsername()` (`BangumiApi.kt:256-265`): `GET https://api.bgm.tv/v0/me` with bearer; returns `BGMUser.username` (`BGMUser.kt:7-9`). Note: `/v0/me` also returns `email` and `reg_time` (`bgm_v0.yaml` around line 1025), which the Kotlin ignores.

### 3.5 Token persistence

Same mechanism as Shikimori with key `track_token_5`:
- `saveToken(oauth: BGMOAuth?)` writes `json.encodeToString(oauth)` (`Bangumi.kt:204-206`).
- `restoreToken()` decodes; any exception returns `null` (`Bangumi.kt:208-214`).
- Stored shape: `{"access_token","token_type","created_at","expires_in","refresh_token","user_id"}`. `created_at` has `@EncodeDefault` (`BGMOAuth.kt:14`), so it is always written. `user_id` is written as `null` when absent (no default, `BGMOAuth.kt:20-21`).

### 3.6 Request interceptor (`BangumiInterceptor.intercept`, lines 20-45)

Same use as Shikimori (`BangumiApi.kt:41`: `client.newBuilder().addInterceptor(interceptor).build()`).

Algorithm, exact order:
1. `originalRequest = chain.request()`.
2. `var currAuth: BGMOAuth = oauth ?: throw Exception("Not authenticated with Bangumi")` (line 23).
3. If `currAuth.isExpired()` (line 25):
   a. `response = chain.proceed(BangumiApi.refreshTokenRequest(currAuth.refreshToken!!))` (line 26). The `!!` is evaluated only in this branch (unlike Shikimori, D3).
   b. If successful: `currAuth = json.decodeFromString<BGMOAuth>(response.body.string())`, then `newAuth(currAuth)` (lines 27-29).
   c. Else `response.close()` and continue with the old `currAuth` (defect D1).
4. Return `originalRequest.newBuilder().header("User-Agent", "jmir1/Aniyomi/v${BuildConfig.VERSION_NAME} (Android) (https://github.com/aniyomiorg/aniyomi)").apply { addHeader("Authorization", "Bearer ${currAuth.accessToken}") }.build().let(chain::proceed)` (lines 35-44).

`isExpired()` (`BGMOAuth.kt:25`): `(System.currentTimeMillis() / 1000) > (createdAt + expiresIn - 3600)`. Comment: "Access token refresh before expired". Bangumi `expires_in` is 604800 seconds (7 days) **[verified]** (`docs-raw/How-to-Auth.md` token response example). So refresh starts when less than 1 hour is left, that is after 6 days 23 hours.

`refreshTokenRequest(token)` (`BangumiApi.kt:286-295`): `POST https://bgm.tv/oauth/access_token`, form body in order: `grant_type=refresh_token`, `client_id`, `client_secret`, `refresh_token`, `redirect_uri=aniyomi://bangumi-auth`. **[verified]** the Bangumi refresh doc lists `redirect_uri` as required for this grant (`docs-raw/How-to-Auth.md`, refresh section). The refresh response returns a new `refresh_token` as well.

`newAuth(oauth)` (lines 47-62): if `oauth == null`, `this.oauth = null`; else `this.oauth = BGMOAuth(accessToken, tokenType, now, expiresIn, refreshToken, this.oauth?.userId)`. Then `bangumi.saveToken(oauth)` persists the argument as given (with `user_id` as decoded). So the in-memory `user_id` can differ from the persisted one (D15).

Thread safety: same as Shikimori (D2).

### 3.7 Logout (`Bangumi.kt:216-220`)

Same order as Shikimori (2.7): `super.logout()`, `trackToken(this).delete()`, `interceptor.newAuth(null)` (which writes `"null"`, D10).

### 3.8 Search (`BangumiApi.kt:129-184`)

Both use `POST https://api.bgm.tv/v0/search/subjects?limit=20` with JSON body:
- Manga: `{"keyword": "<q>", "sort": "match", "filter": {"type": [1]}}` (lines 136-144).
- Anime: `{"keyword": "<q>", "sort": "match", "filter": {"type": [2]}}` (lines 165-173).
- `Content-Type: application/json`.
- Goes through `authClient`, so it requires login (`Not authenticated with Bangumi`). **[verified]** the search endpoint has no `security` entry in `bgm_v0.yaml` (`/v0/search/subjects`, `searchSubjects`), so it is public (D9).
- Response: `BGMSearchResult` (`total`, `limit`, `offset`, `data`).
- Manga results: `.data.filter { it.platform == null || it.platform == "漫画" }` (line 152), then `toMangaTrackSearch`. Anime: no filter (`map { it.toAnimeTrackSearch }`, line 181).
- `sort` enum per spec **[verified]**: `match`, `heat`, `rank`, `score`. Kotlin uses `match` only.

Mapping (`BGMSearch.kt:31-59`):
- `remote_id = id`.
- `title = nameCn.ifBlank { name }` (Kotlin `ifBlank`: blank or empty string falls through).
- `cover_url = images?.common.orEmpty()`.
- `summary`: if `nameCn` is not blank: `"作品原名：" + name + (summary?.let { "\n" + it.trim() } ?: "")`, otherwise `summary?.trim() ?: ""`.
- `score = rating?.score ?: -1.0`.
- `tracking_url = "https://bangumi.tv/subject/<id>"`.
- Manga: `total_chapters = eps`. Anime: `total_episodes = eps`. `eps` defaults to `0`.
- `start_date = date ?: ""` (`YYYY-MM-DD`).

Note: the search result's `score` is `-1.0` when there is no rating (`BGMSearch.kt:40`, `55`), while the Shikimori catalogue score is always present.

### 3.9 Collection lookup (`statusLibManga`, `BangumiApi.kt:186-210`; `statusLibAnime`, lines 212-236)

- `GET https://api.bgm.tv/v0/users/<username>/collections/<remote_id>` with `cache = CacheControl.FORCE_NETWORK` (line 191). Username is inserted without URL encoding (Proposal P-encode in section 8).
- Parsed as `BGMCollectionResponse`. Then the lambda writes into the **same `track` object** and returns it:
  - `track.status = it.getStatus()` (throws `NotImplementedError` for unknown `type`).
  - `track.last_chapter_read = ep_status?.toDouble() ?: 0.0` (anime: `last_episode_seen`).
  - `track.score = rate?.toDouble() ?: 0.0`.
  - `track.total_chapters = subject?.eps?.toLong() ?: 0L` (anime: `total_episodes`).
  - Returns `track` (line 199 and 225): the **same reference** as the input (aliasing, D5).
- A 404 `HttpException` returns `null` ("subject is not collected by user", lines 201-206 and 227-232). Other HTTP codes and all other exceptions are rethrown.
- `private` in the response is parsed (`BGMCollectionResponse.kt:15`) but never copied to the track.
- `ep_status` is read from the same field for manga and anime.

### 3.10 Bind (`Bangumi.kt:84-104` manga, `106-126` anime)

Input `track`, `hasReadChapters` (or `hasSeenEpisodes`).

1. `statusTrack = api.statusLibManga(track, getUsername())`. Because `statusTrack === track`, all the assignments below are self-assignments once `statusLibManga` has run.
2. Found path (`statusTrack != null`):
   a. `track.copyPersonalFrom(statusTrack, copyRemotePrivate = false)`. Self-copy: no effect.
   b. `track.library_id = statusTrack.library_id` (self-copy; `library_id` is never set by the collection read, so this stays `null`).
   c. `track.score = statusTrack.score`, `track.last_chapter_read = ...`, `track.total_chapters = ...`: self-assignments.
   d. `if (track.status != COMPLETED) track.status = if (hasReadChapters) READING else statusTrack.status`. The result is `READING` when local chapters are read, otherwise the remote status.
   e. `update(track)` (`didRead = false`), which sends the PATCH (3.11).
3. Not-found path: `track.status = if (hasReadChapters) READING else PLAN_TO_READ`, `track.score = 0.0`, `add(track)` (POST, 3.11).

Bangumi bind never copies `private` (`copyRemotePrivate = false`), so the local private flag survives a bind.

Note: in the found path with `hasReadChapters == false`, `track.status` already equals the remote status, so step 2d is a no-op.

### 3.11 Write path: POST add, PATCH update

Both build the same JSON body (`BangumiApi.kt:45-52`, `63-70`, `81-88`, `106-113`), keys in this order:
```
{"type": <toApiStatus>, "rate": <score.toInt().coerceIn(0,10)>, "ep_status": <last_chapter_read.toInt()> (manga) | <last_episode_seen.toInt()> (anime), "private": <boolean>}
```
- Target URL: `https://api.bgm.tv/v0/users/-/collections/<remote_id>` (the `-` means the authenticated user).
- Add: **POST**, headers `Content-Type: application/json`. Response `202 Accepted`, no body (comment at line 54). The returned track is unchanged; `library_id` is not set (D19).
- Update: **PATCH** (`Request.Builder().patch(body)`, lines 91-95 and 116-120). Response `204 No Content`.
- Both use `awaitSuccess()`. Non-2xx throws `HttpException`.

Server **[verified]**: `postUserCollection` says "修改条目收藏状态, 如果不存在则创建，如果存在则修改" (create if missing, else modify). Both POST and PATCH accept all fields as optional. The server also states: "由于直接修改剧集条目的完成度可能会引起意料之外效果，只能用于修改书籍类条目的完成度" (directly changing episode completion may cause unexpected effects; use `ep_status` only for book subjects). Consequence for anime: Aniyomi's `ep_status` writes to anime subjects are documented as unsafe (D7).

`update(track, didRead)` (`Bangumi.kt:56-68` manga, `70-82` anime), exact:
```
if (track.status != COMPLETED) {
    if (didRead) {
        if (track.last_chapter_read.toLong() == track.total_chapters && track.total_chapters > 0) {
            track.status = COMPLETED
        } else {
            track.status = READING
        }
    }
}
return api.updateLibManga(track)   // PATCH
```
Unlike Shikimori, there is no `REREADING` guard (not needed; Bangumi has no rereading). `DROPPED`, `ON_HOLD` and `PLAN_TO_READ` all become `READING` on read.

### 3.12 Refresh (`Bangumi.kt:136-146` manga, `142-146` anime)

```
val remoteStatusTrack = api.statusLibManga(track, getUsername()) ?: throw Exception("Could not find manga")
track.copyPersonalFrom(remoteStatusTrack)     // copyRemotePrivate = true (default)
return track
```
Because `remoteStatusTrack === track`, the copy is a self-copy. The real work is the mutation inside `statusLibManga`. Net effect: status, progress, score and total are refreshed from the server; `library_id` and `private` are untouched. The message for a missing entry is `Could not find manga` / `Could not find anime`.

### 3.13 Status labels and lists (`Bangumi.kt:152-186`)

`getStatusListManga()` and `getStatusListAnime()` both return `listOf(READING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_READ)` = `[3, 2, 4, 5, 1]` (`Bangumi.kt:152-158`). This order is the order of the status picker.

`getStatusForManga` (lines 160-168): `READING`→`MR.strings.reading`, `PLAN_TO_READ`→`MR.strings.plan_to_read`, `COMPLETED`→`MR.strings.completed`, `ON_HOLD`→`MR.strings.on_hold`, `DROPPED`→`MR.strings.dropped`, else `null` (no rereading label).

`getStatusForAnime` (lines 169-177): `READING`→`AYMR.strings.watching`, `PLAN_TO_READ`→`AYMR.strings.plan_to_watch`, `COMPLETED`→`MR.strings.completed`, `ON_HOLD`→`MR.strings.on_hold`, `DROPPED`→`MR.strings.dropped`, else `null`.

Ids: `getReadingStatus()` = `READING` (3), `getWatchingStatus()` = `READING` (3), `getRereadingStatus()` = `-1`, `getRewatchingStatus()` = `-1`, `getCompletionStatus()` = `COMPLETED` (2) (`Bangumi.kt:178-186`).

Consequence in the framework (area:27-tracking-core, `MangaTracker.setRemoteLastChapterRead`, `MangaTracker.kt:75-80`): the check `track.status != getRereadingStatus()` is always true for Bangumi, so the "start reading" transition always runs when the first chapter is set.

### 3.14 Private tracking and dates

- `supportsPrivateTracking = true` (`Bangumi.kt:32`). Area:27 shows the private toggle in the search sheet (`app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt:184`). The value is sent on every write (`"private"` key). It is never read back from the server (D14).
- `supportsReadingDates = false` (BaseTracker). `setRemoteStartDate` and `setRemoteFinishDate` (area:27-tracking-core) still call `update`, but the dates are not part of the Bangumi or Shikimori payloads. The dates are therefore local-only for both trackers (section 4).

### 3.15 Remote-track errors on the Bangumi path

| Situation | Where | Result |
|---|---|---|
| Logged out, any authenticated call | interceptor line 23 | `Exception("Not authenticated with Bangumi")` |
| Unknown internal status on write | `toApiStatus` | `NotImplementedError("Unknown status: N")` (D4) |
| Unknown or null `type` on read | `getStatus` | `NotImplementedError("Unknown status: <type>")` |
| 404 on collection GET | `statusLib` | returns `null` (not collected) |
| 401/403/5xx on collection GET | `statusLib` | `HttpException(code)` rethrown |
| Refresh response not 2xx | interceptor 3c | silently ignored, request sent with expired token → server 401 |
| Refresh token null and expired | interceptor 3a | `NullPointerException` |
| Missing `name_cn`, `id` or `name` | `BGMSubject` decoding | search fails (`MissingFieldException`); other nullable fields may be absent (D20) |

---

## 4. Side-by-side comparison

| Topic | Shikimori | Bangumi |
|---|---|---|
| Token lifetime | 86400 s (`doorkeeper.rb:70`) [verified] | 604800 s (`How-to-Auth.md`) [verified] |
| Refresh threshold | expires at `createdAt + expiresIn - 3600` | same formula |
| Refresh request extras | none | `redirect_uri` included |
| Refresh rotates `refresh_token` | yes in the response (stored) | yes in the response (stored) |
| Username saved | numeric id string from `/api/users/whoami` | login name from `/v0/me` |
| Authorise scope | default (`user_rates`, `comments`, `topics`) | none sent |
| Search endpoint | `GET /api/mangas`, `GET /api/animes` | `POST /v0/search/subjects` |
| Search needs login (Kotlin) | yes (`authClient`) | yes (`authClient`) |
| Search needs login (server) | no (public GET) | no (no `security` in spec) [verified] |
| Library lookup | two GETs (catalogue + user_rates list) | one GET (`/v0/users/{u}/collections/{id}`, FORCE_NETWORK) |
| Not-in-list signal | empty user_rates array → `null` | HTTP 404 → `null` |
| Write | POST upsert (add and update identical) | POST (add), PATCH (update), separate status codes 202/204 |
| Score sent | `score.toInt()` unclamped | `rate` clamped to 0..10 |
| Progress field | `chapters` / `episodes` | `ep_status` for both kinds |
| Private flag | not supported | supported, write-only |
| Dates | local only | local only |
| Delete | `DELETE /api/v2/user_rates/{libraryId}` | not supported |
| Rereading status | 6 (`REREADING`) | none (-1) |
| Status count (list) | 6 | 5 |
| Logout storage quirk | writes `"null"` | writes `"null"` |

---

## 5. User-visible messages

Strings that appear to the user through this area, with their source.

Raw (non-localised) exception text, shown by `toast(e.message)` in area:27-tracking-core (`MangaTracker.kt:62`, `113-119`, and the anime equivalents):
- `Not authenticated with Shikimori`, `Not authenticated with Bangumi`.
- `Too many manga in response` (both anime and manga, defect D13).
- `Could not find manga`, `Could not find anime`.
- `Unknown status: <n>` (inside `NotImplementedError`, so `e.message` is only `Unknown status: <n>`).
- `HTTP error <code>` for `HttpException` (`HttpException.kt:13`, `IllegalStateException("HTTP error $code")`).
- Other exceptions use their `message`. A `NullPointerException` from `!!` has a null message, so the toast is empty (`toast(null)`).

Formatted message (`ExceptionFormatter.kt:20`, used by other screens): `HttpException` → `MR.strings.exception_http` = `"HTTP %d, check website in WebView"`.

Silent cases (no message at all):
- Login failure (`catch (e: Throwable) { logout() }` in both trackers).
- Delete failure (logged to logcat only, `AnimeTrackInfoDialog.kt:862-866`).
- Refresh failure (interceptor ignores it).
- Unknown browser for `openInBrowser` falls back to `toast(e.message)` (`ContextExtensions.kt:84-86`).

Localised labels used: the status labels in 2.14 and 3.13, and `MR.strings.track_error` (`"%1$s error: %2$s"`) if area:27-tracking-core uses it.

Logo and colours: Shikimori logo `ic_tracker_shikimori.webp` (`app/src/main/res/drawable-nodpi/`), colour `#282828`. Bangumi logo `ic_tracker_bangumi.webp`, colour `#F09199`.

---

## 6. Call paths from other areas

| Caller (full path under `app/src/main/java/eu/kanade/`) | Method called in this area | Notes |
|---|---|---|
| `domain/track/manga/interactor/AddMangaTracks.kt:36` | `bind(track, hasReadChapters)` | Then reads `track.lastChapterRead` and may call `setRemoteLastChapterRead`. |
| `domain/track/anime/interactor/AddAnimeTracks.kt:36` | `bind(track, hasSeenEpisodes)` | Same for episodes. |
| `domain/track/manga/interactor/TrackChapter.kt:41-43` | `refresh(track)` then `update(track, true)` | Failure queues a delayed update (`DelayedMangaTrackingStore`). |
| `domain/track/anime/interactor/TrackEpisode.kt:42` | `update(track, true)` | Anime equivalent. |
| `domain/track/manga/interactor/SyncChapterProgressWithTrack.kt:44` | `update(track)` | Only for enhanced trackers, so not Shikimori or Bangumi. |
| `tachiyomi/data/track/MangaTracker.kt:66-109` (and `AnimeTracker.kt:67-108`) | `update(track)` via `updateRemote` | Errors toasted, not thrown. |
| `tachiyomi/data/track/MangaTracker.kt:57-64` | `bind` via `AddMangaTracks` (`register`) | Errors toasted. |
| `ui/entries/manga/track/MangaTrackInfoDialog.kt:858-868` | `delete(track)` | Shikimori only. |
| `ui/setting/track/TrackLoginActivity.kt:59-69` | `login(code)` or `logout()` | Area:27. |
| `presentation/more/settings/screen/SettingsTrackingScreen.kt:191-219` | `ShikimoriApi.authUrl()`, `BangumiApi.authUrl()` | Area:27. |

Paths in this table are relative to `app/src/main/java/eu/kanade/` and to `app/src/main/java/eu/kanade/tachiyomi/` where the row says `tachiyomi/`. Other short citations in this document (for example `ShikimoriApi.kt:51`) refer to the scope files listed in p1-inventory.md, and `AppModule.kt` to `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt`.

`updateRemote` writes the result to the database with `insertTrack.await(it)` (`MangaTracker.kt:114-116`) after `update` succeeds. A failed `update` is not written.

---

## 7. Timings, limits, concurrency

### 7.1 Timings

| Name | Value | Where |
|---|---|---|
| Access token refresh margin | 3600 s before `createdAt + expiresIn` | `SMOAuth.kt:21`, `BGMOAuth.kt:25` |
| Shikimori access token lifetime | 86400 s | server `doorkeeper.rb:70` [verified] |
| Bangumi access token lifetime | 604800 s | `How-to-Auth.md` [verified] |
| Bangumi authorisation code validity | 60 s | `How-to-Auth.md` [verified] |
| Shikimori authorisation code validity | 1800 s (30 min) | `doorkeeper.rb:65` [verified] |
| Search page size | `limit=20` | `ShikimoriApi.kt:129`, `145`; `BangumiApi.kt:135`, `164` |
| Score scale | 0..10 | `Shikimori.kt:44-46`, `Bangumi.kt:229-231` |

### 7.2 Limits

- Shikimori `user_rates` GET accepts `limit` up to 1000 **[verified]** but the Kotlin never sends `limit` for the per-target query (`ShikimoriApi.kt:167-172`).
- Shikimori text field maximum 4096 **[verified]** (unused by this area).
- Bangumi episode list `limit` max 1000 (`/v0/users/-/collections/{subject_id}/episodes`) **[verified]**, unused here.

### 7.3 HTTP cache (interaction with area:12-network-stack)

- `GET(url, headers, cache)` in `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:16` sets `DEFAULT_CACHE_CONTROL = CacheControl.Builder().maxAge(10, MINUTES)`. So every Shikimori GET here asks for a cached response up to 10 minutes old. That includes `findLib` steps 1 and 2 and `whoami`.
- Bangumi `statusLibManga` passes `CacheControl.FORCE_NETWORK` (`BangumiApi.kt:191`, `217`). Bangumi search (POST) is never cached.
- Consequence (D12): after a write, a Shikimori refresh can read a stale `user_rates` row for up to 10 minutes, if the OkHttp cache is enabled in area:12-network-stack.

### 7.4 Concurrency

- All network methods run inside `withIOContext` (`ShikimoriApi.kt:43`, `76`, `84`, `117`, `125`, `141`, `157`, `189`, `230`; `BangumiApi.kt:44`, `62`, `80`, `106`, `134`, `163`, `187`, `213`, `239`, `257`), except `getCurrentUser` (`ShikimoriApi.kt:220-227`).
- Login runs in `lifecycleScope.launchIO` (`TrackLoginActivity.kt:62`, `36`) and is not cancelled by the UI.
- The interceptor state (`oauth`) has no synchronisation (D2).
- Two trackers can run in parallel, since each has its own interceptor and token.
- Per-request timeouts are whatever area:12-network-stack configures. This area sets none.

---

## 8. Defects, observed behaviour, and proposals

Each defect says what the Kotlin does, why it matters, and what Kurobayomi does. "D-OPEN" items need a user decision.

| ID | Observed (Kotlin) | Impact | Proposal (Kurobayomi) |
|---|---|---|---|
| D1 | Both interceptors ignore a failed refresh (`ShikimoriInterceptor.kt:30-34`, `BangumiInterceptor.kt:27-31`). The request is sent with the expired token. | User sees `HTTP error 401` in toasts, and no re-login prompt. The auth-expired flag is never set. | On refresh 400 or 401, set the auth-expired flag (area:27-tracking-core `trackAuthExpired`) and fail the call with a typed `TrackError::AuthExpired`. Keep credentials until logout. |
| D2 | `oauth` is a plain `var` with no lock. Parallel requests can each refresh. | Doorkeeper issues a new refresh token on refresh **[verified]**, so a second refresh with the old token can fail and log the user out in effect. | Single-flight refresh: one `tokio::sync::Mutex` around the token; a waiter re-reads the token after the lock. See p1-topic-token-lifecycle.md section 3. |
| D3 | Shikimori evaluates `refreshToken!!` for every request (`ShikimoriInterceptor.kt:25`). | Null refresh token breaks every call, not just refresh. | Check for a missing refresh token only when expired, and return `AuthExpired`. |
| D4 | `NotImplementedError` (an `Error`, not an `Exception`) is thrown for unknown statuses. `updateRemote` catches only `Exception` (`MangaTracker.kt:117`, `AnimeTracker.kt:116`). | Unknown status escapes the catch and can crash the coroutine scope. | Return `TrackError::UnknownStatus(n)`. Never panic on a remote value. |
| D5 | Bangumi `statusLib` returns the input object (`BangumiApi.kt:199`, `225`). | `copyPersonalFrom(self)` in bind and refresh is a no-op. It works, but only by accident. | Return a new `RemoteStatus` value and copy explicitly. |
| D6 | **Withdrawn after verification.** `BGMOAuth.userId: Long?` has no default, but the app's `Json` sets `explicitNulls = false` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:173-177`), so an absent nullable key decodes as `null`. The documented refresh response, which omits `user_id` (`How-to-Auth.md`), therefore decodes. | None. Kept so the IDs stay stable. Kurobayomi still needs `Option` for `user_id` and the refresh body test (EC-B-12). | None needed beyond `Option<u64>`. |
| D7 | Anime progress is written as `ep_status` (`BangumiApi.kt:110`). The server spec says `ep_status` is for book subjects only (`bgm_v0.yaml`, `UserSubjectCollectionModifyPayload.ep_status`), and that anime completion changes may have unexpected effects. | Episode progress for anime may not be recorded, or may be recorded inconsistently, on Bangumi. | Keep the Kotlin behaviour as the default in the first port (parity), and add a P2 task (T-31a-044) to write anime progress through `PATCH /v0/users/-/collections/{subject_id}/episodes` (episode-id based, documented). Mark the Bangumi anime progress as "unverified" in the UI until then. |
| D8 | Shikimori sends `score.toInt()` unclamped (`ShikimoriApi.kt:51`, `92`). The server silently reverts out-of-range values to the old value (`user_rate.rb:162-166`) [verified]. | Local and remote scores can diverge without an error. | Clamp to 0..10 on the client, treating negatives as 0, to match Bangumi. |
| D9 | Both searches go through `authClient`, so both require login (`ShikimoriApi.kt:124-154`, `BangumiApi.kt:129-184`). The server endpoints are public [verified]. | Users cannot search before logging in. | P2 proposal: search with the plain client and no bearer header. |
| D10 | `logout()` writes the string `"null"` to `track_token_N` after deleting it (`Shikimori.kt:248-249`, `Bangumi.kt:218-219`). | Harmless (decode fails, returns null), but it is a stored string that looks like a token. | Delete the secret-store entry; never write a null token. |
| D11 | The access token is also stored in the password preference (`Shikimori.kt:228`, `Bangumi.kt:198`). The client secrets are in source (`ShikimoriApi.kt:259`, `BangumiApi.kt:269`). | Token readable from the preference file. Client secret is public in the repository. | Store tokens in the secret store (p1-rust-design.md section 5). For the client secret, see D-OPEN-1. |
| D12 | Shikimori GETs use the 10 minute cache-control (`Requests.kt:16`). Bangumi collection GET forces network (`BangumiApi.kt:191`). | Stale `user_rates` after a write, if the HTTP cache is on. | Force network on every track read (`Cache-Control: no-cache`). |
| D13 | `"Too many manga in response"` is also thrown for anime (`ShikimoriApi.kt:210`). | Confusing message. | Separate keys: `track.err.too_many_entries` (manga and anime). Same text is fine if the wording is neutral; the key is what matters. |
| D14 | Remote `private` is never read back (Bangumi `BGMCollectionResponse.kt:15` is parsed but unused). Shikimori `copyPersonalFrom` resets local `private` to `false`. | Local private state is never confirmed by the server. | Keep the local value as the source of truth for Bangumi. For Shikimori, do not copy `private` (it is unsupported). |
| D15 | Bangumi in-memory `userId` comes from the previous value, but the persisted token keeps the decoded `user_id` (`BangumiInterceptor.kt:51-58`, `61`). | Minor inconsistency. `userId` is never read anywhere. | Drop the in-memory `userId` field. Keep the persisted field as an optional value, which is no longer used. |
| D16 | Refresh requests use `chain.proceed`, which skips the outer application interceptors of the base client (area:12-network-stack). | Refresh does not get the base client's headers, rate limit or logging. | Refresh through the raw HTTP client, with the same User-Agent. |
| D17 | `SMUserListEntry` overwrites `remote_id` with the user-rate id (`SMUserListEntry.kt:20`, `33`). | Later calls mix two id spaces. | Keep `remote_id` = catalogue id, `library_id` = user-rate id, on every path. |
| D18 | `deleteLibManga` uses `track.libraryId` without a null check (`ShikimoriApi.kt:78`, `119`). | Null id gives `user_rates/null` and a 404. | Refuse to call delete without a library id; return `TrackError::NoLibraryId`. |
| D19 | Bangumi add (POST, 202) does not set `library_id`; the field stays null. | Entry has no remote handle. Only matters for the delete path, which Bangumi does not implement. | Keep null, document it. |
| D20 | Required (non-null, no default) in `BGMSubject`: `id`, `name_cn`, `name` (`BGMSearch.kt:18-21`). A search result without `name_cn` fails the whole search. Nullable ones (`summary`, `date`, `images`, `rating`, `platform`) may be absent. `volumes` and `eps` default to 0. | Kotlin is correct here, but the rule is easy to get wrong in Rust. | Make `name_cn` required in Rust (`String`, serde default to empty string is acceptable too; see p1-rust-design.md section 4.3). Other optional fields use `Option` and `#[serde(default)]`. |
| D21 | Required in `SMEntry`: `id`, `name`, `image`, `score`, `url`, `status`, `kind` (`SMEntry.kt:11-19`). Nullable (may be absent): `chapters`, `episodes`, `aired_on` (`SMEntry.kt:13-21`). | A catalogue entry missing a required field fails the whole search. | Same classification as the Kotlin. |
| D22 | The remote track from `SMUserListEntry.to*Track` has `0` for start and finish dates, and `copyPersonalFrom` copies them (`MangaTrack.kt:36-44`, `AnimeTrack.kt:37-44`). Shikimori bind and refresh therefore erase the local start and finish dates (section 2.10 and 2.13). | Local date history is lost on every bind or refresh. Bind restores only the start date from reading history. | Copy only the progress and status fields from the remote entry. Leave the dates alone, because Shikimori does not provide them. Restore the start date from history only when the local value is `0`, as bind does today. |

### 8.1 Decisions that need the user (D-OPEN)

- **D-OPEN-1, OAuth client ownership.** The Shikimori client id and secret (`ShikimoriApi.kt:258-259`) and the Bangumi client id and secret (`BangumiApi.kt:268-269`) belong to the Aniyomi project. Area:27 proposes keeping the same redirect scheme (`aniyomi://`) and the same clients (topic doc `plans/27-tracking-core/p1-topic-tracker-oauth.md`, option A). Shikimori's authorisation server needs a confidential client secret for the token exchange, so a desktop build cannot hide it. Options: (a) reuse the Aniyomi registrations (least work, but the Kurobayomi app is then sending Aniyomi's identity, and the redirect scheme must be claimed on Linux); (b) register Kurobayomi's own applications at `https://shikimori.one/oauth/applications` and `https://bgm.tv/dev/app` and read `client_id` and `client_secret` from the user's configuration file. Recommendation (Proposal): (b). This area cannot register applications; the user must do it. Until then the code is written against a configuration value, and the Aniyomi constants are not copied.
- **D-OPEN-2, stored secret names.** Area:27 decides the names. This area uses area:27-tracking-core's proposal: service `kurobayomi.track`, accounts `4.token` / `5.token` for the OAuth token JSON and `4.password` / `5.password` (access token) and `4.username` / `5.username` for the credentials (see p1-rust-design.md section 5.5).
- **D-OPEN-3, Bangumi anime progress.** Keep `ep_status` (parity, D7) or switch to the episode endpoint (correct per the spec, more work). Recommendation: parity first, episode endpoint as a P2 task.

---

## 9. Pitfalls found while reading (for the implementer)

1. The POST on Shikimori is an upsert. Do not split add and update into two code paths that differ in verb; keep one function.
2. Bangumi's collection GET returns the same object that the caller holds. In Rust, return a new value; do not reproduce the aliasing.
3. Both trackers use their own status numbers. Never share the numeric constants between them. Use one enum per tracker with explicit `i64` discriminants.
4. Score `-1` means "unscored" in search results, but `-1` is not a valid write value for either tracker. Clamp on write (D8 and section 3.3).
5. Bangumi `rate: 0` deletes the rating. Do not write `0` when the user only wants to keep the current rating.
6. The stored refresh token rotates on every refresh (Doorkeeper default **[verified]** that a new refresh token is issued, and Bangumi's refresh response includes a new `refresh_token`). Always persist the whole response.
7. A 404 means different things: for Bangumi it is "not collected" (returns null); for Shikimori the catalogue 404 is an error and an empty user_rates list is "not collected".
8. `ep_status` is "book progress" on Bangumi. See D7 before sending it for anime.
9. Do not send the `private` key to Shikimori. The Kotlin never does.
10. Both trackers' date fields are local. Do not invent remote date writes.
