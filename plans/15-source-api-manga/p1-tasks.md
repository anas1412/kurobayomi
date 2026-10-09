# Area 15 — Manga source API: implementation tasks

IDs `T-15a-nnn`. Priorities: P0 needed for daily reading from one working source; P1 parity; P2 long tail; P3 dropped or replaced (the task states the replacement). Designs: `p1-rust-design.md`. Behaviour: `p1-behavior.md`. Topics: `p1-topic-http-source-pipeline.md`, `p1-topic-filters.md`, `p1-topic-models-and-sync.md`. Data: `p1-data-and-config.md`. UI: `p1-ui-parity.md`. Tests: `p1-edge-cases-and-tests.md` (EC-*, RC-*, TST-* ids).

Sources lines cite repo-relative Aniyomi paths (under `/home/blackbox/aniyomi-src`) unless prefixed `kurobayomi:`, which is the Kurobayomi repository at `/home/blackbox/kurobayomi`.

## Foundation and models

### T-15a-001 — kuro-source crate skeleton, workspace layout and SourceError
- Priority: P0
- Effort: S
- Depends: none
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/CatalogueSource.kt, kurobayomi:Cargo.toml
- Acceptance:
  - The repository is a Cargo workspace with members `crates/kuro-source` and `crates/kuro-source-http` and the existing package `kurobayomi`; `cargo build --workspace` succeeds on stable Rust.
  - `kuro-source` does not depend on `gtk`, `tokio` runtime features or `reqwest` (checked with `cargo tree -p kuro-source`).
  - `SourceError` has exactly the variants in `p1-rust-design.md` section 3, and `Display` for `Http(404)` is `HTTP error 404`.
  - `cargo test -p kuro-source` runs at least one test (module smoke test).

### T-15a-002 — SManga model, status constants and defaults
- Priority: P0
- Effort: S
- Depends: T-15a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SMangaImpl.kt
- Acceptance:
  - `SManga::new(url, title)` yields `status == 0`, `update_strategy == AlwaysUpdate`, `initialized == false`, all optional fields `None`.
  - Constants `UNKNOWN..ON_HIATUS` equal 0..6 (test TST-03).
  - `MangaStatus` maps 0..6 to the variants and any other value to `Unknown` while `status` keeps the raw value.
  - Status display text keys exist for every variant (area:51) with the English texts in `p1-behavior.md` section 12.

### T-15a-003 — SManga genre parser and detail copy semantics
- Priority: P0
- Effort: S
- Depends: T-15a-002
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt
- Acceptance:
  - `genres()` passes EC-GN-01..06 (TST-04): `None` for null/blank, split on the two-character `", "` only, trim, drop blanks, keep first occurrence.
  - `copy_from` rules of `Manga.copyFrom(SManga)` (behaviour 11) pass EC-MR-01, EC-MR-02, EC-MR-05 when applied to a domain record.
  - Refresh path rules pass EC-MR-03 and EC-MR-04 (`coalesce` semantics: `None` keeps).

### T-15a-004 — SChapter model, defaults and chapter number precision
- Priority: P0
- Effort: S
- Depends: T-15a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapterImpl.kt, app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt
- Acceptance:
  - `SChapter::new(url, name)` yields `date_upload == 0`, `chapter_number == -1.0`, `scanlator == None` (EC-MD-02, EC-MD-03).
  - `copy_from` copies exactly `name`, `url`, `date_upload`, `chapter_number`, `scanlator`.
  - Conversion to the domain chapter number uses the decimal form: `1.1f32` becomes `1.1` (EC-CH-07, TST-24). This is a deliberate correction of the Float-to-Double widening in Kotlin; the change is noted in the migration notes.
  - Scanlator rule: blank becomes `None`, otherwise trimmed (EC-CH-10, EC-CH-11).

