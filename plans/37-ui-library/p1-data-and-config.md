# 37-ui-library — Data models, preferences, storage and config

Paths are repo-relative to `/home/blackbox/aniyomi-src/`.

Sections: D-1 models · D-2 enums and codecs · D-3 preference keys (anime / manga / shared) · D-4 sort flag bit layout · D-5 category rows · D-6 shared preference map · D-7 SQL views and tables · D-8 files and caches · D-9 network (none in scope) · D-10 other preferences read here · D-11 category-delete cascade keys · D-12 test vectors for codecs.

---

## D-1. Models

### D-1.1 `LibraryAnime` — `domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt:5-25`

| Field | Type | Nullable | Source / meaning |
|---|---|---|---|
| `anime` | `Anime` | no | Full entry (`tachiyomi.domain.entries.anime.model.Anime`, area:05). |
| `category` | `Long` | no | `coalesce(MC.category_id, 0)` (`animelibView.sq:32`). One row per `animes_categories` link. |
| `totalCount` | `Long` | no | Fetch type episodes: `coalesce(ES.total, 0)`; seasons: `coalesce(ASS.child_count, 0)`. |
| `seenCount` | `Long` | no | Episodes: `coalesce(ES.seenCount, 0)` (`sum(seen)`); seasons: `coalesce(ASS.fully_seen_seasons, 0)`. |
| `bookmarkCount` | `Long` | no | Episodes: `sum(bookmark)`; seasons: `sum(total_bookmarks)`. |
| `fillermarkCount` | `Long` | no | Episodes: `sum(fillermark)`; seasons: `sum(total_fillermarks)`. |
| `latestUpload` | `Long` | no | `coalesce(max(date_upload), 0)`. |
| `episodeFetchedAt` | `Long` | no | `coalesce(max(date_fetch), 0)`. |
| `lastSeen` | `Long` | no | Episodes: `coalesce(AHS.lastSeen, 0)` (`max(animehistory.last_seen)`); seasons: `max_last_seen`. |

Derived (computed, not stored):
- `id: Long = anime.id`.
- `unseenCount = totalCount - seenCount` (Long; can be negative only if counts are inconsistent).
- `hasBookmarks = bookmarkCount > 0`.
- `hasStarted = seenCount > 0`.

`LibraryAnime` is a `data class`: equality compares all fields (including the embedded `Anime`). Selection uses it (E-09).

### D-1.2 `LibraryManga` — `domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt:5-24`

| Field | Type | Source |
|---|---|---|
| `manga` | `Manga` | Full entry. |
| `category` | `Long` | `coalesce(MC.category_id, 0)`. |
| `totalChapters` | `Long` | `coalesce(C.total, 0)` where `C` = `count(*)` over non-excluded-scanlator chapters. |
| `readCount` | `Long` | `coalesce(C.readCount, 0)` = `sum(read)`. |
| `bookmarkCount` | `Long` | `sum(chapters.bookmark)`. |
| `latestUpload` | `Long` | `max(chapters.date_upload)`. |
| `chapterFetchedAt` | `Long` | `max(chapters.date_fetch)`. |
| `lastRead` | `Long` | `max(history.last_read)`. |

Derived: `id`, `unreadCount = totalChapters - readCount`, `hasBookmarks`, `hasStarted = readCount > 0`. (No `fillermarkCount`.)

### D-1.3 `AnimeLibraryItem` — `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryItem.kt:9-16`

```kotlin
data class AnimeLibraryItem(
    val libraryAnime: LibraryAnime,
    var downloadCount: Long = -1,
    var unseenCount: Long = -1,
    var isLocal: Boolean = false,
    var sourceLanguage: String = "",
    private val sourceManager: AnimeSourceManager = Injekt.get(),
)
```
Defaults `-1`/`""`/`false` are placeholders; `getLibraryFlow` always overwrites them (`AnimeLibraryScreenModel.kt:387-402`). `unseenCount` is badge-gated (0 when the unread badge is off). The manga version names the field `unreadCount` and is a non-data `class` (`MangaLibraryItem.kt:9`).

### D-1.4 `AnimeLibraryMap` — `AnimeLibraryScreenModel.kt:82`
`typealias AnimeLibraryMap = Map<Category, List<AnimeLibraryItem>>` — insertion-ordered by category list order. Manga: `MangaLibraryMap` (`MangaLibraryScreenModel.kt:80`).

### D-1.5 `AnimeLibraryScreenModel.State` — `AnimeLibraryScreenModel.kt:755-811` (`@Immutable data class`)

| Field | Type | Default |
|---|---|---|
| `isLoading` | Boolean | `true` |
| `library` | `AnimeLibraryMap` | `emptyMap()` |
| `searchQuery` | String? | `null` |
| `selection` | `PersistentList<LibraryAnime>` | `persistentListOf()` |
| `hasActiveFilters` | Boolean | `false` |
| `showCategoryTabs` | Boolean | `false` |
| `showAnimeCount` | Boolean | `false` |
| `showAnimeContinueButton` | Boolean | `false` |
| `dialog` | `Dialog?` | `null` |

