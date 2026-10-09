# Area 32: MangaUpdates + Simkl trackers, part 1 - Rust design

Target: Linux, Rust stable, `gtk4` 0.11 (GTK 4.16+, no libadwaita), `tokio`, `reqwest`, `serde`, `serde_json`, `async-trait`, `thiserror`, `tracing`, `keyring` (Secret Service), `url`, `html-escape`. Layout and trait names follow `plans/27-tracking-core/p1-rust-design.md`; this file adds the two concrete services. Behaviour that these types must reproduce is in `p1-behavior.md` (cited as B-x.y); wire shapes are in `p1-data-and-config.md` (cited as D-x.y).

Contents: 1 layout; 2 crates; 3 identity and models (reused from area 27); 4 MangaUpdates design; 5 Simkl design; 6 OAuth and deep link on Linux; 7 secrets; 8 async model; 9 error handling; 10 mockup integration; 11 Android-to-Linux mapping; 12 pitfalls; 13 open decisions.

---

## 1. Layout (this area's additions)

```
crates/kuro-track/src/trackers/
  mod.rs                         re-exports; registry constructor `all_trackers(ctx)`
  mangaupdates/
    mod.rs                       MangaUpdatesTracker (impl Tracker, MangaTracker, DeletableMangaTracker)
    api.rs                       MuApi: the nine endpoints M1-M9 (D-4.1)
    auth.rs                      MuAuth: token holder + request signing (replaces MangaUpdatesInterceptor)
    dto.rs                       wire structs (D-8)
    search.rs                    MuRecord -> MangaTrackSearch mapping
    scores.rs                    92-entry score table and index mapping (B-2.2)
    status.rs                    list ids, labels, status list order (B-2.1)
  simkl/
    mod.rs                       SimklTracker (impl Tracker, AnimeTracker)
    api.rs                       SimklApi: endpoints S2-S11 (D-4.2)
    auth.rs                      SimklAuth: OAuth token holder + header signing (replaces SimklInterceptor)
    oauth.rs                     authorise URL builder, redirect parsing, code exchange glue
    dto.rs                       wire structs (D-8)
    status.rs                    status ids, labels, to/from wire strings (B-3.1)
    sync.rs                      find, add, update, progress, rating sequences (B-3.5, 3.8)
    search.rs                    SimklSearchResult -> AnimeTrackSearch mapping; tracking_url normalisation (D-SK-2 fix)
    scores.rs                    0..10 integer score list
```

GUI glue for this area lives in the GUI crate (`src/track/...`, per area 27 section 1). This area adds only:
```
src/track/ui/login_mangaupdates.rs   credential dialog for MU (B-2.5 dialog behaviour)
src/track/ui/login_simkl.rs          browser step and paste-code fallback (B-3.7, section 6 below)
```

---

## 2. Crates

| Crate | Used for | Why this one |
|---|---|---|
| `reqwest` (`json`, `rustls-tls`) | HTTP for both services | Shared client from area 12; `.json()` and `.error_for_status()` replace `awaitSuccess()` and `parseAs()` |
| `serde`, `serde_json` (`preserve_order`) | DTOs and request bodies | Key order in request bodies is preserved so payloads are byte-identical to Kotlin (D-7) |
| `async-trait` | object-safe trait methods | Same as area 27 |
| `thiserror` | `TrackError` | Same as area 27 |
| `url` | authorise URL and query building | Replaces `android.net.Uri` (`Uri.Builder` in SimklApi.kt:144-148, 185-187) |
| `html-escape` | `htmlDecode` for MU title and description (B-2.14) | Decodes entities; a tag stripper (`ammonia` with no allowed tags) removes markup first |
| `keyring` (Secret Service) | MU session token, SK access token (section 7) | libsecret on KDE and GNOME |
| `tracing` | logs (`logcat` in the Kotlin) | Same level mapping as area 27 section 9 |
| `time` | `date_from` is opaque text (no parsing); dates in models are epoch millis | Not used for SK `date_from`; `std::time::SystemTime` for `now` in generic flow |

Not used: `chrono`, `once_cell` (use `std::sync::LazyLock`), any OAuth crate (the flow is a single code exchange, no PKCE in the source).

---

## 3. Identity and models

Reuse exactly the types from area 27 section 3 (`TrackerId`, `AnimeTrack`, `MangaTrack`, `AnimeTrackSearch`, `MangaTrackSearch`, `ScoreList`, `LogoRef`). Two deliberate differences for these services:

1. `AnimeTrackSearch.score: Option<f64>` (None replaces the -1.0 sentinel, area 27 3.3). The MU and SK search mapping sets `score: None`. Bind then sets it to `Some(0.0)` or the remote value.
2. `remote_url` is `String` and defaults to `""`, never null (`remote_url TEXT NOT NULL`, D-5.1).

```rust
// kuro-track/src/trackers/mangaupdates/mod.rs (signature excerpt; full impl in section 4)
pub struct MangaUpdatesTracker { /* section 4.1 */ }
```

---

## 4. MangaUpdates design

### 4.1 Types

```rust
// kuro-track/src/trackers/mangaupdates/status.rs
pub const READING_LIST: i64 = 0;
pub const WISH_LIST: i64 = 1;
pub const COMPLETE_LIST: i64 = 2;
pub const UNFINISHED_LIST: i64 = 3;
pub const ON_HOLD_LIST: i64 = 4;
pub const REREADING: i64 = -1;     // never sent (B-2.1)

/// Order shown in the status picker (B-2.1).
pub const STATUS_ORDER: [i64; 5] = [READING_LIST, COMPLETE_LIST, ON_HOLD_LIST, UNFINISHED_LIST, WISH_LIST];

pub fn status_label_key(status: i64) -> Option<&'static str> {
    match status {
        READING_LIST => Some("reading_list"),
        WISH_LIST => Some("wish_list"),
        COMPLETE_LIST => Some("complete_list"),
        ON_HOLD_LIST => Some("on_hold_list"),
        UNFINISHED_LIST => Some("unfinished_list"),
        _ => None,
    }
}
```

```rust
// kuro-track/src/trackers/mangaupdates/scores.rs
use std::sync::LazyLock;

/// 92 entries: "-", "1.0".."1.9", ..., "9.9", "10.0" (B-2.2).
pub static SCORE_LIST: LazyLock<Vec<&'static str>> = LazyLock::new(|| { /* build exactly as B-2.2 */ Vec::new() });

/// Replaces `indexToScore` (MangaUpdates.kt:71). Returns Err for out-of-range index (fixes D-MU-7).
pub fn index_to_score(index: usize) -> Result<f64, TrackError> {
    if index == 0 { return Ok(0.0); }
    let s = SCORE_LIST.get(index).ok_or(TrackError::ScoreOutOfRange(index))?;
    Ok(s.parse::<f64>().expect("static table parses"))
}

/// Display text (B-2.2 PROPOSAL: "-" for 0.0 to match the picker, fixes D-MU-8).
pub fn display_score(score: f64) -> String {
    if score == 0.0 { "-".to_owned() } else { format!("{score:?}") } // see note
}
```
Note on `display_score`: Kotlin prints `Double.toString()` (`7.0`, `7.5`, `10.0`). Rust `format!("{score:?}")` for `f64` gives the same shortest round-trip form for these values (`7.0`, `7.5`, `10.0`). The unit test in section 12 pins this.