### T-15a-005 — Page model with watch-based status and progress
- Priority: P0
- Effort: S
- Depends: T-15a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt
- Acceptance:
  - `Page::new(index, url)` starts with state `Queue` and progress `0`; `number()` equals `index + 1`.
  - `update_progress` returns `-1` for absent or zero length and `100 * read / total` with integer division otherwise, computed in u128 (EC-MD-06..09, TST-05).
  - State and progress are observable through `watch` receivers; a receiver created after a change sees the latest value (test).
  - `PageState` variants are `Queue, LoadPage, DownloadImage, Ready, Error` with ordinals 0..4 in declaration order.

### T-15a-006 — MangasPage model
- Priority: P0
- Effort: S
- Depends: T-15a-002
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/MangasPage.kt
- Acceptance:
  - `MangasPage { mangas, has_next_page }` is constructible from a `Vec<SManga>` and a bool, and equality is structural.
  - Used by every catalogue method in T-15a-014 without conversion.

### T-15a-007 — UpdateStrategy enum and stored ordinal codec
- Priority: P0
- Effort: S
- Depends: T-15a-002
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt, data/src/main/java/tachiyomi/data/DatabaseAdapter.kt
- Acceptance:
  - `AlwaysUpdate = 0` and `OnlyFetchOnce = 1`; conversion from i64 maps any unknown value to `AlwaysUpdate` (EC-MD-05).
  - Encoding writes the ordinal; a round trip of both variants is an identity.
  - The library update job skips `OnlyFetchOnce` with the `skipped_reason_not_always_update` text (area:26).

## Filters

### T-15a-008 — Filter enum: Header, Separator, Select, Text, CheckBox
- Priority: P0
- Effort: S
- Depends: T-15a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt
- Acceptance:
  - Variants and default states match `p1-data-and-config.md` section 1.6: Header and Separator carry only a name; Select default index 0; Text default empty; CheckBox default false.
  - Equality is name plus state; hash is consistent with equality (TST-07).
  - `Filter::select_value()` returns `None` for an out-of-range index (EC-FI-01 at model level).

### T-15a-009 — TriState states and raw value mapping
- Priority: P0
- Effort: S
- Depends: T-15a-008
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt, core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt
- Acceptance:
  - Raw values 0, 1, 2 map to Ignore, Include, Exclude; raw 3 and -1 return `SourceError::Parse` (EC-FI-02, TST-08).
  - Next-state cycle is `Ignore → Include → Exclude → Ignore` (TriState.kt:9-15).

### T-15a-010 — Group and Sort selection rules
- Priority: P0
- Effort: M
- Depends: T-15a-008
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt
- Acceptance:
  - Sort click rules (behaviour topic 5.7): same index toggles direction; other index keeps direction (EC-FI-04, EC-FI-05, TST-09).
  - Sort with no selection: tapping any row selects it with `ascending = true` (EC-FI-03; documented change from the Kotlin NullPointerException).
  - Group holds nested filters of any kind, including nested groups; rendering helpers iterate recursively (EC-FI-07).

### T-15a-011 — FilterList container and structural equality
- Priority: P0
- Effort: S
- Depends: T-15a-008
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/FilterList.kt
- Acceptance:
  - `FilterList` derefs to a slice, is `Clone`, and `FilterList::default()` is empty.
  - Equality is structural (EC-FI-11); this deliberately differs from Kotlin's always-false equals, documented in the crate docs.
  - Searches receive a clone of the list (RC-06, RC-07).

### T-15a-039 — Filter sheet renderer for all eight filter kinds
- Priority: P0
- Effort: L
- Depends: T-15a-008, T-15a-009, T-15a-010, T-15a-011, area:40-ui-browse-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt, kurobayomi:spec/entries.json, kurobayomi:src/screens.rs
- Acceptance:
  - The sheet renders a sticky header with Reset and Filter buttons, then rows for each kind as in `p1-ui-parity.md` section 2.2.
  - CheckBox toggles; TriState cycles 0→1→2→0; Text writes on each change (EC-FI-06); Select shows the value at the index and writes the chosen index; out-of-range shows no value and does not crash (EC-FI-01).
  - Group and Sort are collapsible; nested filters render and write state (EC-FI-07).
  - The mockup `browse_source` filter sheet items are no longer used for real values; the screen is built from `FilterList` (verified by a test that feeds a list with a Text item named "Query" and sees that row).
  - Keyboard: Tab and Space or Enter operate every row; Escape closes the sheet.

