# Area 12-network-stack, part 1: behavior spec

OBSERVED = what the Kotlin does (cited as `path:line`). "Proposal:" = what Rust should do. All paths are repo-relative to `/home/blackbox/aniyomi-src`. Short names: `N/` = `core/common/src/main/java/eu/kanade/tachiyomi/network/`, `U/` = `core/common/src/main/java/eu/kanade/tachiyomi/util/system/`. Deep dives: `p1-topic-cloudflare-and-webview.md`, `p1-topic-doh.md`, `p1-topic-rate-limiter.md`, `p1-topic-http-cache.md`.

Library versions that decide behaviour (`gradle/libs.versions.toml:5,29`): OkHttp `5.4.0` (core, logging-interceptor, brotli, zstd, dnsoverhttps), QuickJS `app.cash.quickjs:quickjs-android:0.9.2`. OkHttp library defaults that this document relies on (not in the repo, from OkHttp 5.x): redirects followed up to 20, `retryOnConnectionFailure=true`, `writeTimeout=10 s`, HTTP/2 with ALPN, `ConnectionPool` 5 idle connections / 5 minutes, `Dispatcher` `maxRequests=64` and `maxRequestsPerHost=5`. These apply because the client is never given other values.

---------------------------------------------------------------------------------------------------

## 1. Client construction (`N/NetworkHelper.kt`)

### 1.1 OBSERVED construction order (lines 18-64)

1. `cookieJar = AndroidCookieJar()` (field, public; used by Settings "Clear cookies", `WebViewActivity.clearCookies`, `WebViewScreenModel.clearCookies`).
2. `clientBuilder` (private `run` block), executed once when `NetworkHelper` is constructed:
   1. `OkHttpClient.Builder().cookieJar(cookieJar)`
   2. `.connectTimeout(30, SECONDS)`
   3. `.readTimeout(30, SECONDS)`
   4. `.callTimeout(2, MINUTES)` (whole call: DNS, connect, write, server time, reading the body)
   5. `.cache(Cache(directory = File(context.cacheDir, "network_cache"), maxSize = 5L * 1024 * 1024))` (5 MiB)
   6. `.addInterceptor(UncaughtExceptionInterceptor())`
   7. `.addInterceptor(UserAgentInterceptor(::defaultUserAgentProvider))`
   8. if `preferences.verboseLogging().get()`: `.addNetworkInterceptor(HttpLoggingInterceptor().apply { level = HEADERS })`
   9. `when (preferences.dohProvider().get())` selects one of 13 `builder.doh*()` calls, else `builder` unchanged. Each `doh*()` calls `dns(DnsOverHttps.Builder().client(build())...build())`, and `build()` here snapshots the builder as it is at that moment (cookie jar, the three timeouts, cache, interceptors 6-7, the logging network interceptor if present). The resolver's own HTTP client is that snapshot, so DoH lookups send the default `User-Agent`, share the 5 MiB cache and write network logs, but never pass through the Cloudflare interceptor.
3. `client = clientBuilder.addInterceptor(CloudflareInterceptor(context, cookieJar, ::defaultUserAgentProvider)).build()`.
4. `cloudflareClient` is a deprecated alias of `client` (since extensions-lib 1.5; `@Deprecated("The regular client handles Cloudflare by default")`).
5. `defaultUserAgentProvider() = preferences.defaultUserAgent().get().trim()`, read on every call (so interceptors see a changed value at once; objects that cached it do not, see 4.3).

Resulting application-interceptor order for `network.client`: `UncaughtExceptionInterceptor` -> `UserAgentInterceptor` -> `CloudflareInterceptor`. Extensions and trackers derive clients with `client.newBuilder().addInterceptor(x)`; OkHttp appends, so their interceptors run after Cloudflare and nearer the network (for example `rateLimit(...)` in `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt:42`: `.addInterceptor(interceptor).rateLimit(permits = 85, period = 1.minutes)`). Consequence: the Cloudflare retry (`chain.proceed(request)`) passes through those later interceptors again, so a retry also takes a rate-limit permit.

OkHttp's own internal interceptors (retry/follow-up, bridge: cookies, `Accept-Encoding: gzip`, `Content-Length`, `Host`, `Connection: Keep-Alive`; cache; connect) sit between the application interceptors and the network interceptors. Only `gzip` is requested and decoded; `okhttp-brotli` and `okhttp-zstd` are on the classpath but no `BrotliInterceptor` or `CompressionInterceptor` is added in `NetworkHelper`, so responses are never `br` or `zstd` encoded by request.

