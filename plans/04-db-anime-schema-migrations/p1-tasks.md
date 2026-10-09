# Area 04 — Anime database (schema, views, queries, migrations) — implementation tasks (part 1 of 1)

All paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Ids run T-04a-001 to T-04a-036. Companion documents: `p1-behavior.md` (statement semantics), `p1-data-and-config.md` (columns, enums, flag layouts), `p1-rust-design.md` (crate layout, signatures), `p1-topic-fresh-vs-upgraded-schema.md`, `p1-topic-views-and-aggregates.md`, `p1-topic-triggers-and-versioning.md`, `p1-topic-legacy-migration-chain.md`, `p1-topic-reactive-queries.md`.

Shared-infrastructure note: the manga database (area:03-db-manga-schema-migrations) needs the same connection factory, writer/reader split, change bus and migration runner. T-04a-001, T-04a-002, T-04a-007 and T-04a-008 are written for both databases (they live in `kuro-db::core`, generic over a `DbKind`); whichever area lands them first satisfies the other, the second area only adds its `DbKind::Manga` constants.

## Foundation (connection, schema, codecs)

### T-04a-001 — kuro-db crate: connection factory, pragmas, file location
- Priority: P0
- Effort: M
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AnimeDatabaseHandler.kt
- Acceptance:
  - a workspace crate `kuro-db` (no GTK dependency) builds with `rusqlite` feature `bundled` and `hooks`
  - `Db::open(DbKind::Anime, &Paths)` creates `<data_dir>/anime.db` (never a literal path) and every connection it hands out reports `PRAGMA foreign_keys` = 1, `journal_mode` = wal, `synchronous` = 1 (NORMAL), `busy_timeout` = 5000
  - `recursive_triggers` stays 0 on every connection (asserted in a unit test, because the version triggers rely on it)
  - the file name constant `anime.db` and the 139 schema version are defined once in `kuro-db::anime::consts`

### T-04a-002 — Schema version guard and fresh-install creation flow
- Priority: P0
- Effort: S
- Depends: T-04a-001, T-04a-003
- Sources: data/build.gradle.kts, data/src/main/sqldelightanime/migrations/138.sqm, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - opening an empty file (user_version 0 and no tables) runs the whole fresh schema in one transaction and then sets `PRAGMA user_version = 139`
  - opening a file whose user_version equals `ANIME_SCHEMA_VERSION` (139; becomes 140 when T-04a-036 lands) does nothing; a file with a greater user_version is refused with a typed error `DbError::NewerThanApp { found, supported }` and the file is left untouched
  - a file with user_version between 113 and 138 is routed to the legacy chain of T-04a-025 (P2); until that task exists it is refused with `DbError::NeedsLegacyImport { found }` and left untouched; a non-empty file with user_version below 113 is refused with `DbError::TooOldForImport { found }` and left untouched
  - a failure inside the creation transaction leaves no tables and user_version 0 (rolled back)

### T-04a-003 — Embed the fresh anime schema (10 tables, 9 indexes, 7 triggers, 2 seed rows)
- Priority: P0
- Effort: M
- Depends: T-04a-001
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/dataanime/episodes.sq, data/src/main/sqldelightanime/dataanime/animehistory.sq, data/src/main/sqldelightanime/dataanime/animes_categories.sq, data/src/main/sqldelightanime/dataanime/categories.sq, data/src/main/sqldelightanime/dataanime/anime_sync.sq, data/src/main/sqldelightanime/dataanime/animesources.sq, data/src/main/sqldelightanime/dataanime/anime_relations.sq, data/src/main/sqldelightanime/dataanime/custom_buttons.sq, data/src/main/sqldelightanime/dataanime/extension_store.sq
- Acceptance:
  - `crates/kuro-db/sql/anime/schema.sql` holds every `CREATE TABLE`, `CREATE INDEX`, `CREATE TRIGGER` and seed `INSERT OR IGNORE` of the ten `dataanime/*.sq` files with the SQLDelight annotations (`AS Boolean`, `AS Date`, `AS List<String>`, `AS AnimeUpdateStrategy`, `AS FetchType`, `AS JsonObject`, `import` lines) removed and double-quoted strings rewritten to single quotes (`''` and `'System category can''t be deleted'`)
  - after creation `sqlite_master` lists exactly the 10 tables, 9 named indexes, 7 triggers, and the data rows `categories(_id=0, name='', sort=-1, flags=0, hidden=0)` and `custom_buttons(_id=1, name='+85 s', isFavorite=1, sortIndex=0)` exist, the latter with the byte-exact Lua of `custom_buttons.sq:14-30`
  - a golden test compares `PRAGMA table_info` of every table with `p1-data-and-config.md` section 3 (column order, type, notnull, default)
  - `DELETE FROM categories WHERE _id = 0` fails with message `System category can't be deleted`

