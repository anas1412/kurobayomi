# Area 03 / part 1 — Implementation tasks: manga database schema, statements, migrations

Companion documents (same folder): `p1-inventory.md`, `p1-behavior.md`, `p1-data-and-config.md`, `p1-rust-design.md`, `p1-edge-cases-and-tests.md`, and the topic files `p1-topic-baseline-schema.md`, `p1-topic-migration-history.md`, `p1-topic-query-catalog.md`, `p1-topic-triggers-and-versioning.md`, `p1-topic-reactive-runtime.md`. Test ids of the form `TC-<letter><number>` are defined in `p1-edge-cases-and-tests.md` section 5.

### T-03a-001 — kuro-db crate skeleton and dependencies
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: data/build.gradle.kts, data/src/main/AndroidManifest.xml
- Acceptance:
  - `crates/kuro-db` exists as a workspace member with the modules listed in `p1-rust-design.md` section 1 (`error`, `open`, `schema`, `migrate`, `codec`, `flags`, `models`, `notify`, `runtime`, `sql::*`) and `cargo check -p kuro-db` passes on stable Rust
  - dependencies are `rusqlite 0.40` with `bundled`, `hooks`, `functions`, `backup`, plus `thiserror 2`, `serde_json 1`, `tokio 1`; `rusqlite::version()` reports 3.53 or newer in a smoke test
  - the crate contains no `cfg(target_os)`, no `directories`/`xdg` use, and receives file paths only as arguments (cross-platform rule 1)
  - `cargo check --target x86_64-pc-windows-gnu -p kuro-db` passes (cross-platform rule 7)

### T-03a-002 — Connection open contract (pragmas, busy timeout, version gate)
- Priority: P0
- Effort: M
- Depends: T-03a-001, T-03a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, data/build.gradle.kts, gradle/libs.versions.toml
- Acceptance:
  - `open_connection` sets `foreign_keys = ON`, `journal_mode = WAL` (through `pragma_update_and_check`, result `wal`), `synchronous = NORMAL`, busy timeout 5000 ms, and leaves `recursive_triggers` at 0 (TC-O07)
  - every connection in the pool (writer and readers) has `foreign_keys = 1`
  - `open_and_migrate` implements the decision table of `p1-rust-design.md` section 4: no tables creates the baseline; tables with `application_id != 0x4B555230` return `LegacyFile` (TC-O03); `user_version` above the latest returns `TooNew` (TC-O04); below the latest runs migrations (TC-O05)
  - database file name `manga.db` comes from the `Paths` trait, never from a string literal path

### T-03a-003 — Baseline schema v33 SQL and creation
- Priority: P0
- Effort: M
- Depends: T-03a-001
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelight/data/categories.sq, data/src/main/sqldelight/data/mangas_categories.sq, data/src/main/sqldelight/data/history.sq, data/src/main/sqldelight/data/manga_sync.sq, data/src/main/sqldelight/data/excluded_scanlators.sq, data/src/main/sqldelight/data/extension_repos.sq, data/src/main/sqldelight/data/sources.sq, data/src/main/sqldelight/view/historyView.sq, data/src/main/sqldelight/view/libraryView.sq, data/src/main/sqldelight/view/updatesView.sq
- Acceptance:
  - `sql/schema_v33.sql` equals the text of `p1-topic-baseline-schema.md` section 2 and is embedded with `include_str!`
  - creating it on an empty file produces exactly 9 tables, 6 named indexes + 4 autoindexes, 7 triggers, 3 views, the seed row `(0, '', -1, 0, 0)` in `categories`, `user_version = 33` and `application_id = 0x4B555230` (TC-O01)
  - the file contains only single-quoted string literals (TC-O08) and `history.last_read` is `NOT NULL`
  - table column order equals `p1-data-and-config.md` section 1 (positional-order test against `pragma_table_info`)

### T-03a-004 — Kurobayomi migration runner (versions 34 and later)
- Priority: P0
- Effort: M
- Depends: T-03a-002, T-03a-003
- Sources: data/src/main/sqldelight/migrations/32.sqm, data/build.gradle.kts
- Acceptance:
  - `SchemaBundle.migrations` is a contiguous ascending list; a gap or duplicate fails a unit test at compile-test time
  - each migration runs in its own transaction, then `PRAGMA foreign_key_check` must be empty or the migration is rolled back (TC-O05, TC-O06)
  - before the first pending migration a `VACUUM INTO` backup `manga.db.pre-v<from>-<UTC>.bak` is created and only the newest 3 backups are kept
  - running with no pending migration performs no write and creates no backup (TC-O02)

