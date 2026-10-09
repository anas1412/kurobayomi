# Area 27-tracking-core, part 1: behaviour specification

Paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Line numbers are `path:line`. Labels: **OBSERVED** is what the Kotlin does. **Proposal:** is what Kurobayomi should do. Part 2 files (`ui/entries/*/track/*`, `ui/setting/track/*`) are cited only where part 1 depends on them.

Contents: 0 conventions and glossary; 1 registry; 2 tracker base contract; 3 remote setter rules; 4 domain model and mapping; 5 search models; 6 bind (add tracking); 7 refresh; 8 progress sync; 9 progress triggers (TrackEpisode and TrackChapter); 10 delayed queue and retry job; 11 mark-read policy; 12 settings screen; 13 track info sheet; 14 selectors; 15 search screen; 16 remove tracking; 17 login and OAuth plumbing; 18 message keys; 19 concurrency; 20 timings and limits; 21 cross-area interactions; 22 observed defects.

---

## 0. Conventions and glossary

- **Tracker**: an external service (AniList, MyAnimeList, Kitsu, ...) that stores a user's list. Kotlin type `eu.kanade.tachiyomi.data.track.Tracker`.
- **Tracker id**: a fixed `Long` used as the foreign key `sync_id` (anime) and `sync_id` (manga) in the tables `anime_sync` and `manga_sync`. Ids are in section 1.
- **Track**: one row that binds one entry (anime or manga) to one tracker. Domain types `tachiyomi.domain.track.anime.model.AnimeTrack` and `tachiyomi.domain.track.manga.model.MangaTrack`.
- **Entry**: the anime or manga the user has in the library.
- **Progress**: `lastEpisodeSeen` (anime, `Double`) or `lastChapterRead` (manga, `Double`).
- **Completion status**: the tracker's "Completed" status value (`getCompletionStatus()`).
- **Recognized number**: an episode or chapter whose number is `>= 0f` (`domain/.../episode/model/Episode.kt:27-28`, `domain/.../chapter/model/Chapter.kt:19-20`).
- **Logged in**: `isLoggedIn` is `getUsername().isNotEmpty() && getPassword().isNotEmpty()` (`app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt:32-34`). OAuth trackers save their user name as the username and their access token as the password (`anilist/Anilist.kt:352`, `bangumi/Bangumi.kt:198`, `myanimelist/MyAnimeList.kt:256`, `simkl/Simkl.kt:127`, `shikimori/Shikimori.kt:228`). Enhanced trackers without a login save the constant pair `"user"` / `"pass"` (`kavita/Kavita.kt:90`, `komga/Komga.kt:88`, `jellyfin/Jellyfin.kt:79`, `suwayomi/Suwayomi.kt:81`). Section 17.5.

---

## 1. Tracker registry (TrackerManager)

Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt`.

### 1.1 Constants and instances (OBSERVED, lines 19-37)

| Field | Class | `id` | Name (`BaseTracker` name) | Kinds implemented |
|---|---|---|---|---|
| `myAnimeList` | `MyAnimeList(1L)` | 1 | "MyAnimeList" | Manga, Anime |
| `aniList` | `Anilist(ANILIST)` | 2 | "AniList" | Manga, Anime |
| `kitsu` | `Kitsu(KITSU)` | 3 | "Kitsu" | Anime, Manga |
| `shikimori` | `Shikimori(4L)` | 4 | "Shikimori" | Manga, Anime |
| `bangumi` | `Bangumi(5L)` | 5 | "Bangumi" | Manga, Anime |
| `komga` | `Komga(6L)` | 6 | "Komga" | Manga (enhanced) only |
| `mangaUpdates` | `MangaUpdates(7L)` | 7 | "MangaUpdates" | Manga (deletable) only |
| `kavita` | `Kavita(KAVITA)` | 8 | "Kavita" | Manga (enhanced) only |
| `suwayomi` | `Suwayomi(9L)` | 9 | "Suwayomi" | Manga (enhanced) only |
| `simkl` | `Simkl(SIMKL)` | 101 | "Simkl" | Anime only |
| `jellyfin` | `Jellyfin(JELLYFIN)` | 102 | "Jellyfin" | Anime (enhanced) only |

Companion constants: `ANILIST = 2L`, `KITSU = 3L`, `KAVITA = 8L`, `SIMKL = 101L`, `JELLYFIN = 102L` (lines 20-24).

The class declarations confirm the kinds: `Anilist` (`anilist/Anilist.kt:28-34`: `MangaTracker, AnimeTracker`), `Kitsu` (`kitsu/Kitsu.kt:27-33`), `Shikimori` (`shikimori/Shikimori.kt:26-32`), `Bangumi` (`bangumi/Bangumi.kt:24`: `MangaTracker, AnimeTracker`), `Komga` (`komga/Komga.kt:20`: `EnhancedMangaTracker, MangaTracker`), `MangaUpdates` (`mangaupdates/MangaUpdates.kt:20`: `MangaTracker, DeletableMangaTracker`), `Kavita` (`kavita/Kavita.kt:23`: `EnhancedMangaTracker, MangaTracker`), `Suwayomi` (`suwayomi/Suwayomi.kt:18`: `EnhancedMangaTracker, MangaTracker`), `Simkl` (`simkl/Simkl.kt:20`: `AnimeTracker`), `Jellyfin` (`jellyfin/Jellyfin.kt:20`: `EnhancedAnimeTracker, AnimeTracker`), `MyAnimeList` (`myanimelist/MyAnimeList.kt:25-31`).

### 1.2 Ordering

`trackers` (line 39-42) is `listOf(myAnimeList, aniList, kitsu, shikimori, bangumi, komga, mangaUpdates, kavita, suwayomi, simkl, jellyfin)`. Order matters for UI: it is the order of the settings rows filtered by kind (section 12) and the order used by `loggedInTrackers()`.

### 1.3 Functions (OBSERVED, lines 44-54)

- `loggedInTrackers(): List<Tracker>` = `trackers.filter { it.isLoggedIn }` (line 44). Order is registry order.
- `loggedInTrackersFlow(): Flow<List<Tracker>>` = `combine(trackers.map { it.isLoggedInFlow })` and then keep the trackers whose flag is true, in registry order (lines 46-50). It emits once all flows have emitted.
- `get(id: Long): Tracker?` = `trackers.find { it.id == id }` (line 52). Returns null for unknown ids. Callers treat null as "not available" (see sections 9 and 13).
- `getAll(ids: Set<Long>): List<Tracker>` = `trackers.filter { it.id in ids }` (line 54). Not called from part 1 scope.

### 1.4 Capability summary used by callers (OBSERVED)

- The anime info sheet lists only `loggedInTrackers().filter { it is AnimeTracker }` (`ui/entries/anime/track/AnimeTrackInfoDialog.kt:268-273`). Manga-only trackers are therefore absent from the anime sheet.
- `Tracker.animeService` is `this as AnimeTracker` and `mangaService` is `this as MangaTracker` (`Tracker.kt:48-52`). Calling `animeService` on a manga-only tracker throws `ClassCastException`. All observed callers check `is AnimeTracker` first.

**Proposal:** replace the unchecked casts with `fn as_anime(&self) -> Option<&dyn AnimeTracker>` and `fn as_manga(&self) -> Option<&dyn MangaTracker>`. See `p1-rust-design.md` section 3.

---

## 2. Tracker base contract (Tracker, BaseTracker)

### 2.1 `Tracker` interface (OBSERVED, `Tracker.kt:10-53`)

Members, in declaration order:
- `val id: Long`, `val name: String`, `val client: OkHttpClient`.
- `val supportsReadingDates: Boolean` (application and remote support date fields). Default in `BaseTracker` is `false` (`BaseTracker.kt:23`).
- `val supportsPrivateTracking: Boolean`. Default `false` (`BaseTracker.kt:25`).
- `@DrawableRes fun getLogo(): Int`, `@ColorInt fun getLogoColor(): Int`.
- `fun getCompletionStatus(): Long`, `fun getScoreList(): ImmutableList<String>`.
- `suspend fun login(username: String, password: String)`.
- `@CallSuper fun logout()`.
- `val isLoggedIn: Boolean`, `val isLoggedInFlow: Flow<Boolean>`.
- `fun getUsername(): String`, `fun getPassword(): String`, `fun saveCredentials(username: String, password: String)`.
- `val animeService: AnimeTracker get() = this as AnimeTracker` and `val mangaService: MangaTracker get() = this as MangaTracker`.

### 2.2 `BaseTracker` (OBSERVED, `BaseTracker.kt:11-52`)

- Constructor `BaseTracker(id, name)`. Injected: `trackPreferences: TrackPreferences` (line 16) and `networkService: NetworkHelper` (line 17), both via `injectLazy`.
- `client` = `networkService.client` (line 19-20). Every tracker shares the app's OkHttp client (area 12).
- `logout()` (`@CallSuper`, lines 27-30) = `trackPreferences.setCredentials(this, "", "")`. `setCredentials` writes username `""`, password `""` and sets `trackAuthExpired` to `false` (`TrackPreferences.kt:29-33`).
- `isLoggedIn` (lines 32-34): both username and password are non-empty.
- `isLoggedInFlow` (lazy, lines 36-43): `combine(username.changes(), password.changes()) { u, p -> u.isNotEmpty() && p.isNotEmpty() }`.
- `getUsername()` / `getPassword()` read the preference (lines 45-47).
- `saveCredentials(u, p)` = `trackPreferences.setCredentials(this, u, p)` (lines 49-51). Note `setCredentials` also clears the auth-expired flag.

**Proposal:** `login` must always end in one of two states: credentials saved and logged in, or nothing saved. `logout` clears the token store and the auth-expired flag. Section 17.5 gives the stored keys and section 5 of `p1-rust-design.md` the storage rule.

### 2.3 Login and logout flows (OBSERVED, `ui/setting/track/...` and `SettingsTrackingScreen.kt`)

- Credential login (Kitsu, MangaUpdates): dialog collects username and password (section 12.5). On success the tracker's `login(u, p)` must store them. On failure the caller calls `tracker.logout()` (section 12.5).
- OAuth login (AniList, MyAnimeList, Shikimori, Simkl, Bangumi): opens the auth URL in the default browser, the redirect comes back to the app, and `login(code)` or `login(token)` is called. Section 17.
- Enhanced trackers: `loginNoop()` (no-op) (`SettingsTrackingScreen.kt:231, 239`).

---

## 3. Remote setter rules (AnimeTracker and MangaTracker defaults)

These are default methods of the interfaces. Concrete trackers override `update`, `bind`, `refresh` and the status and score tables, but the rules below are inherited unless a tracker overrides the method.

### 3.1 `AnimeTracker` (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt`)

Abstract members (no body): `getCompletionStatus(): Long` (line 27), `getScoreList(): ImmutableList<String>` (29), `getStatusListAnime(): List<Long>` (36), `getWatchingStatus(): Long` (38), `getRewatchingStatus(): Long` (40), `displayScore(track: DomainAnimeTrack): String` (47), `update(track: AnimeTrack, didWatchEpisode: Boolean = false): AnimeTrack` (49), `bind(track: AnimeTrack, hasSeenEpisodes: Boolean = false): AnimeTrack` (51), `searchAnime(query: String): List<AnimeTrackSearch>` (53), `refresh(track: AnimeTrack): AnimeTrack` (55), `getStatusForAnime(status: Long): StringResource?` (122).

Defaults:
- `indexToScore(index: Int): Double = index.toDouble()` (lines 31-33).
- `get10PointScore(track: DomainAnimeTrack): Double = track.score` (lines 43-45). The source comment records a planned change to store every score on a 10-point scale. Unused by part 1.
- `register(item, animeId)` (lines 58-65): see section 6.4.

### 3.2 `setRemoteAnimeStatus(track, status)` (OBSERVED, lines 67-73)

1. `track.status = status`.
2. If `track.status == getCompletionStatus() && track.total_episodes != 0L` then `track.last_episode_seen = track.total_episodes.toDouble()`.
3. `updateRemote(track)`.

Note: the finish date is not set here. The tracker's `update` may set it.

### 3.3 `setRemoteLastEpisodeSeen(track, episodeNumber: Int)` (OBSERVED, lines 75-88)

