# Area 07 — implementation tasks (domain: entries and items)

Module root: `kuro-core/src/domain/` (layout in p1-rust-design.md section 1). Struct definitions live in `kuro-core/src/data/models.rs` (area:05-data-repos-entries-items). Source paths are repo-relative to `/home/blackbox/aniyomi-src`. Behaviour and vectors: p1-behavior.md, p1-topic-*.md, p1-edge-cases-and-tests.md.

### T-07a-001 — Flag primitives: set_flag, TriState, apply_filter
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: domain/src/main/java/tachiyomi/domain/entries/TriState.kt, core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt, domain/src/main/java/tachiyomi/domain/library/model/Flag.kt
- Acceptance:
  - `set_flag(v, flag, mask)` returns `(v & !mask) | (flag & mask)` and ignores flag bits outside the mask (unit test with 8 vectors including negative `i64`)
  - `TriState::next` cycles Disabled -> EnabledIs -> EnabledNot -> Disabled (3 asserts)
  - `apply_filter` does not call the predicate for Disabled (counter test), calls it once for EnabledIs, negates it for EnabledNot
  - `tri(raw, is, not)` returns Disabled for 0 and for the invalid group value `is | not`
  - Type is re-exported so area:02-preferences-system can use the same `TriState` (no duplicate definition)

### T-07a-002 — Manga chapter_flags layout, constants and accessors
- Priority: P0
- Effort: S
- Depends: T-07a-001, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt
- Acceptance:
  - Module `chapter_flags` holds every constant of p1-data-and-config.md 3.1 with the exact hex values (`SORT_DIR_MASK 0x1`, `UNREAD_MASK 0x6`, `DOWNLOADED_MASK 0x18`, `BOOKMARKED_MASK 0x60`, `SORTING_MASK 0x300`, `DISPLAY_MASK 0x100000`)
  - Accessors `sorting`, `display_mode`, `unread_filter(_raw)`, `downloaded_filter_raw`, `bookmarked_filter(_raw)`, `sort_descending` match the Kotlin on a table of 30 words including `0x6`, `0x18`, `0x60`
  - `expected_next_update` is `None` only when status == 2
  - Round-trip property test: for every group and legal value, set then read returns the value and other bits are unchanged

### T-07a-003 — Anime episode_flags layout, constants and accessors
- Priority: P0
- Effort: S
- Depends: T-07a-001, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt
- Acceptance:
  - Module `episode_flags` holds every constant of p1-data-and-config.md 3.2 (sorting group at `0x600`, fillermarked `0x180`, previews `0x800`, summaries `0x1000`, display `0x100000`)
  - `unseen_filter`, `bookmarked_filter`, `fillermarked_filter`, `sort_descending`, `show_previews`, `show_summaries` return the Kotlin results for a table of 30 words
  - Vectors of p1-topic-flags-bit-layouts.md section 2 (episode rows) reproduce exactly (`0x201`, `0x200`, `0x202`, `0x242`, `0x342`, `0x101B42`)
  - The manga module and the anime module share no constants (compile-time separation)

### T-07a-004 — Anime season_flags layout, SeasonDisplayMode and accessors
- Priority: P1
- Effort: M
- Depends: T-07a-001, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt
- Acceptance:
  - Module `season_flags` holds all constants of p1-data-and-config.md 3.3 (sort types 0x0, 0x2000, 0x4000, 0x6000, 0x8000, 0xA000, 0xC000; grid mode shift 16 mask 0x30000; size shift 18 mask 0x3C0000; overlays 0x400000, 0x800000, 0x1000000, 0x2000000, 0x4000000; display mode 0x8000000)
  - `SeasonDisplayMode` maps 0 CompactGrid, 1 ComfortableGrid, 2 CoverOnlyGrid, 3 List, `from_i64(4) == None`
  - Filter accessors for downloaded, unseen, started, completed, bookmarked, fillermarked return IS/NOT/DISABLED per the table; grid mode and size are read with shifts
  - Vectors `0x5970000` and `0x2001` of p1-topic-flags-bit-layouts.md reproduce; default-preference word is `0x5800000`
  - `seasons_filtered(anime, downloaded_only)` is true when any of the six season filters is not Disabled (downloaded filter forced to EnabledIs when downloaded-only mode is on)

