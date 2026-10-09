# Area 12-network-stack, part 1: implementation tasks

Priority is set by the rule in the project brief. P0 is needed for daily reading and watching (client, cookies, UA, timeouts, errors, JSON, rate limiting, toasts, construction). Cloudflare handling is P1: it is needed only for sources behind Cloudflare, and the hidden-solver spike (T-12a-017) decides whether it can be P0 for the user's sources. DoH is P2 and opt-in (default off in Aniyomi). Every file in the scope has at least one task, either an implementation task or a P3 drop task (see `p1-inventory.md`).

Dependencies on other areas use `area:<id>`; cross-platform rule tasks are named as in `plans/addenda/requirements-cross-platform-tasks.md`.

## Foundations

### T-12a-001 — NetError enum and HTTP status error
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt
- Acceptance:
  - `NetError::Http { code: 404 }` displays exactly `HTTP error 404`, the same text as `HttpException(404).message`.
  - Every variant listed in `p1-rust-design.md` section 2.1 exists and compiles in `crates/kuro-net/src/error.rs`.
  - `NetError::user_message(online)` returns the keys listed in `p1-behavior.md` section 9 (unit test over all variants).

### T-12a-002 — Request, Body, CacheControl and method helpers
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt
- Acceptance:
  - `GET`, `POST`, `PUT`, `DELETE` builders exist in `crates/kuro-net/src/request.rs`; default cache control serialises to `max-age=600` (test).
  - `CacheControl::FORCE_NETWORK` serialises to `no-cache`; `FORCE_CACHE` to `only-if-cached, max-stale=2147483647`; an empty cache control sends no `Cache-Control` header.
  - `POST("ws://x")` rewrites the scheme to `http`; `GET("ftp://x")` returns `NetError::InvalidUrl` (test vectors from `p1-edge-cases-and-tests.md` R1 to R3).
  - An explicit `cache` argument overrides a `Cache-Control` header passed in `headers` (test R4).

### T-12a-003 — NetworkClient, interceptor trait and chain
- Priority: P0
- Effort: M
- Depends: T-12a-001, T-12a-002, area:01-foundation-app-bootstrap
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - `NetworkClient` is `Clone` and cheap (an `Arc`); `new_builder()` copies the interceptor list and leaves the original unchanged (test N4).
  - `Interceptor::intercept` runs in insertion order; `Chain::proceed` is called once per `execute` even when the request is redirected (test with a 302 chain).
  - Calling `execute` on a `NetworkClient` built with no interceptors reaches the transport.

### T-12a-004 — Transport limits (timeouts, redirects, pool, dispatcher)
- Priority: P0
- Effort: S
- Depends: T-12a-003
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - The transport is built with connect timeout 30 s, read timeout 30 s, call timeout 120 s, redirect limit 20, idle pool timeout 300 s, max idle per host 5 (values checked by a unit test that reads the builder config struct).
  - A semaphore set allows at most 64 concurrent requests overall and 5 per host; a 6th request to one host waits without its call timeout running (test with paused time).
  - Requests to a server that accepts the TCP connection but never responds fail with `NetError::Timeout` after the read timeout.

### T-12a-005 — Uncaught exception containment interceptor
- Priority: P0
- Effort: S
- Depends: T-12a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/UncaughtExceptionInterceptor.kt
- Acceptance:
  - It is the first application interceptor in `crates/kuro-net/src/interceptors/uncaught.rs`.
  - A panic in a later interceptor becomes `NetError::Other` and the process keeps running (test I1, I2).
  - `NetError::Canceled` and `NetError::Timeout` pass through unchanged.

### T-12a-006 — Default user agent: setting and UserAgentInterceptor
- Priority: P0
- Effort: S
- Depends: T-12a-003, area:02-preferences-system
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/UserAgentInterceptor.kt
- Acceptance:
  - Request with no `User-Agent` receives the trimmed `default_user_agent`; default value is `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0`.
  - A request whose last `User-Agent` value is non-empty is untouched (tests I3, I4).
  - An empty trimmed provider inserts no header and logs one warning (test N1).

