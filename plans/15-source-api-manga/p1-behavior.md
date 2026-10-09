# Area 15 — Manga source API: behaviour spec

OBSERVED = what the Kotlin code does (cited as repo-relative path:line). PROPOSAL = what Kurobayomi should do; always labelled `Proposal:`.

Deep dives: `p1-topic-http-source-pipeline.md` (request, parse, image, data saver), `p1-topic-filters.md` (filter model and UI), `p1-topic-models-and-sync.md` (SManga/SChapter/Page mapping into the database, chapter sync, library update rules).

## 0. Call graph of the scope

- The app calls the public suspend API only: `CatalogueSource.getPopularManga/getSearchManga/getLatestUpdates`, `MangaSource.getMangaDetails/getChapterList/getPageList`, `HttpSource.getImageUrl/getImage`, `CatalogueSource.getFilterList`, `HttpSource.getMangaUrl/getChapterUrl`, `HttpSource.prepareNewChapter`, `ConfigurableSource.setupPreferenceScreen/sourcePreferences`, `ResolvableSource.getUriType/getManga/getChapter`.
- Callers (all outside the scope, verified by grep):
  - `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt` (page list and image loading for the reader).
  - `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt` (page list, image URL, image bytes, ComicInfo URLs).
  - `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt` (`prepareNewChapter`, chapter sync).
  - `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt` (detail merge into DB).
  - `app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt` (`copyFrom`, `toSManga`, `toDomainManga`).
  - `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt` and `SourceFilterMangaDialog.kt` (filters).
  - `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt` (settings screen).
  - `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt` (deep links).
  - `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt` (source instantiation, `SourceFactory`).
  - `app/src/main/java/aniyomi/util/DataSaver.kt` (`HttpSource.getImage(page, dataSaver)` extension).

## 1. Source identity

### 1.1 `id` (OBSERVED: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:57, :87-91)

Algorithm of `generateId(name, lang, versionId)`:
1. `key = "${name.lowercase()}/$lang/$versionId"` (Kotlin `lowercase()` is locale-independent, `Locale.ROOT` semantics; `lang` is NOT lowercased; `versionId` is decimal Int).
2. `bytes = MD5(key.toByteArray())` (UTF-8 bytes, 16 bytes).
3. For `i` in 0..7: `term_i = (bytes[i].toLong() and 0xff) shl (8 * (7 - i))` (big-endian packing of the first 8 bytes).
4. `id = OR(term_0..term_7) and Long.MAX_VALUE` (sign bit cleared, so `id >= 0`).

`id` is lazy: computed on first access, then cached. `name`, `lang`, `versionId` are read at that moment, so a subclass whose `name` depends on state set after first access keeps the first value.

Worked vectors (computed with the same formula; use as unit-test fixtures):

| name | lang | versionId | key | id (decimal) | id (hex) |
|---|---|---|---|---|---|
| `Example` | `en` | 1 | `example/en/1` | 4851548778462321757 | 0x435429E8CEBA1C5D |
| `MangaDex` | `en` | 1 | `mangadex/en/1` | 2499283573021220255 | 0x22AF3D2AFAE61D9F |
| `Ünïcode Src` | `ja` | 2 | `ünïcode src/ja/2` | 8842957433569127029 | 0x7AB87EEF0FC9AA75 |

Edge cases:
- Changing `name`, `lang` or `versionId` changes the id: every library row (`mangas.source`, see `p1-data-and-config.md`) and every source preference file (`source_<id>`) is orphaned. `versionId` is the documented escape hatch (HttpSource.kt:41-44): bump it when URLs become incompatible.
- `generateId` is `protected` so subclasses can compute an old id to keep migrations alive (HttpSource.kt:51-53).
- Source ids are the only join key between a source and its rows; the app never uses the name for identity.

Proposal: `kuro_source::id::generate_id(name: &str, lang: &str, version_id: i32) -> i64` implements exactly the four steps above using `md5` crate (or `md-5`) and `str::to_lowercase()` (Unicode lowercase, same result for the vectors). Tests: the three vectors above must pass; a fourth test asserts `id >= 0` for 1000 random names.

### 1.2 `toString()` display name (OBSERVED: HttpSource.kt:103)

`"$name (${lang.uppercase()})"`, e.g. `Example (EN)`. `uppercase()` is locale-independent. Used by the app for source labels; `MangaSource.getNameForMangaInfo` (app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt:18-31) chooses between `name` and `toString()` based on how many languages are enabled (`enabledLanguages` minus `all`/`other`): if exactly one active language and the source's language is in it, show `name`; otherwise show `toString()`.

Proposal: `impl Display for dyn HttpSource` (or a helper fn) that returns `format!("{} ({})", name, lang.to_uppercase())`.

### 1.3 `lang` (OBSERVED: MangaSource.kt:24-25, CatalogueSource.kt:13)

