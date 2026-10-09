# Area 47 (part 1) — data models, preference keys, files, endpoints, tables

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Storage backend for preferences on Android is `AndroidPreferenceStore` (SharedPreferences) (`core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt`). The Rust target uses SQLite (see `p1-rust-design.md` section 2).

---

## 1. Data models

### 1.1 Preference abstraction (`core/common/src/main/java/tachiyomi/core/common/preference/`)

| Kotlin type | Members (name: type) | Semantics |
|---|---|---|
| `Preference<T>` (interface) | `key(): String`, `get(): T`, `set(value: T)`, `isSet(): Boolean`, `delete()`, `defaultValue(): T`, `changes(): Flow<T>`, `stateIn(scope): StateFlow<T>` | Typed view of one key. `get()` returns `defaultValue()` when unset. `delete()` removes the key so later reads return the default. |
| `PreferenceStore` (interface) | `getString(key, default): Preference<String>`, `getLong`, `getInt`, `getFloat`, `getBoolean`, `getStringSet(key, default: Set<String>)`, `getObject<T>(key, default, serializer: (T)->String, deserializer: (String)->T)`, `getAll(): Map<String, *>` | Storage factory. |
| `getEnum<T: Enum<T>>(key, default)` (extension, `PreferenceStore.kt:27-…`) | serializer `it.name`; deserializer `enumValueOf(it)` with `IllegalArgumentException` → default | Enum values are stored by constant name. |
| `Preference.appStateKey(key)` | returns `"__APP_STATE_" + key` | App state; excluded from backup. |
| `Preference.privateKey(key)` | returns `"__PRIVATE_" + key` | Private; excluded from backup without consent. |
| `Preference.isPrivate(key)`, `isAppState(key)` | prefix tests | Used by the backup code (area 21). |
| `TriState` enum | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:3-7`) | Stored by name. |

### 1.2 Settings tree model (`app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt`)

`sealed class Preference { abstract val title: String; abstract val enabled: Boolean }`

| Variant | Fields (type, default) |
|---|---|
| `PreferenceGroup` | `title: String`, `enabled: Boolean = true`, `preferenceItems: ImmutableList<PreferenceItem<out Any>>` |
| `PreferenceItem<T>` (sealed, abstract `subtitle: String?`, `icon: ImageVector?`, `onValueChanged: suspend (T) -> Boolean`) | see below |
| `TextPreference` | `title`, `subtitle: String? = null`, `icon = null`, `enabled = true`, `onClick: (() -> Unit)? = null` |
| `SwitchPreference` | `preference: Preference<Boolean>`, `title`, `subtitle = null`, `enabled = true`, `onValueChanged = { true }` |
| `SliderPreference` | `value: Int`, `title`, `valueRange: IntProgression = 0..1`, `steps: Int = (last - first) - 1`, `subtitle = null`, `enabled = true`, `onValueChanged` |
| `ListPreference<T>` | `preference: Preference<T>`, `entries: ImmutableMap<T, String>`, `title`, `subtitle = "%s"`, `subtitleProvider`, `icon = null`, `enabled = true`, `onValueChanged` |
| `BasicListPreference` | same as ListPreference but `value: String`, no `Preference` (unused in scope) |
| `MultiSelectListPreference` | `preference: Preference<Set<String>>`, `entries: ImmutableMap<String, String>`, `title`, `subtitle = "%s"`, `subtitleProvider` (default: labels joined `, `, or `None`), `icon`, `enabled`, `onValueChanged` |
| `EditTextPreference` | `preference: Preference<String>`, `title`, `subtitle = "%s"`, `enabled`, `onValueChanged` |
| `MultiLineEditTextPreference` | `preference`, `canBeBlank = false`, `title`, … (out of scope) |
| `MPVConfPreference` | out of scope (player) |
| `EditTextInfoPreference` | `preference`, `dialogSubtitle: String?`, `validate`, `errorMessage`, `keyboardOptions`, … (out of scope here) |
| `TrackerPreference` | `tracker: Tracker`, `login: () -> Unit`, `logout: () -> Unit`; `title = ""`, `enabled = true`, `subtitle = null` |
| `InfoPreference` | `title`, `enabled = true` |
| `CustomPreference` | `title`, `content: @Composable () -> Unit` |

Default `subtitle = "%s"` means the subtitle is `"%s".format(entries[value])`.

### 1.3 Enums and their stored values

| Enum (file) | Constants (stored name = constant name unless noted) | Numeric value used by prefs | Labels (strings key) |
|---|---|---|---|
| `ThemeMode` (`app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt:118`) | `LIGHT`, `DARK`, `SYSTEM` | — | (switch row, see behaviour §5) |
| `AppTheme` (`domain/ui/model/AppTheme.kt`) | `DEFAULT`, `MONET`, `CLOUDFLARE`, `COTTONCANDY`, `DOOM`, `GREEN_APPLE`, `LAVENDER`, `MATRIX`, `MIDNIGHT_DUSK`, `MOCHA`, `SAPPHIRE`, `NORD`, `STRAWBERRY_DAIQUIRI`, `TAKO`, `TEALTURQUOISE`, `TIDAL_WAVE`, `YINYANG`, `YOTSUBA`, `MONOCHROME`, deprecated `DARK_BLUE`, `HOT_PINK`, `BLUE` | — | `label_default`, `theme_monet`, others |
| `TabletUiMode` (`domain/ui/model/TabletUiMode.kt`) | `AUTOMATIC`, `ALWAYS`, `LANDSCAPE`, `NEVER` | — | `automatic_background`, `lock_always`, `landscape`, `lock_never` |
| `StartScreen` (`domain/ui/model/StartScreen.kt`) | `ANIME`, `MANGA`, `UPDATES`, `HISTORY`, `BROWSE` | — | `label_anime`, `manga`, `label_recent_updates`, `label_recent_manga`, `browse` |
| `NavStyle` (`domain/ui/model/NavStyle.kt`) | `MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE` | — | `pref_bottom_nav_no_*` (AYMR) |
| `ReadingMode` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`) | `DEFAULT`, `LEFT_TO_RIGHT`, `RIGHT_TO_LEFT`, `VERTICAL`, `WEBTOON`, `CONTINUOUS_VERTICAL` | `0x0`, `0x1`, `0x2`, `0x3`, `0x4`, `0x5`; `MASK = 0x7` | `label_default`, `left_to_right_viewer`, `right_to_left_viewer`, `vertical_viewer`, `webtoon_viewer`, `vertical_plus_viewer` |
| `ReaderOrientation` (`…/ReaderOrientation.kt`) | `DEFAULT`, `FREE`, `PORTRAIT`, `LANDSCAPE`, `LOCKED_PORTRAIT`, `LOCKED_LANDSCAPE`, `REVERSE_PORTRAIT` | `0x00`, `0x08`, `0x10`, `0x18`, `0x20`, `0x28`, `0x30`; `MASK = 0x38` | `label_default`, `rotation_free`, `rotation_portrait`, `rotation_landscape`, `rotation_force_portrait`, `rotation_force_landscape`, `rotation_reverse_portrait` |
| `ReaderOrientation.flag` (Android `ActivityInfo`) | `UNSPECIFIED`(-1) for DEFAULT and FREE; `SENSOR_PORTRAIT`(7) PORTRAIT; `SENSOR_LANDSCAPE`(6) LANDSCAPE; `PORTRAIT`(1) LOCKED_PORTRAIT; `LANDSCAPE`(0) LOCKED_LANDSCAPE; `REVERSE_PORTRAIT`(9) REVERSE_PORTRAIT | — | Android-only |
| `TappingInvertMode` (`ReaderPreferences.kt:183-196`) | `NONE`, `HORIZONTAL` (inv H), `VERTICAL` (inv V), `BOTH` (inv H+V) | — | `tapping_inverted_none`, `…_horizontal`, `…_vertical`, `…_both` |
| `ReaderHideThreshold` (`ReaderPreferences.kt:198-203`) | `HIGHEST`(5), `HIGH`(13), `LOW`(31), `LOWEST`(47) | threshold field | `pref_highest`, `pref_high`, `pref_low`, `pref_lowest` |
| `FlashColor` (`ReaderPreferences.kt:177-181`) | `BLACK`, `WHITE`, `WHITE_BLACK` | — | `pref_flash_style_black`, `pref_flash_style_white`, `pref_flash_style_white_black` |
| `ReaderPreferences.TapZones` (list index) | index 0 `label_default`, 1 `l_nav`, 2 `kindlish_nav`, 3 `edge_nav`, 4 `right_and_left_nav`, 5 `disabled_nav` | stored as int | — |
| `ImageScaleType` list index | stored as `index + 1`: 1 `scale_type_fit_screen`, 2 `scale_type_stretch`, 3 `scale_type_fit_width`, 4 `scale_type_fit_height`, 5 `scale_type_original_size`, 6 `scale_type_smart_fit` | 1..6 | — |
| `ZoomStart` list index | stored as `index + 1`: 1 `zoom_start_automatic`, 2 `zoom_start_left`, 3 `zoom_start_right`, 4 `zoom_start_center` | 1..4 | — |
| `AutoTrackState` (`app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt`) | `ALWAYS`, `ASK`, `NEVER` | — | `lock_always`, `default_category_summary` ("Always ask"), `lock_never` |
| `SecurityPreferences.SecureScreenMode` (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:79-83`) | `ALWAYS`, `INCOGNITO`, `NEVER` | — | `lock_always`, `pref_incognito_mode`, `lock_never` |
| `BasePreferences.ExtensionInstaller` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:140-145`) | `LEGACY`(requires system perm), `PACKAGEINSTALLER`(requires system perm), `SHIZUKU`(no), `PRIVATE`(no) | — | `ext_installer_legacy`, `…_packageinstaller`, `…_shizuku`, `…_private` |
| `SourcePreferences.DataSaver` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:118-123`) | `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` | — | `disabled`, `bandwidth_hero`, `wsrv`, `resmush` |
| `LibraryPreferences.EpisodeSwipeAction` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:383-389`) | `ToggleSeen`, `ToggleBookmark`, `ToggleFillermark`, `Download`, `Disabled` (**mixed-case names, stored as-is**) | — | see behaviour §3.4 |
| `LibraryPreferences.ChapterSwipeAction` (`…:391-396`) | `ToggleRead`, `ToggleBookmark`, `Download`, `Disabled` (mixed case) | — | see behaviour §3.5 |
| `LibraryDisplayMode` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`) | `CompactGrid`, `ComfortableGrid`, `List`, `CoverOnlyGrid`; default `CompactGrid` | serialized strings `"COMPACT_GRID"`, `"COMFORTABLE_GRID"`, `"LIST"`, `"COVER_ONLY_GRID"`; any other → default | (not in scope) |
| `SeasonDisplayMode` (`domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt`) | `CompactGrid`, `ComfortableGrid`, `CoverOnlyGrid`, `List` | `toLong`: 0, 1, 2, 3; `fromLong` throws `IllegalArgumentException("Invalid sorting value")` on others | — |
| `Anilist.POINT_10` (`app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt:50`) | string `"POINT_10"` | default of `anilist_score_type` | area 28 |

### 1.4 Sort/filter integer encodings (defaults written by this screen)

Values come from `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt` and `…/manga/model/Manga.kt`.

| Constant | Value | Used as default by |
|---|---|---|
| `Anime.SHOW_ALL` / `Manga.SHOW_ALL` | `0x00000000` | all `default_*_filter_by_*` keys |
| `Anime.EPISODE_SORT_DESC` | `0x00000000` | `default_chapter_sort_by_ascending_or_descending` (episode side) |
| `Anime.EPISODE_SORT_ASC` | `0x00000001` | — |
| `Anime.EPISODE_SORTING_SOURCE` | `0x00000000` | `default_episode_sort_by_source_or_number` |
| `Anime.EPISODE_DISPLAY_NAME` | `0x00000000` | `default_chapter_display_by_name_or_number` (episode side, shared key — see edge cases) |
| `Anime.EPISODE_SHOW_PREVIEWS` / `EPISODE_SHOW_SUMMARIES` | `0x00000000` | `default_episode_show_thumbnail_previews` / `default_episode_show_summaries` |
| `Anime.SEASON_SORT_DESC` | `0x00000000` | `default_season_sort_by_ascending_or_descending` |
| `Anime.SEASON_SORT_ASC` | `0x00000001` | — |
| `Anime.SEASON_SORT_SOURCE` | `0x00000000` | `default_season_sort_by_source_or_number` |
| `Anime.SEASON_DISPLAY_MODE_SOURCE` | `0x00000000` | `default_season_display_mode` |
| `Anime.SEASON_DISPLAY_MODE_NUMBER` / `SEASON_DISPLAY_MODE_MASK` | `0x08000000` | (display-mode bit, area 07) |
| `Manga.CHAPTER_SORT_DESC` | `0x00000000` | `default_chapter_sort_by_ascending_or_descending` (chapter side) |
| `Manga.CHAPTER_SORT_ASC` | `0x00000001` | — |
| `Manga.CHAPTER_SORTING_SOURCE` | `0x00000000` | `default_chapter_sort_by_source_or_number` |
| `Manga.CHAPTER_DISPLAY_NAME` | `0x00000000` | `default_chapter_display_by_name_or_number` (chapter side) |

Bit-flag layouts summary
- `ReadingMode`: bits 0-2 (`MASK 0x7`) hold the mode; values 0-5 only.
- `ReaderOrientation`: bits 3-5 (`MASK 0x38`) hold the orientation; value 0 is "default / unset" and is never offered in the settings list.
- Season display mode (`default_season_display_mode`): bit 27 (`0x08000000`) is the number-vs-source flag; the grid mode is a separate key (`default_season_grid_display_mode`, 0-3).

### 1.5 Domain and transfer types used by the screens

| Type (file) | Fields used here |
|---|---|
| `Category` (`tachiyomi.domain.category.model.Category`) | `id: Long`, `order: Int` (sort key), `visualName: String` (`app/src/main/java/eu/kanade/presentation/category/CategoryExtensions.kt:10`), `isSystemCategory: Boolean` (computed; area 08) |
| `Source` (manga, `domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt`) and `AnimeSource` (`domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt`) | `id: Long`, `name: String`, `visualName: String`, `isStub: Boolean` (copied from the stub check) |
| `MangaSourceWithCount` | `source: Source`, `count: Long`; `id` = `source.id`, `name` = `source.name` |
| `DeletableAnime` (`domain/src/main/java/tachiyomi/domain/source/anime/model/DeletableAnime.kt`) and view `animedeletableView` | read by the screen as `sourceId`, `animeId`, `fetchType` (`FetchType.Seasons` check); the view columns are `_id`, `source`, `fetch_type` (`animedeletableView.sq`) |
| `AnimeSourceWithIds` (`domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSourceWithIds.kt`) | constructor `(source: AnimeSource, ids: List<Long>, orphaned: List<Long>)`; `ids` = deletable ids including non-library children; `orphaned` = library children to detach; the screen reads `id` (source id) and `name` |
| `ExportEntry` (`app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt:15-20`) | `type: ExportEntryType {MANGA, ANIME}`, `title: String`, `author: String?`, `artist: String?` |
| `LibraryExporter.ExportOptions` (`…:40-45`) | `includeTitle`, `includeType`, `includeAuthor`, `includeArtist` (all Boolean; defaults all true in the dialog) |
| `Tracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt`, area 27) | `id: Long` (used in key suffixes), `name: String`, `isLoggedIn`, `isLoggedInFlow`, `getUsername()`, `getPassword()`, `login(u, p)`, `logout()`, `loginNoop()` (enhanced only) |
| `EnhancedMangaTracker` / `EnhancedAnimeTracker` | `getAcceptedSources(): List<String>` — class qualified names of accepted sources |
| `GetApplicationRelease.Result` (`domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt`) | `NewUpdate(release: {version, info, releaseLink, downloadLink})`, `NoNewUpdate`, `OsTooOld` |
| `ChapterCache` (`app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt`) | `readableSize: String`, `clear(): Int` (count of removed files) |
| `UniFile` (`com.hippo.unifile`) | `fromUri(context, uri)`, `displayablePath`, `uri` (SAF) |

---

## 2. Preference keys used by the scope (all keys in the preference classes read for this area)

Columns: the Kotlin file and accessor function, the store call, the **stored name** (after `__APP_STATE_`/`__PRIVATE_` prefixing; `${id}` and `${tracker.id}` are formatted at runtime), and the default expression as written in the source. Generated by `plans/47-settings-screens-general/.prefs_build.py` from the source files. The table covers `BasePreferences`, `SourcePreferences`, `TrackPreferences`, `UiPreferences`, `ReaderPreferences`, `SecurityPreferences`, `NetworkPreferences`, `BackupPreferences`, `DownloadPreferences`, `LibraryPreferences`, `StoragePreferences`.

Companion constants used in defaults (resolved): `ReaderPreferences.MILLI_CONVERSION = 100`, `ReaderPreferences.WEBTOON_PADDING_MIN = 0`, `WEBTOON_PADDING_MAX = 25`; `GLUtil.SAFE_TEXTURE_LIMIT = 2048`; `Anilist.POINT_10 = "POINT_10"`; `LibraryPreferences.DEVICE_ONLY_ON_WIFI = "wifi"`, `DEVICE_NETWORK_NOT_METERED = "network_not_metered"`, `DEVICE_CHARGING = "ac"`, `ENTRY_NON_COMPLETED = "manga_ongoing"`, `ENTRY_HAS_UNVIEWED = "manga_fully_read"`, `ENTRY_NON_VIEWED = "manga_started"`, `ENTRY_OUTSIDE_RELEASE_PERIOD = "manga_outside_release_period"`, `MARK_DUPLICATE_CHAPTER_READ_NEW = "new"`, `…_EXISTING = "existing"`, `MARK_DUPLICATE_EPISODE_SEEN_NEW = "new_episode"`, `…_EXISTING = "existing_episode"`; `DEFAULT_MANGA_CATEGORY_PREF_KEY = "default_category"`, `DEFAULT_ANIME_CATEGORY_PREF_KEY = "default_anime_category"`, `LIBRARY_UPDATE_MANGA_CATEGORIES_PREF_KEY = "library_update_categories"`, `LIBRARY_UPDATE_ANIME_CATEGORIES_PREF_KEY = "animelib_update_categories"`, `…_EXCLUDE` variants `library_update_categories_exclude` and `animelib_update_categories_exclude`; `DownloadPreferences`: `remove_exclude_categories`, `remove_exclude_anime_categories`, `download_new_categories`, `download_new_anime_categories`, and their `_exclude` variants `download_new_categories_exclude`, `download_new_anime_categories_exclude`.

Defaults that are sets (as written): `library_update_restriction` = `{wifi}`; `library_update_manga_restriction` = `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` (order as in source: HAS_UNVIEWED, NON_COMPLETED, NON_VIEWED, OUTSIDE_RELEASE_PERIOD); `backup_flags` = `{categories, chapters, history, track}` (`FLAG_*` constants in area 21).

Category id sets are stored as strings of Long ids (`category.id.toString()`), not as indices.

Dynamic keys (name includes runtime value)
- `pref_filter_animelib_tracked_${id}_v2` and `pref_filter_library_tracked_${id}_v2` (Int id) — library filter (area 37).
- `pref_mangasync_username_${tracker.id}`, `pref_mangasync_password_${tracker.id}`, `pref_tracker_auth_expired_${tracker.id}`, `track_token_${tracker.id}` — tracker credentials (see the `__PRIVATE_` rows).

Note on the generated table: `consts`-resolved values appear in the Key column (e.g. `default_category`). Rows whose Default expression spans several lines are shown collapsed onto one line.

| File | Function | Store call | Key (stored name) | Default expression |
|---|---|---|---|---|
| `BasePreferences.kt` | `downloadedOnly()` | `getBoolean` | `__APP_STATE_pref_downloaded_only` | `false` |
| `BasePreferences.kt` | `incognitoMode()` | `getBoolean` | `__APP_STATE_incognito_mode` | `false` |
| `BasePreferences.kt` | `shownOnboardingFlow()` | `getBoolean` | `__APP_STATE_onboarding_complete` | `false` |
| `BasePreferences.kt` | `displayProfile()` | `getString` | `pref_display_profile_key` | `""` |
| `BasePreferences.kt` | `hardwareBitmapThreshold()` | `getInt` | `pref_hardware_bitmap_threshold` | `GLUtil.SAFE_TEXTURE_LIMIT` |
| `BasePreferences.kt` | `alwaysDecodeLongStripWithSSIV()` | `getBoolean` | `pref_always_decode_long_strip_with_ssiv` | `false` |
| `SourcePreferences.kt` | `sourceDisplayMode()` | `getObject` | `pref_display_mode_catalogue` | `LibraryDisplayMode.default, LibraryDisplayMode.Serializer::serialize, LibraryDisplayMode.Serializer::deserialize` |
| `SourcePreferences.kt` | `enabledLanguages()` | `getStringSet` | `source_languages` | `LocaleHelper.getDefaultEnabledLanguages()` |
| `SourcePreferences.kt` | `showNsfwSource()` | `getBoolean` | `show_nsfw_source` | `true` |
| `SourcePreferences.kt` | `migrationSortingMode()` | `getEnum` | `pref_migration_sorting` | `SetMigrateSorting.Mode.ALPHABETICAL` |
| `SourcePreferences.kt` | `migrationSortingDirection()` | `getEnum` | `pref_migration_direction` | `SetMigrateSorting.Direction.ASCENDING` |
| `SourcePreferences.kt` | `animeExtensionRepos()` | `getStringSet` | `anime_extension_repos` | `emptySet()` |
| `SourcePreferences.kt` | `mangaExtensionRepos()` | `getStringSet` | `extension_repos` | `emptySet()` |
| `SourcePreferences.kt` | `trustedExtensions()` | `getStringSet` | `__APP_STATE_trusted_extensions` | `emptySet()` |
| `SourcePreferences.kt` | `globalSearchFilterState()` | `getBoolean` | `__APP_STATE_has_filters_toggle_state` | `false` |
| `SourcePreferences.kt` | `disabledAnimeSources()` | `getStringSet` | `hidden_anime_catalogues` | `emptySet()` |
| `SourcePreferences.kt` | `disabledMangaSources()` | `getStringSet` | `hidden_catalogues` | `emptySet()` |
| `SourcePreferences.kt` | `incognitoAnimeExtensions()` | `getStringSet` | `incognito_anime_extensions` | `emptySet()` |
| `SourcePreferences.kt` | `incognitoMangaExtensions()` | `getStringSet` | `incognito_manga_extensions` | `emptySet()` |
| `SourcePreferences.kt` | `pinnedAnimeSources()` | `getStringSet` | `pinned_anime_catalogues` | `emptySet()` |
| `SourcePreferences.kt` | `pinnedMangaSources()` | `getStringSet` | `pinned_catalogues` | `emptySet()` |
| `SourcePreferences.kt` | `lastUsedAnimeSource()` | `getLong` | `__APP_STATE_last_anime_catalogue_source` | `-1` |
| `SourcePreferences.kt` | `lastUsedMangaSource()` | `getLong` | `__APP_STATE_last_catalogue_source` | `-1` |
| `SourcePreferences.kt` | `animeExtensionUpdatesCount()` | `getInt` | `animeext_updates_count` | `0` |
| `SourcePreferences.kt` | `mangaExtensionUpdatesCount()` | `getInt` | `ext_updates_count` | `0` |
| `SourcePreferences.kt` | `hideInAnimeLibraryItems()` | `getBoolean` | `browse_hide_in_anime_library_items` | `false` |
| `SourcePreferences.kt` | `hideInMangaLibraryItems()` | `getBoolean` | `browse_hide_in_library_items` | `false` |
| `SourcePreferences.kt` | `enableSourceBlacklist()` | `getBoolean` | `eh_enable_source_blacklist` | `true` |
| `SourcePreferences.kt` | `sourcesTabCategories()` | `getStringSet` | `sources_tab_categories` | `mutableSetOf()` |
| `SourcePreferences.kt` | `sourcesTabCategoriesFilter()` | `getBoolean` | `sources_tab_categories_filter` | `false` |
| `SourcePreferences.kt` | `sourcesTabSourcesInCategories()` | `getStringSet` | `sources_tab_source_categories` | `mutableSetOf()` |
| `SourcePreferences.kt` | `dataSaver()` | `getEnum` | `data_saver` | `DataSaver.NONE` |
| `SourcePreferences.kt` | `dataSaverIgnoreJpeg()` | `getBoolean` | `ignore_jpeg` | `false` |
| `SourcePreferences.kt` | `dataSaverIgnoreGif()` | `getBoolean` | `ignore_gif` | `true` |
| `SourcePreferences.kt` | `dataSaverImageQuality()` | `getInt` | `data_saver_image_quality` | `80` |
| `SourcePreferences.kt` | `dataSaverImageFormatJpeg()` | `getBoolean` | `data_saver_image_format_jpeg` | `false` |
| `SourcePreferences.kt` | `dataSaverServer()` | `getString` | `data_saver_server` | `""` |
| `SourcePreferences.kt` | `dataSaverColorBW()` | `getBoolean` | `data_saver_color_bw` | `false` |
| `SourcePreferences.kt` | `dataSaverExcludedSources()` | `getStringSet` | `data_saver_excluded` | `emptySet()` |
| `SourcePreferences.kt` | `dataSaverDownloader()` | `getBoolean` | `data_saver_downloader` | `true` |
| `TrackPreferences.kt` | `trackUsername()` | `getString` | `__PRIVATE_pref_mangasync_username_${tracker.id}` | `""` |
| `TrackPreferences.kt` | `trackPassword()` | `getString` | `__PRIVATE_pref_mangasync_password_${tracker.id}` | `""` |
| `TrackPreferences.kt` | `trackAuthExpired()` | `getBoolean` | `__PRIVATE_pref_tracker_auth_expired_${tracker.id}` | `false` |
| `TrackPreferences.kt` | `trackToken()` | `getString` | `__PRIVATE_track_token_${tracker.id}` | `""` |
| `TrackPreferences.kt` | `anilistScoreType()` | `getString` | `anilist_score_type` | `Anilist.POINT_10` |
| `TrackPreferences.kt` | `autoUpdateTrack()` | `getBoolean` | `pref_auto_update_manga_sync_key` | `true` |
| `TrackPreferences.kt` | `trackOnAddingToLibrary()` | `getBoolean` | `track_on_adding_to_library` | `true` |
| `TrackPreferences.kt` | `showNextEpisodeAiringTime()` | `getBoolean` | `show_next_episode_airing_time` | `true` |
| `TrackPreferences.kt` | `autoUpdateTrackOnMarkRead()` | `getEnum` | `pref_auto_update_manga_on_mark_read` | `AutoTrackState.ALWAYS` |
| `UiPreferences.kt` | `themeMode()` | `getEnum` | `pref_theme_mode_key` | `ThemeMode.SYSTEM` |
| `UiPreferences.kt` | `appTheme()` | `getEnum` | `pref_app_theme` | `if (DeviceUtil.isDynamicColorAvailable) { AppTheme.MONET } else { AppTheme.DEFAULT }` |
| `UiPreferences.kt` | `themeDarkAmoled()` | `getBoolean` | `pref_theme_dark_amoled_key` | `false` |
| `UiPreferences.kt` | `relativeTime()` | `getBoolean` | `relative_time_v2` | `true` |
| `UiPreferences.kt` | `dateFormat()` | `getString` | `app_date_format` | `""` |
| `UiPreferences.kt` | `tabletUiMode()` | `getEnum` | `tablet_ui_mode` | `TabletUiMode.AUTOMATIC` |
| `UiPreferences.kt` | `startScreen()` | `getEnum` | `start_screen` | `StartScreen.ANIME` |
| `UiPreferences.kt` | `navStyle()` | `getEnum` | `bottom_rail_nav_style` | `NavStyle.MOVE_HISTORY_TO_MORE` |
| `ReaderPreferences.kt` | `pageTransitions()` | `getBoolean` | `pref_enable_transitions_key` | `true` |
| `ReaderPreferences.kt` | `flashOnPageChange()` | `getBoolean` | `pref_reader_flash` | `false` |
| `ReaderPreferences.kt` | `flashDurationMillis()` | `getInt` | `pref_reader_flash_duration` | `MILLI_CONVERSION` |
| `ReaderPreferences.kt` | `flashPageInterval()` | `getInt` | `pref_reader_flash_interval` | `1` |
| `ReaderPreferences.kt` | `flashColor()` | `getEnum` | `pref_reader_flash_mode` | `FlashColor.BLACK` |
| `ReaderPreferences.kt` | `doubleTapAnimSpeed()` | `getInt` | `pref_double_tap_anim_speed` | `500` |
| `ReaderPreferences.kt` | `showPageNumber()` | `getBoolean` | `pref_show_page_number_key` | `true` |
| `ReaderPreferences.kt` | `showReadingMode()` | `getBoolean` | `pref_show_reading_mode` | `true` |
| `ReaderPreferences.kt` | `fullscreen()` | `getBoolean` | `fullscreen` | `true` |
| `ReaderPreferences.kt` | `cutoutShort()` | `getBoolean` | `cutout_short` | `true` |
| `ReaderPreferences.kt` | `keepScreenOn()` | `getBoolean` | `pref_keep_screen_on_key` | `true` |
| `ReaderPreferences.kt` | `defaultReadingMode()` | `getInt` | `pref_default_reading_mode_key` | `ReadingMode.RIGHT_TO_LEFT.flagValue` |
| `ReaderPreferences.kt` | `defaultOrientationType()` | `getInt` | `pref_default_orientation_type_key` | `ReaderOrientation.FREE.flagValue` |
| `ReaderPreferences.kt` | `webtoonDoubleTapZoomEnabled()` | `getBoolean` | `pref_enable_double_tap_zoom_webtoon` | `true` |
| `ReaderPreferences.kt` | `imageScaleType()` | `getInt` | `pref_image_scale_type_key` | `1` |
| `ReaderPreferences.kt` | `zoomStart()` | `getInt` | `pref_zoom_start_key` | `1` |
| `ReaderPreferences.kt` | `readerTheme()` | `getInt` | `pref_reader_theme_key` | `1` |
| `ReaderPreferences.kt` | `alwaysShowChapterTransition()` | `getBoolean` | `always_show_chapter_transition` | `true` |
| `ReaderPreferences.kt` | `preserveReadingPosition()` | `getBoolean` | `pref_preserve_reading_position` | `false` |
| `ReaderPreferences.kt` | `cropBorders()` | `getBoolean` | `crop_borders` | `false` |
| `ReaderPreferences.kt` | `navigateToPan()` | `getBoolean` | `navigate_pan` | `true` |
| `ReaderPreferences.kt` | `landscapeZoom()` | `getBoolean` | `landscape_zoom` | `true` |
| `ReaderPreferences.kt` | `cropBordersWebtoon()` | `getBoolean` | `crop_borders_webtoon` | `false` |
| `ReaderPreferences.kt` | `webtoonSidePadding()` | `getInt` | `webtoon_side_padding` | `WEBTOON_PADDING_MIN` |
| `ReaderPreferences.kt` | `readerHideThreshold()` | `getEnum` | `reader_hide_threshold` | `ReaderHideThreshold.LOW` |
| `ReaderPreferences.kt` | `folderPerManga()` | `getBoolean` | `create_folder_per_manga` | `false` |
| `ReaderPreferences.kt` | `skipRead()` | `getBoolean` | `skip_read` | `false` |
| `ReaderPreferences.kt` | `skipFiltered()` | `getBoolean` | `skip_filtered` | `true` |
| `ReaderPreferences.kt` | `skipDupe()` | `getBoolean` | `skip_dupe` | `false` |
| `ReaderPreferences.kt` | `webtoonDisableZoomOut()` | `getBoolean` | `webtoon_disable_zoom_out` | `false` |
| `ReaderPreferences.kt` | `dualPageSplitPaged()` | `getBoolean` | `pref_dual_page_split` | `false` |
| `ReaderPreferences.kt` | `dualPageInvertPaged()` | `getBoolean` | `pref_dual_page_invert` | `false` |
| `ReaderPreferences.kt` | `dualPageSplitWebtoon()` | `getBoolean` | `pref_dual_page_split_webtoon` | `false` |
| `ReaderPreferences.kt` | `dualPageInvertWebtoon()` | `getBoolean` | `pref_dual_page_invert_webtoon` | `false` |
| `ReaderPreferences.kt` | `dualPageRotateToFit()` | `getBoolean` | `pref_dual_page_rotate` | `false` |
| `ReaderPreferences.kt` | `dualPageRotateToFitInvert()` | `getBoolean` | `pref_dual_page_rotate_invert` | `false` |
| `ReaderPreferences.kt` | `dualPageRotateToFitWebtoon()` | `getBoolean` | `pref_dual_page_rotate_webtoon` | `false` |
| `ReaderPreferences.kt` | `dualPageRotateToFitInvertWebtoon()` | `getBoolean` | `pref_dual_page_rotate_invert_webtoon` | `false` |
| `ReaderPreferences.kt` | `customBrightness()` | `getBoolean` | `pref_custom_brightness_key` | `false` |
| `ReaderPreferences.kt` | `customBrightnessValue()` | `getInt` | `custom_brightness_value` | `0` |
| `ReaderPreferences.kt` | `colorFilter()` | `getBoolean` | `pref_color_filter_key` | `false` |
| `ReaderPreferences.kt` | `colorFilterValue()` | `getInt` | `color_filter_value` | `0` |
| `ReaderPreferences.kt` | `colorFilterMode()` | `getInt` | `color_filter_mode` | `0` |
| `ReaderPreferences.kt` | `grayscale()` | `getBoolean` | `pref_grayscale` | `false` |
| `ReaderPreferences.kt` | `invertedColors()` | `getBoolean` | `pref_inverted_colors` | `false` |
| `ReaderPreferences.kt` | `readWithLongTap()` | `getBoolean` | `reader_long_tap` | `true` |
| `ReaderPreferences.kt` | `readWithVolumeKeys()` | `getBoolean` | `reader_volume_keys` | `false` |
| `ReaderPreferences.kt` | `readWithVolumeKeysInverted()` | `getBoolean` | `reader_volume_keys_inverted` | `false` |
| `ReaderPreferences.kt` | `navigationModePager()` | `getInt` | `reader_navigation_mode_pager` | `0` |
| `ReaderPreferences.kt` | `navigationModeWebtoon()` | `getInt` | `reader_navigation_mode_webtoon` | `0` |
| `ReaderPreferences.kt` | `pagerNavInverted()` | `getEnum` | `reader_tapping_inverted` | `TappingInvertMode.NONE` |
| `ReaderPreferences.kt` | `webtoonNavInverted()` | `getEnum` | `reader_tapping_inverted_webtoon` | `TappingInvertMode.NONE` |
| `ReaderPreferences.kt` | `showNavigationOverlayNewUser()` | `getBoolean` | `reader_navigation_overlay_new_user` | `true` |
| `ReaderPreferences.kt` | `showNavigationOverlayOnStart()` | `getBoolean` | `reader_navigation_overlay_on_start` | `false` |
| `SecurityPreferences.kt` | `useAuthenticator()` | `getBoolean` | `use_biometric_lock` | `false` |
| `SecurityPreferences.kt` | `lockAppAfter()` | `getInt` | `lock_app_after` | `0` |
| `SecurityPreferences.kt` | `secureScreen()` | `getEnum` | `secure_screen_v2` | `SecureScreenMode.INCOGNITO` |
| `SecurityPreferences.kt` | `hideNotificationContent()` | `getBoolean` | `hide_notification_content` | `false` |
| `SecurityPreferences.kt` | `lastAppClosed()` | `getLong` | `__APP_STATE_last_app_closed` | `0` |
| `NetworkPreferences.kt` | `verboseLogging()` | `getBoolean` | `verbose_logging` | `verboseLogging` |
| `NetworkPreferences.kt` | `dohProvider()` | `getInt` | `doh_provider` | `-1` |
| `NetworkPreferences.kt` | `defaultUserAgent()` | `getString` | `default_user_agent` | `"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"` |
| `BackupPreferences.kt` | `backupInterval()` | `getInt` | `backup_interval` | `12` |
| `BackupPreferences.kt` | `lastAutoBackupTimestamp()` | `getLong` | `__APP_STATE_last_auto_backup_timestamp` | `0L` |
| `BackupPreferences.kt` | `backupFlags()` | `getStringSet` | `backup_flags` | `setOf(FLAG_CATEGORIES, FLAG_CHAPTERS, FLAG_HISTORY, FLAG_TRACK)` |
| `DownloadPreferences.kt` | `downloadOnlyOverWifi()` | `getBoolean` | `pref_download_only_over_wifi_key` | `true` |
| `DownloadPreferences.kt` | `useExternalDownloader()` | `getBoolean` | `use_external_downloader` | `false` |
| `DownloadPreferences.kt` | `externalDownloaderSelection()` | `getString` | `external_downloader_selection` | `""` |
| `DownloadPreferences.kt` | `saveChaptersAsCBZ()` | `getBoolean` | `save_chapter_as_cbz` | `true` |
| `DownloadPreferences.kt` | `splitTallImages()` | `getBoolean` | `split_tall_images` | `true` |
| `DownloadPreferences.kt` | `autoDownloadWhileReading()` | `getInt` | `auto_download_while_reading` | `0` |
| `DownloadPreferences.kt` | `autoDownloadWhileWatching()` | `getInt` | `auto_download_while_watching` | `0` |
| `DownloadPreferences.kt` | `removeAfterReadSlots()` | `getInt` | `remove_after_read_slots` | `-1` |
| `DownloadPreferences.kt` | `removeAfterMarkedAsRead()` | `getBoolean` | `pref_remove_after_marked_as_read_key` | `false` |
| `DownloadPreferences.kt` | `removeBookmarkedChapters()` | `getBoolean` | `pref_remove_bookmarked` | `false` |
| `DownloadPreferences.kt` | `downloadFillermarkedItems()` | `getBoolean` | `pref_download_fillermarked` | `false` |
| `DownloadPreferences.kt` | `removeExcludeCategories()` | `getStringSet` | `remove_exclude_categories` | `emptySet()` |
| `DownloadPreferences.kt` | `removeExcludeAnimeCategories()` | `getStringSet` | `remove_exclude_anime_categories` | `emptySet()` |
| `DownloadPreferences.kt` | `downloadNewChapters()` | `getBoolean` | `download_new` | `false` |
| `DownloadPreferences.kt` | `downloadNewEpisodes()` | `getBoolean` | `download_new_episode` | `false` |
| `DownloadPreferences.kt` | `downloadNewChapterCategories()` | `getStringSet` | `download_new_categories` | `emptySet()` |
| `DownloadPreferences.kt` | `downloadNewEpisodeCategories()` | `getStringSet` | `download_new_anime_categories` | `emptySet()` |
| `DownloadPreferences.kt` | `downloadNewChapterCategoriesExclude()` | `getStringSet` | `download_new_categories_exclude` | `emptySet()` |
| `DownloadPreferences.kt` | `downloadNewEpisodeCategoriesExclude()` | `getStringSet` | `download_new_anime_categories_exclude` | `emptySet()` |
| `DownloadPreferences.kt` | `numberOfDownloads()` | `getInt` | `download_slots` | `1` |
| `DownloadPreferences.kt` | `downloadSpeedLimit()` | `getInt` | `download_speed_limit` | `0` |
| `DownloadPreferences.kt` | `downloadNewUnreadChaptersOnly()` | `getBoolean` | `download_new_unread_chapters_only` | `false` |
| `DownloadPreferences.kt` | `downloadNewUnseenEpisodesOnly()` | `getBoolean` | `download_new_unread_episodes_only` | `false` |
| `LibraryPreferences.kt` | `displayMode()` | `getObject` | `pref_display_mode_library` | `LibraryDisplayMode.default, LibraryDisplayMode.Serializer::serialize, LibraryDisplayMode.Serializer::deserialize` |
| `LibraryPreferences.kt` | `mangaSortingMode()` | `getObject` | `library_sorting_mode` | `MangaLibrarySort.default, MangaLibrarySort.Serializer::serialize, MangaLibrarySort.Serializer::deserialize` |
| `LibraryPreferences.kt` | `animeSortingMode()` | `getObject` | `animelib_sorting_mode` | `AnimeLibrarySort.default, AnimeLibrarySort.Serializer::serialize, AnimeLibrarySort.Serializer::deserialize` |
| `LibraryPreferences.kt` | `lastUpdatedTimestamp()` | `getLong` | `__APP_STATE_library_update_last_timestamp` | `0L` |
| `LibraryPreferences.kt` | `autoUpdateInterval()` | `getInt` | `pref_library_update_interval_key` | `0` |
| `LibraryPreferences.kt` | `autoUpdateDeviceRestrictions()` | `getStringSet` | `library_update_restriction` | `setOf( DEVICE_ONLY_ON_WIFI, )` |
| `LibraryPreferences.kt` | `autoUpdateItemRestrictions()` | `getStringSet` | `library_update_manga_restriction` | `setOf( ENTRY_HAS_UNVIEWED, ENTRY_NON_COMPLETED, ENTRY_NON_VIEWED, ENTRY_OUTSIDE_RELEASE_PERIOD, )` |
| `LibraryPreferences.kt` | `autoUpdateMetadata()` | `getBoolean` | `auto_update_metadata` | `false` |
| `LibraryPreferences.kt` | `showContinueViewingButton()` | `getBoolean` | `display_continue_reading_button` | `false` |
| `LibraryPreferences.kt` | `categoryTabs()` | `getBoolean` | `display_category_tabs` | `true` |
| `LibraryPreferences.kt` | `categoryNumberOfItems()` | `getBoolean` | `display_number_of_items` | `false` |
| `LibraryPreferences.kt` | `categorizedDisplaySettings()` | `getBoolean` | `categorized_display` | `false` |
| `LibraryPreferences.kt` | `hideHiddenCategoriesSettings()` | `getBoolean` | `hidden_categories` | `false` |
| `LibraryPreferences.kt` | `filterIntervalCustom()` | `getEnum` | `pref_filter_library_interval_custom` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `downloadBadge()` | `getBoolean` | `display_download_badge` | `false` |
| `LibraryPreferences.kt` | `unreadBadge()` | `getBoolean` | `display_unread_badge` | `true` |
| `LibraryPreferences.kt` | `localBadge()` | `getBoolean` | `display_local_badge` | `true` |
| `LibraryPreferences.kt` | `languageBadge()` | `getBoolean` | `display_language_badge` | `false` |
| `LibraryPreferences.kt` | `newShowUpdatesCount()` | `getBoolean` | `library_show_updates_count` | `true` |
| `LibraryPreferences.kt` | `autoClearItemCache()` | `getBoolean` | `auto_clear_chapter_cache` | `false` |
| `LibraryPreferences.kt` | `randomAnimeSortSeed()` | `getInt` | `library_random_anime_sort_seed` | `0` |
| `LibraryPreferences.kt` | `randomMangaSortSeed()` | `getInt` | `library_random_manga_sort_seed` | `0` |
| `LibraryPreferences.kt` | `animePortraitColumns()` | `getInt` | `pref_animelib_columns_portrait_key` | `0` |
| `LibraryPreferences.kt` | `mangaPortraitColumns()` | `getInt` | `pref_library_columns_portrait_key` | `0` |
| `LibraryPreferences.kt` | `animeLandscapeColumns()` | `getInt` | `pref_animelib_columns_landscape_key` | `0` |
| `LibraryPreferences.kt` | `mangaLandscapeColumns()` | `getInt` | `pref_library_columns_landscape_key` | `0` |
| `LibraryPreferences.kt` | `filterDownloadedAnime()` | `getEnum` | `pref_filter_animelib_downloaded_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterDownloadedManga()` | `getEnum` | `pref_filter_library_downloaded_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterUnseen()` | `getEnum` | `pref_filter_animelib_unread_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterUnread()` | `getEnum` | `pref_filter_library_unread_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterStartedAnime()` | `getEnum` | `pref_filter_animelib_started_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterStartedManga()` | `getEnum` | `pref_filter_library_started_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterBookmarkedAnime()` | `getEnum` | `pref_filter_animelib_bookmarked_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterBookmarkedManga()` | `getEnum` | `pref_filter_library_bookmarked_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterCompletedAnime()` | `getEnum` | `pref_filter_animelib_completed_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterCompletedManga()` | `getEnum` | `pref_filter_library_completed_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterTrackedAnime()` | `getEnum` | `pref_filter_animelib_tracked_${id}_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `filterTrackedManga()` | `getEnum` | `pref_filter_library_tracked_${id}_v2` | `TriState.DISABLED` |
| `LibraryPreferences.kt` | `newMangaUpdatesCount()` | `getInt` | `library_unread_updates_count` | `0` |
| `LibraryPreferences.kt` | `newAnimeUpdatesCount()` | `getInt` | `library_unseen_updates_count` | `0` |
| `LibraryPreferences.kt` | `defaultAnimeCategory()` | `getInt` | `default_anime_category` | `-1` |
| `LibraryPreferences.kt` | `defaultMangaCategory()` | `getInt` | `default_category` | `-1` |
| `LibraryPreferences.kt` | `lastUsedAnimeCategory()` | `getInt` | `__APP_STATE_last_used_anime_category` | `0` |
| `LibraryPreferences.kt` | `lastUsedMangaCategory()` | `getInt` | `__APP_STATE_last_used_category` | `0` |
| `LibraryPreferences.kt` | `animeUpdateCategories()` | `getStringSet` | `animelib_update_categories` | `emptySet()` |
| `LibraryPreferences.kt` | `mangaUpdateCategories()` | `getStringSet` | `library_update_categories` | `emptySet()` |
| `LibraryPreferences.kt` | `animeUpdateCategoriesExclude()` | `getStringSet` | `animelib_update_categories_exclude` | `emptySet()` |
| `LibraryPreferences.kt` | `mangaUpdateCategoriesExclude()` | `getStringSet` | `library_update_categories_exclude` | `emptySet()` |
| `LibraryPreferences.kt` | `filterEpisodeBySeen()` | `getLong` | `default_episode_filter_by_seen` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterChapterByRead()` | `getLong` | `default_chapter_filter_by_read` | `Manga.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterEpisodeByDownloaded()` | `getLong` | `default_episode_filter_by_downloaded` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterChapterByDownloaded()` | `getLong` | `default_chapter_filter_by_downloaded` | `Manga.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterEpisodeByBookmarked()` | `getLong` | `default_episode_filter_by_bookmarked` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterChapterByBookmarked()` | `getLong` | `default_chapter_filter_by_bookmarked` | `Manga.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterEpisodeByFillermarked()` | `getLong` | `default_episode_filter_by_fillermarked` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `sortEpisodeBySourceOrNumber()` | `getLong` | `default_episode_sort_by_source_or_number` | `Anime.EPISODE_SORTING_SOURCE` |
| `LibraryPreferences.kt` | `sortChapterBySourceOrNumber()` | `getLong` | `default_chapter_sort_by_source_or_number` | `Manga.CHAPTER_SORTING_SOURCE` |
| `LibraryPreferences.kt` | `displayEpisodeByNameOrNumber()` | `getLong` | `default_chapter_display_by_name_or_number` | `Anime.EPISODE_DISPLAY_NAME` |
| `LibraryPreferences.kt` | `displayChapterByNameOrNumber()` | `getLong` | `default_chapter_display_by_name_or_number` | `Manga.CHAPTER_DISPLAY_NAME` |
| `LibraryPreferences.kt` | `sortEpisodeByAscendingOrDescending()` | `getLong` | `default_chapter_sort_by_ascending_or_descending` | `Anime.EPISODE_SORT_DESC` |
| `LibraryPreferences.kt` | `sortChapterByAscendingOrDescending()` | `getLong` | `default_chapter_sort_by_ascending_or_descending` | `Manga.CHAPTER_SORT_DESC` |
| `LibraryPreferences.kt` | `showEpisodeThumbnailPreviews()` | `getLong` | `default_episode_show_thumbnail_previews` | `Anime.EPISODE_SHOW_PREVIEWS` |
| `LibraryPreferences.kt` | `showEpisodeSummaries()` | `getLong` | `default_episode_show_summaries` | `Anime.EPISODE_SHOW_SUMMARIES` |
| `LibraryPreferences.kt` | `filterSeasonByDownload()` | `getLong` | `default_season_filter_by_downloaded` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterSeasonByUnseen()` | `getLong` | `default_season_filter_by_unseen` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterSeasonByStarted()` | `getLong` | `default_season_filter_by_started` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterSeasonByCompleted()` | `getLong` | `default_season_filter_by_completed` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterSeasonByBookmarked()` | `getLong` | `default_season_filter_by_bookmarked` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `filterSeasonByFillermarked()` | `getLong` | `default_season_filter_by_fillermarked` | `Anime.SHOW_ALL` |
| `LibraryPreferences.kt` | `sortSeasonBySourceOrNumber()` | `getLong` | `default_season_sort_by_source_or_number` | `Anime.SEASON_SORT_SOURCE` |
| `LibraryPreferences.kt` | `sortSeasonByAscendingOrDescending()` | `getLong` | `default_season_sort_by_ascending_or_descending` | `Anime.SEASON_SORT_DESC` |
| `LibraryPreferences.kt` | `seasonDisplayGridMode()` | `getLong` | `default_season_grid_display_mode` | `SeasonDisplayMode.toLong(SeasonDisplayMode.CompactGrid)` |
| `LibraryPreferences.kt` | `seasonDisplayGridSize()` | `getInt` | `default_season_grid_display_size` | `0` |
| `LibraryPreferences.kt` | `seasonDownloadOverlay()` | `getBoolean` | `default_season_download_overlay` | `false` |
| `LibraryPreferences.kt` | `seasonUnseenOverlay()` | `getBoolean` | `default_season_unseen_overlay` | `true` |
| `LibraryPreferences.kt` | `seasonLocalOverlay()` | `getBoolean` | `default_season_local_overlay` | `true` |
| `LibraryPreferences.kt` | `seasonLangOverlay()` | `getBoolean` | `default_season_lang_overlay` | `false` |
| `LibraryPreferences.kt` | `seasonContinueOverlay()` | `getBoolean` | `default_season_continue_overlay` | `true` |
| `LibraryPreferences.kt` | `seasonDisplayMode()` | `getLong` | `default_season_display_mode` | `Anime.SEASON_DISPLAY_MODE_SOURCE` |
| `LibraryPreferences.kt` | `updateSeasonOnRefresh()` | `getBoolean` | `pref_update_season_on_refresh` | `false` |
| `LibraryPreferences.kt` | `updateSeasonOnLibraryUpdate()` | `getBoolean` | `pref_update_season_on_library_update` | `false` |
| `LibraryPreferences.kt` | `swipeEpisodeStartAction()` | `getEnum` | `pref_episode_swipe_end_action` | `EpisodeSwipeAction.ToggleSeen` |
| `LibraryPreferences.kt` | `swipeEpisodeEndAction()` | `getEnum` | `pref_episode_swipe_start_action` | `EpisodeSwipeAction.ToggleBookmark` |
| `LibraryPreferences.kt` | `swipeChapterStartAction()` | `getEnum` | `pref_chapter_swipe_end_action` | `ChapterSwipeAction.ToggleRead` |
| `LibraryPreferences.kt` | `swipeChapterEndAction()` | `getEnum` | `pref_chapter_swipe_start_action` | `ChapterSwipeAction.ToggleBookmark` |
| `LibraryPreferences.kt` | `markDuplicateReadChapterAsRead()` | `getStringSet` | `mark_duplicate_read_chapter_read` | `emptySet()` |
| `LibraryPreferences.kt` | `markDuplicateSeenEpisodeAsSeen()` | `getStringSet` | `mark_duplicate_seen_episode_seen` | `emptySet()` |
| `StoragePreferences.kt` | `baseStorageDirectory()` | `getString` | `__APP_STATE_storage_dir` | `folderProvider.path()` |

<!-- rows=233 -->

### 2.1 Keys read by this area that are defined in other areas' files

| Key | Owner file | Default | Used by |
|---|---|---|---|
| `pref_theme_mode_key` and the other `UiPreferences` keys | `UiPreferences.kt` (listed above) | — | §5 |
| `anilist_score_type` | `TrackPreferences.kt` | `Anilist.POINT_10` | area 28 |
| `show_nsfw_source` | `SourcePreferences.kt` | `true` | §8 |
| `hidden_anime_catalogues`, `hidden_catalogues` | `SourcePreferences.kt` | empty | area 40/41 (not written here) |
| `trusted_extensions` (app state) | `SourcePreferences.kt` | empty | §10 (revoke) |
| `extension_repos`, `anime_extension_repos` | `SourcePreferences.kt` | empty | §8 (counts) |

---

## 3. Files, folders and formats on disk

### 3.1 Android (observed)

| Item | Path / name | Format | Written by | Read by |
|---|---|---|---|---|
| Preferences | SharedPreferences file (Android default name for `AndroidPreferenceStore(app)`) | XML key/value (`SharedPreferences`) | all `*Preferences` | all screens |
| Chapter/episode page cache | `context.cacheDir/chapter_disk_cache` (DiskLruCache, `PARAMETER_APP_VERSION`, `PARAMETER_VALUE_COUNT`, `PARAMETER_CACHE_SIZE` constants in `ChapterCache.kt`) | DiskLruCache files `<md5>.0` (JSON page list or image bytes); `journal`, `journal.tmp`, `journal.bkp` | `ChapterCache` | `ChapterCache`, Clear cache row |
| Crash log export | `context.createFileInCacheDir("aniyomi_crash_logs.txt")` (cache dir), shared via `toShareIntent` | UTF-8 text: debug info block, optional "Problematic extensions:" list, exception, then `logcat *:E -d -v year -v zone` output | `CrashLogUtil.dumpLogs` | user |
| Library CSV export | user-chosen URI, suggested name `aniyomi_library.csv` | CSV (see behaviour §11) | `LibraryExporter.exportToCsv` | user |
| WebView data | `<dataDir>/app_webview/` | Chromium profile | WebView | cleared by "Clear WebView data" |
| Backup location | `StorageManager.getAutomaticBackupsDirectory()` (area 13) | `.tachibk` backup files | `BackupCreateJob` | `BackupRestoreJob` |
| Storage base | `__APP_STATE_storage_dir` (default `folderProvider.path()`) | SAF tree URI string | `SettingsDataScreen` picker | `StorageManager` |

### 3.2 Linux target (Proposal)

| Item | Path | Format | Notes |
|---|---|---|---|
| Preferences DB | `$XDG_DATA_HOME/kurobayomi/kurobayomi.db` (fallback `~/.local/share/kurobayomi/`), table `prefs` | SQLite, values JSON text | Same DB as the rest of the app (area 03). |
| Chapter cache | `$XDG_CACHE_HOME/kurobayomi/chapter_cache/` (fallback `~/.cache/…`) | one file per key: `<sha256(key)>.json` for page lists and `<sha256(url)>.bin` for images; a `index.json` manifest (Proposal) | Replaces DiskLruCache; md5 replaced by sha256 for new files. |
| Crash log | `$XDG_STATE_HOME/kurobayomi/crash-logs.txt` | UTF-8 text | Opened via `gtk::FileLauncher`. |
| Storage base directory | stored path (plain UTF-8) in `__APP_STATE_storage_dir`; default `$XDG_DATA_HOME/kurobayomi/storage` (Proposal) | directory | Validated as an existing, writable directory. |
| Tracker secrets | OS keyring (Secret Service / KWallet) entries `kurobayomi-tracker-<id>` | keyring | Fallback file `$XDG_DATA_HOME/kurobayomi/secrets/<id>.json` mode 0600. |
| Library CSV | user-chosen path | CSV | Default file name `kurobayomi_library.csv` (Proposal). |
| Licences | embedded at build time | JSON | `cargo about generate` → `include_str!`. |

---

## 4. Network endpoints touched by this area

| Purpose | Method | URL (exact) | Params / headers | Auth | Response | Source |
|---|---|---|---|---|---|---|
| DoH provider Cloudflare (`PREF_DOH_CLOUDFLARE = 1`) | GET (DoH) | `https://cloudflare-dns.com/dns-query` | DoH wire format (OkHttp `DnsOverHttps`) | none | DNS message | `core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt:28` (`dohCloudflare`) |
| DoH Google (`2`) | GET | `https://dns.google/dns-query` | DoH | none | DNS | `DohProviders.kt:45` |
| DoH AdGuard (`3`) | GET | `https://dns-unfiltered.adguard.com/dns-query` | DoH | none | DNS | `DohProviders.kt:59` |
| DoH Quad9 (`4`) | GET | `https://dns.quad9.net/dns-query` | DoH | none | DNS | `DohProviders.kt:71` |
| DoH AliDNS (`5`) | GET | `https://dns.alidns.com/dns-query` | DoH | none | DNS | `DohProviders.kt:83` |
| DoH DNSPod (`6`) | GET | `https://doh.pub/dns-query` | DoH | none | DNS | `DohProviders.kt:95` |
| DoH 360 (`7`) | GET | `https://doh.360.cn/dns-query` | DoH | none | DNS | `DohProviders.kt:105` |
| DoH Quad 101 (`8`) | GET | `https://dns.twnic.tw/dns-query` | DoH | none | DNS | `DohProviders.kt:120` |
| DoH Mullvad (`9`) | GET | `https://dns.mullvad.net/dns-query` (**source literal has a leading space**: `" https://dns.mullvad.net/dns-query"`, line 136) | DoH | none | DNS | `DohProviders.kt:136` |
| DoH Control D (`10`) | GET | `https://freedns.controld.com/p0` | DoH | none | DNS | `DohProviders.kt:151` |
| DoH Njalla (`11`) | GET | `https://dns.njal.la/dns-query` | DoH | none | DNS | `DohProviders.kt:167` |
| DoH Shecan (`12`) | GET | `https://free.shecan.ir/dns-query` | DoH | none | DNS | `DohProviders.kt:180` |
| DoH LibreDNS (`13`) | GET | `https://doh.libredns.gr/dns-query` | DoH | none | DNS | `DohProviders.kt:193` |
| MyAnimeList OAuth authorize | browser navigation | `https://myanimelist.net/v1/oauth2/authorize` (from `BASE_OAUTH_URL` + `/authorize`) | `forceDefaultBrowser` | — | redirect | `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt:385,392` |
| AniList OAuth authorize | browser navigation | `https://anilist.co/api/v2/oauth/authorize` (`BASE_URL + "oauth/authorize"`) | — | — | redirect | `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt:572,584` |
| Shikimori / Simkl / Bangumi authorize | browser navigation | `LOGIN_URL` / `authUrl()` builders | — | — | — | `ShikimoriApi.kt:261`, `SimklApi.kt:248`, `BangumiApi.kt:279` (area 31/32) |
| Tracker help guide | browser | `https://aniyomi.org/help/guides/tracking/` | — | — | HTML | `SettingsTrackingScreen.kt:80` |
| Storage FAQ | browser | `https://aniyomi.org/docs/faq/storage` | — | — | HTML | `SettingsDataScreen.kt:89` |
| Extension Shizuku download | browser | `https://shizuku.rikka.app/download` | — | — | HTML | `SettingsAdvancedScreen.kt:431` (task 085 drop) |
| Battery optimisation help | browser | `https://dontkillmyapp.com/` | — | — | HTML | `SettingsAdvancedScreen.kt:182` (task 073 drop) |
| Translation guide | browser | `https://aniyomi.org/docs/contribute#translation` | — | — | HTML | `AboutScreen.kt:152` |
| Privacy | browser | `https://aniyomi.org/privacy/` | — | — | HTML | `AboutScreen.kt:168` |
| Website | browser | `https://aniyomi.org` | — | — | HTML | `AboutScreen.kt:182` |
| Discord | browser | `https://discord.gg/F32UjdJZrR` | — | — | — | `AboutScreen.kt:187` |
| GitHub repo | browser | `https://github.com/aniyomiorg/aniyomi` | — | — | — | `AboutScreen.kt:192` |
| Release page | browser | `https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG` | — | — | HTML | `AppUpdateChecker.kt:59` (`RELEASE_URL`) |
| Update check | GET (via `GetApplicationRelease`, area 35) | endpoint defined in area 35 (`AppUpdateChecker.checkForUpdate(context, forceCheck = true)`, `AppUpdateChecker.kt:14-…`) | — | none | release JSON | area 35 |
| Data saver (Bandwidth Hero / wsrv.nl / resmush.it) | GET (image requests) | server from `data_saver_server` (Bandwidth Hero) or the fixed service endpoints used by `DataSaver` (area 34) | — | none | image | area 34 |

