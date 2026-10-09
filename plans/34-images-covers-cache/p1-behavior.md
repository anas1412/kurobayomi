# Area 34 — Images, covers and cache: behavior specification

Scope: the 23 files listed in `p1-inventory.md`. All Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`. Labels: **OBSERVED** = what the Kotlin does; **PROPOSAL** = what Kurobayomi should do (always prefixed `Proposal:`); **LIBRARY** = behavior of Coil 3.1.0 or another dependency, taken from the library source fetched at tag `3.1.0` (not from this repository).

Dependency versions (OBSERVED): `gradle/libs.versions.toml:45-47` Coil `io.coil-kt.coil3:coil-bom` 3.1.0; `gradle/libs.versions.toml:33` `com.jakewharton:disklrucache:2.0.2`; `gradle/libs.versions.toml:52` image decoder `com.github.tachiyomiorg:image-decoder:41c059e540` (native decoder artifact; its source is not in this repository, see open question O-3).

---

## 1. Pipeline overview

OBSERVED. An image is shown through Coil's `ImageRequest`. The request carries one data object (`Manga`, `MangaCover`, `Anime`, `AnimeCover`, `BufferedSource`, `Uri`, or a bitmap). Coil resolves a **Keyer** to get a cache key, a **Fetcher** to get bytes or a file, and a **Decoder** to get a bitmap. The app registers all of these in one `ImageLoader` (`app/src/main/java/eu/kanade/tachiyomi/App.kt`, `newImageLoader`). Cover art has two extra layers owned by the app: a per-entry **cover file cache** (`app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt`, `AnimeCoverCache.kt`, `AnimeBackgroundCache.kt`) for library entries, and a **page cache** for reader and player pages (`app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt`).

Sequence for a cover in a list:
1. The composable calls `AsyncImage(model = manga)` (`app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt`).
2. Coil asks the keyer for `manga;<url>;<coverLastModified>` (§10).
3. Coil asks the factory to create a fetcher. `MangaCoverFetcher.MangaFactory` wraps the manga in a fetcher (§4).
4. The fetcher checks the custom cover, then classifies the URL and loads it (§5-§7).
5. Coil decodes with `TachiyomiImageDecoder` or its platform decoder (§13-§15).
6. The composable draws the result with a fade (`crossfade`, §2.4).

## 2. ImageLoader configuration

### 2.1 Component registration order
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/App.kt`:
- Line 186-200 (`components { ... }`), in this order:
  1. `OkHttpNetworkFetcherFactory(callFactoryLazy::value)` — network fetcher using the shared OkHttp client (`Injekt.get<NetworkHelper>().client`).
  2. `TachiyomiImageDecoder.Factory()` (line 189).
  3. `BufferedSourceFetcher.Factory()` (line 191).
  4. `MangaCoverFetcher.MangaFactory(callFactoryLazy)` (line 192).
  5. `MangaCoverFetcher.MangaCoverFactory(callFactoryLazy)` (line 193).
  6. `AnimeImageFetcher.AnimeFactory(callFactoryLazy)` (line 194).
  7. `AnimeImageFetcher.AnimeCoverFactory(callFactoryLazy)` (line 195).
  8. `AnimeKeyer()` (line 197), `MangaKeyer()` (line 198), `AnimeCoverKeyer()` (line 199), `MangaCoverKeyer()` (line 200).
- `val callFactoryLazy = lazy { Injekt.get<NetworkHelper>().client }` (line 187).

### 2.2 Other builder options
OBSERVED:
- `allowRgb565(DeviceUtil.isLowRamDevice(this@App))` (App.kt, same builder). `DeviceUtil.isLowRamDevice` returns `totalMem < 3L * 1024 * 1024 * 1024` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt:95-100`).
- `if (networkPreferences.verboseLogging().get()) logger(DebugLogger())`.
- `fetcherCoroutineContext(Dispatchers.IO.limitedParallelism(8))` and `decoderCoroutineContext(Dispatchers.IO.limitedParallelism(3))` (App.kt, after the `crossfade` call).

### 2.3 Concurrency limits
OBSERVED: at most 8 concurrent fetches, at most 3 concurrent decodes. The comment says "Coil spawns a new thread for every image load by default".

### 2.4 Crossfade
OBSERVED: `crossfade((300 * this@App.animatorDurationScale).toInt())` (App.kt:203). `animatorDurationScale` is a `Context` extension (`app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt:10`) reading the system animator duration scale. A value of 0 disables animation.
PROPOSAL: fade 300 ms when `gtk-enable-animations` is true (read from `gtk::Settings`), else 0 ms. Re-read the setting on each image load.

### 2.5 Verbose logging
OBSERVED: `NetworkPreferences.verboseLogging()` (`core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:11-13`, key `verbose_logging`) turns on Coil's `DebugLogger`. Also `LogcatLogger` at app start (App.kt).
PROPOSAL: `verbose_logging` sets the `kuro_images` tracing target to debug.

## 3. Request options (extras)

### 3.1 Defined extras
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt`:
| Extra | Builder function | Options getter | Default |
|---|---|---|---|
| `cropBordersKey` | `ImageRequest.Builder.cropBorders(enable)` (line 28-30) | `Options.cropBorders` (line 32-33) | `false` (line 35) |
| `customDecoderKey` | `ImageRequest.Builder.customDecoder(enable)` (line 37-39) | `Options.customDecoder` (41-42) | `false` (line 44) |
| `useBackgroundKey` | `ImageRequest.Builder.useBackground(enable)` (line 46-48) | `Options.useBackground` (50-51) | `false` (line 53) |
| `USE_CUSTOM_COVER_KEY` | none (read with `getOrDefault`) | `options.extras.getOrDefault(USE_CUSTOM_COVER_KEY)` | `true` (`MangaCoverFetcher.kt:343`, `AnimeImageFetcher.kt:360`) |

Only `USE_CUSTOM_COVER_KEY` defaults to true. Setting the key false is never done in this repository (grep for `USE_CUSTOM_COVER_KEY` shows reads only).

### 3.2 Request options used by callers
OBSERVED:
- `ReaderPageImageView` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt:333-345`) sends `.cropBorders(config.cropBorders).customDecoder(true)` with `.memoryCachePolicy(DISABLED)`, `.diskCachePolicy(DISABLED)`, `.precision(INEXACT)`, `.crossfade(false)`, and `ViewSizeResolver`, for webtoon pages.
- `AnimeImagesDialog` (`app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt:207-212`) sends `.useBackground(page == 1)`, `.size(Size.ORIGINAL)`, `.memoryCachePolicy(CachePolicy.DISABLED)`.
- `AnimeInfoHeader` (`app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt:126`) sends `.useBackground(true)` for the header backdrop.
- `MangaCoverDialog` (`app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt:~170-180`) sends `.size(Size.ORIGINAL).memoryCachePolicy(DISABLED)`.
- Save and share requests (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt`): `.size(Size.ORIGINAL)` only.
- `SaveImageNotifier.onComplete` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt`): `.data(uri).memoryCachePolicy(DISABLED).size(720, 1280)`.

### 3.3 Coil cache policies
LIBRARY: `CachePolicy` values are `ENABLED`, `READ_ONLY`, `WRITE_ONLY`, `DISABLED`. Default for every request is `ENABLED`. `readEnabled` is true for ENABLED and READ_ONLY; `writeEnabled` for ENABLED and WRITE_ONLY.

## 4. Fetcher factories (which data type goes where)

### 4.1 Manga cover factories
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt`:
- `MangaFactory : Fetcher.Factory<Manga>` (lines 298-318). `create` builds:
  - `url = data.thumbnailUrl`
  - `isLibraryManga = data.favorite`
  - `coverFileLazy = lazy { coverCache.getCoverFile(data.thumbnailUrl) }`
  - `customCoverFileLazy = lazy { coverCache.getCustomCoverFile(data.id) }`
  - `diskCacheKeyLazy = lazy { imageLoader.components.key(data, options)!! }`
  - `sourceLazy = lazy { sourceManager.get(data.source) as? HttpSource }`
- `MangaCoverFactory : Fetcher.Factory<MangaCover>` (lines 320-340): same, with `url = data.url`, `isLibraryManga = data.isMangaFavorite`, `customCoverFileLazy = getCustomCoverFile(data.mangaId)`, `sourceLazy = sourceManager.get(data.sourceId)`.
- `MangaFactory` and `MangaCoverFactory` ignore `useBackground`.

### 4.2 Anime image factories
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt`:
- `AnimeFactory : Fetcher.Factory<Anime>` (lines 299-335). `isBackground = options.useBackground`; `url = if (isBackground) data.backgroundUrl else data.thumbnailUrl` (line 309). When background: `coverCacheLazy = backgroundCache.getBackgroundFile(url)` and `customCoverCacheLazy = backgroundCache.getCustomBackgroundFile(data.id)`. Otherwise cover caches. `isLibraryAnime = data.favorite`.
- `AnimeCoverFactory : Fetcher.Factory<AnimeCover>` (lines 337-357): `url = data.url`, `isLibraryAnime = data.isAnimeFavorite`, cover caches only.

### 4.3 Buffered-source fetcher
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/BufferedSourceFetcher.kt`: `fetch()` returns `SourceFetchResult(source = ImageSource(source = data, fileSystem = options.fileSystem), mimeType = null, dataSource = DataSource.MEMORY)`. It does not copy the buffer; the first reader consumes it.

## 5. Cover fetch orchestration

### 5.1 `fetch()` steps (manga; anime identical with background variants)
OBSERVED, `MangaCoverFetcher.kt:63-81`:
1. `useCustomCover = options.extras.getOrDefault(USE_CUSTOM_COVER_KEY)` (default true).
2. If `useCustomCover`: `customCoverFile = customCoverFileLazy.value`. If `customCoverFile.exists()` return `fileLoader(customCoverFile)` (a file result tagged `DataSource.DISK`).
3. If `url == null` → `error("No cover specified")` (throws `IllegalStateException`).
4. `when (getResourceType(url))`: `URL` → `httpLoader()`; `File` → `fileLoader(File(url.substringAfter("file://")))`; `URI` → `uniFileLoader(url)`; `null` → `error("Invalid image")`.

Note: step 2 means a favourite entry with a custom cover never touches the network or the thumbnail URL.

