# Area 32: MangaUpdates + Simkl trackers, part 1 - behaviour spec

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Line numbers are `path:line` into the files in scope unless marked "external". Labels: **OBSERVED** = what the Kotlin does; **PROPOSAL** = what the Rust port should do. Shorthand: MU = MangaUpdates, SK = Simkl.

Contents: 1 shared tracker contract; 2 MangaUpdates; 3 Simkl; 4 message keys and literal strings; 5 side effects; 6 concurrency and timings; 7 interactions with other areas; 8 observed defects (with proposals); 9 dead code and unused declarations; 10 Android-to-Linux notes that change behaviour.

---

## 1. Shared tracker contract (applies to both services)

### 1.1 Registry and ids
**OBSERVED**
- `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:17-42`. Constants `ANILIST = 2L`, `KITSU = 3L`, `KAVITA = 8L`, `SIMKL = 101L`, `JELLYFIN = 102L`. Instances: `mangaUpdates = MangaUpdates(7L)` (line 33), `simkl = Simkl(SIMKL)` (line 36). The `trackers` list (lines 39-42) contains both; `get(id)` is `trackers.find { it.id == id }`.
- `loggedInTrackers()` filters on `isLoggedIn`; `loggedInTrackersFlow()` combines each tracker's `isLoggedInFlow`.

**PROPOSAL**
- `TrackerId::MANGAUPDATES = 7`, `TrackerId::SIMKL = 101` (as in `plans/27-tracking-core/p1-rust-design.md` section 3.1). The numbers are stored in `sync_id` columns and must not change.

### 1.2 Credentials and login state (BaseTracker)
**OBSERVED** (`app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`)
- Lines 11-14: `abstract class BaseTracker(override val id: Long, override val name: String) : Tracker`.
- Lines 16-17: `trackPreferences` and `networkService` via injekt.
- Lines 19-20: `client` = `networkService.client` (the shared OkHttp client).
- Lines 23, 25: `supportsReadingDates = false`, `supportsPrivateTracking = false`.
- Lines 28-30: `logout()` = `trackPreferences.setCredentials(this, "", "")`. Note `@CallSuper`.
- Lines 32-34: `isLoggedIn` = `getUsername().isNotEmpty() && getPassword().isNotEmpty()`.
- Lines 36-43: `isLoggedInFlow` = combine of username and password `changes()` flows, same predicate.
- Lines 45-51: `getUsername`, `getPassword`, `saveCredentials(u, p)` = `setCredentials(this, u, p)`.

`TrackPreferences.setCredentials` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:29-33`) writes username, password and then `trackAuthExpired = false`. Keys (see data doc section 2): `pref_mangasync_username_<id>`, `pref_mangasync_password_<id>`, `pref_tracker_auth_expired_<id>`.

**OBSERVED consequence for MU and SK**: a tracker is logged in iff both username and password strings are non-empty. MU stores `uid` as the username and the session token as the password (section 2.5). SK stores `user.id` (as a decimal string) as the username and the OAuth access token as the password (section 3.6).

**Note**: `trackAuthExpired` is not read or written by MU or SK (it is used only by MyAnimeList, `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt:269-273`). Neither MU nor SK ever sets the auth-expired flag, even on HTTP 401.

### 1.3 Generic remote-setter flow (MangaTracker and AnimeTracker, external)
These default methods call the per-service `update(...)`. They are the only callers of MU/SK `update` apart from bind. External files: `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt` and `AnimeTracker.kt`.

- `setRemoteMangaStatus(track, status)` (MangaTracker.kt:66-72): sets `status`; if `status == getCompletionStatus() && total_chapters != 0` sets `last_chapter_read = total_chapters`; then `updateRemote`.
- `setRemoteLastChapterRead(track, n)` (74-89): if `last_chapter_read == 0.0 && last_chapter_read < n && status != getRereadingStatus()` then `status = getReadingStatus()`. Then `last_chapter_read = n`. If `total_chapters != 0 && last_chapter_read.toLong() == total_chapters` then `status = getCompletionStatus()` and `finished_reading_date = now`. Then `updateRemote`.
- `setRemoteScore(track, scoreString)` (91-94): `score = indexToScore(getScoreList().indexOf(scoreString))` then `updateRemote`. Note: `indexOf` returns -1 for an unknown string, and then `indexToScore(-1)` is called (see defect D-MU-7).
- `updateRemote(track)` (111-121): calls `update(track)`, then `insertTrack` (domain insert with `idRequired=false`), catching `Exception`, logging `"Failed to update remote track data id=<id>"` and toasting `e.message`.
- Anime equivalents are in `AnimeTracker.kt:67-120` with `episode` names. Same shape, `total_episodes`, `last_episode_seen`, `finished_watching_date`, `getRewatchingStatus()`.

Anime `AnimeTracker.register` (AnimeTracker.kt:58-65) and manga `MangaTracker.register` (MangaTracker.kt:57-64) set the foreign key then call `AddAnimeTracks.bind` / `AddMangaTracks.bind`; on any `Throwable` they toast `e.message`.

### 1.4 Bind flow that calls the services (external)
`app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt:32-79` (anime; manga is the same shape):
1. `allChapters = getEpisodesByAnimeId.await(animeId)`; `hasSeenEpisodes = allChapters.any { it.seen }`.
2. `tracker.bind(item, hasSeenEpisodes)`.
3. `track = item.toDomainTrack(idRequired = false) ?: return`; `insertTrack.await(track)`.
4. If `hasSeenEpisodes`: `latest = allChapters.sortedBy { episodeNumber }.takeWhile { seen }.lastOrNull()?.episodeNumber ?: -1.0`. If `latest > track.lastEpisodeSeen` then set and `tracker.setRemoteLastEpisodeSeen(track.toDbTrack(), latest.toInt())`. If `track.startDate <= 0`, take earliest history `seenAt` and `setRemoteStartDate(..., startDate)` where startDate is converted from system zone to UTC.
5. `syncChapterProgressWithTrack.await(animeId, track, tracker)`.

Important for the service specs: the search result object (`AnimeTrackSearch`, `MangaTrackSearch`) is passed directly as `item` to `bind`. Its `tracking_url` is therefore the URL straight from the search response (see defect D-SK-2).

### 1.5 Search-result model defaults (external)
`app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt`: `score = -1.0` (line 25), `status = 0`, `total_episodes = 0`, `last_episode_seen = 0.0`, `remote_id = 0`, `tracking_url` lateinit (set by the service). The manga search model mirrors this. A search result therefore has `score = -1.0` until the service bind sets it.

---

## 2. MangaUpdates (MU)

### 2.1 Constants
**OBSERVED** (`MangaUpdates.kt:22-27`):
| Name | Value | Meaning in MU API (list_id) |
|---|---|---|
| `READING_LIST` | `0L` | Reading |
| `WISH_LIST` | `1L` | Wish |
| `COMPLETE_LIST` | `2L` | Complete |
| `UNFINISHED_LIST` | `3L` | Unfinished |
| `ON_HOLD_LIST` | `4L` | On hold |
| rereading | `-1` (`getRereadingStatus`, line 65) | none; never sent |
| completion | `COMPLETE_LIST` (2) (`getCompletionStatus`, line 67) | |
| reading | `READING_LIST` (0) (`getReadingStatus`, line 63) | |

`getStatusListManga()` (lines 50-52) returns exactly `[READING_LIST, COMPLETE_LIST, ON_HOLD_LIST, UNFINISHED_LIST, WISH_LIST]` = `[0, 2, 4, 3, 1]` in that order. This order drives the status picker.

`getStatusForManga(status)` (lines 54-61): `0 -> MR.strings.reading_list`, `1 -> MR.strings.wish_list`, `2 -> MR.strings.complete_list`, `4 -> MR.strings.on_hold_list`, `3 -> MR.strings.unfinished_list`, else `null`.

Logo: `R.drawable.ic_manga_updates` (line 46). Logo colour `Color.rgb(146, 160, 173)` (line 48) = `0xFF92A0AD`.

### 2.2 Score list and index mapping
**OBSERVED** (`MangaUpdates.kt:29-39`). `SCORE_LIST = (0..10).flatMap { decimal -> when(decimal) { 0 -> ["-"]; 10 -> ["10.0"]; else -> (0..9).map { "$decimal.$fraction" } } }`.
Resulting list, exact, length 92:
- index 0: `"-"`
- index 1..90: `"1.0"`, `"1.1"`, ... `"1.9"`, `"2.0"`, ... `"9.9"` (ten per integer part 1 to 9)
- index 91: `"10.0"`

So `"0.x"` values do not exist in the list, and the only zero is the `"-"` entry.

`indexToScore(index)` (line 71): `if (index == 0) 0.0 else SCORE_LIST[index].toDouble()`. Examples: index 1 gives `1.0`, index 2 gives `1.1`, index 11 gives `2.0`, index 91 gives `10.0`.

`get10PointScore` is the default (identity, MangaTracker.kt:43-45). `displayScore(track) = track.score.toString()` (line 73), so 0.0 displays as `"0.0"`, not `"-"`, and 7.5 displays as `"7.5"`.

`getScoreList()` returns the 92-element immutable list (line 69).

### 2.3 Status-change rule on read progress
**OBSERVED** `update(track, didReadChapter)` (lines 75-81): `if (track.status != COMPLETE_LIST && didReadChapter) track.status = READING_LIST`, then `api.updateSeriesListItem(track)`, return track. So completed series keep status 2 on every progress update; any other status becomes Reading when a chapter is read through the update path.

### 2.4 HTTP surface (see data doc section 4 for full request and response shapes)
All MU requests go through `authClient` (adds Bearer) except `search` and `authenticate`, which use the bare `client`. `getSeriesRating` and `updateSeriesRating` use `authClient`.

Endpoints used, in call order by flow:
- GET `/v1/lists/series/{remote_id}` (authClient) - `getSeriesListItem` line 45.
- GET `/v1/series/{remote_id}/rating` (authClient) - `getSeriesRating` line 119.
- POST `/v1/lists/series` (authClient) - `addSeriesToList` line 65-70.
- POST `/v1/lists/series/update` (authClient) - `updateSeriesListItem` line 92-97.
- PUT `/v1/series/{remote_id}/rating` (authClient) - `updateSeriesRating` line 134-139.
- DELETE `/v1/series/{remote_id}/rating` (authClient) - line 142-145.
- POST `/v1/lists/series/delete` (authClient) - `deleteSeriesFromList` line 107-112.
- POST `/v1/series/search` (client, no auth) - `search` line 162-167.
- PUT `/v1/account/login` (client, no auth) - `authenticate` line 181-186.

Base URL: `https://api.mangaupdates.com` (line 194). Content type for bodies: `application/vnd.api+json` with no charset (line 195).