### T-07a-005 — Anime viewer_flags: skip-intro and airing accessors and setters
- Priority: P1
- Effort: S
- Depends: T-07a-001, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt, app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt
- Acceptance:
  - Getters `skip_intro_length`, `skip_intro_disable`, `next_episode_to_air`, `next_episode_airing_at` decode the layout (intro bits 0-7, episode bits 8-23, airing time bits 24-55, disable bit 56)
  - Vector: intro 90, episode 13, time 1760000000 encodes to `0x68E77800000D5A` and decodes back
  - `set_skip_intro_length(0)` sets the disable bit and clears the length (`0x168E77800000D00` from the vector above); any non-zero length clears the disable bit
  - `set_next_episode_airing(episode, time)` writes both groups in ONE update (fixes Q17)
  - Length above 255 is rejected or clamped to 255 by the setter (the mask would silently truncate)

### T-07a-006 — Entry defaults, updates and to_update
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt
- Acceptance:
  - `Manga::create()` and `Anime::create()` return exactly the defaults of p1-data-and-config.md 1.1/1.2 (id -1, source -1, status 0, `season_number -1.0`, `fetch_type Episodes`, memo `{}`)
  - `to_update()` sets all 20 (manga) / 28 (anime) optional fields; `apply_update(entry, entry.to_update()) == entry` for a random entry
  - `apply_update` leaves a field untouched when the update field is `None` and never clears `Some` text with `None`
  - Both update structs implement `Default` (all `None` except id)

### T-07a-007 — Cover descriptors (MangaCover, AnimeCover)
- Priority: P0
- Effort: S
- Depends: T-07a-006
- Sources: domain/src/main/java/tachiyomi/domain/entries/EntryCover.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaCover.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeCover.kt
- Acceptance:
  - `as_cover()` maps `(id, source, favorite, thumbnail_url, cover_last_modified)` for both entry kinds
  - The descriptors implement `Hash + Eq` and are used as the key of the cover loader (changing `cover_last_modified` or `favorite` yields a different key)
  - No descriptor exists for the anime background image (documented in code comment)

### T-07a-008 — Chapter and Episode models: create, copy_from, update structs
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt, domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt, domain/src/main/java/tachiyomi/domain/items/episode/model/EpisodeUpdate.kt
- Acceptance:
  - `Chapter::create()` and `Episode::create()` return the defaults of p1-data-and-config.md 2 (dateUpload -1, number -1.0, version 1)
  - `is_recognized_number` is true for 0.0 and false for -1.0, -2.0, -0.0001 is false and 0.0 true
  - `copy_from` copies only name, url, date_upload, number (+ fillermark for episodes) and turns blank (`""`, `"  "`) scanlator, summary and preview_url into `None`; every other field is preserved
  - `to_update()` fills all 12 (chapter) / 17 (episode) optional fields

### T-07a-009 — Recognition core: regex rewrite and value computation
- Priority: P0
- Effort: M
- Depends: area:01-foundation-app-bootstrap
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterRecognition.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeRecognition.kt, domain/src/main/java/tachiyomi/domain/items/season/service/SeasonRecognition.kt
- Acceptance:
  - One generic `parse(recogniser, title, name, known)` implements the 7-step algorithm of p1-topic-item-number-recognition.md section 2 with consuming rewrites of every look-behind and look-ahead (no `fancy-regex`)
  - Regexes compile once (`LazyLock`); `value_of` implements decimal, `extra` 0.99, `omake` 0.98, `special` 0.97, single-letter a..i = 0.1..0.9, j and above 0.0
  - Word boundaries behave as ASCII (`(?-u:\b)`) so non-Latin titles do not move boundaries
  - Parsing 10 000 names takes under 100 ms in release mode

