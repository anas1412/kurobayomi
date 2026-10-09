# Android / third-party API mapping, chunk 7 of 8

Source of the item list: `python3 plans/_tools/inventory.py items imports 7 8` (54 items). First item: `Resource`. Last item: `SystemClock`. All paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`).

Counts are `files/occurrences`: number of Kotlin/Java files importing the class, and whole-word occurrences of the simple name inside those files (the import line itself is included, so 2 means one real use). Replacement choices assume Rust stable, gtk4-rs 0.11 (GTK 4.16, no libadwaita), tokio, rusqlite (feature `bundled`), reqwest, serde, libmpv, Linux with Wayland and X11. Difficulty is trivial / moderate / hard.

Naming caveats (several simple names collide with unrelated classes):
- `Row` here is Glance (`androidx.glance.layout.Row`, home-screen widget), not Compose `Row`.
- `Size`, `Scale`, `ScaleDrawable` are Coil 3 classes (`coil3.size.*`), not Compose geometry.
- `Source` is `okio.Source`, not `okhttp3`/Kotlin; `Segment` is the Seeker library class (`dev.vivvvek.seeker.Segment`), not okio's `Segment`.
- `Resource` / `ResourceFormatted` are moko-resources `StringDesc` factory functions (`dev.icerock.moko.resources.desc.*`), not a data class.
- `Settings` is `android.provider.Settings`; the regex-based count also catches `Icons.Outlined.Settings` and the app's own `SearchableSettings`, which are unrelated (they are listed under the item as false positives).
- `Service` is `android.app.Service`, not "tracker service" (`deleteMangaFromService` in the track dialogs is unrelated).
- `Rule`, `RunWith`, `StartupMode`, `StartupTimingMetric` belong to the `macrobenchmark` Gradle module only.

Recurring Rust crates named in this chunk: `gtk4` 0.11, `gdk-pixbuf`, `image` (features `jpeg`, `png`, `gif`, `webp`), `reqwest`, `futures-util`, `tokio`, `tokio-util`, `rusqlite` (`bundled`), `serde`, `serde_json`, `quick-xml`, `intl_pluralrules`, `pulldown-cmark`, `zip`, `tracing`, `rustix` (statvfs, fstat), `libmpv2`, `thiserror`, `notify-rust` (desktop notifications), `ashpd` (xdg portals), `chrono`, `libc`. Cross-references: `area:12-network-stack`, `area:22-backup-restore`, `area:43-reader-core`, `area:45-player-core`, `area:23-downloads-manga`. These ids name architecture areas of this plan set.

## 1. Index

| # | Item | Fully qualified | Files/occ. | Replacement (one line) | Difficulty |
|---|---|---|---|---|---|
| 1 | Resource | dev.icerock.moko.resources.desc.Resource | 1/2 | `i18n::tr(key)` over parsed `strings.xml` | moderate |
| 2 | ResourceFormatted | dev.icerock.moko.resources.desc.ResourceFormatted | 1/2 | `i18n::tr_fmt(key, &args)` | moderate |
| 3 | Resources | android.content.res.Resources | 6/13 | `gdk::Display` monitor geometry; `gtk::IconTheme` / GResource | trivial |
| 4 | Response | okhttp3.Response | 27/103 | `reqwest::Response` + own `HttpError` | moderate |
| 5 | ResponseBody | okhttp3.ResponseBody | 1/3 | `reqwest::Response::bytes_stream()` wrapped by a progress adapter | moderate |
| 6 | RichText | com.halilibo.richtext.ui.material3.RichText | 1/2 | `pulldown-cmark` to `gtk::TextBuffer` tags | moderate |
| 7 | RichTextStringStyle | com.halilibo.richtext.ui.string.RichTextStringStyle | 1/2 | CSS class `link` colour | trivial |
| 8 | RichTextStyle | com.halilibo.richtext.ui.RichTextStyle | 1/2 | `gtk::TextTag` set, theme CSS | trivial |
| 9 | RoundedCornersTransformation | coil3.transform.RoundedCornersTransformation | 2/4 | dropped (widget); CSS `border-radius` for covers | trivial |
| 10 | Row | androidx.glance.layout.Row | 2/4 | dropped (Android home-screen widget) | trivial |
| 11 | Rule | org.junit.Rule | 2/4 | dropped (macrobenchmark) | trivial |
| 12 | RunWith | org.junit.runner.RunWith | 1/4 | dropped | trivial |
| 13 | SCALE_TYPE_CENTER_INSIDE | com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView.SCALE_TYPE_CENTER_INSIDE | 1/3 | `ImageScaleType::FitScreen` enum value | trivial |
| 14 | SQLiteException | android.database.sqlite.SQLiteException | 2/5 | `rusqlite::Error` mapped to domain errors | trivial |
| 15 | SavedStateHandle | androidx.lifecycle.SavedStateHandle | 2/4 | plain struct fields; optional session file | trivial |
| 16 | Scale | coil3.size.Scale | 3/10 | `enum ScaleMode { Fit, Fill }` in the image loader | trivial |
| 17 | ScaleDrawable | coil3.size.ScaleDrawable | 1/2 | dropped (`gdk::Texture` needs no unwrapping) | trivial |
| 18 | ScaleGestureDetector | android.view.ScaleGestureDetector | 1/6 | `gtk::GestureZoom` + Ctrl+scroll | moderate |
| 19 | SchemeContent | com.google.android.material.color.utilities.SchemeContent | 1/2 | dropped (Monet); GTK theme provides colours | trivial |
| 20 | Score | com.google.android.material.color.utilities.Score | 1/2 | dropped (unused `@Suppress("Unused")` helper) | trivial |
| 21 | SearchManager | android.app.SearchManager | 1/2 | CLI argument `--search` / `gio::Application::open` | trivial |
| 22 | SeekBar | android.widget.SeekBar | 1/5 | `gtk::Scale` (listener class is dead code) | trivial |
| 23 | Seeker | dev.vivvvek.seeker.Seeker | 1/2 | `gtk::Scale` + chapter marks + fill level | moderate |
| 24 | SeekerDefaults | dev.vivvvek.seeker.SeekerDefaults | 1/2 | CSS on `scale` nodes | trivial |
| 25 | Segment | dev.vivvvek.seeker.Segment | 6/18 | `struct Chapter { name, start, color }` from mpv `chapter-list` | trivial |
| 26 | Service | android.app.Service | 10/22 | tokio task + `Arc<AtomicBool>`; no foreground service | moderate |
| 27 | ServiceInfo | android.content.pm.ServiceInfo | 11/22 | dropped (foreground-service type); `gio::Application::hold` | trivial |
| 28 | Settings | android.provider.Settings | 7/21 | `gtk::Settings`, `ashpd`, `/sys/class/backlight` | moderate |
| 29 | SharedPreferences | android.content.SharedPreferences | 8/46 | `PrefStore` (SQLite `prefs` table or JSON file) | moderate |
| 30 | SimpleGridCells | io.woong.compose.grid.SimpleGridCells | 1/2 | `gtk::Grid` with 7 columns | trivial |
| 31 | SingletonImageLoader | coil3.SingletonImageLoader | 1/2 | `ImageLoader` singleton in `AppContext` | moderate |
| 32 | Size | coil3.size.Size | 5/11 | `enum TargetSize { Original, Px(w, h) }` | trivial |
| 33 | SizeMode | androidx.glance.appwidget.SizeMode | 2/4 | dropped (widget) | trivial |
| 34 | Source | okio.Source | 3/6 | `std::io::Read` / `tokio::io::AsyncRead` | trivial |
| 35 | SourceFetchResult | coil3.fetch.SourceFetchResult | 4/16 | `enum ImageBytes { File(PathBuf), Memory(Bytes) }` | moderate |
| 36 | SplashScreen | androidx.core.splashscreen.SplashScreen | 1/4 | dropped; optional delayed `present()` | trivial |
| 37 | SqlDriver | app.cash.sqldelight.db.SqlDriver | 2/4 | `rusqlite::Connection` behind a `Db` handle | moderate |
| 38 | StartupMode | androidx.benchmark.macro.StartupMode | 1/5 | dropped (benchmark) | trivial |
| 39 | StartupTimingMetric | androidx.benchmark.macro.StartupTimingMetric | 1/2 | dropped (benchmark) | trivial |
| 40 | StatFs | android.os.StatFs | 1/4 | `rustix::fs::statvfs` | trivial |
| 41 | StatisticsCallback | com.arthenica.ffmpegkit.StatisticsCallback | 1/2 | `ffmpeg -progress pipe:1` parsing | moderate |
| 42 | StreamResetException | okhttp3.internal.http2.StreamResetException | 1/2 | `reqwest::Error` source chain / cancel token | trivial |
| 43 | StringDesc | dev.icerock.moko.resources.desc.StringDesc | 1/5 | `i18n` module | moderate |
| 44 | StringResource | dev.icerock.moko.resources.StringResource | 66/155 | `&'static str` key constants / `Key` newtype | moderate |
| 45 | StructStat | android.system.StructStat | 1/2 | `rustix::fs::Stat` | trivial |
| 46 | SubsamplingScaleImageView | com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView | 4/27 | custom `gtk::Widget` tile/pan/zoom image view | hard |
| 47 | Subscriber | rx.Subscriber | 1/2 | dropped (RxJava 1 bridge); `async fn` | trivial |
| 48 | Subscription | rx.Subscription | 2/4 | `tokio_util::sync::CancellationToken` / dropped | trivial |
| 49 | SupportSQLiteDatabase | androidx.sqlite.db.SupportSQLiteDatabase | 1/5 | `Connection::pragma_update` at open | trivial |
| 50 | SuppressLint | android.annotation.SuppressLint | 26/57 | dropped (lint annotation) | trivial |
| 51 | SurfaceColors | com.google.android.material.elevation.SurfaceColors | 1/2 | dropped; CSS `@theme_bg_color` | trivial |
| 52 | SwipeableActionsBox | me.saket.swipe.SwipeableActionsBox | 2/4 | `gtk::GestureDrag` row + revealer; P2 | moderate |
| 53 | SystemBarStyle | androidx.activity.SystemBarStyle | 1/3 | dropped (edge-to-edge system bars) | trivial |
| 54 | SystemClock | android.os.SystemClock | 1/3 | `Instant` / `CLOCK_BOOTTIME` | trivial |

---

## 2. Localisation (moko-resources)