Every call uses `awaitSuccess()` (which throws `HttpException(code)` for any non-2xx, `core/common/.../OkHttpExtensions.kt:109-117`), except where a `try` swallows it (see 2.7).

### 2.5 Login (`login(username, password)`, MangaUpdates.kt:115-121)
**OBSERVED** step by step:
1. `api.authenticate(username, password)`: PUT `/v1/account/login` with JSON object `{"username": <u>, "password": <p>}`. Parse `MULoginResponse` and return its `context` (`MUContext`).
2. If the returned value is null, throw `Throwable("Unable to login")`. This branch is unreachable in practice: `MULoginResponse.context` is non-nullable (`dto/MULoginResponse.kt:6-7`), so a missing `context` fails inside parsing with a serialisation error instead.
3. `saveCredentials(authenticated.uid.toString(), authenticated.sessionToken)`: username becomes the numeric uid as a string, password becomes the session token. The typed username and password are NOT stored.
4. `interceptor.newAuth(sessionToken)` sets the in-memory token.

Dialog (external, SettingsTrackingScreen.kt:349-364): on success toast `MR.strings.login_success` and close. On any `Throwable`: call `tracker.logout()`, toast `e.message.toString()` (so a null message renders as the text `"null"`), keep the dialog open with `inputError = true`.

**PROPOSAL**: the port keeps the same storage contract (uid as username, session token as password) so that existing preference values remain valid, but moves the session token into the secret store (section 5 of the tracking-core design). Do not show the uid to the user as a login name; the Kotlin shows it in no UI either (only `isLoggedIn` reads these values).

### 2.6 Session restore and the interceptor (MangaUpdatesInterceptor.kt)
**OBSERVED**
- Line 12: `private var token: String? = mangaUpdates.restoreSession()`. `restoreSession()` (MangaUpdates.kt:123-125) returns `trackPassword(this).get().ifBlank { null }`. The interceptor reads it once, when the lazy `interceptor` is first accessed (MangaUpdates.kt:42), which is the first authenticated call after app start.
- `intercept` (lines 14-26): `token ?: throw IOException("Not authenticated with MangaUpdates")`. Then adds `Authorization: Bearer <token>` (line 21) and `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)` (line 22; value `Aniyomi v0.18.2.1 (xyz.jmir.tachiyomi.mi)` for this build, per `app/build.gradle.kts`). The header replaces any existing User-Agent (`.header`, not `.addHeader`).
- `newAuth(token)` (lines 28-30) sets the in-memory field only. It does not persist.

**OBSERVED defect**: `MangaUpdates.logout()` is not overridden, so it runs `BaseTracker.logout()` (credentials cleared) and never calls `interceptor.newAuth(null)`. The old token stays in memory until the process restarts, so authenticated calls keep working after logout in the same session. (Compare Simkl, section 3.7, which does clear it.) See D-MU-1.

**PROPOSAL**: the Rust `MangaUpdatesAuth` holds the token in an `RwLock<Option<String>>`; `logout()` clears it. Missing token returns `TrackError::NotAuthenticated("MangaUpdates")` with the same text as the Kotlin message.

