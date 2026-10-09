# Area 07 — Domain: entries (manga/anime/season) and items (chapter/episode/season)

Overview for area 07. Every path is repo-relative to `/home/blackbox/aniyomi-src` unless it starts with `kuro-` or `plans/` (Kurobayomi side). Area ids are `area:<id>` with ids from `/home/blackbox/kurobayomi/plans/`.

## 1. Purpose and scope

### 1.1 What Aniyomi does here

The `domain` Gradle module is pure Kotlin with no UI. It defines:

- The entry models `Manga`, `Anime` (with their `*Update` partial-update structs and `*Cover` cache keys), and `SeasonAnime` (a child anime plus episode statistics).
- The item models `Chapter` and `Episode` (with `*Update` structs). Seasons are anime rows whose `parent_id` points at a parent with `fetch_type = Seasons`.
- Bit-flag words (`chapter_flags`, `episode_flags`, `season_flags`, `viewer_flags`) that store every list filter, sort, display and overlay choice, plus the masks and constants that read and write them.
- The "Set*Flags" writers, the "Get*" read facades, the "Update*" facades, and the "SetDefault*Flags" writers that copy library preferences onto entries.
- Three recognisers that derive chapter, episode and season numbers from free-text titles.
- Three comparators (chapter, episode, season) with a collator for alphabetical order.
- Missing-item counts and gaps (the "Skipping N chapters" separators).
- The `should_update` diffs that decide whether a stored row must be rewritten from the source.
- The smart-update ("fetch interval") algorithm that predicts the next release date.
- A small release-check logic (`GetApplicationRelease`), which is in scope only through its test.

### 1.2 Why it matters for a usable Kurobayomi

Everything a reader or player sees in the details screen comes from this area: which chapters or episodes are listed and in what order, the "Missing N chapters" markers, the next-update prediction, the season grid settings, and the default filters that every new entry inherits. The library updater, the sync flow in `area:10-app-domain-layer`, and the backup format (flag words and key names) all depend on the exact numbers defined here. A wrong bit, sort code or recognised number silently corrupts a library, so parity with stored data is the top rule. This area has 21 P0 tasks.

### 1.3 Scope size

- 74 files, 4 041 lines (`p1-inventory.md`): 68 main-source files (3 147 lines) and 6 test files (894 lines).
- 44 tasks (`T-07a-001` to `T-07a-044`). Priority: P0 21, P1 14, P2 5, P3 4. Effort: S 36, M 8, L 0, XL 0.
- No file in scope renders UI. The mockup binding (`T-07a-040`) is the only UI task.

### 1.4 Out of scope (owned elsewhere)

- Repository SQL and the concrete `MangaRepo`, `AnimeRepo`, `ChapterRepo`, `EpisodeRepo` types: `area:05-data-repos-entries-items`. This area takes these as given and only pins the behaviour it needs.
- Library flags (`Flag`, `LibraryDisplayMode`, `MangaLibrarySort`, `AnimeLibrarySort`): `area:08-domain-library-category-history-updates`.
- The `TriState` enum type (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`): `area:02-preferences-system`.
- The sync interactors that call this area (`SyncChaptersWithSource`, `SyncEpisodesWithSource`, `SyncSeasonsWithSource`, `UpdateManga`, `UpdateAnime`, `SetAnimeViewerFlags`) and the details screen models: `area:10-app-domain-layer`.
- The release service and the GitHub client: `area:09-domain-source-track-download-release-misc` and `area:06-data-repos-history-track-source-misc`.
- `compareToWithCollator` (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt`): `area:13-core-util-storage-archive-torrent`.

## 2. Module map

### 2.1 Kotlin packages to responsibilities

File counts and lines come from `p1-inventory.md` (scope files only).