### T-07a-010 — ChapterRecognition and the 34 upstream tests
- Priority: P0
- Effort: S
- Depends: T-07a-009
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterRecognition.kt, domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt
- Acceptance:
  - All 34 upstream tests (57 assertions, table in p1-topic-item-number-recognition.md 5.1) pass in Rust with identical expected values
  - The additional vectors of 5.2 pass, including the `known` shortcut matrix (-2.0, -1.5, -0.5, 0.0, 5.0)
  - `parse_chapter_number("X", "Foo", Some(-2.0)) == -2.0` and `("X", "Foo", None) == -1.0`

### T-07a-011 — EpisodeRecognition (tag stripping) with new tests
- Priority: P0
- Effort: S
- Depends: T-07a-009
- Sources: domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeRecognition.kt
- Acceptance:
  - Tag loop strips leading/trailing `[...]` and `(...)` repeatedly until none remain
  - The 20 vectors of p1-topic-item-number-recognition.md 5.3 pass, including `[Flugel] Kaguya-sama ... [80AC7B2E]` giving 1.0 and `Show Episode 5 special` giving `5.0 + 0.97` (compared as the same f64 sum)
  - Quirk Q7 (`1080p` alone gives 1080.0) is asserted as documented behaviour

### T-07a-012 — SeasonRecognition with new tests
- Priority: P1
- Effort: S
- Depends: T-07a-009
- Sources: domain/src/main/java/tachiyomi/domain/items/season/service/SeasonRecognition.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt
- Acceptance:
  - The 18 vectors of p1-topic-item-number-recognition.md 5.4 pass including quirks Q5 (capital `Hi10` never matches) and Q6 (`Boku.no.Hero.Academia.S02.1080p-ITH` gives `2.0 + 0.108`)
  - `known` shortcut identical to chapters
  - `parse_season_number("Boku no Hero Academia", "Boku no Hero Academia (2017)", None) == -1.0`

### T-07a-013 — Chapter and episode comparators with collator
- Priority: P0
- Effort: M
- Depends: T-07a-002, T-07a-003, T-07a-008
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt
- Acceptance:
  - `chapter_cmp(sorting, descending)` and `episode_cmp` implement the table of p1-topic-sorting-and-filtering.md 1 including the inverted SOURCE row (descending = ascending `source_order`)
  - Numbers compare with `f64::total_cmp`; sort is stable (ties keep input order, tested)
  - Collator: `icu_collator` Primary strength built once from `LC_COLLATE`/`LANG`; `"a"`, `"A"` and `"á"` compare equal; `"Chapter 10"` sorts before `"Chapter 2"`
  - Calling with `descending = false` yields reader/player playback order (oldest first) for all four sortings
  - Unknown sorting value falls back to SOURCE and logs a warning, no panic

### T-07a-014 — Season comparator and sort_seasons
- Priority: P1
- Effort: S
- Depends: T-07a-004, T-07a-013, T-07a-029
- Sources: domain/src/main/java/tachiyomi/domain/items/season/service/SeasonSorter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Ascending comparator per sort code 0x0, 0x2000, 0x4000, 0x6000, 0x8000, 0xA000, 0xC000 using `season_source_order`, `season_number`, `latest_upload`, alphabetical title (lower-cased, collator), `unseen_count`, `last_seen`, `fetched_at`
  - `sort_seasons` reverses when `season_sort_descending()` and then applies the alphabetical tie-break WITHOUT reversing it
  - Default word 0 yields highest `season_source_order` first (not inverted, unlike chapters)
  - Code 0xE000 falls back to SOURCE with a warning (Q8)

### T-07a-015 — Missing-items count, gap and list separators
- Priority: P1
- Effort: S
- Depends: T-07a-008
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/MissingEpisodes.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - `missing_items_count` follows the algorithm of p1-behavior.md section 7 (only exactly -1.0 ignored, truncate toward zero, distinct, sort, count from 0)
  - All 8 upstream tests of MissingItemsTest pass; extra cases `[0.0] -> 0`, `[5.0] -> 4`, `[-2.0, 3.0] -> 2`, `gap(5.0, 7.0) == -3`
  - `chapter_gap`/`episode_gap` return 0 for `None` or unrecognised numbers
  - `insert_missing_separators(list)` produces a separator between neighbours when gap > 0 and one at the end of the list with `floor(number) - 1` clamped at 0, id `"{lower_id}-{higher_id}"`; strings "Missing %1$s chapter(s)" / "Missing %1$s item(s)" and "Skipping %d chapter(s), either the source is missing it..." come from i18n keys