### 2.7 Bind (`bind(track, hasReadChapters)`, MangaUpdates.kt:87-96)
**OBSERVED**
```
try {
  (series, rating) = api.getSeriesListItem(track)
  track.copyFrom(series, rating)           // status, last_chapter_read, score
} catch (Exception) {
  track.score = 0.0
  api.addSeriesToList(track, hasReadChapters)
  track
}
```
- `getSeriesListItem` (MangaUpdatesApi.kt:43-53) fetches the list item (GET list item, which throws for a missing series) and then `getSeriesRating` (see 2.8, which never throws).
- `copyFrom(item, rating)` (MangaUpdates.kt:110-113): `item.copyTo(this)` then `score = rating?.rating ?: 0.0`.
- `MUListItem.copyTo` (dto/MUListItem.kt:17-21): `status = listId ?: READING_LIST`; `last_chapter_read = this.status?.chapter?.toDouble() ?: 0.0`. Note the field names: MU `status.chapter` (an Int) becomes `last_chapter_read`, and MU `list_id` becomes the track `status`.
- **OBSERVED defect**: the `catch` catches every `Exception`, including network failures, JSON errors and HTTP 401. Any of these goes to the add path. If the series is already on the user's list and the lookup fails transiently, `addSeriesToList` runs and MU's add endpoint is called for an existing series. See D-MU-2.

### 2.8 Rating fetch (`getSeriesRating`, MangaUpdatesApi.kt:116-126)
**OBSERVED**: GET `/v1/series/{id}/rating`; parse `MURating`; ANY `Exception` returns `null`.
**OBSERVED consequence**: `copyFrom` sets `score = 0.0` when the rating fetch failed. A later `update` sees `score == 0.0` and calls the DELETE rating path (section 2.10), which removes the user's real rating on the server. See D-MU-3.

### 2.9 Add to list (`addSeriesToList(track, hasReadChapters)`, MangaUpdatesApi.kt:55-78)
**OBSERVED**:
1. `status = if (hasReadChapters) READING_LIST else WISH_LIST` (0 or 1).
2. Body is a JSON **array** with one object: `[{"series": {"id": <remote_id>}, "list_id": <status>}]`.
3. POST `/v1/lists/series`, `awaitSuccess()`.
4. Only if the response `code == 200` exactly (line 73): `track.status = status` and `track.last_chapter_read = 1.0`. For 201, 204 or any other 2xx the track fields are NOT updated (the local `status` stays what it was, typically 0 from the search model, and `last_chapter_read` stays 0.0).
5. `track.last_chapter_read = 1.0` is set unconditionally for code 200, regardless of `hasReadChapters`. The value 1.0 is a hard-coded placeholder, not the user's real progress. The caller (`AddMangaTracks.bind`, section 1.4) then pushes the real progress through `setRemoteLastChapterRead` if the user has read chapters locally.

**PROPOSAL**: keep the 200-only status write to stay compatible with the server, but treat any 2xx as success, set `status` from the request, and set `last_chapter_read = 1.0` only when `hasReadChapters` is true (because 1.0 is meaningful only when the user has read chapters). Record this as a deliberate change in the port's changelog. See D-MU-4.

### 2.10 Update list item (`updateSeriesListItem(track)`, MangaUpdatesApi.kt:80-101)
**OBSERVED**:
1. Body is a JSON array `[{"series": {"id": <remote_id>}, "list_id": <track.status>, "status": {"chapter": <track.last_chapter_read.toInt()>}}]`. The `toInt()` truncates toward zero, so `12.9` becomes `12`.
2. POST `/v1/lists/series/update`, `awaitSuccess()`. A non-2xx throws and the rest is skipped.
3. Then `updateSeriesRating(track)` (lines 128-147):
   - if `track.score < 0.0` return (no request). Note: score -1.0 from a fresh search model is covered by this.
   - else if `track.score != 0.0`: PUT `/v1/series/{id}/rating` with `{"rating": <score as Double>}`.
   - else (score is exactly 0.0): DELETE `/v1/series/{id}/rating`.

Consequence: every progress update performs two requests (list update, then rating PUT or DELETE). A score of exactly 0.0 always deletes the remote rating, including when the user never set one.

**PROPOSAL**: keep the same request sequence for compatibility. Avoid DELETE when the previously fetched remote rating was already null: keep a `remote_rating_known_null` flag. Do not treat a failed rating fetch as 0.0 (section 2.8). See D-MU-3.

### 2.11 Update (`MangaUpdates.update`) and track status after `setRemote*`
Covered in 2.3 and 2.10. Both `update` and the generic flow above return the same `track` object.

### 2.12 Delete (`delete(track: DomainTrack)`, MangaUpdates.kt:83-85; `deleteSeriesFromList`, MangaUpdatesApi.kt:103-114)
**OBSERVED**: POST `/v1/lists/series/delete` with body `[<track.remoteId>]` (a JSON array of one number, `buildJsonArray { add(track.remoteId) }`). `awaitSuccess()`. No local state changes in this function. The caller (`DeletableMangaTracker`) removes the local row separately.

Note: the `delete` parameter type is the DOMAIN `MangaTrack`, not the DB model. `remoteId` is a `Long`.

### 2.13 Refresh (`refresh(track)`, MangaUpdates.kt:105-108)
**OBSERVED**: `(series, rating) = api.getSeriesListItem(track)`; `return track.copyFrom(series, rating)`. Differences from bind:
- No try/catch: any failure propagates (rating failure does not, because `getSeriesRating` swallows errors; list-item failure does).
- Two requests (list item then rating), so it is the second call to `getSeriesRating` that is swallowed.
- It overwrites `status`, `last_chapter_read`, `score` from remote. Title, remote_id, tracking_url are not changed.

### 2.14 Search (`searchManga(query)`, MangaUpdates.kt:98-103; `search`, MangaUpdatesApi.kt:149-173)
**OBSERVED**:
1. Body JSON object: `{"search": <query>, "filter_types": ["drama cd", "novel"]}`. Keys in that order. The array is sent verbatim, with the two strings `drama cd` and `novel`, lowercase, space included.
2. POST `/v1/series/search` on the **unauthenticated** `client`. `awaitSuccess()`. Parse `MUSearchResult`, map `results[*].record` (each `MUSearchResultItem.record`, non-null), then `toTrackSearch(id)` for each, where `id = tracker id 7`.
3. No pagination, no limit, no debounce in this code (the UI debounces, outside scope). Empty `results` gives an empty list.
4. The query is passed as-is; it is not trimmed or lower-cased here.

`MURecord.toTrackSearch(id)` (dto/MURecord.kt:26-37) sets, on `MangaTrackSearch.create(id)`:
- `remote_id = seriesId ?: 0L`
- `title = title?.htmlDecode() ?: ""`
- `total_chapters = 0` (search never gives a count)
- `cover_url = image?.url?.original ?: ""` (the `original` URL, not `thumb`)
- `summary = description?.htmlDecode() ?: ""`
- `tracking_url = url ?: ""`
- `publishing_status = ""`
- `publishing_type = type.toString()`  -> **the string `"null"` when `type` is null** (see D-MU-5)
- `start_date = year.toString()`  -> **the string `"null"` when `year` is null** (D-MU-5)

`htmlDecode` = `parseAsHtml().toString()` (`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:65-67`): HTML entity decoding with tags removed. The exact Jsoup-like parser is external; PROPOSAL section 2.14 of the design doc uses the `html-escape` crate plus a tag stripper and records the difference.

`search` ignores `bayesian_rating`, `rating_votes`, `latest_chapter`, `image.height/width`, `image.url.thumb` (they are parsed but unused).

