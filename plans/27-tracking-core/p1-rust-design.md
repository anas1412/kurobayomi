# Area 27-tracking-core, part 1: Rust design

Target: Linux, GTK 4.16+ through `gtk4` 0.11 (no libadwaita), `tokio`, `rusqlite` (bundled), `reqwest` (area 12), `serde`. Layout follows `plans/01-foundation-app-bootstrap/p1-rust-design.md` section 1. Behaviour that these types must reproduce is in `p1-behavior.md`; the field list is in `p1-data-and-config.md`.

Contents: 1 layout; 2 crates; 3 key types and traits; 4 async model; 5 secrets and login state; 6 retry scheduler; 7 UI integration with the mockup; 8 Android-to-Linux mapping; 9 error handling; 10 pitfalls.

---

## 1. Layout

```
crates/
  kuro-track/                      NEW  tracking logic (no GTK, no OS code)
    src/lib.rs
    src/model.rs                   AnimeTrack, MangaTrack, TrackId, search models, Kind
    src/model/search.rs            AnimeTrackSearch, MangaTrackSearch
    src/tracker.rs                 Tracker, AnimeTracker, MangaTracker, Enhanced*, Deletable*
    src/base.rs                    BaseTracker helper (credential state, login watch channel)
    src/manager.rs                 TrackerManager (registry, loggedIn, watch stream)
    src/policy.rs                  AutoTrackState, mark-read decision
    src/prefs.rs                   TrackPrefs over kuro-prefs keys (section 2 of data doc)
    src/remote.rs                  remote setter rules (status, progress, score, dates, private)
    src/bind.rs                    bind flow (anime and manga), bind_enhanced
    src/sync.rs                    refresh_all, sync_progress (enhanced continuous rule)
    src/progress.rs                track_episode, track_chapter (online, offline, queue)
    src/queue/mod.rs               RetryQueue (kinds, upsert, remove, list)
    src/queue/store.rs             SQL on track_retry_queue (section 7.2 of data doc)
    src/queue/retry.rs             RetryScheduler (attempts, backoff, network wait)
    src/oauth.rs                   AuthRequest, PKCE verifier/challenge, redirect parsing
    src/error.rs                   TrackError
  kuro-db/ (area 05)               TrackRepository impl for anime_sync and manga_sync
  kuro-net/ (area 12)              HTTP client handed to trackers
  kuro-platform/ (bootstrap)       SecretStore, Opener, Connectivity (see section 5, 8)
  kuro-testkit/ (bootstrap)        FakeTracker, in-memory repos, fixtures for track tests
src/                               GUI crate (kurobayomi)
  track/mod.rs                     NEW  glue: TrackViewModel, action dispatch to tokio
  track/settings.rs                NEW  Settings > Tracking page (rows from TrackPrefs table)
  track/ui/info.rs                 NEW  per-entry tracking sheet (replaces static tracking())
  track/ui/search.rs               NEW  tracker search page (replaces tracker_search())
  track/ui/selectors.rs            NEW  status list, number wheel, score list, date picker
  track/ui/logo.rs                 NEW  48 px logo tile
  track/ui/login.rs                NEW  credential dialog, logout dialog
  track/ui/remove.rs               NEW  remove-tracking dialog
  entry.rs                         EDIT tracking(), tracker_search() call into track::ui
```

Rule from the cross-platform addendum: only `kuro-platform` touches OS APIs. `kuro-track` must not depend on GTK, `gio` or `libsecret` directly. It uses the traits in section 5 and 8.

---

## 2. Crates and why

| Crate | Used in | Why |
|---|---|---|
| `tokio` (`rt-multi-thread`, `sync`, `time`, `macros`) | kuro-track | async tracker calls; `JoinSet` for refresh; `tokio::time` for backoff; `watch` for login state |
| `serde`, `serde_json` | kuro-track | token JSON (`track_token_N` shape), tracker responses are in areas 28 to 33 |
| `rusqlite` (`bundled`) | kuro-db via TrackRepository | same SQLite version as the rest of the app |
| `thiserror` | kuro-track | `TrackError` enum |
| `anyhow` | GUI edges | only for converting to user toasts |
| `tracing` | kuro-track | replaces `logcat` (levels in section 9) |
| `futures` | kuro-track | `BoxFuture` in the `Tracker` trait object signature |
| `async-trait` | kuro-track | object-safe async methods on `dyn Tracker` (alternative: `BoxFuture` returns by hand; `async-trait` keeps the code short). Native `async fn` in traits is not object-safe, so `async-trait` is required for `Arc<dyn Tracker>`. |
| `keyring` (Secret Service backend) | kuro-platform (`SecretStore`) | libsecret/Secret Service on KDE/GNOME. Replaces Android app-private preferences for secrets. |
| `gio` (via `gtk4`) | GUI and kuro-platform | `gio::NetworkMonitor::default().is_network_available()` replaces `Context.isOnline()`; `gio::AppInfo::launch_default_for_uri` replaces `openInBrowser` |
| `url` | kuro-track | parse and build the OAuth authorise URL (replaces `android.net.Uri`) |
| `base64`, `sha2`, `rand` | kuro-track | PKCE S256 verifier and challenge (replaces `PkceUtil`) |
| `async-channel` | GUI | results from tokio to the GTK main loop |

Not used: `libadwaita` (not allowed), `once_cell` (use `std::sync::LazyLock`), `chrono` (use `time` or `std::time`).

---

## 3. Key types and traits

