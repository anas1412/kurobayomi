# area:28-tracker-anilist, part 1 — Rust design

Scope: the AniList tracker only. The generic tracker framework (`Tracker`, `MangaTracker`, `AnimeTracker`, `setRemote*`, `updateRemote`, delayed queue, interactors) belongs to area:27-tracking-core and is referenced here by its trait names only.

Current mockup facts (read from `/home/blackbox/kurobayomi`):
- `Cargo.toml`: package `kurobayomi` 0.1.0, edition 2024; deps `gtk` (= `gtk4` 0.11.5, feature `v4_16`), `serde` 1.0.229 (derive), `serde_json` 1.0.151. No tokio, reqwest, rusqlite, chrono yet.
- Single binary crate; modules `src/main.rs`, `clamp.rs`, `entry.rs` (585 lines), `player.rs`, `screens.rs`, `settings.rs`, `spec.rs`, `ui.rs`.
- Tracking UI is mock: `src/entry.rs:496-537` builds a `tracking_sheet` with three hard-coded rows (`AniList` bound, `MyAnimeList`, `Kitsu`) and a static grid `("Watching","status"), ("12 / 24","chapters"), ("8.0","score"), ("Start date","start_date"), ("Finish date","finish_date")`; `src/entry.rs:539-566` builds `tracker_search` with four fake results ("Author name", "Type: Manga", "Score: 8.4", …), a `Track` button and a "Track privately" icon button.
- Dialog ids used: `dialog_of(id, "remove_tracking")`, `dialog_of(id, d)` with `d` in `status`, `chapters`, `score`, `start_date`, `finish_date`.
- `spec/settings.json:34-38` declares `settings_tracking` ("Tracking", "One-way progress sync, enhanced sync"). `spec/settings.json:2039-2044` has the "Tracking guide" action label.

## 1. Crate and module layout

Keep one binary crate for now (matches mockup). Add a `tracker` module tree:

```
src/
  tracker/
    mod.rs                  // pub use anilist::AniList; trait TrackerService (see §4)
    anilist/
      mod.rs                // AniList struct, public API used by UI and by sync jobs
      status.rs             // MangaStatus, AnimeStatus, API string mapping
      score.rs              // ScoreFormat, index<->stored, display, api score
      auth.rs               // OAuthRecord, AuthState, redirect parsing, expiry
      api.rs                // GraphQL client (reqwest), RateLimiter, error mapping
      queries.rs            // const &str GraphQL documents (copied from p1-data §4)
      dto.rs                // serde structs (p1-data §5)
      convert.rs            // DTO -> TrackSearch / Track (p1-behavior §7)
      service.rs            // update/bind/refresh/delete logic (p1-behavior §10)
      html.rs               // description -> plain text
      date.rs               // FuzzyDate <-> epoch ms (local zone)
      error.rs              // AniListError
    model.rs                // Track, TrackSearch (shared with area:27)
```

If area:27 creates a workspace split later (`kurobayomi-core` crate), `tracker/` moves intact; nothing here depends on GTK.

## 2. Public types

```rust
// src/tracker/anilist/status.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum MangaStatus { Reading = 1, Completed = 2, OnHold = 3, Dropped = 4, PlanToRead = 5, Rereading = 6 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum AnimeStatus { Completed = 2, OnHold = 3, Dropped = 4, Watching = 11, PlanToWatch = 15, Rewatching = 16 }

/// AniList MediaListStatus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiStatus { Current, Planning, Completed, Dropped, Paused, Repeating }

impl MangaStatus {
    pub const ALL_ORDER: [MangaStatus; 6] = [Reading, PlanToRead, Completed, Rereading, OnHold, Dropped]; // UI order
    pub fn from_db(v: i64) -> Option<Self>;
    pub fn to_db(self) -> i64;
    pub fn to_api(self) -> ApiStatus;           // p1-behavior §2.5
    pub fn from_api(a: ApiStatus) -> Self;       // p1-behavior §2.6 (manga)
    pub fn label_key(self) -> &'static str;      // i18n key, §8
}
impl AnimeStatus { /* same shape; ALL_ORDER = [Watching, PlanToWatch, Completed, Rewatching, OnHold, Dropped] */ }
```