Search result display (external, `app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt`): the title, type (`MR.strings.track_type` shown only if `publishing_type` is not blank), start date (`MR.strings.label_started` shown only if `start_date` not blank), status, and an "open in browser" action on `tracking_url` if not blank. So `"null"` strings appear as visible text: `Type: null`, `Started: null`.

### 2.15 Authentication call error mapping for search and login
- Search: any HTTP error or parse error propagates to the UI, which shows the error text. There is no retry.
- Login: see 2.5.

### 2.16 Manga-specific helpers
- `MangaTracker.setRemoteMangaStatus` etc. use the MU constants above (generic flow, section 1.3).
- `get10PointScore` is identity (score already 0-10).
- `supportsReadingDates = false`, `supportsPrivateTracking = false` (BaseTracker defaults). So the UI does not show start/finish date editors for MU and does not send private flags. `setRemotePrivate` still sets `track.private` locally and calls `update`, which ignores it for MU.

---

## 3. Simkl (SK)

### 3.1 Constants and status mapping
**OBSERVED** (`Simkl.kt:22-32`):
| Name | Value | Simkl string (`toSimklStatus`, SimklUtils.kt:6-11) | Label key |
|---|---|---|---|
| `WATCHING` | 1L | `"watching"` | `AYMR.strings.watching` |
| `COMPLETED` | 2L | `"completed"` | `MR.strings.completed` |
| `ON_HOLD` | 3L | `"hold"` | `MR.strings.on_hold` |
| `NOT_INTERESTING` | 4L | `"notinteresting"` | `AYMR.strings.not_interesting` |
| `PLAN_TO_WATCH` | 5L | `"plantowatch"` | `AYMR.strings.plan_to_watch` |

- Rewatching status = `0` (`getRewatchingStatus`, Simkl.kt:116). Note `0` is NOT in the status list and `toSimklStatus(0)` throws.
- Watching = 1 (`getWatchingStatus`, line 114); completion = 2 (`getCompletionStatus`, line 118).
- `getStatusListAnime()` (lines 101-103) returns `[1, 2, 3, 4, 5]` in that order.
- `getStatusForAnime` (lines 105-112): `1 -> AYMR.strings.watching`, `5 -> AYMR.strings.plan_to_watch`, `2 -> MR.strings.completed`, `3 -> MR.strings.on_hold`, `4 -> AYMR.strings.not_interesting`, else `null`.
- `toSimklStatus()` (SimklUtils.kt:5-12): maps the five values; any other value throws `NotImplementedError("Unknown status: $status")`.
- `toTrackStatus(String)` (SimklUtils.kt:14-21): `"watching"->1`, `"completed"->2`, `"hold"->3`, `"dropped"` or `"notinteresting"` -> 4, `"plantowatch"->5`; any other string throws `NotImplementedError("Unknown status: $status")`. Note `"dropped"` is accepted on input but never produced on output.

Logo `R.drawable.ic_tracker_simkl` (Simkl.kt:97); logo colour `Color.rgb(0, 0, 0)` (line 99) = `0xFF000000`.

### 3.2 Score list and display
- `SCORE_LIST = IntRange(0, 10).map(Int::toString)` (Simkl.kt:29-31): `["0","1",...,"10"]`, 11 entries.
- `indexToScore` is the default (`index.toDouble()`), so index 7 gives `7.0`.
- `displayScore(track) = track.score.toInt().toString()` (Simkl.kt:42-44): truncates fractions.
- Rating sent to Simkl is `score.toInt()` (SimklApi.kt:71), so the Simkl score is always an integer 0-10.

### 3.3 Update rule (`update(track, didWatchEpisode)`, Simkl.kt:50-62)
**OBSERVED**:
1. If `track.status != COMPLETED` and `didWatchEpisode`:
   - if `track.last_episode_seen.toLong() == track.total_episodes && track.total_episodes > 0` then `status = COMPLETED`
   - else `status = WATCHING`
2. If `status == COMPLETED` already, the status is never changed by this function (the outer guard).
3. Then `api.updateLibAnime(track)` (section 3.8) and return its result.

Note: `total_episodes == 0` (unknown) never auto-completes. Movies have `total_episodes = 1` from the sync mapping (3.6), so watching a movie episode 1 auto-completes it.

### 3.4 Bind (`bind(track, hasSeenEpisodes)`, Simkl.kt:64-81)
**OBSERVED**:
1. `remoteTrack = api.findLibAnime(track)` (section 3.5). Possibly null.
2. If not null:
   - `track.copyPersonalFrom(remoteTrack)` (DB extension, see data doc: copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, `private`).
   - `track.library_id = remoteTrack.library_id` - which is always `null` for Simkl (3.6 never sets it). So a re-bind **clears** `library_id`.
   - If `track.status != COMPLETED` then `track.status = if (hasSeenEpisodes) WATCHING else track.status` (so the remote status is kept when no episode is seen locally).
   - Return `update(track)` (section 3.3 with `didWatchEpisode = false`, so status is not recomputed; it pushes progress, list status, and rating).
3. If null:
   - `track.status = if (hasSeenEpisodes) WATCHING else PLAN_TO_WATCH`
   - `track.score = 0.0`
   - Return `add(track)` (section 3.9).

Note: `copyPersonalFrom` copies remote `score`; when the item is found, the local score from the search model (-1.0) is replaced by the remote user rating (integer 0-10, default 0.0).

### 3.5 Find in list (`findLibAnime(track)`, SimklApi.kt:162-202)
**OBSERVED** step by step:
1. POST `/sync/watched` with body `[{"simkl": <track.remote_id>}]` (a JSON array of one object). The response is parsed as `List<SimklSyncWatched>`; `firstOrNull()`; if the list is empty, return null.
2. If `result != true`, return null. If `last_watched` is null, return null. If `list` is null, return null.
3. `type = track.tracking_url.substringAfter("/").substringBefore("/")` (see defect D-SK-2 for absolute URLs; for `/tv/123` it yields `"tv"`).
4. `queryType = if (type == "tv") "shows" else type`.
5. URL: `$API_URL/sync/all-items/$queryType/$status` with query `date_from=<last_watched>` (the raw string from the response, appended with `appendQueryParameter`, so it is URL-encoded). `status` is the `list` string from step 1 (e.g. `watching`).
6. `typeName = if (type == "movies") "movie" else "show"`.
7. GET that URL (authClient); parse `SimklSyncResult`; `getFromType(queryType)` (SimklSyncItem.kt:15-22 supports exactly `"anime"`, `"tv"`, `"movies"`; anything else throws `Exception("Unknown type: <t>")`).
8. `firstOrNull { item -> item.getFromType(typeName).ids.simkl == track.remote_id }`; if none, return null.
9. `listAnime.toAnimeTrack(typeName, type, status)`.

**OBSERVED defect**: step 4 maps `tv` to `shows`, but step 7 then calls `getFromType("shows")`, which is not one of the supported keys, so it throws `Exception("Unknown type: shows")`. Therefore any TV show that is already in the user's list and that passes steps 1-2 makes `findLibAnime` throw. Consequences: bind of an already-tracked TV show fails; `refresh` of a TV track fails. See D-SK-1.

**OBSERVED**: `getFromType(queryType)` returns `List<SimklSyncItem>?` (nullable `anime`/`tv`/`movies` key). A null list gives `?.firstOrNull` -> null (not found).

