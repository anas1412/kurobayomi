# Area 08 — Domain: library, category, history, updates — overview

Area lead overview for the Kurobayomi plan. Every path is repo-relative to the Aniyomi clone `/home/blackbox/aniyomi-src` unless it starts with `crates/` or `plans/` (Kurobayomi). OBSERVED = what the Kotlin does. Proposal = what Rust should do.

Status of the area documents: all 12 documents of this folder were read in full and the key claims were spot-checked against the Kotlin and the string resources (details in section 4 and section 8).

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

This area is the pure-domain layer for four concepts that the user sees every day:

- **Library**: the set of favourite manga and anime, shown as one page per category, with per-category sort (10 manga / 11 anime types, two directions), TriState filters (downloaded, unread/unseen, started, bookmarked, completed, custom update interval, tracker membership), search with an `id:` grammar, badges, and bulk category changes. Sources: `domain/src/main/java/tachiyomi/domain/library/**` (7 files), `domain/src/main/java/tachiyomi/domain/category/**` (28 files).
- **Categories**: user-defined groups with an ordering number (`sort` column, drifting, not dense), a hidden flag, per-category sort flags (`flags`), and membership rows per entry. Default category = id 0, system row, cannot be deleted.
- **History**: one row per chapter (manga) or episode (anime) with `last_read` / `last_seen` timestamps, manga-only `time_read` duration, the "history" list (one row per entry, newest first), and the "next chapter / next episode" algorithm used by Resume, download-ahead and the library's continue button. Sources: `domain/src/main/java/tachiyomi/domain/history/**` (17 files).
- **Updates**: the feed of new chapters/episodes of favourite entries fetched after the entry was added, limited to upload dates within the last 3 calendar months and to 500 rows. Sources: `domain/src/main/java/tachiyomi/domain/updates/**` (6 files).

Scope size (verified with `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 08-domain-library-category-history-updates 1`): 58 files, 2197 LOC, all in the Gradle module `domain`. No file renders UI. The UI consumers belong to other areas (section 5).

### 1.2 Why it matters for a usable Kurobayomi

- Without the library model there is no home screen: the library is the first thing the user opens. Without categories the library is one flat list, and the user loses the grouping they rely on.
- Without history there is no "continue reading / watching", no resume, and no progress memory. History also feeds the library's unread counts (`libraryView` joins `history` for `lastRead`).
- Without the next-item algorithm, "Resume" and "download next N" cannot pick the right chapter or episode.
- Without updates there is no "what is new" view, and the Updates tab badge has nothing to count.
- The preference keys of this area (87 accessors, including the six category-id sets) are part of the backup format. Keeping key names and value encodings byte-identical lets an Aniyomi backup restore into Kurobayomi with the same sort per category, the same default category and the same filters.

### 1.3 Port decisions in one table

| Kotlin concept | Port decision | Target |
|---|---|---|
| Manga/anime twin classes (most of the 58 files come in copy-paste pairs) | Rewrite as one code path with a runtime `EntryKind { Manga, Anime }` and data-driven differences | `kuro-domain::kind` |
| SQLDelight `Flow` (`subscribeToList`) | `tokio::sync::watch` fed by a change bus | area:03 / area:04 `ChangeBus`, T-08a-032 |
| `withNonCancellableContext` | Detached `tokio::spawn` plus await, so a closed window cannot abort a write | `p1-rust-design.md` section 6 |
| `java.text.Collator` at PRIMARY strength | `icu_collator` (pure Rust) at primary strength | T-08a-015 |
| `kotlin.random.Random(seed)` shuffle | `rand_chacha::ChaCha8Rng` plus Fisher-Yates, seeded by the stored seed | T-08a-015 |
| `ZonedDateTime.minusMonths(3)` | `time` crate with day-of-month clamp | T-08a-025 |
| Glance home-screen widgets | Dropped (section 6.3) | T-08a-038 |
| Injekt factories in `app/src/main/java/eu/kanade/domain/DomainModule.kt` | Explicit `Domain` struct built in `kuro-core::AppContext` | T-08a-042 |

---

## 2. Module maps

### 2.1 Kotlin packages and responsibilities

All paths below are under `domain/src/main/java/tachiyomi/domain/` unless stated otherwise.

