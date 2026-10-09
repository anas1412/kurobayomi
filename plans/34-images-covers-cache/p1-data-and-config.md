# Area 34 — Images, covers and cache: data models, preferences, files, endpoints

Companion to `p1-behavior.md` (section numbers `§N` refer to it) and `p1-rust-design.md`. All Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`.

---

## 1. Domain entry models (inputs to the image pipeline)

OBSERVED. Only the fields the image code reads are listed.

### 1.1 `tachiyomi.domain.entries.manga.model.Manga`
File: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`.
| Field | Type | Nullable | Used by | Notes |
|---|---|---|---|---|
| `id` | `Long` | no | custom cover file name, keys | DB primary key |
| `source` | `Long` | no | source client lookup (`sourceManager.get(data.source)`) | `0` means local source |
| `favorite` | `Boolean` | no | `isLibraryManga` (library cover file cache) | |
| `title` | `String` | no | save names (`Pictures` folder, file name) | |
| `thumbnailUrl` | `String?` (line 29) | yes | fetch URL, cover file name, keyer | null means no cover |
| `coverLastModified` | `Long` (line 21) | no | keyer | epoch milliseconds, `0L` for new entries (line 117) |

### 1.2 `tachiyomi.domain.entries.anime.model.Anime`
File: `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`.
| Field | Type | Nullable | Notes |
|---|---|---|---|
| `id` | `Long` | no | custom cover and background file names |
| `source` | `Long` | no | source lookup |
| `favorite` | `Boolean` | no | `isLibraryAnime` |
| `title` | `String` | no | save folder |
| `thumbnailUrl` | `String?` (line 35) | yes | cover URL |
| `backgroundUrl` | `String?` (line 36) | yes | background URL |
| `coverLastModified` | `Long` (line 26) | no | keyer |
| `backgroundLastModified` | `Long` (line 27) | no | keyer |

### 1.3 `tachiyomi.domain.entries.manga.model.MangaCover`
File: `domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaCover.kt`. `data class MangaCover(mangaId: Long, sourceId: Long, isMangaFavorite: Boolean, url: String?, lastModified: Long) : EntryCover`. Projection: `Manga.asMangaCover()` maps `id`, `source`, `favorite`, `thumbnailUrl`, `coverLastModified`.
Doc comment in source: "Contains the required data for MangaCoverFetcher".

### 1.4 `tachiyomi.domain.entries.anime.model.AnimeCover`
File: `domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeCover.kt:8-14`. `data class AnimeCover(animeId: Long, sourceId: Long, isAnimeFavorite: Boolean, url: String?, lastModified: Long) : EntryCover`. Projection `Anime.asAnimeCover()` at line 16.

### 1.5 Update objects
- `MangaUpdate(id, coverLastModified: Long?)` (written by `UpdateManga.awaitUpdateCoverLastModified`).
- `AnimeUpdate(id, coverLastModified: Long? = null, backgroundLastModified: Long? = null)` at `domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt:17-18`.

## 2. Source page model (`eu.kanade.tachiyomi.source.model.Page`)

