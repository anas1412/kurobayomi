# 16-source-api-anime: behavior spec

All paths are repo-relative to `/home/blackbox/aniyomi-src`. `OBSERVED` = what the Kotlin does. `Proposal:` = what Kurobayomi (Rust) should do. Message keys are `AYMR.strings.*` (Aniyomi strings, file `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`) or `MR.strings.*` (`i18n/src/commonMain/moko-resources/base/strings.xml`).

---

## 1. Source identity

### 1.1 Id generation (OBSERVED)
Source: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:72` and `:101-105`; identical in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:57` and `:87-91`.

Algorithm:
1. `key = "${name.lowercase()}/$lang/$versionId"` (Kotlin `lowercase()` = locale-independent, root locale).
2. `bytes = MD5(key as UTF-8)` (16 bytes).
3. `id = OR over i in 0..7 of ((bytes[i] & 0xff) << (8 * (7 - i)))`, i.e. first 8 bytes big-endian.
4. `id = id & Long.MAX_VALUE` (sign bit forced to 0, so id is never negative).

Operator precedence note: `bytes[it].toLong() and 0xff shl 8 * (7 - it)` parses as `((bytes[it].toLong() and 0xff) shl (8 * (7 - it)))` because infix functions share precedence and multiplication binds tighter.

`versionId` is `Int` with default `1` (`AnimeHttpSource.kt:60`). `id` is `by lazy` (computed once).

Test vectors (computed for this plan with Python `hashlib.md5`):
- name `Example`, lang `en`, versionId `1` → key `example/en/1` → id `4851548778462321757`.
- name `Aniyomi Test`, lang `ja`, versionId `2` → key `aniyomi test/ja/2` → id `7915262807009061958`.

`toString()` = `"$name (${lang.uppercase()})"` (`AnimeHttpSource.kt:117`), e.g. `Example (EN)`.

Proposal: implement `fn generate_id(name: &str, lang: &str, version_id: i32) -> i64` with `md5` crate, `name.to_lowercase()` (Unicode, matches Java `toLowerCase(Locale.ROOT)` for practical names; see edge-case doc for the Turkish/final-sigma caveat), and the bit operations above. Store the id as `i64` in SQLite (matches Kotlin `Long`).

### 1.2 Headers and client (OBSERVED)
- `headers` = `headersBuilder().build()`, `by lazy` (`AnimeHttpSource.kt:77`).
- `headersBuilder()` adds exactly one header: `User-Agent` = `network.defaultUserAgentProvider()` (`AnimeHttpSource.kt:110-112`).
- `defaultUserAgentProvider()` = `preferences.defaultUserAgent().get().trim()` (`core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:73`).
- Default of that preference: `"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"` (`core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:19-22`).
- Because `headers` is lazy, a change to the user agent preference after the first access is NOT reflected in that source's `headers` until restart. OBSERVED consequence; Proposal: keep the lazy semantics but rebuild on preference change (cheap) — or document as known behavior.
- `client` = `network.client` (open, overridable, `AnimeHttpSource.kt:82`).

### 1.3 Base URL
- `baseUrl` abstract, "without trailing slash" (`AnimeHttpSource.kt:41`).
- `getHomeUrl()` defaults to `baseUrl` (`AnimeHttpSource.kt:54`). Consumers: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt:73`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/season/MigrateSeasonSelectScreen.kt:71`.

---

## 2. Request construction defaults (OBSERVED)

### 2.1 `GET` helper
`core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:12-31`:
- `DEFAULT_CACHE_CONTROL = CacheControl.Builder().maxAge(10, MINUTES).build()` → request header `Cache-Control: max-age=600`.
- `DEFAULT_HEADERS = Headers.Builder().build()` (empty).
- `GET(url: String | HttpUrl, headers, cache)` builds `Request` with `url`, `headers`, `cacheControl`.

Every default request built by the anime base class (`GET(baseUrl + x, headers)`) therefore carries `Cache-Control: max-age=600` unless the caller passes another `CacheControl`.

### 2.2 Default request URLs (OBSERVED)
| Hook | Default request URL | Where |
|---|---|---|
| `animeDetailsRequest(anime)` | `baseUrl + anime.url` | `AnimeHttpSource.kt:274-276` |
| `episodeListRequest(anime)` | `baseUrl + anime.url` | `:321-323` |
| `seasonListRequest(anime)` | `baseUrl + anime.url` | `:380-382` |
| `hosterListRequest(episode)` | `baseUrl + episode.url` | `:426-428` |
| `videoListRequest(hoster)` | `hoster.hosterUrl` (absolute, no baseUrl) | `:471-473` |
| `videoListRequest(episode)` (legacy, deprecated) | `baseUrl + episode.url` | `:571-573` |
| `videoUrlRequest(video)` (legacy) | `video.url` (the legacy page URL, empty for new-style Video) | `:643-645` |

Note: `GET("")` throws `IllegalArgumentException` in OkHttp (`toHttpUrl`), so a legacy `videoUrlRequest` on a new-style Video (empty `url`) fails. The loader only calls it when `videoUrl == "null"` (see section 5.3).

### 2.3 Request execution and errors (OBSERVED)
- `Call.awaitSuccess()` (`core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:95-104`): awaits the call; if `!response.isSuccessful` it closes the response and throws `HttpException(response.code)`. Stack trace is copied from the caller.
- `Call.await()` (`:75-93`): cancellable via `suspendCancellableCoroutine`; on cancellation calls `Call.cancel()`; on `IOException` rethrows `IOException(e.message, e)` with the caller's stack.
- `Call.asObservableSuccess()` (deprecated, `:63-70`): same HTTP-status rule, RxJava based.
- `AnimeHttpSource.getImageTile` does NOT use `awaitSuccess`; it calls `client.newCall(GET(url, headers)).execute()` synchronously (`AnimeHttpSource.kt:533-537`). Non-2xx bodies are passed to the decoder. See section 9.

Proposal: one `fetch(req) -> Result<Response, SourceError>` helper that mirrors `awaitSuccess` (non-2xx → `SourceError::Http { status }`), cancels on drop (tokio `JoinHandle` abort or `reqwest` future drop), and maps IO errors to `SourceError::Network`.

