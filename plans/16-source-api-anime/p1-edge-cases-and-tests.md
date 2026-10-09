# 16-source-api-anime: edge cases, failure modes, races, tests

Edge IDs (`E-nn`) are referenced from p1-behavior.md, p1-data-and-config.md and p1-rust-design.md. Each entry gives the observed Kotlin behavior (with path:line), the consequence, and the Rust decision. "Keep" means the Kotlin behavior is reproduced on purpose. "Fix" means Rust deviates on purpose.

---

## A. Routing, recursion and capability defects

### E-01 Hoster routing ignores Parsed hoster selectors (OBSERVED)
- Path: `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt:60-78`.
- Behavior: `checkHasHosters` stops at `ParsedAnimeHttpSource` before inspecting it. A subclass that overrides only `hosterListSelector()` and `hosterFromElement()` has neither `getHosterList`, `hosterListRequest` nor `hosterListParse` declared, so the legacy `getVideoList(episode)` path is used.
- Consequence: hoster selectors of Parsed sources are dead code unless the subclass also overrides one of the three names.
- Rust: `provides_hosters()` is an explicit capability. Parsed helper sets it when a hoster selector is configured. Fix.

### E-02 Self-recursive default overrides (OBSERVED, defect)
- Path: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeCatalogueSource.kt:65-67`.
- `override suspend fun getHosterList(episode) = getHosterList(episode)` and `override suspend fun getVideoList(hoster) = getVideoList(hoster)`.
- Consequence: a non-HTTP `AnimeCatalogueSource` that does not override them: `StackOverflowError` on first call.
- Rust: no default body that re-enters. Required methods or `NotSupported`. Fix.

### E-03 Unsynchronized shared state in parallel hoster loading (OBSERVED)
- Path: `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt:79-113`.
- `hosterStates` is a plain `MutableList` written from several `async` children on `Dispatchers.IO`. `ArrayList` is not thread-safe; index writes are not guaranteed visible to the reader.
- Consequence: a torn or stale read of a state can pick a wrong hoster in phase B.
- Rust: each task returns its result; a single collector owns the vector. Fix.

### E-04 Preferred probe limited to one video per hoster (OBSERVED)
- Path: `HosterLoader.kt:89-111`.
- Only `indexOfFirst { it.preferred && !it.initialized }` is tried in phase A. If it fails, the hoster's other videos are only reached in phase B, after every other hoster's phase A has finished.
- Consequence: a second preferred video on the same server is not tried early.
- Rust: keep (documented). Phase A tries one candidate per hoster, same as Kotlin.

### E-05 Selector dialect differences (Jsoup vs scraper)
- Path: `util/JsoupExtensions.kt`, `online/ParsedAnimeHttpSource.kt`.
- Jsoup supports `:contains(text)`, `:matches(regex)`, `:containsOwn`, `:has(...)` and the `:nth-child` family with Jsoup semantics. `scraper` (html5ever + selectors) supports standard CSS and not Jsoup's text pseudo-classes.
- Consequence: a selector copied from an extension that uses `:contains` returns zero matches silently, so the listing is empty and `hasNextPage` is false.
- Rust: `Selector::parse` failure is an error (no silent empty list). Sources needing text matching filter in code. Fix (explicit error).
- Test: selector parse failure yields `SourceError::Parse`, never `Ok(empty)`.

### E-06 Anime URL normalisation drops domain only when parse succeeds (OBSERVED)
- Path: `AnimeHttpSource.kt:685-699` (`getUrlWithoutDomain`), contrast `HttpSource.kt:412` (manga replaces spaces with `%20` before parsing).
- A URL containing a space fails `java.net.URI` parsing, so the full URL is stored (with domain). The manga twin keeps the path.
- Consequence: the same anime can be stored with two different url forms depending on whether the source emitted a space.
- Rust: `url_without_domain` mirrors the Kotlin algorithm exactly (including the failure fallback) for compatibility with existing rows. Keep, with test vectors in p1-topic-url-and-id-compat.md.

### E-07 `copyHttpServer` can drop `memo` (OBSERVED, probable)
- Path: `model/Video.kt:167-178` calls `this.copy(videoUrl = ..., subtitleTracks = ..., audioTracks = ...)` (with all other arguments defaulted to `this.*`). The deprecated `copy` at `Video.kt:111-142` sets `memo = JsonObject.EMPTY` ignoring `this.memo`. Which overload Kotlin selects here is not confirmed by a compiler run in this plan.
- Consequence if the deprecated overload is chosen: `memo` is lost during local-server rewrite.
- Rust: field-by-field clone keeps `memo`. Fix. Test: `rewrite_to_local` preserves `memo`.

### E-08 Local server port 0 rewrites to `localhost:0` (OBSERVED)
- Path: `AnimeDownloader.kt:452-454` (`httpServer?.listeningPort ?: 0`), `HttpServer.kt:18-25` (start failure leaves port 0).
- Consequence: the stream URL becomes `http://localhost:0/...`, the request fails later with a connection error far from the cause.
- Rust: `rewrite_to_local(0)` returns `SourceError::LocalServer`. Fix.

