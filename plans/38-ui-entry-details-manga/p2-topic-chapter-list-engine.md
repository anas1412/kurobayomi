# Topic: chapter list engine (sort, filter, separators, selection, next unread)

Deep dive for an engineer who implements `kuro-chapters` without opening the Kotlin. Every rule cites its source; the full specification is in p2-behavior.md, this file gives the order of operations and worked examples.

Sources: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt` (MSM), `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt`, `app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt`, `domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt`, `app/src/main/java/eu/kanade/core/util/CollectionUtils.kt`.

## 1. Data flow (one direction)

```
DB chapters (any order, scanlator-excluded rows removed in SQL)
  + download queue state + download cache       (MSM:175-189, 533-564)
  -> raw rows: Vec<ChapterRow>                    (never reordered, never replaced by the view)
  -> view = apply_view(raw, flags, is_local, downloaded_only, collate)   (ChapterFilter.kt:42-52)
       1. keep unread per TriState                 (TriState.kt:5-9)
       2. keep bookmarked per TriState
       3. keep downloaded per TriState, local rows count as downloaded
       4. stable sort by getChapterSort(flags)     (ChapterSorter.kt:7-30)
  -> separators = insertSeparators(view)           (CollectionUtils.kt:8-21, MSM:1165-1190)
  -> list rows = view rows + separator rows