### T-04a-004 — Embed the eight views
- Priority: P0
- Effort: M
- Depends: T-04a-003
- Sources: data/src/main/sqldelightanime/view/episodestatsView.sq, data/src/main/sqldelightanime/view/animehistorystatsView.sq, data/src/main/sqldelightanime/view/animeseasonstatsView.sq, data/src/main/sqldelightanime/view/animelibView.sq, data/src/main/sqldelightanime/view/animeseasonsView.sq, data/src/main/sqldelightanime/view/animehistoryView.sq, data/src/main/sqldelightanime/view/animeupdatesView.sq, data/src/main/sqldelightanime/view/animedeletableView.sq
- Acceptance:
  - `crates/kuro-db/sql/anime/views.sql` creates the eight views in dependency order (`episodestatsView`, `animehistorystatsView`, `animeseasonstatsView`, `animelibView`, `animeseasonsView`, `animehistoryView`, `animeupdatesView`, `animedeletableView`)
  - view fixtures from `p1-edge-cases-and-tests.md` section 3 return the exact numbers listed there (seasons parent `seenCount` 2 of 3 children, empty season counted as fully seen, one library row per category membership)
  - `animelibView` and `animeseasonsView` expose `animes.*` columns first and the computed columns `totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen, bookmarkCount, fillermarkCount` (plus `category` for the library view) in that order

### T-04a-005 — Column codecs (dates, booleans, genre list, enums, memo JSON)
- Priority: P0
- Effort: S
- Depends: T-04a-001
- Sources: data/src/main/java/tachiyomi/data/DatabaseAdapter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt
- Acceptance:
  - `StringList` encodes with the separator `", "` and decodes the empty string to an empty list and NULL to `None`; round-trip test for `["Action","Sci-Fi"]` gives the text `Action, Sci-Fi`
  - `AnimeUpdateStrategy` stores the ordinal (`0 ALWAYS_UPDATE`, `1 ONLY_FETCH_ONCE`) and decodes any other value to `ALWAYS_UPDATE`; `FetchType` stores `0 Seasons`, `1 Episodes` and decodes any other value to `Episodes`
  - `Memo` reads a TEXT or BLOB cell, writes a BLOB holding compact UTF-8 JSON, and an invalid or non-object cell is surfaced as `DbError::BadMemo { table, id }` (never a panic); the default empty object is the two bytes `{}`
  - booleans accept any non-zero integer as true and are written as 0/1; epoch values are `i64` milliseconds in a `Millis` newtype

### T-04a-006 — Flag bit-layout constants (episode_flags, season_flags, viewer, category flags)
- Priority: P0
- Effort: S
- Depends: T-04a-005
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeEpisodeFlags.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeSeasonFlags.kt, domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt, domain/src/main/java/tachiyomi/domain/library/model/Flag.kt
- Acceptance:
  - module `kuro-db::anime::flags` defines every constant of `p1-data-and-config.md` section 5 with identical numeric values (episode flags 0x1 to 0x100000, season flags 0x1 to 0x08000000, viewer masks, category sort type/direction)
  - `fn set_flag(flags: i64, flag: i64, mask: i64) -> i64` equals `flags & !mask | (flag & mask)` and a table test reproduces the Kotlin results of `SetAnimeEpisodeFlags.awaitSetAllFlags` for ten input vectors
  - decoders reproduce `Anime.kt` accessors: `skip_intro_length = viewer & 0xFF`, `next_episode_to_air = (viewer & 0xFFFF00) / 16^2`, `next_episode_airing_at = (viewer & 0x0FFFFFFFF000000) / 16^6`, `skip_intro_disable = viewer & 0x100000000000000 == 0x100000000000000`
  - `SeasonDisplayMode::from_i64` maps 0 CompactGrid, 1 ComfortableGrid, 2 CoverOnlyGrid, 3 List and returns `Err` for any other value (Kotlin throws `IllegalArgumentException`)

