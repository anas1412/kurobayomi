# Area 17 — App source layer: inventory

Scope: the 27 files printed by `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 17-source-app-layer 1`. Every file was read in full. LOC is the value that command reports (line count). All paths are repo-relative to `/home/blackbox/aniyomi-src`.

Port-decision vocabulary: **Port** = behaviour reproduced in Rust with the same semantics; **Rewrite** = same job, Rust-idiomatic shape (no Kotlin structure kept); **Replace-with-crate** = the Kotlin/Android mechanism is replaced by a named crate or std item; **Drop** = not carried over, reason given.

Target crates (see `p1-rust-design.md`): `kuro-domain` (pure types), `kuro-sources` (registry, stores, interactors, remote update), `kuro-core` (`SourceGate` trait), `kurobayomi` (GTK UI, consumes these).

Priority tags refer to the task file `p1-tasks.md`.

## Summary counts

| Metric | Value |
|---|---|
| Files in scope | 27 |
| Total LOC (sum of areas.py column) | 1,253 |
| Classes (incl. 2 data classes) | 23 |
| Nested enums | 3 (`SetMigrateSorting.Mode`, `SetMigrateSorting.Direction`, `SourcePreferences.DataSaver`) |
| Top-level functions (11) and extension properties (2) | 13 |
| Port decision | 21 rows (rows 1-7, 9-18, 22, 24, 26, 27) |
| Rewrite decision | 4 rows (rows 20, 21, 23, 25) |
| Replace-with-crate decision (icon to GTK texture, Compose type dropped) | 2 rows (rows 8, 19) |
| Drop decisions | none at file level; drops are listed per feature in tasks T-17a-039 to T-17a-041 |

## Table

Columns: path | LOC | every declared name | purpose | port decision | target module | priority | task ids.

