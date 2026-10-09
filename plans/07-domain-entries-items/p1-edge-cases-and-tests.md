# Area 07 — edge cases, failure modes, races, tests

## 1. Edge cases and failure modes

### 1.1 Flags

| Case | Behaviour | Test |
|---|---|---|
| Filter group value `0b11` (e.g. `chapterFlags & 0x6 == 0x6`) | reads as DISABLED (no `when` branch matches) | `tri(0x6, 0x2, 0x4) == Disabled` |
| Unknown bits set (bits 10-19 on a chapter word, foreign backup) | untouched by every setter | property test |
| `awaitSetAllFlags` starts from 0 | any unknown bits previously present are cleared | `all_flags_clears_unknown_bits` |
| Same sort tapped twice then a different sort | second tap flips direction; third (different column) forces ASC | vectors in p1-topic-flags-bit-layouts.md 2 |
| Sort word for episodes `0x600` but flags built with the manga constant `0x300` | would silently select NUMBER+UPLOAD bits of another group; constants are per type, never shared | compile-time separation by module |
| Season grid size > 15 | masked to low 4 bits upstream; Rust clamps first (Q21) | `grid_size_16_is_clamped_to_15` |
| Season sort code `0xE000` | Kotlin throws; Rust falls back to SOURCE + warn (Q8) | `season_sort_invalid_falls_back` |
| `SeasonDisplayMode::from_i64(4)` | `None`; unreachable from a 2-bit field | unit |
| `viewerFlags` airing time `0` | `nextEpisodeAiringAt == 0` is the "unknown" sentinel used by the library AiringTime sort (`AnimeLibraryScreenModel.kt:312-322`: compares unseen count first, then keeps entries with airing time 0 after known ones in BOTH directions because the comparator inverts its 0-handling with `isAscending`) | unit on getter |
| Airing time above 2^32-1 | masked to 32 bits (year 2106 overflow) | document only |

### 1.2 Entries

| Case | Behaviour |
|---|---|
| `expectedNextUpdate` for COMPLETED (status 2) | `None`; for status 0 with `nextUpdate = 0` returns epoch 0 (Q14) |
| `Manga::create()` then insert | `id = -1` ignored by the insert; `source = -1` would be stored if the caller forgets to set it (no validation in the domain) |
| `NetworkToLocal` for an existing favourite | returns the stored row, source title discarded |
| `NetworkToLocal` for an existing non-favourite | returns stored row with the source's title in memory only |
| `insertManga` returns `None` | Kotlin NPE through `!!`; Rust returns `Err(DomainError::InsertFailed)` |
| Duplicate `(source, url)` insert race | see 2.1 |
| Library subscription errors | p1-behavior.md 4.1 |
| `getMangaById` of a deleted id | `Err(NotFound)` from the repository; `get_manga_or_none` logs and returns `None` |

### 1.3 Items

| Case | Behaviour |
|---|---|
| Chapter named only with title, no digits | number `-1.0` (unrecognised); `is_recognized_number == false`; ignored by missing-count and gap; sorts first ascending |
| Source number `-2.0` | kept as sentinel `-2.0` (step 1 of recognition); `is_recognized_number == false` |
| Source number `0.0` | kept; recognised |
| Number text with 20+ digits | parses to a large `f64`; `missing_items_count` casts with saturating `as i32` |
| Names with different-case title (`BLEACH 1` vs title `Bleach`) | both lower-cased before removal |
| Title containing regex metacharacters | removal is a literal `replace`, never a regex |
| Empty title | `replace("", "")` is a no-op |
| `dateUpload` = 0 from source | Q1 |
| `copy_from` with blank scanlator (`""`, `"  "`) | stored as `None` (`ifBlank`); NOTE `copyFromSChapter` (area:10-app-domain-layer) additionally `trim()`s non-blank scanlators, the domain `copyFrom` does not |
| Same chapter number from two scanlators | both kept; duplicate-read marking handled by sync (area:10-app-domain-layer) |
| List with all `-1.0` numbers | `missing_items_count == 0` |
| `missing_items_count` input `[0.0]` | items `[0]`, no gap, result 0 |
| `gap(5.0, 7.0)` (higher < lower) | `-3` (negative; callers test `> 0`) |
| `gap(-1.0, x)` or `gap(x, -1.0)` | 0 |
| Recognition input with `\n` | irrelevant to digits; `$`-anchored tag regexes see end of string only (Rust) vs also before a trailing newline (Java) |
| Episode name with only a tag (`[Group]`) | tags stripped to empty, no number, returns `known ?? -1.0` |

### 1.4 Fetch interval

