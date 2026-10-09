# Area 41 part 2: data models, preference keys, storage, endpoints, strings

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Rust types are proposals for the Kurobayomi port; field names keep the Kotlin spelling where the Kotlin name is the on-disk or wire name.

---

## 1. Domain and UI models

### 1.1 `AnimeSource` (domain view of a source)
File: `domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:3-25`

| Field | Kotlin type | Nullable | Default | Rust (proposal) | Meaning |
|---|---|---|---|---|---|
| `id` | `Long` | no | none | `i64` | Source id. Local source is `0`. |
| `lang` | `String` | no | none | `String` | ISO language tag, or `"all"`, `"other"`, `""` (system). |
| `name` | `String` | no | none | `String` | Display name. May be blank for stubs. |
| `supportsLatest` | `Boolean` | no | none | `bool` | Source defines a "latest" listing. |
| `isStub` | `Boolean` | no | none | `bool` | Source is not installed. |
| `pin` | `Pins` | no | `Pins.unpinned` | `SourceSection` (see 1.2) | Pin bits, see 1.2. |
| `isUsedLast` | `Boolean` | no | `false` | `bool` | Copy inserted into the "Last used" section. |
| `visualName` | computed `String` | - | - | `fn visual_name(&self) -> String` | `name` if `lang` empty, else `"$name (${lang.uppercase()})"`. |
| `key` | computed `() -> String` | - | - | `fn key(&self) -> String` | `"$id-lastused"` if `isUsedLast`, else `"$id"`. Used as LazyColumn key. |

### 1.2 Pin bits (`domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt`)
| Constant | Value | Meaning |
|---|---|---|
| `Pin.Unpinned.code` | `0b00` (0) | no flags |
| `Pin.Pinned.code` | `0b01` (1) | user pinned |
| `Pin.Actual.code` | `0b10` (2) | entry belongs in the "Pinned" section |
| `Pins.unpinned` | `0` | `Pins(Pin.Unpinned)` |
| `Pins.pinned` | `0b11` (3) | `Pins(Pin.Pinned, Pin.Actual)` |

Operations: `contains(p)` = `p.code and code == p.code`; `plus(p)` = OR; `minus(p)` = XOR (`domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt:23`). Quirk: `Pins.unpinned - Pin.Actual` = `0b10`. Harmless (see `p2-behavior.md` 3.2).

Rust proposal (no bit fiddling): `enum SourceSection { LastUsed, Pinned, Language(String) }` with `pinned: bool` and `is_last_used_copy: bool`. Observable behavior is identical (verified in `p2-behavior.md` 3.2).

### 1.3 `StubAnimeSource` (`domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:57-109`)
| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | `Long` | ctor | |
| `lang` | `String` | ctor | `""` when unknown |
| `name` | `String` | ctor | `""` when unknown |
| `isInvalid` | `Boolean` | `name.isBlank() || lang.isBlank()` | used by `toString()` |
| `supportsLatest` | `Boolean` | `false` | |
| all `get*` and `getHosterList`, `getVideoList` | - | throw `AnimeSourceNotInstalledException` | error text "Source not found" (`MR.strings.loader_not_implemented_error`) |

`toString()` = `"$name (${lang.uppercase()})"` if not invalid, else `id.toString()`. Shown in "Source not installed: %1$s".

### 1.4 `Anime` (domain entry model used by browse and favorites)
File: `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:16-41`. `data class Anime : Serializable` with the constructor fields in the table below.

| Field | Kotlin type | Nullable | `Anime.create()` default | Rust (proposal) |
|---|---|---|---|---|
| `id` | `Long` | no | `-1` | `i64` |
| `source` | `Long` | no | `-1` | `i64` |
| `favorite` | `Boolean` | no | `false` | `bool` |
| `lastUpdate` | `Long` | no | `0` | `i64` (ms epoch) |
| `nextUpdate` | `Long` | no | `0` | `i64` |
| `fetchInterval` | `Int` | no | `0` | `i32` |
| `dateAdded` | `Long` | no | `0` | `i64` (ms epoch; `0` = not in library) |
| `viewerFlags` | `Long` | no | `0` | `i64` (not used in scope) |
| `episodeFlags` | `Long` | no | `0` | `i64` (not used in scope) |
| `coverLastModified` | `Long` | no | `0` | `i64` |
| `backgroundLastModified` | `Long` | no | `0` | `i64` |
| `url` | `String` | no | `""` | `String` |
| `title` | `String` | no | `""` | `String` |
| `artist` | `String?` | yes | `null` | `Option<String>` |
| `author` | `String?` | yes | `null` | `Option<String>` |
| `description` | `String?` | yes | `null` | `Option<String>` |
| `genre` | `List<String>?` | yes | `null` | `Option<Vec<String>>` |
| `status` | `Long` | no | `0` | `i64` (SAnime status code, 2.4) |
| `thumbnailUrl` | `String?` | yes | `null` | `Option<String>` |
| `backgroundUrl` | `String?` | yes | `null` | `Option<String>` |
| `updateStrategy` | `AnimeUpdateStrategy` | no | `ALWAYS_UPDATE` | enum (2.5) |
| `initialized` | `Boolean` | no | `false` | `bool` |
| `lastModifiedAt` | `Long` | no | `0` | `i64` |
| `favoriteModifiedAt` | `Long?` | yes | `null` | `Option<i64>` |
| `version` | `Long` | no | `0` | `i64` |
| `fetchType` | `FetchType` | no | `FetchType.Episodes` | enum (2.5) |
| `parentId` | `Long?` | yes | `null` | `Option<i64>` |
| `seasonFlags` | `Long` | no | `0` | `i64` |
| `seasonNumber` | `Double` | no | `-1.0` | `f64` |
| `seasonSourceOrder` | `Long` | no | `0` | `i64` |
| `memo` | `JsonObject` | no | `JsonObject` empty (`{}`) | `serde_json::Map` |

