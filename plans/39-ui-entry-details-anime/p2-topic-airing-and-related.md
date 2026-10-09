# Topic: airing countdown, viewer-flag codec, and related anime

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt` (lines 175, 1461-1486, 1492-1543, 1602-1671 for the state fields and getters; `updateAiringTime` 1535-1543, `syncRelatedAnime` 1472-1486).

Callees outside this scope:
- `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt` (165 lines, all read). **Owned by area:52-util-app, not by this part.** It is specified here only as far as this screen consumes it. Rust port: task T-39b-036 (consumer contract) and the owner's task for the provider itself.
- `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt` (59 lines, all read).
- `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt` (viewer-flag constants lines 310-313 and getters 73-83).
- `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt` (44 lines, all read).
- `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetRelatedAnime.kt` (not opened; name and `subscribe(animeId)` signature used at AnimeScreenModel.kt:1463).
- `data/src/main/sqldelightanime/dataanime/anime_relations.sq` (queries `getRelatedAnimeByAnimeId`, `getLastFetchedAt`).

Repo-relative paths; root `/home/blackbox/aniyomi-src`.

---

## 1. Viewer flags (`animes.viewer` column, Long)

The column is named `viewer` in the DB (animes.sq:23) and mapped to `viewerFlags` in the domain model (Anime.kt:24). Layout, from Anime.kt:310-313:

| Field | Mask | Bits | Width | Read | Written |
|---|---|---|---|---|---|
| skip-intro length (seconds) | `ANIME_INTRO_MASK = 0x0000000000000FF` | 0-7 | 8 bits, 0..255 | `(viewerFlags and ANIME_INTRO_MASK).toInt()` (Anime.kt:73-74) | `setFlag(flag, mask)` (SetAnimeViewerFlags.kt:19) |
| next episode to air (number) | `ANIME_AIRING_EPISODE_MASK = 0x000000000FFFF00` | 8-23 | 16 bits | `(viewerFlags and mask).removeHexZeros(2).toInt()` (Anime.kt:79-80) = `/ 256` | `episode.toLong().addHexZeros(2)` = `* 256` (SetAnimeViewerFlags.kt:27, 36) |
| next airing time (epoch seconds) | `ANIME_AIRING_TIME_MASK = 0x0FFFFFFFF000000` | 24-55 | 32 bits | `(viewerFlags and mask).removeHexZeros(6)` = `/ 16^6` (Anime.kt:82-83) | `airingAt.addHexZeros(6)` = `* 16^6` (SetAnimeViewerFlags.kt:28, 46) |
| skip-intro disabled | `ANIME_INTRO_DISABLE_MASK = 0x100000000000000` | 56 | 1 bit | `(viewerFlags and mask) == mask` (Anime.kt:76-77) | `(flag == 0L).toLong().addHexZeros(14)` = `1 * 16^14 = 2^56` when length is 0 (SetAnimeViewerFlags.kt:21) |

Write helper (SetAnimeViewerFlags.kt:51-53): `this and mask.inv() or (flag and mask)`. Every write reads the current row (`animeRepository.getAnimeById(id)`) and then writes only its own field. Two writes in a row (airing episode then airing time) therefore each read the row again (SetAnimeViewerFlags.kt:41-49).

**Race (edge E-AIR-1):** `awaitSetNextEpisodeAiring` performs two read-modify-write steps (lines 31-49). Concurrent writers to `viewerFlags` (for example the skip-intro setter) can lose an update. Proposal: one atomic SQL `UPDATE ... SET viewer = (viewer & ~mask) | value` per field, or a single transaction.

### 1.1 Skip intro setter (`awaitSetSkipIntroLength`, SetAnimeViewerFlags.kt:13-24)

- Writes length `flag` (0..255) into bits 0-7.
- Sets bit 56 when length == 0 and clears it otherwise. A length of 0 therefore disables the skip button for that anime (the comment in the source: "Disable skip intro button if length is set to 0").
- Called from `AnimeScreen.kt:371`: `screenModel.setAnimeViewerFlags.awaitSetSkipIntroLength(animeId, newLength)`. Dialog: `SkipIntroLengthDialog` (presentation part 1).

### 1.2 Default skip intro length

- `GesturePreferences.defaultIntroLength()` = `preferenceStore.getInt("pref_default_intro_length", 85)` (app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt:21).
- The dialog initial value (AnimeScreen.kt:375-381): `if (!anime.skipIntroDisable && anime.skipIntroLength == 0) defaultIntroLength else anime.skipIntroLength`.
- Edge E-AIR-2: when the anime's stored length is 0 and skip is not disabled, the dialog shows the default (85) even though the stored value is 0. Pressing OK then stores the chosen value. Proposal: keep (0 means "use default" for the dialog), document it.

### 1.3 Airing setter (`awaitSetNextEpisodeAiring`, SetAnimeViewerFlags.kt:26-29)

`(episodeNumber: Int, airingAtSeconds: Long)`. A returned pair `Pair(1, 0L)` (no information) is written as-is.

---

## 2. Airing countdown (screen-level flow)

### 2.1 Where it is triggered

1. `observeTrackers()` second collector (AnimeScreenModel.kt:1520-1532): every change of the logged-in tracker list or of the anime's track rows calls `updateAiringTime(anime, trackItems, manualFetch = false)`. Runs at screen open and on each tracker change.
2. `fetchAllFromSource()` public (285-297): after every manual refresh, `updateAiringTime(it.anime, it.trackItems, manualFetch)`. **`it.trackItems` is always the empty list (defect A-1 below).**

### 2.2 `updateAiringTime` (1535-1543)

```
airingEpisodeData = AniChartApi().loadAiringTime(anime, trackItems, manualFetch)
setAnimeViewerFlags.awaitSetNextEpisodeAiring(anime.id, airingEpisodeData)
updateSuccessState { it.copy(nextAiringEpisode = airingEpisodeData) }
```

`AniChartApi()` is constructed per call (its own `OkHttpClient`, AniChartApi.kt:20). No connection reuse across calls.

### 2.3 `loadAiringTime(anime, trackItems, manualFetch)` (AniChartApi.kt:22-49), exact

```
var airingEpisodeData = Pair(anime.nextEpisodeToAir, anime.nextEpisodeAiringAt)
if (anime.status == SAnime.COMPLETED.toLong() && !manualFetch) return airingEpisodeData

