# Topic: Cloudflare interceptor, WebView solving and cookie sharing

Sources: `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt` (138 lines), `.../interceptor/WebViewInterceptor.kt` (113), `core/common/src/main/java/eu/kanade/tachiyomi/network/AndroidCookieJar.kt` (54), `core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt` (116), `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt`. Related, outside this area: `app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt` (area:36-ui-shell-navigation-security-webview), `plans/addenda/requirements-cross-platform.md` (the `WebViewHost` trait rule).

## 1. OBSERVED flow, exact order

Actors: the OkHttp dispatcher thread running the interceptor chain ("caller thread"), and the Android main thread (WebView is main-thread-only).

1. Caller thread: `CloudflareInterceptor` is the third application interceptor. Its inherited `intercept(chain)` (`WebViewInterceptor.kt:53-69`) runs `chain.proceed(request)` first. Anything downstream (extension interceptors, rate limiter) has already run.
2. `shouldIntercept(response)`: `response.code in listOf(403, 503) && response.header("Server") in arrayOf("cloudflare-nginx", "cloudflare")`. `in` on an array uses `equals`: case-sensitive, whole-value. A `Server: Cloudflare` or `Server: cloudflare-nginx/1.2` value is NOT matched.
3. If not intercepted, the response is returned untouched (status 403/503 from a non-Cloudflare server reaches `awaitSuccess` which throws `HttpException`).
4. `WebViewUtil.supportsWebView(context)` false: `launchUI { context.toast(MR.strings.information_webview_required, Toast.LENGTH_LONG) }`, return the ORIGINAL response (still open, unread).
5. Touch `initWebView` (lazy once): no-op on MIUI or on Samsung with `Build.VERSION.SDK_INT == 31`, else `WebSettings.getDefaultUserAgent(context)` in `try/catch(Exception)`. Purpose: warm the WebView stack on the caller thread to limit main-thread blocking.
6. `CloudflareInterceptor.intercept(chain, request, response)`, inside `try`:
   1. `response.close()`.
   2. `cookieManager.remove(request.url, COOKIE_NAMES = ["cf_clearance"], maxAge = 0)`. See `p1-behavior.md` 5.1: for any cookie string where `cf_clearance` is not the first cookie, the unfiltered name has a leading space and the filter does not match, so nothing is removed; and a Domain-scoped `cf_clearance` is not the key that gets overwritten. In practice the old cookie often survives this call.
   3. `oldCookie = cookieManager.get(request.url).firstOrNull { it.name == "cf_clearance" }` (name and value only).
   4. `resolveWithWebView(request, oldCookie)`.
   5. `return chain.proceed(request)`: one retry through the downstream chain with the same `Request` object. No loop, no second Cloudflare check; if Cloudflare still answers 403/503 that response is returned and later becomes `HttpException(403|503)`.
7. `catch (e: CloudflareBypassException)`: `throw IOException(context.stringResource(MR.strings.information_cloudflare_bypass_failure), e)`. `catch (e: Exception)`: `throw IOException(e)`.

### 1.1 `resolveWithWebView` in detail (`CloudflareInterceptor.kt:57-131`)

* Local state: `latch = CountDownLatch(1)`, `webview: WebView?`, booleans `challengeFound`, `cloudflareBypassed`, `isWebViewOutdated` (all plain `var`s mutated from the main thread and read from the caller thread; the latch gives the happens-before edge for the final read).
* `origRequestUrl = originalRequest.url.toString()`, `headers = parseHeaders(originalRequest.headers)` (unsafe headers removed, first value per name).
* On the main executor: `webview = createWebView(originalRequest)` (`WebView(context)` + `setDefaultSettings()` + `settings.userAgentString = request.header("User-Agent") ?: defaultUserAgentProvider()`), then set a `WebViewClient`:
  * `onPageFinished(view, url)`:
    * `isCloudFlareBypassed()` = `cookieManager.get(origRequestUrl.toHttpUrl()).firstOrNull { it.name == "cf_clearance" }.let { it != null && it != oldCookie }`. If true: `cloudflareBypassed = true; latch.countDown()`.
    * Then, independently: `if (url == origRequestUrl && !challengeFound) latch.countDown()` ("the first request didn't return the challenge, abort").
  * `onReceivedError(view, request: WebResourceRequest, error: WebResourceError)`: only for `request.isForMainFrame`; `if (error.errorCode in ERROR_CODES) challengeFound = true else latch.countDown()`.
  * then `webview?.loadUrl(origRequestUrl, headers)`.
