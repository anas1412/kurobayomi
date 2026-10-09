# Area 17 — App source layer: data models, preference keys, storage, endpoints

Paths are repo-relative to `/home/blackbox/aniyomi-src`. "Rust key" is the key used in the Kurobayomi settings store (area:02). Keys are kept byte-identical to Aniyomi so that a future Aniyomi backup import can map them 1:1.

## 1. Preference keys read or written by this area

Store type: `PreferenceStore` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt`). Keys starting with `__APP_STATE_` are app state (`Preference.appStateKey`, `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`; `Preference.isAppState` at `:40-42`).

Set-valued keys use `getStringSet`, whose elements are DECIMAL source ids (for source sets) or PACKAGE NAMES (for incognito sets). Enum keys store `enum.name` (`PreferenceStore.kt:27-45`).

| Kotlin accessor (`SourcePreferences`) | Stored key | Type | Default | Used by | Rust key | Notes |
|---|---|---|---|---|---|---|
| `sourceDisplayMode()` | `pref_display_mode_catalogue` | object via `LibraryDisplayMode.Serializer` | `LibraryDisplayMode.default` (= `CompactGrid`, `domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt:22`) | source list layout | same | serializer format owned by area:08 |
| `enabledLanguages()` | `source_languages` | string set | `LocaleHelper.getDefaultEnabledLanguages()` = `setOf("all", "en", Locale.getDefault().language)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:72-74`) | enabled list, language groups, name rule | same | see section 5 for the system language rule |
| `showNsfwSource()` | `show_nsfw_source` | bool | `true` | source list filter (consumers) | same | read-only here, not used in the 27 files |
| `migrationSortingMode()` | `pref_migration_sorting` | enum `SetMigrateSorting.Mode` | `ALPHABETICAL` | migration list | same | enum name stored |
| `migrationSortingDirection()` | `pref_migration_direction` | enum `SetMigrateSorting.Direction` | `ASCENDING` | migration list | same | enum name stored |
| `animeExtensionRepos()` | `anime_extension_repos` | string set | empty | extension repos (area 18-20) | same | not used here |
| `mangaExtensionRepos()` | `extension_repos` | string set | empty | extension repos | same | the manga key has no prefix (historic) |
| `trustedExtensions()` | `__APP_STATE_trusted_extensions` (`Preference.appStateKey("trusted_extensions")`) | string set | empty | extension trust (area 18-20) | same | app state |
| `globalSearchFilterState()` | `__APP_STATE_has_filters_toggle_state` | bool | `false` | global search filters toggle | same | app state |
| `disabledAnimeSources()` | `hidden_anime_catalogues` | string set of source ids | empty | enabled list, toggles | `hidden_anime_catalogues` | key name says "hidden" but it is the DISABLED set |
| `disabledMangaSources()` | `hidden_catalogues` | string set of source ids | empty | enabled list, toggles | `hidden_catalogues` | same naming quirk |
| `incognitoAnimeExtensions()` | `incognito_anime_extensions` | string set of package names | empty | incognito state | same | |
| `incognitoMangaExtensions()` | `incognito_manga_extensions` | string set of package names | empty | incognito state | same | |
| `pinnedAnimeSources()` | `pinned_anime_catalogues` | string set of source ids | empty | pins | same | |
| `pinnedMangaSources()` | `pinned_catalogues` | string set of source ids | empty | pins | same | |
| `lastUsedAnimeSource()` | `__APP_STATE_last_anime_catalogue_source` (`Preference.appStateKey("last_anime_catalogue_source")`) | long | `-1` | last-used duplicate | same | app state |
| `lastUsedMangaSource()` | `__APP_STATE_last_catalogue_source` (`Preference.appStateKey("last_catalogue_source")`) | long | `-1` | last-used duplicate | same | app state; note the anime and manga key names differ in shape (`last_anime_catalogue_source` vs `last_catalogue_source`) |
| `animeExtensionUpdatesCount()` | `animeext_updates_count` | int | `0` | extension update badge (area 18-20) | same | |
| `mangaExtensionUpdatesCount()` | `ext_updates_count` | int | `0` | same | same | naming inconsistency kept for compatibility |
| `hideInAnimeLibraryItems()` | `browse_hide_in_anime_library_items` | bool | `false` | browse hides library items | same | consumer in browse UI |
| `hideInMangaLibraryItems()` | `browse_hide_in_library_items` | bool | `false` | same | same | |
| `dataSaver()` | `data_saver` | enum `SourcePreferences.DataSaver` | `NONE` | image pipeline (not here) | same | enum name stored |
| `dataSaverIgnoreJpeg()` | `ignore_jpeg` | bool | `false` | image pipeline | same | |
| `dataSaverIgnoreGif()` | `ignore_gif` | bool | `true` | image pipeline | same | |
| `dataSaverImageQuality()` | `data_saver_image_quality` | int | `80` | image pipeline | same | |
| `dataSaverImageFormatJpeg()` | `data_saver_image_format_jpeg` | bool | `false` | image pipeline | same | |
| `dataSaverServer()` | `data_saver_server` | string | `""` | image pipeline | same | |
| `dataSaverColorBW()` | `data_saver_color_bw` | bool | `false` | image pipeline | same | |
| `dataSaverExcludedSources()` | `data_saver_excluded` | string set of source ids | empty | toggle 6.4, flag 4.1 | same | manga only |
| `dataSaverDownloader()` | `data_saver_downloader` | bool | `true` | downloader (area 23-25) | same | |
| (BasePreferences) `incognitoMode()` | `__APP_STATE_incognito_mode` (`BasePreferences.kt:21`) | bool | `false` | global incognito | same | app state |

Inactive commented-out keys (`SourcePreferences.kt:89-95`, the SY block): `eh_enable_source_blacklist` (default `true`), `sources_tab_categories` (empty set), `sources_tab_categories_filter` (`false`), `sources_tab_source_categories` (empty set). They have no readers. Decision: dropped (P3, T-17a-039).

Key-name rules to keep: the disabled set is named "hidden_*" in storage; do not rename, because backups refer to these names.

### 1.1 Pin bit layout

Source `pin` is a 2-bit flag set (`domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt:3-7`, anime `Pin.kt:3-7`):

| Bit | Mask | Name (Kotlin object) | Meaning |
|---|---|---|---|
| 0 | `0b01` | `Pin.Pinned` | the source is pinned; menu reads "Unpin" when set (`MangaSourcesScreen.kt:184`) |
| 1 | `0b10` | `Pin.Actual` | this row is the real pinned-section copy (drives section placement, `MangaSourcesScreenModel.kt:88`) |

Constants: `Pins.unpinned = 0b00`, `Pins.pinned = Pinned|Actual = 0b11` (`Pin.kt:25-29`). `Pins.minus` is XOR (`Pin.kt:23`). Rust: `PinFlags(u8)`; the `minus` operation is replaced by `without(bit)` (AND NOT); behaviour note in behavior 4.1.

## 2. Domain models (fields, types, nullability, defaults)

### 2.1 `Source` (manga) — `domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:3-14`

| Field | Kotlin type | Nullable | Default | Rust |
|---|---|---|---|---|
| `id` | `Long` | no | — | `i64` |
| `lang` | `String` | no | — | `String` (may be `""` or `"other"` or `"all"`) |
| `name` | `String` | no | — | `String` |
| `supportsLatest` | `Boolean` | no | — | `bool` |
| `isStub` | `Boolean` | no | — | `bool` |
| `pin` | `Pins` | no | `Pins.unpinned` | `PinFlags` |
| `isUsedLast` | `Boolean` | no | `false` | `bool` |
| `isExcludedFromDataSaver` | `Boolean` | no | `false` | `bool` (manga only) |
| `visualName` (computed) | `String` | — | — | `fn visual_name(&self) -> String` |
| `key` (function-typed property) | `() -> String` | — | — | `fn key(&self) -> String` |

### 2.2 `AnimeSource` (domain) — `domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:3-11`

Same as 2.1 without `isExcludedFromDataSaver`.

### 2.3 `StubMangaSource` — `domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt:9-13`

`id: Long`, `lang: String`, `name: String`; `isInvalid` (private, computed: `name.isBlank() || lang.isBlank()`). Companion `from(source: MangaSource)`. No `equals`/`hashCode`. Also declares top-level `class SourceNotInstalledException : Exception()` (`:36`).

### 2.4 `StubAnimeSource` — `domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:15-19`

Same fields. `supportsLatest = false`, `supportsRelatedAnime = false` (`:23`, `:48`). `AnimeSourceNotInstalledException` at `:61`. No `equals`.

### 2.5 `Pins` / `Pin` — listed in 1.1.

### 2.6 `MangaSourceWithCount(source: Source, count: Long)`; computed `id` and `name` — `domain/src/main/java/tachiyomi/domain/source/manga/model/MangaSourceWithCount.kt:3-12`.

### 2.7 `SetMigrateSorting.Mode` { `ALPHABETICAL`, `TOTAL` }; `SetMigrateSorting.Direction` { `ASCENDING`, `DESCENDING` } — `app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:14-22`.

### 2.8 `SourcePreferences.DataSaver` { `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` } — `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:118-123`.

### 2.9 `RemoteAnimeEpisodeUpdate(anime: Anime, newEpisodes: List<Episode>)` — `app/src/main/java/mihon/domain/source/models/RemoteAnimeEpisodeUpdate.kt:6-9`. Non-null. Types from `tachiyomi.domain.entries.anime.model.Anime` and `tachiyomi.domain.items.episode.model.Episode`.

### 2.10 `RemoteAnimeSeasonUpdate(anime: Anime, newSeasons: List<Anime>)` — `app/src/main/java/mihon/domain/source/models/RemoteAnimeSeasonUpdate.kt:5-8`. Non-null.

### 2.11 Source-side models read here (owned by area 15/16, fields used by this area)

From `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt`:

| Field | Type | Nullable | Default (`SAnimeImpl.kt`) | Used by |
|---|---|---|---|---|
| `url` | `String` | no | — | `toSAnime` |
| `title` | `String` (lateinit semantics) | no | uninitialised: reading throws `UninitializedPropertyAccessException` | remote update (caught → `""`) |
| `thumbnail_url` | `String?` | yes | — | cover rules |
| `background_url` | `String?` | yes | — | background rules |
| `artist`, `author` | `String?` | yes | — | DB update (coalesce) |
| `status` | `Int` | no | — | DB update (`toLong()`) |
| `description` | `String?` | yes | — | DB update |
| `genre` | `String?` | yes | — | `getGenres()` |
| `update_strategy` | `AnimeUpdateStrategy` | no | `ALWAYS_UPDATE` (`SAnimeImpl.kt:30`) | DB update |
| `fetch_type` | `FetchType` | no | — | `toSAnime` |
| `season_number` | `Double` | no | — | seasons |
| `initialized` | `Boolean` | no | — | — |
| `memo` | `JsonObject` | no | — | DB update |

Status constants (`SAnime.kt:72-79`): `UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2`, `LICENSED = 3`, `PUBLISHING_FINISHED = 4`, `CANCELLED = 5`, `ON_HIATUS = 6`, `UPCOMING = 7`.

`SAnime.getGenres()` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:49-52`): `null` if `genre` is null or blank; otherwise `genre.split(", ").map(trim).filterNot(blank).distinct()`.