### 3.6 Mapping a sync item to a track (`SimklSyncItem.toAnimeTrack`, SimklSyncItem.kt:36-53)
**OBSERVED** for input `(typeName, type, statusString)`:
- `resultData = getFromType(typeName)` (`show` or `movie`): `show!!` or `movie!!` (NPE if absent).
- `title = resultData.title`
- `remote_id = resultData.ids.simkl`
- if `typeName != "movie"`: `total_episodes = totalEpisodesCount!!` (Long), `last_episode_seen = watchedEpisodesCount!!` (Double).
- else: `total_episodes = 1`, `last_episode_seen = if (statusString == "completed") 1.0 else 0.0`.
- `score = userRating?.toDouble() ?: 0.0`
- `status = toTrackStatus(statusString)` (may throw NotImplementedError for an unknown status string).
- `tracking_url = "/$type/${resultData.ids.simkl}"` where `type` is the path segment (`tv`, `anime`, or `movies`), so for a movie tracked from the movies list this is `/movies/<id>`.
- `library_id` is never set (stays null).

### 3.7 Login via browser (`Simkl.login(code)`, Simkl.kt:120-131 and 122-131)
**OBSERVED**:
1. UI: the settings row for Simkl calls `context.openInBrowser(SimklApi.authUrl(), forceDefaultBrowser = true)` (SettingsTrackingScreen.kt:53-61). The auth URL is `https://simkl.com/oauth/authorize?response_type=code&client_id=<CLIENT_ID>&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth` (SimklApi.kt:248-253).
2. The provider redirects to `aniyomi://simkl-auth?code=<code>`. The Android intent filter (AndroidManifest.xml, the `TrackLoginActivity` block, lines 3-21 of the excerpt) routes host `simkl-auth` to `TrackLoginActivity`.
3. `TrackLoginActivity.handleSimkl(data)` (external, `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt:72-83`): `code = data?.getQueryParameter("code")`. If null, `trackerManager.simkl.logout()` and return to settings. Else `lifecycleScope.launchIO { trackerManager.simkl.login(code); returnToSettings() }`.
4. `login(code)` (Simkl.kt:122-131), inside `try`:
   a. `oauth = api.accessToken(code)` (section 3.10).
   b. `interceptor.newAuth(oauth)`: sets the in-memory token AND calls `saveToken(oauth)` (persists JSON, section 3.11).
   c. `user = api.getCurrentUser()` (GET `/users/settings`, SimklApi.kt:204-213; `runBlocking`, returns `account.id` as Int).
   d. `saveCredentials(user.toString(), oauth.accessToken)`: username = decimal user id, password = access token.
   On any `Throwable` in the try block: `logout()` (section 3.12). No error is shown to the user; `TrackLoginActivity` always returns to settings.

**OBSERVED**: `login(username, password)` (Simkl.kt:120) is `login(password)`, i.e. the second argument is treated as the OAuth code. The Simkl settings row does not use the credential dialog, so this path is reached only by programmatic calls.

### 3.8 Update library item (`updateLibAnime(track)`, SimklApi.kt:122-140)
**OBSERVED** in order:
1. `type = substringAfter("/").substringBefore("/")` from `tracking_url`; `mediaType = if (type == "movies") "movies" else "shows"`.
2. If `type != "movies"`: `updateProgress(track)` (section 3.8.1).
3. `addToList(track, mediaType)` (section 3.8.2).
4. `updateRating(track, mediaType)` (section 3.8.3).
5. Return `track`.

#### 3.8.1 Progress (`updateProgress`, SimklApi.kt:87-96; `buildProgressObject`, 98-120)
- POST `/sync/history/remove` with body built by `buildProgressObject(track, add = false)`: `{"shows":[{"ids":{"simkl":<id>},"seasons":[{"number":1}]}]}`.
- Then POST `/sync/history` with `buildProgressObject(track, add = true)`: same plus `"episodes": [{"number":1}, {"number":2}, ..., {"number":N}]` inside the season object, where `N = last_episode_seen.toInt()` (truncation). For `N = 0` the episodes array is `[]`.
- The season number is always 1 (`put("number", 1)`), even for multi-season titles; episode numbers are absolute.
- **OBSERVED**: the remove and add are two separate requests with no transaction. If the add fails after the remove succeeded, the server history for the title is left empty. See D-SK-5.
- Movies never get progress sync (step 2 guard).
- Progress is always sent under the `shows` key, including when `tracking_url` type was `anime` or was empty (search-derived, see D-SK-2).

#### 3.8.2 Add or change list status (`addToList(track, mediaType)`, SimklApi.kt:48-62)
- POST `/sync/add-to-list` with body `{"<mediaType>":[{"ids":{"simkl":<id>},"to":"<track.toSimklStatus()>"}]}`.
- `toSimklStatus()` throws `NotImplementedError` for any status not in {1,2,3,4,5} (so for 0 = rewatching, or any corrupt value). The exception is raised before the request is sent.
- Note: the same endpoint is used for add (3.9) and for change; the server treats it as upsert.

#### 3.8.3 Rating (`updateRating(track, mediaType)`, SimklApi.kt:64-85)
- Body: `{"<mediaType>":[{"ids":{"simkl":<id>},"rating":<score.toInt()>}]}`.
- If `track.score == 0.0`: POST `/sync/ratings/remove` (same body, rating 0 included).
- Else: POST `/sync/ratings`.
- Note: this runs on every update, so a Simkl track with no rating gets a remove call on every progress update.
- Score below 0 (search default -1.0) would go to the `else` branch and send `rating: -1`. This does not happen in normal flow, because bind sets `score` from the remote or 0.0 before any update. Recorded as edge E-SK-9 in the test doc.

### 3.9 Add (`add(track)` private, Simkl.kt:46-48 -> `addLibAnime`, SimklApi.kt:36-46)
**OBSERVED**:
1. `type = substringAfter("/").substringBefore("/")` from `tracking_url`; `mediaType = if (type == "movies") "movies" else "shows"`.
2. `addToList(track, mediaType)` (3.8.2). No progress sync, no rating sync on add.
3. Return `track`.

Consequence: for a freshly found search result (absolute URL, see D-SK-2), `type` is `""`, so `mediaType` is `"shows"` even for movies. Movies added through search therefore go to the Simkl **shows** list.

### 3.10 Token exchange (`accessToken(code)`, SimklApi.kt:215-234)
**OBSERVED**: POST `https://api.simkl.com/oauth/token` on the bare `client`, JSON body (content type `application/json; charset=utf-8`, `jsonMime`):
```
{"code": "<code>", "client_id": "<CLIENT_ID>", "client_secret": "<CLIENT_SECRET>",
 "redirect_uri": "aniyomi://simkl-auth", "grant_type": "authorization_code"}
```
Key order as written. Response parsed as `SimklOAuth` (`{"access_token": "..."}`). `awaitSuccess()`. Runs under `withIOContext`.

