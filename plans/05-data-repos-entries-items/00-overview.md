# Area 05 — Data layer repositories: entries, items, categories (overview)

Paths repo-relative to the read-only Aniyomi clone `/home/blackbox/aniyomi-src`. Rust targets are in the GTK-free crate `kuro-core` (module `kuro_core::data`) unless stated. Labels: **OBSERVED** = what the Kotlin/SQL does; **Proposal:** = what Rust should do.

## 1. Purpose and scope

**What Aniyomi does here.** Every entity Kurobayomi shows (manga, anime, chapters, episodes, seasons, categories, related-anime groups) is read and written through twelve Kotlin files that wrap SQL queries in SQLDelight. They are the only code that turns SQLite rows into domain objects, enforces partial-update semantics (`coalesce`), runs the library and season aggregate views, and exposes change streams (`Flow`) to the UI. The behaviour that matters to users lives mostly in the SQL: triggers that bump `version` and timestamps, `coalesce` updates, cascading deletes, and the `libraryView`/`animelibView` aggregates. Backup, restore, sync and the UI all depend on those details.

**Files in scope (12, 1 640 LOC, no `*Test.kt`):** see `p1-inventory.md`.

| Group | Files | Role |
|---|---|---|
| Column codecs | `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt` | epoch dates, genre list, enum ordinals, memo JSON |
| Categories | `data/src/main/java/tachiyomi/data/category/{manga,anime}/*CategoryRepositoryImpl.kt` | CRUD for the two `categories` tables |
| Entries | `data/src/main/java/tachiyomi/data/entries/manga/{MangaMapper,MangaRepositoryImpl}.kt`, `data/src/main/java/tachiyomi/data/entries/anime/{AnimeMapper,AnimeRepositoryImpl,AnimeRelationRepositoryImpl}.kt` | entity persistence, library views, seasons, related anime |
| Items | `data/src/main/java/tachiyomi/data/items/chapter/{ChapterRepositoryImpl,ChapterSanitizer}.kt`, `data/src/main/java/tachiyomi/data/items/episode/{EpisodeRepositoryImpl,EpisodeSanitizer}.kt` | chapter and episode persistence, name sanitizing |

**Why it matters for a usable Kurobayomi.** Without this layer the mockup's screens (`src/screens.rs`, `src/entry.rs`) keep showing placeholder data. Every P0 screen (library, entry details, reader and player progress, category tabs) needs the repositories here. It is also where the data-safety rules live (atomic batches, no partial writes, no silent data loss), which are what make a daily-use app trustworthy.

**Out of scope for this area (owned elsewhere):** schema DDL, migrations and views (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations); history and source repositories and the handler plumbing `AndroidMangaDatabaseHandler` / `AndroidAnimeDatabaseHandler` (area:06-data-repos-history-track-source-misc, which this area replaces with `kuro_core::data::db`); domain models, flag masks, sorters and recognition (area:07-domain-entries-items); category and library use cases (area:08-domain-library-category-history-updates).

## 2. Module map

### 2.1 Kotlin packages (observed)

| Package | Files in scope | Responsibility |
|---|---|---|
| `tachiyomi.data` | `DatabaseAdapter.kt` | `DateColumnAdapter`, `StringListColumnAdapter` (separator `", "`), `MangaUpdateStrategyColumnAdapter`, `AnimeUpdateStrategyColumnAdapter`, `FetchTypeColumnAdapter`, `MemoColumnAdapter` |
| `tachiyomi.data.category.manga` / `.anime` | `MangaCategoryRepositoryImpl.kt`, `AnimeCategoryRepositoryImpl.kt` | 14 operations each; identical apart from names |
| `tachiyomi.data.entries.manga` | `MangaMapper.kt`, `MangaRepositoryImpl.kt` | `mapManga` (24 cols), `mapLibraryManga` (24+7); 16 repository overrides |
| `tachiyomi.data.entries.anime` | `AnimeMapper.kt`, `AnimeRepositoryImpl.kt`, `AnimeRelationRepositoryImpl.kt` | five positional mappers (`mapAnime` 32, `mapLibraryAnime` 32+8, `mapSeasonAnime` 32+7, `mapRelatedAnime` 32+1, `mapDeletableAnime` 3); 21 overrides; 3 relation methods |
| `tachiyomi.data.items.chapter` | `ChapterRepositoryImpl.kt`, `ChapterSanitizer.kt` | 11 overrides; `mapChapter` (15 cols); sanitizer with a 30-entry trim array (29 distinct) |
| `tachiyomi.data.items.episode` | `EpisodeRepositoryImpl.kt`, `EpisodeSanitizer.kt` | 9 overrides; `mapEpisode` (20 cols); sanitizer identical to the chapter one |