### E-09 Blocking image-tile fetch (OBSERVED)
- Path: `AnimeHttpSource.kt:533-537`: `client.newCall(GET(url, headers)).execute()` inside a `suspend` function, no status check.
- Consequence: blocks the dispatcher thread for the whole download; a 404 HTML body is passed to the decoder and yields `null`.
- Rust: async `fetch` with status check; decode on `spawn_blocking`. Fix.

---

## B. Parsing and data edge cases

### E-10 Empty remote values and what they mean (OBSERVED)
- `UpdateAnimeFromRemote.kt` (`awaitUpdateFromSource`): an empty or null `thumbnail_url` / `background_url` never clears the stored image. An empty remote `title` is ignored for favorites and also ignored for non-favorites (`remoteTitle.isNotEmpty()` is required before use).
- `SAnime.title` is `lateinit`: if a source never sets it, the Kotlin read throws `UninitializedPropertyAccessException`, caught and treated as `""` (`:151-155`).
- Rust: `title: String` required in constructors; empty string means "not set". Keep semantics.

### E-11 `selectInt` and `selectText` edge values (OBSERVED)
- `selectInt`: non-numeric or empty matched text throws `NumberFormatException` (not caught). Missing element returns the default.
- `selectText`: matched but empty element returns `""`, not the default. Only a missing element returns the default.
- Rust: `select_int` returns `Err(Parse)` for non-numeric matched text and `default` for missing. `select_text` returns `Some("")` for an empty match. Keep the distinction. Test both.

### E-12 Filter equality quirks (OBSERVED)
- Path: `model/AnimeFilter.kt:34-39`. `equals` compares `name` and `state` only (no class check). `Header("A")` equals `Select("A", values = ["x"])` when both states are `0`. `CheckBox("A")` never equals `Select("A")` (Boolean vs Int).
- `AnimeFilterList.kt:10-12`: `equals` always returns `false`, so two identical lists compare unequal, including to themselves.
- Rust: derived equality on an enum (class is part of the value). Fix (E-13 documents the deliberate change). The always-false `equals` is not reproduced; value equality is used where the Kotlin used it for change detection.

### E-13 Filter list value equality (Rust deviation, deliberate)
- See E-12. Test: `FilterList(vec![a.clone()]) == FilterList(vec![a])` is `true`; `Header("A") != Select("A")`.

### E-14 Select index out of range (OBSERVED)
- `AnimeFilter.Select` stores `state: Int` and `values: Array<V>`, no bounds check. A source reading `values[state]` with a stale index throws `ArrayIndexOutOfBoundsException` in the source.
- Rust: constructor clamps to `0` with `tracing::warn!` when `index >= values.len()` and `values` non-empty; empty values gives `index = 0` and a `Select` whose `value()` returns `None`. Fix.

### E-15 Torrent title override never uses the torrent's own title (OBSERVED)
- Path: `torrentutils/TorrentUtils.kt:63-65`: `overrideTitle ?: torrent.title`; `overrideTitle` is the non-null `title` parameter, so `torrent.title` is never used. An empty `title` argument also wins (`"" ?: x` is `""`).
- Rust: `title = if title.is_empty() { torrent.title } else { title }`. Fix (documented). Test: empty title falls back to the torrent title.

### E-16 Torrent hash null dereference (OBSERVED)
- Path: `TorrentUtils.kt:67,69`: `torrent.hash!!`. TorrServer JSON with a missing `hash` throws `NullPointerException` (not caught by `getTorrentInfo`).
- Rust: missing hash is `SourceError::Parse("torrent hash missing")`. Fix.