Aniyomi resolves every user-visible string through moko-resources. Two Gradle modules own the XML: `i18n` (Mihon strings, `i18n/src/commonMain/moko-resources/base/strings.xml`, 857 `<string` entries, plus `plurals.xml` with 19 `<plurals`) and `i18n-aniyomi` (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`, 570 entries, `plurals.xml` with 15). `i18n/src/commonMain/moko-resources` holds 67 entries (the `base` directory plus translation directories such as `am`, `ar`, `as`); `i18n-aniyomi/src/commonMain/moko-resources` holds 68. The generated Kotlin accessors are `MR.strings.<name>` (Mihon) and `AYMR.strings.<name>` (Aniyomi); `rg -c` shows 2336 lines using `MR.strings.`, 656 using `AYMR.strings.`, and 37 using `MR.plurals`/`AYMR.plurals`. Placeholders in the XML are Java format specifiers: `%s`, `%d`, positional `%1$s` / `%2$s` (for example `onboarding_storage_info`: `Select a folder where %1$s will store chapter downloads, backups, and more.\n\nA dedicated folder is recommended.\n\nSelected folder: %2$s`), and quote escapes `\"` (for example `delete_category_confirmation`: `Do you wish to delete the category \"%s\"?`).

### 2.1 Resource
* Aniyomi: `StringDesc.Resource(resource).toString(this).fixed()` in `core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt:13` (the `import dev.icerock.moko.resources.desc.Resource` is the extension factory on `StringDesc.Companion`). 1 file / 2 occ. Resolves a plain string for the current locale (moko uses the Android configuration locale of the `Context`).
* Replacement: module `i18n` with `fn tr(key: &Key) -> String`. Proposal: embed `strings.xml` for each locale with `include_str!` through a `build.rs` that runs `quick-xml` and emits a `phf::Map<&'static str, &'static str>` per locale (so no XML parsing at runtime). Locale selection: `LC_MESSAGES` / `LANGUAGE` env, then `glib::language_names()`; fall back to `base`. Lookup order: exact locale (`pt_BR` mapped to the Android directory name `pt-rBR`), language-only, `base`.
* Preserve: the unescape step. Android XML needs `\'` and `\"` and `\n` handled: `\n` becomes newline, `\"` becomes `"`, `\'` becomes `'`, `\\` becomes `\`, `\u00XX` becomes the char. `&lt;`, `&gt;` and `&amp;` are handled by the XML parser. The `.fixed()` workaround (see StringDesc) is replaced by correct unescaping in `build.rs`.
* Difficulty moderate. Risk: 1427 keys across two modules, 67 locales; the locale directory names that are not plain language codes are `ka-rGE`, `nb-rNO`, `pt-rBR`, `zh-rCN`, `zh-rTW` (plus the legacy `in` for Indonesian, `fil`, `ceb`, `sdh`, `sah`, `cv`); map them to POSIX names (`ka_GE`, `nb_NO`, `pt_BR`, `zh_CN`, `zh_TW`, `id`). Generate the table with a unit test that asserts every locale directory name maps to a valid `LanguageIdentifier`.

### 2.2 ResourceFormatted
* Aniyomi: `StringDesc.ResourceFormatted(resource, *args).toString(this).fixed()` (`Localize.kt:17`). 1 file / 2 occ.
* Replacement: `fn tr_fmt(key: &Key, args: &[&dyn Display]) -> String` with a tiny formatter that supports `%s`, `%d`, `%f`, `%%`, and positional `%N$s` / `%N$d`. Proposal: ~40 lines, no crate. Each key's placeholder list is validated in a `build.rs` check (count of distinct positions equals the number of call-site arguments is not checkable statically; instead assert in debug builds that no `%` specifier remains unreplaced).
* Preserve: positional specifiers may appear out of order in translations (`%2$s` before `%1$s`).
* Difficulty moderate. Risk: `%d` with a `Long` / `Int` argument versus `%s` with a number; format all through `Display`.

### 2.3 StringDesc
* Aniyomi: the single entry point class for the four helpers in `Localize.kt:12-26`: `Context.stringResource(resource)`, `Context.stringResource(resource, vararg args)`, `Context.pluralStringResource(resource, count)`, `Context.pluralStringResource(resource, count, vararg args)` (`StringDesc.Plural` and `StringDesc.PluralFormatted`). 1 file / 5 occ. The source comment at the bottom reads "janky workaround" and points at `https://github.com/icerockdev/moko-resources/issues/337`: `private fun String.fixed() = this.replace("""\""", """"""")` (`Localize.kt:27-28`): every backslash left in the resolved string is replaced by a double quote.
* Replacement: `i18n::plural(key, count)` and `i18n::plural_fmt(key, count, args)`. Plural category from the `intl_pluralrules` crate (CLDR: zero, one, two, few, many, other) for the active locale; the XML `<item quantity="one">` selects the entry, falling back to `other`.
* Preserve: Android plural quantity names (`zero`, `one`, `two`, `few`, `many`, `other`); formatting of the count argument as the first positional argument (`%d`).
* Difficulty moderate. Risk: Android XML plurals may omit `other` in some locale files; fall back to the `base` locale's entry rather than showing an empty string.

### 2.4 StringResource
* Aniyomi: the typed key. Representative uses: enum title fields `enum class ExtensionInstaller(val titleRes: StringResource, ...)` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31`), `AutoTrackState(val titleRes: StringResource)` (`app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt:6`), `AppTheme(val titleRes: StringResource?)` (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt:7`), `NavStyle`, `StartScreen`, `TabletUiMode` (`app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt:13`), and composable parameters such as `textRes: StringResource` (`app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt:489`). 66 files / 155 occ.
* Replacement: `pub struct Key(pub &'static str)`; constants generated from the XML names in `build.rs` (`pub const PREF_INCOGNITO_MODE: Key = Key("pref_incognito_mode");`). Enums keep `fn title(&self) -> Key`.
* Preserve: key names exactly (backups, extension repos and third-party docs refer to them; plan documents cite them as message keys).
* Difficulty moderate (volume, not complexity).

---

## 3. Resources, display metrics

### 3.1 Resources
* Aniyomi: `android.content.res.Resources` used for (a) `Resources.getSystem().displayMetrics.widthPixels` in `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt:104` (`margin = widthPixels * (viewer.config.sidePadding / 100f)`), (b) screen width/height in `app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt:97-98`, (c) `val getDisplayMaxHeightInPx: Int get() = max(heightPixels, widthPixels)` (`ImageUtil.kt:727-728`), which feeds `private val optimalImageHeight = getDisplayMaxHeightInPx * 2` (`ImageUtil.kt:609`) and the long-strip splitter `val partCount = (imageHeight - 1) / optimalImageHeight + 1` (`ImageUtil.kt:281-282`) (`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt`), (d) the density multiplier `this * Resources.getSystem().displayMetrics.density` for `Int.dpToPx` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt:9`), (e) `Resources.getSystem().getIdentifier(resName, type, "android")` for internal dimension lookups (`app/src/main/java/eu/kanade/tachiyomi/util/system/InternalResourceHelper.kt:26`), (f) `throw Resources.NotFoundException()` when a drawable id is missing (`app/src/main/java/eu/kanade/presentation/util/Resources.kt:26`). 6 files / 13 occ.
* Replacement: (a) webtoon side padding = `viewer_widget.width() * side_padding / 100.0` computed from the real widget width (the reader window can be any size on desktop; the Android code uses the screen). Pref `webtoon_side_padding` range 0..25 (`ReaderPreferences.WEBTOON_PADDING_MIN = 0`, `WEBTOON_PADDING_MAX = 25`, default 0). (b)(c) `gdk::Display::default().monitor_at_surface()` geometry; Proposal: `optimal_image_height = 2 * max(monitor_width_px, monitor_height_px)` (same formula, evaluated once at reader open from the window's monitor, in device pixels = logical px * `scale_factor`); a page taller than that is split into `ceil(h / optimal)` parts exactly as `ImageUtil.kt:281-282` does. (d) GTK units are already scale-independent: use logical px, no `dpToPx`. (e) not needed. (f) `Result<Texture, ResourceError>` from `gio::resources_lookup_data`.
* Difficulty trivial. Risk: multi-monitor: reading monitor of the window's surface, not the primary.

---

## 4. Network layer (OkHttp, Okio, RxJava)

### 4.1 Response
* Aniyomi: `okhttp3.Response` is the return type of `client.newCall(...).await()` and flows through the extension API (`HttpSource.popularMangaParse(response: Response)`, `searchMangaParse`, `latestUpdatesParse`, `mangaDetailsParse`, `chapterListParse`, `chapterPageParse`, `pageListParse`, `imageUrlParse`, all in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:132-361`, plus `open suspend fun getImage(page: Page): Response` at line 370). App-side users: `app/src/main/java/aniyomi/util/DataSaver.kt:30` (`HttpSource.getImage(page, dataSaver)`), `app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt:144` (`putImageToCache(imageUrl, response)`), `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt:170,220` and `AnimeImageFetcher.kt:171,221,256` (cover fetch + cover cache write). 27 files / 103 occ.
* Observed contracts to keep:
  * `isSuccessful` is HTTP 200..299. `Call.awaitSuccess()` closes the response and throws `HttpException(response.code)` otherwise (`core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt`, `awaitSuccess`). `Call.await()` returns any status.
  * Cover fetch accepts 304: `if (!response.isSuccessful && response.code != HTTP_NOT_MODIFIED) { response.close(); throw IOException(response.message) }` (`MangaCoverFetcher.kt:170-177`; constant `HTTP_NOT_MODIFIED = 304` at line 348). Error text is the reason phrase.
  * `response.networkResponse == null` means "served from OkHttp cache" and is used by `RateLimitInterceptor.kt` to refund the rate-limit slot (see SystemClock).
  * Cover requests set `Cache-Control: no-store` when the network cache policy allows reading, else `CACHE_CONTROL_NO_NETWORK_NO_CACHE` (which makes OkHttp synthesise `504 Unsatisfiable Request`) (`MangaCoverFetcher.kt:180-200`; `CACHE_CONTROL_NO_STORE = CacheControl.Builder().noStore().build()` and `CACHE_CONTROL_NO_NETWORK_NO_CACHE = CacheControl.Builder().noCache().onlyIfCached().build()` at lines 345-346). Source headers (`source.headers`) are attached to the request when the source is known.
  * `response.peekBody(Long.MAX_VALUE)` is used to copy the body into the cover cache without consuming it (`writeResponseToCoverCache`).
  * JSON helper: `Response.parseAs<T>()` decodes from the body stream and closes it (`decodeFromJsonResponse`, `OkHttpExtensions.kt`).
  * Extension parse functions read the body once; the body is closed by the caller after parse.
* Replacement: `reqwest::Response` plus `struct HttpError { status: StatusCode }` (`thiserror`, message `HTTP error {code}` to match `HttpException`). Wrapper `async fn await_success(req) -> Result<Response, HttpError>`; `fn parse_as<T: DeserializeOwned>(resp) -> Result<T>` using `resp.bytes()` then `serde_json::from_slice`. For the cover cache copy: read the body to `Bytes` once (covers are small), write to the cache file, and hand the same `Bytes` to the decoder (no peek needed). ETag/Last-Modified revalidation (the 304 case) is not provided by reqwest: Proposal: store `etag` and `last-modified` beside the cached cover file and send `If-None-Match` / `If-Modified-Since`; treat 304 as success and re-use the file.
* Preserve: success range, error text, 304 acceptance, per-source headers, and "close the body on error paths" (in Rust: drop).
* Difficulty moderate. Risk: extensions that run as Rust-side plugins will receive a different `Response` type; the source mechanism is specified in `area:12-network-stack`.

### 4.2 ResponseBody
* Aniyomi: `ProgressResponseBody(responseBody: ResponseBody, progressListener: ProgressListener) : ResponseBody()` (`core/common/src/main/java/eu/kanade/tachiyomi/network/ProgressResponseBody.kt:13-15`). It forwards `contentType()` and `contentLength()`, and wraps the source in a `ForwardingSource` that accumulates `totalBytesRead += bytesRead` (0 when `bytesRead == -1`) and calls `progressListener.update(totalBytesRead, responseBody.contentLength(), bytesRead == -1L)` on every `read`. It is installed as a network interceptor in `OkHttpClient.newCachelessCallWithProgress(request, listener)` (`OkHttpExtensions.kt`): `cache(null)`, `callTimeout(30, TimeUnit.HOURS)`. Used by the app updater (`app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt`). 1 file / 3 occ.
* Replacement: `async fn download_with_progress(client, req, on_progress: impl FnMut(u64, Option<u64>, bool))`. Use `resp.content_length()`, iterate `bytes_stream()`, write each chunk to a temp file, call `on_progress(total, length, false)` per chunk and `on_progress(total, length, true)` once at EOF. Per-request timeout: `reqwest::RequestBuilder::timeout(Duration::from_secs(30 * 3600))`; no HTTP cache in reqwest so "cacheless" is automatic.
* Preserve: the `done` flag fires exactly when the stream ends; `contentLength` of -1 maps to `None`.
* Difficulty moderate.