### T-12a-007 — Network settings read and typed accessors
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - `NetworkSettings` reads keys `verbose_logging` (default false, true in debug builds), `doh_provider` (default -1), `default_user_agent` (default as in T-12a-006) from `Settings.network` (owned by area 02, keys in `p1-data-and-config.md` section 2).
  - A preference file missing the keys yields exactly the defaults above (test).
  - The legacy key `enable_doh` is not read by this area (area 11 migration owns it).

### T-12a-008 — Await, await_success and request execution
- Priority: P0
- Effort: M
- Depends: T-12a-001, T-12a-002
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt
- Acceptance:
  - `NetworkClient::get` returns a successful response or `NetError::Http { code }` for any status outside 200..=299 and closes the body first (test R6, 304 is an error here).
  - Dropping the returned future cancels the in-flight request (test: server sees connection closed).
  - Error wrapping keeps the caller's origin for diagnostics: a `NetError::Io` carries the original `io::Error` source.

### T-12a-009 — Cookie jar with JSON persistence
- Priority: P0
- Effort: M
- Depends: T-12a-003, area:01-foundation-app-bootstrap
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/AndroidCookieJar.kt
- Acceptance:
  - `CookieJar::load` on `cookies.json` restores persistent cookies only; session cookies are not written (test).
  - A corrupt file is renamed to `cookies.json.corrupt` and the store starts empty; a warning is logged (test C5).
  - Writes are debounced 2 s after the last change, atomic (temp then rename), file mode 0600 (tests C9, C10).
  - `get`, `remove(url, names)` and `remove_all` behave as in `p1-rust-design.md` section 2.7; `remove` deletes domain-scoped cookies (test C1, C2).

### T-12a-010 — Rate limiter, global and per host
- Priority: P0
- Effort: M
- Depends: T-12a-003
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/SpecificHostRateLimitInterceptor.kt
- Acceptance:
  - 85 permits per 60 s: the 86th request waits until the oldest entry is 60 s old (paused-time test L1).
  - Host-scoped limiter ignores requests to other hosts (test in `p1-topic-rate-limiter.md` section 3).
  - Waiters are served in FIFO order (test with 3 waiters).
  - `rate_limit` and `rate_limit_host` builder methods exist for extensions (see T-12a-023).

### T-12a-011 — Rate limiter cache-hit refund and invalid host
- Priority: P0
- Effort: S
- Depends: T-12a-010
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt
- Acceptance:
  - A response with `from_cache == true` removes exactly its own timestamp and wakes waiters (test L3).
  - A cache hit whose timestamp already expired removes nothing (test L4).
  - `rate_limit(0, ...)` and an unparsable host string are rejected at construction (tests I12, I13).

### T-12a-012 — Download progress body and progress call
- Priority: P0
- Effort: M
- Depends: T-12a-008
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/ProgressListener.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/ProgressResponseBody.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt
- Acceptance:
  - `ProgressListener::update` is called after every chunk and once more with `done = true` at end of stream (test `progress_reports_done_on_end_of_stream`).
  - The progress call runs with cache disabled and a 30 h call timeout (`CallOptions.use_cache = false`, `timeout = 30 h`); a cached URL is downloaded again (test).
  - Unknown length reports `content_length == -1` (test R12).

### T-12a-013 — Non-2xx mapping and error text
- Priority: P0
- Effort: S
- Depends: T-12a-001, T-12a-008
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt
- Acceptance:
  - An `Http { code: 103 }` error is produced for 103 and documented as ignorable by callers (area 15 rule, `MangaScreenModel.kt:283-285`).
  - `user_message` for `Http { code: 503 }` gives key `exception_http` with argument `503` (text "HTTP 503, check website in WebView").
  - Redirect responses other than 304 never reach the error (followed by the transport).

