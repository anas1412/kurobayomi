# Area 15 — Manga source API: edge cases, failure modes, races, tests

## 0. Upstream tests

- No `*Test.kt` file exists under `source-api/` (verified: `find source-api -name "*Test*"` returns nothing; 54 `.kt` files in the module, all listed in `p1-inventory.md`). There are therefore no upstream test cases to port from this scope.
- Behaviour that other areas test upstream and that this scope drives: `app/src/test` (area:11), `domain/src/test` (area:07). Those are owned by their areas.
- Kurobayomi writes all tests below as new tests (`T-15a-033`, `T-15a-046`, `T-15a-048`).

## 1. Edge-case catalogue

Columns: ID | area of behaviour | input or condition | expected result (Kurobayomi) | Kotlin result (observed) | Task.

### 1.1 Identity and models

| ID | Input or condition | Expected (Kurobayomi) | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-ID-01 | `generate_id("Example","en",1)` | `4851548778462321757` | same (computed) | T-15a-012 |
| EC-ID-02 | `generate_id("MangaDex","en",1)` | `2499283573021220255` | same | T-15a-012 |
| EC-ID-03 | `generate_id("Ünïcode Src","ja",2)` | `8842957433569127029` | same | T-15a-012 |
| EC-ID-04 | any name/lang/version | `id >= 0` (sign bit clear) | same (`and Long.MAX_VALUE`) | T-15a-012 |
| EC-ID-05 | name with uppercase non-ASCII (e.g. `É`) | lowercased with Unicode rules before hashing | `lowercase()` (Locale.ROOT) | T-15a-012 |
| EC-ID-06 | `versionId` changed | new id (old rows orphaned); documented | same | T-15a-012 |
| EC-MD-01 | `SManga::new` without title | compile error (required) | `lateinit` read throws `UninitializedPropertyAccessException` | T-15a-002 |
| EC-MD-02 | `SChapter` default `chapter_number` | `-1.0` | `-1f` | T-15a-004 |
| EC-MD-03 | `SChapter` `date_upload` default | `0` | `0` | T-15a-004 |
| EC-MD-04 | status `0..6` known; `7`, `-1`, `999` | known names; others → Unknown text, raw kept | Unknown text, raw kept | T-15a-002 |
| EC-MD-05 | `UpdateStrategy` stored `2` | `AlwaysUpdate` | `ALWAYS_UPDATE` (getOrElse) | T-15a-007 |
| EC-MD-06 | `Page::update_progress(n, Some(0))` | `-1` | `-1` | T-15a-005 |
| EC-MD-07 | `Page::update_progress(50, Some(200))` | `25` | `25` | T-15a-005 |
| EC-MD-08 | `Page::update_progress(1, Some(3))` | `33` (integer division) | `33` | T-15a-005 |
| EC-MD-09 | `Page::update_progress(u64::MAX, Some(u64::MAX))` | no overflow: compute in u128 then clamp to `100` | `100 * bytesRead` overflows Long for huge values (observed) | T-15a-005 |
| EC-MD-10 | `Page` cache JSON without `imageUrl` | `image_url = None` | `imageUrl = null` (explicitNulls=false) | T-15a-038 |
| EC-MD-11 | `Page` cache JSON without `index` | parse error | parse error (no default for index) | T-15a-038 |
| EC-MD-12 | `Page` cache JSON with extra key | ignored | ignored (`ignoreUnknownKeys`) | T-15a-038 |

### 1.2 Genres and detail merge

