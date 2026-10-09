# Area 41 (part 1): data models, preferences, storage, endpoints, DB touchpoints

All paths repo-relative to `/home/blackbox/aniyomi-src`. Numbered sections below are referenced from p1-behavior.md and p1-tasks.md.

## 1. Files, folders and formats on disk (touched by this part)

| Item | Kotlin owner | Format | Path (Android) | Kurobayomi path (Proposal) | Notes |
|---|---|---|---|---|---|
| Installed extension APK | `AnimeExtensionManager` / installer (area:19) | `.apk` (Android package) | Android PackageManager | `$XDG_DATA_HOME/kurobayomi/extensions/anime/<pkgName>/` (bundle dir; area:20 defines the format) | The browse UI only needs: list, icon, open dir, uninstall |
| Extension icon | `BrowseAnimeIcons.kt` `getIcon` | PNG/resource from package resources (`appInfo.icon`) | read via PackageManager | `$XDG_CACHE_HOME/kurobayomi/ext-icons/<pkgName>-<density>.png` | Proposal: one size 96 px; decode with `image` |
| Available extension icon | `AnimeExtension.Available.iconUrl` (Coil) | remote image | HTTP | `$XDG_CACHE_HOME/kurobayomi/ext-icons/remote/<sha256(url)>.<ext>` | area:34 cache policy |
| Custom cover (migrated) | `AnimeCoverCache.getCustomCoverFile(id)` / `setCustomCoverToCache` (area:34) | image file | app files dir | `$XDG_DATA_HOME/kurobayomi/covers/custom/<animeId>.jpg` (area:34) | Copied during migrate (behavior 12 step 7) |
| Custom background | `AnimeBackgroundCache` (area:34) | image file | app files dir | `$XDG_DATA_HOME/kurobayomi/backgrounds/custom/<animeId>` (area:34) | Copied during migrate (behavior 12 step 8) |
| Trusted extensions list | `TrustAnimeExtension` (area:19), key `trusted_extensions` (app state) | string set `pkg:versionCode:signatureHash` (area:19) | PreferenceStore | Kurobayomi prefs file, section 2 | Read only here |
| Cookie jar | `NetworkHelper.cookieJar` (area:12) | OkHttp cookie store | app storage | `$XDG_DATA_HOME/kurobayomi/cookies.json` (area:12) | `clearCookies` removes by URL |
| Extension store index cache | `AnimeExtensionStoreRepository` (area:19) | DB table `extension_store` (section 7) | SQLite | rusqlite table | Read by extension stores screen |
| Clipboard | Android ClipboardManager | text | n/a | GDK clipboard | No file |

Format rule for everything under Kurobayomi's data dir: JSON for small state, SQLite (rusqlite) for records, PNG/JPEG for images. The browse UI itself writes no files except via the cover/icon caches and the migrate copies.

## 2. Preference keys used in this part

Keys are the EXACT strings in the Kotlin. "Scope" = where the key is read in this part. Type/default come from the preference getter.

### 2.1 `SourcePreferences` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt`)