Rule of thumb for the port: the Kotlin is one repository class per entity, one mapper per row shape, and SQL in `.sq` files. The Rust port keeps the same split but makes SQL plain `const` strings in the same file as the repository method.

### 2.2 Rust module map (proposal, `kuro-core/src/data/`)

```
db.rs            Db (writer + reader tokio-rusqlite connections), Notifier, TableSet, Subscription   <- handler plumbing
error.rs         DataError { NotFound, Sqlite, Json, Closed }
time.rs          now_secs(), now_millis(), start_of_local_day_millis()
sqlutil.rs       in_clause(n), chunks(500), lower_unicode() scalar function
adapters.rs      string list, strategies, FetchType, memo codecs                  <- DatabaseAdapter.kt
models.rs        Manga, Anime, Chapter, Episode, Category, *Update, Library*, SeasonAnime, DeletableAnime, AnimeRelationGroup
category.rs      CategoryRepo (EntryKind::Manga | Anime)                          <- category/*
manga/mapper.rs  map_manga, map_library_manga                                     <- MangaMapper.kt
manga/repo.rs    MangaRepo                                                        <- MangaRepositoryImpl.kt
manga/chapters.rs ChapterRepo                                                     <- ChapterRepositoryImpl.kt
anime/mapper.rs  five mappers                                                     <- AnimeMapper.kt
anime/repo.rs    AnimeRepo (incl. seasons, deletable parents)                     <- AnimeRepositoryImpl.kt
anime/episodes.rs EpisodeRepo                                                     <- EpisodeRepositoryImpl.kt
anime/relations.rs AnimeRelationRepo                                              <- AnimeRelationRepositoryImpl.kt
items/sanitize.rs sanitize_item_name(name, title) (one function for both)         <- ChapterSanitizer.kt, EpisodeSanitizer.kt
compat.rs        (data::compat) return-shape adapter for the interactor layer     <- behaviour in p1-behavior.md section 0
```

Crates: `rusqlite` (`bundled`, `functions`, `hooks`), `tokio-rusqlite` (one thread per connection, `call(|conn| ...)` closures), `tokio` (`sync`, `time`), `serde_json` (`preserve_order`), `thiserror`, `tracing`, `bitflags`, `time` (`local-offset`). No ORM, no query builder, no `r2d2`. Full table and reasons: `p1-rust-design.md` section 2.

**Ownership boundary:** the Rust structs in `models.rs` are the twins of the domain classes of area:07-domain-entries-items. Only the field sets are fixed here (`p1-data-and-config.md` section 2).

