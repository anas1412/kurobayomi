# Area 06 — Implementation tasks

IDs `T-06a-nnn`. Priorities: P0 usable daily (history, updates list, sources list/paging, DB core), P1 parity (tracking storage, extension stores/repos, custom buttons), P2 long tail (release check, status queries), P3 Android-only (drop with replacement). All paths are repo-relative to the Aniyomi clone. Behaviour: `p1-behavior.md`; design: `p1-rust-design.md`; SQL: `p1-data-and-config.md`.

## Database core

### T-06a-001 — Db handle: open, PRAGMAs, writer connection, reader pool
- Priority: P0
- Effort: L
- Depends: area:01-foundation-app-bootstrap, area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `Db::open(path, DbKind, &dyn Schema, rt)` creates the file, runs `Schema::create` on a fresh file and `Schema::migrate` when `user_version` is lower, inside one transaction; a failed migration leaves the file at the old version
  - `PRAGMA foreign_keys` returns 1 and `PRAGMA journal_mode` returns `wal` on the writer and on every pooled reader connection; `synchronous` is 1 (NORMAL); `busy_timeout` is 5000
  - A newer `user_version` than the schema returns `DbError::Migration` without touching the file
  - `kuro_core::data::db` re-exports `kuro_core::db` so area:05 paths resolve
  - Two `Db` values (manga, anime) coexist and never share a transaction

### T-06a-002 — Db read/write API and transaction semantics
- Priority: P0
- Effort: M
- Depends: T-06a-001
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/MangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AnimeDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt, data/src/main/java/tachiyomi/data/handlers/anime/AnimeTransactionContext.kt
- Acceptance:
  - `write` runs the closure in one IMMEDIATE transaction on `spawn_blocking`, commits on `Ok`, rolls back on `Err` and on panic; test `write_err_rolls_back` and `write_panic_does_not_poison` pass
  - `read` uses a pooled connection and never waits for a running write (test: a 200 ms write closure does not delay a concurrent `read` by more than 50 ms)
  - `write_blocking` inside a write closure returns `DbError::NestedWrite`
  - Dropping the awaiting future of a `write` does not abort the commit (`dropped_future_still_commits`)
  - Repository functions are plain `fn(&Connection)` so two of them compose in one `write` (demonstrated by a test that upserts history and inserts a track atomically)

### T-06a-003 — Reactive queries: table change notification and watch streams
- Priority: P0
- Effort: L
- Depends: T-06a-002
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt
- Acceptance:
  - `Db::watch(tables, f)` emits once immediately and again after every committed write that touched one of the tables; no emission after a rolled-back write
  - Notification is published after `commit()` (test: the re-queried value always includes the committed row)
  - FK cascade deletes (chapter deleted -> history row gone) notify the child table (`cascade_notifies_child_table`)
  - `DELETE FROM history WHERE 1` notifies (`delete_all_notifies`); a lint test greps the crate for `DELETE FROM <table>;` without WHERE and fails
  - 100 inserts in one write cause exactly two emissions (`burst_is_coalesced`); a lagged broadcast receiver causes one re-run (`lagged_triggers_rerun`)
  - `watch_distinct` suppresses equal consecutive values; dropping the `Subscription` stops its task (`subscription_drop_stops_task`)
  - `Subscription::next` can be awaited from a `glib::MainContext` (smoke test with `futures::executor::block_on`)

### T-06a-004 — DbError and save-exception mapping
- Priority: P0
- Effort: S
- Depends: none
- Sources: domain/src/main/java/mihon/domain/extensionrepo/exception/SaveExtensionRepoException.kt, domain/src/main/java/tachiyomi/domain/custombuttons/exception/SaveCustomButtonException.kt
- Acceptance:
  - `DbError` has variants Sqlite, Pool, NotFound, NestedWrite, Join, Migration and `is_constraint()` true for SQLite constraint violations only
  - `RepoError::Save(DbError)` and `CustomButtonError::Save(DbError)` Display texts are exactly "Error Saving Repository to Database" and "Error Saving Custom Button to Database"
  - `One-row` helper maps `QueryReturnedNoRows` to `NotFound`