### T-07a-016 — should_update_db_chapter / episode / season
- Priority: P0
- Effort: S
- Depends: T-07a-008
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/interactor/ShouldUpdateDbChapter.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/ShouldUpdateDbEpisode.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/ShouldUpdateDbSeason.kt
- Acceptance:
  - Chapter compares scanlator, name, date_upload, chapter_number, source_order; episode adds summary, fillermark, preview_url, memo (key-order-insensitive JSON equality); season compares title, season_number, season_source_order only
  - One unit test per field flips the result
  - Quirk Q1 fix: when the source `date_upload` is 0 the date comparison is skipped (db 1700000000000 vs source 0 with equal other fields gives false); documented as the single deviation
  - Season memo difference alone returns false (Q15 preserved)

### T-07a-017 — Fetch interval engine
- Priority: P1
- Effort: M
- Depends: T-07a-006, T-07a-008, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt
- Acceptance:
  - `get_window`, `calculate_interval`, `increase_interval`, `calculate_next_update`, `to_update` implement p1-topic-fetch-interval.md sections 2-5 for any `chrono::TimeZone`; one implementation serves chapters and episodes
  - Vectors of 5.2 reproduce bit-exactly (`1710374400000/1710547199999`, `1710633600000`, `1710547200000`, `1711065600000`, `1701993600000`, DST row `1712271600000`)
  - `calculate_interval` stays lazy (iterator adaptors; 100 000 items under 200 ms)
  - Manga path loads the scanlator-filtered chapter list, anime path all episodes
  - Negative stored interval is kept and used as `abs(interval)`; `(0, 0)` window recomputes from now
  - Quirks 1, 3, 4 preserved and covered by tests

### T-07a-018 — Fetch interval user override and next-update text helpers
- Priority: P1
- Effort: S
- Depends: T-07a-017
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt
- Acceptance:
  - `set_fetch_interval(entry, wheel)` writes `-wheel` through `to_update`; wheel 0 ("Default") restores automatic mode (stored 0, recomputed)
  - Wheel offers 0..=28 (`MAX_INTERVAL`), with item 0 labelled "Default"; always shown (Q20 changed: no release-build gate)
  - `next_update_days(expected_next_update, now)` = whole days from now, clamped at 0; text uses keys `manga_interval_expected_update` / `anime_interval_expected_update` ("New chapters predicted to be released in around %1$s, checking around every %2$s.") when `interval >= 0` and a value exists, else the `..._null` text ("This manga is either completed, or there is no predicted release date.")
  - Plural "1 day"/"%d days" resolved through the i18n plural helper

### T-07a-019 — Fetch interval tests (20 upstream + new)
- Priority: P1
- Effort: S
- Depends: T-07a-017
- Sources: domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt, domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt
- Acceptance:
  - The 10 upstream cases (p1-topic-fetch-interval.md section 6) pass once generically and once per entry kind (20 test executions)
  - New tests from section 7 (window, next-update rows, `increase_interval` table, empty and single-item lists) pass
  - Tests use an injected clock and UTC plus `Europe/Berlin` zones, never the wall clock

### T-07a-020 — Read-side query facades with Kotlin error contracts
- Priority: P0
- Effort: M
- Depends: area:05-data-repos-entries-items, T-07a-006, T-07a-008
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetManga.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnime.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetMangaFavorites.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnimeFavorites.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetMangaByUrlAndSourceId.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnimeByUrlAndSourceId.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetDuplicateLibraryManga.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetDuplicateLibraryAnime.kt
- Acceptance:
  - `get_manga_or_none` / `get_anime_or_none` log and return `None` on any repository error, including a missing row
  - By-url, favourites, favourites-by-source and duplicate lookups pass through; duplicate lookup passes `title.to_lowercase()` and the entry id
  - All 8 facades are covered by an in-memory DB test, plus one error-injection test per swallowed facade
  - A coverage test enumerates the 16 manga and 21 anime repository methods of p1-behavior.md section 4 and asserts each is reachable from `MangaRepo`/`AnimeRepo`