Derived: `expectedNextUpdate` (not used in scope), `sorting`, `displayMode` (episode flags masks, not used here).

Mapping from source (`toDomainAnime(sourceId)`, `app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:92-110`):
| Source field (`SAnime`) | Domain field |
|---|---|
| `url` | `url` |
| `title` | `title` |
| `artist` | `artist` |
| `author` | `author` |
| `description` | `description` |
| `getGenres()` | `genre` |
| `status.toLong()` | `status` |
| `thumbnail_url` | `thumbnailUrl` |
| `background_url` | `backgroundUrl` |
| `update_strategy` | `updateStrategy` |
| `fetch_type` | `fetchType` |
| `season_number` | `seasonNumber` |
| `initialized` | `initialized` |
| `memo` | `memo` |
| (argument) | `source` |
| everything else | `Anime.create()` default |

`getGenres()` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:49-52`): null when `genre` is null or blank; else split on `", "`, trim each, drop blanks, `distinct()`.

### 1.5 `AnimeUpdate` (partial update)
`Anime.toAnimeUpdate()` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt:39-60`) copies every `Anime` field into `AnimeUpdate` (same names). `AnimeUpdate` fields are nullable; `null` = do not change. The repository applies `coalesce(:param, column)` per column (`data/src/main/sqldelightanime/dataanime/animes.sq`, query `update`), so a non-null value from the snapshot overwrites the stored value, including `title`, `url`, `description`, `status`, `lastUpdate`, `nextUpdate`, `viewer`, `episode_flags`, `season_*`, and `memo` (E-BR-02).

### 1.6 `SAnime` (source model, read-only here)
File: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:10-36`.

| Field | Kotlin type | Default (`SAnimeImpl`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnimeImpl.kt:14-36`) |
|---|---|---|
| `url` | `String` | none |
| `title` | `String` | none |
| `thumbnail_url` | `String?` | `null` |
| `background_url` | `String?` | `null` |
| `artist` | `String?` | `null` |
| `author` | `String?` | `null` |
| `status` | `Int` | `0` (UNKNOWN) |
| `description` | `String?` | `null` |
| `genre` | `String?` | `null` |
| `update_strategy` | `AnimeUpdateStrategy` | `ALWAYS_UPDATE` |
| `fetch_type` | `FetchType` | `FetchType.Episodes` |
| `season_number` | `Double` | `-1.0` |
| `initialized` | `Boolean` | `false` |
| `memo` | `JsonObject` | `JsonObject.EMPTY` |

SAnime status constants (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79`): `UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2`, `LICENSED = 3`, `PUBLISHING_FINISHED = 4`, `CANCELLED = 5`, `ON_HIATUS = 6`, `UPCOMING = 7`.

### 1.7 `AnimesPage`
`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimesPage.kt:3`: `class AnimesPage(val animes: List<SAnime>, val hasNextPage: Boolean)`. Rust: `struct AnimesPage { animes: Vec<SAnime>, has_next_page: bool }`.

### 1.8 `AnimeSource` interface methods used in scope
File: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt`

| Member | Line | Signature | Used by |
|---|---|---|---|
| `id` | 22 | `val id: Long` | everything |
| `name` | 27 | `val name: String` | rows, search sort |
| `lang` | 29 | `val lang: String` | grouping, filter |
| `supportsLatest` | 35 | `val supportsLatest: Boolean` | Latest chip and button |
| `getFilterList()` | 40 | `fun getFilterList(): AnimeFilterList = AnimeFilterList()` | browse init, reset, search |
| `getPopularAnime` | 48 | `suspend fun getPopularAnime(page: Int): AnimesPage` | Popular listing |
| `getLatestUpdates` | 56 | `suspend fun getLatestUpdates(page: Int): AnimesPage` | Latest listing |
| `getSearchAnime` | 66 | `suspend fun getSearchAnime(page: Int, query: String, filters: AnimeFilterList): AnimesPage` | search listing, global search |

`AnimeHttpSource.getHomeUrl()` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:54`): `open fun getHomeUrl(): String = baseUrl`.

`ConfigurableAnimeSource` is a marker interface checked with `is` (browse overflow Settings item).

### 1.9 Filter model (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt`)
`sealed class AnimeFilter<T>(val name: String, var state: T)`:

