# 20-extension-strategy-redesign: Rust design

## R1. Decision

Replace APK/dex/classloader extensions with three source kinds, in this order of delivery:

1. **Built-in Rust sources** (`kb-sources-builtin` crate, compiled in). Used for the sources the author wants to keep working without any loader. Full type safety, no sandbox needed.
2. **Declarative TOML scrapers** (`kb-source-scrape`). Non-programmers and porting of simple `ParsedHttpSource` sites. Operations are CSS selectors plus JSON paths, evaluated by the `scraper` crate. No code runs.
3. **WASM components** (`kb-ext-wasm`, wasmtime + WIT). The extension store format for everything else. Sandboxed, typed boundary, one artifact per platform.

Rejected for the primary path: JVM bridge (see p1-topic-option-matrix.md), Lua (weaker typing, no ecosystem), JS via rquickjs as the main language (kept only as an optional fallback behind the same WIT contract if needed later), sidecar processes (no isolation gain over WASM and worse distribution).

Assumption stated explicitly: the Aniyomi APK ecosystem is not reusable by Kurobayomi. The observed Aniyomi index holds 3 entries (checked 2026-10-08) and the upstream keiyoushi manga tree has 407 `en` source directories written against OkHttp, Jsoup, RxJava and Android APIs. Porting those is a per-source rewrite, so migration value is in the existing user library and backup ids (see p1-topic-source-id-and-backups.md), not in the extension binaries.

## R2. Crate and module layout

```
kurobayomi/
  crates/
    kb-source/            Source traits, models, filters, id derivation (no I/O)
      src/lib.rs
      src/manga.rs        MangaSource trait, SManga, SChapter, Page, MangasPage, UpdateStrategy
      src/anime.rs        AnimeSource trait, SAnime, SEpisode, Hoster, Video, TimeStamp, Track
      src/filter.rs       Filter enum, TriState, Sort, FilterList
      src/id.rs           source_id(name, lang, version_id)
      src/http.rs         HttpSourceBase helper (reqwest, headers, url normalisation)
      src/error.rs        SourceError
    kb-ext/               registry, loader, install, trust, repo fetch (depends on kb-source)
      src/registry.rs     ExtensionRegistry (installed/available/untrusted)
      src/loader.rs       scans <data>/extensions, selects, verifies, instantiates
      src/manifest.rs     manifest.toml parse and validation
      src/install.rs      download, checksum, atomic rename, InstallStep, queue
      src/repo.rs         index.min.json fetch and parse
      src/trust.rs        repo key set + per-version trust rows
      src/update.rs       update check, has_update, is_obsolete, throttle
      src/watch.rs        inotify on the extensions directory (notify crate)
    kb-ext-wasm/          wasmtime host, WIT bindings, capability limits
      wit/source.wit      the WIT world (see R6)
      src/host.rs
      src/limits.rs
    kb-source-scrape/     TOML scraper definition and evaluator
    kb-sources-builtin/   built-in Rust sources
    kb-torrent/           torrent backend client (P2)
  src/                    existing GTK application (mockup now, real app later)
    ui.rs, screens.rs, spec.rs, settings.rs, clamp.rs, entry.rs, player.rs, main.rs
  spec/                   spec/*.json extracted from Aniyomi UI (existing)
```

Crate names are proposals. Existing `src/` files stay where they are; the new crates are added to a workspace at the repository root, and `src/` becomes a workspace member.

## R3. Core types (kb-source)

```rust
// crates/kb-source/src/id.rs
use md5::{Digest, Md5};

/// Same derivation as HttpSource.generateId (HttpSource.kt) and
/// AnimeHttpSource.generateId (AnimeHttpSource.kt).
pub fn source_id(name: &str, lang: &str, version_id: i32) -> i64 {
    let key = format!("{}/{}/{}", name.to_lowercase(), lang, version_id);
    let digest = Md5::digest(key.as_bytes());
    let mut v: u64 = 0;
    for b in &digest[0..8] {
        v = (v << 8) | u64::from(*b);
    }
    (v & (i64::MAX as u64)) as i64
}
```