Notes:
- `from_db` returns `None` for unknown integers (the Kotlin would show an empty label; Rust shows "Unknown" key, §8).
- `ApiStatus::from_str` parses `"CURRENT"|"PLANNING"|…` and returns `Err(AniListError::UnknownStatus(String))` (Kotlin throws `NotImplementedError`).

```rust
// src/tracker/anilist/score.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreFormat { Point100, Point10, Point10Decimal, Point5, Point3 }

impl ScoreFormat {
    pub fn from_api(s: &str) -> Result<Self, AniListError>;   // "POINT_100" etc.
    pub fn to_api(self) -> &'static str;
    /// Picker options, index 0..n (p1-behavior §3.3).
    pub fn options(self) -> Vec<String>;
    /// Index -> stored 0..100 score (p1-behavior §3.4). Index beyond range -> Err.
    pub fn index_to_score(self, index: usize) -> Result<f64, AniListError>;
    /// Stored score -> picker index that is displayed as the current value (see §5 for the POINT_3 fix).
    pub fn score_to_index(self, score: f64) -> usize;
    /// Stored score -> display string (p1-behavior §3.5).
    pub fn display(self, score: f64) -> String;
    /// Stored score -> integer scoreRaw for mutations (0..=100, clamped).
    pub fn to_score_raw(score: f64) -> i32;
}

/// Stored 100-point score as float to 10-point number (library/stats averages)
pub fn score_as_10(score: f64) -> f64 { score / 10.0 }
```

Proposal details for `options()` are in §5 (picker design). `index_to_score` for POINT_3: index 0 → 0.0, 1 → 35.0, 2 → 60.0, 3 → 85.0.

```rust
// src/tracker/anilist/auth.rs
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(String);          // Debug impl redacts; Display never implemented
impl std::fmt::Debug for AccessToken { fn fmt(&self, f) -> fmt::Result { f.write_str("AccessToken(<redacted>)") } }

#[derive(Clone)]
pub struct OAuthRecord {
    pub token: AccessToken,
    pub token_type: String,      // "Bearer"
    pub expires_at: i64,         // unix seconds (absolute)
    pub expires_in: i64,         // seconds
}

pub enum AuthState {
    LoggedOut,
    LoggedIn { viewer_id: i64, token: AccessToken, expires_at: i64, score_format: ScoreFormat },
    Expired { viewer_id: i64 },   // set on HTTP 401 (see §6)
}

pub const CLIENT_ID: &str = "5338";
pub fn authorize_url() -> url::Url;   // https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token

/// Parses the redirect URI received by the app: `kurobayomi://anilist-auth#access_token=...&...`
pub fn parse_redirect(uri: &url::Url) -> Result<RedirectOutcome, AniListError>;
pub enum RedirectOutcome { Token { token: AccessToken, expires_in: Option<i64> }, Denied }

pub const DEFAULT_LIFETIME_S: i64 = 31_536_000; // 365 days, same value the Kotlin assumes (in ms there)
```

`parse_redirect` uses `uri.fragment()` parsed with `url::form_urlencoded::parse`, reads `access_token` (required; missing → `Denied`), `expires_in` (optional i64). It does **not** require a trailing `&` (fix for the Kotlin regex fragility, p1-behavior §4.3).

```rust
// src/tracker/anilist/api.rs
pub struct AniListHttp {
    http: reqwest::Client,          // one client; default timeouts 30 s connect 10 s (proposal)
    limiter: RateLimiter,           // shared by all tracker calls (85 / 60 s)
    auth: Arc<RwLock<AuthState>>,
}

pub struct RateLimiter { window: Duration, capacity: usize, log: tokio::sync::Mutex<VecDeque<Instant>> }
impl RateLimiter {
    pub async fn acquire(&self);   // sliding window (p1-behavior §5.1)
}

