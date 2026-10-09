# Area 15 — Manga source API: Rust design (PROPOSAL)

Everything in this document is a proposal unless it says OBSERVED. Behaviour references point to `p1-behavior.md` (section numbers) and `p1-topic-*.md`. The loading mechanism for third-party sources is decided by area:20 and area:18; this document fixes the trait contract that any mechanism must present.

## 1. Crate and module layout

Workspace (the existing package `kurobayomi` at `/home/blackbox/kurobayomi` becomes the GTK app; the source API moves into two library crates next to it):

```
/home/blackbox/kurobayomi
  Cargo.toml                       (workspace root; package kurobayomi = app)
  crates/
    kuro-source/                   no GTK, no tokio runtime dependency (only async-trait + futures-core)
      src/lib.rs
      src/error.rs                 SourceError
      src/id.rs                    generate_id
      src/json.rs                  default_json_options()
      src/model/mod.rs
      src/model/manga.rs           SManga, MangaStatus, genres()
      src/model/chapter.rs         SChapter, copy_from
      src/model/page.rs            Page, PageState, PageStatus (watch handles)
      src/model/mangas_page.rs     MangasPage
      src/model/update_strategy.rs UpdateStrategy
      src/filter.rs                Filter, TriStateState, SortSelection
      src/filter_list.rs           FilterList
      src/source.rs                MangaSource, CatalogueSource, ConfigurableSource,
                                   ResolvableSource, UnmeteredSource, SourceFactory
      src/resolve.rs               UriType
      src/prefs.rs                 SourceSetting, SourcePrefs trait
    kuro-source-http/              reqwest + scraper + url; the HttpSource layer
      src/lib.rs
      src/client.rs                HttpClient (reqwest wrapper, cookies, timeouts)
      src/request.rs               SourceRequest, GET helper, DEFAULT_CACHE_MAX_AGE
      src/response.rs              HttpResponse (final URL, status, body), ensure_success, html(), json()
      src/http_source.rs           trait HttpSource + HttpCore (id, headers, client)
      src/parsed.rs                trait ParsedHttpSource (selector hooks, hasNextPage)
      src/html.rs                  jsoup-compatible text, select_text, select_int, attr_or_text
      src/url.rs                   get_url_without_domain, set_url_without_domain
      src/image.rs                 image fetch with progress, ImageFetcher
      src/data_saver.rs            DataSaver trait + 3 implementations (behaviour topic file)
  src/                              (GTK app, existing; see section 9)
```

Rationale: `kuro-source` must be usable by a future extension loader without pulling GTK or reqwest (area:20 may compile a source to WASM); `kuro-source-http` is the only crate that knows HTTP and HTML.

## 2. Dependencies (and why)

| Crate | Version policy | Replaces (Android/Kotlin) | Used in |
|---|---|---|---|
| `async-trait` | current stable | Kotlin `suspend fun` in interfaces | traits used as `dyn` |
| `thiserror` | current stable | exception classes | `SourceError` |
| `serde`, `serde_json` | current stable | kotlinx.serialization | `Page` cache, prefs JSON |
| `md-5` | current stable (RustCrypto) | `MessageDigest.getInstance("MD5")` | `generate_id`, cache keys |
| `url` | current stable | `java.net.URI`, OkHttp `HttpUrl` | URL join, `setUrlWithoutDomain` |
| `form_urlencoded` | current stable | `java.net.URLEncoder` (data saver) | BANDWIDTH_HERO `url=` parameter |
| `reqwest` (`rustls-tls`, `gzip`, `brotli`, `deflate`, `cookies`, `stream`) | current stable | OkHttp | `kuro-source-http::client` |
| `bytes` | current stable | `okio.Buffer` | response bodies |
| `scraper` | current stable (html5ever + selectors) | Jsoup | `kuro-source-http::html` |
| `encoding_rs` | current stable | OkHttp/Java charset decoding | `HttpResponse::text()` |
| `tokio` (`rt-multi-thread`, `sync`, `time`, `fs`) | current stable | Kotlin coroutines (`Dispatchers.IO`) | runtime for all async source work |
| `futures-util` | current stable | `Flow`, `flatMapMerge` | `buffer_unordered(2)` in downloads |
| `tokio-util` | current stable | `Job` cancellation | `CancellationToken` for reader loader |
| `tracing` | current stable | `logcat` | all logs |
| `wiremock` (dev-dependency) | current stable | (no Android equivalent; tests) | pipeline tests |

Pinned versions are chosen at implementation time with the repository's Cargo.lock; no version is pinned in this plan.

Not used: `rxjava`, `injekt` (replaced by explicit constructor injection in `kuro-app`), `nanohttpd`, `kotlinx-coroutines` (tokio instead), `compose` (no UI in this crate).

## 3. Error model

```rust
// crates/kuro-source/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Non-2xx response. Display: "HTTP error {status}" (same text as HttpException).
    #[error("HTTP error {0}")]
    Http(u16),
    /// Connection, TLS, timeout or body read failure. Message is the transport error text.
    #[error("{0}")]
    Transport(String),
    /// HTML or JSON structure did not match what the source expects.
    #[error("parse error: {0}")]
    Parse(String),
    /// A URL could not be built or parsed.
    #[error("invalid URL: {0}")]
    Url(String),
    /// Page has no image URL when the image was requested (Kotlin: NullPointerException on `imageUrl!!`).
    #[error("page has no image URL")]
    MissingImageUrl,
    /// Source returned an empty page list (downloader raises page_list_empty_error).
    #[error("page list is empty")]
    EmptyPageList,
    /// The operation is not implemented by this source (for example a source without search).
    #[error("not supported: {0}")]
    Unsupported(&'static str),
    /// The caller cancelled the operation.
    #[error("cancelled")]
    Cancelled,
}
```

