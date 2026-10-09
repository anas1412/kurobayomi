# Area 17 — App source layer: implementation tasks

Every scope file (27) is covered by at least one task; see the "Task ids" column of `p1-inventory.md`. Task ids are `T-17a-NNN`. Dependencies on other areas use `area:<id>`; `T-17a-` ids reference this file. Paths are repo-relative to `/home/blackbox/aniyomi-src` (the Kotlin source the port reads), except where a path starts with `kuro`, `src/`, or `spec/`, which are Kurobayomi paths.

Priority legend: P0 needed for daily reading and watching; P1 parity; P2 polish; P3 dropped or replaced (the task states the replacement).

### T-17a-001 — Domain source types (SourceEntry, StubSource, SourceKind)
- Priority: P0
- Effort: S
- Depends: area:09-domain-source-track-download-release-misc
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt
- Acceptance:
  - `kuro-domain::source::model` defines `SourceEntry { id: i64, lang, name, supports_latest, is_stub, pin: PinFlags, is_used_last, data_saver_excluded }` with `Default` for pin and flags, and `StubSource { id, lang, name }` deriving `PartialEq, Eq, Hash`.
  - `SourceEntry::key()` returns `"<id>"` or `"<id>-lastused"`; `visual_name()` returns the name alone when lang is empty, else `"Name (LANG)"` with uppercase lang.
  - `StubSource::is_invalid()` is true for whitespace-only name or lang; `display()` returns the decimal id when invalid.

### T-17a-002 — PinFlags bit model
- Priority: P0
- Effort: S
- Depends: T-17a-001
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt
- Acceptance:
  - `PinFlags::PINNED == 0b11`, `UNPINNED == 0b00`, `PINNED_BIT == 0b01`, `ACTUAL_BIT == 0b10`.
  - `contains(0)` is true for every value (matches Kotlin `pin.code and code == pin.code`).
  - `PINNED.without(ACTUAL_BIT).bits() == 0b01`; `UNPINNED.without(ACTUAL_BIT).bits() == 0b00` (not `0b10`).
  - Unit test covers all four values.

### T-17a-003 — Manga stub store (sources table)
- Priority: P0
- Effort: S
- Depends: area:03-db-manga-schema-migrations, T-17a-001
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaStubSourceRepositoryImpl.kt, data/src/main/sqldelight/data/sources.sq, data/src/main/sqldelight/migrations/16.sqm
- Acceptance:
  - `SqliteStubStore` (table `sources`) implements `all`, `find`, `upsert` with the exact SQL in `p1-rust-design.md` section 4.
  - `upsert` on an existing id updates `lang` and `name` and keeps `_id` (verified by a test that upserts twice).
  - `find` on a missing id returns `None`, not an error.
  - Writes bump a `watch` counter used by the registry.

### T-17a-004 — Anime stub store (animesources table)
- Priority: P0
- Effort: S
- Depends: area:04-db-anime-schema-migrations, T-17a-003
- Sources: data/src/main/java/tachiyomi/data/source/anime/AnimeStubSourceRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/animesources.sq
- Acceptance:
  - Same store as T-17a-003 with table `animesources`; the table name is the only difference (`SourceKind::stub_table`).
  - A test opens the anime DB fixture and round-trips one row.

### T-17a-005 — SourceRegistry core (live map, lookups, lists)
- Priority: P0
- Effort: M
- Depends: T-17a-001, T-17a-002, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - `SourceRegistry` holds `RwLock<Arc<LiveMap>>` and returns `get(id)`, `online_sources()`, `catalogue_sources()` (manga), `all_live()` (anime), `stub_sources()` as in `p1-rust-design.md` 3.1.
  - Lookups never block on I/O; a test with a store that panics on any call passes all lookups after `load_stubs`.
  - `get(999)` on an empty registry returns `None`.

### T-17a-006 — Extension-change loop and readiness
- Priority: P0
- Effort: M
- Depends: T-17a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - On each `InstalledSnapshot` the loop builds the map with the local source first, then extension sources; a later duplicate id replaces an earlier one (test with two extensions both claiming id 30).
  - `ready` is `false` until the first snapshot and stays `true` afterwards (test).
  - Registration tasks are detached: a second snapshot does not abort them (test with a slow store).