Types live in `kuro-track`. They mirror part 1 exactly. Options replace nullable Kotlin types and sentinels (`-1`, `0`) where the sentinel is only used for "absent".

### 3.1 Identity and kind

```rust
// kuro-track/src/model.rs

/// Tracker service id. Values are fixed and stored in sync_id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackerId(pub i64);

impl TrackerId {
    pub const MYANIMELIST: Self = Self(1);
    pub const ANILIST: Self = Self(2);
    pub const KITSU: Self = Self(3);
    pub const SHIKIMORI: Self = Self(4);
    pub const BANGUMI: Self = Self(5);
    pub const KOMGA: Self = Self(6);
    pub const MANGAUPDATES: Self = Self(7);
    pub const KAVITA: Self = Self(8);
    pub const SUWAYOMI: Self = Self(9);
    pub const SIMKL: Self = Self(101);
    pub const JELLYFIN: Self = Self(102);
}

/// Entry kind. Replaces the separate anime and manga code paths where the logic is identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind { Anime, Manga }

/// Progress unit. Anime: episode, manga: chapter.
pub trait Progress: Copy {
    fn value(self) -> f64;
}
```

### 3.2 Domain models

```rust
// kuro-track/src/model.rs

#[derive(Debug, Clone, PartialEq)]
pub struct AnimeTrack {
    /// None before the first insert (replaces id = -1).
    pub id: Option<i64>,
    pub anime_id: i64,
    pub tracker_id: TrackerId,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_episode_seen: f64,
    pub total_episodes: i64,        // 0 = unknown
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,            // epoch millis, 0 = unset
    pub finish_date: i64,           // epoch millis, 0 = unset
    pub private: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MangaTrack {
    pub id: Option<i64>,
    pub manga_id: i64,
    pub tracker_id: TrackerId,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_chapter_read: f64,
    pub total_chapters: i64,
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,
    pub finish_date: i64,
    pub private: bool,
}
```

Rules that must hold (from behaviour 4 and 6):
- `copy_personal_from` (domain form) copies `last_episode_seen`/`last_chapter_read`, `score`, `status`, `start_date`, `finish_date`, and `private` unconditionally. The legacy form has a `copy_remote_private` flag. Implement only the domain form, plus a `copy_remote_private: bool` parameter that defaults to `true` on the domain form, since trackers call the legacy form with `false` in some paths (`behaviour 4.2`). Implement both as:

```rust
pub fn copy_personal_from_anime(dst: &mut AnimeTrack, src: &AnimeTrack, copy_remote_private: bool) {
    dst.last_episode_seen = src.last_episode_seen;
    dst.score = src.score;
    dst.status = src.status;
    dst.start_date = src.start_date;
    dst.finish_date = src.finish_date;
    if copy_remote_private { dst.private = src.private; }
}
```

### 3.3 Search models

```rust
// kuro-track/src/model/search.rs

#[derive(Debug, Clone)]
pub struct AnimeTrackSearch {
    pub anime_id: i64,              // 0 until register
    pub tracker_id: TrackerId,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,              // required; no lateinit (defect D9)
    pub last_episode_seen: f64,
    pub total_episodes: i64,
    pub score: Option<f64>,         // None replaces the -1.0 sentinel
    pub status: i64,
    pub start_date_millis: i64,
    pub finish_date_millis: i64,
    pub private: bool,
    pub tracking_url: String,       // required
    pub authors: Vec<String>,
    pub artists: Vec<String>,
    pub cover_url: String,
    pub summary: String,
    pub publishing_status: String,
    pub publishing_type: String,
    pub start_date: String,         // display text
}

impl PartialEq for AnimeTrackSearch {
    /// Same rule as the source: anime_id, tracker_id, remote_id.
    fn eq(&self, o: &Self) -> bool {
        self.anime_id == o.anime_id && self.tracker_id == o.tracker_id && self.remote_id == o.remote_id
    }
}
impl Eq for AnimeTrackSearch {}
```

`MangaTrackSearch` is the same with `manga_id`, `last_chapter_read`, `total_chapters`.

Proposal: list keys use `(tracker_id, remote_id)` (not the hash), avoiding defect D10.

### 3.4 Tracker traits