| Package (dir) | Files | Responsibility |
|---|---|---|
| `category/model/` | 2 | `Category(id, name, order, flags, hidden)`, `CategoryUpdate` (partial update, `coalesce` semantics), `UNCATEGORIZED_ID = 0L` |
| `category/manga/interactor/` | 12 | `CreateMangaCategoryWithName`, `DeleteMangaCategory`, `GetMangaCategories`, `GetVisibleMangaCategories`, `HideMangaCategory`, `RenameMangaCategory`, `ReorderMangaCategory`, `ResetMangaCategoryFlags`, `SetMangaCategories`, `SetMangaDisplayMode`, `SetSortModeForMangaCategory`, `UpdateMangaCategory` |
| `category/manga/repository/` | 1 | `MangaCategoryRepository` (14 methods) |
| `category/anime/interactor/` | 12 | Same twelve as the manga package for anime (`ResetAnimeCategoryFlags` exists but is never called upstream) |
| `category/anime/repository/` | 1 | `AnimeCategoryRepository` (14 methods) |
| `history/manga/model/` | 3 | `MangaHistory`, `MangaHistoryUpdate`, `MangaHistoryWithRelations` |
| `history/manga/interactor/` | 5 | `GetMangaHistory`, `GetNextChapters` (3 overloads), `GetTotalReadDuration`, `RemoveMangaHistory`, `UpsertMangaHistory` |
| `history/manga/repository/` | 1 | `MangaHistoryRepository` (8 methods) |
| `history/anime/model/` | 3 | `AnimeHistory`, `AnimeHistoryUpdate`, `AnimeHistoryWithRelations` |
| `history/anime/interactor/` | 4 | `GetAnimeHistory`, `GetNextEpisodes` (3 overloads), `RemoveAnimeHistory`, `UpsertAnimeHistory` |
| `history/anime/repository/` | 1 | `AnimeHistoryRepository` (7 methods) |
| `library/model/` | 2 | `Flag.kt` (`Flag`, `Mask`, `FlagWithMask` and the `Long` operators), `LibraryDisplayMode.kt` (enum with codec) |
| `library/manga/` | 2 | `LibraryManga` (row model), `model/MangaLibrarySortMode.kt` (10 types, 2 directions, codec) |
| `library/anime/` | 2 | `LibraryAnime` (row model), `model/AnimeLibrarySortMode.kt` (11 types, 2 directions, codec) |
| `library/service/` | 1 | `LibraryPreferences` (428 lines, 87 accessors, enums `EpisodeSwipeAction`, `ChapterSwipeAction`, constants of update restrictions and duplicate marking) |
| `updates/manga/` | 3 | `GetMangaUpdates`, `MangaUpdatesWithRelations`, `MangaUpdatesRepository` |
| `updates/anime/` | 3 | `GetAnimeUpdates`, `AnimeUpdatesWithRelations`, `AnimeUpdatesRepository` |

Related packages outside this area that are read or written by this area:

- `domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt` (season display mode, `fromLong` throws, see Q11 in [p1-topic-library-preferences-quirks.md](p1-topic-library-preferences-quirks.md)).
- `data/src/main/sqldelight/data/categories.sq` and `data/src/main/sqldelightanime/dataanime/categories.sq`: category tables, system row seed, delete trigger.
- `data/src/main/sqldelight/data/history.sq`, `data/src/main/sqldelightanime/dataanime/animehistory.sq`: history upserts and resets.
- `data/src/main/sqldelight/view/libraryView.sq`, `data/src/main/sqldelightanime/view/animelibView.sq`: library rows.
- `data/src/main/sqldelight/view/historyView.sq`, `data/src/main/sqldelight/view/updatesView.sq`, `data/src/main/sqldelightanime/view/animeupdatesView.sq`: history and updates views.
- Consumers (UI and jobs, not ported here): `app/src/main/java/eu/kanade/tachiyomi/ui/library/`, `ui/category/`, `ui/history/`, `ui/updates/`, `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt`.

### 2.2 Proposed Rust module map

Crate `crates/kuro-domain` (no GTK, no `cfg(target_os)`, headless testable). Full signatures are in [p1-rust-design.md](p1-rust-design.md) section 3.