return withIOContext {
    val matchingTrackItem = trackItems.firstOrNull {
        (it.tracker is Anilist && it.track != null) ||
        (it.tracker is MyAnimeList && it.track != null) ||
        (it.tracker is Simkl && it.track != null)
    } ?: return@withIOContext Pair(1, 0L)

    matchingTrackItem.let { item ->
        item.track!!.let {
            airingEpisodeData = when (item.tracker) {
                is Anilist -> getAnilistAiringEpisodeData(it.remoteId)
                is MyAnimeList -> getAnilistAiringEpisodeData(getAlIdFromMal(it.remoteId))
                is Simkl -> getSimklAiringEpisodeData(it.remoteId)
                else -> Pair(1, 0L)
            }
        }
    }
    return@withIOContext airingEpisodeData
}
```

Rules, in order:
1. Completed anime with `manualFetch == false`: keep the stored values, no network.
2. Otherwise, pick the **first** tracker item (in the order of `trackItems`) that is AniList, MyAnimeList or Simkl and has a bound track. Kitsu, Shikimori, Bangumi and the other trackers never provide airing data.
3. No such item: `Pair(1, 0L)`. This is written to the DB (defect A-1 makes this the common outcome on manual refresh).
4. AniList: `getAnilistAiringEpisodeData(track.remoteId)`.
5. MyAnimeList: `getAnilistAiringEpisodeData(getAlIdFromMal(track.remoteId))`.
6. Simkl: `getSimklAiringEpisodeData(track.remoteId)`. The Simkl `remoteId` is the Simkl id.

Note: `AniChartApi` checks `is Anilist`, `is MyAnimeList`, `is Simkl` on the `Tracker` object. In the scope, `trackItems` is `AnimeTrackItem(track, tracker: Tracker)`.

### 2.4 `getAlIdFromMal(idMal: Long): Long` (AniChartApi.kt:51-76)

- Request: `POST https://graphql.anilist.co`, body JSON `{"query": "query { Media(idMal:<idMal>,type: ANIME) { id } }"}` (the Kotlin string is a raw multi-line string; the query semantics are as shown). Content type `jsonMime` (`eu.kanade.tachiyomi.network.jsonMime`, the JSON media type).
- Any exception on the call: returns `0L`.
- Parse: `response.body.string().substringAfter("id\":").substringBefore("}").toLongOrNull() ?: 0L`. HTTP status is not checked. A `Media: null` response gives `0L` (the search fails silently).