`NetworkHelper` is constructed asynchronously at start-up on the main executor (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:241-244`, `ContextCompat.getMainExecutor(app).execute { get<NetworkHelper>() ... }`) and registered as a singleton (`AppModule.kt:200`). The DoH choice and `verbose_logging` are therefore frozen for the process lifetime: the settings screen shows toast `MR.strings.requires_app_restart` ("Requires app restart to take effect") after changing them.

### 1.2 Consumers of the shared client (OBSERVED)

| Consumer | How it uses the client | Cited |
|---|---|---|
| Manga and anime HTTP sources | `HttpSource.client` / `AnimeHttpSource.client` default to `network.client`; `headersBuilder()` adds `User-Agent: network.defaultUserAgentProvider()`; `getImage` uses `client.newCachelessCallWithProgress(imageRequest(page), page).awaitSuccess()` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:34,68,371-372`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:36,82` |
| Coil image loader (covers, anime images) | `OkHttpNetworkFetcherFactory(callFactoryLazy::value)` and the four custom fetchers take `network.client` unless the source overrides `client` | `app/src/main/java/eu/kanade/tachiyomi/App.kt:184-197`, `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt:171`, `.../AnimeImageFetcher.kt:172` |
| Trackers | `BaseTracker.client = networkService.client`; each API builds an "auth client" with `newBuilder().addInterceptor(...)`; Komga and Jellyfin replace DNS with `Dns.SYSTEM` ("don't use DNS over HTTPS as it breaks IP addressing") | `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt:17-20`, `.../komga/Komga.kt:28-31`, `.../jellyfin/Jellyfin.kt:28-31` |
| Extension repo index | `ExtensionRepoService.client.newCall(GET("$repo/repo.json")).awaitSuccess().parseAs<ExtensionRepoMetaDto>()` | `domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt:13-33` |
| Manga extension list | `MangaExtensionApi` uses `networkService.client` (the anime extension API, `.../extension/anime/api/AnimeExtensionApi.kt`, goes through `AnimeExtensionStoreService`, `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt:24`) | `app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt:54` |
| App update check | `GET https://api.github.com/repos/${arguments.repository}/releases/latest` parsed as `GithubRelease` | `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:19-22` |
| App update download | `network.client.newCachelessCallWithProgress(GET(url), progressListener).await()` | `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt:102` |
| Data saver "reSmush.it" | `client.newCall(GET("http://api.resmush.it/ws.php?img=$imageUrl&qlty=$quality")).execute()` (blocking call, plain HTTP) | `app/src/main/java/aniyomi/util/DataSaver.kt:140-174` |
| Torrent server API | `network.client.newCall(GET("$hostUrl/echo")).awaitSuccess()` and POSTs | `core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerApi.kt:38,63,87` |
| WebView screen | reads `network.cookieJar.remove(url)` for "Clear cookies" | `app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt:124-127`, `WebViewScreenModel.kt:59-64` |
| Players | read only `verbose_logging` (mpv `msg-level`, torrent server logging) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:128`, `PlayerActivity.kt:425`, `app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt:66` |
| Coil debug logging | `if (networkPreferences.verboseLogging().get()) logger(DebugLogger())` | `App.kt:206` |

Clients that do NOT use `NetworkHelper` (per `plans/catalogs/network-endpoints-01.md` section 0.1): AniSkip and AniChart construct a bare `OkHttpClient()` (no cookie jar, no DoH, no cache, no UA interceptor, OkHttp default 10 s timeouts).

### 1.3 Proposal: Rust construction order

`NetworkHelper::new(settings: &NetworkSettings, platform: &Arc<dyn Platform>, webview: Option<Arc<dyn WebViewHost>>) -> NetworkHelper` builds in this order (details and types in `p1-rust-design.md`):

1. `cookie_jar = Arc<CookieJar>::load(paths.data().join("cookies.json"))`.
2. Base transport: `reqwest::Client` with `connect_timeout(30 s)`, `read_timeout(30 s)`, `timeout(2 min)` (whole call including body), `redirect::Policy::limited(20)`, `pool_idle_timeout(5 min)`, `pool_max_idle_per_host(5)`, cookie provider = the jar, `dns_resolver` = DoH resolver when `doh_provider` is 1..13, HTTP/2 enabled.
3. Interceptors, in order: `Uncaught`, `UserAgent(provider)`, `Cloudflare(jar, provider, webview)`.
4. If `verbose_logging`: `NetworkLogger` runs inside the terminal stage (the place where OkHttp puts network interceptors).
5. HTTP cache (P2) sits in the terminal stage in front of the transport, like OkHttp's `CacheInterceptor`.
6. `NetworkHelper.client` is cheap to clone (`Arc` inside). `client.new_builder()` returns a `ClientBuilder` that copies the interceptor list and lets callers append, mirroring `newBuilder().addInterceptor(...)`.

DoH and `verbose_logging` are applied at construction; the settings UI shows `requires_app_restart` exactly as Aniyomi does (a rebuild-on-change design is possible but changes semantics and is not planned).

---------------------------------------------------------------------------------------------------

## 2. Preferences read by this area (`N/NetworkPreferences.kt`)

| Accessor | Key | Type | Default | Notes |
|---|---|---|---|---|
| `verboseLogging()` | `verbose_logging` | Boolean | constructor argument `verboseLogging`, supplied as `isDebugBuildType` by `app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt:36-40` (true only in debug builds); class default `false` | Also drives mpv `msg-level` (`all=v` vs `all=warn`), `PlayerActivity` log level (`info` vs `warn`), torrent server logging and Coil `DebugLogger` |
| `dohProvider()` | `doh_provider` | Int | `-1` | `-1` = off; 1..13 provider ids (section 6) |
| `defaultUserAgent()` | `default_user_agent` | String | `"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"` | Trimmed on every read |

Legacy: migration `mihon.core.migration.migrations.DOHMigration` (version 57f, `app/src/main/java/mihon/core/migration/migrations/DOHMigration.kt`) converted the old boolean `enable_doh` to `doh_provider = PREF_DOH_CLOUDFLARE` and removed `enable_doh`. Owned by `area:11-app-migrations`; Kurobayomi starts without that legacy key, but restoring an Aniyomi backup that carries `enable_doh` must be handled by the backup preference restore (`area:22-backup-restore`).

---------------------------------------------------------------------------------------------------

## 3. Request helpers (`N/Requests.kt`)

### 3.1 OBSERVED

* Private defaults (lines 16-18): `DEFAULT_CACHE_CONTROL = CacheControl.Builder().maxAge(10, MINUTES).build()` (serialises to `max-age=600`), `DEFAULT_HEADERS = Headers.Builder().build()` (empty), `DEFAULT_BODY: RequestBody = FormBody.Builder().build()` (empty form: `Content-Type: application/x-www-form-urlencoded`, `Content-Length: 0`).
* `GET(url: String, headers, cache)` calls `url.toHttpUrl()` (throws `IllegalArgumentException` for an invalid URL; the helper trims leading and trailing ASCII whitespace and requires `http` or `https`), then `GET(url: HttpUrl, ...)` (since extensions-lib 1.4): `Request.Builder().url(url).headers(headers).cacheControl(cache).build()`.
* `POST/PUT/DELETE(url: String, headers, body = DEFAULT_BODY, cache)`: `Request.Builder().url(url)` (OkHttp converts an invalid string to `IllegalArgumentException`, and also accepts `ws:` and `wss:` by rewriting them to `http` and `https`) `.post(body)|.put(body)|.delete(body).headers(headers).cacheControl(cache).build()`. Note the order: `.headers(headers)` REPLACES all headers set before it; `.cacheControl(cache)` then sets (or removes, if `cache.toString()` is empty) the `Cache-Control` header, overriding any `Cache-Control` the caller put in `headers`.
* `suspend fun OkHttpClient.get(url: String | HttpUrl, headers, cache): Response = newCall(GET(...)).awaitSuccess()` (extensions-lib 16) and `suspend fun OkHttpClient.post(url: String, headers, body, cache) = newCall(POST(...)).awaitSuccess()`. There are no `put` or `delete` client extensions.
* `CacheControl` presets used by callers: `CacheControl.FORCE_NETWORK` (header `no-cache`; used by Bangumi, `app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt:191,217`), `CacheControl.FORCE_CACHE` (`only-if-cached, max-stale=2147483647`).

### 3.2 Effect of `max-age=600` on the request (OkHttp cache semantics)

A request `Cache-Control: max-age=600` can only SHORTEN what the server allows: OkHttp computes `freshMillis = min(serverFreshness, 600 s)` (`okhttp/src/commonJvmAndroid/kotlin/okhttp3/internal/cache/CacheStrategy.kt:171-172`). Server freshness comes from `max-age`, then `Expires`, then the heuristic of `CacheStrategy.kt:250-256`: when there is no query string and `Last-Modified` is present, freshness is 10% of the document age at service time. A stored response with no such information has freshness 0 and is revalidated (conditional `If-None-Match` / `If-Modified-Since` when validators exist) or refetched. Only GET and HEAD responses are stored. See `p1-topic-http-cache.md`.

### 3.3 Proposal

`kuro-net::request` mirrors the helpers one-to-one: `get(url, headers, cache)`, `post`, `put`, `delete`, and client methods `HttpClient::get/post` returning a successful response or `NetError::Http`. URL strings that fail to parse return `NetError::InvalidUrl(String)` (message = the parser error). `Cache-Control` precedence is kept: the `cache` argument wins over a `Cache-Control` header in `headers`.

---------------------------------------------------------------------------------------------------

## 4. Interceptors

### 4.1 `UncaughtExceptionInterceptor` (`N/interceptor/UncaughtExceptionInterceptor.kt`)

OBSERVED: `try { chain.proceed(chain.request()) } catch (e: Exception) { if (e is IOException) throw e else throw IOException(e) }`. Subclasses of `java.lang.Error` (for example `OutOfMemoryError`, `StackOverflowError`) are not caught. Purpose (KDoc): OkHttp's async `enqueue` only reports `IOException`s; anything else would crash the process. It is the FIRST application interceptor. Resulting message: `IOException(cause)` has message `cause.toString()` (for example `java.lang.IllegalStateException: boom`), shown to the user by `ExceptionFormatter` as the message of an `IOException` (section 9).

Proposal: Rust has no checked exception split; the equivalent job is (a) convert any panic inside an interceptor future into `NetError::Other(msg)` using `futures::FutureExt::catch_unwind` on the whole chain future, and (b) wrap non-`NetError` failures from interceptors defined outside `kuro-net` (extension plugins) into `NetError::Other`. Cancellation is never converted.

### 4.2 `UserAgentInterceptor` (`N/interceptor/UserAgentInterceptor.kt`)

OBSERVED: `if (originalRequest.header("User-Agent").isNullOrEmpty())` build a request with `.removeHeader("User-Agent").addHeader("User-Agent", defaultUserAgentProvider())`; otherwise proceed unchanged. `Request.header(name)` returns the LAST value of the header (OkHttp `Headers.get` returns the last value); a request carrying two `User-Agent` values where the last is non-empty is left untouched. The provider is evaluated per request. A blank provider result (the preference trimmed to `""`) would add an empty `User-Agent`; the settings dialog prevents blank values (section 8.1), so this only happens through a hand-edited preference file.

Proposal: identical rule using `HeaderMap::get_all(USER_AGENT).iter().last()`: if absent or empty, remove all `User-Agent` headers and insert the provider value. If the provider value is empty, do not insert a header (hyper would otherwise send an empty one) and log a warning once.

### 4.3 Where else the default UA is applied (OBSERVED)

* `HttpSource.headersBuilder()` and `AnimeHttpSource.headersBuilder()`: `add("User-Agent", network.defaultUserAgentProvider())`. `HttpSource.headers` is `by lazy`, so each source instance caches the UA it saw at first use; changing the preference only updates sources created afterwards or the next process. This is why the UA edit dialog says "Requires app restart".
* `WebViewInterceptor.createWebView`: `settings.userAgentString = request.header("User-Agent") ?: defaultUserAgentProvider()` so the challenge page and later requests present the same UA, which Cloudflare requires for `cf_clearance` validity.
* The WebView screen builds headers from `source.headers.toMultimap().mapValues { it.value.getOrNull(0) ?: "" }` (`WebViewActivity.kt:64-77`, `WebViewScreenModel.kt:31-44`), which includes the UA.

### 4.4 `RateLimitInterceptor` and `SpecificHostRateLimitInterceptor`

Full algorithm and the Rust version are in `p1-topic-rate-limiter.md`. Summary of OBSERVED behaviour (`N/interceptor/RateLimitInterceptor.kt`):

* Public API for extensions: `OkHttpClient.Builder.rateLimit(permits: Int, period: Duration = 1.seconds)` (extension-lib 1.5; deprecated overload with `period: Long = 1, unit: TimeUnit = SECONDS` from 1.3), `rateLimitHost(httpUrl: HttpUrl, permits, period: Duration = 1.seconds)`, `rateLimitHost(url: String, permits, period: Duration = 1.seconds)`, deprecated `rateLimitHost(httpUrl, permits, period: Long = 1, unit = SECONDS)`. Each adds one `RateLimitInterceptor(host?, permits, period)` as an application interceptor. KDoc examples: `permits = 5, period = 1.seconds` is 5 requests per second; `permits = 10, period = 2.minutes` is 10 per 2 minutes.
* State per interceptor instance: `requestQueue: ArrayDeque<Long>(permits)` of start timestamps (`SystemClock.elapsedRealtime()` milliseconds), `rateLimitMillis = period.inWholeMilliseconds`, `fairLock = Semaphore(1, true)` (fair, single permit).
* `host`: `null` or equal to `request.url.host` means "limit this request"; any other host is passed through with no accounting. `rateLimitHost(url: String)` uses `url.toHttpUrlOrNull()?.host`; an unparsable string yields `host = null`, which silently limits EVERY host (pitfall, reproduced for parity but logged as a warning in Rust).
* Cancelled call: `IOException("Canceled")` at entry and on every loop iteration after the expiry scan.
* Cached response: if `response.networkResponse == null` the entry for this request is removed from the queue and all waiters are notified, so cache hits cost no permit.

### 4.5 `CloudflareInterceptor` and `WebViewInterceptor`

Step-by-step with the state machine, bug analysis and the Rust design is in `p1-topic-cloudflare-and-webview.md`. Summary of OBSERVED behaviour:

1. `WebViewInterceptor.intercept(chain)`: `response = chain.proceed(request)`; if `!shouldIntercept(response)` return it. `shouldIntercept` (Cloudflare): `response.code in listOf(403, 503) && response.header("Server") in arrayOf("cloudflare-nginx", "cloudflare")` (exact, case-sensitive value match; header name lookup is case-insensitive).
2. `!WebViewUtil.supportsWebView(context)`: show toast `MR.strings.information_webview_required` ("WebView is required for the app to function") with `Toast.LENGTH_LONG` on the UI thread and RETURN the original 403/503 response (no exception, body intact).
3. Lazy one-time `initWebView`: skipped on MIUI and on Samsung with SDK 31; else `WebSettings.getDefaultUserAgent(context)` with all exceptions swallowed.
4. `CloudflareInterceptor.intercept(chain, request, response)`: `response.close()`; `cookieManager.remove(request.url, listOf("cf_clearance"), maxAge = 0)`; `oldCookie = cookieManager.get(request.url).firstOrNull { it.name == "cf_clearance" }`; `resolveWithWebView(request, oldCookie)`; `return chain.proceed(request)` (one retry, no further Cloudflare check on the retry result).
5. `resolveWithWebView`: main-thread WebView, `loadUrl(origRequestUrl, parseHeaders(originalRequest.headers))`, wait on a `CountDownLatch(1)` for at most 30 s (`awaitFor30Seconds` ignores the boolean result), then on the main executor `webview.stopLoading(); webview.destroy()`. Success flag `cloudflareBypassed` set only in `onPageFinished` when a `cf_clearance` cookie exists whose value differs from `oldCookie`.
6. Failure (`!cloudflareBypassed`): if `webview.isOutdated()` (major Chrome version in the WebView UA < 118) show toast `information_webview_outdated` ("Please update the WebView app for better compatibility", LONG); throw `CloudflareBypassException`.
7. `intercept` maps `CloudflareBypassException` to `IOException(context.stringResource(MR.strings.information_cloudflare_bypass_failure), e)` (message "Failed to bypass Cloudflare") and any other `Exception` to `IOException(e)`.
8. `parseHeaders(headers)`: drops unsafe headers (section 4.6), keeps the first value per header name, returns `Map<String, String>`.

### 4.6 Header safety filter (`N/interceptor/WebViewInterceptor.kt:94-113`)

`isRequestHeaderSafe(name, value)`: lower-case both with `Locale.ENGLISH`; unsafe if `name in unsafeHeaderNames` or `name.startsWith("proxy-")`; unsafe if `name == "connection" && value == "upgrade"`; otherwise safe. `unsafeHeaderNames = ["content-length", "host", "trailer", "te", "upgrade", "cookie2", "keep-alive", "transfer-encoding", "set-cookie"]`. Comment: keeping an unsafe header makes WebView fail with `net::ERR_INVALID_ARGUMENT`. Note `cookie` itself is NOT filtered, `connection` with any other value passes, `expect` and `origin` pass.

---------------------------------------------------------------------------------------------------

## 5. Cookies (`N/AndroidCookieJar.kt`)

### 5.1 OBSERVED

* Backing store: `android.webkit.CookieManager.getInstance()`, the same store Chromium WebView uses, so cookies from visited pages (including `cf_clearance` from the challenge) are visible to OkHttp without any copy step.
* `saveFromResponse(url, cookies)`: for each OkHttp `Cookie`, `manager.setCookie(url.toString(), cookie.toString())`; `Cookie.toString()` is the full Set-Cookie form (`name=value; expires=...` or `max-age=0`, `domain=...` unless host-only, `path=...`, `secure`, `httponly`, `samesite=...`).
* `loadForRequest(url)` = `get(url)`: `manager.getCookie(url.toString())` returns `"a=1; b=2"` (names and values only) or null; if non-empty: `split(";").mapNotNull { Cookie.parse(url, it) }` (OkHttp trims the pair, so the space after `;` is harmless; entries without a valid name are dropped); else empty list. OkHttp's bridge sends all returned cookies joined as `name=value; name=value` without re-checking domain or path.
* `remove(url, cookieNames: List<String>? = null, maxAge: Int = -1): Int`: `cookies = manager.getCookie(url.toString()) ?: return 0`; split on `;`, take `substringBefore("=")` (NOT trimmed: for the second and later cookies the name has a leading space, so a `cookieNames` filter such as `["cf_clearance"]` only matches when `cf_clearance` is the first cookie in the string; the unfiltered form works for all because `setCookie` tolerates the space), keep names in `cookieNames` if given, and for each call `manager.setCookie(urlString, "$it=;Max-Age=$maxAge")`. Returns the count of names processed. `maxAge = -1` (default) writes `Max-Age=-1` which expires the cookie; `maxAge = 0` (the Cloudflare call) expires it too. The replacement cookie has no `Domain` attribute, so it is host-only on the request host, with the default path of the URL; a Domain-scoped cookie (`Domain=.example.org`) is a different cookie key in Chromium and is therefore NOT removed by this call.
* `removeAll()`: `manager.removeAllCookies {}` (asynchronous, callback ignored).
* Used by: Settings > Advanced > Networking "Clear cookies" (`SettingsAdvancedScreen.kt:231-236`), WebView screen "Clear cookies" (`remove(url)` with defaults, logging `Cleared $cleared cookies for: $url`), Cloudflare interceptor (`remove` with names and `maxAge = 0`, `get`).

### 5.2 Proposal

* Own cookie store with RFC 6265 matching: `cookie_store::CookieStore` inside `reqwest_cookie_store::CookieStoreMutex`, used as reqwest's `cookie_provider`, so cookies are set from redirects and sent per hop.
* Persistence: JSON file `<data dir>/cookies.json` written through `cookie_store::serde::json::save` (persistent, unexpired cookies only; session cookies are memory-only, the usual browser rule), debounced 2 s after the last change, atomic write (temp file + rename), mode 0600. Load at start; on a parse error rename the file to `cookies.json.corrupt` and start empty (log a warning).
* `get(url) -> Vec<Cookie>`: all unexpired cookies that match the URL (domain, path, secure, host-only).
* `remove(url, names: Option<&[&str]>, _max_age) -> usize`: actually deletes every matching cookie (domain-scoped ones included; fixes the Kotlin limitation above) whose name is in `names` (exact match after trimming), returns the number deleted.
* `remove_all()`: clears memory and the file (write an empty store immediately, no debounce).
* WebKitGTK cookie sync is an explicit import and export; see `p1-topic-cloudflare-and-webview.md` section 5.

---------------------------------------------------------------------------------------------------

## 6. DNS over HTTPS (`N/DohProviders.kt`)

Provider constants, URLs and bootstrap addresses are in `p1-data-and-config.md` section 3; the resolver algorithm is in `p1-topic-doh.md`. Behaviour summary (OBSERVED):

* Selection happens once at construction (`NetworkHelper.kt:42-57`); unknown values (including 0, 14, -1) leave the system resolver in place.
* Each provider function builds `DnsOverHttps` on a copy of the current builder (section 1.1 step 2.9) with the provider URL and `bootstrapDnsHosts(...)` so the DoH host itself is reached by IP without DNS.
* The Mullvad URL literal contains a leading space (`" https://dns.mullvad.net/dns-query"`); OkHttp's URL parser strips leading ASCII whitespace, so it works.
* Settings label list (`SettingsAdvancedScreen.kt:262-276`): `-1` -> `MR.strings.disabled`, then the literal strings "Cloudflare", "Google", "AdGuard", "Quad9", "AliDNS", "DNSPod", "360", "Quad 101", "Mullvad", "Control D", "Njalla", "Shecan", "LibreDNS". After a change the screen shows `MR.strings.requires_app_restart`.
* Trackers Komga and Jellyfin opt out with `Dns.SYSTEM`; AniSkip/AniChart clients never had DoH.

---------------------------------------------------------------------------------------------------

## 7. Response handling (`N/OkHttpExtensions.kt`, `N/HttpException.kt`, `N/ProgressResponseBody.kt`)

### 7.1 `await`, `awaitSuccess`, `HttpException`

OBSERVED:

* `Call.await()` captures `Exception().stackTrace` minus the first frame as `callStack`, then `suspendCancellableCoroutine`: `invokeOnCancellation { call.cancel() }` (exceptions ignored); `enqueue(Callback)`. `onFailure(call, e)`: if the continuation is already cancelled do nothing; else resume with `IOException(e.message, e)` whose stack trace is replaced by `callStack` (so the trace points at the caller, not at the OkHttp dispatcher thread). `onResponse`: `continuation.resume(response) { _, value, _ -> value.close() }` (a response delivered after cancellation is closed).
* `Call.awaitSuccess()`: `response = await(callStack)`; if `!response.isSuccessful` (status outside 200..299): `response.close()` and `throw HttpException(response.code).apply { stackTrace = callStack }`; else return the response (caller must close the body).
* `HttpException(val code: Int) : IllegalStateException("HTTP error $code")` (since extensions-lib 17). Redirects have been followed before this check, so 3xx never reaches it except `304` for hand-conditional requests.
* Special cases in callers: `MangaScreenModel.fetchMangaFromSource` ignores `HttpException` with `code == 103` ("early hints errors that aren't handled by OkHttp", `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:283-285`); `BangumiApi` treats `404` as "subject is not collected by user" (returns null).
* Deprecated RxJava twins `asObservable()` / `asObservableSuccess()` (clone the call per subscriber; request arbiter implements `Producer` and `Subscription`; `onNext` + `onCompleted`; unsubscribe cancels): used by deprecated source methods; dropped (T-01a-038).

### 7.2 `parseAs` (`OkHttpExtensions.kt:127-145`)

`context(json: Json) inline fun <reified T> Response.parseAs(): T = decodeFromJsonResponse(serializer(), this)`; `decodeFromJsonResponse` = `response.body.source().use { json.decodeFromBufferedSource(deserializer, it) }`: streams from the body and always closes it. The app `Json` (from `AppModule`, per `plans/catalogs/network-endpoints-01.md` 0.1) is `ignoreUnknownKeys = true; explicitNulls = false`; no `coerceInputValues`, no `isLenient`. `val jsonMime = "application/json; charset=utf-8".toMediaType()` is the media type for JSON request bodies.

### 7.3 Progress call (`OkHttpExtensions.kt:99-110`, `ProgressListener.kt`, `ProgressResponseBody.kt`)

* `OkHttpClient.newCachelessCallWithProgress(request, listener)`: `newBuilder().cache(null).callTimeout(30, HOURS).addNetworkInterceptor { chain -> proceed(chain.request()); response.newBuilder().body(ProgressResponseBody(response.body, listener)).build() }.build().newCall(request)`. All other settings (30 s connect/read timeouts, cookie jar, application interceptors including Cloudflare) are inherited. Because the wrapper is a NETWORK interceptor it sees the raw (still gzip-compressed) bytes, and `contentLength()` is the `Content-Length` of those same bytes.
* `ProgressResponseBody.contentType()` and `contentLength()` delegate; `source()` returns a buffered `ForwardingSource` whose `read(sink, byteCount)` calls the delegate, adds `bytesRead` (when not -1) to `totalBytesRead`, and calls `progressListener.update(totalBytesRead, responseBody.contentLength(), bytesRead == -1L)`. So `update` fires on every read including the final end-of-stream read (`done = true`, `bytesRead` repeating the previous total). `contentLength` is `-1` for chunked or compressed-without-length responses.
* Listener implementations: `Page.update` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt:36-42`): `progress = if (contentLength > 0) (100 * bytesRead / contentLength).toInt() else -1`; `AppUpdateDownloadJob` (`.../AppUpdateDownloadJob.kt:82-97`): `progress = (100 * (bytesRead.toFloat() / contentLength)).toInt()`, notifies only when `progress > savedProgress && now - 200 > lastTick` (so at most every 200 ms and only upward); with `contentLength = -1` the progress is negative and no notification is posted; `AnimeDownload` and `AnimeDownloadPart` also implement/store a `ProgressListener` (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt:22`, `.../AnimeDownloadPart.kt:59`; owned by `area:24-downloads-anime`).
* App update error handling (`AppUpdateDownloadJob.kt:104-125`): unsuccessful response -> `Exception("Unsuccessful response")`; `CancellationException` or `StreamResetException` with `ErrorCode.CANCEL` -> cancel the notification; any other exception -> `notifier.onDownloadError(url)`.

---------------------------------------------------------------------------------------------------

## 8. Settings that surface this area

### 8.1 Settings > Advanced > Networking group (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:220-307`, owner of the screen is `area:47-settings-screens-general`; behaviour here)

