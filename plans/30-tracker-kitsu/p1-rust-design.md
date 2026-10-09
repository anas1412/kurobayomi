# Area 30-tracker-kitsu: Rust design

Target crate: `/home/blackbox/kurobayomi` (crate `kurobayomi`, edition 2024, `gtk4` 0.11.5 with `v4_16`). Current code: `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/entry.rs`, `src/spec.rs`, `src/settings.rs`, `src/player.rs`, `src/clamp.rs`; spec files `spec/app.json`, `spec/entries.json`, `spec/reader_player.json`, `spec/settings.json`.

Behaviour being ported: `p1-behavior.md`. Wire and data: `p1-data-and-config.md`. Tasks: `p1-tasks.md`.

---

## 1. Module layout

Area:27-tracking-core owns `src/track/mod.rs` (generic tracker registry, the `TrackRecord`/`SearchHit` shared types, and the UI-facing `TrackerRow`). This area owns the Kitsu subtree:

```
src/track/mod.rs                 (area 27) pub mod kitsu; shared Kind, TrackRecord, SearchHit
src/track/kitsu/mod.rs           Kitsu facade: id, names, status/score tables, bind/update/refresh/delete/search, login/logout
src/track/kitsu/error.rs         KitsuError (thiserror), user-visible messages
src/track/kitsu/auth.rs          OAuth, AuthSession (token state + refresh mutex), SecretStore trait + impls
src/track/kitsu/api.rs           Api: every REST call (JSON:API library entries, users, OAuth token)
src/track/kitsu/search.rs        Algolia key fetch (cached), Algolia query, hit -> SearchHit
src/track/kitsu/dto.rs           serde structs for every response/request body
src/track/kitsu/mapping.rs       status tables, score list/index/display, to/from API score
src/track/kitsu/dates.rs         wire timestamp conversion (UTC)
src/track/kitsu/tests/           wiremock integration tests + JSON fixtures (tests/fixtures/kitsu/*.json)
```

Ponytail note: one `Kitsu` struct, no trait for the Kitsu implementation itself (one implementation). A trait appears only where there are two real implementations: `SecretStore` (keyring + settings DB) and `CredentialStore` (settings DB, also faked in tests).

---

## 2. Public surface (signatures)

```rust
// src/track/mod.rs (shared with area 27)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind { Manga, Anime }

#[derive(Clone, Debug, PartialEq)]
pub struct TrackRecord {
    pub id: Option<i64>,
    pub item_id: i64,
    pub kind: Kind,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub progress: f64,
    pub total: i64,
    pub score: f64,
    pub status: i64,
    pub started_ms: i64,
    pub finished_ms: i64,
    pub tracking_url: String,
    pub private: bool,
}

impl TrackRecord {
    /// Kotlin copyPersonalFrom(other, copyRemotePrivate)
    pub fn copy_personal_from(&mut self, other: &TrackRecord, copy_remote_private: bool) {
        self.progress = other.progress;
        self.score = other.score;
        self.status = other.status;
        self.started_ms = other.started_ms;
        self.finished_ms = other.finished_ms;
        if copy_remote_private { self.private = other.private; }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub kind: Kind,
    pub remote_id: i64,
    pub title: String,
    pub total: i64,
    pub cover_url: String,
    pub summary: String,
    pub tracking_url: String,
    pub score: f64,
    pub publishing_status: String,
    pub publishing_type: String,
    pub start_date: String,
}
```

```rust
// src/track/kitsu/mod.rs
pub const KITSU_ID: i64 = 3;
pub const NAME: &str = "Kitsu";

pub const READING: i64 = 1;
pub const WATCHING: i64 = 11;
pub const COMPLETED: i64 = 2;
pub const ON_HOLD: i64 = 3;
pub const DROPPED: i64 = 4;
pub const PLAN_TO_READ: i64 = 5;
pub const PLAN_TO_WATCH: i64 = 15;

pub struct Endpoints {
    pub api_base: url::Url,         // https://kitsu.app/api/edge/
    pub token_url: url::Url,        // https://kitsu.app/api/oauth/token
    pub algolia_key_url: url::Url,  // https://kitsu.app/api/edge/algolia-keys/media/
    pub algolia_url: url::Url,      // https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/
}
impl Endpoints { pub fn production() -> Self; }

pub struct Kitsu { /* api: api::Api, auth: Arc<auth::AuthSession>, search: search::Search, creds: Arc<dyn CredentialStore> */ }

pub trait CredentialStore: Send + Sync {
    fn username(&self) -> Option<String>;
    fn user_id(&self) -> Option<String>;
    fn save(&self, username: &str, user_id: &str) -> Result<(), StoreError>;
    fn clear(&self) -> Result<(), StoreError>;
}

impl Kitsu {
    pub fn new(
        http: reqwest::Client,
        endpoints: Endpoints,
        secrets: Box<dyn auth::SecretStore>,
        creds: Arc<dyn CredentialStore>,
    ) -> Self;

    pub fn is_logged_in(&self) -> bool;                       // username and user_id both non-empty
    pub async fn login(&self, username: &str, password: &str) -> Result<(), KitsuError>;
    pub async fn logout(&self) -> Result<(), KitsuError>;

    pub async fn search(&self, kind: Kind, query: &str) -> Result<Vec<SearchHit>, KitsuError>;
    pub async fn bind(&self, track: &mut TrackRecord, has_progress: bool) -> Result<(), KitsuError>;
    pub async fn update(&self, track: &mut TrackRecord, progress_event: bool) -> Result<(), KitsuError>;
    pub async fn refresh(&self, track: &mut TrackRecord) -> Result<(), KitsuError>;
    pub async fn delete(&self, track: &TrackRecord) -> Result<(), KitsuError>;

    pub fn status_list(kind: Kind) -> &'static [i64];          // manga [1,2,3,4,5]; anime [11,15,2,3,4]
    pub fn status_label(kind: Kind, status: i64) -> Option<&'static str>;   // English text, see section 6.4
    pub fn completion_status() -> i64;                         // 2
    pub fn score_list() -> &'static [&'static str];            // 20 entries, see section 5
    pub fn index_to_score(index: usize) -> f64;
    pub fn display_score(score: f64) -> String;
    pub fn logo_color() -> u32;                                // 0x332532
}
```

