# area:27-tracking-core part 2 — Rust design (track dialogs, track items, tracker login callback)

Targets (from `plans/addenda/requirements-cross-platform.md` and `plans/01-foundation-app-bootstrap/p1-rust-design.md`): stable Rust 2024, gtk4-rs 0.11 (`gtk4` crate, feature `v4_16`, no libadwaita), tokio, rusqlite, reqwest, serde, `kuro-platform` for every OS touch. Nothing in this design uses `#[cfg(target_os)]`, `xdg-open`, libsecret, or `XDG_*` directly (requirement 1 of the addendum).

Part 1 (`p1-rust-design.md` in this folder) owns the `TrackerService` trait, the tracker implementations, `TrackStore` and the refresh/add interactors. This part defines the consumer side and the login callback. Where the names below differ from part 1's final names, part 1 wins and this file is updated in the same change.

Behavior references: `p2-behavior.md` section numbers are cited as "B§n". Data references: `p2-data-and-config.md` as "D§n".

---

## 1. Module layout

```
crates/
  kuro-track/                        (created by part 1; part 2 adds these modules)
    src/lib.rs                        re-exports
    src/entry_kind.rs                 EntryKind                         NEW (p2)
    src/record.rs                     TrackRecord, TrackItem            NEW (p2)
    src/item.rs                       assemble_items() (B§3.4)          NEW (p2)
    src/progress.rs                   progress_range(), truncate rules  NEW (p2)
    src/score.rs                      score_index_lookup() (B§7)        NEW (p2)
    src/dates.rs                      picker <-> stored conversion, bounds (B§8)  NEW (p2)
    src/messages.rs                   MR keys -> English text, track_error format  NEW (p2)
    src/oauth/mod.rs                  OAuth module root                 NEW (p2)
    src/oauth/callback.rs             parse_callback(Uri) -> CallbackOutcome (B§12)  NEW (p2)
    src/oauth/pkce.rs                 PkceSession (verifier, challenge) (D§5.1)  NEW (p2)
    src/oauth/pending.rs              PendingLogin store: part 1 type, p2 adds the CSRF guard use  (p1 owns the type)
    src/error.rs                      TrackError                        NEW (p2, shared with p1)
  kuro-platform/                      (addendum rule 1: every OS touch lives here)
    src/lib.rs                        traits Opener, SecretStore, UriSchemeHandler  (p1 creates; p2 uses)
    src/linux/                        Linux implementations
src/                                  GUI crate (mockup, existing)
  track/                              NEW directory (moves tracking code out of entry.rs)
    mod.rs                            TrackNav (modal stack), entry points
    sheet.rs                          tracking sheet (rows)     replaces entry.rs::tracking()
    search.rs                         remote search page        replaces entry.rs::tracker_search()
    home_model.rs                     HomeModel, loads TrackItem list, refresh on open
    status.rs                         StatusDialog
    progress.rs                       ProgressDialog (episode / chapter)
    score.rs                          ScoreDialog
    dates.rs                          DateDialog, DateRemoveDialog
    remove.rs                         RemoveTrackingDialog
    login.rs                          LoginReturn view (feedback) and manual paste fallback
    toast.rs                          in-window toast overlay (no libadwaita)
```

Rule from the existing mockup: `src/entry.rs` keeps the entry details page. Its `tracking()` and `tracker_search()` functions (`src/entry.rs:496-566`) become thin calls into `src/track/`. The `dialog_of(id, dialog)` helper (`src/entry.rs:25-29`) is replaced by `TrackNav::push`.

---

## 2. Dependencies added by this area

| Crate | Version (min) | Used in | Why |
|---|---|---|---|
| `async-trait` | 0.1 | `kuro-track` | `TrackerService` must be object safe (`Arc<dyn TrackerService>` in `TrackItem`). Native `async fn` in traits is not object safe. |
| `thiserror` | 2 | `kuro-track` | `TrackError` |
| `base64` | 0.22 | `kuro-track::oauth::pkce` | `URL_SAFE_NO_PAD` for the PKCE verifier (D§5.1) |
| `getrandom` | 0.2 | `kuro-track::oauth::pkce` | 50 random bytes (matches `SecureRandom` in `PkceUtil.kt:9`) |
| `url` | 2 | `kuro-track::oauth::callback` | parsing `aniyomi://host?query#fragment`. `gio::Uri` works too; `url` keeps the parser testable without GTK. |
| `chrono` | 0.4 | `kuro-track::dates` | `NaiveDate`, `Local`, `Utc` for the date rules (B§8) |
| `serde_json` | 1 (already present) | `kuro-track::oauth` | token JSON in `SecretStore` |
| `keyring` | 3 | `kuro-platform::linux` only | Secret Service (addendum table: `SecretStore`) |
| `async-channel` | 2 | `src/track` | GTK-thread delivery of results |