## 3. Reading guide

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Table of the 12 scope files: LOC, every top-level declaration, port decision, target module, priority, task IDs. Start here. |
| [p1-behavior.md](p1-behavior.md) | Method-by-method behaviour for manga, chapter, anime, episode, mappers and subscriptions; error policy per method group (section 0); cross-area interaction table (section 10). |
| [p1-data-and-config.md](p1-data-and-config.md) | Database files and pragmas, every domain row shape with defaults, enum stored values, value codecs, the exact `.sq` statements the repositories run, triggers, preference keys, flag bit layouts. The reference for exact SQL and constants. |
| [p1-topic-sql-semantics-and-triggers.md](p1-topic-sql-semantics-and-triggers.md) | Why `coalesce` updates, `is_syncing`, triggers (`last_modified_at`, `favorite_modified_at`, `version`), row order, `IN` lists, insert-and-return-id and change notification must be reproduced exactly. |
| [p1-topic-library-and-season-views.md](p1-topic-library-and-season-views.md) | `libraryView`, `animelibView`, `animeseasonsView`, `animedeletableView` and their stats helper views, with the quirks to preserve. |
| [p1-topic-seasons-and-relations.md](p1-topic-seasons-and-relations.md) | Anime parent/child seasons, `SyncSeasonsWithSource` writer contract, the clear-database algorithm, related-anime groups and their 7-day TTL. |
| [p1-topic-categories.md](p1-topic-categories.md) | Category repository contract, create/delete/reorder/flags interactor rules, the manga delete-cleanup bug, flag layout, proposed Rust `CategoryRepo` API. |
| [p1-topic-item-sanitizer.md](p1-topic-item-sanitizer.md) | The three-step chapter/episode name sanitizer, the exact trim set, and the test vector table. |
| [p1-rust-design.md](p1-rust-design.md) | Rust module layout, crate choices, signatures for `Db`, `Subscription`, every repository, row mapping by column name, threading model, GTK bridge, integration plan into the existing mockup, Rust pitfalls. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | EC-01 to EC-33 (observed behaviour and Rust requirement for each), the test plan grouped by task, fixtures. |
| [p1-tasks.md](p1-tasks.md) | 32 implementation tasks `T-05a-001` to `T-05a-032` with priority, effort, dependencies, sources and acceptance criteria. |

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the area (recommendation: keep)

| # | Decision | Why |
|---|---|---|
| D1 | Repositories are concrete structs, one implementation each; no traits. Tests use `Db::open_in_memory`. | Only one implementation exists; traits add nothing. |
| D2 | Keep the database file names `tachiyomi.db` and `tachiyomi.animedb`, the same schema and the same pragmas (`foreign_keys=ON`, `journal_mode=WAL`, `synchronous=NORMAL`). | An Aniyomi database can be dropped in for a one-off import; area:03/04 schema docs apply unchanged. |
| D3 | Static `coalesce` update statements, never a dynamic `SET` list. | The `AFTER UPDATE OF favorite` trigger must keep firing on every update (`p1-topic-sql-semantics-and-triggers.md` section 1). |
| D4 | Every write is one transaction on one writer connection; reads on a second reader connection. | Matches Aniyomi's `inTransaction = true` writes and WAL reads; no `SQLITE_BUSY` between writers (EC-26). |
| D5 | Subscriptions are `tokio::sync::watch` values fed by a debounced re-query; duplicates are not sent (`PartialEq`). | Fewer GTK list updates; the Kotlin `Flow` has no `distinct` but the UI benefits. |
| D6 | Repository methods return `Result<_, DataError>`; a `data::compat` adapter reproduces Kotlin's swallowed-error shapes only for the interactors that need them. | Failures stay diagnosable in Rust tests (`p1-behavior.md` section 0, T-05a-028). |
| D7 | Two deliberate SQL deviations: `ORDER BY sort, _id` for categories (deterministic tie-break, EC-10) and `insert` returns the new id. | Both are free and remove ambiguity. |
| D8 | Bug fixes that change behaviour: the manga category-delete cleanup (EC-11) and Unicode-correct duplicate lookup (EC-02). | Both are clear defects in the source; the user should confirm (see 4.2). |
| D9 | Drop Paging sources, coroutine transaction context, and the `is_syncing` sync feature (T-05a-027, T-05a-029). | No consumer; the user loses cross-device sync only (backups remain the transfer path). Columns stay for schema compatibility. |
| D10 | Timestamps keep their mixed units; Rust exposes `now_secs()` and `now_millis()` only (EC-25). | Backup and restore compare these values; changing units would break merge decisions. |

### 4.2 Open questions that need the user's call