### 2.5 `getAnilistAiringEpisodeData(id: Long): Pair<Int, Long>` (AniChartApi.kt:78-107)

- Request: `POST https://graphql.anilist.co`, body `{"query": "query { Media(id:<id>) { nextAiringEpisode { episode airingAt } } }"}`.
- Any exception on the call: `Pair(1, 0L)`.
- Parse: `episodeNumber = data.substringAfter("episode\":").substringBefore(",").toIntOrNull() ?: 1`; `airingAt = data.substringAfter("airingAt\":").substringBefore("}").toLongOrNull() ?: 0L`.
- Observed live response for an anime with no airing episode (checked read-only with `curl`, Media id 1): `{"data":{"Media":{"nextAiringEpisode":null}}}`. Both parses then fail and the result is `Pair(1, 0L)`.
- `airingAt` is epoch **seconds** (AniList semantics), stored as seconds in `viewerFlags`, multiplied by 1000 for the countdown (Success.airingTime, AnimeScreenModel.kt:1668-1671).

**Proposal (Rust):** parse with `serde_json` into `{"data":{"Media":{"nextAiringEpisode":{"episode":u32,"airingAt":i64}|null}}}`. `null` gives `(1, 0)`, matching the observed behavior.

### 2.6 `getSimklAiringEpisodeData(id: Long): Pair<Int, Long>` (AniChartApi.kt:109-148)

Start values: `episodeNumber = 1`, `airingAt = 0L`.

For each `calendarType` in `["anime", "tv", "movie_release"]` (in this order):
1. `GET https://data.simkl.in/calendar/<calendarType>.json` (no auth, no headers beyond defaults). On exception: return `Pair(1, 0L)` immediately (the whole function, not just this type).
2. `data = removeAiredSimkl(body)` (see 2.7).
3. `malId = data.substringAfter("\"simkl_id\":$id,", "").substringAfter("\"mal\":\"").substringBefore("\"").toLongOrNull() ?: 0L`.
4. If `malId != 0L`: return `getAnilistAiringEpisodeData(getAlIdFromMal(malId))` (the AniList path, returns immediately even when the MAL lookup fails with `0`).
5. `epNum = data.substringAfter("\"simkl_id\":$id,", "").substringBefore("\"}}").substringAfterLast("\"episode\":")`; `episodeNumber = epNum.substringBefore(",").toIntOrNull() ?: episodeNumber`.
6. `date = data.substringBefore("\"simkl_id\":$id,", "").substringAfterLast("\"date\":\"").substringBefore("\"")`; `airingAt = if (date.isNotBlank()) toUnixTimestamp(date) else airingAt`.
7. If `airingAt != 0L`: return `Pair(episodeNumber, airingAt)` (first calendar type with a date wins).
After the loop: `Pair(episodeNumber, airingAt)`.

