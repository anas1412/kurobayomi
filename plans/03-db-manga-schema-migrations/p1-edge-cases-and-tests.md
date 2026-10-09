# Area 03 / part 1 — Edge cases, failure modes, races, tests, fixtures

All numeric expectations marked "verified" were produced by running SQL in `sqlite3` 3.53.4 or `rusqlite 0.40.2` (bundled SQLite 3.53.2) on this machine.

## 1. Upstream tests in scope

None. The `data` module has no `src/test` directory (`find data -path '*test*'` is empty) and no `androidTest`. Tests that exercise code these tables depend on live in other areas and MUST be ported by those areas:

| Upstream test | Area |
|---|---|
| `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt` (fetch interval, writes `calculate_interval` / `next_update`) | area:07-domain-entries-items |
| `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` (writes `chapter_number`) | area:07-domain-entries-items |

Area 03 therefore defines all of its tests as new tests (section 5).

## 2. Edge cases (each one has a decision and, where testable, a test id from section 5)

| # | Situation | Behaviour (OBSERVED unless stated) | Rust decision | Test |
|---|---|---|---|---|
| E1 | Two `mangas` rows with the same `(source, url)` | No unique index; `getMangaByUrlAndSource` returns the first by `LIMIT 1` (index order of `mangas_url_index`, then rowid) | Keep non-unique; `get_by_url_and_source` uses the same SQL; the importer copies duplicates verbatim | TC-E01 |
| E2 | Insert a manga, delete the highest `_id`, insert another | New row reuses the deleted id (no AUTOINCREMENT) | Keep; consumers must not cache ids across deletes without invalidation | TC-E02 |
| E3 | First category created | `max(order)` over `[-1]` = -1, +1 = 0, so `sort = 0`, `_id = 1` | Keep | TC-C01 |
| E4 | Delete category 0 | Trigger aborts: message `System category can't be deleted`; deleting negative ids also aborts | Surface as `DbError::Constraint("System category can't be deleted")`; UI never offers it | TC-T12 |
| E5 | Delete a user category in use | Cascade removes `mangas_categories` rows; manga `version` NOT bumped (verified: 3 category links, delete one, `mangas_categories` count 3 -> 2; no trigger exists for deletes) | Keep | TC-T13 |
| E6 | `setMangaCategories` with duplicates `[2, 2]` | Two rows, two version bumps, duplicate rows in `libraryView` | Rust repository de-duplicates ids before inserting (deviation, safe: no consumer wants duplicates); the DB stays permissive | TC-C05 |
| E7 | `setMangaCategories` with `[]` | Deletes all links; manga appears under category 0 | Keep | TC-C04 |
| E8 | Manga in two categories | `libraryView` yields two rows (`category` 1 and 2) | Consumers group by `category` | TC-L02 |
| E9 | Chapter with NULL scanlator | Never matched by `excluded_scanlators` (NULL = NULL is unknown) so it always counts | Keep | TC-L04 |
| E10 | Excluded scanlator `''` (empty string) | A chapter with scanlator `''` matches it (string equality); NULL does not | Keep | TC-L05 |
| E11 | All chapters of a manga excluded | `libraryView` subquery returns no group: counters coalesce to 0, manga still listed | Keep | TC-L06 |
| E12 | Chapter deleted while history exists | Cascade deletes the history row (this is how "chapter removed from source" loses history) | Keep | TC-H06 |
| E13 | History reset twice | `last_read` stays 0, `time_read` unchanged | Keep | TC-H04 |
| E14 | Upsert history with `time_read = 0` | Row created/updated, `last_read` set, `time_read + 0` | Keep | TC-H02 |
| E15 | History `LIKE` search with `%`, `_` | Act as wildcards because the pattern is built by SQL string concatenation of `%`, the query and `%` | P2: escape (`ESCAPE '\'`), `T-03a-013` | TC-H08 |
| E16 | History for a manga whose latest-read entry has `last_read = 0` but another chapter has `> 0` | The `>0` one is the max and is shown | Keep | TC-H05 |
| E17 | History for a manga where all entries are reset | `readAt > 0` filter hides the manga | Keep | TC-H05 |
| E18 | Two chapters of one manga with identical `last_read` | `maxReadAtChapterId` is one of them (bare-column rule); the history list shows exactly one row for the manga | Keep; tests assert "exactly one row", not which | TC-H07 |
| E19 | `updatesView` and chapters fetched before the entry was added | Excluded by `date_fetch > date_added` | Keep | TC-U01 |
| E20 | Removing a manga from the library and re-adding | `date_added` set to 0 on removal and to `now` on add; old chapters have `date_fetch < date_added`, so they never reappear in Updates | Keep | TC-U02 |
| E21 | `deleteMangasNotInLibraryBySourceIds([])` | `IN ()`/empty `json_each` matches nothing | Keep (Rust returns early) | TC-M09 |
| E22 | `removeChaptersWithIds` with more than 32766 ids | Kotlin: `too many SQL variables` (error swallowed and logged: nothing deleted). Rust: `json_each` has no limit | Rust deviates intentionally | TC-X02 |
| E23 | `addAllChapters` with a duplicate-key or constraint error mid-list | Transaction rolled back, empty list returned, error logged only | Rust returns `Err`, the sync caller (area:10-app-domain-layer) decides; no partial inserts either way | TC-X03 |
| E24 | `update` for a non-existent id | Zero rows changed, no error | Return `Ok(0)`; repositories translate to `false`/`true` per Kotlin (`updateManga` returns true on success even when nothing matched) | TC-M07 |
| E25 | Coalesce patch with all fields `None` | Statement still runs; triggers T3/T5 fire (`last_modified_at` refreshed) | Rust skips the statement when the patch is empty (no observable difference except the timestamp; documented) | TC-M08 |
| E26 | `genre` stored as `''` | decodes to empty list (not NULL) | Keep (`GenreList` codec) | TC-X01 |
| E27 | `genre` containing `", "` inside one genre | Re-split into two genres on read | Keep | TC-X01 |
| E28 | `status` outside 0..6 | Accepted by the DB, interpreted as unknown by UI | Keep raw `i64` | TC-M10 |
| E29 | `update_strategy` value 7 | Decodes as `ALWAYS_UPDATE` | Keep | TC-X01 |
| E30 | `viewer` flags with orientation bits only | `ReadingMode.fromPreference(value and 7)` = DEFAULT; app must mask before lookup | `flags.rs` helpers mask | TC-F02 |
| E31 | `category.flags` with unknown type bits | Falls back to Alphabetical | Keep | TC-F03 |
| E32 | `extension_repos.insert` with existing `base_url` | `UNIQUE`/PK constraint error -> `SaveExtensionRepoException` -> `RepoAlreadyExists` | map `ConstraintViolation` to `DbError::Constraint` | TC-R01 |
| E33 | `extension_repos.insert` with existing fingerprint under another URL | constraint error -> `DuplicateFingerprint` dialog | same | TC-R02 |
| E34 | `extension_repos.replace` when the NEW base_url also exists as another row | Raw constraint error (not wrapped) | Rust returns `DbError::Constraint` | TC-R04 |
| E35 | Repo refresh with changed signing key | Rejected unless stored fingerprint starts with `NOFINGERPRINT` | owned by area:09-domain-source-track-download-release-misc interactor; DB supports it | TC-R05 |
| E36 | `manga_sync` re-insert for same (manga, tracker) | Old row removed, new `_id`; `INSERT` of a second row never fails with UNIQUE | Keep | TC-S01 |
| E37 | `manga_sync.update` setting `library_id = NULL` | Impossible (coalesce) | Keep | TC-S02 |
| E38 | Deleting a manga with tracks | Tracks cascade-deleted | Keep | TC-S03 |
| E39 | Clock set backwards | `date_fetch` ordering uses `nowMillis + n` per fetch; no DB constraint | Keep | - |
| E40 | `last_modified_at` after `INSERT` is 0 | Not 0 after the first `UPDATE` | Keep; tests allow both | TC-T01 |
| E41 | Importing an Aniyomi v33 file by simply opening it | `application_id` = 0, tables present | Refuse with `LegacyFile` | TC-O03 |
| E42 | Opening a file with `user_version` 99 | `TooNew` | Refuse | TC-O04 |
| E43 | Empty zero-byte `manga.db` | `tables == 0` -> create baseline | Keep | TC-O01 |
| E44 | `PRAGMA foreign_key_check` after migrations returns rows | Roll the migration back, keep backup, show error | Keep | TC-O06 |