### T-12a-014 — Toaster service
- Priority: P0
- Effort: M
- Depends: T-01a-009
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/ToastExtensions.kt
- Acceptance:
  - `Toaster::show` callable from any thread; the message appears in the main window overlay (manual check on KDE).
  - Short shows for 2000 ms, Long for 3500 ms (test with paused time); empty text is ignored (test U1).
  - At most 5 messages are queued; a 6th is dropped while 5 wait (test U2).

### T-12a-030 — Client construction order and restart semantics
- Priority: P0
- Effort: S
- Depends: T-12a-003, T-12a-007
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - `NetworkClient::build` creates the cookie jar, transport, DoH (when set), application chain and terminal stage in the order of `p1-rust-design.md` section 2.6.
  - Changing `doh_provider` or `verbose_logging` at runtime does not change the running client; the restart toast `requires_app_restart` is shown by the settings handler (T-12a-031).
  - `default_user_agent` changes apply to the next request without a restart (test).

### T-12a-036 — JSON response decoding
- Priority: P0
- Effort: S
- Depends: T-12a-008
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt
- Acceptance:
  - `Response::json::<T>()` decodes with serde: unknown fields ignored (test R10), null on non-optional fields is an error (test R9).
  - Bodies over 64 MiB fail with `NetError::Decode("response too large")` (test).
  - The body is consumed and closed in every path (test with a dropped body counter).

## Cloudflare and WebView

### T-12a-015 — Cloudflare challenge detection
- Priority: P1
- Effort: S
- Depends: T-12a-008
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt
- Acceptance:
  - A response is a challenge when status is 403 or 503 and `Server` equals `cloudflare-nginx` or `cloudflare` (tests I6, I7; case-insensitive value per D).
  - A 403 without `Server` is not intercepted (test).
  - Detection runs after downstream interceptors, as in Kotlin (test order).

### T-12a-016 — Challenge flow, single retry and error mapping
- Priority: P1
- Effort: M
- Depends: T-12a-015, T-12a-014, T-12a-009
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt
- Acceptance:
  - The flow of `p1-topic-cloudflare-and-webview.md` section 2.2 runs: remove `cf_clearance`, read the old value, solve, import the result, retry exactly once (tests W1, W5).
  - A failed solve returns `NetError::CloudflareBypassFailed` whose message is "Failed to bypass Cloudflare" (test W3).
  - `WebViewHost::available() == false` shows the toast `information_webkit_missing` and returns the original response unread (test I8).

### T-12a-017 — Hidden WebKitGTK challenge solver and spike
- Priority: P1
- Effort: L
- Depends: T-12a-016, T-req-platform-006
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - Spike first: a hidden 1x1 window with `webkit6` loads `cf_challenge_503.http` through a local server and sets `cf_clearance`; the result is recorded for KDE Wayland and X11 in `plans/12-network-stack/` notes (manual check, see `p1-ui-parity.md` section 3).
  - The solver implements the state machine of `p1-topic-cloudflare-and-webview.md` section 2.5 with a 30 s deadline, a 500 ms cookie poll and an ephemeral `NetworkSession`.
  - Solver finishes with `Cleared`, `NoChallenge`, `Failed(Timeout)`, `Failed(LoadError)` or `Failed(Cancelled)` as in the table of section 2.5; each is covered by a fixture-backed integration test under `KURO_WEBKIT_TESTS=1`.
  - If the spike fails, T-12a-032 is moved to P1 and this task records the failure.

### T-12a-018 — WebKit and cookie jar synchronisation
- Priority: P1
- Effort: M
- Depends: T-12a-009, T-12a-017
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/AndroidCookieJar.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - Before a solve or a WebView page open, `export_for(url)` cookies are added to the WebKit session (test with a local server reading the header).
  - After `Cleared` and after the WebView screen closes, WebKit cookies for the registrable domain are imported; same name, domain and path replaces the jar entry (test).
  - The conversion table of `p1-topic-cloudflare-and-webview.md` section 4 item 4 is implemented (test per field).

