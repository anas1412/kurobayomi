# Area 05 — implementation tasks (part 1 of 1)

Scope: `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt`, category repositories, entry repositories and mappers (manga, anime, relations), chapter/episode repositories and sanitizers. Rust modules named in the tasks are in `p1-rust-design.md` section 1. Test names are in `p1-edge-cases-and-tests.md` section 3; edge-case IDs `EC-nn` are in the same file.

### T-05a-001 — Database open, pragmas, reader/writer connections
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt
- Acceptance:
  - `Db::open(paths, DbKind::Manga)` creates or opens `tachiyomi.db` and `DbKind::Anime` opens `tachiyomi.animedb` in the data directory from the `Paths` trait, running area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations migrations first.
  - On both connections `PRAGMA foreign_keys` reports 1, `journal_mode` reports `wal`, `synchronous` reports 1 (NORMAL), `recursive_triggers` reports 0, and `busy_timeout` reports 5000.
  - `Db::write` runs the closure in one transaction on the writer connection and rolls back on error; `Db::read` uses the reader connection.
  - Opening a database whose `user_version` is greater than the app's known schema version fails with a clear `DataError` (EC-33).
  - `Db::open_in_memory` yields the same schema, pragmas and functions for tests.

### T-05a-002 — Table-change notifier and Subscription type
- Priority: P0
- Effort: L
- Depends: T-05a-001
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt
- Acceptance:
  - `Db::subscribe(tables, debounce, query)` emits the current result first, then re-runs only when a committed write touches an intersecting table; rollbacks emit nothing.
  - Writes made by triggers and by foreign-key cascades are reported (hook-based, with the per-method superset list as fallback); deleting a manga with chapters re-emits a chapter-list subscription (EC-24).
  - Consecutive notifications inside the debounce window cause one re-query; an unchanged result (`PartialEq`) is not re-sent.
  - Dropping the `Subscription` stops its task; no thread or task leak after 1 000 create/drop cycles.
  - `subscribe_by_id` style subscriptions yield `None` when the row is deleted (EC-01).

### T-05a-003 — Column adapters (DatabaseAdapter.kt)
- Priority: P0
- Effort: S
- Depends: none
- Sources: data/src/main/java/tachiyomi/data/DatabaseAdapter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt
- Acceptance:
  - `decode_string_list("")` is empty and otherwise splits on the two-character separator `", "`; `encode_string_list` joins with it; a genre containing `", "` splits on read (EC-19).
  - `UpdateStrategy`, `AnimeUpdateStrategy` decode 0 and 1 to `ALWAYS_UPDATE` and `ONLY_FETCH_ONCE` and every other value (including negatives) to `ALWAYS_UPDATE`; `FetchType` decodes 0 to `Seasons`, 1 to `Episodes`, every other value to `Episodes`.
  - `memo` encodes as compact JSON bytes preserving key order and decodes invalid JSON or non-object JSON to `{}` with a warning (EC-18).
  - Date values are plain `i64` epoch milliseconds (no newtype conversion needed); unit tests cover every codec.

### T-05a-004 — Manga row structs and MangaMapper
- Priority: P0
- Effort: M
- Depends: T-05a-003, area:07-domain-entries-items
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt, domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt
- Acceptance:
  - `Manga`, `MangaUpdate` (all `Option`, `Default`), `LibraryManga` exist with the exact field sets of `p1-data-and-config.md` section 2.1 and `Manga::create()` defaults (id -1, source -1, version 0, update strategy `ALWAYS_UPDATE`).
  - `map_manga` reads by column name; `NULL` `last_update`/`next_update` map to 0; `fetch_interval` is `calculate_interval as i32`; `is_syncing` is not exposed.
  - `map_library_manga` reads the 7 aggregate columns as `f64` then `as i64`; `unread_count`, `has_bookmarks`, `has_started` behave as in Kotlin.
  - A test fails if the number of columns of `libraryView` is not 31.

