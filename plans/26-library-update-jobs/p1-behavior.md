# Area 26 - Library update jobs: behaviour specification

Source root: `/home/blackbox/aniyomi-src`. Labels: **OBSERVED** = what the Kotlin does (with file:line). **Proposal:** = what the Rust port should do. Anything marked *external, not read* is outside this area and the behaviour is only referenced.

Terminology: "manga pass" = `MangaLibraryUpdateJob`; "anime pass" = `AnimeLibraryUpdateJob`; "entry" = a manga or an anime (anime includes season children).

---

## 0. Dependency map (what the jobs call)

| Step | Callee (repo-relative) | Notes |
|---|---|---|
| read library | `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetLibraryManga.kt` -> `MangaRepository.getLibraryManga()` -> SQL view `data/src/main/sqldelight/view/libraryView.sq` | One row per (favourite manga, category) pair, see section 3.1 |
| read library (anime) | `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetLibraryAnime.kt` -> `data/src/main/sqldelightanime/view/animelibView.sq` | Same shape, `fetch_type` dependent counts |
| season children | `domain/src/main/java/tachiyomi/domain/items/season/interactor/GetAnimeSeasonsByParentId.kt` -> `data/src/main/sqldelightanime/view/animeseasonsView.sq` (`getAnimeSeasonsById`) | Returns `SeasonAnime` list; errors -> empty list + logcat |
| fetch window and next update | `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt`, `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt` | Full algorithm section 4 |
| source lookup | `MangaSourceManager.getOrStub`, `AnimeSourceManager.getOrStub` (`eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt:89`, `eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:89`) | external, not read; `getOrStub` returns a stub that throws `SourceNotInstalledException` / `AnimeSourceNotInstalledException` on use |
| manga: fetch details | `source.getMangaDetails(manga.toSManga())` | only when `autoUpdateMetadata` |
| manga: fetch chapters | `source.getChapterList(manga.toSManga())` | external (source API) |
| manga: sync | `eu/kanade/domain/entries/manga/interactor/UpdateManga.kt` `awaitUpdateFromSource` (metadata), `eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt` `await` (chapters) | section 5 |
| anime: fetch + sync | `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt` `awaitEpisodesUpdate(...)` | section 5.2 |
| anime sync | `app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt` `await` | section 5.2 |
| download filter | `domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt`, `domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt` | section 6.3 |
| download enqueue | `MangaDownloadManager.downloadChapters(manga, chapters, autoStart)` (`eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:141`), `MangaDownloadManager.startDownloads()` (line 77); anime equivalents at `eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt:139` and `:75` | external |
| counters | `LibraryPreferences.newMangaUpdatesCount()` / `newAnimeUpdatesCount()` | section 6.4 |
| settings | `LibraryPreferences` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt`) | data doc |

---

## 1. Scheduling

### 1.1 Scheduled (auto) work

**OBSERVED** `MangaLibraryUpdateJob.setupTask` (`eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:439-486`), identical structure in `AnimeLibraryUpdateJob.setupTask` (`eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt:452-499`).

Algorithm:
1. `preferences = Injekt.get<LibraryPreferences>()`; `interval = prefInterval ?: preferences.autoUpdateInterval().get()` (pref key `pref_library_update_interval_key`, default `0`).
2. If `interval > 0`:
   a. `restrictions = preferences.autoUpdateDeviceRestrictions().get()` (key `library_update_restriction`, default `{"wifi"}`).
   b. `networkType = UNMETERED` if `"network_not_metered"` in restrictions, else `CONNECTED`.
   c. `NetworkRequest.Builder()`; if `"wifi"` in restrictions add `TRANSPORT_WIFI`; if `"network_not_metered"` in restrictions add `NET_CAPABILITY_NOT_METERED`.
   d. `Constraints.Builder().setRequiredNetworkRequest(networkRequest, networkType)` (Android 9+ uses the NetworkRequest; older uses `networkType`), `.setRequiresCharging("ac" in restrictions)`, `.setRequiresBatteryNotLow(true)` (always).
   e. `PeriodicWorkRequestBuilder<Job>(interval hours, flex 10 minutes)`; tags: `TAG` (`LibraryUpdate` manga / `AnimeLibraryUpdate` anime) and `WORK_NAME_AUTO` (`LibraryUpdate-auto` / `AnimeLibraryUpdate-auto`).
   f. `setBackoffCriteria(BackoffPolicy.LINEAR, 10, MINUTES)`.
   g. `enqueueUniquePeriodicWork(WORK_NAME_AUTO, ExistingPeriodicWorkPolicy.UPDATE, request)`.
3. Else (`interval <= 0`, i.e. "Off"): `cancelUniqueWork(WORK_NAME_AUTO)`.

Notes:
- The `prefInterval` argument is used when the settings screen passes the newly selected value (`SettingsLibraryScreen.kt:237-238`) because the pref is not yet written. Other callers (`SettingsLibraryScreen.kt:255-256`, `PreferenceRestorer.kt:45-46`, `SetupBackgroundTasksMigration.kt`, `SetupMangaLibraryUpdateMigration.kt`, `SetupAnimeLibraryUpdateMigration.kt`, `UseWorkManagerMigration.kt`) pass nothing.
- `RemoveOneTwoHourUpdateMigration.kt:21-22` calls `setupTask(context, 3)` and `RemoveQuickUpdateMigration.kt:21-22` calls `setupTask(context, 12)`: migrations forcing the old values to valid choices.
- Interval choices offered in UI (`SettingsLibraryScreen.kt`, `ListPreference` entries): `0 -> "Off"`, `12 -> "Every 12 hours"`, `24 -> "Daily"`, `48 -> "Every 2 days"`, `72 -> "Every 3 days"`, `168 -> "Weekly"` (strings `update_never`, `update_12hour`, `update_24hour`, `update_48hour`, `update_72hour`, `update_weekly`). Any positive integer is accepted by the job; the UI only offers these six.
- Restrictions UI (`MultiSelectListPreference`, key `library_update_restriction`): `wifi` -> "Only on Wi-Fi" (`connected_to_wifi`), `network_not_metered` -> "Only on unmetered network" (`network_not_metered`), `ac` -> "When charging" (`charging`). Changing it re-runs `setupTask(context)` for both jobs on the main executor. The item is `enabled = autoUpdateInterval > 0`.
- Restrictions summary string uses `restrictions` with format `"Restrictions: %s"`.

### 1.2 Manual (startNow) work

**OBSERVED** `startNow(context, category)` (manga `MangaLibraryUpdateJob.kt:488-509`, anime `AnimeLibraryUpdateJob.kt:500-521`):
1. `wm = context.workManager`. If `wm.isRunning(TAG)` (any RUNNING work with tag `TAG`) return `false`.
2. `inputData = workDataOf(KEY_CATEGORY to category?.id)`. Manga key `"category"`, anime key `"animeCategory"`. A null category stores a null Long; `inputData.getLong(KEY, -1L)` then yields `-1`.
3. `OneTimeWorkRequestBuilder<Job>()` with tags `TAG` and `WORK_NAME_MANUAL`, data as above.
4. `enqueueUniqueWork(WORK_NAME_MANUAL, ExistingWorkPolicy.KEEP, request)`. Return `true`.

Callers and user-visible results:
- Library tab "refresh" button per category: `MangaLibraryTab.kt:105-109` and `AnimeLibraryTab.kt:106`: `started` true -> snackbar `updating_category` ("Updating category"), false -> `update_already_running` ("An update is already running").
- Updates tab refresh: `MangaUpdatesScreenModel.kt:129` and `AnimeUpdatesScreenModel.kt:133` call `startNow(app)` with no category; result is sent as `Event.LibraryUpdateTriggered(started)`.
- Settings "Refresh library covers": `SettingsAdvancedScreen.kt:321-324` calls `AnimeLibraryUpdateJob.startNow`, `MangaLibraryUpdateJob.startNow`, `AnimeMetadataUpdateJob.startNow`, `MangaMetadataUpdateJob.startNow` in that order.

**Proposal:** Rust keeps the same `startNow(category: Option<CategoryId>) -> bool` contract: returns `false` when a pass for that kind is already running.

### 1.3 Stop

**OBSERVED** `stop(context)` (manga `MangaLibraryUpdateJob.kt:511-526`, anime `AnimeLibraryUpdateJob.kt:523-538`):
1. Query RUNNING works with tag `TAG`; for each: `cancelWorkById(id)`.
2. If that work had tag `WORK_NAME_AUTO` (scheduled run), call `setupTask(context)` to re-enqueue the periodic schedule.
3. Metadata jobs' `stop` only cancels (no re-enqueue) (`MangaMetadataUpdateJob.kt:195-205`, `AnimeMetadataUpdateJob.kt:187-197`).

Triggered by the progress notification "Cancel" action: `NotificationReceiver.kt:270-281` (`ACTION_CANCEL_LIBRARY_UPDATE` -> `MangaLibraryUpdateJob.stop`, `ACTION_CANCEL_ANIMELIB_UPDATE` -> `AnimeLibraryUpdateJob.stop`).

### 1.4 cancelAllWorks

`cancelAllWorks(context)` = `cancelAllWorkByTag(TAG)` (`MangaLibraryUpdateJob.kt:435-437`). Only called by `UseWorkManagerMigration.kt:16-17`.

### 1.5 Worker result semantics

`doWork()` returns:
- `Result.success()` on normal completion, and on `CancellationException` ("Assume success although cancelled").
- `Result.failure()` on any other `Exception` (logged via logcat). Note only `Exception` is caught in the outer try; errors inside entries are caught per entry (section 6.2).
- `Result.retry()` in two pre-checks (section 2).

### 1.6 Foreground

`getForegroundInfo()` builds `ForegroundInfo(Notifications.ID_LIBRARY_PROGRESS /* -101 */, progressNotificationBuilder.build(), type)`, `type = FOREGROUND_SERVICE_TYPE_DATA_SYNC` on API >= 29 (`Build.VERSION_CODES.Q`), else `0`. `setForeground` is attempted; an `IllegalStateException` is logged with `"Not allowed to set foreground job"` and ignored. The metadata jobs do the same but do not do the pre-checks in section 2.

---

## 2. doWork() pre-checks and entry flow (manga pass)

**OBSERVED** `MangaLibraryUpdateJob.doWork()` (`MangaLibraryUpdateJob.kt:94-137`), in order:

1. If `tags.contains("LibraryUpdate-auto")` (a scheduled run):
   a. If `Build.VERSION.SDK_INT < 28` (P) and `"wifi"` in `autoUpdateDeviceRestrictions` and `!context.isConnectedToWifi()` -> `Result.retry()`. (On API 28+ the NetworkRequest constraint handles it.)
   b. If `workManager.isRunning("LibraryUpdate-manual")` (a manual pass is RUNNING) -> `Result.retry()`.
2. `setForeground(getForegroundInfo())` (see 1.6).
3. `libraryPreferences.lastUpdatedTimestamp().set(Instant.now().toEpochMilli())` (app-state key `library_update_last_timestamp`). This happens even when there is nothing to update.
4. `categoryId = inputData.getLong("category", -1L)`; `addMangaToQueue(categoryId)` (section 3).
5. `withIOContext { updateChapterList(); success }`; `finally notifier.cancelProgressNotification()`.

**Anime pass difference (OBSERVED):** `AnimeLibraryUpdateJob.doWork()` (`AnimeLibraryUpdateJob.kt:93-131`) has the same steps but step 1b (manual-running retry) is **absent**. Scheduled anime passes can therefore run concurrently with a manual anime pass. The manga pass has no equivalent protection for the anime pass or vice versa. **Proposal:** apply step 1b to both kinds and make the check global (see pitfall P-5).

Anime `addAnimeToQueue` is called with `categoryId` in the same way (`AnimeLibraryUpdateJob.kt:112-113`).

---

## 3. Queue construction (addMangaToQueue / addAnimeToQueue)

### 3.1 Library input

`getLibraryManga.await()` returns `LibraryManga` rows from `libraryView.sq`:
- Base set: `FROM mangas M ... WHERE M.favorite = 1` (only favourites).
- `LEFT JOIN mangas_categories AS MC ON MC.manga_id = M._id`, `category = coalesce(MC.category_id, 0)`. A manga in N categories yields N rows; a manga in no category yields one row with `category = 0`.
- `totalCount = coalesce(C.total, 0)` where `C` counts chapters excluding `excluded_scanlators` rows; `readCount = coalesce(sum(read), ...)`. `unreadCount = totalChapters - readCount`, `hasStarted = readCount > 0` (`LibraryManga.kt:17-23`).

Anime equivalent (`animelibView.sq`): `WHERE M.favorite = 1`; `category = coalesce(MC.category_id, 0)` from `animes_categories`; counts depend on `fetch_type` (`1 = Episodes`, `0 = Seasons`):
- `fetch_type = 1`: `totalCount = ES.total`, `seenCount = ES.seenCount` (episode stats view).
- `fetch_type = 0`: `totalCount = ASS.child_count`, `seenCount = ASS.fully_seen_seasons`.
- `unseenCount = totalCount - seenCount`, `hasStarted = seenCount > 0` (`LibraryAnime.kt:18-24`).

### 3.2 Category selection (OBSERVED, manga lines 157-180; anime lines 152-175)

```
if categoryId != -1:
    list = library.filter { it.category == categoryId }            // one category, ignores update-categories prefs