```rust
// kuro-track/src/trackers/mangaupdates/dto.rs : see D-8 for the field list.
// Request bodies are built with serde_json::json! into Value, inserting keys in the documented order.
```

```rust
// kuro-track/src/trackers/mangaupdates/auth.rs
pub struct MuAuth {
    token: tokio::sync::RwLock<Option<String>>,   // None = not authenticated (B-2.6)
    user_agent: String,                           // "Kurobayomi v<ver> (<app id>)" (see 4.6)
}

impl MuAuth {
    pub fn new(initial: Option<String>, user_agent: String) -> Self;
    /// Replaces newAuth(token) (MangaUpdatesInterceptor.kt:28-30).
    pub async fn set(&self, token: Option<String>);
    /// Returns the bearer or the Kotlin's error (MangaUpdatesInterceptor.kt:17).
    pub async fn bearer(&self) -> Result<String, TrackError>;   // TrackError::NotAuthenticated("MangaUpdates")
}
```

```rust
// kuro-track/src/trackers/mangaupdates/api.rs
pub struct MuApi {
    http: reqwest::Client,        // shared client from area 12 (no auth)
    auth: std::sync::Arc<MuAuth>,
}

pub const BASE_URL: &str = "https://api.mangaupdates.com";
pub const CONTENT_TYPE: &str = "application/vnd.api+json";

impl MuApi {
    /// M1 + M2 (B-2.7). Returns (list item, rating or None). Rating failure is an Err of its own (fix D-MU-3).
    pub async fn get_series_list_item(&self, remote_id: i64) -> Result<(MuListItem, RatingFetch), TrackError>;
    /// M3 (B-2.9). Returns the HTTP status code (only 200 is used by the caller).
    pub async fn add_series_to_list(&self, remote_id: i64, list_id: i64) -> Result<u16, TrackError>;
    /// M4 then rating (B-2.10).
    pub async fn update_series_list_item(&self, t: &MangaTrack) -> Result<(), TrackError>;
    /// M7 (B-2.12).
    pub async fn delete_series_from_list(&self, remote_id: i64) -> Result<(), TrackError>;
    /// M8 (B-2.14).
    pub async fn search(&self, query: &str) -> Result<Vec<MuRecord>, TrackError>;
    /// M9 (B-2.5). Returns the context; the caller stores uid and session_token.
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<MuContext, TrackError>;
}

/// Tri-state result of M2 (fix for D-MU-3). Kotlin collapsed Unknown and None into 0.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RatingFetch {
    Known(Option<f64>),   // Some(v) = remote rating, None = no rating on server
    Unknown,              // the GET failed; do not send rating requests based on it
}
```

```rust
// kuro-track/src/trackers/mangaupdates/mod.rs
pub struct MangaUpdatesTracker {
    id: TrackerId,                       // TrackerId::MANGAUPDATES (7)
    creds: std::sync::Arc<CredentialStore>,   // section 7; holds uid (username) and session token (password)
    api: MuApi,
    auth: std::sync::Arc<MuAuth>,
    login_state: tokio::sync::watch::Sender<bool>,
    scores: ScoreList,                   // built once from scores::SCORE_LIST (92 entries)
}

#[async_trait::async_trait]
impl Tracker for MangaUpdatesTracker {
    fn id(&self) -> TrackerId { self.id }
    fn name(&self) -> &str { "MangaUpdates" }
    fn logo(&self) -> LogoRef { LogoRef::Embedded(include_bytes!("../../../assets/ic_manga_updates.svg")) }
    fn logo_color(&self) -> u32 { 0xFF92A0AD }          // Color.rgb(146,160,173) (B-2.1)
    fn completion_status(&self) -> i64 { COMPLETE_LIST }
    fn score_list(&self) -> &ScoreList { &self.scores }     // struct field built once from scores::SCORE_LIST
    async fn login(&self, username: &str, password: &str) -> Result<(), TrackError>;  // B-2.5
    fn logout(&self);                                                                 // B-1.2 + fix D-MU-1
    fn is_logged_in(&self) -> bool;
    fn login_state(&self) -> tokio::sync::watch::Receiver<bool> { self.login_state.subscribe() }
    fn save_credentials(&self, username: &str, password: &str);
    fn username(&self) -> String;
    fn password(&self) -> String;
    fn as_manga(&self) -> Option<&dyn MangaTracker> { Some(self) }
    fn as_deletable_manga(&self) -> Option<&dyn DeletableMangaTracker> { Some(self) }
}
```

Note: the `score_list` signature returns a reference; store the 92-element `ScoreList` in the struct (built once) rather than a LazyLock returning a reference to a temporary. Pitfall section 12.

```rust
#[async_trait::async_trait]
impl MangaTracker for MangaUpdatesTracker {
    fn status_list(&self) -> Vec<i64> { STATUS_ORDER.to_vec() }
    fn reading_status(&self) -> i64 { READING_LIST }
    fn rereading_status(&self) -> i64 { REREADING }
    fn status_label(&self, s: i64) -> Option<&'static str> { status_label_key(s) }
    fn index_to_score(&self, i: usize) -> f64 { index_to_score(i).unwrap_or(0.0) }   // see note
    fn display_score(&self, t: &MangaTrack) -> String { display_score(t.score) }
    async fn update(&self, t: &MangaTrack, did_read: bool) -> Result<MangaTrack, TrackError>;  // B-2.3, 2.10
    async fn bind(&self, t: &mut MangaTrack, has_read: bool) -> Result<(), TrackError>;        // B-2.7
    async fn search(&self, q: &str) -> Result<Vec<MangaTrackSearch>, TrackError>;               // B-2.14
    async fn refresh(&self, t: &MangaTrack) -> Result<MangaTrack, TrackError>;                  // B-2.13
}
```
Note on `index_to_score`: the trait method returns `f64` (area 27 signature). The MU module returns `Result` internally; the trait wrapper must not panic, so out-of-range returns `0.0` in the trait method. The generic `setRemoteScore` path must instead reject unknown strings before calling this (fix D-MU-7), so the trait wrapper is never reached with a bad index in practice. PROPOSAL to avoid silent 0.0: change the trait to `fn index_to_score(&self, i: usize) -> Result<f64, TrackError>` in area 27 and make this area follow. Recorded in section 13 as open decision O-2.

```rust
#[async_trait::async_trait]
impl DeletableMangaTracker for MangaUpdatesTracker {
    async fn delete(&self, t: &MangaTrack) -> Result<(), TrackError> {
        self.api.delete_series_from_list(t.remote_id).await   // B-2.12; sends [remote_id]
    }
}
```

