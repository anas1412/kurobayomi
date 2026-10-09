# Area 12-network-stack, part 1: data models and configuration

OBSERVED unless prefixed "Proposal:". Paths are repo-relative to `/home/blackbox/aniyomi-src`. `N/` = `core/common/src/main/java/eu/kanade/tachiyomi/network/`.

## 1. Constants (every literal in scope)

| Constant | Value | Where |
|---|---|---|
| connect timeout | 30 seconds | `N/NetworkHelper.kt:22` |
| read timeout | 30 seconds | `N/NetworkHelper.kt:23` |
| call timeout (shared client) | 2 minutes | `N/NetworkHelper.kt:24` |
| call timeout (progress call) | 30 hours | `N/OkHttpExtensions.kt` (`newCachelessCallWithProgress`) |
| HTTP cache directory | `<cacheDir>/network_cache` | `N/NetworkHelper.kt:28` |
| HTTP cache size | `5L * 1024 * 1024` = 5 242 880 bytes | `N/NetworkHelper.kt:29` |
| default request cache control | `max-age=600` (`maxAge(10, MINUTES)`) | `N/Requests.kt:16` |
| default request headers | empty | `N/Requests.kt:17` |
| default request body | empty `FormBody` | `N/Requests.kt:18` |
| JSON media type | `application/json; charset=utf-8` | `N/OkHttpExtensions.kt:22` |
| Cloudflare status codes | `[403, 503]` | `N/interceptor/CloudflareInterceptor.kt:134` |
| Cloudflare `Server` header values | `["cloudflare-nginx", "cloudflare"]` | `CloudflareInterceptor.kt:135` |
| Cloudflare cookie names | `["cf_clearance"]` | `CloudflareInterceptor.kt:136` |
| Cloudflare wait | 30 seconds (`awaitFor30Seconds`) | `N/interceptor/WebViewInterceptor.kt:81-83` |
| WebView unsafe header names | `content-length, host, trailer, te, upgrade, cookie2, keep-alive, transfer-encoding, set-cookie` plus prefix `proxy-` plus `connection: upgrade` | `WebViewInterceptor.kt:94-113` |
| minimum WebView major version | 118 | `U/WebViewUtil.kt:18` |
| Chrome package / settings package (spoof) | `com.android.chrome` / `com.android.settings` | `U/WebViewUtil.kt:15-16` |
| WebView version fallback text | `how did you get here?` | `U/WebViewUtil.kt:38` |
| rate limiter default period | `1.seconds` (new API) / `1` + `TimeUnit.SECONDS` (deprecated API) | `N/interceptor/RateLimitInterceptor.kt:34-53`, `SpecificHostRateLimitInterceptor.kt` |
| `SAFE_TEXTURE_LIMIT` | 2048 | `U/GLUtil.kt:50` |
| texture option step `MULTIPLIER` | 1024 | `U/GLUtil.kt:65` |
| low-RAM threshold | `3L * 1024 * 1024 * 1024` bytes | `U/DeviceUtil.kt:99` |
| Toast durations (Android platform) | SHORT 2000 ms, LONG 3500 ms | `U/ToastExtensions.kt` defaults |
| progress notification throttle (caller) | 200 ms, only when progress increases | `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt:88-95` |
| OkHttp redirect limit (library default) | 20 | OkHttp 5.4.0 |
| OkHttp dispatcher (library default) | 64 total, 5 per host | OkHttp 5.4.0 |
| OkHttp write timeout (library default) | 10 seconds | OkHttp 5.4.0 |
| OkHttp connection pool (library default) | 5 idle connections, 5 minutes | OkHttp 5.4.0 |
| DoH response size cap (library default) | 65 536 bytes | okhttp-dnsoverhttps 5.4.0 |
| DoH `includeIPv6` (library default) | true (A and AAAA queried) | okhttp-dnsoverhttps 5.4.0 |

## 2. Preferences (`N/NetworkPreferences.kt`)

