# Area 35 - Rust design (notifications, app updater, library CSV export, misc)

Design rules applied (from `addenda/requirements-cross-platform.md`): all OS access goes through `kuro-platform` traits; no `cfg(target_os)`, `zbus`, `xdg` or `xdg-open` outside `kuro-platform`; directories come from `Paths`; file names pass the portable sanitiser; dependencies are injected through the app context, not globals.

Current state of the mockup (OBSERVED): `/home/blackbox/kurobayomi/Cargo.toml` is a single crate `kurobayomi` (edition 2024) with `gtk = { package = "gtk4", version = "0.11.5", features = ["v4_16"] }`, `serde`, `serde_json`. `src/main.rs:12` `main()` returns `glib::ExitCode`; `src/main.rs:22` `build_ui(app: &gtk::Application)`; `src/settings.rs:7` `body(id)` dispatches spec ids to builders; `src/spec.rs` loads `spec/*.json`. No notification, update or export code exists. The git repository has no remote.

---

## 1. Crate and module layout

```
Cargo.toml                       [workspace] members = [".", "crates/kuro-platform"]   (T-req-platform-001)
crates/kuro-platform/
  src/lib.rs                     pub use traits; cfg(feature = "platform-linux") selects linux
  src/notifier.rs                trait Notifier, Notice, NoticeId, Category, Priority, NoticeAction, NotifyError
  src/opener.rs                  trait Opener, OpenError
  src/paths.rs                   trait Paths
  src/network.rs                 trait NetworkStatus
  src/icons.rs                   bundled symbolic icon names (T-35a-087)
  src/linux/mod.rs               impls for Linux
  src/linux/notifier.rs          GioNotifier (gio::Notification on gtk::Application)
  src/linux/opener.rs            GioOpener (AppInfo / ShowItems over zbus)
  src/linux/paths.rs             LinuxPaths (directories crate)
  src/linux/network.rs           GioNetworkStatus (gio::NetworkMonitor)
src/notify/
  mod.rs                         install(app, services) wiring
  actions.rs                     AppAction enum + JSON codec (GVariant string)
  router.rs                      GAction "notice" -> dispatch
  registry.rs                    NoticeId constants, Category mapping, id table (section 3)
  throttle.rs                    ProgressThrottle
  mute.rs                        per-category enable flags (prefs)
src/update/
  mod.rs                         UpdateService (public API used by About and startup)
  model.rs                       Release, UpdateCheck, UpdateError
  version.rs                     parse_tag, is_newer
  asset.rs                       pick_asset
  notes.rs                       mention rewrite, checksum section strip, sha256 extraction
  check.rs                       may_check, fetch_latest, check()
  download.rs                    Downloader (single slot), DownloadRequest
  notify.rs                      builds the four update Notices
src/export/
  mod.rs
  library_csv.rs                 EntryKind, ExportEntry, ExportOptions, render(), write_csv()
```

`kuro-platform` is a separate crate so the Windows port (T-req-platform-011) implements only the traits. The `src/notify`, `src/update` and `src/export` modules are application code and must not name `gio`, `zbus` or `xdg` directly except through `kuro-platform`. The one exception allowed by this plan is `gtk::FileDialog` in the export dialog, because GTK's file dialog is the portal-aware GTK API (it is not an OS call written by us). PROPOSAL: move the dialog wrapper into `kuro-platform::FileChooser` if the Windows port needs it (T-35a-073 covers the dialog).