Note: `name.to_lowercase()` in Rust is Unicode-aware; Kotlin `lowercase()` is also Unicode-aware with the root locale. Test vectors in p1-edge-cases-and-tests.md must include a non-ASCII name.

```rust
// crates/kb-source/src/manga.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SManga {
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,          // joined with ", "
    pub status: MangaStatus,
    pub thumbnail_url: Option<String>,
    pub update_strategy: UpdateStrategy,
    pub initialized: bool,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MangaStatus {
    Unknown = 0,
    Ongoing = 1,
    Completed = 2,
    Licensed = 3,
    PublishingFinished = 4,
    Cancelled = 5,
    OnHiatus = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateStrategy { AlwaysUpdate, OnlyFetchOnce }

impl SManga {
    /// Mirrors SManga.getGenres(): split ", ", trim, drop blanks, dedupe keeping first occurrence.
    pub fn genres(&self) -> Option<Vec<String>> {
        let g = self.genre.as_deref()?.trim();
        if g.is_empty() { return None; }
        let mut out: Vec<String> = Vec::new();
        for part in g.split(", ") {
            let t = part.trim();
            if !t.is_empty() && !out.iter().any(|x| x == t) { out.push(t.to_string()); }
        }
        Some(out)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SChapter {
    pub url: String,
    pub name: String,
    pub date_upload_ms: i64,
    pub chapter_number: f32,
    pub scanlator: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MangasPage { pub mangas: Vec<SManga>, pub has_next_page: bool }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page { pub index: u32, pub url: String, pub image_url: Option<String> }
```

Note on `genre`: `SManga.getGenres` trims only the outer string on the blank check, then splits. The Rust version above trims the whole string before splitting, which yields identical output for all inputs because each part is trimmed again.

```rust
// crates/kb-source/src/manga.rs (trait)
#[async_trait::async_trait]
pub trait MangaSource: Send + Sync {
    fn id(&self) -> i64;
    fn name(&self) -> &str;
    fn lang(&self) -> &str;
    fn supports_latest(&self) -> bool;
    fn filters(&self) -> FilterList;

    async fn popular(&self, page: u32) -> Result<MangasPage, SourceError>;
    async fn latest(&self, page: u32) -> Result<MangasPage, SourceError>;
    async fn search(&self, page: u32, query: &str, filters: &FilterList) -> Result<MangasPage, SourceError>;
    async fn details(&self, manga: &SManga) -> Result<SManga, SourceError>;
    async fn chapters(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError>;
    async fn pages(&self, chapter: &SChapter) -> Result<Vec<Page>, SourceError>;
    /// Default: returns page.image_url or fetches page.url (ImageUrl resolution, HttpSource.getImageUrl).
    async fn image_url(&self, page: &Page) -> Result<String, SourceError>;
}

/// Optional capabilities, mirroring ResolvableSource.kt.
pub trait ResolvableMangaSource: MangaSource {
    fn uri_type(&self, uri: &str) -> UriType;
    async fn manga_from_uri(&self, uri: &str) -> Result<Option<SManga>, SourceError>;
    async fn chapter_from_uri(&self, uri: &str) -> Result<Option<SChapter>, SourceError>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UriType { Manga, Chapter, Unknown }
```

Note: `async fn` in traits is stable since Rust 1.75. Trait objects need `async_trait` (or `dyn`-safe boxed futures). The design uses `async_trait` for `dyn MangaSource` in the registry. Marker traits `Unmetered` and `Configurable` follow the same pattern.

