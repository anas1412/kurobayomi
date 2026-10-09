# Area 02 preferences system, part 1: behaviour specification

Conventions: "OBSERVED" is what the Kotlin does (with `path:line`); "Proposal:" is what Kurobayomi should do. All paths are repo-relative to `/home/blackbox/aniyomi-src`. Key-by-key tables are in `p1-topic-key-registry.md`; storage formats are in `p1-data-and-config.md`; Rust types are in `p1-rust-design.md`.

## 1. The preference contract (OBSERVED)

Sources: `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt`, `PreferenceStore.kt`, `AndroidPreference.kt`, `AndroidPreferenceStore.kt`.

### 1.1 Interfaces

`Preference<T>` (Preference.kt:7-23) has eight members: `key(): String`, `get(): T`, `set(value: T)`, `isSet(): Boolean`, `delete()`, `defaultValue(): T`, `changes(): Flow<T>`, `stateIn(scope): StateFlow<T>`.

`PreferenceStore` (PreferenceStore.kt:3-25) has: `getString(key, default = "")`, `getLong(key, default = 0)`, `getInt(key, default = 0)`, `getFloat(key, default = 0f)`, `getBoolean(key, default = false)`, `getStringSet(key, default = emptySet())`, `getObject(key, defaultValue, serializer: (T) -> String, deserializer: (String) -> T)`, `getAll(): Map<String, *>`. The extension `PreferenceStore.getEnum<T : Enum<T>>(key, defaultValue)` (PreferenceStore.kt:27-43) is `getObject` with `serializer = { it.name }` and `deserializer = { try { enumValueOf(it) } catch (e: IllegalArgumentException) { defaultValue } }`.

Note the defaults of the factory functions (`""`, `0`, `0`, `0f`, `false`, empty set) are used by `PreferenceRestorer` (it calls `preferenceStore.getInt(key)` without a default only to `set` a value) and by `DecoderPreferences` (`getInt("pref_player_filter_brightness")` etc. default 0).

### 1.2 Reading (`AndroidPreference.get`, AndroidPreference.kt:32-40)

1. Call `read(preferences, key, defaultValue)`. For the six primitive subclasses this is the matching `SharedPreferences.getX(key, defaultValue)`. `StringPrimitive.read` returns `getString(key, default) ?: default`; `StringSetPrimitive.read` returns `getStringSet(key, default) ?: default`.
2. If `read` throws `ClassCastException` (the stored value has another primitive type), log `"Invalid value for $key; deleting"`, call `delete()` (remove the key) and return `defaultValue`.
3. For `Object<T>.read` (AndroidPreference.kt:182-188): `preferences.getString(key, null)?.let(deserializer) ?: defaultValue`, wrapped in `try { ... } catch (e: Exception) { defaultValue }`. So a missing key, a deserializer returning null, or any exception all give the default, and the key is NOT deleted (the broken value stays on disk).

### 1.3 Writing

`set(value)` runs `preferences.edit(action = write(key, value))`: one `SharedPreferences.Editor` transaction using `putString`/`putLong`/`putInt`/`putFloat`/`putBoolean`/`putStringSet`; for `Object<T>` it is `putString(key, serializer(value))`. `edit` from `androidx.core.content.edit` calls `apply()` (asynchronous disk write, in-memory value visible immediately). `delete()` is `edit { remove(key) }`. `isSet()` is `preferences.contains(key)`. `defaultValue()` returns the constructor default.

There is no validation or clamping in the store; ranges are enforced only by the settings widgets (see section 5).

### 1.4 Change notification (`changes()`, AndroidPreference.kt:60-66; `keyFlow`, AndroidPreferenceStore.kt:68-79)

`keyFlow` is one `callbackFlow` per store: it registers a `SharedPreferences.OnSharedPreferenceChangeListener` that does `trySend(key)` for every changed key (the key argument can be `null` when the whole file was cleared) and unregisters on close. `changes()` is:

```
keyFlow
  .filter { it == key || it == null }   // own key or "cleared"
  .onStart { emit("ignition") }         // emit once immediately
  .map { get() }                        // re-read current value
  .conflate()                           // slow collectors see only the latest
```

So every collector first gets the current value, then one value per change of that key (or per clear). Because `get()` is re-read, a quick burst `set(1); set(2)` can yield only `2`. `stateIn(scope)` is `changes().stateIn(scope, SharingStarted.Eagerly, get())` (initial value read synchronously at call time).

`InMemoryPreferenceStore.InMemoryPreference.changes()` is `flow { data }`, which never emits anything (a bug, only used in previews); `getStringSet` throws `NotImplementedError` ("Not yet implemented") (InMemoryPreferenceStore.kt:50-52). See `p1-topic-quirks-and-bugs.md` Q10.

### 1.5 Helper functions (Preference.kt:52-76)

| Helper | Behaviour |
|---|---|
| `Preference<T>.getAndSet(block)` | `set(block(get()))` (non-atomic read-modify-write) |
| `Preference<T>.deleteAndGet()` | `delete(); return get()` (resets to default and returns it; used by the subtitle "reset" button, see AniyomiMPVView / SubtitleSettingsTypographyCard: `preferences.subtitleFont().deleteAndGet()`) |
| `Preference<Set<T>> += item` | `set(get() + item)` |
| `Preference<Set<T>> += items` | `set(get() + items)` |
| `Preference<Set<T>> -= item` | `set(get() - item)` |
| `Preference<Boolean>.toggle()` | `set(!get()); return get()` |

### 1.6 Singletons

`PreferenceModule` (`app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt`) registers exactly one `PreferenceStore` (`AndroidPreferenceStore(app)` over `PreferenceManager.getDefaultSharedPreferences(context)`, i.e. file `<package>_preferences.xml`) and one singleton of each class: `NetworkPreferences(preferenceStore, verboseLogging = isDebugBuildType)`, `SourcePreferences`, `SecurityPreferences`, `LibraryPreferences`, `ReaderPreferences`, `PlayerPreferences`, `GesturePreferences`, `DecoderPreferences`, `SubtitlePreferences`, `AudioPreferences`, `TorrentPreferences`, `AdvancedPlayerPreferences`, `TrackPreferences`, `DownloadPreferences`, `BackupPreferences`, `StoragePreferences(folderProvider = AndroidStorageFolderProvider, preferenceStore)`, `UiPreferences`, `BasePreferences(app, preferenceStore)`. The 18 classes share one flat key namespace.

Proposal: one `Prefs` struct built once in the app context (area:01-foundation-app-bootstrap) holding the 18 typed sections; sections receive a clone of the shared `PrefStore` handle.

## 2. Key namespaces (OBSERVED, Preference.kt:25-49)

| Namespace | Prefix | Helper | Purpose | In backup? |
|---|---|---|---|---|
| normal | none | none | user preferences | yes |
| app-state | `__APP_STATE_` | `Preference.appStateKey(key)`, `isAppState(key)` | internal state (last-used source, onboarding done, current storage dir, timestamps, incognito flag) | never: `PreferenceBackupCreator.toBackupPreferences` does `filterKeys { !Preference.isAppState(it) }` |
| private | `__PRIVATE_` | `Preference.privateKey(key)`, `isPrivate(key)` | tracker credentials and tokens | only when the backup option `privateSettings` is on (default false, `BackupOptions.kt:19`) |