Not added: `libadwaita`, a toast crate, a dialog crate (GTK 4.16 has `gtk::AlertDialog`, `gtk::Window`, `gtk::Calendar`, `gtk::DropDown`, `gtk::SpinButton`, `gtk::UriLauncher`).

---

## 3. Core types (`kuro-track`)

```rust
// src/entry_kind.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    Anime,
    Manga,
}

impl EntryKind {
    /// Progress unit label key (B§6): MR chapters for Manga, AYMR episodes for Anime.
    pub fn progress_label_key(self) -> MsgKey {
        match self {
            EntryKind::Anime => MsgKey::Episodes,
            EntryKind::Manga => MsgKey::Chapters,
        }
    }
}
```

```rust
// src/record.rs
/// One row of the anime_sync / manga_sync table, as the dialogs see it (domain model, D§1).
#[derive(Clone, Debug, PartialEq)]
pub struct TrackRecord {
    /// Local `_id`. `-1` means "not persisted yet" (D§1.3 `toDomainTrack(idRequired=false)`).
    pub id: i64,
    pub entry_id: i64,           // animeId / mangaId
    pub tracker_id: i64,         // sync_id
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub progress: f64,           // last_episode_seen / last_chapter_read (REAL)
    pub total: i64,              // total_episodes / total_chapters (0 = unknown)
    pub status: i64,
    pub score: f64,              // stored score value, see TrackerService::index_to_score
    pub remote_url: String,      // "" when unknown
    pub start_date_ms: i64,      // 0 = unset; local midnight of the chosen day
    pub finish_date_ms: i64,     // 0 = unset
    pub private: bool,
}

/// Kotlin AnimeTrackItem / MangaTrackItem (B§3.4). One per logged-in tracker of the kind.
#[derive(Clone)]
pub struct TrackItem {
    pub tracker: Arc<dyn TrackerService>,
    pub record: Option<TrackRecord>,
}
```

`TrackItem` has no `PartialEq` because `Arc<dyn TrackerService>` has none. Equality for the `distinctUntilChanged` step is done on `(tracker.id(), record)` pairs (B§3.2).

```rust
// src/item.rs
/// B§3.4 mapToTrackItem, as a pure function so it can be unit-tested.
pub fn assemble_items(
    kind: EntryKind,
    logged_in: &[Arc<dyn TrackerService>],   // already filtered by is_logged_in, in TrackerManager order
    rows: &[TrackRecord],                     // rows for this entry
    source: &SourceInfo,                      // qualified name or "stub" marker (B§3.4 step 2)
) -> Vec<TrackItem> {
    logged_in
        .iter()
        .filter(|t| t.kind_supported(kind))   // the `is AnimeTracker` / `is MangaTracker` filter
        .map(|t| TrackItem {
            tracker: t.clone(),
            record: rows.iter().find(|r| r.tracker_id == t.id()).cloned(),
        })
        .filter(|item| match item.tracker.as_enhanced(kind) {
            Some(e) => e.accepts(source),     // B§3.4 step 4
            None => true,
        })
        .collect()
}
```

`SourceInfo` holds the source's qualified name (or `None` for a stub). The `accepts` rule is `source.qualified_name in accepted_sources` (B§3.4 step 4).

---

## 4. Tracker-side trait used by this area (subset of part 1)