Rules:
- Parsers return `Result<_, SourceError>`; no panics on malformed HTML. (Kotlin throws; behaviour section 3.3 explains that the callers then mark the page as error. The Rust callers match the same outcome by mapping every `Err` to the page error state, except `Cancelled`.)
- `Http(status)` is produced only by `HttpResponse::ensure_success` (never by parsers).
- A user-visible message is produced by the UI from the variant (area:51 keys), not by `Display`.

## 4. Models

```rust
// crates/kuro-source/src/model/manga.rs
pub const UNKNOWN: i32 = 0;
pub const ONGOING: i32 = 1;
pub const COMPLETED: i32 = 2;
pub const LICENSED: i32 = 3;
pub const PUBLISHING_FINISHED: i32 = 4;
pub const CANCELLED: i32 = 5;
pub const ON_HIATUS: i32 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SManga {
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,        // ", "-joined, parsed by genres()
    pub status: i32,                  // raw, round-trips unknown values
    pub thumbnail_url: Option<String>,
    pub update_strategy: UpdateStrategy,
    pub initialized: bool,
}

impl SManga {
    /// Behaviour 10: identical to SManga.getGenres().
    pub fn genres(&self) -> Option<Vec<String>>;
    /// Display category for the status value; any unknown value maps to Unknown (behaviour 12).
    pub fn status_kind(&self) -> MangaStatus;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MangaStatus { Unknown, Ongoing, Completed, Licensed, PublishingFinished, Cancelled, OnHiatus }
```

Implementation notes:
- `SManga::new(url, title)` sets the defaults from SMangaImpl (`status = 0`, `update_strategy = AlwaysUpdate`, `initialized = false`, all optionals `None`).
- `Clone` is the Kotlin `copy()`.
- `status_kind` does not expose `Other(i32)`; the raw value is kept in `status` for storage.

```rust
// crates/kuro-source/src/model/chapter.rs
#[derive(Debug, Clone, PartialEq)]
pub struct SChapter {
    pub url: String,
    pub name: String,
    pub date_upload: i64,       // epoch ms, 0 = unknown
    pub chapter_number: f32,    // -1.0 = unknown
    pub scanlator: Option<String>,
}
impl SChapter {
    pub fn new(url: impl Into<String>, name: impl Into<String>) -> Self; // date_upload 0, chapter_number -1.0, scanlator None
    pub fn copy_from(&mut self, other: &SChapter);                      // name, url, date_upload, chapter_number, scanlator
}
```

```rust
// crates/kuro-source/src/model/page.rs
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PageState { Queue, LoadPage, DownloadImage, Ready, Error }

/// Live state of one page, observable by the reader and the download UI (Kotlin StateFlow).
pub struct PageStatus {
    pub state: watch::Sender<PageState>,   // initial Queue
    pub progress: watch::Sender<i32>,      // initial 0; -1 = unknown total
}

pub struct Page {
    pub index: i32,                    // assigned by the app (position), never trusted from source
    pub url: String,                   // default ""
    pub image_url: Option<String>,
    pub local_path: Option<std::path::PathBuf>, // replaces android.net.Uri (not serialized)
    pub status: PageStatus,
}

impl Page {
    pub fn new(index: i32, url: impl Into<String>) -> Self;
    pub fn number(&self) -> i32 { self.index + 1 }
    /// Behaviour 5.3 / Page.update: contentLength > 0 -> 100 * read / total (integer), else -1.
    pub fn update_progress(&self, bytes_read: u64, content_length: Option<u64>) -> i32;
}

/// Cache representation (behaviour 1.3 / data topic): only these three fields are persisted.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PageCacheEntry {
    pub index: i32,
    #[serde(default)]
    pub url: String,
    #[serde(rename = "imageUrl", default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}
```

Notes:
- `update_progress` with `content_length == Some(0)` or `None` returns `-1`; it does not use `done`.
- The reader and downloader write through `status.state.send_replace(...)` so late subscribers see the latest value (watch semantics match StateFlow).
- `Page` is not `Clone` (it owns watch senders). Copies for the cache are made with `PageCacheEntry::from(&Page)`.

```rust
// crates/kuro-source/src/model/mangas_page.rs
pub struct MangasPage { pub mangas: Vec<SManga>, pub has_next_page: bool }

// crates/kuro-source/src/model/update_strategy.rs
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateStrategy { #[default] AlwaysUpdate = 0, OnlyFetchOnce = 1 }
impl From<i64> for UpdateStrategy { /* unknown -> AlwaysUpdate */ }
impl From<UpdateStrategy> for i64 { /* ordinal */ }
```

## 5. Filters

```rust
// crates/kuro-source/src/filter.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TriStateState { #[default] Ignore = 0, Include = 1, Exclude = 2 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSelection { pub index: i32, pub ascending: bool }

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Header { name: String },
    Separator { name: String },
    Select { name: String, values: Vec<String>, state: i32 },
    Text { name: String, state: String },
    CheckBox { name: String, state: bool },
    TriState { name: String, state: TriStateState },
    Group { name: String, state: Vec<Filter> },
    Sort { name: String, values: Vec<String>, state: Option<SortSelection> },
}

impl Filter {
    pub fn name(&self) -> &str;
    /// Kotlin Filter.equals: same name and same state (state compared structurally).
    /// Derived PartialEq compares all fields, which is identical for these variants because `name` is a field.
}
```

Important: the Kotlin `Select.state` is an Int index with no bounds check. The Rust `Select::state` stays `i32` (so that an out-of-range value from a source can be represented); every reader must go through `Filter::select_value(&self) -> Option<&str>` which returns `None` for out-of-range (behaviour topic filter section; the UI must not crash).

`TriStateState` is an enum in Rust; the raw Kotlin value 3 or -1 cannot be constructed from a source. Kotlin throws `IllegalStateException` for unknown raw values; Rust maps unknown raw values from `from_raw(i32) -> Result<TriStateState, SourceError::Parse>`.

```rust
// crates/kuro-source/src/filter_list.rs
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterList(pub Vec<Filter>);
impl std::ops::Deref for FilterList { type Target = [Filter]; fn deref(&self) -> &[Filter] { &self.0 } }
```