### 5.2 Resource classification
OBSERVED, `MangaCoverFetcher.kt:282-290` (anime identical at `AnimeImageFetcher.kt:283-291`):
```
cover.isNullOrEmpty()                                  -> null   (invalid)
cover.startsWith("http", true) || cover.startsWith("Custom-", true) -> URL
cover.startsWith("/") || cover.startsWith("file://")   -> File
cover.startsWith("content")                            -> URI
else                                                   -> null   (invalid)
```
Checks are in this order; `startsWith(..., true)` is case-insensitive. The `Custom-` prefix is treated as an HTTP URL; no code in the repository produces that prefix (the only occurrences are the two classification lines), so it is a leftover.

## 6. HTTP loader (`httpLoader`)

### 6.1 Library branch selection
OBSERVED, `MangaCoverFetcher.kt:105-115`:
- `libraryCoverCacheFile = if (isLibraryManga) coverFileLazy.value ?: error("No cover specified") else null`.
- Non-library entries have no cover-file cache; they use Coil's disk cache only.

### 6.2 Read order
OBSERVED, `MangaCoverFetcher.kt:112-132`:
1. If `libraryCoverCacheFile` exists and `options.diskCachePolicy.readEnabled` → `fileLoader(libraryCoverCacheFile)`; return.
2. `snapshot = readFromDiskCache()` (§9.1; null when read disabled or no disk cache).
3. If snapshot is not null:
   a. `snapshotCoverCache = moveSnapshotToCoverCache(snapshot, libraryCoverCacheFile)` (§8.2). If non-null → `fileLoader(snapshotCoverCache)`; return.
   b. Otherwise return `SourceFetchResult(snapshot.toImageSource(), "image/*", DataSource.DISK)`.
4. Network fetch (§7): `response = executeNetworkRequest()`; `responseBody = checkNotNull(response.body) { "Null response source" }`.
5. In an inner try:
   a. `responseCoverCache = writeResponseToCoverCache(response, libraryCoverCacheFile)` (§8.1). If non-null → `fileLoader(responseCoverCache)`; return.
   b. `snapshot = writeToDiskCache(response)` (§9.1). If non-null → `SourceFetchResult(snapshot.toImageSource(), "image/*", DataSource.NETWORK)`; return.
   c. Otherwise return `SourceFetchResult(ImageSource(responseBody.source(), SYSTEM), "image/*", dataSource = if (response.cacheResponse != null) DISK else NETWORK)`.
   On exception in the inner try: `responseBody.close()` and rethrow.
6. Outer catch: `snapshot?.close()` and rethrow.

### 6.3 Snapshot promotion
OBSERVED: `moveSnapshotToCoverCache` copies the disk-cache data into the cover file and removes the disk-cache key (`imageLoader.diskCache?.run { ...; remove(diskCacheKey) }`), so a library entry's image ends up in exactly one place.

## 7. Network request

### 7.1 Client selection
OBSERVED, `MangaCoverFetcher.kt:170-172`: `client = sourceLazy.value?.client ?: callFactoryLazy.value`. A source that is an `HttpSource` uses its own OkHttp client (with source interceptors); anything else (local sources, deleted sources) uses the shared network client.

### 7.2 Request construction
OBSERVED, `MangaCoverFetcher.kt:180-202`:
- `Request.Builder().url(url!!)`.
- If the source is an `HttpSource`, `headers(sourceHeaders)` copies every source header (`sourceLazy.value?.headers`).
- Cache control:
  - if `options.networkCachePolicy.readEnabled` → `CacheControl.Builder().noStore().build()` (constant `CACHE_CONTROL_NO_STORE`, companion line 345), meaning "do not use or store OkHttp's response cache".
  - otherwise → `CacheControl.Builder().noCache().onlyIfCached().build()` (line 346), commented "This causes the request to fail with a 504 Unsatisfiable Request." Effect: the request fails with HTTP 504 unless a cached response exists.

### 7.3 Response acceptance
OBSERVED, `MangaCoverFetcher.kt:173-176`: `if (!response.isSuccessful && response.code != HTTP_NOT_MODIFIED) { response.close(); throw IOException(response.message) }`. `HTTP_NOT_MODIFIED = 304` (line 348). The error message is the HTTP reason phrase only (for example `Not Found`); an empty reason phrase gives an empty message.

### 7.4 Await
OBSERVED: `client.newCall(newRequest()).await()` — the suspending OkHttp extension in `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:101`, which cancels the call when its coroutine is cancelled.

## 8. Cover-file writes

### 8.1 `writeResponseToCoverCache`
OBSERVED, `MangaCoverFetcher.kt:220-231`:
- If `cacheFile == null` or `!options.diskCachePolicy.writeEnabled` → return null (no write).
- `response.peekBody(Long.MAX_VALUE).source().use { writeSourceToCoverCache(input, cacheFile) }` — `peekBody` lets the body be read again from the same response.
- Returns `cacheFile.takeIf { it.exists() }`.
- On any `Exception`: logs (`logcat(LogPriority.ERROR, e) { "Failed to write response data to cover cache ${cacheFile.name}" }`) and returns null, so the caller falls through to the disk cache.

### 8.2 `moveSnapshotToCoverCache`
OBSERVED, `MangaCoverFetcher.kt:204-218`: null cache file → null. Otherwise `fileSystem.source(snapshot.data).use { writeSourceToCoverCache(it, cacheFile) }` then `remove(diskCacheKey)`. Returns `cacheFile.takeIf { exists }`. On exception logs `"Failed to write snapshot data to cover cache ${cacheFile.name}"` and returns null.

### 8.3 `writeSourceToCoverCache`
OBSERVED, `MangaCoverFetcher.kt:233-244`:
1. `cacheFile.parentFile?.mkdirs()`.
2. `cacheFile.delete()` (overwrite is delete-then-write).
3. `cacheFile.sink().buffer().use { output.writeAll(input) }`.
4. On exception: `cacheFile.delete()` (no partial file survives) and rethrow.

Note: this is not atomic. Another reader can open the file while it is being written. The fetcher only returns the file after the write finishes, so the reader in this app never sees a partial file except after a crash mid-write (edge E-IMG-4).

## 9. Coil disk cache (non-library entries and library first-load)

### 9.1 Read and write
OBSERVED, `MangaCoverFetcher.kt:246-271`:
- `readFromDiskCache()`: if `diskCachePolicy.readEnabled` → `imageLoader.diskCache?.openSnapshot(diskCacheKey)`; else null.
- `writeToDiskCache(response)`: `editor = diskCache?.openEditor(diskCacheKey) ?: return null`; then `diskCache.fileSystem.write(editor.data) { response.body.source().readAll(this) }`; return `editor.commitAndOpenSnapshot()`. On exception: `editor.abort()` (its own exceptions ignored) and rethrow. Note that `writeToDiskCache` does not check `writeEnabled`; the caller only reaches it through the network branch, so a WRITE_ONLY/ENABLED check is missing for the disk cache path (OBSERVED quirk; edge E-IMG-9).
- `DiskCache.Snapshot.toImageSource()` (line 273-280): `ImageSource(file = data, fileSystem = SYSTEM, diskCacheKey = diskCacheKey, closeable = this)`.

### 9.2 Library sizing (LIBRARY, `coil-core/src/commonMain/kotlin/coil3/disk/DiskCache.kt`, tag 3.1.0)
- `maxSizePercent = 0.02` (2 percent), `maximumMaxSizeBytes = 250 * 1024 * 1024`, `minimumMaxSizeBytes = 10 * 1024 * 1024`.
- `maxSize = (0.02 * fileSystem.remainingFreeSpaceBytes(directory)).coerceIn(10 MiB, 250 MiB)`; if reading free space throws, `maxSize = 10 MiB`.
- The directory is set by the app's image loader. Repository code does not set it (`App.kt` contains no `diskCache` call), so the library default applies; LIBRARY default directory is `<cacheDir>/image_cache` (see O-6).

### 9.3 Memory cache sizing (LIBRARY, `coil-core/src/androidMain/kotlin/coil3/util/contexts.kt`)
- Fraction: 0.2 normally, 0.15 when `ActivityManager.isLowRamDevice`.
- Total memory: `memoryClass` MiB (or `largeMemoryClass` when the app declares a large heap); default 256 MiB when the lookup throws.
- Memory cache max bytes = fraction * total. Not overridden by the repository.
PROPOSAL: a fixed 64 MiB decoded-texture budget (see design §6.1).

## 10. Keyers (cache keys)

### 10.1 `MangaKeyer` (domain `Manga`)
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt:12-20`:
```
if (data.hasCustomCover())  -> "manga;${data.id};${data.coverLastModified}"
else                        -> "manga;${data.thumbnailUrl};${data.coverLastModified}"
```
`hasCustomCover()` is `coverCache.getCustomCoverFile(id).exists()` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:88-90`). A null `thumbnailUrl` renders as the text `null`.