#[derive(Deserialize)]
pub struct GraphQlEnvelope<T> { pub data: Option<T>, #[serde(default)] pub errors: Vec<GraphQlError> }
#[derive(Deserialize)]
pub struct GraphQlError { pub message: String, #[serde(default)] pub status: Option<i64> }

impl AniListHttp {
    pub async fn execute<V: Serialize, T: DeserializeOwned>(
        &self, query: &'static str, variables: Option<V>, auth: Auth,
    ) -> Result<T, AniListError>;   // Auth::Required | Auth::None  (AniSkip/AniChart do not use this)
}
pub enum Auth { Required, None }
```

Behaviour of `execute` (proposal, see p1-behavior §5.2):
1. `Auth::Required` and state != LoggedIn → `Err(AniListError::NotAuthenticated)`.
2. Acquire rate limiter.
3. Send POST `https://graphql.anilist.co/` with headers `Authorization: Bearer …`, `User-Agent: Kurobayomi v{CARGO_PKG_VERSION} (io.github.kurobayomi)` (App id placeholder must be decided; the Kotlin used `BuildConfig.APPLICATION_ID`), `Content-Type: application/json`.
4. HTTP 401 → set state `Expired`, return `AuthExpired`. 429 → `RateLimited { retry_after: Option<Duration> }`. Other non-2xx → `Http(code)`. 5xx → `Transient`.
5. Decode `GraphQlEnvelope<T>`. If `errors` non-empty → `GraphQl(messages)` even on 200. If `data` is None → `Decode`.

```rust
// src/tracker/anilist/mod.rs
pub struct AniList {
    http: AniListHttp,
    score_format: ScoreFormat,      // cached from Viewer.mediaListOptions.scoreFormat
    viewer_id: Option<i64>,
}

impl AniList {
    pub const TRACKER_ID: i64 = 2;
    pub const NAME: &'static str = "AniList";
    pub const LOGO_RGB: (u8, u8, u8) = (18, 25, 35);
    pub const SUPPORTS_READING_DATES: bool = true;
    pub const SUPPORTS_PRIVATE: bool = true;

    pub async fn login(&mut self, token: AccessToken, expires_in: Option<i64>) -> Result<LoginSuccess, AniListError>;
    pub fn logout(&mut self);                // clears state; caller clears DB-stored token
    pub fn is_logged_in(&self) -> bool;

    pub async fn search_manga(&self, q: &str) -> Result<Vec<MangaTrackSearch>, AniListError>;
    pub async fn search_anime(&self, q: &str) -> Result<Vec<AnimeTrackSearch>, AniListError>;

    pub async fn find_manga_entry(&self, media_id: i64) -> Result<Option<MangaTrack>, AniListError>;
    pub async fn find_anime_entry(&self, media_id: i64) -> Result<Option<AnimeTrack>, AniListError>;

    pub async fn add_manga(&self, t: &MangaTrack) -> Result<i64 /* MediaList id */, AniListError>;
    pub async fn add_anime(&self, t: &AnimeTrack) -> Result<i64, AniListError>;
    pub async fn save_manga(&self, t: &MangaTrack) -> Result<(), AniListError>;   // UpdateManga
    pub async fn save_anime(&self, t: &AnimeTrack) -> Result<(), AniListError>;   // UpdateAnime
    pub async fn delete_manga(&self, list_id: i64) -> Result<(), AniListError>;
    pub async fn delete_anime(&self, list_id: i64) -> Result<(), AniListError>;

    pub async fn anilist_to_mal_id(&self, media_id: i64) -> Result<Option<i64>, AniListError>; // AniSkip, Auth::None
    pub async fn next_airing(&self, media_id: i64) -> Result<Option<(i64, i64)>, AniListError>; // AniChart, Auth::None
}
```

Track service logic (`service.rs`) implements the Kotlin `update/bind/refresh/delete` flows exactly (p1-behavior §10) as free functions over `&AniList` and the domain track type, so they are unit-testable with a fake HTTP layer (`trait Transport`, see §7).

### 2.1 Domain track types (shared with area:27)

```rust
pub struct MangaTrack {
    pub id: Option<i64>,            // local row id, None before insert
    pub manga_id: i64,
    pub tracker_id: i64,            // 2
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_chapter_read: f64,
    pub total_chapters: i64,
    pub score: f64,                 // 0..=100 stored scale
    pub status: i64,                // DB integer, see MangaStatus::to_db
    pub started_reading_date: i64,  // epoch ms, 0 = unset
    pub finished_reading_date: i64,
    pub tracking_url: String,
    pub private: bool,
}
pub struct AnimeTrack { /* analogous: anime_id, last_episode_seen, total_episodes, started_watching_date, finished_watching_date */ }

pub struct MangaTrackSearch { pub remote_id: i64, pub title: String, pub total_chapters: i64,
    pub score: f64 /* -1.0 when unknown */, pub status: i64, pub authors: Vec<String>, pub artists: Vec<String>,
    pub cover_url: String, pub summary: String, pub publishing_status: String, pub publishing_type: String,
    pub start_date: String, pub tracking_url: String, pub private: bool }
pub struct AnimeTrackSearch { /* same with total_episodes, no artists used */ }
```

Note: `status`, `score`, `private` etc. in search results default to -1.0 / 0 / false exactly as the Kotlin `*TrackSearch` classes do.

## 3. Error model

```rust
#[derive(Debug, thiserror::Error)]
pub enum AniListError {
    #[error("Not authenticated with AniList")]              NotAuthenticated,       // was "Not authenticated with Anilist"
    #[error("AniList session expired")]                     AuthExpired,           // HTTP 401
    #[error("AniList rate limit reached")]                  RateLimited { retry_after: Option<std::time::Duration> },
    #[error("HTTP error {0}")]                              Http(u16),              // formats like HttpException
    #[error("AniList server error")]                        Transient,
    #[error("AniList error: {0}")]                          GraphQl(String),
    #[error("could not read AniList response")]             Decode(#[source] serde_json::Error),
    #[error("no network")]                                  Offline,                 // from reqwest is_connect when offline
    #[error("entry not found on your AniList list")]        NotInList,               // was "Could not find manga/anime"
    #[error("unknown AniList status {0}")]                  UnknownStatus(String),
    #[error("unknown AniList score format {0}")]            UnknownScoreFormat(String),
    #[error("score index out of range")]                    ScoreOutOfRange,
    #[error(transparent)]                                   Transport(#[from] reqwest::Error),
}
```

Mapping rules: `Offline` is produced when `reqwest::Error::is_connect()` and the system is offline (check `NetworkManager`/`gio::NetworkMonitor::default().is_network_available()`), otherwise `Transport`. The UI maps `Offline` to the `exception_offline` key and `Http(n)` to `exception_http` (p1-behavior §12).

No `unwrap`/`expect` in the tracker module (Kotlin `!!` sites are replaced by explicit errors; p1-edge-cases E-07…E-10).

## 4. The generic tracker trait (consumed by the UI and by area:27 jobs)

```rust
#[async_trait::async_trait]   // or native async fn in trait (Rust 1.75+) when no dyn needed
pub trait TrackerService: Send + Sync {
    fn id(&self) -> i64;
    fn name(&self) -> &'static str;
    fn is_logged_in(&self) -> bool;
    fn supports_reading_dates(&self) -> bool;
    fn supports_private_tracking(&self) -> bool;
    fn is_deletable(&self) -> bool;
    fn score_options(&self, kind: MediaKind) -> Vec<String>;
    fn display_score(&self, kind: MediaKind, score: f64) -> String;
    fn index_to_score(&self, kind: MediaKind, index: usize) -> Result<f64, TrackError>;
    fn status_options(&self, kind: MediaKind) -> Vec<(i64, &'static str /* i18n key */)>;

    async fn search(&self, kind: MediaKind, query: &str) -> Result<Vec<TrackSearch>, TrackError>;
    async fn bind(&self, kind: MediaKind, local: &mut TrackRow, has_progress: bool) -> Result<(), TrackError>;
    async fn update(&self, kind: MediaKind, row: &mut TrackRow, progress_event: Option<f64>) -> Result<(), TrackError>;
    async fn refresh(&self, kind: MediaKind, row: &mut TrackRow) -> Result<(), TrackError>;
    async fn delete_remote(&self, kind: MediaKind, row: &TrackRow) -> Result<(), TrackError>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MediaKind { Manga, Anime }
```

Rationale: area:27 owns the framework trait; this trait is the minimal surface AniList must satisfy. If area:27 defines a different trait, AniList implements it and this section is updated; the AniList-specific logic stays in `service.rs`. Using a single enum `MediaKind` avoids duplicating the manga and anime code paths in the UI.

## 5. Score picker design (fixes the POINT_3 mismatch)

- The picker stores **index** (not the display string). `score_options()` gives strings only for rendering.
- Initial index = `score_to_index(stored)`:
  - POINT_3: stored 0.0 → index 0 (`"-"`); 35 → 1; 60 → 2; 85 → 3; any other value → nearest bucket by the thresholds in p1-behavior §3.5 (≤35 → 1, ≤60 → 2, else 3).
  - POINT_5: stored 0 → index 0; else `index = round((stored + 10) / 20)`, clamped to 1..=5.
  - POINT_10: `index = round(stored / 10)`, clamped to 0..=10. POINT_100: `index = round(stored)`, clamped to 0..=100. POINT_10_DECIMAL: `index = round(stored)`, clamped to 0..=100.
- Confirm → `index_to_score(index)`. This never produces a negative value (index is always in range), so the Kotlin `indexOf == -1` pathology is impossible.
- Widget: `gtk::DropDown` is not used (wheel picker in the Kotlin); use a `gtk::ListBox` with checkmarks for POINT_3/POINT_5 (≤6 items) and a `gtk::SpinButton` (0..=max, step 1) with a decimal label for POINT_10/POINT_100/POINT_10_DECIMAL. (Final widget choice belongs to area:27 UI parity; this is the proposal.)

## 6. Auth lifecycle (Rust)

```
startup:
  load track row for tracker 2 from settings store (viewer_id, token, expires_at, score_format)
  if token present and expires_at > now  -> AuthState::LoggedIn
  if token present and expires_at <= now -> AuthState::Expired (show "log in again" row)
  else                                   -> LoggedOut

login (after redirect):
  parse_redirect(uri) -> Token{token, expires_in}
  store token with expires_at = now + expires_in.unwrap_or(DEFAULT_LIFETIME_S)
  GET Viewer -> (id, score_format)      // on failure: logout, toast "AniList login failed: <msg>"
  persist viewer_id, score_format

401 during any call:
  state = Expired; keep viewer_id; clear token; toast "AniList session expired, log in again"

logout:
  clear token, viewer_id, score_format, state = LoggedOut
```

Differences from Kotlin (all intentional, listed in p1-behavior §4):
- Login failures are reported (Kotlin silently logs out).
- Expiry is real (Kotlin's arithmetic never expires).
- Token redirect parsing does not depend on a trailing `&`.
- The "null" string written on logout is not written (we delete the row).

## 7. Testing seam

```rust
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn post_graphql(&self, body: &serde_json::Value, bearer: Option<&str>) -> Result<(u16, bytes::Bytes), reqwest::Error>;
}
```
Production: `reqwest` implementation. Tests: a canned-response implementation keyed on the operation name (`AddManga`, `UpdateManga`, …) reading JSON fixtures from `tests/fixtures/anilist/*.json` (p1-edge-cases §4). The rate limiter takes a `Clock` trait (`fn now(&self) -> Instant`, `async fn sleep_until`) so the 85/60 s rule is testable without real time.

## 8. i18n

- Status labels: keys `reading`, `plan_to_read`, `completed`, `repeating`, `paused`, `dropped` (manga) and `watching`, `plan_to_watch`, `repeating_anime`, … as in p1-behavior §12. The Rust side uses fluent-style or simple `&'static str` keys mapped to English defaults; keep the English text identical to the Kotlin strings.
- Error keys (new, proposal): `anilist_err_not_authenticated`, `anilist_err_session_expired`, `anilist_err_not_in_list`, `anilist_err_rate_limited`, `anilist_err_offline`, `anilist_err_unknown_status`.
- Use `MR`-like message keys in the UI layer only; the tracker module returns typed errors.

## 9. Async and threading model

- Runtime: one `tokio` multi-thread runtime created at startup (`tokio::runtime::Runtime::new()` owned by a `static` `OnceLock`), *not* the GLib main context.
- GTK main thread never awaits tracker futures. UI calls `TrackerRuntime::spawn(fut, move |res| glib::idle_add_local_once(...))` or uses `async_channel::Sender` drained by `glib::MainContext::default().spawn_local`. Preferred: `glib::MainContext::default().spawn_local(async move { let r = tokio_handle.spawn(fut).await; ui_update(r) })`.
- Rate limiter and auth state are `Arc`-shared; `RwLock` for auth, `tokio::sync::Mutex` for the request log.
- Cancellation: dropping the `JoinHandle` does not cancel reqwest; use `tokio::select!` with a `CancellationToken` for the search dialog (typing a new query cancels the previous search).
- The Kotlin "non-cancellable" section (`withNonCancellableContext`) for track writes is reproduced by spawning writes detached (`tokio::spawn` without holding the UI handle) so that closing a dialog does not abort an in-flight mutation. The DB write after a successful mutation runs in the same task.

## 10. Persistence hook

- Settings store (area:02): keys `anilist.viewer_id` (i64), `anilist.score_format` (String), `anilist.token_expires_at` (i64, unix s). Token itself in the keyring (`keyring` crate, service `kurobayomi`, user `anilist-token`). Fallback when no secret service is running: settings file with mode 0600 and a UI warning. (Kotlin stored the token in plain preferences.)
- Track rows (area:04/05 tables): AniList writes go through the framework's repository (`insert` with `ON CONFLICT REPLACE`). The Rust code must keep the same uniqueness `(media_kind, tracker_id=2, local_id)`.

## 11. Crates

| Need | Crate | Why |
|---|---|---|
| HTTP | `reqwest` (features `json`, `rustls-tls`) | replaces OkHttp; async; no OpenSSL dependency |
| JSON | `serde`, `serde_json` (already present) | replaces kotlinx.serialization |
| Async runtime | `tokio` (features `rt-multi-thread`, `sync`, `time`, `macros`) | rate limiter and I/O |
| Async in traits | `async-trait` (only if `dyn TrackerService` needed) | otherwise native `async fn` |
| Time | `chrono` (`clock`) | replaces java.time (`LocalDate`, `ZoneId.systemDefault()`) |
| URL | `url` | authorize URL and redirect parsing |
| HTML | `ammonia` (no tags), `html-escape` | replaces `Html.fromHtml` |
| Errors | `thiserror` | typed errors |
| Logs | `tracing` | replaces logcat |
| Secret store | `keyring` | replaces plaintext preferences for the token |
| Bytes | `bytes` | Transport trait |
| Channels | `async-channel` | tokio → GTK handoff |
| Testing | `tokio` `test` macro, `pretty_assertions` (dev-only) | fixtures |

Dependency additions are listed in the tasks file (T-28a-001).

## 12. Android API → Linux mapping (this area)

| Android/Kotlin | Linux / Rust replacement |
|---|---|
| OkHttp `OkHttpClient`, `Interceptor`, `Call.awaitSuccess()` | `reqwest::Client`, middleware-free function `execute`, explicit status mapping |
| `rateLimit` interceptor (`SystemClock`, `Semaphore`) | `RateLimiter` with `tokio::time::Instant` |
| kotlinx.serialization `@Serializable`/`@SerialName` | serde `#[derive(Deserialize)] #[serde(rename = "…")]` |
| `android.net.Uri`, `appendQueryParameter`, `fragment` | `url::Url`, `form_urlencoded` |
| Activity intent-filter `aniyomi://anilist-auth` | `.desktop` file with `MimeType=x-scheme-handler/kurobayomi;` and `Exec=kurobayomi %u`; `GApplication::open` or argv handling receives the URI (fragment is preserved because the URI is passed whole) |
| `context.openInBrowser(url, forceDefaultBrowser)` | `gio::AppInfo::launch_default_for_uri` (or `gtk::UriLauncher` in GTK 4.10+) |
| `SharedPreferences` (`track_token_2`, `anilist_score_type`, `tracking_queue`) | settings store (area:02) + keyring for the token |
| `java.time.LocalDate`, `ZoneId.systemDefault()`, `SimpleDateFormat("yyyy-MM-dd", Locale.US)` | `chrono::NaiveDate`, `chrono::Local`, `format("%Y-%m-%d")` |
| `Html.fromHtml` (`parseAsHtml`) | `ammonia` + `html-escape` (p1-behavior §9) |
| `Dispatchers.IO` / `withIOContext` | `tokio::task::spawn_blocking` not needed; reqwest is async |
| `WorkManager` (delayed updates) | area:27 job scheduler (not AniList-specific) |
| `ConnectivityManager.isOnline()` | `gio::NetworkMonitor::default().is_network_available()` |
| Injekt `injectLazy` | constructor injection (`AniList::new(settings, http)`) |
| `BuildConfig.VERSION_NAME`, `APPLICATION_ID` | `env!("CARGO_PKG_VERSION")`, app id constant |

## 13. Integration with the mockup (concrete edits proposed)

1. `src/entry.rs:496-537` (`tracking()`): replace the three hard-coded rows with a loop over `TrackerService` instances. AniList row shows `title` from the bound track, status label from `status_options`, progress as `"{last} / {total}"` (`total` 0 → only `last`), score from `display_score`, dates from the track. Keep the existing `dialog_of(id, "status" | "chapters" | "score" | "start_date" | "finish_date")` ids; those dialog ids are the same names used by the Kotlin picker screens.
2. `src/entry.rs:539-566` (`tracker_search()`): replace fake results with `search()` output. Keep the `Track` and `Track privately` buttons; wire them to `bind()` with `private` true/false. Add a `Button` "Open in browser" that calls `tracking_url`.
3. `src/entry.rs:522-531`: for not-bound trackers, keep "Add tracking" → `tracker_search` (unchanged).
4. `spec/settings.json` (`settings_tracking`): add the AniList login row with label "Log in with AniList" / "Log out of AniList" driven by `is_logged_in()`. Keep the subtitle text unchanged. Add a row "Tracking guide" (already in spec at line 2044).
5. `src/settings.rs`: add the AniList section that opens `authorize_url()` via `gtk::UriLauncher`.
6. `src/main.rs`: register the scheme handler; forward `kurobayomi://anilist-auth#…` to `tracker::anilist::auth::parse_redirect` then `AniList::login`.
7. `src/spec.rs`: no change required; if `spec/*.json` is the source of truth for strings, add the new keys listed in §8.
8. `src/ui.rs`: toast helper already exists (or add one) for `track_error`-style messages.

The mockup's `tracking_sheet` has no real data flow today; §13 is a change list, not a rewrite.

## 14. Pitfalls

- The GraphQL error envelope is HTTP 200 with `errors`: always check `errors` before using `data` (Kotlin ignores it for mutations).
- The `scoreRaw` value must be an integer in 0..=100; negative values from picker bugs must be impossible by construction (§5).
- `updateLibX` requires `id` (MediaList id). Never send `null`; if `library_id` is None, resolve it first with `find_*_entry`, persist, then update (Kotlin does this inside `update`).
- `delete` must send the MediaList id, not the local row id (Kotlin bug, p1-behavior §10.5).
- Date round trip loses time-of-day (p1-data §12). Do not store the AniList-sent date as a timestamp at 00:00 UTC; store local midnight.
- Search requires login in the Kotlin (interceptor). Proposal: test whether unauthenticated `Page(media(...))` search works on the live API; if it does, use `Auth::None` for search. Not verified in this area (docs site unreachable), so keep login required until checked. A change is a behaviour change and must be called out in release notes.
- Keep the token out of logs and out of `Debug` output (`AccessToken` redacts).
- The Kotlin `ALUserListItem` requires `media.staff` / `media.studios` non-null. Make these `Option` in serde and handle `None` (empty author list), not panic.
- The `mediaList` query returns at most one entry per (user, media) pair; if AniList returns more than one, pick the first and log a warning.
- Do not reuse the Kotlin `Page(perPage:50)` assumption for find queries; find uses the default page size.
