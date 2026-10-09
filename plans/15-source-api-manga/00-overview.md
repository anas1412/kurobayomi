# Area 15 — Manga source API: overview

Area id: `15-source-api-manga`. Scope: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/**` and `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/**` in `/home/blackbox/aniyomi-src` (27 files, 1,314 LOC). Every path below is repo-relative to `/home/blackbox/aniyomi-src` unless it starts with `kurobayomi/`.

Observed (Kotlin behaviour) is cited as `path:line`. Proposals are labelled `Proposal:` in the detail docs.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

The manga source API is the contract between Aniyomi and every manga source. A source is one object that answers five questions for the app:

1. Who am I: `id` (a stable `Long`), `name`, `lang`.
2. Browse a catalogue: popular, search (with typed filters), latest updates. Each returns a page of `SManga` plus `hasNextPage`.
3. Describe a manga: details (`getMangaDetails`), chapter list (`getChapterList`).
4. Produce reader pages: a page list (`getPageList`), then an image URL per page (`getImageUrl`) and image bytes (`getImage`).
5. Optionally expose a settings screen (`ConfigurableSource`), deep-link resolution (`ResolvableSource`), and a flag saying the source is not rate-limited (`UnmeteredSource`).

Almost all of the 27 files are the HTTP-website layer: `HttpSource` (461 LOC) and `ParsedHttpSource` (201 LOC) implement request building, status checks, Jsoup parsing, the image pipeline and the id algorithm. The rest are small interfaces and models (`SManga`, `SChapter`, `Page`, `Filter`, `FilterList`, `MangasPage`).

Full file table with port decisions: [p1-inventory.md](p1-inventory.md).

### 1.2 Why it matters for a usable Kurobayomi

- Without a working source there is nothing to read. The P0 slice of this area (34 of 51 tasks) is the minimum for reading manga: models, id, HTTP pipeline, HTML parsing, catalogue, filter sheet, details merge, chapter sync rules, page list, image pipeline with retry.
- Source ids are the join key for every library row (`mangas.source`) and for every source preference file (`source_<id>`). A wrong id algorithm silently orphans the library. The algorithm is therefore ported bit-exact (T-15a-012, three vectors verified in this review: `Example/en/1` → `4851548778462321757`, `MangaDex/en/1` → `2499283573021220255`, `Ünïcode Src/ja/2` → `8842957433569127029`).
- The model rules in this area decide what the app writes to the database: detail merge (`Manga.copyFrom` vs `UpdateManga`), chapter sync (`distinctBy url`, date rules, chapter number), and the reader/downloader page state machines.

### 1.3 Explicit non-goals of this area

- It does not decide how third-party source code is loaded. Aniyomi extensions are signed Android APKs; running them on Linux needs an Android runtime or a shim. That decision belongs to area:20 (see section 4, decision D1).
- It does not own the HTTP client (cookies, Cloudflare, DNS, user-agent interceptor): area:12.
- It does not own the anime source API: area:16.
- It does not own the browse, reader or settings screens; it defines the types they render and the contracts they call (area:40, area:43, area:47).

---

## 2. Module map

### 2.1 Kotlin packages to responsibilities

| Kotlin package / file group (under `source-api/src/commonMain/kotlin/`) | Files | Responsibility |
|---|---|---|
| `eu/kanade/tachiyomi/source/` | `MangaSource.kt`, `CatalogueSource.kt`, `ConfigurableSource.kt`, `SourceFactory.kt`, `UnmeteredSource.kt`, `PreferenceScreen.kt` (expect) | Root contract (`MangaSource`), browse contract (`CatalogueSource`), settings contract, factory for several sources in one extension, marker for self-hosted sources, Android settings-type placeholder |
| `eu/kanade/tachiyomi/source/online/` | `HttpSource.kt`, `ParsedHttpSource.kt`, `ResolvableSource.kt` | HTTP website base class (request builders, status check, parsing hooks, image pipeline, id, headers), Jsoup convenience base, deep-link contract with `UriType` |
| `eu/kanade/tachiyomi/source/model/` | `SManga.kt`, `SMangaImpl.kt`, `SChapter.kt`, `SChapterImpl.kt`, `UpdateStrategy.kt`, `Page.kt`, `MangasPage.kt`, `Filter.kt`, `FilterList.kt` | Data types returned by sources and passed back (manga metadata, chapter, page, page of results, filters) |
| `eu/kanade/tachiyomi/util/` | `JsoupExtensions.kt`, `JsonExtensions.kt`, `RxExtension.kt`, `VideoInfo.kt` | Element helpers (`selectText`, `selectInt`, `attrOrText`, `asJsoup`), app JSON instance for extensions, Rx bridge (dropped), dead `Video` placeholder (dropped) |
| `eu/kanade/tachiyomi/animesource/` (androidMain only) | `PreferenceScreen.kt` | Android typealias for the anime settings type; lives in the manga source set, not used by the manga API (area:16) |
| `androidMain/` | `AndroidManifest.xml`, `source/PreferenceScreen.kt`, `util/RxExtension.kt` | Android actuals and an empty manifest (dropped) |
| `build.gradle.kts` | Gradle module | Dependency wiring (dropped; replaced by Cargo manifests) |

### 2.2 Callers outside the scope (verified by grep in [p1-behavior.md](p1-behavior.md) section 0)

| Caller | What it uses |
|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt` | page list, image URL, image bytes (reader) |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt` | page list, image URL, image bytes, ComicInfo URLs, queue thresholds |
| `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt` | `prepareNewChapter`, chapter sync |
| `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt` and `.../model/Manga.kt` | detail merge, `copyFrom`, `toSManga`, `toDomainManga` |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/*.kt` | filters, search, Reset and Filter sheet |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt` | source settings screen |
| `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt` | deep links |
| `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt` | source instantiation, `SourceFactory` |
| `app/src/main/java/aniyomi/util/DataSaver.kt` | image URL rewriting (data saver) |
| `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt`, `OkHttpExtensions.kt` | `GET` helper, success check, progress body |

### 2.3 Proposed Rust crate and module map

Workspace (owned by area:01 for the root; this area defines the two library crates):

| Crate | Module | Contents | Replaces |
|---|---|---|---|
| `kuro-source` (no GTK, no HTTP) | `src/error.rs` | `SourceError` (Http, Transport, Parse, Url, MissingImageUrl, EmptyPageList, Unsupported, Cancelled) | exception classes |
| | `src/id.rs` | `generate_id(name, lang, version_id) -> i64` | `HttpSource.generateId` |
| | `src/json.rs` | `default_json_options()` | `JsonExtensions.kt` |
| | `src/model/manga.rs` | `SManga`, `MangaStatus`, `genres()` | `SManga.kt`, `SMangaImpl.kt` |
| | `src/model/chapter.rs` | `SChapter`, `copy_from` | `SChapter.kt`, `SChapterImpl.kt` |
| | `src/model/page.rs` | `Page`, `PageState`, `PageStatus` (watch channels), `PageCacheEntry` | `Page.kt` |
| | `src/model/mangas_page.rs` | `MangasPage` | `MangasPage.kt` |
| | `src/model/update_strategy.rs` | `UpdateStrategy` | `UpdateStrategy.kt` |
| | `src/filter.rs`, `src/filter_list.rs` | `Filter` enum (8 variants), `TriStateState`, `SortSelection`, `FilterList` | `Filter.kt`, `FilterList.kt` |
| | `src/source.rs` | `MangaSource`, `CatalogueSource`, `ConfigurableSource`, `ResolvableSource`, `SourceFactory` traits | `MangaSource.kt`, `CatalogueSource.kt`, `ConfigurableSource.kt`, `SourceFactory.kt` |
| | `src/resolve.rs` | `UriType` | `ResolvableSource.kt` |
| | `src/prefs.rs` | `SourceSetting` schema, `SourcePrefs` trait, `PrefValue`, `preference_key` | `ConfigurableSource.kt` helpers, `SharedPreferences` |
| `kuro-source-http` (reqwest, scraper, url) | `src/client.rs` | `HttpClient::execute`, `execute_streaming` | OkHttp `OkHttpClient` |
| | `src/request.rs` | `SourceRequest`, `get()`, `DEFAULT_CACHE_MAX_AGE` | `Requests.kt` |
| | `src/response.rs` | `HttpResponse`, `ensure_success`, `html()`, `text()`, `json()` | `OkHttpExtensions.kt`, `Response.asJsoup` |
| | `src/http_source.rs` | `HttpCore`, trait `HttpSource`, adapter `Http<S>` implementing the kuro-source traits | `HttpSource.kt` |
| | `src/parsed.rs` | trait `ParsedHttpSource`, `parse_list` | `ParsedHttpSource.kt` |
| | `src/html.rs` | `text`, `select_text`, `select_int`, `attr_or_text` | `JsoupExtensions.kt` |
| | `src/url.rs` | `get_url_without_domain`, `set_url_without_domain` | `HttpSource.kt:391-424` |
| | `src/image.rs` | image fetch with progress | `HttpSource.getImage`, `newCachelessCallWithProgress` |
| | `src/data_saver.rs` | trait `DataSaver`, `NoOp`, `BandwidthHero`, `WsrvNl`, `ReSmushIt` | `aniyomi/util/DataSaver.kt` |
| `kurobayomi` (GTK app, existing package) | `src/` | filter sheet renderer, source settings renderer, browse wiring, reader loader task, download pages, chapter sync rules | the Kotlin callers in section 2.2 (owned by areas 40, 43, 23, 07/10) |

Dependency rule: `kuro-source` must compile without GTK, tokio runtime features or reqwest (T-15a-001 acceptance). Reason: area:20 may later compile a source to WASM against this crate.

Full dependency table with version policy and what each crate replaces: [p1-rust-design.md](p1-rust-design.md) sections 1-2 and 12.

---

## 3. Reading guide

Read in this order: this overview, then the rules (behaviour), then the design, then the tasks.

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Every one of the 27 scope files with declared names, LOC, port decision (Port, Rewrite, Replace-with-crate, Drop) and task ids. |
| [p1-behavior.md](p1-behavior.md) | Observed behaviour of the API: source id algorithm, request builders, error model, parsing contracts, image pipeline summary, URL normalisation, web URLs, `prepareNewChapter`, genres, detail merge, status codes, update strategy, filter summary, preferences, deep links, queue warnings, concurrency, constants table. |
| [p1-topic-http-source-pipeline.md](p1-topic-http-source-pipeline.md) | Step-by-step flow of every HTTP operation, the reader page state machine, downloader per-page steps with retry schedule (2 s, 4 s, 8 s), data saver algorithm, cancellation summary, pipeline test vectors. |
| [p1-topic-filters.md](p1-topic-filters.md) | Filter model (eight kinds, TriState cycle, Sort direction rules), listing states, `search` and `searchGenre` algorithms, filter sheet layout, source contract for filter values. |
| [p1-topic-models-and-sync.md](p1-topic-models-and-sync.md) | How the app consumes `SManga`, `SChapter`, `Page`, `MangasPage`: identity, field mapping, detail merge, chapter sync steps, paging, library eligibility, status filters, test vectors. |
| [p1-rust-design.md](p1-rust-design.md) | Crate layout, dependencies with replacements, error model, Rust type definitions for every model, filters, source traits, settings schema, source preference storage, HTTP layer, `HttpSource` trait and generic `Http<S>` adapter, ParsedHttpSource, integration with the mockup, GTK/tokio threading, concurrency table, Android-to-Linux mapping, pitfalls. |
| [p1-data-and-config.md](p1-data-and-config.md) | Every field with Kotlin type, nullability, default and Rust type; status and filter enum tables; database column mapping for `mangas`, `chapters`, `sources`; app preference keys and defaults; on-disk cache formats; data saver endpoints; constants. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge-case catalogue (EC-*), race conditions (RC-*), failure-mode table, fixtures list, the test list (TST-01..37) mapped to crates and tasks, performance expectations, manual test script. |
| [p1-ui-parity.md](p1-ui-parity.md) | UI elements these types render (browse toolbar, listing states, filter sheet, source settings, status badge, genre chips, deep-link states, warnings) and what the mockup lacks, with desktop equivalents. |
| [p1-tasks.md](p1-tasks.md) | 51 implementation tasks `T-15a-001`..`T-15a-051` with priority, effort, dependencies, Aniyomi sources and acceptance criteria. |

---

## 4. Key design decisions and open questions

Each item needs the user's call unless marked "settled". Recommendation is the position the plan currently takes.

### D1. How do sources run at all? (needs user decision, the most important one)

- Fact: Aniyomi manga sources are Android APK extensions. They call Android framework classes and cannot run on Linux as shipped. Area:20 states this and recommends a phased path (`plans/20-extension-strategy-redesign/p1-topic-option-matrix.md`, section "Recommended path with phases").
- Question: are you willing to port each site you read from to Rust (a built-in source), or do you need to load existing Aniyomi extensions?
- Recommendation: port the sites you use to Rust, compiled in, against the trait contract in [p1-rust-design.md](p1-rust-design.md) section 6. Loader mechanisms (TOML scrapers, WASM) follow later in area:20. Without an answer, T-15a-015 (SourceFactory and loader contract) cannot be closed.

### D2. Which sites are the first working sources? (needs user input)

- The P0 acceptance tests need real saved pages (fixtures, T-15a-048). Each ported source needs fixture tests (rust-design section 13, item 3).
- Recommendation: name one manga site you read daily, and the single HTML-based source family it belongs to. Everything in this area is built and verified against that one source first.

### D3. Crate prefix: `kuro-*` or `kb-*`? (needs decision, cross-area conflict)

- Area:15 (this area), area:01, area:14, area:17 and area:18 name crates `kuro-*` (for example `kuro-source`, `kuro-source-http`, `kuro-core`, `kuro-domain`). Area:17 writes `kuro-sources` (plural).
- Area:20 and area:54 name crates `kb-*` (`kb-source`, `kb-ext`, `kb-sources-builtin`, `kb-source-scrape`, `kb-source-api`).
- Recommendation: `kuro-*` everywhere, because five areas already use it. Area:20 and area:54 rename their crates. This plan does not rename them itself. Area:17's `kuro-sources` should become `kuro-source` or a named sub-module, to be decided with area:17.

### D4. Where do source preferences live? (recommendation settled in the plan, user may object)

- Option A: SQLite table `source_prefs(source_id, key, type, value)` in the app database (chosen in [p1-rust-design.md](p1-rust-design.md) section 7).
- Option B: one JSON file `source_<id>.json` per source.
- Recommendation: option A. Backup and restore then run in one transaction (area:22). The key namespace `source_<id>` is kept so backups keep the same `sourceKey` string.

### D5. Chapter numbers: keep the float rounding or correct it? (recommendation: correct it)

- Observed: `SChapter.chapter_number` is `Float`; `copyFromSChapter` widens it with `.toDouble()`. A source value `1.1f` becomes `1.100000023841858` in the database, and exact-double comparisons with recognised numbers then fail (see [p1-topic-models-and-sync.md](p1-topic-models-and-sync.md) section 5.1).
- Proposal (T-15a-004): keep `f32` in the model, convert with `chapter_number.to_string().parse::<f64>()`, which yields `1.1`. This is a documented deliberate difference.

### D6. Data saver: fix the observed bugs? (recommendation: yes)

- Observed: `RESMUSH_IT` puts the image URL unencoded into the query string and uses plain `http`; `WSRV_NL` does not encode the URL; a `RESMUSH_IT` reply without `"dest":"` returns the whole text as the URL (see [p1-topic-http-source-pipeline.md](p1-topic-http-source-pipeline.md) section 10).
- Proposal: percent-encode the image URL, use `https`, return `SourceError::Parse` when the delimiter is missing. The user loses nothing that works today: a URL with `&` already breaks in Kotlin.

### D7. Sort direction when nothing is selected (recommendation: defined behaviour instead of a crash)

- Observed: a `Sort` with `state == null` throws `NullPointerException` when tapped (`SettingsItems.kt`, see [p1-topic-filters.md](p1-topic-filters.md) section 5.7).
- Proposal: a tap on an unselected `Sort` selects that row with `ascending = true` (T-15a-010, EC-FI-03).

### D8. Stored URLs: encoded or decoded path? (needs user's OK for a migration note)

- Observed: `setUrlWithoutDomain` uses `java.net.URI.path`, which is decoded (`/a%2Fb` becomes `/a/b`). See [p1-behavior.md](p1-behavior.md) section 6.2.
- Proposal: use the `url` crate and keep the encoded form. Stored Aniyomi URLs that contain decoded characters still work in ordinary cases. Impact: a library imported from an Aniyomi backup may have a few paths with encoded reserved characters that now resolve differently. Recommendation: keep encoded, and log a warning on the first request that fails with 404 after a restore.

### D9. `prepareNewChapter` runs on every sync, not only for new chapters (recommendation: keep the observed behaviour)

- Observed: the Kotlin doc says "Called before inserting a new chapter" but the code runs it for every chapter on every sync (`SyncChaptersWithSource.kt:88-96`).
- Proposal: run it on every sync, document it that way (T-15a-026, EC-CH-05). Existing source logic may rely on it.

### D10. Drop the Rx layer, `chapterPageParse`, `Video`, and Android settings types (recommendation: drop, settled)

- Rx `fetch*` methods are deprecated and only wrap the suspend API. `chapterPageParse` has no caller in the app or the scope (verified by grep). `Video`/`VideoUrl` is unused in the manga scope. Users lose nothing.

### D11. Structural equality for `FilterList` (recommendation: yes, settled)

- Observed: `FilterList.equals` always returns `false`, an artefact of Compose recomposition. Rust uses structural equality (T-15a-011).

### D12. Source panic isolation and cool-down (user may tune the value)

- Proposal: each source call runs in a tokio task; a panic becomes `SourceError::Parse("source panicked")`; the source is marked failed for 60 seconds. The 60-second value is a proposal, not an Aniyomi constant (T-15a-013).

### D13. Keep the observed concurrency and retry constants (recommendation: keep)

- Reader preload size 4, reader priorities (preload 0, current 1, retry 2), downloader concurrency 2, image retries 3 with waits 2 s, 4 s, 8 s (`MangaDownloader.kt`). Kept unchanged from Aniyomi.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What this area needs | Tasks that need it |
|---|---|---|
| area:01-foundation-app-bootstrap | Cargo workspace root, tokio runtime handle and GTK-to-tokio bridge pattern; construction order | T-15a-001, T-15a-050 |
| area:02-preferences-system | Preference store for source prefs and the data saver settings (`data_saver`, `ignore_jpeg`, `ignore_gif`, `data_saver_image_quality`, `data_saver_image_format_jpeg`, `data_saver_server`, `data_saver_color_bw`, `data_saver_excluded`, `data_saver_downloader`, `default_user_agent`) | T-15a-018, T-15a-021, T-15a-029 |
| area:03-db-manga-schema-migrations | `mangas`, `chapters`, `sources` tables and migrations; `source_prefs` table if D4 is accepted | T-15a-036 |
| area:05-data-repos-entries-items | Repositories for manga and chapter rows used by the sync | T-15a-035 |
| area:07-domain-entries-items | `ChapterRecognition.parseChapterNumber`, `ChapterSanitizer`, `shouldUpdateDbChapter`, the chapter directory rename rule | T-15a-026, T-15a-035 |
| area:10-app-domain-layer | `UpdateManga` and `Manga.copyFrom` implementation (detail merge) | T-15a-036 |
| area:12-network-stack | `reqwest` client, cookies, Cloudflare handling, user-agent interceptor, redirect policy, OkHttp cache equivalent (`max-age=600`) | T-15a-020, T-15a-021, T-15a-022, T-15a-023 |
| area:17-source-app-layer | Source registry (`SourceHandle` with optional configurable and resolvable handles), language filter, `getNameForMangaInfo` display rule, stub sources | T-15a-044 (and the registry side of T-15a-014) |
| area:18-extensions-manga and area:20-extension-strategy-redesign | Loader, `SourceFactory` instantiation, trust and NSFW gate; choice of mechanism (D1) | T-15a-015 |
| area:23-downloads-manga | Downloader pipeline that calls the page-list, image URL and image byte operations; queue thresholds and warning notification | T-15a-016, T-15a-027, T-15a-028 |
| area:26-library-update-jobs | Library update rules that consume `update_strategy` and `status`; per-source thresholds (60 manga) | T-15a-016 |
| area:34-images-covers-cache | Chapter cache layout, MD5 cache keys, cover deletion in the detail merge, 100 MiB page-list cache limit | T-15a-027, T-15a-028 |
| area:36-ui-shell-navigation-security-webview | WebView screen used by "Open in WebView"; deep-link screen | T-15a-043 |
| area:38-ui-entry-details-manga | Status badge and web URL actions on the entry screen | T-15a-042 |
| area:40-ui-browse-manga | Browse screen, filter sheet container, listing state UI | T-15a-039, T-15a-040 |
| area:43-reader-core | Reader page loader that uses the `Page` model and page state transitions | T-15a-027 |
| area:47-settings-screens-general | Settings screen container for source settings | T-15a-041 |
| area:51-i18n | Message keys (for example `no_results_found`, `exception_offline`, `page_list_empty_error`, `skipped_reason_completed`) | T-15a-049, T-15a-042 |

### 5.2 What others need from this area

| Area | What it needs from area:15 |
|---|---|
| area:16-source-api-anime | Mirror of the design (trait shapes, id rule, filter model); no shared code |
| area:17-source-app-layer | `MangaSource`, `CatalogueSource`, `ConfigurableSource`, `ResolvableSource` traits; `UnmeteredSource` as `is_unmetered()`; `generate_id` |
| area:18-extensions-manga | `SourceFactory` trait and the source id rule |
| area:20-extension-strategy-redesign | Trait contract that any loader must present (rust-design section 6); `kuro-source` must stay free of GTK and HTTP |
| area:21-backup-format-and-create, area:22-backup-restore | Source id and `source_<id>` key; `UpdateStrategy` ordinal; source preference enumeration (`all()`) that skips unsupported keys |
| area:23-downloads-manga | `Page`, `PageState`, per-page download steps, retry schedule, `EmptyPageList` error |
| area:26-library-update-jobs | `UpdateStrategy`, `MangaStatus` (completed skip rule), skip reason texts |
| area:34-images-covers-cache | Page image cache key rules (MD5 of the URL), page-list JSON shape (`PageCacheEntry`) |
| area:38-ui-entry-details-manga | `MangaStatus` and its display keys; `manga_web_url` |
| area:40-ui-browse-manga | `Filter` enum (eight kinds), `FilterList`, `Filter::select_value()`, `search` and `searchGenre` algorithms |
| area:43-reader-core | `Page`, `PageState`, the reader state transition table, preload and priority rules |
| area:51-i18n | Full list of message keys with English texts (behaviour section 12, filter topic section 4, UI parity section 8) |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export` followed by reading the rows of `plans/TASKS.csv` whose file starts with `15-source-api-manga/` (51 rows, `T-15a-001` to `T-15a-051`, no gaps).

### 6.1 Counts

| Priority | Effort S | Effort M | Effort L | Effort XL | Total |
|---|---|---|---|---|---|
| P0 | 23 | 10 | 1 | 0 | 34 |
| P1 | 8 | 3 | 0 | 0 | 11 |
| P2 | 2 | 1 | 0 | 0 | 3 |
| P3 | 3 | 0 | 0 | 0 | 3 |
| Total | 36 | 14 | 1 | 0 | 51 |

P0 tasks: `T-15a-001`, `002`, `003`, `004`, `005`, `006`, `007`, `008`, `009`, `010`, `011`, `012`, `013`, `014`, `020`, `021`, `022`, `023`, `024`, `027`, `028`, `030`, `031`, `032`, `033`, `034`, `035`, `036`, `039`, `040`, `044`, `048`, `050`, `051`.

P3 tasks are drops, each stating its replacement or the reason for dropping: `T-15a-045` (dead `Video`/`VideoUrl` placeholder and the RxJava `awaitSingle` bridge), `T-15a-046` (Android-only files, `expect`/`actual` declarations, Gradle build), `T-15a-047` (deprecated Rx `fetch*` API and the uncalled `chapterPageParse`).

### 6.2 Critical path (P0), in order

Two spines run in parallel and meet at `T-15a-048`. The filter spine is the longest by effort.

HTTP and parsing spine:
1. `T-15a-001` (S): crate skeleton and `SourceError`.
2. `T-15a-002` (S): `SManga`, status constants.
3. `T-15a-013` (M): `MangaSource` and `CatalogueSource` traits with panic isolation.
4. `T-15a-014` (S): catalogue methods and listing support flags.
5. `T-15a-020` (M): `HttpSource` trait, `HttpCore`, generic `Http<S>` adapter. Needs area:12.
6. `T-15a-030` (M): `ParsedHttpSource` contract with scraper.
7. `T-15a-031` (S): list parsing and `hasNextPage`.
8. `T-15a-048` (M): fixtures and test suite for the source API.

Reader and downloader spine (after step 5):
- `T-15a-027` (M): image URL resolution and page state machines. Needs area:43 and area:23.
- `T-15a-028` (M): image download, progress, retry, file naming. Needs area:34.

Filter spine (longest by effort):
1. `T-15a-001` (S), then `T-15a-008` (S): `Filter` enum.
2. `T-15a-010` (M): Group and Sort selection rules.
3. `T-15a-039` (L): filter sheet renderer for all eight kinds. Needs area:40. This is the largest single task in the area.
4. `T-15a-040` (M): browse filter flows (search, searchGenre, reset, listing state).
5. `T-15a-051` (S): remove the invented sample rows from the mockup browse list (also waits on `T-15a-050`).

Detail merge and chapter sync (parallel, needed before the library works): `T-15a-003` (S), `T-15a-004` (S), `T-15a-035` (M), `T-15a-036` (M).

### 6.3 Risks

1. Source availability (D1, D2): no P0 task is achievable until a source is written in Rust. Impact is high, likelihood is certain. Mitigation: pick the first site now (D2).
2. Selector coverage: Jsoup-only pseudo-classes (`:contains`, `:containsOwn`, `:matches`, `:matchesOwn`, `:containsData`, `:eq`, `:gt`, `:lt`) are not supported by `scraper` ([p1-rust-design.md](p1-rust-design.md) section 13, item 1). Mitigation: convert each selector to standard CSS plus a filter closure, covered by fixture tests. Do not build a Jsoup-compatible engine.
3. HTML tree differences between Jsoup and html5ever (rust-design item 3). Mitigation: fixture tests on saved real pages.
4. Text normalisation of NBSP: Jsoup 1.22.2 (`gradle/libs.versions.toml`, line 31, as cited in the topic doc) must be verified with a fixture before relying on the collapse rule (T-15a-033, EC-PH-03).
5. Blocking area:40 (T-15a-039, T-15a-040) and area:20 (T-15a-015) on decisions outside this area.
6. Crate-name conflict (D3) affects the workspace layout in T-15a-001. Resolve before T-15a-001 starts.
7. Chapter-number and URL behaviour changes (D5, D8) alter stored data relative to Aniyomi. Covered by tests (EC-CH-07, EC-URL-02..03), but a library imported from an Aniyomi backup may show differences.
8. Real-site drift: sites change markup; each saved fixture goes stale. Accepted, no mitigation beyond re-saving fixtures.

### 6.4 Lint status

- `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 15-source-api-manga/p1-tasks.md` prints `ok`. This area's tokens use real area ids, so no edit was needed.
- The shared lint run reports 4 errors in `40-ui-browse-manga/p1-tasks.md` (lines 37, 83, 158, 537): `area:41-ui-browse-manga-part-2` is not a real area id. Those lines belong to area:40 and were not changed here.

---

## 7. Definition of done for this area in Kurobayomi

Each item is an observable behaviour on the GTK app with one working source installed.

1. Source identity: a source named `Example`, lang `en`, version 1 gets id `4851548778462321757`. Changing its name changes the id and its library rows show as stub entries, not as moved rows. Unit tests `TST-01` and `TST-02` pass.
2. Browse popular: the browse screen shows page 1 of popular results (page numbering starts at 1), loads page 2 when the list is scrolled to the end and `hasNextPage` is true, and shows "No results found" when a page is empty.
3. Search with text: typing a query runs `getSearchManga(page, query, filters)` and shows results. Empty result shows "No results found". An HTTP 404 shows "HTTP error 404". Network down shows "No Internet connection".
4. Filter sheet: all eight kinds render from the source's own `FilterList`. Reset replaces the filters but does not re-run the search. Filter runs the search and closes the sheet. TriState cycles 0, 1, 2, 0. A Sort row tap toggles direction on the selected row and selects other rows with the current direction. Text updates on every keystroke.
5. Search by genre: clicking a genre chip sets a matching filter when one exists, otherwise runs a text search with the genre name.
6. Source settings: a source that declares `SourceSetting` items shows the settings entry and each widget kind (Switch, Text, List, MultiSelect, Info, Unsupported). Values persist across restart. A missing value shows the default.
7. Manga details: opening a manga fetches details, writes author, artist, description, genres, status, thumbnail and `initialized = true`. Null source fields keep stored values. Empty genre from the refresh path does not clear stored genres.
8. Chapter list: opening a manga syncs chapters. Duplicate URLs are removed (first occurrence kept). Chapter numbers show as typed (`1.1` shows as `1.1`). An empty list for a non-local source shows "No chapters found" and changes nothing in the database.
9. Reader: opening a chapter loads the page list, reindexes pages by position, and loads the current page first, then up to four following pages. Retry on an errored page works with one click.
10. Downloads: a chapter download fetches at most two pages at a time, retries an image three times after 2 s, 4 s and 8 s, names files with the zero-padded page number (minimum width 3), and shows the page error notification with the message text after four failed attempts.
11. Data saver: with `BANDWIDTH_HERO`, `WSRV_NL` or `RESMUSH_IT` selected, the image request uses the transformed URL, and the page keeps its original URL for the cache key. Excluded sources and `ignore_jpeg` / `ignore_gif` keep the original URL.
12. Deep link: pasting a URL that a source recognises opens the manga. A chapter URL opens the reader at that chapter. An unrecognised URL shows "No results found".
13. Web actions: "Open in WebView" opens the source base URL. "Share" and the chapter web link use the request URLs, and show nothing when the URL cannot be built.
14. Status badge: the entry screen shows Ongoing, Completed, Licensed, Publishing finished, Cancelled, On hiatus, or Unknown for every stored status value, including values outside 0 to 6.
15. Tests: `cargo test -p kuro-source -p kuro-source-http` passes all tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5 (TST-01 to TST-37) that are assigned to these crates. The fixtures in section 4 are checked in.