---

## 3. Interface contract and default composition

### 3.1 `AnimeSource` (OBSERVED)
`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt`:
- Abstract: `id`, `name`, `supportsLatest`, `getPopularAnime(page)`, `getLatestUpdates(page)`, `getSearchAnime(page, query, filters)`, `getAnimeEpisodeUpdate(anime, episodes, fetchDetails, fetchEpisodes)`, `getAnimeSeasonUpdate(anime, seasons, fetchDetails, fetchSeasons)`, `getRelatedAnimeList(anime)`.
- Defaults: `lang = ""` (`:30`), `getFilterList() = AnimeFilterList()` (`:40`), `supportsRelatedAnime = false` (`:122`), `getHosterList(episode) = throw IllegalStateException("Not used")` (`:139`), `getVideoList(hoster) = throw IllegalStateException("Not used")` (`:148`).
- Deprecated and throwing `UnsupportedOperationException`: `getSeasonList`, `getVideoList(episode)`, `fetchAnimeDetails`, `getAnimeDetails`, `getEpisodeList`, `fetchEpisodeList`, `fetchVideoList`.
- Every `getAnimeEpisodeUpdate`/`getAnimeSeasonUpdate` doc says: "If a value is not requested, the existing provided value can be returned as-is. The host app may apply any returned updates regardless of the flags".

### 3.2 `AnimeCatalogueSource` defaults (OBSERVED)
`AnimeCatalogueSource.kt`:
- `getPopularAnime`, `getLatestUpdates`, `getSearchAnime` delegate to the deprecated `fetch*` Rx methods and `awaitSingle()` (`:25-35`). `AnimeHttpSource` implements the `fetch*` methods, so the chain is `getX → fetchX → client.newCall(xRequest).asObservableSuccess().map(xParse)`.
- `getAnimeEpisodeUpdate` (`:38-47`):
  ```
  supervisorScope {
    asyncAnime   = if (fetchDetails)  async { getAnimeDetails(anime) } else null
    asyncEpisodes = if (fetchEpisodes) async { getEpisodeList(anime) } else null
    SAnimeEpisodeUpdate(asyncAnime?.await() ?: anime, asyncEpisodes?.await() ?: episodes)
  }
  ```
  - When a flag is false, the original `anime` / `episodes` objects are returned by identity.
  - Both calls run concurrently on the caller's dispatcher. A failure in one child does not cancel the sibling (`supervisorScope`), but the failed `await()` rethrows, so the whole call fails.
- `getAnimeSeasonUpdate` (`:50-59`): identical shape with `getAnimeDetails` and `getSeasonList`.
- `getRelatedAnimeList` throws `Exception("Stub!")` (`:61-63`). It is only called if `supportsRelatedAnime` is true.

### 3.3 Self-recursion defects (OBSERVED, defect)
`AnimeCatalogueSource.kt:65-67`:
```
override suspend fun getHosterList(episode: SEpisode): List<Hoster> = getHosterList(episode)
override suspend fun getVideoList(hoster: Hoster): List<Video> = getVideoList(hoster)
```
Both overrides call themselves. Any concrete subclass that does not override the method recurses until `StackOverflowError`. `AnimeHttpSource` does override both (`:406`, `:451`), so the defect only bites non-HTTP implementers.

Proposal: in Rust, do not provide a default body for `hosters`/`videos_for_hoster` at the trait level; make them required methods, or default to `Err(SourceError::NotSupported)`. Never re-enter the same method.

### 3.4 Capability routing: hosters versus videos (OBSERVED, decision point)
`app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt:60-78` (`checkHasHosters`):
- Walk the class chain starting at `source.javaClass`, moving to `superclass` each step.
- At each class: if it is `ParsedAnimeHttpSource`, `AnimeHttpSource` or `AnimeSource`, return `false` immediately (the check for that class is NOT performed).
- Otherwise, if `declaredMethods` of that class contains a method named `getHosterList`, `hosterListRequest` or `hosterListParse`, return `true`.
- If no superclass remains, return `false`.

Consequence (OBSERVED): a `ParsedAnimeHttpSource` subclass that implements only the selector hooks (`hosterListSelector`, `hosterFromElement`) is routed to the legacy path (`getVideoList(episode)`), because those names are not in the three-name list and the walk stops at `ParsedAnimeHttpSource`. Its hoster selectors are then never used. Recorded as edge case E-01 in the edge-case doc.

Proposal: in Rust, the trait exposes an explicit `fn provides_hosters(&self) -> bool` that each implementation sets. Parsed sources set it from whether the hoster selector is configured. Do not reproduce reflection.

---

## 4. Listing, search and details

### 4.1 Listing result shape
`AnimesPage(animes: List<SAnime>, hasNextPage: Boolean)` (`model/AnimesPage.kt:3`). No ordering is imposed by the base classes; order is the order the source returns.

### 4.2 Parsed listing rule (OBSERVED)
`ParsedAnimeHttpSource.kt:31-43` (popular), `:72-84` (search), `:113-125` (latest):
1. `document = response.asJsoup()`.
2. `animes = document.select(xSelector()).map { xFromElement(it) }`. No dedupe, no filtering.
3. `hasNextPage = xNextPageSelector()?.let { document.select(it).first() } != null`. A null selector gives `false`. A selector that matches at least one element gives `true`.

### 4.3 Details, episodes, seasons, hosters, videos (Parsed, OBSERVED)
- `animeDetailsParse(response)` → `animeDetailsParse(response.asJsoup())` (`:154-156`).
- `episodeListParse(response)` → `document.select(episodeListSelector()).map(episodeFromElement)` (`:173-176`). Order is document order.
- `seasonListParse(response)` → `document.select(seasonListSelector()).map(seasonFromElement)` (`:199-202`).
- `hosterListParse(response)` → `document.select(hosterListSelector()).map(::hosterFromElement)` (`:229-232`).
- `videoListParse(response)` (legacy, no hoster argument) → `document.select(videoListSelector()).map(videoFromElement)` (`:257-260`).
- `videoUrlParse(response)` → `videoUrlParse(document)` (`:282-284`), a single absolute URL string.