```rust
// Part 1 owns the full trait. These are the methods the dialogs call.
#[async_trait::async_trait]
pub trait TrackerService: Send + Sync {
    fn id(&self) -> i64;
    fn name(&self) -> &str;
    fn is_logged_in(&self) -> bool;
    fn kind_supported(&self, kind: EntryKind) -> bool;
    fn supports_reading_dates(&self) -> bool { false }     // D§ B§11, BaseTracker.kt:76
    fn supports_private(&self) -> bool { false }           // BaseTracker.kt:78

    /// Ordered (key, label) pairs, same order as getStatusListAnime()/getStatusListManga().
    fn status_options(&self, kind: EntryKind) -> Vec<StatusOption>;
    fn completion_status(&self, kind: EntryKind) -> i64;   // getCompletionStatus()
    fn score_options(&self) -> Vec<String>;                // getScoreList()
    fn display_score(&self, r: &TrackRecord) -> String;    // displayScore()
    fn index_to_score(&self, index: usize) -> f64 { index as f64 }

    fn as_deletable(&self) -> Option<&dyn Deletable> { None }
    fn as_enhanced(&self, kind: EntryKind) -> Option<&dyn Enhanced> { None }

    async fn search(&self, kind: EntryKind, query: &str) -> Result<Vec<SearchHit>, TrackError>;
    async fn register(&self, kind: EntryKind, hit: SearchHit, entry_id: i64, private: bool)
        -> Result<TrackRecord, TrackError>;   // AnimeTracker.register + AddAnimeTracks.bind
    async fn refresh(&self, kind: EntryKind, r: &TrackRecord) -> Result<TrackRecord, TrackError>;

    // Remote setters. Each returns the record as stored after the remote call succeeded.
    async fn set_status(&self, kind: EntryKind, r: TrackRecord, status: i64) -> Result<TrackRecord, TrackError>;
    async fn set_progress(&self, kind: EntryKind, r: TrackRecord, progress: i64) -> Result<TrackRecord, TrackError>;
    async fn set_score_index(&self, kind: EntryKind, r: TrackRecord, score_label: &str) -> Result<TrackRecord, TrackError>;
    async fn set_start_date(&self, kind: EntryKind, r: TrackRecord, ms: i64) -> Result<TrackRecord, TrackError>;
    async fn set_finish_date(&self, kind: EntryKind, r: TrackRecord, ms: i64) -> Result<TrackRecord, TrackError>;
    async fn set_private(&self, kind: EntryKind, r: TrackRecord, private: bool) -> Result<TrackRecord, TrackError>;

    async fn login_with_code(&self, code: &str, pending: Option<PkceSession>) -> Result<(), TrackError>;
    async fn login_with_token(&self, token: &str) -> Result<(), TrackError>;   // AniList only
    fn logout(&self);
}

pub trait Deletable: Send + Sync {
    fn delete<'a>(&'a self, kind: EntryKind, r: &'a TrackRecord)
        -> Pin<Box<dyn Future<Output = Result<(), TrackError>> + Send + 'a>>;
}

pub trait Enhanced: Send + Sync {
    fn accepts(&self, source: &SourceInfo) -> bool;   // B§3.4 step 4
    fn match_entry(&self, kind: EntryKind, entry: &EntryRef) -> Option<SearchHit>; // match() returns 0..1
}

pub struct StatusOption { pub key: i64, pub label: Option<StatusLabel> }

pub enum StatusLabel {
    /// Built-in tracker label that exists in messages.rs (e.g. MsgKey::StatusReading).
    Key(MsgKey),
    /// Label text supplied by the tracker at runtime.
    Text(String),
}
pub struct SearchHit { pub remote_id: i64, pub title: String, pub tracking_url: String, pub private: bool, pub score: f64, pub status: i64, pub total: i64, pub progress: f64, pub start_date: String, pub cover_url: String, pub summary: String }
```

Notes for part 1 reconciliation:
- `Deletable` and `Enhanced` are sub-traits in part 1's `TrackerService`; the Kotlin names are `DeletableAnimeTracker`/`DeletableMangaTracker`/`EnhancedAnimeTracker`/`EnhancedMangaTracker` (behavior B§10.3, B§3.9). One trait per concept, parameterised by `EntryKind`, removes four copies.
- `SearchHit` mirrors `AnimeTrackSearch` fields (D§1.4). `private` defaults to false and is written by the confirm step (B§4.3).
- `StatusOption.label` is `Option<StatusLabel>` because Kotlin uses `StringResource?` (B§5); `None` renders as empty text (`TrackInfoDialogSelector.kt:80`).

