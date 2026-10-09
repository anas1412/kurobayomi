# Area 40 (part 2): data models, preferences, files, endpoints, DB

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Shared keys that part 1 already specifies (`source_languages`, `hidden_catalogues`, `pinned_catalogues`, `pref_migration_sorting`, `pref_migration_direction`, `pref_display_mode_catalogue`, `migrate_flags`) are not repeated in full; section 5 lists the additional keys this part reads or writes, and section 6 the config file keys. Part 1 section 5 and section 10 remain authoritative for their keys.

---

## 1. Models

### 1.1 Source (domain)

`domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:3-14`

| Field | Kotlin type | Nullable | Default | Meaning |
|---|---|---|---|---|
| `id` | `Long` | no | none | Source id (extension-defined; `0` = local source) |
| `lang` | `String` | no | none | Language code, `"all"`, `"en"`, `"other"` for local, `""` for unknown |
| `name` | `String` | no | none | Display name from the extension |
| `supportsLatest` | `Boolean` | no | none | Source supports "latest" listing |
| `isStub` | `Boolean` | no | none | Source is not installed (stub) |
| `pin` | `Pins` | no | `Pins.unpinned` (`0b00`) | Pin bits (1.3) |
| `isUsedLast` | `Boolean` | no | `false` | Copy created for the last-used section |
| `isExcludedFromDataSaver` | `Boolean` | no | `false` | SY, dropped |

Derived: `visualName` (`Source.kt:16-20`), `key` (`Source.kt:22-27`): `"$id-lastused"` if `isUsedLast`, else `"$id"`.

Domain sub-type `MangaSourceWithCount(source, count)` is the `Pair<Source, Long>` used by the migrate picker (`GetMangaSourcesWithFavoriteCount`, `Long` count).

### 1.2 Pins (domain)

`domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt`. Stored only as `Pins.code: Int` in memory; persisted as the id-string sets (section 5), not as a bitmask.

| Name | Code | Bit |
|---|---|---|
| `Pin.Unpinned` | `0b00` = 0 | none |
| `Pin.Pinned` | `0b01` = 1 | bit 0 |
| `Pin.Actual` | `0b10` = 2 | bit 1 |
| `Pins.unpinned` | 0 | |
| `Pins.pinned` | 3 (`Pinned | Actual`) | |

Operations: `contains(pin) = (pin.code & code) == pin.code`; `plus(pin) = code | pin.code`; `minus(pin) = code ^ pin.code` (XOR, see behavior 1.4).

### 1.3 Category (domain)

`domain/src/main/java/tachiyomi/domain/category/model/Category.kt:13-15`

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | `Long` | none | `0` = system "Uncategorized" (`UNCATEGORIZED_ID = 0L`, `Category.kt:15-16`) |
| `name` | `String` | none | System row name is `""` (`categories.sq`, insert `(0, "", -1, 0)`) |
| `order` / `sort` | `Long` | none | System row `-1` |
| `flags` | `Long` | none | System row `0` |
| `hidden` | `Boolean` | `false` | |
| `isSystemCategory` | derived | `id == 0L` | |

### 1.4 Manga (domain fields used by this area)

`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`

Used here: `id: Long`, `source: Long`, `favorite: Boolean`, `dateAdded: Long` (epoch ms; `0` = not in library), `coverLastModified: Long`, `url: String`, `title: String`, `thumbnailUrl: String?`, `chapterFlags: Long`, `viewerFlags: Long`, `lastUpdate: Long`, `nextUpdate: Long`, `fetchInterval: Int`, `status: Long`, `artist/author/description: String?`, `genre: List<String>?`, `updateStrategy`, `initialized: Boolean`, `lastModifiedAt: Long`, `favoriteModifiedAt: Long?`, `version: Long`.

`Manga.create()` gives the defaults for new rows; `SManga.toDomainManga(sourceId)` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:72-86`) copies `url, title, artist, author, description, genre = getGenres(), status.toLong(), thumbnailUrl = thumbnail_url, updateStrategy, initialized, source`. It does not set `favorite` (stays `false`), `dateAdded` (`0`) or `id` (new row id is assigned by the insert).

`MangaUpdate` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt`): every field is nullable with default `null` (`MangaUpdate.kt:7-22` lists `source`, `lastUpdate`, `nextUpdate`, `dateAdded`, `viewerFlags`, `chapterFlags`, `coverLastModified`, `status`, and the rest of the fields in that file). `Manga.toMangaUpdate()` (`MangaUpdate.kt:29-41`) fills every field from the snapshot, so a full-row write is produced. Proposal: partial updates for favorite toggles (behavior 4.8.1).