```rust
// kuro-track/src/tracker.rs
use std::sync::Arc;
use futures::future::BoxFuture; // only where async-trait cannot be used
use crate::error::TrackError;
use crate::model::{AnimeTrack, MangaTrack, TrackerId};
use crate::model::search::{AnimeTrackSearch, MangaTrackSearch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreList(pub Vec<String>);

/// Every tracker. Replaces `Tracker` (Tracker.kt:10-53).
#[async_trait::async_trait]
pub trait Tracker: Send + Sync {
    fn id(&self) -> TrackerId;
    fn name(&self) -> &str;
    fn logo(&self) -> LogoRef;                 // replaces getLogo() drawable id
    fn logo_color(&self) -> u32;               // 0xAARRGGBB, replaces getLogoColor()
    fn supports_reading_dates(&self) -> bool { false }
    fn supports_private_tracking(&self) -> bool { false }
    fn completion_status(&self) -> i64;
    fn score_list(&self) -> &ScoreList;

    async fn login(&self, username: &str, password: &str) -> Result<(), TrackError>;
    /// Clears credentials and the auth-expired flag (BaseTracker.logout).
    fn logout(&self);

    /// Login state. Derived from stored credentials; see section 5.
    fn is_logged_in(&self) -> bool;
    fn login_state(&self) -> tokio::sync::watch::Receiver<bool>;

    fn save_credentials(&self, username: &str, password: &str);
    fn username(&self) -> String;
    fn password(&self) -> String;

    fn as_anime(&self) -> Option<&dyn AnimeTracker> { None }
    fn as_manga(&self) -> Option<&dyn MangaTracker> { None }
    fn as_deletable_anime(&self) -> Option<&dyn DeletableAnimeTracker> { None }
    fn as_deletable_manga(&self) -> Option<&dyn DeletableMangaTracker> { None }
    fn as_enhanced_anime(&self) -> Option<&dyn EnhancedAnimeTracker> { None }
    fn as_enhanced_manga(&self) -> Option<&dyn EnhancedMangaTracker> { None }
}

pub enum LogoRef { Embedded(&'static [u8]), IconName(&'static str) }

/// Replaces AnimeTracker (AnimeTracker.kt:24-123). Defaults mirror the Kotlin defaults.
#[async_trait::async_trait]
pub trait AnimeTracker: Tracker {
    fn status_list(&self) -> Vec<i64>;
    fn watching_status(&self) -> i64;
    fn rewatching_status(&self) -> i64;
    fn status_label(&self, status: i64) -> Option<&'static str>;   // message key, see section 3.7
    fn index_to_score(&self, index: usize) -> f64 { index as f64 }
    fn score_10(&self, t: &AnimeTrack) -> f64 { t.score }
    fn display_score(&self, t: &AnimeTrack) -> String;

    /// update(track, didWatchEpisode)
    async fn update(&self, t: &AnimeTrack, did_watch_episode: bool) -> Result<AnimeTrack, TrackError>;
    /// bind(track, hasSeenEpisodes)
    async fn bind(&self, t: &mut AnimeTrack, has_seen: bool) -> Result<(), TrackError>;
    async fn search(&self, query: &str) -> Result<Vec<AnimeTrackSearch>, TrackError>;
    async fn refresh(&self, t: &AnimeTrack) -> Result<AnimeTrack, TrackError>;
}

#[async_trait::async_trait]
pub trait MangaTracker: Tracker {
    fn status_list(&self) -> Vec<i64>;
    fn reading_status(&self) -> i64;
    fn rereading_status(&self) -> i64;
    fn status_label(&self, status: i64) -> Option<&'static str>;
    fn index_to_score(&self, index: usize) -> f64 { index as f64 }
    fn score_10(&self, t: &MangaTrack) -> f64 { t.score }
    fn display_score(&self, t: &MangaTrack) -> String;

    async fn update(&self, t: &MangaTrack, did_read_chapter: bool) -> Result<MangaTrack, TrackError>;
    async fn bind(&self, t: &mut MangaTrack, has_read: bool) -> Result<(), TrackError>;
    async fn search(&self, query: &str) -> Result<Vec<MangaTrackSearch>, TrackError>;
    async fn refresh(&self, t: &MangaTrack) -> Result<MangaTrack, TrackError>;
}

/// Replaces EnhancedAnimeTracker (EnhancedAnimeTracker.kt:12-41).
#[async_trait::async_trait]
pub trait EnhancedAnimeTracker: AnimeTracker {
    fn accepted_sources(&self) -> &[&'static str];               // fully qualified source ids
    fn accepts(&self, source_id: &str) -> bool { self.accepted_sources().contains(&source_id) }
    async fn match_entry(&self, title: &str, entry_id: i64) -> Result<Option<AnimeTrackSearch>, TrackError>;
    fn is_track_from(&self, t: &AnimeTrack, source_id: Option<&str>) -> bool;
    fn migrate_track(&self, t: &AnimeTrack, new_source_id: &str) -> Option<AnimeTrack>;
}

#[async_trait::async_trait]
pub trait EnhancedMangaTracker: MangaTracker {
    fn accepted_sources(&self) -> &[&'static str];
    fn accepts(&self, source_id: &str) -> bool { self.accepted_sources().contains(&source_id) }
    async fn match_entry(&self, title: &str, entry_id: i64) -> Result<Option<MangaTrackSearch>, TrackError>;
    fn is_track_from(&self, t: &MangaTrack, source_id: Option<&str>) -> bool;
    fn migrate_track(&self, t: &MangaTrack, new_source_id: &str) -> Option<MangaTrack>;
}

#[async_trait::async_trait]
pub trait DeletableAnimeTracker: AnimeTracker {
    async fn delete(&self, t: &AnimeTrack) -> Result<(), TrackError>;
}
#[async_trait::async_trait]
pub trait DeletableMangaTracker: MangaTracker {
    async fn delete(&self, t: &MangaTrack) -> Result<(), TrackError>;
}
```

Notes on the signatures:
- `match_entry` takes `(title, entry_id)` instead of `Anime`/`Manga` because `kuro-track` must not depend on the entry types of area 07. The caller passes the title and the entry id; the entry id is written to the result's `anime_id` by the caller (behaviour 6.2 step 2b).
- The `accepts(source_id)` check replaces `source::class.qualifiedName in getAcceptedSources()`. The source id is the Kurobayomi source identifier (area 17), a string. This is a deliberate change: Kotlin class names do not exist in Rust.
- `logout(&self)` is synchronous, like the Kotlin default, because it only writes preferences.

### 3.5 Remote setter rules

Free functions, not trait methods, so trackers cannot change the rules. They match behaviour section 3.

