# Area 31-tracker-shikimori-bangumi, part 1: Rust design

Target: Linux, GTK 4.16+ through `gtk4` 0.11 (no libadwaita), `tokio`, `reqwest`, `serde`. Layout follows `plans/27-tracking-core/p1-rust-design.md` section 1: this area adds two modules inside the `kuro-track` crate. Behaviour that the types must reproduce is in p1-behavior.md. Wire shapes are in p1-data-and-config.md. Edge cases and tests are in p1-edge-cases-and-tests.md.

Contents: 1 layout; 2 crates; 3 types and trait implementations; 4 wire layer (DTOs and leniency); 5 auth, tokens and secrets; 6 HTTP requests; 7 assets; 8 async and threading; 9 Android-to-Linux mapping; 10 error model; 11 mockup integration; 12 pitfalls and open decisions.

---

## 1. Layout

```
crates/kuro-track/src/
  shikimori/
    mod.rs          ShikimoriTracker: Tracker, MangaTracker, AnimeTracker, Deletable*   (Shikimori.kt)
    api.rs          ShikimoriApi: every request in p1-data-and-config.md 7.1           (ShikimoriApi.kt)
    auth.rs         ShikimoriSession: bearer, refresh, persistence                     (ShikimoriInterceptor.kt)
    status.rs       ShikimoriStatus enum, API strings, labels                          (ShikimoriUtils.kt)
    dto.rs          serde structs for S1..S11 responses                                (dto/*.kt)
    search.rs       SMEntry -> search result mapping                                   (dto/SMEntry.kt)
    tests/          fixtures and wiremock tests (section 4.4)
  bangumi/
    mod.rs          BangumiTracker: Tracker, MangaTracker, AnimeTracker                (Bangumi.kt)
    api.rs          BangumiApi: every request in p1-data-and-config.md 7.2             (BangumiApi.kt)
    auth.rs         BangumiSession: bearer, refresh, persistence                       (BangumiInterceptor.kt)
    status.rs       BangumiStatus enum, collection types, labels                       (BangumiUtils.kt)
    dto.rs          serde structs for B1..B7 responses                                 (dto/*.kt)
    search.rs       BGMSubject -> search result mapping, summary composition            (dto/BGMSearch.kt)
    tests/
  oauth.rs          shared: authorise URL builder, redirect parsing (owned by area 27)
  error.rs          shared TrackError (owned by area 27; variants added in section 10)
```

Rule from area 27 (`plans/27-tracking-core/p1-rust-design.md`, section 1): `kuro-track` does not depend on GTK, `gio` or `keyring`. The two trackers receive `Arc<dyn SecretStore>` and an HTTP client, both injected.

---

## 2. Crates

| Crate | Used in | Why |
|---|---|---|
| `reqwest` 0.12 (`json`, `rustls-tls`, `gzip`, no default TLS) | `api.rs` of both trackers | replaces OkHttp (`ShikimoriApi.kt`, `BangumiApi.kt`); one client shared through `kuro-net` (area 12) |
| `serde`, `serde_json` | `dto.rs`, `auth.rs` | replaces `kotlinx.serialization` (`AppModule.kt:173-177`) |
| `tokio` (`sync`, `time`) | `auth.rs` | `Mutex` for single-flight refresh (section 5.3); no `spawn` inside trackers |
| `async-trait` | trait objects | same as area 27 (`Tracker` is used as `dyn`) |
| `thiserror` | `error.rs` | `TrackError` |
| `url` | `oauth.rs` | parse and build the authorise URL with exact parameter order (p1-data-and-config.md section 8) |
| `form_urlencoded` (via `url`) | token bodies | replaces `FormBody.Builder` (keeps the key order) |
| `time` | expiry checks | `OffsetDateTime::now_utc().unix_timestamp()` replaces `System.currentTimeMillis() / 1000` |
| `tracing` | logs | replaces `logcat(LogPriority.ERROR, e)` |
| `wiremock` (dev-dependency) | `tests/` | local HTTP server for request shape and status tests |
| `serde` derive with `#[serde(default)]` | DTOs | section 4.3 |

Not used: `once_cell` (use `std::sync::LazyLock`), `chrono` (use `time`), `async-std`.

---

## 3. Types and trait implementations

### 3.1 Shared types (area 27)

`TrackerId` (`i64`, a newtype in area 27), `Kind` (`Anime` | `Manga`), `MangaTrack` and `AnimeTrack` (area 27 `model.rs`). Field names follow p1-data-and-config.md section 2. This area reads and writes these fields only:

- `MangaTrack`: `remote_id: i64`, `library_id: Option<i64>`, `title: String`, `last_chapter_read: f64`, `total_chapters: i64`, `score: f64`, `status: i64`, `started_reading_date: i64`, `finished_reading_date: i64`, `tracking_url: String`, `private: bool`.
- `AnimeTrack`: same with `last_episode_seen`, `total_episodes`, `started_watching_date`, `finished_watching_date`.

### 3.2 Status enums (replaces numeric constants)

Persisted numbers are kept as explicit discriminants (p1-behavior.md section 1.1). Two separate enums, so the two trackers cannot be mixed up.