### 10.2 `AnimeKeyer` (domain `Anime`)
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeCoverKeyer.kt:13-22`:
```
options.useBackground && data.hasCustomBackground() -> "anime;${data.id};${data.backgroundLastModified}"
options.useBackground                              -> "anime;${data.backgroundUrl};${data.backgroundLastModified}"
data.hasCustomCover()                              -> "anime;${data.id};${data.coverLastModified}"
else                                               -> "anime;${data.thumbnailUrl};${data.coverLastModified}"
```
`hasCustomBackground()` is `backgroundCache.getCustomBackgroundFile(id).exists()` (`app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:116-118`).

### 10.3 `MangaCoverKeyer` (projection `MangaCover`)
OBSERVED, `MangaCoverKeyer.kt:22-32`: `if (coverCache.getCustomCoverFile(data.mangaId).exists()) "manga;${data.mangaId};${data.lastModified}" else "manga;${data.url};${data.lastModified}"`. Default constructor parameter `coverCache: MangaCoverCache = Injekt.get()`.

### 10.4 `AnimeCoverKeyer` (projection `AnimeCover`)
OBSERVED, `AnimeCoverKeyer.kt:24-34`: same shape with `anime;` and `data.animeId`, `data.url`, `data.lastModified`.

Consequence: changing a cover's `last_modified` changes its key, so stale memory and disk entries are never shown after an update.

## 11. Local-file and URI loaders

### 11.1 `fileLoader(file)`
OBSERVED, `MangaCoverFetcher.kt:93-103`: `SourceFetchResult(ImageSource(file = file.toOkioPath(), fileSystem = SYSTEM, diskCacheKey = diskCacheKey), mimeType = "image/*", dataSource = DISK)`. The file is not checked for existence here; a missing file causes an I/O error at decode time.

### 11.2 `uniFileLoader(urlString)` (Android content URIs)
OBSERVED, `MangaCoverFetcher.kt:83-91`: `UniFile.fromUri(options.context, urlString.toUri())!!`, then `openInputStream()`. The `!!` throws if the URI cannot be resolved. Dropped on Linux (see task T-34a-022 and data §9.2).

### 11.3 Error texts
OBSERVED: `"No cover specified"` (url null), `"Invalid image"` (classification null), `"Null response source"` (response body null), `"Failed to initialize decoder"` and `"Failed to decode image"` (§14). All are thrown with `error()`/`check()`/`checkNotNull()` and become Coil error results. The UI shows the error placeholder (§25.3).

## 12. Custom cover precedence summary

OBSERVED, combining §5.1 and §10:
- A custom cover (file `covers/custom/<md5(id)>`) wins over the thumbnail for both display (fetch) and cache key (keyer).
- Background requests use the custom background file under the same rule, and only when `useBackground` is true.
- Removing the custom file changes the keyer output only after `awaitUpdateCoverLastModified` runs (§19.5). Deleting the file alone is not enough for a visible refresh; the Delete action always does both (§24.4).

## 13. Decoder selection

### 13.1 Rule
OBSERVED, `TachiyomiImageDecoder.kt:62-70, 72-80`: `create(result, options, imageLoader)` returns `TachiyomiImageDecoder(result.source, options)` when `options.customDecoder || isApplicable(result.source.source())`, else null (so Coil's default decoder runs).
`isApplicable(source)`: `source.peek().inputStream().use { ImageUtil.findImageType(it) }`; returns true for `ImageType.AVIF`, `ImageType.JXL`, `ImageType.HEIF`; false for everything else.

### 13.2 Consequence
JPEG, PNG, GIF, WebP, BMP go to Coil's platform decoder (Android `BitmapFactory`/`ImageDecoder`) unless `customDecoder` is set. Reader webtoon pages set `customDecoder(true)` (§3.2), which sends every page to the native decoder so that the crop option and the ICC profile apply.

### 13.3 Failure to find a decoder
OBSERVED: when the factory returns null for every decoder, Coil reports an unsupported-type error. PROPOSAL: Kurobayomi treats an unsupported type as a decode error with the same user-visible text used for corrupt images (`Failed to decode image`).

### 13.4 Registration priority
OBSERVED: the custom factory is registered first (App.kt:189), before the built-in factories. LIBRARY: Coil consults factories registered through `components` before its defaults, so the custom decoder wins for AVIF, JXL and HEIF.

### 13.5 Animated images
OBSERVED, `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:~60-80` (`isAnimatedAndSupported`): GIF is always animated-supported; WebP animation is supported when `Build.VERSION.SDK_INT >= 28` (`VERSION_CODES.P`); HEIF animation when `>= 30` (`VERSION_CODES.R`). `TachiyomiImageDecoder` decodes only the first frame (`decoder.decode(sampleSize)` returns one bitmap). Animated Coil decoding is therefore Coil's job for GIF and WebP, and AVIF/JXL/HEIF are always stills.
PROPOSAL: task T-34a-033.

## 14. Native decode (`TachiyomiImageDecoder.decode`)

### 14.1 Step list
OBSERVED, `TachiyomiImageDecoder.kt:22-60`:
1. `decoder = resources.sourceOrNull()?.use { ImageDecoder.newInstance(it.inputStream(), options.cropBorders, displayProfile) }`.
2. `check(decoder != null && decoder.width > 0 && decoder.height > 0) { "Failed to initialize decoder" }`.
3. `srcWidth = decoder.width`, `srcHeight = decoder.height`.
4. `dstWidth = options.size.widthPx(options.scale) { srcWidth }`, `dstHeight = options.size.heightPx(options.scale) { srcHeight }` (§15.2).
5. `sampleSize = DecodeUtils.calculateInSampleSize(srcWidth, srcHeight, dstWidth, dstHeight, options.scale)` (§15.1).
6. `bitmap = decoder.decode(sampleSize = sampleSize)`; `decoder.recycle()`.
7. `check(bitmap != null) { "Failed to decode image" }`.
8. If `options.bitmapConfig == Bitmap.Config.HARDWARE && ImageUtil.canUseHardwareBitmap(bitmap)`: `hwBitmap = bitmap.copy(HARDWARE, false)`; if non-null, recycle the software bitmap and use the copy (§16).
9. `DecodeResult(image = bitmap.asImage(), isSampled = sampleSize > 1)`.

### 14.2 Empty image
OBSERVED: a file whose header decodes to width or height 0 fails at step 2 with `Failed to initialize decoder`.

### 14.3 Crop borders
OBSERVED: the flag is passed to the native decoder constructor (step 1). The algorithm is inside the native library (O-3). Reader setting `crop_borders` (default false, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:69`).

### 14.4 Failure text
OBSERVED: `"Failed to decode image"` when `decode` returns null (step 7).

### 14.5 Output
OBSERVED: the output bitmap is a sampled software bitmap (`ARGB_8888` unless `allowRgb565`). `isSampled` is the value consumed by Coil's `DecodeResult`.

### 14.6 Display profile
OBSERVED: `TachiyomiImageDecoder.displayProfile: ByteArray?` (companion `var`, line 88). Set by `ReaderActivity.setDisplayProfile(path)` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:~895-912`) which reads the chosen file (UniFile → bytes) and assigns it both to `SubsamplingScaleImageView.setDisplayProfile` and to `TachiyomiImageDecoder.displayProfile`. Triggered by changes to preference `pref_display_profile_key` (default `""`, `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:38`). An empty or unreadable path does nothing.

### 14.7 EXIF orientation
OBSERVED: the Kotlin code does not read EXIF. LIBRARY: Coil's platform decoder may apply orientation (not verified for the native path). Open question O-1.

## 15. Sampling

### 15.1 Formula
LIBRARY, `coil-core/src/commonMain/kotlin/coil3/decode/DecodeUtils.kt:20-33`:
```kotlin
val widthInSampleSize = (srcWidth / dstWidth).takeHighestOneBit()
val heightInSampleSize = (srcHeight / dstHeight).takeHighestOneBit()
return when (scale) {
    Scale.FILL -> minOf(widthInSampleSize, heightInSampleSize)
    Scale.FIT  -> maxOf(widthInSampleSize, heightInSampleSize)
}.coerceAtLeast(1)
```
All arithmetic is 32-bit Int with integer division. `takeHighestOneBit()` keeps only the highest set bit: `0 -> 0`, `5 -> 4`, `6 -> 4`, `8 -> 8`, `3 -> 2`. For a negative input the highest bit is the sign bit (Int.MIN_VALUE); this only happens for negative destination sentinels divided into positive sources and is always clamped to 1.
Consequences:
- Destination `Int.MIN_VALUE` (undefined, FILL): `srcW / MIN` is `0`, so both terms are 0 and the result is 1.
- Destination `Int.MAX_VALUE` (undefined, FIT): `srcW / MAX` is 0 for any realistic source, result 1.
- Destination 0 (a zero-size request): Kotlin throws `ArithmeticException`. PROPOSAL: treat 0 as undefined (result 1); see edge E-IMG-7.

### 15.2 Target size mapping
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt:13-26`:
- `Size.widthPx(scale, original)` = `if (isOriginal) original() else width.toPx(scale)`; same for height.
- `Dimension.toPx(scale)` = `pxOrElse { when (scale) { Scale.FILL -> Int.MIN_VALUE; Scale.FIT -> Int.MAX_VALUE } }`. A `Pixel(n)` dimension returns `n`; an `Undefined` dimension returns the sentinel.
- `Size.ORIGINAL` (`isOriginal`) returns the source dimension, so the sample size is 1.

### 15.3 Scale default
LIBRARY: Coil's default `Scale` is `FIT`; `ImageRequest` in this repository never sets `scale`, so FIT applies. Note that `ViewSizeResolver` requests in `ReaderPageImageView` also use FIT.

## 16. Hardware bitmaps (Android only)

### 16.1 Support rule
OBSERVED, `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:611-~725`:
- `canUseHardwareBitmap(bitmap)` and `canUseHardwareBitmap(imageSource: BufferedSource)` (lines 318-325) both call `canUseHardwareBitmap(width, height)` (line 330-333): `if (HARDWARE_BITMAP_UNSUPPORTED) return false; return maxOf(width, height) <= hardwareBitmapThreshold`.
- `var hardwareBitmapThreshold: Int = GLUtil.SAFE_TEXTURE_LIMIT` (line 335).
- `HARDWARE_BITMAP_UNSUPPORTED` (line 621-724) is a `when (Build.VERSION.SDK_INT)` with device-name denylists for API 26 (line 622, starting with Samsung `SM-` models, then `Build.DEVICE` values such as `nora`, `james`, `ali`, `evert`), API 27 (line 651), and `else -> false` (line 723).
- `GLUtil.SAFE_TEXTURE_LIMIT = 2048` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt:50`). `GLUtil` computes the device texture limit via EGL and returns `max(maximumTextureSize, SAFE_TEXTURE_LIMIT)` (line 47).
- Preference `pref_hardware_bitmap_threshold` default `GLUtil.SAFE_TEXTURE_LIMIT` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:40`), exposed in advanced settings (`SettingsAdvancedScreen.kt:383` reads `DEVICE_TEXTURE_LIMIT > SAFE_TEXTURE_LIMIT`).