Notes on the signatures:
- `bind`, `update`, `refresh` take `&mut TrackRecord` because the Kotlin mutates the passed track. Callers that need the original keep a clone.
- `update(track, progress_event)`: `progress_event` is the Kotlin `didReadChapter`/`didWatchEpisode` flag. Only the chapter/episode-read path (area 27 `TrackChapter`) passes `true`.
- `delete` takes `&TrackRecord` and uses `remote_id` as the library-entry id (matches Kotlin `track.remoteId`).

---

## 3. Errors (`src/track/kitsu/error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum KitsuError {
    #[error("Not authenticated with Kitsu")]
    NotAuthenticated,
    #[error("Kitsu session has expired, log in again")]
    ReauthRequired,
    #[error("HTTP error {0}")]
    Http(u16),
    #[error("{0}")]
    Transport(String),                      // reqwest error text
    #[error("Failed to decode Kitsu response: {0}")]
    Decode(String),                         // serde_json error text
    #[error("Unknown status")]
    UnknownStatus(String),
    #[error("Missing User data from Kitsu")]
    MissingUserData,
    #[error("Missing {0} data from Kitsu")]
    MissingMediaData(&'static str),          // "Manga" | "Anime" (Aniyomi anime path says "Manga": fixed)
    #[error("Could not find {0}")]
    NotFound(&'static str),                  // "manga" | "anime" (Aniyomi anime path says "manga": fixed)
    #[error("Invalid Kitsu timestamp: {0}")]
    BadDate(String),
    #[error("Kitsu returned no user")]
    NoUser,
}
```

Error mapping rules:
- `reqwest::Error` with `status()` → `Http(code)`; else `Transport`.
- `serde_json::Error` → `Decode`.
- Display strings for `Http`, `NotAuthenticated`, `UnknownStatus`, `MissingUserData`, `MissingMediaData`, `NotFound` are the Aniyomi texts (behaviour section 6 and data section 6.2). The UI shows `e.to_string()` in the same places Aniyomi shows `e.message`.
- `StoreError` (keyring/settings failure) is kept separate and logged; it never blocks a network result.

Proposal on `Transport`: keep the reqwest text. Aniyomi shows the `IOException` message; there is no fixed text.

---

## 4. Authentication (`src/track/kitsu/auth.rs`)

```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OAuth {
    #[serde(rename = "access_token")] pub access_token: String,
    #[serde(rename = "token_type")] pub token_type: String,
    #[serde(rename = "created_at")] pub created_at: i64,
    #[serde(rename = "expires_in")] pub expires_in: i64,
    #[serde(rename = "refresh_token", default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}
impl OAuth {
    pub fn is_expired(&self, now_secs: i64) -> bool { now_secs > self.created_at + self.expires_in - 3600 }
}

pub trait SecretStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, StoreError>;
    fn save(&self, value: &str) -> Result<(), StoreError>;
    fn delete(&self) -> Result<(), StoreError>;
}

pub struct KeyringStore;          // keyring crate, service "kurobayomi.track.kitsu", user "oauth"
pub struct SettingsStore { /* area:02 settings handle; key "track_token_3" */ }
// Proposal: Kitsu::new picks KeyringStore; on StoreError::NoStorage at startup it uses SettingsStore.

pub struct AuthSession {
    token: tokio::sync::Mutex<Option<OAuth>>,
    store: Box<dyn SecretStore>,
    http: reqwest::Client,
    token_url: url::Url,
}

impl AuthSession {
    pub fn restore(http: reqwest::Client, token_url: url::Url, store: Box<dyn SecretStore>) -> Self;
    // parse failures, "", and "null" all become None (behaviour section 4.2)

    pub async fn set(&self, token: Option<OAuth>) -> Result<(), KitsuError>;
    // Some -> store.save(json); None -> store.delete() (Aniyomi stores "null"; see proposal)

    /// Returns the current access token, refreshing first if expired.
    pub async fn bearer(&self) -> Result<String, KitsuError>;

    pub async fn snapshot(&self) -> Option<OAuth>;
}
```

`bearer()` algorithm (the mutex is held for the whole call; this is the race fix):
```rust
let mut guard = self.token.lock().await;
let cur = guard.clone().ok_or(KitsuError::NotAuthenticated)?;
if cur.is_expired(now_secs()) {
    let rt = cur.refresh_token.clone().ok_or(KitsuError::ReauthRequired)?;
    match refresh(&self.http, &self.token_url, &rt).await {
        Ok(mut fresh) => {
            if fresh.refresh_token.is_none() { fresh.refresh_token = cur.refresh_token.clone(); }
            if let Err(e) = self.store_save(&fresh) { log::warn!("{e}"); }
            *guard = Some(fresh);
        }
        Err(KitsuError::Http(code)) if (400..500).contains(&code) => return Err(KitsuError::ReauthRequired),
        Err(e) => log::warn!("token refresh failed, keeping stale token: {e}"),   // Aniyomi behaviour
    }
}
Ok(guard.as_ref().expect("checked above").access_token.clone())
```
- Deviation from Aniyomi (flagged proposal): a 4xx refresh response becomes `ReauthRequired` instead of sending a request with the expired token. Network errors keep the Aniyomi behaviour (stale token, the request then fails with 401).
- Deviation (flagged): `refresh_token` is required only when the token is expired. Aniyomi requires it always (`!!` before the expiry check).
- Deviation (flagged): a refresh response without `refresh_token` keeps the old one.

`refresh()` helper: `POST token_url` with form `grant_type=refresh_token`, `refresh_token`, `client_id`, `client_secret` (order as Aniyomi). Decode `OAuth`.

Constants:
```rust
pub const CLIENT_ID: &str = "dd031b32d2f56c990b1425efe6c42ad847e7fe3ab46bf1299f05ecd856bdb7dd";
pub const CLIENT_SECRET: &str = "54d7307928f63414defd96399fc31ba847961ceaecef3a5fd93144e960c0e151";
```

Refresh request and login request skip the bearer; the refresh request also skips the User-Agent header that Aniyomi happened to omit. Proposal: send the same headers on every Kitsu request (including the refresh), which is a flagged change.

---

## 5. Mapping (`src/track/kitsu/mapping.rs`)

```rust
pub fn api_status(kind: Kind, status: i64) -> Result<&'static str, KitsuError> {
    match (kind, status) {
        (Kind::Manga, READING) | (Kind::Anime, WATCHING) => Ok("current"),
        (_, COMPLETED) => Ok("completed"),
        (_, ON_HOLD) => Ok("on_hold"),
        (_, DROPPED) => Ok("dropped"),
        (Kind::Manga, PLAN_TO_READ) | (Kind::Anime, PLAN_TO_WATCH) => Ok("planned"),
        _ => Err(KitsuError::UnknownStatus(status.to_string())),
    }
}