```rust
// kuro-track/src/remote.rs
use crate::model::{AnimeTrack, MangaTrack};

/// behaviour 3.2
pub fn set_anime_status(t: &mut AnimeTrack, tr: &dyn AnimeTracker, status: i64) {
    t.status = status;
    if status == tr.completion_status() && t.total_episodes != 0 {
        t.last_episode_seen = t.total_episodes as f64;
    }
}

/// behaviour 3.3. `episode` is truncated to i32 as in the source (toInt).
pub fn set_last_episode_seen(t: &mut AnimeTrack, tr: &dyn AnimeTracker, episode: i32, now_millis: i64) {
    if t.last_episode_seen == 0.0 && t.last_episode_seen < episode as f64 && t.status != tr.rewatching_status() {
        t.status = tr.watching_status();
    }
    t.last_episode_seen = episode as f64;
    if t.total_episodes != 0 && (t.last_episode_seen as i64) == t.total_episodes {
        t.status = tr.completion_status();
        t.finish_date = now_millis;
    }
}

/// behaviour 3.4. A missing string yields -1.0, as in the source.
pub fn set_score(t: &mut AnimeTrack, tr: &dyn AnimeTracker, score_text: &str) {
    let idx = tr.score_list().0.iter().position(|s| s == score_text);
    t.score = match idx { Some(i) => tr.index_to_score(i), None => -1.0 };
}
```

`now_millis` is passed in so tests can fix the clock. The source calls `System.currentTimeMillis()` directly.

The manga versions are the same with `last_chapter_read`, `total_chapters`, `reading_status`, `rereading_status`, `start_date` and `finish_date` names.

Proposal on `set_score`: the `-1.0` return keeps the source behaviour. Section 3.8 of the behaviour doc lists the defect. Kurobayomi should instead return `Err(TrackError::UnknownScore)` and not write. Tests in `p1-edge-cases-and-tests.md`.

### 3.6 Error type

```rust
// kuro-track/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    #[error("{0}")]
    Message(String),                    // tracker message, shown verbatim like the source
    #[error("not logged in")]
    NotLoggedIn,
    #[error("unknown score '{0}'")]
    UnknownScore(String),
    #[error("network unavailable")]
    Offline,
    #[error("unauthorised")]
    Unauthorised,
    #[error(transparent)]
    Db(#[from] kuro_db::DbError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("cancelled")]
    Cancelled,
}
```

`Message` carries the text the tracker returns (the source shows `e.message`, behaviour 3.6). `Offline` is produced by the connectivity check, not by reqwest.

### 3.7 Message keys

Messages are not strings in Rust. `kuro-track` returns keys, and the GUI resolves them with the i18n crate (area 51). Keys are the MR/AYMR names in section 18 of the behaviour doc, for example `MsgKey::TrackError { tracker: String, detail: String }` mapped to `"%1$s error: %2$s"`.

```rust
pub enum MsgKey {
    TrackerError { tracker: String, detail: String },     // MR.strings.track_error
    LoginSuccess,                                          // MR.strings.login_success
    LogoutSuccess,                                         // MR.strings.logout_success
    TrackersUpdatedAnime(i64),                             // AYMR.strings.trackers_updated_summary_anime
    TrackersUpdatedManga(i64),                             // AYMR.strings.trackers_updated_summary_manga
    ConfirmUpdateAnime(i64),                               // AYMR.strings.confirm_tracker_update_anime
    ConfirmUpdateManga(i64),                               // MR.strings.confirm_tracker_update
    NoResultsFound,
    UnknownError,
    NoMatchFound,                                          // MR.strings.error_no_match
    Raw(String),                                           // exception text (register/updateRemote failure)
}
```

### 3.8 Bind and progress functions

```rust
// kuro-track/src/bind.rs
pub struct BindInput<'a> {
    pub entry_id: i64,
    pub tracker: &'a dyn AnimeTracker,
    pub item: AnimeTrack,            // from the search result, id None
}

/// Dependencies of the anime bind flow. Repositories are traits so tests use kuro-testkit fakes.
pub struct AnimeDeps<'a> {
    pub tracks: &'a dyn AnimeTrackRepo,
    pub history: &'a dyn AnimeHistoryRepo,
    pub episodes: &'a dyn AnimeEpisodeRepo,
    pub progress: &'a dyn EpisodeProgressSync,   // SyncEpisodeProgressWithTrack, defined below
}

/// Mirror of AddAnimeTracks.bind (behaviour 6.1). Runs as a detached tokio task
/// (no abort on drop), matching withNonCancellableContext.
pub async fn bind_anime(deps: &AnimeDeps<'_>, input: BindInput<'_>) -> Result<AnimeTrack, TrackError>;

/// Mirror of AddAnimeTracks.bindEnhancedTrackers (behaviour 6.2). Returns per-tracker
/// failures; the source only logs them (WARN), so the caller does too.
pub async fn bind_enhanced_anime(deps: &AnimeDeps<'_>, entry_id: i64, entry_title: &str,
                                 source_id: &str, trackers: &[Arc<dyn AnimeTracker>])
    -> Vec<(TrackerId, TrackError)>;

/// Mirror of SyncEpisodeProgressWithTrack.await (behaviour 8.1).
#[async_trait::async_trait]
pub trait EpisodeProgressSync: Send + Sync {
    async fn sync(&self, entry_id: i64, remote: &AnimeTrack, tracker: &dyn AnimeTracker);
}
```

Repository traits (defined in `kuro-track`, implemented in area 05/06): `AnimeTrackRepo`, `MangaTrackRepo`, `AnimeEpisodeRepo`, `MangaChapterRepo`, `AnimeHistoryRepo`, `MangaHistoryRepo`.