## 3. Race conditions and ordering hazards

1. Missed notification between first query and listener registration. SQLDelight registers the listener, then emits; a naive "query then subscribe" loses a commit that lands in between. Rust MUST read the current `watch` generation (`Receiver::borrow_and_update`) BEFORE running the query, and compare after: if the generation moved, run again. Test TC-N04.
2. Writer vs readers (WAL): a reader that started before a commit sees old data; the re-run triggered by that commit uses a NEW read transaction. A subscription loop that keeps one reader transaction open across iterations would show stale data forever. Test TC-N05.
3. Two writers (app + importer + backup restore) contend on the single write lock; `busy_timeout = 5000 ms` then `SQLITE_BUSY` -> `DbError::Sqlite`. The importer works on a copy so it never contends. Test TC-N06.
4. Hook callbacks run inside the writer's commit; they must only flip atomics/send on channels. A hook that calls SQLite deadlocks or errors. Code review item; test TC-N07 asserts hooks do not reenter (a debug assertion counts statements executed during the hook).
5. `Db::write` closures run on `spawn_blocking` threads while holding the writer mutex; holding the mutex across an `.await` is impossible because the closure is synchronous. A closure that blocks on the GTK main thread would deadlock the app; never call `glib::MainContext::invoke` and wait inside a write closure.
6. Rapid chapter writes (one per page turn). Each commit wakes every subscription that depends on `chapters`; library re-queries 250 ms each (measured at 300000 chapters). Subscriptions must be paused when their screen is hidden and the UI debounces (`T-03a-019`). Test TC-N08 asserts that N commits within 50 ms produce at most 2 re-runs (conflation).
7. Backup restore inside a transaction per manga while the library screen is subscribed: many commits in a burst; same conflation applies.
8. Migration vs open readers: migrations run before any reader connection exists (`Db` is constructed after `open_and_migrate`).
9. Second app instance: prevented by `SingleInstance` (area:01-foundation-app-bootstrap / `kuro-platform`); if bypassed, the second process fails with `SQLITE_BUSY` after 5 s on the first write; reads work (WAL).