### E-17 Magnet detection is case-sensitive (OBSERVED)
- Path: `TorrentUtils.kt:24`: `url.startsWith("magnet")`. `MAGNET:?xt=urn:btih:abc` goes down the `.torrent` file path and fails with an HTTP or parse error.
- Rust: case-insensitive scheme check. Fix. Test: `MAGNET:?xt=urn:btih:abc` routes to the magnet path.

### E-18 Error mapping collapses everything to "TorrServer disabled" (OBSERVED)
- Path: `TorrentUtils.kt:28-33`: any exception other than `SocketTimeoutException` (for example HTTP 500, JSON decode error) becomes `DisabledTorrServerException`, whose text is "TorrServer is not running" (`AYMR.strings.torrserver_disabled`).
- Rust: map connection refused to "not running"; map other HTTP and parse errors to their own messages. Fix.

### E-19 Opaque or path-less URLs in `getUrlWithoutDomain` (OBSERVED)
- `uri.path` is null for opaque URIs (e.g. `mailto:x`). Kotlin `var out = uri.path` is a platform `String?`; `out += "?" + query` produces `"null?query"` when a query exists, and returns null from a `String`-typed function otherwise (possible NPE at the call site).
- `http://example.com` has path `""`, so the stored url becomes `""`.
- Rust: a URL with no path component (opaque, e.g. `mailto:x`) returns the original string unchanged (Fix: Kotlin produces `null`). A URL whose path is empty (`http://example.com`) returns `""`, matching Kotlin (Keep), and a `tracing::warn!` is emitted because the stored url is then empty.

### E-20 Java `URI` strictness (OBSERVED)
- `java.net.URI` rejects spaces, `"`, `<`, `>`, `\`, `^`, `{`, `}`, `|` and other characters not allowed raw. Those URLs go through the `URISyntaxException` branch and return the original string.
- Rust: `url::Url::parse` accepts many of these (it percent-encodes). Consequence if unhandled: the Rust result differs from Kotlin for such URLs. Keep: before parsing, reject (return the original string) any input containing a character Java's `URI` rejects raw, which reproduces the `URISyntaxException` fallback. Test vectors in p1-topic-url-and-id-compat.md.

### E-21 Decoded components in stored URLs (OBSERVED)
- `java.net.URI.getPath()`, `getQuery()`, `getFragment()` return decoded text. `https://x/a%20b?q=a%26b` yields `/a b?q=a&b`.
- Rust must reproduce the decoded output because stored rows were written this way. Keep. Test vectors in topic doc.

### E-22 `"null"` sentinel URL (OBSERVED)
- `EpisodeLoader.kt:182`: a video whose `videoUrl` equals the literal string `"null"` is treated as a legacy "resolve later" video.
- Consequence: a legitimate source returning the text `null` as a URL triggers `getVideoUrl` (legacy). Rust never produces the sentinel; not ported. Fix (dropped with T-16a-015).

### E-23 Headers: null versus empty (OBSERVED)
- `Video.headers == null` means "use `source.headers`" (`AnimeDownloader.kt:577`). An empty `Headers` means "send no headers".
- Rust: `Option<Headers>`; `None` = source headers, `Some(empty)` = none. Keep. Test both cases.

### E-24 Synthetic hoster name leaks into UI (OBSERVED)
- `Hoster.NO_HOSTER_LIST = "no_hoster_list"` is the `hosterName` of the synthetic hoster. A UI that displays `hosterName` directly shows the raw key.
- Rust: UI maps `NO_HOSTER_LIST` to "no hoster" label (or hides the hoster row). Keep the constant.

### E-25 Timestamp values unvalidated (OBSERVED)
- `TimeStamp.start` and `end` are `Double` with no range check. A source may return `NaN`, negative numbers or `end < start`.
- Rust: `TimeStamp::new` rejects non-finite values and `end < start` by dropping the entry with a warning. Fix.

---

## C. Async, cancellation and failure paths

### E-26 Combined update all-or-nothing (OBSERVED)
- `AnimeCatalogueSource.kt:43-47`: details and episode children run in `supervisorScope`, but `.await()` of a failing child throws out of the block, so a successful episode list is discarded when details fail.
- Consequence: a details-only failure loses the episode refresh for that run.
- Rust: keep the all-or-nothing contract for `anime_episode_update` (the trait returns one result). The app layer may retry the flags separately. Keep.