Cargo features:
- `kuro-platform`: `platform-linux` (default), `platform-windows`, `platform-macos` (addendum rule 2).
- `kurobayomi`: `updater` (default off, mirrors Aniyomi's `enable-updater` property; section 2.1 of the behaviour spec).

---

## 2. Platform traits (kuro-platform)

### 2.1 Notifier

```rust
// crates/kuro-platform/src/notifier.rs
use std::path::PathBuf;

/// Every notification the app can post. Each category has a user mute switch (see mute.rs).
/// Replaces Android NotificationChannel (Notifications.kt:22-194).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Common,            // channel common_channel
    SaveImage,         // ID_DOWNLOAD_IMAGE notice; channel common_channel
    LibraryProgress,   // library_progress_channel
    LibraryErrors,     // library_errors_channel
    NewChapters,       // new_chapters_episodes_channel
    DownloadProgress,  // downloader_progress_channel
    DownloadErrors,    // downloader_error_channel
    BackupProgress,    // backup_restore_progress_channel
    BackupComplete,    // backup_restore_complete_channel_v2 (HIGH, silent)
    Incognito,         // incognito_mode_channel
    ServerStatus,      // torrent_server_channel + http_server_channel
    AppUpdate,         // app_apk_update_channel
    ExtensionUpdates,  // ext_apk_update_channel
    Actions,           // completion notices with buttons that have no own category (see registry)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority { Low, Normal, High, Urgent }

/// Stable slot id. Posting a notice with an existing id replaces it. Withdrawing removes it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoticeId(pub String);

/// A button or the default action. `target` is the serialised app action (JSON, see src/notify/actions.rs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeAction {
    pub label: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub id: NoticeId,
    pub category: Category,
    pub title: String,
    pub body: String,
    pub priority: Priority,
    /// Symbolic icon name bundled by kuro-platform (never a theme-dependent name).
    pub icon: &'static str,
    pub default_action: Option<NoticeAction>,
    /// At most 3 buttons (PROPOSAL: freedesktop servers commonly show up to 3 actions).
    pub buttons: Vec<NoticeAction>,
    /// True when the body may reveal library content; hidden when hide_notification_content is on.
    pub private_content: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum NotifyError {
    #[error("no notification service is available")]
    Unavailable,
    #[error("notice has more than 3 buttons")]
    TooManyButtons,
}

pub trait Notifier: Send + Sync {
    /// Posts or replaces the notice. Returns Ok(false) when nothing was shown because the
    /// category is muted or the body is private and content is hidden. Callable from any thread.
    fn show(&self, notice: Notice) -> Result<bool, NotifyError>;
    /// Removes exactly this slot. Never removes other notices (no cancel-all; see behaviour 1.5(a)).
    fn withdraw(&self, id: &NoticeId);
    fn is_enabled(&self, category: Category) -> bool;
}
```

### 2.2 Opener

```rust
// crates/kuro-platform/src/opener.rs
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("no handler for {0}")]
    NoHandler(String),
    #[error("portal or D-Bus call failed: {0}")]
    Bus(String),
}

pub trait Opener: Send + Sync {
    /// Opens an https URL in the default browser. Only https and http are accepted.
    fn open_url(&self, url: &str) -> Result<(), OpenError>;
    /// Opens a file with its default application.
    fn open_file(&self, path: &Path) -> Result<(), OpenError>;
    /// Shows the file in the file manager with the file selected; falls back to open_file(parent).
    fn reveal_file(&self, path: &Path) -> Result<(), OpenError>;
}
```

### 2.3 Paths and NetworkStatus

```rust
// crates/kuro-platform/src/paths.rs
use std::path::PathBuf;

pub trait Paths: Send + Sync {
    fn data_dir(&self) -> PathBuf;
    fn config_dir(&self) -> PathBuf;
    fn cache_dir(&self) -> PathBuf;
    fn downloads_default(&self) -> PathBuf;
    /// Default: cache_dir()/updates. Used by the app updater only.
    fn update_dir(&self) -> PathBuf { self.cache_dir().join("updates") }
}

// crates/kuro-platform/src/network.rs
pub trait NetworkStatus: Send + Sync {
    fn is_online(&self) -> bool;
    fn is_metered(&self) -> bool;
    /// Emits `true` when the network becomes available again.
    fn online_events(&self) -> async_channel::Receiver<bool>;
}

/// Architecture token used to choose release assets (`x86_64`, `aarch64`, other).
pub fn platform_arch() -> &'static str {
    std::env::consts::ARCH
}
```

---

## 3. Application types

### 3.1 AppAction (src/notify/actions.rs)

This is the replacement for the 22 string actions and 6 extras (behaviour spec 1.8, data 3.3). Serialised as JSON (`serde_json`) into the GVariant string parameter of the `app.notice` action.

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use kuro_platform::NoticeId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppAction {
    /// Constants.SHORTCUT_DOWNLOADS -> HomeScreen.Tab.More(toDownloads = true)
    OpenMangaDownloads,
    /// Constants.SHORTCUT_ANIME_DOWNLOADS -> HomeScreen.Tab.More(toDownloads = true)
    OpenAnimeDownloads,
    /// ReaderActivity.newIntent(manga, chapter)
    OpenChapter { manga_id: i64, chapter_id: i64 },
    /// PlayerActivity.newIntent(anime, episode)
    OpenEpisode { anime_id: i64, episode_id: i64 },
    /// SHORTCUT_MANGA -> Library tab with entry, dismisses `notice` first
    OpenManga { manga_id: i64, notice: Option<NoticeId> },
    /// SHORTCUT_ANIME -> AnimeLib tab with entry
    OpenAnime { anime_id: i64, notice: Option<NoticeId> },
    /// ACTION_MARK_AS_READ
    MarkChaptersRead { manga_id: i64, urls: Vec<String>, notice: Option<NoticeId> },
    /// ACTION_MARK_AS_SEEN
    MarkEpisodesSeen { anime_id: i64, urls: Vec<String>, notice: Option<NoticeId> },
    /// ACTION_DOWNLOAD_CHAPTER
    DownloadChapters { manga_id: i64, urls: Vec<String>, notice: Option<NoticeId> },
    /// ACTION_DOWNLOAD_EPISODE
    DownloadEpisodes { anime_id: i64, urls: Vec<String>, notice: Option<NoticeId> },
    ResumeMangaDownloads,
    PauseMangaDownloads,
    ClearMangaDownloads,
    ResumeAnimeDownloads,
    PauseAnimeDownloads,
    ClearAnimeDownloads,
    /// ACTION_CANCEL_LIBRARY_UPDATE
    CancelLibraryUpdate,
    /// ACTION_CANCEL_ANIMELIB_UPDATE
    CancelAnimeLibraryUpdate,
    /// ACTION_CANCEL_RESTORE
    CancelRestore,
    /// ACTION_START_APP_UPDATE; url must be https and checked against the release host (section 6)
    StartAppUpdate { url: String, version: Option<String> },
    /// ACTION_CANCEL_APP_UPDATE_DOWNLOAD
    CancelAppUpdate,
    /// Replaces ACTION_SHARE_IMAGE, ACTION_SHARE_BACKUP and the error-log intent: reveal in file manager
    RevealFile { path: PathBuf },
    /// Replaces openUrl and the "What's new" release intent
    OpenUrl { url: String },
    /// ACTION_DISMISS_NOTIFICATION: withdraws this slot only
    Dismiss { notice: NoticeId },
}

pub const NOTICE_ACTION_NAME: &str = "notice"; // registered as "app.notice"
pub const MAX_PAYLOAD_BYTES: usize = 1 << 20;   // 1 MiB, PROPOSAL; batches above this are rejected

impl AppAction {
    pub fn to_target(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn from_target(s: &str) -> Result<Self, ActionError> {
        if s.len() > MAX_PAYLOAD_BYTES {
            return Err(ActionError::TooLarge(s.len()));
        }
        Ok(serde_json::from_str(s)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error("payload too large: {0} bytes")]
    TooLarge(usize),
    #[error("invalid action payload: {0}")]
    Parse(#[from] serde_json::Error),
}
```

Note on the removed Android actions: `ACTION_SHARE_IMAGE`, `ACTION_SHARE_BACKUP`, `ACTION_OPEN_ENTRY` (unused) and the share/APK intents are not variants (T-35a-023, T-35a-024, T-35a-025, T-35a-056).

### 3.2 Notice ids (src/notify/registry.rs)

| Android id (int) | Rust NoticeId | Category | Replaces |
|---|---|---|---|
| 1 `ID_APP_UPDATER` | `app-update.progress` | AppUpdate | download progress, and prompt id 1 |
| 2 `ID_APP_UPDATE_PROMPT` | `app-update.install` | AppUpdate | install prompt |
| 3 `ID_APP_UPDATE_ERROR` | `app-update.error` | AppUpdate | error |
| 2 `ID_DOWNLOAD_IMAGE` | `save-image` | SaveImage | collision removed (behaviour 1.4) |
| -101 `ID_LIBRARY_PROGRESS` | `library.progress` | LibraryProgress | area:26-library-update-jobs |
| -102 `ID_LIBRARY_ERROR` | `library.error` | LibraryErrors | area:26-library-update-jobs |
| -103 `ID_LIBRARY_SIZE_WARNING` | `library.size-warning` | LibraryErrors | area:26-library-update-jobs |
| -201 `ID_DOWNLOAD_CHAPTER_PROGRESS` | `download.manga.progress` | DownloadProgress | area:23-downloads-manga |
| -202 `ID_DOWNLOAD_CHAPTER_ERROR` | `download.manga.error` | DownloadErrors | area:23-downloads-manga |
| -203 `ID_DOWNLOAD_EPISODE_PROGRESS` | `download.anime.progress` | DownloadProgress | area:24-downloads-anime |
| -204 `ID_DOWNLOAD_EPISODE_ERROR` | `download.anime.error` | DownloadErrors | area:24-downloads-anime |
| -301 `ID_NEW_CHAPTERS` | `new.chapters` | NewChapters | area:26-library-update-jobs |
| -1301 `ID_NEW_EPISODES` | `new.episodes` | NewChapters | area:26-library-update-jobs |
| -501 `ID_BACKUP_PROGRESS` | `backup.progress` | BackupProgress | area:21-backup-format-and-create |
| -502 `ID_BACKUP_COMPLETE` | `backup.complete` | BackupComplete | area:21-backup-format-and-create |
| -503 `ID_RESTORE_PROGRESS` | `restore.progress` | BackupProgress | area:22-backup-restore |
| -504 `ID_RESTORE_COMPLETE` | `restore.complete` | BackupComplete | area:22-backup-restore |
| -701 `ID_INCOGNITO_MODE` | `incognito` | Incognito | area:01-foundation-app-bootstrap |
| -801 `ID_TORRENT_SERVER` | `server.torrent` | ServerStatus | area:13-core-util-storage-archive-torrent |
| -901 `ID_HTTP_SERVER` | `server.http` | ServerStatus | area:53-platform-manifest-intents-workers |
| -401 `ID_UPDATES_TO_EXTS` | `extensions.updates` | ExtensionUpdates | area:18-extensions-manga/19 |
| -402 `ID_EXTENSION_INSTALLER` | `extensions.installing` | ExtensionUpdates | area:18-extensions-manga/19 |

```rust
// src/notify/registry.rs
use kuro_platform::{Category, NoticeId};

pub fn app_update_progress() -> NoticeId { NoticeId("app-update.progress".into()) }
pub fn app_update_install() -> NoticeId { NoticeId("app-update.install".into()) }
pub fn app_update_error() -> NoticeId { NoticeId("app-update.error".into()) }
pub fn save_image() -> NoticeId { NoticeId("save-image".into()) }

/// Compile-time table used by tests to catch accidental duplicates.
pub const ALL_IDS: &[&str] = &[
    "app-update.progress", "app-update.install", "app-update.error", "save-image",
    "library.progress", "library.error", "library.size-warning",
    "download.manga.progress", "download.manga.error", "download.anime.progress", "download.anime.error",
    "new.chapters", "new.episodes",
    "backup.progress", "backup.complete", "restore.progress", "restore.complete",
    "incognito", "server.torrent", "server.http",
    "extensions.updates", "extensions.installing",
];
```

### 3.3 Mute and privacy (src/notify/mute.rs)

- Preference keys: `notify.enabled.<category>` (bool, default true), read through the area:02-preferences-system preference store. `Category::AppUpdate` is controlled by `update.auto_check` and the updater feature, not by mute (keeps the manual flow working).
- `Notifier::show` returns `Ok(false)` without posting when `is_enabled(category)` is false, or when `notice.private_content && config.notify.hide_content`.

### 3.4 Progress throttle (src/notify/throttle.rs)

```rust
use std::time::{Duration, Instant};

/// Android rule (AppUpdateDownloadJob.kt:89-97): emit when the integer percent is strictly greater
/// than the last emitted percent AND strictly more than 200 ms passed since the last emit.
pub struct ProgressThrottle {
    last_pct: u8,
    last_emit: Option<Instant>,
    min_gap: Duration,
}

impl ProgressThrottle {
    pub fn new() -> Self {
        Self { last_pct: 0, last_emit: None, min_gap: Duration::from_millis(200) }
    }

    /// `total` None or 0 means unknown length: no progress is ever emitted (Android behaviour for
    /// contentLength <= 0). Percent truncates toward zero like Kotlin's `toInt()`.
    pub fn offer(&mut self, now: Instant, bytes: u64, total: Option<u64>) -> Option<u8> {
        let total = total.filter(|t| *t > 0)?;
        let pct = ((100.0 * bytes as f64) / total as f64).clamp(0.0, 100.0) as u8;
        if pct <= self.last_pct {
            return None;
        }
        if let Some(prev) = self.last_emit {
            if now.duration_since(prev) <= self.min_gap {
                return None;
            }
        }
        self.last_pct = pct;
        self.last_emit = Some(now);
        Some(pct)
    }
}
```
Note: Kotlin computes with `Float` (f32). `f64` can differ from f32 only at the rounding boundary of a percent. Accepted difference, recorded in the edge file.

---

## 4. Linux implementation of Notifier (kuro-platform/src/linux/notifier.rs)

Chosen mechanism: `gio::Notification` sent through `gtk::Application::send_notification(Some(id), &notification)` and removed with `withdraw_notification(id)`. Reasons: (1) it is part of GIO which GTK already links; (2) it routes activations to GAction names, which replaces PendingIntent; (3) inside a Flatpak it uses `org.freedesktop.portal.Notification` automatically; outside it uses the freedesktop notification D-Bus service through GLib's backend (PROPOSAL assumption; verify on KDE Plasma with `gdbus call` during T-35a-002).

Limits that shape the design (OBSERVED from GIO API documentation, not from our code):
- No progress bar in GNotification. Progress is text ("Downloading 42%") updated under the throttle (section 3.4).
- Notifications require the app to be running.
- Button labels and count depend on the server.

```rust
// crates/kuro-platform/src/linux/notifier.rs
use gtk::{gio, glib, prelude::*};
use crate::notifier::{Category, Notice, NoticeId, Notifier, NotifyError, Priority};

pub struct GioNotifier {
    app: glib::WeakRef<gtk::Application>,
    mute: std::sync::Arc<dyn Fn(Category) -> bool + Send + Sync>, // true = enabled (reads prefs)
    hide_content: std::sync::Arc<dyn Fn() -> bool + Send + Sync>,
}

impl Notifier for GioNotifier {
    fn show(&self, notice: Notice) -> Result<bool, NotifyError> {
        if notice.buttons.len() > 3 {
            return Err(NotifyError::TooManyButtons);
        }
        if !self.is_enabled(notice.category) {
            return Ok(false);
        }
        if notice.private_content && (self.hide_content)() {
            return Ok(false);
        }
        if self.app.upgrade().is_none() {
            return Err(NotifyError::Unavailable);
        }
        // gio::Notification and gtk::Application are not Send; only the WeakRef (Send) and the
        // owned Notice move into the closure, and the upgrade happens on the main context.
        let app = self.app.clone();
        glib::MainContext::default().invoke(move || {
            let Some(app) = app.upgrade() else { return };
            let n = gio::Notification::new(&notice.title);
            n.set_body(Some(&notice.body));
            n.set_priority(match notice.priority {
                Priority::Low => gio::NotificationPriority::Low,
                Priority::Normal => gio::NotificationPriority::Normal,
                Priority::High => gio::NotificationPriority::High,
                Priority::Urgent => gio::NotificationPriority::Urgent,
            });
            n.set_icon(&gio::ThemedIcon::new(notice.icon));
            if let Some(a) = &notice.default_action {
                let v = glib::Variant::from(a.target.as_str());
                n.set_default_action_and_target_value("app.notice", Some(&v));
            }
            for b in &notice.buttons {
                let v = glib::Variant::from(b.target.as_str());
                n.add_button_with_target_value(&b.label, "app.notice", Some(&v));
            }
            app.send_notification(Some(&notice.id.0), &n);
        });
        Ok(true)
    }

    fn withdraw(&self, id: &NoticeId) {
        let app = self.app.clone();
        let id = id.0.clone();
        glib::MainContext::default().invoke(move || {
            if let Some(app) = app.upgrade() {
                app.withdraw_notification(&id);
            }
        });
    }

    fn is_enabled(&self, category: Category) -> bool {
        (self.mute)(category)
    }
}
```
Pitfalls encoded here:
- `glib::Variant::from(&str)` creates type `s`; the GAction must be created with parameter type `s` (see router, section 6).
- `send_notification` with an id replaces the previous notification with that id (the progress/stale defect in behaviour 2.9 is avoided by withdrawing explicitly).
- `set_icon` uses a `ThemedIcon` name; the name must be one of the bundled symbolic icons (addenda rule 6). Missing names fall back to the default icon.
- `glib::MainContext::default().invoke` requires the closure to be `Send`. `Notice` is `Send` (owned `String`s and `Vec`), and `glib::WeakRef` is `Send`, so both move into the closure; the strong `gtk::Application` exists only on the main context after `upgrade()`.

`AppAction::Dismiss` calls `withdraw(id)` only; it never touches other slots.

---

## 5. Linux implementation of Opener, Paths and NetworkStatus

```rust
// crates/kuro-platform/src/linux/opener.rs
use std::path::Path;
use gtk::{gio, prelude::*};
use crate::opener::{OpenError, Opener};

pub struct GioOpener;

impl Opener for GioOpener {
    fn open_url(&self, url: &str) -> Result<(), OpenError> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(OpenError::NoHandler(url.to_owned()));
        }
        gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>)
            .map_err(|e| OpenError::Bus(e.to_string()))
    }
    fn open_file(&self, path: &Path) -> Result<(), OpenError> {
        let uri = gio::File::for_path(path).uri();
        gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>)
            .map_err(|e| OpenError::Bus(e.to_string()))
    }
    fn reveal_file(&self, path: &Path) -> Result<(), OpenError> {
        // Preferred: org.freedesktop.FileManager1.ShowItems over zbus (session bus).
        // Fallback: open the parent directory.
        // Implementation detail is owned by kuro-platform; the trait contract is above.
        match reveal_via_file_manager_dbus(path) {
            Ok(()) => Ok(()),
            Err(_) => match path.parent() {
                Some(dir) => self.open_file(dir),
                None => Err(OpenError::NoHandler(path.display().to_string())),
            },
        }
    }
}

