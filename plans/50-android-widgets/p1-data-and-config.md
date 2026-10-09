# 50-android-widgets — Part 1 data models, preferences, files, endpoints, DB

Source root: `/home/blackbox/aniyomi-src`. Everything marked OBSERVED was read from the source. Items marked PROPOSAL are the Linux/Rust data design.

## 1. Domain models used by the widgets (OBSERVED)

### 1.1 `MangaCover` — `domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaCover.kt:8-14`

| Field | Kotlin type | Nullable | Default | Meaning in the widget |
|---|---|---|---|---|
| `mangaId` | `Long` | no | none | entry id; cache key part |
| `sourceId` | `Long` | no | none | source for request headers (`sourceLazy`) |
| `isMangaFavorite` | `Boolean` | no | none | widget always passes `true` (`BaseMangaUpdatesGridGlanceWidget.kt:131`); maps to `isLibraryManga` in the fetcher (`MangaCoverFetcher.kt:330`) |
| `url` | `String?` | yes | none | `thumbnail_url` of the manga; `null` -> "No cover specified" error |
| `lastModified` | `Long` | no | none | `cover_last_modified`; part of the Coil key |

Implements `EntryCover`.

### 1.2 `AnimeCover` — `domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeCover.kt:8-14`

Same shape: `animeId: Long`, `sourceId: Long`, `isAnimeFavorite: Boolean` (widget passes `true`, `BaseAnimeUpdatesGridGlanceWidget.kt:131`), `url: String?`, `lastModified: Long`.

### 1.3 `MangaUpdatesWithRelations` — `domain/src/main/java/tachiyomi/domain/updates/manga/model/MangaUpdatesWithRelations.kt:5-17`

| Field | Type | Nullable | Notes |
|---|---|---|---|
| `mangaId` | `Long` | no | used for dedupe and click target |
| `mangaTitle` | `String` | no | not shown by the widget (no text on covers) |
| `chapterId` | `Long` | no | used by the manager's distinct key |
| `chapterName` | `String` | no | not shown |
| `scanlator` | `String?` | yes | not shown |
| `read` | `Boolean` | no | always `false` for widget rows (query filter) |
| `bookmark` | `Boolean` | no | not shown |
| `lastPageRead` | `Long` | no | not shown |
| `sourceId` | `Long` | no | copied into `MangaCover.sourceId` |
| `dateFetch` | `Long` | no | epoch ms; not used by widget |
| `coverData` | `MangaCover` | no | built by the repository (section 3.1) |

### 1.4 `AnimeUpdatesWithRelations` — `domain/src/main/java/tachiyomi/domain/updates/anime/model/AnimeUpdatesWithRelations.kt`

`animeId: Long`, `animeTitle: String`, `episodeId: Long`, `episodeName: String`, `scanlator: String?`, `seen: Boolean`, `bookmark: Boolean`, `fillermark: Boolean`, `lastSecondSeen: Long`, `totalSeconds: Long`, `sourceId: Long`, `dateFetch: Long`, `coverData: AnimeCover`.

### 1.5 Widget-internal types (OBSERVED)

- `ImmutableList<Pair<Long, Bitmap?>>?` — the feed passed to the Compose body. `null` means loading; `Pair.first` = entry id (manga or anime); `Pair.second` = decoded bitmap or `null` (placeholder).
- `Bitmap` is `android.graphics.Bitmap`, created by `.image?.asDrawable(context.resources)?.toBitmap()`.
- `DpSize` (`androidx.compose.ui.unit.DpSize`) from `LocalSize`/`getAppWidgetSizes`; `Dp` values for paddings.

### 1.6 Enums

None declared in scope. The only enum-like values are the action strings and extra keys below.

## 2. Constants and literal values (OBSERVED)