Keys in each namespace in scope: app-state (13 in scope): `pref_downloaded_only`, `incognito_mode`, `onboarding_complete` (BasePreferences); `trusted_extensions`, `has_filters_toggle_state`, `last_anime_catalogue_source`, `last_catalogue_source` (SourcePreferences); `library_update_last_timestamp`, `last_used_anime_category`, `last_used_category` (LibraryPreferences); `last_app_closed` (SecurityPreferences); `last_auto_backup_timestamp` (BackupPreferences); `storage_dir` (StoragePreferences); plus `last_version_code` used by `App.kt:169` outside the scope. Private (4 per-tracker key families, three strings and one boolean): `pref_mangasync_username_<trackerId>`, `pref_mangasync_password_<trackerId>`, `pref_tracker_auth_expired_<trackerId>`, `track_token_<trackerId>`. The registry lists them all.

Proposal: the three namespaces become three physical stores (section 1 of `p1-data-and-config.md`): `config.toml` (normal), `state.toml` (app-state), OS keyring (private strings). The key strings keep their Aniyomi spelling inside the in-memory registry so backup import/export is a straight copy.

## 3. Codecs for non-primitive preferences

### 3.1 Enum codec (OBSERVED)

Stored as `Enum.name` (exact Kotlin constant spelling). Unknown or corrupt name yields the default silently (no delete). Enum constants must therefore never be renamed. Proposal: Rust enums use `#[serde(rename = "...")]` with the exact Kotlin names (e.g. `"MOVE_HISTORY_TO_MORE"`, `"ToggleSeen"`, `"SensorLandscape"`, `"None"`, `"ENABLED_IS"`) and `#[serde(other)]` is NOT used; instead the codec function returns the default on any parse error, mirroring Kotlin.

### 3.2 `LibraryDisplayMode` (LibraryDisplayMode.kt)

Variants: `CompactGrid`, `ComfortableGrid`, `List`, `CoverOnlyGrid`; `values = {CompactGrid, ComfortableGrid, List, CoverOnlyGrid}`; `default = CompactGrid`. Serialize: `COMFORTABLE_GRID`, `COMPACT_GRID`, `COVER_ONLY_GRID`, `LIST`. Deserialize: exact match on those four strings, anything else gives `default`. Used by `displayMode` (`pref_display_mode_library`) and `sourceDisplayMode` (`pref_display_mode_catalogue`).

### 3.3 Library sort (`MangaLibrarySort`, `AnimeLibrarySort`)

Files: `domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt`, `.../anime/model/AnimeLibrarySortMode.kt`, `.../library/model/Flag.kt`.

String form (stored in `library_sorting_mode` and `animelib_sorting_mode`): `"<TYPE>,<DIRECTION>"` with DIRECTION `ASCENDING` or `DESCENDING`.

Manga TYPE strings: `ALPHABETICAL`, `LAST_READ`, `LAST_MANGA_UPDATE`, `UNREAD_COUNT`, `TOTAL_CHAPTERS`, `LATEST_CHAPTER`, `CHAPTER_FETCH_DATE`, `DATE_ADDED`, `TRACKER_MEAN`, `RANDOM`. Anime TYPE strings: `ALPHABETICAL`, `LAST_SEEN`, `LAST_ANIME_UPDATE`, `UNSEEN_COUNT`, `TOTAL_EPISODES`, `LATEST_EPISODE`, `EPISODE_FETCH_DATE`, `DATE_ADDED`, `TRACKER_MEAN`, `AIRING_TIME`, `RANDOM`.

Deserialize algorithm: empty string gives default; `split(",")`; type by `when` with unknown giving `Alphabetical`; direction is `Ascending` only if `values[1] == "ASCENDING"` else `Descending`; any exception (for example a string without a comma: `values[1]` throws `IndexOutOfBoundsException`) gives default. Default (both) = `Alphabetical, Ascending`.

Integer flag form (the `flags` column of the `categories` table, `Category.flags`; read by `Category?.sort`): `flag = type.flag | direction.flag`, mask `type.mask | direction.mask`.

| Sort type | Manga flag | Anime flag |
|---|---|---|
| Alphabetical | `0b00000000` (0x00) | `0b00000000` |
| LastRead / LastSeen | `0b00000100` (0x04) | `0b00000100` |
| LastUpdate | `0b00001000` (0x08) | `0b00001000` |
| UnreadCount / UnseenCount | `0b00001100` (0x0C) | `0b00001100` |
| TotalChapters / TotalEpisodes | `0b00010000` (0x10) | `0b00010000` |
| LatestChapter / LatestEpisode | `0b00010100` (0x14) | `0b00010100` |
| ChapterFetchDate / EpisodeFetchDate | `0b00011000` (0x18) | `0b00011000` |
| DateAdded | `0b00011100` (0x1C) | `0b00011100` |
| TrackerMean | `0b000100000` (0x20) | `0b000100000` (0x20) |
| AiringTime | not present | `0b00110000` (0x30) |
| Random | `0b00111100` (0x3C) | `0b00111100` (0x3C) |

Type mask = `0b00111100` (0x3C). Direction: `Ascending = 0b01000000` (0x40), `Descending = 0b00000000`, mask `0b01000000`. `Type.valueOf(flag)` = first type with `type.flag == flag and mask`, else default type (Alphabetical); `Direction.valueOf(flag)` likewise (else default direction Ascending). `Category?.sort` = `valueOf(this?.flags)`; null category gives default. Flag algebra (`Flag.kt`): `a + b` where `b` has a mask = `(a and mask.inv()) or (b.flag and mask)`; `Long.contains(flag)` = `flag.flag == this and flag.mask`.

Edge: in the 0x20 TrackerMean flag, `0x20 and 0x3C = 0x20`, so it does not collide with AiringTime (0x30). Types 0x24, 0x28, 0x2C, 0x34, 0x38 are unassigned and fall back to Alphabetical.

### 3.4 `SeasonDisplayMode` (SeasonDisplayMode.kt)

Stored as a Long preference `default_season_grid_display_mode`: `CompactGrid`=0, `ComfortableGrid`=1, `CoverOnlyGrid`=2, `List`=3. `fromLong` throws `IllegalArgumentException("Invalid sorting value")` for any other value. Note the order differs from `LibraryDisplayMode` strings (there is no numeric form for LibraryDisplayMode).

### 3.5 `TriState` and `CheckboxState`

`TriState` (`TriState.kt`): `DISABLED` ("Disable filter"), `ENABLED_IS` ("Enabled with 'is' filter"), `ENABLED_NOT` ("Enabled with 'not' filter"); `next()` cycles DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED. `applyFilter(filter, predicate)` (`domain/src/main/java/tachiyomi/domain/entries/TriState.kt`): DISABLED gives `true`, ENABLED_IS gives `predicate()`, ENABLED_NOT gives `!predicate()`.

`CheckboxState<T>` (`CheckboxState.kt`): `State.Checked(value)`, `State.None(value)` with `next()` toggling; `TriState.Include(value)`, `TriState.Exclude(value)`, `TriState.None(value)` with `next()`: Exclude -> None, Include -> Exclude, None -> Include (note: Include goes to Exclude, not to None, and the cycle is None -> Include -> Exclude -> None). Helpers `T.asCheckboxState(condition)` (Checked if `condition(this)` else None) and `List<T>.mapAsCheckboxState(condition)`. Used by category selection dialogs; no persistence.