### 1.5 Remote manga (SManga) and page

`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt` fields: `url: String`, `title: String`, `artist: String?`, `author: String?`, `description: String?`, `genre: String?`, `status: Int`, `thumbnail_url: String?`, `update_strategy: UpdateStrategy`, `initialized: Boolean`.

`MangasPage` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/MangasPage.kt:3`): `data class MangasPage(val mangas: List<SManga>, val hasNextPage: Boolean)`.

### 1.6 Listing (this area)

`BrowseMangaSourceScreenModel.Listing` (`SRC/browse/BrowseMangaSourceScreenModel.kt:331-348`):

| Variant | Kotlin | Query sentinel | Filters |
|---|---|---|---|
| `Popular` | `data object` | `GetRemoteManga.QUERY_POPULAR` = `"eu.kanade.domain.source.manga.interactor.POPULAR"` | `FilterList()` (fixed) |
| `Latest` | `data object` | `GetRemoteManga.QUERY_LATEST` = `"eu.kanade.domain.source.manga.interactor.LATEST"` | `FilterList()` (fixed) |
| `Search` | `data class Search(query: String?, filters: FilterList)` | none | source filter list |

Proposal (Rust): `enum Listing { Popular, Latest, Search { query: Option<String>, filters: FilterTree } }`. The string sentinels are an in-process encoding of the navigation argument; Kurobayomi passes the enum, and never writes the sentinel strings to disk.

### 1.7 Filter model (source-api)

`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt:3-42`

| Class | Stored state type | Default | Raw constants |
|---|---|---|---|
| `Header(name)` | `Any` (0) | `0` | none |
| `Separator(name = "")` | `Any` (0) | `0` | none |
| `Select<V>(name, values: Array<V>, state: Int = 0)` | `Int` (index) | `0` | none |
| `Text(name, state: String = "")` | `String` | `""` | none |
| `CheckBox(name, state: Boolean = false)` | `Boolean` | `false` | none |
| `TriState(name, state: Int = STATE_IGNORE)` | `Int` | `0` | `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2` |
| `Group<V>(name, state: List<V>)` | `List<V>` (children) | caller | none |
| `Sort(name, values: Array<String>, state: Selection? = null)` | `Sort.Selection?` | `null` | `Selection(index: Int, ascending: Boolean)` |

`Filter.equals` compares `name` and `state` (`Filter.kt:31-36`). `FilterList` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/FilterList.kt:6-16`) is `data class FilterList(val list: List<Filter<*>>)`, and its `equals` is always `false` (`:10-12`); `hashCode` = `list.hashCode()`.

Rust mapping (p2-rust-design.md section 3): `FilterNode` enum with the same variants; `TriStateRaw(i32)` stored raw and validated on read; `Select { values: Vec<SelectValue>, state: usize }` where `SelectValue { text: String, is_string: bool }`; `Sort { values: Vec<String>, state: Option<SortSelection> }`.

### 1.8 TriState (core, UI)

`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:3-15`: `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`; `next()` cycles `DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED`. Not persisted by this area. Mapping to source raw values: behavior 6.2.

### 1.9 Migration sort types (domain)

`domain/...` (not in scope; the enum lives in `app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:14-22`):
- `SetMigrateSorting.Mode`: `ALPHABETICAL`, `TOTAL`.
- `SetMigrateSorting.Direction`: `ASCENDING`, `DESCENDING`.
Persisted with `getEnum` (serializer = enum `name`, `PreferenceStore.kt:27-40`; an unknown stored string falls back to the default).

### 1.10 Global search types (this area, in-memory)

`MangaSourceFilter` (`globalsearch/MangaSearchScreenModel.kt:213-216`): `All`, `PinnedOnly`. Not persisted. Default `PinnedOnly`.

`MangaSearchItemResult` (`globalsearch/MangaSearchScreenModel.kt:218-235`):
- `Loading` (`data object`).
- `Error(throwable: Throwable)`; displayed text = `throwable.message ?: MR.strings.unknown_error` (part 1).
- `Success(result: List<Manga>)`, `isEmpty = result.isEmpty()`.