| ID | Input | Expected | Kotlin | Task |
|---|---|---|---|---|
| EC-GN-01 | `genre = null` | `genres() == None` | `null` | T-15a-003 |
| EC-GN-02 | `genre = ""` | `None` | `null` (isNullOrBlank) | T-15a-003 |
| EC-GN-03 | `genre = "  "` | `None` | `null` | T-15a-003 |
| EC-GN-04 | `genre = "Action, Action, Drama"` | `["Action","Drama"]` (first occurrence order) | same | T-15a-003 |
| EC-GN-05 | `genre = "Action,Drama"` (no space) | `["Action,Drama"]` (one item; split is on `", "` only) | same | T-15a-003 |
| EC-GN-06 | `genre = " , Action ,  "` | `["Action"]` after trim + blank filter | same | T-15a-003 |
| EC-MR-01 | `copy_from` with `author = None` | keep stored author | keep | T-15a-036 |
| EC-MR-02 | `copy_from` with `genre = Some("")` (metadata path) | genres cleared | cleared (observed) | T-15a-036 |
| EC-MR-03 | refresh path with `genre = Some("")` | genres kept (coalesce) | kept | T-15a-036 |
| EC-MR-04 | refresh path with `artist = Some("")` | artist becomes `""` | `""` | T-15a-036 |
| EC-MR-05 | `initialized` merge `true && false` | `false` (metadata path) | `false` | T-15a-036 |
| EC-MR-06 | `thumbnail_url = Some("")` (refresh) | `thumbnailUrl` unchanged; cover not refreshed | same (`takeIf notEmpty`, cover `null`) | T-15a-036 |
| EC-MR-07 | `toSManga` → `toDomainManga` round trip with `OnlyFetchOnce` | `OnlyFetchOnce` kept | `AlwaysUpdate` (observed hazard) | T-15a-036 |

### 1.3 URLs

| ID | Input | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-URL-01 | `https://a.test/x/y?q=1#f` | path `/x/y`, `?q=1`, `#f` | same | T-15a-024 |
| EC-URL-02 | `https://a.test/a b/c` | `/a%20b/c` (encoded form kept) | `/a b/c` (decoded, observed) | T-15a-024 |
| EC-URL-03 | `https://a.test/a%2Fb` | `/a%2Fb` | `/a/b` (decoded, observed) | T-15a-024 |
| EC-URL-04 | `mailto:x@y.test` (opaque) | returned unchanged | `"null"` text (observed) | T-15a-024 |
| EC-URL-05 | not a URL (`::::`) | returned unchanged | returned unchanged (URISyntaxException) | T-15a-024 |
| EC-URL-06 | `baseUrl = "https://a.test"`, `url = "/x"` | `https://a.test/x` | same | T-15a-020 |
| EC-URL-07 | `baseUrl = "https://a.test/"` (trailing slash) and `url = "/x"` | `https://a.test//x` (contract forbids trailing slash; Kurobayomi logs a warning and strips one slash) | `https://a.test//x` (observed) | T-15a-020 |
| EC-URL-08 | `page.url` relative (`"img/1.jpg"`) for image URL request | `SourceError::Url` | `IllegalArgumentException` | T-15a-027 |
| EC-URL-09 | `image_url = None` at image request | `SourceError::MissingImageUrl` | `NullPointerException` (`!!`) | T-15a-027 |

### 1.4 HTTP status and transport

| ID | Condition | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-HT-01 | status 200 | parsed | parsed | T-15a-023 |
| EC-HT-02 | status 204 | `ensure_success` passes; parser decides (empty list) | `isSuccessful` true | T-15a-023 |
| EC-HT-03 | status 301 → 200 | final URL used as base URI | final URL | T-15a-023 |
| EC-HT-04 | status 404 | `Http(404)`, body not parsed | `HttpException(404)` | T-15a-023 |
| EC-HT-05 | status 503 | `Http(503)` | `HttpException(503)` | T-15a-023 |
| EC-HT-06 | status 599 (unusual) | `Http(599)` | `HttpException(599)` | T-15a-023 |
| EC-HT-07 | connect timeout | `Transport(msg)` | `IOException` | T-15a-023 |
| EC-HT-08 | body read error mid-stream on image | image state `Error`, temp file deleted, retry | `IOException` → downloader retry | T-15a-028 |
| EC-HT-09 | image with `Content-Length` absent | progress `-1` per chunk, final `done` | `-1` | T-15a-028 |
| EC-HT-10 | image of 30 h+ transfer | cancelled by the 30 h limit | `callTimeout` 30 h | T-15a-028 |

### 1.5 Parsing helpers