### T-17a-007 — register_stub (equality, write, rename)
- Priority: P0
- Effort: S
- Depends: T-17a-003, T-17a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - With `PartialEq` on `StubSource`, registering an unchanged source performs zero writes and zero rename calls (test counts both).
  - A changed name performs one upsert and one `rename_source(old, new)` call; a first-time source performs one upsert and no rename.
  - A failing store logs and does not stop other registrations (test with one failing id and one good id).
  - The `register_lock` serialises two registrations for one id (test: final row equals the later call).

### T-17a-008 — Fallback chain for unknown ids (createStubSource)
- Priority: P0
- Effort: S
- Depends: T-17a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - Order: stored row, then index data (`ExtensionCatalog::index_source`, persisted), then `StubSource::fallback(id)` (cached in memory, not persisted). Three unit tests, one per branch.
  - `on_index_loaded()` upgrades a cached fallback whose lang and name are empty (test S6 of `p1-topic-registry-and-stubs.md`).

### T-17a-009 — Local source identity
- Priority: P0
- Effort: S
- Depends: area:14-source-local-and-metadata, T-17a-005
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt
- Acceptance:
  - Local source has id `0`, lang `"other"`, and names "Local manga source" / "Local anime source" from the string keys `AYMR.strings.local_manga_source` and `AYMR.strings.local_anime_source`.
  - The registry constructs it once and reuses the same `Arc` across snapshots.
  - Included in the enabled list regardless of language settings (test E-N1 in `p1-edge-cases-and-tests.md`).

### T-17a-010 — Source list queries (online, catalogue, all, stub)
- Priority: P0
- Effort: S
- Depends: T-17a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - Manga `online_sources()` returns only HTTP sources; `catalogue_sources()` returns catalogue sources; anime `all_live()` returns every live source including local.
  - `stub_sources()` excludes live ids (manga: HTTP ids; anime: all live ids), per `p1-behavior.md` 1.4.

### T-17a-011 — SourceGate readiness watch
- Priority: P0
- Effort: S
- Depends: T-17a-006, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - `kuro-core::sources::SourceGate` exposes `ready_watch()` returning `watch::Receiver<bool>` starting at `false`.
  - A UI test sees the spinner page until the first snapshot and the list afterwards.

### T-17a-012 — SourceIdentity adapter over the source API
- Priority: P0
- Effort: S
- Depends: area:15-source-api-manga, T-17a-005
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt, data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt
- Acceptance:
  - `SourceIdentity` returns `supports_latest` from the live source (manga repository maps `supportsLatest = it.supportsLatest` at `MangaSourceRepositoryImpl.kt:25-29`).
  - The repository sets `is_stub` from the stub type, never from the live type.
  - Test: a live manga source with `supportsLatest = true` yields `supports_latest == true`.

### T-17a-013 — Typed errors instead of casts
- Priority: P0
- Effort: S
- Depends: T-17a-005, area:15-source-api-manga
- Sources: data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt
- Acceptance:
  - `catalogue(id)` returns `SourceError::NotCatalogue(id)` for a stub id and `SourceError::NotInstalled(id)` for an unknown id; no panic (replaces `as CatalogueSource` at `MangaSourceRepositoryImpl.kt:76,81,86`).
  - Calling a stub's fetch method returns `NotInstalled`, not a panic (test per stub type).