## 4. Failure modes

| Failure | Detection | Response |
|---|---|---|
| Disk full during commit | `SQLITE_FULL` (rusqlite `ErrorCode::DiskFull`) | transaction rolled back automatically; surface `DbError::Sqlite`; UI shows generic error; no partial state |
| Read-only data directory | open fails with `SQLITE_CANTOPEN`/`READONLY` | fatal startup dialog naming the directory |
| Corrupt file | `PRAGMA integrity_check` != `ok`, or `SQLITE_CORRUPT`/`SQLITE_NOTADB` | quarantine `manga.db.corrupt-<UTC>`, offer newest `.bak` restore or empty library |
| Crash during WAL write | WAL replay on next open (SQLite built-in) | automatic; `manga.db.dirty` marker triggers `integrity_check` |
| Migration fails midway | transaction rollback | `user_version` unchanged; pre-migration backup kept; app refuses to start with a clear message |
| `LegacyFile` | `application_id != KUR0` and tables exist | offer "Import Aniyomi database" (`T-03a-023`) |
| Importer finds orphan rows | `foreign_key_check` rows | delete + report, never abort |
| JSON in-list binding too large | text size > `SQLITE_MAX_LENGTH` (1e9) | not reachable (ids are integers) |
| Trigger raises `ABORT` | `SQLITE_CONSTRAINT_TRIGGER` | `DbError::Constraint` with the trigger's message |