| Variant | Kotlin | State type | Default | Constructor | Rust (proposal) |
|---|---|---|---|---|---|
| `Header` | `open class Header(name) : AnimeFilter<Any>(name, 0)` | `Any` (0) | 0 | name | `Header { name }` |
| `Separator` | `open class Separator(name = "")` | `Any` (0) | 0 | name default `""` | `Separator { name }` |
| `Select<V>` | `abstract class Select<V>(name, values: Array<V>, state: Int = 0)` | `Int` | 0 | | `Select { name, values: Vec<String>, state: usize }` |
| `Text` | `abstract class Text(name, state: String = "")` | `String` | `""` | | `Text { name, state: String }` |
| `CheckBox` | `abstract class CheckBox(name, state: Boolean = false)` | `Boolean` | `false` | | `CheckBox { name, state: bool }` |
| `TriState` | `abstract class TriState(name, state: Int = STATE_IGNORE)` | `Int` | `0` | | `TriState { name, state: TriStateState }` |
| `Group<V>` | `abstract class Group<V>(name, state: List<V>)` | `List<V>` | - | | `Group { name, children: Vec<Filter> }` |
| `Sort` | `abstract class Sort(name, values: Array<String>, state: Selection? = null)` | `Sort.Selection?` | `null` | | `Sort { name, values: Vec<String>, state: Option<SortSelection> }` |

`Sort.Selection` = `data class Selection(val index: Int, val ascending: Boolean)` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:31`).

TriState integers (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:20-24`): `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2`. Helpers: `isIgnored()`, `isIncluded()`, `isExcluded()`.

Equality (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:34-45`): `name == other.name && state == other.state`. Hash: `31 * name.hashCode() + state.hashCode()`.

`AnimeFilterList` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilterList.kt`): `data class AnimeFilterList(val list: List<AnimeFilter<*>>) : List<AnimeFilter<*>> by list`. Overrides `equals` to `return false` (line 10-12) and `hashCode` to `list.hashCode()`. Rust: derive equality over the filter values, never false (see `p2-behavior.md` 5.14).

Source-defined sort order and selection: Sort has no default unless the source sets `state`.

### 1.10 `Listing` (browse)
`sealed class Listing(open val query: String?, open val filters: AnimeFilterList)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:322-345`):

| Variant | `query` | `filters` | Rust (proposal) |
|---|---|---|---|
| `Popular` (`data object`) | `GetRemoteAnime.QUERY_POPULAR` = `"eu.kanade.domain.source.anime.interactor.POPULAR"` | `AnimeFilterList()` | `Listing::Popular` |
| `Latest` (`data object`) | `GetRemoteAnime.QUERY_LATEST` = `"eu.kanade.domain.source.anime.interactor.LATEST"` | `AnimeFilterList()` | `Listing::Latest` |
| `Search(query: String?, filters)` | `query` | `filters` | `Listing::Search { query: Option<String>, filters: FilterList }` |

`Listing.valueOf(query)` maps the two sentinel strings to Popular or Latest, and everything else (including `null`) to `Search(query, AnimeFilterList())`. Rust keeps the sentinel strings only at the navigation boundary, as `enum` variants internally.

Constants: `GetRemoteAnime.QUERY_POPULAR` and `QUERY_LATEST` (`domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt:19-22`).

### 1.11 Browse state and dialogs
`BrowseAnimeSourceScreenModel.State` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:358-366`): `listing: Listing` (required), `filters: AnimeFilterList = AnimeFilterList()`, `toolbarQuery: String? = null`, `dialog: Dialog? = null`; `isUserQuery = listing is Search && !listing.query.isNullOrEmpty()`.

`Dialog` variants (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:347-356`): `Filter` (object); `RemoveAnime(anime: Anime)`; `AddDuplicateAnime(anime: Anime, duplicate: Anime)`; `ChangeAnimeCategory(anime: Anime, initialSelection: ImmutableList<CheckboxState.State<Category>>)`; `Migrate(newAnime: Anime, oldAnime: Anime)`. Note the constructor order of `Migrate` is `(newAnime, oldAnime)` and the call site passes `(dialog.anime, dialog.duplicate)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:271`), so `newAnime` is the browse entry and `oldAnime` is the library duplicate.

