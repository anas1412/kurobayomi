# Area 53 — Rust design: launch glue, notices, desktop integration, network status, job holds

Constraints taken from the plan set: GTK 4.16+ through gtk4-rs 0.11 without libadwaita (`/home/blackbox/kurobayomi/Cargo.toml` already has `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`); tokio; no OS code outside `kuro-platform` (`plans/addenda/requirements-cross-platform.md`, rule 1); trait-based platform services with Linux implementations first (rule 2); no globals (rule 3); no path string literals (rule 4). Companion files: `p1-behavior.md` (what Aniyomi does), `p1-data-and-config.md` (ids, actions, extras, endpoints), `p1-ui-parity.md` (what the user sees), `p1-edge-cases-and-tests.md`, `p1-tasks.md`.

Decisions taken from sibling areas and reused here, not re-decided: launch route table and `LaunchRequest` (area:36 `p1-topic-deeplinks-intents.md` section 2 and `p1-rust-design.md`); `AppAction` JSON codec and the `app.notice` GAction transport, stable string notice ids, categories instead of channels, no progress bar, no bulk withdraw (area:35 `p1-topic-notifications.md` sections 3 to 5); periodic scheduling and `Scheduler` (area:26 `p1-rust-design.md` section 2.7); workspace names `kuro-core`, `kuro-platform`, `kuro-testkit`, binary `kurobayomi` (area:01 `p1-tasks.md` T-01a-001).

## 1. Crate and module layout (this area's additions)

```
Cargo workspace (area:01)
├── kuro-core/                       no GTK, no OS code
│   └── src/
│       ├── launch/                  area:36 owns route_request and LaunchRequest
│       │   ├── mod.rs               LaunchRequest, Kind, ShortcutAction, NavAction, RouteError   (area:36)
│       │   ├── uri.rs               parse_uri(&Url) -> Result<LaunchRequest, LaunchError>        (this area)
│       │   └── argv.rs              parse_argv(&[OsString]) -> Result<LaunchRequest, LaunchError> (this area)
│       ├── notify/
│       │   ├── actions.rs           AppAction enum + JSON codec                                    (area:35, used here)
│       │   ├── ids.rs               NoticeId newtype, registry of fixed ids and their categories  (this area, T-53a-020)
│       │   ├── category.rs          Category enum and switches (notify.enabled.<category>)        (area:35 decision 1)
│       │   └── payload.rs           validation of AppAction payloads (URL, path, id, size)        (this area, T-53a-018)
│       ├── deeplink/
│       │   ├── anime.rs             resolve_anime(query, ctx) -> Result<Resolved, ResolveError>   (this area owns logic, T-53a-048)
│       │   └── manga.rs             resolve_manga(query, ctx) -> Result<Resolved, ResolveError>   (T-53a-050)
├── kuro-platform/                   all OS code
│   └── src/
│       ├── traits.rs                Notifier, Opener, SingleInstance, NetworkStatus, Paths (area:01)
│       └── linux/
│           ├── desktop_entry.rs     writes/validates the .desktop file and [Desktop Action] groups (T-53a-011)
│           ├── mime.rs              shared-mime-info XML for application/x-tachibk (T-53a-003)
│           ├── notify.rs            gio::Notification sender + withdraw (T-53a-017, area:35)
│           ├── opener.rs            gtk::UriLauncher, FileManager1.ShowItems, portal OpenURI (T-53a-039)
│           ├── network.rs           gio::NetworkMonitor + NetworkManager properties via zbus (T-53a-026)
│           ├── folder.rs            folder chooser through gtk::FileDialog::select_folder (T-53a-027)
│           ├── search_provider.rs   org.gnome.Shell.SearchProvider2 D-Bus object (T-53a-066, P2)
│           └── hold.rs              gio::ApplicationHoldGuard wrapper (T-53a-037)
├── kuro-testkit/                    fixtures: fake NetworkStatus, recording Notifier, Url samples
└── src/                             the GTK binary "kurobayomi" (mockup today)
    ├── main.rs                      Application builder, connect_command_line, connect_open, actions
    ├── launch.rs                    applies LaunchRequest to the window (calls route_request, then NavAction)
    ├── notice.rs                    app.notice GAction and dispatch table; Dismiss by id
    ├── nav.rs                       Transition table (shared-axis push/pop mapping) and NavAction executor
    ├── toast.rs                     in-app toast for RouteError and failed notices (behaviour 8, edge E-16)
    ├── screens/deeplink.rs          loading page for resolve (replaces DeepLink*Screen)
    └── screens/downloads.rs         download row and menu (area:23/24 owns behaviour; widget here)
```

Mapping to the existing mockup files (`src/ui.rs`, `src/screens.rs`, `src/spec.rs`, `spec/*.json`):