| # | Question | Recommendation |
|---|---|---|
| Q1 | Should Kurobayomi read Aniyomi backups or databases exactly, including the lossy genre codec (`", "` split, EC-19)? | Yes, keep the codec exactly. Revisit only if area:21/22 decides to use a Kurobayomi-native backup format. |
| Q2 | Accept the two behaviour fixes D8 (manga category-delete cleanup, Unicode duplicate match)? | Yes. Both are defects; the change is invisible except for the fixed cases. |
| Q3 | Priority conflict: `T-05a-026` (P0, wire category tabs to the repository) asks for create, rename, hide, delete and drag reorder, but the logic for those lives in `T-05a-025` (P1). Promote `T-05a-025` to P0, or ship P0 with create and delete only? | Promote `T-05a-025` to P0. The category tabs are on the P0 path and otherwise cannot be edited. |
| Q4 | Expose anime `reset_viewer_flags` in the UI? It zeroes skip-intro and airing bits as well (EC-29). | No UI. Keep the repository method, test it as destructive. |
| Q5 | Clean up non-favorite related-anime rows automatically (EC-14)? | No automatic cleanup. The clear-database screen (area:47-settings-screens-general) already lists them. |
| Q6 | Support `(source, url)` duplicates in chapters and episodes? The schema has no unique index (EC-08). | Keep as is. Adding a UNIQUE index would make old backups fail to restore. |
| Q7 | Keep `busy_timeout = 5000` on both connections (EC-20)? It is not in Aniyomi. | Yes. It is harmless and protects against a second process. |

## 5. Cross-area dependencies

### 5.1 This area needs

| Area | What it provides | Tasks that depend on it |
|---|---|---|
| area:01-foundation-app-bootstrap | `Paths` trait (data directory), app context for `Arc<Db>`, single-instance guard, the error dialog for EC-32 | T-05a-001 |
| area:02-preferences-system | Category-related preference keys (`library_sorting_mode`, `default_category`, `library_update_categories*`, `categorized_display`, `library_random_manga_sort_seed`) | T-05a-025 |
| area:03-db-manga-schema-migrations | `mangas`, `chapters`, `categories`, `mangas_categories`, `excluded_scanlators`, `libraryView`, triggers, `migrate(&mut Connection)`, `user_version` handling | T-05a-001, T-05a-005, T-05a-008, T-05a-023, T-05a-024 |
| area:04-db-anime-schema-migrations | `animes`, `episodes`, `animes_categories`, `anime_relations`, the anime views and triggers | T-05a-001, T-05a-012 to T-05a-015, T-05a-017, T-05a-018 |
| area:06-data-repos-history-track-source-misc | `history` and `animehistory` tables (joined by the library views); the `Db` replacement of the handler; source-related `all_source_and_url` consumers | T-05a-002, T-05a-007, T-05a-014, T-05a-031 |
| area:07-domain-entries-items | Domain models (`Manga`, `Anime`, `Chapter`, `Episode`, `Category` field sets and `create()` defaults), flag masks, `SeasonDisplayMode` | T-05a-004, T-05a-011, T-05a-025 |
| area:08-domain-library-category-history-updates | Category and library use cases that call this area; the category flag-arithmetic test vectors (`LibraryFlagsTest.kt`) | T-05a-010, T-05a-025 |
| area:09-domain-source-track-download-release-misc | `DownloadPreferences` category keys used by the delete cleanup | T-05a-025 |
| area:10-app-domain-layer | Sync interactors (`SyncChaptersWithSource`, `SyncEpisodesWithSource`, `SyncSeasonsWithSource`, `SyncRelatedAnimeWithSource`) that call the `add_all`, `remove_parent_ids` and relation methods; they choose the return-shape adapter | T-05a-028, T-05a-031 |
| area:11-app-migrations | The codec and enum-ordinal assumptions that Aniyomi's app migrations rely on | T-05a-003 |

### 5.2 Other areas need from this area

