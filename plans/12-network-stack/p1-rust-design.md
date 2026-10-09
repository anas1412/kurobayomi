# Area 12-network-stack, part 1: Rust design

Scope and observed behaviour: `p1-inventory.md`, `p1-behavior.md`, `p1-data-and-config.md`. Deep dives: `p1-topic-cloudflare-and-webview.md`, `p1-topic-doh.md`, `p1-topic-rate-limiter.md`, `p1-topic-http-cache.md`. Platform rules: `plans/addenda/requirements-cross-platform.md` (rule 1: no `cfg(target_os)` outside `kuro-platform`; rule 4: every directory from the `Paths` trait).

Current mockup state (checked): `Cargo.toml` depends on `gtk4 0.11.5` (feature `v4_16`), `serde`, `serde_json`. There is no tokio, no HTTP client and no network code. `src/main.rs:12` is the GTK entry point. Networking UI exists only as static spec rows in `spec/settings.json` (group "Networking", around line 2805). Nothing in `src/` is wired to it yet.

## 1. Crate and module layout

Workspace members this area touches (names follow the plan's addenda and area 01):

```
kurobayomi/                       (root package today; becomes a workspace)
  crates/kuro-net/                NEW. HTTP client, interceptors, cookies, DoH, cache, JS
    src/lib.rs
    src/error.rs                  NetError, Display strings, user_message()
    src/request.rs                Request, Method, Body, CacheControl, GET/POST/PUT/DELETE builders
    src/response.rs               Response, await/await_success, parse_as
    src/progress.rs               ProgressListener, ProgressBody (stream adapter), call_with_progress
    src/client.rs                 NetworkClient (cheap clone), ClientBuilder (new_builder), NetworkHelper::new
    src/settings.rs               NetworkSettings (plain struct; filled by kuro-prefs)
    src/cookies.rs                CookieJar (cookie_store + reqwest_cookie_store, JSON persistence)
    src/dns.rs                    DohProvider enum + table, DohResolver (reqwest::dns::Resolve)
    src/cache.rs                  disk HTTP cache with 5 MiB LRU (P2)
    src/js.rs                     JsEngine::evaluate (rquickjs), JsValue, limits (P2)
    src/interceptors/mod.rs       Interceptor trait, Chain
    src/interceptors/uncaught.rs
    src/interceptors/user_agent.rs
    src/interceptors/rate_limit.rs
    src/interceptors/cloudflare.rs
    src/interceptors/safe_headers.rs
  crates/kuro-platform/           trait objects for OS features (rule 1)
    src/webview.rs                WebViewHost trait, SolveRequest, SolveOutcome (always compiled)
    src/linux/webview.rs          WebKitGTK 6.0 implementation (feature "webview", uses webkit6)
    src/fallback.rs               no-op host: available() == false
  src/toast.rs (kuro-ui)          Toaster (GTK overlay), see section 4
```

Rule for this area: `kuro-net` compiles without any GTK or WebKit dependency. Everything that needs a window goes through `WebViewHost` (section 3.3) and `Toaster`. This keeps Windows builds (rule 7) compiling and keeps the HTTP stack testable headless.

## 2. Core types

### 2.1 Errors

```rust
#[derive(thiserror::Error, Debug)]
pub enum NetError {
    #[error("HTTP error {code}")]
    Http { code: u16 },
    #[error("{0}")]
    InvalidUrl(String),
    #[error("Canceled")]
    Canceled,
    #[error("timeout")]
    Timeout,
    #[error("{host}")]
    UnknownHost { host: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Tls(String),
    #[error("Failed to bypass Cloudflare")]
    CloudflareBypassFailed,
    #[error("{0}")]
    Decode(String),
    #[error("{0}")]
    Other(String),
}
```

`Display` of `Http` equals the Kotlin `HttpException` message. `user_message` returns a message key from `kuro-i18n` (area 51) plus arguments; the UI never formats network errors itself:

```rust
pub enum UserMessage { Key(&'static str), KeyArg(&'static str, String), Raw(String) }
impl NetError {
    /// Mirrors app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:16-39.
    pub fn user_message(&self, online: bool) -> UserMessage { /* table in p1-behavior.md section 9 */ }
}
```

Mapping: `Http{code}` -> `KeyArg("exception_http", code)`; `UnknownHost` offline -> `Key("exception_offline")`; `UnknownHost` online -> `KeyArg("exception_unknown_host", host)`; `CloudflareBypassFailed` -> `Key("information_cloudflare_bypass_failure")`; `Canceled` -> not shown (caller treats as cancellation); everything else -> `Raw(display)`.

Note on `reqwest`: `reqwest::Error` is converted with `From`: `is_timeout()` -> `Timeout`; `is_connect()` with a DNS-resolution source -> `UnknownHost`; `is_builder()` -> `InvalidUrl`; anything else -> `Io` or `Other`. Rustls handshake errors -> `Tls`.

### 2.2 Request and response

```rust
pub enum Method { Get, Post, Put, Delete }

pub struct Request {
    pub method: Method,
    pub url: url::Url,
    pub headers: http::HeaderMap,          // case-insensitive; append vs insert as in OkHttp
    pub body: Body,
    pub cache: CacheControl,               // default: CacheControl::default_request() = max-age 600
    pub opts: CallOptions,
}

pub enum Body {
    Empty,                                 // Kotlin DEFAULT_BODY is an empty form
    Form(Vec<(String, String)>),           // application/x-www-form-urlencoded
    Bytes { content_type: mime::Mime, data: bytes::Bytes },
}

pub struct CacheControl {
    pub no_cache: bool, pub no_store: bool,
    pub max_age: Option<std::time::Duration>, pub max_stale: Option<std::time::Duration>,
    pub min_fresh: Option<std::time::Duration>, pub only_if_cached: bool,
}
impl CacheControl {
    pub fn default_request() -> Self { Self { max_age: Some(Duration::from_secs(600)), ..Self::none() } }
    pub const FORCE_NETWORK: ... // no_cache = true       (header "no-cache")
    pub const FORCE_CACHE: ...   // only_if_cached, max_stale = 2147483647 s
    pub fn to_header(&self) -> Option<HeaderValue>;     // empty -> None (header removed)
}

pub struct CallOptions {
    pub timeout: Option<Duration>,          // overrides the 2 min call timeout (progress calls use 30 h)
    pub use_cache: bool,                    // false for newCachelessCallWithProgress
    pub progress: Option<Arc<dyn ProgressListener>>,
    pub cancel: tokio_util::sync::CancellationToken,
}

pub struct Response {
    pub status: http::StatusCode,
    pub headers: http::HeaderMap,
    pub url: url::Url,                       // final URL after redirects
    pub from_cache: bool,                    // OkHttp: networkResponse == null
    body: ResponseBody,                      // private; consumed once
}
impl Response {
    pub fn is_successful(&self) -> bool { self.status.is_success() }   // 200..=299, as Kotlin
    pub fn header(&self, name: &str) -> Option<&str>;                  // last value wins (OkHttp Headers.get)
    pub async fn text(self) -> Result<String, NetError>;
    pub async fn bytes(self) -> Result<bytes::Bytes, NetError>;
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, NetError>;   // parse_as
}
```

`Response::header` returns the last value for repeated headers because `Request.header(name)` and `Response.header(name)` in OkHttp return the last one (`Headers.get`). This matters for `User-Agent` in `UserAgentInterceptor` (behaviour in `p1-behavior.md` 4.2).

`json` uses `serde_json::from_slice` over the fully buffered body (Kotlin streams). The body is limited to 64 MiB as a safety divergence; a larger body returns `NetError::Decode("response too large")`. Unknown fields are ignored (serde default), `null` on an `Option<T>` field is accepted, and `#[serde(default)]` is used where Kotlin's `explicitNulls = false` would make a missing key default. Kotlin's `coerceInputValues` and leniency are off, so serde strictness is equal.

### 2.3 Progress

```rust
pub trait ProgressListener: Send + Sync {
    /// bytes_read: total so far; content_length: -1 when unknown; done: true on the end-of-stream read.
    fn update(&self, bytes_read: u64, content_length: i64, done: bool);
}
```

`ProgressBody` wraps `reqwest::Response::bytes_stream()`: each chunk increments the total and calls `update(total, content_length, false)`; the end of the stream calls `update(total, content_length, true)` (the Kotlin end-of-stream read reports `done = true` with the same total). `content_length` is the response `Content-Length` header parsed as i64, or -1. Divergence: reqwest decompresses transparently, so the Rust counter sums DECODED bytes, while Kotlin's network-level interceptor counts RAW bytes against the raw `Content-Length`. Decision: keep decoded counting and report `Content-Length` as-is. A gzip response whose `Content-Length` is the compressed size can therefore report progress above 100 %; the consumer clamps to 100. Turning off compression to match Kotlin is rejected because it changes what is sent on the wire.

### 2.4 Interceptor chain

OkHttp's interceptor model is kept because extensions and the app need "add an interceptor" (`newBuilder().addInterceptor`). reqwest has no such hook, so `kuro-net` implements it:

```rust
#[async_trait::async_trait]
pub trait Interceptor: Send + Sync {
    async fn intercept(&self, chain: Chain<'_>, req: Request) -> Result<Response, NetError>;
}

pub struct Chain<'a> {
    interceptors: &'a [Arc<dyn Interceptor>],   // remaining application interceptors
    terminal: &'a Transport,                     // reqwest + cache + optional network logger
}
impl<'a> Chain<'a> {
    pub async fn proceed(self, req: Request) -> Result<Response, NetError>;
}

pub struct NetworkClient { inner: Arc<ClientInner> }   // Clone is an Arc bump
struct ClientInner { chain: Vec<Arc<dyn Interceptor>>, transport: Transport, cookies: CookieJar }

impl NetworkClient {
    pub fn new_builder(&self) -> ClientBuilder;           // copies the chain (newBuilder)
    pub async fn execute(&self, req: Request) -> Result<Response, NetError>;
    pub async fn get(&self, url: &str) -> Result<Response, NetError>;            // awaitSuccess semantics
    pub async fn post(&self, url: &str, body: Body) -> Result<Response, NetError>;
    pub fn cookies(&self) -> &CookieJar;
}
```

Order of the application chain (fixed, as `NetworkHelper.kt:20-64`): `UncaughtExceptionInterceptor`, `UserAgentInterceptor`, then any interceptors added by extensions or the rate limiter in call order, then `CloudflareInterceptor`. OkHttp's ordering puts `addInterceptor` (application) before `addNetworkInterceptor`; the terminal stage holds the network logger and the cache, in front of reqwest.

Important: OkHttp runs an application interceptor once per logical call even if redirects happen. The Rust `Chain::proceed` is therefore called once per `execute`, and redirects are handled inside reqwest (`redirect::Policy::limited(20)`). The Cloudflare retry (`chain.proceed(request)`) calls the terminal stage again, which matches Kotlin, where the retry goes through the network interceptors again.

### 2.5 Cancellation

Each `CallOptions` carries a `CancellationToken`. `execute` races the chain against the token (`tokio::select!`). Dropping the future also cancels reqwest's request. A cancelled call returns `NetError::Canceled`, which is never converted into a user-visible message. OkHttp's `call.cancel()` on coroutine cancellation maps to dropping the future.

### 2.6 Client builder and construction

```rust
pub struct NetworkSettings {
    pub verbose_logging: bool,            // default false; true only in debug builds (see p1-data-and-config.md section 2)
    pub doh_provider: i32,                // -1 default
    pub default_user_agent: String,       // trimmed on read
}

impl NetworkClient {
    pub fn build(settings: &NetworkSettings, paths: &dyn kuro_core::Paths,
                 webview: Arc<dyn kuro_platform::WebViewHost>) -> Result<NetworkClient, NetError>;
}
```

Construction order (proposal from `p1-behavior.md` 1.3):

1. `CookieJar::load(paths.data().join("cookies.json"))`.
2. Transport: `reqwest::Client::builder()` with `connect_timeout(30 s)`, `read_timeout(30 s)` (reqwest: per-read inactivity timeout, Kotlin `readTimeout` is the same semantics), `timeout(120 s)` (call timeout), `redirect(Policy::limited(20))`, `pool_idle_timeout(300 s)`, `pool_max_idle_per_host(5)`, `cookie_provider(jar.store())`, `use_rustls_tls()` with `tls_built_in_native_certs(true)` (system trust store, like Android), `dns_resolver(DohResolver)` when `doh_provider` is 1..13, `http2_prior_knowledge` NOT set (ALPN negotiation as OkHttp does), `user_agent` not set (set per request by the interceptor).
3. Dispatcher emulation: `tokio::sync::Semaphore(64)` overall and a per-host `Semaphore(5)` map, acquired in the terminal stage (OkHttp `Dispatcher` limits; these bound concurrent chapter-page downloads per host).
4. Application chain: `UncaughtExceptionInterceptor`, `UserAgentInterceptor(settings.default_user_agent)`, `CloudflareInterceptor(jar, webview, ...)`. Extra interceptors arrive later through `new_builder`.
5. Terminal: optional `NetworkLogger` (when `verbose_logging`), then the cache (P2), then the transport.
6. `DohResolver` is built with its own `reqwest::Client` pinned to the bootstrap addresses (`p1-topic-doh.md` section 4); it never uses the main client.

Restart semantics (matches Kotlin): `doh_provider` and `verbose_logging` are read once at construction. The settings UI changes the value and shows "Requires app restart to take effect" (`requires_app_restart`). `default_user_agent` is read by `UserAgentInterceptor` per request, so the new value applies to new requests immediately, but already-created source objects keep their headers (Kotlin `HttpSource.headers` is lazy). Kurobayomi keeps the restart note for the UA row to match the Kotlin UI.

### 2.7 Cookie jar

```rust
pub struct CookieJar { store: Arc<reqwest_cookie_store::CookieStoreMutex>, path: PathBuf, dirty: Arc<AtomicBool> }

impl CookieJar {
    pub fn load(path: PathBuf) -> Self;                              // corrupt file: rename to .corrupt, start empty
    pub fn store(&self) -> Arc<reqwest_cookie_store::CookieStoreMutex>;
    pub fn get(&self, url: &Url) -> Vec<cookie_store::Cookie<'static>>;  // unexpired matches for url
    pub fn remove(&self, url: &Url, names: Option<&[&str]>) -> usize;    // deletes domain-scoped too
    pub fn remove_all(&self);                                            // immediate write of empty store
    pub fn import(&self, url: &Url, cookies: Vec<cookie_store::Cookie<'static>>);  // from WebKit
    pub fn export_for(&self, url: &Url) -> Vec<cookie_store::Cookie<'static>>;     // to WebKit
}
```

Persistence: a background task (`tokio::spawn`) checks `dirty` every 500 ms and writes after 2 s of quiet time (debounce). Writes use `cookie_store::serde::json::save` into `cookies.json.tmp`, `fsync`, then `rename` (atomic), with file mode `0600`. Session cookies are not saved (`cookie_store` save skips non-persistent cookies, the usual browser rule). `Drop` of the last `CookieJar` clone flushes synchronously.

### 2.8 Interceptors

`uncaught.rs`: wraps the rest of the chain in `futures::FutureExt::catch_unwind`; a panic becomes `NetError::Other(panic message)`. Non-`NetError` failures from extension-defined interceptors are wrapped in `NetError::Other`. `Canceled` and `Timeout` are passed through unchanged.

`user_agent.rs`: `if req.headers.get_all(USER_AGENT).iter().last().map_or(true, |v| v.is_empty())` then `remove(USER_AGENT)` and `insert(USER_AGENT, provider())`, where `provider()` is the trimmed setting. If the trimmed value is empty, no header is inserted and `tracing::warn!` fires once per process (a `OnceLock`).

`rate_limit.rs`: full design in `p1-topic-rate-limiter.md` section 5.

`cloudflare.rs`: design in `p1-topic-cloudflare-and-webview.md` sections 2.2 to 2.5. Uses `safe_headers::parse_headers` from `safe_headers.rs` (T-12a-020).

`safe_headers.rs`: `pub fn safe_request_headers(h: &HeaderMap) -> Vec<(HeaderName, HeaderValue)>` implementing `isRequestHeaderSafe` (lower-case name and value, drop `content-length`, `host`, `trailer`, `te`, `upgrade`, `cookie2`, `keep-alive`, `transfer-encoding`, `set-cookie`, any `proxy-*`, and `connection: upgrade`). Keeps the first value per name (the Kotlin `groupBy ... getOrNull(0)`). Note that `cookie` is not in the list and passes.

## 3. Platform traits

### 3.1 Async model

* One multi-thread `tokio::runtime::Runtime` created in `main` before `gtk::Application::run` (task T-01a-009). Every `kuro-net` future runs on it.
* GTK and WebKit objects are touched only on the GTK main thread. `glib::MainContext::default()` is the bridge: the solver sends a closure with a `tokio::sync::oneshot` sender back, and the GTK side reports the result through the same channel. Widgets never get `await`ed from tokio.
* Blocking work (`rquickjs`, filesystem flush of big files) uses `tokio::task::spawn_blocking`.
* Kotlin's `withIOContext` maps to running on the tokio runtime; there is no separate IO dispatcher.

### 3.2 Why the GTK loop never blocks on the network

Kotlin's Cloudflare path blocks an OkHttp thread for up to 30 s. Rust awaits a future, so no thread is parked: the solve is `async`, the channel wakes the future, and other requests continue. The only blocking point left is the GTK main loop running the WebView, which is non-blocking by nature.

### 3.3 `WebViewHost` (in `kuro-platform/src/webview.rs`)

```rust
#[async_trait::async_trait]
pub trait WebViewHost: Send + Sync {
    fn available(&self) -> bool;
    fn version_string(&self) -> String;                 // "WebKitGTK x.y.z" or "WebKitGTK not available"
    fn is_outdated(&self) -> bool;                      // (major, minor) < (2, 42)
    async fn solve_challenge(&self, req: SolveRequest, cancel: CancellationToken) -> SolveOutcome;
    async fn open_page(&self, req: PageRequest) -> Result<(), PlatformError>;   // WebView screen (area 36)
    async fn clear_data(&self) -> Result<(), PlatformError>;                    // Clear WebView data (T-12a-034)
}
```

The full `SolveRequest`, `SolveOutcome` and `SolveFailure` definitions are in `p1-topic-cloudflare-and-webview.md` section 2.4. Implementations: `linux::WebKitHost` (feature `webview`, crate `webkit6`, Arch package `webkitgtk-6.0`) and `fallback::NoWebView` (`available() == false`, `solve_challenge` returns `Failed { reason: Unavailable }`, which drives the manual cookie import dialog in T-12a-033).

### 3.4 Toaster (in `kuro-ui`)

```rust
pub enum ToastLength { Short, Long }           // 2000 ms, 3500 ms
pub struct Toaster { tx: glib::Sender<ToastMsg> }
impl Toaster {
    pub fn show(&self, text: impl Into<String>, length: ToastLength);   // any thread; empty text ignored
}
```

Implementation: a `gtk::Overlay` child (a `gtk::Label` in a `gtk::Revealer` at the bottom centre, styled by the app's Tokyonight CSS, rule 6). A `VecDeque` of at most 5 pending messages; a new message beyond 5 is dropped (divergence from Android, which queues without limit; the cap prevents a flood from a failing source). Shown one at a time, each for its length, then the next one starts. The `Toaster` handle is held by `AppContext` (area 01), never a global. Send is thread-safe through `glib::Sender`.

## 4. Mapping: Android / Kotlin to Linux / Rust

| Kotlin / Android | Rust / Linux replacement | Notes |
|---|---|---|
| OkHttp 5.4.0 `OkHttpClient` | `reqwest` (rustls, native roots, gzip, brotli, deflate, zstd, cookies, http2) | Interceptors reimplemented in `kuro-net` |
| `okhttp3.Cache` (DiskLruCache) | `http-cache-semantics` + disk store, 5 MiB LRU | `p1-topic-http-cache.md` |
| `okhttp3.CookieJar` + Chromium `CookieManager` | `cookie_store` + `reqwest_cookie_store`, JSON file | WebKit sync is explicit (topic section 4) |
| `DnsOverHttps` | hickory-proto wire codec + reqwest GET | `p1-topic-doh.md` |
| `Dispatcher` (64 / 5) | two semaphore sets | section 2.6 step 3 |
| `RateLimitInterceptor` | tokio mutex + `Notify` + `Instant` | `p1-topic-rate-limiter.md` |
| `CountDownLatch` + WebView on main executor | `oneshot` + `glib::MainContext` | section 3.1 |
| `WebView` / `WebSettings` / `CookieManager` (WebView) | `webkit6` (`WebView`, `NetworkSession::new_ephemeral`, `CookieManager`, `Settings`) | topic section 3 |
| `WebSettings.getDefaultUserAgent` | `webkit6::functions::major_version` and friends | version check only |
| `Toast.makeText` | in-window `Toaster` (section 3.4) | no Android toast on desktop |
| `ContextCompat.getMainExecutor` | `glib::MainContext::default().invoke` | |
| `SystemClock.elapsedRealtime` | `std::time::Instant` / `tokio::time::Instant` | suspend caveat in the rate limiter topic |
| `kotlinx.serialization` `Json` | `serde` + `serde_json` (`ignoreUnknownKeys` by default) | |
| `QuickJS` `app.cash.quickjs` | `rquickjs` (QuickJS-ng bindings) | limits in section 5 |
| `Context.cacheDir` / `dataDir` | `Paths::cache()`, `Paths::data()` | `Paths` in area 01 |
| `ro.miui.*`, `SEM_PLATFORM_INT`, `Build.MANUFACTURER` | none (dropped) | T-12a-038 |
| `EGL10` texture probe | `gdk::GLContext` (area 01, T-01a-022) | outside this area |
| `Resources.getSystem().displayMetrics.density` | none (dropped) | T-12a-040 |
| `Intent.ACTION_VIEW` browser chooser | `gio::AppInfo::launch_default_for_uri` through `kuro-platform` | out of scope here |
| `Context.packageManager` package-name spoof | none (dropped) | T-12a-037 |

## 5. JavaScript evaluation (P2)

```rust
pub enum JsValue { Null, Bool(bool), Int(i32), Double(f64), Str(String) }
pub enum JsError { Script(String), UnsupportedResult(&'static str), MemoryLimit, Timeout }
pub async fn evaluate(script: String) -> Result<JsValue, JsError>;   // spawn_blocking
```

Per call: new `rquickjs::Runtime` with `set_memory_limit(64 MiB)`, `set_max_stack_size(512 KiB)`, interrupt handler that returns `true` after 30 s wall time. A fresh `Context` per call, dropped afterwards. The Kotlin unchecked cast becomes `UnsupportedResult(typeof)` for objects, arrays, functions, `undefined`, BigInt and symbols. Divergence: Kotlin has no limits at all; the three limits are added for safety and are listed in the edge-case file.

## 6. Crates (versions to pin at implementation time, verify on crates.io)

| Crate | Use | Why this one |
|---|---|---|
| `reqwest` (features `rustls-tls-native-roots`, `cookies`, `gzip`, `brotli`, `deflate`, `zstd`, `stream`, `json`) | transport | mature, tokio-native |
| `reqwest_cookie_store` + `cookie_store` | cookie store | works with reqwest's `cookie_provider` |
| `tokio` (rt-multi-thread, time, sync, macros), `tokio-util` (`CancellationToken`) | runtime, cancellation | |
| `async-trait` | `Interceptor`, `WebViewHost` object safety | simplest dyn-compatible async |
| `futures` | `FutureExt::catch_unwind`, `join_all` | |
| `hickory-proto` | DNS wire format for DoH | avoids a hand-written codec |
| `psl` | public suffix check for DoH private-host rule | OkHttp uses PublicSuffixDatabase |
| `http-cache-semantics`, `http-cache-reqwest` (P2) | RFC 9111 freshness | matches OkHttp rules in the topic |
| `cacache` (P2) | disk store for the cache | content-addressed, atomic writes |
| `rquickjs` (P2) | JavaScript for sources | QuickJS binding |
| `webkit6` 0.6.x (feature `webview`) | WebKitGTK 6.0 | pairs with gtk4 0.11 |
| `serde`, `serde_json`, `base64`, `url`, `http`, `bytes`, `mime` | data | already used or standard |
| `thiserror`, `tracing` | errors and logs | `tracing` target `kuro_net::http` (T-01a-005) |

Do not add `native-tls`: rustls with native roots gives the platform trust store without OpenSSL linkage.

## 7. Error handling policy

* Library code returns `Result<_, NetError>`; no `unwrap` or `expect` outside tests.
* A failure inside an interceptor never leaves a permit or a semaphore unreleased: all permits are RAII guards.
* Logs: `tracing::warn!` for recoverable issues (cookie file corrupt, invalid UA, DoH fallback), `tracing::error!` only for bugs (WebKit callback on the wrong thread). Verbose network logs at `debug` on target `kuro_net::http` when `verbose_logging` is on; URL query strings and `Cookie`/`Authorization` header values are redacted (`cf_clearance` value shown as `<redacted>`).

## 8. Pitfalls

1. reqwest's default redirect limit is 10; Kotlin uses 20. Set `Policy::limited(20)` explicitly.
2. reqwest decodes gzip transparently; the progress percentage can exceed 100 when `Content-Length` is the compressed length (section 2.3).
3. `Cookie` header from the jar is attached by reqwest per redirect hop; the Cloudflare retry must not double-add cookies (the jar already holds them).
4. WebKitGTK must run on the GTK thread; calling `webkit6` from a tokio task panics in `gtk::init` checks or silently misbehaves. All WebKit entry points go through `glib::MainContext`.
5. `NetworkSession::new_ephemeral` destroys cookies when dropped; the solver must export cookies to the jar before the session is dropped (topic section 2.5 "Finish always").
6. Hidden `gtk::Window` solver: Wayland may not throttle an unmapped view the same way; topic section 3 item 8 requires a spike (T-12a-017).
7. DoH rejects single-label and `localhost` names while enabled (topic section 2). Do not "fix" this silently: document it in settings.
8. `reqwest::dns::Resolve` results are not cached by reqwest; the DoH resolver keeps its own TTL cache (topic section 4 item 8).
9. A `HeaderValue` cannot contain bytes outside `\t` and `0x20..=0x7E`; the UA setting validation (T-12a-031) uses `HeaderValue::from_str` so the rule is the same as OkHttp.
10. `Instant` does not advance during system suspend on Linux; the rate limiter may be stricter after resume, never looser.

## 9. Integration with the mockup (`src/`)

* `src/main.rs`: create the tokio runtime and the `NetworkClient` in `main` (before `app.run()`), store them in `AppContext` (area 01). No global.
* `src/settings.rs`: the "Networking" group rows (spec `spec/settings.json` lines around 2805 to 2843) get real handlers: Clear cookies (`CookieJar::remove_all` then `Toaster::show("Cookies cleared", Short)`), Clear WebView data (`WebViewHost::clear_data`), DoH combo (values -1 and 1..13 from the enum), UA entry with validation, reset UA. The existing mockup strings must match the English keys listed in `p1-data-and-config.md` section 8.
* `src/ui.rs`: attach the `Toaster` overlay to the main window's `gtk::Overlay`.
* `src/screens.rs` / `src/entry.rs`: "Open in WebView" (`entry.rs:417`) calls `WebViewHost::open_page` (area 36 owns the screen).
* `spec/*.json`: no new spec rows are needed; the Networking rows exist. The Cloudflare check window (section 5 of the UI parity file) is a new screen that has no spec entry yet.

## 10. Decisions still needed from the user

1. Visible challenge window (T-12a-032) is planned as P2. If the hidden solver spike (T-12a-017) fails on the user's KDE Wayland session, this must move to P1 and it changes the default flow.
2. DoH is kept as P2 and opt-in, matching Aniyomi's default (`-1`). Confirm.