### E-27 Details not implemented (OBSERVED)
- `AnimeHttpSource.animeDetailsParse` default throws `UnsupportedOperationException`. A source that implements only episodes, with `fetchDetails = true`, fails the whole combined update (E-26).
- Rust: the trait gains `fn supports_details(&self) -> bool { true }` and `fn supports_episode_list(&self) -> bool { true }` (defaults true; Parsed helpers set them from configured selectors). The app passes `fetch_details = false` (or skips the episode call) when the capability is false, so the all-or-nothing failure of E-26 cannot be triggered by a missing capability. Fix.

### E-28 Episode list not implemented (OBSERVED)
- Same as E-27 for `episodeListParse`.

### E-29 Cancellation must not be swallowed (OBSERVED)
- `HosterLoader.kt:157-160`, `EpisodeLoader.kt:197-200`: `CancellationException` is rethrown; other exceptions become `None`/`Error`.
- Rust: `SourceError::Cancelled` is propagated by `?`; `resolve_once` maps other errors to `None`. Test: cancellation during `resolve_video` aborts the best-video search.

### E-30 Early return cancels siblings (OBSERVED)
- `HosterLoader.kt:101-103,128-130`: `coroutineContext.cancelChildren()` before returning. Partially loaded hosters are dropped.
- Rust: `JoinSet::abort_all()` (section 5.4 of rust design). Test: after success, no further `hosters()`/`videos_for_hoster()` completes after return.

### E-31 Phase A probes run concurrently without a limit (OBSERVED)
- One `async` per hoster; no `Semaphore`. A source with 50 hosters issues 50 concurrent requests.
- Rust: concurrency limited by the rate limiter (area:12) for metered sources. Keep unbounded per call but bounded by the limiter. Test: 3 hosters -> at most 3 concurrent `videos_for_hoster` calls with limiter at 3.

### E-32 Default 10-minute cache header on every request (OBSERVED)
- `Requests.kt:13`: `Cache-Control: max-age=600` on default GETs. Sites that rely on fresh listings receive cached responses from the OkHttp cache (area:12 cache).
- Rust: keep the header on the request (the cache layer decides). Test: request header present.

### E-33 Lazy `headers` are not refreshed (OBSERVED)
- `AnimeHttpSource.kt:77` `by lazy`. The user-agent is captured on first access.
- Rust: `OnceLock` (same). Documented consequence. Fix candidate: rebuild on settings change (not in this scope).

### E-34 Video state is not part of equality (OBSERVED)
- `Video.status` is a body property (`Video.kt:144-149`). `Video` equality ignores state, so the UI must track state separately.
- Rust: `VideoEntry { video, state }`. Keep.

### E-35 `resolveVideo` may return an already-resolved video (OBSERVED)
- `getResolvedVideo` (`HosterLoader.kt:152-168`) skips the call for `initialized == true`. A source that sets `initialized = true` itself prevents resolution. Consequence: a source must not set it.
- Rust: `resolve_once` sets `initialized` only after successful resolution. Keep.

---

## D. Shutdown, server and I/O edge cases

### E-36 Stop-before-start ordering in the server service (OBSERVED)
- `HttpServerService.kt:37-43`: a running server is stopped when a new start intent arrives, even for the same source id. The player may briefly lose the URL.
- Rust: starting a `LocalStreamServer` for a source that already has one returns the existing handle. Fix.

### E-37 Local server start failure messages (OBSERVED)
- `HttpServer.kt:22-24`: start exceptions are logged at DEBUG and swallowed. The service then logs at ERROR and stops (`HttpServerService.kt:57-59`).
- Rust: start errors are returned as `SourceError::LocalServer` and shown with `AYMR.strings.http_server_start_failure`. Fix (visible error).

### E-38 Range support for local streams (PROPOSAL)
- Kotlin source code in scope does not implement range handling; the local server is only the sources' own `NanoHTTPD` subclass. mpv seeking and ffmpeg input seeking send `Range` requests, so Rust must answer them.
- Rust: `LocalHandler::serve` gets the raw `Range` header; the default implementation in the crate answers `206` with `Content-Range`. Test: `bytes=100-199` returns 100 bytes and `Content-Range: bytes 100-199/<total>`.

### E-39 `getVideoThumbnails` cancellation
- `PlayerViewModel.kt` rethrows cancellation and logs others (section 9.2 of behavior). Rust: same.

---

## E. Race conditions to test