### T-04a-007 — Change bus and reactive subscriptions
- Priority: P0
- Effort: M
- Depends: T-04a-001
- Sources: data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/QueryPagingAnimeSource.kt
- Acceptance:
  - an `update_hook` on the writer collects changed table names per transaction and a `commit_hook` publishes the set on a `tokio::sync::broadcast` channel only after commit; a `rollback_hook` discards the set
  - `Db::subscribe(tables, query)` emits the current result immediately and again after any commit touching one of the named tables, coalescing commits that arrive within 50 ms into one emission
  - trigger writes and `ON DELETE CASCADE` deletions are reported (test: deleting an anime row notifies `episodes`, `animehistory`, `animes_categories`, `anime_sync`, `anime_relations`)
  - `delete_all_history` (`DELETE FROM animehistory`, no `WHERE`) notifies `animehistory`; `Transaction::touch(TableSet)` exists as the explicit fallback for statements that SQLite's truncate optimisation could hide from the update hook
  - a subscription yields `Err` items without ending the stream; dropping the stream releases its broadcast receiver

### T-04a-008 — Async handler API (read, write, transaction)
- Priority: P0
- Effort: M
- Depends: T-04a-001
- Sources: data/src/main/java/tachiyomi/data/handlers/anime/AnimeDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AnimeTransactionContext.kt
- Acceptance:
  - `Db::read(|conn| ...)` runs on a pool of 4 read connections via `tokio::task::spawn_blocking`; `Db::write(|tx| ...)` runs on the single writer connection inside `BEGIN IMMEDIATE ... COMMIT` and rolls back on `Err` or panic
  - two concurrent `write` calls serialize (second starts after first commits) and a long write never blocks readers (WAL)
  - the closure receives `&Transaction`; there is no API to nest a second transaction (savepoints are exposed as `tx.savepoint()` only)
  - dropping the future of a `write` that has not started rolls nothing back and never leaves a transaction open

### T-04a-009 — Anime row model and entry repository (CRUD, partial update, lookups)
- Priority: P0
- Effort: M
- Depends: T-04a-003, T-04a-005, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `AnimeRow` has the 32 columns of `p1-data-and-config.md` section 3.1 and is read by column name
  - `insert_anime` binds the 28 named parameters of `animes.sq:166-168` in statement order (the 29th column `last_modified_at` receives the literal 0; `is_syncing` and `favorite_modified_at` take their column defaults), returns `last_insert_rowid()` inside the same transaction
  - `update_anime(AnimeUpdate)` runs the `coalesce(:x, col)` statement of `animes.sq:170-201` with `None` meaning keep, always binds `is_syncing = 0`, and `update_all_anime` runs a batch in one transaction and returns `false` (after logging) instead of propagating errors, as `AnimeRepositoryImpl.updateAnime` does
  - after `update_anime` with all fields `None`, `last_modified_at` and `favorite_modified_at` have changed to the current second (documented trigger side effect, covered by a test)
  - `get_by_url_and_source` returns at most one row (`LIMIT 1`), `get_favorites`, `get_favorites_by_source`, `get_watched_not_in_library`, `reset_viewer_flags` match `animes.sq`

### T-04a-010 — Library read model (animelibView) and season aggregates
- Priority: P0
- Effort: M
- Depends: T-04a-004, T-04a-009, T-04a-007
- Sources: data/src/main/sqldelightanime/view/animelibView.sq, data/src/main/sqldelightanime/view/animeseasonstatsView.sq, data/src/main/sqldelightanime/view/episodestatsView.sq, data/src/main/sqldelightanime/view/animehistorystatsView.sq, data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt
- Acceptance:
  - `library()` and `library_stream()` return `LibraryAnimeRow { anime, category, total_count, seen_count, bookmark_count, fillermark_count, latest_upload, episode_fetched_at, last_seen }`; `seen_count`, `bookmark_count`, `fillermark_count` are read as `f64` and truncated to `i64` exactly like `AnimeMapper.mapLibraryAnime`
  - an anime in two categories appears in two rows with those `category` values; an anime in none appears once with `category = 0`
  - the stream re-emits after any change to `animes`, `episodes`, `animehistory`, `animes_categories` (the four tables the view reads)

### T-04a-011 — Episode row model and episode repository
- Priority: P0
- Effort: M
- Depends: T-04a-003, T-04a-005, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/episodes.sq, data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt, data/src/main/java/tachiyomi/data/items/episode/EpisodeSanitizer.kt
- Acceptance:
  - `add_all_episodes` inserts in one transaction with the 17-parameter statement of `episodes.sq:94-96`, returns each episode with the new `_id`, and returns an empty list (after logging) on any failure
  - `update_all_episodes` runs the coalesce update of `episodes.sq:98-118` for a list in one transaction with `is_syncing = 0`
  - `remove_episodes_with_ids` deletes with an `IN` list chunked at 500 ids, swallowing and logging errors like the Kotlin repository
  - reads by anime id, bookmarked-by-anime id, id, url+anime id, and the reactive `episodes_by_anime_stream` exist; `get_episode_by_url` (used by restore) is exposed and may return several rows (first row wins)
  - the sanitizer `sanitize(name, title)` = trim, strip the leading `title`, then trim the 30 characters listed in `p1-behavior.md` section 12