### 16.2 Use
OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt:318` `setHardwareConfig(ImageUtil.canUseHardwareBitmap(data))` for SSIV sources; `TachiyomiImageDecoder` step 8.

### 16.3 Linux decision
PROPOSAL: no hardware-bitmap concept. GTK uploads the RGBA texture; GPU limits are handled by GTK. The `pref_hardware_bitmap_threshold` key is not read. The device denylist is dropped (task T-34a-034).

## 17. Image type detection

### 17.1 Types
OBSERVED, `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:106-114`: `enum class ImageType(val mime: String, val extension: String)`: `AVIF("image/avif","avif")`, `GIF("image/gif","gif")`, `HEIF("image/heif","heif")`, `JPEG("image/jpeg","jpg")`, `JXL("image/jxl","jxl")`, `PNG("image/png","png")`, `WEBP("image/webp","webp")`.

### 17.2 Sniffing
OBSERVED, `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:46-66, 84-96`:
- `findImageType(openStream: () -> InputStream)` opens the stream, calls `findImageType(stream)`, closes it.
- `findImageType(stream)`: `getImageType(stream)?.format` mapped `Format.Avif→AVIF, Gif→GIF, Heif→HEIF, Jpeg→JPEG, Jxl→JXL, Png→PNG, Webp→WEBP, else→null`; any exception → null.
- `getImageType(stream)`: `bytes = ByteArray(32)`; if `stream.markSupported()` then `mark(32)`, read, `reset()`; otherwise a plain read. `length == -1` → null. Otherwise `ImageDecoder.findType(bytes)` (native).
Only the first 32 bytes are used. An empty stream (length -1) is "no type"; a 1 to 31 byte stream is passed to the native `findType` with the bytes read.

### 17.3 `isImage(name, openStream)`
OBSERVED, `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:39-44`: `name == null → false`; `extension = name.substringAfterLast('.')`; true when `ImageType.entries.any { it.extension == extension }` (case-sensitive) or when `openStream` is given and `findImageType(openStream) != null`.
Consequence: `photo.JPG` fails the name test but passes if the bytes are JPEG; a file named `cover.jpeg` (extension `jpeg`, not in the table) passes only by sniffing.

### 17.4 Extension fallback
OBSERVED, `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:68-70`: `getExtensionFromMimeType(mime, openStream)` looks up the mime in `ImageType.entries`, else sniffs, else returns `"jpg"`.

## 18. Cover, background and custom-cover file caches

### 18.1 Roots
OBSERVED, each class's `getCacheDir(dir)` (`MangaCoverCache.kt:101-104`, `AnimeCoverCache.kt:101-104`, `AnimeBackgroundCache.kt:101-104`) returns `context.getExternalFilesDir(dir) ?: File(context.filesDir, dir).also { mkdirs() }`. Names:
- `MangaCoverCache`: `COVERS_DIR = "covers"`, `CUSTOM_COVERS_DIR = "covers/custom"` (lines 21-22).
- `AnimeCoverCache`: `COVERS_DIR = "animecovers"`, `CUSTOM_COVERS_DIR = "animecovers/custom"` (lines 21-22).
- `AnimeBackgroundCache`: `BACKGROUNDS_DIR = "animebackgrounds"`, `CUSTOM_BACKGROUNDS_DIR = "animebackgrounds/custom"` (lines 21-22).
Both directories are created when the class is constructed (`private val cacheDir = getCacheDir(...)`, `customCoverCacheDir = getCacheDir(...)`), so the directories exist from first injection.
PROPOSAL: the same names under the Linux data directory (`Paths::data_dir()`), not the cache directory, because covers of library entries must survive cache clearing.

### 18.2 Names
OBSERVED: a thumbnail-keyed file is `File(cacheDir, DiskUtil.hashKeyForDisk(url))` (`MangaCoverCache.kt:38-42`); the custom file is `File(customCoverCacheDir, DiskUtil.hashKeyForDisk(mangaId.toString()))` (line 50-52). `hashKeyForDisk(key) = Hash.md5(key)` (`DiskUtil.kt:32-34`), lower-case hex of the MD5 of the UTF-8 bytes (`Hash.kt:37-43`, `encodeHex` with digits `0-9a-f`).

### 18.3 `getCoverFile(thumbnailUrl: String?)`
OBSERVED: `mangaThumbnailUrl?.let { File(...) }`. A null URL returns null. An empty string is not null and produces the file for `md5("")`. The same applies to anime covers and backgrounds.

### 18.4 `getCustomCoverFile(mangaId: Long?)`
OBSERVED: returns `File(customCoverCacheDir, md5(mangaId.toString()))`. A null id hashes the string `"null"` (no crash). Non-null return type.

### 18.5 Anime background file names
OBSERVED: `getBackgroundFile(url)` → `File(cacheDir, md5(url))` under `animebackgrounds`; `getCustomBackgroundFile(animeId)` → `File(customBackgroundCacheDir, md5(animeId.toString()))` under `animebackgrounds/custom`.

### 18.6 Writes
OBSERVED: `setCustomCoverToCache(entry, inputStream)` = `getCustomCoverFile(entry.id).outputStream().use { inputStream.copyTo(it) }` (`MangaCoverCache.kt:62-66`). `outputStream()` truncates. Errors propagate as `IOException` (declared `@Throws`). The same for `setCustomBackgroundToCache`.

### 18.7 Deletes
OBSERVED, `MangaCoverCache.kt:75-99`:
- `deleteFromCache(manga, deleteCustomCover = false): Int`: `var deleted = 0`; `getCoverFile(manga.thumbnailUrl)?.let { if (it.exists() && it.delete()) ++deleted }`; if `deleteCustomCover` then `if (deleteCustomCover(manga.id)) ++deleted`; return `deleted`.
- `deleteCustomCover(mangaId: Long?): Boolean = getCustomCoverFile(mangaId).let { it.exists() && it.delete() }`.
- `AnimeCoverCache.deleteFromCache(anime, deleteCustomCover)` identical with `anime.thumbnailUrl`. `AnimeBackgroundCache.deleteFromCache(anime, deleteCustomBackground)` uses `anime.backgroundUrl` and `deleteCustomBackground(anime.id)`.
- The counting is per file: a thumbnail-plus-custom removal can return 2.

## 19. Cover invalidation and last-modified timestamps

### 19.1 `Manga.prepUpdateCover(coverCache, remoteManga, refreshSameUrl)`
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt:19-38`:
1. `newUrl = remoteManga.thumbnail_url ?: return this`. A null remote URL never refreshes the cover (the comment: the current URL may have become invalid).
2. `if (newUrl.isEmpty()) return this`. An empty remote URL never refreshes.
3. `if (!refreshSameUrl && thumbnailUrl == newUrl) return this`.
4. `when`:
   - `isLocal()` → `copy(coverLastModified = now_ms)` (no cache deletion; local covers are files in the entry's folder).
   - `hasCustomCover(coverCache)` → `coverCache.deleteFromCache(this, false)`; return `this` (timestamp unchanged; the custom cover stays).
   - else → `coverCache.deleteFromCache(this, false)`; `copy(coverLastModified = now_ms)`.
`now_ms` = `Instant.now().toEpochMilli()`.
Decision table is in task T-34a-011 and test cases C1-C7.

### 19.2 `Manga.removeCovers(coverCache)`
OBSERVED, `MangaExtensions.kt:44-50`: `if (isLocal()) return this`; `if (coverCache.deleteFromCache(this, true) > 0) return copy(coverLastModified = now_ms) else return this`.
Anime: `AnimeExtensions.kt:18-24` with the same shape. `Anime.removeBackgrounds(backgroundCache)` (lines 26-33) uses `backgroundCache.deleteFromCache(this, true)` and `copy(backgroundLastModified = now_ms)`.

### 19.3 Edit paths (`editCover`)
OBSERVED, `MangaExtensions.kt:~50-64` and `AnimeExtensions.kt:35-48`:
- Manga `editCover(coverManager, stream, updateManga, coverCache)`: `if (isLocal())` → `coverManager.update(toSManga(), stream)` then `updateManga.awaitUpdateCoverLastModified(id)`; else (remote entry, favourite or not, see below) → `coverCache.setCustomCoverToCache(this, stream)` then `awaitUpdateCoverLastModified(id)`.
- Anime `editCover`: `if (isLocal())` → `coverManager.update(toSAnime(), stream)` + `awaitUpdateCoverLastModified(id)`; `else if (favorite)` → `coverCache.setCustomCoverToCache(this, stream)` + `awaitUpdateCoverLastModified(id)`; non-favourite remote entries do nothing.
- Anime `editBackground`: same shape with `backgroundManager`, `setCustomBackgroundToCache`, `awaitUpdateBackgroundLastModified`.
Note: the manga version does not check `favorite`; the anime version does. The edit button is shown for both (§24.1), so a manga that is not in the library accepts a custom cover, while the same action on an anime that is not in the library does nothing and still reports success (edge E-IMG-14).

### 19.4 Removing from the library
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:~320-330` (`toggleFavorite`, favourite true → remove branch): `if (updateManga.awaitUpdateFavorite(manga.id, false)) { if (manga.removeCovers() != manga) updateManga.awaitUpdateCoverLastModified(manga.id); onRemoved() }`. `removeCovers()` already returns a copy with a new timestamp when files were deleted, so the data-class inequality triggers the DB write. The DB then has the timestamp from `awaitUpdateCoverLastModified`.

### 19.5 Persistence
OBSERVED:
- `UpdateManga.awaitUpdateCoverLastModified(mangaId)` = `mangaRepository.updateManga(MangaUpdate(id, coverLastModified = now_ms))` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:93-95`).
- `UpdateAnime.awaitUpdateCoverLastModified(animeId)` = `animeRepository.updateAnime(AnimeUpdate(id, coverLastModified = now_ms))` (`app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt:37-41`).
- `UpdateAnime.awaitUpdateBackgroundLastModified(animeId)` = `AnimeUpdate(id, backgroundLastModified = now_ms)` (lines 43-47).
- DB: `mangas.cover_last_modified INTEGER NOT NULL` (`data/src/main/sqldelight/data/mangas.sq:23`); `animes.cover_last_modified INTEGER NOT NULL` (`data/src/main/sqldelightanime/dataanime/animes.sq:25`); `animes.background_last_modified INTEGER NOT NULL` (line 39).
- Update uses `coalesce(:coverLastModified, cover_last_modified)` (`mangas.sq:173`), so a null field leaves the column alone.

### 19.6 `hasCustomCover` and `hasCustomBackground`
OBSERVED: `Manga.hasCustomCover(coverCache = Injekt.get())` = `coverCache.getCustomCoverFile(id).exists()` (`Manga.kt:88-90`). `Anime.hasCustomCover` (`Anime.kt:112-114`) and `Anime.hasCustomBackground` (`Anime.kt:116-118`) work the same way. These are filesystem checks on every call; they are not cached.

## 20. Chapter (page) cache