`Category` (`domain/src/main/java/tachiyomi/domain/category/model/Category.kt`): `id: Long`, `name: String`, `order: Long`, `flags: Long`, `hidden: Boolean`; `isSystemCategory = id == UNCATEGORIZED_ID` (`0L`).

`CheckboxState.State<T>` (`mapAsCheckboxState`): checkbox model with `Checked`/`Unchecked`-style states (core/common `tachiyomi.core.common.preference.CheckboxState`); the scope only builds it with `mapAsCheckboxState { id in preselected }`.

### 1.12 Global search state
`AnimeSearchScreenModel.State` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:198-209`):
| Field | Type | Default | Rust |
|---|---|---|---|
| `fromSourceId` | `Long?` | `null` | `Option<i64>` (never set by global search) |
| `searchQuery` | `String?` | `null` | `Option<String>` |
| `sourceFilter` | `AnimeSourceFilter` | `PinnedOnly` | enum |
| `onlyShowHasResults` | `Boolean` | `false` | `bool` (mirrors app state) |
| `items` | `PersistentMap<AnimeSource, AnimeSearchItemResult>` | empty | `Vec<(SourceRef, SearchResult)>` sorted (see 4.2) |
| `progress` | computed `Int` | - | `count(non-loading)` |
| `total` | computed `Int` | - | `items.len()` |
| `filteredItems` | computed map | - | filter by visibility |

`enum AnimeSourceFilter { All, PinnedOnly }` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:212-215`).

`sealed interface AnimeSearchItemResult` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:217-234`):
| Variant | Payload | `isVisible(onlyShowHasResults)` |
|---|---|---|
| `Loading` (object) | none | `!onlyShowHasResults` |
| `Error` | `throwable: Throwable` | `!onlyShowHasResults` |
| `Success` | `result: List<Anime>`; `isEmpty = result.isEmpty()` | `!onlyShowHasResults || !isEmpty` |

Rust: `enum SearchResult { Loading, Error(String), Success(Vec<AnimeId>) }` with `error` stored as its message (the UI shows only `message`, E-GS-06).

### 1.13 Sources list UI model
`sealed interface AnimeSourceUiModel` (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:204-207`): `Item(source: AnimeSource)`, `Header(language: String)`. Key rules: `Header` key = `hashCode()`; `Item` key = `"source-${source.key()}"`.

`AnimeSourcesScreenModel.State` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt:113-120`): `dialog: Dialog? = null`, `isLoading: Boolean = true`, `items: ImmutableList<AnimeSourceUiModel> = persistentListOf()`, `isEmpty = items.isEmpty()`. `Dialog(source: AnimeSource)`. `Event.FailedFetchingSources` (one-shot).

### 1.14 Visibility page state
`AnimeSourcesFilterScreenModel.State` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreenModel.kt:61-81`):
- `Loading` (object).
- `Error(throwable: Throwable)`.
- `Success(items: SortedMap<String, List<AnimeSource>>, enabledLanguages: Set<String>, disabledSources: Set<String>)`, `isEmpty = items.isEmpty()`.

`SortedMap` key order: enabled languages first, then `LocaleHelper.comparator` (4.x in `p2-behavior.md`).

### 1.15 Migration sources state
`MigrateAnimeSourceScreenModel.State` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceScreenModel.kt:83-91`):
- `isLoading: Boolean = true`
- `items: ImmutableList<Pair<AnimeSource, Long>>` (source, favorite count)
- `sortingMode: SetMigrateSorting.Mode = ALPHABETICAL`
- `sortingDirection: SetMigrateSorting.Direction = ASCENDING`
- `isEmpty = items.isEmpty()`.
`Event.FailedFetchingSourcesWithCount` (one-shot, not observed by the tab).

### 1.16 Enums in the sort preferences
`SetMigrateSorting` (`app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:14-22`):
- `Mode { ALPHABETICAL, TOTAL }`
- `Direction { ASCENDING, DESCENDING }`
Stored by `name` (`getEnum` serializer `{ it.name }`, `core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:33-40`). Unknown names fall back to the default.

### 1.17 `LibraryDisplayMode` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`)
| Variant | Stored string | Default |
|---|---|---|
| `CompactGrid` | `COMPACT_GRID` | yes (`default`) |
| `ComfortableGrid` | `COMFORTABLE_GRID` | no |
| `List` | `LIST` | no |
| `CoverOnlyGrid` | `COVER_ONLY_GRID` | no |

`deserialize(unknown)` → `CompactGrid`. Browse renders `CoverOnlyGrid` as compact grid (part 1 `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:141`).

### 1.18 `TriState` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`)
`enum class TriState { DISABLED, ENABLED_IS, ENABLED_NOT }` with `next()`: `DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED`. Mapping to `AnimeFilter.TriState` ints: `DISABLED ↔ 0`, `ENABLED_IS ↔ 1`, `ENABLED_NOT ↔ 2`.