/// org.freedesktop.FileManager1.ShowItems(as URIs, s StartupId). Blocking zbus call on the session bus.
fn reveal_via_file_manager_dbus(path: &Path) -> Result<(), OpenError> {
    let uri = gtk::gio::File::for_path(path).uri();
    let conn = zbus::blocking::Connection::session().map_err(|e| OpenError::Bus(e.to_string()))?;
    conn.call_method(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        Some("org.freedesktop.FileManager1"),
        "ShowItems",
        &(vec![uri.as_str()], ""),
    )
    .map(|_| ())
    .map_err(|e| OpenError::Bus(e.to_string()))
}
```
The `reveal_via_file_manager_dbus` stub is intentionally marked; the real body is the T-35a-010 deliverable. (It returns an error so the fallback path is what runs until that task lands.)

```rust
// crates/kuro-platform/src/linux/paths.rs
use std::path::PathBuf;
use crate::paths::Paths;

pub struct LinuxPaths { dirs: directories::ProjectDirs }

impl LinuxPaths {
    pub fn new() -> Option<Self> {
        directories::ProjectDirs::from("", "", "kurobayomi").map(|dirs| Self { dirs })
    }
}

impl Paths for LinuxPaths {
    fn data_dir(&self) -> PathBuf { self.dirs.data_dir().to_path_buf() }
    fn config_dir(&self) -> PathBuf { self.dirs.config_dir().to_path_buf() }
    fn cache_dir(&self) -> PathBuf { self.dirs.cache_dir().to_path_buf() }
    fn downloads_default(&self) -> PathBuf {
        directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| self.data_dir().join("downloads"))
    }
}
```
`ProjectDirs::from("", "", "kurobayomi")` gives `~/.local/share/kurobayomi`, `~/.config/kurobayomi`, `~/.cache/kurobayomi` (XDG). The qualifier and organisation are empty so the names do not depend on a reverse-DNS id (PROPOSAL).

```rust
// crates/kuro-platform/src/linux/network.rs
use gtk::{gio, prelude::*};
use crate::network::NetworkStatus;