Constants (SimklApi.kt:237-246): `CLIENT_ID = "aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e"`, `CLIENT_SECRET = "2bec9c1d0c00a1e9b0e9e096a71f88d555a6f52da7923df07906df3b21351783"`, `BASE_URL = "https://simkl.com"`, `API_URL = "https://api.simkl.com"`, `OAUTH_URL = "$API_URL/oauth/token"`, `LOGIN_URL = "$BASE_URL/oauth/authorize"`, `POSTERS_URL = "https://simkl.in/posters/"`, `REDIRECT_URL = "aniyomi://simkl-auth"`.

The client secret ships inside the app binary. For a desktop app that is not a secret; see section 10 and security note in the rust design.

### 3.11 Token persistence (`saveToken`, `restoreToken`, Simkl.kt:133-143)
**OBSERVED**:
- `saveToken(oauth)`: `trackPreferences.trackToken(this).set(json.encodeToString(oauth))`. `trackToken` key = `track_token_<id>` = `track_token_101`. The value is the JSON of `SimklOAuth`, i.e. `{"access_token":"<token>"}`. `encodeToString(null)` produces the literal string `null`.
- `restoreToken()`: `json.decodeFromString<SimklOAuth>(stored)`, catching any `Exception` and returning null. So the literal `null` string and an empty string both give null.
- `SimklInterceptor` constructor reads `restoreToken()` once (SimklInterceptor.kt:14), lazily when `Simkl.interceptor` is first used.

### 3.12 Logout (`logout()`, Simkl.kt:145-149)
**OBSERVED**:
1. `super.logout()` clears username and password (`setCredentials("", "")`).
2. `trackPreferences.trackToken(this).delete()` removes `track_token_101`.
3. `interceptor.newAuth(null)` sets the in-memory token to null AND calls `saveToken(null)`, which writes the literal string `"null"` back into `track_token_101`. So the stored value after logout is `"null"`, not absent. `restoreToken()` treats it as null (3.11), so behaviour is correct; the stored value is just untidy. PROPOSAL: delete the key once and do not rewrite it.

### 3.13 Auth interceptor (SimklInterceptor.kt:16-29)
**OBSERVED**:
- `oauth ?: throw Exception("Not authenticated with Simkl")` (line 19) when no token.
- Adds `Authorization: Bearer <accessToken>` (line 23), `simkl-api-key: <CLIENT_ID>` (line 24), and sets `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)` (line 25, replacing).
- No token refresh, no expiry check, no handling of HTTP 401.

### 3.14 Search (`searchAnime(query)`, Simkl.kt:83-87; `searchAnime(search, type)`, SimklApi.kt:142-156)
**OBSERVED**:
- The tracker runs three requests **sequentially** in this order: `api.searchAnime(query, "anime")`, then `"tv"`, then `"movie"`. The results are concatenated (`+`), no de-duplication, no sorting.
- Each request: GET `$API_URL/search/<type>` built with `toUri().buildUpon()` and params in this order: `q=<query>`, `extended=full`, `client_id=<CLIENT_ID>`. The bare `client` is used (no Bearer). Note `type` in the path is `anime`, `tv` or `movie` (singular for movie).
- The response is a JSON array parsed as `List<SimklSearchResult>` and mapped with `toTrackSearch(type)` (the path type, used as fallback).
- Any error in any of the three requests fails the whole search (no partial results).
- No empty-query check here; the UI handles that.

`SimklSearchResult.toTrackSearch(fallbackType)` (SimklSearch.kt:25-37) on `AnimeTrackSearch.create(TrackerManager.SIMKL)`:
- `remote_id = ids.simklId`
- `title = titleRomaji ?: this.title!!` (NPE, which fails the whole search, if both are null)
- `total_episodes = epCount ?: 1`
- `cover_url = poster?.let { "https://simkl.in/posters/${it}_m.webp" } ?: ""`
- `summary = allTitles?.joinToString("\n", prefix = "All titles:\n") ?: ""` (prefix is present even if the list is empty)
- `tracking_url = url` (the absolute URL from the response; required field, a missing `url` fails parsing)
- `publishing_status = status ?: "ended"`
- `publishing_type = type ?: fallbackType`
- `start_date = year?.toString() ?: ""`

Note `poster` gets `_m.webp` appended (medium size).

### 3.15 Refresh (`refresh(track)`, Simkl.kt:89-95)
**OBSERVED**: `api.findLibAnime(track)?.let { remote -> track.copyPersonalFrom(remote); track.total_episodes = remote.total_episodes }`; return `track`. If not found, the track is returned unchanged with no error. If `findLibAnime` throws (defect D-SK-1 for TV), the exception propagates.

Note that `refresh` does not copy `library_id` (unlike bind).

### 3.16 Delete
Simkl does not implement `DeletableAnimeTracker`. There is no delete for Simkl in this area.

### 3.17 Login state and settings row
- Settings row (SettingsTrackingScreen.kt:52-61): login opens the browser; logout opens `TrackingLogoutDialog` (section 4). The row is `TrackerPreference` with `tracker = trackerManager.simkl`.
- After a successful browser login the activity returns to settings (`returnToSettings()`), which re-reads `isLoggedIn`.

---

## 4. Message keys and literal strings

### 4.1 `MR.strings` and `AYMR.strings` used by these files
- `MR.strings.reading_list`, `MR.strings.wish_list`, `MR.strings.complete_list`, `MR.strings.on_hold_list`, `MR.strings.unfinished_list` (MangaUpdates.kt:55-59).
- `AYMR.strings.watching`, `AYMR.strings.plan_to_watch`, `AYMR.strings.not_interesting` (Simkl.kt:106, 107, 110).
- `MR.strings.completed`, `MR.strings.on_hold` (Simkl.kt:108, 109).
- Settings and login UI (external, SettingsTrackingScreen.kt): `MR.strings.username` (MU login label, line 188), `MR.strings.password`, `MR.strings.login_title` (title, argument = tracker.name), `MR.strings.action_close` (close icon description), `MR.strings.loading` (button while processing), `MR.strings.login` (button), `MR.strings.login_success` (toast after login), `MR.strings.logout_title` (argument = tracker.name), `MR.strings.action_cancel`, `MR.strings.logout`, `MR.strings.logout_success` (toast after logout), `MR.strings.services` (group title), `MR.strings.tracking_info` (info row).
- Search result display (external): `MR.strings.track_type`, `MR.strings.label_started`.

### 4.2 Literal (non-localised) strings produced by these files
These appear as-is in logs, toasts or thrown messages (English only; the port should keep the exact text in logs and in developer-facing errors, and localise user-visible ones by the mapping above):
- `"Unable to login"` (MangaUpdates.kt:116): unreachable in practice.
- `"Not authenticated with MangaUpdates"` (MangaUpdatesInterceptor.kt:17): IOException; shown via the toast of the login dialog or the error text of a list call.
- `"Not authenticated with Simkl"` (SimklInterceptor.kt:19): Exception.
- `"Unknown status: $status"` (SimklUtils.kt:11, 20): NotImplementedError.
- `"Unknown type: $type"` and `"Unknown type: $typeName"` (SimklSyncItem.kt:20, 59): Exception.
- `"All titles:\n"` (SimklSearch.kt:31): summary prefix.
- `"aniyomi://simkl-auth"` (SimklApi.kt:246).
- `"simkl-api-key"` header name (SimklInterceptor.kt:24).
- `"Aniyomi v$VERSION_NAME (${APPLICATION_ID})"` User-Agent in both interceptors.