### 4.2 `update` (B-2.3, 2.10)
```rust
async fn update(&self, t: &MangaTrack, did_read: bool) -> Result<MangaTrack, TrackError> {
    let mut t = t.clone();
    if t.status != COMPLETE_LIST && did_read { t.status = READING_LIST; }   // B-2.3
    // rating decision is inside update_series_list_item (section 4.4)
    self.api.update_series_list_item(&t).await?;
    Ok(t)
}
```

### 4.3 `bind` (B-2.7)
```rust
async fn bind(&self, t: &mut MangaTrack, has_read: bool) -> Result<(), TrackError> {
    match self.api.get_series_list_item(t.remote_id).await {
        Ok((item, rating)) => {                    // copyFrom (MangaUpdates.kt:110-113)
            apply_list_item(t, &item);             // status = list_id ?? READING; last_chapter_read = chapter ?? 0
            t.score = match rating {
                RatingFetch::Known(v) => v.unwrap_or(0.0),
                RatingFetch::Unknown => t.score,   // keep previous value (fix D-MU-3)
            };
            Ok(())
        }
        Err(TrackError::Http { status: 404, .. }) => {       // fix D-MU-2: only "not on list" means add
            t.score = 0.0;
            self.add_path(t, has_read).await
        }
        Err(e) => Err(e),                                    // other errors propagate, no add
    }
}

async fn add_path(&self, t: &mut MangaTrack, has_read: bool) -> Result<(), TrackError> {
    let list = if has_read { READING_LIST } else { WISH_LIST };
    let code = self.api.add_series_to_list(t.remote_id, list).await?;
    if code == 200 {   // keep 200-only status write (B-2.9); fix D-MU-4 for last_chapter_read
        t.status = list;
        if has_read { t.last_chapter_read = 1.0; }
    }
    Ok(())
}
```
Note: the 404 mapping is a deliberate change from the Kotlin `catch (Exception)` (defect D-MU-2). Verify on the live API that "not on list" returns 404; if it returns 200 with an empty body, the parse error path must be treated the same way (a `TrackError::Parse` for an empty list item means "not on list"). Recorded as open decision O-1.

`apply_list_item`:
```rust
fn apply_list_item(t: &mut MangaTrack, item: &MuListItem) {
    t.status = item.list_id.unwrap_or(READING_LIST);                       // MUListItem.copyTo
    t.last_chapter_read = item.status.as_ref().and_then(|s| s.chapter).map(f64::from).unwrap_or(0.0);
}
```

### 4.4 Rating and list update (B-2.10)
```rust
pub async fn update_series_list_item(&self, t: &MangaTrack) -> Result<(), TrackError> {
    let body = json!([{
        "series": { "id": t.remote_id },
        "list_id": t.status,
        "status": { "chapter": t.last_chapter_read.trunc() as i64 }   // Kotlin toInt() truncation
    }]);
    self.post("/v1/lists/series/update", &body).await?;              // M4, awaitSuccess semantics
    self.sync_rating(t).await                                         // section below
}

async fn sync_rating(&self, t: &MangaTrack) -> Result<(), TrackError> {
    if t.score < 0.0 { return Ok(()); }                               // B-2.10 first guard
    if t.score != 0.0 {
        self.put_json(&format!("/v1/series/{}/rating", t.remote_id), &json!({"rating": t.score})).await
    } else {
        // Keep the Kotlin behaviour (DELETE on 0.0) but only when the user really cleared the score.
        // Fix D-MU-3: the caller passes whether the score was changed by the user (see note).
        self.delete(&format!("/v1/series/{}/rating", t.remote_id)).await
    }
}
```
Note on `sync_rating` and D-MU-3: the Kotlin sends DELETE whenever `score == 0.0`, which deletes a server rating after a failed fetch. The Rust port holds the last-known remote rating in the track (`remote_score_known: Option<Option<f64>>` on the row or in memory). PROPOSAL: call DELETE only when the last known remote rating is `Some(_)` or unknown-but-user-cleared (a `ScoreChanged` flag set by the UI). When the remote rating is known to be absent, skip the DELETE. This keeps the wire sequence identical in all normal cases and removes the data-loss path. Recorded as open decision O-3 because it adds state to the row.

`post`/`put_json`/`delete` helpers: return `TrackError::Http { status, body_snippet }` on non-2xx (section 9).

### 4.5 Search (B-2.14)
```rust
pub async fn search(&self, query: &str) -> Result<Vec<MuRecord>, TrackError> {
    let body = json!({"search": query, "filter_types": ["drama cd", "novel"]});
    let resp = self.http
        .post(format!("{BASE_URL}/v1/series/search"))
        .header(reqwest::header::CONTENT_TYPE, CONTENT_TYPE)
        .body(body.to_string())
        .send().await?;                           // Network error via From<reqwest::Error>
    let res: MuSearchResult = read_json(resp).await?; // non-2xx => TrackError::Http, body parse => Parse (section 9)
    Ok(res.results.into_iter().map(|i| i.record).collect())
}
```
Mapping `MuRecord -> MangaTrackSearch` (`search.rs`): as B-2.14, with `publishing_type = kind.unwrap_or_default()`, `start_date = year.unwrap_or_default()` (fix D-MU-5 instead of the `"null"` strings), `remote_id = series_id.unwrap_or(0)`, `title = html_decode(title)`, `summary = html_decode(description)`, `cover_url = image.url.original or ""`, `tracking_url = url or ""`, `total_chapters = 0`, `publishing_status = ""`, `score = None`.

`html_decode(s)`: strip tags with `ammonia::Builder::new().tags(HashSet::new()).clean(s).to_string()`, then `html_escape::decode_html_entities`. PROPOSAL note on parity: Kotlin uses the platform parser; edge differences (malformed markup) are listed in the test doc, E-MU-14.

### 4.6 Login and session (B-2.5, 2.6)
```rust
async fn login(&self, username: &str, password: &str) -> Result<(), TrackError> {
    let ctx = self.api.authenticate(username, password).await?;   // M9
    self.save_credentials(&ctx.uid.to_string(), &ctx.session_token);   // username = uid, password = token (B-2.5 step 3)
    self.auth.set(Some(ctx.session_token.clone())).await;
    let _ = self.login_state.send(true);
    Ok(())
}
```
User-Agent: `Kurobayomi v{CARGO_PKG_VERSION} ({APP_ID})` where `APP_ID` is the Linux app id from area 01 (for example `io.github.kurobayomi`; the value is a constant in the bootstrap crate). Replaces the Kotlin `Aniyomi v0.18.2.1 (xyz.jmir.tachiyomi.mi)` string.