File: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt:11-16`. `@Serializable open class Page(val index: Int, val url: String = "", var imageUrl: String? = null, @Transient var uri: Uri? = null) : ProgressListener`. `number` = `index + 1` (computed, not serialized). Status flow is transient.

| Serialized field | Kotlin type | Default | Encoded when |
|---|---|---|---|
| `index` | `Int` | none (required) | always |
| `url` | `String` | `""` | only when not `""` (kotlinx `encodeDefaults = false`) |
| `imageUrl` | `String?` | `null` | only when non-null (`explicitNulls = false`) |
| `uri` | `Uri?` | `null` | never (`@Transient`) |

## 3. Image pipeline types (`data/coil` and `data/saver`)

### 3.1 `ImageType` (`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:106-114`)
| Enum | MIME | Extension |
|---|---|---|
| `AVIF` | `image/avif` | `avif` |
| `GIF` | `image/gif` | `gif` |
| `HEIF` | `image/heif` | `heif` |
| `JPEG` | `image/jpeg` | `jpg` |
| `JXL` | `image/jxl` | `jxl` |
| `PNG` | `image/png` | `png` |
| `WEBP` | `image/webp` | `webp` |
The native `Format` values mapped into it: `Avif, Gif, Heif, Jpeg, Jxl, Png, Webp` (`else -> null`).

### 3.2 `Image` (`app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt:153-182`)
- `sealed class Image(open val name: String, open val location: Location)`.
- `data class Cover(val bitmap: Bitmap, override val name: String, override val location: Location)`.
- `data class Page(val inputStream: () -> InputStream, override val name: String, override val location: Location)`.
- `val data: () -> InputStream`: Cover re-encodes to JPEG quality 100; Page returns `inputStream`.

### 3.3 `Location` (`ImageSaver.kt:184-214`)
- `sealed interface Location`.
- `data class Pictures(val relativePath: String)` (`@ConsistentCopyVisibility`); companion `create(relativePath: String = "")`.
- `data object Cache`.
- `fun directory(context): File` as in behavior §21.3.

### 3.4 `SaveImageResult` and reader events
Referenced by `ReaderViewModel` (`Event.SavedImage(SaveImageResult.Success(uri))` / `Error(e)`), `PlayerViewModel` (`Event.SavedImage`, `Event.ShareImage(uri, seconds)`), `ReaderViewModel` (`Event.CopyImage(uri)`, `Event.ShareImage(uri, page)`). Exact sealed hierarchies are owned by area:43-reader-core and area:45-player-core.

### 3.5 `DataSource` values used
LIBRARY (Coil): `MEMORY`, `DISK`, `NETWORK`. Used as: `fileLoader` → DISK; `BufferedSourceFetcher` → MEMORY; cache snapshot → DISK; network stream → NETWORK, or DISK when `response.cacheResponse != null`.

### 3.6 `ImageRequest` extras (see behavior §3)
| Extra key object | Default | Set by |
|---|---|---|
| `USE_CUSTOM_COVER_KEY` (`Extras.Key(true)`) | `true` | nobody sets it false in this repository |
| `cropBordersKey` (`Extras.Key(default = false)`) | `false` | `ReaderPageImageView` (`config.cropBorders`) |
| `customDecoderKey` (`Extras.Key(default = false)`) | `false` | `ReaderPageImageView` (`true`) |
| `useBackgroundKey` (`Extras.Key(default = false)`) | `false` | `AnimeImagesDialog` (`page == 1`), `AnimeInfoHeader` (`true`) |

## 4. On-disk layout

### 4.1 Android (source of truth)
| Path (relative to app data root) | Content | Created by | Deleted by | Survives cache clear |
|---|---|---|---|---|
| `getExternalFilesDir("covers")` or `filesDir/covers` | library manga covers, file name `md5(thumbnailUrl)` | `MangaCoverCache.getCacheDir` | `deleteFromCache`, `removeCovers`, `prepUpdateCover` | yes (not in cache dir) |
| `covers/custom/` | custom manga covers, file name `md5(mangaId as decimal string)` | same | `deleteCustomCover`, `removeCovers(true)` | yes |
| `animecovers/` | library anime covers, `md5(thumbnailUrl)` | `AnimeCoverCache` | same pattern | yes |
| `animecovers/custom/` | custom anime covers, `md5(animeId)` | same | same | yes |
| `animebackgrounds/` | anime backgrounds, `md5(backgroundUrl)` | `AnimeBackgroundCache` | `deleteFromCache` with flag | yes |
| `animebackgrounds/custom/` | custom anime backgrounds, `md5(animeId)` | same | `deleteCustomBackground` | yes |
| `cacheDir/chapter_disk_cache/` | DiskLruCache: `journal`, `<md5(url)>.0`, `<md5(mangaId+chapterUrl)>.0`, `*.tmp` | `ChapterCache` | `clear()`, auto-clear on launch, OS | no (cache) |
| `cacheDir/shared_image/` | last shared/copied image, one file | `ImageSaver` (Location.Cache) | `deleteRecursively` before each reader/player share | no (cache) |
| `cacheDir/image_cache/` (LIBRARY default) | Coil disk cache for network images | Coil | Coil LRU | no (cache) |
| `<Pictures>/Aniyomi/<relativePath>/<name>.<ext>` | saved pages, frames, covers | `ImageSaver` | user | n/a |
| `<folder>/.nomedia` | empty marker in image folders | `DiskUtil.createNoMediaFile` | never | n/a |

### 4.2 Linux target (proposal; mapping in design §3)
| Android path | Linux path (under `Paths`) | Rule |
|---|---|---|
| `covers`, `covers/custom` | `<data>/covers`, `<data>/covers/custom` | same names (persistent) |
| `animecovers`, `animecovers/custom` | `<data>/animecovers`, `<data>/animecovers/custom` | same |
| `animebackgrounds`, `animebackgrounds/custom` | `<data>/animebackgrounds`, `<data>/animebackgrounds/custom` | same |
| `cacheDir/chapter_disk_cache` | `<cache>/chapter_images` | own format (design §6), same semantics |
| `cacheDir/shared_image` | `<cache>/shared_image` | same |
| `cacheDir/image_cache` | `<cache>/images` | own format (design §6.2) |
| `Pictures/Aniyomi/...` | `<xdg pictures>/Kurobayomi/...` | same relative layout |
| `.nomedia` | written only into library image folders that the user chooses to mark (design §8.4) | same file name |

`<data>` = `Paths::data_dir()` (XDG_DATA_HOME/kurobayomi), `<cache>` = `Paths::cache_dir()` (XDG_CACHE_HOME/kurobayomi), `<xdg pictures>` = `glib::user_special_dir(UserDirectory::Pictures)` through `Paths::pictures_dir()`.

### 4.3 File-name rules
| Name | Rule | Source |
|---|---|---|
| Cache key file (cover) | 32 lowercase hex characters = `md5(utf8(key))` | `DiskUtil.kt:32-34`, `Hash.kt` |
| Custom cover file | `md5(utf8(decimal(id)))` | `MangaCoverCache.kt:50-52` |
| Page-list key | `md5(utf8(decimal(mangaId) + chapter.url))` | `ChapterCache.kt:197-199` |
| Page-image entry | `md5(utf8(imageUrl)) + ".0"` | `ChapterCache.kt:130-134` |
| Saved image name | `buildValidFilename(name + "." + ext)` | `ImageSaver.kt:39` |
| Reader page name | see behavior §22.3 | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:754-766` |
| Player frame name | see behavior §22.4 | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2013-2025` |

## 5. Database columns touched

| Table | Column | Type | Null | Default at insert | Written by | Source |
|---|---|---|---|---|---|---|
| `mangas` | `thumbnail_url` | TEXT | yes | from source | source refresh | `data/src/main/sqldelight/data/mangas.sq` |
| `mangas` | `cover_last_modified` | INTEGER | NOT NULL | `0` for new rows (`Manga.kt:117`) | `awaitUpdateCoverLastModified` | `mangas.sq:23`, update at `mangas.sq:173` (`coalesce(:coverLastModified, cover_last_modified)`) |
| `mangas` | `favorite` | INTEGER | — | — | favourite toggle | used by `isLibraryManga` |
| `animes` | `thumbnail_url` | TEXT | yes | from source | source refresh | `data/src/main/sqldelightanime/dataanime/animes.sq` |
| `animes` | `background_url` | TEXT | yes | from source | source refresh | `animes.sq` (insert list line 167) |
| `animes` | `cover_last_modified` | INTEGER | NOT NULL | — | `awaitUpdateCoverLastModified` | `animes.sq:25` |
| `animes` | `background_last_modified` | INTEGER | NOT NULL | — | `awaitUpdateBackgroundLastModified` | `animes.sq:39` |
| `animes` | `favorite` | INTEGER | — | — | favourite toggle | — |
The `historyView.sq`, `updatesView.sq` views also expose `mangas.cover_last_modified` (lines 10, 38, 57 and line 14) so the history and updates screens get the same keyer input.

## 6. Serialized formats

### 6.1 Image type sniffing table
Input: first 32 bytes of the stream (§behavior 17.2). Output: one of the seven `ImageType` values or none. The table of magic numbers lives in the native library (O-3); the Rust port uses the rules in design §4.4.

### 6.2 Page-list JSON (chapter cache value)
Produced by `json.encodeToString(pages)` where `json = Json { ignoreUnknownKeys = true; explicitNulls = false }` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`; `encodeDefaults` is the kotlinx default `false`).
Encoding example for two pages, where the first has both URLs and the second has neither (`url` is `""` and `imageUrl` is null, so both are omitted):
```json
[{"index":0,"url":"https://example.invalid/p/0","imageUrl":"https://example.invalid/img/0.jpg"},{"index":1}]
```
The URLs are placeholders for illustration. The second object shows the omitted-default rule.
Reading: unknown keys ignored; missing `url` becomes `""`; missing `imageUrl` becomes null; missing `index` is an error (the entry is then treated as a miss).