```rust
#[async_trait::async_trait]
pub trait AnimeTrackRepo: Send + Sync {
    async fn get_by_id(&self, track_id: i64) -> Result<Option<AnimeTrack>, TrackError>;   // WHERE _id = ?
    async fn list_for_entry(&self, anime_id: i64) -> Result<Vec<AnimeTrack>, TrackError>;
    async fn list_all(&self) -> Result<Vec<AnimeTrack>, TrackError>;
    /// Upsert by (anime_id, tracker_id). Keeps _id (proposal, see behaviour defect D1).
    async fn upsert(&self, t: &AnimeTrack) -> Result<i64, TrackError>;
    async fn upsert_all(&self, ts: &[AnimeTrack]) -> Result<(), TrackError>;
    async fn delete(&self, anime_id: i64, tracker_id: TrackerId) -> Result<(), TrackError>;
    fn watch_for_entry(&self, anime_id: i64) -> tokio::sync::watch::Receiver<Vec<AnimeTrack>>;
}
```

`upsert` returns the id so the caller keeps a stable `Option<i64>` (decision: the Kotlin code did not do this, which caused D1).

### 3.9 Progress functions

```rust
// kuro-track/src/progress.rs

pub enum TrackOutcome { Updated, Queued, Skipped, Failed(TrackError) }

/// Mirror of TrackEpisode.await (behaviour 9.1). Proposal: queue on any failure
/// (the manga rule), and check connectivity first so offline does not spend a request.
pub async fn track_episode(ctx: &TrackCtx, entry_id: i64, episode: f64, queue_on_failure: bool)
    -> Vec<(TrackerId, TrackOutcome)>;

pub async fn track_chapter(ctx: &TrackCtx, entry_id: i64, chapter: f64, queue_on_failure: bool)
    -> Vec<(TrackerId, TrackOutcome)>;

pub struct TrackCtx {
    pub trackers: Arc<TrackerManager>,
    pub tracks: Arc<dyn AnimeTrackRepo>,       // or the Manga variant, chosen by Kind
    pub queue: Arc<RetryQueue>,
    pub scheduler: Arc<RetryScheduler>,
    pub net: Arc<dyn Connectivity>,
    pub prefs: Arc<TrackPrefs>,
}
```

`queue_on_failure` is `true` for both kinds in the Proposal. The source differs: anime queues only when offline (behaviour 9.1), manga queues on any failure (behaviour 9.2).

### 3.10 Registry

```rust
// kuro-track/src/manager.rs
pub struct TrackerManager {
    trackers: Vec<Arc<dyn Tracker>>,       // registry order (TrackerManager.kt:39-42)
}

impl TrackerManager {
    pub fn new(http: reqwest::Client, secrets: Arc<dyn SecretStore>, prefs: Arc<TrackPrefs>) -> Self;
    pub fn all(&self) -> &[Arc<dyn Tracker>];
    pub fn get(&self, id: TrackerId) -> Option<Arc<dyn Tracker>>;     // None for unknown
    pub fn get_all(&self, ids: &HashSet<TrackerId>) -> Vec<Arc<dyn Tracker>>;
    pub fn logged_in(&self) -> Vec<Arc<dyn Tracker>>;                 // registry order
    /// Emits the logged-in list whenever any tracker's login state changes.
    pub fn logged_in_stream(&self) -> tokio::sync::watch::Receiver<Vec<TrackerId>>;
}
```

`logged_in_stream` replaces `loggedInTrackersFlow()` (`TrackerManager.kt:46-50`). A `watch` channel emits the latest value, which is the same as Kotlin's `combine` with `distinctUntilChanged`, except the first value is emitted only after the watcher task starts (fine for a GTK page).

### 3.11 Preferences

```rust
// kuro-track/src/prefs.rs
pub struct TrackPrefs {
    pub auto_update_track: kuro_prefs::Pref<bool>,               // pref_auto_update_manga_sync_key, true
    pub track_on_adding_to_library: kuro_prefs::Pref<bool>,      // track_on_adding_to_library, true
    pub show_next_episode_airing_time: kuro_prefs::Pref<bool>,   // show_next_episode_airing_time, true
    pub auto_update_on_mark_read: kuro_prefs::Pref<AutoTrackState>, // pref_auto_update_manga_on_mark_read, ALWAYS
    pub anilist_score_type: kuro_prefs::Pref<String>,            // anilist_score_type, "POINT_10"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum AutoTrackState { #[default] Always, Ask, Never }
```

Stored names: `AutoTrackState` is serialised as `"ALWAYS"`, `"ASK"`, `"NEVER"` for compatibility with exported Aniyomi preferences (area 21). Unknown values fall back to `Always`, as `getEnum` does (`PreferenceStore.kt:37-42`). Credential-related keys are not in `TrackPrefs`; they are in `SecretStore` (section 5).

### 3.12 Retry queue

```rust
// kuro-track/src/queue/mod.rs
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QueueItem { pub kind: Kind, pub entry_id: i64, pub tracker_id: TrackerId, pub progress: f64, pub queued_at_ms: i64, pub attempts: u32 }

pub struct RetryQueue { /* backed by TrackRetryRepo (SQL table in section 7.2 of data doc) */ }

impl RetryQueue {
    /// Keeps the highest progress per (kind, entry_id, tracker_id). A lower value never replaces a higher one.
    pub async fn add(&self, kind: Kind, entry_id: i64, tracker_id: TrackerId, progress: f64, now_ms: i64) -> Result<(), TrackError>;
    pub async fn remove(&self, kind: Kind, entry_id: i64, tracker_id: TrackerId) -> Result<(), TrackError>;
    pub async fn list(&self, kind: Kind) -> Result<Vec<QueueItem>, TrackError>;
    pub async fn is_empty(&self) -> Result<bool, TrackError>;
}
```

