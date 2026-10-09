# Area 40 (part 1): data models, preferences, files, endpoints, DB

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Field types are Kotlin types; Rust equivalents are in p1-rust-design.md.

---

## 1. Extension models (used by the extension screens)

Source: `app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt`.

```kotlin
sealed class MangaExtension {
    abstract val name: String
    abstract val pkgName: String
    abstract val versionName: String
    abstract val versionCode: Long
    abstract val libVersion: Double
    abstract val lang: String?
    abstract val isNsfw: Boolean

    data class Installed(
        override val name: String,
        override val pkgName: String,
        override val versionName: String,
        override val versionCode: Long,
        override val libVersion: Double,
        override val lang: String,          // non-null
        override val isNsfw: Boolean,
        val pkgFactory: String?,
        val sources: List<MangaSource>,     // MangaSource = tachiyomi.domain.source.manga.model.Source
        val icon: Drawable?,                // Android-only, see section 7
        val hasUpdate: Boolean = false,
        val isObsolete: Boolean = false,
        val isShared: Boolean,
        val repoUrl: String? = null,
    ) : MangaExtension()

    data class Available(
        override val name: String,
        override val pkgName: String,
        override val versionName: String,
        override val versionCode: Long,
        override val libVersion: Double,
        override val lang: String,          // non-null
        override val isNsfw: Boolean,
        val sources: List<MangaSource>,     // Available.MangaSource (below)
        val apkName: String,
        val iconUrl: String,
        val repoUrl: String,
    ) : MangaExtension() {
        data class MangaSource(val id: Long, val lang: String, val name: String, val baseUrl: String) {
            fun toStubSource(): StubMangaSource  // id, lang, name
        }
    }

    data class Untrusted(
        override val name: String,
        override val pkgName: String,
        override val versionName: String,
        override val versionCode: Long,
        override val libVersion: Double,
        val signatureHash: String,
        override val lang: String? = null,
        override val isNsfw: Boolean = false,
    ) : MangaExtension()
}
```

Field usage in this area:

| Field | Used by | Notes |
|---|---|---|
| `name` | list row, details header, filter query, trust/remove dialogs | displayed as-is |
| `pkgName` | key for install progress, details lookup, debug copy, stripped name | `eu.kanade.tachiyomi.extension.` prefix is stripped for display |
| `versionName` | list row (if not empty), details info row, debug copy | may be empty; empty hides the row element |
| `versionCode` | debug copy only | `Long` |
| `libVersion` | debug copy only | `Double`; extension library version |
| `lang` | Installed row language (only if non-empty), Available grouping key, details language row | `getSourceDisplayName` |
| `isNsfw` | row warning `ext_nsfw_short`, details NSFW row | |
| `sources` | query filter, WebView (first Available source), enable/disable all, source rows, icon-action visibility (`isNotEmpty`) | |
| `hasUpdate` | updates group, Update button, update-all | Installed only |
| `isObsolete` | `ext_obsolete` warning, `obsolete_extension_message` banner, debug copy | Installed only |
| `isShared` | app-info button visibility (dropped) | Installed only |
| `repoUrl` | open-repo action (converted to GitHub URL when raw) | Installed |
| `iconUrl` | Available icon via image loader | Available |
| `repoUrl` (Available) | not used in this area directly | |
| `signatureHash` | not shown in this area | Untrusted |

## 2. Source models used by the screens

Domain source model (`tachiyomi.domain.source.manga.model.Source`, outside scope, area 09): used fields:

| Field | Type | Used for |
|---|---|---|
| `id` | `Long` | row key, toggles, `"${id}"` string set membership, `LocalMangaSource.ID == 0L` |
| `lang` | `String` | grouping by language, display name, `source.lang != ""` in migration list |
| `name` | `String` | title, search |
| `visualName` | `String` | options dialog title |
| `supportsLatest` | `Boolean` | show Latest button |
| `pin` | `Pins` (bit flags, `Pin.Pinned` = bit 0) | pin icon and label |
| `isStub` | `Boolean` | "not installed" label, stub icon |
| `isExcludedFromDataSaver` | `Boolean` | SY row only (dropped) |
| `key()` | `String` | list keys (`"source-${key}"`) |