`MangaSearchScreenModel.State` (`:199-210`): `fromSourceId: Long?`, `searchQuery: String?`, `sourceFilter: MangaSourceFilter`, `onlyShowHasResults: Boolean`, `items: PersistentMap<CatalogueSource, MangaSearchItemResult>`. Derived: `progress: Int`, `total: Int`, `filteredItems`.

### 1.11 Browse model state

`BrowseMangaSourceScreenModel.State` (`:361-369`): `listing: Listing`, `filters: FilterList = FilterList()`, `toolbarQuery: String? = null`, `dialog: Dialog? = null`; derived `isUserQuery`.

`Dialog` (`:350-359`): `Filter`, `RemoveManga(manga)`, `AddDuplicateManga(manga, duplicate)`, `ChangeMangaCategory(manga, initialSelection: ImmutableList<CheckboxState.State<Category>>)`, `Migrate(newManga, oldManga)`.

### 1.12 Sources list state

`MangaSourcesScreenModel.State` (`SRC/MangaSourcesScreenModel.kt:137-147`): `dialog: Dialog?` (`Dialog(source: Source)`), `isLoading: Boolean = true`, `items: ImmutableList<MangaSourceUiModel>`, `dataSaverEnabled` (SY), derived `isEmpty`.

`MangaSourceUiModel` (presentation, referenced: `app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt:223-226`): `Item(source: Source)`, `Header(language: String)`.

Events: `MangaSourcesScreenModel.Event.FailedFetchingSources` (`:131-133`).

### 1.13 Sources filter state

`MangaSourcesFilterScreenModel.State` (`SRC/MangaSourcesFilterScreenModel.kt:61-81`): `Loading`, `Error(throwable)`, `Success(items: SortedMap<String, List<Source>>, enabledLanguages: Set<String>, disabledSources: Set<String>)`.

### 1.14 Migrate source picker state

`MigrateMangaSourceScreenModel.State` (`MIG/MigrateMangaSourceScreenModel.kt:83-91`): `isLoading: Boolean = true`, `items: ImmutableList<Pair<Source, Long>>`, `sortingMode: SetMigrateSorting.Mode = ALPHABETICAL`, `sortingDirection: SetMigrateSorting.Direction = ASCENDING`, derived `isEmpty`. Event: `FailedFetchingSourcesWithCount` (`:93-95`).

### 1.15 Icon and cover references

`MangaCoverCache` (`app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt:18`): `deleteFromCache(manga, deleteCustomCover: Boolean): Int` (count of deleted files, used by `removeCovers`). File layout: area 34.

---

## 2. Enums and their stored values

| Enum | Kotlin definition | Stored form | Default | Unknown stored value |
|---|---|---|---|---|
| `LibraryDisplayMode` | `domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt:3-44` | `"COMPACT_GRID"`, `"COMFORTABLE_GRID"`, `"COVER_ONLY_GRID"`, `"LIST"` (`serialize()` lines 31-37) | `CompactGrid` | `default` (`CompactGrid`) |
| `SetMigrateSorting.Mode` | `app/.../SetMigrateSorting.kt:14-17` | enum `name`: `"ALPHABETICAL"`, `"TOTAL"` | `ALPHABETICAL` | default |
| `SetMigrateSorting.Direction` | `app/.../SetMigrateSorting.kt:19-22` | `"ASCENDING"`, `"DESCENDING"` | `ASCENDING` | default |
| `SourcePreferences.DataSaver` (SY) | `SourcePreferences.kt:118-123` | `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` | `NONE` | dropped (section 9) |
| `Pin` codes | `domain/.../manga/model/Pin.kt` | `0b00`, `0b01`, `0b10` (in memory) | `Unpinned` | n/a |
| `Filter.TriState` raw | `source-api/.../Filter.kt:17-21` | `0`, `1`, `2` (source state) | `0` | throws in UI (D-BR-02) |
| `MangaSourceFilter` | `globalsearch/MangaSearchScreenModel.kt:213` | `All`, `PinnedOnly` (in memory) | `PinnedOnly` | n/a |
| `TriState` (UI) | `core/.../TriState.kt` | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` (in memory) | n/a | n/a |

Serialization check for `LibraryDisplayMode`: `serialize` for `List` returns `"LIST"` (`LibraryDisplayMode.kt:35`). `deserialize` maps `"COMFORTABLE_GRID"`, `"COMPACT_GRID"`, `"COVER_ONLY_GRID"`, `"LIST"`; any other string returns `default` (`:24-31`).

`getEnum` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:27-40`): serializer `it.name`, deserializer `enumValueOf(it)`, `IllegalArgumentException` -> default.