Exact order:
1. If `track.last_episode_seen == 0.0 && track.last_episode_seen < episodeNumber && track.status != getRewatchingStatus()` then `track.status = getWatchingStatus()`. Since `last_episode_seen == 0.0` already implies `0.0 < episodeNumber` for positive numbers, the rule is "progress is zero, the new number is positive, and the entry is not in the rewatch status".
2. `track.last_episode_seen = episodeNumber.toDouble()`.
3. If `track.total_episodes != 0L && track.last_episode_seen.toLong() == track.total_episodes` then `track.status = getCompletionStatus()` and `track.finished_watching_date = System.currentTimeMillis()`.
4. `updateRemote(track)`.

Edge rules (OBSERVED):
- Episode 0 never sets Watching (rule 1 needs a positive number after the `0.0 <` test).
- Rule 3 uses `toLong()` truncation. `12.0` matches total `12`; `11.9` does not.
- Rule 3 overwrites an existing finish date with now. Re-completing an entry changes its finish date.
- When progress was already above zero, rule 1 does not run, so a Completed status set earlier is not changed by a lower episode. The status is only reset by an explicit status change.

### 3.4 `setRemoteScore(track, scoreString)` (OBSERVED, lines 90-93)

`track.score = indexToScore(getScoreList().indexOf(scoreString))` then `updateRemote(track)`.

Consequences:
- `indexOf` returns `-1` when the string is not in the list. The stored score is then `-1.0`. The search model default sentinel is also `-1.0` (`AnimeTrackSearch.kt:25`, `MangaTrackSearch.kt:25`). The UI treats `-1.0` as "no score" in search results (section 15.4) and `0.0` as "no score" in the info sheet (section 13.4). The two sentinels are inconsistent. Section 22.

### 3.5 Date and private setters (OBSERVED, lines 95-108)

- `setRemoteStartDate(track, epochMillis: Long)`: `track.started_watching_date = epochMillis`, then `updateRemote`. Passing `0` clears the date (used by the remove-date flow, section 14.5).
- `setRemoteFinishDate(track, epochMillis: Long)`: `track.finished_watching_date = epochMillis`, then `updateRemote`.
- `setRemotePrivate(track, private: Boolean)`: `track.private = private`, then `updateRemote`.

### 3.6 `updateRemote(track)` (OBSERVED, lines 110-120; private)