| Mockup item | Use in this area |
|---|---|
| `ui::open(id)` (`src/ui.rs:88` to `:99`) | executes `NavAction::SelectTab` for ids `library`, `updates`, `history`, `browse`, `more` (these are the `spec/app.json` `nav` ids); other ids push a screen |
| `ui::push`, `ui::pop`, `ui::pop_to_root` (`src/ui.rs:34` to `:55`) | `NavAction::Push`, `NavAction::Pop`, `NavAction::PopToRoot` |
| `ui::nav()` first child `Stack` (`src/ui.rs:88` to `:99`) | the tab stack that `SelectTab` switches |
| `screens::downloads()` (`src/screens.rs:354`) | the More → Downloads view that `Shortcut { Downloads }` reaches |
| `screens::open_entry`, `open_media` (`src/screens.rs:62` to `:69`) | entry and media pages for `OpenManga`, `OpenAnime`, `OpenChapter`, `OpenEpisode` once their data is loaded |
| `screen:reader`, `screen:player` widget names (`src/main.rs:58`) | immersive detection; the transition table uses the same names |
| `gtk::Application::builder().application_id("io.github.blackbox.Kurobayomi")` (`src/main.rs:13`) | GApplication identity used by D-Bus notifications, desktop file, single-instance forwarding |
| `NON_UNIQUE` flag for `KUROBAYOMI_SMOKE`, `KUROBAYOMI_OPEN`, `KUROBAYOMI_SHOT` (`src/main.rs:14` to `:16`) | keep for development; release builds stay unique |
| `app.connect_activate(build_ui)` (`src/main.rs:18`) | no `open` or `command-line` handler exists in the mockup; this area adds both (section 6.1) |

## 2. Types

### 2.1 Launch parsing (kuro-core, this area's additions to area:36)

```rust
// kuro-core/src/launch/uri.rs  (this area)
use url::Url;

/// Schemes this app accepts for its own links. `kurobayomi` is primary; `aniyomi` and
/// `tachiyomi` are aliases that Aniyomi-era links still use (behaviour section 2.2, data section 4.2).
pub const ACCEPTED_SCHEMES: [&str; 3] = ["kurobayomi", "aniyomi", "tachiyomi"];

/// Parses an incoming URI into the launch request defined by area:36.
/// Pure: no I/O, no globals.
pub fn parse_uri(raw: &str) -> Result<LaunchRequest, LaunchError> {
    let url = Url::parse(raw).map_err(|e| LaunchError::BadUrl(e.to_string()))?;
    if !ACCEPTED_SCHEMES.contains(&url.scheme()) {
        return Err(LaunchError::UnsupportedScheme(url.scheme().to_owned()));
    }
    // host decides the route. Unknown host is an error, never a silent no-op (edge E-23).
    match url.host_str() {
        Some("add-repo") => Ok(LaunchRequest::OpenUri(url)),        // manga repos for tachiyomi://, anime stores for aniyomi:// (area:36 table)
        Some("extension-store") => Ok(LaunchRequest::OpenUri(url)), // aniyomi:// only, enforced in route_request
        Some(h) if h.ends_with("-auth") => Ok(LaunchRequest::OpenUri(url)), // tracker OAuth (area:27)
        Some(other) => Err(LaunchError::UnknownHost(other.to_owned())),
        None => Err(LaunchError::BadUrl("missing host".into())),
    }
}
```

```rust
// kuro-core/src/launch/argv.rs  (this area)
use std::ffi::OsString;

/// Command-line grammar (documented in `--help`, Proposal):
///   kurobayomi [--action=<shortcut>] [--id=<i64>] [--search=<text>] [--type=manga|anime]
///              [<file.tachibk>] [<url>]...
/// `--action` values are the five desktop actions in behaviour section 2.4 plus the
/// history and extension actions of area:36 (`show-library`, `show-animelib`,
/// `show-updates`, `show-history`, `show-browse`, `show-extensions`, `show-anime-extensions`,
/// `show-downloads`, `show-anime-downloads`, `show-manga`, `show-anime`).
pub fn parse_argv(args: &[OsString]) -> Result<Vec<LaunchRequest>, LaunchError>;
```

Rules: `--search` with empty text yields no request (area:36 `Search { "", _ }` → `[]`). `--type` defaults to `anime` (behaviour section 2.2 step 2: `.ifBlank { "ANIME" }`). An argument that ends with `.tachibk` (after `Path` component check) becomes `LaunchRequest::OpenFile`. A bare `http(s)://` argument becomes `LaunchRequest::Search` with the URL as text (behaviour section 2.8 Proposal). Any other string is `LaunchError::Unrecognised`.

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LaunchError {
    #[error("not a valid URL: {0}")]
    BadUrl(String),
    #[error("unsupported URL scheme: {0}")]
    UnsupportedScheme(String),
    #[error("unknown URL host: {0}")]
    UnknownHost(String),
    #[error("unrecognised argument: {0}")]
    Unrecognised(String),
    #[error("--action {0} needs --id")]
    MissingId(String),
}
```

Dependencies: `url` (WHATWG URL parser, already a transitive dependency of reqwest), `thiserror`. No `clap` (the grammar is seven flags; `lexopt` 0.3 is an option only if the grammar grows).

### 2.2 Notices (kuro-core, area:35 types reused)

```rust
// kuro-core/src/notify/ids.rs  (this area)
/// Stable, human-readable notice id. Replaces the Android integer ids (data section 3.3).
/// Invariant: ASCII, 1..=64 bytes, `[a-z0-9:-]`, no leading or trailing separator.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NoticeId(String);