### T-04a-012 — History repository and history view
- Priority: P0
- Effort: M
- Depends: T-04a-004, T-04a-011
- Sources: data/src/main/sqldelightanime/dataanime/animehistory.sq, data/src/main/sqldelightanime/view/animehistoryView.sq, data/src/main/sqldelightanime/view/animehistorystatsView.sq, data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryMapper.kt
- Acceptance:
  - `upsert_history(episode_id, seen_at_ms)` inserts or updates `last_seen` for that episode (one row per episode, unique `episode_id`)
  - `history_stream(query)` returns one row per anime (its most recently watched episode with `seenAt > 0`), newest first, filtered by `lower(title) LIKE '%' || query || '%'`
  - `reset_history(history_id)` and `reset_history_by_anime(anime_id)` set `last_seen = 0` (rows stay), `delete_all_history()` returns `true` on success and `false` after logging on error, `remove_resetted_history()` deletes rows with `last_seen = 0`
  - after resetting the newest row of an anime the stream shows that anime's next most recent row (documented quirk, covered by a test)

### T-04a-013 — Category repository, system category and category flags
- Priority: P1
- Effort: M
- Depends: T-04a-003, T-04a-006, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/categories.sq, data/src/main/sqldelightanime/dataanime/animes_categories.sq, data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/category/model/Category.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/ReorderAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/CreateAnimeCategoryWithName.kt
- Acceptance:
  - all reads of `categories.sq` exist (`get_category`, all, visible, by anime, visible by anime, plus streams) and return `Category { id, name, order, flags, hidden }` with `order` taken from column `sort`
  - `insert_category(name, order, flags)` never writes `hidden` (default 0); `update_category` is a coalesce partial update; `update_all_flags(Option<i64>)` with `None` changes nothing
  - `set_anime_categories(anime_id, ids)` deletes all links and inserts the new ones in one transaction, and the insert trigger bumps `animes.version` once per inserted id unless `is_syncing = 1`
  - deleting category 0 is rejected by the trigger and surfaces as `DbError::SystemCategory`
  - `delete_category` followed by the re-index of the remaining list reproduces the Kotlin order numbers (including the system category receiving `sort = 0`; see `p1-behavior.md` section 7) and removes the id from the five category preference sets listed in `p1-data-and-config.md` section 6

### T-04a-014 — Tracking table repository (anime_sync)
- Priority: P1
- Effort: S
- Depends: T-04a-003, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/anime_sync.sq, data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt, data/src/main/java/tachiyomi/data/track/anime/AnimeTrackMapper.kt
- Acceptance:
  - `insert_tracks` inserts a list in one transaction; inserting a second row for the same `(anime_id, sync_id)` replaces the first and the replacement receives a new `_id` (documented, tested)
  - `update_track` (used by restore) maps parameter `mediaId` to column `remote_id` and `trackingUrl` to column `remote_url`
  - `delete(anime_id, tracker_id)` removes the single matching row; reads: all, by anime id, one by `_id` (the Kotlin method is misnamed `getTrackByAnimeId` but filters on `_id`)
  - the `private` column defaults to 0 and is read as a boolean

