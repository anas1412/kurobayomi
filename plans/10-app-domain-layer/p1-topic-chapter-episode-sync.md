# Area 10 topic: chapter / episode / season synchronisation

Files: `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt` (234 lines), `app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt` (242), `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt` (109), `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt` (44). The step list is in p1-behavior.md section 6; this file adds the pure-function decomposition, worked examples and the helper algorithms the sync depends on.

## 1. Why this is the most important code in the area

Every refresh (opening a details screen, the library update job, migration, deep links) ends here. Mistakes cause: lost read progress (a chapter re-uploaded under a new URL becomes unread), duplicate notifications, wrongly sorted "Updates" tab, orphaned downloads. The Kotlin has no unit test for it in the repository (`find . -name '*Test.kt'` lists only `MigratorTest`, `LibraryFlagsTest`, `GetApplicationReleaseTest`, `MissingItemsTest`, `TorrentHelpersTest`, `ChapterRecognitionTest`, `AnimeFetchIntervalTest`, `MangaFetchIntervalTest`), so the golden tests in p1-edge-cases-and-tests.md are new.

## 2. Proposal: split into a pure planner and an applier

```rust
/// Pure: no IO, deterministic given inputs. Unit-testable with table tests.
pub fn plan_chapter_sync(input: PlanInput<'_>) -> SyncPlan;

pub struct PlanInput<'a> {
    pub manga_title: &'a str,
    pub manga_id: i64,
    pub source_chapters: Vec<Chapter>,      // already distinct_by(url), sanitized, prepared, number-recognised, sourceOrder = index
    pub db_chapters: &'a [Chapter],
    pub now_millis: i64,
    pub mark_duplicate_as_read: bool,
}
pub struct SyncPlan {
    pub added: Vec<Chapter>,                // with dateFetch, read/bookmark carried
    pub updated: Vec<Chapter>,              // db rows with new name/number/scanlator/sourceOrder/dateUpload
    pub removed_ids: Vec<i64>,
    pub changed_or_duplicate_read_urls: HashSet<String>,
    pub renames: Vec<(Chapter /*db*/, Chapter /*source*/)>,   // downloads to rename (decided by the applier, needs the download manager)
}
```
The applier (`SyncChaptersWithSource::await`) does: guard (empty + not local -> `NoChaptersException`), source normalisation (`distinctBy`, sanitize, `prepare_new_chapter`, `parse_chapter_number`), calls the planner, performs download renames, then the DB writes (ideally one transaction: delete, insert, update, fetch interval, last_update; the Kotlin does them as separate calls), and finally the exclusion filter.

Episodes use the same planner generic over an `Item` trait (number, read flag name differs: `seen`) with extra merge rules (section 5).

## 3. Chapter sync, worked example

State before (manga "Foo", id 7, `markDuplicateReadChapterAsRead = {"new"}`, `nowMillis = 1_000_000`):

| db id | url | number | read | bookmark | dateFetch |
|---|---|---|---|---|---|
| 1 | /c1 | 1.0 | true | false | 100 |
| 2 | /c2 | 2.0 | false | false | 200 |
| 3 | /c3 | 3.0 | true | true | 500 |

Source returns (newest first) `/c5 "Ch 5"` (date 0), `/c4 "Ch 4"` (date 900), `/c3b "Ch 3"` (date 0, number 3.0), `/c2`, `/c1` (unchanged names/numbers/dates, so `ShouldUpdateDbChapter` is false apart from `sourceOrder`, see below).