## 4. Per-entry default templates in `LibraryPreferences` (OBSERVED, LibraryPreferences.kt:171-349)

The `default_*` keys are not read by viewers directly. They are the template written into the per-entry flag columns the first time an entry is created/opened or when the user chooses "Set as default" in the chapter/episode settings sheet. Bit layouts are in `p1-data-and-config.md` section 3.

`setChapterSettingsDefault(manga)` (lines 249-258) writes: `filterChapterByRead <- manga.unreadFilterRaw`, `filterChapterByDownloaded <- manga.downloadedFilterRaw`, `filterChapterByBookmarked <- manga.bookmarkedFilterRaw`, `sortChapterBySourceOrNumber <- manga.sorting`, `displayChapterByNameOrNumber <- manga.displayMode`, `sortChapterByAscendingOrDescending <- if (manga.sortDescending()) CHAPTER_SORT_DESC else CHAPTER_SORT_ASC`.

`setEpisodeSettingsDefault(anime)` (lines 235-247) writes the same plus `filterEpisodeByFillermarked`, `showEpisodeThumbnailPreviews <- anime.showPreviewsRaw`, `showEpisodeSummaries <- anime.showSummariesRaw`.

`setSeasonSettingsDefault(anime)` (lines 330-349) writes 16 values: `filterSeasonByDownload <- anime.seasonUnseenFilterRaw` (BUG, should be `seasonDownloadedFilterRaw`), `filterSeasonByUnseen`, `filterSeasonByStarted`, `filterSeasonByCompleted`, `filterSeasonByBookmarked`, `filterSeasonByFillermarked`, `sortSeasonBySourceOrNumber <- anime.seasonSorting`, `sortSeasonByAscendingOrDescending`, `seasonDisplayGridMode <- SeasonDisplayMode.toLong(anime.seasonDisplayGridMode)`, `seasonDisplayGridSize`, the five overlay booleans, `seasonDisplayMode <- anime.seasonDisplayMode`.

Shared keys (OBSERVED, a latent coupling): `displayEpisodeByNameOrNumber()` and `displayChapterByNameOrNumber()` both use key `default_chapter_display_by_name_or_number`; `sortEpisodeByAscendingOrDescending()` and `sortChapterByAscendingOrDescending()` both use `default_chapter_sort_by_ascending_or_descending`. Consequence: setting the anime default display (name or number) also changes the manga default and vice versa. Constants are numerically identical (name 0x0, number 0x100000; DESC 0x0, ASC 0x1) so values are valid for both. Proposal: keep the shared key strings on the Aniyomi-compat layer (backup import/export), but in Rust keep two separate fields and write both keys on export (anime and manga default each stored; on import the single value initialises both). See `p1-topic-quirks-and-bugs.md` Q2.

## 5. Per-class behaviour

### 5.1 BasePreferences (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt`)

* `downloadedOnly` (`__APP_STATE_pref_downloaded_only`, default false): "Downloaded only" mode, filters library/updates/browse to downloaded items (consumed by area:37-ui-library, area:40-ui-browse-manga/41). Not backed up. Proposal: `State.base.downloaded_only`, a toggle in the app-wide menu; a banner is shown whenever it is on (message key `MR.strings.downloaded_only`).
* `incognitoMode` (`__APP_STATE_incognito_mode`, default false): history and progress tracking suppressed (area:42-ui-updates-history-stats-storage-upcoming). Not backed up. Combined with per-extension incognito sets in `SourcePreferences`.
* `shownOnboardingFlow` (`__APP_STATE_onboarding_complete`, default false).
* `deviceHasPip()` is not a preference: `packageManager.hasSystemFeature(FEATURE_PICTURE_IN_PICTURE)`. Proposal: constant `false` (PiP dropped) so any code branch guarded by it is dead.
* `displayProfile` (`pref_display_profile_key`, default ""), `hardwareBitmapThreshold` (`pref_hardware_bitmap_threshold`, default `GLUtil.SAFE_TEXTURE_LIMIT` = 2048; `core/common/.../GLUtil.kt` also computes `DEVICE_TEXTURE_LIMIT` = max EGL pbuffer width over all EGL configs but at least 2048, and `CUSTOM_TEXTURE_LIMIT_OPTIONS` = `[DEVICE_TEXTURE_LIMIT]` followed by `step * 1024` for `step` from `DEVICE_TEXTURE_LIMIT / 1024` down to 2, skipping values `>= DEVICE_TEXTURE_LIMIT`), `alwaysDecodeLongStripWithSSIV` (default false). All three Android-only; Dropped.
* `ExtensionInstaller` enum (BasePreferences.kt:31-36): `LEGACY(MR.strings.ext_installer_legacy, requiresSystemPermission = true)`, `PACKAGEINSTALLER(MR.strings.ext_installer_packageinstaller, true)`, `SHIZUKU(MR.strings.ext_installer_shizuku, false)`, `PRIVATE(MR.strings.ext_installer_private, false)`.

### 5.2 ExtensionInstallerPreference (`ExtensionInstallerPreference.kt`)

A `Preference<ExtensionInstaller>` decorator over `preferenceStore.getEnum("extension_installer", defaultValue())`.

* `defaultValue()`: `LEGACY` if `context.hasMiuiPackageInstaller` else `PACKAGEINSTALLER`.
* `entries`: all four values, minus `PACKAGEINSTALLER` on MIUI.
* `check(value)`: PACKAGEINSTALLER on MIUI returns LEGACY; SHIZUKU when Shizuku is not installed returns `defaultValue()`; others unchanged.
* `get()`: read base value, run `check`, and if it changed, WRITE the corrected value back (a read can write).
* `set(value)`: `basePref.set(check(value))`.
* `MigrateSecureScreenMigration.kt` (outside scope) also forces LEGACY on MIUI.

Proposal: Dropped (section 5.1 / task T-02a-032). The stored string is carried in `Settings.base.extension_installer: String` untouched so a restored Aniyomi backup re-exports unchanged.

### 5.3 UiPreferences and UI enums (`UiPreferences.kt`, `ui/model/*.kt`)

