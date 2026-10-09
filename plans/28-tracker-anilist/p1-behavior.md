# area:28-tracker-anilist, part 1 — behaviour spec

Repo root: `/home/blackbox/aniyomi-src`. Every path is repo-relative.
Labels: **OBSERVED** = what the Kotlin does (with `path:line`). **Proposal:** = what the Rust port should do.

Short names used below:
- `ANI` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt`
- `API` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt`
- `INT` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt`
- `UTL` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistUtils.kt`
- `DTO/` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/`

---

## 1. Tracker identity and capabilities

**OBSERVED**
- Tracker id `2L` (`app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:20` `const val ANILIST = 2L`, `:28` `val aniList = Anilist(ANILIST)`).
- Display name `"AniList"` (`ANI:31`).
- Logo drawable `R.drawable.ic_tracker_anilist` (`ANI:78`).
- Logo colour `Color.rgb(18, 25, 35)` (`ANI:80`), i.e. `#121923`.
- `supportsReadingDates = true` (`ANI:62`). Used by info dialogs to show start and finish date rows (`app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt:83,109-115`).
- `supportsPrivateTracking = true` (`ANI:64`). Shows the "Track privately" action in search (`app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt:184-194`) and the private toggle in the info dialog (`AnimeTrackInfoDialogHome.kt:84,120-122`).
- Implements `DeletableMangaTracker` and `DeletableAnimeTracker` (`ANI:35-36`). The "also remove from AniList" checkbox is shown only when `tracker is DeletableAnimeTracker` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt:811-817,858`).
- No enhanced tracker interface (`EnhancedAnimeTracker`/`EnhancedMangaTracker`) is implemented.

**Proposal:** Same identity constants in Rust: `TRACKER_ID: i64 = 2`, `NAME: &str = "AniList"`, `LOGO_RGB: (u8,u8,u8) = (18,25,35)`, `supports_reading_dates = true`, `supports_private_tracking = true`, `deletable = true`.

---

## 2. Status model

### 2.1 Local integer constants (stored in DB `status` column)

`ANI:39-47` and `ANI:49-53` (score constants separate, see §3).

| Constant | Value | Used for |
|---|---|---|
| `READING` | 1L | manga, "Reading" |
| `WATCHING` | 11L | anime, "Watching" |
| `COMPLETED` | 2L | both |
| `ON_HOLD` | 3L | both, "Paused" |
| `DROPPED` | 4L | both, "Dropped" |
| `PLAN_TO_READ` | 5L | manga, "Plan to read" |
| `PLAN_TO_WATCH` | 15L | anime, "Plan to watch" |
| `REREADING` | 6L | manga, "Rereading" |
| `REWATCHING` | 16L | anime, "Rewatching" |

The constants are the same integers across manga and anime tables; they are not disambiguated by table, so a manga row holding 11 would display nothing (see edge cases).

### 2.2 Status lists (ordering shown to user)

**OBSERVED**
- `getStatusListManga()` = `listOf(READING, PLAN_TO_READ, COMPLETED, REREADING, ON_HOLD, DROPPED)` (`ANI:82-84`).
- `getStatusListAnime()` = `listOf(WATCHING, PLAN_TO_WATCH, COMPLETED, REWATCHING, ON_HOLD, DROPPED)` (`ANI:86-88`).
- The status picker builds `Map` via `associateWith` over this list (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt:319-322`), preserving order. Manga equivalent at `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt:320`.

### 2.3 Display labels (string keys)

**OBSERVED**
- `getStatusForManga` (`ANI:90-98`):
  - READING → `MR.strings.reading` ("Reading")
  - PLAN_TO_READ → `MR.strings.plan_to_read` ("Plan to read")
  - COMPLETED → `MR.strings.completed` ("Completed")
  - REREADING → `MR.strings.repeating` ("Rereading")
  - ON_HOLD → `MR.strings.paused` ("Paused")
  - DROPPED → `MR.strings.dropped` ("Dropped")
  - else → `null` (renders empty string, `TrackStatusSelector` uses `value?.let { stringResource(it) } ?: ""`)
- `getStatusForAnime` (`ANI:100-108`):
  - WATCHING → `AYMR.strings.watching` ("Watching")
  - PLAN_TO_WATCH → `AYMR.strings.plan_to_watch` ("Plan to watch")
  - COMPLETED → `MR.strings.completed`
  - REWATCHING → `AYMR.strings.repeating_anime` ("Rewatching")
  - ON_HOLD → `MR.strings.paused`
  - DROPPED → `MR.strings.dropped`
  - else → `null`
- Keys are read from `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR).

### 2.4 Getter functions for the generic framework

**OBSERVED** (`ANI:110-118`): `getReadingStatus()=READING`, `getWatchingStatus()=WATCHING`, `getRereadingStatus()=REREADING`, `getRewatchingStatus()=REWATCHING`, `getCompletionStatus()=COMPLETED`.

### 2.5 Local status → AniList MediaListStatus (outgoing)

**OBSERVED** `UTL:10-28`.

Manga (`MangaTrack.toApiStatus`):
| Local | API string |
|---|---|
| READING (1) | `"CURRENT"` |
| COMPLETED (2) | `"COMPLETED"` |
| ON_HOLD (3) | `"PAUSED"` |
| DROPPED (4) | `"DROPPED"` |
| PLAN_TO_READ (5) | `"PLANNING"` |
| REREADING (6) | `"REPEATING"` |
| else | `throw NotImplementedError("Unknown status: $status")` |

Anime (`AnimeTrack.toApiStatus`):
| Local | API string |
|---|---|
| WATCHING (11) | `"CURRENT"` |
| COMPLETED (2) | `"COMPLETED"` |
| ON_HOLD (3) | `"PAUSED"` |
| DROPPED (4) | `"DROPPED"` |
| PLAN_TO_WATCH (15) | `"PLANNING"` |
| REWATCHING (16) | `"REPEATING"` |
| else | `throw NotImplementedError` |

### 2.6 AniList MediaListStatus → local status (incoming)

**OBSERVED** `DTO/ALManga.kt:73-81` (manga) and `DTO/ALAnime.kt:74-82` (anime):

Manga: `"CURRENT"`→READING, `"COMPLETED"`→COMPLETED, `"PAUSED"`→ON_HOLD, `"DROPPED"`→DROPPED, `"PLANNING"`→PLAN_TO_READ, `"REPEATING"`→REREADING, else → `NotImplementedError("Unknown status: $listStatus")`.

Anime: `"CURRENT"`→WATCHING, `"COMPLETED"`→COMPLETED, `"PAUSED"`→ON_HOLD, `"DROPPED"`→DROPPED, `"PLANNING"`→PLAN_TO_WATCH, `"REPEATING"`→REWATCHING, else → `NotImplementedError`.

**Proposal:** Rust enums `MangaStatus` and `AnimeStatus` with explicit `from_api`/`to_api` and `Result` errors instead of panics. Keep the stored integers unchanged for DB and backup compatibility. AniList only has six MediaListStatus values (CURRENT, PLANNING, COMPLETED, DROPPED, PAUSED, REPEATING); any other string is an error.

---

## 3. Score model

### 3.1 Score formats (user's AniList `scoreFormat`)

**OBSERVED**
- The format is **not user-selectable inside Aniyomi**. It is read from the AniList Viewer query at login (`API:getCurrentUser`, `ANI:351`) and stored in preference `anilist_score_type` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:37` `preferenceStore.getString("anilist_score_type", Anilist.POINT_10)`). No UI references this key (grep of `app/src/main/java` finds only `TrackPreferences.kt`, `Anilist.kt`, `AnilistUtils.kt`).
- Default before any login: `POINT_10`.
- Accepted values: `POINT_100`, `POINT_10`, `POINT_10_DECIMAL`, `POINT_5`, `POINT_3` (the five AniList `ScoreFormat` values handled; `ANI:49-53`).
- Any other stored value → `getScoreList()` throws `Exception("Unknown score type")` (`ANI:132`), `indexToScore` throws the same (`ANI:164`), `displayScore` falls to `toApiScore` which throws `NotImplementedError("Unknown score type")` (`UTL:55`).