Proposal: the Linux build keeps the same DoH URLs (reqwest + `hickory-resolver` with `https` upstream) and the same browser links.

---

## 5. Database tables and columns touched by the screens

### 5.1 Clear-database screens (exact SQL)

| Operation | Source file | SQL (verbatim fragment) | Notes |
|---|---|---|---|
| Manga: list sources with non-library manga | `data/src/main/sqldelight/data/mangas.sq:141-145` (`getSourceIdsWithNonLibraryManga`) | `SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source;` | Repository maps each `source` to a `Source` via `sourceManager.getOrStub(sourceId)` (`MangaSourceRepositoryImpl.kt:57-68`). |
| Manga: delete selected | `mangas.sq:147-150` (`deleteMangasNotInLibraryBySourceIds`) | `DELETE FROM mangas WHERE favorite = 0 AND source IN :sourceIds;` | `:sourceIds` is the selected source ids. |
| Manga: remove reset history | `data/src/main/sqldelight/data/history.sq:57-59` (`removeResettedHistory`) | `DELETE FROM history WHERE last_read = 0;` | Removes history rows that were reset to 0. |
| Anime: deletable parents (view) | `data/src/main/sqldelightanime/view/animedeletableView.sq` (`animedeletableView`) | `CREATE VIEW animedeletableView AS SELECT M._id, M.source, M.fetch_type FROM animes AS M WHERE M.favorite = 0 AND M.parent_id IS NULL;` | Query `getDeletableParentAnime`: `SELECT * FROM animedeletableView;` |
| Anime: children of a parent | `data/src/main/sqldelightanime/dataanime/animes.sq:157-160` (`getChildrenByParentId`) | `SELECT * FROM animes WHERE parent_id = :parentId;` | Used recursively for seasons. |
| Anime: delete selected non-library anime | `animes.sq:162-164` (`deleteAnimesNotInLibraryByAnimeIds`) | `DELETE FROM animes WHERE favorite = 0 AND _id IN :animeIds;` | `animeIds` = selected ids plus deletable non-library children. |
| Anime: detach library children | `animes.sq:152-155` (`removeParentIdByIds`) | `UPDATE animes SET parent_id = NULL WHERE _id IN :animeIds;` | `animeIds` here is the orphaned list (library children whose parent is removed). |
| Anime: remove reset history | `data/src/main/sqldelightanime/dataanime/animehistory.sq:54-56` (`removeResettedHistory`) | `DELETE FROM animehistory WHERE last_seen = 0;` | — |
| Anime: source list with counts | `GetAnimeSourcesWithNonLibraryAnime` → view above | (grouped in Kotlin by `sourceId`) | — |

