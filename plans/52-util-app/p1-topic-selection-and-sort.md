# Topic: chapter and episode selection, sorting and dedupe

Sources
- `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt`, `ChapterRemoveDuplicates.kt`, `ChapterFilterDownloaded.kt`
- `app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt`, `EpisodeFilterDownloaded.kt`
- `app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt`, `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt`
- `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt`, `domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt`
- `domain/src/main/java/tachiyomi/domain/entries/TriState.kt`
- Callers: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt` (lines 203-220, 475-486), `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` (lines 1340-1347)

## 1. The pipeline used by the reader and player

```
all chapters of the manga (from DB)
  │
  ├─ sortedWith(getChapterSort(manga, sortDescending = false))     ascending, always, for reading order
  ├─ if skipDupe:  removeDuplicates(selectedChapter)               one chapter per chapterNumber
  └─ if downloadedOnly:  filterDownloadedChapters(manga)           keep cached only (local source: no-op)
```

Player: the same with `getEpisodeSort(anime, sortDescending = false)` and `filterDownloadedEpisodes` (`PlayerViewModel.kt:1340-1347`).

The "next" lookup used by the reader and library (`getNextUnread` / `getNextUnseen`) instead applies the VIEW filters and the VIEW sort:

```
view list (manga's own sort, filters)
  ├─ unread filter     applyFilter(unreadFilter)   { !read }
  ├─ bookmark filter   applyFilter(bookmarkedFilter) { bookmark }
  ├─ downloaded filter applyFilter(downloadedFilter) { downloaded || isLocal }
  └─ sortedWith(getChapterSort(manga))
search:
  sortDescending ? findLast { !read } : find { !read }
```

## 2. Reading direction rule

After sorting by the view comparator, "next" is:
- ascending view: the first unread in the list;
- descending view: the last unread in the list (which is the lowest-numbered unread, the earliest one to read).

Both return the chapter a reader would read next in reading order.

## 3. TriState semantics (`applyFilter`)

| Value | Keep when |
|---|---|
| `DISABLED` | always |
| `ENABLED_IS` | predicate true |
| `ENABLED_NOT` | predicate false |

Downloaded-only mode forces `downloadedFilter = ENABLED_IS` (`Manga.kt:23-25`, `Anime.kt` twin). This applies to the view filter, so with downloaded-only ON the "next" search only sees downloaded chapters.

## 4. Sort comparators

Constants and the direction bit are in `p1-data-and-config.md` section 3.1 (manga) and 3.2 (anime). The comparator table is in `p1-behavior.md` section 5.3.

Summary of the one non-obvious rule: for SOURCE ordering, descending is ASCENDING `sourceOrder`. Upstream source index 0 is usually the newest chapter, so this rule makes "descending" mean newest first. Every other key uses the natural meaning. Keep this in the Rust port (TC-08-b).

Text comparison: `compareToWithCollator` uses `Collator.getInstance(Locale.getDefault())` with PRIMARY strength (case and accent insensitive). Rust: `icu_collator` with `Strength::Primary` for the system locale.

## 5. Dedupe (`removeDuplicates`) in detail

Input: a list sorted ascending, and `current`.
1. Group by `chapterNumber` (insertion order of groups is the order of first appearance in the input).
2. Per group pick: the element with `id == current.id`, else the element with `scanlator == current.scanlator`, else the first element.
3. Output: one element per group in group order.

Why it matters: many sources list the same chapter from several scanlation groups. The reader must not show duplicates, and it should keep the group the user is already on.

Rust detail: key on `f64::to_bits` after mapping `-0.0` to `0.0`. Do not key on `f64` equality alone (NaN never equals itself).

## 6. Downloaded filter

Cache lookup: `MangaDownloadCache.isChapterDownloaded(name, scanlator, mangaTitle, sourceId, skipCache = false)` and `AnimeDownloadCache.isEpisodeDownloaded(...)`. Matching uses directory names built from name and scanlator (download area 23/24). The check is in memory after `renewCache()`.

Local source (`source == 0L`): no check, every unit passes.

## 7. Quirks to preserve (and why)

| Quirk | Where | Preserve? |
|---|---|---|
| SOURCE descending is ascending `sourceOrder` | `ChapterSorter.kt:11-13` | yes (user-visible sort order) |
| `getNextUnread` returns null when the unread filter is "show read" and nothing is unread | `ChapterGetNextUnread.kt:12-20` | yes (test TC-07-d) |
| Downloaded-only makes "next" ignore non-downloaded chapters | `Manga.kt:23-25` | yes |
| `removeDuplicates` keeps the first group member when no id or scanlator match | `ChapterRemoveDuplicates.kt:11-13` | yes |
| `removeDuplicates` treats `scanlator == null` as equal to `null` | same | yes |

## 8. Rust API (Proposal, `kuro-core::selection` and `kuro-core::sort`)

```rust
pub trait Unit: Clone { /* id, number, name, scanlator, is_read, source_order, date_upload */ }
pub fn next_unread<U: Unit>(units: &[U], filter: &ViewFilter, sorted: impl Fn(&[U]) -> Vec<U>, sort_desc: bool,
                            is_downloaded: impl Fn(&U) -> bool, is_local: bool) -> Option<U>;
pub fn dedupe<U: Unit>(units: &[U], current: &U) -> Vec<U>;
pub fn filter_downloaded<U: Unit>(units: &[U], is_local: bool, is_downloaded: impl Fn(&U) -> bool) -> Vec<U>;
pub enum ChapterSorting { Source, Number, UploadDate, Alphabet }
pub fn chapter_sort_key(bits: i64) -> Result<ChapterSorting, SortError>;
```

Tests: TC-07 (next unread), TC-08 (sort), TC-09 (dedupe), TC-10 (downloaded filter). Full vectors in `p1-edge-cases-and-tests.md` sections 5.3 to 5.5.