Important: part 1's `set_*` implementations must reproduce the Kotlin side effects listed in B§5-B§8 (completion sets progress to total; first progress moves to watching; reaching total sets completion and finish date to now). Part 2 does not re-implement them. Test coverage for those rules is in T-27b-032 (edge file section 5).

---

## 5. Dialog state machines (GUI-free models in `src/track/*_model.rs` form, no GTK types)

Each model is a plain struct with `Send` data. The GTK widget code reads the model and calls `confirm()`/`cancel()`. This keeps the logic testable.

```rust
// Status (B§5)
pub struct StatusModel {
    pub item: TrackRecord,                 // copied at push time, like Kotlin's `track`
    pub tracker: Arc<dyn TrackerService>,
    pub kind: EntryKind,
    pub options: Vec<StatusOption>,         // computed once (remember { getSelections() })
    pub selection: i64,                     // initial = item.status
}
impl StatusModel {
    pub fn select(&mut self, key: i64) { self.selection = key; }
    /// Returns the job to run detached. Dialog closes immediately (B§5 confirm).
    pub fn confirm(self) -> DetachedJob { /* tracker.set_status(kind, item, selection) */ }
    // DetachedJob is defined in section 6.
}

// Progress (B§6)
pub struct ProgressModel {
    pub item: TrackRecord,
    pub tracker: Arc<dyn TrackerService>,
    pub kind: EntryKind,
    pub range: RangeInclusive<i64>,        // progress_range(item.total)
    pub selection: i64,                     // initial = truncate(item.progress)
}

// Score (B§7)
pub struct ScoreModel {
    pub item: TrackRecord,
    pub tracker: Arc<dyn TrackerService>,
    pub options: Vec<String>,               // score_options()
    pub selection: String,                  // initial = display_score(item)
}

// Dates (B§8)
pub struct DateModel {
    pub item: TrackRecord,
    pub tracker: Arc<dyn TrackerService>,
    pub start: bool,                         // true = start date
    pub initial_utc_day: NaiveDate,           // seed (B§8.2 rules, see dates.rs)
    pub bounds: DateBounds,                   // B§8.3
    pub selected_utc_day: Option<NaiveDate>,
}

// Remove tracking (B§10)
pub struct RemoveModel {
    pub item: TrackRecord,
    pub tracker: Arc<dyn TrackerService>,
    pub kind: EntryKind,
    pub deletable: bool,                       // tracker.as_deletable().is_some()
    pub also_remote: bool,                     // default false
}

// Search (B§4)
pub struct SearchModel {
    pub entry_id: i64,
    pub kind: EntryKind,
    pub tracker: Arc<dyn TrackerService>,
    pub current_url: Option<String>,
    pub query: String,
    pub state: SearchState,                    // Loading | Results(Vec<SearchHit>) | Failed(String)
    pub selected: Option<usize>,               // index into Results
    pub generation: u64,                       // Proposal: discards stale responses (D-6)
    pub supports_private: bool,
}

pub enum SearchState { Idle, Loading, Results(Vec<SearchHit>), Failed(String) }
```

### 5.1 Home model (B§3)
```rust
pub struct HomeModel {
    pub entry_id: i64,
    pub entry_title: String,
    pub source: SourceInfo,
    pub kind: EntryKind,
    pub items: Vec<TrackItem>,           // latest assembled list
    pub date_format: DateFormatChoice,   // from app_date_format (D§3)
}
```
Load sequence (Proposal, matching B§3.2):
1. Start `refresh_all(entry_id)` detached (B§3.3) and subscribe to the track store for `entry_id`.
2. On every store notification: re-read rows, `assemble_items(...)`, compare with previous (by `(tracker.id(), record)`), replace `items` only if changed, then redraw.
3. Refresh failures are not stored in the model; each one produces a toast through `toast::show(messages::track_error(name, msg))` (B§3.3 step 2).

### 5.2 TrackNav (modal stack)
```rust
pub enum TrackPage {
    Home(HomeModel),
    Status(StatusModel),
    Progress(ProgressModel),
    Score(ScoreModel),
    Date(DateModel),
    DateRemove(DateModel),        // carries the same data, as in B§8.4
    Search(SearchModel),
    Remove(RemoveModel),
}

pub struct TrackNav { stack: Vec<TrackPage> }
impl TrackNav {
    pub fn push(&mut self, page: TrackPage);
    pub fn pop(&mut self);                                  // navigator.pop()
    /// navigator.popUntil { it is HomeScreen }  (B§8.4, B§9)
    pub fn pop_until_home(&mut self);
}
```
The GTK side maps `TrackNav` to a stack of modal `gtk::Window`s (transient for the main window, modal), or to the main content stack for `Search`. Decision and reasons in `p2-ui-parity.md` section 1.