| Case | Behaviour |
|---|---|
| Fewer than 3 distinct upload days and fewer than 3 distinct fetch days | 7 days |
| All items with `dateUpload <= 0` | falls back to fetch dates |
| Items with `dateFetch = 0` | grouped under 1970-01-01 (one distinct day) |
| `fetchInterval < 0` | kept; step = `abs(interval)`; chapter list is not even loaded |
| `lastUpdate` in the future | negative `timeSinceLatest` (Q: p1-topic-fetch-interval.md 5.1 item 3) |
| DST day | window length 47/49 h; result offset error (5.1 item 4) |
| `window == (0, 0)` | recomputed from `ZonedDateTime.now()`, ignoring the `dateTime` argument |
| Zero chapters | list empty -> 7 -> `nextUpdate = lastUpdate-day + 7 d` aligned |

### 1.5 Release check

Network exception: propagates, `lastChecked` untouched; `service.latest == None`: no update and `lastChecked` untouched; three-day throttle is bypassed only by `forceCheck`.

## 2. Race conditions and concurrency hazards

1. **Duplicate insert in `NetworkToLocal`** (p1-behavior.md 4.2): two concurrent browse/search tasks. Mitigation: single writer transaction; additionally area:05-data-repos-entries-items should add a unique index on `(source, url)` for `mangas` and `animes` (open item for area:05-data-repos-entries-items, recorded as a dependency of T-07a-021). Test: spawn 16 tasks calling `network_to_local_manga` with the same url; exactly one row exists and all callers receive the same id.
2. **Read-modify-write of flags from stale snapshots**: two quick toggles of different filter groups both read `manga.chapter_flags = X`; if the second call uses the old snapshot it overwrites the first (lost update). Aniyomi relies on the screen model refreshing from the subscription between taps. Mitigation (Proposal): setters in Rust take the entry id and perform the read-modify-write inside the writer transaction (`UPDATE mangas SET chapter_flags = (chapter_flags & ~:mask) | (:flag & :mask) WHERE _id = :id`), which removes the race entirely; the pure `set_flag` remains for unit tests and for `awaitSetAllFlags` (which writes a whole new word by design). Test: two concurrent tasks setting unread and bookmark filters both persist.
3. **Viewer flags airing update** (Q17): single statement with two masked fields.
4. **`SetDefault*Flags.awaitAll`** iterates the library and writes each entry; a concurrent library update may change `chapter_flags` of an entry between the favourites read and the write (the write replaces the whole word with preference-derived bits, by design). Acceptable; document.
5. **`combine` ordering**: the pair `(manga, chapters)` can be briefly inconsistent after a delete-and-reinsert (chapters of a previous sync with a newer manga row); the details screen must tolerate chapter lists whose `manga_id` differs for one frame (filter by `manga_id` before rendering).
6. **Subscription retry loops** must be cancellable when the screen closes (drop the receiver).

## 3. Upstream tests to port

All tests are JUnit 5 + kotest `shouldBe` (+ mockk for two files). Port as Rust `#[test]` / `#[tokio::test]`; names below are the Rust test names.

### 3.1 `MangaFetchIntervalTest` and `AnimeFetchIntervalTest` (10 tests each, 20 total)

Files: `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt`, `domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt`. They differ only by type names. Port once as a generic test over `ItemDates` plus one thin test per entry kind that checks the manga path loads the scanlator-filtered list (`applyScanlatorFilter = true`) and the anime path the full list. Rust names: `interval_default_7_when_not_enough_distinct_days`, `interval_based_on_more_recent_items`, `interval_based_on_smaller_subset_if_very_few_items`, `interval_7_when_multiple_items_in_1_day`, `interval_7_when_multiple_items_in_2_days`, `interval_1_when_released_every_day`, `interval_1_when_delta_less_than_1_day`, `interval_2_when_released_every_2_days`, `interval_floors_decimal`, `interval_1_when_just_below_every_2_days`. Table with inputs in p1-topic-fetch-interval.md section 6. Setup: `test_time = 2020-01-01T00:00:00Z` (`1577836800000` ms), zone UTC, helper `with_time(duration)` sets both `date_fetch` and `date_upload`. The `@Execution(CONCURRENT)` annotation is irrelevant (Rust tests run in parallel by default; the tests share no state).

### 3.2 `ChapterRecognitionTest` (34 tests, 57 assertions)