Derived: `libraryCount` (lazy; distinct `anime.id` over all page lists), `isLibraryEmpty` (lazy; `libraryCount == 0`), `selectionMode = selection.isNotEmpty()`, `categories = library.keys.toList()`. Functions: `getAnimelibItemsByCategoryId(id)`, `getAnimelibItemsByPage(page)` (page index into `library.values`), `getAnimeCountForCategory(category)`, `getToolbarTitle(...)`.

Manga state: same shape with `showChapterCount`-style names: `showAnimeCount` → `showMangaCount`, `showAnimeContinueButton` → `showMangaContinueButton`, `getMangalibItemsByCategoryId` → `getLibraryItemsByCategoryId`.

### D-1.6 `AnimeLibraryScreenModel.Dialog` — `:729-736`
```kotlin
sealed interface Dialog {
    data object SettingsSheet : Dialog
    data class ChangeCategory(val anime: List<Anime>, val initialSelection: ImmutableList<CheckboxState<Category>>) : Dialog
    data class DeleteAnime(val anime: List<Anime>) : Dialog
}
```
Manga: `SettingsSheet`, `ChangeCategory(manga: List<Manga>, initialSelection)`, `DeleteManga(manga: List<Manga>)` (`MangaLibraryScreenModel.kt:714-721`).

### D-1.7 `ItemPreferences` (private) — `AnimeLibraryScreenModel.kt:738-753`
Fields: `downloadBadge`, `unseenBadge`, `localBadge`, `languageBadge`, `skipOutsideReleasePeriod` (Boolean); `globalFilterDownloaded` (Boolean); `filterDownloaded`, `filterUnseen`, `filterStarted`, `filterBookmarked`, `filterCompleted`, `filterIntervalCustom` (TriState). Manga: `unreadBadge`, `filterUnread`.

### D-1.8 `Category` — `domain/src/main/java/tachiyomi/domain/category/model/Category.kt:5-18`
```kotlin
data class Category(val id: Long, val name: String, val order: Long, val flags: Long, val hidden: Boolean) : Serializable {
    val isSystemCategory: Boolean = id == UNCATEGORIZED_ID   // UNCATEGORIZED_ID = 0L
}
```
`visualName` (`presentation/category/CategoryExtensions.kt:10-21`): system → `label_default` ("Default"); else `name`.

### D-1.9 `CategoryUpdate` (partial update) — referenced by `HideAnimeCategory`, `RenameAnimeCategory`, `ReorderAnimeCategory`, `SetSortModeForAnimeCategory`, `DeleteAnimeCategory`. Fields (nullable = "do not change"): `id: Long`, `name: String?`, `order: Long?`, `flags: Long?`, `hidden: Boolean?`. Confirm exact nullability in `domain/src/main/java/tachiyomi/domain/category/model/CategoryUpdate.kt` when implementing (not re-read in this pass; only the constructor calls in scope were checked).

### D-1.10 `CheckboxState<T>` — `core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt`
- `State<T>`: `Checked(value)`, `None(value)`; `isChecked` = `this is Checked`; `next()`: Checked→None, None→Checked.
- `TriState<T>`: `Include(value)`, `Exclude(value)`, `None(value)`; `next()`: Exclude→None, Include→Exclude, None→Include.
- `asCheckboxState(condition)`, `mapAsCheckboxState` helpers.

### D-1.11 `TriState` — `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`
`enum class TriState { DISABLED, ENABLED_IS, ENABLED_NOT }`, `next()`: DISABLED→ENABLED_IS→ENABLED_NOT→DISABLED.

### D-1.12 `LibraryDisplayMode` — `domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`
`sealed interface` with `CompactGrid`, `ComfortableGrid`, `List`, `CoverOnlyGrid` (data objects). `default = CompactGrid`. `values` set of four. Serializer in D-2.

### D-1.13 `Flag` / `Mask` / `FlagWithMask` — `domain/src/main/java/tachiyomi/domain/library/model/Flag.kt`
- `Long.plus(other: Flag)`: if `other is Mask` → `(this and other.mask.inv()) or (other.flag and other.mask)`; else `this or other.flag`.
- `Flag.plus(other)` same for `this.flag`.
- `Long.contains(other: Flag)`: if Mask → `other.flag == this and other.mask`; else `other.flag == this`.

These are the only bit operations used by the library. Rust: `fn merge(old: i64, flag: i64, mask: i64) -> i64 { (old & !mask) | (flag & mask) }`.

### D-1.14 `AnimeLibrarySort` / `MangaLibrarySort` — `domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt`, `domain/.../library/manga/model/MangaLibrarySortMode.kt`
`data class AnimeLibrarySort(type: Type, direction: Direction) : FlagWithMask` (`flag = type + direction`, `mask = type.mask or direction.mask`, `isAscending`). Nested `sealed class Type(flag)` with `mask = 0b00111100`, `sealed class Direction(flag)` with `mask = 0b01000000`. Companion: `types`, `directions`, `default = AnimeLibrarySort(Type.Alphabetical, Direction.Ascending)`, `valueOf(flag: Long?)`, `deserialize`. Extension `val Category?.sort: AnimeLibrarySort = AnimeLibrarySort.valueOf(this?.flags)`.