else:
    include = prefs.mangaUpdateCategories  (key "library_update_categories", default {})
    included = if include.isNotEmpty: library.filter { it.category in include } else: library
    exclude = prefs.mangaUpdateCategoriesExclude (key "library_update_categories_exclude", default {})
    excludedIds = if exclude.isNotEmpty: library.filter { it.category in exclude }.map { it.manga.id } else: []
    list = included.filterNot { it.manga.id in excludedIds }.distinctBy { it.manga.id }
```
- Anime keys: `animelib_update_categories`, `animelib_update_categories_exclude`.
- Note: the exclude list removes an entry if it is in ANY excluded category even if it is also in an included category (exclusion wins). `distinctBy` keeps the first row per manga id, which removes duplicates produced by multi-category membership.
- Category id `0` means "no category" (the default category row). The UI passes ids as strings and the job converts with `toLong()`.

### 3.3 Item restrictions

Applied to `list` (`MangaLibraryUpdateJob.kt:182-225`, anime `AnimeLibraryUpdateJob.kt:196-239`). `restrictions = libraryPreferences.autoUpdateItemRestrictions().get()` (key `library_update_manga_restriction`, **default `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}`**; the UI shows these as "skip" options). The **manual pass also applies these restrictions**: the filter does not check `categoryId`.

Checks, evaluated in order, first match skips the entry and records a reason (the reason list is only written to logcat):

| # | Condition (exact) | Skip reason message key (MR) | English text |
|---|---|---|---|
| 1 | `updateStrategy != ALWAYS_UPDATE` (manga: `UpdateStrategy`, anime: `AnimeUpdateStrategy`) | `skipped_reason_not_always_update` | "Skipped because series does not require updates" |
| 2 | `"manga_ongoing" in restrictions && status == SManga.COMPLETED (2)` (anime uses `SAnime.COMPLETED = 2`) | `skipped_reason_completed` | "Skipped because series is complete" |
| 3 | `"manga_fully_read" in restrictions && unreadCount != 0` (anime: `"manga_fully_read"`, `unseenCount != 0`) | `skipped_reason_not_caught_up` | "Skipped because there are unread chapters" |
| 4 | `"manga_started" in restrictions && totalChapters > 0 && !hasStarted` (anime: `totalCount > 0 && !hasStarted`) | `skipped_reason_not_started` | "Skipped because no chapters are read" |
| 5 | `"manga_outside_release_period" in restrictions && nextUpdate > fetchWindowUpperBound` | `skipped_reason_not_in_release_period` | "Skipped because no release was expected today" |
| 6 | otherwise keep | - | - |

Notes:
- Check 2 uses the **status integer** `SManga.COMPLETED == 2`. Other extensions' status values are not special-cased.
- Check 3 is `!= 0`, not `> 0`: a manga with `totalChapters = 0` and `readCount = 0` gives `unreadCount = 0` and passes check 3.
- The `fetchWindowUpperBound` is `mangaFetchInterval.getWindow(ZonedDateTime.now()).second` (section 4.2), which is `tomorrow 00:00 local - 1 ms` in epoch ms. An entry whose `nextUpdate` is later than that is skipped.
- Check 5 for seasons uses the season's own `nextUpdate`.
- After filtering: `sortedBy { it.manga.title }` (anime `it.anime.title`). Kotlin `sortedBy` on `String` uses natural `compareTo` (UTF-16 code unit order, case-sensitive).
- The ordering of check 1 before check 2 means an entry that is both non-ALWAYS_UPDATE and completed is logged with the strategy reason.

Then `notifier.showQueueSizeWarningNotificationIfNeeded(mangaToUpdate)` (section 8.2). Then the skipped-reason log is produced (`logcat { ... groupBy reason ... }`) with a `// TODO: surface skipped reasons to user?` comment. **No user-visible surfacing exists.**

### 3.4 Season expansion (anime only, OBSERVED `AnimeLibraryUpdateJob.kt:177-194`)

`includeSeasons = prefs.updateSeasonOnLibraryUpdate()` (key `pref_update_season_on_library_update`, default `false`).

For each library entry `libAnime` (after category selection, before restrictions), `flatMap`:
- `FetchType.Seasons`:
  - if `includeSeasons`: `getAnimeSeasonsByParentId.await(libAnime.anime.id)` -> keep only seasons `s` where `s.anime.fetchType == FetchType.Episodes && !s.anime.favorite`; map each to `toLibraryAnime()` (which sets `category = -1L`, `episodeFetchedAt = fetchedAt`).
  - else: empty list. **The parent (Seasons-type) entry itself is never queued by the anime pass.**
- `FetchType.Episodes`: `listOf(libAnime)`.