---

## 2. Bit layouts and integer codes

| Name | Bits / value | Used in | Notes |
|---|---|---|---|
| Pin | bit0 = Pinned (0b01), bit1 = Actual (0b10), `Pins.pinned` = 0b11 | sources list grouping | section 1.2 |
| TriState filter | 0 IGNORE, 1 INCLUDE, 2 EXCLUDE | source filters, genre search | section 1.9 |
| Sort selection | `index: Int` (`-1` not used; `null` = no selection), `ascending: Boolean` | source sort filter | section 1.9 |
| Category system id | `0` | add flow | `Category.UNCATEGORIZED_ID` |
| Local source id | `0` | source list, global search, migration | `LocalAnimeSource.ID` |
| Stub name/lang | `""` | unknown stub | `app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:125` |
| `fetch_type` column default | `1` | `data/src/main/sqldelightanime/dataanime/animes.sq` | `FetchType` ordinal: `Seasons = 0`, `Episodes = 1` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:10-18`) |
| `update_strategy` column default | `0` | `data/src/main/sqldelightanime/dataanime/animes.sq` | `AnimeUpdateStrategy.ALWAYS_UPDATE` is ordinal 0 (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt:15`) |

---

## 3. Preference keys used in scope

Keys are stored in the app preference store. "App-state" keys carry the prefix `__APP_STATE_` (`Preference.appStateKey`, `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`). The prefix is applied to the key string.

| Storage key (as stored) | Accessor (file:line) | Type | Default | Read by | Written by |
|---|---|---|---|---|---|
| `pref_display_mode_catalogue` | `SourcePreferences.sourceDisplayMode()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:16-21`) | `LibraryDisplayMode` (object pref) | `CompactGrid` | browse toolbar, browse body | browse display-mode menu |
| `source_languages` | `enabledLanguages()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:23-26`) | `Set<String>` | `LocaleHelper.getDefaultEnabledLanguages()` = `{"all", "en", <system language>}` | sources list, visibility page, global search (snapshot) | visibility page language toggle |
| `hidden_anime_catalogues` | `disabledAnimeSources()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:56`) | `Set<String>` of source ids | `{}` | sources list, visibility page, global search (snapshot) | source disable toggles |
| `pinned_anime_catalogues` | `pinnedAnimeSources()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:62`) | `Set<String>` of source ids | `{}` | sources list, global search (snapshot) | pin toggles |
| `__APP_STATE_last_anime_catalogue_source` | `lastUsedAnimeSource()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:65-68`) | `Long` | `-1` | sources list | browse init (unless incognito) |
| `__APP_STATE_has_filters_toggle_state` | `globalSearchFilterState()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:49-52`) | `Boolean` | `false` | global search | "Has results" chip |
| `incognito_anime_extensions` | `incognitoAnimeExtensions()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:59`) | `Set<String>` of package names | `{}` | incognito check | extension settings (area 19) |
| `browse_hide_in_anime_library_items` | `hideInAnimeLibraryItems()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:77-80`) | `Boolean` | `false` | browse (snapshot) | settings (area 47/02) |
| `pref_migration_sorting` | `migrationSortingMode()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:30-33`) | enum name in `SetMigrateSorting.Mode` | `ALPHABETICAL` | migrate tab | migrate tab sort toggle |
| `pref_migration_direction` | `migrationSortingDirection()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:35-38`) | enum name in `SetMigrateSorting.Direction` | `ASCENDING` | migrate tab | migrate tab direction toggle |
| `pref_animelib_columns_portrait_key` | `LibraryPreferences.animePortraitColumns()` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:102`) | `Int` | `0` (adaptive) | browse columns | library settings (area 37) |
| `pref_animelib_columns_landscape_key` | `LibraryPreferences.animeLandscapeColumns()` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:105`) | `Int` | `0` (adaptive) | browse columns | library settings (area 37) |
| `default_anime_category` | `LibraryPreferences.defaultAnimeCategory()` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:153`, key constant `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:414`) | `Int` | `-1` (ask) | add flow | category settings (area 08) |
| `__APP_STATE_incognito_mode` | `BasePreferences.incognitoMode()` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21`) | `Boolean` | `false` | incognito check | incognito toggle (area 36) |

Storage notes:
- Sets are stored as string sets. Source ids are stored in decimal (`"${source.id}"`).
- Enum preferences are stored by enum name.
- `__APP_STATE_` keys are app state, not user settings. Whether backups include them is decided in area 21. Scope does not write any non-app-state key besides the list above.

Rust key mapping (proposal): keep the same key strings in the new settings store, so a Kurobayomi settings file can later be filled from Aniyomi backup data (area 21). Namespacing: `browse.*` for the scope keys, `app_state.*` for `__APP_STATE_` keys.

