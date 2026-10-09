# Topic: HTTP source pipeline (request, parse, page images, data saver)

Scope: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt`, `ParsedHttpSource.kt`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsoupExtensions.kt`, and the callers that drive it. Source of truth for the request rules: `p1-behavior.md` sections 2-5. This topic gives the step-by-step flow and state machines.

## 1. The seven operations and their call chains (OBSERVED)

| Operation | Public suspend API | Chain inside `HttpSource` | Success check | Parse |
|---|---|---|---|---|
| Popular | `CatalogueSource.getPopularManga(page)` | `fetchPopularManga` → `client.newCall(popularMangaRequest(page)).asObservableSuccess()` | non-2xx → `HttpException(code)` | `popularMangaParse(response)` |
| Search | `getSearchManga(page, query, filters)` | `fetchSearchManga` → `Observable.defer { client.newCall(searchMangaRequest(page, query, filters)).asObservableSuccess() }` (catches `NoClassDefFoundError`, rethrows `RuntimeException`) | same | `searchMangaParse` |
| Latest | `getLatestUpdates(page)` | `fetchLatestUpdates` → `latestUpdatesRequest(page)` | same | `latestUpdatesParse` |
| Details | `getMangaDetails(manga)` | `fetchMangaDetails` → `mangaDetailsRequest(manga)` | same | `mangaDetailsParse(response).apply { initialized = true }` |
| Chapter list | `getChapterList(manga)` | `fetchChapterList` → `chapterListRequest(manga)` | same | `chapterListParse(response)` |
| Page list | `getPageList(chapter)` | `fetchPageList` → `pageListRequest(chapter)` | same | `pageListParse(response)` |
| Image URL | `getImageUrl(page)` | `fetchImageUrl` → `imageUrlRequest(page)` | same | `imageUrlParse(response)` |
| Image bytes | `getImage(page)` | `client.newCachelessCallWithProgress(imageRequest(page), page).awaitSuccess()` | same (`awaitSuccess`) | none (returns the `Response`) |

Notes:
- `getPopularManga`, `getSearchManga`, `getLatestUpdates`, `getMangaDetails`, `getChapterList`, `getPageList` are default suspend methods that call their Rx `fetch*` and `awaitSingle()`. The two layers add no logic. Kurobayomi keeps one async function per operation (`p1-rust-design.md` section 8.3).
- `getImage` returns the raw `okhttp3.Response`. The caller reads `response.body` (in `ChapterCache.putImageToCache`, and in the downloader `saveTo`). Kurobayomi returns `bytes::Bytes` or a streamed body.

## 2. Request construction (exact)

Order of operations for every request built by the default builders:
1. Start from the base string: `baseUrl` (no trailing slash by contract) plus the stored path (`manga.url`, `chapter.url`) — or, for image URLs, the stored absolute URL (`page.url`, `page.imageUrl`).
2. `toHttpUrl()` (OkHttp) — parse; failure throws `IllegalArgumentException`.
3. Attach `headers` (the lazily captured `Headers` of the source, containing `User-Agent` by default).
4. Attach `CacheControl` `max-age=600` (`DEFAULT_CACHE_CONTROL`, Requests.kt:12). Only meaningful with an OkHttp cache (area:12).
5. Method is always `GET` for the default builders (`POST` only through the `POST` helper, which no HttpSource default uses).

Image requests use the same builder but with `newCachelessCallWithProgress`: the client is derived with `cache(null)`, `callTimeout(30, TimeUnit.HOURS)` and a network interceptor wrapping the body in `ProgressResponseBody` (OkHttpExtensions.kt:119-131). The cache header from the request is therefore not used for images.

Rust equivalent (PROPOSAL): `SourceRequest { method, url, headers, body, cache_max_age }`; `HttpClient::execute` for ordinary calls; `HttpClient::execute_streaming` for images (no cache, 30 h timeout, progress callback).

## 3. Response handling (exact)

`asObservableSuccess` (OkHttpExtensions.kt:63-70):
```
if (!response.isSuccessful) { response.close(); throw HttpException(response.code) }
```
`awaitSuccess` (OkHttpExtensions.kt:109-116): same, with the caller's stack trace attached to the exception.
`HttpException(code)` message: `"HTTP error $code"` (HttpException.kt).

The response body is read once: `Response.asJsoup()` calls `body.string()` (whole body, charset from `Content-Type` or UTF-8). Parsers that take a `Response` must therefore read the body exactly once. In Kurobayomi the body is read into `Bytes` by `HttpClient::execute` and parsers borrow it.

