# Area 07 — topic: item sorting, season sorting, and the filter pipeline

Files: `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt` (30 lines), `domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt` (30), `domain/src/main/java/tachiyomi/domain/items/season/service/SeasonSorter.kt` (36); `core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt` (collator); filter callers `app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt`, `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1701-1722`.

## 1. Chapter and episode comparators

OBSERVED. `getChapterSort(manga, sortDescending = manga.sortDescending()): (Chapter, Chapter) -> Int` selects on `manga.sorting` (`chapterFlags & 0x300`). `getEpisodeSort(anime, sortDescending = anime.sortDescending())` selects on `anime.sorting` (`episodeFlags & 0x600`). Table (c1/c2 are the two compared items; the Kotlin `compareTo` on `Long`/`Double` primitives is `Long.compare`/`Double.compare`):

| `sorting` | `sortDescending == true` | `sortDescending == false` |
|---|---|---|
| SOURCE (0) | `c1.sourceOrder` vs `c2.sourceOrder` (ASCENDING by `sourceOrder`) | `c2.sourceOrder` vs `c1.sourceOrder` (descending by `sourceOrder`) |
| NUMBER (`0x100` manga / `0x200` anime) | `c2.number` vs `c1.number` (descending number) | `c1.number` vs `c2.number` |
| UPLOAD_DATE (`0x200` / `0x400`) | `c2.dateUpload` vs `c1.dateUpload` | `c1.dateUpload` vs `c2.dateUpload` |
| ALPHABET (`0x300` / `0x600`) | `c2.name.compareToWithCollator(c1.name)` | `c1.name.compareToWithCollator(c2.name)` |
| anything else | throws `NotImplementedError("Invalid chapter sorting method: <value>")` / `"Invalid episode sorting method: ..."` (unreachable: the 2-bit field has exactly these four values) | same |

The SOURCE row is inverted on purpose: a source returns items newest-first, `sourceOrder` is the index in that list (0 = newest), so "descending" (newest at the top, which is the default) is ascending `sourceOrder`.

`sortDescending` defaults to the entry's stored direction; callers needing playback order pass `sortDescending = false` explicitly (reader and player next-item logic, history "next chapter": `ReaderViewModel.kt:206`, `PlayerViewModel.kt:1340`, `GetNextChapters.kt:24`, `GetNextEpisodes.kt:24`), which means ascending number/date/name or DESCENDING `sourceOrder` (oldest first).

Numbers compare with `Double.compare` semantics: total order, `-0.0 < 0.0`, `NaN` greater than everything. Unrecognised numbers (-1.0) therefore sort before every real number ascending and after descending.

Ties: `sortedWith` is stable (Kotlin `List.sortedWith` = `java.util.Arrays.sort`/TimSort on objects, stable; Sequence version also stable), so equal keys keep the repository order. The repository order of `getChapterByMangaId` is unspecified SQL order (usually `_id`), so identical numbers (duplicates from two scanlators) appear in insertion order.

Proposal (Rust): `fn chapter_cmp(sorting: ChapterSorting, descending: bool) -> impl Fn(&Chapter, &Chapter) -> Ordering` (and episode equivalent via a shared generic over a small trait `Item { source_order(), number(), date_upload(), name() }`); use `slice::sort_by` (stable) and `f64::total_cmp` for numbers (identical to `Double.compare`). An unknown sorting value must not panic: fall back to SOURCE ordering and log a warning (the Kotlin `NotImplementedError` is unreachable for chapters/episodes).

## 2. Collator (`compareToWithCollator`)

OBSERVED (`SortUtil.kt:1-16`): a lazily created `java.text.Collator` for `Locale.getDefault()` at first use, `strength = Collator.PRIMARY`; `String.compareToWithCollator(other) = collator.compare(this, other)`. PRIMARY strength: differences of case and diacritics are ignored (`a`, `A`, `á` compare equal), only base letters count. It is NOT a natural sort: digits compare character by character, so "Chapter 10" sorts before "Chapter 2". The locale is captured once; changing the system language while the app runs does not rebuild it. `Collator` is documented as not thread-safe for concurrent mutation but `compare` is safe.