| Name | Type | Value | Location |
|---|---|---|---|
| `CoverWidth` | `Dp` | `58.dp` | `components/manga/UpdatesMangaCover.kt:16`, `components/anime/UpdatesAnimeCover.kt:16` |
| `CoverHeight` | `Dp` | `87.dp` | `components/manga/UpdatesMangaCover.kt:17`, `components/anime/UpdatesAnimeCover.kt:17` |
| row padding | `Dp` | `4.dp` vertical | `components/*/Updates*Widget.kt:63` |
| cell padding | `Dp` | `3.dp` horizontal | `components/*/Updates*Widget.kt:71` |
| row unit for count | `Float` | `95` | `util/GlanceUtils.kt:36` |
| column unit for count | `Float` | `64` | `util/GlanceUtils.kt:37` |
| row cap | `Int` | `coerceIn(1, 10)` | `util/GlanceUtils.kt:36` |
| column cap | `Int` | `coerceIn(1, 10)` | `util/GlanceUtils.kt:37` |
| window | `Int` | `minusMonths(3)` | `entries/*/Base*GlanceWidget.kt:160` |
| DB limit | `Long` | `500` | `domain/.../interactor/GetMangaUpdates.kt:21`, `GetAnimeUpdates.kt:21` |
| locked padding | `Dp` | `8.dp` | `components/*/Locked*Widget.kt:33` |
| locked font | `TextUnit` | `12.sp`, centre | `components/*/Locked*Widget.kt:40-41` |
| cover screen bottom padding | `Dp` | `24.dp` | `entries/*/*CoverScreenGlanceWidget.kt:13` (manga) and anime twin |
| cover screen foreground | `Color` | `Color.White` | `entries/*/*CoverScreenGlanceWidget.kt:10` |
| `sizeMode` | `SizeMode` | `SizeMode.Exact` | `entries/*/Base*GlanceWidget.kt:61` |
| Coil memory cache | `CachePolicy` | `DISABLED` | `Base*GlanceWidget.kt:136` |
| Coil precision | `Precision` | `EXACT` | `Base*GlanceWidget.kt:137` |
| Coil scale | `Scale` | `FILL` | `Base*GlanceWidget.kt:139` |
| Pre-API-31 corner transform | `RoundedCornersTransformation(roundPx)` | `roundPx` = `appwidget_inner_radius` in px | `Base*GlanceWidget.kt:120,141-143` |
| API threshold | `Int` | `Build.VERSION_CODES.S` (31) | `Base*GlanceWidget.kt:141` |
| Placeholder fallback | drawable | `appwidget_cover_error` | `components/*/Updates*Cover.kt:41` |
| Image content scale | `ContentScale` | `Crop` | `components/*/Updates*Cover.kt:36,44` |
| Manifest enabled gate | bool | `@bool/glance_appwidget_available` | `AndroidManifest.xml` (defined by the Glance AAR, not in this repo) |

Action and extra strings (`core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt`):

| Constant | Value | Line |
|---|---|---|
| `MANGA_EXTRA` | `"manga"` | 7 |
| `ANIME_EXTRA` | `"anime"` | 9 |
| `MAIN_ACTIVITY` | `"eu.kanade.tachiyomi.ui.main.MainActivity"` | 11 |
| `SHORTCUT_ANIMELIB` | `"eu.kanade.tachiyomi.SHOW_ANIMELIB"` | 14 |
| `SHORTCUT_LIBRARY` | `"eu.kanade.tachiyomi.SHOW_LIBRARY"` | 15 |
| `SHORTCUT_ANIME` | `"eu.kanade.tachiyomi.SHOW_ANIME"` | 16 |
| `SHORTCUT_MANGA` | `"eu.kanade.tachiyomi.SHOW_MANGA"` | 17 |
| `SHORTCUT_UPDATES` | `"eu.kanade.tachiyomi.SHOW_RECENTLY_UPDATED"` | 18 |

Extras are `Long` (`putExtra(MANGA_EXTRA, mangaId)`, read with `getLong`).

## 3. Preference keys touched (OBSERVED)