```
crates/kuro-domain/src/
  lib.rs            pub mod category, history, library, updates, kind, error
  kind.rs           EntryKind { Manga, Anime } with table/column accessors
  error.rs          DomainError { Db, InvalidName, DuplicateName, NotFound, SystemCategory, Join }
  category/
    model.rs        Category, CategoryUpdate, SYSTEM_CATEGORY_ID
    repo.rs         CategoryRepo (SQL over kuro_db::Db, one instance per kind)
    read.rs         all / visible / by_entry, watch_all / watch_visible
    write.rs        CategoryService: create, rename, toggle_hidden, move_before, delete
    membership.rs   set_entry_categories, resolve_add_target, bulk_change
    sort_mode.rs    set_sort_mode, reset_flags, initial_flags
  library/
    flags.rs        merge / contains (Flag algebra)
    sort.rs         SortType, Direction, LibrarySort, string codec
    display_mode.rs LibraryDisplayMode + codec
    model.rs        LibraryRow (one per favourite entry and category)
    repo.rs         LibraryRepo::all(), subscribe()
    pipeline.rs     build_library: group, display rule, filter, sort, search
    search.rs       matches(query, item, ctx)
    selection.rs    Selection (toggle, range, select_all, invert)
    prefs.rs        LibraryPrefs definition, re-exported by kuro-prefs::library
    restrictions.rs DEVICE_* and ENTRY_* constants, update candidate selection
    service.rs      LibraryService (tokio task, watch channel)
  history/
    model.rs        HistoryWithRelations, HistoryUpdate
    repo.rs         upsert, list(query), latest, by_entry, reset_one, reset_entry, clear_all, total_duration
    recorder.rs     HistoryRecorder (session timer, anime throttle, incognito gate)
    next_item.rs    next_items for chapters and episodes
    view_model.rs   date separators, resume, remove actions
  updates/
    model.rs        UpdateRow
    repo.rs         UpdatesRepo
    service.rs      UpdatesService and new-update counters
    window.rs       three_months_ago
```

Crates used (all stable, no `unsafe`): `kuro-db`, `kuro-prefs`, `tokio` (sync, rt, time), `rusqlite` (through `kuro-db`), `icu_collator` and `icu_locale`, `rand` and `rand_chacha`, `time` (formatting, local-offset, macros), `thiserror`, `tracing`, `serde` (only for enum codecs). Full table with the Android API each one replaces: [p1-rust-design.md](p1-rust-design.md) sections 1 and 8.

---

## 3. Reading guide

Read in this order: the inventory, the behaviour, the data, then the design. The topic files are deep dives and can be read on demand.

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Every one of the 58 scope files with LOC, every top-level declaration, purpose, port decision, target module, priority and the task ids that cover it. Count check at the end. |
| [p1-behavior.md](p1-behavior.md) | The algorithms, OBSERVED with source paths and line numbers, then Proposal. Sections: 1 Category (create, rename, reorder, delete, membership, sort per category, add-to-library default, bulk change, library updates, backup interplay), 2 Library (read models, display, pipeline, filters, sort comparators, search, selection, active category, sort sheet), 3 History (recording, upsert and reset, list query, history tab, next item, duration), 4 Updates (queries, tab behaviour), 5 Library preferences behaviour. |
| [p1-data-and-config.md](p1-data-and-config.md) | Every model field with Kotlin and Rust types, enum string codecs, bit layouts (sort flags, chapter and episode flags, season flags, anime viewer flags), the SQL statements that matter, the 87-row preference table with keys, namespaces and defaults, and the Rust data constants. |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, the twin-handling design (`EntryKind`), key type signatures, the pipeline and services, error type, non-cancellable writes, threading model, the Android-to-Linux mapping table, the desktop input mapping, the mockup integration table and ten pitfalls. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | The upstream test that is ported (`LibraryFlagsTest`), new test vectors per area, 23 edge cases (E1 to E23), six race conditions, fixtures and property tests. |
| [p1-tasks.md](p1-tasks.md) | 44 implementation tasks T-08a-001 to T-08a-044 in the required task format. Lint status: ok. |
| [p1-topic-flag-algebra-and-sort-modes.md](p1-topic-flag-algebra-and-sort-modes.md) | Full flag table for manga and anime sort types, the flag algebra worked examples, the Rust reference implementation of `LibrarySort`, the string codec rules and the sort application checklist. |
| [p1-topic-library-pipeline.md](p1-topic-library-pipeline.md) | The observed data flow of the library screen model, the order of filter, sort and search, the Rust `LibraryInputs` and `build_library` shape, triggers and threading, the selection helper. |
| [p1-topic-category-lifecycle.md](p1-topic-category-lifecycle.md) | How the `sort` column drifts (worked example), hidden categories, per-category versus global sort state, the Default category inside every algorithm, the Rust `CategoryService` surface. |
| [p1-topic-history-and-next-item.md](p1-topic-history-and-next-item.md) | History data semantics per kind, the list query with a worked example, the recording policy, the next chapter and next episode algorithm with the example table, date separators. |
| [p1-topic-updates-feed.md](p1-topic-updates-feed.md) | What counts as an update, the three queries and their windows, the counters and tab badge, display rules, the Rust `UpdatesService`. |
| [p1-topic-library-preferences-quirks.md](p1-topic-library-preferences-quirks.md) | 30 observed quirks and upstream defects (Q1 to Q30) with the Kurobayomi decision for each: keep, fix or irrelevant. |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the plan (no user call needed unless he objects)