### 6.3 Keyer key formats
| Keyer | Format | Source |
|---|---|---|
| `MangaKeyer` (custom) | `manga;<id>;<coverLastModified>` | `MangaCoverKeyer.kt:14-16` |
| `MangaKeyer` (thumbnail) | `manga;<thumbnailUrl or "null">;<coverLastModified>` | `MangaCoverKeyer.kt:17` |
| `AnimeKeyer` background custom | `anime;<id>;<backgroundLastModified>` | `AnimeCoverKeyer.kt:16` |
| `AnimeKeyer` background | `anime;<backgroundUrl or "null">;<backgroundLastModified>` | `AnimeCoverKeyer.kt:17` |
| `AnimeKeyer` cover custom | `anime;<id>;<coverLastModified>` | `AnimeCoverKeyer.kt:18` |
| `AnimeKeyer` cover | `anime;<thumbnailUrl or "null">;<coverLastModified>` | `AnimeCoverKeyer.kt:19` |
| `MangaCoverKeyer` (custom) | `manga;<mangaId>;<lastModified>` | `MangaCoverKeyer.kt:27` |
| `MangaCoverKeyer` (url) | `manga;<url or "null">;<lastModified>` | `MangaCoverKeyer.kt:29` |
| `AnimeCoverKeyer` (custom) | `anime;<animeId>;<lastModified>` | `AnimeCoverKeyer.kt:29` |
| `AnimeCoverKeyer` (url) | `anime;<url or "null">;<lastModified>` | `AnimeCoverKeyer.kt:31` |
Timestamps are decimal epoch milliseconds (Long, no padding). Null IDs cannot occur for `Long` ids; null URLs render as `null`.