```rust
// crates/kb-source/src/anime.rs
#[derive(Clone, Debug, PartialEq)]
pub struct SAnime {
    pub url: String, pub title: String,
    pub thumbnail_url: Option<String>, pub background_url: Option<String>,
    pub artist: Option<String>, pub author: Option<String>,
    pub status: AnimeStatus, pub description: Option<String>, pub genre: Option<String>,
    pub update_strategy: UpdateStrategy, pub fetch_type: FetchType,
    pub season_number: f64, pub initialized: bool,
    pub memo: serde_json::Value,
}
#[repr(i32)]
pub enum AnimeStatus { Unknown = 0, Ongoing = 1, Completed = 2, Licensed = 3,
    PublishingFinished = 4, Cancelled = 5, OnHiatus = 6, Upcoming = 7 }
pub enum FetchType { Seasons, Episodes }

#[derive(Clone, Debug, PartialEq)]
pub struct SEpisode {
    pub url: String, pub name: String, pub episode_number: f32, pub fillermark: bool,
    pub scanlator: Option<String>, pub date_upload_ms: i64, pub summary: Option<String>,
    pub preview_url: Option<String>, pub memo: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hoster {
    pub hoster_url: String, pub hoster_name: String,
    pub videos: Option<Vec<Video>>, pub internal_data: String,
    pub lazy: bool, pub memo: serde_json::Value,
}
pub const NO_HOSTER_LIST: &str = "no_hoster_list";

#[derive(Clone, Debug, PartialEq)]
pub struct Video {
    pub video_url: String, pub video_title: String,
    pub resolution: Option<i32>, pub bitrate: Option<i32>,
    pub headers: Vec<(String, String)>, pub preferred: bool,
    pub subtitle_tracks: Vec<Track>, pub audio_tracks: Vec<Track>,
    pub timestamps: Vec<TimeStamp>,
    pub mpv_args: Vec<(String, String)>,       // ffmpeg lists dropped, see R9
    pub internal_data: String, pub initialized: bool,
    pub memo: serde_json::Value,
}
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Track { pub url: String, pub lang: String }
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChapterType { Opening, Ending, Recap, MixedOp, Other }
#[derive(Clone, Debug, PartialEq)]
pub struct TimeStamp { pub start: f64, pub end: f64, pub name: String, pub kind: ChapterType }

#[async_trait::async_trait]
pub trait AnimeSource: Send + Sync {
    fn id(&self) -> i64;
    fn name(&self) -> &str;
    fn lang(&self) -> &str;
    fn supports_latest(&self) -> bool;
    fn filters(&self) -> FilterList;
    async fn popular(&self, page: u32) -> Result<AnimesPage, SourceError>;
    async fn latest(&self, page: u32) -> Result<AnimesPage, SourceError>;
    async fn search(&self, page: u32, query: &str, filters: &FilterList) -> Result<AnimesPage, SourceError>;
    async fn details(&self, anime: &SAnime) -> Result<SAnime, SourceError>;
    async fn episodes(&self, anime: &SAnime) -> Result<Vec<SEpisode>, SourceError>;
    async fn seasons(&self, anime: &SAnime) -> Result<Vec<SAnime>, SourceError>; // default Err(Unsupported)
    async fn hosters(&self, episode: &SEpisode) -> Result<Vec<Hoster>, SourceError>;
    async fn videos(&self, hoster: &Hoster) -> Result<Vec<Video>, SourceError>;
    async fn resolve_video(&self, video: Video) -> Result<Option<Video>, SourceError>; // default Ok(Some(video))
    fn sort_hosters(&self, hosters: Vec<Hoster>) -> Vec<Hoster> { hosters }
    fn sort_videos(&self, videos: Vec<Video>) -> Vec<Video> { videos }
    fn supports_related(&self) -> bool { false }
    async fn related(&self, anime: &SAnime) -> Result<Vec<AnimeRelation>, SourceError>;
    async fn episode_update(&self, anime: &SAnime, episodes: &[SEpisode], fetch_details: bool, fetch_episodes: bool)
        -> Result<AnimeEpisodeUpdate, SourceError>;
    async fn season_update(&self, anime: &SAnime, seasons: &[SAnime], fetch_details: bool, fetch_seasons: bool)
        -> Result<AnimeSeasonUpdate, SourceError>;
}

#[derive(Clone, Debug, PartialEq)] pub struct AnimeRelation { pub name: String, pub animes: Vec<SAnime> }
#[derive(Clone, Debug, PartialEq)] pub struct AnimesPage { pub animes: Vec<SAnime>, pub has_next_page: bool }
```