Consequences (OBSERVED):
- Season children are queued individually; children of type `Seasons` (nested) are ignored.
- Children that are favourited themselves are excluded (they are library entries already handled).
- The children use their own `updateStrategy`, `status`, `nextUpdate`, counts for the restriction checks.
- In the update step, children (`parentId != null`) skip the favourite check (section 6.2).

**Manga has no season concept.**

---

## 4. Fetch interval algorithm (MangaFetchInterval / AnimeFetchInterval)

Both classes are identical except types (`Chapter`/`Episode`, `dateUpload`/`dateFetch`, `MAX_INTERVAL` duplicated). Constants: `MAX_INTERVAL = 28`, `GRACE_PERIOD = 1L` (days). Source: `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt:13-130`, `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt:13-130`.

### 4.1 toMangaUpdate(manga, dateTime, window) -> (nextUpdate, fetchInterval)

```
interval = if manga.fetchInterval < 0 then manga.fetchInterval
           else calculateInterval(chapters = getChaptersByMangaId(manga.id, applyScanlatorFilter=true), zone = dateTime.zone)
currentWindow = if window == (0,0) then getWindow(ZonedDateTime.now()) else window
nextUpdate = calculateNextUpdate(manga, interval, dateTime, currentWindow)
return MangaUpdate(id, nextUpdate, fetchInterval = interval)
```
- A **negative stored `fetchInterval` is a manual override** meaning "every abs(value) days, fixed". `0` or positive means "recalculate".
- Anime uses `getEpisodesByAnimeId.await(anime.id)` (no scanlator filter argument).

### 4.2 getWindow(dateTime)

```
today = dateTime.toLocalDate().atStartOfDay(dateTime.zone)
lower = today.minusDays(1)
upper = today.plusDays(1)
return (lower.toEpochSecond() * 1000, upper.toEpochSecond() * 1000 - 1)
```
So the window is yesterday 00:00 through tomorrow 00:00 minus 1 ms, in the device zone (note: `toEpochSecond()*1000` is ms at second precision).

### 4.3 calculateInterval(chapters, zone) -> Int in 1..28

```
chapterWindow = if chapters.size <= 8 then 3 else 10
uploadDates = chapters
    .filter { dateUpload > 0 }
    .sortedByDescending { dateUpload }
    .map { startOfDay(local date of dateUpload in zone) }
    .distinct()
    .take(chapterWindow)
fetchDates = chapters
    .sortedByDescending { dateFetch }                  // no > 0 filter
    .map { startOfDay(local date of dateFetch in zone) }
    .distinct()
    .take(chapterWindow)

interval =
  if uploadDates.size >= 3:
      delta  = uploadDates.last().until(uploadDates.first(), DAYS)   // oldest -> newest, in days
      period = uploadDates.indexOf(uploadDates.last())               // == uploadDates.size - 1
      delta.floorDiv(period).toInt()
  else if fetchDates.size >= 3:
      delta  = fetchDates.last().until(fetchDates.first(), DAYS)
      period = fetchDates.indexOf(fetchDates.last())                 // == fetchDates.size - 1
      delta.floorDiv(period).toInt()
  else 7

return interval.coerceIn(1, 28)
```
Key facts: the list is newest-first, so `first()` is newest and `last()` is the oldest kept date. `period` is the number of gaps between `size` distinct dates (size - 1). With exactly 3 dates the divisor is 2.

Worked example (for the test suite): upload dates (local day ordinal) 2026-01-01, 2026-01-08, 2026-01-15, 2026-01-22 with 4 chapters -> window 3, distinct and take 3 newest: 2026-01-22, 2026-01-15, 2026-01-08. delta = 2026-01-08 until 2026-01-22 = 14 days, period = 2, interval = 7.

### 4.4 calculateNextUpdate(entry, interval, dateTime, window) -> epoch ms

```
if entry.nextUpdate in [window.first, window.second + 1]:      // inclusive range via rangeTo
    return entry.nextUpdate                                    // already scheduled inside the window: keep

latestDate = startOfDay( local date of ( entry.lastUpdate > 0 ? lastUpdate : Instant.now() ) in dateTime.zone )
timeSinceLatest = ChronoUnit.DAYS.between(latestDate, dateTime)   // whole days, Int
cycle = timeSinceLatest.floorDiv(
            if interval < 0 then abs(interval)
            else increaseInterval(interval, timeSinceLatest, increaseWhenOver = 10))
return (latestDate + (cycle + 1) * abs(interval) days, at dateTime.offset) * 1000
```

`increaseInterval(delta, t, increaseWhenOver)`:
```
if delta >= 28 return 28
cycle = t.floorDiv(delta) + 1
if cycle > increaseWhenOver (10) then increaseInterval(delta * 2, t, 10) else delta
```
So the effective interval doubles (1 -> 2 -> 4 -> 8 -> 16 -> 28 cap) while the entry has been "missed" for more than 10 cycles. It never exceeds 28 and never goes below the calculated interval.

Note that `entry.lastUpdate` is the **last time the chapter/episode list changed** (`awaitUpdateLastUpdate`), so a long-quiet series gets an increasing interval.

### 4.5 When the interval is recalculated

- In `SyncChaptersWithSource.await` (`SyncChaptersWithSource.kt:146-155`): when there is nothing to add, remove or update, `updateManga.awaitUpdateFetchInterval(manga, now, fetchWindow)` runs only if `manualFetch || manga.fetchInterval == 0 || manga.nextUpdate < fetchWindow.first`.
- Otherwise (changes present) it always runs at `SyncChaptersWithSource.kt:222`, followed by `awaitUpdateLastUpdate(manga.id)` (`:226`).
- Anime equivalents: `SyncEpisodesWithSource.kt:158-166`, `:234-238`.

Important: `manga.nextUpdate < fetchWindow.first` means "overdue" (before yesterday 00:00). If the job runs with the window computed at queue time (`updateChapterList` computes `fetchWindow` once at pass start, `MangaLibraryUpdateJob.kt:256`), entries get the same window for the whole pass.

---

## 5. Source fetch and sync

### 5.1 Manga entry (`updateManga(manga, fetchWindow)`, `MangaLibraryUpdateJob.kt:344-360`)

1. `source = sourceManager.getOrStub(manga.source)`.
2. If `libraryPreferences.autoUpdateMetadata()` (key `auto_update_metadata`, default `false`): `networkManga = source.getMangaDetails(manga.toSManga())`; `updateManga.awaitUpdateFromSource(manga, networkManga, manualFetch = false, coverCache)`.
3. `chapters = source.getChapterList(manga.toSManga())` (network, may throw).
4. `dbManga = getManga.await(manga.id)?.takeIf { it.favorite } ?: return emptyList()` (re-read after network; if removed from library during the pass, nothing is synced).
5. `syncChaptersWithSource.await(chapters, dbManga, source, manualFetch = false, fetchWindow)`.

### 5.2 Anime entry (`updateAnime(anime, fetchWindow)`, `AnimeLibraryUpdateJob.kt:359-372`)

1. `source = sourceManager.getOrStub(anime.source)`.
2. `updateAnimeFromRemote.awaitEpisodesUpdate(source, anime, fetchDetails = autoUpdateMetadata, fetchEpisodes = true, fetchWindow)` then `getOrThrow()`.
   - Inside (`UpdateAnimeFromRemote.kt:54-87`): `episodes = episodeRepository.getEpisodeByAnimeId(id).sortedBy { sourceOrder }`; `source.getAnimeEpisodeUpdate(anime.toSAnime(), episodes.map(toSEpisode), fetchDetails, fetchEpisodes)` (external, not read); `awaitUpdateFromSource(anime, update.anime, manualFetch=false)`; `newEpisodes = syncEpisodesWithSource.await(update.episodes, anime, source, manualFetch=false, fetchWindow)`; returns `RemoteAnimeEpisodeUpdate(anime = animeRepository.getAnimeById(id), newEpisodes)`; exceptions wrapped in `Result.failure(e)` (logged).
3. `return if (update.anime.favorite) update.newEpisodes else emptyList()`.

Note: anime does not re-check `favorite` before the sync (the sync runs even if the entry was un-favourited mid-pass); only the return value is gated. Manga gates before sync. **Pitfall P-7.**

### 5.3 Metadata update inside anime sync (`UpdateAnimeFromRemote.awaitUpdateFromSource`, `UpdateAnimeFromRemote.kt:142-214`)

- `title`: taken from remote only when `remoteTitle` is non-empty AND `!localAnime.favorite`; else `null` (no change). Remote title is read through `try { remoteAnime.title } catch (UninitializedPropertyAccessException) { "" }`.
- `coverLastModified`:
  - `null` if `thumbnail_url` null/empty (never clear covers);
  - `null` if `!manualFetch && localAnime.thumbnailUrl == remote.thumbnail_url`;
  - `now` if local is local (`isLocal()`);
  - if `hasCustomCover(coverCache)`: delete cache, `null`;
  - else delete cache and `now`.