| Package (under `domain/src/main/java/`) | Files | LOC | Responsibility |
|---|---|---|---|
| `tachiyomi/domain/entries` | 2 | 12 | `applyFilter` over `TriState`; `EntryCover` marker |
| `tachiyomi/domain/entries/manga/model` | 3 | 208 | `Manga` (23 fields), `MangaUpdate` (20 optional fields), `MangaCover`; flag accessors; `create()` |
| `tachiyomi/domain/entries/manga/interactor` | 10 | 411 | `GetManga*`, `GetLibraryManga`, `GetMangaWithChapters`, `NetworkToLocalManga`, `MangaFetchInterval`, `SetMangaChapterFlags`, `ResetMangaViewerFlags`, `GetDuplicateLibraryManga` |
| `tachiyomi/domain/entries/manga/repository` | 1 | 41 | `MangaRepository` interface, 16 methods (replaced, see D5) |
| `tachiyomi/domain/entries/anime/model` | 5 | 453 | `Anime` (31 fields), `AnimeUpdate` (28 optional fields), `AnimeCover`, `AnimeRelationGroup`, `NoSeasonsException` |
| `tachiyomi/domain/entries/anime/interactor` | 12 | 653 | Anime twins of the manga interactors, `GetRelatedAnime`, `GetAnimeWithEpisodesAndSeasons`, `SetAnimeEpisodeFlags`, `SetAnimeSeasonFlags`, `AnimeFetchInterval` |
| `tachiyomi/domain/entries/anime/repository` | 2 | 70 | `AnimeRepository` (21 methods) and `AnimeRelationRepository` (3 methods); both replaced |
| `tachiyomi/domain/items/chapter/model` | 3 | 88 | `Chapter` (14 fields), `ChapterUpdate` (12 optional fields), `NoChaptersException` |
| `tachiyomi/domain/items/chapter/interactor` | 6 | 143 | `GetChapter*`, `GetChaptersByMangaId`, `UpdateChapter`, `ShouldUpdateDbChapter`, `SetMangaDefaultChapterFlags` |
| `tachiyomi/domain/items/chapter/service` | 3 | 206 | `ChapterRecognition`, `ChapterSorter`, `MissingChapters` |
| `tachiyomi/domain/items/chapter/repository` | 1 | 30 | `ChapterRepository` (11 methods; replaced) |
| `tachiyomi/domain/items/episode/model` | 3 | 116 | `Episode` (19 fields), `EpisodeUpdate` (17 optional fields), `NoEpisodesException` |
| `tachiyomi/domain/items/episode/interactor` | 6 | 150 | `GetEpisode*`, `GetEpisodesByAnimeId`, `UpdateEpisode`, `ShouldUpdateDbEpisode`, `SetAnimeDefaultEpisodeFlags` |
| `tachiyomi/domain/items/episode/service` | 3 | 217 | `EpisodeRecognition`, `EpisodeSorter`, `MissingEpisodes` |
| `tachiyomi/domain/items/episode/repository` | 1 | 26 | `EpisodeRepository` (9 methods; replaced) |
| `tachiyomi/domain/items/season/interactor` | 3 | 76 | `GetAnimeSeasonsByParentId`, `SetAnimeDefaultSeasonFlags`, `ShouldUpdateDbSeason` |
| `tachiyomi/domain/items/season/service` | 2 | 173 | `SeasonRecognition`, `SeasonSorter` |
| `aniyomi/domain/anime` | 2 | 74 | `SeasonAnime` (child plus episode statistics), `SeasonDisplayMode` (grid mode, 2 bits) |
| Tests (`domain/src/test/java/...`) | 6 | 894 | `MangaFetchIntervalTest`, `AnimeFetchIntervalTest` (10 tests each), `ChapterRecognitionTest` (34 tests), `MissingItemsTest` (8), `LibraryFlagsTest` (5), `GetApplicationReleaseTest` (4) |

### 2.2 Proposed Rust module map

Target crate: `kuro-core` (no GTK code; GTK is in the UI crate). Layout from `p1-rust-design.md` section 1:

```
kuro-core/src/
  data/models.rs            structs Manga, Anime, Chapter, Episode, *Update, SeasonAnime, LibraryManga, LibraryAnime (owned by area:05)
  domain/
    flags.rs                set_flag, TriState, apply_filter
    entries/
      manga.rs              chapter_flags constants, impl Manga accessors
      anime.rs              episode_flags, season_flags, viewer_flags constants, impl Anime accessors
      season_display.rs     SeasonDisplayMode
      update.rs             to_update(), apply_update() for tests
      cover.rs              MangaCover, AnimeCover (cache keys)
      season_anime.rs       SeasonAnime methods
      fetch_interval.rs     calculate_interval, get_window, calculate_next_update
      queries.rs            get/subscribe facades, favourites, duplicates, library subscription
      network_to_local.rs   insert-or-reuse in one writer transaction
      flag_setters.rs       chapter/episode/season setters, pure and masked-write forms
      default_flags.rs      default flag application, reset viewer flags
    items/
      model.rs              Chapter/Episode create, copy_from, is_recognized_number, to_update
      recognition.rs        Recogniser, parse_chapter_number, parse_episode_number, parse_season_number
      sorter.rs             chapter_cmp, episode_cmp, season_cmp, sort_seasons (ICU collator)
      missing.rs            missing_items_count, chapter_gap, gap
      should_update.rs      should_update_db_chapter / episode / season
      queries.rs            item get/update facades (_or_none, _or_empty, _logged contracts)
    errors.rs               NoChapters, NoEpisodes, NoSeasons
    release.rs              GetApplicationRelease logic (P2)
```