```

Rule 1: the raw list is the source of truth. The upstream bug (E-SEL-1) replaced it with the view inside `toggleSelection`; the port must not.
Rule 2: view indices are recomputed when rows, flags, or download states that affect "downloaded" change. Otherwise only the changed row is updated.

## 2. Sort in five steps (worked example)

Input (raw DB order), each row as `(id, number, source_order, read)`:

| id | number | source_order |
|---|---|---|
| 101 | 3 | 2 |
| 102 | 1 | 4 |
| 103 | 2 | 3 |
| 104 | 5 | 0 |
| 105 | 4 | 1 |

Defaults: sorting SOURCE (0x000), direction DESC (bit 0x1 = 0).

1. Read the comparator key: sorting = SOURCE, descending = true.
2. SOURCE + descending uses ascending `source_order` (ChapterSorter.kt:12-13): sequence `104(0), 105(1), 101(2), 103(3), 102(4)`.
3. The view is `[104, 105, 101, 103, 102]`, which shows numbers `5, 4, 3, 2, 1`: newest first, because source index 0 is the newest.
4. Switch to NUMBER (0x100): the setter flips nothing because the sort key differs; it sets the key and direction ASC (MSM:943-949, SetMangaChapterFlags.kt:47-70). View: `102(1), 103(2), 101(3), 105(4), 104(5)`.
5. Choose NUMBER again: the setter flips the direction to DESC. View: `104(5), 105(4), 101(3), 103(2), 102(1)`.

Comparators by row (from ChapterSorter.kt):

| sorting | descending | comparator key |
|---|---|---|
| SOURCE 0x000 | true | `source_order` asc |
| SOURCE 0x000 | false | `source_order` desc |
| NUMBER 0x100 | true | `chapter_number` desc |
| NUMBER 0x100 | false | `chapter_number` asc |
| UPLOAD 0x200 | true | `date_upload` desc |
| UPLOAD 0x200 | false | `date_upload` asc |
| ALPHABET 0x300 | true | `name` desc (collator, primary) |
| ALPHABET 0x300 | false | `name` asc (collator, primary) |

`chapter_number` ties are not broken by id (upstream). Stability keeps the DB order for ties. The port keeps this (stable sort on the raw order); the golden test fixes the raw order.

## 3. Filter tri-states (worked example)

Row set: `A(read=false, bookmark=false, downloaded=false)`, `B(read=true, bookmark=true, downloaded=true)`, `C(read=false, bookmark=true, downloaded=false)`.

| Unread | Bookmarked | Downloaded | Result |
|---|---|---|---|
| Disabled | Disabled | Disabled | A, B, C |
| EnabledIs | Disabled | Disabled | A, C |
| EnabledNot | Disabled | Disabled | B |
| Disabled | EnabledIs | Disabled | B, C |
| Disabled | EnabledNot | Disabled | A |
| Disabled | Disabled | EnabledIs | B (plus local rows) |
| Disabled | Disabled | EnabledNot | A, C |

Downloaded uses `DownloadState::Downloaded` only. A QUEUE row is not downloaded; a local manga row is always downloaded (MSM:1213, ChapterFilter.kt:32).

## 4. Separators (worked example)

Ascending view numbers `[5, 6, 8.5, 10]`:

| position | before | after | lower | higher | count | output |
|---|---|---|---|---|---|---|
| start | none | 5 | none | 5 | floor(5) - 1 = 4 | separator "4 missing" before 5 |
| 5-6 | 5 | 6 | 5 | 6 | 0 | none |
| 6-8.5 | 6 | 8.5 | 6 | 8.5 | floor(8.5)-6-1 = 1 | separator before 8.5 |
| 8.5-10 | 8.5 | 10 | 8.5 | 10 | floor(10)-8-1 = 1 | separator before 10 |
| end | 10 | none | (none, higher = before) | - | not computed | none |

Descending view `[10, 8.5, 6, 5]` (the same chapters reversed):

| position | before | after | lower = after | higher = before | count | output |
|---|---|---|---|---|---|---|
| start | none | 10 | 10 | none | not computed | none |
| 10-8.5 | 10 | 8.5 | 8.5 | 10 | floor(10)-floor(8.5)-1 = 1 | separator before 8.5 |
| 8.5-6 | 8.5 | 6 | 6 | 8.5 | floor(8.5)-6-1 = 1 | separator before 6 |
| 6-5 | 6 | 5 | 5 | 6 | 0 | none |
| end | 5 | none | lower = none | higher = 5 | floor(5)-1 = 4 | separator "4 missing" after 5 |

Interpretation: in descending order the "before the first chapter" gap is never shown; the gap below the last displayed chapter is shown at the bottom. This is the upstream behavior and the port keeps it.

Negative numbers (`-1.0`) contribute 0 to every gap (E-GAP-1), and the start-gap formula clamps at 0.

Test vector equivalence: `missing_count([-1, 1, 2, 2.2, 4, 6, 10, 10]) == 5` (MissingItemsTest). The separator counts above sum to the same missing count when no separator is cut off by filters.

## 5. Selection (worked example)

View order ids `[1..=10]`, nothing selected, anchors none.

1. Long press 3 (`LongPress`, selected): first selection. Set `{3}`, anchors `(3, 3)`.
2. Long press 7: range from anchor high (3) + 1 to 7 - 1 = 4..=6 added; anchors become `(3, 7)`. Set `{3,4,5,6,7}`.
3. Tap 5 (`Tap`, deselect): remove 5. Anchors unchanged (5 is not an anchor). Set `{3,4,6,7}`.
4. Tap 3 (`Tap`, deselect at the low anchor): low anchor moves to the first selected in view order, which is 4. Set `{4,6,7}`.
5. Long press 1 (`LongPress`, selected): 1 < low anchor 4, so range is `2..4` exclusive of 4 -> `{2,3}` added; anchors `(1, 7)`. Set `{1,2,3,4,6,7}`.
6. Select all (raw): adds every raw id; anchors reset to none.
7. Invert (raw): every id flips.

Every step is a pure function of (view ids, selection, anchors). Anchors are chapter ids and are looked up in the current view; if an anchor is not in the view (filtered out), the range falls back to the selected chapter alone (port rule, E-SEL-2 safety).

## 6. Next unread and download picks (worked example)

Rows (u = unread, r = read, ND = NotDownloaded): `5r`, `4u(ND)`, `3u(Error)`, `2r`, `1u(ND)`, `7u(ND)`. With NUMBER descending the view is `7u, 5r, 4u, 3u, 2r, 1u`.

- Next unread (descending): last unread in view order = `1` (ChapterGetNextUnread.kt:41-46). Result: chapter 1.
- Unread and not downloaded (`getUnreadChapters`): `7u(ND), 4u(ND), 1u(ND)`. `3u` is ERROR and is excluded (E-DL-3).
- Unread sorted oldest first (`getUnreadChaptersSorted`): sort the three by NUMBER desc gives `7, 4, 1`; the descending reverse gives `1, 4, 7`.
- NEXT_1 = `[1]`, NEXT_5 = `[1, 4, 7]`, UNVIEWED = `[7, 4, 1]` (unsorted view order).

## 7. Mark previous as read (worked example)

View (NUMBER desc) `[9, 8, 7, 6]`, pointer 7.

1. Chronological list (descending reverses): `[6, 7, 8, 9]`.
2. Index of pointer 7 is 1.
3. Mark read `take(1)` = `[6]`.

The pointer itself is not marked. Matching is by id (E-PREV-1).

## 8. Tests this file implies

See p2-edge-cases-and-tests.md section 3.2 to 3.6 for named tests. The worked examples above are the first golden cases: sort (section 2, both directions, four sorts), filter (section 3, seven rows), separators (section 4, both directions), selection (section 5, seven steps), next unread and download picks (section 6), mark previous (section 7).
