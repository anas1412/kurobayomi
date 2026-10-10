# Network hosts and endpoints catalog, chunk 1 of 2

Source of the item list: `python3 plans/_tools/inventory.py items hosts 1 2` (39 hosts). First item: `anansi-project.github.io`. Last item: `github-readme-stats.vercel.app`. Every path below is repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Chunk 2 covers the remaining hosts (among them `graphql.anilist.co`, `kitsu.app`, `myanimelist.net`, `simkl.com`, `simkl.in`, `wsrv.nl`, `github.com`, `shikimori.one`); where one of those is the real endpoint of a chunk 1 item (for example `graphql.anilist.co` for `anilist.co`) it is described here too, because the code cannot be understood otherwise.

Reading guide. OBSERVED is what the Kotlin does. "Proposal:" is what the Rust app (Kurobayomi) should do. Task ids are not assigned here; the owning areas are `area:12-network-stack` (HTTP client, DNS, rate limiting, cookies), `area:tracking` (all tracker clients), `area:player` (AniSkip, airing time), `area:reader` / `area:downloads` (data saver) and `area:app-shell` (about screen, update check, help links).

## 0. Cross-cutting facts that apply to every row (OBSERVED)

### 0.1 The shared OkHttp client, `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt`

* Cookie jar `AndroidCookieJar` (shared with the Android WebView cookie store).
* Timeouts (`NetworkHelper.kt:20-31`): `connectTimeout(30, SECONDS)`, `readTimeout(30, SECONDS)`, `callTimeout(2, MINUTES)`.
* HTTP cache: directory `<cacheDir>/network_cache`, `maxSize = 5L * 1024 * 1024` (5 MiB). Applies only to GET (OkHttp never caches POST/PUT/DELETE).
* Interceptors, in order: `UncaughtExceptionInterceptor`, `UserAgentInterceptor`, optional `HttpLoggingInterceptor(Level.HEADERS)` as a network interceptor when preference `verbose_logging` is true, then `CloudflareInterceptor` (added last, after the DoH selection).
* `UserAgentInterceptor` (`.../interceptor/UserAgentInterceptor.kt:13-17`) only sets a User-Agent when the request has none (empty or absent). The default comes from `NetworkPreferences.defaultUserAgent()` (key `default_user_agent`), default value `"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"`.
* Request helpers (`.../network/Requests.kt`): `GET/POST/PUT/DELETE(url, headers, body, cache)`. Default `CacheControl` is `maxAge(10, MINUTES)` on every request built by these helpers. Default body for POST/PUT/DELETE is an empty `FormBody`.
* `awaitSuccess()` (`.../network/OkHttpExtensions.kt:109-117`): closes the response and throws `HttpException(code)` (message `"HTTP error $code"`, `HttpException.kt`) for any status outside 200..299. Redirects are followed by OkHttp's default (up to 20).
* The tracker "auth client" in each `*Api.kt` is `client.newBuilder().addInterceptor(<TrackerInterceptor>).build()`, so tracker calls also pass the Cloudflare interceptor and the DoH setting. Search/login calls of some trackers deliberately use the plain `client` (no bearer header): MAL token call, Kitsu login and Algolia search, Bangumi token call, Simkl search and token call, MangaUpdates search and login.
* JSON config (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`): `Json { ignoreUnknownKeys = true; explicitNulls = false }`. There is NO `coerceInputValues`, so an unknown enum string in a DTO fails the whole decode (matters for AniSkip below).
* AniSkip and AniChart (see their rows) do not use this client: each constructs a bare `OkHttpClient()` with no cookie jar, no DoH, no cache, no User-Agent interceptor (OkHttp's own `okhttp/<version>` UA) and default 10 s connect/read/write timeouts.

### 0.2 Tracker identity and common state (`app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:17-42`, `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`)

| Tracker | Id | Class |
|---|---|---|
| MyAnimeList | 1 | `MyAnimeList(1L)` |
| AniList | 2 (`TrackerManager.ANILIST`) | `Anilist` |
| Kitsu | 3 (`KITSU`) | `Kitsu` |
| Shikimori | 4 | `Shikimori` |
| Bangumi | 5 | `Bangumi` |
| Komga | 6 | `Komga` |
| MangaUpdates | 7 | `MangaUpdates` |
| Kavita | 8 (`KAVITA`) | `Kavita` |
| Suwayomi | 9 | `Suwayomi` |
| Simkl | 101 (`SIMKL`) | `Simkl` |
| Jellyfin | 102 (`JELLYFIN`) | `Jellyfin` |

Credential storage keys (`TrackPreferences.kt:14-35`), all with the private prefix (`Preference.privateKey(...)` = `__PRIVATE_` + key, dropped from backups unless "Private settings" is on):

* `pref_mangasync_username_<trackerId>` (string, default `""`)
* `pref_mangasync_password_<trackerId>` (string, default `""`; holds the access token for OAuth trackers, the user id for Kitsu, the session token for MangaUpdates)
* `pref_tracker_auth_expired_<trackerId>` (boolean, default false; reset to false by `saveCredentials`)
* `track_token_<trackerId>` (string, default `""`; JSON of the OAuth object for AniList, MAL, Bangumi, Kitsu, Simkl)

Non-private preferences used by the rows below: `anilist_score_type` (string, default `POINT_10`, `TrackPreferences.kt:37`), `pref_auto_update_manga_sync_key` (bool, default true), `track_on_adding_to_library` (bool, default true), `show_next_episode_airing_time` (bool, default true, `TrackPreferences.kt:43-46`), `pref_auto_update_manga_on_mark_read` (enum `AutoTrackState`, default `ALWAYS`).

`BaseTracker.isLoggedIn` = username not empty AND password not empty (`BaseTracker.kt`). Logging out clears both plus the token key.

### 0.3 OAuth redirect on Android (`app/src/main/AndroidManifest.xml:243-260`, `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`)

`TrackLoginActivity` is exported with scheme `aniyomi` and hosts `myanimelist-auth`, `anilist-auth`, `bangumi-auth`, `shikimori-auth`, `simkl-auth`. Handling:

* `anilist-auth`: implicit grant. Reads the URI FRAGMENT with regex `(?:access_token=)(.*?)(?:&)` (so the fragment must contain a trailing `&`, normally `&token_type=Bearer&expires_in=...`), calls `Anilist.login(token)`; no match: `logout()`.
* `bangumi-auth`, `myanimelist-auth`, `shikimori-auth`, `simkl-auth`: authorization-code grant. Reads the query parameter `code`; present: `login(code)`; absent: `logout()`.
* After each case `returnToSettings()`.

Proposal (applies to every OAuth tracker): there is no custom URL scheme in a plain GTK app, so Kurobayomi runs a one-shot loopback HTTP listener on `127.0.0.1:<random free port>` (tokio + hyper or `tiny_http`) for the duration of the login, opens the authorize URL with `gtk::UriLauncher` / `xdg-open`, and registers its own OAuth application per tracker with redirect `http://127.0.0.1:<port>/<tracker>-auth` where the provider allows a loopback redirect (MAL: yes for app type "other"; AniList, Bangumi, Simkl: register `http://127.0.0.1` / `http://localhost` with the fixed port range the developer picks, e.g. 38250; confirm in each provider's developer console). Do not reuse Aniyomi's client ids or secrets: they are bound to the `aniyomi://` redirect and belong to the Aniyomi project. Secrets embedded in the Aniyomi source (Bangumi `CLIENT_SECRET` at `BangumiApi.kt:269`, Simkl `CLIENT_SECRET` at `SimklApi.kt:238`, Kitsu `CLIENT_SECRET` at `KitsuApi.kt:430`) are intentionally not reproduced in this plan. Store tokens in the Secret Service (`oo7` crate, libsecret) instead of the settings file; fall back to a 0600 file under `$XDG_DATA_HOME/kurobayomi/` when no secret service runs.

### 0.4 Generic verification advice for tracker clients

* Record real responses once per endpoint into `tests/fixtures/<tracker>/<name>.json` (strip tokens) and unit-test the DTO deserialisation against them with `serde_json`.
* Run the HTTP layer against a `wiremock` server for the request-shape tests (method, path, query, headers, body JSON).
* One manual smoke script per tracker (`cargo run --example track_smoke -- <tracker> <query>`) that logs in with a throw-away account, searches, binds, updates progress to 1, refreshes, deletes. Never run it against a real library.

## 1. Item table (all 39, role and decision)

| Host | Role | Keep in Rust? |
|---|---|---|
| `anansi-project.github.io` | ComicInfo.xml schema documentation link (comment only) | Doc-only; schema is implemented in `area:local-source` / `area:downloads` |
| `anidex.moe` | Torrent tracker announce URL inside the default TorrServer tracker list | Keep as data only if torrent streaming is implemented (P3) |
| `anilist.co` | Tracker: AniList GraphQL API (`graphql.anilist.co`), OAuth authorize page, web links; also AniSkip id mapping and airing-time lookup | Yes, P1 (tracker), P2 (AniSkip/airing) |
| `aniyomi.org` | Project website, docs, FAQ, privacy policy links (no API calls) | Replace with Kurobayomi docs links; see section 6 |
| `api.aniskip.com` | Skip-time (intro/outro) API for the player | Yes, P2 |
| `api.bgm.tv` | Tracker: Bangumi REST API | Yes, P2 |
| `api.github.com` | App update check (latest release) | Replace/drop, P3 |
| `api.manga.com` | Fictional example host in KDoc of the extension-facing rate limiter | Not a real endpoint; semantics kept in `area:12-network-stack` |
| `api.mangaupdates.com` | Tracker: MangaUpdates REST API | Yes, P2 |
| `api.myanimelist.net` | Tracker: MyAnimeList API v2 | Yes, P1 |
| `api.resmush.it` | Image data-saver backend "reSmush.it" | Optional, P2 |
| `api.simkl.com` | Tracker: Simkl REST API | Yes, P2 |
| `awqo5j657s-dsn.algolia.net` | Kitsu search backend (Algolia index `production_media`) | Yes, P2 (with Kitsu) |
| `bangumi.tv` | Bangumi web link for a subject (`tracking_url`) | Yes, with Bangumi |
| `bgm.tv` | Bangumi OAuth authorize and token host; README link | Yes, with Bangumi |
| `bugs.chromium.org` | Chromium bug link in a code comment (WebView init crash workaround) | Dropped (no Android WebView) |
| `cloudflare-dns.com` | DNS-over-HTTPS provider "Cloudflare" | Yes, P2 |
| `coil-kt.github.io` | Coil docs link in a comment (animated image formats) | Doc-only; replaced by gdk-pixbuf/`image` crate decision in `area:reader` |
| `contrib.rocks` | README contributors image | Drop (README decoration) |
| `controld.com` | Control D free DNS page, cited in a DoH comment | Doc-only; provider kept via `freedns.controld.com` |
| `cs.android.com` | Android source link in a comment (night-mode context) | Dropped (Android-only) |
| `data.simkl.in` | Simkl calendar JSON used for next-episode airing time | Yes, P2 (optional) |
| `developer.android.com` | Android docs links in comments, CONTRIBUTING and a benchmark README | Dropped (Android-only) |
| `discord.gg` | Community invite links (README, CONTRIBUTING, About screen) | Replace with Kurobayomi's own link or omit |
| `dns-unfiltered.adguard.com` | DoH provider "AdGuard" (unfiltered) | Yes, P2 |
| `dns.alidns.com` | DoH provider "AliDNS" | Yes, P2 |
| `dns.google` | DoH provider "Google" | Yes, P2 |
| `dns.mullvad.net` | DoH provider "Mullvad" | Yes, P2 |
| `dns.njal.la` | DoH provider "Njalla" | Yes, P2 |
| `dns.quad9.net` | DoH provider "Quad9" | Yes, P2 |
| `dns.twnic.tw` | DoH provider "Quad 101" | Yes, P2 |
| `doh.360.cn` | DoH provider "360" | Yes, P2 |
| `doh.libredns.gr` | DoH provider "LibreDNS" | Yes, P2 |
| `doh.pub` | DoH provider "DNSPod" | Yes, P2 |
| `dontkillmyapp.com` | Battery-optimisation help link in Advanced settings | Dropped (Android-only) |
| `free.shecan.ir` | DoH provider "Shecan" | Yes, P2 |
| `freedns.controld.com` | DoH provider "Control D" | Yes, P2 |
| `gist.github.com` | Attribution link in a Compose scrollbar helper comment | Dropped (Compose code not ported) |
| `github-readme-stats.vercel.app` | README pinned-repo cards | Drop (README decoration) |

## 2. Tracker hosts

### 2.1 `anilist.co`

Role: tracker "AniList" (id 2, manga and anime, `Anilist` implements `MangaTracker`, `AnimeTracker`, `DeletableMangaTracker`, `DeletableAnimeTracker`, supports reading dates and private tracking). Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/{Anilist.kt, AnilistApi.kt, AnilistInterceptor.kt, AnilistUtils.kt, dto/*.kt}`.

Constants (`AnilistApi.kt:570-574`): `CLIENT_ID = "5338"`, `API_URL = "https://graphql.anilist.co/"`, `BASE_URL = "https://anilist.co/api/v2/"`, `BASE_MANGA_URL = "https://anilist.co/manga/"`, `BASE_ANIME_URL = "https://anilist.co/anime/"`. `tracking_url` of a result is `BASE_*_URL + mediaId`.

#### Auth
* Authorize URL (`AnilistApi.kt:584-587`): `GET https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token` (implicit grant, no secret, no redirect_uri parameter: the redirect is whatever is configured on AniList's side, `aniyomi://anilist-auth`).
* The result token is in the redirect FRAGMENT (see 0.3). `Anilist.login(token)` (`Anilist.kt`): `createOAuth(token)` builds `ALOAuth(accessToken = token, tokenType = "Bearer", expires = now_ms + 31536000000, expiresIn = 31536000000)` (a fixed 365 days, the real expiry is never read), `interceptor.setAuth(oauth)`, then `getCurrentUser()`, `scorePreference.set(scoreType)` and `saveCredentials(username = viewerId.toString(), password = accessToken)`. On any Throwable: `logout()` (silent failure, no message).
* Every authenticated request (`AnilistInterceptor.kt`): throws `Exception("Not authenticated with Anilist")` if the stored token string is null or empty; loads the OAuth JSON from `track_token_2` if not loaded; if `oauth.isExpired()` (`now > expires`) calls `anilist.logout()` and throws `IOException("Token expired")`; adds headers `Authorization: Bearer <accessToken>` and `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`.
* Quirk (OBSERVED bug): the `oauth` setter stores `expires * 1000 - 60 * 1000` although `expires` is already milliseconds, so the effective client-side expiry is far in the future and the token is never treated as expired locally; expiry surfaces as an HTTP 401 `HttpException`. `setAuth` persists the un-multiplied object, `loadOAuth()` re-applies the setter on every process start.
* Proposal: store `expires_at = now + 365d` correctly, treat HTTP 401 or a GraphQL `errors[0].message == "Invalid token"` as "log out and show the re-login prompt" (message key `track_token_expired`-style, define in `area:tracking`), keep the explicit User-Agent `Kurobayomi/<version> (+https://<project url>)`.

#### Rate limit
`authClient` is built with `.rateLimit(permits = 85, period = 1.minutes)` (`AnilistApi.kt:42`), a host-agnostic sliding window (algorithm in 0.1 and the `api.manga.com` row). AniList documents 90 requests per minute; 85 leaves headroom. Proposal: same, one shared limiter object for all AniList calls (including AniSkip and AniChart lookups, which in Aniyomi bypass it).

#### Endpoint: every call is `POST https://graphql.anilist.co/` with `Content-Type: application/json; charset=utf-8` (`jsonMime`) and body `{"query": "<graphql>", "variables": {...}}`

Status mapping (`AnilistUtils.kt`, both directions in `dto/ALManga.kt`/`dto/ALAnime.kt`): internal `READING = 1`, `WATCHING = 11`, `COMPLETED = 2`, `ON_HOLD = 3`, `DROPPED = 4`, `PLAN_TO_READ = 5`, `PLAN_TO_WATCH = 15`, `REREADING = 6`, `REWATCHING = 16`. API strings: `CURRENT`, `COMPLETED`, `PAUSED`, `DROPPED`, `PLANNING`, `REPEATING`. Any other internal value throws `NotImplementedError("Unknown status: <n>")`.

| Operation | Query name and shape | Variables | Response fields consumed | Used by (`AnilistApi.kt`) |
|---|---|---|---|---|
| Add manga | `mutation AddManga($mangaId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean) { SaveMediaListEntry (mediaId: $mangaId, progress: $progress, status: $status, private: $private) { id status } }` | `mangaId = remote_id`, `progress = last_chapter_read.toInt()`, `status`, `private` | `data.SaveMediaListEntry.id` (deserialised as `ALAddEntryResult`, value discarded: returns the input track) | `addLibManga` :45 |
| Update manga | `mutation UpdateManga($listId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean, $score: Int, $startedAt: FuzzyDateInput, $completedAt: FuzzyDateInput) { SaveMediaListEntry(id: $listId, progress: $progress, status: $status, private: $private, scoreRaw: $score, startedAt: $startedAt, completedAt: $completedAt) { id status progress } }` | `listId = library_id`, `progress`, `status`, `score = track.score.toInt()` (0..100 raw), `startedAt`/`completedAt` = `{year,month,day}` or all three `null` when the stored epoch is 0 (local time zone), `private` | none (response body ignored, `awaitSuccess` only) | `updateLibManga` :81 |
| Delete manga | `mutation DeleteManga($listId: Int) { DeleteMediaListEntry(id: $listId) { deleted } }` | `listId = track.libraryId` | none | `deleteLibManga` :117 |
| Add anime | same as Add manga with `$animeId`, `progress = last_episode_seen.toInt()` | | | `addLibAnime` :138 |
| Update anime | same as Update manga, dates from `started_watching_date`/`finished_watching_date` | | | `updateLibAnime` :174 |
| Delete anime | same as Delete manga | | | `deleteLibAnime` :210 |
| Search manga | `query Search($query: String) { Page (perPage: 50) { media(search: $query, type: MANGA, format_not_in: [NOVEL]) { id staff { edges { role id node { name { full userPreferred native } } } } title { userPreferred } coverImage { large } format status chapters description startDate { year month day } averageScore } } }` | `query` | `data.Page.media[]`: `id`, `title.userPreferred`, `coverImage.large`, `description` (HTML-decoded), `format` (underscores replaced by `-`), `status`, `startDate`, `chapters`, `averageScore` (missing = -1), `staff.edges[].role/node.name` (role containing `"Story"` goes to authors, containing `"Art"` to artists; name = `userPreferred ?: full ?: native`) | `search` :231 |
| Search anime | `query Search($query: String) { Page (perPage: 50) { media(search: $query, type: ANIME) { id studios { edges { isMain node { name } } } title { userPreferred } coverImage { large } format status episodes description startDate { year month day } averageScore } } }` | `query` | same; `episodes` | `searchAnime` :293 |
| Find manga in user list | `query ($id: Int!, $manga_id: Int!) { Page { mediaList(userId: $id, type: MANGA, mediaId: $manga_id) { id status scoreRaw: score(format: POINT_100) progress private startedAt { year month day } completedAt { year month day } media { id title { userPreferred } coverImage { large } format status chapters description startDate { year month day } staff { edges { role id node { name { full userPreferred native } } } } } } } }` | `id = AniList user id (Anilist.getUsername().toInt())`, `manga_id = remote_id` | first element of `data.Page.mediaList`: `id` (becomes `library_id`), `status`, `scoreRaw`, `progress`, `private`, `startedAt`, `completedAt`, nested media | `findLibManga` :350 |
| Find anime in user list | same with `type: ANIME`, `$anime_id`, media block with `episodes` and `studios` | | | `findLibAnime` :431 |
| Current user | `query User { Viewer { id mediaListOptions { scoreFormat } } }` | none | `data.Viewer.id` (Int), `data.Viewer.mediaListOptions.scoreFormat` | `getCurrentUser` :519 |

Behaviours around the calls (`Anilist.kt`):
* Fuzzy dates: `ALFuzzyDate.toEpochMilli()` builds `LocalDate.of(year!!, month!!, day!!)` at start of day in the system zone; any null or invalid gives 0.
* Score formats (`anilist_score_type`, values `POINT_100`, `POINT_10`, `POINT_10_DECIMAL`, `POINT_5`, `POINT_3`; set from `Viewer.mediaListOptions.scoreFormat` at login). The internal score is always the 0..100 raw value (`scoreRaw`, requested as `score(format: POINT_100)`). `indexToScore(index)`: `POINT_10` index*10; `POINT_100` index; `POINT_5` 0 -> 0 else index*20-10; `POINT_3` 0 -> 0 else index*25+10; `POINT_10_DECIMAL` index. `displayScore`: `POINT_5` -> `"<(score+10)/20 as int> ★"` or `"0 ★"`; `POINT_3` -> 0 -> `"0"`, <=35 `😦`, <=60 `😐`, else `😊`; otherwise `toApiScore()` (`POINT_10`: `score.toInt()/10`, `POINT_100`: `score.toInt()`, `POINT_10_DECIMAL`: `score/10`). `get10PointScore = score / 10.0`.
* `update(track, didReadChapter)`: if `library_id` null or 0 first `findLibManga` (throws `"$track not found on user library"`); if status != COMPLETED and a chapter was read: when `last_chapter_read.toLong() == total_chapters && total_chapters > 0` set COMPLETED and `finished_reading_date = now`; else if status != REREADING set READING and, when `last_chapter_read == 1.0`, `started_reading_date = now`. Then `updateLibManga`. The anime variant uses episodes, WATCHING/REWATCHING and `*_watching_date`.
* `bind`: `findLibManga`; found: `copyPersonalFrom(remote, copyRemotePrivate = false)`, copy `library_id`, set READING if not COMPLETED/REREADING and chapters were read, then `update`; not found: status = READING if chapters read else PLAN_TO_READ, score 0, `add` (mutation Add).
* `refresh`: `findLibManga` or throw `Exception("Could not find manga")` / `"Could not find anime"`; copies personal fields, title and total.
* `delete`: library id 0/null: look it up first, return silently if not on the list.
* Init guard: a stored `anilist_score_type` of a non-string type (from API v1) triggers `logout()` and deletes the preference.

Where else AniList is called with a bare client: `AniSkipApi.getMalIdFromAL` and `AniChartApi` (see the `api.aniskip.com` and `data.simkl.in` rows). Those use the same endpoint and no token.

Doc/web links: `README.md:33` lists AniList in the tracker list (link text only). `anilist.co/manga/<id>` and `anilist.co/anime/<id>` are opened from the track sheet.

Rust: keep, `area:tracking`. `reqwest` + `serde_json`; put the GraphQL documents in `src/track/anilist/queries/*.graphql` and embed with `include_str!` (exact text above, whitespace-normalised). A tiny `governor` rate limiter (85 per minute) in front. Defer hand-written parsing: use typed `serde` structs mirroring `ALSearchItem`, `ALUserListItem`, `ALFuzzyDate` (`year`, `month`, `day` all `Option<i32>`).

Verification: unit-test `ALFuzzyDate`, status maps, score conversions (table-driven for all five formats, indices 0..=max); `wiremock` the POST body for `UpdateManga` and assert `startedAt` is `{"year":null,"month":null,"day":null}` when 0; live smoke with a throw-away AniList account, check `SaveMediaListEntry` then `DeleteMediaListEntry` and that `X-RateLimit-Remaining` response header decreases. Manual check of the implicit-grant fragment flow with the loopback page (section 0.3): the server never sees the fragment, so the loopback page must run a 5-line script that POSTs `location.hash` to `/anilist-auth/token`.

### 2.2 `bgm.tv`, `api.bgm.tv`, `bangumi.tv` (Bangumi)

Role: tracker "Bangumi" (id 5, manga and anime, no deletion support, supports private tracking). Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/{Bangumi.kt, BangumiApi.kt, BangumiInterceptor.kt, BangumiUtils.kt, dto/*.kt}`.

Constants (`BangumiApi.kt:268-277`): `CLIENT_ID = <Bangumi client id, not reproduced>`, `API_URL = "https://api.bgm.tv"`, `OAUTH_URL = "https://bgm.tv/oauth/access_token"`, `LOGIN_URL = "https://bgm.tv/oauth/authorize"`, `REDIRECT_URL = "aniyomi://bangumi-auth"`, `APP_JSON = "application/json"`. The `CLIENT_SECRET` is sent in token requests (not reproduced here).

Hosts: `bgm.tv` serves the OAuth authorize and token endpoints (and the README link `README.md:33`); `api.bgm.tv` serves the REST API; `bangumi.tv` is only used to build the user-facing subject link `https://bangumi.tv/subject/<id>` (`app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMSearch.kt:41` and `:56`).

#### Auth
* Authorize: `GET https://bgm.tv/oauth/authorize?client_id=<id>&response_type=code&redirect_uri=aniyomi%3A%2F%2Fbangumi-auth`.
* Token: `POST https://bgm.tv/oauth/access_token`, form body `grant_type=authorization_code`, `client_id`, `client_secret`, `code`, `redirect_uri=aniyomi://bangumi-auth`. Response (`dto/BGMOAuth.kt`): `access_token`, `token_type`, `created_at` (default now in seconds, `@EncodeDefault`), `expires_in`, `refresh_token` (nullable), `user_id` (nullable Long). Uses the plain client.
* Refresh: same URL, form `grant_type=refresh_token`, `client_id`, `client_secret`, `refresh_token`, `redirect_uri`. Triggered in `BangumiInterceptor` when `isExpired()` = `now_s > created_at + expires_in - 3600` (refresh one hour early). On refresh failure the response is closed and the old token is used anyway. `newAuth` re-stamps `created_at = now_s` and keeps the old `userId`.
* Every authenticated request adds `User-Agent: jmir1/Aniyomi/v<VERSION_NAME> (Android) (https://github.com/aniyomiorg/aniyomi)` and `Authorization: Bearer <accessToken>`; with no stored token throws `Exception("Not authenticated with Bangumi")`.
* `login(code)`: token request, `interceptor.newAuth`, `GET /v0/me` for the username (the API returns the user id string when the user never set a username), `saveCredentials(username, accessToken)`; any Throwable: `logout()`.
* Proposal: Bangumi's User-Agent policy requires an identifying UA; use `<github-user>/Kurobayomi/v<version> (Linux) (<repo url>)`.

#### Endpoints (all JSON, `awaitSuccess`)
| Operation | Request | Body / params | Response consumed | Code |
|---|---|---|---|---|
| Add manga/anime to collection | `POST https://api.bgm.tv/v0/users/-/collections/<remote_id>` header `Content-Type: application/json` | `{"type": <1..5>, "rate": score.toInt().coerceIn(0,10), "ep_status": last_chapter_read|last_episode_seen as Int, "private": <bool>}` | 202 Accepted, no body | `BangumiApi.kt:43-78` |
| Update collection | `PATCH https://api.bgm.tv/v0/users/-/collections/<remote_id>` same body | same | 204 No Content | `BangumiApi.kt:79-127` |
| Search manga | `POST https://api.bgm.tv/v0/search/subjects?limit=20` | `{"keyword": q, "sort": "match", "filter": {"type": [1]}}` (1 = "Book") | `BGMSearchResult{total, limit, offset, data[]}`; client-side filter `platform == null || platform == "漫画"` | `BangumiApi.kt:129-156` |
| Search anime | same with `"type": [2]` (Anime), no platform filter | | `data[]` | `BangumiApi.kt:158-184` |
| Collection status | `GET https://api.bgm.tv/v0/users/<username>/collections/<remote_id>` with `CacheControl.FORCE_NETWORK` | none | `BGMCollectionResponse{rate, type, ep_status=0, vol_status=0, private=false, subject{volumes, eps}}`; 404 means "not collected" and returns null | `BangumiApi.kt:186-236` |
| Token | see Auth | | | `BangumiApi.kt:238-254` |
| Current user | `GET https://api.bgm.tv/v0/me` | | `username` | `BangumiApi.kt:256-266` |

Search result mapping (`dto/BGMSearch.kt`): `remote_id = id`; `title = name_cn` unless blank then `name`; `cover_url = images.common`; `summary` = if `name_cn` not blank `"作品原名：" + name + "\n" + summary.trim()` else `summary.trim()`; `score = rating.score ?: -1.0`; `tracking_url = https://bangumi.tv/subject/<id>`; `total_chapters` / `total_episodes = eps`; `start_date = date` (`YYYY-MM-DD`). Fields declared: `id`, `name_cn`, `name`, `summary`, `date`, `images{common}`, `volumes`, `eps`, `rating{score}`, `platform`.

Status map (`BangumiUtils.kt`, `Bangumi.kt:203-207`): `PLAN_TO_READ = 1`, `COMPLETED = 2`, `READING = 3`, `ON_HOLD = 4`, `DROPPED = 5` (same numbers on the API: `type` 1 wish, 2 done, 3 doing, 4 on hold, 5 dropped). The anime tracker reuses the same constants (`WATCHING` is `READING`, `PLAN_TO_WATCH` is `PLAN_TO_READ`). Unknown status throws `NotImplementedError`. Scores: list `0..10` integers, `indexToScore(i) = i`, display `score.toInt()`.

Behaviours: `bind` calls `statusLib*` with the stored username; found: copy personal fields, score, progress, total; status = READING if any chapter/episode read and not COMPLETED, else keep remote status; then `update`. Not found: status READING or PLAN_TO_READ, score 0, `add` (POST). `update` completes the entry when `last == total && total > 0` (`Bangumi.kt:36-61`). `refresh` (`Bangumi.kt:136-146`) calls `statusLib*` and throws `Exception("Could not find manga")` / `"Could not find anime"` when the collection returns 404.

Rate limit: none client-side.

Rust: keep as P2. Verification: wiremock for 202/204/404 handling (404 on GET must map to "not on list", not an error); live smoke with a throw-away account; note `rate` is clamped to 0..10 even though Bangumi has 1..10 (0 clears).

### 2.3 `api.myanimelist.net`

Role: tracker "MyAnimeList" (id 1, manga and anime, deletable). Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/{MyAnimeList.kt, MyAnimeListApi.kt, MyAnimeListInterceptor.kt, MyAnimeListUtils.kt, dto/*.kt}`. Related host `myanimelist.net` (chunk 2): OAuth endpoints and web links.

Constants (`MyAnimeListApi.kt:383-388`): `CLIENT_ID = "686b980ff4240fccce7f6a654cea07ce"`, `BASE_OAUTH_URL = "https://myanimelist.net/v1/oauth2"`, `BASE_API_URL = "https://api.myanimelist.net/v2"`, `LIST_PAGINATION_AMOUNT = 250`. Search prefixes in `MyAnimeList.kt:25-26`: `SEARCH_ID_PREFIX = "id:"`, `SEARCH_LIST_PREFIX = "my:"`.

#### Auth (OAuth2 authorization code with PKCE, no secret)
* PKCE verifier: 50 random bytes (`SecureRandom`), Base64 URL-safe without padding (`app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt`), kept in a static `codeVerifier`. The `code_challenge` parameter is the verifier itself and no `code_challenge_method` is sent, i.e. method `plain` (MAL's default).
* Authorize: `GET https://myanimelist.net/v1/oauth2/authorize?client_id=<id>&code_challenge=<verifier>&response_type=code`.
* Token: `POST https://myanimelist.net/v1/oauth2/token` form `client_id`, `code`, `code_verifier`, `grant_type=authorization_code`. Response `MALOAuth{token_type, refresh_token, access_token, expires_in, created_at (default now seconds)}`; `isExpired() = created_at + (expires_in - 60) < now_s`.
* Refresh (in `MyAnimeListInterceptor`, `synchronized`): form `client_id`, `refresh_token`, `grant_type=refresh_token`, with header `Authorization: Bearer <old access token>`. HTTP 401 on refresh: `setAuthExpired()` (pref `pref_tracker_auth_expired_1` = true) and throw `MALTokenExpired` ("MAL: Login has expired"); other failure: `MALTokenRefreshFailed` ("MAL: Failed to refresh account token"). If the auth-expired flag is set, every request throws `MALTokenExpired` before the network. No auth: `IOException("MAL: User is not authenticated")`. The interceptor adds only `Authorization: Bearer <token>` (the User-Agent line is commented out in the source, so the shared default Firefox UA is sent).
* `login(code)`: token, `setAuth`, `GET /users/@me` -> `name`, `saveCredentials(name, accessToken)`; Throwable: `logout()`.

#### Endpoints
| Operation | Request | Params / body | Response consumed | Code |
|---|---|---|---|---|
| Current user | `GET /v2/users/@me` | | `name` | `MyAnimeListApi.kt:66-79` |
| Search manga | `GET /v2/manga?q=<first 64 chars>&nsfw=true` | MAL 400s above 64 chars | `data[].node.id`, then one `getMangaDetails` per id in parallel (`async`/`awaitAll`); results whose `publishing_type` contains `"novel"` are dropped | `MyAnimeListApi.kt:81-98` |
| Search anime | `GET /v2/anime?q=<64 chars>&q=<full query>&nsfw=true` (OBSERVED quirk: `q` is appended twice; the second, untruncated value is what MAL sees last) | | `data[].node.id` then `getAnimeDetails` each | `MyAnimeListApi.kt:100-117` |
| Manga details | `GET /v2/manga/<id>?fields=id,title,synopsis,num_chapters,mean,main_picture,status,media_type,start_date` | | `MALManga{id, title, synopsis="", num_chapters, mean=-1.0, main_picture{large=""}, status, media_type, start_date}`; `publishing_status = status` and `publishing_type = media_type` with `_` replaced by space; `tracking_url = https://myanimelist.net/manga/<id>` | `MyAnimeListApi.kt:119-148` |
| Anime details | `GET /v2/anime/<id>?fields=id,title,synopsis,num_episodes,mean,main_picture,status,media_type,start_date` | | analogous, `https://myanimelist.net/anime/<id>` | `MyAnimeListApi.kt:150-179` |
| Update manga list entry (also used for Add) | `PUT /v2/manga/<id>/my_list_status` form body | `status` (`reading`,`completed`,`on_hold`,`dropped`,`plan_to_read`; default `reading`), `is_rereading` = `status == REREADING`, `score` = `track.score.toString()` (a Double string like `8.0`), `num_chapters_read` Int, optional `start_date`, `finish_date` as `yyyy-MM-dd` (Locale.US, local zone; epoch 0 sends an EMPTY string, which clears the date) | `MALListMangaItemStatus{is_rereading, status, num_chapters_read (Double), score (Int), start_date?, finish_date?}` | `MyAnimeListApi.kt:181-206` |
| Update anime list entry | `PUT /v2/anime/<id>/my_list_status` form: `status` (`watching`, ...`plan_to_watch`), `is_rewatching`, `score`, `num_watched_episodes`, dates | | `MALListAnimeItemStatus{is_rewatching, status, num_episodes_watched, score, start_date, finish_date}` | `MyAnimeListApi.kt:208-233` |
| Delete manga/anime entry | `DELETE /v2/manga/<id>/my_list_status`, `DELETE /v2/anime/<id>/my_list_status` | | none | `MyAnimeListApi.kt:235-249` |
| Find list entry | `GET /v2/manga/<id>?fields=num_chapters,my_list_status{start_date,finish_date}` (anime: `num_episodes`) | | `num_chapters` -> `total_chapters`; `my_list_status` null means not on list | `MyAnimeListApi.kt:251-285` |
| Search my list ("my:" prefix) | `GET /v2/users/@me/mangalist?fields=list_status{start_date,finish_date}&limit=250[&offset=<n>]` (the same `mangalist` path is used for anime in `findListItemsAnime`, OBSERVED bug: it queries the MANGA list) | pagination follows `paging.next` non-blank, offset += 250 | `data[].node.{id,title}` filtered by case-insensitive `contains(query)`, then details for each | `MyAnimeListApi.kt:287-342` |

Status ids (`MyAnimeList.kt:15-23`): `READING 1`, `WATCHING 11`, `COMPLETED 2`, `ON_HOLD 3`, `DROPPED 4`, `PLAN_TO_READ 6`, `PLAN_TO_WATCH 16`, `REREADING 7`, `REWATCHING 17`. Reverse map `getStatus(String?)` defaults to `READING` for unknown. A server item with `is_rereading=true` is mapped to REREADING/REWATCHING (`parseMangaItem`, `parseAnimeItem`). Score list `0..10`, `indexToScore(i) = i`, display `score.toInt()`.

Behaviours: `update` marks COMPLETED with `finished_*_date = now` when `last == total && total > 0`, otherwise READING (and `started_*_date = now` when last is 1.0) unless REREADING; `bind` finds the list item (found: copy personal fields, WATCHING/READING if read and not COMPLETED/REREADING, then `update`; else add with READING or PLAN_TO_READ); `refresh` = `findListItem ?: add(track)`.

Rate limit: none client-side. MAL returns 429/403 when abused; Proposal: a 2 requests/second limiter and a retry-once-after-`Retry-After`.

Rust: keep, P1. Use the `oauth2` crate (PKCE plain via `PkceCodeChallenge::from_code_verifier_plain`... or hand-rolled, it is 20 lines). Verification: unit-test PKCE verifier length (50 bytes -> 67 chars), `wiremock` refresh race (two concurrent calls with an expired token must produce ONE refresh request), form-encoding test that epoch 0 yields `start_date=` (empty), live smoke.

### 2.4 `api.mangaupdates.com`

Role: tracker "MangaUpdates" (id 7, manga only, deletable). Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/{MangaUpdates.kt, MangaUpdatesApi.kt, MangaUpdatesInterceptor.kt, dto/*.kt}`.

Constants (`MangaUpdatesApi.kt:193-196`): `BASE_URL = "https://api.mangaupdates.com"`, `CONTENT_TYPE = "application/vnd.api+json".toMediaType()` (used for every JSON request body).

Auth: username and password login, no OAuth. `PUT /v1/account/login` body `{"username":..., "password":...}` -> `MULoginResponse.context{session_token, uid}`. `saveCredentials(uid.toString(), session_token)` and `interceptor.newAuth(token)`. If the call returns no context: `Throwable("Unable to login")`. The password is not stored. Every authenticated call adds `Authorization: Bearer <session_token>` and `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`; no token: `IOException("Not authenticated with MangaUpdates")`. There is no refresh; on expiry calls fail with HTTP 401 (OBSERVED: no handling, the user must log in again).

List ids (`MangaUpdates.kt:11-15`): `READING_LIST = 0`, `WISH_LIST = 1`, `COMPLETE_LIST = 2`, `UNFINISHED_LIST = 3`, `ON_HOLD_LIST = 4`. The internal status equals the MangaUpdates `list_id`. Menu order: reading, complete, on hold, unfinished, wish. Rereading status = -1 (none). Score list (`MangaUpdates.kt:29-39`): `"-"` first, then `"<d>.<f>"` for d in 1..9 and f in 0..9 (so `1.0`, `1.1`, up to `9.9`, 90 entries), then `"10.0"`; 92 entries in total. `indexToScore(0) = 0.0`, else `SCORE_LIST[index].toDouble()`; display `score.toString()`.

| Operation | Request | Body / params | Response consumed | Code |
|---|---|---|---|---|
| Get list entry | `GET /v1/lists/series/<remote_id>` | | `MUListItem{series{id,title}, list_id, status{volume, chapter}, priority}`: internal status = `list_id ?: 0`, `last_chapter_read = status.chapter ?: 0` | `MangaUpdatesApi.kt:43-53` |
| Get rating | `GET /v1/series/<id>/rating` | | `MURating{rating}` (any exception -> null, score 0) | `MangaUpdatesApi.kt:116-126` |
| Add to list | `POST /v1/lists/series` | `[{"series":{"id":<id>},"list_id":<0 if chapters read else 1>}]` | on HTTP 200 sets `status = list_id` and `last_chapter_read = 1.0` locally | `MangaUpdatesApi.kt:55-78` |
| Update list entry | `POST /v1/lists/series/update` | `[{"series":{"id":<id>},"list_id":<status>,"status":{"chapter":<last_chapter_read.toInt()>}}]` then rating update | none | `MangaUpdatesApi.kt:80-101` |
| Rating set | `PUT /v1/series/<id>/rating` | `{"rating": <score Double>}` when score > 0 | none | `MangaUpdatesApi.kt:128-147` |
| Rating clear | `DELETE /v1/series/<id>/rating` when score == 0.0 (negative scores skip the call) | | none | |
| Delete from list | `POST /v1/lists/series/delete` | `[<remote_id>]` | none | `MangaUpdatesApi.kt:103-114` |
| Search | `POST /v1/series/search` (plain client) | `{"search": q, "filter_types": ["drama cd","novel"]}` | `results[].record{series_id, title, url, description, image{url{original}}, type, year, bayesian_rating, rating_votes, latest_chapter}`; mapped: `title`/`description` HTML-decoded, `cover_url = image.url.original`, `tracking_url = url`, `publishing_type = type`, `start_date = year`, `total_chapters = 0`, `publishing_status = ""` | `MangaUpdatesApi.kt:149-173` |

Behaviours: `update`: if status != COMPLETE_LIST and a chapter was read set READING_LIST; then `updateSeriesListItem`. `bind`: `getSeriesListItem` ok -> copy list item and rating; any exception -> score 0 and `addSeriesToList(track, hasReadChapters)`. `refresh` = `getSeriesListItem`. No rate limit client-side (MangaUpdates asks for conservative use).

Rust: keep, P2. Verification: wiremock asserting the three array-shaped bodies; the content type `application/vnd.api+json` must be sent exactly.

### 2.5 `api.simkl.com`

Role: tracker "Simkl" (id 101, ANIME ONLY, not deletable). Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/{Simkl.kt, SimklApi.kt, SimklInterceptor.kt, SimklUtils.kt, dto/*.kt}`.

Constants (`SimklApi.kt:237-246`): `CLIENT_ID = "aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e"`, `BASE_URL = "https://simkl.com"`, `API_URL = "https://api.simkl.com"`, `OAUTH_URL = "$API_URL/oauth/token"`, `LOGIN_URL = "$BASE_URL/oauth/authorize"`, `POSTERS_URL = "https://simkl.in/posters/"`, `REDIRECT_URL = "aniyomi://simkl-auth"`.

Auth: authorize `GET https://simkl.com/oauth/authorize?response_type=code&client_id=<id>&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth`; token `POST https://api.simkl.com/oauth/token` JSON `{"code","client_id","client_secret","redirect_uri","grant_type":"authorization_code"}` -> `SimklOAuth{access_token}` (no refresh, no expiry). Authenticated calls add `Authorization: Bearer <token>`, `simkl-api-key: <CLIENT_ID>`, `User-Agent: Aniyomi v...`; no token: `Exception("Not authenticated with Simkl")`. Login then `GET /users/settings` -> `account.id`, `saveCredentials(id.toString(), token)`. (OBSERVED: `getCurrentUser` uses `runBlocking`.)

Status ids (`Simkl.kt:11-15`): `WATCHING 1`, `COMPLETED 2`, `ON_HOLD 3`, `NOT_INTERESTING 4`, `PLAN_TO_WATCH 5`; API strings (`SimklUtils.kt`): `watching`, `completed`, `hold`, `notinteresting`, `plantowatch`; reverse also maps `dropped` to NOT_INTERESTING. Score list `0..10` ints; `getRewatchingStatus() = 0`.

| Operation | Request | Body / params | Response consumed | Code |
|---|---|---|---|---|
| Search | `GET https://api.simkl.com/search/<type>?q=<q>&extended=full&client_id=<CLIENT_ID>` for `type` in `anime`, `tv`, `movie` (three calls, concatenated in that order; plain client) | | `[{ids{simkl_id}, title_romaji, title, ep_count, poster, all_titles[], url, status, type, year}]`; `remote_id = ids.simkl_id`, `title = title_romaji ?: title`, `total_episodes = ep_count ?: 1`, `cover_url = https://simkl.in/posters/<poster>_m.webp`, `summary = "All titles:\n" + all_titles.join("\n")`, `tracking_url = url`, `publishing_status = status ?: "ended"`, `publishing_type = type ?: <requested type>`, `start_date = year` | `SimklApi.kt:142-160` |
| Add to list | `POST /sync/add-to-list` | `{"<shows|movies>":[{"ids":{"simkl":<id>},"to":"<status>"}]}` (media type is `movies` only when the first path segment of `tracking_url` is `movies`, else `shows`) | none | `SimklApi.kt:36-62` |
| Remove watched history | `POST /sync/history/remove` | `{"shows":[{"ids":{"simkl":id},"seasons":[{"number":1}]}]}` (season 1, no episodes = remove all) | none | `SimklApi.kt:87-96` |
| Add watched history | `POST /sync/history` | `{"shows":[{"ids":{"simkl":id},"seasons":[{"number":1,"episodes":[{"number":1},...,{"number":last_episode_seen}]}]}]}` | none | |
| Set rating | `POST /sync/ratings` `{"<type>":[{"ids":{"simkl":id},"rating":<score.toInt()>}]}`; when score is 0: `POST /sync/ratings/remove` same body | | none | `SimklApi.kt:64-85` |
| Check watched | `POST /sync/watched` `[{"simkl":<id>}]` | | `[{result, last_watched, list}]`; requires `result == true`, `last_watched` and `list` not null | `SimklApi.kt:162-202` |
| Fetch list item | `GET /sync/all-items/<anime|tv->shows|movies>/<list status>?date_from=<last_watched>` | | `SimklSyncResult{anime[], tv[], movies[]}`; select the entry with `show|movie.ids.simkl == remote_id`; `total_episodes = total_episodes_count`, `last_episode_seen = watched_episodes_count`, `score = user_rating`; movies: total 1, seen 1 only when status completed | `SimklApi.kt:162-202` |

`update` order (`updateLibAnime`): for shows `updateProgress` (remove all, then add 1..N), then `addToList`, then `updateRating`. This is O(N) request body size and 3 to 4 requests per progress change. Simkl `tracking_url` of a track is stored as `/<type>/<simkl id>` (from the sync result) and parsed with `substringAfter("/").substringBefore("/")`.

`Simkl.update` completes the entry when `last == total && total > 0`, else WATCHING when an episode was watched. `bind`: found -> copy personal fields, WATCHING if episodes seen, then `update`; not found -> WATCHING or PLAN_TO_WATCH, score 0, `add`.

Rate limit: none client-side. Rust: keep as P2 (AniList and MAL cover most users). Proposal: reduce progress sync to a single `POST /sync/history` of the NEW episode range instead of remove-all and re-add. Verification: wiremock with a 200-episode track to confirm body size, live smoke.

### 2.6 `awqo5j657s-dsn.algolia.net` (Kitsu search)

Role: search backend for tracker "Kitsu" (id 3, manga and anime, deletable). The constant is written upper-case in the source (`AWQO5J657S-dsn.algolia.net`); hostnames are case-insensitive. Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/{Kitsu.kt, KitsuApi.kt, KitsuInterceptor.kt, KitsuUtils.kt, KitsuDateHelper.kt, dto/*.kt}`. The rest of Kitsu goes to `kitsu.app` (chunk 2).

Constants (`KitsuApi.kt:428-452`): `CLIENT_ID = "dd031b32d2f56c990b1425efe6c42ad847e7fe3ab46bf1299f05ecd856bdb7dd"` (public Kitsu app credentials, secret not reproduced), `BASE_URL = "https://kitsu.app/api/edge/"`, `LOGIN_URL = "https://kitsu.app/api/oauth/token"`, `BASE_MANGA_URL = "https://kitsu.app/manga/"`, `BASE_ANIME_URL = "https://kitsu.app/anime/"`, `ALGOLIA_KEY_URL = "https://kitsu.app/api/edge/algolia-keys/media/"`, `ALGOLIA_URL = "https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/"`, `ALGOLIA_APP_ID = "AWQO5J657S"`, `VND_API_JSON = "application/vnd.api+json"`.

#### Search flow (2 requests)
1. `GET https://kitsu.app/api/edge/algolia-keys/media/` with the AUTH client (needs a logged-in Kitsu token) -> `{"media":{"key":"<search-only api key>"}}` (`KitsuSearchResult.media.key`).
2. `POST https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/` (plain client) headers `X-Algolia-Application-Id: AWQO5J657S`, `X-Algolia-API-Key: <key from 1>`, `Content-Type: application/json; charset=utf-8`; body `{"params": "query=<url-encoded query><filter>"}` where `<filter>` is
   * manga: `&facetFilters=%5B%22kind%3Amanga%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22chapterCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D`
   * anime: same with `kind%3Aanime` and `episodeCount` instead of `chapterCount`.
   (Decoded: facet filter `["kind:manga"]` / `["kind:anime"]`, attributes `["synopsis","averageRating","canonicalTitle","chapterCount"|"episodeCount","posterImage","startDate","subtype","endDate", "id"]`.)
   Response `{"hits":[{id, canonicalTitle, chapterCount, episodeCount, subtype, posterImage{original}, synopsis, averageRating, startDate, endDate}]}`; hits with `subtype == "novel"` are dropped. Mapping: `remote_id = id`, `title = canonicalTitle`, `total_* = count ?: 0`, `cover_url = posterImage.original`, `summary = synopsis`, `tracking_url = https://kitsu.app/<manga|anime>/<id>`, `score = averageRating ?: -1.0`, `publishing_status = "Publishing"` if `endDate == null` else `"Finished"`, `publishing_type = subtype`, `start_date` = `yyyy-MM-dd` (Locale.US) of `startDate * 1000` (Unix seconds).

#### The rest of the Kitsu API (for completeness, hosted on `kitsu.app`)
* Login: `POST https://kitsu.app/api/oauth/token` form `grant_type=password`, `username`, `password`, `client_id`, `client_secret`; refresh form `grant_type=refresh_token`, `refresh_token`, client id and secret. `KitsuOAuth{access_token, token_type, created_at, expires_in, refresh_token}`, refreshed one hour early (`now_s > created_at + expires_in - 3600`). Authenticated requests add `Authorization: Bearer`, `User-Agent: Aniyomi v...`, `Accept: application/vnd.api+json`, `Content-Type: application/vnd.api+json`. After login `GET /api/edge/users?filter[self]=true` -> `data[0].id` (string) stored as the "password".
* Library: add `POST /api/edge/library-entries` (JSON:API body `data{type:"libraryEntries", attributes{status, progress, private}, relationships{user{data{id,type:"users"}}, media{data{id,type:"manga"|"anime"}}}}`, response `data.id` becomes `remote_id`); update `PATCH /library-entries/<id>` with `attributes{status, progress, ratingTwenty, startedAt, finishedAt, private}`; delete `DELETE /library-entries/<id>`; find `GET /library-entries?filter[manga_id]=<id>&filter[user_id]=<uid>&include=manga` (anime: `anime_id`, `include=anime`); refresh `GET /library-entries?filter[id]=<id>&include=manga|anime`.
* Status strings: `current`, `completed`, `on_hold`, `dropped`, `planned`; internal ids `READING 1`, `WATCHING 11`, `COMPLETED 2`, `ON_HOLD 3`, `DROPPED 4`, `PLAN_TO_READ 5`, `PLAN_TO_WATCH 15`. Score: `ratingTwenty = (score * 2).toInt()` as a string when `score > 0` else null; read back as `ratingTwenty / 2.0`; score list `"0"` then `1.0`..`10.0` in 0.5 steps (`DecimalFormat("0.#")` of `it/2f` for 2..20). Dates `yyyy-MM-dd'T'HH:mm:ss.SSS'Z'` (`Locale.ENGLISH`, formatter default zone, OBSERVED: uses the device zone but labels it Z); epoch 0 sends JSON null.

Rate limit: none client-side. Algolia free-tier key is read-only search.

Rust: keep as P2 with the tracker. Key point: the Algolia key must be fetched per search (the code does not cache it); Proposal: cache the key for 1 hour and refetch once on a 403 from Algolia. Verification: wiremock for the two-step flow and the exact `params` string (URL-encoded query concatenated with the filter constant, no extra encoding of the filter); live check that a query returns hits with `kind` manga only.

## 3. Player-adjacent and app services

### 3.1 `api.aniskip.com`

Role: community skip-times API (openings, endings, recaps) for the video player. File: `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt`, called from `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2031-2056` (`aniSkipResponse`) and `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:1280-1308` (`fileLoaded`).

Request (`AniSkipApi.kt:22-25`, `getResult(malId, episodeNumber, episodeLength)`): `GET https://api.aniskip.com/v2/skip-times/<malId>/<episodeNumber>?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength=<episodeLength>`. No auth, no custom headers, bare client (0.1). `episodeLength` is the player duration in whole SECONDS as Long (`player.duration`, an Int). `episodeNumber` is `episode_number.toInt()` (so 12.5 becomes 12; episode 0 is requested as 0).

Response model (`AniSkipApi.kt:68-117`): `{"found": bool, "results": [{"interval": {"startTime": Double, "endTime": Double}, "skipType": "op"|"ed"|"recap"|"mixed-op"}]}`. `found == false` or an exception (network, parse, HTTP error body) returns null silently. Result mapping to `TimeStamp(start, end, name, type)`: names `"Opening"`, `"Ending"`, `"Recap"`, `"Mixed-op"`; `ChapterType` values `Opening`, `Ending`, `Recap`, `MixedOp` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:14-20`, also `Other`).
* OBSERVED defect: the request asks for `mixed-ed` but the enum has no `mixed-ed` member and the JSON config has no `coerceInputValues`, so ONE `mixed-ed` result makes the entire response fail to decode and the player gets no skip times at all.
* Proposal: accept unknown `skipType` values (`#[serde(other)]`), map `mixed-ed` to a new `MixedEd` ChapterType named "Mixed-ed", and skip only the unknown ones.

MAL id source (`PlayerViewModel.kt:2031-2056`): requires at least one track on the anime; for the tracks in order, `MyAnimeList` track -> `remoteId` is the MAL id; `Anilist` track -> `AniSkipApi.getMalIdFromAL(remoteId)`; other trackers -> null. NOTE the loop RETURNS on the first track it examines (`return malId?.let {...}` inside `map`), so only the first track is ever tried, whichever tracker it belongs to; if that first track is Kitsu/Simkl the result is null even when a MAL track exists. Returns null if the player duration is null.

AniList to MAL id (`getMalIdFromAL`, `AniSkipApi.kt:46-65`): `POST https://graphql.anilist.co` JSON `{"query":"query{\nMedia(id:<id>){idMal}\n}"}` (note the literal newline escapes produced by `trimMargin`), parsed with `body.substringAfter("idMal\":").substringBefore("}").toLongOrNull() ?: 0`. A null `idMal` yields `0`, then `getResult(0, ...)` runs against AniSkip and normally returns `found:false`.

Gating (`PlayerActivity.kt:1286-1296`, `PlayerPreferences.kt:48-56`): the lookup runs in `fileLoaded` only when `pref_enable_skip_intro` (default true) AND `pref_enable_ani_skip` (default false) AND NOT (`pref_disabled_ani_skip_chapters` (default true) AND the file already has chapters). `fileLoaded` is launched with `viewModel.viewModelScope.launchIO` (`PlayerActivity.kt:757`) and the lookup is wrapped in `runBlocking`, i.e. it blocks that IO thread (not the UI thread) until the HTTP call finishes (10 s read timeout worst case), so chapters, tracks and the skip button wait for it. Merge: `ChapterUtils.mergeChapters(currentChapters, stamps, duration)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/ChapterUtils.kt:21-70`): stamps sorted by start; first start under 1.0 s snaps to 0.0; a closing empty chapter (index -1) is inserted at `end` unless `abs(end - duration) <= 1.0` or the next stamp starts within 1.0 s of this end; existing player chapters within 1.0 s of an external start are replaced; external chapter colour `0xFFD8BBDF` for non-Other types. Skip behaviour then follows `pref_enable_auto_skip_ani_skip` (default false), `pref_enable_netflix_style_aniskip` and `pref_waiting_time_aniskip` (default 5 s) (`PlayerViewModel.kt:2059-2110`).

Rate limit: none (AniSkip is a free community service; be gentle).

Rust: keep, P2, in `area:player`. Do the lookup asynchronously (tokio task) when the episode starts and merge the chapters when it returns instead of blocking; cache the result per `(malId, episode, duration)` in the SQLite DB for 7 days (negative results for 1 day). mpv: add the merged chapters through the `chapter-list` property (`set_property("chapter-list", [...])` via libmpv) and drive the skip button from the property observer. Verification: unit tests for `mergeChapters` with the 1.0 s snaps, a fixture JSON including a `mixed-ed` entry, and a manual check on a known MAL id (for example the first episode of a popular series) comparing the returned `interval` to the real opening.

### 3.2 `data.simkl.in` (and the AniList/MAL id lookups, "AniChart")

Role: next-episode airing time shown on the anime detail screen ("Upcoming"). File: `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt`, called from `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1495-1545` (`updateAiringTime`). Bare `OkHttpClient()`.

Flow (`loadAiringTime(anime, trackItems, manualFetch)`):
* If `anime.status == SAnime.COMPLETED` and not a manual fetch, return the stored `(nextEpisodeToAir, nextEpisodeAiringAt)`.
* Pick the FIRST tracker item that has a bound track and whose tracker is `Anilist`, `MyAnimeList` or `Simkl`; none: `Pair(1, 0L)`.
* `Anilist`: use its remote id directly. `MyAnimeList`: map to the AniList id first. `Simkl`: calendar lookup below. Result is written with `setAnimeViewerFlags.awaitSetNextEpisodeAiring` and shown when `show_next_episode_airing_time` (default true).

AniList GraphQL (`POST https://graphql.anilist.co`, JSON `{"query": ...}`, no token):
* MAL -> AniList id: `query { Media(idMal:<idMal>,type: ANIME) { id } }`, parsed with `substringAfter("id\":").substringBefore("}").toLongOrNull() ?: 0L`.
* Airing: `query { Media(id:<id>) { nextAiringEpisode { episode airingAt } } }`, parsed with `substringAfter("episode\":").substringBefore(",").toIntOrNull() ?: 1` and `substringAfter("airingAt\":").substringBefore("}").toLongOrNull() ?: 0L` (Unix seconds). Failure: `Pair(1, 0L)`.

Simkl calendar (`getSimklAiringEpisodeData`, `AniChartApi.kt:109-148`): for each of `anime`, `tv`, `movie_release` in this order: `GET https://data.simkl.in/calendar/<type>.json` (no auth; the CDN serves the public calendar of upcoming releases), then `removeAiredSimkl` keeps the text after the first entry whose `date` is in the future; the Simkl id is located by the text `"simkl_id":<id>,`; if the entry carries a `"mal":"<id>"` field the AniList lookup path is used; otherwise episode number (`"episode":` before `"}}`) and the preceding `"date":"<ISO offset date-time>"` are returned as `(episode, epochSeconds)`.
* OBSERVED defects: all parsing is substring based on raw JSON text (breaks on whitespace or key reordering); `removeAiredSimkl` passes the LIST index of the first future entry to `body.substring(index)` as a CHARACTER offset, so it normally discards only a few characters instead of the aired entries; a network exception on the first calendar type aborts with `(1, 0)` without trying the others.
* Proposal: parse JSON with `serde_json` (`Vec<CalendarEntry>`), find `ids.simkl_id == id`, read `date` (RFC 3339) and `episode.episode`; verify the JSON schema against a live download before relying on the field names, because the code never decodes it structurally. Reuse `area:12-network-stack` client. Cache calendar JSON for 6 hours.

Rust: keep, P2 (nice-to-have). Verification: fixture-based tests for the AniList airing response (including `nextAiringEpisode: null`), the MAL to AniList lookup, a Simkl calendar fixture downloaded once.

### 3.3 `api.github.com`

Role: application update check. Files: `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt`, `data/src/main/java/tachiyomi/data/release/GithubRelease.kt`, `domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:370-385`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt` (manual "Check for updates").

Request (`ReleaseServiceImpl.kt:21`): `GET https://api.github.com/repos/<repository>/releases/latest` through the shared client (so it carries the 10-minute `CacheControl`, the Firefox User-Agent and DoH). `repository` is `aniyomiorg/aniyomi` for release builds and `aniyomiorg/aniyomi-preview` for preview builds (`AppUpdateChecker.kt:43-47`). No auth header, so the unauthenticated GitHub limit applies (60 requests/hour per IP).

Response fields consumed (`GithubRelease.kt`): `tag_name`, `body`, `html_url`, `assets[].name`, `assets[].browser_download_url`. The release `body` has GitHub usernames turned into markdown links: regex `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` (case-insensitive) replaced by `[@name](https://github.com/name)`. The download link picks the asset whose name contains `-arm64-v8a`, `-armeabi-v7a`, `-x86_64` or `-x86` matching `Build.SUPPORTED_ABIS[0]`, else the asset with no ABI suffix; none: no update (`null`).

Decision logic (`GetApplicationRelease.kt`): unless `forceCheck`, skip if less than 3 days (`ChronoUnit.DAYS`) since app-state key `last_app_check` (long, default 0, `Preference.appStateKey`). The key is written only after a successful fetch. New version test: strip every non-digit/non-dot char from tag and from `BuildConfig.VERSION_NAME`; preview builds compare the integer tag against `COMMIT_COUNT`; release builds split on `.`, and return true if ANY component of the new version is greater than the same-position component of the old one. OBSERVED bugs: this is not lexicographic (`0.1.9` vs installed `0.2.0` reports an update because 9 > 0), and a tag with FEWER components than the installed version (for example `0.19` against `0.18.2.1`) throws `IndexOutOfBoundsException` at `newSemVer[index]`, which `MainActivity` catches and logs (`MainActivity.kt:389-391`) and the About screen reports as an error. Automatic checks run at start only when `updaterEnabled` (`app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt:7`); the app version is `0.18.2.1` (`app/build.gradle.kts:23`). Release page links: `https://github.com/<repo>/releases/tag/<tag>` (`AppUpdateChecker.kt:59`).

Proposal: Linux distribution is through the user's package manager or `cargo install`; there is no in-app APK download. Keep ONLY a P3 "check for new version" action: `GET https://api.github.com/repos/<owner>/kurobayomi/releases/latest`, compare with `semver::Version` (proper ordering), show a toast/inline row in About with a link to `html_url`, rate-limited to once per 3 days unless forced, `Accept: application/vnd.github+json` and `User-Agent: Kurobayomi/<version>` (GitHub rejects requests without a UA). Dropped: automatic download and installer (`AppUpdateDownloadJob`), ABI asset selection, preview channel; the user loses one-tap updates, replaced by the package manager. Verification: unit-test version comparison table (`0.1.9` < `0.2.0`), a fixture of the releases/latest response, and a test that a 403 rate-limit response is shown as "could not check" without crashing.

### 3.4 `api.resmush.it` (data saver)

Role: one of three optional image "data saver" back ends (`DataSaver` enum: `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT`; preference `data_saver`, default `NONE`). File: `app/src/main/java/aniyomi/util/DataSaver.kt`. MANGA ONLY: `DataSaver(source: MangaSource, ...)` is used by `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt:51` and by the downloader (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:374-375` when `data_saver_downloader` is true, default true).

Preferences (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:97-116`): `ignore_jpeg` (false), `ignore_gif` (true), `data_saver_image_quality` (Int, 80; choices 10, 20, 40, 50, 70, 80, 90, 95), `data_saver_image_format_jpeg` (false), `data_saver_server` (string, `""`), `data_saver_color_bw` (false), `data_saver_excluded` (string set of source ids as decimal strings, empty), `data_saver_downloader` (true). A source id in `data_saver_excluded` gets `NoOp` whenever the saver is not `NONE`.

reSmush.it request (`DataSaver.kt:165-169`): `GET http://api.resmush.it/ws.php?img=<imageUrl>&qlty=<quality>` (PLAIN HTTP, imageUrl NOT percent-encoded, executed synchronously with the shared client). The returned compressed-image URL is cut from the body text with `substringAfter("\"dest\":\"").substringBefore("\",")`; the reSmush JSON escapes slashes (`http:\/\/...`), which OkHttp's URL parser tolerates (backslash is treated as slash). On an error response (reSmush returns `{"error":...}`) the substring logic returns the whole body, producing an invalid URL and a failed page load. The format switch (JPEG/WebP) is ignored for reSmush (the setting is disabled in the UI, `SettingsAdvancedScreen.kt` around line 535).

URL-selection rules shared by all three savers (`DataSaver.kt:68-77, 101-110, 153-162`): if the URL contains `.jpeg` or `.jpg` (case-insensitive) and `ignore_jpeg` is on, keep the original; else if it contains `.gif` and `ignore_gif` is on, keep; otherwise compress. `HttpSource.getImage(page, dataSaver)` temporarily replaces `page.imageUrl` with the compressed URL, fetches through the SOURCE's own client and headers (so source Referer headers are sent to the proxy too), then restores the original URL.

Other two back ends, for the full picture: Bandwidth Hero: `<data_saver_server trimmed of trailing '/'>/?jpg=<0|1>&l=<quality>&bw=<0|1>&url=<url-encoded>` (skipped when the server is blank or already contained in the URL); wsrv.nl: `https://wsrv.nl/?url=<url>` + `&q=<quality>&n=-1` and `&output=jpg` for animated webp/gif when JPEG is requested, otherwise `&output=jpg&q=<q>` or `&output=webp&q=<q>` (`wsrv.nl` is a chunk 2 host).

Proposal: P2, `area:reader`, `area:downloads`. Keep Bandwidth Hero (user's own server) and wsrv.nl; implement reSmush with HTTPS (`https://api.resmush.it/ws.php?img=<encoded url>&qlty=<q>`) and real JSON parsing (`{"dest": "..."}` or `{"error": n, "error_long": "..."}`), falling back to the original URL on error or when the proxy request fails, plus a timeout of 15 s. Privacy note to show in the setting's subtitle: the page URL (and for reSmush the full image URL) is sent to a third party. Verification: table tests for the three URL builders using the Kotlin examples above; fixture tests for the reSmush success/error bodies.

### 3.5 `anidex.moe`

Role: default entry in the BitTorrent tracker list passed to the embedded TorrServer (`pref_torrserver_tackers`, note the misspelling in the key). Source `core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt:13-41`; line 16 is `http://anidex.moe:6969/announce`. The default list is a newline-separated string with all spaces removed (`.replace(" ", "")`) and contains these 26 announce URLs: `http://nyaa.tracker.wf:7777/announce`, `http://anidex.moe:6969/announce`, `http://tracker.anirena.com:80/announce`, `udp://tracker.uw0.xyz:6969/announce`, `http://share.camoe.cn:8080/announce`, `http://t.nyaatracker.com:80/announce`, `udp://47.ip-51-68-199.eu:6969/announce`, `udp://9.rarbg.me:2940`, `udp://9.rarbg.to:2820`, `udp://exodus.desync.com:6969/announce`, `udp://explodie.org:6969/announce`, `udp://ipv4.tracker.harry.lu:80/announce`, `udp://open.stealth.si:80/announce`, `udp://opentor.org:2710/announce`, `udp://opentracker.i2p.rocks:6969/announce`, `udp://retracker.lanta-net.ru:2710/announce`, `udp://tracker.cyberia.is:6969/announce`, `udp://tracker.dler.org:6969/announce`, `udp://tracker.ds.is:6969/announce`, `udp://tracker.internetwarriors.net:1337`, `udp://tracker.openbittorrent.com:6969/announce`, `udp://tracker.opentrackr.org:1337/announce`, `udp://tracker.tiny-vps.com:6969/announce`, `udp://tracker.torrent.eu.org:451/announce`, `udp://valakas.rollo.dnsabr.com:2710/announce`, `udp://www.torrent.eu.org:451/announce`.

Usage: `TorrentServerUtils.setTrackersList()` (`core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt:13-18`) splits on `\n`, joins with `",\n"` and calls `TorrServer.addTrackers(...)` of the embedded Go library `xyz.secozzi.torrserver`. The server then listens on `http://127.0.0.1:<pref_torrserver_port, default "8090">`. Endpoints used by the app (`TorrentServerApi.kt`): `GET /echo` (liveness), `POST /torrents` JSON `{"action":"add","link":...,"title":...,"poster":...,"data":...,"save_to_db":<bool>}`, `POST /torrent/upload` multipart (`file`, `save`, `title`), and play URL `http://127.0.0.1:<port>/stream/<urlencoded filename>?link=<hash>&index=<file index>&play`. Other preferences: `pref_torrserver_enable` (false), `pref_torrserver_shownotice` (false), `pref_torrserver_proxymode` (enum `ProxyMode`, default `None`; values `None("tracker")` (OBSERVED: same wire value as `Tracker`), `Tracker("tracker")`, `Peers("peers")`, `Full("full")`), `pref_torrserver_proxyurl` (""). Several of these trackers are dead (RARBG closed in 2023); the list is only a hint list.

Proposal: P3, `area:player`. Do not embed a torrent engine. If the user wants torrent playback, support an external `torrserver` binary (the `TorrServer` project is packaged in the AUR) that Kurobayomi starts as a child process or connects to on `127.0.0.1:8090`, using the same four endpoints; ship the tracker list as a plain editable text setting with this default. What the user loses: no zero-setup torrent streaming. Verification: start a local `torrserver`, hit `/echo`, add a public-domain magnet, open the `/stream` URL in mpv and check playback.

## 4. DNS over HTTPS providers (13 selectable providers, 14 hosts)

Files: `core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt`, selection in `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:42-57`, preference `doh_provider` (Int, default `-1` = disabled, `NetworkPreferences.kt`), UI `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:259-288` (a list preference; changing it toasts `requires_app_restart`), migration `app/src/main/java/mihon/core/migration/migrations/DOHMigration.kt` (version 57: old boolean `enable_doh` true becomes `doh_provider = PREF_DOH_CLOUDFLARE`, the old key is removed).

Mechanism (OBSERVED): each provider is `OkHttpClient.Builder.dohX()` = `dns(DnsOverHttps.Builder().client(build()).url(<endpoint>).bootstrapDnsHosts(<ips>).build())`. `client(build())` snapshots the builder (without Cloudflare interceptor), so DoH queries do not pass through the app interceptors. `bootstrapDnsHosts` are the IPs used to reach the DoH server itself (no system DNS involved for that hop). OkHttp's `DnsOverHttps` defaults: GET method with `?dns=<base64url wire-format query>` and `Accept: application/dns-message`, includes IPv6 AAAA lookups, `resolvePrivateAddresses = false`, `resolvePublicAddresses = true`. OkHttp does not fall back to system DNS when DoH fails: the request fails with `UnknownHostException`. The setting applies to all HTTP traffic of the shared client, i.e. sources, extensions, trackers, image loading and downloads, but NOT to the bare clients of AniSkip/AniChart and not to mpv playback.

Preference values and the exact table:

| Pref value | Constant | UI label | DoH URL | Bootstrap IPs | Source line |
|---|---|---|---|---|---|
| -1 | (none) | Disabled (`MR.strings.disabled`) | system DNS | | `NetworkPreferences` |
| 1 | `PREF_DOH_CLOUDFLARE` | Cloudflare | `https://cloudflare-dns.com/dns-query` | 162.159.36.1, 162.159.46.1, 1.1.1.1, 1.0.0.1, 162.159.132.53, 2606:4700:4700::1111, 2606:4700:4700::1001, 2606:4700:4700::0064, 2606:4700:4700::6400 | `DohProviders.kt:26-41` |
| 2 | `PREF_DOH_GOOGLE` | Google | `https://dns.google/dns-query` | 8.8.4.4, 8.8.8.8, 2001:4860:4860::8888, 2001:4860:4860::8844 | `:43-53` |
| 3 | `PREF_DOH_ADGUARD` | AdGuard | `https://dns-unfiltered.adguard.com/dns-query` ("Unfiltered" chosen so no site is blocked; the default AdGuard DNS also works) | 94.140.14.140, 94.140.14.141, 2a10:50c0::1:ff, 2a10:50c0::2:ff | `:55-67` |
| 4 | `PREF_DOH_QUAD9` | Quad9 | `https://dns.quad9.net/dns-query` | 9.9.9.9, 149.112.112.112, 2620:fe::fe, 2620:fe::9 | `:69-79` |
| 5 | `PREF_DOH_ALIDNS` | AliDNS | `https://dns.alidns.com/dns-query` | 223.5.5.5, 223.6.6.6, 2400:3200::1, 2400:3200:baba::1 | `:81-91` |
| 6 | `PREF_DOH_DNSPOD` | DNSPod | `https://doh.pub/dns-query` | 1.12.12.12, 120.53.53.53 | `:93-101` |
| 7 | `PREF_DOH_360` | 360 | `https://doh.360.cn/dns-query` | 101.226.4.6, 218.30.118.6, 123.125.81.6, 140.207.198.6, 180.163.249.75, 101.199.113.208, 36.99.170.86 | `:103-116` |
| 8 | `PREF_DOH_QUAD101` | Quad 101 | `https://dns.twnic.tw/dns-query` | 101.101.101.101, 2001:de4::101, 2001:de4::102 | `:118-127` |
| 9 | `PREF_DOH_MULLVAD` | Mullvad | `https://dns.mullvad.net/dns-query` (the source string has a LEADING SPACE, `" https://dns.mullvad.net/dns-query"`, which `toHttpUrl()` trims) (non-ad-blocking variant) | 194.242.2.2, 2a07:e340::2 | `:129-142` |
| 10 | `PREF_DOH_CONTROLD` | Control D | `https://freedns.controld.com/p0` (unfiltered profile `p0`; comment source `https://controld.com/free-dns/?`) | 76.76.2.0, 76.76.10.0, 2606:1a40::, 2606:1a40:1:: | `:144-159` |
| 11 | `PREF_DOH_NJALLA` | Njalla | `https://dns.njal.la/dns-query` (non-logging, uncensored) | 95.215.19.53, 2001:67c:2354:2::53 | `:161-173` |
| 12 | `PREF_DOH_SHECAN` | Shecan | `https://free.shecan.ir/dns-query` (source `https://shecan.ir/`) | 178.22.122.100, 185.51.200.2 | `:175-186` |
| 13 | `PREF_DOH_LIBREDNS` | LibreDNS | `https://doh.libredns.gr/dns-query` (source `https://libredns.gr/`) | 116.202.176.26, 2a01:4f8:1c0c:8274::1 | `:188-199` |

Host to role index (every chunk 1 DoH host): `cloudflare-dns.com` Cloudflare; `dns.google` Google; `dns-unfiltered.adguard.com` AdGuard unfiltered; `dns.quad9.net` Quad9; `dns.alidns.com` AliDNS; `doh.pub` DNSPod; `doh.360.cn` 360; `dns.twnic.tw` Quad 101 (Taiwan Network Information Center); `dns.mullvad.net` Mullvad; `freedns.controld.com` Control D DoH endpoint; `controld.com` Control D informational page (doc-only, comment at `DohProviders.kt:147`); `dns.njal.la` Njalla; `free.shecan.ir` Shecan; `doh.libredns.gr` LibreDNS.

Proposal (P2, `area:12-network-stack`): keep the same provider list, ids and the "Disabled" default as the stable preference values (so a future Aniyomi-settings import keeps working). Implement as a `reqwest::dns::Resolve` adapter on top of `hickory-resolver` configured per provider with the bootstrap IPs as the name-server socket addresses on port 443 using the HTTPS protocol with `tls_dns_name = <host>` and `http_endpoint = <path>` (check the exact field names against the pinned hickory version), in order IPv4 first. Apply it to the shared `reqwest::Client` through `ClientBuilder::dns_resolver`. Do not apply to libmpv (mpv resolves itself; document this). If DoH resolution fails, surface the error ("Could not resolve host via <provider>") rather than silently using system DNS, matching the Kotlin behaviour. Trim the Mullvad URL. Changing the setting rebuilds the client live (no restart needed; Aniyomi needed a restart because OkHttp's DNS is fixed at build time). Linux note: a user running `systemd-resolved` with DoT does not need this; the setting stays because it is cheap and users on locked-down networks rely on it.

Verification: per provider a `#[ignore]` online test that resolves `example.com` and `one.one.one.one` through the provider (run manually); offline unit test that every table row parses (`Url::parse`, `IpAddr::from_str`); manual cross-check `curl --doh-url <url> https://example.com -I` for each URL, and `curl -s -H 'accept: application/dns-json' 'https://dns.google/resolve?name=example.com'` for the JSON-capable ones; confirm that with Disabled selected no DoH request is made (Wireshark/`ss -tn dst :443` not needed, use a log counter in the resolver).

## 5. Rate limiting: `api.manga.com`

`api.manga.com` is not a real service: it is the example host in the KDoc of the extension-facing helpers `OkHttpClient.Builder.rateLimitHost(...)` in `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/SpecificHostRateLimitInterceptor.kt` (lines 20, 45, 66; sibling example `imagecdn.manga.com`, 10 requests per 2 minutes). No request is ever sent to it.

Semantics to reproduce (OBSERVED), because extensions and trackers depend on them (`core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt`):
* Parameters: `permits` (Int), `period` (Duration, default 1 second), optional `host` (null = every host; set = only requests whose `url.host` equals it, others pass through untouched). The legacy overload takes `(httpUrl, permits, period: Long = 1, unit = SECONDS)`; the new ones take `(httpUrl, permits, period: Duration = 1.seconds)` and `(url: String, permits, period)` (`toHttpUrlOrNull()?.host`, so an invalid URL gives host null, which rate-limits EVERY host).
* Algorithm: a fair (FIFO) binary semaphore serialises callers; a deque holds the monotonic timestamps (`SystemClock.elapsedRealtime()`) of the last `permits` requests. When the deque is full, expired entries (older than `period`) are dropped; if none expired the thread waits for the oldest to expire (wait/notify), checking cancellation (`IOException("Canceled")`) each loop. The request then records its timestamp and proceeds. After the response arrives, if `response.networkResponse == null` (served from the HTTP cache) its timestamp is removed again and waiters are notified, so cached responses do not consume permits.
* Known users in the app: AniList only, 85 per 1 minute (`AnilistApi.kt:42`). Everything else is enforced by extension code.

Proposal (`area:12-network-stack`): a `RateLimiter { permits, period, host: Option<String> }` as a `tower::Layer` or a small wrapper around the `reqwest` client with a `tokio::sync::Mutex<VecDeque<Instant>>` and `tokio::time::sleep_until`, same cache-refund rule when the response came from the local HTTP cache (`cacache`/`http-cache-reqwest`), same pass-through for non-matching hosts. Because Rust extensions are not OkHttp-based, expose the limiter in the source API as per-source configuration. Verification: a paused-time tokio test (`tokio::time::pause`) with permits 5 per second and 12 requests asserting completion times 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 2, 2 seconds, and a test that cached responses are refunded.

## 6. Documentation, community and help links (no API traffic)

All of these are opened in the system browser (`uriHandler.openUri`) or are comments. Rust: use `gtk::UriLauncher` (xdg-desktop-portal OpenURI inside Flatpak) or `gtk::show_uri`. Replace the Aniyomi website links with Kurobayomi's own docs/repo URL where the document exists; where Kurobayomi has no equivalent page, keep the Aniyomi page only if it is still accurate for the feature (marked "reuse") or drop the button.

### 6.1 `aniyomi.org`

Every use, with the meaning and a decision:

| URL | Where (path:line) | Purpose | Decision |
|---|---|---|---|
| `https://aniyomi.org` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:182` (Website icon); `README.md:3`; `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:47` (KDoc example of a source base url) | Website | Replace with the Kurobayomi project page |
| `https://aniyomi.org/docs/guides/getting-started` | `app/src/main/java/eu/kanade/presentation/more/onboarding/GuidesStep.kt:59` (`GETTING_STARTED_URL`), opened from `GuidesStep.kt:38`, `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt:190`, `.../library/manga/MangaLibraryTab.kt:198` | Empty-library "Getting started" button and onboarding guides step | Replace with an in-app help page (P2) or a docs link |
| `https://aniyomi.org/docs/faq/storage` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:89` (`HELP_URL`), opened at `:98` and from `app/src/main/java/eu/kanade/presentation/more/onboarding/StorageStep.kt:103` | Help for the storage location setting | Replace with Kurobayomi's storage doc |
| `https://aniyomi.org/help/guides/tracking/` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt:80` | Help button on the tracking settings | Replace |
| `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare` | `app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt:183` | Cloudflare troubleshooting from the in-app WebView | Replace (the WebView exists only if `area:webview` keeps one) |
| `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads` | `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt:406`, `.../library/manga/MangaLibraryUpdateNotifier.kt:393` | Link from the "large update" warning notification | Replace with an in-app explanation |
| `https://aniyomi.org/help/guides/troubleshooting` | `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:427` (`ERROR_LOG_HELP_URL`) | Written at the top of the manga update error log file (`MR.strings.library_errors_help`) | Replace |
| `https://aniyomi.org/docs/guides/troubleshooting/` | `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt:439` (`ERROR_LOG_HELP_URL`) | Same for the anime update error log | Replace |
| `https://aniyomi.org/help/guides/source-migration/` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:36`, `.../browse/manga/migration/sources/MigrateMangaSourceTab.kt:35` | Help on the source-migration tab | Replace |
| `https://aniyomi.org/docs/guides/troubleshooting/` | `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:4` (`URL_HELP`), used by `app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt:172` and the browse/migration "help" empty-state actions | General "Help" link | Replace |
| `https://aniyomi.org/docs/faq/updates/upcoming` | `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:5` (`URL_HELP_UPCOMING`), used at `app/src/main/java/mihon/feature/upcoming/anime/UpcomingAnimeScreenContent.kt:99` and `.../upcoming/manga/UpcomingMangaScreenContent.kt:99` | Help on the "Upcoming" calendar | Replace |
| `https://aniyomi.org/docs/guides/player-settings/custom-buttons` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreen.kt:44` | FAQ for the player custom-buttons feature (Lua) | Replace if custom buttons are kept (`area:player`) |
| `https://aniyomi.org/docs/contribute#translation` | `AboutScreen.kt:152`; `CONTRIBUTING.md:33`; `i18n/README.md:5` | "Help translate" (Weblate) | Replace with Kurobayomi's own contribution doc |
| `https://aniyomi.org/privacy/` | `AboutScreen.kt:168` | Privacy policy | Replace: Kurobayomi has no analytics; state which third-party hosts it contacts (this catalog) |
| `https://aniyomi.org/docs/faq/browse/extensions` and `https://aniyomi.org/` | `.github/ISSUE_TEMPLATE/config.yml:4`, `:10` | GitHub issue-template contact links | Repository meta only, not ported |
| `https://aniyomi.org/docs/faq/general` and `.../docs/guides/troubleshooting/` | `.github/ISSUE_TEMPLATE/report_issue.yml:98`; `README.md:48` (FAQ, changelog `https://aniyomi.org/changelogs/`) | Issue template checklist and README | Repository meta only |
| `https://aniyomi.org/help/guides/local-anime/` | `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:366` (`HELP_URL`) | Help for the local anime source | Replace |
| `https://aniyomi.org/help/guides/local-manga/` | `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:379` (`HELP_URL`) | Help for the local manga source | Replace |

Test advice: a unit test that every help-URL constant is an `https` URL and appears in one `HELP_LINKS` table (so a link audit is a one-file diff), and a manual click-through of each button once.

### 6.2 Other doc/community/meta hosts

| Host | Where | Role | Rust |
|---|---|---|---|
| `anansi-project.github.io` | `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt:64` (`// https://anansi-project.github.io/docs/comicinfo/schemas/v2.0`) | Specification of the ComicInfo.xml v2.0 schema that the `ComicInfo` data class (`@XmlSerialName("ComicInfo", "", "")`) mirrors; read by the local source and written by the downloader | Not a runtime dependency. The schema is the contract for `area:local-source` and `area:downloads`; use it to write the `quick-xml`/`serde` struct and a golden test file |
| `bugs.chromium.org` | `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt:36` (`id=1279562`) | Comment on why WebView user-agent initialisation is skipped on MIUI and Android 12 Samsung devices (crash) | Dropped: Android WebView only; the Linux Cloudflare solution is decided in `area:12-network-stack` |
| `coil-kt.github.io` | `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:75` | Comment listing the animated formats Coil supports (GIF always; animated WebP on Android 9+, animated HEIF on Android 11+) used by `isAnimatedAndSupported` | Replaced: the reader decides animated support with `gdk-pixbuf` animations or `image` crate; no OS-version gating |
| `contrib.rocks` | `README.md:60` | Contributors image | Dropped |
| `cs.android.com` | `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:117` | Comment about wrapping a Context for the reader night mode | Dropped: GTK theming handles dark/light |
| `developer.android.com` | `CONTRIBUTING.md:19`, `:24`; `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:47` (Android 13 clipboard confirmation: the toast "copied to clipboard" is shown only when `SDK_INT <= S_V2`); `macrobenchmark/README.md:5`; boilerplate `proguard-rules.pro` comments in `core-metadata`, `data`, `domain`, `presentation-core`, `presentation-widget`, `source-local` | Android documentation | Dropped. Clipboard note: on Linux always show a toast after `gdk::Clipboard::set_text` (message key `copied_to_clipboard`, text truncated to 50 chars with centre ellipsis) |
| `discord.gg` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:187` (`https://discord.gg/F32UjdJZrR`); `README.md:12`, `:48`; `CONTRIBUTING.md:29`; `.github/ISSUE_TEMPLATE/config.yml:7`; `CODE_OF_CONDUCT.md:63` (`https://discord.gg/tachiyomi`) | Community server invite (Aniyomi, and the legacy Tachiyomi server) | Replace the About row with Kurobayomi's own community link, or omit |
| `dontkillmyapp.com` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:182` (title `"Don't kill my app!"`, subtitle string `about_dont_kill_my_app`) | Help about OEM battery killers for background work | Dropped: Linux has no such killer; background updates use systemd user timers or an in-process scheduler (`area:library-updates`); what the user loses: nothing |
| `gist.github.com` | `presentation-core/src/main/java/tachiyomi/presentation/core/util/Scrollbar.kt:28` (`.../33a360fcab2105a31e5355af98216f5a`) | Attribution comment for a Compose scrollbar | Dropped: GTK scrolled windows have native scrollbars |
| `github-readme-stats.vercel.app` | `README.md:52`, `:53` | Pinned-repository cards for `aniyomi-website` and `aniyomi-mpv-lib` (`bg_color=161B22&text_color=c9d1d9&title_color=818cf8&icon_color=818cf8&border_radius=8&hide_border=true`) | Dropped (decoration) |
| `controld.com` | `DohProviders.kt:147` | Source comment for the Control D provider | See section 4 |

`aniyomi-mpv-lib` (the pinned card) is the fork of mpv used by the Android player; Linux uses the system `libmpv` (`area:player`).

## 7. Summary of what each tracker/service needs from the Rust network layer

* A shared `reqwest::Client` with: cookie store, connect 30 s, read 30 s, total 2 min, optional DoH resolver, default Firefox User-Agent applied when the request has none, 5 MiB HTTP cache for GET with a 10-minute default max-age on helper-built requests (`area:12-network-stack`).
* `HttpError { status }` with display `"HTTP error <code>"` for every non-2xx response.
* A per-client rate limiter (AniList: 85 per minute).
* A token store (Secret Service) and a loopback OAuth listener (section 0.3).
* A JSON:API content type for Kitsu and MangaUpdates (`application/vnd.api+json`), GraphQL POST helper for AniList, form-encoded bodies for MAL and token endpoints, PATCH/PUT/DELETE support.
* Typed `serde` DTOs; the Kotlin substring parsing in AniSkip and AniChart must NOT be ported (section 3.1 and 3.2 list the defects).