`MangaSource.lang` defaults to `""`. `CatalogueSource` overrides it as abstract and documents it as "An ISO 639-1 compliant language code (two letters in lower case)" (CatalogueSource.kt:10-12). The compiler does not enforce the format; the app uses it for language filters (`enabledLanguages`), so a source must return lowercase two-letter codes or `"all"`/`"other"` (the app's special values, see MangaSourceExtensions.kt:19-22).

Proposal: `lang()` returns `&str`; validation at registration: lowercase ASCII, length 2 or the reserved values `all` and `other`; otherwise log a warning and keep it.

### 1.4 `versionId` (OBSERVED: HttpSource.kt:45)

`open val versionId = 1`. Type Int.

### 1.5 `headers` and User-Agent (OBSERVED: HttpSource.kt:62, :96-98; core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/UserAgentInterceptor.kt:10-24; core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt:73)

- `headersBuilder()` default: a builder containing exactly one header, `User-Agent` = `network.defaultUserAgentProvider()`, which returns `preferences.defaultUserAgent().get().trim()`.
- `headers` is `by lazy { headersBuilder().build() }`: the value is captured on first access. Changing the UA preference later does not change `headers` for that source instance.
- `UserAgentInterceptor` (network-wide, area:12): if the outgoing request has no non-empty `User-Agent`, it removes any empty `User-Agent` and adds the current default; otherwise it leaves the request alone. Net effect: a source's own `User-Agent` (from `headersBuilder`) is always sent; the preference is used only for requests that have none.
- Subclasses override `headersBuilder()` to add headers (for example `Referer`). `headers` is then the captured value.

Proposal: `fn headers(&self) -> &HeaderMap` computed once with `OnceLock` from `headers_builder()`. The network layer (area:12) inserts the global UA only when the request lacks one.

### 1.6 `client` (OBSERVED: HttpSource.kt:67-68)

`open val client: OkHttpClient get() = network.client`. Overridable per source (for example to add interceptors). Proposal: `fn client(&self) -> &reqwest::Client` returning the shared client by default; a source may return its own client built from the shared builder.

## 2. Request construction

### 2.1 The GET helper (OBSERVED: core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:12-41)

- `DEFAULT_CACHE_CONTROL = CacheControl.Builder().maxAge(10, MINUTES).build()` (`max-age=600`).
- `DEFAULT_HEADERS = Headers.Builder().build()` (empty).
- `GET(url: String, headers, cache)` = `GET(url.toHttpUrl(), headers, cache)`; `GET(url: HttpUrl, ...)` builds `Request.Builder().url(url).headers(headers).cacheControl(cache).build()`.
- An invalid URL string throws (OkHttp `toHttpUrl()` throws `IllegalArgumentException`); the error surfaces as a source failure.

Note: the 10-minute cache-control only matters when the OkHttp cache is configured (area:12). Kurobayomi (reqwest) has no HTTP cache by default; the value is kept as a documented constant `DEFAULT_CACHE_MAX_AGE_SECS = 600` for the cache layer (area:12).

### 2.2 Default request builders (OBSERVED: HttpSource.kt:233-235, :271-273, :316-318, :352-354, :381-383)

| Method | Default | Notes |
|---|---|---|
| `mangaDetailsRequest(manga)` | `GET(baseUrl + manga.url, headers)` | Also used by `getMangaUrl`. |
| `chapterListRequest(manga)` | `GET(baseUrl + manga.url, headers)` | `protected open`; not used by `getMangaUrl`. |
| `pageListRequest(chapter)` | `GET(baseUrl + chapter.url, headers)` | Also used by `getChapterUrl`. |
| `imageUrlRequest(page)` | `GET(page.url, headers)` | `page.url` is used as-is (not joined with `baseUrl`). Sources must store an absolute URL in `Page.url`. |
| `imageRequest(page)` | `GET(page.imageUrl!!, headers)` | Throws `NullPointerException` if `imageUrl` is null. Kurobayomi: return `SourceError::MissingImageUrl`. |
| `popularMangaRequest`, `searchMangaRequest`, `latestUpdatesRequest` | abstract | Must be provided. |

`baseUrl` is documented as "without the trailing slash, like: http://mysite.com" (HttpSource.kt:36-38). The concatenation is a plain string join: `baseUrl + manga.url` produces `http://mysite.com/manga/x` only if `manga.url` starts with `/`. Stored URLs start with `/` because of `setUrlWithoutDomain` (section 6).

## 3. Execution and error model

### 3.1 Which path actually runs (OBSERVED: CatalogueSource.kt:27-52, MangaSource.kt:35-62, HttpSource.kt:112-118, :148-160, :184-190, :214-225, :252-263, :297-308, :335-344)

- Public suspend methods on `CatalogueSource`/`MangaSource` are default implementations that call the deprecated Rx `fetch*` method and `awaitSingle()`.
- `HttpSource` does NOT override `getPopularManga`/`getSearchManga`/`getLatestUpdates`; it overrides the Rx `fetch*` methods. So browsing goes through: `getPopularManga` → `fetchPopularManga` → `client.newCall(popularMangaRequest(page)).asObservableSuccess().map { popularMangaParse(it) }` → `awaitSingle()`.
- `HttpSource` overrides `getMangaDetails`, `getChapterList`, `getPageList`, `getImageUrl` as suspend functions that call their Rx counterpart and `awaitSingle()`. Same request → success check → parse path.
- `getImage` is the only suspend path that uses `awaitSuccess()` directly (section 4.4).
- Consequence for Kurobayomi: one async path per operation: `build request → execute with success check → parse`. Behaviour is identical; the Rx layers add no semantics beyond the success check.

### 3.2 Success check (OBSERVED: core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:63-70, :109-116)

- `asObservableSuccess`: if `!response.isSuccessful` (status not 2xx), close the response and throw `HttpException(response.code)`.
- `awaitSuccess`: same rule, close, throw `HttpException(code)` with the caller's stack trace attached.
- `HttpException(code)` extends `IllegalStateException("HTTP error $code")` (core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt).
- Redirects are followed by OkHttp before this check, so `response.request.url` is the final URL (used as the base URI for HTML parsing, section 4.1).

Proposal: `SourceError::Http { status: u16 }` with display `HTTP error {status}`. Non-2xx responses are never parsed. Redirect policy and cookies are area:12.

### 3.3 Error propagation and caller reaction

- Parsers throw ordinary exceptions (Jsoup `NumberFormatException` from `selectInt`, `NullPointerException` from `!!`, `IllegalArgumentException` from URL building). Nothing in the scope catches them. The caller decides (section 5).
- Reader (`HttpPageLoader.internalLoadPage`): any `Throwable` sets `page.status = ERROR`; `CancellationException` is rethrown without status change (app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt:189-205).
- Downloader (`MangaDownloader`): a failure in the image-URL step (`download.source.getImageUrl(page)`) sets `page.status = ERROR` only; no notification is sent and the image step then returns at once because `page.imageUrl == null` (MangaDownloader.kt:451-458 `getOrDownloadImage`, first check). A failure in the image step (`getOrDownloadImage` → `downloadImage`, which retries; see section 19) sets `page.progress = 0`, `page.status = ERROR`, and calls `notifier.onError(e.message, chapterName, mangaTitle, mangaId)` (MangaDownloader.kt around lines 484-500). `CancellationException` is rethrown in both steps.
- Page list empty in the downloader: `throw Exception(context.stringResource(MR.strings.page_list_empty_error))` (MangaDownloader.kt around line 352).

Proposal: every operation returns `Result<T, SourceError>`; the UI maps `SourceError` to a user-visible message (section 12).

## 4. Parsing contracts

### 4.1 HTML body access (OBSERVED: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsoupExtensions.kt:24-26)

`Response.asJsoup(html: String? = null): Document = Jsoup.parse(html ?: body.string(), request.url.toString())`.
- Reads the whole body into memory (`body.string()`), which also closes it. Use `html` only when the body was already read.
- Base URI = the final request URL (after redirects). Relative `abs:` attribute resolution uses it.

Proposal: `HttpResponse { status, url: Url (final), headers, body: Bytes }`; `html()` decodes as UTF-8 (see section 4.1.1) and parses with base `url`.

#### 4.1.1 Charset
Not specified in Kotlin: `body.string()` uses the charset from the response `Content-Type` header, defaulting to UTF-8 (OkHttp). Proposal: decode with the `charset=` parameter when present, else UTF-8 (`encoding_rs` crate), matching OkHttp's default.

### 4.2 Parsed sources: list pages (OBSERVED: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ParsedHttpSource.kt:23-35, :61-73, :99-111)

For popular, search and latest, the algorithm is identical:
1. `document = response.asJsoup()`.
2. `mangas = document.select(<xSelector>()).map { <xFromElement>(it) }` (document order; every match is mapped; the first mapping exception fails the whole page, there is no per-item skip).
3. `hasNextPage = <xNextPageSelector>()?.let { document.select(it).first() } != null`.
   - If the selector function returns `null`, `hasNextPage` is `false`.
   - If the selector matches nothing, `hasNextPage` is `false`.
   - The value of the matched element is not used, only its presence.
4. Return `MangasPage(mangas, hasNextPage)`.

Note the Kotlin code calls `popularMangaNextPageSelector()` after mapping the elements; the selector functions are pure in every known extension, so the order does not matter for Kurobayomi.

### 4.3 Parsed sources: details, chapters, pages, image URL (OBSERVED: ParsedHttpSource.kt:137-139, :153-156, :175-177, :191-193)

- `mangaDetailsParse(response)` = `mangaDetailsParse(response.asJsoup())`, the subclass returns one `SManga`. The HttpSource layer then sets `initialized = true` on the result (HttpSource.kt:223).
- `chapterListParse(response)` = `document.select(chapterListSelector()).map { chapterFromElement(it) }`. Output order = document order. The source list is NOT reversed or sorted. Sorting into reading order is the app's job (`sourceOrder`, section 5 in `p1-topic-models-and-sync.md`).
- `pageListParse(response)` = `pageListParse(document)`. The returned `Page.index` is ignored by the app (section 5.2).
- `imageUrlParse(response)` = `imageUrlParse(document)`, returns the absolute image URL as a `String`.

### 4.4 HttpSource base parsers
Abstract in `HttpSource`: `popularMangaParse`, `searchMangaParse`, `latestUpdatesParse`, `mangaDetailsParse`, `chapterListParse`, `chapterPageParse`, `pageListParse`, `imageUrlParse`. `chapterPageParse(response): SChapter` is declared abstract but is not called anywhere in the scope or in the app (verified by grep of `chapterPageParse` across `app`, `domain`, `data`, `core`); Kurobayomi drops it (no caller) and records the drop in `p1-tasks.md` T-15a-047.

## 5. Image pipeline (summary; detail in `p1-topic-http-source-pipeline.md`)

### 5.1 `getImageUrl(page)` contract (OBSERVED: HttpSource.kt:334-344, :352-354)
- Doc claims: "If there's any error, it will return null instead of throwing an exception" (HttpSource.kt:328-329). OBSERVED: the return type is `String` (non-null) and the code throws on error. The doc is wrong; the behaviour is "throws".
- Request: `imageUrlRequest(page)` (default `GET(page.url)`), success check, `imageUrlParse`.

### 5.2 Page list reindexing (OBSERVED: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt:81-89; MangaDownloader.kt around lines 362-371)
- Reader: `ReaderPage(index, page.url, page.imageUrl)` with `index` = list position ("Don't trust sources and use our own indexing").
- Downloader: `pages.mapIndexed { index, page -> ... }` assigns list positions; comment "Don't trust index from source".
- Proposal: the source's `Page.index` is ignored; the app assigns `index = position`.

### 5.3 Image bytes (OBSERVED: HttpSource.kt:370-383; core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:119-131)
- `getImage(page)` = `client.newCachelessCallWithProgress(imageRequest(page), page).awaitSuccess()`.
- `newCachelessCallWithProgress` builds a derived client: `cache(null)`, `callTimeout(30, HOURS)`, and a network interceptor that wraps the response body in `ProgressResponseBody(body, listener)`, the listener being the `Page` itself.
- `Page.update(bytesRead, contentLength, done)` sets `progress = 100 * bytesRead / contentLength` (integer division, Kotlin `Long` arithmetic, then `.toInt()`) when `contentLength > 0`, else `-1` (model/Page.kt:43-49).
- Callers replace the page's `imageUrl` temporarily through the data saver (topic file).

## 6. URL normalisation (OBSERVED: HttpSource.kt:391-424)

### 6.1 `setUrlWithoutDomain` (SChapter and SManga extension members)
Assigns `this.url = getUrlWithoutDomain(url)`.

### 6.2 `getUrlWithoutDomain(orig)` algorithm
1. `uri = java.net.URI(orig.replace(" ", "%20"))`. Only ASCII space is replaced.
2. `out = uri.path` (Java `getPath()` returns the DECODED path; `%20` becomes a space, `%2F` becomes `/`).
3. If `uri.query != null` (decoded), `out += "?" + query`.
4. If `uri.fragment != null` (decoded), `out += "#" + fragment`.
5. On `URISyntaxException` return `orig` unchanged.

Observed quirks (kept for compatibility, flagged):
- Decoding: `/a%2Fb` becomes `/a/b`; `%3F` in a path becomes `?`-free text but still changes the path. A page whose path contains encoded reserved characters changes URL on normalisation.
- Opaque URIs (no path, for example `mailto:`) give `uri.path == null`; Kotlin string concatenation then yields the literal text `null` followed by any query or fragment.
- A relative `orig` (no scheme or host) is returned as its path.

Proposal: use the `url` crate: parse `orig` (after replacing spaces with `%20`), take `path()` (percent-ENCODED, which differs from Java's decoded path), append `?query` and `#fragment` as they appear in the encoded form. This deliberately differs from Java for `%2F`-style escapes (keeps the URL working) and is covered by a test. Record in the migration notes that stored URLs produced by Aniyomi may contain decoded characters; `GET` of the stored URL still works for ordinary paths.

### 6.3 Where the stored URL is used
`SManga.url` and `SChapter.url` are stored in the DB (`mangas.url`, `chapters.url`, both `TEXT NOT NULL`, see `p1-data-and-config.md`), and joined with `baseUrl` for requests (section 2.2). Changing the domain of a source therefore only requires changing `baseUrl`.

## 7. Web URLs for a manga or chapter (OBSERVED: HttpSource.kt:433-446; app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt:315-345, :410; app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:630-640; app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt around line 663)

- `getMangaUrl(manga) = mangaDetailsRequest(manga).url.toString()`.
- `getChapterUrl(chapter) = pageListRequest(chapter).url.toString()`. Note: the chapter web URL uses the PAGE-LIST request, not the chapter-list request.
- Both are `open`. Errors are caught by callers: `MangaScreen.getMangaUrl` catches `Exception` and returns `null` (only for `HttpSource`); `ReaderViewModel.getChapterUrl` logs and returns `null`.
- Uses: "Open in WebView" (`WebViewScreen(url, initialTitle, sourceId)`), "Share" (`toShareIntent`, type `text/plain`), `assistUrl` (MangaScreen.kt:109), reader "open chapter in browser" (ReaderActivity.kt:619), and ComicInfo `Web` URLs: `source.getChapterUrl(chapter.toSChapter()).trim()` added to the track URLs, then `.distinct()` (MangaDownloader.kt around lines 657-664).

Proposal: `fn manga_web_url(&self, m: &SManga) -> Result<String, SourceError>` and `fn chapter_web_url(&self, c: &SChapter) -> Result<String, SourceError>`, both defaulting to the request URL. Callers show nothing when `Err` (matches OBSERVED: `null` leads to no action).

## 8. `prepareNewChapter` hook (OBSERVED: HttpSource.kt:448-455; app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt:88-96)

- Documented as "Called before inserting a new chapter into database. Use it if you need to override chapter fields, like the title or the chapter number. Do not change anything to [manga]." (HttpSource.kt:448-454).
- OBSERVED call site: inside `for (sourceChapter in sourceChapters)`, BEFORE the lookup of the existing DB row. It runs for every chapter of every sync (new and existing), and only when `source is HttpSource`.
- Sequence per chapter: `sChapter = chapter.toSChapter()` → `source.prepareNewChapter(sChapter, manga.toSManga())` → `chapter = chapter.copyFromSChapter(sChapter)` → chapter-number recognition (`ChapterRecognition.parseChapterNumber(manga.title, chapter.name, chapter.chapterNumber)`) → DB lookup by `url`.
- Because the hook runs before the DB lookup, a hook that rewrites `url` makes the chapter look new. Documented here as an observed hazard.
- `copyFromSChapter` (app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt:19-27): copies `name`, `url`, `dateUpload = date_upload`, `chapterNumber = chapter_number.toDouble()`, `scanlator = scanlator?.ifBlank { null }?.trim()`.

Proposal: keep the call on every sync (the doc's "new chapter only" behaviour is not what the code does, and existing extensions may rely on running on every sync), but document it as "called for every chapter on every sync". Expose `prepare_new_chapter(&self, chapter: &mut SChapter, manga: &SManga)`, default no-op.

## 9. Chapter and manga mapping details

Section 7 and the topic file hold the full mapping tables. Key rules repeated here because they are edge-case relevant:
- `SChapter.chapter_number` default `-1` (SChapterImpl.kt:13); domain `Chapter.create()` uses `-1.0`.
- `SChapter.date_upload` default `0` (SChapterImpl.kt:11); the sync treats `0` as "unknown" (`p1-topic-models-and-sync.md` section 5.3).
- `SChapter.scanlator` blank becomes `null` and surrounding whitespace is trimmed on the way into the DB (Chapter.kt:24).
- `SManga.genre` is split and normalised by `getGenres()` (section 10).

## 10. Genres (OBSERVED: SManga.kt:29-32; app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:38-80)

`getGenres()`:
1. If `genre` is null or blank (`isNullOrBlank`), return `null`.
2. Else `genre.split(", ")` (the two-character separator comma+space, literal; `"a,b"` stays one item).
3. `map { it.trim() }`, then `filterNot { it.isBlank() }`, then `distinct()` (first occurrence order kept).

Mapping rules:
- DB → SManga: `Manga.toSManga()` sets `genre = genre.orEmpty().joinToString()` (default separator `", "`), so an empty list becomes `""`.
- Source → DB, in `Manga.copyFrom(other: SManga)`: if `other.genre != null` then `genres = other.getGenres()` (possibly `null`, which clears the stored genres), else keep the existing genres. So a source returning `""` clears genres; returning `null` keeps them.
- `UpdateManga` (app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:70): `genre = remoteManga.getGenres()` in the `MangaUpdate`. In the SQL update, `genre = coalesce(:genre, genre)` (data/src/main/sqldelight/data/mangas.sq, update block), so `null` means "keep the stored value". Therefore on the library refresh path a source genre of `""` (which `getGenres()` turns into `null`) does NOT clear the stored genres; only the `Manga.copyFrom(other: SManga)` overload clears them, and that overload is used by the metadata job (app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt:125, `.copyFrom(networkManga)`).
- `StringListColumnAdapter` stores the list joined with `", "` (data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:16, :19-27), so a genre containing `", "` round-trips as two genres.

Proposal: `SManga::genres(&self) -> Option<Vec<String>>` with identical logic; `copy_from` applies the same `Option` rules. Tests cover `""`, `null`, `"a, a, b"` → `["a","b"]`, `"a,b"` → `["a,b"]`.

## 11. Manga detail merge rules (OBSERVED: app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:50-69; UpdateManga.kt:40-79)

`Manga.copyFrom(other: SManga)`:
- `author = other.author ?: author`; `artist = other.artist ?: artist`; `description = other.description ?: description`. A source `null` keeps the stored value; an empty string overwrites it.
- `thumbnailUrl = other.thumbnail_url ?: thumbnailUrl`.
- `status = other.status.toLong()` (always overwritten, including `0`).
- `updateStrategy = other.update_strategy` (always overwritten).
- `initialized = other.initialized && initialized` (true only if both are true).

`UpdateManga` (library refresh of a favourite or of a non-favourite):
- `title`: null (not updated) when the remote title is empty or the local manga is a favourite; otherwise the remote title. (UpdateManga.kt:42-43.)
- `coverLastModified`:
  - `remote thumbnail_url` null or empty → `null` (never clear a cover).
  - not manual fetch AND local `thumbnailUrl == remote thumbnail_url` → `null`.
  - local manga is local (`isLocal()`) → `now` (millis).
  - local has a custom cover → delete cached cover (not the custom one), value `null`.
  - else → delete cached cover, value `now` (millis).
- `thumbnailUrl = remote.thumbnail_url?.takeIf { it.isNotEmpty() }`.
- `status = remote.status`, `updateStrategy = remote.update_strategy`, `initialized = true` are always written.
- `author`, `artist`, `description`, `genre` are passed as `MangaUpdate` fields; `null` there means "keep stored value" (SQL `coalesce(:x, x)`, mangas.sq update block). So a source `null` never clears these on the refresh path; an empty string overwrites `author`/`artist`/`description` and is turned into `null` (keep) for `genre`.
- Full write set and the SQL are in `p1-topic-models-and-sync.md`.

Proposal: same rules; the cover cache work is area:34.

## 12. Status values and their user-visible text

Integer constants (OBSERVED: SManga.kt:48-54):

| Constant | Value | Icon (MangaInfoHeader.kt:509-514) | Text key (MR.strings.*) | English text (i18n/src/commonMain/moko-resources/base/strings.xml) |
|---|---|---|---|---|
| `UNKNOWN` | 0 | `Icons.Outlined.Block` (else branch) | `unknown` | Unknown |
| `ONGOING` | 1 | `Icons.Outlined.Schedule` | `ongoing` | Ongoing |
| `COMPLETED` | 2 | `Icons.Outlined.DoneAll` | `completed` | Completed |
| `LICENSED` | 3 | `Icons.Outlined.AttachMoney` | `licensed` | Licensed |
| `PUBLISHING_FINISHED` | 4 | `Icons.Outlined.Done` | `publishing_finished` | Publishing finished |
| `CANCELLED` | 5 | `Icons.Outlined.Close` | `cancelled` | Cancelled |
| `ON_HIATUS` | 6 | `Icons.Outlined.Pause` | `on_hiatus` | On hiatus |

Any other integer shows `unknown` and the `Block` icon (MangaInfoHeader.kt:515-531). The status is stored as an INTEGER (`mangas.status INTEGER NOT NULL`), so unknown source values are kept.

Other status uses: `COMPLETED` drives library filter "Completed" (MangaLibraryScreenModel.kt:216), library update restriction `ENTRY_NON_COMPLETED` (MangaLibraryUpdateJob.kt:196), stats (MangaStatsScreenModel.kt:56, :112), and upcoming (`GetUpcomingManga`, domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt:12-15: `ONGOING` and `PUBLISHING_FINISHED`).

Proposal: `enum MangaStatus { Unknown=0, Ongoing=1, Completed=2, Licensed=3, PublishingFinished=4, Cancelled=5, OnHiatus=6, Other(i32) }` with `from_i32`/`to_i32`; `Other` keeps the raw value for round-trip. UI shows the `Unknown` text for `Other`.

## 13. UpdateStrategy and library updates (OBSERVED: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt:9-23; app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:186-200; DatabaseAdapter.kt:29-34)

- `ALWAYS_UPDATE` (ordinal 0): included in library updates (subject to other restrictions).
- `ONLY_FETCH_ONCE` (ordinal 1): skipped during library updates; the skipped list entry text is `skipped_reason_not_always_update` ("Skipped because series does not require updates").
- DB: `mangas.update_strategy INTEGER AS UpdateStrategy NOT NULL DEFAULT 0`; `MangaUpdateStrategyColumnAdapter.encode = ordinal`, `decode = entries.getOrElse(v) { ALWAYS_UPDATE }` (so unknown stored values become `ALWAYS_UPDATE`).
- The decision is made in the library update job, not in the source. A source sets `update_strategy` in `getMangaDetails`; the value is copied into the DB by the detail merge (section 11). `Manga.toSManga()` does NOT copy `updateStrategy` (Manga.kt:38-49), so a round-trip through `SManga` resets the value to `ALWAYS_UPDATE`. Observed hazard; the app only calls `toSManga` for the source call, and then `copyFrom` copies the source's value back.

Proposal: `UpdateStrategy` is copied both ways in `to_s_manga` (fix of the observed hazard), with a test that a round trip preserves it.

## 14. Browse, search and filters (summary; detail in `p1-topic-filters.md`)

- `getFilterList()` is called when the browse screen opens and whenever a fresh filter sheet is needed (`BrowseMangaSourceScreenModel.kt:150`, `:171`, `:187`). Each call returns new filter objects; the app keeps its own mutable copy.
- `search(query, filters)` stores `Listing.Search(query, filters)`; the source receives the same `FilterList`.
- `searchGenre(genreName)` (BrowseMangaSourceScreenModel.kt:184-227) sets a matching filter and runs the search; if no filter matches, it searches for the text instead.
- Filter dialog (`SourceFilterMangaDialog.kt`): reset (`action_reset`) and filter (`action_filter`) buttons in a sticky header.

## 15. Preferences of a source (OBSERVED: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt:9-29; app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt:131-160; app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt:37-56)

- Key: `preferenceKey() = "source_$id"` (decimal id). Storage: an Android `SharedPreferences` file named exactly that key, `MODE_PRIVATE`. `getSourcePreferences()` and `sourcePreferences()` return that file.
- `sourcePreferences(key: String)` opens any named file (used by backup restore).
- UI: `MangaSourcePreferencesScreen.populateScreen()` gets the source with `getOrStub(sourceId)`; if `ConfigurableSource`, sets `preferenceDataStore = SharedPreferencesDataStore(source.sourcePreferences())`, calls `setupPreferenceScreen(screen)`, then for each child preference: `isIconSpaceReserved = false`, `isSingleLineTitle = false`, dialog title defaults to the preference title when empty, and EditText preferences get incognito IME.
- Entry points: source options and settings in the browse toolbar when `source is ConfigurableSource` (BrowseMangaSourceToolbar.kt:44, :85) and the manga screen (MangaScreen.kt:146).
- Backup: `PreferenceBackupCreator.createSource` includes, for every `ConfigurableSource` among `getCatalogueSources()`, one entry `BackupSourcePreferences(preferenceKey(), all.toBackupPreferences())`, only when the map is not empty (sensitive keys are filtered by `withPrivatePreferences(includePrivatePreferences)`).
- Restore: `PreferenceRestorer` opens `sourcePreferences(it.sourceKey)` (app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt:52).
- Used by Kavita tracker: `(sourceManager.get(sourceId) as ConfigurableSource).sourcePreferences()` (app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt:128).

Proposal: see `p1-data-and-config.md` section 4 and the design section of `p1-rust-design.md`. A source declares settings with a typed schema (`SourceSetting::{Text, Switch, List, MultiSelect, Info}`) instead of a live preference screen.

## 16. Deep links (OBSERVED: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt:25-60)

1. `sourceManager.getCatalogueSources().filterIsInstance<ResolvableSource>().firstOrNull { it.getUriType(query) != UriType.Unknown }`. First match wins, in source list order.
2. `manga = source.getManga(query)?.let { getMangaFromSManga(it, source.id) }`.
3. If `source.getUriType(query) == UriType.Chapter && manga != null`: `chapter = source.getChapter(query)?.let { getChapterFromSChapter(it, manga, source) }`, else `chapter = null`.
4. State: `manga == null` → `NoResults`; `chapter == null` → `Result(manga)`; else `Result(manga, chapter.id)`.
- No try/catch in the model: exceptions from `getManga`/`getChapter` propagate out of the `launchIO` block (observed).
- `getUriType` is called twice for chapter links (once for selection, once for branch).

Proposal: `UriType` as enum; resolution order as above; errors mapped to `NoResults` with a logged error (a deliberate change; recorded in the tasks file).

## 17. Queue warnings and unmetered sources (OBSERVED)

- `UnmeteredSource` marker: sources that implement it are excluded from:
  - the download-queue warning counts (`MangaDownloader.kt:308-311`: `queuedDownloads = queueState.count { it.source !is UnmeteredSource }`; `maxDownloadsFromSource = groupBy(source).filterKeys { it !is UnmeteredSource }.max size`);
  - the library-update per-source warning (`MangaLibraryUpdateNotifier.kt:120-126`: `filterKeys { sourceManager.get(it) !is UnmeteredSource }`).
- Constants (exact):
  - `MangaDownloader.WARNING_NOTIF_TIMEOUT_MS = 30_000L` (MangaDownloader.kt:767).
  - `MangaDownloader.CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 15` (MangaDownloader.kt:768).
  - `DOWNLOADS_QUEUED_WARNING_THRESHOLD = 30` (MangaDownloader.kt:769, private).
  - `MANGA_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 60` (MangaDownloader.kt does not declare it; the library notifier declares it at MangaLibraryUpdateNotifier.kt:400, also in MangaLibraryUpdateJob.kt:428 and MangaMetadataUpdateJob.kt:178).
- Warning fires when `queuedDownloads > 30` OR `maxDownloadsFromSource > 15`, and only when the queue was empty before the add and `autoStart` is set (MangaDownloader.kt:305-318).
- Library warning fires when `maxUpdatesFromSource > 60` (MangaLibraryUpdateNotifier.kt:120-135).
- Messages: download warning uses `AYMR.strings.download_queue_size_warning` (base text "Warning: large bulk downloads may lead to sources becoming slower and/or blocking Aniyomi. Tap to learn more."); library warning uses `MR.strings.label_warning` ("Warning") and `MR.strings.notification_size_warning` ("Large updates harm sources and may lead to slower updates and also increased battery usage. Tap to learn more.").

Proposal: `pub trait UnmeteredSource { }` marker; `fn is_unmetered(src: &dyn MangaSource) -> bool` via a `fn unmetered(&self) -> bool { false }` method on `MangaSource` set by implementors. The thresholds are constants in `kuro-app::downloads`/`kuro-app::library` (area:23, area:26), not in this crate.

## 18. Concurrency and dispatchers (OBSERVED)

- Reader page loader: `PriorityBlockingQueue<PriorityPage>` (HttpPageLoader.kt:46); one background coroutine loops `queue.take()` and processes items one at a time through `collect(::internalLoadPage)` (HttpPageLoader.kt:52-62). Effective concurrency: 1 image at a time per reader.
- Priorities: current page = 1 (on `loadPage` when QUEUE), preload = 0 (`preloadSize = 4` next pages, HttpPageLoader.kt:48, :112, :253-263), user retry = 2 (`retryPage`). Ordering: higher priority first, ties FIFO by a global counter (HttpPageLoader.kt:215-227).
- Downloader: `pageList.asFlow().flatMapMerge(concurrency = 2)` (MangaDownloader.kt:380-393): two pages in flight; each page's work runs on `Dispatchers.IO`.
- Source calls run inside `withIOContext` in the callers.
- Cancellation: `CancellationException` is always rethrown and never sets `ERROR` (HttpPageLoader.kt:198-204; MangaDownloader.kt around line 485).

Proposal: tokio multi-thread runtime; reader loader = one tokio task with `tokio::sync::mpsc` + a `BinaryHeap` (priority, sequence); downloader = `futures::stream::buffer_unordered(2)` per download; `reqwest` requests are non-blocking so no `spawn_blocking` is needed for network; HTML parsing (`scraper`) is CPU work and runs in `tokio::task::spawn_blocking` when the document is larger than 256 KiB (proposal constant `HTML_BLOCKING_THRESHOLD_BYTES = 262_144`).

## 19. Timings, limits and constants (complete list for this scope)

| Name | Value | Where |
|---|---|---|
| `DEFAULT_CACHE_CONTROL` max-age | 600 s (10 min) | Requests.kt:12 |
| image call timeout (`callTimeout`) | 30 hours | OkHttpExtensions.kt:121-123 |
| `HttpSource.versionId` default | 1 | HttpSource.kt:45 |
| `Page.State` count | 5 values | Page.kt:51-57 |
| `SManga` status codes | 0..6 | SManga.kt:48-54 |
| `Filter.TriState` values | 0, 1, 2 | Filter.kt:18-20 |
| `Sort.Selection` | index >= 0, ascending Boolean | Filter.kt:28 |
| `chapter_number` default | -1 | SChapterImpl.kt:13 |
| `date_upload` default | 0 | SChapterImpl.kt:11 |
| download concurrency | 2 | MangaDownloader.kt:380 |
| image download retry | 3 retries after the first attempt; waits `(2 shl attempt) * 1000` ms = 2 s, 4 s, 8 s (`retryWhen { attempt < 3 }`) | MangaDownloader.kt `downloadImage` (around lines 540-553) |
| downloaded page file name | `"%0${digits}d".format(page.number)` with `digits = max(3, length of total page count)`; extension from the response | MangaDownloader.kt:459-462 (`getOrDownloadImage`) |
| temporary download files | `"$filename.tmp"`, deleted before a page is retried and at download start (`*.tmp` in the chapter temp dir) | MangaDownloader.kt:462-466 and 386-389 |
| reader preload size | 4 | HttpPageLoader.kt:48 |
| reader priorities | preload 0, current 1, retry 2 | HttpPageLoader.kt:112, :109-111, :130 |
| download queue warning | 30 downloads or 15 per source | MangaDownloader.kt:768-769 |
| library per-source warning | 60 | MangaLibraryUpdateNotifier.kt:400 |
| warning notification timeout | 30 000 ms | MangaDownloader.kt:767 |
| data saver defaults | see topic file | app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:97-116 |
| download speed limit | `downloadSpeedLimit * 1024` bytes/s to throttler | MangaDownloader.kt around line 400+ |

## 20. Interactions with other areas

- area:12-network-stack: HTTP client, cookies, Cloudflare WebView retry, DoH, user-agent interceptor, `GET`/`POST` helpers, `ProgressResponseBody`, `HttpException`.
- area:02-preferences-system: `SourcePreferences` (data saver, default UA is area:12), `preferenceStore.getEnum`.
- area:17-source-app-layer: `MangaSourceManager.getCatalogueSources`, `getOrStub`, `StubMangaSource`, language filtering.
- area:18-extensions-manga: APK/class loading, `tachiyomi.extension.class` metadata, NSFW gate, `SourceFactory`, trust.
- area:20-extension-strategy-redesign: how Kurobayomi loads sources (in-process trait vs WASM/Lua/JS). This area defines the trait; area:20 chooses the loader.
- area:21/22-backup: `BackupSourcePreferences`, `PreferenceBackupCreator`, `PreferenceRestorer`, manga backup stores `updateStrategy` (BackupManga.kt:38).
- area:23-downloads-manga and area:26-library-update-jobs: consume thresholds and page states.
- area:34-images-covers-cache: cover cache deletion in UpdateManga.
- area:43-reader-core: page loader.
- area:40-ui-browse-manga: browse screen and filter sheet.
- area:38-ui-entry-details-manga: status badge and web URL actions.
- area:07-domain-entries-items, area:13-core-util-storage-archive-torrent: `ChapterRecognition`, `ChapterSanitizer` (chapter name cleanup), file storage.
- area:51-i18n: string keys listed in section 12 and 17.
- area:36-ui-shell-navigation-security-webview: WebView screen used by "Open in WebView".
