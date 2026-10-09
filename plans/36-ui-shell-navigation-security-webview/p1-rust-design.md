# Area 36 — Rust design: app shell, navigation, lock, WebView, onboarding

Target: Kurobayomi (`/home/blackbox/kurobayomi`), crate `kurobayomi` edition 2024, `gtk4` 0.11 with feature `v4_16` (Cargo.toml). Vanilla GTK 4, no libadwaita. Existing mockup modules: `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/entry.rs`, `src/player.rs`, `src/settings.rs`, `src/spec.rs`, `src/clamp.rs`, data in `spec/app.json` (screens: home, anime_library, manga_library, updates, history, more, download_queue, categories, stats, storage, onboarding, new_update, crash; nav: library, updates, history, browse, more).

Behaviour being implemented: `p1-behavior.md`. Data: `p1-data-and-config.md`. Tasks: `p1-tasks.md`.

---

## 1. Crate and module layout

New modules (all under `src/`):

```
src/shell/mod.rs          Shell: owns window, navigator, tab host, lock gate, toast sink; build()
src/shell/route.rs        Route enum, Tab/TabKind, NavAction, pure route_request()
src/shell/nav.rs          Navigator (stack of Route), pop_until_root, replace, lifecycle hooks
src/shell/tabs.rs         NavStyle, tab lists, start tab, back rule, badges (watch channels)
src/shell/startup.rs      StartupGate (500 ms / 5000 ms), migration await, changelog decision
src/shell/intents.rs      LaunchRequest (CLI args, GApplication open/actions, notifications) -> NavAction
src/shell/banners.rs      AppStateBanners model (downloaded-only, incognito, indexing) + widget
src/shell/theme.rs        ThemeMode, AppTheme (name parsing), CSS class mapping, colour-scheme portal
src/shell/layout.rs       TabletUiMode, Chrome decision from width/height/mode
src/shell/lock.rs         LockConfig, LockState, on_app_stopped/on_app_started/unlock (pure, testable)
src/shell/auth.rs         Authenticator trait; PassphraseAuth (argon2id); FprintdAuth (zbus)
src/shell/unlock_window.rs  Full-window lock overlay (replaces UnlockActivity)
src/shell/onboarding.rs   OnboardingStep trait + 4 steps + wizard host
src/shell/update_prompt.rs NewUpdate page: changelog cleanup, accept/reject/open
src/ui/appbar.rs          HeaderBar builder: title, subtitle (ellipsised), count pill, action mode, actions
src/ui/search.rs          SearchToolbar: gtk::SearchEntry wrapper, placeholder, close/reset, Enter handling
src/ui/tabbed.rs          TabbedScreen (tab row + stack, per-tab search/actions) and TabbedDialog
src/ui/sheet.rs           AdaptiveSheet: modal window (phone/tablet variants), max width 460
src/ui/menu.rs            DropdownMenu fixed width 196, RadioMenuItem, NestedMenuItem, EntryDownloadMenu
src/ui/fab.rs             FloatingActionAddButton with collapse-on-scroll
src/ui/download_indicator.rs  Ring indicator (26 px, 2 px stroke) + long-press
src/ui/empty.rs           EmptyScreen (icon, text, up to 2 actions)
src/ui/date.rs            relative date text (chrono), relative_time / date_format rules
src/web/mod.rs            WebContext per profile, cookie bridge, settings
src/web/window.rs         WebView page (screen and standalone window share it)
src/web/policy.rs         navigation decision, cloudflare marker, header set
src/deeplink/mod.rs       DeepLinkKind, trampolines replaced by intents
src/deeplink/resolve.rs   resolve_anime / resolve_manga (tokio), State enum
src/ui/deeplink.rs        Loading / NoResults / Result page
```

The mockup's `src/ui.rs` already has `nav_init`, `page`, `screen`, `decoration`, `with_spec`; `src/screens.rs` has `body(id)`. The shell calls them rather than replacing them.

`Cargo.toml` additions (proposal; versions to be checked on crates.io at implementation time, pin to the newest stable):
```toml
tokio = { version = "1", features = ["rt-multi-thread", "time", "sync", "macros"] }
async-channel = "2"
futures = "0.3"
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
url = "2"
argon2 = "0.5"
rand_core = { version = "0.6", features = ["getrandom"] }
secrecy = "0.10"
zeroize = "1"
ashpd = { version = "0.11", default-features = false, features = ["tokio"] }   # portals: FileChooser, OpenURI, Notification, Settings
zbus = "5"                                                                        # fprintd (net.reactivated.Fprint)
webkit6 = "0.4"                                                                   # WebKitGTK 6.0 bindings, same GTK4 family as gtk4 0.11
```
Use `webkit6` at the version whose `gtk4` requirement matches `gtk4 = 0.11`; the system package is `webkit2gtk-6.0` (CachyOS/Arch). Use `ashpd` with the `tokio` feature so the portal futures run on the tokio runtime (§3).