* Caller thread: `latch.awaitFor30Seconds()` (`await(30, SECONDS)`, result ignored).
* Cleanup on the main executor: `if (!cloudflareBypassed) isWebViewOutdated = webview?.isOutdated() == true`; `webview?.run { stopLoading(); destroy() }`. This runs asynchronously: the caller thread does not wait for it, so `isWebViewOutdated` read right after is racy (it can still be `false` when read).
* Failure path: if `!cloudflareBypassed`: `if (isWebViewOutdated) context.toast(MR.strings.information_webview_outdated, Toast.LENGTH_LONG)` (called from the caller thread; `Toast.makeText` off the main thread works only on a thread with a Looper, so on an OkHttp dispatcher thread this toast can throw `RuntimeException: Can't toast on a thread that has not called Looper.prepare()`, which the outer `catch (e: Exception)` turns into `IOException(e)` instead of the bypass-failure message); then `throw CloudflareBypassException()`.

### 1.2 Reading of the success and failure conditions (analysis, not proven by a run)

`WebResourceError.getErrorCode()` returns `WebViewClient.ERROR_*` constants, which are negative numbers (`ERROR_UNKNOWN = -1`, `ERROR_HOST_LOOKUP = -2`, ..., `ERROR_UNSAFE_RESOURCE = -16`). HTTP statuses such as 403 and 503 are delivered through `onReceivedHttpError`, which this class does not override. Therefore `error.errorCode in ERROR_CODES` cannot be true, `challengeFound` can never become true, and the first `onPageFinished` whose `url` equals the original URL counts the latch down immediately (before the challenge JavaScript has had time to finish), unless `cf_clearance` already differs. The Tachiyomi/Mihon original of this class overrode `onReceivedHttpError` with `errorResponse.statusCode in ERROR_CODES`; this Aniyomi snapshot has the WebResourceError variant. This is the probable reason the "Failed to bypass Cloudflare" error is commonly reported. The Rust port implements the intent of the original (challenge detection through the real HTTP status) and not the dead branch.

## 2. Proposal: Rust state machine

### 2.1 Constants

```rust
pub const CF_STATUS_CODES: [u16; 2] = [403, 503];
pub const CF_SERVER_VALUES: [&str; 2] = ["cloudflare-nginx", "cloudflare"];
pub const CF_COOKIE_NAMES: [&str; 1] = ["cf_clearance"];
pub const CF_SOLVE_TIMEOUT: Duration = Duration::from_secs(30);
pub const CF_COOKIE_POLL: Duration = Duration::from_millis(500);
pub const CF_MAX_PARALLEL_SOLVES: usize = 2;
```

`CF_SERVER_VALUES` match is case-insensitive in Rust (header values from Cloudflare are lower-case today; the relaxation removes a trivial miss and changes nothing else).

### 2.2 Interceptor (async)