---

## 6. Async and threading model

Threads:
- GTK main thread: all widgets, `TrackNav`, models mutated on the main thread only.
- Tokio runtime: one multi-thread runtime owned by `kuro-core::AppContext` (area:01-foundation-app-bootstrap). Network, DB writes, and tracker calls run there.
- Results travel back with `async_channel::Sender<UiMsg>`. The GTK side drains it with `glib::spawn_future_local` (one per dialog instance).

| Kotlin construct (B§14) | Rust equivalent |
|---|---|
| `screenModelScope.launch` (cancelled with the screen) | `glib::spawn_future_local` owned by the dialog; the future is dropped when the window closes. Network work it awaits is `tokio::spawn`ed and the `JoinHandle` is dropped (keeps running only if the handle is detached). Use a `CancellationToken` to stop UI-side waiting. |
| `launchNonCancellable` (survives the screen) | `tokio::spawn` of the write job, no abort handle kept. Job body is the trait call + store write. Owned by `kuro-core` runtime; not tied to any window. |
| `withIOContext` | `tokio::task::spawn_blocking` only for rusqlite calls. Trait calls are already async (reqwest). |
| `withUIContext` (toast) | `tx.send(UiMsg::Toast(text))` then drained on the main thread. |
| `supervisorScope` + `async` per tracker in refresh | `futures::future::join_all` over per-tracker futures; each future maps its own error to `Option<(TrackerName, TrackError)>` so one failure does not cancel the rest. |
| `runBlocking` in Simkl identity (B§12.5) | Not reproduced: the Rust `login_with_code` is async. |

`DetachedJob` (used by every `confirm`) is `pub type DetachedJob = Pin<Box<dyn Future<Output = Result<TrackRecord, TrackError>> + Send + 'static>>;` and is spawned with `tokio::spawn` by the caller; the caller keeps no handle.

Serialization of remote writes (Proposal, new behavior): one `tokio::sync::Mutex<()>` per `(entry_id, tracker_id)` in `kuro-track`. Writes for the same row queue; this prevents two dialogs from racing `set_progress` and `set_status` on the same remote entry. Kotlin has no such lock (defect D-6 family).

Cancellation: closing a dialog never cancels a write that already started (matches B§14 "continue after the screen is popped"). A write that is queued but not started is also kept, because the user already pressed OK.

---

## 7. Errors and messages

### 7.1 `TrackError`
```rust
#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    #[error("{0}")]
    Remote(String),                  // raw tracker message (Kotlin prints e.message raw, B§13)
    #[error("not logged in")]
    NotLoggedIn,
    #[error("No match found")]       // MsgKey::ErrorNoMatch text
    NoMatch,
    #[error("unknown tracker {0}")]
    UnknownTracker(i64),             // replaces Kotlin `!!` (B§17 D-15)
    #[error("storage: {0}")]
    Store(String),
    #[error("unknown score label: {0}")]
    UnknownScore(String),            // replaces index -1 (B§7, D-8)
}
```
`TrackError::toast_text(&self, tracker_name: &str) -> String`:
- `Remote(m)` → `format!("{tracker_name} error: {m}")` (the `track_error` format, B§3.3 step 2). Empty `m` yields `"{name} error: "` exactly like Kotlin `e.message ?: ""`.
- `NoMatch` → `"No match found"`.
- Others: the same `"{name} error: {message}"` format for consistency.

### 7.2 Messages (`src/track/messages.rs` and `kuro-track::messages`)
A closed enum `MsgKey` with one variant per key in B§13 and the English value as a const. Localization is out of scope for this area (area:02 or i18n owns it); the enum is the single point to plug a translation table into.

