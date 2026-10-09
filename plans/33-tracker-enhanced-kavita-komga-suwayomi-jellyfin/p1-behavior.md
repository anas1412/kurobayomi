# area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin: behaviour spec

Source root: `/home/blackbox/aniyomi-src`. Every `path:line` is repo-relative. Labels: **OBSERVED** = what the Kotlin does (read from source). **Proposal:** = what the Rust port should do. Defect ids `D-nn` are listed in section 6 and referenced from tasks.

Upstream server behaviour (Komga, Kavita, Suwayomi) was checked against the upstream server repositories with read-only `gh api` calls. Those facts are labelled **UPSTREAM** and cite the server file.

---

## 0. Tracker identity table

OBSERVED. From `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:19-37` and each tracker's constructor.

| Tracker | Class | Id (`Long`) | Kind | Logo resource | Logo colour (`Color.rgb`) | Accepted source class name | Status constants |
|---|---|---|---|---|---|---|---|
| Komga | `Komga` (`komga/Komga.kt:20`) | 6 | Manga, enhanced | `R.drawable.ic_tracker_komga` | `rgb(51, 37, 50)` = `#332532` (`Komga.kt:37`) | `eu.kanade.tachiyomi.extension.all.komga.Komga` (`Komga.kt:97`) | UNREAD 1, READING 2, COMPLETED 3 (`Komga.kt:23-25`) |
| Kavita | `Kavita` (`kavita/Kavita.kt:23`) | `KAVITA = 8L` | Manga, enhanced | `R.drawable.ic_tracker_kavita` | `rgb(74, 198, 148)` = `#4AC694` (`Kavita.kt:40`) | `eu.kanade.tachiyomi.extension.all.kavita.Kavita` (`Kavita.kt:99`) | UNREAD 1, READING 2, COMPLETED 3 (`Kavita.kt:26-28`) |
| Suwayomi | `Suwayomi` (`suwayomi/Suwayomi.kt:18`) | 9 | Manga, enhanced | `R.drawable.ic_tracker_suwayomi` | `rgb(255, 35, 35)` = `#FF2323`, comment `// TODO` (`Suwayomi.kt:24`) | `eu.kanade.tachiyomi.extension.all.tachidesk.Tachidesk` (`Suwayomi.kt:89`) | UNREAD 1, READING 2, COMPLETED 3 (`Suwayomi.kt:27-29`) |
| Jellyfin | `Jellyfin` (`jellyfin/Jellyfin.kt:20`) | `JELLYFIN = 102L` | Anime, enhanced | `R.drawable.ic_tracker_jellyfin` | `rgb(0, 11, 37)` = `#000B25` (`Jellyfin.kt:39`) | `eu.kanade.tachiyomi.animeextension.all.jellyfin.Jellyfin` (`Jellyfin.kt:86`) | UNSEEN 1, WATCHING 2, COMPLETED 3 (`Jellyfin.kt:23-25`) |

All four: `supportsReadingDates = false`, `supportsPrivateTracking = false` (defaults from `BaseTracker.kt:23-25`). All four: `getScoreList()` returns an empty list and `displayScore()` returns `""` (Kavita `Kavita.kt:57-59`, Komga `Komga.kt:54-56`, Suwayomi `Suwayomi.kt:47-49`, Jellyfin `Jellyfin.kt:56-58`). `getRereadingStatus()` / `getRewatchingStatus()` return `-1` (`Kavita.kt:53`, `Komga.kt:50`, `Suwayomi.kt:43`, `Jellyfin.kt:52`). So the score, rereading and private features are absent for these trackers.

Status labels (OBSERVED):
- Kavita, Komga, Suwayomi: `UNREAD -> MR.strings.unread`, `READING -> MR.strings.reading`, `COMPLETED -> MR.strings.completed`, else `null` (`Kavita.kt:44-49`, `Komga.kt:41-46`, `Suwayomi.kt:34-39`).
- Jellyfin: `UNSEEN -> AYMR.strings.unseen`, `WATCHING -> AYMR.strings.watching`, `COMPLETED -> MR.strings.completed`, else `null` (`Jellyfin.kt:43-48`).
- Status list order: `listOf(UNREAD, READING, COMPLETED)` (`Kavita.kt:42`, `Komga.kt:39`, `Suwayomi.kt:32`) and `listOf(UNSEEN, WATCHING, COMPLETED)` for Jellyfin (`Jellyfin.kt:41`).
- Reading status (set on progress): `READING` for Kavita, Komga, Suwayomi (`Kavita.kt:51`, `Komga.kt:48`, `Suwayomi.kt:41`); `WATCHING` for Jellyfin (`Jellyfin.kt:50`).
- Completion status: `COMPLETED` for all four (`Kavita.kt:55`, `Komga.kt:52`, `Suwayomi.kt:45`, `Jellyfin.kt:54`).

English strings (OBSERVED, `i18n/src/commonMain/moko-resources/base/strings.xml:774-776`, `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:592` and `:518`): `unread` = "Unread", `reading` = "Reading", `completed` = "Completed", `unseen` = "Unseen", `watching` = "Watching".

---

## 1. Framework these trackers depend on (OBSERVED, owned by area:27-tracking-core)

### 1.1 Credentials and login state (`BaseTracker.kt`)

- `isLoggedIn` = `getUsername().isNotEmpty() && getPassword().isNotEmpty()` (`BaseTracker.kt:32-34`).
- `saveCredentials(u, p)` calls `trackPreferences.setCredentials(this, u, p)` (`BaseTracker.kt:49-51`). `logout()` saves `("", "")` (`BaseTracker.kt:27-30`).
- Enhanced trackers call `saveCredentials("user", "pass")` from both `login()` and `loginNoop()` (`Kavita.kt:89-97`, `Komga.kt:87-95`, `Suwayomi.kt:80-86`, `Jellyfin.kt:78-84`). The real credentials live in the source extension's own preferences. The dummy pair only makes `isLoggedIn` true (comment at `Komga.kt:91-92`).
- Settings UI: the "Enhanced trackers" group (`SettingsTrackingScreen.kt:224-245`) shows one row per enhanced tracker whose accepted source is installed (see 1.4). The row's login action calls `loginNoop()` (`SettingsTrackingScreen.kt:231`, `:239`) and its logout action calls `service.logout` (`:232`, `:240`).

### 1.2 Enhanced-tracker contract (OBSERVED)

`EnhancedMangaTracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt:12-41`):
- `accept(source) = source::class.qualifiedName in getAcceptedSources()` (`:16-18`).
- `getAcceptedSources()`, `loginNoop()`, `match(manga): MangaTrackSearch?`, `isTrackFrom(track, manga, source): Boolean`, `migrateTrack(track, manga, newSource): MangaTrack?`.

`EnhancedAnimeTracker` (`EnhancedAnimeTracker.kt:12-41`) is the same shape for anime.

The doc comment on both says: "A tracker that will never prompt the user to manually bind an entry." So these trackers are never searched by the user. They bind automatically.

### 1.3 Automatic binding (OBSERVED)

`AddMangaTracks.bindEnhancedTrackers(manga, source)` (`app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt:81-105`):
1. `trackerManager.loggedInTrackers().filterIsInstance<EnhancedMangaTracker>().filter { it.accept(source) }` (`:82-85`).
2. For each: `service.match(manga)` (`:88`). If it returns a track: `track.manga_id = manga.id`, then `service.mangaService.bind(track)` (`:89-90`), then `insertTrack.await(track.toDomainTrack(idRequired = false)!!)` (`:91`), then `syncChapterProgressWithTrack.await(manga.id, domainTrack, service.mangaService)` (`:93-97`).
3. Any exception is logged and swallowed (`:99-101`).
4. The whole loop runs inside `withNonCancellableContext { withIOContext { ... } }` (`:81-82`).