`AnimeUpdateStrategy` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt:10-22`) includes `ALWAYS_UPDATE` and `ONLY_FETCH_ONCE`.

`SAnimeEpisodeUpdate(anime: SAnime, episodes: List<SEpisode>)` and `SAnimeSeasonUpdate(anime: SAnime, seasons: List<SAnime>)` (`SAnimeEpisodeUpdate.kt:4-5`, `SAnimeSeasonUpdate.kt:4-5`) — the return types of the source calls.

### 2.12 `AnimeUpdate` (written by the remote update) — `domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt:7-37`

All fields nullable with default `null`. The SQL update uses `coalesce(:x, x)` (`data/src/main/sqldelightanime/dataanime/animes.sq:170-200`), so `null` = keep the stored value.

Fields written by this area and the value passed:

| Field | Value passed | Effect |
|---|---|---|
| `id` | `localAnime.id` | key |
| `title` | remote title if non-empty AND `!localAnime.favorite`, else `null` | titles of library entries are never overwritten |
| `coverLastModified` | see behavior 10 / topic file (Long ms or `null`) | cover cache invalidation timestamp |
| `backgroundLastModified` | same for background | |
| `author` | `remote.author` (may be `null` → keep) | |
| `artist` | `remote.artist` (may be `null` → keep) | |
| `description` | `remote.description` (may be `null` → keep) | |
| `genre` | `remote.getGenres()` (`List<String>?`; `null` → keep) | encoded with `StringListColumnAdapter` (`AnimeRepositoryImpl.kt`, `genre = value.genre?.let(StringListColumnAdapter::encode)`) |
| `thumbnailUrl` | `remote.thumbnail_url` if non-empty, else `null` | |
| `backgroundUrl` | `remote.background_url` if non-empty, else `null` | |
| `status` | `remote.status.toLong()` (never null → ALWAYS overwritten, even with `0` = UNKNOWN) | |
| `updateStrategy` | `remote.update_strategy` (never null → ALWAYS overwritten) | |
| `initialized` | `true` (always) | |
| `memo` | `remote.memo` (never null → ALWAYS overwritten with the source value, even `{}`) | |

`AnimeRepositoryImpl.updateAnime` returns `Boolean` and catches all exceptions to `false` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:140-148`).