| # | Decision | Where |
|---|---|---|
| D1 | Persisted layout stays identical to Aniyomi: same two SQLite schemas, same preference keys, same value encodings. Backups restore byte-compatible state. | [p1-data-and-config.md](p1-data-and-config.md) section 6 |
| D2 | One runtime enum `EntryKind` instead of generics or two code copies. Per-kind differences are data (table names, token tables, `AiringTime` existence). | [p1-rust-design.md](p1-rust-design.md) section 2 |
| D3 | Repositories are concrete structs, not traits. Only three seam traits: `DownloadIndex`, `SourceLookup`, `TrackScores`. | [p1-rust-design.md](p1-rust-design.md) section 1 (ponytail note) |
| D4 | Fix upstream quirks Q3, Q8, Q9, Q11, Q13, Q14, Q16, Q18, Q19, Q21, Q24, Q26, Q27, Q28 (first run only). Keep Q1, Q2 (keys kept, names by effect), Q4, Q5, Q6 (value kept, constant renamed), Q7, Q12, Q15, Q17, Q20 (hidden, see Q-E), Q22, Q23, Q25, Q29, Q30. Q10 is irrelevant in Rust. Full table in [p1-topic-library-preferences-quirks.md](p1-topic-library-preferences-quirks.md). | quirks document |
| D5 | Sort string codec and flag codec are lenient: unknown tokens fall back to Alphabetical, unknown direction falls back to Descending, nothing throws. | [p1-topic-flag-algebra-and-sort-modes.md](p1-topic-flag-algebra-and-sort-modes.md) section 5 |
| D6 | Anime history writes are throttled to at most one upsert per 5 s of playback, plus unconditional writes on pause, seek end, episode change, stop and close. Upstream wrote every second. | [p1-topic-history-and-next-item.md](p1-topic-history-and-next-item.md) section 3 |
| D7 | Writes that the user cannot retry (category create, delete, reorder, history upsert) run on detached tasks, so a closed window cannot cut them. | [p1-rust-design.md](p1-rust-design.md) section 6 |

### 4.2 Open questions that need the user's call

Each question has a recommendation. Where the plan already takes the recommended option, the task ids are given.

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q-A | The system Default category ships with `flags = 0`, which means sort Z to A. Keep upstream behaviour or fix it on first run? | (a) keep Z to A; (b) first run writes `64` (A to Z) | (b). Existing databases are left unchanged. Tasks: T-08a-006. |
| Q-B | The Categories screen hides hidden categories when `hidden_categories` is on, so they cannot be unhidden there. Keep or fix? | (a) keep; (b) always list hidden categories dimmed, and `hidden_categories` only filters the Storage screen | (b). Tasks: T-08a-036. |
| Q-C | Bulk "Set categories" drops hidden memberships of an entry (upstream defect Q26). | (a) keep; (b) read all categories of each entry before applying the change | (b). Tasks: T-08a-017. |
| Q-D | Category names: upstream checks duplicates only in the dialog, case-sensitively. Backup restore keys categories by exact name. | (a) allow duplicates; (b) reject duplicates after trim, case-sensitive | (b), so backup restore stays unambiguous. Tasks: T-08a-008. |
| Q-E | Customized update frequency filter (`pref_filter_library_interval_custom`) is hidden in release builds upstream (Q20). Kurobayomi has no release/debug split. | (a) show it always; (b) show only with `KUROBAYOMI_DEV=1`, as T-08a-039 currently says | (a). Kurobayomi is a single-user app and the filter is implemented. T-08a-039 currently prescribes (b). Decide before implementing T-08a-039. |
| Q-F | History search: upstream passes the raw query into `LIKE`, so `%` and `_` act as wildcards (Q18). | (a) keep wildcards; (b) escape with `ESCAPE '\'` | (b). Tasks: T-08a-018. |
| Q-G | Reading duration per session: upstream adds `now - start` with no sanity check (a suspended laptop could add days). | (a) no clamp; (b) clamp to `0..12 h` per flush | (b). Tasks: T-08a-019. |
| Q-H | Random sort: upstream uses `java.util.Random`-compatible seeded shuffle; Rust uses ChaCha8. The order differs from Aniyomi for the same seed. | (a) byte-compatible shuffle; (b) accept a different order | (b). The seed is device-local state, and only stability within a session matters. Tasks: T-08a-015. |
| Q-I | Updates tab window: upstream filters on upload date within three months (Q17). An old chapter first discovered today never appears. | (a) keep upstream rule; (b) filter on fetch date | (a). It is the intended "new releases only" rule. Tasks: T-08a-024. |
| Q-J | Home-screen widgets (Glance) are not ported. | (a) drop; (b) rebuild as a KDE Plasma widget or tray menu | (a) now. `UpdatesRepo::by_status(kind, false, three_months_ago)` stays available for a later widget. Tasks: T-08a-038. |
| Q-K | Library search grammar quirk: `-xyz` matches every entry whose source and genres do not contain `xyz` (behaviour 2.6 step 3). | (a) keep; (b) require a non-empty positive part | (a). Documented behaviour, pinned by a test vector. Tasks: T-08a-016. |