| ID | Input | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-PH-01 | `select_text` no match | `None` (caller applies default) | `defaultValue` | T-15a-032 |
| EC-PH-02 | `select_text` matched empty element | `Some("")` | `""` (not default) | T-15a-032 |
| EC-PH-03 | text `"  A \u{00A0} B  "` | `"A B"` (NBSP collapsed; verify jsoup 1.22.2) | `"A B"` | T-15a-032 |
| EC-PH-04 | `select_int` text `"12"` | `12` | `12` | T-15a-032 |
| EC-PH-05 | `select_int` text `"+7"` | `7` | `7` | T-15a-032 |
| EC-PH-06 | `select_int` text `""` | `SourceError::Parse` | `NumberFormatException` | T-15a-032 |
| EC-PH-07 | `select_int` no match | default | default | T-15a-032 |
| EC-PH-08 | `attr_or_text(el, "href")` with child `<a href>` only | `""` (attribute of the element itself) | `""` | T-15a-032 |
| EC-PH-09 | `attr_or_text(el, "text")` | element text | element text | T-15a-032 |
| EC-PH-10 | `has_next_page` with `None` selector | `false` | `false` | T-15a-031 |
| EC-PH-11 | `has_next_page` selector matches nothing | `false` | `false` | T-15a-031 |
| EC-PH-12 | `has_next_page` matches an element with no text | `true` (presence only) | `true` | T-15a-031 |
| EC-PH-13 | list selector matches zero elements, next selector matches | empty list with `has_next_page = true` | same | T-15a-031 |
| EC-PH-14 | one element mapping fails | whole page `Err(Parse)` | exception fails whole page | T-15a-031 |
| EC-PH-15 | invalid CSS in a selector hook | `Err(Parse)` | `SelectorParseException` | T-15a-030 |
| EC-PH-16 | relative `href` with `<base href>` in document | Kurobayomi resolves against `base` element (jsoup `abs:` does too) — test the same | jsoup behaviour | T-15a-033 |
| EC-PH-17 | `html()` on a body with BOM | BOM removed, parsed normally | Jsoup strips BOM (verify) | T-15a-033 |
| EC-PH-18 | `text()` with non-UTF-8 charset header `windows-1252` | decoded with that charset | OkHttp `body.string()` charset | T-15a-033 |
| EC-PH-19 | body `null` in `json()` (empty body) | `Err(Parse)` | exception | T-15a-033 |

### 1.6 Chapter and page lists

| ID | Input | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-CH-01 | chapter list `[]` for non-local source | `NoChaptersException` ("No chapters found"), DB untouched | same | T-15a-035 |
| EC-CH-02 | duplicate URLs `[a, b, a]` | `[a, b]`, `source_order` 0, 1 | same (`distinctBy`) | T-15a-035 |
| EC-CH-03 | new chapters `[c3, c2, c1]`, all `dateUpload = 0` | `dateUpload = now` for the first, then `maxSeen` (0 → now) | same | T-15a-035 |
| EC-CH-04 | `dateFetch` of new chapters | first gets `now + n`, last gets `now + 1` | same | T-15a-035 |
| EC-CH-05 | `prepare_new_chapter` rewrites `url` | chapter treated as new (lookup happens after hook) | same (observed hazard) | T-15a-026 |
| EC-CH-06 | `prepare_new_chapter` on a local source | not called | not called | T-15a-026 |
| EC-CH-07 | chapter number `1.1f` | stored as `1.1` | `1.100000023841858` (observed) | T-15a-004 |
| EC-CH-08 | name `"  One Piece - Chapter 1 "` with title `One Piece` | `"Chapter 1"` | same | T-15a-035 |
| EC-CH-09 | name starts with title in different case | unchanged | unchanged (`removePrefix` is case-sensitive) | T-15a-035 |
| EC-CH-10 | scanlator `"  "` | `None` | `null` | T-15a-035 |
| EC-CH-11 | scanlator `" Group X "` | `"Group X"` | `"Group X"` | T-15a-035 |
| EC-PL-01 | page list `[]` in downloader | `EmptyPageList` → "No pages found" | `page_list_empty_error` | T-15a-027 |
| EC-PL-02 | page list `[p(index=7)]` | page index 0 | reindexed to 0 (position) | T-15a-034 |
| EC-PL-03 | reader cache hit for the page list | the cached list is used as is; image failures never invalidate the page list (only image-cache eviction re-queues a page) | same (HttpPageLoader.getPages cache first; loadPage QUEUE only on eviction) | T-15a-034 |