`asJsoup(html = null)` (`util/JsoupExtensions.kt:24-26`): `Jsoup.parse(html ?: body.string(), request.url.toString())`. The base URI is the request URL of the `Response` (the final URL after redirects, as OkHttp exposes it). `body.string()` consumes the body.

### 4.4 Jsoup helper semantics (OBSERVED)
`util/JsoupExtensions.kt`:
- `selectText(css, defaultValue = null)`: `select(css).first()?.text() ?: defaultValue`. Returns `defaultValue` only when no element matches; a matched empty element returns `""`.
- `selectInt(css, defaultValue = 0)`: `select(css).first()?.text()?.toInt() ?: defaultValue`. No match → default. Matched element with non-numeric text (including `""`) → `NumberFormatException` (not caught).
- `attrOrText(css)`: `if (css != "text") attr(css) else text()`. Missing attribute yields `""` (Jsoup behavior), not null.

### 4.5 Episode list ordering (OBSERVED, consumer)
The base class returns the list in source order. The app's sync (`app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt`, owned by area:07) decides ordering and `sourceOrder`. Proposal: store the index from the returned list as `source_order` and never re-sort inside the source layer.

### 4.6 Combined update flow (OBSERVED, consumer)
`app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt`:
- Episodes: local episodes loaded `sortedBy { it.sourceOrder }`, passed as `episodes` to `getAnimeEpisodeUpdate(anime.toSAnime(), episodes, fetchDetails, fetchEpisodes)` inside `withIOContext`. The returned `anime` is applied via `awaitUpdateFromSource(anime, update.anime, manualFetch)`, then episodes are reconciled through `SyncEpisodesWithSource.await(rawSourceEpisodes = update.episodes, anime, source, manualFetch, fetchWindow)`.
- Seasons: `seasons` loaded `sortedBy { it.anime.seasonSourceOrder }`, same pattern through `getAnimeSeasonUpdate` and `SyncSeasonsWithSource`.
- `awaitUpdateFromSource` (`UpdateAnimeFromRemote.kt:142-196`): title taken from the remote only when the local entry is not a favorite and the remote title is non-empty; cover and background timestamps are recomputed from `thumbnail_url` / `background_url`. Empty remote image URL never clears a stored image.
- Both paths catch `Exception`, log at ERROR and return `Result.failure(e)`.