Dependency direction is `domain -> data`, never the reverse. Structs are defined once in `data/models.rs`; this area adds `impl` blocks and constants. Recognisers, comparators, missing counts, fetch interval and should_update are pure functions. Repository calls are plain async functions that take `&MangaRepo`, `&AnimeRepo`, `&ChapterRepo` or `&EpisodeRepo` (decision D5). Crates: `regex`, `chrono` (`clock`), `icu_collator` with `sys-locale`, `serde_json`, `tracing`, `thiserror`, `tokio`. No `bitflags`, no `async-trait`, no repository traits.

## 3. Reading guide

The folder has 12 documents. Read in this order: behaviour first, then data, then decisions, then tasks.

1. [p1-inventory.md](p1-inventory.md): one row per scope file (74 files) with LOC, declarations, port decision, target module and task ids. Start here to find any Kotlin file.
2. [p1-behavior.md](p1-behavior.md): the observed behaviour of every interactor, flag setter, default-flag call site, query error contract, recognition and sorting summary, missing-count algorithm, should_update rules, fetch-interval summary, and concurrency notes.
3. [p1-data-and-config.md](p1-data-and-config.md): the field tables for every model with defaults, the full flag bit layouts for manga, anime episodes, anime seasons and anime viewer, the `LibraryPreferences` key table, constants, and the verbatim regular expressions.
4. [p1-topic-flags-bit-layouts.md](p1-topic-flags-bit-layouts.md): bit maps (bit 0 is the least significant) and worked hex vectors for every flag word, plus the Rust representation proposal.
5. [p1-topic-item-number-recognition.md](p1-topic-item-number-recognition.md): the 7-step recognition algorithm, the regexes per recogniser, the Rust `regex` rewrite of look-behind, and all verified vectors (57 upstream chapter assertions, 56 new chapter vectors, episode and season prototype values).
6. [p1-topic-sorting-and-filtering.md](p1-topic-sorting-and-filtering.md): the chapter, episode and season comparators, the collator behaviour, and the filter pipeline order used by the details screen.
7. [p1-topic-fetch-interval.md](p1-topic-fetch-interval.md): the smart-update algorithm (`getWindow`, `toMangaUpdate`, `calculateInterval`, `calculateNextUpdate`), the quirks it keeps for parity, worked vectors, the 10 upstream tests and the new tests.
8. [p1-topic-release-check.md](p1-topic-release-check.md): the `GetApplicationRelease` flow, the `isNewVersion` defects, the four upstream tests, and the Kurobayomi decision (opt-in, one repository constant, no asset selection).
9. [p1-topic-quirks-and-decisions.md](p1-topic-quirks-and-decisions.md): the 22 observed quirks (Q1 to Q22) with policy (fix or preserve) and the 9 design decisions (D1 to D9).
10. [p1-rust-design.md](p1-rust-design.md): the Rust module layout, the crates, the signatures of every public function, the concurrency model, the error model, the mockup binding table (`spec/entries.json` sheets to setter calls), and the Android-to-Rust library mapping.
11. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases and failure modes per area, the race conditions, the upstream tests to port with their Rust names and data, the new tests to write, and the fixtures.
12. [p1-tasks.md](p1-tasks.md): the 44 task blocks (`T-07a-001` to `T-07a-044`) in the format checked by `_tools/tasks.py lint`. Export to [../TASKS.csv](../TASKS.csv) with `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`.

## 4. Key design decisions and open questions

### 4.1 Decisions already taken (from `p1-topic-quirks-and-decisions.md`)