### T-06a-005 — Test harness: temp-file databases and fixture builders
- Priority: P0
- Effort: M
- Depends: T-06a-001, area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: data/src/main/sqldelight/data/history.sq, data/src/main/sqldelight/view/historyView.sq, data/src/main/sqldelightanime/dataanime/animehistory.sq
- Acceptance:
  - `Db::open_temp(kind, schema)` returns a usable database and its `TempDir`
  - `Fixture::manga(&db)` and `Fixture::anime(&db)` insert mangas/chapters/history/tracks stating only relevant columns and reject `date_added`/`date_fetch` values between 1 and 10^11
  - Fixture builders are behind `cfg(test)` or the `testkit` feature and are used by every test in this plan

### T-06a-006 — Drop QueryPagingSource (Paging3 over limit/offset)
- Priority: P3
- Effort: S
- Depends: T-06a-003
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/QueryPagingMangaSource.kt, data/src/main/java/tachiyomi/data/handlers/anime/QueryPagingAnimeSource.kt
- Acceptance:
  - Decision recorded in `kuro-core/src/db/mod.rs` docs: no paging source exists; reason "no caller in Aniyomi (grep `subscribeToPagingSource` has no consumer outside the handlers)"; user loses nothing
  - `grep -r "QueryPaging" kuro-core` returns nothing

### T-06a-007 — Drop Android handler plumbing (coroutine transaction context, driver factories)
- Priority: P3
- Effort: S
- Depends: T-06a-002
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt, data/src/main/java/tachiyomi/data/handlers/anime/AnimeTransactionContext.kt, data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt
- Acceptance:
  - No `TransactionElement`/thread-local equivalent exists; the replacement is documented as `Db::write` (single writer, synchronous closure)
  - Database inspector factory switch (debug builds) is dropped with the reason "Android Studio only"; `rusqlite` bundled SQLite is used in all builds

## History

### T-06a-010 — Manga history repository (CRUD, accumulated read time)
- Priority: P0
- Effort: M
- Depends: T-06a-002, T-06a-003, T-06a-005
- Sources: data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/history/manga/MangaHistoryMapper.kt, data/src/main/sqldelight/data/history.sq
- Acceptance:
  - `upsert` of chapter 10 with durations 60000 then 30000 leaves `time_read = 90000` and the second `last_read`; after `reset_by_id` `last_read = 0` and `time_read = 90000`; a further 5000 gives 95000
  - `by_manga_id` includes reset rows; `reset_by_manga_id` touches only that manga's rows
  - `delete_all` returns true and removes all rows including `time_read`; `total_read_duration` then returns 0
  - `total_read_duration` sums reset rows too (`coalesce(sum(time_read), 0)`)
  - Failures of `reset_*`, `delete_all`, `upsert` are logged and do not propagate (H1 test: FK violation returns normally)

### T-06a-011 — Anime history repository
- Priority: P0
- Effort: M
- Depends: T-06a-002, T-06a-003, T-06a-005
- Sources: data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryMapper.kt, data/src/main/sqldelightanime/dataanime/animehistory.sq
- Acceptance:
  - `upsert` overwrites `last_seen` (older value overwrites newer: H12 test) and creates the row when absent
  - `by_anime_id`, `reset_by_id`, `reset_by_anime_id`, `delete_all` behave like the manga twin (no duration column)
  - Mapper maps the 10 view columns in the documented order (`episodeNumber` Double)