### T-03a-005 — Column codecs and storage row structs
- Priority: P0
- Effort: M
- Depends: T-03a-001
- Sources: data/src/main/java/tachiyomi/data/DatabaseAdapter.kt, data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelight/data/history.sq, data/src/main/sqldelight/data/manga_sync.sq
- Acceptance:
  - `GenreList` encodes with `", "`, decodes `''` to an empty list and NULL to `None` (TC-X01)
  - `UpdateStrategy::from_db(7)` returns `AlwaysUpdate`, `to_db` returns 0/1
  - `MangaRow`, `ChapterRow`, `CategoryRow`, `HistoryRow`, `TrackRow`, `ExtensionRepoRow`, `StubSourceRow`, `LibraryRow`, `HistoryViewRow`, `UpdateViewRow` and the patch structs exist with the fields of `p1-rust-design.md` section 2 and map by column name
  - `sum()` columns of the library view are read as `i64`; `lastUpdate`/`nextUpdate` NULL map to 0 at the domain boundary but stay `Option<i64>` in rows

### T-03a-006 — Flag constants and helpers (chapter, category sort, viewer)
- Priority: P0
- Effort: S
- Depends: T-03a-001
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/categories.sq
- Acceptance:
  - constants and masks equal `p1-data-and-config.md` section 4.1 to 4.3 value for value (table-driven test, TC-F01..F03)
  - `set_flag(value, flag, mask) == (value & !mask) | (flag & mask)` and reproduces `SetMangaChapterFlags.awaitSetAllFlags` starting from 0
  - library sort decode: every type value (0, 4, 8, 12, 16, 20, 24, 28, 32, 60) and both directions round-trip; an unknown type falls back to Alphabetical
  - reading mode / orientation lookups mask first and fall back to DEFAULT

### T-03a-007 — mangas statements (18 statements)
- Priority: P0
- Effort: M
- Depends: T-03a-003, T-03a-005, T-03a-026
- Sources: data/src/main/sqldelight/data/mangas.sq
- Acceptance:
  - all 18 statements of `p1-topic-query-catalog.md` section A exist as constants and typed functions with the same SQL semantics (TC-M01..M12)
  - `update` is the single `coalesce` statement; a `None` field never clears a column (TC-M11); ordinary updates send `is_syncing = false`
  - `insert` writes `last_modified_at = 0`, leaves `favorite_modified_at` NULL, and returns `last_insert_rowid()` from the same transaction (TC-M01)
  - `IN` lists use `json_each`; an empty list performs no query (TC-M09)
  - `get_by_id` returns `NotFound` when absent, `get_by_url_and_source` returns `Option`

### T-03a-008 — chapters statements (11 statements)
- Priority: P0
- Effort: M
- Depends: T-03a-003, T-03a-005, T-03a-026
- Sources: data/src/main/sqldelight/data/chapters.sq
- Acceptance:
  - all 11 statements of `p1-topic-query-catalog.md` section B exist (TC-X02, TC-X03, TC-L04..L06)
  - `get_by_manga_id(manga_id, apply_scanlator_filter)` excludes chapters whose `(manga_id, scanlator)` is in `excluded_scanlators` only when the flag is true and never excludes NULL-scanlator chapters
  - batch insert runs in one transaction and rolls back entirely on any error
  - `getScanlatorsByMangaId` returns every row (not distinct) with NULL mapped to `''`
  - removing 40000 ids succeeds in one statement

### T-03a-009 — categories statements and system-category invariants
- Priority: P0
- Effort: M
- Depends: T-03a-003, T-03a-005, T-03a-006
- Sources: data/src/main/sqldelight/data/categories.sq, domain/src/main/java/tachiyomi/domain/category/manga/interactor/CreateMangaCategoryWithName.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/ReorderMangaCategory.kt
- Acceptance:
  - the 10 statements of `p1-topic-query-catalog.md` section C exist; list queries order by `sort, _id` (TC-C02)
  - deleting category 0 fails with `System category can't be deleted` (TC-T12)
  - `update_all_flags(None)` changes nothing; `Some(x)` updates every row including the system category (TC-C03)
  - the first created category gets `sort = 0` and `_id = 1`; `flags` defaults to the global sort flag (TC-C01)
  - renumbering after delete and reorder touches user categories only (Q7 fix)