### 20.1 Storage
OBSERVED, `ChapterCache.kt:35-40`: `DiskLruCache.open(File(context.cacheDir, "chapter_disk_cache"), PARAMETER_APP_VERSION, PARAMETER_VALUE_COUNT, PARAMETER_CACHE_SIZE)` with `PARAMETER_APP_VERSION = 1`, `PARAMETER_VALUE_COUNT = 1`, `PARAMETER_CACHE_SIZE = 100L * 1024 * 1024` (lines 203-209). The cache is in the app's internal cache directory (evictable by the OS).
`readableSize` (lines 56-57) = `Formatter.formatFileSize(context, DiskUtil.getDirectorySize(cacheDir))`.

### 20.2 Keys
OBSERVED:
- Page-list key: `DiskUtil.hashKeyForDisk("${chapter.mangaId}${chapter.url}")` (`getKey`, lines 197-199). Concatenation with no separator.
- Image key: `DiskUtil.hashKeyForDisk(imageUrl)` (lines 116-118, 130-134, 144-151).
- On disk, DiskLruCache stores an entry as `<key>.0` (value index 0) with a journal file `journal`; temporary writes use `<key>.0.tmp` (DiskLruCache naming, LIBRARY).

### 20.3 Page-list read and write
OBSERVED:
- `getPageListFromCache(chapter)`: `diskCache.get(key).use { json.decodeFromString(it.getString(0)) }` (lines 65-73). A missing key makes `get` return null, so `it.getString` throws (NullPointerException). The caller catches it (§20.7).
- `putPageListToCache(chapter, pages)` (lines 81-108): `cachedValue = json.encodeToString(pages)`; `editor = diskCache.edit(key) ?: return` (silent no-op when another edit of the same key is active); write UTF-8 bytes; `diskCache.flush()`; `editor.commit()`; `abortUnlessCommitted()`. Any `Exception` → `logcat(WARN) { "Failed to put page list to cache" }` and ignored. `finally` → `abortUnlessCommitted()`.
- JSON: `Json { ignoreUnknownKeys = true; explicitNulls = false }` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`). The `Page` model is `@Serializable` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt:11`) with fields `index: Int`, `url: String = ""`, `imageUrl: String? = null`; `uri` is transient.

### 20.4 Image read and write
OBSERVED:
- `isImageInCache(imageUrl)` (lines 116-122): `diskCache.get(key).use { it != null }`; catches `IOException` → false. A missing entry returns false (Kotlin `use` accepts a null receiver).
- `getImageFile(imageUrl)` (lines 130-134): `File(diskCache.directory, md5(imageUrl) + ".0")`. Does not check existence.
- `putImageToCache(imageUrl, response)` (lines 144-162): `editor = diskCache.edit(key) ?: throw IOException("Unable to edit key")`; `response.body.source().saveTo(editor.newOutputStream(0))` (Okio copy, `core/common/.../util/storage/` `saveTo`); `diskCache.flush()`; `editor.commit()`; `finally { response.body.close(); editor?.abortUnlessCommitted() }`.

### 20.5 Eviction
LIBRARY: DiskLruCache evicts least-recently-used entries when the total size exceeds `maxSize`; `get` promotes an entry. Eviction happens inside `commit`. The journal records operations; `flush()` forces the journal to disk.

### 20.6 Clearing
OBSERVED, `ChapterCache.kt:164-195`:
- `clear(): Int`: for each child of `cacheDir` (`listFiles()`), call `removeFileFromCache(it.name)` and count `true`.
- `removeFileFromCache(file)`: names `journal` and any name starting with `journal.` are skipped (return false). Otherwise `key = file.substringBeforeLast(".")` and `diskCache.remove(key)` (returns true only for a real entry). Exceptions → `logcat(WARN) { "Failed to remove file from cache" }`, false.
- Effect: `<key>.0` files are removed and counted; `<key>.0.tmp` files are not removed (their derived key is not an entry), so temporary files can persist. Edge E-IMG-6.

### 20.7 Reader read-through (consumer: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt`)
OBSERVED:
- `getPages()` (lines 71-85): `try { chapterCache.getPageListFromCache(chapter.chapter.toDomainChapter()!!) } catch (e: Throwable) { if (e is CancellationException) throw e; source.getPageList(chapter.chapter) }`. Then maps each page to `ReaderPage(index, page.url, page.imageUrl)` ("don't trust sources and use our own indexing").
- `loadPage(page)` (lines 91-115): if `page.status == READY` and `imageUrl != null` and `!isImageInCache(imageUrl)` → status `QUEUE`. If status `ERROR` → `QUEUE`. Then queue the page at priority 1 and preload the next pages at priority 0.
- `internalLoadPage(page)` (lines 190-205): if `imageUrl` null/empty → status `LOAD_PAGE`, `imageUrl = source.getImageUrl(page)`. If `!isImageInCache(imageUrl)` → status `DOWNLOAD_IMAGE`; `imageResponse = source.getImage(page, dataSaver)`; `putImageToCache(imageUrl, imageResponse)`. Then `page.stream = { getImageFile(imageUrl).inputStream() }`; status `READY`. Any `Throwable` sets status `ERROR`; a `CancellationException` is rethrown after the status is set.
- `recycle()` (lines 140-158): cancel scope, clear queue, then `launchIO { putPageListToCache(chapter, pages.map { Page(it.index, it.url, it.imageUrl) }) }` for online chapters that have a page list.

### 20.8 Downloader reuse
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:481-482`: `chapterCache.isImageInCache(page.imageUrl!!) -> copyImageFromCache(chapterCache.getImageFile(page.imageUrl!!), ...)`. Downloads prefer the page cache over the network.

### 20.9 Clear on launch
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:~322-328`: `if (isLaunch && libraryPreferences.autoClearItemCache().get()) lifecycleScope.launchIO { chapterCache.clear() }`. `auto_clear_chapter_cache` default false (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:93`).

### 20.10 Manual clear
OBSERVED, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:~290-335`: a text preference `AYMR.strings.pref_clear_chapter_cache` with subtitle `MR.strings.used_cache` (`Used: %1$s`) showing `readableSize`. On tap: `clear()` in `launchNonCancellable`; success toast `MR.strings.cache_deleted` formatted with the count (`Cache cleared, %1$d files deleted`) and bump of the size refresh counter; failure toast `MR.strings.cache_delete_error`.

## 21. Saving images (`ImageSaver`)

### 21.1 Entry points
OBSERVED:
- Reader: `ReaderViewModel.saveImage()` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:804-837`) and `shareImage()` (lines ~840-875).
- Player: `PlayerViewModel.saveImage(imageStream, timePos)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1847-1880`) and `shareImage` (around 1880-1910).
- Manga cover: `MangaCoverScreenModel.saveCover(context)` and `shareCover(context)` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt`).
- Anime image: `AnimeImageScreenModel.saveImage(context)` and `shareImage(context)` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt:59-111`).
- `ImageSaver` is a singleton (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:221`: `addSingletonFactory { ImageSaver(app) }`).

### 21.2 Image variants
OBSERVED, `ImageSaver.kt:153-182`:
- `sealed class Image(open val name: String, open val location: Location)`.
- `Image.Cover(bitmap: Bitmap, name, location)`. Its `data` lambda: `ByteArrayOutputStream()`; `bitmap.compress(Bitmap.CompressFormat.JPEG, 100, baos)`; `ByteArrayInputStream(baos.toByteArray())`. Covers are always re-encoded as JPEG quality 100.
- `Image.Page(inputStream: () -> InputStream, name, location)`. `data = inputStream` (no re-encode).
- `typealias ImageBackground = Image.Cover` (line 29), unused in the repository (grep shows no reference beyond the definition).

### 21.3 Locations
OBSERVED, `ImageSaver.kt:184-214`:
- `Location.Pictures(relativePath: String)`; companion `create(relativePath = "")`.
- `Location.Cache` (object).
- `directory(context)`: `Cache → context.cacheImageDir`. `Pictures → File(Environment.getExternalStoragePublicDirectory(DIRECTORY_PICTURES), context.stringResource(MR.strings.app_name))`, then `File(file, relativePath)` when `relativePath` is non-empty. `MR.strings.app_name` = `Aniyomi` (`i18n/src/commonMain/moko-resources/base/strings.xml`, `app_name`).
- `cacheImageDir` = `File(cacheDir, "shared_image")` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt:8-9`).
PROPOSAL: `Pictures` → `<XDG Pictures>/Kurobayomi/<relativePath>`; `Cache` → `<cache>/shared_image`.

### 21.4 `save(image)`
OBSERVED, `ImageSaver.kt:35-46`:
1. `data = image.data`.
2. `type = ImageUtil.findImageType(data) ?: throw IllegalArgumentException("Not an image")`. For `Cover` the data is the freshly re-encoded JPEG, so the type is always JPEG.
3. `filename = DiskUtil.buildValidFilename("${image.name}.${type.extension}")`.
4. If `SDK_INT < Q` or the location is not `Pictures` → `save(data(), image.location.directory(context), filename)`.
5. Otherwise `saveApi29(...)` (MediaStore; §21.5).

### 21.5 Plain save
OBSERVED, `ImageSaver.kt:48-62`: `directory.mkdirs()`; `destFile = File(directory, filename)`; copy the stream into `destFile.outputStream()` (truncating, so an existing file with the same name is overwritten); `DiskUtil.scanMedia(context, destFile.toUri())`; return `destFile.getUriCompat(context)` (a `FileProvider` content URI).
PROPOSAL: keep the write and overwrite rule; drop the media scan (no Linux equivalent, see T-34a-044) and return a `file://` URI.

### 21.6 MediaStore save (Android Q+, Pictures only; dropped on Linux)
OBSERVED, `ImageSaver.kt:64-114`: `isMimeTypeSupported = MimeTypeMap.hasMimeType(type.mime)`; collection = `MediaStore.Images.Media` (supported) or `MediaStore.Files` (unsupported) on `VOLUME_EXTERNAL_PRIMARY`; `relativePath = [DIRECTORY_PICTURES or DIRECTORY_DOCUMENTS, "Aniyomi", imageLocation].joinToString(File.separator)`; content values: `RELATIVE_PATH`, `DISPLAY_NAME` (`image.name` when supported, else `filename`), `MIME_TYPE`, `DATE_MODIFIED = Instant.now().epochSecond`. `findUriOrDefault` queries by `RELATIVE_PATH = normalizedPath AND DISPLAY_NAME = filename` and reuses the existing row (so the file is overwritten); otherwise inserts; insert returning null → `IOException(stringResource(MR.strings.error_saving_picture))`. The copy failure → `IOException("Error saving picture")`. Then `DiskUtil.scanMedia`.
Linux replacement: the plain save (§21.5). Behavior difference: non-image MIME types never occur here, so the `DOCUMENTS` fallback is never used.

