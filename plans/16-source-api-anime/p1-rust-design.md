# 16-source-api-anime: Rust design

Scope reminder: the anime source API (`AnimeSource`, `AnimeHttpSource`, `Video`, `Hoster`, filters, season/episode fetch types) plus the manga-side files that share the same package. This scope has no screens. The UI rules that depend on it live in the player and entry-detail plans (area:36, area:39).

Repo-relative Kotlin paths are under `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/` unless stated.

---

## 1. Where it plugs into the existing mockup

Current layout (`/home/blackbox/kurobayomi`):
- `Cargo.toml`: `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde`, `serde_json`.
- `src/main.rs` declares `mod clamp; mod entry; mod player; mod screens; mod settings; mod spec; mod ui;`.
- `src/player.rs` builds the player and reader widgets from literal strings (`player(title, episode)`, `reader(title, chapter)`). These become data-driven once the loader in section 5 exists.
- `src/spec.rs` reads `spec/*.json` (extracted from Aniyomi UI) into typed `Spec`, `Screen`, `Sheet`, `Dialog`, `Item`.
- `src/entry.rs`, `src/screens.rs`, `src/settings.rs`: UI only.

Plan:
- Add `src/source/` (sections 2-4) and `src/player/loader.rs` (section 5). Add `mod source; mod player_loader;` to `src/main.rs`. No changes to `spec.rs` in this area.
- `spec/*.json` is a UI description, not a source of data. Source data is never read from `spec/`.

Proposed `Cargo.toml` additions (crate versions to be pinned at implementation time to versions at least two weeks old):

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "io-util", "fs"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "gzip", "brotli", "deflate", "cookies", "stream"] }
async-trait = "0.1"
url = "2"
md-5 = "0.10"            # md5 for source id; or the md5 crate
scraper = "0.20"         # CSS selectors over HTML (html5ever); replaces Jsoup
regex = "1"              # no lookaround: see section 3.4
thiserror = "1"
tracing = "0.1"
async-channel = "2"
bytes = "1"
axum = { version = "0.7", default-features = false, features = ["tokio", "http1"] }   # local stream server
```

Pick `md-5` or `md5` (either). Do not add a second JSON library; `serde_json` already exists.

---

## 2. Module layout

```
src/source/
  mod.rs          re-exports; SourceError; SourceId
  model.rs        SAnime, SEpisode, SManga, SChapter, AnimesPage, MangasPage, AnimeRelation,
                  UpdateStrategy, FetchType, AnimeStatus, MangaStatus, UpdateResults
  filter.rs       Filter / FilterList (anime and manga share one generic type)
  video.rs        Video, VideoState, Track, TimeStamp, ChapterType, Headers (ordered)
  hoster.rs       Hoster, HosterState, NO_HOSTER_LIST
  thumbnail.rs    ThumbnailInfo, TileInfo
  traits.rs       AnimeSource, AnimeCatalogueSource, MangaSource, CatalogueSource,
                  ConfigurableSource, ResolvableSource, UnmeteredSource, UriType
  http.rs         HttpSourceBase: headers, GET helper, generate_id, url without domain
  scrape.rs       Parsed source helpers (scraper), selectText/selectInt/attrOrText
  prefs.rs        source preference namespace, SourcePref descriptors
  codec.rs        serde codecs: SerializableVideo / SerializableHoster (P2), defaultJson parity
  http_server.rs  LocalStreamServer + LocalHandler (placeholder rewrite target)
  torrent.rs      TorrentInfo, TorrentFile, magnet builder, TorrServer client (P2)
  registry.rs     builtin source registry (no APK loading)
src/player_loader.rs    EpisodeLoader + HosterLoader equivalents (hoster discovery, selection, resolution)
```

The GTK code never imports `reqwest` or `scraper` directly; only `src/source/` does.

---

## 3. Core types

### 3.1 Errors

```rust
// src/source/mod.rs
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Non-2xx HTTP status. Mirrors okhttp3 HttpException(code).
    #[error("HTTP {status}")]
    Http { status: u16, url: String },
    /// Transport failure (connect, TLS, timeout, body read). Mirrors IOException.
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    /// HTML/JSON/bencode parse failure or missing required element.
    #[error("parse: {0}")]
    Parse(String),
    /// Source does not implement this capability (Kotlin: UnsupportedOperationException / "Not used").
    #[error("not supported: {0}")]
    NotSupported(&'static str),
    /// Invalid URL built from base + path (Kotlin: IllegalArgumentException from toHttpUrl).
    #[error("bad url: {0}")]
    BadUrl(String),
    /// Local stream server could not bind or start.
    #[error("local server: {0}")]
    LocalServer(String),
    /// Cancelled by the caller (Kotlin CancellationException). Never converted to None.
    #[error("cancelled")]
    Cancelled,
}
```

Rules:
- `Cancelled` is propagated, never swallowed. The Kotlin code rethrows `CancellationException` in `getResolvedVideo` and `loadHosterVideos`.
- Every other error in `resolve_video`, `getImageTile`-style optional calls maps to `None` plus a `tracing::warn!`, matching the Kotlin `try/catch`.

### 3.2 Models

```rust
// src/source/model.rs
#[derive(Debug, Clone, PartialEq)]
pub struct SAnime {
    pub url: String,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub background_url: Option<String>,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub status: AnimeStatus,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub update_strategy: UpdateStrategy,
    pub fetch_type: FetchType,
    pub season_number: f64,          // default -1.0
    pub initialized: bool,           // default false
    pub memo: serde_json::Map<String, serde_json::Value>,
}

