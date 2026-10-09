# area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin: Rust design

Builds on `plans/27-tracking-core/p1-rust-design.md` (traits, `TrackError`, `MsgKey`, `BaseTracker`, `SecretStore`, retry queue). Where this document changes a 27 signature, it says so in section 3.1. Source behaviour is in `p1-behavior.md`; data shapes are in `p1-data-and-config.md`.

---

## 1. Layout

```
crates/kuro-track/
  src/trackers/mod.rs                 registry: the four trackers, constructed from TrackerDeps
  src/trackers/enhanced.rs            EnhancedError, EntryRef, accept(), default is_track_from / migrate_track
  src/trackers/http.rs                shared reqwest client builder (system DNS, timeouts, UA header helper)
  src/trackers/sources.rs             SourceDirectory + SourcePrefs traits (installed-source lookup, source prefs)
  src/trackers/kavita/mod.rs          KavitaTracker (impl MangaTracker + EnhancedMangaTracker)
  src/trackers/kavita/api.rs          KavitaApi: series, volumes, latest, mark-until, authenticate
  src/trackers/kavita/auth.rs         KavitaAuth: 3-slot JWT table, single-flight load, invalidate on 401
  src/trackers/kavita/dto.rs          SeriesDto, VolumeDto, ChapterDto, AuthenticationDto (serde)
  src/trackers/komga/mod.rs           KomgaTracker
  src/trackers/komga/api.rs           KomgaApi: series/readlist read, v2 progress, PUT progress
  src/trackers/komga/dto.rs           SeriesDto, SeriesMetadataDto, BookMetadataAggregationDto, ReadProgress*, ReadList
  src/trackers/suwayomi/mod.rs        SuwayomiTracker
  src/trackers/suwayomi/api.rs        SuwayomiApi: full, chapters, PUT chapter
  src/trackers/suwayomi/dto.rs        MangaDataClass, ChapterDataClass, SourceDataClass
  src/trackers/jellyfin/mod.rs        JellyfinTracker (impl AnimeTracker + EnhancedAnimeTracker)
  src/trackers/jellyfin/api.rs        JellyfinApi: item, episodes, played-items POST
  src/trackers/jellyfin/auth.rs       JellyfinAuth: user id -> api key lookup over slots 1..10
  src/trackers/jellyfin/dto.rs        JFItem, JFUserData, JFItemList
  src/trackers/jellyfin/url.rs        TrackingUrl: parse /Users/{u}/Items/{i}#fragment (movie | seriesId,{s})

crates/kuro-sources/src/identity.rs   generate_source_id(name, lang, version) -> i64 (shared with area 15/16)
crates/kuro-track/tests/enhanced_*.rs fixture tests (section 9 of p1-edge-cases-and-tests.md)
```

Rule from 27: `kuro-track` must not depend on GTK, `gio`, or `libsecret`. The trackers depend on `SourceDirectory`, `SourcePrefs` and `SecretStore` traits, never on the concrete app.

---

## 2. Crates and why

| Crate | Used for | Replaces (Kotlin) | Notes |
|---|---|---|---|
| `reqwest` (`rustls-tls`, `json`, `gzip`) | HTTP | OkHttp + `awaitSuccess`/`parseAs` | Client built in `kuro-net` (area 12). Trackers hold a clone. |
| `tokio` (`rt-multi-thread`, `sync`, `time`, `macros`) | async, `RwLock`/`Mutex` for auth, `try_join!` | coroutines, `withIOContext` | |
| `futures` | `stream::iter(...).buffer_unordered(n)` for the Jellyfin marking | `async {}` per item | |
| `serde`, `serde_json` | DTOs | kotlinx.serialization | `ignoreUnknownKeys` and `explicitNulls=false` behaviour via `#[serde(default)]` on optional fields |
| `md-5` (crate name `md-5`, use path `md5`) | source id | `MessageDigest.getInstance("MD5")` | RustCrypto |
| `time` | Jellyfin `DatePlayed` (UTC) | `SimpleDateFormat` | Not `chrono` (27 rule) |
| `url` | URL parsing and building, path segment edits, fragments | `HttpUrl` (okhttp) | `path_segments_mut()` replaces `removePathSegment` |
| `base64` | not needed (reqwest `basic_auth` encodes) | `Credentials.basic` | See pitfall P-3 |
| `thiserror` | `EnhancedError`, `KavitaError`, `JellyfinError` | exceptions | |
| `tracing` | WARN/ERROR logs with the same wording | `logcat` | |
| `keyring` via `SecretStore` (27) | not used by enhanced trackers, only by Komga if the credential decision in section 10 picks a stored password | — | |

`async-trait` is already used by 27 for the `dyn` tracker traits; these trackers reuse it. `once_cell` is not used (use `std::sync::OnceLock`).

---

## 3. Key types and traits

### 3.1 Trait changes to the 27 contract

27 section 3.4 declares `match_entry(&self, title: &str, entry_id: i64)`. That is not enough for these trackers, because Kavita, Komga, Suwayomi and Jellyfin all match by the **source-provided URL** (`manga.url`, `anime.url`), not by title (`Kavita.kt:101-106`, `Komga.kt:99-104`, `Suwayomi.kt:92-97`, `Jellyfin.kt:88-93`). The design therefore changes the signature to:

```rust
// kuro-track/src/trackers/enhanced.rs
#[derive(Debug, Clone)]
pub struct EntryRef {
    pub id: i64,                 // manga_id / anime_id (set on the result by the caller)
    pub source_id: i64,          // Kurobayomi source id (area 17)
    pub url: String,             // the source's manga.url / anime.url
    pub title: String,           // for logs only
}
```

Proposal: change 27's `EnhancedMangaTracker::match_entry` and `EnhancedAnimeTracker::match_entry` to take `&EntryRef`. The 27 callers in `bind_enhanced` (27 behaviour 6.2) pass `EntryRef` built from the entry row. This is a required 27 edit (flag for the 27 owner).

