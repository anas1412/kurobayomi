# Android/Java/third-party API mapping, chunk 2 of 8

This file catalogs the 54 items returned by `python3 plans/_tools/inventory.py items imports 2 8`. The first item is `BroadcastReceiver` (`android.content.BroadcastReceiver`) and the last is `DecodeResult` (`coil3.decode.DecodeResult`); the list is alphabetical by simple class name. Every item was checked against the Kotlin in `/home/blackbox/aniyomi-src` (paths are repo-relative); two third-party behaviours that are not in this repository were read from upstream sources and are labelled as such: the natural-order comparator (`gpanther/java-nat-sort`, exact algorithm quoted) and Coil/OkHttp/WorkManager library semantics (named "Library semantics", from the documented behaviour of the pinned versions: Coil 3.1.0, OkHttp 5.4.0, okio 3.10.2, SQLDelight 2.0.2, Glance 1.1.1, ConstraintLayout 2.2.1; versions from `gradle/libs.versions.toml`, `gradle/compose.versions.toml`, `gradle/androidx.versions.toml`).

Related areas: area:12-network-stack, area:34-images-covers-cache, area:43-reader-core, area:44-reader-viewers, area:45-player-core, area:46-player-controls-ui, area:26-library-update-jobs, area:53-platform-manifest-intents-workers, area:50-android-widgets, area:54-build-release-distribution-docs, area:02-preferences-system, area:20-extension-strategy-redesign.

## 0. How to read this catalog

- OBSERVED text states what the Kotlin does. Text under "Replacement", "Preserve" or marked "Proposal:" is the Rust/GTK plan.
- "Usage" counts: number of Kotlin files that import the class (`grep -rlE '^import <fqcn>'` over `*.kt`, `*.kts`, `*.java`) and the number of lines mentioning the simple name outside import lines (`\bName\b`); the second number is approximate because comments and same-named identifiers are included.
- Difficulty: trivial (under half a day, no design choice), moderate (needs design or several moving parts), hard (needs a custom subsystem or unavoidable platform gaps).
- Target stack: stable Rust, `gtk4-rs` 0.11 (GTK 4.16+, no libadwaita), `tokio`, `reqwest`, `rusqlite`, `serde`, libmpv.
- Because libadwaita is not used, anything Aniyomi gets from Material or AppCompat (toasts, bottom sheets, animation helpers, theming) is built from plain GTK widgets, CSS and `gtk::Snapshot`.

## 0.1 Index (all 54 items)

| Item | Section | Files | Uses | Linux/Rust replacement | Difficulty |
|---|---|---|---|---|---|
| `BroadcastReceiver` | 3 Framework | 11 | 14 | gio actions on `gtk::Application` (notifications), `gio::FileMonitor` (extensions), drop PiP/APK receivers | moderate |
| `Buffer` | 1 Network | 4 | 7 | `bytes::Bytes` / `Vec<u8>` | trivial |
| `BufferedSource` | 1 Network | 9 | 25 | `bytes::Bytes` + `std::io::Read`, atomic `save_to` | trivial |
| `Build` | 3 Framework | 47 | 95 | delete API gates; `std::env::consts`, `/etc/os-release` | trivial |
| `Bundle` | 3 Framework | 16 | 19 | drop (state restore); argv lists for external tools | trivial |
| `By` | 3 Framework | 1 | 7 | drop (UI Automator benchmark script) | trivial |
| `Cache` | 1 Network | 1 | 1 | `http-cache-reqwest` + 5 MiB sweeper | moderate |
| `CacheControl` | 1 Network | 4 | 18 | `CacheMode` enum producing header strings | trivial |
| `CachePolicy` | 2 Image/graphics | 6 | 9 | Rust enum `CachePolicy` | trivial |
| `Call` | 1 Network | 3 | 15 | `reqwest::Client` futures + `CancellationToken` | moderate |
| `Callback` | 1 Network | 1 | 1 | async/await (no callback) | trivial |
| `Canvas` | 2 Image/graphics | 1 | 1 | `gtk::DrawingArea` + `cairo::Context` | trivial |
| `CaseInsensitiveSimpleNaturalComparator` | 5 Helpers/DB | 1 | 1 | hand-written `natural_cmp` (exact algorithm) | trivial |
| `Charset` | 5 Helpers/DB | 1 | 1 | `quick-xml` serde + manual declaration | trivial |
| `CircleCropTransformation` | 2 Image/graphics | 2 | 2 | `image` crate circle mask, 192 px | trivial |
| `CircularProgressIndicator` | 2 Image/graphics | 5 | 5 | `gtk::Spinner` + custom cairo arc | moderate |
| `ClipData` | 3 Framework | 3 | 3 | `gdk::Clipboard` / `ContentProvider` | trivial |
| `ClipboardManager` | 3 Framework | 2 | 2 | `gdk::Display::clipboard()` | trivial |
| `Color` | 2 Image/graphics | 18 | 29 | `u32` ARGB + `gdk::RGBA`, own mpv colour parser | trivial |
| `ColorDrawable` | 2 Image/graphics | 1 | 7 | `PageBackground` enum + `Snapshot` colour/gradient | trivial |
| `ColorMatrix` | 2 Image/graphics | 1 | 2 | `graphene::Matrix` + `Vec4` offset | moderate |
| `ColorMatrixColorFilter` | 2 Image/graphics | 1 | 1 | `Snapshot::push_color_matrix` in a wrapper widget | moderate |
| `ColorProvider` | 2 Image/graphics | 10 | 10 | drop with Glance widgets | trivial |
| `ColorUtils` | 2 Image/graphics | 3 | 3 | `palette` crate / 25 lines HSL | trivial |
| `Column` | 2 Image/graphics | 2 | 2 | drop with Glance widgets (`gtk::Box` if strip is built) | trivial |
| `ColumnAdapter` | 5 Helpers/DB | 1 | 6 | `rusqlite` `FromSql`/`ToSql` newtypes | trivial |
| `CommonExtension` | 6 Build tooling | 1 | 2 | Cargo workspace settings | trivial |
| `CompilationMode` | 6 Build tooling | 1 | 6 | drop (ART benchmarking) | trivial |
| `ComponentActivity` | 3 Framework | 2 | 2 | `gtk::ApplicationWindow` | trivial |
| `ComponentName` | 3 Framework | 2 | 11 | argv-based external players/downloaders, `gio::AppInfo` discovery | moderate |
| `ComposeCompilerGradlePluginExtension` | 6 Build tooling | 1 | 1 | drop | trivial |
| `ComposeFeatureFlag` | 6 Build tooling | 1 | 1 | drop | trivial |
| `Configuration` | 3 Framework | 13 | 23 | window size classes, XDG colour-scheme portal | trivial |
| `ConnectivityManager` | 1 Network | 1 | 1 | `gio::NetworkMonitor` (+ NetworkManager D-Bus for Wi-Fi) | moderate |
| `Conscrypt` | 1 Network | 1 | 1 | drop; rustls with native roots | trivial |
| `ConstraintLayout` | 5 Helpers/DB | 5 | 5 | `gtk::Overlay` + boxes + small custom layout manager | moderate |
| `Constraints` | 4 Background work | 6 | 6 | in-process scheduler + UPower + `gio::NetworkMonitor` | moderate |
| `Consumer` | 3 Framework | 1 | 1 | `gtk::Application` open/command-line + channel | trivial |
| `ContentScale` | 2 Image/graphics | 2 | 4 | `gtk::ContentFit::Cover` | trivial |
| `ContentUris` | 3 Framework | 1 | 1 | drop MediaStore; plain file save in Pictures | trivial |
| `Context` | 3 Framework | 184 | 389 | `AppEnv` + XDG dirs + gio/GTK equivalents per category | moderate |
| `ContextCompat` | 3 Framework | 13 | 18 | `glib::MainContext` invoke, `gdk::Texture::from_resource` | trivial |
| `ContextThemeWrapper` | 3 Framework | 3 | 3 | CSS classes on reader window, explicit colours | moderate |
| `Cookie` | 1 Network | 2 | 5 | `cookie` crate | moderate |
| `CookieJar` | 1 Network | 1 | 1 | `reqwest::cookie::CookieStore` over `reqwest_cookie_store` | hard |
| `CookieManager` | 1 Network | 2 | 3 | `webkit6` `CookieManager` synced with the jar | hard |
| `CoroutineWorker` | 4 Background work | 12 | 12 | `Job` trait on tokio scheduler | moderate |
| `CreationExtras` | 3 Framework | 1 | 1 | drop (plain constructors) | trivial |
| `Credentials` | 1 Network | 1 | 1 | `base64` over Latin-1 bytes | trivial |
| `DataSource` | 2 Image/graphics | 3 | 11 | `ImageOrigin` enum | trivial |
| `DateUtils` | 3 Framework | 2 | 2 | `chrono`/`glib::DateTime` + own plurals | moderate |
| `DebugLogger` | 2 Image/graphics | 1 | 1 | `tracing` + reload handle | trivial |
| `DecelerateInterpolator` | 2 Image/graphics | 1 | 2 | tick-callback easing `1-(1-t)^2` | trivial |
| `DecodeResult` | 2 Image/graphics | 1 | 2 | `Decoded` struct; `image`/`libheif-rs`/`jxl-oxide` | moderate |

## 0.2 Decisions that cut across items

| Decision | Items | What the user loses |
|---|---|---|
| Android IPC (Intents, broadcasts, PendingIntents, ComponentName, Bundle extras) replaced by `gio::Application` actions, D-Bus activation and spawned processes | `BroadcastReceiver`, `Bundle`, `ComponentName`, `Consumer`, `Context`, `ContextCompat` | nothing functional; notification buttons need the installed `.desktop` file |
| APK installation machinery dropped (extension delivery is decided in area:20-extension-strategy-redesign) | `BroadcastReceiver` (installer receivers), `Context` (`DownloadManager`) | installing Android extension APKs |
| Android home-screen widgets dropped | `ColorProvider`, `Column`, `ContentScale` (Glance) | the cover grid on the launcher; the same data is shown in the Updates tab |
| Picture-in-Picture and audio-noisy receivers | `BroadcastReceiver` | PiP window; auto-pause on headphone unplug unless P2 PulseAudio/PipeWire watcher is built |
| Per-activity state restore dropped | `Bundle`, `CreationExtras` | nothing (desktop process is not killed in the background) |
| Android-only tuning and device quirks dropped | `Build`, `Conscrypt`, `By`, `CompilationMode`, `CommonExtension`, `ComposeCompilerGradlePluginExtension`, `ComposeFeatureFlag` | nothing |
| Cookie store shared between HTTP client and WebView is rebuilt as an explicit sync | `Cookie`, `CookieJar`, `CookieManager` | needs WebKitGTK (`webkitgtk-6.0`) installed for Cloudflare solving and the in-app browser |
| Foreground services dropped; jobs run in-process and keep the app alive with `Application::hold()` | `CoroutineWorker`, `Constraints` | periodic jobs do not run while Kurobayomi is closed (P2: systemd user timer) |
| Per-window dark/light theme switching is impossible in GTK 4 | `ContextThemeWrapper`, `Configuration` | reader chrome uses explicit colours instead of following the GTK theme |


## 1. Network, HTTP and byte-stream classes (okhttp3, okio, android.webkit, Conscrypt, ConnectivityManager)

Related areas: area:12-network-stack, area:34-images-covers-cache, area:20-extension-strategy-redesign, area:36-ui-shell-navigation-security-webview.

Proposal (applies to the whole section): one `reqwest::Client` (features `rustls-tls-native-roots`, `gzip`, `brotli`, `zstd`, `deflate`, `cookies` off because the app owns its own jar, `stream`) built once in a `NetworkHelper` struct with the exact timeouts below, shared by every source and tracker. Request helpers `get/post/put/delete` mirror `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt`.

### Buffer