Proposal: `icu_collator` (ICU4X, pure Rust, compiled data) with `CollatorOptions { strength: Some(Strength::Primary), ..Default }` built once in a `LazyLock` from the user's locale (`LC_COLLATE` then `LANG`, via the `sys-locale` crate; fall back to `und`). Reasons: no C dependency, handles non-Latin scripts the way Android's ICU-backed `Collator` does (Android's `java.text.Collator` IS ICU, so ICU4X is closer to the original behaviour than JDK rules). If the dependency is judged too heavy for a solo project the fallback is `unicode-normalization` + `str::to_lowercase` + `Ord`, which loses locale-specific orders (document the loss: "Ä" would sort after "Z" in Swedish). Decision D3 in p1-topic-quirks-and-decisions.md.

Also used by season alphabetical sorting (section 3) and by library alphabetical sorts (area:08-domain-library-category-history-updates `AnimeLibraryScreenModel`/`MangaLibraryScreenModel`).

## 3. Season sorting

OBSERVED (`SeasonSorter.kt`):

```
val seasonSortAlphabetically: (SeasonAnime, SeasonAnime) -> Int =
    i1.anime.title.lowercase().compareToWithCollator(i2.anime.title.lowercase())

fun getSeasonSortComparator(anime: Anime): Comparator<SeasonAnime>  // ascending only
    when (anime.seasonSorting):
        SEASON_SORT_SOURCE   (0x0000) -> s1.anime.seasonSourceOrder compareTo s2.anime.seasonSourceOrder
        SEASON_SORT_SEASON   (0x2000) -> s1.anime.seasonNumber      compareTo s2.anime.seasonNumber       (Double)
        SEASON_SORT_UPLOAD   (0x4000) -> s1.latestUpload            compareTo s2.latestUpload
        SEASON_SORT_ALPHABET (0x6000) -> seasonSortAlphabetically(s1, s2)
        SEASON_SORT_COUNT    (0x8000) -> s1.unseenCount             compareTo s2.unseenCount
        SEASON_SORT_LAST_SEEN(0xA000) -> s1.lastSeen                compareTo s2.lastSeen
        SEASON_SORT_FETCHED  (0xC000) -> s1.fetchedAt               compareTo s2.fetchedAt
        else (0xE000)                 -> throw NotImplementedError("Invalid season sorting method: ...")
```

The caller (`AnimeScreenModel.kt:1709-1711`):

```
comparator = getSeasonSortComparator(anime)
    .let { if (anime.seasonSortDescending()) it.reversed() else it }
    .thenComparator(seasonSortAlphabetically)
```

So season order = chosen key (ascending, reversed when `seasonFlags & 1 == 0`) then alphabetical title ascending as a deterministic tie-break (the tie-break is NOT reversed). The default stored word (0) is SOURCE sorting with descending direction, i.e. highest `seasonSourceOrder` first (the reverse of the source list). Unlike chapters the SOURCE row here is not inverted: "descending" literally means the last source entry first. Preserve this asymmetry.

The ascending/descending "flip" UI for seasons uses the same state machine as chapters (p1-behavior.md 3.1.1).

Season filters (`AnimeScreenModel.kt:1713-1721`) run BEFORE sorting: unseen (`!season.seen`), started (`season.hasStarted`), completed (`season.anime.status == SAnime.COMPLETED`), bookmarked, fillermarked, downloaded (`downloadCount > 0 || isLocal`), each through `applyFilter`.

Proposal: `fn season_cmp(anime: &Anime) -> impl Fn(&SeasonAnime, &SeasonAnime) -> Ordering` ascending; `fn sort_seasons(list: &mut [SeasonAnime], anime: &Anime)` applies reversal then tie-break. The reachable `0xE000` state falls back to SOURCE with a `tracing::warn!` (Kotlin crashes).

## 4. Filter pipeline used by the details screen

Order (chapter list): `filter(unread)`, `filter(bookmarked)`, `filter(downloaded || local)`, then `sortedWith(getChapterSort(manga))`; the screen then inserts "missing count" separators between neighbours (p1-behavior.md section 7) on the SORTED list. The downloaded `TriState` is forced to `ENABLED_IS` when the global "downloaded only" mode is on (app layer). Also: `Manga.chaptersFiltered()`-style booleans (`unreadFilter != DISABLED || downloadedFilter != DISABLED || bookmarkedFilter != DISABLED`) decide whether the filter icon is highlighted; the episode version additionally includes fillermarked; the season version (`seasonsFiltered()`, `app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:207-214`) ORs the six season filters.

The episode filter order is unseen, bookmarked, fillermarked, downloaded then `sortedWith(getEpisodeSort(anime))` (`EpisodeFilter.kt`, area:10-app-domain-layer).

## 5. Tests

No upstream tests exist for sorters. New tests (p1-edge-cases-and-tests.md section 4): inversion of SOURCE, stable ties, NUMBER with -1 and 0.0, collator primary equality (`"Chapter 2a"` vs `"chapter 2A"` equal -> original order kept), 0xE000 fallback, season reversal with tie-break.