Also 27's `accepts(source_id: &str)` takes a string. These trackers store the source's **Kotlin class name** (e.g. `eu.kanade.tachiyomi.extension.all.kavita.Kavita`), which has no meaning in Kurobayomi. Proposal: `accepted_sources()` returns the Kurobayomi source identity keys derived from the extension package and language, for example `("kavita", "all")`. Section 4 defines the mapping. Keep the 27 `accepts(&str)` signature and pass the identity key.

### 3.2 Source directory and preferences (new, `sources.rs`)

```rust
/// Replaces `sourceManager.get(id) as ConfigurableSource` (Kavita.kt:122-129,
/// JellyfinInterceptor.kt:54, SuwayomiApi.kt:109-111). Returns None when the source is not installed.
pub trait SourceDirectory: Send + Sync {
    fn source_prefs(&self, source_id: i64) -> Option<Arc<dyn SourcePrefs>>;
}

/// Read-only view of a source's preference file (`source_<id>`). Returns the default when absent.
pub trait SourcePrefs: Send + Sync {
    fn get_string(&self, key: &str, default: &str) -> String;
}
```

Fixes D-05 and D-17: a missing source is `None`, which the code handles per slot instead of throwing.

The preference store is area:02-preferences-system's per-source preference table. The trackers read it on every call (no in-memory cache of credentials, see section 3.6 and pitfall P-7).

### 3.3 Enhanced error type (new, `enhanced.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum EnhancedError {
    #[error("Unauthorized / api key not valid")]
    Unauthorized,                    // 401 from Kavita authenticate or any API call
    #[error("Error fetching JWT token")]
    ServerError,                     // 500 from Kavita authenticate
    #[error("source not installed")]
    SourceNotInstalled,              // D-05 / D-17 fixed: explicit, per slot
    #[error("source not configured")]
    SourceNotConfigured,
    #[error("search is not supported for this tracker")]
    SearchUnsupported,               // D-21 (replaces TODO / Exception("Not used"))
    #[error("tracking URL has no fragment")]
    MissingFragment,                 // D-28
    #[error("no chapter {0} on the server")]
    ChapterNotOnServer(String),      // D-25
    #[error("no episode {0} on the server")]
    EpisodeNotFound(String),         // D-12
    #[error("unexpected response: {0}")]
    Decode(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

impl From<EnhancedError> for TrackError {
    fn from(e: EnhancedError) -> Self {
        match e {
            EnhancedError::Unauthorized => TrackError::Unauthorised,
            EnhancedError::Http(h) => TrackError::Http(h),
            other => TrackError::Message(other.to_string()),
        }
    }
}
```

User-visible text: `Message` is shown verbatim by 27's `MsgKey::TrackerError`. The English strings above are log text only. The UI shows the localised key `track.enhanced.<variant>` (new keys, see section 7.2), so an English string never reaches the UI from this crate. Proposal: the `From` impl becomes `MsgKey::TrackerError { tracker, detail: key }` once 27 accepts the key form.

### 3.4 Shared helpers (`enhanced.rs`)

```rust
pub fn accepts(accepted: &[&str], source_identity: &str) -> bool {
    accepted.iter().any(|a| *a == source_identity)
}

/// Kotlin isTrackFrom(track, entry, source) for Kavita, Komga, Jellyfin.
/// Suwayomi overrides it (D-09 fixed, see SuwayomiTracker).
pub fn is_track_from(track_remote_url: &str, entry_url: &str, source_identity: Option<&str>, accepted: &[&str]) -> bool {
    track_remote_url == entry_url && source_identity.is_some_and(|s| accepts(accepted, s))
}

/// Kotlin migrateTrack: copy remote_url to the new entry's url if the new source is accepted.
pub fn migrate_remote_url(new_source_identity: &str, accepted: &[&str], new_entry_url: &str) -> Option<String> {
    accepts(accepted, new_source_identity).then(|| new_entry_url.to_owned())
}
```

### 3.5 Kavita

```rust
// trackers/kavita/mod.rs
pub const KAVITA_TRACKER_ID: i64 = 8;
pub const UNREAD: i64 = 1;
pub const READING: i64 = 2;
pub const COMPLETED: i64 = 3;
pub const KAVITA_SLOTS: usize = 3;

pub struct KavitaTracker {
    base: BaseTracker,                       // 27 §5.2 (credential state, watch channel)
    api: KavitaApi,
    deps: Arc<TrackerDeps>,                  // SourceDirectory, clock
}

#[async_trait::async_trait]
impl MangaTracker for KavitaTracker {
    fn status_list(&self) -> Vec<i64> { vec![UNREAD, READING, COMPLETED] }
    fn reading_status(&self) -> i64 { READING }
    fn rereading_status(&self) -> i64 { -1 }
    fn status_label(&self, s: i64) -> Option<&'static str> {
        match s { UNREAD => Some("unread"), READING => Some("reading"), COMPLETED => Some("completed"), _ => None }
    }
    fn display_score(&self, _: &MangaTrack) -> String { String::new() }
    async fn update(&self, t: &MangaTrack, _did_read: bool) -> Result<MangaTrack, TrackError>;  // see note
    async fn bind(&self, _t: &mut MangaTrack, _has_read: bool) -> Result<(), TrackError> { Ok(()) }
    async fn search(&self, _q: &str) -> Result<Vec<MangaTrackSearch>, TrackError> {
        Err(EnhancedError::SearchUnsupported.into())
    }
    async fn refresh(&self, t: &MangaTrack) -> Result<MangaTrack, TrackError>;
    // score_list() = [] (default), completion_status() = COMPLETED
}
```

Note on `update` (D-02 fix): the Kotlin status branch is dead code. Proposal: `update` writes progress and returns the server-refreshed `MangaTrack`. The caller persists **that** returned track's `status`, `last_chapter_read` and `total_chapters`. This changes behaviour: the status now follows the server after every write. It matches what the Kotlin status effectively is (server-driven), so it is a correction, not a new rule.

Refresh (D-03 fix): copy only `status`, `last_chapter_read`, `total_chapters` from the remote result. Keep `score`, `started_reading_date`, `finished_reading_date`, `private` from the local track.

```rust
// trackers/kavita/auth.rs
pub struct KavitaAuth {
    table: tokio::sync::RwLock<Option<[Option<KavitaSlot>; KAVITA_SLOTS]>>,  // None = never loaded
    load_gate: tokio::sync::Mutex<()>,                                       // single-flight loader
}