### T-06a-012 — History list query: latest entry per series, search, latest
- Priority: P0
- Effort: M
- Depends: T-06a-010, T-06a-011
- Sources: data/src/main/sqldelight/view/historyView.sq, data/src/main/sqldelightanime/view/animehistoryView.sq, data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt
- Acceptance:
  - The five-row scenario from `p1-topic-history-and-updates-views.md` section 1 returns exactly the documented rows (latest chapter per manga; removing the latest reveals the previous; all-reset hides the manga)
  - `watch(query)` re-emits within one notification after an upsert, a reset or a chapter delete
  - Ordering is `readAt DESC, id DESC`; `latest()` equals the first list row
  - ASCII case-insensitive search works; with the `kuro_fold` improvement "éclair" finds "Éclair" and `100%` matches literally; `HISTORY_SEARCH_PARITY=1` restores wildcard semantics
  - Empty query returns every entry; rows carry `cover.is_*_favorite` from `favorite`

### T-06a-013 — History maintenance queries (removeResetted, lookup by chapter/episode url)
- Priority: P1
- Effort: S
- Depends: T-06a-010, T-06a-011
- Sources: data/src/main/sqldelight/data/history.sq, data/src/main/sqldelightanime/dataanime/animehistory.sq, app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt
- Acceptance:
  - `remove_resetted(&Connection)` deletes only rows with `last_read = 0` (or `last_seen = 0`) and returns the count
  - `by_chapter_url` / `by_episode_url` return the history row of the chapter/episode with that url or None
  - Both are plain `fn(&Connection)` usable inside area:22's restore transaction

### T-06a-014 — HistoryRecorder: session timer and incognito rule
- Priority: P0
- Effort: M
- Depends: T-06a-010, T-06a-011, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - `start(t0)` then `flush(t1)` stores `max(0, t1 - t0)` ms and sets start to None; a second flush without `start` stores 0
  - With incognito true at flush time nothing is written
  - Reader hooks: window hidden/unfocused flushes, window shown restarts, loading another chapter flushes then restarts (matches `ReaderActivity.onPause/onResume` and `loadNewChapter`)
  - Player hook records `AnimeHistoryUpdate(episodeId, now)` on progress save when not incognito
  - App shutdown awaits outstanding history writes (test with a `JoinSet`)

## Updates

### T-06a-020 — Updates repositories (recent list, limit 500, three-month window)
- Priority: P0
- Effort: M
- Depends: T-06a-003, T-06a-005
- Sources: data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt, data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt, data/src/main/sqldelight/view/updatesView.sq, data/src/main/sqldelightanime/view/animeupdatesView.sq
- Acceptance:
  - U1-U6 tests pass (zero `date_upload` excluded, strict `>` on both comparisons, favourite only, 500 limit keeps the newest by `date_fetch`)
  - Output order is `date_fetch DESC, id DESC` and is asserted explicitly
  - `three_months_ago_ms` clamps like `ZonedDateTime.minusMonths(3)` (U7 vectors)
  - Mapper builds the 14/16-column rows without `dateUpload`; anime rows carry `fillermark`, `last_second_seen`, `total_seconds`
  - Watcher re-emits when a chapter's `read` flag changes

### T-06a-021 — Updates by read/seen status queries
- Priority: P2
- Effort: S
- Depends: T-06a-020
- Sources: data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt, data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt
- Acceptance:
  - `with_read(read, after)` and `watch_with_read` return only rows with that `read` state inside the same window and limit
  - Anime twin uses `seen`
  - Documented as having no caller in the first release (kept for parity)

## Tracks

### T-06a-030 — Manga track repository
- Priority: P1
- Effort: M
- Depends: T-06a-002, T-06a-003, T-06a-005
- Sources: data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt, data/src/main/java/tachiyomi/data/track/manga/MangaTrackMapper.kt, data/src/main/sqldelight/data/manga_sync.sq
- Acceptance:
  - T1 test: inserting the same (manga, tracker) twice leaves one row with a different `_id`, and `by_id(old)` is None
  - `insert_all` is one transaction (T2 test)
  - `by_id` is documented and tested as a row-id lookup
  - Deleting a manga cascades its tracks and notifies `watch_all` (T3)
  - Mapper column order matches `SELECT *` (status before score in SQL, domain order per mapper)
  - Fractional `last_chapter_read` round-trips exactly (T7)

