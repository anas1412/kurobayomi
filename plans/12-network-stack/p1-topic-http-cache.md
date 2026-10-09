# Topic: HTTP response cache (5 MiB, `network_cache`)

Sources: `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:26-31` (cache construction), `.../network/Requests.kt:16` (default `Cache-Control`), `.../network/OkHttpExtensions.kt:119-132` (`newCachelessCallWithProgress`), and upstream OkHttp 5.4.0 `okhttp/src/commonJvmAndroid/kotlin/okhttp3/internal/cache/CacheStrategy.kt` (read during this review, tag `parent-5.4.0`).

## 1. OBSERVED configuration

* `Cache(directory = File(context.cacheDir, "network_cache"), maxSize = 5L * 1024 * 1024)` (5 242 880 bytes). `context.cacheDir` is Android's app cache dir, which the OS may clear.
* Storage engine: OkHttp `DiskLruCache` (journal file plus per-entry files, LRU eviction at 5 MiB).
* Only the shared client has it. `newCachelessCallWithProgress` calls `newBuilder().cache(null)`, so update and page downloads never read or write it.
* Stored only for GET (and HEAD). Any other method invalidates the cached entry for the same URL (OkHttp `HttpMethod.invalidatesCache`).
* Used by every source request. Covers and images go through Coil's own cache first, so OkHttp's cache mostly serves API and HTML pages.

## 2. OBSERVED storage rules (`CacheStrategy.kt`)

`isCacheable(response, request)` returns true only for these response codes, with the status-specific rules:

| Status | Rule |
|---|---|
| 200, 203, 204, 300, 301, 404, 405, 410, 414, 501, 308 | cacheable unless `no-store` |
| 302, 307 | cacheable only if the response has `Expires`, or `max-age`, or `public`, or `private` |
| anything else | never cached |

Then `!response.cacheControl.noStore && !request.cacheControl.noStore`.

## 3. OBSERVED freshness (`CacheStrategy.kt:157-256`)

1. `freshMillis = computeFreshnessLifetime()`:
   * response `max-age` if present; else
   * `Expires - servedDate` (or `receivedResponseMillis` when no `Date`), clamped to >= 0; else
   * heuristic: if `Last-Modified` exists AND the URL has no query string, `(servedMillis - lastModified) / 10` clamped to >= 0.
   * otherwise 0.
2. If the request has `max-age`, `freshMillis = min(freshMillis, requestMaxAge)` (line 171-172). The request `max-age=600` from `DEFAULT_CACHE_CONTROL` therefore caps a long server lifetime at 10 minutes.
3. `min-fresh` adds to the required age; `max-stale` extends acceptance only when the response is not `must-revalidate`.
4. Fresh (`age + minFresh < fresh + maxStale` and not `no-cache`): served from cache with no network call. Not fresh: conditional request with `If-None-Match` (ETag) or `If-Modified-Since` (Last-Modified) when a validator exists; a `304` refreshes the stored headers; anything else replaces the entry.
5. `CacheControl.FORCE_NETWORK` (`no-cache`) bypasses the cache for reads. `FORCE_CACHE` (`only-if-cached, max-stale=2147483647`) returns the entry or a 504.

## 4. Consequences for Kurobayomi

* Feature: a repeated open of a chapter list or a details page within the freshness window costs no network time (the window is the server's `max-age` capped at 10 minutes for requests that use `DEFAULT_CACHE_CONTROL`).
* Trap: the default cache control is set per request in `Requests.kt`, so a source that builds `Request.Builder()` directly bypasses the 10 minute cap and uses the server's own freshness.
* The cache has no per-host or per-source isolation beyond the URL key, and no `Vary` beyond what OkHttp checks (`Vary` headers are compared on read).

## 5. Rust design (proposal, `kuro-net::cache`, P2)

Use `http-cache-semantics` (RFC 9111 policy computation) with a disk store from the `http-cache-reqwest` middleware. Check the crate's current API before implementation; the design only fixes the behaviour below, not the crate calls:

* Directory: `Paths::cache().join("network_cache")` (rule 4 of `plans/addenda/requirements-cross-platform.md`). The cache dir is `$XDG_CACHE_HOME/kurobayomi` on Linux and comes from the `Paths` trait only.
* Cap: 5 MiB total, enforced by Kurobayomi (not by the store): after each write, if the sum of entry sizes is above 5 MiB, delete least recently used entries until it is at or below 5 MiB. LRU order uses the entry's last access time stored in an index file.
* Only GET and HEAD are stored; any other method removes the entry for its URL.
* Response codes and `no-store` rules: section 2 of this file (same table).
* Freshness: section 3. The heuristic (10% rule) is kept because it changes which responses are served without a network request.
* `FORCE_NETWORK` and `FORCE_CACHE` map to `CacheControl::no_cache` and `only_if_cached`.
* Persistence: the cache survives restarts (OkHttp does), unlike `cacheDir` on Android which the OS may clear. A "Clear network cache" action is not in the Kotlin app and is not added here (the cache is cleared by deleting the directory).
* Progress/download calls bypass the cache exactly like `newCachelessCallWithProgress`.

Divergences to record: (a) `http-cache-semantics` may differ from OkHttp on rare header combinations; the corpus test in `p1-edge-cases-and-tests.md` pins the rules above; (b) LRU eviction runs on write, not on a timer, so the store can exceed 5 MiB briefly during a large response.