| Area | What it needs |
|---|---|
| area:21-backup-format-and-create | `getFavorites`, `getReadMangaNotInLibrary` (`read = 1 OR last_page_read != 0`), `all_source_and_url`, exported `version`, `last_modified_at`, `favorite_modified_at`, memo JSON |
| area:22-backup-restore | `insert`/`update_all`, `by_url_and_source`, `by_url`, `delete_non_library_*` for restore cleanup, `add_all` returning ids |
| area:37-ui-library | `library()` and `subscribe_library()` (one row per entry and category link, EC-13), `library_by_category` helper |
| area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | `subscribe_by_id`, chapter and episode subscriptions with the scanlator filter, category subscriptions, `set_categories` |
| area:42-ui-updates-history-stats-storage-upcoming | `subscribe_upcoming(statuses)` with local start-of-day cut-off (manga {1, 4}; anime {1, 4, 7}) |
| area:43-reader-core | Manga `viewer` flags stored as opaque `i64`; `reset_viewer_flags` semantics |
| area:47-settings-screens-general | Clear-database flow (`subscribe_deletable_parents`, `children_by_parent`, `remove_parent_ids`, `delete_non_library_by_ids`), category management screens |

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows whose `file` starts with `05-data-repos-entries-items/` in `plans/TASKS.csv`. `tasks.py lint p1-tasks.md` prints `ok`.

**Counts (32 tasks):**

| Priority | Tasks | S | M | L | XL |
|---|---|---|---|---|---|
| P0 | 20 | 4 | 14 | 2 | 0 |
| P1 | 9 | 6 | 3 | 0 | 0 |
| P2 | 1 | 0 | 1 | 0 | 0 |
| P3 | 2 | 2 | 0 | 0 | 0 |
| **Total** | **32** | **12** | **18** | **2** | **0** |

P0 tasks: T-05a-001, 002, 003, 004, 005, 006, 007, 008, 009, 010, 011, 012, 013, 014, 015, 016, 022, 023, 026, 030.

**P0 critical path (in order):**

1. T-05a-030 (time helpers, S) and T-05a-003 (column codecs, S) and T-05a-009 (sanitizer, S): no dependencies, start immediately.
2. T-05a-001 (database open, pragmas, connections, M): needs area:01, area:03, area:04. Start the T-05a-023 harness (M) at the same time.
3. T-05a-002 (table-change notifier and `Subscription`, L): needs T-05a-001. This is the largest risk item (see 6.2).
4. T-05a-004 (manga row structs and mapper, M) then T-05a-005 (manga reads, M) and T-05a-006 (manga writes, M).
5. T-05a-007 (library view and grouping, M): needs T-05a-002 and T-05a-005.
6. T-05a-010 (category repository, M) in parallel with step 4.
7. T-05a-011 (anime structs and mappers, M), T-05a-012 and T-05a-013 (anime reads and writes, M), T-05a-014 (anime library and season stats, M).
8. T-05a-008 (chapters, M), T-05a-015 (episodes, M), T-05a-016 (episode sanitizer entry point, S).
9. T-05a-022 (GTK bridge, M): needs T-05a-002.
10. T-05a-026 (wire the mockup, L): needs T-05a-007, 008, 010, 014, 015, 022. If Q3 is accepted it also needs T-05a-025 in P0.

Serial spine at effort ceilings (T-05a-001 M, 002 L, 005 M, 007 M, 026 L; M = 3 days, L = 7.5 days): about 24 working days, roughly 5 weeks for one developer. Steps 4 to 8 can overlap and shorten this; the estimate is an upper bound, not a plan.

**P1 (after the critical path):** T-05a-017 (seasons), T-05a-019 (Unicode duplicates), T-05a-020 (upcoming), T-05a-021 (excluded scanlators), T-05a-024 (trigger regression), T-05a-025 (category rules), T-05a-028 (compat adapter), T-05a-031 (restore and clear-database helpers), T-05a-032 (real Aniyomi database fixture test).

**P2:** T-05a-018 (related-anime relations).

**P3:** T-05a-027 (drop Paging and transaction machinery), T-05a-029 (drop `is_syncing` sync feature). Both are decision records with no code.

### 6.1 Corrections made to the area's task file during this pass

- `T-05a-013` said "28-parameter" coalesce statement; the Kotlin `animes.sq` update has 29 `coalesce` values plus `:animeId` (29 `coalesce` lines counted in `animes.sq`). Changed to "29-column coalesce statement (29 coalesce values plus :animeId)".
- Short `area:NN` tokens (`area:01`, `area:03/04`, `area:06`, `area:07`, `area:08`, `area:10`, `area:37`, `area:38`) across the folder were replaced by full ids from `plans/_tools/areas.py` output. No task IDs were changed.

### 6.2 Risks