### 4.3 Source
* Aniyomi: `okio.Source` in `ProgressResponseBody.source(source: Source): Source` (line 33) and the cover-cache writers `writeSourceToCoverCache(input: Source, cacheFile: File)` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt:233`, `AnimeImageFetcher.kt:234`). The writer does `cacheFile.parentFile?.mkdirs(); cacheFile.delete(); try { sink.writeAll(input) } catch { cacheFile.delete(); throw }`: always replaces the file and removes the partial file on failure. 3 files / 6 occ.
* Replacement: `std::io::Read` / `tokio::io::AsyncRead`; `fn write_cover_cache(src: &mut impl Read, path: &Path) -> io::Result<()>` with `create_dir_all(parent)`, write to `path.with_extension("tmp")` then `rename` (atomic; stronger than the Kotlin behaviour), delete the temp on error.
* Difficulty trivial.

### 4.4 StreamResetException
* Aniyomi: `AppUpdateDownloadJob.kt:118`: `val shouldCancel = e is CancellationException || (e is StreamResetException && e.errorCode == ErrorCode.CANCEL)`; cancel means "user cancelled the download": the notifier is cancelled silently instead of showing a download error. 1 file / 2 occ.
* Replacement: a `CancellationToken` owned by the download task; cancellation is checked at each chunk and returns `DownloadError::Cancelled`, which the UI treats as "dismiss progress, no error toast". HTTP/2 `RST_STREAM(CANCEL)` from the server maps to a `reqwest::Error` and is a genuine error, which differs from OkHttp's client-initiated cancel and is acceptable.
* Difficulty trivial. Risk: do not treat server-side resets as cancels.

### 4.5 Subscriber
* Aniyomi: `core/common/src/main/java/tachiyomi/core/common/util/lang/RxCoroutineBridge.kt:22` implements `Observable<T>.awaitSingle()` by subscribing with a `Subscriber<T>` that requests 1 item, resumes the continuation on `onNext`, resumes with `IllegalStateException("Should have invoked onNext")` if completed without a value, and on error attempts an atomic `tryResumeWithException` (comment: "Rx1 observable throws NoSuchElementException if cancellation happened before element emission"). 1 file / 2 occ.
* Replacement: dropped. RxJava 1 exists only for the deprecated `fetch*` methods of the extension API (`HttpSource.fetchPopularManga(page): Observable<MangasPage>`, `fetchSearchManga`, `fetchLatestUpdates`, `fetchMangaDetails`, `fetchChapterList`, `fetchPageList`, `fetchImageUrl` in `HttpSource.kt:112-342`). Kurobayomi's source API is `async fn`. Users lose nothing unless a loaded extension only implements the deprecated Rx methods; see `area:12-network-stack` for the source ABI decision.
* Difficulty trivial.

### 4.6 Subscription
* Aniyomi: `OkHttpExtensions.kt:31` `Call.asObservable()` builds an `AtomicBoolean(), Producer, Subscription` arbiter: `request(n)` runs `call.execute()` once (guarded by `compareAndSet(false, true)`), `unsubscribe()` calls `call.cancel()`, `isUnsubscribed()` is `call.isCanceled()`. `asObservableSuccess()` adds the same `HttpException(response.code)` check as `awaitSuccess`. `RxCoroutineBridge.kt:57` `unsubscribeOnCancellation` wires coroutine cancellation to `Subscription.unsubscribe()`. 2 files / 4 occ.
* Replacement: dropped (see Subscriber). Cancellation in Rust is by dropping the future or a `CancellationToken`; reqwest aborts the in-flight request when the future is dropped.
* Difficulty trivial.

---

## 5. Markdown changelog rendering

### 5.1 RichText
* Aniyomi: `RichText(modifier, style = RichTextStyle(...)) { Markdown(content = changelogInfo); TextButton(...) }` in `app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt:45`. The screen is `InfoScreen` with headline `MR.strings.update_check_notification_update_available`, subtitle = version name, accept button `MR.strings.update_check_confirm`, reject `MR.strings.action_not_now`, and an "open in browser" button `MR.strings.update_check_open` with the `OpenInNew` icon placed after the markdown. The markdown is the GitHub release body. 1 file / 2 occ.
* Replacement: `pulldown-cmark` (CommonMark + tables/strikethrough/tasklist flags) rendered into a read-only `gtk::TextView` with a `gtk::TextTagTable`: tags `h1`..`h3` (scale 1.5 / 1.25 / 1.1, bold), `bold`, `italic`, `code` (monospace + `background`), `link` (underline + foreground), list items with a `"\u{2022} "` prefix and left margin 16 px per nesting level. Clicking a `link` tag opens `gio::AppInfo::launch_default_for_uri`. "Open in browser" button is a plain `gtk::Button`.
* Preserve: the version string is shown verbatim; the update dialog only appears when the checker finds a newer release (see `area:12-network-stack` for the checker; it lives in `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt`).
* Difficulty moderate. Note: for a Linux distribution build, the in-app self-updater should be replaced by package-manager updates (AUR); the changelog screen is then informational (P3).

### 5.2 RichTextStringStyle
* Aniyomi: `RichTextStringStyle(linkStyle = SpanStyle(color = MaterialTheme.colorScheme.primary))` (`NewUpdateScreen.kt:50`). 1 file / 2 occ. Only the link colour is customised.
* Replacement: the `link` `gtk::TextTag` takes its colour from the theme's link colour: `gtk::StyleContext` `lookup_color("link_color")` fallback `accent_color`/`theme_selected_bg_color`; re-read on theme change.
* Difficulty trivial.

### 5.3 RichTextStyle
* Aniyomi: `RichTextStyle(stringStyle = ...)` container at `NewUpdateScreen.kt:49`. 1 file / 2 occ. All other style fields are the library defaults.
* Replacement: the tag table of 5.1. Do not add a configuration struct: one function `build_changelog_tags()`.
* Difficulty trivial.

---

## 6. Home-screen widgets (Glance), Coil transformations

The module `presentation-widget` implements two Android home-screen widgets (updates grid for manga and for anime) with Jetpack Glance. Files: `presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt`, `.../entries/anime/BaseAnimeUpdatesGridGlanceWidget.kt`, `.../components/manga/UpdatesMangaWidget.kt`, `.../components/anime/UpdatesAnimeWidget.kt`, `.../util/GlanceUtils.kt`. Desktop Linux has no widget host.

Decision for all of 6.x: dropped. User loses: the home-screen "recent updates" cover grid and its tap-to-open-entry shortcut. Replacement for the capability: the in-app Updates tab (P1) and an optional `--open-entry <id>` CLI argument; a KDE Plasma widget is out of scope.

### 6.1 RoundedCornersTransformation
* Aniyomi: `.transformations(RoundedCornersTransformation(roundPx))` applied only when `Build.VERSION.SDK_INT < Build.VERSION_CODES.S` (system rounds corners on Android 12+) at `BaseMangaUpdatesGridGlanceWidget.kt:142` and `BaseAnimeUpdatesGridGlanceWidget.kt:142`; `roundPx` is `R.dimen.appwidget_inner_radius`. 2 files / 4 occ.
* Replacement: dropped for the widget. For in-app covers, rounded corners are CSS `border-radius` on the `gtk::Picture` container with `overflow: hidden`.
* Difficulty trivial.

### 6.2 Row
* Aniyomi: Glance `Row(modifier = GlanceModifier.padding(vertical = 4.dp).fillMaxWidth(), horizontalAlignment = CenterHorizontally, verticalAlignment = CenterVertically)` per cover row; each cell is a `Box` with `padding(horizontal = 3.dp)` and a click action starting `Constants.MAIN_ACTIVITY` with action `Constants.SHORTCUT_MANGA` (or anime), extra `Constants.MANGA_EXTRA` = id, flags `FLAG_ACTIVITY_NEW_TASK | FLAG_ACTIVITY_CLEAR_TOP`, and a per-id category as a workaround for `https://issuetracker.google.com/issues/238793260` (`UpdatesMangaWidget.kt:61-85`). 2 files / 4 occ.
* Replacement: dropped (see 6.x intro).
* Difficulty trivial.

### 6.3 SizeMode
* Aniyomi: `override val sizeMode = SizeMode.Exact` at `BaseMangaUpdatesGridGlanceWidget.kt:61` and the anime twin. Exact mode makes Glance re-render per widget size; the grid rows x columns are computed from the largest installed widget size (`maxBy { height * width }`, `calculateRowAndColumnCount(topPadding, bottomPadding)`), then `.distinctBy { mangaId }.take(rowCount * columnCount)` of updates newer than `DateLimit = ZonedDateTime.now().minusMonths(3)`; when app lock is enabled (`SecurityPreferences.useAuthenticator()`) a locked placeholder is shown. 2 files / 4 occ.
* Replacement: dropped. Note for the in-app Updates screen: the same 3-month `DateLimit` is the default window of the updates feed.
* Difficulty trivial.

---

## 7. Test and benchmark infrastructure (dropped)

Module `macrobenchmark` (`macrobenchmark/src/main/java/tachiyomi/macrobenchmark/BaselineProfileGenerator.kt`, `.../StartupBenchmark.kt`) measures Android cold/warm/hot startup and generates a baseline profile. No Linux counterpart is needed. Replacement for startup measurement: `tracing` spans around `AppContext::init` and first-window map, plus `cargo bench` (`criterion`) for the library query path if wanted.

### 7.1 Rule
* Aniyomi: `@get:Rule val rule = ...` in `BaselineProfileGenerator.kt:10` and `StartupBenchmark.kt:55`. 2 files / 4 occ.
* Replacement: dropped. Rust tests use per-test local fixtures (`tempfile::TempDir`) instead of JUnit rules.
* Difficulty trivial.

### 7.2 RunWith
* Aniyomi: `@RunWith(AndroidJUnit4ClassRunner::class)` on `ColdStartupBenchmark`, `WarmStartupBenchmark`, `HotStartupBenchmark` (`StartupBenchmark.kt:33,40,47`). 1 file / 4 occ.
* Replacement: dropped; `#[test]`.
* Difficulty trivial.

### 7.3 StartupMode
* Aniyomi: `StartupMode.COLD`, `.WARM`, `.HOT` passed to `AbstractStartupBenchmark(StartupMode)` (`StartupBenchmark.kt:34,41,48,54`). 1 file / 5 occ.
* Replacement: dropped. Cold/warm map loosely to "first launch of the process" vs "second `gio::Application` activation with a running primary instance"; no test required.
* Difficulty trivial.

### 7.4 StartupTimingMetric
* Aniyomi: `metrics = listOf(StartupTimingMetric())` (`StartupBenchmark.kt:79`). 1 file / 2 occ.
* Replacement: dropped; `tracing::info_span!("startup")` and a `--timings` CLI flag if ever wanted.
* Difficulty trivial.