1. `sourceOrder` becomes the list index: `/c5 0, /c4 1, /c3b 2, /c2 3, /c1 4`. Previously stored `sourceOrder` of db rows were (say) `1,0`-style values; because `sourceOrder` is compared, `/c2` (db sourceOrder 1 vs 3) and `/c1` (0 vs 4) are in `updatedChapters` (name/number/scanlator/sourceOrder written; read state untouched).
2. `removedChapters = [db 3]` (`/c3` is not in the source list).
3. New chapters in order: `/c5`: `dateUpload == 0` and `maxSeenUploadDate == 0` -> `dateUpload = nowMillis = 1_000_000`. `/c4`: `dateUpload = 900 != 0` -> `maxSeenUploadDate = max(0, 900) = 900`. `/c3b`: `dateUpload == 0` and `maxSeenUploadDate = 900` -> `dateUpload = 900`.
4. `readChapterNumbers = {1.0, 3.0}`; `deletedChapterNumbers = {3.0}`, `deletedReadChapterNumbers = {3.0}`, `deletedBookmarkedChapterNumbers = {3.0}`, `deletedChapterNumberDateFetchMap = {3.0 -> 500}`.
5. Fetch dates (`itemCount = 3`): `/c5 -> 1_000_003`, `/c4 -> 1_000_002`, `/c3b -> 1_000_001`.
6. `/c3b`: number 3.0 in `readChapterNumbers` and `markDuplicateAsRead` -> `read = true`, url added to `changedOrDuplicateReadUrls`; then number in `deletedChapterNumbers` -> `read = (3.0 in deletedRead) = true`, `bookmark = true`, `dateFetch = 500` (so it stays old in the Updates tab), url added again.
7. DB: delete id 3; insert `/c5, /c4, /c3b`; update ids 1,2 (sourceOrder); fetch interval recomputed; `last_update = now`.
8. Return value: `[/c5, /c4]` (`/c3b` filtered out by `changedOrDuplicateReadUrls`). Only these two trigger "new chapters" notifications and auto-download.

Check of the deletion-map quirk: if two removed chapters share number 3.0 with dateFetch 500 and 400, sorted descending `[500, 400]`, `associate` keeps the last pair, so the map holds `400`.

## 4. Why the ordering rules exist (implement them literally)

- `distinctBy { url }` keeps the FIRST occurrence; later duplicate URLs are discarded before `sourceOrder` is assigned, so indices are contiguous over the distinct list.
- "Sources MUST return the chapters from most to less recent" (comment at `SyncChaptersWithSource.kt:181`). The descending `dateFetch` values (from `now + N` for the first listed new chapter down to `now + 1` for the last) keep the newest chapter at the top when the Updates tab sorts by `date_fetch DESC`, and keep the relative order of one batch stable.
- `maxSeenUploadDate` only advances while walking NEW chapters with a real date; chapters already in the DB do not contribute. The date of a new chapter without `date_upload` is therefore the date of the previously walked (newer) new chapter with a date, or `now` if none came before.
- `dateUpload` of a db chapter is overwritten only if the source value is non-zero.
- `ShouldUpdateDbChapter` compares `scanlator, name, dateUpload, chapterNumber, sourceOrder` as raw values: `dateUpload` 0 from the source versus a stored non-zero value is a difference (the row is "updated" but `dateUpload` is kept because of the `!= 0L` guard), so such a chapter is re-written on every sync (harmless).
- Float widening: `SChapter.chapter_number` is a 32-bit float; the domain stores `chapter_number.toDouble()`. In Rust keep `f32` in the source struct and convert with `as f64`, never parse decimal text again, otherwise `ShouldUpdateDbChapter` sees `12.1 != 12.100000381469727` forever.

## 5. Episode-specific rules

- `copyFromSEpisode` does not `trim()` the scanlator.
- Updated rows: `name, episodeNumber, scanlator, summary, sourceOrder, memo` always; `dateUpload` only if non-zero; `fillermark` taken from the source only while the stored value is `false`; `previewUrl` taken from the source only while the stored value is null/blank.
- `ShouldUpdateDbEpisode` additionally compares `summary, fillermark, previewUrl, memo`.
- Duplicate-seen marking uses preference `mark_duplicate_seen_episode_seen` value `"new_episode"`.
- No scanlator exclusion and no `dateFetch` special handling difference.

## 6. Season sync and recognition