---

## D-2. Enum and codec tables

### D-2.1 `LibraryDisplayMode` string codec (`LibraryDisplayMode.kt:24-41`)
| Value | Serialized |
|---|---|
| CompactGrid | `COMPACT_GRID` |
| ComfortableGrid | `COMFORTABLE_GRID` |
| CoverOnlyGrid | `COVER_ONLY_GRID` |
| List | `LIST` |
Deserialize: any other string → `CompactGrid` (default).

### D-2.2 `TriState` storage
`LibraryPreferences` uses `getEnum` (`core/common/.../PreferenceStore.kt:27-37`): stored as the enum **name**: `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`. Unknown names → the default `DISABLED` (`getEnum` catches `IllegalArgumentException` from `enumValueOf` and returns `defaultValue`, `PreferenceStore.kt:35-41`).

### D-2.3 Anime sort token codec (`AnimeLibrarySortMode.kt:91-141`)
Serialized as `"<TYPE>,<DIRECTION>"`.
| Type | Token | Flag |
|---|---|---|
| Alphabetical | `ALPHABETICAL` | 0b00000000 |
| LastSeen | `LAST_SEEN` | 0b00000100 |
| LastUpdate | `LAST_ANIME_UPDATE` | 0b00001000 |
| UnseenCount | `UNSEEN_COUNT` | 0b00001100 |
| TotalEpisodes | `TOTAL_EPISODES` | 0b00010000 |
| LatestEpisode | `LATEST_EPISODE` | 0b00010100 |
| EpisodeFetchDate | `EPISODE_FETCH_DATE` | 0b00011000 |
| DateAdded | `DATE_ADDED` | 0b00011100 |
| TrackerMean | `TRACKER_MEAN` | 0b000100000 (= 32) |
| AiringTime | `AIRING_TIME` | 0b00110000 (= 48) |
| Random | `RANDOM` | 0b00111100 (= 60) |

Direction: `ASCENDING` (flag 0b01000000) or `DESCENDING` (flag 0). Serialize writes `ASCENDING` only for Ascending, otherwise `DESCENDING`.
Deserialize: empty string → default. Unknown type token → `Alphabetical`. Direction: exactly `ASCENDING` → Ascending, anything else → Descending. Any exception (e.g., missing comma) → default.

### D-2.4 Manga sort token codec (`MangaLibrarySortMode.kt:97-137`)
| Type | Token | Flag |
|---|---|---|
| Alphabetical | `ALPHABETICAL` | 0 |
| LastRead | `LAST_READ` | 4 |
| LastUpdate | `LAST_MANGA_UPDATE` | 8 |
| UnreadCount | `UNREAD_COUNT` | 12 |
| TotalChapters | `TOTAL_CHAPTERS` | 16 |
| LatestChapter | `LATEST_CHAPTER` | 20 |
| ChapterFetchDate | `CHAPTER_FETCH_DATE` | 24 |
| DateAdded | `DATE_ADDED` | 28 |
| TrackerMean | `TRACKER_MEAN` | 32 |
| Random | `RANDOM` | 60 |

No AiringTime. Same direction rules.

### D-2.5 Test vectors (for T-37a-051)
| Input | Decoded |
|---|---|
| `""` | Alphabetical, Ascending |
| `"UNSEEN_COUNT,DESCENDING"` | UnseenCount, Descending |
| `"RANDOM,ASCENDING"` | Random, Ascending |
| `"FOO,ASCENDING"` | Alphabetical, Ascending |
| `"LAST_SEEN"` (no comma) | exception → default (Alphabetical, Ascending) |
| flags `0` | Alphabetical, Descending (see §B-5.1 / E-20) |
| flags `0x40` | Alphabetical, Ascending |
| flags `60 | 0x40` = `124` | Random, Ascending |
| flags `48` (anime) | AiringTime, Descending |
| flags `48` (manga) | Type.valueOf → no manga type with flag 48 → falls back to `default.type` = Alphabetical |

Manga flag `48` falls back to Alphabetical because `types` lacks AiringTime. This is the only case where the anime and manga codecs disagree on a stored flag value.

---

## D-3. Preference keys (library-scope) with defaults

Source: `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt`.