### 4.7 `FetchType` contract (OBSERVED)
`model/FetchType.kt:3-19`:
- `Seasons`: the entry only calls `getSeasonList` (`getAnimeSeasonUpdate`).
- `Episodes`: the entry only calls `getEpisodeList` (`getAnimeEpisodeUpdate`).
- Stated lifecycle: "will not update after it's been initialized to either Seasons or Episodes".
- Default on `SAnimeImpl`: `FetchType.Episodes` (`SAnimeImpl.kt:32`).
- Persisted by the app through `FetchTypeColumnAdapter` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:216`, owned by area:04).
- Consumer UI: `app/src/main/java/eu/kanade/presentation/entries/anime/SeasonSettingsDialog.kt`.

Proposal: `FetchType` is a Rust enum stored as its variant name; the update code matches on it. Once set on an entry, the value is not changed by remote updates.

### 4.8 `UpdateStrategy` contract (OBSERVED)
`model/AnimeUpdateStrategy.kt:9-23`: `ALWAYS_UPDATE` (default, `SAnimeImpl.kt:30`) is included in library updates unless other restrictions apply; `ONLY_FETCH_ONCE` is skipped during library updates ("previously known to be finished and have only a single chapter"). Effect is applied by the library update job (area:26), not by sources.

### 4.9 `SAnime` status integers (OBSERVED)
`model/SAnime.kt:72-79`: `UNKNOWN=0, ONGOING=1, COMPLETED=2, LICENSED=3, PUBLISHING_FINISHED=4, CANCELLED=5, ON_HIATUS=6, UPCOMING=7`. Default `status = 0` (`SAnimeImpl.kt:22`). Note: manga `SManga` has the same constants except `UPCOMING` (`source/model/SManga.kt:48-54`).

Proposal: Rust enum `AnimeStatus` with explicit discriminants 0 through 7, plus a lossless `Unknown(i32)` for values written by future versions. Unknown integers display as Unknown.

### 4.10 `getGenres()` (OBSERVED)
`SAnime.kt:49-52`: if `genre` is null or blank → `null`. Else `genre.split(", ").map(trim).filterNot(blank).distinct()`. The separator is the literal two characters comma-space. `"a,b"` is one genre. `distinct` keeps first occurrence order.

### 4.11 `copy()` (OBSERVED)
`SAnime.kt:54-69` copies these 14 fields into a new `SAnimeImpl`: `url, title, artist, author, description, genre, status, thumbnail_url, background_url, update_strategy, fetch_type, season_number, initialized, memo`. Nothing is shared by reference except the `memo` JsonObject (immutable in practice).

### 4.12 `SEpisode.copyFrom` (OBSERVED)
`SEpisode.kt:39-49` copies `name, url, date_upload, episode_number, fillermark, scanlator, summary, preview_url, memo`. It is used to refresh an existing object from a remote one.

---

## 5. Hoster and video discovery (app side, consumes this API)

Entry point: `EpisodeLoader.getHosters(episode, anime, source)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt:33-41`):
1. `isDownloaded = downloadManager.isEpisodeDownloaded(episode.name, episode.scanlator, anime.title, anime.source, skipCache = true)` (`:49-57`).
2. `when`:
   - downloaded → `getHostersOnDownloaded` (builds one Video from downloads, wrapped with `toHosterList()`; any `Throwable` → empty list, `:105-117`).
   - `source is AnimeHttpSource` → `getHostersOnHttp` (section 5.1).
   - `source is LocalAnimeSource` → `getHostersOnLocal` (section 5.4).
   - else → `error("source not supported")`.

### 5.1 HTTP hosters (OBSERVED, `EpisodeLoader.kt:86-96`)
- If `checkHasHosters(source)` (section 3.4): `source.getHosterList(episode.toSEpisode())` then `.let { source.run { it.sortHosters() } }`.
- Else (legacy): `source.getVideoList(episode.toSEpisode())`, then `.sortVideos()`, then `.toHosterList()` (one synthetic hoster, section 5.5).

### 5.2 Hoster loading (OBSERVED, `EpisodeLoader.kt:189-204`, `loadHosterVideos(source, hoster, force = false)`)
1. If `!force && hoster.lazy` → `HosterState.Idle(hoster.hosterName)`. The videos are not fetched yet.
2. Else `getVideos(source, hoster)`:
   - `hoster.videoList != null && source is AnimeHttpSource` → `hoster.videoList.parseVideoUrls(source)` then sort (step 4).
   - `hoster.videoList != null` (non-HTTP source) → `hoster.videoList` as-is (no sort, see step 4).
   - `source is AnimeHttpSource` → `source.getVideoList(hoster)` then `parseVideoUrls(source)` then sort.
   - else → `error("source not supported")`.
3. `HosterState.Ready(hoster.hosterName, videos, List(videos.size) { Video.State.QUEUE })`.
4. For `AnimeHttpSource`, `source.run { videos.sortVideos() }` is applied last (`:161-165`).
5. Any non-cancellation `Exception` → `HosterState.Error(hoster.hosterName)`. `CancellationException` is rethrown (`:197-200`).

### 5.3 Legacy URL fixup, `parseVideoUrls` (OBSERVED, `EpisodeLoader.kt:180-187`)
For each video with `videoUrl == "null"` (the literal string, produced by the deprecated constructor when the URL is null), call `source.getVideoUrl(video)` and copy the video with the new `videoUrl`. Other videos pass through unchanged.

### 5.4 Local hosters (OBSERVED, `EpisodeLoader.kt:124-143`)
`episode.url.split('/', limit = 2)` → `(animeDirName, episodeName)`. Find the file under the local base directory, take `videoFile!!.uri`, build `Video(videoUri.toString(), "Local source: ${episode.url}")`, wrap with `toHosterList()`. Any exception → empty list.

### 5.5 Synthetic hoster (OBSERVED, `Hoster.kt:59-68`)
`List<Video>.toHosterList()` returns `listOf(Hoster(hosterUrl = "", hosterName = "no_hoster_list", videoList = this, memo = EMPTY))`. `NO_HOSTER_LIST = "no_hoster_list"` (`Hoster.kt:57`). It is never shown as a real server name; the UI treats it as the single unnamed server. Proposal: keep the constant and map it to "no hoster" in UI.

---

## 6. Video selection and resolution (app side, `HosterLoader.kt`)

### 6.1 `selectBestVideo(hosterState)` (OBSERVED, `HosterLoader.kt:27-67`)
Input: list of `HosterState`. Output: `(hosterIndex, videoIndex)` or `(-1, -1)`.

Only states of type `Ready` are considered; hosters keep their list index.

Tier 1, preferred:
- A pair `(video, state)` is preferred when `video.preferred && (state == READY || state == QUEUE)`.
- Choose the first hoster (in list order) containing a preferred pair; within it the first preferred video index.

Tier 2, first valid URL:
- A pair is valid when `video.videoUrl.isNotEmpty() && (state == READY || state == QUEUE)`.
- Choose the first hoster with a valid pair; first valid index within it.

Otherwise `(-1, -1)`.

Note: `ERROR` and `LOAD_VIDEO` states are never selected here. A video that was preferred but is in `ERROR` falls through to tier 2.

### 6.2 `getBestVideo(source, hosterList)` (OBSERVED, `HosterLoader.kt:78-150`)
Runs in `withContext(Dispatchers.IO)`.

Phase A, parallel preferred probing:
1. `hosterStates = MutableList(n) { HosterState.Idle("") }`.
2. For every hoster, `async`:
   a. `hosterState = EpisodeLoader.loadHosterVideos(source, hoster)` and store it at its index.
   b. If Ready and there is an index `prefIndex` of a video with `preferred && !initialized`:
      - set that video's state to `LOAD_VIDEO`;
      - `resolved = getResolvedVideo(source, video)`;
      - if `resolved?.videoUrl` is non-empty → `cancelChildren()` and throw `EarlyReturnException(resolved)`;
      - else set that video's state to `ERROR`.
3. `awaitAll()`.
4. The `EarlyReturnException` is caught at the outermost `try` (`:147-149`) and returned as the best video. Cancelling siblings means other hosters' probes are abandoned.

Phase B, sequential fallback:
1. `(hosterIdx, videoIdx) = selectBestVideo(hosterStates)`.
2. While `hosterIdx != -1`:
   - set video state `LOAD_VIDEO`;
   - `resolved = getResolvedVideo(source, video)`;
   - if non-empty URL → `cancelChildren()` and return it;
   - else set state `ERROR` and reselect.
3. Return `null` when nothing is left.

Ordering subtleties (OBSERVED):
- Phase A can return a video from a later hoster before an earlier one has been probed, because the first successful resolution wins. This is intended as "first ready wins".
- Phase A only probes one video per hoster (the first preferred one that is not `initialized`). Other videos are reached only in phase B.
- Each coroutine on `Dispatchers.IO` assigns its own index of the plain `MutableList` `hosterStates`. Not synchronized. Proposal: use one owner task per hoster and channel results back to a single collector.

### 6.3 `getResolvedVideo(source, video)` (OBSERVED, `HosterLoader.kt:152-168`)
- If `source is AnimeHttpSource && !video.initialized`: `source.resolveVideo(video)`; on exception: rethrow `CancellationException`, otherwise `null`.
- Else the input video is used unchanged (non-HTTP sources and already-initialized videos are NOT resolved again).
- Result: `resolvedVideo?.copy(initialized = true)`. A null result stays null.
- A resolved video with empty `videoUrl` is treated as a failure by both phases.

### 6.4 `resolveVideo` in the base class (OBSERVED, `AnimeHttpSource.kt:500-502`)
`open suspend fun resolveVideo(video: Video): Video? = video`. Sources override it to turn a page URL into a direct stream URL (lazy resolution).

Proposal: keep `resolve_video(&self, Video) -> Result<Option<Video>, SourceError>`. Errors other than cancellation count as `None`, same as the Kotlin.

---

## 7. Sorting contract (OBSERVED)

- `sortHosters()` default returns the list unchanged (`AnimeHttpSource.kt:591-593`).
- `sortVideos()` default calls the deprecated protected `sort()` extension, which returns `this` (`AnimeHttpSource.kt:600-611`). The member extension `List<Video>.sort()` in the class shadows the stdlib `sort()`, so the default order is the source's order. Result: no sorting by default.
- The first hoster and first video in the sorted order win ties in `selectBestVideo` (section 6.1), so sources control priority by the order they return. Sources override `sortVideos` to put preferred quality first.

Proposal: `fn sort_hosters(&self, hosters: Vec<Hoster>) -> Vec<Hoster> { hosters }` and `fn sort_videos(&self, videos: Vec<Video>) -> Vec<Video> { videos }` as default trait methods. Do not add a global quality sort in the Rust app; this is a source decision.

---

## 8. Video model behaviors (OBSERVED, `model/Video.kt`)

### 8.1 Defaults
Primary constructor (`:30-45`): `videoUrl = ""`, `videoTitle = ""`, `resolution: Int? = null`, `bitrate: Int? = null`, `headers: Headers? = null`, `preferred = false`, `subtitleTracks = []`, `audioTracks = []`, `timestamps = []`, `mpvArgs = []`, `ffmpegStreamArgs = []`, `ffmpegVideoArgs = []`, `internalData = ""`, `initialized = false`, `memo = JsonObject.EMPTY`.

Runtime-only: `status: State = State.QUEUE` (`@Transient @Volatile`, `:144-149`). Not part of equality (`data class` equality only covers primary constructor properties).

### 8.2 `State` (OBSERVED, `:180-185`)
`QUEUE → LOAD_VIDEO → READY` or `ERROR`. Set by the app loader (section 6.2). `READY` is set by the player after the stream is opened (outside this scope).

### 8.3 Legacy constructors (OBSERVED, `:60-85`)
- `Video(url, quality, videoUrl: String?, headers, subtitleTracks, audioTracks)` sets `videoTitle = quality`, `videoUrl = videoUrl ?: "null"`, and stores `url` in a private `videoPageUrl`.
- `Video(url, quality, videoUrl, uri, headers)` delegates to the one above.
- `url` getter returns `videoPageUrl` (empty for new-style videos). `quality` getter returns `videoTitle`.
Proposal: drop all legacy constructors and accessors. Reason: they exist only for APK extensions built against ext-lib 15/16. Consequence for users: none for native sources; old extension APKs are not loadable anyway (see area:19 / area:20). Record in tasks T-16a-015.

### 8.4 `usesHttpServer()` (OBSERVED, `:151-165`)
Returns true when any of these match the regex `http:\/\/localhost:1(?!\d)`: `videoUrl`, any audio track URL, any subtitle track URL.

The negative lookahead `(?!\d)` means `http://localhost:1` followed by a digit (for example port `12345`) does NOT match. A URL that is exactly `http://localhost:1` or `http://localhost:1/...` matches.