### T-05a-005 — MangaRepo read methods
- Priority: P0
- Effort: M
- Depends: T-05a-001, T-05a-004
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
  - `by_id` returns `NotFound` for a missing id; `by_url_and_source` returns the first match (`LIMIT 1`); `favorites`, `read_not_in_library`, `subscribe_favorites_by_source` use the exact SQL of `p1-data-and-config.md` section 5.1.
  - `read_not_in_library` returns non-favorites with a chapter `read = 1` or `last_page_read != 0` and nothing else.
  - `subscribe_by_id` and `subscribe_by_url_and_source` emit `Option<Manga>` and re-emit after an update.
  - All tests of the "Manga repo" group in `p1-edge-cases-and-tests.md` that concern reads pass.

### T-05a-006 — MangaRepo write methods (insert, partial update, categories, reset)
- Priority: P0
- Effort: M
- Depends: T-05a-001, T-05a-004
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/mangas_categories.sq
- Acceptance:
  - `insert` binds the 21 columns of the Kotlin statement by name, writes `last_modified_at = 0`, ignores the input id, returns `last_insert_rowid()` from the same transaction.
  - `update`/`update_all` execute the static `coalesce` statement in one transaction with `is_syncing = 0`; `None` keeps the column, nullable columns cannot be cleared (EC-27 for missing ids returns Ok).
  - `set_categories` deletes old links then inserts links in list order in one transaction; a nonexistent category id rolls everything back and leaves old links intact (EC-22); version increases by the number of inserted links (EC-06).
  - `reset_viewer_flags` sets `viewer = 0` on all rows.

### T-05a-007 — Manga library view query and grouping helper
- Priority: P0
- Effort: M
- Depends: T-05a-002, T-05a-005
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/sqldelight/view/libraryView.sq, domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt
- Acceptance:
  - `library()` returns one `LibraryManga` per (favorite manga, category link) and one row with `category = 0` for a manga with no links (EC-13).
  - Counts exclude chapters of excluded scanlators; a manga with no chapters has zero for every aggregate; `last_read` comes from the `history` table.
  - `subscribe_library()` debounces 100 ms and depends on `mangas, chapters, mangas_categories, excluded_scanlators, history`.
  - `library_by_category(rows)` helper returns rows grouped by category id.
  - Query on a 5 000-manga / 200 000-chapter generated fixture completes in under 500 ms on the reference machine (measured in a `#[ignore]` benchmark test).

### T-05a-008 — ChapterRepo
- Priority: P0
- Effort: M
- Depends: T-05a-001, T-05a-003
- Sources: data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt, data/src/main/sqldelight/data/chapters.sq, domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt, domain/src/main/java/tachiyomi/domain/items/chapter/repository/ChapterRepository.kt
- Acceptance:
  - `add_all` inserts in order inside one transaction, returns chapters with new ids in the same order, and leaves the table unchanged when any element fails (EC-07).
  - `update`/`update_all` use the coalesce statement with `is_syncing = 0`; `remove_with_ids` chunks 500 ids in one transaction (EC-17, 1 200 ids test).
  - `by_manga_id(id, apply_filter)` and `subscribe_by_manga_id` implement the `LEFT JOIN excluded_scanlators` filter exactly; `scanlators` is not distinct and maps NULL to `""`; `bookmarked` returns bookmarked only.
  - `by_url_and_manga_id` with duplicate rows returns the smallest `_id` and logs a warning (EC-09); `Chapter::create()` has version 1, date_upload -1, chapter_number -1.0.

### T-05a-009 — Item name sanitizer (ChapterSanitizer)
- Priority: P0
- Effort: S
- Depends: none
- Sources: data/src/main/java/tachiyomi/data/items/chapter/ChapterSanitizer.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt
- Acceptance:
  - `sanitize_item_name(name, title)` implements trim (Kotlin whitespace set including U+001C-U+001F, excluding U+0085), single case-sensitive `strip_prefix(title)`, then `trim_matches` over the 29-character set (whitespace list plus `-` `_` `,` `:`).
  - Every vector in the table of `p1-topic-item-sanitizer.md` passes, including the empty result when name equals title and the U+001F case.
  - The function is pure, allocation-light and has a documented constant array for the trim set.