- `backgroundLastModified`: same logic with `background_url`, `backgroundCache`.
- `thumbnailUrl = remote.thumbnail_url?.takeIf(notEmpty)`; `backgroundUrl` likewise.
- Writes `AnimeUpdate(id, title, coverLastModified, backgroundLastModified, author, artist, description, genre = getGenres(), thumbnailUrl, backgroundUrl, status = status.toLong(), updateStrategy = update_strategy, initialized = true, memo = memo)`.

The manga equivalent (`UpdateManga.awaitUpdateFromSource`, `UpdateManga.kt:29-77`) is the same minus background and memo; title is `null` when remote empty or local favourite. Note the manga version does `if (remoteTitle.isEmpty() || localManga.favorite) null else remoteTitle`.

### 5.4 Chapter sync (`SyncChaptersWithSource.await`, `SyncChaptersWithSource.kt:49-233`)

Full ordered algorithm (OBSERVED):

1. If `rawSourceChapters` is empty and source is not local -> throw `NoChaptersException` (message key `no_chapters_error`).
2. `now = ZonedDateTime.now()`, `nowMillis`.
3. `sourceChapters = raw.distinctBy { url }.mapIndexed { i, s -> Chapter.create().copyFromSChapter(s).copy(name = ChapterSanitizer.sanitize(s.name, manga.title), mangaId, sourceOrder = i) }`. `sourceOrder` 0 is the first item returned by the source (sources return newest first).
4. `dbChapters = getChaptersByMangaId.await(manga.id)`.
5. `removedChapters = dbChapters.filterNot { db.url in sourceUrls }`.
6. `maxSeenUploadDate = 0`.
7. For each `sourceChapter` in order:
   - if `source is HttpSource`: `source.prepareNewChapter(sChapter, manga)` (external hook) and copy back.
   - `chapterNumber = ChapterRecognition.parseChapterNumber(manga.title, chapter.name, chapter.chapterNumber)` (external, not read) and `copy(chapterNumber)`.
   - `dbChapter = dbChapters.find { url == chapter.url }`.
   - If null (**new**): if `dateUpload == 0` then `dateUpload = (maxSeenUploadDate == 0 ? nowMillis : maxSeenUploadDate)`; else `maxSeenUploadDate = max(maxSeenUploadDate, sourceChapter.dateUpload)`. Add to `newChapters`.
   - Else if `shouldUpdateDbChapter.await(dbChapter, chapter)` (external): compute rename need (`downloadProvider.isChapterDirNameChanged && downloadManager.isChapterDownloaded`) and call `downloadManager.renameChapter` when needed; build `toChangeChapter = dbChapter.copy(name, chapterNumber, scanlator, sourceOrder)`, and `dateUpload` from the source only if non-zero. Add to `updatedChapters`.
8. If `newChapters`, `removedChapters` and `updatedChapters` are all empty: possibly recompute interval (section 4.5), `return emptyList()`.
9. `readChapterNumbers = dbChapters.filter { read && isRecognizedNumber }.map { chapterNumber }.toSet()`.
10. For each `removedChapter`: if `read` add its number to `deletedReadChapterNumbers`; if `bookmark` add to `deletedBookmarkedChapterNumbers`; always add to `deletedChapterNumbers` (TreeSet<Double>).
11. `deletedChapterNumberDateFetchMap = removed.sortedByDescending { dateFetch }.associate { chapterNumber to dateFetch }` (for a duplicated number the LAST entry in the sorted order wins, i.e. the oldest dateFetch).
12. `markDuplicateAsRead = libraryPreferences.markDuplicateReadChapterAsRead() (key "mark_duplicate_read_chapter_read", default {}) .contains("new")`.
13. `itemCount = newChapters.size`; `updatedToAdd = newChapters.map { c -> `dateFetch = nowMillis + itemCount--`; if c.chapterNumber in readChapterNumbers && markDuplicateAsRead -> read = true and add url to changedOrDuplicateReadUrls; if !c.isRecognizedNumber or number not in deletedChapterNumbers -> return c; else read = number in deletedReadChapterNumbers, bookmark = number in deletedBookmarkedChapterNumbers, dateFetch = deletedMap[number] ?: keep, add url to changedOrDuplicateReadUrls }`.
    - Result: the first new chapter gets `nowMillis + N`, the last gets `nowMillis + 1`, so newest-first ordering in the database is preserved.
14. If removed non-empty: `chapterRepository.removeChaptersWithIds(removed ids)`.
15. If `updatedToAdd` non-empty: `updatedToAdd = chapterRepository.addAllChapters(updatedToAdd)` (returns rows with ids).
16. If `updatedChapters` non-empty: `updateChapter.awaitAll(updated.toChapterUpdate())`.
17. `updateManga.awaitUpdateFetchInterval(manga, now, fetchWindow)`; `updateManga.awaitUpdateLastUpdate(manga.id)` (sets `lastUpdate = now`).
18. `excludedScanlators = getExcludedScanlators.await(manga.id).toHashSet()`.
19. Return `updatedToAdd.filterNot { it.url in changedOrDuplicateReadUrls || it.scanlator in excludedScanlators }`.

So the returned "new chapters" exclude: chapters auto-marked read as duplicates, chapters that replaced deleted-read/bookmarked entries (they are treated as not new), and chapters from excluded scanlators. These are the chapters that feed notifications and downloads.

### 5.5 Episode sync (`SyncEpisodesWithSource.await`, `SyncEpisodesWithSource.kt:47-241`)

Identical structure with these differences (OBSERVED):
- Throws `NoEpisodesException` (key AYMR `no_episodes_error`).
- `episodeNumber` via `EpisodeRecognition.parseEpisodeNumber`.
- Read/seen naming: `seen`, `bookmark`, `fillermark`.
- Existing-episode update additionally copies `summary`, `memo`, `dateUpload`, `fillermark` only if the local is false (`if (!toChangeEpisode.fillermark) fillermark = source.fillermark`), `previewUrl` only if local is null/blank.
- Mark-duplicate pref: `markDuplicateSeenEpisodeAsSeen()` (key `mark_duplicate_seen_episode_seen`, default `{}`) contains `"new_episode"`.
- No excluded-scanlator filter on return; return is `updatedToAdd.filterNot { url in changedOrDuplicateReadUrls }`.
- Interval update call `updateAnime.awaitUpdateFetchInterval` and `awaitUpdateLastUpdate` as in chapters.

---

## 6. Update pass execution (updateChapterList / updateEpisodeList)

### 6.1 Concurrency model (OBSERVED, `MangaLibraryUpdateJob.kt:249-312`; anime `AnimeLibraryUpdateJob.kt:263-327`)

- `semaphore = Semaphore(5)`.
- `mangaToUpdate.groupBy { it.manga.source }.values` -> one group per source id. Each group runs in its own `async`.
- Within a group: `semaphore.withPermit { mangaInSource.forEach { ... } }`. **The permit covers the whole group, so at most 5 sources are processed concurrently and entries of one source are processed strictly sequentially.** This is the per-source politeness rule ("websites may not like it and could ban the user").
- Group ordering is the `groupBy` insertion order (order of first appearance in the sorted list, i.e. alphabetical by title of the first entry).
- `awaitAll()` on all groups.
- Per entry: `ensureActive()` (cancellation check). Then the library check: manga `getManga.await(id)?.favorite != true -> skip` (`return@forEach`); anime `anime.parentId == null && getAnime.await(id)?.favorite != true -> skip` (seasons are exempt).
- Each entry is wrapped in `withUpdateNotification(currentlyUpdating, progressCount, entry) { ... }`.

`withUpdateNotification` (OBSERVED):
1. `ensureActive()`; add entry to `currentlyUpdating` (CopyOnWriteArrayList); `showProgressNotification(currentlyUpdating, completed.get(), toUpdate.size)`.
2. run block.
3. `ensureActive()`; remove entry; `completed.getAndIncrement()`; `showProgressNotification(...)` again.
- Note: when the block throws, step 3 is skipped (the entry stays in the "currently updating" list). The block itself catches `Throwable` (section 6.2), so this only matters for cancellation thrown by the `ensureActive()` at step 3, where the entry is intentionally left listed.

### 6.2 Per-entry block (manga, `MangaLibraryUpdateJob.kt:272-306`)

```
try {
    newChapters = updateManga(manga, fetchWindow).sortedByDescending { sourceOrder }
    if newChapters not empty:
        toDownload = filterChaptersForDownload.await(manga, newChapters)
        if toDownload not empty:
            downloadChapters(manga, toDownload)      // enqueue with autoStart = false
            hasDownloads = true
        newMangaUpdatesCount += newChapters.size      // getAndSet { it + n }
        newUpdates.add(manga to newChapters.toTypedArray())
} catch (e: Throwable) {
    errorMessage = when (e) {
        NoChaptersException -> stringResource(no_chapters_error)            // "No chapters found"
        SourceNotInstalledException -> stringResource(loader_not_implemented_error)  // "Source not found"
        else -> e.message                                                  // may be null
    }
    failedUpdates.add(manga to errorMessage)
}
```
- Note the `sortedByDescending { sourceOrder }` ordering. For newly added chapters, `sourceOrder = i` where i=0 is newest on the source, so sorting by descending sourceOrder yields **oldest-first** order in `newChapters`. This order is what the notifications use (`chapters.first()` for the tap target, and `first()` of the sorted-unique set for summaries is sorted by chapter number separately). **Pitfall P-8: `openChapter` on tap uses `chapters.first()`, which is the oldest new chapter, not the newest.** Proposal: open the new chapter with the highest chapter number (fix P-8; decided in T-26a-034).
- Anime per-entry block (`AnimeLibraryUpdateJob.kt:291-320`) is the same, with `filterEpisodesForDownload` but **no download enqueue**, see 6.3.
- Error message keys in anime: `NoEpisodesException` -> AYMR `no_episodes_error` ("No episodes found"); `AnimeSourceNotInstalledException` -> MR `loader_not_implemented_error`; `else -> e.message`.
- `e.message` can be null -> `failedUpdates` holds a null message; the error-file grouping prints `! null`. **Pitfall P-9.**
- `catch (e: Throwable)` also catches `CancellationException` raised inside the block; such an entry is recorded as failed instead of stopping the pass. **Pitfall P-10.** Proposal: rethrow cancellation.

