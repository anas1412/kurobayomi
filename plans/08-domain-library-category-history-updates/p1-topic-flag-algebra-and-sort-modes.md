# Area 08 — Topic: flag algebra and library sort modes

Sources: `domain/src/main/java/tachiyomi/domain/library/model/Flag.kt`, `domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt`, `domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt`, test `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt`, consumers `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt:201-330` and the anime twin.

## 1. What is encoded where

A library sort is stored in two ways:

1. **Global**: the preference `library_sorting_mode` (manga) / `animelib_sorting_mode` (anime) as a string `"<TYPE>,<DIRECTION>"`; used as the template for new categories and as the value `ResetXCategoryFlags` copies into every category.
2. **Per category**: the integer column `categories.flags` (the sort lives in bits 2-6). The library tab sorts each category with `MangaLibrarySort.valueOf(category.flags)`. When "per-category settings for sort" (`categorized_display`) is off, every category simply carries the same flags (the global value is written to all categories on every change), so the tab logic never needs to look at the global preference.

## 2. Full flag table (manga, 10 types x 2 directions)

| Type | Descending flag | Ascending flag |
|---|---|---|
| Alphabetical | 0 (0x00, `0b0000000`) | 64 (0x40, `0b1000000`) |
| LastRead | 4 (0x04, `0b0000100`) | 68 (0x44, `0b1000100`) |
| LastUpdate | 8 (0x08, `0b0001000`) | 72 (0x48, `0b1001000`) |
| UnreadCount | 12 (0x0C, `0b0001100`) | 76 (0x4C, `0b1001100`) |
| TotalChapters | 16 (0x10, `0b0010000`) | 80 (0x50, `0b1010000`) |
| LatestChapter | 20 (0x14, `0b0010100`) | 84 (0x54, `0b1010100`) |
| ChapterFetchDate | 24 (0x18, `0b0011000`) | 88 (0x58, `0b1011000`) |
| DateAdded | 28 (0x1C, `0b0011100`) | 92 (0x5C, `0b1011100`) |
| TrackerMean | 32 (0x20, `0b0100000`) | 96 (0x60, `0b1100000`) |
| Random | 60 (0x3C, `0b0111100`) | 124 (0x7C, `0b1111100`) |

Anime is the same with the renames of behaviour section 2.5 and one extra row: **AiringTime** = 48 (0x30) descending / 112 (0x70) ascending. Anime therefore has 11 types (`AnimeLibrarySort.types.size == 11`, manga 10, directions 2 each, display modes 4 — asserted by `LibraryFlagsTest."Check the amount of flags"`).

Decode rule (`valueOf(flag: Long?)`): `null -> default (Alphabetical, Ascending)`. Else `type = first type with (flag and 0x3C) == type.flag`, falling back to Alphabetical when none matches (unassigned type patterns: manga 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38; anime 0x24, 0x28, 0x2C, 0x34, 0x38 because 0x30 is AiringTime there); `direction = Ascending if (flag and 0x40) != 0 else Descending`. Bits outside `0x7C` are ignored (but preserved when a flag is rewritten with `Long + Flag`).

Worked decodes: `0` -> (Alphabetical, Descending); `64` -> (Alphabetical, Ascending); `0x24` -> (Alphabetical, Descending); `0x3C` -> (Random, Descending); `255` -> (Random, Ascending); `128` -> (Alphabetical, Descending); `0x30` -> manga (Alphabetical, Descending), anime (AiringTime, Descending).

## 3. Operator semantics with worked examples

Notation: `a + b` where `b` is a mask-carrying flag = `(a & !b.mask) | (b.flag & b.mask)`.