Kotlin `FilterList.equals` always returns `false` (FilterList.kt:10-12). Not ported: equality is structural. Compose-specific recomposition behaviour is not needed in GTK; the UI recomputes the filter sheet explicitly.

## 6. Source traits

```rust
// crates/kuro-source/src/source.rs
use async_trait::async_trait;
use crate::{error::SourceError, filter_list::FilterList, filter::*, model::*, resolve::UriType, prefs::SourceSetting};

#[async_trait]
pub trait MangaSource: Send + Sync {
    /// Stable identity; see behaviour 1.1.
    fn id(&self) -> i64;
    fn name(&self) -> &str;
    /// ISO 639-1 lowercase, or "all"/"other" (reserved). Default "" (MangaSource.lang).
    fn lang(&self) -> &str { "" }
    /// Kotlin getMangaDetails: returns the updated copy; `initialized` set by the implementation.
    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError>;
    /// Source order is preserved; the app assigns sourceOrder = position after distinct-by-url.
    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError>;
    /// Returned Page.index values are ignored by the app.
    async fn get_page_list(&self, chapter: &SChapter) -> Result<Vec<Page>, SourceError>;
    /// Behaviour 17: marks self-hosted sources excluded from queue warnings.
    fn is_unmetered(&self) -> bool { false }
}

#[async_trait]
pub trait CatalogueSource: MangaSource {
    /// Kotlin supportsLatest: required, no default (every source states it).
    fn supports_latest(&self) -> bool;
    /// Kotlin getFilterList(): a fresh list each call.
    fn filter_list(&self) -> FilterList;
    async fn get_popular_manga(&self, page: i32) -> Result<MangasPage, SourceError>;
    async fn get_search_manga(&self, page: i32, query: &str, filters: &FilterList) -> Result<MangasPage, SourceError>;
    async fn get_latest_updates(&self, page: i32) -> Result<MangasPage, SourceError>;
}

pub trait ConfigurableSource: MangaSource {
    /// Replaces Kotlin setupPreferenceScreen(screen). Called when the settings screen opens.
    fn setup_settings(&self) -> Vec<SourceSetting>;
}

#[async_trait]
pub trait ResolvableSource: MangaSource {
    fn uri_type(&self, uri: &str) -> UriType;
    async fn manga_from_uri(&self, uri: &str) -> Result<Option<SManga>, SourceError>;
    async fn chapter_from_uri(&self, uri: &str) -> Result<Option<SChapter>, SourceError>;
}

/// One extension class may expose several sources (behaviour section 0, SourceFactory).
pub trait SourceFactory: Send + Sync {
    fn create_sources(&self) -> Vec<std::sync::Arc<dyn CatalogueSource>>;
}
```

Notes:
- `Arc<dyn CatalogueSource>` is used everywhere so one source object can serve the browse screen, the reader and downloads concurrently. Trait upcasting (stable since Rust 1.86) converts an `Arc<dyn CatalogueSource>` to `Arc<dyn MangaSource>` where needed.
- `ConfigurableSource` and `ResolvableSource` are separate traits, not methods on `MangaSource`. The registry (area:17) holds optional handles per source: `struct SourceHandle { source: Arc<dyn CatalogueSource>, configurable: Option<Arc<dyn ConfigurableSource>>, resolvable: Option<Arc<dyn ResolvableSource>> }`. A source that implements a capability trait is registered with the handle set to `Some`. This replaces Kotlin `is ConfigurableSource` checks.
- `get_manga_details` takes `&SManga`; Kotlin takes the object by value and returns a new one. The Rust version returns a new value and never mutates the input (behaviour 2.2 notes the Kotlin callers do not rely on in-place mutation).
- `is_unmetered` replaces the marker interface `UnmeteredSource` (no methods) with a method, because trait objects cannot be tested with `is`.
- Deprecated Rx `fetch*` methods are not present (dropped, `p1-tasks.md` T-15a-047).
- `chapterPageParse` (declared abstract by Kotlin `HttpSource`) has no caller; it is not present in Rust (T-15a-047).

```rust
// crates/kuro-source/src/resolve.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UriType { Manga, Chapter, Unknown }
```

### 6.1 Settings schema (`crates/kuro-source/src/prefs.rs`)

Kotlin lets a source add any Android preference widget. Rust restricts the schema to the widgets the UI renders (see `p1-ui-parity.md`):

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum SourceSetting {
    /// Switch / SwitchPreference. Stored as bool.
    Switch { key: String, title: String, summary: Option<String>, default: bool },
    /// EditTextPreference. Stored as string. Empty string allowed.
    Text { key: String, title: String, summary: Option<String>, default: String, password: bool },
    /// ListPreference. `values[i]` is stored; `entries[i]` is shown.
    List { key: String, title: String, summary: Option<String>, entries: Vec<String>, values: Vec<String>, default: String },
    /// MultiSelectListPreference. Stored as a set of values.
    MultiSelect { key: String, title: String, summary: Option<String>, entries: Vec<String>, values: Vec<String>, default: Vec<String> },
    /// Read-only text (PreferenceCategory header or informational preference).
    Info { title: String, summary: Option<String> },
    /// Any other widget the extension declared. Shown as Info with the title; value not stored.
    Unsupported { key: Option<String>, title: String },
}

