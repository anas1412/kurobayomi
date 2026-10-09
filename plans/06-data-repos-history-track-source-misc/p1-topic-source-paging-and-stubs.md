# Topic: source list flows, stub sources and remote paging

Scope files: `data/src/main/java/tachiyomi/data/source/{manga,anime}/{*SourceRepositoryImpl,*SourcePagingSource,*StubSourceRepositoryImpl}.kt` (8 files). Read alongside: `domain/src/main/java/tachiyomi/domain/source/{manga,anime}/{model,repository,interactor}/**`, `app/src/main/java/eu/kanade/tachiyomi/source/{manga/AndroidMangaSourceManager,anime/AndroidAnimeSourceManager}.kt`, `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt`, `BrowseMangaSourceScreenModel.kt:105-125`, `BrowseAnimeSourceScreenModel.kt:105-125`.

The source *objects* (`CatalogueSource`, `AnimeSource`, `HttpSource`, filters, `MangasPage`) are owned by area:15-source-api-manga, area:16-source-api-anime and area:17-source-app-layer (the `SourceManager`). This area owns (a) the view of those objects as `Source` rows for the UI, (b) the persistent stub-source table, (c) the paging algorithm over a source's `popular/latest/search` calls.

## 1. Contracts this area depends on

Rust traits (owned by area:15/16/17; names here are the minimum this area calls — the owning areas may spell them differently):

```rust
#[async_trait]
pub trait CatalogueSource: Send + Sync {
    fn id(&self) -> i64; fn lang(&self) -> &str; fn name(&self) -> &str; fn supports_latest(&self) -> bool;
    fn is_http(&self) -> bool;                                         // `is HttpSource` (false for the Local source)
    async fn popular(&self, page: u32) -> Result<ItemsPage<SManga>, SourceError>;
    async fn latest(&self, page: u32) -> Result<ItemsPage<SManga>, SourceError>;
    async fn search(&self, page: u32, query: &str, filters: &FilterList) -> Result<ItemsPage<SManga>, SourceError>;
}
pub struct ItemsPage<T> { pub items: Vec<T>, pub has_next_page: bool }
pub trait SourceManager {                                              // area:17
    fn catalogue_sources(&self) -> watch::Receiver<Arc<Vec<Arc<dyn CatalogueSource>>>>;   // Flow<List<CatalogueSource>>
    fn get(&self, id: i64) -> Option<Arc<dyn CatalogueSource>>;
    fn get_or_stub(&self, id: i64) -> Arc<dyn CatalogueSource>;        // StubSource when not installed
}
```

The anime flavour is the same with `SAnime`/`AnimeFilterList` and `sources()` (all anime sources, not only catalogue ones).

## 2. Source list flows (`SourceRepo`)

Implemented as `kuro_core::data::source::{manga,anime}::SourceRepo { db: Db, sources: Arc<dyn SourceManager> }`.

| Method | Return | Algorithm |
|---|---|---|
| `sources()` | `Stream<Vec<Source>>` | for each emission of `catalogue_sources()`: map each to `Source { id, lang, name, supports_latest: src.supports_latest(), is_stub: false, pin: unpinned, is_used_last: false }` |
| `online_sources()` | `Stream<Vec<Source>>` | same, only those with `is_http()`; `supports_latest = false` (Kotlin leaves it at the mapper default) |
| `sources_with_favorite_count()` | `Stream<Vec<(Source, i64)>>` | `combine_latest(db.watch({Mangas}, favorite_counts), sources.catalogue_sources())` and map on either change; for each `(source_id, count)` row: `src = sources.get_or_stub(id)`; `Source { id: src.id(), lang, name, supports_latest: false, is_stub: src.is_stub(), .. }` |
| `sources_with_non_library_manga()` (manga only) | `Stream<Vec<(Source, i64)>>` | `db.watch({Mangas}, non_library_counts)` mapped through `get_or_stub`; does NOT re-emit when the source list changes (parity) |