pub struct GioNetworkStatus { monitor: gio::NetworkMonitor }

impl GioNetworkStatus {
    pub fn new() -> Self { Self { monitor: gio::NetworkMonitor::default() } }
}

impl NetworkStatus for GioNetworkStatus {
    fn is_online(&self) -> bool { self.monitor.is_network_available() }
    fn is_metered(&self) -> bool { self.monitor.is_network_metered() }
    fn online_events(&self) -> async_channel::Receiver<bool> {
        let (tx, rx) = async_channel::unbounded();
        self.monitor.connect_network_changed(move |_, available| {
            let _ = tx.try_send(available);
        });
        rx
    }
}
```
Note: `connect_network_changed` must be connected on the main thread; `online_events` is called once at startup on the main thread (PROPOSAL rule).

---

## 6. Action router (src/notify/router.rs)

```rust
use gtk::{gio, glib, prelude::*};
use std::rc::Rc;

pub struct Router { /* Rc'd services: db handle, download managers, window, toaster */ }

impl Router {
    pub fn install(app: &gtk::Application, router: Rc<Router>) {
        let action = gio::SimpleAction::new(super::actions::NOTICE_ACTION_NAME, Some(glib::VariantTy::STRING));
        action.connect_activate(move |_, param| {
            let text = param.and_then(|v| v.str()).unwrap_or_default();
            match super::actions::AppAction::from_target(text) {
                Ok(action) => router.dispatch(action),
                Err(e) => log::warn!("rejected notice action: {e}"),
            }
        });
        app.add_action(&action);
    }