| Id | Decision | Recommendation to keep |
|---|---|---|
| D1 | Recognition uses the `regex` crate with a consuming rewrite of look-behind; `fancy-regex` only if a vector fails | Keep. All 57 upstream vectors were verified with the rewrite. |
| D2 | `chrono` (`clock` feature) for local-day arithmetic; epoch milliseconds as `i64` at module boundaries | Keep. |
| D3 | `icu_collator` at PRIMARY strength for alphabetical sorts | Keep (see OQ-3 for the fallback). |
| D4 | Behavioural fixes are limited to Q1, Q2, Q8, Q13, Q16, Q17, Q20, Q21; everything else is bug-for-bug | Keep. Each fix has a regression test (T-07a-041). |
| D5 | No repository traits; functions take the concrete area:05 repo types | Keep. The in-memory SQLite harness from area:05 (T-05a-023) replaces mocks. |
| D6 | Kotlin "interactor per verb" classes become free functions grouped per file | Keep. |
| D7 | Flags are `i64` words with `const` masks; no `bitflags!` | Keep. Stored values are a file-format contract. |
| D8 | Facades that swallowed errors keep that contract under `_or_none`, `_or_empty`, `_logged` names | Keep, so the UI cannot gain new failure modes by accident. |
| D9 | `Clock` is injected (`now_ms`, zone) | Keep. Needed for the deterministic fetch-interval tests. |

### 4.2 Open questions that need the user's call

Each question has a recommendation. Only OQ-6 changes task ordering; the rest affect behaviour and can be decided while the P0 path runs.

- **OQ-1. Bug-for-bug or fix the recognition defects?** Q5 (capital `Hi10` never matches), Q6 (dotted titles give 2.108), Q7 (resolution-only names give 1080), and the ordinal behaviour (`2nd` gives 2.0 by accident) all change stored `chapter_number` or `episode_number`. A change would make `should_update_db_*` report existing rows as changed on the next sync. **Recommendation: keep parity** (preserve all four). Add the vectors as documentation tests. Revisit when a data migration can be run.
- **OQ-2. Share the two default preference keys or split them?** `default_chapter_display_by_name_or_number` and `default_chapter_sort_by_ascending_or_descending` are used by both manga and anime (Q3). Saving "set as default" on an anime overwrites the manga default and the reverse. **Recommendation: keep sharing for v1** so backups and copied preference files keep round-tripping. Add a later task with `default_episode_*` keys plus a migration.
- **OQ-3. Collator: `icu_collator` or `unicode-normalization` plus `to_lowercase`?** `icu_collator` is closest to Android's ICU-backed `Collator` and handles all scripts, but it adds compiled data to the binary. The fallback loses locale-specific orders (for example, `Ä` in Swedish). **Recommendation: `icu_collator`.** Check the binary-size cost once, then keep it.
- **OQ-4. Library retry cap.** Upstream retries a `NullPointerException` forever every 500 ms (Q16). **Recommendation: a cap of 20 attempts (10 s), then a visible error toast**, as in `p1-behavior.md` section 4.1. Alternative: retry forever with a visible spinner. The cap is the safer choice.
- **OQ-5. Season parent model.** Seasons are child `animes` rows under a parent with `fetch_type = Seasons` (ported as-is). **Recommendation: keep**, because existing libraries contain these rows. The alternative (flattening seasons into episodes) would need a migration and loses the season grid.
- **OQ-6. Priority of `T-07a-015` (missing counts, P1).** `T-07a-040` (the mockup binding, P0) depends on it, so a P0 task depends on a P1 task. **Recommendation: promote `T-07a-015` to P0.** The "Missing N chapters" separators are visible on the details screen, so a usable details screen needs them. Alternative: remove the dependency from `T-07a-040`. Not changed in this pass because task priority is the lead's call.
- **OQ-7. Release-check repository.** `GetApplicationRelease` needs one GitHub `owner/repo` string (`KUROBAYOMI_REPO`). The release check is opt-in (default off). **Recommendation: the user chooses the repository and confirms the opt-in default.** Nothing is sent to GitHub until the user turns the check on.
- **OQ-8. Airing and skip-intro data in `viewer_flags`.** `T-07a-005` (P1) keeps accessors and setters for the anime airing time and skip-intro length. These values come from AniList and Simkl, which are not in this area. **Recommendation: keep the accessors (they are needed to read stored backups) and defer any UI for airing data** until the tracker area exists in `plans/`.
- **OQ-9. Unknown sort codes.** `0xE000` in `season_flags` is an invalid sort code that Aniyomi reaches only through corrupted words (Q8). **Recommendation: fall back to source order and log a warning** (fix D4). Already in `T-07a-014`.