Variants (exact English): `ErrorNoMatch` "No match found"; `TrackError` "%1$s error: %2$s" (formatted by function); `TrackStartedReadingDate` "Start date"; `TrackFinishedReadingDate` "Finish date"; `TrackRemoveDateConfTitle` "Remove date?"; `TrackRemoveStartDateConfText` "This will remove your previously selected start date from %s"; `TrackRemoveFinishDateConfText` "This will remove your previously selected finish date from %s"; `ActionCancel` "Cancel"; `ActionRemove` "Remove"; `ActionOk` "OK"; `TrackDeleteTitle` "Remove %s tracking?"; `TrackDeleteText` "This will remove the tracking locally."; `TrackDeleteRemoteText` "Also remove from %s"; `Status` "Status"; `Score` "Score"; `Chapters` "Chapters"; `Episodes` "Episodes"; `CopiedToClipboard` "Copied to clipboard:\n%1$s"; `TrackActivityName` "Tracker login".

Formatting helper: `fn fmt1(key: MsgKey, arg: &str) -> String` replaces `%s` / `%1$s` (only one argument per key here except `TrackError` with two; implement `track_error(name, msg)` explicitly).

### 7.3 Client credentials
- Client ids: constants in the tracker modules of part 1 (public values, D§5.1).
- Client secrets (Bangumi, Shikimori, Simkl; D§5.5): loaded by `kuro-track::config::ClientSecrets::load()` from, in order: env var `KURO_TRACK_BANGUMI_SECRET` / `KURO_TRACK_SHIKIMORI_SECRET` / `KURO_TRACK_SIMKL_SECRET`, then `$XDG_CONFIG_HOME`-equivalent through `kuro-platform::Paths::config_dir()` file `trackers.toml`. Missing secret → the tracker's login is disabled with message "Tracker not configured" (new message key `TrackerNotConfigured`, Proposal). Do NOT commit secret values.
- Decision needed from the user (open question OQ-1 in `p2-topic-oauth-callback.md`): keep the upstream-embedded secrets, or register Kurobayomi's own OAuth apps. Blocks T-27b-022 acceptance for these three trackers only.

---

## 8. Platform mapping (`kuro-platform`)

| Kotlin/Android API (B) | Rust / Linux | Where |
|---|---|---|
| `Context.openInBrowser(url)` (`ContextExtensions.kt:71-83`) | `kuro_platform::Opener::open_url(&str)`; Linux impl: `gtk::UriLauncher::new(url).launch(parent, cancellable, cb)` (GTK 4.10+, portal aware) with fallback `gio::AppInfo::launch_default_for_uri` | kuro-platform linux |
| `Context.copyToClipboard(label, content)` (`ContextExtensions.kt:39-52`) | `gdk::Display::default()?.clipboard().set_text(content)`. GTK abstracts the OS, so it stays in the GUI crate (addendum: GTK is the portable layer). Blank content → return (B§3.7). | `src/track/sheet.rs` |
| `Toast` via `Context.toast` | `src/track/toast.rs`: an overlay `gtk::Box` with a `gtk::Label`, auto-hide after 3500 ms (Proposal; no duration is set in the Kotlin toast call), stacked, `add_css_class("toast")`. | `src/track/toast.rs` |
| `TrackPreferences` tokens (`__PRIVATE_` keys) | `kuro_platform::SecretStore` (`get/set/delete` by string key). Linux impl: Secret Service via `keyring`, service name `kurobayomi`. Fallback when Secret Service is unavailable: refuse to store and show a message; do not write tokens to the config file. | kuro-platform linux |
| Intent filter `aniyomi://*-auth` (`AndroidManifest.xml:243-261`) | `kuro_platform::UriSchemeHandler`: installs a `.desktop` file with `MimeType=x-scheme-handler/aniyomi;` (packaging task, owned by area:36 deep links) and delivers inbound URIs through the GTK `Application::open` signal (GApplication `HANDLES_OPEN`). Single instance is provided by `gio::Application` with an app id. | kuro-platform linux + `src/main.rs` |
| `Intent.FLAG_ACTIVITY_CLEAR_TOP` / `MainActivity` relaunch (`BaseOAuthLoginActivity.kt:44-47`) | Not needed: `Application::open` calls `window.present()` on the existing main window and the tracking page is rebuilt from the model. Dropped (T-27b-033). | src/main.rs |

The `open` signal delivers `gio::File`s for files and URIs for schemes. Implementation detail in `p2-topic-oauth-callback.md`.