#[derive(Clone, Debug)]
pub struct KavitaSlot { pub api_url: String, pub jwt: String }

impl KavitaAuth {
    /// Returns the JWT for an api base, loading the table once (single-flight).
    pub async fn token_for(&self, api_base: &str, http: &reqwest::Client,
                           dir: &dyn SourceDirectory) -> Result<String, EnhancedError>;
    /// Clears the slot whose api_url matches (D-07), so the next call re-authenticates.
    pub async fn invalidate(&self, api_base: &str);
}
```

`token_for` algorithm (replaces `loadOAuth` + `OAuth.getToken`):
1. If the table is loaded, find the slot with `api_url == api_base`. If found and `jwt` non-empty, return it.
2. Else acquire `load_gate`. Re-check (another task may have loaded). If the table is still unloaded, run the load:
   - For slot `n` in `1..=3`: `prefs = dir.source_prefs(source_id("kavita_<n>", "all", 1))`. `None` → slot stays empty and a WARN is logged (D-05 fix). `Some(p)`: `url = p.get_string("APIURL", "")`, `key = p.get_string("APIKEY", "")`. Either empty → empty slot.
   - Else call `authenticate(url, key)`. `Err(Unauthorized | ServerError | Http)` → log WARN for that slot and leave it empty; **continue** with the next slot (D-05 fix). `Ok(None)` → empty slot. `Ok(Some(token))` → slot `{ api_url: url, jwt: token }`.
   - Store the table.
3. Return the matching slot's JWT, or `EnhancedError::Unauthorized` when no slot matches (the Kotlin sends `Bearer null` here; Proposal: fail fast, D-06 cousin).
4. `invalidate(api_base)` sets that slot's `jwt` to `""`. The API layer calls it on a 401 and retries **once** (D-07 fix).

### 3.6 Kavita API

```rust
// trackers/kavita/api.rs
pub struct KavitaApi {
    http: reqwest::Client,          // system DNS (D-04 fix: one client for all calls)
    auth: Arc<KavitaAuth>,
    dir: Arc<dyn SourceDirectory>,
}

impl KavitaApi {
    /// `{host}/api/series/{id}` -> `{host}/api`. Kotlin: `url.split("/api/").first() + "/api"`.
    pub fn api_base(url: &str) -> String;
    /// Last path segment parsed as i32 (Kotlin `substringAfterLast("/").toInt()`).
    pub fn series_id(url: &str) -> Result<i32, EnhancedError>;

    pub async fn get_track_search(&self, url: &str) -> Result<MangaTrackSearch, EnhancedError>;
    pub async fn update_progress(&self, t: &MangaTrack) -> Result<MangaTrack, EnhancedError>;

    async fn authenticate(&self, api_url: &str, api_key: &str) -> Result<Option<String>, EnhancedError>;
    async fn total_chapters(&self, api: &str, id: i32) -> Result<i64, EnhancedError>;
    async fn latest_chapter_read(&self, api: &str, id: i32) -> Result<f64, EnhancedError>;
}
```

`get_track_search(url)` (behaviour 2.5):
```rust
let api = Self::api_base(url);
let id = Self::series_id(url)?;
let series: SeriesDto = self.get_json(url).await?;                     // step 1
let (total, latest) = tokio::try_join!(                                 // steps 5 and 8, run in parallel
    self.total_chapters(&api, id),
    self.latest_chapter_read(&api, id),
)?;
Ok(MangaTrackSearch {
    title: series.name.clone(),
    cover_url: series.thumbnail_url.unwrap_or_default(),               // D-01: "" never "null"
    tracking_url: url.to_owned(),
    total_chapters: total,
    status: kavita_status(series.pages, series.pages_read),
    last_chapter_read: latest,
    ..Default::default()
})
```

`kavita_status(pages, pages_read)` (behaviour 2.5 step 7), **with the D-24 fix**:
```rust
pub fn kavita_status(pages: i32, pages_read: i32) -> i64 {
    if pages_read == 0 { UNREAD }          // fixed order: 0/0 is UNREAD, not COMPLETED (D-24)
    else if pages_read == pages { COMPLETED }   // == as in Kotlin (pagesRead > pages stays READING)
    else { READING }
}
```
Parity note: Kotlin gives `COMPLETED` for `pages == pagesRead` including `0 == 0`. The fix changes only the `0/0` case (an empty series). Flag as intentional (D-24). `pagesRead > pages` stays `READING` in both.

`total_chapters(api, id)` (behaviour 2.6), with D-23 guards:
```rust
let volumes: Vec<VolumeDto> = self.get_json(&format!("{api}/Series/volumes?seriesId={id}")).await?;
let mut volume_count: i64 = 0;
let mut max_chapter: i64 = 0;
for v in &volumes {
    if v.chapters.is_empty() { continue; }                                   // D-23: Kotlin throws here
    let mut max_f = f32::MIN;
    for c in &v.chapters {
        let n: f32 = c.number.as_deref().unwrap_or("-1").parse()
            .map_err(|_| EnhancedError::Decode(format!("chapter number {:?}", c.number)))?;   // D-23
        max_f = max_f.max(n);
    }
    if max_f == 0.0 { volume_count += 1; }
    else if (max_chapter as f32) < max_f { max_chapter = max_f as i64; }    // truncation, as Kotlin toLong()
}
Ok(if max_chapter > volume_count { max_chapter } else { volume_count })
```
Parity: `max_f as i64` truncates toward zero, as Kotlin `Float.toLong()` does. `max_f == 0.0` compares to `0f` (Kotlin `== 0f`, so `-0.0 == 0.0`, same as Rust).

`latest_chapter_read(api, id)` (behaviour 2.7):
```rust
let resp = self.send_get(&format!("{api}/Tachiyomi/latest-chapter?seriesId={id}")).await?;
match resp.status().as_u16() {
    200 => {
        let dto: ChapterDto = resp.json().await?;
        let n = dto.number.ok_or_else(|| EnhancedError::Decode("number null".into()))?;
        let v: f64 = n.replace(',', ".").parse().map_err(|_| EnhancedError::Decode(n.clone()))?;
        Ok(v.max(0.0))                                                       // D-22: clamp "-1"
    }
    204 => Ok(0.0),
    _ => Ok(0.0),                                                           // Kotlin falls through silently
}
```

`update_progress(t)` (behaviour 2.8):
```rust
let api = Self::api_base(&t.tracking_url);
let id = Self::series_id(&t.tracking_url)?;
let url = format!("{api}/Tachiyomi/mark-chapter-until-as-read?seriesId={id}&chapterNumber={}", kotlin_double(t.last_chapter_read));
self.post_json_empty(&url, "{}").await?;                               // any 2xx
self.get_track_search(&t.tracking_url).await.map(|s| s.into_track_with(t))
```

`kotlin_double(f64) -> String` helper: format with the Kotlin `Double.toString()` rules. `12.0` → `"12.0"`, `12.5` → `"12.5"`, `1e-4` → `"1.0E-4"`. Implement with `format!("{:?}", v)` for finite values (Rust's Debug for f64 prints `12.0`, `12.5`, and `0.0001` as `0.0001`), and special-case exponents below `1e-3` or at or above `1e7` to Kotlin's `E` form. Note in pitfall P-6. Only values in `[0, 1e7)` occur in practice (chapter numbers), so the simple `{:?}` form is sufficient; document the range assumption with a `debug_assert!`.

`authenticate(api_url, api_key)` (behaviour 2.3), with D-06 fix:
```rust
let url = reqwest::Url::parse_with_params(&format!("{api_url}/Plugin/authenticate"),
            &[("apiKey", api_key), ("pluginName", "Tachiyomi-Kavita")])?;   // encodes the key (Proposal)