### T-06a-031 — Anime track repository
- Priority: P1
- Effort: M
- Depends: T-06a-002, T-06a-003, T-06a-005
- Sources: data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt, data/src/main/java/tachiyomi/data/track/anime/AnimeTrackMapper.kt, data/src/main/sqldelightanime/dataanime/anime_sync.sq
- Acceptance:
  - Same tests as T-06a-030 against `anime_sync` with `anime_id`, `last_episode_seen`, `total_episodes`
  - `getTrackByAnimeId` equivalent is named `by_id` and queries `_id`

### T-06a-032 — Coalesced track update for backup restore
- Priority: P1
- Effort: S
- Depends: T-06a-030, T-06a-031
- Sources: data/src/main/sqldelight/data/manga_sync.sq, data/src/main/sqldelightanime/dataanime/anime_sync.sq, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt
- Acceptance:
  - `update_coalesced(&Connection, &Patch)` runs `UPDATE <table> SET <each of the 13 data columns> = coalesce(?, <that column>) WHERE _id = ?` with the 14 parameters in the documented order
  - A patch with all `None` leaves the row unchanged (T5); a patch with only `last_chapter_read` changes only that column
  - Usable inside an area:22 transaction

### T-06a-033 — Track model methods (copy_personal_from, new, conversions)
- Priority: P1
- Effort: S
- Depends: T-06a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrackImpl.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrackImpl.kt, app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt
- Acceptance:
  - `MangaTrack::new(tracker_id)` has the documented defaults; `copy_personal_from(other, false)` leaves `private` unchanged and copies the five personal fields; with `true` also copies `private` (T6)
  - Domain-level `copy_personal_from` always copies `private`
  - Anime twin uses `last_episode_seen`, `started/finished_watching_date` semantics (stored as `start_date`/`finish_date`)
  - No separate "Db" struct exists: trackers (area:27) mutate the domain struct; ids <= 0 mean unsaved

## Legacy item models

### T-06a-034 — Replace legacy Chapter/Episode DB models (identity key, recognised number)
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/Chapter.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/ChapterImpl.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/Episode.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/EpisodeImpl.kt
- Acceptance:
  - Reader/player/sync code uses the domain `Chapter`/`Episode` plus the source DTOs; no struct mirrors the Kotlin `ChapterImpl`
  - `is_recognized_number()` is `number >= 0.0` on both domain structs
  - `ChapterKey { id, url }` / `EpisodeKey` implement `Eq + Hash` over (id, url) and are used wherever Kotlin relied on `ChapterImpl.equals`
  - Source chapter numbers are parsed as `f32` and widened to `f64` exactly like `Float.toDouble()`

## Sources

### T-06a-040 — Source list flows (all, online, with favourite count)
- Priority: P0
- Effort: M
- Depends: T-06a-003, area:15-source-api-manga, area:16-source-api-anime, area:17-source-app-layer
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt, data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - `sources()` copies `supports_latest`; `online_sources()` excludes non-HTTP sources and leaves `supports_latest = false`; `with_favorite_count` leaves it false and sets `is_stub` from the stub test
  - `combine_latest` emits nothing until both inputs have emitted, then on every input change (S1 test: removing a source flips `is_stub` and keeps name/lang)
  - Favourite counts come from `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source ORDER BY source`; sources without favourites are absent
  - Anime `sources()` uses all anime sources, manga uses catalogue sources

### T-06a-041 — Stub source repository
- Priority: P0
- Effort: S
- Depends: T-06a-003, T-06a-005
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaStubSourceRepositoryImpl.kt, data/src/main/java/tachiyomi/data/source/anime/AnimeStubSourceRepositoryImpl.kt, data/src/main/sqldelight/data/sources.sq, data/src/main/sqldelightanime/dataanime/animesources.sq
- Acceptance:
  - `upsert` inserts or overwrites lang/name by id (S9); `get` returns None for unknown ids; `watch_all` re-emits after an upsert
  - Table `sources` is used in the manga DB and `animesources` in the anime DB
  - A blank stub (`""`, `""`) is representable and `to_string()` returns the id