### T-15a-040 — Browse filter flows: search, searchGenre, reset, listing state
- Priority: P0
- Effort: M
- Depends: T-15a-039, T-15a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt
- Acceptance:
  - Popular, Latest and Search listings are distinct states; the sentinel query strings map as in the filter topic section 3.1 (`valueOf` rule).
  - `search(query, filters)` creates or updates `Listing::Search` exactly as in the filter topic section 3.2.
  - `searchGenre` passes EC-FI-08 and EC-FI-09 (TST via `searchGenre` fixtures) with the Select branch using the unfiltered `values` array.
  - Reset replaces all filter objects and does not start a search (EC-FI-10); Filter starts a search with the current sheet state.
  - A running listing is not mutated by sheet edits (RC-06).

### T-15a-044 — Source display name and language label
- Priority: P0
- Effort: S
- Depends: T-15a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - `Display` for a source is `"{name} ({LANG})"` with uppercase language (HttpSource.kt:103); `getNameForMangaInfo` logic reproduced: name only when exactly one active language and the source language is in it, otherwise the full display (behaviour 1.2).
  - Active languages come from the `source_languages` preference with default `{all, en, system language}` (data file 4.2).
  - Test covers `Example` + `en` → `Example (EN)` and the one-language rule.

## Identity and HTTP base

### T-15a-012 — generate_id (source id MD5 algorithm) with test vectors
- Priority: P0
- Effort: S
- Depends: T-15a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - `generate_id("Example","en",1) == 4851548778462321757`, `generate_id("MangaDex","en",1) == 2499283573021220255`, `generate_id("Ünïcode Src","ja",2) == 8842957433569127029` (EC-ID-01..03).
  - The result is non-negative for 1000 generated inputs (EC-ID-04).
  - The key is `name.to_lowercase() + "/" + lang + "/" + version_id` (not lowercasing `lang`); changing `version_id` changes the id (EC-ID-06).
  - Uses the `md-5` crate; output identical to the Kotlin function for the vectors.

### T-15a-013 — MangaSource and CatalogueSource traits with panic isolation
- Priority: P0
- Effort: M
- Depends: T-15a-002, T-15a-004, T-15a-005, T-15a-011
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/CatalogueSource.kt
- Acceptance:
  - `MangaSource` has `id`, `name`, `lang` (default empty), `get_manga_details(&SManga)`, `get_chapter_list(&SManga)`, `get_page_list(&SChapter)`, `is_unmetered` (default false); traits are object-safe and used as `Arc<dyn MangaSource>`.
  - `CatalogueSource` adds `supports_latest`, `filter_list`, `get_popular_manga`, `get_search_manga`, `get_latest_updates`.
  - Trait upcasting from `Arc<dyn CatalogueSource>` to `Arc<dyn MangaSource>` compiles on the pinned toolchain.
  - A source call that panics inside its spawned task returns `SourceError::Parse("source panicked")` and does not abort the process (EC-HT, section 2 of the edge-case file; TST-37).
  - The Rx `fetch*` methods and `chapterPageParse` are absent (T-15a-047).

### T-15a-014 — Catalogue methods and listing support flags
- Priority: P0
- Effort: S
- Depends: T-15a-013, T-15a-011
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/CatalogueSource.kt, data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt
- Acceptance:
  - Popular, search and latest are called with `page` starting at 1 (paging rule in topic models section 6).
  - When `supports_latest()` is false the browse screen does not offer the Latest listing (area:40 acceptance; the flag is read from the trait).
  - An empty page is reported to the UI as the "No results found" state, not as an end-of-list marker (EC: section 1.2 of the UI doc).
  - Pager page size 25 is a display setting (area:06).