pub fn local_status(kind: Kind, api: &str) -> Result<i64, KitsuError> {
    Ok(match (kind, api) {
        (_, "current") => if kind == Kind::Manga { READING } else { WATCHING },
        (_, "completed") => COMPLETED,
        (_, "on_hold") => ON_HOLD,
        (_, "dropped") => DROPPED,
        (_, "planned") => if kind == Kind::Manga { PLAN_TO_READ } else { PLAN_TO_WATCH },
        _ => return Err(KitsuError::UnknownStatus(api.to_string())),
    })
}
```
Check against Aniyomi: `toApiStatus` for manga accepts READING/COMPLETED/ON_HOLD/DROPPED/PLAN_TO_READ only; anime accepts WATCHING/COMPLETED/ON_HOLD/DROPPED/PLAN_TO_WATCH only. The table above: `(Kind::Manga, WATCHING)` falls through to `_ => Err`, and `(Kind::Anime, READING)` also errors. Correct. Wire `current` parses to READING for manga and WATCHING for anime: matches DTO:36-77.

Score:
```rust
pub const SCORE_LIST: [&str; 20] = ["0","1","1.5","2","2.5","3","3.5","4","4.5","5","5.5","6","6.5","7","7.5","8","8.5","9","9.5","10"];

pub fn index_to_score(index: usize) -> f64 { if index > 0 { (index as f64 + 1.0) / 2.0 } else { 0.0 } }
```
Aniyomi's `indexToScore(-1)` (missing string) returns `0.0`. With `usize` input the caller maps "not found" to 0 explicitly: `score_list().iter().position(|s| *s == text).map_or(0.0, index_to_score)`.

Display (Aniyomi `DecimalFormat("0.#")`, locale-dependent):
```rust
pub fn display_score(score: f64) -> String { format!("{score}") }   // Rust f64 Display: 7.0 -> "7", 7.5 -> "7.5", 0.0 -> "0"
```
Equivalence check: values are multiples of 0.5 in [0, 10], so the Rust output equals `DecimalFormat("0.#")` in the `.` locale. Unit test T-30a-035.

Wire score:
```rust
pub fn to_api_score(score: f64) -> Option<String> {
    if score > 0.0 { Some(((score * 2.0).trunc() as i64).to_string()) } else { None }
}
pub fn from_api_rating(rating: Option<i64>) -> f64 { rating.map_or(0.0, |r| r as f64 / 2.0) }
```

Kotlin `(score * 2).toInt()` truncates toward zero; `trunc() as i64` matches for all inputs in range.

---

## 6. API layer (`src/track/kitsu/api.rs`)

```rust
pub(crate) struct Api {
    http: reqwest::Client,       // base client (login, algolia POST)
    auth: Arc<AuthSession>,
    base: url::Url,              // api_base
}