Legend: **[A]** anime-only key; **[M]** manga-only key; **[S]** shared by anime and manga UI in this scope; **[AS]** `__APP_STATE_` prefixed (internal state, not user settings). `APP_STATE_PREFIX = "__APP_STATE_"` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:47`).

### D-3.1 Display and layout
| Key | Default | Type | Scope | Accessor / line |
|---|---|---|---|---|
| `pref_display_mode_library` | `CompactGrid` | object (D-2.1) | [S] | `displayMode()` :18-23 |
| `pref_animelib_columns_portrait_key` | `0` | Int | [A] | `animePortraitColumns()` :102 |
| `pref_library_columns_portrait_key` | `0` | Int | [M] | `mangaPortraitColumns()` :103 |
| `pref_animelib_columns_landscape_key` | `0` | Int | [A] | `animeLandscapeColumns()` :105 |
| `pref_library_columns_landscape_key` | `0` | Int | [M] | `mangaLandscapeColumns()` :106 |
| `display_download_badge` | `false` | Boolean | [S] | `downloadBadge()` :81 |
| `display_unread_badge` | `true` | Boolean | [S] | `unreadBadge()` :83 (label "unseen" for anime) |
| `display_local_badge` | `true` | Boolean | [S] | `localBadge()` :85 |
| `display_language_badge` | `false` | Boolean | [S] | `languageBadge()` :87 |
| `display_continue_reading_button` | `false` | Boolean | [S] | `showContinueViewingButton()` :61-62 |
| `display_category_tabs` | `true` | Boolean | [S] | `categoryTabs()` :66 |
| `display_number_of_items` | `false` | Boolean | [S] | `categoryNumberOfItems()` :68 |
| `library_show_updates_count` | `true` | Boolean | not in scope (area:26/35) | `newShowUpdatesCount()` :89 |

### D-3.2 Sorting
| Key | Default | Type | Scope | Line |
|---|---|---|---|---|
| `animelib_sorting_mode` | `AnimeLibrarySort.default` = `ALPHABETICAL,ASCENDING` | string codec (D-2.3) | [A] | `animeSortingMode()` :32-37 |
| `library_sorting_mode` | `MangaLibrarySort.default` = `ALPHABETICAL,ASCENDING` | string codec (D-2.4) | [M] | `mangaSortingMode()` :25-30 |
| `library_random_anime_sort_seed` | `0` | Int | [A] | `randomAnimeSortSeed()` :97 |
| `library_random_manga_sort_seed` | `0` | Int | [M] | `randomMangaSortSeed()` :98 |
| `categorized_display` | `false` | Boolean | [S] | `categorizedDisplaySettings()` :70 |
| `hidden_categories` | `false` | Boolean | [S] | `hideHiddenCategoriesSettings()` :72 |

### D-3.3 Filters (TriState, default DISABLED)
| Key | Scope | Accessor line |
|---|---|---|
| `pref_filter_animelib_downloaded_v2` | [A] | `filterDownloadedAnime()` :110-111 |
| `pref_filter_library_downloaded_v2` | [M] | `filterDownloadedManga()` :113-114 |
| `pref_filter_animelib_unread_v2` | [A] (anime "unseen") | `filterUnseen()` :116-117 |
| `pref_filter_library_unread_v2` | [M] | `filterUnread()` :119-120 |
| `pref_filter_animelib_started_v2` | [A] | `filterStartedAnime()` :122-123 |
| `pref_filter_library_started_v2` | [M] | `filterStartedManga()` :125-126 |
| `pref_filter_animelib_bookmarked_v2` | [A] | `filterBookmarkedAnime()` :128-129 |
| `pref_filter_library_bookmarked_v2` | [M] | `filterBookmarkedManga()` :131-132 |
| `pref_filter_animelib_completed_v2` | [A] | `filterCompletedAnime()` :134-135 |
| `pref_filter_library_completed_v2` | [M] | `filterCompletedManga()` :137-138 |
| `pref_filter_animelib_tracked_${id}_v2` | [A] | `filterTrackedAnime(id: Int)` :140-141 |
| `pref_filter_library_tracked_${id}_v2` | [M] | `filterTrackedManga(id: Int)` :143-144 |
| `pref_filter_library_interval_custom` | [S] | `filterIntervalCustom()` :74-77 |

Note: the `_v2` suffix. A v1 key is not read anywhere in scope.

### D-3.4 Update-related keys read by library UI
| Key | Default | Type | Used for | Line |
|---|---|---|---|---|
| `library_update_manga_restriction` (StringSet) | `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` (the list is `ENTRY_HAS_UNVIEWED, ENTRY_NON_COMPLETED, ENTRY_NON_VIEWED, ENTRY_OUTSIDE_RELEASE_PERIOD`) | Set<String> | [S] — gate for interval filter and skip logic | `autoUpdateItemRestrictions()` :49-57 |
| `pref_library_update_interval_key` | `0` | Int | not used in scope | :40 |
| `library_update_last_timestamp` (app state) | `0` | Long | not used in scope | :39 |

Restriction tokens (`LibraryPreferences.kt:403-406`): `manga_ongoing` (ENTRY_NON_COMPLETED), `manga_fully_read` (ENTRY_HAS_UNVIEWED), `manga_started` (ENTRY_NON_VIEWED), `manga_outside_release_period` (ENTRY_OUTSIDE_RELEASE_PERIOD). The token prefix `manga_` is used for anime too.

### D-3.5 Category-related state
| Key | Default | Type | Scope | Line |
|---|---|---|---|---|
| `__APP_STATE_last_used_anime_category` | `0` | Int | [AS][A] | `lastUsedAnimeCategory()` :156 |
| `__APP_STATE_last_used_category` | `0` | Int | [AS][M] | `lastUsedMangaCategory()` :157 |
| `default_anime_category` | `-1` | Int | not read in library UI (used when adding entries; area:08) | `defaultAnimeCategory()` :153 |
| `default_category` | `-1` | Int | manga equivalent | `defaultMangaCategory()` :154 |
| `animelib_update_categories` | `{}` | StringSet of ids | update include list | :159-160 |
| `animelib_update_categories_exclude` | `{}` | StringSet | update exclude list | :165-166 |
| `library_update_categories`, `library_update_categories_exclude` | `{}` | StringSet | manga counterparts | :162-169 |

`defaultAnimeCategory` deletion on category delete: §D-11.

### D-3.6 Other keys read by the library UI
| Key | Default | Type | Scope | Source |
|---|---|---|---|---|
| `__APP_STATE_pref_downloaded_only` | `false` | Boolean | global downloaded-only, `BasePreferences.downloadedOnly()` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-19`) | [AS] |
| `pref_always_use_external_player` | `false` | Boolean | `PlayerPreferences.alwaysUseExternalPlayer()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt:67-70`) | not app-state |
| nav style pref (area:36) | per `NavStyle` | enum | `currentNavigationStyle()` (`app/src/main/java/eu/kanade/presentation/util/Navigator.kt:43`) | area:36 |
| `mark_duplicate_*`, `default_*_filter_*`, `default_*_sort_*` | various | — | entry-level defaults, area:07 (not library UI) | `LibraryPreferences.kt:172-349` |