### 4.7 Logout (fix D-MU-1)
```rust
fn logout(&self) {
    self.save_credentials("", "");          // BaseTracker.logout (B-1.2)
    tokio::runtime::Handle::current().block_on(self.auth.set(None));  // NOT on the GTK thread; see below
    let _ = self.login_state.send(false);
}
```
Pitfall: `logout` is sync in the trait; do not `block_on` on the GTK main thread. Replace the body with `self.auth.clear_blocking()` built on a `std::sync::RwLock` (not tokio) so it is callable from any thread. PROPOSAL used in the final design: `MuAuth.token: std::sync::RwLock<Option<String>>` (reads are short and never await). Same for Simkl.

---

## 5. Simkl design

### 5.1 Types

```rust
// kuro-track/src/trackers/simkl/status.rs
pub const WATCHING: i64 = 1;
pub const COMPLETED: i64 = 2;
pub const ON_HOLD: i64 = 3;
pub const NOT_INTERESTING: i64 = 4;
pub const PLAN_TO_WATCH: i64 = 5;
pub const REWATCHING: i64 = 0;   // not in list; toWire returns Err

pub const STATUS_ORDER: [i64; 5] = [WATCHING, COMPLETED, ON_HOLD, NOT_INTERESTING, PLAN_TO_WATCH];

pub fn to_wire(status: i64) -> Result<&'static str, TrackError> {   // SimklUtils.kt:5-12
    Ok(match status {
        WATCHING => "watching",
        COMPLETED => "completed",
        ON_HOLD => "hold",
        NOT_INTERESTING => "notinteresting",
        PLAN_TO_WATCH => "plantowatch",
        other => return Err(TrackError::UnknownStatus(other)),        // fix D-SK-7: typed error
    })
}

pub fn from_wire(s: &str) -> Result<i64, TrackError> {               // SimklUtils.kt:14-21
    Ok(match s {
        "watching" => WATCHING,
        "completed" => COMPLETED,
        "hold" => ON_HOLD,
        "dropped" | "notinteresting" => NOT_INTERESTING,
        "plantowatch" => PLAN_TO_WATCH,
        other => return Err(TrackError::UnknownWireStatus(other.to_owned())),
    })
}

pub fn label_key(status: i64) -> Option<&'static str> {              // Simkl.kt:105-112
    match status {
        WATCHING => Some("watching"),          // AYMR
        PLAN_TO_WATCH => Some("plan_to_watch"),// AYMR
        COMPLETED => Some("completed"),        // MR
        ON_HOLD => Some("on_hold"),            // MR
        NOT_INTERESTING => Some("not_interesting"), // AYMR
        _ => None,
    }
}
```

```rust
// kuro-track/src/trackers/simkl/scores.rs
pub const SCORE_LIST: [&str; 11] = ["0","1","2","3","4","5","6","7","8","9","10"];   // Simkl.kt:29-31
pub fn display_score(score: f64) -> String { (score.trunc() as i64).to_string() }    // Simkl.kt:42-44
```

```rust
// kuro-track/src/trackers/simkl/auth.rs
pub struct SimklAuth {
    token: std::sync::RwLock<Option<SimklOAuth>>,   // restored from the secret store at start (section 7)
    client_id: &'static str,
    user_agent: String,
}
impl SimklAuth {
    pub fn bearer(&self) -> Result<String, TrackError>;   // TrackError::NotAuthenticated("Simkl") (SimklInterceptor.kt:19)
    pub fn set(&self, t: Option<SimklOAuth>);             // newAuth (SimklInterceptor.kt:31-34), persistence done by the caller
}
```

```rust
// kuro-track/src/trackers/simkl/api.rs
pub const CLIENT_ID: &str = "aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e";
const CLIENT_SECRET: &str = "2bec9c1d0c00a1e9b0e9e096a71f88d555a6f52da7923df07906df3b21351783";
pub const BASE_URL: &str = "https://simkl.com";
pub const API_URL: &str = "https://api.simkl.com";
pub const POSTERS_URL: &str = "https://simkl.in/posters/";
pub const REDIRECT_URL: &str = "aniyomi://simkl-auth";     // see section 6 for the Linux handling

pub struct SimklApi {
    http: reqwest::Client,
    auth: std::sync::Arc<SimklAuth>,
}

impl SimklApi {
    pub async fn access_token(&self, code: &str) -> Result<SimklOAuth, TrackError>;     // S2
    pub async fn current_user_id(&self) -> Result<i32, TrackError>;                    // S3 (async, fix D-SK-9)
    pub async fn find_watched(&self, remote_id: i64) -> Result<Option<SimklSyncWatched>, TrackError>; // S4
    pub async fn all_items(&self, query_type: &str, status: &str, date_from: &str) -> Result<SimklSyncResult, TrackError>; // S5
    pub async fn add_to_list(&self, media: MediaKind, remote_id: i64, to: &str) -> Result<(), TrackError>; // S6
    pub async fn history_remove(&self, remote_id: i64) -> Result<(), TrackError>;      // S7
    pub async fn history_add(&self, remote_id: i64, last_episode: u32) -> Result<(), TrackError>;        // S8
    pub async fn rating_set(&self, media: MediaKind, remote_id: i64, rating: i32) -> Result<(), TrackError>;  // S9
    pub async fn rating_remove(&self, media: MediaKind, remote_id: i64) -> Result<(), TrackError>;     // S10
    pub async fn search(&self, kind: SearchKind, query: &str) -> Result<Vec<SimklSearchResult>, TrackError>; // S11
}

/// Body key used by S6, S9, S10. Kotlin: `if (type == "movies") "movies" else "shows"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind { Shows, Movies }
impl MediaKind {
    pub fn body_key(self) -> &'static str { match self { Self::Shows => "shows", Self::Movies => "movies" } }
}

/// Search type path segment (SimklApi.kt:142-156): "anime", "tv", "movie" (singular for movie).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind { Anime, Tv, Movie }
impl SearchKind {
    pub fn path(self) -> &'static str { match self { Self::Anime => "anime", Self::Tv => "tv", Self::Movie => "movie" } }
}
```

```rust
// kuro-track/src/trackers/simkl/mod.rs
pub struct SimklTracker {
    id: TrackerId,                  // TrackerId::SIMKL (101)
    creds: std::sync::Arc<CredentialStore>,
    api: SimklApi,
    auth: std::sync::Arc<SimklAuth>,
    login_state: tokio::sync::watch::Sender<bool>,
    scores: ScoreList,              // built once from SCORE_LIST
}

#[async_trait::async_trait]
impl Tracker for SimklTracker {
    fn id(&self) -> TrackerId { self.id }
    fn name(&self) -> &str { "Simkl" }
    fn logo(&self) -> LogoRef { LogoRef::Embedded(include_bytes!("../../../assets/ic_tracker_simkl.svg")) }
    fn logo_color(&self) -> u32 { 0xFF000000 }      // Color.rgb(0,0,0) (Simkl.kt:99)
    fn completion_status(&self) -> i64 { COMPLETED }
    fn score_list(&self) -> &ScoreList { &self.scores }
    async fn login(&self, _username: &str, password: &str) -> Result<(), TrackError> { self.login_code(password).await }  // Simkl.kt:120
    fn logout(&self);                                                                    // section 5.6
    /* is_logged_in, login_state, save_credentials, username, password: as area 27 BaseTracker */
    fn as_anime(&self) -> Option<&dyn AnimeTracker> { Some(self) }
}

