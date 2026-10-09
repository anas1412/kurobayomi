# Area 12-network-stack: overview

Area lead summary for the HTTP stack of Aniyomi (Kotlin, OkHttp 5.4.0) and its Rust replacement in Kurobayomi (`reqwest` based, crate `kuro-net`). Every path in this file is repo-relative to `/home/blackbox/aniyomi-src` unless it starts with `plans/` or `src/`, which are relative to `/home/blackbox/kurobayomi`. OBSERVED = what the Kotlin does. Proposal = what the Rust side should do.

## 1. Purpose and scope

### 1.1 What Aniyomi does here

Every network request an Aniyomi user triggers goes through one shared OkHttp client built in `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt`. That client carries:

* the cookie jar (`AndroidCookieJar`, backed by Chromium's `CookieManager` so the WebView and OkHttp share cookies),
* timeouts (connect 30 s, read 30 s, whole call 2 min),
* a 5 MiB HTTP disk cache in `<cacheDir>/network_cache`,
* two application interceptors that always run (`UncaughtExceptionInterceptor`, `UserAgentInterceptor`),
* a Cloudflare interceptor that solves `403`/`503` challenges in a hidden WebView,
* an optional DNS-over-HTTPS resolver (13 providers, selected by `doh_provider`),
* optional HTTP logging (`verbose_logging`).

Sources (manga, anime), trackers (AniList, MAL, Kitsu, Bangumi, Komga, Jellyfin, and the others in areas 27-33), extension repository indexes, the Coil image loader, the app updater and the torrent server API all use this one client. Per-request rate limiting (`rateLimit`, `rateLimitHost`) is layered on top for extensions and for AniList (`app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt:42`, `.rateLimit(permits = 85, period = 1.minutes)`).

### 1.2 Why it matters for a usable Kurobayomi

* Without the transport, no source, tracker, cover or update check works. This is P0.
* Cloudflare handling decides whether a large share of real sites load at all. Its P1 tasks (T-12a-015 to T-12a-022) are the single largest risk to daily use (see section 4 and 6).
* Cookies and user agent decide whether a logged-in tracker or source session survives. Getting these wrong looks like random logouts.
* DoH and rate limits are optional but are user-visible settings (Settings > Advanced > Networking).

### 1.3 In scope and out of scope

In scope (21 Kotlin files, 1572 lines, listed in section 2.1): the `network` package, its six interceptors, and five `util/system` helpers (`WebViewUtil`, `ToastExtensions`, `DeviceUtil`, `GLUtil`, `DensityExtensions`).

Out of scope, owned elsewhere:

* Preference storage and typed accessors: `area:02-preferences-system` (this area reads `Settings.network`).
* Process bootstrap, tokio runtime, tracing setup, `Paths`, GL texture probe, low-RAM check: `area:01-foundation-app-bootstrap`.
* The Networking rows in Settings: `area:47-settings-screens-general` (behaviour is specified here).
* The WebView browsing screen: `area:36-ui-shell-navigation-security-webview`.
* Source, tracker and extension API surfaces that consume the client: areas 15-20 and 27-33.

## 2. Module map

### 2.1 Kotlin packages to responsibilities (observed)

Line totals: `network` top level 728 lines, `network/interceptor` 508 lines, `util/system` 336 lines, total 1572.

| Package | File | Lines | Responsibility |
|---|---|---|---|
| `eu.kanade.tachiyomi.network` | `AndroidCookieJar.kt` | 54 | OkHttp `CookieJar` on Chromium `CookieManager` |
| `eu.kanade.tachiyomi.network` | `DohProviders.kt` | 199 | 13 DoH provider builders, bootstrap IP lists |
| `eu.kanade.tachiyomi.network` | `HttpException.kt` | 13 | `HttpException(code)`, message `HTTP error <code>` |
| `eu.kanade.tachiyomi.network` | `JavaScriptEngine.kt` | 26 | QuickJS evaluation for sources, one engine per call |
| `eu.kanade.tachiyomi.network` | `NetworkHelper.kt` | 74 | Builds the shared client and the Cloudflare client alias |
| `eu.kanade.tachiyomi.network` | `NetworkPreferences.kt` | 25 | Accessors `verboseLogging`, `dohProvider`, `defaultUserAgent` |
| `eu.kanade.tachiyomi.network` | `OkHttpExtensions.kt` | 145 | `await`, `awaitSuccess`, `parseAs`, progress call, RxJava bridges |
| `eu.kanade.tachiyomi.network` | `ProgressListener.kt` | 5 | Progress callback interface |
| `eu.kanade.tachiyomi.network` | `ProgressResponseBody.kt` | 51 | Body wrapper that reports bytes read |
| `eu.kanade.tachiyomi.network` | `Requests.kt` | 136 | `GET`/`POST`/`PUT`/`DELETE` request factories, client `get`/`post` |
| `eu.kanade.tachiyomi.network.interceptor` | `CloudflareInterceptor.kt` | 138 | Cloudflare detection, hidden WebView solve, one retry |
| `eu.kanade.tachiyomi.network.interceptor` | `WebViewInterceptor.kt` | 113 | Base class: header filter, WebView creation, availability |
| `eu.kanade.tachiyomi.network.interceptor` | `RateLimitInterceptor.kt` | 128 | Sliding-window limiter, `rateLimit` builder extensions |
| `eu.kanade.tachiyomi.network.interceptor` | `SpecificHostRateLimitInterceptor.kt` | 77 | Host-scoped `rateLimitHost` builder extensions |
| `eu.kanade.tachiyomi.network.interceptor` | `UncaughtExceptionInterceptor.kt` | 28 | Converts non-`IOException` failures to `IOException` |
| `eu.kanade.tachiyomi.network.interceptor` | `UserAgentInterceptor.kt` | 24 | Adds the default `User-Agent` when missing or empty |
| `eu.kanade.tachiyomi.util.system` | `WebViewUtil.kt` | 116 | WebView availability, version text, default settings, HTML extraction |
| `eu.kanade.tachiyomi.util.system` | `ToastExtensions.kt` | 33 | Android toast helpers |
| `eu.kanade.tachiyomi.util.system` | `DeviceUtil.kt` | 113 | Vendor ROM probes, low-RAM test |
| `eu.kanade.tachiyomi.util.system` | `GLUtil.kt` | 65 | GL max texture size probe and option list |
| `eu.kanade.tachiyomi.util.system` | `DensityExtensions.kt` | 9 | `Int.dpToPx` |

Interceptor order for `network.client` (OBSERVED, `NetworkHelper.kt:18-64`): `UncaughtExceptionInterceptor` then `UserAgentInterceptor`, then (for the shared client) `CloudflareInterceptor`. Extension and tracker clients derived with `newBuilder().addInterceptor(...)` run after Cloudflare and nearer the network, so a Cloudflare retry passes through their rate limiter again.

### 2.2 Rust crate and module map (proposal)

Taken from `p1-rust-design.md` section 1. `kuro-net` must compile without GTK or WebKit (rule 1 of `plans/addenda/requirements-cross-platform.md`).

```
crates/kuro-net/
  src/lib.rs
  src/error.rs            NetError, Display strings, user_message()          <- Kotlin HttpException, exception formatting
  src/request.rs          Request, Method, Body, CacheControl, builders      <- Requests.kt
  src/response.rs         Response, await/await_success, parse_as            <- OkHttpExtensions.kt
  src/progress.rs         ProgressListener, ProgressBody, call_with_progress <- ProgressListener.kt, ProgressResponseBody.kt
  src/client.rs           NetworkClient, ClientBuilder, NetworkHelper::new   <- NetworkHelper.kt
  src/settings.rs         NetworkSettings (plain struct, filled by kuro-prefs) <- NetworkPreferences.kt
  src/cookies.rs          CookieJar (cookie_store + reqwest_cookie_store)   <- AndroidCookieJar.kt
  src/dns.rs              DohProvider enum and table, DohResolver           <- DohProviders.kt
  src/cache.rs            disk HTTP cache, 5 MiB LRU (P2)                  <- NetworkHelper cache, OkHttp CacheStrategy
  src/js.rs               JsEngine::evaluate via rquickjs (P2)             <- JavaScriptEngine.kt
  src/interceptors/mod.rs Interceptor trait, Chain
  src/interceptors/uncaught.rs
  src/interceptors/user_agent.rs
  src/interceptors/rate_limit.rs                                           <- RateLimitInterceptor.kt, SpecificHost...
  src/interceptors/cloudflare.rs                                           <- CloudflareInterceptor.kt
  src/interceptors/safe_headers.rs                                         <- WebViewInterceptor.kt header filter
crates/kuro-platform/
  src/webview.rs          WebViewHost trait, SolveRequest, SolveOutcome (always compiled)
  src/linux/webview.rs    WebKitGTK 6.0 implementation (feature "webview", crate webkit6)
  src/fallback.rs         no-op host, available() == false
crates/kuro-core/         Paths trait (shared helpers, area:01)
crates/kuro-ui (src/toast.rs, the existing src/ tree)   Toaster overlay
```

Mapping of the Kotlin `util/system` files: `WebViewUtil` goes to `kuro-platform` (`WebViewHost::version_string`, `is_outdated`, `available`, `clear_data`); `ToastExtensions` goes to `kuro-ui::Toaster`; `DeviceUtil` keeps only the low-RAM rule (T-01a-030, `kuro-core`); `GLUtil` keeps only its option-list algorithm (T-12a-039, owned by area 01 for the probe); `DensityExtensions` is dropped.

### 2.3 Library mapping (proposal)

| Kotlin / Android | Rust / Linux | Source of the decision |
|---|---|---|
| OkHttp 5.4.0 client | `reqwest` (rustls, native roots) | `p1-rust-design.md` section 4 |
| `CookieJar` plus Chromium `CookieManager` | `cookie_store` plus `reqwest_cookie_store`, JSON file `cookies.json` | `p1-rust-design.md` 2.7 |
| `DnsOverHttps` | `hickory-proto` wire codec plus reqwest GET | `p1-topic-doh.md` section 4 |
| `Dispatcher` (64 total, 5 per host) | two `tokio::sync::Semaphore` sets | `p1-rust-design.md` 2.6 |
| `okhttp3.Cache` | `http-cache-semantics` plus disk store | `p1-topic-http-cache.md` section 5 |
| `CountDownLatch` plus main-executor WebView | `tokio::sync::oneshot` plus `glib::MainContext` | `p1-rust-design.md` 3.1 |
| `WebView`, `WebSettings`, `CookieManager` | `webkit6` (`WebView`, `NetworkSession::new_ephemeral`, `CookieManager`, `Settings`) | `p1-topic-cloudflare-and-webview.md` section 3 |
| `Toast.makeText` | `kuro-ui::Toaster` (GTK overlay) | `p1-rust-design.md` 3.4 |
| `kotlinx.serialization` | `serde`, `serde_json` | `p1-rust-design.md` section 4 |
| `app.cash.quickjs` 0.9.2 | `rquickjs` | `p1-behavior.md` section 10 |
| `SystemClock.elapsedRealtime` | `std::time::Instant` / `tokio::time::Instant` | `p1-topic-rate-limiter.md` section 5 |

Do not add `native-tls`. Rustls with native roots gives the system trust store without OpenSSL.

## 3. Reading guide

All documents are in this folder. Read them in this order for a first pass: inventory, behaviour, data and config, Rust design, then the topic files, then edge cases, UI parity and tasks.

1. [p1-inventory.md](p1-inventory.md): one row per Kotlin scope file with LOC, declarations, decision (Port, Rewrite, Replace-with-crate, Drop) and task IDs.
2. [p1-behavior.md](p1-behavior.md): OBSERVED behaviour of every file, with `path:line` citations, and the matching Proposal for each part. Covers construction order, consumers, request helpers, interceptors, cookies, DoH, response handling, settings, user-visible messages, QuickJS, util helpers, threading.
3. [p1-data-and-config.md](p1-data-and-config.md): every constant, the three preferences with keys and defaults, the complete 13-provider DoH table (URLs and bootstrap IPs), data models, files on disk, endpoints touched, and the English strings with line numbers in `strings.xml`.
4. [p1-rust-design.md](p1-rust-design.md): crate and module layout, Rust types (`NetError`, `Request`, `Response`, `Interceptor`, `Chain`, `NetworkClient`, `CookieJar`), the `WebViewHost` and `Toaster` traits, Kotlin-to-Rust mapping table, JavaScript limits, crate list, error policy, pitfalls, mockup integration, open decisions.
5. [p1-topic-cloudflare-and-webview.md](p1-topic-cloudflare-and-webview.md): the Cloudflare flow step by step, a finding that the Kotlin `onReceivedError` branch never fires for HTTP 403/503, the Rust state machine, WebKitGTK implementation notes, cookie sharing rules, failure matrix.
6. [p1-topic-doh.md](p1-topic-doh.md): OkHttp `DnsOverHttps` lookup algorithm, the private-host rule, the 65536-byte cap, the Rust resolver algorithm and failure table.
7. [p1-topic-http-cache.md](p1-topic-http-cache.md): OkHttp cache storage and freshness rules, the 10 minute request cap, the Rust cache design (P2).
8. [p1-topic-rate-limiter.md](p1-topic-rate-limiter.md): the public API, the sliding-window algorithm, the host quirk, the Rust limiter design (see risk R1 in section 6).
9. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge-case tables C, D, N, R, I, L, W, U with Kotlin and Rust expectations, race conditions, intentional non-reproductions, the test list and fixtures.
10. [p1-ui-parity.md](p1-ui-parity.md): every toast, the browse-screen error surface, the Cloudflare challenge window (hidden and visible), the Networking settings rows and the UA dialog, states, keyboard behaviour, responsive rules.
11. [p1-tasks.md](p1-tasks.md): 44 task blocks `T-12a-001` to `T-12a-044` with priority, effort, dependencies, sources and acceptance criteria.

## 4. Key design decisions and open questions

Each item needs the user's call. The recommendation is the area lead's proposal and is the default if no answer is given.

| # | Question | Options | Recommendation | Where |
|---|---|---|---|---|
| Q1 | Should the Cloudflare solver run hidden (1x1 window) or visible by default? | (a) hidden first, visible fallback (T-12a-032, P2); (b) visible from the start | (a), but decide after the T-12a-017 spike on the user's KDE Wayland session. If the spike fails, promote T-12a-032 to P1 and make the visible window the default. | `p1-topic-cloudflare-and-webview.md` 3 item 8; `p1-rust-design.md` 10 item 1 |
| Q2 | Is Cloudflare solving required for daily use? | (a) keep P1; (b) promote T-12a-015 and T-12a-016 to P0 | (b) if any source the user reads is behind Cloudflare. Without it, `403`/`503` from Cloudflare-protected sites is a hard failure in the P0 build. | section 6 risk R2 |
| Q3 | DoH default | (a) off (`-1`), as Aniyomi; (b) on | (a) off. The private-host rule makes `localhost` and single-label LAN names fail while DoH is on, so DoH must stay opt-in and documented. | `p1-rust-design.md` 10 item 2; `p1-topic-doh.md` 2 |
| Q4 | Does the Rust client send `Accept-Encoding: br, zstd`? | (a) gzip only, as Aniyomi on the wire; (b) gzip, deflate, br, zstd (the features listed in `p1-rust-design.md` section 6) | (a) for parity. Kotlin requests `gzip` only (`okhttp-brotli` and `okhttp-zstd` are on the classpath but no interceptor requests them). Enabling the `brotli` and `zstd` reqwest features changes the header without a stated reason. Choose (b) only for bandwidth. | section 6 risk R5 |
| Q5 | Rate limiter lock design | (a) as written in the topic file (one `tokio::sync::Mutex` held across the sleep); (b) Kotlin-equivalent: a fair one-permit gate plus a short-held queue lock | (b). Design (a) makes the cache-hit refund wait for the blocked waiter. See risk R1. | `p1-topic-rate-limiter.md` section 5; section 6 R1 |
| Q6 | Timeout semantics for queued requests | (a) call timeout starts at dispatch (OkHttp behaviour, `p1-edge-cases-and-tests.md` 2.7); (b) timeout starts after the dispatcher permit is granted (`p1-rust-design.md` 2.6 step 3) | (a). The two documents disagree. A request waiting for a host slot must not time out in the queue. Use (a) and count queue wait inside the 2 min budget only after acquisition, or document a deliberate divergence. Decide and fix both documents. | section 6 risk R4 |
| Q7 | `doh_provider` and `verbose_logging` take effect only after restart | (a) restart note as Aniyomi; (b) rebuild the client on change | (a) for P0/P1. Rebuild is a small addition later, not in scope now. | `p1-rust-design.md` 2.6 restart semantics |
| Q8 | Toast queue | (a) cap 5, drop the rest (Kurobayomi divergence); (b) unlimited queue as Android | (a). A failing source can emit many toasts. | `p1-ui-parity.md` section 1 |
| Q9 | QuickJS safety limits | (a) 64 MiB memory, 512 KiB stack, 30 s interrupt (new limits, divergence from Kotlin, which has none); (b) no limits as Kotlin | (a). A source script that loops forever otherwise blocks a worker thread forever in Aniyomi. | `p1-rust-design.md` section 5 |
| Q10 | JSON body cap | (a) 64 MiB buffered (`Response::json`); (b) stream like Kotlin | (a). Source API responses are small. A streaming parser is extra code with no observed need. | `p1-rust-design.md` 2.2 |
| Q11 | Cookie persistence | (a) own JSON store, debounced 2 s, mode 0600, session cookies memory-only; (b) persist session cookies too | (a). Browser rule. A crash can lose up to 2 s of cookies (edge case C10). | `p1-rust-design.md` 2.7; `p1-edge-cases-and-tests.md` C10 |
| Q12 | Redact `Cookie` and `Authorization` in verbose logs | (a) redact; (b) log values as Aniyomi HEADERS level | (a). Verbose logs get pasted into bug reports. | `p1-rust-design.md` section 7; edge case N9 |
| Q13 | Cloudflare `Server` header match | (a) case-insensitive; (b) exact as Kotlin | (a). Harmless relaxation. | `p1-topic-cloudflare-and-webview.md` 2.1 |
| Q14 | Keep the restart note on the UA row after a change | (a) yes, as Aniyomi; (b) no, since the UA interceptor reads the value per request | (a). Sources cache their headers (`HttpSource.headers` is lazy), so the restart note is still true for them. | `p1-behavior.md` 4.3 |

Decided (no question open): the Kotlin `cloudflareClient` alias is not carried over (`p1-edge-cases-and-tests.md` section 3); RxJava bridges are dropped (T-01a-038); `spoofedPackageName` and `getInferredUserAgent` (T-12a-037), vendor ROM probes (T-12a-038) and `dpToPx` (T-12a-040) are dropped.

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | Needed item | Task that needs it |
|---|---|---|
| area:01-foundation-app-bootstrap | `Paths` trait (data, cache, config dirs), tokio runtime (T-01a-009), tracing setup with the verbose switch (T-01a-005), warm-up hook (T-01a-018), GL texture probe (T-01a-022), low-RAM check (T-01a-030), WebView-package spoof drop (T-01a-026), RxJava drop (T-01a-038), `AppContext` holder | T-12a-003, T-12a-014, T-12a-026, T-12a-028, T-12a-037, T-12a-038, T-12a-039, T-12a-040 |
| area:02-preferences-system | `Settings.network` with the keys `verbose_logging`, `doh_provider`, `default_user_agent`; bootstrap preference group Network (T-01a-011) | T-12a-006, T-12a-007, T-12a-024, T-12a-031 |
| T-req-platform-006 (`plans/addenda/requirements-cross-platform-tasks.md`) | `WebViewHost` trait with browser-fallback path | T-12a-017, T-12a-019 |
| area:11-app-migrations | `DOHMigration` (version 57, `enable_doh` to `doh_provider`) is not ported here; the Rust app starts without `enable_doh` | none in this area (note only) |
| area:22-backup-restore | An Aniyomi backup that carries `enable_doh` or the three network keys must map onto `Settings.network` | none in this area (note only) |
| area:20-extension-strategy-redesign | Extension-facing network API surface (`rate_limit`, `rate_limit_host`, `get`, `post`, interceptor registration) | T-12a-029 |
| area:36-ui-shell-navigation-security-webview | WebView screen uses `WebViewHost::open_page`, `clear_data`, cookie removal per URL, page HTML extraction | T-12a-034, T-12a-041 |
| area:47-settings-screens-general | Networking group rows in `spec/settings.json` (around line 2805) and the new DoH helper line and cookie import dialog | T-12a-031, T-12a-044 |
| area:51-i18n | Message keys listed in `p1-data-and-config.md` section 8 and the new Kurobayomi keys in section 9 | T-12a-013, T-12a-019, T-12a-031 |
| area:52-util-app | `isOnline` for `exception_offline` versus `exception_unknown_host` | T-12a-013 |
| area:01 and area:43/44 (reader) | GL texture probe result used by the reader image pipeline | T-12a-039 |

### 5.2 What others need from this area

| Area | What they consume |
|---|---|
| area:15-source-api-manga, area:16-source-api-anime, area:17-source-app-layer | `NetworkClient` for `HttpSource` and `AnimeHttpSource`; `get`, `post`, `get_image` with progress (`newCachelessCallWithProgress` semantics); default headers including `User-Agent` |
| area:18-extensions-manga, area:19-extensions-anime, area:20-extension-strategy-redesign | `rate_limit`, `rate_limit_host`, `JsEngine::evaluate`, `NetError` types |
| area:27-tracking-core, area:28 to area:33 (trackers) | Auth clients built with `new_builder()`; DNS-system override for Komga and Jellyfin; AniList limit `85 per 1 minute` |
| area:24-downloads-anime, area:23-downloads-manga | `ProgressListener` and `call_with_progress` for page downloads |
| area:26-library-update-jobs | Shared client with per-host concurrency limits |
| area:34-images-covers-cache | Image loader uses the shared client; reSmush.it data saver (plain HTTP GET, `DataSaver.kt:140-174`) |
| area:35-data-misc-updater-notification-export | GitHub release check (`ReleaseServiceImpl.kt:19-22`) and app update download with progress |
| area:36-ui-shell-navigation-security-webview | `WebViewHost`, cookie jar `get`/`remove`, `NetworkSession` sharing rules |
| area:40-ui-browse-manga, area:41-ui-browse-anime | `NetError::user_message(online)` for error panels |
| area:45-player-core, area:46-player-controls-ui | Verbose logging switch for mpv message level |
| area:47-settings-screens-general | Action rows and the DoH and UA controls |
| area:52-util-app | `Toaster` for user messages |

## 6. Task summary

### 6.1 Counts

Source: `python3 plans/_tools/tasks.py export` writes `plans/TASKS.csv`. Rows whose file starts with `12-network-stack/` (44 tasks, all in `p1-tasks.md`).

| Priority | Effort S | Effort M | Effort L | Effort XL | Total |
|---|---|---|---|---|---|
| P0 | 10 | 6 | 0 | 0 | 16 |
| P1 | 9 | 7 | 1 | 0 | 17 |
| P2 | 4 | 3 | 1 | 0 | 8 |
| P3 | 3 | 0 | 0 | 0 | 3 |
| Total | 26 | 16 | 2 | 0 | 44 |

P0 tasks: T-12a-001, 002, 003, 004, 005, 006, 007, 008, 009, 010, 011, 012, 013, 014, 030, 036.
P1 tasks: T-12a-015, 016, 017 (L), 018, 019, 020, 021, 022, 023, 026, 029, 031, 034, 035, 041, 042, 043.
P2 tasks: T-12a-024, 025 (L), 027, 028, 032, 033, 039, 044.
P3 tasks: T-12a-037, 038, 040.

### 6.2 P0 critical path (in order)

1. T-12a-001 (NetError) and T-12a-002 (Request, Body, CacheControl) in parallel. Both depend only on area:01 (no other task).
2. T-12a-003 (NetworkClient, Interceptor trait, Chain). Depends on 001, 002 and area:01.
3. T-12a-004 (transport limits) and T-12a-008 (await, await_success, execute) in parallel. 004 depends on 003. 008 depends on 001, 002.
4. T-12a-005 (uncaught containment), T-12a-006 (UA; needs area:02), T-12a-007 (settings reader; needs area:02), T-12a-009 (cookie jar), T-12a-010 (rate limiter), T-12a-014 (toaster; needs T-01a-009).
5. T-12a-011 (refund and invalid host) after 010. T-12a-012 (progress), T-12a-013 (non-2xx and text), T-12a-036 (JSON) after 008.
6. T-12a-030 (construction order and restart semantics). Depends on 003 and 007. This is the task that builds the shared client the app uses for its first real request.

Longest chain: 001 or 002, then 003, then 010, then 011 (four levels); or 001 or 002, then 003, then 004, then 030 (four levels).

Note for the daily-use goal: P0 contains no Cloudflare handling and no cookie-import path. A source behind Cloudflare fails with `HTTP 403` or `HTTP 503` until P1 lands. See risk R2.

### 6.3 Risks

| # | Risk | Effect | Mitigation and owner |
|---|---|---|---|
| R1 | The rate limiter design in `p1-topic-rate-limiter.md` section 5 holds one `tokio::sync::Mutex` across the sleep. The cache-hit refund calls `self.window.lock().await` and waits for the sleeping waiter to release the lock. `refund.notify_waiters()` then wakes nobody useful, so a freed permit is not used until the head entry expires. Verified by reading the design code. | Extension or AniList requests stall longer than the limit requires after cache hits. Not a deadlock: entries expire. | Change T-12a-010 to the Kotlin-equivalent shape: a fair one-permit gate (`tokio::sync::Semaphore::new(1)`) for waiters plus a short-held `std::sync::Mutex` for the queue, the sleep done outside the queue lock, and the refund done under the short lock. Add the test `ratelimit_cache_hit_refunds_permit` with a waiter already sleeping. Owner: this area, before T-12a-010 is closed. |
| R2 | Cloudflare handling is P1. Cloudflare-protected sources fail in the P0 build. | Daily use blocked for those sources. | Decide Q2. If promoted, T-12a-015 and T-12a-016 and the hidden solver spike T-12a-017 move to P0. The visible-window fallback stays P2 unless the spike fails. |
| R3 | The hidden WebKitGTK solver (T-12a-017) might not complete on KDE Wayland when the window is unmapped or 1x1. Edge case in `p1-topic-cloudflare-and-webview.md` 3 item 8. | Solver never completes; T-12a-032 becomes P1. | Run the spike first, as the first act of T-12a-017. |
| R4 | Timeout semantics disagree between `p1-rust-design.md` 2.6 and `p1-edge-cases-and-tests.md` 2.7 (Q6). | A queued request could time out in the queue, or the design could differ from the test. | Decide Q6, then correct both documents and the test `dispatcher queued request does not time out` in `p1-edge-cases-and-tests.md`. |
| R5 | `Accept-Encoding` (Q4). The reqwest feature list in `p1-rust-design.md` section 6 adds brotli and zstd, which changes the wire header. | Behaviour differs from Aniyomi on the wire; a few servers may respond with br or zstd that Aniyomi never gets. | Follow Q4. Pin the header in a test. |
| R6 | Progress percentages above 100 with gzip (`p1-rust-design.md` 2.3). | Progress UI shows more than 100 per cent if not clamped. | Clamp at the consumer (T-12a-012 acceptance). |
| R7 | Dependency versions are not pinned in the plans: `webkit6` 0.6.x, `reqwest_cookie_store`, `hickory-proto`, `psl`, `http-cache-semantics`, `rquickjs`. | An API may differ at implementation time. | Verify the versions on crates.io when each task starts. The plan names only crates and features. |
| R8 | Cookie sync between reqwest's jar and WebKitGTK is manual (import and export on specific events). | Logged-in state can diverge between the WebView screen and the HTTP client. | Keep the rules in `p1-topic-cloudflare-and-webview.md` section 4. Add the round-trip test in `p1-edge-cases-and-tests.md` 4.3. |

### 6.4 Lint and dependency status

* `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/12-network-stack/p1-tasks.md` prints `ok`. No changes were needed.
* Every `area:` dependency token in `p1-tasks.md` already names a real area folder: `area:01-foundation-app-bootstrap` (5 uses), `area:02-preferences-system` (2), `area:20-extension-strategy-redesign` (1), `area:36-ui-shell-navigation-security-webview` (1), `area:47-settings-screens-general` (1). No token was changed.
* Task IDs were not changed. Every `T-` dependency resolves, including `T-req-platform-006` in `plans/addenda/requirements-cross-platform-tasks.md`.
* `tasks.py export` reports four errors outside this area: `40-ui-browse-manga/p1-tasks.md` lines 37, 83, 158 and 537 use `area:41-ui-browse-manga-part-2`, which does not exist. Not fixed here (another area's file).

## 7. Definition of done for this area in Kurobayomi

Each item is observable by the user or by a test named in `p1-edge-cases-and-tests.md` section 4.2.

1. A manga or anime source request succeeds through one shared client. The request carries `User-Agent` equal to the trimmed `default_user_agent` setting, or to the Firefox default `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0`.
2. Errors show the texts of section 2 of `p1-ui-parity.md`: `HTTP <code>, check website in WebView`, `No Internet connection`, `Couldn't reach <host>`, `Failed to bypass Cloudflare`. No raw Rust `Debug` text reaches the UI.
3. A `403` or `503` response with `Server: cloudflare` or `cloudflare-nginx` triggers the solver. With WebKitGTK installed, the solver returns when `cf_clearance` changes. The request is retried once. Without WebKitGTK, the toast `WebKitGTK is required to solve Cloudflare checks. Install webkitgtk-6.0.` shows and the original response is returned.
4. Twenty parallel requests to one Cloudflare-protected host produce one solve and twenty retries.
5. A cancelled download or list fetch stops within one scheduler tick (no waiting for the 30 s solve timeout).
6. Cookies set by a site persist across an app restart in `<data dir>/cookies.json` with mode `0600`. Settings > Advanced > Networking > Clear cookies removes them and shows the toast `Cookies cleared`.
7. Settings > Networking > DNS over HTTPS offers Disabled plus the 13 providers. Choosing one shows `Requires app restart to take effect`. After restart, lookups go through the chosen provider (manual check in `p1-edge-cases-and-tests.md` 4.4 item 3, `doh_provider = 2` resolves `example.org` through `dns.google`). With DoH on, `localhost` fails with `Couldn't reach localhost`, not a crash.
8. The default user agent dialog rejects a value containing `é` or a newline with `Invalid user agent string`, and accepts a tab-prefixed value after trimming.
9. The rate limiter enforces `85` requests per `60 s` for the AniList client. A cache hit gives its permit back immediately, including while another request waits (test `ratelimit_cache_hit_refunds_permit`).
10. Verbose logging (when on) writes HTTP lines to the log file with `Cookie` and `Authorization` values replaced by `<redacted>`.
11. A JavaScript source snippet that returns an object raises `UnsupportedResult("object")`, not a crash. A script that loops forever stops after 30 s.
12. Clear WebView data empties the WebKitGTK data directory and shows `WebView data cleared`. On failure it shows `Error occurred while clearing`.
13. The 5 MiB disk cache (P2) serves a repeated chapter-list request inside the freshness window without a network call, with freshness capped at 600 s for requests that use the default cache control.
14. Every test named in `p1-edge-cases-and-tests.md` section 4.2 exists under `crates/kuro-net/tests/` and passes in CI, with the WebKit integration tests gated by `KURO_WEBKIT_TESTS=1`.
