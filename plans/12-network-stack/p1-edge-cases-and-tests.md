# Area 12-network-stack, part 1: edge cases, failure modes and tests

Upstream tests: none of the 21 scope files has a `*Test.kt` in the repository (checked: no test source under `core/common/src/test` references the network package, the interceptors, cookies, DoH, rate limiting, WebView or `GLUtil`). The only test in `core/common` is `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt`, which is outside this area. Every test below is NEW and written for Kurobayomi, with the Kotlin behaviour as the oracle where one exists.

Conventions: "K" = observed Kotlin behaviour (cited), "R" = required Rust behaviour, "D" = deliberate divergence (listed in the Rust design or the topic files).

## 1. Edge cases by file

### 1.1 `AndroidCookieJar.kt`

| # | Case | K | R |
|---|---|---|---|
| C1 | `remove(url, ["cf_clearance"], 0)` when `cf_clearance` is the second cookie | name is `" cf_clearance"` with a leading space, filter misses, nothing removed (`AndroidCookieJar.kt:44-47`) | removed (D: trimmed names) |
| C2 | `remove` on a domain-scoped `cf_clearance` (`Domain=.example.org`) | not removed (different Chromium key) | removed |
| C3 | `get` with a cookie string containing an entry without `=` | `Cookie.parse` returns null, entry dropped | same: entry skipped |
| C4 | `get` for a URL with `Secure` cookies over `http` | not returned by Chromium | not returned (RFC 6265) |
| C5 | corrupt `cookies.json` | not applicable (Chromium DB) | renamed to `cookies.json.corrupt`, empty store, warning |
| C6 | `remove_all` while a solve is writing `cf_clearance` | async, callback ignored | single mutex: the solver's import happens after remove, or both serialise; the final state contains the solver's cookies only if the import came later |
| C7 | expiry: cookies with `Max-Age=0` from a response | Chromium deletes | the store deletes (cookie_store semantics) |
| C8 | `saveFromResponse` with `expires` in the past | Chromium drops | dropped |
| C9 | file mode of `cookies.json` | Chromium DB in app private dir | `0600` |
| C10 | write debounce vs crash | Chromium writes immediately | up to 2 s of cookies lost on crash (D, documented) |

### 1.2 `DohProviders.kt` and `NetworkHelper.kt:42-57`

| # | Case | K | R |
|---|---|---|---|
| D1 | `doh_provider = 0`, `14`, `-1` | system DNS | system DNS |
| D2 | `localhost` with DoH on | `UnknownHostException("private hosts not resolved")` | `NetError::UnknownHost` with the same reason in `Display` (D: no crash) |
| D3 | `nas` single-label name | same as D2 | same as D2 |
| D4 | DoH server responds with only AAAA (A query fails) | returns IPv6 addresses | same |
| D5 | Both queries fail | `UnknownHostException` | `UnknownHost` |
| D6 | response with `Content-Length: 70000` | `IOException("response size exceeds limit (65536 bytes): 70000 bytes")` | same message |
| D7 | chunked response of 70000 bytes | read without limit | read capped at 65536, same error (D) |
| D8 | answer contains a private IP (10.0.0.5) | accepted | accepted (OkHttp does not filter IP answers; `resolvePrivateAddresses` concerns hostnames only) |
| D9 | `Mullvad` URL with leading space | parsed OK | Rust literal has no space |
| D10 | DoH endpoint host unreachable on all bootstrap IPs | `UnknownHostException` after trying each address | `UnknownHost { host = provider host }` |

### 1.3 `NetworkHelper.kt`

| # | Case | K | R |
|---|---|---|---|
| N1 | `default_user_agent` set to `""` through a hand-edited file | empty `User-Agent` header added (UA interceptor inserts an empty value) | no header inserted, warning once (D) |
| N2 | `default_user_agent` with a non-ASCII character | `Headers.add` throws `IllegalArgumentException` at the time the UA is applied | rejected at the settings layer; at request time the header is skipped with a warning (D: no crash) |
| N3 | `verbose_logging` changed at runtime | applied at next start | same |
| N4 | `new_builder` after the client is built | copies the chain; the original client unchanged | same |
| N5 | cache directory missing | created by OkHttp | created by the cache module |
| N6 | cache directory corrupt | OkHttp's journal recovers or the cache is unusable (logs) | the cache is disabled for the session with a warning |
| N7 | two `NetworkHelper` instances in one process | Kotlin: DI singleton | Rust: one instance, passed through `AppContext` |
| N8 | `callTimeout` of 2 min reached on a large body | `InterruptedIOException("timeout")` | `NetError::Timeout` |
| N9 | Verbose logging prints a `Cookie` header | HEADERS level logs them | D: `Cookie` and `Authorization` values redacted |