impl Api {
    pub async fn login_password(&self, username: &str, password: &str) -> Result<OAuth, KitsuError>;
    pub async fn current_user_id(&self) -> Result<String, KitsuError>;
    pub async fn add_entry(&self, track: &mut TrackRecord, user_id: &str) -> Result<(), KitsuError>;
    pub async fn update_entry(&self, track: &TrackRecord) -> Result<(), KitsuError>;
    pub async fn remove_entry(&self, library_id: i64) -> Result<(), KitsuError>;
    pub async fn find_entry(&self, kind: Kind, media_id: i64, user_id: &str) -> Result<Option<TrackRecord>, KitsuError>;
    pub async fn get_entry(&self, kind: Kind, library_id: i64) -> Result<TrackRecord, KitsuError>;
}
```
Every authed call: `let bearer = self.auth.bearer().await?;` then headers:
```rust
.header(reqwest::header::AUTHORIZATION, format!("Bearer {bearer}"))
.header(reqwest::header::USER_AGENT, USER_AGENT)            // "Kurobayomi v{CARGO_PKG_VERSION} (io.github.blackbox.Kurobayomi)"
.header(reqwest::header::ACCEPT, "application/vnd.api+json")
.header(reqwest::header::CONTENT_TYPE, "application/vnd.api+json")
.header("Cache-Control", "no-cache")                        // Proposal, see behaviour 5.3
```

Bodies (serde_json::json!, key order does not matter for JSON semantics but keep it for fixture diffs):
- Add: `{"data":{"type":"libraryEntries","attributes":{"status":S,"progress":P,"private":B},"relationships":{"user":{"data":{"id":U,"type":"users"}},"media":{"data":{"id":M,"type":KIND}}}}}` where `P = progress.trunc() as i64`, `M` is a JSON number, `U` a JSON string.
- Update: `{"data":{"type":"libraryEntries","id":L,"attributes":{"status":S,"progress":P,"ratingTwenty":R,"startedAt":SA,"finishedAt":FA,"private":B}}}` with `R = to_api_score(score)` (`null` if None), `SA = dates::to_wire(started_ms)` (`null` if None), `FA` likewise.
- Remove: `DELETE {base}library-entries/{L}` with `Content-Type` header and no body.
- Find: `GET {base}library-entries?filter[manga_id]={M}&filter[user_id]={U}&include=manga` (anime: `filter[anime_id]`, `include=anime`). Build the query string by hand (raw brackets), not with `Url::query_pairs_mut`, which would percent-encode `[` and `]`.
- Get: `GET {base}library-entries?filter[id]={L}&include=manga|anime`.
- Current user: `GET {base}users?filter[self]=true`.
- Login: `POST token_url` with `application/x-www-form-urlencoded`, fields in order `username`, `password`, `grant_type=password`, `client_id`, `client_secret`.

Response decoding helper:
```rust
async fn json<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, KitsuError> {
    let status = resp.status();
    if !status.is_success() { return Err(KitsuError::Http(status.as_u16())); }
    let bytes = resp.bytes().await.map_err(map_reqwest)?;
    serde_json::from_slice(&bytes).map_err(|e| KitsuError::Decode(e.to_string()))
}
```
Write paths call `ensure_success(resp)` and drop the body (matching Aniyomi, which does not read PATCH/DELETE bodies).

`find_entry` returns `None` when `data` or `included` is empty (behaviour 6.5). `get_entry` returns `NotFound(kind)` in that case.

Converting list entries (`dto::ListEntry` → `TrackRecord`), mirrors `firstToMangaTrack`/`firstToAnimeTrack`:
```rust
fn to_track(kind: Kind, item: ListItem, media: ListMedia) -> Result<TrackRecord, KitsuError> {
    Ok(TrackRecord {
        id: None,
        item_id: 0,
        kind,
        remote_id: item.id,                                   // library-entry id
        library_id: None,
        title: media.canonical_title,
        progress: item.attributes.progress as f64,
        total: match kind { Kind::Manga => media.chapter_count, Kind::Anime => media.episode_count }.unwrap_or(0),
        score: mapping::from_api_rating(item.attributes.rating_twenty),
        status: mapping::local_status(kind, &item.attributes.status)?,
        started_ms: dates::from_wire(item.attributes.started_at.as_deref())?,
        finished_ms: dates::from_wire(item.attributes.finished_at.as_deref())?,
        tracking_url: format!("{}{}", base_url(kind), item.id),   // reproduces Aniyomi's library-id quirk, see T-30a-029
        private: item.attributes.private,
    })
}
```
Note: Aniyomi's `publishing_status`, `publishing_type`, `summary`, `cover_url` on the list result are not part of `TrackRecord`. They are dropped for the Rust record (no consumer persists them). Proposal: `ListMedia` keeps them for the UI detail sheet only if area 27 requests; otherwise dropped.

`MissingUserData` / `MissingMediaData` checks happen before conversion: `data.is_empty()` → `MissingUserData`; `included.is_empty()` → `MissingMediaData(kind_label)`.

Note on `status`: unknown status string → `UnknownStatus`, matching Aniyomi.

Dates are parsed in conversion order: started, then finished. Aniyomi evaluates `started_reading_date` then `finished_reading_date` (inside `apply`), so the first failing date determines the error. Same here.

Important: `to_track` fails the whole call if `status` or a date is bad. This is Aniyomi behaviour. Proposal (flagged): log and treat the bad date as 0; keep status errors fatal. Decision in T-30a-016.

---

## 7. Search (`src/track/kitsu/search.rs`)

```rust
pub(crate) struct Search {
    http: reqwest::Client,        // base client, no bearer
    key_fetch: Arc<AuthedGet>,    // authed GET of algolia_key_url (needs AuthSession)
    key: tokio::sync::Mutex<Option<String>>,
    algolia_url: url::Url,
}

