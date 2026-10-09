# Area 29-tracker-myanimelist, part 1: Rust design

Scope: the MyAnimeList tracker from `p1-behavior.md`. Repo-relative source paths are under `/home/blackbox/aniyomi-src`.

Assumed shared pieces (owned by other areas, see the cross-references):
- `kuro-core`: errors, `tracing`, `time` (area:01-foundation-app-bootstrap), preference store (area:02-preferences-system).
- `kuro-net`: shared HTTP client builder (area:12-network-stack). This design builds its own client without a cookie jar and without an HTTP cache (section 4).
- `kuro-db`: SQLite access (area:05-data-repos-entries-items). Tables from area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations.
- `kuro-platform`: OS-specific code (area:01-foundation-app-bootstrap and this area's secret store and browser launch).
- `kuro-track`: generic tracker types and the `TrackerService` trait (area:27-tracking-core). This design names the trait as assumed and lists its exact shape in section 3.

Nothing in this design uses libadwaita. All GTK widgets are vanilla GTK 4 (gtk4-rs 0.11, GTK 4.16 features).

## 1. Module layout

```
crates/kuro-track/src/
  lib.rs                       re-exports; the generic TrackerService trait (area:27-tracking-core)
  mal/
    mod.rs                     pub use; MAL_TRACKER_ID = 1; MalTracker
    consts.rs                  CLIENT_ID (from config, section 5), URL bases, PAGE_SIZE = 250, QUERY_MAX = 64
    status.rs                  MalStatus enum, wire mapping (both directions), labels
    score.rs                   MalScore(0..=10), index/score conversion, display
    dates.rs                   parse_mal_date, to_mal_date, local midnight, empty-string clearing rule
    dto.rs                     all serde DTOs (section 6)
    token.rs                   MalToken, expiry rule, serde
    pkce.rs                    verifier generation, authorize URL
    auth.rs                    MalAuth: token lock, refresh, expired flag, TokenStore trait
    redirect.rs                loopback HTTP listener for the code (tokio), paste fallback parser
    api.rs                     MalApi: every endpoint in p1-data-and-config.md section 7
    search.rs                  search routing (id:, my:, default) and N+1 bounded fetches
    tracker.rs                 MalTracker: TrackerService impl (update, bind, refresh, delete, logout)
    error.rs                   MalError (section 4.3)
crates/kuro-platform/src/secret.rs    SecretStore over the Secret Service (libsecret) with file fallback
crates/kuro-platform/src/browser.rs   open_url(&str) via GTK UriLauncher / gio
src/tracking/                         GTK UI for the Settings row, login dialog, tracking sheet (section 7)
```

Rule: `kuro-track::mal` contains no GTK type and no `cfg(target_os)`. Token storage and browser launch are injected as traits so tests run without a desktop.

## 2. Crates and why

| Crate (features) | Crate | Why | Replaces |
|---|---|---|---|
| `reqwest` 0.12 (`default-features = false`, features `rustls-tls`, `json`, `form`, `gzip`) | kuro-net, used by kuro-track | Async HTTP. rustls so no OpenSSL link. `form` provides `RequestBuilder::form` used for every MAL form body | OkHttp |
| `serde` (`derive`), `serde_json` | kuro-track | Same JSON library role as kotlinx.serialization | kotlinx.serialization |
| `url` | kuro-track | Query building (`query_pairs_mut`) | `android.net.Uri.Builder` |
| `base64` 0.22 | kuro-track | `URL_SAFE_NO_PAD` engine for the verifier | `java.util.Base64.getUrlEncoder().withoutPadding()` |
| `rand` 0.8 (`std`, `std_rng`, `getrandom` via OsRng) | kuro-track | Secure random bytes via `OsRng` | `SecureRandom` |
| `time` 0.3 (`formatting`, `parsing`, `macros`) | kuro-core, reused | Date strings. Local offset is NOT used here, see pitfalls | `SimpleDateFormat` |
| `chrono` 0.4 (`clock`) | kuro-track | Local-calendar conversion for `yyyy-MM-dd` in the process time zone. Chosen because `time`'s local-offset lookup can fail in multithreaded Unix processes. See section 9.1 | `SimpleDateFormat` with default zone |
| `tokio` (`rt-multi-thread`, `sync`, `time`, `net`, `macros`, `io-util`) | kuro-core | Runtime, `Mutex`, loopback listener | Coroutines, `Mutex`-free code under `synchronized` |
| `futures` | kuro-track | `BoxFuture`, `buffer_unordered` for bounded fan-out | `async` / `awaitAll` |
| `thiserror` | kuro-track | Typed errors | Exception subclasses |
| `tracing` | kuro-core | Logs | `logcat` |
| `keyring` 3 (`sync-secret-service`, `crypto-rust`) | kuro-platform | Secret Service (libsecret) store for the token | Android app preferences (plain text in the app sandbox) |
| `gtk4` 0.11 (`v4_16`) | the app crate (`src/`) | Dialogs, Settings row, tracking sheet | Compose and Material |
| `glib` (via gtk4) | the app crate | `spawn_local`, `MainContext` | `lifecycleScope` |
| `tiny_http` is NOT used | | the loopback listener is written on tokio | not needed |

Dependencies NOT added: `oauth2` (the flow is 2 requests and one verifier; the crate's types add nothing), `chrono-tz`, `time-tz`, `dashmap`.

## 3. Key types and signatures

### 3.1 Status

```rust
// crates/kuro-track/src/mal/status.rs
#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MalStatus {
    Reading = 1,
    Watching = 11,
    Completed = 2,
    OnHold = 3,
    Dropped = 4,
    PlanToRead = 6,
    PlanToWatch = 16,
    Rereading = 7,
    Rewatching = 17,
}

impl TryFrom<i64> for MalStatus {
    type Error = UnknownStatus;
    fn try_from(v: i64) -> Result<Self, UnknownStatus> { /* 1,11,2,3,4,6,16,7,17 */ }
}

pub const MANGA_LIST: [MalStatus; 6] = [Reading, Completed, OnHold, Dropped, PlanToRead, Rereading];
pub const ANIME_LIST: [MalStatus; 6] = [Watching, Completed, OnHold, Dropped, PlanToWatch, Rewatching];

/// Local status -> wire string, manga side. None when the source would return null.
pub fn manga_wire(s: MalStatus) -> Option<&'static str>;   // Rereading -> "reading"
/// Local status -> wire string, anime side.
pub fn anime_wire(s: MalStatus) -> Option<&'static str>;   // Reading -> "watching", PlanToRead -> "plan_to_watch"
/// Wire string -> local code. Unknown -> Reading (see behaviour D-17).
pub fn from_wire(s: Option<&str>) -> i64;
pub fn manga_label(s: i64) -> Option<&'static str>; // 1 "Reading", 2 "Completed", 3 "On hold", 4 "Dropped", 6 "Plan to read", 7 "Rereading"; None otherwise
pub fn anime_label(s: i64) -> Option<&'static str>; // 11 "Watching", 2 "Completed", 3 "On hold", 4 "Dropped", 16 "Plan to watch", 17 "Rewatching"; None otherwise (code 1 has no anime label)
```

Rule: keep the raw `i64` in `kuro-db` rows. Convert with `TryFrom` only at the API boundary. A row with an unknown code is shown with no label and is never sent to MAL (see behaviour D-23 for the proposal).

Labels: use the exact English strings from `p1-behavior.md` section 1.5 as `&'static str`. Each function returns the label for its own list only. They are not gettext-based in this area. The i18n area supplies the translated table later.

### 3.2 Score

```rust
// crates/kuro-track/src/mal/score.rs
pub const SCORE_LIST: [&str; 11] = ["0","1","2","3","4","5","6","7","8","9","10"];
pub fn index_to_score(index: usize) -> f64 { index as f64 }          // 0..=10
pub fn score_from_label(label: &str) -> Option<f64>;                  // indexOf semantics; None instead of -1 (D-12)
pub fn display_score(score: f64) -> String { (score as i64).to_string() }   // Kotlin toInt(): truncation
pub fn wire_score(score: f64) -> String { format!("{score:?}") }      // see D-11; default keeps "8.0"
```

`wire_score` reproduces Kotlin `Double.toString` for the default behaviour. The proposal (integer `8`) is a switch, see `p1-edge-cases-and-tests.md`.

### 3.3 Tracker service (area:27-tracking-core contract, assumed shape)

```rust
// crates/kuro-track/src/lib.rs (area:27-tracking-core owns this trait; shown as the shape this area implements)
pub type TrackFuture<'a, T> = futures::future::BoxFuture<'a, Result<T, TrackError>>;

pub trait TrackerService: Send + Sync {
    fn id(&self) -> i64;                                    // 1 for MAL
    fn name(&self) -> &'static str;                         // "MyAnimeList"
    fn supports_reading_dates(&self) -> bool;               // true
    fn supports_private_tracking(&self) -> bool;            // false
    fn is_logged_in(&self) -> bool;                         // username && token both present

    fn manga_update<'a>(&'a self, t: &'a mut MangaTrackRow, did_read: bool) -> TrackFuture<'a, ()>;
    fn anime_update<'a>(&'a self, t: &'a mut AnimeTrackRow, did_watch: bool) -> TrackFuture<'a, ()>;
    fn manga_bind<'a>(&'a self, t: &'a mut MangaTrackRow, has_read: bool) -> TrackFuture<'a, ()>;
    fn anime_bind<'a>(&'a self, t: &'a mut AnimeTrackRow, has_seen: bool) -> TrackFuture<'a, ()>;
    fn manga_refresh<'a>(&'a self, t: &'a mut MangaTrackRow) -> TrackFuture<'a, ()>;
    fn anime_refresh<'a>(&'a self, t: &'a mut AnimeTrackRow) -> TrackFuture<'a, ()>;
    fn manga_search<'a>(&'a self, q: &'a str) -> TrackFuture<'a, Vec<MangaSearchHit>>;
    fn anime_search<'a>(&'a self, q: &'a str) -> TrackFuture<'a, Vec<AnimeSearchHit>>;
    fn manga_delete<'a>(&'a self, remote_id: i64) -> TrackFuture<'a, ()>;
    fn anime_delete<'a>(&'a self, remote_id: i64) -> TrackFuture<'a, ()>;
    fn login_url(&self) -> Result<url::Url, TrackError>;    // also stores the new verifier
    fn login_with_code<'a>(&'a self, code: &'a str) -> TrackFuture<'a, ()>;   // success or logout
    fn logout(&self);
}
```

`MangaTrackRow` and `AnimeTrackRow` are the area:05-data-repos-entries-items row types (fields in `p1-data-and-config.md` section 4.2). The `MangaSearchHit` and `AnimeSearchHit` types are the area:27-tracking-core search holders with the fields in section 4.3 of the data doc.

A deliberate change from Kotlin: `login_with_code` returns `Result`. Kotlin's `login` swallows the error (`MyAnimeList.kt:257-259`). The caller decides what to show. The logout-on-failure step still runs, to keep upstream state (behaviour D-03 is marked as a proposal for a toast).

### 3.4 MAL client

```rust
// crates/kuro-track/src/mal/api.rs
pub struct MalApi {
    http: reqwest::Client,        // no cookie store, no HTTP cache (section 4)
    auth: Arc<MalAuth>,           // attaches Bearer for api calls
    oauth_base: url::Url,         // https://myanimelist.net/v1/oauth2
    api_base: url::Url,           // https://api.myanimelist.net/v2
    client_id: String,            // from config (section 5)
}

impl MalApi {
    pub async fn exchange_code(&self, code: &str, verifier: &str) -> Result<MalToken, MalError>; // E2
    pub async fn current_user(&self) -> Result<String, MalError>;                              // E4
    pub async fn search_manga(&self, q: &str) -> Result<Vec<MalSearchItem>, MalError>;         // E5
    pub async fn search_anime(&self, q: &str) -> Result<Vec<MalSearchItem>, MalError>;         // E6
    pub async fn manga_details(&self, id: i64) -> Result<MangaDetails, MalError>;              // E7
    pub async fn anime_details(&self, id: i64) -> Result<AnimeDetails, MalError>;              // E8
    pub async fn manga_list_entry(&self, id: i64) -> Result<Option<MangaListEntry>, MalError>; // E9
    pub async fn anime_list_entry(&self, id: i64) -> Result<Option<AnimeListEntry>, MalError>; // E10
    pub async fn my_list_page(&self, offset: u32) -> Result<ListPage, MalError>;               // E11 manga
    pub async fn put_manga(&self, id: i64, body: MangaPut) -> Result<MangaListEntry, MalError>; // E12
    pub async fn put_anime(&self, id: i64, body: AnimePut) -> Result<AnimeListEntry, MalError>; // E13
    pub async fn delete_manga(&self, id: i64) -> Result<(), MalError>;                          // E14
    pub async fn delete_anime(&self, id: i64) -> Result<(), MalError>;                          // E15
}
```

Form bodies are typed, so the field order in `p1-data-and-config.md` section 7 is fixed by the struct:

```rust
pub struct MangaPut {
    pub status: &'static str,        // one of "reading", "completed", "on_hold", "dropped", "plan_to_read"
    pub is_rereading: bool,          // serialised "true"/"false"
    pub score: String,               // wire_score(...)
    pub num_chapters_read: i64,      // last_chapter_read.trunc()
    pub start_date: Option<String>,  // None = omit; Some("") = clear
    pub finish_date: Option<String>,
}
pub struct AnimePut { /* status, is_rewatching, score, num_watched_episodes, start_date, finish_date */ }
```

Use `reqwest::RequestBuilder::form(&pairs)` with a `Vec<(&str, String)>` built in the same order as the Kotlin (`MyAnimeListApi.kt:183-193`). Do not use `#[derive(Serialize)]` for the form, so the order and the `start_date` omission rule stay explicit.

### 3.5 Auth

```rust
// crates/kuro-track/src/mal/auth.rs
pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<MalToken>, MalError>;
    fn save(&self, t: &MalToken) -> Result<(), MalError>;
    fn clear(&self) -> Result<(), MalError>;
}

pub struct MalAuth {
    store: Arc<dyn TokenStore>,
    state: tokio::sync::Mutex<AuthState>,   // replaces `synchronized(this)`
    expired_flag: AtomicBool,               // mirrors __PRIVATE_pref_tracker_auth_expired_1
    refresh_url: url::Url,
}

struct AuthState { token: Option<MalToken> }

impl MalAuth {
    /// Returns the Bearer token to attach, refreshing first if needed.
    pub async fn bearer(&self, http: &reqwest::Client) -> Result<String, MalError>;
    pub fn set_token(&self, t: Option<MalToken>) -> Result<(), MalError>;   // setAuth
    pub fn expired(&self) -> bool;
    pub fn set_expired(&self, v: bool);                                     // setAuthExpired / setCredentials
}
```

`bearer` algorithm (same as `p1-behavior.md` section 4.7 and 4.8):

```rust
pub async fn bearer(&self, http: &reqwest::Client) -> Result<String, MalError> {
    if self.expired() { return Err(MalError::LoginExpired); }
    let mut st = self.state.lock().await;                       // single-flight
    let tok = st.token.clone().ok_or(MalError::NotAuthenticated)?;   // see note below
    if !tok.is_expired(now_secs()) { return Ok(tok.access_token); }
    if self.expired() { return Err(MalError::LoginExpired); }   // re-check under lock
    if let Some(fresh) = st.token.as_ref().filter(|t| !t.is_expired(now_secs())) { return Ok(fresh.access_token.clone()); }
    let resp = http.post(self.refresh_url.clone())
        .bearer_auth(&tok.access_token)                         // manual header, as in refreshTokenRequest
        .form(&[("client_id", CLIENT_ID), ("refresh_token", &tok.refresh_token), ("grant_type", "refresh_token")])
        .send().await.map_err(|_| MalError::RefreshFailed)?;
    if resp.status().as_u16() == 401 { self.set_expired(true); return Err(MalError::LoginExpired); }
    if !resp.status().is_success() { return Err(MalError::RefreshFailed); }
    let new: MalToken = resp.json().await.map_err(|_| MalError::RefreshFailed)?;
    self.store.save(&new)?;
    let access = new.access_token.clone();
    st.token = Some(new);
    Ok(access)
}
```

Note on `NotAuthenticated`: Kotlin checks `oauth == null` after the refresh step (`MyAnimeListInterceptor.kt:28-30`). The Rust code checks it first. The observable result is the same for every path that reaches the network. The order is the one in the Kotlin when there is no token and the expired flag is false: Kotlin throws `User is not authenticated` after skipping refresh. Rust matches that.

Lock held across the refresh request is intentional. The refresh request does not call `bearer`, so there is no re-entry. Other callers wait on the mutex, which matches `synchronized(this)`.

### 3.6 Login flow

```rust
// crates/kuro-track/src/mal/pkce.rs
pub fn new_verifier() -> String;             // 50 random bytes, URL_SAFE_NO_PAD -> 68 chars
pub fn authorize_url(client_id: &str, verifier: &str) -> url::Url;
// order: client_id, code_challenge (= verifier, plain), response_type=code. No redirect_uri (D-…: MAL uses the registered one).
```

```rust
// crates/kuro-track/src/mal/redirect.rs
pub struct Redirect { pub listener: tokio::net::TcpListener, pub url: url::Url }
pub async fn wait_for_code(listener: TcpListener, timeout: Duration) -> Result<String, MalError>;
pub fn parse_pasted(input: &str) -> Result<String, MalError>;   // accepts a full redirect URL or a bare code
```

Login steps (Rust, proposal; order matches `p1-behavior.md` section 4.3 and 4.4):
1. Settings row click (logged out): `let v = new_verifier(); store v in the MalTracker pending slot AND in the config file pending_verifier (section 6 of the data doc)`.
2. Start the loopback listener on `127.0.0.1:<MAL_REDIRECT_PORT>` (section 5). If the bind fails, show the paste dialog only.
3. Open `authorize_url(...)` with `kuro-platform::browser::open_url`.
4. Wait for the code (timeout 5 minutes, proposal) or for the user to paste the redirect URL into the dialog.
5. `login_with_code(code)`: exchange (E2), `set_token`, `current_user` (E4), save username, clear `pending_verifier`.
6. On a missing `code` (consent denied, MAL error param): show "Login was not completed" and keep the existing login. This is proposal D-04.
7. On exchange or user-fetch failure: show the error text and call `logout()` like Kotlin (D-03 proposal: keep the toast, logout keeps upstream behaviour).

Custom-scheme alternative (not the default): register `x-scheme-handler/kurobayomi` in a `.desktop` file and receive `kurobayomi://myanimelist-auth?code=...` through `GApplication::open`. Needs a MAL app registration with that scheme. Keep as a fallback if loopback is refused by MAL.

### 3.7 Trackers in the app

`MalTracker` owns `Arc<MalApi>`, `Arc<MalAuth>`, `Arc<dyn TokenStore>`, a `PreferenceHandle` for the username and auth-expired flag (area:02-preferences-system), and the pending verifier slot. `MalTracker::new(prefs, secret_store, config)` is called once by the app at startup (equivalent of `TrackerManager.myAnimeList`, `TrackerManager.kt:27`).

## 4. Async model and threads

- GTK main loop runs on the main thread. All widget updates happen there.
- A tokio multi-thread runtime (area:01-foundation-app-bootstrap) runs all HTTP and file work. The MAL code never blocks the GTK thread.
- Pattern for every UI action:

```rust
// in the GTK thread
let tracker = tracker.clone();
glib::MainContext::default().spawn_local(async move {
    let res = RUNTIME.spawn(async move { tracker.manga_search(&q).await }).await;   // JoinHandle is a Future
    match res {
        Ok(Ok(hits)) => render(hits),
        Ok(Err(e)) => toast(e.to_string()),
        Err(join) => toast(format!("internal: {join}")),
    }
});
```

- Search fan-out (section 7.3 of the behaviour doc): `futures::stream::iter(ids).map(|id| api.manga_details(id)).buffer_unordered(4)`, collected in input order (`buffered(4)` keeps order; prefer `buffered(4)`). This is a proposal (D-13). Kotlin uses unlimited `async`. The proposal keeps result order as the Kotlin does.
- Refresh single-flight: `MalAuth::state` is a `tokio::sync::Mutex`. Never hold a `std::sync::Mutex` across an `.await`.
- Loopback listener: `tokio::net::TcpListener::bind(("127.0.0.1", port))`. Accept one connection, read the request line, reply with a small HTML page ("You can close this tab"), and drop the listener. Timeout with `tokio::time::timeout`.
- Cancellation: dropping the UI future cancels the HTTP call. The Kotlin uses `launchNonCancellable` for updates (`AnimeTrackInfoDialog.kt:833-834`). Rust keeps the same semantics by spawning the update on the runtime (`RUNTIME.spawn`), which is not tied to the UI future.
- Persistence after remote success: write the local row through `kuro-db` on the runtime, after the PUT returns (as `updateRemote` does, `MangaTracker.kt:111-121`).

## 5. Configuration of the OAuth client (decision needed)

Kotlin embeds `CLIENT_ID = "686b980ff4240fccce7f6a654cea07ce"` (`MyAnimeListApi.kt:383`), which is registered to the Aniyomi project. Proposal:

- Kurobayomi does not reuse that id. The user registers an application at MyAnimeList (API config page, `https://myanimelist.net/apiconfig`) and sets the client id in `$XDG_CONFIG_HOME/kurobayomi/tracking/myanimelist.toml` as `client_id`, or in the environment variable `KUROBAYOMI_MAL_CLIENT_ID`.
- The redirect URI registered with MAL is `http://localhost:<port>/myanimelist-auth`. Proposal port: 8765. Verify MAL accepts `localhost` with a port. If it does not, switch to the custom scheme (section 3.6).
- With no client id configured, the Settings row shows "Set a MyAnimeList client id to log in" and the login is not started.

This is the one decision the user must make before login works. It is listed in the task file (T-29a-040).

## 6. Error handling

```rust
// crates/kuro-track/src/mal/error.rs
#[derive(Debug, thiserror::Error)]
pub enum MalError {
    #[error("MAL: User is not authenticated")]
    NotAuthenticated,
    #[error("MAL: Failed to refresh account token")]
    RefreshFailed,
    #[error("MAL: Login has expired")]
    LoginExpired,
    #[error("HTTP error {0}")]
    Http(u16),
    #[error("network error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("invalid date: {0}")]
    Date(String),
    #[error("no client id configured")]
    NoClientId,
    #[error("no pending login")]
    NoPendingLogin,
    #[error("secret store: {0}")]
    Store(String),
}
```

- The first four `Display` strings are the Kotlin strings exactly (`p1-behavior.md` section 8.1). They are kept verbatim so that logs match upstream, and because the Kotlin has no i18n keys for them.
- `HTTP error {0}` matches `HttpException` (`HttpException.kt:13`).
- Map `reqwest` timeouts and connection errors to `Transport`. Do not map them to `RefreshFailed` except inside `bearer`.
- Do not panic on bad input. Every JSON field that is optional in the Kotlin is `Option` or `#[serde(default)]` here.

## 7. Integration with the mockup (`/home/blackbox/kurobayomi/src`)

Current state (observed):
- `src/entry.rs:496-537` `tracking()` builds the tracking sheet with a fixed list `[("AniList", true), ("MyAnimeList", false), ("Kitsu", false)]`. The MAL row shows "Add tracking" and no state. The bound row shows fixed values ("Watching", "12 / 24", "8.0", "Start date", "Finish date").
- `src/entry.rs:539-567` `tracker_search()` shows four fake results with the fixed text "Author name" and "Score: 8.4". The "Track" button does nothing.
- The remove dialog `remove_tracking` (in `spec/entries.json`) says "Remove AniList tracking?" and has "Also remove from AniList".
- `src/spec.rs` loads `spec/*.json` for layout text. It does not hold any tracker state.

Changes proposed for this area (kept minimal):
1. Replace the fixed list in `tracking()` with the values from `MalTracker` when it is logged in. Keep the AniList and Kitsu rows as they are (other areas).
2. In `tracker_search()`, call `MalTracker::manga_search` or `anime_search` (the entry type decides) with the query from the search entry. Show the results from `MangaSearchHit` / `AnimeSearchHit`: title, type, status, score, start date. The "Track" button calls `bind` through the flow in section 3.6 and then closes the sheet.
3. In `tracking()`, the MAL row: logged out -> "Log in to MyAnimeList" button that starts the login flow; logged in -> the bound layout with the real status label from `manga_label` / `anime_label`, progress, score (`display_score`), start and finish dates (local dates from epoch).
4. `remove_tracking` dialog for MAL: copy from `spec/entries.json`, change the title to "Remove MyAnimeList tracking?" (the `track_delete_title` format with the tracker name), keep the "Also remove from MyAnimeList" switch, and call `manga_delete` / `anime_delete` only when the switch is on (`AnimeTrackInfoDialog.kt:833-834`).
5. Keep the mockup's `dialog_of` and `open` helpers. Do not add new navigation infrastructure.

Nothing in `src/ui.rs` or `src/screens.rs` needs to change for this area.

## 8. Android API to Linux mapping for this scope

| Android / Kotlin | Linux / Rust | Notes |
|---|---|---|
| `OkHttpClient` (shared, cookie jar, 5 MiB cache, Cloudflare interceptor) | `reqwest::Client` built in `MalApi::new` | Own client: no cookie store, no HTTP cache, `connect_timeout(30s)`, `read_timeout(30s)`, `timeout(120s)` (matches `NetworkHelper.kt:23-25`). Cloudflare handling is area:12-network-stack's concern; MAL does not need it. |
| `Interceptor` (`MyAnimeListInterceptor`) | `MalAuth::bearer` called in `MalApi` request helper | Explicit call, not a middleware. Easier to test. |
| `synchronized(this)` | `tokio::sync::Mutex<AuthState>` | Must be async-aware. |
| `AtomicBoolean`-like preference flag | `AtomicBool` mirrored to the config file | Persisted in the config file (section 6 of the data doc). |
| `Preference.privateKey(...)` | config file keys without prefix + secret store for the token | The `__PRIVATE_` prefix only matters for Android backups. Kurobayomi backups exclude the secret store by design (area:22-backup-restore). |
| `SharedPreferences` + `json.encodeToString(MALOAuth?)` | `serde_json::to_string(&Option<MalToken>)` into the secret store | Store the token only in the secret store. No `"null"` string (D-07). |
| `SecureRandom` | `rand::rngs::OsRng` | |
| `Base64.getUrlEncoder().withoutPadding()` | `base64::engine::general_purpose::URL_SAFE_NO_PAD` | |
| `Uri.Builder.appendQueryParameter` | `url::Url::query_pairs_mut().append_pair` | Order preserved. |
| `SimpleDateFormat("yyyy-MM-dd", Locale.US)` (default zone) | `chrono::Local` + `format("%Y-%m-%d")` and `NaiveDate::parse_from_str` | See section 9.1. |
| `lifecycleScope.launchIO { }` | `RUNTIME.spawn(async { })` | |
| `Context.openInBrowser(url, forceDefaultBrowser = true)` | `gtk::UriLauncher::new(url).launch(...)` with `gio::AppInfo::launch_default_for_uri` as fallback | No "force default browser" needed on Linux, the default handler is used. |
| `Intent` + `TrackLoginActivity` deep link | loopback listener or `GApplication::open` for a custom scheme | Section 3.6. |
| `Toast` | GTK inline `Revealer` label at the bottom of the window (no libadwaita) | Same message text. |
| `logcat(LogPriority.ERROR, e)` | `tracing::error!` | |
| `Color.rgb(46, 81, 162)` | `0x2E51A2` as a CSS colour `#2E51A2` | Used for the logo tile background. |
| `R.drawable.ic_tracker_mal` (`drawable-nodpi/ic_tracker_mal.webp`) | GResource PNG, converted from the webp at build time | A 48 px PNG. Conversion is a build step (area:01-foundation-app-bootstrap). |
| `TrackerManager.myAnimeList = MyAnimeList(1L)` (Injekt) | explicit `MalTracker::new(...)` in app startup | No DI container. |
| `JSON` `ignoreUnknownKeys = true` | `#[serde(default)]` on optional fields, unknown fields ignored by default in serde | serde ignores unknown fields unless `deny_unknown_fields`. |

## 9. Pitfalls

1. Local time: `chrono::Local` reads the process time zone through libc `localtime_r`, which is safe to call from many threads. The `time` crate's `UtcOffset::current_local_offset()` returns an error in multithreaded Unix processes unless an unsafe opt-in is enabled. The exact feature name and behaviour depend on the pinned `time` version, so check its docs before relying on it. Do not use it for MAL dates. Use `chrono::Local` for `yyyy-MM-dd` conversion only. Keep `time` for everything else (area:01-foundation-app-bootstrap).
2. Epoch 0 means "unset" for dates, and it is sent as `start_date=` (empty) so MAL clears the date. Do not omit the field for 0. Omit only on formatting failure, which does not happen in practice.
3. Completion timestamps are `now` in ms, not midnight. Converting them to a date uses the local calendar date of that instant. Store them as the source does.
4. `score` is `f64` locally. Send `wire_score`. Never send `-1` (D-12).
5. `num_chapters_read` truncates. `12.7` is sent as `12`. Keep it so that round-trips match the Kotlin.
6. The token expiry margin is 60 s. If `expires_in < 60`, the margin is clamped in the proposal (D-19). Keep the upstream margin for normal values.
7. The loopback listener port must match the one registered with MAL exactly. A port collision means the paste fallback is the only option.
8. The verifier is secret enough to hold in memory, but a restart in the middle of login must not break the flow. Persist it only until the code is consumed, then clear it (section 6 of the data doc).
9. `reqwest` `.form()` encodes `+` for spaces. MAL accepts this for `q`. If a live test shows otherwise, switch to percent-encoding with `%20`.
10. `keyring` on a system without a Secret Service daemon returns an error on every call. The fallback file is 0600. Do not silently store the token in the config file.
11. The `my:` search with an empty title loads the whole list and then every detail. Keep the Kotlin behaviour but cap the list size shown (the UI shows the first 200 results, proposal).
12. GTK widgets are not `Send`. All UI state lives on the main thread. Pass plain data (`Vec<Hit>`) back through the `JoinHandle`.
13. Do not add libadwaita widgets (`AdwToast`, `AdwMessageDialog`). Use `gtk::AlertDialog` (GTK 4.10+) for confirm dialogs.