## 5. New tests to write (Rust, in `crates/kuro-db/tests/`; each uses an in-memory or temp-file database)

IDs are referenced by tasks. "Expected" values are exact.

### 5.1 Schema and open

| Id | Test | Expected |
|---|---|---|
| TC-O01 | `open_and_migrate` on a new path | file created; `user_version = 33`; `application_id = 0x4B555230`; `journal_mode = wal`; `foreign_keys = 1`; 9 tables, 6 named + 4 auto indexes, 7 triggers, 3 views; `categories` has exactly one row `(0, '', -1, 0, 0)` |
| TC-O02 | reopen | no change, no backup file created |
| TC-O03 | open a database created from the Aniyomi `.sq` DDL (`fixtures/aniyomi_fresh_v33.sql` with `PRAGMA user_version = 33`) | `Err(LegacyFile { found: 33 })` |
| TC-O04 | set `user_version = 99` on a KUR0 file | `Err(TooNew { found: 99, supported: 33 })` |
| TC-O05 | KUR0 file at version 33 with `MANGA_SCHEMA.migrations` containing a test migration 34 (`CREATE TABLE t(x)`) | runs once, `user_version = 34`, a `.bak` copy exists, `foreign_key_check` clean |
| TC-O06 | migration 34 that violates an FK | rolled back, `user_version = 33`, error returned |
| TC-O07 | `recursive_triggers` | `PRAGMA recursive_triggers` returns 0 after open |
| TC-O08 | double-quote check | baseline contains no `"` characters outside comments (grep test) |
| TC-S00 | conformance: `sqlite_master` of a fresh database vs `fixtures/schema_v33.expected.txt` (the sorted `type|name|tbl_name` lines from `p1-topic-baseline-schema.md` section 3 plus normalised `sql`) | identical |
| TC-S01 | every statement constant of `kuro_db::sql::*` prepares against the baseline | all 74 prepare |
| TC-S02 | `EXPLAIN QUERY PLAN` guards | `getMangaByUrlAndSource` uses `mangas_url_index`; `getFavorites` uses `library_favorite_index`; `getChaptersByMangaId` uses `chapters_manga_id_index` |

### 5.2 Triggers and versions (matrix of `p1-topic-triggers-and-versioning.md` section 4)

TC-T01 .. TC-T10 = the ten SQL step rows of that matrix in order (expected manga/chapter versions 1/1, 1/1, 2/2, 3/2, 3/2, 6/2, 6/2, 6/3, 6/3, 7/3). TC-T11 = the three description cases (versions 0, 0, 1). TC-T12: deleting category 0 fails with message `System category can't be deleted` and the row remains; deleting `_id = -1` (inserted manually) also fails. TC-T13: deleting a user category removes its links without bumping `mangas.version`.

### 5.3 Statement semantics