| Key | Behaviour |
|---|---|
| `pref_theme_mode_key` | `ThemeMode` default SYSTEM. `setAppCompatDelegateThemeMode`: LIGHT -> `MODE_NIGHT_NO`, DARK -> `MODE_NIGHT_YES`, SYSTEM -> `MODE_NIGHT_FOLLOW_SYSTEM`. Proposal: LIGHT/DARK set `gtk-application-prefer-dark-theme` false/true on the default `gtk::Settings`; SYSTEM reads the xdg-desktop-portal setting `org.freedesktop.appearance color-scheme` (0 no preference, 1 dark, 2 light) and subscribes to `SettingChanged`; if the portal is unavailable, leave GTK's own value alone. |
| `pref_app_theme` | `AppTheme` default MONET if `DeviceUtil.isDynamicColorAvailable` (Material You available, or Samsung with API >= S) else DEFAULT. Proposal default DEFAULT (see `p1-rust-design.md` section 9 about theme ownership). |
| `pref_theme_dark_amoled_key` | bool false. Dark theme uses pure black surfaces. |
| `relative_time_v2` | bool true. Consumed by `LocalDate.toRelativeString` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:47-72`). With relative off: `dateFormat.format(date)`. With relative on, `difference = days_between(date, today)`: `< -7` formatted date; `< 0` plural `MR.plurals.upcoming_relative_time` with `abs(difference)`; `< 1` (today) `MR.strings.relative_time_today`; `< 7` plural `MR.plurals.relative_time` with `difference`; otherwise formatted date. The `LocalDateTime` variant (`DateExtensions.kt:75`) additionally uses hours (`AYMR.plurals.upcoming_relative_time_hours`) within the same day. Both are read once per composition (`remember { preferences.relativeTime().get() }`, `DateText.kt:51`), so a change applies on the next screen. |
| `app_date_format` | string, `""` default. `UiPreferences.dateFormat(format)`: `""` gives `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`, otherwise `DateTimeFormatter.ofPattern(format, Locale.getDefault())`. |
| `tablet_ui_mode` | `TabletUiMode` default AUTOMATIC; titles `MR.strings.automatic_background`, `MR.strings.lock_always`, `MR.strings.landscape`, `MR.strings.lock_never`. Inert on desktop. |
| `start_screen` | `StartScreen` default ANIME. Titles: ANIME `AYMR.strings.label_anime`, MANGA `AYMR.strings.manga`, UPDATES `MR.strings.label_recent_updates`, HISTORY `MR.strings.label_recent_manga`, BROWSE `MR.strings.browse`. Maps to tabs `AnimeLibraryTab`, `MangaLibraryTab`, `UpdatesTab`, `HistoriesTab`, `BrowseTab`. |
| `bottom_rail_nav_style` | `NavStyle` default MOVE_HISTORY_TO_MORE. Titles `AYMR.strings.pref_bottom_nav_no_manga`, `_no_updates`, `_no_history`, `_no_browse`. `tabs` = list `[AnimeLibraryTab, MangaLibraryTab, UpdatesTab, HistoriesTab, BrowseTab, MoreTab]` with the style's `moreTab` removed (so exactly 5 visible tabs; the removed one is reachable via More). Icons: MOVE_MANGA_TO_MORE `Icons.Outlined.CollectionsBookmark`, MOVE_UPDATES_TO_MORE `R.drawable.ic_updates_outline_24dp`, MOVE_HISTORY_TO_MORE `Icons.Outlined.History`, MOVE_BROWSE_TO_MORE `Icons.Outlined.Explore`. |

`AppTheme` titles (AppTheme.kt:7-31): DEFAULT `MR.strings.label_default`, MONET `MR.strings.theme_monet`, CLOUDFLARE `AYMR.strings.theme_cloudflare`, COTTONCANDY `AYMR.strings.theme_cottoncandy`, DOOM `AYMR.strings.theme_doom`, GREEN_APPLE `MR.strings.theme_greenapple`, LAVENDER `MR.strings.theme_lavender`, MATRIX `AYMR.strings.theme_matrix`, MIDNIGHT_DUSK `MR.strings.theme_midnightdusk`, MOCHA `AYMR.strings.theme_mocha`, SAPPHIRE `AYMR.strings.theme_sapphire`, NORD `MR.strings.theme_nord`, STRAWBERRY_DAIQUIRI `MR.strings.theme_strawberrydaiquiri`, TAKO `MR.strings.theme_tako`, TEALTURQUOISE `MR.strings.theme_tealturquoise`, TIDAL_WAVE `MR.strings.theme_tidalwave`, YINYANG `MR.strings.theme_yinyang`, YOTSUBA `MR.strings.theme_yotsuba`, MONOCHROME `MR.strings.theme_monochrome`; DARK_BLUE, HOT_PINK, BLUE are "Deprecated" with `titleRes = null` (hidden from the picker; migrated elsewhere; still valid stored names).

Date-pattern translation (Proposal): the pattern list offered by the Aniyomi UI is `""`, `MM/dd/yy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd MMM yyyy`, `MMM dd, yyyy`. Implement `datefmt::format(pattern, date)` with a small translator java-pattern -> chrono `strftime`: `yyyy`->`%Y`, `yy`->`%y`, `MMM`->`%b`, `MM`->`%m`, `dd`->`%d`; unknown letters make the translator fall back to the empty-pattern behaviour (locale short date via `glib::DateTime::format("%x")`). Empty pattern gives locale short date, like Kotlin.

### 5.4 SourcePreferences (`SourcePreferences.kt`)

* `source_languages` default `LocaleHelper.getDefaultEnabledLanguages()` = `setOf("all", "en", Locale.getDefault().language)`. Proposal: derive `<system language>` from `LC_MESSAGES`/`LANG` (first two letters, lower case) at first run; once the user edits the set, it is stored and never re-derived. Edge: `LANG=C` or `POSIX` gives `{"all","en"}`.
* `show_nsfw_source` default true. When false NSFW-flagged sources are hidden from browse and global search.
* `pref_migration_sorting` (`SetMigrateSorting.Mode`: ALPHABETICAL, TOTAL) default ALPHABETICAL and `pref_migration_direction` (`SetMigrateSorting.Direction`: ASCENDING, DESCENDING) default ASCENDING; both written together by `SetMigrateSorting.await(mode, direction)`.
* `anime_extension_repos`, `extension_repos`: legacy string sets (migrated into DB tables by the app migrations; area:11-app-migrations, area:20-extension-strategy-redesign). Kurobayomi reads them only to import from an old backup.
* `trusted_extensions`: app-state string set of trusted extension signature hashes (Android signing certificate SHA-256 hex). Proposal: the trust unit is decided by area:20-extension-strategy-redesign.
* `has_filters_toggle_state` (app-state, false): last state of the global-search "Only show results" filter toggle.
* `hidden_anime_catalogues` / `hidden_catalogues`: set of source ids (as decimal strings) that are disabled ("hidden") in the Sources tab. `pinned_anime_catalogues` / `pinned_catalogues`: pinned source ids. `incognito_anime_extensions` / `incognito_manga_extensions`: package names of extensions with incognito forced on.
* `last_anime_catalogue_source` / `last_catalogue_source`: app-state Long, `-1` = none.
* `animeext_updates_count` / `ext_updates_count`: Int counters for the extension-update badge (plain int, written by the extension-update checker; recomputable).
* `browse_hide_in_anime_library_items` / `browse_hide_in_library_items`: hide entries already in the library from browse grids.
* Data saver block (SY port): `data_saver` (`DataSaver`: NONE default, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT), `ignore_jpeg` (false), `ignore_gif` (true), `data_saver_image_quality` (80), `data_saver_image_format_jpeg` (false), `data_saver_server` ("" custom Bandwidth Hero server host), `data_saver_color_bw` (false), `data_saver_excluded` (set of source ids), `data_saver_downloader` (true: apply to downloads too). Image rewriting is area:34-images-covers-cache.

### 5.5 TrackPreferences (`TrackPreferences.kt`)

* `trackUsername(tracker)` key `__PRIVATE_pref_mangasync_username_<tracker.id>`, `trackPassword(tracker)` `__PRIVATE_pref_mangasync_password_<tracker.id>`, `trackAuthExpired(tracker)` `__PRIVATE_pref_tracker_auth_expired_<tracker.id>` (bool), `trackToken(tracker)` `__PRIVATE_track_token_<tracker.id>`. All default empty/false.
* `setCredentials(tracker, username, password)`: writes username, then password, then sets `trackAuthExpired` to false (three separate writes, not atomic).
* `anilist_score_type` default `Anilist.POINT_10` = `"POINT_10"`.
* `pref_auto_update_manga_sync_key` (true): push progress to trackers after reading/watching. `track_on_adding_to_library` (true): show the tracking dialog when favouriting. `show_next_episode_airing_time` (true). `pref_auto_update_manga_on_mark_read` (`AutoTrackState` default ALWAYS): titles ALWAYS `MR.strings.lock_always`, ASK `MR.strings.default_category_summary` (reused string "Always ask"), NEVER `MR.strings.lock_never`.
* Proposal: usernames/passwords/tokens are stored with the `SecretStore` trait (Secret Service), item attributes `service=kurobayomi`, `kind=tracker-<field>`, `tracker=<id>`. `trackAuthExpired` is not secret and goes to `state.toml`. The store trait is the sole code path allowed to touch these values.

### 5.6 LibraryPreferences non-template keys (`LibraryPreferences.kt:18-169`)

* Display: `pref_display_mode_library` (LibraryDisplayMode, CompactGrid), `library_sorting_mode` / `animelib_sorting_mode` (sort objects, section 3.3).
* `library_update_last_timestamp` (app-state Long 0): epoch ms of last library update. `pref_library_update_interval_key` (Int hours, 0 = off; UI 0, 12, 24, 48, 72, 168; migrations rewrite 1 and 2 to 3, and a retired quick-update value to 12).
* `library_update_restriction` (string set, default `{"wifi"}`): values `wifi` (`DEVICE_ONLY_ON_WIFI`), `network_not_metered` (`DEVICE_NETWORK_NOT_METERED`), `ac` (`DEVICE_CHARGING`). Consumed by `MangaLibraryUpdateJob.setupTask` (MangaLibraryUpdateJob.kt:~444-470): networkType = UNMETERED if `network_not_metered` present else CONNECTED; transport WIFI if `wifi`; requires charging if `ac`; always `setRequiresBatteryNotLow(true)`; periodic with flex 10 minutes and linear backoff 10 minutes. Proposal mapping in section 8.
* `library_update_manga_restriction` (set, default all four): `manga_ongoing` = `ENTRY_NON_COMPLETED`, `manga_fully_read` = `ENTRY_HAS_UNVIEWED`, `manga_started` = `ENTRY_NON_VIEWED`, `manga_outside_release_period` = `ENTRY_OUTSIDE_RELEASE_PERIOD`. Evaluation order and skip reasons (MangaLibraryUpdateJob.kt:178-222, anime version AnimeLibraryUpdateJob.kt:205-235), first matching rule skips the entry:
  1. `entry.updateStrategy != UpdateStrategy.ALWAYS_UPDATE` skips with `MR.strings.skipped_reason_not_always_update` (not a preference, always on);
  2. `ENTRY_NON_COMPLETED in restrictions && status == COMPLETED` skips with `MR.strings.skipped_reason_completed`;
  3. `ENTRY_HAS_UNVIEWED in restrictions && unreadCount != 0` (anime: `unseenCount != 0`) skips with `MR.strings.skipped_reason_not_caught_up`;
  4. `ENTRY_NON_VIEWED in restrictions && totalChapters > 0 && !hasStarted` skips with `MR.strings.skipped_reason_not_started`;
  5. `ENTRY_OUTSIDE_RELEASE_PERIOD in restrictions && nextUpdate > fetchWindowUpperBound` skips with `MR.strings.skipped_reason_not_in_release_period`.
  The kept list is sorted by `title`. Skip reasons are only logged. UI labels: `AYMR.strings.pref_update_only_completely_read` (HAS_UNVIEWED), `MR.strings.pref_update_only_started`, `MR.strings.pref_update_only_non_completed`, `MR.strings.pref_update_only_in_release_period`. The constant names say "unviewed" but the semantics are "skip entries that still have unread items"; keep the string values.
* Categories for update: `library_update_categories` and `animelib_update_categories` (include sets of category ids as strings; empty = all), `library_update_categories_exclude` and `animelib_update_categories_exclude` (exclude sets). Algorithm: included = entries whose category id is in the include set (all entries when the set is empty); excluded ids = entries in any exclude category; result = included minus excluded, distinct by entry id.
* `auto_update_metadata` (false): refetch entry details during update. `display_continue_reading_button` (false): Continue button on library cards. `display_category_tabs` (true), `display_number_of_items` (false), `categorized_display` (false: per-category display/sort/filter instead of one global), `hidden_categories` (false: hide hidden categories), `pref_filter_library_interval_custom` (TriState DISABLED: filter by custom update interval).
* Badges: `display_download_badge` (false), `display_unread_badge` (true), `display_local_badge` (true), `display_language_badge` (false), `library_show_updates_count` (true).
* `auto_clear_chapter_cache` (false): clear the page/video cache on app exit.
* `library_random_anime_sort_seed`, `library_random_manga_sort_seed` (Int 0): seed for the Random sort so the order is stable between recompositions; regenerated when the user re-selects Random.
* Columns: `pref_animelib_columns_portrait_key`, `pref_library_columns_portrait_key`, `pref_animelib_columns_landscape_key`, `pref_library_columns_landscape_key` (Int; 0 = "Auto" (label `MR.strings.label_auto`); the library settings sheet slider has `valueRange = 0..10`, label `MR.strings.pref_library_columns`, or `AYMR.strings.pref_library_rows` when the display mode is List; the landscape key is used when the window is landscape, the portrait key otherwise, `AnimeLibrarySettingsDialog.kt:250-283`).
* Filters (TriState, DISABLED): `pref_filter_animelib_downloaded_v2`, `pref_filter_library_downloaded_v2`, `pref_filter_animelib_unread_v2` (anime "unseen"), `pref_filter_library_unread_v2`, `pref_filter_animelib_started_v2`, `pref_filter_library_started_v2`, `pref_filter_animelib_bookmarked_v2`, `pref_filter_library_bookmarked_v2`, `pref_filter_animelib_completed_v2`, `pref_filter_library_completed_v2`, and per tracker `pref_filter_animelib_tracked_<id>_v2`, `pref_filter_library_tracked_<id>_v2` (id = tracker id Int).
* Update counters `library_unread_updates_count`, `library_unseen_updates_count` (Int 0): badge on the Updates tab.
* Categories: `default_category`, `default_anime_category` (Int, -1 = ask; 0 = default category; else category id), `__APP_STATE_last_used_anime_category`, `__APP_STATE_last_used_category` (Int 0).
* Swipe actions: `pref_episode_swipe_end_action` default ToggleSeen is read by `swipeEpisodeStartAction()`; `pref_episode_swipe_start_action` default ToggleBookmark is read by `swipeEpisodeEndAction()`; same crossing for chapters (`pref_chapter_swipe_end_action` default ToggleRead via `swipeChapterStartAction()`, `pref_chapter_swipe_start_action` default ToggleBookmark via `swipeChapterEndAction()`). Enum values: `EpisodeSwipeAction` {ToggleSeen, ToggleBookmark, ToggleFillermark, Download, Disabled}; `ChapterSwipeAction` {ToggleRead, ToggleBookmark, Download, Disabled}.
* Duplicates: `mark_duplicate_read_chapter_read` set of `"new"`/`"existing"`; `mark_duplicate_seen_episode_seen` set of `"new_episode"`/`"existing_episode"` (empty by default): when a chapter is marked read, same-numbered chapters (already existing, or newly added) are also marked read.

### 5.7 Season keys (`LibraryPreferences.kt:260-357`)

Defaults: filters `SHOW_ALL` (0); `default_season_sort_by_source_or_number` `SEASON_SORT_SOURCE` (0); `default_season_sort_by_ascending_or_descending` `SEASON_SORT_DESC` (0); `default_season_grid_display_mode` 0 (CompactGrid); `default_season_grid_display_size` 0; overlays `default_season_download_overlay` false, `default_season_unseen_overlay` true, `default_season_local_overlay` true, `default_season_lang_overlay` false, `default_season_continue_overlay` true; `default_season_display_mode` `SEASON_DISPLAY_MODE_SOURCE` (0). Behaviour switches: `pref_update_season_on_refresh` false, `pref_update_season_on_library_update` false.

### 5.8 DownloadPreferences (`DownloadPreferences.kt`)

* `pref_download_only_over_wifi_key` true; `use_external_downloader` false; `external_downloader_selection` "" (package name on Android).
* `save_chapter_as_cbz` true; `split_tall_images` true.
* `auto_download_while_reading` and `auto_download_while_watching` (Int 0 = off; UI 0, 2, 3, 5, 10 "next unread chapters/unseen episodes"): read once at ViewModel construction (`ReaderViewModel.kt:226`, `PlayerViewModel.kt:1112`), so changing it mid-session has no effect until the next open.
* `remove_after_read_slots` (-1): after finishing chapter at position p, delete downloaded chapter at index `p - slots` of the chapter list (`ReaderViewModel.kt:509-514`); -1 returns early; UI -1 disabled, 0 last read, 1 second to last, 2 third, 3 fourth, 4 fifth.
* `pref_remove_after_marked_as_read_key` false; `pref_remove_bookmarked` false (also delete bookmarked); `pref_download_fillermarked` false (download fillermarked episodes too).
* Category sets: `remove_exclude_categories`, `remove_exclude_anime_categories` (never auto-delete from these); `download_new_categories`, `download_new_anime_categories` (include list for download-new); `download_new_categories_exclude`, `download_new_anime_categories_exclude`.
* `download_new` false, `download_new_episode` false (auto-download newly fetched items); `download_new_unread_chapters_only` false, `download_new_unread_episodes_only` false (key spelled `unread` for episodes too).
* `download_slots` 1 (UI 1..5, info string `AYMR.strings.download_slots_info`); `download_speed_limit` 0 KiB/s (0 = off; `MangaDownloader.kt:525` uses `bytesPerSecond(limit.toLong() * 1024)`; label "$speedLimit KiB/s").
* `DownloadPreferences.categoryPreferenceKeys` = the six category-set keys; used by the restorer to remap category ids.

### 5.9 BackupPreferences and StoragePreferences

* `backup_interval` Int hours, default 12; UI 0 (off), 6, 12, 24, 48, 168. On change `BackupCreateJob.setupTask(context, it)`. Migration `EnableAutoBackupMigration` sets 0 to 12 once.
* `__APP_STATE_last_auto_backup_timestamp` Long 0 (shown in the info text `MR.strings.last_auto_backup_info`).
* `backup_flags` string set default `{"1","2","4","8"}` (FLAG_CATEGORIES, FLAG_CHAPTERS, FLAG_HISTORY, FLAG_TRACK). The full flag list in `PreferenceValues.kt` is FLAG_CATEGORIES "1", FLAG_CHAPTERS "2", FLAG_HISTORY "4", FLAG_TRACK "8", FLAG_SETTINGS "10", FLAG_EXT_SETTINGS "20", FLAG_EXTENSIONS "40" (decimal-looking strings of hexadecimal bit values; they are labels, not parsed). Never read by app code (dead); current backup options are `BackupOptions` booleans.
* `__APP_STATE_storage_dir` default `folderProvider.path()` = `Uri(<ExternalStorage>/Aniyomi)` string. Proposal: plain absolute path, default `Paths::data_dir()/downloads`'s parent (area:01-foundation-app-bootstrap); the directory contains `downloads/`, `autobackup/`, `local/`, `localanime/`.

### 5.10 SecurityPreferences and app lock (`SecurityPreferences.kt`, `SecureActivityDelegate.kt`)

* `use_biometric_lock` false; `lock_app_after` Int minutes: UI values `0` (Always), `1`, `2`, `5`, `10`, `-1` (Never); `secure_screen_v2` (`SecureScreenMode`: ALWAYS `MR.strings.lock_always`, INCOGNITO `MR.strings.pref_incognito_mode`, NEVER `MR.strings.lock_never`; default INCOGNITO); `hide_notification_content` false; `__APP_STATE_last_app_closed` Long 0.
* Lock algorithm (SecureActivityDelegate.kt:40-75):
  1. On leaving the app (`onApplicationStopped`), if the authenticator is enabled and the unlock UI is not currently authenticating: if `requireUnlock` is already true, return; else if `lock_app_after > 0`, write `last_app_closed = now_ms`.
  2. On entering foreground (`onApplicationStart`): if authenticator disabled return; if not authenticating and `requireUnlock` false, set `requireUnlock = when (lockDelay) { -1 -> false; 0 -> true; else -> last_app_closed + lockDelay * 60_000 <= now_ms }`; then delete `last_app_closed`.
  3. `unlock()` sets `requireUnlock = false`.
* Migration `MigrateSecureScreenMigration` converted the old boolean `secure_screen` to `SecureScreenMode.ALWAYS`.
* Proposal (area:36-ui-shell-navigation-security-webview): same state machine with focus-in/focus-out of the toplevel; unlock UI is a passphrase dialog verified against an argon2id hash stored in the keyring item `kind=applock`; PAM/polkit is NOT used. Secure screen is dropped.

### 5.11 NetworkPreferences

`verbose_logging` default = constructor argument (`isDebugBuildType`: true only for debug builds); `doh_provider` Int default -1 (provider ids 1..13, see data file); `default_user_agent` default `"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"`. The user agent is applied as the header of every outgoing request that does not set its own (area:12-network-stack). `verbose_logging` also drives mpv `msg-level=all=v` (else `all=warn`).

### 5.12 TorrentPreferences

`pref_torrserver_enable` false; `pref_torrserver_shownotice` false; `pref_torrserver_port` `"8090"` (String); `pref_torrserver_tackers` (sic, typo is the stored key) default = 26 tracker announce URLs (list in `p1-data-and-config.md` section 6), spaces removed with `.replace(" ", "")` while newlines are kept; `pref_torrserver_proxymode` (`ProxyMode` default `None`; wire value of `ProxyMode.None` is `"tracker"`, the same as `Tracker`); `pref_torrserver_proxyurl` "".

### 5.13 ReaderPreferences (`ReaderPreferences.kt`)

All keys and defaults are in the registry. Behavioural notes:

* Integer-coded preferences with their value tables: `pref_default_reading_mode_key` (ReadingMode.flagValue: 0 DEFAULT, 1 LEFT_TO_RIGHT, 2 RIGHT_TO_LEFT, 3 VERTICAL, 4 WEBTOON, 5 CONTINUOUS_VERTICAL; mask 0x7; default 2), `pref_default_orientation_type_key` (ReaderOrientation.flagValue: 0 DEFAULT, 0x08 FREE, 0x10 PORTRAIT, 0x18 LANDSCAPE, 0x20 LOCKED_PORTRAIT, 0x28 LOCKED_LANDSCAPE, 0x30 REVERSE_PORTRAIT; mask 0x38; default 0x08), `pref_image_scale_type_key` (UI index+1; labels `scale_type_fit_screen`, `scale_type_stretch`, `scale_type_fit_width`, `scale_type_fit_height`, `scale_type_original_size`, `scale_type_smart_fit`; default 1), `pref_zoom_start_key` (index+1; `zoom_start_automatic`, `zoom_start_left`, `zoom_start_right`, `zoom_start_center`; default 1), `pref_reader_theme_key` (0 white `white_background`, 1 black `black_background`, 2 gray `gray_background`, 3 automatic `automatic_background`; default 1), `reader_navigation_mode_pager` and `reader_navigation_mode_webtoon` (index into `TapZones`: 0 `label_default`, 1 `l_nav`, 2 `kindlish_nav`, 3 `edge_nav`, 4 `right_and_left_nav`, 5 `disabled_nav`; default 0), `color_filter_mode` (index into `ColorFilterMode`: 0 `label_default`=SrcOver, 1 `filter_mode_multiply`=Modulate, 2 `filter_mode_screen`=Screen, and only on Android API >= 28: 3 `filter_mode_overlay`=Overlay, 4 `filter_mode_lighten`=Lighten, 5 `filter_mode_darken`=Darken; default 0).
* Per-manga reading mode and orientation override these defaults through the manga's `viewer_flags` (area:07-domain-entries-items/43). `ReadingMode.fromPreference(x)` returns DEFAULT for unknown ints; `toViewer` throws `IllegalStateException("Preference value must be resolved: $preference")` for DEFAULT, so the app must resolve DEFAULT to the preference default before opening a viewer. `isPagerType(flag)`: type is Pager for LTR/RTL/VERTICAL.
* `ReaderHideThreshold` (`reader_hide_threshold`, default LOW): HIGHEST 5, HIGH 13, LOW 31, LOWEST 47 = scroll distance (px/dp) that hides the reader menu in webtoon mode.
* `webtoon_side_padding` 0..25 (`WEBTOON_PADDING_MIN`/`MAX`), percent of width per side.
* Flash: `pref_reader_flash` false, `pref_reader_flash_duration` default 100 ms (`MILLI_CONVERSION`), `pref_reader_flash_interval` 1 (flash every N pages), `pref_reader_flash_mode` (`FlashColor` BLACK default, WHITE, WHITE_BLACK).
* `pref_double_tap_anim_speed` 500 ms.
* Dual page: eight booleans (`pref_dual_page_split`, `pref_dual_page_invert`, `pref_dual_page_split_webtoon`, `pref_dual_page_invert_webtoon`, `pref_dual_page_rotate`, `pref_dual_page_rotate_invert`, `pref_dual_page_rotate_webtoon`, `pref_dual_page_rotate_invert_webtoon`), all false.
* Colour filter: `pref_custom_brightness_key` false, `custom_brightness_value` -75..100 (negative = black overlay with alpha, 0 = none, positive = set brightness; on desktop positive/negative both become an overlay or the monitor is untouched, see UI notes in area:43-reader-core), `pref_color_filter_key` false, `color_filter_value` ARGB int (UI sliders 0..255 each for R, G, B, A), `pref_grayscale` false, `pref_inverted_colors` false.
* Tapping inversion: `reader_tapping_inverted`, `reader_tapping_inverted_webtoon` (`TappingInvertMode` NONE, HORIZONTAL, VERTICAL, BOTH with titles `tapping_inverted_none`, `_horizontal`, `_vertical`, `_both`; flags `shouldInvertHorizontal`, `shouldInvertVertical`).
* Overlay hints: `reader_navigation_overlay_new_user` true (shown once; cleared after first show), `reader_navigation_overlay_on_start` false.
* Others: `pref_enable_transitions_key` true (page transition animation), `pref_show_page_number_key` true, `pref_show_reading_mode` true (toast of mode on open), `fullscreen` true, `cutout_short` true, `pref_keep_screen_on_key` true, `pref_enable_double_tap_zoom_webtoon` true, `always_show_chapter_transition` true, `pref_preserve_reading_position` false, `crop_borders` false, `crop_borders_webtoon` false, `navigate_pan` true, `landscape_zoom` true, `create_folder_per_manga` false (screenshot/save-image folder), `skip_read` false, `skip_filtered` true, `skip_dupe` false (next/previous chapter skipping rules), `webtoon_disable_zoom_out` false, `reader_long_tap` true, `reader_volume_keys` false, `reader_volume_keys_inverted` false.

### 5.14 Player preferences (`PlayerPreferences.kt`, `GesturePreferences.kt`, `DecoderPreferences.kt`, `AudioPreferences.kt`, `SubtitlePreferences.kt`, `AdvancedPlayerPreferences.kt`)

How each becomes an mpv option is OBSERVED in `AniyomiMPVView.kt:initOptions` (lines 118-170) and `setupAudioOptions`/`setupSubtitlesOptions`. The full mapping, with the Linux libmpv call, is `p1-topic-player-mpv-mapping.md`. Key behaviours:

* `pref_progress_preference` float 0.85: an episode is marked seen when `position / duration >= 0.85` (consumed by area:45-player-core).
* `pref_default_intro_length` 85 s: manual skip-intro jump; `pref_skip_length_preference` 10 s: seek button/gesture step.
* `pref_player_speed` 1.0 and `pref_player_aspect_state` Fit are the persisted last values; `default_speed_presets` is a string set of decimal strings (sorted numerically when displayed).
* Skip intro (AniSkip): `pref_enable_skip_intro` true (show button), `pref_enable_auto_skip_ani_skip` false (auto skip), `pref_enable_netflixStyle_aniskip` false, `pref_waiting_time_aniskip` 5 s, `pref_enable_ani_skip` false (fetch AniSkip timestamps), `pref_disabled_ani_skip_chapters` true (do not use AniSkip when the file has chapters).
* `pref_player_time_to_disappear` 4000 ms control auto-hide; `pref_panel_opacity` 60 percent; `pref_show_loading` true; `pref_show_current_chapter` true; `pref_display_vol_as_per` true (volume shown as percent vs raw); `pref_reduce_motion` false; `invert_duration` false (show remaining time); `player_fullscreen` true; `player_hide_controls` false.
* `pref_remember_brightness` false with `player_brightness_value` -1.0 and `pref_remember_volume` false with `player_volume_value` -1.0 (-1 = unset).
* Hosters: `pref_show_failed_hosters` false, `pref_show_empty_hosters` false (video hoster list filters).
* `pref_auto_play_enabled` false ("Old" section, still read for autoplay next).
* Gestures: `pref_gesture_volume_brightness` true, `pref_swap_volume_and_brightness` false, `pref_gesture_horizontal_seek` true, `pref_show_seekbar` false, `pref_player_smooth_seek` false; double-tap `SingleActionGesture` {None, Seek, PlayPause, Switch, Custom}: left Seek, center PlayPause, right Seek; media controls: previous Switch, play/pause PlayPause, next Switch. `Custom` sends the mpv key codes `0x10001..0x10006` (`CustomKeyCodes` DoubleTapLeft, DoubleTapCenter, DoubleTapRight, MediaPrevious, MediaPlay, MediaNext).
* Decoder: `pref_try_hwdec` true -> `hwdec=auto` else `no`; `pref_gpu_next` false -> vo `gpu-next` else `gpu`; `pref_video_debanding` (`Debanding` None, CPU, GPU): CPU sets `vf=gradfun=radius=12`, GPU sets `deband=yes`; `use_yuv420p` true sets `vf=format=yuv420p` (NOTE: this second `setOptionString("vf", ...)` overwrites the CPU-debanding filter, so CPU debanding is lost whenever YUV420P is on; see quirk Q9). Filter sliders (`pref_player_filter_brightness`, `_saturation`, `_contrast`, `_gamma`, `_hue`) are Int default 0, applied as mpv options `brightness`, `saturation`, `contrast`, `gamma`, `hue` (range -100..100 in mpv).
* Audio: `pref_audio_lang` "" (mpv `alang`, comma list), `pref_audio_pitch_correction` true (`audio-pitch-correction`), `pref_audio_config` (`AudioChannels`; applied in `PlayerActivity.kt:586` as `MPVLib.setPropertyString(it.property, it.value)`), `pref_audio_volume_boost_cap` 30 (`volume-max = cap + 100`), `pref_audio_delay` Int ms (mpv `audio-delay` seconds = ms / 1000.0).
* Subtitles: all values in the registry; colours converted with `toColorHexString()` to `#AARRGGBB`; `pref_sub_justify` (`SubtitleJustification` value strings `left`, `center`, `right`, `auto`); `pref_border_style_subtitles` (`SubtitlesBorderStyle` values `outline-and-shadow`, `opaque-box`, `background-box`); `pref_override_subtitles_ass` true sets `sub-ass-override=force` and `sub-ass-justify=yes`; delays in ms divided by 1000.0 for `sub-delay`, `secondary-sub-delay`; `sub-speed`; `sub-pos` default 100; `sub-scale` default 1.0; `sub-font-size` default 55; `sub-border-size` 3; `sub-shadow-offset` 0; font name default `"Sans Serif"`.
* Advanced: `mpv_scripts` false (load user scripts/fonts/shaders folders), `pref_mpv_conf` and `pref_mpv_input` free text (contents of mpv.conf and input.conf), `pref_player_statistics_page` Int 0 (0 off; 1..N toggles `stats/display-page-N` after init, `AniyomiMPVView.kt:postInitOptions`).

