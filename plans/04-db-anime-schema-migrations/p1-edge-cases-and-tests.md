# Area 04 — Anime database — edge cases, failure modes, tests and fixtures

Paths are repo-relative to `/home/blackbox/aniyomi-src`. All "measured" results come from a harness run in SQLite 3.53.4 (Python `sqlite3`, `foreign_keys = ON` set outside any transaction, schema built from the `.sq` files with annotations removed). The Rust tests below must reproduce them against `rusqlite` with bundled SQLite; where the bundled version differs the test compares behaviour, not version numbers.

Section index: 1 edge cases and failure modes, 2 upstream tests to port, 3 view fixtures with expected numbers, 4 trigger and versioning tests, 5 repository tests, 6 codec and flag tests, 7 legacy-chain seed and expectations, 8 fixtures and tooling.

## 1. Edge cases, failure modes, race conditions

| # | Case | Behaviour to preserve or decision | Test |
|---|---|---|---|
| 1 | `(source, url)` duplicates | No unique index exists (`animes.sq`); `getAnimeByUrlAndSource` is `LIMIT 1` (first by rowid). Do not add a unique index (a restore of a damaged file would start failing). Log a warning when the insert helper sees an existing pair | insert two rows with equal pair; assert the lookup returns the lower `_id` |
| 2 | Source ids above 2^63 | Stored as signed 64-bit; Rust uses `i64` everywhere and converts from `u64` with `as i64` | round-trip `0xF000_0000_0000_0001u64 as i64` |
| 3 | Row id reuse | No AUTOINCREMENT: after deleting the highest `_id` the next insert reuses it. Nothing may cache ids across deletes (history screen keys by `(animeId, episodeId)`) | delete last row, insert, assert same id |
| 4 | `coalesce` cannot write NULL | Clearing a nullable column (description, thumbnail) is impossible through `update`; empty string is the "cleared" value; `parent_id` is cleared by `remove_parent_ids` only | try to clear `parent_id` via update, assert unchanged |
| 5 | Partial updates and lost updates | Two writers touching different columns of one row do not overwrite each other (each `update` only assigns non-NULL params). Rust must keep the statement shape and never read-modify-write a whole row | two threads: one sets `episode_flags`, one sets `last_update`; both survive |
| 6 | Every `animes.update` stamps `favorite_modified_at` | Verified (all-NULL update stamps both stamps) | trigger test 4.1 |
| 7 | `description` NULL to text does not bump `version` | NULL comparison; keep | trigger test 4.3 |
| 8 | Unicode lowercase | SQLite `lower()` and `LIKE` are ASCII-only; Kotlin lowercases Unicode before binding, so non-ASCII duplicate detection fails in Aniyomi (measured: `lower('ÉCLAIR')` = `Éclair`, `'ÉCLAIR' LIKE '%éclair%'` is 0). Decision: `kuro_lower` custom function (T-04a-022) | insert `Éclair`, query `éclair` finds it |
| 9 | LIKE wildcards in the history search | Aniyomi passes raw user text (`%` matches everything). Decision: escape `%`, `_`, `\` (T-04a-022) | search `%` returns nothing unless a title contains `%` |
| 10 | History: resetting the newest row | The next newest row of that anime becomes its entry (verified); the anime disappears only when all its rows are 0 | fixture V1 |
| 11 | History ties | Equal `last_seen` for two episodes of one anime: exactly one row is shown (verified) | insert tie |
| 12 | Fresh DB `last_seen` NULL | Rows with NULL never appear (`seenAt > 0`); codec maps to `None` | insert NULL via raw SQL |
| 13 | Season parent with zero-episode child | Counts as fully seen (`0 = 0`); verified | fixture V2 |
| 14 | Dangling `parent_id` | Deleting a parent leaves children pointing at a missing id (no FK). They are invisible to `animedeletableView` (non-null parent) and show as normal rows elsewhere. The clear-database flow orphans favorites first for that reason | delete parent; query children |
| 15 | Library query row multiplication | N category links give N rows; duplicated links (the table has no UNIQUE) give duplicate rows. Decision: `set_anime_categories` de-duplicates the id list before inserting | pass `[1,1,2]`, expect 2 link rows |
| 16 | `anime_sync` REPLACE | Re-inserting `(anime_id, sync_id)` replaces the row and assigns a new `_id` (measured 1 to 2). UI keys by `(anime_id, sync_id)`, never by `_id` | measured case |
| 17 | `IN` lists | `removeEpisodesWithIds`, `removeParentIdByIds`, `deleteAnimesNotInLibraryByAnimeIds`, `getUpcomingAnime` build `IN (?,...)`. A library clear with 40,000 ids exceeds SQLite's variable limit (32766 for 3.32 and later, 999 earlier). Decision: chunk at 500; an empty list is a no-op | delete 1,200 ids; assert all gone |
| 18 | Foreign keys per connection | Cascades only work with `foreign_keys = ON` on THAT connection; pool connections all set it at open; a test opens a raw connection and asserts the pragma on every pooled connection | pragma test |
| 19 | Triggers and `recursive_triggers` | Must stay OFF; if ON, `update_anime_version`'s inner UPDATE re-fires itself (the guard `new.is_syncing = 0 AND changed-columns` would fail on the inner UPDATE anyway) but `last_modified_at` stamps would be repeated | pragma test |
| 20 | WAL files and crashes | After a crash SQLite recovers from `anime.db-wal` on next open; the app never deletes the `-wal` file; backups copy after `PRAGMA wal_checkpoint(TRUNCATE)` | kill-9 test in CI (optional) |
| 21 | Two app instances | `kuro-platform::SingleInstance` prevents it; if bypassed, `busy_timeout = 5000` yields `SQLITE_BUSY` after 5 s, mapped to `DbError::Busy` | open writer twice, hold a lock, expect Busy within 5.5 s |
| 22 | Disk full or read-only file | `rusqlite::Error::SqliteFailure(DiskFull | ReadOnly)` mapped to `DbError::Io`; the write closure's transaction rolls back; UI shows the generic internal error text | tmpfs of 1 MB |
| 23 | Newer database than the app | `user_version > 139` is refused untouched (`DbError::NewerThanApp`) | set user_version 200 |
| 24 | Interrupted creation | Creation runs in one transaction; a crash leaves an empty file with `user_version = 0` that is recreated on next start | kill during create |
| 25 | Interrupted legacy upgrade | Each step is its own transaction and sets `user_version`; resume continues at the failed step; the `.bak-v<N>` copy exists | fail step 120 on purpose |
| 26 | Clock changes | `strftime('%s','now')` follows the system clock; `last_modified_at` may go backwards when the user changes the clock. Nothing orders by it except restore ordering | none |
| 27 | Time zone / DST | `getUpcomingAnime` receives local midnight in ms; on DST days midnight to midnight is 23 or 25 hours; compute with `chrono::Local` start-of-day, not `now - 24h` | DST fixture (Europe/Berlin 2026-03-29) |
| 28 | Category delete re-index | After a delete the system category's `sort` becomes 0 and users' categories 1..n (verified by reading the interactor); `ReorderAnimeCategory` writes 0..n-1 for non-system categories, so after a delete + reorder both the system and the first user category have `sort = 0`; SQL leaves such ties unspecified. Reproduce the numbers (backups store `order`) and add `, _id ASC` as the tie-break in the Rust category queries (Proposal) so the system category sorts first | interactor test 5.5 |
| 29 | Category writes race | Delete and create are not under the reorder Mutex in Kotlin. Decision: one `tokio::sync::Mutex` for all category writers | concurrent create/delete test |
| 30 | History reset vs upsert race | Reset sets 0; a concurrent upsert from the player sets `now`; last statement wins; no lock needed | none |
| 31 | `memo` invalid | Not an object or invalid UTF-8: read as empty object, log warning with table and id; never fail a list query because of one row (Aniyomi throws) | corrupt memo cell |
| 32 | `genre` with embedded `", "` | Splits into extra items on read (lossy, same as Aniyomi) | round-trip `["A, B"]` gives `["A","B"]` |
| 33 | `date_upload = -1` / `episode_number = -1.0` | Sentinel values from `Episode.create()`; sorting code treats `episode_number < 0` as unrecognised; the DB stores them as given | none |
| 34 | `getEpisodesByAnimeId` order | Unspecified; every consumer sorts. The Rust function sorts by `source_order ASC, _id ASC` only as a stable default for tests | none |
| 35 | Large transactions | `addAllEpisodes` with 5,000 rows in one transaction must finish under 1 s on SSD (about 5,000 prepared executions) | benchmark gate |
| 36 | Reactive storm | Player progress writes each few seconds while the library stream is subscribed: coalescing 50 ms and distinct-until-changed prevent redraws | stream test |
| 37 | Seed rows replay | `INSERT OR IGNORE` of category 0 and button 1 may run on every start; they must not overwrite user edits of button 1 (they do not: IGNORE) | edit button 1, reopen |
| 38 | Hidden category and library | `hidden` only filters `getVisible*`; entries stay in the library and in `animelibView` | none |

## 2. Upstream tests to port

There are no tests inside `data/` (no `data/src/test`). The tests in other modules that exercise columns of this database:

### 2.1 `domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt` (10 tests; they define the values written to `animes.calculate_interval`)

Fixture: `testTime = 2020-01-01T00:00:00Z`, zone UTC, base episode with `dateFetch = dateUpload = testTime` in ms. Helper `episodeWithTime(d)` sets both dates to `testTime + d`. Function under test: `calculateInterval(episodes, zone) -> Int`. Port each as a Rust test of `kuro-core::anime::fetch_interval::calculate_interval` (owned by area:07-domain-entries-items; listed here because it persists into this table):

1. fifty episodes all at `+1 day` (and again with `dateUpload = 0`) gives 7.
2. five episodes at `7,14,21,28,35` days, then ten episodes at `lastUploadDate + 1..10` days (the Kotlin helper `lastUploadDate()` returns the raw epoch-millisecond value of the last old episode as a duration, so the new group sits far after the old one; only its ten consecutive days matter) gives 1.
3. three episodes at `7,14,21` days, then three at `lastUploadDate + 365 days + 7,14,21 days` (same helper) gives 7 (six episodes use a window of 3 distinct days; the newest three are 7 days apart).
4. ten episodes at `+10 hours` gives 7.
5. two at `+1 day` and five at `+2 days` gives 7.
6. twenty at `1..20` days gives 1.
7. twenty at `15*i` hours gives 1.
8. twenty at `2*i` days gives 2.
9. five at `25*i` hours gives 1 (also without upload dates).
10. twenty at `43*i` hours gives 1.
Cases 1 and 9 each assert twice (with upload dates, then with `dateUpload = 0` so the fetch dates decide).

### 2.2 `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt` (anime halves; they define `categories.flags`)

* `AnimeLibrarySort.types.size = 11`, `directions.size = 2`.
* `LastSeen + Ascending` composed with `DateAdded + Ascending` equals `0b01011100` (`0x5C`).
* `DateAdded + Ascending` flag equals `0b01011100`.
* `UnseenCount + Descending` flag equals `0b00001100`; adding a `DateAdded + Ascending` sort onto it gives `0b01011100` and differs from the old flag.
* default sort `Alphabetical + Ascending` flag equals `0b01000000` (`0x40`).

### 2.3 Behavioural tests specified here (no Kotlin original)

Sections 3 to 7.

## 3. View fixtures with expected numbers (T-04a-004, T-04a-010, T-04a-012, T-04a-015, T-04a-020)

Common builders fill every NOT NULL column with defaults: `source = 1`, `status = 1`, `initialized = 1`, `viewer = 0`, `episode_flags = 0`, `cover_last_modified = 0`, `season_flags = 0`, `season_number = -1.0`, `season_source_order = 0`, `background_last_modified = 0`, `memo = {}`; episodes `last_second_seen = 0`, `total_seconds = 0`, `source_order = 0`.

**V1 (history and entry stats).** Anime 1 `Alpha`, favorite 1, `fetch_type = 1`, `date_added = 100`. Episodes 1, 2, 3 (`episode_number` 1, 2, 3; `date_fetch = 200`, `date_upload = 150`); episodes 1 and 2 seen, 3 unseen. History rows: episode 1 at 1000, episode 2 at 3000, episode 3 at 2000.
* `history(query = "")` returns exactly `[(history for episode 2, seenAt 3000)]`.
* Reset the history row of episode 2: result becomes `[(episode 3, 2000)]`. Reset all three: result is empty.
* Library row for anime 1 (before resets): `totalCount 3, seenCount 2, latestUpload 150, episodeFetchedAt 200, lastSeen 3000, bookmarkCount 0, fillermarkCount 0, category 0`.
* Search `ALP` (ASCII upper case) matches; search `zzz` returns empty; search `%` returns the row (raw LIKE, Aniyomi behaviour) UNLESS the escaping of T-04a-022 is on, in which case it returns empty.

**V2 (season parent).** Anime 10 `Parent` favorite 1, `fetch_type = 0`. Children (favorite 0, `parent_id = 10`, `fetch_type = 1`): 11 `S1` with episodes 20, 21 both seen; 12 `S2` with episode 22 seen and episode 23 unseen and bookmarked; 13 `S3` with no episodes. Episode dates as in V1.
* Library row for 10: `totalCount 3` (children), `seenCount 2` (S1 and empty S3), `latestUpload 150`, `episodeFetchedAt 200`, `lastSeen 0`, `bookmarkCount 1`, `fillermarkCount 0`, `category 0`.
* `seasons_of(10)` rows: id 11 `total 2, seen 2, bookmark 0`; id 12 `total 2, seen 1, bookmark 1`; id 13 `total 0, seen 0, bookmark 0`. Rows are not ordered by the query.
* Library contains exactly anime 1 and 10 (children are not favorites).

**V3 (categories).** Categories 1 `a` (sort 0) and 2 `b` (sort 1); links `(anime 1, cat 1)` and `(anime 1, cat 2)`. Library rows for anime 1: two rows with `category` 1 and 2; anime 10 still one row with `category 0`. Remove both links: one row with `category 0`.

**V4 (updates).** Anime 30 favorite, `date_added = 100`; episodes: A `date_fetch = 100`, B `date_fetch = 101`, C `date_fetch = 99`. Feed for anime 30 contains only B. Then (anime 31, `date_added = 10`) with episodes fetched 50 (upload 500), 70 (upload 300), 60 (upload 900): `recent_updates(after = 0, limit = 500)` returns fetch 70, 60, 50 in that order; with `after = 350` returns fetch 60 then 50 (upload 900 and 500); `limit = 1` returns the newest only. An anime that is not a favorite never appears. `updates_by_seen(seen = true)` returns only seen episodes.

**V5 (deletable).** Rows: 40 `favorite 0, parent_id NULL`; 41 `favorite 0, parent_id 10`; 42 `favorite 1`. `deletable_parents()` returns only 40 with its `source` and `fetch_type`.

**V6 (watched not in library).** Anime 50 favorite 0 with one seen episode; anime 51 favorite 0 with an episode `last_second_seen = 5`; anime 52 favorite 0 without progress; anime 53 favorite 1 with a seen episode. `get_watched_not_in_library()` returns 50 and 51.

**V7 (upcoming).** `start_of_day = local midnight ms`. Anime 60 favorite, status 1, `next_update = start_of_day`; anime 61 status 7, `next_update = start_of_day + 1 day`; anime 62 status 2 (COMPLETED), future update; anime 63 status 1 with `next_update` NULL; anime 64 favorite 0 status 1 future update. Result with statuses `{1,4,7}`: `[60, 61]` ordered by `next_update ASC`. Statuses `{}` returns nothing.

## 4. Trigger and versioning tests (T-04a-003, T-04a-027)

Use the measured values in `p1-topic-triggers-and-versioning.md` section 3; each row is one `#[test]`. Stamps are asserted within 2 seconds of `now`. Extra:
* 4.1 `animes.update` with all parameters NULL changes `last_modified_at` and `favorite_modified_at` and not `version`.
* 4.2 inserting two category links bumps `version` by 2; deleting both bumps by 0.
* 4.3 `description` NULL to text does not bump; text to text bumps by 1.
* 4.4 `UPDATE episodes SET seen = 1, version = 5, is_syncing = 0` gives episode version 6 (explicit value plus the trigger bump), anime version +1.
* 4.5 with the anime `is_syncing = 1`, an episode `bookmark` change bumps the episode (9 to 10) and not the anime.
* 4.6 `DELETE FROM categories WHERE _id = 0` fails with the message `System category can't be deleted`; deleting a normal category succeeds and removes its links.
* 4.7 the seed rows exist after creation and after a second `INSERT OR IGNORE` replay with unchanged content.