let resp = self.http.post(url)
    .header(reqwest::header::CONTENT_TYPE, "application/json; charset=utf-8")
    .body("{}")
    .timeout(Duration::from_secs(120))                                      // callTimeout parity
    .send().await;
match resp {
    Err(e) if e.is_timeout() => { tracing::warn!(...); Ok(None) }          // SocketTimeoutException parity
    Err(e) => { tracing::error!(...); Err(e.into()) }
    Ok(r) => match r.status().as_u16() {
        200 => Ok(Some(r.json::<AuthenticationDto>().await?.token)),
        401 => Err(EnhancedError::Unauthorized),
        500 => Err(EnhancedError::ServerError),
        _ => Ok(None),
    }
}
```

### 3.7 Komga

```rust
pub const KOMGA_TRACKER_ID: i64 = 6;
pub struct KomgaTracker { base: BaseTracker, api: KomgaApi }
pub struct KomgaApi { http: reqwest::Client, auth: KomgaAuth }   // see section 10 (blocking decision)
pub const READLIST_API: &str = "/api/v1/readlists";

impl KomgaApi {
    pub async fn get_track_search(&self, url: &str) -> Result<MangaTrackSearch, EnhancedError>;
    pub async fn update_progress(&self, t: &MangaTrack) -> Result<MangaTrack, EnhancedError>;
}

/// Parity with KomgaApi.kt:76 (readlist/series progress shapes) and fix D-18.
pub fn v2_progress_url(url: &str) -> String { url.replace("/api/v1/series/", "/api/v2/series/") + "/read-progress/tachiyomi" }

pub fn komga_status(books_count: i32, unread: i32, read: i32) -> i64 {
    if books_count == unread { UNREAD }        // Kotlin branch order kept (D-20: empty series is UNREAD)
    else if books_count == read { COMPLETED }
    else { READING }
}
```

Readlist versus series (behaviour 3.2 and 3.3):
- `is_readlist = url.contains(READLIST_API)`.
- Series progress: `ReadProgressV2Dto`. Readlist progress: `ReadProgressDto` with `to_v2()` (`maxNumberSort = books_count as f32`, `lastReadContinuousNumberSort = last_read_continuous_index as f64`).
- D-18 fix: parse V2 when the URL is v1 series **or** v2 series; V1 only for readlists.

Update payload (behaviour 3.4):
- v1 series URL: `{"lastBookNumberSortRead": <f64>}` via `serde_json::json!({ "lastBookNumberSortRead": t.last_chapter_read })`. Note `serde_json` writes `12.0` for `12.0_f64`, matching Kotlin.
- Otherwise: `{"lastBookRead": t.last_chapter_read as i32}` (truncation, as `toInt()`).

Content type: `application/json` (no charset), matching `KomgaApi.kt:96`. Use `.header(CONTENT_TYPE, "application/json")`.

### 3.8 Suwayomi

```rust
pub const SUWAYOMI_TRACKER_ID: i64 = 9;
pub struct SuwayomiTracker { base: BaseTracker, api: SuwayomiApi }
pub struct SuwayomiApi { http: reqwest::Client, dir: Arc<dyn SourceDirectory>, source_id: i64 }

impl SuwayomiApi {
    /// Reads "Server URL Address", "Login (Basic Auth)", "Password (Basic Auth)" per call (D-08 fix).
    fn settings(&self) -> Result<SuwayomiSettings, EnhancedError>;
    pub async fn get_track_search(&self, track_url: &str) -> Result<MangaTrackSearch, EnhancedError>;
    pub async fn update_progress(&self, t: &MangaTrack) -> Result<MangaTrack, EnhancedError>;
}