### 6.4 Backup and export formats
None. Covers and images are not part of the backup format in this area (area:21-backup-format-and-create/area:22-backup-restore own backups; custom covers are not exported by this code).

## 7. Request and option shapes (Rust-side equivalents)

### 7.1 `LoadOptions`
| Field | Type | Default | Maps from |
|---|---|---|---|
| `use_custom_cover` | bool | true | `USE_CUSTOM_COVER_KEY` |
| `use_background` | bool | false | `useBackgroundKey` |
| `crop_borders` | bool | false | `cropBordersKey` |
| `custom_decoder` | bool | false | `customDecoderKey` |
| `memory_cache` | `CachePolicy` | `Enabled` | `ImageRequest.memoryCachePolicy` |
| `disk_cache` | `CachePolicy` | `Enabled` | `diskCachePolicy` |
| `network_cache` | `CachePolicy` | `Enabled` | `networkCachePolicy` |
| `size` | `RequestSize` (`Original` or `Pixels(w, h)`) | `Pixels(0, 0)` (undefined) | `ImageRequest.size` |
| `scale` | `Scale` (`Fit`, `Fill`) | `Fit` | `ImageRequest.scale` (LIBRARY default) |
Note on `size`: in Kotlin the size comes from the target view (for list covers) or is set to `Size.ORIGINAL` (viewers, save and share). In Rust, list covers pass explicit pixel sizes from the widget (design §5.2); `Pixels(0, 0)` means "undefined" and maps to the sentinel rule of behavior §15.2 (sample size 1).

### 7.2 `CachePolicy`
Values `Enabled`, `ReadOnly`, `WriteOnly`, `Disabled`. `read_enabled() = matches!(self, Enabled | ReadOnly)`; `write_enabled() = matches!(self, Enabled | WriteOnly)`.

### 7.3 `Scale`
`Fit` maps sentinel `i32::MAX` for undefined dimensions; `Fill` maps `i32::MIN`.