### 21.7 Share staging
OBSERVED, reader `shareImage` (app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:~840-875), player `shareImage` (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:~1880-1910), anime image `shareImage` (app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt:~93-111): `destDir = context.cacheImageDir`; inside a launch: `destDir.deleteRecursively()`; `imageSaver.save(Image.Page(..., location = Location.Cache))`. Then `Event.ShareImage(uri, page)` or `Event.CopyImage(uri)` (reader) or `startActivity(uri.toShareIntent(context))` (cover screens). Comment: "Only the last shared image will be kept so it won't be taking lots of internal disk space." Manga cover share uses `saveCoverInternal(context, temp = true)` with `Location.Cache`, which does not call `deleteRecursively` first (so covers accumulate in the staging directory until a reader or player share clears it; edge E-IMG-15).

### 21.8 Names
OBSERVED:
- Reader page: `generateFilename(manga, page)` (§22.3).
- Player frame: `generateFilename(anime, seconds)` (§22.4).
- Manga cover: `name = manga.title`.
- Anime cover and background: `name = if (isCover) "cover" else "background"`.
- Anime image save location: `Location.Pictures(anime.title)` (folder per anime); manga cover: `Location.Pictures.create()` (no folder).
- Reader folder: `relativePath = if (readerPreferences.folderPerManga().get()) DiskUtil.buildValidFilename(manga.title) else ""` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:~814-820`). Preference `create_folder_per_manga` (default false, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:81`).
- Player folder: `relativePath = DiskUtil.buildValidFilename(anime.title)` (always a folder per anime).

### 21.9 Errors and notifications
OBSERVED:
- Reader `saveImage`: on exception `notifier.onError(e.message)` and `Event.SavedImage(SaveImageResult.Error(e))`.
- Player `saveImage`: same shape.
- Covers and anime images show snackbars instead (§23.1).

## 22. File names

### 22.1 `DiskUtil.buildValidFilename(origName)`
OBSERVED, `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:109-~125`:
1. `name = origName.trim('.', ' ')` (strips leading and trailing dots and spaces).
2. If `name` is empty → `"(invalid)"`.
3. Each char: if `isValidFatFilenameChar(c)` keep it, else `'_'`.
4. `return sb.toString().take(240)` (240 UTF-16 code units, not bytes; a comment says the FAT limit is 255 minus reserved room).
`isValidFatFilenameChar(c)` (lines 130-138): false for code units `0x00..0x1f`, and for `"`, `*`, `/`, `:`, `<`, `>`, `?`, `\`, `|`, and `0x7f`; true otherwise.
Also: `MAX_FILE_NAME_BYTES = 250` (line ~142; comment "Safe theoretical max filename size is 255 bytes and 1 char = 2-4 bytes (UTF-8)"). `NOMEDIA_FILE = ".nomedia"` (line 140).
Note: the leading-dot strip means a title beginning with `.` loses it; Windows reserved names are not handled (the cross-platform addendum adds them; see T-34a-042).

### 22.2 `String.takeBytes(n)` and `String.byteSize()`
OBSERVED, `core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:45-60`:
- `byteSize() = toByteArray(UTF_8).size`.
- `takeBytes(n)`: if the UTF-8 size is at most `n` return the string; else `bytes.decodeToString(endIndex = n).replace("�", "")`. The replacement strip removes a partial trailing character produced by cutting mid-sequence.

### 22.3 Reader page file name
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:754-766`:
```
filenameSuffix = " - ${page.number}"          // page.number = index + 1
name = buildValidFilename("${manga.title} - ${chapter.name}".takeBytes(250 - filenameSuffix.byteSize())) + filenameSuffix
```
The suffix is not sanitised (it contains digits and ` - `). Example: title `A`, chapter `B`, page 3 → `A - B - 3`.

### 22.4 Player frame file name
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2013-2025`: `timePos` is the string from `Utils.prettyTime(seconds)` (player utility, area:45-player-core); `filenameSuffix = " - $timePos"`; `buildValidFilename("${anime.title} - ${episode.name}".takeBytes(250 - suffix.byteSize())) + suffix`. `generateFilename` returns null when `currentEpisode` is null, and `saveImage` then returns without doing anything (`?: return`).

### 22.5 Filenames for covers
OBSERVED: the final name goes through `ImageSaver.save` step 3 (`buildValidFilename("${name}.${ext}")`), so a manga title such as `Jo/Jo` becomes `Jo_Jo.jpg`.

## 23. Save and share UX

### 23.1 User-visible messages
OBSERVED (base English strings):
| Key | Text | Where shown |
|---|---|---|
| `MR.strings.cover_saved` | `Cover saved` | Manga cover save success (snackbar) |
| `MR.strings.error_saving_cover` | `Error saving cover` | Manga cover save failure |
| `MR.strings.error_sharing_cover` | `Error sharing cover` | Manga cover share failure |
| `MR.strings.cover_updated` | `Cover updated` | Cover edit or delete success |
| `MR.strings.notification_cover_update_failed` | `Failed to update cover` | Cover edit or delete failure |
| `MR.strings.action_edit_cover` | `Edit cover` | Edit icon content description |
| `MR.strings.error_saving_picture` | `Error saving picture` | MediaStore save failure (dropped) |
| `MR.strings.picture_saved` | `Picture saved` | Save notification title |
| `MR.strings.download_notifier_title_error` | `Error` | Save notification error title |
| `MR.strings.unknown_error` | `Unknown error` | Save notification error fallback text |
| `AYMR.strings.background_saved` | `Background saved` | Anime background save success |
| `AYMR.strings.error_saving_background` | `Error saving background` | Failure |
| `AYMR.strings.error_sharing_background` | `Error sharing background` | Share failure |
| `AYMR.strings.background_updated` | `Background updated` | Background edit or delete success |
| `AYMR.strings.notification_background_update_failed` | `Failed to update background` | Failure |
| `AYMR.strings.action_edit_background` | `Edit background` | Edit icon content description on page 2 |
Sources: `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`.

### 23.2 Save notification
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt`:
- Channel `Notifications.CHANNEL_COMMON`; id `Notifications.ID_DOWNLOAD_IMAGE` (shared constant, so a new save replaces the previous notification). `onClear()` cancels it (called at the start of every save).
- `onComplete(uri)`: loads the saved file with `ImageRequest` (`size(720, 1280)`, memory cache disabled) and posts `showCompleteNotification(uri, bitmap)`; on load error it calls `onError(null)`.
- Complete: title `Picture saved`; small icon `R.drawable.ic_photo_24dp`; `BigPictureStyle` with the bitmap if loaded; large icon = bitmap; `setAutoCancel(true)`; clears old actions; content intent = `NotificationHandler.openImagePendingActivity(context, uri)`; one action `Share` (`ic_share_24dp`) = `NotificationReceiver.shareImagePendingBroadcast(context, uri)`.
- Error: title `Error`; text = `error ?: "Unknown error"`; small icon `android.R.drawable.ic_menu_report_image`.

### 23.3 Reader save success
OBSERVED: on success `notifier.onComplete(uri)` then `Event.SavedImage(SaveImageResult.Success(uri))` on the UI channel. The reader shows a toast or snackbar depending on the event handler in `ReaderActivity` (area:43-reader-core).