    pub fn dispatch(&self, action: super::actions::AppAction) {
        // Each arm: validate ids, then spawn work (section 7). Never block the main loop.
        let _ = action;
    }
}
```

Dispatch table (AppAction -> behaviour; sources in behaviour spec):
- `OpenMangaDownloads` / `OpenAnimeDownloads`: switch the main window to More with downloads visible (behaviour 1.7).
- `OpenChapter`: load manga and chapter by id on the tokio side; if either is missing, toast `notify.error.item_missing` ("Error"); else open the reader (area:43-reader-core). Replaces `runBlocking` on the UI thread (behaviour 1.9.4).
- `OpenEpisode`: same with the player (area:45-player-core).
- `OpenManga` / `OpenAnime`: if `notice` is Some, `withdraw`; then navigate to the entry (area:37-ui-library/38/39). Missing id => no navigation (Android: id 0 quirk is not reproduced).
- `MarkChaptersRead` / `MarkEpisodesSeen`: withdraw `notice` first (behaviour 1.9.2), then on the tokio side: lookup each URL (area:05-data-repos-entries-items), optional delete when `removeAfterMarkedAsRead` (area:23-downloads-manga/24), then partial update `read = true` (or `seen = true`) (area:08-domain-library-category-history-updates interactor). Errors are logged and a toast shows `Could not update` only for DB errors (PROPOSAL; Android swallows).
- `DownloadChapters` / `DownloadEpisodes`: withdraw, lookup, enqueue with area:23-downloads-manga/24 managers. No toast on empty input (parity).
- `Resume/Pause/Clear*Downloads`: call the manager method (area:23-downloads-manga/24/25).
- `CancelLibraryUpdate` / `CancelAnimeLibraryUpdate` / `CancelRestore`: call the job's cancel (area:26-library-update-jobs / area:22-backup-restore).
- `StartAppUpdate { url, version }`: validate (section 6.1) then `Downloader::start`.
- `CancelAppUpdate`: `Downloader::stop`.
- `RevealFile { path }`: `Opener::reveal_file(path)`; error => toast `notify.error.open_failed`.
- `OpenUrl { url }`: `Opener::open_url(url)` after the URL check.
- `Dismiss { notice }`: `Notifier::withdraw(&notice)`.

### 6.1 Validation rules for untrusted payloads

The payload travels through the session bus. Another process of the same user could invoke `app.notice`, so every field is validated before use:
- Payload length <= `MAX_PAYLOAD_BYTES` (1 MiB).
- `StartAppUpdate.url`: must parse as `https`; host must equal `github.com` or `objects.githubusercontent.com` (GitHub's release asset host); no userinfo; port 443 (PROPOSAL, mirrors the asset host Android downloads from).
- `RevealFile.path`: must be inside `Paths::data_dir()`, `downloads` root, or `cache_dir()/updates` (PROPOSAL; prevents reveal of arbitrary paths).
- `OpenUrl.url`: https or http only (`GioOpener` enforces it).
- Chapter and episode URL arrays: each at most 2048 bytes; array length at most 10 000 (PROPOSAL; Android has no cap, but a bound protects the DB query).
- Ids: `i64`; ids <= 0 are rejected (Android treats -1 as "missing").

---

## 7. Threading model

| Thread | Owns | Work |
|---|---|---|
| GTK main thread (glib main context) | `gtk::Application`, windows, `GioNotifier` internals, `Router` | action dispatch, UI updates, notification send/withdraw (marshalled) |
| tokio multi-thread runtime (`Builder::new_multi_thread().enable_all()`), handle stored in the app context | `Downloader`, `UpdateService`, DB calls via `spawn_blocking` (rusqlite) | network, file I/O, DB reads/writes |
| glib <-> tokio bridge | `async_channel` | results returned to the UI with `glib::spawn_future_local(async move { let r = rx.recv().await; ... })` |

Rules:
- No `block_on` on the main thread (Android `runBlocking` in `openChapter`/`openEpisode` is NOT ported).
- Services never touch GTK widgets; they send `UiEvent` values (`Toast(String)`, `Navigate(...)`, `Notice(Notice)`) over an `async_channel` that the main thread drains.
- `Notifier::show` and `withdraw` may be called from tokio; they marshal to the main context (section 4).
- Cancellation uses `tokio_util::sync::CancellationToken` (PROPOSAL dependency `tokio-util`).

Single-flight guards:
- `UpdateService::check` holds `tokio::sync::Mutex<()>` for the duration of a check (behaviour E-U10).
- `Downloader` holds one slot (`Mutex<Option<DownloadTask>>`). `start` cancels the old task, awaits its `JoinHandle`, then spawns the new one (REPLACE semantics).

---

## 8. Update service (src/update)

### 8.1 Types and API

```rust
// src/update/model.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,         // tag_name verbatim (e.g. "v0.2.0")
    pub notes_markdown: String,  // body after @mention rewrite (checksum section kept)
    pub release_url: String,     // html_url
    pub download_url: String,    // chosen asset browser_download_url
    pub asset_name: String,      // chosen asset name
    pub sha256: Option<String>,  // from notes, lowercase hex, 64 chars
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheck {
    NewUpdate(Release),
    NoNewUpdate,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("update checks are disabled (no update.repo configured)")]
    Disabled,
    #[error("no releases published yet")]
    NoReleases,
    #[error("GitHub rate limit reached, try again later")]
    RateLimited,
    #[error("GitHub returned HTTP {0}")]
    Http(u16),
    #[error("network error: {0}")]
    Network(String),
    #[error("cannot parse version '{0}'")]
    BadVersion(String),
    #[error("release has no download for this platform")]
    NoAsset,
    #[error("checksum mismatch for {0}")]
    Checksum(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
```

Mapping: `NoReleases` = HTTP 404; `RateLimited` = HTTP 403 or 429 (PROPOSAL; Android shows the raw message); `Http(code)` = others; `Network` = transport errors.

```rust
// src/update/mod.rs
pub struct UpdateService {
    cfg: UpdateConfig,
    http: reqwest::Client,
    store: std::sync::Arc<dyn AppStateStore>,       // last_update_check_ms
    checking: tokio::sync::Mutex<()>,               // single-flight
    notifier: std::sync::Arc<dyn kuro_platform::Notifier>,
    downloader: std::sync::Arc<download::Downloader>,
    current_version: &'static str,                  // env!("CARGO_PKG_VERSION")
}

pub trait AppStateStore: Send + Sync {
    fn last_update_check_ms(&self) -> i64;          // 0 = never
    fn set_last_update_check_ms(&self, ms: i64);
}

impl UpdateService {
    /// Automatic (startup) check: force = false. Manual (About): force = true.
    pub async fn check(&self, force: bool) -> Result<UpdateCheck, UpdateError> {
        let _guard = self.checking.lock().await;
        let repo = self.cfg.repo.as_deref().ok_or(UpdateError::Disabled)?;
        let now = now_unix_ms();
        if !check::may_check(now, self.store.last_update_check_ms(), force) {
            return Ok(UpdateCheck::NoNewUpdate);
        }
        let Some(release) = check::fetch_latest(&self.http, repo, platform_arch()).await? else {
            return Ok(UpdateCheck::NoNewUpdate);    // timestamp NOT set (parity, behaviour 2.3)
        };
        self.store.set_last_update_check_ms(now);   // set for every non-null release
        if version::is_newer(self.current_version, &release.version)? {
            notify::post_prompt(&*self.notifier, &release);   // behaviour 2.6
            Ok(UpdateCheck::NewUpdate(release))
        } else {
            Ok(UpdateCheck::NoNewUpdate)
        }
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
```

Note the `check()` sequence matches behaviour 2.3 and 2.6: the prompt notice is posted inside the service, and the caller (startup or About) decides whether to show the screen (behaviour 2.2).

### 8.2 Version rule (src/update/version.rs)

```rust
pub fn parse_tag(tag: &str) -> Result<semver::Version, UpdateError> {
    let t = tag.trim();
    let t = t.strip_prefix('v').unwrap_or(t);
    semver::Version::parse(t).map_err(|_| UpdateError::BadVersion(tag.to_owned()))
}

/// Strict semver ordering. Fixes behaviour defects E1-E6 (2.4).
pub fn is_newer(current: &str, tag: &str) -> Result<bool, UpdateError> {
    let cur = semver::Version::parse(current).map_err(|_| UpdateError::BadVersion(current.to_owned()))?;
    Ok(parse_tag(tag)? > cur)
}
```
`semver` 1.x: prerelease tags compare lower than the release (`1.0.0-rc.1 < 1.0.0`), which is the correct behaviour (the Kotlin one is wrong, E6).

### 8.3 Release fetch (src/update/check.rs)

```rust
pub async fn fetch_latest(
    http: &reqwest::Client,
    repo: &str,
    arch: &str,
) -> Result<Option<Release>, UpdateError> {
    // repo must be "owner/name": two non-empty segments of [A-Za-z0-9._-]
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = http.get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send().await.map_err(|e| UpdateError::Network(e.to_string()))?;
    match resp.status().as_u16() {
        200..=299 => {}
        404 => return Err(UpdateError::NoReleases),
        403 | 429 => return Err(UpdateError::RateLimited),
        other => return Err(UpdateError::Http(other)),
    }
    let dto: GithubRelease = resp.json().await.map_err(|e| UpdateError::Network(e.to_string()))?;
    let Some(asset) = asset::pick_asset(&dto.assets, arch) else {
        return Ok(None);       // behaviour 2.5 step 3: no timestamp update
    };
    let raw_body = dto.body.unwrap_or_default();      // PROPOSAL: missing body = empty (E-U9)
    Ok(Some(Release {
        version: dto.tag_name,
        notes_markdown: notes::rewrite_mentions(&raw_body),
        release_url: dto.html_url,
        download_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
        sha256: notes::sha256_for(&raw_body, &asset.name),   // checksum is read from the RAW body
    }))
}

#[derive(serde::Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub body: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(serde::Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}
```
(`sha256` is read from the raw body; `notes_markdown` is the body after the mention rewrite. Both come from the same `body` field.)

Hmm — that snippet contains "body_raw_or_empty" which is an invented method; to avoid confusion: keep the raw body in `let raw = dto.body.clone().unwrap_or_default();` before rewriting. I will not fix in the doc further beyond a note. Actually better to correct it now for quality. I'll edit after writing: replace `dto.body.unwrap_or_default()` and `dto.body_raw_or_empty()` lines with raw var approach. Let me plan edit.

### 8.4 Asset selection (src/update/asset.rs)

```rust
use super::check::GithubAsset;

/// Every architecture token the release pipeline may use. An asset with none of these is "Universal".
const ARCH_TOKENS: &[&str] = &["x86_64", "aarch64", "x86", "armeabi-v7a", "arm64-v8a"];
const EXTS: &[&str] = &[".AppImage", ".tar.zst"];

/// Parity with ReleaseServiceImpl.getDownloadLink (ReleaseServiceImpl.kt:37-43):
/// 1) an asset whose name contains the current architecture token;
/// 2) else an asset with no architecture token at all (the Kotlin `map[null]` case).
/// Within a tier, AppImage is preferred over tar.zst; the first match in listing order wins
/// (Kotlin `associate` keeps the LAST match; the change is deliberate and tested).
pub fn pick_asset<'a>(assets: &'a [GithubAsset], arch: &str) -> Option<&'a GithubAsset> {
    let by_ext = |a: &&GithubAsset| EXTS.iter().any(|e| a.name.ends_with(e));
    let with_arch = |a: &&GithubAsset| a.name.contains(arch);
    let without_arch = |a: &&GithubAsset| !ARCH_TOKENS.iter().any(|t| a.name.contains(t));
    assets.iter().filter(by_ext).filter(with_arch).min_by_key(|a| ext_rank(&a.name))
        .or_else(|| assets.iter().filter(by_ext).filter(without_arch).min_by_key(|a| ext_rank(&a.name)))
}

fn ext_rank(name: &str) -> u8 { if name.ends_with(".AppImage") { 0 } else { 1 } }
```
Tie note: `min_by_key` returns the first minimum in iteration order (`Iterator::min_by_key` returns the FIRST element with the minimum? Doc: "If several elements are equally minimum, the first element is returned." Yes.)

Note the `version` parameter is unused in `pick_asset`; remove or use. Implementation may drop it. (I'll keep the signature minimal in the final code: `pick_asset(assets, arch)`.) Since this is design doc, I can fix by editing: change signature to `(assets, arch)`. I'll edit below.

### 8.5 Notes (src/update/notes.rs)

- `rewrite_mentions(s)`: same rule as Kotlin (behaviour 2.5 step 5), regex `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` case-insensitive. The Rust `regex` crate has no lookaround, so this is a hand scan (no new dependency): a `@` whose previous char is not a word char, followed by 1 to 39 characters that are ASCII alphanumerics or single interior hyphens, ending in an alphanumeric. Matches become `[@user](https://github.com/user)`. Test cases are in p1-edge-cases-and-tests.md.
- `strip_checksums_section(s)` for DISPLAY only (behaviour 2.8): removes from the first `---` line that precedes a `Checksums` occurrence to the end. Rust: find the first index of `"---"` at start of a line such that `"Checksums"` appears after it; truncate there. (Kotlin's regex `---(\R|.)*Checksums(\R|.)*` removes the span from the first `---` to the end of the text after the last `Checksums` line-run — equivalent when `Checksums` is the last section; implement as: find the earliest `---` where the remainder contains `Checksums`, and drop from it to the end.) Document this equivalence in tests.
- `sha256_for(raw_body, asset_name)`: reads the `### Checksums` section of the RAW body (live format, checked on the Aniyomi latest release on 2026-10-08): a Markdown table `| Variant | SHA-256 |` followed by rows `| Universal | <64 hex>` and `| arm64-v8a | <64 hex>` (the last cell may lack a closing pipe). The variant label is mapped to the asset: `Universal` = the asset whose name has no architecture token; any other label = the asset whose name contains that label. PROPOSAL extra: also accept the `sha256sum` form `<64 hex>  <asset_name>`. Returns None when no row matches (no verification, the notice says so). Lowercase hex compared case-insensitively.

### 8.6 Download (src/update/download.rs)

```rust
pub struct DownloadRequest {
    pub url: String,
    pub asset_name: String,
    pub version: Option<String>,   // shown in the progress title when present (behaviour 2.9 E-U3 fix)
    pub sha256: Option<String>,
}

pub struct Downloader {
    http: reqwest::Client,         // no total timeout; connect_timeout 30 s (PROPOSAL)
    paths: std::sync::Arc<dyn kuro_platform::Paths>,
    notifier: std::sync::Arc<dyn kuro_platform::Notifier>,
    net: std::sync::Arc<dyn kuro_platform::NetworkStatus>,
    slot: tokio::sync::Mutex<Option<Task>>,
    events: async_channel::Sender<crate::UiEvent>,
}

struct Task { cancel: tokio_util::sync::CancellationToken, handle: tokio::task::JoinHandle<()> }

impl Downloader {
    /// REPLACE semantics: cancels any running download and waits for it to end.
    pub async fn start(self: &std::sync::Arc<Self>, req: DownloadRequest) -> Result<(), UpdateError>;
    pub fn stop(&self);     // CancelAppUpdate
}
```

Algorithm of the task (behaviour 2.9 mapped):
1. If `!net.is_online()`: post progress notice with text "Waiting for network", await the first `true` on `net.online_events()`, then continue (replaces WorkManager `CONNECTED`; metered allowed, behaviour E-U5).
2. Post progress notice (`app-update.progress`, Low priority, icon bundled, buttons: `[Cancel => CancelAppUpdate]`), title = `version` or the app name (`Kurobayomi`), body "Downloading…" (ongoing semantics: Linux notifications have no ongoing flag; PROPOSAL: keep until withdrawn by the task).
3. `dir = paths.update_dir()`; create it; `part = dir.join(sanitised(asset_name) + ".part")`; `final = dir.join(sanitised(asset_name))`.
4. `resp = http.get(url).send()`. Non-2xx => error path (step 9).
5. Stream `resp.bytes_stream()` into `tokio::fs::File` (BufWriter). Each chunk: `bytes += len`; `if let Some(p) = throttle.offer(Instant::now(), bytes, resp.content_length()) { notifier.show(progress notice with body "Downloading {p}%") }`. Also check the cancellation token each chunk.
6. On EOF: flush, `sync_all`, if `sha256` is Some: compute SHA-256 of `part` (stream re-read) and compare case-insensitively; mismatch => `UpdateError::Checksum`, delete `part`, error path.
7. `tokio::fs::rename(part, final)`.
8. `notifier.withdraw(progress id)`; post install notice (`app-update.install`): body "Downloaded {asset_name}"; default action `RevealFile { path: final }`; buttons: `[Open folder => RevealFile{final}, Dismiss => Dismiss{app-update.install}]`; priority Normal.
9. Error path (any `Err`, not cancel): delete `part` (ignore errors); `withdraw(progress)`; post error notice (`app-update.error`, body "Download error", priority Normal, buttons `[Retry => StartAppUpdate{url, version}, Cancel => Dismiss{app-update.error}]`). This withdraws the progress notice, which fixes the stale-ongoing defect (behaviour 2.9 step 7).
10. Cancel path: delete `part`; `withdraw(progress)`; no error notice (parity with `CancellationException` handling).

Empty or invalid URL: `start` returns `UpdateError::NoAsset`/`Network` before spawning (Android: `Result.failure()` silently; PROPOSAL: log at warn and show nothing).

Sanitised name: `kuro_platform::sanitize_file_name(asset_name)` (addenda rule 5; T-req-platform-008). Asset names that fail sanitisation are rejected.

### 8.7 Install step on Linux (replacement of APK install)

- `AppImage` assets: the user runs "Open folder" and replaces the file manually (in-place replacement is out of scope, T-35a-056 Dropped).
- Package-managed installs: the app detects the install kind by checking whether the running executable path is under the package manager's directories (the detection lives in `kuro-platform::install_kind()`; PROPOSAL, T-35a-055). For package-managed installs the check runs, and the prompt body says "Update with your package manager" with only the "What's new" button (no download).

### 8.8 Notices (src/update/notify.rs)

`post_prompt(notifier, release)` posts into the `app-update.progress` slot, the same slot the Android prompt uses (id 1, AppUpdateNotifier.kt:229), so a download started from the prompt replaces it. Specification:
- id: `app-update.progress`; category `AppUpdate`; title "New version available!" (`update.available.title`); body `release.version`; priority Normal; icon `software-update-available` (bundled symbolic name, T-35a-087); default action `OpenUrl { url: release.release_url }`; buttons `[Download => StartAppUpdate { url, version }, What's new => OpenUrl { url: release.release_url }]`. Parity (AppUpdateNotifier.kt:194-199) makes the body click start the download instead; that is recorded as defect E-U4 and changed here. T-35a-048 acceptance states this.

---

## 9. Library CSV export (src/export/library_csv.rs)

```rust
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind { Manga, Anime }

impl EntryKind {
    pub fn csv_name(self) -> &'static str {
        match self { EntryKind::Manga => "manga", EntryKind::Anime => "anime" }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportEntry {
    pub kind: EntryKind,
    pub title: String,
    pub author: Option<String>,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions { pub title: bool, pub kind: bool, pub author: bool, pub artist: bool }

impl Default for ExportOptions {
    fn default() -> Self { Self { title: true, kind: true, author: true, artist: true } }
}

/// Byte-exact port of LibraryExporter.generateCsvData (LibraryExporter.kt:65-95).
pub fn render(entries: &[ExportEntry], opts: ExportOptions) -> String {
    let rows: Vec<String> = entries.iter().map(|e| {
        let mut cells: Vec<String> = Vec::new();
        if opts.title { cells.push(cell(Some(&e.title))); }
        if opts.kind { cells.push(cell(Some(e.kind.csv_name()))); }
        if opts.author { cells.push(cell(e.author.as_deref())); }
        if opts.artist { cells.push(cell(e.artist.as_deref())); }
        cells.join(",")
    }).collect();
    rows.join("\r\n")
}

/// Kotlin: null or isBlank -> "" ; contains \r \n " , -> quoted with doubled quotes ; else raw.
fn cell(value: Option<&str>) -> String {
    match value {
        None => String::new(),
        Some(v) if v.trim().is_empty() => String::new(),
        Some(v) if v.contains(['\r', '\n', '"', ',']) => format!("\"{}\"", v.replace('"', "\"\"")),
        Some(v) => v.to_owned(),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("cannot write {0}: {1}")]
    Write(std::path::PathBuf, std::io::Error),
}

/// Writes atomically: `<name>.kuro-tmp` in the same directory, sync, rename.
pub async fn write_csv(
    path: std::path::PathBuf,
    entries: Vec<ExportEntry>,
    opts: ExportOptions,
) -> Result<usize, ExportError> {
    tokio::task::spawn_blocking(move || {
        let text = render(&entries, opts);
        let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let tmp = path.with_file_name(format!("{file_name}.kuro-tmp"));
        let write = || -> std::io::Result<()> {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            std::fs::rename(&tmp, &path)
        };
        write().map_err(|e| { let _ = std::fs::remove_file(&tmp); ExportError::Write(path.clone(), e) })?;
        Ok(entries.len())
    })
    .await
    .expect("export task panicked")
}
```
`render` returns "" for zero entries and for zero columns with N rows gives N-1 CRLFs (behaviour 3.4). `Some(v) if v.contains([...])` uses the array-of-char pattern (stable since Rust 1.58).

Note on `expect`: a panic in `spawn_blocking` is a bug, not an error path; it is fine to surface it.

Favourites query (area:05-data-repos-entries-items owns the DB; export only consumes):
```sql
SELECT title, author, artist FROM animes WHERE favorite = 1 ORDER BY _id;
SELECT title, author, artist FROM mangas WHERE favorite = 1 ORDER BY _id;
```
Then `entries = anime.map(Anime) ++ manga.map(Manga)` (anime first, behaviour 3.1 step 6).

### 9.1 Export dialog and file chooser (GTK)

- Dialog: `gtk::Window` (modal, transient for the main window) or `gtk::AlertDialog`-like composition: title "Select data to include" (`export.columns_title`), four `gtk::CheckButton`s with labels Title, Type, Author, Artist (`export.*_col`), bottom row buttons Cancel and Save.
- Sensitivity rule (behaviour 3.1 step 3): when the Title toggle becomes inactive, set Type, Author, Artist inactive and insensitive; when it becomes active, make them sensitive (their state stays false until toggled, parity with Kotlin where re-checking Title does not restore them).
- `ExportOptions` lives on the settings screen object (in memory, default all true). Not persisted (behaviour 3.1 step 7).
- Save: `gtk::FileDialog::builder().title("Export library").initial_name("kurobayomi_library.csv").build()`, `set_default_filter` with a `gtk::FileFilter` containing mime `text/csv`, then `dialog.save(Some(&window), gio::Cancellable::NONE, move |res| ...)`. `Err(e)` with `e.matches(gtk::DialogError::Dismissed)` is a silent cancel (behaviour 3.1 step 5).
- On a chosen file: snapshot the favourites on the tokio side at save time (not at dialog open; fixes the stale snapshot, behaviour 3.1 step 6), then `write_csv`. Success => toast `export.done`. Error => toast `export.failed` with the error text.

Replaces: `ActivityResultContracts.CreateDocument("text/csv")` (SettingsDataScreen.kt:372-374) and the `openOutputStream` path.

---

## 10. Wiring into the mockup

- `src/main.rs` `build_ui(app)` (line 22): after `app` exists, `notify::install(app, Rc::new(Router::new(...)))` before the window is presented, so `app.notice` exists before any notification can be clicked. Application id: set in the `gtk::Application` builder (currently not set in `main()`; PROPOSAL: `dev.kurobayomi.Kurobayomi`, a new reverse-DNS id with no existing registration in the repo; packaging task T-35a-011).
- `src/settings.rs` `body(id)` (line 7): the Export group's "Library List" action (spec/settings.json, section "Export", line ~2457-2466) opens the `export_columns` dialog from `spec/settings.json` (`dialogs[id="export_columns"]`, lines 2469-2500). The mockup's generic `dialog_of(id, dialog)` helper (settings.rs:123-127) shows spec dialogs; the export dialog needs real behaviour, so it gets a dedicated builder (T-35a-072) instead of the spec-driven dialog.
- About page: the spec row "Check for updates" (spec/settings.json:3173, notes at line 3197) calls `UpdateService::check(true)` and shows the spinner while the future runs; the result is delivered through `UiEvent`.
- Mockup gaps: no notifications, no router, no updater, no export, no kuro-platform crate (see p1-ui-parity.md).

---

## 11. Android API to Linux/Rust mapping (complete for this scope)

| Android (scope file) | Use | Linux / Rust replacement | Owner trait/module |
|---|---|---|---|
| `NotificationManagerCompat` (Notifications.kt, NotificationExtensions.kt) | post/cancel | `gio::Notification` + `send_notification`/`withdraw_notification` | `Notifier` (kuro-platform) |
| `NotificationChannel`, `NotificationChannelGroup` (Notifications.kt:114-197) | importance, groups, badges, sound | `Category` + per-category enable prefs; priority mapped (section 3.2 of data file); no OS channels | `Category`, `mute.rs` |
| `deleteNotificationChannel` deprecated list (Notifications.kt:96-118) | migration of old channels | none (Dropped, T-35a-012) | - |
| `PendingIntent.getActivity/getBroadcast` (NotificationHandler.kt, NotificationReceiver.kt companion) | deferred actions | GAction `app.notice` with JSON `AppAction` target | `AppAction`, `Router` |
| `BroadcastReceiver` NotificationReceiver (onReceive) | action dispatch on UI thread | `Router::dispatch` on the glib main context; work on tokio | `router.rs` |
| `runBlocking` in openChapter/openEpisode (NotificationReceiver.kt:224-225, 244-245) | blocking DB read on UI thread | async lookup on tokio, result via `async_channel` | `UiEvent` |
| `Intent.ACTION_VIEW` + `setDataAndType` (image, error log, release page) | open with default app | `Opener::open_file` / `reveal_file` / `open_url` | `Opener` |
| `Intent.createChooser` + `ACTION_SEND` (`Uri.toShareIntent`) | share sheet | `Opener::reveal_file` (Dropped share sheet, T-35a-023/024) | `Opener` |
| `PackageInstaller` via `ACTION_VIEW` APK (AppUpdateNotifier.kt:275-300) | install APK | none (Dropped; reveal file) | `Opener` |
| `FileProvider` + `content://` URI (provider_paths.xml, FileExtensions.kt:16) | share/install | plain `PathBuf` | `Paths` |
| `externalCacheDir/update.apk` (AppUpdateDownloadJob.kt:106) | download target | `Paths::update_dir()/<name>` | `Paths` |
| `WorkManager` OneTimeWorkRequest + Constraints CONNECTED + REPLACE (AppUpdateDownloadJob.kt:133-154) | background download | `Downloader` single slot on tokio; `NetworkStatus::is_online` | `Downloader`, `NetworkStatus` |
| `setForeground` + `ForegroundInfo` + `FOREGROUND_SERVICE_TYPE_DATA_SYNC` (AppUpdateDownloadJob.kt:61-71) | keep process alive | none needed (process runs); Dropped | - |
| `ProgressListener` + `newCachelessCallWithProgress` (OkHttpExtensions.kt:119) | progress | reqwest `bytes_stream()` + `content_length()` + `ProgressThrottle` | `throttle.rs` |
| `setProgress(100, p, false)` (AppUpdateNotifier.kt:264) | native progress bar | text in body ("Downloading 42%"); GIO has no progress bar | `throttle.rs` |
| `setOnlyAlertOnce(true)` (AppUpdateNotifier.kt:265) | no re-alert | `Notifier::show` replacement semantics (same id replaces silently; PROPOSAL: low priority) | `Notifier` |
| `setOngoing(true)` (AppUpdateNotifier.kt:244, 283) | non-dismissable | not available in GIO; progress is withdrawn explicitly | `Downloader` |
| `POST_NOTIFICATIONS` runtime check (NotificationExtensions.kt:30-37) | permission | none on Linux; `Notifier::show` returns `NotifyError::Unavailable` when the service is down | `Notifier` |
| `cancelAll()` (NotificationReceiver.kt:193) | clear everything | never used (behaviour 1.5(a)) | - |
| `Dispatchers.IO` / `withIOContext` (AppUpdateChecker, AppUpdateDownloadJob, LibraryExporter) | off-main work | tokio tasks and `spawn_blocking` | tokio |
| `Injekt.get<...>()` (NotificationReceiver.kt:299-300) | DI | explicit `AppContext` fields (addenda rule 3) | app context |
| `Context.toast(...)` (NotificationReceiver.kt:232, SettingsDataScreen.kt:328) | transient text | in-app toast in the window (gtk::Overlay) | `UiEvent::Toast` |
| `Settings > Export > CreateDocument("text/csv")` (SettingsDataScreen.kt:372-374) | save file | `gtk::FileDialog::save` with CSV filter | export dialog |
| `Constants.SHORTCUT_*` intents (core Constants.kt) | deep links from notifications | `AppAction` variants + navigation calls (area:36-ui-shell-navigation-security-webview) | `Router` |
| `android.R.drawable.*`, `R.drawable.ic_*` (Notifier icons) | small icons | bundled symbolic names (`kuro-platform::icons`) | `icons.rs` |
| `R.color.accent_blue` (NotificationExtensions.kt:71) | notification tint | not used (GIO has no tint); theme comes from the app CSS | - |
| `network_security_config.xml` | cleartext + system/user CAs | reqwest defaults + native roots feature | area:12-network-stack client |
| `searchable.xml` | system search UI | in-app search entry (area:40-ui-browse-manga/41); P3 search provider | - |
| S Pen `s_pen_actions.xml` | Samsung remote actions | keyboard shortcuts via `gtk::ShortcutController` (reader/player areas) | - |
| `Intent.FLAG_ACTIVITY_CLEAR_TOP / SINGLE_TOP` (NotificationHandler, NotificationReceiver) | task management | not applicable; router switches tabs in the single window | `Router` |

---

## 12. Error handling

- Library errors: `thiserror` enums per module (`NotifyError`, `OpenError`, `ActionError`, `UpdateError`, `ExportError`). No `unwrap` on user- or network-derived data.
- `anyhow` is not used in library code; the app boundary logs with `log` / `tracing` (PROPOSAL: `log` crate, matching the mockup's dependency set, which has none yet).
- Every Android `catch (e: Exception) { logcat(...) }` maps to `log::error!` plus, when the user asked for the action (manual update check, export), a toast with the message. Background failures never toast (parity: AppUpdateChecker auto path, library notifiers).
- `UpdateError::Checksum` deletes the partial file and shows the error notice (never installs).

---

## 13. Crates (new dependencies and why)

| Crate | Version (PROPOSAL, latest stable at implementation time) | Used for | Why this and not the alternative |
|---|---|---|---|
| `tokio` | 1 (features `rt-multi-thread`, `fs`, `sync`, `macros`) | async runtime for network, DB offload, download task | the plan mandates tokio |
| `tokio-util` | 0.7 | `CancellationToken` | standard cancellation primitive |
| `reqwest` | 0.12 (features `stream`, `rustls-tls-native-roots`, `json`) | GitHub API and asset download | plan mandates reqwest; native roots reproduce system CAs |
| `serde`, `serde_json` | already in mockup | AppAction codec, GitHub DTOs | already present |
| `semver` | 1 | version rule (replaces the Kotlin digit-stripping comparison) | strict ordering with prerelease support |
| `sha2` | 0.10 | optional checksum verification (section 8.6) | small, widely used |
| `hex` | 0.4 | compare checksum strings | avoids hand-rolled hex; PROPOSAL, can be removed if `format!("{:x}")` is used instead |
| `async-channel` | 2 | glib <-> tokio bridge, network events | works without the glib main context |
| `thiserror` | 2 | error enums | standard |
| `log` | 0.4 | logging facade | mockup has no logger yet; `log` is the minimal choice |
| `directories` | 6 | XDG dirs in kuro-platform | addenda names it explicitly |
| `url` | 2 | URL validation for AppAction payloads | parses and checks schemes/hosts |
| `zbus` | 5 (blocking feature) | `FileManager1.ShowItems` in kuro-platform only | the addenda restricts zbus to kuro-platform |
| `glib`, `gio` | via gtk | UI bridge and notifications | already in the GTK stack |

Not used: `csv` (byte-exact parity conflicts with its quoting), `notify-rust` (GIO covers the need inside GTK; addenda allows either), `zbus` (only inside kuro-platform for `FileManager1.ShowItems`), `fancy-regex` (mention rewrite is hand-scanned).

---

## 14. Pitfalls

1. GAction parameter type must match the variant: `Some(glib::VariantTy::STRING)` on the action and `glib::Variant::from(&str)` on the notification target. A mismatch silently drops the click.
2. `gio::Notification` buttons are only shown by servers that support actions; some GNOME-style servers show none. Every notice needs a default action (click the body), which the plan sets for every notice.
3. Notification ids are strings in GIO; the Android integer ids are not reused (section 3.2).
4. `withdraw_all` / `cancelAll` must not appear in the codebase (grep check in T-35a-007).
5. The release asset host (`objects.githubusercontent.com`) is not `github.com`; the URL check must allow both, and redirects from `github.com` land on it (reqwest follows redirects by default; the check runs on the initial URL only, which is the API-provided one).
6. `semver` rejects four-part versions. Kurobayomi versions must be semver (Cargo already requires it).
7. `Notice` must stay `Send` when moved to `glib::MainContext::invoke`. Do not put `gtk` objects in `Notice`.
8. Do not block the glib main loop on the file dialog or the DB. The dialog callback runs on the main thread and only spawns work.
9. Sandboxed builds (Flatpak) route file reveal and URL open through portals automatically; the trait contract does not change, so no `cfg` is added.
10. `Paths::update_dir()` must be created before writing; `tokio::fs::create_dir_all` first.
11. Notification body text must be escaped for markup if the server interprets it (freedesktop servers may render limited markup). Escape `&`, `<`, `>` in titles and bodies (PROPOSAL, in `Notifier::show` for GIO).
12. The About "Check for updates" row must not be clickable while a check runs (Android: `isCheckingUpdates` guard, AboutScreen.kt:113).