### 8.5 `copyHttpServer(port)` (OBSERVED, `:167-178`)
- `newHost = "http://localhost:$port"`.
- `videoUrl = localUrl.replace(videoUrl, newHost)` (Regex.replace with the input string). Subtitle and audio URLs are rewritten the same way.
- Other fields are copied with `this.copy(videoUrl = ..., subtitleTracks = ..., audioTracks = ...)` (the other fields keep their values; see E-07 for the `memo` exception).

Defect to verify (OBSERVED, probable): the deprecated `copy` in `Video.kt:111-142` sets `memo = JsonObject.EMPTY` regardless of the receiver, so if overload resolution picks it, `memo` is dropped by `copyHttpServer`. Recorded as edge E-07. Proposal: the Rust struct copies every field, including `memo`.

### 8.6 Constants
- `MPV_ARGS_TAG = "ANIYOMI_MPV_ARGS"` (`:188`). Used only for downloaded files (section 10.3).

---

## 9. Thumbnails and image tiles

### 9.1 Model (OBSERVED, `model/ThumbnailInfo.kt`)
`ThumbnailInfo(tileInfo: List<TileInfo>, imageTileUrls: List<String>)`. `TileInfo(imageIndex: Int, timeMs: Long, x: Int, y: Int, width: Int, height: Int)`. `imageIndex` indexes `imageTileUrls`. `x, y` are the top-left pixel of the tile within the tile image; `width, height` the tile size in pixels. `timeMs` is the start of the preview.

### 9.2 Loading (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1580-1606`)
- Only for `AnimeHttpSource`. Otherwise nothing.
- `source.getVideoThumbnails(video)`: default returns `null` (`AnimeHttpSource.kt:522-524`).
- When non-null: `tileInfo` sorted by `timeMs` (ascending); `imageTileUrls` stored unchanged.
- Preload (`PlayerViewModel.kt:1592`): `imageTileUrls.take(2)` (first two tile images only), each via `getImageTile(url)`; non-null bitmaps are stored in an in-memory cache keyed by index.
- Any non-cancellation exception → log ERROR "Failed to fetch thumbnails" (`:1602`). The player continues without previews.

### 9.3 `getImageTile(url)` (OBSERVED, `AnimeHttpSource.kt:533-537`)
Default: `client.newCall(GET(url, headers)).execute().body.byteStream().use { BitmapFactory.decodeStream(it) }`. Blocking, no status check, returns null for undecodable data.

Proposal: `async fn image_tile(&self, url: &str) -> Result<Option<Vec<u8>>, SourceError>`. Use `fetch` (section 2.3) so non-2xx is an error, return the raw bytes, decode in the UI with gdk-pixbuf or the `image` crate on a worker task. Keep the "first two tiles" preload rule and the timeMs sort.

---

## 10. Local HTTP server and placeholder URLs