| Risk | Where | Mitigation |
|---|---|---|
| Change notification misses writes made by triggers or foreign-key cascades (SQLite hook coverage is not verified for every case) | T-05a-002, EC-24 | Spike the `update_hook`/`commit_hook` behaviour first; keep the per-method superset publish as a fallback; test that deleting a manga re-emits chapter lists. |
| `tokio-rusqlite` futures awaited on the GLib main context (`glib::spawn_future_local`) are not verified | T-05a-022 | A 20-line spike before T-05a-002 starts. If it fails, drive the connection futures from a small tokio runtime and post results to the GTK thread. |
| `time` crate `local-offset` can fail on Linux in multi-threaded processes | T-05a-030 | Compute the offset once at startup on the main thread and cache it (already in the task). |
| Library view re-aggregates all chapters on each change; 500 000 chapters may be slow | T-05a-007 | The 100 ms debounce plus the benchmark in T-05a-007 (500 ms budget). Only materialise if the benchmark fails. |
| Positional row mapping drifts when a column is added in area:03/04 | T-05a-004, T-05a-011 | Map by column name (`Statement::column_names()`), and count-tests (`libraryView` 31, `animelibView` 40, `animeseasonsView` 39, related query 33). |
| Behaviour differences from Aniyomi (D8) | T-05a-019, T-05a-025 | List them in the user-facing changelog; the tests in `p1-edge-cases-and-tests.md` cover them. |
| Genre codec is lossy by construction | T-05a-003 | Keep (Q1). Documented in `p1-data-and-config.md` section 4. |
| Data written by Kurobayomi must still open in Aniyomi | D2, T-05a-032 | Round-trip test: update one row, all other columns unchanged. |

## 7. Definition of done for this area

Observable in Kurobayomi on a daily-use Linux desktop:

1. **Fresh start:** the first launch creates `tachiyomi.db` and `tachiyomi.animedb` in the data directory, WAL mode, with the system category row (id 0, name "") present. Both databases open with `foreign_keys`, `journal_mode=wal`, `synchronous=1`, `recursive_triggers=0`, `busy_timeout=5000`.
2. **Aniyomi import check:** the fixture databases produced by the real Aniyomi app (`tests/fixtures/aniyomi_manga.db`, `aniyomi_anime.db`) open with no migration error, and the library shows the expected counts (3 manga, 3 anime including a Seasons parent with 2 children).
3. **Library:** every favorite manga and anime appears in the category tabs. An entry in three categories appears under all three. An entry with no category appears under Default. Unread and bookmark counts ignore chapters from excluded scanlators.
4. **Entry details:** the chapter or episode list loads without freezing the window. Sanitized names are shown (a chapter named "One Piece 1000" under title "One Piece" displays as "1000"). The scanlator filter changes the list immediately.
5. **Categories:** create, rename, hide, delete and drag reorder persist across restart. Creating a category gives it the next order and the current global sort. Deleting a category renumbers the rest and removes its id from the update and download category settings (both manga and anime). The duplicate-name message `A category with this name already exists!` appears for exact duplicates.
6. **Set categories:** assigning an entry to categories replaces its links in one step. A failed assignment leaves the old links in place.
7. **Atomic writes:** a failed chapter batch leaves the chapter list unchanged. Bulk "mark read" on 200 chapters refreshes the library once.
8. **Live updates:** after any write, the visible list refreshes within about 150 ms (debounce plus query), with the scroll position kept.
9. **Upcoming:** the upcoming list uses the local start of today and re-computes after midnight without a restart.
10. **Seasons:** a Seasons parent shows child counts and seen counts aggregated over its children; a child with zero episodes counts as fully seen (EC-15).
11. **Clear database:** the anime clear-database screen lists only non-favorite top-level entries as deletable, keeps favorite children by nulling their `parent_id`, and never deletes a favorite.
12. **No raw SQL in the GTK crate:** `kurobayomi/src/*` contains no `rusqlite` import. All access goes through `kuro_core::data` repositories and `data_bridge::bind`.
13. **Tests:** `cargo test -p kuro-core` runs the whole data suite headless in under 10 seconds; every public repository method has at least one test.