### 4.3 Inconsistencies found in the area documents (to reconcile before implementation)

| # | Inconsistency | Where | Resolution in this overview |
|---|---|---|---|
| I-1 | The topic file shows `CategoryService` methods that take `kind: EntryKind` as a parameter, while the Rust design uses one service instance per kind. | [p1-topic-category-lifecycle.md](p1-topic-category-lifecycle.md) section 5 versus [p1-rust-design.md](p1-rust-design.md) section 3 | Use the per-kind instance of the Rust design. The `kind` parameter in the topic file is the older draft. |
| I-2 | The topic file shows `LibraryInputs<K: EntryKind>` (generic), while the Rust design uses a runtime enum everywhere. | [p1-topic-library-pipeline.md](p1-topic-library-pipeline.md) section 2 versus [p1-rust-design.md](p1-rust-design.md) section 2 | Use the runtime enum. Generics would double the compile units for no gain. |
| I-3 | The behaviour document refers to "Kobayomi" (should be Kurobayomi) in the note on the category screen in section 1.5. | [p1-behavior.md](p1-behavior.md) section 1.5 | Typo only. |
| I-4 | The task for the custom-interval filter gates it behind an environment variable, while the quirks document keeps the upstream hidden state (Q20) and this overview recommends showing it (Q-E above). | [p1-tasks.md](p1-tasks.md) T-08a-039 versus [p1-topic-library-preferences-quirks.md](p1-topic-library-preferences-quirks.md) Q20 | Open, see Q-E. |

### 4.4 Spot-check results (source against documents)

Checked against the Kotlin and the string resources. All matched.

- `domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt` lines 36 to 42: the list is `mangaUpdateCategories()` twice, then the remove-exclude and download-new sets, and `mangaUpdateCategoriesExclude()` is missing (Q8).
- `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` lines 361 to 375: swipe keys are crossed as documented (Q2). The file has 87 `preferenceStore.get` calls, matching the 87 accessors of the plan.
- Line 331 of the same file: `filterSeasonByDownload().set(anime.seasonUnseenFilterRaw)` (Q3).
- `data/src/main/sqldelight/data/categories.sq`: the seed row `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);` and the delete trigger with message `System category can't be deleted`.
- `i18n/src/commonMain/moko-resources/base/strings.xml` (lines 849, 919 to 923): `relative_time_span_never` = "Never", `skipped_reason_not_always_update` = "Skipped because series does not require updates", `skipped_reason_completed` = "Skipped because series is complete", `skipped_reason_not_caught_up` = "Skipped because there are unread chapters", `skipped_reason_not_started` = "Skipped because no chapters are read", `skipped_reason_not_in_release_period` = "Skipped because no release was expected today".
- `i18n/src/commonMain/moko-resources/base/strings.xml`: `updates_last_update_info_just_now` = "Just now", `error_category_exists` = "A category with this name already exists!", `no_next_chapter` = "Next chapter not found", `internal_error` = "InternalError: Check crash logs for further information", `clear_history_completed` = "History deleted".
- `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`: `no_next_episode` = "Next Episode not found!", `recent_anime_time` = "Ep. %1$s - %2$s".

---

## 5. Cross-area dependencies

Only area folders 01 to 13 exist in `plans/` today. Areas named below that have no folder are shown as "not yet authored". Their tasks keep the dependency as a Note line, not as a `Depends` token, because the task linter only accepts real folder ids. Provisional mappings are marked.