## 6. Per-source preference files (OBSERVED, source-api `Preferences.kt`)

`preferencesKey(id: Long) = "source_$id"`. `sourcePreferences(key)` = `Application.getSharedPreferences(key, MODE_PRIVATE)` (one XML file per source under `shared_prefs/`). Extensions that implement `ConfigurableSource`/`ConfigurableAnimeSource` call these to store e.g. mirror URL, credentials. The settings screen (`MangaSourcePreferencesScreen.kt:140`) wraps the same SharedPreferences with `SharedPreferencesDataStore` so AndroidX preference widgets (`EditTextPreference`, `ListPreference`, `SwitchPreferenceCompat`, `MultiSelectListPreference`) read/write them; text fields apply an incognito IME flag.

Proposal: `SourcePrefs` (typed map with the same six value kinds) persisted as `$XDG_CONFIG_HOME/kurobayomi/sources/source_<id>.toml`, each value stored as `{ t = "int"|"long"|"float"|"string"|"bool"|"set", v = ... }` so the Android type is not lost (TOML integers do not distinguish Int and Long). Credentials-like keys are not auto-detected; the user can include or exclude source settings in a backup via the "Private settings" option exactly as in Aniyomi (private keys use the same `__PRIVATE_` prefix inside a source file).

## 7. Backup and restore of preferences (OBSERVED)