- Usage: 4 files, 7 occurrences outside import lines. Representative: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt:151` (`streamFn().use { process(item, Buffer().readFrom(it)) }`), `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt:191`, `core/common/src/main/java/eu/kanade/tachiyomi/network/ProgressResponseBody.kt:38` (`read(sink: Buffer, byteCount: Long)`), `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:144` (`val output = Buffer()` then `half.compress(Bitmap.CompressFormat.JPEG, 100, output.outputStream())`) and the same pattern in `rotateImage` (`ImageUtil.kt:150-158`); `splitInHalf` is `ImageUtil.kt:129`.
- What it does: an in-memory growable byte queue. The reader copies a whole page `InputStream` into one `Buffer`, then repeatedly `peek()`s it (type sniff, animated check, background colour) before handing it to the image view.
- Replacement: `bytes::Bytes` (cheap clone, slicing = peek) for finished data and `Vec<u8>` while filling; `std::io::Read::read_to_end` replaces `readFrom`. JPEG re-encode at quality 100 becomes `image::codecs::jpeg::JpegEncoder::new_with_quality(&mut vec, 100)`.
- Preserve: a page is fully buffered before decoding; split-in-half and rotate re-encode to JPEG quality 100 (lossy re-encode is the observed behaviour, keep it, Proposal: keep PNG source as PNG when the input is PNG to avoid loss; user-visible difference is none).
- Difficulty: trivial. Risk: a 100-megapixel webtoon strip decoded fully is large; keep only the encoded `Bytes` in the page cache and decode on demand.

### BufferedSource

- Usage: 9 files, 25 occurrences outside import lines. Representative: `app/src/main/java/eu/kanade/tachiyomi/data/coil/BufferedSourceFetcher.kt:13`, `app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt:72` (`isApplicable(source: BufferedSource)` peeks the first bytes), `app/src/main/java/eu/kanade/tachiyomi/util/storage/OkioExtensions.kt:14,32` (`saveTo(file)`, `saveTo(stream)`), `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt` (`findImageType(source.peek().inputStream())`, `isAnimatedAndSupported(source)`), `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt`.
- What it does: a readable stream with `peek()` (look ahead without consuming). `BufferedSourceFetcher` returns `SourceFetchResult(... mimeType = null, dataSource = DataSource.MEMORY)` so Coil can decode in-memory pages. `saveTo(file)` = `file.parentFile?.mkdirs()`, copy, and on any exception `close()` + `file.delete()` + rethrow. `saveTo(stream)` copies with `writeAll`, `flush()`, closes both ends.
- Replacement: `bytes::Bytes` + `std::io::Cursor<&[u8]>`; peek = `&bytes[..n]`. `fn save_to(path: &Path, data: &Bytes) -> io::Result<()>`: `create_dir_all(parent)`, write to `<name>.part`, `fsync`, `rename` to final name; on error remove the `.part`. Proposal: this is strictly safer than Aniyomi, which leaves a deleted-on-error final file but can expose a half-written file to a concurrent reader.
- Preserve: parent directories are created; a failed copy never leaves a file behind.
- Difficulty: trivial. Risk: none beyond using the atomic rename on the same filesystem (keep the temp file next to the target).

### Cache

- Usage: 1 file, 1 occurrence outside import lines. `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:27`: `Cache(directory = File(context.cacheDir, "network_cache"), maxSize = 5L * 1024 * 1024)`.
- What it does: OkHttp RFC 7234 disk cache shared by every request made through `NetworkHelper.client` (sources, trackers, updater). Image requests opt out with `no-store` (see CacheControl).
- Replacement: `http-cache-reqwest` (crate) with `CACacheManager` rooted at `glib::user_cache_dir()/kurobayomi/network_cache` (that is `$XDG_CACHE_HOME/kurobayomi/network_cache`, fallback `~/.cache`).
- Preserve: 5 MiB cap. `cacache` has no size limit, so Proposal: a sweeper run at start-up and after every 50 writes that deletes oldest-by-mtime entries until the directory is <= 5 242 880 bytes. Default request freshness is 10 minutes (see CacheControl) so repeated browse/detail fetches inside 10 minutes hit the cache.
- Difficulty: moderate. Risk: `http-cache` follows the `Cache-Control` of responses; many extension sites send `no-store`/`private`, which OkHttp also honours, so behaviour is equivalent. If the crate proves awkward the fallback is dropping the HTTP cache entirely (loss: repeated identical GETs inside 10 minutes hit the network again).

### CacheControl

- Usage: 4 files, 18 occurrences outside import lines. `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:15`: `private val DEFAULT_CACHE_CONTROL = CacheControl.Builder().maxAge(10, MINUTES).build()`; every `GET/POST/PUT/DELETE` (and the suspend `OkHttpClient.get/post/put/delete`) defaults to it. `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt:345` `CACHE_CONTROL_NO_STORE = CacheControl.Builder().noStore().build()`, `:346` `CACHE_CONTROL_NO_NETWORK_NO_CACHE = CacheControl.Builder().noCache().onlyIfCached().build()` (identical constants at `AnimeImageFetcher.kt:362-363`). `app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt:191,217`: `GET(url, cache = CacheControl.FORCE_NETWORK)`.
- Wire values (OkHttp serialises the builder into the request `Cache-Control` header): `maxAge(10, MINUTES)` -> `max-age=600`; `noStore()` -> `no-store`; `noCache().onlyIfCached()` -> `no-cache, only-if-cached`; `FORCE_NETWORK` -> `no-cache`.
- Proposal: `enum CacheMode { MaxAge(Duration), NoStore, NoCacheOnlyIfCached, ForceNetwork }` with `fn header(&self) -> &'static str/String` giving the four strings above; default `MaxAge(600 s)`.
- Preserve (cover fetcher rule, `MangaCoverFetcher.kt:180-203` (`newRequest()`)): if the image request's network policy allows reading (`options.networkCachePolicy.readEnabled`) send `no-store` (do not fill the 5 MiB HTTP cache with covers, the image disk cache owns them); otherwise send `no-cache, only-if-cached`, which in OkHttp yields a synthetic `504 Unsatisfiable Request (only-if-cached)`. Rust: when the policy forbids network, return `Err(ImageError::NetworkDisabled)` without sending a request.
- Difficulty: trivial. Risk: none.

### Call