### 6.3 Download queueing

- Manga: `FilterChaptersForDownload.await` -> `downloadManager.downloadChapters(manga, chapters, autoStart = false)` (the private wrapper `downloadChapters` at `MangaLibraryUpdateJob.kt:332-336`, comment: "We don't want to start downloading while the library is updating, because websites may don't like it and they could ban the user"). After the whole pass: if `hasDownloads` -> `downloadManager.startDownloads()` (section 6.5).
- Anime: `filterEpisodesForDownload.await` computes `episodesToDownload` and sets `hasDownloads = true` if non-empty, **but nothing enqueues them**. The anime `downloadEpisodes(anime, episodes)` private helper (`AnimeLibraryUpdateJob.kt:347-351`, `downloadManager.downloadEpisodes(anime, episodes, false)`) exists and is never called (verified: no call site in the repo). Consequence: the anime pass calls `downloadManager.startDownloads()` with an empty queue, and new episodes are never auto-downloaded by the library update. **Observed defect; see pitfall P-1.** The notification action "Download" still works.

`FilterChaptersForDownload.await(manga, newChapters)` (`FilterChaptersForDownload.kt:29-44`):
1. Return empty if `newChapters` empty, or `!downloadPreferences.downloadNewChapters()` (key `download_new`, default `false`), or `!manga.shouldDownloadNewChapters()`.
2. If `!downloadPreferences.downloadNewUnreadChaptersOnly()` (key `download_new_unread_chapters_only`, default `false`) return all new chapters.
3. Else `readChapterNumbers = getChaptersByMangaId(manga.id).filter { read && isRecognizedNumber }.map(chapterNumber).toSet()`; return new chapters whose number is not in that set.

`shouldDownloadNewChapters` (`:52-67`): false if not favourite; `categories = getCategories(id).map(id).ifEmpty { [0] }`; `included = downloadPreferences.downloadNewChapterCategories()` (as Longs), `excluded = ...Exclude()`; then:
- both empty -> true;
- any category in excluded -> false;
- included empty -> true;
- else any category in included.

Anime: `FilterEpisodesForDownload.kt` is the same with `download_new_episode` (default `false`), `download_new_unread_episodes_only` (default `false`), `getEpisodesByAnimeId` `seen`, and category prefs keyed by `downloadNewEpisodeCategories` / `...Exclude`. (Keys not read; see data doc.)

Note the asymmetry: the "unread only" option for chapters compares **chapter numbers** against read chapters (so a re-uploaded chapter with a read number is not downloaded); anime does the same with episode numbers.

### 6.4 Counters and writes

- `newMangaUpdatesCount` (key `library_unread_updates_count`, default `0`), `newAnimeUpdatesCount` (key `library_unseen_updates_count`, default `0`): incremented by the number of new items per entry (`getAndSet { it + n }`). Read by the Updates tab badge (`main.rs` in the mockup shows "8" as a placeholder).
- DB writes per entry are performed by the sync (section 5) and by `awaitUpdateLastUpdate`.

### 6.5 Pass end (OBSERVED `updateChapterList` after `awaitAll`, `MangaLibraryUpdateJob.kt:314-329`)

1. `notifier.cancelProgressNotification()` (id -101).
2. If `newUpdates` non-empty: `notifier.showUpdateNotifications(newUpdates)`; then if `hasDownloads`: `downloadManager.startDownloads()`.
3. If `failedUpdates` non-empty: `errorFile = writeErrorFile(failedUpdates)`; `notifier.showUpdateErrorNotification(failedUpdates.size, errorFile.getUriCompat(context))`.

Order matters: summary notifications are shown **before** downloads start, and the error notification is shown last.

### 6.6 Errors that abort the whole pass

Only exceptions escaping `updateChapterList` (e.g. from `coroutineScope`, `notifier` calls or `writeErrorFile` outside its own try) reach `doWork`'s catch and return `failure()`. `writeErrorFile` swallows its own exceptions and returns `File("")`, which makes `getUriCompat` produce an empty URI for the error notification.

---

## 7. Error log file