### 2.13 `Anime` helpers used

`Anime.hasCustomCover(coverCache)` = `coverCache.getCustomCoverFile(id).exists()` (`app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:112-114`); `Anime.hasCustomBackground(backgroundCache)` = `backgroundCache.getCustomBackgroundFile(id).exists()` (`:116-118`). `Anime.toSAnime()` (`:49-59`) builds an `SAnime` with `genre = genre.orEmpty().joinToString()` and `status = status.toInt()`.

## 3. Database tables touched

### 3.1 Manga `sources` table

Source: `data/src/main/sqldelight/data/sources.sq`; created by migration `data/src/main/sqldelight/migrations/16.sqm` (lines 1-5).

```sql
CREATE TABLE sources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);
```

Queries:
- `findAll`: `SELECT * FROM sources;` (`sources.sq:7-9`) — `subscribeAllManga()`.
- `findOne`: `SELECT * FROM sources WHERE _id = :id;` (`sources.sq:11-14`) — `getStubMangaSource`.
- `upsert`: `INSERT INTO sources(_id, lang, name) VALUES (:id, :lang, :name) ON CONFLICT(_id) DO UPDATE SET lang = :lang, name = :name WHERE _id = :id;` (`sources.sq:16-24`).

No delete query exists (stub rows are never removed by this area).