File `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt`. Rust names are the Kotlin names snake-cased: `basic_ch_prefix`, `basic_ch_prefix_with_space_after_period`, `basic_ch_prefix_with_decimal`, `basic_ch_prefix_with_alpha_postfix`, `name_containing_one_number`, `name_containing_one_number_and_decimal`, `name_containing_one_number_and_alpha`, `chapter_containing_manga_title_and_number`, `..._number_decimal`, `..._number_alpha`, `extreme_case`, `extreme_case_with_decimal`, `extreme_case_with_alpha`, `chapter_containing_dot_v2`, `number_in_manga_title`, `space_between_ch_x`, `chapter_title_with_ch_substring`, `chapter_containing_multiple_zeros`, `chapter_with_version_before_number`, `version_attached_to_chapter_number`, `number_after_manga_title_with_chapter_in_chapter_title_case`, `number_after_unwanted_tag`, `unparseable_chapter`, `chapter_with_time_in_title`, `chapter_with_alpha_without_dot`, `chapter_title_containing_extra_and_vol`, `..._omake_and_vol`, `..._special_and_vol`, `chapter_title_containing_commas`, `chapter_title_containing_hyphens`, `chapters_containing_season`, `chapters_in_format_sx_chapter_xx`, `chapters_ending_with_s`, `chapters_containing_ordinals`. Every assertion with its expected value is in p1-topic-item-number-recognition.md 5.1. The assertion helper: `parse_chapter_number(title, name, None) == expected` with `==` on `f64` (the expected literals are exactly representable as the sums the algorithm produces; if a platform differs, use `assert_eq!(a.to_bits(), b.to_bits())`-free comparison with `(a-b).abs() < 1e-12`).

### 3.3 `MissingItemsTest` (8 tests)

File `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt`. Tests and data:

| Rust name | Input | Expected |
|---|---|---|
| `missing_count_returns_0_when_empty_list` | `[]` | 0 |
| `missing_count_returns_0_when_all_unknown_numbers` | `[-1.0, -1.0, -1.0]` | 0 |
| `missing_count_handles_repeated_base_numbers` | `[1.0, 1.0, 1.1, 1.5, 1.6, 1.99]` | 0 |
| `missing_count_returns_number_of_missing_items` | `[-1.0, 1.0, 2.0, 2.2, 4.0, 6.0, 10.0, 10.0]` | 5 |
| `chapter_gap_returns_difference` | `gap(chapter 10.0, 9.0) == 0`, `(10.0, 8.0) == 1`, `(10.0, 8.5) == 1`, `(10.0, 1.1) == 8`; same four with plain `f64` arguments | as listed |
| `chapter_gap_returns_0_if_either_not_valid` | `(-1.0, 10.0)`, `(99.0, -1.0)` for chapters and plain numbers | 0 |
| `episode_gap_returns_difference` | same numbers with `Episode::create()` items | as chapter |
| `episode_gap_returns_0_if_either_not_valid` | same | 0 |

Note: the Kotlin assertions write `shouldBe 0f` / `1f` / `8f` against an `Int`; kotest treats numerically equal values as equal. In Rust compare `i32` with integer literals.

### 3.4 `GetApplicationReleaseTest` (4 tests)

File `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt`; data in p1-topic-release-check.md section 4. Rust names: `new_update_when_preview_and_tag_number_exceeds_commit_count`, `new_update_when_release_newer`, `no_update_when_release_older`, `no_update_within_three_days_and_service_not_called`.

### 3.5 `LibraryFlagsTest` (5 tests, area:08-domain-library-category-history-updates owns the code under test)

File `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt`. Production types: `domain/src/main/java/tachiyomi/domain/library/model/Flag.kt`, `LibraryDisplayMode.kt`, `domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt`, `domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt`. Numbers (binary literals as in the source):

Manga sort types: `Alphabetical 0b00000000`, `LastRead 0b00000100`, `LastUpdate 0b00001000`, `UnreadCount 0b00001100`, `TotalChapters 0b00010000`, `LatestChapter 0b00010100`, `ChapterFetchDate 0b00011000`, `DateAdded 0b00011100`, `TrackerMean 0b000100000` (= 0b00100000), `Random 0b00111100`; type mask `0b00111100`. Anime adds `AiringTime 0b00110000` and renames: `LastSeen 0b100`, `UnseenCount 0b1100`, `TotalEpisodes 0b10000`, `LatestEpisode 0b10100`, `EpisodeFetchDate 0b11000`. Direction mask `0b01000000`; `Ascending 0b01000000`, `Descending 0b00000000`. `MangaLibrarySort.default = (Alphabetical, Ascending)`. Operators (`Flag.kt:21-35`): `Long + Flag`: if the flag has a mask, `this & !mask | (flag & mask)`, else `this | flag`; `Flag + Flag` same with `this.flag` as the left value; `FlagWithMask` sorts have `flag = type + direction` and `mask = type.mask | direction.mask = 0b01111100`.