### 4.3 Open items for other areas

- area:05 must add a unique index on `(source, url)` for `mangas` and `animes` (or an equivalent `INSERT ... WHERE NOT EXISTS`). Without it the duplicate-insert race (edge cases section 2, item 1) is not closed. Tracked as a dependency of `T-07a-021`.
- area:05 must provide a masked `UPDATE ... SET chapter_flags = (chapter_flags & ~:mask) | (:flag & :mask)` form so flag setters can be race-free (edge cases section 2, item 2). Without it, `T-07a-024` to `T-07a-026` fall back to snapshot writes.
- area:02 owns the `TriState` type. Rust design section 3.1 says `T-07a-001` defines it if area:02 has not landed yet. The two areas must agree on one file.

## 5. Cross-area dependencies

### 5.1 What this area needs

| From | What is needed | Where it is used | Tasks |
|---|---|---|---|
| area:01-foundation-app-bootstrap | Workspace, `AppContext`, `tracing` setup, `Clock` injection, `kuro-testkit`, the tokio-to-GTK bridge | All tasks; recognition core is the first area 07 code to run | T-07a-001, T-07a-009, T-07a-032, T-07a-034, T-07a-039 |
| area:02-preferences-system | Preference store, typed `LibraryPrefs` accessors, `TriState` type | Default flags, "set as default", release-check opt-in | T-07a-027, T-07a-036, T-07a-001 |
| area:05-data-repos-entries-items | `models.rs` structs (field sets must match `p1-data-and-config.md`), `MangaRepo`/`AnimeRepo`/`ChapterRepo`/`EpisodeRepo`, `Subscription` (watch-based), COALESCE partial updates, `SeasonAnime` aggregate view, viewer-reset SQL, in-memory harness (T-05a-023), unique index (see 4.3) | Every query, write and subscription in this area | T-07a-002, T-07a-003, T-07a-006, T-07a-008, T-07a-017, T-07a-020 to T-07a-025, T-07a-028, T-07a-029, T-07a-033 |
| area:08-domain-library-category-history-updates | `Flag`, `LibraryDisplayMode`, `MangaLibrarySort`, `AnimeLibrarySort`, the library-sort collator use, the history playback-order callers | Port of `LibraryFlagsTest`; library sorts | T-07a-035 |
| area:09-domain-source-track-download-release-misc | Release service and installer ownership (installer and notifier are dropped here) | Release check | T-07a-036, T-07a-037 |
| area:06-data-repos-history-track-source-misc | Release service implementation (`ReleaseServiceImpl`) | Release check | T-07a-036 |
| area:10-app-domain-layer | Sync interactors that call this area, details-screen model, `UpdateManga`/`UpdateAnime` | Mockup binding, default-flag call sites, `should_update` callers | T-07a-040, T-07a-044 |
| area:13-core-util-storage-archive-torrent | `compareToWithCollator` (utility), cover cache location | Alphabetical sorts; cover cache keys | T-07a-013, T-07a-007 |
| The source API (no `plans/` folder yet) | Status constants (`SManga`/`SAnime`: `COMPLETED = 2`, `UPCOMING = 7`), source-listed chapter and episode fields | `expected_next_update`, sync mapping | T-07a-006 |

### 5.2 What other areas need from this area

- area:10-app-domain-layer needs: model constructors (`create`, `copy_from`, `to_update`), recognisers, comparators, `should_update`, fetch-interval `to_update`, `network_to_local`, the item and entry facades with their error contracts, and the default-flag appliers. Everything the sync interactors call is in this area.
- area:08-domain-library-category-history-updates needs: `get_chapter_sort` and `get_episode_sort` (playback order used by `GetNextChapters` and `GetNextEpisodes`, with `sortDescending = false`), the fetch window (`get_window`) for the library updater skip test, and the library subscription.
- The backup series (area:21 and area:22 in the upstream numbering; no `plans/` folder yet) needs: the flag words, the key names of section 4 of `p1-data-and-config.md`, and `fetch_interval` values. Backups must round-trip byte-for-byte (`p1-edge-cases-and-tests.md` section 4, item 17).
- The reader and player series (no `plans/` folder yet) needs: `set_flag` and the viewer flags (`ReadingMode`, `ReaderOrientation` masks stay in the reader), and the missing-chapter overload used for transition pages.
- The i18n catalog (no `plans/` folder yet) needs: the message keys listed in section 11 of `p1-behavior.md` and in `p1-rust-design.md` section 5.