### 10.1 Contract (OBSERVED)
- `open fun createHttpServer(): HttpServer? = null` (`AnimeHttpSource.kt:511-513`).
- `HttpServer : NanoHTTPD(0)` (`model/HttpServer.kt:7`): ephemeral port (0). `url = "http://localhost:$listeningPort"`. `start()` catches any `Exception`, logs DEBUG "Failed to start http server" and leaves `isRunning = false`. `stop()` sets `isRunning = false`. `PLACEHOLDER_URL = "http://localhost:1"` (`:33`).
- A source emits URLs that start with `http://localhost:1` as placeholders; `copyHttpServer(port)` rewrites them (section 8.5).

### 10.2 Player lifecycle (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt`)
- Started with the source id as extra. `server = (sourceManager.get(sourceId) as? AnimeHttpSource)?.createHttpServer()`. Null → `stopSelf()` and `START_NOT_STICKY`.
- A running server is stopped before a new one starts (`:37-43`).
- `start()` failure → `stopSelf()`, `START_NOT_STICKY`. Success → foreground notification on channel `http_server_channel` (`Notifications.kt:81`, importance LOW), text `AYMR.strings.http_server_is_running` ("Http server is running"), action `MR.strings.action_close` ("Close") which sends `ACTION_STOP`, ongoing, auto-cancel off, chronometer on.
- `onDestroy` sets `isRunning = false` and stops the server.
- Failure string: `AYMR.strings.http_server_start_failure` ("Failed to start http server").

### 10.3 Downloader (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt:448-466`)
- If `video.usesHttpServer()` and the external-downloader setting is off: `httpServer = download.source.createHttpServer(); httpServer.start(); download.video = copyHttpServer(httpServer?.listeningPort ?: 0)`.
- Otherwise, if `usesHttpServer()`: `MainActivity.startHttpServerService(context, source.id)` returns `(success, port)`; failure throws `Exception("Failed to start server")`; success copies with `port`.
- Defect to note (OBSERVED): when `createHttpServer()` returns null, `listeningPort ?: 0` yields port 0, and the URLs become `http://localhost:0/...`. Recorded as E-08.

### 10.4 Rust design (Proposal)
- One `LocalStreamServer` per playing or downloading episode, bound to `127.0.0.1:0`, implemented with `axum` (or `tiny_http` for minimal footprint). It forwards requests to the source's upstream URL with the video's headers, supports `Range` requests (required for seeking in mpv and ffmpeg), and shuts down when the player closes.
- Refuse to rewrite when the server failed to bind (`port == 0`). Return `SourceError::LocalServer` instead. This fixes E-08.

---

## 11. Preferences

### 11.1 Keys (OBSERVED, `animesource/utils/Preferences.kt:13-40`, `animesource/ConfigurableAnimeSource.kt:22-29`)
- `preferencesKey(id: Long) = "source_$id"`. Decimal id, no padding. Example: id `4851548778462321757` → `source_4851548778462321757`.
- `sourcePreferences(key)` = `Injekt.get<Application>().getSharedPreferences(key, Context.MODE_PRIVATE)`. Each key is a separate XML file in the app's private storage.
- `ConfigurableAnimeSource.getSourcePreferences()` uses the same key (`:16-17`).

Proposal: one namespace per source id in the settings store owned by area:02 (`scope = "source", key = "source_<id>"`), values typed JSON. Keep the decimal id string so keys stay comparable to Aniyomi backups.

### 11.2 Setup screen (OBSERVED)
`ConfigurableAnimeSource.setupPreferenceScreen(screen: PreferenceScreen)` (`:19`). `PreferenceScreen` is `androidx.preference.PreferenceScreen` on Android (`source-api/src/androidMain/.../PreferenceScreen.kt`). Sources populate it with androidx preference widgets; the app renders them.

Proposal: `fn setup_preferences(&self) -> Vec<SourcePref>` with variants `Switch`, `List`, `MultiSelect`, `EditText`, `Category`, each carrying `key`, `title`, `summary`, `default`. The GTK UI renders them (area:36/39 owns the screen). Drop the androidx dependency.

---

## 12. Deep-link resolution (`ResolvableAnimeSource`, OBSERVED)
`online/ResolvableAnimeSource.kt`:
- `getUriType(uri) -> UriType` where `UriType` is `Anime`, `Episode` or `Unknown` (sealed interface, data objects).
- `getAnime(uri): SAnime?` called when `Anime`; `getEpisode(uri): SEpisode?` called when `Episode`.
- Null results are "not found". Unknown types are not resolved.
- Consumers: `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreenModel.kt`.

Proposal: `trait ResolvableAnimeSource { fn uri_type(&self, uri: &str) -> UriType; async fn anime_for_uri(&self, uri: &str) -> Result<Option<SAnime>, SourceError>; async fn episode_for_uri(&self, uri: &str) -> Result<Option<SEpisode>, SourceError>; }` and call it from the CLI or desktop open-URI handler.

---

## 13. Unmetered sources (OBSERVED)

`UnmeteredSource` is a marker. Observed effects:
- Download queue warning (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt:313-320`): `queuedDownloads` counts only downloads whose source is not unmetered. `maxDownloadsFromSource` groups by source and excludes unmetered sources. Warning when `queuedDownloads > DOWNLOADS_QUEUED_WARNING_THRESHOLD (20)` or `maxDownloadsFromSource > EPISODES_PER_SOURCE_QUEUE_WARNING_THRESHOLD (10)`. Warning shows `AYMR.strings.download_queue_size_warning` ("Warning: large bulk downloads may lead to sources becoming slower and/or blocking Aniyomi. Tap to learn more.") for `WARNING_NOTIF_TIMEOUT_MS = 30_000` ms with a help URL.
- Library update notifier (`app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt:139-145`): groups to-update anime by source, excludes unmetered sources, warns when the largest group exceeds `ANIME_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 60` (`:413`).
- Season fetch (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:690-701`): unmetered sources fetch all seasons in parallel (`async(Dispatchers.IO)` then `awaitAll()`); others fetch sequentially with `ensureActive()` before each.