### 1.7 Images and data saver

| ID | Condition | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-IM-01 | image already in cache | no network call; READY | same | T-15a-027 |
| EC-IM-02 | cache evicted for a READY page at `loadPage` | page → QUEUE, re-downloaded | same | T-15a-027 |
| EC-IM-03 | ERROR page subscribed again | QUEUE, retried | same | T-15a-027 |
| EC-IM-04 | user retry | ERROR → QUEUE, priority 2 | same | T-15a-027 |
| EC-IM-05 | data saver `NONE` | original URL | same | T-15a-029 |
| EC-IM-06 | data saver server blank (BANDWIDTH_HERO) | original URL | same | T-15a-029 |
| EC-IM-07 | BANDWIDTH_HERO URL already contains the server | original URL | same | T-15a-029 |
| EC-IM-08 | BANDWIDTH_HERO server `https://bw.test/` (trailing slash) | `https://bw.test/?jpg=..` (single slash) | same (`trimEnd('/')`) | T-15a-029 |
| EC-IM-09 | WSRV_NL with `.webp` and `format = true` | `…&output=jpg&q=80&n=-1` | same | T-15a-029 |
| EC-IM-10 | WSRV_NL URL with `&` in query | Kurobayomi encodes the URL (documented change) | not encoded (observed) | T-15a-029 |
| EC-IM-11 | RESMUSH_IT response without `"dest":"` | `Parse` error | returns whole text as URL (observed) | T-15a-029 |
| EC-IM-12 | data saver excluded for this source id | `NoOp` | same | T-15a-029 |
| EC-IM-13 | data saver applied in downloader with `data_saver_downloader = false` | `NoOp` | same | T-15a-029 |
| EC-IM-14 | page image download fails 3 times then succeeds on 4th | READY | same (`retryWhen` attempt < 3) | T-15a-028 |
| EC-IM-15 | fails 4 times | ERROR + notification with message | same | T-15a-028 |
| EC-IM-16 | cancellation during download | temp file deleted, no ERROR state | same (CancellationException rethrown) | T-15a-028 |
| EC-IM-17 | filename width for 5 pages | `001`..`005` (digits = max(3, len("5"))) | same | T-15a-028 |
| EC-IM-18 | filename width for 1234 pages | 4 digits (`0001`) | same | T-15a-028 |

### 1.8 Filters

| ID | Input | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-FI-01 | `Select` state 5 with 3 values | no panic; display empty; `select_value()` None | `IndexOutOfBoundsException` (composition) | T-15a-039 |
| EC-FI-02 | `TriState` raw 3 | `Parse` error at construction from raw | `IllegalStateException` (UI) | T-15a-009 |
| EC-FI-03 | `Sort` state `None`, user taps row 2 | `Selection(2, true)` | `NullPointerException` (observed) | T-15a-010 |
| EC-FI-04 | `Sort` state `Selection(1, true)`, tap row 1 | `Selection(1, false)` | same | T-15a-010 |
| EC-FI-05 | `Sort` state `Selection(1, false)`, tap row 0 | `Selection(0, false)` (direction kept) | same | T-15a-010 |
| EC-FI-06 | `Text` typed character by character | state updated per keystroke | same (no debounce) | T-15a-039 |
| EC-FI-07 | Group with nested Group with nested TriState | recursive rendering and state write | same | T-15a-039 |
| EC-FI-08 | `searchGenre` no match | listing query = genre name, filters defaults | same | T-15a-040 |
| EC-FI-09 | `searchGenre` matching `Select` value | `Select.state = index` and query null | same (index computed on the filtered list; differs only with non-string values) | T-15a-040 |
| EC-FI-10 | Reset while a search is running | filters replaced; running search unchanged until Filter | same | T-15a-040 |
| EC-FI-11 | `FilterList` equality | structural in Rust | always false in Kotlin | T-15a-011 |

### 1.9 Source preferences and settings