`combine_latest` semantics (kotlinx `combine`): no output until both inputs have produced a value; afterwards every input emission produces an output computed from the latest values; implementation: `tokio::select!` over the db `Subscription` and the `watch::Receiver`, caching the last db rows.

Sort order: SQL has no `ORDER BY` (`GROUP BY source` returns groups in source-id order in practice). Consumers (`GetMangaSourcesWithFavoriteCount` in `app/src/main/java/eu/kanade/domain/source/manga/interactor/`, area:10) sort by name; the Rust query adds `ORDER BY source`.

Stub flag (`isStub`) rule: true iff `getOrStub` returned a `StubMangaSource` (source not currently installed). The UI renders such rows with the stored name/lang and an "uninstalled" treatment (area:40).

## 3. Stub source repository

Table `sources(_id, lang, name)` in the manga DB, `animesources` in the anime DB. Rust `StubSourceRepo { db, kind }`:

```rust
pub async fn all(&self) -> Result<Vec<StubSource>, DbError>;                 // SELECT * FROM sources
pub fn watch_all(&self) -> Subscription<Vec<StubSource>>;                    // table Sources / AnimeSources
pub async fn get(&self, id: i64) -> Result<Option<StubSource>, DbError>;
pub async fn upsert(&self, id: i64, lang: &str, name: &str) -> Result<(), DbError>;
```

Behaviour of the (area:17) manager that drives it, restated because the table's contents depend on it:
* On every change of the installed-extension list the manager rebuilds its source map (always containing the Local source) and, for every source of every extension, calls `registerStubSource(StubMangaSource.from(source))`: read the stored row; if equal (id, lang, name) do nothing; else `upsert`; if a row existed before (the name/lang changed), tell the download manager to rename the source's download folders (`renameSource(old, new)`, area:23/24).
* `getOrStub(id)`: installed source, else cached stub, else (blocking!) `createStubSource(id)`: stored row -> else data from the extension manager's *available* extension sources (`extensionManager.getSourceData(id)`, which also registers/upserts it) -> else `StubMangaSource(id, "", "")` (blank name and lang: `toString()` prints the id).
* The table is never pruned: sources stay known forever so a library entry from a removed extension still shows its source name.
Rust deviation: `get_or_stub` must not block a runtime worker (`runBlocking` in Kotlin). Proposal: area:17 preloads the whole stub table into an in-memory map at startup and keeps it updated from `watch_all()`; `get_or_stub` is then a map lookup that falls back to the blank stub.

## 4. `RemotePager`

```rust
pub enum Listing { Popular, Latest, Search { query: String, filters: FilterList } }

pub struct RemotePager<T> {
    source: Arc<dyn CatalogueSource>, listing: Listing,
    next_key: Option<u32>,      // page number of the next append; None = end of list or not yet loaded
    loaded_any: bool,
}
#[derive(Debug)]
pub enum PagerError { NoResults, SourceNotInstalled, Source(SourceError) }

impl RemotePager {
    pub fn new(source: Arc<dyn CatalogueSource>, listing: Listing) -> Self;
    /// Paging3 REFRESH: key = null -> page 1. Replaces whatever the UI showed. On error the pager state is unchanged.
    pub async fn refresh(&mut self) -> Result<Vec<SManga>, PagerError>;
    /// Paging3 APPEND with key = next_key. Ok(None) if there is no next page. On error `next_key` is unchanged so `append()` is also the retry.
    pub async fn append(&mut self) -> Result<Option<Vec<SManga>>, PagerError>;
    pub fn has_next(&self) -> bool;
}
```

Algorithm of the private `load(page)` (exact port of `MSP:43-61`):