### 3.2 Stored score

**OBSERVED**: Local `score` is a `Double` always on the **100-point scale** regardless of user format. `ANI:136-144` `get10PointScore(track) = track.score / 10.0`. `scoreRaw` in every mutation/query is the AniList raw 0-100 value (`score(format: POINT_100)` in the find queries, `scoreRaw:` in the save mutations). 0.0 means "unscored".

### 3.3 Score list shown in the picker (`getScoreList`)

**OBSERVED** `ANI:120-134`:

| Format | List (index 0..n) | Size |
|---|---|---|
| POINT_10 | `IntRange(0,10).map(Int::toString)` → `"0","1",…,"10"` | 11 |
| POINT_100 | `IntRange(0,100).map(Int::toString)` → `"0",…,"100"` | 101 |
| POINT_5 | `IntRange(0,5).map { "$it ★" }` → `"0 ★","1 ★",…,"5 ★"` | 6 |
| POINT_3 | `persistentListOf("-", "😦", "😐", "😊")` | 4 |
| POINT_10_DECIMAL | `IntRange(0,100).map { (it / 10f).toString() }` → `"0.0","0.1",…,"10.0"` | 101 |

### 3.4 Index → stored score (`indexToScore`, applied on confirm)

**OBSERVED** `ANI:146-166`:

| Format | Rule | Index 0 | Index 1 | Index 2 | Index 3 | Index 5 | Index 10 |
|---|---|---|---|---|---|---|---|
| POINT_10 | `index * 10.0` | 0 | 10 | 20 | 30 | 50 | 100 |
| POINT_100 | `index.toDouble()` | 0 | 1 | 2 | 3 | 5 | 10 |
| POINT_5 | `0 → 0.0`, else `index*20 - 10` | 0 | 10 | 30 | 50 | 90 | n/a |
| POINT_3 | `0 → 0.0`, else `index*25 + 10` | 0 | 35 | 60 | 85 | n/a | n/a |
| POINT_10_DECIMAL | `index.toDouble()` | 0 | 1 | 2 | 3 | 5 | 10 |

Notes:
- POINT_10_DECIMAL index `i` (display `i/10`) stores raw `i`, so the 100-point scale is consistent.
- `indexToScore(-1)` (value not found in list, see §3.6) is computed with the same formula. Results: POINT_10 → -10.0; POINT_100 → -1.0; POINT_5 → -30.0; POINT_3 → -15.0; POINT_10_DECIMAL → -1.0. These negative values are written to AniList as `scoreRaw` (`API` casts with `toInt()`), see edge cases.

### 3.5 Stored score → display string (`displayScore`)

**OBSERVED** `ANI:168-184` (manga) and `ANI:186-202` (anime; identical body):
- POINT_5:
  - `score == 0.0` → `"0 ★"`
  - else → `"${((score + 10) / 20).toInt()} ★"` (integer division: 10→"1 ★", 30→"2 ★", 50→"3 ★", 70→"4 ★", 90→"5 ★"; 35→"2 ★")
- POINT_3:
  - `score == 0.0` → `"0"`
  - `score <= 35` → `"😦"`
  - `score <= 60` → `"😐"`
  - else → `"😊"`
- else (POINT_10, POINT_100, POINT_10_DECIMAL) → `track.toApiScore()` (`UTL:32-56`):
  - POINT_10: `(this.toInt() / 10).toString()` (integer division; 85 → "8")
  - POINT_100: `this.toInt().toString()`
  - POINT_10_DECIMAL: `(this / 10).toString()` (Double; 85.0 → "8.5", 0.0 → "0.0", 100.0 → "10.0")
  - POINT_5 / POINT_3 branches in `toApiScore` are unreachable from `displayScore`.

### 3.6 Score picker initial selection and confirm (generic framework applies `indexOf`)

**OBSERVED**
- The picker's pre-selected value is `displayScore(track)` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt:436`).
- The wheel start index is `selections.indexOf(selection).takeIf { it > 0 } ?: (selections.size / 2)` (`app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt:135`). Note `takeIf { it > 0 }` also rejects index 0, so a selection at index 0 falls back to the middle.
- On confirm, the framework calls `setRemoteScore(track, scoreString)` → `track.score = indexToScore(getScoreList().indexOf(scoreString))` then `updateRemote` (`app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt:90-93`).
- **Mismatch for POINT_3:** `displayScore` returns `"0"` for unscored, but the list contains `"-"` (not `"0"`). `indexOf("0") == -1`, so the wheel opens at the middle (😐, index 2). Confirming without moving stores 60.0.

**Proposal:** Rust score picker stores the selected *index* and maps via the format table directly, avoiding string round-trips. Unscored POINT_3 should open on index 0 (`"-"`, stores 0.0).

---

## 4. Auth and OAuth

### 4.1 Constants

**OBSERVED** `API:570-584`:
- `CLIENT_ID = "5338"`
- `API_URL = "https://graphql.anilist.co/"` (trailing slash)
- `BASE_URL = "https://anilist.co/api/v2/"`
- `BASE_MANGA_URL = "https://anilist.co/manga/"`
- `BASE_ANIME_URL = "https://anilist.co/anime/"`
- `authUrl()` = `"https://anilist.co/api/v2/oauth/authorize"` + query `client_id=5338` then `response_type=token` (built with `Uri.buildUpon().appendQueryParameter`, so the order is `client_id` first).
- Full login URL: `https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token`.
- `mangaUrl(id) = "https://anilist.co/manga/" + id`; `animeUrl(id) = "https://anilist.co/anime/" + id` (used for `tracking_url`, see §8).

### 4.2 Login entry point