### T-03a-010 — mangas_categories statements
- Priority: P0
- Effort: S
- Depends: T-03a-003, T-03a-009
- Sources: data/src/main/sqldelight/data/mangas_categories.sq
- Acceptance:
  - `set_categories(manga_id, ids)` deletes then inserts inside one transaction, de-duplicating ids (TC-C05); an empty list leaves the manga uncategorised (TC-C04)
  - each inserted link bumps `mangas.version` by 1 for a non-syncing manga (3 ids give +3) (TC-T06)
  - deleting links bumps nothing

### T-03a-011 — history statements
- Priority: P0
- Effort: M
- Depends: T-03a-003, T-03a-005
- Sources: data/src/main/sqldelight/data/history.sq, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - the 8 statements of `p1-topic-query-catalog.md` section E exist (TC-H01..H04, TC-H09)
  - `upsert(chapter_id, read_at_ms, delta_ms)` sets `last_read` to the new value and ADDS the delta to `time_read` (two upserts of 7 give 14)
  - `reset_*` set `last_read = 0` and keep the row; `remove_resetted` deletes only those; `remove_all` deletes everything
  - `get_read_duration` is 0 on an empty table and counts soft-deleted rows

### T-03a-012 — Library view and query
- Priority: P0
- Effort: M
- Depends: T-03a-003, T-03a-005
- Sources: data/src/main/sqldelight/view/libraryView.sq, data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt
- Acceptance:
  - `libraryView` equals the baseline text and the `library` query returns 31 columns mapped into `LibraryRow` (TC-L01..L07)
  - one row per (favourite manga, category link); no link yields `category = 0`; excluded-scanlator chapters are not counted
  - helper `group_by_category(rows) -> BTreeMap<i64, Vec<LibraryRow>>` exists for consumers and is tested with a manga in two categories (TC-L02)

### T-03a-013 — History view queries (list with search, latest)
- Priority: P0
- Effort: M
- Depends: T-03a-011, T-03a-012
- Sources: data/src/main/sqldelight/view/historyView.sq, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt
- Acceptance:
  - `history(query)` returns one row per manga (the chapter with the maximum `last_read`), only rows with `readAt > 0`, newest first (TC-H05, TC-H07, TC-H10)
  - an empty query returns every manga; search is a case-insensitive substring match of the title
  - P2 part: `%`, `_` and `\` in the user query are escaped with `ESCAPE '\'` so they match literally (TC-H08); the escaped form is the only difference from upstream and is covered by a test
  - `get_latest_history` returns the single most recent entry or none

### T-03a-014 — Updates view queries
- Priority: P1
- Effort: S
- Depends: T-03a-003, T-03a-005
- Sources: data/src/main/sqldelight/view/updatesView.sq, domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt
- Acceptance:
  - `get_recent_updates(after, limit)` and `get_updates_by_read_status(read, after, limit)` filter `dateUpload > after` and return rows ordered by `date_fetch` descending (TC-U01..U03)
  - chapters fetched before `date_added` never appear (TC-U01, TC-U02)
  - the caller passes `limit = 500` (constant `UPDATES_PAGE_LIMIT = 500` exported from the crate)

### T-03a-015 — manga_sync (tracker binding) statements
- Priority: P1
- Effort: M
- Depends: T-03a-003, T-03a-005
- Sources: data/src/main/sqldelight/data/manga_sync.sq, data/src/main/java/tachiyomi/data/track/manga/MangaTrackMapper.kt
- Acceptance:
  - the 6 statements of `p1-topic-query-catalog.md` section F exist; `get_track_by_id` is named by what it does (Q9)
  - inserting a second track for the same (manga, tracker) leaves one row with a new `_id` (TC-S01)
  - `update` is the `coalesce` statement and cannot clear `library_id` (TC-S02); deleting a manga removes its tracks (TC-S03)
  - `private` round-trips as bool; tracker id constants for `sync_id` (1 to 9 for manga trackers) are documented in code