### T-06a-042 — RemotePager (popular, latest, search paging)
- Priority: P0
- Effort: M
- Depends: area:15-source-api-manga, area:16-source-api-anime, area:12-network-stack
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt, data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt
- Acceptance:
  - First load requests page 1; `next_key = page + 1` only when `has_next_page` (S3, S4, S5 tests)
  - An empty page (any page) yields `PagerError::NoResults`; an error on append keeps `next_key` so a second `append()` retries the same page
  - `refresh()` restarts at page 1; the pager is replaced (dropped) by the UI when query/filters change and dropping cancels the in-flight request
  - Search, popular and latest call the matching source method with the page number and (for search) query and filters
  - UI contract documented: prefetch distance 25 items

### T-06a-043 — Listing routing and source-not-installed semantics
- Priority: P0
- Effort: S
- Depends: T-06a-042, T-06a-041
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetRemoteManga.kt, domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt, app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - `Listing { Popular, Latest, Search{query, filters} }` replaces the sentinel query strings (not ported)
  - `pager(source_id, listing)` for an uninstalled manga source returns `SourceNotInstalled` (no panic) and the UI text is "Source not found" (`loader_not_implemented_error`)
  - Error-to-message mapping table implemented: NoResults -> "No results found"; HTTP -> "HTTP %d, check website in WebView"; offline -> "No Internet connection"; unknown host -> "Couldn't reach %s"; others `"<Kind>: <message>"` (plain Exception/IOException show only the message)

### T-06a-044 — Non-library source counts (manga)
- Priority: P1
- Effort: S
- Depends: T-06a-040
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt, data/src/main/sqldelight/data/mangas.sq, domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetMangaSourcesWithNonLibraryManga.kt
- Acceptance:
  - Returns `(Source, count)` for `favorite = 0` rows grouped by source, with `is_stub` from `get_or_stub`
  - Does not re-emit when only the source list changes (parity), but re-emits when `mangas` changes
  - Used by the clear-database screen test fixture (area:47)

## Extension stores and repos

### T-06a-050 — Extension store wire formats (JSON, protobuf, legacy)
- Priority: P1
- Effort: L
- Depends: none
- Sources: data/src/main/java/mihon/data/extension/anime/model/BaseNetworkAnimeExtensionStore.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtension.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtensionRepo.kt
- Acceptance:
  - `WireStore` decodes the JSON fixture and the protobuf fixture to equal values (tags 1-4, 101, 102; extension tags 1-9; enum 0-3)
  - `contentWarning` accepts `UNSPECIFIED/SAFE/MIXED/NSFW` and the `CONTENT_WARNING_*` aliases; any other string errors
  - Unknown JSON keys are ignored; absent nullable keys are None; `WireLegacyRepo` requires `meta` (a modern store JSON fails to decode as legacy)
  - `WireLegacyExtension` decodes `sources` null, empty and populated
  - Conversions to `ExtensionStore` follow the table in `p1-data-and-config.md` section 1.5 (`badge_label = shortName ?: name` for legacy; `is_legacy` flags)

### T-06a-051 — Store service: fetch with gzip/JSON/protobuf sniffing and index_v2
- Priority: P1
- Effort: M
- Depends: T-06a-050, area:12-network-stack
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt
- Acceptance:
  - E1-E9 tests pass using `FakeHttp` (empty body, HTML, bad legacy URL, double `/index.min.json`, self-referencing `index_v2` ends with `TooManyIndexHops` after at most 4 requests, oversize gzip, corrupt gzip, BOM)
  - A legacy `index.min.json` URL stores the derived `.../repo.json` as `index_url`; a modern store keeps the typed URL; an `index_v2` hop stores the v2 URL
  - The error `Display` strings match `p1-topic-extension-store-index.md` section 3 (including "Provided legacy store url is not valid")
  - Gzip is detected by magic bytes only (not by header), and the legacy `repo.json` second request is not gzip-sniffed