Live schema (read-only sample, `https://data.simkl.in/calendar/anime.json`, first object, abbreviated):
```
{"title":"...","poster":"...","date":"2026-10-07T00:00:00+09:00","release_date":"...","rank":4762,
 "ratings":{"simkl":{"rating":8.5,"votes":4}},"url":"...",
 "ids":{"simkl_id":3194924,"slug":"...","tmdb":null,"mal":"62248"},
 "episode":{"episode":14,"url":"..."},"anime_type":"ona"}
```
The array is in source order; `date` is ISO 8601 with a numeric offset. `ids.mal` is a **string**, may be absent or null.

**Proposal (Rust):** deserialize the array with `serde_json` into `Vec<CalendarItem { date: DateTime<FixedOffset>, ids: { simkl_id: i64, mal: Option<String> }, episode: { episode: u32 } }>`, then for each calendar type: take the **earliest** item with `ids.simkl_id == id` and `date > now`. If its `mal` parses to a non-zero id use the AniList path (step 4), else `(episode, date.timestamp())`. An item with no upcoming date gives no airing data for that type. This replaces the substring logic; the output is the same for well-formed feeds.

### 2.7 `removeAiredSimkl(body)` (AniChartApi.kt:150-158) and the offset defect

```
currentTimeInMillis = now
index = body.split("\"date\":\"").drop(1).indexOfFirst {
    date = it.substringBefore("\"")
    time = if (date.isNotBlank()) toUnixTimestamp(date) else 0L
    time * 1000 > currentTimeInMillis
}
return if (index >= 0) body.substring(index) else ""
```

- `index` is the position in the **split list** of fragments (one fragment per `"date":"` occurrence), but it is used as a **character offset** into `body`. **Defect A-2:** the returned substring starts at an arbitrary character, not at the first upcoming entry. For a typical feed the list index is small, so the start is near the top of the file and the later `substringAfter` searches usually still find the id, which hides the bug. For an id that appears before character `index`, the search returns nothing and the result is `Pair(1, 0L)` for that type.
- `toUnixTimestamp` (AniChartApi.kt:160-164) is `OffsetDateTime.parse(s).toInstant().epochSecond`. It is called for every date in the feed and throws `DateTimeParseException` on a malformed string. The exception is **not caught** in `removeAiredSimkl`, `getSimklAiringEpisodeData`, or `loadAiringTime`. **Defect A-3:** one malformed date in the Simkl feed throws out of `loadAiringTime`. In `observeTrackers` this is inside `collectLatest` in a `launchIO` (AnimeScreenModel.kt:1529-1531), so the coroutine fails.

### 2.8 Defect list for the airing flow

| ID | Defect | Where | Proposal |
|---|---|---|---|
| A-1 | Manual refresh passes an always-empty `trackItems`, so `loadAiringTime` returns `Pair(1, 0L)` and overwrites the stored airing data | AnimeScreenModel.kt:295 with the field never written (1620) | Use the same tracker-item list as the observer (fix the field) |
| A-2 | Simkl "remove aired" uses a list index as a character offset | AniChartApi.kt:157 | Filter by parsed date (section 2.6 Proposal) |
| A-3 | Malformed date throws out of the airing load | AniChartApi.kt:160-164 and callers | Parse with error handling; treat a bad entry as absent |
| A-4 | `AniChartApi()` creates a new `OkHttpClient` per call | AniChartApi.kt:20; AnimeScreenModel.kt:1540 | One shared client (reqwest client held by the app) |
| A-5 | `getAlIdFromMal` does not check HTTP status or JSON validity | AniChartApi.kt:61-74 | Check status; treat non-200 as 0 |
| A-6 | Airing overwrite on completed anime when manual refresh is pressed | AniChartApi.kt:28 bypassed by `manualFetch` | Keep the stored value unless the new result has a real episode |

### 2.9 Countdown display values