### 7.5 SuppressLint
* Aniyomi: Android Lint suppressions, 26 files / 57 occ. Distinct values found: `RestrictedApi` (4), `UnsafeOptInUsageError` (3), `ViewConstructor` (2), `SetJavaScriptEnabled` (2; WebView screens), `PrivateApi` (2), `PackageManagerGetSignatures` (2; extension signature checking), `LaunchActivityFromNotification` (2; `app/src/main/java/eu/kanade/tachiyomi/App.kt:81`), `ClickableViewAccessibility` (2), `BatteryLife` (2; `app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt:105`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:164`), and one each of `UnusedBoxWithConstraintsScope`, `UnspecifiedRegisterReceiverFlag`, `UnrememberedMutableState`, `PrivateResource` (with `RestrictedApi`, `MonetColorScheme.kt:73`), `NewApi`, `MutableCollectionMutableState`, `DiscouragedApi`, `ComposeParameterOrder`.
* Replacement: dropped (annotations carry no runtime behaviour). The cases that signal real Android-only behaviour are tracked under their owning item: `BatteryLife` under Settings, `PackageManagerGetSignatures` (extension APK signature checks) in the extension chapter (`area:12-network-stack` covers extension repos; signature trust is replaced by signed manifests/fingerprints of repository entries, not APK signatures), `SetJavaScriptEnabled` in the WebView catalog chunk. Rust equivalent for silencing: `#[allow(...)]` when clippy needs it.
* Difficulty trivial.

---

## 8. Image loading (Coil 3)

Coil is the single image pipeline for covers, extension icons and reader pages. It is configured in `App.newImageLoader` (`app/src/main/java/eu/kanade/tachiyomi/App.kt:130-210`, via `SingletonImageLoader.Factory`).

### 8.1 SingletonImageLoader
* Aniyomi: `class App : Application(), DefaultLifecycleObserver, SingletonImageLoader.Factory` (`App.kt:74`). `newImageLoader(context)` builds one `ImageLoader` with components in this order: `OkHttpNetworkFetcherFactory(callFactoryLazy::value)` (network fetcher on the shared `NetworkHelper.client`), `TachiyomiImageDecoder.Factory()` (decoder), `BufferedSourceFetcher.Factory()`, `MangaCoverFetcher.MangaFactory`, `MangaCoverFetcher.MangaCoverFactory`, `AnimeImageFetcher.AnimeFactory`, `AnimeImageFetcher.AnimeCoverFactory`, keyers `AnimeKeyer`, `MangaKeyer`, `AnimeCoverKeyer`, `MangaCoverKeyer`. Options: `crossfade((300 * animatorDurationScale).toInt())` (300 ms), `allowRgb565(DeviceUtil.isLowRamDevice(app))`, `DebugLogger` when `networkPreferences.verboseLogging()`, `fetcherCoroutineContext(Dispatchers.IO.limitedParallelism(8))`, `decoderCoroutineContext(Dispatchers.IO.limitedParallelism(3))` (comment: "Coil spawns a new thread for every image load by default"). 1 file / 2 occ.
* Replacement: `struct ImageLoader` in `AppContext`: `tokio::sync::Semaphore::new(8)` around fetches, `Semaphore::new(3)` around decodes (decode on `tokio::task::spawn_blocking`), an in-memory LRU of `gdk::Texture` (`lru` crate; Proposal: 64 MiB budget counted as `w*h*4`), a disk cache directory `$XDG_CACHE_HOME/kurobayomi/covers`, and a keyer. Crossfade: `gtk::Picture` inside a `gtk::Overlay`, fading opacity 0 to 1 over 300 ms via `adw`-free CSS `transition: opacity 300ms`; skip when `gtk::Settings::is_gtk_enable_animations()` is false (see Settings 10.5). `allowRgb565` dropped (desktop RAM).
* Keyers (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt:13-30`): for a domain manga with a custom cover the key is `"manga;${id};${coverLastModified}"`, otherwise `"manga;${thumbnailUrl};${coverLastModified}"`; for a `MangaCover` the key is `"manga;${mangaId};${lastModified}"` when the custom cover file exists, else `"manga;${url};${lastModified}"`; the anime keyers (`AnimeCoverKeyer.kt`) mirror this. Proposal: reuse exactly these strings as logical keys and hash them (`blake3` hex) for cache file names, so a changed `lastModified` invalidates the cached texture.
* Difficulty moderate. Risk: no widget may block the GTK main thread on decode; hand textures back with `glib::MainContext::invoke`.

### 8.2 SourceFetchResult
* Aniyomi: the return value of every Coil `Fetcher.fetch()`: `SourceFetchResult(source = ImageSource(...), mimeType = ..., dataSource = DataSource.DISK|MEMORY)`. 4 files / 16 occ. Variants:
  * `MangaCoverFetcher.fileLoader(file)` (`MangaCoverFetcher.kt:80-95`): `ImageSource(file = file.toOkioPath(), fileSystem = FileSystem.SYSTEM, diskCacheKey = diskCacheKey)`, `mimeType = "image/*"`, `DataSource.DISK`.
  * `uniFileLoader(urlString)` for `content://` URIs: opens `UniFile.fromUri(...)`, wraps the input stream as `ImageSource(source = ..., fileSystem = FileSystem.SYSTEM)`, `DataSource.DISK`.
  * `httpLoader()`: if library manga, reads the cover cache file; else reads the Coil disk cache snapshot, else performs `executeNetworkRequest()` and writes via `writeToDiskCache` (`diskCache.openEditor(key)`, write, `commitAndOpenSnapshot()`, `abort()` on failure) and, for library items, `moveSnapshotToCoverCache` which copies the snapshot to the library cover file, removes the snapshot, and returns the cover file.
  * `BufferedSourceFetcher` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/BufferedSourceFetcher.kt:18`): wraps an in-memory `BufferedSource` (a reader page held in RAM), `mimeType = null`, `DataSource.MEMORY`.
  * `AnimeImageFetcher.kt:87,95,128,148,156`: the anime cover/screenshot twins (same structure).
* Cover source resolution (`MangaCoverFetcher.kt:getResourceType`): empty/null: error `"No cover specified"`/`"Invalid image"`; starts with `http` (case-insensitive) or `Custom-`: network; starts with `/` or `file://`: local file (`url.substringAfter("file://")`); starts with `content`: URI; else invalid. Custom cover (`USE_CUSTOM_COVER_KEY`, default `true`) wins over everything when its file exists.
* Replacement: `enum ImageBytes { File(PathBuf), Memory(Bytes) }` returned by `trait CoverFetcher`; implementation `fn resolve_cover(url: &str, is_library: bool, use_custom: bool) -> Result<ImageBytes>`. `content://` is dropped (Android storage-access-framework): `file://` and absolute paths cover Linux.
* Preserve: precedence (custom cover, then library cover cache, then disk cache, then network); library covers survive a cache clear (they live in the cover cache, not the disposable image cache); partial files are deleted on error.
* Difficulty moderate.

### 8.3 Size
* Aniyomi: `Size.ORIGINAL` for full-size cover/image viewing (`app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt:172`, `.../anime/components/AnimeImagesDialog.kt:212`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt:96`, `.../anime/AnimeImageScreenModel.kt:119`) and, in `app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt:13-24`, `Size.widthPx(scale, original: () -> Int)` and `heightPx`: when `isOriginal` return the supplied original dimension, else `width.toPx(scale)`. 5 files / 11 occ.
* Replacement: `enum TargetSize { Original, Px(u32, u32) }`; `fn width_px(size, scale, original) -> u32`.
* Difficulty trivial.

### 8.4 Scale
* Aniyomi: `Scale.FILL` / `Scale.FIT` in `Utils.kt:21-24`: `Dimension.toPx(scale)` = pixel value if defined, otherwise `Int.MIN_VALUE` for `FILL` and `Int.MAX_VALUE` for `FIT` (an undefined dimension means "unbounded" for fit and "unconstrained minimum" for fill). Also `.scale(Scale.FILL)` in the widget request (`BaseMangaUpdatesGridGlanceWidget.kt:139`). 3 files / 10 occ.
* Replacement: `enum ScaleMode { Fit, Fill }` with the same undefined-dimension rule (`u32::MAX` for Fit, `0` for Fill).
* Difficulty trivial.

### 8.5 ScaleDrawable
* Aniyomi: `app/src/main/java/eu/kanade/tachiyomi/util/system/DrawableExtensions.kt:11`: `fun Drawable.getBitmapOrNull(): Bitmap? = when (this) { is BitmapDrawable -> bitmap; is ScaleDrawable -> child.toBitmap(); else -> null }`. Coil 3 wraps a result in `ScaleDrawable` when it rescales for the target. 1 file / 2 occ.
* Replacement: dropped; the loader returns a `gdk::Texture` directly (see Bitmap in the chunk-1 catalog `android-api-mapping-01.md`).
* Difficulty trivial.

---

## 9. Reader page view and gestures

### 9.1 SubsamplingScaleImageView
* Aniyomi: the tiled, zoomable, pannable image view used for every non-animated reader page. 4 files / 27 occ. Uses, all in `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt` unless noted:
  * Construction (`prepareNonAnimatedImageView`, line 247): `setMaxTileSize(ImageUtil.hardwareBitmapThreshold)`: the value is pref `pref_hardware_bitmap_threshold` (`BasePreferences.kt:40`, default `GLUtil.SAFE_TEXTURE_LIMIT = 2048`, `core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt:50`), initialised on first run to `GLUtil.DEVICE_TEXTURE_LIMIT` = `max(largest EGL pbuffer size, 2048)` (`App.kt:141-143`, `GLUtil.kt:47`) and mirrored into `ImageUtil.hardwareBitmapThreshold` on change (`App.kt:145-146`; editable in `SettingsAdvancedScreen.kt:366`), `setDoubleTapZoomStyle(ZOOM_FOCUS_CENTER)`, `setPanLimit(PAN_LIMIT_INSIDE)`, `setMinimumTileDpi(180)`, state listener forwards `onScaleChanged(newScale, origin)`, click listener forwards `onViewClicked()`. In webtoon mode the subclass `WebtoonSubsamplingImageView` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonSubsamplingImageView.kt`) overrides `onTouchEvent` to `return false` (all gestures belong to the webtoon recycler).
  * Per-image config (`setNonAnimatedImage`, line 289): `setDoubleTapZoomDuration(zoomDuration scaled)` (pref `pref_double_tap_anim_speed`, default 500 ms, `ReaderPreferences.kt:26`), `setMinimumScaleType(config.minimumScaleType)`, `setMinimumDpi(1)` ("Just so that very small image will be fit for initial load"), `setCropBorders(config.cropBorders)`. `onReady`: `setupZoom(config)`, `landscapeZoom(true)` when visible, then `onImageLoaded()`.
  * `setupZoom`: `maxScale = scale * MAX_ZOOM_SCALE` with `private const val MAX_ZOOM_SCALE = 5F` (file end), `setDoubleTapZoomScale(scale * 2)`; start position: `ZoomStartPosition.LEFT` centres on `PointF(0, 0)`, `RIGHT` on `PointF(sWidth, 0)`, `CENTER` on the current centre. Pref `pref_zoom_start_key` default 1; options in order `zoom_start_automatic`, `zoom_start_left`, `zoom_start_right`, `zoom_start_center` (`ReaderPreferences.kt:219-224`).
  * `landscapeZoom(forward)` (lines 125-160): applies only if `config.landscapeZoom` (pref `landscape_zoom`, default `true`), `minimumScaleType == SCALE_TYPE_CENTER_INSIDE`, `sWidth > sHeight` and `scale == minScale`; after `postDelayed(500)` it animates to `targetScale = height / sHeight` centred at: LEFT: forward `(0,0)`, backward `(sWidth, 0)`; RIGHT: forward `(sWidth, 0)`, backward `(0,0)`; CENTER: the view centre; animation 500 ms, `EASE_IN_OUT_QUAD`, interruptible.
  * Panning helpers: `canPanLeft`/`canPanRight` use `getPanRemaining(RectF)` and test `value > 1` px; `panLeft`/`panRight` move the centre by `view.width / view.scale` (one screen width in image space) with `animateCenter(...).withEasing(EASE_OUT_QUAD).withDuration(250).withInterruptible(true)`.
  * Source of pixels: `BitmapDrawable` becomes `ImageSource.bitmap`; a `BufferedSource` becomes `ImageSource.inputStream(data.inputStream())` after `setHardwareConfig(ImageUtil.canUseHardwareBitmap(data))` (hardware bitmaps only when `max(w,h) <= hardwareBitmapThreshold`) when the page is not webtoon or pref `pref_always_decode_long_strip_with_ssiv` (`BasePreferences.kt:42`, default `false`) is on; otherwise (webtoon) the page is decoded by Coil with `customDecoder(true)`, `cropBorders(config.cropBorders)`, `Precision.INEXACT`, size = view size, caches disabled, and handed over as `ImageSource.bitmap`.
  * `recycle()` releases tiles when the page leaves the window.
  * Display profile: `SubsamplingScaleImageView.setDisplayProfile(data)` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:906`, and `TachiyomiImageDecoder.displayProfile = data`) loads the ICC profile file selected by pref `pref_display_profile_key` (default `""`, `BasePreferences.kt:38`) and applies it to every decoded page.
  * Scale types: `pref_image_scale_type_key` default 1; list order `scale_type_fit_screen`, `scale_type_stretch`, `scale_type_fit_width`, `scale_type_fit_height`, `scale_type_original_size`, `scale_type_smart_fit` (`ReaderPreferences.kt:220-227`); the chip row stores `index + 1` (`app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt:84-88`) and the value is passed straight to `setMinimumScaleType`. Webtoon pages are always `SCALE_TYPE_FIT_WIDTH` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt:201`). The numeric constants of the forked library are not in this repository (binary dependency).
* Replacement: a custom `gtk::Widget` `ZoomableImage` (subclass via `glib::subclass`) owning a decoded `image::DynamicImage` or a `Vec<Tile>` of `gdk::MemoryTexture` tiles, rendered in `snapshot()` with `gtk::Snapshot::push_clip`/`append_texture` under a scale+translate transform. Input: `gtk::GestureZoom` (pinch), `gtk::GestureDrag` (pan), `gtk::GestureClick` n_press 2 (double tap: toggle between `min_scale` and `min_scale * 2` centred on the click; 500 ms ease-in-out-quad via `adw`-free `gtk::Widget::add_tick_callback` + `gdk::FrameClock`), `gtk::EventControllerScroll` (Ctrl+wheel zoom, plain wheel pans), `gtk::EventControllerKey`. Pan limit inside: clamp the centre so the scaled image never leaves the viewport (`PAN_LIMIT_INSIDE`). `max_scale = min_scale * 5`. Tiling: because GTK textures above the GL texture limit fall back to slow paths, Proposal: tile any image whose longer side exceeds 4096 px into 1024 x 1024 `gdk::MemoryTexture`s decoded on `spawn_blocking`, keep at most the visible tiles plus a 1-tile margin; images at or below 4096 px are one texture. The pref `pref_always_decode_long_strip_with_ssiv` becomes inert (webtoon always uses the strip path). Crop borders: scan decoded pixels for uniform margins (area:43-reader-core owns the algorithm). ICC display profile: apply with `lcms2` (crate) at decode when the pref is set, else no colour management (GTK has none).
* Preserve: 5x max zoom; double-tap target `scale * 2`; zoom start LEFT/CENTER/RIGHT; landscape auto-zoom conditions and the 500 ms delay and 500 ms animation; `canPan*` threshold of 1 px; pan step of exactly one viewport width; tile-based memory use for tall pages.
* Difficulty hard. Risks: smooth pinch-zoom of 10k-px images; HiDPI scale factors (tile sizes in device px); Wayland has no global pointer position for gesture centres, use `GestureZoom::bounding_box_center`.

### 9.2 SCALE_TYPE_CENTER_INSIDE
* Aniyomi: `import com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView.SCALE_TYPE_CENTER_INSIDE` (`ReaderPageImageView.kt:34`); used at line 128 (`config!!.minimumScaleType == SCALE_TYPE_CENTER_INSIDE` gates landscape zoom) and line 429 (`val minimumScaleType: Int = SCALE_TYPE_CENTER_INSIDE` default of `Config`). In the preference list it is value 1, "Fit screen". 1 file / 3 occ.
* Replacement: `enum ImageScaleType { FitScreen = 1, Stretch = 2, FitWidth = 3, FitHeight = 4, OriginalSize = 5, SmartFit = 6 }` (`#[repr(u8)]`, serde as integer for settings compatibility; `FitScreen` is `Default`). Landscape zoom gate: `scale_type == FitScreen`.
* Difficulty trivial.

### 9.3 ScaleGestureDetector
* Aniyomi: `WebtoonFrame` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt:22,76-87`): `ScaleGestureDetector(context, ScaleListener())` forwards `onScaleBegin` to `recycler.onScaleBegin()` (if `detector.isDoubleTapping` then `isQuickScaling = true`), `onScale` to `recycler.onScale(detector.scaleFactor)`, `onScaleEnd` to `recycler.onScaleEnd()`. Quick scale (double-tap-and-drag zoom) is enabled by `scaleDetector.isQuickScaleEnabled = doubleTapZoom` (pref default `true`). `WebtoonRecyclerView.onScale` (`WebtoonRecyclerView.kt:184-205`): `currentScale *= scaleFactor`, clamped to `[minRate, MAX_SCALE_RATE]` where `MIN_RATE = 0.5f`, `DEFAULT_RATE = 1f`, `MAX_SCALE_RATE = 3f` (`WebtoonRecyclerView.kt:347-349`) and `minRate = if (zoomOutDisabled) DEFAULT_RATE else MIN_RATE`; applies `scaleX = scaleY = currentScale`; when `currentScale < 1` the layout height becomes `originalHeight / currentScale` (so the strip still fills the screen), else `originalHeight`; positions are re-clamped, and at exactly 1 the translation resets to `(0, 0)`. Zoom animation duration `ANIMATOR_DURATION_TIME = 200` ms. 1 file / 6 occ.
* Replacement: a `gtk::GestureZoom` on the webtoon container gives the `scale-changed` ratio since gesture begin: convert to incremental factor `s_now / s_prev`. Quick scale (double-tap then drag) is replaced by Ctrl+scroll wheel and double-click toggle (desktop mice have no double-tap-drag). Keep constants 0.5 / 1.0 / 3.0 and 200 ms.
* Preserve: clamp range, the height compensation below 1x, reset at 1x, `zoomOutDisabled` pref raising the minimum to 1.
* Difficulty moderate. Risk: GTK scroll offsets live in `gtk::Adjustment`s; implement zoom as a transform on a `gtk::Viewport` child with `gtk::ScrolledWindow` kinetic scrolling disabled while zoomed, or write a custom scroller. Specified in `area:43-reader-core`.

---

## 10. Platform services, settings, storage

### 10.1 SavedStateHandle
* Aniyomi: process-death restoration of the reader and player: `ReaderViewModel` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:88,123-134`) stores `"chapter_id"` (Long, default -1) and `"page_index"` (Int, default -1); `PlayerViewModel` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:155,1120-1140`) stores `"episode_position"` (Long), `"quality_index"` (`Pair<Int, Int>`, default `Pair(-1, -1)`) and `"episode_id"` (Long, default -1). Each is a property whose setter writes `savedState[key] = value`. 2 files / 4 occ.
* Replacement: ordinary struct fields. A desktop process is not killed behind the user's back. Resume-after-crash is covered by progress persistence: the reader writes `last_page_read` to the chapters table and history on page change, and the player writes `last_second_seen` (see `area:43-reader-core`, `area:45-player-core`). Proposal: no separate session file (P3).
* Difficulty trivial. User loses: nothing visible on desktop.

### 10.2 SearchManager
* Aniyomi: `val query = intent.getStringExtra(SearchManager.QUERY) ?: intent.getStringExtra(Intent.EXTRA_TEXT)` in `MainActivity.handleIntentAction` (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:502`), within the branch `Intent.ACTION_SEARCH, Intent.ACTION_SEND, "com.google.android.gms.actions.SEARCH_ACTION"` ("triggered by saying or typing 'search *query* on *Tachiyomi*' in Google Search/Google Assistant"). If the query is non-empty: `navigator.popUntilRoot()`, read `INTENT_SEARCH_TYPE` (blank defaults to `"ANIME"`, parsed with `DeepLinkScreenType.valueOf`), then push `GlobalMangaSearchScreen(query)` followed by `DeepLinkMangaScreen(query)` (anime analog). 1 file / 2 occ.
* Replacement: CLI `kurobayomi --search "<query>" [--type manga|anime]` and `gio::Application` `command-line` signal (single-instance: the second process forwards to the running one). Default type `anime`, same as the Kotlin default. Sharing text into the app (`ACTION_SEND`) is replaced by `kurobayomi --search "$(wl-paste)"` or a `.desktop` action `Exec=kurobayomi --search %u`. Google Assistant integration is dropped (user loses voice search).
* Difficulty trivial.