- Usage: 3 files, 15 occurrences outside import lines. Representative: `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt` (`Call.await()`, `Call.awaitSuccess()`, `OkHttpClient.newCachelessCallWithProgress`), `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt:56` and `AnimeImageFetcher.kt:57,300,338` (`Lazy<Call.Factory>`).
- What it does: a prepared request that is executed once. Behaviour to keep:
  - `await()` runs the call on OkHttp's dispatcher; cancellation of the coroutine calls `Call.cancel()` (errors swallowed); a failure is wrapped as `IOException(e.message, e)` with the caller's stack trace attached; a response that arrives after cancellation is closed.
  - `awaitSuccess()` additionally closes the response and throws `HttpException(code)` when `!response.isSuccessful` (status outside 200..299).
  - `newCachelessCallWithProgress(request, listener)`: client copy with `.cache(null)`, `.callTimeout(30, HOURS)` and a network interceptor that wraps the body in `ProgressResponseBody` (see Buffer); used for large downloads (app update APK, extension APK, `AppUpdateDownloadJob.kt`'s progress listener throttled to one notification per 200 ms and only when the integer percent increases).
  - Client timeouts (`NetworkHelper.kt:24-26`): `connectTimeout(30, SECONDS)`, `readTimeout(30, SECONDS)`, `callTimeout(2, MINUTES)`.
  - A deprecated Rx variant `Call.asObservableSuccess()` exists only for old extensions (`OkHttpExtensions.kt:62`).
- Replacement: `reqwest::Client` builder `.connect_timeout(30 s).read_timeout(30 s).timeout(120 s)`; an `async fn send(&self, req) -> Result<Response, NetworkError>`; cancellation = dropping the future or `tokio::select!` with a `tokio_util::sync::CancellationToken`. `awaitSuccess` = `if !status.is_success() { Err(HttpError { code }) }` where the `Display` text matches `HttpException` ("HTTP error {code}", see area:12-network-stack). The progress call: a second client built without the 120 s total timeout (`.timeout(Duration::from_secs(30*3600))`) and a `bytes_stream()` adapter that calls `listener(total_read, content_length, done)` per chunk.
- Dropped: the Rx `asObservableSuccess` (nothing in Rust loads old Rx extensions; area:20-extension-strategy-redesign decides the extension ABI).
- Difficulty: moderate. Risk: reqwest `.timeout` includes body streaming (same as OkHttp `callTimeout`), so long downloads need the dedicated client above or they fail at 120 s.

### Callback

- Usage: 1 file, 1 occurrence outside import lines. `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:85` inside `private suspend fun Call.await(callStack)`: `this.enqueue(object : Callback { onFailure(call, e) ... onResponse(call, response) ... })`.
- What it does: the async bridge between OkHttp and coroutines. `onFailure`: if the continuation is already cancelled return, else resume with `IOException(e.message, e)` carrying `callStack`. `onResponse`: resume with the response; if the resume is cancelled the response is closed (`resume(response) { _, value, _ -> value.close() }`).
- Replacement: none needed; `reqwest` is natively `async`. Dropping a `Response` closes the connection.
- Difficulty: trivial. Risk: none.

### Cookie

- Usage: 2 files, 5 occurrences outside import lines. `core/common/src/main/java/eu/kanade/tachiyomi/network/AndroidCookieJar.kt:12,18,22,26` (`Cookie.parse(url, it)` for each `;`-separated pair returned by the WebView store), `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt:48,66-72` (finds the `cf_clearance` cookie and compares with the previous one using `!=`, OkHttp structural equality).
- What it does: the data class for one cookie. Cookies read back from the WebView string `name=value; name2=value2` carry no attributes, so `Cookie.parse` fills defaults (host-only, path from the URL, no expiry).
- Replacement: crate `cookie` (`Cookie::parse`, `Cookie::name/value`) plus `cookie_store`/`reqwest_cookie_store` (`CookieStoreMutex`) as the jar.
- Preserve: Cloudflare success test is "a `cf_clearance` cookie exists for the request URL AND it differs from the cookie that existed before the challenge". Proposal: compare `(name, value)` only (expiry changes without a re-solve are not a success).
- Difficulty: moderate. Risk: attribute-less cookies read from WebKit must be rebuilt with domain/path from the request URL or `cookie_store` will not return them.

### CookieJar

- Usage: 1 file, 1 occurrence outside import lines. `core/common/src/main/java/eu/kanade/tachiyomi/network/AndroidCookieJar.kt:8` (`class AndroidCookieJar : CookieJar`), used by `NetworkHelper.kt:19,22` (`.cookieJar(cookieJar)`) and `CloudflareInterceptor`.
- What it does (full API):
  - `saveFromResponse(url, cookies)`: for each cookie `manager.setCookie(url.toString(), cookie.toString())` (writes into the WebView's global store).
  - `loadForRequest(url)` = `get(url)`.
  - `get(url)`: `manager.getCookie(url.toString())`; null/empty -> empty list; else split on `";"` and `Cookie.parse(url, piece)` ignoring parse failures.
  - `remove(url, cookieNames: List<String>? = null, maxAge: Int = -1): Int`: for every existing cookie name (filtered by `cookieNames` when non-null) writes `"$name=;Max-Age=$maxAge"`; returns how many were removed. Cloudflare calls it as `remove(request.url, listOf("cf_clearance"), 0)`.
  - `removeAll()`: `manager.removeAllCookies {}` (used by the "clear cookies" setting, area:47-settings-screens-general).
- Key design fact: the HTTP client and the WebView share one cookie store; that is how a Cloudflare challenge solved in a WebView lets the next OkHttp request through.
- Replacement: `struct CookieJar { store: Arc<reqwest_cookie_store::CookieStoreMutex> }` implementing `reqwest::cookie::CookieStore` (`set_cookies(&self, headers: &mut dyn Iterator<Item=&HeaderValue>, url: &Url)`, `cookies(&self, url) -> Option<HeaderValue>`), with inherent methods `get(url)`, `remove(url, names: Option<&[&str]>, max_age) -> usize`, `remove_all()`. Persist as JSON at `glib::user_data_dir()/kurobayomi/cookies.json` (`CookieStore::save_json`, atomic write) on every change after a 2 s debounce and on exit; load on start; session cookies are persisted too (Android's CookieManager persists them).
- WebKit bridge (see CookieManager below): before loading a page in the WebView, push the jar's cookies for that host into `webkit::CookieManager`; when the challenge completes, read the host's cookies from WebKit back into the jar.
- Difficulty: hard (two stores to keep consistent). Risk: reqwest's `CookieStore::cookies` is synchronous while WebKit's cookie API is async and main-thread only; hence the explicit sync-at-boundaries design instead of a live shared store.

### CookieManager

- Usage: 2 files, 3 occurrences outside import lines. `AndroidCookieJar.kt:10` (`CookieManager.getInstance()`), `core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt:49` (support probe: `CookieManager.getInstance()` inside try/catch, failure logged and `supportsWebView` returns false, then also requires `FEATURE_WEBVIEW`), `WebViewUtil.kt:92` (`setDefaultSettings`: `CookieManager.getInstance().acceptThirdPartyCookies(this)`).
- What it does: the global WebView cookie store.
- Replacement: `webkit6` crate (WebKitGTK 6.0 for GTK4, Arch package `webkitgtk-6.0`): `NetworkSession::cookie_manager()`, `set_persistent_storage(path, CookiePersistentStorage::Sqlite)`, `set_accept_policy(CookieAcceptPolicy::Always)` (equivalent of accepting third-party cookies), async `cookies(uri)`, `add_cookie`, `delete_cookie`, `delete_all_cookies`. `supportsWebView` becomes "the `webkit6` library loaded and a `WebView` could be created" (runtime check at first use; if missing show `information_webview_required`, see area:51-i18n).
- Difficulty: hard. Risk: WebKitGTK is a heavy optional dependency (about 150 MB installed); Proposal: gate Cloudflare solving and the in-app browser behind a cargo feature `webview` so a build without it still reads sources that do not need it.

### Credentials

- Usage: 1 file, 1 occurrence outside import lines. `app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt:38`: `val credentials = Credentials.basic(baseLogin, basePassword)` then `add("Authorization", credentials)`, only when both login and password are non-empty (`:37`). The same client is built with `.dns(Dns.SYSTEM)` (`:31`) so DNS-over-HTTPS never applies to the user's LAN server.
- What it does: builds `Basic base64(login ":" password)` where the text is encoded ISO-8859-1 (OkHttp default charset); characters above U+00FF become `?`.
- Replacement: `base64::engine::general_purpose::STANDARD` over the Latin-1 bytes (`s.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })`). Do not use `RequestBuilder::basic_auth` (it encodes UTF-8, a different header for non-ASCII passwords).
- Preserve: header sent only when both fields non-empty; bypass DoH for Suwayomi (build that client with the system resolver).
- Difficulty: trivial. Risk: none.

### Conscrypt

- Usage: 1 file, 1 occurrence outside import lines. `app/src/main/java/eu/kanade/tachiyomi/App.kt:90`: `Security.insertProviderAt(Conscrypt.newProvider(), 1)` only when `Build.VERSION.SDK_INT < Build.VERSION_CODES.Q` (Android 8/9 lacked TLS 1.3).
- Replacement: drop. `rustls` (via reqwest `rustls-tls-native-roots`) provides TLS 1.2 and 1.3 and reads the system trust store (`/etc/ssl/certs`, p11-kit on CachyOS). What the user loses: nothing.
- Related: libmpv gets `tls-verify=yes` and, Proposal, `tls-ca-file=/etc/ssl/certs/ca-certificates.crt` instead of Aniyomi's bundled `cacert.pem` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:134-135`).
- Difficulty: trivial. Risk: a user-installed corporate CA is honoured by `native-roots` but not by `webpki-roots`; use native roots.

### ConnectivityManager

- Usage: 1 file, 1 occurrence outside import lines. `app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt:10` (`val Context.connectivityManager`), consumed by `isOnline()` (`:16-26`), `isConnectedToWifi()` (`:31-45`) and `app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt:19-41` (`activeNetworkState()`, `networkStateFlow()` via `registerDefaultNetworkCallback` + `onCapabilitiesChanged`/`onLost`).
- Observed semantics:
  - `NetworkState(isConnected, isValidated, isWifi)` with `isOnline = isConnected && isValidated`; `isValidated` = `NET_CAPABILITY_VALIDATED` (the OS probed real internet reachability); `isWifi` = Wi-Fi enabled and active transport is `TRANSPORT_WIFI`.
  - `isOnline()` = active network has any transport in `TRANSPORT_CELLULAR..TRANSPORT_LOWPAN` (0..6: cellular, wifi, bluetooth, ethernet, vpn, wifi-aware, lowpan; on Android < 8.1 the upper bound is wifi-aware, 5).
  - `isConnectedToWifi()` (Q+): Wi-Fi enabled AND `TRANSPORT_WIFI` AND `NET_CAPABILITY_INTERNET`.
  - Callers: download jobs (`MangaDownloadJob.kt:57-80` pause with `download_notifier_text_only_wifi` / `download_notifier_no_network`), library update restriction "wifi" (`MangaLibraryUpdateJob.kt:98-100`), tracking retry (`TrackChapter`/`TrackEpisode`: queue the update for later when `!context.isOnline()`), error text choice (`app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:21-27`: an `UnknownHostException` while `!isOnline()` shows `exception_offline` = "No Internet connection", otherwise `exception_unknown_host` = "Couldn't reach %s"; an `HttpException` shows `exception_http` = "HTTP %d, check website in WebView").
- Replacement: `gio::NetworkMonitor::default()`: `is_network_available()` for `isConnected`, `connectivity() == NetworkConnectivity::Full` for `isValidated`, `is_network_metered()` for the "not metered" restriction, `connect_network_changed` for the flow (tokio `watch::Sender<NetworkState>`). Wi-Fi detection has no GIO API: Proposal, query NetworkManager over D-Bus (`zbus`, `org.freedesktop.NetworkManager` property `PrimaryConnectionType == "802-11-wireless"`); when NetworkManager is absent treat "wifi" as "not metered".
- Difficulty: moderate. Risk: `NetworkMonitor` may report `Limited` behind captive portals or on setups without a connectivity-check URL (systemd-networkd); treat `Limited`/`Portal` as offline only for the "online" check and let requests themselves fail naturally.

### HTTP logging switch (belongs with section 2 `DebugLogger`)

The HTTP logging interceptor (`NetworkHelper.kt:33-39`, `HttpLoggingInterceptor.Level.HEADERS` when the verbose-logging preference is on) maps to `tracing` events at target `kurobayomi::http` with header logging; Proposal: redact `Authorization`, `Cookie`, `Set-Cookie` (OkHttp prints them in clear).


## 2. Image pipeline, drawing and colour classes (Coil 3, android.graphics, androidx.core.graphics, animation)

Related areas: area:34-images-covers-cache, area:43-reader-core, area:44-reader-viewers, area:49-theme-and-design-system, area:35-data-misc-updater-notification-export.

Proposal (applies to the whole section): one `ImageLoader` service in Rust with a `Fetcher` trait (`Url`, `File`, `Bytes`), a `Keyer` (cache key string), a `Decoder` returning `Decoded`, a memory LRU (`moka::sync::Cache`, weight = decoded byte size, capacity 25 % of free RAM at start, Coil's default share) and a disk cache (`cacache`, size 2 % of the free space of the cache filesystem clamped to 10 MiB..250 MiB, Coil 3 defaults; Aniyomi sets neither, see `app/src/main/java/eu/kanade/tachiyomi/App.kt:182-211`). Fetch concurrency 8 and decode concurrency 3 mirror `fetcherCoroutineContext(Dispatchers.IO.limitedParallelism(8))` and `decoderCoroutineContext(Dispatchers.IO.limitedParallelism(3))` (`App.kt:208-209`): use two `tokio::sync::Semaphore`s with 8 and 3 permits. Crossfade is `300 ms * animatorDurationScale` (`App.kt:203`); Proposal: 300 ms, 0 when `gtk::Settings::is_gtk_enable_animations()` is false.

### CachePolicy

- Usage: 6 files, 9 occurrences outside import lines. `app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt:213`, `app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt:173`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt:38` (all `.memoryCachePolicy(CachePolicy.DISABLED)`), `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt:326-327,401-402` (memory and disk both `DISABLED` for page images), `presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt` (memory `DISABLED` while resizing covers).
- Library semantics (Coil 3.1.0, `gradle/libs.versions.toml:45`): enum `ENABLED(true,true)`, `READ_ONLY(true,false)`, `WRITE_ONLY(false,true)`, `DISABLED(false,false)` with `readEnabled`/`writeEnabled`. The cover fetchers consult `options.diskCachePolicy.readEnabled/writeEnabled` and `options.networkCachePolicy.readEnabled` (`MangaCoverFetcher.kt:112,191,221,247`; `AnimeImageFetcher.kt:113,192,222,248`).
- Replacement: `#[derive(Clone, Copy)] enum CachePolicy { Enabled, ReadOnly, WriteOnly, Disabled }` with `read_enabled()`/`write_enabled()`; `ImageRequest { memory: CachePolicy, disk: CachePolicy, network: CachePolicy }` defaulting to `Enabled`.
- Preserve: reader page images bypass both caches (they are held by the chapter cache / downloads, area:43-reader-core); the full-size cover/image dialogs and the save-image notification bypass only the memory cache.
- Difficulty: trivial. Risk: none.

### CircleCropTransformation

- Usage: 2 files, 2 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt:306` and `.../anime/AnimeLibraryUpdateNotifier.kt:320`: `ImageRequest.Builder(context).data(manga).transformations(CircleCropTransformation()).size(NOTIF_ICON_SIZE).build()` with `NOTIF_ICON_SIZE = 192` (`:399` / `:412`); the bitmap becomes the large icon of the "new chapters/episodes" notification.
- Library semantics (Coil): centre-crop to a square of `min(width, height)`, then draw an anti-aliased circle mask (`PorterDuff.Mode.SRC_IN`); output is `min x min` pixels.
- Replacement: `image` crate. `fn circle_crop(img: &RgbaImage, size: u32) -> RgbaImage`: scale-to-fill to `size x size` (Lanczos3), then multiply alpha by coverage `clamp(r - dist + 0.5, 0, 1)` where `r = size / 2` and `dist` is the distance from the pixel centre to the image centre. Pass to the notification as `image-data` (hint `(iiibiiay)`) via `gio::Notification::set_icon(&gio::BytesIcon)` or `notify-rust` `Hint::ImageData`.
- Preserve: 192 px edge; circle crop; fall back to the app icon when the cover fails to load (Aniyomi returns `null` and the notification shows no large icon).
- Difficulty: trivial. Risk: KDE Plasma renders `image-data` at about 64-128 px; 192 px is downscaled by the shell.

### DataSource

- Usage: 3 files, 11 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt` (returns `DataSource.DISK` for cover-cache/disk-cache files, `DataSource.NETWORK` for a fresh download, `DataSource.DISK` when `response.cacheResponse != null` else `NETWORK`), `AnimeImageFetcher.kt:90,102,131,151` (same), `BufferedSourceFetcher.kt:21` (`DataSource.MEMORY`).
- Library semantics: enum `MEMORY_CACHE, MEMORY, DISK, NETWORK`; Coil uses `MEMORY_CACHE` to skip the crossfade animation and for logging.
- Replacement: `enum ImageOrigin { MemoryCache, Memory, Disk, Network }` carried in `FetchResult`; the UI fades in only when the origin is not `MemoryCache`.
- Difficulty: trivial. Risk: none.

### DebugLogger

- Usage: 1 file, 1 occurrence outside import lines. `App.kt:205`: `if (networkPreferences.verboseLogging().get()) logger(DebugLogger())`. The same preference enables OkHttp header logging at client creation (`NetworkHelper.kt:33`); both are read once at start-up, so toggling needs a restart in Aniyomi.
- Replacement: `tracing` + `tracing-subscriber` (`EnvFilter` behind a `reload::Handle`). Verbose on = filter `kurobayomi::image=debug,kurobayomi::http=debug,info`; off = `info`. Proposal: apply the toggle live through the reload handle (an improvement; Aniyomi needs a restart).
- Difficulty: trivial. Risk: none.

### DecodeResult

- Usage: 1 file, 2 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt:22,56`: `DecodeResult(image = bitmap.asImage(), isSampled = sampleSize > 1)`.
- Observed decoder algorithm (`TachiyomiImageDecoder.kt:20-70`):
  1. `ImageDecoder.newInstance(inputStream, options.cropBorders, displayProfile)` (native library `tachiyomi.decoder`; `displayProfile` is a static `ByteArray?` ICC profile set from the reader "display profile" preference, `ReaderActivity.kt` `setDisplayProfile`).
  2. Fail with `"Failed to initialize decoder"` unless width and height are > 0.
  3. Destination size = `options.size.widthPx(options.scale) { srcWidth }` / `heightPx`; `sampleSize = DecodeUtils.calculateInSampleSize(srcW, srcH, dstW, dstH, scale)` (Coil: largest power of two `highestOneBit(src/dst)` per axis, `min` of the two for `Scale.FILL`, `max` for `Scale.FIT`, floor 1).
  4. `decoder.decode(sampleSize)`; fail with `"Failed to decode image"` if null.
  5. If the request asked for `Bitmap.Config.HARDWARE` and `ImageUtil.canUseHardwareBitmap(bitmap)` copy to a hardware bitmap.
  6. Return `isSampled = sampleSize > 1`.
  - The factory applies this decoder only when `options.customDecoder` is set or the sniffed `ImageUtil.ImageType` is `AVIF`, `JXL` or `HEIF` (`TachiyomiImageDecoder.kt:65-80`; the `customDecoder` extra is `app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt:37-42`); everything else uses Android's decoders.
- Replacement: `struct Decoded { texture: gdk::Texture, width: u32, height: u32, is_sampled: bool }`. Decoders: `image` crate for PNG/JPEG/GIF/WebP/BMP; `libheif-rs` for HEIF and AVIF; `jxl-oxide` for JXL. Sampling: decode full size then `fast_image_resize` down by the same power-of-two `sample_size` so memory use matches; `is_sampled` tells the cache not to reuse the entry for a larger request. Hardware bitmaps have no equivalent; `gdk::MemoryTexture` uploads to GL lazily.
- Crop borders (`options.cropBorders`): implemented in the native decoder in Aniyomi; Proposal: a Rust pass that trims rows/columns whose pixels are all within a tolerance of the edge colour; algorithm specified in area:44-reader-viewers.
- Difficulty: moderate (hard for `cropBorders` and ICC display profile; colour management through `lcms2` crate `Profile::new_icc`). Risk: JXL/HEIF crate build requirements (`libheif` system package, `dav1d`/`aom` for AV1); document in the README.

### Canvas

- Usage: 1 file, 1 occurrence outside import lines. `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderNavigationOverlayView.kt:65` (`override fun onDraw(canvas: Canvas)`).
- Observed: the reader tap-zone overlay. For each region of the active navigation layout (normalised `RectF` 0..1) it scales the canvas by `(width, height)`, fills the rect with `regionPaint.color = region.type.color`, then translates to the rect's top-left and draws the region name (`stringResource(region.type.nameRes)`) centred at `(width * |l-r|/2, height * |t-b|/2)` twice: first with `textBorderPaint` (`Color.BLACK`, `textSize = 64f`, `Paint.Style.STROKE`, `strokeWidth = 8f`, centre aligned) then with `textPaint` (fill). A click starts `animate().alpha(0f).setDuration(FADE_DURATION)` with `FADE_DURATION = 1000L` (`:116`) and hides the view on end.
- Replacement: `gtk::DrawingArea::set_draw_func` with `cairo::Context` (`rectangle`, `fill`, `text_path` + `set_line_width(8.0)` + `stroke_preserve`, `fill`), font size 64 only if the window is phone-sized; Proposal: scale font to `min(width, height) / 12`. Fade: put the area inside a `gtk::Revealer` with `RevealerTransitionType::Crossfade` and `set_transition_duration(1000)`.
- Difficulty: trivial. Risk: none.

### Color

- Usage: 18 files, 29 occurrences outside import lines. Groups: (a) tracker brand colours `Tracker.getLogoColor(): Int` (`app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt:27`), (b) reader backgrounds, (c) subtitle colour parsing, (d) image-analysis primitives.
- (a) Exact values: Anilist `Color.rgb(18, 25, 35)` (`Anilist.kt:80`), Bangumi `rgb(240, 145, 153)` (`Bangumi.kt:150`), Jellyfin `rgb(0, 11, 37)` (`Jellyfin.kt:39`), Kavita `rgb(74, 198, 148)` (`Kavita.kt:40`), Kitsu `rgb(51, 37, 50)` (`Kitsu.kt:59`), Komga `rgb(51, 37, 50)` (`Komga.kt:37`), MangaUpdates `rgb(146, 160, 173)` (`MangaUpdates.kt:48`), MyAnimeList `rgb(46, 81, 162)` (`MyAnimeList.kt:63`), Shikimori `rgb(40, 40, 40)` (`Shikimori.kt:181`), Simkl `rgb(0, 0, 0)` (`Simkl.kt:99`), Suwayomi `rgb(255, 35, 35)` (`Suwayomi.kt:24`, flagged with a to-do comment in the source as a placeholder). They paint the rounded background behind the tracker logo (`app/src/main/java/eu/kanade/presentation/track/components/TrackLogoIcon.kt:36`).
- (b) Reader backgrounds (`ReaderActivity.kt:834,845-851`): `grayBackgroundColor = Color.rgb(0x20, 0x21, 0x25)`; theme index 0 -> `Color.WHITE`, 1 (else branch) -> `Color.BLACK`, 2 -> gray, 3 -> `automaticBackgroundColor()`.
- (c) `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsColorsCard.kt:204`: `android.graphics.Color.parseColor(it.uppercase())` on the string returned by `MPVLib.getPropertyString(sub-color | sub-border-color | sub-back-color | ...)`. mpv returns `#AARRGGBB` (alpha first) or `#RRGGBB`; `parseColor` throws `IllegalArgumentException` on malformed input and the code force-unwraps.
- (d) `ImageUtil.kt:343-590` reads `.red/.green/.blue/.alpha` of ARGB ints (see ColorDrawable).
- Replacement: `type Argb = u32` (`0xAARRGGBB`), `fn rgb(r,g,b) -> Argb { 0xFF00_0000 | r<<16 | g<<8 | b }`, accessors `alpha/red/green/blue`, `fn parse_mpv_color(s: &str) -> Option<Argb>` accepting `#RRGGBB` and `#AARRGGBB` (case-insensitive) and returning `None` instead of panicking, `fn to_rgba(c: Argb) -> gdk::RGBA` (`RGBA::new(r/255.0, g/255.0, b/255.0, a/255.0)`). Do not use `gdk::RGBA::parse` for 8-digit hex: GDK reads `#RRGGBBAA`, the opposite order.
- Tracker logo colours: store as a `const LOGO_COLOR: Argb` on each tracker (area:27-tracking-core); apply with a per-tracker CSS class generated from the constant.
- Difficulty: trivial. Risk: alpha-order mix-up between mpv (`AARRGGBB`) and GDK (`RRGGBBAA`).

### ColorDrawable

- Usage: 1 file, 7 occurrences outside import lines. `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:344,346,410,525,526,547,572` (all inside `chooseBackground(context, imageStream): Drawable`, lines 338-575, the "automatic" reader background; the return type is `ColorDrawable` or a `GradientDrawable`).
- Observed algorithm summary (the owning plan is area:43-reader-core; the primitives are fixed here): image decoded fully with `ImageDecoder`; if null or width < 50 or height < 50 -> solid white. Sample points: `top = 5`, `bot = height - 5`, `left = (int)(width * 0.0275)`, `right = width - left`, `midX = width / 2`, `midY = height / 2`, `offsetX = (int)(width * 0.01)`. Pixel predicates (`:582-589`): `isDark` = `red < 40 && blue < 40 && green < 40 && alpha > 200`; `isCloseTo(other)` = each channel difference `< 30`; `isWhite` = `red + blue + green > 740`. Four columns (`left`, `right`, `left - offsetX`, `right + offsetX`) are scanned at `y` in `0 until height step height / 25` counting white/dark streaks (thresholds 14, 6, 22, 18, 9 as in the source) and the result is: solid colour of `topLeftPixel` when all six top/bottom sample points are non-white and mutually close; solid `blackColor`/white in landscape; otherwise a 4-colour vertical gradient or a solid colour per the table at `:538-575`.
- Gradient semantic (`GradientDrawable(Orientation.TOP_BOTTOM, intArrayOf(c0, c0, c1, c1))`): four evenly spaced stops at 0, 1/3, 2/3, 1, so `c0` for the top third, a blend across the middle third, `c1` for the bottom third.
- Replacement: `enum PageBackground { Solid(Argb), Gradient { top: Argb, bottom: Argb } }` returned by `fn choose_background(img: &RgbaImage, landscape: bool) -> PageBackground`; painted with `gtk::Snapshot::append_color` / `append_linear_gradient` using `gsk::ColorStop`s at offsets `0.0, 1/3, 2/3, 1.0`. `landscape` = window width > height (replaces `Configuration.ORIENTATION_LANDSCAPE`).
- Difficulty: trivial for the drawable, moderate for porting `chooseBackground` (about 240 lines of pixel heuristics; copy line by line and unit-test against fixtures).
- Risk: the heuristic must be ported exactly or backgrounds flicker between pages.

### ColorMatrix

- Usage: 1 file, 2 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:810-833` (`getCombinedPaint(grayscale, invertedColors)`).
- Observed: start with the identity matrix; if `grayscale` call `setSaturation(0f)` (Android saturation matrix with `s = 0`: rows R,G,B each `[0.213, 0.715, 0.072, 0, 0]`, alpha row `[0,0,0,1,0]`); if `invertedColors` `postConcat` the matrix `[-1,0,0,0,255; 0,-1,0,0,255; 0,0,-1,0,255; 0,0,0,1,0]`. Both on: output rgb = `255 - (0.213 R + 0.715 G + 0.072 B)` for all three channels.
- Replacement: `graphene::Matrix` + offset `graphene::Vec4` in the 0..1 domain: grayscale rows `[0.213, 0.715, 0.072, 0]` x3 and `[0,0,0,1]`, offset 0; invert = diag(-1,-1,-1,1) with offset `(1,1,1,0)` (255/255); both = rows `[-0.213, -0.715, -0.072, 0]` x3, offset `(1,1,1,0)`. Build with `Matrix::from_float` (graphene is column-major: transpose when filling from row notation) and unit-test with the pixels white, black, `#FF0000`.
- Difficulty: moderate (transposition errors). Risk: GSK applies the matrix to non-premultiplied colour; transparent pixels are unaffected.

### ColorMatrixColorFilter

- Usage: 1 file, 1 occurrence outside import lines. `ReaderActivity.kt:810-812` (`getCombinedPaint`) wraps the matrix in a `Paint.colorFilter`, set as the layer paint of the viewer container (`setLayerPaint(grayscale, invertedColors)` re-run whenever either preference changes, `ReaderActivity.kt:866-868`).
- Replacement: a small `ColorFilterBin` widget (subclass of `gtk::Widget` overriding `snapshot`) that wraps the viewer child: `snapshot.push_color_matrix(&matrix, &offset); self.snapshot_child(child, snapshot); snapshot.pop();`, skipped entirely when both options are off (no extra offscreen pass). Update with `queue_draw()` on preference change.
- Difficulty: moderate. Risk: an offscreen pass over a full-window viewer costs GPU time; it is only active when the user enables a filter.

### ColorUtils

- Usage: 3 files, 3 occurrences outside import lines. `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsMainScreen.kt:68` and `.../player/PlayerSettingsMainScreen.kt:68` (`ColorUtils.colorToHSL(surface.toArgb(), arr)` then `getPalerSurface()`), `ReaderActivity.kt:498` (`ColorUtils.setAlphaComponent(SurfaceColors.SURFACE_2.getColor(this), if (isNightMode()) 230 else 242)`, "90% dark 95% light", used for the system bars colour).
- Observed `getPalerSurface()` (`SettingsMainScreen.kt:60-72`): convert surface colour to HSL (`h` 0..360, `s`,`l` 0..1), `l = l - 0.05` in dark mode or `l + 0.02` in light mode, clamp to `0..1`, rebuild. It tints the two-pane settings list background.
- Replacement: `palette` crate (`Hsl::from_color(Srgb)`) or 25 lines of RGB<->HSL. The pale-surface rule is kept for the settings navigation pane; the colour source is the theme background (read it from the widget `Settings`/CSS: `gtk::StyleContext::lookup_color("theme_bg_color")` still works in GTK 4.16 though deprecated; fallback `#242424` dark / `#fafafa` light). Status/navigation bar colouring is dropped (desktop has no system bars); the toolbar alpha (230/255 dark, 242/255 light) is reused for the reader's translucent top and bottom bars via CSS `alpha(@theme_bg_color, 0.90 or 0.95)`.
- Difficulty: trivial. Risk: `lookup_color` deprecation; isolate it in one helper.

### DecelerateInterpolator

- Usage: 1 file, 2 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt:126` (zoom animation, duration `ANIMATOR_DURATION_TIME = 200` ms, `:346`) and `:164` (`zoomFling`, `animatorSet.duration = 400`).
- Library semantics: factor 1.0 -> `f(t) = 1 - (1 - t)^2`.
- Replacement: no libadwaita `Animation`; use `widget.add_tick_callback` driven by the frame clock: `t = (frame_time - start) / duration`, value `= from + (to - from) * (1 - (1 - t).powi(2))`, remove the callback at `t >= 1`. Animated quantities are translation X/Y and the scale rate (zoom) or translation only (fling; fling distance `distanceTimeFactor * velocity / 2`, clamped by `getPositionX/Y`). Respect `gtk-enable-animations` by snapping to the end value.
- Difficulty: trivial. Risk: none.

### CircularProgressIndicator

- Usage: 5 files, 5 occurrences outside import lines. (1) Material Views: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt:103` and `.../webtoon/WebtoonTransitionHolder.kt:105` (`CircularProgressIndicator(context)` with `isIndeterminate = true`, followed by a text `transition_pages_loading`). (2) `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderProgressIndicator.kt:21` wrapping `CombinedCircularProgressIndicator` in `presentation-core/src/main/java/tachiyomi/presentation/core/components/CircularProgressIndicator.kt`. (3) Glance `presentation-widget/.../UpdatesAnimeWidget.kt:43` and `UpdatesMangaWidget.kt:43` (drop with widgets).
- Observed `CombinedCircularProgressIndicator(progress)`: if `progress() == 0f` show indeterminate, else determinate whose arc follows `progress` (animated with `ProgressIndicatorDefaults.ProgressAnimationSpec`) AND the whole arc rotates continuously `0 -> 360` over `tween(2000, LinearEasing)` repeating, "to give feedback that the application isn't stuck"; switch between the two with a cross-fade. `ReaderProgressIndicator.setProgress(0..100)` divides by 100 and `layoutParams` is `WRAP_CONTENT` centred.
- Replacement: indeterminate = `gtk::Spinner`; determinate = a custom `gtk::DrawingArea` (`Cairo` arc from `-90 deg`, sweep `2*pi*progress`, line width 4, rotated by `angle = (frame_time_ms % 2000) / 2000 * 360 deg` via a tick callback, colour = the widget's `color()`), shown in a `gtk::Stack` with `StackTransitionType::Crossfade`.
- Difficulty: moderate (custom drawing). Risk: none.

### ContentScale

- Usage: 2 files, 4 occurrences outside import lines. `presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaCover.kt:36,44` and the anime twin: `ContentScale.Crop` for the cover image and for the `appwidget_cover_error` placeholder drawable.
- Replacement: Glance widgets are dropped (see Column and ColorProvider). The same crop behaviour elsewhere is `gtk::Picture::set_content_fit(gtk::ContentFit::Cover)` (GTK 4.8+) with `set_can_shrink(true)`.
- Difficulty: trivial. Risk: none.

### ColorProvider

- Usage: 10 files, 10 occurrences outside import lines. Ten files under `presentation-widget/src/main/java/tachiyomi/presentation/widget/` (for example `components/manga/UpdatesMangaWidget.kt:33` `contentColor: ColorProvider`, `LockedMangaWidget.kt:24` `foreground: ColorProvider`).
- What it does: Glance's theme-aware colour handle for Android home-screen widgets (AppWidgets).
- Decision: drop, with `Column` and `ContentScale` (rationale in the next item). User loses: the home-screen "recent updates" cover grid. Replacement for the information: the in-app Updates tab (area:42-ui-updates-history-stats-storage-upcoming) and desktop notifications for new items.
- Difficulty: trivial (nothing to port). Risk: none.

### Column

- Usage: 2 files, 2 occurrences outside import lines. `presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt:51` and `.../anime/UpdatesAnimeWidget.kt:51`: `androidx.glance.layout.Column(modifier = GlanceModifier.fillMaxHeight(), verticalAlignment = CenterVertically, horizontalAlignment = CenterHorizontally)` laying out rows of covers.
- Decision: the two Glance widgets (`UpdatesGridGlanceWidget`, `UpdatesGridCoverScreenGlanceWidget`, plus their receivers, locked variants and `MangaWidgetManager`/`AnimeWidgetManager`) are dropped: GTK applications cannot host desktop widgets, and KDE Plasma widgets are written in QML, outside this project (area:50-android-widgets carries the formal drop task). Kept as data for an optional "recent updates strip" inside the app: cover `58 x 87` dp (`UpdatesMangaCover.kt:16-17`), inner corner radius `12` dp (`presentation-widget/src/main/res/values/dimens.xml`), data window `ZonedDateTime.now().minusMonths(3)` (`BaseMangaUpdatesGridGlanceWidget.kt` `DateLimit`), `distinctBy { it.mangaId }` then `take(rowCount * columnCount)`.
- Layout equivalent if the strip is built: `gtk::FlowBox` (`gtk::Box` vertical rows inside a `gtk::ScrolledWindow`).
- Difficulty: trivial. Risk: none.


## 3. Android framework classes (android.os, android.content, android.net, androidx.core, androidx.activity, androidx.lifecycle)

Related areas: area:01-foundation-app-bootstrap, area:36-ui-shell-navigation-security-webview, area:52-util-app, area:53-platform-manifest-intents-workers, area:35-data-misc-updater-notification-export.

Proposal (applies to the whole section): the Android `Context` disappears. Process-wide facts live in an `AppPaths`/`AppEnv` struct created in `main()`; per-window state lives in the GTK `Application`/`ApplicationWindow`. Desktop notifications are `gio::Notification` actions routed to `gio::SimpleAction`s on the `gtk::Application` (D-Bus activatable, so a click launches the app if it was closed; needs an installed `.desktop` file with `DBusActivatable=true`).

### BroadcastReceiver

- Usage: 11 files, 14 occurrences outside import lines. All eleven receivers:
  1. `app/src/main/java/eu/kanade/tachiyomi/App.kt:246` `DisableIncognitoReceiver`: action `ACTION_DISABLE_INCOGNITO_MODE = "tachi.action.DISABLE_INCOGNITO_MODE"` (`App.kt:274`), registered with `RECEIVER_NOT_EXPORTED` while incognito is on (`App.kt:255-262`), sent by the content `PendingIntent.getBroadcast` of the ongoing incognito notification (`App.kt:125-132`, `FLAG_ONE_SHOT or FLAG_IMMUTABLE`; tapping the notification turns incognito off); sets `basePreferences.incognitoMode().set(false)`.
  2. `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt:53` "global" receiver for every notification button. Action strings are `"$ID.NotificationReceiver.<X>"` with `ID = BuildConfig.APPLICATION_ID` (`:379`). `<X>` values: `SHARE_IMAGE`, `SEND_BACKUP`, `CANCEL_RESTORE`, `CANCEL_LIBRARY_UPDATE`, `CANCEL_ANIMELIB_UPDATE`, `ACTION_START_APP_UPDATE`, `CANCEL_APP_UPDATE_DOWNLOAD`, `MARK_AS_READ`, `MARK_AS_SEEN`, `ACTION_OPEN_CHAPTER`, `ACTION_OPEN_EPISODE`, `ACTION_DOWNLOAD_CHAPTER`, `ACTION_DOWNLOAD_EPISODE`, `ACTION_OPEN_ENTRY`, `ACTION_RESUME_DOWNLOADS`, `ACTION_PAUSE_DOWNLOADS`, `ACTION_CLEAR_DOWNLOADS`, `ACTION_RESUME_ANIME_DOWNLOADS`, `ACTION_PAUSE_ANIME_DOWNLOADS`, `ACTION_CLEAR_ANIME_DOWNLOADS`, `ACTION_DISMISS_NOTIFICATION`. Extras (`:412-417`, each `"$ID.NotificationReceiver.<NAME>"`): `URI`, `NOTIFICATION_ID` (int, default -1), `EXTRA_GROUP_ID`, `EXTRA_MANGA_ID`, `EXTRA_CHAPTER_ID` (longs, default -1), `EXTRA_CHAPTER_URL`.
  3. `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/InstallerAnime.kt:27` and `.../manga/installer/InstallerManga.kt:27` `cancelReceiver` (cancel an install session).
  4. `.../anime/installer/PackageInstallerInstallerAnime.kt:23` and `.../manga/installer/PackageInstallerInstallerManga.kt:24` `packageActionReceiver` (PackageInstaller session results; registered `RECEIVER_EXPORTED` for anime at `:108-112`, `RECEIVER_NOT_EXPORTED` for manga at `:123-127`).
  5. `.../anime/util/AnimeExtensionInstallReceiver.kt:24` and `.../manga/util/MangaExtensionInstallReceiver.kt:24`: filter `ACTION_PACKAGE_ADDED`, `ACTION_PACKAGE_REPLACED`, `ACTION_PACKAGE_REMOVED` (data scheme `package`) plus app-private `"${BuildConfig.APPLICATION_ID}.ACTION_EXTENSION_ADDED|REPLACED|REMOVED"` (`:123-125`); loads the extension in a `CoroutineScope(SupervisorJob())` and calls the listener (`onExtensionInstalled`, `onExtensionUntrusted`, ...); an add that is part of a replace is ignored (`isReplacing`).
  6. `.../anime/util/AnimeExtensionInstaller.kt:262` and `.../manga/util/MangaExtensionInstaller.kt:262` `DownloadCompletionReceiver` for `DownloadManager.ACTION_DOWNLOAD_COMPLETE` (registered `RECEIVER_EXPORTED`, `:277`).
  7. `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:147` `noisyReceiver`: on `AudioManager.ACTION_AUDIO_BECOMING_NOISY` calls `viewModel.pause()` and clears `FLAG_KEEP_SCREEN_ON`.
  8. `PlayerActivity.kt:812` `pipReceiver`: filter `PIP_INTENTS_FILTER = "pip_control"`, extra `PIP_INTENT_ACTION = "media_control"` with `PIP_PAUSE = 1`, `PIP_PLAY = 2`, `PIP_PREVIOUS = 3`, `PIP_NEXT = 4`, `PIP_SKIP = 5` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt:107-113`); `PIP_SKIP` = `seekBy(10)`.
- Replacement per receiver: (1) and (2): `enum NotificationAction` mapped 1:1 to `gio::SimpleAction` names on the application (`app.disable-incognito`, `app.mark-as-read` with an `(x x)` parameter manga/chapter id, `app.open-chapter`, `app.download-chapter`, `app.pause-downloads`, `app.resume-downloads`, `app.clear-downloads` and the anime variants, `app.cancel-library-update`, `app.cancel-restore`, `app.start-app-update`, `app.cancel-app-update`, `app.share-image`, `app.share-backup`, `app.open-entry`); notification buttons use `gio::Notification::add_button_with_target_value`. `ACTION_DISMISS_NOTIFICATION` becomes `Application::withdraw_notification(id)`. `SHARE_*` become "Open containing folder" (`gtk::FileLauncher::open_containing_folder`, GTK 4.10) because there is no system share sheet. (3), (4), (6): drop with APK installation (area:20-extension-strategy-redesign owns how extensions are delivered); extension downloads are plain `reqwest` futures whose completion needs no broadcast. (5): `gio::FileMonitor` on the extensions directory (or the `notify` crate) emitting `ExtensionAdded/Replaced/Removed`; debounce 300 ms; a create immediately followed by a rename-over counts as "replaced". (7): `libpulse-binding` subscription (`Facility::Sink`, `Operation::Removed`) or `pipewire` registry events: when the currently used default sink disappears pause playback; P2, otherwise dropped (user loses auto-pause on headphone unplug). (8): Picture-in-Picture is Android-only; dropped (user loses the PiP window); alternative P3 "always on top" mini window via `gtk::Window` + X11 `_NET_WM_STATE_ABOVE` only (Wayland compositors do not allow it).
- Difficulty: moderate. Risk: `gio::Notification` buttons only work after the app registers on the session bus with its application id; verify on KDE with the Flatpak and the plain binary.

### Build

- Usage: 47 files, 95 occurrences outside import lines. 95 occurrences. Distribution of the symbols (`grep` over `*.kt`): `Build.VERSION.SDK_INT` 74, `VERSION_CODES.Q` 18, `P` 15, `S` 13, `TIRAMISU` 12, `UPSIDE_DOWN_CAKE` 6, `R` 5, `O_MR1` 4, `S_V2` 1; device strings `MANUFACTURER` 4, `DEVICE` 4, `MODEL` 3, `DISPLAY` 3, `VERSION.RELEASE` 2, `RELEASE_OR_PREVIEW_DISPLAY` 1, `RELEASE_OR_CODENAME` 1, `INCREMENTAL` 1, `SUPPORTED_ABIS` 1, `PRODUCT` 1, `BRAND` 1. `minSdk = 26` (`buildSrc/src/main/kotlin/mihon/buildlogic/AndroidConfig.kt:9`), `compileSdk = TARGET_SDK = 36`.
- Observed purposes:
  - API-level gates: all are "feature available on newer Android" (notifications permission on Tiramisu `PermissionStep.kt:59,85`, `MonetColorScheme.kt:23-25,59,78`, foreground-service type on Q in every job `MangaDownloadJob.kt:48` and others, MediaStore saving on Q `ImageSaver.kt:41`, TLS `App.kt:89`, animated WebP on P / HEIF on R `ImageUtil.kt:79-81`, `BitmapRegionDecoder` on S `ImageUtil.kt:601`, `overrideActivityTransition` (activity open/close slide animations `shared_axis_x_push_enter`/`shared_axis_x_pop_enter`, replaced by `overridePendingTransition` below API 34) in `ReaderActivity.kt:147,296`, `WebViewActivity.kt:43,100` and `SecureActivityDelegate.kt:112`).
  - Device quirks (all Android-only): `DeviceUtil.isSamsung` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt:47-48`), MIUI detection and `miuiMajorVersion` (`Build.VERSION.INCREMENTAL`, `core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt:13-28`), Zidoo TV box (`app/src/main/java/eu/kanade/tachiyomi/util/system/TvUtils.kt:38`), hardware-bitmap blacklist keyed on `Build.MODEL`/`Build.DEVICE` for API 26 and 27 (`ImageUtil.kt:621-665`), the WebView pre-initialisation skip on MIUI or Samsung+S (`WebViewInterceptor.kt:37`).
  - Diagnostics text: `CrashLogUtil.kt:48-53` (`Android version: ${Build.VERSION.RELEASE} (SDK ${Build.VERSION.SDK_INT}; build ${Build.DISPLAY})`, `Android build ID`, `Device brand`, `Device manufacturer`, `Device name: ${Build.DEVICE} (${Build.PRODUCT})`, `Device model`) and `DebugInfoScreen.kt:118,138-148`.
  - Update asset choice: `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:42` picks the APK by `Build.SUPPORTED_ABIS[0]` (`map[abi] ?: map[null]`).
  - mpv cache size: `AniyomiMPVView.kt:138-140` `cacheMegs = if (SDK_INT >= O_MR1) 64 else 32`.
- Replacement: every API-level gate resolves to "true" (the Linux target is always the newest feature set); delete the branches. Device quirks are dropped (the lists are Android hardware). Diagnostics: a `DebugInfo` struct filled from `std::env::consts::{OS, ARCH}`, `/etc/os-release` (`PRETTY_NAME`), kernel from `uname` (`rustix::system::uname()`), GTK version (`gtk::major_version()/minor_version()/micro_version()`), `libmpv` version (`mpv_client_api_version`), desktop session (`XDG_CURRENT_DESKTOP`, `XDG_SESSION_TYPE`), GPU renderer (`gdk::Display` GL info), app version from `env!("CARGO_PKG_VERSION")`. Update asset choice: match the release asset name containing `std::env::consts::ARCH` (`x86_64`, `aarch64`). mpv demuxer cache: keep 64 MiB (`demuxer-max-bytes` and `demuxer-max-back-bytes` = 67 108 864), desktop users may raise it in the player settings (area:48-settings-player-and-scripts).
- Difficulty: trivial. Risk: none; the loss is the Android-device workarounds, irrelevant on Linux.

### Bundle

- Usage: 16 files, 19 occurrences outside import lines. Two roles. (a) Activity lifecycle: `onCreate(savedInstanceState: Bundle?)` in `MainActivity.kt:158`, `ReaderActivity.kt:145`, `PlayerActivity.kt:233`, `BaseActivity.kt:21`, `CrashActivity.kt:13`, `UnlockActivity.kt:20`, `WebViewActivity.kt:42`, `BaseOAuthLoginActivity.kt:19`, `DeepLinkMangaActivity.kt:11`, `DeepLinkAnimeActivity.kt:11`, both `*ExtensionInstallActivity.kt:25`, `onSaveInstanceState` in `MainActivity.kt:575` and `PlayerActivity.kt:990`, `PreferenceFragmentCompat.onCreatePreferences(savedInstanceState, rootKey)` in both `*SourcePreferencesScreen.kt:130`. (b) HTTP header carrier for external apps: `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt:736,756` (1DM extra `extra_headers`; ADM extra `android.media.intent.extra.HTTP_HEADERS` with the value `a.second.replace("http", "h_ttp")`), `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:215` (`headers` for Web Video Caster).
- Replacement: (a) dropped: nothing is destroyed and recreated on a desktop; the resume state that matters (reader/player position, last tab) is already in the database or in the persisted UI state file `$XDG_STATE_HOME/kurobayomi/ui.toml` (area:02-preferences-system). (b) see ComponentName: external downloaders and players are spawned as processes with explicit arguments; headers become `--header` style arguments.
- Difficulty: trivial. Risk: none; user loses nothing on desktop.

### ClipData

- Usage: 3 files, 3 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:44` (`ClipData.newPlainText(label, content)`), `ReaderActivity.kt:749` (`ClipData.newUri(contentResolver, "", uri)` for "copy page image"), `app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt:26` (`ClipData.newRawUri(null, uri)` so a share target gets read permission).
- Replacement: text: `gdk::Display::default().clipboard().set_text(&s)`. Image: `clipboard.set_texture(&gdk::Texture)` (decode the page file to a texture) or `set_content(&gdk::ContentProvider::for_bytes("image/png", bytes))`. File URI: `gdk::FileList`/`ContentProvider::for_value(&gio::File)`. The share-permission clip (`newRawUri`) is dropped.
- Difficulty: trivial. Risk: on Wayland the clipboard content disappears when the app exits unless a clipboard manager (Klipper on Plasma) holds it; acceptable.

### ClipboardManager

- Usage: 2 files, 2 occurrences outside import lines. `ContextExtensions.kt:43` and `ReaderActivity.kt:748`.
- Observed `Context.copyToClipboard(label, content)` (`ContextExtensions.kt:36-55`): no-op when `content.isBlank()`; `setPrimaryClip(ClipData.newPlainText(label, content))`; on Android <= 12L (`SDK_INT <= S_V2`) show toast `copied_to_clipboard` ("Copied to clipboard:\n%1$s") with `content.truncateCenter(50)` (`truncateCenter(count, replacement = "...")`: if `length <= count` unchanged, else `pieceLength = floor((count - replacement.length) / 2)`, result `take(pieceLength) + "..." + takeLast(pieceLength)`, so 50 -> 23 + 3 + 23); on any `Throwable` log and toast `clipboard_copy_error` ("Failed to copy to clipboard"). 24 call sites use it.
- Replacement: `fn copy_to_clipboard(text: &str)` doing the same, always showing the toast (desktop has no system confirmation), toast implemented as an in-app overlay (a `gtk::Overlay` with a self-hiding `gtk::Revealer`, 2 s).
- Difficulty: trivial. Risk: none.

### By

- Usage: 1 file, 7 occurrences outside import lines. `macrobenchmark/src/main/java/tachiyomi/macrobenchmark/BaselineProfileGenerator.kt:20-30`: `device.findObject(By.text("Manga")).click()` then `"Updates"`, `"History"`, `"More"`, `"Settings"` (a UI Automator script used to record an Android baseline profile; the Browse/Extensions lines are commented out).
- Decision: drop. Baseline profiles and macrobenchmarks are ART ahead-of-time compilation aids with no Rust counterpart. Replacement for the intent (smoke test navigation): a `cargo test` UI smoke test that builds the window headless (`gtk::init()` under `xvfb-run`/`weston --backend=headless`) and activates each main tab action; P3.
- User loses: nothing.
- Difficulty: trivial. Risk: none.

### ComponentActivity

- Usage: 2 files, 2 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:361` (cast to register new-intent listeners) and `app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt:26` (`inline fun ComponentActivity.setComposeContent(parent, content)` wrapping every screen in `TachiyomiTheme { CompositionLocalProvider(LocalTextStyle provides bodySmall, LocalContentColor provides onBackground) }`).
- Replacement: `gtk::ApplicationWindow` (main, reader, player windows). The theme wrapper disappears: the GTK theme styles everything; default body text uses the theme font (Proposal: no override of font sizes, see area:49-theme-and-design-system).
- Difficulty: trivial. Risk: none.

### Consumer

- Usage: 1 file, 1 occurrence outside import lines. `MainActivity.kt:362`: `Consumer<Intent> { trySend(it) }` registered with `addOnNewIntentListener`, inside a `callbackFlow` (`HandleOnNewIntent`, `:357-367`) so `handleIntentAction` runs for each new intent delivered to the single-instance main activity. Deep links declared in `app/src/main/AndroidManifest.xml`: `tachiyomi://add-repo` (`:71-72`), `aniyomi://add-repo` (`:82-83`), `aniyomi://extension-store` (`:93-94`), a `file`/`content` VIEW filter for any mime type (`:104-107`, opens backup files) and text/plain share filters (`:155`, `:186`). The OAuth redirects `aniyomi://{myanimelist-auth,anilist-auth,bangumi-auth,shikimori-auth,simkl-auth}` (`:252-258`) go to a separate activity (`BaseOAuthLoginActivity`), not through this consumer.
- Replacement: the single-instance behaviour = `gtk::Application` with `ApplicationFlags::HANDLES_COMMAND_LINE | HANDLES_OPEN`; a second launch forwards its arguments/URIs to the primary instance over D-Bus and fires `connect_command_line`/`connect_open`; the handler sends a message through an `async_channel` consumed by the navigation controller (a `gtk` main-context task). The desktop file registers `MimeType=x-scheme-handler/aniyomi;x-scheme-handler/tachiyomi;` so the add-repo, extension-store and the five `*-auth` OAuth redirect links open Kurobayomi (the redirect URIs are registered with each tracker's OAuth client, so the `aniyomi://` scheme and host names must stay unchanged unless Kurobayomi registers its own client ids; area:27-tracking-core).
- Difficulty: trivial. Risk: none.

### Configuration

- Usage: 13 files, 23 occurrences outside import lines. (1) Orientation checks, 8 sites: `app/src/main/java/eu/kanade/presentation/library/manga/MangaLibraryPager.kt:73`, `.../anime/AnimeLibraryPager.kt:73`, both `*LibrarySettingsDialog.kt:251`, both `Browse*SourceScreenModel.kt:128,139`, `MigrateSeasonSelectScreenModel.kt:54` (`isLandscape` selects the portrait/landscape grid column count preference). (2) Tablet UI: `app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt:12-39`: `TABLET_UI_REQUIRED_SCREEN_WIDTH_DP = 720`, `TABLET_UI_MIN_SCREEN_WIDTH_PORTRAIT_DP = 700`, `TABLET_UI_MIN_SCREEN_WIDTH_LANDSCAPE_DP = 600`; `TabletUiMode` `AUTOMATIC` (smallest width >= 700 in portrait, >= 600 otherwise), `ALWAYS`, `LANDSCAPE` (only when landscape), `NEVER`. (3) Night mode: `isNightMode()` = `uiMode and UI_MODE_NIGHT_MASK == UI_MODE_NIGHT_YES` (`DisplayExtensions.kt:54`) and `createReaderThemeContext` (see ContextThemeWrapper). (4) `WindowSize.kt:12,19`: `screenWidthDp > MediumWidthWindowSize` / `> ExpandedWidthWindowSize` (Material window-size classes: medium > 600 dp, expanded > 840 dp). (5) `TvUtils.kt:16` `UI_MODE_TYPE_TELEVISION`. (6) `ThemePreviews.kt:13` preview annotations.
- Replacement: width class from the window's logical width: `compact < 600`, `medium 600..840`, `expanded > 840` (px = dp at scale 1); `is_landscape = width > height` read in `size-allocate`/`notify::default-width`. Two-pane layouts switch on `width >= 700` (portrait) / `>= 600` (landscape) which for a desktop window means "almost always two-pane"; Proposal: keep `TabletUiMode` out of the settings (desktop is always tablet-like; below 600 px width fall back to single pane). Dark mode: `gtk::Settings::is_gtk_application_prefer_dark_theme()` plus `gtk-theme-name` suffix `-dark`, or the XDG `org.freedesktop.appearance color-scheme` portal value via `ashpd` (2 = prefer light, 1 = dark). TV mode dropped.
- Difficulty: trivial. Risk: KDE does not set `prefer-dark`; use the portal/`gtk-theme-name` check so "System" theme mode follows Plasma (the user runs Tokyo Night GTK theme, which is dark regardless).

### ContentUris

- Usage: 1 file, 1 occurrence outside import lines. `app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt:141`: `ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, id)` inside `findUriOrDefault(path, filename, default)` which queries MediaStore for an existing row with `RELATIVE_PATH=? AND DISPLAY_NAME=?` (path normalised to end with `/`) so a re-save of the same name overwrites instead of creating `name (1).jpg`.
- Observed whole saver (`ImageSaver.kt:30-120`): file name = `DiskUtil.buildValidFilename("${image.name}.${type.extension}")`; the type is sniffed from the bytes (`findImageType`, error `"Not an image"`). Pictures location = `<Pictures|Documents>/<app_name>/<relativePath>`; Documents is used when the mime type is not known to `MimeTypeMap`. Display name is `image.name` for supported mimes else the full filename. A `Cover` is JPEG-compressed at quality 100. Any failure becomes `IOException(error_saving_picture)`.
- Replacement: drop MediaStore. `fn save_image(data: Bytes, name: &str, rel: &str) -> io::Result<PathBuf>`: directory `glib::user_special_dir(glib::UserDirectory::Pictures)` (fallback `~/Pictures`) `/Kurobayomi/<rel>`, `create_dir_all`, overwrite an existing file of the same name (matches `findUriOrDefault`), atomic write, then notify the file manager nothing else needed (no `scanMedia`). Cache images go to `user_cache_dir()/kurobayomi/shared_images`.
- Difficulty: trivial. Risk: filename sanitiser must match `buildValidFilename` (area:13-core-util-storage-archive-torrent).

### ComponentName

- Usage: 2 files, 11 occurrences outside import lines. `AnimeDownloader.kt:742` (`ComponentName("idm.internet.download.manager", "idm.internet.download.manager.Downloader")` for 1DM with extras `extra_filename = "$filename.mkv"`, `extra_headers`; `:765` `ComponentName(pkgName, "$pkgName.AEditor")` for ADM with extras `com.dv.get.ACTION_LIST_ADD = "<videoUrl><info>$filename.mkv"` and `com.dv.get.ACTION_LIST_PATH`), `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:332-345` (external player activities): `is.xyz.mpv` -> `$pkg.MPVActivity`; MX Player (`com.mxtech.videoplayer`, `.ad`, `.pro`) -> `$pkg.ActivityScreen`; VLC `org.videolan.vlc` -> `$pkg.gui.video.VideoPlayerActivity`; mpvKt (`live.mehiz.mpvkt`, `.preview`) -> `live.mehiz.mpvkt.ui.player.PlayerActivity`; mpv-remote `com.husudosu.mpvremote` -> `$pkg.MainActivity`; Just Player `com.brouken.player` -> `$pkg.PlayerActivity`; Next Player `dev.anilbeesetti.nextplayer` -> `$pkg.feature.player.PlayerActivity`; X Player `video.player.videoplayer` -> `com.inshot.xplayer.activities.PlayerActivity`; Web Video Caster `com.instantbits.cast.webvideo` (package only). Mime type per extension (`ExternalIntents.kt:314-322`, keyed on the last path segment after `.`: `mp4` -> `video/mp4`): `mkv` -> `video/x-matroska`, `m3u8` -> `application/x-mpegURL`, else `video/any`.
- Replacement: Android application-level IPC is dropped. Linux equivalents: "external player" setting = a desktop entry or command. Proposal: `ExternalPlayer { name, command: Vec<String> }` with presets `mpv`: `["mpv", "--force-media-title={title}", "--http-header-fields={headers}", "--sub-file={sub}", "--start={seconds}", "{url}"]`; `vlc`: `["vlc", "--meta-title={title}", "--http-user-agent={ua}", "--http-referrer={referer}", "{url}"]`; `celluloid`: `["celluloid", "{url}"]`; discover installed players by scanning `gio::AppInfo::all_for_type("video/x-matroska")`. Spawn with `std::process::Command` (no shell, arguments as separate argv entries so URLs with `;` or `&` are safe). External downloaders: preset `aria2c` (`--header="K: V"`, `--dir`, `--out`) and `yt-dlp` for HLS; 1DM and ADM are dropped (Android apps; user loses nothing on Linux).
- Difficulty: moderate. Risk: header quoting; always pass headers as one argv entry each.

### Context

- Usage: 184 files, 389 occurrences outside import lines. 184 files, the most pervasive class. Categories with counts (grep over `app core domain data presentation-core presentation-widget source-api`): string resources `stringResource`/`getString` 1877 sites; `PendingIntent` 169; `LocalContext.current` 137; toast 135; `startActivity` 50; `applicationContext` 48; `packageManager` 37; `SharedPreferences`/`PreferenceManager` 39; `contentResolver` 25; `resources.` 24; `cacheDir` 23; `getSystemService` 14; `filesDir` 10; `externalCacheDir` 5; `sendBroadcast` 4; `getExternalFilesDirs` 1.
- System services obtained: `ClipboardManager` x2, `DownloadManager` x2 (`MangaExtensionInstaller.kt:47`, `AnimeExtensionInstaller.kt:47`), `UiModeManager` (`MonetColorScheme.kt:79`, contrast), `PowerManager` (`PermissionStep.kt:65`, battery-optimisation check), `ActivityManager` (`DeviceUtil.kt:97` memory info for `isLowRamDevice`), `INPUT_METHOD_SERVICE` (`PlayerViewModel.kt:965`), `AUDIO_SERVICE` (`PlayerActivity.kt:122`, system volume), plus `ConnectivityManager`/`WifiManager` wrappers (see ConnectivityManager).
- Replacement table:
  - String resources -> `gettext` style catalogue loaded by the i18n layer (area:51-i18n): `fn tr(key: &str) -> String`, `fn trn(key, n)` for plurals.
  - Directories: `cacheDir`/`externalCacheDir` -> `glib::user_cache_dir()/kurobayomi`; `filesDir` -> `glib::user_data_dir()/kurobayomi`; configuration -> `glib::user_config_dir()/kurobayomi`; downloads default `glib::user_special_dir(UserDirectory::Videos | Documents)/Kurobayomi`; `getExternalFilesDirs` (`DiskUtil.getExternalStorages`, root of each mounted external volume) -> mounted volumes from `gio::VolumeMonitor::default().mounts()` (use `Mount::root().path()`).
  - `contentResolver` and `Uri` -> `gio::File`; `UniFile` -> `std::path::PathBuf` (area:13-core-util-storage-archive-torrent).
  - `startActivity(ACTION_VIEW url)` -> `gtk::UriLauncher::new(url).launch(...)` (GTK 4.10) with `openInBrowser` error surfaced as a toast with the error text, as `ContextExtensions.kt:75-88` does.
  - `PendingIntent` -> `gio::Notification` actions (see BroadcastReceiver).
  - Toast -> in-app toast overlay (Revealer) for short messages, desktop notification only for background events.
  - `SharedPreferences` -> TOML config (area:02-preferences-system).
  - `packageManager` -> dropped except external-player discovery (ComponentName).
  - `DownloadManager` -> `reqwest` download with progress; `UiModeManager` contrast -> dropped (GTK theme owns contrast); `PowerManager` check -> dropped (no battery optimiser), inhibit idle during playback/reading with `gtk::Application::inhibit(window, ApplicationInhibitFlags::IDLE, reason)` (portal `org.freedesktop.portal.Inhibit` under Flatpak); `ActivityManager` low-RAM -> dropped; `InputMethodManager` -> dropped; `AudioManager` -> mpv `volume` property (0-100, max 130 optional) and `ashpd`/`wpctl` only if system volume integration is wanted (area:45-player-core).
- Difficulty: moderate (breadth, not depth). Risk: global singletons; Proposal: a single `AppEnv` passed by `Arc` to services, avoiding `static mut`.

### ContextCompat

- Usage: 13 files, 18 occurrences outside import lines. `ContextCompat.getMainExecutor(context).execute { }`: `SettingsLibraryScreen.kt:254` (re-schedule both library update jobs after the restrictions preference changes), `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:242-251` (warm-up on the main executor: `get<NetworkHelper>()`, `get<MangaSourceManager>()`, `get<AnimeSourceManager>()`, `get<Database>()`, `get<AnimeDatabase>()` "for a faster cold start"), `app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt:45` (BiometricPrompt executor), `CloudflareInterceptor.kt:30` (UI-thread executor for the WebView). `registerReceiver(..., RECEIVER_NOT_EXPORTED|EXPORTED)` (see BroadcastReceiver). `startForegroundService(context, intent)` (`*ExtensionInstaller.kt:202`). `getDrawable(context, id)` (`app/src/main/java/eu/kanade/presentation/util/Resources.kt:25`, `rememberResourceBitmapPainter` throws `Resources.NotFoundException` when missing). `getExternalFilesDirs` (`DiskUtil.kt:19`).
- Replacement: main executor -> `glib::MainContext::default().invoke_local(...)`/`glib::idle_add_once`; warm-up -> `tokio::spawn` of DB open and source-manager load right after the first window is presented (do not block `activate`); biometric -> dropped (see area:36-ui-shell-navigation-security-webview; replacement is an app PIN or none); foreground service -> `gtk::Application::hold()` while a download/update job runs; `getDrawable` -> `gdk::Texture::from_resource("/org/kurobayomi/icons/<name>.png")` via `gio::resources_register_include!`.
- Difficulty: trivial. Risk: opening both SQLite files on the main thread freezes first paint; open them on the blocking pool.

### ContextThemeWrapper

- Usage: 3 files, 3 occurrences outside import lines. (1) `ContextExtensions.kt:137-147` `Context.createReaderThemeContext()`: dark background if reader theme 1 (Black) or 2 (Gray); for 3 (Automatic) it follows the app theme mode (`ThemeMode.SYSTEM` -> `isNightMode()`, else `themeMode == DARK`); white background (0) is light. If the resulting night flag differs from the current configuration it builds a `ContextThemeWrapper(this, R.style.Theme_Tachiyomi)`, `applyOverrideConfiguration` with the flipped `uiMode` night bits, re-applies `ThemingDelegate.getThemeResIds(appTheme, themeDarkAmoled)`. So the reader chrome (toolbar, bottom sheet, dialogs) matches the page background, regardless of the app theme. (2) `AnimeSourcePreferencesScreen.kt:127` / `MangaSourcePreferencesScreen.kt:127` wrap the fragment context with `R.attr.preferenceTheme` so extension-provided `PreferenceScreen`s render.
- Replacement: GTK 4 cannot switch dark/light per window. Proposal: reader/player windows set a CSS class `reader-dark` or `reader-light` on the window and a dedicated `CssProvider` defines the colours of the reader chrome explicitly (for example `background: rgba(32,33,37,0.9)`) so it contrasts with the page background independent of the global theme; colours derive from the same rule (dark for black/gray, light for white, automatic = luminance of the current page background). Extension preference screens: built from a declarative schema in Rust widgets (area:20-extension-strategy-redesign); no theme wrapper needed.
- Difficulty: moderate. Risk: explicit colours ignore the user's GTK theme in the reader chrome (user loses theme matching there; acceptable because pages are black/white/grey anyway).

### CreationExtras

- Usage: 1 file, 1 occurrence outside import lines. `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:148-150`: `PlayerViewModelProviderFactory.create(modelClass, extras)` builds `PlayerViewModel(activity, extras.createSavedStateHandle())`.
- Replacement: drop. `PlayerViewModel` becomes a plain Rust struct `PlayerController` owned by the player window, constructed with explicit dependencies (`Arc<SourceManager>`, `Arc<DownloadManager>`, ...). The `SavedStateHandle` (process-death restore of anime id, episode id, position) is replaced by the persisted history/progress rows.
- Difficulty: trivial. Risk: none.

### DateUtils

- Usage: 2 files, 2 occurrences outside import lines. (1) `app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt:37-43` `relativeTimeSpanString(epochMillis)`: `<= 0` -> `relative_time_span_never`; `now - epochMillis < 1 minute` -> `updates_last_update_info_just_now`; else `DateUtils.getRelativeTimeSpanString(epochMillis, now, DateUtils.MINUTE_IN_MILLIS).toString()`. (2) `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt:137` `DateUtils.formatElapsedTime(remainingTime.toLong())` for the sleep timer ("timer_remaining").
- Library semantics: `getRelativeTimeSpanString` with minute resolution gives locale text "N minutes ago" (< 1 h), "N hours ago" (< 24 h), "N days ago" (< 7 days), and for older dates a short absolute date (`FORMAT_SHOW_DATE | FORMAT_ABBREV_MONTH`, year added only when it differs from the current year, e.g. "Oct 3" / "Oct 3, 2025"); future times give "in N minutes". `formatElapsedTime(seconds)` gives `MM:SS`, or `H:MM:SS` when >= 3600 s (minutes zero-padded, hours not).
- Replacement: `fn relative_time(epoch_ms: i64, now: i64) -> String` with `chrono` and the i18n plural catalogue (`n_minutes_ago`, `n_hours_ago`, `n_days_ago`), absolute date via `glib::DateTime::format("%b %-d")` / `"%b %-d, %Y"` (locale aware); `fn format_elapsed(secs: u64) -> String { if secs >= 3600 { format!("{}:{:02}:{:02}", h, m, s) } else { format!("{:02}:{:02}", m, s) } }`. Keep the three special cases ahead of the generic path.
- Difficulty: moderate (localised plurals). Risk: Android's string table is localised by the OS; Kurobayomi must ship its own translations for the three plural keys (area:51-i18n).


## 4. Background work (androidx.work)

Related areas: area:26-library-update-jobs, area:23-downloads-manga, area:24-downloads-anime, area:21-backup-format-and-create, area:22-backup-restore, area:53-platform-manifest-intents-workers, area:35-data-misc-updater-notification-export.

Proposal (applies to both items below): an in-process `JobScheduler` built on tokio.

```
enum JobOutcome { Success, Failure, Retry }
enum Backoff { Linear(Duration), Exponential(Duration) }     // WorkManager caps the delay at 5 hours, min 10 s
enum Policy { Keep, Replace }                                 // ExistingWorkPolicy
struct JobSpec { unique_name: &'static str, tags: &'static [&'static str], constraints: Constraints, backoff: Backoff, policy: Policy }
struct PeriodicSpec { every: Duration, flex: Duration /* 10 min */, update: bool /* ExistingPeriodicWorkPolicy.UPDATE */ }
```

- `trait Job { async fn run(&self, ctx: JobCtx, cancel: CancellationToken) -> JobOutcome; }`; `JobCtx` carries the input data (typed struct instead of `workDataOf`).
- Unique work: a `HashMap<&str, JoinHandle>`; `Keep` ignores a second enqueue while one exists, `Replace` cancels the running token first.
- Periodic work: persist `last_run_ms` per unique name in the `scheduler` table (or config); on start-up compute `due = last_run + every`; if `now >= due` run immediately (catch-up), else sleep until `due` (WorkManager runs inside the last `flex` of the window, 10 minutes; Proposal: ignore the flex window, run at `due`). Minimum interval 15 minutes like WorkManager. `interval == 0` cancels (`cancelUniqueWork`).
- Retry: after `Retry` or a constraint failure wait `backoff * attempt` (Linear) or `backoff * 2^(attempt-1)` (Exponential), capped at 5 h.
- While any job runs hold the application (`gtk::Application::hold()`), so closing the last window does not kill downloads (Proposal: closing the window with active jobs asks to keep running in the background via a tray `StatusNotifierItem` using the `ksni` crate, or quits when the user confirms).
- No cross-process persistence for one-shot jobs: the work they carry (download queue, restore progress) is already persisted by their own services.

### Constraints

- Usage: 6 files, 6 occurrences outside import lines. Six call sites.
  1. `app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt:59` and `.../anime/service/DelayedAnimeTrackingUpdateJob.kt:66`: `Constraints(requiredNetworkType = NetworkType.CONNECTED)`; `OneTimeWorkRequestBuilder`, `setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 5, TimeUnit.MINUTES)`, tag and unique name `"DelayedMangaTrackingUpdate"` / `"DelayedAnimeTrackingUpdate"`, `ExistingWorkPolicy.REPLACE`.
  2. `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt:94`: `Constraints(requiresBatteryNotLow = true)`; `PeriodicWorkRequestBuilder(interval hours, flex 10 minutes)`, `BackoffPolicy.EXPONENTIAL, 10, MINUTES`, tag `TAG_AUTO = "BackupCreator"` (`:131`, manual tag `"BackupCreator:manual"`), `ExistingPeriodicWorkPolicy.UPDATE`, input `IS_AUTO_BACKUP_KEY = true`; interval is `backupPreferences.backupInterval()` (hours); `<= 0` -> `cancelUniqueWork(TAG_AUTO)`.
  3. `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:439-486` `setupTask` (and the anime twin `AnimeLibraryUpdateJob.kt:452-495`): device restrictions are the string set `autoUpdateDeviceRestrictions` whose members are `DEVICE_ONLY_ON_WIFI = "wifi"`, `DEVICE_NETWORK_NOT_METERED = "network_not_metered"`, `DEVICE_CHARGING = "ac"` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:399-401`). `networkType = UNMETERED if "network_not_metered" in restrictions else CONNECTED`; a `NetworkRequest` adds `TRANSPORT_WIFI` when `"wifi"` and `NET_CAPABILITY_NOT_METERED` when `"network_not_metered"`; `setRequiresCharging("ac" in restrictions)`; `setRequiresBatteryNotLow(true)` always. Periodic with `interval = autoUpdateInterval` hours and flex 10 minutes; `setBackoffCriteria(BackoffPolicy.LINEAR, 10, MINUTES)`; tags `TAG` + `WORK_NAME_AUTO`; unique names `"LibraryUpdate-auto"`, `"LibraryUpdate-manual"`, `"AnimeLibraryUpdate-auto"`, `"AnimeLibraryUpdate-manual"` (tags `"LibraryUpdate"`, `"AnimeLibraryUpdate"`); `ExistingPeriodicWorkPolicy.UPDATE`; interval 0 cancels the auto job.
  4. `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt:134`: `Constraints(requiredNetworkType = CONNECTED)`; unique/tag `"AppUpdateDownload"`, `ExistingWorkPolicy.REPLACE`, input `EXTRA_DOWNLOAD_URL = "DOWNLOAD_URL"`, `EXTRA_DOWNLOAD_TITLE = "DOWNLOAD_TITLE"`.
- Constraint evaluation to implement:
  - `network_connected` -> `gio::NetworkMonitor::is_network_available()` (Full connectivity preferred).
  - `unmetered` -> `!NetworkMonitor::is_network_metered()`.
  - `wifi` -> NetworkManager `PrimaryConnectionType == "802-11-wireless"` (area:12-network-stack), else `unmetered`.
  - `charging` ("ac") -> UPower (`org.freedesktop.UPower` object `/org/freedesktop/UPower`, property `OnBattery == false`) via `zbus`; a machine without a battery is always "charging".
  - `battery_not_low` -> UPower display device `/org/freedesktop/UPower/devices/DisplayDevice` property `Percentage > 15.0` (WorkManager treats the OS "battery low" broadcast, about 15 %, as low) or `OnBattery == false`; no battery -> satisfied.
  - A job whose constraints are unmet waits and re-evaluates on `NetworkMonitor::network-changed` and UPower `PropertiesChanged`; it is not a failure.
- Observed fallback in `doWork` for Android < 9 (`MangaLibraryUpdateJob.kt:95-101`): "wifi only" is checked by hand and `Result.retry()` returned when not on Wi-Fi; irrelevant on Linux (single code path).
- Difficulty: moderate. Risk: a desktop that is suspended misses periodic runs; catch-up on resume (`logind` `PrepareForSleep(false)` via `zbus`) re-evaluates `due`.

### CoroutineWorker

- Usage: 12 files, 12 occurrences outside import lines. Twelve classes (eleven jobs plus the `setForegroundSafely` helper `suspend fun CoroutineWorker.setForegroundSafely` in `app/src/main/java/eu/kanade/tachiyomi/util/system/WorkManagerExtensions.kt`).

| Class | Path | Kind | Unique name / tags | Existing policy | Notes |
|---|---|---|---|---|---|
| `MangaLibraryUpdateJob` | `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt` | periodic + manual | `LibraryUpdate-auto`, `LibraryUpdate-manual` | UPDATE / KEEP | `startNow(category)` returns false when tag `LibraryUpdate` is already RUNNING; `stop()` cancels running work found by tag |
| `AnimeLibraryUpdateJob` | `.../data/library/anime/AnimeLibraryUpdateJob.kt` | periodic + manual | `AnimeLibraryUpdate-auto`, `AnimeLibraryUpdate-manual` | UPDATE / KEEP | same, manual input `KEY_CATEGORY` (long, -1 = all) |
| `MangaMetadataUpdateJob` | `.../data/library/manga/MangaMetadataUpdateJob.kt` | manual | `MetadataUpdate` | KEEP | `MANGA_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 60` (`:178`; progress queue warning when one source has more entries than this) |
| `AnimeMetadataUpdateJob` | `.../data/library/anime/AnimeMetadataUpdateJob.kt` | manual | `MetadataUpdate` | KEEP | same constant name and value at `AnimeMetadataUpdateJob.kt:170` |
| `MangaDownloadJob` | `.../data/download/manga/MangaDownloadJob.kt` | long-running | `MangaDownloader` | REPLACE | stays alive while `downloadManager.isRunning && networkCheck` |
| `AnimeDownloadJob` | `.../data/download/anime/AnimeDownloadJob.kt` | long-running | `AnimeDownloader` | REPLACE | same |
| `BackupCreateJob` | `.../data/backup/create/BackupCreateJob.kt` | periodic + manual | `BackupCreator`, `BackupCreator:manual` | UPDATE | foreground id `ID_BACKUP_PROGRESS` |
| `BackupRestoreJob` | `.../data/backup/restore/BackupRestoreJob.kt` | manual | `BackupRestore` | KEEP | `stop()` = `cancelUniqueWork`; cancellation reports `restoring_backup_canceled` and returns success |
| `AppUpdateDownloadJob` | `.../data/updater/AppUpdateDownloadJob.kt` | manual | `AppUpdateDownload` | REPLACE | APK download with progress |
| `DelayedMangaTrackingUpdateJob` | `app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt` | one-shot retry | `DelayedMangaTrackingUpdate` | REPLACE | retries stored tracker updates (`DelayedMangaTrackingStore`) |
| `DelayedAnimeTrackingUpdateJob` | `app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt` | one-shot retry | `DelayedAnimeTrackingUpdate` | REPLACE | same |

- Result contract used everywhere: `Result.success()`, `Result.failure()`, `Result.retry()`. Library update `doWork` (`MangaLibraryUpdateJob.kt:94-137`): when started as the auto job and a manual job is RUNNING return `retry()`; then `setForeground(getForegroundInfo())` (ignoring `IllegalStateException`); `libraryPreferences.lastUpdatedTimestamp().set(now)`; `categoryId = inputData.getLong(KEY_CATEGORY, -1L)`; queue; run; `CancellationException` is mapped to `Result.success()` ("assume success although cancelled"); any other exception -> `Result.failure()`; always `notifier.cancelProgressNotification()` in `finally`.
- Download job loop (`MangaDownloadJob.kt:56-100`): network gate `checkNetworkState(state, requireWifi)`: online and (not requireWifi or isWifi) -> continue; online but no wifi -> `downloaderStop(download_notifier_text_only_wifi)`; offline -> `downloaderStop(download_notifier_no_network)`; failing the first check returns `Result.failure()`; otherwise `setForegroundSafely()` (which waits 500 ms after `setForeground`), collects `combineTransform(networkStateFlow(), downloadOnlyOverWifi().changes())` and spins `while (active) { active = !isStopped && downloadManager.isRunning && networkCheck }` (a busy loop in Aniyomi).
- Foreground service info (`ForegroundInfo(notificationId, notification, FOREGROUND_SERVICE_TYPE_DATA_SYNC on Q+)`) exists so Android does not kill the process; ids `Notifications.ID_LIBRARY_PROGRESS`, `ID_BACKUP_PROGRESS`, `ID_RESTORE_PROGRESS`, `ID_APP_UPDATER`, `ID_DOWNLOAD_CHAPTER_PROGRESS`.
- Replacement: each class becomes an `impl Job` registered with the scheduler above; `setForeground` becomes "post/update the progress notification" (`gio::Notification` with the same id string; GNotification has no progress bar, Proposal: update the body text "N/M" and also show progress in the in-app Downloads/Updates views, area:23-downloads-manga). The busy loop is replaced by awaiting a `tokio::sync::watch` change (`running`, `network_ok`) so an idle download job consumes no CPU.
- Inputs `workDataOf(...)` become typed args: `LibraryUpdateArgs { category_id: Option<i64> }`, `BackupCreateArgs { auto: bool, location: PathBuf, options: BackupOptions }`, `RestoreArgs { location: PathBuf, sync: bool, options: RestoreOptions }`, `AppUpdateArgs { url, title }`.
- Difficulty: moderate. Risk: periodic jobs only run while the application runs; a systemd user timer (`kurobayomi --run-job library-update`) is a P2 follow-up; user loses background updates while the app is closed until then.

## 5. Language-level helpers, database and serialization adapters

Related areas: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:05-data-repos-entries-items, area:13-core-util-storage-archive-torrent, area:14-source-local-and-metadata.

### ColumnAdapter

- Usage: 1 file, 6 occurrences outside import lines. `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt` defines six adapters; they are bound in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:130-155` and declared in the SQL with `AS`: `history.sq:6 last_read INTEGER AS Date`, `animehistory.sq:6 last_seen INTEGER AS Date`, `mangas.sq:13 genre TEXT AS List<String>`, `animes.sq:15 genre TEXT AS List<String>`, `mangas.sq:25 update_strategy INTEGER AS UpdateStrategy NOT NULL DEFAULT 0`, `animes.sq:27 update_strategy INTEGER AS AnimeUpdateStrategy NOT NULL DEFAULT 0`, `animes.sq:33 fetch_type INTEGER AS FetchType NOT NULL DEFAULT 1`, `episodes.sq:24 memo BLOB AS JsonObject NOT NULL DEFAULT '{}'`, `animes.sq:40 memo BLOB AS JsonObject NOT NULL DEFAULT '{}'`.
- Exact semantics:
  - `DateColumnAdapter`: `Date(databaseValue: Long)` / `value.time` -> epoch milliseconds in an INTEGER.
  - `StringListColumnAdapter`: separator `", "` (comma + space, `LIST_OF_STRINGS_SEPARATOR`). Decode: empty string -> empty list, else `split(", ")`; encode `joinToString(", ")`. A NULL column is NULL (the `?.let` callers pass `null` through, e.g. `AnimeRepositoryImpl.kt:197`). Edge case: a genre that itself contains `", "` splits in two on reload (observed; keep for backup compatibility).
  - `MangaUpdateStrategyColumnAdapter` / `AnimeUpdateStrategyColumnAdapter`: ordinal; `entries.getOrElse(value) { ALWAYS_UPDATE }`. Ordinals: `ALWAYS_UPDATE = 0`, `ONLY_FETCH_ONCE = 1` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt`, `.../animesource/model/AnimeUpdateStrategy.kt`).
  - `FetchTypeColumnAdapter`: `getOrElse(value) { FetchType.Episodes }`; ordinals `Seasons = 0`, `Episodes = 1` (`.../animesource/model/FetchType.kt`); the SQL default `1` means `Episodes`.
  - `MemoColumnAdapter`: JSON object serialised with `value.toString().encodeToByteArray()` and read with `Json.decodeFromString<JsonObject>(bytes.decodeToString())`.
- Connection setup (`AppModule.kt:80-125`): two databases `tachiyomi.db` (manga) and `tachiyomi.animedb` (anime), both with `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL` executed in `onOpen`; the requery SQLite build is used in release (newer SQLite than the OS one).
- Replacement: `rusqlite` (feature `bundled` so the SQLite version is fixed) with newtypes implementing `rusqlite::types::{FromSql, ToSql}`:
  - `struct EpochMs(i64)`, `struct GenreList(Vec<String>)` (`split(", ")`, empty -> empty vec), `enum UpdateStrategy { AlwaysUpdate = 0, OnlyFetchOnce = 1 }` with `from_i64` falling back to `AlwaysUpdate`, `enum FetchType { Seasons = 0, Episodes = 1 }` falling back to `Episodes`, `struct Memo(serde_json::Map<String, Value>)`.
  - `Memo::column_result` must accept both `ValueRef::Blob` and `ValueRef::Text` because the default literal `'{}'` is stored as TEXT in a BLOB-affinity column (`rusqlite` `Vec<u8>` would reject TEXT); write always as BLOB of the UTF-8 JSON.
  - Open both files with `Connection::open_with_flags` and run the three pragmas on every new connection (`rusqlite::Connection::pragma_update`); use a small pool (`r2d2_sqlite`) with `max_size = 4` because WAL allows concurrent readers.
- Keep the file names and schema versions so an existing Aniyomi database can be opened unchanged (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations).
- Difficulty: trivial. Risk: Kotlin `Date` is millisecond-based while backups use seconds in some fields; always store ms in the DB, convert at the backup boundary (area:21-backup-format-and-create).

### Charset

- Usage: 1 file, 1 occurrence outside import lines. The item name is `nl.adaptivity.xmlutil.XmlDeclMode.Charset` (enum constant), used once: `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:185` `xmlDeclMode = Charset` in the singleton `XML { defaultPolicy { ignoreUnknownChildren() }; autoPolymorphic = true; xmlDeclMode = Charset; indent = 2; xmlVersion = XmlVersion.XML10 }` (`:180-188`).
- What it does: the XML encoder writes an XML declaration that includes the encoding (UTF-8), instead of none or version-only. The `XML` instance is used to write `ComicInfo.xml` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:677` `xml.encodeToString(ComicInfo.serializer(), comicInfo)`; `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:187`) and to read it back (unknown child elements ignored, `ignoreUnknownChildren`).
- Replacement: `quick-xml` with `serde` (`quick_xml::se::Serializer`, `indent(' ', 2)`) and a manually prefixed declaration `<?xml version="1.0" encoding="UTF-8"?>\n`; deserialisation with `quick_xml::de::from_str` into a struct without `deny_unknown_fields` (unknown elements ignored). Quote style of the declaration in Aniyomi output is not verifiable from this repository (it depends on the xmlutil platform writer); ComicInfo consumers (Komga, Mihon, Kavita) accept both quote styles.
- Difficulty: trivial. Risk: element order and attribute vs child representation of `ComicInfo` fields must match the `ComicInfo` serializer exactly (area:14-source-local-and-metadata).

### CaseInsensitiveSimpleNaturalComparator

- Usage: 1 file, 1 occurrence outside import lines. Single wrapper `String.compareToCaseInsensitiveNaturalOrder` (`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:36-39`, library `com.github.gpanther:java-nat-sort:natural-comparator-1.1`, `gradle/libs.versions.toml:54`). Callers: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt:18` (sort archive entries), `.../loader/DirectoryPageLoader.kt:19` (sort page files, `name.orEmpty()`), `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:292` (`c2.name.compareToCaseInsensitiveNaturalOrder(c1.name)`, chapter fallback order descending), `:342` and `:356` (sort directory entries / archive entries to find the first image used as the cover, `updateCover`), `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:286` (episodes by `episode_number` descending, ties name descending).
- Exact algorithm (library source `AbstractSimpleNaturalComparator.java`, read from the upstream repository `gpanther/java-nat-sort`, verified):
  1. Walk both strings with indices over UTF-16 code units. At each step read one char from each.
  2. If one is an ASCII digit (`'0'..'9'`) and the other is not: the digit side sorts first (`-1` when the first string has the digit).
  3. If neither is a digit: compare `Character.toLowerCase(c1) - Character.toLowerCase(c2)`; non-zero returns immediately.
  4. If both are digits: read each maximal digit run as an unsigned 64-bit number (`num = num * 10 + digit`, silently wrapping on overflow), compare as unsigned; if different return that order; otherwise continue after both runs (so `01` equals `1`, leading zeros are ignored).
  5. When one string ends first: if the first string still has characters return `1`, else if the second does return `-1`, else `0`.
- Replacement: write this function by hand (about 40 lines) instead of using the `natord` crate, whose handling of leading zeros and whitespace differs. `fn natural_cmp(a: &str, b: &str) -> Ordering` over `a.encode_utf16()` vectors (to keep UTF-16 code-unit semantics, which only differ for characters above U+FFFF) with `char::to_lowercase().next()` per code unit via `char::from_u32` (surrogates compared as-is), digits ASCII only, `u64::wrapping_mul/wrapping_add`.
- Test vectors: `["page10.jpg", "page2.jpg", "Page1.jpg"]` sorts to `Page1.jpg, page2.jpg, page10.jpg`; `["a01", "a1"]` compare equal; `["1", "a"]` -> `1` first; `["", "a"]` -> empty first.
- Difficulty: trivial. Risk: reader page order depends on it; any deviation reorders pages of local/archive chapters.

### ConstraintLayout

- Usage: 5 files, 5 occurrences outside import lines. `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt:164` (the whole controls overlay) and four panels: `.../components/panels/AudioDelayPanel.kt:57`, `SubtitleDelayPanel.kt:81`, `SubtitleSettingsPanel.kt:66`, `VideoFiltersPanel.kt` (each places one card).
- Observed `PlayerControls` layout (all inside `CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Ltr)` so left/right never mirror): container fills the window, background `Brush.verticalGradient(0f to Black, 0.2f to Transparent, 0.7f to Transparent, 1f to Black)` with alpha `transparentOverlay`, horizontal padding `MaterialTheme.padding.medium` = 16 dp (`presentation-core/.../material/Constants.kt:18`). Margins use `spacing.medium` = 16 dp. References and constraints:
  - `topLeftControls`: top = parent.top + 16, start = parent.start, width fills between its start and `topRightControls.start` (`fillToConstraints`).
  - `topRightControls`: top = parent.top + 16, end = parent.end.
  - `unlockControlsButton`: top = parent.top + 16, start = parent.start + 16.
  - `brightnessSlider`: vertically centred (top and bottom linked to parent), horizontally at `start = parent.start + 16`, or `end = parent.end + 16` when "swap volume and brightness" (`gesturePreferences.swapVolumeBrightness()`); `volumeSlider` takes the opposite side. Both auto-hide 2000 ms after the last change (`LaunchedEffect(volume, mpvVolume, isVolumeSliderShown) { delay(2000) ... }`).
  - `centerControls`: `start = parent.absoluteLeft`, `end = parent.absoluteRight`, top/bottom to parent (centred).
  - `playerUpdates`: horizontally centred, vertical bias `0.2f` (20 % from the top).
  - `seekbar`: `bottom = parent.bottom + 16`.
  - `bottomRightControls`: `bottom = seekbar.top`, `end = seekbar.end`; `bottomLeftControls`: `bottom = seekbar.top`, `start = seekbar.start`, width fills to `bottomRightControls.start`.
  - `thumbnail`: `fillMaxWidth`, `bottom = seekbar.top + 16`.
- Replacement: a `gtk::Overlay` whose main child is the mpv `gtk::GLArea`/dmabuf widget and whose overlays are: a vertical `gtk::Box` (top row `gtk::Box` with `hexpand` left group + right group, margin-top 16, margin-start/end 16; spacer `vexpand`; bottom column: thumbnail, bottom-controls row, seekbar with margin-bottom 16); `halign/valign` centred children for centre controls; sliders as `gtk::Scale` (vertical) with `valign = Center` and `halign = Start/End` + 16 px margins; `playerUpdates` in a small custom `gtk::LayoutManager` implementing `y = 0.2 * (parent_height - child_height)`. Force LTR with `widget.set_direction(gtk::TextDirection::Ltr)` on the overlay root. The gradient scrim = a `gtk::DrawingArea`/snapshot linear gradient with the stops above. Panels: each is a `gtk::Box` card `halign=Center valign=Center` inside the overlay (positions in area:46-player-controls-ui).
- Difficulty: moderate (bias layout). Risk: mpv rendering widget must stay below the overlay; use `gtk::Overlay::add_overlay` with `set_measure_overlay(false)` and `set_clip_overlay(true)`.

## 6. Build and benchmark tooling (dropped)

Related area: area:54-build-release-distribution-docs.

### CommonExtension

- Usage: 1 file, 2 occurrences outside import lines. `buildSrc/src/main/kotlin/mihon/buildlogic/ProjectExtensions.kt:26,65`: `configureAndroid(commonExtension: CommonExtension<*, *, *, *, *, *>)` sets `compileSdk = 36`, `buildToolsVersion = "35.0.1"`, `minSdk = 26`, `ndk.version = "27.1.12297006"` (`AndroidConfig.kt:7-11`), Java 17 source/target with core library desugaring, Kotlin `jvmTarget` 17, `-opt-in=kotlin.RequiresOptIn`, `allWarningsAsErrors` driven by the Gradle property `warningsAsErrors`; `configureCompose` enables `buildFeatures.compose` and the Compose BOM.
- Replacement: the root `Cargo.toml` workspace: `[workspace.package] edition = "2021"`, `rust-version` = minimum stable that supports gtk4-rs 0.11 and `[workspace.lints.rust] warnings = "deny"` only for CI (set through `RUSTFLAGS="-D warnings"` in the CI job, matching the opt-in `warningsAsErrors` property); `[profile.release] lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`. Core library desugaring, NDK and SDK levels have no equivalent (drop).
- Difficulty: trivial. Risk: none.

### CompilationMode

- Usage: 1 file, 6 occurrences outside import lines. `macrobenchmark/src/main/java/tachiyomi/macrobenchmark/StartupBenchmark.kt:52-71`: cold/warm/hot startup benchmarks (10 iterations each, `StartupTimingMetric`, package `xyz.jmir.tachiyomi.mibenchmark`) over `CompilationMode.None()`, `Partial(baselineProfileMode = Disable, warmupIterations = 1)`, `Partial(baselineProfileMode = Require)`, `Full()`.
- Decision: drop (ART compilation modes do not exist for native code). Replacement for the intent (watch start-up time): a `criterion`-free manual check, `hyperfine 'kurobayomi --quit-after-first-frame'` documented in area:54-build-release-distribution-docs, and a `tracing` span `startup` from `main()` to the first `map` signal of the main window logged at `info`; P3.
- User loses: nothing.
- Difficulty: trivial. Risk: none.

### ComposeCompilerGradlePluginExtension

- Usage: 1 file, 1 occurrence outside import lines. `ProjectExtensions.kt:78-95`: sets `featureFlags`, and when Gradle properties `enableComposeCompilerMetrics` / `enableComposeCompilerReports` are true writes `compose-metrics` / `compose-reports` under the root build dir.
- Decision: drop (Compose does not exist in the Rust app). User loses nothing.
- Difficulty: trivial. Risk: none.

### ComposeFeatureFlag

- Usage: 1 file, 1 occurrence outside import lines. `ProjectExtensions.kt:79`: `featureFlags.set(setOf(ComposeFeatureFlag.OptimizeNonSkippingGroups))` (a Compose compiler optimisation that removes redundant groups from non-skippable functions).
- Decision: drop. The GTK equivalent concern (avoid rebuilding large widget trees) is handled by list models (`gio::ListStore` + `gtk::ListView`/`GridView` with factories) in the library, updates and browse screens (area:37-ui-library).
- Difficulty: trivial. Risk: none.