### T-15a-015 — SourceFactory and extension loader contract
- Priority: P1
- Effort: S
- Depends: T-15a-013, area:20-extension-strategy-redesign, area:18-extensions-manga
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/SourceFactory.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - `SourceFactory::create_sources()` returns a `Vec<Arc<dyn CatalogueSource>>`; a factory returning zero sources is accepted and logged.
  - The loader contract matches MangaExtensionLoader.kt:317-333: each class entry yields one source or many; a failing entry is skipped with a logged error and the other entries still load (test with one failing fake).
  - Which loading mechanism is used (in-process, WASM, Lua) is decided by area:20; this task only fixes the trait.

### T-15a-016 — UnmeteredSource marker and per-source queue thresholds
- Priority: P1
- Effort: S
- Depends: T-15a-014, area:23-downloads-manga, area:26-library-update-jobs
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/UnmeteredSource.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt
- Acceptance:
  - `is_unmetered()` excludes a source from the counts in both warnings (EC-UM-01..06, TST-34).
  - Constants: download queue 30 total, 15 per source, notification timeout 30 000 ms; library update 60 per source (behaviour section 17).
  - Warning fires only when the queue was empty before the add and auto-start is on (download path).

### T-15a-017 — ResolvableSource (deep link) trait and UriType
- Priority: P2
- Effort: S
- Depends: T-15a-013
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ResolvableSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt
- Acceptance:
  - `UriType { Manga, Chapter, Unknown }`; `uri_type`, `manga_from_uri`, `chapter_from_uri` as in the design.
  - Resolution takes the first source in list order whose `uri_type != Unknown` (EC-DL-04); chapter links look up the chapter only when the type is `Chapter` and a manga resolved (EC-DL-02).
  - An error inside `manga_from_uri` yields `NoResults` and a log entry (EC-DL-03, documented change).

### T-15a-018 — ConfigurableSource trait and SourceSetting schema
- Priority: P1
- Effort: M
- Depends: T-15a-013, area:02-preferences-system
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt
- Acceptance:
  - `SourceSetting` variants `Switch, Text, List, MultiSelect, Info, Unsupported` with the fields in the design section 6.1.
  - `Http<S>` implements `ConfigurableSource` and the registry marks a source configurable only when `is_configurable()` returns true (the Kotlin `is ConfigurableSource` rule).
  - Unsupported widgets render as Info rows and never write a value (EC-PR-03).
  - `preference_key(id) == "source_" + id` (EC-PR-01).

### T-15a-019 — Per-source preference store and backup scope
- Priority: P1
- Effort: M
- Depends: T-15a-018, area:21-backup-format-and-create, area:22-backup-restore
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt
- Acceptance:
  - `SourcePrefs` backed by the `source_prefs` table (data file section 7 / design section 7); get, set, remove, string set and bool work; absent keys return `None` (EC-PR-02).
  - Backup includes one entry per configurable source with at least one non-app-state key, values of types Int, Long, Float, String, Boolean and Set of String (EC-PR-04..06).
  - Restore writes into `source_<id>` even when the source is not installed (EC-PR-07).
  - Sources without settings are absent from the backup file.

### T-15a-020 — HttpSource trait, HttpCore and the generic Http adapter
- Priority: P0
- Effort: M
- Depends: T-15a-014, area:12-network-stack
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - `HttpSource` requires `base_url`, `name`, `lang`, `supports_latest`, all request builders and parsers; `version_id` defaults to 1.
  - `Http<S>` implements `MangaSource` and `CatalogueSource`; details sets `initialized = true`; page list reindexes (T-15a-034).
  - Default builders produce `GET baseUrl + path` for details, chapter list and page list and `GET page.url` for image URLs (behaviour 2.2; EC-URL-06).
  - Test: a fake source with a wiremock server returns parsed details and the request path matches `manga.url`.