### 10.3 Service
* Aniyomi: foreground services. 10 files / 22 occ:
  * `HttpServerService` (`app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt:26`): starts a source-provided local HTTP server (`(sourceManager.get(sourceId) as? AnimeHttpSource)?.createHttpServer()`), stores `port = httpServer.listeningPort`, publishes `isRunning: StateFlow<Boolean>`, ongoing notification `http_server_is_running` with Close action, returns `START_STICKY`; `ACTION_STOP` stops it; failures log "Failed to start http server" and stop.
  * `TorrentServerService` (`app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt:34`): starts the embedded TorrServer (Go library `TorrServer.startServer(port, path = filesDir, proxyMode, proxyUrl)`), then `api.setPort(port)`, waits 10, calls `torrentServerUtils.setTrackersList()`; `ACTION_START` / `ACTION_STOP`; the user's torrent port and proxy mode come from `TorrentPreferences`.
  * `AnimeExtensionInstallService` / `MangaExtensionInstallService` (`app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt:22`, manga twin line 22) plus the `Installer*` classes that take a `Service` (`InstallerAnime.kt:20`, `PackageInstallerInstallerAnime.kt:19`, `ShizukuInstallerAnime.kt:21` and manga twins): queue APK installs; show an ongoing "installing" notification (`ext_install_service_notif`); `onStartCommand` requires data URI, download id and installer enum (`BasePreferences.ExtensionInstaller`: `LEGACY`, `PACKAGEINSTALLER`, `SHIZUKU`, `PRIVATE`, `BasePreferences.kt:31-36`; the service builds only `PACKAGEINSTALLER` and `SHIZUKU` installers and logs "Not implemented for installer" otherwise), otherwise `stopSelf()`.
* Replacement: there is no service layer. Long-running work is `tokio::spawn` tasks owned by `AppContext`, observable through `tokio::sync::watch<…>` channels the GTK side polls. Torrent streaming: spawn the `torrserver` binary (or a libtorrent-rs session) as a child process (`tokio::process::Command`), kill on drop; the HTTP server for sources becomes an in-process `hyper`/`tiny_http` server on `127.0.0.1:0`. Extension install services are dropped: a "install" is writing the extension package into `$XDG_DATA_HOME/kurobayomi/extensions` (see `area:12-network-stack`); Shizuku and the Android PackageInstaller do not exist.
* Preserve: the stop semantics (idempotent start replacing a running server; `isRunning` exposed as an observable), the 10-second wait before setting the tracker list for torrents.
* Difficulty moderate.