pub struct SuwayomiSettings { pub base_url: String, pub login: String, pub password: String }
```

`get_track_search` (behaviour 4.3–4.4):
```rust
let url = match track_url.parse::<i64>() {
    Ok(id) => {
        let base = self.settings()?.base_url;
        if base.is_empty() { return Err(EnhancedError::SourceNotConfigured); }     // D-26 fix
        format!("{base}/api/v1/manga/{id}")
    }
    Err(_) => track_url.to_owned(),
};
let m: MangaDataClass = self.get_json(&format!("{url}/full")).await?;
Ok(MangaTrackSearch {
    title: m.title.clone(),
    cover_url: format!("{url}/thumbnail"),
    summary: m.description.clone().unwrap_or_default(),
    tracking_url: url.clone(),
    total_chapters: m.chapter_count,
    publishing_status: m.status.clone(),
    last_chapter_read: m.last_chapter_read.as_ref().map(|c| c.chapter_number).unwrap_or(0.0),
    status: suwayomi_status(m.unread_count, m.chapter_count),
    ..Default::default()
})
```

`suwayomi_status(unread: Option<i64>, chapters: i64) -> i64`:
```rust
match unread {
    Some(u) if u == chapters => UNREAD,
    Some(0) => COMPLETED,
    _ => READING,                     // None falls through, as in Kotlin (null matches nothing)
}
```

`update_progress` (behaviour 4.5), with D-25 fix:
```rust
let chapters: Vec<ChapterDataClass> = self.get_json(&format!("{}/chapters", t.tracking_url)).await?;
let target = t.last_chapter_read;
let idx = chapters.iter().find(|c| c.chapter_number == target)
    .ok_or_else(|| EnhancedError::ChapterNotOnServer(kotlin_double(target)))?.index;
self.http.put(format!("{}/chapter/{idx}", t.tracking_url))
    .headers(self.headers()?)
    .form(&[("markPrevRead", "true"), ("read", "true")])
    .send().await?.error_for_status()?;
self.get_track_search(&t.tracking_url).await.map(|s| s.into_track_with(t))
```
`.form()` sends `application/x-www-form-urlencoded`. Kotlin `FormBody` also sends that. Parity.

Headers (behaviour 4.2): `User-Agent` = the default-UA preference (`default_user_agent`, area 12). If both login and password are non-empty, `basic_auth(login, Some(password))`.

`SuwayomiTracker::is_track_from` (D-09 fix): `self.accepts(source) && track.remote_url == entry.url`. Kotlin does not compare the URL. Proposal: compare, as the other three do.

### 3.9 Jellyfin

```rust
pub const JELLYFIN_TRACKER_ID: i64 = 102;
pub const UNSEEN: i64 = 1;
pub const WATCHING: i64 = 2;
pub const COMPLETED: i64 = 3;

pub struct JellyfinTracker { base: BaseTracker, api: JellyfinApi }

pub enum TrackingUrl<'a> {
    Movie { user_id: &'a str, item_id: &'a str, url: &'a Url },
    Season { user_id: &'a str, season_id: &'a str, series_id: &'a str, url: &'a Url },
}

impl TrackingUrl<'_> {
    /// Parses `…/Users/{u}/Items/{i}#movie…` or `…#seriesId,{s}`. Missing fragment → MissingFragment (D-28).
    pub fn parse(url: &Url) -> Result<TrackingUrl<'_>, EnhancedError>;
}
```

`TrackingUrl::parse` rules (behaviour 5.1):
- Fragment `None` → `MissingFragment`.
- `pathSegments[1]` = user id; `pathSegments.last()` = item id.
- `fragment.starts_with("movie")` → `Movie`.
- `fragment.starts_with("seriesId")` → `Season`, `series_id = fragment.split(',').last()`.
- Other → `Decode("unknown fragment")`.

`JellyfinAuth` (behaviour 5.2, D-15 and D-16 fixes):
```rust
pub struct JellyfinAuth { dir: Arc<dyn SourceDirectory> }   // no cache: prefs are an in-memory map

impl JellyfinAuth {
    /// Slots 1..=10. Skips missing/unconfigured slots (D-17). Returns the api key for user_id, or
    /// EnhancedError::SourceNotConfigured ("Please log in through the extension").
    pub fn api_key_for(&self, user_id: &str) -> Result<String, EnhancedError> {
        for n in 1..=10 {
            let Some(p) = self.dir.source_prefs(source_id(&jellyfin_name(n), "all", 1)) else { continue };
            let uid = p.get_string("user_id", "");
            if uid.is_empty() { continue; }
            if uid == user_id {
                let key = p.get_string("api_key", "");
                if key.is_empty() { return Err(EnhancedError::SourceNotConfigured); }   // D-16: not cached
                return Ok(key);
            }
        }
        Err(EnhancedError::SourceNotConfigured)
    }
}

/// "jellyfin" for n = 1, "jellyfin (n)" otherwise (JellyfinInterceptor.kt:44-46).
pub fn jellyfin_name(n: u32) -> String { if n == 1 { "jellyfin".into() } else { format!("jellyfin ({n})") } }
```

Auth transport (Proposal, pitfall P-8): send the key in the `X-Emby-Token` header (Jellyfin accepts it), not the `api_key` query. This keeps the key out of URLs and logs. The Kotlin used the query; keep the query only if a test server rejects the header.

`JellyfinApi` (behaviour 5.3 to 5.5):
```rust
pub struct JellyfinApi { http: reqwest::Client, auth: JellyfinAuth }

impl JellyfinApi {
    pub async fn get_track_search(&self, track_url: &Url) -> Result<AnimeTrackSearch, EnhancedError>;
    pub async fn update_progress(&self, t: &AnimeTrack) -> Result<AnimeTrack, EnhancedError>;
    async fn episodes(&self, season_url: &Url, series_id: &str, season_id: &str, user_id: &str) -> Result<Vec<JFItem>, EnhancedError>;
}
```

`episodes(...)` (behaviour 5.4), the URL build with D-13 and D-14 fixes:
```rust
let host = url.origin().ascii_serialization();          // scheme://host[:port]
let mut u = Url::parse(&host)?;
u.path_segments_mut().unwrap().extend(["Shows", series_id, "Episodes"]);
u.query_pairs_mut()
    .append_pair("seasonId", season_id)
    .append_pair("userId", user_id)
    .append_pair("Fields", "Overview,MediaSources")
    .append_pair("SortBy", "IndexNumber")               // D-14 fix (not in Kotlin)
    .append_pair("SortOrder", "Ascending");