#[async_trait::async_trait]
impl AnimeTracker for SimklTracker {
    fn status_list(&self) -> Vec<i64> { STATUS_ORDER.to_vec() }
    fn watching_status(&self) -> i64 { WATCHING }
    fn rewatching_status(&self) -> i64 { REWATCHING }
    fn status_label(&self, s: i64) -> Option<&'static str> { label_key(s) }
    fn display_score(&self, t: &AnimeTrack) -> String { display_score(t.score) }
    async fn update(&self, t: &AnimeTrack, did_watch: bool) -> Result<AnimeTrack, TrackError>;  // 5.3
    async fn bind(&self, t: &mut AnimeTrack, has_seen: bool) -> Result<(), TrackError>;        // 5.4
    async fn search(&self, q: &str) -> Result<Vec<AnimeTrackSearch>, TrackError>;               // 5.7
    async fn refresh(&self, t: &AnimeTrack) -> Result<AnimeTrack, TrackError>;                  // 5.8
}
```

### 5.2 Track-type normalisation (fix D-SK-2)
Kotlin derives the type from `tracking_url` with `substringAfter("/").substringBefore("/")`, which yields `""` for an absolute URL. The port parses it explicitly:

```rust
/// Path type of a stored tracking_url: "/tv/123" -> Some("tv"), "/movies/9" -> Some("movies").
/// Absolute search URLs are normalised at search time (section 5.7), so this sees the stored form.
pub fn path_type(tracking_url: &str) -> Option<&str> {
    let rest = tracking_url.strip_prefix('/')?;          // Kotlin: substringAfter("/") then substringBefore("/")
    Some(rest.split('/').next().unwrap_or(""))           // "" => None (absent)
}

