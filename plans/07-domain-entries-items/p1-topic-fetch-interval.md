# Area 07 — topic: fetch interval ("smart update") — MangaFetchInterval / AnimeFetchInterval

Files: `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt` (130 lines), `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt` (130). The two classes are byte-for-byte the same algorithm; they differ in (a) the item type (`Chapter` vs `Episode`), (b) the repository call (`getChaptersByMangaId.await(manga.id, applyScanlatorFilter = true)` vs `getEpisodesByAnimeId.await(anime.id)`), (c) the update type (`MangaUpdate` vs `AnimeUpdate`). Tests: `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt` and `domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt` (10 tests each, identical numbers; only the type names differ).

Purpose: guess how often a series releases, so the library updater can skip entries "outside their release period" instead of fetching every entry every time. Result fields on the entry: `fetchInterval` (days) and `nextUpdate` (epoch ms). Consumers: `SyncChaptersWithSource` / `SyncEpisodesWithSource` / `SyncSeasonsWithSource` -> `UpdateManga.awaitUpdateFetchInterval` / `UpdateAnime.awaitUpdateFetchInterval` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:313-321`), the library update job (`app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:184,217,256`), the SetInterval dialog (`app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt`).

## 1. Definitions

- `MAX_INTERVAL = 28` (days), `GRACE_PERIOD = 1` (day), `increaseWhenOver = 10`.
- A "day" is a calendar day in the zone of the `ZonedDateTime` passed in (the system default zone in production).
- `fetchInterval == 0`: never computed. `> 0`: computed (1..28). `< 0`: user override of `abs(value)` days (1..28). The details dialog writes `-wheelValue`; wheel value 0 ("Default") writes `0`.
- `Pair<Long, Long> window`: `(lowerEpochMs, upperEpochMs)`; `(0, 0)` is the sentinel meaning "compute from now".

## 2. `getWindow(dateTime)`

```
today  = dateTime.toLocalDate().atStartOfDay(dateTime.zone)      // local midnight of dateTime's date
lower  = today.minusDays(1)                                       // wall-clock arithmetic, DST aware
upper  = today.plusDays(1)
return (lower.toEpochSecond() * 1000, upper.toEpochSecond() * 1000 - 1)
```

The window therefore covers yesterday 00:00 through "end of tomorrow minus 1 ms" (2 days, or 47/49 hours on DST days). Example: now = 2024-03-15T10:30:00Z in UTC gives `(1710374400000, 1710547199999)`: `lower` = 2024-03-14T00:00:00Z, `upper` = 2024-03-16T00:00:00Z, so the window is [Mar 14 00:00, Mar 16 00:00) with the last millisecond excluded by the `- 1`. In Europe/Berlin on 2024-03-31 12:00 (the 23-hour DST day) the window is `(1711753200000, 1711922399999)` = 2024-03-30T00:00+01:00 .. 2024-03-31T23:59:59.999+02:00.

## 3. `toMangaUpdate(manga, dateTime, window)`

```
interval = if manga.fetchInterval < 0 then manga.fetchInterval
           else calculateInterval(getChaptersByMangaId(manga.id, applyScanlatorFilter = true), dateTime.zone)
currentWindow = if window == (0, 0) then getWindow(ZonedDateTime.now()) else window
nextUpdate = calculateNextUpdate(manga, interval, dateTime, currentWindow)
return MangaUpdate(id = manga.id, nextUpdate = nextUpdate, fetchInterval = interval)
```

`takeIf { it < 0 } ?: calculateInterval(...)` means: a stored `0` or positive value is ALWAYS recomputed from the chapter list; only negative (user) values are sticky. A negative interval is returned unchanged (still negative) and written back.

For manga the chapter list is the scanlator-filtered list (`applyScanlatorFilter = true`: chapters from excluded scanlators are not used for the estimate). The anime variant has no such filter.

## 4. `calculateInterval(items, zone)`

```
window = if items.size <= 8 then 3 else 10
uploadDates = items.filter(dateUpload > 0)
                   .sortedByDescending(dateUpload)                      // stable
                   .map(localDate(dateUpload, zone).atStartOfDay())     // LocalDateTime at 00:00
                   .distinct()                                          // keeps first occurrences, order preserved
                   .take(window)