### D-3.7 Preference binding semantics
- Each `Preference<T>` offers `get()`, `set()`, `delete()`, `changes()` (Flow), `asState(scope)` (Compose state).
- Writes are synchronous to the store and the change flow fires.
- Rust: wrap each key in a typed struct with `get`, `set`, and a `watch::Receiver` (or glib signal). Keys above are the migration contract.

---

## D-4. Sort flag bit layout (category `flags` column)

Both anime and manga categories store sort in the `flags` INTEGER column. Layout:

```
bit:   7 6 5 4 3 2 1 0
       - D T T T T - -        D = direction (bit 6), T = type (bits 2-5)
```
- Type mask: `0b00111100` (0x3C = 60). Shift: none; the type value is stored at its own bit position (e.g., LastSeen = 4).
- Direction mask: `0b01000000` (0x40 = 64). Ascending sets bit 6.
- Bits 0-1 and 7+ are unused by sort. Preserve them on write (`(old & !mask) | (new & mask)`).

Complete flag table:

| Anime type | value | Manga type | value |
|---|---|---|---|
| Alphabetical | 0 | Alphabetical | 0 |
| LastSeen | 4 | LastRead | 4 |
| LastUpdate | 8 | LastUpdate | 8 |
| UnseenCount | 12 | UnreadCount | 12 |
| TotalEpisodes | 16 | TotalChapters | 16 |
| LatestEpisode | 20 | LatestChapter | 20 |
| EpisodeFetchDate | 24 | ChapterFetchDate | 24 |
| DateAdded | 28 | DateAdded | 28 |
| TrackerMean | 32 | TrackerMean | 32 |
| AiringTime | 48 | — | — |
| Random | 60 | Random | 60 |
| Direction Ascending | 64 (0x40) | Direction Ascending | 64 (0x40) |
| Direction Descending | 0 | Direction Descending | 0 |

Notes on the decode rule (`Type.valueOf`): finds the first type `t` where `t.flag == (flags & t.mask)`. Because `Alphabetical` has flag 0 and `Random` mask bits overlap, lookup is done on the masked value (`flag and type.mask`). The code is `types.find { type -> type.flag == flag and type.mask } ?: default.type`. Equivalent Rust: `types.iter().find(|t| t.flag == (flags & t.mask)).unwrap_or(default)`. Note the comparison uses `flag and type.mask`, i.e., masked by the **type's** mask, which is always `0b00111100`.

---

## D-5. Category rows

### D-5.1 Table `categories` (anime: `data/src/main/sqldelightanime/dataanime/categories.sq`; manga: `data/src/main/sqldelight/data/categories.sq`)
```
categories(
  _id     INTEGER NOT NULL PRIMARY KEY,
  name    TEXT NOT NULL,
  sort    INTEGER NOT NULL,        -- Category.order
  flags   INTEGER NOT NULL,        -- sort bits (D-4)
  hidden  INTEGER NOT NULL DEFAULT 0
)
```
Seed: `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);` (`categories.sq:10`; also migration `120.sqm:2`). The `_id = 0` row is the system ("Default") category, `hidden = 0`, `flags = 0`.

Queries used in scope (anime; manga analogous):
- `getVisibleCategories`: `SELECT _id AS id, name, sort AS order, flags, hidden FROM categories WHERE hidden = 0 ORDER BY sort;` (`categories.sq:36-44`).
- `getAllCategories` (not shown in excerpt; all rows ordered by `sort`).
- `getCategoriesByAnimeId`: join `animes_categories` on `category_id` where `anime_id = :animeId`.
- `getVisibleCategoriesByAnimeId`: same join plus `AND C.hidden = 0` (`categories.sq:59-72`).
- `insert`: `INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags);` (`:76-77`).
- `delete`: `DELETE FROM categories WHERE _id = :categoryId;`.
- `update`: partial update by id.