// Note: the Kotlin keeps the path root only (encodedPath("/")), so a reverse-proxy prefix is dropped.
// Proposal: keep the prefix, see D-13.
```

Parity note: adding `SortBy`/`SortOrder` changes the result order only when the server default differs. It is the fix for D-14.

`get_track_search` (behaviour 5.3):
- Season: `items = episodes(...)`; `total = items.last().index_number.ok_or(Decode)?` (D-14: use the maximum index instead, `items.iter().filter_map(|i| i.index_number).max()`). First unwatched index logic from behaviour 5.4 (position-based, parity). Proposal: keep position-based for the continuous rule, because the list is sorted ascending after the D-14 fix.
- Movie: `last_episode_seen = if played {1.0} else {0.0}`, `total_episodes = 1`.

`update_progress` (behaviour 5.5), **Proposal D-11 and D-32 fixes**:
```rust
// Item ids to mark played, in ascending index order.
let item_ids: Vec<String> = match tracking {
    Movie { item_id, .. } => {
        // D-32: the Kotlin posts "played" for any target. This port posts only when the target is >= 1.0.
        // There is no "unplayed" call in this port (the Kotlin has none either).
        if t.last_episode_seen >= 1.0 { vec![item_id.to_owned()] } else { vec![] }
    }
    Season { user_id, season_id, series_id, .. } => {
        let items = self.episodes(url, series_id, season_id, user_id).await?;
        let target = t.last_episode_seen + EPISODE_EPSILON;
        if !consts::JELLYFIN_MARK_RANGE {
            // Kotlin parity: exactly one episode whose index equals the target (JellyfinApi.kt:131-141).
            items.iter().filter(|e| e.index_number.is_some_and(|n| (n as f64 - t.last_episode_seen).abs() < EPISODE_EPSILON))
                .map(|e| e.id.clone()).collect()
        } else {
            // D-11 proposal: every unplayed episode with index <= target (continuous rule).
            items.iter().filter(|e| !e.user_data.played && e.index_number.is_some_and(|n| (n as f64) <= target))
                .map(|e| e.id.clone()).collect()
        }
        // Empty result: log at INFO and return Ok (D-12 parity: no error).
    }
};
futures::stream::iter(item_ids)
    .map(|id| self.mark_played(user_id, &id, OffsetDateTime::now_utc()))
    .buffer_unordered(consts::JELLYFIN_MARK_CONCURRENCY)
    .try_collect::<Vec<()>>().await?;