`add` is one SQL statement, which removes the read-modify-write race (defect D6):
`INSERT INTO track_retry_queue(kind, entry_id, tracker_id, progress, queued_at, attempts) VALUES (?1, ?2, ?3, ?4, ?5, 0) ON CONFLICT(kind, entry_id, tracker_id) DO UPDATE SET progress = MAX(progress, excluded.progress);`

### 3.13 Connectivity

```rust
// kuro-platform/src/lib.rs (trait, implemented in linux/)
pub trait Connectivity: Send + Sync {
    /// true when the OS reports a usable network (replaces Context.isOnline()).
    fn is_online(&self) -> bool;
    /// Resolves when the network becomes available; used by the retry scheduler.
    fn wait_online(&self) -> futures::future::BoxFuture<'static, ()>;
}
```

Linux implementation: `gio::NetworkMonitor::default()`; `is_online` = `is_network_available()`; `wait_online` subscribes to `network-changed` and resolves when `is_network_available()` is true. Caveat: GIO reports link availability, not internet reachability. A captive portal passes the check and the request then fails, which is handled by the retry path.

---

## 4. Async model

Threads:
- GTK main loop: all widgets, all `watch` receivers that update widgets (polled through `glib::spawn_future_local` consuming an `async_channel`).
- Tokio multi-thread runtime owned by `kuro-core` (area 01): tracker HTTP, DB writes via the repo (area 05 provides a blocking pool or async wrapper), retry scheduler.

Rules:
1. UI code calls a `TrackAction` enum method, which sends the action to a `tokio` task and returns immediately. The result comes back on an `async_channel` and is applied on the GTK thread.
2. Writes that Kotlin marks `withNonCancellableContext` run as spawned tasks whose `JoinHandle` is detached (the task keeps running when the page closes). Example: `tokio::spawn(async move { track_chapter(...).await })` with no `abort()`.
3. Refresh (`RefreshAnimeTracks`) is cancellable by page close in Kotlin. Proposal: spawn it and let it finish, so the local row is never half-written (behaviour 19).
4. Shared state across tasks: `Arc<TrackerManager>`, `Arc<RetryQueue>`. No `Mutex` around a tracker: trackers hold their own `reqwest::Client` (cheap clone) and credentials behind `RwLock`.
5. Queue writes are single SQL statements (section 3.12). No in-memory cache, so there is no lock to hold across `await`.

Per-tracker concurrency: `Semaphore` of 4 permits per tracker id for network calls in refresh. Rationale: Kotlin uses unbounded `async` per binding (at most 11 trackers, so the bound is practical rather than a change). Limit comes from area 12's rate limiter if it exists.

---

## 5. Secrets and login state

### 5.1 `SecretStore` (kuro-platform)

```rust
pub trait SecretStore: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError>;
}
```

Linux implementation: `keyring` with the Secret Service backend (`keyring::Entry::new(service, account)`). `service` = `"kurobayomi.track"`, `account` = `"<tracker_id>.password"` and `"<tracker_id>.username"` and `"<tracker_id>.token"`.

Fallback when no Secret Service is running (for example a minimal window manager): store in `$XDG_DATA_HOME/kurobayomi/secrets.json` with mode `0600`, written atomically. Log a warning once. Never store secrets in the preference file.

### 5.2 `BaseTracker` in Rust

```rust
pub struct BaseTracker {
    pub id: TrackerId,
    pub name: &'static str,
    secrets: Arc<dyn SecretStore>,
    prefs: Arc<TrackPrefs>,
    login_tx: tokio::sync::watch::Sender<bool>,
    auth_expired: std::sync::atomic::AtomicBool,
}

impl BaseTracker {
    pub fn save_credentials(&self, user: &str, pass: &str) -> Result<(), TrackError> {
        // write username, password, clear auth_expired, then publish login state.
        // Order matches TrackPreferences.setCredentials (TrackPreferences.kt:29-33).
    }
    pub fn logout(&self) { /* save "", "" and clear auth_expired; publish false */ }
    pub fn is_logged_in(&self) -> bool { /* both non-empty */ }
}
```

Login state rule: `is_logged_in` is `username != "" && password != ""`, exactly as `BaseTracker.kt:32-34`. The `watch` channel is updated in `save_credentials` and `logout` only, so no polling is needed.

Proposal fix: Kotlin wrote username and password with two separate preference writes, so a crash between them leaves a half-login. The Rust version writes both secrets in one `set` sequence and publishes the login state after both succeed. A partial write is detected at startup as "logged out" (username or password missing).

### 5.3 Token JSON

OAuth trackers store their OAuth object as JSON under `track_token_<id>` (data doc section 3). Rust: `token: Option<OAuthToken>` in each tracker, with `serde` to and from the secret store under account `<id>.token`. The password slot holds the access token (for compatibility with `is_logged_in`), as in the source.

---

## 6. Retry scheduler

Replaces `DelayedAnimeTrackingUpdateJob` and `DelayedMangaTrackingUpdateJob` (WorkManager). Behaviour section 10.