### D-5.2 Table `animes_categories` (anime) / `mangas_categories` (manga)
```
animes_categories(_id INTEGER PK, anime_id INTEGER NOT NULL, category_id INTEGER NOT NULL, FK category_id → categories(_id))
```
Membership is many-to-many. An entry with no row is in category 0 via `coalesce(MC.category_id, 0)`.

Replace-all write: `AnimeRepositoryImpl.setAnimeCategories` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:95-102`) runs in a transaction: `deleteAnimeCategoryByAnimeId(animeId)`, then `insert(animeId, categoryId)` for each id.

### D-5.3 Category domain record in the UI
`Category(id, name, order, flags, hidden)`. Visible list = `hidden = 0`; management list = all (when `hidden_categories` is off) or visible (when on). Both exclude `isSystemCategory` in the management screens (`AnimeCategoryScreenModel.kt:53`).

---

## D-6. Shared preference map (anime and manga read the same key)

These keys are read by both the anime and manga library UI. Changing one changes both. Rust must decide per key (T-37a-049 records the decision). The upstream behavior is listed so the decision is explicit.

| Key | Used by anime and manga for | Upstream behavior |
|---|---|---|
| `pref_display_mode_library` | layout mode | Shared (one mode for both libraries). |
| `display_download_badge` | download badge | Shared. |
| `display_unread_badge` | unseen/unread badge | Shared. Label differs per kind. |
| `display_local_badge` | local badge | Shared. |
| `display_language_badge` | language badge | Shared. |
| `display_continue_reading_button` | continue button | Shared. |
| `display_category_tabs` | tabs visibility | Shared. |
| `display_number_of_items` | category counts | Shared. |
| `hidden_categories` | management visibility | Shared. |
| `categorized_display` | per-category sort | Shared. |
| `pref_filter_library_interval_custom` | interval filter | Shared. |
| `library_update_manga_restriction` | skip/gate logic | Shared (set holds `manga_` and anime tokens). |
| `__APP_STATE_pref_downloaded_only` | downloaded-only | Shared (app-wide). |

Per-kind (not shared): sort mode keys, sort seeds, columns (portrait/landscape), all filter keys, last-used category index.

---

## D-7. SQL views and tables touched

### D-7.1 `animelibView` (anime library rows) — `data/src/main/sqldelightanime/view/animelibView.sq`
```sql
CREATE VIEW animelibView AS
SELECT M.*,
  CASE M.fetch_type WHEN 1 THEN coalesce(ES.total,0)        WHEN 0 THEN coalesce(ASS.child_count,0) END AS totalCount,
  CASE M.fetch_type WHEN 1 THEN coalesce(ES.seenCount,0)    WHEN 0 THEN coalesce(ASS.fully_seen_seasons,0) END AS seenCount,
  CASE M.fetch_type WHEN 1 THEN coalesce(ES.latestUpload,0) WHEN 0 THEN coalesce(ASS.max_latest_upload,0) END AS latestUpload,
  CASE M.fetch_type WHEN 1 THEN coalesce(ES.fetchedAt,0)    WHEN 0 THEN coalesce(ASS.max_fetched_at,0) END AS episodeFetchedAt,
  CASE M.fetch_type WHEN 1 THEN coalesce(AHS.lastSeen,0)    WHEN 0 THEN coalesce(ASS.max_last_seen,0) END AS lastSeen,
  CASE M.fetch_type WHEN 1 THEN coalesce(ES.bookmarkCount,0) WHEN 0 THEN coalesce(ASS.total_bookmarks,0) END AS bookmarkCount,
  CASE M.fetch_type WHEN 1 THEN coalesce(ES.fillermarkCount,0) WHEN 0 THEN coalesce(ASS.total_fillermarks,0) END AS fillermarkCount,
  coalesce(MC.category_id, 0) AS category