### 1.4 `Requests.kt` and `OkHttpExtensions.kt`

| # | Case | K | R |
|---|---|---|---|
| R1 | `GET(" https://x.test ")` | trimmed by `toHttpUrl` | trimmed (`Url::parse` after `trim_matches(is_ascii_whitespace)`) |
| R2 | `GET("ftp://x")` | `toHttpUrl` returns null, `IllegalArgumentException` | `NetError::InvalidUrl("unsupported scheme ftp")` |
| R3 | `POST("ws://x")` | rewritten to `http` | rewritten to `http` |
| R4 | `.headers(h)` then `.cacheControl(c)` where `h` has `Cache-Control` | the cache argument wins | same |
| R5 | `cache = CacheControl.FORCE_NETWORK` | header `no-cache` | same |
| R6 | `awaitSuccess` on 304 | `HttpException(304)` | `Http { code: 304 }` |
| R7 | `awaitSuccess` on 103 (early hints) | `HttpException(103)`; `MangaScreenModel` ignores it | `Http { 103 }`; the caller ignores it, same rule (area 15) |
| R8 | cancellation after the response arrives but before the caller reads it | response closed (`resume` onCancellation) | response dropped, connection released |
| R9 | JSON with `"x": null` for a non-nullable field | fails (`explicitNulls = false` treats it as absent, then missing field error) | serde: missing field error unless `#[serde(default)]` |
| R10 | JSON with unknown key | ignored | ignored |
| R11 | JSON body that is not UTF-8 | kotlinx decodes as UTF-8, errors on malformed bytes | `serde_json` errors on invalid UTF-8 (same) |
| R12 | progress call with unknown length | `contentLength = -1`, `Page.update` gives progress -1 | same |
| R13 | progress call where the server sends more than Content-Length | counted, update called | counted (D: over 100 % clamped by the consumer) |

### 1.5 Interceptors

| # | Case | K | R |
|---|---|---|---|
| I1 | extension throws `NullPointerException` in an interceptor | `UncaughtExceptionInterceptor` wraps into `IOException(npe)` | `NetError::Other("...")` |
| I2 | `Error` (OOM) inside an interceptor | not caught | panic caught by `catch_unwind` and converted (D: process survives) |
| I3 | `UserAgentInterceptor`, two `User-Agent` headers, last one empty | replaced | replaced |
| I4 | `UserAgentInterceptor`, two headers, last non-empty | untouched | untouched |
| I5 | Cloudflare response with `Server: Cloudflare` (capital C) | not intercepted | intercepted (D: case-insensitive value) |
| I6 | Cloudflare 403 from a non-Cloudflare server | returned, `HttpException(403)` | same |
| I7 | Cloudflare 503 with `Server: cloudflare-nginx` | intercepted | intercepted |
| I8 | WebView unsupported | toast, original response returned | toast, original response returned |
| I9 | `isRequestHeaderSafe("Connection", "upgrade")` | unsafe | unsafe |
| I10 | `isRequestHeaderSafe("Cookie", "a=b")` | safe | safe |
| I11 | `isRequestHeaderSafe("Proxy-Authorization", "x")` | unsafe (prefix) | unsafe |
| I12 | `rateLimitHost("not a url", 1)` | host null: limits every host (K quirk) | constructor error; extension gets `InvalidUrl` (D) |
| I13 | `rate_limit(0, ...)` | `NoSuchElementException` at first request | rejected at construction |

### 1.6 `RateLimitInterceptor.kt` and `SpecificHostRateLimitInterceptor.kt`