**OBSERVED** `writeErrorFile(errors)` (`MangaLibraryUpdateJob.kt:393-420`; anime `AnimeLibraryUpdateJob.kt:405-432`):
1. If `errors` non-empty: `file = context.createFileInCacheDir("aniyomi_update_errors.txt")` (cache dir, file overwritten each run).
2. Write header: `stringResource(library_errors_help, ERROR_LOG_HELP_URL) + "\n\n"`. Manga URL `https://aniyomi.org/help/guides/troubleshooting`; anime URL `https://aniyomi.org/docs/guides/troubleshooting/`. String: `"For help on how to fix library update errors, see %1$s"`.
3. `errors.groupBy({ it.second }, { it.first })` (group by message, preserving order of first appearance): for each `(error, entries)`:
   - write `"\n! ${error}\n"`;
   - `entries.groupBy { it.source }`: for each source id, `source = sourceManager.getOrStub(srcId)`; write `"  # $source\n"` (the source's `toString`, name); then for each entry `"    - ${title}\n"`.
4. Return file. On any exception return `File("")`.

Format example:
```
For help on how to fix library update errors, see https://aniyomi.org/help/guides/troubleshooting

! No chapters found
  # SourceName
    - Title A
! null
  # Other
    - Title B
```

---

## 8. Notifications (summary; full spec in `p1-topic-notifications.md`)

### 8.1 Channels and IDs (OBSERVED `Notifications.kt`)

| Constant | Value | Use |
|---|---|---|
| `CHANNEL_LIBRARY_PROGRESS` | `"library_progress_channel"` | progress + size warning, importance LOW, group `group_library`, no badge |
| `CHANNEL_LIBRARY_ERROR` | `"library_errors_channel"` | error notification, LOW, group library, no badge |
| `CHANNEL_NEW_CHAPTERS_EPISODES` | `"new_chapters_episodes_channel"` | new chapters/episodes, DEFAULT importance |
| `ID_LIBRARY_PROGRESS` | `-101` | progress (foreground) |
| `ID_LIBRARY_ERROR` | `-102` | error |
| `ID_LIBRARY_SIZE_WARNING` | `-103` | size warning |
| `ID_NEW_CHAPTERS` | `-301` | manga summary group |
| `ID_NEW_EPISODES` | `-1301` | anime summary group |
| `GROUP_NEW_CHAPTERS` | `"eu.kanade.tachiyomi.NEW_CHAPTERS"` | |
| `GROUP_NEW_EPISODES` | `"eu.kanade.tachiyomi.NEW_EPISODES"` | |

Per-entry new-items notifications use `NotificationWithIdAndTag(entry.id.hashCode(), notification)` on the same channel, with no tag. Manga and anime both use the channel `CHANNEL_NEW_CHAPTERS_EPISODES` and the same `ID_LIBRARY_*` ids, so the two passes **share** progress/error/size notifications (a manga pass overwrites an anime pass's progress) and the per-entry ids can collide across tables (**pitfall P-11**).

### 8.2 Queue size warning

`showQueueSizeWarningNotificationIfNeeded(list)` (manga `MangaLibraryUpdateNotifier.kt:120-143`; anime `:138-161`):
- `maxUpdatesFromSource = list.groupBy { source }.filterKeys { sourceManager.get(it) !is UnmeteredSource }.maxOfOrNull { size } ?: 0`.
- If `maxUpdatesFromSource <= 60` return.
- Else notify `ID_LIBRARY_SIZE_WARNING` on `CHANNEL_LIBRARY_PROGRESS`: title `label_warning` ("Warning"), big text `notification_size_warning` ("Large updates harm sources and may lead to slower updates and also increased battery usage. Tap to learn more."), small icon `ic_warning_white_24dp`, timeout `30_000 ms` (`MangaDownloader.WARNING_NOTIF_TIMEOUT_MS` manga; `AnimeDownloader.WARNING_NOTIF_TIMEOUT_MS` anime, both `30_000L`), content intent opens `HELP_WARNING_URL`.
- `HELP_WARNING_URL = "https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads"` (both kinds; manga source has a `// TODO: Change when implemented` comment).
- Anime has an unused `showQueueSizeWarningNotification()` overload with the same body and no threshold check (dead code).
- `UnmeteredSource` sources are exempt from the count.

### 8.3 Progress notification

- Builder (`progressNotificationBuilder`, lazy, cached): channel progress; content title `app_name` ("Aniyomi", untranslated); small icon `ic_refresh_24dp`; large icon `ic_launcher` bitmap; ongoing; only-alert-once; one action "Cancel" (`action_cancel`) with the cancel broadcast PendingIntent.
- `showProgressNotification(entries, current, total)`:
  - content title: `notification_updating_progress` formatted with `percentFormatter.format(current.toFloat() / total)`; format is `NumberFormat.getPercentInstance()` with `RoundingMode.DOWN` and `maximumFractionDigits = 0` (e.g. "Updating library… (42%)").
  - if `!hideNotificationContent`: big text = `entries.joinToString("\n") { title.chop(40) }`.
  - `setProgress(total, current, false)`; notify `ID_LIBRARY_PROGRESS`.
- Cancelled at pass end (`cancelProgressNotification`).

### 8.4 Error notification

`showUpdateErrorNotification(failed, uri)`: if `failed == 0` return. Notify `ID_LIBRARY_ERROR` on `CHANNEL_LIBRARY_ERROR`: title `notification_update_error` ("%1$d update(s) failed"), text `action_show_errors` ("Tap to see details"), small icon `ic_ani`, tap opens the error file as `text/plain` via `ACTION_VIEW` with `FLAG_GRANT_READ_URI_PERMISSION` (`NotificationReceiver.openErrorLogPendingActivity`). This one is NOT suppressed by `hideNotificationContent`.

### 8.5 Summary and per-entry "new" notifications

`showUpdateNotifications(updates)` (`MangaLibraryUpdateNotifier.kt:173-227`):
1. Summary (always, ID `-301`/`-1301`):
   - title `notification_new_chapters` ("New chapters found") / AYMR `notification_new_episodes` ("New episodes found").
   - If `updates.size == 1 && !hideNotificationContent`: content text = `title.chop(45)`.
   - Else: content text = plural `notification_new_chapters_summary` ("For %d entry"/"For %d entries") / `notification_new_episodes_summary` ("For %d title"/"For %d titles") with `updates.size`; and if `!hideNotificationContent`, BigText of `updates.joinToString("\n") { title.chop(45) }`.
   - small icon `ic_ani`, large icon `ic_launcher`, group `GROUP_NEW_CHAPTERS`/`GROUP_NEW_EPISODES`, `GROUP_ALERT_SUMMARY`, group summary true, priority HIGH, content intent = `MainActivity` with action `Constants.SHORTCUT_UPDATES`, `FLAG_UPDATE_CURRENT | FLAG_IMMUTABLE`, auto-cancel.
2. If `!hideNotificationContent`: in `launchUI`, notify each `(entry, items)` as `createNewChaptersNotification(entry, chapters)`:
   - title: entry title; text and BigText: `getNewChaptersDescription(chapters)`; small icon `ic_ani`; large icon = coil image of the entry cover, circle-cropped, size 192 px, or none on failure; group; HIGH priority; content intent = reader for `chapters.first()` (`ReaderActivity.newIntent(manga.id, chapter.id)`, request code `manga.id.hashCode()`); anime content intent = episode player (`openEpisodePendingActivity(anime, episodes.first())`);
   - actions in order: (a) "Mark as read" (`action_mark_as_read`) / "Mark as watched" (`AYMR action_mark_as_seen`) broadcast; (b) "View chapters" (`action_view_chapters`) / "View episodes" (`action_view_episodes`), opens the entry screen (`SHORTCUT_MANGA` with `MANGA_EXTRA`, `notificationId`, `groupId`); (c) "Download" (`action_download`) only when `chapters.size <= 15` (manga `CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 15`) or `episodes.size <= 10` (anime `EPISODES_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 10`).

Note: the per-entry notification uses `setContentTitle(manga.title)` without chop. Summary uses chop 45.

### 8.6 getNewChaptersDescription (OBSERVED `MangaLibraryUpdateNotifier.kt:313-372`)

```
displayable = chapters.filter { isRecognizedNumber }.sortedBy { chapterNumber }
                       .map { formatChapterNumber(chapterNumber) }.toSet()    // LinkedHashSet, first occurrence kept
when displayable.size:
  0 -> plural notification_chapters_generic(chapters.size)    // "%1$d new chapter"/"%1$d new chapters"
  1 -> remaining = chapters.size - 1
       remaining == 0 -> "Chapter %1$s" (notification_chapters_single, first)
       else           -> "Chapter %1$s and %2$d more" (notification_chapters_single_and_more, first, remaining)   // remaining = chapters.size - 1
  else -> if displayable.size > 5:
              remaining = displayable.size - 5
              plural notification_chapters_multiple_and_more(remaining, first5.joinToString(", "), remaining)
              // "Chapters %1$s and 1 more" (one) / "Chapters %1$s and %2$d more" (other)
          else "Chapters %1$s" (notification_chapters_multiple, displayable.joinToString(", "))
```
- Anime: identical with `isRecognizedNumber`, `formatEpisodeNumber`, `NOTIF_MAX_EPISODES = 5`, plural keys `notification_episodes_generic` ("%d new episode"/"%1$d new episodes"), `AYMR notification_episodes_single` ("Episode %1$s"), `..._single_and_more` ("Episode %1$s and %2$d more"), `..._multiple` ("Episodes %1$s"), plural `notification_episodes_multiple_and_more` (one: "Episodes %1$s and %2$d more", other: same).
- Known quirk: the "single and more" count uses `chapters.size - 1` which includes unrecognised-number items, while the multi-number truncation remaining uses the distinct displayable count. Preserve for parity (**pitfall P-12**).
- `formatChapterNumber` / `formatEpisodeNumber` (`eu.kanade.presentation.util`, external) render the Double; output like "2.5".

### 8.7 chop

`chop(n)` (`eu.kanade.tachiyomi.util.lang`, external, not read) is used with 40 (progress list), 45 (summary and per-entry lists). **Proposal:** Rust truncates to n Unicode scalar values and appends "..." when cut (exact Kotlin semantics to be verified before release; test in T-26a-037).

### 8.8 Hide content

`SecurityPreferences.hideNotificationContent()` (external). When true: no per-entry notifications, no big text anywhere, progress has no list; summary title/text remains (generic). Error and size-warning notifications are not affected.

---

## 9. Metadata-only passes (Refresh library covers)

### 9.1 Manga (`MangaMetadataUpdateJob.kt`)

- `doWork`: `setForeground`; `addMangaToQueue()` = `mangaToUpdate = getLibraryManga.await()` (no category, no restriction filter, all favourites including duplicates across categories; **no distinctBy**, so duplicates are processed twice; **pitfall P-13**); `showQueueSizeWarningNotificationIfNeeded`.
- `updateMetadata()`: semaphore 5, grouped by source (same concurrency as 6.1); per entry, `withUpdateNotification`; `source = sourceManager.get(manga.source) ?: return@withUpdateNotification` (silently skipped if not installed, no error recorded); `networkManga = source.getMangaDetails(manga.toSManga())`; `updatedManga = manga.prepUpdateCover(coverCache, networkManga, true).copyFrom(networkManga)`; `updateManga.await(updatedManga.toMangaUpdate())`; an exception there is logged as "Manga doesn't exist anymore"; any other Throwable is logged and ignored.
- `prepUpdateCover(coverCache, networkManga, manualFetch = true)` and `copyFrom` (external, `eu.kanade.tachiyomi.util.prepUpdateCover`, `eu.kanade.domain.entries.manga.model.copyFrom`) are not read; the `manualFetch = true` argument forces cover refresh regardless of URL equality (matches `UpdateManga.awaitUpdateFromSource` semantics in 5.3).
- No errors are surfaced to the user (no error file, no error notification). `finally` cancels progress notification.
- The worker returns `Result.success()` after `updateMetadata()` completes; an exception escaping `updateMetadata()` (none expected, since every entry catches Throwable) would return `Result.failure()`.

### 9.2 Anime (`AnimeMetadataUpdateJob.kt`)

- `addAnimeToQueue()` = `animeToUpdate = getLibraryAnime.await()` (same duplicate issue, **P-13**).
- Per entry: `source = sourceManager.get(anime.source) ?: return@withUpdateNotification`; `updateAnimeFromRemote.awaitEpisodesUpdate(source, anime, fetchDetails = true).getOrThrow()` with `fetchEpisodes` **default false**; errors logged and ignored.
- So the anime metadata pass refreshes details and covers (`fetchDetails = true`, `fetchEpisodes = false`). `UpdateAnimeFromRemote.awaitEpisodesUpdate` still runs `syncEpisodesWithSource` on whatever episode list the source's `getAnimeEpisodeUpdate` returns with `fetchEpisodes = false` (external, not read). Expected effect: details and covers refresh; episode list is not re-fetched. Verify in T-26a-052 against the source API.

Note on wording: the menu label is "Refresh library covers" (`pref_refresh_library_covers`), but the anime job refreshes details and episode statuses, not only covers.

### 9.3 Shared unique work name (OBSERVED)

Both metadata jobs use `TAG = "MetadataUpdate"` and `WORK_NAME_MANUAL = "MetadataUpdate"`. `enqueueUniqueWork("MetadataUpdate", KEEP, ...)` and `isRunning("MetadataUpdate")` therefore treat manga and anime metadata as one job. Starting anime metadata while manga metadata runs returns `false` and does nothing. **Pitfall P-3.**

---

## 10. DataSaver (image proxy) - `app/src/main/java/aniyomi/util/DataSaver.kt`

### 10.1 Factory

`DataSaver(source, preferences)`:
1. `mode = preferences.dataSaver().get()` (key `data_saver`, enum `SourcePreferences.DataSaver`, default `NONE`).
2. If `mode != NONE && source.id.toString() in preferences.dataSaverExcludedSources().get()` (key `data_saver_excluded`, default `{}`) -> `NoOp`.
3. `NONE -> NoOp`, `BANDWIDTH_HERO -> BandwidthHeroDataSaver`, `WSRV_NL -> WsrvNlDataSaver`, `RESMUSH_IT -> ReSmushItDataSaver`.

Enum values exactly: `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` (`SourcePreferences.kt:118-122`).

Consumed by `HttpPageLoader` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt:51`) and the manga downloader (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:375`) via `HttpSource.getImage(page, dataSaver)`.

### 10.2 getImage(page, dataSaver)

```
imageUrl = page.imageUrl ?: return getImage(page)       // no URL cached -> plain fetch
page.imageUrl = dataSaver.compress(imageUrl)
try   { return getImage(page) }                         // network via the source
finally { page.imageUrl = imageUrl }                    // restore original URL
```

### 10.3 Bandwidth Hero (`BandwidthHeroDataSaver`)

Preferences: `dataSavedServer = dataSaverServer().trimEnd('/')` (key `data_saver_server`, default `""`); `ignoreJpg = dataSaverIgnoreJpeg()` (key `ignore_jpeg`, default `false`); `ignoreGif = dataSaverIgnoreGif()` (key `ignore_gif`, default `true`); `format = dataSaverImageFormatJpeg()` -> `"1"` or `"0"` (key `data_saver_image_format_jpeg`, default `false`); `quality = dataSaverImageQuality()` (key `data_saver_image_quality`, default `80`); `colorBW = dataSaverColorBW()` -> `"1"`/`"0"` (key `data_saver_color_bw`, default `false`).

`compress(imageUrl)`:
- If `dataSavedServer` is blank OR `imageUrl.contains(dataSavedServer)`: return `imageUrl` unchanged.
- If URL contains `.jpeg` or `.jpg` (case-insensitive): if `ignoreJpg` return unchanged, else `getUrl`.
- Else if contains `.gif`: `ignoreGif ? unchanged : getUrl`.
- Else `getUrl`.

`getUrl(imageUrl) = "$server/?jpg=$format&l=$quality&bw=$colorBW&url=" + URLEncoder.encode(imageUrl, "utf-8")`.

### 10.4 wsrv.nl (`WsrvNlDataSaver`)

Preferences: `ignoreJpg`, `ignoreGif` as above; `format = dataSaverImageFormatJpeg()` (Boolean, not string); `quality` as above.

`compress`: same jpg/gif decision as 10.3, but `else -> getUrl` always.

`getUrl(imageUrl)` = `"https://wsrv.nl/?url=$imageUrl"` + suffix, where suffix is:
- if the URL contains `.webp` (case-insensitive) or `.gif`:
  - `format == false` -> `"&q=$quality&n=-1"` (keep animated extension);
  - `format == true` -> `"&output=jpg&q=$quality&n=-1"`;
- else:
  - `format == true` -> `"&output=jpg&q=$quality"`;
  - `format == false` -> `"&output=webp&q=$quality"`.

Note: the image URL is NOT percent-encoded here (raw concatenation). **Pitfall P-14** (URLs with `&` or `#` break the proxy query). Proposal: percent-encode.

### 10.5 reSmush.it (`ReSmushItDataSaver`)

Preferences: `ignoreJpg`, `ignoreGif`, `quality`. `compress` as in 10.4 (jpg/gif rule, `else -> getUrl`).

`getUrl(imageUrl)`: **synchronous** request `GET http://api.resmush.it/ws.php?img=$imageUrl&qlty=$quality` through `NetworkHelper.client` (`execute()`), then `body.string().substringAfter("\"dest\":\"").substringBefore("\",")`. The returned value is the optimised image URL from the JSON `dest` field. No error handling: on failure or missing field the substring result is the whole body or an unusable string. **Pitfall P-15** (blocking call on the reader's loader thread; plain HTTP; unescaped image URL; no failure fallback). Proposal: async call, JSON parse with `serde_json`, fall back to the original URL on any error, use HTTPS only if the server supports it (not verified; leave as `http` only if the service requires it, otherwise decide in T-26a-044).

---

## 11. Chapter and episode utilities

### 11.1 filterDownloadedChapters (`ChapterFilterDownloaded.kt`)
- If `manga.isLocal()` return `this`.
- Else `downloadCache.isChapterDownloaded(name, scanlator, manga.title, manga.source, skipCache = false)` per chapter, keep those true. Note the trailing `false` is the `skipCache` parameter (cache used).

### 11.2 filterDownloadedEpisodes (`EpisodeFilterDownloaded.kt`)
- Same with `AnimeDownloadCache.isEpisodeDownloaded`.

### 11.3 getNextUnread (`ChapterGetNextUnread.kt`)
- Input: chapters already filtered by the view filters, which are applied first by `applyFilters` (`app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt`): unread filter (`!read`), bookmark filter, downloaded filter (`downloaded || isLocal`) and then `sortedWith(getChapterSort(manga))`.
- If `manga.sortDescending()` (`chapterFlags & CHAPTER_SORT_DIR_MASK == CHAPTER_SORT_DESC`) -> `findLast { !read }`; else `find { !read }`.
- Note: the function does not apply `applyFilters`' unread filter itself beyond what the list contains; callers pass the full chapter list and rely on `!it.read`. The list-of-Item variant returns `.chapter`.
- Callers: `MangaScreenModel.kt:648-650` (uses `successState.chapters`), `MangaLibraryScreenModel.kt:436-437` (`getChaptersByMangaId(manga.id, applyScanlatorFilter = true)` then `getNextUnread(manga, downloadManager)`).

### 11.4 getNextUnseen (`EpsiodeGetNextUnseen.kt`)
- Same with `anime.sortDescending()` (`episodeFlags & EPISODE_SORT_DIR_MASK == EPISODE_SORT_DESC`), `!seen`, `EpisodeList.Item`. Callers `AnimeScreenModel.kt:750-759`, `AnimeLibraryScreenModel.kt:450-451`.

### 11.5 removeDuplicates (`ChapterRemoveDuplicates.kt`)
- `groupBy { chapterNumber }` (Double key, so `1.0` and `1.00` group together; `NaN` not expected) then for each group pick `find { id == current.id } ?: find { scanlator == current.scanlator } ?: first()`. Output order is the group order (first appearance). Used by the reader (`ReaderViewModel.kt:209`, `:481`) to avoid showing duplicate chapters.

---

## 12. Edge cases and defects found (OBSERVED)

| ID | Where | Behaviour | Severity | Proposal |
|---|---|---|---|---|
| P-1 | AnimeLibraryUpdateJob.kt:347-351, 296-300 | New episodes are never queued for download; `downloadEpisodes` unused | High (feature gap) | Wire the enqueue as manga does (`downloadEpisodes(anime, toDownload, autoStart = false)`) |
| P-3 | Metadata jobs shared work name `"MetadataUpdate"` | Manga and anime metadata refreshes are mutually exclusive | Medium | Use separate names `MetadataUpdate-manga` / `-anime` |
| P-5 | Anime doWork lacks manual-running check | Scheduled anime pass can run alongside a manual pass | Medium | Global "pass running" lock per kind plus retry |
| P-7 | Anime updateAnime syncs before favourite recheck | Un-favourited anime still gets sync writes | Low | Re-check favourite before sync |
| P-8 | Notification tap uses `chapters.first()` (oldest in `sortedByDescending{sourceOrder}`) | Opens an older chapter | Low | Open the newest new chapter (highest chapter number) |
| P-9 | error map `e.message` nullable | Error file prints `! null` | Low | Use `e.to_string()` fallback |
| P-10 | `catch (e: Throwable)` inside entry | Cancellation becomes a failure entry | Low | Rethrow cancellation |
| P-11 | Shared notification ids for manga and anime; per-entry ids = `id.hashCode()` | Notifications overwrite each other | Low | Namespace ids (hash of kind + id) |
| P-12 | `getNewChaptersDescription` remaining counts | Quirk in "and N more" math | Low | Keep for parity; document |
| P-13 | Metadata jobs do not dedupe multi-category entries | Entries processed twice | Low | `distinct` by id |
| P-14 | wsrv.nl URL not encoded | Breaks for `&`/`#` in image URLs | Medium | Percent-encode |
| P-15 | reSmush.it synchronous HTTP over plain `http://`, no fallback | Blocks loader, leaks image URLs in clear text | Medium | Async, HTTPS-first, fallback to original |
| P-16 | `startNow` returns false when `isRunning(TAG)` even if the running work is the scheduled auto pass | User sees "already running" while a scheduled pass runs (intended) | Info | Keep (the scheduled pass is a legitimate run) |
| P-17 | `setupTask` on `interval <= 0` cancels only `WORK_NAME_AUTO`; a running auto pass continues | Expected | Info | Keep |
| P-18 | `dismissNotification` (private 2-arg, NotificationReceiver.kt:191-194) calls `notificationManager.cancelAll()` | Dismissing one notification via `ACTION_DISMISS_NOTIFICATION` cancels the progress notification too | Low | Cancel only the given id |

Other edge cases (OBSERVED, all handled):
- Empty library or empty queue: `updateChapterList` does nothing; no notifications except `cancelProgressNotification`; `lastUpdatedTimestamp` still updates; the worker returns success.
- Entry without source installed: `getOrStub` returns a stub; `getChapterList` throws `SourceNotInstalledException`; error `loader_not_implemented_error` ("Source not found") is recorded.
- Source returns zero items: manga throws `NoChaptersException` inside `SyncChaptersWithSource`, which the per-entry `catch (Throwable)` records as a failure with message "No chapters found". Local sources (`source.isLocal()`) are exempt from the throw. Anime is the same with "No episodes found".
- Entry removed from library mid-pass: skipped at the favourite check; if removed during the network call, `updateManga` returns empty.
- Interval override: a stored negative `fetch_interval` is respected everywhere.
- Restrictions include `manga_outside_release_period` by default: an entry with `nextUpdate` after tomorrow 00:00 is skipped in automatic AND manual passes (manual is not exempt).
- `fetchWindow` `(0,0)` passed to `toMangaUpdate` means "compute now" (`MangaFetchInterval.kt:26-30`).

---

## 13. Message keys (MR / AYMR) used by this area

| Key | Module | English value | Used in |
|---|---|---|---|
| `skipped_reason_not_always_update` | MR | Skipped because series does not require updates | queue (log only) |
| `skipped_reason_completed` | MR | Skipped because series is complete | queue (log only) |
| `skipped_reason_not_caught_up` | MR | Skipped because there are unread chapters | queue (log only) |
| `skipped_reason_not_started` | MR | Skipped because no chapters are read | queue (log only) |
| `skipped_reason_not_in_release_period` | MR | Skipped because no release was expected today | queue (log only) |
| `no_chapters_error` | MR | No chapters found | error file / error map |
| `no_episodes_error` | AYMR | No episodes found | error file / error map |
| `loader_not_implemented_error` | MR | Source not found | error file / error map |
| `library_errors_help` | MR | For help on how to fix library update errors, see %1$s | error file header |
| `notification_update_error` | MR | %1$d update(s) failed | error notification title |
| `action_show_errors` | MR | Tap to see details | error notification text |
| `notification_updating_progress` | MR | Updating library… (%s) | progress title |
| `app_name` | MR | Aniyomi (translatable=false) | progress title |
| `notification_size_warning` | MR | Large updates harm sources and may lead to slower updates and also increased battery usage. Tap to learn more. | size warning |
| `label_warning` | MR | Warning | size warning title |
| `notification_new_chapters` | MR | New chapters found | summary title |
| `notification_new_episodes` | AYMR | New episodes found | summary title |
| `notification_new_chapters_summary` (plural) | MR | one: For %d entry / other: For %d entries | summary text |
| `notification_new_episodes_summary` (plural) | AYMR | one: For %d title / other: For %d titles | summary text |
| `notification_chapters_generic` (plural) | MR | one: %1$d new chapter / other: %1$d new chapters | description |
| `notification_chapters_single` | MR | Chapter %1$s | description |
| `notification_chapters_single_and_more` | MR | Chapter %1$s and %2$d more | description |
| `notification_chapters_multiple` | MR | Chapters %1$s | description |
| `notification_chapters_multiple_and_more` (plural) | MR | one: Chapters %1$s and 1 more / other: Chapters %1$s and %2$d more | description |
| `notification_episodes_generic` (plural) | AYMR | one: %d new episode / other: %1$d new episodes | description |
| `notification_episodes_single` | AYMR | Episode %1$s | description |
| `notification_episodes_single_and_more` | AYMR | Episode %1$s and %2$d more | description |
| `notification_episodes_multiple` | AYMR | Episodes %1$s | description |
| `notification_episodes_multiple_and_more` (plural) | AYMR | Episodes %1$s and %2$d more (both) | description |
| `action_mark_as_read` | MR | Mark as read | action |
| `action_mark_as_seen` | AYMR | Mark as watched | action |
| `action_view_chapters` | MR | View chapters | action |
| `action_view_episodes` | AYMR | View episodes | action |
| `action_download` | MR | Download | action |
| `action_cancel` | MR | Cancel | progress action |
| `channel_*` | MR/AYMR | channel names (see Notifications.kt:137-196) | channels |
| `update_already_running` | MR | An update is already running | snackbar |
| `updating_category` | MR | Updating category | snackbar |
| `update_never` ... `update_weekly` | MR | Off / Every 12 hours / Daily / Every 2 days / Every 3 days / Weekly | settings |
| `pref_library_update_interval` | MR | Automatic updates | settings |
| `pref_library_update_restriction` | MR | Automatic updates device restrictions | settings |
| `connected_to_wifi`, `network_not_metered`, `charging`, `restrictions` | MR | Only on Wi-Fi / Only on unmetered network / When charging / Restrictions: %s | settings |
| `pref_category_library_update` | MR | Global update | settings group |
| `pref_refresh_library_covers` | MR | Refresh library covers | settings action |
| `download_error` | AYMR | (used by openChapter toast when entry or chapter missing) | notification open |

Plural quantity rules follow Android `Plurals` (`one`, `other`).

---

## 14. Timings, limits and constants (single table)

| Name | Value | Where |
|---|---|---|
| Semaphore permits (sources in parallel) | 5 | both jobs, both metadata jobs |
| Periodic flex | 10 minutes | `PeriodicWorkRequestBuilder(interval h, 10 min)` |
| Backoff | LINEAR, 10 minutes | `setBackoffCriteria` |
| Auto interval choices | 0, 12, 24, 48, 72, 168 hours (UI) | SettingsLibraryScreen |
| Battery not low | always required | Constraints |
| Queue warning threshold (per non-unmetered source) | 60 entries | notifiers |
| Warning notification timeout | 30 000 ms | Manga/AnimeDownloader constants |
| Summary/entry title chop | 45 characters | NOTIF_TITLE_MAX_LEN |
| Progress title chop | 40 characters | `chop(40)` |
| Chapter-number list truncation | 5 | NOTIF_MAX_CHAPTERS / NOTIF_MAX_EPISODES |
| Notification cover size | 192 px | NOTIF_ICON_SIZE |
| Download action threshold | chapters <= 15, episodes <= 10 | Manga/AnimeDownloader constants |
| Chapter window (fetch interval) | 3 if <= 8 chapters else 10 | FetchInterval |
| Fetch interval min/max | 1 / 28 days | MAX_INTERVAL |
| Default interval when no history | 7 days | FetchInterval |
| Grace period | 1 day either side | GRACE_PERIOD |
| Increase-interval threshold | 10 missed cycles | `increaseWhenOver = 10` |
| Stored interval override | negative = fixed abs(days) | MangaUpdate.fetchInterval |
| Error file | `aniyomi_update_errors.txt` in cache dir | writeErrorFile |
| Data saver quality default | 80 | `data_saver_image_quality` |
| Bandwidth Hero endpoint | user-configured `data_saver_server` (no default) | BandwidthHeroDataSaver |
| wsrv.nl endpoint | `https://wsrv.nl/` | WsrvNlDataSaver |
| reSmush.it endpoint | `http://api.resmush.it/ws.php` | ReSmushItDataSaver |
| Help URLs | manga `https://aniyomi.org/help/guides/troubleshooting`; anime `https://aniyomi.org/docs/guides/troubleshooting/`; warning `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads` | constants |

---

## 15. Interactions with other areas

- Downloads: area:23-downloads-manga and area:24-downloads-anime own enqueue and run; this area only calls `downloadChapters(..., autoStart=false)` and `startDownloads()`. Download-side warning thresholds are defined there (`MangaDownloader.kt:767-768`, `AnimeDownloader.kt:940-941`) and reused for the notification actions here.
- Notifications actions: NotificationReceiver (area:01-foundation-app-bootstrap) implements the broadcast receivers listed in 8.5. Mark-as-read/seen honours `removeAfterMarkedAsRead` (deletes downloads).
- Backup/restore: `PreferenceRestorer.kt:45-46` re-applies the schedule after restore. Restore does not touch `library_update_last_timestamp`.
- Migrations listed in 1.1.
- Track sync and source details are separate areas.