### T-05a-010 — CategoryRepo (manga and anime kinds)
- Priority: P0
- Effort: M
- Depends: T-05a-001, T-05a-002
- Sources: data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt, data/src/main/sqldelight/data/categories.sq, data/src/main/sqldelightanime/dataanime/categories.sq, domain/src/main/java/tachiyomi/domain/category/model/Category.kt
- Acceptance:
  - All 14 repository operations exist for both kinds (`get`, `all`, `all_visible`, their subscriptions, `by_entry`, `visible_by_entry` and subscriptions, `insert`, `update_partial`, `update_partials`, `update_all_flags`, `delete`) with the link tables `mangas_categories` and `animes_categories` selected by kind.
  - `all()` includes the system row (id 0, name "", order -1) first; ordering is `ORDER BY sort, _id` (EC-10); `all_visible()` excludes hidden rows.
  - `insert` ignores the passed id and hidden, returns the new id; `update_partial` applies coalesce including `hidden` as 1/0; `update_partials` is atomic; `update_all_flags(None)` is a no-op; deleting id 0 fails with "System category can't be deleted" (EC-23); `hidden` values other than 1 read as visible (EC-30).
  - Deleting a category cascades its link rows.

### T-05a-011 — Anime row structs and AnimeMapper (five mappers)
- Priority: P0
- Effort: M
- Depends: T-05a-003, area:07-domain-entries-items
- Sources: data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt, domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt, domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/DeletableAnime.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeRelationGroup.kt
- Acceptance:
  - `Anime` (32 fields), `AnimeUpdate`, `LibraryAnime`, `SeasonAnime`, `DeletableAnime`, `AnimeRelationGroup` exist with the exact field sets and `Anime::create()` defaults (`fetch_type = Episodes`, `season_number = -1.0`, `memo = {}`).
  - `map_anime`, `map_library_anime`, `map_season_anime`, `map_related_anime`, `map_deletable_anime` read by column name; `episode_flags` fills `episode_flags`; NULL stat columns read as 0 (EC-16).
  - Column-count tests: `animelibView` 40, `animeseasonsView` 39, related query 33.

### T-05a-012 — AnimeRepo reads and simple writes
- Priority: P0
- Effort: M
- Depends: T-05a-001, T-05a-011
- Sources: data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/dataanime/animes_categories.sq
- Acceptance:
  - `by_id`, `subscribe_by_id`, `by_url_and_source`, `subscribe_by_url_and_source`, `favorites`, `watched_not_in_library` (`seen = 1 OR last_second_seen != 0`), `subscribe_favorites_by_source`, `set_categories`, `reset_viewer_flags` behave like their manga counterparts using `animes` and `animes_categories`.
  - `reset_viewer_flags` exists in the repository but is not wired to any UI (EC-29).
  - Tests mirror the manga read tests.

### T-05a-013 — AnimeRepo insert and 29-column partial update
- Priority: P0
- Effort: M
- Depends: T-05a-001, T-05a-011
- Sources: data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - `insert` writes all 29 columns of the Kotlin statement including `fetch_type`, `parent_id`, `season_flags`, `season_number`, `season_source_order`, `background_url`, `background_last_modified`, `memo`, and returns the new id; a full round trip equals the input except `id`, `last_modified_at`, `favorite_modified_at`.
  - `update`/`update_all` use the 29-column coalesce statement (29 `coalesce` values plus `:animeId`) in one transaction with `is_syncing = 0`; `parent_id` can be set but not cleared; `memo` is encoded only when provided.
  - Updating only `title` still refreshes `favorite_modified_at` (EC-04).

### T-05a-014 — Anime library view and season stats query
- Priority: P0
- Effort: M
- Depends: T-05a-002, T-05a-012
- Sources: data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/sqldelightanime/view/animelibView.sq, data/src/main/sqldelightanime/view/animeseasonstatsView.sq, data/src/main/sqldelightanime/view/episodestatsView.sq, data/src/main/sqldelightanime/view/animehistorystatsView.sq
- Acceptance:
  - `library()` and `subscribe_library()` return per-category rows with stats switched by `fetch_type` (episode stats for 1, season aggregates for 0) exactly as in `p1-topic-library-and-season-views.md` section 2.
  - A Seasons parent with children whose totals/seen are {0/0, 5/5, 5/3} reports `total_count = 3`, `seen_count = 2` (EC-15); bookmark and fillermark counts sum over children.
  - Subscription depends on `animes, episodes, animehistory, animes_categories` and debounces 100 ms.