| # | Case | K | R |
|---|---|---|---|
| L1 | 85 permits per minute, 85 quick requests then one more | waits until the oldest expires | same, verified with paused tokio time |
| L2 | cancel while waiting | next wake-up checks `isCanceled`, throws `IOException("Canceled")` | returns `Canceled` immediately (D: faster) |
| L3 | cache hit | permit returned | permit returned |
| L4 | cache hit with an entry already pruned | `timestamp < first` returns, nothing removed | nothing removed |
| L5 | `period = Duration.ZERO` | `rateLimitMillis = 0`, no waiting, `first <= now` always true | same |
| L6 | host limit with redirect | one permit | one permit |

### 1.7 `CloudflareInterceptor.kt` and `WebViewInterceptor.kt`

| # | Case | K | R |
|---|---|---|---|
| W1 | solve succeeds on first page finish | `cloudflareBypassed`, retry | same |
| W2 | page without challenge (200 OK, no cf_clearance) | latch released at first finish, failure | `NoChallenge`, failure |
| W3 | challenge never finishes | 30 s timeout, failure | `Timeout` at 30 s |
| W4 | network error on the first load | latch released (`onReceivedError` main frame, error code not in 403/503) | `Failed(LoadError)` |
| W5 | retry still returns 403 | response returned, `HttpException(403)` | same |
| W6 | `cf_clearance` value equal to the old one | not bypassed (`it != oldCookie` compares name, value, domain, path) | not bypassed (compares value) |
| W7 | 20 parallel requests to one Cloudflare site | 20 WebViews, 20 solves | one solve, 20 retries |
| W8 | cancellation during the solve | thread blocked until timeout | solve aborted immediately (D) |
| W9 | WebKit crashed in the solver | not handled (WebView renderer crash) | `Failed(LoadError("web process terminated"))` (D) |
| W10 | `WebViewUtil.getVersion` with no WebView package | `"how did you get here?"` | `"WebKitGTK not available"` |
| W11 | `isOutdated` with WebKitGTK 2.40 | outdated when Chrome < 118 | outdated when `(2,40) < (2,42)` |
| W12 | retry after a successful solve reads the new cookie | yes (CookieManager is shared) | yes (jar import happens before retry) |

### 1.8 `util/system/*`

| # | Case | K | R |
|---|---|---|---|
| U1 | `toast(null)` | empty toast | ignored (D) |
| U2 | 6 toasts in one second | Android queues all | 5 kept, 6th dropped (D) |
| U3 | `CUSTOM_TEXTURE_LIMIT_OPTIONS` for 16384 | 16384, 15360, ..., 2048 (15 values) | same vectors (section 2.5) |
| U4 | `CUSTOM_TEXTURE_LIMIT_OPTIONS` for 3000 | 3000, 2048 | same |
| U5 | `DeviceUtil.isLowRamDevice` with 2.9 GiB | true | true (T-01a-030) |
| U6 | `miuiMajorVersion` for `V12.5.3.0.QFGMIXM` | 12 | not ported (dropped) |

## 2. Race conditions and concurrency hazards

1. Cloudflare `isOutdated` read races the main-thread teardown (`CloudflareInterceptor.kt:111-119`). Rust: `outdated` is computed inside the solve before teardown and returned with the outcome.
2. `isWebViewOutdated` and `cloudflareBypassed` are plain `var`s shared between threads (`CloudflareInterceptor.kt:64-66`). Rust: owned by the state machine on the GTK thread; the outcome is moved through a oneshot.
3. Two requests hit one Cloudflare site and both expire `cf_clearance` (`cookieManager.remove`): the second expiry can delete the cookie the first solve just stored. Rust: single flight per domain (topic section 2.3) so only one remove happens per solve.
4. Rate limiter waiters: a cache-hit notify can arrive before a waiter starts waiting. Rust: the waiter checks the queue under the lock before sleeping (section 5 of the rate-limiter topic).
5. Cookie jar writes: `remove_all` racing with a debounced save. Rust: the save takes a snapshot under the store lock, and `remove_all` sets `dirty` and writes synchronously; the later debounced write writes the empty snapshot.
6. DoH A and AAAA results are appended in arrival order. Tests must compare sets, not sequences.
7. `Dispatcher` limits: 64 total and 5 per host. A slow host with 5 in-flight calls delays a 6th call without a timeout started (Kotlin). Rust: the semaphore wait is inside the call timeout only after acquisition; the 2 min call timeout starts when the request is dispatched (OkHttp starts `callTimeout` at `enqueue`). Divergence recorded in the rust design section 2.6; tests check that a queued request does not time out while waiting for a permit.
8. The WebKit solver is started from `tokio` and must run on the GTK thread: a test asserts `glib::MainContext::is_owner()` inside the callback.