---

## 3. Bit layouts

This area defines no bit flags of its own. Bit layouts it touches:

| Bitmask | Where | Layout |
|---|---|---|
| `Pins.code` | `Pin.kt` | bit0 = Pinned, bit1 = Actual (section 1.2). Stored as id sets, not as bits. |
| `Manga.chapterFlags` (Long) | written indirectly via `SetMangaDefaultChapterFlags` | defined by area 38/08 (`Manga.kt:77-88`: `CHAPTER_SORT_DIR_MASK = 0x1`, `CHAPTER_UNREAD_MASK = 0x6`, `CHAPTER_SHOW_DOWNLOADED = 0x8`, `CHAPTER_SHOW_NOT_DOWNLOADED = 0x10`, ...). Not interpreted here. |
| `migrate_flags` (Int) | not used by this area (dialog in part 1) | part 1 section 4 |

---

## 4. Files, folders, formats on disk

This area writes no files directly. Kurobayomi persistence applies:

| Item | Path (Kurobayomi) | Format | Owner |
|---|---|---|---|
| Config (preferences of this area) | `$XDG_CONFIG_HOME/kurobayomi/browse.toml` | TOML, `[browse]` table | part 1 section 10 (file) and this part (section 6 keys) |
| Database | `$XDG_DATA_HOME/kurobayomi/kurobayomi.db` | SQLite (`mangas`, `mangas_categories`, `categories`) | area 03 |
| Covers | `$XDG_CACHE_HOME/kurobayomi/covers/` | JPEG/PNG | area 34 |

Android paths replaced by this area's operations: `MangaCoverCache` cover files (area 34).

---

## 5. Preference keys used or written by this area

Keys are the Aniyomi `PreferenceStore` keys (exact). The Kurobayomi TOML key is in section 6.