### 5.1 What this area needs from others

| Need | Owner | Status | Used by |
|---|---|---|---|
| Crate skeleton, Tokio runtime, `AppContext`, local time offset, single-instance lock, shutdown wait for pending writes | area:01-foundation-app-bootstrap | Folder exists | T-08a-001, T-08a-019 |
| Typed preferences, `define_prefs!`-style macro, change channels per key, `Preference` namespaces (normal and app-state) | area:02-preferences-system | Folder exists | T-08a-026 and the eight tasks that depend on it |
| `manga.db` and `anime.db` schemas, `categories` / `history` / `animehistory` tables, `libraryView`, `updatesView`, `animelibView`, the `ChangeBus`, the membership version trigger, `Db::in_memory` | area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations | Folders exist | T-08a-005, T-08a-007, T-08a-018, T-08a-024, T-08a-032 |
| Entry (manga and anime) repository and membership writes | area:05-data-repos-entries-items | Folder exists | T-08a-005, T-08a-011, T-08a-022 |
| History, track, updates and handler repository implementations | area:06-data-repos-history-track-source-misc | Folder exists | T-08a-018, T-08a-024 |
| Entity models `Manga` and `Anime`, cover models, chapter and episode sorters, default chapter and season flags | area:07-domain-entries-items | Folder exists | T-08a-022, T-08a-027 |
| Download index (`DownloadIndex::count`), track models and scores (`TrackScores`), the five download-category preference sets | area:09-domain-source-track-download-release-misc | Folder exists. Provisional for tracker framework and download runtime. | T-08a-010, T-08a-014, T-08a-015, T-08a-041 |
| Source lookup (`getOrStub`, language and display name), add-to-library entry points | area:10-app-domain-layer | Folder exists. Provisional for source layer (area:17 folds into area:10). | T-08a-041, T-08a-030 |
| Tracker framework (`area:27-tracking-core`) | not yet authored | Provisional mapping to area:09 for domain track types | T-08a-014, T-08a-015 |
| Library update jobs, `library_update_last_timestamp` writer, update candidate consumers (`area:26-library-update-jobs`) | not yet authored | Needed for the "Update library" action and the update counters | T-08a-025, T-08a-028 |
| Backup creators and restorers (`area:21-backup-format-and-create`, `area:22-backup-restore`) | not yet authored | Needed for category export, restore remapping | T-08a-035 |
| Image cache (`area:34-images-covers-cache`) | not yet authored | Needed for the auto-clear cache on launch | T-08a-037 |
| Reader and player recording call sites (`area:43-reader-core`, `area:45-player-core`) | not yet authored | Needed to create history rows at all. This is the largest external block on P0. | T-08a-019 |
| Download runtime (`area:23-downloads-manga`, `area:24-downloads-anime`) | not yet authored | Needed for badges and download-ahead | T-08a-041 |

### 5.2 What others need from this area

| Consumer | Needs | Task |
|---|---|---|
| Library UI (`area:37-ui-library`, not yet authored; interim: mockup `src/screens.rs`) | `LibraryService::state()`, `LibraryPrefs`, `CategoryService`, `Selection`, `bulk_change`, search `matches` | T-08a-013 to T-08a-017, T-08a-033 |
| History and updates UI (`area:42-ui-updates-history-stats-storage-upcoming`, not yet authored) | `HistoryRepo::list`, `UpdatesService`, `reset_entry`, `reset_one`, `clear_all`, date separators, `total_read_duration_ms` for the statistics screen | T-08a-020, T-08a-021, T-08a-023, T-08a-025, T-08a-034 |
| Reader (`area:43-reader-core`) and player (`area:45-player-core`) | `HistoryRecorder`, `next_items` for resume and download-ahead | T-08a-019, T-08a-022 |
| Download manager (`area:23`, `area:24`) | `next_items(kind, entry, from, only_pending)` for "download next N" | T-08a-022 |
| Library update jobs (`area:26`) | `update_candidates`, `skip_reason`, `reset_new_counts`, `UPDATES_LIMIT`, `UPDATES_WINDOW_MONTHS` | T-08a-025, T-08a-028 |
| Backup (`area:21`, `area:22`) | Category export and restore helpers, the six library category-id keys and six download category-id keys | T-08a-035 |
| Settings screens (`area:47-settings-screens-general`, not yet authored) | Default-category preference list (`default_category`, `default_anime_category`) and the "Clear database" action `remove_reset_history()` | T-08a-030, T-08a-018 |
| Statistics (`area:42`) | `total_read_duration_ms()` | T-08a-023 |