### T-12a-019 — WebView availability, version and outdated check
- Priority: P1
- Effort: S
- Depends: T-req-platform-006
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt
- Acceptance:
  - `available()` is false when the `webview` feature is off, when `webkit6` cannot initialise, or when no display exists (tests with each condition).
  - `version_string()` returns `WebKitGTK x.y.z`, or `WebKitGTK not available` (test W10).
  - `is_outdated()` is true for 2.40 and false for 2.42 (test W11).

### T-12a-020 — Safe request header filter
- Priority: P1
- Effort: S
- Depends: T-12a-016
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt
- Acceptance:
  - `safe_request_headers` drops exactly the names and rules of `p1-behavior.md` section 4.6, verified by `fixtures/safe_headers.json` (tests I9 to I11).
  - The first value per name is kept (test with repeated `Accept`).
  - `Cookie` is kept (test I10).

### T-12a-021 — Single flight per domain and solve concurrency cap
- Priority: P1
- Effort: M
- Depends: T-12a-017
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt
- Acceptance:
  - 20 parallel requests to one domain produce one solve and 20 retries (test W7).
  - The domain key is the registrable domain from the `psl` crate, falling back to the host when it has no public suffix.
  - At most 2 distinct domains solve at once; a third waits FIFO (test).

### T-12a-022 — Cancellation through the challenge flow
- Priority: P1
- Effort: S
- Depends: T-12a-021
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt
- Acceptance:
  - Cancelling the call aborts the solve within one GTK iteration and releases the WebKit view (test W8, deviation D recorded).
  - A waiter on a shared solve that is cancelled does not cancel the solve for the other waiters (test).
  - Cancelling a rate-limit wait returns `NetError::Canceled` without consuming a permit (test L2).

### T-12a-031 — Networking settings rows and dialogs
- Priority: P1
- Effort: M
- Depends: T-12a-006, T-12a-007, T-12a-035, T-12a-034, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - The Networking group shows rows 1 to 5 of `p1-ui-parity.md` section 4 in that order, with the subtitle of row 4 showing the quoted value.
  - UA dialog: OK disabled when blank or unchanged; invalid header characters show `Invalid user agent string` and keep the dialog (test `ua_values.json`).
  - DoH dropdown lists the 14 entries of `p1-data-and-config.md` section 3 plus Disabled; a change shows `Requires app restart to take effect`; the helper line about local names is shown (manual check).

### T-12a-032 — Visible challenge window
- Priority: P2
- Effort: M
- Depends: T-12a-017
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt
- Acceptance:
  - Visible mode window is 960 x 720 with the title `Cloudflare check`, the hint text of `p1-ui-parity.md` section 3, and a Cancel button; Escape cancels.
  - It closes on `Cleared`; the deadline is 120 s in visible mode (paused-time test).
  - A second request for the same domain while it is open shares the solve (test with T-12a-021).

### T-12a-033 — Manual cf_clearance import dialog
- Priority: P2
- Effort: S
- Depends: T-12a-009, T-12a-016
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/AndroidCookieJar.kt
- Acceptance:
  - When the solver is unavailable, the user can paste a `cf_clearance` value and the browser user agent; the cookie is stored for the domain with the entered user agent set as default for that domain.
  - Empty value is rejected; a value with `;` or spaces is rejected (test).
  - Strings `cookie_import_title` and `cookie_import_hint` are used (see `p1-data-and-config.md` section 9).

### T-12a-034 — Clear WebView data
- Priority: P1
- Effort: S
- Depends: T-12a-019, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - `clear_data` deletes the WebKit data and cache directories under `Paths` (never a literal path) and returns success (test with temp dirs).
  - Success shows `WebView data cleared`; failure shows `Error occurred while clearing` and logs at error level (test with an unwritable directory).
  - Cookies in the jar are not deleted by this action (only by T-12a-035), matching Kotlin where the WebView profile contains the cookie DB but the app cookie jar is the same store (documented D).