impl Search {
    pub async fn search(&self, kind: Kind, query: &str) -> Result<Vec<SearchHit>, KitsuError> {
        let key = self.key().await?;                       // cached; refetch on 403 (once)
        let params = format!("query={}{}", form_urlencoded_space_plus(query), filter(kind));
        let body = serde_json::json!({ "params": params });
        let resp = self.http.post(self.algolia_url.clone())
            .header("X-Algolia-Application-Id", "AWQO5J657S")
            .header("X-Algolia-API-Key", key)
            .header(reqwest::header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(body.to_string())
            .send().await.map_err(map_reqwest)?;
        let hits: AlgoliaResult = json(resp).await?;
        Ok(hits.hits.into_iter()
            .filter(|h| h.subtype.as_deref() != Some("novel"))
            .map(|h| to_hit(kind, h))
            .collect())
    }
}

pub const FILTER_MANGA: &str = "&facetFilters=%5B%22kind%3Amanga%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22chapterCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D";
pub const FILTER_ANIME: &str = "&facetFilters=%5B%22kind%3Aanime%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22episodeCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D";
```

Query encoding: Java `URLEncoder.encode(query, "UTF-8")` = `application/x-www-form-urlencoded` encoding with space as `+`. Use `url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()`. It leaves `*`, `-`, `.`, `_` and ASCII alphanumerics; encodes everything else (including `~`) as `%XX` uppercase; space to `+`. Same as Java. Verified by unit test with `"a b&c~d*e"` → `a+b%26c%7Ed*e`.

`to_hit`: as behaviour section 12. `start_date`: `startDate` seconds → `chrono::DateTime::from_timestamp(secs, 0)` → `format("%Y-%m-%d")` **in the local zone** to match Aniyomi (`SimpleDateFormat` default zone). Proposal: keep local zone for display (the mockup shows Started "2020-01-01"), flagged in T-30a-018.

Key fetch: `GET algolia_key_url` through the authed path (`bearer()` required), decode `{"media":{"key":"<key>"}}`. Cache in `self.key`. On `Http(403)` from Algolia: clear cache, refetch once, retry once. Proposal (flagged).

`AlgoliaResult`: `hits: Vec<AlgoliaHit>`; `AlgoliaHit { id: i64 (lenient), canonical_title: String, chapter_count: Option<i64>, episode_count: Option<i64>, subtype: Option<String>, poster_image: Option<Cover>, synopsis: Option<String>, average_rating: Option<f64>, start_date: Option<i64>, end_date: Option<i64> }`. Missing required fields fail the whole response (Aniyomi behaviour). Proposal (flagged): skip hits that fail to decode, log each.

---

## 8. Facade logic (`src/track/kitsu/mod.rs`)

### 8.1 Login / logout
```rust
pub async fn login(&self, username: &str, password: &str) -> Result<(), KitsuError> {
    let token = self.api.login_password(username, password).await?;     // no token stored yet
    self.auth.set(Some(token)).await?;                                   // step 2 in Aniyomi
    let user_id = match self.api.current_user_id().await {
        Ok(id) => id,
        Err(e) => { let _ = self.auth.set(None).await; return Err(e); }  // Proposal: transactional
    };
    self.creds.save(username, &user_id)?;                                // email + user id, not password
    Ok(())
}

pub async fn logout(&self) -> Result<(), KitsuError> {
    let _ = self.creds.clear();
    self.auth.set(None).await                                            // Proposal: remove, not "null"
}
```
Aniyomi's `login` does not roll back on step 3 failure, and the UI's `checkLogin` calls `logout()` on any failure, so the end state is the same in the UI. The Rust version rolls back inside `login`.

`is_logged_in`: `creds.username()` and `creds.user_id()` both `Some` and non-empty. Matches `BaseTracker.isLoggedIn` (non-empty username and password) with user id standing in for password.

### 8.2 bind / update / refresh / delete
Algorithms are in behaviour sections 7-10. Rust code shape:

```rust
pub async fn bind(&self, t: &mut TrackRecord, has_progress: bool) -> Result<(), KitsuError> {
    let user = self.user_id()?;                       // NotAuthenticated when missing (Aniyomi sends "")
    match self.api.find_entry(t.kind, t.remote_id, &user).await? {
        Some(remote) => {
            t.copy_personal_from(&remote, false);
            t.remote_id = remote.remote_id;
            if t.status != COMPLETED && has_progress {
                t.status = active(t.kind);
            }
            self.api.update_entry(t).await
        }
        None => {
            t.status = if has_progress { active(t.kind) } else { plan(t.kind) };
            t.score = 0.0;
            self.api.add_entry(t, &user).await
        }
    }
}
```
`active(kind)` = `READING` or `WATCHING`; `plan(kind)` = `PLAN_TO_READ` or `PLAN_TO_WATCH`.

```rust
pub async fn update(&self, t: &mut TrackRecord, progress_event: bool) -> Result<(), KitsuError> {
    if t.status != COMPLETED && progress_event {
        if t.progress.trunc() as i64 == t.total && t.total > 0 {
            t.status = COMPLETED;
            t.finished_ms = now_ms();
        } else {
            t.status = active(t.kind);
            if t.progress == 1.0 { t.started_ms = now_ms(); }
        }
    }
    self.api.update_entry(t).await
}
```
`refresh`: `let remote = self.api.get_entry(t.kind, t.remote_id).await?; t.copy_personal_from(&remote, true); t.total = remote.total; Ok(())`.

`delete`: `self.api.remove_entry(t.remote_id).await`.

`update_entry` on a track whose `remote_id == 0` (unbound): Aniyomi sends the request anyway. Proposal: return `KitsuError::NotFound(...)` locally without a request (T-30a-010). Flagged deviation; avoids a useless 404.

### 8.3 Search
`pub async fn search(&self, kind, query)`: delegates to `Search::search`. Requires login: the `Search` key fetch calls `auth.bearer()`, which returns `NotAuthenticated` when there is no token.

---

## 9. Dates (`src/track/kitsu/dates.rs`)

```rust
pub const WIRE: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";

pub fn to_wire(ms: i64) -> Option<String> {
    if ms == 0 { return None; }
    chrono::DateTime::from_timestamp_millis(ms).map(|d| d.format(WIRE).to_string())
}

pub fn from_wire(s: Option<&str>) -> Result<i64, KitsuError> {
    match s {
        None => Ok(0),
        Some(v) => chrono::NaiveDateTime::parse_from_str(v, WIRE)
            .map(|d| d.and_utc().timestamp_millis())
            .map_err(|_| KitsuError::BadDate(v.to_string())),
    }
}
```
Notes:
- `%.3f` prints `.SSS` with 3 digits, same as the Kotlin pattern.
- Plain `Z` in a chrono format string is a literal character.
- Empty string → `BadDate` (Aniyomi `parse("")` throws).
- Aniyomi's local-zone defect is NOT reproduced; this is a flagged deviation (T-30a-016). Unit test: `to_wire(1714564800000)` = `"2024-05-01T12:00:00.000Z"` (UTC) regardless of the host zone.

---

## 10. Concurrency model

- One `tokio` multi-thread runtime owned by the app (area 01/27), e.g. `static RT: OnceLock<tokio::runtime::Runtime>`. All Kitsu futures run there.
- The GTK main thread never awaits network I/O directly. Pattern:
```rust
fn spawn_kitsu<T: Send + 'static>(
    fut: impl std::future::Future<Output = T> + Send + 'static,
    on_done: impl FnOnce(T) + 'static,
) {
    let (tx, rx) = async_channel::bounded(1);
    RT.get().expect("runtime").spawn(async move { let _ = tx.send(fut.await).await; });
    gtk::glib::spawn_future_local(async move {
        if let Ok(v) = rx.recv().await { on_done(v); }
    });
}
```
`on_done` runs on the GTK thread and may touch widgets.
- `SecretStore` calls (keyring) are blocking. Wrap them in `tokio::task::spawn_blocking` inside `AuthSession::set`/`bearer`, or call them from the runtime's blocking pool.
- Mutexes: `AuthSession.token` (tokio mutex, held across the refresh request by design); `Search.key` (tokio mutex, held across the key fetch so concurrent searches fetch once).
- No global lock across entries. Two different tracks can PATCH concurrently. Aniyomi does not serialise either. Proposal (T-30a-026): a per-library-entry async mutex to keep a `bind` and a read-event `update` from interleaving on the same entry.

Timeouts (single `reqwest::Client` built once, shared by login, search, and API):
```rust
reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(30))
    .read_timeout(Duration::from_secs(30))
    .timeout(Duration::from_secs(120))     // Aniyomi callTimeout
    .user_agent(USER_AGENT)
    .use_rustls_tls()                      // see pitfalls: prefer rustls-tls-native-roots
    .build()?;