| Rust name | Assertion |
|---|---|
| `check_the_amount_of_flags` | `LibraryDisplayMode::values().len() == 4`; manga `types.len() == 10`, `directions.len() == 2`; anime `types.len() == 11`, `directions.len() == 2` |
| `flag_plus_operator_library_sort` | `(LastRead, Ascending) + (DateAdded, Ascending)` equals `0b01011100` for manga (types LastRead/DateAdded) and anime (LastSeen/DateAdded) |
| `flag_plus_operator` | `(DateAdded, Ascending).flag == 0b01011100` for both |
| `flag_plus_operator_with_old_flag_as_base` | `(UnreadCount, Descending).flag == 0b00001100` (anime `UnseenCount`); `old_flag + (DateAdded, Ascending) == 0b01011100` and `!= old_flag`; the anime half of the Kotlin test computes `animesort.flag + animesort` (uses the NEW sort as both operands, an upstream copy-paste slip) and still expects `0b01011100`; port both lines as written |
| `default_flags` | `default.type + default.direction == 0b01000000` for both (Alphabetical 0 + Ascending 0b01000000) |

These tests depend on area:08-domain-library-category-history-updates's `LibrarySort` types (T-08a tasks); task T-07a-035 owns the Rust port of this test file and lists area:08-domain-library-category-history-updates as dependency.

## 4. New tests to write

Flags (property + vectors):
1. For every group of every flag word and every legal value: `set_flag` then accessor returns the value; all other bits unchanged (use `proptest` over random starting words; `proptest` is a dev-dependency only).
2. Vectors in p1-topic-flags-bit-layouts.md 2.
3. `awaitSetAllFlags` equivalents: default preferences give `0` for chapter and episode words and `0x5800000` for the season word.
4. TriState cycle and `apply_filter` laziness (predicate not called for Disabled; counter test).

Entries:
5. `to_update` round trip: `apply_update(entry, entry.to_update()) == entry`.
6. `expected_next_update` for status 2 vs 1.
7. `network_to_local` four cases (absent, non-favourite title overlay, favourite unchanged, concurrent duplicates).
8. Facade error contracts: repository returns error -> `get_chapter_or_none` returns `None`, `update_chapter_logged` returns without error.

Items:
9. `should_update_db_*`: each field flips the result (9 episode cases, 5 chapter, 3 season incl. memo ignored), Q1 deviation test, memo equality ignoring key order.
10. `copy_from` blank handling for scanlator, summary, preview_url.
11. Sorting: SOURCE inversion; NUMBER/UPLOAD/ALPHABET both directions; stability on ties; `-1.0` ordering; collator equality `"a" ~ "A" ~ "á"`; `"Chapter 10" < "Chapter 2"` (non-natural); 0xE000 fallback; season reversal with alphabetical tie-break (not reversed); `playback order` call with `descending = false`.
12. `missing_items_count` extra: `[0.0] -> 0`, `[5.0] -> 4`, `[-2.0, 3.0] -> 2`, `[-0.5, 2.0] -> 1`; `gap(5.0, 7.0) == -3`.
13. Recognition: additional vectors (p1-topic-item-number-recognition.md 5.2-5.4, ~56 cases), the `known` shortcut matrix, alpha table `a..j`, keyword priority (`extra` is tested before `omake` before `special`: title `Zed`, input `Ch.4.specialextra` gives 4.99 and `Ch.4.omakespecial` gives 4.98; a one-letter title such as `X` would first delete the `x` inside `extra`).
14. Fetch interval: vectors in p1-topic-fetch-interval.md 5.2 and section 7; `calculate_interval` laziness check (list of 100 000 items finishes under 200 ms in release).
15. Release check: tests listed in p1-topic-release-check.md 4.

Integration:
16. Details-screen model (area:10-app-domain-layer) smoke: sort/filter pipeline on a 2 000-chapter fixture under 50 ms.
17. Backup round trip: flag words and `fetch_interval` survive export/import byte-for-byte (the backup series (no plans folder yet) contract test using this area's accessors).

## 5. Fixtures needed

- `kuro-testkit::domain::{manga(), anime(), chapter(n), episode(n), season(title, number)}` builders returning `create()` defaults with chosen fields (T-07a-034).
- `ItemDates` generators: `every_n_days(n, count)`, `every_n_hours(h, count)`, matching the upstream helper `withTime`.
- A fixed `Clock` (`FixedClock { now_ms, zone }`), zones: UTC and `Europe/Berlin` (chrono-tz as dev-dependency only).
- Recognition vector files as Rust constant arrays (title, name, known, expected) so the Kotlin and Rust suites can be diffed; store in `kuro-core/tests/data/recognition_vectors.rs`.
- Seeded database with one parent anime (`fetch_type = Seasons`) and three children for season ordering tests (area:05-data-repos-entries-items harness).