Rationale for each crate is in §4.

---

## 2. Key types and signatures

### 2.1 Routes and tabs (`src/shell/route.rs`)

```rust
/// Which kind of catalogue an entry belongs to. Mirrors DeepLinkScreenType and the Tab kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Anime, Manga }

/// Tab identity without payload (equality and NavStyle lookup).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabKind { AnimeLibrary, MangaLibrary, Updates, History, Browse, More }

/// Tab with its one-shot payload, mirrors HomeScreen.Tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tab {
    AnimeLib { anime_id: Option<i64> },
    Library { manga_id: Option<i64> },
    Updates,
    History,
    Browse { to_extensions: bool, anime: bool },
    More { to_downloads: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Home,                                        // root, always stack[0]
    Entry { kind: Kind, id: i64, from_source: bool },
    GlobalSearch { kind: Kind, query: String, filter: Option<String> },
    DeepLink { kind: Kind, query: String },
    Downloads,
    Settings { dest: Option<SettingsDest> },
    RestoreBackup { uri: String },
    ExtensionStores { kind: Kind, url: String },     // Anime: AnimeExtensionStoresScreen; Manga: MangaExtensionReposScreen
    NewUpdate(UpdateInfo),
    Onboarding,
    WebView { url: String, title: Option<String>, source_id: Option<i64> },
}

/// Pure navigation intent produced by routing code; applied to the Navigator/UI by the shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavAction {
    SelectTab(Tab),           // Tab payload is applied as in HomeScreen.openTabEvent
    PopToRoot,
    Push(Route),
    Replace(Route),
    SearchLibrary(String),    // HomeScreen.search
}
```

Only `Route::Home` is ever the root. `Route::Settings` is a `SettingsDest` enum owned by area 47.

### 2.2 Routing as a pure function (`src/shell/intents.rs`)

```rust
/// One inbound request, from GApplication open/activate, CLI, desktop actions or notifications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchRequest {
    /// Desktop action / shortcut. Carries the Constants.kt action name and id if any.
    Shortcut { action: ShortcutAction, id: Option<i64> },
    /// Search via CLI (`--search`), GApplication action, or a text drop.
    Search { query: String, kind: Option<Kind> },
    /// Extension-driven search (INTENT_SEARCH / INTENT_ANIMESEARCH equivalent).
    ExtSearch { kind: Kind, query: String, filter: Option<String> },
    /// File opened with the desktop (MIME application/x-tachibk).
    OpenFile(std::path::PathBuf),
    /// URI opened (tachiyomi://, aniyomi://).
    OpenUri(url::Url),
    /// Notification action: dismiss id and group id (GNotification withdraw).
    Notification { id: u32, group_id: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    AnimeLib, Library, AnimeEntry, MangaEntry, Updates, History,
    Catalogues, AnimeExtensions, Extensions, Downloads, AnimeDownloads,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RouteError {
    #[error("missing id for shortcut {0:?}")]
    MissingId(ShortcutAction),
}

/// Pure. Returns the NavActions to apply in order, plus whether the window may be shown
/// (`ready`), mirroring MainActivity.handleIntentAction.
pub fn route_request(req: &LaunchRequest) -> Result<(Vec<NavAction>, bool), RouteError>;
```

`route_request` must reproduce behaviour §4 exactly, including: a `Shortcut` with a missing id returns `Err(MissingId)` (Kotlin: `return false`, and the caller must still show the window, see §6.1); `.tachibk` file -> `[PopToRoot, Push(Route::RestoreBackup)]`; `Search` with empty query -> `(vec![], true)`.

The `kind` default for `Search` is `Kind::Anime` (behaviour §4.2). An unknown `type` string in the Kotlin throws; in Rust the parse is done in the caller and maps to `Err` with a toast.

### 2.3 Navigator (`src/shell/nav.rs`)