| Key | Kotlin accessor | Type | Default | Values | Effect | Restart needed |
|---|---|---|---|---|---|---|
| `verbose_logging` | `verboseLogging()` | Boolean | `false` in the class; `true` in debug builds (`isDebugBuildType`) | true / false | HEADERS-level HTTP log, mpv `msg-level all=v`, `PlayerActivity` log level `info`, torrent server logging, Coil debug logger | yes (toast `requires_app_restart`) |
| `doh_provider` | `dohProvider()` | Int | `-1` | `-1` off; `1`..`13` (table 3) | selects DNS resolver | yes |
| `default_user_agent` | `defaultUserAgent()` | String | `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0` | any string accepted as an HTTP header value (visible ASCII 0x20-0x7E and tab) | default `User-Agent`, WebView UA | yes for already-created sources (their `headers` are lazy) |

Legacy key: `enable_doh` (Boolean, removed by `mihon/core/migration/migrations/DOHMigration.kt`, version 57).

Proposal (TOML in `<config>/settings.toml`, owner `area:02-preferences-system`):

```toml
[network]
verbose_logging = false     # true in debug builds
doh_provider = -1
default_user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"
```

## 3. DoH providers (`N/DohProviders.kt`): complete table

Pref id | constant | Settings label | DoH URL (GET `?dns=`) | Bootstrap addresses (in this order) | Source comment in file
---|---|---|---|---|---
1 | `PREF_DOH_CLOUDFLARE` | Cloudflare | `https://cloudflare-dns.com/dns-query` | `162.159.36.1`, `162.159.46.1`, `1.1.1.1`, `1.0.0.1`, `162.159.132.53`, `2606:4700:4700::1111`, `2606:4700:4700::1001`, `2606:4700:4700::0064`, `2606:4700:4700::6400` | none
2 | `PREF_DOH_GOOGLE` | Google | `https://dns.google/dns-query` | `8.8.4.4`, `8.8.8.8`, `2001:4860:4860::8888`, `2001:4860:4860::8844` | none
3 | `PREF_DOH_ADGUARD` | AdGuard | `https://dns-unfiltered.adguard.com/dns-query` | `94.140.14.140`, `94.140.14.141`, `2a10:50c0::1:ff`, `2a10:50c0::2:ff` | "AdGuard 'Default' DNS works too but for the sake of making sure no site is blacklisted, we use 'Unfiltered'"
4 | `PREF_DOH_QUAD9` | Quad9 | `https://dns.quad9.net/dns-query` | `9.9.9.9`, `149.112.112.112`, `2620:fe::fe`, `2620:fe::9` | none
5 | `PREF_DOH_ALIDNS` | AliDNS | `https://dns.alidns.com/dns-query` | `223.5.5.5`, `223.6.6.6`, `2400:3200::1`, `2400:3200:baba::1` | none
6 | `PREF_DOH_DNSPOD` | DNSPod | `https://doh.pub/dns-query` | `1.12.12.12`, `120.53.53.53` | none
7 | `PREF_DOH_360` | 360 | `https://doh.360.cn/dns-query` | `101.226.4.6`, `218.30.118.6`, `123.125.81.6`, `140.207.198.6`, `180.163.249.75`, `101.199.113.208`, `36.99.170.86` | none
8 | `PREF_DOH_QUAD101` | Quad 101 | `https://dns.twnic.tw/dns-query` | `101.101.101.101`, `2001:de4::101`, `2001:de4::102` | none
9 | `PREF_DOH_MULLVAD` | Mullvad | `https://dns.mullvad.net/dns-query` (source literal has a leading space) | `194.242.2.2`, `2a07:e340::2` | "Mullvad DoH, without ad blocking option, Source: https://mullvad.net/en/help/dns-over-https-and-dns-over-tls"
10 | `PREF_DOH_CONTROLD` | Control D | `https://freedns.controld.com/p0` | `76.76.2.0`, `76.76.10.0`, `2606:1a40::`, `2606:1a40:1::` | "Control D, unfiltered option, Source: https://controld.com/free-dns/?"
11 | `PREF_DOH_NJALLA` | Njalla | `https://dns.njal.la/dns-query` | `95.215.19.53`, `2001:67c:2354:2::53` | "Njalla, Non logging and uncensored" (function name in source is `dohNajalla`)
12 | `PREF_DOH_SHECAN` | Shecan | `https://free.shecan.ir/dns-query` | `178.22.122.100`, `185.51.200.2` | "Source: https://shecan.ir/"
13 | `PREF_DOH_LIBREDNS` | LibreDNS | `https://doh.libredns.gr/dns-query` | `116.202.176.26`, `2a01:4f8:1c0c:8274::1` | "Source: https://libredns.gr/"