See `p1-topic-backup-interop.md` for the full algorithm and proto layouts. Summary: backup writes every entry of `preferenceStore.getAll()` whose value is Int, Long, Float, String, Boolean or Set<String> and whose key is not app-state, dropping private keys unless requested; per-source preferences are written per `ConfigurableSource` that has at least one pref. Restore writes a value only when the existing stored value has the same primitive type (or the key does not exist); `default_category` and `default_anime_category` are remapped by category name; the six category-set keys of `LibraryPreferences.categoryPreferenceKeys` and `DownloadPreferences.categoryPreferenceKeys` are remapped by name and merged with `+=`; after restoring app prefs the three scheduled jobs are re-scheduled.

## 8. Concurrency and timing

* Kotlin: `SharedPreferences` reads are in-memory after first load and thread-safe; writes `apply()` to disk asynchronously; a change event is delivered on the main thread. Flows are conflated, so intermediate values can be dropped.
* Proposal: `PrefStore` is `Arc<RwLock<Inner>>`; `set` updates memory under the write lock, queues a flush, and sends a `KeyChanged(key)` event on a `tokio::sync::broadcast::Sender` (capacity 256). A single background task (tokio) coalesces flushes: wait 250 ms after the first dirty mark, then serialise to `config.toml.tmp`, fsync, rename over `config.toml` (and likewise `state.toml`); a forced flush happens on app quit and before a backup is created. GTK widgets subscribe via `glib::MainContext::channel`-style forwarding (`async-channel` receiver polled with `glib::spawn_future_local`), so handlers always run on the GTK main thread. Burst writes (a slider drag) are therefore one disk write per 250 ms.
* Network/battery constraints of scheduled jobs (`library_update_restriction`, `pref_download_only_over_wifi_key`) map to GIO `NetworkMonitor` (`connectivity`, `network-metered` properties) and `UPower` `OnBattery`/`BatteryLevel` over D-Bus (zbus) inside `kuro-platform` (area:26-library-update-jobs owns scheduling). `wifi` is treated as "not metered and not `NetworkMonitor::connectivity == Limited`" because Linux desktops usually do not expose the transport type; documented loss: a metered Wi-Fi is treated as non-Wi-Fi, an unmetered Ethernet is treated as Wi-Fi.