---

## 4. Files and folders on disk

Scope code writes no files directly. Side effects through other areas:
- `coverCache.deleteFromCache(anime, true)` and `backgroundCache.deleteFromCache(anime, true)` delete cached cover and background files for an anime when it is removed from the library (area:34). Cover and background cache locations are defined in area:34.
- `addTracks.bindEnhancedTrackers` (area:27) and `setAnimeDefaultEpisodeFlags` (area:24/08) may write other rows; their file effects are outside this scope.
- Stub source records are written by `AndroidAnimeSourceManager.registerStubSource` (`app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:106-115`), which may call `downloadManager.renameSource`. Area 17 owns it.

---

## 5. Network endpoints

Scope makes no direct HTTP requests. All remote data comes from the source instance:

| Call | Method name | Parameters | Response | Owner |
|---|---|---|---|---|
| Popular list | `AnimeSource.getPopularAnime(page: Int)` | 1-based page | `AnimesPage` | area:16 (each extension implements HTTP) |
| Latest list | `AnimeSource.getLatestUpdates(page: Int)` | 1-based page | `AnimesPage` | area:16 |
| Search | `AnimeSource.getSearchAnime(page: Int, query: String, filters: AnimeFilterList)` | page, text, filter values | `AnimesPage` | area:16 |
| Home URL (WebView, assist) | `AnimeHttpSource.getHomeUrl()` | none | `baseUrl` string | area:16 |

Paging assumptions (from `AnimeSourcePagingSource`): page numbers start at 1 and increase by 1 (`nextKey = page + 1`). The global search requests page 1 only. A source that returns an empty `animes` list is treated as "no results" (error `NoEpisodesException`).

External links opened from scope (no HTTP from the app):
| Purpose | URL | Opened by |
|---|---|---|
| Browse help (non-local) | `https://aniyomi.org/docs/guides/troubleshooting/` | `Constants.URL_HELP` (`core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:4`) |
| Local source help | `https://aniyomi.org/help/guides/local-anime/` | `LocalAnimeSource.HELP_URL` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:366`) |
| Migration guide | `https://aniyomi.org/help/guides/source-migration/` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:36`) |

---

## 6. Database

Table and queries touched. All anime rows use SQLDelight in `data/src/main/sqldelightanime/dataanime/`.

### 6.1 `animes` (columns used in scope)
| Column | Type | Constraint | Used for |
|---|---|---|---|
| `_id` | `INTEGER` | PK | `Anime.id` |
| `source` | `INTEGER` | NOT NULL | source id |
| `url` | `TEXT` | NOT NULL | lookup key with `source` |
| `title` | `TEXT` | NOT NULL | title; duplicate check uses `LOWER(title)` |
| `favorite` | `INTEGER AS Boolean` | NOT NULL | library membership |
| `date_added` | `INTEGER` | NOT NULL | 0 when not favorite |
| `cover_last_modified` | `INTEGER` | NOT NULL | cover cache key |
| `background_last_modified` | `INTEGER` | NOT NULL | background cache key |
| `thumbnail_url`, `background_url` | `TEXT` | nullable | cover URLs |
| `artist`, `author`, `description` | `TEXT` | nullable | details |
| `genre` | `TEXT AS List<String>` | nullable | genres |
| `status` | `INTEGER` | NOT NULL | SAnime status |
| `initialized` | `INTEGER AS Boolean` | NOT NULL | details fetched |
| `viewer`, `episode_flags` | `INTEGER` | NOT NULL | not used in scope |
| `update_strategy` | `INTEGER AS AnimeUpdateStrategy` | NOT NULL DEFAULT 0 | |
| `fetch_type` | `INTEGER AS FetchType` | NOT NULL DEFAULT 1 | |
| `season_flags`, `season_number`, `season_source_order`, `parent_id` | mixed | mixed | season data (area 39/40) |
| `memo` | `BLOB AS JsonObject` | NOT NULL DEFAULT `'{}'` | |
| `last_update`, `next_update`, `calculate_interval`, `last_modified_at`, `favorite_modified_at`, `version`, `is_syncing` | mixed | mixed | not used by scope except through full-row update |