/// Key/value store for one source. Namespace = "source_<id>" (behaviour 15, preferenceKey()).
pub trait SourcePrefs: Send + Sync {
    fn get_bool(&self, key: &str) -> Option<bool>;
    fn set_bool(&self, key: &str, value: bool);
    fn get_string(&self, key: &str) -> Option<String>;
    fn set_string(&self, key: &str, value: String);
    fn get_string_set(&self, key: &str) -> Option<Vec<String>>;
    fn set_string_set(&self, key: &str, value: Vec<String>);
    fn remove(&self, key: &str);
    /// All entries, for backup (behaviour 15, PreferenceBackupCreator).
    fn all(&self) -> Vec<(String, PrefValue)>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrefValue { Bool(bool), Int(i32), Long(i64), Float(f32), String(String), StringSet(Vec<String>) }

pub fn preference_key(source_id: i64) -> String { format!("source_{source_id}") }
```

Behaviour: `get_*` returns `None` when absent; the UI then uses the schema default. Writes are synchronous from the UI thread (SQLite on the writer connection; area:03/area:06 handle the pool). `all()` skips `Unsupported` keys.

## 7. Source preference storage (proposal)

Two candidates; this plan chooses option A.

- Option A (chosen): SQLite table in the app database, owned by area:02/area:03:
  ```sql
  CREATE TABLE source_prefs (
      source_id INTEGER NOT NULL,
      key       TEXT    NOT NULL,
      type      TEXT    NOT NULL CHECK (type IN ('bool','int','long','float','string','string_set')),
      value     TEXT    NOT NULL,   -- JSON: true/false, number, string, or array of strings
      PRIMARY KEY (source_id, key)
  );
  ```
  Reason: one file per source (Option B) makes backups and restores write many files and complicates atomic restore. The table keeps backup/restore in one transaction (area:22).
- Option B: one JSON file `source_<id>.json` per source under the config directory.

Key names and the `source_<id>` namespace are kept so that backup entries keep the same `sourceKey` string.

## 8. HTTP layer (`kuro-source-http`)

### 8.1 Request and response

```rust
// crates/kuro-source-http/src/request.rs
pub const DEFAULT_CACHE_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(10 * 60); // Requests.kt:12

#[derive(Debug, Clone)]
pub struct SourceRequest {
    pub method: http::Method,           // GET for every built-in default
    pub url: url::Url,
    pub headers: http::HeaderMap,
    pub body: Option<bytes::Bytes>,
    pub cache_max_age: Option<std::time::Duration>, // None = no cache directive
}

/// Kotlin GET(url, headers, cache = DEFAULT_CACHE_CONTROL).
pub fn get(url: url::Url, headers: http::HeaderMap) -> SourceRequest;
```

```rust
// crates/kuro-source-http/src/response.rs
#[derive(Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub url: url::Url,          // final URL after redirects (Kotlin response.request.url)
    pub headers: http::HeaderMap,
    pub body: bytes::Bytes,     // fully read (asJsoup reads the whole body)
}

impl HttpResponse {
    /// Kotlin asObservableSuccess / awaitSuccess: 2xx passes, otherwise Err(Http(status)).
    pub fn ensure_success(self) -> Result<Self, SourceError>;
    /// Kotlin Response.asJsoup(html = null): decode, parse with base URI = self.url.
    pub fn html(&self) -> scraper::Html;
    /// Decode with the charset from Content-Type, default UTF-8 (encoding_rs).
    pub fn text(&self) -> Result<String, SourceError>;
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, SourceError>;
}
```

`html()` returns an owned `scraper::Html`. `scraper::Html` is not `Send` in all versions; parsers call `html()` and finish all selection inside the same synchronous function (see section 10).

### 8.2 Client

```rust
// crates/kuro-source-http/src/client.rs
pub struct HttpClient {
    inner: reqwest::Client,          // shared; cookies, TLS, proxies set by area:12
}
impl HttpClient {
    /// Kotlin awaitSuccess path: full body in memory.
    pub async fn execute(&self, req: SourceRequest) -> Result<HttpResponse, SourceError>;
    /// Kotlin newCachelessCallWithProgress: no cache, 30 h call timeout, progress callback per chunk.
    pub async fn execute_streaming(
        &self,
        req: SourceRequest,
        on_progress: &mut (dyn FnMut(u64, Option<u64>, bool) + Send),
    ) -> Result<bytes::Bytes, SourceError>;
}
```

- `execute` maps `reqwest::Error` to `SourceError::Transport(e.to_string())` and non-2xx responses through `ensure_success`.
- `execute_streaming` sets a per-request timeout of `Duration::from_secs(30 * 60 * 60)` (30 hours, OkHttpExtensions.kt:121-123) and does not use the response cache. It calls `on_progress(bytes_read, content_length, done)` after each chunk; `done = true` on the last chunk.
- Progress mapping into `Page`: `page.update_progress(bytes_read, content_length)` (behaviour 5.3).
- The default `User-Agent` is set by the network layer when absent (behaviour 1.5); the source's own UA header is never replaced.

### 8.3 HttpSource trait

```rust
// crates/kuro-source-http/src/http_source.rs
use kuro_source::{error::SourceError, filter::*, filter_list::FilterList, model::*, prefs::SourceSetting};

/// State every HTTP source carries (Kotlin fields `network`, `id` lazy, `headers` lazy).
pub struct HttpCore {
    pub client: std::sync::Arc<HttpClient>,
    /// Returns the user's default User-Agent (area:12 preference `default_user_agent`, trimmed).
    pub user_agent: std::sync::Arc<dyn Fn() -> String + Send + Sync>,
    id: std::sync::OnceLock<i64>,
    headers: std::sync::OnceLock<http::HeaderMap>,
}

impl HttpCore {
    pub fn new(client: std::sync::Arc<HttpClient>, user_agent: std::sync::Arc<dyn Fn() -> String + Send + Sync>) -> Self;
    /// Behaviour 1.1: computed on first call from name, lang and version_id, then cached.
    pub fn id(&self, name: &str, lang: &str, version_id: i32) -> i64;
    /// Behaviour 1.5: computed on first call from `build()`, then cached.
    pub fn headers(&self, build: impl FnOnce() -> http::HeaderMap) -> &http::HeaderMap;
}

pub trait HttpSource: Send + Sync {
    fn core(&self) -> &HttpCore;
    fn name(&self) -> &str;
    fn lang(&self) -> &str;
    /// Behaviour 1.4 / HttpSource.kt:36-38. No trailing slash.
    fn base_url(&self) -> &str;
    /// Behaviour 1.4. Default 1.
    fn version_id(&self) -> i32 { 1 }