### T-05a-015 — EpisodeRepo
- Priority: P0
- Effort: M
- Depends: T-05a-001, T-05a-003
- Sources: data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/episodes.sq, domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt, domain/src/main/java/tachiyomi/domain/items/episode/model/EpisodeUpdate.kt, domain/src/main/java/tachiyomi/domain/items/episode/repository/EpisodeRepository.kt
- Acceptance:
  - `add_all` binds the 17 columns (including `summary`, `preview_url`, `fillermark`, `memo`, `total_seconds`) atomically with ids in order; failure leaves the table unchanged.
  - `update`/`update_all` pass `summary`, `previewUrl`, `fillermark`, `memo` through coalesce; `remove_with_ids` chunks; `by_anime_id` has no scanlator filter; `subscribe_by_anime_id`, `bookmarked`, `by_id`, `by_url_and_anime_id` work.
  - Corrupt `memo` bytes do not fail the list query (EC-18); `Episode::create()` has version 1 and date_upload -1.

### T-05a-016 — Episode name sanitizer entry point
- Priority: P0
- Effort: S
- Depends: T-05a-009
- Sources: data/src/main/java/tachiyomi/data/items/episode/EpisodeSanitizer.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Episode sync code calls the same `sanitize_item_name` with the anime title; a test asserts the chapter and episode trim sets are identical (the two Kotlin arrays have the same 30 entries).
  - Vectors from `p1-topic-item-sanitizer.md` run through the episode wrapper as well.

### T-05a-017 — Season (parent/child) repository operations
- Priority: P1
- Effort: M
- Depends: T-05a-012, T-05a-013, T-05a-014
- Sources: data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/sqldelightanime/view/animeseasonsView.sq, data/src/main/sqldelightanime/view/animedeletableView.sq, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt
- Acceptance:
  - `seasons_by_parent`, `subscribe_seasons_by_parent`, `children_by_parent`, `remove_parent_ids` (chunked, transactional), `subscribe_deletable_parents` match `p1-topic-seasons-and-relations.md`; `animeseasonsView` includes non-favorite children, `animedeletableView` only non-favorite top-level rows.
  - Deleting a parent leaves children's `parent_id` dangling (no FK) and `remove_parent_ids` nulls them.
  - Subscriptions re-emit after episode, history or anime changes.

### T-05a-018 — AnimeRelationRepo
- Priority: P2
- Effort: M
- Depends: T-05a-001, T-05a-011
- Sources: data/src/main/java/tachiyomi/data/entries/anime/AnimeRelationRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/anime_relations.sq, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt
- Acceptance:
  - `replace_relations` deletes the anime's rows and inserts every `(group, related id)` pair with a running `sort_order` starting at 0 using `INSERT OR REPLACE`, all in one transaction with the given `fetched_at`.
  - `last_fetched_at` returns None for an anime with no rows; `subscribe_related` returns groups in order of first member by `sort_order`, grouping by name.
  - Deleting either anime cascades its relation rows.

### T-05a-019 — Unicode-correct duplicate title lookup
- Priority: P1
- Effort: S
- Depends: T-05a-005, T-05a-012
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetDuplicateLibraryManga.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetDuplicateLibraryAnime.kt
- Acceptance:
  - A scalar function `lower_unicode` is registered on every connection; `duplicates(id, title)` compares `lower_unicode(title)` with `title.to_lowercase()` and still requires `favorite = 1` and `_id != id`.
  - "Éclair" finds "éCLAIR"; ASCII titles behave as before; non-favorites and the entry itself are excluded (EC-02).