## 6. Task summary

### 6.1 Counts (from `TASKS.csv`, rows whose file starts with `07-domain-entries-items/`)

| Priority | Count | Effort S | Effort M | Effort L | Effort XL |
|---|---|---|---|---|---|
| P0 | 21 | 16 | 5 | 0 | 0 |
| P1 | 14 | 11 | 3 | 0 | 0 |
| P2 | 5 | 5 | 0 | 0 | 0 |
| P3 | 4 | 4 | 0 | 0 | 0 |
| **Total** | **44** | **36** | **8** | **0** | **0** |

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 07-domain-entries-items/p1-tasks.md` prints `ok`. The export validator reports errors only in other areas (none in this one).

Effort upper bound for P0 by the rule (S at most 1 day, M at most 3 days): 16 x 1 + 5 x 3 = 31 days. This is an upper bound, not a schedule. Real parallelism is high at the start (see 6.2).

### 6.2 P0 critical path (in order)

1. **T-07a-001** (S): flag primitives `set_flag`, `TriState`, `apply_filter`. Depends on area:01. Everything else in the area uses it.
2. **T-07a-002**, **T-07a-003** (S, parallel): manga `chapter_flags` and anime `episode_flags` constants and accessors. Depend on 001 and area:05.
3. **T-07a-006** (S, parallel with 002): entry defaults, `to_update`, `apply_update`. Depends on area:05 only.
4. **T-07a-008** (S, parallel with 002): chapter and episode models (`create`, `copy_from`, update structs).
5. **T-07a-007** (S): cover descriptors. Depends on 006.
6. **T-07a-009** (M): recognition core (regex rewrite, 7-step algorithm). Depends on area:01. Start early; it is the largest pure-logic risk.
7. **T-07a-010**, **T-07a-011** (S, parallel): chapter recognition with the 34 upstream tests; episode recognition with tag stripping.
8. **T-07a-013** (M): chapter and episode comparators with the collator. Depends on 002, 003, 008.
9. **T-07a-016** (S): `should_update_db_chapter`, `_episode`, `_season` (Q1 fix). Depends on 008.
10. **T-07a-020** (M): read-side facades with the Kotlin error contracts. Depends on 006, 008, area:05.
11. **T-07a-022** (M): combined subscriptions and the library subscription retry. Depends on 020.
12. **T-07a-021** (S): `network_to_local` for manga and anime in one transaction. Depends on 006 and area:05 (unique index, see 4.3).
13. **T-07a-023** (S): item facades for chapter and episode (get and update). Depends on 008.
14. **T-07a-024**, **T-07a-025** (S, parallel): chapter and episode flag setters, with the sort-or-flip state machine. Depend on 002 and 003.
15. **T-07a-032**, **T-07a-033**, **T-07a-034** (S, parallel): error types and user messages; repository coverage matrix; fixtures and builders.
16. **T-07a-040** (M): bind the mockup entry screens to the domain flags. Depends on 013, 024, 025, 026 (P1), 015 (P1, see OQ-6), 022, and area:10. This task is the end of the P0 path and the proof that the screen works.

Work in parallel from the start: tasks 002, 003, 006, 008 and 009 do not depend on each other. A single engineer can do them in order 001, 002, 003, 006, 008, 009, 010, 011, 013, 016, 020, 022, 021, 023, 024, 025, 032, 033, 034, 040.

### 6.3 Risks

1. **Recognition parity.** The Kotlin regexes use look-behind; the Rust `regex` crate does not. The rewrite must give the same numbers for all 57 upstream vectors plus the 56 new ones. Mitigation: vectors as data (`kuro-core/tests/data/recognition_vectors.rs`); `fancy-regex` as fallback (D1). Episode and season recognisers have no upstream tests, only prototype values, so their expected numbers are not independently verified.
2. **Priority inversion** (OQ-6). `T-07a-040` (P0) depends on `T-07a-015` (P1).
3. **Duplicate rows from browse and global search.** Without the area:05 unique index, the race in `NetworkToLocal` can create two rows for one `(source, url)`. The race is rare, but the damage (two library entries for one title) is visible and hard to undo.
4. **Lost updates on flag writes.** Snapshot-based setters can overwrite a concurrent flag change. The masked-update form (see 4.3) removes the race; without it, the risk remains for fast double taps.
5. **Fetch-interval accuracy.** The algorithm has known parity quirks (Q9 to Q11): `nextUpdate` can be in the past for long-silent series, and the offset is taken from today's date. Changing these changes which library entries are refreshed. The DST and timezone tests need the `chrono` zone rules on the target machine.
6. **Collator cost.** `icu_collator` adds compiled data. Sorting a 5 000-chapter list must stay in milliseconds (rust design section 4); measure after `T-07a-013`.
7. **Missing upstream tests.** There are no upstream tests for episode or season recognition, and the sorting comparators have few. The new tests in `p1-edge-cases-and-tests.md` section 4 cover these; they encode the Kotlin behaviour as observed, not as verified against a real library.
8. **Unplanned areas.** The source API, tracker, backup, reader and player areas are referenced but do not have folders in `plans/` (only areas 01 to 13 exist). Every reference to them is written as plain text in these docs. When those areas appear, their ids must be added to the `area:` tokens of the affected tasks.
9. **Q1 behaviour change.** The should-update fix (D4) changes when `lastUpdate` is bumped for sources that report `dateUpload = 0`. This is intentional but visible in the "Latest update" sort.

## 7. Definition of done for this area in Kurobayomi

Each item is observable in the running app or by a test named in `p1-edge-cases-and-tests.md`.

1. **Chapter order.** In a manga details screen, the chapter list sorts by source, number, upload date or name as the sort sheet says, and tapping the same sort column twice flips the arrow. Tapping a different column always starts ascending. Tap sequence on Number from a fresh entry: ascending, descending, ascending (`0x201`, `0x200`, `0x201`).
2. **Episode order and filters.** The same behaviour for episodes, with the seen, bookmarked, fillermarked and downloaded filters combined with AND.
3. **Season grid.** The season list shows the grid mode (compact, comfortable, cover only, list), the grid size (0 to 15, clamped), the five overlays, and the title or number display, all restored after restart.
4. **Recognised numbers.** A set of 57 upstream chapter titles and their expected numbers passes (`ChapterRecognitionTest` port). The 56 new vectors in `p1-topic-item-number-recognition.md` section 5.2 pass.
5. **Missing counts.** A chapter list that starts at 5 and has no gaps shows "Missing 4 chapters" at the bottom, as upstream does. A gap between 3 and 7 shows "Skipping 3 chapters, either the source is missing them or they have been filtered out".
6. **No duplicate rows.** Opening browse or global search for the same source ten times produces exactly one row per `(source, url)`.
7. **Default flags.** Opening a non-library entry applies the current defaults. "Set as default" for chapters shows "Updated default chapter settings" and writes the two shared keys (OQ-2). "Reset to default" restores the defaults for that entry.
8. **Library never freezes.** If the library query fails transiently, the library grid recovers within 10 s or shows an error toast. It never waits forever (OQ-4).
9. **Corrupt flags do not crash.** A season word with sort bits `0xE000` opens the season list in source order and logs one warning. No panic.
10. **Next-update prediction.** For a manga with 10 chapters released weekly, the details dialog predicts "New chapters predicted to be released in around ... checking around every 7 days." The custom interval wheel from 0 to 28 is visible in release builds (Q20 fix).
11. **Manual interval.** Setting the interval to 5 in the dialog stores `fetch_interval = -5` and keeps it across library updates. Setting 0 restores automatic mode.
12. **Backup parity.** A flag word and fetch interval exported from Aniyomi and imported into Kurobayomi give the same filters, sort and interval, and the reverse. Checked by a round-trip test (`p1-edge-cases-and-tests.md` section 4, item 17).
13. **Mockup binding.** Every tristate, sort, display and overlay widget in the chapter, episode and season settings sheets writes its flag through the domain setter, and the screen re-renders from the subscription, not from local widget state (`T-07a-040`).
14. **Release check.** With the check off (default), no request is sent to GitHub. With it on, a release with the same version number as the running app shows no update, and a newer tag shows "New version X available" with an "Open release page" button (OQ-7).
15. **Viewer flags round trip.** Skip-intro length and airing values read from a stored anime word are the same numbers Aniyomi shows for that word (`T-07a-005`).