```rust
pub struct Navigator { stack: Vec<Route> }

impl Navigator {
    pub fn new() -> Self { Self { stack: vec![Route::Home] } }
    pub fn last(&self) -> &Route;                   // Voyager lastItem
    pub fn push(&mut self, r: Route);
    pub fn pop(&mut self) -> Option<Route>;         // never pops the Home root
    pub fn replace(&mut self, r: Route);            // replaces the top entry
    pub fn pop_until_root(&mut self);
    pub fn len(&self) -> usize;
    /// Pages to drop when popped (disposeSteps = true). Returned for the UI to destroy widgets.
    pub fn apply(&mut self, a: NavAction) -> Vec<Route>;
}
```
Invariant: `stack[0] == Route::Home` always; `pop` on length 1 returns `None`.

### 2.4 Tabs and badges (`src/shell/tabs.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavStyle { MoveMangaToMore, MoveUpdatesToMore, MoveHistoryToMore, MoveBrowseToMore }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartScreen { Anime, Manga, Updates, History, Browse }

impl NavStyle {
    pub fn more_tab(self) -> TabKind;   // MoveMangaToMore -> MangaLibrary, MoveUpdatesToMore -> Updates,
                                        // MoveHistoryToMore -> History, MoveBrowseToMore -> Browse
    pub fn bar_tabs(self) -> Vec<TabKind>;  // [AnimeLibrary, MangaLibrary, Updates, History, Browse, More] minus more_tab()
}

impl StartScreen {
    pub fn tab(self) -> TabKind;   // Anime->AnimeLibrary, Manga->MangaLibrary, Updates, History, Browse
}

/// HomeScreen.goToStartScreen
pub fn start_target(default: TabKind, more: TabKind) -> TabKind {
    if default != more { default } else { TabKind::AnimeLibrary }
}

/// HomeScreen BackHandler enabled rule, see behaviour §2.6.
pub fn back_enabled(current: TabKind, default: TabKind, more: TabKind) -> bool {
    let a = current == more;
    let b = current != default;
    let c = current != TabKind::AnimeLibrary;
    let d = default != more;
    (a || b) && (c || d)
}

/// Bar is hidden when the current tab is the one moved into More (behaviour §2.3).
pub fn bar_visible(current: TabKind, more: TabKind, requested: bool) -> bool {
    requested && current != more
}

/// Badge model, reactive. Values come from preferences (sources listed in p1-data-and-config.md §3).
pub struct Badges {
    pub updates: tokio::sync::watch::Receiver<u32>,  // (anime + manga) if show_updates_count, else 0
    pub extensions: tokio::sync::watch::Receiver<u32>, // manga + anime extension updates
}
pub fn updates_badge(anime: u32, manga: u32, show: bool) -> u32 { if show { anime + manga } else { 0 } }
```
`start_tab` is read once at startup in Kotlin (behaviour §2.1). Proposal: read it on every `Home` build so a change applies on next open, and document it.

### 2.5 Startup gate (`src/shell/startup.rs`)

```rust
pub struct StartupGate { started: std::time::Instant, ready: std::cell::Cell<bool> }  // GTK thread only
pub const MIN_SHOW: std::time::Duration = std::time::Duration::from_millis(500);
pub const MAX_WAIT: std::time::Duration = std::time::Duration::from_millis(5000);

impl StartupGate {
    /// true while the placeholder spinner should keep the window hidden (Kotlin keepOnScreen).
    pub fn keep_hidden(&self, now: std::time::Instant) -> bool {
        let e = now.duration_since(self.started);
        e <= MIN_SHOW || (!self.ready.get() && e <= MAX_WAIT)
    }
    pub fn mark_ready(&self) { self.ready.set(true) }
}

/// Changelog decision (MainActivity.kt:294): shown once, after a migration, in non-debug builds.
pub fn show_changelog(did_migrate: bool, debug_build: bool) -> bool { did_migrate && !debug_build }
```
Note the logic is the Kotlin's: `keep_hidden` is true when `e <= MIN` even if ready. The window is presented on the first GTK tick where `keep_hidden` is false.

### 2.6 Lock (`src/shell/lock.rs`)

```rust
pub const LAST_CLOSED_KEY: &str = "__APP_STATE_last_app_closed";
pub const MS_PER_MINUTE: i64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockConfig { pub enabled: bool, pub lock_after_minutes: i32 }   // -1 never, 0 always, >0 minutes

/// Process-global, in memory only. Initial value true (always lock after a kill).
#[derive(Debug)]
pub struct LockState { pub require_unlock: bool, pub authenticating: bool }
impl Default for LockState { fn default() -> Self { Self { require_unlock: true, authenticating: false } } }