**OBSERVED** `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt:171-180`: the AniList row's login action calls `context.openInBrowser(AnilistApi.authUrl(), forceDefaultBrowser = true)`. Logout opens `LogoutDialog(trackerManager.aniList)`. There is no username/password dialog for AniList (unlike Kitsu and MangaUpdates).

### 4.3 Redirect handling

**OBSERVED** `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt:19-31`:
1. The Activity is `exported`, with intent-filter scheme `aniyomi` and host `anilist-auth` (`app/src/main/AndroidManifest.xml:250-257`). Full redirect is expected as `aniyomi://anilist-auth#access_token=<TOKEN>&token_type=Bearer&expires_in=<SECONDS>` (format inferred from the regex; the redirect itself is not in source).
2. Regex (literal): `(?:access_token=)(.*?)(?:&)` applied with `find` to `data.fragment.toString()`.
3. If group 1 matched → `lifecycleScope.launchIO { trackerManager.aniList.login(token); returnToSettings() }`.
4. Else → `trackerManager.aniList.logout(); returnToSettings()` (synchronous on main thread).
5. `returnToSettings()` (`app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt:29-36`) finishes the activity and starts `MainActivity` with `FLAG_ACTIVITY_CLEAR_TOP | FLAG_ACTIVITY_SINGLE_TOP`.

**OBSERVED edge:** the regex requires a `&` after the token. If AniList ever returns the token as the last fragment parameter, group 1 does not match and the user is logged out silently. Proposal: parse the fragment with `url::form_urlencoded` and read `access_token`, which has no trailing-delimiter requirement.

### 4.4 `login(token)`

**OBSERVED** `ANI:344-356`:
```
try {
  oauth = api.createOAuth(token)
  interceptor.setAuth(oauth)
  (username, scoreType) = api.getCurrentUser()
  scorePreference.set(scoreType)
  saveCredentials(username.toString(), oauth.accessToken)
} catch (e: Throwable) {
  logout()
}
```
- Errors are **swallowed**: nothing is thrown or toasted. The caller `TrackLoginActivity` always returns to settings. The user sees the tracker as logged out.
- `login(username, password)` (`ANI:344`) delegates to `login(password)`, i.e. the *password* argument is the access token. Used only through the generic interface; the AniList UI never calls it.
- `saveCredentials(username, password)` stores username = Viewer `id` as decimal string, password = access token (`BaseTracker.kt:49-51` → `TrackPreferences.setCredentials`, which also sets `pref_tracker_auth_expired_2 = false`).

### 4.5 `createOAuth`

**OBSERVED** `API:515-517`: `ALOAuth(accessToken = token, tokenType = "Bearer", expires = System.currentTimeMillis() + 31536000000, expiresIn = 31536000000)`.
- `31536000000` ms = 365 days. The value is **milliseconds** here.
- `expires` is therefore an absolute epoch-millisecond timestamp. `expiresIn` is a duration in ms (not AniList's seconds value; the redirect's `expires_in` is ignored).

### 4.6 Interceptor state and the expiry bug

**OBSERVED** `INT:18-21`: the private setter for `oauth` stores `value?.copy(expires = value.expires * 1000 - 60 * 1000)`, documented as "make the token expire 1 minute before its original expiration date" and "Anilist returns the date without milliseconds" (comment at `INT:13-15`).

Control flow in `intercept` (`INT:23-50`):
1. `token.isNullOrEmpty()` → `throw Exception("Not authenticated with Anilist")`.
2. `if (oauth == null) oauth = anilist.loadOAuth()` (this assignment goes through the setter, so the `*1000` is applied).
3. `if (oauth!!.isExpired())` — **NPE when `oauth` is null** (the null check at `INT:39` is unreachable because `!!` at line 33 already threw). `KotlinNullPointerException` with a null message; the UI formats it as `"NullPointerException: null"` per `ExceptionFormatter` (`app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:35-38`).
4. If expired → `anilist.logout()` then `throw IOException("Token expired")`.
5. Adds headers `Authorization: Bearer <accessToken>` and `User-Agent: Aniyomi v${VERSION_NAME} (${APPLICATION_ID})`.
6. `chain.proceed(authRequest)`.

**Expiry arithmetic (OBSERVED, verified by reading):**
- `isExpired()` = `System.currentTimeMillis() > expires` (`DTO/ALOAuth.kt:17`).
- On login: stored `expires` = now_ms + 31 536 000 000. Setter makes it `(now_ms + 31 536 000 000) * 1000 - 60 000`, roughly 1.8 × 10^15 ms (year ~57 000). It never expires.
- After restart: `loadOAuth()` returns the **stored** value (now_ms + 365 d, un-multiplied), the setter multiplies by 1000 again → same far-future value.
- Therefore the 1-minute-early expiry the comment describes never happens, and `isExpired` is effectively always false. The "Token expired" path is dead in practice. A token revoked remotely is only detected by an HTTP 401 that the code does not special-case (it surfaces as `HttpException(401)`, message `"HTTP error 401"`).

**Proposal:**
- Store the real absolute expiry in **seconds** taken from the redirect's `expires_in` (if present) or fallback 31 536 000 s from login time, and compare in seconds.
- Remove the `*1000` setter entirely.
- Treat HTTP 401 on any authenticated call as "token invalid": clear the token, set `auth_expired` (`pref_tracker_auth_expired_2`), and surface a re-login prompt. (Reuse the existing `trackAuthExpired` flag that MyAnimeList already uses, `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt:269-273`.)
- Make `loadOAuth` absence return "not authenticated" without NPE.

### 4.7 `logout`

**OBSERVED** `ANI:358-362`:
1. `super.logout()` → `trackPreferences.setCredentials(this, "", "")` (username, password emptied; `pref_tracker_auth_expired_2 = false`) (`BaseTracker.kt:28-30`).
2. `trackPreferences.trackToken(this).delete()` (`track_token_2` reset to default `""`).
3. `interceptor.setAuth(null)` → `token = null; oauth = null; anilist.saveOAuth(null)`.
4. `saveOAuth(null)` calls `trackPreferences.trackToken(this).set(json.encodeToString(alOAuth))` (`ANI:364-366`). `alOAuth` is null, so the stored value is the 4-character string `null`, not an empty string. `loadOAuth` then fails to decode and returns null (`ANI:368-374`). Harmless, but the preference is not "deleted" as step 2 suggests.

`isLoggedIn` (`BaseTracker.kt:32-34`) = username non-empty AND password non-empty.

### 4.8 `loadOAuth` / `saveOAuth`

**OBSERVED** `ANI:364-374`: `saveOAuth` writes `json.encodeToString(alOAuth)` into `track_token_2`. `loadOAuth` decodes `ALOAuth` and returns null on any exception.

Serialized keys: `access_token`, `token_type`, `expires` (Long), `expires_in` (Long) (`DTO/ALOAuth.kt:8-14`).

### 4.9 Score-format migration

**OBSERVED** `ANI:68-76`: in `init`, `scorePreference.get()` is called. If the stored value is an int (APIv1), `ClassCastException` is caught → `logout()` and `scorePreference.delete()`. The user is silently logged out once.