Proposal: `trait UnmeteredSource {}` marker; concurrency limits (area:12 rate limiter) skip sources that implement it. Thresholds stay as above.

---

## 14. Filters (OBSERVED, `model/AnimeFilter.kt`, `model/AnimeFilterList.kt`)

### 14.1 Shapes
| Class | State type | Default state | Notes |
|---|---|---|---|
| `Header(name)` | `Any` (`0`) | `0` | Display only |
| `Separator(name = "")` | `Any` (`0`) | `0` | Display only |
| `Select<V>(name, values: Array<V>, state: Int = 0)` | `Int` index into `values` | `0` | No bounds check at construction |
| `Text(name, state: String = "")` | `String` | `""` | |
| `CheckBox(name, state: Boolean = false)` | `Boolean` | `false` | |
| `TriState(name, state: Int = STATE_IGNORE)` | `Int` | `0` | `STATE_IGNORE=0`, `STATE_INCLUDE=1`, `STATE_EXCLUDE=2`; helpers `isIgnored/isIncluded/isExcluded` |
| `Group<V>(name, state: List<V>)` | `List<V>` | caller-provided | Container of child filters |
| `Sort(name, values: Array<String>, state: Selection? = null)` | `Selection?` | `null` | `data class Selection(index: Int, ascending: Boolean)` |

### 14.2 Equality (OBSERVED)
- `AnimeFilter.equals`: same object, or `other is AnimeFilter<*>` with equal `name` and `state`. The subclass is not compared, so `Header("A")` equals `Select("A", values)` when both states are `0`. Recorded as E-12.
- `hashCode = 31 * name.hashCode() + state.hashCode()` (null-safe).
- `AnimeFilterList.equals` ALWAYS returns `false` (`AnimeFilterList.kt:10-12`), even against itself, so Compose/state comparisons treat any two lists as different. `hashCode = list.hashCode()`.

Proposal: filters are Rust enums with explicit variants. Equality is derived (class and state). The filter list is a plain `Vec` wrapped in `Arc`; comparisons use value equality, not the always-false quirk.

### 14.3 Filter list construction (OBSERVED)
`constructor(vararg fs)` yields `fs.asList()` when non-empty, else `emptyList()`. Order is preserved; filters are applied by the source in order.

### 14.4 Filter state persistence
Not in this scope. Filter values are kept in the UI (area:37/39).

---

## 15. Torrent path (OBSERVED, `torrentutils/`)

### 15.1 `getTorrentInfo(url, title)` (`TorrentUtils.kt:20-42`)
1. If `url.startsWith("magnet")` (case-sensitive):
   - `torrentServerApi.addTorrent(url, title, "", "", false)`.
   - `SocketTimeoutException` → throw `DeadTorrentException()`.
   - Any other `Exception` → throw `DisabledTorrServerException()` (`aniyomi.core.common.torrent`, out of scope). User text: `AYMR.strings.torrserver_disabled` = "TorrServer is not running" (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:390`). Mapping lives in `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt`.
2. Else (`.torrent` file URL): `network.client.get(url)` (awaitSuccess semantics) → `TorrentHelpers.parseTorrentDetailsFromTorrentFileContent(body.byteStream())`.
3. `torrentToTorrentInfo(torrent, title)`.

### 15.2 `torrentToTorrentInfo` (OBSERVED, `TorrentUtils.kt:63-73`)
- `title = overrideTitle ?: torrent.title`. `overrideTitle` is the non-null `title` argument, so the torrent's own title is never used (even an empty string wins). Recorded as E-15.
- `files = torrent.fileStats?.map { TorrentFile(file.path, file.id ?: 0, file.length, torrent.hash!!, torrent.trackers ?: []) } ?: []`.
- `hash = torrent.hash!!` (NPE when absent; E-16).
- `size = torrent.torrentSize ?: -1` (Long).
- `trackers = torrent.trackers ?: []`.

### 15.3 Magnet per file (OBSERVED, `TorrentFile.kt:12-15`)
`magnet:?xt=urn:btih:<hash>` + (if trackers non-empty: `&tr=` + trackers each URL-encoded with `URLEncoder.encode(t, "UTF-8")` (form encoding, space becomes `+`) joined by `&tr=`) + `&index=<indexFile>`.

Test vector: hash `abc`, trackers `["udp://tracker.example:80/announce?x=a b"]`, index `2` → `magnet:?xt=urn:btih:abc&tr=udp%3A%2F%2Ftracker.example%3A80%2Fannounce%3Fx%3Da+b&index=2`.

### 15.4 Player handoff (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:1087-1105`)
If `torrentPreferences.torrServerEnable()` and (`videoUrl.startsWith(torrentServerApi.hostUrl)` or `startsWith("magnet")` or `endsWith("torrent")`): start the TorrServer service and call `torrentLinkHandler(videoUrl, videoTitle, videoOptions)`. `videoOptions = mpvArgs.joinToString(",") { "$option=\"$value\"" }`.

---

## 16. Playback and download option consumption (OBSERVED)

### 16.1 mpv (`PlayerActivity.kt:1089`)
`mpvArgs` pairs become `option="value"` joined by `,`. Passed to the torrent handler (above). Normal playback uses the same string format (see area:39 for the player).

### 16.2 ffmpeg (`AnimeDownloader.kt:579-668`)
- Headers: `headers = video.headers ?: download.source.headers`; `headerOptions = headers.joinToString("", "-headers '", "'") { "${name}: ${value}\r\n" }`. Note the `\r\n` terminators inside the single-quoted argument.
- Input order: for each input URL that starts with `http`, `headerOptions` is added before `-i "<url>"`. `ffmpegStreamArgs` are added before the main input as `-<key> "<value>"`.
- Mapping: `-map 0:v`; audio tracks `-map (i + 1 + subtitleCount):a` for each audio input; `-map 0:a?`; subtitle tracks `-map (i + 1):s`; `-map 0:s? -map 0:t?`.
- Output: `-f matroska -c:a copy -c:v copy -c:s copy`, metadata `-metadata:s:<type>:<i> "title=<lang>"` for each track, `ffmpegVideoArgs` as `-<key> "<value>"`, then `"<file>" -y`.
- Duration from ffprobe with the same headers (`getDuration`).
- Success: rename `.tmp` to `.mkv`. Failure: `Exception("Error in ffmpeg!")`.