    /// Kotlin CatalogueSource.supportsLatest (required; no default in Kotlin).
    fn supports_latest(&self) -> bool;
    /// Kotlin HttpSource.getFilterList() = FilterList() (HttpSource.kt:460).
    fn filter_list(&self) -> FilterList { FilterList::default() }

    /// Behaviour 1.5 (HttpSource.kt:96-98): the User-Agent header from the preference, unless overridden.
    fn headers_builder(&self) -> http::HeaderMap {
        let mut h = http::HeaderMap::new();
        if let Ok(v) = http::HeaderValue::from_str(&(self.core().user_agent)()) {
            h.insert(http::header::USER_AGENT, v);
        }
        h
    }

    /// Settings declared by the source (Kotlin ConfigurableSource). Empty when the source has none.
    fn settings(&self) -> Vec<SourceSetting> { Vec::new() }
    /// True when the source is configurable (Kotlin `is ConfigurableSource`). Used by the registry.
    fn is_configurable(&self) -> bool { false }
    /// True when the source is unmetered (Kotlin `is UnmeteredSource`).
    fn is_unmetered(&self) -> bool { false }

    // --- request builders (defaults = Kotlin HttpSource defaults) ---
    fn popular_request(&self, page: i32) -> Result<SourceRequest, SourceError>;
    fn search_request(&self, page: i32, query: &str, filters: &FilterList) -> Result<SourceRequest, SourceError>;
    fn latest_request(&self, page: i32) -> Result<SourceRequest, SourceError>;
    /// GET(baseUrl + manga.url) (HttpSource.kt:233-235).
    fn details_request(&self, manga: &SManga) -> Result<SourceRequest, SourceError> { self.join_base(&manga.url) }
    /// GET(baseUrl + manga.url) (HttpSource.kt:271-273).
    fn chapter_list_request(&self, manga: &SManga) -> Result<SourceRequest, SourceError> { self.join_base(&manga.url) }
    /// GET(baseUrl + chapter.url) (HttpSource.kt:316-318).
    fn page_list_request(&self, chapter: &SChapter) -> Result<SourceRequest, SourceError> { self.join_base(&chapter.url) }
    /// GET(page.url) with page.url used as-is (HttpSource.kt:352-354).
    fn image_url_request(&self, page: &Page) -> Result<SourceRequest, SourceError> {
        let url = url::Url::parse(&page.url).map_err(|e| SourceError::Url(format!("{}: {e}", page.url)))?;
        Ok(get(url, self.core_headers()))
    }
    /// GET(page.imageUrl!!) (HttpSource.kt:381-383). A missing image URL is MissingImageUrl.
    fn image_request(&self, page: &Page) -> Result<SourceRequest, SourceError> {
        let raw = page.image_url.as_deref().ok_or(SourceError::MissingImageUrl)?;
        let url = url::Url::parse(raw).map_err(|e| SourceError::Url(format!("{raw}: {e}")))?;
        Ok(get(url, self.core_headers()))
    }

    // --- parsers (abstract in Kotlin) ---
    fn popular_parse(&self, resp: HttpResponse) -> Result<MangasPage, SourceError>;
    fn search_parse(&self, resp: HttpResponse) -> Result<MangasPage, SourceError>;
    fn latest_parse(&self, resp: HttpResponse) -> Result<MangasPage, SourceError>;
    fn details_parse(&self, resp: HttpResponse) -> Result<SManga, SourceError>;
    fn chapter_list_parse(&self, resp: HttpResponse) -> Result<Vec<SChapter>, SourceError>;
    fn page_list_parse(&self, resp: HttpResponse) -> Result<Vec<Page>, SourceError>;
    fn image_url_parse(&self, resp: HttpResponse) -> Result<String, SourceError>;

    /// Behaviour 8 (HttpSource.kt:455): called for every chapter on every sync.
    fn prepare_new_chapter(&self, _chapter: &mut SChapter, _manga: &SManga) {}

    /// Behaviour 7 (HttpSource.kt:433-435): the details request URL, as text.
    fn manga_web_url(&self, manga: &SManga) -> Result<String, SourceError> {
        Ok(self.details_request(manga)?.url.to_string())
    }
    /// Behaviour 7 (HttpSource.kt:444-446): the PAGE-LIST request URL, as text.
    fn chapter_web_url(&self, chapter: &SChapter) -> Result<String, SourceError> {
        Ok(self.page_list_request(chapter)?.url.to_string())
    }

    /// Joins `path` to `base_url()` with plain string concatenation (behaviour 2.2), then parses.
    /// Parse failure is SourceError::Url (Kotlin: IllegalArgumentException from toHttpUrl()).
    fn join_base(&self, path: &str) -> Result<SourceRequest, SourceError> {
        let raw = format!("{}{}", self.base_url(), path);
        let url = url::Url::parse(&raw).map_err(|e| SourceError::Url(format!("{raw}: {e}")))?;
        Ok(get(url, self.core_headers()))
    }
    fn core_headers(&self) -> http::HeaderMap { self.core().headers(|| self.headers_builder()).clone() }
}
```

All request builders return `Result<SourceRequest, SourceError>`; a URL that cannot be parsed is `SourceError::Url`, the counterpart of Kotlin's `IllegalArgumentException` from `toHttpUrl()`.

The generic adapter below gives every HTTP source the `MangaSource` and `CatalogueSource` implementations. It replaces the Kotlin inheritance of `HttpSource`:

```rust
pub struct Http<S: HttpSource>(pub S);