Domain pin (`domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt`):
- `Unpinned = 0b00 (0)`, `Pinned = 0b01 (1)`, `Actual = 0b10 (2)`.
- `Pins(code: Int = 0)`; `contains(pin) = (pin.code and code) == pin.code`; `plus`, `minus` (xor). `Pins.pinned = Pins(Pinned, Actual) = 0b11`; `Pins.unpinned = 0`.
- Note: `Pins.minus` uses xor, so subtracting a bit not set adds it. Keep as-is.

`MangaExtensionSourceItem` (`app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionSources.kt:33-37`): `data class MangaExtensionSourceItem(source: MangaSource, enabled: Boolean, labelAsName: Boolean)`.

`MangaSource` (interface, `app/src/main/java/eu/kanade/tachiyomi/source/MangaSource.kt`, area 17): `id`, `name`, `lang`, `toString()` (used as label when `labelAsName`).

`CatalogueSource` (area 15/17): `supportsLatest`, `baseUrl`, `pin`, and the remaining fields used by the list code

`StubMangaSource(id, lang, name)`: placeholder for missing extensions; `isStub = true`.

`LocalMangaSource.ID = 0L`, `HELP_URL = "https://aniyomi.org/help/guides/local-manga/"` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:378-379`).

## 3. Manga model fields used by the grids and migration

Domain `tachiyomi.domain.entries.manga.model.Manga` (area 07):

| Field | Type | Used for |
|---|---|---|
| `id` | `Long` | MangaCover.mangaId, navigation, DB keys |
| `source` | `Long` | MangaCover.sourceId, migrate source |
| `url` | `String` | (not used here) |
| `title` | `String` | cell title, search, sort in favorites list |
| `favorite` | `Boolean` | in-library badge, cover alpha |
| `thumbnailUrl` | `String?` | MangaCover.url |
| `coverLastModified` | `Long` | MangaCover.lastModified (cache busting) |
| `dateAdded` | `Long` (epoch ms) | migration "Migrate" keeps it |
| `chapterFlags` | `Long` | copied to new manga on migrate |
| `viewerFlags` | `Long` | copied to new manga on migrate |
| `asMangaCover()` | extension | MangaCover from manga |

`MangaCover(mangaId, sourceId, isMangaFavorite, url, lastModified)` (cover model, area 34).

## 4. Migration models

`MangaMigrationFlag` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/MangaMigrationFlags.kt:11-15`):
```kotlin
data class MangaMigrationFlag(
    val flag: Int,              // one of 1, 2, 8, 16
    val isDefaultSelected: Boolean,
    val titleId: StringResource,
)
```

Flag bit constants (same file, lines 29-32, all `private const val`):
- `CHAPTERS = 0b00001` (1)
- `CATEGORIES = 0b00010` (2)
- `CUSTOM_COVER = 0b01000` (8)
- `DELETE_DOWNLOADED = 0b10000` (16)
- bit `0b00100` (4) is unassigned and must be preserved if present in a stored value.

`MigrateMangaSourceScreenModel` state pairs: `ImmutableList<Pair<Source, Long>>`, the Long is the count of favorites from that source (set in part 2).

Migration sort (`app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt`):
- `enum Mode { ALPHABETICAL, TOTAL }`
- `enum Direction { ASCENDING, DESCENDING }`
- Stored with `getEnum` (see preference table: stored as the enum name string).

`MangaSourceUiModel` (`app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt:223-226`): `Item(source: Source)`, `Header(language: String)`.

`MangaExtensionUiModel` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt:240-249`):
```kotlin
object MangaExtensionUiModel {
    sealed interface Header {
        data class Resource(val textRes: StringResource) : Header
        data class Text(val text: String) : Header
    }
    data class Item(val extension: MangaExtension, val installStep: InstallStep)
}
typealias ItemGroups = MutableMap<MangaExtensionUiModel.Header, List<MangaExtensionUiModel.Item>>
```

Browse-related enums:
- `LibraryDisplayMode` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`): `CompactGrid`, `ComfortableGrid`, `List`, `CoverOnlyGrid`. Default `CompactGrid`. Serialized as `"COMPACT_GRID"`, `"COMFORTABLE_GRID"`, `"LIST"`, `"COVER_ONLY_GRID"`. Deserialize unknown -> `CompactGrid`.
- `MangaSourceFilter` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt:213-216`): `All`, `PinnedOnly`.
- `InstallStep` (`app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt`): `Idle, Pending, Downloading, Installing, Installed, Error`; `isCompleted = Installed || Error || Idle`.
- `BasePreferences.ExtensionInstaller(titleRes, requiresSystemPermission)` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31`).
- `Listing` (`BrowseMangaSourceScreenModel.Listing`, part 2): `Popular`, `Latest`.