### 10.4 ServiceInfo
* Aniyomi: `ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC` passed into `ForegroundInfo` for every long-running WorkManager job when `SDK_INT >= Q`, else `0`: `BackupCreateJob.kt:73`, `BackupRestoreJob.kt:68`, `AnimeDownloadJob.kt:49`, `MangaDownloadJob.kt:49`, `AnimeLibraryUpdateJob.kt:139`, `MangaLibraryUpdateJob.kt:145`, `AnimeMetadataUpdateJob.kt:80`, `MangaMetadataUpdateJob.kt:86`, `AppUpdateDownloadJob.kt:66`, plus the two services at `TorrentServerService.kt:133` and `HttpServerService.kt:92` (all under `app/src/main/java/eu/kanade/tachiyomi/data/...`). 11 files / 22 occ.
* Replacement: dropped. While a job runs the app calls `gio::Application::hold()` / `release()` (or `inhibit` with `gtk::ApplicationInhibitFlags::SUSPEND`/`IDLE` for long downloads) so the process is not closed when the last window is closed; progress notifications use `gio::Notification` (see `area:23-downloads-manga` for download progress).
* Difficulty trivial.

### 10.5 Settings
* Aniyomi: `android.provider.Settings` is used for system-settings deep links and two system values (7 files / 21 occ.; matches in `Icons.Outlined.Settings` at `AnimeExtensionDetailsScreen.kt:442`/`MangaExtensionDetailsScreen.kt:438` and `SearchableSettings.highlightKey` at `app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt:47` are unrelated name collisions):
  * `Settings.ACTION_APPLICATION_DETAILS_SETTINGS` for the extension "App info" button (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt:184`, manga twin line 185).
  * `Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` (`app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt:106`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:166`).
  * `Settings.ACTION_APP_NOTIFICATION_SETTINGS` with `Settings.EXTRA_APP_PACKAGE` (`SettingsAdvancedScreen.kt:131-132`).
  * `Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES` (`app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:175`).
  * `Settings.Global.ANIMATOR_DURATION_SCALE` default `1f`: `Context.animatorDurationScale` (`app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt:11`), multiplied into durations (crossfade 300 ms in `App.kt`, reader double-tap zoom duration in `ReaderPageImageView.kt:420-422` `(this * animatorDurationScale).toInt().coerceAtLeast(1)`).
  * `Settings.System.SCREEN_BRIGHTNESS` read as a float and normalised `normalize(0f, 255f, 0f, 1f)` for the player's initial brightness slider, fallback `0f` on any exception (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:288`).
* Replacement:
  * App info, battery optimisation, notification settings, unknown sources: dropped (no Linux concept). User loses nothing; the equivalent notification control is the desktop environment's own per-app notification settings.
  * Animator scale: `gtk::Settings::default().is_gtk_enable_animations()`; scale = `1.0` if true, `0.0` if false; keep the `coerceAtLeast(1)` rule so a duration never becomes 0 ms for gesture animations.
  * Brightness: the player slider controls mpv's `brightness` property (range -100..100) rather than the display backlight; the initial value is `0` (mpv neutral), mapping the slider's 0..1 to -100..100. Display backlight control (read `/sys/class/backlight/*/brightness` / `max_brightness`, write via `org.freedesktop.login1.Session.SetBrightness` over `zbus`) is optional P3, off by default because it changes global state.
* Difficulty moderate (mostly decisions). Risk: do not write system brightness from a video player on desktop.

### 10.6 StatFs
* Aniyomi: `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:53,65,77`: `getTotalStorageSpace(file) = blockCountLong * blockSizeLong`, `getAvailableStorageSpace(file) = availableBlocksLong * blockSizeLong` (also a `UniFile` overload using `StatFs(f.uri.path)`), every call wrapped in `try { ... } catch (_: Exception) { -1L }`. Used for the "low storage" checks before downloads and the storage summary in settings. 1 file / 4 occ.
* Replacement: `rustix::fs::statvfs(path)`: total = `f_blocks * f_frsize`, available = `f_bavail * f_frsize` (unprivileged space, equals Android `availableBlocks`). Return `-1` as `Option<u64>::None` on error. Proposal: `fn total_space(p: &Path) -> Option<u64>`, `fn available_space(p: &Path) -> Option<u64>`; callers treat `None` as "unknown, do not block".
* Difficulty trivial.

### 10.7 StructStat
* Aniyomi: `core/archive/src/main/kotlin/mihon/core/archive/ZipWriter.kt:57-73` converts `android.system.StructStat` (from `Os.fstat(fd)`) into libarchive's `ArchiveEntry.StructStat` field by field (`stDev, stMode, stNlink, stUid, stGid, stRdev, stSize, stBlksize, stBlocks, stAtim, stMtim, stCtim, stIno`; `Long.toTimespec()` sets only `tvSec`). The writer (`ZipWriter.kt`) uses libarchive: `Archive.setCharset(archive, UTF-8)`, `writeSetFormatZip`, `writeZipSetCompressionStore` (STORE, no compression, since images are already compressed), `writeOpenFd`; per file: `ArchiveEntry.setPathnameUtf8(entry, file.name)`, `setStat`, `writeHeader`, loop `Os.read(fd, buffer)` / `writeData` until a read returns 0 bytes, `writeFinishEntry`. This produces the `.cbz` archives of downloaded chapters. 1 file / 2 occ.
* Replacement: the `zip` crate: `ZipWriter::new(file)`, `FileOptions::default().compression_method(CompressionMethod::Stored)`, `start_file(name, options)` with the file's mtime from `std::fs::metadata().modified()` converted via `zip::DateTime::try_from(time::OffsetDateTime)`, `std::io::copy` per file, `finish()`. UTF-8 names are the default (set the language-encoding flag). Unix mode from `PermissionsExt::mode()` via `options.unix_permissions(mode)`. Only mtime matters to readers; the other `stat` fields are dropped.
* Preserve: STORE method, UTF-8 pathnames, file order = page order (callers add `001.jpg`... in sequence), and ComicInfo.xml included as a normal entry (see `area:23-downloads-manga`).
* Difficulty trivial.

### 10.8 SystemBarStyle
* Aniyomi: edge-to-edge setup in `MainActivity.kt:192-199`: `lightStyle = SystemBarStyle.light(Color.TRANSPARENT, Color.BLACK)`, `darkStyle = SystemBarStyle.dark(Color.TRANSPARENT)`, `enableEdgeToEdge(statusBarStyle = if (statusBarBackgroundColor.luminance() > 0.5) lightStyle else darkStyle, navigationBarStyle = if (isSystemInDarkTheme) darkStyle else lightStyle)`. The status-bar background colour itself encodes app state: indexing banner, download-only banner, incognito banner, else surface (`MainActivity.kt:180-190`). 1 file / 3 occ.
* Replacement: dropped (no system bars on desktop). The three state banners (indexing downloads, "downloaded only", "incognito") become a coloured `gtk::Revealer` strip at the top of the main window (`.banner-indexing`, `.banner-downloaded-only`, `.banner-incognito` CSS classes, colours taken from the GTK theme: warning, accent, and dark respectively).
* Difficulty trivial.

### 10.9 SplashScreen
* Aniyomi: `androidx.core.splashscreen.SplashScreen` at `MainActivity.kt:162,318-322,424-440`: `installSplashScreen()` only on first launch (`isLaunch`); `setKeepOnScreenCondition { elapsed <= SPLASH_MIN_DURATION || !ready && elapsed <= SPLASH_MAX_DURATION }` with `SPLASH_MIN_DURATION = 500` ms and `SPLASH_MAX_DURATION = 5000` ms (`MainActivity.kt:665-666`); a custom exit animation (only below Android 12): the content slides by `16.dpToPx` while the splash fades, both `SPLASH_EXIT_ANIM_DURATION = 400L` ms (`MainActivity.kt:667`) with `LinearOutSlowInInterpolator` / `FastOutSlowInInterpolator`. 1 file / 4 occ.
* Replacement: dropped. GTK shows the window immediately; to avoid a flash of an empty library, `present()` the main window only after the library first query has returned or 500 ms elapsed (whichever is later) but never later than 5000 ms. This keeps both timings as a startup contract (P2).
* Difficulty trivial.

### 10.10 SurfaceColors
* Aniyomi: `SurfaceColors.SURFACE_2.getColor(this)` blended with alpha `if (isNightMode()) 230 else 242` (90% dark, 95% light) as the reader's toolbar/status-bar/navigation-bar colour (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:499-507`). 1 file / 2 occ.
* Replacement: reader toolbar uses a CSS class `.reader-toolbar { background-color: alpha(@theme_bg_color, 0.95); }` with `0.90` under the dark theme (`gtk::Settings::is_gtk_application_prefer_dark_theme()` or colour luminance < 0.5). System bar colouring is dropped.
* Difficulty trivial.

### 10.11 SchemeContent
* Aniyomi: `app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt:75`: builds a Material 3 content colour scheme from a seed colour: `SchemeContent(Hct.fromInt(seed), dark, contrast)` where contrast = `UiModeManager.contrast` on Android 14+ else `0.0`, then maps `MaterialDynamicColors` roles (primary, onPrimary, primaryContainer, ..., surfaceContainerLowest; `scrim = Color.Black`; note the original maps `tertiaryContainer` from `dynamicColors.tertiary()`, an upstream quirk) to a Compose `ColorScheme`. It is the fallback for Android 8.1-11 (wallpaper-colour based theming); Android 12+ uses the system `dynamicLightColorScheme`. 1 file / 2 occ.
* Replacement: dropped. Kurobayomi uses the user's GTK theme (no libadwaita) so Monet/dynamic colour does not exist. The "Monet" `AppTheme` option is not offered; the `AppTheme` enum (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt:7-28`) lists `DEFAULT, MONET, CLOUDFLARE, COTTONCANDY, DOOM, GREEN_APPLE, LAVENDER, MATRIX, MIDNIGHT_DUSK, MOCHA, SAPPHIRE, NORD, STRAWBERRY_DAIQUIRI, TAKO, TEALTURQUOISE, TIDAL_WAVE, YINYANG, YOTSUBA, MONOCHROME` plus the deprecated `DARK_BLUE, HOT_PINK` (null title); a stored `MONET`, deprecated or unknown value falls back to `DEFAULT`.
* Difficulty trivial. User loses: wallpaper-derived colours.

### 10.12 Score
* Aniyomi: `Score.score(QuantizerCelebi.quantize(bitmapPixels, 128), 1, 0)[0].takeIf { it != 0 }` in `MonetColorScheme.extractSeedColorFromImage(bitmap)` (`MonetColorScheme.kt:53`), annotated `@Suppress("Unused")`: it would extract a theme seed colour from an image (cover-based theming) with 128 quantised colours. Nothing calls it. 1 file / 2 occ.
* Replacement: dropped (dead code). If cover-coloured detail pages are ever wanted: `color-thief`/`palette` crates.
* Difficulty trivial.

---

## 11. Preferences and database