| Kotlin getter | Key | Type | Default | Meaning | Read/written by (this part) |
|---|---|---|---|---|---|
| `sourceDisplayMode()` | `pref_display_mode_catalogue` | object (LibraryDisplayMode serialized string) | `LibraryDisplayMode.default` = `CompactGrid` | browse grid mode (shared manga/anime) | browse toolbar, migrate season |
| `enabledLanguages()` | `source_languages` | string set | `setOf("all", "en", Locale.getDefault().language)` | visible languages in source list, extension list, search | extension list/filter, source filter, global search (model) |
| `showNsfwSource()` | `show_nsfw_source` | bool | `true` | include NSFW extensions and sources | GetAnimeExtensionsByType |
| `migrationSortingMode()` | `pref_migration_sorting` | enum name | `SetMigrateSorting.Mode.ALPHABETICAL` | migrate source picker sort mode | migrate source picker |
| `migrationSortingDirection()` | `pref_migration_direction` | enum name | `SetMigrateSorting.Direction.ASCENDING` | migrate source picker direction | migrate source picker |
| `animeExtensionRepos()` | `anime_extension_repos` | string set | `emptySet()` | repo URLs (area:19) | not read here (stores screen) |
| `trustedExtensions()` | `Preference.appStateKey("trusted_extensions")` | string set | `emptySet()` | trust records (area:19) | trust action |
| `globalSearchFilterState()` | `Preference.appStateKey("has_filters_toggle_state")` | bool | `false` | "Has results" chip state | global search + migrate search |
| `disabledAnimeSources()` | `hidden_anime_catalogues` | string set of source ids (decimal) | `emptySet()` | disabled sources | source toggles, details, global search model |
| `incognitoAnimeExtensions()` | `incognito_anime_extensions` | string set of pkgNames | `emptySet()` | per-extension incognito | details incognito switch |
| `pinnedAnimeSources()` | `pinned_anime_catalogues` | string set of source ids | `emptySet()` | pinned sources | source list pin, global search order, migrate search order |
| `lastUsedAnimeSource()` | `Preference.appStateKey("last_anime_catalogue_source")` | long | `-1` | last browsed anime source (part 2 uses; not read here) | not in this part |
| `animeExtensionUpdatesCount()` | `animeext_updates_count` | int | `0` | updates badge | extension tab badge |
| `hideInAnimeLibraryItems()` | `browse_hide_in_anime_library_items` | bool | `false` | hide favorites in browse lists (season migrate) | MigrateSeasonSelectScreenModel |

Manga-side keys the anime UI does NOT read: `pinned_catalogues`, `hidden_catalogues`, `incognito_manga_extensions`, `ext_updates_count`, `browse_hide_in_library_items`, `extension_repos`.

### 2.2 `LibraryPreferences` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt`)

| Getter | Key | Default | Used by |
|---|---|---|---|
| `animePortraitColumns()` | `pref_animelib_columns_portrait_key` | `0` (0 = adaptive) | MigrateSeasonSelectScreenModel.getColumnsPreference |
| `animeLandscapeColumns()` | `pref_animelib_columns_landscape_key` | `0` | same |

### 2.3 `BasePreferences` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt`)

- `extensionInstaller()`: `ExtensionInstallerPreference(context, preferenceStore)` (area:19 owns the key). Enum values in section 4.

### 2.4 `PreferenceStore` raw keys (used directly by a `Migrate*Dialog` model)