### 23.4 Share intent
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt:13-37`: `Uri.toShareIntent(context, type = "image/*", message = null)`: `Intent.ACTION_SEND`; for `content` URIs adds `EXTRA_STREAM` (and optional `EXTRA_TEXT` message); for `http`/`https` adds `EXTRA_TEXT` = URI string; always `clipData = ClipData.newRawUri(null, uri)`, type `image/*`, `FLAG_GRANT_READ_URI_PERMISSION`; wrapped in `Intent.createChooser(shareIntent, stringResource(MR.strings.action_share))` with `FLAG_ACTIVITY_NEW_TASK`. Title `Share`.
PROPOSAL: `Opener`/share trait in `kuro-platform` (design §10).

### 23.5 Copy image
OBSERVED: `Event.CopyImage(uri)` (reader) copies the staged image to the clipboard (area:43-reader-core owns the handler).

## 24. Cover and background editing (user actions)

### 24.1 Edit button
OBSERVED:
- Manga cover dialog (`MangaCoverDialog.kt:~88-110`): the edit icon `IconButton` (`contentDescription = action_edit_cover`). If `isCustomCover` the click opens a dropdown menu with `Edit` (`MR.strings.action_edit`) and `Delete` (`MR.strings.action_delete`); otherwise it calls `onEditClick(EditCoverAction.EDIT)` directly.
- Anime images dialog (`AnimeImagesDialog.kt:~118-124`): same, with `isCover && isCustomCover || !isCover && isCustomBackground` as the menu condition.
- `EditCoverAction` enum values: `EDIT`, `DELETE` (`app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt:11-14`).

### 24.2 Picker
OBSERVED: `rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { if (it == null) return; sm.editCover(context, it) }`, launched with `getContent.launch("image/*")` (`MangaScreen.kt:262-275`, `AnimeScreen.kt:333-348`). Cancelling the picker does nothing.

### 24.3 Accepting bytes
OBSERVED: the stream is copied as-is (`MangaCoverScreenModel.editCover`: `context.contentResolver.openInputStream(data)?.use { manga.editCover(...) }`). No image validation happens. A non-image file is written as the cover; the next load fails with a decode error (`Failed to decode image` or Coil's error). PROPOSAL: validate with `sniff` before writing and show `Failed to update cover` for non-images (edge E-IMG-3).

### 24.4 Delete
OBSERVED: manga `deleteCustomCover(context)` = `coverCache.deleteCustomCover(mangaId)`; `updateManga.awaitUpdateCoverLastModified(mangaId)`; `notifyCoverUpdated`. Anime `deleteCustomImage` = `coverCache.deleteCustomCover(animeId)` + `awaitUpdateCoverLastModified`, or `backgroundCache.deleteCustomBackground(animeId)` + `awaitUpdateBackgroundLastModified`. Errors → `notifyFailedImageUpdate`.

### 24.5 Migration copy
OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt:293` calls `coverCache.setCustomCoverToCache(...)`; `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt:340` calls `coverCache.setCustomCoverToCache(...)` and `:348` calls `backgroundCache.setCustomBackgroundToCache(...)`. These copy the custom image from the old entry to the new one during migration.

## 25. Cover display (`ItemCover`)

### 25.1 Ratios
OBSERVED, `app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt`: `enum class ItemCover(val ratio: Float) { Square(1f / 1f), Book(2f / 3f), Thumb(16f / 9f) }`.

### 25.2 Rendering
OBSERVED: `AsyncImage(model = data, placeholder = ColorPainter(CoverPlaceholderColor), error = rememberResourceBitmapPainter(id = R.drawable.cover_error), contentDescription, modifier = modifier.aspectRatio(ratio).clip(shape).then(clickable(role = Button) if onClick != null), contentScale = ContentScale.Crop)`. `shape` default `MaterialTheme.shapes.extraSmall`. `CoverPlaceholderColor = Color(0x1F888888)` (ARGB, alpha `0x1F` = 31/255 ≈ 12 percent grey). The placeholder is shown while loading and when `data` is null.

### 25.3 Error
OBSERVED: `R.drawable.cover_error` is a vector drawable (`app/src/main/res/drawable/cover_error.xml`); `rememberResourceBitmapPainter(id)` (`app/src/main/java/eu/kanade/presentation/util/Resources.kt`) converts it to a `BitmapPainter` via `ContextCompat.getDrawable(context, id).toBitmap()`, or throws `Resources.NotFoundException()` if the drawable is missing. The remembered painter is keyed by `id` only.

### 25.4 Other users
OBSERVED: browse cards `BrowseMangaIcons.kt:94`, `BrowseAnimeIcons.kt:94` use the same error painter.

## 26. Full-size cover and background viewer

### 26.1 Pages
OBSERVED:
- Manga: `MangaCoverDialog` shows one `ReaderPageImageView`.
- Anime: `AnimeImageScreenModel` owns `pagerState = PagerState(pageCount = { 2 })`; `isCover = pagerState.currentPage != 1` (line ~30 of the model). Page 0 = cover, page 1 = background. `AnimeImagesDialog` has a `HorizontalPager(state = pagerState)`.
- Switch button (anime only): `onImageSwitchClicked` animates to `1 - currentPage`; its icon is `KeyboardArrowRight` when showing the cover and `KeyboardArrowLeft` when showing the background; its description is the alternate label (`action_edit_background` or `action_edit_cover`).

### 26.2 Image request and display
OBSERVED, `MangaCoverDialog.kt:~168-188` and `AnimeImagesDialog.kt:~205-226`: `ImageRequest.Builder(view.context).data(manga or anime).useBackground(page == 1) (anime only).size(Size.ORIGINAL).memoryCachePolicy(CachePolicy.DISABLED).target { image -> drawable = image.asDrawable(resources); copy = BitmapDrawable(resources, bitmap.copy(HARDWARE, false)) (or the drawable); view.setImage(copy, ReaderPageImageView.Config(zoomDuration = 500)) }`. Comment: "Copy bitmap in case it came from memory cache. Because SSIV needs to thoroughly read the image." The view is `ReaderPageImageView` (SubsamplingScaleImageView), so it supports pinch zoom and pan. `view.updatePadding(top = statusBarPx, bottom = bottomPx)`.
Note: the `copy` is made to a HARDWARE bitmap; on Linux the copy is a plain RGBA copy (§16.3).

### 26.3 Dialog frame
OBSERVED: `Dialog(properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false))`; `Scaffold(containerColor = Transparent, bottomBar = Row(padding 4.dp, navigationBarsPadding))`; the image area is `clickableNoIndication(onClick = onDismissRequest)` and `onViewClicked = onDismissRequest` so a single tap on the image also dismisses. Snackbar host at the top of the scaffold.
Pills: `ActionsPill` = `Row` clipped to `MaterialTheme.shapes.extraLarge`, background `colorScheme.background` at alpha 0.95. Left pill: `Close` (`action_close`), plus the anime switch. Right pill: `AppBarActions` with `Share` (`action_share`, icon Share) and `Save` (`action_save`, icon Save), then the edit `Box` with the dropdown when `onEditClick != null`.

## 27. Formatting utilities

### 27.1 Chapter and episode numbers
OBSERVED, `app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt:6-17`: one shared `DecimalFormat("#.###", DecimalFormatSymbols().apply { decimalSeparator = '.' })`. `formatChapterNumber(d)` and `formatEpisodeNumber(d)` both return `formatter.format(d)`.
Java DecimalFormat rules (default `RoundingMode.HALF_EVEN`, `maximumFractionDigits = 3`, `minimumFractionDigits = 0`, grouping off, integer digits as needed, zero printed as `0`).
Verified by running the JDK's `DecimalFormat` on this pattern (in the scratch test, JDK 21 default locale `en`): `0.0 → 0`, `0.5 → 0.5`, `1.0 → 1`, `1.5 → 1.5`, `1.3333 → 1.333`, `1.0005 → 1`, `1.0015 → 1.002`, `1.0025 → 1.002`, `2.25 → 2.25`, `-0.5 → -0.5`, `10.0 → 10`, `0.0005 → 0`, `0.0004 → 0`, `100.1234 → 100.123`, `NaN → NaN`, `+Infinity → ∞`.
Locale effect (verified with default locale `ar`): digits become Arabic-Indic (`١.٥`), and a negative sign gets a right-to-left mark (`؜-٠.٥`), and NaN becomes `ليس رقم`. PROPOSAL: ASCII output in all locales (design §11); edge E-IMG-17.
Use sites: `MangaHistoryItem.kt:79`, `MangaScreen.kt:793`, `AnimeSeasonListItem.kt:33`, `AnimeScreen.kt:598,931,1085`, `AnimeHistoryItem.kt:80`.

### 27.2 Duration text
OBSERVED, `app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt:14-31` `Duration.toDurationString(context, fallback)`:
```
toComponents { days, hours, minutes, seconds, _ ->
  buildList(4) {
    if (days != 0L) add("${day_short}")                              // %dd
    if (hours != 0) add("${hour_short}")                             // %dh
    if (minutes != 0 && (days == 0L || hours == 0)) add(minute_short) // %dm
    if (seconds != 0 && days == 0L && hours == 0) add(seconds_short)  // %ds
  }.joinToString(" ").ifBlank { fallback }
}
```
Strings: `day_short = "%dd"`, `hour_short = "%dh"`, `minute_short = "%dm"`, `seconds_short = "%ds"` (`i18n/src/commonMain/moko-resources/base/strings.xml`).
Examples: 90061 s → days 1, hours 1, minutes 1, seconds 1 → `1d 1h`. 3660 s → `1h 1m`. 3630 s (1h 0m 30s) → `1h` (the seconds rule needs hours == 0). 330 s → `5m 30s`. 0 → fallback. Used by `app/src/main/java/eu/kanade/presentation/more/stats/MangaStatsScreenContent.kt:64` and `app/src/main/java/eu/kanade/presentation/more/stats/AnimeStatsScreenContent.kt:68` with `none` as fallback.

### 27.3 Relative update time
OBSERVED, `TimeUtils.kt:35-44` `relativeTimeSpanString(epochMillis: Long)`:
1. `now = Instant.now().toEpochMilli()`.
2. `epochMillis <= 0L` → `relative_time_span_never` = `Never`.
3. `now - epochMillis < 60_000` (1 minute, including negative differences, so future timestamps also match) → `updates_last_update_info_just_now` = `Just now`.
4. Otherwise `DateUtils.getRelativeTimeSpanString(epochMillis, now, DateUtils.MINUTE_IN_MILLIS).toString()`.
LIBRARY (Android `DateUtils`, approximate): minute resolution means minute-level phrases such as `5 minutes ago` and hour phrases such as `2 hours ago`, switching to a date for older values. The exact transition and the date format are locale dependent and are not reproduced here (O-7).

## 28. Exception to message

### 28.1 Mapping
OBSERVED, `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:16-39` `context(context: Context) val Throwable.formattedMessage`. Branch order:
1. `DisabledTorrServerException` → `AYMR.strings.torrserver_disabled` = `TorrServer is not running`.
2. `HttpException` → `MR.strings.exception_http` with `code`: `HTTP %d, check website in WebView` (for example `HTTP 404, check website in WebView`).
3. `UnknownHostException` → if `!context.isOnline()` → `MR.strings.exception_offline` = `No Internet connection`; else `MR.strings.exception_unknown_host` with `message ?: ""` = `Couldn't reach %s`.
4. `NoChaptersException`, `NoEpisodesException` → `MR.strings.no_results_found` = `No results found`.
5. `SourceNotInstalledException`, `AnimeSourceNotInstalledException` → `MR.strings.loader_not_implemented_error` = `Source not found`.
6. Fallback (`when (val className = this::class.simpleName)`): `"Exception"` or `"IOException"` → `message ?: className`; any other class → `"$className: $message"` (so a null message prints `Name: null`).
Note: `this::class.simpleName` is null for anonymous classes; then the `else` branch prints `null: message`.

## 29. Navigation helpers (`Navigator.kt`)

### 29.1 Back press
OBSERVED: `val LocalBackPress: ProvidableCompositionLocal<(() -> Unit)?> = staticCompositionLocalOf { null }` (line 35). Screens call the provided lambda to pop the stack. Used by `SearchableSettings.kt`, `SettingsMainScreen.kt`, `PlayerSettingsMainScreen.kt`, `AboutScreen.kt`.

### 29.2 Transitions
OBSERVED: `DefaultNavigatorScreenTransition(navigator)` uses `materialSharedAxisX(forward = navigator.lastEvent != StackEvent.Pop, slideDistance = rememberSlideDistance())` inside `ScreenTransition`. `ScreenTransition` = `AnimatedContent(targetState = navigator.lastItem, transitionSpec = transition, label = "transition") { screen -> navigator.saveableState("transition", screen) { content(screen) } }`.
PROPOSAL: 300 ms `GtkStack` crossfade in both directions (task T-34a-060).

### 29.3 IO scope per screen model
OBSERVED: `val ScreenModel.ioCoroutineScope` (lines 55-61) uses `ScreenModelStore.getOrPutDependency(screenModel = this, name = "ScreenModelIoCoroutineScope", factory = { key -> CoroutineScope(Dispatchers.IO + SupervisorJob()) + CoroutineName(key) }, onDispose = { scope -> scope.cancel() })`. Lives as long as the screen model.
PROPOSAL: a `tokio` task group or `glib::MainContext` task owned by the screen object, cancelled on drop.

### 29.4 Tabs and assist URL
OBSERVED: `interface Tab` (lines 39-44) with `onReselect(navigator)` default no-op and `currentNavigationStyle()` reading `uiPreferences.navStyle()` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:41`, key `bottom_rail_nav_style`, default `NavStyle.MOVE_HISTORY_TO_MORE`). `interface AssistContentScreen { fun onProvideAssistUrl(): String? }` (lines 63-65), implemented by `BrowseAnimeSourceScreen`, and others, for the Android assist gesture.
PROPOSAL: `onReselect` maps to re-clicking the sidebar item (scroll to top). The assist interface is dropped (Android-only).

### 29.5 Screen keys
OBSERVED: `abstract class Screen : Screen { override val key: ScreenKey = uniqueScreenKey }` (lines 46-49). Each screen instance gets a unique key so `saveableState` keeps state per instance.

## 30. Window, permission and view helpers

### 30.1 Secure screen (`FLAG_SECURE`)
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt:6-12`: `Window.setSecureScreen(enabled)` sets or clears `FLAG_SECURE`.
Policy, `app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt:~70-80`: secure when `secure_screen_v2 == ALWAYS || (secure_screen_v2 == INCOGNITO && incognito_mode)`. `SecurityPreferences.SecureScreenMode` = `ALWAYS`, `INCOGNITO`, `NEVER` (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:30-34`). Default `INCOGNITO` (line 17). `incognito_mode` default false (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21`). The flag is applied on `onCreate` and each change (combined flow).
Linux: no equivalent (T-34a-064). Setting kept, no effect.

### 30.2 Install-unknown-apps state
OBSERVED, `app/src/main/java/eu/kanade/presentation/util/Permissions.kt:15-34`: `rememberRequestPackageInstallsPermissionState(initialValue = false)` returns a `Boolean` updated on every `ON_RESUME` from `context.packageManager.canRequestPackageInstalls()`. Callers: `AnimeExtensionsScreen.kt:156` and `MangaExtensionsScreen.kt:151` (initialValue true), `app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt:54` (default false).

### 30.3 Popup menu
OBSERVED, `ViewExtensions.kt:65-83` `View.popupMenu(menuRes, initMenu, onMenuItemClick)`: `PopupMenu(context, this, Gravity.NO_GRAVITY, R.attr.actionOverflowMenuStyle, 0)`; inflate `menuRes`; run `initMenu` on `popup.menu`; set item click listener that runs `onMenuItemClick` and returns true; `show()`; returns the popup. Callers: `app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt:89` and `app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt:99` (`R.menu.download_single`; hides `move_to_top` when position ≤ 1 and `move_to_bottom` when last).

### 30.4 Tablet layout
OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt:12` `TABLET_UI_REQUIRED_SCREEN_WIDTH_DP = 720`; `Configuration.isTabletUi() = smallestScreenWidthDp >= 720` (lines 20-22). `presentation/util/WindowSize.kt` exposes `@Composable isTabletUi()`. 17 files call it (for example `app/src/main/java/eu/kanade/presentation/components/AdaptiveSheet.kt:75`, `app/src/main/java/eu/kanade/presentation/reader/components/ChapterNavigator.kt:56`).

### 30.5 Visible on screen
OBSERVED, `ViewExtensions.kt:85-101` `View?.isVisibleOnScreen()`: false for null; false when `!isShown`; `getGlobalVisibleRect(rect)`; intersects with `Rect(0, 0, displayMetrics.widthPixels, displayMetrics.heightPixels)`; returns the intersection (non-empty means visible). Only caller: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt:301` (`if (isVisibleOnScreen()) landscapeZoom(true)` after a successful load).

### 30.6 Compose hosting
OBSERVED, `ViewExtensions.kt:26-56`: `ComponentActivity.setComposeContent` and `ComposeView.setComposeContent` wrap content in `TachiyomiTheme` with `LocalTextStyle = MaterialTheme.typography.bodySmall` and `LocalContentColor = colorScheme.onBackground`. The view variant sets `ViewCompositionStrategy.DisposeOnViewTreeLifecycleDestroyed`. Used by `CrashActivity.kt:19`.

### 30.7 Preference accessor
OBSERVED, `EditTextPreferenceExtensions.kt:8-10`: exposes the package-private `onBindEditTextListener` of `androidx.preference.EditTextPreference` (a reflection-free getter for an Android Preference internal). Callers: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt:153` and `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt:153`, which read the source's existing bind listener in order to wrap it for source-defined text preferences.

### 30.8 Animations in lists
OBSERVED, `FastScrollAnimateItem.kt:7-9`: `Modifier.animateItemFastScroll()` calls `animateItem(fadeInSpec = null, fadeOutSpec = null)` on the `LazyItemScope`, working around issue 352584409 (fast-scroll jank). Nine callers (for example `MangaHistoryScreen.kt:91`).

### 30.9 Drawable to painter
OBSERVED, `Resources.kt:21-29` (see §25.3).

## 31. Threading and dispatchers

OBSERVED summary:
- Cover fetch and network: Coil fetcher context `Dispatchers.IO.limitedParallelism(8)`.
- Decode: `Dispatchers.IO.limitedParallelism(3)`.
- Cover-file writes and page-cache writes: on the calling coroutine (IO when launched by `launchIO`).
- Saves: `viewModelScope.launchNonCancellable` (reader, player) or `screenModelScope.launch` (covers) on the main dispatcher; `imageSaver.save` runs on the calling coroutine (`withIOContext` in cover screen models).
- Cover edit and delete: `screenModelScope.launchIO`.
- Snackbars: `snackbarHostState.showSnackbar` from `screenModelScope.launch`.
- `ChapterCache` operations are synchronised by DiskLruCache internally; `putPageListToCache` is launched on IO from `recycle`.
- `FileProvider` URIs are produced inside `ImageSaver` (no thread requirement).
Race conditions and their handling: §20.3 (`edit` silently skipped when another edit is active), §8.3 (non-atomic cover writes), §21.7 (share staging delete races a previous share still being read), §19.4 (double bump on removal).

## 32. Interactions with other areas

- area:01-foundation-app-bootstrap: `Paths` directories; app start `App.kt` builder; `kuro-platform` traits.
- area:02-preferences-system: `verbose_logging`, `pref_display_profile_key`, `pref_hardware_bitmap_threshold`, `secure_screen_v2`, `incognito_mode`, `use_biometric_lock`, `lock_app_after`, `create_folder_per_manga`, `auto_clear_chapter_cache`, `bottom_rail_nav_style`.
- area:03-db-manga-schema-migrations/area:04-db-anime-schema-migrations/area:05-data-repos-entries-items DB: `cover_last_modified`, `background_last_modified` columns and the update queries.
- area:07-domain-entries-items/area:08-domain-library-category-history-updates domain: `Manga`, `Anime`, `MangaCover`, `AnimeCover` shapes; `hasCustomCover`.
- area:12-network-stack: OkHttp client, `await()`, source headers, `HttpSource`.
- area:13-core-util-storage-archive-torrent: `DiskUtil`, `Hash`, `StringExtensions`, `UniFile` (dropped content-URI path), `FileExtensions`.
- area:14-source-local-and-metadata: local cover, background and thumbnail managers write into entry folders; `isLocal()`; `.nomedia`.
- area:23-downloads-manga: page-cache reuse (§20.8), filename rules.
- area:35-data-misc-updater-notification-export: notification channel and ids (`SaveImageNotifier`).
- area:36-ui-shell-navigation-security-webview: navigation helpers, secure screen, tablet layout, popup menus.
- area:38-ui-entry-details-manga/area:39-ui-entry-details-anime: cover and background presentation, edit actions.
- area:43-reader-core/area:44-reader-viewers: page loader, reader image view, save and share handlers.
- area:45-player-core: `Utils.prettyTime`, player frame save.
- area:49-theme-and-design-system: `TachiyomiTheme`, shapes (`extraSmall`, `extraLarge`), colours.
- area:51-i18n: string keys in §23.1, §27.2, §28.1.

## 33. Open questions (unresolved, each has a stated default in the plan)

- O-1: Does Coil's native decode path apply EXIF orientation for JPEG covers? Default in plan: apply EXIF orientation (task T-34a-032), verify on sample files.
- O-2: Exact conversion rules of the native `tachiyomi.decoder` crop algorithm (`ImageDecoder.newInstance(stream, crop, profile)`). Source is not in the repository. Default: implement border detection in Rust with the same threshold idea; validate against Android output on the fixture set (T-34a-070).
- O-3: Source of `tachiyomi.decoder` (`com.github.tachiyomiorg:image-decoder:41c059e540`). Needed for exact HEIF brand list and crop behaviour. Default: use Rust crates (design §4) and document differences.
- O-4: HEIF brand list used by `ImageDecoder.findType` (native). Default: sniff `ftyp` brands `heic`, `heix`, `hevc`, `hevx`, `heim`, `heis`, `mif1`, `msf1`, `avif`/`avis` routed to AVIF.
- O-5: Whether Aniyomi on Arabic devices shows Arabic-Indic digits in chapter numbers (verified in JDK, unverified on Android ICU). Default: ASCII digits.
- O-6: Coil default disk-cache directory name. Default in Kurobayomi: its own `<cache>/images` directory.
- O-7: Exact Android `DateUtils.getRelativeTimeSpanString` phrases (locale and transition behaviour). Default: English phrases `N minutes ago`, `N hours ago`, `N days ago` up to 7 days; then `Mon D` (current year) or `Mon D, YYYY`.
- O-8: Behaviour of the ICC display profile on GTK (no colour management in GTK 4.16 for images). Default: drop, see T-34a-031 (P2).
- O-9: Whether `ReaderPageImageView` webtoon path (no memory and disk cache) should be kept on Linux. Default: keep, it is a reader option (area:44-reader-viewers).