| Expression (Kotlin) | Computation | Result |
|---|---|---|
| `MangaLibrarySort(LastRead, Ascending) + MangaLibrarySort(DateAdded, Ascending)` (`Flag.plus(Flag)`; test 2) | left.flag = `0x04|0x40 = 0x44`; right.mask = `0x3C|0x40 = 0x7C`; `(0x44 & !0x7C) | (0x5C & 0x7C) = 0x00 | 0x5C` | `0b01011100` = 92 |
| `AnimeLibrarySort(LastSeen, Ascending) + AnimeLibrarySort(DateAdded, Ascending)` | same numbers | `0b01011100` |
| `MangaLibrarySort(UnreadCount, Descending).flag + MangaLibrarySort(DateAdded, Ascending)` (`Long.plus(Flag)`; test 3) | `(0x0C & !0x7C) | (0x5C & 0x7C) = 0 | 0x5C` | `0b01011100`; `!= 0x0C` |
| `MangaLibrarySort.default.type + default.direction` (`Flag.plus(Flag)` with the Direction mask; test 4) | type flag 0 `& !0x40 | (0x40 & 0x40)` | `0b01000000` = 64 |
| `(category?.flags ?: 0) + type + direction` (`SetSortModeForXCategory`) with `flags = 0x44`, type `UnreadCount`, direction `Descending` | step 1: `0x44 + UnreadCount` = `(0x44 & !0x3C) | (0x0C & 0x3C) = 0x40 | 0x0C = 0x4C`; step 2: `0x4C + Descending` = `(0x4C & !0x40) | (0x00 & 0x40) = 0x0C` | 12 |
| `0x45 + DateAdded + Ascending` (low bit set) | step 1 `(0x45 & !0x3C) | 0x1C = 0x41 | 0x1C = 0x5D`; step 2 `0x5D | 0x40 = 0x5D` | 0x5D (bit 0 preserved) |
| `Long.contains(flag)`: `0x5C in DateAdded` (Type is a `Mask`) | `DateAdded.flag (0x1C) == (0x5C & 0x3C = 0x1C)` | true |
| `0x5C in Ascending` | `0x40 == (0x5C & 0x40 = 0x40)` | true |
| `0x1C in Ascending` | `0x40 == 0x00` | false |

Precision note: in Kotlin `a and b.inv() or (c and d)` is evaluated strictly left to right with equal infix precedence, which equals `((a and b.inv()) or (c and d))`; the Rust version writes the parentheses explicitly.

## 4. Rust reference implementation (Proposal; this is the code the tests of T-08a-002/003 pin down)

One `SortType` enum serves both kinds (the numeric flags are identical; only the string tokens and the existence of `AiringTime` differ).

```rust
// kuro-domain::library::flags
/// Kotlin `Long.plus(other: Flag)` when `other` carries a mask.
pub const fn merge(value: i64, flag: i64, mask: i64) -> i64 { (value & !mask) | (flag & mask) }
/// Kotlin `Long.contains(other: Flag)` when `other` carries a mask.
pub const fn contains(value: i64, flag: i64, mask: i64) -> bool { flag == (value & mask) }

// kuro-domain::library::sort
pub const SORT_TYPE_MASK: i64 = 0b0011_1100;
pub const SORT_DIR_MASK:  i64 = 0b0100_0000;
pub const SORT_MASK:      i64 = SORT_TYPE_MASK | SORT_DIR_MASK;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Direction { Ascending, Descending }
impl Direction {
    pub const ALL: [Direction; 2] = [Direction::Ascending, Direction::Descending];
    pub const fn flag(self) -> i64 { match self { Self::Ascending => 0b0100_0000, Self::Descending => 0 } }
    pub const fn from_flags(f: i64) -> Self { if f & SORT_DIR_MASK != 0 { Self::Ascending } else { Self::Descending } }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum SortType {
    Alphabetical, LastActivity /* LastRead | LastSeen */, LastUpdate, UnreadCount /* UnreadCount | UnseenCount */,
    TotalItems, LatestItem, ItemFetchDate, DateAdded, TrackerMean, AiringTime /* anime only */, Random,
}
impl SortType {
    /// Declaration order of `MangaLibrarySort.types` / `AnimeLibrarySort.types`.
    pub const MANGA: [SortType; 10] = [Self::Alphabetical, Self::LastActivity, Self::LastUpdate, Self::UnreadCount,
        Self::TotalItems, Self::LatestItem, Self::ItemFetchDate, Self::DateAdded, Self::TrackerMean, Self::Random];
    pub const ANIME: [SortType; 11] = [Self::Alphabetical, Self::LastActivity, Self::LastUpdate, Self::UnreadCount,
        Self::TotalItems, Self::LatestItem, Self::ItemFetchDate, Self::DateAdded, Self::TrackerMean, Self::Random, Self::AiringTime];
    pub const fn flag(self) -> i64 {
        match self {
            Self::Alphabetical => 0b0000_0000, Self::LastActivity => 0b0000_0100, Self::LastUpdate => 0b0000_1000,
            Self::UnreadCount => 0b0000_1100,  Self::TotalItems => 0b0001_0000,   Self::LatestItem => 0b0001_0100,
            Self::ItemFetchDate => 0b0001_1000, Self::DateAdded => 0b0001_1100,   Self::TrackerMean => 0b0010_0000,
            Self::AiringTime => 0b0011_0000,   Self::Random => 0b0011_1100,
        }
    }
    pub fn from_flags(kind: EntryKind, f: i64) -> Self {
        let set: &[SortType] = if kind == EntryKind::Anime { &Self::ANIME } else { &Self::MANGA };
        set.iter().copied().find(|t| t.flag() == f & SORT_TYPE_MASK).unwrap_or(Self::Alphabetical)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct LibrarySort { pub ty: SortType, pub dir: Direction }
impl LibrarySort {
    pub const DEFAULT: Self = Self { ty: SortType::Alphabetical, dir: Direction::Ascending };
    /// `type + direction` (Kotlin `Flag.plus(Flag)` with the direction mask).
    pub const fn flag(self) -> i64 { merge(self.ty.flag(), self.dir.flag(), SORT_DIR_MASK) }
    pub fn from_flags(kind: EntryKind, f: Option<i64>) -> Self {
        match f { None => Self::DEFAULT, Some(f) => Self { ty: SortType::from_flags(kind, f), dir: Direction::from_flags(f) } }
    }
    pub fn is_ascending(self) -> bool { self.dir == Direction::Ascending }
    /// Kotlin `(category.flags) + type + direction`: replaces bits 0x7C only.
    pub const fn apply_to(self, category_flags: i64) -> i64 {
        merge(merge(category_flags, self.ty.flag(), SORT_TYPE_MASK), self.dir.flag(), SORT_DIR_MASK)
    }
}
```