Grid columns (`BrowseMangaSourceScreenModel.getColumnsPreference`, part 2 file, lines 127-135):
- `orientation == LANDSCAPE` -> `libraryPreferences.mangaLandscapeColumns()`; else `mangaPortraitColumns()`; `.get()`.
- `0` -> `GridCells.Adaptive(128.dp)`; `n > 0` -> `GridCells.Fixed(n)`.

Dialog sealed types:
- `BrowseMangaSourceScreenModel.Dialog`: `Filter`, `Migrate(newManga, oldManga)` (part 2).
- `MangaMigrateSearchScreenDialogScreenModel.Dialog`: `Migrate(manga)`.
- `MangaExtensionFilterEvent`: `FailedFetchingLanguages`.
- `MangaExtensionDetailsEvent`: `Uninstalled`.
- `MigrationMangaEvent`: `FailedFetchingFavorites`.

## 5. Preference keys (exact)

Keys, types, and defaults used or read by this area. Stores use `tachiyomi.core.common.preference` (`PreferenceStore`).

| Key | Owner | Type | Default | Read/write in this area |
|---|---|---|---|---|
| `source_languages` | `SourcePreferences.enabledLanguages()` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:23-26`) | StringSet | `LocaleHelper.getDefaultEnabledLanguages()` = `{"all", "en", <system language code>}` | write: language toggle (ToggleLanguage); read: extension language filter, source filter |
| `pref_migration_sorting` | `migrationSortingMode()` (line 30-33) | enum `SetMigrateSorting.Mode` | `ALPHABETICAL` | write: migration source sort toggle; read: migration source list |
| `pref_migration_direction` | `migrationSortingDirection()` (line 35-38) | enum `SetMigrateSorting.Direction` | `ASCENDING` | same |
| `hidden_catalogues` | `disabledMangaSources()` (line 57) | StringSet of source id strings | empty set | write: source filter checkbox; read: source filter checkbox, browse filter |
| `pinned_catalogues` | `pinnedMangaSources()` (line 63) | StringSet of source id strings | empty set | write: pin button; read: sort in migrate search, pinned-only filter |
| `incognito_manga_extensions` | `incognitoMangaExtensions()` (line 60) | StringSet of package names | empty set | write: incognito switch; read: details screen |
| `ext_updates_count` | `mangaExtensionUpdatesCount()` (line 75) | Int | `0` | read: tab badge; written by extension manager (area 18) |
| `migrate_flags` | `MigrateMangaDialogScreenModel.migrateFlags` (`MigrateMangaDialog.kt:165-167`) | Int | `Int.MAX_VALUE` (0x7FFFFFFF) | read: dialog default selection; write: every migrate call |
| `pref_display_mode_catalogue` | `sourceDisplayMode()` (`SourcePreferences.kt:16-19`) | LibraryDisplayMode via `Serializer` | `LibraryDisplayMode.default` = `CompactGrid` | read/write: browse display mode (shared with anime browse) |
| `show_nsfw_source` | `showNsfwSource()` | Boolean | `true` | read in part 2 source list; not read directly here |
| (see `ExtensionInstallerPreference`) | `BasePreferences.extensionInstaller()` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:23`) | `ExtensionInstaller` enum | area 02 owns the key and default | read: permission banner (dropped) |
| library column prefs | `libraryPreferences.mangaPortraitColumns()`, `mangaLandscapeColumns()` (area 37) | Int | `0` (adaptive 128dp) | read: grid columns |

Notes:
- `hidden_catalogues` stores **ids as strings** (`"${source.id}"`), not names. Keep the same encoding so that existing backups import without conversion.
- `pinned_catalogues` the same encoding (`"${it.id}" in pinnedSources`).
- `source_languages` contains language codes such as `"all"`, `"en"`, `"ja"`. `"all"` is multi-language.