### T-03a-016 — excluded_scanlators statements
- Priority: P1
- Effort: S
- Depends: T-03a-003, T-03a-026
- Sources: data/src/main/sqldelight/data/excluded_scanlators.sq, app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt
- Acceptance:
  - `insert`, `remove` (set of names, empty set is a no-op) and `get_by_manga_id` exist
  - `set_excluded(manga_id, set)` reads the current set, inserts `new - current` and removes `current - new` in one transaction (no duplicates after repeated calls)
  - library counters change accordingly (TC-L05, TC-L06)

### T-03a-017 — extension_repos statements and conflict mapping
- Priority: P1
- Effort: M
- Depends: T-03a-003, T-03a-005
- Sources: data/src/main/sqldelight/data/extension_repos.sq, data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/UpdateMangaExtensionRepo.kt
- Acceptance:
  - the 8 statements exist; `upsert` and `replace` are written `website = :website` (not `=: website`) and prepare successfully (TC-S01 of conformance)
  - a primary-key or fingerprint conflict on `insert`/`upsert` returns `DbError::Constraint`; `replace` on a fingerprint match overwrites `base_url`, `name`, `short_name`, `website` (TC-R01..R04)
  - `count` equals the number of rows; `find_all` is ordered by rowid (insertion order)
  - the helper `classify_conflict(base_url, fingerprint)` returns `RepoAlreadyExists`, `DuplicateFingerprint(existing)` or `Other` exactly like `CreateMangaExtensionRepo.handleInsertionError`

### T-03a-018 — sources (stub source) statements
- Priority: P1
- Effort: S
- Depends: T-03a-003, T-03a-005
- Sources: data/src/main/sqldelight/data/sources.sq, app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt
- Acceptance:
  - `find_all`, `find_one`, `upsert` exist; upserting an existing id replaces `lang` and `name` (TC-G01)
  - `StubSource::visual_name` returns `"<name> (<LANG>)"` when both are non-blank and the numeric id otherwise

### T-03a-019 — Change notification and subscription dependency sets
- Priority: P0
- Effort: L
- Depends: T-03a-001, T-03a-003, T-03a-033
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt, data/src/main/sqldelight/view/libraryView.sq, data/src/main/sqldelight/view/historyView.sq, data/src/main/sqldelight/view/updatesView.sq
- Acceptance:
  - `Tables` bit set has the 9 constants of `p1-topic-reactive-runtime.md` section 3.2; `update_hook`/`commit_hook`/`rollback_hook` are installed on the writer only and `commit_hook` returns false (TC-N01..N03, TC-N09)
  - a chapter `read` update publishes `CHAPTERS | MANGAS` (trigger-driven) and a cascade delete publishes `MANGAS | CHAPTERS | HISTORY`
  - `Db::subscribe(deps, query)` emits once immediately, conflates bursts (TC-N08), never misses a commit between generation read and query (TC-N04) and uses a fresh read transaction per run (TC-N05)
  - the dependency table of section 3.4 is encoded as constants (`LIBRARY_DEPS`, `HISTORY_DEPS`, `UPDATES_DEPS`, `CHAPTERS_OF_MANGA_DEPS`, `CATEGORIES_OF_MANGA_DEPS` and the others)
  - subscriptions expose `pause()`/`resume()` so hidden screens do not re-run the 250 ms library query; the UI debounce (300 ms trailing) is documented for area:37-ui-library

### T-03a-020 — Trigger and versioning fidelity tests
- Priority: P0
- Effort: M
- Depends: T-03a-003, T-03a-007, T-03a-008, T-03a-010
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelight/data/mangas_categories.sq, data/src/main/sqldelight/data/categories.sq
- Acceptance:
  - TC-T01..TC-T13 pass with the exact version numbers of `p1-topic-triggers-and-versioning.md` section 4
  - `last_modified_at` is an integer number of seconds after the first update (within 2 s of `strftime('%s','now')`) and 0 after insert (TC-T01)
  - `PRAGMA recursive_triggers` stays 0 throughout