/// Build the stored form from an absolute Simkl URL: "https://simkl.com/anime/1/x" -> "/anime/<id>".
/// The id comes from ids.simkl_id, not from the URL (the URL slug is not needed).
/// Rule (section 5.7): first path segment of the URL, with "movie" mapped to "movies";
/// if no known segment is present, use `fallback` (the search kind's path, see SearchKind::path).
pub fn normalised_tracking_url(absolute: &str, simkl_id: i64, fallback: &str) -> String {
    let seg = url::Url::parse(absolute)
        .ok()
        .and_then(|u| u.path_segments().and_then(|mut s| s.next().map(str::to_owned)))
        .filter(|s| matches!(s.as_str(), "anime" | "tv" | "movies" | "movie"));
    let seg = match seg.as_deref() {
        Some("movie") => "movies".to_owned(),
        Some(s) => s.to_owned(),
        None => fallback.to_owned(),
    };
    format!("/{seg}/{simkl_id}")
}
```

Media kind rule: `MediaKind::Movies` iff `path_type == Some("movies")` or `"movie"` (the Kotlin only checks `"movies"`; the search type `movie` must normalise to `movies` when the stored form is built, so the stored path is `/movies/<id>` for movie results). PROPOSAL recorded as open decision O-4 (the server path for movies is `movies` in the stored form; confirm live).

### 5.3 `update` (B-3.3, 3.8)
```rust
async fn update(&self, t: &AnimeTrack, did_watch: bool) -> Result<AnimeTrack, TrackError> {
    let mut t = t.clone();
    if t.status != COMPLETED && did_watch {                               // B-3.3
        t.status = if t.total_episodes > 0 && t.last_episode_seen as i64 == t.total_episodes {
            COMPLETED
        } else {
            WATCHING
        };
    }
    self.sync_library(&t).await?;     // progress (shows only), add-to-list, rating
    Ok(t)
}
```
Note: `t.last_episode_seen as i64` truncates toward zero, matching Kotlin `toLong()`. Keep the cast; do not use `round`.

`sync_library` (B-3.8):
```rust
async fn sync_library(&self, t: &AnimeTrack) -> Result<(), TrackError> {
    let kind = media_kind(t);                         // "movies" => Movies else Shows (B-3.8 step 1)
    if kind == MediaKind::Shows {
        self.api.history_remove(t.remote_id).await?;              // S7
        self.api.history_add(t.remote_id, t.last_episode_seen.trunc() as u32).await?;   // S8
    }
    self.api.add_to_list(kind, t.remote_id, to_wire(t.status)?).await?;   // S6, Err before any request if status unknown
    if t.score == 0.0 {
        self.api.rating_remove(kind, t.remote_id).await?;         // S10 (B-3.8.3)
    } else {
        self.api.rating_set(kind, t.remote_id, t.score.trunc() as i32).await?;   // S9
    }
    Ok(())
}
```
Note: `to_wire(t.status)?` is evaluated after `history_*` calls in this order; the Kotlin also calls `toSimklStatus()` inside `addToList` after the progress calls. To keep identical side effects on an unknown status, validate first: `let wire = to_wire(t.status)?;` at the top of `sync_library` (PROPOSAL, it changes only the error ordering, no request is sent for an invalid status).

Progress order note (defect D-SK-5): Kotlin order is remove, then add. The port keeps this order by default (O-5 decides whether to switch to add-first).

### 5.4 `bind` (B-3.4)
```rust
async fn bind(&self, t: &mut AnimeTrack, has_seen: bool) -> Result<(), TrackError> {
    match self.find_lib_anime(t).await? {
        Some(remote) => {
            copy_personal(t, &remote);                                // last_episode_seen, score, status, dates, private
            // library_id: keep local when remote has none (fix D-SK-3)
            if remote.library_id.is_some() { t.library_id = remote.library_id; }
            if t.status != COMPLETED && has_seen { t.status = WATCHING; }   // B-3.4 step 2c
            let updated = self.update(t, false).await?;   // did_watch = false, so no status recompute
            *t = updated;
            Ok(())
        }
        None => {
            t.status = if has_seen { WATCHING } else { PLAN_TO_WATCH };
            t.score = 0.0;
            self.add(t).await                             // B-3.9
        }
    }
}
```
Note: the Kotlin sets `track.status = if (hasSeenEpisodes) WATCHING else track.status`. The port's `if t.status != COMPLETED && has_seen` gives the same result because the `else` branch leaves the status unchanged.

### 5.5 `find_lib_anime` (B-3.5, fix D-SK-1)
```rust
async fn find_lib_anime(&self, t: &AnimeTrack) -> Result<Option<AnimeTrack>, TrackError> {
    let Some(watched) = self.api.find_watched(t.remote_id).await? else { return Ok(None) };   // S4
    if watched.result != Some(true) { return Ok(None); }
    let (Some(last_watched), Some(list)) = (watched.last_watched, watched.list) else { return Ok(None) };
    let url_type = path_type(&t.tracking_url).unwrap_or("");          // "tv" | "anime" | "movies" | ""
    // URL path segment: tv -> shows (B-3.5 step 4). Response key: the same name the response uses.
    let (path_seg, response_key, is_movie) = match url_type {
        "tv" => ("shows", "tv", false),
        "movies" => ("movies", "movies", true),
        "anime" => ("anime", "anime", false),
        other => return Err(TrackError::UnknownMediaType(other.to_owned())),  // fix D-SK-2 consequence: never build "/all-items//"
    };
    let all = self.api.all_items(path_seg, &list, &last_watched).await?;        // S5, date_from raw
    let items = match response_key { "tv" => all.tv, "movies" => all.movies, _ => all.anime };
    let want = if is_movie { "movie" } else { "show" };
    let Some(item) = items.unwrap_or_default().into_iter()
        .find(|i| i.get_from_type(want).map(|r| r.ids.simkl) == Some(t.remote_id)) else { return Ok(None) };
    // Type segment for the stored tracking_url is the response key ("tv", "anime", "movies"), as in Kotlin.
    item.to_track(want, response_key, &list)   // B-3.6
        .map(Some)
}
```
Fix explained: the Kotlin passes the URL path segment (`shows`) to the response parser (`getFromType("shows")`) which throws. The port keys the parse on the response key (`tv`), which is the only key the parser needs.

Note on `url_type_for_response`: the stored `tracking_url` after bind uses the type segment: `tv`, `anime`, `movies`. Use `response_key` as the type segment in `to_track` so the rewritten URL is `/tv/<id>`, `/anime/<id>` or `/movies/<id>`. (Kotlin passes the same `type` value, which is `tv` for TV.)

`get_from_type` for `SimklSyncItem`: `"show" -> show.ok_or(MissingField)`, `"movie" -> movie.ok_or(MissingField)` (replaces `!!`). `SimklSyncResult` has no `get_from_type` in the port (the response key is chosen directly), which removes the `Unknown type: shows` exception.

`to_track` (B-3.6): as the Kotlin, with `Option` in place of `!!`:
- show: `total_episodes = total_episodes_count.ok_or(MissingField)?`, `last_episode_seen = watched_episodes_count.ok_or(MissingField)?`.
- movie: `total_episodes = 1`, `last_episode_seen = if status == "completed" {1.0} else {0.0}`.
- `score = user_rating.map(f64::from).unwrap_or(0.0)`, `status = from_wire(list)?`, `title = result.title`, `remote_id = result.ids.simkl`, `tracking_url = format!("/{type}/{remote_id}")`, `library_id = None`.

### 5.6 Logout (B-3.12, fix D-SK-4)
```rust
fn logout(&self) {
    self.creds.clear(TrackerId::SIMKL);      // username, password, token (delete, do not write "null")
    self.auth.set(None);                      // in-memory (Simkl.kt:148)
    let _ = self.login_state.send(false);
}
```

### 5.7 Search (B-3.14, 3.15)
```rust
async fn search(&self, q: &str) -> Result<Vec<AnimeTrackSearch>, TrackError> {
    let mut out = Vec::new();
    for kind in [SearchKind::Anime, SearchKind::Tv, SearchKind::Movie] {   // sequential, same order (B-3.14)
        for r in self.api.search(kind, q).await? {
            out.push(search_result_to_track(r, kind));
        }
    }
    Ok(out)
}
```
`search_result_to_track` (`search.rs`), B-3.14 field by field, with two intentional changes:
- `title = title_romaji.or(title).unwrap_or_default()` (fix D-SK-8: no panic; an empty title is skipped by the UI).
- `tracking_url = normalised_tracking_url(&r.url, r.ids.simkl_id, kind.path())` (fix D-SK-2). The absolute URL is kept in a display-only field `source_url` on the search model so the "open in browser" action still works (B-1.4: `openInBrowser(tracking_url)`). Note for the UI: the open action must use `source_url`, since `tracking_url` is no longer absolute for SK search results.
- `score = None`, `cover_url = poster.map(|p| format!("{POSTERS_URL}{p}_m.webp")).unwrap_or_default()`, `summary = all_titles.map(|v| format!("All titles:\n{}", v.join("\n"))).unwrap_or_default()`, `publishing_status = status.unwrap_or("ended")`, `publishing_type = kind.unwrap_or(fallback)`, `start_date = year.map(|y| y.to_string()).unwrap_or_default()`, `total_episodes = ep_count.unwrap_or(1)`.

Note the `"All titles:\n"` prefix is kept even when the list is empty (`Some(vec![])` gives `"All titles:\n"`), as the Kotlin.

`normalised_tracking_url` rule: take the first path segment after the host (`anime`, `tv`, `movies`, `movie`); map `movie` to `movies`; return `format!("/{seg}/{simkl_id}")`; if the URL has no such segment, return `format!("/{fallback}/{simkl_id}")` where `fallback` is the search kind (`anime`, `tv`, `movies` for Movie). Only the search-derived value changes; the Kotlin stored value after bind (`/tv/<id>`) is unchanged.

### 5.8 Refresh (B-3.15)
```rust
async fn refresh(&self, t: &AnimeTrack) -> Result<AnimeTrack, TrackError> {
    let mut t = t.clone();
    if let Some(remote) = self.find_lib_anime(&t).await? {
        copy_personal(&mut t, &remote);
        t.total_episodes = remote.total_episodes;
    }
    Ok(t)
}
```

### 5.9 Add (B-3.9)
```rust
async fn add(&self, t: &mut AnimeTrack) -> Result<(), TrackError> {
    let kind = media_kind(t);                                   // see 5.2 (absolute-URL bug fixed by normalisation)
    self.api.add_to_list(kind, t.remote_id, to_wire(t.status)?).await   // S6 only; no progress, no rating
}
```

### 5.10 Login via code (B-3.7)
```rust
pub async fn login_code(&self, code: &str) -> Result<(), TrackError> {
    let outcome = async {
        let oauth = self.api.access_token(code).await?;        // S2
        self.auth.set(Some(oauth.clone()));                     // in-memory
        self.creds.store_simkl_token(&oauth).await?;            // section 7 (secret store)
        let uid = self.api.current_user_id().await?;            // S3
        self.save_credentials(&uid.to_string(), &oauth.access_token);
        Ok::<(), TrackError>(())
    }.await;
    match outcome {
        Ok(()) => { let _ = self.login_state.send(true); Ok(()) }
        Err(e) => { self.logout(); Err(e) }                      // B-3.7: any error => logout (Kotlin swallows; port returns the error for the UI log only)
    }
}
```
Difference: the Kotlin `login(code)` swallows the error (`catch Throwable { logout() }`) and returns normally. The port returns `Err` so the caller can log it, but the UI does not show a toast (matches the Kotlin). Recorded in the edge test list.

---

## 6. OAuth and deep link on Linux

Kotlin flow: browser opens `https://simkl.com/oauth/authorize?response_type=code&client_id=...&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth`; the redirect `aniyomi://simkl-auth?code=...` is delivered to `TrackLoginActivity` by the Android intent filter (AndroidManifest.xml).