### 4.3 Toast behaviour (external, generic flow)
Any exception raised during `updateRemote` (section 1.3) is shown as a toast with `e.message` (may be `null` and then shown as nothing or `"null"`; the generic code calls `toast(e.message)` with a nullable string). Fine for the port: show `e.to_string()` with the kind prefix, never the literal `"null"`.

---

## 5. Side effects

### 5.1 Network writes
- MU: list add, list update, rating PUT/DELETE, list delete. Every progress update therefore writes two remote resources.
- SK: history remove + add (for shows), add-to-list, rating set or remove. A progress update is 4 writes for shows (history remove, history add, add-to-list, ratings or ratings/remove); for movies it is 2 writes (add-to-list, ratings or ratings/remove).

### 5.2 Local writes
- Token: SK writes `track_token_101` on login, on logout (as `"null"`), and on `newAuth`.
- Credentials: both services write username and password on login (MU: uid + token; SK: user id + access token) and clear both on logout.
- DB: the caller (generic flow and AddAnimeTracks/AddMangaTracks) writes the track row via `insertTrack` after bind or update. MU and SK do not touch the DB directly.
- Preferences: `pref_tracker_auth_expired_*` is reset to false on every `setCredentials`, and is never set by MU or SK.

### 5.3 Notifications and work
- No notification, no WorkManager job, no background sync is started by these files. Background tracking sync is the library update job (`app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt`, outside this area; it calls the generic flow, not these files directly).

### 5.4 Browser and deep links
- Opening the browser for Simkl auth: `context.openInBrowser(url, forceDefaultBrowser = true)`.
- Opening a search result: `context.openInBrowser(tracking_url)` if not blank (external).

---

## 6. Concurrency, timings and limits

### 6.1 Dispatchers
- MU: `api.*` functions are `suspend` and use the caller's dispatcher (the UI or IO caller of `AddMangaTracks`/`updateRemote`). Search and rating use `awaitSuccess` directly; no `withIOContext` wrapper in MU (the caller provides IO context).
- SK: `SimklApi` wraps every public call in `withIOContext` (SimklApi.kt:37, 123, 143, 163, 216). `getCurrentUser()` is NOT suspend and uses `runBlocking` (line 205), which blocks the thread it runs on. It is called from `Simkl.login(code)` inside a `launchIO` coroutine (TrackLoginActivity), so it blocks an IO thread for the duration of one HTTP request.
- `TrackLoginActivity` launches the login on `launchIO`, then calls `returnToSettings()` (external).

### 6.2 Ordering and parallelism
- MU: strictly sequential per call. `updateSeriesListItem` runs the list update first and the rating call after (2.10).
- SK: `updateLibAnime` is strictly sequential: progress remove, progress add, add-to-list, rating (3.8). `searchAnime` is sequential across the three types (3.14). No parallel calls anywhere in scope.

### 6.3 Timeouts, retries, limits
- No retry, backoff or timeout logic is defined in the scope files. Retries and timeouts come from the shared OkHttp client (external, area 12-network-stack).
- Request cache: `GET()` defaults to `CacheControl.Builder().maxAge(10, MINUTES)` (external, `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:16`). Every MU and SK GET sends `Cache-Control: max-age=600`, which allows a cached response up to 10 minutes old when the client has a cache. The POST, PUT and DELETE helpers in the same file use the same default cache object, but only GET responses are served from the OkHttp cache. PROPOSAL: the Rust client does not add an HTTP cache for these trackers.
- MU search: no page size; the server returns what it returns.
- SK search: no page size; results are whatever `search/<type>` returns with `extended=full`.
- Score ranges: MU 0.0 to 10.0 with one decimal place (92 list entries); SK integer 0 to 10 (sent as `rating` integer).
- Progress limits: SK sends episodes 1..N in one request, so N can be large (a 1000-episode show sends 1000 objects in one array). No batching in code.
- Date values: SK `date_from` is the server's `last_watched` string (opaque, not parsed). Track dates in the DB are epoch millis (`started_*`, `finished_*`) and are not sent to SK or MU by these files.

---

## 7. Interactions with other areas
- area:27-tracking-core: the generic setter flow (1.3), bind flow (1.4), TrackPreferences keys, BaseTracker, TrackerManager registry.
- area:05 (DB): tracks are inserted via `InsertMangaTrack` / `InsertAnimeTrack` (external).
- area:12-network-stack: the OkHttp client, `awaitSuccess`, `parseAs`, JSON MIME, GET cache default.
- area:13-core-util: `htmlDecode`, `openInBrowser`, `toast`.
- area:26-library-update-jobs: background progress sync goes through the generic flow.
- area:21-backup-create / area:22-backup-restore: backup stores track rows (`tracking_url`, `remote_id`, `status`, `score`, `last_episode_seen`, etc.) and restores them without calling the services. Restore does not re-login. Track tokens are not backed up (external, to confirm in those areas).
- area:03 and 04 (DB migrations): `tracking_url` is stored as text; no schema change needed for these two services.
- area:02-preferences-system: `track_token_101`, `pref_mangasync_*` key patterns.

---

## 8. Observed defects (fix list for the port; each has a PROPOSAL)

Severity: H = data loss or broken feature for a common path; M = wrong data shown or sent; L = cosmetic or unreachable.