Group title `MR.strings.label_network` ("Networking"). Rows in order:

1. "Clear cookies" (`MR.strings.pref_clear_cookies`): `networkHelper.cookieJar.removeAll()` then toast `MR.strings.cookies_cleared` ("Cookies cleared"). No confirmation dialog.
2. "Clear WebView data" (`MR.strings.pref_clear_webview_data`): in a `try`: create a `WebView`, `setDefaultSettings()`, `clearCache(true)`, `clearFormData()`, `clearHistory()`, `clearSslPreferences()`; `WebStorage.getInstance().deleteAllData()`; recursively delete `"${applicationInfo.dataDir}/app_webview/"` (the whole Chromium profile, which also contains the cookie database); toast `MR.strings.webview_data_deleted` ("WebView data cleared"). On any `Throwable`: log at ERROR and toast `MR.strings.cache_delete_error` ("Error occurred while clearing").
3. "DNS over HTTPS (DoH)" list (`MR.strings.pref_dns_over_https`) bound to `doh_provider`; on change toast `requires_app_restart`; always accepts.
4. "Default user agent string" (`MR.strings.pref_user_agent_string`) edit-text bound to `default_user_agent`; subtitle is the value (`"%s"`). Dialog rules from `EditTextPreferenceWidget`: OK is enabled only when the text differs from the stored value and is not blank (`canBeBlank` default false); the trailing icon is an error icon when blank, otherwise a clear button. `onValueChanged`: attempts `Headers.Builder().add("User-Agent", it)`; an `IllegalArgumentException` (OkHttp rejects any character outside `\t` and 0x20..0x7E, including all non-ASCII) shows toast `MR.strings.error_user_agent_string_invalid` ("Invalid user agent string") and rejects the value; success shows `requires_app_restart`. The string `error_user_agent_string_blank` ("User agent string can not be blank") exists in resources but no code references it.
5. "Reset default user agent string" (`MR.strings.pref_reset_user_agent_string`): enabled only while `userAgent != userAgentPref.defaultValue()`; click calls `userAgentPref.delete()` then toast `requires_app_restart`.