### T-05a-020 — Upcoming queries with local start of day
- Priority: P1
- Effort: S
- Depends: T-05a-005, T-05a-012, T-05a-030
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt, domain/src/main/java/mihon/domain/upcoming/anime/interactor/GetUpcomingAnime.kt
- Acceptance:
  - `subscribe_upcoming(statuses)` selects `next_update >= startOfDay AND favorite = 1 AND status IN (...) ORDER BY next_update ASC`, with manga statuses {1, 4} and anime {1, 4, 7} supplied by the caller.
  - The cut-off is recomputed on every re-query from an injectable clock and a midnight refresh fires (EC-03); results are sorted ascending.

### T-05a-021 — Excluded scanlators: filter and set/get
- Priority: P1
- Effort: S
- Depends: T-05a-008
- Sources: data/src/main/sqldelight/data/excluded_scanlators.sq, app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/GetExcludedScanlators.kt, data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt
- Acceptance:
  - `excluded_scanlators(manga_id)` and `set_excluded_scanlators(manga_id, set)` implement the diff algorithm in one transaction (insert only missing pairs, delete removed ones with `scanlator IN (...)`).
  - Chapter list and library counts honour the exclusions; a NULL-scanlator chapter is never excluded; duplicate rows inserted by hand multiply rows when the filter flag is 0 (EC-12 documented test).

### T-05a-022 — GTK bridge for subscriptions
- Priority: P0
- Effort: M
- Depends: T-05a-002
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt, data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt
- Acceptance:
  - `data_bridge::bind(sub, apply)` runs `apply` on the GTK main thread for each emission and returns an abortable handle; aborting stops updates.
  - A helper updates a `gio::ListStore` with `splice` so a `gtk::ListView` keeps scroll position when an emission only changes a few rows.
  - A test drives a `glib::MainContext` manually and receives values in order.

### T-05a-023 — Repository test harness and builders
- Priority: P0
- Effort: M
- Depends: T-05a-001
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt
- Acceptance:
  - `tests/support` offers `Db::open_in_memory`, builders for manga, anime, chapters, episodes, categories, an injectable clock, and sorting helpers.
  - `cargo test -p kuro-core` runs the whole data suite headless in under 10 s.
  - Every public repository method has at least one test (checked by a script that greps the method list).

### T-05a-024 — Trigger regression tests
- Priority: P1
- Effort: S
- Depends: T-05a-006, T-05a-008, T-05a-013, T-05a-015
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/dataanime/episodes.sq, data/src/main/sqldelight/data/mangas_categories.sq, data/src/main/sqldelightanime/dataanime/animes_categories.sq
- Acceptance:
  - Tests assert the version-bump matrix of EC-05, chapter-read bumping both versions, episode `fillermark` not bumping, category-link insert bumping the parent, `favorite_modified_at` set on any update (EC-04), `last_modified_at` in seconds (EC-25).
  - A trigger or schema change in area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations that alters these results fails the suite.

### T-05a-025 — Category rules: create order, delete renumbering, reorder, exclude cleanup
- Priority: P1
- Effort: M
- Depends: T-05a-010, area:08-domain-library-category-history-updates, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/category/manga/interactor/CreateMangaCategoryWithName.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/ReorderMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetSortModeForMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/ResetMangaCategoryFlags.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt
- Acceptance:
  - Create uses `next_order = max(order) + 1` over all categories including the system row (0 when only the system row exists) and initial flags from the global sort preference.
  - Delete renumbers all remaining categories by index (system row becomes 0, user rows 1..n), resets the default-category preference when it equals the deleted id, and removes the id from the update-include, update-exclude, remove-exclude, download-new and download-new-exclude sets for both manga and anime (manga exclude-set bug fixed, EC-11).
  - Reorder skips the system row, serialises through a mutex, returns Unchanged for an unknown id and an error for an out-of-range index; the tie scenario of EC-10 passes.
  - Sort-mode update applies mask-based flag arithmetic, regenerates the random seed for Random, and updates one category when categorized display is on, otherwise the global preference and all categories.