### T-15a-021 — Headers and User-Agent resolution
- Priority: P0
- Effort: S
- Depends: T-15a-020, area:12-network-stack
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/UserAgentInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt
- Acceptance:
  - Default headers contain `User-Agent` from `default_user_agent`, trimmed; default value is the Firefox 136 string in data file 4.2.
  - Headers are computed once per source instance (`OnceLock`); a source's own UA header is sent unchanged; the network layer adds the global UA only when a request has none (behaviour 1.5).
  - Test covers both cases.

### T-15a-022 — Default request builder and cache directive
- Priority: P0
- Effort: S
- Depends: T-15a-020
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt
- Acceptance:
  - Builders set method GET, the headers, and `cache_max_age = 600 s` (DEFAULT_CACHE_CONTROL).
  - The POST helper is not needed for any default and is absent.
  - A test asserts the three values on a built request.

### T-15a-023 — Success check, HttpException mapping and transport errors
- Priority: P0
- Effort: S
- Depends: T-15a-020, area:12-network-stack
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt
- Acceptance:
  - 2xx passes; every other status returns `Http(code)` and the body is never parsed (EC-HT-01..06, TST-13).
  - Redirect results expose the final URL on `HttpResponse.url` (EC-HT-03).
  - Connect and TLS failures become `Transport(text)`; a timeout test (black-hole server) passes (TST-14, EC-HT-07).