#[async_trait::async_trait]
impl<S: HttpSource + 'static> kuro_source::MangaSource for Http<S> {
    fn id(&self) -> i64 { self.0.core().id(self.0.name(), self.0.lang(), self.0.version_id()) }
    fn name(&self) -> &str { self.0.name() }
    fn lang(&self) -> &str { self.0.lang() }
    fn is_unmetered(&self) -> bool { self.0.is_unmetered() }
    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
        let resp = self.0.core().client.execute(self.0.details_request(manga)?).await?;
        let mut details = self.0.details_parse(resp)?;
        details.initialized = true;                  // HttpSource.kt:223
        Ok(details)
    }
    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
        let resp = self.0.core().client.execute(self.0.chapter_list_request(manga)?).await?;
        self.0.chapter_list_parse(resp)
    }
    async fn get_page_list(&self, chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
        let resp = self.0.core().client.execute(self.0.page_list_request(chapter)?).await?;
        let pages = self.0.page_list_parse(resp)?;
        // Behaviour 5.2: the index a source sets is discarded; the list position is the index.
        Ok(pages.into_iter().enumerate().map(|(i, p)| Page { index: i as i32, ..p }).collect())
    }
}

#[async_trait::async_trait]
impl<S: HttpSource + 'static> kuro_source::CatalogueSource for Http<S> {
    fn supports_latest(&self) -> bool { self.0.supports_latest() }
    fn filter_list(&self) -> FilterList { self.0.filter_list() }
    async fn get_popular_manga(&self, page: i32) -> Result<MangasPage, SourceError> {
        let resp = self.0.core().client.execute(self.0.popular_request(page)?).await?;
        self.0.popular_parse(resp)
    }
    async fn get_search_manga(&self, page: i32, query: &str, filters: &FilterList) -> Result<MangasPage, SourceError> {
        let resp = self.0.core().client.execute(self.0.search_request(page, query, filters)?).await?;
        self.0.search_parse(resp)
    }
    async fn get_latest_updates(&self, page: i32) -> Result<MangasPage, SourceError> {
        let resp = self.0.core().client.execute(self.0.latest_request(page)?).await?;
        self.0.latest_parse(resp)
    }
}

impl<S: HttpSource + 'static> kuro_source::ConfigurableSource for Http<S> {
    fn setup_settings(&self) -> Vec<SourceSetting> { self.0.settings() }
}
```

`Http<S>` implements `ConfigurableSource` for every HTTP source; the registry (area:17) stores `configurable: Some(..)` only when `S::is_configurable()` returns true, which is how the Kotlin `is ConfigurableSource` check is reproduced. `ResolvableSource` is implemented the same way for sources that need deep links (`uri_type`, `manga_from_uri`, `chapter_from_uri` as additional methods of `HttpSource` with the same delegation pattern; Kotlin has no default for these, so the Rust trait method is required only for sources that opt in).

Rationale for the adapter: Kotlin's abstract class gives every HTTP source the same `getMangaDetails`/`getPageList` code. A generic wrapper gives the same shared behaviour without inheritance, and keeps `kuro-source` free of HTTP.

### 8.4 ParsedHttpSource

```rust
// crates/kuro-source-http/src/parsed.rs
pub trait ParsedHttpSource: HttpSource {
    fn popular_selector(&self) -> &'static str;
    fn popular_from_element(&self, el: scraper::ElementRef) -> Result<SManga, SourceError>;
    fn popular_next_page_selector(&self) -> Option<&'static str>;
    fn search_selector(&self) -> &'static str;
    fn search_from_element(&self, el: scraper::ElementRef) -> Result<SManga, SourceError>;
    fn search_next_page_selector(&self) -> Option<&'static str>;
    fn latest_selector(&self) -> &'static str;
    fn latest_from_element(&self, el: scraper::ElementRef) -> Result<SManga, SourceError>;
    fn latest_next_page_selector(&self) -> Option<&'static str>;
    fn details_from_document(&self, doc: &scraper::Html) -> Result<SManga, SourceError>;
    fn chapter_selector(&self) -> &'static str;
    fn chapter_from_element(&self, el: scraper::ElementRef) -> Result<SChapter, SourceError>;
    fn page_list_from_document(&self, doc: &scraper::Html) -> Result<Vec<Page>, SourceError>;
    fn image_url_from_document(&self, doc: &scraper::Html) -> Result<String, SourceError>;
}

/// Behaviour 4.2 (list pages): select, map every element, hasNextPage = next selector matches.
pub fn parse_list(
    doc: &scraper::Html,
    item: &str,
    from_el: impl Fn(scraper::ElementRef) -> Result<SManga, SourceError>,
    next: Option<&str>,
) -> Result<MangasPage, SourceError>;
```

Key rules:
- Selectors are `&'static str` in the trait, matching Kotlin `abstract fun ...Selector(): String`. A selector that fails to compile returns `SourceError::Parse` (the Kotlin Jsoup throws `Selector.SelectorParseException`).
- `has_next_page = next.map(|s| doc.select(&Selector::parse(s)?).next().is_some()).unwrap_or(false)`. A `None` selector yields `false` (behaviour 4.2).
- Parsing happens entirely inside `parse_list` / the `*_parse` function, which is synchronous. No `.await` occurs while a `scraper::Html` is alive.

## 9. Integration with the existing Kurobayomi mockup

Existing files (OBSERVED, `/home/blackbox/kurobayomi`):
- `Cargo.toml`: single package `kurobayomi`, dependencies `gtk` (gtk4 0.11.5, feature v4_16), `serde`, `serde_json`. No tokio, no HTTP, no HTML.
- `src/spec.rs`: loads `spec/*.json` (`Spec::load()`), `Screen`, `Sheet`, `Dialog`, `Item` (all item kinds). Used for every screen's labels.
- `src/screens.rs`: hand-built screen bodies; `body(id)` matches screen ids (for example `"library"`, `"anime_library"`); `Source A` and `Source B` with counts at line ~359 are invented sample rows.
- `src/ui.rs`: generic widget builders and the fallback `ui::generic` for screens without a builder.
- `spec/entries.json`: screen `browse` (tabs, menu, dialogs `source_options`, `extension_trust`, `remove_private_extension`), screen `browse_source` with sheet `filter_sheet` whose items are of types `info`, `switch`, `tristate`, `text`, `list`, `sort`, `action` (the last one is the Group filter).
- `spec/settings.json`: settings screens (`settings_browse` exists).