`AnimeEpisodeUpdate` and `AnimeSeasonUpdate` are the Rust forms of `SAnimeEpisodeUpdate` and `SAnimeSeasonUpdate` (both are small data classes in `source-api/.../model/`; their field lists must be copied from those files in T-20a-097 before coding, and the Kotlin remains the reference).

```rust
// crates/kb-source/src/filter.rs
pub enum TriState { Ignore = 0, Include = 1, Exclude = 2 }

pub enum Filter {
    Header { name: String },
    Separator { name: String },
    Select { name: String, values: Vec<String>, state: i32 },
    Text { name: String, state: String },
    CheckBox { name: String, state: bool },
    TriStateItem { name: String, state: TriState },
    Group { name: String, items: Vec<Filter> },
    Sort { name: String, values: Vec<String>, state: Option<SortSelection> },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortSelection { pub index: i32, pub ascending: bool }
pub struct FilterList(pub Vec<Filter>);
```

Equality semantics: Kotlin `Filter.equals` compares `name` and `state` only. Implement `PartialEq` manually with the same rule.

## R4. Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source is not installed")]              NotInstalled,
    #[error("operation not supported by this source")] Unsupported,
    #[error("network error: {0}")]                     Network(#[from] reqwest::Error),
    #[error("http status {0}")]                        Http(u16),
    #[error("parse error: {0}")]                       Parse(String),
    #[error("selector '{0}' matched nothing")]         MissingSelector(String),
    #[error("extension failed: {0}")]                  Extension(String),
    #[error("timed out after {0:?}")]                  Timeout(std::time::Duration),
}
```

Mapping: Kotlin `UnsupportedOperationException` -> `Unsupported`; `IllegalStateException("Not used")` (deprecated RxJava defaults) does not exist in Rust. Kotlin HTTP 4xx/5xx -> `Http(code)` via `awaitSuccess()` semantics (reqwest `error_for_status`).

## R5. Registry, loader and install (kb-ext)

```rust
pub struct ExtensionRegistry {
    installed: tokio::sync::watch::Sender<BTreeMap<String, InstalledExtension>>,
    available: tokio::sync::watch::Sender<BTreeMap<String, AvailableExtension>>,
    untrusted: tokio::sync::watch::Sender<BTreeMap<String, UntrustedExtension>>,
    sources: Arc<arc_swap::ArcSwap<SourceTable>>,   // id -> live Arc<dyn Source>
}

pub struct InstalledExtension {
    pub pkg: String, pub name: String,
    pub version_name: String, pub version_code: i64, pub api: ApiVersion,
    pub kind: ExtKind, pub lang: String, pub nsfw: bool,
    pub sources: Vec<SourceDescriptor>,
    pub has_update: bool, pub is_obsolete: bool,
    pub repo_url: Option<String>,
    pub origin: Origin,                 // Builtin | ScraperToml | Wasm
}
pub struct AvailableExtension {
    pub pkg: String, pub name: String, pub version_name: String, pub version_code: i64,
    pub api: ApiVersion, pub lang: String, pub nsfw: bool,
    pub sources: Vec<SourceDescriptor>, pub apk_name: String,    // "component" file name in the repo
    pub sha256: Option<String>, pub icon_url: String, pub repo_url: String,
}
pub struct UntrustedExtension { pub pkg: String, pub name: String, pub version_code: i64, pub key_id: String }