| Id | Test | Expected |
|---|---|---|
| TC-M01 | insert manga, `get_by_id` | all 24 columns round-trip; `last_modified_at = 0`; `favorite_modified_at = None`; `is_syncing = false`; `version` as inserted |
| TC-M02 | `get_by_url_and_source` with two rows same key | returns the lower rowid |
| TC-M03 | `get_favorites`, `get_favorite_by_source_id`, `get_source_id_with_favorite_count` on 3 sources | counts per source exact |
| TC-M04 | `get_read_manga_not_in_library`: non-favourite with one chapter `read = 1`, another with `last_page_read = 3`, another untouched | the first two returned, third not |
| TC-M05 | `get_upcoming_manga(start, [1, 4])` with statuses 1, 2, 4 and `next_update` before/after `start` | only status 1/4 with `next_update >= start`, ascending |
| TC-M06 | `get_duplicate_library_manga("title", id)` | favourites only, `_id != id`, case-insensitive for ASCII |
| TC-M07 | update of missing id | `Ok(0)` |
| TC-M08 | empty patch | `Ok(0)` and no statement executed |
| TC-M09 | `delete_mangas_not_in_library_by_source_ids([])` | no rows deleted, no error |
| TC-M10 | `status = 99` | stored and read back as 99 |
| TC-M11 | coalesce: patch description `None` over existing text | text unchanged |
| TC-M12 | `reset_viewer_flags` | every `viewer = 0`, every `last_modified_at > 0` |
| TC-X01 | `GenreList` codec: `None`, `Some([])`, `["a","b"]`, `["a, b"]` | NULL, `''`, `'a, b'`, `'a, b'`; re-read gives `None`, `[]`, `["a","b"]`, `["a","b"]` |
| TC-X02 | `remove_with_ids` with 40000 ids | all removed in one statement |
| TC-X03 | `add_all_chapters` rolled back on error | none inserted |
| TC-C01 | `create_category` first | `sort = 0`, `_id = 1`, `flags = 64` (default sort preference) |
| TC-C02 | create three, reorder, delete middle | orders `0,1` (Rust fix of Q7: system untouched at -1); `get_categories` ordered `sort, _id` |
| TC-C03 | `update_all_flags(Some(x))` / `update_all_flags(None)` | all rows incl. system get `x` / unchanged |
| TC-C04 | `set_categories(m, [])` | no links; manga listed under `category = 0` in library |
| TC-C05 | `set_categories(m, [2, 2])` | one row (Rust de-dup) |
| TC-L01 | library: 2 chapters read of 3, one bookmarked, history on one | `totalCount 3, readCount 2, bookmarkCount 1, lastRead = max(last_read), latestUpload = max(date_upload), chapterFetchedAt = max(date_fetch)` |
| TC-L02 | manga in categories 1 and 2 | two rows |
| TC-L03 | manga with no links | one row, `category = 0` |
| TC-L04 | NULL scanlator chapters with exclusions present | still counted |
| TC-L05 | excluded `''` | counted out only chapters with `''` |
| TC-L06 | everything excluded | counters 0, manga present |
| TC-L07 | non-favourite excluded from view | absent |
| TC-L08 | after migration 34 view | identical rows to baseline view on the fixture (EXCEPT both ways = 0) |
| TC-H01 | upsert new | `last_read = readAt`, `time_read = delta` |
| TC-H02 | upsert twice (delta 7, 7) | `time_read = 14`, `last_read` = second `readAt` (verified 14) |
| TC-H03 | `get_read_duration` empty / populated | 0 / sum |
| TC-H04 | `reset_history_by_id`, by manga | `last_read = 0`; rows remain; `time_read` unchanged |
| TC-H05 | history view: reset all of a manga | manga hidden; reset all but one -> that one shown |
| TC-H06 | delete chapter | history row gone |
| TC-H07 | tie on `last_read` | exactly one row per manga |
| TC-H08 | search `a%`/`_` | wildcard behaviour baseline; with escape: literal |
| TC-H09 | `remove_resetted_history` | only `last_read = 0` rows removed |
| TC-H10 | `get_latest_history` | greatest `readAt`; none when no rows with `readAt > 0` |
| TC-U01 | updates view: chapter fetched before `date_added` | excluded |
| TC-U02 | remove and re-add | old chapters stay excluded |
| TC-U03 | `getUpdatesByReadStatus(read=false, after, limit)` | filters and limit; ordering by `date_fetch` desc (verified order 500, 300, 200, 150) |
| TC-S01 | tracker re-insert | one row, new `_id` |
| TC-S02 | tracker update coalesce | NULL cannot clear |
| TC-S03 | manga delete | tracks gone |
| TC-R01..R05 | extension repos: duplicate url, duplicate fingerprint, `upsert` conflicts, `replace` conflict on both keys, count | constraint errors as in table E32-E35; `replace` overwrites `base_url` of the matching fingerprint row |
| TC-F01..F03 | flags helpers: `set_flag`, library sort decode of every type/direction value, viewer masks | exact table of `p1-data-and-config.md` section 4 |
| TC-G01 | stub sources upsert/find | name change visible |

### 5.4 Reactive notifications