| Key (exact) | Owner | Type | Default | Read | Write | Area behaviour |
|---|---|---|---|---|---|---|
| `source_languages` | `SourcePreferences.enabledLanguages()` (`SourcePreferences.kt:23-26`) | StringSet | `LocaleHelper.getDefaultEnabledLanguages()` = `{"all","en", systemLang}` (`LocaleHelper.kt:72-74`) | Sources list (`GetEnabledMangaSources.kt:32`), Sources filter (`GetLanguagesWithMangaSources.kt:18`), global search (`MangaSearchScreenModel.kt:49`) | Language toggle (`ToggleLanguage.kt:10-15`) | |
| `hidden_catalogues` | `disabledMangaSources()` (`SourcePreferences.kt:57`) | StringSet of id strings | empty | all source lists and search | `ToggleMangaSource` (`ToggleMangaSource.kt:15-19`) | ids as decimal strings |
| `pinned_catalogues` | `pinnedMangaSources()` (`SourcePreferences.kt:63`) | StringSet of id strings | empty | Sources list, global search sort | `ToggleMangaSourcePin.kt:11-16` | |
| `last_catalogue_source` | `lastUsedMangaSource()` (`SourcePreferences.kt:69-72`), key is `Preference.appStateKey("last_catalogue_source")` | Long | `-1` | `GetEnabledMangaSources.kt:24` | `BrowseMangaSourceScreenModel.kt:100-102` (unless incognito) | stored key: `__APP_STATE_last_catalogue_source` |
| `pref_display_mode_catalogue` | `sourceDisplayMode()` (`SourcePreferences.kt:16-21`) | LibraryDisplayMode (serialized, section 2) | `CompactGrid` | browse grid | `displayMode` setter (`BrowseMangaSourceScreenModel.kt:77`) | shared with anime |
| `has_filters_toggle_state` | `globalSearchFilterState()` (`SourcePreferences.kt:49-52`), key `Preference.appStateKey("has_filters_toggle_state")` | Boolean | `false` | global and migrate search (`MangaSearchScreenModel.kt:68-70`) | `toggleFilterResults()` (`:120-122`) | stored key: `__APP_STATE_has_filters_toggle_state` |
| `browse_hide_in_library_items` | `hideInMangaLibraryItems()` (`SourcePreferences.kt:82-85`) | Boolean | `false` | browse paging filter (`BrowseMangaSourceScreenModel.kt:108,121`) | settings (presentation) | |
| `pref_migration_sorting` | `migrationSortingMode()` (`SourcePreferences.kt:30-33`) | enum | `ALPHABETICAL` | migrate picker | `SetMigrateSorting.kt:10` | |
| `pref_migration_direction` | `migrationSortingDirection()` (`SourcePreferences.kt:35-38`) | enum | `ASCENDING` | migrate picker | `SetMigrateSorting.kt:11` | |
| `pref_library_columns_portrait_key` | `LibraryPreferences.mangaPortraitColumns()` (`LibraryPreferences.kt:103`) | Int | `0` | browse grid columns (`BrowseMangaSourceScreenModel.kt:129-133`) | area 37 | `0` = adaptive |
| `pref_library_columns_landscape_key` | `LibraryPreferences.mangaLandscapeColumns()` (`LibraryPreferences.kt:106`) | Int | `0` | browse grid columns (`:129-133`) | area 37 | |
| `default_category` | `LibraryPreferences.defaultMangaCategory()` (`LibraryPreferences.kt:154`; const `DEFAULT_MANGA_CATEGORY_PREF_KEY = "default_category"` at `:413`) | Int | `-1` | add-to-library decision (`BrowseMangaSourceScreenModel.kt:258`) | settings (area 37) | `-1` = always ask, `0` = uncategorised, `>0` = category id |
| `incognito_mode` | `BasePreferences.incognitoMode()` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21`), key `Preference.appStateKey("incognito_mode")` | Boolean | `false` | `GetMangaIncognitoState.kt:16` | area 02 / 36 | stored key: `__APP_STATE_incognito_mode` |
| `incognito_manga_extensions` | `incognitoMangaExtensions()` (`SourcePreferences.kt:60`) | StringSet of package names | empty | `GetMangaIncognitoState.kt:18-19` | `ToggleMangaIncognito.kt:10` | |

Preference keys in `SourcePreferences` not used by this area (listed so that the dropped SY keys are explicit): `data_saver` (`:97`), `ignore_jpeg` (`:99`), `ignore_gif` (`:101`), `data_saver_image_quality` (`:103`), `data_saver_image_format_jpeg` (`:105-108`), `data_saver_server` (`:110`), `data_saver_color_bw` (`:112`), `data_saver_excluded` (`:114`), `data_saver_downloader` (`:116`). All dropped (behavior section 15). `show_nsfw_source` (`:28`) is part 1.

Migration of stored keys on Kurobayomi import (for users coming from Aniyomi backups): the id-string sets and the enum names are the same strings, so no conversion is needed.

---

## 6. Kurobayomi config keys (browse.toml)

Extends the `[browse]` table defined in part 1 section 10. Keys added by this part:

```toml
[browse]
# part 1 keys: display_mode, hidden_sources, pinned_sources, enabled_languages,
#              incognito_extensions, migrate_sort_mode, migrate_sort_direction,
#              migrate_flags, show_nsfw_sources