### 11.1 SharedPreferences
* Aniyomi: Android key-value store. 8 files / 46 occ. Roles:
  * App preference store: `AndroidPreferenceStore(context, sharedPreferences = PreferenceManager.getDefaultSharedPreferences(context))` (`core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt:13-18`) with typed accessors `getString`, `getLong`, `getInt`, `getFloat`, `getBoolean`, `getStringSet`, `getObject(key, default, serializer, deserializer)`, `getAll()`.
  * Typed wrapper `AndroidPreference<T>` (`.../AndroidPreference.kt:17-210`): `get()` returns the stored value or the default; on `ClassCastException` it logs "Invalid value for $key; deleting", deletes the key and returns the default; `set`, `isSet()` (`contains`), `delete()`, `defaultValue()`, `changes(): Flow<T>` = key-change flow filtered `it == key || it == null` (null means `clear()`), `onStart { emit("ignition") }` then `map { get() }.conflate()`; `stateIn(scope)` = eager `StateFlow`. `Object<T>.read` returns the default on any deserialiser exception; stored as a string.
  * Other named files: `"active_downloads"` (download queue; key = chapter/episode id as string, value = JSON `DownloadObject(mangaId, chapterId, order)` with `order = counter++`, restored sorted by `order`, entries whose manga/source/chapter no longer resolve are skipped, store cleared after restore; `MangaDownloadStore.kt:32,86-135`), `"chapters_to_delete"` (pending deleter, `MangaDownloadPendingDeleter.kt:26`), `"tracking_queue"` (delayed tracking: key = trackId string, value = last chapter read as float string; `DelayedMangaTrackingStore.kt:13-45`), `"source_<id>"` (per-extension settings: `ConfigurableSource.preferenceKey() = "source_$id"`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt:15-29`, MODE_PRIVATE), and the Suwayomi tracker which reuses `source_<sourceId>` where `sourceId` = first 8 bytes (big-endian) of `MD5("tachidesk/en/1")` AND `Long.MAX_VALUE` (`app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt:104-111`) with keys `"Server URL Address"`, `"Login (Basic Auth)"`, `"Password (Basic Auth)"` (defaults empty).
  * Migrations read raw prefs (23 of the 46 files under `app/src/main/java/mihon/core/migration/migrations/` touch `SharedPreferences`/`PreferenceManager`, e.g. `MigrateToTriStateMigration.kt:43-50`: old boolean true becomes 1, false becomes 0). Preference JSON in backups is produced from `getAll()` (see `area:22-backup-restore`).
  * `SharedPreferencesDataStore` (`app/src/main/java/eu/kanade/tachiyomi/data/preference/SharedPreferencesDataStore.kt:7`) adapts SharedPreferences to AndroidX `PreferenceDataStore` for the settings UI (get/put Boolean, Int, Long, Float, String, StringSet).
* Replacement: `struct PrefStore` backed by a SQLite table `prefs(file TEXT, key TEXT, type TEXT, value TEXT, PRIMARY KEY (file, key))` in the app database (single atomic transaction per write, WAL), with an in-memory `HashMap<(file,key), Value>` cache and `tokio::sync::broadcast::Sender<(file, Option<key>)>` as the change flow. `enum Value { Str(String), Long(i64), Int(i32), Float(f32), Bool(bool), StrSet(BTreeSet<String>) }`. API: `Pref<T>` with `get() / set() / is_set() / delete() / default() / changes() -> impl Stream<Item = T>` (stream yields the current value first, matching the "ignition"). Type mismatch on read (stored `Int`, requested `Bool`): delete key and return default, with `tracing::warn!("Invalid value for {key}; deleting")`. Files: `default` (the app preferences), `active_downloads`, `chapters_to_delete`, `tracking_queue`, `source_<id>` (extension prefs). Backup compatibility: export the `default` file as `{key: {type, value}}` per `area:22-backup-restore`. Per-source settings for Rust-native sources are exposed through the same API. Passwords in tracker/source settings: store in the Secret Service via the `oo7` crate when available, SQLite otherwise (P2).
* Preserve: default-on-missing; delete-on-type-mismatch; `changes()` emitting the current value immediately; `getAll()` listing for backup; the `Object` default on deserialiser failure; download queue ordering by counter; the queue being cleared after restore.
* Difficulty moderate. Risk: Android stores `Set<String>` order-less and `Float` as 32-bit: keep `f32` for values that round-trip through backups.

### 11.2 SQLiteException
* Aniyomi: `catch (ex: SQLiteException) { throw SaveExtensionRepoException(ex) }` around `insert` and `upsert` of extension repos (`data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt:45,59`, anime twin) and `SaveCustomButtonException(ex)` around the custom-button insert (`data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt:32`). Exception messages: `"Error Saving Repository to Database"` (`domain/src/main/java/mihon/domain/extensionrepo/exception/SaveExtensionRepoException.kt:10`) and `"Error Saving Custom Button to Database"` (`domain/src/main/java/tachiyomi/domain/custombuttons/exception/SaveCustomButtonException.kt:10`); both extend `IOException`. Consumer: `CreateMangaExtensionRepo` catches `SaveExtensionRepoException`, logs `WARN` "SQL Conflict attempting to add new manga repository ${repo.baseUrl}" and calls `handleInsertionError(repo)` (conflict resolution by base URL / fingerprint), `domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt:35-40`; the migration `TrustExtensionRepositoryMigration.kt:43` also catches it. 2 files / 5 occ.
* Replacement: `rusqlite::Error::SqliteFailure(e, _)`; map `e.code == ErrorCode::ConstraintViolation` (extended codes `SQLITE_CONSTRAINT_UNIQUE`, `SQLITE_CONSTRAINT_PRIMARYKEY`) to `SaveExtensionRepoError` / `SaveCustomButtonError` (`thiserror`, same messages), other SQLite errors propagate as `DbError`. Keep the interactor's conflict handling.
* Preserve: only SQLite failures are wrapped; other exceptions pass through.
* Difficulty trivial.

### 11.3 SqlDriver
* Aniyomi: the SQLDelight driver held by the database handlers (`data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt:19`, anime twin). Its only use in the handler is `driver.currentTransaction() != null` inside `dispatch` (line 97): if the caller is already inside a transaction it runs the block directly (same thread), otherwise `withContext(getCurrentMangaDatabaseContext())`; when `inTransaction == true` it uses `withMangaTransaction { block(db) }`. Dispatchers: `queryDispatcher = Dispatchers.IO`, `transactionDispatcher = queryDispatcher`. Two databases exist: manga `tachiyomi.db` (schema `Database.Schema`) and anime `tachiyomi.animedb` (`AnimeDatabase.Schema`), each with its own adapters (`DateColumnAdapter`, `StringListColumnAdapter`). 2 files / 4 occ.
* Replacement: `struct Db { conn: Arc<Mutex<rusqlite::Connection>> }` (one writer connection) plus `r2d2`-free read connections opened with `SQLITE_OPEN_READ_ONLY` under WAL for concurrent reads. All access via `tokio::task::spawn_blocking`. Nested transactions: a task-local depth counter; `BEGIN IMMEDIATE` at depth 0, `SAVEPOINT sp_N` at deeper levels, so `await(inTransaction = true)` composed inside another transaction does not deadlock. Reactive queries (the `subscribeTo*` family of the handler) use `rusqlite::Connection::update_hook` to fire a `tokio::sync::watch` per table; consumers re-run their query (matches SQLDelight's `Query.addListener`). Schema ownership and the column adapters are specified in the catalogs `plans/catalogs/sql-*.md`; the decision whether to keep two database files is: keep both file names (`tachiyomi.db`, `tachiyomi.animedb`) only if backup/restore wants raw DB import; otherwise one file with prefixed tables (see `area:22-backup-restore`).
* Difficulty moderate. Risk: forgetting `PRAGMA foreign_keys = ON` per connection.

### 11.4 SupportSQLiteDatabase
* Aniyomi: `AppModule.kt:89-100,114-125` `onOpen(db: SupportSQLiteDatabase)` for both databases: `setPragma(db, "foreign_keys = ON")`, `setPragma(db, "journal_mode = WAL")`, `setPragma(db, "synchronous = NORMAL")`, where `setPragma` runs `db.query("PRAGMA $pragma")`, `moveToFirst()`, `close()` (because `journal_mode` returns a row). Factory: `RequerySQLiteOpenHelperFactory()` (bundled modern SQLite) except `FrameworkSQLiteOpenHelperFactory()` when `BuildConfig.DEBUG && SDK_INT >= R` (database inspector). 1 file / 5 occ.
* Replacement: after `Connection::open`, run `conn.pragma_update(None, "foreign_keys", "ON")`, `pragma_update(None, "journal_mode", "WAL")` (use `pragma_update_and_check` or `query_row` because it returns a row), `pragma_update(None, "synchronous", "NORMAL")`. `rusqlite` feature `bundled` replaces Requery. Applies to every connection, including read-only ones (`foreign_keys` is per connection).
* Preserve: the three PRAGMAs exactly; `user_version` migrations (see `plans/catalogs/sql-*.md`).
* Difficulty trivial.

---

## 12. Player controls, downloads (ffmpeg)

### 12.1 SeekBar
* Aniyomi: `open class SimpleSeekBarListener : SeekBar.OnSeekBarChangeListener` (`app/src/main/java/eu/kanade/tachiyomi/widget/listener/SimpleSeekBarListener.kt:5-12`) with empty overrides for `onProgressChanged`, `onStartTrackingTouch`, `onStopTrackingTouch`. `rg SimpleSeekBarListener` finds only its definition: dead XML-view-era code. 1 file / 5 occ. (A different Compose file `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt` is the live player seek bar, covered under Seeker.)
* Replacement: not ported. `gtk::Scale` with `connect_change_value` covers the three callbacks if ever needed.
* Difficulty trivial.

### 12.2 Seeker
* Aniyomi: `Seeker(value, range, onValueChange, onValueChangeFinished, readAheadValue, segments, modifier, colors)` in `SeekbarWithTimers` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt:108-136`). Layout: a 48 dp high row `[VideoTimer(position) 92 dp] [Seeker weight 1] [VideoTimer(duration or remaining) 92 dp]`, spacing `MaterialTheme.padding.extraSmall`. Value shown: `position = seekPosition` if gesture seeking, `internalSeekPosition` while dragging, else `playerPosition`; value coerced to `0f..duration`. `onValueChange` stores `internalSeekPosition` and forwards; `onValueChangeFinished` forwards `internalSeekPosition`. `readAheadValue` is mpv `demuxer-cache-time` (observed as `MPV_FORMAT_INT64`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:216`, forwarded by `PlayerActivity.kt:672` to `viewModel.updateReadAhead`, `PlayerViewModel.kt:622`). Segments: `chapters.filter { it.start in 0f..duration }`, with a leading `Segment("", 0f)` inserted when the first start is not 0 (the source has a trailing `+ it` expression after the if/else that may duplicate the list; do not replicate, deduplicate by `start`). Timers: `VideoTimer` text `Utils.prettyTime(value.toInt(), isInverted)`; tapping a timer toggles `timersInverted` (position as remaining time `position - duration`, second timer shows remaining when inverted). 1 file / 2 occ.
* Replacement: `gtk::Scale` (horizontal, `draw_value(false)`, range `0..duration` in seconds, step 1) in a `gtk::Box` with two `gtk::Button`-styled flat `gtk::Label` timers (`min-width` 92 px, click via `gtk::GestureClick`). Chapter ticks: `Scale::add_mark(start, gtk::PositionType::Bottom, None)` per segment (marks drawn by the theme). Read-ahead: `gtk::Range::set_show_fill_level(true)` + `set_fill_level(readahead_seconds)` + `set_restrict_to_fill_level(false)`. Seeking contract: while dragging only update the displayed position (`internalSeekPosition`); send one mpv `seek <t> absolute` at release (`gtk::GestureClick` released on the scale), exactly like `onValueChangeFinished(internalSeekPosition)`. Chapters come from mpv `chapter-list/count`, `chapter-list/N/title`, `chapter-list/N/time` (`PlayerViewModel.kt:479-490`). Timer text comes from `Utils.prettyTime(value.toInt(), isInverted)` (`import is.xyz.mpv.Utils`, `SeekBar.kt:48`, from the mpv-android library, not this repository); Proposal: `h:mm:ss` when >= 3600 s else `mm:ss`, with a leading `-` when inverted (also used for chapter start times in `ChaptersSheet.kt:94`).
* Preserve: chapter filter and synthetic zero segment, timer inversion toggle, `0..duration` clamp, read-ahead as a second fill.
* Difficulty moderate.

### 12.3 SeekerDefaults
* Aniyomi: `SeekerDefaults.seekerColors(progressColor = colorScheme.primary, thumbColor = colorScheme.primary, trackColor = colorScheme.background, readAheadColor = colorScheme.inversePrimary)` (`SeekBar.kt:131`). 1 file / 2 occ.
* Replacement: CSS on the player scale: `scale trough { background: @theme_bg_color; } scale highlight { background: @accent_color; } scale slider { background: @accent_color; } scale fill { background: alpha(@accent_color, 0.5); }` (node names of GTK 4's `scale`: `trough`, `highlight`, `fill`, `slider`).
* Difficulty trivial.

### 12.4 Segment
* Aniyomi: `dev.vivvvek.seeker.Segment(name: String, start: Float, color: Color = Unspecified)`. App wrapper `IndexedSegment(name, start, color, index, chapterType = ChapterType.Other)` with `toSegment()` (`SeekBar.kt:55-65`, `Unspecified = IndexedSegment("", 0f)`). Other users (6 files / 18 occ.): `BottomLeftPlayerControls.kt:45` (`currentChapter: Segment?` shown as the current-chapter chip), `PlayerSheets.kt:69-70` (`chapter`, `chapters: ImmutableList<Segment>` for the chapters sheet), `components/CurrentChapter.kt:54`. `ChapterType` (`animesource/model/ChapterType`) marks skip-intro style chapters (`PlayerViewModel.kt:2073,2104`; `PlayerActivity.kt:1366` names a timestamp when `name.isEmpty() && type != ChapterType.Other`).
* Replacement: `#[derive(Clone)] struct Chapter { name: String, start: f64, color: Option<gdk::RGBA>, index: usize, kind: ChapterType }`, `enum ChapterType { Opening, Ending, Recap, MixedOp, Other }` (declaration order of `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:14-20`; `TimeStamp(start: Double, ..., type: ChapterType = ChapterType.Other)` at line 27). The skip-button logic is specified in `area:45-player-core`.
* Difficulty trivial.

### 12.5 StatisticsCallback
* Aniyomi: `AnimeDownloader.kt:602-607` (inside `ffmpegDownload`, lines 568-640): `val statCallback = StatisticsCallback { s -> val outTime = (s.time / 1000.0).toLong(); if (duration != 0L && outTime > 0) { download.progress = (100 * outTime / duration).toInt() } }`. `duration` comes from ffprobe: `-v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "<file>"` (prefixed by header options for HTTP inputs), parsed with `toFloatOrNull()` then `.toLong()`; 0 when unknown (progress then stays unchanged). Log callback: messages with `log.level <= Level.AV_LOG_WARNING` are logged at ERROR. The command (built in `getFFmpegOptions`, lines ~620-700): inputs (`-headers '<K: V\r\n...>'` before each `http` input, `-i "<url>"`), maps `-map 0:v`, audio maps `-map N:a`, `-map 0:a?`, subtitle maps `-map N:s`, `-map 0:s? -map 0:t?`, `-f matroska -c:a copy -c:v copy -c:s copy`, per-track `-metadata:s:<type>:<i> "title=<lang>"`, source-provided `ffmpegStreamArgs` and `ffmpegVideoArgs` (`-key "value"`), output `"<tmp>.tmp" -y`. On success the temp file `<filename>.tmp` is renamed to `<filename>.mkv`; failure: `Exception("Error in ffmpeg!")`; cancelling the coroutine cancels the ffmpeg session. 1 file / 2 occ.
* Replacement: spawn the system `ffmpeg`/`ffprobe` (Arch package `ffmpeg`) with `tokio::process::Command` and an argument vector (no shell quoting; the Kotlin code concatenates a string and re-splits it with `FFmpegKitConfig.parseArguments`). Add `-progress pipe:1 -nostats -loglevel warning`; read stdout lines `key=value`; on `out_time_us=<n>` (microseconds) compute `out_time_s = n / 1_000_000`, then `progress = 100 * out_time_s / duration_s` when `duration_s > 0 && out_time_s > 0`; `progress=end` marks completion. stderr lines at warning or worse are logged with `tracing::error!`. Cancel: `child.kill().await` on `CancellationToken`. Exit status 0 then `rename(tmp, "<name>.mkv")`. If `ffmpeg` is missing: error `ffmpeg not found` surfaced as a download error, and a startup health check warns once (P1 for anime downloads, see `area:45-player-core` and `area:23-downloads-manga`).
* Preserve: copy codecs (no re-encode), matroska container, output extension `.mkv`, per-track language metadata, `.tmp` then atomic rename, progress formula, `-y` overwrite.
* Difficulty moderate. Risk: `out_time_ms` in ffmpeg's `-progress` output is actually microseconds; use `out_time_us`.

### 12.6 SwipeableActionsBox
* Aniyomi: `me.saket.swipe.SwipeableActionsBox(modifier = clipToBounds(), startActions, endActions, swipeThreshold, backgroundUntilSwipeThreshold = surfaceContainerLowest)` wrapping each chapter row (`app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaChapterListItem.kt:86-92`) and episode row (`.../anime/components/AnimeEpisodeListItem.kt:108-112`). `swipeActionThreshold = 56.dp` (`MangaChapterListItem.kt:242`). Action background while armed: `primaryContainer`. Icons (`getSwipeAction`, lines 187-225): ToggleRead: `Done` / `RemoveDone` (undo when already read); ToggleBookmark: `BookmarkAdd` / `BookmarkRemove`; Download: `Download` (NOT_DOWNLOADED, ERROR), `FileDownloadOff` (QUEUE, DOWNLOADING), `Delete` (DOWNLOADED); Disabled: no action (null). `onSwipe` is `onChapterSwipe(action)`, executed in `MangaScreenModel.executeChapterSwipeAction` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:605-630`): ToggleRead `markChaptersRead(listOf(chapter), !chapter.read)`; ToggleBookmark `bookmarkChapters(listOf(chapter), !chapter.bookmark)`; Download maps ERROR/NOT_DOWNLOADED to `START_NOW`, QUEUE/DOWNLOADING to `CANCEL`, DOWNLOADED to `DELETE`; Disabled throws `IllegalStateException`. 2 files / 4 occ.
* Preferences (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:361-386`): the accessor names are crossed with the key names, and the screen models cross them again (`MangaScreenModel.kt:144-145`: `chapterSwipeStartAction = swipeChapterEndAction().get()`), so the net effect is: swipe-start (left-to-right) action uses key `"pref_chapter_swipe_start_action"` default `ToggleBookmark`; swipe-end (right-to-left) action uses key `"pref_chapter_swipe_end_action"` default `ToggleRead`. Episodes likewise: start uses `"pref_episode_swipe_start_action"` default `ToggleBookmark`, end uses `"pref_episode_swipe_end_action"` default `ToggleSeen`. Enums: `ChapterSwipeAction { ToggleRead, ToggleBookmark, Download, Disabled }`; `EpisodeSwipeAction { ToggleSeen, ToggleBookmark, ToggleFillermark, Download, Disabled }` (stored by name).
* Replacement: P2. Use `gtk::GestureDrag` on the row (horizontal, 56 px threshold, row translated via `gtk::Fixed`/margin with a coloured action box revealed behind) for touchpad/touch users; the desktop-first replacement (P1) is the row context menu and hover buttons (`gtk::PopoverMenu`: Mark read/unread, Bookmark, Download/Delete) plus the multi-select toolbar. Keep the two prefs and both enums so settings and backups round-trip, and apply the same actions.
* Difficulty moderate.