### 8.2 Other surfaces

* Advanced > "Verbose logging" switch (`MR.strings.pref_verbose_logging`, summary `pref_verbose_logging_summary`: "Print verbose logs to system log (reduces app performance)"): toast `requires_app_restart` on change (`SettingsAdvancedScreen.kt:111-119`).
* Debug info screen shows `WebViewUtil.getVersion(context)` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt:73`) and crash logs include `WebView: ${WebViewUtil.getVersion(context)}` (`app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt:54`).
* WebView screen opens with `WebViewUtil.supportsWebView` check: if false, toast `information_webview_required` (LONG) and `finish()` (`WebViewActivity.kt:55-59`).

---------------------------------------------------------------------------------------------------

## 9. User-visible failure messages produced from this area's exceptions

`app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:16-39` (owner `area:10-app-domain-layer`) formats `Throwable.formattedMessage`:

| Throwable | Message key and text |
|---|---|
| `HttpException` | `MR.strings.exception_http` = "HTTP %d, check website in WebView" |
| `UnknownHostException`, device offline (`Context.isOnline()` false) | `MR.strings.exception_offline` = "No Internet connection" |
| `UnknownHostException`, online | `MR.strings.exception_unknown_host` = "Couldn't reach %s" with the exception message (the host) |
| Cloudflare failure (`IOException`, class name `IOException`) | message passthrough: `MR.strings.information_cloudflare_bypass_failure` = "Failed to bypass Cloudflare" |
| Other exceptions | `"Exception"`/`"IOException"` -> `message ?: className`; else `"$className: $message"` |

Also `MR.strings.information_cloudflare_help` = "Tap here for help with Cloudflare" exists (used by the browse error UI, `area:40-ui-browse-manga`). Proposal: `kuro-net::error::NetError::user_message(online: bool) -> Message` returns a key plus arguments so the UI layer localises; see `p1-rust-design.md`.

---------------------------------------------------------------------------------------------------

## 10. `JavaScriptEngine` (`N/JavaScriptEngine.kt`)

OBSERVED: `suspend fun <T> evaluate(script: String): T = withIOContext { QuickJs.create().use { it.evaluate(script) as T } }`. A new interpreter per call, closed afterwards (the doc: "result as a primitive type (e.g., String, Int)", since extensions-lib 1.4). The cast is unchecked: a result of another type throws `ClassCastException` at the use site. QuickJS-android 0.9.2 limits are the library defaults (no memory limit, default GC threshold and stack size; no interrupt handler, so an infinite loop blocks the IO thread forever). Script errors raise `QuickJsException`. Registered as singleton `JavaScriptEngine(app)` in `AppModule.kt:201`. The `Context` parameter is unused.

Proposal: `rquickjs` runtime per call inside `tokio::task::spawn_blocking`; memory limit 64 MiB, stack limit 512 KiB, interrupt handler that aborts after 30 s of wall time (these three limits are new safety limits, documented as divergences); result mapped to `JsValue::{Null, Bool, Int(i32), Double(f64), Str(String)}`; any other JS type returns `JsError::UnsupportedResult(typeof)` instead of a cast failure.

---------------------------------------------------------------------------------------------------

## 11. `util/system` helpers

### 11.1 `ToastExtensions.kt`

`Context.toast(resource: StringResource, duration = Toast.LENGTH_SHORT, block)` resolves the string and delegates; `Context.toast(text: String?, duration, block)` calls `Toast.makeText(applicationContext, text.orEmpty(), duration)`, applies `block` to the toast, then `show()`; returns the `Toast`. A null text shows an EMPTY toast. Android durations: `LENGTH_SHORT` = 2000 ms, `LENGTH_LONG` = 3500 ms (platform constants). Callers from background threads must hop to the UI thread themselves (`launchUI { context.toast(...) }` in `WebViewInterceptor`).

Proposal: `Toaster` service (section 4 of `p1-rust-design.md`), `Short = 2000 ms`, `Long = 3500 ms`, FIFO queue (Android also serialises toasts), skip empty messages, max 5 queued (new ones beyond the cap are dropped). Safe to call from any thread.

### 11.2 `WebViewUtil.kt`

* `MINIMUM_WEBVIEW_VERSION = 118`; `WebView.isOutdated() = getWebViewMajorVersion() < 118`; major version = group 1 of the regex `.*Chrome/(\d+)\..*` matched against the WebView's DEFAULT user agent (obtained by setting `settings.userAgentString = null`, reading, and restoring); 0 when no match (so a WebView with an unknown UA counts as outdated).
* `getVersion(context)`: `WebView.getCurrentWebViewPackage()`; null -> the literal `"how did you get here?"`; else `"<app label> <versionName>"`.
* `supportsWebView(context)`: `CookieManager.getInstance()` inside try/catch Throwable (log ERROR, return false) AND `packageManager.hasSystemFeature(FEATURE_WEBVIEW)`.
* `spoofedPackageName`: `com.android.chrome` if installed else `com.android.settings`; used by `App.getPackageName()` to replace `X-Requested-With` on WebView requests (`App.kt:225-237`). Dropped (T-01a-026).
* `getInferredUserAgent(context)`: builds a Chrome-like UA from the WebView UA with `"; Android .*?\\)"` -> `"; Android 10; K)"` and `"Version/.* Chrome/"` -> `"Chrome/"`. No callers in the repository. Dropped.
* `WebView.getHtml()`: `evaluateJavascript("document.documentElement.outerHTML")`, resumes with the JSON-encoded string (quoted and escaped); consumed by the WebView screen (`app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt:80`, `area:36-ui-shell-navigation-security-webview`).
* `WebView.setDefaultSettings()`: `javaScriptEnabled = true`, `domStorageEnabled = true`, `useWideViewPort = true`, `loadWithOverviewMode = true`, `cacheMode = LOAD_DEFAULT`, `setSupportZoom(true)`, `builtInZoomControls = true`, `displayZoomControls = false`, and `CookieManager.acceptThirdPartyCookies(this)`.

### 11.3 `DeviceUtil.kt`

* `isMiui`: system property `ro.miui.ui.version.name` non-empty; `miuiMajorVersion`: `Build.VERSION.INCREMENTAL.substringBefore('.').trimStart('V').toIntOrNull()` (for the KDoc input `V12.5.3.0.QFGMIXM` this yields 12; the KDoc's "e.g., 13" is just an example of the return type); `isMiuiOptimizationDisabled()`: property `persist.sys.miui_optimization` equal to `"0"` or `"false"`, else reflective `android.miui.AppOpsUtils.isXOptMode()`; `isSamsung`: `Build.MANUFACTURER` equals `samsung` ignoring case; `oneUiVersion`: `SEM_PLATFORM_INT - 90000`, `< 0` -> 1.0 else `(v/10000).toString() + "." + v % 10000 / 100` as Double; `invalidDefaultBrowsers = ["android", "com.hihonor.android.internal.app", "com.huawei.android.internal.app", "com.zui.resolver", "com.transsion.resolver", "com.android.intentresolver"]`; `isLowRamDevice(context)`: `memInfo.totalMem < 3L * 1024 * 1024 * 1024`.
* Consumers: MIUI file-picker warnings in backup screens (`SettingsDataScreen.kt:241`, `CreateBackupScreen.kt:84`, `RestoreBackupScreen.kt:72`), debug info (One UI / MIUI version rows), `MigrateSecureScreenMigration`, `ContextExtensions.kt:101` (browser chooser), Coil `allowRgb565(DeviceUtil.isLowRamDevice(this))` (`App.kt:204`).
* Decision: none of the vendor probes exist on Linux; the low-RAM rule is implemented by T-01a-030 with the same 3 GiB threshold.

### 11.4 `GLUtil.kt`

* `DEVICE_TEXTURE_LIMIT` (lazy): EGL10 default display; `eglInitialize`; enumerate all configs; maximum of `EGL_MAX_PBUFFER_WIDTH` over configs; `eglTerminate`; result `max(maximum, SAFE_TEXTURE_LIMIT)`. `SAFE_TEXTURE_LIMIT = 2048`.
* `CUSTOM_TEXTURE_LIMIT_OPTIONS` (lazy): `steps = DEVICE_TEXTURE_LIMIT / 1024` (integer division); list = `[DEVICE_TEXTURE_LIMIT]` followed by, for `step` from `steps` down to 2: `value = step * 1024`, skipped when `value >= DEVICE_TEXTURE_LIMIT`, else appended. Examples: 16384 -> 15 values `16384, 15360, ..., 2048`; 8192 -> 7; 4096 -> `4096, 3072, 2048`; 3000 -> `3000, 2048`; 2048 -> `2048`.
* Consumers: `BasePreferences.hardwareBitmapThreshold` default `GLUtil.SAFE_TEXTURE_LIMIT` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:40`), `App.kt:142` stores `DEVICE_TEXTURE_LIMIT` when the preference is unset, Advanced screen option list and enabled flag `DEVICE_TEXTURE_LIMIT > SAFE_TEXTURE_LIMIT` (`SettingsAdvancedScreen.kt:367,383`), `ImageUtil.kt:328`.
* Decision: implemented by T-01a-022 (GL probe through `gdk::GLContext`). Note `area:02-preferences-system` marks `pref_hardware_bitmap_threshold` as DROP in its registry (T-02a task list), which only hides the setting; the probe and algorithm stay available to the reader image pipeline (`area:43-reader-core`).