---

## 9. Mockup integration (existing code)

| Mockup symbol | Lines | Replacement |
|---|---|---|
| `src/entry.rs::tracking()` | 498-537 | `src/track/sheet.rs::build(entry_id, kind)` from `HomeModel.items`. Hard-coded rows "AniList" (bound), "MyAnimeList", "Kitsu" and `title_n(1)`, `"12 / 24"`, `"8.0"` are removed. |
| `src/entry.rs::tracker_search()` | 539-566 | `src/track/search.rs::build(SearchModel)`; 4 hard-coded result rows removed; "Track" and "Track privately" buttons wired to `SearchModel::confirm`. |
| `src/entry.rs::dialog_of` | 25-29 | `TrackNav::push(TrackPage::...)`. |
| `spec/entries.json` dialogs `status`, `chapters`, `episodes`, `score`, `start_date`, `finish_date`, `remove_date`, `remove_tracking` (lines 1387-1681) | spec | Keep only as label source for the static UI text; the `buttons` arrays there show "Remove" on date dialogs which matches B§8.1. The `remove_tracking` switch label "Also remove from AniList" is generated per tracker. |
| `src/ui.rs::show_dialog` (line 406) | — | Used for simple confirmation only (remove date, remove tracking); the rest are custom windows. |
| `src/ui.rs::push` / `pop` (lines 34-47) | — | Used for `Search` page (non-modal, full page), as the mockup does today. |

The mockup's `tracking_sheet` menu (`spec/entries.json:1387-1420`: "Open in browser", "Copy link", "Track privately", "Track publicly", "Remove") maps 1:1 to B§3.5 callbacks. "Track privately"/"Track publicly" is a single toggle in Kotlin (B§3.8); the mockup shows two entries. Proposal: keep one toggle labelled by the current state.

---

## 10. Login return path (summary; full plan in `p2-topic-oauth-callback.md`)
1. `kuro-platform` receives `aniyomi://<host>?...#...` via `Application::open`.
2. `kuro-track::oauth::callback::parse_callback(uri) -> CallbackOutcome` (pure, tested; full enum in `p2-topic-oauth-callback.md` section 5): `AniListToken { token, expires_in }`, `Code { tracker, code }`, `Denied { tracker }`, `Malformed { tracker }`, `UnknownHost(String)`, `NotAllowed { tracker }` (login CSRF guard, topic section 7.3).
3. `src/track/login.rs` runs the matching `login_with_*` (detached job) and shows a toast with the result; the settings page (area:47) refreshes its row.
4. No window creation, no "loading" activity. A visible spinner is shown in the settings row while the job runs (Proposal, replaces `LoadingScreen`).

---

## 11. Pitfalls
- GTK objects are not `Send`. Never move `gtk::Widget` or `TrackPage` containing widgets into `tokio::spawn`. Move the plain model data only.
- `async-trait` boxes every call. Fine at this call rate (user action level), do not use it inside the image or list render loops.
- `chrono::Local` reads `TZ` at process start only if the environment changes; use `Local::now()` at each validation (picker bounds), do not cache "today".
- Date conversions are the easiest place to get off-by-one errors. Use the table in `p2-edge-cases-and-tests.md` section 3 as the test source.
- PKCE verifier must be stored per attempt (`PendingLogins`), never in a static (Kotlin keeps it in a companion `var`, `MyAnimeListApi.kt:390`; a process restart between browser launch and callback breaks the exchange).
- `Application::open` can deliver the callback before the main window exists (cold start by URI). Create the window first, then route the URI (see topic file).
- `keyring` on KDE needs the Secret Service provider (KWallet exposes it). If absent, show the explicit error from section 8, do not fall back to a plain file.
- `gtk::Calendar` cannot disable days. Clamp on select (see `p2-ui-parity.md` section 6).
- The shared implementation over `EntryKind` must not branch on kind inside the GTK widget code more than necessary; put label and field differences in `EntryKind` methods.
- Query decoding for OAuth callbacks: use `percent_encoding::percent_decode_str`, not `url::Url::query_pairs()` or `form_urlencoded`, because those turn `+` into a space and Android `Uri.getQueryParameter` does not (edge case E-52).
- Never log a full `aniyomi://` URI or a token. Log scheme and host only (topic section 3, 9).