### T-04a-015 — Updates feed (animeupdatesView)
- Priority: P1
- Effort: S
- Depends: T-04a-004, T-04a-007
- Sources: data/src/main/sqldelightanime/view/animeupdatesView.sq, data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt
- Acceptance:
  - `recent_updates(after_ms, limit)` and `updates_by_seen(seen, after_ms, limit)` filter `dateUpload > after`, return newest `date_fetch` first (explicit `ORDER BY datefetch DESC` added in Rust, since the view's ORDER BY is not a contract), and cap at the caller limit (500 from `GetAnimeUpdates`)
  - episodes with `date_fetch <= date_added` of their anime and anime that are not favorites never appear
  - the row carries `fillermark`, `last_second_seen`, `totalSeconds` and the cover fields (`source`, `favorite`, `thumbnailUrl`, `coverLastModified`)

### T-04a-016 — Stub source table (animesources)
- Priority: P1
- Effort: S
- Depends: T-04a-003, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/animesources.sq, data/src/main/java/tachiyomi/data/source/anime/AnimeStubSourceRepositoryImpl.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - `upsert_stub_source(id, lang, name)` inserts or overwrites `lang` and `name`; `get_stub_source(id)` returns `Option`; `stub_sources_stream()` emits the full list after every upsert
  - source ids are stored as signed 64-bit integers and round-trip values above 2^63 as their two's complement negative form

### T-04a-017 — Related-anime table (anime_relations)
- Priority: P2
- Effort: S
- Depends: T-04a-003, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/anime_relations.sq, data/src/main/java/tachiyomi/data/entries/anime/AnimeRelationRepositoryImpl.kt
- Acceptance:
  - `replace_relations(anime_id, groups, fetched_at)` deletes the old rows then inserts every `(group name, related id)` pair with `sort_order` counting up from 0 across all groups, in one transaction
  - `related_stream(anime_id)` returns groups in first-seen order of `sort_order` with their anime in `sort_order`; `last_fetched_at(anime_id)` returns the value of an arbitrary row or `None`
  - deleting either the owning anime or a related anime removes the relation rows (foreign keys cascade)

### T-04a-018 — Custom buttons table and button rules
- Priority: P2
- Effort: M
- Depends: T-04a-003, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/custom_buttons.sq, data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/CreateCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/DeleteCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ReorderCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ToggleFavoriteCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt
- Acceptance:
  - repository functions: `find_all` ordered by `sortIndex`, `insert` (new rows always `isFavorite = 0`), `update_partial`, `delete`, `last_insert_id`
  - create assigns `sortIndex = max + 1` (0 when empty); delete renumbers survivors `0..n-1`; reorder moves one item and renumbers; toggle-favorite makes exactly one button favorite (turning the favorite off sets only that row)
  - `CustomButton::content_for(primary_id)` replaces the literal text `$id` with the button id and `$isPrimary` with `true` or `false` (applied in this order to each of content, longPressContent, onStartup)
  - a seed button with `_id = 1` exists on a fresh database and cannot be missing after `INSERT OR IGNORE` is replayed

### T-04a-019 — Extension store table (extension_store)
- Priority: P1
- Effort: S
- Depends: T-04a-003, T-04a-008
- Sources: data/src/main/sqldelightanime/dataanime/extension_store.sq, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeExtensionStoreRestorer.kt
- Acceptance:
  - `upsert_store` writes all 8 columns and overwrites 7 on conflict of `index_url`; the written SQL reads `signing_key = :signingKey` (the source's `=: signingKey` spacing is a SQLDelight quirk that is not copied)
  - `get`, `get_all`, `count_stream`, `delete(index_url)` exist; `insert_from_preference(index_url, name)` writes `badge_label = name`, `signing_key = 'NO_SIGNING_KEY'`, `contact_website = index_url`, `contact_discord = NULL`, `is_legacy = 0`, `extension_list_url = NULL`
  - the table is the place area:20-extension-strategy-redesign reads store records from

### T-04a-020 — Season hierarchy queries (parent_id, fetch_type, deletable parents)
- Priority: P1
- Effort: M
- Depends: T-04a-010, T-04a-009
- Sources: data/src/main/sqldelightanime/view/animeseasonsView.sq, data/src/main/sqldelightanime/view/animedeletableView.sq, data/src/main/sqldelightanime/dataanime/animes.sq, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt
- Acceptance:
  - `seasons_of(parent_id)` and its stream read `animeseasonsView WHERE parent_id = :parentId` with the aggregate columns; ordering is the caller's job (sort flags), the query adds none
  - `children_of(parent_id)` returns `animes WHERE parent_id = :parentId`; `remove_parent_ids(ids)` sets `parent_id = NULL` for the listed ids (chunked at 500) and does not delete rows
  - `deletable_parents()` stream returns `(id, source, fetch_type)` for anime with `favorite = 0 AND parent_id IS NULL`
  - there is no foreign key on `parent_id`; deleting a parent leaves children with a dangling `parent_id` (documented, tested) and the clear-database flow of T-04a-021 handles it

### T-04a-021 — Clear anime database maintenance operation
- Priority: P2
- Effort: M
- Depends: T-04a-020, T-04a-012
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt, data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/dataanime/animehistory.sq
- Acceptance:
  - `clear_non_library(ids, orphaned)` runs in one transaction: delete `animes WHERE favorite = 0 AND _id IN ids` (chunked at 500), set `parent_id = NULL` for `orphaned` (those ids minus ids already deleted), then `DELETE FROM animehistory WHERE last_seen = 0`
  - `collect_deletable_children(parent)` reproduces the recursion of `ClearAnimeDatabaseScreenModel.getDeletableChildren`: favorite children are orphaned, non-favorite children are deleted, a non-favorite child with `fetch_type = Seasons` is recursed into
  - after the operation episodes, history, category links, tracks and relations of the deleted anime are gone (cascade) and library anime are untouched; the UI strings are `AYMR.strings.pref_clear_anime_database`, `AYMR.strings.clear_database_confirmation`, `MR.strings.clear_database_completed`, `MR.strings.database_clean`, `MR.strings.clear_database_source_item_count`

### T-04a-022 — Unicode-aware lowercase function for title matching
- Priority: P1
- Effort: S
- Depends: T-04a-001
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/view/animehistoryView.sq, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetDuplicateLibraryAnime.kt
- Acceptance:
  - every connection registers a deterministic scalar SQL function `kuro_lower(text)` implemented with Rust `str::to_lowercase`
  - `get_duplicate_library_anime(id, title)` compares `kuro_lower(title) = :title_lowercased` and finds `Éclair` when the argument is `éclair` (the stock `lower()` returns `Éclair` unchanged and the Kotlin behaviour would miss the match for non-ASCII titles)
  - `history_stream(query)` uses `kuro_lower(title) LIKE '%' || kuro_lower(:query) || '%' ESCAPE '\'` with `%`, `_` and `\` in the user text escaped, so a search for `100%` matches only titles containing that literal text

### T-04a-023 — Restore support: DB-level API for backup restore
- Priority: P1
- Effort: M
- Depends: T-04a-009, T-04a-011, T-04a-012, T-04a-014, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeCategoriesRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/CustomButtonRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeExtensionStoreRestorer.kt
- Acceptance:
  - functions exist for every statement the restorers run directly: `all_source_and_url()`, `history_by_episode_url(url)`, `episode_by_url(url)`, `update_anime_syncing(...)` (writes `is_syncing = 1` and the supplied `version` without bumping), `insert_anime_for_restore(...)` (writes `next_update = 0`, `calculate_interval = 0`), `update_episode_restore(...)` (nulls every column except flags, progress, `version`, `memo`), `insert_category_returning_id`, `insert_custom_button_returning_id`
  - the version rule of `AnimeRestorer.restoreExistingAnime` is implemented as a pure function `merge_existing(backup, db)` returning which side supplies the fields, with the backup winning only when `backup.version > db.version`
  - a restore of an existing anime ends with `is_syncing = 0` (the final partial update clears it); a test crashes the closure in the middle and asserts the transaction rolled back so no row keeps `is_syncing = 1`

### T-04a-024 — Startup self-heal for stuck is_syncing flags
- Priority: P1
- Effort: S
- Depends: T-04a-009, T-04a-011
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/dataanime/episodes.sq
- Acceptance:
  - on every open after migration the two statements `UPDATE animes SET is_syncing = 0 WHERE is_syncing = 1` and `UPDATE episodes SET is_syncing = 0 WHERE is_syncing = 1` run (the Kotlin statements `resetIsSyncing` exist but have no caller; this is a Proposal to make them useful)
  - the cleanup does not change `version` (the version triggers only react to the listed column changes)

### T-04a-025 — Legacy chain: import a copied Aniyomi animedb (versions 114 to 138)
- Priority: P2
- Effort: L
- Depends: T-04a-002, T-04a-003, T-04a-004
- Sources: data/src/main/sqldelightanime/migrations/113.sqm, data/src/main/sqldelightanime/migrations/114.sqm, data/src/main/sqldelightanime/migrations/115.sqm, data/src/main/sqldelightanime/migrations/116.sqm, data/src/main/sqldelightanime/migrations/117.sqm, data/src/main/sqldelightanime/migrations/118.sqm, data/src/main/sqldelightanime/migrations/119.sqm, data/src/main/sqldelightanime/migrations/120.sqm, data/src/main/sqldelightanime/migrations/121.sqm, data/src/main/sqldelightanime/migrations/122.sqm, data/src/main/sqldelightanime/migrations/123.sqm, data/src/main/sqldelightanime/migrations/124.sqm, data/src/main/sqldelightanime/migrations/125.sqm, data/src/main/sqldelightanime/migrations/126.sqm, data/src/main/sqldelightanime/migrations/127.sqm, data/src/main/sqldelightanime/migrations/128.sqm, data/src/main/sqldelightanime/migrations/129.sqm, data/src/main/sqldelightanime/migrations/130.sqm, data/src/main/sqldelightanime/migrations/131.sqm, data/src/main/sqldelightanime/migrations/132.sqm, data/src/main/sqldelightanime/migrations/133.sqm, data/src/main/sqldelightanime/migrations/134.sqm, data/src/main/sqldelightanime/migrations/135.sqm, data/src/main/sqldelightanime/migrations/136.sqm, data/src/main/sqldelightanime/migrations/137.sqm, data/src/main/sqldelightanime/migrations/138.sqm
- Acceptance:
  - files `crates/kuro-db/sql/anime/legacy/113.sql` to `138.sql` (26 files; file N upgrades user_version N to N+1) hold the steps with annotations stripped, and a runner applies every step from the stored user_version to 138 in one transaction per step, writing `PRAGMA user_version = N+1` after each
  - a fixture built from the reconstructed v113 baseline of `p1-topic-legacy-migration-chain.md` section 2 upgrades to 139 and passes the equivalence test of T-04a-026
  - user_version below 113 is refused (`DbError::TooOldForImport`) and the original file is copied to `<name>.bak-v<N>` before any step runs
  - step 132 and 135 are applied as written (all animes end with `fetch_type = 1`, so season parents from pre-135 databases become plain episode-type entries; documented user loss)

### T-04a-026 — Fresh-versus-upgraded equivalence and drift tests
- Priority: P2
- Effort: M
- Depends: T-04a-025
- Sources: data/src/main/sqldelightanime/migrations/115.sqm, data/src/main/sqldelightanime/migrations/126.sqm, data/src/main/sqldelightanime/migrations/120.sqm, data/src/main/sqldelightanime/migrations/131.sqm, data/src/main/sqldelightanime/migrations/136.sqm
- Acceptance:
  - a test compares `sqlite_master` of a fresh database with an upgraded one and allows exactly the differences in `p1-topic-fresh-vs-upgraded-schema.md` section 2 (index name `library_favorite_index`, trigger names `system_anime_category_delete_trigger` and `update_last_favorited_at_animes`, extra column `animes_categories.last_modified_at` with trigger `update_last_modified_at_animes_categories`, leftover table `animehistory_temp`, defaults of five columns, `animehistory.last_seen` NOT NULL)
  - after a successful upgrade the importer drops the leftover `animehistory_temp` table (Proposal; Aniyomi never removes it)
  - the row codecs of T-04a-005 read both the fresh and the upgraded database without any per-origin branch

### T-04a-027 — Test kit, fixtures and golden dumps
- Priority: P0
- Effort: M
- Depends: T-04a-003, T-04a-004
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/view/animelibView.sq
- Acceptance:
  - crate `kuro-testkit` offers `TestDb::anime()` (in-memory, same pragmas, same schema) and builders `AnimeBuilder`, `EpisodeBuilder`, `HistoryBuilder` that fill every NOT NULL column with the defaults of `Anime::create()` / `Episode::create()`
  - the upstream-derived cases of `p1-edge-cases-and-tests.md` sections 2 to 6 are implemented as `#[test]` functions and pass in CI
  - a golden file `crates/kuro-db/tests/golden/anime_schema_v139.txt` (sorted `sqlite_master` dump) is checked in and compared on every run

### T-04a-028 — Wire the anime database into the app context and the mock screens
- Priority: P0
- Effort: M
- Depends: T-04a-001, T-04a-002, T-04a-010, T-04a-012, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt
- Acceptance:
  - the app context exposes `ctx.anime_db: Arc<AnimeDb>` created at startup before the first window is shown, and closes it (WAL checkpoint, `PRAGMA optimize`) on shutdown
  - `src/screens.rs` functions `library`, `history`, `updates` and `categories` obtain their rows from `ctx.anime_db` streams through one `glib` bridge function (`ui::db_bridge::spawn_stream`) instead of the hard-coded sample rows; with an empty database each screen shows its empty-state text from the spec
  - no sample data is inserted into the database by the application (the library is empty until the user adds an entry)

### T-04a-029 — Source-to-id queries for browse and migration
- Priority: P1
- Effort: S
- Depends: T-04a-009, T-04a-022
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetDuplicateLibraryAnime.kt
- Acceptance:
  - `source_favorite_counts_stream()` returns `(source_id, count)` for `favorite = 1` grouped by `source` and re-emits when `animes` changes
  - `get_duplicate_library_anime(id, title_lower)` returns library anime whose lowercased title equals the argument and whose `_id` differs
  - `get_by_url_and_source` is used by the network-to-local step; the lack of a UNIQUE constraint on `(source, url)` is preserved (no index added) and duplicates resolve to the first row

### T-04a-030 — Upcoming anime query
- Priority: P2
- Effort: S
- Depends: T-04a-009
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, domain/src/main/java/mihon/domain/upcoming/anime/interactor/GetUpcomingAnime.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt
- Acceptance:
  - `upcoming_stream(statuses, start_of_day_ms)` runs `next_update >= :startOfDay AND favorite = 1 AND status IN (...) ORDER BY next_update ASC`, with the caller passing local midnight in epoch milliseconds and the status set `{1, 4, 7}` (ONGOING, PUBLISHING_FINISHED, UPCOMING)
  - rows with `next_update` NULL or 0 are excluded; an empty status set returns no rows

### T-04a-031 — Database file safety: integrity check, backup before upgrade, corruption quarantine
- Priority: P1
- Effort: S
- Depends: T-04a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - before running any migration step on a non-empty file the app copies `anime.db` (and `anime.db-wal` if present after a checkpoint) to `anime.db.bak-v<old_version>` in the same folder, keeping at most 3 such copies (oldest removed)
  - when `PRAGMA quick_check` returns anything other than `ok` at open, the file is renamed to `anime.db.corrupt-<unix seconds>` and a new empty database is created, after the UI shows a blocking dialog offering restore from a backup (Aniyomi has no such step; Proposal)
  - the check is skipped for databases larger than 512 MB and logged

### T-04a-032 — Periodic maintenance (optimize, checkpoint)
- Priority: P2
- Effort: S
- Depends: T-04a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `PRAGMA optimize` runs when the connection pool closes and once per 24 hours of uptime; `PRAGMA wal_checkpoint(PASSIVE)` runs after every 1000 committed write transactions
  - none of these actions run inside a user-initiated write transaction and none changes `user_version`

### T-04a-033 — Drop: Paging source over SQL queries
- Priority: P3
- Effort: S
- Depends: none
- Sources: data/src/main/java/tachiyomi/data/handlers/anime/QueryPagingAnimeSource.kt, data/src/main/java/tachiyomi/data/handlers/anime/AnimeDatabaseHandler.kt
- Acceptance:
  - decision recorded: `subscribeToPagingSource` and `QueryPagingAnimeSource` (Jetpack Paging 3, `LIMIT/OFFSET` pages with invalidation) are not ported because no code path in the app calls them; the user loses nothing
  - if a future screen needs paging, the replacement is keyset pagination on the view columns (documented in `p1-rust-design.md` section 9)

### T-04a-034 — Drop: Android SQLite driver wiring and SQLDelight build features
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, data/build.gradle.kts
- Acceptance:
  - decision recorded: `AndroidSqliteDriver`, `RequerySQLiteOpenHelperFactory`, `FrameworkSQLiteOpenHelperFactory` (Android Studio Database Inspector in debug builds), `Database.Schema` callbacks, `schemaOutputDirectory` and generated `.db` schema snapshots are replaced by `rusqlite` with bundled SQLite; the user loses nothing
  - the replacement for `verifyMigrations` is the golden test of T-04a-027

### T-04a-035 — Drop: statements with no caller
- Priority: P3
- Effort: S
- Depends: none
- Sources: data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/dataanime/episodes.sq
- Acceptance:
  - decision recorded: `getAllAnime` and `getAnimesWithFavoriteTimestamp` (animes.sq:101, 109) have no caller in the app and are not ported; the user loses nothing (they served the removed sync feature)
  - `resetIsSyncing` is not dropped but re-purposed by T-04a-024; the columns `version`, `is_syncing`, `last_modified_at`, `favorite_modified_at` stay in the schema because backups carry them (`BackupAnime.version`, `lastModifiedAt`)

### T-04a-036 — Kurobayomi-native schema step 139 to 140: category-link indexes
- Priority: P2
- Effort: S
- Depends: T-04a-002, T-04a-010
- Sources: data/src/main/sqldelightanime/dataanime/animes_categories.sq, data/src/main/sqldelightanime/view/animelibView.sq
- Acceptance:
  - `native/139.sql` creates `animes_categories_anime_id_index ON animes_categories(anime_id)` and `animes_categories_category_id_index ON animes_categories(category_id)` and sets `user_version = 140`; `ANIME_SCHEMA_VERSION` becomes 140, the fresh schema file includes both indexes, and the golden dump is regenerated
  - `EXPLAIN QUERY PLAN SELECT * FROM animelibView` no longer contains an `AUTOMATIC COVERING INDEX` line for the alias `MC`
  - the legacy chain still ends at 139 and is followed by this native step, so a copied Aniyomi file reaches 140
  - results of every query of section 3 of `p1-edge-cases-and-tests.md` are unchanged