### 11.5 `DensityExtensions.kt`

`Int.dpToPx = (this * Resources.getSystem().displayMetrics.density).toInt()` (truncating). Used by pager/webtoon holders, `MainActivity.kt:439`, widgets. Dropped: GTK layout is in logical pixels.

---------------------------------------------------------------------------------------------------

## 12. Concurrency and threading (OBSERVED)

* `await()` completes on OkHttp's dispatcher thread; coroutine callers resume on their dispatcher. `awaitSuccess` and `parseAs` are called inside `withIOContext` by most callers.
* Interceptors run on the dispatcher thread (or the caller thread for `execute()`); the Cloudflare interceptor BLOCKS that thread up to 30 s on a `CountDownLatch`; WebView calls are marshalled to the main thread with `ContextCompat.getMainExecutor(context).execute`.
* `Dispatcher`: at most 64 async calls overall and 5 per host run at once; extra calls wait in the dispatcher queue (no timeout started yet). This also bounds how many chapter-page requests hit one source host concurrently.
* The rate limiter's `fairLock` serialises waiters in FIFO order; only the head waiter sleeps inside the monitor.
* Rust mapping: tokio multi-thread runtime `kuro-rt` (T-01a-009); no thread is blocked for Cloudflare, the solve is an awaited future that talks to the GTK main loop through a channel (`p1-rust-design.md` section 5).

---------------------------------------------------------------------------------------------------

## 13. Interactions with other areas

* `area:02-preferences-system`, `area:01-foundation-app-bootstrap`: `Settings.network`, `AppContext::network()`, warm-up (`T-01a-018`).
* `area:11-app-migrations`: `DOHMigration` (`enable_doh`).
* `area:15-source-api-manga`, `area:16-source-api-anime`, `area:17-source-app-layer`: sources hold `network.client`; the `HttpSource` surface (headers, `client`, `getImage`).
* `area:18-extensions-manga`, `area:19-extensions-anime`, `area:20-extension-strategy-redesign`: the network API surface exposed to extensions (T-12a-029), `JavaScriptEngine`.
* `area:27-tracking-core` through `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin`: auth clients, `Dns.SYSTEM`, `rateLimit(85, 1.minutes)` for AniList.
* `area:34-images-covers-cache`: Coil fetchers use the shared client.
* `area:36-ui-shell-navigation-security-webview`: WebView screen, cookie clear, `getHtml`.
* `area:47-settings-screens-general`: hosts the Networking group.
* `area:51-i18n`: message keys listed above.
* `area:52-util-app`: `isOnline`, `ExceptionFormatter` callers.