## 8. Preferences used by this area

Keys as written in source; the store is `PreferenceStore` (area:02-preferences-system). "Default" is the value passed to the getter.

| Key | Type | Default | Read by | Source | Kurobayomi |
|---|---|---|---|---|---|
| `verbose_logging` | Boolean | constructor-supplied (`core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:11-13`) | Coil logger, App start | `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:12` | keep, debug log level |
| `pref_hardware_bitmap_threshold` | Int | `GLUtil.SAFE_TEXTURE_LIMIT` = 2048 | `ImageUtil.hardwareBitmapThreshold` (set elsewhere) | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:40` | drop (T-34a-034) |
| `pref_display_profile_key` | String | `""` | `ReaderActivity.setDisplayProfile` | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:38` | keep as P2 (T-34a-031) |
| `auto_clear_chapter_cache` | Boolean | `false` | `MainActivity` launch clear | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:93` | keep (T-34a-039) |
| `create_folder_per_manga` | Boolean | `false` | reader save folder | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:81` | keep (T-34a-044) |
| `crop_borders` | Boolean | `false` | reader decode option | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:69` | keep (T-34a-030) |
| `cutout_short` | Boolean | `true` | reader window (not this area) | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:34` | out of scope |
| `secure_screen_v2` | enum name (`ALWAYS`, `INCOGNITO`, `NEVER`) | `INCOGNITO` | secure screen | `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:17` | keep stored, no effect on Linux (T-34a-064) |
| `incognito_mode` (app state key) | Boolean | `false` | secure screen | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21` (`Preference.appStateKey("incognito_mode")`) | keep |
| `use_biometric_lock` | Boolean | `false` | secure delegate | `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:13` | out of scope (area:36-ui-shell-navigation-security-webview) |
| `lock_app_after` | Int | `0` | secure delegate | `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:15` | out of scope (area:36-ui-shell-navigation-security-webview) |
| `bottom_rail_nav_style` | enum (`NavStyle`) | `MOVE_HISTORY_TO_MORE` | `Tab.currentNavigationStyle` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:41` | out of scope (area:36-ui-shell-navigation-security-webview) |
| `animatorDurationScale` (system) | Float | system value | crossfade | `app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt:10` | `gtk-enable-animations` |

Deleted or never-written keys: none in this area.

### 8.1 `pref_hardware_bitmap_threshold`
Int, default `GLUtil.SAFE_TEXTURE_LIMIT` (2048). Android only. Rust does not read it; the key is not migrated (behavior §16.3, task T-34a-034).

### 8.2 `auto_clear_chapter_cache`
Boolean, default `false` (`LibraryPreferences.kt:93`). Clears the chapter cache on app launch (behavior §20.9).

### 8.3 Chapter cache constants
`PARAMETER_APP_VERSION = 1`, `PARAMETER_VALUE_COUNT = 1`, `PARAMETER_CACHE_SIZE = 100 * 1024 * 1024` bytes (`ChapterCache.kt:203-209`). Rust: size limit 104857600 bytes, LRU; no version or value-count constants (the file layout is its own, design §6.3).

### 8.4 `secure_screen_v2`
Enum name stored as string, values `ALWAYS`, `INCOGNITO`, `NEVER`, default `INCOGNITO` (`SecurityPreferences.kt:17, 30-34`). Linux: stored and shown, no effect (behavior §30.1, task T-34a-064).

## 9. Network endpoints and response shapes

### 9.1 Cover and background fetch
- Method: `GET`.
- URL: the stored `thumbnail_url` (manga and anime) or `background_url` (anime background). Values are whatever the source returned; the app does not rewrite them.
- Headers: every header of the source's `HttpSource.headers` (for example `User-Agent`, `Referer`, or source-specific headers). None are added by the image code.
- Cache-Control request header: `no-store` when `network_cache` is enabled (behavior §7.2); `no-cache, only-if-cached` otherwise.
- Accepted status: 2xx, and `304 Not Modified` (accepted without error, the response body is then empty).
- Failure: any other status → `IOException(response.message)`.
- Body: the raw image bytes (no content-type check; `mimeType = "image/*"`).
- Client: the source's own OkHttp client, else the shared app client.
- Timeouts: those of the shared client (area:12-network-stack).