---

## 5. Transport and rate limiting

**OBSERVED** `API:40-43`:
- `authClient = client.newBuilder().addInterceptor(interceptor).rateLimit(permits = 85, period = 1.minutes).build()`.
- Every request is `POST` to `API_URL`, body = `payload.toString().toRequestBody(jsonMime)`, `jsonMime = "application/json; charset=utf-8"` (`core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:22`).
- Payload shape: `{"query": "<graphql text>", "variables": {...}}`. Variables are always present for mutations and queries except `getCurrentUser` (no `variables` key at all, `API:532-534`).
- Interceptor order: auth header is added by `INT` first (outer), then the rate limiter (`core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt`).

### 5.1 Rate limiter algorithm (OBSERVED)

`RateLimitInterceptor` (`core/common/.../interceptor/RateLimitInterceptor.kt:58-127`):
- Sliding-window log: a queue of timestamps (`SystemClock.elapsedRealtime`) with capacity `permits`.
- Before sending: while `queue.size >= permits`, remove entries older than `now - period`; if none removed, wait on the queue monitor for `first - periodStart` ms; checks cancellation.
- Appends the current timestamp, then proceeds.
- Fairness: `Semaphore(1, true)`.
- If the response is served from cache (`networkResponse == null`), its timestamp is removed from the queue.
- Scope: per `OkHttpClient` built in `AnilistApi` (one window for all AniList calls of this app instance).
- Limit: **85 requests per any rolling 60 seconds**.

**Proposal:** `tokio::sync::Mutex<VecDeque<Instant>>` with the same rule: capacity 85, window 60 s, wait with `tokio::time::sleep_until(first + 60s)`. Share one limiter across search, find, and mutation calls.

### 5.2 Response handling (OBSERVED)

- `awaitSuccess()` (`core/common/.../OkHttpExtensions.kt:109-117`): if `!response.isSuccessful` → close and throw `HttpException(response.code)` with message `"HTTP error $code"` (`core/common/.../HttpException.kt:13`).
- Mutations `updateLibManga`, `updateLibAnime`, `deleteLibManga`, `deleteLibAnime` call `awaitSuccess()` and **never read the body**. A GraphQL-level `"errors"` array in an HTTP-200 body is silently ignored.
- `addLibManga` / `addLibAnime` and all query functions parse the body with `parseAs<T>()` (kotlinx). A body without the expected `data` path throws a serialization exception. Note `ALAddEntryResult` only declares `data.SaveMediaListEntry.id` (`DTO/ALAddEntry.kt`); a GraphQL `errors` body without `data` fails to parse.
- User-facing text for `HttpException` in the formatter: `MR.strings.exception_http` = `"HTTP %d, check website in WebView"` (`i18n/src/commonMain/moko-resources/base/strings.xml`). Tracker toasts use `e.message` instead, giving `"HTTP error 401"`.