| Key | Type | Default | Accessor | Read by | Notes |
|---|---|---|---|---|---|
| `use_biometric_lock` | Boolean | `false` | `SecurityPreferences.useAuthenticator()` (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:13`) | `BaseMangaUpdatesGridGlanceWidget.kt:69`, `BaseAnimeUpdatesGridGlanceWidget.kt:69`, `MangaWidgetManager.kt:28`, `AnimeWidgetManager.kt:28` | Lock flag. Widgets show the locked placeholder when true. |
| `lock_app_after` | Int | `0` | `SecurityPreferences.lockAppAfter()` (line 15) | not read in scope | Documented here because the app lock depends on it; the widget ignores it (behaviour section 3). |

Preferences not used by the widget: no per-widget settings, no configuration activity (the provider XMLs have no `android:configure`), no widget-specific Glance state. Glance's per-instance state store is not used (`GlanceStateDefinition` is not declared), so nothing is written per widget instance.

## 4. Provider XML resources (OBSERVED, verbatim values)

### 4.1 `presentation-widget/src/main/res/xml/updates_grid_homescreen_widget_info.xml`

```xml
<appwidget-provider
    android:description="@string/appwidget_updates_description"
    android:previewImage="@drawable/updates_grid_widget_preview"
    android:initialLayout="@layout/appwidget_loading"
    android:minWidth="240dp"
    android:minHeight="80dp"
    android:minResizeWidth="80dp"
    android:minResizeHeight="110dp"
    android:maxResizeWidth="600dp"
    android:maxResizeHeight="600dp"
    android:targetCellWidth="4"
    android:targetCellHeight="2"
    android:resizeMode="horizontal|vertical"
    android:widgetCategory="home_screen" />
```

- No `android:updatePeriodMillis`: default 0, so no periodic refresh.
- Used by both `MangaUpdatesGridGlanceReceiver` and `AnimeUpdatesGridGlanceReceiver`.

### 4.2 `presentation-widget/src/main/res/xml/updates_grid_lockscreen_widget_info.xml`

Attributes: `android:description="@string/appwidget_updates_description"`, `android:previewImage="@drawable/updates_grid_coverscreen_widget_preview"`, `android:initialLayout="@layout/appwidget_coverscreen_loading"`, `android:resizeMode="horizontal|vertical"`, `android:widgetCategory="keyguard"`.

### 4.3 `presentation-widget/src/main/res/xml/updates_grid_samsung_cover_widget_info.xml`

`<samsung-appwidget-provider display="sub_screen" privacyWidget="true" />`.

### 4.4 Manifest receivers (OBSERVED, `presentation-widget/src/main/AndroidManifest.xml`)

| Receiver class | Label | Provider meta-data | Intent filter |
|---|---|---|---|
| `tachiyomi.presentation.widget.entries.manga.MangaUpdatesGridGlanceReceiver` | `@string/label_recent_updates` | `android.appwidget.provider` -> `updates_grid_homescreen_widget_info` | `android.appwidget.action.APPWIDGET_UPDATE` |
| `tachiyomi.presentation.widget.entries.anime.AnimeUpdatesGridGlanceReceiver` | `@string/label_recent_anime_updates` | same XML as the manga home | same |
| `tachiyomi.presentation.widget.entries.manga.MangaUpdatesGridCoverScreenGlanceReceiver` | `@string/label_recent_updates` | `updates_grid_lockscreen_widget_info`, `com.samsung.android.appwidget.provider` -> `updates_grid_samsung_cover_widget_info`, `com.samsung.android.sdk.subscreen.widget.support_visibility_callback` = `true` | same |
| `tachiyomi.presentation.widget.entries.anime.AnimeUpdatesGridCoverScreenGlanceReceiver` | `@string/label_recent_anime_updates` | same as manga cover | same |

All four: `android:enabled="@bool/glance_appwidget_available"`, `android:exported="false"`.

## 5. Colour and dimension resources (OBSERVED)

### 5.1 Widget tokens (`presentation-widget/src/main/res/values/colors_appwidget.xml`)

| Token | Value |
|---|---|
| `appwidget_background` | `@color/tachiyomi_surface` |
| `appwidget_coverscreen_background` | `#00000000` |
| `appwidget_on_background` | `@color/tachiyomi_onSurface` |
| `appwidget_surface_variant` | `@color/tachiyomi_surfaceVariant` |
| `appwidget_on_surface_variant` | `@color/tachiyomi_onSurfaceVariant` |
| `appwidget_secondary_container` | `@color/tachiyomi_secondaryContainer` |
| `appwidget_on_secondary_container` | `@color/tachiyomi_onSecondaryContainer` |

Used in code: only `appwidget_on_secondary_container` (manga and anime home foreground) and `appwidget_secondary_container` (via `drawable/appwidget_background.xml`). `appwidget_background` (the colour), `appwidget_on_background`, `appwidget_surface_variant` and `appwidget_on_surface_variant` are declared but unused in scope.

### 5.2 Night values (`values-night-v31/colors_appwidget.xml`, Android 12+)

`appwidget_background` = `m3_sys_color_dynamic_dark_surface`, `appwidget_on_background` = `m3_sys_color_dynamic_dark_on_surface`, `appwidget_surface_variant` = `m3_sys_color_dynamic_dark_surface_variant`, `appwidget_on_surface_variant` = `m3_sys_color_dynamic_dark_on_surface_variant`, `appwidget_secondary_container` = `m3_sys_color_dynamic_dark_secondary_container`, `appwidget_on_secondary_container` = `m3_sys_color_dynamic_dark_on_secondary_container`. The `m3_sys_color_dynamic_*` tokens come from the Material components library (`libs.material`), not from this repo.

### 5.3 Light values (`values-v31/colors_appwidget.xml`)

Same six names with `m3_sys_color_dynamic_light_*`.

### 5.4 Tachiyomi tokens these fall back to (OBSERVED, `presentation-core`)

| Token | Light (`presentation-core/src/main/res/values/colors_tachiyomi.xml`) | Night (`presentation-core/src/main/res/values-night/colors_tachiyomi.xml`) |
|---|---|---|
| `tachiyomi_surface` | `#FEFBFF` (line 28) | `#1B1B1F` (line 28) |
| `tachiyomi_onSurface` | `#1B1B1F` (line 29) | `#E3E2E6` (line 29) |
| `tachiyomi_surfaceVariant` | `#F3EDF7` (line 30) | `#211F26` (line 30) |
| `tachiyomi_onSurfaceVariant` | `#44464F` (line 31) | `#C5C6D0` (line 31) |
| `tachiyomi_secondaryContainer` | `#D9E2FF` (line 20) | `#00429B` (line 20) |
| `tachiyomi_onSecondaryContainer` | `#001945` (line 21) | `#D9E2FF` (line 21) |

Placeholder fill: `cover_placeholder` = `#1F888888` (`presentation-core/src/main/res/values/colors.xml:5`).

### 5.5 Dimensions

| Resource | Value | File |
|---|---|---|
| `appwidget_background_radius` | `16dp` | `presentation-widget/src/main/res/values/dimens.xml:2` |
| `appwidget_inner_radius` | `12dp` | `presentation-widget/src/main/res/values/dimens.xml:3` |
| `appwidget_background_radius` (API 31+) | `@android:dimen/system_app_widget_background_radius` | `presentation-widget/src/main/res/values-v31/dimens.xml:2` |
| `appwidget_background_radius` (app module) | `16dp` | `app/src/main/res/values/dimens.xml:4` (also `appwidget_inner_radius` = `12dp`, line 5) |

Resource-merge note (OBSERVED from the directory layout, not run): on API 31+ the library's `values-v31` value is the more specific qualifier, so it is the one selected. The app module's `values/` value applies below API 31. Verify on a device before relying on it.

### 5.6 Drawables and layouts (OBSERVED)

- `drawable/appwidget_background.xml`: rectangle, solid `@color/appwidget_secondary_container`, corners `@dimen/appwidget_background_radius`.
- `drawable/appwidget_coverscreen_background.xml`: rectangle, solid `@color/appwidget_coverscreen_background`.
- `drawable/appwidget_cover_error.xml`: `layer-list`. Item 1: rectangle, solid `@color/cover_placeholder`, corners `@dimen/appwidget_inner_radius`. Item 2 (inset 24 dp on all sides): `vector` 24x24dp, viewport 24x24, path fill `@color/cover_placeholder`, a picture-frame glyph (path string in the file).
- `layout/appwidget_loading.xml`: `FrameLayout` match_parent, background `@drawable/appwidget_background`, `TextView` wrap_content, gravity centre, text `@string/loading`, colour `?android:attr/textColorPrimary`.
- `layout/appwidget_coverscreen_loading.xml`: same, background `@drawable/appwidget_coverscreen_background`, colour `@android:color/white`.

Drawable and layout names referenced from `W/` code: `R.drawable.appwidget_background`, `R.drawable.appwidget_coverscreen_background`, `R.drawable.appwidget_cover_error`, `R.color.appwidget_on_secondary_container`, `R.dimen.appwidget_background_radius`, `R.dimen.appwidget_inner_radius`.

## 6. Message keys (OBSERVED)

See `p1-behavior.md` section 12 for the full table. Keys: `information_no_recent`, `appwidget_unavailable_locked`, `loading`, `label_recent_updates`, `label_recent_anime_updates`, `appwidget_updates_description`. Accessed in Kotlin via `MR.strings.<key>` (`tachiyomi.i18n.MR`), including by the anime widget.

## 7. Database objects read (OBSERVED)

### 7.1 Tables and columns used through the views

Manga (`data/src/main/sqldelight/data/`):

- `mangas`: `_id`, `title`, `source`, `favorite`, `thumbnail_url`, `cover_last_modified`, `date_added` (`mangas.sq:6-24`).
- `chapters` (`data/src/main/sqldelight/data/chapters.sq`): `_id`, `manga_id`, `name`, `scanlator`, `read` (Boolean), `bookmark`, `last_page_read`, `date_fetch`, `date_upload`.
- Indexes (same file): `chapters_manga_id_index ON chapters(manga_id)`, `chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0`. The partial index on unread rows matches the widget filter `read = 0`, but the view joins on `chapters.manga_id`, so the planner's use of it is not guaranteed (edge case E-9).

Anime (`data/src/main/sqldelightanime/dataanime/`):

- `animes`: `_id`, `title`, `source`, `favorite`, `thumbnail_url`, `cover_last_modified`, `date_added` (`animes.sq:26`).
- `episodes` (`data/src/main/sqldelightanime/dataanime/episodes.sq`): `_id`, `anime_id`, `name`, `scanlator`, `seen` (Boolean), `bookmark`, `fillermark`, `last_second_seen`, `total_seconds`, `date_fetch`, `date_upload`.
- Indexes (same file): `episodes_anime_id_index ON episodes(anime_id)`, `episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0`.

### 7.2 Views

- `updatesView` — `data/src/main/sqldelight/view/updatesView.sq` (full text in `p1-behavior.md` section 13).
- `animeupdatesView` — `data/src/main/sqldelightanime/view/animeupdatesView.sq`.
- Queries: `getUpdatesByReadStatus(read, after, limit)`, `getUpdatesBySeenStatus(seen, after, limit)`. Both are `SELECT * ... WHERE <flag> = :x AND dateUpload > :after LIMIT :limit;` with no ORDER BY.

Writes: none. The widget performs no INSERT/UPDATE/DELETE.

## 8. Files on disk (OBSERVED)

### 8.1 Cover cache

- Library covers (`isLibraryManga` true, which is always the case for widget covers): `MangaCoverCache.getCoverFile(url)` = `File(cacheDir, DiskUtil.hashKeyForDisk(url))` with `cacheDir = context.getExternalFilesDir("covers") ?: File(context.filesDir, "covers")` (`app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt:21,28,38-41,101-104`). File names are the hash of the URL (the class doc comment says md5).
- Custom covers: `MangaCoverCache.getCustomCoverFile(mangaId)` = `File(customCoverCacheDir, DiskUtil.hashKeyForDisk(mangaId.toString()))` with `customCoverCacheDir` for `"covers/custom"` (`MangaCoverCache.kt:22,30,50-51`).
- Non-library covers (Coil's own disk cache) are not used by the widget.

### 8.2 Writes made by the widget path

- Coil network loads write the library cover file (`MangaCoverFetcher.httpLoader`, `libraryCoverCacheFile` branch, `MangaCoverFetcher.kt:105-145`), because the widget passes `isLibraryManga = true`.
- Nothing else is written.

### 8.3 Glance-owned storage

Glance's AppWidget state and `GlanceAppWidgetManager` ids are managed by the Glance library. This widget declares no state definition, so nothing is persisted per instance by the app.

## 9. Network endpoints (OBSERVED)

Only one HTTP request: the cover image.

- Method: `GET`.
- URL: `MangaCover.url` = the manga's `thumbnail_url` (`mangas.thumbnail_url`), or `AnimeCover.url` = `animes.thumbnail_url`. Set by the source. No fixed host.
- Client: `sourceLazy.value?.client` if the source is loaded, otherwise `callFactoryLazy.value` (`MangaCoverFetcher.kt:170-173`).
- Headers: the source's headers (`sourceLazy.value?.headers`) when the source is loaded. No other custom headers.
- Cache control: `CACHE_CONTROL_NO_STORE` (`noStore()`) when the request's network cache policy is readable; otherwise `CACHE_CONTROL_NO_NETWORK_NO_CACHE` (`noCache().onlyIfCached()`) (`MangaCoverFetcher.kt:191-197,345-346`).
- Success check: `response.isSuccessful || response.code == 304` (`HTTP_NOT_MODIFIED`), else `IOException(response.message)` (`MangaCoverFetcher.kt:170-177`).

No API, no auth, no tracker endpoint, no analytics. Covers from sources that need login or Cloudflare handling follow the source's client (area:12-network-stack and area:15-source-api-manga and area:16-source-api-anime).

## 10. Glance and Android APIs in scope (OBSERVED)

| API | Version / source | Used for |
|---|---|---|
| `androidx.glance:glance-appwidget:1.1.1` | `gradle/compose.versions.toml:18` | `GlanceAppWidget`, `GlanceAppWidgetReceiver`, `GlanceAppWidgetManager`, `provideContent`, `Box`, `Column`, `Row`, `Text`, `Image`, `ImageProvider`, `CircularProgressIndicator`, `clickable`, `actionStartActivity`, `cornerRadius`, `appWidgetBackground`, `LocalSize`, `LocalContext`, `updateAll`, `SizeMode` |
| `io.coil-kt.coil3:coil-bom` `3.1.0` | `gradle/libs.versions.toml:45` | `ImageRequest`, `imageLoader.executeBlocking`, `Precision`, `Scale`, `CachePolicy`, `RoundedCornersTransformation`, `asDrawable` |
| `androidx.compose.ui.unit` | Compose | `Dp`, `DpSize`, `dp`, `sp` |
| `androidx.core.graphics.drawable.toBitmap` | AndroidX core | Drawable to Bitmap |
| `kotlinx.collections.immutable` | `kotlinx.immutables` | `ImmutableList`, `toImmutableList` |
| `uy.kohesive.injekt` | `libs.injekt` | DI of `Application`, interactors, `SecurityPreferences` |
| `logcat` | `tachiyomi.core.common.util.system.logcat` | error log in managers |
| `ZonedDateTime`, `Instant` | `java.time` | `DateLimit` |

## 11. Desktop data design (PROPOSAL)

### 11.1 Feed JSON (`kurobayomi --updates-json`)

```json
{
  "schema": 1,
  "generated_at_ms": 1791417600000,
  "kind": "manga",
  "locked": false,
  "grid": { "rows": 2, "cols": 4, "cap": 8 },
  "window_after_ms": 1783478400000,
  "items": [
    {
      "entry_id": 1234,
      "kind": "manga",
      "title": "Example Title",
      "unread_update_id": 98765,
      "cover_path": "/home/USER/.cache/kurobayomi/covers/9f2c...webp"
    }
  ]
}
```

Rules:

- `schema` = 1 for this design.
- `kind` = `"manga"` or `"anime"`.
- `locked` = `true` when `use_biometric_lock` is on (see 11.3). When locked, `items` is `[]` and `title`/`cover_path` never appear.
- `grid.cap` = `rows * cols`, with rows and cols from the request (1..10 each).
- `title` is included only for tooltips and screen readers (the Android widget had none, see the UI file).
- `cover_path` is `null` when no cached thumbnail exists (placeholder shown).
- `unread_update_id` = `chapterId` (manga) or `episodeId` (anime).
- Times in epoch milliseconds, UTC.

Error envelope (non-zero exit):

```json
{ "schema": 1, "error": "database busy", "code": "db_busy" }
```

Exit codes: `0` ok (including locked and empty feeds), `2` usage error (bad args), `3` database unavailable, `4` preferences unreadable. Codes and messages are proposals; they are not Android values.

### 11.2 Cover thumbnails (PROPOSAL)

- Directory: `$XDG_CACHE_HOME/kurobayomi/widget-covers/` (default `~/.cache/kurobayomi/widget-covers/`), owned by area:34-images-covers-cache. File name = hex of SHA-256 of the source `thumbnail_url` + `"|" + lastModified`, plus `.webp`. The key includes `cover_last_modified` so a changed cover is re-rendered.
- Size: 58x87 logical px at scale 1, rendered at the device scale (default 2x: 116x174 px). Center-crop to aspect 58:87, then round the corners with radius 12 logical px (24 px at 2x) into the alpha channel. Baking the mask avoids a QML layer.
- Written only by the Kurobayomi process (the in-app cover loader) and read-only for the CLI.

### 11.3 Lock flag (PROPOSAL)

Read from the app's preference store (area:02-preferences-system) key `use_biometric_lock` (Boolean, default `false`), exactly as the Android widget does. No new key. The CLI never reads `lock_app_after`. The app-level unlock state is not visible to the CLI; this matches the Android behaviour (locked placeholder while the lock flag is on). A stricter variant (show data when unlocked in-process) would need a shared unlock marker and is recorded as an open question in the rust design.

### 11.4 Applet configuration

None. Two applet ids (`org.kurobayomi.updates.manga`, `org.kurobayomi.updates.anime`) replace the four Android providers. No stored settings. Size comes from the Plasma layout.

## 12. Reference: what Android stores that the desktop does not need

- `GlanceId` to instance mapping (managed by Glance).
- `AppWidgetProviderInfo` XML (replaced by `metadata.json`, see the rust design).
- Samsung sub-screen registration (no equivalent).