## 3. Upstream behaviour not reproduced (intentional)

* RxJava `asObservable` / `asObservableSuccess` (dropped, T-01a-038).
* `spoofedPackageName` and the `X-Requested-With` replacement (dropped, T-12a-037).
* `getInferredUserAgent` (no callers, dropped, T-12a-037).
* Vendor ROM probes in `DeviceUtil` (dropped, T-12a-038).
* `dpToPx` (dropped, T-12a-040).
* `cloudflareClient` deprecated alias: not carried over (Kurobayomi has no extension that calls it; the alias would only obscure which client is used). If an extension needs it, `NetworkClient` is the answer.

## 4. Test plan (new tests)

Each test name is a `#[test]` or `#[tokio::test]` in `crates/kuro-net/tests/`. Fixtures are under `crates/kuro-net/tests/fixtures/`.

### 4.1 Fixtures

| File | Content | Used by |
|---|---|---|
| `fixtures/cf_challenge.html` | a minimal page that, after 2 s of JavaScript, sets `document.cookie = "cf_clearance=test-<random>; path=/"` and then redirects to `/ok` | W1, W3, T-12a-017 spike |
| `fixtures/cf_challenge_503.http` | raw response: `HTTP/1.1 503 Service Unavailable`, `Server: cloudflare`, `Content-Type: text/html`, body = `cf_challenge.html` | W1 |
| `fixtures/cf_403_no_server.http` | `403` with no `Server` header | I6 |
| `fixtures/doh_a_example.bin` | DNS wire response for `example.test` A = `93.184.216.34` (generated with hickory-proto in a test helper, stored as bytes) | D4, D9 |
| `fixtures/doh_aaaa_example.bin` | AAAA = `2606:2800:220:1:248:1893:25c8:1946` | D4 |
| `fixtures/doh_oversize.bin` | 70 000 zero bytes served with `Content-Length: 70000` | D6 |
| `fixtures/cookies_corrupt.json` | `{"not": "a cookie store"` | C5 |
| `fixtures/cookies_v1.json` | a file written by the Rust store (round-trip test) | C2, C9 |
| `fixtures/glutil_vectors.json` | `[{"max": 16384, "options": [16384,15360,14336,13312,12288,11264,10240,9216,8192,7168,6144,5120,4096,3072,2048]}, {"max": 8192, "options": [8192,7168,6144,5120,4096,3072,2048]}, {"max": 4096, "options": [4096,3072,2048]}, {"max": 3000, "options": [3000,2048]}, {"max": 2048, "options": [2048]}]` (derived from the Kotlin algorithm, `GLUtil.kt:52-62`, and the examples in `p1-behavior.md` 11.4) | U3, U4 |
| `fixtures/safe_headers.json` | table of (name, value, expected) for I9 to I11 and the list in `WebViewInterceptor.kt:102-112` | I9-I11 |
| `fixtures/ua_values.json` | UA strings: valid Firefox UA (accepted), `"Mozillaé"` (rejected), `"a\nb"` (rejected), `"\tTab"` (accepted, trimmed to `Tab`) | section 5 of the UI parity file, T-12a-031 |

### 4.2 Tests