```
intercept(chain):
  request = chain.request()
  response = chain.proceed(request).await?                 // downstream first, like Kotlin
  if !should_intercept(&response): return response
  if !webview.available():                                  // supportsWebView
      toaster.show_long(Key::InformationWebkitMissing)      // parity of information_webview_required
      return response                                       // original response, body intact
  drop(response)                                            // response.close()
  old = jar.get(url).find(name == "cf_clearance")
  jar.remove(url, Some(["cf_clearance"]))                   // really deletes (domain-scoped too)
  outcome = solver.solve_single_flight(domain_key(url), SolveRequest{...}, cancel).await
  match outcome:
    Cleared{cookies} => jar.import(url, cookies); return chain.proceed(request).await   // one retry
    NoChallenge | Failed(Timeout|LoadError) => {
        if webview.is_outdated() { toaster.show_long(Key::InformationWebviewOutdatedLinux) }
        return Err(NetError::CloudflareBypassFailed)        // message key information_cloudflare_bypass_failure
    }
    Failed(Cancelled) => return Err(NetError::Canceled)
    Failed(Unavailable) => (same as !available above)
```

`is_outdated()` is evaluated while the web view is still alive (inside `solve`, returned in the outcome as `outdated: bool`) so the Kotlin race on `isWebViewOutdated` does not exist. The toast is posted through the thread-safe `Toaster`, never from the network thread's own UI call.

### 2.3 `domain_key` and single flight

Concurrent chapter-page requests to one Cloudflare site all get 403/503 together. In Kotlin each creates its own WebView and blocks its own thread, and each also expires the cookie again. Rust: `SolveCoordinator` keeps `HashMap<String /* registrable domain, via the psl crate, else host */, Shared<SolveFuture>>`; the first caller starts the solve, others await the same future (all receive the same `SolveOutcome`) and each then retries its own request once. At most `CF_MAX_PARALLEL_SOLVES` (2) distinct domains solve at once; others queue FIFO (a `tokio::sync::Semaphore`).

### 2.4 `WebViewHost` trait (lives in `kuro-platform`; satisfies T-req-platform-006)

```rust
#[async_trait::async_trait]
pub trait WebViewHost: Send + Sync {
    fn available(&self) -> bool;                       // WebKitGTK 6.0 loadable and a display exists
    fn version_string(&self) -> String;                // WebViewUtil.getVersion: "<name> <version>"
    fn is_outdated(&self) -> bool;                     // major < MIN_WEBKITGTK (see 3.5)
    async fn solve_challenge(&self, req: SolveRequest, cancel: CancellationToken) -> SolveOutcome;
    async fn open_page(&self, req: PageRequest) -> Result<(), PlatformError>;   // area:36-ui-shell-navigation-security-webview WebView screen and tracker logins
    async fn clear_data(&self) -> Result<(), PlatformError>;                    // "Clear WebView data"
}

pub struct SolveRequest {
    pub url: Url,
    pub headers: Vec<(String, String)>,     // already filtered by safe_request_headers()
    pub user_agent: String,                 // request UA, else default UA
    pub cookies: Vec<Cookie<'static>>,      // jar cookies for the URL except cf_clearance
    pub old_clearance: Option<String>,      // value of the removed cf_clearance, for the "differs" test
    pub timeout: Duration,                  // CF_SOLVE_TIMEOUT
    pub cookie_names: &'static [&'static str],
}
pub enum SolveOutcome {
    Cleared { cookies: Vec<Cookie<'static>>, outdated: bool },   // every cookie WebKit holds for the registrable domain
    NoChallenge { outdated: bool },
    Failed { reason: SolveFailure, outdated: bool },
}
pub enum SolveFailure { Timeout, LoadError(String), Unavailable, Cancelled }
```

A browser-fallback implementation (no WebKitGTK, or Windows later) returns `Failed{Unavailable}` and the caller offers the manual import dialog (T-12a-033).

### 2.5 State machine inside the Linux implementation (runs on the GTK main thread)

States: `Loading` -> `ChallengeSeen` -> `Cleared`; terminal states `Cleared`, `NoChallenge`, `Failed(..)`.

Inputs and transitions (the deadline of `timeout` runs from creation of the web view):