### T-17a-014 — Naming rules (visual name, info name, stub display)
- Priority: P0
- Effort: S
- Depends: T-17a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt
- Acceptance:
  - `name_for_info` implements the 3-row table of `p1-behavior.md` 2.3 (test with enabled sets of size 0, 1 and 2, with and without the source's language).
  - Excluded tags `all` and `other` do not count as enabled languages (test).
  - Stub display returns the id for `""` names and `""` langs (test S7, S8).

### T-17a-015 — SourcePreferences typed core keys
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Typed accessors exist for every active key in `p1-data-and-config.md` section 1 with the stated default (test reads an empty store and checks each default).
  - Set-valued keys store decimal id strings; a test round-trips `{4294967296, 7}`.
  - Enum keys store the enum name; an unknown stored name yields the default.
  - App-state keys use the `__APP_STATE_` prefix.

### T-17a-016 — Enabled-source pipeline (manga and anime)
- Priority: P0
- Effort: M
- Depends: T-17a-015, T-17a-002
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt
- Acceptance:
  - `enabled_sources` is a pure function and reproduces vectors E-N1 to E-N5 and the main vector in `p1-edge-cases-and-tests.md` section 2.
  - Output is recomputed on any settings change and published only when it differs from the last output (test counts publications).
  - Manga rows carry `data_saver_excluded`; anime rows never do.

### T-17a-017 — Language grouping and ordering
- Priority: P0
- Effort: M
- Depends: T-17a-015, T-17a-045
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - Only HTTP sources are grouped; within a group disabled sources come last, then case-insensitive name.
  - Groups: enabled languages first, then `"all"` first, then localized display name; ties broken by raw code.
  - Test: the example in `p1-topic-enable-pin-language-incognito.md` section 3 gives `en, ja, fr` order.

### T-17a-018 — Default enabled languages from the system locale
- Priority: P0
- Effort: S
- Depends: T-17a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - `ja_JP.UTF-8` gives `{all, en, ja}`; `C` or unset gives `{all, en}`; `pt_BR` gives `{all, en, pt}`.
  - The default is used only while the key is unset.

### T-17a-019 — Source enable/disable flips, bulk set, pins
- Priority: P0
- Effort: S
- Depends: T-17a-015
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSource.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt, app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSourcePin.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSourcePin.kt
- Acceptance:
  - `flip_disabled`, `set_disabled(ids, disable)` and `toggle_pin` each perform one persisted write and return the new membership.
  - Flipping twice restores the original set (property test).
  - The naming trap is removed: no helper named `isEnabled` returns "is disabled".

### T-17a-020 — Language toggle
- Priority: P0
- Effort: S
- Depends: T-17a-015
- Sources: app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt
- Acceptance:
  - Adding and removing a language does not change any per-source disabled or pinned set (test).
  - Toggling `"en"` twice restores the original `source_languages`.

### T-17a-021 — Incognito state and toggles
- Priority: P1
- Effort: S
- Depends: T-17a-015, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaIncognitoState.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeIncognitoState.kt, app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaIncognito.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeIncognito.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - `is_incognito(global, pkg, set)` truth table: global true gives true; null package gives false unless global; package in set gives true.
  - Stub and local sources are incognito only through the global flag (test).
  - Toggle persists a package name, and the parameter is named `pkg_name` in Rust.

### T-17a-022 — Data-saver exclusion toggle and enum
- Priority: P1
- Effort: S
- Depends: T-17a-015
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleExcludeFromMangaDataSaver.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `toggle_data_saver_excluded` flips membership in `data_saver_excluded`.
  - `DataSaver` enum has exactly `NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT`; default `NONE`; stored by name.
  - The enabled list marks `data_saver_excluded` rows (T-17a-016).

### T-17a-023 — Migration sort settings and toggles
- Priority: P1
- Effort: M
- Depends: T-17a-015
- Sources: app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceScreenModel.kt
- Acceptance:
  - `Mode { ALPHABETICAL, TOTAL }` and `Direction { ASCENDING, DESCENDING }` with defaults `ALPHABETICAL` and `ASCENDING`.
  - Mode toggle keeps the direction and direction toggle keeps the mode (test both).
  - `set_migration_sorting` writes mode then direction (order test with a recording store).

### T-17a-024 — Migration source list with favourite counts
- Priority: P1
- Effort: M
- Depends: T-17a-016, T-17a-045, area:03-db-manga-schema-migrations
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt, data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
  - Query `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source ORDER BY source` (the `ORDER BY` is a deliberate addition, see `p1-data-and-config.md` 3.1).
  - Local id 0 is removed before sorting (test).
  - Output matches the four vectors in `p1-topic-migration-sorting-and-favorites.md` section 6.

### T-17a-025 — Extension icon for a source
- Priority: P2
- Effort: S
- Depends: T-17a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt
- Acceptance:
  - `icon_for_source(id) -> Option<gdk::Texture>` loads the package icon, caches per package name, and returns `None` for local, stub and missing packages (test F7).
  - Fallback icon name `application-x-addon-symbolic` is used by the UI when `None`.

### T-17a-026 — Source preference namespace key
- Priority: P1
- Effort: S
- Depends: T-17a-001, area:09-domain-source-track-download-release-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt
- Acceptance:
  - `preference_key(id) == format!("source_{id}")` (test for id `-1` and `1234567890123`).
  - Backup export of source preferences uses the same key string (cross-check with area 02 backup task).

### T-17a-027 — Episode refresh orchestration
- Priority: P0
- Effort: L
- Depends: T-17a-028, T-17a-030, T-17a-032, area:07-domain-entries-items, area:05-data-repos-entries-items, area:15-source-api-manga
- Sources: app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Order of operations: read episodes ordered by source order, call the source, write metadata, run episode sync with the ORIGINAL anime object, re-read the anime, return the result (test records the call order).
  - `NoEpisodesException` for an empty non-local list becomes `Result` failure (test E-S1); local source succeeds with empty list (E-S2).
  - Stub source fails with `NotInstalled` and performs no DB write (E-S3).

### T-17a-028 — Cover and background cache rules and timestamps
- Priority: P0
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeBackgroundCache.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `cover_last_modified` and `background_last_modified` implement the 5-row decision table in `p1-topic-remote-entry-update.md` 5.2 and 5.3 (one test per row, vectors E-R1 to E-R7).
  - Timestamps are milliseconds since epoch.
  - A null local URL does not panic and deletes nothing.

### T-17a-029 — Season refresh
- Priority: P1
- Effort: M
- Depends: T-17a-027, area:07-domain-entries-items
- Sources: app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt, app/src/main/java/mihon/domain/source/models/RemoteAnimeSeasonUpdate.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt
- Acceptance:
  - Seasons are read ordered by season source order, then the same metadata write, then `SyncSeasonsWithSource`.
  - Empty non-local season list fails with `NoSeasonsException` (test E-S6).

### T-17a-030 — Error and cancellation semantics for refreshes
- Priority: P0
- Effort: S
- Depends: T-17a-032
- Sources: app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt
- Acceptance:
  - `RemoteError` has `Source`, `Db`, `Fetch`, `Cancelled`; cancellation is not logged as an error and is not returned as a generic failure (test E-S5).
  - A `false` from `update_anime` becomes `RemoteError::Db` and stops the episode sync (test E-S4; documented difference from Kotlin).

### T-17a-031 — Fetch flags and fetch window pass-through
- Priority: P0
- Effort: S
- Depends: T-17a-027
- Sources: app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - `fetch_details`, `fetch_episodes`, `manual_fetch` and `fetch_window: (i64, i64)` (default `(0, 0)`) reach the source and the sync unchanged (test with a recording fake).
  - `manual_fetch = true` forces the cover and background timestamps to be rewritten (E-R3).

### T-17a-032 — Remote update result models
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: app/src/main/java/mihon/domain/source/models/RemoteAnimeEpisodeUpdate.kt, app/src/main/java/mihon/domain/source/models/RemoteAnimeSeasonUpdate.kt
- Acceptance:
  - `RemoteEpisodeUpdate { anime, new_episodes }` and `RemoteSeasonUpdate { anime, new_seasons }` with non-null fields, matching the Kotlin data classes.

### T-17a-033 — Torrent source check
- Priority: P2
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent, T-17a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt
- Acceptance:
  - `is_source_for_torrents` returns false for `None`, local and stub sources, and returns the owning extension's `is_torrent` flag otherwise.
  - A missing owner returns `false` and does not panic (test F6).

### T-17a-034 — Stub list shows all persisted stubs
- Priority: P1
- Effort: S
- Depends: T-17a-004, T-17a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - With five stored stubs and none touched in the session, `stub_sources()` returns five (Kotlin returns zero; the fix is documented in `p1-topic-registry-and-stubs.md` section 4).
  - Live ids are still excluded.

### T-17a-035 — Last-used source duplicate
- Priority: P0
- Effort: S
- Depends: T-17a-016
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - The duplicate row follows its source immediately and carries `is_used_last = true` (test vector row 2 of section 2).
  - A pinned last-used source keeps the Pinned bit on the duplicate, so its menu reads Unpin.
  - `last_*_catalogue_source` defaults to `-1`; a value that is not installed produces no duplicate (E-N3).

### T-17a-036 — Start-up load order and warm-up
- Priority: P0
- Effort: S
- Depends: T-17a-003, T-17a-004, T-17a-006, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - `SourceSystem::start` loads both stub tables before the first live snapshot is applied (test records order).
  - The window between app start and readiness is logged once at info level.
  - Test E-B1, E-B2, E-B3 pass.

### T-17a-037 — Memory-only get_or_stub on the UI thread
- Priority: P0
- Effort: S
- Depends: T-17a-005, T-17a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - `get_or_stub` performs no I/O: test E-C6 (10,000 unknown ids in under 50 ms total on the test machine).
  - No `block_on` appears in the `kuro-sources` crate (CI grep check).

### T-17a-038 — Fakes and fixtures for source tests
- Priority: P0
- Effort: S
- Depends: T-17a-005, T-17a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - In-memory `StubStore`, fake `ExtensionCatalog`, fake `SourceRenamer`, fake `AnimeSourceLike` exist (section 8 of `p1-edge-cases-and-tests.md`).
  - Fixture rows of sections 2 and 3 load from `fixtures/sources/manga_rows.json`.

### T-17a-039 — Drop the commented SY source keys
- Priority: P3
- Effort: S
- Depends: T-17a-015
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - No Rust code or key constant for `eh_enable_source_blacklist`, `sources_tab_categories`, `sources_tab_categories_filter`, `sources_tab_source_categories`.
  - The decision is recorded in `p1-data-and-config.md` section 1 (dropped: no readers in Aniyomi; the feature was a SY-only fork feature).

### T-17a-040 — Drop Compose image types
- Priority: P3
- Effort: S
- Depends: T-17a-025
- Sources: app/src/main/java/eu/kanade/domain/source/anime/model/AnimeSource.kt, app/src/main/java/eu/kanade/domain/source/manga/model/MangaSource.kt
- Acceptance:
  - No Rust type mirrors `ImageBitmap`; icons are `gdk::Texture` only (T-17a-025).
  - Grep check: no `ImageBitmap` or `Drawable` in `kuro-*` crates.

### T-17a-041 — Replace Injekt and Context wiring with constructor injection
- Priority: P3
- Effort: S
- Depends: area:01-foundation-app-bootstrap, T-17a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `SourceSystem::new` takes its dependencies as arguments; no service locator.
  - The download renamer is attached through `set_renamer` after construction (the `OnceLock` cycle breaker); a test covers rename before and after attach.

### T-17a-042 — Tests for the enabled pipeline
- Priority: P0
- Effort: S
- Depends: T-17a-016
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt
- Acceptance:
  - Tests E-N1 to E-N5 and the main vector of `p1-edge-cases-and-tests.md` section 2 pass for both manga and anime.

### T-17a-043 — Tests for remote update rules
- Priority: P0
- Effort: S
- Depends: T-17a-028, T-17a-031
- Sources: app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt
- Acceptance:
  - Tests E-R1 to E-R7, E-T1 to E-T4, E-U1 to E-U10 pass.

### T-17a-044 — Drop runBlocking and Dispatchers in favour of tokio
- Priority: P3
- Effort: S
- Depends: T-17a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - No `runBlocking`, `Dispatchers` or `CoroutineScope` equivalents remain in Rust; the replacement is tokio tasks and `spawn_blocking` (`p1-rust-design.md` section 11).

### T-17a-045 — Locale services: display names and collator
- Priority: P0
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt
- Acceptance:
  - `display_name("ja")` gives a localized name in the UI locale with the first letter uppercased; `zh-CN` maps to `zh-Hans`, `zh-TW` to `zh-Hant`.
  - Collator at primary strength orders `"alpha" < "Beta"` and treats `"a"` and `"A"` as equal (test).
  - Unknown tags return the raw code instead of failing.

### T-17a-046 — Migration comparator tests
- Priority: P1
- Effort: S
- Depends: T-17a-024
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt
- Acceptance:
  - Tests E-M1 to E-M6 pass, including the tie-order rule for DESCENDING (no list reversal).

### T-17a-047 — Per-anime refresh mutex
- Priority: P1
- Effort: S
- Depends: T-17a-027
- Sources: app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt
- Acceptance:
  - Two concurrent refreshes of one anime serialise; test E-C4 shows no duplicate episode rows.

### T-17a-048 — Log orphaned download folders at start
- Priority: P2
- Effort: S
- Depends: T-17a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - At start, download folders that match no current source name are logged at info level with their names (test with a temp dir fixture).
  - Nothing is deleted.

### T-17a-049 — Language grouping tests
- Priority: P0
- Effort: S
- Depends: T-17a-017
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt
- Acceptance:
  - Tests cover: enabled-first order; disabled sources last inside a group; two codes with identical display names both appear (tie-break, F10); a group whose sources are all disabled still appears.

### T-17a-050 — Default language and environment parse tests
- Priority: P1
- Effort: S
- Depends: T-17a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - Vectors: `ja_JP.UTF-8`, `pt_BR@euro`, `C`, `POSIX`, empty string, `zh-Hant_TW`: each gives the expected set.