| Race | Where | Test |
|---|---|---|
| Two hosters resolve at the same time; both want to win | `best_video` phase A | Both resolve; exactly one video returned; the other task is aborted (no panic) |
| Player closes while phase A is resolving | `best_video` + `abort_all` | No task outlives the call; no UI update after close |
| `resolve_video` returns a video whose URL is empty | `resolve_once` | Treated as failure; next candidate tried |
| Season update and episode update run on the same anime concurrently | `anime_episode_update` / `anime_season_update` | Each returns its own list; no shared mutable state |
| Local server stopped while a download reads through it | `LocalStreamServer::stop` | Download sees connection close and fails with error, not a hang (test timeout 5 s) |
| Settings change of user agent during a request | `HttpBase::headers` | In-flight request keeps old header; next request (after restart) has new header (documented E-33) |

---

## F. Upstream tests to port

Upstream test files inside this scope: none (`find source-api -iname "*test*"` returns only `UpdateStrategy.kt` files, which are not tests).

Related upstream test in another area, to port as a fixture (owned by area:13, reused here):
- `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt` — two cases:
  1. `test-torrent-file.torrent` → `Torrent(title="test-torrent-file", hash="104701de4b710ee9720ee249bf220384659e8c38", torrentSize=18, trackers=["udp://tracker.opentrackr.org:1337/announce"], fileStats=[FileStats(id=null, path="test-torrent-file", length=18)])`.
  2. `test-torrent-directory.torrent` → `Torrent(title="test-torrent-directory", hash="863eb76f1cd0643d9fd2eb4a7da866c3752991b5", torrentSize=12, trackers=[6 entries, one duplicated: "udp://tracker.opentrackr.org:1337/announce" twice], fileStats=[FileStats(1,"3/3.1",4), FileStats(2,"2",2), FileStats(3,"1",2), FileStats(4,"3/3.2",4)])`.
  Fixtures: `core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-file.torrent`, `core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-directory.torrent`. Copy both into `tests/fixtures/torrent/`.

---

## G. New tests to write (Rust)

Unit tests (`cargo test`), each named for the behavior it locks:

1. `source_id_matches_kotlin_vectors` — `generate_id("Example","en",1) == 4851548778462321757`; `generate_id("Aniyomi Test","ja",2) == 7915262807009061958`; result always `>= 0`.
2. `source_id_lowercases_name` — `generate_id("EXAMPLE","en",1) == generate_id("example","en",1)`.
3. `display_name_format` — `"Example (EN)"`.
4. `headers_default_user_agent_only` — `headers()` returns exactly one pair `User-Agent`; value trimmed.
5. `get_request_adds_cache_control` — request carries `Cache-Control: max-age=600`.
6. `fetch_non_2xx_is_http_error` — 404 → `SourceError::Http{status:404}`; 200 → Ok.
7. `url_without_domain_vectors` — see topic doc table (at least 12 vectors).
8. `genres_split_and_distinct` — `"Action, Drama, Action,  ,Comedy"` → `["Action","Drama","Comedy"]`; `"a,b"` → `["a,b"]`; `"   "` → `None`.
9. `status_roundtrip` — ints 0..7 map to variants and back; 99 → `Other(99)` → 99.
10. `anime_defaults` — `SAnime::default()` has `season_number == -1.0`, `fetch_type == Episodes`, `update_strategy == AlwaysUpdate`, `status == 0`, `initialized == false`.
11. `episode_defaults` — `episode_number == -1.0f32`, `date_upload == 0`, `fillermark == false`.
12. `filter_equality` — `Header{"A"} != Select{"A", values:["x"], index:0}`; `FilterList` value equality true for equal contents.
13. `select_index_clamped` — index 5 with 2 values → 0 and warning flag set (E-14).
14. `parse_listing_has_next` — selector present → true; absent → false; `None` selector → false.
15. `parse_listing_bad_selector_is_error` — `:contains(x)` → `SourceError::Parse` (E-05).
16. `select_int_rules` — missing → default; `"12"` → 12; `""` → Err (E-11).
17. `select_text_empty_match` — `<a></a>` → `Some("")` (E-11).
18. `attr_or_text_text_keyword` — `"text"` → text content; other → attribute or `""` (E-11).
19. `hoster_provides_hosters_explicit` — `provides_hosters()` drives routing; a source with only selectors and no hoster capability goes legacy (E-01 fixed: routed to hosters when capability is true).
20. `select_best_prefers_preferred_tier` — preferred in hoster 2 beats URL-valid in hoster 1 (6.1 tier 1).
21. `select_best_tier2_first_nonempty` — no preferred → first non-empty URL in list order.
22. `select_best_ignores_error_and_loading` — video in `Error` state not selected.
23. `select_best_empty_is_none` — no ready hosters → `None`.
24. `rewrite_to_local_port_zero_is_error` — E-08.
25. `rewrite_to_local_keeps_memo` — E-07.
26. `placeholder_detection` — `http://localhost:1/x` true; `http://localhost:12/x` false; `http://localhost:1` true; `https://localhost:1/x` false; `http://localhost:10/` false.
27. `uses_local_server_checks_tracks` — placeholder only in an audio track → true.
28. `resolve_once_empty_url_is_none` — resolved video with `""` → None.
29. `resolve_once_cancel_propagates` — `SourceError::Cancelled` from resolve → Err(Cancelled) (E-29).
30. `resolve_once_skips_initialized` — `initialized = true` input is not sent to `resolve_video` (mock counter = 0).
31. `best_video_early_return_aborts_others` — mock source with slow hoster 2; hoster 1 preferred resolves; call returns in < hoster 2 delay; mock counter shows no completion of hoster 2 after return (E-30).
32. `best_video_phase_b_fallback` — phase A finds nothing; phase B resolves the first valid candidate.
33. `best_video_all_fail_is_none`.
34. `magnet_case_insensitive` — `MAGNET:?xt=urn:btih:abc` routed to magnet (E-17).
35. `torrent_title_fallback` — empty title → torrent title (E-15).
36. `torrent_missing_hash_is_parse_error` — E-16.
37. `magnet_builder_vector` — `magnet:?xt=urn:btih:abc&tr=udp%3A%2F%2Ftracker.example%3A80%2Fannounce%3Fx%3Da+b&index=2` for hash `abc`, one tracker, index 2; no trackers → no `&tr=`.
38. `default_json_tolerates_unknown_keys` — serde codec reads `SerializableVideo` with an unknown key (P2 test).
39. `serializable_video_pair_array_wire` — `headers` encodes as `[["k","v"]]` (P2).
40. `timestamp_validation` — `NaN` dropped; `end < start` dropped (E-25).
41. `headers_null_vs_empty` — E-23.
42. `local_server_range_206` — E-38, integration test with `reqwest` against the bound port.
43. `local_server_bind_failure` — occupying a fixed port does not affect port 0; start failure returns `LocalServer` error (E-37).
44. `anime_episode_update_all_or_nothing` — details failure → `Err` (E-26).
45. `anime_episode_update_flags_false_returns_inputs` — identical instances returned when both flags are false (behavior 3.2).