File header comment: "Based on https://github.com/square/okhttp/blob/ef5d0c83f7bbd3a0c0534e7ca23cbc4ee7550f3b/okhttp-dnsoverhttps/src/test/java/okhttp3/dnsoverhttps/DohProviders.java". Id -1 (disabled) and every other integer: system DNS. Rust enum: `DohProvider { Cloudflare = 1, Google = 2, AdGuard = 3, Quad9 = 4, AliDns = 5, DnsPod = 6, Qihoo360 = 7, Quad101 = 8, Mullvad = 9, ControlD = 10, Njalla = 11, Shecan = 12, LibreDns = 13 }` with `fn from_pref(i32) -> Option<DohProvider>`.

## 4. Data models

### 4.1 OkHttp `Cookie` as seen through `AndroidCookieJar`

Fields of `okhttp3.Cookie`: `name: String`, `value: String`, `expiresAt: Long` (epoch ms; `253402300799999` = `MAX_DATE` when session), `domain: String`, `path: String`, `secure: Boolean`, `httpOnly: Boolean`, `persistent: Boolean`, `hostOnly: Boolean`, `sameSite: String?`. Cookies read back from `CookieManager.getCookie` carry only `name` and `value`; `Cookie.parse(url, "name=value")` fills `domain = url.host`, `hostOnly = true`, `path = default path of url`, `expiresAt = MAX_DATE`, `persistent = false`, `secure = false`, `httpOnly = false`. Equality (`it != oldCookie`) compares all fields, which for two parsed cookies of the same URL reduces to name and value.

Proposal (`kuro-net::cookies`): store type is `cookie_store::Cookie` (name, value, domain, path, secure, http_only, expires, same_site, host_only). Public API:

```rust
pub struct CookieJar { /* Arc<CookieStoreMutex>, path, dirty flag */ }
impl CookieJar {
    pub fn load(path: PathBuf) -> Self;
    pub fn get(&self, url: &Url) -> Vec<Cookie<'static>>;
    pub fn remove(&self, url: &Url, names: Option<&[&str]>) -> usize;
    pub fn remove_all(&self);
    pub fn insert_raw(&self, url: &Url, set_cookie: &str) -> Result<(), CookieError>;
    pub fn export_for(&self, url: &Url) -> Vec<Cookie<'static>>;   // for WebKit import
    pub fn import(&self, url: &Url, cookies: Vec<Cookie<'static>>);// from WebKit export
}
```

### 4.2 Request and headers (`Requests.kt`)

`Request` = method (GET, POST, PUT, DELETE), URL (`HttpUrl`), `Headers` (case-insensitive multimap, insertion order preserved, `add` appends, `set`/`header` replace), body (`RequestBody?`), tags. `CacheControl` fields: `noCache`, `noStore`, `maxAgeSeconds`, `maxStaleSeconds`, `minFreshSeconds`, `onlyIfCached`, `noTransform`, `immutable`; presets `FORCE_NETWORK` (`no-cache`), `FORCE_CACHE` (`only-if-cached, max-stale=2147483647`). Header value validity (`Headers.Builder.add`): each char must be `\t` or in `0x20..=0x7E`; names must be non-empty visible ASCII without `:` or whitespace; violating either throws `IllegalArgumentException`. Leading and trailing whitespace of the value is trimmed.

Proposal:

```rust
pub struct Request { pub method: Method, pub url: Url, pub headers: HeaderMap, pub body: Body, pub cache: CacheControl, pub opts: CallOptions }
pub enum Body { Empty, Form(Vec<(String,String)>), Bytes { content_type: Mime, data: Bytes }, Stream(...) }
pub struct CacheControl { pub no_cache: bool, pub no_store: bool, pub max_age: Option<Duration>, pub max_stale: Option<Duration>, pub min_fresh: Option<Duration>, pub only_if_cached: bool, pub no_transform: bool, pub immutable: bool }
pub struct CallOptions { pub timeout: Option<Duration>, pub use_cache: bool, pub progress: Option<Arc<dyn ProgressListener>>, pub cancel: CancellationToken }
```