self.get_track_search(url).await.map(|s| s.into_track_with(t))
```

`mark_played(user_id, item_id, now)`:
```rust
let date = now.to_offset(UtcOffset::UTC).format(&DATE_FMT)?;      // D-10 fix
let mut u = self.base_url.join(&format!("Users/{user_id}/PlayedItems/{item_id}"))?;
u.query_pairs_mut().append_pair("DatePlayed", &date);
self.http.post(u).header("X-Emby-Token", &key).send().await?.error_for_status()?;
```

`DATE_FMT` (for the `time` crate):
```rust
static DATE_FMT: &[time::format_description::FormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
```
This produces `2026-10-08T14:03:21.123Z`. The UTC conversion is the D-10 fix.

`JellyfinTracker::update(t, did_watch)` → `api.update_progress(t)`. The `did_watch` flag is ignored (D-31, parity). Document it.

### 3.10 Shared client construction (`http.rs`)

```rust
pub fn tracker_client(base: &reqwest::Client) -> reqwest::Client {
    // Kotlin: networkService.client.newBuilder().dns(Dns.SYSTEM) (all four trackers).
    // reqwest uses the system resolver by default (getaddrinfo). Do not enable a DoH resolver here.
    base.clone()
}
```
The client comes from `kuro-net` (area 12) with connect timeout 30 s, read timeout 30 s, and per-request call timeout 120 s (`NetworkHelper.kt:23-25`). The Kotlin OkHttp HTTP cache is not replicated (pitfall P-5).

---

## 4. Source identity (`crates/kuro-sources/src/identity.rs`)

```rust
/// Kotlin: MD5("<name lowercase>/<lang>/<versionId>"), first 8 bytes big-endian, & i64::MAX.
pub fn generate_source_id(name: &str, lang: &str, version_id: i32) -> i64 {
    use md5::{Digest, Md5};
    let key = format!("{}/{}/{}", name.to_lowercase(), lang, version_id);
    let digest = Md5::digest(key.as_bytes());
    let mut v: u64 = 0;
    for b in &digest[..8] { v = (v << 8) | u64::from(*b); }
    (v & (i64::MAX as u64)) as i64
}
```

Trackers call it with the names in `p1-data-and-config.md` section 4:
- Kavita: `generate_source_id(&format!("kavita_{n}"), "all", 1)` for `n` in `1..=3`.
- Jellyfin: `generate_source_id(&jellyfin_name(n), "all", 1)` for `n` in `1..=10`.
- Suwayomi: `generate_source_id("tachidesk", "en", 1)` = `3100117499901280806`.

Regression vectors are in `p1-edge-cases-and-tests.md` section 7 (task T-33a-002).

`accepted_sources()` (section 3.1 proposal): Kavita `[("kavita", "all")]`, Komga `[("komga", "all")]`, Suwayomi `[("tachidesk", "en")]`, Jellyfin `[("jellyfin", "all")]`. Identity string = `"{name}/{lang}"`. The source directory of area 17 maps an installed source to this string.

---

## 5. Concurrency and async model

Follows 27 section 4. Specific to these trackers:

1. **Kavita series read** runs `series`, then `try_join!(total_chapters, latest_chapter_read)` (section 3.6). Kotlin runs them sequentially. Same results, lower latency.
2. **Kavita auth** is single-flight through `KavitaAuth.load_gate` (`tokio::sync::Mutex<()>`). The table itself sits behind `RwLock`. A load is never run twice in parallel (Kotlin can, see behaviour 9).
3. **Jellyfin marking** runs up to 4 `POST`s concurrently (`buffer_unordered(4)`). Kotlin does one POST.
4. **Refresh** is the 27 path: `JoinSet` with a `Semaphore` of 4 permits per tracker id.
5. **No `spawn_blocking`**: all four trackers are async end to end. The Kotlin blocking `execute()` calls in Kavita (`KavitaApi.kt:41-44`, `:103-107`, `:127-128`) become `await`.
6. **Non-cancellable writes** (27 §4 rule 2): `update_progress` runs under the 27 `spawn` helper with the join handle detached. A page close does not abort a half-written sync.
7. The GTK thread never touches these types. Results come back through 27's `async_channel`.

---

## 6. Error handling and logging

| Situation | Kotlin | Rust |
|---|---|---|
| 401 on Kavita authenticate | IOException wrapped (D-06) | `EnhancedError::Unauthorized`, slot skipped, other slots continue |
| 500 on Kavita authenticate | same | `EnhancedError::ServerError`, slot skipped |
| Kavita token timeout | `null` (skip slot) | `Ok(None)` |
| Missing source (slot or Jellyfin) | NPE/ClassCastException | `SourceNotInstalled` → skip slot, WARN once per load |
| Unknown Komga progress shape | parse exception | `Decode`, error returned to caller, logged WARN "Could not get item: {url}" |
| `match` failure | swallowed to `null` (D-29) | keep `Result`; `bind_enhanced` logs and continues (27) |
| Search | TODO / `Exception("Not used")` | `EnhancedError::SearchUnsupported` → `TrackError::Message` |
| Missing chapter / episode | NoSuchElement / silent | `ChapterNotOnServer` (error) / `EpisodeNotFound` (log only, parity) |
| Log messages | logcat WARN/ERROR with the texts in `p1-behavior.md` section 7 | `tracing::warn!` / `tracing::error!`, same wording |

Logging: never log the API key or the JWT. Log the URL with the query string removed (pitfall P-8).

---

## 7. Settings and UI integration (no UI file in scope; wiring only)

### 7.1 Settings group

The mockup's `spec/settings.json` `settings_tracking` screen already has an "Enhanced trackers" group with four `action` rows (Komga, Kavita, Suwayomi, Jellyfin) and the static info text. Two gaps versus Kotlin:
- Kotlin shows only the trackers whose accepted source is installed (`SettingsTrackingScreen.kt:116-135`). The mockup always shows four rows.
- Kotlin appends "Available but source not installed: %s" (`enhanced_services_not_installed`) for the missing ones. The mockup has no such line.

Proposal: `src/track/settings.rs` (27 §1) builds the group at runtime from `TrackerManager::enhanced_*()` and the `SourceDirectory`. Each row is an `action`, and its subtitle shows "Logged in" or "Not logged in" based on `is_logged_in()`. The info row is built from `enhanced_tracking_info` plus `enhanced_services_not_installed` with the names joined by `", "`.

### 7.2 Message keys (new, for the enhanced error variants)

These keys do not exist in `strings.xml`. Proposal: add them to the Kurobayomi string table (area 51) with the English text shown:

| Key | English text | Used for |
|---|---|---|
| `track.enhanced.unauthorized` | "Unauthorized / api key not valid" | `Unauthorized` |
| `track.enhanced.server_error` | "Error fetching JWT token" | `ServerError` |
| `track.enhanced.login_required` | "Please log in through the extension" | `SourceNotConfigured` (Jellyfin, Kavita) |
| `track.enhanced.search_unsupported` | "Search is not available for this tracker" | `SearchUnsupported` |
| `track.enhanced.missing_fragment` | "Tracking link is incomplete" | `MissingFragment` |
| `track.enhanced.chapter_missing` | "Chapter %s is not on the server" | `ChapterNotOnServer` |

These are new strings, so the text is a Proposal. The Kotlin shows raw exception text in the toast (`MangaTracker.kt:119`). The port shows the key text.

### 7.3 Logo and colour

27 §1 `src/track/ui/logo.rs` renders a 48 px tile from `TrackerLogo`. Each tracker returns:
- Kavita: asset `ic_tracker_kavita` (webp in Kotlin), colour `#4AC694`.
- Komga: `ic_tracker_komga`, `#332532`.
- Suwayomi: `ic_tracker_suwayomi`, `#FF2323` (Kotlin marks this value `// TODO`, D-27).
- Jellyfin: `ic_tracker_jellyfin`, `#000B25`.

Convert the four webp files to PNG or SVG and ship them in the GResource bundle (T-33a-100). Proposal: use the tracker's brand colour for the tile background only where the logo is transparent.

---

## 8. Constants (single source of truth)

```rust
pub mod consts {
    pub const KAVITA_ID: i64 = 8;
    pub const KOMGA_ID: i64 = 6;
    pub const SUWAYOMI_ID: i64 = 9;
    pub const JELLYFIN_ID: i64 = 102;
    pub const KAVITA_SLOTS: usize = 3;          // Kavita.kt:118 (`for (id in 1..3)`)
    pub const JELLYFIN_SLOTS: usize = 10;       // JellyfinInterceptor.kt:71
    pub const CONNECT_TIMEOUT_SECS: u64 = 30;   // NetworkHelper.kt:23
    pub const READ_TIMEOUT_SECS: u64 = 30;     // NetworkHelper.kt:24
    pub const CALL_TIMEOUT_SECS: u64 = 120;    // NetworkHelper.kt:25
    pub const JELLYFIN_MARK_CONCURRENCY: usize = 4;   // Proposal
    pub const JELLYFIN_MARK_RANGE: bool = true;       // Proposal: D-11 fix; false = Kotlin parity
    pub const EPISODE_EPSILON: f64 = 0.001;     // JellyfinApi.kt:162 (`< 0.001`)
    pub const SUWAYOMI_SOURCE_NAME: &str = "tachidesk";
    pub const SUWAYOMI_SOURCE_LANG: &str = "en";
}
```

---

## 9. Testing seams

- `SourceDirectory` and `SourcePrefs` are traits. Tests use an in-memory directory (`kuro-testkit`, 27 provides the helper).
- `reqwest` is hidden behind a `HttpSend` trait used by the API structs, so unit tests replay recorded JSON (fixtures in `p1-edge-cases-and-tests.md` section 5). Proposal: use `wiremock` as a dev-dependency for the request-shape tests (URL, query, headers, body).
- The clock is a `fn() -> OffsetDateTime` injected into `JellyfinApi`.

---

## 10. Open decisions (blocking, need the product owner)

1. **Komga authentication (blocking for T-33a-060).** The Kotlin sends no `Authorization` header and relies on cookies in the shared cookie jar (`NetworkHelper.kt:18`, `:22`). Kurobayomi has no extension-owned cookie jar. Options:
   - (a) Ask the user for the Komga URL, username and password in the tracker login dialog, store the password in `SecretStore` (27 §5.1), and send Basic auth. Recommended.
   - (b) Copy cookies from the Komga source's browser session. Not recommended.
   The rest of the Komga tracker is independent of this choice, so T-33a-060 can start with (a) stubbed.
2. **Kavita token renewal (D-07).** Kotlin never renews. Proposal: re-authenticate on 401 once. Confirm that the Kavita server returns 401 (not a redirect) when the JWT expires. Not verified.
3. **Jellyfin multi-episode marking (D-11).** Proposal: mark the continuous range. Strict parity is one const (`JELLYFIN_MARK_RANGE`).
4. **Refresh and personal fields (D-03).** Proposal: keep local score and dates on refresh. Those trackers have no score list and no date support (`BaseTracker.kt:23-25`), so the visible effect is limited to backups. Confirm.
5. **Enhanced credentials (section 5.2 of data doc).** Proposal: a boolean flag instead of dummy `"user"`/`"pass"`. Confirm that backup import of `pref_mangasync_username_<id>` values still maps to logged-in state (area 21/22).

---

## 11. Android to Linux mapping (this scope)

| Android / Kotlin | Linux / Rust in this design |
|---|---|
| OkHttp `OkHttpClient` + `newBuilder()` | `reqwest::Client` (one per tracker, cloned) |
| OkHttp `Interceptor` (Kavita, Jellyfin) | explicit `AuthContext` argument, no chain |
| `Dns.SYSTEM` | reqwest default resolver |
| `Credentials.basic` | `RequestBuilder::basic_auth` (pitfall P-3) |
| `FormBody` | `RequestBuilder::form` |
| `MediaType` / `toRequestBody` | `RequestBuilder::header(CONTENT_TYPE, …)` + `.body(…)` |
| `HttpUrl`, `newBuilder`, `encodedPath`, `removePathSegment` | `url::Url`, `path_segments_mut`, `query_pairs_mut` |
| `kotlinx.serialization` `@Serializable`, `@SerialName` | `serde` `#[derive]`, `#[serde(rename)]`, `#[serde(default)]` |
| `injekt` `injectLazy` | constructor injection (`TrackerDeps`) |
| `sourceManager.get(id) as ConfigurableSource` | `SourceDirectory::source_prefs(id) -> Option` |
| `SharedPreferences` `source_<id>` | area:02-preferences-system per-source preference store via `SourcePrefs` |
| `MessageDigest("MD5")` | `md-5` crate |
| `SimpleDateFormat(..., Locale.US)` | `time` crate `format_description!` |
| `lazy {}` | `OnceLock` or constructor-time values |
| `TODO("…")` | `Err(EnhancedError::SearchUnsupported)` |
| `withIOContext {}` | `async fn` on the tokio runtime (no wrapper) |
| `withNonCancellableContext` | detached `tokio::spawn` (27) |
| `logcat(LogPriority.WARN) { … }` | `tracing::warn!` |
| `Context.getString` / `MR.strings` | message keys resolved by the GUI (27 §3.7) |
| Android drawable `R.drawable.ic_tracker_*` (webp) | GResource PNG/SVG (T-33a-100) |

---

## 12. Pitfalls

- **P-1 Dead status.** Do not port the local status branch of `update` as if it were effective. It is never persisted (D-02). Keep status server-driven.
- **P-2 Refresh wipes personal fields.** Copy only the remote-owned fields on refresh (D-03).
- **P-3 Basic auth charset.** OkHttp's `Credentials.basic(user, pass)` uses ISO-8859-1 by default, while `reqwest::RequestBuilder::basic_auth` uses UTF-8. For ASCII credentials the header is identical. For non-Latin-1 passwords the two differ. Verify against the OkHttp version the app uses before choosing; the Proposal is UTF-8.
- **P-4 Source ids with spaces.** Jellyfin slot names contain a space and parentheses (`jellyfin (2)`). Do not normalise them.
- **P-5 OkHttp cache.** Kotlin GETs can come from the 5 MiB HTTP cache. Stale progress is possible in Kotlin and not in Rust. Do not add a cache layer.
- **P-6 Kotlin Double formatting.** `kotlin_double` must match `Double.toString()` for the chapter-number range (`[0, 1e7)`). `format!("{:?}")` matches for that range. Keep the `debug_assert!`.
- **P-7 Preference reads.** Read source prefs per call. Do not cache credentials (D-08 and D-16).
- **P-8 Secrets in URLs.** The Kotlin Jellyfin path puts `api_key` in every URL and Kavita puts `apiKey` in the authenticate query. Proposal: headers for Jellyfin (`X-Emby-Token`), and redact the query in logs for Kavita authenticate (`apiKey=***`).
- **P-9 `value as f32` vs `f64`.** Kavita chapter numbers are parsed as `Float` in Kotlin (`toFloat()`) and then `toLong()`. Rust must parse `f32` for the max walk and `f64` for `last_chapter_read`, to match the Kotlin precision.
- **P-10 Season URLs.** A Jellyfin tracking URL points to a season. Its `played` flag is the season's, not the series'. The status comes from the episode walk, not from the season flag, when the fragment is `seriesId`.
- **P-11 Empty server responses.** Kavita `latest-chapter` returns 204 with no body. Do not call `.json()` on 204.
- **P-12 Interface drift.** The four trackers' `match` takes a URL. Do not reuse 27's title-based `match_entry` (section 3.1).