---

## 6. Task summary and critical path

### 6.1 Counts

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export` followed by the rows of `/home/blackbox/kurobayomi/plans/TASKS.csv` whose file starts with `08-domain-library-category-history-updates/`.

Total: 44 tasks (T-08a-001 to T-08a-044).

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 7 | 11 | 2 | 0 | 20 |
| P1 | 6 | 10 | 0 | 0 | 16 |
| P2 | 6 | 0 | 0 | 0 | 6 |
| P3 | 2 | 0 | 0 | 0 | 2 |
| **Total** | **21** | **21** | **2** | **0** | **44** |

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 08-domain-library-category-history-updates/p1-tasks.md` prints `ok`. The export reports no errors whose file is in this area. Other areas still show 61 export errors (unknown area tokens and dangling dependencies) that belong to their own folders.

### 6.2 P0 critical path (in order)

Effort in brackets. Tasks in the same line can run in parallel.

1. T-08a-001 [S] kuro-domain skeleton, `EntryKind`, `DomainError`. Needs area:01.
2. T-08a-002 [S] flag algebra. Then T-08a-004 [S] display mode, and T-08a-006 [S] category model, in parallel with step 2.
3. T-08a-003 [M] `LibrarySort` codec (needs 002).
4. T-08a-032 [M] change bus and watch plumbing (needs area:03 and area:04 schemas, and 001).
5. T-08a-007 [M] category repository (needs 006 and 032).
6. T-08a-026 [M] `LibraryPrefs`, 87 accessors (needs area:02, 003, 004).
7. T-08a-005 [M] library rows (needs area:03, area:04, area:05, 001).
8. T-08a-013 [M] library page model, Default-category rule, hidden categories (needs 005, 007, 026).
9. T-08a-014 [M] filter engine, T-08a-015 [L] sort engine, T-08a-016 [S] search grammar. These three run in parallel after step 8.
10. T-08a-011 [S] set entry categories, T-08a-012 [M] per-category sort setter and initial flags, T-08a-030 [S] add-to-library default. These run in parallel with step 9 once 007 and 026 are done.
11. T-08a-018 [M] history models and repository (needs area:03, area:04, area:06, 032).
12. T-08a-022 [M] next chapter and next episode (needs 018, area:05, area:07).
13. T-08a-019 [M] history recording (needs 018 and the reader and player areas, not yet authored). This is the first P0 task that cannot be finished inside this plan alone.
14. T-08a-033 [L] wire library, categories and category dialogs into the mockup (needs 013 to 017, 030 and the P1 category tasks it lists). This is the first end-to-end P0 milestone.

Not on the P0 path but needed for a usable first release: T-08a-008 to T-08a-010 (category create, rename, reorder, delete, P1), T-08a-017 (bulk change, P1), T-08a-020 and T-08a-021 (history list and reset, P1), T-08a-024 and T-08a-025 (updates, P1).

### 6.3 Dropped and Android-only items

| Task | Decision | What the user loses |
|---|---|---|
| T-08a-038 | Drop Glance home-screen widgets (updates grid). Replacement: Updates tab and area:26 notifications. | The launcher widget. Nothing else. |
| T-08a-042 | Drop Injekt wiring, `Serializable`, `logcat`, Compose state, Flow facades. Replacement: explicit `Domain` struct, `tracing`, `Result`, `watch`. | Nothing functional. |

Haptic feedback on long press and pull-to-refresh are dropped as Android input features. Their desktop replacements are in [p1-rust-design.md](p1-rust-design.md) section 8.

### 6.4 Risks