## 5. Repository tests (T-04a-009 to T-04a-021)

5.1 `insert_anime` then `get_by_id`: all 32 columns round-trip; `genre ["Action","Sci-Fi"]` stored as `Action, Sci-Fi`; `memo {"k":1}` stored as BLOB; `update_strategy` and `fetch_type` as integers.
5.2 `update_anime` with only `title` changes only the title (and stamps); with `genre = Some(vec![])` stores the empty string and reads back as an empty list.
5.3 `add_all_episodes` returns ids in insertion order; on a constraint failure in element 3 of 5 (use an invalid `anime_id` with foreign keys ON) returns an empty list and stores nothing.
5.4 `update_all_episodes` partial: only listed fields change; `is_syncing` ends 0.
5.5 category interactor port: create (order = max + 1), delete (after delete of the middle of three user categories the remaining orders are `system 0, c1 1, c3 2`), reorder (non-system only, 0..n-1), hide toggle, sort-per-category flags with `categorized_display` true and false (all categories receive the flags when false).
5.6 `set_anime_categories` de-duplicates, replaces, and bumps version once per distinct id.
5.7 tracks: re-insert replaces and changes `_id`; `update_track` renames; delete removes one.
5.8 history: upsert twice keeps one row and updates the time; reset then upsert revives; `remove_resetted_history` removes only zeros; `delete_all_history` empties.
5.9 relations: `replace_relations` with groups `[("Sequel",[2,3]),("Spin-off",[4])]` stores sort orders 0,1,2; stream returns the groups in that order; deleting anime 3 removes its relation row.
5.10 custom buttons: create twice (sortIndex 1, 2 after the seed 0), delete the first (survivors renumbered 0,1), toggle favorite on button 2 (button 1 seed becomes non-favorite, exactly one favorite), reorder, expansion of `$id` and `$isPrimary` (text `if $isPrimary then` becomes `if true then` for the primary button and `if false then` for others).
5.11 stub sources: upsert overwrites lang and name.
5.12 extension store: upsert twice updates seven columns; `insert_from_preference` writes the documented constants; count stream emits 0, 1, 0 across insert and delete.
5.13 clear-database port: build parent P1 (non-favorite, `fetch_type = 0`) with children C1 (non-favorite episodes-type), C2 (favorite), C3 (non-favorite, `fetch_type = 0`) with grandchild G1 (non-favorite); expected `ids = [P1, C1, C3, G1]`, `orphaned = [C2]`; after the operation P1, C1, C3, G1 are gone, C2 remains with `parent_id NULL`, and reset history rows are gone.
5.14 duplicate detection: library titles `Éclair` and `Naruto`; `get_duplicate_library_anime(id=other, "éclair")` finds the first (needs `kuro_lower`), `"NARUTO"` lower-cased by the caller finds the second; the entry itself is excluded by id.
5.15 `source_favorite_counts`: three favorites in source 1, one in source 2, a non-favorite in source 3 gives `[(1,3),(2,1)]`.