/// Storage for the single timestamp (backed by the app_state table, area 02).
pub trait AppStateKv {
    fn get_i64(&self, key: &str) -> Option<i64>;
    fn set_i64(&mut self, key: &str, value: i64);
    fn delete(&mut self, key: &str);
}

/// Kotlin SecureActivityDelegate.onApplicationStopped
pub fn on_app_stopped(cfg: LockConfig, st: &LockState, kv: &mut dyn AppStateKv, now_ms: i64) {
    if !cfg.enabled || st.authenticating || st.require_unlock { return; }
    if cfg.lock_after_minutes > 0 { kv.set_i64(LAST_CLOSED_KEY, now_ms); }
}

/// Kotlin SecureActivityDelegate.onApplicationStart
pub fn on_app_started(cfg: LockConfig, st: &mut LockState, kv: &mut dyn AppStateKv, now_ms: i64) {
    if cfg.enabled && !st.authenticating && !st.require_unlock {
        st.require_unlock = match cfg.lock_after_minutes {
            -1 => false,
            0 => true,
            m => kv.get_i64(LAST_CLOSED_KEY).unwrap_or(0) + i64::from(m) * MS_PER_MINUTE <= now_ms,
        };
    }
    if cfg.enabled { kv.delete(LAST_CLOSED_KEY); }   // Kotlin deletes after the block, after the enabled check
}

pub fn unlock(st: &mut LockState) { st.require_unlock = false; }
```

Note: Kotlin's `onApplicationStart` deletes `lastClosedPref` regardless of `requireUnlock`, but only after the `useAuthenticator` early return. The `if cfg.enabled` at the end is that early return. Same semantics.

Edge with `m` values other than -1/0: Kotlin's `else` branch covers them. The `m =>` arm does too.

### 2.7 Authenticator (`src/shell/auth.rs`)

```rust
use futures::future::BoxFuture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome { Success, Failed, Cancelled, Unavailable(String) }

pub trait Authenticator: Send + Sync {
    fn name(&self) -> &'static str;                       // "passphrase" | "fprintd"
    fn available(&self) -> BoxFuture<'static, bool>;
    fn verify(&self, reason: &str) -> BoxFuture<'static, AuthOutcome>;
}

/// Argon2id PHC stored in $XDG_CONFIG_HOME/kurobayomi/lock.json (mode 0600).
pub struct PassphraseAuth { phc_path: std::path::PathBuf }
impl PassphraseAuth {
    pub fn set(&self, new: &secrecy::SecretString) -> anyhow::Result<()>;   // argon2id, OsRng salt, 0600 write
    pub fn check(&self, candidate: &secrecy::SecretString) -> anyhow::Result<bool>;
}

/// fprintd over system D-Bus: net.reactivated.Fprint.Device.VerifyStart / VerifyStatus.
pub struct FprintdAuth;
```

Parameters for argon2id (proposal): `m_cost = 19456 KiB, t_cost = 2, p_cost = 1` (the OWASP minimum set). Verify uses `argon2::PasswordVerifier` with constant-time comparison.

### 2.8 Theme (`src/shell/theme.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode { Light, Dark, System }

/// Stored by name (e.g. "MONET"). Unknown names fall back to Default. Deprecated names
/// BLUE, DARK_BLUE, HOT_PINK are kept for backup compatibility and map to Default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppTheme {
    Default, Monet, Cloudflare, CottonCandy, Doom, GreenApple, Lavender, Matrix, MidnightDusk,
    Mocha, Sapphire, Nord, StrawberryDaiquiri, Tako, TealTurquoise, TidalWave, YinYang, Yotsuba,
    Monochrome,
}
impl AppTheme {
    pub fn parse(name: &str) -> Self;
    pub fn stored_name(self) -> &'static str;
    /// Linux: Monet has no meaning -> Default. Visual palette is the GTK theme.
    pub fn effective(self) -> Self;
}

/// CSS class added to the window: "kuro-amoled" when the amoled flag is on and the mode is dark.
pub fn css_classes(amoled: bool, dark: bool) -> Vec<&'static str>;

/// ThemeMode -> gtk::Settings preference. System follows the portal color-scheme.
pub fn apply_color_scheme(mode: ThemeMode, portal_dark: Option<bool>);
```
`portal_dark`: `ashpd::desktop::settings::Settings::color_scheme()` returns `PreferDark` (1) / `PreferLight` (2) / `NoPreference` (0). Map `PreferDark -> Some(true)`, `PreferLight -> Some(false)`, `NoPreference -> None` (then GTK default).

### 2.9 Layout (`src/shell/layout.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabletUiMode { Automatic, Always, Landscape, Never }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chrome { Sidebar, BottomBar }