Callers of `bindEnhancedTrackers` (OBSERVED): `BrowseMangaSourceScreenModel.kt:248`, `MangaScreenModel.kt:374`, `MangaHistoryScreenModel.kt:225`. Anime equivalents: `BrowseAnimeSourceScreenModel.kt:240`, `AnimeScreenModel.kt:450`, `AnimeHistoryScreenModel.kt:220`.

### 1.4 Installed-source filter in the settings screen (OBSERVED)

`SettingsTrackingScreen.kt:116-135`: an enhanced tracker is listed as "available" if any installed catalogue source's class name is in its accepted list. Otherwise it is listed under the "Available but source not installed" text (`enhanced_services_not_installed`, `i18n/.../strings.xml:536`). The info text is `enhanced_tracking_info` (`strings.xml:537`) followed by a blank line and the missing-source text with the tracker names joined by `", "` (`joinToString { it.name }`, `:133`).

### 1.5 Remote persistence and sync (OBSERVED)

- `SyncChapterProgressWithTrack.await(mangaId, remoteTrack, tracker)` (`app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt:21-50`):
  1. Return if the tracker is not `EnhancedMangaTracker` (`:26-28`).
  2. `sortedChapters = chapters.sortedBy { chapterNumber }.filter { isRecognizedNumber }` (`:30-32`).
  3. `chapterUpdates` = chapters with `chapterNumber <= remoteTrack.lastChapterRead && !read`, mapped to `read = true` (`:34-36`).
  4. `localLastRead = sortedChapters.takeWhile { read }.lastOrNull()?.chapterNumber ?: 0F` (`:39`). Only the continuous read prefix counts.
  5. `lastRead = max(remoteTrack.lastChapterRead, localLastRead.toDouble())` (`:40`). `updatedTrack = remoteTrack.copy(lastChapterRead = lastRead)` (`:41`).
  6. In a `try` (`:43`): `tracker.update(updatedTrack.toDbTrack())` (`:44`), then `updateChapter.awaitAll(chapterUpdates)` (`:45`), then `insertTrack.await(updatedTrack)` (`:46`). Any `Throwable` is logged at WARN and swallowed (`:47-49`).
- Anime: `SyncEpisodeProgressWithTrack.kt:21-50` is identical with `lastEpisodeSeen`, `episodeNumber` and `seen`.
- `TrackChapter` (`app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt:25-59`), for every logged-in tracker whose chapter number is greater than `track.lastChapterRead`: `updatedTrack = service.mangaService.refresh(track.toDbTrack()).toDomainTrack(idRequired = true)!!.copy(lastChapterRead = chapterNumber)` (`:40-42`); then `service.mangaService.update(updatedTrack.toDbTrack(), true)` (`:43`); then `insertTrack.await(updatedTrack)` (`:44`). Failures are queued in `delayedTrackingStore` (`:46-50`).
- `MangaTracker.updateRemote(track)` (`MangaTracker.kt:111-121`) calls `update(track)` (default `didReadChapter = false`) and then `insertTrack.await(track.toDomainTrack)` on the same object. It is used by `setRemoteMangaStatus`, `setRemoteLastChapterRead`, `setRemoteScore`, `setRemoteStartDate`, `setRemoteFinishDate`, `setRemotePrivate` (`MangaTracker.kt:66-109`). Errors go to a toast with `e.message` (`:117-119`).

### 1.6 `copyPersonalFrom` (OBSERVED)