| Event | Condition | Action |
|---|---|---|
| `load-changed(COMMITTED)` | the loading document URI equals the original URL ignoring the fragment | read `web_view.main_resource().response().status_code()`; if in `{403, 503}` set `challenge_seen = true` |
| `load-changed(FINISHED)` | any | run `check_cookie()`; if not cleared and `uri == original` and `!challenge_seen`: finish with `NoChallenge` (Kotlin: "first request didn't return the challenge, abort") |
| `CookieManager::changed` | any | `check_cookie()` |
| poll tick every 500 ms | any | `check_cookie()` (covers cookie writes without a signal) |
| `load-failed(uri, error)` | main frame (`uri == original` or the committed document) and `!challenge_seen` | finish with `Failed(LoadError(error.message()))` |
| deadline | none of the above finished | finish with `Failed(Timeout)` |
| `cancel` token fired | any | finish with `Failed(Cancelled)` |

`check_cookie()`: read `CookieManager::cookies(original_url)`; `cleared` iff a cookie named `cf_clearance` exists with `value != old_clearance`. On `cleared`: collect all cookies for the registrable domain, finish with `Cleared`.

Finish always: `stop_loading()`, drop the view, close the hidden window, and the ephemeral `NetworkSession`, then send the outcome through the oneshot. `outdated` is computed before teardown.

### 2.6 Why the 30 s budget stays

The Kotlin budget (30 s) covers a typical JS challenge (2 to 10 s). An interactive challenge ("Verify you are human", Turnstile) cannot finish inside a hidden view and times out in Aniyomi too. T-12a-032 adds the visible fallback.

## 3. Linux implementation notes (`kuro-platform::linux::webview`, crate `webkit6` 0.6.1 which depends on `gtk4 ^0.11`)

1. Dependency: Arch package `webkitgtk-6.0` (repository `extra`, version 2.54.1 at time of writing). Cargo feature `webview` pulls `webkit6`; without it the host returns `available() == false`.
2. Session: `webkit6::NetworkSession::new_ephemeral()` per solve (no disk writes, no leakage into later solves). `WebView::builder().network_session(&session).build()`.
3. Settings mapping of `WebView.setDefaultSettings()` (`WebViewUtil.kt:77-93`): `javaScriptEnabled = true` -> `Settings::set_enable_javascript(true)`; `domStorageEnabled = true` -> `set_enable_html5_local_storage(true)` and `set_enable_html5_database(true)`; viewport and zoom flags -> not needed in a desktop view (zoom is on by default; `set_zoom_text_only(false)`); `cacheMode = LOAD_DEFAULT` -> `NetworkSession` default; `acceptThirdPartyCookies` -> `CookieManager::set_accept_policy(CookieAcceptPolicy::Always)`.
4. User agent: `Settings::set_user_agent(Some(&req.user_agent))`.
5. Extra request headers: build a `URIRequest::new(url)`, `uri_request.http_headers()` returns the mutable `soup::MessageHeaders`; `append(name, value)` for each filtered header except `User-Agent` (taken from settings), then `web_view.load_request(&request)`.
6. Cookie injection: for each cookie in `req.cookies`, `soup::Cookie::new(name, value, domain, path, max_age)` -> `CookieManager::add_cookie(&cookie, None::<&Cancellable>, callback)` before the load; await all callbacks.
7. Version and outdated: `webkit6::functions::major_version()`, `minor_version()`, `micro_version()` (compile-time-independent runtime values). `version_string()` = `"WebKitGTK {major}.{minor}.{micro}"`. `is_outdated()` = `(major, minor) < (2, 42)` (WebKitGTK 2.42 shipped September 2023, the same timeframe as Chrome 118, the Aniyomi minimum). Proposal constant `MIN_WEBKITGTK = (2, 42)`.
8. Visibility risk: an unmapped `WebView` may throttle timers and rendering. The solver must put the view in a real `gtk::Window` that is presented 1x1 pixel, undecorated and non-focusable; the T-12a-017 acceptance includes a spike that proves a local fake challenge (see `p1-edge-cases-and-tests.md` fixture `cf_challenge.html`) completes with the window not visible to the user, and records the compositor behaviour on Wayland (KDE Plasma) and X11. If it does not work hidden, the fallback is the visible window of T-12a-032 used from the start.
9. `X-Requested-With`: WebKitGTK does not send it, so the package-name spoof (`App.kt:225-237`, `WebViewUtil.spoofedPackageName`) has no counterpart (T-12a-037, drop).