### 9.2 Local and URI resources
- `file://<path>` or absolute `/path`: local file read, no network.
- `content://...` (Android Storage Access Framework): not supported on Linux; a value starting with `content` is classified as URI and fails with `Invalid image` in Kurobayomi (behavior §5.2).
- `Custom-...` values: classified as HTTP (`Type.URL`). No producer exists in this repository.

### 9.3 No other endpoints
This area adds no API calls. Save and share use no network. Notifications and sharing are local.

## 10. UI tokens (presentation)

| Token | Value | Source |
|---|---|---|
| Cover ratio `Square` | 1 : 1 | `ItemCover.kt` |
| Cover ratio `Book` | 2 : 3 | `ItemCover.kt` |
| Cover ratio `Thumb` | 16 : 9 | `ItemCover.kt` |
| Cover corner shape | `MaterialTheme.shapes.extraSmall` | `ItemCover.kt` default parameter |
| Cover placeholder colour | `#1F888888` (ARGB) | `ItemCover.kt` (`CoverPlaceholderColor`) |
| Cover content scale | Crop | `ItemCover.kt` |
| Viewer pill shape | `MaterialTheme.shapes.extraLarge` | `AnimeImagesDialog.kt` `ActionsPill` |
| Viewer pill background | `colorScheme.background` at alpha 0.95 | `AnimeImagesDialog.kt` |
| Viewer pill padding | 4 dp (row) | `AnimeImagesDialog.kt` |
| Viewer zoom duration | 500 ms | `AnimeImagesDialog.kt`, `MangaCoverDialog.kt` (`ReaderPageImageView.Config(zoomDuration = 500)`) |
| Viewer action offset (menu) | `DpOffset(8.dp, 0.dp)` | both dialogs |
| Crossfade | `300 * animatorDurationScale` ms | `App.kt:203` |
| Snackbar | `withDismissAction = true` | cover screen models |
| Notification big picture bound | 720 x 1280 | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt` |
| Error drawable | `R.drawable.cover_error` (vector) | `app/src/main/res/drawable/cover_error.xml` |
| Save icon set | `Icons.Outlined.Share`, `Icons.Outlined.Save`, `Icons.Outlined.Edit`, `Icons.Outlined.Close`, `Icons.AutoMirrored.Outlined.KeyboardArrowRight/Left` | dialogs |

Mockup token check (Kurobayomi): `src/ui.rs:176-182` draws `cover(w, h)` as a `gtk::Frame` with an `image-x-generic-symbolic` icon. The spec file `spec/entries.json` has screens `full_cover` (id at line 235) and `full_images` (line 849) with the group items Close, Share (and more). The ratio tokens above must be applied to the frame instead of the fixed `(w, h)` pairs used in `src/entry.rs:67` (`w * 10 / 7`, i.e. 1:1.43) and `src/entry.rs:91` (`cover(130, 190)`).

## 11. Mapping from Kotlin types to Rust types

| Kotlin | Rust (proposal) |
|---|---|
| `Long` ids | `i64` |
| `Long` epoch millis | `i64` (`chrono::DateTime::timestamp_millis` semantics; no `Instant` dependency needed) |
| `String?` | `Option<String>` |
| `ByteArray` | `bytes::Bytes` (shared) or `Vec<u8>` |
| `File` | `std::path::PathBuf` |
| `Bitmap` (ARGB_8888) | `gdk::MemoryTexture` (RGBA8) built on the GTK thread |
| `InputStream` | `tokio::io::AsyncRead` or `std::io::Read` on a blocking task |
| `Uri` | `url::Url` for http, `PathBuf` for file |
| `Json` (kotlinx) | `serde_json` with `#[serde(default, skip_serializing_if = ...)]` |
| `DiskLruCache` | own struct over `rusqlite` index + files (design §6.3) |
| sealed `Image` | `enum ImageSaveSource { Cover(RgbaImage), Page(Bytes) }` |
| `enum class EditCoverAction` | `enum EditImageAction { Edit, Delete }` |
| `enum class ImageType` | `enum ImageFormat { Avif, Gif, Heif, Jpeg, Jxl, Png, Webp }` with `mime()` and `extension()` |