### T-03a-021 — Schema conformance and statement-prepare tests
- Priority: P0
- Effort: M
- Depends: T-03a-003, T-03a-007, T-03a-008, T-03a-009, T-03a-011, T-03a-015, T-03a-017, T-03a-018
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelight/data/categories.sq, data/src/main/sqldelight/data/extension_repos.sq, data/src/main/sqldelight/view/libraryView.sq, data/build.gradle.kts
- Acceptance:
  - TC-S00: `sqlite_master` of a fresh database equals `fixtures/schema_v33.expected.txt`
  - TC-S01: all 74 statement constants prepare against the baseline (replaces SQLDelight's compile-time verification)
  - TC-S02: `EXPLAIN QUERY PLAN` assertions for the three indexed lookups
  - a CI step regenerates the expected listing and fails on drift

### T-03a-022 — Embedded legacy migrations 1..32 and replay harness
- Priority: P2
- Effort: M
- Depends: T-03a-001
- Sources: data/src/main/sqldelight/migrations/1.sqm, data/src/main/sqldelight/migrations/2.sqm, data/src/main/sqldelight/migrations/3.sqm, data/src/main/sqldelight/migrations/4.sqm, data/src/main/sqldelight/migrations/5.sqm, data/src/main/sqldelight/migrations/6.sqm, data/src/main/sqldelight/migrations/7.sqm, data/src/main/sqldelight/migrations/8.sqm, data/src/main/sqldelight/migrations/9.sqm, data/src/main/sqldelight/migrations/10.sqm, data/src/main/sqldelight/migrations/11.sqm, data/src/main/sqldelight/migrations/12.sqm, data/src/main/sqldelight/migrations/13.sqm, data/src/main/sqldelight/migrations/14.sqm, data/src/main/sqldelight/migrations/15.sqm, data/src/main/sqldelight/migrations/16.sqm, data/src/main/sqldelight/migrations/17.sqm, data/src/main/sqldelight/migrations/18.sqm, data/src/main/sqldelight/migrations/19.sqm, data/src/main/sqldelight/migrations/20.sqm, data/src/main/sqldelight/migrations/21.sqm, data/src/main/sqldelight/migrations/22.sqm, data/src/main/sqldelight/migrations/23.sqm, data/src/main/sqldelight/migrations/24.sqm, data/src/main/sqldelight/migrations/25.sqm, data/src/main/sqldelight/migrations/26.sqm, data/src/main/sqldelight/migrations/27.sqm, data/src/main/sqldelight/migrations/28.sqm, data/src/main/sqldelight/migrations/29.sqm, data/src/main/sqldelight/migrations/30.sqm, data/src/main/sqldelight/migrations/31.sqm, data/src/main/sqldelight/migrations/32.sqm
- Acceptance:
  - `sql/legacy/1.sql` .. `32.sql` are generated by the four stripping rules of `p1-topic-migration-history.md` section 4 and committed; a test regenerates them from the `.sqm` text and compares (TC-I01)
  - the legacy v13 fixture plus migrations 14..32 yields the upgraded-schema differences of `p1-topic-baseline-schema.md` section 4 and nothing else (TC-I02)
  - migration 10 reproduces the single shared `date_added` (TC-I04), migration 28 trims scanlators (TC-I10), migration 29 clamps MangaUpdates scores (TC-I09)
  - the module is behind the `legacy-import` feature and is not compiled into a build that disables it

### T-03a-023 — Aniyomi tachiyomi.db importer
- Priority: P2
- Effort: L
- Depends: T-03a-022, T-03a-003, T-03a-004, T-03a-027
- Sources: data/src/main/sqldelight/migrations/14.sqm, data/src/main/sqldelight/migrations/15.sqm, data/src/main/sqldelight/migrations/26.sqm, data/src/main/sqldelight/migrations/32.sqm, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `import_aniyomi_db(src, dest, progress)` follows the 9 steps of `p1-topic-migration-history.md` section 5 and returns `ImportReport` (TC-I03, TC-I05, TC-I06, TC-I07)
  - source versions below 14 and above 33 are refused with the documented messages; 14 to 33 are accepted
  - the source file is opened read-only and copied (with `-wal`/`-shm`) before any write; the live database is untouched on failure and renamed to `manga.db.bak-<UTC>` only after success and user confirmation (TC-I08)
  - copied rows keep `_id`, `version`, `last_modified_at` and `favorite_modified_at` exactly (no trigger side effects)
  - the report lists applied quirks (`DateAddedBackfillBug`, `HistoryTempDropped`, `MangasCategoriesTimestampDropped`)
  - manual test with a real exported database through `KURO_TEST_ANIYOMI_DB` documented

### T-03a-024 — Migration 34: chapters.url index and favourites-first library view
- Priority: P2
- Effort: S
- Depends: T-03a-004, T-03a-012
- Sources: data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelight/view/libraryView.sq
- Acceptance:
  - `sql/migrations/34.sql` contains exactly `CREATE INDEX chapters_url_index ON chapters(url);` and the redefinition of `libraryView` with `AND chapters.manga_id IN (SELECT _id FROM mangas WHERE favorite = 1)` from `p1-topic-baseline-schema.md` section 5
  - on the benchmark database the url lookup is below 1 ms and the library query with 500 favourites out of 5000 manga is below 100 ms (measured 0.05 ms and 34 ms)
  - the result rows of the old and the new view are identical on the benchmark and on the TC-L fixtures (EXCEPT both ways returns 0 rows) (TC-L08)
  - `MANGA_SCHEMA.migrations` lists the new file and `SCHEMA_VERSION` becomes 34

### T-03a-025 — Unicode-aware duplicate title matching
- Priority: P2
- Effort: S
- Depends: T-03a-007
- Sources: data/src/main/sqldelight/data/mangas.sq, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetDuplicateLibraryManga.kt
- Acceptance:
  - a scalar SQL function `kuro_lower(text)` (Rust `str::to_lowercase`) is registered on every connection through rusqlite `functions` with `FunctionFlags::SQLITE_DETERMINISTIC | SQLITE_UTF8`
  - `get_duplicate_library_manga` compares `kuro_lower(title) = :title` with `:title` lowercased by Rust; a library entry "Ärger" is found when adding "ärger" (fails upstream, verified `lower('Ä')` stays `Ä`)
  - ASCII behaviour is unchanged (TC-M06)

### T-03a-026 — IN-list handling through json_each
- Priority: P1
- Effort: S
- Depends: T-03a-001
- Sources: data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/excluded_scanlators.sq
- Acceptance:
  - helpers `ids_json(&[i64]) -> String` and `strings_json(&[String]) -> String` exist and every `IN :list` statement uses `IN (SELECT value FROM json_each(:x))`
  - 40000 chapter ids are removed by one statement (TC-X02); an empty list is a no-op without touching SQLite
  - string lists with quotes, backslashes and non-ASCII characters round-trip (test)

### T-03a-027 — Integrity check, corruption quarantine, dirty marker
- Priority: P1
- Effort: M
- Depends: T-03a-002, T-03a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - a `manga.db.dirty` marker is created on open and removed on clean shutdown; when found at startup `PRAGMA integrity_check` runs before the UI shows library data
  - a failing check or `SQLITE_CORRUPT`/`SQLITE_NOTADB` moves the file (with sidecars) to `manga.db.corrupt-<UTC>` and returns `DbError::Corrupt`
  - the UI-facing error enum distinguishes `LegacyFile`, `TooNew`, `Corrupt`, `Io` so the dialog texts of `T-03a-039` can be chosen
  - restore from the newest pre-migration backup is available through one function and tested with a deliberately truncated file

### T-03a-028 — Maintenance: WAL checkpoint, optimize, vacuum after bulk delete
- Priority: P2
- Effort: S
- Depends: T-03a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt
- Acceptance:
  - on clean shutdown `PRAGMA wal_checkpoint(TRUNCATE)` and `PRAGMA optimize` run on the writer
  - after "Clear database" (`T-03a-029`) removes more than 1000 chapters, `PRAGMA wal_checkpoint(TRUNCATE)` runs and `VACUUM` is offered (not forced) to the caller
  - `ANALYZE` runs after a legacy import and after a restore of more than 1000 entries

### T-03a-029 — Clear database operations
- Priority: P1
- Effort: S
- Depends: T-03a-007, T-03a-011
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/history.sq, app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt
- Acceptance:
  - `sources_with_non_library_manga()` returns `(source_id, count)` for `favorite = 0`
  - `delete_non_library_by_sources(ids)` followed by `remove_resetted_history()` run inside ONE transaction (Q17); afterwards no chapters, history, tracks, category links or excluded scanlators remain for the deleted entries, and library entries are untouched
  - an empty source list is a no-op
  - the screen text keys to use are `clear_database_source_item_count`, `database_clean`, `clear_database_completed`, `clear_database_confirmation` (AYMR), `pref_clear_manga_database` (AYMR) (owned by area:47-settings-screens-general)

### T-03a-030 — Startup hooks: reset is_syncing, reset viewer flags
- Priority: P1
- Effort: S
- Depends: T-03a-007, T-03a-008
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/chapters.sq, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - after every successful open the two `resetIsSyncing` statements run once; rows with `is_syncing = 1` become 0 (test sets a row to 1 and reopens)
  - `reset_viewer_flags()` sets every `viewer` to 0 and returns success/failure so the settings action can show `pref_reset_viewer_flags_success` or `pref_reset_viewer_flags_error`
  - neither operation changes `version`

### T-03a-031 — Backup/restore-facing query contract
- Priority: P1
- Effort: S
- Depends: T-03a-007, T-03a-008, T-03a-011, T-03a-015, T-03a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaCategoriesRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaExtensionRepoRestorer.kt
- Acceptance:
  - functions exist for exactly the calls the backup code makes: `get_all_source_and_url`, `get_read_manga_not_in_library`, `get_chapters_by_manga_id(apply_scanlator_filter = false)`, `get_history_by_chapter_url`, `get_chapter_by_url`, `get_tracks_by_manga_id`, `get_excluded_scanlators`, category insert + `last_insert_rowid`
  - a restore-style update with `is_syncing = true` does not change `version` and a later ordinary update resets the flag (TC-T08 to TC-T10 analogues)
  - restore of a history item uses `upsert` with the delta `max(backup, db) - db.time_read` and yields a stored total equal to `max(backup, db)` (unit test)
  - one restore transaction per manga is possible through `Db::write` with nested helpers taking `&Transaction`

### T-03a-032 — Upcoming manga query
- Priority: P1
- Effort: S
- Depends: T-03a-007
- Sources: data/src/main/sqldelight/data/mangas.sq, domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt, app/src/main/java/mihon/feature/upcoming/manga/UpcomingMangaScreenModel.kt
- Acceptance:
  - `get_upcoming_manga(start_of_day_ms, statuses)` returns favourites with `next_update >= start_of_day_ms` and status in the given set, ascending by `next_update` (TC-M05)
  - the default status set is `{1, 4}` (ONGOING, PUBLISHING_FINISHED) exposed as a constant
  - start of day is computed from the local time zone by the caller (documented in the function)

### T-03a-033 — Db handle: writer/reader pool and transaction API
- Priority: P0
- Effort: L
- Depends: T-03a-002, area:06-data-repos-history-track-source-misc
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/manga/MangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt
- Acceptance:
  - `Db::read` and `Db::write` run closures through `spawn_blocking`; `write` uses an IMMEDIATE transaction, commits on `Ok`, rolls back on `Err` or panic
  - four reader connections plus one writer; all have the pragmas of `T-03a-002`; the prepared-statement cache capacity is at least 64
  - nested helper functions accept `&Connection`/`&Transaction`, so an outer transaction covers nested repository calls (a test performs insert + update + delete across three helpers and rolls all back with one `Err`)
  - area:06-data-repos-history-track-source-misc builds the repository-facing handler on this type; the task list of area:06-data-repos-history-track-source-misc references this one instead of duplicating it

### T-03a-034 — Wire the database into the mockup app and replace fabricated data
- Priority: P0
- Effort: M
- Depends: T-03a-019, T-03a-033, T-03a-012, T-03a-013, T-03a-014, T-03a-009
- Sources: data/src/main/sqldelight/view/libraryView.sq, data/src/main/sqldelight/view/historyView.sq, data/src/main/sqldelight/view/updatesView.sq, data/src/main/sqldelight/data/categories.sq
- Acceptance:
  - `main.rs::build_ui` opens `manga.db` through `open_and_migrate` before building pages and stores the `Db` handle where `ui.rs` keeps its `thread_local!` state, exposed as `ui::db()`
  - `screens::library`, `screens::history`, `screens::updates`, `screens::categories` and `settings::clear_db` read from the database instead of `SAMPLE`/`title_n`; an empty database shows the spec `empty` text of the screen ("Your library is empty", "Nothing watched recently" style strings, "Nothing to clear")
  - no sample rows are inserted anywhere (the app starts with an empty library)
  - a write in one place (for example adding a category) updates the visible list without manual refresh through the subscription of `T-03a-019`
  - GTK widgets are only touched on the main thread (verified by running with `GTK_DEBUG=interactive` off and a debug assertion in the bridge)

### T-03a-035 — Sibling database contract for the anime schema
- Priority: P0
- Effort: S
- Depends: T-03a-002, area:04-db-anime-schema-migrations
- Sources: data/build.gradle.kts, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `SchemaBundle`, `open_and_migrate`, `Db`, `Tables` and the notification machinery are generic over the bundle: `ANIME_SCHEMA` (file `anime.db`, own baseline and version, same `application_id` scheme with a different constant) can be defined by area:04-db-anime-schema-migrations without touching manga code
  - the manga and anime databases are separate files with separate writers (no cross-database transaction)
  - a test opens both in one process and checks that a commit in one never notifies subscribers of the other

### T-03a-036 — Drop: SQLDelight Gradle plugin, generated Kotlin API, manifest
- Priority: P3
- Effort: S
- Depends: T-03a-003
- Sources: data/build.gradle.kts, data/src/main/AndroidManifest.xml, gradle/libs.versions.toml
- Acceptance:
  - decision recorded: the Gradle build file (SQLDelight plugin 2.0.2, `Database`/`AnimeDatabase` declarations, `kotlinx-serialization` and `projects.*` wiring) and the empty manifest are not ported; replacement is the Cargo manifest of `kuro-db` plus hand-kept SQL constants
  - what the user loses: nothing visible; the developer loses SQLDelight's compile-time SQL checking, replaced by TC-S01 (every statement prepares) and TC-S00 (schema listing)
  - the `kotlinx-serialization` JSON/protobuf/okio dependencies of this module are covered by the areas that own the code using them (backup: area:21-backup-format-and-create)

### T-03a-037 — Drop: Android SQLite drivers and Database Inspector support
- Priority: P3
- Effort: S
- Depends: T-03a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, gradle/libs.versions.toml
- Acceptance:
  - decision recorded: `AndroidSqliteDriver`, `RequerySQLiteOpenHelperFactory` (requery sqlite-android 3.45.0), `FrameworkSQLiteOpenHelperFactory` for the debug-only Android Studio inspector, `androidx.sqlite` 2.4.0 and the SQLDelight coroutines/paging extensions are replaced by `rusqlite` bundled SQLite 3.53.2
  - what the user loses: nothing; the developer inspects `manga.db` with the `sqlite3` CLI or any SQLite browser (WAL mode readable concurrently)
  - the three pragmas of `AppModule.kt:88-95` are carried over by `T-03a-002`

### T-03a-038 — Drop: schema snapshot directory and migration verification
- Priority: P3
- Effort: S
- Depends: T-03a-021
- Sources: data/build.gradle.kts, data/src/main/sqldelight/migrations/32.sqm
- Acceptance:
  - decision recorded: `schemaOutputDirectory` (no `.db` snapshots are committed upstream, so SQLDelight's migration verification was never active) is replaced by the conformance tests of `T-03a-021` and the importer replay tests of `T-03a-022`
  - what the user loses: nothing

### T-03a-039 — User-facing messages for database-level failures
- Priority: P1
- Effort: S
- Depends: T-03a-027
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - new Kurobayomi-only strings are defined (Aniyomi has none): library created by a newer version (`TooNew`), Aniyomi database found and import offered (`LegacyFile`), database damaged with restore option (`Corrupt`), import report summary, pre-migration backup notice
  - existing keys are reused where upstream has them: `clear_database_*`, `pref_reset_viewer_flags*`, `error_category_exists`, `error_repo_exists`, `invalid_repo_name`, `action_replace_repo_title`, `action_replace_repo_message`, `confirm_add_duplicate_manga`, `database_clean`
  - strings are registered in the i18n catalog of area:51-i18n with the same placeholder style (`%1$d`, `%1$s`)