## 6. Codec and flag tests (T-04a-005, T-04a-006)

* `StringList`: `"" -> []`, `NULL -> None`, `"A, B" -> ["A","B"]`, `["A","B"] -> "A, B"`, `[] -> ""`.
* `AnimeUpdateStrategy` decode of 0 and 1, and of 2, -1 and 99 (fallback `ALWAYS_UPDATE`); `FetchType` decode of 0, 1 and 2 (fallback `Episodes`).
* `Memo`: TEXT `'{}'`, BLOB `x'7b7d'`, BLOB of `{"a":[1,2]}`; invalid JSON and a JSON array give the empty object plus a warning.
* Flag setter: `set_flag(0, 0x2, 0x6)` = 2; `set_flag(0x7, 0x4, 0x6)` = 0x5; ten random `(flags, flag, mask)` triples compared against the Kotlin formula.
* `Anime` viewer accessors: `viewer = (1_700_000_000 << 24) | (12 << 8) | 85` gives `skip_intro_length 85`, `next_episode_to_air 12`, `next_episode_airing_at 1_700_000_000`, `skip_intro_disable false`; `viewer | (1 << 56)` gives `skip_intro_disable true` with the other three unchanged; `viewer = 0` gives 0, 0, 0, false.
* `SeasonDisplayMode` mapping 0..3 and rejection of 4.
* Category sort flags: the 11 type constants and 2 direction constants of `p1-data-and-config.md` section 5.4; `Type::from_flags(0x24)` (not a defined type) falls back to Alphabetical.