### T-12a-035 — Clear cookies action
- Priority: P1
- Effort: S
- Depends: T-12a-009
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/AndroidCookieJar.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - `remove_all` clears memory and writes an empty file immediately; a toast `Cookies cleared` appears (test C6).
  - After clearing, a request to a site that set a cookie sends no `Cookie` header (test).

### T-12a-041 — WebView HTML extraction and default settings
- Priority: P1
- Effort: S
- Depends: T-12a-017
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - `document.documentElement.outerHTML` is read as a raw string (no JSON quoting) (test with a page containing quotes and `\n`).
  - Settings applied to views: JavaScript on, local storage on, third-party cookies accepted, default cache policy (matches `p1-rust-design.md` settings table in `p1-topic-cloudflare-and-webview.md` section 3 item 3).

### T-12a-042 — Challenge integration tests with a fake server
- Priority: P1
- Effort: M
- Depends: T-12a-017
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt
- Acceptance:
  - A local `hyper` server serving `fixtures/cf_challenge_503.http` then a page returns success through `NetworkClient` (run under `KURO_WEBKIT_TESTS=1` with xvfb).
  - A server that never sets `cf_clearance` yields `CloudflareBypassFailed` after 30 s (paused or real time, documented).

### T-12a-043 — Retry semantics tests
- Priority: P1
- Effort: S
- Depends: T-12a-016
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt
- Acceptance:
  - The retry happens exactly once: a server that always answers 503 sees exactly two requests (test).
  - The second 503 is returned as `Http { code: 503 }` from `await_success` (test W5).

## Extension and client API

### T-12a-023 — Client builder for extensions: add_interceptor, rate_limit, rate_limit_host
- Priority: P1
- Effort: M
- Depends: T-12a-003, T-12a-010
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/SpecificHostRateLimitInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - `ClientBuilder::add_interceptor`, `rate_limit(permits, Duration)`, `rate_limit_host(url, permits, Duration)` exist; `rate_limit` default period is 1 s (test).
  - The deprecated Kotlin overloads with `Long` period and `TimeUnit` have no Rust counterpart; the Rust API takes `Duration` only (documented in the extension notes of area 20).
  - Each limiter instance has independent state (test: two builders, two limiters).

### T-12a-029 — Extension-facing network API surface
- Priority: P1
- Effort: M
- Depends: T-12a-008, T-12a-023, area:20-extension-strategy-redesign
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - The public surface in `kuro-net` re-exports: `get`, `post`, `execute`, `json`, `text`, `NetError`, `CacheControl`, `ProgressListener`, `rate_limit`, `rate_limit_host`, `add_interceptor` (compile test of an example extension crate).
  - `default_user_agent` is available to extensions through `NetworkClient::default_user_agent()` (test).
  - Documented in area 20 that the Kotlin `cloudflareClient` alias is not provided (see `p1-edge-cases-and-tests.md` section 3).

### T-12a-026 — Verbose HTTP logging
- Priority: P1
- Effort: S
- Depends: T-12a-007, T-01a-005
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - With `verbose_logging = true`, each request logs method, URL, status and headers at `debug` on target `kuro_net::http`; with false nothing is logged at that target (test with a tracing subscriber).
  - `Cookie`, `Set-Cookie` and `Authorization` values are logged as `<redacted>` (test N9).

## DNS over HTTPS and cache

### T-12a-024 — DoH provider table and preference mapping
- Priority: P2
- Effort: S
- Depends: T-12a-007
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt
- Acceptance:
  - The enum has 13 variants with ids 1..13 and the exact URLs and bootstrap addresses of `p1-data-and-config.md` section 3 (table test).
  - `from_pref(-1)`, `from_pref(0)`, `from_pref(14)` return `None` (test D1).
  - The Mullvad URL has no leading space (test D9).