### 4.3 Response

OkHttp `Response` fields used: `code`, `isSuccessful` (200..=299), `header(name)`, `headers`, `body`, `networkResponse` (null when served from cache), `cacheResponse`, `protocol`, `request`. Proposal:

```rust
pub struct Response { pub status: StatusCode, pub headers: HeaderMap, pub url: Url /* final */, pub version: Version, pub from_cache: bool, body: ResponseBody }
impl Response { pub fn is_successful(&self) -> bool { (200..300).contains(&self.status.as_u16()) } }
```

### 4.4 Errors

```rust
#[derive(thiserror::Error, Debug)]
pub enum NetError {
    #[error("HTTP error {code}")] Http { code: u16 },                    // HttpException
    #[error("{0}")] InvalidUrl(String),
    #[error("Canceled")] Canceled,
    #[error("timeout")] Timeout,
    #[error("{host}")] UnknownHost { host: String },                      // UnknownHostException
    #[error("{0}")] Io(#[from] std::io::Error),
    #[error("{0}")] Tls(String),
    #[error("Failed to bypass Cloudflare")] CloudflareBypassFailed,       // key information_cloudflare_bypass_failure
    #[error("{0}")] Decode(String),                                        // JSON / charset
    #[error("{0}")] Other(String),
}
```

`Display` of `Http` is exactly `HTTP error <code>` (the Kotlin `IllegalStateException` message). User text goes through `user_message()` (table in `p1-behavior.md` section 9).

### 4.5 Progress

`ProgressListener::update(bytes_read: u64, content_length: i64, done: bool)` with `content_length = -1` when unknown. Call order: after every read of the body including the final zero-length read (`done = true`).

### 4.6 Rate limiter state

`permits: usize`, `period_ms: u64 = period.inWholeMilliseconds`, `queue: VecDeque<Instant>` capacity hint `permits`, `host: Option<String>`, fair gate. See `p1-topic-rate-limiter.md`.

### 4.7 Settings enums (UI)

DoH list entries: `(-1, "Disabled" = MR.strings.disabled)` then the 13 labels of table 3. User-agent edit state: current value, default value, `is_default = value == default`.

## 5. Files and folders on disk

| Item | Aniyomi location | Format | Proposal for Kurobayomi |
|---|---|---|---|
| HTTP response cache | `<cacheDir>/network_cache` | OkHttp `DiskLruCache` journal + entries, 5 MiB LRU | `<cache dir>/network_cache/` managed by `kuro-net::cache` (P2); size cap 5 MiB; safe to delete |
| Cookies | Chromium cookie DB inside `<dataDir>/app_webview/Default/Cookies` | SQLite (Chromium) | `<data dir>/cookies.json` (JSON, 0600) |
| WebView profile | `<dataDir>/app_webview/` (deleted by "Clear WebView data") | Chromium profile | WebKitGTK `NetworkSession` data directory `<data dir>/webkit/data` and cache `<cache dir>/webkit` created only when the persistent WebView screen is used; the Cloudflare solver uses an ephemeral session and writes nothing |
| Update APK download | `<externalCacheDir>/update.apk` | file | not applicable (`area:35-data-misc-updater-notification-export`) |
| Network log | system logcat via `HttpLoggingInterceptor` | text | `tracing` target `kuro_net::http` into the log file of T-01a-005 |

Directories always come from the `Paths` trait (`plans/addenda/requirements-cross-platform.md` rule 4).

## 6. Network endpoints touched by this area