## 7. Legacy chain seed and expectations (T-04a-025, T-04a-026)

Seed on the reconstructed v113 baseline (`p1-topic-legacy-migration-chain.md` section 2):

```sql
INSERT INTO categories VALUES (1,'cat',0,64);
INSERT INTO animes VALUES (1,100,'/a',NULL,NULL,NULL,'Action, Drama','Alpha',1,NULL,1,5,6,1,0,0,0,1000);
INSERT INTO animes VALUES (2,100,'/b',NULL,NULL,NULL,NULL,'Beta',2,NULL,0,NULL,NULL,0,0,0,0,0);
INSERT INTO episodes VALUES (1,1,'/e1','E1',NULL,1,0,0,1400,1.0,0,50,0);
INSERT INTO episodes VALUES (2,1,'/e2','E2',NULL,0,1,300,1400,2.0,1,60,0);
INSERT INTO animehistory VALUES (1,1,NULL,500);
INSERT INTO animehistory VALUES (2,2,9000,500);
INSERT INTO animes_categories VALUES (1,1,1);
INSERT INTO anime_sync VALUES (1,1,2,77,NULL,'t',1.0,12,1,8.0,'u',0,0);
```

Expected after running steps 113 to 138 (verified with foreign keys OFF and ON; `integrity_check` = ok, `foreign_key_check` empty):
* `animes`: both rows keep ids 1 and 2, `fetch_type = 1` for both, `date_added` 1000 and 0, `last_modified_at` = upgrade time (seconds), `memo = x'7b7d'`.
* `episodes`: `date_upload` becomes `date_fetch` (50 and 60; step 113), `fillermark = 0`, `memo = x'7b7d'`.
* `animehistory`: `(1, episode 1, last_seen 0)` (NULL coalesced), `(2, episode 2, 9000)`; the old rows remain in `animehistory_temp` (to be dropped by the Rust runner).
* `categories`: `(0,'',-1,0,0)` and `(1,'cat',0,64,0)`; `animes_categories` row gets `last_modified_at` = upgrade time.
* `anime_sync`: `private = 0` appended.
* `animelibView` row for anime 1: `totalCount 2, seenCount 1, lastSeen 9000, category 1`.
* the runner rejects `user_version` 112, upgrades 113, 130, 138, ignores 139, refuses 140.

## 8. Fixtures and tooling

* `kuro-testkit::TestDb::anime()` (in-memory, pragmas as production, schema from `schema.sql` and `views.sql`) and `TestDb::anime_file(dir)` for hook and WAL tests.
* Builders `AnimeBuilder`, `EpisodeBuilder`, `HistoryBuilder`, `CategoryBuilder`, `TrackBuilder` (defaults of `Anime::create()`: `id -1`, `fetch_type Episodes`, `season_number -1.0`; `Episode::create()`: `episode_number -1.0`, `date_upload -1`, `version 1`).
* A deterministic clock is not injectable into SQLite triggers; tests compare stamps to `now` with a 2 s tolerance.
* Golden files: `anime_schema_v139.txt` (sorted `sqlite_master` dump) and `anime_views_columns.txt` (the column contract of `p1-topic-views-and-aggregates.md` section 2, produced by `PRAGMA table_info` on each view).
* The legacy fixture of section 7 as `tests/fixtures/anime_v113_baseline.sql` plus seed.
* CI runs the whole suite with `cargo test -p kuro-db` and also compiles for `x86_64-pc-windows-gnu` (`cargo check`) per the cross-platform requirement.