pub const WIDE_MIN_WIDTH_PX: i32 = 760;   // mockup main.rs threshold

pub fn chrome_for(mode: TabletUiMode, width: i32, height: i32, maximized: bool) -> Chrome {
    let wide = width >= WIDE_MIN_WIDTH_PX || maximized;
    match mode {
        TabletUiMode::Automatic => if wide { Chrome::Sidebar } else { Chrome::BottomBar },
        TabletUiMode::Always => Chrome::Sidebar,
        TabletUiMode::Landscape => if width > height { Chrome::Sidebar } else { Chrome::BottomBar },
        TabletUiMode::Never => Chrome::BottomBar,
    }
}
```
Consider the mockup behaviour: `wide` also hides the sidebar when the reader/player is on screen (`immersive`). Keep that rule in `Shell`, not in `chrome_for`.

### 2.10 Banners (`src/shell/banners.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerState { pub downloaded_only: bool, pub incognito: bool, pub indexing: bool }

/// Order: indexing, downloaded-only, incognito (top to bottom). Background is tokenised by kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerKind { Indexing, DownloadedOnly, Incognito }
pub fn visible(state: BannerState) -> Vec<BannerKind>;

/// Status-bar tint rule from MainActivity.kt:184-189 (first match wins). Used for the header bar accent.
pub fn accent(state: BannerState) -> Option<BannerKind> {
    if state.indexing { Some(BannerKind::Indexing) }
    else if state.downloaded_only { Some(BannerKind::DownloadedOnly) }
    else if state.incognito { Some(BannerKind::Incognito) } else { None }
}
```

### 2.11 WebView (`src/web/*`)

```rust
pub struct WebProfile { pub context: webkit6::WebContext, pub cookies: webkit6::CookieManager }

pub enum NavDecision { Allow, Block }

/// Mirrors WebViewScreenContent.shouldOverrideUrlLoading (proposal: headers set once, no reload).
pub fn decide_navigation(url: &url::Url) -> NavDecision {
    if url.as_str().starts_with("blob:http") { return NavDecision::Allow; }
    if url.as_str().starts_with("intent://") { return NavDecision::Block; }
    NavDecision::Allow
}

pub const CLOUDFLARE_MARKERS: [&str; 2] = ["window._cf_chl_opt", "Ray ID is"];
pub fn looks_like_cloudflare(html: &str) -> bool { CLOUDFLARE_MARKERS.iter().any(|m| html.contains(m)) }

/// Sets the five WebView settings from WebViewUtil.setDefaultSettings (no Android equivalents dropped).
pub fn apply_settings(view: &webkit6::WebView, user_agent: Option<&str>, debug: bool);
/// Remove cookies for a host (cookieJar.remove(url) equivalent), returns the number removed.
pub async fn clear_cookies_for(cookies: &webkit6::CookieManager, host: &str) -> usize;
```
`apply_settings` mapping: `javascript_enabled = true`; `enable_developer_extras = debug`; `enable_smooth_scrolling = true`; `zoom` controls are the WebKit default (`set_zoom_level`); `user_agent` from source headers `user-agent` if present. The DOM storage is WebKit-default enabled, so there is no direct flag: document this. The `cacheMode = LOAD_DEFAULT` maps to the default `WebContext` cache model `WebKitCacheModel::DocumentBrowser`.

### 2.12 Deep links (`src/deeplink/*`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    NoResults,
    Entry { entry_id: i64, child_id: Option<i64> },   // child_id = episode id or chapter id
}

pub async fn resolve_anime(query: &str, ctx: &ResolveCtx) -> anyhow::Result<Resolved>;
pub async fn resolve_manga(query: &str, ctx: &ResolveCtx) -> anyhow::Result<Resolved>;

/// Shared dependencies, owned by areas 05, 07, 15, 16.
pub struct ResolveCtx { /* source registry, repos, update-from-remote */ }
```
`resolve_*` must return `Err` on failure (Kotlin silently stays loading; Proposal adds the error state, see §6.4). The UI state enum:

```rust
pub enum DeepLinkPage { Loading, NoResults, Failed(String), Done(Resolved) }
```

### 2.13 Onboarding (`src/shell/onboarding.rs`)

```rust
pub trait OnboardingStep {
    fn is_complete(&self) -> bool;
    fn build(&self) -> gtk::Widget;
}
pub struct Onboarding { pub step: usize, pub steps: Vec<Box<dyn OnboardingStep>> }
impl Onboarding {
    pub const LAST: usize = 3;
    pub fn accept_label(&self) -> &'static str { if self.step == Self::LAST { "Finish" } else { "Next" } }
    pub fn back(&mut self) -> bool;          // returns true if handled (step > 0)
    pub fn accept(&mut self) -> Accept;       // Next or Finish
}
pub enum Accept { Advanced, Finished }
```
The steps are `ThemeStep`, `StorageStep`, `DesktopStep` (replaces PermissionStep), `GuidesStep`. `StorageStep::is_complete()` reads `storage_dir` from config (`Option<PathBuf>`).

---

## 3. Threading and async model

- GTK main thread: every widget mutation, `Navigator::apply`, lock overlay, toasts, `gio::Notification`. No blocking calls.
- tokio runtime: built once in `main` (`tokio::runtime::Builder::new_multi_thread().enable_all()`), stored in a `OnceLock`. Background work: migrations (area 11), update check, deep-link resolve, cookie IO, portal calls (ashpd with tokio feature), fprintd D-Bus (zbus).
- Bridge: `glib::MainContext::default().spawn_local` for UI tasks awaiting an async result; or `async_channel::bounded(16)` filled from tokio and drained with `glib::spawn_future_local`. Pattern:
  ```rust
  let (tx, rx) = async_channel::bounded::<DeepLinkPage>(4);
  rt.spawn(async move { let _ = tx.send(run_resolve(q).await).await; });
  glib::spawn_future_local(async move { while let Ok(p) = rx.recv().await { page.set(p); } });
  ```
- Migration gate: `shell::startup` awaits the migration future (from tokio) before presenting the window. This replaces `Migrator.awaitAndRelease()` `runBlocking`.
- Lock: `on_app_stopped` runs on `Application::shutdown` (or window close), `on_app_started` on `activate` after the window is built. Both are synchronous, on the GTK thread, and only touch the KV store.
- Cancellation: deep-link resolve is spawned with a `tokio_util::sync::CancellationToken` cancelled when the page is dropped (Kotlin's `screenModelScope` does the same). Add `tokio-util` if used.

---

## 4. Crate choices (why)

| Need | Choice | Reason |
|---|---|---|
| UI toolkit | gtk4 0.11 (existing) | Mandated: vanilla GTK 4, theme applies |
| WebView | webkit6 (WebKitGTK 6.0) | The only maintained WebKit binding for GTK 4 on Linux; supports cookie manager and `decide-policy` |
| Portals (file chooser, open URI, notification, colour scheme) | ashpd | Sandbox-safe; works under Flatpak and native; avoids direct GTK dialogs for folder choice |
| Passphrase | argon2 (RustCrypto) | Memory-hard, PHC string storage |
| fprintd | zbus | Pure-Rust D-Bus, async |
| Secrets in memory | secrecy + zeroize | Avoids accidental Debug/log leaks of the passphrase |
| Async runtime | tokio | Mandated |
| Dates | chrono (clock feature) | Local time without the `time` crate's soundness-sensitive local-offset path |
| URLs | url | Parsing for scheme checks (`blob:`, `intent://`) and query params |
| Errors | thiserror (typed module errors), anyhow (top-level only) | Standard split |
| Logging | tracing | Replaces logcat |