- **D-SK-1 (H)** TV lookup throws. `SimklApi.findLibAnime` (SimklApi.kt:184, 191-194): `queryType = "shows"` for `tv`, then `SimklSyncResult.getFromType("shows")` (SimklSyncItem.kt:15-22) throws `Exception("Unknown type: shows")`. Affects bind and refresh of any already-listed TV show. PROPOSAL: map `"tv"` to the response key `tv` for the parse step and to `shows` only for the URL path; parse `SimklSyncResult` by the response key (`anime`, `tv`, `movies`), and never call getFromType with the URL path segment.
- **D-SK-2 (H)** Search-derived `tracking_url` is absolute. `SimklSearch.kt:33-36` sets `tracking_url = url` (absolute `https://simkl.com/...`), and `substringAfter("/").substringBefore("/")` yields `""` for it. Then `addLibAnime` sends movies to the `shows` list (SimklApi.kt:41), `findLibAnime` builds `/sync/all-items//<status>` (SimklApi.kt:185) when the item is already watched, and `updateLibAnime` sends progress for every item. PROPOSAL: normalise on search: parse the path type from the URL (`/anime/`, `/tv/`, `/movies/`, `/movie/` -> `movies`), store the normalised form `/<type>/<id>` in `tracking_url` at search time, and keep the absolute URL only for display in a separate field. Keep the Kotlin behaviour (`/tv/<id>` after bind) as the stored form.
- **D-SK-3 (L)** Re-bind sets the in-memory `library_id` to null (Simkl.kt:68; SimklSyncItem.kt:36-53 never sets it). The DB `update` statement uses `library_id = coalesce(:libraryId, library_id)` (`data/src/main/sqldelightanime/dataanime/anime_sync.sq:52`, and the manga equivalent), so a stored value survives an update with null; whether the in-memory null reaches the DB depends on which repository path `InsertAnimeTrack` takes (area:05, to verify). PROPOSAL: never copy a null `library_id` over a non-null one.
- **D-SK-4 (M)** `logout` writes `"null"` back into the token key (Simkl.kt:145-149). PROPOSAL: delete only.
- **D-SK-5 (H)** Progress is remove-then-add without a transaction (SimklApi.kt:87-96). A failed add leaves the server history empty for that title. PROPOSAL: send the add first using a full episode list, then remove only episodes not in the list (the server also has `sync/history` upsert semantics; verify with the API docs before changing). If the Kotlin order must be kept for compatibility, on add failure retry the add once and surface the error.
- **D-SK-6 (M)** Every update sends a rating call, even when the rating did not change (SimklApi.kt:64-85). PROPOSAL: skip when the score equals the last known remote score (keep the last known value in the track row).
- **D-SK-7 (M)** Unknown or rewatching status throws `NotImplementedError` before any request (SimklUtils.kt:11). PROPOSAL: map status 0 to `PLAN_TO_WATCH` or return a typed error `TrackError::UnknownStatus`, never panic.
- **D-SK-8 (L)** `title!!` in search mapping (SimklSearch.kt:28) fails the whole search when both titles are null. PROPOSAL: fall back to `""` and skip the record.
- **D-SK-9 (M)** `getCurrentUser` uses `runBlocking` inside a suspend chain (SimklApi.kt:204-213). PROPOSAL: make it `async fn`.
- **D-SK-10 (L)** `SimlkSearchResultIds` class name typo (SimklSearch.kt:41). PROPOSAL: name it `SearchIds` in Rust (not user-visible).
- **D-SK-11 (M)** The Simkl API key is also sent as the `client_id` query param in search (SimklApi.kt:147) and the client secret is in the binary (SimklApi.kt:238). See section 10.
- **D-MU-1 (M)** Logout keeps the in-memory token (MangaUpdates.kt has no logout override; MangaUpdatesInterceptor.kt:28-30 never cleared). PROPOSAL: override `logout` to clear the auth token as well.
- **D-MU-2 (H)** Bind's `catch (e: Exception)` (MangaUpdates.kt:91-95) treats every error as "not on list" and calls add. PROPOSAL: distinguish HTTP 404 (not on list -> add) from other errors (propagate; do not add).
- **D-MU-3 (H)** A failed rating fetch becomes `score = 0.0` (MangaUpdatesApi.kt:123-125 with copyFrom at MangaUpdates.kt:112), and the next update then DELETEs the user's real remote rating (MangaUpdatesApi.kt:142-145). PROPOSAL: keep a tri-state rating (`Unknown`, `None`, `Some(f64)`); on `Unknown` do not send rating requests; on refresh failure keep the previous score.
- **D-MU-4 (M)** Add sets `last_chapter_read = 1.0` unconditionally on HTTP 200 and sets status only on 200 (MangaUpdatesApi.kt:72-77). PROPOSAL in 2.9.
- **D-MU-5 (M)** `"null"` strings from `type.toString()` and `year.toString()` (MURecord.kt:35-36). PROPOSAL: empty string when null.
- **D-MU-6 (L)** Unreachable `Throwable("Unable to login")` (MangaUpdates.kt:116-118). PROPOSAL: a parse failure is the error; do not keep a dead branch.
- **D-MU-7 (M)** `indexToScore(-1)` throws `IndexOutOfBoundsException` (MangaUpdates.kt:71) when a score string is not in the list (`setRemoteScore` path, MangaTracker.kt:92). PROPOSAL: unknown string -> reject with a typed error, do not write.
- **D-MU-8 (L)** `displayScore` shows `0.0` where the list shows `-` for the zero entry (MangaUpdates.kt:73). PROPOSAL: display `-` for 0.0 to match the picker.
- **D-MU-9 (M)** Login dialog toasts `e.message.toString()` (SettingsTrackingScreen.kt:361), which is `"null"` for null messages. PROPOSAL: show `"<service> login failed: <kind>"` plus the HTTP status.
- **D-MU-10 (L)** `MangaUpdatesApi.search` uses the unauthenticated client but `getSeriesListItem` uses `authClient`; `refresh` lacks try/catch. Consistent with the design; keep.

---

## 9. Dead code and unused declarations (documented, not carried over)
- `MURating.copyTo(track)` (dto/MURating.kt:11-15): never called (only `MUListItem.copyTo` is used; `MangaUpdates.copyFrom` calls `item.copyTo` and sets score from the rating directly). Decision: Drop; the Rust struct stays, no conversion function.
- `MangaUpdatesApi.authenticate` returns `MUContext?` but never returns null. Decision: return `Result<MUContext, TrackError>`.
- `MUSeries.title`, `MUListItem.priority`, `MUImage.height/width`, `MURecord.bayesianRating`, `ratingVotes`, `latestChapter`, `MUStatus.volume`: parsed but unused. Keep them in the DTO (cheap, and the DTO shape is documented), mark as unused in Rust with `#[allow(dead_code)]`-free `Option` fields (serde ignores missing).
- `Simkl.getScoreList` etc. are used; `SimklSyncWatched.lastWatched` is used; `SimklUser` used.
- `SimklSyncItem.toAnimeTrack` sets `total_episodes` etc. only for shows/movies; the `statusString` parameter is used.
- `SimklApi.updateRating` for score 0 uses `/sync/ratings/remove` with `rating` in body: kept as is.
- `Simkl.kt` private `add` is used only by `bind`.

---

## 10. Android-to-Linux notes that change behaviour
- **Deep link** `aniyomi://simkl-auth`: on Android the intent filter (AndroidManifest.xml) receives the redirect. On Linux there is no manifest. PROPOSAL: register a `x-scheme-handler/aniyomi` MIME type in the Kurobayomi `.desktop` file and, as a fallback, show a "paste the redirect URL or code" entry in the Simkl login dialog (parse `code` from the query). The code is short-lived; accept it once and then discard it. Note the redirect URI is fixed by the Simkl app registration (`aniyomi://simkl-auth`); the port must either keep that URI (requires the same registration, so the desktop handler is the only supported path) or request a new registration. Record this as an open decision in the rust design section 8.
- **Browser open**: `openInBrowser(url, forceDefaultBrowser=true)` -> `gio::AppInfo::launch_default_for_uri` (or `xdg-open`). `forceDefaultBrowser` has no Linux equivalent beyond the default handler; document.
- **Secret storage**: Android app preferences -> `track_token_101` (JSON) and `pref_mangasync_password_7` in the Kurobayomi preference store (SQLite). PROPOSAL: move the MU session token and the SK access token to the secret store (libsecret via `keyring`), keep the username fields in the preference store. Migration: read the old values once on first run if present.
- **Client secret**: `CLIENT_SECRET` is in the binary today. PROPOSAL: keep it as a compile-time constant (matching the source), and document that it is public-equivalent for a desktop client.
- **Thread model**: `runBlocking` (SK) -> async; Kotlin coroutines -> tokio tasks with results sent to the GTK loop over a channel (rust design section 4).
- **Time**: none of these files use wall-clock time directly. The generic flow does (`System.currentTimeMillis()`), which is external.
- **Locale**: user-visible labels come from MR/AYMR keys; the port maps them to its own string table (area:02 or the bootstrap i18n plan).