Runs on `withIOContext`:
1. `update(track)` (the tracker's network call). The return value is **discarded**.
2. `track.toDomainTrack(idRequired = false)?.let { insertTrack.await(it) }`. The local row is written from the mutated input object, not from the tracker's response.
3. On `Exception`: `logcat(ERROR)` with "Failed to update remote track data id=${track.id}", then `withUIContext { toast(e.message) }`. The toast text is the raw exception message, with no localized prefix.
4. `Throwable` (not `Exception`) is not caught here and propagates to the caller.

**Proposal:** keep the same order (remote first, then local). Do not write local state when the remote call failed. Show the error through the error-toast key `track_error` (section 18) or a plain error line, since raw messages can be empty.

### 3.7 `MangaTracker` (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt`)

Mirror of 3.1 with names `getStatusListManga`, `getReadingStatus`, `getRereadingStatus`, `getStatusForManga`, `searchManga`, and fields `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date`:
- `setRemoteMangaStatus` (lines 66-72): same as 3.2 with `total_chapters` and `last_chapter_read`.
- `setRemoteLastChapterRead(track, chapterNumber: Int)` (lines 74-89): same rules as 3.3. Watching becomes Reading, Rereading is `getRereadingStatus()`, the completion check is `last_chapter_read.toLong() == total_chapters`, the finish date is `finished_reading_date = System.currentTimeMillis()`.
- `setRemoteScore`, `setRemoteStartDate`, `setRemoteFinishDate`, `setRemotePrivate`, `updateRemote`: identical to 3.4 to 3.6, using `started_reading_date` and `finished_reading_date`.
- `register(item, mangaId)` (lines 57-64): identical to 6.4 with `addTracks.bind`.

Comparison note: the manga `setRemoteLastChapterRead` condition has the formatting on separate lines (lines 82-84) but the same logic.

### 3.8 Completion-status and reading-status semantics (OBSERVED)

- Anime: `getWatchingStatus()`, `getRewatchingStatus()`, `getCompletionStatus()` and `getStatusListAnime()` are the tracker's own numbers. The app never hard-codes values in part 1.
- The status picker shows `getStatusListAnime()` in that order, with labels `getStatusForAnime(it)` (section 14.2).
- The info sheet status label uses `(item.tracker as? AnimeTracker)?.getStatusForAnime(track.status)`. An unknown status gives `null` and an empty label.

---

## 4. Domain model and mapping

### 4.1 Domain models (OBSERVED)

`domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20`:
`data class AnimeTrack(id: Long, animeId: Long, trackerId: Long, remoteId: Long, libraryId: Long?, title: String, lastEpisodeSeen: Double, totalEpisodes: Long, status: Long, score: Double, remoteUrl: String, startDate: Long, finishDate: Long, private: Boolean) : Serializable`.

`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt:5-20`: same fields with `mangaId`, `lastChapterRead`, `totalChapters`.

### 4.2 Legacy DB-facing model (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt` is an interface with mutable `var` fields: `id: Long?`, `anime_id`, `tracker_id`, `remote_id`, `library_id: Long?`, `title`, `last_episode_seen: Double`, `total_episodes: Long`, `score: Double`, `status: Long`, `started_watching_date: Long`, `finished_watching_date: Long`, `tracking_url: String`, `private: Boolean`. `create(serviceId)` returns `AnimeTrackImpl` with `tracker_id` set. Manga is the same with `manga_id`, `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date`.

`copyPersonalFrom(other, copyRemotePrivate = true)` (anime: `AnimeTrack.kt:37-44`) copies `last_episode_seen`, `score`, `status`, `started_watching_date`, `finished_watching_date`, and `private` only when `copyRemotePrivate` is true. Manga is identical with the manga names. Trackers call it with `copyRemotePrivate = false` in some paths, for example `anilist/Anilist.kt:283`, `kitsu/Kitsu.kt:171`, `bangumi/Bangumi.kt:87`.

### 4.3 Domain-side extension functions (OBSERVED, `app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt`)

- `AnimeTrack.copyPersonalFrom(other: AnimeTrack): AnimeTrack` (lines 6-15). Domain version. Copies `lastEpisodeSeen`, `score`, `status`, `startDate`, `finishDate`, `private` (unconditionally; there is no `copyRemotePrivate` flag in the domain version). The difference from 4.2 is intentional-looking but must be preserved: the domain version always copies `private`.
- `AnimeTrack.toDbTrack(): DbAnimeTrack` (lines 17-31) = `DbAnimeTrack.create(trackerId)` and sets:
  `id` = `id`, `anime_id` = `animeId`, `remote_id` = `remoteId`, `library_id` = `libraryId`, `title`, `last_episode_seen` = `lastEpisodeSeen`, `total_episodes` = `totalEpisodes`, `status`, `score`, `tracking_url` = `remoteUrl`, `started_watching_date` = `startDate`, `finished_watching_date` = `finishDate`, `private`.
- `DbAnimeTrack.toDomainTrack(idRequired: Boolean = true): AnimeTrack?` (lines 33-51):
  `val trackId = id ?: if (!idRequired) -1 else return null`. Then builds the domain object. `trackerId` = `tracker_id.toLong()`.
  Note: `tracker_id` is already `Long` in the DB model. The `.toLong()` is a no-op, not a bug.
  Note: `id` is `Long?`. With `idRequired = false`, a null id becomes `-1`, which is the sentinel for "not yet inserted".

Manga equivalents in `app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt`: `copyPersonalFrom` (lines 6-15), `toDbTrack` (lines 17-31, uses `manga_id`, `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date`), `toDomainTrack` (lines 33-51). Here `trackerId = tracker_id` (no cast).

### 4.4 Mapping invariants (OBSERVED and Proposal)

- A round trip `DomainTrack -> DbTrack -> DomainTrack` is identity when `id != -1` (fields map one to one).
- With `idRequired = false`, a `-1` id survives the round trip. The repository then performs an insert, not an update (section 4.5).
- `copyPersonalFrom` is used by tracker update code to take the server's progress, score, status, and dates into the local object.

### 4.5 Repository write semantics (OBSERVED in area 06, relevant here)

`InsertAnimeTrack.await(track)` calls `AnimeTrackRepositoryImpl.insertAnime` which runs `anime_syncQueries.insert(...)` (`data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt:43-71`). The SQL (`data/src/main/sqldelightanime/dataanime/anime_sync.sq:42-44`) is `INSERT INTO anime_sync(anime_id,sync_id,remote_id,...) VALUES (...)`. **It does not include `_id`.** The table has `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE` (`anime_sync.sq:18`). So every write of an existing binding replaces the row and assigns a new `_id` (see section 22, defect D1). The manga table behaves the same (`manga_sync.sq:18, 42`).

`InsertAnimeTrack.await` and `awaitAll` catch `Exception`, log at ERROR, and return normally (`domain/.../InsertAnimeTrack.kt:12-25`). Failures in inserts are therefore silent to the caller.

---

## 5. Search models

### 5.1 `AnimeTrackSearch` (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt`)

`class AnimeTrackSearch : AnimeTrack` (DB interface) with overridden defaults:
- `id: Long? = null`, `anime_id: Long = 0`, `tracker_id: Long = 0`, `remote_id: Long = 0`, `library_id: Long? = null`.
- `title: String` declared `lateinit`.
- `last_episode_seen: Double = 0.0`, `total_episodes: Long = 0`, `score: Double = -1.0`, `status: Long = 0`.
- `started_watching_date: Long = 0`, `finished_watching_date: Long = 0`, `private: Boolean = false`.
- `tracking_url: String` declared `lateinit`.
- Extra fields (not in the DB interface): `authors: List<String> = emptyList()`, `artists: List<String> = emptyList()`, `cover_url: String = ""`, `summary: String = ""`, `publishing_status: String = ""`, `publishing_type: String = ""`, `start_date: String = ""` (a display string, not a timestamp).
- `equals`: same class and equal `anime_id`, `tracker_id` and `remote_id`.
- `hashCode`: `31`-based over `anime_id`, `tracker_id`, `remote_id` (lines 64-69).
- `companion create(serviceId)` sets `tracker_id` only (lines 71-75).

`MangaTrackSearch` (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt`) is identical with `manga_id`, `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date`.

Important: `title` and `tracking_url` are `lateinit`. Reading them before assignment throws `UninitializedPropertyAccessException`. Section 22, defect D9.

### 5.2 Use of search results

- Result equality (`==`) drives selection in the search list: `selected = it == selected` (`AnimeTrackerSearch.kt:220`). Equality ignores all display fields.
- Keys in the list use `it.hashCode()` (`AnimeTrackerSearch.kt:216`). Two results with the same `(0, tracker_id, remote_id)` triple collide. Section 22, defect D10.
- Result `remote_id` is set by the tracker's search implementation (outside part 1).

---

## 6. Bind (add tracking): AddAnimeTracks and AddMangaTracks

Sources: `app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt` and `.../manga/interactor/AddMangaTracks.kt`. The two files differ only in names (chapter vs episode, read vs seen) and in the history source. The anime steps are given in full; the manga differences are listed in 6.3.

### 6.1 `AddAnimeTracks.bind(tracker, item, animeId)` (OBSERVED, lines 32-79)

Wrapped in `withNonCancellableContext { withIOContext { ... } }`.

1. `allChapters = getEpisodesByAnimeId.await(animeId)` (all episodes of the entry, unfiltered).
2. `hasSeenEpisodes = allChapters.any { it.seen }`.
3. `tracker.bind(item, hasSeenEpisodes)`. The returned value is ignored. The tracker implementation mutates `item` in place (outside part 1).
4. `var track = item.toDomainTrack(idRequired = false) ?: return`. With `idRequired = false` the `?: return` is effectively dead (section 22, D12).
5. `insertTrack.await(track)`. This writes the binding. The row gets a new `_id` (section 4.5).
6. If `hasSeenEpisodes`:
   a. `latestLocalReadChapterNumber = allChapters.sortedBy { it.episodeNumber }.takeWhile { it.seen }.lastOrNull()?.episodeNumber ?: -1.0`. Note: this includes episodes with negative (unrecognized) numbers if they are seen. `takeWhile` is contiguous from the lowest number.
   b. If `latest > track.lastEpisodeSeen`: `track = track.copy(lastEpisodeSeen = latest)` and `tracker.setRemoteLastEpisodeSeen(track.toDbTrack(), latest.toInt())`. The `toInt()` truncates fractional numbers.
   c. If `track.startDate <= 0`:
      - `firstReadChapterDate = Injekt.get<GetAnimeHistory>().await(animeId).sortedBy { it.seenAt }.firstOrNull()?.seenAt`.
      - If not null: `startDate = firstReadChapterDate.time.convertEpochMillisZone(ZoneOffset.systemDefault(), ZoneOffset.UTC)`, `track = track.copy(startDate = startDate)`, and `tracker.setRemoteStartDate(track.toDbTrack(), startDate)`.
7. `syncChapterProgressWithTrack.await(animeId, track, tracker)` (section 8). For non-enhanced trackers this returns immediately.

Errors: any exception propagates out of `bind`. `AnimeTracker.register` catches `Throwable` and shows the raw message in a toast (section 6.4).

**Date conversion (OBSERVED):** `Long.convertEpochMillisZone(from, to)` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:29-37`) converts the instant to a local date-time in `from`, then re-interprets that wall-clock time in `to`. For `from = systemDefault` and `to = UTC`, a history timestamp at 2026-03-01 23:30 local becomes 2026-03-01 23:30 UTC. The calendar day is kept, the instant is shifted by the UTC offset. The remote start date therefore represents "the local calendar day, stored as UTC midnight-ish wall time".

### 6.2 `AddAnimeTracks.bindEnhancedTrackers(anime, source)` (OBSERVED, lines 81-107)

Wrapped in `withNonCancellableContext { withIOContext { ... } }`.

1. `trackerManager.loggedInTrackers()`, then `filterIsInstance<EnhancedAnimeTracker>()`, then `.filter { it.accept(source) }`.
2. For each service (sequential, registry order), inside `try`:
   a. `service.match(anime)?.let { track -> ... }`. `match` returns zero or one `AnimeTrackSearch` (enhanced trackers match by source-side identifiers, outside part 1). `null` means no match and nothing happens.
   b. `track.anime_id = anime.id`.
   c. `(service as Tracker).animeService.bind(track)`. This is `bind(track, hasSeenEpisodes = false)` (default parameter).
   d. `insertTrack.await(track.toDomainTrack(idRequired = false)!!)`.
   e. `syncChapterProgressWithTrack.await(anime.id, track.toDomainTrack(idRequired = false)!!, service.animeService)` (section 8, enhanced path runs).
3. `catch (e: Exception)`: `logcat(WARN, e) { "Could not match anime: ${anime.title} with service $service" }`. Nothing is shown to the user. The loop continues with the next service.

Caller points (OBSERVED): add to library in the anime screen (`ui/entries/anime/AnimeScreenModel.kt:450`), in the browse grid (`ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:240`), and in the history screen (`ui/history/anime/AnimeHistoryScreenModel.kt:220`). Manga counterparts: `ui/entries/manga/MangaScreenModel.kt:374` and `ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt:248`.

Important: `bindEnhancedTrackers` does not check whether the anime already has a binding for that tracker. Calling it again rebinds (the insert replaces by `(anime_id, sync_id)`). **Proposal:** skip a service that already has a binding for this entry.

### 6.3 Manga differences (OBSERVED, `AddMangaTracks.kt`)

- Uses `GetChaptersByMangaId`, `hasReadChapters = allChapters.any { it.read }`, `tracker.bind(item, hasReadChapters)`.
- The latest read number uses `.sortedBy { it.chapterNumber }.takeWhile { it.read }.lastOrNull()?.chapterNumber ?: -1.0`.
- The start-date source is `GetMangaHistory.await(mangaId).sortedBy { it.readAt }.firstOrNull()?.readAt`.
- `setRemoteLastChapterRead(track.toDbTrack(), latest.toInt())` and `setRemoteStartDate`.
- `syncChapterProgressWithTrack.await(mangaId, track, tracker)`.
- `bindEnhancedTrackers(manga, source)` uses `EnhancedMangaTracker`, `service.match(manga)`, `track.manga_id = manga.id`, `(service as Tracker).mangaService.bind(track)`, `service.mangaService`.

### 6.4 `register(item, animeId)` (OBSERVED, `AnimeTracker.kt:58-65`; manga `MangaTracker.kt:57-64`)

1. `item.anime_id = animeId`.
2. `try { addTracks.bind(this, item, animeId) }` (the bind in 6.1).
3. `catch (e: Throwable) { withUIContext { Injekt.get<Application>().toast(e.message) } }`. No logging. The toast shows the raw message. A null message shows nothing (the toast extension accepts a nullable message; treat null as empty).

Callers: the search screen's `registerTracking` (`ui/entries/anime/track/AnimeTrackInfoDialog.kt`, part 2) which calls `register` on the chosen result with `private` already set. Section 15.6.

---

## 7. Refresh (pull remote state)

### 7.1 `RefreshAnimeTracks.await(animeId): List<Pair<Tracker?, Throwable>>` (OBSERVED, `RefreshAnimeTracks.kt:25-45`)

Runs inside `supervisorScope`:
1. `getTracks.await(animeId)` (all bindings of the entry).
2. `.map { it to trackerManager.get(it.trackerId) }`.
3. `.filter { (_, service) -> service?.isLoggedIn == true }`. Bindings for trackers that are missing or logged out are skipped without error.
4. For each remaining binding, `async { ... }`:
   a. `updatedTrack = service!!.animeService.refresh(track.toDbTrack()).toDomainTrack()!!` (`idRequired = true`).
   b. `insertTrack.await(updatedTrack)`.
   c. `syncEpisodeProgressWithTrack.await(animeId, updatedTrack, service.animeService)`.
   d. Return `null` on success, or `service to e` on any `Throwable`.
5. `awaitAll()` then `filterNotNull()`. The result is the list of failures.

Notes:
- Parallel, one coroutine per binding. Failures do not cancel the others (`supervisorScope`).
- `Throwable` includes cancellation. A cancelled scope yields failures.
- Not wrapped in `withNonCancellableContext`. Leaving the screen can cancel it (OBSERVED design; **Proposal:** run refresh to completion once started, see section 19).
- Failure toasts are shown by the caller (section 13.6).

Manga: `RefreshMangaTracks.await(mangaId)` (`RefreshMangaTracks.kt:25-45`) is identical with `getTracks.await(mangaId)`, `service!!.mangaService.refresh(...)`, and `syncChapterProgressWithTrack.await(mangaId, updatedTrack, service.mangaService)`.

Who calls it (OBSERVED): the anime info sheet on open (`AnimeTrackInfoDialog.kt` part 2, `Model.init`), the anime entry screen before the mark-read check (`AnimeScreenModel.kt:877-889`), and the manga counterparts.

---

## 8. Progress sync (remote to local, local to remote)

### 8.1 `SyncEpisodeProgressWithTrack.await(animeId, remoteTrack, service: AnimeTracker)` (OBSERVED, `SyncEpisodeProgressWithTrack.kt:21-50`)

1. `if (service !is EnhancedAnimeTracker) return`. **For every non-enhanced tracker this function is a no-op.** This matters: a normal tracker's progress is pushed only by the setters in section 3, and local-to-remote continuity is not computed.
2. `sortedEpisodes = getEpisodesByAnimeId.await(animeId).sortedBy { it.episodeNumber }.filter { it.isRecognizedNumber }` (number `>= 0f`).
3. `episodeUpdates = sortedEpisodes.filter { e -> e.episodeNumber <= remoteTrack.lastEpisodeSeen && !e.seen }.map { it.copy(seen = true).toEpisodeUpdate() }`. Remote-ahead episodes become seen locally.
4. `localLastSeen = sortedEpisodes.takeWhile { it.seen }.lastOrNull()?.episodeNumber ?: 0F`. Only the continuous run from the lowest recognized number counts ("only take into account continuous watching").
5. `lastSeen = max(remoteTrack.lastEpisodeSeen, localLastSeen.toDouble())`. Never decreases the remote value.
6. `updatedTrack = remoteTrack.copy(lastEpisodeSeen = lastSeen)`.
7. `try { service.update(updatedTrack.toDbTrack()); updateEpisode.awaitAll(episodeUpdates); insertTrack.await(updatedTrack) } catch (e: Throwable) { logcat(WARN, e) }`.

Ordering (OBSERVED): remote update first, then local episodes, then the binding row. If the remote update throws, nothing local is written. `UpdateEpisode.awaitAll` catches its own `Exception` and logs (`domain/.../episode/interactor/UpdateEpisode.kt:20-27`), so DB failures there do not reach the `catch` here.

Note: `service.update(...)` is called with the default `didWatchEpisode = false`.

### 8.2 `SyncChapterProgressWithTrack.await(mangaId, remoteTrack, tracker: MangaTracker)` (OBSERVED, `SyncChapterProgressWithTrack.kt:21-50`)

Identical with `getChaptersByMangaId`, `chapter.read`, `chapterNumber`, `read = true`, `toChapterUpdate()`, `tracker.update(...)`, `updateChapter.awaitAll(...)`, `insertTrack.await(...)`.

### 8.3 Who calls the sync (OBSERVED)

- `AddAnimeTracks.bind` (6.1 step 7): always called, returns immediately for non-enhanced.
- `AddAnimeTracks.bindEnhancedTrackers` (6.2 step 2e): enhanced path.
- `RefreshAnimeTracks.await` (7.1 step 4c).

---

## 9. Progress triggers: TrackEpisode and TrackChapter

These push "I have now seen episode N" (or "read chapter N") to every logged-in tracker that is behind N. They are the only routes that call the tracker `refresh` before `update`.

### 9.1 `TrackEpisode.await(context, animeId, episodeNumber: Double, setupJobOnFailure = true)` (OBSERVED, `app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt:25-58`)

Wrapped in `withNonCancellableContext`.
1. `tracks = getTracks.await(animeId)`. If empty, return.
2. For each `track`, in list order: `service = trackerManager.get(track.trackerId)`. Skip (return null for this track, no work, no queue) when `service == null || !service.isLoggedIn || episodeNumber <= track.lastEpisodeSeen`.
3. For the rest, launch `async { runCatching { ... } }`:
   - If `context.isOnline()`:
     a. `updatedTrack = service.animeService.refresh(track.toDbTrack()).toDomainTrack(idRequired = true)!!.copy(lastEpisodeSeen = episodeNumber)`.
     b. `service.animeService.update(updatedTrack.toDbTrack(), true)` (`didWatchEpisode = true`).
     c. `insertTrack.await(updatedTrack)`.
     d. `delayedTrackingStore.removeAnimeItem(track.id)`.
   - Else (offline):
     a. `delayedTrackingStore.addAnime(track.id, episodeNumber)`.
     b. If `setupJobOnFailure`: `DelayedAnimeTrackingUpdateJob.setupTask(context)`.
4. `awaitAll()`, then `mapNotNull { it.exceptionOrNull() }`, then each failure is logged at INFO. Nothing is shown to the user.

Key differences from manga (9.2):
- **An online error is not queued** (the `runCatching` is outside all logic and there is no catch that queues). A failed online update is dropped silently.
- `refresh` is called before `update` so the remote state is merged first (the tracker's `refresh` returns a fresh object, and `copy(lastEpisodeSeen = episodeNumber)` then overrides the progress).
- `DelayedAnimeTrackingUpdateJob.setupTask` is enqueued from inside the coroutine, after the store write.

### 9.2 `TrackChapter.await(context, mangaId, chapterNumber: Double, setupJobOnFailure = true)` (OBSERVED, `app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt:24-60`)

Same structure with these differences:
- The filter is written as a nested duplicate `if` (lines 31-35), which is equivalent to a single test.
- The online path is inside `try`. On `Exception`, the code (a) calls `delayedTrackingStore.addManga(track.id, chapterNumber)`, (b) if `setupJobOnFailure` calls `DelayedMangaTrackingUpdateJob.setupTask(context)`, and (c) rethrows `e`. The rethrow is caught by `runCatching` and logged at INFO.
- So a failed online manga update **is** queued. An offline device queues through the same path, because `refresh` throws when there is no network (the manga path does not check `isOnline()`).

Behavioural consequence (OBSERVED): the manga path needs no connectivity check. Any failure of `refresh` or `update` (including HTTP 401 or server error) queues the chapter. The anime path queues only when `isOnline()` is false.

**Proposal:** unify on the manga rule (queue on any failure), plus the anime offline check to avoid a network call when the network is known to be down. The retry job then retries transient failures for everyone.

### 9.3 Call sites (OBSERVED)

| Call site | Condition before the call | Source |
|---|---|---|
| Reader: chapter completed | `!incognitoMode`, `trackPreferences.autoUpdateTrack()` true, manga not null. Calls `trackChapter.await(app, manga.id, chapter_number)` in `launchNonCancellable` | `ui/reader/ReaderViewModel.kt:918-929` |
| Player: episode completed | `!incognitoMode`, `hasTrackers`, `autoUpdateTrack()` true, anime not null. Calls `trackEpisode.await(app, anime.id, episode_number)` | `ui/player/PlayerViewModel.kt:1947-1958` |
| External player intent: episode seen | `autoUpdateTrack()` true. For each binding with a logged-in `AnimeTracker` and `episodeNumber > lastEpisodeSeen`, runs in `withIOContext`, calls `update` or queues with the anime id (defect D2) | `ui/player/ExternalIntents.kt:524-560` |
| Mark-as-read/watched in entry screen | Governed by `AutoTrackState`, not by `autoUpdateTrack()`; see section 11 | `ui/entries/manga/MangaScreenModel.kt:755-794`, `ui/entries/anime/AnimeScreenModel.kt:870-905` |

Note the reader and player use `chapter_number.toDouble()` and `episode_number.toDouble()` from the DB entity, which are `Float` in the domain. Float to Double conversion can produce `12.100000381…` for `12.1f`. Section 22, D7.

---

## 10. Delayed queue and retry job

The queue is the offline and failure buffer for progress updates. Two parallel implementations exist (anime and manga), both backed by one SharedPreferences file. Full design: `p1-topic-progress-queue.md`.

### 10.1 Store (OBSERVED)

`DelayedAnimeTrackingStore(context)` (`app/src/main/java/eu/kanade/domain/track/anime/store/DelayedAnimeTrackingStore.kt:8-44`):
- File: `context.getSharedPreferences("tracking_queue", Context.MODE_PRIVATE)` (line 13). **The same file is used by the manga store** (`DelayedMangaTrackingStore.kt:13`). Keys are the track id as a decimal string. Values are `Float`.
- `addAnime(trackId: Long, lastEpisodeSeen: Double)`:
  1. `previous = preferences.getFloat(trackId.toString(), 0f)`.
  2. If `lastEpisodeSeen > previous` (compared as `Double` vs `Float`): `putFloat(trackId.toString(), lastEpisodeSeen.toFloat())`. The write is `edit { }` (asynchronous apply in Android; see section 19).
  3. A debug log line is written when queuing.
  - Effect: the queue keeps only the highest pending value per track id. A lower value never overwrites a higher one. A new track id starts from `0f`, so any value above `0` is queued.
- `removeAnimeItem(trackId)`: `remove(trackId.toString())`.
- `getAnimeItems(): List<DelayedAnimeTrackingItem>`: maps every entry in `preferences.all` to `DelayedAnimeTrackingItem(trackId = key.toLong(), lastEpisodeSeen = value.toString().toFloat())`. **No kind filter.** Manga entries in the same file are returned as anime entries (defect D3).
- `DelayedAnimeTrackingItem(trackId: Long, lastEpisodeSeen: Float)` is a nested data class.

`DelayedMangaTrackingStore` (`app/src/main/java/eu/kanade/domain/track/manga/store/DelayedMangaTrackingStore.kt:8-44`) is identical with `addManga(trackId, lastChapterRead)`, `removeMangaItem`, `getMangaItems()` returning `DelayedTrackingItem(trackId, lastChapterRead: Float)`.

### 10.2 Job (OBSERVED)

`DelayedAnimeTrackingUpdateJob` (`app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt`) is a WorkManager `CoroutineWorker`.

`doWork()` (lines 26-60):
1. `if (runAttemptCount > 3) return Result.failure()`. Items stay in the store (the failure does not clear the queue).
2. Resolve `GetAnimeTracks` and `TrackEpisode` and the store from DI (`Injekt.get`).
3. In `withIOContext`:
   - `delayedTrackingStore.getAnimeItems().mapNotNull { item -> track = getTracks.awaitOne(item.trackId); if (track == null) removeAnimeItem(item.trackId); track?.copy(lastEpisodeSeen = item.lastEpisodeSeen.toDouble()) }`. `awaitOne(id)` looks up the binding by its **`_id`** (`anime_sync.sq:31-34`, `getTrackByAnimeId ... WHERE _id = :id`; the name is misleading).
   - `forEach { animeTrack -> logcat(DEBUG) ...; trackEpisode.await(context, animeTrack.animeId, animeTrack.lastEpisodeSeen, setupJobOnFailure = false) }`.
4. `return if (delayedTrackingStore.getAnimeItems().isEmpty()) Result.success() else Result.retry()`.

Note the job calls `TrackEpisode`, which applies its own skip rule (9.1 step 2). A queue entry whose binding is already at or past the queued value is **skipped by TrackEpisode and never removed** from the queue (defect D5). The job then returns `retry()` until the attempt limit.

Scheduling (`setupTask(context)`, lines 65-77):
- `Constraints(requiredNetworkType = NetworkType.CONNECTED)`.
- `OneTimeWorkRequestBuilder<...>().setConstraints(...).setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 5.minutes.toJavaDuration()).addTag(TAG).build()`.
- `context.workManager.enqueueUniqueWork(TAG, ExistingWorkPolicy.REPLACE, request)`. Unique name is `TAG` = `"DelayedAnimeTrackingUpdate"`.

Manga job (`DelayedMangaTrackingUpdateJob.kt`): identical, with `TAG = "DelayedMangaTrackingUpdate"`, `BackoffPolicy.EXPONENTIAL, 5, TimeUnit.MINUTES`, `getMangaItems()`, `getTracks.awaitOne(item.trackId)`, `track.copy(lastChapterRead = item.lastChapterRead.toDouble())`, and `trackChapter.await(context, track.mangaId, track.lastChapterRead, setupJobOnFailure = false)` (`DelayedMangaTrackingUpdateJob.kt:36-52`).

### 10.3 Timing and retry rules (OBSERVED from WorkManager usage; Android semantics)

- Attempt numbering: `runAttemptCount` starts at 0 for the first run. Body runs for counts 0 to 3 (four runs). The fifth start (count 4) returns failure without doing work.
- `Result.retry()` is returned when items remain. WorkManager then reschedules with exponential backoff starting at 5 minutes (`BackoffPolicy.EXPONENTIAL`). WorkManager's documented exponential formula is `delay = initial * 2^(attempt - 1)`, capped at 5 hours. Verify against the WorkManager version in use (the repository pins it).
- `ExistingWorkPolicy.REPLACE` cancels any pending job and enqueues a new one. Every `setupTask` call **resets** the attempt count and backoff. A burst of offline progress events therefore keeps postponing the retry.
- Constraint `CONNECTED` means the job runs only when some network is connected (WorkManager semantics, not internet reachability).

### 10.4 Queue lifecycle (summary, OBSERVED)

1. Enqueue: offline or failed progress update (9.1, 9.2) and external player (D2).
2. Process: job (10.2) once the network is connected, up to four runs.
3. Remove: only after a successful online `TrackEpisode`/`TrackChapter` (`removeAnimeItem(track.id)` at `TrackEpisode.kt:44`), or when `awaitOne(id)` returns null (`DelayedAnimeTrackingUpdateJob.kt:40-42`).
4. Never removed: entries whose binding is at or past the queued value (D5), entries for logged-out trackers (TrackEpisode skips them before any removal), and entries left after the attempt limit.
5. Not in backup: the SharedPreferences file is not part of the preferences backup (the backup reads `Preference`-prefixed keys in `app` preferences, not this file). Verify in area 21.

---

## 11. Mark-read policy (AutoTrackState)

### 11.1 Enum (OBSERVED, `app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt:6-10`)

`enum class AutoTrackState(val titleRes: StringResource) { ALWAYS(MR.strings.lock_always), ASK(MR.strings.default_category_summary), NEVER(MR.strings.lock_never) }`.

English labels: `ALWAYS` = "Always", `ASK` = "Always ask" (note: the ASK label reuses the key `default_category_summary`; it is not a lock key), `NEVER` = "Never". Source: `i18n/src/commonMain/moko-resources/base/strings.xml` for `lock_always`, `lock_never`, `default_category_summary`.

Persisted by enum name (`getEnum` serializer `it.name`, `PreferenceStore.kt:27-45`) under key `pref_auto_update_manga_on_mark_read`, default `ALWAYS`. Unknown stored strings fall back to the default (`enumValueOf` failure returns `defaultValue`).

### 11.2 Decision flow for mark read or watched (OBSERVED, manga lines 755-794; anime lines 870-905 are identical with episode names)

`markChaptersRead(chapters, read)` (`MangaScreenModel.kt:755`) and `markEpisodesSeen(episodes, seen)` (`AnimeScreenModel.kt:870`):

1. Toggle selection off. If `chapters` is empty, return.
2. In `launchIO`: `setReadStatus.await(read, chapters)` (the DB write, area 07).
3. If `!read` (or `!seen`), or `successState?.hasLoggedInTrackers == false`, or `autoTrackState == NEVER`: return.
4. `refreshTrackers()` (section 7; failures toasted with `track_error`, section 13.6).
5. `tracks = getTracks.await(mangaId)`. `maxChapterNumber = chapters.maxOf { it.chapterNumber }` (the largest number in the selection, `Float`).
6. `shouldPromptTrackingUpdate = tracks.any { maxChapterNumber > track.lastChapterRead }`. If false, return.
7. If `autoTrackState == ALWAYS`:
   a. `trackChapter.await(context, mangaId, maxChapterNumber)`.
   b. Toast `AYMR.strings.trackers_updated_summary_manga` with `maxChapterNumber.toInt()` (English "Trackers updated to chapter %d"; anime: `trackers_updated_summary_anime` "Trackers updated to episode %d").
   c. return.
8. Else (ASK): `snackbarHostState.showSnackbar(message = confirm_tracker_update(maxChapterNumber.toInt()), actionLabel = action_ok, duration = Short, withDismissAction = true)`. Message: "Update trackers to chapter %d?" (`i18n/.../strings.xml`, `confirm_tracker_update`); anime: `AYMR.strings.confirm_tracker_update_anime` "Update trackers to episode %d?".
9. If the result is `ActionPerformed`: `trackChapter.await(context, mangaId, maxChapterNumber)`. No success toast in the ASK path.

Notes (OBSERVED):
- The check uses `maxChapterNumber` of the selection, not of the entry. Marking chapter 3 read when the tracker is at 5 does not prompt.
- `toInt()` truncates. Marking chapter 12.5 shows "chapter 12".
- The snackbar duration `SnackbarDuration.Short` is the Compose constant. **Proposal:** 4000 ms (Compose's M3 Short value; verify against the Compose version used). Dismiss action present (an X).
- The `autoUpdateTrack()` preference (`pref_auto_update_manga_sync_key`) is **not** checked on this path. Only the reader and player check it (section 9.3). This asymmetry is observed; keep it unless the product decides otherwise.

### 11.3 `hasLoggedInTrackers` and `trackingCount` (OBSERVED, `MangaScreenModel.kt:1064-1090`, `AnimeScreenModel.kt:1494-1520`)

Combined flow:
- `getTracks.subscribe(entryId)` and `trackerManager.loggedInTrackersFlow()`.
- `supportedTrackers = loggedIn.filter { (it as? EnhancedMangaTracker)?.accept(source!!) ?: true }`. Non-enhanced logged-in trackers always count as supported.
- `supportedTrackerIds = supportedTrackers.map { it.id }.toHashSet()`.
- `supportedTrackerTracks = tracks.filter { it.trackerId in supportedTrackerIds }`.
- Emit `(supportedTrackerTracks.size, supportedTrackers.isNotEmpty())`, `distinctUntilChanged`, `collectLatest`.
- `hasLoggedInTrackers = supportedTrackers.isNotEmpty()`; `trackingCount = supportedTrackerTracks.size`.

### 11.4 Auto-open tracking sheet after add (OBSERVED)

`autoOpenTrack` (`MangaScreenModel.kt:158-159`; anime `AnimeScreenModel.kt:190`): `successState.trackingAvailable && trackPreferences.trackOnAddingToLibrary().get()`, where `trackingAvailable = trackingCount > 0` (`MangaScreenModel.kt:1198-1199`).

Effect: after adding to library, the track sheet opens only when the entry already has at least one supported binding, which happens after `bindEnhancedTrackers` matched something. A plain add with no enhanced match does not open the sheet even when the pref is on. The pref label says "Open track menu on adding to library" (section 12.2), so the behaviour is narrower than the label. Section 22, D13.

---

## 12. Settings screen (Settings > Tracking)

Source: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt`.

### 12.1 Screen shell (OBSERVED)

- Title: `MR.strings.pref_category_tracking` = "Tracking" (`i18n/.../strings.xml`).
- App-bar action: help icon (`Icons.Outlined.HelpOutline`, content description `MR.strings.tracking_guide` = "Tracking guide") that opens `https://aniyomi.org/help/guides/tracking/` (lines 78-86). **Drop (P3)**: the external guide belongs to the upstream project. **Proposal:** omit the icon, or point it to Kurobayomi's own documentation once it exists.
- The preference list is built by `getPreferences()` in this order (lines 138-246): switches, then the auto-update list, then the "Trackers" group, then the "Enhanced trackers" group.

### 12.2 Top preferences (OBSERVED)

| Order | Kind | Title (English) | Key | Default | Source line |
|---|---|---|---|---|---|
| 1 | Switch | "Update progress after reading/watching" (`AYMR.strings.pref_auto_update_manga_sync`) | `pref_auto_update_manga_sync_key` | `true` | lines 139-142 |
| 2 | Switch | "Open track menu on adding to library" (`AYMR.strings.pref_track_on_add_library`) | `track_on_adding_to_library` | `true` | 143-146 |
| 3 | Switch | "Show next episode's airing time " (`AYMR.strings.pref_show_next_episode_airing_time`, note trailing space) | `show_next_episode_airing_time` | `true` | 147-150 |
| 4 | List | "Update progress when marked as read/watched" (`AYMR.strings.pref_auto_update_manga_on_mark_read`) | `pref_auto_update_manga_on_mark_read` | `ALWAYS` | 151-157 |

Entries for item 4 are `AutoTrackState.entries` mapped to their labels in declaration order: Always, Always ask, Never.

### 12.3 Trackers group (OBSERVED, lines 158-223)

Title `MR.strings.services` = "Trackers". Rows in this exact order, each a `TrackerPreference` (logo, name, checkmark when logged in; `TrackingPreferenceWidget`, `PreferenceItem.kt:219-228`):
1. MyAnimeList: login opens `MyAnimeListApi.authUrl()` in the default browser (`openInBrowser(..., forceDefaultBrowser = true)`); logout opens the logout dialog.
2. AniList: login opens `AnilistApi.authUrl()`, same pattern.
3. Kitsu: login shows the credential dialog with label `MR.strings.email`.
4. MangaUpdates: login shows the credential dialog with label `MR.strings.username`.
5. Shikimori: login opens `ShikimoriApi.authUrl()`.
6. Simkl: login opens `SimklApi.authUrl()`.
7. Bangumi: login opens `BangumiApi.authUrl()`.
8. An `InfoPreference` with `MR.strings.tracking_info` = "One-way sync to update the chapter progress in external tracker services. Set up tracking for individual entries from their tracking button."

Row behaviour (`PreferenceItem.kt:219-228`): tapping a row when `isLoggedIn` is true opens the logout dialog; otherwise the login action runs. The check-mark icon is shown when logged in (tint `Color(0xFF4CAF50)`, content description `MR.strings.login_success`).

Note the logo icon for each row is `TrackLogoIcon(tracker)` (section 13.3).

### 12.4 Enhanced trackers group (OBSERVED, lines 116-136 and 224-245)

Partitioning (lines 116-127):
- `enhancedMangaTrackers = trackers.filter { it is EnhancedMangaTracker }.partition { accepted sources intersect installed catalogue sources }`. The first list is "available", the second is "not installed".
- `enhancedAnimeTrackers` likewise, using `animeSourceManager.getAll()` instead of catalogue sources.
- The "installed" check is `sourceManager.getCatalogueSources().any { it::class.qualifiedName in acceptedSources }` (manga) and `animeSourceManager.getAll().any { ... }` (anime).

Info text (lines 129-136): `MR.strings.enhanced_tracking_info` = "Provides enhanced features for specific sources. Entries are automatically tracked when added to your library." If any tracker is in the not-installed list, append `"\n\n" + MR.strings.enhanced_services_not_installed` with `%s` replaced by the names joined with `", "` (Kotlin `joinToString` default separator). English: "Available but source not installed: %s".

Group title `MR.strings.enhanced_services` = "Enhanced trackers". Rows: available manga enhanced trackers (login = `loginNoop()`, logout = `service.logout`), then available anime enhanced trackers, then the info preference. The "not installed" trackers have no row; their names appear only in the info text.

### 12.5 Login dialog (OBSERVED, `TrackingLoginDialog`, lines 250-347)

- Title: `MR.strings.login_title` with tracker name: "Log in to %1$s".
- Header right: close icon button (`Icons.Outlined.Close`, content description `MR.strings.action_close` = "Close").
- Field 1: outlined text field, label is the passed string resource (`MR.strings.email` "Email address" for Kitsu, `MR.strings.username` "Username" for MangaUpdates), `singleLine`, `ImeAction.Next`, initial value `tracker.getUsername()`. `isError = inputError && !processing`.
- Field 2: label `MR.strings.password` "Password", initial value `tracker.getPassword()`, trailing eye icon toggles visibility (initially hidden), `KeyboardType.Password`, `ImeAction.Done`, `singleLine`, `isError` as field 1.
- Confirm button: full width, enabled when `!processing && username.isNotBlank() && password.isNotBlank()`. Label: `MR.strings.loading` "Loading…" while processing, else `MR.strings.login` "Login". No separate cancel button; dismiss is the close icon or outside-tap.
- On click (background IO): `processing = true`, `checkLogin(...)`, `inputError = !result`, `if (result) onDismissRequest()`, `processing = false`.

`checkLogin` (lines 349-364):
- `tracker.login(username, password)`; on success `withUIContext { toast(MR.strings.login_success) }` "Logged in" and returns true.
- On `Throwable`: `tracker.logout()` (clears the stored credentials), then `toast(e.message.toString())` and returns false. `e.message.toString()` yields the literal text `"null"` when the message is null (defect D14).

Note: the trailing fields are not cleared on failure; the user can retry with the same text.

### 12.6 Logout dialog (OBSERVED, `TrackingLogoutDialog`, lines 366-406)

- Title: `MR.strings.logout_title` "Log out from %1$s?", centred, `fillMaxWidth`.
- Two buttons side by side, each weight 1: outlined "Cancel" (`MR.strings.action_cancel`) dismisses; filled "Log out" (`MR.strings.logout`, English "Log out") with error colours (container `colorScheme.error`, content `onError`).
- On "Log out": `tracker.logout()`, dismiss, toast `MR.strings.logout_success` "You are now logged out".

### 12.7 Dialog state (OBSERVED, lines 97-114)

A single `dialog: Any?` state holds either `LoginDialog(tracker, uNameStringRes)` or `LogoutDialog(tracker)`. Only one dialog at a time.

---

## 13. Track info sheet (per-entry tracking summary)

Shared composables: `AnimeTrackInfoDialogHome` (`presentation/track/anime/AnimeTrackInfoDialogHome.kt:56-132`) and `MangaTrackInfoDialogHome` (`presentation/track/manga/MangaTrackInfoDialogHome.kt:66-142`). The two are identical except for progress naming (episodes or chapters) and the status lookup. Host screens are in part 2.

### 13.1 Container (OBSERVED)

- Vertical column, `animateContentSize()`, full width, `verticalScroll`, padding 16 dp, `windowInsetsPadding(systemBars)`, `spacedBy(24.dp)` between items.
- One item per `trackItems` element, in the order given (the host orders them by the registry; section 13.6).

### 13.2 Item with a binding (`track != null`) (OBSERVED, lines 80-130 anime)

Per item:
- `supportsScoring = tracker.animeService.getScoreList().isNotEmpty()`.
- `supportsReadingDates = tracker.supportsReadingDates`.
- `supportsPrivate = tracker.supportsPrivateTracking`.
- Title row: logo icon (13.3), title, overflow menu (13.5).
- Detail card: status cell, progress cell, score cell (only if `supportsScoring`), then a second row with start and end date cells (only if `supportsReadingDates`).

Field values:
- `title` = `track.title`.
- `status` = `(tracker as? AnimeTracker)?.getStatusForAnime(track.status)` (`StringResource?`). Rendered as `stringResource(it)` or `""`.
- Progress text: `"${track.lastEpisodeSeen.toInt()}"`, and if `totalEpisodes > 0` then `"$progress / $totalEpisodes"` (lines 90-98). Manga uses `lastChapterRead` and `totalChapters`. Example: "12 / 24".
- `score` = `tracker.animeService.displayScore(track).takeIf { supportsScoring && track.score != 0.0 }`. A score of exactly `0.0` shows the placeholder.
- `startDate` text = `dateFormat.format(track.startDate.toLocalDate())`, shown only when `supportsReadingDates && track.startDate != 0L`. Placeholder when null: `MR.strings.track_started_reading_date` "Start date".
- `endDate` text = `dateFormat.format(track.finishDate.toLocalDate())`, shown only when `supportsReadingDates && track.finishDate != 0L`. Placeholder: `MR.strings.track_finished_reading_date` "Finish date".
- `private = track.private`; `onTogglePrivate` is provided only when `supportsPrivate`.
- `dateFormat` comes from `UiPreferences.dateFormat(...)` in the host (part 2). `toLocalDate()` uses the system time zone (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:39-41`).

Cell tap handlers: status → `onStatusClick`; progress → `onEpisodeClick` or `onChaptersClick`; score → `onScoreClick` (only if supported); start date → `onStartDateEdit` (only if reading dates); end date → `onEndDateEdit` (only if reading dates).

Note (OBSERVED): the anime sheet uses `MR.strings.track_started_reading_date` and `track_finished_reading_date` placeholders ("Start date", "Finish date"). These are generic strings, not anime-specific.

### 13.3 Logo icon `TrackLogoIcon` (OBSERVED, `presentation/track/components/TrackLogoIcon.kt:21-45`)

- Size 48 dp, background `Color(tracker.getLogoColor())` with `MaterialTheme.shapes.medium`, padding 4 dp, image `tracker.getLogo()`, content description `tracker.name`.
- With `onClick != null`: `clickableNoIndication(onClick, onLongClick)`. In the sheet: click = open the tracker page in the browser (`onOpenInBrowser`), long-press = copy the link (`onCopyLink`).
- Private badge (`BadgedBox`): when `private` is true, a `Badge` with `primary` container, `onPrimary` content, offset `x = -5 dp`, containing `Icons.Filled.VisibilityOff` 14 dp with content description `MR.strings.tracked_privately` "Tracked privately".

### 13.4 Title cell (OBSERVED, lines 183-203)

- Height 48 dp, weight 1, `combinedClickable(onClick = onNewSearch, onLongClick = copy title to clipboard)`, padding start 16 dp, content start-aligned, `titleMedium`, one line, ellipsis.
- Click re-opens the search screen (section 15) pre-filled with the title (host behaviour, part 2). Long-press copies the title (`copyToClipboard(title, title)`). `copyToClipboard` skips blank content and, on Android 12 and lower only, shows "Copied to clipboard:\n<content truncated to 50 chars>" (`app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:39-52`, string `copied_to_clipboard`).

### 13.5 Overflow menu `TrackInfoItemMenu` (OBSERVED, `MangaTrackInfoDialogHome.kt:326-388`)

Icon button (`Icons.Default.MoreVert`, content description `MR.strings.label_more` "More"), then a dropdown with these items in order:
1. `MR.strings.action_open_in_browser` "Open in browser".
2. `MR.strings.action_copy_link` "Copy link".
3. If `onTogglePrivate != null`: `MR.strings.action_toggle_private_off` "Track publicly" when `private` is true, else `MR.strings.action_toggle_private_on` "Track privately".
4. `MR.strings.action_remove` "Remove".

Each item closes the menu after running the action.

### 13.6 Empty item (`track == null`) (OBSERVED)

`TrackInfoItemEmpty` (anime lines 270-287): logo icon without click, then a text button "Add tracking" (`MR.strings.add_tracking`) with padding start 16 dp, weight 1. Tapping it calls `onNewSearch`.

Host behaviour (part 2, `AnimeTrackInfoDialog.kt:157-172`, `TrackerAnimeRemoveScreen`, `registerEnhancedTracking`): for an enhanced tracker the "Add tracking" action runs `registerEnhancedTracking` (match and register, error toast `MR.strings.error_no_match` "No match found"). For other trackers it opens the search screen.

### 13.7 Unset-text rendering (OBSERVED, `MangaTrackInfoDialogHome.kt:279-304`)

`TrackDetailsItem(text: String?, onClick, modifier, placeholder = "")`:
- Clickable, `fillMaxHeight`, padding 12 dp, centred.
- Text is `text ?: placeholder`, `bodyMedium`, centred, max 2 lines, ellipsis.
- Colour alpha is `0.5` when `text == null` (constant `UNSET_TEXT_ALPHA = 0.5F`), else `1.0`.

Note: a placeholder shown from `text == null` gets alpha 0.5. An empty string `""` (status with no label) gets alpha 1.0 and shows nothing.

### 13.8 Host refresh and load (OBSERVED, part 2 `AnimeTrackInfoDialog.kt:201-286`, manga equivalent)

- On open: `refreshTrackers()` (section 7) in a background coroutine. For each failed binding (`it.first != null`): log at ERROR and toast `MR.strings.track_error` with args `(tracker.name, e.message ?: "")`. English: "%1$s error: %2$s".
- Subscribes to the binding flow, `catch` logs, `distinctUntilChanged`, maps to items with `mapToTrackItem()`, then `collectLatest` updates state.
- `mapToTrackItem()`: `loggedInTrackers().filter { it is AnimeTracker }`, each mapped to `AnimeTrackItem(binding-or-null, tracker)`, then filter `(tracker as? EnhancedAnimeTracker)?.accept(source) ?: true`. The `source` is `AnimeSourceManager.getOrStub(sourceId)` (a stub when the source is not installed).
- `togglePrivate(item)`: `(item.tracker as? AnimeTracker)?.setRemotePrivate(item.track!!.toDbTrack(), !item.track.private)` in `launchNonCancellable`. No optimistic UI; the sheet updates when the DB flow emits.

---

## 14. Selectors (pickers)

Source: `app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt`. All are `AlertDialogContent` with title, body and a button row: optional third button (`thirdButton`), then Cancel (`MR.strings.action_cancel`), then OK (`MR.strings.action_ok` "OK"). Buttons are `TextButton`. The dialog is padded by system bars.

### 14.1 `BaseSelector` (OBSERVED, lines 196-233)

- Parameters: `title`, `content: BoxScope`, `onConfirm`, `onDismissRequest`, optional `thirdButton: RowScope`.
- Layout: title; body `Box(fillMaxWidth)`; buttons row with `spacedBy(MaterialTheme.padding.small, End)`. When `thirdButton != null` it is placed first, followed by a weight-1 spacer.

### 14.2 `TrackStatusSelector` (OBSERVED, lines 49-94)

- Title: `MR.strings.status` "Status".
- Body: a scrollable column of rows, one per `(key, labelRes)` in `selections` (a `Map<Long, StringResource?>`, in its iteration order).
- Each row: `clip(RoundedCornerShape(8.dp))`, `selectable(selected = key == selection, onClick = onSelectionChange(key))`, `fillMaxWidth`, `minimumInteractiveComponentSize`. Contains a `RadioButton` (not clickable itself) and the label (`bodyLarge`, start padding 24 dp). A null label renders empty text.
- Divider above when `canScrollBackward` and below when `canScrollForward`.
- Source of `selections` (OBSERVED, part 2 `AnimeTrackInfoDialog.kt:306-313`): `tracker.animeService.getStatusListAnime().associateWith { getStatusForAnime(it) }`. Manga uses `getStatusListManga()` and `getStatusForManga`.
- OK calls `setStatus()` which runs `setRemoteAnimeStatus(track, selection)` in `launchNonCancellable`, then pops.
- Initial selection: the current `track.status`.

Labels seen in the preview data (OBSERVED, preview only; real lists come from each tracker): Reading, Plan to read, Completed, On hold, Dropped, Rereading (manga); Watching, Plan to watch, Completed, On hold, Dropped, Rewatching (anime, AYMR "watching", "plan_to_watch"). These are tracker-supplied, so Kurobayomi should take the list from the tracker, not hard-code it.

### 14.3 `TrackItemSelector` (number wheel) (OBSERVED, lines 96-119)

- Title: `MR.strings.chapters` "Chapters" (manga) or `AYMR.strings.episodes` "Episodes" (anime) based on `isManga`.
- Body: `WheelNumberPicker(items = range.toImmutableList(), startIndex = selection, onSelectionChanged)`, centred.
- Range (part 2, `AnimeTrackInfoDialog.kt:371-385`): `endRange = if (track.totalEpisodes > 0) track.totalEpisodes else 10000`; `range = 0..endRange.toInt()`. Manga: `totalChapters`, same 10000 fallback.
- Selection starts at `track.lastEpisodeSeen.toInt()` (manga: `lastChapterRead.toInt()`).
- OK calls `setEpisode()` → `setRemoteLastEpisodeSeen(track, selection)` (section 3.3), then pops.

Note: the wheel is 0-based and the list includes the total. Choosing a value above the total is impossible when a total is known.

### 14.4 `TrackScoreSelector` (OBSERVED, lines 121-142)

- Title: `MR.strings.score` "Score".
- Body: `WheelTextPicker(items = selections, startIndex = selections.indexOf(selection).takeIf { it > 0 } ?: (selections.size / 2), onSelectionChanged = onSelectionChange(selections[it]))`.
- Initial selection (part 2): `displayScore(track)` (a string). Score list is `getScoreList()` (for example "0".."10" for the default list in `DummyTracker`; real lists are tracker-specific).
- Start index rule: `indexOf(selection)`; **when the index is 0 the rule falls back to the middle** (`takeIf { it > 0 }` excludes 0). When not found (`-1`) it also falls back to the middle. Section 22, D8.
- OK calls `setRemoteScore(track, selection)`, which calls `indexToScore(indexOf(selection))` (section 3.4).

### 14.5 `TrackDateSelector` (OBSERVED, lines 144-193)

- Title: the host-supplied string (`MR.strings.track_started_reading_date` "Start date" or `MR.strings.track_finished_reading_date` "Finish date").
- Body: `DatePicker(state = pickerState, title = null, headline = null, showModeToggle = false)`, `selectableDates` supplied by the host.
- Initial selection: `initialSelectedDateMillis` (UTC midnight millis, see 14.6).
- Buttons: if `onRemove != null`, a "Remove" text button (`MR.strings.action_remove` "Remove") at the start, then a weight-1 spacer; then Cancel and OK. OK calls `onConfirm(pickerState.selectedDateMillis!!)`.
- Buttons row padding: start 12, top 8, end 12, bottom 24 dp.

Host (part 2, `AnimeTrackInfoDialog.kt:459-551`):
- `initialSelection` = `(if start then startDate else finishDate).takeIf { it != 0L } ?: now`, converted from system zone to UTC using `convertEpochMillisZone(systemDefault, UTC)`.
- On confirm: `setDate(millis)` converts UTC millis back to local (`convertEpochMillisZone(UTC, systemDefault)`), then `setRemoteStartDate` or `setRemoteFinishDate` in `launchNonCancellable`.
- `canRemove` = `startDate > 0` (start) or `finishDate > 0` (finish). The Remove button shows only then.

### 14.6 Date constraints (OBSERVED, `AnimeTrackInfoDialog.kt:466-505`)

`SelectableDates` implementation with the rules, using the system zone:
- `isSelectableDate(utcMillis)`: `date = Instant.ofEpochMilli(utcMillis).atZone(systemDefault).toLocalDate()`.
  1. Future dates (`date > LocalDate.now()`) → false.
  2. If `start && finishDate > 0`: `date <= finishLocalDate`.
  3. Else if `!start && startDate > 0`: `date >= startLocalDate`.
  4. Else true.
- `isSelectableYear(year)`: same rules at year granularity. Future years false (`year > now.year`). Start: `year <= finishYear`; end: `year >= startYear`.

Note: `utcMillis` from the picker is UTC midnight of the chosen day. Converting it with the system zone can move it to the previous local day for zones west of UTC. **Proposal:** convert using the UTC date part (`Instant -> date in UTC`), then compare dates in local terms. Keep the 14.5 conversion pair so the stored value stays the same as today's app.

### 14.7 `TrackDateRemoverScreen` (remove confirmation) (OBSERVED, part 2 lines 591-657)

- `AlertDialogContent` with delete icon, title `MR.strings.track_remove_date_conf_title` "Remove date?", text `MR.strings.track_remove_start_date_conf_text` "This will remove your previously selected start date from %s" or `track_remove_finish_date_conf_text` (tracker name substituted).
- Buttons: Cancel (`action_cancel`), then a filled tonal button in error container colours with `action_remove` "Remove". On remove: `setRemoteStartDate(track, 0)` (or finish) in `launchNonCancellable`, then `navigator.popUntil { it is AnimeTrackInfoDialogHomeScreen }`.

---

## 15. Track search screen

Source: `presentation/track/anime/AnimeTrackerSearch.kt` (manga identical except types). Host (part 2): `TrackServiceSearchScreen` (`AnimeTrackInfoDialog.kt:677-760`).

### 15.1 Host model (OBSERVED, part 2 `AnimeTrackInfoDialog.kt:706-770`)

- `textFieldState = rememberTextFieldState(initialQuery)`, `initialQuery = track?.title ?: animeTitle` (when opened from a row; otherwise the entry title).
- `Model.init`: if `initialQuery.isNotBlank()` run `trackingSearch(initialQuery)`.
- `trackingSearch(query)`: set `queryResult = null` and `selected = null` (shows loading), then in `withIOContext`: `Result.success(tracker.animeService.searchAnime(query))`, or `Result.failure(e)` on `Throwable`. Then `selected = result.getOrNull()?.find { it.tracking_url == currentUrl }` (auto-selects the result already bound, if any).
- `onDispatchQuery` is wired to `trackingSearch(textFieldState.text.toString())`. **No blank check**: an empty query is sent to the tracker (D11).
- `supportsPrivateTracking = tracker.supportsPrivateTracking`.
- `registerTracking(item)`: `launchNonCancellable { tracker.animeService.register(item, animeId) }`.
- On confirm: `selected.private = private`, `registerTracking(selected)`, then pop.

### 15.2 Screen layout (OBSERVED, `AnimeTrackerSearch.kt:88-235`)

- Scaffold with top bar and bottom bar.
- Top bar: back arrow (`Icons.AutoMirrored.Outlined.ArrowBack`, `onSurfaceVariant`) that pops. Title: single-line `BasicTextField` with placeholder `MR.strings.action_search_hint` "Search…" when empty, IME action Search, `runOnEnterKeyPressed` and `onKeyboardAction` both run `onDispatchQuery()` then clear focus. Cursor colour primary. Actions: when text is non-empty, a close (X) icon button that clears the text and requests focus. A divider below.
- Bottom bar (`AnimatedVisibility(visible = selected != null)`, enter `fadeIn() + slideInVertically { it / 2 }`, exit `slideOutVertically { it / 2 } + fadeOut()`): row padding 8 dp, `windowInsetsPadding(navigationBars)`. Buttons: "Track" (`MR.strings.action_track` "Track", weight 1, elevated) → `onConfirmSelection(false)`; if `supportsPrivateTracking`, an elevated icon button (`Icons.Filled.VisibilityOff`, description `MR.strings.action_toggle_private_on` "Track privately") → `onConfirmSelection(true)`.

### 15.3 Body states (OBSERVED, lines 198-234)

| State | Condition | Rendering |
|---|---|---|
| Loading | `queryResult == null` | `LoadingScreen` (spinner) |
| Empty | result success, empty list | `EmptyScreen` with `MR.strings.no_results_found` "No results found" |
| Error | result failure | `EmptyScreen` with message `exceptionOrNull()?.message ?: MR.strings.unknown_error` ("Unknown error") |
| Results | success, non-empty | `ScrollbarLazyColumn`, content padding inner plus 12 dp vertical, spacing 12 dp, items keyed by `hashCode()` |

### 15.4 Result card (OBSERVED, lines 237-356)

- `Box` filling width, horizontal padding 12 dp, clipped with 16 dp rounded corners, background `colorScheme.surface`, border 2 dp in `outline` when selected else transparent. Inner padding 12 dp.
- Selected: a `CheckCircle` icon (primary tint) at top end.
- Tap: clear focus, then select. Long-press: opens a dropdown.
- Row: `ItemCover.Book(data = cover_url, modifier = height(96.dp))`, spacer 12 dp, then a column:
  - Title: `titleMedium`, up to 2 lines, ellipsis, end padding 28 dp (room for the check icon).
  - Authors and artists: `(authors + artists).distinct().joinToString()` (default separator ", "), shown only when either list is non-empty, `bodySmall`, secondary alpha, 1 line.
  - Detail lines (each `title: value`, title `titleSmall`, value `bodyMedium` secondary alpha, 1 line, shown only when non-blank):
    - `MR.strings.track_type` "Type" = `publishing_type` lowercased then capitalised.
    - `MR.strings.label_started` "Started" = `start_date`.
    - `MR.strings.track_status` "Status" = `publishing_status` lowercased then capitalised.
    - `MR.strings.score` "Score" = `score.toString()` when `score != -1.0`. A score such as `8.0` prints as "8.0".
- Description: `summary.trim()`, when non-blank: `bodySmall`, 4 lines max, ellipsis, `paddingFromBaseline(top = 24 dp)`, secondary alpha.
- Long-press menu (`SearchResultItemDropDownMenu`): "Copy to clipboard" (`MR.strings.action_copy_to_clipboard`) copies the title and closes the menu; "Open in browser" (`MR.strings.action_open_in_browser`) opens `tracking_url` in the browser when non-blank, and does **not** close the menu (D15).

Capitalisation (OBSERVED): `toLowerCase(Locale.current).capitalize(Locale.current)` lowercases the whole string and uppercases only the first character. "ONGOING" becomes "Ongoing". For anime the `publishing_type` "OVA" becomes "Ova".

### 15.5 Keyboard and focus (OBSERVED)

- Search field: Enter (hardware) and the IME Search key both dispatch a query and clear focus.
- Clear button: clears text and focuses the field.
- Result tap clears focus first, so the IME hides before the bottom bar appears.

### 15.6 Confirm (OBSERVED)

`onConfirmSelection(private)`: `selected = state.selected ?: return`, `selected.private = private`, `registerTracking(selected)`, `navigator.pop()`. The register call runs `launchNonCancellable` so leaving the screen does not cancel it.

---

## 16. Remove tracking (unregister)

Host dialog (part 2 `AnimeTrackInfoDialog.kt:772-865`):
- Title: `MR.strings.track_delete_title` "Remove %s tracking?" (tracker name).
- Text: `MR.strings.track_delete_text` "This will remove the tracking locally."
- Checkbox (shown only if `tracker is DeletableAnimeTracker`): `MR.strings.track_delete_remote_text` "Also remove from %s".
- Buttons: Cancel, then an error-container filled tonal button `MR.strings.action_ok` "OK".
- On OK: `deleteTrack.await(animeId, serviceId)` (local delete: `DELETE FROM anime_sync WHERE anime_id = :animeId AND sync_id = :syncId`, `anime_sync.sq:23-25`), then if the checkbox was ticked, `(tracker as DeletableAnimeTracker).delete(track)` in a try that logs ERROR on failure. No toast on remote failure.
- Then pop.

Ordering (OBSERVED): the local delete happens first, then the remote delete. A remote failure leaves a remote entry while the local binding is gone. **Proposal:** remote first, and keep the local row when the remote delete fails, or show an error. Section 22 D16.

---

## 17. Login and OAuth plumbing

### 17.1 Redirect routing (OBSERVED)

- Deep-link scheme and hosts (manifest, see area 53): `aniyomi://myanimelist-auth`, `aniyomi://anilist-auth`, `aniyomi://bangumi-auth`, `aniyomi://shikimori-auth`, `aniyomi://simkl-auth` (area 36/53 notes; `catalogs/android-api-mapping-*.md` lists the manifest lines).
- A separate activity, `TrackLoginActivity` extends `BaseOAuthLoginActivity` (part 2). It switches on `data.host`:
  - `"anilist-auth"` → `handleAnilist`, `"bangumi-auth"` → `handleBangumi`, `"myanimelist-auth"` → `handleMyAnimeList`, `"shikimori-auth"` → `handleShikimori`, `"simkl-auth"` → `handleSimkl` (`ui/setting/track/TrackLoginActivity.kt:9-17`).
  - Any other host: nothing. The activity keeps showing its loading screen (`BaseOAuthLoginActivity.kt:19-27`). Defect D17.

### 17.2 AniList (implicit grant) (OBSERVED, `TrackLoginActivity.kt:19-31`)

- Token is in the URL fragment. Regex `(?:access_token=)(.*?)(?:&)` is applied to `data.fragment.toString()`.
- If the regex matches a group: `aniList.login(token)` on IO, then return to settings.
- Else: `aniList.logout()`, then return to settings.
- Defect D18: the regex needs a trailing `&`. A fragment that ends with `access_token=...` (no following parameter) does not match and the user is logged out.

### 17.3 Code grant trackers (Bangumi, MyAnimeList, Shikimori, Simkl) (OBSERVED)

- `data.getQueryParameter("code")`. If non-null: `tracker.login(code)` on IO, then return to settings. If null: `tracker.logout()` then return. Simkl uses `data?.getQueryParameter("code")` (nullable `data`).
- The `login(code)` call is the tracker's own token exchange (tracker code, areas 28 to 33).

### 17.4 Return to settings (OBSERVED, `BaseOAuthLoginActivity.kt:29-36`)

`finish()`, then start `MainActivity` with `FLAG_ACTIVITY_CLEAR_TOP or FLAG_ACTIVITY_SINGLE_TOP`.

### 17.5 Credential storage (OBSERVED, `TrackPreferences.kt`)

Per tracker id `N`:
- `pref_mangasync_username_N` (private key, default `""`).
- `pref_mangasync_password_N` (private key, default `""`).
- `pref_tracker_auth_expired_N` (private key, Boolean, default `false`).
- `track_token_N` (private key, String, default `""`). Holds a JSON token object for OAuth trackers (AniList `ALOAuth`, Bangumi `BGMOAuth`, see `anilist/Anilist.kt:360-370`, `bangumi/Bangumi.kt:205-210`).

"Private key" means the stored key is prefixed `__PRIVATE_` (`core/common/.../Preference.kt:29-33, 48`). Backups skip private keys (`app/.../PreferenceBackupCreator.kt:82`). That is why passwords and tokens are absent from backup files. Area 21 describes the full backup behaviour.

Auth-expired flag: `MyAnimeList` reads it and sets it to `true` when its token is refused (`myanimelist/MyAnimeList.kt:269-273`). `setCredentials` resets it to `false`.

**Proposal:** store passwords and token JSON in the OS keyring (section 5 of `p1-rust-design.md`). Keep only non-secret flags (auth-expired) in the preference store.

### 17.6 Login-state flow (OBSERVED)

`isLoggedInFlow` (`BaseTracker.kt:36-43`) reacts to changes of the username and password preferences only. OAuth trackers write both through `saveCredentials` (section 17.5), so a token refresh that rewrites the password emits a new value. A token written to `track_token_N` alone does not emit.

---

## 18. Message keys (user-visible)

All keys resolve from `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) or `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR). English values are quoted. Placeholders are `%s`, `%d` or `%1$s`.

| Key | English | Where used (part 1) |
|---|---|---|
| `MR.strings.pref_category_tracking` | "Tracking" | settings title |
| `MR.strings.tracking_guide` | "Tracking guide" | help icon description (P3 drop) |
| `MR.strings.tracking_info` | "One-way sync to update the chapter progress in external tracker services. Set up tracking for individual entries from their tracking button." | info row (text says "chapter"; used for anime too) |
| `MR.strings.services` | "Trackers" | group title |
| `MR.strings.enhanced_services` | "Enhanced trackers" | group title |
| `MR.strings.enhanced_tracking_info` | "Provides enhanced features for specific sources. Entries are automatically tracked when added to your library." | info row |
| `MR.strings.enhanced_services_not_installed` | "Available but source not installed: %s" | info row suffix |
| `MR.strings.login_title` | "Log in to %1$s" | login dialog title |
| `MR.strings.email` | "Email address" | Kitsu field label |
| `MR.strings.username` | "Username" | MangaUpdates field label |
| `MR.strings.password` | "Password" | password label |
| `MR.strings.login` | "Login" | confirm button |
| `MR.strings.loading` | "Loading…" | confirm while processing |
| `MR.strings.login_success` | "Logged in" | toast; check-mark description |
| `MR.strings.logout_title` | "Log out from %1$s?" | logout dialog title |
| `MR.strings.logout` | "Log out" | logout confirm |
| `MR.strings.logout_success` | "You are now logged out" | toast |
| `MR.strings.action_close` | "Close" | close icon |
| `MR.strings.action_cancel` | "Cancel" | dialog buttons |
| `MR.strings.action_ok` | "OK" | dialog buttons, snackbar action |
| `MR.strings.action_remove` | "Remove" | date remove, menu |
| `MR.strings.lock_always` | "Always" | AutoTrackState.ALWAYS label |
| `MR.strings.default_category_summary` | "Always ask" | AutoTrackState.ASK label (key reused) |
| `MR.strings.lock_never` | "Never" | AutoTrackState.NEVER label |
| `AYMR.strings.pref_auto_update_manga_sync` | "Update progress after reading/watching" | switch |
| `AYMR.strings.pref_track_on_add_library` | "Open track menu on adding to library" | switch |
| `AYMR.strings.pref_show_next_episode_airing_time` | "Show next episode's airing time " | switch |
| `AYMR.strings.pref_auto_update_manga_on_mark_read` | "Update progress when marked as read/watched" | list |
| `MR.strings.track_error` | "%1$s error: %2$s" | refresh failure toast |
| `MR.strings.confirm_tracker_update` | "Update trackers to chapter %d?" | snackbar (manga) |
| `AYMR.strings.confirm_tracker_update_anime` | "Update trackers to episode %d?" | snackbar (anime) |
| `AYMR.strings.trackers_updated_summary_manga` | "Trackers updated to chapter %d" | toast (manga, ALWAYS) |
| `AYMR.strings.trackers_updated_summary_anime` | "Trackers updated to episode %d" | toast (anime, ALWAYS) |
| `MR.strings.action_search_hint` | "Search…" | search placeholder |
| `MR.strings.no_results_found` | "No results found" | empty search |
| `MR.strings.unknown_error` | "Unknown error" | search error fallback |
| `MR.strings.action_track` | "Track" | bottom bar |
| `MR.strings.action_toggle_private_on` | "Track privately" | private button and menu |
| `MR.strings.action_toggle_private_off` | "Track publicly" | menu |
| `MR.strings.tracked_privately` | "Tracked privately" | private badge description |
| `MR.strings.add_tracking` | "Add tracking" | empty row |
| `MR.strings.error_no_match` | "No match found" | enhanced match failure toast (part 2) |
| `MR.strings.action_open_in_browser` | "Open in browser" | menus |
| `MR.strings.action_copy_link` | "Copy link" | menu |
| `MR.strings.action_copy_to_clipboard` | "Copy to clipboard" | result menu |
| `MR.strings.label_more` | "More" | overflow description |
| `MR.strings.status` | "Status" | status selector title |
| `MR.strings.track_status` | "Status" | search detail label |
| `MR.strings.track_type` | "Type" | search detail label |
| `MR.strings.label_started` | "Started" | search detail label |
| `MR.strings.score` | "Score" | score title and label |
| `MR.strings.chapters` | "Chapters" | number wheel title (manga) |
| `AYMR.strings.episodes` | "Episodes" | number wheel title (anime) |
| `MR.strings.track_started_reading_date` | "Start date" | date placeholder and title |
| `MR.strings.track_finished_reading_date` | "Finish date" | date placeholder and title |
| `MR.strings.track_remove_date_conf_title` | "Remove date?" | date remover title |
| `MR.strings.track_remove_start_date_conf_text` | "This will remove your previously selected start date from %s" | date remover text |
| `MR.strings.track_remove_finish_date_conf_text` | "This will remove your previously selected finish date from %s" | date remover text |
| `MR.strings.track_delete_title` | "Remove %s tracking?" | remove dialog title |
| `MR.strings.track_delete_text` | "This will remove the tracking locally." | remove dialog text |
| `MR.strings.track_delete_remote_text` | "Also remove from %s" | remove checkbox |

Message keys used by part 2 but outside the part 1 files are listed here for completeness and because they share the same flows.

Toast texts from exceptions (OBSERVED, not localised): `register` failure, `updateRemote` failure, and the login failure text (`e.message.toString()`).

---

## 19. Concurrency and dispatchers

Wrappers (OBSERVED, `core/common/src/main/java/tachiyomi/core/common/util/lang/`):
- `withIOContext` = IO dispatcher.
- `withNonCancellableContext` = runs the block so that cancellation of the calling scope does not stop it midway.
- `withUIContext` = Main dispatcher, used to show toasts.
- `launchNonCancellable` and `launchIO` are used by screen models.

Per function:

| Function | Dispatcher and cancellation | Notes |
|---|---|---|
| `AnimeTracker.register` | caller; inner `addTracks.bind` is non-cancellable | Toast via UI context |
| `AddAnimeTracks.bind` | non-cancellable, IO | the whole bind finishes even if the screen is closed |
| `AddAnimeTracks.bindEnhancedTrackers` | non-cancellable, IO | sequential over services |
| `RefreshAnimeTracks.await` | caller, `supervisorScope` | cancellable (see 7.1 notes) |
| `TrackEpisode.await` / `TrackChapter.await` | non-cancellable; per-track `async` children | `awaitAll` waits for all |
| `updateRemote` | IO | `withUIContext` for toast |
| Queue store writes | caller; `edit {}` on SharedPreferences | asynchronous apply |
| Retry job | WorkManager, IO inside | runs after the constraint is met |
| Search query | `screenModelScope.launch` + IO | result replaces state |
| Settings login | `launchIO` | `processing` flag |

Thread-safety (OBSERVED): the queue store does read-modify-write (`getFloat` then `putFloat`) without a lock. Two concurrent `addAnime` calls for the same key can race (the later write can lower the stored value). Section 22, D6.

---

## 20. Timings, limits and constants

| Name | Value | Source |
|---|---|---|
| Retry job attempts | 4 body runs (runAttemptCount 0 to 3); the 5th start fails immediately | `DelayedAnimeTrackingUpdateJob.kt:27`, manga `:26` |
| Retry backoff | `EXPONENTIAL`, initial 5 minutes (anime `5.minutes`, manga `5, MINUTES`) | `:72`, manga `:65` |
| Retry constraint | `NetworkType.CONNECTED` | `:67`, manga `:60` |
| Unique work name | `"DelayedAnimeTrackingUpdate"`, `"DelayedMangaTrackingUpdate"` | `:63`, manga `:56` |
| Work policy | `ExistingWorkPolicy.REPLACE` | `:76`, manga `:69` |
| Queue file | `tracking_queue` (shared by both kinds) | `DelayedAnimeTrackingStore.kt:13`, manga `:13` |
| Queue value type | `Float` (single precision) | `DelayedAnimeTrackingStore.kt:20` |
| Unset score sentinel (search) | `-1.0` | `AnimeTrackSearch.kt:25` |
| Unset score sentinel (info) | `0.0` (shown as placeholder) | `AnimeTrackInfoDialogHome.kt:101` |
| Unset text alpha | `0.5F` | `MangaTrackInfoDialogHome.kt:279` |
| Unset start/finish date | `0L` | `AnimeTrackInfoDialogHome.kt:109, 113` |
| Wheel range without total | `0..10000` | `AnimeTrackInfoDialog.kt:371-385` |
| Snackbar duration | `SnackbarDuration.Short` | `MangaScreenModel.kt:772-781` |
| Search results padding | 12 dp vertical, 12 dp spacing, 12 dp horizontal card margin | `AnimeTrackerSearch.kt:211-255` |
| Search card corner | 16 dp | `AnimeTrackerSearch.kt:249` |
| Search card border | 2 dp | `AnimeTrackerSearch.kt:258-262` |
| Search cover height | 96 dp (book ratio) | `AnimeTrackerSearch.kt:284` |
| Description lines | 4 max | `AnimeTrackerSearch.kt:349` |
| Logo tile | 48 dp, padding 4 dp | `TrackLogoIcon.kt:35-37` |
| Private badge icon | 14 dp, offset x -5 dp | `AnimeTrackInfoDialogHome.kt:166, 181` |
| Check icon in settings | 32 dp, padding 4 dp, colour `#FF4CAF50` | `TrackingPreferenceWidget.kt:53-58` |
| Date-selector button padding | 12, 8, 12, 24 dp | `TrackInfoDialogSelector.kt:168-171` |

---

## 21. Cross-area interactions

- **Area 06 (track repository):** `anime_sync` and `manga_sync` tables, `UNIQUE (…, sync_id) ON CONFLICT REPLACE`, the `_id`-less insert, the `getTrackByAnimeId` name bug. Section 4.5 and D1.
- **Area 02 (preferences):** `Preference.privateKey`, `getEnum`, `getString`, `getBoolean` with the defaults in section 12.2 and 17.5.
- **Area 05, 07 (entries and items):** `GetEpisodesByAnimeId`, `GetChaptersByMangaId`, `UpdateEpisode.awaitAll`, `UpdateChapter.awaitAll`, `isRecognizedNumber`.
- **Area 08 and 09 (history, domain track):** `GetAnimeHistory`, `GetMangaHistory`, the domain interactors `GetAnimeTracks`, `InsertAnimeTrack`, `DeleteAnimeTrack` (`domain/src/main/java/tachiyomi/domain/track/anime/interactor/*.kt`).
- **Area 12 (network):** `NetworkHelper.client` for every tracker.
- **Area 14, 16, 18, 19 (sources):** `AnimeSourceManager.getOrStub`, `MangaSourceManager`, `EnhancedTracker.accept(source)` checks `source::class.qualifiedName` against `getAcceptedSources()`.
- **Area 21, 22 (backup):** private keys are skipped by `PreferenceBackupCreator`; the track rows are backed up through `AnimeBackupCreator`/`MangaBackupCreator`, which call `anime_syncQueries` (`data/backup/create/creators/AnimeBackupCreator.kt`). The delayed queue is not in backup.
- **Area 23, 24 (downloads):** none beyond entries.
- **Area 36 (shell and navigation):** deep links, `TrackLoginActivity`.
- **Area 37 to 41 (library and entry UI):** `AnimeScreenModel`, `MangaScreenModel`, `ReaderViewModel`, `PlayerViewModel`, `ExternalIntents` call the functions in this document.
- **Area 49 (theme):** colours in the cards, `MaterialTheme` roles mapped in `p1-ui-parity.md`.
- **Area 51 (i18n):** string resources.
- **Area 53 (platform):** WorkManager replacement, `openInBrowser`, `copyToClipboard`, intents.

---

## 22. Observed defects (verified in the source; Kurobayomi must not copy them)

Each defect has a test in `p1-edge-cases-and-tests.md`.

- **D1 Track row identity changes on every write.** `anime_sync.sq:42-44` and `manga_sync.sq:41-43` insert without `_id`, with `UNIQUE (…, sync_id) ON CONFLICT REPLACE`. SQLite assigns `max(_id)+1` after the conflicting row is deleted, so `_id` is kept only when that row was the largest id. Every other write changes it. The delayed queue is keyed by `_id`, so queued work can point at a stale id. **Fix:** upsert that keeps `_id`.
- **D2 External player queues by anime id.** `ExternalIntents.kt:549` calls `delayedTrackingStore.addAnime(track.animeId, …)`. The store key is the track id (`_id`). The job then looks up the wrong binding or none. **Fix:** pass `track.id`.
- **D3 Shared queue file with no kind namespace.** Both stores use `tracking_queue` (`DelayedAnimeTrackingStore.kt:13`, `DelayedMangaTrackingStore.kt:13`). Keys are bare ids. An anime track id and a manga track id with the same number overwrite each other, and `getAnimeItems()` returns manga keys as anime items. **Fix:** separate tables or key prefixes.
- **D4 Retry queue is never cleared on the attempt limit.** After the fifth start returns `failure()`, entries stay in the store, and the next `setupTask` restarts the cycle.
- **D5 Stale queue entries are never removed.** `TrackEpisode` skips a binding with `episodeNumber <= lastEpisodeSeen` (`TrackEpisode.kt:32`) before any removal. The job then keeps retrying. Same in manga (`TrackChapter.kt:31`).
- **D6 Non-atomic read-modify-write in the queue store.** `addAnime` reads then writes without a lock. Concurrent adds can lower the stored value.
- **D7 Float to Double conversion of progress.** Reader and player pass `chapter_number.toDouble()` from a `Float` (`ReaderViewModel.kt:926`, `PlayerViewModel.kt:1957`). `12.1f.toDouble()` is `12.100000381469727`. The remote comparison `episodeNumber <= lastEpisodeSeen` then uses the imprecise value. **Fix:** store `f64` end to end or round to the source's precision.
- **D8 Score wheel start index.** `takeIf { it > 0 }` (`TrackInfoDialogSelector.kt:135`) treats index 0 as "not found" and starts from the middle.
- **D9 `lateinit` search fields.** `title` and `tracking_url` in search models (`AnimeTrackSearch.kt:19, 35`) throw if a tracker does not set them.
- **D10 Search keys collide.** `hashCode` ignores display fields and uses `anime_id` which is 0 for search results (`AnimeTrackSearch.kt:64-69`).
- **D11 Blank search query is sent.** No `isBlank` guard in `trackingSearch`.
- **D12 Dead `?: return`.** `AddAnimeTracks.kt:38` and manga line 38 can only return when `idRequired` is true, and it is passed `false`. Harmless but misleading.
- **D13 Auto-open after add requires an existing binding.** Section 11.4. The label suggests it always opens.
- **D14 "null" toast text.** `e.message.toString()` (`SettingsTrackingScreen.kt:361`).
- **D15 Menu stays open after "Open in browser".** `SearchResultItemDropDownMenu` (`AnimeTrackerSearch.kt:369-381`).
- **D16 Local delete before remote delete.** Section 16.
- **D17 OAuth activity hangs on an unknown host.** `TrackLoginActivity.kt:9-17` has no else branch and no finish call.
- **D18 AniList regex needs a trailing `&`.** `TrackLoginActivity.kt:20`.
- **D19 Manga refresh and insert and sync write the row through a stale object.** `setRemote*` mutate a DB copy, then `AddAnimeTracks.bind` continues with a domain copy that does not contain those mutations (`AddAnimeTracks.kt:52-72` versus the `toDbTrack()` call at `:55`, `:72`). For enhanced trackers, `SyncEpisodeProgressWithTrack` then writes the stale `status`, `startDate` and `finishDate` back (`:46`). Verify with a test.
- **D20 Anime start date uses history `seenAt` nulls first.** `sortedBy { it.seenAt }` on nullable values puts nulls first, so a history row with null `seenAt` yields no start date (`AddAnimeTracks.kt:59-62`).