Required changes (PROPOSAL, to be done by the tasks in `p1-tasks.md`):

| Mockup element | Change | Notes |
|---|---|---|
| `Cargo.toml` | Convert to a workspace; add `crates/kuro-source`, `crates/kuro-source-http`; add `tokio`, `async-trait`, `reqwest`, `scraper`, `url`, `md-5`, `form_urlencoded`, `encoding_rs`, `bytes`, `thiserror`, `futures-util`, `tokio-util`, `tracing` to the app or to the crates | Dependency list in section 2 |
| `spec/entries.json` `browse_source.sheets[filter_sheet]` | Items `info` must be split into `Header` (heading) and `Separator` (divider) kinds in the spec; `action` used for Group must carry `filter_kind: "group"` | The mockup's sample option lists (`Option 1`, `Option 2`, `Popularity`, `Latest`, `Title`) are spec samples; real values come from the source's `filter_list()` at runtime |
| `src/screens.rs` `body()` | Add a builder for `browse_source` that renders a `FilterList` (not the spec sample items) | Filter renderer contract in `p1-topic-filters.md` |
| `src/screens.rs` ~line 359 `Source A/Source B` | Replace with the registry (`SourceHandle` list from area:17) | Remove the sample rows when wiring; do not keep invented sources |
| Source options dialog (`source_options`) | Items `Pin`, `Disable`, `Exclude from data saver` (manga only) | `Exclude from data saver` writes `data_saver_excluded` (StringSet, behaviour 4.2 of the data file) |
| Source settings screen | New screen fed by `ConfigurableSource::setup_settings()` (`p1-ui-parity.md`) | Not in the mockup |
| `src/main.rs` | Create one tokio multi-thread runtime at start-up and keep its handle in the app state; create `Arc<HttpClient>` once | area:01 and area:12 own construction order |

Threading between GTK and tokio (PROPOSAL, applies to every async call from the UI):
- The GTK main thread never awaits network or parser work.
- Pattern: `let (tx, rx) = async_channel::bounded(1); runtime.spawn(async move { tx.send(source.get_popular_manga(page).await).await }); glib::MainContext::default().spawn_local(async move { match rx.recv().await { Ok(res) => update_widgets(res), Err(_) => {} } });`
- Widgets are touched only in `spawn_local` futures on the main context.
- Alternative for long-lived streams (page status, download progress): `tokio::sync::watch` receivers polled from `glib::timeout_future` or converted with `glib::MainContext::spawn_local` loop; never a blocking `recv`.
- `async_channel` is an added dependency (small, GTK ecosystem standard).

## 10. Concurrency model

| Work | Executor | Concurrency | Source |
|---|---|---|---|
| Catalogue calls (popular/search/latest) | tokio multi-thread runtime | one task per user action; cancelled when the browse screen changes listing (use `CancellationToken`) | behaviour 3.1 |
| Details, chapter list | tokio | one task per manga open; coalesced if the same manga is requested twice | behaviour 3.1 |
| Page list and image URL for reader | reader loader task (below) | serial | behaviour 18 |
| Image bytes for reader | same reader loader task | serial (1 at a time) | behaviour 18 |
| Downloads | download task per download, pages through `futures_util::StreamExt::buffer_unordered(2)` | 2 pages in flight | behaviour 18 |
| HTML parsing | synchronous inside the parser; `tokio::task::spawn_blocking` when the body is larger than 262 144 bytes | CPU bound | behaviour 18 proposal |
| Disk cache writes | `tokio::fs` or `spawn_blocking` | per item | area:34 |

Reader loader (PROPOSAL, replaces `HttpPageLoader` queue):
- Input: `mpsc::UnboundedSender<LoadRequest { page: Arc<PageHandle>, priority: u8, seq: u64 }>`.
- The loader task keeps a `BinaryHeap<LoadRequest>` ordered by `(priority desc, seq asc)` (behaviour 18: higher priority first, FIFO within priority).
- Priorities: preload 0, current page 1, user retry 2; preload size 4.
- Each popped request: if `status != Queue`, skip; run `resolve_image_url` (if needed, state LoadPage), then the cache check, then `execute_streaming` (state DownloadImage), then state Ready or Error. `CancellationError` leaves the state unchanged.
- The loader is cancelled through a `CancellationToken` on reader close (behaviour 18 recycle).

## 11. Data saver as an async trait

Behaviour topic file `p1-topic-http-source-pipeline.md` describes three modes. The trait:

```rust
// crates/kuro-source-http/src/data_saver.rs
#[async_trait::async_trait]
pub trait DataSaver: Send + Sync {
    /// Returns the URL to request instead of `image_url`. Identity when the mode does not apply.
    async fn compress(&self, image_url: &str) -> Result<String, SourceError>;
}
pub struct NoOp;                            // identity
pub struct BandwidthHero { /* server, ignore_jpg, ignore_gif, format_jpeg, quality, color_bw */ }
pub struct WsrvNl { /* ignore_jpg, ignore_gif, format_jpeg, quality */ }
pub struct ReSmushIt { /* client, ignore_jpg, ignore_gif, quality */ }
```

- Only `ReSmushIt` performs I/O inside `compress` (the Kotlin version calls `execute()` synchronously; the Rust version awaits).
- Application point: `image_request` is built from the compressed URL for the duration of one request; `Page.image_url` itself is never changed by the data saver (behaviour topic, DataSaver.kt:30-41 restores it in `finally`). In Rust, pass the compressed URL to `image_request` explicitly.

## 12. Android → Linux mapping (this scope)