| ID | Condition | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-PR-01 | key `source_<id>` for id `4851548778462321757` | `source_4851548778462321757` | same | T-15a-019 |
| EC-PR-02 | read absent key | `None` → schema default | default | T-15a-019 |
| EC-PR-03 | schema `Unsupported` widget | row shown as Info; never stored | Kotlin widget as declared | T-15a-018 |
| EC-PR-04 | backup of a source with zero prefs | omitted from backup | omitted (`filter { it.prefs.isNotEmpty() }`) | T-15a-019 |
| EC-PR-05 | backup value of unsupported type (for example `Double`) | omitted | omitted (`else -> null`) | T-15a-019 |
| EC-PR-06 | backup key `isAppState` | omitted | omitted | T-15a-019 |
| EC-PR-07 | restore into a source id not installed | stored under `source_<id>`, shown when installed | same | T-15a-019 |

### 1.10 Deep links

| ID | Condition | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-DL-01 | no source resolves | `NoResults` | same | T-15a-017 |
| EC-DL-02 | manga resolves, chapter does not | `Result(manga)` | same | T-15a-017 |
| EC-DL-03 | `getManga` throws | `NoResults` and log (Kurobayomi) | exception propagates out of the coroutine (observed) | T-15a-017 |
| EC-DL-04 | two sources resolve the same URI | first in list order | first | T-15a-017 |

### 1.11 Warnings and unmetered sources

| ID | Condition | Expected | Kotlin (observed) | Task |
|---|---|---|---|---|
| EC-UM-01 | 16 chapters queued from one metered source | warning (15 per source) | same | T-15a-016 |
| EC-UM-02 | 16 chapters from one unmetered source | no warning | same | T-15a-016 |
| EC-UM-03 | 31 queued total across metered sources | warning (30 total) | same | T-15a-016 |
| EC-UM-04 | queue non-empty before add | no warning | same (`wasEmpty`) | T-15a-016 |
| EC-UM-05 | library update: 61 manga from one metered source | warning (60) | same | T-15a-016 |
| EC-UM-06 | library update source lookup returns null | counted as metered (`null !is UnmeteredSource`) | same | T-15a-016 |

## 2. Failure modes and handling

| Failure | Where detected | Handling (Kurobayomi) | User sees |
|---|---|---|---|
| DNS / connect / TLS | transport | `SourceError::Transport`; reader page ERROR; download ERROR + notification | page error with retry; download error row |
| HTTP 4xx/5xx | `ensure_success` | `SourceError::Http(code)`; never parsed | "HTTP error 404" text (formatter) |
| Malformed HTML | parser | `Parse` error; the list page fails; no partial list | error message in browse |
| Missing selector match | parser | empty list or `None` value; caller decides | "No results found" (empty page) |
| Invalid URL from a parser | `Url::parse` | `SourceError::Url`; no request sent | error message |
| Source panic | the source call runs in a spawned tokio task; `JoinError::is_panic()` at the adapter boundary (T-15a-013) | converted to `SourceError::Parse("source panicked")`; the source is marked failed for 60 s (proposal) | error message; source still listed |
| Empty chapter list | sync | `NoChaptersException`, DB untouched | "No chapters found" |
| Image 3x fails | downloader | ERROR + notification | notification with message |
| Settings read failure (DB) | prefs store | return default; log | default value shown |
| Cancellation | tokio | `SourceError::Cancelled` never shown | nothing |

## 3. Race conditions and ordering hazards