| Key | Type | Default | Owner | Used by | Notes |
|---|---|---|---|---|---|
| `migrate_flags` | int | `Int.MAX_VALUE` (2147483647) | `MigrateAnimeDialog.kt:216` | anime migrate dialog | SHARED with manga: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt:166` uses the same string. A value saved by one dialog is read by the other (bug, behavior E9; T-41a-033). |

### 2.5 Proposal: Kurobayomi keys

Keep Kotlin key strings for import/backup parity (area:21/22). Store in the Rust prefs (area:02). Typed accessors in `kurobayomi::prefs::browse` (p1-rust-design.md section 5). Proposal for anime-specific migrate flags: key `anime_migrate_flags` (int, default `0x7FFFFFFF`), keep `migrate_flags` read-only for one release to import the old value.

## 3. Domain models (fields, types, nullability, defaults)

### 3.1 `AnimeExtension` (`domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt`)

`sealed class AnimeExtension` with abstract `name: String`, `pkgName: String`, `versionName: String`, `versionCode: Long`, `libVersion: Double`, `lang: String?`, `isNsfw: Boolean`, `isTorrent: Boolean`.

| Variant | Field | Type | Nullable | Default |
|---|---|---|---|---|
| Installed | name, pkgName, versionName | String | no | - |
| | versionCode | Long | no | - |
| | libVersion | Double | no | - |
| | lang | String | no (overrides `String?`) | - |
| | isNsfw, isTorrent | Boolean | no | - |
| | pkgFactory | String? | yes | - |
| | sources | List<AnimeSource> (`eu.kanade.tachiyomi.animesource.AnimeSource`) | no | - |
| | icon | Drawable? | yes | - |
| | hasUpdate | Boolean | no | `false` |
| | isObsolete | Boolean | no | `false` |
| | isShared | Boolean | no | - (no default) |
| | store | AnimeExtensionStore? | yes | `null` |
| Available | (same base fields) | | | |
| | sources | List<Available.AnimeSource> | no | - |
| | apkUrl | String | no | - |
| | iconUrl | String | no | - |
| | store | AnimeExtensionStore | no | - |
| | nested `AnimeSource` | id: Long, lang: String, name: String, baseUrl: String | no | - (method `toStubSource(): StubAnimeSource`) |
| Untrusted | (base fields; lang nullable; isNsfw false; isTorrent false defaults) | | | |
| | signatureHash | String | no | - |

Note: `Available` always has `lang: String`, but the base is `String?`. In the screen model, the per-source split copies `lang = it.lang` from the source.

### 3.2 `AnimeExtensionSourceItem` (`app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt`)

`data class AnimeExtensionSourceItem(source: AnimeSource, enabled: Boolean, labelAsName: Boolean)`. All non-null.

### 3.3 `AnimeExtensionUiModel` (in `AnimeExtensionsScreenModel.kt`)

- `Header.Resource(textRes: StringResource)`, `Header.Text(text: String)`.
- `Item(extension: AnimeExtension, installStep: InstallStep)`.
- `typealias ItemGroups = MutableMap<Header, List<Item>>`.

### 3.4 `AnimeExtensions` (returned by GetAnimeExtensionsByType)

4-tuple: `updates: List<Installed>`, `installed: List<Installed>`, `available: List<Available>`, `untrusted: List<Untrusted>` (order used by destructuring is `(_updates, _installed, _available, _untrusted)`).

### 3.5 `AnimeExtensionFilterState` (AnimeExtensionFilterScreenModel.kt)

- `Loading` (object).
- `Success(languages: ImmutableList<String>, enabledLanguages: ImmutableSet<String> = persistentSetOf())`, `isEmpty = languages.isEmpty()`.

### 3.6 `AnimeExtensionDetailsScreenModel.State`

`extension: AnimeExtension.Installed? = null`, `isIncognito: Boolean = false`, `_sources: ImmutableList<AnimeExtensionSourceItem>? = null` (private). Derived: `sources = _sources ?: []`, `isLoading = extension == null || _sources == null`.

### 3.7 `AnimeExtensionsScreenModel.State`

`isLoading: Boolean = true`, `isRefreshing: Boolean = false`, `items: ItemGroups = mutableMapOf()`, `updates: Int = 0`, `installer: BasePreferences.ExtensionInstaller? = null`, `searchQuery: String? = null`. Derived `isEmpty = items.isEmpty()`.

### 3.8 `AnimeSource` (domain, `domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt`)

Interface used by this part: `id: Long`, `name: String`, `lang: String`, `isStub: Boolean`, `supportsLatest: Boolean`, `pin: Pins` (`Pin` flags), `visualName: String` (derived: `name` if `lang` empty, else `"$name (${lang.uppercase()})"`), `icon: ImageBitmap?` (via `eu.kanade.domain.source.anime.model.icon`).

### 3.9 `StubAnimeSource` (`domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt`)

`class StubAnimeSource(id: Long, lang: String, name: String)`, `isStub = true`, `isInvalid = name.isBlank() || lang.isBlank()` (private, default expression), `toString() = if (!isInvalid) "$name (${lang.uppercase()})" else id.toString()`.

### 3.10 `LocalAnimeSource` (`source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt`, Android actual)

`companion object { const val ID = 0L; const val HELP_URL = "https://aniyomi.org/help/guides/local-anime/" }`.

### 3.11 `Anime` (domain, `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`) fields used here

`id: Long`, `source: Long`, `url: String`, `title: String`, `favorite: Boolean`, `thumbnailUrl: String?`, `coverLastModified: Long`, `fetchType: FetchType` (line 42), `episodeFlags: Long`, `viewerFlags: Long`, `dateAdded: Long`, `parentId`, `seasonSourceOrder`. Field list is authoritative from area:07; only the above are touched.

`AnimeCover(animeId: Long, sourceId: Long, isAnimeFavorite: Boolean, url: String?, lastModified: Long)` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeCover.kt`); `Anime.asAnimeCover()` maps `id, source, favorite, thumbnailUrl, coverLastModified`.