`LibrarySort::flag()` equals the Kotlin `flag` property; the Kotlin `mask` property (`type.mask or direction.mask`) is `SORT_MASK`. `apply_to` is exactly `(flags) + type + direction`.

## 5. String codec rules (both kinds)

Serialize: `"<TYPE>,<DIRECTION>"`, `ASCENDING` / `DESCENDING`. Deserialize algorithm (never fails):

```
if s.is_empty():            return DEFAULT
parts = s.split(',')                           // Kotlin split keeps empty pieces; "A,B,C" has 3 parts, only parts[0], parts[1] are read
type = token table of the kind (section 2 of p1-data-and-config.md), unknown token => Alphabetical
if parts.len() < 2:        return DEFAULT        // Kotlin: values[1] throws -> catch -> default (note: NOT the parsed type)
dir  = if parts[1] == "ASCENDING" { Ascending } else { Descending }
```

Case sensitivity: exact, uppercase. A string `"LAST_READ"` without a direction returns DEFAULT (Alphabetical, Ascending), not LastRead. Anime parser knows `LAST_SEEN`, `LAST_ANIME_UPDATE`, `UNSEEN_COUNT`, `TOTAL_EPISODES`, `LATEST_EPISODE`, `EPISODE_FETCH_DATE`, `AIRING_TIME`; manga parser knows `LAST_READ`, `LAST_MANGA_UPDATE`, `UNREAD_COUNT`, `TOTAL_CHAPTERS`, `LATEST_CHAPTER`, `CHAPTER_FETCH_DATE`; both know `ALPHABETICAL`, `DATE_ADDED`, `TRACKER_MEAN`, `RANDOM`. Because the manga parser does not know the anime tokens (and vice versa) a cross-kind string degrades to Alphabetical.

## 6. Sort application checklist (maps to T-08a-015)

1. Per category: derive `sort` from that category's flags.
2. Random: shuffle with the persisted seed (stable until the seed changes); no direction, no tie-break.
3. Else comparator by type, reversed if descending, then tie-break by lowercased-title collation ascending.
4. UnreadCount / UnseenCount: zero counts always last — implement exactly as the Kotlin `when` so the `reversed()` interplay holds: for ascending, items with count 0 compare greater than non-zero items (sort last); for descending the comparator returns `-1` for `i1 == 0`, which after `reversed()` becomes `+1`, i.e. zeros still last. Test vectors in `p1-edge-cases-and-tests.md`.
5. AiringTime: sort by unseen count first (ascending numeric, then reversed for descending), then by `nextEpisodeAiringAt` with 0 (unknown) last in both directions, same trick as (4).