pub struct SourceDescriptor { pub id: i64, pub lang: String, pub name: String, pub base_url: String }
```

Loader pipeline (replaces B1.1):
```rust
pub enum LoadResult {
    Success(InstalledExtension),
    Untrusted(UntrustedExtension),
    Error { pkg: String, reason: LoadError },
}
pub async fn load_all(dir: &Path, trust: &TrustStore, ctx: &LoadContext) -> Vec<LoadResult>;
```
Steps per directory: parse `manifest.toml` -> validate `api` major/minor in supported set -> verify `component.sha256` equals sha256 of `component.wasm` -> trust check -> NSFW check (`ctx.show_nsfw`) -> instantiate (WASM) or build (scraper/builtin) -> derive `lang` from the set of source langs (same rule as B1.2) -> `Success`.

Concurrency: `futures::stream::iter(dirs).buffer_unordered(8)`. Instantiation of WASM components is CPU-bound and runs in `tokio::task::spawn_blocking`.

Install:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallStep { Idle, Pending, Downloading, Installing, Installed, Error }
impl InstallStep { pub fn is_completed(self) -> bool { matches!(self, Self::Idle | Self::Installed | Self::Error) } }

pub struct Installer { sem: tokio::sync::Semaphore, /* one permit: one install at a time */ }
pub async fn install(&self, ext: &AvailableExtension, progress: watch::Sender<InstallStep>) -> Result<(), InstallError>;
```
Flow: `Pending` (permit wait) -> `Downloading` (stream to `.tmp/<pkg>.<version>.part`) -> sha256 check if `ext.sha256` is `Some`, mismatch -> `Error` -> `Installing` (parse manifest, instantiate, check `version_code >= installed.version_code`, check `key_id` equals installed key_id) -> rename into `<pkg>/` -> `Installed`. Cancel: drop the future, delete the temp file, send `Idle`.

Watch: `notify::RecommendedWatcher` on the extensions root, 300 ms debounce (T-20a-040), then reload the affected `<pkg>/`.

## R6. WASM contract (kb-ext-wasm)

WIT sketch (the full file lives at `crates/kb-ext-wasm/wit/source.wit`; version `1.0.0`):

```wit
package kurobayomi:source@1.0.0;

interface types {
  record manga { url: string, title: string, artist: option<string>, author: option<string>,
    description: option<string>, genre: option<string>, status: u8,
    thumbnail-url: option<string>, update-strategy: u8, initialized: bool }
  record chapter { url: string, name: string, date-upload-ms: s64, chapter-number: f32,
    scanlator: option<string> }
  record page { index: u32, url: string, image-url: option<string> }
  record mangas-page { mangas: list<manga>, has-next-page: bool }
  variant source-error { network(string), http(u16), parse(string), extension(string) }
}

interface manga-source {
  use types.{manga, chapter, page, mangas-page, source-error};
  popular: func(page: u32) -> result<mangas-page, source-error>;
  latest: func(page: u32) -> result<mangas-page, source-error>;
  search: func(page: u32, query: string, filters: string) -> result<mangas-page, source-error>;
  details: func(m: manga) -> result<manga, source-error>;
  chapters: func(m: manga) -> result<list<chapter>, source-error>;
  pages: func(c: chapter) -> result<list<page>, source-error>;
  image-url: func(p: page) -> result<string, source-error>;
}

world manga-extension {
  import http;          // host-provided fetch, see R7
  import settings;      // per-source key/value store
  export manga-source;
}
```

Filters cross the boundary as a JSON string (`filters: string`), so the filter enum needs no WIT variant tree; the JSON schema is the serde form of `kb_source::filter::Filter`.

Anime world: same shape plus `hosters`, `videos`, `resolve-video`, `episodes`, `seasons`, `episode-update`, `season-update`, `related`. Video records carry `headers` as `list<tuple<string,string>>` and the `timestamps` list.

## R7. Host capabilities and limits

- `http` import: `fetch(request) -> result<response, source-error>`; the host applies the per-host rate limiter from area:12-network-stack, its user agent, a 30 s timeout (**Proposal**), and a 16 MiB response cap per call.
- No filesystem, no sockets, no clock beyond `now-ms()`, no environment. Settings only through `settings` import.
- Wasmtime config: `consume_fuel(true)` with a default budget of 10,000,000,000 fuel per call (**Proposal**, tune with a benchmark), `max_memory_size = 256 MiB`, epoch interruption at 5 s per call as a second guard.
- Instantiation time is bounded at 2 s; a timeout is `Error`.

