# Topic: DNS over HTTPS (DoH)

Sources: `core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt` (199 lines), `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:42-57`. Library: OkHttp 5.4.0 `okhttp-dnsoverhttps`, upstream file `okhttp-dnsoverhttps/src/main/kotlin/okhttp3/dnsoverhttps/DnsOverHttps.kt` at tag `parent-5.4.0` (read during this review; line numbers below refer to that file).

Provider table, bootstrap addresses and labels: `p1-data-and-config.md` section 3. This file describes the resolver algorithm the Rust port must reproduce.

## 1. OBSERVED: wiring

1. `NetworkHelper.kt:42-57` reads `preferences.dohProvider().get()` once, while the builder is constructed. Ids 1..13 call `builder.dohX()`; any other value (`-1`, `0`, `14`, ...) leaves the system resolver.
2. Each `OkHttpClient.Builder.dohX()` executes `DnsOverHttps.Builder().client(build())`. `build()` snapshots the builder at that moment, so the DoH requests use a client that has: the cookie jar, the 30 s connect and read timeouts, the 2 minute call timeout, the 5 MiB cache, `UncaughtExceptionInterceptor`, `UserAgentInterceptor`, and (only when `verbose_logging` is on) the `HttpLoggingInterceptor` network interceptor. The DoH client does NOT contain `CloudflareInterceptor`, because that interceptor is added after the `when` block (`NetworkHelper.kt:60-64`).
3. The DoH requests therefore receive `User-Agent: <default_user_agent>` from `UserAgentInterceptor`, and their responses are stored in the shared HTTP cache (GET, no `Cache-Control` set by `buildRequest`).
4. The outer `dns(...)` installs the `DnsOverHttps` as the `Dns` of the final client. Every hostname lookup of the final client, including the lookups for the DoH host itself, goes through `DnsOverHttps.lookup`. The DoH host is protected by `bootstrapDnsHosts(...)`: `buildBootstrapClient` (`DnsOverHttps.kt` companion) returns `BootstrapDns(builder.url!!.host, hosts)` when the list is non-null, which answers the DoH host name with exactly those addresses and no other lookup.

## 2. OBSERVED: `lookup(hostname)` (`DnsOverHttps.kt`, class body)

```kotlin
override fun lookup(hostname: String): List<InetAddress> {
    if (!resolvePrivateAddresses || !resolvePublicAddresses) {
        val privateHost = isPrivateHost(hostname)
        if (privateHost && !resolvePrivateAddresses) throw UnknownHostException("private hosts not resolved")
        if (!privateHost && !resolvePublicAddresses) throw UnknownHostException("public hosts not resolved")
    }
    return lookupHttps(hostname)
}
```

Defaults in the builder (`DnsOverHttps.kt`, `Builder`): `includeIPv6 = true`, `post = false`, `resolvePrivateAddresses = false`, `resolvePublicAddresses = true`. Consequences:

* `isPrivateHost(host)` = `PublicSuffixDatabase.get().getEffectiveTldPlusOne(host) == null`. A host with no known public suffix (`localhost`, `nas`, `printer.lan`, any single-label name) is "private" and fails with `UnknownHostException("private hosts not resolved")` WHILE DoH IS ON. The system resolver would have answered. This is a real divergence that affects LAN-hosted tracker servers (Komga, Jellyfin, Kavita on a LAN name). Those trackers already use `Dns.SYSTEM` (`area:27-tracking-core` and related). Kurobayomi keeps this rule and documents it, and the settings UI shows a hint (T-12a-031).
* The check does NOT inspect answer IP addresses. OkHttp does not reject answers that are RFC 1918 or loopback addresses.

## 3. OBSERVED: `lookupHttps(hostname)`

1. Build a list of network calls: always an `A` query; an `AAAA` query only when `includeIPv6`.
2. Each query: `Request.Builder().header("Accept", "application/dns-message")` then, for GET (the default), `query = DnsRecordCodec.encodeQuery(hostname, type)`, `encoded = query.base64Url().replace("=", "")`, `url = dnsUrl.newBuilder().addQueryParameter("dns", encoded).build()`. The POST variant sends the wire bytes with `Content-Type: application/dns-message` to the provider URL with `?hostname=<name>` added as a cache-key override.
3. Both calls run concurrently with `enqueue`. A `CountDownLatch(count)` waits for all. Results are appended under `synchronized(results)`, so the order of `A` and `AAAA` results is arrival order, not fixed.
4. `processResponse`: on success, `readResponse` decodes the answers and `results.addAll(addresses)`. Failures are appended to `failures` under `synchronized(failures)`.
5. `readResponse`: `if (!response.isSuccessful) throw IOException("response: " + code + " " + message)`; `if (body.contentLength() > 65536) throw IOException("response size exceeds limit (65536 bytes): N bytes")`; then `body.source().readByteString()` and `DnsRecordCodec.decodeAnswers(hostname, bytes)`. The size check uses only the declared `Content-Length`. A chunked response (`contentLength() == -1`) is read without any cap.
6. Result: if `results` is non-empty return it (possibly only the IPv4 or only the IPv6 answers when one query failed). Otherwise `throwBestFailure(hostname, failures)`, which throws `UnknownHostException` for the hostname.