### 12.7 SimpleGridCells
* Aniyomi: `io.woong.compose.grid.SimpleGridCells.Fixed(DAYS_OF_WEEK)` for the "upcoming" calendar grid (`app/src/main/java/mihon/feature/upcoming/components/calendar/Calendar.kt:84`, `DAYS_OF_WEEK = 7`). Behaviour (lines 66-130): weekday headers are `DayOfWeek` starting at `WeekFields.of(Locale.getDefault()).firstDayOfWeek`, rendered `TextStyle.NARROW` bold (`SemiBold`); `emptyFieldCount = weekDays.indexOf(firstOfMonth.dayOfWeek)` blank cells, then one `CalendarDay` per day. `CalendarDay` (`CalendarDay.kt:28-90`): day number 16 sp SemiBold, today circled with a 1 dp `onBackground` border, days before today at `DISABLED_ALPHA = .38f` (`presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:9`), up to `MAX_EVENTS = 3` indicator dots (`events.coerceAtMost(3)`) in `primary`, clickable. On medium-width windows (not expanded) the grid is limited to 360 dp. 1 file / 2 occ.
* Replacement: `gtk::Grid` with `set_column_homogeneous(true)`, 7 columns; first weekday from `glib::DateTime`/`libc::nl_langinfo(_NL_TIME_FIRST_WEEKDAY)`; narrow weekday names via `glib::DateTime::format("%a")` first character (or ICU-less table per locale); day cells are `gtk::Button` flat with a `gtk::DrawingArea` dot row. `gtk::Calendar` is not used because it cannot show per-day dot counts or disable styling of past days.
* Difficulty trivial.

---

## 13. Rate limiting

### 13.1 SystemClock
* Aniyomi: `SystemClock.elapsedRealtime()` (monotonic milliseconds since boot, includes deep sleep) at `RateLimitInterceptor.kt:90,110`, inside the interceptor that implements the extension-lib `rateLimit(permits, period)` API (`period` default `1.seconds`; e.g. `permits = 5, period = 1 second` is 5 requests per second; `permits = 10, period = 2 minutes`). Algorithm (OBSERVED): a `fairLock = Semaphore(1, true)` serialises waiters FIFO; `requestQueue: ArrayDeque<Long>` of request timestamps. Loop while `requestQueue.size >= permits`: `periodStart = now - rateLimitMillis`; remove from the head every timestamp `<= periodStart`; if a request was cancelled throw `IOException("Canceled")`; if some were removed break; else wait on the queue monitor for `requestQueue.first - periodStart` ms (re-loop on interrupt). Then `timestamp = now; requestQueue.addLast(timestamp)`. After `chain.proceed`, if `response.networkResponse == null` (served from the OkHttp cache) the slot is refunded: remove the first occurrence of `timestamp` (unless the queue is empty or the timestamp is older than the head) and `notifyAll()`. The `host` field limits the rule to one host (`null` matches every host; the public extension API passes `null`, i.e. applies to all requests of that client). A cancelled call before start throws `IOException("Canceled")`. 1 file / 3 occ.
* Replacement: `struct RateLimiter { permits: usize, period: Duration, queue: tokio::sync::Mutex<VecDeque<Instant>> }`. `async fn acquire(&self, cancel: &CancellationToken) -> Result<Slot, Canceled>`: fairness through `tokio::sync::Mutex` (FIFO wake order); in the loop compute `period_start = now - period`, pop expired heads, and `tokio::select! { _ = sleep(first + period - now) => {}, _ = cancel.cancelled() => return Err(Canceled), _ = notify.notified() => {} }`; push `Instant::now()`. `Slot::refund()` removes the timestamp and `notify_waiters()` when the response was served from the local cache (our cover/page cache; reqwest has no HTTP cache so refunds only apply to our own cache hits). Time source: `std::time::Instant` (CLOCK_MONOTONIC) is acceptable; Linux `CLOCK_BOOTTIME` (via `rustix::time::clock_gettime(ClockId::Boottime)`) is closer to `elapsedRealtime` if the machine suspends mid-window, but after resume extra waiting is harmless, so use `Instant`.
* Preserve: sliding-window semantics (an entry at exactly `period_start` has expired, `<=`), FIFO fairness, cancellation message `Canceled`, cache-hit refund.
* Difficulty trivial. Risk: do not `sleep` while holding the queue mutex; compute the wait, drop the guard, sleep, retake.