Linux design (`src/track/ui/login_simkl.rs` plus `kuro-track::simkl::oauth`):
1. `authorize_url() -> url::Url` builds the same URL as `SimklApi.authUrl()` (params in the same order: `response_type`, `client_id`, `redirect_uri`). `url` encodes `aniyomi://simkl-auth` as `aniyomi%3A%2F%2Fsimkl-auth`, identical to Android `Uri` encoding.
2. Open the URL with `gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>)` (replaces `openInBrowser(..., forceDefaultBrowser = true)`; there is no per-browser override on Linux, document it).
3. Register the scheme: the Kurobayomi `.desktop` file (area 01) declares `MimeType=x-scheme-handler/aniyomi;`. When the browser hands `aniyomi://simkl-auth?code=...` to the desktop, the app is started with the URL as argument (or D-Bus activation). The app parses the `code` query parameter and calls `SimklTracker::login_code(code)`.
4. Fallback (required because the desktop handler may not be installed when run from a build dir): the Simkl login dialog has a "Paste redirect URL" entry. It accepts either the full `aniyomi://simkl-auth?code=...` text or the bare code. Parsing: `url::Url::parse` then `query_pairs()` for `code`; if the input has no scheme, treat the whole string as the code if it is 20 to 200 chars of `[A-Za-z0-9._-]`.
5. A missing `code` on the redirect is the Kotlin's logout branch (TrackLoginActivity.kt:79-82). The port does the same: call `logout()` and show nothing else.
6. The `code` is single use; it is kept in memory only for the exchange and is never written to logs (`tracing` fields exclude it).

Open decision O-6 (section 13): the redirect URI `aniyomi://simkl-auth` is fixed by the Simkl app registration. Whether a loopback redirect (`http://127.0.0.1:<port>/simkl-auth`) is accepted by Simkl must be checked before choosing; the Kotlin uses the custom scheme and this design keeps it.

---

## 7. Secrets and credential store

Area 27 section 5 defines `SecretStore` (kuro-platform) on top of `keyring`. This area's keys:

| Logical key | Service | Stored in | Value |
|---|---|---|---|
| `track.7.session` | `kurobayomi.track` | secret store | MU session token (replaces `pref_mangasync_password_7`) |
| `track.101.token` | `kurobayomi.track` | secret store | SK access token JSON `{"access_token": "..."}` (replaces `track_token_101`) |
| `track.7.username` | (prefs) | preference store | MU uid as decimal string (replaces `pref_mangasync_username_7`) |
| `track.101.username` | (prefs) | preference store | SK user id decimal string (replaces `pref_mangasync_username_101`) |

`CredentialStore` (new, in `kuro-track/src/creds.rs`):
```rust
pub struct CredentialStore { secrets: std::sync::Arc<dyn SecretStore>, prefs: std::sync::Arc<dyn TrackPrefsStore> }
impl CredentialStore {
    pub fn username(&self, id: TrackerId) -> String;            // "" default
    pub fn password(&self, id: TrackerId) -> Option<String>;     // from secret store; None when absent
    pub fn set(&self, id: TrackerId, username: &str, password: &str) -> Result<(), TrackError>;
    pub fn clear(&self, id: TrackerId);                           // delete all three entries; never writes "null"
}
```
Migration (one-time, on first start of Kurobayomi): read `pref_mangasync_password_7`, `pref_mangasync_password_101`, `track_token_101` from the imported Aniyomi preferences if present, move into the secret store, delete the old keys. `track_token_101` value `"null"` is treated as absent (B-3.11).

If the secret store is unavailable (no Secret Service running), fall back to the preference store with a visible warning in the tracking settings row ("Tokens stored unencrypted"). Document in the UI parity doc. Do not silently store secrets in plain prefs.

Client secret: `CLIENT_SECRET` stays a compile-time constant (see section 11).

---

## 8. Async model

- All HTTP runs on tokio tasks spawned by the tracking runtime (area 27 section 4). The GTK main thread never awaits.
- Results go to the GTK loop through `async_channel` (area 27). Each action returns `Result<AnimeTrack | MangaTrack | Vec<Search>, TrackError>`; the GUI maps errors to toast text.
- Sequencing matches the Kotlin exactly where it affects remote state:
  - MU `update`: list update then rating (two awaits, in order).
  - SK `sync_library`: history remove, history add, add-to-list, rating (four awaits, in order).
  - SK `search`: three awaits in order anime, tv, movie.
- No `runBlocking` anywhere (fix D-SK-9). `current_user_id` is `async`.
- Logout and token set use `std::sync::RwLock` (never held across await), so `logout()` is callable from the GTK thread.
- Cancellation: a dropped future (window closed) may stop between two writes. This is the same hazard as Kotlin's `withNonCancellableContext` absence in these files; PROPOSAL: wrap the SK progress pair (remove + add) and the MU list update in a `tokio::spawn` that is not cancelled by the UI, so a closed window does not leave history removed without re-adding. Test E-SK-P1 in the test doc.
- Timeouts: use the area 12 client timeout (connect 10 s, total 30 s proposal). No per-tracker retry (matches the source). Idempotency: all SK writes are upserts on the server (add-to-list, ratings, history add); the history remove+add pair is retried as a pair only by the user.

---

## 9. Error handling

`read_json(resp)` (shared helper in `kuro-track/src/http.rs`): if `resp.status()` is not 2xx, read at most 512 bytes of the body as `body_snippet` (no token echo) and return `TrackError::Http { status, body_snippet }`; otherwise `resp.json::<T>().await` mapping serde errors to `TrackError::Parse`. Equivalent of `awaitSuccess()` + `parseAs()` (`core/common/.../OkHttpExtensions.kt:109-117, 134-143`).

```rust
// kuro-track/src/error.rs (shared; this area's variants)
#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    #[error("Not authenticated with {0}")]
    NotAuthenticated(&'static str),                       // "MangaUpdates" | "Simkl" (B-4.2 literal text)
    #[error("HTTP {status}")]
    Http { status: u16, body_snippet: String },            // awaitSuccess equivalent; message "HTTP <code>" like Kotlin HttpException
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error("could not read the service response: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Unknown status: {0}")]
    UnknownStatus(i64),                                    // Kotlin NotImplementedError text
    #[error("Unknown status: {0}")]
    UnknownWireStatus(String),
    #[error("Unknown type: {0}")]
    UnknownMediaType(String),
    #[error("missing field: {0}")]
    MissingField(&'static str),                            // replaces `!!` NPEs (fix D-SK-1, D-SK-8)
    #[error("score index {0} is not in the list")]
    ScoreOutOfRange(usize),                                // fix D-MU-7
    #[error("Unable to login")]
    LoginRejected,                                         // used only if the API returns an explicit reject
}
```
Mapping rules:
- `Http { status: 401 | 403 }` on an authenticated call: show "Session expired, log in again" in the tracking row (new UI string, not in Kotlin, because the Kotlin only shows the raw exception). Keep the auth-expired flag concept but store it as `auth_expired: bool` on the credential store (not written by Kotlin for these two services; PROPOSAL, documented in the UI parity file).
- `Http { status: 404 }` on M1 is "not on list" (4.3). On other endpoints a 404 is an ordinary error.
- Network errors show "Network error: <reason>".
- Parse errors show "Unexpected response from <service>" and log the snippet (first 200 bytes, no tokens).
- Never log tokens, passwords, OAuth codes, or the Authorization header. `tracing` fields for requests record method and path only.