- `airingEpisodeNumber` = `nextAiringEpisode.first` as Double (AnimeScreenModel.kt:1665-1666).
- `airingTime` = `nextAiringEpisode.second * 1000 - now` (1668-1671). Not clamped; a past airing time gives a negative value. The presentation (part 1) decides when to show the row (the mockup says: "Airing in 02d 05h 13m 09s").
- Show the row only when: `showNextEpisodeAirTime` (TrackPreferences.kt:43-46, default `true`), `anime.status != COMPLETED` (`expectedNextUpdate` uses the same test, Anime.kt:50-53), and the source provides airing time (part 1).

---

## 3. Related anime

### 3.1 Eligibility and data

- `supportsRelatedAnime` on the source (default false, AnimeSource.kt:121). `observeRelatedAnime` always runs; `syncRelatedAnime` returns early when false (AnimeScreenModel.kt:1473-1474).
- Relations are stored in the `anime_relations` table (name, related_anime_id, sort_order, anime_id, fetched-at). Query `getRelatedAnimeByAnimeId` = `SELECT A.*, R.name FROM anime_relations AS R JOIN animes AS A ON A._id = R.related_anime_id WHERE R.anime_id = :animeId ORDER BY R.sort_order ASC;` (anime_relations.sq:17-23).
- `getRelatedAnime.subscribe(animeId)` is a Flow of `List<AnimeRelationGroup>` (AnimeScreenModel.kt:1463-1466). Consumer: `State.Success.relatedAnime` (1615).

### 3.2 `SyncRelatedAnimeWithSource.await(anime, forceRefresh)` (SyncRelatedAnimeWithSource.kt:18-38), exact

```
source = sourceManager.get(anime.source)?.takeIf { it.supportsRelatedAnime } ?: return
now = Instant.now().toEpochMilli()
if (!forceRefresh) {
    lastFetchedAt = relationRepository.getLastFetchedAt(anime.id)
    if (lastFetchedAt != null && now - lastFetchedAt < TTL) return
}
groups = source.getRelatedAnimeList(anime.toSAnime())
    .map { relation ->
        relation.name to relation.animes
            .distinctBy { it.url }
            .filterNot { it.url == anime.url }
            .take(MAX_PER_GROUP)
            .map { networkToLocalAnime.await(it.toDomainAnime(anime.source)).id }
    }
    .filter { (_, ids) -> ids.isNotEmpty() }
relationRepository.replaceRelations(anime.id, groups, now)
```

Constants: `TTL = 7.days.inWholeMilliseconds` = 604,800,000 ms (line 41). `MAX_PER_GROUP = 10` (line 42).

Rules:
1. Source must be registered (`get`, not `getOrStub`) and must support related anime.
2. Cache: skip when fetched less than 7 days ago, unless forced.
3. Each group: dedupe by `url`, remove the anime itself (by `url`), keep at most 10, convert each to a local DB row (`networkToLocalAnime.await` creates a non-favorite `animes` row when absent and returns its id), keep the group only when at least one id remains.
4. Replace the stored relations for the anime and stamp `now`.

Triggers:
- Screen open: `syncRelatedAnime()` with `forceRefresh = false` (AnimeScreenModel.kt:268).
- Full refresh: `syncRelatedAnime(forceRefresh = true)` when `fetchDetails` is true (309-311). `fetchDetails` is true for the manual refresh.
- Failures: `Throwable` logged as ERROR "Failed to fetch related anime" (1481-1482). `isLoadingRelatedAnime` is always reset in `finally` (1483-1484).

Side effect: each related title can create a row in `animes` (not favorite). Those rows show up in the library database but not in the library UI. Proposal: keep (the rows are needed for navigation), mark them as "stub related" in memo, and garbage-collect them after 30 days (new task in the data area).

### 3.3 Navigation from related rows (AnimeScreen.kt:223-228)