```
Proposal: `default-features = false`, features `["json", "rustls-tls-native-roots"]`, so the system CA store (KDE/Arch) is used.

---

## 11. Crate choices

| Need | Crate (proposed version) | Why |
|---|---|---|
| HTTP client | `reqwest` 0.12 (`default-features = false`, `json`, `rustls-tls-native-roots`) | Replaces OkHttp; async; per-request headers |
| JSON | `serde` 1 (`derive`), `serde_json` 1 | Replaces kotlinx.serialization |
| Time | `chrono` 0.4 | Replaces `SimpleDateFormat`/`System.currentTimeMillis` |
| Async runtime | `tokio` 1 (`rt-multi-thread`, `sync`, `time`, `macros`) | Replaces coroutines + `Dispatchers.IO` |
| GTK bridge | `async-channel` 2 | Runtime to GTK main context |
| Form/URL encoding | `url` 2 (`form_urlencoded`) | Replaces `URLEncoder`, `FormBody` |
| Errors | `thiserror` 2 | `KitsuError` |
| Secret storage | `keyring` 3 (Secret Service backend on Linux) | Replaces SharedPreferences token storage; KWallet exposes Secret Service |
| Logging | `log` 0.4 + the app's logger (area 01) | Replaces `logcat` |
| Tests (dev) | `wiremock` 0.6, `tokio` (macros) | HTTP fixture server for every endpoint |

Not used: a JSON:API library (the payloads are small and fixed), any OAuth crate (password grant is one POST), a retry crate (Aniyomi does not retry).

---

## 12. Android to Linux/Rust mapping (scope files)

| Android/Kotlin | Used in | Rust / Linux replacement |
|---|---|---|
| `android.graphics.Color.rgb(51, 37, 50)` | Kitsu.kt:59 | `0x332532` constant; applied as CSS colour in GTK |
| `R.drawable.ic_tracker_kitsu` | Kitsu.kt:57 | Not bundled (see T-30a-030); text tile with the initial "K" and colour swatch |
| `dev.icerock.moko.resources.StringResource` | Kitsu.kt, KitsuApi.kt | English literal + key string (area 27 i18n) |
| `androidx.core.net.toUri` | KitsuApi.kt:3 (`buildUpon().encodedQuery`) | `url::Url` + manual raw query building |
| `kotlinx.serialization` (`@Serializable`, `@SerialName`, `Json`, `buildJsonObject`, `putJsonObject`, `encodeToString`, `decodeFromString`) | all | `serde`, `serde_json::json!`, `serde(rename)` |
| `okhttp3.OkHttpClient` / `Interceptor` / `Request` / `RequestBody` / `FormBody` / `MediaType` / `Headers` | KitsuApi, KitsuInterceptor | `reqwest::Client`, `RequestBuilder`, `.form()`, `.header()` |
| `eu.kanade.tachiyomi.network.GET/POST/DELETE/awaitSuccess/parseAs/jsonMime` | KitsuApi | local helpers `json()`, `ensure_success()` |
| `uy.kohesive.injekt.injectLazy` | Kitsu.kt, KitsuApi.kt, KitsuInterceptor.kt | constructor injection |
| `tachiyomi.core.common.util.lang.withIOContext` | KitsuApi.kt | `tokio` runtime `spawn` |
| `java.text.SimpleDateFormat` (default zone) | KitsuDateHelper.kt, KitsuSearch.kt | `chrono`, UTC for wire, local for display |
| `java.text.DecimalFormat("0.#")` (default locale) | Kitsu.kt:98-112 | `format!("{score}")`, always `.` |
| `java.net.URLEncoder` (UTF-8) | KitsuApi.kt:254,285 | `url::form_urlencoded::byte_serialize` |
| `kotlinx.collections.immutable.ImmutableList` | Kitsu.kt getScoreList | `&'static [&'static str]` |
| `logcat` | (via BaseTracker callers) | `log` |

---

## 13. Integration with the Kurobayomi mockup

Current mockup behaviour (read from source):
- `src/entry.rs::tracking()` (lines 498-537) builds three hard-coded rows `[("AniList", true), ("MyAnimeList", false), ("Kitsu", false)]`. Kitsu shows an "Add tracking" button that calls `open("tracker_search")`.
- `src/entry.rs::tracker_search()` (lines 539-570) builds four placeholder list rows (`title_n(i)`, "Author name", fixed Type/Started/Status/Score strings), a SearchEntry prefilled with `title_n(1)`, and a revealer bar with a suggested-action "Track" button and an icon button "Track privately". Selecting a row reveals the bar.
- `src/entry.rs::dialog_of(id, name)` (line 25) opens the dialogs listed in `spec/entries.json` (for example `remove_tracking`, title "Remove AniList tracking?", text "This will remove the tracking locally.").
- `spec/entries.json` holds the tracking sheet spec (`id: tracking_sheet`, row subtitles such as "Add tracking" and "Placeholder text when unset; only if tracker supports scoring").

Plug-in plan (Proposal):
1. Replace the hard-coded tracker array in `tracking()` with `TrackerRow` values from area 27 (`src/track/mod.rs`): `pub struct TrackerRow { pub tracker_id: i64, pub name: &'static str, pub bound: Option<TrackRecord> }`. Kitsu's row: `name = "Kitsu"`, `logo_color = 0x332532`.
2. For a bound Kitsu row, the grid shows `status` through `Kitsu::status_label(kind, t.status)`, `progress` / `total` ("12 / 24"; `total == 0` shows "12 / ?"), `score` through `Kitsu::display_score`, and the dates. Those four cells exist in the mockup at lines 515-520 and map to `status`, `chapters`, `score`, `start_date`, `finish_date` dialogs.
3. The "Remove" action in the row menu (line 514) opens `remove_tracking` with the Kitsu checkbox "Also remove from Kitsu" (shown only for Kitsu, since it is Deletable). The mockup dialog currently has no checkbox; the spec entry needs a `checkbox` field.
4. `tracker_search()` replaces the four placeholder rows with `SearchHit`s from `Kitsu::search(kind, query)` via `spawn_kitsu`. Title, total, cover (`cover_url`, fetched by area 27 image cache), `publishing_status`, `publishing_type`, `start_date`, and the score (labelled as the Kitsu average rating, not the 0-10 entry score).
5. The Kitsu login dialog is new: `src/track/kitsu/login_dialog.rs` (Proposal) built with `gtk::Window` (modal, transient for the main window), `gtk::Entry` (email, input purpose `Email`), `gtk::PasswordEntry` (show-peek-icon), and a suggested button "Login" that is `set_sensitive(!processing && both non-blank)`. Errors show as a `gtk::Label` with CSS class `error` below the button, instead of a toast only. Toast-equivalent: the existing app toast/overlay if area 27 provides it.
6. Logout: `gtk::AlertDialog` (GTK 4.10+) with buttons "Cancel" and "Log out" (destructive), then `Kitsu::logout`.
7. Delete: `gtk::AlertDialog` with message "Remove Kitsu tracking?" (mockup wording pattern from `track_delete_title`) and detail "This will remove the tracking locally." plus `gtk::CheckButton` "Also remove from Kitsu" (only for Kitsu). OK button uses `destructive-action` style.

The mockup does not own any state; `dialog_of`/`open` remain the navigation helpers. Area 27 owns the store that feeds `TrackerRow`.

---

## 14. Testing hooks

- `Endpoints` is a plain struct, so tests point every URL at a `wiremock::MockServer`.
- `SecretStore` and `CredentialStore` have in-memory implementations under `#[cfg(test)]`.
- `now_secs()` and `now_ms()` are injected via a `Clock` function pointer field (`fn() -> i64`) in `Kitsu`, so expiry and timestamp tests are deterministic. Ponytail: a `fn` field, not a trait.

---

## 15. Pitfalls

1. `keyring` calls block; never call them on a GTK thread or directly inside an async task without `spawn_blocking`.
2. Secret Service may be absent (no KWallet, headless). Fall back to the settings table; log once.
3. `reqwest` `rustls-tls` with `webpki-roots` ignores the system CA store; use `rustls-tls-native-roots`.
4. Algolia rotates keys; a cached key can go stale for a long session. The one-refetch-on-403 rule covers this.
5. Kitsu refresh-token rotation behaviour is not visible in source. The mutex prevents this app from racing itself; if the server invalidates the old refresh token on each refresh, every refresh must persist the new one before releasing the lock (it does, in `bearer`).
6. `reqwest` `Response::bytes` must be read before the connection can be reused; always consume bodies (Aniyomi leaves PATCH bodies unread; Rust must drop them explicitly to avoid pool starvation).
7. `chrono` `%.3f` output for values with sub-millisecond precision truncates; the stored epoch is millisecond-precise, so no loss.
8. Locale: never call `format!` with a locale-dependent helper for scores; `Display` on f64 is locale-independent.
9. The Kitsu search endpoint requires login: the UI must not offer tracker search for a logged-out Kitsu row (or must show "Log in to Kitsu first").
10. `DELETE` with an empty body: `reqwest::Client::delete(url)` sends no body and no `Content-Type` unless set; set `Content-Type` explicitly to mirror Aniyomi.
11. Do not reuse `TrackRecord.remote_id` meaning across calls: it is the media id before `add`/`find` and the library-entry id after. Keep this invariant in one place (`bind`) and document it on the field.
