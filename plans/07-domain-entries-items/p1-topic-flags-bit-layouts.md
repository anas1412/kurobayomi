# Area 07 — topic: bit-flag words (chapter/episode/season/viewer) — layouts, worked vectors, Rust representation

Sources: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:75-103`, `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:215-313`, `domain/src/main/java/tachiyomi/domain/library/model/Flag.kt`. The full constant tables with Kotlin names are in p1-data-and-config.md section 3; this file gives the bit maps, the invariants, vectors and the Rust mapping.

## 1. Bit maps (bit 0 = least significant)

```
Manga.chapterFlags (i64, bits 21-63 unused)
 bit: 20 | 19..10 | 9 8 | 7 6 5 | 4 3 | 2 1 | 0
      D  |  --    | S S |  B B  |  Dl | R R | dir        D = display (0 name, 1 number)
 SS = sorting (00 source, 01 number, 10 upload date, 11 alphabet)
 BB at bits 6,5: bit5 = SHOW_BOOKMARKED(0x20), bit6 = SHOW_NOT_BOOKMARKED(0x40)
 Dl bits 4,3: bit3 = SHOW_DOWNLOADED(0x08), bit4 = SHOW_NOT_DOWNLOADED(0x10)
 RR bits 2,1: bit1 = SHOW_UNREAD(0x02), bit2 = SHOW_READ(0x04)
 bit 0: direction (0 = DESC, 1 = ASC)

Anime.episodeFlags (i64)
 bit: 20 | 19..13 | 12 | 11 | 10 9 | 8 7 | 6 5 | 4 3 | 2 1 | 0
      D  |  --     | Su | Pv | S S | F F | B B | Dl  | U U | dir
 bit12 NOT_SUMMARIES(0x1000), bit11 NOT_PREVIEWS(0x800), SS sorting 00 source / 01 number(0x200) / 10 upload(0x400) / 11 alphabet(0x600),
 FF fillermark bit7 = SHOW(0x80) bit8 = SHOW_NOT(0x100), BB bookmark, Dl download, UU unseen as above.

Anime.seasonFlags (i64)
 bit: 27 | 26 | 25 | 24 | 23 | 22 | 21..18 | 17 16 | 15 14 13 | 12 11 | 10 9 | 8 7 | 6 5 | 4 3 | 2 1 | 0
      Dsp| Cn | Lg | Lc | Un | Dl | size   | mode  | sort type| Fm    | Bm   | Cp  | St  | Un  | Dl  | dir
 (Dsp = title display mode, Cn = continue overlay, Lg = language overlay, Lc = local overlay, Un = unseen overlay,
  Dl(22) = downloaded overlay; Fm fillermarked filter, Bm bookmarked, Cp completed, St started, Un(4:3) unseen filter, Dl(2:1) downloaded filter)

Anime.viewerFlags (i64)
 bit: 63-57  | 56 | 55-24            | 23-8          | 7-0
      --     | Dis| airing time (32b)| airing ep(16b)| intro seconds (8b)