- Tap: `navigator.push(AnimeScreen(relatedAnime.id, fromSource = !relatedAnime.favorite))`. A non-library related anime opens with `fromSource = true`. Its init then loads and refreshes as for any entry (behavior 2.3).
- Long-press: `navigator.push(GlobalAnimeSearchScreen(relatedAnime.title))`.

### 3.4 Presentation contract (part 1, named only)

Display groups: small groups packed into rows of up to N slots, large groups with their own heading and a horizontal list; display mode from `relatedAnimeDisplayMode` (`sourcePreferences.sourceDisplayMode()`, key `pref_display_mode_catalogue`, default `LibraryDisplayMode.default`, SourcePreferences.kt:16-20). Details in `p2-ui-parity.md`.

### 3.5 Related-anime defects

| ID | Defect | Where | Proposal |
|---|---|---|---|
| R-1 | `syncRelatedAnime` uses `sourceManager.get` in the sync interactor but `getOrStub` elsewhere; a missing source silently disables relations | SyncRelatedAnimeWithSource.kt:19 | Return a typed "source missing" result |
| R-2 | Related rows persist as non-favorite anime with no garbage collection | 3.2 step 3 | Add GC (see 3.2) |
| R-3 | `distinctBy { url }` before `filterNot { url == anime.url }` means a self-link with a different URL spelling is kept | line 30-31 | Normalize URLs before comparing |

---

## 4. Rust mapping

| Kotlin | Rust |
|---|---|
| viewer flag constants and getters | `kuro_core::anime::flags::viewer::{INTRO_MASK, AIRING_EPISODE_MASK, AIRING_TIME_MASK, INTRO_DISABLE_BIT}` and `ViewerFlags(u64)` with `skip_intro_seconds()`, `set_skip_intro_seconds(u8)`, `next_airing() -> (u16, i64)`, `set_next_airing(u16, i64)` (shift-based, same bit layout) |
| `SetAnimeViewerFlags` | `kuro_core::anime::viewer::ViewerStore` with one atomic SQL update per field (fixes E-AIR-1) |
| `AniChartApi.loadAiringTime` | `kuro_core::anime::airing::load_airing(anime, tracks, manual_fetch, &AiringClient) -> Result<(u16, i64)>` |
| `AniChartApi` HTTP | `reqwest::Client` (shared, rustls) with `POST https://graphql.anilist.co` and `GET https://data.simkl.in/calendar/{anime,tv,movie_release}.json`; `serde_json` for parsing |
| `toUnixTimestamp` | `chrono::DateTime::parse_from_rfc3339(..)?.timestamp()` with error mapped to "absent" |
| `SyncRelatedAnimeWithSource` | `kuro_core::anime::related::sync(anime, force, &SourceRegistry, &RelationRepo, &AnimeRepo) -> Result<SyncOutcome>` with `TTL: Duration = Duration::from_secs(7 * 24 * 3600)` and `MAX_PER_GROUP: usize = 10` |
| `AnimeRelationGroup` | `struct RelationGroup { name: String, animes: Vec<AnimeRef> }` |
| `isLoadingRelatedAnime` | `bool` field on the entry model, cleared in a drop guard |

---

## 5. Tests (summary)

- Viewer flags: round trip for length 0..255; disable bit set only when length is 0; airing episode 0..65535; airing time 0..2^32-1; writes do not disturb other fields.
- Airing decision table: completed + automatic (no network); completed + manual (network); tracker list order (first AniList/MAL/Simkl with track wins); no track (1, 0).
- AniList parse: `nextAiringEpisode: null` gives (1, 0); valid gives (episode, airingAt seconds).
- Simkl parse: fixtures from the live schema (section 2.6), MAL present vs absent, earliest upcoming per id, malformed date does not throw (A-3), offset defect regression (A-2).
- Related sync: TTL boundary (exactly 7 days refetches), dedupe, self-exclusion, max 10 per group, empty groups dropped, missing source returns a typed result (R-1).