## 9. Interactions with other areas

| Area | Interaction |
|---|---|
| area:01-foundation-app-bootstrap | owns `Paths` (config/state/data dirs), the app context where `Prefs` lives, and startup order (load prefs before building the window) |
| area:11-app-migrations | runs one-time key rewrites; needs `PrefStore` raw access (`get_raw`, `set_raw`, `remove`) and the `last_version_code` app-state key |
| area:12-network-stack | reads `default_user_agent`, `doh_provider`, `verbose_logging` |
| area:21-backup-format-and-create/22 backup | uses `PrefStore::export_all(include_private)` and `import(entries, remap)` |
| area:26-library-update-jobs | reads interval, device and item restrictions, category include/exclude, `auto_update_metadata` |
| area:27-tracking-core trackers | credentials via `SecretStore`, switches in `Settings.tracking` |
| area:36-ui-shell-navigation-security-webview, area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime, area:42-ui-updates-history-stats-storage-upcoming | UI pages bind widgets to prefs; `changes()` streams drive live updates |
| area:43-reader-core/44 reader, area:45-player-core/46/48 player | read their sections; reader and player apply value tables above |
| area:47-settings-screens-general settings screens | render the settings pages from `spec/*.json` bound by `kuro_prefs::bind` |
| area:20-extension-strategy-redesign extensions | per-source preference screens through `SourcePrefs` |