## R8. Threading model

- GTK main loop owns widgets and reads `watch::Receiver` snapshots through `glib::MainContext::spawn_local`.
- Tokio multi-thread runtime (4 workers) runs sources, HTTP and the registry. Bridge: `async-channel` or `glib::MainContext::channel`.
- Never call a blocking source method on the GTK thread. WASM instantiate and sha256 run on `spawn_blocking`.
- Installation is serialised by the one-permit semaphore; repo fetch uses a `JoinSet` with the per-repo limit of 4 concurrent requests.

## R9. Anime playback details

- Player: libmpv via `libmpv2` (or `libmpv-sys` with a thin wrapper). `Video.video_url` goes to `loadfile`; `headers` become `http-header-fields`; subtitle tracks are added with `sub-add`, audio with `audio-add`.
- `ffmpegStreamArgs` and `ffmpegVideoArgs` are dropped. User loses: source-specified ffmpeg stream tweaks.
- Thumbnails and seek preview: tile sheet decoded by `image` crate, cropped per `TileInfo`, displayed in GTK. Libmpv has no tile preview API.
- `createHttpServer` is dropped. User loses: sources that serve local data through a NanoHTTPD proxy. Replacement is libmpv's own HTTP and `http-header-fields`.

## R10. Mapping of Android APIs to Linux (for this area)

| Android / Kotlin | Linux / Rust replacement |
|---|---|
| `PackageManager.getInstalledPackages`, `getPackageArchiveInfo` | directory scan of `<data>/extensions` + manifest parse |
| `ChildFirstPathClassLoader`, `Class.forName` | wasmtime component instantiate (`wasmtime::component::Component`) |
| `Signature`, `SigningInfo`, `Hash.sha256` | `sha2::Sha256` over the component bytes; key id = sha256 of the public key |
| `DownloadManager` + 1 s polling | `reqwest` stream + `tokio::sync::watch` |
| `BroadcastReceiver` (package added/replaced/removed) | `notify` inotify watcher, debounced 300 ms |
| `Notification` + `NotificationCompat.BigTextStyle` | `notify-rust` (org.freedesktop.Notifications over D-Bus); KDE shows it natively |
| `PackageInstaller`, Shizuku, Install Activity | dropped (no APK) |
| `SharedPreferences` (`source_<id>`) | SQLite `source_settings` table (D9) |
| `Injekt` singletons | explicit constructor injection, `Arc<AppState>` |
| RxJava `Observable` and `awaitSingle` | `async fn` + `async_trait` |
| `Dispatchers.IO` | `tokio::task::spawn_blocking` for CPU work |
| `Context.getFilesDir()/cacheDir` | `$XDG_DATA_HOME/kurobayomi/`, `$XDG_CACHE_HOME/kurobayomi/` |
| `LocalBroadcastManager` (installer queue cancel) | `CancellationToken` (tokio-util) |
| `Locale.getDefault().language` | `LANG`/`LC_MESSAGES` parse (first two letters) |
| NanoHTTPD local server | dropped |

## R11. Pitfalls

- Source ids are signed 64-bit in the Kotlin code (`Long`). Serialize as strings in JSON and TOML; JavaScript-side tooling loses precision above 2^53.
- `name.to_lowercase()` vs Kotlin `lowercase()`: equal for the ASCII and common Unicode inputs in the tests. Include the Turkish dotted-I case in tests (Kotlin `lowercase()` with root locale maps `İ` to `i̇`, Rust maps the same). Verify once before release.
- `Video.url` and `Video.video_url` are different in Kotlin (`url` is the page URL). Keep both fields distinct in Rust, do not merge.
- `Hoster.lazy` hosters must not be fetched while building the episode list (performance and rate limit).
- The WASM sandbox has no DNS of its own; an extension that needs a host it does not declare in its manifest `allowed_hosts` list gets `network` error (**Proposal**, new manifest field).