```rust
// kuro-track/src/queue/retry.rs
pub struct RetryScheduler {
    queue: Arc<RetryQueue>,
    net: Arc<dyn Connectivity>,
    wake: tokio::sync::Notify,                 // enqueue wakes the loop
    run: Arc<dyn RetryRunner>,                 // calls track_episode / track_chapter for an item
}

#[async_trait::async_trait]
pub trait RetryRunner: Send + Sync {
    /// Returns true when the item was resolved (updated, or skipped because remote is already at or past it).
    async fn run_item(&self, item: &QueueItem) -> Result<bool, TrackError>;
}

pub const MAX_ATTEMPTS: u32 = 4;               // body runs for attempts 0..=3, then give up
pub const INITIAL_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5 * 60);
pub const MAX_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5 * 60 * 60); // WorkManager cap

impl RetryScheduler {
    /// Replaces setupTask: resets attempt counters (ExistingWorkPolicy.REPLACE) and wakes the loop.
    pub fn kick(&self);
    pub fn spawn(self: Arc<Self>) -> tokio::task::JoinHandle<()>;
}
```

Loop (per kind, anime and manga run in one loop over both):
1. Wait for `kick` or the startup signal.
2. Wait until `net.is_online()`, using `net.wait_online()` (replaces `NetworkType.CONNECTED`).
3. For each queued item: if no binding exists for `(entry_id, tracker_id)`, remove the item and continue (behaviour 10.2). Else call `run_item`. Successful items are removed. Items that return `false` (skipped, see defect D5) are also removed by the Proposal.
4. If any item remains and `attempts < MAX_ATTEMPTS`, increment attempts, sleep `min(INITIAL_BACKOFF * 2^(attempt-1), MAX_BACKOFF)`, go to step 2.
5. If attempts reached `MAX_ATTEMPTS`, log at WARN and keep the items queued (no silent drop; the next `kick` restarts the count). Proposal: surface a single toast "N tracking updates are pending" the next time the entry page opens.

Constants and their source: `MAX_ATTEMPTS` mirrors `runAttemptCount > 3` (`DelayedAnimeTrackingUpdateJob.kt:27`); `INITIAL_BACKOFF` mirrors `5.minutes` (`:72`); `MAX_BACKOFF` mirrors WorkManager's documented 5-hour cap.

Exponential formula: `delay = INITIAL_BACKOFF * 2^(attempt - 1)`. Verify against WorkManager's implementation when the Android behaviour is re-checked; the Rust constant is a Proposal.

Persistence: the queue is in SQLite, so it survives restarts. On app start, `kick` runs once if the queue is non-empty.

---

## 7. UI integration with the mockup

Current mockup (`src/entry.rs`):
- `tracking()` (lines 496-537) renders a fixed list of three tracker cards with hard-coded text.
- `tracker_search()` (lines 539-568) renders four static results.
- `dialog_of(id, name)` opens dialogs defined in `spec/entries.json`.
- `ui::open(id)` navigates by screen id; `ui::show_dialog(&Dialog)` shows a dialog from the spec.

Plan:
1. Keep `open()` and `dialog_of()` for the navigation shell, but feed the content from `track::ui`.
2. `track::ui::info::build(entry_id, kind, cx) -> GBox` replaces `tracking()`. It reads `TrackRepo::list_for_entry` and `TrackerManager::logged_in`.
3. `track::ui::search::build(entry_id, kind, tracker_id, initial_query, current_url) -> GBox` replaces `tracker_search()`. Results come from `search()`.
4. Dialogs (`status`, `chapters`, `score`, `start_date`, `finish_date`, `remove_date`, `remove_tracking`) are built in `track::ui::selectors` and `track::ui::remove`, not from `spec/entries.json` text. Their titles and option labels come from the `MsgKey` table (section 3.7) and the tracker's `status_list`. The spec file keeps layout notes only.
5. `spec/settings.json` `settings_tracking` rows are replaced by `track::settings::build(cx)`. Keys and defaults come from section 2 of the data doc through `TrackPrefs`.
6. `src/settings.rs` should call `track::settings::build` for `settings_tracking`.

Widget mapping (GTK, no libadwaita):
- Track info card: `gtk::Box` (vertical) with a `gtk::Frame` or `gtk::ListBox` row; logo as a 48 px `gtk::Frame` with `set_size_request(48, 48)`, filled with the logo texture on the logo color via CSS.
- Dialogs: `gtk::Window` with `set_modal(true)` and `set_transient_for(main)`, or `gtk::AlertDialog` (GTK 4.10+) for the simple confirms (logout, remove). Use `gtk::Window` for the search and date pickers.
- Number wheel: `gtk::SpinButton` over an `adjustment` with range `0..=max` (replaces `WheelNumberPicker`). Keyboard arrows and scroll work natively.
- Score wheel: `gtk::DropDown` over a `gtk::StringList` (replaces `WheelTextPicker`).
- Status list: `gtk::ListBox` with `gtk::CheckButton` group (radio) per row.
- Date picker: `gtk::Calendar` in a popover, plus the "Remove" button. `gtk::Calendar` supports min and max day via the "day-selected" handler; reject out-of-range days by resetting (GTK Calendar has no selectable-dates predicate).
- Search: `gtk::SearchEntry` (search key, clear button built in) with `activate` as the dispatch signal.
- Long-press: `gtk::GestureLongPress` (replaces `combinedClickable`).

---

## 8. Android to Linux mapping (for the symbols in this scope)