Related queries in other tables, read by the favourite-count path:
- `mangas.sq:103-109` `getSourceIdWithFavoriteCount`: `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source;` (note: `SELECT` column list is `source, count(*)`).
- `mangas.sq:141-145` `getSourceIdsWithNonLibraryManga`: `SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source;` (consumed by `MangaSourceRepositoryImpl.getMangaSourcesWithNonLibraryManga`, not in scope).
- `data/src/main/sqldelightanime/dataanime/animes.sq:114-120` `getAnimeSourceIdWithFavoriteCount`: `SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source;`.

Neither favourite query has an `ORDER BY`; result order is unspecified SQL (in practice SQLite emits groups in `source` order). Proposal: `ORDER BY source` added in the Rust query so tie-break order is deterministic (the Kotlin tie order is the DB order).

### 3.2 Anime `animesources` table

Source: `data/src/main/sqldelightanime/dataanime/animesources.sq` (identical shape):
```sql
CREATE TABLE animesources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);
```
Queries `findAll` (`SELECT * FROM animesources;`), `findOne` (`WHERE _id = :id`), `upsert` (identical text with `animesources`). Caller: `AnimeStubSourceRepositoryImpl` (`data/src/main/java/tachiyomi/data/source/anime/AnimeStubSourceRepositoryImpl.kt`).

### 3.3 Table ownership

The two tables live in two separate SQLite files (manga DB and anime DB), so the registry must hold two store handles. Schema and migration numbering are owned by area:03 and area:04.