Kurobayomi: store the same keys in its own config file with the same defaults (p1-rust-design.md section 10).

## 6. Extension package and install files (as used here)

This area does not write extension files itself. It uses:
- `MangaExtensionManager` (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt`, area 18) for install/update/uninstall/trust/cancel and `installedExtensionsFlow`.
- Package lookup `context.isPackageInstalled(pkgName)` (`eu.kanade.tachiyomi.util.system`, Android `PackageManager`). Kurobayomi replacement: lookup the extension record in the local extension registry (see D3 in p1-behavior.md section 16).
- Extension icon: Android PackageManager resource id `appInfo.icon` (`BrowseMangaIcons.kt:125-145`). Kurobayomi: icon file from the extension bundle.

Kurobayomi on-disk layout (proposal, final decision in area 18/20):
- `$XDG_DATA_HOME/kurobayomi/extensions/manga/<pkgName>/` for the installed extension bundle (manifest + icon + code). Untrusted bundles are placed in `.../untrusted/<pkgName>/` until trusted.
- `$XDG_CACHE_HOME/kurobayomi/ext-icons/<pkgName>-<px>.png` for decoded icons at the requested size (40 px rows, 112 px details).
- Cover and icon fetches for Available extensions use the image cache directory from area 34.

## 7. Network endpoints touched

| Endpoint | Method | Who | Purpose |
|---|---|---|---|
| `extension.iconUrl` (from the repo index) | GET | Coil `AsyncImage` in `MangaExtensionIcon` (Available) | extension icon; placeholder `ColorPainter(Color(0x1F888888))`; error placeholder `R.drawable.cover_error` |
| `source.baseUrl` (HttpSource) | none in this area | `WebViewScreen(url = baseUrl)` | opens the site in a WebView (area 36) |
| `repoUrl` raw GitHub -> GitHub URL | none | `uriHandler.openUri` | regex `https://raw.githubusercontent.com/(.+?)/(.+?)/.+` -> `https://github.com/{user}/{repo}`; opens in browser |
| `https://aniyomi.org/docs/guides/troubleshooting/` (`Constants.URL_HELP`) | none | `uriHandler.openUri` | help link |
| `https://aniyomi.org/help/guides/local-manga/` (`LocalMangaSource.HELP_URL`) | none | `uriHandler.openUri` | local source help |
| `findAvailableExtensions()` | GET (repo index, area 18) | extension manager | refresh |
| Cookie jar removal | none (local) | `network.cookieJar.remove(url)` | clears cookies for each HTTP source base URL |

Kurobayomi: URL conversion must match the regex byte-for-byte. Cookie removal uses the host of the URL (`HttpUrl`), same semantics as OkHttp `CookieJar.remove`.

## 8. Database tables touched by the migration engine

Schema source files: `data/src/main/sqldelight/data/*.sq`.

`mangas` (`mangas.sq`, `CREATE TABLE mangas(`):
- `_id INTEGER NOT NULL PRIMARY KEY`, `source INTEGER NOT NULL`, `url TEXT NOT NULL`, `title TEXT NOT NULL`, `favorite INTEGER AS Boolean NOT NULL`, `thumbnail_url TEXT`, `last_update INTEGER`, plus further columns defined in that file; this area reads and writes only the columns named here.
- Written by `UpdateManga.await(MangaUpdate(...))` for `favorite`, `chapterFlags`, `viewerFlags`, `dateAdded` (step 8).
- Written by `UpdateManga.awaitUpdateFavorite(oldId, favorite=false)` (step 6).

`chapters` (`chapters.sq`, `CREATE TABLE chapters(`):
- Columns (`data/src/main/sqldelight/data/chapters.sq`): `_id`, `manga_id`, `url`, `name`, `scanlator`, `read INTEGER AS Boolean`, `bookmark INTEGER AS Boolean`, `last_page_read INTEGER`, `chapter_number REAL`, `source_order`, `date_fetch`, `date_upload`, `last_modified_at`, `version`, `is_syncing`. `FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE`.
- Read by `GetChaptersByMangaId` for old and new manga (step 2).
- Written by `UpdateChapter.awaitAll` (step 2) with `read`, `bookmark`, `date_fetch` changes.
- `isRecognizedNumber` is a property of the domain chapter model (area 07). Definition (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:19-20`): `get() = chapterNumber >= 0f`. A chapter number of `-1` (or any negative value) is unrecognised.

`mangas_categories` (`mangas_categories.sq`, `CREATE TABLE mangas_categories(`):
- `_id`, `manga_id INTEGER NOT NULL`, `category_id INTEGER NOT NULL`, foreign keys to `categories(_id)` and `mangas(_id)` with `ON DELETE CASCADE`.
- Read by `GetMangaCategories.await(oldId)`; replaced by `SetMangaCategories.await(newId, ids)` (step 3).

`tracks` (table defined in `data/src/main/sqldelight/data/` by area 06/27; read the `.sq` file for columns): read by `GetMangaTracks.await(oldId)`; the copies with `mangaId = newId` are inserted via `InsertMangaTrack.awaitAll` (step 4).

`manga_sync` (`manga_sync.sq`): not touched in this area directly (used by tracking, area 27).

Downloads on disk: `MangaDownloadManager.deleteManga(oldManga, oldSource)` (step 5); the folder is the source/manga directory under the download root (area 23).

Covers: `MangaCoverCache.setCustomCoverToCache(newManga, stream)` and `getCustomCoverFile(oldId)` (step 7; area 34).

## 9. Entities created or changed by a migration (summary)

| Entity | Change | Condition |
|---|---|---|
| new manga | `favorite = true`, `chapterFlags`, `viewerFlags` copied, `dateAdded` set | always (step 8) |
| new manga chapters | `dateFetch`, `bookmark` copied from matching old chapter; `read = true` when `chapterNumber <= maxRead` | `CHAPTERS` flag |
| new manga categories | replaced with old manga's categories | `CATEGORIES` flag |
| new manga tracks | copied (with enhanced-tracker migration) | always |
| old manga | `favorite = false` | "Migrate" (replace) only |
| old downloads | deleted | `DELETE_DOWNLOADED` flag and old source present |
| new manga custom cover | set from old cover file | `CUSTOM_COVER` flag and old has custom cover |
| `migrate_flags` preference | set to the bitmap | every migrate call |

## 10. Config file (Kurobayomi) for the preferences above

Format: TOML at `$XDG_CONFIG_HOME/kurobayomi/browse.toml` (area 02 owns the file system; keys here are the Kurobayomi names with the Aniyomi key kept as a comment so imports work).

```toml
[browse]
display_mode = "COMPACT_GRID"          # pref_display_mode_catalogue; COMPACT_GRID | COMFORTABLE_GRID | LIST | COVER_ONLY_GRID
hidden_sources = []                    # hidden_catalogues: source ids as strings
pinned_sources = []                    # pinned_catalogues: source ids as strings
enabled_languages = ["all", "en"]      # source_languages (default: all, en, system language)
incognito_extensions = []              # incognito_manga_extensions: package names
migrate_sort_mode = "ALPHABETICAL"     # pref_migration_sorting: ALPHABETICAL | TOTAL
migrate_sort_direction = "ASCENDING"   # pref_migration_direction: ASCENDING | DESCENDING
migrate_flags = 2147483647             # migrate_flags (Int.MAX_VALUE default); bit map, see section 4
show_nsfw_sources = true               # show_nsfw_source
```

Values written by the app must round-trip through the `serialize()`/`deserialize()` functions above. `deserialize` of an unknown `display_mode` falls back to `COMPACT_GRID`; of an unknown sort mode falls back to `ALPHABETICAL`; of an unknown direction to `ASCENDING`.

## 11. Strings and resources

- String keys are listed in p1-behavior.md section 18. Kurobayomi uses the same keys in its own i18n table (area 02/36); the English text comes from `app/src/main/res/` and the moko `MR` resources (`MR.strings.*`, `AYMR.strings.*`), not copied here.
- Drawables used: `R.drawable.ic_glasses_24dp` (incognito), `R.drawable.cover_error` (cover error), `R.drawable.anim_browse_enter` (tab animation), `R.mipmap.ic_local_source`, `R.mipmap.ic_default_source` (source icons). Kurobayomi: SVG symbolic icons, see D8 in p1-behavior.md.

## 12. Icon names used (Material to GTK symbolic)

| Compose icon | Used for | GTK symbolic proposal |
|---|---|---|
| `Icons.Outlined.CollectionsBookmark` | in-library badge | `bookmark-new-symbolic` |
| `Icons.Outlined.Public` | web view action | `web-browser-symbolic` |
| `Icons.Outlined.Refresh` | retry, refresh | `view-refresh-symbolic` |
| `Icons.AutoMirrored.Outlined.HelpOutline` | help | `help-about-symbolic` |
| `Icons.AutoMirrored.Outlined.ArrowForward` | source row arrow | `go-next-symbolic` |
| `Icons.Outlined.Error` | global search error | `dialog-error-symbolic` |
| `Icons.Outlined.Close` | cancel install | `window-close-symbolic` |
| `Icons.Outlined.Settings` | settings | `emblem-system-symbolic` |
| `Icons.Outlined.GetApp` | install, update | `folder-download-symbolic` |
| `Icons.Outlined.VerifiedUser` | trust | `security-high-symbolic` |
| `Icons.Filled.Dangerous` | untrusted icon | `dialog-warning-symbolic` (error tint) |
| `Icons.Filled.Warning` | stub source icon | `dialog-warning-symbolic` (error tint) |
| `Icons.Outlined.PushPin` / `Icons.Filled.PushPin` | pin | `view-pin-symbolic` (filled variant unavailable: use CSS class `pinned`) |
| `Icons.Outlined.DoneAll` | "All" chip | `object-select-symbolic` |
| `Icons.Outlined.FilterList` | filters, "has results" | `view-filter-symbolic` |
| `Icons.Filled.ViewModule` | display mode grid | `view-grid-symbolic` |
| `Icons.AutoMirrored.Filled.ViewList` | display mode list | `view-list-symbolic` |
| `Icons.Outlined.SortByAlpha` | sort alpha | `view-sort-ascending-symbolic` (alpha sort) |
| `Icons.Outlined.Numbers` | sort by count | `format-text-bold-symbolic` placeholder (no GTK "numbers" icon: use a bundled SVG) |
| `Icons.Outlined.ArrowUpward` / `ArrowDownward` | sort direction | `pan-up-symbolic` / `pan-down-symbolic` |
| `Icons.AutoMirrored.Outlined.Launch` | open repo | `external-link-symbolic` (bundled if absent) |
| `Icons.Outlined.DoneAll` | all | as above |

Icon names are proposals; the final list is set in area 02/36 (icon theme). Where a GTK icon is absent in Adwaita, ship an SVG in `data/icons/`.

## 13. Sizes and layout constants

- `CommonEntryItemDefaults.GridHorizontalSpacer = 4.dp`, `GridVerticalSpacer = 4.dp`, `BrowseFavoriteCoverAlpha = 0.34f` (`app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:50-54`).
- Browse grid padding `PaddingValues(8.dp)` added to content padding.
- List padding `vertical = 8.dp`.
- Global search card width `96.dp`, title max lines `3` (`GlobalMangaSearchCardRow`).
- Source row: `BaseBrowseItem` padding horizontal `medium`, vertical `small`.
- Extension icon: 40 dp in list rows (`Modifier.size(40.dp)`), 8 dp padding while busy (animated), 112 dp in details header.
- Extension details buttons: `padding(horizontal = medium)`, `top = small`, spacing `medium`.
- Info row: `extraLarge` horizontal, `small` vertical.
- Migration dialog checkbox list: vertical scroll.
- Loading overlay during migration: `background.copy(alpha = 0.7f)`.
- Progress bar in global search: `LinearProgressIndicator`, `BottomStart`, `fillMaxWidth`.
- Spinner sizes: loading row 16dp vertical padding, global search spinner 16 dp (stroke 2 dp), extension spinner 40 dp (stroke 2 dp).
- `SECONDARY_ALPHA` (`tachiyomi.presentation.core.components.material`) used for the unpinned icon tint.
- `SEARCH_DEBOUNCE_MILLIS = 250L`.
- Snackbar duration: `SnackbarDuration.Indefinite` for list errors.

All of these are Compose dp values; Kurobayomi uses CSS-like px at 96 dpi scaling via GTK, documented in p1-ui-parity.md section 11.