Proposal: build the same argument vector (no shell quoting) and run the bundled or system `ffmpeg` with `std::process::Command`; pass headers as `-headers` with CRLF; stop using FFmpegKit (Android only).

### 16.3 Downloaded video metadata (OBSERVED, `PlayerActivity.kt:1305-1330`)
For `file`, `content` or `data` URLs only, mpv metadata key `ANIYOMI_MPV_ARGS` (`Video.MPV_ARGS_TAG`) is split on `;`, each item split on `=` with limit 2, and set with `setPropertyString(option, value)`. Errors log ERROR "Failed to read video metadata".

---

## 17. Manga-side behaviors (overlap with area:15)

Covered here only where the anime API shares code:
- `HttpSource` (`source/online/HttpSource.kt`) mirrors the anime base class, with three deliberate differences:
  1. `getUrlWithoutDomain` replaces spaces with `%20` before `URI(...)` (`:412`). The anime version does not. Effect: an anime URL containing a space fails to parse and is returned unchanged (with its domain), E-06.
  2. `getImage(page)` uses `client.newCachelessCallWithProgress(imageRequest(page), page).awaitSuccess()` (`:370-373`), which disables the cache and reports progress through `Page.update` (`source/model/Page.kt:43-49`). Progress: `100 * bytesRead / contentLength` (integer), or `-1` if `contentLength <= 0`. The progress call uses `callTimeout(30, HOURS)` (`OkHttpExtensions.kt:119-131`).
  3. `Page` state enum `QUEUE, LOAD_PAGE, DOWNLOAD_IMAGE, READY, ERROR` (`Page.kt:51-57`); `status` and `progress` are `StateFlow`s.
- `CatalogueSource` (manga) has `supportsLatest`, `getFilterList`, and abstract methods; no combined update API.
- `UpdateStrategy` (manga) is identical to `AnimeUpdateStrategy`.

---

## 18. Concurrency and dispatchers (OBSERVED)
- Source methods are `suspend`. They run on the caller's context. The app calls them under `withIOContext` (`UpdateAnimeFromRemote.kt`) and `Dispatchers.IO` (`HosterLoader.kt:82`).
- `AnimeCatalogueSource.getAnimeEpisodeUpdate` and `getAnimeSeasonUpdate` run their two children in parallel inside `supervisorScope`.
- Hoster loading in `getBestVideo` runs one `async` per hoster (unbounded). Early return cancels the rest.
- Image tile fetch is blocking inside a suspend function (`execute()`), so it blocks the caller's thread. E-09.

Proposal (see p1-rust-design.md): every source call is an `async` method on the tokio runtime; GTK receives results via `glib::MainContext::channel`. Source calls never block the GTK main thread. Concurrency per source is bounded by the rate limiter (area:12), not by the source layer.

---

## 19. Timings, limits and constants (collected)
| Name | Value | Where |
|---|---|---|
| HTTP `max-age` default | 600 s (10 min) | `core/common/.../Requests.kt:13` |
| Progress request call timeout | 30 h | `OkHttpExtensions.kt:122` |
| Default source version id | 1 | `AnimeHttpSource.kt:60` |
| Source id bit mask | `Long.MAX_VALUE` (sign bit cleared) | `AnimeHttpSource.kt:104` |
| Preview tile preload count | 2 | `PlayerViewModel.kt:1592` |
| Download warning notification timeout | 30 000 ms | `AnimeDownloader.kt:940` |
| Downloads queued warning threshold | 20 | `AnimeDownloader.kt:942` |
| Episodes per source queue warning | 10 | `AnimeDownloader.kt:941` |
| Anime per source library warning | 60 | `AnimeLibraryUpdateNotifier.kt:413` |
| Local server placeholder | `http://localhost:1` | `HttpServer.kt:33` |
| Local server bind port | 0 (ephemeral) | `HttpServer.kt:7` |
| Default user agent | Firefox 136 string | `NetworkPreferences.kt:22` |

---

## 20. Error paths and user-visible text (scope)
| Situation | Behavior | Key / text |
|---|---|---|
| Non-2xx response | `HttpException(code)` | formatted by `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt` (outside scope) |
| TorrServer down (magnet) | `DisabledTorrServerException` | `AYMR.strings.torrserver_disabled` "TorrServer is not running" |
| TorrServer timeout (magnet) | `DeadTorrentException` | (no string in scope; mapped by the caller) |
| Hoster error | `HosterState.Error(hosterName)` | UI (area:39) |
| Local server start failure | service stops | `AYMR.strings.http_server_start_failure` "Failed to start http server" |
| Local server running | foreground notice | `AYMR.strings.http_server_is_running` "Http server is running"; action `MR.strings.action_close` "Close" |
| Download queue large | warning notification | `AYMR.strings.download_queue_size_warning` |
| Thumbnail failure | silent for user, logged | "Failed to fetch thumbnails" (logcat) |

---

## 21. Interactions with other areas
- area:02 preferences: key storage backend for `source_<id>`.
- area:04/05 DB: `fetch_type`, `update_strategy`, `season_number`, `memo` persistence.
- area:07/08: episode/season sync, library update decisions.
- area:09: source manager (`sourceManager.get`, `getOrStub`).
- area:12: HTTP client, default cache header, user agent, cookie and Cloudflare handling, rate limits (`UnmeteredSource` exemption).
- area:13: torrent helpers and storage used by `TorrentHelpers` (outside scope).
- area:15: manga-side source API (shared files).
- area:19/20: extension loading. The whole APK loader is outside this scope; this scope only defines what an extension implements.
- area:23/24: anime downloads (ffmpeg, HTTP server use, queue warnings).
- area:26: library update job and `ONLY_FETCH_ONCE` skip.
- area:36/39: player and entry screens.
- area:14: local anime source (`LocalAnimeSource`) implements `AnimeSource`-family contracts.