impl NoticeId {
    pub const APP_UPDATE_PROGRESS: &'static str = "notice:app-update-progress";
    pub const APP_UPDATE_PROMPT: &'static str = "notice:app-update-prompt";
    pub const APP_UPDATE_ERROR: &'static str = "notice:app-update-error";
    pub const SAVE_IMAGE: &'static str = "notice:save-image";
    pub const INCOGNITO: &'static str = "notice:incognito";
    pub const TORRENT_SERVER: &'static str = "notice:torrent-server";
    pub const HTTP_SERVER: &'static str = "notice:http-server";
    pub const NEW_CHAPTERS: &'static str = "notice:new-chapters";
    pub const NEW_EPISODES: &'static str = "notice:new-episodes";
    pub const EXTENSIONS_UPDATES: &'static str = "notice:extensions-updates";
    pub const EXTENSION_INSTALL: &'static str = "notice:extension-install";
    pub const LIBRARY_PROGRESS: &'static str = "notice:library-progress";
    pub const LIBRARY_ERROR: &'static str = "notice:library-error";
    pub const LIBRARY_SIZE_WARNING: &'static str = "notice:library-size-warning";
    pub const DOWNLOAD_PROGRESS: &'static str = "notice:download-progress";
    pub const DOWNLOAD_ERROR: &'static str = "notice:download-error";
    pub const BACKUP_PROGRESS: &'static str = "notice:backup-progress";
    pub const BACKUP_COMPLETE: &'static str = "notice:backup-complete";
    pub const RESTORE_PROGRESS: &'static str = "notice:restore-progress";
    pub const RESTORE_COMPLETE: &'static str = "notice:restore-complete";

    /// Per-entry notice: `notice:entry:<manga|anime>:<entry-id>`.
    pub fn entry(kind: Kind, entry_id: i64) -> Self;
}
```

```rust
// kuro-core/src/notify/category.rs  (area:35 decision 1; mapping from the Android channels)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Progress,        // was LOW progress channels (library, downloader, backup/restore)
    Errors,          // was LOW errors channels
    NewEntries,      // was DEFAULT new_chapters_episodes_channel
    Complete,        // was HIGH backup_restore_complete_channel_v2 (urgency normal, no sound)
    AppUpdates,      // was DEFAULT app_apk_update_channel
    ExtensionUpdates,// was DEFAULT ext_apk_update_channel
    Incognito,       // was LOW incognito_mode_channel
    Servers,         // was LOW torrent_server_channel and http_server_channel
    Common,          // was LOW common_channel
}

impl Category {
    /// Preference key that switches the category (area:02 key naming): `notify.enabled.<name>`, default true.
    pub fn pref_key(self) -> &'static str;
    /// Freedesktop `category` hint and urgency for the desktop server (see section 3.3).
    pub fn hints(self) -> CategoryHints;
}
```

Note the mapping of `Progress`: the Android channel "library_progress_channel" also appears in the deprecated list; the Linux category is never deleted (defect E-02 is not carried).

```rust
// kuro-core/src/notify/actions.rs  (area:35 section 3.1; additions from this area)
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AppAction {
    Dismiss { notice: NoticeId },
    ResumeDownloads { kind: Kind },
    PauseDownloads { kind: Kind },
    ClearDownloads { kind: Kind },
    MarkChaptersRead { manga_id: i64, urls: Vec<String>, notice: Option<NoticeId>, group: Option<NoticeId> },
    MarkEpisodesSeen { anime_id: i64, urls: Vec<String>, notice: Option<NoticeId>, group: Option<NoticeId> },
    DownloadChapters { manga_id: i64, urls: Vec<String>, notice: Option<NoticeId>, group: Option<NoticeId> },
    DownloadEpisodes { anime_id: i64, urls: Vec<String>, notice: Option<NoticeId>, group: Option<NoticeId> },
    OpenManga { id: i64, notice: Option<NoticeId> },
    OpenAnime { id: i64, notice: Option<NoticeId> },
    OpenChapter { manga_id: i64, chapter_id: i64, notice: Option<NoticeId> },
    OpenEpisode { anime_id: i64, episode_id: i64, notice: Option<NoticeId> },
    CancelLibraryUpdate { kind: Kind },
    CancelRestore,
    StartAppUpdate { url: url::Url, title: Option<String> },
    CancelAppUpdate,
    RevealFile { path: std::path::PathBuf },
    OpenFile { path: std::path::PathBuf },
    OpenUrl { url: url::Url },
    CopyText { text: String },
    DisableIncognito,
    Shortcut { shortcut: ShortcutAction, id: Option<i64> },
}
```

`Kind` and `ShortcutAction` are from area:36 (`Kind::{Manga, Anime}`, `ShortcutAction` variants listed there). `Dismiss` is the only variant that withdraws notices; no variant exists for "withdraw all" (defect E-01 is structurally impossible).

Payload validation (`kuro-core/src/notify/payload.rs`, T-53a-018):

```rust
pub const MAX_URLS_PER_ACTION: usize = 500;     // Proposal: a chapter list is bounded by one library update batch
pub const MAX_URL_BYTES: usize = 2048;          // Proposal: matches common browser limit
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024; // Proposal: GAction target size guard