### 3.12 `AnimeUpdate` (migration final step)

`AnimeUpdate(id: Long, favorite: Boolean? = null, episodeFlags: Long? = null, viewerFlags: Long? = null, dateAdded: Long? = null, ...)`. Only the four listed are set by this part; others stay null (= no change). Exact nullability is area:07's; the migration passes `favorite = true`, `episodeFlags = oldAnime.episodeFlags`, `viewerFlags = oldAnime.viewerFlags`, `dateAdded`.

### 3.13 `Episode` fields used by migration (area:07, `domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt`)

`id`, `animeId`, `seen: Boolean`, `bookmark: Boolean`, `fillermark: Boolean`, `lastSecondSeen: Long`, `episodeNumber: Double`, `sourceOrder: Long`, `dateFetch: Long`, `isRecognizedNumber: Boolean` (derived), `toEpisodeUpdate()`.

Migration copies: `dateFetch`, `bookmark` (from matching prev episode), `seen` (derived). Does NOT copy `lastSecondSeen`, `fillermark` (E20).

### 3.14 `AnimeTrack` (area:27)

`copy(animeId = newAnime.id)` plus enhanced-tracker `migrateTrack` result (`remoteUrl` replaced with the new anime's url when accepted).

### 3.15 `StubAnimeSource` and `AnimeSourceFilter` (part 2 types consumed here)

`enum class AnimeSourceFilter { All, PinnedOnly }` (AnimeSearchScreenModel.kt:212). `sealed interface AnimeSearchItemResult { Loading; Error(throwable); Success(result: List<Anime>) { isEmpty } }`.

## 4. Enums and serialized values

| Enum | Values (declaration order) | Persisted form | File |
|---|---|---|---|
| `InstallStep` | `Idle, Pending, Downloading, Installing, Installed, Error` | in-memory only | `app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt` |
| `BasePreferences.ExtensionInstaller(titleRes, requiresSystemPermission)` | `LEGACY(ext_installer_legacy, true)`, `PACKAGEINSTALLER(ext_installer_packageinstaller, true)`, `SHIZUKU(ext_installer_shizuku, false)`, `PRIVATE(ext_installer_private, false)` | preference enum name (`getEnum`) | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31-36` |
| `SetMigrateSorting.Mode` | `ALPHABETICAL, TOTAL` | enum name in `pref_migration_sorting` | `app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt` |
| `SetMigrateSorting.Direction` | `ASCENDING, DESCENDING` | enum name in `pref_migration_direction` | same |
| `LibraryDisplayMode` | `CompactGrid, ComfortableGrid, List, CoverOnlyGrid` (default `CompactGrid`) | `COMPACT_GRID`, `COMFORTABLE_GRID`, `LIST`, `COVER_ONLY_GRID` (unknown -> default) | `domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt` |
| `FetchType` | `Seasons (ordinal 0), Episodes (ordinal 1)` | DB column `animes.fetch_type INTEGER AS FetchType NOT NULL DEFAULT 1` (ordinal) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt` and `data/src/main/sqldelightanime/dataanime/animes.sq:33` |
| `Pin` (flags) | `Unpinned = 0b00`, `Pinned = 0b01`, `Actual = 0b10` (`code: Int`) | bit field in source `pin` | `domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt` |
| `AnimeSourceFilter` | `All, PinnedOnly` | in-memory | part 2 |
| `LocaleHelper` special lang keys | `"all"`, `"other"`, `"last_used"` (`LAST_USED_KEY`), `"pinned"` (`PINNED_KEY`) | strings | `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:84-85` |

`AnimeExtensionUiModel.Header.Resource` identity is the `StringResource` (moko `MR.strings.ext_updates_pending`, `MR.strings.ext_installed`), so the header key is stable across emissions.

## 5. Bit-flag layouts

### 5.1 Migration option bitmap (`AnimeMigrationFlags`)

```
bit0 0x01  EPISODES           (only when fetchType == Episodes)
bit1 0x02  CATEGORIES
bit2 0x04  CUSTOM_BACKGROUND  (only when a custom background file exists)
bit3 0x08  CUSTOM_COVER       (only when a custom cover file exists)
bit4 0x10  DELETE_DOWNLOADED  (only when downloadCount > 0)
bit5..31   unused (preserved in migrate_flags, never read)
```

Default `migrate_flags` = `0x7FFFFFFF` (all 31 low bits set). Stored value is the full bitmap of the LAST confirmed selection, i.e. `getSelectedFlagsBitMap` output, so a previously unselected flag persists as 0 and is not defaulted back on.

Subtle: because the saved bitmap is a bitmap of SELECTED flags for one anime and is applied to every anime, a flag absent from the current anime (for example CUSTOM_COVER on an anime without a cover) is simply not shown and does not contribute.

### 5.2 Pin bits

`Pin.Pinned = 0b01` (bit 0), `Pin.Actual = 0b10` (bit 1). `Pin.Pinned in source.pin` means bit 0 set.

### 5.3 Episode and anime flags copied (not decoded here)

`Anime.episodeFlags` and `Anime.viewerFlags` are copied verbatim by migrate (behavior 12 step 9). Their bit layouts belong to area:07 / area:39.

## 6. Network endpoints (only what this part triggers)

| Name | Trigger | Method | URL | Notes |
|---|---|---|---|---|
| Extension index (all stores) | extension list `init` and refresh | GET | Owned by `AnimeExtensionStoreRepository.fetchExtensions()` (area:19). Store index URLs come from `extension_store.index_url` (section 7). | Result becomes `availableExtensionsFlow`. Errors -> toast `extension_api_error` ("Failed to fetch available extensions"). |
| Available extension icon | `AsyncImage(model = extension.iconUrl)` | GET | `Available.iconUrl` | Placeholder color `#1F888888`; error drawable `cover_error` |
| Available extension APK | install | GET | `Available.apkUrl` | Downloaded by installer (area:19) |
| Source search (global, migrate) | search model | source-defined | `AnimeSource.getSearchAnime(1, query, filters)` (area:16) | Page 1 only |
| Source home (WebView) | "Open in WebView", empty-state WebView action | GET (WebView) | `AnimeHttpSource.getHomeUrl()` (area:16) and `baseUrl` for Available | `WebViewScreen(url, initialTitle, sourceId)` |
| Help | browse toolbar overflow Help, empty state Help | external | `Constants.URL_HELP` = `https://aniyomi.org/docs/guides/troubleshooting/` | Opens in external browser |
| Local source help | local source empty state and toolbar | external | `LocalAnimeSource.HELP_URL` = `https://aniyomi.org/help/guides/local-anime/` | |
| Repo link | details "Open source repo" | external | `https://github.com/<user>/<repo>` derived from `https://raw.githubusercontent.com/(.+?)/(.+?)/.+` | fallback to raw `indexUrl` |
| Season list | migrate seasons | source-defined | `updateAnimeFromRemote.awaitSeasonsUpdate(anime, fetchSeasons = true)` (area:16/area:10) | `newSeasons: List<SAnime>` |
| Episode list (migrate) | migrate step 1 | source-defined | `updateAnimeFromRemote.awaitEpisodesUpdate(source, anime, fetchEpisodes = true)` | Throws on failure |

Auth: none in this part. Cookies: `NetworkHelper.cookieJar` (area:12) is used only for clear-cookies. Headers: none set here (source-defined headers are area:16/area:12).

Last extension check throttle (for reference, not read here): `last_ext_check` (long, default 0, one day) in `AnimeExtensionApi` (`app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt`, declaration near lines 20-27, `checkForUpdates` lines 34-44). The UI refresh (`findAvailableExtensions`) calls `findExtensions()` directly and does NOT apply the throttle (OBSERVED: the throttle is applied only when `fromAvailableExtensionList = true` in `checkForUpdates`).

## 7. DB tables and columns touched

| Table (file) | Columns | Operation in this part |
|---|---|---|
| `animes` (`data/src/main/sqldelightanime/dataanime/animes.sq`) | `_id`, `source`, `url`, `title`, `favorite`, `viewer`, `episode_flags`, `date_added`, `favorite_modified_at`, `fetch_type` (DEFAULT 1), `season_source_order`, `parent_id` | Migrate step 9 `UpdateAnime.await`; step 6 `awaitUpdateFavorite(false)` sets `date_added = 0`; season list rows inserted by `networkToLocalAnime`; global-search result DB state read via `getAnime.subscribe` |
| `episodes` (`episodes.sq`) | `_id`, `anime_id`, `seen` (Boolean), `bookmark` (Boolean), `last_second_seen`, `episode_number REAL`, `source_order`, `date_fetch`, `fillermark` | Migrate step 2: read (`getEpisodesByAnimeId`) for both anime; write (`updateEpisode.awaitAll`) of `date_fetch`, `bookmark`, `seen` for the new anime; `last_second_seen` and `fillermark` NOT written (E20) |
| `animes_categories` (`animes_categories.sq`) | `_id`, `anime_id`, `category_id` (FK categories, cascade) | Migrate step 3: `getCategories.await(oldId)`, `setAnimeCategories.await(newId, ids)` replaces |
| `anime_sync` (`anime_sync.sq`) | `_id`, `anime_id`, `sync_id`, `remote_id`, `library_id`, `title`, `last_episode_seen REAL`, `total_episodes`, `status`, `score REAL`, `remote_url`, `start_date`, ... | Migrate step 4: `getTracks.await(oldId)`, `insertTrack.awaitAll(migrated)` with `anime_id = newId` (the new row, not an update) |
| `animesources` (`animesources.sq`) | `_id`, `lang`, `name` | Not written by this part; the source name for the ID map is read via SourceManager |
| `extension_store` (`extension_store.sq`) | `index_url` (PK), `name`, `badge_label`, `signing_key`, `contact_website`, `contact_discord`, `is_legacy`, `extension_list_url` | Read indirectly through `AnimeExtension.store` (stores screen out of scope) |
| `animehistory` (`animehistory.sq`) | — | Not touched here |
| downloads | (filesystem; `AnimeDownloadCache`) | `getDownloadCount(anime)` (flag visibility) and `deleteAnime(oldAnime, oldSource)` (step 5) |

Views: `animelibView`, `animeseasonsView` (`data/src/main/sqldelightanime/view/`) are read by the library/entries code, not by this part.

Note on migration atomicity: Kotlin runs each interactor in its own transaction (area:05). The migration is NOT a single transaction across steps (behavior 12). Proposal: wrap steps 2-9 in one `rusqlite` transaction per anime pair, with the network fetch (step 1) BEFORE the transaction, so a fetch failure rolls back nothing and leaves the DB untouched (T-41a-034).

## 8. Migration copy/creation semantics per DB field (summary for implementers)

| Field of NEW anime | Written when | Value |
|---|---|---|
| `favorite` | always (step 9) | `true` |
| `episode_flags` | always (step 9) | old `episodeFlags` |
| `viewer` | always (step 9) | old `viewerFlags` |
| `date_added` | always (step 9) | `replace ? old.dateAdded : now()` |
| episodes `seen`/`bookmark`/`date_fetch` | flag EPISODES | see behavior 12 step 2 |
| categories | flag CATEGORIES | replaced with old's categories |
| tracks | always (no flag) | copied with new `anime_id`, `remote_url` replaced for enhanced trackers, dropped if rejected |
| custom cover file | flag CUSTOM_COVER and old has one | byte copy |
| custom background file | flag CUSTOM_BACKGROUND and old has one | byte copy |

| Field of OLD anime | Written when | Value |
|---|---|---|
| `favorite` | `replace` only (step 6) | `false`, `date_added = 0` |
| downloaded files | flag DELETE_DOWNLOADED and oldSource != null | deleted |

## 9. Persisted migrate state

`migrate_flags` is written at the START of `migrateAnime` (before any step), so even a failed migration remembers the flags the user chose. Manga and anime share the key (section 2.4), so picking flags in one dialog changes the default selection in the other.

## 10. Extension install and trust state (read-side only)

- `installedExtensionsFlow` (Map pkgName -> Installed), `availableExtensionsFlow` (Map pkgName -> Available), `untrustedExtensionsFlow` (Map pkgName -> Untrusted). All three are `StateFlow<Map<String, T>>` mapped to `Flow<List<T>>` via `mapExtensions(scope)` (`AnimeExtensionManager.kt:68-75, 392`).
- `updatePendingUpdatesCount` writes `animeext_updates_count` = count of Installed with `hasUpdate` (`AnimeExtensionManager.kt:382-388`).
- `installer.downloadAndInstall(apkUrl, extension)` / `cancelInstall(pkgName)` / `uninstallApk(pkgName)`: owned by area:19.

## 11. Cover and icon sizes (for caches)

| Use | Size | Source |
|---|---|---|
| Extension list icon | 40 dp box (icon fills with 0 or 8 dp padding) | AnimeExtensionsScreen.kt:298-320 |
| Extension details icon | 112 dp, density XXXHIGH (~640 dpi) | AnimeExtensionDetailsScreen.kt:267-271 |
| Source row icon | `height(40.dp).aspectRatio(1f)` = 40x40 dp | BrowseAnimeIcons.kt:38-40 |
| Browse comfortable grid cover | from `EntryComfortableGridItem` (area:37/39) | |
| Global search card | 96 dp wide | GlobalAnimeSearchCardRow.kt:64 |
| Stub warning icon | `Icons.Filled.Warning` tinted `error` | BrowseAnimeIcons.kt:49-57 |
| Untrusted icon | `Icons.Filled.Dangerous` tinted `error`, 40x40 | BrowseAnimeIcons.kt:115-120 |

## 12. Pins (`pinned_anime_catalogues`) and source ordering keys

- Pin set: `pinned_anime_catalogues` (string set of source ids). Pinned state is per source id, not per extension.
- Ordering uses `"${source.id}" !in pinnedSources` as the first sort key (global, migrate, and source list via part 2).
- Pin button writes through part 2's `ToggleAnimeSourcePin` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSourcePin.kt`, not read here).

## 13. Localization inputs

- `LocaleHelper.getDefaultEnabledLanguages()` uses `Locale.getDefault().language` (device language). Kurobayomi: parse `LANG`/`LC_MESSAGES` (`en_US.UTF-8` -> `en`), with `zh_CN` -> `zh-CN` mapping per `LocaleHelper.getLocalizedDisplayName` (section 0 of p1-behavior.md).
- Display names use `Locale.getDisplayName(locale)` (Java). Kurobayomi: `icu` or the `language-tags` crate with a small table; Proposal: use `icu_locid` (ICU4X) `DisplayNames` for parity. Cite the choice in p1-rust-design.md section 9.

## 14. Test fixtures (data side)

Fixtures are listed in p1-edge-cases-and-tests.md section 5.