### T-12a-025 — DoH resolver (RFC 8484 GET)
- Priority: P2
- Effort: L
- Depends: T-12a-024, T-12a-004
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt
- Acceptance:
  - Queries A and AAAA concurrently, query string `dns` is base64url without padding, header `Accept: application/dns-message` (tests D4, `doh_query_is_base64url_without_padding`).
  - A hostname with no public suffix fails with `private hosts not resolved` while DoH is on (test D2, D3).
  - Bodies over 65 536 bytes fail with `response size exceeds limit (65536 bytes): N bytes` (test D6, D7).
  - The provider host is resolved only from bootstrap addresses, never through DoH itself (test).

### T-12a-027 — HTTP disk cache (5 MiB)
- Priority: P2
- Effort: M
- Depends: T-12a-004
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt
- Acceptance:
  - Cache directory is `Paths::cache()/network_cache` with a 5 242 880 byte cap enforced by LRU eviction after each write (test: total size stays <= cap).
  - Freshness follows `p1-topic-http-cache.md` sections 2 and 3, including the 10 % heuristic and the 600 s request cap (corpus test).
  - Progress calls bypass the cache (test T-12a-012).

### T-12a-028 — JavaScript evaluation for sources
- Priority: P2
- Effort: M
- Depends: area:01-foundation-app-bootstrap
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/JavaScriptEngine.kt
- Acceptance:
  - `evaluate` returns `Null`, `Bool`, `Int`, `Double` or `Str` for the matching JS values (tests).
  - An object result returns `JsError::UnsupportedResult("object")` instead of a cast error (test).
  - A script that loops forever is aborted after 30 s and returns `JsError::Timeout` (test with a 2 s configured limit).
  - Memory above 64 MiB aborts with `JsError::MemoryLimit` (test).

## Drops and polish

### T-12a-037 — Drop WebView package spoofing and inferred user agent
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - Dropped: `spoofedPackageName` (replaces `X-Requested-With` on Android; WebKitGTK sends no such header) and `getInferredUserAgent` (no callers). Loss: none for Linux users; recorded in `p1-inventory.md`.
  - Grep test in CI finds no `X-Requested-With` rewrite in `crates/kuro-net`.

### T-12a-038 — Drop vendor ROM probes in DeviceUtil
- Priority: P3
- Effort: S
- Depends: T-01a-030
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - Dropped: `isMiui`, `miuiMajorVersion`, `isMiuiOptimizationDisabled`, `isSamsung`, `oneUiVersion`, `invalidDefaultBrowsers`. Loss: none on Linux; the MIUI file-picker warnings do not apply.
  - `isLowRamDevice` is provided by T-01a-030 with the 3 GiB threshold (test U5), not in this area.

### T-12a-039 — Texture limit option vectors
- Priority: P2
- Effort: S
- Depends: T-01a-022
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt
- Acceptance:
  - The option generator reproduces `fixtures/glutil_vectors.json` (tests U3, U4) including the 16384 case with 15 values.
  - `SAFE_TEXTURE_LIMIT` is 2048 and the device limit is `max(probed, 2048)` (test).
  - The GL probe itself is implemented by T-01a-022, not here.

### T-12a-040 — Drop dpToPx
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt
- Acceptance:
  - Dropped: `Int.dpToPx`. Loss: none; GTK lays out in logical pixels and the compositor applies the scale.
  - No Rust function multiplies by a display density in `crates/kuro-net` or `src/` (grep check).

### T-12a-044 — Settings helper for DoH and the local-name rule
- Priority: P2
- Effort: S
- Depends: T-12a-031, T-12a-025
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - While DoH is on, the Networking group shows the helper line "Local names such as localhost are not resolved while DoH is on." (manual check).
  - The helper is hidden when DoH is Disabled (test of the view state).
