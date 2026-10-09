# Topic: chapter list engine (filter, sort, missing markers, selection, FAB rules)

Deep dive for the subsystem that turns `Chapter` rows into the list the details screen shows. Sources: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt` (State.Success, lines 1138-1243), `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt`, `domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt`, `domain/src/main/java/tachiyomi/domain/entries/TriState.kt`, `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/SetMangaChapterFlags.kt`, and the presentation files in scope.

## 1. Inputs and outputs

```
inputs:   chapters (DB subscription, scanlator-filtered)   [Vec<ChapterItem>]
          manga.chapterFlags (Long)                        [ChapterFlags]
          downloaded_only (preference)                     [bool]
          is_local (manga source == local id)              [bool]
          selected ids (model-owned set)                   [BTreeSet<i64>]

outputs:  processed: Vec<ChapterItem>        (filtered + sorted)
          rows: Vec<ListRow>                 (processed items + Missing markers)
          header_count = processed.len()
          missing_warning = missing_chapters_count(processed numbers)
          fab_visible = processed.any(!read) && !any_selected_total
          fab_label = if chapters.any(read) "Resume" else "Start"
          action_mode = selected_visible > 0
          bulk_bar_visible = selected_visible > 0
```

Two selected counters are needed because filters can hide selected items (edge E-04):
- `selected_total` = count over the unfiltered list (drives back-handler, pull-refresh disable, FAB hiding).
- `selected_visible` = count over the processed list (drives toolbar action mode and bulk bar visibility).

## 2. Pipeline, step by step

1. Read `flags = manga.chapterFlags`.
2. `unread = flags.unread()`, `bookmarked = flags.bookmarked()`, `downloaded = if downloaded_only { EnabledIs } else { flags.downloaded_raw() }`.
3. Keep items where `unread.keep(!read)`.
4. Keep items where `bookmarked.keep(bookmark)`.
5. Keep items where `downloaded.keep(state == Downloaded || is_local)`.
6. Stable-sort by `compare(flags.sort_mode(), flags.sort_descending(), a, b)`.
7. Build `rows` by inserting Missing markers between adjacent processed items (and before the first and after the last position, where only the "before first" can produce a marker).

Worked example A (default flags `0`, source sort, desc=true):

| source order | number | read | bookmark | state | kept? |
|---|---|---|---|---|---|
| 0 | 12 | false | false | Downloaded | yes |
| 1 | 11 | true | false | NotDownloaded | yes |
| 2 | 10 | false | true | NotDownloaded | yes |

Sort SOURCE with `sortDescending == true` is ascending sourceOrder: result `0, 1, 2` = numbers `12, 11, 10` (newest first from most sources). This is why default lists look newest first.

Worked example B (flags = `0x101` = number, ASC):
- `sortDescending() == false` → SOURCE/NUMBER branch `c1.number.compareTo(c2.number)` ascending: `10, 11, 12`.

Worked example C (unread filter EnabledIs, flags `0x2`): keeps only read == false: from A keeps sourceOrder 0 and 2.

## 3. Missing markers, in detail

`with_missing_markers(list, desc)` iterates positions `0..=len`:

- `before = list[i-1]` (None at i=0), `after = list[i]` (None at i=len).
- `(lower, higher) = if desc { (after, before) } else { (before, after) }`.
- No marker if `higher` is None (i.e., at the end in ascending order, or at the start in descending order).
- If `lower` is None: `count = max(floor(higher.number) - 1, 0)` (chapters below the first visible chapter).
- Else `count = floor(higher.number) - floor(lower.number) - 1` when both numbers are ≥ 0 and recognised, else 0.
- Emit `Missing { id: "{lower.id or 'null'}-{higher.id}", count }` when `count > 0`.

Worked example D (ascending, numbers `[1, 2, 5, 6, 9]`):
- Position 0: before None, after 1 → lower None, higher 1 → count `max(1-1,0)=0` → none.
- Between 2 and 5: count `floor(5)-floor(2)-1 = 2` (3, 4) → marker `"<id2>-<id5>"` count 2.
- Between 6 and 9: count `9-6-1 = 2` (7, 8) → marker `"<id6>-<id9>"` count 2.
- End: higher None → none.
- Header missing warning: `missing_chapters_count([1,2,5,6,9]) = 4` (3,4,7,8). Note the header total (4) equals the sum of markers (2+2) here, but the header count is computed independently over the processed numbers (it also counts gaps before 1, e.g. list `[3,4]` gives header 2 and a first marker of 2).

Worked example E (descending, numbers `[9, 6, 5, 2, 1]` in list order):
- Between 6 and 5: `desc=true` → lower = after (5), higher = before (6): count `6-5-1 = 0` → none.
- Between 5 and 2: lower = 2, higher = 5: count `5-2-1 = 2` → marker `"<id2>-<id5>"`.
- Note the id uses lower first: `"<id2>-<id5>"` in both directions. Keep.

## 4. Header and text rules

- Header count: `processed.len()` (never the null "Chapters" fallback, edge E-01).
- Missing warning text: `pluralStringResource(AYMR.plurals.missing_items, count)` → "Missing 1 item" / "Missing N items". Hidden when count is 0.
- Row title: display NAME → `chapter.name`; NUMBER → `"Chapter {format_chapter_number(number)}"`.
- Subtitle: `[date] • [Page: N] • [scanlator]`, separators only between present parts (MangaChapterListItem.kt:141-173). Page is `last_page_read + 1`, shown only when `!read && last_page_read > 0`.

## 5. Selection rules

Model-owned set `selected_ids` and per-item `selected` flags derived from it on every snapshot (`selected = chapter.id in selectedChapterIds`, MangaScreenModel.kt:563).

Interactions (UI → model):
- Row long-press (non-selection): select item; anchor = its processed index (range anchor set). Range selection (part 2) uses `selectedPositions`.
- Row click in selection mode: toggle item.
- Row click outside selection mode: open reader.
- Select all: all processed items selected (`toggleAllSelection(true)`); invert: flip.
- Back (Escape): if `selected_total > 0` clear all; else pop.

Edge: when the filter hides a selected item, it stays in `selected_ids` (counted in `selected_total`) but is not visible. Its bulk actions are not offered (the bar uses the visible selection). Choose one policy and test it (proposal: clear hidden selection when filters change, so the user never acts on invisible items; labelled new behaviour, default is Aniyomi behaviour = keep).

## 6. Sort and filter writes

- Sort click on a row: `flags = flags.select_sort(mode)` (flip if same mode; else new mode + ASC). One write through `SetMangaChapterFlags.awaitSetSortingModeOrFlipOrder`.
- Filter click (TriState): `flags.with(next_bits, mask)` with the bits of `TriState.next()` mapped as in `SetMangaChapterFlags` (unread: IS=0x2, NOT=0x4, DISABLED=0; bookmarked: IS=0x20, NOT=0x40; downloaded: IS=0x8, NOT=0x10).
- Display click: `flags.with(DISPLAY_NUMBER or 0, DISPLAY_MASK)`.
- Set as default: store the current flags as library defaults; optional apply to all manga (part 2).
- Reset: apply library defaults to this manga (part 2 `setMangaDefaultChapterFlags.await(manga)`).

All writes must be serialized per manga (failure mode table in edge-cases section 2).

## 7. Performance

- The Kotlin code recomputes `processedChapters` lazily once per `State.Success` instance and caches via `by lazy`. Rust: recompute on snapshot and on filter/sort change only; store `processed` and `rows` in the model; the `ListView` model is updated with a diff (`splice`) rather than rebuilt.
- 10 000 items target: filter + sort + marker build under 20 ms in release (test in edge-cases 4.11).

## 8. Portability notes

- `is_recognized_number` uses `>= 0`, so `-1.0` (unknown) is not recognised; `0.0` is recognised (chapter 0 exists).
- Integer conversions use truncation toward zero (Kotlin `toInt()`), which Rust `as i64` matches.
- Collation: primary strength (accents and case ignored at equal base letters). See rust design 2.2.