FROM animes M
LEFT JOIN episodestatsView AS ES ON M._id = ES.anime_id
LEFT JOIN animehistorystatsView AS AHS ON M._id = AHS.anime_id
LEFT JOIN animes_categories AS MC ON MC.anime_id = M._id
LEFT JOIN animeseasonstatsView AS ASS ON M._id = ASS.parent_id
WHERE M.favorite = 1;
```
Query name: `animelib` = `SELECT * FROM animelibView`.

Fetch type: `1` = episodes (the entry's own episodes are counted), `0` = seasons (children counted via `parent_id`). Default `fetch_type` column value is `1` (`animes.sq`).

Dependent views:
- `episodestatsView`: per `episodes.anime_id`: `total = count(*)`, `seenCount = sum(seen)`, `latestUpload = coalesce(max(date_upload),0)`, `fetchedAt = coalesce(max(date_fetch),0)`, `bookmarkCount = sum(bookmark)`, `fillermarkCount = sum(fillermark)`.
- `animehistorystatsView`: per `episodes.anime_id`: `lastSeen = max(ah.last_seen)` via `LEFT JOIN animehistory ah ON episodes._id = ah.episode_id`.
- `animeseasonstatsView`: per `season.parent_id` (seasons with `parent_id IS NOT NULL`): `child_count = count(*)`, `fully_seen_seasons = sum(CASE WHEN season.fetch_type = 1 AND total = seenCount THEN 1 ELSE 0 END)`, `max_latest_upload`, `max_fetched_at`, `max_last_seen`, `total_bookmarks`, `total_fillermarks`.

Note: for a season-parent, `seenCount` counts fully-seen **seasons**, and `totalCount` counts **seasons**, so unseen = unseen seasons (E-48).

### D-7.2 `libraryView` (manga library rows) — `data/src/main/sqldelight/view/libraryView.sq`
```sql
CREATE VIEW libraryView AS
SELECT M.*,
  coalesce(C.total,0) AS totalCount,
  coalesce(C.readCount,0) AS readCount,
  coalesce(C.latestUpload,0) AS latestUpload,
  coalesce(C.fetchedAt,0) AS chapterFetchedAt,
  coalesce(C.lastRead,0) AS lastRead,
  coalesce(C.bookmarkCount,0) AS bookmarkCount,
  coalesce(MC.category_id,0) AS category
FROM mangas M
LEFT JOIN (
  SELECT chapters.manga_id,
         count(*) AS total,
         sum(read) AS readCount,
         coalesce(max(chapters.date_upload),0) AS latestUpload,
         coalesce(max(history.last_read),0) AS lastRead,
         coalesce(max(chapters.date_fetch),0) AS fetchedAt,
         sum(chapters.bookmark) AS bookmarkCount
  FROM chapters
  LEFT JOIN excluded_scanlators ON chapters.manga_id = excluded_scanlators.manga_id AND chapters.scanlator = excluded_scanlators.scanlator
  LEFT JOIN history ON chapters._id = history.chapter_id
  WHERE excluded_scanlators.scanlator IS NULL
  GROUP BY chapters.manga_id
) AS C ON M._id = C.manga_id
LEFT JOIN mangas_categories AS MC ON MC.manga_id = M._id
WHERE M.favorite = 1;
```
Query name: `library` = `SELECT * FROM libraryView`.
Manga scanlator exclusion applies to counts and dates. `excluded_scanlators(manga_id, scanlator)` with `ON DELETE CASCADE` (`data/src/main/sqldelight/data/excluded_scanlators.sq:1-6`).

Note: `chapters.scanlator` may be NULL; `chapters.scanlator = excluded.scanlator` never matches NULL, so NULL-scanlator chapters are never excluded.

### D-7.3 Tables
- `animes`: `_id, source, url, artist, author, description, genre (List<String>), title, status (INTEGER; 2 = completed), thumbnail_url, favorite (bool), last_update, next_update, initialized, viewer, episode_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, favorite_modified_at, version, is_syncing, fetch_type (default 1), parent_id, season_flags, season_number, season_source_order, background_url, background_last_modified, memo` (`data/.../dataanime/animes.sq:8-35`).
- `episodes`: `_id, anime_id, url, name, scanlator, seen (bool), bookmark (bool), last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing, summary, preview_url, fillermark (bool), memo` (`dataanime/episodes.sq`).
- `animehistory`: `_id, episode_id UNIQUE, last_seen (Date)` (`dataanime/animehistory.sq:3-8`).
- `mangas`: `_id, source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, favorite_modified_at, version, is_syncing` (`data/src/main/sqldelight/data/mangas.sq`).
- `chapters`: `_id, manga_id, url, name, scanlator, read (bool), bookmark (bool), last_page_read, chapter_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing` (`data/src/main/sqldelight/data/chapters.sq`).
- `history`: `_id, chapter_id UNIQUE, last_read (Date), time_read` (`data/src/main/sqldelight/data/history.sq:3-8`).
- `excluded_scanlators`: `manga_id, scanlator` (`data/src/main/sqldelight/data/excluded_scanlators.sq`).
- `categories`, `animes_categories`, `mangas_categories` (D-5).

Write paths touched by this area (all delegate to other areas):
- `animes.favorite = 0` via `UpdateAnime`/`AnimeUpdate(favorite = false)` (remove from library).
- `animes_categories` replace-all via `SetAnimeCategories`.
- `categories` insert/update/delete/reorder via category interactors.
- `episodes.seen` / `chapters.read` via `SetSeenStatus`/`SetReadStatus`.

### D-7.4 `ORDER BY` and sorting in SQL
The library query has **no** ORDER BY; all sorting is in Kotlin (`applySort`). Rust must sort in memory too, or add an index per sort. Category order is `ORDER BY sort` (categories).

---

## D-8. Files, folders, caches (touched, not owned)

| Path / cache | Touched by | Owner area |
|---|---|---|
| Cover cache (`AnimeCoverCache`, `MangaCoverCache`) — `deleteFromCache(entry, true)` | remove-from-library (`removeCovers`) | area:34-images-covers-cache |
| Background cache (`AnimeBackgroundCache`) | remove-from-library (`removeBackgrounds`) | area:34 |
| Downloaded episodes/chapters on disk | "Delete downloaded" in delete dialog via `AnimeDownloadManager.deleteAnime(anime, source)` (which removes the anime directory, queue entries, cache entries, and the source directory if empty) | area:24-downloads-anime, area:25 |
| `LocalAnimeSource` folder | never deleted by this area (E-28) | area:14 |

`AnimeDownloadManager.deleteAnime(anime, source, removeQueued = true)` (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt:299-312`): `downloader.removeFromQueue(anime)`, delete the anime directory found via `provider.findAnimeDir(title, source)`, `cache.removeAnime`, delete the source directory if now empty and `cache.removeSource`.