## 4. Rust port (proposal, `kuro-net::dns`)

Crates: `hickory-proto` for the wire format (`Message`, `Query`, `RecordType::{A, AAAA}`, `to_vec`, `from_vec`) so no DNS codec is hand-written; `reqwest` for HTTP; `base64` with the URL-safe alphabet and no padding (`base64::engine::general_purpose::URL_SAFE_NO_PAD`), which equals OkHttp's `base64Url().replace("=", "")`.

```rust
pub struct DohResolver {
    provider: DohProvider,                 // table in p1-data-and-config.md section 3
    http: reqwest::Client,                 // built with resolve_to_addrs(provider host, bootstrap)
    include_ipv6: bool,                    // true
    resolve_private: bool,                 // false: reproduce OkHttp rule
    resolve_public: bool,                  // true
}

impl reqwest::dns::Resolve for DohResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving { /* ... */ }
}
```

Algorithm for `resolve(name)`:

1. If `!resolve_private` and the name has no registrable domain according to the Public Suffix List (crate `psl`, `psl::domain_str` returns None) return `io::Error` with text `private hosts not resolved`.
2. If the name parses as an IP literal, return it without a query (OkHttp's `Dns` is never called for literals; reqwest does the same).
3. Issue `A` and, when `include_ipv6`, `AAAA` queries concurrently with `futures::future::join_all`, each with an overall deadline of 10 s (divergence: OkHttp uses the 2 minute call timeout; a DNS query that takes that long is a bug, so a tight bound is safer).
4. Accept a response only when HTTP status is 2xx, and when `Content-Length` (if present) is `<= 65536`; additionally read at most 65536 bytes from a chunked body and fail with the same message when the limit is exceeded (divergence: OkHttp does not cap chunked bodies).
5. Decode with `hickory_proto::op::Message::from_vec`; take `answers()` whose `record_type` matches the query; `rdata().as_a()` / `as_aaaa()` gives the address. A response with `RCODE != NoError` is a failure.
6. Merge: union of the successful answers in arrival order (matching OkHttp), deduplicated; empty union returns the best failure (first error in the order they were recorded).
7. The DoH provider host is pinned with `reqwest::ClientBuilder::resolve_to_addrs(host, &bootstrap_addrs)` so the DoH client never calls DoH itself. The DoH client uses the system resolver for anything else (none is needed).
8. The DoH client is a separate `reqwest::Client` from the main one: it has no interceptors, no cookie store, and no cache. It sends `User-Agent` set to the configured default (matches Kotlin's UA interceptor effect on DoH requests). Divergence: Kotlin DoH responses are cached through OkHttp's disk cache; Rust does not cache DNS answers (they are short lived and the OS resolver cache is not used by DoH). Resolved addresses are cached in-process per host for the answer TTL, capped to 300 s (divergence: documented here because OkHttp has no DNS cache of its own beyond HTTP cache).

Tests (see `p1-edge-cases-and-tests.md`): private-host rejection for `localhost`, IPv6 merge, 65536-byte limit, base64 without padding, bootstrap pinning.

## 5. Provider list edge cases

* `dohMullvad()` builds the URL from `" https://dns.mullvad.net/dns-query"` with a leading space. OkHttp's `toHttpUrl()` strips leading and trailing ASCII whitespace, so the URL is valid. Rust uses `"https://dns.mullvad.net/dns-query"` with no space (the space is an accident of the source, not behaviour to keep).
* Function `dohNajalla` is registered for `PREF_DOH_NJALLA` (11) and its label is "Njalla".
* `doh360` is named after the HTTP host `doh.360.cn`; the Rust enum variant is `Qihoo360`.
* `dohQuad101` points at `dns.twnic.tw` (the TWNIC operated "Quad 101" resolver), not at a Cloudflare host.
* The Kotlin list is a closed set; a future id needs a new variant and a migration rule (none is planned).

## 6. Failure table

| Situation | OkHttp (Kotlin) | Rust proposal |
|---|---|---|
| Provider unreachable on all bootstrap IPs | `IOException` from the `BootstrapDns` connect attempts, surfaced as `UnknownHostException` for the target host | `NetError::UnknownHost { host }`, message `Couldn't reach <host>` (`exception_unknown_host`) |
| Only the AAAA query fails | IPv4 addresses returned | same |
| Both queries fail | `UnknownHostException` | `NetError::UnknownHost` |
| `localhost` or a single-label name while DoH is on | `UnknownHostException("private hosts not resolved")` | `NetError::UnknownHost { host }` with a settings hint, not a crash |
| `doh_provider` is `0`, `14`, `-1` | system DNS | system DNS |