### T-07a-021 — network_to_local (manga and anime) in one transaction
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items, T-07a-006
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/NetworkToLocalManga.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt
- Acceptance:
  - Absent (url, source): inserts and returns the argument with the new id; existing non-favourite: returns the stored row with the source title in memory only (DB title unchanged); existing favourite: returns the stored row unchanged
  - 16 concurrent calls with the same (url, source) leave exactly one row and return the same id (select+insert in one writer transaction)
  - Insert returning no id yields an error (Kotlin NPE through `!!`)
  - The unused `sourceManager` dependency of the anime variant is not carried over

### T-07a-022 — Combined subscriptions and library subscription retry
- Priority: P0
- Effort: M
- Depends: area:05-data-repos-entries-items, T-07a-020
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetMangaWithChapters.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnimeWithEpisodesAndSeasons.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetLibraryManga.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetLibraryAnime.kt
- Acceptance:
  - `subscribe_manga_with_chapters(id, scanlator_filter)` emits only after both sources emitted once, then on every change of either (test: toggling a chapter's read flag emits a new pair)
  - `subscribe_anime_with_episodes_and_seasons` does the same for three sources
  - `await` variants (`awaitManga`, `awaitChapters(id, scanlatorFilter)`, `awaitAnime`, `awaitEpisodes`, `awaitSeasons`) pass errors through
  - Library subscription retries a transient error after 500 ms up to 20 times then surfaces an error value; other errors are logged and the previous list is kept
  - Dropping the receiver cancels the task (no leak, tested with a counter)

### T-07a-023 — Chapter, episode and season item facades (get, update)
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items, T-07a-008
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/interactor/GetChapter.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/GetChapterByUrlAndMangaId.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/GetChaptersByMangaId.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/UpdateChapter.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/GetEpisode.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/GetEpisodeByUrlAndAnimeId.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/GetEpisodesByAnimeId.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/UpdateEpisode.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/GetAnimeSeasonsByParentId.kt
- Acceptance:
  - `get_chapter`/`get_episode` (by id and by url+entry id) log and return `None` on error; by-url variants named `get_chapter_by_url_and_manga_id` log nothing (Kotlin swallows silently) and the parameter is called `manga_id` (Q19)
  - `get_chapters_by_manga_id(id, scanlator_filter)`, `get_episodes_by_anime_id`, `get_seasons_by_parent_id` return an empty vector on error with a logged message
  - `update_chapter` / `update_all_chapters` / episode twins log and swallow errors and return `()`
  - Each facade has a happy-path and an error-injection test

### T-07a-024 — Chapter flag setters (SetMangaChapterFlags)
- Priority: P0
- Effort: S
- Depends: T-07a-002, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/SetMangaChapterFlags.kt
- Acceptance:
  - Pure functions `chapter_flags_after_*` plus write wrappers exist for downloaded, unread, bookmark, display, sort-or-flip and set-all (6 groups applied from 0 in the order unread, downloaded, bookmarked, sorting, direction, display)
  - Sort-or-flip state machine matches p1-behavior.md 3.1.1 (same column flips; new column forces ASC), vector 0 -> `0x100|0x1` -> flip -> `0x100`
  - Writes are single-statement masked updates inside the writer transaction so two concurrent setters on different groups both persist
  - Returns the repository's `bool`; unit test for each method

### T-07a-025 — Episode flag setters (SetAnimeEpisodeFlags)
- Priority: P0
- Effort: S
- Depends: T-07a-003, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeEpisodeFlags.kt
- Acceptance:
  - Setters for downloaded, unseen, bookmark, fillermark, display, sort-or-flip (mask `0x600`), previews (`0x800`), summaries (`0x1000`) and set-all (9 groups in the documented order)
  - Vectors `0x201`, `0x200`, `0x202`, `0x242`, `0x342`, `0x101B42` reproduce through the setters
  - Concurrency test as T-07a-024

### T-07a-026 — Season flag setters (SetAnimeSeasonFlags)
- Priority: P1
- Effort: S
- Depends: T-07a-004, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeSeasonFlags.kt
- Acceptance:
  - Setters for the 6 filters, sort-or-flip (mask `0xE000`, dir `0x1`), grid mode (shift 16), grid size (shift 18, clamped 0..=15), five overlay booleans, display mode, and set-all (16 parameters applied from 0 in the documented order)
  - Overlay setters set or clear exactly their bit (`0x400000`, `0x800000`, `0x1000000`, `0x2000000`, `0x4000000`)
  - Vector: grid List + size 5 + three overlays gives `0x5970000`

### T-07a-027 — Default flags application and "set as default" persistence
- Priority: P1
- Effort: M
- Depends: T-07a-024, T-07a-025, T-07a-026, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/SetAnimeDefaultEpisodeFlags.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - `apply_default_*_flags(entry)` reads the `default_*` keys of p1-data-and-config.md 4 (names and defaults exact) and writes the resulting word; with all defaults the chapter and episode words are 0 and the season word is `0x5800000`
  - `*_to_library` applies to every favourite in a detached task that survives dialog close
  - `set_*_settings_default(entry)` persists the entry's raw groups into the same keys; Q2 fixed (season downloaded default is written from the downloaded raw bits); Q3 preserved (episode and chapter share `default_chapter_display_by_name_or_number` and `default_chapter_sort_by_ascending_or_descending`)
  - Call sites wired: details screen load for non-favourites (episode + season defaults for anime), browse add-to-library (episode defaults only), "Set as default" with the apply-to-all switch, "Reset to default" (p1-behavior.md 3.4)
  - Snackbar texts "Updated default chapter/episode/season settings" come from i18n keys

### T-07a-028 — Reset viewer flags (manga and anime)
- Priority: P2
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/ResetMangaViewerFlags.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/ResetAnimeViewerFlags.kt
- Acceptance:
  - `reset_viewer_flags_manga` and `_anime` run `UPDATE ... SET viewer = 0` through the repositories and return the repository bool
  - After the call every entry has `viewer_flags == 0` (test with 3 entries)
  - Anime skip-intro, disable bit and airing data are cleared too (documented in the settings help text)

### T-07a-029 — SeasonAnime stats model and to_library_anime
- Priority: P1
- Effort: S
- Depends: T-07a-006, area:05-data-repos-entries-items
- Sources: domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt, domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt
- Acceptance:
  - `seen()` is `total == seen_count` (a child with zero episodes is seen), `unseen_count`, `has_started` (seen_count > 0), `has_bookmarks`, `has_fillermarks` as in p1-data-and-config.md 1.3
  - `to_library_anime()` uses category -1 and maps `fetched_at` to `episode_fetched_at`
  - Table-driven test with 6 rows

### T-07a-030 — Season parent/child helpers
- Priority: P1
- Effort: S
- Depends: T-07a-029, T-07a-021, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/repository/AnimeRepository.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/GetAnimeSeasonsByParentId.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/NoSeasonsException.kt
- Acceptance:
  - Wrappers for `getAnimeSeasonsById`, `getAnimeSeasonsByIdAsFlow`, `removeParentIdByIds`, `getDeletableParentAnime`, `getChildrenByParentId` exist on the facade with documented error behaviour (only `get_seasons_by_parent_id` swallows)
  - Detaching seasons leaves rows in place with `parent_id` NULL (test)
  - `NoSeasons` is raised by the sync code (area:10-app-domain-layer) when the source returns an empty list for a non-local source

### T-07a-031 — Related anime groups
- Priority: P2
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeRelationGroup.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetRelatedAnime.kt, domain/src/main/java/tachiyomi/domain/entries/anime/repository/AnimeRelationRepository.kt
- Acceptance:
  - `AnimeRelationGroup { name, anime }` and `subscribe_related_anime(anime_id)` return groups in repository order
  - `get_last_fetched_at` and `replace_relations(anime_id, groups, fetched_at)` are reachable through the facade (signature parity with the Kotlin repository)
  - Test: replace then subscribe yields the new groups

### T-07a-032 — Domain error types and user-visible messages
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/model/NoChaptersException.kt, domain/src/main/java/tachiyomi/domain/items/episode/model/NoEpisodesException.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/NoSeasonsException.kt, app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - `NoChapters`, `NoEpisodes`, `NoSeasons` are error types; details-screen snackbars show "No chapters found", "No episodes found", "No seasons found"
  - Library-update logs and generic error dialogs show "No results found" for `NoChapters` and `NoEpisodes`; `NoSeasons` uses its own text
  - Strings are referenced by i18n key, not literal

### T-07a-033 — Repository coverage matrix
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/repository/MangaRepository.kt, domain/src/main/java/tachiyomi/domain/entries/anime/repository/AnimeRepository.kt, domain/src/main/java/tachiyomi/domain/items/chapter/repository/ChapterRepository.kt, domain/src/main/java/tachiyomi/domain/items/episode/repository/EpisodeRepository.kt
- Acceptance:
  - A table in code comments (and a compile-checked test calling each) maps every Kotlin repository method (16 + 21 + 11 + 9 = 57) to the Rust repo method of area:05-data-repos-entries-items
  - Missing mappings fail the test with the Kotlin method name
  - Signature differences (suspend-returning-Flow vs `Subscription`) are listed

### T-07a-034 — Domain test fixtures and builders
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - `kuro-testkit::domain` exposes `manga()`, `anime()`, `chapter(n)`, `episode(n)`, `season(title, number)`, `ItemDates` generators `every_n_days` / `every_n_hours`, a `FixedClock`
  - Recognition vectors are stored as constant arrays in `kuro-core/tests/data/recognition_vectors.rs`
  - Builders default to the `create()` values and are used by at least 3 test files

### T-07a-035 — Port LibraryFlagsTest
- Priority: P1
- Effort: S
- Depends: area:08-domain-library-category-history-updates, T-07a-001
- Sources: domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt, domain/src/main/java/tachiyomi/domain/library/model/Flag.kt
- Acceptance:
  - The 5 tests of p1-edge-cases-and-tests.md 3.5 pass against area:08-domain-library-category-history-updates's sort types (counts 4, 10, 2, 11, 2; `0b01011100`; `0b00001100`; default `0b01000000`)
  - Including the upstream slip `animesort.flag + animesort` kept as written
  - `Flag + Flag` and `i64 + Flag` operators match `Flag.kt` semantics (mask-aware vs plain OR)

### T-07a-036 — Release check logic with fixed version compare
- Priority: P2
- Effort: S
- Depends: area:09-domain-source-track-download-release-misc, area:06-data-repos-history-track-source-misc, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt, domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt, data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - The 4 upstream tests pass (preview 2000 vs 1000, v2.0.0 vs v1.0.0, v1.0.0 vs v2.0.0, three-day throttle with zero service calls)
  - Compare is tuple-wise with missing components as 0; `1.0.5` vs `1.1.0` is false; `1.2` vs `1.2.0` is false; `nightly` does not panic (Q13)
  - Throttle 3 days, `forceCheck` bypass, `last_app_check` updated only after a successful lookup; the setting is opt-in (default off)
  - Release endpoint `GET https://api.github.com/repos/{repo}/releases/latest` with a User-Agent header; asset ABI selection and mention linking are not implemented

### T-07a-037 — Drop Android release installer, notifier and ABI asset selection
- Priority: P3
- Effort: S
- Depends: T-07a-036
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt
- Acceptance:
  - Decision recorded: no in-place install (package manager owns files); the user loses one-tap self-update and the Android update notification, replaced by "New version available" banner with an "Open release page" button
  - `BUILD_TYPES` (arm64-v8a, armeabi-v7a, x86_64, x86), `Build.SUPPORTED_ABIS` and the `@user` Markdown linking regex are not ported
  - No code path downloads a binary

### T-07a-038 — Drop Compose and DI annotations: Immutable, Serializable, Injekt factories
- Priority: P3
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/domain/DomainModule.kt
- Acceptance:
  - `@Immutable` (Compose stability hint) and `java.io.Serializable` (Android Parcel/bundle passing) have no Rust counterpart; entries are plain `Clone` structs shared as `Arc`
  - Every `addFactory { X(get()) }` in `DomainModule.kt` for scoped classes has a documented replacement (free function taking `&AppContext` or a repo)
  - The user loses nothing (the annotations only affect Android recomposition and process-death restore)

### T-07a-039 — Replace logcat and withNonCancellableContext
- Priority: P3
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetManga.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt
- Acceptance:
  - Every `logcat(LogPriority.ERROR, e)` in scope becomes `tracing::error!(target = "domain", ...)` with the error chain
  - `withNonCancellableContext` blocks become detached `tokio::spawn` tasks; closing the dialog does not abort an in-progress `apply to all` run (test with an aborted caller)
  - No `unwrap()` on repository results in domain code

### T-07a-040 — Bind the mockup entry screens to the domain flags
- Priority: P0
- Effort: M
- Depends: T-07a-013, T-07a-024, T-07a-025, T-07a-026, T-07a-015, T-07a-022, area:10-app-domain-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt
- Acceptance:
  - In `src/entry.rs` the Chapter/Episode/Season settings sheets (spec screens `manga_entry_details`, `anime_entry_details`) call the setters per the table in p1-rust-design.md section 6; tapping "Bookmarked" cycles Disabled -> Is -> Not -> Disabled and the list updates from the subscription
  - The filter icon is tinted when any filter is not Disabled (`chapters_filtered`, `episodes_filtered`)
  - The list is `filter -> sort` with filters in the Kotlin order (unread/unseen, bookmarked, fillermarked, downloaded) and shows missing-count separators
  - The season sheet's `list Grid display mode` entry is relabelled "Display" and drives `SEASON_DISPLAY_MODE`
  - No change to `spec/*.json`

### T-07a-041 — Quirk regression tests and deviation list
- Priority: P2
- Effort: S
- Depends: T-07a-016, T-07a-017, T-07a-012, T-07a-014, T-07a-027
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/interactor/ShouldUpdateDbChapter.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/items/season/service/SeasonSorter.kt
- Acceptance:
  - One named test per row Q1-Q22 of p1-topic-quirks-and-decisions.md asserting the chosen policy (preserved or fixed)
  - A `DEVIATIONS.md` section in the repo root generated from the table lists only Q1, Q2, Q8, Q13, Q16, Q17, Q20, Q21
  - CI fails if a preserved quirk changes without updating the table

### T-07a-042 — Drop dead code: OsTooOld result and unused NetworkToLocalAnime source manager
- Priority: P3
- Effort: S
- Depends: T-07a-021, T-07a-036
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt, domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - `Result::OsTooOld` is not defined (never produced upstream: the producer is commented out in `AppUpdateChecker.kt`)
  - `NetworkToLocalAnime`'s unused `sourceManager` parameter is not carried over
  - The user loses nothing (both are dead upstream)

### T-07a-043 — Property tests for flag words and recognition invariants
- Priority: P2
- Effort: S
- Depends: T-07a-002, T-07a-003, T-07a-004, T-07a-009
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeRecognition.kt
- Acceptance:
  - `proptest` (dev-dependency only) checks: set-then-get returns the value for every group of the three flag words; unrelated bits never change; 10 000 random names never panic in any recogniser; recognition result is `-1.0` or a finite non-negative number
  - Random `i64` words never panic any accessor

### T-07a-044 — Entry details: first-open flow of default flags and non-favourite refresh
- Priority: P1
- Effort: S
- Depends: T-07a-027, T-07a-020, area:10-app-domain-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt
- Acceptance:
  - Opening a non-favourite entry re-applies the default flags before the first list render (anime: episode and season defaults); favourites keep their stored words
  - Adding to the library from browse applies the default episode/chapter flags (not the season defaults)
  - Test: non-favourite with `chapter_flags = 0x101` opened with defaults 0 shows `chapter_flags == 0` afterwards; favourite unchanged