| ID | Race | Consequence if naive | Required behaviour (Kurobayomi) | Task |
|---|---|---|---|---|
| RC-01 | reader `loadPage` and worker both touch a page's status | state skips or duplicates a load | status changes only in the worker, except QUEUE enqueue; `loadPage` sets QUEUE under a mutex | T-15a-027 |
| RC-02 | user `retryPage` while the page is in flight | two downloads of the same image | `retryPage` only enqueues if state is ERROR; in-flight pages ignore it | T-15a-027 |
| RC-03 | reader closed while queued | queue keeps dead requests | `CancellationToken` clears QUEUE entries; in-flight dropped | T-15a-027 |
| RC-04 | downloader `flatMapMerge(2)` writes the same `tmp` name | corrupt file | file name derived from page number; pages distinct | T-15a-028 |
| RC-05 | data saver setting changed during download | mixed URLs | setting read once at download start (`DataSaver(...)` created before the loop) | T-15a-029 |
| RC-06 | filter sheet edits while search runs | running listing mutates | listing filters are cloned at search time (Kurobayomi); Kotlin shares the object | T-15a-040 |
| RC-07 | `Sort` state mutated in the UI while the source reads it | torn read | `FilterList` passed by reference to a `search` snapshot clone | T-15a-040 |
| RC-08 | sync and library update for the same manga | duplicate inserts | sync takes a per-manga async mutex | T-15a-035 |
| RC-09 | `prepare_new_chapter` url rewrite causes duplicate chapters | duplicates | documented; a test asserts the observed rule | T-15a-026 |
| RC-10 | source preference write from settings while the source reads it | reads old value | reads are cheap; the source reads on each call (no caching) | T-15a-019 |
| RC-11 | `headers` captured at first access, UA preference changed later | old UA for that source | Kurobayomi captures UA at source construction; a UA change applies on next start (documented) | T-15a-021 |
| RC-12 | two windows open the same manga | double detail fetch | request de-duplication by `(source, url)` | T-15a-013 |

## 4. Fixtures required

| Fixture | Content | Used by |
|---|---|---|
| `fixtures/source/list_popular.html` | 3 manga cards with title link, `img src` and a `li.next a` next link | T-15a-031 |
| `fixtures/source/list_popular_no_next.html` | same without next link | T-15a-031 |
| `fixtures/source/list_empty.html` | empty container and next link present | EC-PH-13 |
| `fixtures/source/details.html` | title, author, genre list with `, ` separators, status word, thumbnail | T-15a-036 |
| `fixtures/source/chapters.html` | 5 chapter rows; one with NBSP in name; one duplicate URL; one with date text | T-15a-035 |
| `fixtures/source/pages.html` | 3 `img` elements in document order | T-15a-034 |
| `fixtures/source/image_url.html` | one `img` with relative `src` and `<base href>` | EC-PH-16 |
| `fixtures/source/windows1252.html` | body bytes in windows-1252 with header `charset=windows-1252` | EC-PH-18 |
| `fixtures/http/redirect/*` | wiremock: 301 chain, 404, 503, slow body, body without Content-Length | T-15a-023, T-15a-028 |
| `fixtures/id/vectors.json` | the three id vectors from behaviour 1.1 | T-15a-012 |
| `fixtures/filters/*.json` | one `FilterList` per kind, plus a nested group and out-of-range values | T-15a-039, T-15a-040 |
| `fixtures/prefs/source_<id>.json` | one value of each `PrefValue` type | T-15a-019 |
| `fixtures/sync/*.json` | sequences of source chapter lists and DB states for the sync rules | T-15a-035 |
| `fixtures/datasaver/*.txt` | expected compressed URLs for each mode (behaviour topic data saver section) | T-15a-029 |

Fixture rules: real pages from at least two live sites per supported source family are saved once and checked in; personal data is not stored; no fixture is generated from the code under test.

## 5. Tests to write (new, all in Rust)