## 4. On-disk files and folders

No file is written directly by the 27 files. Indirect effects:
- `renameSource` moves the download folder of a source: `provider.findSourceDir(oldSource)` → `provider.getSourceDirName(newSource)`; capitalisation-only changes go through `newName + MangaDownloader.TMP_DIR_SUFFIX` (`MangaDownloadManager.kt:352-368`; anime `AnimeDownloadManager.kt:359-375`). The suffix constants and provider logic are owned by area:23-25.
- `CoverCache`/`BackgroundCache` deletions for the remote update (`AnimeCoverCache.kt:75-80`, `AnimeBackgroundCache.kt:75-80`): `deleteFromCache(anime, false)` deletes the file named `DiskUtil.hashKeyForDisk(thumbnailUrl)` under `cacheDir` (`AnimeCoverCache.kt:38-42`); `false` keeps the custom cover file.
- Extension package icons: read from the installed package (`MangaExtensionManager.kt:97-109`). No file path in this area.

## 5. System locale rule (for the default `source_languages`)

Kotlin: `setOf("all", "en", Locale.getDefault().language)`. `Locale.getDefault().language` returns the ISO 639-1 language code (`"ja"`, `"en"`, `"zh"`, `"pt"`), not the region. For a `C`/`POSIX` locale Java reports `"en"`.

Proposal (Rust): parse `LC_ALL` → `LC_MESSAGES` → `LANG` (first non-empty), strip encoding (`.UTF-8`) and modifiers (`@...`), take the part before `_` or `-`, map `C`/`POSIX`/empty to `en`. Result set `{"all", "en", <code>}` (a set; if `<code>` is `en` the set has 2 items). Store it once on first run, as Kotlin does (the preference default is only used while the key is unset).

## 6. Network endpoints

None in the 27 files. All HTTP traffic for sources happens inside source implementations (area 15/16) and the extension index (area 18-20). `UpdateAnimeFromRemote` calls `AnimeSource.getAnimeEpisodeUpdate` and `getAnimeSeasonUpdate` (default implementations in the anime source API, owned by area 16), which may perform HTTP.

## 7. Serialization and backup interop

- Source-level preferences: the source namespace key `source_<id>` (`MangaSourceExtensions.kt:14`, `AnimeSourceExtensions.kt:14`). Keep the literal format.
- Sets of source ids are serialised as decimal strings (`"1234567890"`), never as hex or as numbers. Keep this for backup and for importing an Aniyomi backup.
- Pins are NOT stored as bit flags; the bit layout exists only in memory (`Pins` is derived from set membership in 4.1).
- Whether app-state keys (`__APP_STATE_*`) are included in backups is owned by area:02 (not decided here).

## 8. Constants summary

| Constant | Value | Source |
|---|---|---|
| Local source id | `0` | `LocalMangaSource.kt:378`, `LocalAnimeSource.kt` |
| Local source lang | `"other"` | `LocalMangaSource.kt:72`, `LocalAnimeSource.kt:68` |
| Local manga name string key | `AYMR.strings.local_manga_source` = "Local manga source" | `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:495` |
| Local anime name string key | `AYMR.strings.local_anime_source` = "Local anime source" | `...strings.xml:496` |
| "No source" last-used sentinel | `-1` | `SourcePreferences.kt:65-72` |
| Stub fallback lang and name | `""`, `""` | `AndroidMangaSourceManager.kt:123`, `AndroidAnimeSourceManager.kt:125` |
| Excluded language tags in the name rule | `"all"`, `"other"` | `MangaSourceExtensions.kt:21` |
| Pinned flag value | `0b01` (pinned), `0b10` (actual), `0b11` (both) | `Pin.kt` |
| Source-name comparator | `String.CASE_INSENSITIVE_ORDER` | `GetEnabledMangaSources.kt:34` |
| Collator strength | `Collator.PRIMARY`, default locale | `core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:6-10` |
| Special language key in comparator | `"all"` sorts first | `LocaleHelper.kt:18-19` |
| zh mapping | `zh-CN` → `zh-Hans`, `zh-TW` → `zh-Hant` | `LocaleHelper.kt:62-63` |