`SyncSeasonsWithSource` is a smaller diff-and-write (p1-behavior.md 6.3). `SeasonRecognition.parseSeasonNumber(animeTitle, seasonName, seasonNumber: Double? = null)` (`domain/src/main/java/tachiyomi/domain/items/season/service/SeasonRecognition.kt`):
1. If `seasonNumber != null && (seasonNumber == -2.0 || seasonNumber > -1.0)` return it.
2. `clean = seasonName.lowercase().replace(animeTitle.lowercase(), "").trim().replace(',', '.').replace('-', '.').replace(unwantedWhiteSpace, "")`.
3. While `tagRegex` matches, remove all matches. `tagRegex = ^\[[^\]]+\]|\[[^\]]+\]\s*$|^\([^\)]+\)|\([^\)]+\)\s*$`.
4. `number = Regex(([0-9]+)(\.[0-9]+)?(\.?[a-z]+)?)`. No match -> return `seasonNumber ?: -1.0`. More than one match -> remove `unwanted = \b\d+p\b|\d+x\d+|Hi10|\(\d+\)` then try `basic = (?<=\bs\.|\bs|season) *([0-9]+)(\.[0-9]+)?(\.?[a-z]+)?` first, then the first `number` match. Otherwise the first match.
5. `getSeasonNumberFromMatch`: `initial = group1.toDouble()`, plus `checkForDecimal(group2, group3)`: group2 non-empty -> `group2.toDouble()` (e.g. `.5` -> 0.5); group3 non-empty: contains `extra` -> 0.99, `omake` -> 0.98, `special` -> 0.97; else trimmed of leading `.`, if length 1 -> `a=0.1 .. i=0.9` (`(c - 'a' + 1)/10`, `>= 10` -> 0.0); otherwise 0.0.
6. `unwantedWhiteSpace = \s(?=extra|special|omake)`.
Rust: the `regex` crate has no look-behind, so `basic` needs `fancy-regex` (see p1-rust-design.md section 4), or an equivalent manual scan: after finding the keyword (`s.`, `s`, `season`) match the number pattern at the following position with optional spaces.

## 7. Fetch-interval helpers used by the sync (owned by area:07, constants repeated here because the sync's "early return" depends on them)

`MangaFetchInterval` (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt`; the anime twin has identical constants):
- `MAX_INTERVAL = 28`, `GRACE_PERIOD = 1`.
- `getWindow(dateTime)`: `today = start of local day`; `(today - 1 day).epochSecond * 1000` to `(today + 1 day).epochSecond * 1000 - 1`.
- The sentinel `fetchWindow = (0, 0)` means "compute the window now" (`toMangaUpdate`).
- `toMangaUpdate(manga, dateTime, window)`: `interval = manga.fetchInterval.takeIf { it < 0 } ?: calculateInterval(chapters(applyScanlatorFilter=true), zone)` (a NEGATIVE stored interval is a user-pinned interval and is kept); `nextUpdate = calculateNextUpdate(manga, interval, dateTime, window)`.
- `calculateInterval`: `chapterWindow = if (chapters.size <= 8) 3 else 10`; take the distinct local-day start of `dateUpload > 0` chapters sorted descending, first `chapterWindow`; same for `dateFetch`; if `uploadDates.size >= 3`: `delta days between last and first` floor-divided by `indexOf(last)`; else if `fetchDates.size >= 3` the same on fetch dates; else 7; clamp to `1..28`.
- `calculateNextUpdate`: if `manga.nextUpdate` lies inside `[window.first, window.second + 1]` keep it; else `latestDate = start of day of (lastUpdate > 0 ? lastUpdate : now)`; `timeSinceLatest = days between latestDate and dateTime`; `cycle = timeSinceLatest.floorDiv(if interval < 0 then |interval| else increaseInterval(interval, timeSinceLatest, 10))`; result `latestDate + (cycle + 1) * |interval| days` at `dateTime.offset` in ms. `increaseInterval(delta, t, over)`: `delta >= 28 -> 28`; `cycle = t.floorDiv(delta) + 1`; `cycle > over -> increaseInterval(delta*2, ...)` else `delta`.
The sync's early return `if (manualFetch || manga.fetchInterval == 0 || manga.nextUpdate < fetchWindow.first)` recomputes the schedule only when needed, to avoid writes on every no-change refresh.

## 8. Related anime

`SyncRelatedAnimeWithSource` (p1-behavior.md 6.4) has a TTL of 7 days and a cap of 10 entries per group; the cap and `distinctBy url` happen BEFORE the DB insert, so at most 10 rows per group are created. The group name is the source-provided string, shown as a heading by the UI (area:39).