Integration tests (with a recorded HTML fixture per parser):
- `scrape_popular_listing_fixture` — fixture HTML → expected `Vec<SAnime>` titles, urls, hasNextPage.
- `scrape_episode_list_document_order` — episode order equals document order (behavior 4.3).

---

## H. Fixtures needed

| Fixture | Used by | Source |
|---|---|---|
| `tests/fixtures/source_id.json` (name, lang, version, expected id) | test 1–2 | the two vectors above plus 4 more computed with Python `hashlib` at implementation time |
| `tests/fixtures/url_vectors.json` (input, expected) | test 7 | topic doc table |
| `tests/fixtures/torrent/test-torrent-file.torrent` and `test-torrent-directory.torrent` | torrent parser (area:13) and test 35–36 | copied from upstream resources (section F) |
| `tests/fixtures/html/popular_listing.html`, `episode_list.html` | scrape tests | hand-written minimal HTML matching a selector set documented in the test |
| Mock `AnimeSource` (in-crate `#[cfg(test)]`) | tests 19–33 | counts calls per method; configurable delays with `tokio::time::sleep` |
| Local HTTP upstream (test-only `axum` route) | test 42 | in-crate |

No network access in tests. The HTTP client in tests is `reqwest` pointed at a local `axum` server.

---

## I. Non-obvious failure modes to document for users

1. A source that uses only the legacy `getVideoList(episode)` path shows one hoster named "no hoster list" (E-24). Expected, displayed as "no hoster".
2. Video list order is the source order unless the source sorts it (no global sort). A source that returns 1080p after 480p gets 1080p second if it is not preferred.
3. A stream whose URL is `http://localhost:1/...` only works after the local server starts. Playback and download start it automatically; a manual external-player launch of such a stream fails (no server).
4. Torrent sources need the TorrServer port (`pref_torrserver_port`, default 8090) to be reachable at `127.0.0.1`. The error is "TorrServer is not running" when the connection is refused (E-18).
5. The default user agent is a Firefox 136 string. Sites that block it need a source-level header override, not a global change.