### T-05a-026 — Wire the mockup screens to the repositories
- Priority: P0
- Effort: L
- Depends: T-05a-007, T-05a-010, T-05a-014, T-05a-015, T-05a-022, T-05a-008
- Sources: data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt
- Acceptance:
  - `categories()` in `src/screens.rs` lists real categories per kind tab, hides the system category, supports create, rename, hide toggle, delete and drag reorder through the repository with the duplicate-name check from `spec/app.json` (`A category with this name already exists!`) and the empty-state text.
  - The manga and anime library screens render rows from `subscribe_library()` grouped by category, and the entry details screen shows the entry and its chapter or episode list from the repositories; the set-categories dialog persists with `set_categories`.
  - No screen touches `rusqlite` directly; all access goes through `data_bridge::bind`.
  - Starting with an empty database shows the empty states from the spec without errors.

### T-05a-027 — Drop: SQLDelight codegen, Paging sources, coroutine transaction machinery
- Priority: P3
- Effort: S
- Depends: none
- Sources: data/src/main/java/tachiyomi/data/handlers/manga/QueryPagingMangaSource.kt, data/src/main/java/tachiyomi/data/handlers/anime/QueryPagingAnimeSource.kt, data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt, data/src/main/java/tachiyomi/data/handlers/anime/AnimeTransactionContext.kt
- Acceptance:
  - Decision recorded in code docs: no `PagingSource` equivalent; lists load fully and virtualise in `gtk::ListView`; transactions are closures on the connection thread. User loses nothing (library sizes fit in memory).
  - No Rust module named `paging` exists in `kuro-core::data`.

### T-05a-028 — Legacy return-shape adapter for interactors
- Priority: P1
- Effort: S
- Depends: T-05a-006, T-05a-008, T-05a-012, T-05a-013, T-05a-015
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt, data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt
- Acceptance:
  - `data::compat` maps `Err` to the Kotlin shapes: update/reset to `false`, `add_all` to an empty list, remove and `remove_parent_ids` to unit, each logging with `tracing::error!`; all other methods propagate errors.
  - A table-driven test confirms which methods swallow and which propagate, matching `p1-behavior.md` section 0.

### T-05a-029 — Drop: sync-state column `is_syncing` and `resetIsSyncing`
- Priority: P3
- Effort: S
- Depends: none
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelightanime/dataanime/episodes.sq
- Acceptance:
  - Update structs do not expose `is_syncing`; every update statement still writes `is_syncing = 0`; the unused `resetIsSyncing` statements get no Rust method.
  - Decision documented: the SyncYomi-style sync feature that would set the flag is not ported; the user loses cross-device sync (backups remain the transfer mechanism); the columns stay for schema compatibility.

### T-05a-030 — Time helpers (seconds vs milliseconds, local midnight)
- Priority: P0
- Effort: S
- Depends: none
- Sources: data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
  - `now_millis()`, `now_secs()` and `start_of_local_day_millis(clock)` exist; the latter equals Kotlin's `LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000` for fixed test instants in two time zones including a DST-change day.
  - The local UTC offset is cached at startup; failure to read it falls back to UTC with a warning.

### T-05a-031 — Restore and clear-database helper queries
- Priority: P1
- Effort: M
- Depends: T-05a-005, T-05a-012, T-05a-008, T-05a-015
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelightanime/dataanime/episodes.sq, app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt
- Acceptance:
  - `all_source_and_url` (both kinds), `ChapterRepo::by_url`, `EpisodeRepo::by_url`, `MangaRepo::delete_non_library_by_sources` (`favorite = 0 AND source IN`), `AnimeRepo::delete_non_library_by_ids` (`favorite = 0 AND _id IN`) exist and are chunked/transactional.
  - Deleting non-library anime never deletes a favorite even when its id is passed, and cascades episodes, category links and relations.
  - Documented as consumed by area:22-backup-restore and area:47-settings-screens-general.

### T-05a-032 — Compatibility test against real Aniyomi databases
- Priority: P1
- Effort: S
- Depends: T-05a-023, T-05a-007, T-05a-014
- Sources: data/src/main/sqldelight/view/libraryView.sq, data/src/main/sqldelightanime/view/animelibView.sq, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `tests/fixtures/aniyomi_manga.db` and `aniyomi_anime.db` produced by the real app open without migration errors; the library queries return the expected counts for the fixture (3 manga, 3 anime incl. a Seasons parent with 2 children).
  - Round-trip: Rust `update` of one row leaves all other columns byte-identical (checked by dumping rows before and after).
