# Area 08 — Topic: updates feed ("Updates" tab data)

Sources: `domain/src/main/java/tachiyomi/domain/updates/**` (6 files), `data/src/main/sqldelight/view/updatesView.sq`, `data/src/main/sqldelightanime/view/animeupdatesView.sq`, `data/src/main/java/tachiyomi/data/updates/**`, `app/src/main/java/eu/kanade/tachiyomi/ui/updates/**`, `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt` (writers), widgets (`presentation-widget`).

## 1. What an "update" is

A row of `updatesView`: a chapter of a **favourite** manga whose `date_fetch` is greater than the manga's `date_added`. `date_fetch` is set when the chapter row is first inserted by `SyncChaptersWithSource` (`dateFetch = nowMillis + itemCount--`: within one sync the newest source chapter gets the largest value, so the order within one fetch follows source order); `date_added` is set when the manga is put into the library. Chapters fetched before or at the moment the user adds the manga have `date_fetch <= date_added` and are therefore not updates; only chapters inserted by a later fetch qualify. Removing a manga from the library and adding it again resets `date_added`, which hides all chapters fetched before the re-add.

## 2. The three queries

| Method | Filter | Used by |
|---|---|---|
| `subscribe(instant)` -> `getRecentUpdates(after = instant.toEpochMilli(), limit = 500)` | `dateUpload > after` | Updates tab (after = now minus 3 calendar months, computed once at screen model creation) |
| `subscribe(read/seen, after)` -> `getUpdatesByReadStatus(read, after, 500)` | `read = :read AND dateUpload > after` | home-screen widget (`read = false`, `after = now - 3 months` re-evaluated per use) |
| `await(read/seen, after)` | same | one-shot variant (no current caller in the app tree) |

Key facts: the window is on **upload** date, the sort on **fetch** date, the cap is 500 rows, the sort is `date_fetch DESC` inside the view (implementation detail of SQLite, hence the explicit ORDER BY in the Rust query). `SyncChaptersWithSource` (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt:106-112`) stores a new chapter whose source reports `date_upload = 0` with `date_upload = if (maxSeenUploadDate == 0L) nowMillis else maxSeenUploadDate` (the largest upload date seen earlier in the same sync), so such chapters normally pass `dateUpload > after`. `date_fetch` of new chapters is `nowMillis + itemCount--` (the first, i.e. most recent, source chapter gets the largest value; the comment in the source states sources MUST return chapters from most to least recent); a chapter that reappears after having been removed re-uses the old `date_fetch` of its removed twin "to not pollute 'Updates' tab" (lines 187-202), so it does not jump to the top of the feed again.

Rust SQL (Proposal, same semantics plus deterministic order):

```sql
SELECT m._id AS manga_id, m.title, c._id AS chapter_id, c.name, c.scanlator, c.read, c.bookmark, c.last_page_read,
       m.source, m.favorite, m.thumbnail_url, m.cover_last_modified, c.date_upload, c.date_fetch
FROM mangas m JOIN chapters c ON m._id = c.manga_id
WHERE m.favorite = 1 AND c.date_fetch > m.date_added AND c.date_upload > ?1
ORDER BY c.date_fetch DESC, c._id DESC
LIMIT 500;
```
(`AND c.read = ?2` for the filtered variant.) For anime add `e.fillermark, e.last_second_seen, e.total_seconds` and filter `e.seen`.

## 3. Counters and the tab badge

- Writers: each library update run adds `newChapters.size` (manga) or the new episode count (anime) to `library_unread_updates_count` / `library_unseen_updates_count` (`getAndSet { it + n }` on the preference) for every entry that received new items.
- Reader: the Updates tab (`resetNewUpdatesCount`) sets both of its kind to 0 when it is shown and again when it is disposed; the bottom navigation badge = `anime + manga` if `library_show_updates_count`, hidden when 0.
- Note: the tab sets only its own kind's counter (`newMangaUpdatesCount` in the manga tab; the anime tab resets `newAnimeUpdatesCount`). Because `UpdatesTab` composes both sub-tabs, both are reset when the Updates tab is opened.

## 4. Display rules (domain-side)

Header per local day of `dateFetch`; first row "Library last updated: <relative time>" where `lastUpdatedTimestamp` is epoch ms (`eu.kanade.presentation.util.TimeUtils.relativeTimeSpanString(epochMillis)`: `<= 0` -> `MR.strings.relative_time_span_never`; younger than 1 minute -> `MR.strings.updates_last_update_info_just_now`; else Android `DateUtils.getRelativeTimeSpanString(t, now, MINUTE_IN_MILLIS)` = minutes/hours/yesterday/date phrases, which Rust reproduces with its own formatter in area:52-util-app).

Item state: `unread/unseen` dot when `!read`; bookmark icon when `bookmark`; fillermark icon (anime); `lastPageRead` shown as `Page: N` when `lastPageRead > 0 && !read`; anime shows `lastSecondSeen/totalSeconds` as `mm:ss/mm:ss` when `lastSecondSeen > 0`. Items of read chapters are drawn with 0.38 alpha. Download state per item from the download manager (area:23/24).

## 5. Rust service (Proposal)

```rust
pub trait UpdatesService {
    fn subscribe_recent(&self, kind: EntryKind, after: EpochMs) -> watch::Receiver<Arc<Vec<UpdateRow>>>;
    fn subscribe_by_status(&self, kind: EntryKind, done: bool, after: EpochMs) -> watch::Receiver<Arc<Vec<UpdateRow>>>;
    async fn recent(&self, kind: EntryKind, done: bool, after: EpochMs) -> Result<Vec<UpdateRow>, DbError>;
    fn reset_new_counts(&self, kind: EntryKind);          // prefs.new_{manga,anime}_updates_count = 0
}
pub fn three_months_ago(now: OffsetDateTime) -> EpochMs;   // calendar months in the local zone, like ZonedDateTime.minusMonths(3)
```
`three_months_ago`: subtract 3 months keeping the day-of-month clamped to the month length (Kotlin `ZonedDateTime.minusMonths` clamps, e.g. 31 May minus 3 months = 28/29 Feb).