impl Default for SAnime {
    fn default() -> Self {
        SAnime {
            url: String::new(), title: String::new(), thumbnail_url: None, background_url: None,
            artist: None, author: None, status: AnimeStatus::Unknown, description: None, genre: None,
            update_strategy: UpdateStrategy::AlwaysUpdate, fetch_type: FetchType::Episodes,
            season_number: -1.0, initialized: false, memo: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimeStatus { Unknown, Ongoing, Completed, Licensed, PublishingFinished, Cancelled, OnHiatus, Upcoming, Other(i32) }
impl AnimeStatus {
    pub fn from_i32(v: i32) -> Self { match v { 0 => Self::Unknown, 1 => Self::Ongoing, 2 => Self::Completed,
        3 => Self::Licensed, 4 => Self::PublishingFinished, 5 => Self::Cancelled, 6 => Self::OnHiatus,
        7 => Self::Upcoming, other => Self::Other(other) } }
    pub fn to_i32(self) -> i32 {
        match self { Self::Unknown => 0, Self::Ongoing => 1, Self::Completed => 2, Self::Licensed => 3,
            Self::PublishingFinished => 4, Self::Cancelled => 5, Self::OnHiatus => 6, Self::Upcoming => 7,
            Self::Other(x) => x }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateStrategy { #[default] AlwaysUpdate = 0, OnlyFetchOnce = 1 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FetchType { Seasons = 0, #[default] Episodes = 1 }

#[derive(Debug, Clone, PartialEq)]
pub struct SEpisode {
    pub url: String,
    pub name: String,
    pub episode_number: f32,         // default -1.0
    pub fillermark: bool,            // default false
    pub scanlator: Option<String>,
    pub date_upload: i64,            // default 0, epoch milliseconds (area:07)
    pub summary: Option<String>,
    pub preview_url: Option<String>,
    pub memo: serde_json::Map<String, serde_json::Value>,
}

pub struct AnimesPage { pub animes: Vec<SAnime>, pub has_next_page: bool }
pub struct AnimeRelation { pub name: String, pub animes: Vec<SAnime> }
pub struct SAnimeEpisodeUpdate { pub anime: SAnime, pub episodes: Vec<SEpisode> }
pub struct SAnimeSeasonUpdate { pub anime: SAnime, pub seasons: Vec<SAnime> }
```

Notes:
- `SAnime.status` is `AnimeStatus`, not `i32`, but the integer is preserved for unknown values (`Other`). The DB column stays `INTEGER`.
- `date_upload` is `i64` in Rust. The Kotlin type is `Long`; no value above `i64::MAX` is possible.
- `episode_number` stays `f32` to match Kotlin `Float`. Persisted as REAL (f64) with exact round-trip (`f32 as f64 as f32`).

### 3.3 Filters

```rust
// src/source/filter.rs
#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Header { name: String },
    Separator { name: String },                       // name default ""
    Select { name: String, values: Vec<String>, index: usize },   // index default 0
    Text { name: String, value: String },             // default ""
    CheckBox { name: String, checked: bool },         // default false
    TriState { name: String, state: TriStateValue },  // default Ignore
    Group { name: String, children: Vec<Filter> },
    Sort { name: String, values: Vec<String>, selection: Option<SortSelection> }, // default None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TriStateValue { #[default] Ignore = 0, Include = 1, Exclude = 2 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSelection { pub index: usize, pub ascending: bool }

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterList(pub Vec<Filter>);     // equality is value equality (not always false)
```

Rules:
- `Select.values` in Kotlin is `Array<V>` with generic `V`. For Rust, the common instantiations are `String` (display label) and typed values converted to labels; keep `Vec<String>` and store the index. Where a source needs a typed value, it maps `index -> value` in its own code.
- `Select.index` must be `< values.len()`. Constructors validate and clamp to 0 when out of range and log a warning. Kotlin does not validate (E-12 family edge), Rust fixes it deliberately.
- `Header`/`Separator` are separate variants. `Filter::eq` is derived, so `Header{name:"A"}` never equals `Select{name:"A"}`. This deliberately differs from the Kotlin `equals` (edge E-13).
- The filter list is `Clone` and cheap to pass by `Arc<FilterList>` between UI and source tasks.

### 3.4 Video and hoster

```rust
// src/source/video.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoState { #[default] Queue, LoadVideo, Ready, Error }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track { pub url: String, pub lang: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChapterType { Opening, Ending, Recap, MixedOp, #[default] Other }

#[derive(Debug, Clone, PartialEq)]
pub struct TimeStamp { pub start: f64, pub end: f64, pub name: String, pub kind: ChapterType }

/// Ordered, duplicate-allowing header list. Kotlin okhttp3.Headers semantics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Headers(pub Vec<(String, String)>);

#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    pub video_url: String,                         // "" default; "null" = legacy marker (never produced by Rust sources)
    pub video_title: String,                       // "" default
    pub resolution: Option<i32>,
    pub bitrate: Option<i32>,
    pub headers: Option<Headers>,                  // None = use source headers
    pub preferred: bool,                           // false
    pub subtitle_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    pub timestamps: Vec<TimeStamp>,
    pub mpv_args: Vec<(String, String)>,
    pub ffmpeg_stream_args: Vec<(String, String)>,
    pub ffmpeg_video_args: Vec<(String, String)>,
    pub internal_data: String,                     // ""
    pub initialized: bool,                         // false
    pub memo: serde_json::Map<String, serde_json::Value>,
    /// Runtime only; not part of equality or persistence.
    pub state: VideoState,
}
```

`Video::eq` must compare every persisted field and ignore `state`. Implement `PartialEq` by hand, or put `state` in a wrapper. Choose the wrapper: `struct VideoEntry { video: Video, state: VideoState }` in the loader, so `Video` stays a pure value. That matches Kotlin behaviour exactly (status is a body property).

```rust
// src/source/hoster.rs
pub const NO_HOSTER_LIST: &str = "no_hoster_list";

#[derive(Debug, Clone, PartialEq)]
pub struct Hoster {
    pub hoster_url: String,                 // ""
    pub hoster_name: String,                // ""
    pub video_list: Option<Vec<Video>>,     // None = fetch later
    pub internal_data: String,              // ""
    pub lazy: bool,                         // false
    pub memo: serde_json::Map<String, serde_json::Value>,
}

impl Hoster {
    /// Kotlin List<Video>.toHosterList()
    pub fn synthetic(videos: Vec<Video>) -> Self {
        Hoster { hoster_url: String::new(), hoster_name: NO_HOSTER_LIST.into(),
                 video_list: Some(videos), internal_data: String::new(), lazy: false, memo: Default::default() }
    }
}

/// Runtime state per hoster (Kotlin HosterState in the player package).
#[derive(Debug, Clone, PartialEq)]
pub enum HosterState {
    Idle { hoster_name: String },
    Loading { hoster_name: String },
    Ready { hoster_name: String, videos: Vec<VideoEntry>, },
    Error { hoster_name: String },
}
pub struct VideoEntry { pub video: Video, pub state: VideoState }
```

Note: the Kotlin `Hoster.State {IDLE, LOADING, READY, ERROR}` is not persisted and is replaced here by the `HosterState` enum the loader uses.

### 3.5 Thumbnails

```rust
// src/source/thumbnail.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileInfo { pub image_index: usize, pub time_ms: i64, pub x: u32, pub y: u32, pub width: u32, pub height: u32 }
#[derive(Debug, Clone, PartialEq)]
pub struct ThumbnailInfo { pub tile_info: Vec<TileInfo>, pub image_tile_urls: Vec<String> }
```

Kotlin `Int` for `imageIndex`, `x`, `y`, `width`, `height`. Rust uses `usize`/`u32` for values that are never negative in practice. A source returning a negative value fails `serde`/conversion and the thumbnail set is discarded (log, no crash). `time_ms` is `i64` (Kotlin `Long`).

### 3.6 Traits

```rust
// src/source/traits.rs
use async_trait::async_trait;

pub struct SourceId(pub i64);

/// Mirrors AnimeSource. Every method is async so network-backed sources never block GTK.
#[async_trait]
pub trait AnimeSource: Send + Sync {
    fn id(&self) -> i64;
    fn name(&self) -> &str;
    fn lang(&self) -> &str { "" }
    fn supports_latest(&self) -> bool;
    fn supports_details(&self) -> bool { true }        // E-27: false = never call the details path
    fn supports_episode_list(&self) -> bool { true }   // E-28
    fn filter_list(&self) -> FilterList { FilterList::default() }

    /// Display: "Name (LANG)" (AnimeHttpSource.toString).
    fn display_name(&self) -> String { format!("{} ({})", self.name(), self.lang().to_uppercase()) }

    async fn popular(&self, page: i32) -> Result<AnimesPage, SourceError>;
    async fn latest(&self, page: i32) -> Result<AnimesPage, SourceError>;
    async fn search(&self, page: i32, query: &str, filters: &FilterList) -> Result<AnimesPage, SourceError>;

    async fn anime_episode_update(&self, anime: SAnime, episodes: Vec<SEpisode>,
        fetch_details: bool, fetch_episodes: bool) -> Result<SAnimeEpisodeUpdate, SourceError>;
    async fn anime_season_update(&self, anime: SAnime, seasons: Vec<SAnime>,
        fetch_details: bool, fetch_seasons: bool) -> Result<SAnimeSeasonUpdate, SourceError>;

    fn supports_related_anime(&self) -> bool { false }
    async fn related_anime(&self, _anime: &SAnime) -> Result<Vec<AnimeRelation>, SourceError> {
        Err(SourceError::NotSupported("related_anime"))
    }

    /// True when this source implements hoster lists (replaces EpisodeLoader.checkHasHosters).
    fn provides_hosters(&self) -> bool;
    async fn hosters(&self, episode: &SEpisode) -> Result<Vec<Hoster>, SourceError> {
        Err(SourceError::NotSupported("hosters"))
    }
    async fn videos_for_hoster(&self, hoster: &Hoster) -> Result<Vec<Video>, SourceError> {
        Err(SourceError::NotSupported("videos_for_hoster"))
    }
    /// Legacy path: videos directly for an episode (no hoster level).
    async fn videos_for_episode(&self, _episode: &SEpisode) -> Result<Vec<Video>, SourceError> {
        Err(SourceError::NotSupported("videos_for_episode"))
    }

    fn sort_hosters(&self, hosters: Vec<Hoster>) -> Vec<Hoster> { hosters }
    fn sort_videos(&self, videos: Vec<Video>) -> Vec<Video> { videos }

    /// Lazy resolution (AnimeHttpSource.resolveVideo). Ok(None) = could not resolve.
    async fn resolve_video(&self, video: Video) -> Result<Option<Video>, SourceError> { Ok(Some(video)) }

    async fn video_thumbnails(&self, _video: &Video) -> Result<Option<ThumbnailInfo>, SourceError> { Ok(None) }
    async fn image_tile(&self, _url: &str) -> Result<Option<bytes::Bytes>, SourceError> { Ok(None) }

    /// Replaces createHttpServer(): the source's own local request handler. None = not needed.
    fn local_handler(&self) -> Option<Arc<dyn LocalHandler>> { None }
}

/// Kotlin HttpServer subclass contract: the source answers requests on the local port.
#[async_trait]
pub trait LocalHandler: Send + Sync {
    /// `path` is the request path with query (e.g. "/seg/1.ts?x=1"), `range` the raw Range header if present.
    async fn serve(&self, path: &str, range: Option<&str>) -> Result<LocalResponse, SourceError>;
}

pub struct LocalResponse {
    pub status: u16,                       // 200, 206, 404
    pub headers: Vec<(String, String)>,    // Content-Type, Content-Length, Content-Range, Accept-Ranges
    pub body: bytes::Bytes,
}
```

Design decisions:
- `hosters` and `videos_for_hoster` have NO default that calls themselves. Defaults return `NotSupported`. This removes the recursion defect in `AnimeCatalogueSource.kt:65-67` (E-02).
- `provides_hosters` is explicit. The reflection-based `checkHasHosters` is replaced by a declared capability (E-01 fixed by design).
- `supports_latest` is required (Kotlin had no default on `AnimeSource`).
- `anime_episode_update` takes owned values because the caller moves them into a task.

```rust
pub trait ConfigurableSource: AnimeSource {
    /// Replaces setupPreferenceScreen(PreferenceScreen). Declarative descriptors, rendered by the UI.
    fn preferences(&self) -> Vec<SourcePref>;
}

pub enum SourcePref {
    Switch   { key: String, title: String, summary: Option<String>, default: bool },
    List     { key: String, title: String, summary: Option<String>, entries: Vec<String>, entry_values: Vec<String>, default: String },
    MultiSelect { key: String, title: String, summary: Option<String>, entries: Vec<String>, entry_values: Vec<String>, default: Vec<String> },
    EditText { key: String, title: String, summary: Option<String>, default: String },
    Category { title: String, children: Vec<SourcePref> },
}

#[async_trait]
pub trait ResolvableAnimeSource: AnimeSource {
    fn uri_type(&self, uri: &str) -> UriType;
    async fn anime_for_uri(&self, uri: &str) -> Result<Option<SAnime>, SourceError>;
    async fn episode_for_uri(&self, uri: &str) -> Result<Option<SEpisode>, SourceError>;
}
pub enum UriType { Anime, Episode, Unknown }

/// Marker: exempt from per-source queue warnings and concurrency limits in the scheduler.
pub trait UnmeteredSource {}
```

Manga traits (`MangaSource`, `CatalogueSource`, `ResolvableSource`) are defined in the same file with the same shape (`get_manga_details`, `get_chapter_list`, `get_page_list`, `popular_manga`, `search_manga`, `latest_manga`, `filter_list`). Owned by area:15 for their semantics; the trait skeletons are created in this area's task T-16a-042.

### 3.7 Concrete base struct for HTTP sources

Kotlin `AnimeHttpSource` is an abstract class with defaults. Rust has no inheritance, so the defaults live in a helper that each HTTP source embeds:

```rust
// src/source/http.rs
pub struct HttpBase {
    pub base_url: String,        // no trailing slash
    pub version_id: i32,         // default 1
    pub name: String,
    pub lang: String,
    pub client: reqwest::Client, // from the network stack (area:12)
    pub user_agent: String,      // from settings default_user_agent, trimmed
    headers: std::sync::OnceLock<Headers>, // lazy, like Kotlin `by lazy`
}

impl HttpBase {
    pub fn id(&self) -> i64 { generate_id(&self.name, &self.lang, self.version_id) }

    /// headersBuilder(): exactly one header, User-Agent. Computed once.
    pub fn headers(&self) -> &Headers {
        self.headers.get_or_init(|| Headers(vec![("User-Agent".to_string(), self.user_agent.clone())]))
    }

    /// Kotlin GET(url, headers): caller passes the headers; Cache-Control max-age=600 is always added
    /// (Requests.kt DEFAULT_CACHE_CONTROL). Ordered duplicates are preserved by RequestBuilder::header.
    pub fn get_request(&self, url: &str, headers: &Headers) -> Result<reqwest::RequestBuilder, SourceError> {
        let parsed = url::Url::parse(url).map_err(|e| SourceError::BadUrl(format!("{url}: {e}")))?;
        let mut rb = self.client.get(parsed).header("Cache-Control", "max-age=600");
        for (k, v) in &headers.0 {
            rb = rb.header(k.as_str(), v.as_str());
        }
        Ok(rb)
    }

    /// Mirrors Call.awaitSuccess(): non-2xx => SourceError::Http. Dropping the future cancels the request.
    pub async fn fetch(&self, rb: reqwest::RequestBuilder) -> Result<reqwest::Response, SourceError> {
        let resp = rb.send().await?;
        if !resp.status().is_success() {
            return Err(SourceError::Http { status: resp.status().as_u16(), url: resp.url().to_string() });
        }
        Ok(resp)
    }

    /// Body as text plus the final URL after redirects (Jsoup base URI, asJsoup()).
    pub async fn fetch_text(&self, rb: reqwest::RequestBuilder) -> Result<(String, url::Url), SourceError> {
        let resp = self.fetch(rb).await?;
        let final_url = resp.url().clone();
        Ok((resp.text().await?, final_url))
    }
}

/// Kotlin HttpSource.kt id generator, byte-exact.
pub fn generate_id(name: &str, lang: &str, version_id: i32) -> i64 {
    use md5::{Digest, Md5};
    let key = format!("{}/{}/{}", name.to_lowercase(), lang, version_id);
    let digest = Md5::digest(key.as_bytes());
    let mut v: i64 = 0;
    for i in 0..8 { v |= (digest[i] as i64) << (8 * (7 - i)); }
    v & i64::MAX
}
```

`generate_id` gets its own unit test with the two vectors from behavior 1.1 (`4851548778462321757`, `7915262807009061958`).

Important: `(digest[i] as i64) << 56` for `i = 0` sets bit 63 for bytes >= 0x80, which the `& i64::MAX` clears. This is the same as Kotlin.

URL helper (Kotlin `getUrlWithoutDomain`, decoded components, see E-06):

```rust
// Signature. Algorithm: p1-topic-url-and-id-compat.md section 2.3 (steps 1 to 8).
// Java URI strictness: reject-and-return-original; path, query and fragment are percent-decoded.
pub fn url_without_domain(orig: &str) -> String;
```

The exact decoding rules are in `p1-topic-url-and-id-compat.md`.

### 3.8 Parsed (scraper) sources

Kotlin `ParsedAnimeHttpSource` (abstract, Jsoup) becomes a helper that takes selector strings from configuration:

```rust
pub struct ListingSelectors {
    pub item: String,              // Jsoup selector of each item
    pub next_page: Option<String>, // None = never has next page
}
pub fn parse_listing<F>(doc: &scraper::Html, sel: &ListingSelectors, mut from_el: F) -> Result<AnimesPage, SourceError>
where F: FnMut(scraper::ElementRef) -> Result<SAnime, SourceError> {
    let item = scraper::Selector::parse(&sel.item).map_err(|e| SourceError::Parse(format!("{e:?}")))?;
    let animes = doc.select(&item).map(&mut from_el).collect::<Result<Vec<_>, _>>()?;
    let has_next_page = match &sel.next_page {
        None => false,
        Some(s) => {
            let next = scraper::Selector::parse(s).map_err(|e| SourceError::Parse(format!("{e:?}")))?;
            doc.select(&next).next().is_some()
        }
    };
    Ok(AnimesPage { animes, has_next_page })
}
```

Limit of `scraper` versus Jsoup: no `:contains()`, no `:matches()`, no `:has()` on older versions; see E-05. Proposal: a source that needs `:contains` writes a filter over the element list in code.

### 3.9 Local stream server

```rust
// src/source/http_server.rs
pub struct LocalStreamServer { port: u16, shutdown: tokio::sync::oneshot::Sender<()> }

// Signatures (behavior specified in the bullets below):
//   pub async fn start(handler: Arc<dyn LocalHandler>) -> Result<LocalStreamServer, SourceError>
//       binds 127.0.0.1:0 with axum; returns Err(LocalServer) if bind fails. Never returns port 0.
//   pub fn url(&self) -> String                  -> "http://localhost:<port>"
//   pub fn port(&self) -> u16
//   pub async fn stop(self)                      -> sends shutdown on the oneshot channel, awaits axum graceful shutdown
```

Behavior:
- Every request is delegated to `LocalHandler::serve(path, range)`. The handler decides which paths exist; an `Err` maps to HTTP 500 and `Ok` to the returned status (404 is the handler's choice).
- Forwards `Range` headers and returns 206 with correct `Content-Range`. mpv and ffmpeg seek with ranges.
- Adds the video's `Headers` to the upstream request.
- Lifetime: created when playback or download starts and dropped when it ends (replaces the foreground service in `HttpServerService`).

---

## 4. Video model algorithms in Rust

### 4.1 Placeholder detection

```rust
pub const PLACEHOLDER_PREFIX: &str = "http://localhost:1";

fn placeholder_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"http://localhost:1($|[^0-9])").unwrap())
}

pub fn is_placeholder(url: &str) -> bool { placeholder_re().is_match(url) }
```

### 4.2 `uses_http_server`

```rust
impl Video {
    pub fn uses_local_server(&self) -> bool {
        is_placeholder(&self.video_url)
            || self.audio_tracks.iter().any(|t| is_placeholder(&t.url))
            || self.subtitle_tracks.iter().any(|t| is_placeholder(&t.url))
    }
}
```

### 4.3 `copy_with_local_server(port)`

```rust
impl Video {
    pub fn rewrite_to_local(&self, port: u16) -> Result<Video, SourceError> {
        if port == 0 { return Err(SourceError::LocalServer("port 0".into())); }   // E-08
        let host = format!("http://localhost:{port}");
        let rewrite = |s: &str| placeholder_re().replace_all(s, |c: &regex::Captures| format!("{host}{}", &c[1])).into_owned();
        let mut v = self.clone();                            // copies every field including memo (E-07 fixed)
        v.video_url = rewrite(&self.video_url);
        for t in &mut v.subtitle_tracks { t.url = rewrite(&t.url); }
        for t in &mut v.audio_tracks { t.url = rewrite(&t.url); }
        Ok(v)
    }
}
```

Equivalence with Kotlin: `replace_all` rewrites every placeholder occurrence; `$1`-style capture keeps the trailing non-digit character (or nothing at end of string). The Kotlin version replaces the placeholder text `http://localhost:1` with the new host, leaving the rest of the URL untouched, which the capture reproduces.

Edge: the Kotlin `Regex.replace(input, replacement)` interprets `$` in the replacement. The port string has no `$`, so no difference.

### 4.4 Equality of Video

`Video` derives `PartialEq` and does not include `state` (the `VideoEntry` wrapper holds it). Kotlin `data class` equality compares the constructor properties only; this matches.

### 4.5 Serialization (P2 only)

Use serde with `#[serde(default)]` on every field except `internal_data` in `SerializableHoster`, and `#[serde(rename_all = "camelCase")]` to match the Kotlin JSON keys in data-and-config section 5. `Pair` serializes as a 2-element array; implement a `PairVec` newtype with a custom serializer. Reading ignores unknown keys (no `deny_unknown_fields`).

---

## 5. Player loader (`src/player_loader.rs`)

This is the Rust equivalent of `EpisodeLoader` and `HosterLoader`. It owns the algorithms in behavior sections 5 and 6, and is the only code that calls `hosters`, `videos_for_hoster`, `videos_for_episode`, `resolve_video`.

### 5.1 Hoster discovery

```rust
pub enum HosterSource { Downloaded, Http, Local }

/// HTTP branch of EpisodeLoader.getHostersOnHttp. Errors propagate to the caller.
pub async fn http_hosters(s: &Arc<dyn AnimeSource>, episode: &SEpisode) -> Result<Vec<Hoster>, SourceError> {
    if s.provides_hosters() {
        let hs = s.hosters(episode).await?;
        Ok(s.sort_hosters(hs))
    } else {
        let vs = s.videos_for_episode(episode).await?;
        Ok(vec![Hoster::synthetic(s.sort_videos(vs))])
    }
}

// Downloaded and local branches (EpisodeLoader.getHostersOnDownloaded / getHostersOnLocal) build a single
// Hoster via Hoster::synthetic(vec![video]); any error inside them yields an empty Vec, as in Kotlin.
```

Rule: `hosters()` errors propagate to the UI as an error for the episode (Kotlin: the exception propagates out of `getHosters`). Downloaded and local branches swallow errors and return an empty list, as the Kotlin does.

### 5.2 Loading one hoster

```rust
pub async fn load_hoster(s: &Arc<dyn AnimeSource>, h: &Hoster, force: bool) -> Result<HosterState, SourceError> {
    if !force && h.lazy {
        return Ok(HosterState::Idle { hoster_name: h.hoster_name.clone() });
    }
    match get_videos(s, h).await {
        Ok(videos) => Ok(HosterState::Ready {
            hoster_name: h.hoster_name.clone(),
            videos: videos.into_iter().map(|video| VideoEntry { video, state: VideoState::Queue }).collect(),
        }),
        Err(SourceError::Cancelled) => Err(SourceError::Cancelled),   // Kotlin rethrows CancellationException
        Err(_) => Ok(HosterState::Error { hoster_name: h.hoster_name.clone() }),
    }
}

async fn get_videos(s: &Arc<dyn AnimeSource>, h: &Hoster) -> Result<Vec<Video>, SourceError> {
    // Kotlin order: parseVideoUrls (legacy "null"), then sortVideos for HTTP sources.
    let videos = match &h.video_list {
        Some(list) => list.clone(),
        None => s.videos_for_hoster(h).await?,
    };
    Ok(s.sort_videos(videos))
}
```

Differences from Kotlin, deliberate:
- The Kotlin `parseVideoUrls` legacy step (`videoUrl == "null"` → `getVideoUrl`) is NOT ported. It exists only for ext-lib 15/16 APKs. Rust sources never produce `"null"`. (Recorded as a drop in T-16a-015.)
- Sorting is applied to both explicit and fetched lists for every source. Kotlin sorted only for `AnimeHttpSource`; in Rust all sources are `AnimeSource`, and the default `sort_videos` is identity, so the result is the same for sources that do not override it.

### 5.3 Selection

```rust
/// Kotlin HosterLoader.selectBestVideo, exact tiers.
pub fn select_best(hosters: &[HosterState]) -> Option<(usize, usize)> {
    let ready = || hosters.iter().enumerate().filter_map(|(i, h)| match h {
        HosterState::Ready { videos, .. } => Some((i, videos)), _ => None });
    let counts = |s: VideoState| matches!(s, VideoState::Ready | VideoState::Queue);
    // tier 1: preferred
    if let Some((hi, vs)) = ready().find(|(_, vs)| vs.iter().any(|e| e.video.preferred && counts(e.state))) {
        let vi = vs.iter().position(|e| e.video.preferred && counts(e.state)).unwrap();
        return Some((hi, vi));
    }
    // tier 2: first non-empty URL
    if let Some((hi, vs)) = ready().find(|(_, vs)| vs.iter().any(|e| !e.video.video_url.is_empty() && counts(e.state))) {
        let vi = vs.iter().position(|e| !e.video.video_url.is_empty() && counts(e.state)).unwrap();
        return Some((hi, vi));
    }
    None
}
```

Kotlin returns `Pair(-1, -1)` for none; Rust returns `None`. The tuple is `(hoster_index, video_index)`.

### 5.4 Best video with parallel preferred probing

```rust
pub type ProbeOut = (usize, HosterState, Option<Video>);

pub async fn best_video(s: Arc<dyn AnimeSource>, hosters: Vec<Hoster>) -> Result<Option<Video>, SourceError> {
    // Phase A: one task per hoster. Each loads its videos, then tries its first preferred,
    // not-yet-initialized video. The first task that resolves a non-empty URL wins.
    let mut set: tokio::task::JoinSet<Result<ProbeOut, SourceError>> = tokio::task::JoinSet::new();
    for (i, h) in hosters.iter().cloned().enumerate() {
        let s = s.clone();
        set.spawn(async move {
            let mut st = load_hoster(&s, &h, false).await?;
            let mut found = None;
            if let HosterState::Ready { videos, .. } = &mut st {
                if let Some(pi) = videos.iter().position(|e| e.video.preferred && !e.video.initialized) {
                    videos[pi].state = VideoState::LoadVideo;
                    let candidate = videos[pi].video.clone();
                    found = resolve_once(&s, candidate).await?;
                    if found.is_none() {
                        videos[pi].state = VideoState::Error;
                    }
                }
            }
            Ok((i, st, found))
        });
    }

    let mut states: Vec<Option<HosterState>> = vec![None; hosters.len()];
    while let Some(joined) = set.join_next().await {
        let (i, st, found) = joined.map_err(|e| SourceError::Parse(format!("probe task: {e}")))??;
        states[i] = Some(st);
        if let Some(v) = found {
            set.abort_all();                       // Kotlin cancelChildren()
            return Ok(Some(v));
        }
    }
    let mut states: Vec<HosterState> = states
        .into_iter()
        .zip(hosters.iter())
        .map(|(st, h)| st.unwrap_or_else(|| HosterState::Idle { hoster_name: h.hoster_name.clone() }))
        .collect();

    // Phase B: sequential fallback over select_best(). Each tried video moves to Error on failure.
    loop {
        let Some((hi, vi)) = select_best(&states) else { return Ok(None) };
        let candidate = match &mut states[hi] {
            HosterState::Ready { videos, .. } => {
                videos[vi].state = VideoState::LoadVideo;
                videos[vi].video.clone()
            }
            _ => return Ok(None),
        };
        match resolve_once(&s, candidate).await? {
            Some(v) => return Ok(Some(v)),
            None => {
                if let HosterState::Ready { videos, .. } = &mut states[hi] {
                    videos[vi].state = VideoState::Error;
                }
            }
        }
    }
}

### 5.5 Resolution

```rust
pub async fn resolve_once(s: &Arc<dyn AnimeSource>, v: Video) -> Result<Option<Video>, SourceError> {
    if v.initialized { return Ok(if v.video_url.is_empty() { None } else { Some(v) }); }
    let resolved = match s.resolve_video(v).await {
        Ok(r) => r,
        Err(SourceError::Cancelled) => return Err(SourceError::Cancelled),
        Err(e) => { tracing::warn!(?e, "resolve_video failed"); None }
    };
    Ok(resolved.map(|mut r| { r.initialized = true; r }).filter(|r| !r.video_url.is_empty()))
}
```

Kotlin subtlety preserved: for already-initialized videos the source is NOT asked again (`!video.initialized` guard). Note the Kotlin version marks `initialized = true` on the copy even for non-HTTP sources; Rust does the same by setting the flag after resolution, and for already-initialized input it returns unchanged.

---

## 6. Async model and threads

| Layer | Runs on | Blocking allowed |
|---|---|---|
| GTK main loop (`glib::MainContext::default()`) | UI thread | never |
| tokio multi-thread runtime (started once in `main`) | worker threads | no blocking I/O (use `tokio::fs`, `spawn_blocking` for decode) |
| Source calls (`AnimeSource` methods) | tokio tasks | no |
| Image decode (thumbnail bytes, pixbuf) | `spawn_blocking` | yes |
| SQLite (rusqlite) | dedicated writer thread (area:03/04) | yes |

Bridge: a task posts results to the UI with `glib::MainContext::channel` or `async_channel` consumed by `glib::spawn_future_local`:

```rust
let (tx, rx) = async_channel::bounded::<UiMsg>(16);
rt.spawn(async move { let r = source.popular(1).await; let _ = tx.send(UiMsg::Popular(r)).await; });
glib::spawn_future_local(async move { while let Ok(msg) = rx.recv().await { apply(msg); } });
```

Rules:
- Never call `block_on` from the GTK thread.
- Cancellation is `JoinHandle::abort()` (tokio). Requests in flight are dropped, which cancels the reqwest future.
- The `Arc<dyn AnimeSource>` is shared; sources hold `reqwest::Client` (already internally `Arc`).

Concurrency limits: the source layer does not limit concurrency; the network stack (area:12) rate limiter does, and it is bypassed for `UnmeteredSource`.

---

## 7. Android → Linux mapping (this scope)

| Android / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| APK-loaded `AnimeSource` (ext-lib classloader) | builtin Rust sources in `registry.rs`; third-party mechanism decided in area:20 | `AnimeSourceFactory` and `SourceFactory` dropped (T-16a-038) |
| `SharedPreferences` per source (`Context.MODE_PRIVATE`) | settings store row `source_<id>` (area:02) | keys unchanged |
| OkHttp `Call`, `Request`, `Response`, `Headers`, `CacheControl` | reqwest `Client`, `RequestBuilder`, `Response`, `Vec<(String,String)>`, `Cache-Control` header string | ordered headers as Vec |
| `Call.awaitSuccess()` | `HttpBase::fetch` (non-2xx → `SourceError::Http`) | |
| RxJava `Observable` bridge (`awaitSingle`) | native `async fn` | Rx dropped (T-16a-048) |
| Jsoup `Document`, `Element`, `select` | `scraper::Html`, `ElementRef`, `Selector::parse` | CSS subset differences: E-05 |
| `java.net.URI` (decoded path/query) | `url::Url` plus explicit percent-decoding to match Java | see topic doc |
| `MessageDigest("MD5")` | `md-5` crate | exact bytes |
| `NanoHTTPD` local server | `axum` on `127.0.0.1:0` | port 0 is an error (E-08) |
| `BitmapFactory.decodeStream` | bytes to `gdk::Texture::from_bytes` or `image` crate on `spawn_blocking` | |
| `kotlinx.serialization` `Json` | `serde_json` | `defaultJson` semantics in codec.rs |
| `FFmpegKit` (downloader, Android) | `std::process::Command` running `ffmpeg` / `ffprobe` (area:24) | args built as a Vec, no shell |
| Intent extras (`hostList` serialized) | in-process typed `Vec<Hoster>` | no JSON round trip |
| `ForegroundService` (`HttpServerService`) | owned `LocalStreamServer` handle tied to playback | no notification needed; GTK shows status |
| Android `Notification` channel `http_server_channel` | none (drop); optional status line in player | user loss: none, the notification is informational |
| `androidx.preference.PreferenceScreen` | `Vec<SourcePref>` descriptor rendered by GTK | androidx dropped |
| `Injekt` DI (`injectLazy`) | explicit constructor injection in `registry.rs` | |
| `logcat` | `tracing` with `tracing-subscriber` | levels: DEBUG→debug, ERROR→error |
| `Dispatchers.IO` / `supervisorScope` | tokio tasks; `JoinSet` | |

---

## 8. Pitfalls to avoid

1. Blocking `execute()` as in `getImageTile` (E-09). Use async `fetch`.
2. Using `i32` status with magic numbers. Use `AnimeStatus`.
3. Reflection-based capability checks (`checkHasHosters`). Use `provides_hosters()`.
4. Mutable shared state from parallel tasks (behavior 6.2). Use a collector.
5. Silent `port == 0` rewrites (E-08).
6. Treating `Cancelled` as a normal error.
7. Using `regex` with lookaround (unsupported). The placeholder pattern is written with a capture.
8. Forgetting the default `Cache-Control: max-age=600` on source requests; some sites cache-bust by query, others depend on it. Keep it.
9. Recomputing `headers` on every request (Kotlin computes once, lazily). Use `OnceLock`.
10. Using `f64` for `episode_number`. Kotlin is `Float`; keep `f32` in memory.