| Test | Covers |
|---|---|
| `cookie_remove_matches_first_and_second_cookie` | C1 (expected: removed) |
| `cookie_remove_domain_scoped` | C2 |
| `cookie_load_corrupt_renames_and_starts_empty` | C5 |
| `cookie_save_is_atomic_and_0600` | C9 (Unix only; gated by `cfg(unix)` inside the test, not in production code) |
| `cookie_debounce_writes_once_after_quiet_period` | C10: with `tokio::time::pause`, 10 changes in 1 s produce one write after 2 s |
| `cookie_remove_all_writes_immediately` | C6 |
| `doh_rejects_single_label_when_private_disallowed` | D2, D3 |
| `doh_merges_a_and_aaaa_as_sets` | D4 |
| `doh_fails_when_both_queries_fail` | D5 |
| `doh_rejects_declared_oversize_body` | D6 |
| `doh_caps_chunked_body` | D7 |
| `doh_query_is_base64url_without_padding` | section 3 of the DoH topic: query string has no `=` |
| `doh_provider_from_pref_maps_1_to_13_and_others_to_none` | D1 |
| `doh_client_never_uses_doh_for_itself` | section 4 item 7 of the DoH topic: mock DNS counts zero calls for the provider host |
| `ua_last_header_wins` | I3, I4 |
| `ua_empty_provider_inserts_nothing` | N1 |
| `ua_rejects_invalid_chars_via_header_value` | fixture `ua_values.json` |
| `safe_headers_table` | I9-I11, fixture `safe_headers.json` |
| `cf_detects_503_with_cloudflare_server` | I7 |
| `cf_ignores_capitalised_server_value_as_in_kotlin_or_matches_d` | I5 (D: matches) |
| `cf_single_flight_for_20_parallel_requests` | W7 (the solver mock counts 1 solve) |
| `cf_retry_once_then_return_403` | W5 |
| `cf_timeout_at_30s_with_paused_time` | W3 |
| `cf_no_challenge_ends_immediately` | W2 |
| `cf_load_error_maps_to_failure` | W4 |
| `cf_same_cookie_value_is_not_bypassed` | W6 |
| `cf_cancel_aborts_solve_immediately` | W8 |
| `cf_toast_when_webkit_unavailable_returns_original_response` | I8 |
| `ratelimit_85_per_minute_waits_for_oldest` | L1 (paused time) |
| `ratelimit_cancel_while_waiting_returns_canceled` | L2 |
| `ratelimit_cache_hit_refunds_permit` | L3 |
| `ratelimit_zero_permits_rejected` | I13 |
| `ratelimit_host_only_counts_matching_host` | section 3 of the rate-limit topic |
| `ratelimit_invalid_host_string_is_error` | I12 |
| `request_trims_url_whitespace` | R1 |
| `request_rejects_non_http_scheme` | R2 |
| `request_post_rewrites_ws_scheme` | R3 |
| `request_cache_argument_overrides_header` | R4 |
| `response_304_is_http_error_in_await_success` | R6 |
| `progress_reports_done_on_end_of_stream` | section 2.3 of the rust design |
| `progress_unknown_length_reports_minus_one` | R12 |
| `json_ignores_unknown_fields` | R10 |
| `json_null_on_non_optional_is_error` | R9 |
| `glutil_option_vectors` | U3, U4, fixture `glutil_vectors.json` |
| `low_ram_threshold_three_gib` | U5 |
| `uncaught_panic_becomes_other_error` | I1, I2 |
| `timeout_error_maps_to_timeout_variant` | N8 |
| `js_unsupported_result_type` | section 5 of the rust design (object result) |
| `js_memory_limit_aborts` | section 5 |
| `js_interrupt_after_30s` | section 5 (paused time not applicable; use a 2 s test limit behind a config constant) |

### 4.3 Integration tests (run in CI with a local HTTP server, no internet)

* A local `hyper` server serving `cf_challenge_503.http` then `ok` after a cookie is set: end-to-end through `NetworkClient` with the real WebKit solver, behind the feature `webview` and an env var `KURO_WEBKIT_TESTS=1` (requires a display; run under `xvfb-run` in CI).
* DoH end-to-end against a local DoH server that speaks RFC 8484 GET (serves the fixtures).
* Cookie round trip: set cookie through a response, restart the jar from disk, send request, server sees the cookie.

### 4.4 Manual acceptance checks (user machine, KDE Plasma)

1. Hidden solver on Wayland: solve a real Cloudflare-protected site; confirm no visible window and the request completes (T-12a-017).
2. Same on X11.
3. Settings > Networking: change DoH, restart, confirm `requires_app_restart` toast and the resolver change (for example `doh_provider = 2` resolves `example.org` through `dns.google`).
4. Clear cookies: a logged-in tracker session is lost on the next request.
5. UA change plus restart: the request header shows the new value (`nc -l` or a request inspector).

## 5. Fixtures and data needed from outside this area

* `hickory-proto` for building DNS fixtures (dev-dependency only).
* `webkit6` and a running display for the WebKit integration tests (CI image with `xvfb`).
* A Cloudflare-protected test site cannot be used in CI (no network); the manual check in 4.4 covers it.