### 6.2 Queries used by scope
| Purpose | SQL (verbatim fragment) | Location |
|---|---|---|
| Lookup by url and source (every browse item) | `SELECT * FROM animes WHERE url = :url AND source = :source LIMIT 1;` | `data/src/main/sqldelightanime/dataanime/animes.sq:81-85` |
| Insert new browse entry | `INSERT INTO animes(source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, episode_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, version, fetch_type, parent_id, season_flags, season_number, season_source_order, background_url, background_last_modified, memo) VALUES (:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite, :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :episodeFlags, :coverLastModified, :dateAdded, :updateStrategy, :calculateInterval, 0, :version, :fetchType, :parentId, :seasonFlags, :seasonNumber, :seasonSourceOrder, :backgroundUrl, :backgroundLastModified, :memo)` | `data/src/main/sqldelightanime/dataanime/animes.sq:166-168`, executed via `AnimeRepositoryImpl.insertAnime` in `handler.awaitOneOrNullExecutable(inTransaction = true)` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:104`) |
| Live lookup by url and source (per item) | same as above, through `getAnimeByUrlAndSourceIdAsFlow` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:42`) | `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:42` |
| Favorite count per source (migrate tab) | `SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source;` | `data/src/main/sqldelightanime/dataanime/animes.sq:114-120` |
| Duplicate check (add flow) | `SELECT * FROM animes WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id;` (`:title` = `anime.title.lowercase()`) | `data/src/main/sqldelightanime/dataanime/animes.sq:128-133`, `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:72-76` |
| Update (favorite toggle, full row) | `UPDATE animes SET <column> = coalesce(:<param>, <column>)` repeated for each updatable column (`source`, `url`, `artist`, `author`, `description`, `genre`, `title`, `status`, `thumbnail_url`, `favorite`, `last_update`, `next_update`, `initialized`, `viewer`, `episode_flags`, `cover_last_modified`, `date_added`, `update_strategy`, `calculate_interval`, `version`, `is_syncing`, `fetch_type`, `parent_id`, `season_flags`, `season_number`, `season_source_order`, `background_url`, `background_last_modified`, `memo`); full statement at `data/src/main/sqldelightanime/dataanime/animes.sq:170` | `data/src/main/sqldelightanime/dataanime/animes.sq` query `update` (line 170 onward); executed by `partialUpdateAnime` from `AnimeRepositoryImpl.updateAnime` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:140-148`) |

Transaction and failure semantics:
- `updateAnime` returns `false` and logs `ERROR` on any exception (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:140-148`). The browse favorite flow ignores the return value (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:243` discards it), so a failed favorite write is silent.
- `setAnimeCategories` runs inside a transaction: delete all rows for the anime, then insert one row per category id (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:95-102`). `SetAnimeCategories.await` catches exceptions and logs them (`domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetAnimeCategories.kt:11-17`).
- `insertAnime` runs inside a transaction and returns the new `_id` (null if none).

### 6.3 `animes_categories`
Columns: `_id INTEGER PK`, `anime_id INTEGER NOT NULL FK → animes(_id)`, `category_id INTEGER NOT NULL FK → categories(_id) ON DELETE CASCADE` (`data/src/main/sqldelightanime/dataanime/animes_categories.sq:1-8`). Written by `setAnimeCategories` as described above. Read by `getCategoriesByAnimeId` (area 08) for the category picker preselection.

### 6.4 Stub source table
`AnimeStubSourceRepository` (`domain/src/main/java/tachiyomi/domain/source/anime/repository/AnimeStubSourceRepository.kt`) stores `(id, lang, name)` records for uninstalled sources. Table and columns are owned by area:17.

---

## 7. User-visible strings

Resolution: `MR.strings.<key>` from `i18n/src/commonMain/moko-resources/base/strings.xml`; `AYMR.strings.<key>` from `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`. English text shown. Placeholders keep the Android format (`%s`, `%1$s`, `%d`).

### 7.1 Sources and visibility
| Key | Namespace | English text | Used in |
|---|---|---|---|
| `label_anime_sources` | AYMR | "Anime Sources" | Sources tab title |
| `action_global_search` | MR | "Global search" | Sources toolbar |
| `action_filter` | MR | "Filter" | Sources toolbar, browse chip, filter sheet button |
| `internal_error` | MR | "InternalError: Check crash logs for further information" | failure snackbar and visibility toast |
| `source_empty_screen` | MR | "No source found" | sources list empty |
| `action_pin` | MR | "Pin" | pin button and dialog |
| `action_unpin` | MR | "Unpin" | pin button and dialog |
| `action_disable` | MR | "Disable" | dialog |
| `last_used_source` | MR | "Last used" | section header |
| `pinned_sources` | MR | "Pinned" | section header, global search chip |
| `other_source` | MR | "Other" | section header |
| `multi_lang` | MR | "Multi" | section header for `all` |
| `label_sources` | MR | "Sources" | visibility page title |
| `source_filter_empty_screen` | MR | "No installed source found" | visibility page empty |
| `latest` | MR | "Latest" | chip, button |
| `popular` | MR | "Popular" | chip |
| `local_anime_source` | AYMR | "Local anime source" | local source name |

