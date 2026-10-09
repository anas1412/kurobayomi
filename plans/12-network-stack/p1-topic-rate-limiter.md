# Topic: rate limiter (sliding window, global and per host)

Sources: `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt` (128 lines), `.../interceptor/SpecificHostRateLimitInterceptor.kt` (77 lines). Callers: `plans/catalogs/network-endpoints-01.md` (AniList and tracker limits), `area:27-tracking-core` through `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin`, and extension code through `OkHttpClient.Builder.rateLimit` / `rateLimitHost`.

## 1. Public API (OBSERVED)

| Function | Since | Signature (repo-relative line) |
|---|---|---|
| `rateLimit` (deprecated) | extension-lib 1.3 | `fun OkHttpClient.Builder.rateLimit(permits: Int, period: Long = 1, unit: TimeUnit = TimeUnit.SECONDS)` (`RateLimitInterceptor.kt:34-38`) |
| `rateLimit` | extension-lib 1.5 | `fun OkHttpClient.Builder.rateLimit(permits: Int, period: Duration = 1.seconds)` (`RateLimitInterceptor.kt:53-54`) |
| `rateLimitHost` (deprecated) | 1.3 | `(httpUrl: HttpUrl, permits: Int, period: Long = 1, unit = SECONDS)` (`SpecificHostRateLimitInterceptor.kt:31-38`) |
| `rateLimitHost` | 1.5 | `(httpUrl: HttpUrl, permits: Int, period: Duration = 1.seconds)` (`:55-59`) |
| `rateLimitHost` | 1.5 | `(url: String, permits: Int, period: Duration = 1.seconds)` (`:76-77`) |

All of them call `addInterceptor(...)` on the builder, which means the limiter is an APPLICATION interceptor: it runs once per logical call, before redirects and before the network. Each builder call creates a new `RateLimitInterceptor` with its own queue. Two `rateLimit` calls on one builder create two independent limiters, each enforcing its own limit on all requests.

The KDoc examples: `permits = 5, period = 1.seconds` means 5 requests per second; `permits = 10, period = 2.minutes` means 10 requests per 2 minutes.

Consumers in this repository: `AniList` tracker `rateLimit(85, 1.minutes)` as listed in `plans/catalogs/network-endpoints-01.md` (see `area:27-tracking-core`). Source extensions are external; they call the same functions.

## 2. Constructor state (OBSERVED, `RateLimitInterceptor.kt:58-66`)

* `host: String?`, `permits: Int`, `period: Duration`.
* `requestQueue = ArrayDeque<Long>(permits)`: start timestamps in milliseconds of `SystemClock.elapsedRealtime()`.
* `rateLimitMillis = period.inWholeMilliseconds` (truncation: `1.5.seconds` is 1500, `999.microseconds` is 0).
* `fairLock = Semaphore(1, true)`: one permit, fair (FIFO).
* `@Suppress("PLATFORM_CLASS_MAPPED_TO_KOTLIN")` on the class only.

## 3. Algorithm (OBSERVED, `RateLimitInterceptor.kt:68-127`)

```
intercept(chain):
  call = chain.call()
  if call.isCanceled(): throw IOException("Canceled")
  request = chain.request()
  if host != null && host != request.url.host: return chain.proceed(request)      // no accounting
  fairLock.acquire()                                                              // InterruptedException -> IOException(e)
  try:
    synchronized(requestQueue):
      while requestQueue.size >= permits:
        periodStart = elapsedRealtime() - rateLimitMillis
        hasRemovedExpired = false
        while requestQueue not empty and requestQueue.first <= periodStart:
          requestQueue.removeFirst(); hasRemovedExpired = true
        if call.isCanceled(): throw IOException("Canceled")
        else if hasRemovedExpired: break
        else: try wait(requestQueue.first - periodStart)  catch InterruptedException: continue
      timestamp = elapsedRealtime()
      requestQueue.addLast(timestamp)
  finally:
    fairLock.release()
  response = chain.proceed(request)
  if response.networkResponse == null:                                            // served from cache
    synchronized(requestQueue):
      if requestQueue empty or timestamp < requestQueue.first: return
      requestQueue.removeFirstOccurrence(timestamp)
      notifyAll()
  return response
```

Observed details that the port must keep:

1. The `host` check uses `request.url.host` after redirects are NOT yet followed, so a redirect to another host still counts against the original host's limit only if the redirected request passes through this interceptor again (it does not: redirects are handled inside OkHttp's `RetryAndFollowUpInterceptor`, which sits before application interceptors). So a redirect chain costs one permit in total.
2. Cancellation is checked at entry and inside the wait loop. A call cancelled while waiting throws `IOException("Canceled")`, which the caller sees as a cancelled call.
3. A `wait` is woken early only by `notifyAll()` from the cache-hit path. There is no other wake-up source; a sleep that is too long is not shortened when the call is cancelled. (The cancel check happens only after the wait returns.) Rust removes this flaw (see section 5).
4. `permits = 0` (or negative) is not validated. The loop then calls `requestQueue.first` on an empty deque and throws `NoSuchElementException`, which `UncaughtExceptionInterceptor` converts into `IOException`. Rust rejects `permits == 0` at construction.
5. Cache hit: `networkResponse == null` means the response came from the cache, no network I/O happened, and the permit is returned by removing exactly this call's timestamp. The guard `timestamp < requestQueue.first` means the entry has already expired and been pruned, so nothing is removed. Only entries equal to `timestamp` are removed (`removeFirstOccurrence`). Because timestamps are milliseconds, two requests in the same millisecond share a value and removal takes the first match, which is equivalent.
6. A non-cached response that fails with an exception keeps its permit (no refund). A `304 Not Modified` from the network counts as network traffic and keeps its permit.

## 4. The `host` quirk (OBSERVED)

`rateLimitHost(url: String, ...)` uses `url.toHttpUrlOrNull()?.host`. For an unparsable string the host becomes `null`, and `null` means "limit every request" (`RateLimitInterceptor.kt:73-74`: `null, request.url.host -> {}`). A typo in an extension's host URL therefore throttles all sources. Rust: an unparsable host logs a warning once (`tracing::warn!`) and the limiter is created with `host = None` only when the caller passed `None`; for an invalid string the constructor returns an error and the extension receives `NetError::InvalidUrl`. This is a divergence, listed in `p1-edge-cases-and-tests.md`.

## 5. Rust design (proposal, `kuro-net::interceptors::rate_limit`)

```rust
pub struct RateLimit {
    host: Option<String>,                 // None = every request
    permits: usize,                       // >= 1, validated
    period: Duration,                     // > 0, validated
    window: tokio::sync::Mutex<VecDeque<tokio::time::Instant>>, // FIFO-fair waiters
    refund: tokio::sync::Notify,          // wakes waiters on cache-hit refund
}

impl RateLimit {
    pub fn new(host: Option<&str>, permits: usize, period: Duration) -> Result<Self, NetError>;
}

#[async_trait]
impl Interceptor for RateLimit {
    async fn intercept(&self, chain: Chain<'_>, req: Request) -> Result<Response, NetError> {
        if let Some(h) = &self.host { if req.url.host_str() != Some(h) { return chain.proceed(req).await; } }
        let cancel = req.opts.cancel.clone();
        let stamp = {
            let mut q = self.window.lock().await;               // FIFO: tokio Mutex is fair
            loop {
                let now = Instant::now();
                while let Some(&front) = q.front() { if front + self.period <= now { q.pop_front(); } else { break; } }
                if q.len() < self.permits { break; }
                let wait_for = q.front().map(|f| (*f + self.period).saturating_duration_since(now)).unwrap_or_default();
                tokio::select! {
                    _ = tokio::time::sleep(wait_for) => {}
                    _ = self.refund.notified() => {}
                    _ = cancel.cancelled() => return Err(NetError::Canceled),
                }
            }
            let now = Instant::now(); q.push_back(now); now
        };
        let resp = chain.proceed(req).await?;
        if resp.from_cache {
            let mut q = self.window.lock().await;
            if let Some(pos) = q.iter().position(|t| *t == stamp) { q.remove(pos); self.refund.notify_waiters(); }
        }
        Ok(resp)
    }
}
```

Notes for the implementer:

* The Kotlin fair semaphore and the monitor collapse into the single `tokio::sync::Mutex`, which is FIFO. Only the head waiter sleeps; others block on the lock, as in Kotlin.
* `Instant` is monotonic (`CLOCK_MONOTONIC`), the same guarantee as `elapsedRealtime`. Kotlin's `elapsedRealtime` includes deep sleep; `Instant` does not advance during system suspend on Linux (`CLOCK_MONOTONIC` stops during suspend). After resume, a window may therefore be longer than real time. Divergence accepted: a limiter may be slightly too strict after suspend, never too loose.
* `refund` uses `notify_waiters`, which wakes only the currently blocked sleepers; a waiter that arrives later re-checks the queue under the lock first, so no wake-up is lost.
* Waiting is cancellable: the `CancellationToken` in `CallOptions` (T-12a-022) aborts within one scheduler tick.
* Only the rate limit is per interceptor instance, the same as Kotlin. Two builders' limiters do not share state.

Tests: `p1-edge-cases-and-tests.md` section "Rate limiter".