| URL | Method | Params and headers | Auth | Response | By |
|---|---|---|---|---|---|
| each DoH URL in table 3 | GET (OkHttp default `post = false`) | `?dns=<base64url, no padding, DNS wire query>`; `Accept: application/dns-message` | none | `200`, body = DNS wire message (`Content-Type: application/dns-message`), at most 65 536 bytes | `DnsOverHttps` |
| any challenged site | GET (original request URL) via WebView | original request headers minus unsafe ones; `User-Agent` | none | Cloudflare challenge HTML (403 or 503, `Server: cloudflare`), then page; sets `cf_clearance` cookie | `CloudflareInterceptor` |
| `https://api.github.com/repos/${repository}/releases/latest` | GET | default headers, `max-age=600` | none | JSON `GithubRelease` | `ReleaseServiceImpl` (consumer; owner `area:35-data-misc-updater-notification-export`) |
| `http://api.resmush.it/ws.php?img=<url>&qlty=<quality>` | GET (blocking `execute()`) | none | none | JSON; the code reads `"dest":"..."` with `substringAfter`/`substringBefore` | `DataSaver.ReSmushItDataSaver` (consumer; owner `area:34-images-covers-cache`) |
| `$repo/repo.json` | GET | none | none | `ExtensionRepoMetaDto` JSON | `ExtensionRepoService` (consumer) |

Everything else (source sites, trackers, torrent server) is catalogued in `plans/catalogs/network-endpoints-01.md` and `network-endpoints-02.md`.

## 7. Database tables touched

None. This area reads and writes no SQLite table. (Sources and extension repositories are stored by `area:05-data-repos-entries-items` and `area:06-data-repos-history-track-source-misc`; the cookie store is not in a database.)

## 8. User-visible strings (English, from `i18n/src/commonMain/moko-resources/base/strings.xml`)

| Key | Text | Line |
|---|---|---|
| `MR.strings.label_network` | Networking | 599 |
| `MR.strings.pref_clear_cookies` | Clear cookies | 600 |
| `MR.strings.pref_dns_over_https` | DNS over HTTPS (DoH) | 601 |
| `MR.strings.pref_user_agent_string` | Default user agent string | 602 |
| `MR.strings.error_user_agent_string_blank` | User agent string can not be blank (unused in code) | 603 |
| `MR.strings.error_user_agent_string_invalid` | Invalid user agent string | 604 |
| `MR.strings.pref_reset_user_agent_string` | Reset default user agent string | 605 |
| `MR.strings.requires_app_restart` | Requires app restart to take effect | 606 |
| `MR.strings.cookies_cleared` | Cookies cleared | 607 |
| `MR.strings.pref_clear_webview_data` | Clear WebView data | 617 |
| `MR.strings.webview_data_deleted` | WebView data cleared | 618 |
| `MR.strings.cache_delete_error` | Error occurred while clearing | 588 |
| `MR.strings.pref_verbose_logging` | Verbose logging | 633 |
| `MR.strings.pref_verbose_logging_summary` | Print verbose logs to system log (reduces app performance) | 634 |
| `MR.strings.information_cloudflare_bypass_failure` | Failed to bypass Cloudflare | 955 |
| `MR.strings.information_cloudflare_help` | Tap here for help with Cloudflare | 956 |
| `MR.strings.information_webview_required` | WebView is required for the app to function | 959 |
| `MR.strings.information_webview_outdated` | Please update the WebView app for better compatibility | 961 |
| `MR.strings.exception_http` | HTTP %d, check website in WebView | 995 |
| `MR.strings.exception_offline` | No Internet connection | 996 |
| `MR.strings.exception_unknown_host` | Couldn't reach %s | 997 |
| `MR.strings.disabled` | Disabled (DoH off entry) | 512 |
| `MR.strings.action_webview_back`, `action_webview_forward`, `action_webview_refresh` | Back, Forward, Refresh | 161-163 |

The line numbers refer to `i18n/src/commonMain/moko-resources/base/strings.xml`.

## 9. Proposal: Kurobayomi Linux counterparts for new strings

Keys follow the same names in `kuro-i18n` (area 51). New keys introduced by this area: `information_webkit_missing` ("WebKitGTK is required to solve Cloudflare checks. Install webkitgtk-6.0."), `information_webview_outdated_linux` ("Please update WebKitGTK for better compatibility"), `cloudflare_check_title` ("Cloudflare check"), `cloudflare_check_hint` ("Complete the check in this window; it closes by itself when it succeeds."), `cookie_import_title` ("Import Cloudflare cookie"), `cookie_import_hint` ("Paste the value of the cf_clearance cookie from your browser and the browser's user agent."), `network_error_tls` ("Secure connection failed: %s").