| Android / Kotlin symbol | Used in | Linux / Rust replacement | Notes |
|---|---|---|---|
| `Context.isOnline()` (`util/system/NetworkExtensions.kt:16`) | `TrackEpisode`, external player | `Connectivity::is_online` via `gio::NetworkMonitor` | GIO reports link availability; see 3.13 |
| `androidx.work.CoroutineWorker`, `OneTimeWorkRequestBuilder`, `enqueueUniqueWork(REPLACE)`, `BackoffPolicy.EXPONENTIAL`, `NetworkType.CONNECTED` | delayed update jobs | `RetryScheduler` on tokio | section 6 |
| `context.getSharedPreferences("tracking_queue")` | delayed stores | `track_retry_queue` table | section 7.2 of data doc |
| `Preference` / `PreferenceStore` (`core/common/preference`) | `TrackPreferences` | `kuro-prefs` `Pref<T>` | area 02 |
| Private prefs `__PRIVATE_` | credentials, tokens | `SecretStore` (Secret Service) | section 5 |
| `android.net.Uri` | OAuth URL building and parsing | `url::Url` | `query_pairs`, `fragment` |
| `PkceUtil` | MyAnimeList | `sha2` + `base64` (`URL_SAFE_NO_PAD`) | area 28 |
| `openInBrowser(url, forceDefaultBrowser = true)` | settings OAuth rows, track search "Open in browser" | `gio::AppInfo::launch_default_for_uri` (`xdg-open`) | `forceDefaultBrowser` has no Linux equivalent; the default handler is always used |
| `copyToClipboard(text, label)` | title and link copy | `gtk::gdk::Display::default().clipboard().set_text(text)` | |
| `Toast` (`toast(String)`) | errors and confirmations | in-app overlay (`gtk::Overlay` + `gtk::Revealer`, 3 s auto-hide) | area 36 owns the overlay; this module emits `MsgKey` |
| `Snackbar` with action (`showSnackbar`) | ASK mark-read prompt | `gtk::Revealer` banner with an OK button, 4000 ms | section 11 of behaviour |
| `DeepLink` `aniyomi://…-auth` (manifest) | OAuth redirect | custom scheme `kurobayomi://` via `.desktop` `MimeType=x-scheme-handler/kurobayomi` (area 53), or loopback `127.0.0.1` listener | `p1-topic-tracker-oauth.md` |
| `logcat(priority, e)` | all logging | `tracing::{warn!, error!, info!}` | section 9 |
| `Injekt.get` / `injectLazy` | everywhere | constructor injection through `AppContext` (area 01) | |
| `withIOContext` / `withNonCancellableContext` / `withUIContext` | interactors | `tokio::spawn` (detached) / `glib::spawn_future_local` | section 4 |
| `DatePicker` with `SelectableDates` | date selector | `gtk::Calendar` + day validation | section 7 |
| `WheelNumberPicker`, `WheelTextPicker` | selectors | `gtk::SpinButton`, `gtk::DropDown` | section 7 |
| `ItemCover.Book` | search result cover | area 34 image pipeline, 2:3 frame | |
| `Compose preview annotations` | preview providers | dropped; fixtures in `kuro-testkit` | |

---

## 9. Error handling policy

- Library errors: `TrackError` (`thiserror`). Public functions return `Result<_, TrackError>`.
- Per-tracker errors in batch operations (refresh, bind enhanced, track episode) are collected into `Vec<(TrackerId, TrackError)>` and returned. Nothing panics on a tracker error.
- Failure display: `MsgKey::TrackerError` for refresh (behaviour 13.6), `MsgKey::Raw(e.to_string())` for register and remote-update failures (the source shows raw text).
- Logging levels (replacing `logcat`):
  - `error!` for remote-update failure in `update_remote` (`logcat(ERROR, "Failed to update remote track data id=…")`).
  - `warn!` for sync failures in `SyncEpisodeProgressWithTrack` and for enhanced-match failures.
  - `info!` for `TrackEpisode`/`TrackChapter` failures (matches `logcat(INFO)`).
  - `debug!` for queue add and retry items.
- No `unwrap` on tracker data. `lateinit` cases in the source (`title`, `tracking_url`) are `Option` or required constructor fields in Rust.

---

## 10. Pitfalls

1. Id churn (defect D1). The source keys the queue by the track `_id`, which changes on every write. **Decision:** the queue is keyed by `(kind, entry_id, tracker_id)`, which is stable. This also removes the anime-id confusion of defect D2, because the typed `entry_id` and `TrackerId` cannot be swapped silently. It is a deliberate change from the source; document it in the release notes.
2. Float precision (defect D7): store progress as `f64` throughout. Never convert from `f32`.
3. Mutable copies (defect D19): do not mutate a DB-facing copy and then continue with a second copy. Use one `AnimeTrack` value and pass `&mut` through the setter chain.
4. Trackers that are manga-only: `as_anime()` returns `None`. Do not use `unwrap` on it; the UI never lists them in the anime sheet (behaviour 1.4).
5. Clock: `set_last_episode_seen` needs `now_millis` injected. Tests fix it.
6. GIO connectivity is not internet reachability. A request can still fail after `is_online() == true`; the queue handles it.
7. Secret Service may be missing. The fallback file must not be world-readable. Check the mode on creation (`0600`) and warn when falling back.
8. The `watch` receiver for login state must be cloned per page, not shared across pages (cloning is cheap).
9. GTK widgets must only be touched on the main thread. Results from tokio cross via `async_channel`.
10. `tracker_id` is `i64` in the DB; the Rust newtype `TrackerId` must not be created from a raw `u32`.