### T-06a-052 — Store getExtensions and AvailableExtension mapping
- Priority: P1
- Effort: M
- Depends: T-06a-050, T-06a-051
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtension.kt, domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt
- Acceptance:
  - Three branches (list URL, inline list, legacy `index.min.json`) each covered by a fixture test; E10-E18 pass
  - `lang` is the single language or "all" (also for zero sources); `is_nsfw` for MIXED and NSFW; legacy `name` stripped of the `Aniyomi: ` prefix; legacy `lib_version` from `substringBeforeLast('.')`; legacy synthetic source `(0, name, lang, "")`
  - `apk_url`/`icon_url` of legacy entries are `{base}/apk/{apk}` and `{base}/icon/{pkg}.png`
  - A failing store returns `Err` and is skipped by `fetch_extensions`, which preserves store order and fans out with concurrency 8

### T-06a-053 — Extension store repository (persist, refresh, remove, flows)
- Priority: P1
- Effort: M
- Depends: T-06a-051, T-06a-052, T-06a-003
- Sources: data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/extension_store.sq
- Acceptance:
  - `add` fetches then upserts; adding the same index URL twice leaves one row (E19)
  - `refresh_all` is sequential, continues after a failing store, keeps the old row on failure (E20) and logs `Failed to refresh extension store '<name> (<index_url>)'`
  - A refresh that changes `signing_key` logs a WARN; a refresh that returns a different `index_url` inserts the new row and deletes the old in one transaction (improvement over Kotlin's duplicate row)
  - `watch_all` orders by rowid; `watch_count` re-emits on insert/delete
  - Row mapping order is `index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url`

### T-06a-054 — Migration helper: insert_from_preference and legacy preference import
- Priority: P1
- Effort: S
- Depends: T-06a-053, T-06a-056, area:11-app-migrations
- Sources: data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt
- Acceptance:
  - `insert_from_preference(url, name)` stores `signing_key = "NO_SIGNING_KEY"`, `contact_website = url`, `is_legacy = false`, `extension_list_url = NULL`
  - E27 and E28 vectors produce the documented rows; failures are logged per URL and do not stop the loop; the preference keys `anime_extension_repos` and `extension_repos` are deleted afterwards
  - After the import `refresh_all` runs once so legacy rows get `is_legacy = true` before the first listing

### T-06a-055 — Manga extension repo repository
- Priority: P1
- Effort: M
- Depends: T-06a-002, T-06a-003, T-06a-004, T-06a-005
- Sources: data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt, data/src/main/sqldelight/data/extension_repos.sq, domain/src/main/java/mihon/domain/extensionrepo/manga/repository/MangaExtensionRepoRepository.kt
- Acceptance:
  - `insert` wraps any SQLite error as `RepoError::Save`; `upsert` updates by `base_url` and still fails on a fingerprint clash with another row; `replace` rewrites the row owning the fingerprint including its `base_url`
  - `get_by_fingerprint`, `get`, `all`, `watch_all`, `watch_count` match the SQL in `p1-data-and-config.md` 3.5
  - E24/E25: `replace` onto a third repo's `base_url` returns `Err` instead of panicking

### T-06a-056 — Manga repo interactors: create, update, replace, delete
- Priority: P1
- Effort: M
- Depends: T-06a-055, area:12-network-stack
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/UpdateMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/ReplaceMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/DeleteMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt, domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoDto.kt
- Acceptance:
  - `create` returns InvalidUrl for non-https, non-`/index.min.json`, unparsable URLs and failed `repo.json` fetches (E22, E23); RepoAlreadyExists, DuplicateFingerprint(old, new), Success per the algorithm; URL normalisation vector `https://Example.COM:443/a/../index.min.json` -> base `https://example.com`
  - `update_all` fetches concurrently, writes only when the stored fingerprint starts with `NOFINGERPRINT` or equals the fetched one (E26); one failing repo does not cancel the others
  - The fetch uses `GET {base}/repo.json` and parses `{"meta": {name, shortName?, website, signingKeyFingerprint}}`
  - UI result mapping: InvalidUrl -> "Invalid repo URL", RepoAlreadyExists -> "This repo already exists!", DuplicateFingerprint -> conflict dialog, Error -> no visible effect

### T-06a-057 — Extension store/repo screens: state machines and mockup wiring
- Priority: P1
- Effort: M
- Depends: T-06a-053, T-06a-056, area:47-settings-screens-general, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/ExtensionReposDialogs.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Anime store screen: Loading, empty ("You haven't added an extension store yet"), card list (website, Discord only if present, copy URL, delete), refresh button, add FAB; Create dialog disables confirm when empty/processing/already existing and shows the error message from `StoreError`; Confirm dialog (deep link) shows "Do you wish to add the extension store below?"
  - Successful add or delete triggers a re-listing of available extensions; refresh button only refreshes stores
  - Manga repo screen: create result events as toasts, Conflict dialog with Replace, delete confirmation with the base URL
  - URL schemes `aniyomi://add-repo`, `aniyomi://extension-store` and `tachiyomi://add-repo` with `?url=` open the matching confirm dialog (single-instance forwarding of the URL)

## Custom buttons

### T-06a-060 — Custom button repository with seeded default button
- Priority: P1
- Effort: M
- Depends: T-06a-002, T-06a-003, T-06a-004, T-06a-005
- Sources: data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/custom_buttons.sq, data/src/main/sqldelightanime/migrations/129.sqm
- Acceptance:
  - A fresh anime database contains exactly the seeded row (id 1, `+85 s`, favourite, sort 0) with texts byte-identical to the SQL (C1; `seed_matches_schema_sql`)
  - `all()` orders by `sortIndex, _id`; `create` inserts `is_favorite = false` and returns the id; `insert_raw` accepts `is_favorite` (restore path)
  - `update` applies coalesce semantics (None keeps the column); `update_many` is one transaction; `delete` removes the row; the seed is not re-inserted on restart

### T-06a-061 — Custom button interactors: create, delete, reorder, favourite, update
- Priority: P1
- Effort: M
- Depends: T-06a-060
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/interactor/CreateCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/DeleteCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ReorderCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ToggleFavoriteCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/UpdateCustomButton.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreenModel.kt
- Acceptance:
  - C2-C6, C9 tests pass; create computes `max+1` atomically (two concurrent creates give consecutive indexes)
  - Reorder with an out-of-range index returns an error mapped to the toast "InternalError: Check crash logs for further information"; unknown id returns `Unchanged`
  - Toggle on a non-favourite leaves exactly one favourite; toggle on the favourite leaves none; 50 concurrent toggles keep the invariant
  - Restore helper reports the number of inserted buttons so area:22 can confirm with the user

### T-06a-062 — Custom button placeholder expansion and Lua script assembly for libmpv
- Priority: P1
- Effort: M
- Depends: T-06a-060, area:45-player-core, area:48-settings-player-and-scripts
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/assets/aniyomi.lua
- Acceptance:
  - `expand_placeholders` reproduces C7 and C8 (`$id` replaced everywhere first, then `$isPrimary`)
  - `build_player_scripts` writes one Lua file per button containing the preamble (package.path extension, `require 'aniyomi'`), the expanded onStartup text, `function button<ID>()`, `mp.register_script_message('call_button_<ID>', button<ID>)` and the long-press twin with `call_button_<ID>_long`; blocks are joined with newlines
  - Button press sends `script-message call_button_<id>` through libmpv; long press `call_button_<id>_long`
  - A syntax error in one button leaves the other buttons working (C10); with zero buttons old script files are removed (C11)
  - The `aniyomi` Lua module shipped with the app implements every function listed in `p1-topic-custom-buttons.md` section 4

## Release check

### T-06a-070 — GitHub latest-release client
- Priority: P2
- Effort: M
- Depends: area:12-network-stack
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt, data/src/main/java/tachiyomi/data/release/GithubRelease.kt, domain/src/main/java/tachiyomi/domain/release/service/ReleaseService.kt
- Acceptance:
  - `latest(repo)` requests `https://api.github.com/repos/<repo>/releases/latest`, parses `tag_name`, `body` (null allowed), `html_url`, `assets[].name/browser_download_url`
  - Returns `Ok(None)` when no asset matches; non-2xx returns `ReleaseError::Http(code)` (R4)
  - Startup check is silent on failure and disabled when `UPDATE_REPOSITORY` is empty or the `updater` feature is off; manual check reports the error message
  - The three-day throttle and `last_app_check` handling stay in area:09

### T-06a-071 — Release notes @mention linkifier
- Priority: P2
- Effort: S
- Depends: T-06a-070
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - All vectors of `p1-edge-cases-and-tests.md` section 5 pass using `fancy-regex` with the exact pattern `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` and case-insensitive matching
  - Replacement keeps the `@` in the link text and strips it in the URL

### T-06a-072 — Replace Android ABI asset selection with Linux architecture selection
- Priority: P3
- Effort: S
- Depends: T-06a-070
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - `pick_download_link(assets, ARCH)` implements the `associate` + `map[arch] ?: map[None]` rule with tags `x86_64` and `aarch64`, later asset wins on duplicate keys
  - The Android ABI list (`arm64-v8a`, `armeabi-v7a`, `x86_64`, `x86`) is not present in the Rust code
  - Vectors in the edge-case file pass

### T-06a-073 — Drop in-app APK download/install and the preview update channel
- Priority: P3
- Effort: S
- Depends: T-06a-070
- Sources: data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt
- Acceptance:
  - Dialog buttons: "Download" opens `download_link` with the `Opener`, "Open on GitHub" opens `release_link`, "Not now" closes
  - No APK installer, no download progress notification, no preview-channel repository or `rNNNN` comparison exists in the Rust code; the drop reasons (Android installer only; no preview build type) and the user loss (no one-tap in-app install; distro package manager is the update path) are written in `kuro-core/src/release/mod.rs` docs
  - Release notes shown in the dialog have the checksum footer removed with `(?s)---.*Checksums.*`

## Integration

### T-06a-080 — Wire History and Updates screens of the mockup to repositories
- Priority: P0
- Effort: M
- Depends: T-06a-012, T-06a-014, T-06a-020, area:42-ui-updates-history-stats-storage-upcoming
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - `screens::history()` and `screens::updates()` render rows from the repositories (no `title_n` placeholders), grouped under date headers by local date, per Anime/Manga tab with separate search text
  - Trash opens the existing spec dialog; its switch chooses `reset_by_*_id(entry)` versus `reset_by_id(row)`; "Remove everything" calls `delete_all()` and shows "History deleted" on success
  - Empty states: "Nothing read recently" / "Nothing watched recently", "No results found" while searching, and "No recent updates"
  - Manual checklist items 1-3 of `p1-edge-cases-and-tests.md` section 6 pass

### T-06a-081 — Wire source browse list and pager of the mockup
- Priority: P0
- Effort: M
- Depends: T-06a-040, T-06a-042, T-06a-043, area:40-ui-browse-manga, area:41-ui-browse-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt
- Acceptance:
  - The browse grid calls `append()` when the last visible index is within 25 items of the end, shows footer spinner and footer error row with Retry, and a full-area error with Retry after a failed refresh
  - Changing query or filters replaces the pager and cancels the in-flight request
  - Duplicate items across pages collapse by (source, url) through the local-row mapping of area:10
  - Source list screens use `sources_with_favorite_count` and show stub sources with their stored name