fetchDates  = items.sortedByDescending(dateFetch)                       // no filter: dateFetch 0 gives 1970-01-01
                   .map(localDate(dateFetch, zone).atStartOfDay()) .distinct() .take(window)
interval =
   if uploadDates.size >= 3: uploadDates.last().until(uploadDates.first(), DAYS)  floorDiv  uploadDates.indexOf(uploadDates.last())
   else if fetchDates.size >= 3: same with fetchDates
   else 7
return interval.coerceIn(1, 28)
```

Because the dates are distinct, `indexOf(last)` = `size - 1` (2..9), never 0. `until(..., DAYS)` between two `LocalDateTime` values at midnight is an exact integer day difference (no DST involvement). `floorDiv` of a non-negative delta by a positive period is ordinary integer division. Note that with distinct dates in strictly descending order, `delta >= size - 1 >= 2`, so the interval before clamping is at least 1.

Meaning: average gap in days between the most recent distinct release days within the sample window. Window 3 for short series (<= 8 items) compares only the latest three distinct release days; window 10 uses the latest ten.

## 5. `calculateNextUpdate(entry, interval, dateTime, window)`

```
if entry.nextUpdate in window.first ..= window.second + 1:      // inclusive both ends; window.second + 1 is exactly the upper midnight
    return entry.nextUpdate                                       // already scheduled inside the window: keep

latestDate    = localDate(if entry.lastUpdate > 0 then entry.lastUpdate else Instant.now(), dateTime.zone).atStartOfDay()
timeSinceLatest = DAYS.between(latestDate, dateTime).toInt()      // whole days from latestDate (local midnight) to dateTime's local date-time; truncates toward zero
step = if interval < 0 then abs(interval) else increaseInterval(interval, timeSinceLatest, 10)
cycle = timeSinceLatest.floorDiv(step)
return latestDate.plusDays((cycle + 1) * abs(interval)).toEpochSecond(dateTime.offset) * 1000
```

```
increaseInterval(delta, timeSinceLatest, increaseWhenOver):
    if delta >= 28: return 28
    cycle = timeSinceLatest.floorDiv(delta) + 1
    if cycle > increaseWhenOver: return increaseInterval(delta * 2, timeSinceLatest, increaseWhenOver)
    else return delta