### T-15a-024 — URL without domain (setUrlWithoutDomain)
- Priority: P0
- Effort: S
- Depends: T-15a-020
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - Path, query and fragment are kept in their encoded form; spaces become `%20` before parsing (EC-URL-01, EC-URL-02).
  - Encoded reserved characters are not decoded (EC-URL-03, documented difference from Kotlin's decoded `getPath()`).
  - Opaque and unparseable inputs are returned unchanged or as `Url` errors without producing the text `null` (EC-URL-04, EC-URL-05).

### T-15a-025 — Manga and chapter web URLs
- Priority: P1
- Effort: S
- Depends: T-15a-020
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `manga_web_url` equals the details request URL; `chapter_web_url` equals the PAGE-LIST request URL (behaviour 7; not the chapter-list URL).
  - Share, Open in WebView (source base URL) and the reader's open-in-browser action use these functions; a failure shows no action.
  - Test: a source overriding `page_list_request` changes `chapter_web_url` only.

### T-15a-026 — prepare_new_chapter hook
- Priority: P1
- Effort: S
- Depends: T-15a-020, area:07-domain-entries-items
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt
- Acceptance:
  - The hook is called for every chapter on every sync for HTTP sources only (EC-CH-06), before the database lookup (EC-CH-05 asserts the url-rewrite consequence).
  - The default is a no-op; the hook receives a mutable `SChapter` and a read-only `SManga`.
  - Documentation string states "called for every chapter on every sync", replacing the Kotlin doc's "new chapter" wording.

## Images and reader/downloader

### T-15a-027 — Image URL resolution and page state machines (reader and downloader)
- Priority: P0
- Effort: M
- Depends: T-15a-020, T-15a-005, area:43-reader-core, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - Reader: one worker task with a `BinaryHeap` ordered by priority desc then sequence asc; priorities preload 0, current 1, retry 2; preload size 4 (behaviour 18, HTTP pipeline topic section 8).
  - The reader state table in the topic file is enforced: transitions tested (TST-27); EC-IM-01..04 pass.
  - Downloader: image URL step sets `LoadPage` and `Error` as in the HTTP pipeline topic section 9; a failed URL step does not notify (documented observed behaviour).
  - `MissingImageUrl` is returned, not panicked, when an image is requested without a URL (EC-URL-09).
  - Cancellation of the reader leaves no QUEUE entries (RC-03, TST-36).

### T-15a-028 — Image download: streaming, progress, retry schedule and file naming
- Priority: P0
- Effort: M
- Depends: T-15a-027, area:12-network-stack, area:34-images-covers-cache
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/ProgressResponseBody.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `execute_streaming` has no cache, a 30-hour overall timeout, and reports `(bytes, total, done)` per chunk; progress maps through `update_progress` (EC-HT-09, EC-HT-10).
  - Download retries 3 times after the first attempt with waits 2 s, 4 s and 8 s; 3 failures then success yields READY (EC-IM-14); 4 failures yield ERROR and a notification (EC-IM-15). Test uses paused tokio time.
  - File name is `format!("{:0width$}", number)` with width `max(3, digits(total))`; temp file `name.tmp` renamed to `name.ext` on success; temp file removed on failure (EC-IM-17, EC-IM-18, TST-29).
  - Cancellation deletes the temp file and does not set ERROR (EC-IM-16).
  - At most 2 pages of one download are in flight (`buffer_unordered(2)`).

### T-15a-029 — Data saver modes
- Priority: P2
- Effort: M
- Depends: T-15a-028, area:02-preferences-system
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Modes NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT with the URL rules of data file section 6.2 and HTTP pipeline topic section 10; defaults from data file 4.2.
  - Excluded source ids (`data_saver_excluded`) and the downloader toggle (`data_saver_downloader`, default true) are honoured (EC-IM-12, EC-IM-13).
  - Test fixtures in `fixtures/datasaver` pass (TST-30); the ReSmush response without `"dest":"` is a `Parse` error (EC-IM-11); WSRV and ReSmush URLs are percent-encoded (EC-IM-10, documented change).
  - The mode is read once at download start (RC-05).

## Parsing

### T-15a-030 — ParsedHttpSource contract with scraper
- Priority: P0
- Effort: M
- Depends: T-15a-020
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ParsedHttpSource.kt
- Acceptance:
  - The trait has selector and element hooks named as in the Kotlin base class; `scraper::Html` is used only in synchronous parse functions (no `.await` while a document is alive; checked by review and by `Send` assertion tests on the public futures).
  - Invalid selectors return `Parse` (EC-PH-15).
  - Jsoup-only pseudo-classes are listed in the port notes and converted to CSS plus a filter closure in each ported source (pitfall 1 in the design).

### T-15a-031 — List parsing and hasNextPage
- Priority: P0
- Effort: S
- Depends: T-15a-030
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ParsedHttpSource.kt
- Acceptance:
  - `has_next_page` is true only when the next-page selector matches, false for `None` or no match (EC-PH-10, EC-PH-11); presence only, not text (EC-PH-12).
  - Zero list items with a next link gives an empty list with `has_next_page = true` (EC-PH-13).
  - One failing element fails the whole page (EC-PH-14).
  - Fixtures `list_popular.html` and `list_popular_no_next.html` pass.

### T-15a-032 — Jsoup-compatible element helpers
- Priority: P0
- Effort: S
- Depends: T-15a-030
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsoupExtensions.kt
- Acceptance:
  - `select_text`: `None` for no match, `Some("")` for an empty match (EC-PH-01, EC-PH-02).
  - `select_int`: default for no match, `Parse` for empty or non-numeric text, accepts a leading `+` (EC-PH-04..07).
  - `attr_or_text`: operates on the element itself; `"text"` returns the element text (EC-PH-08, EC-PH-09).
  - `text()` collapses ASCII whitespace and NBSP and trims (EC-PH-03, TST-16).

### T-15a-033 — Response to HTML: base URI, charset and BOM
- Priority: P0
- Effort: S
- Depends: T-15a-030
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsoupExtensions.kt
- Acceptance:
  - Base URI is the final request URL after redirects (EC-HT-03); relative links resolve against it (EC-PH-16).
  - Charset from `Content-Type` is honoured, default UTF-8 (EC-PH-18).
  - A UTF-8 BOM is removed before parsing (EC-PH-17); an empty body for JSON is a `Parse` error (EC-PH-19).
  - Test: `fixtures/source/windows1252.html` decodes correctly.

### T-15a-034 — Page list parsing and reindexing
- Priority: P0
- Effort: S
- Depends: T-15a-005, T-15a-030
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - Pages get `index = position` regardless of the source value (EC-PL-02).
  - An empty page list in the downloader fails with `EmptyPageList` (message "No pages found") (EC-PL-01).
  - A cached page list is used as is; image failures do not invalidate it (EC-PL-03).
  - Page list cache write keeps only `index`, `url`, `imageUrl` (T-15a-038).

### T-15a-035 — Chapter list parsing and sync-side rules
- Priority: P0
- Effort: M
- Depends: T-15a-004, T-15a-030, area:07-domain-entries-items, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt, data/src/main/java/tachiyomi/data/items/chapter/ChapterSanitizer.kt
- Acceptance:
  - Source list order is preserved and `source_order` is the position after `distinctBy url` (EC-CH-02).
  - Empty source list for a non-local source fails with `NoChaptersException` and the DB is untouched (EC-CH-01).
  - New chapters with `date_upload == 0` take `now` for the first and the running `maxSeenUploadDate` afterwards (EC-CH-03); `date_fetch` is `now + n` decreasing with list position (EC-CH-04).
  - Name sanitising: trim, exact case-sensitive prefix removal of the manga title, trim of the listed characters (EC-CH-08, EC-CH-09).
  - Concurrent syncs for one manga are serialised (RC-08).

### T-15a-036 — Manga detail merge rules and update strategy round trip
- Priority: P0
- Effort: M
- Depends: T-15a-003, T-15a-013, area:10-app-domain-layer, area:03-db-manga-schema-migrations
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
  - Metadata path (`copy_from`) and refresh path (`UpdateManga`) produce the results of the test vectors in `p1-topic-models-and-sync.md` section 11 (EC-MR-01..06).
  - `to_s_manga` copies `update_strategy` and a round trip of `OnlyFetchOnce` keeps it (EC-MR-07, documented change).
  - Cover refresh rules: no cover change for an empty thumbnail; `now` for a changed thumbnail; custom covers are kept (behaviour 11; area:34 owns the files).
  - `initialized` is true after a refresh and `false && true = false` after the metadata path.

### T-15a-037 — Default JSON options for source payloads
- Priority: P1
- Effort: S
- Depends: T-15a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsonExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - `default_json_options()` ignores unknown fields and treats absent fields as defaults (`ignoreUnknownKeys`, `explicitNulls = false` behaviour).
  - Test: a JSON payload with an extra key parses; a missing optional key yields `None`.

### T-15a-038 — Page cache serialization shape
- Priority: P1
- Effort: S
- Depends: T-15a-005
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt
- Acceptance:
  - `PageCacheEntry` serialises `index`, `url`, and `imageUrl` (only when present) (EC-MD-10).
  - Missing `index` is an error; unknown keys are ignored (EC-MD-11, EC-MD-12).
  - Cache key is the lowercase MD5 hex of `"{mangaId}{chapterUrl}"` (behaviour data section 5), keeping existing cache files valid.

## Settings, browse UI and deep links

### T-15a-041 — Source settings screen renderer
- Priority: P1
- Effort: M
- Depends: T-15a-018, area:47-settings-screens-general, area:40-ui-browse-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt, kurobayomi:src/screens.rs
- Acceptance:
  - Title is "Name (LANG)" for the source; a loading state is shown until sources load (UI doc section 3.1).
  - Each `SourceSetting` renders as the widget in the UI doc section 3.2; Text, List and MultiSelect open a dialog with OK and Cancel; OK persists, Cancel does not.
  - Changed values are read by the source on the next call (RC-10).
  - Settings screen is reachable only for configurable sources (browse toolbar Settings item).

### T-15a-042 — Manga status badge on the entry screen
- Priority: P1
- Effort: S
- Depends: T-15a-002, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - Icon and text for each status exactly as in behaviour section 12 (seven rows including unknown).
  - Text is single-line with ellipsis; stub-source warning icon shown for stub sources.
  - Badge text keys resolve through area:51.

### T-15a-043 — Deep link resolution flow
- Priority: P2
- Effort: S
- Depends: T-15a-017, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt
- Acceptance:
  - States `Loading`, `NoResults`, `Result(manga)`, `Result(manga, chapterId)` as in behaviour section 16.
  - A link opened from the desktop launcher or a pasted URL reaches the same flow (area:36).
  - Test with fake resolvers covers EC-DL-01..04.

## Drops and replacements

### T-15a-045 — Drop Video/VideoUrl and the RxJava awaitSingle bridge
- Priority: P3
- Effort: S
- Depends: T-15a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt
- Acceptance:
  - No `Video`, `VideoUrl` or `awaitSingle` symbol exists in the Rust workspace (grep test).
  - Drop reason documented: dead code in the manga scope; Rx replaced by async functions; no user-visible loss.

### T-15a-046 — Drop Android-only source-api files, expect/actual and Gradle build
- Priority: P3
- Effort: S
- Depends: T-15a-001
- Sources: source-api/build.gradle.kts, source-api/src/androidMain/AndroidManifest.xml, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/PreferenceScreen.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/source/PreferenceScreen.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt
- Acceptance:
  - The Rust workspace has no Gradle or Android manifest files for these crates.
  - `PreferenceScreen` is replaced by `SourceSetting` (T-15a-018); the anime `PreferenceScreen` typealias is out of scope (area:16).
  - Dependency mapping recorded in `p1-rust-design.md` section 12 and in the crate `Cargo.toml` comments.

### T-15a-047 — Drop deprecated Rx fetch API and chapterPageParse
- Priority: P3
- Effort: S
- Depends: T-15a-013
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/CatalogueSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - No `fetch_*` function and no Rx type appears in the Rust crates.
  - `chapterPageParse` has no caller in Aniyomi (grep verified in the behaviour spec section 4.4) and is not ported.
  - Drop reason: removed API surface, no caller; user loses nothing.

### T-15a-048 — Fixtures and test suite for the source API
- Priority: P0
- Effort: M
- Depends: T-15a-012, T-15a-030, T-15a-031, T-15a-032, T-15a-033
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ParsedHttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - Fixture directories from `p1-edge-cases-and-tests.md` section 4 exist, each file with a source note (URL and capture date) and no personal data.
  - Tests TST-01..TST-26 and TST-30 exist and pass with `cargo test --workspace`.
  - `cargo clippy --workspace -- -D warnings` passes.
  - Each EC-* row marked "documented change" has a test asserting the new behaviour.

### T-15a-049 — Error messages and formatting for source errors
- Priority: P1
- Effort: S
- Depends: T-15a-023, area:51-i18n
- Sources: app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - `Http(code)` text "HTTP error {code}"; `Transport` offline text "No Internet connection" when the network is down, otherwise "Couldn't reach {host}" (keys `exception_offline`, `exception_unknown_host`).
  - `EmptyPageList` → "No pages found"; `NoChaptersException` → "No results found" in browse and "No chapters found" in library updates.
  - `SourceNotInstalled` → "Source not found".

### T-15a-050 — GTK main thread and tokio bridge for source calls
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap, T-15a-013
- Sources: kurobayomi:src/main.rs, kurobayomi:Cargo.toml
- Acceptance:
  - One multi-thread tokio runtime is created at start-up; source calls never block the GTK thread (test: a source that sleeps 2 s leaves the main loop responsive within 50 ms).
  - Results return to widgets only through `glib::MainContext::spawn_local`; no widget is touched from a tokio task (review checklist plus a debug assertion).
  - Browse listing requests are cancelled when the listing changes (CancellationToken).

### T-15a-051 — Remove invented sample rows from the mockup browse list
- Priority: P0
- Effort: S
- Depends: T-15a-039, T-15a-050
- Sources: kurobayomi:src/screens.rs, kurobayomi:spec/entries.json
- Acceptance:
  - `Source A` and `Source B` sample rows are no longer produced by any builder; the browse list is produced from the source registry (area:17).
  - The `browse_source` spec is used only for labels and menu texts; filter values come from `FilterList`.
  - A grep for "Source A" in `src/` returns nothing.