```

Invariants: groups are disjoint; the unused bits stay whatever they were (setFlag never touches bits outside the mask); a two-bit filter group value `11` is possible only through corruption or a foreign backup and reads as DISABLED.

## 2. Worked vectors (computed, hex)

setFlag(v, flag, mask) = (v & !mask) | (flag & mask).

| Start | Operation | Result |
|---|---|---|
| episode 0x000 | sort by NUMBER (not current) | 0x201 (number 0x200, direction forced ASC 0x1) |
| 0x201 | same sort again (flip) | 0x200 |
| 0x200 | same sort again | 0x201 |
| 0x200 | unseen filter = SHOW_UNSEEN (0x2) | 0x202 |
| 0x202 | bookmarked filter = SHOW_NOT_BOOKMARKED (0x40) | 0x242 |
| 0x242 | fillermarked filter = SHOW_NOT_FILLERMARKED (0x100) | 0x342 |
| 0x342 | previews NOT (0x800), summaries NOT (0x1000), display NUMBER (0x100000) | 0x101B42 |
| season 0 | grid mode List (3 << 16 = 0x30000), size 5 (5 << 18 = 0x140000), overlays unseen (0x800000) + local (0x1000000) + continue (0x4000000) | 0x5970000 |
| season 0 | sort by SEASON (0x2000) | 0x2001 (ASC forced) |
| viewer 0 | intro length 90 (0x5A) ; enable bit cleared ; next ep 13 (13 << 8 = 0xD00) ; airing time 1760000000 (<< 24) | 0x68E77800000D5A |
| 0x68E77800000D5A | `awaitSetSkipIntroLength(0)` | intro bits 0, disable bit set (1 << 56): 0x168E77800000D00 |

Season sort-type codes: SOURCE 0x0000, SEASON 0x2000 (0b010 << 13), UPLOAD 0x4000, ALPHABET 0x6000, COUNT 0x8000, LAST_SEEN 0xA000, FETCHED 0xC000; the code 0xE000 is unassigned (crash in Kotlin, Q8).

Default-pref flags: with all `default_*` preferences at their defaults, `SetAnimeDefaultEpisodeFlags` writes `episodeFlags = 0`; `SetMangaDefaultChapterFlags` writes `chapterFlags = 0`; `SetAnimeDefaultSeasonFlags` writes `seasonFlags = 0x5800000` (derivation in the next paragraph).

Season default value: grid mode CompactGrid (0), size 0, download overlay false, unseen overlay true (0x800000), local overlay true (0x1000000), lang overlay false, continue overlay true (0x4000000), display mode SOURCE (0), all filters 0, sort SOURCE, direction DESC: `0x800000 | 0x1000000 | 0x4000000 = 0x5800000`. (The 0x5970000 row above is a different example with a List grid and size 5.)

## 3. Rust representation (Proposal)

Multi-bit enumerated groups do not fit `bitflags!` (it models independent bits). Use plain constants plus a newtype with typed accessors:

```rust
// kuro-core/src/domain/flags.rs
pub const fn set_flag(v: i64, flag: i64, mask: i64) -> i64 { (v & !mask) | (flag & mask) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TriState { Disabled, EnabledIs, EnabledNot }
impl TriState { pub fn next(self) -> Self { /* Disabled -> Is -> Not -> Disabled */ } }

/// Maps a 2-bit filter group to a TriState; value 0b11 and 0 map to Disabled.
pub const fn tri(raw: i64, is: i64, not: i64) -> TriState {
    if raw == is { TriState::EnabledIs } else if raw == not { TriState::EnabledNot } else { TriState::Disabled }
}

// kuro-core/src/domain/entries/manga.rs
pub mod chapter_flags {
    pub const SORT_DESC: i64 = 0x0;        pub const SORT_ASC: i64 = 0x1;       pub const SORT_DIR_MASK: i64 = 0x1;
    pub const SHOW_UNREAD: i64 = 0x2;      pub const SHOW_READ: i64 = 0x4;      pub const UNREAD_MASK: i64 = 0x6;
    pub const SHOW_DOWNLOADED: i64 = 0x8;  pub const SHOW_NOT_DOWNLOADED: i64 = 0x10; pub const DOWNLOADED_MASK: i64 = 0x18;
    pub const SHOW_BOOKMARKED: i64 = 0x20; pub const SHOW_NOT_BOOKMARKED: i64 = 0x40; pub const BOOKMARKED_MASK: i64 = 0x60;
    pub const SORTING_SOURCE: i64 = 0x0;   pub const SORTING_NUMBER: i64 = 0x100;
    pub const SORTING_UPLOAD_DATE: i64 = 0x200; pub const SORTING_ALPHABET: i64 = 0x300; pub const SORTING_MASK: i64 = 0x300;
    pub const DISPLAY_NAME: i64 = 0x0;     pub const DISPLAY_NUMBER: i64 = 0x10_0000; pub const DISPLAY_MASK: i64 = 0x10_0000;
}
```

Same shape for `episode_flags`, `season_flags`, `viewer_flags` (constants copied from p1-data-and-config.md section 3). Typed enums for the "sorting" groups (`ChapterSorting::{Source, Number, UploadDate, Alphabet}`) convert from the masked word with `from_flags(word) -> Option<Self>` (None for unknown, impossible for chapters/episodes, possible for seasons). Accessors are `impl Manga { pub fn unread_filter(&self) -> TriState; pub fn sorting(&self) -> i64; pub fn sort_descending(&self) -> bool; }` (the complete accessor list is in p1-rust-design.md 3.2) in `kuro-core/src/domain/entries/`. Property tests: for every group and every legal value, `set_flag` then read-back returns the value and leaves all other bits unchanged (p1-edge-cases-and-tests.md).

Persist and compare the words as `i64` exactly (SQLite INTEGER); a backup restore (the backup series (no plans folder yet)) copies them verbatim, which makes the bit layout a file-format contract: never renumber a constant.

Do not use `as` casts to build the season grid mode: write `(mode.to_i64() << 16)` through a function `season_grid_mode_flag(mode) -> i64` and read with `SeasonDisplayMode::from_i64((word & MODE_MASK) >> 16)`.