pub fn validate(action: &AppAction) -> Result<(), PayloadError>;
// Rules:
// - every url string parses with url::Url and has scheme https or http (for source urls) only where the variant allows it
// - StartAppUpdate and OpenUrl accept https only (the release download links are https)
// - RevealFile and OpenFile paths must be absolute and must start with one of the Paths roots (cache, data, download folders)
// - ids must be > 0 for entries (Android uses hashCode of Long, so negative ids cannot occur from the store)
// - urls.len() <= MAX_URLS_PER_ACTION; each url.len() <= MAX_URL_BYTES
```

Errors: `PayloadError::{TooLarge, BadUrl(String), BadScheme(String), NotAbsolute(PathBuf), OutsideRoots(PathBuf), TooManyUrls(usize), BadId(i64)}` (thiserror).

### 2.3 Platform traits (kuro-platform/src/traits.rs)

These follow `plans/addenda/requirements-cross-platform.md` (the trait table). Signatures given here are the ones this area needs; the rest of the table belongs to other areas.

```rust
use crate::{NoticeId, Category, NetworkState};

/// Desktop notices. Implementations never block; `post` returns once the server has accepted the notice.
pub trait Notifier: Send + Sync {
    fn post(&self, notice: &Notice) -> Result<(), NotifyError>;
    fn withdraw(&self, id: &NoticeId);          // withdraw one id only; no bulk call exists on the trait
    fn server_available(&self) -> bool;         // replaces POST_NOTIFICATIONS gating (behaviour 5.4)
}

pub struct Notice {
    pub id: NoticeId,
    pub category: Category,
    pub title: String,
    pub body: String,
    pub urgency: Urgency,                       // Low, Normal, Critical (critical only for error prompts that need attention)
    pub buttons: Vec<NoticeButton>,             // at most 3 (Proposal; server limits vary)
    pub default_action: Option<AppAction>,      // what clicking the body does (area:35 decision 6)
}

pub struct NoticeButton { pub label: String, pub action: AppAction }

/// Opens files, folders and URLs with the desktop's handlers.
pub trait Opener: Send + Sync {
    fn open_url(&self, url: &url::Url) -> Result<(), OpenError>;
    fn open_file(&self, path: &std::path::Path) -> Result<(), OpenError>;
    fn reveal(&self, path: &std::path::Path) -> Result<(), OpenError>; // FileManager1.ShowItems, falls back to open_file(parent)
}

/// One running instance; forwards launch requests from a second process.
pub trait SingleInstance: Send + Sync {
    fn forward(&self, args: &[std::ffi::OsString]) -> Result<bool, SingleInstanceError>; // true when handled by the running instance
}