---

## D-9. Network endpoints

None in scope. The library UI never calls a source or tracker API directly. Indirect calls:
- Tracker score conversion `get10PointScore(track)` is local arithmetic on stored track scores (see `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt:43`, `Anilist.kt:136-143`).
- Library update (`AnimeLibraryUpdateJob.startNow`) is area:26.
- Global search and entry screens are other areas.

---

## D-10. Constants referenced

| Constant | Value | File |
|---|---|---|
| `SEARCH_DEBOUNCE_MILLIS` | `250L` | `app/src/main/java/eu/kanade/presentation/components/AppBar.kt:65` |
| `Category.UNCATEGORIZED_ID` | `0L` | `domain/.../category/model/Category.kt:16` |
| `SAnime.ONGOING` | 1 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:73` |
| `SAnime.COMPLETED` | 2 | `SAnime.kt:74` |
| `SManga.ONGOING` | 1 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:49` |
| `SManga.COMPLETED` | 2 | `SManga.kt:50` |
| `Anime.SHOW_ALL`, `Manga.SHOW_ALL` | used by entry-level filters (area:07) | `LibraryPreferences.kt:174` |
| `LibraryPreferences.ENTRY_OUTSIDE_RELEASE_PERIOD` | `"manga_outside_release_period"` | `LibraryPreferences.kt:406` |
| `LibraryPreferences.ENTRY_NON_COMPLETED` | `"manga_ongoing"` | `:403` |
| `LibraryPreferences.ENTRY_HAS_UNVIEWED` | `"manga_fully_read"` | `:404` |
| `LibraryPreferences.ENTRY_NON_VIEWED` | `"manga_started"` | `:405` |
| `GETTING_STARTED_URL` | external guide URL (value in `presentation/more/onboarding`; area:36) | `AnimeLibraryTab.kt:38` |
| `Anime.fetchInterval` | Int; `< 0` = outside release period | `domain/.../entries/anime/model/Anime.kt:22` |
| Grid min cell width | 128 dp | `LazyLibraryGrid.kt:22` |
| Badge padding | 3 dp h / 1 dp v | `presentation-core/.../components/Badges.kt:38-57` |

---

## D-11. Category delete cascade (keys touched)

`DeleteAnimeCategory` (`domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt:38-50`) removes the deleted id (as a string) from these StringSet preferences when present:

| Key | Owner |
|---|---|
| `animelib_update_categories` | `LibraryPreferences.animeUpdateCategories()` |
| `animelib_update_categories_exclude` | `LibraryPreferences.animeUpdateCategoriesExclude()` |
| `remove_exclude_anime_categories` | `DownloadPreferences.removeExcludeAnimeCategories()` (area:24/25) |
| `download_new_anime_categories` | `DownloadPreferences.downloadNewEpisodeCategories()` (area:24/25) |
| `download_new_anime_categories_exclude` | `DownloadPreferences.downloadNewEpisodeCategoriesExclude()` |

If `default_anime_category` equals the deleted id, that preference is deleted (reset to -1 "none").

Manga: `library_update_categories`, `library_update_categories_exclude`, `remove_exclude_categories`, `download_new_categories`, `download_new_categories_exclude`, and `default_category`.

Literal key strings verified in `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:76-81`: anime `remove_exclude_anime_categories`, `download_new_anime_categories`, `download_new_anime_categories_exclude`; manga `remove_exclude_categories`, `download_new_categories`, `download_new_categories_exclude`.

---

## D-12. Codec and invariant checks (for tests)

1. Round-trip: `deserialize(serialize(x)) == x` for every anime and manga sort type × both directions (except Random: round-trips with seed unchanged).
2. Flag round-trip: `valueOf(flagOf(type, dir)) == (type, dir)` for all anime types and both directions; manga the same, excluding AiringTime.
3. `merge(old, flag, mask)` keeps bits outside the mask (bits 0-1, 7+).
4. Display mode: unknown string → CompactGrid.
5. Category `flags = 0` decodes to (Alphabetical, Descending) (see E-20).
6. `hasStarted == (seenCount > 0)`; `unseenCount == totalCount - seenCount`.