| Test ID | Name | Asserts | Crate |
|---|---|---|---|
| TST-01 | `generate_id_vectors` | EC-ID-01..03 | kuro-source |
| TST-02 | `generate_id_non_negative` | EC-ID-04 for 1000 generated names | kuro-source |
| TST-03 | `manga_status_round_trip` | EC-MD-04 | kuro-source |
| TST-04 | `genres_split_rules` | EC-GN-01..06 | kuro-source |
| TST-05 | `page_progress_rules` | EC-MD-06..09 | kuro-source |
| TST-06 | `page_cache_json_shape` | EC-MD-10..12 | kuro-source |
| TST-07 | `filter_equality_and_hash` | EC-FI-11 and name+state equality | kuro-source |
| TST-08 | `tristate_raw_mapping` | EC-FI-02 | kuro-source |
| TST-09 | `sort_selection_rules` | EC-FI-03..05 | kuro-source |
| TST-10 | `select_bounds` | EC-FI-01 | kuro-source |
| TST-11 | `url_without_domain` | EC-URL-01..05 | kuro-source-http |
| TST-12 | `base_url_join` | EC-URL-06..09 | kuro-source-http |
| TST-13 | `ensure_success_codes` | EC-HT-01..06 | kuro-source-http |
| TST-14 | `transport_timeout` | EC-HT-07 with a black-hole server | kuro-source-http |
| TST-15 | `progress_without_length` | EC-HT-09 | kuro-source-http |
| TST-16 | `html_text_normalisation` | EC-PH-03 | kuro-source-http |
| TST-17 | `select_int_rules` | EC-PH-04..07 | kuro-source-http |
| TST-18 | `attr_or_text_rules` | EC-PH-08..09 | kuro-source-http |
| TST-19 | `list_page_parse_rules` | EC-PH-10..14 | kuro-source-http |
| TST-20 | `invalid_selector_error` | EC-PH-15 | kuro-source-http |
| TST-21 | `base_element_resolution` | EC-PH-16 | kuro-source-http |
| TST-22 | `charset_decoding` | EC-PH-18, EC-PH-19 | kuro-source-http |
| TST-23 | `chapter_list_distinct_and_order` | EC-CH-02..04 | kuro-app (domain) |
| TST-24 | `chapter_number_precision` | EC-CH-07 | kuro-app (domain) |
| TST-25 | `chapter_name_sanitize` | EC-CH-08..09 | kuro-app (domain) |
| TST-26 | `page_reindex` | EC-PL-02 | kuro-source-http |
| TST-27 | `reader_state_machine` | EC-IM-01..04 with a fake source | kuro-app (reader) |
| TST-28 | `downloader_retry_schedule` | EC-IM-14..16 with paused tokio time | kuro-app (downloads) |
| TST-29 | `file_name_width` | EC-IM-17..18 | kuro-app (downloads) |
| TST-30 | `data_saver_modes` | EC-IM-05..13 (`fixtures/datasaver`) | kuro-source-http |
| TST-31 | `settings_defaults_and_keys` | EC-PR-01..03 | kuro-source |
| TST-32 | `backup_filter_rules` | EC-PR-04..06 | kuro-app (backup) |
| TST-33 | `deeplink_resolution` | EC-DL-01..04 with fake resolvers | kuro-app |
| TST-34 | `unmetered_thresholds` | EC-UM-01..06 | kuro-app (downloads, library) |
| TST-35 | `race_retry_while_in_flight` | RC-02 with two concurrent calls | kuro-app (reader) |
| TST-36 | `race_cancel_clears_queue` | RC-03 | kuro-app (reader) |
| TST-37 | `source_panic_isolated` | adapter catches a panic and returns `Parse` | kuro-source-http |

Each test must fail on the Kotlin-observed behaviour where the table says "documented change", and pass on the Kurobayomi behaviour. Tests for behaviour that matches Kotlin pass on both.

## 6. Performance expectations (proposal, measured in T-15a-013/T-15a-027)

- Parsing a 500 KB list page with 60 items: under 50 ms in release builds on a 4-core laptop; above 262 144 bytes parse on the blocking pool (behaviour 18).
- Reader: next image is requested within 50 ms of the current page becoming visible when its state is QUEUE.
- Download: two pages in flight; no more than two `tmp` files per download at any moment (`ls` test).
- Filter sheet with 200 filter rows: first paint under 100 ms (lazy rows).

## 7. Manual test script (for T-15a-046 acceptance on the real GTK app)

1. Open a source, browse popular, search with text, open a manga, read a chapter. Expected: pages appear in order, errors show the formatted messages in section 2.
2. Open the filter sheet; exercise every kind (section 2.2 of the UI doc); Reset, then Filter. Expected: Sort tap rules, TriState cycle, Text updates per key.
3. Open the source settings; change each `SourceSetting` kind; restart; values persist.
4. Block the network mid-chapter; expected: image ERROR states, retry on click.
5. Turn on each data saver mode with a test server; expected: request URL in the log is the transformed one.