| Android / Kotlin | Kurobayomi (Linux) | Notes |
|---|---|---|
| OkHttp `OkHttpClient`, `Request`, `Response` | `reqwest::Client`, `SourceRequest`, `HttpResponse` | cookies/cache/Cloudflare: area:12 |
| `Call.asObservableSuccess()` / `awaitSuccess()` | `HttpResponse::ensure_success()` | same status rule |
| `newCachelessCallWithProgress` | `HttpClient::execute_streaming` | no cache, 30 h timeout |
| `ProgressListener` (Page implements it) | `FnMut(u64, Option<u64>, bool)` closure, result mapped to `PageStatus.progress` | |
| Jsoup `Document`, `Element`, `select`, `text()`, `attr()` | `scraper::Html`, `scraper::ElementRef`, `Selector`, jsoup-compatible `html::text()`, `attr()` | see pitfalls |
| `Response.asJsoup()` | `HttpResponse::html()` | base URI = final URL |
| `kotlinx.serialization` `Json` (ignoreUnknownKeys, explicitNulls=false) | `serde_json` with `#[serde(default)]` and `skip_serializing_if` | behaviour data 4.3 |
| RxJava `Observable`, `awaitSingle()` | not used; async fns | Rx dropped |
| Kotlin `suspend` / `Dispatchers.IO` / `withIOContext` | `async fn` on tokio | |
| `MutableStateFlow` (Page.status, Page.progress) | `tokio::sync::watch::Sender` | latest value semantics |
| `PriorityBlockingQueue` (reader) | `BinaryHeap` in one task fed by `mpsc` | |
| `flatMapMerge(concurrency = 2)` | `buffer_unordered(2)` | |
| `Injekt.get<T>()` / `injectLazy` | constructor parameters (`Arc<HttpClient>`) | area:01 composition root |
| `Context.getSharedPreferences` (`source_<id>`) | `SourcePrefs` backed by SQLite `source_prefs` | section 7 |
| `android.net.Uri` (Page.uri) | `PathBuf` (`local_path`) | |
| `DiskLruCache` (chapter cache) | file store under `$XDG_CACHE_HOME/kurobayomi/chapter-cache` (area:34) | keys = md5 hex as in Kotlin |
| `Hash.md5` lowercase hex | `md5::compute(bytes)` formatted `{:x}` | lowercase, identical output |
| `java.net.URLEncoder.encode(s, "utf-8")` | `form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>()` | both produce `+` for space and keep `*-._` and alphanumerics |
| `java.net.URI` path/query/fragment | `url::Url` | decoded vs encoded difference (behaviour 6.2) |
| `logcat` | `tracing` macros | |
| `MR.strings.*`, `AYMR.strings.*` | Kurobayomi message catalogue keyed by the same names (area:51) | |
| `@Stable`, Compose `State` | none | not needed |
| `ConfigurableSource.setupPreferenceScreen(PreferenceScreen)` | `ConfigurableSource::setup_settings() -> Vec<SourceSetting>` | section 6.1 |
| `is ConfigurableSource` / `is UnmeteredSource` | `SourceHandle` capability fields / `is_unmetered()` | section 6 |

## 13. Pitfalls

1. CSS selector coverage: Jsoup accepts jsoup-only pseudo-classes that `scraper` (the `selectors` crate) does not support. Known Jsoup-only forms include `:contains(text)`, `:containsOwn(text)`, `:matches(regex)`, `:matchesOwn(regex)`, `:containsData(text)`, `:eq(n)`, `:gt(n)`, `:lt(n)`, `:has(selector)` (check the `scraper` version pinned for `:has` support). Rule: ports of existing extensions convert such selectors to standard CSS plus a Rust filter closure, and the conversion is covered by a fixture test. Do not implement a Jsoup-compatible selector engine.
2. Text normalisation: Jsoup `text()` collapses runs of whitespace into single spaces and trims the ends (Jsoup `StringUtil`; jsoup 1.22.2 pinned in `gradle/libs.versions.toml`, line 31). The Rust helper `html::text()` must collapse ASCII whitespace (space, tab, LF, FF, CR) AND U+00A0 (NBSP) and trim. Verify the NBSP behaviour against the pinned jsoup with a fixture before relying on it. `scraper`'s own `text()` does not normalise whitespace.
3. HTML tree builder differences: Jsoup and html5ever are both HTML5-oriented but differ in table foster-parenting and some malformed-markup recovery. Each ported source needs fixture tests with real saved pages; do not assume identical trees.
4. `selectInt` on non-numeric text: Kotlin `text().toInt()` throws `NumberFormatException` (the empty string throws too). Rust returns `SourceError::Parse`. Same outcome for callers.
5. `attrOrText(css)` takes the attribute of the element itself, not of a child selected by `css` (JsoupExtensions.kt:16-18). Port it as `attr_or_text(el, attr)` and never select children inside it.
6. `scraper::Html` and `ElementRef` borrow the document: keep them in synchronous functions; `Send` is not guaranteed across `.await`.
7. Redirect-final URL: `HttpResponse.url` must be the post-redirect URL. reqwest exposes it via `Response::url()`; take it before consuming the body.
8. `Url::join` vs Jsoup `abs:`: `abs:href` in Jsoup resolves against the base URI with its own rules; `Url::join` differs for some malformed inputs (for example `//host` without scheme handling). Test with fixtures.
9. `get_url_without_domain` decoding: Java decodes the path; Rust (`url`) keeps percent-encoding. Stored `url` values from Aniyomi may contain decoded characters; the migration must not rewrite them (behaviour 6.2).
10. Source id stability: `generate_id` must match Kotlin bit for bit, including the lowercase of `name` (Unicode). Tests: three vectors in behaviour 1.1.
11. `Arc<dyn CatalogueSource>` and trait upcasting: needs a Rust toolchain at least 1.86 (edition 2024 is already used by the mockup).
12. Blocking the GTK thread: parsing a 500 KB catalogue page inline takes measurable time; keep parsing off the main thread (section 10).
13. Mockup sample rows: `Source A`/`Source B` in `src/screens.rs` must not survive into the real browse list.
14. Data saver ReSmush: an unescaped `imageUrl` in the query string breaks for URLs containing `&`. Rust must percent-encode the image URL there, which deliberately differs from the Kotlin code (documented in the data file section 6.2). This is a bug fix, not a behaviour the users rely on.