```text
page = key.unwrap_or(1)
result = source.<popular|latest|search>(page)        # search additionally receives (query, filters)
if result is Err(e)           -> Err(map(e))       # includes stub: SourceNotInstalled
if result.items.is_empty()    -> Err(NoResults)    # applies to EVERY page, also an empty page N+1 after has_next_page = true
next_key = if result.has_next_page { Some(page + 1) } else { None }
Ok(result.items)
```

Mapping of errors to the message shown (area:40/41 use `ExceptionFormatter`): `NoResults` -> `MR.strings.no_results_found` "No results found"; `SourceNotInstalled` -> `loader_not_implemented_error` "Source not found"; `SourceError::Http(code)` -> `exception_http` "HTTP %d, check website in WebView"; `SourceError::Dns/offline` -> `exception_offline` ("No Internet connection") when the machine has no connectivity (`NetworkMonitor` of area:12) else `exception_unknown_host` ("Couldn't reach %s"); everything else `"<ErrorKind>: <message>"` (plain "Exception"/"IOException" kinds show just the message).

Listing routing (`GetRemoteManga.subscribe`): sentinel queries `QUERY_POPULAR = "eu.kanade.domain.source.manga.interactor.POPULAR"`, `QUERY_LATEST = "eu.kanade.domain.source.manga.interactor.LATEST"` (anime: `...anime.interactor.POPULAR/LATEST`) exist only because the Kotlin screen state stores one `String? query`. Rust uses the `Listing` enum directly; the sentinel constants are not ported (the persisted/serialised use of those strings: none — they are in-memory only, verified: not part of any preference or backup).

Divergences from Paging3 to be aware of (all deliberate):
1. Paging3 serialises loads per `PagingSource` and cancels in-flight refresh when invalidated; the Rust pager takes `&mut self`, so the UI keeps one pager per listing instance inside the screen's state, and **replaces** it (dropping the in-flight future cancels the HTTP request) whenever query or filters change (Kotlin builds a new `Pager` through `distinctUntilChanged()` on the listing).
2. `prefetchDistance = 25`: the grid asks for `append()` when the user scrolls within 25 items of the end and no load is in flight and `has_next()`. `initialLoadSize = 75` has no effect (sources dictate page size).
3. `getRefreshKey` is irrelevant: invalidation recreates the pager; the first load is always page 1.
4. Kotlin catches `Exception` including `CancellationException`, converting cancellation into a `LoadResult.Error`; Rust propagates cancellation by drop and never reports it as an error.
5. Duplicates across pages are removed downstream by mapping each item to its local DB row through `networkToLocalManga` (`(source, url)` identity, area:10), then (optionally) hiding library items (`hideInLibraryItems`). The pager returns raw source items.

## 5. Repository factories

```rust
pub fn pager(&self, source_id: i64, listing: Listing) -> Result<RemotePager, PagerError>
```

Manga: `sources.get(source_id)` returning `None` -> `Err(PagerError::SourceNotInstalled)` instead of the Kotlin `NullPointerException` from `null as CatalogueSource` (area:40 shows "Source not found" in the list area, same text as the anime path). Anime: `get_or_stub` and let the first `load` fail with `SourceNotInstalled` (identical user-visible result).

## 6. UI contracts for the consumers (area:40/41)

* List states: `refresh` in flight -> centered spinner; error on refresh -> full-area error with message + Retry (`action_retry`) + "Open in WebView" for HTTP errors (area:36); append in flight -> footer spinner; append error -> footer row with message and Retry; end of list (`next_key == None` after a successful page) -> nothing (no "no more results" row is produced by this layer; `MR.strings.no_more_results` is a UI string used by global search).
* Grid column count and cover density are library preferences owned by area:47, not by this area.
* Retry = call `append()` again for append errors, `refresh()` for refresh errors.

## 7. Tests

See `p1-edge-cases-and-tests.md` section 4: fake `CatalogueSource` scripted with pages (`[a,b] has_next`, `[] has_next`, error on page 2), asserting key progression, empty-page error on page 2, retry semantics, and refresh resetting the key.