```rust
// crates/kuro-track/src/shikimori/status.rs
use crate::error::TrackError;

/// Internal status, persisted in manga_sync.status / anime_sync.status for sync_id 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum ShikiStatus {
    Reading = 1,
    Completed = 2,
    OnHold = 3,
    Dropped = 4,
    PlanToRead = 5,
    Rereading = 6,
}

impl ShikiStatus {
    pub const ALL: [ShikiStatus; 6] = [
        ShikiStatus::Reading, ShikiStatus::Completed, ShikiStatus::OnHold,
        ShikiStatus::Dropped, ShikiStatus::PlanToRead, ShikiStatus::Rereading,
    ];

    pub fn from_internal(v: i64) -> Option<Self> {
        Self::ALL.into_iter().find(|s| *s as i64 == v)
    }

    /// Wire string sent in `user_rate.status` (ShikimoriUtils.kt:6-24).
    pub const fn api_str(self) -> &'static str {
        match self {
            Self::Reading => "watching",
            Self::Completed => "completed",
            Self::OnHold => "on_hold",
            Self::Dropped => "dropped",
            Self::PlanToRead => "planned",
            Self::Rereading => "rewatching",
        }
    }

    /// Inverse of `api_str` (ShikimoriUtils.kt:26-34). Unknown strings are an error,
    /// never a panic (defect D4).
    pub fn from_api(s: &str) -> Result<Self, TrackError> {
        Ok(match s {
            "watching" => Self::Reading,
            "completed" => Self::Completed,
            "on_hold" => Self::OnHold,
            "dropped" => Self::Dropped,
            "planned" => Self::PlanToRead,
            "rewatching" => Self::Rereading,
            other => return Err(TrackError::UnknownStatus(other.to_owned())),
        })
    }
}
```

