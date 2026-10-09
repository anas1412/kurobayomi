# Area 04: Anime database (schema, views, queries, migrations) — overview

Area id: `04-db-anime-schema-migrations`. Source scope: `data/src/main/sqldelightanime/` (44 files, 1869 LOC) in the Aniyomi clone at `/home/blackbox/aniyomi-src`. All repo-relative paths below are relative to that clone. "OBSERVED" means what the Kotlin/SQL does; "Proposal:" means what Kurobayomi does.

## 1. Purpose and scope

**What Aniyomi does here.** One SQLite file, `tachiyomi.animedb` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:106`), holds everything the anime side of the app remembers:

- library entries and their metadata (`animes`, 32 columns),
- episodes with watch progress in whole seconds (`episodes`, 20 columns),
- one history row per episode (`animehistory`), read through `animehistoryView`,
- library categories and the anime-to-category links (`categories`, `animes_categories`),
- tracker bindings (`anime_sync`), stub sources (`animesources`), related-anime groups (`anime_relations`),
- user-defined player buttons whose bodies are mpv Lua (`custom_buttons`),
- extension repositories ("stores", `extension_store`).

Eight views (`data/src/main/sqldelightanime/view/*.sq`) compute the library rows, season aggregates, history rows, the updates feed, and the "clear database" candidates. Triggers maintain `version`, `is_syncing` and the modification stamps that backup restore uses to decide which copy wins.

**Why it matters for a usable Kurobayomi.** Every anime screen that the mockup currently fills with literals reads or writes this database: library (`src/screens.rs:154`), history (`src/screens.rs:305`), updates (`src/screens.rs:277`), categories (`src/screens.rs:404`), entry details (`src/entry.rs:81`), clear-database (`src/settings.rs:223`). Player progress and history writes (area:45-player-core) land here. Without this area there is no persistence and no progress, so the daily reading/watching loop cannot work.

**In scope.** 10 tables, 9 named indexes, 7 triggers, 8 views, 78 named statements in `.sq` files, 26 migration scripts `113.sqm` to `138.sqm` (schema version 139), the value codecs and bit-flag layouts stored in these columns, and the SQL-level semantics that the repositories depend on.

**Out of scope (owned elsewhere).** The manga database (area:03-db-manga-schema-migrations), the domain interactors that compute the stored values such as fetch intervals and flag setters (area:07-domain-entries-items, area:08-domain-library-category-history-updates), the backup file format and restore logic (area:21-backup-format-and-create, area:22-backup-restore), tracker semantics of `anime_sync.status` (area:27-tracking-core), and all UI. No file in this scope renders UI (see [p1-inventory.md](p1-inventory.md)).

**Spot-check done for this overview.** `AppModule.kt:104-123` (file name, pragmas `foreign_keys`, `journal_mode = WAL`, `synchronous = NORMAL`) and the `animes.sq` statement positions (`getAnimeById` at line 75 through `selectLastInsertedRowId` at line 203) match the docs. The migration count (26) and the `.sq` file count (10 tables, 8 views) match the directory listing.

## 2. Module map

### 2.1 Kotlin (Aniyomi) packages to responsibility

| Kotlin location (repo-relative) | Responsibility | Owner area |
|---|---|---|
| `data/src/main/sqldelightanime/dataanime/*.sq` (10 files) | DDL for 10 tables, named indexes, triggers, seed rows, named statements | area:04 |
| `data/src/main/sqldelightanime/view/*.sq` (8 files) | 8 view definitions and the statements that read them | area:04 |
| `data/src/main/sqldelightanime/migrations/113.sqm` to `138.sqm` (26 files) | Upgrade chain from user_version 113 to 139 | area:04 |
| `data/src/main/java/tachiyomi/data/handlers/anime/` | `AnimeDatabaseHandler`: `await`, `awaitList/One/OneOrNull`, `subscribeTo*`, transaction dispatch | area:04 (T-04a-007, T-04a-008) |
| `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt` | Column codecs: genre list, update strategy, fetch type, memo, dates | area:04 (T-04a-005) |
| `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt` | Database construction, file name, pragmas, driver choice | area:04 (file facts) and area:01-foundation-app-bootstrap (runtime wiring) |
| `data/src/main/java/tachiyomi/data/entries/anime/` (`AnimeRepositoryImpl`, `AnimeMapper`, `AnimeRelationRepositoryImpl`) | Entry repository, library mapping, relations | area:05-data-repos-entries-items (Rust repository), SQL facade here |
| `data/src/main/java/tachiyomi/data/items/episode/` (`EpisodeRepositoryImpl`, `EpisodeSanitizer`) | Episode repository, name sanitizer | area:05-data-repos-entries-items |
| `data/src/main/java/tachiyomi/data/history/anime/`, `.../category/anime/`, `.../track/anime/`, `.../updates/anime/`, `.../source/anime/`, `.../custombutton/` | Repositories for history, categories, tracks, updates, stub sources, custom buttons | area:06-data-repos-history-track-source-misc |
| `data/src/main/java/mihon/data/extension/anime/repository/` | Extension store repository | area:06 (storage), area:19-extensions-anime (fetch) |
| `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt` and `.../library/anime/model/AnimeLibrarySortMode.kt` | Flag layouts and sort encoding (the numbers stored here) | area:07-domain-entries-items, area:08-domain-library-category-history-updates |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/` | Restore writes that touch these tables directly | area:22-backup-restore |

### 2.2 Proposed Rust crate and module map

Defined in [p1-rust-design.md](p1-rust-design.md) section 1.

```text
crates/
  kuro-db/                 no GTK dependency
    sql/anime/schema.sql   10 tables, 9 indexes, 7 triggers, 2 seeds        (T-04a-003)
    sql/anime/views.sql    8 views in dependency order                        (T-04a-004)
    sql/anime/queries/*.sql named statements, `-- name:` blocks
    sql/anime/legacy/113.sql .. 138.sql                                       (T-04a-025, P2)
    sql/anime/native/139.sql Kurobayomi-only step 139 -> 140 (indexes)        (T-04a-036, P2)
    src/core/              SHARED with area:03: conn, pool, bus, error, migrate, sqlfn
    src/anime/mod.rs       AnimeDb facade (async), consts (file name, schema version 139)
    src/anime/codec.rs     Millis, StringList, UpdateStrategy, FetchType, Memo
    src/anime/flags.rs     episode/season/viewer/category flag constants
    src/anime/model.rs     AnimeRow, EpisodeRow, LibraryAnimeRow, SeasonAnimeRow, HistoryRow, ...
    src/anime/sql/         entries, episodes, history, categories, tracks, updates,
                           sources, relations, custom_buttons, extension_store, maintenance
    src/anime/legacy.rs    runner for copied Aniyomi files
  kuro-testkit/            TestDb::anime(), builders (shared with other areas)
src/                       existing GTK crate; consumes kuro_db::anime::AnimeDb
```

Crate dependencies: `rusqlite` (bundled, `hooks`, `functions`, `backup`), `r2d2` and `r2d2_sqlite`, `tokio`, `tokio-stream`, `serde_json` (`preserve_order`), `thiserror`, `bitflags`, `async-channel`, `tracing`, `chrono`. No ORM and no query builder: the SQL text is the contract.

## 3. Reading guide

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | File-by-file table of all 44 scope files with port decision, target module, priority and task ids. |
| [p1-data-and-config.md](p1-data-and-config.md) | Column-level data model of the fresh schema, value conventions, trigger SQL, every bit-flag layout, preference keys, backup field map, file layout. |
| [p1-behavior.md](p1-behavior.md) | Statement-by-statement semantics, transaction and dispatch rules, interactor algorithms that decide stored values, feeds, clear-database flow, restore interplay, error paths. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | 38 edge cases with decisions, upstream tests to port, numeric view fixtures V1 to V7, trigger and repository test lists, legacy-chain expectations, test kit. |
| [p1-rust-design.md](p1-rust-design.md) | Crate layout, dependencies, core types (`Db`, `DbError`), change bus, codecs, row models, SQL function signatures, schema creation and upgrade flow, threading model, Android-to-Linux mapping, pitfalls. |
| [p1-tasks.md](p1-tasks.md) | The 36 implementation tasks T-04a-001 to T-04a-036 with priority, effort, dependencies, sources and acceptance criteria. |
| [p1-topic-fresh-vs-upgraded-schema.md](p1-topic-fresh-vs-upgraded-schema.md) | The ten measured differences between a fresh database and one upgraded through the migration chain, and the Rust rules they imply. |
| [p1-topic-legacy-migration-chain.md](p1-topic-legacy-migration-chain.md) | What each of the 26 migration scripts does to data, the reconstructed v113 baseline, and the import runner design. |
| [p1-topic-reactive-queries.md](p1-topic-reactive-queries.md) | How SQLDelight `asFlow()` maps to the Rust commit-hook bus, the per-stream table dependency sets, and the subscription tests. |
| [p1-topic-triggers-and-versioning.md](p1-topic-triggers-and-versioning.md) | The seven triggers, the measured effect table on `version`, `last_modified_at` and `favorite_modified_at`, and the `is_syncing` lifecycle. |
| [p1-topic-views-and-aggregates.md](p1-topic-views-and-aggregates.md) | Verbatim text of the view definitions, the column contract of each view, the aggregate rules, and the query-plan facts. |

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the area documents

| # | Decision | Why | Recommendation |
|---|---|---|---|
| D1 | The fresh Kurobayomi schema is the Aniyomi 139 shape (same table and column names and order, same trigger bodies, same version semantics). Kurobayomi adds only native step 139 to 140 (two category-link indexes, T-04a-036). | Backup `version` and `last_modified_at` are compared across devices; the golden dump and the legacy import stay meaningful. | Keep. |
| D2 | SQL stays verbatim in `.sql` files; no ORM. Rows are mapped by column name, never by position. | Positional `SELECT *` breaks when a migration adds a column (see [p1-topic-fresh-vs-upgraded-schema.md](p1-topic-fresh-vs-upgraded-schema.md) section 3). | Keep. |
| D3 | One writer connection behind `Db::write(\|tx\| ...)` (`BEGIN IMMEDIATE`), a pool of 4 read-only connections, WAL, `busy_timeout = 5000`. Nested transactions are impossible by construction. | Replaces the Kotlin inherited-transaction context machinery. | Keep. |
| D4 | Reactive streams are driven by a table-level commit hook (`update_hook` collects table names, `commit_hook` publishes, 50 ms coalescing, distinct-until-changed). | Mirrors SQLDelight's per-table listener registry with one mechanism. | Keep, with the correction in 6.3 below. |
| D5 | Intentional deviations from Aniyomi: `kuro_lower()` for non-ASCII title matching (T-04a-022); escaped `LIKE` input in history search; `IN` lists chunked at 500; `_id ASC` tie-break in category queries; tolerant memo read (empty object plus a warning, no crash); clear-database statements in one transaction; `is_syncing` reset at startup (T-04a-024); a `quick_check` and quarantine on open (T-04a-031). | Each removes a documented Aniyomi bug or crash path. | Keep all; each is a separate task. |
| D6 | Dropped: Paging 3 source (T-04a-033), Android driver and SQLDelight build features (T-04a-034), `getAllAnime` and `getAnimesWithFavoriteTimestamp` which have no caller (T-04a-035). | No caller or Android-only. | Keep. The user loses nothing. |
| D7 | No unique index on `(source, url)` is added. Duplicates resolve to the lowest `_id`. | A damaged restore would start failing with a unique index. | Keep. |

### 4.2 Open questions that need the user's call

| # | Question | Recommendation |
|---|---|---|
| Q1 | Should Kurobayomi import a raw Aniyomi `tachiyomi.animedb` file at all (T-04a-025, T-04a-026, P2), or is backup restore enough? The chain was tested only on a reconstructed v113 baseline because no real v113 file was available. | Defer. Make backup restore the only import path for v1. Keep T-04a-025 at P2 and treat its tests as proof of the reconstruction only. |
| Q2 | The test plan says CI also compiles for `x86_64-pc-windows-gnu` ([p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 8). The target is Linux (CachyOS, KDE). | Drop the Windows check unless `addenda/requirements-cross-platform.md` makes Windows a requirement. Confirm before implementing. |
| Q3 | Genre is stored as text joined with `", "`, so a genre that contains `", "` splits on read. | Keep for backup-interchange compatibility. Accept the loss and document it. |
| Q4 | `memo` (opaque JSON per entry and per episode) has no Kurobayomi consumer until area:20-extension-strategy-redesign decides what extensions store. | Keep the columns. They are cheap and backups carry them. |
| Q5 | `version` and `is_syncing` exist for restore conflict resolution. Kurobayomi has no sync service. | Keep them. Backup restore (area:22-backup-restore) needs them. |
| Q6 | Clear-database deletes all reset history rows (`DELETE FROM animehistory WHERE last_seen = 0`), including rows of library entries. Those rows are invisible to the UI. | Keep Aniyomi's semantics. Wrap the three statements in one transaction (T-04a-021). |
| Q7 | Repository ownership overlaps: tasks T-04a-009, T-04a-011, T-04a-013, T-04a-014, T-04a-016 to T-04a-019 specify Rust repository functions that area:05 and area:06 also cover. | Area 04 owns the SQL, the codecs and the `kuro_db::anime::sql` functions. Areas 05 and 06 own the typed facade calls. Confirm with those area leads. |
| Q8 | Data location `<data_dir>/anime.db` with `data_dir` from `kuro-platform::Paths` (Linux: `$XDG_DATA_HOME/kurobayomi`). | Confirm with area:01-foundation-app-bootstrap. |
| Q9 | Quarantine dialog on a failed `quick_check` (T-04a-031) is a Proposal beyond Aniyomi parity. | Keep at P1; it is small and protects the only copy of the user's library. |

### 4.3 Corrections found while reviewing the area documents

1. **Truncate optimisation and history notifications.** [p1-rust-design.md](p1-rust-design.md) section 4 (pitfall 2) argues that `DELETE FROM animehistory` (no `WHERE`) notifies through the update hook because foreign keys are on. Under SQLite's documented truncate-optimisation rules, a table with no triggers and no foreign-key parent references is truncated, and the update hook is not called. `animehistory` has no triggers and no child references, so the rationale is wrong. Fix: `removeAllHistory` must call `tx.touch(TableSet::ANIMEHISTORY)` explicitly. Do not rely on the hook. This was not re-verified in this run; the test in T-04a-007 decides it.
2. **Bus element type.** [p1-rust-design.md](p1-rust-design.md) section 3 uses `broadcast::Sender<TableSet>`; [p1-topic-reactive-queries.md](p1-topic-reactive-queries.md) section 2 uses `Sender<Arc<TableSet>>`. `TableSet` is `Copy`, so use the plain value.

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Used by tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | `Paths::data_dir()`, the shared tokio runtime, app context (`ctx.anime_db`), shutdown hook, `glib` bridge `ui::db_bridge::spawn_stream` | T-04a-001, T-04a-028, T-04a-032 |
| area:02-preferences-system | Keys `default_anime_category`, `animelib_update_categories`, `animelib_update_categories_exclude`, `remove_exclude_anime_categories`, `download_new_anime_categories`, `download_new_anime_categories_exclude`, `animelib_sorting_mode`, `categorized_display`, `library_random_anime_sort_seed`, `last_used_anime_category` | T-04a-013, T-04a-021 |
| area:03-db-manga-schema-migrations | Shared `kuro-db::core` (connection factory, writer/reader split, bus, migration runner). Whichever area lands first owns it. | T-04a-001, T-04a-002, T-04a-007, T-04a-008 |
| area:07-domain-entries-items | `AnimeFetchInterval` algorithm and its 10 tests, flag setters `SetAnimeEpisodeFlags` and `SetAnimeSeasonFlags`, episode defaults | T-04a-006, T-04a-009 |
| area:08-domain-library-category-history-updates | Category interactors (create, delete re-index, reorder, sort flags) that decide the numbers written | T-04a-013 |
| area:16-source-api-anime | `SAnime.status` ints, `FetchType` and `AnimeUpdateStrategy` enum definitions, the source-id scheme (`animes.source`) | T-04a-005, T-04a-009 |
| area:19-extensions-anime and area:20-extension-strategy-redesign | The extension store model and the fetch logic that fills `extension_store` | T-04a-019 |
| area:22-backup-restore | Restorers call the raw queries of T-04a-023 and rely on the version rule | T-04a-023 |
| area:27-tracking-core | Meaning of `anime_sync.status` and `sync_id` | T-04a-014 |
| area:48-settings-player-and-scripts and area:45-player-core | The mpv Lua dialect of `custom_buttons`; the player writes progress and history | T-04a-018, T-04a-012 |

### 5.2 What others need from this area

| Area | What they consume |
|---|---|
| area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc | The `kuro_db::anime::sql` functions and `AnimeDb` facade, plus the row models of `p1-rust-design.md` section 6 |
| area:07-domain-entries-items, area:08-domain-library-category-history-updates | Row types and the flag and codec constants of T-04a-005 and T-04a-006 |
| area:21-backup-format-and-create, area:22-backup-restore | Direct table access rules (sections 8, 11, 14 of [p1-behavior.md](p1-behavior.md)) and the restore API of T-04a-023 |
| area:37-ui-library, area:39-ui-entry-details-anime, area:41-ui-browse-anime, area:42-ui-updates-history-stats-storage-upcoming | Streams: `library_stream`, `history_stream`, `updates_stream`, `seasons_stream`, `episodes_by_anime_stream`, `related_stream`, `categories_stream`, `custom_buttons_stream`, `extension_store_*` |
| area:45-player-core, area:46-player-controls-ui | Progress and history write functions (`UpdateEpisode`, `upsert_history`) and the custom button expansion |
| area:20-extension-strategy-redesign | `extension_store` rows |

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows of `TASKS.csv` whose file starts with `04-db-anime-schema-migrations/`.

### 6.1 Counts

- **36 tasks** (T-04a-001 to T-04a-036; no ids changed).
- Priority: **P0 = 14**, **P1 = 11**, **P2 = 8**, **P3 = 3**.
- Effort: **S = 18**, **M = 17**, **L = 1** (T-04a-025), **XL = 0**.
- Per priority and effort: P0 M 11, P0 S 3, P1 M 3, P1 S 8, P2 M 3, P2 S 4, P2 L 1, P3 S 3.
- `p1-tasks.md` lints clean (`tasks.py lint` prints `ok`).

### 6.2 P0 critical path

Engineer-days on the longest chain: 7 M-tasks, so roughly 7 to 21 days (M is up to 3 days).

1. T-04a-001: kuro-db crate, connection factory, pragmas, file location.
2. T-04a-003: embed the fresh schema (10 tables, 9 indexes, 7 triggers, 2 seeds).
3. T-04a-004: embed the eight views.
4. T-04a-005 and T-04a-008 (parallel with step 2 once step 1 is done): codecs and async handler. T-04a-006 (flag constants) and T-04a-007 (change bus) run in parallel as well.
5. T-04a-002: schema version guard (P0, S, needed before T-04a-028).
6. T-04a-009: anime row model and entry repository.
7. T-04a-011: episode row model and repository.
8. T-04a-010: library read model (needs 004, 009, 007), in parallel with step 7.
9. T-04a-012: history repository and view (needs 004, 011).
10. T-04a-027: test kit and golden dumps (needs 003, 004); can run in parallel with steps 6 to 9.
11. T-04a-028: wire the database into the app context and the mock screens (needs 001, 002, 010, 012). Last.

T-04a-024 (startup reset of stuck `is_syncing` flags) is P1, so it is off the P0 path. It should still land before the first daily-use release, because a stuck flag silently suppresses version bumps.

### 6.3 Risks

1. **Version and trigger exactness.** Backup compatibility depends on `version`, `is_syncing` and the stamps matching Aniyomi trigger behaviour. Any drift from the measured table in [p1-topic-triggers-and-versioning.md](p1-topic-triggers-and-versioning.md) section 3 is a silent restore bug. Mitigation: T-04a-027 tests and the golden dump.
2. **Hook-based notifications.** The truncate case in 4.3 item 1 can drop UI refreshes if not handled. Mitigation: explicit `touch` plus a test.
3. **Overlap with area:05 and area:06.** Duplicate repository work is likely (Q7). Mitigation: decide ownership before T-04a-009 starts.
4. **Query cost on the library.** `animelibView` re-runs on every write to the four tables it reads, including progress writes during playback. Mitigation: 50 ms coalescing and the library screen not subscribed during playback (area:37-ui-library); T-04a-036 indexes if needed.
5. **Legacy chain confidence.** The v113 baseline is reconstructed, not taken from a real file. Mitigation: Q1 deferral to P2 and a real Aniyomi file as a fixture when one is available.
6. **Lossy encodings.** Genre split on `", "` and memo tolerant reads are deliberate losses; a user who edits genres with commas will see them split. Mitigation: documented in T-04a-005 acceptance.
7. **Bundled SQLite version.** Behaviour is pinned by tests, not version numbers ([p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) header). A different SQLite build could change the truncate behaviour noted in 4.3.
8. **Windows target.** Q2. If Windows stays in the CI matrix, cost rises for no user value.

## 7. Definition of done for this area in Kurobayomi

Observable behaviours, each checkable from the running app or from `cargo test -p kuro-db`:

1. A fresh start with no `anime.db` creates `<data_dir>/anime.db` at `user_version = 139` with the 10 tables, 9 indexes, 7 triggers, 8 views, and the seed rows `categories(_id=0)` and `custom_buttons(_id=1, name='+85 s')`. The golden dump matches `crates/kuro-db/tests/golden/anime_schema_v139.txt`.
2. Opening a file with a newer `user_version` refuses it with "database is newer than this app" and leaves the file byte-identical.
3. Adding an anime to the library, then restarting, keeps it. The library view shows `totalCount`, `seenCount` and `category` per category membership, and an entry in two categories appears twice.
4. Marking an episode seen, or changing its progress, updates `episodes.version` and `animes.version` exactly as in the trigger table of section 3 of [p1-topic-triggers-and-versioning.md](p1-topic-triggers-and-versioning.md). A running library stream updates within about 50 ms.
5. History shows one row per anime (its most recently watched episode, `seenAt > 0`), newest first. Resetting the newest row shows the next most recent row. Searching `%` returns nothing unless a title contains `%`. A search for `éclair` finds `Éclair`.
6. The updates feed shows only episodes of library entries fetched after the entry was added, newest fetch first, capped at 500.
7. Category create, rename, hide, reorder and delete behave as in [p1-behavior.md](p1-behavior.md) section 7. Deleting the system category shows the internal-error text. Deleting a category removes it from the five preference sets.
8. Clear anime database lists non-library parents grouped by source, and removes them with their children, episodes, history, tracks and relations. Library entries are untouched. Favorite children of deleted seasons survive with `parent_id` cleared.
9. Custom player buttons: exactly one primary button at any time; `$id` and `$isPrimary` are expanded in content, long-press and startup code.
10. Tracker bindings: re-binding the same tracker for one entry replaces the row (and the database assigns a new row id); the UI keys by `(anime_id, sync_id)`.
11. A crash in the middle of a restore or a category write leaves no row with `is_syncing = 1` after restart.
12. A copied Aniyomi `tachiyomi.animedb` at user_version 113 to 138 imports to 139 and then 140, with the data checks of [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 7. This is P2 and gated by Q1.
13. Opening a database with a failed `PRAGMA quick_check` quarantines the file as `anime.db.corrupt-<unix seconds>` and creates a new empty database after showing the blocking dialog (T-04a-031).
14. `cargo test -p kuro-db` passes every test listed in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) sections 2 to 6 on Linux.