| Id | Test | Expected |
|---|---|---|
| TC-N01 | single insert into `mangas` | one notification with `MANGAS` bit |
| TC-N02 | rolled-back transaction | none |
| TC-N03 | `UPDATE chapters SET read = 1` | notification mask = `CHAPTERS | MANGAS` (trigger T6), matches the verified `[3]` |
| TC-N04 | commit between "read generation" and "run query" | subscription runs again, final value reflects the commit |
| TC-N05 | long-lived subscriber, later commit | next emission shows the new row |
| TC-N06 | two writers | second waits up to the busy timeout then errors |
| TC-N07 | hook re-entrancy guard | no SQLite call inside a hook (debug counter) |
| TC-N08 | 20 commits in 50 ms with one subscriber | at most 2 emissions after the burst |
| TC-N09 | cascade delete | `DELETE mangas` notifies `MANGAS|CHAPTERS|HISTORY` (verified hook events delete mangas, chapters, history) |

### 5.5 Legacy importer (feature `legacy-import`)

| Id | Test | Expected |
|---|---|---|
| TC-I01 | strip rules (`p1-topic-migration-history.md` section 4) applied to all 32 `.sqm` | every produced file executes; no `AS Boolean/Long/Float/Date`, no `import`, no `"` string literal |
| TC-I02 | v13 fixture + migrations 14..32 | schema equals the "upgraded" side of the verified diff: `history.last_read NOT NULL`, extra `mangas_categories.last_modified_at`, trigger `update_last_favorited_at_mangas`, trigger `update_last_modified_at_mangas_categories`, table `history_temp` |
| TC-I03 | importer output vs baseline | `sqlite_master` equals baseline; rows copied; no `history_temp`; no trigger side effects: copied `version` and `last_modified_at` values equal source values |
| TC-I04 | migration 10 quirk | fixture with mangas A (fetch 500, 400) and B (100, 900) -> both `date_added = 400` after v10 -> report contains `DateAddedBackfillBug` |
| TC-I05 | orphan rows (`chapters.manga_id` pointing nowhere) | deleted and counted |
| TC-I06 | `user_version` 13 / 14 / 33 / 34 | refused / accepted / accepted / refused |
| TC-I07 | WAL-mode source with a `-wal` sibling | content of the WAL is included in the import |
| TC-I08 | swap | live file renamed `manga.db.bak-<UTC>`, new file in place; failure leaves the old file untouched |
| TC-I09 | migration 29 on `sync_id = 7` rows with score -1 | 0 |
| TC-I10 | migration 28 | scanlator `'  Group '` -> `'Group'` |

## 6. Fixtures

| File | Content | How produced |
|---|---|---|
| `crates/kuro-db/tests/fixtures/aniyomi_fresh_v33.sql` | the `.sq` DDL with the mechanical fixes of the baseline doc, but `history.last_read` nullable, trigger `update_last_favorite_at_mangas`; ends with `PRAGMA user_version = 33;` | generated once by the script of `p1-topic-baseline-schema.md` (strip imports/annotations/quotes) |
| `crates/kuro-db/tests/fixtures/legacy_v13.sql` | the six tables of `p1-topic-migration-history.md` section 2 with 3 mangas, 5 chapters, 3 history rows, 2 categories, 2 tracks, `PRAGMA user_version = 13;` | hand-written from the table in section 2 |
| `crates/kuro-db/tests/fixtures/schema_v33.expected.txt` | sorted `sqlite_master` listing | produced from the baseline by `sqlite3` |
| `crates/kuro-db/tests/fixtures/big_library.rs` (generator, not a binary file) | 5000 mangas, 300000 chapters, 50000 history rows, 7000 category links | the `WITH RECURSIVE` script used for the measurements; used by an `#[ignore]` benchmark that asserts library view < 1 s and history view < 200 ms on CI hardware |
| any path in environment variable `KURO_TEST_ANIYOMI_DB` (file name pattern `aniyomi_real_<n>.db`) | a real database exported by the user (NOT committed: personal data) | `T-03a-023` manual test; documented path via environment variable `KURO_TEST_ANIYOMI_DB` |