Not used: libadwaita (forbidden), GLib `GSettings` for prefs (prefs live in the SQLite app-state/preference store, area 02), a separate DI container (Kotlin Injekt), Voyager/Compose equivalents (replaced by `Navigator` + GTK widgets).

---

## 5. Mapping: Android to Linux

| Android concept (file) | Linux / Rust replacement |
|---|---|
| `Activity`, `setComposeContent` (MainActivity, BaseActivity) | `gtk::ApplicationWindow` built in `shell::Shell::build` |
| `Navigator` / Voyager screens (Navigator.kt) | `shell::nav::Navigator` + `gtk::Stack` keyed by route id |
| `TabNavigator` "HomeTabs" (HomeScreen.kt) | `gtk::Stack` of tab pages; `TabKind` ids |
| `NavigationBar` / `NavigationRail` | `gtk::StackSwitcher` (narrow) / `gtk::ListBox` sidebar (wide) |
| `isTabletUi` + `prepareTabletUiContext` (DisplayExtensions.kt) | `layout::chrome_for` |
| `SplashScreen` (core splashscreen lib) | spinner placeholder + `StartupGate` |
| `Intent` extras / actions (Constants.kt) | `LaunchRequest` + `route_request` |
| `onNewIntent` `callbackFlow` | GApplication `open` / `activate` signals |
| `ACTION_VIEW` with `.tachibk` | `.desktop` MIME `application/x-tachibk` + `open` |
| `ACTION_VIEW` `tachiyomi://`, `aniyomi://` | `.desktop` `x-scheme-handler/tachiyomi;x-scheme-handler/aniyomi;` + `open` |
| `ACTION_SEARCH`, `ACTION_SEND text/plain` | `--search` CLI argument, Browse page drop target |
| Static shortcuts XML (`@xml/shortcuts`) | `.desktop` `[Desktop Action]` entries + `gio::SimpleAction` on the app |
| `NotificationReceiver.dismissNotification` | `gio::Notification` with `default-action` and `withdraw_notification` |
| `BiometricPrompt` + `AuthenticatorUtil` | `auth::Authenticator` (passphrase, fprintd) |
| `FLAG_SECURE` (`Window.setSecureScreen`) | dropped; overlay blanking on lock (pitfall §7) |
| `WebView` (`AccompanistWebView`) | `webkit6::WebView` in `web::window` |
| `CookieManager` + `NetworkHelper.cookieJar` | `webkit6::CookieManager` + sync to reqwest jar (area 12) |
| `Intent.ACTION_SEND` share | copy to clipboard + toast (portal share not used) |
| `Intent.ACTION_VIEW` open in browser (forceDefaultBrowser) | `gio::AppInfo::launch_default_for_uri` |
| `ActivityResultContracts.StartActivityForResult` (external player) | child process wait on mpv/external player (area 45/46) |
| `ActivityResultContracts.RequestPermission` | dropped (no runtime permissions on Linux) |
| `ACTION_MANAGE_UNKNOWN_APP_SOURCES`, `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | dropped |
| SAF folder picker (`storageLocationPicker`) | `ashpd` FileChooser (`directory(true)`) |
| `Dispatchers.IO` / `launchIO` | `tokio::task::spawn_blocking` or async tasks on the runtime |
| `lifecycleScope` / `LaunchedEffect` | `glib::spawn_future_local` tied to the widget (cancel on destroy) |
| `BackHandler` | `gtk::EventControllerKey` on Escape + window `close-request` handling |
| `Haptic feedback` (`HapticFeedback`) | dropped |
| `Edge-to-edge` / system bar styling | dropped (GTK title bar) |
| Edge-to-edge navigation scrim | dropped |
| Android `Theme.Tachiyomi_*` styles | CSS named colours on `:root`-equivalent window class (`kuro-amoled`) |
| `Dynamic colour` (Monet) | not available; `AppTheme::effective()` maps to Default |
| `Migrator.awaitAndRelease` (runBlocking) | awaited future before window present |
| `AssistContent` | dropped |
| Circular reveal, ViewPager, Spinner, NumberPicker, SeekBar | GTK widgets from shared UI (not part of this area's port) |

---

## 6. Interaction with the existing mockup

### 6.1 `src/main.rs`
- `main()` (lines 12-19): keep. Add: create the tokio runtime; build `Shell` on `connect_activate`; add `connect_open` (for `.tachibk` and URIs) and `set_accels` for keyboard (§8).
- `build_ui` (lines 22-~70): replace the hand-built `nav` Stack and `sidebar`/`bottom` with `shell::Shell::build(app)` that returns a struct holding the same widgets. Keep the immersive rule (sidebar hidden when `screen:reader` or `screen:player` is visible) and the `refresh` closure, now driven by `chrome_for`.
- `tabs()` (lines 141-152): iterates `spec.nav` ids. Replace with the `TabKind` list from `NavStyle::bar_tabs()`. Keep the page builder `ui::page(&title, &b.content, b.end, false)`.
- `sidebar()` (lines 154+): keep. Its badges (`updates` "8", `browse` "1" hard-coded in `sidebar()` at src/main.rs:175) must be replaced with the live `Badges` receivers (§2.4) — this is a required change, the mockup's badge values are placeholders.

### 6.2 Nav ids and the two library tabs
The mockup's `spec.nav` has `library` (one tab, with an anime/manga `kind_tabs` toggle in `screens::libraries`). Aniyomi has two separate bar entries, `AnimeLibraryTab` and `MangaLibraryTab`, and `StartScreen` / `NavStyle` depend on them. Proposal: change `spec/app.json` `nav` to `anime_library`, `manga_library`, `updates`, `history`, `browse`, `more` (the screens with those ids already exist in `spec/app.json` `screens`), and let `NavStyle` remove one of them. Keep `screens::libraries` for the shared library body. This is a required change to match behaviour §2.4.

### 6.3 `src/ui.rs`
- `nav_init(&nav, &win)` (existing): keep; the shell calls it once.
- `page(title, content, end, _)`: reused for every tab page. Its header uses the new `ui::appbar` builder.
- `screen(id)`: used for titles. The `onboarding` screen (title "Welcome!") and `new_update` (title "New version available!") are used by `shell::onboarding` and `shell::update_prompt`.
- `decoration(bool)`: header button layout, keep.

### 6.4 `src/spec.rs`
- Types `Screen`, `Sheet`, `Dialog`, `Group`, `Item`: unchanged. Add an optional `nav_hidden_when: Option<String>` to `Nav` only if needed; not required by this area.

### 6.5 `src/screens.rs`
- `body(id)` returns `Option<Built>`. `nav` ids handled by `tabs()`. Add arms for `onboarding`, `new_update`, `webview`, `deeplink` ids (the mockup already has `fn webview()` in `src/entry.rs:572`; use it for `Route::WebView`).

### 6.6 `src/entry.rs`
- `webview()` (line 572) returns the static mockup. Replace with `web::window::WebPage`. `migrate_*` and `global_search` are out of this area.

### 6.7 Mockup gaps versus this area (all must be closed)
1. No navigation stack (only one Stack of tabs). Add `Navigator`.
2. No lock. Add `shell::lock` + `unlock_window`.
3. No deep-link or external-open routing. Add `route_request` and GApplication `open`.
4. Badges are hard-coded (`"8"`, `"1"`).
5. Onboarding is a spec screen only; no gate and no stepper. Add `shell::onboarding`.
6. No update prompt wiring (screen exists, no route).
7. WebView is a static mockup.
8. Theme: no `ThemeMode` / `AppTheme` plumbing.
9. No startup gate (the window appears immediately, Kotlin keeps a splash up to 500 ms).

---

## 7. Pitfalls

1. Lock privacy: when locking, first replace the content with an empty page and hide the content widgets before presenting the overlay. Compositors may keep a snapshot of the window for the task switcher (no FLAG_SECURE equivalent). Document this in the security notes.
2. Never call `block_on` inside a GTK callback. The migration and the lock prompt must be futures.
3. `DeepLinkScreenType::valueOf` in Kotlin throws for unknown types. In Rust parse with `FromStr` and return `RouteError`; never `unwrap`.
4. The `Navigator::pop` must never remove `Route::Home`; the Kotlin BackHandler guard (behaviour §2.6) must be used, not "pop until empty".
5. The WebView cookie bridge: two stores (WebKit cookies, reqwest jar). Pick one source of truth per host or sync on navigation finish. Document the choice in area 12.
6. `webkit6::WebView` must be created on the GTK thread and only after `WebContext` exists; keep one `WebContext` per profile, not per tab.
7. `gtk::FileDialog` (GTK 4.10+) vs ashpd: use ashpd for sandbox compatibility; fall back to `gtk::FileDialog` if the portal is missing.
8. GApplication `open` may be called before `activate`. Queue requests until the window exists (startup gate).
9. The mockup sets `NON_UNIQUE` when `KUROBAYOMI_SMOKE|OPEN|SHOT` env vars are set (src/main.rs:15-16). Keep it; it is a dev-only path.
10. Keep both the lock state and its timestamp in the same DB transaction if they are written together (the app_state KV, area 02) so a crash cannot leave `require_unlock` inconsistent with the stored timestamp.

---

## 8. Keyboard and pointer (desktop replacement for gestures)

| Action | Key | Source behaviour |
|---|---|---|
| Back (pop, or go to start tab) | Escape (and mouse button 8 / back) | BackHandler (behaviour §2.6) |
| Select bar tab N (1..5) | Alt+1..Alt+5 (position in `bar_tabs()`) | Proposal |
| Search in current page | Ctrl+F | SearchToolbar open (behaviour §5.6) |
| Reset search | Escape when search is non-empty | SearchToolbar reset (§5.6) |
| Cancel action mode | Escape | AppBar close icon (§5.3) |
| Open settings | Ctrl+, | Proposal (More tab reselect opens Settings, §2.7) |
| Lock now | Ctrl+L | Proposal |
| Quit | Ctrl+Q | GTK default |

Use `gtk::Application::set_accels_for_action` so the bindings show in menus and are remappable (area 47 keybinding setting, if added).