Base URI for HTML: `request.url.toString()` = the final URL after redirects (`Response.request` is the last request in OkHttp's redirect chain).

## 4. Parsed list pages (ParsedHttpSource) — exact flow

Applies to popular, search and latest with their own selector hooks:

```
document = response.asJsoup()
mangas   = document.select(<kind>Selector()).map { <kind>FromElement(it) }   // no try/catch; any exception fails the page
hasNext  = <kind>NextPageSelector()?.let { document.select(it).first() } != null
return MangasPage(mangas, hasNext)
```

Edge cases (OBSERVED):
- Empty match list → `mangas = []`, `hasNextPage` from the next-page selector only (a source can report a next page with zero items).
- Selector returns `null` → `hasNextPage = false` (because `null != null` is `false`).
- `select(...)` with an invalid selector string → Jsoup throws `Selector.SelectorParseException`; the page fails.
- The element hooks may return a manga with `url` or `title` unset? In Kotlin `url` and `title` are `lateinit` on `SMangaImpl`: reading an unset value throws `UninitializedPropertyAccessException`. Kurobayomi requires both at construction (`SManga::new(url, title)`), so the same mistake fails at the source with a parse error instead of at a later read.

## 5. Parsed details, chapter list, page list, image URL (ParsedHttpSource)

- Details: `mangaDetailsParse(response.asJsoup())` → subclass hook `mangaDetailsParse(document)`; `initialized = true` set afterwards by `HttpSource.fetchMangaDetails`.
- Chapter list: `document.select(chapterListSelector()).map { chapterFromElement(it) }` — document order, no reversal (behaviour 4.3).
- Page list: subclass hook `pageListParse(document)`; list order is reading order for the reader; the app reindexes.
- Image URL: subclass hook `imageUrlParse(document)` returns an absolute URL string.

## 6. Element helpers (JsoupExtensions.kt) — exact semantics

| Helper | Kotlin | Behaviour |
|---|---|---|
| `selectText(css, defaultValue = null)` | `select(css).first()?.text() ?: defaultValue` | Returns `defaultValue` only when NO element matches. An element with empty text returns `""`, not the default. Text is Jsoup-normalised (behaviour topic section 6.1 below). |
| `selectInt(css, defaultValue = 0)` | `select(css).first()?.text()?.toInt() ?: defaultValue` | No match → default. Matching element with non-numeric or empty text → `NumberFormatException` (not the default). `toInt()` accepts a leading `+` or `-` and digits only; surrounding whitespace is already removed by `text()`. |
| `attrOrText(css)` | `if (css != "text") attr(css) else text()` | Operates on THIS element, not a child: `attr(css)` returns the attribute value of the element itself (empty string when absent). The literal argument `"text"` returns the element's text instead. |
| `asJsoup(html = null)` | `Jsoup.parse(html ?: body.string(), request.url.toString())` | See section 3. |

### 6.1 Jsoup text normalisation (what `text()` produces)
- Jsoup `Element.text()` collapses every run of whitespace characters into one ASCII space and removes leading and trailing whitespace. It is the normalisation used by all `selectText` results and every `text()` call in extensions.
- Characters treated as whitespace by Jsoup's `StringUtil` include space, tab, LF, FF, CR and U+00A0 (no-break space). Verify against the Jsoup version pinned in `gradle/libs.versions.toml` (line 31: `jsoup = "org.jsoup:jsoup:1.22.2"`) with a fixture containing NBSP before relying on it (test T-15a-033).
- Kurobayomi helper `html::text(el)` (PROPOSAL): concatenates descendant text nodes in document order, then collapses the whitespace set above into single spaces and trims.

### 6.2 Kurobayomi helpers (PROPOSAL)
```rust
pub fn text(el: scraper::ElementRef) -> String;                       // jsoup-normalised
pub fn select_text(el: scraper::ElementRef, css: &str) -> Result<Option<String>, SourceError>; // None when no match
pub fn select_int(el: scraper::ElementRef, css: &str, default: i32) -> Result<i32, SourceError>;
pub fn attr_or_text(el: scraper::ElementRef, css: &str) -> String;     // "text" -> text(el), else el.attr(css) or ""
```
`select_text` returns `Ok(None)` for no match (Kotlin `defaultValue` is applied by the caller), and `Ok(Some(""))` for an empty match, matching Kotlin.

## 7. Page list and reindexing (exact)

Reader (`HttpPageLoader.getPages`, HttpPageLoader.kt:~81-89):
1. Try `chapterCache.getPageListFromCache(chapter)`.
2. On any `Throwable` except `CancellationException`, call `source.getPageList(chapter.chapter)`.
3. Map each source page to `ReaderPage(index = position, url = page.url, imageUrl = page.imageUrl)`.

Downloader (MangaDownloader.kt around lines 352-371):
1. If `download.pages` already exists, reuse it (resume).
2. Otherwise `download.source.getPageList(chapter)`; empty list → `Exception(page_list_empty_error)`.
3. `reIndexedPages = pages.mapIndexed { index, page -> ...}` (the comment says "Don't trust index from source").
4. `download.pages = reIndexedPages`.

Proposal: one `get_page_list` in `Http<S>` does the reindex (T-15a-034), and the reader and downloader both receive reindexed pages.

Page list cache (reader): on `HttpPageLoader.recycle()` for online chapters, `pagesToSave = pages.map { Page(it.index, it.url, it.imageUrl) }` written to `ChapterCache` (key `md5("${mangaId}${chapterUrl}")`, JSON). Only `index`, `url`, `imageUrl` are stored (transient fields are not). Read back on the next open (step 1 above). A cache read failure is ignored and falls back to the network.

## 8. Image URL resolution and image download — reader state machine (OBSERVED)

Reader components (HttpPageLoader.kt):
- One background coroutine consumes a `PriorityBlockingQueue<PriorityPage>` and processes each page through `internalLoadPage` sequentially (because `collect` is sequential). Concurrency = 1 image per reader.
- `loadPage(page)` (called when the page becomes visible/needed):
  1. If `page.status == READY` and `imageUrl != null` and `!chapterCache.isImageInCache(imageUrl)` → `status = QUEUE` (cache was evicted).
  2. If `status == ERROR` → `status = QUEUE` (automatic retry when the page is subscribed).
  3. If `status == QUEUE` → enqueue the page with priority 1.
  4. Enqueue up to `preloadSize = 4` following pages that are `QUEUE`, with priority 0 (`preloadNextPages`, pages after `pageIndex`, `subList(pageIndex+1, min(pageIndex+1+4, size))`).
  5. Suspend until cancelled; on cancellation remove still-`QUEUE` entries from the queue.
- `retryPage(page)` (user action): if `ERROR` → `QUEUE`; enqueue with priority 2.
- `internalLoadPage(page)`:
  1. If `imageUrl` is null or empty: `status = LOAD_PAGE`; `imageUrl = source.getImageUrl(page)`.
  2. `imageUrl!!`.
  3. If `!chapterCache.isImageInCache(imageUrl)`: `status = DOWNLOAD_IMAGE`; `chapterCache.putImageToCache(imageUrl, source.getImage(page, dataSaver))`.
  4. `page.stream = chapterCache.getImageFile(imageUrl).inputStream`; `status = READY`.
  5. Any `Throwable`: `status = ERROR`; `CancellationException` rethrown.
- Retry policy: none in the reader (a failed page waits for the user or a new subscription). There is no timed retry here, unlike the downloader.

Reader state table:

| From | Event | To | Source call |
|---|---|---|---|
| QUEUE | worker picks it | LOAD_PAGE (only if no imageUrl) | `getImageUrl` |
| LOAD_PAGE | URL resolved | DOWNLOAD_IMAGE (only if not cached) | none |
| LOAD_PAGE | URL failed | ERROR | none |
| DOWNLOAD_IMAGE | bytes stored | READY | `getImage` |
| DOWNLOAD_IMAGE | failed | ERROR | `getImage` |
| READY | cache evicted when loaded again | QUEUE | none |
| ERROR | `loadPage` or `retryPage` | QUEUE | none |
| any | cancelled | unchanged | none |

Priorities and order: heap order by priority desc, then creation order asc (`PriorityPage.compareTo`, HttpPageLoader.kt:215-227; global counter `AtomicInteger`).

## 9. Image download — downloader (OBSERVED)

Per download (MangaDownloader.kt around lines 355-420):
1. Clean `*.tmp` files in the chapter temp directory.
2. `dataSaver = if (sourcePreferences.dataSaverDownloader().get()) DataSaver(download.source, sourcePreferences) else DataSaver.NoOp` (default `data_saver_downloader = true`).
3. `download.status = DOWNLOADING`.
4. `pageList.asFlow().flatMapMerge(concurrency = 2) { page -> flow { ... }.flowOn(Dispatchers.IO) }.collect { notifier.onProgressChange(download) }`.

Per page inside the flow:
1. If `page.imageUrl` is null or empty: `status = LOAD_PAGE`; `imageUrl = source.getImageUrl(page)`; on failure `status = ERROR` (no notification).
2. `getOrDownloadImage(page, download, tmpDir, dataSaver)` (withIOContext):
   - If `page.imageUrl == null` → return (page stays ERROR).
   - `digits = max(3, (download.pages?.size ?: 0).toString().length)`; `filename = "%0${digits}d".format(page.number)`.
   - Delete `"$filename.tmp"` if it exists.
   - Reuse an existing file in the temp dir whose name starts with `"$filename."` or `"${filename}__001"`.
   - Else, if the chapter cache has the image, copy it to `"$filename.tmp"` (`copyImageFromCache`).
   - Else `downloadImage(page, source, tmpDir, filename, dataSaver)`.
   - On success: `splitTallImageIfNeeded(page, tmpDir)`, `page.uri = file.uri`, `progress = 100`, `status = READY`.
   - On `Throwable` (not `CancellationException`): `progress = 0`, `status = ERROR`, `notifier.onError(e.message, chapter.name, manga.title, manga.id)`.

`downloadImage` (MangaDownloader.kt around lines 520-556):
- `status = DOWNLOAD_IMAGE`, `progress = 0`.
- `flow { response = source.getImage(page, dataSaver); file = tmpDir.createFile("$filename.tmp"); throttled copy with bytesPerSecond = downloadSpeedLimit * 1024; extension = getImageExtension(response, file); file.renameTo("$filename.$extension"); emit(file) }`.
- On exception inside the copy: close response, delete the temp file, rethrow.
- Retry: `retryWhen { _, attempt -> if (attempt < 3) { delay((2L shl attempt) * 1000); true } else false }`. So: first attempt, then retries after 2 s, 4 s and 8 s. Four attempts in total.
- `flowOn(Dispatchers.IO)`, `.first()`.

Progress: `ProgressResponseBody` updates the `Page` progress during reader and downloader reads (from the network interceptor); the downloader resets progress to `0` before the request and sets `100` when READY.

Notes:
- Only the first `Throwable` after the retry budget reaches `ERROR`. Each retry re-requests the image with the same (possibly data-saver-transformed) URL.
- `speed limit = 0` means the throttler's own semantics (area:23); Kurobayomi keeps the same `× 1024` conversion.

Proposal for Rust (T-15a-027, T-15a-028):
- Reader: single tokio task with `BinaryHeap` (behaviour 18 priorities), states exactly as the table above.
- Downloader: `buffer_unordered(2)`, the same per-page steps, retry with `tokio::time::sleep(Duration::from_secs(2 << attempt))` for attempt 0..2, the same file naming (`%0{digits}d`) and `.tmp` then rename.
- The state table is enforced by a `fn transition(from: PageState, to: PageState) -> Result<(), InvalidTransition>` in debug builds (asserts), so UI-visible states cannot skip steps.

## 10. Data saver (DataSaver.kt) — full algorithm

Selection (`DataSaver(source, preferences)`, DataSaver.kt:53-64):
1. `mode = preferences.dataSaver().get()`.
2. If `mode != NONE` and `source.id.toString() in preferences.dataSaverExcludedSources().get()` → `NoOp`.
3. `NONE` → `NoOp`; `BANDWIDTH_HERO` → `BandwidthHeroDataSaver(prefs)`; `WSRV_NL` → `WsrvNlDataSaver(prefs)`; `RESMUSH_IT` → `ReSmushItDataSaver(prefs)`.

Application (`HttpSource.getImage(page, dataSaver)`, DataSaver.kt:30-41):
```
imageUrl = page.imageUrl ?: return getImage(page)           // no URL → plain request (which then throws)
page.imageUrl = dataSaver.compress(imageUrl)
try { return getImage(page) } finally { page.imageUrl = imageUrl }
```
The stored `imageUrl` is restored in `finally`, so the page keeps its original URL for caching (the image cache key is the original `imageUrl` from `chapterCache.putImageToCache(imageUrl, ...)` in the reader — note that the cache key uses the ORIGINAL URL because `imageUrl` is captured before the call; OBSERVED in HttpPageLoader.kt:191-200).

Per-mode rules (common): for `.jpeg` or `.jpg` (case-insensitive `contains`), if `ignore_jpeg` is true, return the URL unchanged, otherwise compress. For `.gif`, if `ignore_gif` is true return unchanged, otherwise compress. Any other extension: compress.

BANDWIDTH_HERO:
- `dataSavedServer = data_saver_server.trimEnd('/')`.
- If `dataSavedServer` is blank OR the image URL already contains `dataSavedServer` → unchanged.
- Otherwise: `"$dataSavedServer/?jpg=$format&l=$quality&bw=$colorBW&url=$escaped"` where `format = "1"` if `data_saver_image_format_jpeg` else `"0"`, `quality = data_saver_image_quality` (Int, default 80), `colorBW = "1"` if `data_saver_color_bw` else `"0"`, `escaped = URLEncoder.encode(imageUrl, "utf-8")`.

WSRV_NL:
- Always builds `"https://wsrv.nl/?url=$imageUrl" + tail`, where the tail depends on the URL containing `.webp` or `.gif` (case-insensitive):
  - webp or gif: `format == false` → `&q=$quality&n=-1`; `format == true` → `&output=jpg&q=$quality&n=-1`.
  - otherwise: `format == true` → `&output=jpg&q=$quality`; `format == false` → `&output=webp&q=$quality`.
- `format` = `data_saver_image_format_jpeg`. The image URL is NOT encoded (observed).

RESMUSH_IT:
- Always calls `GET http://api.resmush.it/ws.php?img=$imageUrl&qlty=$quality` synchronously (`client.newCall(...).execute()`), then returns the text between `"dest":"` and the next `",`.
- The image URL is NOT encoded; the scheme is plain `http` (observed). A response without `"dest":"` yields the whole remaining text as URL (`substringAfter` returns the input when the delimiter is absent; observed).
- Kurobayomi fixes: percent-encode the image URL (`form_urlencoded`), use `https`, and return `SourceError::Parse` when the delimiter is absent (documented deliberate difference, `p1-rust-design.md` section 13 item 14).

Preference defaults: `data_saver` NONE, `ignore_jpeg` false, `ignore_gif` true, `data_saver_image_quality` 80, `data_saver_image_format_jpeg` false, `data_saver_server` "", `data_saver_color_bw` false, `data_saver_excluded` empty, `data_saver_downloader` true (SourcePreferences.kt:97-116). Full table in `p1-data-and-config.md` section 4.2.

## 11. Source-side URL helpers used by the pipeline

- `getMangaUrl(manga)` = `mangaDetailsRequest(manga).url.toString()` (HttpSource.kt:433-435).
- `getChapterUrl(chapter)` = `pageListRequest(chapter).url.toString()` (HttpSource.kt:444-446).
- These are used for WebView, share, ComicInfo `Web` tags (behaviour 7). They build requests without performing them.
- `setUrlWithoutDomain` is called by extensions while parsing (behaviour 6).

## 12. Cancellation and errors across the pipeline (summary)

| Situation | Reader | Downloader | Rust (PROPOSAL) |
|---|---|---|---|
| Reader closed | loader cancelled; QUEUE entries removed; in-flight request cancelled by coroutine cancellation | n/a | `CancellationToken` cancels the loader task; in-flight `reqwest` future dropped |
| Page list 404 | `HttpPageLoader.getPages` has no fallback after a failed network call (the exception propagates to the reader's page-list load) | the exception propagates out of the downloader's page-list step (MangaDownloader.kt around lines 341-352), so the download does not start | `SourceError::Http(404)` returned to caller |
| Image URL fails | `ERROR` (retry on next subscription) | `ERROR`, no notification, page skipped (behaviour 3.3) | same states; downloader also emits a log, not a notification (PROPOSAL: keep the observed behaviour but surface a count in the download row) |
| Image bytes fail | `ERROR` | retried 3 times (2/4/8 s) then `ERROR` + notification | same |
| Empty page list | reader shows empty list | `page_list_empty_error` | `SourceError::EmptyPageList` |
| Data saver URL invalid | fetch error | fetch error | `SourceError::Url` |

## 13. Test vectors for this pipeline (for T-15a-048)

- Redirect: `GET https://example.test/manga/a` → 301 to `https://example.test/series/a/` → 200. `HttpResponse.url` must be `https://example.test/series/a/`; relative `abs:` links resolve against it.
- Non-2xx: 404 and 503 → `SourceError::Http(404)` and `Http(503)`; the body is not parsed.
- Image with 0 `Content-Length`: progress = -1 for every chunk, final chunk `done = true`.
- Data saver BANDWIDTH_HERO with server `https://bw.example.test/` (trailing slash) and URL `https://cdn.example.test/a b.jpg`: result `https://bw.example.test/?jpg=0&l=80&bw=0&url=https%3A%2F%2Fcdn.example.test%2Fa+b.jpg`.
- Data saver WSRV_NL, `format = false`, URL `https://cdn.example.test/x.webp`: result `https://wsrv.nl/?url=https://cdn.example.test/x.webp&q=80&n=-1`.
- Data saver with `ignore_gif = true` and URL ending `.GIF`: unchanged.