---

## 10. Mockup integration (`src/entry.rs`, `src/track/...`)

The mockup has the tracking sheet and search page as static UI:
- `src/entry.rs:17-18` routes `tracking_sheet` and `tracker_search`.
- `src/entry.rs:496-520` `tracking()` builds the sheet with "Add tracking" (line 530) and a remove dialog (`remove_tracking`, line 514).
- `src/entry.rs:539-565` `tracker_search()` builds the search page with a "Track" suggested action (line 563) and a "Track privately" icon button (line 564).

Changes this area requires in the GUI (detail in `p1-ui-parity.md`):
1. `tracker_search()` gets a live search that calls `MangaUpdatesTracker::search` or `SimklTracker::search` depending on the entry kind (manga vs anime). The "Track" button calls the bind action (`register`). "Track privately" sets `private = true` before bind (only meaningful for trackers with `supports_private_tracking`, which both are not; the button must be hidden for MU and SK, see parity doc).
2. Login rows in Settings > Tracking: MU opens the credential dialog; SK opens the browser and shows the paste fallback (section 6).
3. The status picker uses `status_list()` in the order from section 4.1 / 5.1. Score picker uses `score_list()`.
4. The remove dialog calls `DeletableMangaTracker::delete` for MU (it removes the list entry on the server). For SK there is no delete (the dialog removes the local row only); the parity doc states this.

---

## 11. Android to Linux mapping (for symbols in this scope)

| Android/Kotlin symbol | Linux / Rust replacement | Behaviour change |
|---|---|---|
| `android.net.Uri`, `Uri.Builder` (SimklApi.kt:144-148, 185-187, 248-253) | `url::Url` with `query_pairs_mut()` | none (same encoding) |
| `androidx.core.net.toUri()` | `url::Url::parse` | none |
| `android.graphics.Color.rgb` (MangaUpdates.kt:48, Simkl.kt:99) | `u32` ARGB constants | none |
| `R.drawable.ic_manga_updates`, `R.drawable.ic_tracker_simkl` | embedded SVG assets in `kuro-track/assets/` or theme icon names | visual only |
| `dev.icerock.moko.resources.StringResource` (labels) | `&'static str` keys into the app string table | none |
| `uy.kohesive.injekt.injectLazy` (json, prefs) | constructor injection (`Arc`) | none |
| `kotlinx.serialization.json.Json` | `serde_json` | none; key order preserved (D-7) |
| `okhttp3.Interceptor` (MangaUpdatesInterceptor, SimklInterceptor) | `reqwest` default headers built per request in `auth.rs` | none |
| `runBlocking` (SimklApi.kt:205) | `async fn` | fix D-SK-9 |
| `withIOContext` | tokio task on the runtime | none |
| `Context.openInBrowser(url, forceDefaultBrowser)` | `gio::AppInfo::launch_default_for_uri` | no per-browser override |
| `AndroidManifest` intent filter (`aniyomi://simkl-auth`) | `.desktop` `MimeType=x-scheme-handler/aniyomi;` plus paste fallback | section 6 |
| `TrackPreferences` (`PreferenceStore`) | `kuro-prefs` for usernames; secret store for tokens | section 7 |
| `toast(e.message)` | GTK toast with the `TrackError` text (section 9) | `"null"` text never shown (D-MU-9) |
| `BuildConfig.VERSION_NAME`, `APPLICATION_ID` | `env!("CARGO_PKG_VERSION")` and the Linux app id | User-Agent text changes |

Drop: nothing user-visible. `MURating.copyTo` (dead) is not ported as a function.

---

## 12. Pitfalls
1. `score_list()` returns a reference; keep `ScoreList` in the struct.
2. Do not parse SK `tracking_url` with string slicing on absolute URLs; use `path_type` and the normalisation in 5.2/5.7.
3. `f64` to integer: Kotlin `toInt()` truncates; use `as i64` (truncates toward zero) and `trunc()` explicitly. `round()` would change progress (for example 12.9 would become 13).
4. MU chapter numbers are `Int` on the wire; `last_chapter_read` is `f64` locally. Values like `12.5` are sent as `12`.
5. `serde` with `Option<T>` and missing keys: add `#[serde(default)]` to nullable wire fields so a missing key is `None`, matching the Kotlin intent (D-data 4.2 note).
6. Keep the MU 92-entry list and its order exact; the picker index is stored in the UI, not the value.
7. Simkl `date_from` is opaque text from the server. Never parse it.
8. Do not cache GET responses (Kotlin default `max-age=600` header; port sends no cache directive and uses no HTTP cache for these hosts).
9. `keyring` on KDE needs the Secret Service (KWallet bridge). If it is absent the fallback in section 7 applies; the test list covers this.
10. A `watch` channel send ignores receivers that are gone; `let _ = send(...)` is intentional.

---

## 13. Open decisions (need user or live verification)
- **O-1** MU "not on list" status: 404 (assumed) or 200 with empty body. Check against the live API with a known unlisted series id before shipping section 4.3. Impact: wrong branch means either a duplicate add or a lost bind.
- **O-2** `TrackerTrait::index_to_score` return type: keep `f64` (area 27) with a clamp, or change to `Result<f64, TrackError>` (preferred; affects area 27 and this area).
- **O-3** MU rating DELETE on 0.0: adding a "remote rating known" flag to the row changes the DB schema slightly (in `kuro-prefs` or as a column in `manga_sync`). Decide where to store it.
- **O-4** Stored movie path: `/movies/<id>` (proposed) vs Kotlin's `/movie/<id>` form from search (never actually stored, the Kotlin keeps the absolute URL). Confirm with one live search and one live bind.
- **O-5** Progress order: keep remove-then-add (Kotlin) or add-then-trim. Needs a test against the live history endpoint behaviour; the default is to keep the Kotlin order.
- **O-6** Simkl redirect URI: keep `aniyomi://simkl-auth` (requires the desktop handler) or request a loopback URI from Simkl. Default: keep the custom scheme.
- **O-7** Client secret in the binary: keep (matches source, not secret on desktop) or move to a build-time env var. Default: keep.
- **O-8** The live response shapes of Simkl `/sync/all-items` and `/search` and MU `/v1/*` could not be re-verified in this session (external API docs fetch failed). The DTOs follow the Kotlin exactly; verify on first live run and record the fixtures (test doc E-section).