`MangaTrack.copyPersonalFrom(other, copyRemotePrivate = true)` (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:37-44`) copies `last_chapter_read`, `score`, `status`, `started_reading_date`, `finished_reading_date`, and `private` (when `copyRemotePrivate`). `AnimeTrack.copyPersonalFrom` (`anime/AnimeTrack.kt:37-44`) copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, `private`.

The search models default `score = -1.0`, `started/finished = 0`, `private = false`, and `cover_url = ""` (`MangaTrackSearch.kt:25-41`, `AnimeTrackSearch.kt` same). The four trackers never set `score`, `started_*`, `finished_*`, or `private` on the search result. So `refresh()` overwrites local values with defaults (defect D-03).

---

## 2. Kavita (`app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/`)

### 2.1 Preference keys read from the Kavita source (OBSERVED)

For each slot `n` in `1..3` (`Kavita.kt:118-148`):
- Source id = MD5 derivation of `"kavita_$n/all/1"` (`Kavita.kt:122-129`). Worked values: slot 1 `7266875667263635239`, slot 2 `3764816291200082561`, slot 3 `778637366235559582` (computed with the algorithm in `p1-data-and-config.md` section 4).
- Preference file = `source_<id>` (`ConfigurableSource.kt:20-22` via `sourcePreferences()`).
- `APIURL` default `""`, `APIKEY` default `""` (`Kavita.kt:131-132`).

### 2.2 `loadOAuth()` algorithm (OBSERVED, `Kavita.kt:118-148`)

```
oauth = new OAuth()                                  // 3 SourceAuth slots, all apiUrl="" jwt=""
for id in 1..3:
    authentication = oauth.authentications[id - 1]
    sourceId = md5_id("kavita_<id>/all/1")
    prefs = (sourceManager.get(sourceId) as ConfigurableSource).sourcePreferences()   // (A)
    prefApiUrl = prefs.getString("APIURL", "")
    prefApiKey = prefs.getString("APIKEY", "")
    if prefApiUrl empty or prefApiKey empty: continue                                  // not configured
    token = api.getNewToken(prefApiUrl, prefApiKey)                                    // (B)
    if token is null or empty: continue                                               // unreachable
    authentication.apiUrl = prefApiUrl
    authentication.jwtToken = token
authentications = oauth                                                                // only after the loop
```

- (A) If the source is not installed, `sourceManager.get` returns `null` and the cast to a non-null type throws. The exception is not caught, so `loadOAuth` aborts (defect D-05).
- (B) An exception from `getNewToken` also aborts the loop (D-05).
- `authentications` is assigned only when the loop finishes normally. If the loop throws, the field stays `null` and the next request retries `loadOAuth` (`KavitaInterceptor.kt:14-16`).

### 2.3 JWT acquisition: `KavitaApi.getNewToken(apiUrl, apiKey)` (OBSERVED, `KavitaApi.kt:40-84`)

Request: `POST {apiUrl}/Plugin/authenticate?apiKey={apiKey}&pluginName=Tachiyomi-Kavita`, body `{}`, content type `application/json; charset=utf-8` (`KavitaApi.kt:41-45`). The key is not URL-encoded (Proposal: encode it). `apiUrl` must already end in `/api` because the path is appended directly (`:43`). `getApiFromUrl` (`:30-32`) produces the same form from a series URL.

The request runs on the constructor `client`, which is the DoH-enabled base client, not `authClient` (defect D-04). It uses `execute()` (blocking), inside `use {}`.

Response handling (`:47-70`):
| HTTP code | Action | Returns |
|---|---|---|
| 200 | `parseAs<AuthenticationDto>().token` | the token string |
| 401 | log WARN "Unauthorized / api key not valid: Cleaned api URL: {apiUrl}, Api key is empty: {apiKey.isEmpty()}", then `throw IOException("Unauthorized / api key not valid")` | (caught by the generic handler, see below) |
| 500 | log WARN "Error fetching JWT token. Cleaned api URL: ...", then `throw IOException("Error fetching JWT token")` | (caught by the generic handler) |
| any other | nothing | falls through to `return null` (`:78`) |

Exception handling (`:72-84`):
- `SocketTimeoutException`: log WARN "Could not fetch JWT token. Probably due to connectivity issue or the url '{apiUrl}' is not available, skipping" and return `null`.
- Any other `Exception` (this includes the IOExceptions just thrown for 401 and 500, and deserialization errors): log ERROR "Unhandled exception fetching JWT token for url: '{apiUrl}'" and `throw IOException(e)`.

So the 401 and 500 messages are never seen as-is. The caller gets `IOException(IOException("Unauthorized / api key not valid"))` whose message is `java.io.IOException: Unauthorized / api key not valid` (defect D-06). A 401 or 500 aborts `loadOAuth` (D-05). Any other non-200 code (for example 403 or 404) silently skips that slot.

### 2.4 Auth interceptor (OBSERVED, `KavitaInterceptor.kt:9-24`)

For every request on `authClient`:
1. If `kavita.authentications == null`, call `kavita.loadOAuth()` (`:11-13`).
2. `jwtToken = authentications?.getToken(kavita.api.getApiFromUrl(request.url.toString()))` (`:15-17`).
3. Add `Authorization: Bearer $jwtToken` (`:23` with `.addHeader`) and `User-Agent: Aniyomi v${BuildConfig.VERSION_NAME} (${BuildConfig.APPLICATION_ID})` (`:24` with `.header`), then proceed.

`OAuth.getToken(apiUrl)` (`KavitaModels.kt:72-79`) returns the `jwtToken` of the first slot whose `apiUrl` equals the argument (exact string match). It returns `null` when none matches. The header then becomes `Bearer null` (string) (Proposal: fail fast with "not logged in").

- No refresh: the token is never renewed. `authentications` is never reset to `null`, so the token lives until the process restarts (D-07).

### 2.5 Series read: `KavitaApi.getTrackSearch(url)` (OBSERVED, `KavitaApi.kt:150-176`)

Input `url` is a series URL such as `{host}/api/series/{id}` (the extension's `manga.url`). Steps, in order:
1. `GET url` on `authClient` with `awaitSuccess()` and parse `SeriesDto` (`:152-156`).
2. `track = seriesDto.toTrack()` (`KavitaModels.kt:25-29`): `title = name`, `summary = ""`, with `create(KAVITA)` setting `tracker_id = 8`.
3. `cover_url = seriesDto.thumbnail_url.toString()` (`:158`). The Kotlin property `thumbnail_url` is also the JSON key (no `@SerialName`). The upstream Kavita `SeriesDto` (`Kavita.Models/DTOs/SeriesDto.cs`, read-only check) has no such property, so the field is always its default `""` and `cover_url` ends up `""`. The four-character string `"null"` appears only if a server sends `"thumbnail_url": null` (D-01).
4. `tracking_url = url` (`:159`).
5. `total_chapters = getTotalChapters(url)` (`:160`), see 2.6.
6. `title = seriesDto.name` (`:161`).
7. `status = when (seriesDto.pagesRead) { seriesDto.pages -> COMPLETED; 0 -> UNREAD; else -> READING }` (`:162-166`). The first branch wins, so `pages == 0 && pagesRead == 0` gives `COMPLETED` (D-24).
8. `last_chapter_read = getLatestChapterRead(url)` (`:167`), see 2.7.
9. Any exception: log WARN "Could not get item: $url" and rethrow (`:171-174`).

Total requests: 3 (series, volumes, latest-chapter).

### 2.6 Total chapters: `getTotalChapters(url)` (OBSERVED, `KavitaApi.kt:101-124`)

- Request: `GET {api}/Series/volumes?seriesId={id}` on `authClient`, parse `List<VolumeDto>` (`:103-107`). `api` = `getApiFromUrl(url)`; `id` = `getIdFromUrl(url)` (`:87-96`).
- Walk the list with two counters, `volumeNumber = 0L` and `maxChapterNumber = 0L` (`:108-109`):
  - For each volume: if `volume.chapters.maxOf { it.number!!.toFloat() } == 0f` then `volumeNumber++` (`:111-112`).
  - Else if `maxChapterNumber < volume.chapters.maxOf { it.number!!.toFloat() }` then `maxChapterNumber = volume.chapters.maxOf { it.number!!.toFloat().toLong() }` (`:113-115`). The assigned value is truncated to a whole number, so `10.5` becomes `10`.
- Result: `return if (maxChapterNumber > volumeNumber) maxChapterNumber else volumeNumber` (`KavitaApi.kt:119`).
- Failure: any exception is logged at WARN "Exception fetching Total Chapters. Request:{requestUrl}" and rethrown (`:120-123`, log at `:121`).

Edge behaviour (OBSERVED from the Kotlin semantics):
- A volume with an empty `chapters` list makes `maxOf` throw `NoSuchElementException` (D-23). The whole call fails, so `getTrackSearch` fails, `match` returns `null`, and `refresh` throws.
- A `number` that is not numeric makes `toFloat()` throw `NumberFormatException` (D-23).
- `number` is nullable and `!!` is used, so a JSON `null` throws `NullPointerException`.
- `VolumeDto.number` (`Int`) is not used by this function.

UPSTREAM (Kavita server, `Kavita.Services/TachiyomiService.cs`, read-only research): loose-leaf volumes are encoded in chapter numbers as `volume * 10000` (`GetChaptersUntilVolume` uses `chapterNumber * 10_000`, and `GetLatestChapter` uses `number / 10_000f`). The Kotlin total-chapter count does not decode this. It is recorded as a risk in `p1-topic-kavita-tracker.md`.

### 2.7 Latest read chapter: `getLatestChapterRead(url)` (OBSERVED, `KavitaApi.kt:126-148`)

- `GET {api}/Tachiyomi/latest-chapter?seriesId={seriesId}` on `authClient` (`:127-128`).
- HTTP 200: `parseAs<ChapterDto>().number!!.replace(",", ".").toDouble()`. The comma replacement handles locale decimals (`:133`).
- HTTP 204: `0.0` (`:137-139`).
- Any other code: falls through to `return 0.0` (`:146`), silently.
- Exception: log WARN "Exception getting latest chapter read. Could not get itemRequest: {requestUrl}" and rethrow (`:141-145`).
- Upstream (`Kavita.Server/Controllers/TachiyomiController.cs:26-35`, `Kavita.Services/TachiyomiService.cs:38-94`): returns 204 when the user has no progress (`PagesRead == 0` or `PagesRead > Pages`). Otherwise returns the highest fully-read chapter, or for volume-only series `volumeNumber * 10000`. The client does not decode that encoding.
- `number` of `"-1"` (the `ChapterDto` default, `KavitaModels.kt:45-58`) parses to `-1.0` with no guard (D-22).

Total requests for `getTrackSearch`: 3.

### 2.8 Mark progress: `KavitaApi.updateProgress(track)` (OBSERVED, `KavitaApi.kt:178-193`)

1. `POST {api}/Tachiyomi/mark-chapter-until-as-read?seriesId={id}&chapterNumber={track.last_chapter_read}` on `authClient`, body `{}` with content type `application/json; charset=utf-8`, `awaitSuccess()`.
   - `last_chapter_read` is a Kotlin `Double` in string-template form: `12.0`, `12.5`. It is not URL-encoded.
2. Return `getTrackSearch(track.tracking_url)` (`:192`). The server state is therefore re-read after each write.

UPSTREAM (`Kavita.Services/TachiyomiService.cs:108-150`, `:164-172`): marks every non-special chapter with `MaxNumber <= chapterNumber` as read. `chapterNumber < 1.0` is treated as a volume number (`* 10_000`). The endpoint returns `200` with a bool body; the client ignores the body.

Total requests: 4 (POST, then series, volumes, latest).

### 2.9 Kavita `update(track, didReadChapter)` (OBSERVED, `Kavita.kt:61-72`)

```
if track.status != COMPLETED:
    if didReadChapter:
        if track.last_chapter_read.toLong() == track.total_chapters && track.total_chapters > 0:
            track.status = COMPLETED
        else:
            track.status = READING
return api.updateProgress(track)
```

The `track` mutated here is the copy passed in by the caller. `SyncChapterProgressWithTrack.kt:44` and `TrackChapter.kt:43` pass `toDbTrack()` copies, and the return value is discarded. `updateRemote` (`MangaTracker.kt:113`) passes `didReadChapter = false`, so the branch is skipped. So the status computed here is never persisted (D-02). The status in the DB comes only from `refresh()`, which takes the server's value. **Proposal:** remove the local status branch and keep `update` = write + no-op. Keep the status fully server-driven.

### 2.10 Kavita `refresh`, `match`, `bind`, `search`, `login`

- `refresh(track)`: `remoteTrack = api.getTrackSearch(track.tracking_url)`; `track.copyPersonalFrom(remoteTrack)`; `track.total_chapters = remoteTrack.total_chapters`; return `track` (`Kavita.kt:82-87`). Effect: `score`, `started/finished_reading_date`, and `private` are overwritten (D-03).
- `match(manga)`: `try { api.getTrackSearch(manga.url) } catch (e: Exception) { null }` (`Kavita.kt:101-106`). All errors become `null`, which hides auth and network failures from the bind flow.
- `bind(track, hasReadChapters)`: returns the track unchanged (`Kavita.kt:74-76`).
- `searchManga(query)`: `TODO("Not yet implemented: search")` (`Kavita.kt:78-80`). This throws `NotImplementedError`, which is an `Error`, not an `Exception`. So `catch (e: Exception)` does not catch it (D-21).
- `login(u, p)` and `loginNoop()`: `saveCredentials("user", "pass")` (`Kavita.kt:89-97`).
- `isTrackFrom(track, manga, source)`: `track.remoteUrl == manga.url && source?.let { accept(it) } == true` (`Kavita.kt:108-110`).
- `migrateTrack(track, manga, newSource)`: `if accept(newSource) track.copy(remoteUrl = manga.url) else null` (`Kavita.kt:111-116`).

---

## 3. Komga (`app/src/main/java/eu/kanade/tachiyomi/data/track/komga/`)

### 3.1 Client (OBSERVED, `Komga.kt:28-33`, `KomgaApi.kt:28-32`)

- `client` = `networkService.client.newBuilder().dns(Dns.SYSTEM).build()` (eager `val`, `Komga.kt:28-31`). There is no auth interceptor.
- Default headers: only `User-Agent: Aniyomi v${BuildConfig.VERSION_NAME} (${BuildConfig.APPLICATION_ID})` (`KomgaApi.kt:29-31`).
- No `Authorization` header is sent. Authentication therefore depends on cookies in the shared `AndroidCookieJar` (`NetworkHelper.kt:18`, `:22`). This is an open question (D-19). If the cookie is absent, requests return 401 and the tracker fails.

### 3.2 Readlist versus series (OBSERVED, `KomgaApi.kt:21`, `:40-50`)

`READLIST_API = "/api/v1/readlists"`. A tracking URL that contains that substring is a readlist. Otherwise it is a series.

### 3.3 `getTrackSearch(url)` (OBSERVED, `KomgaApi.kt:36-82`)

1. Track: if readlist, `GET url` parse `ReadListDto` → `toTrack()` = `{ title = name }` (`:109-113`). Otherwise `GET url` parse `SeriesDto` → `toTrack()` = `{ title = metadata.title, summary = metadata.summary, publishing_status = metadata.status }` (`:103-107`).
2. Progress: `GET {url with "/api/v1/series/" replaced by "/api/v2/series/"}/read-progress/tachiyomi` (`:53-56`). Note `String.replace` replaces every occurrence.
   - If `url` contains `/api/v1/series/` (`:59`): parse `ReadProgressV2Dto` (`:60`).
   - Otherwise (readlists, and any URL that is not the v1 series form): parse `ReadProgressDto` then `toV2()` (`:62`, `KomgaModels.kt:89-98`).
   - Note the progress URL for a readlist is **not** rewritten, so it stays `/api/v1/readlists/{id}/read-progress/tachiyomi`.
3. `track.apply`:
   - `cover_url = "$url/thumbnail"` (`:68`).
   - `tracking_url = url` (`:69`).
   - `total_chapters = progress.maxNumberSort.toLong()` (`:70`). Float truncated to whole.
   - `status = when (progress.booksCount) { progress.booksUnreadCount -> UNREAD; progress.booksReadCount -> COMPLETED; else -> READING }` (`:71-75`). The first matching branch wins, so an empty series (`0, 0, 0`) is `UNREAD`.
   - `last_chapter_read = progress.lastReadContinuousNumberSort` (`:76`).
4. Exception: log WARN "Could not get item: $url" and rethrow (`:78-81`).

Total requests: 2 (item, progress).

UPSTREAM check (Komga `komga/src/main/kotlin/org/gotson/komga/interfaces/api/rest/SeriesController.kt:773-790`): `GET v2/series/{seriesId}/read-progress/tachiyomi` and `PUT v2/series/{seriesId}/read-progress/tachiyomi` exist and are documented as "Mihon specific". The v1 series path has no progress endpoint. So the v2 rewrite is required. `ReadListController.kt:411-435` has `GET` and `PUT {id}/read-progress/tachiyomi` for readlists. Both exist.

### 3.4 `updateProgress(track)` (OBSERVED, `KomgaApi.kt:84-101`)

1. Payload:
   - If `tracking_url` contains `/api/v1/series/`: `ReadProgressUpdateV2Dto(track.last_chapter_read)` encoded as `{"lastBookNumberSortRead": <Double>}` (`:85-86`).
   - Else: `ReadProgressUpdateDto(track.last_chapter_read.toInt())` encoded as `{"lastBookRead": <Int>}` (`:87-88`). `toInt()` truncates.
2. `PUT {tracking_url with /api/v1/series/ replaced by /api/v2/series/}/read-progress/tachiyomi`, headers as in 3.1, body `Content-Type: application/json` (no charset), `awaitSuccess()` (`:90-99`).
3. Return `getTrackSearch(track.tracking_url)` (`:100`).

UPSTREAM semantics:
- Series v2 PUT (`SeriesController.kt:784-801`): marks every book with `metadata.numberSort <= lastBookNumberSortRead` as complete, skipping books already complete. It never marks books unread. Responds `204`.
- Readlist PUT (`ReadListController.kt:421-438`): marks the first `lastBookRead` books (by `readList.number`, index `< lastBookRead`) as complete. `lastBookRead` is a count, not a number. Responds `404` if the readlist is not visible to the user.

Total requests: 3 (PUT, item, progress).

### 3.5 Komga `update`, `refresh`, `match`, and others (OBSERVED, `Komga.kt`)

- `update(track, didReadChapter)` (`:58-70`): same branch as Kavita (2.9), including the dead status write (D-02), then `api.updateProgress(track)`.
- `refresh(track)` (`:80-85`): `copyPersonalFrom(remote)` and `total_chapters = remote.total_chapters` (D-03).
- `searchManga(query)`: `throw Exception("Not used")` (`:76-78`).
- `match(manga)`: `try { api.getTrackSearch(manga.url) } catch (e: Exception) { null }` (`:99-104`).
- `bind(track, hasReadChapters)`: returns `track` (`:72-74`).
- `login`, `loginNoop`: `saveCredentials("user", "pass")` (`:87-95`).
- `getAcceptedSources()`: `["eu.kanade.tachiyomi.extension.all.komga.Komga"]` (`:97`).
- `isTrackFrom`: URL equals and accepted (`:106-107`). `migrateTrack`: copy `remoteUrl` (`:109-114`).

---

## 4. Suwayomi (`app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/`)

### 4.1 Settings (OBSERVED, `SuwayomiApi.kt:103-126`)

- Source id = MD5 of `"tachidesk/en/1"` (`:103-107`). Worked value `3100117499901280806`.
- Preference file: `source_<id>` (`:109-111`), opened with `Context.MODE_PRIVATE` equivalent (`0x0000`).
- Keys and defaults (`:121-126`):
  - `"Server URL Address"` default `""`.
  - `"Login (Basic Auth)"` default `""`.
  - `"Password (Basic Auth)"` default `""`.
  These are the exact preference keys. They contain spaces and parentheses.
- `baseUrl`, `baseLogin`, `basePassword` are `by lazy` (`:45-47`). They are read once per `SuwayomiApi` instance (D-08).

### 4.2 Headers (OBSERVED, `SuwayomiApi.kt:35-43`)

- `User-Agent` = `network.defaultUserAgentProvider()` (`:36`), which is the user's default-UA preference (`NetworkHelper.kt:73`).
- If both `basePassword` and `baseLogin` are non-empty: `Authorization: <Credentials.basic(login, password)>` (`:37-40`). The format is `Basic base64(login:password)`.
- The headers object is `by lazy` (`:43`).

### 4.3 Base URL and track URL (OBSERVED, `SuwayomiApi.kt:49-56`)

`getTrackSearch(trackUrl)`:
- If `trackUrl.toLong()` succeeds: `url = "$baseUrl/api/v1/manga/$mangaId"`.
- If `NumberFormatException`: `url = trackUrl` (used as is).
- The stored `tracking_url` is this `url` (`:68`).
- If `baseUrl` is empty and `trackUrl` is numeric, the URL is `/api/v1/manga/123`, which is not absolute. OkHttp throws, the exception is caught in `match`, and the match returns `null` (Proposal: fail with "Server URL not set").

### 4.4 `getTrackSearch(trackUrl)` (OBSERVED, `SuwayomiApi.kt:49-78`)

1. `GET "$url/full"` with headers, `awaitSuccess()`, parse `MangaDataClass` (`:58-62`).
2. Build the result (`:64-77`):
   - `title = manga.title`.
   - `cover_url = "$url/thumbnail"`.
   - `summary = manga.description.orEmpty()`.
   - `tracking_url = url`.
   - `total_chapters = manga.chapterCount` (`Long`).
   - `publishing_status = manga.status` (a string such as `"ONGOING"`, as sent by the server).
   - `last_chapter_read = manga.lastChapterRead?.chapterNumber ?: 0.0`.
   - `status = when (manga.unreadCount) { manga.chapterCount -> UNREAD; 0L -> COMPLETED; else -> READING }` (`:72-76`). Order matters:
     - `unreadCount == chapterCount` → UNREAD, including `0 == 0` (an empty manga).
     - `unreadCount == 0` → COMPLETED.
     - Otherwise READING.
     - `unreadCount` is nullable. `null` matches neither branch and gives READING (because `null == chapterCount` is false and `null == 0L` is false).
3. Total requests: 1.

### 4.5 `updateProgress(track)` (OBSERVED, `SuwayomiApi.kt:80-101`)

1. `chapters = GET "$url/chapters"` parse `List<ChapterDataClass>` (`:82-86`).
2. `lastChapterIndex = chapters.first { it.chapterNumber == track.last_chapter_read }.index` (`:87`). The comparison is exact `Double` equality. `first` throws `NoSuchElementException` if none matches (D-25). For example `last_chapter_read = 0.0` with no chapter numbered `0` fails.
3. `PUT "$url/chapter/$lastChapterIndex"` with headers and `FormBody` (`charset = utf8`): `markPrevRead=true`, `read=true` (`:89-98`).
4. Return `getTrackSearch(track.tracking_url)` (`:100`).

UPSTREAM (`server/src/main/kotlin/suwayomi/tachidesk/manga/controller/MangaController.kt:386-402`): the chapter handler is parameterised by `mangaId` and `chapterIndex`, with form params `read`, `bookmarked`, `markPrevRead`, `lastPageRead`. Chapter index starts at 1, as `ChapterDataClass.index` says.

Total requests: 4 (chapters, PUT, item, plus the call to `getTrackSearch` in `refresh`, when used).

### 4.6 Suwayomi tracker class (OBSERVED, `Suwayomi.kt`)

- `api = SuwayomiApi(id)` (lazy, `:20`).
- `update` (`:51-63`): same dead status branch as Kavita (D-02), then `api.updateProgress(track)`.
- `refresh` (`:73-78`): `copyPersonalFrom` and total (D-03).
- `searchManga` (`:69-71`): `TODO("Not yet implemented")` (`NotImplementedError`, D-21).
- `match` (`:92-97`): `try { api.getTrackSearch(manga.url) } catch (e: Exception) { null }`.
- `isTrackFrom(track, manga, source)` (`:99-103`): `source?.let { accept(it) } == true`. **The `remoteUrl` is not compared** (D-09). Every Suwayomi-sourced entry matches any Suwayomi track in the migration dialog.
- `migrateTrack` (`:105-110`): if `accept(newSource)` copy with `remoteUrl = manga.url`, else `null`.
- `getAcceptedSources()` (`:88-90`): `["eu.kanade.tachiyomi.extension.all.tachidesk.Tachidesk"]`.
- `loginNoop` and `login` as Kavita (`:80-86`).

---

## 5. Jellyfin (`app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/`)

### 5.1 Tracking URL contract (OBSERVED from parsing in `JellyfinApi.kt` and `JellyfinInterceptor.kt`)

A tracking URL is an absolute HTTP(S) URL with:
- a path `/Users/{userId}/Items/{itemId}` (userId = `pathSegments[1]`; item id = `pathSegments.last()`),
- a **fragment** that is required (`httpUrl.fragment!!` at `JellyfinApi.kt:33` and `:126`; a missing fragment throws `NullPointerException`). Fragment forms:
  - `movie…` (`startsWith("movie")`, `:129`): the item is a movie. The item id is `pathSegments.last()`.
  - `seriesId,{seriesId}` (`startsWith("seriesId")`, `:41`): the item is a **season** (its id is the last path segment and is used as `seasonId`). The series id is `fragment.split(",").last()` (`:72-73`).
- The exact producer is the Jellyfin source extension, which is outside this repo. The consumer contract above is the spec (see `p1-data-and-config.md` section 3).

### 5.2 Client and auth (OBSERVED)

- `client` (lazy): `networkService.client.newBuilder().addInterceptor(JellyfinInterceptor()).dns(Dns.SYSTEM).build()` (`Jellyfin.kt:28-33`).
- `JellyfinInterceptor.intercept` (`JellyfinInterceptor.kt:19-42`):
  1. `uaRequest` = request with `User-Agent: Aniyomi v{VERSION} ({APPLICATION_ID})` (`:23-25`).
  2. If the URL already has query `api_key` → proceed with `uaRequest` only (`:28-30`).
  3. `userId = url.queryParameter("userId") ?: url.pathSegments[1]` (`:32`).
  4. `apiKey = apiKeys[userId] ?: getApiKey(userId)?.also { apiKeys[userId] = it } ?: throw IOException("Please log in through the extension")` (`:33-34`).
  5. Add query `api_key={apiKey}` and proceed (`:36-41`).
- `getId(suffix)` (`JellyfinInterceptor.kt:44-49`): key = `"jellyfin" + (if suffix == 1 "" else " ($suffix)") + "/all/1"`, MD5, first 8 bytes big-endian, `and Long.MAX_VALUE`.
- `getApiKey(userId)` (`:51-67`): for `i` in `1..10`: `sourceId = getId(i)`; `prefs = (sourceManager.get(sourceId) as ConfigurableAnimeSource).sourcePreferences()`; `sourceUserId = prefs.getString("user_id", "")`; if empty continue; if `sourceUserId == userId` return `prefs.getString("api_key", "")`. After the loop return `null`.
- Preference keys (defined by the Jellyfin extension): `user_id` and `api_key`, both default `""`.
- The `apiKeys` cache (`:17`) is a plain `mutableMapOf`, shared across coroutines (D-15). An empty `api_key` is cached (D-16). If slot `i` is not installed, `sourceManager.get` returns `null` and the cast throws `NullPointerException` (D-17).
- The auth key is sent as a query parameter, so it appears in every URL, including logs (Proposal: send as header `X-Emby-Token` only if the server accepts it; otherwise redact in logs).

Worked ids: `jellyfin/all/1` = `1100359934660540567`; `jellyfin (2)/all/1` = `5716273013801838310`; `jellyfin (10)/all/1` = `3559930647551045450`.

### 5.3 Item read: `JellyfinApi.getTrackSearch(url)` (OBSERVED, `JellyfinApi.kt:30-53`)

1. `httpUrl = url.toHttpUrl()`; `fragment = httpUrl.fragment!!`.
2. `GET url`, `awaitSuccess()`, parse `JFItem` → `toTrack()` (`:33-37`).
3. `toTrack()` (`:55-67`): `title = name`; `total_episodes = 1`; if `userData.played`: `last_episode_seen = 1.0`, `status = COMPLETED`; else `last_episode_seen = 0.0`, `status = UNSEEN`.
4. `tracking_url = url` (`:39`).
5. If `fragment.startsWith("seriesId")`: `getTrackFromSeries(track, httpUrl)` (`:41-42`). Otherwise return the track as is (`:44`).
6. Any exception: WARN "Could not get item: $url", rethrow (`:47-51`).

Total requests: 1 (movie) or 2 (series).

### 5.4 Series progress: `getEpisodesUrl(url)` and `getTrackFromSeries` (OBSERVED, `JellyfinApi.kt:69-121`)

`getEpisodesUrl(url)` (`:69-84`) builds: start from `url.newBuilder()`, set `encodedPath("/")`, `fragment(null)`, `encodedQuery(null)`, then `addPathSegment("Shows")`, `addPathSegment(fragment.split(",").last())`, `addPathSegment("Episodes")`, `addQueryParameter("seasonId", url.pathSegments.last())`, `addQueryParameter("userId", url.pathSegments[1])`, `addQueryParameter("Fields", "Overview,MediaSources")`.

Result (for `https://h/Users/U/Items/S#seriesId,R`): `https://h/Shows/R/Episodes?seasonId=S&userId=U&Fields=Overview,MediaSources`. No `SortBy` or `SortOrder` is sent (D-14).

`getTrackFromSeries(track, url)` (`:86-121`):
1. `episodesUrl = getEpisodesUrl(url)`; `GET` and parse `JFItemList` → `episodes = items` (`:87-93`).
2. `totalEpisodes = episodes.last().indexNumber!!` (`:95`). Uses the **last list element**, not the maximum. An empty list throws `NoSuchElementException`. A `null` IndexNumber throws `NullPointerException`.
3. `firstUnwatched = episodes.indexOfFirst { !it.userData.played }` (`:96`).
4. Branches (`:98-120`):
   - `firstUnwatched == 0` → `total_episodes = totalEpisodes`, `last_episode_seen = 0.0`, `status = UNSEEN`. Return.
   - `firstUnwatched == -1` (all played) → `total_episodes = totalEpisodes`, `last_episode_seen = totalEpisodes.toDouble()`, `status = COMPLETED`. Return.
   - Otherwise → `lastContinuousSeen = episodes[firstUnwatched - 1].indexNumber!!`; `total_episodes = totalEpisodes`; `last_episode_seen = lastContinuousSeen.toDouble()`; `status = WATCHING`. Return.
5. The "continuous" rule uses list position, not index value. A gap in index numbers is ignored, and an out-of-order list produces a wrong value (D-14).

Total requests: 2 (item plus episodes).

### 5.5 Mark progress: `JellyfinApi.updateProgress(track)` (OBSERVED, `JellyfinApi.kt:123-159`)

1. `httpUrl = track.tracking_url.toHttpUrl()`; `fragment = httpUrl.fragment!!`.
2. `itemId`:
   - If `fragment.startsWith("movie")` (`:126-127`): `itemId = httpUrl.pathSegments.last()`. `last_episode_seen` is **not checked** (D-32).
   - Else (`:128-141`): `GET getEpisodesUrl(httpUrl)` parse `JFItemList`; `itemId = episodes.firstOrNull { it.indexNumber!!.equalsTo(track.last_episode_seen) }?.id`.
   - `equalsTo(other: Double)` = `abs(this - other) < 0.001` (`:161-163`, with `this: Long` converted).
3. If `itemId != null` (`:143`):
   - `time = DATE_FORMATTER.format(Date())` with `"yyyy-MM-dd'T'HH:mm:ss.SSS'Z'"` and `Locale.US` (`:144`, `:166`). The literal `'Z'` is written without a timezone conversion (D-10).
   - `postUrl = httpUrl.newBuilder()`: `fragment(null)`, `removePathSegment(3)`, `removePathSegment(2)`, `addPathSegment("PlayedItems")`, `addPathSegment(itemId)`, `addQueryParameter("DatePlayed", time)` (`:145-151`). For `/Users/U/Items/S`, the result path is `/Users/U/PlayedItems/{itemId}` (D-13).
   - `POST postUrl` with no body, `awaitSuccess()` (`:153-155`).
4. If `itemId == null`: nothing is posted and no error is raised (D-12).
5. Return `getTrackSearch(track.tracking_url)` (`:158`).

UPSTREAM (Jellyfin OpenAPI, read-only knowledge): `POST /Users/{userId}/PlayedItems/{itemId}` with optional `datePlayed` query. Jellyfin marks only that item as played.

Total requests: series 2 to 3 (episodes, optional POST, item plus episodes for refresh); movie 1 to 2.

### 5.6 Jellyfin tracker class (OBSERVED, `Jellyfin.kt`)

- `update(track, didWatchEpisode)` → `api.updateProgress(track)` (`:60-62`). `didWatchEpisode` is ignored (D-31).
- `bind` returns `track` (`:64-66`).
- `searchAnime` throws `Exception("Not used")` (`:68-69`).
- `refresh(track)` → `getTrackSearch(tracking_url)`, `copyPersonalFrom`, `total_episodes` (`:71-76`).
- `login`, `loginNoop` → dummy credentials (`:78-84`).
- `getAcceptedSources` (`:86`), `match` (`:88-93`): `try { api.getTrackSearch(anime.url) } catch → null`.
- `isTrackFrom` (`:95-96`): `remoteUrl == anime.url && accept(source)`.
- `migrateTrack` (`:98-105`): copy `remoteUrl` if accepted.

---

## 6. Observed defects and the Proposal for each

Each defect is observed from source. None has been run yet. Each is verified by a test in `p1-edge-cases-and-tests.md`.

| Id | Where | Observed | Proposal |
|---|---|---|---|
| D-01 | `KavitaApi.kt:158` | `cover_url` is never filled from the server (upstream `SeriesDto` has no `thumbnail_url`), so it is always `""`. A JSON `null` value would store `"null"`. | Proposal: derive the cover from the series id (verify the Kavita cover endpoint first); treat JSON null as `""` |
| D-02 | `Kavita.kt:61-72`, `Komga.kt:58-70`, `Suwayomi.kt:51-63`, `SyncChapterProgressWithTrack.kt:44`, `TrackChapter.kt:43` | The status written in `update()` goes to a copy, and the caller discards it. The status is always server-driven from `refresh`. | Remove the local branch. Keep `update` as write-only. Document that status is remote-driven. |
| D-03 | `MangaTrack.kt:37-44`, `MangaTrackSearch.kt:25-41` | `refresh` copies `score = -1.0`, dates `0`, `private = false` from the search default | Copy only `status`, `last_*_read` and `total_*` from the remote. Keep score, dates and private. |
| D-04 | `KavitaApi.kt:21-27` vs `:40-84` | `getNewToken` uses DoH client; the rest use system DNS | Use system DNS for all (see T-33a-004) |
| D-05 | `Kavita.kt:118-148` | One 401, 500, or missing source aborts `loadOAuth` for all slots, so every Kavita request fails | Treat each slot independently; log per slot; never let one slot abort the others |
| D-06 | `KavitaApi.kt:40-84` | The `IOException("Unauthorized…")` is wrapped by the generic `catch` and its message becomes `java.io.IOException: Unauthorized…` | Return a typed `KavitaAuthError::{Unauthorized, ServerError}` |
| D-07 | `Kavita.kt:118`, `KavitaInterceptor.kt:11-13` | The JWT is never refreshed, and `authentications` is never reset | On 401 from an API call, clear the slot and re-authenticate once |
| D-08 | `SuwayomiApi.kt:45-47` | Server URL and credentials are read once per instance | Read preferences per request (or on settings change) |
| D-09 | `Suwayomi.kt:99-103` | `isTrackFrom` ignores `remoteUrl` | Compare `remoteUrl == manga.url` as the other three do |
| D-10 | `JellyfinApi.kt:166`, `:144` | `DatePlayed` is formatted in local time with a literal `Z` | Format `OffsetDateTime::now_utc()` with `…Z` |
| D-11 | `JellyfinApi.kt:123-159` | Only one episode is marked played per update | Proposal: mark every unplayed episode whose index is `<= last_episode_seen` (continuous rule), each as its own POST |
| D-12 | `JellyfinApi.kt:143` | If no episode matches the target number, nothing happens and no error is shown | Return a typed `NoMatchingEpisode` error and surface it in the log |
| D-13 | `JellyfinApi.kt:145-151` | `removePathSegment(3)` and `(2)` assume the URL has no prefix path | Derive the Users/Items indices by searching for the `Users` and `Items` segments |
| D-14 | `JellyfinApi.kt:69-121` | No `SortBy`/`SortOrder` is requested. `last()`, `indexOfFirst` and `episodes[firstUnwatched-1]` depend on server order | Request `SortBy=IndexNumber&SortOrder=Ascending`, and use the max IndexNumber for the total |
| D-15 | `JellyfinInterceptor.kt:17`, `:33` | `apiKeys` is an unsynchronised `MutableMap` | Use `Mutex` or `DashMap` |
| D-16 | `JellyfinInterceptor.kt:33`, `:62` | An empty `api_key` is cached | Treat empty as not configured and do not cache |
| D-17 | `JellyfinInterceptor.kt:54` | Missing source slot → `NullPointerException` | Skip missing slots |
| D-18 | `KomgaApi.kt:59-63` | A stored tracking URL in v2 form (`/api/v2/series/`) is parsed as v1 progress and fails | Accept both forms; parse v2 when the URL is v2 |
| D-19 | `Komga.kt:28-31`, `KomgaApi.kt:28-32` | No Authorization header; authentication depends on a shared cookie jar | Open decision. See `p1-rust-design.md` section 10 (blocking). |
| D-20 | `KomgaApi.kt:71-75` | Empty series (`0,0,0`) is UNREAD by branch order | Keep behaviour. Document it. |
| D-21 | `Kavita.kt:78-80`, `Suwayomi.kt:69-71` | `TODO()` throws `NotImplementedError` (an `Error`) | Return `Err(TrackError::SearchUnsupported)` |
| D-22 | `KavitaApi.kt:126-148` | `"-1"` is parsed to `-1.0` as the latest chapter | Clamp to `>= 0.0` |
| D-23 | `KavitaApi.kt:101-124` | Empty `chapters` throws `NoSuchElementException`; non-numeric `number` throws `NumberFormatException` | Skip empty volumes; parse with error mapping per volume |
| D-24 | `KavitaApi.kt:162-166` | `pages == 0 && pagesRead == 0` is COMPLETED | Compute `pagesRead == 0` first, then `pagesRead >= pages` |
| D-25 | `SuwayomiApi.kt:87` | `first {}` throws when the number does not exist | Return `Err(TrackError::ChapterNotOnServer)` |
| D-26 | `SuwayomiApi.kt:49-56` | Numeric id with empty base URL produces a relative URL | Reject with "Server URL not set" |
| D-27 | `Suwayomi.kt:24` | Logo colour has a `// TODO` | Use the logo's own colour (sample from asset) |
| D-28 | `Jellyfin.kt:86`, `JellyfinApi.kt:126` | Missing fragment → `NullPointerException` | Parse the fragment as an optional field and error if absent |
| D-29 | `Kavita.kt:101-106`, `Komga.kt:99-104`, `Suwayomi.kt:92-97`, `Jellyfin.kt:88-93` | `match` swallows every error, which hides auth failures from the bind flow | Return `Result`, keep the caller's log and toast |
| D-30 | `KavitaApi.kt:86-96` | `getIdFromUrl` uses `substringAfterLast("/").toInt()`; query strings or trailing slashes break it | Parse the last non-empty path segment |
| D-31 | `Jellyfin.kt:60-62` | `didWatchEpisode` ignored | Keep. Document. |
| D-32 | `JellyfinApi.kt:126-127` | A movie is marked played on any update, even `last_episode_seen = 0` | Mark played only when `last_episode_seen >= 1.0`; mark unplayed otherwise |

---

## 7. Message keys and user-visible text

| Key or text | Source | When shown | Port |
|---|---|---|---|
| `MR.strings.unread` ("Unread") | `Kavita.kt:45`, `Komga.kt:42`, `Suwayomi.kt:35` | Status picker | Same key |
| `MR.strings.reading` ("Reading") | same | Status picker | Same key |
| `MR.strings.completed` ("Completed") | `Kavita.kt:46`, `Komga.kt:44`, `Suwayomi.kt:37`, `Jellyfin.kt:46` | Status picker | Same key |
| `AYMR.strings.unseen` ("Unseen") | `Jellyfin.kt:44` | Status picker | Same key |
| `AYMR.strings.watching` ("Watching") | `Jellyfin.kt:45` | Status picker | Same key |
| `MR.strings.enhanced_tracking_info` ("Provides enhanced features for specific sources. Entries are automatically tracked when added to your library.") | `SettingsTrackingScreen.kt:129` | Settings, Tracking | Same key |
| `MR.strings.enhanced_services_not_installed` ("Available but source not installed: %s") | `SettingsTrackingScreen.kt:131-134` | Settings, Tracking | Same key |
| `MR.strings.enhanced_services` ("Enhanced trackers") | `SettingsTrackingScreen.kt:225` | Settings group title | Same key |
| `"Not used"` | `Komga.kt:77`, `Jellyfin.kt:69` | Never shown (only if the search UI is reached for these trackers) | Return `SearchUnsupported` |
| `"Please log in through the extension"` | `JellyfinInterceptor.kt:34` | Literal English. Shown as toast text through `MangaTracker.updateRemote` (`MangaTracker.kt:119`) | Use a message key `track.enhanced_login_required` (new) |
| `"Unauthorized / api key not valid"`, `"Error fetching JWT token"` | `KavitaApi.kt:55`, `:64` | Only in logs (wrapped, see D-06) | Typed errors |

Log-only strings are English literals in `logcat`. The port uses `tracing` with the same wording.

---

## 8. Timings, limits and networking defaults

OBSERVED from `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:20-31`:
- `connectTimeout` 30 s, `readTimeout` 30 s, `callTimeout` 2 min.
- OkHttp `Cache` at `cacheDir/network_cache`, max 5 MiB. Tracker clients inherit it through `newBuilder()`. GET responses may be served from the HTTP cache (risk recorded in `p1-topic-enhanced-tracker-framework.md` section 4). The Rust port must not add an HTTP cache.
- `UncaughtExceptionInterceptor` and `UserAgentInterceptor` are in the base chain (`:32-33`).
- Cloudflare interceptor is in the base chain (`:61-63`).

Per-tracker constants:
- Kavita: 3 slots (`Kavita.kt:118`, `KavitaModels.kt:65-70`).
- Jellyfin: 10 slots (`JellyfinInterceptor.kt:71`).
- Komga: none.
- Suwayomi: 1 source (`SuwayomiApi.kt:103`).
- No retry logic exists in any of the four trackers. The retry queue is in area:27-tracking-core (`TrackChapter.kt:46-50`).
- Connectivity: none checked inside the trackers.

Request counts per user action (OBSERVED from the call graph above):

| Action | Kavita | Komga | Suwayomi | Jellyfin (season) | Jellyfin (movie) |
|---|---|---|---|---|---|
| Bind (`match`) | 3 | 2 | 1 | 2 | 1 |
| Refresh | 3 | 2 | 1 | 2 | 1 |
| Update (`update`) | 4 | 3 | 3 | 2 to 3 | 2 |
| Per-chapter auto-track (`TrackChapter`: refresh + update) | 7 | 5 | 4 | 4 to 5 | 3 |

Suwayomi "update" = chapters GET + PUT + item GET = 3. Suwayomi "per-chapter" = refresh 1 + update 3 = 4.

---

## 9. Concurrency and dispatchers (OBSERVED)

- Interactors wrap work in `withIOContext` and `withNonCancellableContext` (`AddMangaTracks.kt:81-82`, `TrackChapter.kt:25`). The tracker APIs themselves are `suspend` and use `withIOContext` (`KavitaApi.kt:150`, `KomgaApi.kt:37`, `SuwayomiApi.kt:49`, `JellyfinApi.kt:31` wraps `getTrackSearch` only; `updateProgress` has no wrapper and runs on the caller's dispatcher).
- `KavitaApi.getNewToken` and the Kavita interceptor use blocking `execute()` (`KavitaApi.kt:41-44`, `:127`, `:103`). They run on whichever dispatcher the caller uses. This blocks a thread from the IO pool.
- `Kavita.authentications` is a `var` with no volatile or lock (`Kavita.kt:31`). Two concurrent first requests can run `loadOAuth` twice. The result is the same, so the race is benign but wasteful.
- `JellyfinInterceptor.apiKeys` is a `mutableMapOf` written from any thread (D-15).
- `TrackChapter` launches one `async` per tracker (`TrackChapter.kt:37`) inside `withNonCancellableContext`.

Rust mapping in `p1-rust-design.md` section 5.

---

## 10. Interactions with other areas

- area:27-tracking-core owns the base `Tracker` contract, persistence (`insertTrack`), the retry queue and the settings screen. This area supplies the four concrete trackers only.
- area:12-network-stack owns the shared client, DoH and the user-agent provider. This area overrides DNS to system resolution.
- area:15-source-api-manga and area:16-source-api-anime own the source-id algorithm (`p1-data-and-config.md` section 4). This area uses it for the Kavita, Jellyfin and Suwayomi lookups.
- area:17-source-app-layer owns the source registry (`sourceManager.get`). The Rust equivalent must return "not installed" instead of `null`.
- area:20-extension-strategy-redesign owns the extension preference keys. The Kavita `APIURL`/`APIKEY`, the Jellyfin `user_id`/`api_key` and the Suwayomi keys are written by external extensions. The port reads them with `kuro-prefs` (area 02) and the per-source preference store.
- area:02-preferences-system owns the `source_<id>` preference file format (`p1-data-and-config.md` section 3).
- area:05-data-repos-entries-items owns the `anime_sync` and `manga_sync` tables.
- area:06-data-repos-history-track-source-misc owns the `track` repository wiring.
- area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin is part 1 of 1. No other area references its files.

---

## 11. Acceptance behaviours for the port (summary)

1. Matching a Kavita series with `match` returns a track with `total_chapters` from the volumes walk (2.6), `last_chapter_read` from latest-chapter (2.7), and `status` from pages (2.5 step 7).
2. A Komga v2 series with `booksCount = 10, booksUnreadCount = 10` gives status `UNREAD`; with `booksReadCount = 10` gives `COMPLETED`; otherwise `READING` (3.3 step 3).
3. Suwayomi `updateProgress` for `last_chapter_read = 5.0` sends `PUT …/chapter/{index of chapterNumber 5.0}` with `markPrevRead=true&read=true` (4.5).
4. Jellyfin series progress for episodes with played flags `[T,T,F,F]` gives `last_episode_seen = episodes[1].indexNumber`, status `WATCHING` (5.4). For `[F,F]` gives `0.0` and `UNSEEN`. For `[T,T]` gives `totalEpisodes` and `COMPLETED`.
5. Jellyfin `updateProgress` on a season URL with fragment `seriesId,R` posts to `/Users/U/PlayedItems/{episodeId}?DatePlayed=…Z` where the date is UTC (5.5, with D-10 fixed).
6. Kavita 401 during `loadOAuth` on slot 1 does not stop slots 2 and 3 (D-05 fixed).
7. `refresh` on any of the four trackers keeps local score, dates and private flag (D-03 fixed).