global_search_has_results_only = false   # __APP_STATE_has_filters_toggle_state (Boolean, default false)
hide_in_library_items = false            # browse_hide_in_library_items (Boolean, default false)
last_used_source = -1                    # __APP_STATE_last_catalogue_source (i64, default -1 = none)
default_manga_category = -1              # default_category (i32, -1 = always ask, 0 = uncategorised, >0 = category id; owned by area 37)
library_columns_portrait = 0             # pref_library_columns_portrait_key (owned by area 37)
library_columns_landscape = 0            # pref_library_columns_landscape_key (owned by area 37)
```

Notes:
- `last_used_source` and `global_search_has_results_only` are app state, not user settings. They are written by the app and are not shown in the settings UI.
- `default_manga_category`, `library_columns_*` are read here and written by area 37 / area 08.
- Rules from part 1: unknown keys are preserved on save; writes are atomic (temp file + rename).
- Deserialization: `migrate_sort_mode` unknown -> `ALPHABETICAL`; `migrate_sort_direction` unknown -> `ASCENDING`; `display_mode` unknown -> `COMPACT_GRID`; `default_manga_category` non-integer -> `-1`; `library_columns_*` negative -> `0`.

---

## 7. Network endpoints

This area makes no direct HTTP call. All network access goes through the `CatalogueSource` interface of the installed source (extension), which is area 15/16 (manga source API) and area 12 (network stack).

| Operation | Kotlin call | Page numbering | Parameters | Result |
|---|---|---|---|---|
| Popular | `source.getPopularManga(page)` (`MangaSourcePagingSource.kt:26`) | 1-based (`params.key ?: 1`, `:44`) | `page: Int` | `MangasPage` |
| Latest | `source.getLatestUpdates(page)` (`MangaSourcePagingSource.kt:32`) | 1-based | `page: Int` | `MangasPage` |
| Search (browse) | `source.getSearchManga(page, query, filters)` (`MangaSourcePagingSource.kt:21`) | 1-based | `query: String` (may be `""`), `filters: FilterList` | `MangasPage` |
| Search (global / migrate) | `source.getSearchManga(1, query, source.getFilterList())` (`MangaSearchScreenModel.kt:161`) | page 1 only | `query: String`, default filters | `MangasPage` |
| Filter list | `source.getFilterList()` (`BrowseMangaSourceScreenModel.kt:94,150,187,217`; `MangaSearchScreenModel.kt:161`) | n/a | none | `FilterList` (local, no request) |
| Supports latest | `CatalogueSource.supportsLatest` (`BrowseMangaSourceScreen.kt:175`) | n/a | n/a | Boolean (no request) |
| WebView | `WebViewScreen(url = source.baseUrl, ...)` (`BrowseMangaSourceScreen.kt:117-123`) | n/a | `baseUrl` of `HttpSource` | opens in the WebView screen (area 36) |

External URLs opened by this scope (no auth, plain links):

| Use | URL | Source |
|---|---|---|
| Local source help | `https://aniyomi.org/help/guides/local-manga/` | `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:379` (`HELP_URL`) |
| Browse error help | `https://aniyomi.org/docs/guides/troubleshooting/` | `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:4` (`URL_HELP`) |
| Migrate guide | `https://aniyomi.org/help/guides/source-migration/` | `MIG/MigrateMangaSourceTab.kt:35` |

---

## 8. Database tables and columns touched

Schemas: `data/src/main/sqldelight/data/mangas.sq`, `mangas_categories.sq`, `categories.sq`.

### 8.1 mangas

Columns (exact, from `mangas.sq:3-27`): `_id INTEGER NOT NULL PRIMARY KEY`, `source INTEGER NOT NULL`, `url TEXT NOT NULL`, `artist TEXT`, `author TEXT`, `description TEXT`, `genre TEXT AS List<String>`, `title TEXT NOT NULL`, `status INTEGER NOT NULL`, `thumbnail_url TEXT`, `favorite INTEGER AS Boolean NOT NULL`, `last_update INTEGER`, `next_update INTEGER`, `initialized INTEGER AS Boolean NOT NULL`, `viewer INTEGER NOT NULL`, `chapter_flags INTEGER NOT NULL`, `cover_last_modified INTEGER NOT NULL`, `date_added INTEGER NOT NULL`, `update_strategy INTEGER AS UpdateStrategy NOT NULL DEFAULT 0`, `calculate_interval INTEGER DEFAULT 0 NOT NULL`, `last_modified_at INTEGER NOT NULL DEFAULT 0`, `favorite_modified_at INTEGER`, `version INTEGER NOT NULL DEFAULT 0`, `is_syncing INTEGER NOT NULL DEFAULT 0`.

Indexes: `library_favorite_index ON mangas(favorite) WHERE favorite = 1`, `mangas_url_index ON mangas(url)`.
Trigger: `update_last_favorite_at_mangas AFTER UPDATE OF favorite ON mangas` sets `favorite_modified_at = strftime('%s','now')` (`mangas.sq:25-30`). Consequence: every favorite change stamps `favorite_modified_at` in seconds (not ms; `dateAdded` is ms).

Queries used by this scope:

| Query (`mangas.sq` name) | Line | Used by | SQL |
|---|---|---|---|
| `getSourceIdWithFavoriteCount` | `:103-109` | `MangaSourceRepositoryImpl.getMangaSourcesWithFavoriteCount` | `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source;` |
| `getDuplicateLibraryManga` | `:117-122` | `GetDuplicateLibraryManga` | `SELECT * FROM mangas WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id;` |
| `getSourceIdsWithNonLibraryManga` | `:141-146` | `MangaSourceRepositoryImpl.getMangaSourcesWithNonLibraryManga` (cleanup UI, outside scope) | `SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source;` |
| `deleteMangasNotInLibraryBySourceIds` | `:148-152` | cleanup (outside scope) | `DELETE FROM mangas WHERE favorite = 0 AND source IN :sourceIds;` |
| `getMangaByUrlAndSource` | (area 05) | `NetworkToLocalManga` via `MangaRepositoryImpl.kt:28-36` | `(url, source)` lookup |
| `update` | `:156-178` | `updateManga.await` (`changeMangaFavorite`) | `UPDATE mangas SET col = coalesce(:param, col), ...` for 22 columns |

`getSourceIdWithFavoriteCount` has no `ORDER BY` (behavior 11.2 proposes adding one).

### 8.2 mangas_categories

Columns (`data/src/main/sqldelight/data/mangas_categories.sq:1-9`): `_id INTEGER NOT NULL PRIMARY KEY`, `manga_id INTEGER NOT NULL` (FK to `mangas._id`, `ON DELETE CASCADE`), `category_id INTEGER NOT NULL` (FK to `categories._id`, `ON DELETE CASCADE`).

Write used here: `MangaRepositoryImpl.setMangaCategories(mangaId, categoryIds)` (`data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:91-97`): inside one transaction, `deleteMangaCategoryByMangaId(mangaId)` then `insert(mangaId, categoryId)` for each id. Empty list = remove all categories.

### 8.3 categories

Columns (`categories.sq:1-7`): `_id INTEGER NOT NULL PRIMARY KEY`, `name TEXT NOT NULL`, `sort INTEGER NOT NULL`, `flags INTEGER NOT NULL`, `hidden INTEGER NOT NULL DEFAULT 0`.
System row (`categories.sq:9-10`): `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);`. A trigger (`system_category_delete_trigger`, line 11 onward) forbids deleting it.

Queries used here:
- `getCategoriesByMangaId` (`categories.sq:47-53` and following): returns `id, name, order, flags, hidden` for the manga's links. Used by `GetMangaCategories.await(mangaId)` and `subscribe(mangaId)`.
- `getAllMangaCategoriesAsFlow` (behavior 4.8.3): all rows, used by `getCategories()`.

### 8.4 Transactions and ordering

- `setMangaCategories` is one transaction. `updateManga` is one statement. `setMangaDefaultChapterFlags` is a separate write (`SetMangaChapterFlags`, area 08). The three are not in one transaction and have no defined order in `changeMangaFavorite` / `addFavorite` (behavior 4.8.2, 4.8.1).

---

## 9. Dropped configuration (SY fork data saver)

Dropped with reason in behavior section 15. Keys listed in section 5 (`data_saver`, `ignore_jpeg`, `ignore_gif`, `data_saver_image_quality`, `data_saver_image_format_jpeg`, `data_saver_server`, `data_saver_color_bw`, `data_saver_excluded`, `data_saver_downloader`). Kurobayomi does not read them. Backup import of a file that contains them keeps them as unknown data (area 21/22 rule: keep unknown keys in a passthrough map).

---

## 10. Data-model invariants (for tests)

- `Source.key` is unique per row: a last-used copy has the suffix `-lastused`.
- `Pins.code` is in `0..=3`. Values outside are not produced by this area.
- `Listing::Popular` and `Listing::Latest` carry no filters; `Listing::Search` carries the filter tree that was active when the search was issued.
- `dateAdded` = `0` iff the manga is not a favorite that was added via this screen (set by `changeMangaFavorite`).
- `favorite_modified_at` is seconds (trigger), `dateAdded` is milliseconds (`Instant.toEpochMilli()`).
- Language codes in `source_languages` are compared exactly (`"en"` != `"en-US"`).
- A source id in `hidden_catalogues` or `pinned_catalogues` is a decimal string without a sign (Long to string).