| # | Risk | Effect | Mitigation |
|---|---|---|---|
| R-1 | Area folders 21, 22, 23, 24, 26, 27, 34, 37, 42, 43, 45 and 47 do not exist yet. Most P0 reading and watching use depends on 43 and 45. | T-08a-019 cannot be completed, so no history row is ever written by the reader. | Write those areas next, or stub the recorder call site and mark T-08a-019 partial. |
| R-2 | Provisional mappings (27 to 09, 23 and 24 to 09, 17 to 10). | A later area may own the concept differently and re-open these dependencies. | Keep the Note lines in the tasks file; re-map when the real area lands. |
| R-3 | `icu_collator` at primary strength may differ from `java.text.Collator` on non-Latin titles. The test vectors cover Latin and one accented example. | Sort order differs for some titles. | Add a golden test per script family before T-08a-015 ships. |
| R-4 | Area 02 is a dependency of eight tasks through T-08a-026. | Any delay there delays category, library and sort work. | Keep T-08a-026 small: keys and defaults first, the registry test second. |
| R-5 | Deliberate upstream-bug fixes (Q3, Q8, Q9, Q26, Q28, Q27) change behaviour. A restored backup can therefore look different from the Android app for the same data. | User confusion when comparing screens. | Manual checklist of T-08a-040 with a real backup. |
| R-6 | SQLite rowid reuse: after the highest category id is deleted, a new category can take the same id, and a stale id in an exclude set would then silently exclude it from updates. | Silent update exclusion. | T-08a-010 cleans all five sets per kind. Restore remapping (T-08a-035) must keep working. |
| R-7 | History flushes on window close race with process exit. | Last session time lost. | Shutdown waits up to 2 s for pending flushes (T-08a-019, `p1-edge-cases-and-tests.md` race 4). |
| R-8 | Watch channels keep only the latest value. A burst of toast events could be lost. | Missing "History deleted" or error toast. | Toasts use `mpsc::unbounded_channel` (pitfall 8 in [p1-rust-design.md](p1-rust-design.md)). |

---

## 7. Definition of done for this area

Each item must be observable in the running Kurobayomi (or by the named test), not only in code.

1. `cargo test -p kuro-domain` passes headless. It includes the ported `LibraryFlagsTest` vectors and every vector of [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 2.
2. Categories: create, rename, hide, drag-reorder, and delete work from the Categories screen. Deleting a category removes it from the five preference sets for its kind and the entries that lost their only category appear in Default. Default shows "Default" and always sorts first.
3. First run on an empty database: the Default category sorts A to Z (`flags = 64`). An existing Aniyomi database keeps its own flags.
4. Library: one page per visible category in sort order. The Default page is hidden when the library has entries and none is in Default. An entry in two categories appears on both pages, and the library count is distinct by entry.
5. Sort per category: choosing a sort on one tab changes only that tab when "per-category settings" is on, and changes all tabs when it is off. Switching it off resets every category to the global sort, for both manga and anime.
6. Filters: TriState cycling works, filters combine in the documented order, the tracker filter ignores logged-out trackers, and "Downloaded only" forces the downloaded filter on.
7. Search: plain text, `id:<n>`, and comma-separated sub-constraints with `-` negation behave as in [p1-behavior.md](p1-behavior.md) section 2.6. The tab badges show match counts while searching.
8. Bulk "Set categories" on a selection preserves hidden memberships of each entry, and the common and mixed tri-state works.
9. Reading or watching a chapter or episode creates a History row immediately. The History tab lists one row per entry, newest first, with date headers. Resume opens the next pending item. Reset one, reset entry, and clear all work with the exact dialog texts.
10. Anime playback writes history at most once per 5 s, plus on pause, seek end, episode change, stop and close.
11. Updates tab: favourites only, items fetched after the entry was added, upload date within three months, at most 500 rows, newest fetch first. Opening the tab resets the new-updates counter.
12. A real Aniyomi backup, supplied by the user, restores categories with the same names and order. Each category shows the same sort as on the Android device, and `categorized_display` is recomputed as documented. This is the manual check of T-08a-040.
13. Every preference key listed in [p1-data-and-config.md](p1-data-and-config.md) section 5 exists with the documented default. The registry test in T-08a-026 passes.
14. Nothing in this area creates a Glance widget, a Compose object, or a Java class. `grep -ri "androidx\|kotlinx\|injekt" crates/kuro-domain` returns nothing.

---

## 8. Notes on this document

- Counts in this document were produced from `plans/TASKS.csv` after `tasks.py export`, not typed by hand.
- Only the dependency tokens of area 08 were changed. Task ids were not changed. Each dependency that could not be expressed with an existing folder id was moved into a `- Note:` line directly under `- Depends:` in [p1-tasks.md](p1-tasks.md). The 13 tasks affected are T-08a-014, T-08a-015, T-08a-019, T-08a-025, T-08a-028, T-08a-033, T-08a-034, T-08a-035, T-08a-036, T-08a-037, T-08a-041, T-08a-043 and T-08a-044.
- Prose references to area ids inside the other area documents (for example `area:52-util-app` in [p1-topic-updates-feed.md](p1-topic-updates-feed.md)) were not changed. They are not dependency tokens and the linter does not read them.