| # | Path | LOC | Declared names (complete) | Purpose | Port decision (reason) | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|---|
| 1 | app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeIncognitoState.kt | 33 | class `GetAnimeIncognitoState`; members `await(sourceId: Long?)`, `subscribe(sourceId: Long?)` | Answers "is this anime source (or everything) incognito?" for the UI and history writers | Port: pure rule (global flag OR extension package in set); the Flow form becomes a `watch` value recomputed on settings change | `kuro-sources::incognito` | P1 | T-17a-021 |
| 2 | app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt | 58 | class `GetAnimeSourcesWithFavoriteCount`; members `subscribe()`, private `sortFn(direction, sorting)` | Migration source list for anime, ordered by mode and direction | Port: comparator reproduced exactly (stub-first rule, reverse for DESC) | `kuro-sources::migration` | P1 | T-17a-023, T-17a-024 |
| 3 | app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt | 42 | class `GetEnabledAnimeSources`; member `subscribe()` | Produces the anime browse list: filter by language/disabled, sort by name, pin flags, last-used duplicate | Port: pure function `enabled_sources` | `kuro-sources::enabled` | P0 | T-17a-016, T-17a-035, T-17a-042 |
| 4 | app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt | 34 | class `GetLanguagesWithAnimeSources`; member `subscribe()` | Groups online anime sources by language for the "Language" section of the Extension/Source screens | Port: grouping + sorted map with enabled-first + `LocaleHelper.comparator` | `kuro-sources::languages` | P0 | T-17a-017, T-17a-018, T-17a-049 |
| 5 | app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeIncognito.kt | 14 | class `ToggleAnimeIncognito`; member `await(extensions: String, enable: Boolean)` | Adds/removes an extension package from the anime incognito set | Port (parameter named `extensions` in Kotlin is a package name) | `kuro-sources::incognito` | P1 | T-17a-021 |
| 6 | app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt | 37 | class `ToggleAnimeSource`; members `await(source: AnimeSource, enable)`, `await(sourceId: Long, enable)`, `await(sourceIds: List<Long>, enable: Boolean)`, private `isEnabled(sourceId)` | Enable/disable one or many anime sources (writes the disabled set) | Port: three entry points collapse into one `set_enabled(ids, enable)` plus a flip helper | `kuro-sources::enabled` | P0 | T-17a-019 |
| 7 | app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSourcePin.kt | 17 | class `ToggleAnimeSourcePin`; member `await(source: AnimeSource)` | Pin/unpin an anime source | Port | `kuro-sources::pins` | P0 | T-17a-019 |
| 8 | app/src/main/java/eu/kanade/domain/source/anime/model/AnimeSource.kt | 16 | extension property `AnimeSource.icon: ImageBitmap?` (Compose + Android Drawable) | Source icon for Compose lists | Replace-with-crate: icon resolved from extension package file to a GTK `gdk::Texture` (or freedesktop fallback). Compose `ImageBitmap` dropped. | `kuro-sources::icons` | P2 | T-17a-025, T-17a-040 |
| 9 | app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt | 23 | class `SetMigrateSorting`; member `await(mode: Mode, direction: Direction)`; enum `Mode { ALPHABETICAL, TOTAL }`; enum `Direction { ASCENDING, DESCENDING }` | Stores migration source sort mode and direction | Port: enums verbatim; two writes | `kuro-sources::migration` | P1 | T-17a-023 |
| 10 | app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt | 16 | class `ToggleLanguage`; member `await(language: String)` | Adds/removes a language code from `source_languages` | Port | `kuro-sources::languages` | P0 | T-17a-020 |
| 11 | app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt | 52 | class `GetEnabledMangaSources`; member `subscribe()` | Manga browse list (manga twin of #3) with data-saver exclusion flag | Port (adds `is_excluded_from_data_saver`) | `kuro-sources::enabled` | P0 | T-17a-016, T-17a-035, T-17a-042 |
| 12 | app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt | 34 | class `GetLanguagesWithMangaSources`; member `subscribe()` | Manga twin of #4 | Port | `kuro-sources::languages` | P0 | T-17a-017, T-17a-018, T-17a-049 |
| 13 | app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaIncognitoState.kt | 33 | class `GetMangaIncognitoState`; members `await(sourceId: Long?)`, `subscribe(sourceId: Long?)` | Manga twin of #1 | Port | `kuro-sources::incognito` | P1 | T-17a-021 |
| 14 | app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt | 58 | class `GetMangaSourcesWithFavoriteCount`; members `subscribe()`, private `sortFn(direction, sorting)` | Manga twin of #2 | Port | `kuro-sources::migration` | P1 | T-17a-023, T-17a-024, T-17a-046 |
| 15 | app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleExcludeFromMangaDataSaver.kt | 20 | class `ToggleExcludeFromMangaDataSaver`; member `await(source: Source)` | Flips a manga source in/out of the data-saver exclusion set | Port | `kuro-sources::datasaver` | P1 | T-17a-022 |
| 16 | app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaIncognito.kt | 14 | class `ToggleMangaIncognito`; member `await(extensions: String, enable: Boolean)` | Manga twin of #5 | Port | `kuro-sources::incognito` | P1 | T-17a-021 |
| 17 | app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSource.kt | 37 | class `ToggleMangaSource`; members `await(source: Source, enable)`, `await(sourceId: Long, enable)`, `await(sourceIds: List<Long>, enable: Boolean)`, private `isEnabled(sourceId)` | Manga twin of #6 | Port | `kuro-sources::enabled` | P0 | T-17a-019 |
| 18 | app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSourcePin.kt | 17 | class `ToggleMangaSourcePin`; member `await(source: Source)` | Manga twin of #7 | Port | `kuro-sources::pins` | P0 | T-17a-019 |
| 19 | app/src/main/java/eu/kanade/domain/source/manga/model/MangaSource.kt | 16 | extension property `Source.icon: ImageBitmap?` | Manga twin of #8 | Replace-with-crate (GTK texture); Compose dropped | `kuro-sources::icons` | P2 | T-17a-025, T-17a-040 |
| 20 | app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt | 125 | class `SourcePreferences`; accessor functions (each returns a `Preference<T>`): `sourceDisplayMode`, `enabledLanguages`, `showNsfwSource`, `migrationSortingMode`, `migrationSortingDirection`, `animeExtensionRepos`, `mangaExtensionRepos`, `trustedExtensions`, `globalSearchFilterState`, `disabledAnimeSources`, `disabledMangaSources`, `incognitoAnimeExtensions`, `incognitoMangaExtensions`, `pinnedAnimeSources`, `pinnedMangaSources`, `lastUsedAnimeSource`, `lastUsedMangaSource`, `animeExtensionUpdatesCount`, `mangaExtensionUpdatesCount`, `hideInAnimeLibraryItems`, `hideInMangaLibraryItems`, `dataSaver`, `dataSaverIgnoreJpeg`, `dataSaverIgnoreGif`, `dataSaverImageQuality`, `dataSaverImageFormatJpeg`, `dataSaverServer`, `dataSaverColorBW`, `dataSaverExcludedSources`, `dataSaverDownloader`; nested `enum DataSaver { NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT }`; commented-out SY block (`enableSourceBlacklist`, `sourcesTabCategories`, `sourcesTabCategoriesFilter`, `sourcesTabSourcesInCategories`) | Typed view over the preference store for every source-level setting | Rewrite as a typed struct over the settings store (area:02) | `kuro-sources::prefs` | P0 (core keys), P1 (data saver, migration, repos), P3 (commented SY keys dropped) | T-17a-015, T-17a-022, T-17a-023, T-17a-039 |
| 21 | app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt | 127 | class `AndroidAnimeSourceManager(context, extensionManager, sourceRepository) : AnimeSourceManager`; private `_isInitialized`, `downloadManager` (injectLazy), `scope`, `sourcesMapFlow`, `stubSourcesMap`; overrides `isInitialized`, `sources`, `get`, `getOrStub`, `getAll`, `getOnlineSources`, `getStubSources`; private `registerStubSource(source)`, `createStubSource(id)` | Anime live-source registry, stub registry, extension-to-source mapping | Rewrite as generic `SourceRegistry<Anime>`; fixes the stub-map quirk (see behavior) | `kuro-sources::registry` | P0 | T-17a-005, T-17a-006, T-17a-007, T-17a-008, T-17a-009, T-17a-010, T-17a-011, T-17a-013, T-17a-034, T-17a-036, T-17a-037, T-17a-038, T-17a-041, T-17a-044, T-17a-048 |
| 22 | app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt | 40 | extension functions `AnimeSource.icon(): Drawable?`, `AnimeSource.getPreferenceKey(): String`, `AnimeSource.toStubSource(): StubAnimeSource`, `AnimeSource.getNameForAnimeInfo(): String`, `AnimeSource.isLocalOrStub(): Boolean`, `AnimeSource?.isSourceForTorrents(): Boolean` | Helpers used across anime UI and backup | Port (except `icon()` = Replace-with-crate) | `kuro-sources::naming`, `kuro-sources::icons`, `kuro-sources::torrent` | P0 (naming, stub, pref key), P2 (torrent, icon) | T-17a-014, T-17a-026, T-17a-033, T-17a-025 |
| 23 | app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt | 125 | class `AndroidMangaSourceManager(context, extensionManager, sourceRepository) : MangaSourceManager`; private `_isInitialized`, `downloadManager`, `scope`, `sourcesMapFlow`, `stubSourcesMap`; overrides `isInitialized`, `catalogueSources`, `get`, `getOrStub`, `getOnlineSources`, `getCatalogueSources`, `getStubSources`; private `registerStubSource`, `createStubSource` | Manga twin of #21 (catalogue-based instead of `sources`) | Rewrite as generic `SourceRegistry<Manga>` | `kuro-sources::registry` | P0 | T-17a-005, T-17a-006, T-17a-007, T-17a-008, T-17a-009, T-17a-010, T-17a-011, T-17a-013, T-17a-034, T-17a-036, T-17a-037, T-17a-038, T-17a-041, T-17a-044, T-17a-048 |
| 24 | app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt | 33 | extension functions `MangaSource.icon(): Drawable?`, `MangaSource.getPreferenceKey(): String`, `MangaSource.toStubSource(): StubMangaSource`, `MangaSource.getNameForMangaInfo(): String`, `MangaSource.isLocalOrStub(): Boolean` | Manga twin of #22 (no torrent helper) | Port (icon = Replace-with-crate) | `kuro-sources::naming`, `kuro-sources::icons` | P0 | T-17a-014, T-17a-026, T-17a-025 |
| 25 | app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt | 215 | class `UpdateAnimeFromRemote(sourceManager, episodeRepository, animeRepository, syncEpisodesWithSource, syncSeasonsWithSource, coverCache, backgroundCache)`; public `awaitEpisodesUpdate` (2 overloads), `awaitSeasonsUpdate` (2 overloads); private `awaitUpdateFromSource(localAnime, remoteAnime, manualFetch): Boolean` | Pulls fresh metadata + episode/season lists from the source and writes them into the DB | Rewrite: same rules, returns `Result<_>` with no cancellation swallowing | `kuro-sources::remote_update` | P0 | T-17a-027, T-17a-028, T-17a-029, T-17a-030, T-17a-031, T-17a-043, T-17a-047 |
| 26 | app/src/main/java/mihon/domain/source/models/RemoteAnimeEpisodeUpdate.kt | 9 | data class `RemoteAnimeEpisodeUpdate(anime: Anime, newEpisodes: List<Episode>)` | Result of an episode refresh | Port | `kuro-sources::remote_update` | P0 | T-17a-032 |
| 27 | app/src/main/java/mihon/domain/source/models/RemoteAnimeSeasonUpdate.kt | 8 | data class `RemoteAnimeSeasonUpdate(anime: Anime, newSeasons: List<Anime>)` | Result of a season refresh | Port | `kuro-sources::remote_update` | P1 | T-17a-029, T-17a-032 |

## Name index (every declared name, for grep)

Classes: `GetAnimeIncognitoState`, `GetAnimeSourcesWithFavoriteCount`, `GetEnabledAnimeSources`, `GetLanguagesWithAnimeSources`, `ToggleAnimeIncognito`, `ToggleAnimeSource`, `ToggleAnimeSourcePin`, `SetMigrateSorting`, `ToggleLanguage`, `GetEnabledMangaSources`, `GetLanguagesWithMangaSources`, `GetMangaIncognitoState`, `GetMangaSourcesWithFavoriteCount`, `ToggleExcludeFromMangaDataSaver`, `ToggleMangaIncognito`, `ToggleMangaSource`, `ToggleMangaSourcePin`, `SourcePreferences`, `AndroidAnimeSourceManager`, `AndroidMangaSourceManager`, `UpdateAnimeFromRemote`, `RemoteAnimeEpisodeUpdate` (data), `RemoteAnimeSeasonUpdate` (data).

Nested enums: `SetMigrateSorting.Mode` (`ALPHABETICAL`, `TOTAL`), `SetMigrateSorting.Direction` (`ASCENDING`, `DESCENDING`), `SourcePreferences.DataSaver` (`NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT`).

Top-level functions and extension properties: `AnimeSource.icon` (property, AnimeSource.kt), `Source.icon` (property, MangaSource.kt), `AnimeSource.icon()`, `AnimeSource.getPreferenceKey()`, `AnimeSource.toStubSource()`, `AnimeSource.getNameForAnimeInfo()`, `AnimeSource.isLocalOrStub()`, `AnimeSource?.isSourceForTorrents()` (AnimeSourceExtensions.kt), `MangaSource.icon()`, `MangaSource.getPreferenceKey()`, `MangaSource.toStubSource()`, `MangaSource.getNameForMangaInfo()`, `MangaSource.isLocalOrStub()` (MangaSourceExtensions.kt).

Private members with behaviour: `ToggleAnimeSource.isEnabled`, `ToggleMangaSource.isEnabled`, `GetAnimeSourcesWithFavoriteCount.sortFn`, `GetMangaSourcesWithFavoriteCount.sortFn`, `UpdateAnimeFromRemote.awaitUpdateFromSource`, `AndroidAnimeSourceManager.registerStubSource`, `AndroidAnimeSourceManager.createStubSource`, `AndroidMangaSourceManager.registerStubSource`, `AndroidMangaSourceManager.createStubSource`.

## Cross-scope types referenced (owned elsewhere, read for exactness)

| Type | Owner | Path |
|---|---|---|
| `MangaSource`, `CatalogueSource`, `AnimeSource`, `AnimeHttpSource`, `HttpSource` | area:15-source-api-manga / area:16 (anime API, plan not yet written) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt`, `.../source/CatalogueSource.kt`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt` |
| `Source` (manga domain), `StubMangaSource`, `Pin`, `Pins`, `MangaSourceWithCount`, `MangaSourceManager`, `MangaSourceRepository`, `MangaStubSourceRepository` | area:09 | `domain/src/main/java/tachiyomi/domain/source/manga/**` |
| `AnimeSource` (domain), `StubAnimeSource`, `Pin`, `Pins`, `AnimeSourceManager`, `AnimeSourceRepository`, `AnimeStubSourceRepository` | area:09 | `domain/src/main/java/tachiyomi/domain/source/anime/**` |
| `MangaSourceRepositoryImpl`, `AnimeSourceRepositoryImpl`, `MangaStubSourceRepositoryImpl`, `AnimeStubSourceRepositoryImpl` | area:09 / area:06 | `data/src/main/java/tachiyomi/data/source/{manga,anime}/` |
| `LocalMangaSource`, `LocalAnimeSource` (id 0, lang "other") | area:14-source-local-and-metadata | `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/{manga,anime}/` |
| `MangaExtensionManager`, `AnimeExtensionManager` | area:18/19/20 (extension plans) | `app/src/main/java/eu/kanade/tachiyomi/extension/{manga,anime}/` |
| `MangaDownloadManager.renameSource`, `AnimeDownloadManager.renameSource` | area:23-25 (downloads) | `app/src/main/java/eu/kanade/tachiyomi/data/download/{manga,anime}/` |
| `SyncEpisodesWithSource`, `SyncSeasonsWithSource`, `MangaCoverCache`/`AnimeCoverCache`/`AnimeBackgroundCache` | area:07, area:05, area:13 | `app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt` |
| `LocaleHelper` | area:01 / area:02 | `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt` |
| `Preference`, `PreferenceStore`, `getEnum`, `getAndSet` | area:02 | `core/common/src/main/java/tachiyomi/core/common/preference/` |
| `compareToWithCollator` | area:13 | `core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt` |
| `BasePreferences.incognitoMode` | area:02 | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt` |
| `LibraryDisplayMode` | area:08 | `domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt` |

## Test files in scope

None. No `*Test.kt` exists in the 27 scope files, so there are no upstream tests to port from this area. Tests to write are listed in `p1-edge-cases-and-tests.md`.

## UI in scope

None of the 27 files renders UI. The Compose-typed `icon` properties (rows 8 and 19) are consumed by UI but are not screens. `p1-ui-parity.md` is therefore intentionally not written. Consumer screens are owned by the UI areas; the mockup gap for source lists is recorded in `p1-rust-design.md` section 9.

## Mismatch with area:09 (flagged)

`p1-topic-sources-pins-stubs.md` in area:09 describes the local source with `lang ""`. The Kotlin code gives `lang = "other"` for both local sources (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:72` and `.../anime/LocalAnimeSource.kt:68`). This area uses `"other"`. area:09 should be corrected.