/// Live connectivity. `subscribe` yields the current value immediately and then every change.
pub trait NetworkStatus: Send + Sync {
    fn current(&self) -> NetworkState;
    fn subscribe(&self) -> tokio::sync::watch::Receiver<NetworkState>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct NetworkState {
    pub is_connected: bool,   // gio::NetworkMonitor::is_network_available()
    pub is_validated: bool,   // gio::NetworkMonitor::connectivity() == Full
    pub is_wifi: bool,        // NM WirelessEnabled && PrimaryConnectionType == "802-11-wireless"
    pub is_metered: bool,     // gio::NetworkMonitor::network_metered() || NM Metered in {1, 3}
}
impl NetworkState {
    pub fn is_online(&self) -> bool { self.is_connected && self.is_validated } // same rule as Kotlin isOnline
}
```

`Paths` (area:01) supplies `cache_dir()`, `data_dir()` and `download_default()`, among the other directories listed in the addendum. This area calls `Paths` only; no path literals.

### 2.4 Holds and jobs

```rust
// kuro-platform/src/linux/hold.rs  (T-53a-037)
/// Keeps the application process alive while background work runs, even with no window.
/// Replaces Android foreground services (behaviour section 9.5).
pub struct BackgroundHold {
    _guard: gtk::gio::ApplicationHoldGuard,   // released on drop
    label: &'static str,                      // for logs: "torrent-server", "http-server", "library-update", "backup", "app-update"
}

pub struct HoldRegistry { /* counts holds per label; gio hold is taken on 0 -> 1 and released on 1 -> 0 */ }
impl HoldRegistry {
    pub fn acquire(&self, label: &'static str) -> BackgroundHold;
    pub fn active(&self) -> Vec<&'static str>;
}
```

Holds are taken on the GTK main thread; the guard is `Send`? `gio::ApplicationHoldGuard` is not `Send`, so holds are created on the main thread and passed around as `HoldToken` (an `Arc<AtomicUsize>` counter plus a message to the main thread that owns the guard). This keeps all GTK objects on one thread (see section 5).

The app-update download uses the job runner of area:26. Its semantics for this area are `replace` (a new start cancels the running download, tag `AppUpdateDownload`, Android `ExistingWorkPolicy.REPLACE`) and `cancel`. Signature (area:26 `Scheduler` extension, Proposal):

```rust
pub struct UniqueJob { pub name: &'static str, pub policy: JobPolicy }
pub enum JobPolicy { Replace, Keep }
pub fn run_unique<F, Fut>(&self, job: UniqueJob, f: F) -> JobHandle
where F: FnOnce(CancellationToken) -> Fut + Send + 'static, Fut: Future<Output = JobOutcome> + Send;
// Replace cancels the current token for `name` and waits for it to finish before starting (the Kotlin REPLACE semantics).
```

### 2.5 Progress throttle (kuro-core, T-53a-044)

```rust
// Reproduces AppUpdateDownloadJob.kt:82 to :98 exactly.
pub struct ProgressThrottle { saved: u8, last_tick: Option<std::time::Instant> }
impl ProgressThrottle {
    pub const MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);
    pub fn new() -> Self { Self { saved: 0, last_tick: None } }
    /// Returns Some(percent) when a notice update must be sent.
    /// percent = floor(100 * bytes / total) as u8; total == 0 yields None (Kotlin divides by zero: edge E-21).
    pub fn offer(&mut self, bytes: u64, total: u64, now: std::time::Instant) -> Option<u8> {
        if total == 0 { return None; }
        let p = ((100.0 * (bytes as f32 / total as f32)) as u8).min(100);
        let due = self.last_tick.map_or(true, |t| now.duration_since(t) > Self::MIN_INTERVAL);
        if p > self.saved && due {
            self.saved = p;
            self.last_tick = Some(now);
            Some(p)
        } else { None }
    }
}
```

The Kotlin comparison is `currentTime - 200 > lastTick` with `lastTick = 0` at start. Rust uses `Option<Instant>` so the first update is always due. The strict `>` is kept.

### 2.6 Desktop entry and MIME (kuro-platform/src/linux/desktop_entry.rs, mime.rs)

Desktop entry (packaging installs it under `share/applications/io.github.blackbox.Kurobayomi.desktop`; the app verifies it at start, T-53a-011):

```ini
[Desktop Entry]
Type=Application
Name=Kurobayomi
GenericName=Manga and anime reader
Comment=Read manga and watch anime
Exec=kurobayomi %U
Icon=io.github.blackbox.Kurobayomi
Terminal=false
Categories=AudioVideo;Graphics;Viewer;
StartupNotify=true
DBusActivatable=true
StartupWMClass=io.github.blackbox.Kurobayomi
MimeType=application/x-tachibk;x-scheme-handler/kurobayomi;x-scheme-handler/aniyomi;x-scheme-handler/tachiyomi;
Actions=show-library;show-animelib;show-updates;show-history;show-browse;

[Desktop Action show-library]
Name=Manga library
Exec=kurobayomi --action=show-library

[Desktop Action show-animelib]
Name=Anime library
Exec=kurobayomi --action=show-animelib

[Desktop Action show-updates]
Name=Updates
Exec=kurobayomi --action=show-updates

[Desktop Action show-history]
Name=History
Exec=kurobayomi --action=show-history

[Desktop Action show-browse]
Name=Browse
Exec=kurobayomi --action=show-browse
```

`DBusActivatable=true` is required so that a notification click can start the application when it is not running (GApplication activation over D-Bus). The `Name=` values here are English defaults; localised names come from the packaging step (area:51). `Categories` and `StartupWMClass` values are the reverse-DNS id, so KDE groups the window with the launcher.

MIME (shared-mime-info, `kuro-platform/src/linux/mime.rs` writes the XML at packaging time):

```xml
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-tachibk">
    <comment>Kurobayomi backup</comment>
    <sub-class-of type="application/gzip"/>
    <glob pattern="*.tachibk"/>
  </mime-type>
</mime-info>
```

Rationale: the Kotlin backup MIME is `application/x-protobuf+gzip` for sharing (`NotificationReceiver.kt:93`), while the file extension is `.tachibk`. Association is by extension (the manifest matched by path pattern). `gzip` subclass lets generic archive tools open the file.

### 2.7 Network status (kuro-platform/src/linux/network.rs, T-53a-026)

```rust
pub struct LinuxNetwork { /* gio::NetworkMonitor + zbus::Connection (system bus) */ }
impl NetworkStatus for LinuxNetwork { /* see section 3.2 */ }
```

Wi-Fi detection reads two NetworkManager properties on `org.freedesktop.NetworkManager` (`/org/freedesktop/NetworkManager`): `WirelessEnabled` (bool) and `PrimaryConnectionType` (string). Metered reads `Metered` (uint32). If the system bus or NetworkManager is absent, `is_wifi = false` and `is_metered` falls back to GIO. Subscription: `PropertiesChanged` signal on the same object plus GIO `network-changed` signal; both update one `watch::Sender<NetworkState>`.

### 2.8 Search provider (kuro-platform/src/linux/search_provider.rs, P2, T-53a-066)

The GNOME Shell search provider interface `org.gnome.Shell.SearchProvider2` has methods `GetInitialResultSet(terms)`, `GetSubsearchResultSet(prev, terms)`, `GetResultMetas(ids)`, `ActivateResult(id, terms, timestamp)`, `LaunchSearch(terms, timestamp)`. The implementation serves one result per manga or anime title; `ActivateResult` sends `LaunchRequest::Search` to the running instance through `SingleInstance`. The `.ini` file for GNOME is written at packaging time (`share/gnome-shell/search-providers/io.github.blackbox.Kurobayomi.search-provider.ini`, with `DesktopId=io.github.blackbox.Kurobayomi.desktop` and `BusName=io.github.blackbox.Kurobayomi.SearchProvider`, `ObjectPath=/io/github/blackbox/Kurobayomi/SearchProvider`, `Version=2`).

## 3. Platform implementations

### 3.1 Notifier (Linux)

`kuro-platform/src/linux/notify.rs` builds a `gio::Notification` per `Notice`:

- `set_title`, `set_body` from `Notice`.
- `set_priority`: `Low` → `NotificationPriority::Low`, `Normal` → `Normal`, `Critical` → `Urgent`. `Complete` (Android HIGH) maps to `Normal` (a `High` priority is not a GIO value; Urgent would persist).
- `set_default_action_and_target_value("app.notice", Some(&serde_json::to_string(&action)?.to_variant()))` for `default_action`.
- `add_button_with_target_value(label, "app.notice", payload)` for up to three buttons.
- post with `gtk::Application::send_notification(Some(id.as_str()), &notification)`; withdraw with `withdraw_notification(id.as_str())`.
- `server_available()` checks the `org.freedesktop.Notifications` name on the session bus (zbus `NameHasOwner`) or `xdg-desktop-portal` availability inside Flatpak; false means the caller shows an in-app toast (behaviour 5.4).
- Progress is not shown in notices (area:35 decision 4). Download progress lives in the download screen. Notice text for progress is updated at most every 200 ms and only when the percent rises (`ProgressThrottle`, section 2.5).

### 3.2 Opener (Linux)

- `open_url`: `gtk::UriLauncher::new(url).launch(Some(&window), None::<&gio::Cancellable>, callback)` (GTK 4.10 and above; the portal is used automatically inside Flatpak).
- `open_file`: `gtk::FileLauncher::new(&gio::File::for_path(path)).launch(...)` (GTK 4.10 and above).
- `reveal`: D-Bus call `org.freedesktop.FileManager1.ShowItems(["file://…"], "")` on the session bus; on failure, `open_file(path.parent())`.
- Never call `xdg-open` directly (rule 1 of the addendum).

### 3.3 Category hints (Linux desktop server)

Freedesktop notification hints used via GIO are limited to what GLib exposes; the category is set through the `category` hint only with `notify-rust` or direct D-Bus. Decision for this area: GIO only (area:35 decision list). Therefore categories map to the title prefix and to the in-app switch; no `category` hint is sent. `CategoryHints` (section 2.2) exists for a later direct-D-Bus fallback and is documented here, not implemented in this area.

### 3.4 Folder chooser (T-53a-027)

`gtk::FileDialog::new()` with `select_folder(Some(&window), None::<&gio::Cancellable>, callback)`. Inside Flatpak the call uses `org.freedesktop.portal.FileChooser`; the granted folder is persisted through the portal document store (`ashpd` `document` API) so the path survives restarts. Outside Flatpak the path is the user's selection.

## 4. Route application (src/launch.rs and src/nav.rs)

```rust
// src/launch.rs  (kurobayomi binary)
pub fn apply(window: &gtk::ApplicationWindow, req: LaunchRequest) {
    match kuro_core::launch::route_request(&req) {
        Ok((actions, ready)) => {
            for a in actions { crate::nav::execute(window, a); }
            if ready { window.present(); }        // the window is shown after the navigation (mirrors MainActivity ready flag)
        }
        Err(e) => crate::toast::show(window, &e.to_string()),  // RouteError: missing id, unknown kind
    }
}
```

```rust
// src/nav.rs  (kurobayomi binary)
pub fn execute(window: &gtk::ApplicationWindow, action: NavAction) {
    match action {
        NavAction::SelectTab(tab) => crate::ui::open(tab.spec_id()),     // library, updates, history, browse, more
        NavAction::PopToRoot => crate::ui::pop_to_root(),
        NavAction::Push(screen) => crate::ui::push(&screen.build()),     // screen.build() returns the page widget
        NavAction::Pop => crate::ui::pop(),
    }
}

/// Transition table (this area). Replaces shared_axis_x_* resources (behaviour 14.3).
pub enum Transition { SharedAxisPush, SharedAxisPop }
impl Transition {
    pub fn stack_type(self) -> gtk::StackTransitionType {
        match self { Self::SharedAxisPush => gtk::StackTransitionType::SlideLeft,
                     Self::SharedAxisPop  => gtk::StackTransitionType::SlideRight }
    }
    pub const DURATION_MS: u32 = 300;   // translate duration in the Kotlin XML
}
```

`crate::ui::push` and `pop` in the mockup (`src/ui.rs:34` to `:47`) do not yet set a transition; the Proposal sets `nav.set_transition_type(...)` and `set_transition_duration(300)` before `set_visible_child` (the stack is `SlideLeftRight` today; `main.rs:23`). Pop uses `SlideRight`.

## 5. Threading and the GTK main loop

- The GTK main thread owns every widget, every `gio::Application` object and every `ApplicationHoldGuard`.
- Background work runs on the tokio runtime created once in `main` (area:26: `worker_threads = 4`, Proposal). The runtime handle is passed to the `Scheduler`, the resolvers and the notice dispatcher.
- Results return to the main thread with `glib::MainContext::default().spawn_local` inside a future that awaits an `async_channel::Receiver`, or with `glib::MainContext::channel`. No `runBlocking` equivalent exists: the Kotlin `runBlocking` on the broadcast thread (defect E-03) is replaced by an async handler that returns immediately.

Sequence for a notice button press:

```rust
// src/notice.rs
pub fn install(app: &gtk::Application, handle: tokio::runtime::Handle, ctx: AppCtx) {
    let action = gtk::gio::SimpleAction::new("notice", Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        let Some(json) = param.and_then(|v| v.get::<String>()) else { return };
        let action: AppAction = match serde_json::from_str(&json) {
            Ok(a) => a,
            Err(e) => { tracing::warn!("bad notice payload: {e}"); return; }
        };
        if let Err(e) = kuro_core::notify::payload::validate(&action) {
            tracing::warn!("rejected notice payload: {e}"); return;
        }
        match action {
            AppAction::Dismiss { notice } => ctx.notifier.withdraw(&notice),     // withdraw one id only
            other => dispatch(&handle, ctx.clone(), other),                       // spawns on tokio, result via glib
        }
    });
    app.add_action(&action);
}
```

Notes: `app.notice` must be added before `activate` so a D-Bus activation from a notification can run it (GApplication). `tracing` is used for logs; the existing mockup has no logging crate, so this area adds `tracing` and `tracing-subscriber` only in the binary.

## 6. GApplication wiring (src/main.rs changes)

### 6.1 Command line and open

The mockup connects only `activate`. This area adds:

- `app.add_main_option` entries for `--action`, `--id`, `--search`, `--type` (strings and ints) so `--help` is generated by GLib.
- `app.connect_command_line(|app, cmdline| { let args = cmdline.arguments(); ... })` → `parse_argv` → queue the `LaunchRequest`s until the window exists. Returns `ExitCode::SUCCESS`.
- `app.connect_open(|app, files, hint| ...)` for file activation (`.tachibk` from the file manager). Each `gio::File::path()` is parsed as `OpenFile` when it ends in `.tachibk`.
- For a second process: GApplication forwards command-line and open calls to the primary instance automatically (no custom IPC). This is the `SingleInstance::forward` implementation for the GTK binary.

### 6.2 Startup order

1. `Application::builder().application_id(...)` (kept), `flags` as today.
2. `install` the `app.notice` action (section 5) before `run`.
3. `connect_command_line` stores requests in an `Rc<RefCell<VecDeque<LaunchRequest>>>` (main thread only).
4. `connect_activate` builds the window (`build_ui`), then drains the queue through `launch::apply`. A request with `ready == false` leaves the window on the default tab (the Kotlin `ready` flag, area:36).
5. `hold()` guards are created by the `HoldRegistry` on demand (section 2.4).

## 7. Android API to Linux mapping (this area)

| Android API or component | Used in | Linux replacement | Crate or facility | Task |
|---|---|---|---|---|
| `Activity`, `singleTop`, `singleTask`, `isTaskRoot` | MainActivity, reader, player, deep-link trampolines | `gtk::Application` single instance (GApplication) | gtk4 0.11 | T-53a-001 |
| `Intent` action/extras, `getIntExtra`, `getLongExtra` | everywhere | `LaunchRequest`, `NoticeId`, `AppAction` (typed) | serde_json, url | T-53a-017, T-53a-018 |
| `IntentFilter` VIEW scheme/host | manifest | `parse_uri` and `.desktop` `MimeType=x-scheme-handler/...` | url | T-53a-004 |
| `IntentFilter` `.tachibk` pathPattern | manifest | `application/x-tachibk` MIME + `connect_open` | shared-mime-info, gio | T-53a-003, T-53a-010 |
| `ShortcutManager` / `shortcuts.xml` | launcher long-press | `.desktop` `[Desktop Action]` | desktop entry spec | T-53a-011 |
| `searchable.xml` / `SEARCH` intents | global search from system | GNOME SearchProvider2 and KRunner (P2) | zbus | T-53a-066 |
| `Notification`, `NotificationChannel`, `NotificationManagerCompat` | everywhere | `gio::Notification`, categories (in-app switches) | gtk4/gio | T-53a-017, T-53a-021 |
| `PendingIntent.getBroadcast/getActivity` (request codes, FLAG_UPDATE_CURRENT) | notification buttons | `AppAction` in the GAction target; no identity issue | gio | T-53a-018 |
| `BroadcastReceiver` (manifest, runtime) | receiver, incognito | GAction `app.notice`; incognito `AppAction::DisableIncognito` | gio | T-53a-017, T-53a-022 |
| `NotificationManager.cancelAll()` | dismiss | not carried (withdraw by id only) | gio | T-53a-019 |
| `POST_NOTIFICATIONS` runtime permission | notify gate | notification server availability check | zbus | T-53a-023 |
| `WorkManager` `OneTimeWorkRequest`, `enqueueUniqueWork(REPLACE)` | app update | `run_unique(Replace)` | tokio, CancellationToken | T-53a-034 |
| `setForeground` / `ForegroundInfo` / `dataSync` | updater, servers | notice plus `BackgroundHold` | gio | T-53a-035, T-53a-037 |
| `Service` `START_STICKY` / `stopSelf` | torrent, HTTP server | `Service` objects with start and stop functions | tokio | T-53a-035 |
| `ConnectivityManager`, `NetworkCapabilities`, `WifiManager.isWifiEnabled` | downloads on Wi-Fi | `NetworkStatus` (gio + NetworkManager) | gio, zbus | T-53a-026 |
| `Environment.isExternalStorageManager`, `MANAGE_EXTERNAL_STORAGE` | settings | folder chooser through portal | gtk4 FileDialog, ashpd | T-53a-027 |
| `Intent.ACTION_VIEW` with `image/*`, `text/plain`, `http` | open image, error log, help links | `Opener::open_file`, `Opener::open_url` | gtk4 UriLauncher / FileLauncher | T-53a-039 |
| `Intent.ACTION_SEND` chooser (`toShareIntent`) | share image, backup | `Opener::reveal` and "Copy link" | gio, FileManager1 D-Bus | T-53a-040 |
| `FileProvider` (`content://`) | share and APK | none (paths) | none | T-53a-064 |
| `PackageInstaller`, `ACTION_INSTALL_PACKAGE`, `ACTION_UNINSTALL_PACKAGE` | extensions | none (area:20) | none | T-53a-036, T-53a-038 |
| `ShizukuProvider`, `rikka.shizuku` | extensions | none | none | T-53a-038 |
| `PowerManager.isIgnoringBatteryOptimizations` | settings | none | none | T-53a-028 |
| `PowerManager.WakeLock` (`WAKE_LOCK`) | none in code | none | none | T-53a-028 |
| Samsung `REMOTE_ACTION` / `s_pen_actions` | reader, player | keyboard map (area:43, area:46) | gtk4 EventControllerKey | T-53a-065 |
| Picture-in-picture (`supportsPictureInPicture`) | player | compact mode (area:46) | gtk4 | T-53a-033 |
| Glance `AppWidget` receivers | widgets | none (dropped) | none | T-53a-068 |
| `network_security_config` (cleartext, user CAs) | HTTP client | reqwest rustls config | reqwest, rustls-native-certs, webpki-roots | T-53a-063 |
| `Theme.NoDisplay` (trampolines) | deep links | none (no window) | none | T-53a-046 |
| `AppCompat per-app locales` (`READ_APP_SPECIFIC_LOCALES`) | language | desktop locale and in-app language (area:51) | gettext/Fluent per area:51 | T-53a-031 |
| `Migrator` wait (splash) | MainActivity start | loading state of the library page | none | T-53a-069 |

## 8. Error handling

- Each module has one `thiserror` enum: `LaunchError` (section 2.1), `PayloadError` (section 2.2), `NotifyError { Unavailable, Rejected(String) }`, `OpenError { NoHandler, Io(std::io::Error), Portal(String) }`, `SingleInstanceError { Bus(String) }`, and `ResolveError` (defined by area:36, section 3 of its deep-link topic; this area adds the `Failed { reason }` case used by the anime and manga resolvers).
- Errors reach the user only through one channel: `toast::show(window, text)` for foreground actions and `Notice` for background ones. Silent failure is not allowed (edge E-07 and E-16).
- `anyhow` is not used in library crates; the binary may use it at the top level.
- Logging: `tracing` with the target `kurobayomi::notice`, `kurobayomi::launch`, `kuro_platform::linux::network`. Verbose logging maps to the `RUST_LOG` filter and the area:02 preference `verboseLogging`.

## 9. Testing hooks (kuro-testkit)

- `RecordingNotifier` (implements `Notifier`, records posts and withdrawals in a `Vec`, used by the notice dispatch tests).
- `FixedNetwork(NetworkState)` (implements `NetworkStatus`, used by download-gating tests).
- `Url` fixtures: the URI and argv samples in `p1-edge-cases-and-tests.md` section 4.
- Golden files: the `.desktop` text (section 2.6) and the MIME XML (section 2.6) are compared byte-for-byte in a test, so packaging cannot drift.

## 10. Pitfalls

1. `DBusActivatable=true` plus a `.desktop` file whose `Exec` matches the installed binary name is required for a notification click to start a closed app. Without it, the click does nothing when no window exists.
2. GIO notifications take the application id from the running process. KDE matches the notice to the desktop file by that id. If the id and the `.desktop` basename differ, the notice shows a generic icon.
3. `withdraw_notification` needs the same id string used at post time. Ids are never derived from `Hash` of a changing value; they come from `NoticeId` constructors only.
4. GIO has no progress bar and no "ongoing" flag. Do not reproduce the Android ongoing notice for downloads; use the download screen.
5. A GAction `app.notice` target is a string GVariant. Payloads are JSON strings; the maximum length is enforced before parsing (64 KiB, section 2.2).
6. `gtk::UriLauncher` and `gtk::FileLauncher` need a parent window for the portal dialog. Pass the main window, never `None`, or the portal may ignore the request inside Flatpak.
7. `NetworkManager` property names are case-sensitive (`PrimaryConnectionType`, `WirelessEnabled`, `Metered`). A wrong name returns `org.freedesktop.DBus.Error.UnknownProperty`; treat that as "unknown", not as "false".
8. `ApplicationHoldGuard` is tied to the main thread. Acquiring from tokio tasks panics in debug builds; use the channel to the main thread (section 5).
9. `connect_command_line` runs in the primary instance only when another process forwards args. The first process runs `activate` and `command-line` both; the queue must not double-apply a request.
10. Freedesktop notification servers ignore more than a few buttons; Proposal caps buttons at three.