Relationship: `animes.parent_id` links a season row to its parent anime. `fetch_type = Seasons` means the entry has children.

### 5.2 Other tables read by the screens (listed for dependency tracking)

| Screen | Table / query | Purpose |
|---|---|---|
| Library (categories) | `categories` (`GetMangaCategories.subscribe()`, `GetAnimeCategories.subscribe()`) | category names, order, ids |
| Library reset flags | `ResetMangaCategoryFlags` (area 08) | clears per-category sort/filter flags |
| Reader reset flags | `ResetMangaViewerFlags` (SQL owned by area 05; the `viewer` column is written in `mangas.sq` insert, line 154) | resets per-series viewer flags for all manga |
| Export favourites | `GetAnimeFavorites.await()` → `animes.sq:87-90` (`getFavorites`, `WHERE favorite = 1`); `GetMangaFavorites.await()` (`mangas.sq`, area 05) | rows exported to CSV |
| Covers refresh | `AnimeLibraryUpdateJob`, `MangaLibraryUpdateJob`, `AnimeMetadataUpdateJob`, `MangaMetadataUpdateJob` (areas 26, 34) | writes `cover_last_modified` and related columns |

### 5.3 Indices relevant to the above
- `animelib_favorite_index ON animes(favorite) WHERE favorite = 1` (`animes.sq:43`) — accelerates favourite listing.

---

## 6. Wire/format details for CSV export (exact)

- Row separator: `\r\n` (`LibraryExporter.kt:85`). No header. No trailing newline.
- Field separator: `,`.
- Escape set: `["\r", "\n", "\"", ","]` (`LibraryExporter.kt:63`). Quote with `"..."` and double internal quotes.
- Null/blank → empty field (`isNullOrBlank` → `""`).
- Column values: title (as stored), type `anime`/`manga` (lowercase enum name), author, artist.
- Encoding: `String.toByteArray()` (UTF-8 on Android). No BOM.
- File name suggested: `aniyomi_library.csv` (`SettingsDataScreen.kt:397`). MIME: `text/csv`.

## 7. Enumerated labels for default category lists

- Anime default category list: `[-1 → "Always ask"] + [id → visualName …]` (ordered as `GetAnimeCategories.subscribe()` emits).
- Manga default category list: same with manga categories.
- Download exclude lists: `id.toString() → visualName`.