### 3.1 Threading

WebKit objects are used only on the GTK main thread. `solve_challenge` (called from tokio) does `glib::MainContext::default().invoke(move || { ... })` through the bridge of T-01a-009 and awaits a `tokio::sync::oneshot::Receiver`. Dropping the future (call cancelled) fires the `CancellationToken`, which the main-thread code observes through a `glib` source and finishes with `Cancelled`.

## 4. Cookie jar and WebKit: the sharing problem

Android: one process-wide Chromium store shared by WebView and OkHttp. Linux: the HTTP client is reqwest, the browser is WebKitGTK, two stores. Rules:

1. The jar is the source of truth for the HTTP client.
2. Before a solve and before opening a page in the WebView screen: `jar.export_for(url)` -> `CookieManager::add_cookie` into the view's session.
3. After a solve (on `Cleared`) and when the WebView screen reports navigation (`load-changed(FINISHED)`) or closes: `CookieManager::cookies(uri)` for the page URL and its registrable domain -> `jar.import(url, cookies)`; imported cookies replace jar cookies with the same (name, domain, path).
4. `soup::Cookie` -> `cookie_store` conversion maps: name, value, domain (leading dot means not host-only), path, secure, http_only, expires (`soup::Cookie::expires()` as `DateTime`, none for session), same-site.
5. Tracker logins and user cookie clearing (`WebViewActivity.clearCookies`) call `jar.remove(url, None)` and `WebViewHost::clear_cookies_for(url)`.

## 5. WebViewUtil replacement

| Kotlin member | Rust |
|---|---|
| `MINIMUM_WEBVIEW_VERSION = 118` | `MIN_WEBKITGTK = (2, 42)` |
| `isOutdated()` regex on UA | numeric check of runtime version |
| `getVersion` (`"how did you get here?"` fallback) | `"WebKitGTK x.y.z"` or `"WebKitGTK not available"` |
| `supportsWebView` | `WebViewHost::available()` = feature compiled, `webkit6` initialises (`webkit6::NetworkSession::default()` does not panic) and `gdk::Display::default().is_some()` |
| `getHtml` | `web_view.evaluate_javascript("document.documentElement.outerHTML", ...)` returns a `JSCValue`; use `to_str()` to get the raw string (the JSON-quoting of Android does not occur) |
| `setDefaultSettings` | section 3 item 3 |
| `spoofedPackageName`, `getInferredUserAgent` | dropped |

## 6. Failure and message matrix

| Situation | Kotlin result | Rust result |
|---|---|---|
| WebView unsupported | toast `information_webview_required`, original 403/503 response returned | same behaviour, toast text "WebKitGTK is required to solve Cloudflare checks. Install webkitgtk-6.0." (`information_webkit_missing`) |
| Challenge solved | retry succeeds | same |
| Challenge not solved in 30 s | `IOException("Failed to bypass Cloudflare")`, plus outdated toast when WebView major < 118 | `NetError::CloudflareBypassFailed` with the same message; outdated toast when WebKitGTK < 2.42 |
| First load shows no challenge | latch released at first `onPageFinished`, ends as failure | `NoChallenge`, ends as failure |
| Network error during the web view load | latch released, failure | `Failed(LoadError)`, failure |
| Retry still blocked | the 403/503 response is returned | same |
| Call cancelled while solving | the thread keeps blocking until the latch times out | solve is aborted at once |
| 20 concurrent requests blocked | 20 WebViews | one solve per domain |