The Bangumi enum follows the same shape (`crates/kuro-track/src/bangumi/status.rs`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum BgmStatus { PlanToRead = 1, Completed = 2, Reading = 3, OnHold = 4, Dropped = 5 }

impl BgmStatus {
    pub const ALL: [BgmStatus; 5] = [Self::PlanToRead, Self::Completed, Self::Reading, Self::OnHold, Self::Dropped];
    pub fn from_internal(v: i64) -> Option<Self> { Self::ALL.into_iter().find(|s| *s as i64 == v) }
    /// Value of `type` in B6/B7 bodies and in B5 responses (BangumiUtils.kt:6-22, BGMCollectionResponse.kt:18-25).
    pub const fn collection_type(self) -> i64 { self as i64 }
    pub fn from_collection_type(t: Option<i64>) -> Result<Self, TrackError> {
        match t.and_then(Self::from_internal) {
            Some(s) => Ok(s),
            None => Err(TrackError::UnknownStatus(format!("{t:?}"))),
        }
    }
}
```

Note: for Bangumi the collection type and the internal value are the same number (1..5), so `collection_type` is the identity. This is intentional and matches the Kotlin, but the two must stay separate functions so that a future change to one does not silently change the other.

Labels (both trackers) return a message key, not a string. Key names are the MR and AYMR names (area 27 section 3.7 defines `MsgKey`):

```rust
pub enum StatusLabel { Reading, Completed, OnHold, Dropped, PlanToRead, Rereading, Watching, PlanToWatch, RewatchingAnime }
// Shikimori manga: Reading->MR.reading, PlanToRead->MR.plan_to_read, Completed->MR.completed,
//   OnHold->MR.on_hold, Dropped->MR.dropped, Rereading->MR.repeating
// Shikimori anime: Reading->AYMR.watching, PlanToRead->AYMR.plan_to_watch, Completed->MR.completed,
//   OnHold->MR.on_hold, Dropped->MR.dropped, Rereading->AYMR.repeating_anime
// Bangumi: as above without Rereading (returns None for it).
```

### 3.3 Tracker structs

```rust
// crates/kuro-track/src/shikimori/mod.rs
pub struct ShikimoriTracker {
    base: BaseTracker,                      // area 27: id, name, secrets, prefs, login watch channel
    http: reqwest::Client,                  // the shared client from kuro-net (area 12)
    session: ShikimoriSession,              // auth.rs
    api: ShikimoriApi,                      // api.rs, holds `http` clone and `session`
    config: Arc<ShikimoriClientConfig>,     // client_id and client_secret, section 5.1
}

pub const SHIKIMORI_ID: TrackerId = TrackerId(4);

#[async_trait::async_trait]
impl Tracker for ShikimoriTracker {
    fn id(&self) -> TrackerId { SHIKIMORI_ID }
    fn name(&self) -> &str { "Shikimori" }
    fn logo(&self) -> LogoRef { LogoRef::Embedded(include_bytes!("../../../../assets/tracker/shikimori.png")) }
    fn logo_color(&self) -> u32 { 0xFF28_2828 }          // Color.rgb(40,40,40) (Shikimori.kt:181)
    fn supports_private_tracking(&self) -> bool { false } // BaseTracker default
    fn completion_status(&self) -> i64 { ShikiStatus::Completed as i64 }
    fn score_list(&self) -> &ScoreList { &SCORE_LIST_0_10 }
    async fn login(&self, _username: &str, password: &str) -> Result<(), TrackError> {
        // Kotlin: login(username, password) = login(password) (Shikimori.kt:221). `password` is the OAuth code.
        self.login_with_code(password).await
    }
    fn logout(&self) { self.base.logout(); self.session.clear(); }   // order: section 5.4
    fn is_logged_in(&self) -> bool { self.base.is_logged_in() }
    fn login_state(&self) -> tokio::sync::watch::Receiver<bool> { self.base.login_state() }
    fn save_credentials(&self, u: &str, p: &str) { let _ = self.base.save_credentials(u, p); }
    fn username(&self) -> String { self.base.username() }
    fn password(&self) -> String { self.base.password() }
    fn as_anime(&self) -> Option<&dyn AnimeTracker> { Some(self) }
    fn as_manga(&self) -> Option<&dyn MangaTracker> { Some(self) }
    fn as_deletable_anime(&self) -> Option<&dyn DeletableAnimeTracker> { Some(self) }
    fn as_deletable_manga(&self) -> Option<&dyn DeletableMangaTracker> { Some(self) }
}

pub const BANGUMI_ID: TrackerId = TrackerId(5);
// BangumiTracker mirrors ShikimoriTracker, with supports_private_tracking() = true,
// logo colour 0xFFF0_9199 (Color.rgb(240,145,153), Bangumi.kt:150), no deletable traits,
// rereading_status() = -1 and rewatching_status() = -1 (Bangumi.kt:182-184).
```

`ScoreList` values: `["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]`, 11 items (`Shikimori.kt:44-46`, `Bangumi.kt:229-231`). A single `static SCORE_LIST_0_10: LazyLock<ScoreList>` in area 27 serves both.

Manga and anime trait methods are implemented on the same struct. The shared logic lives in free functions so the two kinds cannot drift apart:

```rust
// crates/kuro-track/src/shikimori/mod.rs (internal)
fn next_status_after_read(current: ShikiStatus, did_read: bool, last: f64, total: i64) -> ShikiStatus {
    // Shikimori.kt:77-89 (manga) and 91-103 (anime), exact.
    if current == ShikiStatus::Completed || !did_read { return current; }
    if last as i64 == total && total > 0 { return ShikiStatus::Completed; }   // toLong() truncates
    if current == ShikiStatus::Rereading { return current; }
    ShikiStatus::Reading
}
```

`last as i64` truncates toward zero, the same as Kotlin `Double.toLong()` for finite values. Non-finite values are rejected earlier (section 4.3).

Bangumi's version has no rereading guard (`Bangumi.kt:56-68`):

```rust
fn bgm_next_status_after_read(current: BgmStatus, did_read: bool, last: f64, total: i64) -> BgmStatus {
    if current == BgmStatus::Completed || !did_read { return current; }
    if last as i64 == total && total > 0 { BgmStatus::Completed } else { BgmStatus::Reading }
}
```

Bind rules (shared shape, p1-behavior.md 2.10 and 3.10) are free functions in each `mod.rs` because they need the status enum.

### 3.4 Remote value types (what `api.rs` returns)

The remote track is not a `MangaTrack`. Area 27 expects `bind` and `refresh` to mutate a `&mut MangaTrack`. Both trackers therefore fetch into a plain value and copy explicitly. This removes defect D5.

```rust
// crates/kuro-track/src/shikimori/api.rs
pub(crate) struct ShikiLibEntry {
    pub library_id: i64,             // user-rate id
    pub target_id: i64,              // catalogue id from the catalogue record
    pub title: String,               // catalogue name
    pub status: ShikiStatus,
    pub score: f64,                  // 0..10 as stored remotely
    pub progress: f64,               // chapters (manga) or episodes (anime)
    pub total: Option<i64>,          // catalogue chapters or episodes, None when null
    pub tracking_url: String,        // "https://shikimori.one" + url
}
```

```rust
// crates/kuro-track/src/bangumi/api.rs
pub(crate) struct BgmLibEntry {
    pub subject_id: i64,
    pub status: BgmStatus,
    pub score: f64,                  // rate, 0 when absent
    pub progress: f64,               // ep_status, 0 when absent
    pub total: i64,                  // subject.eps, 0 when absent
}
```

### 3.5 Copy rules (explicit)

Replaces `copyPersonalFrom` from area 27 for these two trackers. The rules come from p1-behavior.md 2.10, 2.13 and 3.10:

```rust
// Shikimori: copy from ShikiLibEntry into MangaTrack
fn copy_shiki_personal(t: &mut MangaTrack, r: &ShikiLibEntry) {
    t.last_chapter_read = r.progress;
    t.score = r.score;
    t.status = r.status as i64;
    // Dates and private are NOT copied. Kotlin copies them, which erases local dates (defect D22).
}

// Bangumi: copy from BgmLibEntry into MangaTrack
fn copy_bgm_personal(t: &mut MangaTrack, r: &BgmLibEntry) {
    t.last_chapter_read = r.progress;
    t.score = r.score;
    t.status = r.status as i64;
    t.total_chapters = r.total;
    // private is never read back (defect D14); library_id stays as it was.
}
```

Copying only progress, score and status is a deliberate deviation for Shikimori (dates). It is stated in p1-behavior.md D22 and tested in EC-S-14 (p1-edge-cases-and-tests.md).

---

## 4. Wire layer (DTOs and leniency)

### 4.1 Shikimori structs

```rust
// crates/kuro-track/src/shikimori/dto.rs
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct SmOAuth {
    pub access_token: String,
    pub token_type: String,
    pub created_at: i64,
    pub expires_in: i64,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SmUser { pub id: i64 }                       // Int in Kotlin, i64 here

#[derive(Debug, Deserialize)]
pub(crate) struct SmAddEntryResponse { pub id: i64 }

#[derive(Debug, Deserialize)]
pub(crate) struct SmEntry {
    pub id: i64,
    pub name: String,
    #[serde(default)] pub chapters: Option<i64>,
    #[serde(default)] pub episodes: Option<i64>,
    pub image: SmCover,
    pub score: f64,
    pub url: String,
    pub status: String,
    pub kind: String,
    #[serde(default)] pub aired_on: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SmCover { pub preview: String }

#[derive(Debug, Deserialize)]
pub(crate) struct SmUserListEntry {
    pub id: i64,
    pub chapters: f64,
    pub episodes: f64,
    pub score: i64,
    pub status: String,
}
```

Notes:
- `SmOAuth.refresh_token` is `Option` with `serde(default)`, so an absent key is `None`. Matches the Kotlin `explicitNulls = false` decode (p1-data-and-config.md section 1).
- `SmUserListEntry.score` is `i64` (the Kotlin `Int`). The server clamps to 0..10 (D8), so a value outside that range is still accepted as read.
- `SmEntry.chapters` and `episodes` are `Option` with `serde(default)`, not `Option` without default, because the Kotlin already treats them as nullable and fails only at the `!!` (D21). The Rust mapping returns an error instead of a panic.

### 4.2 Bangumi structs

```rust
// crates/kuro-track/src/bangumi/dto.rs
#[derive(Debug, Deserialize)]
pub(crate) struct BgmOAuth {
    pub access_token: String,
    pub token_type: String,
    #[serde(default)]
    pub created_at: Option<i64>,     // absent on responses; set to now on decode (see 5.2)
    pub expires_in: i64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub user_id: Option<i64>,        // never read; kept so the stored JSON round-trips
}

#[derive(Debug, Deserialize)]
pub(crate) struct BgmUser { pub username: String }

#[derive(Debug, Deserialize)]
pub(crate) struct BgmCollection {
    #[serde(default)] pub rate: Option<i64>,
    #[serde(default, rename = "type")] pub kind: Option<i64>,
    #[serde(default)] pub ep_status: Option<i64>,
    #[serde(default)] pub vol_status: Option<i64>,
    #[serde(default)] pub private: bool,
    #[serde(default)] pub subject: Option<BgmSlimSubject>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BgmSlimSubject {
    #[serde(default)] pub volumes: Option<i64>,
    #[serde(default)] pub eps: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BgmSearchResult {
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    #[serde(default)]
    pub data: Vec<BgmSubject>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BgmSubject {
    pub id: i64,
    pub name_cn: String,             // required (D20)
    pub name: String,                // required
    #[serde(default)] pub summary: Option<String>,
    #[serde(default)] pub date: Option<String>,
    #[serde(default)] pub images: Option<BgmImages>,
    #[serde(default)] pub volumes: i64,
    #[serde(default)] pub eps: i64,
    #[serde(default)] pub rating: Option<BgmRating>,
    #[serde(default)] pub platform: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BgmImages { #[serde(default)] pub common: Option<String> }

#[derive(Debug, Deserialize)]
pub(crate) struct BgmRating { #[serde(default)] pub score: Option<f64> }
```

`BgmCollection.kind` is named `kind` in Rust because `type` is a keyword; the serde rename keeps the wire name.

### 4.3 Leniency rules

These apply to all 14 DTOs listed in p1-inventory.md.

1. Required in the Kotlin (non-null, no default) → required in Rust (no `serde(default)`). Examples: `id`, `name`, `name_cn`, `status`, `kind`, `score`, `url`, `image` (`SMEntry`), `expires_in`, `access_token`, `token_type`, `total`, `limit`, `offset`, `username`.
2. Nullable in the Kotlin → `Option<T>` with `#[serde(default)]`. The Kotlin decodes an absent nullable key as null (`explicitNulls = false`), so this matches.
3. Defaults in the Kotlin (`= 0`, `= false`, `= emptyList()`, `= null`) → `#[serde(default)]` with the same default value for the non-`Option` fields.
4. Floats: reject `NaN` and infinity after decoding (serde_json already rejects them in JSON). Reject `score < 0` or `> 10` on read with a log line, but keep the value so the UI shows what the server sent (the Kotlin does not check).
5. Unknown keys are ignored (serde default for structs without `deny_unknown_fields`). Never add `deny_unknown_fields`.
6. `SMEntry.kind` and `status` are passed through as strings. Do not turn them into enums (the catalogue has values this area does not know).

### 4.4 Fixtures and tests

- Fixtures under `crates/kuro-track/tests/fixtures/shikimori/` and `crates/kuro-track/tests/fixtures/bangumi/`. One JSON file per response type in p1-data-and-config.md sections 1 and 7. Each fixture is written from the Kotlin DTO fields and the server schemas; none are copied from a live account.
- Tests use `wiremock`. Each test starts a `MockServer`, points the tracker's base URLs at it (the base URLs are constructor arguments, default to the production constants), and asserts method, path, query order, headers and body.

---

## 5. Auth, tokens and secrets

### 5.1 Client configuration (decision D-OPEN-1)

```rust
// crates/kuro-track/src/oauth.rs (area 27 owns the file; this area adds the struct)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OAuthClient {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct TrackClientConfig {
    #[serde(default)] pub shikimori: Option<OAuthClient>,
    #[serde(default)] pub bangumi: Option<OAuthClient>,
}
```

Loaded from the user's config file (area 02), section `[track.clients]`. When an entry is absent, the tracker reports `TrackError::NotConfigured("Shikimori")` on login and login rows show "not configured". The Aniyomi client values are not in Kurobayomi source (D-OPEN-1 and D11).

Redirect URI: the `aniyomi://` scheme from the Kotlin is not decided here. Area 27 chooses the scheme (`plans/27-tracking-core/p1-topic-tracker-oauth.md`, section 3). This area takes the redirect string from `oauth.rs` as a constant parameter `redirect_uri: &str`, so the choice is made once.

### 5.2 Session (replaces `ShikimoriInterceptor` and `BangumiInterceptor`)

```rust
// crates/kuro-track/src/shikimori/auth.rs (Bangumi has the same shape in bangumi/auth.rs)
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct StoredToken {
    pub access_token: String,
    pub token_type: String,
    pub created_at: i64,              // unix seconds
    pub expires_in: i64,
    pub refresh_token: Option<String>,
}

pub(crate) struct ShikimoriSession {
    slot: Mutex<Option<StoredToken>>, // lock is held for the whole refresh (single flight)
    secrets: Arc<dyn SecretStore>,
    http: reqwest::Client,            // raw client for the token endpoint (not the interceptor chain)
    client: OAuthClient,
    token_url: String,
    auth_expired: Arc<dyn AuthExpiredFlag>,
    account: String,                  // "4.token"
    clock: Arc<dyn Clock>,            // injectable for tests
}

impl ShikimoriSession {
    /// Equivalent of the Kotlin interceptor body (ShikimoriInterceptor.kt:20-43).
    pub(crate) async fn bearer(&self) -> Result<String, TrackError> {
        let mut slot = self.slot.lock().await;
        let token = slot.as_ref().ok_or(TrackError::NotLoggedIn)?;
        if !is_expired(token, self.clock.now_unix()) {
            return Ok(token.access_token.clone());
        }
        // Expired: refresh. Missing refresh token is checked here, not on every request (D3).
        let Some(refresh) = token.refresh_token.clone() else {
            self.auth_expired.set(true);
            return Err(TrackError::AuthExpired);
        };
        match self.refresh(&refresh).await {
            Ok(new_token) => {
                self.store(&new_token)?;
                let access = new_token.access_token.clone();
                *slot = Some(new_token);
                Ok(access)
            }
            Err(RefreshFailure::Rejected) => {
                // Server refused the refresh (400 or 401). Keep the credentials, flag the tracker (D1).
                self.auth_expired.set(true);
                Err(TrackError::AuthExpired)
            }
            Err(RefreshFailure::Transport(e)) => Err(TrackError::Http(e)),   // keep token, retry later
        }
    }

    async fn refresh(&self, refresh_token: &str) -> Result<StoredToken, RefreshFailure> {
        // Body order from ShikimoriApi.kt:267-275, see p1-data-and-config.md 7.1 S2.
        let body = form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &self.client.client_id),
            ("client_secret", &self.client.client_secret),
            ("refresh_token", refresh_token),
        ]);
        let resp = self.http.post(&self.token_url).body(body)
            .header("content-type", "application/x-www-form-urlencoded")
            .send().await.map_err(|e| RefreshFailure::Transport(e.into()))?;
        let status = resp.status();
        if status == reqwest::StatusCode::BAD_REQUEST || status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(RefreshFailure::Rejected);
        }
        if !status.is_success() {
            return Err(RefreshFailure::Transport(TrackError::Http(status.as_u16())));
        }
        let raw: SmOAuth = resp.json().await.map_err(|e| RefreshFailure::Transport(e.into()))?;
        Ok(raw.into_stored(self.clock.now_unix()))
    }

    /// Persist the whole token, not only the access token (Doorkeeper rotates refresh tokens).
    fn store(&self, t: &StoredToken) -> Result<(), TrackError> {
        let json = serde_json::to_string(t).map_err(|e| TrackError::Decode(e.to_string()))?;
        self.secrets.set(SECRET_SERVICE, &self.account, &json)?;
        Ok(())
    }

    pub(crate) fn clear(&self) {
        // Logout deletes the entry. Never writes the string "null" (D10).
        let _ = self.secrets.delete(SECRET_SERVICE, &self.account);
        // Clear the in-memory slot without awaiting. If a refresh holds the lock, the next
        // bearer() call still finds no secret on restart; see EC-S-22.
    }
}

pub(crate) const SECRET_SERVICE: &str = "kurobayomi.track";

pub(crate) fn is_expired(t: &StoredToken, now: i64) -> bool {
    // SMOAuth.kt:21: now > createdAt + expiresIn - 3600
    now > t.created_at + t.expires_in - 3600
}

pub(crate) enum RefreshFailure { Rejected, Transport(TrackError) }

impl SmOAuth {
    /// `created_at` comes from the server; the fallback is only used if it is absent (never for Shikimori).
    pub(crate) fn into_stored(self, now: i64) -> StoredToken {
        StoredToken {
            access_token: self.access_token,
            token_type: self.token_type,
            created_at: if self.created_at > 0 { self.created_at } else { now },
            expires_in: self.expires_in,
            refresh_token: self.refresh_token,
        }
    }
}
```

Rules that the code above implements:
- The lock is held across the refresh. A second caller waits, then re-reads the token: single flight. This fixes D2.
- The refresh request uses `self.http` directly, not the interceptor chain. This fixes D16.
- A 400 or 401 on refresh sets the auth-expired flag and returns `AuthExpired`. Any other failure returns `Http` or `Transport` and keeps the token, so a later call can retry. This fixes D1.
- `clear` uses `try_lock`. If a refresh is in progress during logout, the slot is left as it is and the secret entry is deleted. The next `bearer` call then fails at `secrets.get` on startup (the restore below), so the stale in-memory token is never used past its refresh. Tested in EC-S-22.

Bangumi's `refresh` body differs: it includes `redirect_uri` (p1-data-and-config.md 7.2 B2). Bangumi's `created_at` is set to `clock.now_unix()` when a token response is decoded, because the response has no `created_at` (`BGMOAuth.kt:14-15`).

### 5.3 Restore at startup

Replaces the lazy `restoreToken()` (`ShikimoriInterceptor.kt:18`):

```rust
impl ShikimoriSession {
    pub(crate) fn restore(secrets: Arc<dyn SecretStore>, account: &str) -> Option<StoredToken> {
        let raw = secrets.get(SECRET_SERVICE, account).ok()??;
        serde_json::from_str::<StoredToken>(&raw).ok()    // any failure -> logged out, as in the Kotlin (returns null)
    }
}
```

Restore happens in the tracker constructor, not lazily, so `is_logged_in` is correct before the first request. The Kotlin restores lazily, which is equivalent for `isLoggedIn` because `isLoggedIn` reads the username and password preferences, not the token.

### 5.4 Logout order

The Kotlin order is `super.logout()`, `trackToken.delete()`, `newAuth(null)` (`Shikimori.kt:246-250`). The Rust order:
1. `base.logout()`: clear username and password secrets, clear the auth-expired flag, publish `false` (area 27 section 5.2).
2. `session.clear()`: delete the token secret, clear the in-memory slot.

Step 1 first means the UI stops showing the account immediately even if the secret deletion fails. A failed delete is logged, not shown.

### 5.5 Credentials

`save_credentials(username, password)` writes `username` (Shikimori: numeric id string; Bangumi: login name), then `password` (the access token). Both go through `SecretStore` (area 27 section 5.1 naming: `<id>.username`, `<id>.password`). Proposal: write both within one `set` sequence and publish login state only after both succeed (area 27 section 5.2 already proposes this).

---

## 6. HTTP requests

### 6.1 Shared client and headers

- One `reqwest::Client` from `kuro-net` (area 12). Per-tracker code does not build its own client. Timeouts and proxy come from there.
- Headers added per request:
  - `Authorization: Bearer <token>` on authenticated calls, from `session.bearer().await?`.
  - `User-Agent` from `user_agent()` below (section 6.2).
  - `Content-Type` exactly as the Kotlin: `application/json; charset=utf-8` for Shikimori POST (`jsonMime`), `application/json` for Bangumi POST and PATCH (`APP_JSON`), `application/x-www-form-urlencoded` for token bodies (`FormBody`).
- Cache: every GET uses `Cache-Control: no-cache` (defect D12). The Kotlin used the 10-minute default for Shikimori (`Requests.kt:16`); this is a deliberate change.

### 6.2 User-Agent

```rust
// crates/kuro-track/src/lib.rs
pub fn user_agent() -> String {
    format!(
        "Kurobayomi/{} ({}) ({})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,          // "linux"
        env!("CARGO_PKG_REPOSITORY"),  // set in the crate's Cargo.toml `repository`
    )
}
```

Bangumi requires a descriptive UA with a developer id and app name and forbids generic values such as `Bangumi/1.0` (`docs-raw/user agent.md`). The format above includes the project repository from `Cargo.toml`, which satisfies that rule. Shikimori has no UA rule in the source, so the same value is used.

### 6.3 Request builders (one function per row of p1-data-and-config.md section 7)

The request body is built with `serde_json::json!` in key order. `serde_json` preserves insertion order only with the `preserve_order` feature, so enable it for `kuro-track` (`serde_json = { version = "1", features = ["preserve_order"] }`). This keeps the body key order identical to the Kotlin, which matters for the S10 and B6 tests.

```rust
// Shikimori S10 body (ShikimoriApi.kt:43-53), key order as built
fn shiki_user_rate_body(user_id: &str, target_id: i64, kind: Kind, progress: f64, score: f64, status: ShikiStatus) -> serde_json::Value {
    let (target_type, progress_key) = match kind {
        Kind::Manga => ("Manga", "chapters"),
        Kind::Anime => ("Anime", "episodes"),
    };
    serde_json::json!({
        "user_rate": {
            "user_id": user_id,
            "target_id": target_id,
            "target_type": target_type,
            progress_key: progress as i64,   // Kotlin .toInt(): truncation
            "score": score as i64,           // unclamped in Kotlin (D8); clamped in Rust, see 6.4
            "status": status.api_str(),
        }
    })
}
```

Note: `progress as i64` and `score as i64` truncate toward zero, the same as Kotlin `.toInt()`. Rust `as` saturates on overflow, while Kotlin wraps. Values in range are identical; no real value overflows.

```rust
// Bangumi B6 / B7 body (BangumiApi.kt:45-52, 81-88), key order as built
fn bgm_collection_body(status: BgmStatus, score: f64, progress: f64, private: bool) -> serde_json::Value {
    serde_json::json!({
        "type": status.collection_type(),
        "rate": (score as i64).clamp(0, 10),   // Kotlin: toInt().coerceIn(0, 10)
        "ep_status": progress as i64,
        "private": private,
    })
}
```

### 6.4 Clamping decision

D8 (Shikimori score) is fixed in Rust: `score` is clamped to `0..=10` before the write, with negatives written as `0`. Behaviour change is noted in p1-behavior.md D8. Bangumi already clamps in the Kotlin.

### 6.5 Request sequences

The sequences are the Kotlin ones (p1-behavior.md), with these exceptions:
- `find_lib` (Shikimori) sends both requests in sequence, not in parallel, because the second needs no data from the first but the Kotlin order is the documented order. Keep sequential; the cost is one extra round trip.
- `search` (both trackers) is sent with the bearer if logged in and without it otherwise (proposal P-anon, D9). The Kotlin always sends the bearer and fails when logged out.

---

## 7. Assets

Logo files: `app/src/main/res/drawable-nodpi/ic_tracker_shikimori.webp` (2948 bytes) and `ic_tracker_bangumi.webp` (1354 bytes). In Kurobayomi:
- Convert each to a 48 px PNG and embed it with `include_bytes!` in `assets/tracker/shikimori.png` and `assets/tracker/bangumi.png` under the repository root.
- `LogoRef::Embedded(&'static [u8])` (area 27) carries the bytes; the GUI decodes with `gdk::Texture::from_bytes`.
- Display: 48 px tile (area 27 section 7 `logo.rs`), background tinted with `logo_color()`.
- Licence: the logos are Aniyomi's copies of the providers' marks. Before shipping, the user should confirm the provider marks may be redistributed (open question, p1-edge-cases-and-tests.md OQ-3).

---

## 8. Async and threading

- All network calls are `async` and run on the tokio runtime owned by `kuro-track` (area 27 section 4). Nothing blocks the GTK main loop.
- The GUI calls tracker methods through the dispatcher in area 27 (`TrackViewModel`), which sends results back over an `async-channel` to the GTK thread.
- `session.bearer()` holds a `tokio::sync::Mutex` across one HTTP request (refresh only). No other lock is held across an await.
- Login: the OAuth code is delivered by the redirect handler (area 27 `oauth.rs`). `login_with_code` runs on tokio and writes secrets; the GUI shows the result when the login-state watch channel changes.
- Cancellation: area 27's `TrackError::Cancelled` is produced when the caller drops the future. The session's `Mutex` is released on drop, so a cancelled refresh does not leave the slot locked.
- Retry and backoff for failed writes are in area 27 (`queue/retry.rs`). This area returns errors and does not retry on its own. The Kotlin does not retry either (p1-behavior.md 7.4).
- Clock: `Clock` is a trait with `now_unix() -> i64`. Production uses `time::OffsetDateTime::now_utc()`. Tests inject a fixed clock to exercise expiry without sleeping.

---

## 9. Android-to-Linux mapping

| Android or Kotlin mechanism (used in scope) | Linux or Rust replacement | Notes |
|---|---|---|
| `OkHttpClient`, `newBuilder().addInterceptor` | `reqwest::Client` + `ShikimoriSession::bearer` / `BangumiSession::bearer` | Interceptor logic moves to explicit calls, so ordering is visible. |
| `okhttp3.Interceptor` chain | explicit function calls | defect D16 (refresh bypasses outer interceptors) is fixed by construction. |
| `kotlinx.serialization` (`@Serializable`, `@SerialName`, `@EncodeDefault`, `Json`) | `serde` derive, `#[serde(rename)]`, `#[serde(default)]` | section 4.3. |
| `kotlinx.serialization.json.buildJsonObject` | `serde_json::json!` with `preserve_order` | section 6.3. |
| `Uri.buildUpon().appendQueryParameter` | `url::Url` with `query_pairs_mut` | keep the parameter order (p1-data-and-config.md section 8). |
| `FormBody.Builder` | `form_urlencoded::Serializer` (via `url`) | key order preserved. |
| `CacheControl.FORCE_NETWORK` / default 10-minute cache | `Cache-Control: no-cache` header | D12. |
| `BuildConfig.VERSION_NAME`, `APPLICATION_ID` | `env!("CARGO_PKG_VERSION")`, `env!("CARGO_PKG_REPOSITORY")` | section 6.2. |
| `System.currentTimeMillis() / 1000` | `time::OffsetDateTime::now_utc().unix_timestamp()` behind `Clock` | section 8. |
| `Preference` (`track_token_N`, `pref_mangasync_*`) | `SecretStore` (`keyring`, Secret Service) and `kuro-prefs` | area 27 section 5 and data doc section 10. |
| `Json.encodeToString(null)` leaving `"null"` | delete the secret | D10. |
| `Color.rgb(40, 40, 40)` | `0xFF282828` | section 3.3. |
| `R.drawable.ic_tracker_*` (webp) | `include_bytes!` PNG | section 7. |
| `dev.icerock.moko.resources.StringResource` | `MsgKey` (area 27 section 3.7) | section 10. |
| `injekt` `injectLazy` | constructor injection | section 11. |
| `withIOContext` | `tokio::task` (no explicit dispatcher) | section 8. |
| `logcat(LogPriority.ERROR, e)` | `tracing::error!` with the same message | |
| `openInBrowser(url, forceDefaultBrowser = true)` | `gio::AppInfo::launch_default_for_uri` (area 27, `Opener` trait) | `forceDefaultBrowser` has no Linux equivalent that avoids re-entering Kurobayomi; the default handler is used. |
| `TrackLoginActivity` + `aniyomi://` intent filter | `.desktop` `MimeType` + redirect handler (area 27) | area 27 decides the scheme. |

Crates for the mapping: `reqwest`, `serde`, `serde_json`, `url`, `time`, `keyring`, `tracing`, `thiserror`, `async-trait`, `tokio`, `wiremock` (tests).

---

## 10. Error model

`TrackError` is area 27's enum (`crates/kuro-track/src/error.rs`). This area adds these variants (area 31 owns the list, area 27 owns the enum):

```rust
// added to crates/kuro-track/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    // area 27 variants, unchanged: Message(String), NotLoggedIn, Unauthorised, Offline, Http(reqwest::Error), Db(kuro_db::DbError), Cancelled
    #[error("Not authenticated with {0}")]
    NotAuthenticated(&'static str),        // "Not authenticated with Shikimori" (Shikimori.kt via ShikimoriInterceptor.kt:23)
    #[error("authentication expired")]
    AuthExpired,                           // D1: refresh rejected
    #[error("HTTP error {0}")]
    HttpStatus(u16),                       // HttpException text (HttpException.kt:13)
    #[error("Too many entries in response")]
    TooManyEntries,                        // ShikimoriApi.kt:177-179, 209-211
    #[error("Could not find {0}")]
    NotFound(&'static str),                // "manga" or "anime" (Shikimori.kt:166, Bangumi.kt:137)
    #[error("Unknown status: {0}")]
    UnknownStatus(String),                 // D4: returns an error, never panics
    #[error("Missing library id")]
    NoLibraryId,                           // D18
    #[error("Tracker not configured: {0}")]
    NotConfigured(&'static str),           // D-OPEN-1
    #[error("Invalid response: {0}")]
    Decode(String),
}
```

Mapping to user messages (proposal; the Kotlin shows the raw text):
- `NotAuthenticated`, `TooManyEntries`, `NotFound`, `UnknownStatus`, `NoLibraryId`, `NotConfigured`, `Decode` → `MsgKey::TrackerError { tracker, detail }` (`MR.strings.track_error`, "%1$s error: %2$s") with a fixed detail key per variant. Localised detail text is provided by area 51.
- `AuthExpired` → a dedicated key (`track.err.auth_expired`), with an action to log in again. This is new (D1).
- `HttpStatus(401)` → the same auth-expired key when the session is authenticated, so an expired token that the refresh did not catch is still reported as auth expired.
- `HttpStatus(n)` otherwise → `MR.strings.exception_http` ("HTTP %d, check website in WebView").
- `Offline` and `Cancelled` → as area 27 defines.

---

## 11. Mockup integration

Current mockup (`/home/blackbox/kurobayomi/src/entry.rs`):
- `tracking()` (lines 498-537) hard-codes three rows `AniList`, `MyAnimeList` and `Kitsu` with `("AniList", true)`, `("MyAnimeList", false)` and `("Kitsu", false)`, and a fixed `Watching`, `12 / 24`, `8.0` grid.
- `tracker_search()` (lines 539-572) shows four fixed rows `title_n(i)` with `Author name`, `Type: Manga`, `Started: 2020-01-01`, `Status: Publishing`, `Score: 8.4`. Its `Track` and `Track privately` buttons do nothing.
- `src/settings.rs` and `spec/settings.json` (line 34) define `settings_tracking`, but the mockup does not list Shikimori or Bangumi.

Integration for this area (on top of area 27's `src/track/` module):
1. `tracking()` iterates the tracker registry (`TrackerManager::trackers()`, area 27) and renders one row per tracker in registry order (`TrackerManager.kt:39-42`: `myAnimeList, aniList, kitsu, shikimori, bangumi, komga, mangaUpdates, kavita, suwayomi, simkl, jellyfin`). The logo tile uses `LogoRef`. Shikimori and Bangumi appear as normal rows; there is no special case.
2. `tracker_search()` takes results from `search()` of the chosen tracker. For Shikimori the row shows `Type` (`publishing_type`), `Started` (`start_date`), `Status` (`publishing_status`), `Score` (`score`, one decimal). For Bangumi it shows the same fields from `BGMSubject` (`date`, `score` or `-`). When `score` is `-1.0`, show `-` (not `-1.0`); this is a display rule, not a data change.
3. "Track privately" (`view-conceal-symbolic`, `entry.rs:564`) is shown only when `supports_private_tracking()` is true, which is Bangumi only (p1-behavior.md 3.14). The mockup shows it always.
4. The remove dialog's "Also remove from <tracker>" checkbox appears only when the tracker implements `DeletableMangaTracker` or `DeletableAnimeTracker`, which is Shikimori only (`entry.rs` `remove_tracking` dialog, owned by area 27).
5. Score picker uses the `ScoreList` of the tracker (0..10 for both).
6. Status picker uses `status_list()` in the order given in p1-data-and-config.md section 3: Shikimori `[1,2,3,4,5,6]`, Bangumi `[3,2,4,5,1]`.

Mockup gaps that this area adds (to be implemented in `src/track/`): the registry-driven rows, the `search()` result rows with the real fields, the conditional private button, the conditional delete checkbox, and the login row for each tracker. The static strings in `entry.rs` are placeholders from the mockup and are not kept.

---

## 12. Pitfalls and open decisions

1. The POST in Shikimori is an upsert. One `user_rate_upsert` function is used by both add and update (p1-behavior.md 2.11). Do not introduce a PATCH.
2. Keep the two status enums separate (section 3.2). The numbers collide: `1` is `READING` for Shikimori and `PLAN_TO_READ` for Bangumi.
3. `remote_id` is the catalogue id on every path. `library_id` is the user-rate id (Shikimori) or null (Bangumi). Do not overwrite one with the other (D17).
4. Do not copy dates from the remote entry for Shikimori (D22). Do not copy `private` for Shikimori (never supported).
5. Bangumi `ep_status` for anime is written as the Kotlin does (parity), with the risk in D7. The episode endpoint is P2 (T-31a-044).
6. A stored refresh token rotates on every refresh. Persist the whole response. Never persist only the access token.
7. `score = -1` in search results must not reach a write. The write path clamps (section 6.4).
8. The OAuth clients (D-OPEN-1) are configuration. The Aniyomi constants are not copied.
9. The redirect scheme is area 27's decision. This area takes the string as a parameter.
10. Tests need a `Clock` seam (section 8). Without it, expiry tests become slow or flaky.
11. `serde_json` `preserve_order` must be enabled for `kuro-track`, otherwise request-body tests fail for key order.
12. Login errors are silent in the Kotlin. In Rust they are returned to the caller and shown as a toast, so the user learns why a login failed. This is a deliberate change (p1-behavior.md section 5).