**Proposal:**
- Parse every response into `{ data?, errors? }`. If `errors` is non-empty, return `Err(AniListError::GraphQl { messages })` even on HTTP 200.
- Map HTTP 401 → `AuthExpired`, 429 → `RateLimited` (retry-after honoured if present; behaviour of AniList's 429 response is not in source, so do not assume a header), 5xx → `Transient`, other → `Http(code)`.
- Keep the user-visible message `track_error` pattern (see §12).

---

## 6. Queries and mutations (behaviour; exact text in `p1-data-and-config.md` §4)

| Function | Operation | Used by | Result handling |
|---|---|---|---|
| `addLibManga` | mutation `AddManga` → `SaveMediaListEntry(mediaId, progress, status, private)` returns `{id status}` | `bind` (manga, not found) | Parses `ALAddEntryResult`; returns the **input** track unchanged (`API:73-76`). The new entry `id` is discarded. |
| `addLibAnime` | mutation `AddAnime`, same shape | `bind` (anime, not found) | Same as above (`API:166-169`). |
| `updateLibManga` | mutation `UpdateManga` → `SaveMediaListEntry(id, progress, status, private, scoreRaw, startedAt, completedAt)` returns `{id status progress}` | `update` | Body ignored; returns input track (`API:111-113`). |
| `updateLibAnime` | mutation `UpdateAnime`, same shape | `update` | Body ignored (`API:204-206`). |
| `deleteLibManga` | mutation `DeleteManga` → `DeleteMediaListEntry(id)` returns `{deleted}` | `delete` | Body ignored (`API:133-135`). `listId` = `track.libraryId` (see bug in §10). |
| `deleteLibAnime` | mutation `DeleteAnime`, same | `delete` | Body ignored (`API:226-228`). |
| `search` | query `Search` (manga) `Page(perPage:50) media(search,type:MANGA,format_not_in:[NOVEL])` | `searchManga` | Maps each media via `toALManga().toTrack()`. |
| `searchAnime` | query `Search` (anime) `Page(perPage:50) media(search,type:ANIME)` | `searchAnime` | Maps via `toALAnime().toTrack()`. |
| `findLibManga` | query `(id, manga_id)` `Page{ mediaList(userId,type:MANGA,mediaId) }` | `update` (missing library_id), `bind`, `refresh`, `delete` | `firstOrNull()`. |
| `findLibAnime` | same, `type: ANIME` | same | same |
| `getLibManga` / `getLibAnime` | wraps find | `refresh` | Throws `Exception("Could not find manga")` / `("Could not find anime")` when absent (`API:507-513`). |
| `getCurrentUser` | query `User` `Viewer{id mediaListOptions{scoreFormat}}` | `login` | Returns `Pair(id, scoreFormat)`. |

Note: `search` and `searchAnime` use the same operation name `Search` for both; AniList does not care.

---

## 7. Mapping AniList → local models

### 7.1 Search results (`ALSearchItem` → `*TrackSearch`)

**OBSERVED** `DTO/ALSearchItem.kt:20-44`, `DTO/ALManga.kt:24-47`, `DTO/ALAnime.kt:24-48`:

| AniList field | Local field | Rule |
|---|---|---|
| `id` | `remote_id` | direct |
| `title.userPreferred` | `title` | direct (non-null in DTO) |
| `coverImage.large` | `cover_url` | direct |
| `description` | `summary` | `description?.htmlDecode() ?: ""` (see §9) |
| `format` | `publishing_type` | `format?.replace("_", "-") ?: ""` (e.g. `ONE_SHOT` → `ONE-SHOT`) |
| `status` | `publishing_status` | `status ?: ""` (raw, e.g. `NOT_YET_RELEASED` stays with underscore) |
| `startDate` (fuzzy) | `start_date` | only if `startDateFuzzy != 0L`: formatted `yyyy-MM-dd` in **system default zone** via `SimpleDateFormat(Locale.US)`; `IllegalArgumentException` → `""` |
| `chapters` (manga) / `episodes` (anime) | `total_chapters` / `total_episodes` | `?: 0` |
| `averageScore` | `score` | `averageScore ?: -1` then `.toDouble()` |
| `staff` (manga) | `authors`, `artists` | see §7.3 |
| `studios` (anime) | `authors` | see §7.4 |
| (computed) | `tracking_url` | `https://anilist.co/manga/<id>` or `/anime/<id>` |

Also `AnimeTrackSearch.create(ANILIST)` / `MangaTrackSearch.create(ANILIST)` sets `tracker_id = 2`.

`MangaTrackSearch` equality/hash uses `(manga_id, tracker_id, remote_id)` (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt:127-145`). Search items have `manga_id = 0` at creation.

### 7.2 Fuzzy date → epoch (`ALFuzzyDate.toEpochMilli`)

**OBSERVED** `DTO/ALFuzzyDate.kt:13-20`: `LocalDate.of(year!!, month!!, day!!).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()`; any exception → `0L`.
- A date with year but no month/day → `0L`. A date with year-month-day that does not exist (e.g. Feb 30) → `0L`.
- Time of day is always 00:00 local.

### 7.3 Manga staff

**OBSERVED** `DTO/ALManga.kt:42-46`: for each `staff.edges` element `e`:
- `name = e.node.name() ?: continue` where `name()` = `userPreferred ?: full ?: native` (`DTO/ALSearchItem.kt:80-82`).
- If `"Story" in e.role` (substring match, case-sensitive) → `authors += name` (append, duplicates possible).
- If `"Art" in e.role` → `artists += name`.
- `"Story & Art"` adds to both lists.

### 7.4 Anime studios

**OBSERVED** `DTO/ALAnime.kt:43-47`: `authors = studios.edges.filter { it.isMain }.ifEmpty { studios.edges }.take(3).map { it.node.name }`.

### 7.5 User list entry → local `MangaTrack` / `AnimeTrack`

**OBSERVED** `DTO/ALManga.kt:60-71` and `DTO/ALAnime.kt:61-72`:

| Source | Local field | Rule |
|---|---|---|
| `ALUserListItem.id` | `library_id` | Long, the AniList MediaList entry id |
| `media.id` | `remote_id` | |
| `media.title.userPreferred` | `title` | |
| `status` | `status` | §2.6 |
| `scoreRaw` (already POINT_100 via query) | `score` | `toDouble()` |
| `progress` | `last_chapter_read` (manga) / `last_episode_seen` (anime) | `toDouble()` |
| `startedAt` | `started_reading_date` / `started_watching_date` | `toEpochMilli()` (0 if absent) |
| `completedAt` | `finished_reading_date` / `finished_watching_date` | same |
| `media.chapters` / `media.episodes` | `total_chapters` / `total_episodes` | |
| `private` | `private` | direct |

`tracking_url` is **not** set on user-list tracks (remains `""`). The track's URL is only set on search results. Consequence: a track bound via `refresh` keeps whatever `tracking_url` was stored before; a track created via `bind` retains the search item's URL. (Edge case.)

---

## 8. Track row writes (what gets persisted)

The Anilist tracker itself does not write the DB. The framework does (`AnimeTracker.updateRemote` at `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt:110-120`; `MangaTracker.kt:234-244`): after `update(track)` it calls `track.toDomainTrack(idRequired = false)` and `insertTrack.await(...)`. Insert is `INSERT ... ON CONFLICT REPLACE` keyed on `(anime_id, sync_id)` (SQL in `data/src/main/sqldelightanime/dataanime/anime_sync.sq`), so each insert replaces the local row (new `_id`). Spec for this is in area:27-tracking-core and area:04-db-anime-schema-migrations; AniList-specific consequence: `library_id` returned by `findLibX` must be included in the payload before `insertTrack`, or it is lost on the next replace (see edge case E-11).

---

## 9. Description HTML decode

**OBSERVED** `core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:65-67`: `htmlDecode() = this.parseAsHtml().toString()`, where `parseAsHtml` is `androidx.core.text.parseAsHtml` (Android `Html.fromHtml` legacy mode). Effects: entities decoded, tags stripped, `<br>` becomes `\n`, `<p>`/block boundaries become newlines, leading and trailing whitespace is not trimmed by the function itself (the UI trims, `AnimeTrackerSearch.kt:248` `trackSearch.summary.trim()`).

**Proposal:** Rust: `description.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n")`, then strip tags with `ammonia` (no allowed tags), then `html_escape::decode_html_entities`, then trim. Document the small difference (Android renders `<p>` as blank lines; Rust version keeps `\n\n` after `</p>` if present).

---

## 10. Track operations (per method)

### 10.1 `update(track: MangaTrack, didReadChapter: Boolean)` — `ANI:212-235`

Exact order:
1. If `track.library_id == null || track.library_id == 0L`:
   - `libManga = api.findLibManga(track, getUsername().toInt())`; if null → `throw Exception("$track not found on user library")`. `$track` calls `toString()` on `MangaTrackImpl`, which has no override (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrackImpl.kt`), so the user-visible text is a JVM object-identity string like `eu.kanade.tachiyomi.data.database.models.manga.MangaTrackImpl@1a2b3c`. Observed; proposal: never show this text.
   - `track.library_id = libManga.library_id`.
2. If `track.status != COMPLETED`:
   - If `didReadChapter`:
     - If `track.last_chapter_read.toLong() == track.total_chapters && track.total_chapters > 0`:
       - `status = COMPLETED`; `finished_reading_date = System.currentTimeMillis()`.
     - Else if `track.status != REREADING`:
       - `status = READING`.
       - If `track.last_chapter_read == 1.0` → `started_reading_date = System.currentTimeMillis()`.
3. `return api.updateLibManga(track)`.

Consequences (OBSERVED):
- When status is REREADING and a chapter is read (not the last one), status stays REREADING and started date is not touched.
- `started_reading_date` is set to now every time progress becomes exactly 1.0 with didRead, even if a start date already existed.
- Status ON_HOLD, DROPPED, PLAN_TO_READ are overwritten to READING by a read event. Same for anime (ON_HOLD/DROPPED/PLAN_TO_WATCH → WATCHING).

### 10.2 `update(track: AnimeTrack, didWatchEpisode: Boolean)` — `ANI:237-260`

Identical structure with: `library_id` resolved by `findLibAnime`; `last_episode_seen.toLong() == total_episodes`; `started_watching_date`/`finished_watching_date`; statuses `WATCHING`, `REWATCHING`, `COMPLETED`; `last_episode_seen == 1.0` for start date.

Note: the `didWatchEpisode` flag is passed as `true` from `TrackEpisode` (`app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt:42`) and `ExternalIntents` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:546`). The generic `setRemote*` methods call `update(track)` with the default `false`, so status auto-transitions do not run for manual changes; the framework applies its own transitions (§11).

### 10.3 `bind(track, hasReadChapters)` — `ANI:280-318` (manga) / `ANI:300-318` (anime)

1. `remoteTrack = api.findLibManga(track, getUsername().toInt())`.
2. If found:
   - `track.copyPersonalFrom(remoteTrack, copyRemotePrivate = false)`: copies `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date` from remote; **keeps the local `private`** (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:37-44`).
   - `track.library_id = remoteTrack.library_id`.
   - If `track.status != COMPLETED`:
     - `isRereading = status == REREADING` (anime: `REWATCHING`).
     - `status = if (!isRereading && hasReadChapters) READING else status` (anime: `WATCHING`).
   - `return update(track)` (no didRead flag, so §10.1 step 2 does not run; step 1 is skipped because `library_id` is now set).
3. Else:
   - `status = if (hasReadChapters) READING else PLAN_TO_READ` (anime: `WATCHING` / `PLAN_TO_WATCH`).
   - `score = 0.0`.
   - `return add(track)` → `api.addLibManga(track)` → sends `progress = last_chapter_read.toInt()` (the search track's value, normally 0).

Notes:
- `bind` overwrites local progress with remote progress even if local is ahead. The interactor (`app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt`, area:27) pushes the local latest-read value afterwards via `setRemoteLastChapterRead` if it is higher; AniList contributes nothing extra.
- Not-found path: the new entry `id` returned by `SaveMediaListEntry` is discarded (`API:73-76`), so `library_id` stays whatever the search track had (`null`). Next `update` re-resolves it (one extra query).
- `start_date` / `finish_date` are not sent on add.

### 10.4 `refresh(track)` — `ANI:328-342`

1. `remoteTrack = api.getLibManga(track, getUsername().toInt())` → throws `"Could not find manga"` if the entry is no longer on the user list.
2. `track.copyPersonalFrom(remoteTrack)` (DB version, `copyRemotePrivate` default **true**): copies progress, score, status, dates, and private.
3. `track.title = remoteTrack.title`; `track.total_chapters = remoteTrack.total_chapters`.
4. Returns `track`. `library_id` and `remote_id` are not changed.

Anime: identical with `"Could not find anime"`, `total_episodes`.

### 10.5 `delete(track)` — `ANI:262-278`

Manga:
```
if (track.libraryId == null || track.libraryId == 0L) {
    val libManga = api.findLibManga(track.toDbTrack(), user) ?: return
    return api.deleteLibManga(track.copy(id = libManga.library_id!!))   // BUG
}
api.deleteLibManga(track)
```
- **BUG (OBSERVED):** `track.copy(id = ...)` writes the *local row id* field, not `libraryId`. `deleteLibManga` reads `track.libraryId` (still null/0), so the mutation is sent with `listId: null`. The remote entry is not deleted in this branch. Errors are logged by the caller (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt` `deleteMangaFromService` catches and logs at ERROR, no toast).
- Anime identical bug at `ANI:271-278`.
- If the find returns null → silent `return` (nothing deleted remotely, no error).

**Proposal:** `track.copy(library_id = libManga.library_id)` before calling delete; assert `library_id` non-null before the mutation.

### 10.6 Local-remote status helper flows used by the framework (for reference)

- `setRemoteAnimeStatus` / `setRemoteMangaStatus`: sets status; if status == COMPLETED and total != 0 → progress = total; then `updateRemote`.
- `setRemoteLastEpisodeSeen` / `setRemoteLastChapterRead`: if progress was 0 and new > 0 and status != rereading/rewatching → status = WATCHING/READING; progress = n; if total != 0 and progress == total → COMPLETED + finished date = now; then `updateRemote`.
- `setRemoteScore`: `score = indexToScore(indexOf(string))`.
- `setRemoteStartDate` / `setRemoteFinishDate`: sets the date and `updateRemote`. Date 0 removes the date (UI: `TrackDateRemoverScreen`, `AnimeTrackInfoDialog.kt:659-675`).
- `setRemotePrivate`: sets `private`, `updateRemote`.
- Exact sources: `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt:67-108` and `MangaTracker.kt:189-232`.

Note: `setRemoteLastEpisodeSeen` sets COMPLETED when progress equals total, **even when status was REWATCHING**, because that branch does not check the rewatching status (only the start check does).

---

## 11. Progress-driven flows (who calls what, with AniList impact)

| Trigger | Code path | Calls into Anilist |
|---|---|---|
| Player marks episode seen (auto-track on) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1947-1957` → `TrackEpisode.await` (`anime/interactor/TrackEpisode.kt:172-213`) | `refresh` then `update(track, true)`; delayed queue on offline |
| External player episode seen | `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:524-560` | `update(track, true)` **without** `refresh` first (divergence); delayed queue on offline |
| Mark-as-seen in entry screen | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:868-905` | With `ALWAYS`: `TrackEpisode.await` + toast `AYMR.trackers_updated_summary_anime`. With `ASK`: snackbar `AYMR.confirm_tracker_update_anime`, action `MR.action_ok`. With `NEVER` nothing. |
| Manga read | `TrackChapter.await` (`manga/interactor/TrackChapter.kt`) | `refresh` then `update(track, true)`; queue on **any** exception |
| Add to library | `AnimeScreenModel.kt:450` (`bindEnhancedTrackers`, enhanced only) and register flow (`AnimeTracker.register` → `AddAnimeTracks.bind`) | `bind` |
| Refresh on screen open | `AnimeTrackInfoDialog.kt:240-260` → `RefreshAnimeTracks.await` | `refresh` for each logged-in tracker |

**OBSERVED** `TrackEpisode.await` skip condition: `episodeNumber <= track.lastEpisodeSeen` → skip (`TrackEpisode.kt:186`). Manga equivalent `TrackChapter.kt:31-35` (duplicated nested if, same logic).

**OBSERVED** `TrackEpisode` offline path: `context.isOnline()` false → `delayedTrackingStore.addAnime(track.id, episodeNumber)` and `DelayedAnimeTrackingUpdateJob.setupTask(context)` (`TrackEpisode.kt:199-204`). API failure (online) → `runCatching` swallows, **no queueing** (anime). Manga queues on any exception (`TrackChapter.kt:46-51`). Inconsistency noted; see edge cases.

**Auto-track settings** (keys in §12 of data doc): `pref_auto_update_manga_sync_key` (`autoUpdateTrack`, default true), `pref_auto_update_manga_on_mark_read` (`autoUpdateTrackOnMarkRead`, enum `AutoTrackState`, default `ALWAYS`).

---

## 12. User-visible strings (keys, English text)

Tracker-related keys used by the AniList path (values from `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`):

| Key | Text | Where used |
|---|---|---|
| `MR.strings.reading` | Reading | manga status label |
| `MR.strings.plan_to_read` | Plan to read | manga status label |
| `MR.strings.completed` | Completed | both status labels |
| `MR.strings.repeating` | Rereading | manga REREADING label |
| `MR.strings.paused` | Paused | ON_HOLD label |
| `MR.strings.dropped` | Dropped | DROPPED label |
| `AYMR.strings.watching` | Watching | anime |
| `AYMR.strings.plan_to_watch` | Plan to watch | anime |
| `AYMR.strings.repeating_anime` | Rewatching | anime REWATCHING |
| `MR.strings.login_title` | Log in to %1$s | generic login dialog (not AniList) |
| `MR.strings.login_success` | Logged in | generic (not AniList) |
| `MR.strings.logout_title` | Log out from %1$s? | AniList logout dialog |
| `MR.strings.logout_success` | You are now logged out | AniList logout confirm toast |
| `MR.strings.track_error` | %1$s error: %2$s | refresh failures on info dialog open (`AnimeTrackInfoDialog.kt:250-257`) |
| `MR.strings.track_delete_title` | Remove %s tracking? | remove dialog title |
| `MR.strings.track_delete_text` | This will remove the tracking locally. | remove dialog body |
| `MR.strings.track_delete_remote_text` | Also remove from %s | checkbox, deletable only |
| `MR.strings.error_no_match` | No match found | enhanced only |
| `MR.strings.track_started_reading_date` | Start date | date picker title |
| `MR.strings.track_finished_reading_date` | Finish date | date picker title |
| `MR.strings.track_remove_date_conf_title` | Remove date? | |
| `MR.strings.track_remove_start_date_conf_text` | This will remove your previously selected start date from %s | |
| `MR.strings.track_remove_finish_date_conf_text` | This will remove your previously selected finish date from %s | |
| `MR.strings.track_type` | Type | search result row |
| `MR.strings.label_started` | Started | search result row |
| `MR.strings.track_status` | Status | search result row, status selector title |
| `MR.strings.score` | Score | score selector title, search result row |
| `MR.strings.action_track` | Track | search bottom bar |
| `MR.strings.action_toggle_private_on` | Track privately | search private button |
| `MR.strings.action_search_hint` | Search… | search field hint |
| `MR.strings.action_open_in_browser` | Open in browser | search result menu |
| `MR.strings.action_copy_to_clipboard` | Copy to clipboard | search result menu |
| `MR.strings.no_results_found` | No results found | empty search |
| `MR.strings.unknown_error` | Unknown error | search error fallback |
| `MR.strings.tracking_info` | One-way sync to update the chapter progress in external tracker services. Set up tracking for individual entries from their tracking button. | settings |
| `MR.strings.tracking_guide` | Tracking guide | settings app bar |
| `MR.strings.services` | Trackers | settings group title |
| `AYMR.strings.pref_auto_update_manga_sync` | Update progress after reading/watching | settings |
| `AYMR.strings.pref_track_on_add_library` | Open track menu on adding to library | settings |
| `AYMR.strings.pref_show_next_episode_airing_time` | Show next episode's airing time | settings |
| `AYMR.strings.pref_auto_update_manga_on_mark_read` | Update progress when marked as read/watched | settings |
| `AYMR.strings.trackers_updated_summary_anime` | Trackers updated to episode %d | toast after ALWAYS |
| `AYMR.strings.confirm_tracker_update_anime` | Update trackers to episode %d? | snackbar on ASK |
| `MR.strings.lock_always` / `lock_never` / `default_category_summary` | Always / Never / Always ask | `AutoTrackState` labels |
| `MR.strings.exception_http` | HTTP %d, check website in WebView | `ExceptionFormatter` for HttpException |
| `MR.strings.exception_offline` | No Internet connection | formatter, unknown host while offline |
| `MR.strings.exception_unknown_host` | Couldn't reach %s | formatter |

Hard-coded (non-localized) strings from AniList code: `"Not authenticated with Anilist"`, `"Token expired"`, `"No authentication token"`, `"Unknown status: $status"`, `"Unknown score type"`, `"Could not find manga"`, `"Could not find anime"`, `"$track not found on user library"`. These reach the user through toasts where `e.message` is shown (`app/src/main/java/eu/kanade/tachiyomi/data/track/anime/...` `Injekt.get<Application>().toast(e.message)` in `AnimeTracker.kt:118`). Proposal: localize them with new keys `anilist_err_not_authenticated`, `anilist_err_token_expired`, `anilist_err_not_in_list`.

---

## 13. Search behaviour

- Query text from the search field; initial query = `it.track?.title ?: animeTitle` (`AnimeTrackInfoDialog.kt:162`). Search runs automatically on screen open if initial query not blank (`AnimeTrackInfoDialog.kt:727-732`).
- `trackingSearch(query)`: sets `queryResult = null` (loading), then `withIOContext { searchAnime(query) }` wrapped in `Result` (`AnimeTrackInfoDialog.kt:734-753`).
- Preselection: the item whose `tracking_url == currentUrl` is pre-selected (`AnimeTrackInfoDialog.kt:750`).
- Result list: `items(key = { it.hashCode() })`; hashCode uses `(anime_id, tracker_id, remote_id)`.
- Empty result → `"No results found"`. Error → shows `Throwable.message` (or `"Unknown error"` if null) (`AnimeTrackerSearch.kt:226-231`).
- No pagination: `Page(perPage: 50)`, first page only.
- Confirm: `onConfirmSelection(private)` sets `selected.private = private` then `registerTracking(selected)` → `tracker.animeService.register(item, animeId)` (`AnimeTrackInfoDialog.kt:707-711,756-758`).

Search row rendering (`AnimeTrackerSearch.kt:237-356`):
- Title (max 2 lines, ellipsis); authors + artists joined `distinct()` by `", "` (1 line).
- `type` = `publishing_type.lowercase().capitalize()` (e.g. "Manga", "One-shot"). Row shown if not blank.
- `start_date` row "Started" if not blank.
- `status` = `publishing_status.lowercase().capitalize()` (e.g. "Finished", "Not_yet_released" — underscores kept). Row "Status" if not blank.
- Score row shown only if `score != -1.0`, text = `score.toString()` (Double, e.g. "72.0").
- Summary = `summary.trim()`, max 4 lines.
- Long-press opens menu: Copy to clipboard (title), Open in browser (`tracking_url`, if not blank).
- Selected item: 2 dp outline, check icon top-end. Bottom bar appears when selected: "Track" button and (if `supportsPrivateTracking`) a "Track privately" icon button (`VisibilityOff`).

---

## 14. Info dialog behaviour (AniList instance)

Driven by `AnimeTrackInfoDialogHome` (`app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt`):
- Score row: `displayScore(track)` only when `scoreList` non-empty (always true for AniList) **and** `track.score != 0.0`. Unscored → no row content but the row's click target remains (`onScoreClick` always set).
- Episodes row: `"${lastEpisodeSeen.toInt()}"`, plus `" / $totalEpisodes"` if total > 0.
- Start date: shown if `supportsReadingDates && startDate != 0L`; click always opens date picker.
- End date: shown if `supportsReadingDates && finishDate != 0L`.
- Private toggle: shown if `supportsPrivateTracking`; `togglePrivate` calls `setRemotePrivate(!private)` (`AnimeTrackInfoDialog.kt:262-266`).
- "Copy link" and "Open in browser" use `remoteUrl` (`AnimeTrackInfoDialog.kt:187-199`), skipped when blank.
- Remove: `TrackerAnimeRemoveScreen` → `unregisterTracking` (local delete) and, if the checkbox is ticked and tracker deletable, `delete(track)` (`AnimeTrackInfoDialog.kt:849-873`).

Episode picker (`TrackEpisodeSelectorScreen`, `AnimeTrackInfoDialog.kt:371-402`):
- Range: `0..total` if `total > 0`, else `0..10000`.
- Initial = `lastEpisodeSeen.toInt()`.
- Confirm → `setRemoteLastEpisodeSeen(track, selection)`.

Date picker (`TrackDateSelectorScreen`, `AnimeTrackInfoDialog.kt:459-588`):
- Initial selection = stored date (converted to UTC millis), or now if 0.
- Future dates disallowed. Start date must be ≤ finish date (if finish set); finish must be ≥ start (if start set) — year-level checks in `isSelectableYear`.
- Confirm → `setDate(millis)`: converts UTC millis back to local (`convertEpochMillisZone(UTC, systemDefault)`) and calls `setRemoteStartDate`/`setRemoteFinishDate` (local millis at 00:00 of the picked day in system zone, because DatePicker gives midnight UTC).
- Remove (only if date set) → confirm dialog → `setRemote*Date(…, 0)`.

Status picker (`TrackStatusSelectorScreen`, `AnimeTrackInfoDialog.kt:287-340`): initial selection = `track.status`; confirm → `setRemoteAnimeStatus`.

---

## 15. Cross-area call sites (behaviour that depends on AniList)

### 15.1 AniSkip (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2031-2054`)
- For each track of the anime: `is Anilist -> AniSkipApi().getMalIdFromAL(track.remoteId)`.
- `getMalIdFromAL(id)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt:46-65`) sends a raw unparameterised query `query{ Media(id:$id){idMal} }` to `https://graphql.anilist.co` via a plain `OkHttpClient()` (no auth, no rate limit), then extracts the number after `idMal":` with string search. Returns `0` on exception or unparsable.
- Note: `PlayerViewModel.kt:2041-2053` has a bug: the `return` inside `map` returns from `aniSkipResponse` after the first track regardless; the function ends `return null` anyway. Owned by the player area.

### 15.2 AniChart airing data (`app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt`)
- `loadAiringTime(anime, trackItems, manualFetch)`: if anime status is COMPLETED and not manual → returns stored values. Otherwise picks the first track item that is Anilist (or MAL/Simkl) with a non-null track; none → `Pair(1, 0L)`.
- Anilist: `getAnilistAiringEpisodeData(remoteId)`: query `query { Media(id:$id) { nextAiringEpisode { episode airingAt } } }` to `https://graphql.anilist.co` with plain `OkHttpClient()`; episode parsed by substring (`"episode\":"` up to `,`), airingAt by substring up to `}`. Missing → episode 1, airingAt 0.
- MAL: `getAlIdFromMal(idMal)` query `Media(idMal:$idMal,type: ANIME){ id }` then the same airing query.
- Simkl path is out of scope.
- Settings key `show_next_episode_airing_time` (default true) is the UI gate (`TrackPreferences.kt:43-46`).

### 15.3 Library and stats
- `get10PointScore` used by `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt:272` and `manga/MangaLibraryScreenModel.kt:269` (mean score), and stats screens (`AnimeStatsScreenModel.kt:156-164`, `MangaStatsScreenModel.kt:141-149`). Score scale is therefore the 100-point stored value /10 for all formats.

### 15.4 Backup
- Track rows (including `library_id`, `remote_url`, `private`, dates) are serialised by `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupTracking.kt` and `BackupAnimeTracking.kt` (area:21/22). AniList-specific: `library_id` must round-trip, otherwise the first update re-queries.

---

## 16. Concurrency and dispatch

- Every AniList network call is `withIOContext { }` (`ANI`/`API`), i.e. `Dispatchers.IO`.
- Track interactors use `withNonCancellableContext` and `async`/`awaitAll` per tracker (`TrackEpisode.kt`, `RefreshAnimeTracks.kt` with `supervisorScope`).
- Login from the Activity runs on `lifecycleScope.launchIO`.
- The rate limiter is `synchronized` + `Semaphore(1, true)`. Blocking happens on the calling IO thread.
- `AnilistInterceptor` fields (`token`, `oauth`) are plain `var`s without synchronisation; concurrent refreshes from several `async` calls may race on `oauth` lazy-load. Benign because loads are idempotent.

**Proposal (Rust):** one `AnilistClient` owned by a tokio runtime; `Arc<RwLock<Option<AuthState>>>` for the token; a single `RateLimiter` (Mutex<VecDeque<Instant>>). The GTK main thread never awaits these; results are delivered with `glib::MainContext::channel` / `async_channel` to the UI.

---

## 17. Timings and limits (complete list)

| Name | Value | Source |
|---|---|---|
| Rate limit | 85 requests / 60 s rolling | `API:42` |
| Token lifetime assumed at login | 31 536 000 000 ms (365 d) | `API:516` |
| Interceptor early-expiry margin (intended, not effective) | 60 000 ms, applied as `*1000` bug | `INT:20` |
| Search page size | `perPage: 50` | `API:236,297` |
| Anime episode picker upper bound when total unknown | 10 000 | `AnimeTrackInfoDialog.kt:380` |
| Author list cap (anime studios) | 3 | `DTO/ALAnime.kt:46` |
| Search summary max lines | 4 | `AnimeTrackerSearch.kt:349` |
| Search title max lines | 2 | `AnimeTrackerSearch.kt:293` |
| Delayed queue retry (framework) | `BackoffPolicy.EXPONENTIAL`, 5 min; `runAttemptCount > 3` → failure | `app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt:27,72` |
| AniSkip/AniChart request timeout | OkHttp default (no explicit timeouts) | `AniSkipApi.kt`, `AniChartApi.kt` |
| Auth token wrong-expiry guard | `isExpired` = now > expires | `DTO/ALOAuth.kt:17` |

---

## 18. Message formatting for toast-visible errors

`TrackerManager`-driven toasts use `e.message` directly (`AnimeTracker.kt:118`, `MangaTracker.kt:242`). Consequences:
- `HttpException(401)` → `"HTTP error 401"`.
- `IOException("Token expired")` → `"Token expired"`.
- `Exception("Not authenticated with Anilist")`.
- `NullPointerException` from `oauth!!` → message `null` → toast shows `"null"`.
- `NumberFormatException` from `getUsername().toInt()` when username is not numeric (e.g. never logged in properly) → `For input string: ""` (empty username). Only reachable when `isLoggedIn` was bypassed; in normal flows `isLoggedIn` is false and the tracker is not called.

Proposal: map each to a localized key (see §12 proposal).