```

Interpretation: starting from the last time the chapter list changed (`lastUpdate`), predict the next boundary on a grid of `interval` days. The "missed too many checks" rule doubles the step used to compute `cycle` (1, 2, 4, 8, 16, 28 days) when more than 10 periods have passed since the last change, so long-silent series are checked less often.

### 5.1 Quirks (OBSERVED; keep for parity, see p1-topic-quirks-and-decisions.md Q9-Q11)

1. `cycle` uses the (possibly doubled) `step`, but the returned time adds `(cycle + 1) * abs(interval)` days with the ORIGINAL interval. When doubling happened the result is usually in the past. Example: `lastUpdate = 2023-12-01T00:00Z`, `interval = 1`, now = 2024-03-15T10:30Z: `timeSinceLatest = 105`; `increaseInterval(1, 105, 10)`: cycle 106 > 10 -> 2: 53 -> 4: 27 -> 8: 14 -> 16: 7 <= 10 -> returns 16; `cycle = 105 floorDiv 16 = 6`; `next = Dec 1 + 7 * 1 day = 2023-12-08T00:00Z` (`1701993600000`), 97 days in the past. Effect: the entry's `nextUpdate` is `<= fetchWindowUpperBound`, so the updater never skips it; the long-silent series are checked every run, which is the opposite of the intent of the doubling. A later refresh that finds nothing new calls `awaitUpdateFetchInterval` again only if `manualFetch || fetchInterval == 0 || nextUpdate < fetchWindow.first` (true here), so it recomputes to the same past date every run.
2. `Instant.now()` (not `dateTime`) is used when `lastUpdate <= 0`.
3. `DAYS.between(LocalDateTime, ZonedDateTime)` converts the zoned value to a local date-time in its own zone first; if `lastUpdate` is in the future relative to `dateTime`, the result is negative, truncated toward zero (e.g. -0.5 day gives 0, -1.5 gives -1) and `floorDiv` of a negative gives a negative cycle (`-1 floorDiv 7 = -1`, so `cycle + 1 = 0` and `nextUpdate = latestDate`).
4. The conversion back to epoch uses `toEpochSecond(dateTime.offset)`, the offset of `dateTime` NOW, not the offset valid on the target date. Example: Europe/Berlin, now `2024-03-29T10:00+01:00`, `lastUpdate` = 2024-03-29 00:00 local (`1711666800000`), interval 7 gives `1712271600000` = 2024-04-05T01:00+02:00 (the intended local midnight, 2024-04-05T00:00+02:00, is 1 h earlier because DST began on Mar 31). Harmless in practice (the result is only compared against day-granular windows).
5. `nextUpdate` equal to the lower window bound or `window.second + 1` is "inside" (closed interval, `rangeTo`).

### 5.2 Worked vectors (UTC unless stated; computed with a Python prototype of the algorithm)

`now = 2024-03-15T10:30:00Z`, `window = (1710374400000, 1710547199999)`; `nowMs = 1710498600000`.

| nextUpdate | lastUpdate | interval | Result (ms) | Meaning |
|---|---|---|---|---|
| 0 | 1710028800000 (2024-03-10) | 7 | 1710633600000 | 2024-03-17T00:00Z (since = 5, cycle 0) |
| 0 | 1710028800000 | 1 | 1710547200000 | 2024-03-16T00:00Z (since = 5, step 1, cycle 5, +6 d) |
| 0 | 0 | 7 | 1711065600000 | 2024-03-22T00:00Z (latest = today, since = 0) |
| 0 | 1701388800000 (2023-12-01) | 1 | 1701993600000 | 2023-12-08 (quirk 1: in the past) |
| 0 | 1710028800000 | -3 | 1710547200000 | 2024-03-16 (user interval 3: step 3, since 5, cycle 1, +6 d) |
| 1710374400005 | 0 | 7 | 1710374400005 | inside the window, unchanged |

## 6. Upstream tests to port (10, run for both item types)

`testTime = 2020-01-01T00:00:00Z`, zone `UTC`, `item = create().copy(dateFetch = t, dateUpload = t)` (epoch ms of `testTime`), helper `withTime(item, duration)` sets BOTH `dateFetch` and `dateUpload` to `testTime + duration`, `lastUploadDate(list)` = last element's `dateUpload` as a Duration since the epoch. All expectations are `calculateInterval(list, UTC) shouldBe n`.

| # | Test name | Input | n |
|---|---|---|---|
| 1 | returns default interval of 7 days when not enough distinct days | 50 items at `1.days`; then the same 50 with `dateUpload = 0` | 7 and 7 |
| 2 | returns interval based on more recent episodes/chapters | old: `(1..5)` at `(it*7).days`; new: `(1..10)` at `old.lastUploadDate() + it.days`; list = old + new (15 items, window 10) | 1 |
| 3 | returns interval based on smaller subset of recent episodes/chapters if very few episodes/chapters | old `(1..3)` at `(it*7).days`; new `(1..3)` at `old.lastUploadDate() + 365.days + (it*7).days` (6 items, window 3) | 7 |
| 4 | returns interval of 7 days when multiple episodes/chapters in 1 day | 10 items at `10.hours` | 7 |
| 5 | returns interval of 7 days when multiple episodes/chapters in 2 days | 2 items at `1.days` + 5 items at `2.days` | 7 |
| 6 | returns interval of 1 day when episodes/chapters are released every 1 day | `(1..20)` at `it.days` | 1 |
| 7 | returns interval of 1 day when delta is less than 1 day | `(1..20)` at `(15*it).hours` | 1 |
| 8 | returns interval of 2 days when episodes/chapters are released every 2 days | `(1..20)` at `(2*it).days` | 2 |
| 9 | returns interval with floored value when interval is decimal | `(1..5)` at `(25*it).hours`; then the same with `dateUpload = 0` (fetch dates used) | 1 and 1 |
| 10 | returns interval of 1 day when episodes/chapters are released just below every 2 days | `(1..20)` at `(43*it).hours` | 1 |

All 10 were evaluated with a Python prototype of the algorithm and pass. Traces for the non-obvious ones: test 7: day offsets of `15*i h` for i = 1..20 are 0,1,1,2,3,3,4,5,5,6,6,7,8,8,9,10,10,11,11,12; the ten most recent distinct days are 12..3, delta 9, period 9, result 1. Test 10: day offsets `floor(43*i/24)` for i = 20 down to 11 are 35,34,32,30,28,26,25,23,21,19; delta 16, period 9, `16 floorDiv 9 = 1`. Test 9: five items at days 1..5; size 5 <= 8 so window 3, dates 5,4,3, delta 2, period 2, result 1. Test 3: six items (window 3): days 407,400,393 (21+365+7k), delta 14, period 2, result 7.

## 7. Additional tests to write (no upstream coverage)

- `get_window` for UTC 2024-03-15T10:30Z and for Europe/Berlin 2024-03-31T12:00 (vectors in section 2 and 5.2).
- `calculate_next_update` rows of section 5.2, including the DST row (quirk 4), the "inside window" row, the negative interval row, and `lastUpdate` in the future.
- `increase_interval` table: `(1, 105, 10) -> 16`, `(7, 5, 10) -> 7`, `(28, 1000, 10) -> 28`, `(20, 400, 10) -> 28` (20: cycle 21 > 10 -> 40 >= 28 -> 28).
- `to_update` with `fetchInterval = -5` returns `fetchInterval = -5` and a `nextUpdate` computed with step 5.
- Empty and single-item lists give 7.
- All items with `dateUpload` in the future still compute (no clamp to now).

## 8. Rust design notes

- Time library: DST-correct local-day arithmetic needs an IANA zone database. The `time` crate (proposed by area:05-data-repos-entries-items for time helpers) does not ship one; `chrono` with the `clock` feature reads local zone rules through the OS (`chrono::Local`) and allows a generic `TimeZone` parameter (tests use `Utc` or `chrono_tz`). Proposal: use `chrono` inside `kuro-core::domain::fetch_interval`, expose epoch-millisecond `i64` at the boundary, and reconcile with area:05-data-repos-entries-items `T-05a-030` (time helpers) by letting that task wrap the same `chrono` functions. Decision D2 in p1-topic-quirks-and-decisions.md.
- API: `fn calculate_interval<Tz: TimeZone>(items: &[ItemDates], tz: &Tz) -> i32` where `ItemDates { date_upload: i64, date_fetch: i64 }`; one generic implementation for chapters and episodes; `fn get_window<Tz>(now: DateTime<Tz>) -> (i64, i64)`; `fn to_update(...)` returns `{ next_update: i64, fetch_interval: i32 }` merged into the entry's update.
- `floorDiv` = `i32::div_euclid` for a positive divisor (the divisor is always positive here).
- `DAYS.between` = `(end.naive_local() - latest_midnight).num_days()` which truncates toward zero like Java.
- `toEpochSecond(dateTime.offset)` = `latest.plus_days(n).and_utc().timestamp() - offset_seconds(dateTime)`; implement exactly this (quirk 4) rather than `.and_local_timezone(tz)`.