### 7.2 Browse
| Key | Namespace | English text | Used in |
|---|---|---|---|
| `action_display_mode` | MR | "Display mode" | toolbar button content description |
| `action_display_comfortable_grid` | MR | "Comfortable grid" | radio item |
| `action_display_grid` | MR | "Compact grid" | radio item |
| `action_display_list` | MR | "List" | radio item |
| `label_help` | MR | "Help" | overflow (local) and empty-state action |
| `action_open_in_web_view` | MR | "Open in WebView" | overflow and empty-state action |
| `action_settings` | MR | "Settings" | overflow (configurable source) |
| `action_retry` | MR | "Retry" | empty-state and snackbar action |
| `local_source_help_guide` | MR | "Local source guide" | empty-state action (local) |
| `source_not_installed` | MR | "Source not installed: %1$s" | missing-source screen |
| `are_you_sure` | MR | "Are you sure?" | remove and duplicate dialog title |
| `remove_manga` | MR | "You are about to remove \"%s\" from your library" | remove dialog text |
| `action_cancel` | MR | "Cancel" | dialog buttons |
| `action_remove` | MR | "Remove" | remove dialog confirm |
| `confirm_add_duplicate_manga` | MR | "You have an entry in your library with the same name.\n\nDo you still wish to continue?" | duplicate dialog text (manga key reused for anime) |
| `action_show_anime` | AYMR | "Show anime" | duplicate dialog |
| `action_migrate_duplicate` | MR | "Migrate existing entry" | duplicate dialog |
| `action_add_anyway` | MR | "Add anyway" | duplicate dialog |
| `action_reset` | MR | "Reset" | filter sheet |
| `no_results_found` | MR | "No results found" | empty search (`NoEpisodesException` text) and global card row |
| `loader_not_implemented_error` | MR | "Source not found" | `AnimeSourceNotInstalledException` text |
| `exception_http` | MR | "HTTP %d, check website in WebView" | HTTP error text |
| `exception_offline` | MR | "No Internet connection" | offline error text |
| `exception_unknown_host` | MR | "Couldn't reach %s" | DNS error text |
| `torrserver_disabled` | AYMR | "TorrServer is not running" | torrent source error text |
| `label_migration_anime` | AYMR | "Migrate Anime" | migrate tab title |

### 7.3 Global search
| Key | Namespace | English text | Used in |
|---|---|---|---|
| `has_results` | MR | "Has results" | chip |
| `all` | MR | "All" | chip |
| `pinned_sources` | MR | "Pinned" | chip |

### 7.4 Migration sources tab
| Key | Namespace | English text | Used in |
|---|---|---|---|
| `migration_selection_prompt` | MR | "Select a source to migrate from" | sticky header |
| `action_sort_alpha` | MR | "Alphabetically" | sort-mode icon description |
| `action_sort_count` | MR | "Total entries" | sort-mode icon description |
| `action_asc` | MR | "Ascending" | direction icon description |
| `action_desc` | MR | "Descending" | direction icon description |
| `information_empty_library` | MR | "Your library is empty" | empty state |
| `not_installed` | MR | "Not installed" | stub source label |
| `migration_help_guide` | MR | "Source migration guide" | help action |

### 7.5 Unused in this scope but referenced by the part 1 components
`action_pin`/`action_unpin` appear on both sides; no other key is needed here.

---

## 8. Constants summary

| Constant | Value | Location |
|---|---|---|
| Paging page size | `25` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:112` |
| Global search thread pool | `5` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:46` |
| Global search page number | `1` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:160` |
| Adaptive grid cell | `128.dp` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:134` |
| Pin.Pinned / Pin.Actual / Pins.pinned | `1` / `2` / `3` | `domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt` |
| `LocalAnimeSource.ID` | `0L` | `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:365` |
| `Category.UNCATEGORIZED_ID` | `0L` | `domain/src/main/java/tachiyomi/domain/category/model/Category.kt` |
| `default_anime_category` default | `-1` | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:153` |
| `last_anime_catalogue_source` default | `-1` | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:67` |
| `QUERY_POPULAR` | `"eu.kanade.domain.source.anime.interactor.POPULAR"` | `domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt:20` |
| `QUERY_LATEST` | `"eu.kanade.domain.source.anime.interactor.LATEST"` | `domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt:21` |
| `PINNED_KEY` | `"pinned"` | `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:84` |
| `LAST_USED_KEY` | `"last_used"` | `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:85` |
| TriState ints | `0 / 1 / 2` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:21-23` |
| Snackbar durations | Indefinite (paging), Short (sources failure) | `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:69` (part 1), `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:79` |
| Grid content padding | `8.dp` | `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceComfortableGrid.kt:33` (part 1) |
| Grid spacing | `4.dp` | `app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:50-51` |
| Favorite cover alpha | `0.34` | `app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:52` |
| Secondary alpha | `0.78` | `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:10` |
| Card width (global) | `96.dp` | `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt:64` (part 1) |
| Card title max lines (global) | `3` | `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt:67` (part 1) |
